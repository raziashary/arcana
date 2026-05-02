#!/usr/bin/env bash
# AIRS AutoDev worker setup script — Linux / macOS
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/AIRS-Foundry/autodev/main/src-cli/setup.sh | bash
# With a fleet token:
#   curl -fsSL .../setup.sh | bash -s -- --token "TOKEN" --type "worker-backend"
#
# The script:
#   1. Installs the airsctl CLI binary
#   2. Redeems the fleet token (if provided)
#   3. Installs the appropriate worker plugin
#   4. Verifies the installation
#
# Supports: Ubuntu/Debian, Fedora/RHEL, macOS (Homebrew)
# Requires: curl, bash >= 4.0

set -euo pipefail

# ─── Colours ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

info()    { echo -e "${CYAN}[INFO]${RESET}  $*"; }
success() { echo -e "${GREEN}[OK]${RESET}    $*"; }
warn()    { echo -e "${YELLOW}[WARN]${RESET}  $*"; }
error()   { echo -e "${RED}[ERROR]${RESET} $*"; exit 1; }

# ─── Argument parsing ─────────────────────────────────────────────────────────
TOKEN=""
WORKER_TYPE="worker-backend"
AIRSCTL_VERSION="${AIRSCTL_VERSION:-latest}"
INSTALL_DIR="${AIRSCTL_INSTALL_DIR:-$HOME/.local/bin}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --token)  TOKEN="$2";       shift 2 ;;
    --type)   WORKER_TYPE="$2"; shift 2 ;;
    --dir)    INSTALL_DIR="$2"; shift 2 ;;
    --version) AIRSCTL_VERSION="$2"; shift 2 ;;
    -h|--help)
      echo "Usage: setup.sh [--token TOKEN] [--type WORKER_TYPE] [--dir INSTALL_DIR]"
      exit 0 ;;
    *) warn "Unknown argument: $1"; shift ;;
  esac
done

# ─── Detect OS ────────────────────────────────────────────────────────────────
detect_os() {
  if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "macos"
  elif [[ -f /etc/debian_version ]]; then
    echo "debian"
  elif [[ -f /etc/fedora-release ]] || [[ -f /etc/redhat-release ]]; then
    echo "rhel"
  else
    echo "linux"
  fi
}

OS=$(detect_os)
ARCH=$(uname -m)
case "$ARCH" in
  x86_64|amd64) ARCH="x86_64" ;;
  aarch64|arm64) ARCH="aarch64" ;;
  *) error "Unsupported architecture: $ARCH" ;;
esac

info "Detected OS: $OS | Arch: $ARCH"

# ─── Install airsctl ──────────────────────────────────────────────────────────
install_airsctl() {
  info "Installing airsctl CLI..."

  mkdir -p "$INSTALL_DIR"

  RELEASE_BASE="https://github.com/AIRS-Foundry/autodev/releases"
  if [[ "$AIRSCTL_VERSION" == "latest" ]]; then
    DOWNLOAD_URL="$RELEASE_BASE/latest/download/airsctl-${OS}-${ARCH}"
  else
    DOWNLOAD_URL="$RELEASE_BASE/download/${AIRSCTL_VERSION}/airsctl-${OS}-${ARCH}"
  fi

  # Try to download; if download fails, fall back to cargo install
  if curl -fsSL --max-time 30 -o "$INSTALL_DIR/airsctl" "$DOWNLOAD_URL" 2>/dev/null; then
    chmod +x "$INSTALL_DIR/airsctl"
    success "airsctl installed to $INSTALL_DIR/airsctl"
  else
    warn "Binary download failed. Attempting to build from source via cargo..."
    if ! command -v cargo &>/dev/null; then
      info "Installing Rust toolchain..."
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
      # shellcheck source=/dev/null
      source "$HOME/.cargo/env"
    fi
    cargo install airsctl --root "$HOME/.local" --locked 2>/dev/null \
      || cargo install --git https://github.com/AIRS-Foundry/autodev --bin airsctl --root "$HOME/.local"
    INSTALL_DIR="$HOME/.local/bin"
    success "airsctl built and installed."
  fi

  # Add to PATH if needed
  if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
    SHELL_RC="$HOME/.bashrc"
    [[ "$SHELL" == *zsh* ]] && SHELL_RC="$HOME/.zshrc"
    echo "export PATH=\"\$PATH:$INSTALL_DIR\"" >> "$SHELL_RC"
    export PATH="$PATH:$INSTALL_DIR"
    info "Added $INSTALL_DIR to PATH (restart shell or run: export PATH=\"\$PATH:$INSTALL_DIR\")"
  fi
}

# ─── Verify airsctl is available ──────────────────────────────────────────────
ensure_airsctl() {
  if ! command -v airsctl &>/dev/null; then
    install_airsctl
  else
    success "airsctl found: $(airsctl --version 2>/dev/null | head -1)"
  fi
}

# ─── Redeem token ─────────────────────────────────────────────────────────────
redeem_token() {
  if [[ -z "$TOKEN" ]]; then
    info "No token provided — installing '$WORKER_TYPE' directly."
    return
  fi

  info "Validating setup token..."
  if airsctl worker-setup validate --token "$TOKEN" 2>/dev/null; then
    success "Token valid."
    info "Redeeming token (installing '$WORKER_TYPE')..."
    airsctl worker-setup redeem --token "$TOKEN"
  else
    warn "Token validation failed — installing plugin directly."
  fi
}

# ─── Install worker plugin ────────────────────────────────────────────────────
install_worker_plugin() {
  info "Installing plugin: $WORKER_TYPE"
  airsctl plugin install "$WORKER_TYPE"
  success "Plugin '$WORKER_TYPE' installed."
}

# ─── Verify installation ──────────────────────────────────────────────────────
verify_installation() {
  info "Verifying installation..."
  if airsctl plugin verify "$WORKER_TYPE"; then
    success "All dependency checks passed for '$WORKER_TYPE'."
  else
    warn "Some checks failed — review output above and install missing dependencies."
  fi
}

# ─── Main ─────────────────────────────────────────────────────────────────────
main() {
  echo ""
  echo -e "${BOLD}${CYAN}AIRS AutoDev Worker Setup${RESET}"
  echo -e "Worker type: ${BOLD}$WORKER_TYPE${RESET}"
  echo ""

  ensure_airsctl

  if [[ -n "$TOKEN" ]]; then
    redeem_token
  else
    install_worker_plugin
  fi

  verify_installation

  echo ""
  success "Setup complete! Run ${BOLD}airsctl plugin list --installed${RESET} to see installed plugins."
  echo ""
}

main "$@"
