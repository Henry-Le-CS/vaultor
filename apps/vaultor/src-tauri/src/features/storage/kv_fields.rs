use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::VaultError;

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[derive(Debug, sqlx::FromRow)]
pub struct KvFieldRow {
    pub id: String,
    pub secret_id: String,
    pub title: String,
    pub value_enc: Vec<u8>,
    pub value_nonce: Vec<u8>,
    pub hidden: bool,
    pub sort_order: i64,
    pub updated_at: i64,
}

/// Input from the frontend for creating/updating a field.
#[derive(Debug, serde::Deserialize)]
pub struct KvFieldInput {
    #[serde(default)]
    pub id: Option<String>,
    pub title: String,
    pub value: String,
    pub hidden: bool,
}

/// Decrypted field sent back to the frontend.
#[derive(Debug, serde::Serialize)]
pub struct KvFieldDecrypted {
    pub id: String,
    pub title: String,
    pub value: String,
    pub hidden: bool,
    pub sort_order: i64,
}

pub async fn list_for_secret(
    pool: &SqlitePool,
    secret_id: &str,
) -> Result<Vec<KvFieldRow>, VaultError> {
    sqlx::query_as::<_, KvFieldRow>(
        "SELECT id, secret_id, title, value_enc, value_nonce,
                CAST(hidden AS BOOLEAN) as hidden, sort_order, updated_at
         FROM kv_fields WHERE secret_id = ? ORDER BY sort_order",
    )
    .bind(secret_id)
    .fetch_all(pool)
    .await
    .map_err(|e| VaultError::Database(e.to_string()))
}

/// Insert all fields for a newly created secret. Existing fields are NOT deleted first.
pub async fn insert_fields(
    pool: &SqlitePool,
    secret_id: &str,
    fields: &[(KvFieldInput, Vec<u8>, Vec<u8>)], // (input, ciphertext, nonce)
) -> Result<(), VaultError> {
    let now = now_ms();
    for (i, (field, ct, nonce)) in fields.iter().enumerate() {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO kv_fields
             (id, secret_id, title, value_enc, value_nonce, hidden, sort_order, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(secret_id)
        .bind(&field.title)
        .bind(ct)
        .bind(nonce)
        .bind(field.hidden as i64)
        .bind(i as i64)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;
    }
    Ok(())
}

/// Replace all fields for a secret (delete existing, then insert new).
pub async fn replace_fields(
    pool: &SqlitePool,
    secret_id: &str,
    fields: &[(KvFieldInput, Vec<u8>, Vec<u8>)],
) -> Result<(), VaultError> {
    sqlx::query("DELETE FROM kv_fields WHERE secret_id = ?")
        .bind(secret_id)
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;

    insert_fields(pool, secret_id, fields).await
}

/// Upsert fields for a secret, preserving stable IDs.
///
/// - Fields with an `id` are upserted (ON CONFLICT DO UPDATE).
/// - Fields without an `id` get a freshly generated UUID.
/// - Any existing field whose ID is absent from the incoming list is deleted.
pub async fn upsert_fields(
    pool: &SqlitePool,
    secret_id: &str,
    fields: &[(KvFieldInput, Vec<u8>, Vec<u8>)],
) -> Result<(), VaultError> {
    let now = now_ms();

    // IDs that will survive in the new list.
    let incoming_ids: Vec<String> = fields.iter().filter_map(|(f, _, _)| f.id.clone()).collect();

    // Delete fields that are no longer in the list.
    let existing: Vec<(String,)> = sqlx::query_as("SELECT id FROM kv_fields WHERE secret_id = ?")
        .bind(secret_id)
        .fetch_all(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;

    for (id,) in existing {
        if !incoming_ids.contains(&id) {
            sqlx::query("DELETE FROM kv_fields WHERE id = ?")
                .bind(&id)
                .execute(pool)
                .await
                .map_err(|e| VaultError::Database(e.to_string()))?;
        }
    }

    // Upsert each field.
    for (i, (field, ct, nonce)) in fields.iter().enumerate() {
        let id = field
            .id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        sqlx::query(
            "INSERT INTO kv_fields
             (id, secret_id, title, value_enc, value_nonce, hidden, sort_order, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               title       = excluded.title,
               value_enc   = excluded.value_enc,
               value_nonce = excluded.value_nonce,
               hidden      = excluded.hidden,
               sort_order  = excluded.sort_order,
               updated_at  = excluded.updated_at",
        )
        .bind(&id)
        .bind(secret_id)
        .bind(&field.title)
        .bind(ct.as_slice())
        .bind(nonce.as_slice())
        .bind(field.hidden as i64)
        .bind(i as i64)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;
    }

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
            "CREATE TABLE kv_fields (
                id TEXT PRIMARY KEY, secret_id TEXT NOT NULL,
                title TEXT NOT NULL, value_enc BLOB NOT NULL,
                value_nonce BLOB NOT NULL, hidden INTEGER DEFAULT 1,
                sort_order INTEGER DEFAULT 0, updated_at INTEGER DEFAULT 0,
                FOREIGN KEY (secret_id) REFERENCES secrets(id) ON DELETE CASCADE
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Seed a namespace + secret for FK references.
        sqlx::query("INSERT INTO namespaces VALUES ('ns1', 'Test', 0, 0)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO secrets VALUES ('s1', 'ns1', 'Sec', 'kv', 0, 0, 0)")
            .execute(&pool)
            .await
            .unwrap();

        pool
    }

    fn field(title: &str, value: &str, hidden: bool) -> (KvFieldInput, Vec<u8>, Vec<u8>) {
        (
            KvFieldInput {
                id: None,
                title: title.to_string(),
                value: value.to_string(),
                hidden,
            },
            value.as_bytes().to_vec(), // fake ciphertext
            vec![0u8; 12],             // fake nonce
        )
    }

    fn field_with_id(id: &str, title: &str) -> (KvFieldInput, Vec<u8>, Vec<u8>) {
        (
            KvFieldInput {
                id: Some(id.to_string()),
                title: title.to_string(),
                value: "v".to_string(),
                hidden: false,
            },
            b"ct".to_vec(),
            vec![0u8; 12],
        )
    }

    #[tokio::test]
    async fn insert_and_list_fields() {
        let pool = test_pool().await;
        let fields = vec![
            field("Username", "alice", false),
            field("Password", "hunter2", true),
        ];
        insert_fields(&pool, "s1", &fields).await.unwrap();

        let rows = list_for_secret(&pool, "s1").await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].title, "Username");
        assert_eq!(rows[0].sort_order, 0);
        assert!(!rows[0].hidden);
        assert_eq!(rows[1].title, "Password");
        assert_eq!(rows[1].sort_order, 1);
        assert!(rows[1].hidden);
    }

    #[tokio::test]
    async fn list_empty_returns_empty() {
        let pool = test_pool().await;
        let rows = list_for_secret(&pool, "s1").await.unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn replace_fields_removes_old() {
        let pool = test_pool().await;
        insert_fields(&pool, "s1", &[field("Old", "old", false)])
            .await
            .unwrap();
        assert_eq!(list_for_secret(&pool, "s1").await.unwrap().len(), 1);

        replace_fields(
            &pool,
            "s1",
            &[field("New1", "n1", false), field("New2", "n2", true)],
        )
        .await
        .unwrap();

        let rows = list_for_secret(&pool, "s1").await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].title, "New1");
        assert_eq!(rows[1].title, "New2");
    }

    #[tokio::test]
    async fn upsert_adds_new_fields() {
        let pool = test_pool().await;
        upsert_fields(&pool, "s1", &[field("Brand New", "v", false)])
            .await
            .unwrap();

        let rows = list_for_secret(&pool, "s1").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "Brand New");
    }

    #[tokio::test]
    async fn upsert_updates_existing_field() {
        let pool = test_pool().await;
        // Insert a field with a known ID.
        insert_fields(&pool, "s1", &[field("Title1", "v1", false)])
            .await
            .unwrap();
        let existing_id = list_for_secret(&pool, "s1").await.unwrap()[0].id.clone();

        // Upsert with the same ID but different title.
        upsert_fields(&pool, "s1", &[field_with_id(&existing_id, "Updated Title")])
            .await
            .unwrap();

        let rows = list_for_secret(&pool, "s1").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "Updated Title");
        assert_eq!(rows[0].id, existing_id);
    }

    #[tokio::test]
    async fn upsert_deletes_removed_fields() {
        let pool = test_pool().await;
        insert_fields(
            &pool,
            "s1",
            &[field("Keep", "v", false), field("Remove", "v", false)],
        )
        .await
        .unwrap();
        let rows = list_for_secret(&pool, "s1").await.unwrap();
        assert_eq!(rows.len(), 2);
        let keep_id = rows[0].id.clone();

        // Upsert with only the first field — second should be deleted.
        upsert_fields(&pool, "s1", &[field_with_id(&keep_id, "Keep")])
            .await
            .unwrap();

        let rows = list_for_secret(&pool, "s1").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "Keep");
    }
}
