//! Tauri command handlers for git remote storage.
//!
//! Each git remote has its own isolated environment:
//!   - Working clone:  `{app_data_dir}/git-repos/{id}/`
//!   - Environment DB: `{app_data_dir}/git-repos/{id}/vault.db`
//!
//! Local mode uses the user-configured `vaultor.db` (tracked by `db_path_state`).
//! Git mode uses the remote-specific `vault.db`.  Switching environments replaces
//! the active pool — no data bleeds between local and git.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, State};
use tokio::sync::Mutex as TokioMutex;

use crate::error::VaultError;
use crate::features::git_storage::subprocess::{
    default_branch, ls_remote_heads, validate_branch, validate_url, GitRunner,
};
use crate::features::git_storage::sync::sync;
use crate::features::git_storage::{now_ms, read_vault_from_dir, replace_vault, VaultJson};
use crate::features::settings::config::{AppSettings, GitRemoteConfig};
use crate::features::storage::db;

// ── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct SyncResultDto {
    pub namespaces_synced: usize,
    pub secrets_synced: usize,
    pub committed: bool,
    pub pushed: bool,
    pub pulled: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct GitConnectionResult {
    pub branches: Vec<String>,
    pub default_branch: String,
}

#[derive(Debug, serde::Serialize)]
pub struct GitStatusDto {
    pub connected: bool,
    pub url: Option<String>,
    pub branch: Option<String>,
    pub last_synced: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct RemoveGitRemoteResult {
    pub new_active_url: Option<String>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn repo_dir_for(data_dir: &std::path::Path, config: &GitRemoteConfig) -> PathBuf {
    data_dir.join("git-repos").join(&config.id)
}

/// Open a new SQLite database at `new_path` (running migrations), swap it into
/// the shared pool state, and close the old pool.
///
/// The lock is held only briefly for the swap — the async open and close happen
/// outside the critical section.
async fn switch_active_pool(
    db: &Arc<TokioMutex<SqlitePool>>,
    new_path: &std::path::Path,
) -> Result<(), VaultError> {
    let new_pool = db::open(new_path).await?;
    let old_pool = {
        let mut lock = db.lock().await;
        std::mem::replace(&mut *lock, new_pool)
    };
    old_pool.close().await;
    Ok(())
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Test connectivity to a remote git repository and return available branches.
///
/// Runs `git ls-remote --heads <url>` using system credentials (SSH agent,
/// osxkeychain, etc.). No token is stored by the app.
#[tauri::command]
pub async fn test_git_connection(url: String) -> Result<GitConnectionResult, VaultError> {
    validate_url(&url)?;

    let branches = ls_remote_heads(&url)?;

    if branches.is_empty() {
        return Err(VaultError::Validation(
            "Repository has no branches. Please push an initial commit first.".to_string(),
        ));
    }

    let default = default_branch(&branches).unwrap_or_else(|| branches[0].clone());

    Ok(GitConnectionResult {
        branches,
        default_branch: default,
    })
}

/// Connect to a remote git repository and enter git mode.
///
/// Flow:
/// 1. Clone the remote into `git-repos/{id}/`.
/// 2. Open an isolated DB at `git-repos/{id}/vault.db` (runs migrations).
/// 3. Load whatever vault data exists in the repo into the git DB.
///    An empty/fresh repo produces an empty vault — local data is never touched.
/// 4. Switch the active pool to the git DB.
/// 5. Save the connection config.
#[tauri::command]
pub async fn connect_git_remote(
    app: AppHandle,
    db: State<'_, Arc<TokioMutex<SqlitePool>>>,
    settings: State<'_, Arc<Mutex<AppSettings>>>,
    url: String,
    branch: String,
) -> Result<(), VaultError> {
    validate_url(&url)?;
    validate_branch(&branch)?;

    let data_dir = app.path().app_data_dir()?;
    let config_dir = app.path().app_config_dir()?;

    let id = uuid::Uuid::new_v4().to_string();
    let repo_dir = data_dir.join("git-repos").join(&id);
    let git_db_path = repo_dir.join("vault.db");

    // Remove stale working tree so connect is idempotent.
    if repo_dir.exists() {
        std::fs::remove_dir_all(&repo_dir)
            .map_err(|e| VaultError::Io(format!("failed to remove old git-repo: {e}")))?;
    }
    std::fs::create_dir_all(&repo_dir)
        .map_err(|e| VaultError::Io(format!("failed to create git-repo dir: {e}")))?;

    // Shallow-clone to minimise bandwidth on first connect.
    let runner = GitRunner::new(&repo_dir);
    runner.clone_repo(&url, &branch)?;

    // Switch to the git-specific DB (creates it fresh, runs migrations).
    switch_active_pool(db.inner(), &git_db_path).await?;

    // Load vault from the cloned repo files.  An empty repo → empty DB,
    // which is the correct state for a fresh git environment.
    let vault_data_dir = repo_dir.join("vault-data");
    let vault = read_vault_from_dir(&vault_data_dir).unwrap_or_else(|_| VaultJson {
        format_version: 1,
        created_at: now_ms(),
        namespaces: vec![],
        secrets: vec![],
    });
    {
        let pool = db.lock().await.clone();
        replace_vault(&pool, &vault).await?;
    }

    // Persist connection config.
    {
        let mut s = settings.lock().unwrap();
        s.add_git_remote(GitRemoteConfig {
            id,
            url,
            branch,
            last_synced: Some(now_ms()),
        });
        s.save(&config_dir).map_err(VaultError::Validation)?;
    }

    Ok(())
}

/// Return the current git remote connection status.
///
/// Reads directly from in-memory settings — no git subprocess is invoked.
#[tauri::command]
pub async fn get_git_status(
    settings: State<'_, Arc<Mutex<AppSettings>>>,
) -> Result<GitStatusDto, VaultError> {
    let s = settings.lock().unwrap();
    match s.active_git_remote() {
        Some(remote) => Ok(GitStatusDto {
            connected: true,
            url: Some(remote.url.clone()),
            branch: Some(remote.branch.clone()),
            last_synced: remote.last_synced,
        }),
        None => Ok(GitStatusDto {
            connected: false,
            url: None,
            branch: None,
            last_synced: None,
        }),
    }
}

/// Switch the active git remote to a different configured repository.
///
/// Flow:
/// 1. Fetch the latest remote state (clone if working tree is missing).
/// 2. Switch the active pool to that remote's isolated `vault.db`.
/// 3. Replace the DB contents with the repo's vault files (authoritative source).
/// 4. Update settings.
#[tauri::command]
pub async fn switch_git_remote(
    app: AppHandle,
    db: State<'_, Arc<TokioMutex<SqlitePool>>>,
    settings: State<'_, Arc<Mutex<AppSettings>>>,
    url: String,
) -> Result<(), VaultError> {
    let data_dir = app.path().app_data_dir()?;
    let config_dir = app.path().app_config_dir()?;

    let new_config = {
        let s = settings.lock().unwrap();
        s.git_remotes
            .iter()
            .find(|r| r.url == url)
            .cloned()
            .ok_or_else(|| {
                VaultError::Validation(format!("no repository with url '{url}' is configured"))
            })?
    };

    let repo_dir = repo_dir_for(&data_dir, &new_config);
    let git_db_path = repo_dir.join("vault.db");

    // Clone if missing; otherwise fetch + hard-reset to remote tip.
    let runner = GitRunner::new(&repo_dir);
    if !repo_dir.exists() {
        std::fs::create_dir_all(&repo_dir)
            .map_err(|e| VaultError::Io(format!("failed to create git-repo dir: {e}")))?;
        runner.clone_repo(&new_config.url, &new_config.branch)?;
    } else {
        runner.fetch(&new_config.branch)?;
        runner.reset_hard_fetch_head()?;
    }

    // Switch pool and reload from repo files.
    switch_active_pool(db.inner(), &git_db_path).await?;

    let vault_data_dir = repo_dir.join("vault-data");
    let vault = read_vault_from_dir(&vault_data_dir).unwrap_or_else(|_| VaultJson {
        format_version: 1,
        created_at: now_ms(),
        namespaces: vec![],
        secrets: vec![],
    });
    {
        let pool = db.lock().await.clone();
        replace_vault(&pool, &vault).await?;
    }

    // Update active setting.
    {
        let mut s = settings.lock().unwrap();
        s.set_active_git_remote(&url);
        if let Some(remote) = s.active_git_remote_mut() {
            remote.last_synced = Some(now_ms());
        }
        s.save(&config_dir).map_err(VaultError::Validation)?;
    }

    Ok(())
}

/// Switch back to local SQLite mode.
///
/// The git remote config is kept in the list so the user can switch back
/// without re-entering credentials.  The working clone is left intact.
#[tauri::command]
pub async fn disconnect_git_remote(
    app: AppHandle,
    db: State<'_, Arc<TokioMutex<SqlitePool>>>,
    settings: State<'_, Arc<Mutex<AppSettings>>>,
    db_path_state: State<'_, Arc<Mutex<PathBuf>>>,
) -> Result<(), VaultError> {
    let config_dir = app.path().app_config_dir()?;

    // Switch pool back to the local database.
    let local_db_path = db_path_state.lock().unwrap().clone();
    switch_active_pool(db.inner(), &local_db_path).await?;

    // Clear the active remote (but keep it in the configured list).
    {
        let mut s = settings.lock().unwrap();
        s.active_git_url = None;
        s.save(&config_dir).map_err(VaultError::Validation)?;
    }

    Ok(())
}

/// Run a full sync cycle against the active git remote:
/// pull remote → merge with local → push merged result.
///
/// Updates `last_synced` in settings after a successful push or pull.
#[tauri::command]
pub async fn sync_git(
    app: AppHandle,
    db: State<'_, Arc<TokioMutex<SqlitePool>>>,
    settings: State<'_, Arc<Mutex<AppSettings>>>,
) -> Result<SyncResultDto, VaultError> {
    let data_dir = app.path().app_data_dir()?;
    let config_dir = app.path().app_config_dir()?;

    let config = {
        let s = settings.lock().unwrap();
        s.active_git_remote().cloned()
    }
    .ok_or_else(|| VaultError::Validation("no active git remote configured".to_string()))?;

    let repo_dir = repo_dir_for(&data_dir, &config);
    let vault_data_dir = repo_dir.join("vault-data");
    let runner = GitRunner::new(&repo_dir);

    let pool = db.lock().await.clone();
    let result = sync(&pool, &runner, &config, &vault_data_dir).await?;

    // Stamp last_synced on any meaningful sync (committed, pushed, or pulled).
    if result.committed || result.pushed || result.pulled {
        let mut s = settings.lock().unwrap();
        if let Some(remote) = s.active_git_remote_mut() {
            remote.last_synced = Some(now_ms());
        }
        s.save(&config_dir).map_err(VaultError::Validation)?;
    }

    Ok(SyncResultDto {
        namespaces_synced: result.namespaces_synced,
        secrets_synced: result.secrets_synced,
        committed: result.committed,
        pushed: result.pushed,
        pulled: result.pulled,
    })
}

/// Remove a configured git repository and clean up its local clone.
///
/// If it was the active repository:
/// - Switches pool to the next configured remote (if any), OR
/// - Falls back to local mode.
#[tauri::command]
pub async fn remove_git_remote(
    app: AppHandle,
    db: State<'_, Arc<TokioMutex<SqlitePool>>>,
    settings: State<'_, Arc<Mutex<AppSettings>>>,
    db_path_state: State<'_, Arc<Mutex<PathBuf>>>,
    url: String,
) -> Result<RemoveGitRemoteResult, VaultError> {
    let data_dir = app.path().app_data_dir()?;
    let config_dir = app.path().app_config_dir()?;

    let (removed_config, was_active) = {
        let s = settings.lock().unwrap();
        let config = s.git_remotes.iter().find(|r| r.url == url).cloned();
        let was_active = s.active_git_url.as_deref() == Some(url.as_str());
        (config, was_active)
    };

    let removed_config = removed_config.ok_or_else(|| {
        VaultError::Validation(format!("no repository with url '{url}' is configured"))
    })?;

    // Remove from settings list.
    {
        let mut s = settings.lock().unwrap();
        s.remove_git_remote(&url);
        s.save(&config_dir).map_err(VaultError::Validation)?;
    }

    // If removed repo was active, switch environment.
    if was_active {
        let new_config = {
            let s = settings.lock().unwrap();
            s.active_git_remote().cloned()
        };

        if let Some(new_config) = new_config {
            // Switch to the next configured git remote.
            let repo_dir = repo_dir_for(&data_dir, &new_config);
            let git_db_path = repo_dir.join("vault.db");
            let runner = GitRunner::new(&repo_dir);

            if !repo_dir.exists() {
                std::fs::create_dir_all(&repo_dir)
                    .map_err(|e| VaultError::Io(format!("failed to create git-repo dir: {e}")))?;
                runner.clone_repo(&new_config.url, &new_config.branch)?;
            } else {
                runner.fetch(&new_config.branch)?;
                runner.reset_hard_fetch_head()?;
            }

            switch_active_pool(db.inner(), &git_db_path).await?;

            let vault_data_dir = repo_dir.join("vault-data");
            let vault = read_vault_from_dir(&vault_data_dir).unwrap_or_else(|_| VaultJson {
                format_version: 1,
                created_at: now_ms(),
                namespaces: vec![],
                secrets: vec![],
            });
            let pool = db.lock().await.clone();
            replace_vault(&pool, &vault).await?;
        } else {
            // No remotes left — fall back to local mode.
            let local_db_path = db_path_state.lock().unwrap().clone();
            switch_active_pool(db.inner(), &local_db_path).await?;
        }
    }

    // Delete the removed repo's local clone directory (best-effort).
    let removed_repo_dir = repo_dir_for(&data_dir, &removed_config);
    if removed_repo_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&removed_repo_dir) {
            tracing::error!(
                target: "vaultor::git",
                error = %e,
                "failed to remove git-repo dir after remove_git_remote"
            );
        }
    }

    let new_active_url = {
        let s = settings.lock().unwrap();
        s.active_git_url.clone()
    };

    Ok(RemoveGitRemoteResult { new_active_url })
}
