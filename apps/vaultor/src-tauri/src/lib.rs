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

fn init_tracing(log_dir: &std::path::Path) {
    use tracing_subscriber::fmt;
    use tracing_subscriber::prelude::*;

    let _ = std::fs::create_dir_all(log_dir);
    let file_appender = tracing_appender::rolling::never(log_dir, "vaultor.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Leak the guard so it lives for the duration of the process.
    std::mem::forget(guard);

    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_level(true);

    tracing_subscriber::registry().with(file_layer).init();
}

pub fn run() {
    // ── Tracing — write to ~/Library/Logs/Vaultor/vaultor.log ────────────────
    {
        let log_dir = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir())
            .join("Library/Logs/Vaultor");
        init_tracing(&log_dir);
    }

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

            // db_path_state always tracks the LOCAL db path.
            let local_db_path: PathBuf = settings.resolved_db_path(&data_dir);

            // If a git remote was active on last exit and its environment-specific
            // DB exists, resume in git mode.  Otherwise start in local mode.
            let initial_db_path = settings
                .active_git_remote()
                .map(|remote| data_dir.join("git-repos").join(&remote.id).join("vault.db"))
                .filter(|p| p.exists())
                .unwrap_or_else(|| local_db_path.clone());

            let settings_state = Arc::new(Mutex::new(settings));
            let db_path_state = Arc::new(Mutex::new(local_db_path));

            app.manage(settings_state);
            app.manage(db_path_state);

            // ── Open the database and wrap in a swappable mutex ────────
            let pool = tauri::async_runtime::block_on(db::open(&initial_db_path))?;
            app.manage(Arc::new(tokio::sync::Mutex::new(pool)));

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
            commands::settings::set_tutorial_seen,
            commands::settings::get_storage_location,
            commands::settings::pick_folder,
            commands::settings::move_storage,
            // Namespaces
            commands::namespaces::list_namespaces,
            commands::namespaces::create_namespace,
            commands::namespaces::rename_namespace,
            commands::namespaces::delete_namespace,
            // Password generator
            commands::generate::generate_password,
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
            // Git remote storage
            commands::git_storage::test_git_connection,
            commands::git_storage::connect_git_remote,
            commands::git_storage::sync_git,
            commands::git_storage::get_git_status,
            commands::git_storage::switch_git_remote,
            commands::git_storage::disconnect_git_remote,
            commands::git_storage::remove_git_remote,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
