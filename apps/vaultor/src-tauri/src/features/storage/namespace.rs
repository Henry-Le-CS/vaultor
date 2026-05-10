use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::VaultError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct Namespace {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<Namespace>, VaultError> {
    sqlx::query_as::<_, Namespace>(
        "SELECT id, name, created_at, updated_at FROM namespaces ORDER BY created_at",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| VaultError::Database(e.to_string()))
}

pub async fn create(pool: &SqlitePool, name: &str) -> Result<Namespace, VaultError> {
    let id = Uuid::new_v4().to_string();
    let now = now_ms();

    sqlx::query("INSERT INTO namespaces (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(name)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;

    Ok(Namespace {
        id,
        name: name.to_string(),
        created_at: now,
        updated_at: now,
    })
}

pub async fn rename(pool: &SqlitePool, id: &str, new_name: &str) -> Result<(), VaultError> {
    let now = now_ms();
    let rows = sqlx::query("UPDATE namespaces SET name = ?, updated_at = ? WHERE id = ?")
        .bind(new_name)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?
        .rows_affected();

    if rows == 0 {
        Err(VaultError::NotFound("namespace".to_string()))
    } else {
        Ok(())
    }
}

pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), VaultError> {
    let rows = sqlx::query("DELETE FROM namespaces WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?
        .rows_affected();

    if rows == 0 {
        Err(VaultError::NotFound("namespace".to_string()))
    } else {
        Ok(())
    }
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
            "CREATE TABLE IF NOT EXISTS namespaces (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create table");

        pool
    }

    #[tokio::test]
    async fn create_and_list() {
        let pool = test_pool().await;
        create(&pool, "Project A").await.unwrap();
        create(&pool, "Project B").await.unwrap();
        let ns = list(&pool).await.unwrap();
        assert_eq!(ns.len(), 2);
        assert!(ns.iter().any(|n| n.name == "Project A"));
        assert!(ns.iter().any(|n| n.name == "Project B"));
    }

    #[tokio::test]
    async fn rename_namespace() {
        let pool = test_pool().await;
        let ns = create(&pool, "Old Name").await.unwrap();
        rename(&pool, &ns.id, "New Name").await.unwrap();
        let updated = list(&pool).await.unwrap();
        assert_eq!(updated[0].name, "New Name");
    }

    #[tokio::test]
    async fn delete_namespace() {
        let pool = test_pool().await;
        let ns = create(&pool, "To Delete").await.unwrap();
        delete(&pool, &ns.id).await.unwrap();
        let remaining = list(&pool).await.unwrap();
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn rename_nonexistent_returns_err() {
        let pool = test_pool().await;
        let result = rename(&pool, "nonexistent", "Whatever").await;
        assert!(result.is_err());
    }
}
