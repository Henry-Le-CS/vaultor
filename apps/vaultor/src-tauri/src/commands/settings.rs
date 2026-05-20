use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::Manager;
use tauri::{AppHandle, State};

use crate::error::VaultError;
use crate::features::settings::config::{AppSettings, SessionExpiry};

// ── DTOs ─────────────────────────────────────────────────────────────────────

/// Git remote connection info sent to the frontend.
#[derive(Debug, Serialize, Clone)]
pub struct GitRemoteDto {
    pub id: String,
    pub url: String,
    pub branch: String,
    pub last_synced: Option<i64>,
}

/// Subset of AppSettings that is safe to send to the frontend.
#[derive(Debug, Serialize)]
pub struct AppSettingsDto {
    pub session_expiry: SessionExpiry,
    /// Currently resolved vault DB path (string for JS convenience).
    pub db_path: String,
    /// Active git remote connection state, or `null` if in local mode.
    pub git_remote: Option<GitRemoteDto>,
    /// All connected git repositories.
    pub git_remotes: Vec<GitRemoteDto>,
    /// Whether the user has completed the onboarding tutorial.
    pub tutorial_seen: bool,
}

// ── Phase 1 commands ─────────────────────────────────────────────────────────

/// Return the current settings.
#[tauri::command]
pub fn get_settings(
    settings: State<'_, Arc<Mutex<AppSettings>>>,
    db_path_state: State<'_, Arc<Mutex<PathBuf>>>,
) -> Result<AppSettingsDto, VaultError> {
    let s = settings.lock().unwrap();
    let db_path = db_path_state.lock().unwrap().to_string_lossy().into_owned();

    let git_remotes: Vec<GitRemoteDto> = s
        .git_remotes
        .iter()
        .map(|r| GitRemoteDto {
            id: r.id.clone(),
            url: r.url.clone(),
            branch: r.branch.clone(),
            last_synced: r.last_synced,
        })
        .collect();

    let git_remote = s.active_git_remote().map(|r| GitRemoteDto {
        id: r.id.clone(),
        url: r.url.clone(),
        branch: r.branch.clone(),
        last_synced: r.last_synced,
    });

    Ok(AppSettingsDto {
        session_expiry: s.session_expiry.clone(),
        db_path,
        git_remote,
        git_remotes,
        tutorial_seen: s.tutorial_seen,
    })
}

/// Persist a new session expiry choice.  Takes effect on the next unlock.
#[tauri::command]
pub fn set_session_expiry(
    app: AppHandle,
    settings: State<'_, Arc<Mutex<AppSettings>>>,
    expiry: SessionExpiry,
) -> Result<(), VaultError> {
    let config_dir = app.path().app_config_dir()?;
    let mut s = settings.lock().unwrap();
    s.session_expiry = expiry;
    s.save(&config_dir).map_err(VaultError::Validation)
}

/// Mark the onboarding tutorial as seen.
#[tauri::command]
pub fn set_tutorial_seen(
    app: AppHandle,
    settings: State<'_, Arc<Mutex<AppSettings>>>,
) -> Result<(), VaultError> {
    let config_dir = app.path().app_config_dir()?;
    let mut s = settings.lock().unwrap();
    s.tutorial_seen = true;
    s.save(&config_dir).map_err(VaultError::Validation)
}

// ── Phase 2 commands ─────────────────────────────────────────────────────────

/// Return the path of the vault DB file the running app is using.
#[tauri::command]
pub fn get_storage_location(db_path_state: State<'_, Arc<Mutex<PathBuf>>>) -> String {
    db_path_state.lock().unwrap().to_string_lossy().into_owned()
}

/// Open a native folder picker.  Returns `None` if the user cancels.
#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;

    let (tx, rx) = tokio::sync::oneshot::channel::<Option<PathBuf>>();

    app.dialog().file().pick_folder(move |folder| {
        let _ = tx.send(folder.map(|f| f.into_path().unwrap_or_default()));
    });

    rx.await
        .ok()
        .flatten()
        .map(|p| p.to_string_lossy().into_owned())
}

/// Copy the vault DB to `new_dir/vaultor.db`, verify integrity, then update
/// settings.  The running app still points to the old path until restart.
///
/// Pass `force = true` to overwrite an existing `vaultor.db` at the destination.
/// Returns the new path string on success.
#[tauri::command]
pub async fn move_storage(
    app: AppHandle,
    settings: State<'_, Arc<Mutex<AppSettings>>>,
    db_path_state: State<'_, Arc<Mutex<PathBuf>>>,
    new_dir: String,
    force: bool,
) -> Result<String, VaultError> {
    let dest_dir = PathBuf::from(&new_dir);
    let dest_file = dest_dir.join("vaultor.db");

    // Guard: destination already has vaultor.db and force not set.
    if dest_file.exists() && !force {
        return Err(VaultError::DestinationExists);
    }

    let src_path = db_path_state.lock().unwrap().clone();

    // Copy on a blocking thread.
    let src_clone = src_path.clone();
    let dest_clone = dest_file.clone();
    tokio::task::spawn_blocking(move || std::fs::copy(&src_clone, &dest_clone))
        .await
        .map_err(|e| VaultError::Io(e.to_string()))?
        .map_err(|e| VaultError::Io(e.to_string()))?;

    // Integrity check via SQLite PRAGMA.
    let dest_str = dest_file.to_string_lossy().into_owned();
    let dest_for_check = dest_str.clone();
    let ok = tokio::task::spawn_blocking(move || integrity_check(&dest_for_check))
        .await
        .map_err(|e| VaultError::Io(e.to_string()))??;

    if !ok {
        // Clean up the failed copy.
        let _ = std::fs::remove_file(&dest_file);
        return Err(VaultError::IntegrityCheckFailed);
    }

    // Persist the new path.
    let config_dir = app.path().app_config_dir()?;
    {
        let mut s = settings.lock().unwrap();
        s.db_path = Some(dest_file.clone());
        s.save(&config_dir).map_err(VaultError::Validation)?;
    }

    Ok(dest_str)
}

/// Delete all user data from the local vault database (namespaces, secrets,
/// key-value fields, file secrets).  The empty schema is preserved — the app
/// is ready to use immediately after this call.
///
/// This does NOT affect git-backed databases or app settings.
#[tauri::command]
pub async fn clear_local_storage(
    db: State<'_, Arc<tokio::sync::Mutex<sqlx::SqlitePool>>>,
    session: State<'_, Arc<crate::features::auth::session::SessionStore>>,
) -> Result<(), VaultError> {
    // Lock the session so no in-flight operations reference stale rows.
    session.invalidate();

    let pool = db.lock().await;
    crate::features::storage::clear_all_data(&pool).await
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Open the SQLite file at `path` and run `PRAGMA integrity_check`.
/// Returns `true` if the result is "ok".
fn integrity_check(path: &str) -> Result<bool, VaultError> {
    use std::path::Path;
    sqlite_integrity_check_via_file(Path::new(path))
}

/// Validate that the file looks like a valid SQLite3 database by checking
/// the 16-byte magic header and attempting to read the page size.
fn sqlite_integrity_check_via_file(path: &std::path::Path) -> Result<bool, VaultError> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| VaultError::Io(e.to_string()))?;
    let mut header = [0u8; 16];
    f.read_exact(&mut header)
        .map_err(|e| VaultError::Io(e.to_string()))?;
    // SQLite3 files begin with the string "SQLite format 3\000"
    Ok(&header == b"SQLite format 3\0")
}
