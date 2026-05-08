# Bundle sidecars for Tauri packaging
# This script runs before Tauri bundles the app, ensuring sidecars are in the resources directory

$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$TauriDir = Join-Path $RepoRoot "src-tauri"
$SidecarsDir = Join-Path $TauriDir "resources\sidecars"

Write-Host "Bundling sidecars for Tauri packaging..."

# Create resources/sidecars directory
if (-not (Test-Path $SidecarsDir)) {
    New-Item -ItemType Directory -Force -Path $SidecarsDir | Out-Null
}

$OrchSrc = Join-Path $RepoRoot "submodules\core-orchestrator"
$MemorySrc = Join-Path $RepoRoot "submodules\memory-mcp-server"

# Verify submodules exist
if (-not (Test-Path $OrchSrc)) {
    Write-Error "core-orchestrator submodule not found at $OrchSrc"
    exit 1
}

if (-not (Test-Path $MemorySrc)) {
    Write-Error "memory-mcp-server submodule not found at $MemorySrc"
    exit 1
}

# Copy core-orchestrator
Write-Host "Copying core-orchestrator..."
$robocopyArgs = @(
    $OrchSrc,
    (Join-Path $SidecarsDir "core-orchestrator"),
    "/E",
    "/XD", ".venv",
    "/XD", ".git",
    "/NFL",
    "/NDL",
    "/NJH",
    "/NJS",
    "/NC",
    "/NS"
)
$robocopyResult = robocopy @robocopyArgs
if ($robocopyResult -gt 7) {
    Write-Error "Failed to copy core-orchestrator"
    exit 1
}

# Copy memory-mcp-server
Write-Host "Copying memory-mcp-server..."
$robocopyArgs = @(
    $MemorySrc,
    (Join-Path $SidecarsDir "memory-mcp-server"),
    "/E",
    "/XD", ".venv",
    "/XD", ".git",
    "/NFL",
    "/NDL",
    "/NJH",
    "/NJS",
    "/NC",
    "/NS"
)
$robocopyResult = robocopy @robocopyArgs
if ($robocopyResult -gt 7) {
    Write-Error "Failed to copy memory-mcp-server"
    exit 1
}

Write-Host "Sidecars bundled successfully"
