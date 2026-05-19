//! Git remote storage — JSON wire format, vault export, and vault import.
//!
//! The git repo stores one JSON file per entity:
//!   vault-data/namespaces/{id}.json
//!   vault-data/secrets/{id}.json
//!
//! All encrypted blobs are base64-encoded for JSON compatibility.
//! The encryption is unchanged — same AES-256-GCM as the SQLite path.

pub mod merge;
pub mod subprocess;
pub mod sync;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::VaultError;
use crate::features::storage::{file_secrets, kv_fields, namespace, secret};

// ── JSON wire format structs ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultJson {
    pub format_version: u32,
    pub created_at: i64,
    pub namespaces: Vec<NamespaceJson>,
    pub secrets: Vec<SecretJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceJson {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretJson {
    pub id: String,
    pub namespace_id: String,
    pub name: String,
    pub kind: String,
    pub is_draft: bool,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub kv_fields: Vec<KvFieldJson>,
    #[serde(default)]
    pub file_secret: Option<FileSecretJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvFieldJson {
    pub id: String,
    pub title: String,
    /// AES-256-GCM ciphertext — base64-encoded.
    pub value_enc: String,
    /// 12-byte GCM nonce — base64-encoded.
    pub value_nonce: String,
    pub hidden: bool,
    pub sort_order: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSecretJson {
    pub id: String,
    pub filename: String,
    /// AES-256-GCM ciphertext — base64-encoded.
    pub content_enc: String,
    /// 12-byte GCM nonce — base64-encoded.
    pub content_nonce: String,
    pub size_bytes: i64,
}

// ── Helpers ──────────────────────────────────────────────────────────────────

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

// ── Export ───────────────────────────────────────────────────────────────────

/// Read the entire vault from SQLite and produce a `VaultJson` snapshot.
///
/// Encrypted blobs are base64-encoded; no decryption occurs.
pub async fn export_vault(pool: &SqlitePool) -> Result<VaultJson, VaultError> {
    let namespaces = namespace::list(pool).await?;

    let ns_json: Vec<NamespaceJson> = namespaces
        .iter()
        .map(|ns| NamespaceJson {
            id: ns.id.clone(),
            name: ns.name.clone(),
            created_at: ns.created_at,
            updated_at: ns.updated_at,
        })
        .collect();

    let mut secrets_json: Vec<SecretJson> = Vec::new();

    for ns in &namespaces {
        let secrets = secret::list_by_namespace(pool, &ns.id).await?;
        for s in secrets {
            let secret_json = match s.kind.as_str() {
                "kv" => {
                    let fields = kv_fields::list_for_secret(pool, &s.id).await?;
                    let kv: Vec<KvFieldJson> = fields
                        .into_iter()
                        .map(|f| KvFieldJson {
                            id: f.id,
                            title: f.title,
                            value_enc: B64.encode(&f.value_enc),
                            value_nonce: B64.encode(&f.value_nonce),
                            hidden: f.hidden,
                            sort_order: f.sort_order,
                            updated_at: f.updated_at,
                        })
                        .collect();

                    SecretJson {
                        id: s.id,
                        namespace_id: s.namespace_id,
                        name: s.name,
                        kind: s.kind,
                        is_draft: s.is_draft,
                        created_at: s.created_at,
                        updated_at: s.updated_at,
                        kv_fields: kv,
                        file_secret: None,
                    }
                }
                "file" => {
                    let file = file_secrets::get_for_secret(pool, &s.id).await?;
                    SecretJson {
                        id: s.id,
                        namespace_id: s.namespace_id,
                        name: s.name,
                        kind: s.kind,
                        is_draft: s.is_draft,
                        created_at: s.created_at,
                        updated_at: s.updated_at,
                        kv_fields: vec![],
                        file_secret: Some(FileSecretJson {
                            id: file.id,
                            filename: file.filename,
                            content_enc: B64.encode(&file.content_enc),
                            content_nonce: B64.encode(&file.content_nonce),
                            size_bytes: file.size_bytes,
                        }),
                    }
                }
                _ => continue,
            };
            secrets_json.push(secret_json);
        }
    }

    Ok(VaultJson {
        format_version: 1,
        created_at: now_ms(),
        namespaces: ns_json,
        secrets: secrets_json,
    })
}

// ── Import ───────────────────────────────────────────────────────────────────

/// Upsert a `VaultJson` snapshot into SQLite.
///
/// - Namespaces and secrets are inserted or updated (by id).
/// - kv_fields are replaced wholesale per secret (delete + insert).
/// - File secrets are replaced wholesale per secret.
/// - Nothing is deleted from SQLite during import; deletions are handled
///   by the caller after merge (the merged VaultJson already omits deleted items).
pub async fn import_vault(pool: &SqlitePool, vault: &VaultJson) -> Result<(), VaultError> {
    for ns in &vault.namespaces {
        sqlx::query(
            "INSERT INTO namespaces (id, name, created_at, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name,
               updated_at = excluded.updated_at
             WHERE excluded.updated_at > namespaces.updated_at",
        )
        .bind(&ns.id)
        .bind(&ns.name)
        .bind(ns.created_at)
        .bind(ns.updated_at)
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;
    }

    for s in &vault.secrets {
        sqlx::query(
            "INSERT INTO secrets (id, namespace_id, name, kind, is_draft, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name,
               is_draft = excluded.is_draft,
               updated_at = excluded.updated_at
             WHERE excluded.updated_at > secrets.updated_at",
        )
        .bind(&s.id)
        .bind(&s.namespace_id)
        .bind(&s.name)
        .bind(&s.kind)
        .bind(s.is_draft as i64)
        .bind(s.created_at)
        .bind(s.updated_at)
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;

        match s.kind.as_str() {
            "kv" => import_kv_fields(pool, &s.id, &s.kv_fields).await?,
            "file" => {
                if let Some(f) = &s.file_secret {
                    import_file_secret(pool, &s.id, f).await?;
                }
            }
            _ => {}
        }
    }

    Ok(())
}

async fn import_kv_fields(
    pool: &SqlitePool,
    secret_id: &str,
    fields: &[KvFieldJson],
) -> Result<(), VaultError> {
    for f in fields {
        let value_enc = B64
            .decode(&f.value_enc)
            .map_err(|e| VaultError::Validation(format!("bad base64 value_enc: {e}")))?;
        let value_nonce = B64
            .decode(&f.value_nonce)
            .map_err(|e| VaultError::Validation(format!("bad base64 value_nonce: {e}")))?;

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
               updated_at  = excluded.updated_at
             WHERE excluded.updated_at > kv_fields.updated_at",
        )
        .bind(&f.id)
        .bind(secret_id)
        .bind(&f.title)
        .bind(&value_enc)
        .bind(&value_nonce)
        .bind(f.hidden as i64)
        .bind(f.sort_order)
        .bind(f.updated_at)
        .execute(pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;
    }
    Ok(())
}

async fn import_file_secret(
    pool: &SqlitePool,
    secret_id: &str,
    f: &FileSecretJson,
) -> Result<(), VaultError> {
    let content_enc = B64
        .decode(&f.content_enc)
        .map_err(|e| VaultError::Validation(format!("bad base64 content_enc: {e}")))?;
    let content_nonce = B64
        .decode(&f.content_nonce)
        .map_err(|e| VaultError::Validation(format!("bad base64 content_nonce: {e}")))?;

    sqlx::query(
        "INSERT INTO file_secrets
         (id, secret_id, filename, content_enc, content_nonce, size_bytes)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           filename      = excluded.filename,
           content_enc   = excluded.content_enc,
           content_nonce = excluded.content_nonce,
           size_bytes    = excluded.size_bytes",
    )
    .bind(&f.id)
    .bind(secret_id)
    .bind(&f.filename)
    .bind(&content_enc)
    .bind(&content_nonce)
    .bind(f.size_bytes)
    .execute(pool)
    .await
    .map_err(|e| VaultError::Database(e.to_string()))?;

    Ok(())
}

// ── Write to disk ────────────────────────────────────────────────────────────

/// Write a `VaultJson` snapshot to the git working directory.
///
/// Creates the directory tree:
/// ```text
/// vault_data_dir/
///   meta.json
///   namespaces/{id}.json    — one per namespace
///   secrets/{id}.json       — one per secret
/// ```
///
/// Existing files are overwritten. Stale files (from deleted entities) are NOT
/// removed here; the sync layer handles `git rm` before calling this.
pub fn write_vault_to_dir(vault: &VaultJson, vault_data_dir: &Path) -> Result<(), VaultError> {
    let ns_dir = vault_data_dir.join("namespaces");
    let secrets_dir = vault_data_dir.join("secrets");

    std::fs::create_dir_all(&ns_dir)
        .map_err(|e| VaultError::Io(format!("failed to create namespaces dir: {e}")))?;
    std::fs::create_dir_all(&secrets_dir)
        .map_err(|e| VaultError::Io(format!("failed to create secrets dir: {e}")))?;

    // Write meta.json — deliberately excludes `created_at` so it does not
    // change on every write and cause spurious git commits.
    #[derive(Serialize)]
    struct MetaJson {
        format_version: u32,
    }
    let meta_json = serde_json::to_string_pretty(&MetaJson {
        format_version: vault.format_version,
    })
    .map_err(|e| VaultError::Io(format!("failed to serialize meta.json: {e}")))?;
    std::fs::write(vault_data_dir.join("meta.json"), meta_json)
        .map_err(|e| VaultError::Io(format!("failed to write meta.json: {e}")))?;

    // Write one file per namespace
    for ns in &vault.namespaces {
        let json = serde_json::to_string_pretty(ns)
            .map_err(|e| VaultError::Io(format!("failed to serialize namespace {}: {e}", ns.id)))?;
        std::fs::write(ns_dir.join(format!("{}.json", ns.id)), json)
            .map_err(|e| VaultError::Io(format!("failed to write namespace {}: {e}", ns.id)))?;
    }

    // Write one file per secret
    for secret in &vault.secrets {
        let json = serde_json::to_string_pretty(secret).map_err(|e| {
            VaultError::Io(format!("failed to serialize secret {}: {e}", secret.id))
        })?;
        std::fs::write(secrets_dir.join(format!("{}.json", secret.id)), json)
            .map_err(|e| VaultError::Io(format!("failed to write secret {}: {e}", secret.id)))?;
    }

    Ok(())
}

// ── Read from disk ───────────────────────────────────────────────────────────

/// Read a `VaultJson` snapshot from the working-directory JSON files.
///
/// Used after `git reset --hard` (or a fresh clone) to import the remote state
/// into SQLite without a round-trip through `git show`.  Corrupt or missing
/// files are skipped with a warning rather than aborting the import.
pub fn read_vault_from_dir(vault_data_dir: &Path) -> Result<VaultJson, VaultError> {
    let ns_dir = vault_data_dir.join("namespaces");
    let secrets_dir = vault_data_dir.join("secrets");

    let mut namespaces: Vec<NamespaceJson> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&ns_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<NamespaceJson>(&content) {
                    Ok(ns) => namespaces.push(ns),
                    Err(e) => tracing::warn!(
                        target: "vaultor::git",
                        path = %path.display(),
                        error = %e,
                        "skipping corrupt namespace file"
                    ),
                },
                Err(e) => tracing::warn!(
                    target: "vaultor::git",
                    path = %path.display(),
                    error = %e,
                    "failed to read namespace file"
                ),
            }
        }
    }

    let mut secrets: Vec<SecretJson> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&secrets_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<SecretJson>(&content) {
                    Ok(s) => secrets.push(s),
                    Err(e) => tracing::warn!(
                        target: "vaultor::git",
                        path = %path.display(),
                        error = %e,
                        "skipping corrupt secret file"
                    ),
                },
                Err(e) => tracing::warn!(
                    target: "vaultor::git",
                    path = %path.display(),
                    error = %e,
                    "failed to read secret file"
                ),
            }
        }
    }

    Ok(VaultJson {
        format_version: 1,
        created_at: now_ms(),
        namespaces,
        secrets,
    })
}

// ── Replace vault ─────────────────────────────────────────────────────────────

/// Clear all vault data from SQLite and import a fresh snapshot.
///
/// Used when switching active git repositories: the local SQLite is replaced
/// wholesale with the new repo's contents rather than merged.  This prevents
/// data from one repo leaking into another.
pub async fn replace_vault(pool: &SqlitePool, vault: &VaultJson) -> Result<(), VaultError> {
    // Delete all existing data (children before parents to respect FK order).
    for stmt in &[
        "DELETE FROM kv_fields",
        "DELETE FROM file_secrets",
        "DELETE FROM secrets",
        "DELETE FROM namespaces",
    ] {
        sqlx::query(stmt)
            .execute(pool)
            .await
            .map_err(|e| VaultError::Database(e.to_string()))?;
    }

    import_vault(pool, vault).await
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::TempDir;

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
                value_nonce BLOB NOT NULL, hidden INTEGER NOT NULL DEFAULT 1,
                sort_order INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE file_secrets (
                id TEXT PRIMARY KEY, secret_id TEXT NOT NULL,
                filename TEXT NOT NULL, content_enc BLOB NOT NULL,
                content_nonce BLOB NOT NULL, size_bytes INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    fn make_vault() -> VaultJson {
        VaultJson {
            format_version: 1,
            created_at: 1_000_000,
            namespaces: vec![NamespaceJson {
                id: "ns-1".into(),
                name: "Work".into(),
                created_at: 1_000,
                updated_at: 2_000,
            }],
            secrets: vec![SecretJson {
                id: "sec-1".into(),
                namespace_id: "ns-1".into(),
                name: "API Key".into(),
                kind: "kv".into(),
                is_draft: false,
                created_at: 1_000,
                updated_at: 2_000,
                kv_fields: vec![KvFieldJson {
                    id: "field-1".into(),
                    title: "Key".into(),
                    value_enc: B64.encode(b"ciphertext"),
                    value_nonce: B64.encode(b"nonce123456!"),
                    hidden: true,
                    sort_order: 0,
                    updated_at: 2_000,
                }],
                file_secret: None,
            }],
        }
    }

    #[test]
    fn vault_json_round_trip() {
        let v = make_vault();
        let json = serde_json::to_string(&v).unwrap();
        let v2: VaultJson = serde_json::from_str(&json).unwrap();
        assert_eq!(v2.namespaces[0].id, "ns-1");
        assert_eq!(v2.secrets[0].kv_fields[0].title, "Key");
    }

    #[tokio::test]
    async fn import_and_re_export_round_trip() {
        let pool = test_pool().await;
        let original = make_vault();

        import_vault(&pool, &original).await.unwrap();

        let exported = export_vault(&pool).await.unwrap();
        assert_eq!(exported.namespaces.len(), 1);
        assert_eq!(exported.namespaces[0].name, "Work");
        assert_eq!(exported.secrets.len(), 1);
        assert_eq!(exported.secrets[0].kv_fields.len(), 1);
        assert_eq!(exported.secrets[0].kv_fields[0].title, "Key");
        assert_eq!(
            exported.secrets[0].kv_fields[0].value_enc,
            B64.encode(b"ciphertext")
        );
    }

    #[tokio::test]
    async fn import_upsert_updates_newer() {
        let pool = test_pool().await;
        let original = make_vault();
        import_vault(&pool, &original).await.unwrap();

        // Re-import with updated namespace name and newer updated_at
        let updated = VaultJson {
            namespaces: vec![NamespaceJson {
                id: "ns-1".into(),
                name: "Work Updated".into(),
                created_at: 1_000,
                updated_at: 9_999,
            }],
            secrets: vec![],
            ..original
        };
        import_vault(&pool, &updated).await.unwrap();

        let ns = namespace::list(&pool).await.unwrap();
        assert_eq!(ns[0].name, "Work Updated");
    }

    #[tokio::test]
    async fn import_upsert_ignores_older() {
        let pool = test_pool().await;
        let original = make_vault();
        import_vault(&pool, &original).await.unwrap();

        // Re-import with stale updated_at — should NOT overwrite
        let stale = VaultJson {
            namespaces: vec![NamespaceJson {
                id: "ns-1".into(),
                name: "Stale Name".into(),
                created_at: 1_000,
                updated_at: 1, // older than 2_000
            }],
            secrets: vec![],
            ..VaultJson {
                format_version: 1,
                created_at: 1_000_000,
                namespaces: vec![],
                secrets: vec![],
            }
        };
        import_vault(&pool, &stale).await.unwrap();

        let ns = namespace::list(&pool).await.unwrap();
        assert_eq!(ns[0].name, "Work"); // unchanged
    }

    // ── write_vault_to_dir tests ─────────────────────────────────────────────

    #[test]
    fn write_vault_creates_expected_files() {
        let tmp = TempDir::new().unwrap();
        let vault_data = tmp.path().join("vault-data");
        let vault = make_vault();

        write_vault_to_dir(&vault, &vault_data).unwrap();

        assert!(vault_data.join("meta.json").exists(), "meta.json missing");
        assert!(
            vault_data.join("namespaces").join("ns-1.json").exists(),
            "namespace file missing"
        );
        assert!(
            vault_data.join("secrets").join("sec-1.json").exists(),
            "secret file missing"
        );
    }

    #[test]
    fn write_vault_meta_json_is_valid() {
        let tmp = TempDir::new().unwrap();
        let vault_data = tmp.path().join("vault-data");
        let vault = make_vault();

        write_vault_to_dir(&vault, &vault_data).unwrap();

        let meta_str = std::fs::read_to_string(vault_data.join("meta.json")).unwrap();
        let meta: serde_json::Value = serde_json::from_str(&meta_str).unwrap();
        assert_eq!(meta["format_version"], 1);
        // created_at is intentionally omitted from meta.json to avoid spurious commits
        assert!(meta.get("created_at").is_none() || meta["created_at"].is_null());
    }

    #[test]
    fn write_vault_secret_json_contains_encrypted_blob() {
        let tmp = TempDir::new().unwrap();
        let vault_data = tmp.path().join("vault-data");
        let vault = make_vault();

        write_vault_to_dir(&vault, &vault_data).unwrap();

        let sec_str =
            std::fs::read_to_string(vault_data.join("secrets").join("sec-1.json")).unwrap();
        let sec: serde_json::Value = serde_json::from_str(&sec_str).unwrap();
        assert_eq!(sec["id"], "sec-1");
        assert_eq!(sec["kind"], "kv");
        // value_enc is base64 — must be present and non-empty
        let enc = sec["kv_fields"][0]["value_enc"].as_str().unwrap();
        assert!(!enc.is_empty());
        // Verify it decodes back to the original ciphertext
        assert_eq!(B64.decode(enc).unwrap(), b"ciphertext");
    }

    #[test]
    fn write_vault_idempotent_overwrite() {
        let tmp = TempDir::new().unwrap();
        let vault_data = tmp.path().join("vault-data");
        let vault = make_vault();

        // Write twice — second call must not fail.
        write_vault_to_dir(&vault, &vault_data).unwrap();
        write_vault_to_dir(&vault, &vault_data).unwrap();

        // Content still valid after second write.
        let meta_str = std::fs::read_to_string(vault_data.join("meta.json")).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&meta_str).is_ok());
    }

    #[test]
    fn write_vault_empty_vault() {
        let tmp = TempDir::new().unwrap();
        let vault_data = tmp.path().join("vault-data");
        let vault = VaultJson {
            format_version: 1,
            created_at: 0,
            namespaces: vec![],
            secrets: vec![],
        };

        write_vault_to_dir(&vault, &vault_data).unwrap();

        assert!(vault_data.join("meta.json").exists());
        assert!(vault_data.join("namespaces").exists());
        assert!(vault_data.join("secrets").exists());
    }
}
