#!/usr/bin/env bash
# create-dmg.sh — Build the Arcana macOS DMG installer.
# Phase 5.3 — Platform Installers.
#
# Usage:
#   ./create-dmg.sh [--sign] [--notarize] [--version 1.2.3] [--channel stable]
#
# Environment variables:
#   APPLE_ID           — Apple ID for notarization
#   APPLE_TEAM_ID      — Team ID (10-char)
#   APPLE_APP_PASSWORD — App-specific password for notarization
#   SIGNING_IDENTITY   — Code signing identity (e.g. "Developer ID Application: Your Name (ABCDE12345)")
#
# Prerequisites:
#   - Xcode Command Line Tools
#   - create-dmg (brew install create-dmg) for fancy DMG layout
#   - Tauri CLI + Rust + Node

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TAURI_DIR="$REPO_ROOT/src-tauri"
DIST_DIR="$REPO_ROOT/dist"
ENTITLEMENTS="$SCRIPT_DIR/entitlements.plist"

# ─── Defaults ─────────────────────────────────────────────────────────────────

SIGN=false
NOTARIZE=false
VERSION=""
CHANNEL="stable"

# ─── Argument parsing ─────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
  case "$1" in
    --sign)      SIGN=true;       shift ;;
    --notarize)  NOTARIZE=true;   shift ;;
    --version)   VERSION="$2";    shift 2 ;;
    --channel)   CHANNEL="$2";    shift 2 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

# ─── Helpers ──────────────────────────────────────────────────────────────────

step() { echo; echo "==> $*"; }
die()  { echo "ERROR: $*" >&2; exit 1; }

require() {
  command -v "$1" >/dev/null 2>&1 || die "Required command not found: $1. $2"
}

# ─── Prerequisites ────────────────────────────────────────────────────────────

step "Checking prerequisites"

require cargo  "Install Rust: https://rustup.rs"
require node   "Install Node.js: https://nodejs.org"
require npm    "Install Node.js: https://nodejs.org"

if [[ -z "$VERSION" ]]; then
  VERSION=$(grep -m1 '^version' "$TAURI_DIR/Cargo.toml" | sed 's/.*= *"\(.*\)"/\1/')
fi

echo "  Version  : $VERSION"
echo "  Channel  : $CHANNEL"
echo "  Sign     : $SIGN"
echo "  Notarize : $NOTARIZE"

# ─── Frontend deps ────────────────────────────────────────────────────────────

step "Installing frontend dependencies"
cd "$REPO_ROOT"
npm ci --prefer-offline

# ─── Build Tauri app ─────────────────────────────────────────────────────────

step "Building Tauri app (release, universal)"
cd "$REPO_ROOT"

export ARCANA_CHANNEL="$CHANNEL"

# Build for both architectures to create a Universal binary
# If this is CI on arm64 only, omit --target universal-apple-darwin
if [[ "${CI:-}" == "true" ]]; then
  npm run tauri -- build --bundles dmg
else
  # Local dev: match current arch for speed
  npm run tauri -- build --bundles dmg
fi

# ─── Locate built DMG ─────────────────────────────────────────────────────────

DMG_DIR="$TAURI_DIR/target/release/bundle/dmg"
DMG_FILE=$(find "$DMG_DIR" -name "*.dmg" | sort | tail -1)
[[ -f "$DMG_FILE" ]] || die "No DMG found in $DMG_DIR"

echo "  DMG: $DMG_FILE"

# ─── Code signing (hardened runtime) ──────────────────────────────────────────

if [[ "$SIGN" == "true" ]]; then
  step "Signing .app bundle"

  IDENTITY="${SIGNING_IDENTITY:-Developer ID Application}"
  APP_BUNDLE="$TAURI_DIR/target/release/bundle/macos/Arcana.app"

  [[ -d "$APP_BUNDLE" ]] || die "App bundle not found: $APP_BUNDLE"

  # Sign all binaries recursively, then the bundle itself
  find "$APP_BUNDLE" -type f \( -name "*.dylib" -o -name "*.so" -o -perm +111 \) | while read -r binary; do
    codesign --force --options runtime \
      --entitlements "$ENTITLEMENTS" \
      --sign "$IDENTITY" \
      --timestamp \
      "$binary"
  done

  codesign --force --options runtime \
    --entitlements "$ENTITLEMENTS" \
    --sign "$IDENTITY" \
    --timestamp \
    --deep \
    "$APP_BUNDLE"

  codesign --verify --deep --strict "$APP_BUNDLE"
  echo "  App bundle signed successfully"

  # Re-sign the DMG
  step "Signing DMG"
  codesign --force --sign "$IDENTITY" --timestamp "$DMG_FILE"
  codesign --verify "$DMG_FILE"
  echo "  DMG signed successfully"
fi

# ─── Notarization ─────────────────────────────────────────────────────────────

if [[ "$NOTARIZE" == "true" ]]; then
  step "Notarizing DMG"

  [[ -n "${APPLE_ID:-}" ]]           || die "APPLE_ID not set"
  [[ -n "${APPLE_TEAM_ID:-}" ]]      || die "APPLE_TEAM_ID not set"
  [[ -n "${APPLE_APP_PASSWORD:-}" ]] || die "APPLE_APP_PASSWORD not set"

  xcrun notarytool submit "$DMG_FILE" \
    --apple-id "$APPLE_ID" \
    --team-id  "$APPLE_TEAM_ID" \
    --password "$APPLE_APP_PASSWORD" \
    --wait \
    --progress

  xcrun stapler staple "$DMG_FILE"
  xcrun stapler validate "$DMG_FILE"
  echo "  Notarized and stapled"
fi

# ─── Copy to dist/ ────────────────────────────────────────────────────────────

step "Copying to dist/"
mkdir -p "$DIST_DIR"

CHANNEL_SUFFIX=""
[[ "$CHANNEL" != "stable" ]] && CHANNEL_SUFFIX="-$CHANNEL"
ARCH=$(uname -m)
DEST_NAME="arcana_${VERSION}${CHANNEL_SUFFIX}_${ARCH}.dmg"
DEST_PATH="$DIST_DIR/$DEST_NAME"

cp "$DMG_FILE" "$DEST_PATH"

# SHA-256 checksum
shasum -a 256 "$DEST_PATH" | awk '{print $1"  '"$DEST_NAME"'"}' > "${DEST_PATH}.sha256"

echo
echo "Build complete:"
echo "  $DEST_PATH"
echo "  ${DEST_PATH}.sha256"
