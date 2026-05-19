//! Integration tests for the git remote storage round-trip.
//!
//! These tests exercise the full disk ↔ git ↔ SQLite path using real git
//! subprocesses and a temporary bare repository as the "remote".
//!
//! Requirements: `git` must be on PATH (always true on macOS).

use std::path::{Path, PathBuf};
use std::process::Command;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use tempfile::TempDir;

use vaultor_lib::features::git_storage::subprocess::GitRunner;
use vaultor_lib::features::git_storage::{
    export_vault, import_vault, write_vault_to_dir, NamespaceJson, SecretJson, VaultJson,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Create an in-memory SQLite pool with the minimal schema.
async fn make_pool() -> SqlitePool {
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
            sort_order INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL DEFAULT 0
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

/// Populate a pool with one namespace and one KV secret.
async fn seed_pool(pool: &SqlitePool) {
    sqlx::query("INSERT INTO namespaces VALUES ('ns-1', 'Work', 1000, 2000)")
        .execute(pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO secrets VALUES ('sec-1', 'ns-1', 'API Key', 'kv', 0, 1000, 2000)")
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO kv_fields VALUES ('f-1', 'sec-1', 'Key', X'deadbeef', X'cafebabe000000000000', 1, 0, 2000)",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// Initialise a bare git repo and return its path.
fn init_bare_repo(parent: &Path) -> PathBuf {
    let bare = parent.join("remote.git");
    let status = Command::new("git")
        .args(["init", "--bare", bare.to_str().unwrap()])
        .output()
        .expect("git init --bare")
        .status;
    assert!(status.success(), "git init --bare failed");
    bare
}

/// Clone a bare repo into a working directory and set up git identity.
fn clone_working(bare: &Path, parent: &Path, name: &str) -> PathBuf {
    let working = parent.join(name);
    let status = Command::new("git")
        .args(["clone", bare.to_str().unwrap(), working.to_str().unwrap()])
        .output()
        .expect("git clone")
        .status;
    assert!(status.success(), "git clone failed for {name}");

    // Configure identity so commits don't fail in CI.
    for (key, val) in [
        ("user.email", "test@vaultor"),
        ("user.name", "Vaultor Test"),
    ] {
        Command::new("git")
            .args(["-C", working.to_str().unwrap(), "config", key, val])
            .output()
            .expect("git config");
    }

    working
}

/// Push an initial commit so the branch exists on the bare remote.
fn push_initial_commit(working: &Path) {
    // Write a placeholder file so git can make a commit.
    std::fs::create_dir_all(working.join("vault-data")).unwrap();
    std::fs::write(working.join("vault-data").join(".gitkeep"), b"").unwrap();

    for args in [
        vec!["add", "."],
        vec!["commit", "--author=Test <test@vaultor>", "-m", "init"],
        vec!["push", "origin", "HEAD"],
    ] {
        let status = Command::new("git")
            .args(&args)
            .current_dir(working)
            .output()
            .expect("git command")
            .status;
        assert!(status.success(), "initial commit step {:?} failed", args);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Export a vault, write to a git working tree, commit, push to a bare repo,
/// then clone into a second working tree and verify the files round-trip.
#[tokio::test]
async fn write_and_read_via_bare_repo() {
    let tmp = TempDir::new().unwrap();

    // Setup: bare remote + device-1 working clone.
    let bare = init_bare_repo(tmp.path());
    let device1 = clone_working(&bare, tmp.path(), "device1");
    push_initial_commit(&device1);

    // Populate device-1 pool and export vault.
    let pool = make_pool().await;
    seed_pool(&pool).await;
    let vault = export_vault(&pool).await.unwrap();

    // Write vault JSON into device-1 working tree.
    let vault_data_dir = device1.join("vault-data");
    write_vault_to_dir(&vault, &vault_data_dir).unwrap();

    // Commit and push via GitRunner.
    let runner = GitRunner::new(&device1);
    runner.add_vault_data_all().unwrap();
    let committed = runner.commit("Vaultor connect test").unwrap();
    assert!(committed, "expected commit to succeed");

    let branch = "main";
    let pushed = runner.push_force_with_lease(branch).unwrap();
    assert!(pushed, "push to bare repo should succeed");

    // Device 2: clone from the same bare remote.
    let device2 = clone_working(&bare, tmp.path(), "device2");
    let runner2 = GitRunner::new(&device2);

    // List remote namespace files via git ls-tree.
    let ns_files = runner2
        .list_remote_dir(branch, "vault-data/namespaces/")
        .unwrap();
    assert_eq!(
        ns_files.len(),
        1,
        "expected 1 namespace file, got {ns_files:?}"
    );
    assert!(
        ns_files[0].ends_with("ns-1.json"),
        "unexpected filename: {}",
        ns_files[0]
    );

    // Read and verify namespace JSON.
    let ns_content = runner2
        .show_remote_file(branch, "vault-data/namespaces/ns-1.json")
        .unwrap();
    let ns: NamespaceJson = serde_json::from_str(&ns_content).expect("valid namespace JSON");
    assert_eq!(ns.id, "ns-1");
    assert_eq!(ns.name, "Work");

    // Read and verify secret JSON.
    let secret_files = runner2
        .list_remote_dir(branch, "vault-data/secrets/")
        .unwrap();
    assert_eq!(secret_files.len(), 1);
    let sec_content = runner2
        .show_remote_file(branch, "vault-data/secrets/sec-1.json")
        .unwrap();
    let sec: SecretJson = serde_json::from_str(&sec_content).expect("valid secret JSON");
    assert_eq!(sec.id, "sec-1");
    assert_eq!(sec.kind, "kv");
    assert_eq!(sec.kv_fields.len(), 1);
    assert_eq!(sec.kv_fields[0].title, "Key");
}

/// Verify that import_vault round-trips through export correctly.
/// This is the SQLite side of the integration story.
#[tokio::test]
async fn export_write_clone_import_roundtrip() {
    let tmp = TempDir::new().unwrap();

    // Setup.
    let bare = init_bare_repo(tmp.path());
    let device1 = clone_working(&bare, tmp.path(), "device1");
    push_initial_commit(&device1);

    // Export from device-1 pool.
    let pool1 = make_pool().await;
    seed_pool(&pool1).await;
    let vault = export_vault(&pool1).await.unwrap();

    // Write + commit + push.
    write_vault_to_dir(&vault, &device1.join("vault-data")).unwrap();
    let runner1 = GitRunner::new(&device1);
    runner1.add_vault_data_all().unwrap();
    runner1.commit("Vaultor connect").unwrap();
    runner1.push_force_with_lease("main").unwrap();

    // Device 2: fresh pool, clone from bare remote and import.
    let pool2 = make_pool().await;
    let device2 = clone_working(&bare, tmp.path(), "device2");
    let runner2 = GitRunner::new(&device2);

    // Read remote namespaces.
    let ns_files = runner2
        .list_remote_dir("main", "vault-data/namespaces/")
        .unwrap();
    let mut namespaces: Vec<NamespaceJson> = Vec::new();
    for f in &ns_files {
        if !f.ends_with(".json") {
            continue;
        }
        let content = runner2
            .show_remote_file("main", &format!("vault-data/namespaces/{f}"))
            .unwrap();
        namespaces.push(serde_json::from_str(&content).unwrap());
    }

    // Read remote secrets.
    let sec_files = runner2
        .list_remote_dir("main", "vault-data/secrets/")
        .unwrap();
    let mut secrets: Vec<SecretJson> = Vec::new();
    for f in &sec_files {
        if !f.ends_with(".json") {
            continue;
        }
        let content = runner2
            .show_remote_file("main", &format!("vault-data/secrets/{f}"))
            .unwrap();
        secrets.push(serde_json::from_str(&content).unwrap());
    }

    // Import into device-2 pool.
    let remote_vault = VaultJson {
        format_version: 1,
        created_at: 0,
        namespaces,
        secrets,
    };
    import_vault(&pool2, &remote_vault).await.unwrap();

    // Re-export from pool2 and verify it matches original.
    let re_exported = export_vault(&pool2).await.unwrap();
    assert_eq!(re_exported.namespaces.len(), 1);
    assert_eq!(re_exported.namespaces[0].id, "ns-1");
    assert_eq!(re_exported.namespaces[0].name, "Work");
    assert_eq!(re_exported.secrets.len(), 1);
    assert_eq!(re_exported.secrets[0].id, "sec-1");
    assert_eq!(re_exported.secrets[0].kv_fields[0].title, "Key");
    // Verify encrypted blob survived the round-trip.
    assert_eq!(
        re_exported.secrets[0].kv_fields[0].value_enc,
        B64.encode([0xde, 0xad, 0xbe, 0xef]),
    );
}
