# Vaultor

A secure, local password manager for macOS. All data stays on your machine — no cloud, no accounts.

---

## Architecture

### Overview

Vaultor is a [Tauri v2](https://tauri.app) desktop application. The Rust process owns all security-sensitive logic; the Svelte frontend is a thin display layer.

```
┌─────────────────────────────────────────┐
│           Svelte 5 Frontend             │
│  DoorScreen  │  AppShell  │  Stores     │
│              │            │             │
│         Tauri IPC (invoke)              │
└──────────────────┬──────────────────────┘
                   │
┌──────────────────▼──────────────────────┐
│            Rust Backend                 │
│                                         │
│  features/auth     Session token        │
│  features/keychain macOS Keychain key   │
│  features/vault    AES-256-GCM cipher   │
│  features/storage  SQLite repositories  │
│                                         │
│  commands/         Tauri IPC handlers   │
└─────────────────────────────────────────┘
         │                    │
   macOS Keychain          SQLite DB
   (TouchID-gated)  ~/Library/Application
                    Support/Vaultor/
```

### Technology stack

| Layer | Technology |
|---|---|
| Application framework | Tauri v2 |
| Frontend | Svelte 5 + Vite 6 |
| Encryption | AES-256-GCM (per-record, random nonce) |
| Key storage | macOS Keychain (`security-framework` crate) |
| Biometric auth | TouchID via macOS LocalAuthentication |
| Storage | SQLite via `sqlx` |
| Memory safety | `secrecy` + `zeroize` crates |

### Project structure

```
apps/
  vaultor/
    src-tauri/                Rust backend
      src/
        main.rs               Binary entry point
        lib.rs                Tauri app builder
        error.rs              Unified VaultError type
        features/
          auth/               Session token + TouchID bridge
          keychain/           Keychain key generation & retrieval
          vault/              AES-256-GCM encrypt/decrypt
          storage/            SQLite repositories (Phase 3+)
          settings/           AppSettings, SessionExpiry, load/save (settings.json)
        commands/             Tauri IPC command handlers
      migrations/
        001_initial.sql       Full database schema
      capabilities/
        default.json          Tauri permission set
      icons/                  App icons (RGBA PNG + .icns)
      Cargo.toml
      tauri.conf.json
    src/                      Svelte frontend
      App.svelte              Root component
      app.css                 Design tokens + global styles
      lib/
        api.ts                Typed Tauri invoke() wrappers
        stores/
          session.ts          2-minute session store
        components/
          login/
            DoorScreen.svelte Double-door login animation
          layout/
            AppShell.svelte   Three-panel shell
            NamespaceSidebar  Namespace switcher (gear icon → Settings)
            SecretsList       Secrets list panel (inline delete)
            SecretDetail      Secret detail panel
          settings/
            SettingsModal     Storage location + session expiry
          shared/
            ConfirmModal      Reusable confirm dialog (typed-phrase support)
    package.json
docs/
plans/
  user-stories/
  technical-solutions/
  executions/
```

### Authentication & encryption flow

1. App opens → `DoorScreen` shown (two animated panels, logo at seam).
2. User clicks **Login** → TouchID prompt via macOS LocalAuthentication.
3. On success → Rust retrieves 256-bit key from Keychain (Secure Enclave-backed), creates a 2-minute in-memory session token.
4. Doors animate open → `AppShell` revealed.
5. All secret field values and file contents are encrypted with **AES-256-GCM**:
   - Random 12-byte nonce per record.
   - `secret_id` used as AAD (additional authenticated data) — ciphertext is row-bound.
6. Plaintext exists only in Rust memory, wrapped in `secrecy::SecretString`, and is zeroized on drop.
7. After the configured timeout (2 / 5 / 10 min, or never for "Until quit"), the session expires → Tauri emits a `vault://session-expired` event → frontend re-masks all fields.

### UI layout

```
┌──────┬──────────────────┬─────────────────────────────┐
│      │                  │                             │
│  NS  │  Secrets List    │     Secret Detail           │
│      │                  │                             │
│  [A] │  API Keys        │  Field: Token               │
│  [B] │  DB Passwords *  │  Value: ••••••   [Copy]     │
│      │  SSH Keys        │                             │
│  [+] │  [ New ]         │  (nothing selected →        │
│      │                  │   "No secret selected")     │
└──────┴──────────────────┴─────────────────────────────┘
NS = Namespace sidebar (64 px, fixed)   * = unsaved draft
```

All three panels are user-resizable via drag handles (Phase 8).

### Database schema

```sql
namespaces   id, name, created_at, updated_at
secrets      id, namespace_id, name, kind ('kv'|'file'), is_draft, timestamps
kv_fields    id, secret_id, title, value_enc, value_nonce, hidden, sort_order
file_secrets id, secret_id, filename, content_enc, content_nonce, size_bytes
```

Secret names are plaintext (visible without auth). All `*_enc` columns are AES-256-GCM ciphertext.

---

## Supported Features

- [x] Double-door login screen with logo animation
- [x] Three-panel desktop layout (namespace sidebar, secrets list, secret detail)
- [x] Design token system (lavender brand palette)
- [x] Session store with configurable auto-expiry (2 / 5 / 10 min or Until quit)
- [x] Full SQLite schema defined
- [x] Namespace CRUD (create, rename, delete with typed confirmation)
- [x] Key-value and file secrets (create, read, delete)
- [x] AES-256-GCM encryption for all secret values
- [x] TouchID authentication via macOS LocalAuthentication
- [x] Encryption key stored in macOS Keychain (`kSecAccessControlBiometryCurrentSet`)
- [x] Settings modal — view/copy/move vault file location, change session expiry
- [x] Native folder picker (tauri-plugin-dialog)
- [x] Session persists settings across restarts (`settings.json` in app config dir)
- [x] Git remote backup — encrypted push/pull to any private git repository
- [x] Additive merge with field-level conflict resolution (`updated_at` wins)
- [x] Git connection test and status panel (`GitSyncStatus`)

## Not Yet Implemented

| Feature | Notes |
|---|---|
| File secrets drag-and-drop + download | File stored/encrypted; no DnD UI yet |
| Encrypted draft persistence | Drafts live in memory only |
| Resizable panels | Panel widths fixed |
| Multi-device key transfer | Git sync works; decryption on a second device requires manual key export (not implemented) |
| Clipboard auto-clear | Copied values persist in clipboard until manually cleared |
| Background / automatic git sync | Sync is manual ("Sync Now" only) |

---

## Usage

### Prerequisites (development only)

| Tool | Version |
|---|---|
| Rust | 1.77.2 or later |
| Node.js | 18 or later |
| npm | 9 or later |
| macOS | 13 (Ventura) or later |
| Xcode Command Line Tools | required for macOS SDK |

End users need **none of the above** — see [Building a release](#building-a-release).

### Development

```bash
# 1. Install frontend dependencies (first time only)
cd apps/vaultor
npm install

# 2. Start the development server (Vite + Tauri hot-reload)
npm run tauri -- dev
```

The app window will open automatically. Hot-reload applies to Svelte changes. Rust changes trigger a Cargo recompile.

### Running checks

```bash
# From apps/vaultor — frontend type check
npm run check

# From apps/vaultor — build frontend (required before Rust gates)
npm run build

# From apps/vaultor/src-tauri — Rust gates
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

> **Important:** `cargo` commands must be run after `npm run build` because
> `tauri::generate_context!()` validates the `frontendDist` path (`../dist`)
> at compile time.

---

## Building a release

The output is a **standalone macOS `.app` bundle** — no Rust, Node.js, or any
other toolchain required on the end-user machine. WKWebView is a macOS system
component and ships with the OS.

```bash
cd apps/vaultor

# Build frontend then Tauri release bundle
npm run build
npm run tauri -- build
```

Output location:

```
apps/vaultor/src-tauri/target/release/bundle/macos/Vaultor.app
apps/vaultor/src-tauri/target/release/bundle/dmg/Vaultor_0.1.0_aarch64.dmg
```

Distribute either the `.app` (drag to `/Applications`) or the `.dmg` installer.

### Code signing (optional for personal use)

Without signing, macOS will show an "unidentified developer" warning on first open.
To dismiss it: right-click → Open → Open.

For distribution, sign with an Apple Developer certificate:

```bash
# Set your signing identity in tauri.conf.json under bundle.macOS.signingIdentity
# or pass via environment:
APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
npm run tauri -- build
```
