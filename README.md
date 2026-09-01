# AutoDev

**A Model-Agnostic AI Code Editor for AIRS Foundry**

*One app. Any model. Full autonomy.*

---

## Overview

AutoDev is a downloadable desktop application that combines the best capabilities of Claude Code and Goose into a single, branded, model-agnostic AI developer agent platform. Purpose-built for AIRS Foundry's AutoDev worker pipeline, it delivers enterprise-grade agent orchestration with zero vendor lock-in.

**Vision:** Any AIRS team member or AutoDev worker agent can download one binary, enter their LLM API key (any provider), select their model, and start executing agentic development tasks with the full capability stack: AUTODEV.md, Skills, Recipes, Subagents, MCP servers, Hooks, and the AutoDev worker pipeline.

**Why Build This:**
- **Claude Code limitations:** Locked to Anthropic subscriptions with rate limits that cripple 24/7 worker agents; 512,000-line source leak (March 31, 2026) exposed vulnerabilities; no model flexibility; proprietary and unauditable
- **Goose limitations:** AGENTS.md ecosystem is newer and less mature; not tailored to AutoDev's Supervisor → Worker → QC pipeline; generic tool with no AIRS-specific automation
- **AutoDev advantage:** Model-agnostic by design (use Anthropic API directly or OpenAI/Gemini/Ollama); AUTODEV.md-native with SQLite-backed instruction system; AutoDev-native GitHub Issues board understanding; secure and auditable (AIRS controls the harness); downloadable and distributable single installer

**Key Features:**
- 🤖 **Model-agnostic:** Use any LLM (Anthropic, OpenAI, Gemini, Ollama, Azure, Bedrock)
- 🧠 **Advanced memory system:** 3-tier architecture (episodic/semantic/procedural) with context scoring, compression, staleness tracking
- 🎯 **Agent orchestration:** 21 finite state machines governing lifecycle + A2A protocol
- 📝 **AUTODEV.md system:** SQLite-backed instruction system, editable from UI, no manual file editing required
- 🔌 **MCP servers:** External MCP servers can be registered and are health-checked in Settings. AutoDev does not yet call them, and it does not host one — the in-app MCP server was removed because it was an unauthenticated local endpoint exposing `bash`, `write_file` and `spawn_subagent`.
- 🛠️ **Skills & Recipes:** Reusable YAML workflows with cron scheduling for fleet orchestration
- 🚀 **Single installer:** No Python, Redis, or Docker setup required for end users
- 🎭 **Dual-mode execution:** PA mode (Rust-native for speed) + Worker mode (orchestrated via core-orchestrator with real-time SSE streaming back into the UI)
- 🔄 **Git integration:** Structured git operations via git2-rs. The git module exposes worktree create/list/remove as a first-class capability, but worktrees are not (yet) created automatically per task — Workers currently share the configured workspace
- 📊 **Cost tracking:** Real-time token spend per session with per-agent, per-task reporting
- 🛡️ **Workspace path-jail:** Every writing tool enforces that file paths and shell working directories stay inside the user's configured workspace
- 🧯 **Budget circuit breaker:** Per-task token / USD / turn caps, pre-emptively halting runaway loops and escalating to the Supervisor
- 🧪 **Build evidence gate:** Workers can't push tasks to QC review without a recent successful build on record
- 👥 **Headless QC dispatcher:** A fleet of short-lived reviewer subagents reviews completed work in parallel with a concurrency cap and a rolling cost guard
- 🧭 **Semantic task routing:** Supervisor learns which Worker performs best for which task type from historical outcomes
- 🩺 **MCP & sidecar health monitors:** Per-server liveness pings, TCP-alive checks, auto-reconnect, and a sidecar crash banner with one-click "Copy crash report"
- 🪦 **PID-based orphan detection:** Tasks abandoned by a crashed Worker are released within the next Supervisor poll (~2 min) instead of waiting 4 hours
- 📐 **Backend↔Frontend contract:** INTERFACE.md authored by the Supervisor and validated against both sides, with structured drift detection in diff analysis
- 🗺️ **Phase-based decomposition:** Epics broken into ordered phases with blockers, milestones, and a live Epic Phase View
- 🔎 **Diff risk scoring:** `analyze_diff` tool gives the QC agent a structured risk score, classification, summary, and review notes per PR

---

## Architecture

AutoDev is built on **Tauri v2** (Rust + React) and bundles two powerful Python sidecars as git submodules:

```
┌─────────────────────────────────────────────────────────────────┐
│                     AutoDev Desktop App                         │
├──────────────────────┬──────────────────────────────────────────┤
│   React/TS Frontend  │         Tauri v2 Rust Backend            │
│   (src/)             │         (src-tauri/)                     │
│                      │                                          │
│   ┌─────────────┐    │    ┌──────────────────────────────────┐  │
│   │ Zustand      │◄──invoke──►│ Tauri Commands              │  │
│   │ Store        │    │    │  ├── agent::*                    │  │
│   ├─────────────┤    │    │  ├── provider::*                 │  │
│   │ Tab Views    │    │    │  ├── vault::*                    │  │
│   │ ├── Chat     │    │    │  ├── settings::*                │  │
│   │ ├── AutoDev  │    │    │  └── sidecar::*                 │  │
│   │ ├── IDE      │    │    ├──────────────────────────────────┤  │
│   │ ├── Recipes  │    │    │ Core Modules                    │  │
│   │ ├── Logs     │    │    │  ├── agent/   (tool exec loop)  │  │
│   │ └── Settings │    │    │  ├── mcp/     (MCP client)       │  │
│   ├─────────────┤    │    │  ├── recipes/ (YAML + cron)     │  │
│   │ Components   │    │    │  ├── vault/  (OS keychain)      │  │
│   │ └── etc.     │    │    │  ├── git/    (git2-rs)          │  │
│   └─────────────┘    │    │  ├── github/ (Issues + PRs)     │  │
│                      │    │  └── autodev/(dispatch, locks)  │  │
│                      │    └──────────────────────────────────┘  │
│                      │                  │                        │
│                      │    ┌─────────────┴─────────────┐         │
│                      │    │     Sidecar Manager        │         │
│                      │    │  ┌─────────┐ ┌──────────┐ │         │
│                      │    │  │ Core    │ │ Memory   │ │         │
│                      │    │  │ Orch.   │ │ MCP Srv  │ │         │
│                      │    │  │ (Python)│ │ (Python) │ │         │
│                      │    │  └────┬────┘ └────┬─────┘ │         │
│                      │    │       │           │       │         │
│                      │    │  ┌────┴───────────┴────┐  │         │
│                      │    │  │       Redis          │  │         │
│                      │    │  │   (standalone bin)   │  │         │
│                      │    │  └─────────────────────┘  │         │
│                      │    └───────────────────────────┘         │
└──────────────────────┴──────────────────────────────────────────┘
```

### Dual-Mode Architecture

AutoDev operates in **TWO modes** with different execution paths:

#### Mode 1: PA (Personal Assistant) — Rust-Native Execution

**Used by:** Razi, team members, developers using the desktop app interactively

**Execution Path:**
```
User → Tauri (Rust) → Direct LLM API (Anthropic/OpenAI/etc.)
                    ↓
              Built-in tools (read_file, write_file, bash, git2-rs)
                    ↓
              External MCP servers — registered and health-checked, not yet called
                    ↓
              Response → UI
```

**Why NOT delegate to core-orchestrator:**
- **Lower latency** — no HTTP round-trip (100-500ms saved per turn)
- **Simpler deployment** — works even if Python sidecars fail to start
- **Settings UI** — test provider API keys without starting core-orchestrator
- **Headless CLI** — `airsctl -p "..."` runs standalone, no Python dependency
- **Offline-first** — PA mode + Ollama works without internet

**Limitations:** No A2A, no mem0 semantic memory, no 21 FSMs, no context scoring — intentionally kept simple.

#### Mode 2: Worker Agent — Orchestrated via core-orchestrator

**Used by:** AutoDev worker agents (backend, frontend, cleanup, etc.), Supervisor, QC agent running recipes

**Execution Path:**
```
Recipe trigger → Tauri → HTTP POST /api/v1/agent/invoke → core-orchestrator
                                                         ↓
                                                   LivingAgent (21 FSMs)
                                                         ↓
                                                   litellm provider
                                                         ↓
                                                   airs_mcp client → external MCPs
                                                         ↓
                                                   mem0 memory → Redis
                                                         ↓
                                                   A2A protocol
                                                         ↓
                                                   SSE stream → Tauri → UI
```

**Why delegate to core-orchestrator:**
- **A2A protocol** — supervisor ↔ workers coordination
- **mem0 semantic memory** — long-term context, retrieval-augmented generation
- **21 FSMs** — agent lifecycle, tool execution tracking, memory management, etc.
- **Context scoring** — relevance ranking, smart compaction
- **Feedback-driven synopsis** — conversation evolution
- Too complex to reimplement in Rust; Python ecosystem advantage

**Live visibility:** Mode 2 invocations stream tool calls, tool results, and intermediate model output back into the desktop app via SSE (`invoke_worker_streaming`), so the Live Monitor and Worker Requests panel show what the LivingAgent is doing in real time — not just whether it is "idle" or "working."

**Smart fallback:** A two-probe watchdog (HTTP `/health` + raw TCP probe) keeps the fleet on Mode 2 when the orchestrator is busy-but-alive and only declares it dead when both probes fail. Each recipe declares a `FallbackPolicy` (`Mode2Only` or `Mode1Capable`); recipes that depend on MCP/memory/A2A pause gracefully when Mode 2 is unreachable rather than running on a degraded engine.

### PA Mode Toggle

Within **Mode 1: PA**, the ChatHeader provides a dropdown to switch between three sub-modes:

| Mode | Enum Variant | Tool Registry | System Prompt Source | Use Case |
|------|---------------|---------------|---------------------|----------|
| 🔧 Fast Development | `PAMode::FastDevelopment` | All builtin_tools (read_file, write_file, edit_file, bash, grep, glob, git) + All MCP tools | `BASE_SYSTEM_PROMPT` | Razi coding directly, quick fixes |
| ✋ Review Mode | `PAMode::Review` | All builtin_tools + permission_check wrapper + All MCP tools | `REVIEW_SYSTEM_PROMPT` + caution instructions | Reviewing changes, cautious editing |
| 📋 Delegate Mode | `PAMode::Delegate` | Only `create_task` tool + **MCP tools (read-only)** | `DELEGATE_SYSTEM_PROMPT` | Workers are live, PA delegates work |

**Delegate Mode Interaction Flow:**
1. User: "Add /health endpoint to orchestrator"
2. PA creates task on **internal AutoDev board** (SQLite) via `create_task` tool
3. PA confirms: "Task #142 created on board. Supervisor will assign to Worker Backend on next poll."
4. **Supervisor recipe** (polls `~/.autodev/tasks.db` every 2 min): Sees task #142, assigns to Worker Backend
5. **Worker Backend recipe**: Executes via core-orchestrator, implements endpoint, opens PR on GitHub
6. **QC Agent recipe**: Reviews PR, updates task status to "qc-approved"
7. **Supervisor**: Sees QC approval, instructs worker to merge, closes task #142
8. PA (if monitoring AutoDev Panel): Shows task completion notification

### Sidecars (bundled as git submodules)

**Core Orchestrator** (`submodules/core-orchestrator/`):
- FastAPI-based agent runtime with LivingAgent (2425 lines)
- 21 finite state machines for lifecycle management
- MemoryCore with 3-tier memory (episodic/semantic/procedural)
- MCP client system for external MCP servers
- A2A protocol for agent-to-agent communication
- SSE streaming for real-time updates
- Runtime: Python 3.13+ / FastAPI / uvicorn on port 4000

**Memory MCP Server** (`submodules/memory-mcp-server/`):
- FastMCP-based server exposing 6 memory tools
- Multi-provider support (mem0, Redis, MongoDB, ChromaDB, Vertex AI)
- Circuit breaker pattern with automatic failover
- Runtime: Python 3.13+ / FastMCP / Starlette on port 5000

**Redis:**
- Standalone binary for warm-tier memory caching
- In-memory cache with persistence option
- Port: 6379

---

## Tech Stack

| Layer | Technology | Purpose |
|-------|-----------|---------|
| **Desktop Shell** | Tauri v2 (Rust) | Cross-platform native wrapper (~15MB base) |
| **Frontend** | React 19 + TypeScript + Vite 8 | Agent UI, chat, code editor, task board |
| **State Management** | Zustand | Global state (sessions, config, agent status) |
| **Styling** | Tailwind CSS 3 | AIRS design system (industrial-minimal: #0A0A0A background, #00FF88 accent) |
| **Backend** | Rust (Tauri commands) | Code editing tools, file I/O, command execution |
| **Provider Layer** | Anthropic, OpenAI, Gemini, Ollama, Azure | Multi-provider LLM abstraction |
| **MCP health** | reqwest | Liveness probes for external MCP servers (no client yet) |
| **Git Integration** | git2-rs (libgit2) | Structured git operations, worktrees |
| **Orchestrator** | Python 3.13 + FastAPI + litellm | Agent lifecycle, FSMs, A2A protocol |
| **Memory** | Python 3.13 + mem0 | Context storage, scoring, synopsis evolution |
| **Cache** | Redis 7+ | Warm-tier memory caching |
| **Keychain** | keyring crate | OS keychain integration for API keys |
| **Database** | SQLite (rusqlite) | Session persistence, task board, worker registry |

---

## Project Structure

```
autodev/
├── README.md                      # This file
├── package.json                   # Node.js dependencies (Tauri v2, React 19, Vite 8, Tailwind 3)
├── .gitmodules                    # Submodule config
├── .gitignore
│
├── src/                           # React frontend
│   ├── main.tsx                   # React root + Zustand provider
│   ├── App.tsx                    # Tab router + layout shell
│   ├── stores/                    # Zustand stores
│   │   ├── agentStore.ts          # Agent state: messages, status, streaming
│   │   ├── sessionStore.ts        # Session list, active session
│   │   ├── settingsStore.ts       # Provider config, theme, autodev settings
│   │   ├── costStore.ts           # Cost tracking state
│   │   ├── sidecarStore.ts        # Sidecar status
│   │   ├── autodevMdStore.ts      # AUTODEV.md instruction system
│   │   ├── taskStore.ts           # Task board state
│   │   ├── workerStore.ts         # Worker registry state
│   │   ├── workspaceStore.ts      # Workspace folder state
│   │   ├── modelConfigStore.ts    # Model configuration (temp, top-p, thinking)
│   │   ├── recipeStore.ts         # Recipe management
│   │   ├── gitStore.ts            # Git integration state
│   │   └── skillStore.ts          # Skills system
│   ├── tabs/                      # Top-level tab views
│   │   ├── Chat.tsx               # Main chat interface with session bar
│   │   ├── AutoDevPanel.tsx       # Agent roster + task board
│   │   ├── IDE.tsx                # File tree + code viewer + git panel
│   │   ├── Recipes.tsx            # Recipe browser + editor
│   │   ├── Logs.tsx               # Log viewer with filtering
│   │   └── Settings.tsx           # Provider + model router + AutoDev config
│   ├── components/                # Shared UI components
│   │   ├── layout/                # TitleBar, TabBar, StatusBar, Sidebar
│   │   ├── chat/                  # MessageBubble, ToolCallDisplay, InputArea, PermissionDialog
│   │   ├── settings/              # ProviderCard, ModelSelector, KeyInput, AutoDevConfig
│   │   ├── git/                   # GitPanel, DiffViewer, BranchManager, StashManager
│   │   ├── autodev/               # AutodevMdCard, AutodevMdEditor, WorkerRequestsWidget
│   │   ├── workers/               # WorkerTable, WorkerCreateModal
│   │   ├── recipes/               # RecipeEditor, RecipeCard
│   │   └── common/                # Button, Card, Badge, Spinner, Icon
│   ├── hooks/                     # Custom React hooks
│   │   ├── useTauriEvent.ts       # Listen to Tauri events
│   │   ├── useTauriInvoke.ts      # Invoke Tauri commands
│   │   └── useAgent.ts            # Agent interaction hook
│   ├── lib/                       # Utilities
│   │   ├── tauri.ts               # Tauri API wrappers
│   │   ├── format.ts              # Text formatting (markdown, code highlight)
│   │   └── cost.ts                # Cost calculation helpers
│   ├── types/                     # TypeScript type definitions
│   │   ├── agent.ts               # AgentStatus, Message, ToolCall types
│   │   ├── provider.ts            # Provider, Model types
│   │   ├── session.ts             # Session types
│   │   └── events.ts              # Event payload types
│   └── styles/
│       ├── globals.css            # Tailwind base + custom tokens
│       └── theme.ts               # Design system constants
│
├── src-tauri/                     # Rust backend
│   ├── Cargo.toml                 # Rust dependencies (Tauri v2, anyhow, tokio, rusqlite, git2, reqwest)
│   ├── tauri.conf.json            # Window config (1200×800, min 1000×700), dark theme, CSP
│   ├── build.rs                   # Tauri build script
│   └── src/
│       ├── main.rs                # Tauri entry point, app setup, plugin registration
│       ├── lib.rs                 # Re-exports all modules
│       ├── commands/              # #[tauri::command] functions (thin wrappers)
│       │   ├── mod.rs
│       │   ├── agent_commands.rs   # send_message, cancel, get_history
│       │   ├── provider_commands.rs # test_connection, list_providers, set_active
│       │   ├── settings_commands.rs # get_settings, update_settings
│       │   ├── vault_commands.rs    # store_key, get_key, delete_key
│       │   ├── sidecar_commands.rs  # start_sidecar, stop_sidecar, sidecar_status
│       │   ├── session_commands.rs  # list_sessions, get_session, delete_session
│       │   ├── task_commands.rs     # list_tasks, update_task_status, assign_task, create_task
│       │   ├── worker_commands.rs   # list_workers, create_worker, update_worker_status
│       │   ├── autodev_instructions_commands.rs # AUTODEV.md CRUD
│       │   ├── github_mcp_commands.rs # GitHub MCP config
│       │   ├── model_config_commands.rs # Model config CRUD
│       │   ├── recipe_commands.rs  # list_recipes, save_recipe, delete_recipe, generate_recipe_yaml
│       │   ├── git_commands.rs     # 29 git commands (clone, status, diff, commit, push, pull, worktree, etc.)
│       │   ├── log_commands.rs     # list_log_files, read_log_file
│       │   └── file_commands.rs    # pick_workspace_dir, @-mention file picker
│       ├── agent/                   # Core agent engine
│       │   ├── mod.rs
│       │   ├── engine.rs            # AgentEngine: the main agentic loop
│       │   ├── context.rs           # Conversation context builder (system prompt, AUTODEV.md, history)
│       │   ├── tool_registry.rs     # ToolRegistry: register, find, list tools
│       │   ├── tool_executor.rs     # Execute tools with permission checks, concurrency control
│       │   ├── tool_trait.rs        # The Tool trait definition
│       │   ├── builtin_tools/       # Built-in tool implementations
│       │   │   ├── mod.rs
│       │   │   ├── read_file.rs
│       │   │   ├── write_file.rs
│       │   │   ├── edit_file.rs
│       │   │   ├── bash.rs
│       │   │   ├── list_directory.rs
│       │   │   ├── glob.rs
│       │   │   ├── grep.rs
│       │   │   ├── create_task.rs
│       │   │   ├── list_tasks_tool.rs
│       │   │   ├── update_task_status_tool.rs
│       │   │   └── request_supervisor_action.rs
│       │   ├── permission.rs        # Permission system (ManualApproval, Autonomous, SmartApprove)
│       │   └── compaction.rs        # Conversation compaction when context fills
│       ├── provider/                # LLM provider abstraction (PA mode only)
│       │   ├── mod.rs
│       │   ├── traits.rs            # Provider trait: complete(), stream(), models()
│       │   ├── anthropic.rs         # Anthropic API client
│       │   ├── openai.rs            # OpenAI API client
│       │   ├── gemini.rs            # Google Gemini API client
│       │   ├── ollama.rs            # Ollama local API client
│       │   ├── azure.rs             # Azure OpenAI API client
│       │   ├── model_router.rs      # Route agent roles to specific models
│       │   └── streaming.rs         # Unified streaming response parser
│       ├── mcp/                     # MCP client (PA mode)
│       │   ├── mod.rs
│       │   └── health.rs            # Liveness probes for external MCP servers
│       ├── vault/                   # OS keychain integration
│       │   ├── mod.rs
│       │   └── keychain.rs          # Store/retrieve API keys securely
│       ├── session/                 # Session persistence
│       │   ├── mod.rs
│       │   ├── manager.rs           # SessionManager (CRUD, list)
│       │   ├── storage.rs           # SQLite storage backend
│       │   └── types.rs             # Session, Conversation, Message types
│       ├── recipes/                 # Recipe engine
│       │   ├── mod.rs
│       │   ├── parser.rs            # YAML recipe parser
│       │   ├── runner.rs            # Recipe execution engine
│       │   ├── scheduler.rs         # Cron scheduler for recurring recipes
│       │   └── defaults.rs          # Default system recipes (supervisor, workers)
│       ├── git/                     # Git integration
│       │   ├── mod.rs
│       │   ├── worktree.rs          # Worktree creation/removal/listing
│       │   ├── repo_manager.rs      # Multi-repo management
│       │   ├── diff.rs              # Diff computation for IDE tab + file history
│       │   ├── status.rs            # Working tree status
│       │   ├── branch.rs            # Branch operations
│       │   ├── remote.rs            # Push, pull, fetch operations
│       │   ├── commit.rs            # Commit operations
│       │   ├── history.rs           # Git log, blame
│       │   ├── stash.rs             # Stash operations
│       │   ├── checkpoint.rs        # File checkpointing per message (undo support)
│       │   └── types.rs             # GitRepo, GitStatus, GitDiff types
│       ├── db/                      # Internal databases
│       │   ├── mod.rs
│       │   ├── tasks.rs             # Task CRUD operations via rusqlite
│       │   ├── workers.rs           # Worker CRUD operations via rusqlite
│       │   ├── autodev_instructions.rs # AUTODEV.md SQLite storage
│       │   ├── model_configs.rs     # Model configuration storage
│       │   └── schema.rs            # SQLite schema definitions + migrations
│       ├── github/                  # GitHub integration
│       │   ├── mod.rs
│       │   ├── issues.rs            # GitHub Issues API
│       │   └── labels.rs            # Label management
│       ├── autodev/                 # AutoDev-specific logic
│       │   ├── mod.rs
│       │   ├── dispatch.rs          # Dispatch signal handling
│       │   ├── lockfiles.rs         # Task lock file management
│       │   └── heartbeat.rs         # Agent heartbeat reporting
│       ├── sidecar/                 # Sidecar process management
│       │   ├── mod.rs
│       │   ├── manager.rs           # Start/stop/monitor sidecars (core-orchestrator, memory-mcp)
│       │   ├── client.rs            # OrchestratorClient for health checks and invocation
│       │   └── health.rs            # Health check endpoints
│       ├── skills/                  # Skills system
│       │   ├── mod.rs
│       │   ├── types.rs
│       │   ├── loader.rs            # Skill loading from ~/.autodev/skills/
│       │   ├── engine.rs            # Skill execution with parameter substitution
│       │   └── registry.rs          # Skill registry with category organization
│       ├── cost/                    # Cost tracking
│       │   ├── mod.rs
│       │   └── tracker.rs           # Token counting + cost calculation per session
│       ├── state/                   # Application state
│       │   ├── mod.rs
│       │   └── app_state.rs         # Shared mutable state (Arc<Mutex<AppState>>)
│       ├── events.rs                # Tauri event emission helpers
│       ├── errors.rs                # Unified error types (thiserror)
│       └── logging.rs               # tracing subscriber setup
│
├── docs/                          # Documentation
│   ├── autodev-vision/
│   │   └── autodev-master-plan.md   # Living product spec (single source of truth)
│   ├── development/
│   │   └── DEVELOPMENT_PLAN.md     # Master development plan with phases
│   ├── submodules-management/
│   │   └── SUBMODULE_SETUP_GUIDE.md # Git submodule workflow guide
│   └── ENVIRONMENT_SETUP.md       # Installation guide
│
├── submodules/                    # Git submodules (Python projects)
│   ├── core-orchestrator/         # Agent orchestration engine (tracked via git submodule)
│   │   ├── src/airs/agent/living_agent.py  # LivingAgent (2425 lines)
│   │   ├── src/airs/memory/memory_core.py  # MemoryCore (2654 lines)
│   │   ├── src/airs/state/        # 21 finite state machines
│   │   ├── src/airs/bridges/     # Unified tool handler, MCP bridge, memory compiler
│   │   ├── src/airs/mcp/          # MCP client system
│   │   └── mcp_config.yaml         # MCP server configuration
│   └── memory-mcp-server/         # Memory system (tracked via git submodule)
│       ├── src/mcp_server/server.py  # FastMCP-based server (1093 lines)
│       ├── src/memory_providers/     # Multi-provider memory manager
│       └── main.py
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
- **Node.js 20+** & npm (pinned to v22.1.0 for compatibility)
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
# Clone the repo with submodules
git clone --recurse-submodules https://github.com/AIRS-Foundry/autodev.git
cd autodev

# If you cloned without --recurse-submodules, initialize manually:
git submodule update --init --recursive

# Install npm dependencies
npm install

# Run development server
npm run tauri dev
```

**Note:** The sidecars (core-orchestrator and memory-mcp-server) will auto-start on app launch. No manual Python setup required for development.

---

## Development

### Running Locally

```powershell
# Terminal 1: Start Tauri dev server (auto-starts sidecars)
npm run tauri dev

# Terminal 2 (optional): Test core-orchestrator independently
cd submodules/core-orchestrator
uv venv && .venv\Scripts\activate && uv sync
python src/main.py
```

**Important:** Do NOT run `uv sync` in the main autodev repo. AutoDev is a Tauri app (Rust + Node.js), not a Python project. Only run `uv sync` inside submodules when developing them locally.

### Building for Production

```powershell
# Build all sidecars (Windows PowerShell)
.\build\bundle-core.ps1
.\build\bundle-memory-mcp.ps1
.\build\fetch-redis.ps1

# Build Tauri app
npm run tauri build
```

The build process will:
1. Bundle Python submodules into standalone executables using PyInstaller
2. Download Redis binary
3. Build the Tauri app with all sidecars embedded
4. Produce platform-specific installers (.msi for Windows, .AppImage for Linux, .dmg for macOS)

### Working with Submodules

Core Orchestrator and Memory MCP Server are tracked as **git submodules** (commit references, not copied code). To edit:

1. Navigate into the submodule:
   ```powershell
   cd submodules/core-orchestrator
   ```

2. Work as a normal git repo:
   ```powershell
   git checkout main
   git pull origin main
   git checkout -b fix/your-feature
   # Make changes
   git add .
   git commit -m "fix: your changes"
   git push origin fix/your-feature
   ```

3. Update the pointer in the main autodev repo:
   ```powershell
   cd ../..  # back to autodev root
   git add submodules/core-orchestrator
   git commit -m "chore: bump core-orchestrator (fix included)"
   ```

See [`docs/submodules-management/SUBMODULE_SETUP_GUIDE.md`](docs/submodules-management/SUBMODULE_SETUP_GUIDE.md) for detailed submodule workflow instructions.

### Updating Submodules

```bash
# Update all submodules to their latest main branch
git submodule update --remote --merge

# Or update a specific submodule
cd submodules/core-orchestrator
git pull origin main
cd ../..
git add submodules/core-orchestrator
git commit -m "chore: update core-orchestrator to latest"
```

---

## Documentation

| Document | Description |
|----------|-------------|
| [`docs/autodev-vision/autodev-master-plan.md`](docs/autodev-vision/autodev-master-plan.md) | **Living product spec** - single source of truth for vision, features, architecture, UI/UX |
| [`docs/development/DEVELOPMENT_PLAN.md`](docs/development/DEVELOPMENT_PLAN.md) | **Master development plan** - phased build plan, module breakdown, coding conventions |
| [`docs/submodules-management/SUBMODULE_SETUP_GUIDE.md`](docs/submodules-management/SUBMODULE_SETUP_GUIDE.md) | Git submodule workflow guide - how to work with core-orchestrator and memory-mcp-server |
| [`docs/ENVIRONMENT_SETUP.md`](docs/ENVIRONMENT_SETUP.md) | Complete environment setup guide |

---

## Key Concepts

### Dual-Mode Execution

AutoDev operates in two distinct modes:

**PA Mode (Personal Assistant) - Rust-Native:**
- Used by Razi, team members, developers using the desktop app interactively
- Direct LLM API calls (no HTTP round-trip to Python)
- Built-in tools execute natively in Rust for speed
- Lower latency (~100-500ms saved per turn)
- Works offline with Ollama
- Simpler deployment (works even if Python sidecars fail)

**Worker Mode - Orchestrated via Core Orchestrator:**
- Used by AutoDev worker agents (backend, frontend, cleanup, etc.), Supervisor, QC agent
- Delegates to Python sidecars via HTTP (FastAPI endpoints)
- Full LivingAgent capabilities (21 FSMs, A2A protocol, mem0 semantic memory)
- Context scoring, smart compaction, feedback-driven synopsis
- Complex agent orchestration requiring Python ecosystem advantages

### PA Mode Toggle

Within PA mode, three sub-modes control tool availability:

| Mode | Tools Available | System Prompt | Use Case |
|------|----------------|---------------|----------|
| 🔧 Fast Development | All builtin_tools + MCP tools | BASE_SYSTEM_PROMPT | Direct coding, quick fixes |
| ✋ Review Mode | All tools + permission prompts | REVIEW_SYSTEM_PROMPT | Cautious editing, code review |
| 📋 Delegate Mode | Only `create_task` tool | DELEGATE_SYSTEM_PROMPT | Workers are live, PA delegates work |

### Internal Task Board (Not GitHub Issues)

AutoDev uses an internal SQLite task board (`~/.autodev/tasks.db`) for task assignment:
- **Why:** Single app = no external coordination needed. Faster, offline support, full control.
- **Flow:** PA creates task → Supervisor assigns to workers → Workers execute → QC reviews → Supervisor merges
- **GitHub still used:** Workers create PRs on GitHub repos, but task assignment happens internally

### Submodules

Core Orchestrator and Memory MCP Server are tracked as **git submodules** (commit references, not copied code):
- Submodules are separate git repos with their own history
- To edit: work inside `submodules/{repo}` as a normal git repo
- Commit and push changes to the submodule's remote
- Update the pointer in the main autodev repo
- **Do NOT run `uv sync` in the main repo** - AutoDev is a Tauri app (Rust + Node.js), not a Python project
- Only run `uv sync` inside submodules when developing them locally

See [`docs/submodules-management/SUBMODULE_SETUP_GUIDE.md`](docs/submodules-management/SUBMODULE_SETUP_GUIDE.md) for detailed submodule workflow.

### Tool Routing

Three execution paths in AutoDev:
- **Fast path** (Tauri Rust): File operations, grep, command execution - used in PA mode
- **Stateful path** (Core Orchestrator): Agent FSMs, A2A protocol, MCP client pool - used in Worker mode
- **Memory path** (Memory MCP): Context storage, scoring, retrieval - used in Worker mode

### AUTODEV.md System

AUTODEV.md is a first-class UI feature - created, edited, and stored entirely within the AutoDev app:
- SQLite-backed storage (`~/.autodev/config.db`)
- Two scopes: Global (applies to all repos) and Per-Repo (specific to a workspace)
- Editable from UI with Monaco editor (deferred - currently textarea)
- Auto-injected into system prompt based on current workspace
- No manual file editing required

### Git Integration

AutoDev uses **git2-rs** (libgit2 Rust bindings) for structured git operations:
- No shell-out to git binary (type-safe, no parsing)
- Worktree create / list / remove exposed as Tauri commands and a git module (not yet auto-wired per task; Workers share the configured workspace today)
- File checkpointing for undo support (50 snapshots per file)
- Commit attribution with `Generated-by: AutoDev/<role>` trailers
- Pull is fast-forward only (safer for agents)
- Integrated into IDE tab as togglable Git panel

### Fleet→Supervisor Communication

Workers can communicate with the Supervisor via the `request_supervisor_action` tool:
- **decompose_audit:** Worker provides full suggested task payloads, Supervisor creates them atomically
- **reassign:** Worker requests task reassignment to different worker
- **needs_info:** Worker asks human PA a question (also fired automatically when the budget circuit breaker trips)
- **report_issue:** Worker reports blocking issue or error
- **request_review:** Worker requests QC review of completed work

### Workspace Path-Jail

Every file-writing builtin tool (`write_file`, `edit_file`, `append_file`) and the `bash` tool's working directory pass through a single jail helper that canonicalizes both the target path and the configured workspace and refuses anything that resolves outside it. The bash tool additionally scans the command string for `>`, `>>`, and `tee` redirect targets as a best-effort defense. Refusals come back as a structured error message designed for the LLM to read and self-correct from. On Windows the comparison is case-insensitive and matches at component boundaries so siblings like `C:\foobar` do not falsely satisfy a `C:\foo` workspace.

### Build Evidence Gate

Workers cannot transition a task to `qc_review` unless they have produced a recent, successful build. A session-keyed ring buffer stamps every build command that exits cleanly (the `EXIT:0` sentinel); the status-update tool consults that ring and refuses the transition if no fresh evidence exists, returning the Worker a structured message telling it to run its build and retry. Failed builds get a dedicated `build_failed` task status so they are not silently retried.

### Budget Circuit Breaker

Every task can carry a budget — a maximum number of tokens, a maximum USD cost, and a maximum number of agent turns. The agent engine checks the budget at the *start* of every loop turn and halts immediately when any limit is reached, rather than after the next expensive LLM call. A halted task is moved to `needs_info` with an auto-filed `WorkerRequest` against the Supervisor, not marked failed — the Supervisor decides to raise the budget, split the work, or reassign. Defaults are configurable in Settings (per-task tokens / USD / warn-at-%).

### Headless QC Dispatcher

QC is fanned out across short-lived reviewer subagents instead of a single QC agent serializing every review. A lightweight `qc-dispatcher` recipe spawns one reviewer per task in `qc_review` (concurrency capped at 6, rolling $5/hr cost guard), each reviewer reads the diff, runs its checks, writes its verdict, and terminates. Tasks carry `under_review_by` (which reviewer owns them) and `task_qc_cost_usd` (cumulative review cost) columns. A stranded-reviewer reconciler clears dangling references when a host process dies.

### Backend↔Frontend Interface Contract

The Supervisor authors `INTERFACE.md` via the `write_interface_spec` tool, capturing the agreed API surface (endpoints, request/response shapes, error codes) between backend and frontend workers. The diff analyzer compares every API-surface change against this contract and flags drift; the validation pipeline's Tier 2.6 fails the frontend's build when it calls an endpoint the backend does not expose. Tier 2.5 is a pure-Rust import↔export matcher that catches missing symbols before the slow compiler does.

---

## Features

### From Claude Code

- CLAUDE.md/AGENTS.md support (backward compatible)
- Headless `-p` mode for CLI automation (airsctl)
- Subagents + Agent Teams (Phase 4)
- Skills, Hooks, Plugins (Phase 3)
- An MCP client, so external MCP servers can actually be called (today they are only registered and health-checked)
- Git worktree primitive exposed via git2-rs (per-task auto-wiring deferred — capability available to recipes that opt in)
- Context compaction when approaching token limit
- Plan mode for complex multi-step tasks

### From Goose

- Model-agnostic provider layer (Anthropic, OpenAI, Gemini, Ollama, Azure)
- Recipes (YAML workflows) + cron scheduler (Phase 2)
- Per-session cost audit + export
- Approval modes (Autonomous, Manual, Smart Approve, Chat-only)
- `.goosehints` directory-scoped context
- Custom distributions
- ACP (Agent Client Protocol) for IDE plugins (deferred)

### AutoDev-Native

**Fleet & Task Board**
- **AutoDev Panel:** Live task board with multi-column kanban (Ready, In Progress, QC Review, Rejected, Done Today)
- **Agent Roster:** Dynamic table showing all agents with status dots, current tasks, elapsed time
- **Worker Management:** Create custom workers, bind recipes, auto-launch on app startup
- **Headless QC Dispatcher:** Per-task ephemeral reviewer subagents (concurrency cap = 6, rolling $5/hr cost guard, `under_review_by` + `task_qc_cost_usd` columns, stranded-reviewer reconciler, QC ROI panel)
- **Fleet Request Queue:** Workers can request supervisor action (decompose, reassign, report issues, needs-info, request-review)
- **Rejected-Task Feedback Loop:** `rejection_reason` + `retry_count` columns; Supervisor re-dispatches rejected tasks back to the originating Worker
- **Semantic Task Routing:** Worker affinity learning from `routing_history` (static pattern fast-path + historical scoring) + Worker Affinity Stats panel
- **Phase-Based Decomposition:** Epics → ordered phases with blockers, milestones, automatic unblock-on-completion, dedicated Epic Phase View + Epics tab
- **Live Monitor:** Mode 2 tool calls streamed via SSE in real time; lazy slug registration so events never get dropped

**Safety & Reliability**
- **Workspace Path-Jail:** `write_file` / `edit_file` / `append_file` / `bash` (cwd + redirect targets) all enforce that paths stay inside the configured workspace
- **Build Evidence Gate:** Session-keyed ring of recent `EXIT:0` builds; Workers cannot transition tasks to `qc_review` without recent successful build evidence
- **Budget Circuit Breaker:** Per-task token / USD / turn caps with pre-emptive halt and `needs_info` escalation; per-task `BudgetBar` widget; daily budget summary
- **PID-Based Orphan Detection:** Workers record their OS PID + hostname when claiming; orphans released within ~2 min via the Supervisor's poll
- **Backend↔Frontend Interface Contract:** `INTERFACE.md` authored via `write_interface_spec`; Tier 2.6 API-contract validation; structured drift detection in `analyze_diff`
- **Tier 2.5 Export Cross-Reference:** Pure-Rust import↔export matcher catches missing/renamed exports before the slow compiler does
- **Fleet Smoke Recipe + `e2e_probe`:** Scheduled end-to-end check that boots the app and confirms it serves an expected response

**Visibility & Diagnostics**
- **Sidecar Crash Banner:** Captured stderr tail + "Copy crash report" markdown bundle
- **Workspace Mismatch Banner:** Startup check comparing app cwd vs. configured workspace
- **Stray Files Banner:** Startup scan for worker-generated artifacts in unexpected places
- **MCP Connection Health Monitor:** 30-second ping loop, Connected / Degraded / Disconnected / Reconnecting states, auto-reconnect with exponential backoff, status-bar `⚠️ MCP` indicator
- **TCP-Alive Health Probe:** Watchdog uses HTTP `/health` + raw TCP together so a busy-but-alive orchestrator does not trigger premature fallback
- **Structured Error Taxonomy:** Every recoverable failure tagged with a `kind=` field (nine canonical kinds documented in `LOG_ERROR_KINDS.md`)
- **Diff Risk Scoring (`analyze_diff`):** Two-pass (structural Rust + single LLM classification) producing risk score, classification, summary, review notes, and suggested checks; rendered as a `DiffAnalysisCard` on QC tasks
- **Logs Tab:** Centralized log viewer with filtering, search, export; daily rotation with 14-day retention (`AUTODEV_LOG_RETENTION_DAYS` override)

**Editing & UX**
- **AUTODEV.md System:** SQLite-backed instruction system, editable from UI, no manual file editing
- **PA Mode Toggle:** Three sub-modes (Fast Development, Review, Delegate) with tool filtering
- **Key Vault:** Encrypted API key storage via OS keychain (keyring crate)
- **Model Router:** Route agent roles to specific models (Settings → Model Router table)
- **Cost Tracking:** Real-time token spend per session with per-agent, per-task reporting
- **Context Window Meter:** Visual progress bar showing token usage with color-coded thresholds
- **Model Configuration:** Per-model settings for temperature, top-p, reasoning effort, thinking budget
- **GitHub Integration:** PAT storage, GitHub MCP config, PR creation via workers
- **Workspace Folder:** Native OS folder picker, @-mention path resolution
- **Git Panel:** Full git integration with Changes, History, Branches, Stash, Undo tabs
- **File Checkpointing:** Auto-checkpoint before writes, undo/redo support, file history
- **Sidecar Management:** Start Python sidecars, health checks, log routing. AutoDev does **not** start Redis — it probes for one and runs without it (the warm memory tier is simply disabled).
- **Browser-Test Skill:** Playwright-driven visual regression and E2E checks invoked by the QC agent on frontend PRs

---

## Development Phases

AutoDev follows a phased development approach:

### Phase 1 ✅ COMPLETE
**App Shell + Provider + Chat**
- Tauri app scaffold with 5-tab layout
- Provider layer (Anthropic, OpenAI, Gemini, Ollama)
- Chat tab with tool execution and streaming
- Session persistence with multiple sessions UI
- Rich tool display (syntax highlighting, grep results, glob tables)
- AUTODEV.md SQLite storage system
- Cost tracking with context window meter
- PA Mode Toggle (Fast Development, Review, Delegate)

### Phase 2 ✅ COMPLETE
**Recipes + AutoDev Panel + GitHub**
- Recipe engine with YAML parsing and cron scheduler
- AutoDev Panel with agent roster and task board
- Internal task board (SQLite at `~/.autodev/tasks.db`)
- Worker management system (create, bind recipes, auto-launch)
- GitHub PAT storage and GitHub MCP config
- Model configuration (temperature, top-p, reasoning effort, thinking budget)
- Workspace folder selection with @-mention resolution
- Recipe auto-generation via AI
- Fleet→Supervisor communication system (worker request queue)
- Recipe agent sandbox isolation
- Native task board tools (list_tasks, update_task_status)
- Logs tab with filtering and export
- Sidecar integration (auto-start, health checks, log routing)

### Phase 3 ✅ COMPLETE
**Skills + Hooks + IDE + Git + MCP Server**
- Skills system with YAML frontmatter and parameter substitution
- Hooks system for event-driven extensibility
- IDE tab with file tree and code viewer
- Git integration via git2-rs (29 commands)
- File checkpointing with undo support
- Git panel with Changes, History, Branches, Stash, Undo tabs
- AIRS MCP server (exposes canonical Rust tools)
- Sidecar management improvements

### Phase 4 ✅ COMPLETE
**Subagents + Model Router + Dispatch**
- Subagent creation and management (`spawn_subagent`, `spawn_parallel_subagents`, ephemeral lifecycle with create→invoke→delete)
- Enhanced model router with complex routing rules
- Push-based dispatch (workers kicked the moment a task lands; no waiting for the next cron tick)
- AutoDev native mode (boot-start recipe, FSEvents dispatch watch)

### Phase 5 ✅ COMPLETE
**Fleet Hardening + Operational Safety**
- Workspace path-jail (write/edit/append/bash + redirect heuristic)
- Build evidence gate (`EXIT:0` ring), rejected-task feedback loop, `retry_count`
- Headless QC dispatcher: ephemeral reviewer subagents, concurrency cap, $5/hr cost guard, `under_review_by` + `task_qc_cost_usd`, stranded reviewer reconciler, QC ROI panel
- Per-recipe `FallbackPolicy` (`Mode2Only` / `Mode1Capable`), worker idle pre-flight, TCP-alive health probe
- Mode 2 SSE streaming into Live Monitor (`invoke_worker_streaming`), lazy slug registration
- Sidecar crash banner with "Copy crash report", Workspace mismatch banner, Stray files banner
- Structured `kind=` log taxonomy + `LOG_ERROR_KINDS.md`, daily log rotation with 14-day retention, cached runtime config

### Phase 6 ✅ COMPLETE
**Ruflo-Inspired Capabilities**
- Semantic task routing with worker affinity learning (`routing_history`, `suggest_worker` MCP tool, Worker Affinity Stats panel)
- PID-based orphan detection (replaces 4-hour stale-lock with ~2-minute reconciliation via `sysinfo`)
- Diff risk scoring for QC agent (`analyze_diff` MCP tool, two-pass structural + LLM classifier, `DiffAnalysisCard`)
- Budget circuit breaker (per-task token/USD/turn caps, pre-emptive halt, `needs_info` escalation, `BudgetBar`)
- Phase-based task decomposition (epics → phases → blockers → milestones, Epic Phase View, dedicated Epics tab)
- MCP connection health monitor (30s ping loop, auto-reconnect with backoff, status-bar indicator)
- Browser-test skill (Playwright-driven QC for frontend PRs, install detection, capability flag)

### Phase 7 ✅ COMPLETE
**Backend↔Frontend Contract & E2E**
- `INTERFACE.md` contract authored by Supervisor via `write_interface_spec`
- Tier 2.5 export cross-reference + Tier 2.6 API-contract validation, interface drift detection in `analyze_diff`
- Fleet smoke recipe + `e2e_probe` builtin tool (timeout-wrapped GET + expected-text match)

### Phase 8 📋 PLANNED
**Plugin + CLI + Installers**
- AIRS Plugin (one-click worker PC configuration)
- Headless CLI (`airsctl`)
- Installers for Windows/Mac/Linux
- IT staff runbook

---

## Current Status

**Phase:** Phases 1–7 Complete ✅
**Next:** Phase 8 — Plugin, CLI, and platform installers

**Completed:**
- ✅ Phase 1: App shell, Provider layer, Chat tab with tool execution, PA Mode Toggle, AUTODEV.md system
- ✅ Phase 2: Recipes engine, AutoDev Panel, Worker management, GitHub integration, Sidecar integration
- ✅ Phase 3: Skills system, IDE tab, Git integration, File checkpointing, AIRS MCP server
- ✅ Phase 4: Subagent lifecycle, enhanced model router, push-based dispatch, AutoDev native mode
- ✅ Phase 5: Path-jail, build evidence gate, QC dispatcher, fallback policy, crash/mismatch/stray banners, log retention + error taxonomy
- ✅ Phase 6: Ruflo-inspired features (semantic routing, PID orphans, diff risk, budget breaker, phase decomposition, MCP health monitor, browser-test skill)
- ✅ Phase 7: INTERFACE.md contract, fleet smoke recipe + `e2e_probe`, tier 2.5/2.6 validation

**Environment verified:**
- ✅ Python 3.13+
- ✅ uv package manager
- ✅ Rust stable (via rustup)
- ✅ Node.js v22.1.0 / npm 10.7.0
- ✅ Git 2.45.0+
- ✅ Submodules initialized (core-orchestrator, memory-mcp-server)
- ✅ npm packages installed (React 19, Vite 8, TypeScript, Tailwind 3, Tauri CLI)

---

## Contributing

This is an internal AIRS Foundry project. For questions or contributions, contact:

**Programming Lead:** Razi  
**Email:** razi@airsfoundry.com

### Coding Conventions

**Rust (src-tauri):**
- Use `anyhow` for error handling
- Async with Tokio
- Tauri commands exposed via `#[tauri::command]`
- All file I/O through the `agent::filesystem` module (not raw std::fs)
- Log with `tracing` crate

**TypeScript/React (src/):**
- Functional components only
- State management: Zustand for global state (sessions, config, agent status)
- API calls to Tauri backend via `@tauri-apps/api/core` invoke()
- No useEffect for data fetching — use Zustand actions
- Tailwind for all styling, no CSS-in-JS

### Design System

AutoDev uses an industrial-minimal design language:
- Background: #0A0A0A
- Surface: #111111
- Border: #222222
- Accent: #00FF88 (active states, CTAs, success)
- Danger: #FF4444 (errors, rejections)
- Warning: #FFAA00 (in-progress, pending)
- Text primary: #F0F0F0
- Text secondary: #666666
- Font: IBM Plex Mono (code, numbers, agent names) + IBM Plex Sans (prose)
- Border radius: 2px
- Spacing: 8px base unit

---

## License

Proprietary - AIRS Foundry © 2026

---

## FAQ

**Q: Do I need Docker?**  
A: No. Redis is optional and externally managed — AutoDev probes for one and runs without it. No Docker required for development or production.

**Q: Can I use different LLM providers?**  
A: Yes. AutoDev supports Anthropic, OpenAI, Gemini, Ollama, Azure OpenAI, and Google Bedrock via the provider abstraction layer.

**Q: Is this compatible with my existing CLAUDE.md files?**  
A: Yes. AutoDev has an AUTODEV.md system (SQLite-backed) that is separate from CLAUDE.md. For migration, you can import CLAUDE.md content into AUTODEV.md via the UI.

**Q: How do I update the submodules?**  
A: `git submodule update --remote --merge` to update all submodules to their latest main branch. See [`docs/submodules-management/SUBMODULE_SETUP_GUIDE.md`](docs/submodules-management/SUBMODULE_SETUP_GUIDE.md) for detailed instructions.

**Q: Why Node v22.1.0?**  
A: Vite 8 prefers v22.12+, but v22.1.0 works fine. Warnings can be ignored. The version is pinned for compatibility.

**Q: Do I need to run `uv sync` in the main repo?**  
A: No. AutoDev is a Tauri app (Rust + Node.js), not a Python project. Only run `uv sync` inside submodules when developing them locally.

**Q: How do the sidecars work?**  
A: The sidecars (core-orchestrator and memory-mcp-server) are Python projects tracked as git submodules. They auto-start on app launch via the SidecarManager, which spawns them as subprocesses and monitors their health. Logs are routed to the AutoDev Logs tab.

**Q: What's the difference between PA mode and Worker mode?**  
A: PA mode uses Rust-native execution for speed and lower latency (direct LLM API calls). Worker mode delegates to Python sidecars for complex agent orchestration with full LivingAgent capabilities (21 FSMs, mem0 semantic memory, A2A protocol).

**Q: How does the internal task board work?**  
A: AutoDev uses an internal SQLite task board (`~/.autodev/tasks.db`) for task assignment instead of GitHub Issues. This enables faster, offline-capable coordination. Workers still create PRs on GitHub repos, but task assignment happens internally via the Supervisor recipe.

**Q: How do I create a custom worker?**  
A: Go to Settings → Workers tab, click "Create Worker", fill in the worker details (name, type, primary repos, model, polling interval), and bind a recipe. The worker will auto-launch on app startup.

**Q: What is the Fleet→Supervisor communication system?**  
A: Workers can communicate with the Supervisor via the `request_supervisor_action` tool to request task decomposition, reassignment, report issues, ask questions, or request QC review. This enables intelligent task escalation without manual intervention.

**Q: How does file checkpointing work?**  
A: AutoDev automatically checkpoints files before writes (50 snapshots per file). Checkpoints are stored at `~/.autodev/checkpoints/<session_id>/` and indexed in SQLite. You can undo to any checkpoint via the Git panel's Undo tab.

**Q: What's the difference between Recipes and Skills?**  
A: Recipes are YAML workflows for fleet orchestration (workers, supervisor, QC agents) with cron scheduling. Skills are markdown files with YAML frontmatter for reusable prompt workflows that can be invoked via slash commands or programmatically by agents.

**Q: What stops a worker from writing files outside my project folder?**
A: The workspace path-jail. Every file-writing builtin tool (`write_file`, `edit_file`, `append_file`) and the `bash` tool's working directory pass through a canonicalize-then-`starts_with` check against the configured workspace, with a redirect-target heuristic on shell commands. A misbehaving agent that tries to write to `C:\Windows\foo.txt` gets a structured refusal it can self-correct from. A Workspace Mismatch banner also fires at startup when the app's cwd differs from the configured workspace.

**Q: Can a runaway agent burn through my whole API budget?**
A: No. Every task carries an optional token / USD / turn budget; the agent engine checks the budget at the *start* of every loop turn and halts immediately when any limit is hit. A halted task is moved to `needs_info` and a `WorkerRequest` is filed against the Supervisor automatically. Fleet-wide defaults live in Settings → Default Worker Budget.

**Q: How does QC scale when several workers finish at once?**
A: A lightweight `qc-dispatcher` recipe spawns one ephemeral reviewer subagent per task in `qc_review` (concurrency capped at 6 by default, with a rolling $5/hr cost guard). Each reviewer reads the diff, runs its checks, writes a verdict, and terminates. A `qc:decision` event fires a visible toast on every approval or rejection.

**Q: What happens if a Worker process crashes mid-task?**
A: PID-based orphan detection. Workers record their OS PID + hostname when they claim a task. The orphan detector runs on the Supervisor's poll cadence (~2 minutes), checks each claimed task's PID via the `sysinfo` crate, and releases any task whose host process is no longer alive — back to the `ready` queue, with an Orphan Alert toast on the AutoDev panel.

**Q: What does the orchestrator falling over actually look like in the UI?**
A: A two-probe watchdog (HTTP `/health` + raw TCP) distinguishes "busy but alive" from "actually dead." When it does declare the orchestrator dead and the breaker opens, a Sidecar Crash banner appears with the captured stderr tail and a one-click "Copy crash report" button. Each recipe's `FallbackPolicy` decides whether it can keep running on Mode 1 or should pause gracefully as `fallback_idle`.

**Q: How does the Supervisor decide who gets a task?**
A: Two layers. First a small set of hand-written static patterns (keywords like "fastapi", "react", "docker" → worker-backend/-frontend/-platform); if those are ambiguous or disagree with history, the Supervisor calls the `suggest_worker` MCP tool which scores Workers by historical success rate × duration ranking for that task type (`routing_history` table). A Worker Affinity Stats panel exposes the per-Worker per-type success rates.

---

**For detailed setup instructions, see [`docs/ENVIRONMENT_SETUP.md`](docs/ENVIRONMENT_SETUP.md)**

**For the complete product specification, see [`docs/autodev-vision/autodev-master-plan.md`](docs/autodev-vision/autodev-master-plan.md)**

**For the master development plan, see [`docs/development/DEVELOPMENT_PLAN.md`](docs/development/DEVELOPMENT_PLAN.md)**
