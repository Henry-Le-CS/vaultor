# Vaultor — Architecture Reference

> Last updated: 2026-05-14
> Source of truth for system design, module boundaries, data model, and security model.

---

## 1. Overview

Vaultor is a single-user macOS desktop password manager built with **Tauri v2** (Rust backend + Svelte 5 frontend). All data is encrypted at rest using AES-256-GCM. Authentication is via macOS TouchID. No cloud dependencies in the base configuration.

```
┌──────────────────────────────────────────┐
│              macOS desktop               │
│                                          │
│  ┌─────────────────────────────────────┐ │
│  │          Vaultor.app                │ │
│  │                                     │ │
│  │  ┌─────────────┐  ┌──────────────┐ │ │
│  │  │  WebView     │  │  Rust core   │ │ │
│  │  │  (Svelte 5)  │◄─┤  (Tauri IPC) │ │ │
│  │  └─────────────┘  └──────┬───────┘ │ │
│  │                           │         │ │
│  │                    ┌──────▼───────┐ │ │
│  │                    │   SQLite DB  │ │ │
│  │                    │  (encrypted) │ │ │
│  │                    └─────────────┘ │ │
│  └─────────────────────────────────────┘ │
│                                          │
│  macOS Keychain  ←→  encryption key      │
│  TouchID / LA    ←→  biometric gate      │
└──────────────────────────────────────────┘
```

---

## 2. Repository Layout

```
vault/
├── apps/
│   └── vaultor/
│       ├── src-tauri/              # Rust backend
│       │   ├── src/
│       │   │   ├── main.rs         # entry point
│       │   │   ├── lib.rs          # Tauri builder + state registration
│       │   │   ├── error.rs        # VaultError (thiserror)
│       │   │   ├── features/
│       │   │   │   ├── auth/
│       │   │   │   │   ├── session.rs      # in-memory SessionStore + expiry
│       │   │   │   │   └── touchid.rs      # LocalAuthentication FFI (touchid.m)
│       │   │   │   ├── keychain/
│       │   │   │   │   └── mod.rs          # KeyProvider trait + KeychainKeyProvider
│       │   │   │   ├── vault/
│       │   │   │   │   └── cipher.rs       # AES-256-GCM encrypt/decrypt
│       │   │   │   ├── storage/
│       │   │   │   │   ├── db.rs           # sqlx pool + migrations
│       │   │   │   │   ├── namespace.rs
│       │   │   │   │   ├── secret.rs
│       │   │   │   │   ├── kv_fields.rs
│       │   │   │   │   ├── file_secrets.rs
│       │   │   │   │   └── secret.rs
│       │   │   │   ├── settings/
│       │   │   │   │   └── config.rs       # AppSettings + SessionExpiry + GitRemoteConfig
│       │   │   │   └── git_storage/
│       │   │   │       ├── mod.rs          # VaultJson structs, export_vault, import_vault, write_vault_to_dir
│       │   │   │       ├── subprocess.rs   # GitRunner, sanitize_url, validate_url/branch
│       │   │   │       ├── merge.rs        # merge_vault, merge_secrets, merge_kv_fields
│       │   │   │       └── sync.rs         # sync() — full pull→merge→push cycle
│       │   │   └── commands/               # Tauri IPC command handlers (thin)
│       │   │       ├── auth.rs
│       │   │       ├── namespaces.rs
│       │   │       ├── secrets.rs
│       │   │       ├── files.rs
│       │   │       ├── settings.rs
│       │   │       └── git_storage.rs      # test_git_connection, connect_git_remote, sync_git, …
│       │   ├── migrations/
│       │   │   ├── 001_initial.sql
│       │   │   └── 002_kv_fields_updated_at.sql
│       │   ├── tests/
│       │   │   └── git_sync_integration.rs # integration tests (bare repo round-trip)
│       │   └── Cargo.toml
│       ├── src/                    # Svelte 5 frontend
│       │   ├── App.svelte
│       │   ├── app.css             # design tokens + global styles
│       │   └── lib/
│       │       └── components/
│       │           ├── login/      DoorScreen.svelte
│       │           ├── layout/     AppShell, NamespaceSidebar, SecretsList, SecretDetail
│       │           ├── secrets/    KeyValueEditor, FileSecretEditor
│       │           └── settings/   SettingsModal, GitConnectForm, GitSyncStatus
│       └── package.json
├── docs/
│   └── architecture.md             # this file
├── plans/
│   ├── user-stories/
│   └── technical-solutions/
└── Makefile
```

---

## 3. Data Model

### SQLite schema (`migrations/001_initial.sql`)

```sql
namespaces
  id          TEXT  PRIMARY KEY   -- UUIDv4
  name        TEXT  NOT NULL      -- plaintext (not sensitive)
  created_at  INT   NOT NULL      -- Unix ms
  updated_at  INT   NOT NULL

secrets
  id            TEXT  PRIMARY KEY
  namespace_id  TEXT  → namespaces(id) CASCADE DELETE
  name          TEXT  NOT NULL    -- plaintext label (not sensitive)
  kind          TEXT  NOT NULL    -- 'kv' | 'file'
  is_draft      INT   DEFAULT 0
  created_at    INT   NOT NULL
  updated_at    INT   NOT NULL

kv_fields
  id          TEXT  PRIMARY KEY
  secret_id   TEXT  → secrets(id) CASCADE DELETE
  title       TEXT  NOT NULL      -- plaintext field label
  value_enc   BLOB  NOT NULL      -- AES-256-GCM ciphertext
  value_nonce BLOB  NOT NULL      -- 12-byte GCM nonce
  hidden      INT   DEFAULT 1
  sort_order  INT   DEFAULT 0
  updated_at  INT   DEFAULT 0     -- added by migration 002; used for field-level merge

file_secrets
  id            TEXT  PRIMARY KEY
  secret_id     TEXT  → secrets(id) CASCADE DELETE
  filename      TEXT  NOT NULL    -- original filename (plaintext)
  content_enc   BLOB  NOT NULL    -- AES-256-GCM ciphertext
  content_nonce BLOB  NOT NULL    -- 12-byte GCM nonce
  size_bytes    INT   NOT NULL    -- original size (≤ 1 048 576 bytes)
```

### Default DB location

```
~/Library/Application Support/com.vaultor.app/vaultor.db
```

Configurable via `settings.json` (`db_path` field). Moving is done by copy-verify-update, with a restart required.

---

## 4. Encryption Model

### Cipher

**AES-256-GCM** via the `aes-gcm` crate.

Each encrypted blob is produced independently:

```
nonce (12 bytes, random via OsRng)
ciphertext = AES-256-GCM(key=32_bytes, nonce, plaintext, aad=secret_id)
```

`secret_id` is bound as **Additional Authenticated Data (AAD)**. This means:
- A ciphertext row is cryptographically tied to its `secret_id`.
- Moving a blob to a different secret (different `secret_id`) causes authentication failure on decrypt.
- Tampering with either the ciphertext or the nonce produces an error — no silent corruption.

### Encryption key

A single **256-bit random key** is generated on first launch and stored in the macOS Keychain:

```
Keychain item:
  Service  = "com.vaultor.enckey"
  Account  = "default"
  Value    = 32 random bytes
```

Access control: `kSecAccessControlBiometryCurrentSet` — the item becomes inaccessible if enrolled fingerprints change (a deliberate security feature).

The key is **never written to disk outside the Keychain**. It lives in Rust memory (`[u8; 32]`) only for the duration of an encryption or decryption operation.

### KeyProvider trait

```rust
pub trait KeyProvider: Send + Sync + 'static {
    fn get_or_create_key(&self) -> Result<[u8; 32], VaultError>;
}
```

- `KeychainKeyProvider` — real macOS Keychain (production)
- `MockKeyProvider` — fixed test key (unit tests in headless CI)

---

## 5. Authentication & Session Model

### TouchID flow

1. User clicks Login on `DoorScreen`.
2. Rust calls `touchid::prompt(reason)` on a `spawn_blocking` thread.
3. This invokes `vaultor_touchid_prompt()` (compiled ObjC, `touchid.m`) via FFI.
4. On success: `SessionStore::create_session()` is called → returns a random session token, stored in-memory with expiry = `now + session_expiry_duration`.
5. Door open animation plays; main vault UI is revealed.

### Session store

- Lives in `Arc<SessionStore>` managed by Tauri state.
- Frontend receives: `session_active: bool` + `expires_at: i64` (Unix ms).
- Session token itself never leaves Rust.
- A Rust background task emits `vault://session-expired` event after expiry.
- On expiry: frontend re-masks all visible fields and re-shows DoorScreen.

### Session expiry options

| Option | Duration |
|---|---|
| 2 minutes | Default |
| 5 minutes | — |
| 10 minutes | — |
| Until quit | No TTL |

Persisted in `settings.json` → `session_expiry`. Takes effect on next unlock.

---

## 6. IPC Boundary (Tauri Commands)

All secret decryption happens **in Rust**. Plaintext crosses the IPC boundary only when:
- A valid session token exists.
- The command explicitly requests decrypted content (`get_kv_secret`, `get_file_secret`).

The frontend never receives the encryption key or the session token.

### Registered commands

```
Auth:       unlock_vault, lock_vault, session_status
Settings:   get_settings, set_session_expiry, get_storage_location, pick_folder, move_storage
Namespaces: list_namespaces, create_namespace, rename_namespace, delete_namespace
Secrets:    list_secrets, create_kv_secret, get_kv_secret, update_kv_secret,
            delete_secret, save_kv_draft, commit_draft, discard_draft
Files:      create_file_secret, get_file_secret, update_file_secret
Git remote: test_git_connection, connect_git_remote, sync_git,
            get_git_status, disconnect_git_remote
```

---

## 7. Settings

Settings are stored at:
```
~/Library/Application Support/com.vaultor.app/settings.json
```

```json
{
  "db_path": "/optional/custom/path/vaultor.db",
  "session_expiry": "minutes_2",
  "git_remote": {
    "url": "git@github.com:user/vault.git",
    "branch": "main",
    "last_synced": 1715000000000
  }
}
```

`db_path` is `null` (absent) when using the default location. `git_remote` is `null` when git sync is not configured.

---

## 8. Frontend Architecture

- **Framework**: Svelte 5 with vanilla CSS (no Tailwind).
- **Build**: Vite + `@sveltejs/vite-plugin-svelte`.
- **IPC**: `@tauri-apps/api` `invoke()` calls.

### Key components

| Component | Role |
|---|---|
| `DoorScreen.svelte` | Full-window login, door-open CSS animation |
| `AppShell.svelte` | Three-panel layout with drag-resize handle |
| `NamespaceSidebar.svelte` | Namespace list + gear icon (opens Settings) |
| `SecretsList.svelte` | Secret rows for active namespace |
| `SecretDetail.svelte` | Field view/edit, copy, download |
| `KeyValueEditor.svelte` | KV field editor with +/remove |
| `FileSecretEditor.svelte` | File drag-and-drop + picker |
| `SettingsModal.svelte` | Settings overlay (storage location, session expiry) |

### Design tokens

```css
--brand: #cbc3e3   --brand-mid: #a897cc   --brand-dark: #9282bc
--bg-a:  #f7f3ff   --bg-b: #ede8f5        --card: #ffffff
--text:  #3d3557   --muted: #8c7faa       --border: #e4daf0
--err:   #c96b8a   --ok: #5a9a78
```

---

## 9. Build & Development

### Prerequisites

- Rust (stable)
- Node.js + npm
- macOS (TouchID + Keychain are macOS-only)

### Common commands

```bash
# Dev server with hot reload
make dev

# Production .app bundle
make build

# Build + install to /Applications
make build-install

# Open the built .app
make open

# Clean Rust artifacts
make clean
```

### Build output

`cargo tauri build` produces:
```
apps/vaultor/src-tauri/target/release/bundle/macos/Vaultor.app
```

> **Gate order**: always `npm run build` from `apps/vaultor/` before any `cargo` command —
> `tauri::generate_context!()` validates the `dist/` path at compile time.

### Gate sequence (required before any plan advances)

```bash
cd apps/vaultor && npm run build
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo test --test '*'
```

---

## 10. Security Boundaries Summary

| Boundary | What crosses it | Direction |
|---|---|---|
| Keychain ↔ Rust | 32-byte key | One-way (read) |
| TouchID ↔ Rust | Success/failure signal | One-way (read) |
| Rust ↔ IPC | Plaintext field values (session-gated) | Rust → Frontend |
| IPC ↔ Frontend | `session_active`, `expires_at`, names | Rust → Frontend |
| SQLite ↔ Rust | Encrypted blobs + nonces | Both |
| Disk | Encrypted blobs only, never plaintext | Both |

**Plaintext never touches disk.** Secret names/labels are stored plaintext (by design — they appear in the list without auth). Field values and file contents are always encrypted before any disk write.

---

## 11. Git Remote Storage

Optional encrypted backup to any git repository.

### Architecture

```
{app_data_dir}/git-repo/       — local working clone
  vault-data/
    meta.json
    namespaces/{id}.json       — one per namespace
    secrets/{id}.json          — one per secret (with kv_fields or file_secret inline)
```

All blobs in JSON are base64-encoded AES-256-GCM ciphertext — identical encryption to the SQLite path. The repo never contains plaintext values.

### Sync cycle (pull → merge → push)

1. `git fetch origin <branch>`
2. Read remote files via `git ls-tree` + `git show origin/<branch>:…`
3. Export local vault from SQLite
4. Merge: additive union by ID, newer `updated_at` wins on conflict
5. Remove stale disk files (deleted entities), write merged JSON
6. Import merged vault into SQLite
7. `git add -A vault-data/` + `git commit` + `git push --force-with-lease` (retry once on rejection)

### Authentication

System `git` subprocess inherits SSH agent and `osxkeychain` — no PAT stored by the app.

### Multi-device key limitation

The AES-256-GCM key is device-bound (macOS Keychain). A second device pulling the same repo cannot decrypt blobs without the same 32-byte key. Secure key transfer is not yet implemented.

---

## 12. Known Limitations (v1)

- macOS only — TouchID and Keychain are macOS-specific.
- No password-based fallback — if Keychain item is deleted externally, data is irrecoverable.
- Clipboard auto-clear not implemented.
- Zeroize in JavaScript WebView is not guaranteed (JS GC timing).
- `kSecAccessControlBiometryCurrentSet` — fingerprint changes invalidate the Keychain item.
- Git remote: encryption key is device-bound — multi-device sync requires manual key transfer (not yet implemented).
- Git remote: no automatic background sync (manual "Sync Now" only in v1).
