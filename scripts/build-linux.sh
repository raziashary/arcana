#!/usr/bin/env bash
# build-linux.sh — Build AutoDev Linux installers (AppImage + .deb).
# Phase 5.3 — Platform Installers.
#
# Usage:
#   ./scripts/build-linux.sh [--appimage] [--deb] [--upload] [--channel stable]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TAURI_DIR="$REPO_ROOT/src-tauri"
DIST_DIR="$REPO_ROOT/dist"

BUILD_APPIMAGE=false
BUILD_DEB=false
UPLOAD=false
CHANNEL="stable"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --appimage) BUILD_APPIMAGE=true; shift ;;
    --deb)      BUILD_DEB=true;      shift ;;
    --all)      BUILD_APPIMAGE=true; BUILD_DEB=true; shift ;;
    --upload)   UPLOAD=true;         shift ;;
    --channel)  CHANNEL="$2";        shift 2 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

# Default: build both
if [[ "$BUILD_APPIMAGE" == "false" && "$BUILD_DEB" == "false" ]]; then
  BUILD_APPIMAGE=true
  BUILD_DEB=true
fi

echo "AutoDev Linux Build"
echo "==================="

VERSION=$(grep -m1 '^version' "$TAURI_DIR/Cargo.toml" | sed 's/.*= *"\(.*\)"/\1/')
ARCH=$(uname -m)
echo "  Version : $VERSION"
echo "  Arch    : $ARCH"
echo "  Channel : $CHANNEL"

mkdir -p "$DIST_DIR"

# Install frontend deps
echo
echo "==> Installing frontend dependencies"
cd "$REPO_ROOT"
npm ci --prefer-offline

# Build Tauri with all Linux targets
echo
echo "==> Building Tauri (release)"
export AUTODEV_CHANNEL="$CHANNEL"

BUNDLES=""
[[ "$BUILD_APPIMAGE" == "true" ]] && BUNDLES="${BUNDLES:+$BUNDLES,}appimage"
[[ "$BUILD_DEB"      == "true" ]] && BUNDLES="${BUNDLES:+$BUNDLES,}deb"

npm run tauri -- build --bundles "$BUNDLES"

CHANNEL_SUFFIX=""
[[ "$CHANNEL" != "stable" ]] && CHANNEL_SUFFIX="-$CHANNEL"

# ── AppImage ──────────────────────────────────────────────────────────────────
if [[ "$BUILD_APPIMAGE" == "true" ]]; then
  echo
  echo "==> Packaging AppImage"
  APPIMAGE=$(find "$TAURI_DIR/target/release/bundle/appimage" -name "*.AppImage" | sort | tail -1)
  if [[ -f "$APPIMAGE" ]]; then
    DEST="$DIST_DIR/autodev_${VERSION}${CHANNEL_SUFFIX}_${ARCH}.AppImage"
    cp "$APPIMAGE" "$DEST"
    chmod +x "$DEST"
    sha256sum "$DEST" | awk '{print $1"  autodev_'"${VERSION}${CHANNEL_SUFFIX}_${ARCH}"'.AppImage"}' > "${DEST}.sha256"
    echo "  $DEST"
  else
    echo "  WARNING: AppImage not found — skipping"
  fi
fi

# ── .deb ──────────────────────────────────────────────────────────────────────
if [[ "$BUILD_DEB" == "true" ]]; then
  echo
  echo "==> Packaging .deb"
  DEB=$(find "$TAURI_DIR/target/release/bundle/deb" -name "*.deb" | sort | tail -1)
  if [[ -f "$DEB" ]]; then
    DEST="$DIST_DIR/autodev_${VERSION}${CHANNEL_SUFFIX}_${ARCH}.deb"
    cp "$DEB" "$DEST"
    sha256sum "$DEST" | awk '{print $1"  autodev_'"${VERSION}${CHANNEL_SUFFIX}_${ARCH}"'.deb"}' > "${DEST}.sha256"
    echo "  $DEST"
  else
    echo "  WARNING: .deb not found — skipping"
  fi
fi

# ── Upload ────────────────────────────────────────────────────────────────────
if [[ "$UPLOAD" == "true" ]]; then
  TAG="${RELEASE_TAG:-}"
  [[ -n "$TAG" ]] || { echo "ERROR: RELEASE_TAG not set" >&2; exit 1; }

  echo
  echo "==> Uploading to GitHub Releases ($TAG)"
  for f in "$DIST_DIR/"*.AppImage "$DIST_DIR/"*.deb "$DIST_DIR/"*.sha256; do
    [[ -f "$f" ]] || continue
    echo "  Uploading $(basename "$f")..."
    gh release upload "$TAG" "$f" --clobber
  done
  echo "  Upload complete"
fi

echo
echo "Build complete. Artifacts in: $DIST_DIR"
