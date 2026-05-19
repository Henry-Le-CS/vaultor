#!/usr/bin/env bash
set -euo pipefail

# ── Vaultor installer ────────────────────────────────────────────────────────
# Downloads the latest Vaultor release DMG and installs to /Applications.

REPO="HenryLeCS/vault"
APP_NAME="Vaultor"

echo "Fetching latest release..."
LATEST=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" \
         | grep '"tag_name"' | sed 's/.*"v\([^"]*\)".*/\1/')

if [ -z "$LATEST" ]; then
  echo "Error: could not determine latest version." >&2
  exit 1
fi

DMG="${APP_NAME}-${LATEST}-macos.dmg"
URL="https://github.com/$REPO/releases/download/v${LATEST}/${DMG}"

echo "Downloading ${APP_NAME} v${LATEST}..."
curl -L -o "/tmp/${DMG}" "$URL"

echo "Installing to /Applications..."
hdiutil attach "/tmp/${DMG}" -mountpoint /tmp/vaultor-mount -quiet
cp -r "/tmp/vaultor-mount/${APP_NAME}.app" "/Applications/"
hdiutil detach /tmp/vaultor-mount -quiet
rm "/tmp/${DMG}"

# Remove Gatekeeper quarantine (required for unsigned builds).
xattr -d com.apple.quarantine "/Applications/${APP_NAME}.app" 2>/dev/null || true

echo ""
echo "${APP_NAME} v${LATEST} installed successfully."
echo "Launch: open /Applications/${APP_NAME}.app"
