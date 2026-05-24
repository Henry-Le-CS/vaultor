#!/usr/bin/env bash
set -euo pipefail

# ── Vaultor uninstaller ─────────────────────────────────────────────────────
# Removes Vaultor.app, its data, settings, logs, and Keychain entry.

APP_NAME="Vaultor"
APP_PATH="/Applications/${APP_NAME}.app"
DATA_DIR="$HOME/Library/Application Support/com.vaultor.app"
LOG_DIR="$HOME/Library/Logs/Vaultor"
KEYCHAIN_SERVICE="com.vaultor.enckey"
KEYCHAIN_ACCOUNT="default"

echo "Vaultor Uninstaller"
echo "==================="
echo ""
echo "This will remove:"
echo "  - ${APP_PATH}"
echo "  - ${DATA_DIR}  (vault database, settings, git repo clones)"
echo "  - ${LOG_DIR}  (application logs)"
echo "  - Keychain entry: ${KEYCHAIN_SERVICE}"
echo ""

# ── Confirm ──────────────────────────────────────────────────────────────────

read -rp "Are you sure you want to uninstall Vaultor and delete ALL vault data? (y/N) " answer
if [[ ! "$answer" =~ ^[Yy]$ ]]; then
  echo "Cancelled."
  exit 0
fi

echo ""

# ── Quit the app if running ──────────────────────────────────────────────────

if pgrep -xq "$APP_NAME"; then
  echo "Closing ${APP_NAME}..."
  osascript -e "tell application \"${APP_NAME}\" to quit" 2>/dev/null || true
  sleep 1
  # Force-kill if still running
  pkill -x "$APP_NAME" 2>/dev/null || true
fi

# ── Remove the app bundle ────────────────────────────────────────────────────

if [ -d "$APP_PATH" ]; then
  echo "Removing ${APP_PATH}..."
  rm -rf "$APP_PATH"
else
  echo "App not found at ${APP_PATH} (already removed?)."
fi

# ── Remove application data ─────────────────────────────────────────────────

if [ -d "$DATA_DIR" ]; then
  echo "Removing data directory..."
  rm -rf "$DATA_DIR"
else
  echo "Data directory not found (already removed?)."
fi

# ── Remove logs ──────────────────────────────────────────────────────────────

if [ -d "$LOG_DIR" ]; then
  echo "Removing log directory..."
  rm -rf "$LOG_DIR"
else
  echo "Log directory not found (already removed?)."
fi

# ── Remove Keychain entry ────────────────────────────────────────────────────

echo "Removing Keychain entry..."
security delete-generic-password \
  -s "$KEYCHAIN_SERVICE" \
  -a "$KEYCHAIN_ACCOUNT" 2>/dev/null && echo "Keychain entry removed." \
  || echo "Keychain entry not found (already removed?)."

echo ""
echo "Vaultor has been completely uninstalled."
