use std::sync::Arc;

use sqlx::SqlitePool;
use tauri::State;
use tokio::sync::Mutex;

use crate::error::VaultError;
use crate::features::storage::namespace::{self, Namespace};

#[tauri::command]
pub async fn list_namespaces(
    db: State<'_, Arc<Mutex<SqlitePool>>>,
) -> Result<Vec<Namespace>, VaultError> {
    let pool = db.lock().await.clone();
    namespace::list(&pool).await
}

#[tauri::command]
pub async fn create_namespace(
    name: String,
    db: State<'_, Arc<Mutex<SqlitePool>>>,
) -> Result<Namespace, VaultError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(VaultError::Validation(
            "Namespace name cannot be empty".to_string(),
        ));
    }
    let pool = db.lock().await.clone();
    namespace::create(&pool, &name).await
}

#[tauri::command]
pub async fn rename_namespace(
    id: String,
    name: String,
    db: State<'_, Arc<Mutex<SqlitePool>>>,
) -> Result<(), VaultError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(VaultError::Validation(
            "Namespace name cannot be empty".to_string(),
        ));
    }
    let pool = db.lock().await.clone();
    namespace::rename(&pool, &id, &name).await
}

#[tauri::command]
pub async fn delete_namespace(
    id: String,
    db: State<'_, Arc<Mutex<SqlitePool>>>,
) -> Result<(), VaultError> {
    let pool = db.lock().await.clone();
    namespace::delete(&pool, &id).await
}
