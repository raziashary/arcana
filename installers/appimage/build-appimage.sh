#!/usr/bin/env bash
# build-appimage.sh — Build the AutoDev AppImage for Linux.
# Phase 5.3 — Platform Installers.
#
# Usage:
#   ./build-appimage.sh [--version 1.2.3] [--channel stable] [--arch x86_64]
#
# Prerequisites:
#   - Rust + Cargo
#   - Node.js + npm
#   - appimagetool (downloaded automatically if not in PATH)
#   - linuxdeploy (downloaded automatically if not in PATH)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TAURI_DIR="$REPO_ROOT/src-tauri"
DIST_DIR="$REPO_ROOT/dist"
APPDIR_TEMPLATE="$SCRIPT_DIR/AppDir"

# ─── Defaults ─────────────────────────────────────────────────────────────────

VERSION=""
CHANNEL="stable"
ARCH="${ARCH:-$(uname -m)}"
TOOLS_DIR="$HOME/.local/share/autodev-build-tools"

# ─── Argument parsing ─────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) VERSION="$2";  shift 2 ;;
    --channel) CHANNEL="$2";  shift 2 ;;
    --arch)    ARCH="$2";     shift 2 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

# ─── Helpers ──────────────────────────────────────────────────────────────────

step() { echo; echo "==> $*"; }
die()  { echo "ERROR: $*" >&2; exit 1; }

download_tool() {
  local name="$1" url="$2" dest="$3"
  if [[ ! -f "$dest" ]]; then
    echo "  Downloading $name..."
    mkdir -p "$(dirname "$dest")"
    curl -fsSL -o "$dest" "$url"
    chmod +x "$dest"
  fi
}

# ─── Prerequisites ────────────────────────────────────────────────────────────

step "Checking prerequisites"

command -v cargo >/dev/null || die "Rust/Cargo not found. Install: https://rustup.rs"
command -v node  >/dev/null || die "Node.js not found. Install: https://nodejs.org"
command -v npm   >/dev/null || die "npm not found"

# Download appimagetool if not present
if ! command -v appimagetool >/dev/null 2>&1; then
  APPIMAGETOOL="$TOOLS_DIR/appimagetool-${ARCH}.AppImage"
  download_tool "appimagetool" \
    "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-${ARCH}.AppImage" \
    "$APPIMAGETOOL"
  export PATH="$TOOLS_DIR:$PATH"
  ln -sf "$APPIMAGETOOL" "$TOOLS_DIR/appimagetool" 2>/dev/null || true
fi

if [[ -z "$VERSION" ]]; then
  VERSION=$(grep -m1 '^version' "$TAURI_DIR/Cargo.toml" | sed 's/.*= *"\(.*\)"/\1/')
fi

echo "  Version : $VERSION"
echo "  Channel : $CHANNEL"
echo "  Arch    : $ARCH"

# ─── Frontend deps ────────────────────────────────────────────────────────────

step "Installing frontend dependencies"
cd "$REPO_ROOT"
npm ci --prefer-offline

# ─── Build Tauri app ─────────────────────────────────────────────────────────

step "Building Tauri app (release)"
cd "$REPO_ROOT"

export AUTODEV_CHANNEL="$CHANNEL"
npm run tauri -- build --bundles appimage

# ─── Locate the built AppImage ────────────────────────────────────────────────

BUNDLE_DIR="$TAURI_DIR/target/release/bundle/appimage"
BUILT_APPIMAGE=$(find "$BUNDLE_DIR" -name "*.AppImage" | sort | tail -1)
[[ -f "$BUILT_APPIMAGE" ]] || die "No AppImage found in $BUNDLE_DIR"

echo "  Built: $BUILT_APPIMAGE"

# ─── Copy to dist/ ────────────────────────────────────────────────────────────

step "Copying to dist/"
mkdir -p "$DIST_DIR"

CHANNEL_SUFFIX=""
[[ "$CHANNEL" != "stable" ]] && CHANNEL_SUFFIX="-$CHANNEL"
DEST_NAME="autodev_${VERSION}${CHANNEL_SUFFIX}_${ARCH}.AppImage"
DEST_PATH="$DIST_DIR/$DEST_NAME"

cp "$BUILT_APPIMAGE" "$DEST_PATH"
chmod +x "$DEST_PATH"

# SHA-256 checksum
sha256sum "$DEST_PATH" | awk '{print $1"  '"$DEST_NAME"'"}' > "${DEST_PATH}.sha256"

echo
echo "Build complete:"
echo "  $DEST_PATH"
echo "  ${DEST_PATH}.sha256"
