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
