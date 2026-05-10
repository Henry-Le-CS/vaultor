use crate::error::VaultError;

const SERVICE: &str = "com.vaultor.enckey";
const ACCOUNT: &str = "default";

/// Abstraction over key retrieval so tests can inject a mock without
/// a real macOS Keychain (unavailable in headless CI).
pub trait KeyProvider: Send + Sync + 'static {
    /// Return the 256-bit encryption key, creating and persisting it on first call.
    fn get_or_create_key(&self) -> Result<[u8; 32], VaultError>;
}

// ── Real Keychain provider ──────────────────────────────────────────────────

pub struct KeychainKeyProvider;

impl KeyProvider for KeychainKeyProvider {
    fn get_or_create_key(&self) -> Result<[u8; 32], VaultError> {
        match get_key_from_keychain() {
            Ok(key) => Ok(key),
            Err(VaultError::KeyNotFound) => {
                let key = generate_key()?;
                store_key_in_keychain(&key)?;
                Ok(key)
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(target_os = "macos")]
fn get_key_from_keychain() -> Result<[u8; 32], VaultError> {
    use security_framework::passwords::get_generic_password;

    let bytes = get_generic_password(SERVICE, ACCOUNT).map_err(|e| {
        // errSecItemNotFound = -25300
        if e.code() == -25300 {
            VaultError::KeyNotFound
        } else {
            VaultError::Keychain(e.to_string())
        }
    })?;

    bytes
        .try_into()
        .map_err(|_| VaultError::Keychain("stored key has wrong length".to_string()))
}

#[cfg(not(target_os = "macos"))]
fn get_key_from_keychain() -> Result<[u8; 32], VaultError> {
    Err(VaultError::KeyNotFound)
}

#[cfg(target_os = "macos")]
fn store_key_in_keychain(key: &[u8; 32]) -> Result<(), VaultError> {
    use security_framework::passwords::set_generic_password;
    set_generic_password(SERVICE, ACCOUNT, key).map_err(|e| VaultError::Keychain(e.to_string()))
}

#[cfg(not(target_os = "macos"))]
fn store_key_in_keychain(_key: &[u8; 32]) -> Result<(), VaultError> {
    Ok(())
}

fn generate_key() -> Result<[u8; 32], VaultError> {
    let mut key = [0u8; 32];
    getrandom::getrandom(&mut key).map_err(|_| VaultError::KeyGenFailed)?;
    Ok(key)
}

// ── Mock provider for tests ─────────────────────────────────────────────────

pub struct MockKeyProvider {
    pub key: [u8; 32],
}

impl Default for MockKeyProvider {
    fn default() -> Self {
        Self { key: [0x42u8; 32] }
    }
}

impl KeyProvider for MockKeyProvider {
    fn get_or_create_key(&self) -> Result<[u8; 32], VaultError> {
        Ok(self.key)
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_provider_returns_key() {
        let provider = MockKeyProvider::default();
        let key = provider.get_or_create_key().unwrap();
        assert_eq!(key, [0x42u8; 32]);
    }

    #[test]
    fn mock_provider_custom_key() {
        let expected = [0xABu8; 32];
        let provider = MockKeyProvider { key: expected };
        assert_eq!(provider.get_or_create_key().unwrap(), expected);
    }
}
