pub mod db;
pub mod file_secrets;
pub mod kv_fields;
pub mod namespace;
pub mod secret;

use crate::error::VaultError;
use sqlx::SqlitePool;

/// Delete all user data (namespaces, secrets, kv_fields, file_secrets) and
/// VACUUM the database.  The schema is preserved.
pub async fn clear_all_data(pool: &SqlitePool) -> Result<(), VaultError> {
    sqlx::query("DELETE FROM file_secrets")
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;
    sqlx::query("DELETE FROM kv_fields")
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;
    sqlx::query("DELETE FROM secrets")
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;
    sqlx::query("DELETE FROM namespaces")
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;
    sqlx::query("VACUUM")
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn full_test_pool() -> SqlitePool {
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
                created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
                FOREIGN KEY (namespace_id) REFERENCES namespaces(id) ON DELETE CASCADE
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE kv_fields (
                id TEXT PRIMARY KEY, secret_id TEXT NOT NULL,
                title TEXT NOT NULL, value_enc BLOB NOT NULL,
                value_nonce BLOB NOT NULL, hidden INTEGER NOT NULL DEFAULT 1,
                sort_order INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (secret_id) REFERENCES secrets(id) ON DELETE CASCADE
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

        pool
    }

    async fn count(pool: &SqlitePool, table: &str) -> i64 {
        let q = format!("SELECT COUNT(*) as cnt FROM {table}");
        let row: (i64,) = sqlx::query_as(&q).fetch_one(pool).await.unwrap();
        row.0
    }

    #[tokio::test]
    async fn clear_all_data_removes_everything() {
        let pool = full_test_pool().await;

        // Seed data.
        sqlx::query("INSERT INTO namespaces VALUES ('ns1','Work',0,0)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO secrets VALUES ('s1','ns1','cred','kv',0,0,0)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO kv_fields VALUES ('f1','s1','user',X'AA',X'BB',1,0,0)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO file_secrets VALUES ('fs1','s1','key.pem',X'CC',X'DD',100)")
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(count(&pool, "namespaces").await, 1);
        assert_eq!(count(&pool, "secrets").await, 1);
        assert_eq!(count(&pool, "kv_fields").await, 1);
        assert_eq!(count(&pool, "file_secrets").await, 1);

        clear_all_data(&pool).await.unwrap();

        assert_eq!(count(&pool, "namespaces").await, 0);
        assert_eq!(count(&pool, "secrets").await, 0);
        assert_eq!(count(&pool, "kv_fields").await, 0);
        assert_eq!(count(&pool, "file_secrets").await, 0);
    }

    #[tokio::test]
    async fn clear_all_data_on_empty_db() {
        let pool = full_test_pool().await;
        // Should succeed even with no data.
        clear_all_data(&pool).await.unwrap();
        assert_eq!(count(&pool, "namespaces").await, 0);
    }

    #[tokio::test]
    async fn clear_preserves_schema() {
        let pool = full_test_pool().await;

        sqlx::query("INSERT INTO namespaces VALUES ('ns1','Test',0,0)")
            .execute(&pool)
            .await
            .unwrap();

        clear_all_data(&pool).await.unwrap();

        // Schema still works — we can insert again.
        sqlx::query("INSERT INTO namespaces VALUES ('ns2','New',0,0)")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(count(&pool, "namespaces").await, 1);
    }
}
