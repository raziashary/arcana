# AutoDev IT Operations Runbook

**Version:** Phase 5.3 — Platform Installers & Auto-Updater  
**Audience:** IT administrators, DevOps, enterprise deployment teams  
**Last updated:** 2026-05

---

## Table of Contents

1. [Overview](#overview)
2. [**Developer Guide — Building & Releasing Installers**](#developer-guide--building--releasing-installers)
   - [One-time setup](#one-time-setup-do-this-once-ever)
   - [Building a local installer for testing](#building-a-local-installer-for-testing)
   - [Releasing a new version to GitHub](#releasing-a-new-version-to-github)
   - [What happens automatically after you push a tag](#what-happens-automatically-after-you-push-a-tag)
   - [Publishing the release](#publishing-the-release)
   - [How users get the update](#how-users-get-the-update-auto-updater)
   - [Releasing a hotfix](#releasing-a-hotfix-patch-update)
   - [Quick reference — most common commands](#quick-reference--most-common-commands)
3. [System Requirements](#system-requirements)
4. [Installation](#installation)
   - [Windows](#windows-installation)
   - [macOS](#macos-installation)
   - [Linux](#linux-installation)
5. [Silent / Unattended Installation](#silent--unattended-installation)
6. [Network Requirements](#network-requirements)
7. [User Data & Storage](#user-data--storage)
8. [Auto-Updater](#auto-updater)
9. [Proxy Configuration](#proxy-configuration)
10. [Uninstallation](#uninstallation)
11. [Troubleshooting](#troubleshooting)
12. [Security Considerations](#security-considerations)
13. [Enterprise Deployment (Group Policy / MDM)](#enterprise-deployment)

---

## Developer Guide — Building & Releasing Installers

This section is for **you** — the developer who wants to build an installer, push a release to GitHub, and have the auto-updater deliver it to users. Start here.

---

### One-time setup (do this once, ever)

These steps only need to happen once on your machine and once on GitHub. After that, releasing is just "bump version → push a tag."

#### Step 1 — Install prerequisites on your Windows machine

Open PowerShell as Administrator and run:

```powershell
# 1. Install Rust (if not already installed)
winget install Rustlang.Rustup
# Then restart PowerShell, and run:
rustup default stable
rustup target add x86_64-pc-windows-msvc

# 2. Install Node.js LTS (if not already installed)
winget install OpenJS.NodeJS.LTS

# 3. Install WiX v4 (for MSI building) — requires .NET SDK
winget install Microsoft.DotNet.SDK.8
dotnet tool install --global wix

# 4. Install GitHub CLI (for uploading releases)
winget install GitHub.cli
gh auth login    # follow the prompts, choose HTTPS, log in with browser

# 5. Verify everything works
cargo --version   # should show cargo 1.75+
node --version    # should show v20+
wix --version     # should show 4.x
gh --version      # should show gh 2.x
```

#### Step 2 — Clone the repo (if you haven't already)

```powershell
git clone https://github.com/AIRS-Foundry/autodev.git "C:\AIRS Foundry\autodev\autodev"
cd "C:\AIRS Foundry\autodev\autodev"
npm install
```

#### Step 3 — Set up GitHub repository secrets (do once, in your browser)

These secrets let GitHub Actions sign and upload your releases automatically.

1. Go to your repo on GitHub: `https://github.com/AIRS-Foundry/autodev`
2. Click **Settings** → **Secrets and variables** → **Actions**
3. Click **New repository secret** and add each of these:

| Secret name | What to put in it | Why it's needed |
|-------------|-------------------|-----------------|
| `AUTODEV_UPDATE_PUBKEY` | Your Ed25519 public key (base64) — see Step 4 below | Embeds in the app so it can verify update signatures |
| `WINDOWS_CERT_PATH` | Path to your `.p12` code-signing cert on the runner | Signs the Windows MSI |
| `WINDOWS_CERT_PASSWORD` | Password for the `.p12` cert | Unlocks the cert for signing |
| `APPLE_CERT_BASE64` | Your Apple Developer certificate, base64-encoded | Signs the macOS app |
| `APPLE_CERT_PASSWORD` | Password for the Apple cert | Unlocks the cert |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: AIRS Foundry Ltd (ABCDE12345)` | Tells codesign which cert to use |
| `APPLE_ID` | Your Apple ID email | For notarization |
| `APPLE_TEAM_ID` | Your 10-character Apple Team ID | For notarization |
| `APPLE_APP_PASSWORD` | App-specific password from appleid.apple.com | For notarization (not your real password) |
| `KEYCHAIN_PASSWORD` | Any random secure string | Protects the temp keychain in the runner |

> **Don't have signing certificates yet?** That's fine — skip the signing secrets for now. Installers will still build, they just won't be signed. Skip Steps 3–4 and come back when you're ready to distribute publicly.

#### Step 4 — Generate your Ed25519 update signing key (do once)

This key is used to sign every update installer so the app can verify it's really from you.

```powershell
# Install the key generation tool
cargo install ed25519-keygen --quiet

# Generate the keypair — saves two files in the current directory
ed25519-keygen

# This creates:
#   autodev.key      ← PRIVATE KEY — keep this secret, never commit it
#   autodev.key.pub  ← PUBLIC KEY — paste this into the GitHub secret above
```

```powershell
# View the public key (paste this into the AUTODEV_UPDATE_PUBKEY secret)
Get-Content autodev.key.pub

# Store the private key somewhere safe (password manager, encrypted vault)
# You'll need it to sign every release — if you lose it, users can't update
```

> **Important:** Store `autodev.key` in your password manager (e.g. 1Password, Bitwarden). Put `autodev.key.pub` content into the `AUTODEV_UPDATE_PUBKEY` GitHub secret. Never commit either file.

#### Step 5 — Push your code to GitHub (first time)

```powershell
cd "C:\AIRS Foundry\autodev\autodev"

# If you haven't set the remote yet:
git remote add origin https://github.com/AIRS-Foundry/autodev.git

# Push main branch
git push -u origin main
```

You're now set up. Everything below is what you do for every release going forward.

---

### Building a local installer for testing

Before releasing publicly, build and test the installer on your own machine.

#### Build a Windows MSI (locally, no signing)

```powershell
cd "C:\AIRS Foundry\autodev\autodev"

# This builds the full MSI installer
powershell -ExecutionPolicy Bypass -File installers\wix\build-msi.ps1

# The finished .msi file appears in:
#   dist\autodev_1.0.0_x64_en-US.msi
```

The script will:
1. Run `npm ci` (install frontend dependencies)
2. Run `npm run tauri -- build` (compile Rust + bundle frontend)
3. Tauri's bundler calls WiX to create the `.msi`
4. Copy the `.msi` to `dist/` and generate a `.sha256` checksum file

To test install it:
```powershell
# Double-click the .msi, or run silently:
msiexec /i dist\autodev_1.0.0_x64_en-US.msi /qn

# To uninstall:
msiexec /x dist\autodev_1.0.0_x64_en-US.msi /qn
```

#### Build a debug version (faster, for development)

```powershell
# No bundling — just compiles and runs the app
npm run tauri dev

# Or build a debug binary without packaging:
npm run tauri -- build --debug --no-bundle
# Output: src-tauri\target\debug\autodev.exe
```

---

### Releasing a new version to GitHub

This is the main workflow. It's simpler than it looks — GitHub Actions does all the hard work.

#### Step 1 — Update the version number

You need to bump the version in **two places**:

```powershell
# 1. Open src-tauri/Cargo.toml and change the version:
#    version = "1.0.0"   →   version = "1.1.0"
notepad src-tauri\Cargo.toml

# 2. Open src-tauri/tauri.conf.json and change the version:
#    "version": "1.0.0"   →   "version": "1.1.0"
notepad src-tauri\tauri.conf.json
```

> **Version numbering convention:**  
> `MAJOR.MINOR.PATCH` — e.g. `1.0.0`  
> - Bump PATCH (1.0.**1**) for bug fixes  
> - Bump MINOR (1.**1**.0) for new features  
> - Bump MAJOR (**2**.0.0) for breaking changes  
> - Add `-beta.1` suffix for beta releases: `1.1.0-beta.1`  
> - Add `-nightly` suffix for nightly builds: `1.1.0-nightly`

#### Step 2 — Commit the version bump

```powershell
cd "C:\AIRS Foundry\autodev\autodev"

git add src-tauri/Cargo.toml src-tauri/tauri.conf.json
git commit -m "chore: bump version to 1.1.0"
```

#### Step 3 — Push your changes to GitHub

```powershell
git push origin main
```

Wait for the CI checks to pass (shown on GitHub under Actions). If anything fails, fix it before tagging.

#### Step 4 — Create and push a version tag

This is the trigger that starts the automated release build.

```powershell
# Create the tag (replace 1.1.0 with your actual version)
git tag v1.1.0

# Push the tag to GitHub — this triggers the build pipeline
git push origin v1.1.0
```

That's it. GitHub Actions takes over from here.

---

### What happens automatically after you push a tag

Once you push `v1.1.0`, GitHub Actions runs the `build-release.yml` workflow. Here's exactly what it does — you can watch it live at `https://github.com/AIRS-Foundry/autodev/actions`:

```
Tag pushed: v1.1.0
        │
        ▼
[Job 1] metadata
   Reads the tag, determines version="1.1.0", channel="stable"
        │
        ▼
[Job 2] create-release
   Creates a DRAFT GitHub Release titled "AutoDev 1.1.0"
   (Draft = not visible to public yet)
        │
        ├──────────────────────────────────────────┐
        ▼                    ▼                     ▼
[Job 3] build-windows   [Job 4] build-macos   [Job 5] build-linux
   runs on Windows           runs on macOS        runs on Ubuntu
   ● installs Rust           ● installs Rust       ● installs apt deps
   ● installs Node           ● installs Node       ● installs Rust
   ● installs WiX v4         ● imports cert        ● installs Node
   ● builds MSI              ● builds DMG          ● builds AppImage
   ● signs MSI (if cert)     ● signs + notarizes   ● builds .deb
   ● uploads .msi +          ● uploads .dmg +      ● uploads files
     .sha256 to release        .sha256 to release    to release
        │                    │                     │
        └──────────────────────────────────────────┘
                             │
                             ▼
              [All three builds complete]
              Release now has 6 files attached:
                ● autodev_1.1.0_x64_en-US.msi
                ● autodev_1.1.0_x64_en-US.msi.sha256
                ● autodev_1.1.0_aarch64.dmg
                ● autodev_1.1.0_aarch64.dmg.sha256
                ● autodev_1.1.0_x86_64.AppImage
                ● autodev_1.1.0_x86_64.AppImage.sha256
                ● autodev_1.1.0_x86_64.deb
                ● autodev_1.1.0_x86_64.deb.sha256
```

**How long does it take?**  
About 15–25 minutes total. Windows is usually the slowest (Rust compile + WiX). All three platforms build simultaneously.

**Where to watch it:**  
Go to `https://github.com/AIRS-Foundry/autodev/actions` → click the workflow run named `v1.1.0`. You'll see live logs for each job.

**If a job fails:**  
Click on the failed job to see the error. Common causes:
- Missing secret (check Settings → Secrets)
- Rust compile error (run `cargo check` locally first)
- npm error (run `npm ci` locally first)

---

### Publishing the release

The release is created as a **draft** so you can review it before users see it. The CI workflow automatically publishes it if you pushed a non-beta tag, but you can also do it manually.

#### Review the draft release

1. Go to `https://github.com/AIRS-Foundry/autodev/releases`
2. You'll see "AutoDev 1.1.0 (Draft)" at the top
3. Click it to see the release notes and verify all 8 files are attached
4. Edit the release notes if needed (GitHub auto-generates them from commit messages)

#### Publish it (make it public)

**Option A — via browser:**  
Click the draft release → click **Edit** → uncheck **Set as a draft** → click **Publish release**

**Option B — via command line:**
```powershell
gh release edit v1.1.0 --draft=false
```

Once published, the release appears at `https://github.com/AIRS-Foundry/autodev/releases/latest` — this is the URL the auto-updater checks.

---

### How users get the update (auto-updater)

Once the release is published on GitHub, here's what happens for existing users:

1. **Automatic check:** Every 4 hours, every running copy of AutoDev silently checks the GitHub Releases API
2. **Toast notification:** A blue toast appears in the bottom-right corner: *"Update available — AutoDev 1.1.0"*
3. **User clicks "Update":** A dialog opens showing release notes and a Download button
4. **Download:** The correct installer for their platform downloads to `~/.autodev/updates/`
5. **Verification:** AutoDev checks the SHA-256 checksum and Ed25519 signature
6. **Install scheduled:** The installer is registered to run when the app restarts (Windows RunOnce, macOS LaunchAgent)
7. **Restart:** User clicks "Install & Restart" — app closes, installer runs silently, app reopens at 1.1.0
8. **Data intact:** All sessions, API keys, workers, plans, and settings are preserved

Users can also check manually: **Settings → Updates → Check for updates**

> **Critical updates:** If you set `"critical": true` in the GitHub Release (in the release body as JSON metadata), the update dialog opens automatically and the user cannot dismiss it without acting on it.

---

### Releasing a hotfix (patch update)

Same flow as above, just with a PATCH version bump:

```powershell
# 1. Make your fix
git add .
git commit -m "fix: crash when opening empty workspace"

# 2. Bump version (1.0.0 → 1.0.1)
# Edit Cargo.toml and tauri.conf.json

git add src-tauri/Cargo.toml src-tauri/tauri.conf.json
git commit -m "chore: bump to 1.0.1"

# 3. Push + tag
git push origin main
git tag v1.0.1
git push origin v1.0.1

# 4. Watch Actions build it, then publish the release
gh release edit v1.0.1 --draft=false
```

---

### Quick reference — most common commands

```powershell
# ── Daily development ──────────────────────────────────────────────────────
npm run tauri dev                    # Run in dev mode (hot-reload)
cargo check --manifest-path src-tauri/Cargo.toml   # Check Rust compiles

# ── Build a local installer for testing ───────────────────────────────────
powershell -ExecutionPolicy Bypass -File installers\wix\build-msi.ps1
# Output: dist\autodev_<version>_x64_en-US.msi

# ── Release a new version ─────────────────────────────────────────────────
# 1. Bump version in Cargo.toml + tauri.conf.json
git add src-tauri/Cargo.toml src-tauri/tauri.conf.json
git commit -m "chore: bump to X.Y.Z"
git push origin main

# 2. Tag and push (triggers CI build)
git tag vX.Y.Z
git push origin vX.Y.Z

# 3. Watch the build
gh run watch                         # Stream live CI logs in terminal

# 4. Publish when ready
gh release edit vX.Y.Z --draft=false

# ── Check release status ──────────────────────────────────────────────────
gh release list                      # All releases
gh release view v1.1.0               # Details of a specific release

# ── If something goes wrong ───────────────────────────────────────────────
gh release delete v1.1.0 --yes       # Delete a bad release
git tag -d v1.1.0                    # Delete local tag
git push origin :refs/tags/v1.1.0   # Delete remote tag (then re-tag after fix)

# ── Beta / nightly releases ───────────────────────────────────────────────
git tag v1.1.0-beta.1
git push origin v1.1.0-beta.1
# CI detects "-beta" in the tag → marks release as pre-release automatically
```

---

---

## Overview

AutoDev is a local-first AI development assistant that runs as a native desktop application (built with Tauri/Rust). It does **not** require a central server — all AI inference is routed directly from the end-user machine to the configured LLM provider API.

**Key architectural properties:**
- Single self-contained executable (no system runtimes required beyond WebKit)
- All user data stored in `~/.autodev/` — outside the application bundle
- Optional Python sidecars (memory-mcp-server, core-orchestrator) managed by the app itself
- LLM API keys stored in the **OS-native credential store** (Windows Credential Manager / macOS Keychain / libsecret on Linux)

---

## System Requirements

| Platform | Minimum | Recommended |
|----------|---------|-------------|
| **Windows** | Windows 10 1809 (build 17763) 64-bit | Windows 11 23H2 |
| **macOS** | macOS 11.0 Big Sur | macOS 14 Sonoma |
| **Linux** | Ubuntu 22.04 / Fedora 38 | Ubuntu 24.04 |
| **CPU** | x86_64, 2 cores | x86_64 or ARM64, 4+ cores |
| **RAM** | 4 GB | 8 GB |
| **Disk** | 500 MB (app only) | 2 GB (with Python sidecars) |
| **Network** | Required for LLM API calls | — |

### Software dependencies (Linux only)

```
libwebkit2gtk-4.1-0 >= 2.40.0
libgtk-3-0 >= 3.24.0
libayatana-appindicator3-1   (optional, for system tray)
libssl3 or libssl1.1
```

These are installed automatically by the `.deb` package. For AppImage installations, they must be present on the host system.

---

## Installation

### Windows Installation

**MSI installer (recommended):**
```powershell
# Interactive installation
autodev_<version>_x64_en-US.msi

# Silent installation
msiexec /i autodev_<version>_x64_en-US.msi /qn /norestart

# Silent installation with custom install directory
msiexec /i autodev_<version>_x64_en-US.msi /qn /norestart INSTALLDIR="C:\Tools\AutoDev"

# Install and log to file
msiexec /i autodev_<version>_x64_en-US.msi /qn /norestart /l*v autodev_install.log
```

**Installation path:** `%ProgramFiles%\AutoDev\` (default)  
**Shortcuts created:** Start Menu → AutoDev, Desktop (optional — prompted during install)  
**Registry keys:**
- `HKLM\SOFTWARE\AutoDev` — install metadata
- `HKLM\SOFTWARE\Classes\.autodev` — file association
- `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce\AutoDevUpdate` — created at update time, removed after update applies

### macOS Installation

1. Download `autodev_<version>_aarch64.dmg` (Apple Silicon) or `autodev_<version>_x86_64.dmg` (Intel)
2. Open the DMG
3. Drag **AutoDev.app** to **/Applications/**
4. On first launch, macOS Gatekeeper will verify the notarization signature

**Manual trust (if Gatekeeper blocks):**
```bash
sudo xattr -rd com.apple.quarantine /Applications/AutoDev.app
```

**MDM deployment (Jamf/Kandji):**  
The DMG is notarized and stapled — no additional trust configuration needed on managed machines.

### Linux Installation

**AppImage (portable — no root required):**
```bash
chmod +x autodev_<version>_x86_64.AppImage
./autodev_<version>_x86_64.AppImage

# Install to system PATH
sudo mv autodev_<version>_x86_64.AppImage /usr/local/bin/autodev
```

**Debian/Ubuntu (.deb):**
```bash
sudo apt install ./autodev_<version>_amd64.deb
```

**RPM (Fedora/RHEL):**
```bash
sudo rpm -ivh autodev_<version>_x86_64.rpm
# or
sudo dnf install ./autodev_<version>_x86_64.rpm
```

---

## Silent / Unattended Installation

### Windows (SCCM / Intune)

**Detection rule:** File exists `%ProgramFiles%\AutoDev\AutoDev.exe`

**Install command:**
```
msiexec /i autodev_<version>_x64_en-US.msi /qn /norestart ALLUSERS=1
```

**Uninstall command:**
```
msiexec /x {PRODUCT-GUID} /qn /norestart
```
*(Product GUID is in the MSI property table, readable via `msiinfo` or Orca)*

**Return codes:** Standard MSI codes — 0 = success, 3010 = success/reboot pending

### macOS (Jamf)

Use a **DMG policy** with the AutoDev DMG file. Jamf will handle mounting, copying the `.app` to `/Applications/`, and unmounting.

Policy type: `macOS Packages` → Select DMG

### Linux (Ansible / Chef)

```yaml
# Ansible task
- name: Install AutoDev
  ansible.builtin.apt:
    deb: /tmp/autodev_{{ autodev_version }}_amd64.deb
    state: present
```

---

## Network Requirements

AutoDev makes outbound HTTPS connections to:

| Destination | Port | Purpose |
|-------------|------|---------|
| `api.anthropic.com` | 443 | Anthropic Claude API |
| `api.openai.com` | 443 | OpenAI API |
| `generativelanguage.googleapis.com` | 443 | Google Gemini API |
| `api.github.com` | 443 | GitHub MCP integration, update checks |
| `releases.airs.dev` | 443 | AutoDev update manifest + binaries |
| `127.0.0.1:8767` | — | Internal MCP server (localhost only) |
| `127.0.0.1:8000` | — | memory-mcp-server (localhost only) |
| `127.0.0.1:4000` | — | core-orchestrator (localhost only) |

**Minimum required for basic functionality:**
- One of the LLM provider endpoints above

**Firewall rule (Windows — deployed automatically by MSI):**
```
Name: AutoDev MCP Server
Direction: Inbound
Protocol: TCP
LocalPort: 8767
Scope: LocalSubnet
Action: Allow
```

---

## User Data & Storage

All user data is stored **outside** the application bundle and is **never modified by the installer or uninstaller**:

```
~/.autodev/
├── config.db              # SQLite: providers, sessions, workers, recipes
├── analytics.db           # SQLite: cost tracking, token usage
├── sessions/              # Conversation history (JSON)
├── plans/                 # Agent plan trees
├── skills/                # Custom skill definitions (YAML)
├── hooks/                 # Event hook scripts
├── updates/               # Downloaded update installers (cleaned automatically)
└── restore_point.json     # Created pre-update, removed post-update

~/.airs/
├── plugins/               # Installed plugins
└── fleet_secret           # Worker fleet authentication token
```

**Backup recommendation:** Back up `~/.autodev/config.db` and `~/.autodev/sessions/` before major updates.

**Data persistence across updates:** ✅ All user data is preserved across application updates. The AutoDev updater performs a pre-update integrity check and creates a restore point before applying any update.

---

## Auto-Updater

AutoDev includes a built-in auto-updater that checks for new versions every 4 hours.

### How it works

1. **Check:** Background task queries `https://api.github.com/repos/AIRS-Foundry/autodev/releases/latest`
2. **Notify:** If a newer version is found, a toast notification appears in the app
3. **Download:** User clicks "Update" → installer downloaded to `~/.autodev/updates/`
4. **Verify:** SHA-256 checksum + Ed25519 signature verified before install
5. **Install:** OS-specific mechanism (Windows: RunOnce batch; macOS: LaunchAgent; Linux: AppImage copy)
6. **Restart:** App restarts to apply the update

### Disabling auto-updater (enterprise)

To prevent AutoDev from checking for updates, set the environment variable:
```
AUTODEV_DISABLE_UPDATER=1
```

Or add to `~/.autodev/config.json`:
```json
{ "updater": { "enabled": false } }
```

### Update channels

| Channel | Description |
|---------|-------------|
| `stable` | Production releases (default) |
| `beta` | Feature previews, may have minor issues |
| `nightly` | Daily builds from `develop` branch |

Users can switch channels in **Settings → Updates → Update channel**.

### Deploying a specific version (lockdown)

To lock all users to a specific version and prevent updates:
```
AUTODEV_DISABLE_UPDATER=1
```

Deploy via Group Policy (Windows) or `/etc/environment` (Linux) or `launchctl setenv` (macOS).

---

## Proxy Configuration

AutoDev uses the system proxy settings on all platforms. To configure a custom proxy:

**Windows:** Set via Internet Explorer / System Settings → Proxy  
**macOS:** System Settings → Network → Proxies  
**Linux:**
```bash
export HTTPS_PROXY=http://proxy.company.com:8080
export HTTP_PROXY=http://proxy.company.com:8080
export NO_PROXY=localhost,127.0.0.1,::1
```

For per-user proxy without sudo, add to `~/.autodev/config.json`:
```json
{
  "proxy": {
    "https": "http://proxy.company.com:8080",
    "no_proxy": "localhost,127.0.0.1"
  }
}
```

**SSL inspection / MITM proxy:**  
If your corporate proxy performs SSL inspection, add the CA certificate to the system trust store. AutoDev uses the OS certificate store on all platforms.

---

## Uninstallation

### Windows
```
Control Panel → Programs → Uninstall a program → AutoDev → Uninstall
```
Or silently:
```
msiexec /x {PRODUCT-GUID} /qn /norestart
```

⚠️ **User data in `%USERPROFILE%\.autodev\` is NOT removed.** Delete manually if required.

### macOS
```bash
rm -rf /Applications/AutoDev.app
# Remove user data (optional):
rm -rf ~/.autodev ~/.airs
```

### Linux (.deb)
```bash
sudo apt remove autodev
# With user data purge:
sudo apt purge autodev && rm -rf ~/.autodev ~/.airs
```

### Linux (AppImage)
```bash
rm /usr/local/bin/autodev   # or wherever you placed it
# Remove user data (optional):
rm -rf ~/.autodev ~/.airs
```

---

## Troubleshooting

### App won't launch (Windows)

1. Check Event Viewer → Application for AutoDev errors
2. Run from PowerShell to see stderr: `& "C:\Program Files\AutoDev\AutoDev.exe"`
3. Check WebView2 runtime: `Get-Package -Name "Microsoft Edge WebView2 Runtime"`
4. If WebView2 is missing: install from https://developer.microsoft.com/en-us/microsoft-edge/webview2/

### App won't launch (Linux)

```bash
# Check library dependencies
ldd /usr/bin/autodev | grep "not found"

# Run with DMA-BUF renderer disabled (some VMs/Wayland compositors)
WEBKIT_DISABLE_DMABUF_RENDERER=1 autodev

# AppImage sandbox issues
AUTODEV_NO_SANDBOX=1 ./autodev.AppImage
```

### App crashes on startup

Check logs:
```
# Windows
%APPDATA%\com.airs-foundry.autodev\logs\

# macOS
~/Library/Logs/com.airs-foundry.autodev/

# Linux
~/.local/share/com.airs-foundry.autodev/logs/
```

Or use the built-in log viewer: **Settings → Logs**

### Sidecars not starting

The Python sidecars (memory-mcp-server, core-orchestrator) require Python 3.11+.

```bash
python3 --version   # Must be 3.11+

# Check AUTODEV_SIDECAR_PATH
echo $AUTODEV_SIDECAR_PATH

# Run in Demo Mode (no sidecars)
AUTODEV_SKIP_SIDECARS=1 autodev
```

Or enable Demo Mode in **Settings → Demo Mode**.

### Update fails to apply

1. Check `~/.autodev/updates/` for the downloaded installer
2. Verify checksum manually:
   - Windows: `Get-FileHash <file> -Algorithm SHA256`
   - macOS/Linux: `sha256sum <file>`
3. Compare against the `.sha256` sidecar file from the release
4. If checksum matches but signature fails, contact IT (may indicate MITM or tampered download)
5. Manually download and install from https://airs.dev/autodev/releases

### API connection issues

1. Test API key: **Settings → Providers → [Provider] → Test**
2. Verify firewall allows outbound HTTPS to the provider endpoint
3. Check proxy configuration (see [Proxy Configuration](#proxy-configuration))
4. Check **Settings → Logs** for detailed error messages

---

## Security Considerations

### API Key Storage

API keys are stored in the OS-native credential store:
- **Windows:** Windows Credential Manager (`wincred`)
- **macOS:** Keychain
- **Linux:** `libsecret` / GNOME Keyring (falls back to encrypted file)

Keys are **never** stored in `~/.autodev/config.db` in plaintext.

### Update Signature Verification

All AutoDev updates are verified with:
1. **SHA-256 checksum** — verifies file integrity
2. **Ed25519 signature** — verifies the update was signed by AIRS Foundry

The Ed25519 public key is embedded in the application binary at compile time. Update verification fails if either check fails.

### Data Residency

- All conversation data is stored **locally** in `~/.autodev/`
- Conversations are sent to the configured LLM provider API — review that provider's data residency policy
- AutoDev does **not** send telemetry or usage data to AIRS Foundry

### Least Privilege

AutoDev does not require elevated privileges at runtime. The only time elevated privileges are needed:
- **Windows:** MSI installation (one-time, as Administrator)
- **Linux deb/rpm:** Package installation (one-time, with `sudo`)
- **Linux deb auto-update:** `pkexec apt-get install` for .deb format updates (polkit prompt)

---

## Enterprise Deployment

### Group Policy (Windows)

Create an ADMX template or use registry-based policies:

```reg
[HKEY_LOCAL_MACHINE\SOFTWARE\Policies\AIRS-Foundry\AutoDev]
"DisableUpdater"=dword:00000001
"DefaultChannel"="stable"
"AllowedProviders"="anthropic,openai"
```

### Mobile Device Management (macOS — Jamf/Kandji)

**Configuration profile payload:**
```xml
<dict>
  <key>DisableUpdater</key>
  <true/>
  <key>DefaultChannel</key>
  <string>stable</string>
</dict>
```

Payload domain: `com.airs-foundry.autodev`

### Pre-configuring API Keys (enterprise deployment)

For managed machines, pre-configure API keys using the CLI tool (`airsctl`):
```bash
airsctl provider set-key anthropic "sk-ant-..." --system
airsctl provider set-active anthropic claude-opus-4-5
```

Or via environment variable (picked up on first launch):
```
AUTODEV_ANTHROPIC_KEY=sk-ant-...
```

### Deploying Skills & Recipes enterprise-wide

Place custom skill YAML files in a shared network location and configure:
```json
// ~/.autodev/config.json
{
  "skills": {
    "extra_paths": ["/mnt/shared/autodev-skills/"]
  }
}
```

Workers will load skills from both the standard path (`~/.autodev/skills/`) and any configured extra paths.

---

## Support

- Documentation: https://docs.airs.dev/autodev
- Enterprise support: enterprise@airs.dev
- Issue tracker: https://github.com/AIRS-Foundry/autodev/issues
- Status page: https://status.airs.dev
