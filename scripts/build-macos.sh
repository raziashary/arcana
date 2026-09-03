#!/usr/bin/env bash
# build-macos.sh — Build AutoDev macOS DMG.
# Phase 5.3 — Platform Installers.
#
# Usage:
#   ./scripts/build-macos.sh [--sign] [--notarize] [--upload] [--channel stable]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SIGN=""
NOTARIZE=""
UPLOAD=false
CHANNEL="stable"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --sign)      SIGN="--sign";           shift ;;
    --notarize)  NOTARIZE="--notarize";   shift ;;
    --upload)    UPLOAD=true;             shift ;;
    --channel)   CHANNEL="$2";            shift 2 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

echo "AutoDev macOS Build"
echo "==================="

bash "$REPO_ROOT/installers/dmg/create-dmg.sh" \
  $SIGN $NOTARIZE \
  --channel "$CHANNEL"

if [[ "$UPLOAD" == "true" ]]; then
  TAG="${RELEASE_TAG:-}"
  TOKEN="${GITHUB_TOKEN:-}"

  [[ -n "$TAG" ]]   || { echo "ERROR: RELEASE_TAG not set" >&2; exit 1; }
  [[ -n "$TOKEN" ]] || { echo "ERROR: GITHUB_TOKEN not set" >&2; exit 1; }

  echo "Uploading to GitHub Releases ($TAG)..."
  for f in "$REPO_ROOT/dist/"*.dmg "$REPO_ROOT/dist/"*.sha256; do
    [[ -f "$f" ]] || continue
    echo "  Uploading $(basename "$f")..."
    gh release upload "$TAG" "$f" --clobber
  done
  echo "Upload complete"
fi
