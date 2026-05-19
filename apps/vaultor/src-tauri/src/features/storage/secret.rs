use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::VaultError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct SecretMeta {
    pub id: String,
    pub namespace_id: String,
    pub name: String,
    pub kind: String,
    pub is_draft: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub async fn list_by_namespace(
    pool: &SqlitePool,
    namespace_id: &str,
) -> Result<Vec<SecretMeta>, VaultError> {
    sqlx::query_as::<_, SecretMeta>(
        "SELECT id, namespace_id, name, kind,
                CAST(is_draft AS BOOLEAN) as is_draft, created_at, updated_at
         FROM secrets WHERE namespace_id = ? ORDER BY created_at",
    )
    .bind(namespace_id)
    .fetch_all(pool)
    .await
    .map_err(|e| VaultError::Database(e.to_string()))
}

pub async fn create(
    pool: &SqlitePool,
    namespace_id: &str,
    name: &str,
    kind: &str,
    is_draft: bool,
) -> Result<SecretMeta, VaultError> {
    let id = Uuid::new_v4().to_string();
    let now = now_ms();
    let draft_int = is_draft as i64;

    sqlx::query(
        "INSERT INTO secrets (id, namespace_id, name, kind, is_draft, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(namespace_id)
    .bind(name)
    .bind(kind)
    .bind(draft_int)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| VaultError::Database(e.to_string()))?;

    Ok(SecretMeta {
        id,
        namespace_id: namespace_id.to_string(),
        name: name.to_string(),
        kind: kind.to_string(),
        is_draft,
        created_at: now,
        updated_at: now,
    })
}

pub async fn rename(pool: &SqlitePool, id: &str, name: &str) -> Result<(), VaultError> {
    let now = now_ms();
    sqlx::query("UPDATE secrets SET name = ?, updated_at = ? WHERE id = ?")
        .bind(name)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))
        .map(|_| ())
}

pub async fn set_draft(pool: &SqlitePool, id: &str, is_draft: bool) -> Result<(), VaultError> {
    let now = now_ms();
    sqlx::query("UPDATE secrets SET is_draft = ?, updated_at = ? WHERE id = ?")
        .bind(is_draft as i64)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))
        .map(|_| ())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), VaultError> {
    sqlx::query("DELETE FROM secrets WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))
        .map(|_| ())
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
                created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
                FOREIGN KEY (namespace_id) REFERENCES namespaces(id) ON DELETE CASCADE
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert a namespace for FK references.
        sqlx::query("INSERT INTO namespaces (id, name, created_at, updated_at) VALUES ('ns1', 'Test', 0, 0)")
            .execute(&pool)
            .await
            .unwrap();

        pool
    }

    #[tokio::test]
    async fn create_and_list_by_namespace() {
        let pool = test_pool().await;
        create(&pool, "ns1", "Secret A", "kv", false).await.unwrap();
        create(&pool, "ns1", "Secret B", "file", false)
            .await
            .unwrap();

        let secrets = list_by_namespace(&pool, "ns1").await.unwrap();
        assert_eq!(secrets.len(), 2);
        assert!(secrets
            .iter()
            .any(|s| s.name == "Secret A" && s.kind == "kv"));
        assert!(secrets
            .iter()
            .any(|s| s.name == "Secret B" && s.kind == "file"));
    }

    #[tokio::test]
    async fn create_sets_correct_fields() {
        let pool = test_pool().await;
        let s = create(&pool, "ns1", "My Secret", "kv", true).await.unwrap();
        assert_eq!(s.namespace_id, "ns1");
        assert_eq!(s.name, "My Secret");
        assert_eq!(s.kind, "kv");
        assert!(s.is_draft);
        assert!(s.created_at > 0);
        assert_eq!(s.created_at, s.updated_at);
    }

    #[tokio::test]
    async fn rename_secret() {
        let pool = test_pool().await;
        let s = create(&pool, "ns1", "Old", "kv", false).await.unwrap();
        rename(&pool, &s.id, "New").await.unwrap();

        let secrets = list_by_namespace(&pool, "ns1").await.unwrap();
        assert_eq!(secrets[0].name, "New");
    }

    #[tokio::test]
    async fn delete_secret() {
        let pool = test_pool().await;
        let s = create(&pool, "ns1", "Delete Me", "kv", false)
            .await
            .unwrap();
        delete(&pool, &s.id).await.unwrap();

        let secrets = list_by_namespace(&pool, "ns1").await.unwrap();
        assert!(secrets.is_empty());
    }

    #[tokio::test]
    async fn set_draft_flag() {
        let pool = test_pool().await;
        let s = create(&pool, "ns1", "Draft Test", "kv", false)
            .await
            .unwrap();
        assert!(!s.is_draft);

        set_draft(&pool, &s.id, true).await.unwrap();
        let secrets = list_by_namespace(&pool, "ns1").await.unwrap();
        assert!(secrets[0].is_draft);

        set_draft(&pool, &s.id, false).await.unwrap();
        let secrets = list_by_namespace(&pool, "ns1").await.unwrap();
        assert!(!secrets[0].is_draft);
    }

    #[tokio::test]
    async fn list_empty_namespace() {
        let pool = test_pool().await;
        let secrets = list_by_namespace(&pool, "ns1").await.unwrap();
        assert!(secrets.is_empty());
    }

    #[tokio::test]
    async fn list_filters_by_namespace() {
        let pool = test_pool().await;
        // Add a second namespace.
        sqlx::query("INSERT INTO namespaces (id, name, created_at, updated_at) VALUES ('ns2', 'Other', 0, 0)")
            .execute(&pool)
            .await
            .unwrap();

        create(&pool, "ns1", "In NS1", "kv", false).await.unwrap();
        create(&pool, "ns2", "In NS2", "kv", false).await.unwrap();

        let ns1_secrets = list_by_namespace(&pool, "ns1").await.unwrap();
        assert_eq!(ns1_secrets.len(), 1);
        assert_eq!(ns1_secrets[0].name, "In NS1");

        let ns2_secrets = list_by_namespace(&pool, "ns2").await.unwrap();
        assert_eq!(ns2_secrets.len(), 1);
        assert_eq!(ns2_secrets[0].name, "In NS2");
    }
}
