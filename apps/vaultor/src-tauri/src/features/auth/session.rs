use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use secrecy::{ExposeSecret, SecretVec};
use tokio::sync::oneshot;

use crate::error::VaultError;

/// Status returned to the frontend (no key material, no token).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionStatus {
    pub active: bool,
    pub expires_at_ms: Option<u64>,
}

struct SessionInner {
    /// `None` means UntilQuit (no time-based expiry).
    expires_at: Option<SystemTime>,
    /// Cancels the background expiry task when dropped or replaced.
    cancel_tx: Option<oneshot::Sender<()>>,
    /// Encryption key held in memory for the session duration.
    /// Zeroized when the session is invalidated.
    key: Option<SecretVec<u8>>,
}

impl Drop for SessionInner {
    fn drop(&mut self) {
        self.wipe();
    }
}

impl SessionInner {
    fn wipe(&mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
        // SecretVec zeroizes its contents on drop.
        drop(self.key.take());
        self.expires_at = None;
    }

    /// True when key is present and time limit (if any) has not elapsed.
    fn is_active(&self) -> bool {
        match (self.expires_at, self.key.as_ref()) {
            (None, Some(_)) => true, // UntilQuit
            (Some(t), Some(_)) => SystemTime::now() < t,
            _ => false,
        }
    }
}

pub struct SessionStore {
    inner: Mutex<SessionInner>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(SessionInner {
                expires_at: None,
                cancel_tx: None,
                key: None,
            }),
        }
    }

    /// Start a new session.
    ///
    /// `ttl = None` means UntilQuit: the session stays active until
    /// `invalidate()` is called or the process exits.
    ///
    /// Returns `(expires_at_ms, cancel_receiver)`:
    /// - `expires_at_ms` is `None` for UntilQuit sessions.
    /// - The caller MUST listen on `cancel_rx` if it spawns a timer task;
    ///   for UntilQuit sessions no timer task should be spawned (but the
    ///   receiver may simply be dropped).
    pub fn create(
        &self,
        key_bytes: [u8; 32],
        ttl: Option<Duration>,
    ) -> (Option<u64>, oneshot::Receiver<()>) {
        let (cancel_tx, cancel_rx) = oneshot::channel();

        let expires_at = ttl.map(|d| SystemTime::now() + d);
        let expires_at_ms =
            expires_at.map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64);

        let mut inner = self.inner.lock().unwrap();
        // Cancel any existing session timer
        if let Some(old_tx) = inner.cancel_tx.take() {
            let _ = old_tx.send(());
        }
        inner.expires_at = expires_at;
        inner.cancel_tx = Some(cancel_tx);
        inner.key = Some(SecretVec::new(key_bytes.to_vec()));

        (expires_at_ms, cancel_rx)
    }

    /// Check whether the session is currently active.
    pub fn is_valid(&self) -> bool {
        self.inner.lock().unwrap().is_active()
    }

    /// Retrieve the encryption key if the session is valid.
    pub fn get_key(&self) -> Result<Vec<u8>, VaultError> {
        let inner = self.inner.lock().unwrap();
        if inner.is_active() {
            Ok(inner.key.as_ref().unwrap().expose_secret().to_vec())
        } else {
            Err(VaultError::SessionExpired)
        }
    }

    /// Explicitly invalidate the session (lock_vault).
    pub fn invalidate(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.wipe();
    }

    /// Return the current status (safe to send to frontend).
    pub fn status(&self) -> SessionStatus {
        let inner = self.inner.lock().unwrap();
        match (inner.expires_at, inner.key.as_ref()) {
            // UntilQuit: active, no expiry timestamp
            (None, Some(_)) => SessionStatus {
                active: true,
                expires_at_ms: None,
            },
            // Timed session still valid
            (Some(t), Some(_)) if SystemTime::now() < t => {
                let ms = t.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
                SessionStatus {
                    active: true,
                    expires_at_ms: Some(ms),
                }
            }
            // Expired or no session
            _ => SessionStatus {
                active: false,
                expires_at_ms: None,
            },
        }
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn make_store() -> SessionStore {
        SessionStore::new()
    }

    #[test]
    fn new_session_is_inactive() {
        let store = make_store();
        assert!(!store.is_valid());
        assert!(store.get_key().is_err());
    }

    #[test]
    fn session_active_after_create() {
        let store = make_store();
        let key = [1u8; 32];
        store.create(key, Some(Duration::from_secs(120)));
        assert!(store.is_valid());
        assert!(store.get_key().is_ok());
    }

    #[test]
    fn invalidate_clears_session() {
        let store = make_store();
        store.create([2u8; 32], Some(Duration::from_secs(120)));
        assert!(store.is_valid());
        store.invalidate();
        assert!(!store.is_valid());
        assert!(store.get_key().is_err());
    }

    #[test]
    fn session_expired_after_zero_ttl() {
        let store = make_store();
        store.create([3u8; 32], Some(Duration::from_secs(0)));
        std::thread::sleep(Duration::from_millis(10));
        assert!(!store.is_valid());
        assert!(store.get_key().is_err());
    }

    #[test]
    fn get_key_returns_correct_bytes() {
        let store = make_store();
        let expected = [0xDEu8; 32];
        store.create(expected, Some(Duration::from_secs(120)));
        let got = store.get_key().unwrap();
        assert_eq!(got, expected.to_vec());
    }

    #[test]
    fn second_create_cancels_first() {
        let store = make_store();
        store.create([10u8; 32], Some(Duration::from_secs(120)));
        store.create([20u8; 32], Some(Duration::from_secs(120)));
        let key = store.get_key().unwrap();
        assert_eq!(key, vec![20u8; 32]);
    }

    #[test]
    fn until_quit_session_is_valid_indefinitely() {
        let store = make_store();
        store.create([5u8; 32], None); // UntilQuit
        assert!(store.is_valid());
        assert!(store.get_key().is_ok());
    }

    #[test]
    fn until_quit_session_invalidated_by_lock() {
        let store = make_store();
        store.create([6u8; 32], None);
        assert!(store.is_valid());
        store.invalidate();
        assert!(!store.is_valid());
        assert!(store.get_key().is_err());
    }

    #[test]
    fn until_quit_status_has_null_expiry() {
        let store = make_store();
        store.create([7u8; 32], None);
        let s = store.status();
        assert!(s.active);
        assert!(s.expires_at_ms.is_none());
    }
}
