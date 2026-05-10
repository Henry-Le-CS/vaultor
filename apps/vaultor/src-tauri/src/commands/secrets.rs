use std::sync::Arc;

use sqlx::SqlitePool;
use tauri::State;

use crate::error::VaultError;
use crate::features::auth::session::SessionStore;
use crate::features::storage::{
    kv_fields::{self, KvFieldDecrypted, KvFieldInput},
    secret::{self, SecretMeta},
};
use crate::features::vault::cipher;

// ── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct CreateKvSecretInput {
    pub namespace_id: String,
    pub name: String,
    pub fields: Vec<KvFieldInput>,
}

#[derive(serde::Deserialize)]
pub struct UpdateKvSecretInput {
    pub id: String,
    pub name: String,
    pub fields: Vec<KvFieldInput>,
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// List all secret metadata for a namespace (no decryption needed).
#[tauri::command]
pub async fn list_secrets(
    namespace_id: String,
    db: State<'_, SqlitePool>,
) -> Result<Vec<SecretMeta>, VaultError> {
    secret::list_by_namespace(&db, &namespace_id).await
}

/// Create a new key-value secret with encrypted fields.
#[tauri::command]
pub async fn create_kv_secret(
    input: CreateKvSecretInput,
    session: State<'_, Arc<SessionStore>>,
    db: State<'_, SqlitePool>,
) -> Result<SecretMeta, VaultError> {
    let key = session.get_key()?;
    let key: [u8; 32] = key
        .try_into()
        .map_err(|_| VaultError::Crypto("invalid key length".to_string()))?;

    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(VaultError::Validation(
            "Secret name cannot be empty".to_string(),
        ));
    }

    // Create the secret row (not a draft)
    let meta = secret::create(&db, &input.namespace_id, &name, "kv", false).await?;

    // Encrypt each field
    let encrypted: Vec<(KvFieldInput, Vec<u8>, Vec<u8>)> = input
        .fields
        .into_iter()
        .map(|f| cipher::encrypt(&key, &meta.id, &f.value).map(|(ct, nonce)| (f, ct, nonce)))
        .collect::<Result<_, _>>()?;

    kv_fields::insert_fields(&db, &meta.id, &encrypted).await?;
    Ok(meta)
}

/// Retrieve and decrypt all fields for a KV secret. Requires an active session.
#[tauri::command]
pub async fn get_kv_secret(
    id: String,
    session: State<'_, Arc<SessionStore>>,
    db: State<'_, SqlitePool>,
) -> Result<Vec<KvFieldDecrypted>, VaultError> {
    let key = session.get_key()?;
    let key: [u8; 32] = key
        .try_into()
        .map_err(|_| VaultError::Crypto("invalid key length".to_string()))?;

    let rows = kv_fields::list_for_secret(&db, &id).await?;

    rows.into_iter()
        .map(|row| {
            let value = cipher::decrypt(&key, &id, &row.value_enc, &row.value_nonce)?;
            Ok(KvFieldDecrypted {
                id: row.id,
                title: row.title,
                value,
                hidden: row.hidden,
                sort_order: row.sort_order,
            })
        })
        .collect()
}

/// Update the name and fields of an existing KV secret.
#[tauri::command]
pub async fn update_kv_secret(
    input: UpdateKvSecretInput,
    session: State<'_, Arc<SessionStore>>,
    db: State<'_, SqlitePool>,
) -> Result<(), VaultError> {
    let key = session.get_key()?;
    let key: [u8; 32] = key
        .try_into()
        .map_err(|_| VaultError::Crypto("invalid key length".to_string()))?;

    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(VaultError::Validation(
            "Secret name cannot be empty".to_string(),
        ));
    }

    secret::rename(&db, &input.id, &name).await?;

    let encrypted: Vec<(KvFieldInput, Vec<u8>, Vec<u8>)> = input
        .fields
        .into_iter()
        .map(|f| cipher::encrypt(&key, &input.id, &f.value).map(|(ct, nonce)| (f, ct, nonce)))
        .collect::<Result<_, _>>()?;

    kv_fields::replace_fields(&db, &input.id, &encrypted).await
}

/// Delete a secret and all its associated data (CASCADE handles fields).
#[tauri::command]
pub async fn delete_secret(id: String, db: State<'_, SqlitePool>) -> Result<(), VaultError> {
    secret::delete(&db, &id).await
}

/// Save a draft KV secret (creates with is_draft=true or updates existing draft).
#[tauri::command]
pub async fn save_kv_draft(
    input: CreateKvSecretInput,
    session: State<'_, Arc<SessionStore>>,
    db: State<'_, SqlitePool>,
) -> Result<SecretMeta, VaultError> {
    let key = session.get_key()?;
    let key: [u8; 32] = key
        .try_into()
        .map_err(|_| VaultError::Crypto("invalid key length".to_string()))?;

    let name = if input.name.trim().is_empty() {
        "Untitled".to_string()
    } else {
        input.name.trim().to_string()
    };

    let meta = secret::create(&db, &input.namespace_id, &name, "kv", true).await?;

    let encrypted: Vec<(KvFieldInput, Vec<u8>, Vec<u8>)> = input
        .fields
        .into_iter()
        .map(|f| cipher::encrypt(&key, &meta.id, &f.value).map(|(ct, nonce)| (f, ct, nonce)))
        .collect::<Result<_, _>>()?;

    kv_fields::insert_fields(&db, &meta.id, &encrypted).await?;
    Ok(meta)
}

/// Commit a draft secret (set is_draft=false).
#[tauri::command]
pub async fn commit_draft(id: String, db: State<'_, SqlitePool>) -> Result<(), VaultError> {
    secret::set_draft(&db, &id, false).await
}

/// Discard a draft secret (delete it entirely).
#[tauri::command]
pub async fn discard_draft(id: String, db: State<'_, SqlitePool>) -> Result<(), VaultError> {
    secret::delete(&db, &id).await
}
