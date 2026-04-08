# AutoDev

**A Model-Agnostic AI Code Editor for AIRS Foundry**

*One app. Any model. Full autonomy.*

---

## Overview

AutoDev is a downloadable desktop application that combines the best capabilities of Claude Code and Goose into a single, branded, model-agnostic AI developer agent platform. Purpose-built for AIRS Foundry's AutoDev worker pipeline, it delivers enterprise-grade agent orchestration with zero vendor lock-in.

**Key Features:**
- 🤖 **Model-agnostic:** Use any LLM (Anthropic, OpenAI, Gemini, Ollama, local inference)
- 🧠 **Advanced memory system:** 3-tier architecture (hot/warm/cold) with context scoring
- 🎯 **Agent orchestration:** 21 finite state machines governing lifecycle + A2A protocol
- 📝 **CLAUDE.md compatible:** Zero migration cost from existing workflows
- 🔌 **MCP native:** Connect to GitHub, Slack, Adobe, and custom MCP servers
- 🛠️ **Skills & Recipes:** Reusable YAML workflows with cron scheduling
- 🚀 **Single installer:** No Python, Redis, or Docker setup required

---

## Architecture

AutoDev is built on **Tauri v2** (Rust + React) and bundles two powerful Python sidecars:

```
┌─────────────────────────────────────┐
│      AutoDev Desktop App            │
│   (Tauri + React + TypeScript)     │
└──────────┬──────────────────────────┘
           │
    ┌──────┴──────┬──────────────────┐
    ▼             ▼                  ▼
┌────────┐  ┌─────────────┐  ┌──────────────┐
│ Rust   │  │   Core      │  │   Memory     │
│ Tools  │  │Orchestrator │  │  MCP Server  │
│        │  │  (Python)   │  │   (Python)   │
└────────┘  └─────────────┘  └──────────────┘
```

**Sidecars (bundled as standalone binaries):**
- **Core Orchestrator:** FastAPI-based agent runtime with 21 FSMs, A2A protocol, SSE streaming
- **Memory MCP Server:** 3-tier memory system (hot/warm/cold) with graph storage
- **Redis:** In-memory cache for warm-tier storage

---

## Tech Stack

| Layer | Technology | Purpose |
|-------|-----------|---------|
| **Desktop Shell** | Tauri v2 (Rust) | Cross-platform native wrapper (~15MB base) |
| **Frontend** | React + TypeScript + Vite | Agent UI, chat, code editor, task board |
| **Styling** | Tailwind CSS | AIRS design system (industrial-minimal) |
| **Backend** | Rust (Tauri commands) | Code editing tools, file I/O, command execution |
| **Orchestrator** | Python 3.13 + FastAPI | Agent lifecycle, FSMs, A2A protocol |
| **Memory** | Python 3.13 + mem0 | Context storage, scoring, synopsis evolution |
| **Cache** | Redis 7+ | Warm-tier memory caching |

---

## Project Structure

```
autodev/
├── README.md                      # This file
├── package.json                   # Node.js dependencies
├── .gitmodules                    # Submodule config
├── .gitignore
│
├── src/                           # React frontend
│   ├── components/
│   ├── pages/
│   └── main.tsx
│
├── src-tauri/                     # Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs
│       ├── commands.rs            # Tauri commands (edit_file, etc.)
│       └── sidecar.rs             # Sidecar lifecycle management
│
├── docs/                          # Documentation
│   ├── autodev-vision/
│   │   └── autodev-master-plan.md   # Living product spec
│   ├── submodules-management/
│   │   └── SUBMODULE_SETUP_GUIDE.md # Git submodule workflow
│   └── ENVIRONMENT_SETUP.md       # Installation guide
│
├── submodules/                    # Git submodules (Python projects)
│   ├── core-orchestrator/         # Agent orchestration engine
│   └── memory-mcp-server/         # Memory system
│
├── build/                         # Build scripts
│   ├── bundle-core.ps1            # PyInstaller for core-orchestrator
│   ├── bundle-memory-mcp.ps1      # PyInstaller for memory-mcp-server
│   └── fetch-redis.ps1            # Download Redis binary
│
└── sidecars/                      # Build output (gitignored)
    ├── core-orchestrator.exe
    ├── memory-mcp-server.exe
    └── redis-server.exe
```

---

## Quick Start

### Prerequisites

- **Python 3.13+**
- **uv** (Python package manager)
- **Rust** (stable via rustup)
- **Node.js 20+** & npm
- **Git**

**Check your installation:**
```powershell
python --version  # 3.13+
uv --version
rustc --version
node --version    # v20+
```

See [`docs/ENVIRONMENT_SETUP.md`](docs/ENVIRONMENT_SETUP.md) for detailed installation instructions.

### Installation

```powershell
# Clone the repo
git clone https://github.com/AIRS-Foundry/autodev.git
cd autodev

# Initialize submodules
git submodule update --init --recursive

# Install npm dependencies
npm install

# Run development server (once Tauri is configured)
npm run tauri dev
```

---

## Development

### Running Locally

```powershell
# Terminal 1: Start Tauri dev server
npm run tauri dev

# Terminal 2 (optional): Test core-orchestrator independently
cd submodules/core-orchestrator
uv venv && .venv\Scripts\activate && uv sync
python src/main.py
```

### Building for Production

```powershell
# Build all sidecars
.\build\bundle-core.ps1
.\build\bundle-memory-mcp.ps1
.\build\fetch-redis.ps1

# Build Tauri app
npm run tauri build
```

---

## Documentation

| Document | Description |
|----------|-------------|
| [`docs/autodev-vision/autodev-master-plan.md`](docs/autodev-vision/autodev-master-plan.md) | **Living product spec** - single source of truth |
| [`docs/ENVIRONMENT_SETUP.md`](docs/ENVIRONMENT_SETUP.md) | Complete environment setup guide |
| [`docs/submodules-management/SUBMODULE_SETUP_GUIDE.md`](docs/submodules-management/SUBMODULE_SETUP_GUIDE.md) | Git submodule workflow |

---

## Key Concepts

### Submodules
Core Orchestrator and Memory MCP Server are tracked as **git submodules** (commit references, not copied code). To edit:
1. Work inside `submodules/{repo}` as a normal git repo
2. Commit and push changes to the submodule's remote
3. Update the pointer in the main autodev repo

See [`docs/submodules-management/SUBMODULE_SETUP_GUIDE.md`](docs/submodules-management/SUBMODULE_SETUP_GUIDE.md) for details.

### Tool Routing
- **Fast path** (Tauri Rust): File operations, grep, command execution
- **Stateful path** (Core Orchestrator): Agent FSMs, A2A protocol, MCP client pool
- **Memory path** (Memory MCP): Context storage, scoring, retrieval

### No `uv sync` in Main Repo
AutoDev is a **Tauri app** (Rust + Node.js), not a Python project. Only run `uv sync` inside submodules when developing them locally.

---

## Features (from Claude Code + Goose)

### From Claude Code
- CLAUDE.md/AGENTS.md support
- Headless `-p` mode for CLI automation
- Subagents + Agent Teams
- Skills, Hooks, Plugins
- MCP client integration
- Git worktree per task
- Context compaction
- Plan mode

### From Goose
- Model-agnostic provider layer
- Recipes (YAML workflows) + cron scheduler
- Per-session cost audit + export
- Approval modes (Autonomous/Manual/Smart/Chat-only)
- `.goosehints` directory-scoped context
- Custom distributions
- ACP (Agent Client Protocol) for IDE plugins

### AutoDev-Native
- **AutoDev Panel:** Live GitHub Issues board, agent roster, QC queue
- **Key Vault:** Encrypted API key storage (OS keychain)
- **Model Router:** Route tasks to different models by priority/role
- **Cost Meter:** Real-time token spend tracking
- **Dispatch Signal UI:** Trigger workers directly from UI

---

## Current Status

**Phase:** Environment setup complete ✅  
**Next:** Scaffold Tauri app structure

**Environment verified:**
- ✅ Python 3.13.3
- ✅ uv 0.7.4
- ✅ Rust 1.94.0 / Cargo 1.94.0
- ✅ Node.js v22.1.0 / npm 10.7.0
- ✅ Git 2.45.0
- ✅ Submodules initialized (core-orchestrator, memory-mcp-server)
- ✅ npm packages installed (React, Vite, TypeScript, Tailwind, Tauri CLI)

---

## Contributing

This is an internal AIRS Foundry project. For questions or contributions, contact:

**Programming Lead:** Razi  
**Email:** razi@airsfoundry.com

---

## License

Proprietary - AIRS Foundry © 2026

---

## FAQ

**Q: Do I need Docker?**  
A: No. Redis runs as a standalone binary sidecar.

**Q: Can I use different LLM providers?**  
A: Yes. AutoDev supports Anthropic, OpenAI, Gemini, Ollama, Azure, and Bedrock.

**Q: Is this compatible with my existing CLAUDE.md files?**  
A: Yes. 100% backward compatible with Claude Code workflows.

**Q: How do I update the submodules?**  
A: `git submodule update --remote --merge`

**Q: Why Node v22.1.0 warnings?**  
A: Vite 8 prefers v22.12+, but v22.1.0 works fine. Warnings can be ignored.

---

**For detailed setup instructions, see [`docs/ENVIRONMENT_SETUP.md`](docs/ENVIRONMENT_SETUP.md)**
