//! Sync engine — hash-aware fetch → pull-or-merge → push cycle.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use sqlx::SqlitePool;

use crate::error::VaultError;
use crate::features::git_storage::merge::merge_vault;
use crate::features::git_storage::subprocess::GitRunner;
use crate::features::git_storage::{
    export_vault, import_vault, now_ms, read_vault_from_dir, write_vault_to_dir, NamespaceJson,
    SecretJson, VaultJson,
};
use crate::features::settings::config::GitRemoteConfig;

// ── Public result type ────────────────────────────────────────────────────────

pub struct SyncResult {
    pub namespaces_synced: usize,
    pub secrets_synced: usize,
    pub committed: bool,
    pub pushed: bool,
    /// `true` when the remote was ahead and we reset-hard to it
    /// (no local commit or push was made).
    pub pulled: bool,
}

// ── Main sync function ────────────────────────────────────────────────────────

/// Full sync cycle (hash-aware, deletion-aware):
///
/// 1.  Snapshot working-tree IDs (last-sync state on disk).
/// 2.  Fetch latest from remote.
/// 3.  Compare `HEAD` hash to `FETCH_HEAD` hash.
/// 4.  Read remote namespace and secret files via `git show`.
/// 5.  Export local vault from SQLite.
/// 6.  Compute locally-deleted IDs + field-deletions **before** overwriting disk.
/// 7.  Write local vault to disk; stage with `git add -A`.
/// 8.  Detect staged changes:
///     - No changes + hashes match  → already up to date.
///     - No changes + remote ahead  → reset-hard + import (no commit/push).
///     - Local changes present      → filter remote, merge, commit, push.
pub async fn sync(
    pool: &SqlitePool,
    runner: &GitRunner,
    config: &GitRemoteConfig,
    vault_data_dir: &Path,
) -> Result<SyncResult, VaultError> {
    let branch = &config.branch;

    // ── 0. Snapshot working-tree IDs (= last-sync state on disk) ─────────────
    let disk_ns_ids = collect_disk_ids(&vault_data_dir.join("namespaces"))?;
    let disk_secret_ids = collect_disk_ids(&vault_data_dir.join("secrets"))?;

    // ── 1. Fetch ──────────────────────────────────────────────────────────────
    runner.fetch(branch)?;

    // ── 2. Hash comparison ────────────────────────────────────────────────────
    let local_hash = runner.local_hash()?;
    let remote_hash = runner.remote_hash()?;
    // If FETCH_HEAD is absent (no prior fetch) or equal to HEAD, remote is not ahead.
    let remote_ahead = remote_hash.as_deref() != Some(local_hash.as_str());

    // ── 3. Read remote namespaces ─────────────────────────────────────────────
    let remote_ns_names = runner.list_remote_dir(branch, "vault-data/namespaces/")?;
    let mut remote_namespaces: Vec<NamespaceJson> = Vec::with_capacity(remote_ns_names.len());
    for filename in &remote_ns_names {
        if !filename.ends_with(".json") {
            continue;
        }
        let path = format!("vault-data/namespaces/{filename}");
        match runner.show_remote_file(branch, &path) {
            Ok(content) => match serde_json::from_str::<NamespaceJson>(&content) {
                Ok(ns) => remote_namespaces.push(ns),
                Err(e) => {
                    tracing::error!(
                        target: "vaultor::git",
                        path = %path,
                        error = %e,
                        "skipping corrupt remote namespace file"
                    );
                }
            },
            Err(e) => {
                tracing::error!(
                    target: "vaultor::git",
                    path = %path,
                    error = %e,
                    "failed to read remote namespace file — skipping"
                );
            }
        }
    }

    // ── 4. Read remote secrets ────────────────────────────────────────────────
    let remote_secret_names = runner.list_remote_dir(branch, "vault-data/secrets/")?;
    let mut remote_secrets: Vec<SecretJson> = Vec::with_capacity(remote_secret_names.len());
    for filename in &remote_secret_names {
        if !filename.ends_with(".json") {
            continue;
        }
        let path = format!("vault-data/secrets/{filename}");
        match runner.show_remote_file(branch, &path) {
            Ok(content) => match serde_json::from_str::<SecretJson>(&content) {
                Ok(s) => remote_secrets.push(s),
                Err(e) => {
                    tracing::error!(
                        target: "vaultor::git",
                        path = %path,
                        error = %e,
                        "skipping corrupt remote secret file"
                    );
                }
            },
            Err(e) => {
                tracing::error!(
                    target: "vaultor::git",
                    path = %path,
                    error = %e,
                    "failed to read remote secret file — skipping"
                );
            }
        }
    }

    // ── 5. Export local vault ─────────────────────────────────────────────────
    let local_vault = export_vault(pool).await?;

    // ── 6. Compute locally-deleted IDs ────────────────────────────────────────
    // Must happen BEFORE write_vault_to_dir so on-disk files still reflect the
    // previous sync snapshot.
    let local_ns_ids: HashSet<&str> = local_vault
        .namespaces
        .iter()
        .map(|n| n.id.as_str())
        .collect();
    let local_secret_ids: HashSet<&str> =
        local_vault.secrets.iter().map(|s| s.id.as_str()).collect();

    let deleted_ns_ids: HashSet<&str> = disk_ns_ids
        .iter()
        .map(String::as_str)
        .filter(|id| !local_ns_ids.contains(id))
        .collect();
    let deleted_secret_ids: HashSet<&str> = disk_secret_ids
        .iter()
        .map(String::as_str)
        .filter(|id| !local_secret_ids.contains(id))
        .collect();

    // Field-level deletion: compare current SQLite fields vs the on-disk
    // snapshot of each KV secret (the state as of the previous sync).
    let mut deleted_fields_by_secret: HashMap<String, HashSet<String>> = HashMap::new();
    for local_secret in &local_vault.secrets {
        if local_secret.kind != "kv" {
            continue;
        }
        let disk_path = vault_data_dir
            .join("secrets")
            .join(format!("{}.json", local_secret.id));
        if let Ok(content) = std::fs::read_to_string(&disk_path) {
            if let Ok(disk_secret) = serde_json::from_str::<SecretJson>(&content) {
                let local_field_ids: HashSet<&str> = local_secret
                    .kv_fields
                    .iter()
                    .map(|f| f.id.as_str())
                    .collect();
                for disk_field in &disk_secret.kv_fields {
                    if !local_field_ids.contains(disk_field.id.as_str()) {
                        deleted_fields_by_secret
                            .entry(local_secret.id.clone())
                            .or_default()
                            .insert(disk_field.id.clone());
                    }
                }
            }
        }
    }

    // ── 7. Write local vault to disk, remove deleted files, and stage ─────────
    write_vault_to_dir(&local_vault, vault_data_dir)?;

    // Remove locally-deleted entity files BEFORE staging so that `git add -A`
    // picks them up as deletions.  Without this, the JSON files linger on disk,
    // `has_staged_changes` returns false, and the deletion is silently skipped.
    for id in &deleted_ns_ids {
        let path = vault_data_dir.join("namespaces").join(format!("{id}.json"));
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| {
                VaultError::Io(format!("failed to remove deleted namespace file {id}: {e}"))
            })?;
        }
    }
    for id in &deleted_secret_ids {
        let path = vault_data_dir.join("secrets").join(format!("{id}.json"));
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| {
                VaultError::Io(format!("failed to remove deleted secret file {id}: {e}"))
            })?;
        }
    }

    runner.add_vault_data_all()?;

    // ── 8. Detect staged changes ──────────────────────────────────────────────
    let has_local_changes = runner.has_staged_changes()?;

    // ── Path A: already up to date ────────────────────────────────────────────
    if !has_local_changes && !remote_ahead {
        tracing::debug!(target: "vaultor::git", "sync: already up to date");
        return Ok(SyncResult {
            namespaces_synced: local_vault.namespaces.len(),
            secrets_synced: local_vault.secrets.len(),
            committed: false,
            pushed: false,
            pulled: false,
        });
    }

    // ── Path B: no local changes but remote is ahead → reset-hard ────────────
    if !has_local_changes {
        runner.reset_hard_fetch_head()?;
        let pulled_vault = read_vault_from_dir(vault_data_dir)?;
        import_vault(pool, &pulled_vault).await?;
        prune_sqlite(pool, &pulled_vault).await?;

        tracing::debug!(
            target: "vaultor::git",
            namespaces = pulled_vault.namespaces.len(),
            secrets = pulled_vault.secrets.len(),
            "sync: pulled remote changes (reset-hard)"
        );

        return Ok(SyncResult {
            namespaces_synced: pulled_vault.namespaces.len(),
            secrets_synced: pulled_vault.secrets.len(),
            committed: false,
            pushed: false,
            pulled: true,
        });
    }

    // ── Path C: local has changes → merge + commit + push ────────────────────

    // Build remote VaultJson, filtering out locally-deleted items.
    let remote_vault = VaultJson {
        format_version: 1,
        created_at: now_ms(),
        namespaces: remote_namespaces
            .into_iter()
            .filter(|ns| !deleted_ns_ids.contains(ns.id.as_str()))
            .collect(),
        secrets: remote_secrets
            .into_iter()
            .filter(|s| !deleted_secret_ids.contains(s.id.as_str()))
            .map(|mut s| {
                if s.kind == "kv" {
                    if let Some(del_ids) = deleted_fields_by_secret.get(&s.id) {
                        s.kv_fields.retain(|f| !del_ids.contains(&f.id));
                    }
                }
                s
            })
            .collect(),
    };

    // Merge filtered remote + local.
    let merged = merge_vault(remote_vault, local_vault);

    // Remove stale disk files so `git add -A` picks up deletions.
    let merged_ns_ids: HashSet<&str> = merged.namespaces.iter().map(|n| n.id.as_str()).collect();
    let merged_secret_ids: HashSet<&str> = merged.secrets.iter().map(|s| s.id.as_str()).collect();
    remove_stale_files(&vault_data_dir.join("namespaces"), &merged_ns_ids)?;
    remove_stale_files(&vault_data_dir.join("secrets"), &merged_secret_ids)?;

    // Write merged vault to disk.
    write_vault_to_dir(&merged, vault_data_dir)?;

    // Import merged vault into SQLite.
    import_vault(pool, &merged).await?;

    // Prune SQLite rows absent from the merged vault.
    prune_sqlite(pool, &merged).await?;

    let namespaces_synced = merged.namespaces.len();
    let secrets_synced = merged.secrets.len();

    // Stage all changes (including deletions).
    runner.add_vault_data_all()?;

    // Commit — returns false if nothing changed vs HEAD (merge == HEAD).
    let ts = now_ms();
    let committed = runner.commit(&format!("Vaultor sync {ts}"))?;

    // Push with one retry on stale-lease rejection.
    let mut pushed = false;
    if committed {
        let ok = runner.push_force_with_lease(branch)?;
        if ok {
            pushed = true;
        } else {
            runner.fetch(branch)?;
            let retry_ok = runner.push_force_with_lease(branch)?;
            if retry_ok {
                pushed = true;
            } else {
                return Err(VaultError::Io("git push rejected after retry".to_string()));
            }
        }
    }

    tracing::debug!(
        target: "vaultor::git",
        namespaces_synced,
        secrets_synced,
        committed,
        pushed,
        "sync complete"
    );

    Ok(SyncResult {
        namespaces_synced,
        secrets_synced,
        committed,
        pushed,
        pulled: false,
    })
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Collect the UUID stems of all `*.json` files in `dir`.
/// Returns an empty set if the directory does not exist.
fn collect_disk_ids(dir: &Path) -> Result<HashSet<String>, VaultError> {
    let mut ids = HashSet::new();
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        ids.insert(stem.to_string());
                    }
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(VaultError::Io(format!("failed to read dir {dir:?}: {e}"))),
    }
    Ok(ids)
}

/// Delete any `{id}.json` inside `dir` whose stem is absent from `keep_ids`.
fn remove_stale_files(dir: &Path, keep_ids: &HashSet<&str>) -> Result<(), VaultError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(VaultError::Io(format!("failed to read dir {dir:?}: {e}"))),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        if !keep_ids.contains(stem) {
            tracing::debug!(
                target: "vaultor::git",
                path = %path.display(),
                "removing stale vault-data file"
            );
            std::fs::remove_file(&path).map_err(|e| {
                VaultError::Io(format!("failed to remove stale file {path:?}: {e}"))
            })?;
        }
    }

    Ok(())
}

/// Delete SQLite rows that are absent from the given `VaultJson` snapshot.
///
/// Covers namespaces, secrets, and kv_fields for surviving KV secrets.
async fn prune_sqlite(pool: &SqlitePool, vault: &VaultJson) -> Result<(), VaultError> {
    let vault_ns_ids: HashSet<&str> = vault.namespaces.iter().map(|n| n.id.as_str()).collect();
    let vault_secret_ids: HashSet<&str> = vault.secrets.iter().map(|s| s.id.as_str()).collect();

    // Prune namespaces.
    let db_ns: Vec<(String,)> = sqlx::query_as("SELECT id FROM namespaces")
        .fetch_all(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;
    for (id,) in db_ns {
        if !vault_ns_ids.contains(id.as_str()) {
            sqlx::query("DELETE FROM namespaces WHERE id = ?")
                .bind(&id)
                .execute(pool)
                .await
                .map_err(|e| VaultError::Database(e.to_string()))?;
        }
    }

    // Prune secrets.
    let db_secrets: Vec<(String,)> = sqlx::query_as("SELECT id FROM secrets")
        .fetch_all(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;
    for (id,) in db_secrets {
        if !vault_secret_ids.contains(id.as_str()) {
            sqlx::query("DELETE FROM secrets WHERE id = ?")
                .bind(&id)
                .execute(pool)
                .await
                .map_err(|e| VaultError::Database(e.to_string()))?;
        }
    }

    // Prune kv_fields for each surviving KV secret.
    for secret in &vault.secrets {
        if secret.kind != "kv" {
            continue;
        }
        let field_ids: HashSet<&str> = secret.kv_fields.iter().map(|f| f.id.as_str()).collect();
        let db_fields: Vec<(String,)> =
            sqlx::query_as("SELECT id FROM kv_fields WHERE secret_id = ?")
                .bind(&secret.id)
                .fetch_all(pool)
                .await
                .map_err(|e| VaultError::Database(e.to_string()))?;
        for (field_id,) in db_fields {
            if !field_ids.contains(field_id.as_str()) {
                sqlx::query("DELETE FROM kv_fields WHERE id = ?")
                    .bind(&field_id)
                    .execute(pool)
                    .await
                    .map_err(|e| VaultError::Database(e.to_string()))?;
            }
        }
    }

    Ok(())
}
