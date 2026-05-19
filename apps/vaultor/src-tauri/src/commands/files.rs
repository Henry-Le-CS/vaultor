use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use sqlx::SqlitePool;
use tauri::State;
use tokio::sync::Mutex;

use crate::error::VaultError;
use crate::features::auth::session::SessionStore;
use crate::features::storage::{file_secrets, secret};
use crate::features::vault::cipher;

#[derive(serde::Deserialize)]
pub struct CreateFileInput {
    pub namespace_id: String,
    pub name: String,
    pub filename: String,
    /// Base64-encoded file content (max 1 MiB decoded).
    pub content_b64: String,
}

#[derive(serde::Serialize)]
pub struct FileSecretInfo {
    pub filename: String,
    pub size_bytes: i64,
    /// Base64-encoded decrypted content.
    pub content_b64: String,
}

/// Create a new file secret (encrypted). Returns the secret metadata.
#[tauri::command]
pub async fn create_file_secret(
    input: CreateFileInput,
    session: State<'_, Arc<SessionStore>>,
    db: State<'_, Arc<Mutex<SqlitePool>>>,
) -> Result<crate::features::storage::secret::SecretMeta, VaultError> {
    let key = session.get_key()?;
    let key: [u8; 32] = key
        .try_into()
        .map_err(|_| VaultError::Crypto("invalid key length".to_string()))?;

    let content = B64
        .decode(&input.content_b64)
        .map_err(|_| VaultError::Validation("invalid base64 content".to_string()))?;

    file_secrets::validate_size(content.len())?;

    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(VaultError::Validation(
            "Secret name cannot be empty".to_string(),
        ));
    }

    let pool = db.lock().await.clone();
    let meta = secret::create(&pool, &input.namespace_id, &name, "file", false).await?;
    let (ct, nonce) = cipher::encrypt_bytes(&key, &meta.id, &content)?;
    file_secrets::insert(&pool, &meta.id, &input.filename, &ct, &nonce, content.len()).await?;

    Ok(meta)
}

/// Retrieve and decrypt a file secret. Returns base64-encoded content + filename.
#[tauri::command]
pub async fn get_file_secret(
    id: String,
    session: State<'_, Arc<SessionStore>>,
    db: State<'_, Arc<Mutex<SqlitePool>>>,
) -> Result<FileSecretInfo, VaultError> {
    let key = session.get_key()?;
    let key: [u8; 32] = key
        .try_into()
        .map_err(|_| VaultError::Crypto("invalid key length".to_string()))?;

    let pool = db.lock().await.clone();
    let row = file_secrets::get_for_secret(&pool, &id).await?;
    let plaintext = cipher::decrypt_bytes(&key, &id, &row.content_enc, &row.content_nonce)?;

    Ok(FileSecretInfo {
        filename: row.filename,
        size_bytes: row.size_bytes,
        content_b64: B64.encode(&plaintext),
    })
}

/// Replace the file content of an existing file secret.
#[tauri::command]
pub async fn update_file_secret(
    id: String,
    filename: String,
    content_b64: String,
    session: State<'_, Arc<SessionStore>>,
    db: State<'_, Arc<Mutex<SqlitePool>>>,
) -> Result<(), VaultError> {
    let key = session.get_key()?;
    let key: [u8; 32] = key
        .try_into()
        .map_err(|_| VaultError::Crypto("invalid key length".to_string()))?;

    let content = B64
        .decode(&content_b64)
        .map_err(|_| VaultError::Validation("invalid base64 content".to_string()))?;

    file_secrets::validate_size(content.len())?;

    let pool = db.lock().await.clone();
    let (ct, nonce) = cipher::encrypt_bytes(&key, &id, &content)?;
    file_secrets::replace(&pool, &id, &filename, &ct, &nonce, content.len()).await
}
