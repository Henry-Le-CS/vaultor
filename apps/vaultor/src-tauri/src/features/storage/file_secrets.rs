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

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("in-memory pool");

        sqlx::query(
            "CREATE TABLE namespaces (
                id TEXT PRIMARY KEY, name TEXT NOT NULL,
                created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE secrets (
                id TEXT PRIMARY KEY, namespace_id TEXT NOT NULL,
                name TEXT NOT NULL, kind TEXT NOT NULL,
                is_draft INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE file_secrets (
                id TEXT PRIMARY KEY, secret_id TEXT NOT NULL,
                filename TEXT NOT NULL, content_enc BLOB NOT NULL,
                content_nonce BLOB NOT NULL, size_bytes INTEGER NOT NULL,
                FOREIGN KEY (secret_id) REFERENCES secrets(id) ON DELETE CASCADE
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO namespaces VALUES ('ns1', 'Test', 0, 0)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO secrets VALUES ('s1', 'ns1', 'File Sec', 'file', 0, 0, 0)")
            .execute(&pool)
            .await
            .unwrap();

        pool
    }

    #[test]
    fn validate_size_within_limit() {
        assert!(validate_size(1_048_576).is_ok());
        assert!(validate_size(0).is_ok());
        assert!(validate_size(500_000).is_ok());
    }

    #[test]
    fn validate_size_exceeds_limit() {
        assert!(validate_size(1_048_577).is_err());
        assert!(validate_size(2_000_000).is_err());
    }

    #[tokio::test]
    async fn insert_and_get() {
        let pool = test_pool().await;
        let enc = b"encrypted-content";
        let nonce = b"123456789012";
        let id = insert(&pool, "s1", "readme.txt", enc, nonce, 42)
            .await
            .unwrap();
        assert!(!id.is_empty());

        let row = get_for_secret(&pool, "s1").await.unwrap();
        assert_eq!(row.filename, "readme.txt");
        assert_eq!(row.content_enc, enc.to_vec());
        assert_eq!(row.content_nonce, nonce.to_vec());
        assert_eq!(row.size_bytes, 42);
    }

    #[tokio::test]
    async fn get_nonexistent_returns_not_found() {
        let pool = test_pool().await;
        let result = get_for_secret(&pool, "s1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn replace_overwrites() {
        let pool = test_pool().await;
        insert(&pool, "s1", "old.txt", b"old", b"123456789012", 3)
            .await
            .unwrap();

        replace(&pool, "s1", "new.txt", b"new-content", b"nonce_nonce_", 11)
            .await
            .unwrap();

        let row = get_for_secret(&pool, "s1").await.unwrap();
        assert_eq!(row.filename, "new.txt");
        assert_eq!(row.content_enc, b"new-content".to_vec());
        assert_eq!(row.size_bytes, 11);
    }

    #[tokio::test]
    async fn insert_multiple_then_get_returns_first() {
        let pool = test_pool().await;
        // Insert two file secrets for the same secret_id (edge case).
        insert(&pool, "s1", "first.txt", b"a", b"123456789012", 1)
            .await
            .unwrap();
        insert(&pool, "s1", "second.txt", b"b", b"123456789012", 1)
            .await
            .unwrap();

        // get_for_secret uses LIMIT 1, so it returns one row.
        let row = get_for_secret(&pool, "s1").await.unwrap();
        assert!(row.filename == "first.txt" || row.filename == "second.txt");
    }
}
