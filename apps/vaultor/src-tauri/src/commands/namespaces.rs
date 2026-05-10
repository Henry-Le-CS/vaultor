use sqlx::SqlitePool;
use tauri::State;

use crate::error::VaultError;
use crate::features::storage::namespace::{self, Namespace};

#[tauri::command]
pub async fn list_namespaces(db: State<'_, SqlitePool>) -> Result<Vec<Namespace>, VaultError> {
    namespace::list(&db).await
}

#[tauri::command]
pub async fn create_namespace(
    name: String,
    db: State<'_, SqlitePool>,
) -> Result<Namespace, VaultError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(VaultError::Validation(
            "Namespace name cannot be empty".to_string(),
        ));
    }
    namespace::create(&db, &name).await
}

#[tauri::command]
pub async fn rename_namespace(
    id: String,
    name: String,
    db: State<'_, SqlitePool>,
) -> Result<(), VaultError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(VaultError::Validation(
            "Namespace name cannot be empty".to_string(),
        ));
    }
    namespace::rename(&db, &id, &name).await
}

#[tauri::command]
pub async fn delete_namespace(id: String, db: State<'_, SqlitePool>) -> Result<(), VaultError> {
    namespace::delete(&db, &id).await
}
