use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, State};

use crate::error::VaultError;
use crate::features::auth::session::{SessionStatus, SessionStore};
use crate::features::keychain::KeyProvider;
use crate::features::settings::config::AppSettings;

/// Authenticate via TouchID, retrieve the encryption key from the Keychain,
/// and create a session whose TTL is determined by `AppSettings.session_expiry`.
///
/// Blocks on the ObjC TouchID prompt — must be called async so Tauri runs it
/// on a Tokio worker thread, not the main thread.
#[tauri::command]
pub async fn unlock_vault(
    app: AppHandle,
    session: State<'_, Arc<SessionStore>>,
    key_provider: State<'_, Arc<dyn KeyProvider + Send + Sync>>,
    settings: State<'_, Arc<Mutex<AppSettings>>>,
) -> Result<SessionStatus, VaultError> {
    // 1. Show TouchID prompt on a blocking thread.
    tokio::task::spawn_blocking(|| crate::features::auth::touchid::prompt("Unlock Vaultor"))
        .await
        .map_err(|_| VaultError::AuthFailed)??;

    // 2. Retrieve (or generate on first run) the encryption key.
    let kp = Arc::clone(&*key_provider);
    let key_bytes: [u8; 32] = tokio::task::spawn_blocking(move || kp.get_or_create_key())
        .await
        .map_err(|_| VaultError::AuthFailed)??;

    // 3. Read TTL from persisted settings (None = UntilQuit).
    let ttl = settings.lock().unwrap().session_expiry.to_duration();

    // 4. Create session — get back the optional expiry and a cancellation receiver.
    let session_arc = Arc::clone(&*session);
    let (expires_at_ms, cancel_rx) = session_arc.create(key_bytes, ttl);

    // 5. If a TTL was set, spawn a background task that emits session-expired
    //    when it elapses.  UntilQuit sessions skip this entirely.
    if let Some(ttl_duration) = ttl {
        let session_for_task = Arc::clone(&*session);
        tokio::spawn(async move {
            tokio::select! {
                _ = tokio::time::sleep(ttl_duration) => {
                    session_for_task.invalidate();
                    app.emit("vault://session-expired", ()).ok();
                }
                // cancel_rx fires when invalidate() or a second unlock_vault() cancels this task
                _ = cancel_rx => {}
            }
        });
    }

    Ok(SessionStatus {
        active: true,
        expires_at_ms,
    })
}

/// Manually lock the vault (invalidates the session token and wipes the in-memory key).
#[tauri::command]
pub fn lock_vault(session: State<'_, Arc<SessionStore>>) {
    session.invalidate();
}

/// Return the current session status without side effects.
#[tauri::command]
pub fn session_status(session: State<'_, Arc<SessionStore>>) -> SessionStatus {
    session.status()
}
