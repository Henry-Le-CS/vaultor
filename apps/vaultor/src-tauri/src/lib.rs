use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub mod commands;
pub mod error;
pub mod features;

use features::auth::session::SessionStore;
use features::keychain::{KeyProvider, KeychainKeyProvider};
use features::settings::config::AppSettings;
use features::storage::db;
use tauri::Manager;

pub fn run() {
    let session = Arc::new(SessionStore::new());
    let key_provider: Arc<dyn KeyProvider + Send + Sync> = Arc::new(KeychainKeyProvider);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(session)
        .manage(key_provider)
        .setup(|app| {
            // ── Config dir (settings.json lives here) ────────────────
            let config_dir = app.path().app_config_dir()?;
            std::fs::create_dir_all(&config_dir)?;

            // ── Data dir (default DB location) ────────────────────────
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;

            // ── Load persisted settings ───────────────────────────────
            let settings = AppSettings::load_or_default(&config_dir);
            let db_path: PathBuf = settings.resolved_db_path(&data_dir);

            let settings_state = Arc::new(Mutex::new(settings));
            let db_path_state = Arc::new(Mutex::new(db_path.clone()));

            app.manage(settings_state);
            app.manage(db_path_state);

            // ── Open the database ──────────────────────────────────────
            let pool = tauri::async_runtime::block_on(db::open(&db_path))?;
            app.manage(pool);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Auth
            commands::auth::unlock_vault,
            commands::auth::lock_vault,
            commands::auth::session_status,
            // Settings
            commands::settings::get_settings,
            commands::settings::set_session_expiry,
            commands::settings::get_storage_location,
            commands::settings::pick_folder,
            commands::settings::move_storage,
            // Namespaces
            commands::namespaces::list_namespaces,
            commands::namespaces::create_namespace,
            commands::namespaces::rename_namespace,
            commands::namespaces::delete_namespace,
            // Secrets
            commands::secrets::list_secrets,
            commands::secrets::create_kv_secret,
            commands::secrets::get_kv_secret,
            commands::secrets::update_kv_secret,
            commands::secrets::delete_secret,
            commands::secrets::save_kv_draft,
            commands::secrets::commit_draft,
            commands::secrets::discard_draft,
            // File secrets
            commands::files::create_file_secret,
            commands::files::get_file_secret,
            commands::files::update_file_secret,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
