#Requires -Version 5.1
<#
.SYNOPSIS
    Build the AutoDev Windows MSI installer using Tauri + WiX v4.

.DESCRIPTION
    1. Verifies prerequisites (Rust, Node, WiX toolset)
    2. Builds the Tauri application in release mode
    3. Signs the installer with the code-signing certificate if CERT_PATH is set
    4. Outputs autodev_<version>_x64_en-US.msi to dist/

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

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# ─── Paths ────────────────────────────────────────────────────────────────────

$RepoRoot    = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
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

# WiX v4 (installed as dotnet tool or standalone)
$wixInstalled = (Get-Command "wix" -ErrorAction SilentlyContinue) -or
                (Get-Command "heat" -ErrorAction SilentlyContinue)
if (-not $wixInstalled) {
    Write-Host "  Installing WiX v4 as dotnet tool..." -ForegroundColor Yellow
    dotnet tool install --global wix
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
npm ci --prefer-offline
Pop-Location

# ─── Build Tauri app ─────────────────────────────────────────────────────────

Write-Step "Building Tauri app (release)"
Push-Location $RepoRoot

$env:AUTODEV_CHANNEL = $Channel

# Generate Ed25519 keypair if not already present (dev builds only)
if (-not $env:AUTODEV_UPDATE_PUBKEY) {
    Write-Host "  AUTODEV_UPDATE_PUBKEY not set — using dev placeholder" -ForegroundColor Yellow
}

npm run tauri -- build --target x86_64-pc-windows-msvc --bundles msi

Pop-Location

# ─── Locate built MSI ─────────────────────────────────────────────────────────

$MsiPattern = Join-Path $TauriDir "target\x86_64-pc-windows-msvc\release\bundle\msi\*.msi"
$MsiFiles   = Get-ChildItem -Path (Split-Path $MsiPattern -Parent) -Filter "*.msi" -ErrorAction SilentlyContinue
if (-not $MsiFiles) {
    Write-Error "No MSI found at $MsiPattern — build may have failed"
}
$Msi = $MsiFiles | Sort-Object LastWriteTime | Select-Object -Last 1
Write-Host "  MSI: $($Msi.FullName)"

# ─── Code signing ────────────────────────────────────────────────────────────

if ($Sign) {
    Write-Step "Signing installer"

    $certPath = $env:CERT_PATH
    $certPass = $env:CERT_PASSWORD
    $tsUrl    = $env:TIMESTAMP_URL ?? "http://timestamp.digicert.com"

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
        /d   "AutoDev" `
        /du  "https://airs.dev/autodev" `
        $Msi.FullName

    & $signtool verify /pa /v $Msi.FullName
    Write-Host "  Signed successfully" -ForegroundColor Green
}

# ─── Copy to dist/ ────────────────────────────────────────────────────────────

Write-Step "Copying to dist/"

New-Item -ItemType Directory -Force -Path $DistDir | Out-Null

$ChannelSuffix = if ($Channel -eq "stable") { "" } else { "-$Channel" }
$DestName      = "autodev_${Version}${ChannelSuffix}_x64_en-US.msi"
$DestPath      = Join-Path $DistDir $DestName

Copy-Item $Msi.FullName $DestPath -Force

# Generate SHA-256 checksum
$Hash = Get-FileHash $DestPath -Algorithm SHA256
"$($Hash.Hash.ToLower())  $DestName" | Out-File "${DestPath}.sha256" -Encoding ascii

Write-Host ""
Write-Host "Build complete:" -ForegroundColor Green
Write-Host "  $DestPath"
Write-Host "  $DestPath.sha256"
