use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::VaultError;

#[derive(Debug, sqlx::FromRow)]
pub struct KvFieldRow {
    pub id: String,
    pub secret_id: String,
    pub title: String,
    pub value_enc: Vec<u8>,
    pub value_nonce: Vec<u8>,
    pub hidden: bool,
    pub sort_order: i64,
}

/// Input from the frontend for creating/updating a field.
#[derive(Debug, serde::Deserialize)]
pub struct KvFieldInput {
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
                CAST(hidden AS BOOLEAN) as hidden, sort_order
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
    for (i, (field, ct, nonce)) in fields.iter().enumerate() {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO kv_fields (id, secret_id, title, value_enc, value_nonce, hidden, sort_order)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(secret_id)
        .bind(&field.title)
        .bind(ct)
        .bind(nonce)
        .bind(field.hidden as i64)
        .bind(i as i64)
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
