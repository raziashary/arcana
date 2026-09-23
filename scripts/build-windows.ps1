#Requires -Version 5.1
<#
.SYNOPSIS
    Build Arcana Windows installers (MSI + NSIS).

.DESCRIPTION
    Convenience wrapper around installers/wix/build-msi.ps1.
    Optionally signs and uploads to GitHub Releases.

.PARAMETER Sign
    Sign the installer. Requires CERT_PATH + CERT_PASSWORD.

.PARAMETER Upload
    Upload to GitHub Releases. Requires GITHUB_TOKEN + RELEASE_TAG.

.PARAMETER Channel
    stable (default), beta, or nightly.
#>

[CmdletBinding()]
param(
    [switch]$Sign,
    [switch]$Upload,
    [ValidateSet("stable","beta","nightly")]
    [string]$Channel = "stable"
)

$ErrorActionPreference = "Stop"
$ScriptDir = $PSScriptRoot
$RepoRoot  = Split-Path $ScriptDir -Parent

Write-Host "Arcana Windows Build" -ForegroundColor Cyan
Write-Host "=====================" -ForegroundColor Cyan

$buildScript = Join-Path $RepoRoot "installers\wix\build-msi.ps1"

$params = @{ Channel = $Channel }
if ($Sign)   { $params.Sign   = $true }

& $buildScript @params

if ($Upload) {
    $tag = $env:RELEASE_TAG
    if (-not $tag) {
        Write-Error "RELEASE_TAG env var not set — cannot upload"
    }

    $token = $env:GITHUB_TOKEN
    if (-not $token) {
        Write-Error "GITHUB_TOKEN env var not set — cannot upload"
    }

    $distDir = Join-Path $RepoRoot "dist"
    $msiFiles = Get-ChildItem $distDir -Filter "*.msi"
    $sha256Files = Get-ChildItem $distDir -Filter "*.sha256"

    foreach ($file in @($msiFiles + $sha256Files)) {
        Write-Host "  Uploading $($file.Name)..." -ForegroundColor Cyan
        gh release upload $tag $file.FullName --clobber
    }
    Write-Host "Upload complete" -ForegroundColor Green
}
