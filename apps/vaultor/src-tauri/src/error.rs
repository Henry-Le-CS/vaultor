use thiserror::Error;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("authentication failed")]
    AuthFailed,

    #[error("session expired or not active")]
    SessionExpired,

    #[error("encryption key not found in keychain")]
    KeyNotFound,

    #[error("keychain error: {0}")]
    Keychain(String),

    #[error("key generation failed")]
    KeyGenFailed,

    #[error("database error: {0}")]
    Database(String),

    #[error("{0} not found")]
    NotFound(String),

    #[error("validation: {0}")]
    Validation(String),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("io error: {0}")]
    Io(String),

    /// Returned by `move_storage` when destination already contains `vaultor.db`
    /// and `force` was not set.  Frontend matches on this exact string.
    #[error("destination_exists")]
    DestinationExists,

    #[error("integrity check failed on copied database")]
    IntegrityCheckFailed,

    #[error("tauri error: {0}")]
    Tauri(#[from] tauri::Error),
}

/// Make VaultError serialisable so Tauri commands can return it as a JS error.
impl serde::Serialize for VaultError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
