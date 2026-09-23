#Requires -Version 5.1
<#
.SYNOPSIS
    Build the Arcana Windows MSI installer using Tauri + WiX v4.

.DESCRIPTION
    1. Verifies prerequisites (Rust, Node, WiX toolset)
    2. Builds the Tauri application in release mode
    3. Signs the installer with the code-signing certificate if CERT_PATH is set
    4. Outputs arcana_<version>[_channel]_x64_en-US.exe to dist/, plus a
       matching .sha256 sidecar.

    NOTE: despite this script's name, the artifact it collects is the NSIS
    installer (.exe) from the Tauri bundler, not an .msi. The description
    previously claimed .msi, which is what the updater's asset matcher was
    written against — so it matched nothing and Windows was never offered an
    update. See docs/restructurization/development/_verify/P0-CORRECTIONS.md
    (P0-29).

.PARAMETER Version
    Override version string (default: read from Cargo.toml)

.PARAMETER Sign
    Sign the installer. Requires CERT_PATH and CERT_PASSWORD env vars.

.PARAMETER Channel
    Release channel: stable (default), beta, nightly

.EXAMPLE
    .\build-msi.ps1 -Sign
    .\build-msi.ps1 -Version 1.2.3 -Channel beta
#>

[CmdletBinding()]
param(
    [string]$Version = "",
    [switch]$Sign,
    [ValidateSet("stable","beta","nightly")]
    [string]$Channel = "stable"
)

$ErrorActionPreference = "Stop"

# ─── Paths ────────────────────────────────────────────────────────────────────

$RepoRoot    = Split-Path (Split-Path $PSScriptRoot -Parent)
$TauriDir    = Join-Path $RepoRoot "src-tauri"
$CargoToml   = Join-Path $TauriDir "Cargo.toml"
$DistDir     = Join-Path $RepoRoot "dist"

# ─── Helpers ──────────────────────────────────────────────────────────────────

function Write-Step([string]$msg) {
    Write-Host ""
    Write-Host "==> $msg" -ForegroundColor Cyan
}

function Assert-Command([string]$cmd, [string]$hint) {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) {
        Write-Error "Required command not found: '$cmd'. $hint"
        exit 1
    }
}

function Get-CargoVersion {
    $content = Get-Content $CargoToml -Raw
    if ($content -match 'version\s*=\s*"([^"]+)"') {
        return $Matches[1]
    }
    throw "Could not read version from Cargo.toml"
}

# ─── Prerequisites ────────────────────────────────────────────────────────────

Write-Step "Checking prerequisites"

Assert-Command "cargo"         "Install Rust from https://rustup.rs"
Assert-Command "node"          "Install Node.js from https://nodejs.org"
Assert-Command "npm"           "Install Node.js from https://nodejs.org"

# Clean dist folder
$DistDir = Join-Path $RepoRoot "dist"
if (Test-Path $DistDir) {
    Write-Host "  Cleaning dist folder..."
    Remove-Item -Path $DistDir -Recurse -Force
}

# WiX v4 (installed as dotnet tool or standalone)
$wixInstalled = (Get-Command "wix" -ErrorAction SilentlyContinue) -or
                (Get-Command "heat" -ErrorAction SilentlyContinue)
if (-not $wixInstalled) {
    Write-Host "  Installing WiX v4 as dotnet tool..." -ForegroundColor Yellow
    try {
        dotnet tool install --global wix
    } catch {
        Write-Error "Failed to install WiX v4: $_"
        exit 1
    }
}

if (-not $Version) {
    $Version = Get-CargoVersion
}

Write-Host "  Version  : $Version"
Write-Host "  Channel  : $Channel"
Write-Host "  Sign     : $Sign"
Write-Host "  Repo root: $RepoRoot"

# ─── Install frontend dependencies ───────────────────────────────────────────

Write-Step "Installing frontend dependencies"
Push-Location $RepoRoot
try {
    npm ci --prefer-offline
} catch {
    Write-Error "npm ci failed: $_"
    Pop-Location
    exit 1
}
Pop-Location

# P8-63 — the bundling step is gone. It copied two Python trees into
# `src-tauri/resources/sidecars` before the Tauri bundler ran, and would now
# fail at its own existence check: the directories it copied from were deleted
# in `P8-C`.

# ─── Build Tauri app ─────────────────────────────────────────────────────────

Write-Step "Building Tauri app (release)"

Write-Host "  RepoRoot: $RepoRoot"
Write-Host "  TauriDir: $TauriDir"

Push-Location $RepoRoot

$env:ARCANA_CHANNEL = $Channel

# The update-signing public key is compiled into the binary from this
# environment variable (src-tauri/src/updater/verify.rs uses option_env!, and
# build.rs reruns on change). Nothing is generated here — a keypair is created
# once, out of band; see docs/updater-signing.md.
#
# A debug build without it simply cannot verify updates. A RELEASE build without
# it fails to compile on purpose, rather than shipping a binary that refuses
# every update it is offered.
if (-not $env:ARCANA_UPDATE_PUBKEY) {
    Write-Host "  ARCANA_UPDATE_PUBKEY not set - this build cannot verify updates" -ForegroundColor Yellow
}

try {
    npm run tauri build
} catch {
    Write-Error "Tauri build failed: $_"
    Pop-Location
    exit 1
}

Pop-Location

# ─── Locate built NSIS installer ────────────────────────────────────────────────────

Write-Host "  Locating built NSIS installer..."
$TargetDir = Join-Path $RepoRoot "target"
$NsisDir = Join-Path $TargetDir "release\bundle\nsis"
Write-Host "  TargetDir: $TargetDir"
Write-Host "  NsisDir: $NsisDir"

# Check if target directory exists
if (-not (Test-Path $TargetDir)) {
    Write-Error "Target directory not found: $TargetDir - Tauri build may have failed"
    exit 1
}

# Check if NSIS directory exists
if (-not (Test-Path $NsisDir)) {
    Write-Error "NSIS directory not found: $NsisDir - Tauri build may have failed or used a different target"
    Write-Host "  Contents of target directory:"
    Get-ChildItem -Path $TargetDir -Recurse | Select-Object FullName | ForEach-Object { Write-Host "    $($_.FullName)" }
    exit 1
}

$NsisFiles = Get-ChildItem -Path $NsisDir -Filter "*.exe" -ErrorAction SilentlyContinue
if (-not $NsisFiles) {
    Write-Error "No NSIS installer found at $NsisDir - build may have failed"
    Write-Host "  Contents of NSIS directory:"
    Get-ChildItem -Path $NsisDir -Recurse | Select-Object FullName | ForEach-Object { Write-Host "    $($_.FullName)" }
    exit 1
}
$Installer = $NsisFiles | Sort-Object LastWriteTime | Select-Object -Last 1
Write-Host "  Installer: $($Installer.FullName)"

# ─── Code signing ────────────────────────────────────────────────────────────

if ($Sign) {
    Write-Step "Signing installer"

    $certPath = $env:CERT_PATH
    $certPass = $env:CERT_PASSWORD
    $tsUrl    = if ($env:TIMESTAMP_URL) { $env:TIMESTAMP_URL } else { "http://timestamp.digicert.com" }

    if (-not $certPath) { Write-Error "CERT_PATH env var not set" }
    if (-not $certPass) { Write-Error "CERT_PASSWORD env var not set" }

    $signtool = "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.0.22621.0\x64\signtool.exe"
    if (-not (Test-Path $signtool)) {
        $signtool = "signtool"
    }

    & $signtool sign `
        /fd  sha256 `
        /td  sha256 `
        /tr  $tsUrl `
        /f   $certPath `
        /p   $certPass `
        /d   "Arcana" `
        $Installer.FullName

    & $signtool verify /pa /v $Installer.FullName
    Write-Host "  Signed successfully" -ForegroundColor Green
}

# ─── Copy to dist/ ────────────────────────────────────────────────────────────

Write-Step "Copying to dist/"

try {
    New-Item -ItemType Directory -Force -Path $DistDir | Out-Null
} catch {
    Write-Error "Failed to create dist directory: $_"
    exit 1
}

$ChannelSuffix = if ($Channel -eq "stable") { "" } else { "-$Channel" }
$DestName      = "arcana_${Version}${ChannelSuffix}_x64_en-US.exe"
$DestPath      = Join-Path $DistDir $DestName

try {
    Copy-Item $Installer.FullName $DestPath -Force
} catch {
    Write-Error "Failed to copy installer to dist: $_"
    exit 1
}

# Generate SHA-256 checksum
try {
    $Hash = Get-FileHash $DestPath -Algorithm SHA256
    "$($Hash.Hash.ToLower())  $DestName" | Out-File "${DestPath}.sha256" -Encoding ascii
} catch {
    Write-Error "Failed to generate checksum: $_"
    exit 1
}

Write-Host ""
Write-Host "Build complete:" -ForegroundColor Green
Write-Host "  $DestPath"
Write-Host "  $DestPath.sha256"
