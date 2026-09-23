# AIRS Arcana worker setup script — Windows PowerShell
# Usage:
#   iwr -useb https://raw.githubusercontent.com/raziashary/arcana/main/src-cli/setup.ps1 | iex
# With a fleet token:
#   & ([scriptblock]::Create((iwr -useb .../setup.ps1))) --Token "TOKEN" --Type "worker-backend"
#
# The script:
#   1. Installs the arcanactl CLI binary (winget or cargo fallback)
#   2. Redeems the fleet token (if provided)
#   3. Installs the appropriate worker plugin via winget / package managers
#   4. Verifies the installation
#
# Requires: PowerShell 5.1+, Windows 10/11, internet access.
# Run as Administrator for system-wide installs; user install works without elevation.

[CmdletBinding()]
param(
    [string]$Token       = "",
    [string]$Type        = "worker-backend",
    [string]$InstallDir  = "$env:LOCALAPPDATA\arcanactl\bin",
    [string]$Version     = "latest"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# ─── Helpers ──────────────────────────────────────────────────────────────────
function Write-Info    ($msg) { Write-Host "[INFO]  $msg" -ForegroundColor Cyan    }
function Write-Success ($msg) { Write-Host "[OK]    $msg" -ForegroundColor Green   }
function Write-Warn    ($msg) { Write-Host "[WARN]  $msg" -ForegroundColor Yellow  }
function Write-Err     ($msg) { Write-Host "[ERROR] $msg" -ForegroundColor Red; exit 1 }

function Test-Command ($cmd) {
    $null -ne (Get-Command $cmd -ErrorAction SilentlyContinue)
}

# ─── Install arcanactl ──────────────────────────────────────────────────────────
function Install-Arcanactl {
    Write-Info "Installing arcanactl CLI..."

    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    $arch = if ([System.Environment]::Is64BitOperatingSystem) { "x86_64" } else { "x86" }
    $exeName = "arcanactl-windows-$arch.exe"
    $releaseBase = "https://github.com/raziashary/arcana/releases"
    $downloadUrl = if ($Version -eq "latest") {
        "$releaseBase/latest/download/$exeName"
    } else {
        "$releaseBase/download/$Version/$exeName"
    }

    $outPath = Join-Path $InstallDir "arcanactl.exe"

    try {
        Write-Info "Downloading arcanactl from GitHub Releases..."
        Invoke-WebRequest -Uri $downloadUrl -OutFile $outPath -UseBasicParsing -TimeoutSec 30
        Write-Success "arcanactl downloaded to $outPath"
    } catch {
        Write-Warn "Binary download failed ($_). Trying cargo install..."
        if (-not (Test-Command "cargo")) {
            Write-Info "Installing Rust via winget..."
            winget install -e --id Rustlang.Rustup --accept-source-agreements --accept-package-agreements
            # Refresh PATH
            $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" +
                        [System.Environment]::GetEnvironmentVariable("Path", "User")
        }
        cargo install arcanactl --root "$env:LOCALAPPDATA\arcanactl"
        $InstallDir = "$env:LOCALAPPDATA\arcanactl\bin"
        Write-Success "arcanactl built via cargo."
    }

    # Add to user PATH if not already present
    $userPath = [System.Environment]::GetEnvironmentVariable("Path", "User")
    if ($userPath -notlike "*$InstallDir*") {
        [System.Environment]::SetEnvironmentVariable(
            "Path", "$userPath;$InstallDir", "User"
        )
        $env:Path = "$env:Path;$InstallDir"
        Write-Info "Added $InstallDir to user PATH. Restart terminal to persist."
    }
}

# ─── Ensure arcanactl available ─────────────────────────────────────────────────
function Ensure-Arcanactl {
    if (-not (Test-Command "arcanactl")) {
        Install-Arcanactl
    } else {
        $ver = (& arcanactl --version 2>$null) -join ""
        Write-Success "arcanactl found: $ver"
    }
}

# ─── Redeem token ─────────────────────────────────────────────────────────────
function Invoke-TokenRedeem {
    if ([string]::IsNullOrWhiteSpace($Token)) {
        Write-Info "No token provided — installing '$Type' directly."
        return
    }

    Write-Info "Validating setup token..."
    try {
        arcanactl worker-setup validate --token $Token 2>$null
        Write-Success "Token valid."
        Write-Info "Redeeming token (installing '$Type')..."
        arcanactl worker-setup redeem --token $Token
    } catch {
        Write-Warn "Token validation failed — installing plugin directly."
    }
}

# ─── Install worker plugin ────────────────────────────────────────────────────
function Install-WorkerPlugin {
    Write-Info "Installing plugin: $Type"
    arcanactl plugin install $Type
    Write-Success "Plugin '$Type' installed."
}

# ─── Verify ───────────────────────────────────────────────────────────────────
function Invoke-Verify {
    Write-Info "Verifying installation..."
    try {
        arcanactl plugin verify $Type
        Write-Success "All dependency checks passed for '$Type'."
    } catch {
        Write-Warn "Some checks failed — review output above."
    }
}

# ─── Main ─────────────────────────────────────────────────────────────────────
function Main {
    Write-Host ""
    Write-Host "AIRS Arcana Worker Setup" -ForegroundColor Cyan -NoNewline
    Write-Host " | Worker type: " -NoNewline
    Write-Host $Type -ForegroundColor White
    Write-Host ""

    Ensure-Arcanactl

    if (-not [string]::IsNullOrWhiteSpace($Token)) {
        Invoke-TokenRedeem
    } else {
        Install-WorkerPlugin
    }

    Invoke-Verify

    Write-Host ""
    Write-Success "Setup complete! Run 'arcanactl plugin list --installed' to see installed plugins."
    Write-Host ""
}

Main
