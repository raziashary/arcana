# AutoDev Environment Setup Guide
## Complete Installation & Configuration Checklist

**Target Directory:** `c:\AIRS Foundry\autodev\autodev`

---

## Overview

AutoDev is a Tauri-based desktop application that bundles:
- **Core Orchestrator** (Python FastAPI sidecar)
- **Memory MCP Server** (Python MCP sidecar)
- **Redis** (Standalone binary sidecar)
- **React + TypeScript** frontend
- **Rust** backend (Tauri)

---

## 1. Prerequisites Checklist

### ✅ Required Software

| Software | Version | Check Command | Installation Link |
|----------|---------|---------------|-------------------|
| **Python** | 3.13+ | `python --version` | https://www.python.org/downloads/ |
| **uv** | Latest | `uv --version` | `pip install uv` or https://docs.astral.sh/uv/ |
| **Rust** | Stable | `rustc --version` | https://rustup.rs/ |
| **Node.js** | 20+ LTS | `node --version` | https://nodejs.org/ |
| **npm** | 10+ | `npm --version` | (comes with Node.js) |
| **Git** | Latest | `git --version` | https://git-scm.com/ |

### ✅ Optional (Development Tools)

| Tool | Purpose | Check Command |
|------|---------|---------------|
| **VS Code** | IDE | `code --version` |
| **Tauri CLI** | Development | `cargo tauri --version` |
| **PyInstaller** | Building sidecars | `pyinstaller --version` |

---

## 2. Check Your Current Installation Status

Run these commands to see what you already have:

```powershell
# Python check
python --version
# Should show: Python 3.13.x

# uv check
uv --version
# Should show: uv x.x.x

# Rust check
rustc --version
cargo --version
# Should show: rustc 1.x.x and cargo 1.x.x

# Node.js check
node --version
npm --version
# Should show: v20.x.x and 10.x.x

# Git check
git --version
# Should show: git version 2.x.x
```

---

## 3. Installation Instructions

### 3.1 Install Python 3.13

**If Python is not installed or version < 3.13:**

1. Download Python 3.13 from https://www.python.org/downloads/
2. **Important:** Check "Add Python to PATH" during installation
3. Verify installation:
   ```powershell
   python --version
   ```

### 3.2 Install uv (Python Package Manager)

```powershell
# Install uv globally
pip install uv

# Verify installation
uv --version
```

**Alternative installation (recommended):**
```powershell
# Install via standalone installer
powershell -c "irm https://astral.sh/uv/install.ps1 | iex"
```

### 3.3 Install Rust

**If Rust is not installed:**

1. Download rustup from https://rustup.rs/
2. Run the installer (it will download and install Rust)
3. Restart your terminal
4. Verify installation:
   ```powershell
   rustc --version
   cargo --version
   rustup --version
   ```

**Update Rust if already installed:**
```powershell
rustup update stable
```

### 3.4 Install Node.js & npm

**If Node.js is not installed or version < 20:**

1. Download Node.js LTS (20.x) from https://nodejs.org/
2. Run the installer
3. Verify installation:
   ```powershell
   node --version
   npm --version
   ```

### 3.5 Install Tauri CLI (for development)

```powershell
cargo install tauri-cli
```

Verify:
```powershell
cargo tauri --version
```

---

## 4. Do You Need to Run `uv sync` in Submodules?

### Short Answer: **Only if you want to develop/test the submodules locally**

### Detailed Explanation:

**In the main autodev repo (`c:\AIRS Foundry\autodev\autodev`):**
- ❌ **NO** `uv venv` or `uv sync` needed
- The main repo is a Tauri app (Rust + Node), not Python
- Submodules are just git references, not executed directly

**Inside submodules (for development/testing only):**

If you want to test core-orchestrator or memory-mcp-server independently:

```powershell
# For core-orchestrator
cd c:\AIRS` Foundry\autodev\autodev\submodules\core-orchestrator
uv venv
.venv\Scripts\activate
uv sync

# Test it
python src/main.py

# For memory-mcp-server
cd c:\AIRS` Foundry\autodev\autodev\submodules\memory-mcp-server
uv venv
.venv\Scripts\activate
uv sync

# Test it
python src/server.py
```

**When building sidecars (later):**
- The build scripts will handle dependency installation
- PyInstaller bundles everything into standalone executables
- You don't manually install deps for bundled sidecars

### Summary Table

| Context | Run `uv sync`? | Why? |
|---------|----------------|------|
| Main autodev repo | ❌ No | It's a Tauri app (Rust+Node), not Python |
| Submodule (for development) | ✅ Yes | To test Python code locally |
| Submodule (for production build) | ❌ No | Build scripts handle it via PyInstaller |

---

## 5. Initialize AutoDev Project Structure

### 5.1 Create Tauri Project Structure

```powershell
cd "c:\AIRS Foundry\autodev\autodev"

# Create directories
mkdir src
mkdir src-tauri
mkdir build
mkdir sidecars

# Sidecars is gitignored (build output)
# Build scripts will populate it
```

### 5.2 Initialize Node.js Frontend

```powershell
cd "c:\AIRS Foundry\autodev\autodev"

# Initialize npm project
npm init -y

# Install React + TypeScript + Vite
npm install react react-dom
npm install -D @vitejs/plugin-react vite typescript @types/react @types/react-dom

# Install Tailwind CSS
npm install -D tailwindcss postcss autoprefixer
npx tailwindcss init -p

# Install Tauri API
npm install @tauri-apps/api @tauri-apps/plugin-shell
```

### 5.3 Initialize Tauri Backend

```powershell
cd "c:\AIRS Foundry\autodev\autodev"

# Initialize Tauri (this creates src-tauri/ structure)
npm install -D @tauri-apps/cli

# Create Tauri project (skip if you want to scaffold manually)
# Or we'll create Cargo.toml and tauri.conf.json manually
```

---

## 6. Verify Environment Is Ready

Run this verification script:

```powershell
# Save this as verify-environment.ps1
Write-Host "=== AutoDev Environment Verification ===" -ForegroundColor Cyan

# Python
Write-Host "`n[Python]" -ForegroundColor Yellow
try {
    $pythonVersion = python --version 2>&1
    if ($pythonVersion -match "3\.1[3-9]") {
        Write-Host "✅ $pythonVersion" -ForegroundColor Green
    } else {
        Write-Host "❌ Python 3.13+ required, found: $pythonVersion" -ForegroundColor Red
    }
} catch {
    Write-Host "❌ Python not found" -ForegroundColor Red
}

# uv
Write-Host "`n[uv]" -ForegroundColor Yellow
try {
    $uvVersion = uv --version 2>&1
    Write-Host "✅ $uvVersion" -ForegroundColor Green
} catch {
    Write-Host "❌ uv not found - Install: pip install uv" -ForegroundColor Red
}

# Rust
Write-Host "`n[Rust]" -ForegroundColor Yellow
try {
    $rustVersion = rustc --version 2>&1
    $cargoVersion = cargo --version 2>&1
    Write-Host "✅ $rustVersion" -ForegroundColor Green
    Write-Host "✅ $cargoVersion" -ForegroundColor Green
} catch {
    Write-Host "❌ Rust not found - Install: https://rustup.rs/" -ForegroundColor Red
}

# Node.js
Write-Host "`n[Node.js]" -ForegroundColor Yellow
try {
    $nodeVersion = node --version 2>&1
    $npmVersion = npm --version 2>&1
    if ($nodeVersion -match "v[2-9][0-9]") {
        Write-Host "✅ Node $nodeVersion" -ForegroundColor Green
        Write-Host "✅ npm $npmVersion" -ForegroundColor Green
    } else {
        Write-Host "❌ Node 20+ required, found: $nodeVersion" -ForegroundColor Red
    }
} catch {
    Write-Host "❌ Node.js not found" -ForegroundColor Red
}

# Git
Write-Host "`n[Git]" -ForegroundColor Yellow
try {
    $gitVersion = git --version 2>&1
    Write-Host "✅ $gitVersion" -ForegroundColor Green
} catch {
    Write-Host "❌ Git not found" -ForegroundColor Red
}

# Submodules
Write-Host "`n[Submodules]" -ForegroundColor Yellow
$submodules = git submodule status
if ($submodules -match "core-orchestrator" -and $submodules -match "memory-mcp-server") {
    Write-Host "✅ Submodules initialized" -ForegroundColor Green
} else {
    Write-Host "⚠️  Run: git submodule update --init --recursive" -ForegroundColor Yellow
}

Write-Host "`n=== Verification Complete ===" -ForegroundColor Cyan
```

Run it:
```powershell
cd "c:\AIRS Foundry\autodev\autodev"
.\verify-environment.ps1
```

---

## 7. Project Structure After Setup

```
c:\AIRS Foundry\autodev\autodev/
├── .git/                          # Git repository
├── .gitignore                     # Ignore sidecars/, node_modules/, etc.
├── .gitmodules                    # Submodule config (already exists)
├── README.md
├── package.json                   # Node.js dependencies
├── tsconfig.json                  # TypeScript config
├── vite.config.ts                 # Vite config
├── tailwind.config.js             # Tailwind CSS config
├── postcss.config.js              # PostCSS config
│
├── src/                           # React frontend
│   ├── main.tsx
│   ├── App.tsx
│   ├── components/
│   └── styles/
│
├── src-tauri/                     # Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   └── src/
│       ├── main.rs
│       ├── commands.rs
│       └── sidecar.rs
│
├── docs/                          # Documentation
│   ├── autodev-vision/
│   │   └── autodev-master-plan.md
│   ├── submodules-management/
│   └── ENVIRONMENT_SETUP.md       # This file
│
├── submodules/                    # Git submodules
│   ├── core-orchestrator/         # Python FastAPI orchestrator
│   └── memory-mcp-server/         # Python Memory MCP server
│
├── build/                         # Build scripts
│   ├── bundle-core.sh             # PyInstaller for core-orchestrator
│   ├── bundle-memory-mcp.sh       # PyInstaller for memory-mcp-server
│   └── fetch-redis.sh             # Download Redis binary
│
└── sidecars/                      # Build output (gitignored)
    ├── core-orchestrator.exe
    ├── memory-mcp-server.exe
    └── redis-server.exe
```

---

## 8. Next Steps After Environment Setup

1. **✅ Verify all prerequisites are installed** (see section 6)

2. **Initialize frontend:**
   ```powershell
   npm install
   ```

3. **Create Tauri configuration files:**
   - `src-tauri/Cargo.toml`
   - `src-tauri/tauri.conf.json`
   - `src-tauri/src/main.rs`

4. **Create build scripts:**
   - `build/bundle-core.sh` (or `.ps1` for Windows)
   - `build/bundle-memory-mcp.sh`

5. **Test Tauri development server:**
   ```powershell
   npm run tauri dev
   ```

6. **Test submodules independently** (optional):
   ```powershell
   cd submodules/core-orchestrator
   uv venv && .venv\Scripts\activate && uv sync
   python src/main.py
   ```

---

## 9. Common Issues & Solutions

### Issue: Python not found or wrong version

**Solution:**
- Install Python 3.13+ from https://www.python.org/downloads/
- Make sure "Add to PATH" was checked during installation
- Restart terminal after installation

### Issue: `uv` command not found

**Solution:**
```powershell
pip install uv
# OR
powershell -c "irm https://astral.sh/uv/install.ps1 | iex"
```

### Issue: Rust/Cargo not found

**Solution:**
- Install from https://rustup.rs/
- Restart terminal
- Run `rustup default stable`

### Issue: Node/npm not found

**Solution:**
- Install Node.js 20 LTS from https://nodejs.org/
- Restart terminal

### Issue: Submodules are empty

**Solution:**
```powershell
git submodule update --init --recursive
```

### Issue: Permission denied on scripts

**Solution (PowerShell):**
```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

---

## 10. Quick Start Commands (After Setup)

```powershell
# Navigate to project
cd "c:\AIRS Foundry\autodev\autodev"

# Install frontend dependencies
npm install

# Run development server (once Tauri is configured)
npm run tauri dev

# Build for production (once build scripts are ready)
npm run tauri build

# Update submodules to latest
git submodule update --remote --merge

# Test a submodule locally
cd submodules/core-orchestrator
uv venv
.venv\Scripts\activate
uv sync
python src/main.py
```

---

## 11. Key Concepts

### Git Submodules
- **core-orchestrator** and **memory-mcp-server** are separate repos
- They're tracked as **commit references**, not copied code
- Edit them inside `submodules/` directory
- Commit changes in the submodule first, then update pointer in main repo
- See `docs/submodules-management/SUBMODULE_SETUP_GUIDE.md` for details

### Tool Integration Architecture
- **Fast path** (Tauri Rust): `edit_file`, `read_file`, `grep_search`, `run_command`
  - No HTTP overhead, direct filesystem access
- **Stateful path** (Core Orchestrator): FSM state, A2A protocol, agent lifecycle
  - Routed via HTTP to `localhost:8765`
- **Memory path** (Memory MCP): 3-tier storage, context scoring, synopsis evolution
  - Core Orchestrator proxies to Memory MCP on `localhost:8766`

### Build Process
- **Development:** Run submodules as Python processes (via sidecar launch)
- **Production:** PyInstaller bundles them into standalone executables
- **User sees:** One installer, one app, no Python/Redis/Docker setup needed

---

## 12. FAQ

### Q: Do I run `uv sync` in the main autodev repo?
**A:** No. The main repo is a Tauri app (Rust + Node.js), not a Python project. Only run `uv sync` inside submodules when developing/testing them locally.

### Q: Do I need Docker?
**A:** No. Redis runs as a standalone binary sidecar. No containers needed.

### Q: Do I need to install Redis?
**A:** No. The build scripts will download a pre-built Redis binary for your OS.

### Q: Can I use Python 3.12?
**A:** No. The submodules require Python 3.13+ (check `submodules/*/pyproject.toml`).

### Q: What if I edit code in a submodule?
**A:** Work inside the submodule as a normal git repo:
1. `cd submodules/core-orchestrator`
2. Make changes, commit, push to submodule remote
3. `cd ../..` back to main repo
4. `git add submodules/core-orchestrator` to update the pointer
5. Commit and push in main repo

### Q: Do I need PyInstaller now?
**A:** Not yet. You'll need it when building production bundles, but we'll set that up later.

### Q: Why do I see Node version warnings?
**A:** Vite 8 prefers Node 22.12+, but 22.1.0 works fine. You can update Node or ignore the warnings.

---

## 13. Current Environment Status

Based on your terminal output, you have:

✅ **Python 3.13.3** - Perfect  
✅ **uv 0.7.4** - Perfect  
✅ **Rust 1.94.0** - Perfect  
✅ **Cargo 1.94.0** - Perfect  
✅ **Node v22.1.0** - Good (minor version warnings can be ignored)  
✅ **npm 10.7.0** - Perfect  
✅ **Git 2.45.0** - Perfect  

✅ **Directories created:** `src/`, `src-tauri/`, `build/`, `sidecars/`  
✅ **npm packages installed:** React, Vite, TypeScript, Tailwind, Tauri CLI  
✅ **Submodules configured:** core-orchestrator, memory-mcp-server  

**Status:** Environment fully ready. Next step: scaffold Tauri app structure.

---

## Summary

**Prerequisites installed:**
1. ✅ Python 3.13.3
2. ✅ uv 0.7.4
3. ✅ Rust 1.94.0 / Cargo 1.94.0
4. ✅ Node.js v22.1.0 / npm 10.7.0
5. ✅ Git 2.45.0

**You DON'T need to:**
- ❌ Run `uv sync` in the main autodev repo (it's Tauri, not Python)
- ❌ Run `uv sync` in submodules unless you're developing them locally
- ❌ Install Docker or Redis manually

**Next:** Create Tauri configuration files and start building the app structure.
