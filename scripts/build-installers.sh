#!/usr/bin/env bash
# build-installers.sh — Cross-platform build orchestrator.
# Phase 5.3 — Platform Installers.
#
# Detects the current OS and delegates to the appropriate platform script.
# Intended for use in CI and local development alike.
#
# Usage:
#   ./scripts/build-installers.sh [options]
#
# Options:
#   --channel <stable|beta|nightly>   (default: stable)
#   --sign                             Sign the installer
#   --notarize                         Notarize (macOS only)
#   --upload                           Upload to GitHub Releases
#   --all-linux                        Build AppImage + deb on Linux

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

CHANNEL="stable"
SIGN=false
NOTARIZE=false
UPLOAD=false
ALL_LINUX=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --channel)    CHANNEL="$2"; shift 2 ;;
    --sign)       SIGN=true;    shift ;;
    --notarize)   NOTARIZE=true; shift ;;
    --upload)     UPLOAD=true;   shift ;;
    --all-linux)  ALL_LINUX=true; shift ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

OS="$(uname -s)"

case "$OS" in
  Darwin)
    ARGS=(--channel "$CHANNEL")
    [[ "$SIGN"     == "true" ]] && ARGS+=(--sign)
    [[ "$NOTARIZE" == "true" ]] && ARGS+=(--notarize)
    [[ "$UPLOAD"   == "true" ]] && ARGS+=(--upload)
    bash "$SCRIPT_DIR/build-macos.sh" "${ARGS[@]}"
    ;;
  Linux)
    ARGS=(--channel "$CHANNEL")
    [[ "$ALL_LINUX" == "true" ]] && ARGS+=(--all)
    [[ "$UPLOAD"    == "true" ]] && ARGS+=(--upload)
    bash "$SCRIPT_DIR/build-linux.sh" "${ARGS[@]}"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    SIGN_FLAG=""
    [[ "$SIGN" == "true" ]] && SIGN_FLAG="-Sign"
    UPLOAD_FLAG=""
    [[ "$UPLOAD" == "true" ]] && UPLOAD_FLAG="-Upload"
    powershell -ExecutionPolicy Bypass -File "$SCRIPT_DIR/build-windows.ps1" \
      -Channel "$CHANNEL" $SIGN_FLAG $UPLOAD_FLAG
    ;;
  *)
    echo "ERROR: Unsupported OS: $OS" >&2
    exit 1
    ;;
esac
