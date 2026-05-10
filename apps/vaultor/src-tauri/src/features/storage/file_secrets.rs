use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::VaultError;

const MAX_FILE_BYTES: usize = 1_048_576; // 1 MiB

#[derive(Debug, sqlx::FromRow)]
pub struct FileSecretRow {
    pub id: String,
    pub secret_id: String,
    pub filename: String,
    pub content_enc: Vec<u8>,
    pub content_nonce: Vec<u8>,
    pub size_bytes: i64,
}

pub fn validate_size(bytes: usize) -> Result<(), VaultError> {
    if bytes > MAX_FILE_BYTES {
        return Err(VaultError::Validation(format!(
            "File exceeds 1 MiB limit ({bytes} bytes)"
        )));
    }
    Ok(())
}

pub async fn insert(
    pool: &SqlitePool,
    secret_id: &str,
    filename: &str,
    content_enc: &[u8],
    content_nonce: &[u8],
    size_bytes: usize,
) -> Result<String, VaultError> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO file_secrets (id, secret_id, filename, content_enc, content_nonce, size_bytes)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(secret_id)
    .bind(filename)
    .bind(content_enc)
    .bind(content_nonce)
    .bind(size_bytes as i64)
    .execute(pool)
    .await
    .map_err(|e| VaultError::Database(e.to_string()))?;
    Ok(id)
}

pub async fn get_for_secret(
    pool: &SqlitePool,
    secret_id: &str,
) -> Result<FileSecretRow, VaultError> {
    sqlx::query_as::<_, FileSecretRow>(
        "SELECT id, secret_id, filename, content_enc, content_nonce, size_bytes
         FROM file_secrets WHERE secret_id = ? LIMIT 1",
    )
    .bind(secret_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| VaultError::Database(e.to_string()))?
    .ok_or_else(|| VaultError::NotFound("file secret".to_string()))
}

pub async fn replace(
    pool: &SqlitePool,
    secret_id: &str,
    filename: &str,
    content_enc: &[u8],
    content_nonce: &[u8],
    size_bytes: usize,
) -> Result<(), VaultError> {
    sqlx::query("DELETE FROM file_secrets WHERE secret_id = ?")
        .bind(secret_id)
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;

    insert(
        pool,
        secret_id,
        filename,
        content_enc,
        content_nonce,
        size_bytes,
    )
    .await?;
    Ok(())
}
