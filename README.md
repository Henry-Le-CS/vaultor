# Vaultor

[![CI](https://github.com/HenryLeCS/vault/actions/workflows/ci.yml/badge.svg)](https://github.com/HenryLeCS/vault/actions/workflows/ci.yml)

A private, offline password manager for macOS. Your secrets never leave your device unless you choose to back them up yourself.

---

## What it does

Vaultor stores your passwords, API keys, SSH credentials, and other sensitive files on your Mac — encrypted, locked behind your fingerprint, and completely offline.

- No account to create
- No subscription
- No cloud service to trust or breach
- No ads

Your data lives in a single encrypted file on your Mac. Only your fingerprint (TouchID) can open it.

---

## Why use Vaultor over 1Password / Bitwarden / LastPass?

| | Vaultor | Cloud password managers |
|---|---|---|
| Data location | Your Mac only | Their servers |
| Account required | No | Yes |
| Works offline | Always | Sometimes |
| Monthly cost | Free | $3–5/month |
| Breach risk | Local only | Cloud + your device |

If you want complete control over where your data lives, Vaultor is for you.

---

## Requirements

- **macOS 13 (Ventura) or later**
- A Mac with **TouchID** (Touch Bar, Touch ID button, or Magic Keyboard with Touch ID)

That's it. No additional software needed to run Vaultor.

---

## Installation

### Option A — One-line install (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/HenryLeCS/vault/main/install.sh | bash
```

This downloads the latest `.dmg` from GitHub Releases, installs Vaultor to `/Applications`, and removes the Gatekeeper quarantine flag.

### Option B — Download manually

Grab the latest `.dmg` from the [Releases](../../releases) page:

1. Download **Vaultor-x.y.z-macos.dmg**
2. Open the DMG and drag **Vaultor** into Applications
3. On first launch: right-click → Open → Open (unsigned app workaround)

### First launch warning

macOS will show a warning on first launch because Vaultor is not yet signed with an Apple Developer certificate. To open it anyway:

1. Right-click (or Control-click) on `Vaultor.app`
2. Select **Open**
3. Click **Open** in the dialog

You only need to do this once. Alternatively, run:

```bash
xattr -d com.apple.quarantine /Applications/Vaultor.app
```

---

## Getting started

When you first open Vaultor, an **onboarding tutorial** walks you through the basics — creating namespaces, adding secrets, copying values, and configuring backups. You can re-open the tutorial at any time by clicking the **?** button in the bottom-right corner.

---

## How to use it

### Unlocking

When you open Vaultor, a door screen appears. Click **Login** and authenticate with your fingerprint. The vault unlocks and stays open until it auto-locks (2 minutes by default) or you close the app.

### Organizing your secrets

Secrets are grouped into **namespaces** — think of them as folders. Create a namespace for each context: *Work*, *Personal*, *Dev*, *Finance*, etc.

Click the **+** button in the left sidebar to add a namespace.

### Adding a secret

1. Select a namespace from the left sidebar
2. Click **New** in the middle panel
3. Give it a name (e.g. *GitHub token*, *AWS credentials*)
4. Add key-value fields — for example:
   - Field: `Username`, Value: `alice@example.com`
   - Field: `Password`, Value: `hunter2`
5. Click **Save**

### Viewing a secret

Click any secret in the list. Values are hidden by default (shown as bullets). Click the eye icon to reveal a field, or the copy icon to copy it directly to your clipboard without displaying it.

### File secrets

You can also store files — SSH private keys, certificates, config files. Drag a file onto the detail panel or use the file picker. The file is encrypted and stored the same way as text secrets.

### Namespaces

- **Rename**: double-click a namespace name
- **Delete**: right-click → Delete (requires you to type the namespace name to confirm — this deletes all secrets inside)

### Auto-lock

The vault automatically locks after a configurable period of inactivity. To change it:

1. Click the gear icon (bottom of the left sidebar)
2. Under **Session expiry**, choose 2 minutes, 5 minutes, 10 minutes, or *Until I quit the app*

---

## Backing up your vault (optional)

By default, your vault is a single file:
```
~/Library/Application Support/com.vaultor.app/vaultor.db
```

You can move it anywhere on your Mac via **Settings → Storage location**.

### Git sync (advanced)

Vaultor can push an encrypted backup to any private git repository (GitHub, GitLab, Gitea, etc.). Only ciphertext is ever pushed — the repo contains no readable passwords. To configure it:

1. Open **Settings → Git backup**
2. Paste your repository URL (SSH or HTTPS)
3. Enter the branch name
4. Click **Connect**

Sync is manual — click **Sync now** whenever you want to push changes. Pull happens automatically at sync time if the remote has newer data.

> **Multi-device note:** The encryption key is tied to your Mac's Keychain. A second Mac pulling the same git repo cannot decrypt the backup without the original key. Secure key transfer between devices is not yet implemented.

---

## Security model

Vaultor's security is based on three principles:

### 1. Your fingerprint is the only key

Your 256-bit encryption key is generated once and stored in the **macOS Keychain**, protected by TouchID (`kSecAccessControlBiometryCurrentSet`). The key is:
- Never written to disk outside the Keychain
- Never visible to the app's interface layer
- Invalidated automatically if your enrolled fingerprints change

### 2. Everything sensitive is encrypted

All field values and file contents are encrypted with **AES-256-GCM** before being written to disk. Each value gets its own random encryption context — compromising one record does not compromise others. Secret names and labels are stored unencrypted (they appear in the list before you unlock), but values never are.

### 3. The app locks itself

After the configured timeout, the app wipes the in-memory session and re-locks. All visible values are masked. Your fingerprint is required again to continue.

### What Vaultor cannot protect against

- **If your Mac is unlocked and someone sits at it** — they can click Login and use your fingerprint authorization window (they'd still need your finger).
- **If your Keychain is deleted** — your encrypted data becomes permanently unrecoverable. Keep regular backups of the vault file in a safe location.
- **Clipboard contents** — copied values remain in the clipboard after copying. Vaultor does not yet auto-clear the clipboard.

---

## Known limitations

| Limitation | Details |
|---|---|
| macOS only | Keychain and TouchID are macOS-specific |
| No password fallback | If the Keychain item is lost, data is unrecoverable |
| Single-device encryption key | Multi-device git sync requires manual key transfer (not yet implemented) |
| No clipboard auto-clear | Copied values stay in clipboard until you clear it |
| No browser extension | Manual copy-paste only |

---

## Building from source

If you want to build Vaultor yourself rather than use a release binary:

**Prerequisites:**
- [Rust](https://rustup.rs) (stable, 1.77.2+)
- [Node.js](https://nodejs.org) 18+
- macOS 13+ with Xcode Command Line Tools

```bash
git clone <repo-url>
cd vault/apps/vaultor

# Install frontend dependencies
npm install

# Development (hot-reload)
npm run tauri -- dev

# Production build
npm run build
npm run tauri -- build
```

The output app bundle is at:
```
apps/vaultor/src-tauri/target/release/bundle/macos/Vaultor.app
```

---

## Contributing

Bug reports, security disclosures, and pull requests are welcome. For security issues, please open a private issue rather than a public one.

See `docs/architecture.md` for the full technical design reference.

---

## License

See [LICENSE](LICENSE) in this repository.
