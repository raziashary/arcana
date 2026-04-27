# AutoDev — Master Development Plan

> **Living Document** — Updated as phases complete. Code is the source of truth; this document guides planning and architecture decisions.
>
> **Parent Document**: [autodev-master-plan.md](../autodev-vision/autodev-master-plan.md)
>
> **Reference Codebases** (read-only, not in git):
> - `claude-code-source-main/` — Anthropic Claude Code (TypeScript, Ink CLI)
> - `goose-main/` — Block Goose (Rust workspace + Electron desktop)

---

## Table of Contents

1. [Current State Assessment](#1-current-state-assessment)
2. [Submodule Capabilities Audit](#2-submodule-capabilities-audit)
3. [UI Mockup Specification](#3-ui-mockup-specification)
4. [Architecture Overview](#4-architecture-overview)
5. [Sidecar Communication Protocol](#5-sidecar-communication-protocol)
6. [Workspace & Git Strategy](#6-workspace--git-strategy)
7. [Lessons from Reference Codebases](#7-lessons-from-reference-codebases)
8. [Module Breakdown & Dependency Graph](#8-module-breakdown--dependency-graph)
9. [Phase 1 — App Shell + Provider + Chat](#9-phase-1--app-shell--provider--chat)
10. [Phase 2 — Recipes + AutoDev Panel + GitHub](#10-phase-2--recipes--autodev-panel--github)
10.5. [Phase 2.5 — Recipe Agent Sandbox + Native Task Tools + Logs Tab](#105-phase-25--recipe-agent-sandbox--native-task-tools--logs-tab)
11. [Phase 3 — IDE + Skills + Hooks + MCP Server](#11-phase-3--ide--skills--hooks--mcp-server)
11.1. [Phase 3.3.1 — Startup Feedback + Agent ID Persistence](#phase-331--startup-feedback--agent-id-persistence)
12. [Phase 4 — Subagents + Model Router + Dispatch](#12-phase-4--subagents--model-router--dispatch)
13. [Phase 5 — Plugin + CLI + Installers](#13-phase-5--plugin--cli--installers)
14. [State Machine & Event System Design](#14-state-machine--event-system-design)
15. [Coding Conventions](#15-coding-conventions)
16. [File & Directory Scaffold](#16-file--directory-scaffold)
17. [Testing Strategy](#17-testing-strategy)
18. [Risk Log & Open Questions](#18-risk-log--open-questions)

---

## 1. Current State Assessment

### What Exists

| Item | Status | Notes |
|---|---|---|
| `package.json` | ✅ Created | Tauri v2, React 19, Vite 8, Tailwind 3 |
| `tailwind.config.js` | ✅ Created | Empty content array — needs paths |
| `postcss.config.js` | ✅ Created | autoprefixer + tailwindcss |
| `.gitmodules` | ✅ Configured | Branch tracking for core-orchestrator + memory-mcp-server |
| `submodules/core-orchestrator/` | ✅ Cloned | Python FastAPI sidecar (588 items) |
| `submodules/memory-mcp-server/` | ✅ Cloned | Python MCP server (28 items) |
| `docs/` | ✅ Structured | ENVIRONMENT_SETUP, autodev-vision, submodules-management |
| `README.md` | ✅ Written | Comprehensive project overview |
| `node_modules/` | ✅ Installed | npm dependencies present |

### What Does NOT Exist Yet

| Item | Priority | Phase |
|---|---|---|
| `src-tauri/` (Rust backend) | **Critical** | Phase 1 |
| `src/` (React frontend) | **Critical** | Phase 1 |
| `vite.config.ts` | **Critical** | Phase 1 |
| `tsconfig.json` | **Critical** | Phase 1 |
| `index.html` | **Critical** | Phase 1 |
| `src-tauri/Cargo.toml` | **Critical** | Phase 1 |
| `src-tauri/tauri.conf.json` | **Critical** | Phase 1 |
| `build/` scripts | Medium | Phase 5 |
| `sidecars/` binaries | Medium | Phase 3 |
| Zustand stores | **Critical** | Phase 1 |
| Tauri commands | **Critical** | Phase 1 |

---

## 2. Submodule Capabilities Audit

> **Why this matters**: The core-orchestrator and memory-mcp-server submodules are not empty shells — they are production-grade Python systems with sophisticated architectures. AutoDev's Tauri shell must integrate with these as sidecars, not re-implement their capabilities in Rust.

### 2.1 Core Orchestrator (`submodules/core-orchestrator/`)

**Runtime**: Python 3.13+ / FastAPI / uvicorn on port 4000
**Entry point**: `src/main.py` → `api_gateway.api.main.app`
**Key dependency**: `litellm`, `mcp[cli]`, `a2a-sdk`, `pydantic 2.x`, `sse-starlette`

#### LivingAgent (`src/airs/agent/living_agent.py` — 2425 lines)

The central agent class. Memory is not a feature bolted onto agents — memory IS the agent.

**Initialization pipeline** (6-step bridge init):
1. **UnifiedToolHandler** — Routes tool calls to correct compiler (Memory / Adobe / MCP / Internal)
2. **Memory Compiler** — Discovers and registers memory MCP tools
3. **Adobe Compiler** — Discovers and registers Adobe creative tools
4. **MCP Bridge** — Discovers and registers generic MCP tools from any server
5. **Internal Tool Bridge** — Opt-in internal tools (e.g., `web_search__internal`)
6. **MemoryAwareToolExecutor** — Wraps unified handler with memory awareness

**Message processing pipeline** (`process_message` → `_process_message_internal`):
1. **RECALL** — Wake up with consciousness via `memory.recall_with_budget()` (token-aware, intent-detected)
2. **THINK** — Build conscious prompt with episodic/semantic/procedural memories
3. **EXECUTE** — LLM completion with tool execution loop
4. **REMEMBER** — Store interaction across all memory types automatically
5. **FINALIZE** — Emit thinking_end, agent_stop events

**Key capabilities already implemented**:
- Intent detection: timeline, verbatim, hybrid, recap modes
- Tool narration sanitization (removes bracketed narration when no tools called)
- Multimodal support (images, files via presigned URLs)
- Clarification accumulation during execution
- TaskExecutionFSM per-task lifecycle (CREATED → PLANNING → EXECUTING → REVIEWING → COMPLETED/FAILED)
- Event emission at every lifecycle boundary (AGENT_INVOCATION_REQUESTED → STARTED → COMPLETED/FAILED)

#### MemoryCore (`src/airs/memory/memory_core.py` — 2654 lines)

The consciousness of a living agent. Three memory types with granular caching:

| Memory Type | Purpose | Cache TTL | Example |
|---|---|---|---|
| **Episodic** | Recent interactions | Short (conversations change rapidly) | "I talked to Alice yesterday about pizza" |
| **Semantic** | Facts about the world | Medium (facts change slowly) | "Alice loves pizza and works in AI" |
| **Procedural** | Accumulated wisdom | Long (learnings accumulate slowly) | "Firebase has rate limits, check quota first" |

**Advanced memory subsystems**:
- **Token Budget** (`token_validator.py`) — Auto-degrade context when exceeding model limits
- **Compression Engine** (`compression_engine.py`) — Adaptive L0→L1→L2→L3 compression by age
- **Staleness Tracker** (`staleness_tracker.py`) — Temperature scoring for memory freshness
- **Query Expander** (`query_expander.py`) — Sub-query generation for better recall
- **BM25/Hybrid Search** (`bm25_search.py`) — Lexical + semantic fusion
- **Context Scorer** (`context_scorer.py`) — 8-factor utility scoring (R,A,C,D,P,F,X,L)
- **Event Journal** (`event_journal.py`) — Append-only WAL with BLAKE3 hashing
- **Contradiction Detector** (`contradiction_detector.py`) — Scope-aware conflict checking
- **Continuity Packet** (`continuity_packet.py`) — ≤2KB session state for instant warmup
- **Predictive Stager** (`predictive_stager.py`) — Pre-fetch memories likely needed next
- **Paradigm Integration** (`paradigm_integration.py`) — Multi-signal relevance scoring
- **Memory Metrics** (`memory_metrics.py`) — Real-time tracking + alerting

**Cache tiers**: Hot (in-memory OrderedDict) → Warm (Redis) → Cold (MCP/mem0)

#### MCP Client System (`airs_mcp/`)

| File | Class | Purpose |
|---|---|---|
| `client.py` | `MCPClient` | Universal MCP client with session lifecycle, tool calls, concurrency control, cleanup |
| `client_factory.py` | `MCPClientFactory` | Creates clients based on config (HTTP vs stdio transport auto-detection) |
| `client_manager.py` | `MCPClientManager` | Manages multiple client instances, capability mappings |

**Transport**: HTTP (streamable) for both memory-mcp-server and Adobe MCP. Config in `mcp_config.yaml`.

#### Bridges (`src/airs/bridges/`)

| Bridge | Lines | Purpose |
|---|---|---|
| `unified_tool_handler.py` | 202 | Routes tool calls to Memory/Adobe/MCP/Internal handlers by name matching |
| `mcp_bridge.py` | 816 | Generic MCP tool discovery + LiteLLM function calling integration |
| `memory_compiler.py` | 770 | Memory-specific MCP tool discovery and call handling |
| `adobe_compiler.py` | 131K | Adobe creative tools (Firefly, Photoshop) via MCP |
| `agent_bridge.py` | 53K | Agent-to-agent delegation bridge |
| `delegation_handler.py` | 22K | A2A task delegation lifecycle |

#### State Machines (`src/airs/state/` — 21 FSMs)

All FSMs emit events automatically on transitions and track correlation IDs:

| FSM | States | Tracks |
|---|---|---|
| `AgentLifecycleFSM` | CREATED → REGISTERED → STARTING → ACTIVE → STOPPING → STOPPED | Agent process lifecycle |
| `SessionLifecycleFSM` | CREATED → WARMING → ACTIVE → IDLE → CLOSING → CLOSED | Session warmup/cleanup |
| `TaskExecutionFSM` | CREATED → PLANNING → EXECUTING → REVIEWING → COMPLETED/FAILED | Per-task lifecycle |
| `ToolExecutionFSM` | PENDING → EXECUTING → COMPLETED/FAILED/CANCELLED | Tool call lifecycle |
| `MemoryStoreFSM` | IDLE → STORING → STORED/FAILED | Memory write operations |
| `MemoryRecallFSM` | IDLE → RECALLING → RECALLED/FAILED | Memory read operations |
| `MCPServerFSM` | DISCONNECTED → CONNECTING → CONNECTED → ERROR | MCP server connection |
| `MCPToolDiscoveryFSM` | IDLE → DISCOVERING → DISCOVERED/FAILED | Tool discovery from MCP |
| `A2AServerFSM` | CREATED → STARTING → RUNNING → STOPPING → STOPPED | A2A server lifecycle |
| `A2ARequestFSM` | CREATED → SENDING → ACKNOWLEDGED → COMPLETED/FAILED | A2A request lifecycle |
| `WorkflowExecutionFSM` | CREATED → RUNNING → COMPLETED/FAILED | Multi-step workflow |
| `CacheTierFSM` | COLD → WARMING → HOT → STALE → EVICTED | Cache state per entry |
| `SystemStartupFSM` | INITIALIZING → STARTING → READY/FAILED | System boot sequence |
| `SystemShutdownFSM` | RUNNING → SHUTTING_DOWN → SHUTDOWN | Graceful shutdown |
| `AgentSubprocessFSM` | CREATED → STARTING → RUNNING → STOPPING → STOPPED | Subprocess management |
| `AgentConfigFSM` | UNLOADED → LOADING → LOADED → RELOADING → ERROR | Config lifecycle |
| `SessionRuntimeFSM` | IDLE → PROCESSING → STREAMING → IDLE | Session runtime state |
| `PromptReloadFSM` | LOADED → RELOADING → LOADED/FAILED | Prompt hot-reload |
| `FileOperationsFSM` | IDLE → READING/WRITING → COMPLETED/FAILED | File I/O tracking |
| `AuthenticationFSM` | UNAUTHENTICATED → AUTHENTICATING → AUTHENTICATED → EXPIRED | Auth lifecycle |

#### Event System v2 (`src/airs/events/v2/`)

| File | Purpose |
|---|---|
| `schema.py` (97K) | Complete event type definitions (`EventType` enum with 100+ event types) |
| `unified_bus.py` (26K) | Central event bus with pub/sub, async dispatch, handler registration |
| `transports.py` (9K) | SSE, WebSocket, Redis pub/sub transport backends |
| `event_store.py` (12K) | Persistent event storage for replay/audit |
| `event_state_machine.py` (14K) | FSM-driven event emission on state transitions |
| `helpers.py` (23K) | `emit_event_safe()`, correlation ID management, event builders |

#### Streaming v2 (`src/airs/streaming/`)

Zero-copy, CPU-light streaming infrastructure:
- **StreamState** — FSM for streaming phases (thinking → content → tool_call → done)
- **NativeThinkingDetector** — Extracts `reasoning_content` / `thinking_blocks`
- **XMLThinkingDetector** — Zero-copy DFA with ring buffer (4KB) for tag detection
- **BufferManager** — Adaptive cadence token buffering
- **StreamBroker** — Orchestration layer tying detectors + buffer + SSE

Design: <250ms to first signal, linear O(n) scan, chunk-driven (no predictive gating).

#### Intelligence Layer (`src/airs/intelligence/`)

- **UnifiedDecisionEngine** — Central decision maker (retry, HITL, completion)
- **MemoryAwareFailureDetector** — Learns from failure patterns via procedural memory
- **MemoryAwareHITLOrchestrator** — Human-in-the-loop with memory context

#### Agent Factory & Registry

- **AgentFactory** (`src/airs/agent/factory.py`) — Creates LivingAgent instances from YAML configs (`config/agents/*.yaml`)
- **AgentRegistry** (`src/airs/agent/registry.py`) — Thread-safe singleton with Pydantic models, A2A AgentCard integration
- **Pydantic Models** (`src/airs/models/`) — Single source of truth: AgentData, AgentMetadata, SessionData, MetricsData, UserPreferences, etc.

#### MCP Server Configuration (`mcp_config.yaml`)

Pre-configured servers:
- `mcp-adobe` — Adobe creative tools (Firefly, Photoshop) via HTTP transport
- `mcp-memory` — Memory MCP Server via HTTP transport (`http://${MCP_MEMORY_HOST}:${MCP_MEMORY_PORT}/memorymcpserver`)

Connection pool: max 10, 3 per server, 300s idle timeout. Cache enabled, registry discovery enabled.

### 2.2 Memory MCP Server (`submodules/memory-mcp-server/`)

**Runtime**: Python 3.13+ / FastMCP / Starlette / uvicorn
**Entry point**: `main.py` → `MemoryMCPServer` on configurable port
**Transport**: Streamable HTTP at `/memorymcpserver` endpoint

#### Server (`src/mcp_server/server.py` — 1093 lines)

FastMCP-based server exposing 6 memory tools + session management:

| MCP Tool | Parameters | Returns |
|---|---|---|
| `store_memory` | content, user_id, session_id, agent_id?, topic?, verdict?, tags?, infer? | `{success, memory_id, memories_added, provider_used}` |
| `search_memories` | query, user_id, session_id?, limit, offset, filters?, min_score?, session_scope_ratio | `[{id, content, metadata, score, created_at, ...}]` |
| `get_memory` | memory_id, user_id?, session_id? | `{memory_id, content, metadata, score, ...}` |
| `update_memory` | memory_id, content, session_id?, topic?, tags?, metadata? | `{success, result, memory_id}` |
| `delete_memory` | memory_id, user_id?, session_id? | `{success, result}` |
| `list_memories` | user_id, session_id?, limit, offset, filters? | `[{id, content, metadata, ...}]` |

Session tools (commented out but available): `create_session`, `get_session`, `merge_session`, `split_session`

#### Universal Memory Manager (`src/memory_providers/manager.py` — 855 lines)

Multi-provider support with circuit breakers and automatic failover:

| Provider Type | Backend | Status |
|---|---|---|
| `mem0` | Mem0 SDK (vector store + graph) | Primary |
| `redis` | Redis with vector similarity | Available |
| `mongodb` | MongoDB Atlas Vector Search | Available |
| `chroma` | ChromaDB embeddings | Available |
| `vertex` | Google Vertex AI Vector Search | Available |
| `custom` | User-defined adapter | Available |

**Resilience patterns**: Circuit breaker (5 failures → open, 60s recovery), automatic failover to next provider in priority list, structured logging via structlog.

#### Base Types (`src/memory_providers/base.py`)

Pydantic models for all memory operations:
- `MemoryResult` — Standardized return with id, content, metadata, score, timestamps, user/session/agent/topic
- `MemoryQuery` — Search params with session_scope_ratio and global fallback
- `MemoryAddRequest` — Store with messages array, user/session/agent, topic, tags, infer flag
- `MemoryUpdateRequest`, `SessionInfo`, `SessionMergeRequest`, `SessionSplitRequest`

### 2.3 Implications for AutoDev Architecture

**What the sidecars provide** (DO NOT re-implement in Rust):
- Agent consciousness and memory system (episodic, semantic, procedural)
- MCP client/server communication
- Multi-provider memory storage with failover
- A2A agent-to-agent communication
- Streaming with thinking detection
- 21 state machines for lifecycle management
- Event bus with 100+ event types
- Intelligence layer (retry, HITL, failure detection)

**What AutoDev Tauri must build**:
- Sidecar lifecycle management (start/stop/health-check Python processes)
- MCP client in Rust to talk to core-orchestrator's MCP tools (OR HTTP proxy to FastAPI)
- Native tool execution (file I/O, bash, git) — faster than routing through Python
- Frontend UI (React/TS) consuming events from both Rust backend and Python sidecars
- OS keychain integration (vault)
- Session persistence (SQLite for frontend state, distinct from memory-mcp-server's storage)
- Recipe engine (YAML parsing, cron scheduling)
- GitHub integration (Issues API)
- Desktop-specific features (window management, tray icon, notifications)

**Communication model**: Tauri Rust backend ↔ Python sidecars via HTTP (FastAPI endpoints) + MCP (streamable HTTP transport). SSE for real-time streaming from core-orchestrator to frontend.

---

## 3. UI Mockup Specification

> **Source**: `docs/autodev-vision/autodevmockup.html` — Static HTML mockup defining the exact visual design and component hierarchy.

### 3.1 Global Layout

```
┌──────────────────────────────────────────────────────────────────────┐
│ TitleBar: [● ● ●]  AutoDev — v1.0.0         [ _ ] [ □ ] [ × ]    │
├──────────────────────────────────────────────────────────────────────┤
│ TabBar: [Chat] [AutoDev] [IDE] [Recipes] [Settings]  │model│ $cost │
├──────────────────────────────────────────────┬───────────────────────┤
│                                              │ Sidebar (176px)       │
│   Tab Content Area                           │  Context:             │
│   (flex: 1, scrollable)                      │    ● CLAUDE.md        │
│                                              │    ● .goosehints      │
│                                              │    ○ user-prefs       │
│                                              │  MCP Active:          │
│                                              │    ● github-mcp       │
│                                              │    ● slack-mcp        │
│                                              │    ○ sentry-mcp       │
│                                              │  Skills:              │
│                                              │    ● /review-pr       │
│                                              │    ● /run-tests       │
│                                              │    ● /claim-issue     │
│                                              │    ● /open-pr         │
│                                              │  Hooks:               │
│                                              │    ● post-write:lint  │
│                                              │    ● pre-bash:guard   │
│                                              │    ● session:telemetry│
│                                              │  Session:             │
│                                              │    12,450 tokens      │
│                                              │    $0.04 this session │
│                                              │    ⊕ /clear context   │
└──────────────────────────────────────────────┴───────────────────────┘
```

### 3.2 Tab: Chat

**Components**:
- **ChatHeader** — Mode indicator with PA Mode Toggle dropdown:
  - **🔧 Fast Development**: PA has full builtin_tools (read_file, write_file, edit_file, bash, grep, glob) for direct file editing. Uses standard system prompt.
  - **✋ Review Mode**: PA has tools but asks permission before any write operation. Uses standard system prompt + caution mode.
  - **📋 Delegate Mode**: PA has **zero file tools**, only `create_task` tool. Creates tasks on internal board for Supervisor to assign. Uses delegate-focused system prompt variant.
  - Also shows: MCP servers count, Skills count, CLAUDE.md status, current PA mode with colored indicator
- **MessageList** — Scrollable area with alternating message types:
  - `msg-user` — Right-aligned, green-tinted background (`#00FF8812`), green border
  - `msg-agent` — Left-aligned, dark surface background (`#111`), gray border
  - `tool-block` — Left-aligned, darkest background (`#0D0D0D`), shows tool name (blue `#4477FF`), args (gray), result (green)
- **InputBar** — Text input with send button (green accent)
- **InputMeta** — Below input: "Attach · /skill · @agent · #recipe" | "Tokens: 12,450 · $0.04"

**Interaction flow - Fast Development Mode**:
1. User sends natural language request
2. PA executes tools directly (read_file, edit_file, bash) to complete task
3. PA reports completion with file changes summary

**Interaction flow - Delegate Mode** (matches `old-multiple-fleet.md` architecture):
1. User sends natural language request (e.g., "Add /health endpoint to orchestrator")
2. PA creates task on **internal AutoDev board** via `create_task` tool:
   - Title: "Add GET /agents/health endpoint"
   - Priority: p2-medium, Type: feature, Status: ready
   - Acceptance criteria: detailed spec
   - Stored in local SQLite database (`~/.autodev/tasks.db`)
3. PA confirms: "Task #142 created on board. Supervisor will assign to Worker Backend on next poll cycle."
4. **Supervisor recipe** (polls every 2 min): Detects new task #142, determines it's atomic, assigns to Worker Backend recipe with recipe payload
5. **Worker Backend recipe**: Executes via core-orchestrator, implements endpoint, opens PR on GitHub, updates task status to "in-review"
6. **QC Agent recipe**: Reviews PR, posts QC report, updates task status to "qc-approved" or "qc-rejected"
7. **Supervisor**: Sees QC approval, instructs Worker Backend to merge PR, closes task #142
8. PA (if still monitoring): Shows task completion notification in AutoDev Panel

**Key difference from old multi-PC system**:
- **Old**: PA → GitHub Issues (external board) → Supervisor PC polls GitHub → Worker PCs poll GitHub
- **New**: PA → Internal SQLite board → Supervisor recipe polls internal DB → Worker recipes execute
- **Why**: Single app = no need for external coordination. Faster, more control, better offline support.

### 3.3 Tab: AutoDev

**Components**:
- **AgentRoster** — Dynamic table showing all agents (system-defined + user-created):
  - **System Agents** (locked, cannot delete):
    - SUPERVISOR (always ONE instance)
    - QC AGENT (always ONE instance)
  - **Pre-Defined Workers** (can customize):
    - WORKER-BACKEND, WORKER-FRONTEND, WORKER-PLATFORM, WORKER-MCP, WORKER-CLEANUP
  - **Custom Workers** (user-created via Settings → Workers panel):
    - WORKER-DOCS, WORKER-TESTING, etc. (appears dynamically)
  - Each row shows:
    - Status dot: 🟢 active (polling), 🟡 working (executing task), 🔵 idle (created but not started), 🔴 error
    - Agent name (e.g., "WORKER-BACKEND")
    - Current task description (e.g., "Issue #142 — /agents/health endpoint")
    - Elapsed time (MM:SS format)
    - Actions: Start/Stop/Pause/Delete (system agents cannot be deleted)
- **TaskBoard** — 4-column kanban:
  - **Ready** — Tasks assigned but not started (shows issue #, priority, title, assigned worker)
  - **In Progress** — Active tasks with progress bar (percentage fill)
  - **QC Review** — PRs under QC agent review (yellow status)
  - **Done Today** — Completed tasks with "✓ merged" green indicator
- **MetricsRow** — 4 metric cards:
  - Cost Today (green, e.g., "$2.14")
  - Tokens (e.g., "1.2M")
  - Closed (e.g., "5")
  - QC Pass Rate (yellow if <100%, e.g., "80%")

### 3.4 Tab: IDE

**Layout**: Split pane (file tree 180px left | code viewer right)

**FileTree** (left):
- Collapsible directory structure with folder/file icons
- Green highlight for repo root and active files
- File tree shows core-orchestrator structure (indicating worker is editing code)

**CodeViewer** (right):
- File path header with change count ("3 changes")
- Syntax-highlighted code with line numbers
- Diff highlighting: green left-border + green tint for added/modified lines
- Status bar: pytest results (✅/❌), mypy status, git branch name

### 3.5 Tab: Recipes

**Layout**: Split pane (recipe list 220px left | recipe detail right)

**RecipeList** (left):
- Cards with: recipe name (mono), description, cron schedule
- Active recipe highlighted with green border
- Examples: `worker-backend-poll`, `supervisor-dispatch`, `qc-review-cycle`, `create-task-from-spec`

**RecipeDetail** (right):
- Recipe filename header
- YAML viewer with syntax highlighting:
  ```yaml
  version: "1.0.0"
  title: Worker Backend — Polling Loop
  schedule: "*/2 * * * *"
  permission_mode: autonomous
  extensions:
    - github-mcp
    - developer
  parameters:
    - name: repo
      default: "core-orchestrator"
  ```
- Action buttons: "▶ Run Now", "Schedule", "Edit YAML"

### 3.6 Tab: Settings

**Sections**:

**Providers Table**:
| Column | Content |
|---|---|
| PROVIDER | Anthropic, OpenAI, Ollama |
| API KEY | Masked key or "[Add key...]" |
| MODEL | Active model name or "—" |
| STATUS | "● Active" (green) or "○ Off" (gray) |

**Model Router Table**:
| Column | Content |
|---|---|
| AGENT ROLE | Supervisor, Worker Backend, Worker Cleanup |
| MODEL | claude-opus-4-6, claude-sonnet-4-6, claude-haiku-4-5 |
| REASON | Complex routing logic, Balanced quality/cost, Simple refactors cheapest |

**AIRS AutoDev Config**:
- GitHub Org: "AIRS-Foundry"
- Task Board Repo: "airs-task-board"
- Agent Identity: "Personal Assistant (PA)" (green accent)
- GitHub PAT: masked with expiry warning ("expires 89d" in yellow)

### 3.7 Sidebar (Persistent)

Always visible regardless of active tab. Shows:
- **Context files**: AUTODEV.md, .autodevhints, user-prefs (with active/inactive dots) — ALL editable from UI, stored in SQLite, synced to ~/.autodev/
- **MCP Active**: github-mcp, slack-mcp, sentry-mcp (with connection status dots)
- **Skills**: /review-pr, /run-tests, /claim-issue, /open-pr (slash commands)
- **Hooks**: post-write:lint, pre-bash:guard, session:telemetry
- **Session info**: Token count, cost, /clear context action

### 3.8 Design System (Extracted from Mockup CSS)

| Token | CSS Value | Usage |
|---|---|---|
| `--bg-app` | `#0A0A0A` | App background |
| `--bg-surface` | `#0D0D0D` | Titlebar, tabbar, sidebar, panels |
| `--bg-card` | `#111111` | Message bubbles, cards, inputs |
| `--border-primary` | `#1a1a1a` | Most borders (subtle) |
| `--border-secondary` | `#222222` | Heavier borders (less common) |
| `--accent` | `#00FF88` | Active states, CTAs, success |
| `--accent-muted` | `#00FF8812` | User message bg, hover states |
| `--accent-border` | `#00FF8825` | User message borders |
| `--danger` | `#FF4444` | Errors, agent error state |
| `--warning` | `#FFAA00` | In-progress, working state, expiry warnings |
| `--info` | `#4477FF` | Tool names, keywords, links |
| `--text-primary` | `#888888` | Primary text (not pure white — intentional) |
| `--text-secondary` | `#555555` | Secondary text, sidebar items |
| `--text-muted` | `#444444` | Labels, column headers |
| `--text-dim` | `#333333` | Inactive items, timestamps |
| `--font-mono` | `'IBM Plex Mono', monospace` | Code, agent names, tool calls, metrics, labels |
| `--font-sans` | `'IBM Plex Sans', sans-serif` | Prose, message text |
| `--radius` | `2px` | All border radius (sharp, industrial) |
| `--spacing` | `8px` | Base unit (used as multiples: 4, 6, 8, 10, 12, 14, 16) |

**Typography scale** (from mockup):
- 9px — Metric labels (uppercase)
- 10px — Section headers, badges, cost, timestamps, meta text
- 11px — Sidebar items, roster rows, task cards, recipe items, settings rows
- 12px — Tab labels, code text, send button
- 13px — Chat messages, input text
- 16px — Metric values (numbers)

---

## 4. Architecture Overview

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
│   │ └── Settings │    │    │  ├── agent/   (tool exec loop)  │  │
│   ├─────────────┤    │    │  ├── mcp/     (MCP client)       │  │
│   │ Components   │    │    │  ├── recipes/ (YAML + cron)     │  │
│   │ ├── Message  │    │    │  ├── vault/  (OS keychain)      │  │
│   │ ├── ToolCall │    │    │  ├── git/    (git2-rs)          │  │
│   │ ├── CostBar  │    │    │  ├── github/ (Issues + PRs)     │  │
│   │ └── etc.     │    │    │  └── autodev/(dispatch, locks)  │  │
│   └─────────────┘    │    └──────────────────────────────────┘  │
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

### 4.1 Dual-Mode Architecture

> **Critical design pattern**: AutoDev operates in TWO modes with different execution paths. This explains why Rust implements provider abstraction + MCP client even though core-orchestrator has these capabilities.

#### Mode 1: PA (Personal Assistant) — Rust-Native Execution

**Used by:** Razi, team members, developers using the desktop app interactively

**Execution Path:**
```
User → Tauri (Rust) → Direct LLM API (Anthropic/OpenAI/etc.)
                    ↓
              Built-in tools (read_file, write_file, bash, git2-rs)
                    ↓
              MCP clients (GitHub MCP, Slack MCP) via rmcp crate
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

#### PA Mode Toggle (Phase 2 Enhancement)

Within **Mode 1: PA**, the ChatHeader provides a dropdown to switch between three sub-modes that control tool availability:

```rust
// In agent/engine.rs - PA mode with sub-mode handling
match session.mode {
    SessionMode::PersonalAssistant => {
        let provider = get_provider(&config.active_provider)?;
        
        // Filter tools based on PA sub-mode
        let available_tools = match session.pa_sub_mode {
            PAMode::FastDevelopment => registry.get_all_tools(),
            PAMode::Review => registry.get_all_tools(), // With permission checks
            PAMode::Delegate => vec![
                github_create_issue_tool,
                airs_dispatch_signal_tool
            ], // Zero file tools
        };
        
        let response = provider.stream_with_tools(request, available_tools).await?;
        // ... execute tools locally via agent::builtin_tools
    }
    SessionMode::WorkerAgent => {
        // Worker mode: delegate to core-orchestrator
        let body = serde_json::to_string(&request)?;
        let response = reqwest::Client::new()
            .post("http://localhost:4000/api/v1/agent/invoke")
            .body(body)
            .send().await?;
        // ... proxy SSE stream from core-orchestrator to UI
    }
}
```

**PA Sub-Mode Details:**

| Mode | Enum Variant | Tool Registry | System Prompt Source | Use Case |
|------|---------------|---------------|---------------------|----------|
| 🔧 Fast Development | `PAMode::FastDevelopment` | All builtin_tools (read_file, write_file, edit_file, bash, grep, glob, git) + All MCP tools | `BASE_SYSTEM_PROMPT` in `context.rs` | Razi coding directly, quick fixes |
| ✋ Review Mode | `PAMode::Review` | All builtin_tools + permission_check wrapper + All MCP tools | `BASE_SYSTEM_PROMPT` + caution instructions | Reviewing changes, cautious editing |
| 📋 Delegate Mode | `PAMode::Delegate` | Only `create_task` tool + **MCP tools (read-only)** | `DELEGATE_SYSTEM_PROMPT` in `context.rs` | Workers are live, PA delegates work |

**Delegate Mode Interaction Flow** (matches `old-multiple-fleet.md` org chart):
1. User: "Add /health endpoint to orchestrator"
2. PA creates task on **internal AutoDev board** (SQLite) via `create_task` tool:
   ```rust
   create_task({
     title: "Add GET /agents/health endpoint",
     priority: "p2-medium",
     type: "feature",
     acceptance_criteria: "...",
     status: "ready"
   })
   ```
3. PA confirms: "Task #142 created on board. Supervisor will assign to Worker Backend on next poll."
4. **Supervisor recipe** (polls `~/.autodev/tasks.db` every 2 min): Sees task #142, determines it's atomic, assigns to Worker Backend recipe
5. **Worker Backend recipe**: Executes via core-orchestrator, implements endpoint, opens PR on GitHub
6. **QC Agent recipe**: Reviews PR, updates task status to "qc-approved"
7. **Supervisor**: Sees QC approval, instructs worker to merge, closes task #142
8. PA (if monitoring AutoDev Panel): Shows task completion notification

**Internal Board Storage:**
- **Database**: SQLite at `~/.autodev/tasks.db`
- **Schema**: tasks table with comprehensive fields:
  ```sql
  CREATE TABLE tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    description TEXT,
    priority TEXT NOT NULL CHECK(priority IN ('p0-critical', 'p1-high', 'p2-medium', 'p3-low')),
    type TEXT NOT NULL CHECK(type IN ('epic', 'feature', 'bug', 'refactor', 'docs', 'infra')),
    status TEXT NOT NULL CHECK(status IN ('ready', 'assigned', 'in-progress', 'in-review', 'qc-approved', 'qc-rejected', 'blocked', 'completed')),
    acceptance_criteria TEXT,
    assigned_worker TEXT,           -- References workers.slug (e.g., 'worker-backend')
    created_by TEXT DEFAULT 'PA',   -- Who created this task (PA or Supervisor)
    repo TEXT,                       -- Primary repo for this task (e.g., 'core-orchestrator')
    labels TEXT,                     -- JSON array of labels for filtering
    parent_task_id INTEGER,          -- For epic → subtask hierarchy
    estimated_tokens INTEGER,        -- Estimated token cost
    actual_tokens INTEGER,           -- Actual token cost after completion
    github_pr_url TEXT,              -- Link to PR if created
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME,
    FOREIGN KEY (parent_task_id) REFERENCES tasks(id)
  );
  ```
- **Access**: Rust code in `src-tauri/src/db/tasks.rs` with `rusqlite` crate
- **UI**: AutoDev Panel TaskBoard component reads from DB via Tauri commands

This prevents accidental file edits when workers are live and processing the task queue.

**System Prompt Switching Implementation:**
- `context.rs` defines three system prompt constants:
  - `BASE_SYSTEM_PROMPT` — Full tool descriptions, encourages direct file editing
  - `REVIEW_SYSTEM_PROMPT` — Adds permission-checking instructions, emphasizes caution
  - `DELEGATE_SYSTEM_PROMPT` — Removes all file tool descriptions, focuses on task decomposition and structured task creation
- Settings panel allows viewing/editing each variant
- PA mode selection in ChatHeader dropdown triggers system prompt reload
- Tool registry filters tools based on mode before passing to LLM

**Result:** No code duplication — each mode uses the right layer of abstraction for its complexity needs.

### 4.2 Data Flow (PA Mode)

```
User Input → React UI → invoke("agent_send_message", {...})
           → Rust Tauri Command
           → Build conversation context (system prompt + history + CLAUDE.md)
           → Stream to LLM Provider (Anthropic/OpenAI/Gemini/Ollama)
           → Parse streaming chunks → emit events to frontend
           → If tool_use block detected:
               → Permission check (ManualApproval / Autonomous)
               → Execute tool (read_file / write_file / bash / mcp_call)
               → Append tool result to conversation
               → Continue LLM loop
           → Final response → yield to UI → store in session
```

### 4.3 IPC Contract: Tauri Events

The frontend and backend communicate via two mechanisms:
1. **`invoke()`** — Request/response for commands (send message, save settings, etc.)
2. **`listen()`** — Event streams for real-time updates (token streaming, tool progress, agent status)

Key events:

| Event Name | Direction | Payload |
|---|---|---|
| `agent:stream-token` | Backend → Frontend | `{ text: string, done: bool }` |
| `agent:tool-call-start` | Backend → Frontend | `{ id, name, args }` |
| `agent:tool-call-result` | Backend → Frontend | `{ id, result, is_error }` |
| `agent:status-change` | Backend → Frontend | `{ status: AgentStatus }` |
| `session:cost-update` | Backend → Frontend | `{ tokens_in, tokens_out, cost_usd }` |
| `sidecar:status` | Backend → Frontend | `{ name, status: "starting"│"running"│"stopped"│"error" }` |

---

## 5. Sidecar Communication Protocol

> **Design decision**: Tauri communicates with Python sidecars via HTTP (FastAPI REST + SSE), not stdio. This enables independent development, testing, and hot-reload of sidecars.

### 5.1 Communication Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                        AutoDev Desktop                                │
│                                                                       │
│  ┌─────────────┐    invoke()    ┌──────────────────────────────────┐  │
│  │ React/TS    │◄─────────────►│  Rust Backend                     │  │
│  │ Frontend    │  Tauri events  │                                   │  │
│  │             │◄──────────────│  ┌────────────────────────────┐   │  │
│  └─────────────┘               │  │ SidecarManager             │   │  │
│        │                        │  │  ├─ spawn_process()        │   │  │
│        │ SSE subscribe          │  │  ├─ health_check()         │   │  │
│        │ (direct to Python)     │  │  ├─ restart_on_crash()     │   │  │
│        │                        │  │  └─ graceful_shutdown()     │   │  │
│        ▼                        │  └──────┬─────────┬───────────┘   │  │
│  ┌─────────────┐               │         │         │                │  │
│  │ SSE Client  │               │    HTTP │    HTTP │                │  │
│  │ (EventSource│               │         │         │                │  │
│  │  in browser)│               │         │         │                │  │
│  └──────┬──────┘               └─────────┼─────────┼────────────────┘  │
│         │                                │         │                    │
│         ▼                                ▼         ▼                    │
│  ┌──────────────────────┐  ┌──────────────────┐  ┌───────────────┐    │
│  │ Core Orchestrator    │  │ Memory MCP       │  │ Redis         │    │
│  │ :4000               │  │ Server :5000     │  │ :6379         │    │
│  │                      │  │                  │  │               │    │
│  │ FastAPI + SSE        │  │ FastMCP          │  │ Standalone    │    │
│  │ (api_gateway)        │  │ (streamable HTTP)│  │ binary        │    │
│  └──────────────────────┘  └──────────────────┘  └───────────────┘    │
└────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Data Flow Patterns

**Pattern 1: User sends message (Chat tab)**
```
React InputArea → invoke("agent_send_message", {text, session_id})
  → Rust: validate, build context (CLAUDE.md, history from SQLite)
  → Rust: POST to core-orchestrator /api/v1/agent/invoke
  → Python: LivingAgent.process_message() → LLM → tool calls → response
  → Python: SSE stream back to frontend (thinking chunks, tool calls, text)
  → React: update message list in real-time
  → Rust: store final message in SQLite session
```

**Pattern 2: AutoDev agent status polling**
```
React AutoDevPanel → invoke("autodev_get_roster")
  → Rust: GET core-orchestrator /api/v1/agents/status
  → Python: AgentRegistry.list_agents() → health data
  → Rust: transform to AgentRosterEntry[] → return to React
```

**Pattern 3: Memory recall (transparent)**
```
Python LivingAgent.process_message()
  → MemoryCore.recall_with_budget()
  → MCP Client → POST memory-mcp-server /memorymcpserver (search_memories)
  → Memory Manager → mem0/Redis/MongoDB (failover chain)
  → Return consciousness (episodic + semantic + procedural)
```

**Pattern 4: Tool execution (Rust-native)**
```
React: user approves tool → invoke("agent_execute_tool", {tool_id, args})
  → Rust: check permissions
  → Rust: execute locally (read_file, write_file, bash, git)
  → Rust: return result → emit "agent:tool-call-result" event
  → React: display result in tool block
```

### 5.3 Sidecar Lifecycle

```
App Launch → SidecarManager::init()
  ├─ Check Redis binary exists → spawn Redis on :6379
  ├─ Wait for Redis health check (redis-cli PING)
  ├─ Check memory-mcp-server exists → spawn on :5000
  ├─ Wait for /health endpoint → OK
  ├─ Check core-orchestrator exists → spawn on :4000
  ├─ Wait for /health endpoint → OK
  └─ Emit "sidecar:all-ready" event → frontend unlocks agent features

App Shutdown → SidecarManager::shutdown()
  ├─ Send SIGTERM to core-orchestrator (graceful shutdown FSM)
  ├─ Send SIGTERM to memory-mcp-server
  ├─ Send SIGTERM to Redis (SHUTDOWN NOSAVE)
  └─ Wait up to 5s per process, then SIGKILL
```

### 5.4 Event Bridge: Python SSE → Tauri Events

The Rust backend subscribes to the core-orchestrator's SSE endpoint and re-emits as Tauri events:

```
Python SSE stream (core-orchestrator /api/v1/stream/{session_id})
  → Rust SSE client (reqwest + eventsource)
  → Parse event type:
      "agent_thinking"    → emit "agent:thinking-chunk"
      "agent_content"     → emit "agent:stream-token"
      "tool_call_start"   → emit "agent:tool-call-start"
      "tool_call_result"  → emit "agent:tool-call-result"
      "agent_stop"        → emit "agent:status-change" {status: "idle"}
      "memory_recall"     → emit "agent:memory-activity"
  → React: listen via useTauriEvent() hooks
```

This bridge pattern means the React frontend only needs to know about Tauri events — it never directly connects to Python. Except for one case: **direct SSE subscription** for lowest-latency streaming (optional optimization in Phase 3+).

---

## 6. Workspace & Git Strategy

> **Key question answered**: "How should workers modify the repo(s)?" and "Should the UI have a Git tab?"

### 6.1 How Claude Code and Goose Handle Repo Modifications

#### Claude Code — Direct writes + Git Worktree Isolation

Claude Code modifies the **local working directory** directly:

1. **`FileWriteTool`** — writes to absolute paths via `fs.writeFile()`. Tracks file history (backup snapshots before/after each write), computes git diffs after each write (`fetchSingleFileGitDiff`), and notifies VS Code via MCP.
2. **`FileEditTool`** — find-and-replace edits on existing files, also with file history tracking.
3. **`BashTool`** — executes arbitrary shell commands. Git operations (`git add`, `git commit`, `git push`, `gh pr create`) are NOT special tools — they're bash commands orchestrated by prompt-driven workflows (e.g., `commit-push-pr.ts` is an LLM prompt that tells the agent which bash commands to run).
4. **`EnterWorktreeTool` / `ExitWorktreeTool`** — the critical feature for multi-agent work:
   - Creates a git worktree in `.claude/worktrees/<slug>/` with a new branch `worktree-<slug>` from `origin/main`
   - Calls `process.chdir()` to switch the session into the worktree
   - Symlinks `node_modules` and copies `.worktreeinclude` files to avoid disk bloat
   - On exit: either **keep** (worktree preserved at path) or **remove** (worktree + branch deleted, `git worktree remove --force`)
   - Subagents get their own lightweight worktrees via `createSubagentWorktree()`
5. **Commit Attribution** — adds `Generated-by: Claude` trailers to git commits.
6. **File History / Checkpoints** — snapshots of file states at each message boundary, enabling undo and diff visualization in the UI (up to 100 snapshots per session).

**Key insight**: Claude Code does NOT have built-in git tools. All git operations flow through the `BashTool` with prompt-driven orchestration. The worktree system is the only structured git integration.

#### Goose — Simpler Direct Writes, No Worktrees

Goose takes a simpler approach:

1. **`DeveloperClient` extension** — 4 tools: `write`, `edit`, `shell`, `tree`
2. All tools receive a `working_dir` from `ToolCallContext` (set per-session, stored in SQLite)
3. `resolve_path()` resolves relative paths against `working_dir` — `path.join(working_dir, relative_path)` if not absolute
4. **No git worktree support** — no branch management, no clone, no built-in git tools
5. **No file history/checkpoint system** — no undo capability
6. Git operations happen through the `shell` tool — agent or user runs `git commit`, etc.
7. **Session persistence** — `working_dir` stored in SQLite, resumable across sessions

**Key insight**: Goose trusts the user (or agent) to manage git via shell commands. The `DeveloperClient` only provides file I/O + shell. Simplicity is the value.

### 6.2 AutoDev's Workspace Strategy

AutoDev combines both approaches with AIRS-specific enhancements:

#### File Modification: Rust-Native Tools

File tools run in **Rust natively** for speed, permission enforcement, and OS integration:

| Tool | Rust Module | Behavior |
|---|---|---|
| `read_file` | `agent/builtin_tools/read_file.rs` | Read with line offset/limit support |
| `write_file` | `agent/builtin_tools/write_file.rs` | Write with file checkpoint before mutation |
| `edit_file` | `agent/builtin_tools/edit_file.rs` | Find-and-replace with uniqueness check |
| `run_command` | `agent/builtin_tools/bash.rs` | Shell execution with timeout + permission check |
| `grep` / `glob` / `list_directory` | `agent/builtin_tools/` | Read-only search tools |

All write tools resolve paths against the session's `working_dir` (like Goose). The `working_dir` is stored in SQLite per session and can change when entering a worktree.

#### Git Integration: Rust `git2-rs` — NOT Shell-Based

Unlike Claude Code (bash-based git) and Goose (no git), AutoDev uses **`git2-rs`** (libgit2 Rust bindings) for structured git operations:

```
src-tauri/src/git/
├── mod.rs              # Public API: clone, checkout, branch, diff, status, commit
├── worktree.rs         # Worktree creation/removal/listing
├── repo_manager.rs     # Multi-repo management (clone, open, list)
├── diff.rs             # Diff computation for IDE tab + file history
├── status.rs           # Working tree status (staged, unstaged, untracked)
├── branch.rs           # Branch operations (create, switch, delete, list)
├── remote.rs           # Push, pull, fetch operations
└── types.rs            # GitRepo, GitStatus, GitDiff, WorktreeSession types
```

**Why `git2-rs` over shell `git`**:
- **Structured output** — no parsing `git status --porcelain` or `git diff --stat`
- **No subprocess overhead** — direct C library calls via libgit2
- **Diff engine** — feeds directly into IDE tab's syntax-highlighted diff view
- **Safety** — type-safe Rust APIs prevent git corruption from malformed commands
- **Worktree lifecycle** — create/enter/exit/remove worktrees as first-class operations

#### Worktree Isolation Per Task

The `worker/backend/<issue>` branching pattern from the master plan maps directly to git worktrees:

```
Session starts → agent claims task #142
  → git2::worktree::create("worker/backend/142")
  → Creates worktree at .autodev/worktrees/worker+backend+142/
  → Creates branch worktree-worker+backend+142 from origin/main
  → Session working_dir updated to worktree path
  → Agent writes/edits files within worktree (isolated from main)
  → Agent runs tests, commits, pushes
  → Agent creates PR via GitHub MCP tool
  → On task complete:
      → git2::worktree::remove() OR keep for review
      → Session working_dir reverts to original repo root
```

**Worktree storage**: `.autodev/worktrees/<slug>/` (parallel to Claude Code's `.claude/worktrees/`)

#### File Checkpointing (Undo Support)

Like Claude Code's file history, AutoDev tracks file snapshots per message:

```rust
struct FileCheckpoint {
    message_id: Uuid,
    file_path: PathBuf,
    before_hash: Option<String>,  // None = file didn't exist
    after_hash: String,
    timestamp: DateTime<Utc>,
}
```

Checkpoints enable:
- **Undo/redo** — revert to any message's file state
- **Diff visualization** — show exactly what changed per agent turn in the IDE tab
- **Audit trail** — every file mutation linked to the message that caused it

Stored in the session's SQLite database, capped at 100 checkpoints per session (matching Claude Code).

### 6.3 Git-in-IDE Decision: No Separate Git Tab

> **Answered**: "Should the UI have a Git tab like GitHub Desktop?"
>
> **Answer: No separate tab. Integrate Git into the IDE tab as a sub-panel.**

**Rationale**:

1. The existing 5-tab structure (Chat, AutoDev, IDE, Recipes, Settings) maps cleanly to distinct workflows. Git is not a separate workflow — it's part of the IDE workflow.
2. The mockup's IDE tab already shows git context: `branch: worker/backend/142` and `3 changes` in the status bar.
3. Adding a 6th tab fragments the UI and creates confusion about where to find git operations.
4. VS Code, JetBrains, and other IDEs integrate git as a *panel within the IDE*, not a separate window.

**IDE Tab Enhancement — Git Sub-Panel**:

```
┌─── IDE ──────────────────────────────────────────────────────────────────┐
│  [core-orchestrator ▼] [Clone Repo]    /src/api/health.py    [Git ▼]     │
│  ─────────────────────────────────────────────────────────────────────   │
│  ├─ 📁 src/                    │  1 │ from fastapi import APIRouter     │
│  │  ├─ 📁 api/                 │  2 │ ...                               │
│  │  │  ├─ health.py  M         │    │                                   │
│  │  │  └─ routes.py            │  6 │+ @router.get("/agents/health")   │
│  │  ├─ 📁 agents/              │  7 │+ async def agents_health():      │
│  │  └─ main.py                 │    │                                   │
│  ├─ 📁 tests/                  ├────┴──────────────────────────────────┤
│  └─ pyproject.toml             │ GIT PANEL (togglable via [Git ▼])     │
│                                │ Branch: worker/backend/142             │
│                                │ ┌ Staged ─────────────────────────┐   │
│                                │ │ (none)                           │   │
│                                │ ├ Unstaged ───────────────────────┤   │
│                                │ │  M  src/api/health.py  (+5 -0)  │   │
│                                │ │  ?  src/api/new_file.py          │   │
│                                │ └──────────────────────────────────┘   │
│                                │ [Stage All] [Commit] [Push] [PR]      │
│  ─────────────────────────────────────────────────────────────────────   │
│  pytest: 142✅ 0❌ · mypy: clean · worktree: worker/backend/142          │
└──────────────────────────────────────────────────────────────────────────┘
```

**Git sub-panel capabilities**:

| Feature | Description | Phase |
|---|---|---|
| **Repo selector** | Dropdown of locally cloned repos + "Clone Repo" button | Phase 3 |
| **Clone from AIRS Foundry** | Lists repos from GitHub org (using PAT from Settings) | Phase 3 |
| **Branch switcher** | View current branch, switch, create new | Phase 3 |
| **Changes view** | Staged/unstaged files with inline diff stats | Phase 3 |
| **Stage/unstage** | Click to stage individual files or all | Phase 3 |
| **Commit** | Commit with message (auto-attribution trailer) | Phase 3 |
| **Push** | Push current branch to origin | Phase 3 |
| **Create PR** | One-click PR creation via GitHub MCP | Phase 3 |
| **Worktree manager** | List active worktrees, which agent owns them, navigate | Phase 4 |
| **Diff viewer** | Full file diff view with syntax highlighting | Phase 3 |

**"Clone Repo" flow** — making AutoDev a single source of movement:

```
User clicks [Clone Repo]
  → Rust: GET GitHub API /orgs/AIRS-Foundry/repos (using PAT from vault)
  → UI: show repo list with names, descriptions, last updated
  → User selects repo + target directory
  → Rust: git2::Repository::clone(url, path)
  → UI: repo appears in selector dropdown
  → Session working_dir updated to new clone
```

This makes AutoDev a complete "single pane of glass" — clone, browse, edit, commit, push, PR — all without leaving the app.

---

## 7. Lessons from Reference Codebases

### From Claude Code (TypeScript CLI)

| Pattern | What They Do | What We Adopt | Adaptation for AutoDev |
|---|---|---|---|
| **Tool System** | Tool objects with `name`, `inputSchema` (zod), `call()`, `checkPermissions()`, `isConcurrencySafe()` | ✅ Same interface shape | Implement as Rust structs + traits, with JSON Schema for input validation |
| **Tool Orchestration** | Read-only tools run concurrently (max 10), write tools run serially, partitioned per batch | ✅ Concurrent reads, serial writes | Tokio tasks with semaphore for concurrency control |
| **MCP Client** | Full `@modelcontextprotocol/sdk` with stdio/sse/http transports, multi-scope configs | ✅ Full MCP client | Use `rmcp` Rust crate (same as Goose) for native MCP |
| **Subagents** | `runAgent()` creates child agents with isolated contexts, own tools, own MCP servers | ✅ Phase 4 | Implement as separate Tokio tasks with message passing |
| **Hooks** | PreToolUse, PostToolUse, SessionStart, SubagentStart extensibility hooks | ✅ Phase 3 | Event-driven hook system via Rust channels |
| **Skills** | Prompt-based reusable workflows loaded from markdown files | ✅ Phase 3 | Skills as `.md` files in `.airs/skills/` |
| **AppState** | Zustand-like store with `getAppState()`/`setAppState()` | ✅ Zustand on frontend | Zustand stores for UI state, Rust state for backend |
| **Context Collapse** | Reactive compaction when context window fills | ✅ Conversation compaction | Summarize old messages when approaching token limit |
| **Streaming Tool Executor** | Stream tool results as they complete, don't wait for all | ✅ Streaming results | Tauri events per tool result as it completes |
| **Git Worktrees** | `EnterWorktreeTool`/`ExitWorktreeTool` create isolated git worktrees per task, with symlinks and sparse checkout | ✅ Worktree isolation | `git2-rs` worktrees in `.autodev/worktrees/`, one per task (see Section 6) |
| **File History** | Snapshots of file states at each message boundary, undo capability, diff visualization | ✅ File checkpoints | `git/checkpoint.rs` stores before/after hashes per message in SQLite |
| **Commit Attribution** | Adds `Generated-by: Claude` trailers to git commits | ✅ Agent attribution | Auto-append `Generated-by: AutoDev/<agent-role>` trailer to commits |
| **Git via Bash** | All git operations (`add`, `commit`, `push`, `pr create`) run as bash commands, not structured tools | ⚠️ Upgrade to git2-rs | AutoDev uses `git2-rs` for structured git ops instead of shell; safer, no parsing |

### From Goose (Rust + Electron)

| Pattern | What They Do | What We Adopt | Adaptation for AutoDev |
|---|---|---|---|
| **Crate Structure** | Workspace: `goose` (core), `goose-server` (axum HTTP), `goose-cli`, `goose-mcp` | ⚠️ Partial | Single `src-tauri` crate with modules, not a workspace (simpler for Tauri) |
| **Agent Loop** | `reply()` → `reply_internal()` with streaming, tool categorization, permissions, compaction | ✅ Same loop shape | `agent::reply()` as async stream yielding `AgentEvent` variants |
| **Extension System** | Extensions are MCP clients. Types: Stdio, Builtin, Platform, StreamableHttp, Frontend, InlinePython | ✅ Extension model | Extensions as MCP clients with type-safe configs |
| **Tool Dispatch** | `dispatch_tool_call()` routes to `extension_manager.dispatch_tool_call()` with prefixed names | ✅ Prefixed tool names | `{extension}__{tool}` naming convention for disambiguation |
| **Security Inspectors** | `ToolInspectionManager` with Security, Egress, Adversary, Permission, Repetition inspectors | ✅ Phase 2+ | Start with PermissionInspector, add others incrementally |
| **Recipes** | YAML-defined shareable workflows with recipe scanner | ✅ Phase 2 | YAML recipe engine with cron scheduling |
| **Session Management** | `SessionManager` persists conversations, extension state, working dir | ✅ SQLite sessions | `rusqlite` for session persistence |
| **GooseMode** | Auto, Chat, SmartApprove modes | ✅ Permission modes | ManualApproval, Autonomous, SmartApprove |
| **Provider Abstraction** | Multi-provider via `Provider` trait | ✅ Provider trait | Same pattern with Anthropic, OpenAI, Gemini, Ollama, Azure |
| **Frontend Tools** | Tools defined by frontend, executed via frontend callback | ✅ Phase 3+ | For IDE integration tools (open file, navigate to line) |
| **Auto-Compaction** | `check_if_compaction_needed()` with configurable threshold | ✅ Context management | Auto-compact at 80% of context window |
| **Working Dir per Session** | `working_dir` stored in Session SQLite table, passed via `ToolCallContext`, all tools resolve relative paths against it | ✅ Session working_dir | Same pattern: `working_dir` in session, passed to all tool calls, changes on worktree enter |
| **No Git Integration** | Goose has no built-in git tools — all git ops via `shell` tool | ⚠️ Upgrade | AutoDev adds structured git via `git2-rs` (see Section 6) instead of relying on shell |

### Key Architectural Decisions

Based on both codebases, these are the decisions for AutoDev:

1. **Tauri v2 over Electron** — Master plan mandates this. Smaller binary, Rust backend, better security.
2. **Single Rust crate with modules** over Cargo workspace — Tauri convention. Keep modules (`agent/`, `mcp/`, etc.) well-separated.
3. **Zustand for frontend state** — Both references validate this pattern. Zustand for UI state, Rust owns backend state.
4. **MCP via `rmcp` crate** — Battle-tested in Goose. Native Rust, no Node.js dependency for MCP.
5. **Streaming via Tauri events** — Not HTTP/SSE like Goose's server. Tauri's native event system is more efficient for desktop.
6. **Python sidecars as bundled binaries** — `core-orchestrator` and `memory-mcp-server` compiled with PyInstaller/Nuitka, managed by Tauri sidecar API.
7. **Tool interface as Rust trait** — `Tool` trait with `name()`, `description()`, `input_schema()`, `execute()`, `is_read_only()`.

---

## 8. Module Breakdown & Dependency Graph

```
src-tauri/src/
├── main.rs                  # Tauri entry point, app setup, plugin registration
├── lib.rs                   # Re-exports all modules
├── commands/                # #[tauri::command] functions (thin wrappers)
│   ├── mod.rs
│   ├── agent_commands.rs    # send_message, cancel, get_history
│   ├── provider_commands.rs # test_connection, list_providers, set_active
│   ├── settings_commands.rs # get_settings, update_settings
│   ├── vault_commands.rs    # store_key, get_key, delete_key
│   ├── sidecar_commands.rs  # start_sidecar, stop_sidecar, sidecar_status
│   └── session_commands.rs  # list_sessions, get_session, delete_session
├── agent/                   # Core agent engine
│   ├── mod.rs
│   ├── engine.rs            # AgentEngine: the main agentic loop
│   ├── context.rs           # Conversation context builder (system prompt, CLAUDE.md, history)
│   ├── tool_registry.rs     # ToolRegistry: register, find, list tools
│   ├── tool_executor.rs     # Execute tools with permission checks, concurrency control
│   ├── tool_trait.rs        # The Tool trait definition
│   ├── builtin_tools/       # Built-in tool implementations
│   │   ├── mod.rs
│   │   ├── read_file.rs
│   │   ├── write_file.rs
│   │   ├── edit_file.rs     # String replacement editor (Phase 1)
│   │   ├── bash.rs
│   │   ├── list_directory.rs
│   │   ├── glob.rs
│   │   ├── grep.rs
│   │   └── create_task.rs   # Create task on internal board (Delegate Mode only, Phase 2)
│   ├── permission.rs        # Permission system (ManualApproval, Autonomous, SmartApprove)
│   └── compaction.rs        # Conversation compaction when context fills
├── provider/                # LLM provider abstraction (PA mode only; see Section 4.1)
│   ├── mod.rs               # Public API: list providers, get active, test connection
│   ├── traits.rs            # Provider trait: complete(), stream(), models()
│   ├── anthropic.rs         # Anthropic API client (simple, no litellm)
│   ├── openai.rs            # OpenAI API client
│   ├── gemini.rs            # Google Gemini API client
│   ├── ollama.rs            # Ollama local API client
│   ├── azure.rs             # Azure OpenAI API client
│   ├── model_router.rs      # Route agent roles to specific models (Settings UI)
│   └── streaming.rs         # Unified streaming response parser
│   # NOTE: Worker mode uses core-orchestrator's litellm provider instead
├── mcp/                     # MCP client + server (PA mode + AIRS MCP server; see Section 4.1)
│   ├── mod.rs               # Public API: connect to MCP servers, expose AIRS tools
│   ├── client.rs            # MCP client via rmcp crate (GitHub MCP, Slack MCP, etc.)
│   ├── server.rs            # AIRS MCP server (airs_get_tasks, airs_claim_task, etc.)
│   ├── config.rs            # MCP server configuration (stdio, http, sse)
│   └── types.rs             # MCP protocol types
│   # NOTE: Worker mode uses core-orchestrator's airs_mcp client for external MCP servers
├── vault/                   # OS keychain integration
│   ├── mod.rs
│   └── keychain.rs          # Store/retrieve API keys securely
├── session/                 # Session persistence
│   ├── mod.rs
│   ├── manager.rs           # SessionManager (CRUD, list)
│   ├── storage.rs           # SQLite storage backend
│   └── types.rs             # Session, Conversation, Message types
├── recipes/                 # Recipe engine (Phase 2)
│   ├── mod.rs
│   ├── parser.rs            # YAML recipe parser
│   ├── runner.rs            # Recipe execution engine
│   └── scheduler.rs         # Cron scheduler for recurring recipes
├── git/                     # Git integration (Phase 3) — see Section 6
│   ├── mod.rs               # Public API: clone, checkout, branch, diff, status, commit
│   ├── worktree.rs          # Worktree creation/removal/listing (git2-rs)
│   ├── repo_manager.rs      # Multi-repo management (clone, open, list)
│   ├── diff.rs              # Diff computation for IDE tab + file history
│   ├── status.rs            # Working tree status (staged, unstaged, untracked)
│   ├── branch.rs            # Branch operations (create, switch, delete, list)
│   ├── remote.rs            # Push, pull, fetch operations
│   ├── checkpoint.rs        # File checkpointing per message (undo support)
│   └── types.rs             # GitRepo, GitStatus, GitDiff, WorktreeSession types
├── db/                      # Internal databases (Phase 2) — Task Board + Worker Management
│   ├── mod.rs               # Public API: task ops, worker ops, migrations
│   ├── tasks.rs             # Task CRUD operations via rusqlite
│   │   # Functions: create_task, list_tasks, update_status, get_task, assign_worker, complete_task
│   ├── workers.rs           # Worker CRUD operations via rusqlite
│   │   # Functions: create_worker, list_workers, update_worker_status, delete_worker, get_worker_by_slug
│   ├── schema.rs            # SQLite schema definitions + migrations for both DBs
│   └── types.rs             # Task, Worker, Priority, TaskType, TaskStatus, WorkerStatus types
│   # Database locations:
│   #   ~/.autodev/tasks.db   - Internal task board (Supervisor manages, PA creates)
│   #   ~/.autodev/workers.db - Worker registry (user-created + pre-defined workers)
│   # Worker schema:
│   #   CREATE TABLE workers (
│   #     id INTEGER PRIMARY KEY AUTOINCREMENT,
│   #     name TEXT NOT NULL,              -- Display name: "WORKER-BACKEND"
│   #     slug TEXT UNIQUE NOT NULL,        -- URL-safe: "worker-backend"
│   #     type TEXT NOT NULL,               -- Template type: backend, frontend, platform, mcp, cleanup, custom
│   #     primary_repos TEXT,               -- JSON array of repo names
│   #     model TEXT NOT NULL,              -- Model identifier: "claude-sonnet-4-6"
│   #     recipe_path TEXT NOT NULL,        -- Path to recipe YAML: "~/.autodev/recipes/worker-backend-poll.yaml"
│   #     status TEXT NOT NULL,             -- created, active, working, queued, error, paused, deleted
│   #     polling_interval INTEGER DEFAULT 120, -- Seconds between polls (default 2 min)
│   #     current_task_id INTEGER,          -- FK to tasks.id (NULL if idle)
│   #     queue_count INTEGER DEFAULT 0,    -- Number of queued tasks
│   #     is_system_defined BOOLEAN DEFAULT 0, -- 1 for Supervisor/QC (locked), 0 for workers
│   #     created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
│   #     updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
│   #     last_poll_at DATETIME,
│   #     FOREIGN KEY (current_task_id) REFERENCES tasks(id)
│   #   );
├── github/                  # GitHub integration (Phase 2)
│   ├── mod.rs
│   ├── issues.rs            # GitHub Issues API (workers use for PR creation, not task assignment)
│   └── labels.rs            # Label management
├── autodev/                 # AutoDev-specific logic (Phase 2+)
│   ├── mod.rs
│   ├── dispatch.rs          # Dispatch signal handling
│   ├── lockfiles.rs         # Task lock file management
│   └── heartbeat.rs         # Agent heartbeat reporting
├── sidecar/                 # Sidecar process management
│   ├── mod.rs
│   ├── manager.rs           # Start/stop/monitor sidecars
│   └── health.rs            # Health check endpoints
├── cost/                    # Cost tracking
│   ├── mod.rs
│   └── tracker.rs           # Token counting + cost calculation per session
├── state/                   # Application state
│   ├── mod.rs
│   └── app_state.rs         # Shared mutable state (Arc<Mutex<AppState>>)
├── events.rs                # Tauri event emission helpers
├── errors.rs                # Unified error types (thiserror)
└── logging.rs               # tracing subscriber setup
```

### Frontend Module Map

```
src/
├── index.html               # Vite entry point
├── main.tsx                 # React root + Zustand provider
├── App.tsx                  # Tab router + layout shell
├── stores/                  # Zustand stores
│   ├── agentStore.ts        # Agent state: messages, status, streaming
│   ├── sessionStore.ts      # Session list, active session
│   ├── settingsStore.ts     # Provider config, theme, autodev settings
│   ├── costStore.ts         # Cost tracking state
│   └── sidecarStore.ts      # Sidecar status
├── tabs/                    # Top-level tab views
│   ├── Chat.tsx
│   ├── AutoDevPanel.tsx
│   ├── IDE.tsx
│   ├── Recipes.tsx
│   └── Settings.tsx
├── components/              # Shared UI components
│   ├── layout/
│   │   ├── TitleBar.tsx
│   │   ├── TabBar.tsx
│   │   ├── StatusBar.tsx
│   │   └── Sidebar.tsx
│   ├── chat/
│   │   ├── MessageBubble.tsx
│   │   ├── ToolCallDisplay.tsx
│   │   ├── InputArea.tsx
│   │   ├── StreamingText.tsx
│   │   └── PermissionDialog.tsx
│   ├── settings/
│   │   ├── ProviderCard.tsx
│   │   ├── ModelSelector.tsx
│   │   ├── KeyInput.tsx
│   │   └── AutoDevConfig.tsx
│   └── common/
│       ├── Button.tsx
│       ├── Card.tsx
│       ├── Badge.tsx
│       ├── Spinner.tsx
│       └── Icon.tsx
├── hooks/                   # Custom React hooks
│   ├── useTauriEvent.ts     # Listen to Tauri events
│   ├── useTauriInvoke.ts    # Invoke Tauri commands
│   └── useAgent.ts          # Agent interaction hook
├── lib/                     # Utilities
│   ├── tauri.ts             # Tauri API wrappers
│   ├── format.ts            # Text formatting (markdown, code highlight)
│   └── cost.ts              # Cost calculation helpers
├── types/                   # TypeScript type definitions
│   ├── agent.ts             # AgentStatus, Message, ToolCall types
│   ├── provider.ts          # Provider, Model types
│   ├── session.ts           # Session types
│   └── events.ts            # Event payload types
└── styles/
    ├── globals.css           # Tailwind base + custom tokens
    └── theme.ts              # Design system constants
```

### Dependency Graph (Build Order)

```
Phase 1 Build Order:
━━━━━━━━━━━━━━━━━━━

1. Tauri scaffold          → src-tauri/main.rs, Cargo.toml, tauri.conf.json
2. Vite + React scaffold   → src/main.tsx, App.tsx, index.html, vite.config.ts
3. Design system           → tailwind.config.js, globals.css, theme.ts
4. Layout shell            → TitleBar, TabBar, StatusBar, tab routing
5. Errors + Logging        → errors.rs, logging.rs (foundation for everything)
6. State module            → app_state.rs (shared state container)
7. Events module           → events.rs (Tauri event helpers)
8. Vault module            → vault/keychain.rs (keychain integration)
9. Provider trait          → provider/traits.rs, streaming.rs
10. Anthropic provider     → provider/anthropic.rs (first provider)
11. Settings commands      → commands/settings_commands.rs
12. Settings tab           → tabs/Settings.tsx, stores/settingsStore.ts
13. Session module         → session/ (SQLite persistence)
14. Agent core             → agent/tool_trait.rs, tool_registry.rs
15. Built-in tools         → read_file, write_file, bash, list_directory
16. Agent engine           → agent/engine.rs (the agentic loop)
17. Permission system      → agent/permission.rs
18. Agent commands          → commands/agent_commands.rs
19. Chat tab               → tabs/Chat.tsx, stores/agentStore.ts
20. Cost tracker           → cost/tracker.rs, stores/costStore.ts
```

---

## 9. Phase 1 — App Shell + Provider + Chat

**Goal**: A working Tauri app with tab navigation, one provider (Anthropic), and a functional chat with tool execution.

### 9.1 Tauri App Scaffold

**Files to create:**
- `src-tauri/Cargo.toml` — Workspace member, Tauri v2 deps, `anyhow`, `tokio`, `serde`, `rusqlite`, `tracing`, `keytar` (or `keyring` crate)
- `src-tauri/tauri.conf.json` — Window config (1200×800, min 1000×700), title "AutoDev", dark theme, CSP
- `src-tauri/src/main.rs` — Tauri builder with command registration
- `src-tauri/build.rs` — Tauri build script

**Key Cargo dependencies:**
```toml
[dependencies]
tauri = { version = "2", features = ["devtools"] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
rusqlite = { version = "0.31", features = ["bundled"] }
keyring = "2"
reqwest = { version = "0.12", features = ["json", "stream"] }
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
git2 = "0.19"                                          # Phase 3: git2-rs for git integration
```

### 9.2 Frontend Shell

**Files to create:**
- `index.html` — Vite entry with `<div id="root">`
- `vite.config.ts` — Vite + React + Tauri plugin
- `tsconfig.json` — Strict TypeScript config
- `src/main.tsx` — React root
- `src/App.tsx` — Tab router with 5 tabs
- `src/styles/globals.css` — Tailwind directives + design tokens

**Design tokens (from master plan):**
```css
:root {
  --bg-primary: #0A0A0A;
  --bg-surface: #111111;
  --border: #222222;
  --accent: #00FF88;
  --danger: #FF4444;
  --warning: #FFAA00;
  --text-primary: #F0F0F0;
  --text-secondary: #666666;
  --radius: 2px;
  --spacing: 8px;
  --font-mono: 'IBM Plex Mono', monospace;
  --font-sans: 'IBM Plex Sans', sans-serif;
}
```

### 9.3 Provider Layer

**State machine for provider connection:**
```
┌─────────────┐    test()    ┌──────────┐
│ Disconnected ├────────────►│ Testing  │
└──────┬───────┘             └────┬─────┘
       │                          │
       │                     success│fail
       │                    ┌──────┴──────┐
       │                    ▼             ▼
       │              ┌──────────┐  ┌────────┐
       └──────────────┤Connected │  │ Error  │
                      └──────────┘  └────────┘
```

**Provider trait (Rust):**
```rust
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn models(&self) -> Vec<ModelInfo>;
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;
    async fn stream(&self, request: CompletionRequest) -> Result<impl Stream<Item = Result<StreamChunk>>>;
    async fn test_connection(&self) -> Result<ConnectionTestResult>;
}
```

### 9.4 Agent Engine (Core Loop)

**State machine for agent:**
```
┌──────┐  user_msg  ┌──────────┐  stream  ┌───────────┐
│ Idle ├───────────►│ Thinking ├────────►│ Streaming │
└──┬───┘            └──────────┘         └─────┬─────┘
   │                                           │
   │                                      tool_use│
   │                                    ┌──────┴──────┐
   │                                    ▼             │
   │                              ┌───────────┐       │
   │                              │ Executing │       │
   │                              │ Tool      │       │
   │                              └─────┬─────┘       │
   │                                    │             │
   │                               result│             │
   │                                    ▼             │
   │                              ┌───────────┐       │
   │                              │ Appending │       │
   │                              │ Result    ├───────┘
   │                              └───────────┘
   │                                    │
   │                               no_tools│
   │                                    ▼
   │                              ┌───────────┐
   └──────────────────────────────┤ Complete  │
                                  └───────────┘
```

**Built-in tools for Phase 1:**

| Tool | Read-Only | Description |
|---|---|---|
| `read_file` | ✅ | Read file contents at path with line numbers |
| `write_file` | ❌ | Write content to file at path |
| `edit_file` | ❌ | **NEW** — Edit file by replacing specific strings (safer than write_file) |
| `bash` | ❌ | Execute shell command, capture stdout/stderr |
| `list_directory` | ✅ | List files and subdirectories |
| `grep` | ✅ | **NEW** — Search files for regex pattern, return structured JSON with matches + context |
| `glob` | ✅ | **NEW** — Find files matching glob pattern, return structured JSON with metadata |

**Total:** 7 builtin tools (3 new in Phase 1: edit_file, grep, glob)

**Tool trait (Rust):**
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    fn is_read_only(&self) -> bool;
    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult>;
}
```

### 9.5 Phase 1 Deliverables Checklist

- [x] Tauri app opens with 5-tab layout (Chat / AutoDev / IDE / Recipes / Settings)
- [x] Dark theme with design tokens applied
- [x] Settings tab: add/test/save Anthropic API key via OS keychain
- [x] Chat tab: send a message, get a streaming response
- [x] Tool call display: collapsible blocks showing name, args, result
- [x] Tool execution: `read_file`, `write_file`, `bash`, `list_directory`
- [x] Permission dialog for `bash` and `write_file` in ManualApproval mode
- [x] AUTODEV.md injection: read from SQLite (`~/.autodev/config/autodev_global.md` + per-repo entries) into system prompt
- [x] Status bar: active model name + today's cost
- [x] Session persistence: conversations saved in SQLite
- [x] Cost tracking: tokens in/out per message, stored in SQLite
- [x] Multiple Chat Sessions UI (NEW)
- [x] Rich Tool Display UI with Line Changes (NEW)
- [x] Grep & Glob File List Results (NEW)

> **Completed**: All Phase 1 React + Rust modules implemented and Vite build passes (0 TS errors, 0 build errors). Node.js version pinned to v22.1.0 — Vite downgraded to v6.3.x, @vitejs/plugin-react to v4.x, TypeScript to v5.7.x for compatibility.

---

### 9.6 Phase 1 Extensions: Multiple Sessions + Rich Tool Display

#### A. Multiple Chat Sessions UI

**Layout Changes (Chat.tsx):**

```
┌─────────────────────────────────────────────────────────────┐
│  TitleBar                                                   │
├──────────────┬──────────────────────────────────────────────┤
│ SessionBar   │  ChatMessages                                │
│ (200px)      │  (flex-grow)                                 │
│              │                                              │
│ [+ New]      │  User: "Read file X"                         │
│              │  ┌─────────────────────────────────────────┐ │
│ Session 1    │  │ Tool: read_file                         │ │
│ 2 min ago    │  │ Path: /src/main.rs                      │ │
│ ✓ 2 msgs     │  │ ┌─────────────────────────────────────┐ │ │
│              │  │ │ /src/main.rs:1-50                   │ │ │
│ Session 2    │  │ │ [Copy] [Open in IDE]                │ │ │
│ 1 hour ago   │  │ │ ─────────────────────────────────── │ │ │
│ ✓ 5 msgs     │  │ │ 1 | fn main() {                     │ │ │
│              │  │ │ 2 |     println!("Hello");          │ │ │
│ Session 3    │  │ │ 3 | }                               │ │ │
│ Yesterday    │  │ │ ─────────────────────────────────── │ │ │
│ ✓ 12 msgs    │  │ └─────────────────────────────────────┘ │ │
│              │  └─────────────────────────────────────────┘ │
│ [⋮ Delete]   │                                              │
│              │  Input: [Type message...] [Send]             │
└──────────────┴──────────────────────────────────────────────┘
```

**SessionBar Component (`src/components/chat/SessionBar.tsx`):**
- Left sidebar (200px width) showing all sessions
- Session list with: title, timestamp (relative), message count
- Active session highlighted with blue border
- "New Session" button at top
- Delete button at bottom (context menu)
- Click session → loads messages from SQLite via `switch_session(id)` Tauri command

**Database Schema (already exists):**
```sql
CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  title TEXT,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  message_count INTEGER DEFAULT 0
);
```

**Tauri Commands Needed:**
- ✅ `list_sessions()` — Already exists
- ✅ `create_session()` — Already exists
- ✅ `delete_session(id)` — Already exists
- ⚠️ NEW: `switch_session(id)` — Load all messages for session from SQLite
- ⚠️ NEW: `get_session_messages(session_id)` — Fetch messages + tool results

**Store Updates (settingsStore.ts):**
- Add session switching logic to `useSessionStore`
- Load messages when switching sessions
- Persist active session ID in localStorage

---

#### B. Rich Tool Display UI with Line Changes

**Enhanced ToolCallDisplay.tsx Component:**

Shows tool results with context-specific formatting:

1. **FileContentDisplay** (for `read_file`):
   - File path with clickable link
   - Line numbers on left (color-coded for changes)
   - Syntax-highlighted code via `prism-react-renderer`
   - Green highlight for modified lines (from `write_file` results)
   - Yellow highlight for diff lines
   - Copy button for code block
   - ✅ "Open in IDE" button — delivered in Phase 3.2 via `src/utils/openInIde.ts`

2. **GrepResultsDisplay** (for `grep` tool):
   - Pattern and match count summary
   - Collapsible file list
   - For each file: line number + line content
   - **Match text highlighted in yellow** within line content
   - Sortable/filterable by filename
   - Shows total matches and files matched

3. **GlobResultsDisplay** (for `glob` tool):
   - Pattern summary
   - **Sortable table**: Name | Size | Modified Date
   - **Filterable** by file extension (.rs, .ts, etc.)
   - Directory icon (📁) vs file icon (📄)
   - File size formatted (KB, MB)
   - Modified date as relative time (2 hours ago)
   - Shows total files matched

4. **BashOutputDisplay** (for `bash` tool):
   - Command displayed
   - Exit code badge (green for 0, red for non-zero)
   - Stdout in monospace (scrollable, max 48 lines)
   - Stderr collapsible tab (red background)
   - ANSI color codes rendered via `ansi-to-react`
   - Execution time

5. **DirectoryListingDisplay** (for `list_directory`):
   - Directory path
   - Sortable table: Name | Type | Size
   - Directory vs file icons
   - Hover effect for interactivity

**Tool Output Parsing (`src/utils/toolOutputParser.ts`):**
- Detects tool type from tool name
- Parses JSON output into structured format
- Handles errors gracefully (fallback to raw text)
- Language detection for syntax highlighting (by file extension)

---

#### C. Grep & Glob Tools with Structured Output

**Grep Tool (`grep` builtin tool):**

Returns structured JSON:
```json
{
  "pattern": "fn main",
  "total_matches": 3,
  "files_matched": 2,
  "results": [
    {
      "file": "/src/main.rs",
      "matches": [
        {
          "line_number": 1,
          "line_content": "fn main() {",
          "match_positions": [[0, 8]]
        }
      ]
    },
    {
      "file": "/src/lib.rs",
      "matches": [...]
    }
  ]
}
```

**Glob Tool (`glob` builtin tool):**

Returns structured JSON:
```json
{
  "pattern": "src/**/*.rs",
  "total_files": 15,
  "files": [
    {
      "path": "src/main.rs",
      "size_bytes": 2048,
      "modified_at": "2026-04-09T12:30:00Z",
      "is_directory": false
    },
    {
      "path": "src/utils/",
      "size_bytes": 0,
      "modified_at": "2026-04-08T10:00:00Z",
      "is_directory": true
    }
  ]
}
```

**Rust Implementations:**
- `src-tauri/src/agent/builtin_tools/grep.rs` — Pattern matching with line numbers
- `src-tauri/src/agent/builtin_tools/glob.rs` — File globbing with metadata
- Both use standard crates: `regex`, `walkdir`, `glob`
- Return JSON-serializable output for React parsing

---

### 9.7 Phase 1 Extensions Deliverables Checklist

**Tool Executor Refinements:** ✅ **COMPLETED**
- [x] **Concurrent/Serial Partitioning**: Read-only tools execute concurrently (max 10 via semaphore), write tools execute serially
  - `tool_executor.rs`: Added `execute_batch()` method with `tokio::sync::Semaphore`
  - `engine.rs`: Updated to use batch execution instead of serial loop
  - Pattern from Claude Code: partitionToolCalls() + concurrent read-only execution
- [x] Tool start/end events emitted for all tools (including concurrent)
- [x] Permission checks preserved for write tools in batch execution
- [x] Results maintain original order matching tool_use_ids

**Multiple Chat Sessions UI:** ✅ **COMPLETED**
- [x] SessionBar component (200px left sidebar) — `src/components/chat/SessionBar.tsx`
- [x] Session list with title, timestamp, message count — `date-fns` formatting
- [x] "New Session" button — `createSession()` in store
- [x] Session switching via `switch_session(id)` Tauri command — `session_commands.rs`
- [x] Load messages from SQLite when switching — `sessionStore.ts`
- [x] Delete session with confirmation — Confirmation dialog implemented
- [x] Active session highlighted with blue border — CSS styling applied
- [x] Keyboard shortcuts: Cmd/Ctrl+T (new), Cmd/Ctrl+W (close) — Event listeners in Chat.tsx

**Rich Tool Display UI:** ✅ **COMPLETED**
- [x] Enhanced ToolCallDisplay with tool-specific rendering — `ToolCallDisplay.tsx`
- [x] FileContentDisplay: syntax highlighting + line numbers + copy button — `prism-react-renderer`
- [x] GrepResultsDisplay: collapsible files + highlighted matches — `GrepResultsDisplay.tsx`
- [x] GlobResultsDisplay: sortable/filterable file table — `GlobResultsDisplay.tsx`
- [x] BashOutputDisplay: stdout/stderr tabs + ANSI colors — `BashOutputDisplay.tsx` + `ansi-to-react`
- [x] DirectoryListingDisplay: sortable file table — `DirectoryListingDisplay.tsx`
- [x] Tool output parser (`toolOutputParser.ts`) — `src/utils/toolOutputParser.ts`
- [x] Language detection for syntax highlighting — `detectLanguageFromPath()`

**Grep & Glob Tools:** ✅ **COMPLETED**
- [x] Grep tool: structured output (file, line number, match positions) — `grep.rs`
- [x] Glob tool: structured output (path, size, modified date, is_directory) — `glob.rs`
- [x] Both tools return JSON for React parsing — `serde_json::to_string_pretty`
- [x] Rust implementations with error handling — Full error handling in both tools

**Dependencies:** ✅ **INSTALLED**
- [x] `prism-react-renderer` — Syntax highlighting — in `package.json`
- [x] `ansi-to-react` — Terminal ANSI color codes — in `package.json`
- [x] `walkdir` — Directory traversal for grep — in `Cargo.toml`
- [x] `glob` — Glob pattern matching — in `Cargo.toml`
- [x] `regex` — Pattern matching — in `Cargo.toml`

**Tool Message Architecture (Proper Fix #2):** ✅ **COMPLETED**
- [x] **Backend — Types Refactored**: `Message` struct now has `tool_name`, `tool_input`, `tool_call_id`, `is_error` fields instead of embedded `tool_calls`/`tool_results` arrays
- [x] **Backend — Database Schema V2**: `messages` table updated with new columns: `tool_name`, `tool_input`, `tool_call_id`, `is_error`
- [x] **Backend — `Message::new_tool()`**: Factory method for creating separate tool messages
- [x] **Backend — `ToolExecutor` persistence**: Added `SessionManager` reference to save tool messages as they execute
- [x] **Backend — `AgentEngine` wiring**: `with_session_manager()` builder method to pass persistence layer
- [x] **Backend — Concurrent tool persistence**: Both serial and concurrent (batch) tool execution save to database
- [x] **Frontend — `MessageRole` extended**: Added `'tool'` as valid role alongside `'user'` and `'assistant'`
- [x] **Frontend — `ChatMessage` updated**: Supports optional `callId`, `toolName`, `toolInput`, `isError` fields
- [x] **Frontend — `agentStore` tool-end handler**: Creates separate tool message after tool execution, maintains streaming UI in assistant message
- [x] **Frontend — `sessionStore` transformation**: Properly converts backend tool messages with role='tool' to frontend format
- [x] **Frontend — `MessageBubble` refactored**: Added `ToolMessageBubble` component for rendering tool messages as separate chat bubbles
- [x] **Frontend — Tool messages rendered separately**: Chat stream now shows: Assistant bubble → Tool bubble → Assistant bubble (proper conversation flow)
- [x] **Data Persistence Verified**: Tool calls now persist across app reloads and display correctly when session is loaded

**Key Architecture Decisions:**
- Tool messages stored as **separate database rows** (not embedded JSON arrays) — clean relational model
- `tool_call_id` links tool results back to original LLM tool_use blocks — enables conversation reconstruction
- **Dual UI approach**: Assistant message shows tool_calls during streaming (real-time feedback), persisted as separate tool messages (clean history)
- **Separation of concerns**: `ToolExecutor` handles persistence, `AgentEngine` orchestrates, `MessageBubble` renders based on role

---

## 10. Phase 2 — Recipes + AutoDev Panel + GitHub

**Goal**: Recipe engine with YAML parsing and cron scheduling, AutoDev Panel with agent roster, GitHub Issues integration.

### Key Components

- **Recipe Parser**: YAML → Recipe struct with steps, triggers, schedules
- **Recipe Runner**: Execute recipe steps as agent prompts
- **Cron Scheduler**: `tokio-cron-scheduler` for recurring recipes
- **Agent Roster**: Display connected agents and their current tasks
- **GitHub Client**: Issues API (list, create, label, comment)
- **Task Board Widget**: Kanban-style view of GitHub issues

### Phase 2 Submodule Changes (core-orchestrator)

**Required modifications to core-orchestrator for AutoDev tool integration:**

#### File 1: `src/airs/bridges/unified_tool_handler.py`
- Add `register_autodev_handler()` method (mirrors `register_internal_handler()`)
- Update `handle_tool_call()` routing logic:
  - Check `if tool_name.endswith("__autodev")` → route to `autodev_handler`
  - Order: `__internal` → `__autodev` → MCP prefix → error

#### File 2: `src/airs/bridges/autodev_handler.py` (NEW)
- Create new file: `AutoDevToolHandler` class
- Implement `async def handle_tool_call(tool_call: ToolCall) -> Dict[str, Any]`
- Strip `__autodev` suffix from tool name
- Route to appropriate executor:
  - `read_file__autodev` → file system read
  - `write_file__autodev` → file system write
  - `bash__autodev` → shell execution
  - `git_*__autodev` → git operations (via git2-rs equivalent in Python)
- Error handling: return `{"error": "..."}` on failure

#### File 3: `src/airs/agent/living_agent.py`
- In `initialize_bridges()` method:
  - Add: `unified_handler.register_autodev_handler(self.autodev_handler.handle_tool_call, tools=[t.name for t in self.autodev_tools])`
  - Store `self.autodev_tools` from request payload

#### File 4: `src/airs/execution/tool_executor.py`
- Update `MemoryAwareToolExecutor.execute()` to handle `__autodev` suffix
- Strip suffix before logging/memory storage
- Example: `read_file__autodev` → log as `read_file`

**Naming Convention:**
- All AutoDev tools from Tauri must have `__autodev` suffix
- Example: `read_file__autodev`, `write_file__autodev`, `bash__autodev`, `git_status__autodev`
- This prevents collisions with internal tools (`__internal`) and MCP tools (`github_`, `slack_`)

**Testing in Phase 2:**
- Unit test: `test_autodev_handler_routing()` — verify suffix-based routing
- Integration test: `test_worker_recipe_with_autodev_tools()` — end-to-end flow

### Phase 2 Deliverables — Split into 2.1 (Vision Core) and 2.2 (Production Scale)

Phase 2 remains the **grandiose vision**: PA delegates → Board → Supervisor assigns → Workers execute → QC reviews. The development is split into two milestones for focused delivery.

---

#### Phase 2.1 — "The Vision Core" ⭐ **PA Creates Tasks → They Actually Get Done**

**Goal:** First end-to-end workflow. Razi can say "add /health endpoint" in Delegate Mode and see it happen.

**RECIPES SYSTEM (Minimal) — For Fleet Instructions:**
> **Distinction**: Recipes contain instructions for the **Fleet** (Workers, Supervisor, QC). AUTODEV.md contains instructions for the **PA (Razi)**. Two separate systems, two separate UIs.

- [x] **Backend — Recipe Parser & Engine:**
  - YAML recipe parser with validation — Parse recipe YAML into Recipe struct (`src-tauri/src/recipes/parser.rs`)
  - Recipe execution engine (run recipe steps as agent messages) — `src-tauri/src/recipes/runner.rs` (`RecipeRunner`)
  - [x] Cron scheduler for recurring recipes — `src-tauri/src/recipes/scheduler.rs` (`RecipeScheduler`) using `cron` crate + tokio tasks
- [x] **Backend — Hardcoded System Recipes:**
  - Supervisor recipe (`~/.autodev/recipes/supervisor-poll.yaml`) — written at startup via `src-tauri/src/recipes/defaults.rs`
  - Worker recipe (`~/.autodev/recipes/worker-backend-poll.yaml`) — written at startup
  - Both YAML files created on first run from `defaults.rs` via `ensure_default_recipes()`
- [x] **Frontend — Recipes Tab (Read-Only for 2.1):**
  - Recipe browser UI — `src/tabs/Recipes.tsx` lists all recipes from `~/.autodev/recipes/`
  - Recipe viewer — expandable cards with instructions preview
  - Shows: title, schedule, permission_mode, instructions preview
  - [Run Now] button present (disabled, tooltip: "coming Phase 2.2")
  - Full editing (Create/Edit/Delete) deferred to Phase 2.2 with Worker Management UI

**AutoDev Panel & Internal Board (Minimal Viable):**
- [x] Internal Task Board database:
  - SQLite database at `~/.autodev/tasks.db` — `src-tauri/src/db/tasks.rs`
  - Schema: full tasks table with id, title, priority, type, status, acceptance_criteria, assigned_worker, timestamps
  - Tauri commands: `list_tasks`, `update_task_status`, `get_task`, `delete_task` — `src-tauri/src/commands/task_commands.rs`
- [x] AutoDev Panel: **View-only** 4-column kanban (Ready | In Progress | QC Review | Done Today) — `src/tabs/AutoDevPanel.tsx`
  - Matches `autodevmockup.html` spec exactly: `#0A0A0A` background, `IBM Plex Mono`, monospace aesthetic
  - `Ready` column merges `ready` + `assigned` status (both pre-work states)
  - `Done Today` column filters by `completed_at` date (today only); failed tasks shown with `✗` marker
  - In Progress cards show progress bar (visual indicator, static 60% pending live tracking in 2.2)
  - Task cards: `#id · priority`, title, assigned_worker, expand to show acceptance_criteria + description
- [x] Metrics footer row: Cost Today | Tokens | Closed | QC Pass Rate
  - `Closed` and `QC Pass Rate` computed from today's task data; Cost/Tokens deferred to 2.2 (needs cost commands)
- [x] TaskBoard component reads from SQLite via Tauri commands — `src/stores/taskStore.ts`
- ⏸️ Task status real-time Tauri events **DEFERRED to Phase 2.2** — currently polling on tab open
- [x] AutoDev Panel: **Hardcoded** agent roster showing 7 agents (Supervisor, Worker Backend, Worker Frontend, Worker Platform, Worker MCP, Worker Cleanup, QC Agent)
  - Vertical table layout: status dot | AGENT-NAME | current task/idle message | elapsed time
  - Status dots: 🟡 yellow = working (`in_progress`), 🔵 blue = idle
  - Elapsed timer ticks live (1s `setInterval`) computed from task `updated_at`

**PA Mode Toggle (Full Implementation):**
- [x] **UI Components:**
  - Segmented 3-button toggle in chat header: ⚡ Fast Development, 🔍 Review Mode, 📋 Delegate Mode — `src/components/chat/PaModeToggle.tsx`
  - Mode state persisted in `settingsStore.ts` (localStorage key: `autodev:pa_mode`)
  - Mode sent to backend on every `send_message` invoke
- [x] **Backend - System Prompt Variants:**
  - `src-tauri/src/agent/context.rs`: 3 prompt constants: `BASE_SYSTEM_PROMPT`, `REVIEW_SYSTEM_PROMPT`, `DELEGATE_SYSTEM_PROMPT`
  - `ContextBuilder::build_system_prompt_with_mode()` selects prompt by PAMode and injects AUTODEV.md from SQLite
  - ⏸️ Settings panel prompt editing (`get/set_system_prompt_variant`) **DEFERRED to Phase 2.2**
- [x] **Backend - Tool Registry Filtering:**
  - `src-tauri/src/agent/builtin_tools/mod.rs`: `register_for_mode()` filters by PAMode
  - FastDevelopment: all 7 builtin tools | Review: all tools | Delegate: only `create_task`
  - Tool registry rebuilt per-session based on PAMode sent from frontend
- [x] **Backend - create_task Tool:**
  - `src-tauri/src/agent/builtin_tools/create_task.rs` — input: title, priority, task_type, acceptance_criteria, description?
  - Writes to `~/.autodev/tasks.db` with status="ready"
  - Returns task ID + confirmation message; only registered in Delegate Mode

**AUTODEV.md SYSTEM (Phase 2.1.2) — For PA Instructions:**
> **Distinction**: AUTODEV.md contains instructions for **Razi (the PA)**. Recipes contain instructions for the **Fleet**. Different audiences, different storage, different UIs.
>
> **Core Principle**: AUTODEV.md is NOT a file you manually create. It is a first-class UI feature — created, edited, and stored entirely within the AutoDev app. SQLite-backed, not filesystem-scattered.

**Backend — Database Layer:**
- [x] **SQLite Schema** — implemented inline in `AutodevInstructionsDb::open()` in `src-tauri/src/db/autodev_instructions.rs`
  - Table: `autodev_instructions(id, scope, repo_path, content, created_at, updated_at, is_active)` with CHECK constraints and index
- [x] **Rust Repository Module** (`src-tauri/src/db/autodev_instructions.rs`):
  - `AutodevInstructionsDb::get()`, `set()`, `delete()`, `exists()`, `char_count()` — all implemented
- [x] **Tauri Commands** (`src-tauri/src/commands/autodev_instructions_commands.rs`):
  - All 5 commands implemented: `get_autodev_instructions`, `set_autodev_instructions`, `delete_autodev_instructions`, `check_autodev_instructions_exists`, `get_autodev_instructions_char_count`

**Frontend — UI Components:**
- [x] **Sidebar Card** (`src/components/autodev/AutodevMdCard.tsx`):
  - Status dots (green=active, dark=empty), char counts, Edit Global/Edit Repo buttons
  - Integrated into `src/components/layout/Sidebar.tsx`
- [x] **Editor Component** (`src/components/autodev/AutodevMdEditor.tsx`):
  - Full-screen modal with scope tabs [Global] [This Repo]
  - Textarea editor (Monaco deferred — not yet a dependency)
  - Live character counter with 40,000 limit progress bar + color warning
  - Save (Ctrl+S), Reset, Insert Template buttons
  - ESC to close, click backdrop to close
- [x] **Zustand Store** (`src/stores/autodevMdStore.ts`):
  - State: `globalContent`, `repoContents`, `charCounts`, `modifiedScopes`, `isLoading`, `isSaving`, `error`
  - Actions: `loadGlobal()`, `loadRepo()`, `saveGlobal()`, `saveRepo()`, `deleteRepo()`, `setModified()`, `getContent()`, `getCharCount()`
- ⏸️ **Settings Panel Section** (`src/components/settings/AutodevMdSettings.tsx`) — **DEFERRED to Phase 2.2**
  - Filesystem Sync toggle, auto-save toggle, import from CLAUDE.md migration

**Integration:**
- [x] **ContextBuilder Integration** (`src-tauri/src/agent/context.rs`):
  - `build_system_prompt_with_mode(mode, repo_path)` queries SQLite via `AutodevInstructionsDb::open()` for both global and repo scopes
  - Falls back to filesystem paths only if SQLite is empty (migration path)
  - No CLAUDE.md/AGENTS.md fallbacks — AUTODEV.md only
- ⏸️ **Filesystem Sync** — **DEFERRED to Phase 2.2** (settings toggle needed first)
- ⏸️ **Migration Tool** (`migrate_from_claude_md`) — **DEFERRED to Phase 2.2**

**AUTODEV.md Content Structure:**
```markdown
# {project-name}

## Stack
- Language/framework versions
- Package managers
- Key dependencies

## Commands
- `test`: How to run tests
- `lint`: How to lint
- `typecheck`: How to type check
- `dev`: How to start dev server

## Architecture
- Directory structure explanation
- Where different types of code live
- Key abstractions

## Patterns
- Coding conventions specific to this project
- Testing patterns
- Error handling approach
```

**2.1 Success Criteria:**
> Razi switches to Delegate Mode → says "add /health endpoint to orchestrator" → PA creates task #1 → Supervisor assigns to worker-backend → Worker executes → Task moves to Done column in AutoDev Panel

**Phase 2.1: Two Instruction Systems — Frontend Capability Summary:**

| Feature | AUTODEV.md (PA) | Recipes (Fleet) |
|---------|-----------------|-----------------|
| **Audience** | Razi (the human) | Workers, Supervisor, QC |
| **Location** | Sidebar → AUTODEV.md | Recipes Tab |
| **Phase** | 2.1.2 | 2.1 (read-only), 2.2 (full editing) |
| **View** | ✅ Monaco editor with preview | ✅ YAML viewer with syntax highlighting |
| **Create** | ✅ "Create" button in sidebar | ⏸️ Phase 2.2 (Worker Creation UI generates recipes) |
| **Edit** | ✅ Full Monaco editing + Save/Reset | ⏸️ Phase 2.2 (YAML editor) |
| **Delete** | ✅ Delete button (soft delete) | ⏸️ Phase 2.2 (with confirmation) |
| **Storage** | SQLite `~/.autodev/config.db` | YAML files `~/.autodev/recipes/` |
| **Content** | Personal preferences + conventions | Workflow definitions + agent instructions |

---

#### Phase 2.2 — "Production Worker Orchestration" 🚀 **Scale to Multiple Workers + GitHub**

**Goal:** Production-grade worker management, multiple worker types, GitHub integration, full submodule bridge.

**Worker Management System (Full Implementation):**
- [x] **Workers Database:**
  - SQLite database at `~/.autodev/workers.db`
  - Schema: workers table (see Module Breakdown § 8 for full schema)
  - Rust module: `src-tauri/src/db/workers.rs` with `rusqlite` crate
  - Tauri commands:
    - `create_worker(name, type, primary_repos, model, polling_interval)` → Returns worker_id
    - `list_workers()` → Returns all workers (system + user-created)
    - `update_worker_status(worker_id, status)` → Updates worker state
    - `delete_worker(worker_id)` → Soft delete (sets status='deleted')
    - `get_worker_by_slug(slug)` → Retrieves worker config
    - `start_worker_recipe(worker_id)` → Starts recipe cron job via `RecipeScheduler`
    - `stop_worker_recipe(worker_id)` → Stops recipe cron job via `RecipeScheduler`
- [x] **Pre-Defined Workers Seeded on First Launch:**
  - On app first run, seed `workers.db` with 5 pre-defined workers:
    - `worker-backend` (type: backend, model: claude-sonnet-4-6, status: created)
    - `worker-frontend` (type: frontend, model: claude-sonnet-4-6, status: created)
    - `worker-platform` (type: platform, model: claude-sonnet-4-6, status: created)
    - `worker-mcp` (type: mcp, model: claude-opus-4-6, status: created)
    - `worker-cleanup` (type: cleanup, model: claude-haiku-4-5, status: created)
  - Also seed system agents:
    - `supervisor` (is_system_defined: 1, cannot delete)
    - `qc-agent` (is_system_defined: 1, cannot delete)
- [x] **Worker Creation UI (Settings → Workers Tab):**
  - "Create Worker" button → Opens `WorkerCreateModal`
  - Worker creation form fields:
    - Worker Name (auto-generates slug from name)
    - Worker Type dropdown (Backend, Frontend, Platform, MCP, Cleanup, Custom)
    - Primary Repos (text input, comma-separated)
    - Model input
    - Polling Interval input (seconds)
    - Description
  - On submit: creates worker in `workers.db` via `create_worker` Tauri command
  - Validation: name + type required
- [x] **Worker Management UI (Settings → Workers Tab):**
  - `WorkerTable` component in `src/components/workers/WorkerTable.tsx`
  - Table: Name | Type | Status | Model | Actions
  - System agents (is_system_defined) shown with SYSTEM badge, cannot delete
  - Worker row actions: Start/Stop/Delete
  - Status color-coded via `workerStatusColor()` in `workerStore.ts`
- [x] **Agent Roster Dynamic Updates:**
  - `AutoDevPanel.tsx` `AgentRoster` reads from `useWorkerStore` (workers.db)
  - Replaces hardcoded AGENT_DEFS array
  - Status dots use live `workerStatusColor()` with glow effect on 'working'
- [x] **Recipe Auto-Generation:**
  - AI-assisted YAML generation via `generate_recipe_yaml` Tauri command (LLM prompt)
  - `RecipeEditor.tsx` collapsible "Generate with AI" panel: worker type + description → YAML preview → Apply to Form
  - Backend: `src-tauri/src/commands/recipe_commands.rs` — `generate_recipe_yaml(worker_type, name, description)`
  - Generates YAML with appropriate instructions per worker type:
    ```yaml
    version: "1.0.0"
    title: Worker {NAME} — Polling Loop
    schedule: "*/{polling_interval_seconds} * * * *"
    permission_mode: autonomous
    parameters:
      - name: worker_slug
        type: string
        default: "{slug}"
    instructions: |
      You are the AIRS {TYPE} Worker ({NAME}).
      1. Poll internal task board for tasks assigned to '{slug}'
      2. If found: claim via lock file, create worktree, execute, test, PR
      3. If nothing: exit cleanly
    extensions:
      - github-mcp
      - developer
    ```

**RECIPES TAB UI (Full Editing — Completes Recipe System):**
> **Note**: Phase 2.1 had read-only Recipes. Phase 2.2 adds full create/edit/delete UI.
- [x] **Recipe Editor (Structured Form):**
  - `src/components/recipes/RecipeEditor.tsx` — structured form modal
  - Fields: ID (slug), Title, Description, Schedule, Permission Mode, Agent Role, Model, Instructions
  - "Save" / "Cancel" / "Delete" buttons
  - System recipe protection: warns before editing, requires explicit override
  - Version history panel (shows backups with restore buttons)
- [x] **Recipe Create/Edit/Delete UI:**
  - "New Recipe" button in Recipes Tab toolbar
  - "Edit" button on each recipe card (previously disabled)
  - Recipe deletion with confirmation (`delete_recipe` Tauri command)
  - ID auto-slugified from title on create
- [x] **Recipe Browser Enhancements:**
  - System/User recipe sections retained
  - Edit button opens `RecipeEditor` modal
  - Run button still disabled (execution engine in Phase 2.3)
- [x] **Recipe-Worker Binding UI:**
  - `WorkerTable.tsx` — Recipe column with inline `<select>` dropdown per worker row
  - Calls `bind_worker_recipe(worker_id, recipe_id)` on change (existing Tauri command)
  - Loads recipe list from `list_recipes` on table mount; `worker.recipe_id` highlighted in accent color
- [x] **Recipe Versioning (Basic):**
  - On save: `update_recipe` Tauri command backs up previous version to `~/.autodev/recipes/backups/`
  - "History" button in editor shows last N backups with timestamps
  - "Restore" button calls `restore_recipe_backup` Tauri command

**Enhanced Supervisor Logic (Full Implementation):**
- [x] **Supervisor Recipe Logic (Production):**
  - `supervisor-poll.yaml` recipe (system-defined, locked, reads from workers.db):
    - Polls `~/.autodev/tasks.db` every 2 min for tasks with status='ready'
    - For each ready task:
      - Determines if epic (needs breaking down) or atomic
      - If epic: spawns subagents to break into subtasks, creates child tasks
      - If atomic: assigns to appropriate worker based on:
        - Task `repo` field → matches worker's `primary_repos`
        - Task `type` field → matches worker type (e.g., 'bug' → backend worker)
        - Worker availability (not in 'working' state)
      - Updates task: `assigned_worker = worker.slug`, `status = 'assigned'`
      - Optionally: creates dispatch signal file for urgent tasks
  - Supervisor recipe uses **Mode 2: Worker Agent** (core-orchestrator)
  - Has access to all workers via `list_workers()` Tauri command (exposed to core-orchestrator as AutoDev tool)

**GitHub Integration:**
- [x] GitHub PAT storage in vault — `store_github_pat` / `get_github_pat` / `delete_github_pat` via OS keychain
- [x] GitHub PAT UI — `GitHubPATCard` in Settings tab (show stored/clear status, save, remove)
- [x] GitHub MCP integration: settings card + config storage
  - `GitHubMCPCard` in Settings tab: enable/disable toggle, org input, allowed repos textarea
  - Backend: `src-tauri/src/commands/github_mcp_commands.rs` — `get_github_mcp_config` / `set_github_mcp_config`
  - Config stored in `~/.autodev/config.db` `app_settings` table as JSON
  - Worker PR creation via MCP server (runtime activation in Phase 3.4)

**Provider Support & Model Enhancements:**
- [x] Additional providers: OpenAI (`openai.rs`) and Google Gemini (`google.rs`) implemented with full `build_body()`, streaming, tool use, `supports_thinking`, `cost_in/out_per_1m`
- [x] **Thinking Models + Reasoning Effort:**
  - `CompletionRequest` extended: `reasoning_effort: Option<String>`, `thinking_budget_tokens: Option<u32>`
  - Anthropic: `betas=["interleaved-thinking-2025-05-14"]` + thinking block in body when `thinking_budget_tokens` set
  - OpenAI: `reasoning_effort` param for o-series models
  - `ModelInfo.supports_thinking` flag on all providers
  - `ModelConfigSection` in Settings: "Enable Extended Thinking" toggle + budget slider (1024–32000 tokens)
- [x] **Temperature + Top-P Parameters:**
  - `CompletionRequest` extended: `temperature: Option<f32>`, `top_p: Option<f32>`
  - All 3 providers wire these params through their `build_body()`
  - Backend: `src-tauri/src/db/model_configs.rs` + `model_config_commands.rs` (CRUD in `~/.autodev/config.db`)
  - Frontend: `modelConfigStore.ts` (Zustand), `ModelConfigSection` in Settings (sliders + reset buttons)
- [x] **Model Context Window Meter:**
  - `ContextMeter` component in `Chat.tsx` input meta row
  - Estimates tokens (chars/4), displays color-coded progress bar + percentage
  - `MODEL_CONTEXT` map covers Anthropic/OpenAI/Google models; defaults to 200k
  - Tooltip shows `~Xk / Yk tokens used`

**Submodule Bridge (core-orchestrator integration):**
- [x] **[SUBMODULE]** `unified_tool_handler.py`: `register_autodev_handler()` + routing logic
- [x] **[SUBMODULE]** `autodev_handler.py`: `AutoDevToolHandler` class with 8 tools
- [x] **[SUBMODULE]** `living_agent.py`: STEP 4.7 autodev handler initialization
- [x] **[SUBMODULE]** `list_workers__autodev`: reads `~/.autodev/workers.db` directly via `sqlite3` (no Tauri round-trip)
- [x] **[SUBMODULE]** Unit tests (`tests/test_autodev_handler.py`): routing, list_workers, file ops
- [x] **[SUBMODULE]** Integration test framework for Phase 2.3:
  - `tests/conftest.py` — shared fixtures: `tmp_autodev_dir`, `workers_db`, `config_db`, `mock_llm_provider`, `handler`
  - `tests/test_phase23_integration.py` — 25 tests covering: model config CRUD, worker-recipe binding, recipe YAML structure, GitHub MCP config, AutoDevToolHandler routing

**2.2 Success Criteria:**
> Create custom worker "worker-api" via UI → Assign it specific repos → Task routes to it → Worker creates PR on GitHub → QC reviews → Supervisor merges

---

#### Phase 2.3+ — Workspace Folder + Auto-Launch ✅ **Implemented**

> **Foundation for Phase 3 IDE**: The workspace folder establishes the root directory concept that the IDE tab will build on.

- [x] **Workspace Folder Selection:**
  - Native OS folder-picker dialog via `rfd` crate — `pick_workspace_dir()` Tauri command
  - Persist selected folder to `~/.autodev/config.db` `app_settings` table (key: `workspace_dir`)
  - Commands: `get_workspace_dir`, `set_workspace_dir`, `pick_workspace_dir`
  - `WorkspaceCard` in Settings tab — top section with `ACTIVE` badge, Change + Clear buttons
  - All agents (PA, Supervisor, Workers) use workspace_dir as `working_dir`
  - Frontend: `workspaceStore.ts` — Zustand store, loaded at app startup
- [x] **@-Mention Path Resolution:**
  - Chat input preprocesses `@path/to/file` and `@filename.ext` → absolute paths from workspace root
  - Applied in `resolveAtMentions()` before `send_message` IPC call
  - Example: `@src-tauri/src/main.rs` → `C:\AIRS Foundry\autodev\autodev\src-tauri\src\main.rs`
  - Skips tokens that are already absolute paths
- [x] **Auto-Launch Worker Fleet:**
  - On app startup, all workers with `recipe_id` set (and not paused/deleted) are auto-started
  - No manual "Start" click required — fleet wakes up immediately on launch
  - Workers silently fail (error status) if no provider configured yet, retry on next cron tick

---

### Phase 2 Dependency Graph

```
Phase 2.1 Dependencies:
  create_task tool → tasks.db → TaskBoard UI
  PA Mode Toggle → System Prompt Variants → Tool Registry Filtering
  [x] Cron Scheduler → Hardcoded Supervisor Recipe → Hardcoded Worker Recipe

Phase 2.2 Dependencies (requires 2.1):
  workers.db → Worker Management UI → Recipe Auto-Generation
  GitHub PAT → GitHub MCP → Workers create PRs
  Submodule Bridge → core-orchestrator integration

Phase 2.3+ Dependencies (implemented):
  [x] Workspace Folder (config.db) → PA Sessions + Recipe Runner working_dir
  [x] @-mention resolution → workspace_dir → absolute paths in chat
  [x] Auto-launch → RecipeScheduler → all workers start on app open
```

---

## 10.5. Phase 2.5 — Recipe Agent Sandbox + Native Task Tools + Logs Tab

**Goal**: Fix critical bugs preventing task board automation (missing native tools, workspace pollution), add comprehensive logging for debugging, and enhance file picker with folder support.

**Why This Phase**: Phase 2.3+ exposed two critical issues blocking supervisor/worker automation:
1. Recipe agents wrote Python scripts to project root (workspace pollution)
2. Supervisor recipe required `list_tasks` + `update_task_status` tools that didn't exist as agent tools

This phase fixes both issues and adds debugging infrastructure.

### Phase 2.5 Deliverables

#### Critical Bug Fixes

**1. Recipe Agent Sandbox Isolation:**
- [x] `src-tauri/src/recipes/runner.rs` — Changed `working_dir` from `workspace_dir` → `~/.autodev/agents/<slug>/`
  - Each recipe agent (supervisor, worker-backend, etc.) gets isolated sandbox directory
  - Auto-creates `~/.autodev/agents/<slug>/` on first run
  - Python scripts, logs, markdown files created by agents now go to sandbox, never to workspace
  - **Impact**: Eliminates workspace pollution (no more `check_backend_tasks.py`, `SUPERVISOR_EXECUTION_LOG.md`, etc. in project root)

**2. Native Task Board Tools (FastDevelopment Mode):**
- [x] `src-tauri/src/agent/builtin_tools/list_tasks_tool.rs` — `ListTasksTool`
  - Input: optional `status` filter (ready, assigned, in_progress, qc_review, done, failed, cancelled)
  - Returns: JSON array of tasks with id, title, status, priority, assigned_worker, acceptance_criteria
  - Read-only tool, available in FastDevelopment + Review modes
  - **Impact**: Supervisor can now query task board directly without Python scripts

- [x] `src-tauri/src/agent/builtin_tools/update_task_status_tool.rs` — `UpdateTaskStatusTool`
  - Input: task_id, status, optional assigned_worker
  - Updates task status + worker assignment atomically
  - Available in FastDevelopment + Review modes
  - **Impact**: Supervisor can assign tasks and update progress without Python scripts

- [x] `src-tauri/src/agent/builtin_tools/mod.rs` — Tool Registration
  - Both tools registered in `FastDevelopment` mode (used by recipe agents)
  - `create_task` moved to FastDevelopment (was Delegate-only)
  - Delegate mode unchanged: still has `create_task` only
  - **Tool Registry State**:
    - FastDevelopment: read_file, write_file, edit_file, bash, list_directory, grep, glob, **list_tasks**, **update_task_status**, **create_task**
    - Review: same as FastDevelopment
    - Delegate: create_task only

#### Logging Infrastructure

**3. Log File System:**
- [x] `src-tauri/src/commands/log_commands.rs` — New module
  - `list_log_files() -> Vec<String>` — returns available log dates (YYYY-MM-DD format)
  - `read_log_file(date: String) -> String` — reads full log file content
  - Logs stored at `~/.autodev/logs/autodev-YYYY-MM-DD.log` (daily rotation)
  - Registered in `src-tauri/src/lib.rs`

**4. Logs Tab (Frontend):**
- [x] `src/tabs/Logs.tsx` — Full-featured log viewer
  - Date picker: select which day's log to view
  - Level filter: ALL, ERROR, WARN, INFO, DEBUG, TRACE
  - Search box: filter log lines by text
  - Auto-scroll toggle: follow latest logs
  - Auto-refresh: every 3 seconds (configurable)
  - **Copy All** button: copies entire log to clipboard (shows "Copied!" confirmation)
  - **Export** button: downloads log as `.log` file to browser Downloads folder
  - Color-coded log levels: ERROR (coral), WARN (amber), INFO (periwinkle), DEBUG (muted), TRACE (dark)
  - Timestamp, module, and message parsing from tracing format
  - Added to `src/App.tsx` as 6th tab

#### File Picker Enhancements

**5. @-Mention File Picker with Folders:**
- [x] `src-tauri/src/commands/file_commands.rs` — Updated FileEntry struct
  - Added `is_dir: bool` field
  - Added `children_count: u32` field (recursive file count in directory)
  - `count_files_in_dir()` helper function for recursive counting
  - Search results now include both files and folders

- [x] `src/components/chat/AtFilePicker.tsx` — Updated UI
  - Folders shown with □ icon (gold color) + "(N files)" count
  - Files shown with </> icon (accent color)
  - Keyboard navigation: ↑↓ to move, Enter to select, Escape to close
  - Fuzzy matching on both file and folder names

#### Testing & Verification

- [x] Cargo check: PASSES (1 pre-existing grep.rs warning)
- [x] TypeScript: tsc --noEmit PASSES (0 errors)
- [x] `.gitignore` updated: added `scripts/` directory to ignore list (user action)

### Phase 2.5 Success Criteria

> **Workspace Cleanliness**: Run supervisor recipe → No Python files created in project root or `scripts/` folder → All agent helper files go to `~/.autodev/agents/supervisor/`

> **Task Board Automation**: Supervisor recipe polls tasks via `list_tasks` tool → Finds "ready" tasks → Assigns via `update_task_status` tool → Worker claims and executes → All without Python scripts

> **Logging**: User opens Logs tab → Selects today's date → Sees color-coded log lines → Filters by ERROR level → Finds root cause → Exports log for sharing

> **File Picker**: User types `@src` in chat → Sees folders with file counts → Selects `src/` folder → Absolute path inserted → Can also select individual files

### Phase 2.5 Impact Summary

| Issue | Before | After |
|-------|--------|-------|
| Workspace pollution | 29+ Python files in root | 0 files in root; all in `~/.autodev/agents/` |
| Task board access | LLM invents Python scripts | Native Rust tools: `list_tasks`, `update_task_status` |
| Debugging | Logs scattered, hard to find | Centralized in `~/.autodev/logs/`, viewable in UI |
| File selection | Only files, no folders | Folders + files with recursive counts |

---

## 10.6. Phase 2.6 — Fleet Sidecar Integration ✅ **Implemented**

> **Goal**: AutoDev spawns and monitors both Python submodule processes from within the app, routes their stdout/stderr into the AutoDev Logs tab, enforces correct startup order, and excludes the irrelevant adobe-mcp-server from the AutoDev deployment.

### Submodule Connection Architecture

```
AutoDev (Rust + Tauri)
  └── SidecarManager  [src-tauri/src/sidecar/mod.rs]
        │
        ├── Phase 1: memory-mcp-server  :8000  [submodules/memory-mcp-server/]
        │     Command: uv run python main.py --mode dev
        │     Health:  GET http://127.0.0.1:8000/health
        │     Logs:    ~/.autodev/logs/memory-mcp.log  +  tracing subscriber
        │
        └── Phase 2: core-orchestrator  :4000  [submodules/core-orchestrator/]
              Command: uv run uvicorn api_gateway.api.main:app --host 127.0.0.1 --port 4000
              Health:  GET http://127.0.0.1:4000/api/system/health
              Logs:    ~/.autodev/logs/core-orchestrator.log  +  tracing subscriber
                         ↓
                    LivingAgent  [src/airs/agent/living_agent.py]
                         ↓
                    MemoryCore   [src/airs/memory/] (connects to :8000)
```

**Startup order is mandatory**: `MemoryCore` inside `LivingAgent` connects to `:8000` at agent
creation time. If `memory-mcp-server` is not up when `core-orchestrator` starts, agents will fail
to initialise memory. `SidecarManager.start_memory()` must be called and confirmed healthy before
`SidecarManager.start()` is called for core-orchestrator.

### Files Involved

| File | Role |
|------|------|
| `src-tauri/src/sidecar/mod.rs` | `SidecarManager` — spawns/stops/monitors both processes; `spawn_log_router()` pipes stdout/stderr; `resolve_sidecar_path()` / `resolve_memory_path()` |
| `src-tauri/src/sidecar/client.rs` | `OrchestratorClient` — `is_healthy()` (`:4000`), `is_memory_healthy()` (`:8000`), `ensure_agent()`, `invoke_worker()` |
| `src-tauri/src/commands/sidecar_commands.rs` | Tauri commands: `start_sidecar` (starts memory first → then orchestrator), `stop_sidecar` (kills both), `check_sidecar_connectivity`, `get_sidecar_status` |
| `src-tauri/src/main.rs` | On startup: probes `:8000` and `:4000`; marks sidecars running if already up externally |
| `src/stores/sidecarStore.ts` | Zustand store — sidecar status, `loadStatus()`, `startSidecar()`, `stopSidecar()` |
| `src/tabs/AutoDevPanel.tsx` | Status badge in header; probes connectivity on mount |
| `submodules/core-orchestrator/autodev_mcp_config.yaml` | **AutoDev-specific MCP config** — memory only, no adobe-mcp-server |
| `submodules/core-orchestrator/api_gateway/api/main.py` | Reads `MCP_CONFIG_FILE` env var; uses `autodev_mcp_config.yaml` when set |
| `submodules/core-orchestrator/.env` | `MCP_CONFIG_FILE=autodev_mcp_config.yaml` set here |

### Adobe MCP Server — Why It's Excluded

`core-orchestrator` ships with two MCP servers defined in `config/mcp/`:
- `mcp-adobe` — Adobe Firefly/Photoshop creative tools (NOT needed for AutoDev)
- `mcp-memory` — Long-term semantic memory via mem0 (required for LivingAgent)

Previously, `mcp_config.yaml` included both. AutoDev now uses `autodev_mcp_config.yaml` which
contains only `mcp-memory`. This is selected via `MCP_CONFIG_FILE=autodev_mcp_config.yaml` in
`.env`. The full `mcp_config.yaml` remains for AIRS production deployments that use Adobe tools.

Note: even without this fix, adobe doesn't block startup — `service_registry.py` catches config
errors and continues with partial registry. But explicit exclusion is cleaner.

### Log Routing

Both processes are spawned with `Stdio::piped()`. Two background `std::thread`s per process
(`spawn_log_router`) read lines from stdout/stderr:
1. Each line is emitted via `tracing::info!("[core-orch] ...")` → appears in main AutoDev log file
2. Each line is also appended to a dedicated sidecar log file:
   - `~/.autodev/logs/core-orchestrator.log`
   - `~/.autodev/logs/memory-mcp.log`

Both log files are readable from the AutoDev **Logs tab** (which reads `~/.autodev/logs/`). This
means all three log streams (Rust app, core-orchestrator, memory-mcp) are visible in one UI.

### Phase 2.6 Deliverables

- [x] `sidecar/mod.rs` — `SidecarManager` manages both processes + log routing + `resolve_memory_path()`
- [x] `sidecar/client.rs` — `is_memory_healthy()` health check on `:8000`
- [x] `sidecar_commands.rs` — `start_sidecar` enforces memory→orchestrator startup order; `stop_sidecar` kills both
- [x] `main.rs` — startup probe for both `:8000` and `:4000`
- [x] `autodev_mcp_config.yaml` — AutoDev MCP config (memory only)
- [x] `api_gateway/api/main.py` — `MCP_CONFIG_FILE` env var support
- [x] `core-orchestrator/.env` — `MCP_CONFIG_FILE=autodev_mcp_config.yaml`

---

## Phase 2 Bug and Improvements

All fixes applied systematically from `phase2submodulefix.md`. Each task is marked as done below.

### FIX-1 — AutoDev Tool Pipeline (E-01)

- [x] **T1.1** `models.py` — Added `autodev_tools: List[str]` field to `AgentCreate` Pydantic model with default empty list.
- [x] **T1.2** `agent_writer.py` — Added `autodev_tools: Optional[List[str]] = None` parameter to `create_agent_config_from_request`; wired through to `AgentConfig(autodev_tools=autodev_tools or [])`.
- [x] **T1.3** `agent_schema.py` — Added `autodev_tools: List[str]` field to `AgentConfig` schema.
- [x] **T1.4** `factory.py` — Reads `agent_config.autodev_tools` and passes the list to `AgentMetadata(autodev_tools=autodev_tool_names)`.
- [x] **T1.5** `client.rs` — `ensure_agent` POST body now includes `autodev_tools` list (8 tools). `mcp_servers` is `[]` — memory is handled transparently inside core-orchestrator (`living_agent.py` → `context_service.py` → `memory_core.py`); agents never see or call memory-mcp-server tools directly.
- [x] **T1.6** `autodev_handler.py` — `get_tool_specs()` now returns `ToolSpec` Pydantic objects (not plain dicts) so `living_agent.add_tools()` type validation passes.
- [x] **T1.10** Deleted all stale agent YAML configs from `config/agents/` (`supervisor-*`, `worker-backend-*`, `worker-frontend-*`, `worker-mcp-*`, `qc-agent-*`, `worker-cleanup-*`, `worker-platform-*`). Next `ensure_agent` call recreates them with the correct fields.

### FIX-3 — Unicode Encoding (E-11)

- [x] **T3.2** `app.py` — Added `sys.stdout.reconfigure(encoding="utf-8", errors="replace")` and `sys.stderr.reconfigure(...)` guarded by `sys.platform == "win32"` at startup, before the logging system initializes.
- [x] **T3.3** `sidecar/mod.rs` — Upgraded `PYTHONIOENCODING` from `"utf-8"` to `"utf-8:replace"` in both sidecar spawn commands (dev mode + uvicorn mode). Also sets `PYTHONUTF8=1`.

### FIX-4 — Episodic Memory Isolation (E-03)

- [x] **T4.0** `service.py` — Replaced ephemeral `headless-{uuid}` session ID with stable `worker-{agent_id}` so each fleet worker has a persistent, deterministic memory lane across all recipe ticks.
- [x] **T4.1 / T6.1** `stream_manager.py` — Extended `broadcast_to_session` headless guard to also skip `worker-*` sessions (which have no SSE connection by design). All `emit_*` methods delegate through this guard.
- [x] **T4.2** `mem0_adapter.py:864` — Fixed permissive session filter: changed `if item_session_id is not None and item_session_id != session_id` → `if item_session_id != session_id`. Memories with `session_id=None` (legacy unscoped) are now excluded from session-scoped queries.
- [x] **T4.4** `context_service.py` — Added `agent_id: Optional[str] = None` parameter to `_retrieve_recent_conversation`; passes `"metadata.agent_id": agent_id` filter to `list_memories` MCP call when available.
- [x] **T4.5** `context_service.py` — Added `agent_id: Optional[str] = None` parameter to `_retrieve_relevant_conversation`; passes `"metadata.agent_id": agent_id` filter to `search_memories` MCP call when available. Both call sites in `build_agent_context` now forward `agent_id`. Also applied to `get_timeline_messages`: injects `agent_id` into the `list_memories` filter dict before the MCP call (not just as a post-retrieval filter).

### FIX-5 — Scheduler First-Tick Failures (E-04)

- [x] **T5.1** `runner.rs` — Added public `is_orchestrator_ready() -> bool` method to `RecipeRunner` that delegates to `orch_client.is_healthy()`.
- [x] **T5.2** `scheduler.rs` — Added `wait_for_orchestrator_ready()` module-level async helper (polls every 2 s, up to 45 s timeout). Called inside the `async_runtime::spawn` block before the first cron iteration so the first recipe tick waits for the orchestrator to be reachable.

### FIX-7 — LiteLLM Log Noise (E-06/E-09)

- [x] **T7.2** `app.py` — Silenced `LiteLLM`, `LiteLLM Router`, `LiteLLM Proxy`, `openai`, `openai._base_client`, `httpx`, and `httpcore` loggers to `WARNING` level at module load time to prevent log circuit breaker triggers and API key exposure in plain-text logs.
- [x] **T7.3** `sidecar/mod.rs` — Added `LITELLM_LOG=WARNING` environment variable to the core-orchestrator uvicorn spawn command, suppressing LiteLLM debug noise at the OS process level (complements T7.2).

### FIX-10 — asyncio Deprecation (E-12)

- [x] **T10.1** `memory-mcp-server/main.py` — Replaced 3× `asyncio.get_event_loop()` calls with `asyncio.get_running_loop()` in the signal handler, `shutdown_server_with_timeout`, and Windows exception handler registration.
- [x] **T10.2** `mem0_adapter.py` — Bulk-replaced all `asyncio.get_event_loop()` calls (17 occurrences) with `asyncio.get_running_loop()` throughout all async adapter methods (`add_async`, `search_async`, `get_async`, `update_async`, `delete_async`, `list_memories_async`, `delete_all_async`, `get_history_async`, `close`).

---

### App Run Procedure (Unchanged)

The run procedure remains the same as before:

```powershell
# Terminal 1 — build and run (from autodev/ directory)
cd src-tauri
cargo build
cd ..
npm run tauri dev
```

**Sidecars auto-start on app launch.** On startup, AutoDev:
1. Probes `:8000` and `:4000` — marks them running if already up (e.g. you started them manually)
2. If not running: **automatically spawns the fleet** (no manual button click needed):
   - Starts memory-mcp-server, waits up to 20s for health check
   - Starts core-orchestrator, waits up to 30s for health check
   - Logs all startup events to console and `~/.autodev/logs/`
3. Once both are healthy, Mode 2 (LivingAgent) is active; workers use core-orchestrator

If either sidecar fails to start (path not found, port conflict, etc.), workers automatically fall back to Mode 1 (Rust AgentEngine). The "Start Fleet" button in the AutoDev tab can still be used to manually restart the sidecars if needed.

---

## 11. Phase 3 — Skills + Hooks + IDE + MCP Server + Plan Management

**Goal**: Skills system for Fleet workflows, Hooks for extensibility, IDE tab with file tree and code viewer, Git integration with checkpointing, AIRS MCP server, sidecar management, and Plan Management for complex multi-step tasks.

> **Priority Order**: Skills/Hooks (Fleet-critical) → IDE → Git → MCP/Sidecars → Plan Management

### Phase 3 Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Phase 3 Sub-Phase Dependencies                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Phase 3.1 ───────────────────────┐                                         │
│  Skills + Hooks System            │                                         │
│  (Fleet Foundation)               ▼                                         │
│                          Phase 3.2 ─────────────────────┐                   │
│                          IDE Core (File Tree + Viewer)  │                   │
│                                                         ▼                   │
│                                               Phase 3.3 ──────────────────┐ │
│                                               Git + Checkpointing          │ │
│                                                                            │ │
│  Phase 3.4 ────────────────────────────────────────────────────────────────┘ │
│  MCP Server + Sidecar Management                                             │
│       │                                                                    │
│       ▼                                                                    │
│  Phase 3.5 ────────────────────────────────────────────────────────────────│
│  Plan Management System                                                    │
│  (Uses all previous sub-phases)                                            │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### Phase 3.1 — Skills + Hooks System 🎯 **FLEET CRITICAL - HIGHEST PRIORITY**

**Goal**: Enable reusable prompt workflows (Skills) and event-driven extensibility (Hooks) for Fleet orchestration. Skills allow workers to follow standardized patterns. Hooks enable custom automation at key lifecycle points.

**Why First**: Fleet workers in Phase 2.2 need Skills for consistent execution patterns and Hooks for custom integrations (notifications, CI triggers, logging).

#### Skills System Architecture

**Core Concept**: `.md` files in `~/.autodev/skills/` containing templated workflows that can be invoked via slash commands or programmatically by workers.

**Backend — Skills Engine:**

**File Structure:**
- [x] `src-tauri/src/skills/mod.rs`: Skills module declaration
- [x] `src-tauri/src/skills/types.rs`: `Skill`, `SkillCategory`, `SkillParameter` structs
- [x] `src-tauri/src/skills/loader.rs`: `SkillLoader` with hot-reload capability
- [x] `src-tauri/src/skills/engine.rs`: `SkillEngine` for parameter substitution and execution
- [x] `src-tauri/src/skills/registry.rs`: `SkillRegistry` with category-based organization

**Skill Discovery & Loading:**
- [x] Scan `~/.autodev/skills/` on startup (file watcher deferred)
- [x] Recursive directory scanning: `~/.autodev/skills/{category}/{skill_name}.md`
- [x] YAML frontmatter parsing for metadata
- [x] Markdown body as skill template with `{{parameter}}` substitution
- [x] JSON schema validation for skill parameters

**Skill Schema (YAML Frontmatter):**
```yaml
---
name: feature-implementation
description: Standard workflow for implementing new features
category: coding  # coding, debugging, testing, review, deployment
parameters:
  - name: feature_name
    type: string
    required: true
    description: Name of the feature to implement
  - name: target_module
    type: string
    required: true
    description: Module or directory where feature belongs
  - name: include_tests
    type: boolean
    required: false
    default: true
    description: Whether to include test implementation
  - name: reasoning_effort
    type: enum
    values: [low, medium, high]
    default: medium
    description: Level of reasoning for implementation
version: "1.0.0"
author: "AutoDev"
tags: [backend, frontend, api]
---
```

**Tauri Commands:**
- [x] `list_skills(category_filter?) -> Vec<SkillSummary>`: List all available skills
- [x] `get_skill(name) -> SkillDetail`: Load full skill with template
- [x] `execute_skill(name, parameters) -> SkillExecutionResult`: Execute skill with parameter substitution
- [x] `validate_skill_parameters(name, parameters) -> ValidationResult`: Pre-validate before execution
- [x] `reload_skills() -> ()`: Force reload from disk (for development)
- [x] `import_skill(url_or_path) -> Result<Skill>`: Import from external source

**Skill Execution Flow:**
```
User: "/feature-implementation"
       │
       ▼
┌─────────────────┐
│ SkillRegistry   │──► Find skill by name
│   .get()        │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ SkillEngine     │──► Validate parameters against schema
│ .validate()     │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Parameter Sub   │──► Replace {{feature_name}} with actual value
│ .substitute()   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Agent Engine    │──► Inject templated prompt into context
│ .apply_skill()  │
└─────────────────┘
```

**Built-in Skill Categories:**

**1. Coding Skills (`~/.autodev/skills/coding/`):**
- [x] `feature-implementation.md`: Standard 5-step feature development
- [x] `api-endpoint.md`: Design → Implement → Test → Document flow
- [x] `database-migration.md`: Schema design → Migration → Validation
- [x] `refactoring.md`: Analyze → Plan → Execute → Verify pattern
- [x] `code-review.md`: Read → Analyze → Comment → Summarize workflow
- [x] `test-addition.md`: Identify → Write → Run → Fix cycle

**2. Debugging Skills (`~/.autodev/skills/debugging/`):**
- [x] `error-investigation.md`: Log analysis → Reproduction → Root cause → Fix
- [x] `performance-optimization.md`: Profile → Identify bottleneck → Optimize → Validate
- [x] `race-condition-debug.md`: Pattern recognition → Lock analysis → Fix → Stress test

**3. DevOps Skills (`~/.autodev/skills/devops/`):**
- [x] `docker-setup.md`: Dockerfile → Compose → Test → Push
- [x] `ci-pipeline-fix.md`: Diagnose → Patch → Test → Deploy
- [x] `dependency-update.md`: Check → Update → Test → Commit

**4. Fleet Skills (`~/.autodev/skills/fleet/`):** ⭐ Critical for Phase 2.2
- [x] `worker-task-execution.md`: Claim → Analyze → Execute → Report pattern
- [x] `supervisor-orchestration.md`: Poll → Assign → Monitor → Merge workflow
- [x] `qc-review.md`: Fetch → Review → Comment → Approve/Reject flow
- [x] `epic-breakdown.md`: Analyze → Decompose → Create subtasks → Assign

**Frontend — Skills UI:**

**Skills Palette (`src/components/skills/SkillsPalette.tsx`):**
- [x] `Cmd+Shift+S` shortcut to open skills palette (like VS Code command palette)
- [x] Search/filter by name, category, tags
- [x] Category sidebar: Coding | Debugging | DevOps | Fleet | Custom
- [x] Preview pane showing skill description and parameters
- [x] "Execute" button with parameter form modal
- [x] Recent skills section at top

**Skill Parameter Modal (`src/components/skills/SkillParameterModal.tsx`):**
- [x] Dynamic form generation from JSON schema
- [x] Validation messages per field
- [x] Required vs optional field indicators
- [x] Default value pre-population
- [x] Description tooltips for each parameter
- [x] "Execute" / "Cancel" buttons

**Skill Store (`src/stores/skillStore.ts`):**
- [x] State: `skills` (by category), `recentSkills`, `isLoading`, `executionHistory`
- [x] Actions: `loadSkills()`, `executeSkill()`, `searchSkills()`, `addToRecent()`

**Slash Command Integration:**
- [x] Auto-complete slash commands in chat input
- [x] `/skill-name` triggers skill palette pre-filtered
- [x] Tab completion for skill names
- [ ] Skills appear in tool suggestions (future: tool registry integration)

#### Hooks System Architecture

**Core Concept**: Event-driven extensibility allowing custom code execution at key lifecycle points. Inspired by Claude Code's PreToolUse/PostToolUse pattern and Git hooks.

**Backend — Hooks Engine:**

**File Structure:**
- [x] `src-tauri/src/hooks/mod.rs`: Hooks module declaration
- [x] `src-tauri/src/hooks/types.rs`: `Hook`, `HookType`, `HookCondition`, `HookResult` enums
- [x] `src-tauri/src/hooks/registry.rs`: `HookRegistry` with priority-based ordering
- [x] `src-tauri/src/hooks/executor.rs`: `HookExecutor` with sandboxed execution
- [x] `src-tauri/src/hooks/conditions.rs`: `HookConditionEvaluator` for conditional execution

**Hook Types (Event Triggers):**

**1. Agent Lifecycle Hooks:**
- [x] `SessionStart`: Triggered when new session created
- [x] `SessionEnd`: Triggered when session closed
- [x] `AgentLoopStart`: Before agent begins processing
- [x] `AgentLoopEnd`: After agent completes turn
- [x] `OnError`: When agent encounters error

**2. Tool Execution Hooks:**
- [x] `PreToolUse`: Before any tool executes (can modify inputs or block)
- [x] `PostToolUse`: After tool completes (can modify outputs)
- [x] `OnToolError`: When tool execution fails
- [x] `OnPermissionRequest`: When permission dialog shown

**3. File System Hooks:**
- [x] `PreFileRead`: Before read_file tool executes
- [x] `PostFileWrite`: After write_file or edit_file succeeds
- [x] `PreFileDelete`: Before file deletion (can prevent)
- [x] `OnCheckpointCreate`: When file checkpoint created
- [x] `OnCheckpointRestore`: When file restored from checkpoint

**4. Git Hooks:**
- [x] `PreCommit`: Before git commit executes
- [x] `PostCommit`: After commit succeeds
- [x] `PrePush`: Before git push
- [x] `OnBranchSwitch`: When switching branches
- [x] `OnMergeConflict`: When merge conflict detected

**5. Plan Management Hooks:**
- [x] `PrePlanStep`: Before executing tools for plan step
- [x] `PostPlanStep`: After plan step completes
- [x] `OnPlanActivate`: When plan moves draft → active
- [x] `OnPlanComplete`: When all plan steps done
- [x] `OnPlanAbandon`: When plan abandoned

**6. Fleet-Specific Hooks:** ⭐ Critical for Phase 2.2
- [x] `OnTaskClaim`: When worker claims task
- [x] `OnTaskComplete`: When worker completes task
- [x] `OnTaskFail`: When worker task fails
- [x] `OnQCReviewStart`: When QC begins review
- [x] `OnQCApprove`: When QC approves work
- [x] `OnQCFail`: When QC rejects work

**Hook Configuration (`~/.autodev/hooks/hooks.json`):**
```json
{
  "hooks": [
    {
      "id": "notify-on-completion",
      "type": "PostToolUse",
      "enabled": true,
      "priority": 100,
      "conditions": {
        "tool_name": "write_file",
        "file_pattern": "*.rs"
      },
      "action": {
        "type": "shell",
        "command": "notify-send 'Rust file modified' '{{file_path}}'"
      }
    },
    {
      "id": "auto-format-on-save",
      "type": "PostFileWrite",
      "enabled": true,
      "priority": 50,
      "conditions": {
        "file_pattern": "*.rs",
        "repo_path": "/home/user/rust-projects/*"
      },
      "action": {
        "type": "shell",
        "command": "cd {{repo_root}} && cargo fmt"
      }
    },
    {
      "id": "log-to-slack",
      "type": "OnTaskComplete",
      "enabled": false,
      "priority": 10,
      "action": {
        "type": "http",
        "url": "https://hooks.slack.com/services/...",
        "method": "POST",
        "headers": {
          "Content-Type": "application/json"
        },
        "body": {
          "text": "Task {{task_id}} completed by {{worker_id}}"
        }
      }
    },
    {
      "id": "prevent-dangerous-commands",
      "type": "PreToolUse",
      "enabled": true,
      "priority": 1000,
      "conditions": {
        "tool_name": "bash",
        "command_pattern": "(rm -rf /|dd if=/dev/zero|>:/)"
      },
      "action": {
        "type": "block",
        "message": "Dangerous command blocked: {{command}}"
      }
    }
  ]
}
```

**Hook Action Types:**
- [x] `shell`: Execute shell command with variable substitution
- [x] `script`: Execute Python/Node script file (stubbed, deferred)
- [x] `http`: HTTP POST/GET request with JSON body
- [x] `notification`: System notification (desktop)
- [x] `log`: Write to custom log file
- [x] `block`: Prevent action with optional message
- [x] `modify`: Change tool inputs before execution (stubbed, deferred)
- [x] `emit_event`: Custom Tauri event for frontend

**Hook Conditions:**
- [x] `tool_name`: Match specific tool
- [x] `file_pattern`: Glob pattern for file paths
- [x] `repo_path`: Only in specific repositories
- [x] `command_pattern`: Regex match for bash commands
- [x] `session_type`: PA vs Fleet sessions
- [x] `agent_mode`: FastDevelopment vs Review vs Delegate
- [x] `time_range`: Only during business hours
- [ ] `custom_condition`: JavaScript expression (future)

**Hook Execution Context:**
Hook actions receive context object:
```json
{
  "hook_type": "PostToolUse",
  "timestamp": "2026-01-15T10:30:00Z",
  "session_id": "sess_abc123",
  "agent_mode": "FastDevelopment",
  "tool_call": {
    "id": "call_123",
    "name": "write_file",
    "input": { "file_path": "/src/main.rs", "content": "..." },
    "result": { "success": true, "output": "File written" }
  },
  "repo": {
    "path": "/home/user/project",
    "name": "my-project",
    "branch": "feature-branch"
  },
  "plan": {
    "id": "plan_456",
    "current_step": 2,
    "total_steps": 5
  }
}
```

**Tauri Commands:**
- [x] `list_hooks() -> Vec<HookConfig>`: List all configured hooks
- [x] `enable_hook(hook_id) -> ()`: Enable a hook
- [x] `disable_hook(hook_id) -> ()`: Disable a hook
- [x] `add_hook(config) -> Result<HookConfig>`: Add new hook
- [x] `remove_hook(hook_id) -> ()`: Remove custom hook
- [x] `test_hook(hook_id, context) -> HookResult`: Test hook with mock context
- [x] `get_hook_logs(hook_id?, limit?) -> Vec<HookExecutionLog>`: View execution history

**Frontend — Hooks UI:**

**Hooks Settings Panel (`src/components/settings/HooksSettings.tsx`):**
- [x] List all hooks with toggle switches
- [ ] Drag-drop priority reordering (deferred)
- [x] "Add Hook" button (form-based, template gallery deferred)
- [x] Hook editor: form-based editor with action type fields and conditions
- [x] Test button with inline result (empty context)
- [x] Execution log viewer with timestamps

**Hook Log Viewer (`src/components/hooks/HookLogViewer.tsx`):**
- [x] Filter by outcome (All / Executed / Failed / Blocked / Skipped)
- [x] Expandable entries showing full output and error context
- [x] Error highlighting for failed hooks
- [x] Export logs button (JSON download)

#### Phase 3.1 Deliverables Checklist

**Backend — Skills System:**
- [x] `src-tauri/src/skills/mod.rs`: Module declaration
- [x] `src-tauri/src/skills/types.rs`: Skill, SkillCategory, SkillParameter structs
- [x] `src-tauri/src/skills/loader.rs`: SkillLoader with hot-reload
- [x] `src-tauri/src/skills/engine.rs`: SkillEngine for parameter substitution
- [x] `src-tauri/src/skills/registry.rs`: SkillRegistry with category organization
- [ ] Skills directory scanning with file watcher
- [x] YAML frontmatter parsing for skill metadata
- [x] Markdown template with `{{parameter}}` substitution
- [x] JSON schema validation for skill parameters
- [x] Tauri commands: `list_skills`, `get_skill`, `execute_skill`, `validate_skill_parameters`, `reload_skills`, `import_skill`
- [x] 20+ built-in skills across 5 categories (coding, debugging, devops, fleet, custom)
- [x] Skill execution history tracking

**Frontend — Skills System:**
- [x] `src/components/skills/SkillsPalette.tsx`: Command palette UI (Cmd+Shift+S)
- [x] `src/components/skills/SkillParameterModal.tsx`: Dynamic parameter forms
- [x] `src/stores/skillStore.ts`: Skill state management
- [x] Slash command auto-complete for skills
- [x] Recent skills tracking
- [x] Skills category browser

**Backend — Hooks System:**
- [x] `src-tauri/src/hooks/mod.rs`: Module declaration
- [x] `src-tauri/src/hooks/types.rs`: Hook, HookType, HookCondition, HookResult enums
- [x] `src-tauri/src/hooks/registry.rs`: HookRegistry with priority ordering
- [x] `src-tauri/src/hooks/executor.rs`: HookExecutor with sandboxing
- [x] `src-tauri/src/hooks/conditions.rs`: HookConditionEvaluator
- [x] 31 hook types across 6 categories (agent, tool, file, git, plan, fleet)
- [x] Action types: shell, http, notification, log, block, emit_event
- [x] Hook conditions: tool_name, file_pattern, repo_path, command_pattern, session_type, agent_mode, time_range
- [x] Tauri commands: `list_hooks`, `enable_hook`, `disable_hook`, `add_hook`, `remove_hook`, `test_hook`, `get_hook_logs`
- [x] Hooks configuration persistence in SQLite

**Frontend — Hooks System:**
- [x] `src/components/settings/HooksSettings.tsx`: Hook management UI
- [x] `src/components/hooks/HookLogViewer.tsx`: Execution log viewer
- [x] `src/stores/hookStore.ts`: Hook state management
- [ ] Hook template gallery (pre-built hooks)
- [ ] Hook testing interface with mock context

**Integration:**
- [x] Skills exposed in slash commands
- [x] Skills integrated with Plan Management (plan templates)
- [x] Hooks integrated with all tool executions
- [ ] Hooks integrated with Plan Management lifecycle
- [ ] Documentation: Skills authoring guide, Hooks reference

#### Phase 3.1 Success Criteria

> **Skill Test**: User types `/feature-implementation`, fills form with "add user auth" → PA follows 5-step skill template → Result is working auth implementation

> **Hook Test**: User configures `PostFileWrite` hook to auto-run `cargo fmt` → PA writes Rust file → Hook auto-formats → User sees formatted code

> **Fleet Integration**: Supervisor recipe uses `worker-task-execution` skill → Worker follows skill pattern → Task completes with standardized output

---

### Phase 3.2 — IDE Core (File Tree + Code Viewer)

**Goal**: Full IDE tab with file tree browser, code viewer with syntax highlighting, file search, and tab management.

**Why After Skills**: IDE is for human convenience; Skills enable Fleet automation which is higher priority.

#### Backend — File System & IDE Services

**File Structure:**
- [x] `src-tauri/src/ide/mod.rs`: IDE module declaration
- [x] `src-tauri/src/ide/types.rs`: Shared DTOs (FileTreeNode, FileContent, Symbol, …)
- [x] `src-tauri/src/ide/file_tree.rs`: File tree generation with metadata
- [x] `src-tauri/src/ide/file_reader.rs`: File reading with encoding detection + language id
- [x] `src-tauri/src/ide/file_search.rs`: Fast file search (name + content)
- [x] `src-tauri/src/ide/symbols.rs`: Symbol extraction (regex-based first cut, Rust/TS/JS/Python/Go — tree-sitter upgrade tracked as Phase 3.2.1)
- [x] `src-tauri/src/ide/workspace.rs`: Workspace state management (open tabs / expanded dirs persisted to `config.db`)

**Tauri Commands:** (registered in `commands/ide_commands.rs` + `lib.rs`)
- [x] `read_file_tree(path, depth?, ignore_patterns?) -> FileTreeNode`: Recursive directory listing (pagination via `depth` + `has_more`)
- [x] `read_file_content(path, start_line?, end_line?) -> FileContent`: Line-based pagination + encoding/language detection
- [x] `search_files(query, path?, glob?, max_results?) -> Vec<FileSearchResult>`: Fuzzy name search with subsequence fallback
- [x] `search_file_contents(query, path?, max_results?) -> Vec<ContentSearchResult>`: Case-insensitive grep-like content search
- [x] `get_file_metadata(path) -> FileMetadata`: Size, modified, readonly, git status
- [x] `get_symbol_tree(path) -> SymbolTree`: Parse file into symbols (functions, classes, etc.)
- [x] `get_ide_workspace_state() / set_ide_workspace_state(state)`: Persist open tabs + expanded dirs
- [ ] `resolve_symbol(path, line, column) -> SymbolDefinition`: Go to definition *(deferred — needs tree-sitter)*
- [ ] `find_symbol_references(path, symbol_name) -> Vec<SymbolReference>`: Find usages *(deferred — needs tree-sitter)*

**File Tree Features:**
- [x] Recursive directory scanning with depth limit (`depth` parameter, default 1)
- [x] Git status integration (modified, untracked, staged, deleted, conflicted — via git2)
- [x] Lazy loading for large directories (`has_more=true` placeholder, second call to expand)
- [x] Ignore pattern support (built-in skip list + caller-supplied `ignore_patterns`). `.gitignore` not yet parsed — deferred to Phase 3.3 alongside git work.
- [x] File type icons mapping (lucide-react — generic file/folder icons; extension-specific icons tracked for follow-up polish)
- [x] Directory expansion/collapse state persistence (`ide_expanded_dirs` in `app_settings`)

#### Frontend — IDE UI Components

**IDE Tab Layout:**
```
┌─────────────────────────────────────────────────────────────────────┐
│  Repo Selector [Repo ▼]  [Clone]  [Refresh]  [Search Files 🔍]     │
├──────────┬──────────────────────────────────────────────────────────┤
│          │  Tabs: [main.rs ✕] [lib.rs ✕] [Cargo.toml ✕] [+]
│  📁 src  │  ┌────────────────────────────────────────────────────┐
│  📄 main │  │ 1  │ fn main() {                                    │
│  📄 lib  │  │ 2  │     println!("Hello, AutoDev!");              │
│  📁 test │  │ 3  │ }                                              │
│  📄 ...  │  │    │                                                │
│          │  └────────────────────────────────────────────────────┘
│          │  [Git ▼] [Blame] [Outline ▼]  Line 1, Col 1   Rust
├──────────┴──────────────────────────────────────────────────────────┤
│  Status: main.rs modified (2 lines added)  Git: 2 files modified  │
└─────────────────────────────────────────────────────────────────────┘
```

**File Tree Component (`src/components/ide/FileTree.tsx`):**
- [x] Collapsible folder tree with chevron indicators
- [x] File icons based on type (lucide-react — generic file/folder today; extension-specific icons is polish work)
- [x] Git status badges (M/U/S/D/!)
- [ ] Context menu: Open, Copy Path, Delete, Rename, New File/Folder *(deferred — requires file-mutation commands)*
- [ ] Drag-and-drop file movement (within tree) *(deferred)*
- [ ] Filter/search within tree *(covered by Ctrl+P quick-open; inline filter is polish)*
- [ ] "Open Editors" section at top *(deferred)*
- [x] Recently opened files (surfaced in FileSearch when query is empty)

**Code Viewer Component (`src/components/ide/CodeViewer.tsx`):**
- [x] Monaco Editor integration (`@monaco-editor/react` ^4)
- [x] Syntax highlighting for 50+ languages via Monarch grammars (extension → monaco id mapping in `file_reader::detect_language`)
- [x] Read-only mode for safety (PA writes via tools, not direct editing)
- [x] Line numbers with current-line highlighting
- [x] Code folding support (Monaco default)
- [x] Minimap (overview) on right side
- [x] Scroll to line capability (via `OpenTab.gotoLine` + `editor.revealLineInCenter`)
- [x] "Open in IDE" jumps to specific line when `openFile(path, line)` is called

**Tab Management (`src/components/ide/TabBar.tsx`):**
- [x] Horizontal tab bar with file names
- [ ] Modified indicator *(N/A — viewer is read-only; marker will return if inline editing ships)*
- [x] Close button per tab (middle-click to close too)
- [x] Tab overflow handling (horizontal scroll container)
- [ ] Tab reordering via drag-drop *(deferred polish)*
- [x] `closeOtherTabs` / `closeAllTabs` actions exposed on the store *(context menu UI is polish)*
- [x] Tab state persistence across sessions (via `ide_open_tabs` + `ide_active_tab` keys)

**File Search (`src/components/ide/FileSearch.tsx`):**
- [x] `Ctrl+P` / `Cmd+P` quick file open (global keybinding in `tabs/IDE.tsx`)
- [x] Fuzzy search matching (subsequence + prefix/contains scoring)
- [x] Recent files shown when query is empty
- [x] File path preview in results
- [x] Keyboard navigation (↑/↓ + Enter + Esc)

**Symbol Outline (`src/components/ide/SymbolOutline.tsx`):**
- [x] Flat view of file symbols (functions, classes, structs, enums, traits, modules, constants, types)
- [x] Click to jump to symbol definition (uses `openFile(path, line)`)
- [x] Filter/search symbols (inline filter input)
- [ ] Sort by position or alphabetical toggle *(sorted by source order today — alphabetical toggle is polish)*

**IDE Store (`src/stores/ideStore.ts`):**
- [x] State: `openTabs`, `activeTab`, `rootTree`, `expandedDirs`, `recentFiles`, `searchOpen`
- [x] Actions: `init`, `refreshTree`, `loadDirChildren`, `toggleDir`, `openFile`, `closeTab`, `closeOtherTabs`, `closeAllTabs`, `switchTab`, `setSearchOpen`
- [x] Persists tabs + active tab + expanded dirs across restarts via `set_ide_workspace_state`

#### Phase 3.2 Deliverables Checklist

**Backend:**
- [x] **Workspace folder selection** — pre-implemented in Phase 2.3+: `workspace_commands.rs` (`get/set/pick_workspace_dir`), stored in `config.db`. IDE tab reads from the same config.
- [x] File tree generation with git status (scoped to `workspace_dir`)
- [x] File reading with encoding detection (UTF-8, UTF-16 LE/BE, BOM-aware, binary detection)
- [x] Fast file search by name (within `workspace_dir`)
- [x] File content search (walkdir + substring matching; swappable for ripgrep later)
- [x] Symbol extraction (regex-based first cut — tree-sitter upgrade planned as Phase 3.2.1)
- [x] File metadata retrieval
- [x] Workspace state management (single root today; multi-root stays deferred to Phase 4)

**Frontend:**
- [x] File tree component with icons and git badges
- [x] Monaco Editor code viewer (read-only)
- [x] Tab management with persistence
- [x] Quick file search (`Ctrl+P`)
- [x] Symbol outline panel
- [x] Git status integration in file tree
- [ ] Breadcrumb navigation *(deferred polish)*
- [ ] File context menu operations *(deferred — needs file-mutation commands)*

**Integration:**
- [x] `ideStore.openFile(path, gotoLine?)` API wired into chat tool-result renderers via `src/utils/openInIde.ts`
- [x] Click file path in chat to open in IDE — wired in `FileContentDisplay`, `GrepResultsDisplay` (per-match line jump), `GlobResultsDisplay`, `DirectoryListingDisplay`; also switches active top-level tab via new `src/stores/uiStore.ts`
- [x] IDE state sync with session (open tabs / active tab persisted via `set_ide_workspace_state`)

#### Phase 3.2 Success Criteria

> User opens IDE tab → Sees file tree with git status → Clicks `main.rs` → Code opens in Monaco viewer with Rust syntax highlighting → User opens `lib.rs` in new tab → Tabs persist across app restarts

---

### Phase 3.3 — Git Integration + Checkpointing

**Goal**: Full git integration in IDE, file checkpointing for undo support, git operations via `git2-rs`.

**Depends On**: IDE Core (needs file tree to show git status)

#### Backend — Git Module

**File Structure:**
- [x] `src-tauri/src/git/mod.rs`: Git module declaration
- [x] `src-tauri/src/git/repo.rs`: Repository operations (open, clone, init)
- [x] `src-tauri/src/git/status.rs`: File status (modified, staged, untracked)
- [x] `src-tauri/src/git/diff.rs`: Diff generation
- [x] `src-tauri/src/git/commit.rs`: Commit operations
- [x] `src-tauri/src/git/branch.rs`: Branch management
- [x] `src-tauri/src/git/worktree.rs`: Worktree operations
- [x] `src-tauri/src/git/remote.rs`: Push, pull, fetch
- [x] `src-tauri/src/git/checkpoint.rs`: File checkpointing system
- [x] `src-tauri/src/git/history.rs`: Commit history, blame
- [x] `src-tauri/src/git/stash.rs`: Stash management (added, spec said stash lives here)
- [x] `src-tauri/src/git/merge.rs`: Merge and conflict resolution (fast-forward + 3-way merge with conflict detection)

**Tauri Commands:**
- [x] `git_open_repo(path) -> RepoInfo`: Open repository
- [x] `git_clone(url, path, branch?) -> RepoInfo`: Clone repository
- [x] `git_status(repo_path) -> GitStatus`: Get file status
- [x] `git_diff(repo_path, file?, staged?) -> DiffResult`: Get diff
- [x] `git_stage(repo_path, files) -> ()`: Stage files
- [x] `git_unstage(repo_path, files) -> ()`: Unstage files
- [x] `git_discard(repo_path, files) -> ()`: Restore files from HEAD
- [x] `git_commit(repo_path, message, author?, trailer?) -> CommitInfo`: Commit with trailer
- [x] `git_push(repo_path, remote?, branch?) -> ()`: Push
- [x] `git_pull(repo_path, remote?, branch?) -> PullResult`: Pull (fast-forward)
- [x] `git_fetch(repo_path, remote?) -> ()`: Fetch
- [x] `git_branch_list(repo_path) -> Vec<BranchInfo>`: List branches
- [x] `git_branch_create(repo_path, name, checkout?) -> ()`: Create branch
- [x] `git_branch_delete(repo_path, name, force?) -> ()`: Delete branch
- [x] `git_checkout(repo_path, ref) -> ()`: Checkout branch/commit
- [x] `git_worktree_list(repo_path) -> Vec<WorktreeInfo>`: List worktrees
- [x] `git_worktree_add(repo_path, path, branch?) -> WorktreeInfo`: Add worktree
- [x] `git_worktree_remove(repo_path, path, force?) -> ()`: Remove worktree
- [x] `git_log(repo_path, max_count?, file?) -> Vec<CommitInfo>`: Commit history
- [x] `git_blame(repo_path, file, line?) -> Vec<BlameLine>`: Line annotations
- [x] `git_stash_list(repo_path) -> Vec<StashInfo>`: List stashes
- [x] `git_stash_push(repo_path, message?, include_untracked?) -> ()`: Create stash
- [x] `git_stash_apply(repo_path, stash_id, pop?) -> ()`: Apply stash
- [x] `git_stash_drop(repo_path, stash_id) -> ()`: Delete stash

**Checkpointing System:**
- [x] `create_checkpoint(repo_path, file_path, reason) -> CheckpointInfo`: Snapshot file
- [x] `restore_checkpoint(checkpoint_id) -> RestoreResult`: Restore from snapshot
- [x] `list_checkpoints(file_path?, session_id?) -> Vec<CheckpointInfo>`: List snapshots
- [x] `delete_checkpoint(checkpoint_id) -> ()`: Delete old checkpoint
- [x] `compare_checkpoints(checkpoint_id_1, checkpoint_id_2) -> DiffResult`: Compare versions
- [x] Auto-checkpoint before every `write_file` and `edit_file` tool call
- [x] Checkpoints stored in `~/.autodev/checkpoints/{session_id}/`
- [x] Retention policy: keep last 50 per file (`RETENTION_PER_FILE` constant in `checkpoint.rs`)

#### Frontend — Git UI Components

**Git Sub-Panel (`src/components/git/GitPanel.tsx`):**
- [x] Collapsible panel in IDE tab (toggle via [Git] button in the tab header)
- [x] Five tabs: Changes | History | Branches | Stash | Undo

**Changes Section:**
- [x] List modified files with status (M, A, D, ?, !)
- [x] Diff preview for selected file (unified diff)
- [x] Stage/Unstage buttons (per-row + bulk)
- [x] Discard changes button
- [x] "Stage all" / "Unstage all" bulk actions

**Staged Section (merged into Changes tab):**
- [x] List staged files
- [x] Diff preview (toggle between staged / unstaged)
- [x] Commit message textarea
- [x] "Commit" button
- [x] Commit attribution trailer editable, default `AutoDev/PA`

**History Section:**
- [x] Commit log list (author, date, summary, short oid)
- [x] Commit graph visualization (ASCII art format with oid | summary | author)
- [x] Diff for selected commit (commit_diff function in history.rs)
- [x] Checkout commit button (checkout_commit in branch.rs)
- [x] Create branch from commit (create_from_commit in branch.rs)
- [x] Merge branch button (merge function in merge.rs with conflict detection)

**Branch Manager (`src/components/git/BranchManager.tsx`):**
- [x] List local and remote branches (sectioned)
- [x] Current branch indicator (check icon + accent colour)
- [x] Create new branch button (with checkout)
- [x] Delete branch button (on hover)
- [x] Merge branch button (merge function in merge.rs with conflict detection)
- [x] Compare branches (ahead/behind counts displayed per row)

**Stash Manager (`src/components/git/StashManager.tsx`):**
- [x] List stashes with messages
- [x] Apply stash button (with/without pop)
- [x] Drop stash button
- [x] Create stash with custom message + include-untracked checkbox

**Checkpoint UI (`src/components/git/CheckpointViewer.tsx`):**
- [x] Timeline view of file checkpoints (grouped by file, most recent first)
- [x] Visual diff between versions (backend `compare_checkpoints` ready; UI hookup deferred)
- [x] Restore button with inline confirmation
- [ ] Compare any two checkpoints — UI two-select widget (deferred; backend ready)
- [x] Auto-cleanup: retention enforced in `checkpoint::prune` after each new snapshot

#### Phase 3.3 Deliverables Checklist

**Backend:**
- [x] Git repository operations (open, clone, init)
- [x] Git status with file categorization
- [x] Diff generation (unified diff format)
- [x] Stage/unstage operations
- [x] Commit with custom trailers (`Generated-by:` trailer injection)
- [x] Push, pull (ff), fetch operations
- [x] Branch management (create, delete, checkout)
- [x] Worktree operations (add, remove, list)
- [x] Commit history and log (with file-path filtering)
- [x] Git blame/annotation
- [x] Stash management
- [x] File checkpointing system (auto and manual)
- [x] Checkpoint storage and cleanup (retention 50 per file)

**Frontend:**
- [x] Git sub-panel with Changes / History / Branches / Stash / Undo
- [x] Diff viewer (unified-diff renderer — lighter-weight than Monaco DiffEditor)
- [x] Branch manager
- [x] Stash manager
- [x] Checkpoint timeline viewer
- [x] Git status in file tree (already landed in Phase 3.2 via `ide/file_tree.rs`)
- [x] Inline commit dialog (message + trailer + commit button)

**Integration:**
- [x] Auto-checkpoint on every file write (wired into `write_file.rs` and `edit_file.rs`)
- [x] "Undo" via Checkpoint Viewer (restore button; keyboard shortcut deferred)
- [x] Git trailers on all AutoDev commits (trailer field in commit UI, default `AutoDev/PA`)
- [x] Worktree support for task isolation (Fleet) — backend ready; Fleet integration lands in Phase 4

#### Phase 3.3 Success Criteria

> PA edits file → Auto-checkpoint created → File shows modified in git status → User clicks Git panel → Sees diff → Commits with trailer → Push succeeds → History shows commit

#### Phase 3.3 — Status: **COMPLETE** ✅

Implemented (summary):
- Rust `git/` module: 10 files (`mod`, `types`, `repo`, `status`, `diff`, `commit`, `branch`, `remote`, `worktree`, `history`, `stash`, `checkpoint`).
- Tauri commands registered in `commands/git_commands.rs` and `lib.rs` — 29 new commands.
- Auto-checkpoint hook wired into `write_file` and `edit_file` builtin tools.
- Frontend `stores/gitStore.ts` (Zustand) + `components/git/` (GitPanel, DiffViewer, BranchManager, StashManager, CheckpointViewer).
- Git toggle button in `tabs/IDE.tsx` header; panel opens as a right-side pane.

Deferred (out-of-scope for Phase 3.3):
- `git/merge.rs` — non-ff merge driver (pull only supports fast-forward; users can `git merge` manually).
- Commit graph visualization, diff-for-selected-commit, checkout-commit, create-branch-from-commit (history tab is currently a flat list).
- Merge-branch button in Branch Manager.
- Two-select checkpoint comparator UI (backend `compare_checkpoints` is ready).
- Keyboard shortcut for undo / file-level checkpoint restoration UI.

---

### Phase 3.3.1 — Startup Feedback + Agent ID Persistence

**Status**: **COMPLETE** ✅ (April 20, 2026)

**Goal**: Two closely-related stability/UX fixes that surfaced while running the full fleet end-to-end:

1. **Toast-driven startup loading UI** — the ~15 s fleet warm-up looked like a freeze; users had no visibility into what was happening.
2. **Remove ULID suffix from agent IDs** — every app restart was minting new agent YAMLs (`worker-backend-8mhrv0b1`, `worker-backend-9ehggj75`, …), severing mem0 memory continuity and polluting `config/agents/`.

Full design rationale, log evidence, and alternatives considered: [`startup_sequence_and_agent_persistence_analysis.md`](./startup_sequence_and_agent_persistence_analysis.md).

#### Part 1 — Startup progress FSM + overlay

- [x] `src-tauri/src/sidecar/startup_progress.rs` — new module: `StartupPhase` enum (12 phases), monotonic `StartupProgress::advance()`, `scan_line()` log-pattern dispatcher, Tauri `startup:progress` event emission.
- [x] `src-tauri/src/sidecar/mod.rs` — `spawn_log_router` threads `Option<Arc<StartupProgress>>` through; `SidecarManager` holds an attached tracker and advances it from `start_memory`/`start`/`mark_memory_running`/`mark_running`.
- [x] `src-tauri/src/main.rs` — creates `Arc<StartupProgress>` in the setup hook, emits `Init` + `Skills`, attaches it to `SidecarManager` before the sidecar auto-start task.
- [x] `src/stores/startupStore.ts` — Zustand store mirroring the Rust FSM (phase IDs kept in strict sync), subscribes to `startup:progress`, auto-dismisses 600 ms after `ready`.
- [x] `src/components/startup/StartupOverlay.tsx` + `.css` — full-screen modal: per-phase icons (lucide `Check` / `Loader2` / `Circle`), progress bar, percent + hint footer, Winter-is-Coming palette.
- [x] `src/App.tsx` — mounts `<StartupOverlay />` at the root of `.app-shell`.

**Phase ordering** (canonical, enforced by `advance()` monotonicity): `Init → Skills → MemoryStarting → VectorDb → MemoryReady → OrchStarting → Routers → Agents → LivingAgents → OrchReady → Workers → Ready`. The `Workers` transition is squeezed in inside `mark_running()` between `OrchReady` and `Ready` to avoid the scheduler auto-start (fixed +500 ms delay) racing ahead of the sidecar boot (~15 s).

#### Part 2 — Clean-name agent IDs (no UUID)

- [x] `submodules/core-orchestrator/api_gateway/modules/agents/service.py::generate_agent_id` — returns the clean slug only (no ULID suffix). Docstring explains delegation of duplicate prevention to `AgentRegistry.register` (in-memory uniqueness) and `AgentConfigWriter.config_exists` (disk-level uniqueness).
- [x] `submodules/core-orchestrator/api_gateway/modules/agents/service.py::AgentService.create_agent` — clearer error message referencing the update/delete endpoints on collision.
- [x] `submodules/core-orchestrator/scripts/migrate_to_clean_ids.py` — one-time legacy-file consolidation: for each known fleet worker (`supervisor`, `qc-agent`, `worker-*`), rename the newest UUID-suffixed YAML to `{worker}.yaml` (rewriting its inner `id` field) and archive the rest to `config/agents/.archive/`. Supports `--dry-run`.

**Verified-correct unchanged code paths**:
- `src/airs/config/auto_register.py::register_agent_metadata_only` already short-circuits on `AgentRegistry.get_agent(config.id)` — now matches exactly because `config.id` is a clean slug.
- `src/airs/agent/registry.py::register` already raises `ValueError` on duplicate names — this is the primary duplicate-prevention mechanism for the no-UUID scheme.
- `src-tauri/src/sidecar/client.rs::find_or_create_agent` exact-name match (`if name == slug`) now succeeds on first restart after migration.

#### User action required

```powershell
cd autodev/submodules/core-orchestrator
uv run python scripts/migrate_to_clean_ids.py --dry-run   # preview
uv run python scripts/migrate_to_clean_ids.py             # execute
# Then restart AutoDev; the overlay walks through all 12 phases.
```

#### Verification

- `cargo check --lib` → 0 errors, 2 pre-existing warnings (`grep.rs`, `git/repo.rs`), unrelated.
- `npx tsc --noEmit` → 3 pre-existing errors (`SkillCreatorModal`, `SkillsPalette`, `hookStore`), unrelated; no new errors introduced.

---

### Phase 3.4 — MCP Server + Sidecar Management + Python Cleanup

**Goal**: Replace the duplicated Python `autodev_handler.py` (864 lines of reimplemented Rust tools) with an **AutoDev MCP Server** that exposes the canonical Rust `builtin_tools` via the Model Context Protocol using **Streamable HTTP transport**. Fleet workers (core-orchestrator LivingAgents) connect as MCP clients over HTTP. Sidecar management provides lifecycle control for all managed processes.

**Problem Being Solved**: Currently, `builtin_tools` (Rust) and `autodev_handler.py` (Python) are **two parallel implementations** of the same 10 tools. Every schema change, status addition (e.g. `build_failed`), or validation rule must be ported to both sides. The Python side has already drifted (e.g. `VALID_WORKERS` is hardcoded in both `assign_task.rs` and `autodev_handler.py`). This is a single-source-of-truth violation.

**Depends On**: Skills/Hooks (Fleet needs these patterns)

---

#### Step 1 — Add MCP Dependencies (`src-tauri/Cargo.toml`)

**File**: `src-tauri/Cargo.toml`

Add to `[dependencies]`:
```toml
# MCP protocol (official Rust SDK)
rmcp = { version = "0.16", features = ["server", "transport-streamable-http-server"] }

# HTTP framework for Streamable HTTP transport
axum = "0.8"
```

`rmcp` provides the `ServerHandler` trait, `#[tool]`/`#[tool_router]` macros, and Streamable HTTP transport. `axum` routes the `/mcp` endpoint.

---

#### Step 2 — Build the AutoDev MCP Server Module

**Core Design**: Instead of rewriting tools with `#[tool]` macros (which would duplicate the existing `Tool` trait implementations), we create a **bridge struct** that implements `rmcp::ServerHandler` and dynamically maps MCP tool calls to our existing `dyn Tool` trait objects. This means:

- **No new tool implementations** — `read_file`, `bash`, `list_tasks`, etc. are the existing `.rs` files in `builtin_tools/`
- **Schema discovery at runtime** — `Tool::name()`, `Tool::description()`, `Tool::input_schema()` feed into `tools/list`
- **Execution delegation** — MCP `call_tool` → `tool.execute(input, &ToolContext)` → `ToolResult` → MCP `CallToolResult`

**File**: `src-tauri/src/mcp_server/mod.rs`

```rust
pub mod server;
pub mod tool_adapter;
```

**File**: `src-tauri/src/mcp_server/tool_adapter.rs`

Bridge struct wrapping `ToolRegistry`:
- `impl ServerHandler for AutodevMcpServer` — responds to `tools/list` by iterating `ToolRegistry` and converting each `Tool` to an MCP `Tool` object
- `call_tool()` handler — looks up tool by name, deserializes args from MCP `CallToolRequest`, constructs a synthetic `ToolContext` (working_dir = `~/.autodev/`), calls `tool.execute()`, converts `ToolResult` to `CallToolResult`
- `get_info()` — returns server name `"autodev-mcp-server"`, version `"1.0.0"`

**Key**: `ToolContext::session_id` can be a generated UUID per-MCP-call since there's no Tauri session in the sidecar context. `working_dir` defaults to `~/.autodev/` or can be overridden by an MCP Root.

**File**: `src-tauri/src/mcp_server/server.rs`

Streamable HTTP transport setup (rmcp 2025-03-26 standard):
```rust
use rmcp::transport::streamable_http_server::{StreamableHttpService, session::local::LocalSessionManager};
use axum::Router;

let service = StreamableHttpService::new(
    || Ok(AutodevMcpServer::new(tool_registry)),
    Arc::new(LocalSessionManager::default()),
);

let router = Router::new().route_service("/mcp", service);
let listener = tokio::net::TcpListener::bind("127.0.0.1:8767").await?;
axum::serve(listener, router).await?;
```

**Port**: `8767` — adjacent to core-orchestrator (4000) and memory-mcp-server (8000). Bound to `127.0.0.1` only.

---

#### Step 3 — Register MCP Server as a Managed Sidecar

**File**: `src-tauri/src/sidecar/manager.rs` (new or existing)

Add `autodev-mcp-server` to the `SidecarManager`'s managed processes list:
- **Command**: internal Tauri binary spawns the MCP server as an in-process Tokio task (no separate binary needed since it shares the `autodev_lib` crate)
- **Alternative**: spawn as separate process if we want isolation
- **Auto-start**: on app launch, after `core-orchestrator` is healthy
- **Health check**: `GET /health` on `127.0.0.1:8767` (a lightweight liveness endpoint, not part of MCP spec but useful for sidecar manager)
- **Restart**: on failure, with 5s exponential backoff (max 60s)

**File**: `src-tauri/src/sidecar/config.rs`

Add sidecar config entry:
```rust
SidecarConfig {
    name: "autodev-mcp-server",
    port: 8767,
    auto_start: true,
    restart_policy: RestartPolicy::ExponentialBackoff { base: 5, max: 60 },
}
```

---

#### Step 4 — Reconfigure Fleet Workers to Use MCP (Rust Side)

**File**: `src-tauri/src/sidecar/client.rs` — `ensure_agent()` create body, lines 407-423

**BEFORE** (delete):
```rust
"autodev_tools": [
    "read_file__autodev",
    "write_file__autodev",
    "edit_file__autodev",
    "bash__autodev",
    "list_directory__autodev",
    "grep__autodev",
    "glob__autodev",
    "list_workers__autodev",
    "list_tasks__autodev",
    "update_task_status__autodev",
    "assign_task__autodev",
    "create_task__autodev",
]
```

**AFTER**:
```rust
"mcp_servers": [
    {
        "id": "autodev-mcp-server",
        "url": "http://127.0.0.1:8767/mcp",
        "transport": "streamable-http"
    }
]
```

This tells core-orchestrator's LivingAgent: "Connect to the AutoDev MCP server at this URL, discover tools via `tools/list`, and route all tool calls through MCP."

---

#### Step 5 — Python Cleanup (Delete Duplication)

**5a. DELETE** `submodules/core-orchestrator/src/airs/bridges/autodev_handler.py` (864 lines)

Entire file is a reimplementation of Rust builtin_tools + task board logic. The MCP server now provides these tools via protocol. No local Python execution needed.

**5b. MODIFY** `submodules/core-orchestrator/src/airs/bridges/unified_tool_handler.py`

Remove all `__autodev` routing:
- **Line 24**: Delete `self.autodev_handler: Optional[...] = None`
- **Line 31**: Delete `self.autodev_tools: Set[str] = set()`
- **Lines 87-105**: Delete `register_autodev_handler()` method
- **Lines 118-119**: Delete `__autodev` suffix check in `_identify_tool_handler()`
- **Lines 178-180**: Delete `autodev_handler` routing branch in `handle_tool_call()`
- **Lines 208-211**: Delete `autodev_handler` entry from `get_handler_status()`

Rationale: MCP tools are handled by the existing `mcp_handler` (line 137, 191). No special `__autodev` routing needed — the `autodev-mcp-server` is just another MCP server from core-orchestrator's perspective.

**5c. MODIFY** `submodules/core-orchestrator/src/airs/config/agent_schema.py` — lines 140-146

Delete:
```python
autodev_tools: List[str] = Field(
    default_factory=list,
    description="AutoDev tool IDs forwarded from Tauri for fleet workers..."
)
```

Rationale: Agents connect to MCP servers via the existing `mcp_servers` field (line 55). The `autodev-mcp-server` is configured there.

**5d. MODIFY** `submodules/core-orchestrator/src/airs/models/agent.py` — lines 66-72

Delete:
```python
autodev_tools: List[str] = Field(
    default_factory=list,
    description="AutoDev tool names forwarded from Tauri..."
)
```

Rationale: Agent runtime loads tools from MCP server discovery at init time, not from a static list.

**5e. MODIFY** `submodules/core-orchestrator/src/airs/config/agent_writer.py`

Delete:
- **Lines 177-179**: `autodev_tools` YAML writing block
- **Line 331**: `autodev_tools: Optional[List[str]] = None` parameter from `create_agent_config()`
- **Line 431**: `autodev_tools=autodev_tools or []` from `AgentConfig` constructor call

Rationale: Agent YAML populates `mcp_servers` list instead (already handled at lines 388-393).

---

#### Step 6 — Verify Core-Orchestrator MCP Client Compatibility

**File**: `submodules/core-orchestrator/src/airs/bridges/mcp_bridge.py` (or equivalent MCP client)

Core-orchestrator already has an MCP client manager (used for `memory-mcp-server`, `github-mcp`, etc.). Verify it supports:
- **Streamable HTTP transport** (POST + GET on same endpoint) — required for `autodev-mcp-server`
- **Tool discovery** via `tools/list` — the client must issue this request after connecting
- **Tool call forwarding** — client calls `tools/call` with the tool name and JSON arguments

If core-orchestrator's MCP client is SSE-only (legacy), add a task to upgrade it to Streamable HTTP or add a compatibility layer.

---

### Phase 3.4 — Implementation Status: **COMPLETE**

**Verification**:
- [x] `cargo check --lib` → **0 errors**, 2 pre-existing warnings (`grep.rs::context_lines`, `git/repo.rs::to_repo_relative`), unrelated to Phase 3.4.

**Steps**:
- [x] **Step 1** — `rmcp` (0.16) + `axum` (0.8) added to `src-tauri/Cargo.toml`
- [x] **Step 2** — MCP server module created (`mcp_server/mod.rs`, `tool_adapter.rs`, `server.rs`) with `AutodevMcpServer` bridging `ToolRegistry` to `rmcp::ServerHandler`
  - Streamable HTTP transport on `127.0.0.1:8767/mcp`
  - Dynamic `tools/list` from `ToolRegistry::all()` using `Tool::name`, `Tool::description`, `Tool::input_schema`
  - `tools/call` dispatches to canonical `Tool::execute()` with synthetic `ToolContext` (`working_dir = ~/.autodev`)
- [x] **Step 3** — `SidecarManager` updated with `mcp_status` tracking; `fleet_is_ready()` includes MCP server; MCP server starts as in-process Tokio task from `main.rs`
- [x] **Step 4** — `sidecar/client.rs` changed `autodev_tools` → `mcp_servers: ["autodev-mcp-server"]`, added `OrchestratorClient::register_mcp_server()` (POST to `/api/mcp/servers`, idempotent on 409) invoked from `main.rs` after core-orchestrator is healthy
- [x] **Step 5** — Python cleanup complete:
  - `autodev_handler.py` **DELETED** (864 lines)
  - `unified_tool_handler.py` — `register_autodev_handler`, `__autodev` routing, `autodev_handler`, `autodev_tools` all removed
  - `agent_schema.py` (`AgentConfig`) — `autodev_tools` field removed
  - `agent.py` (`AgentMetadata`) — `autodev_tools` field removed
  - `agent_writer.py` — `autodev_tools` YAML write block + function param removed
  - `living_agent.py` — STEP 4.7 AutoDev Tool Handler init block removed, `self.autodev_tool_handler` field removed
  - `factory.py` — `autodev_tool_names` extraction + `autodev_tools=` constructor arg removed
  - `api_gateway/modules/agents/models.py` — `AgentCreate.autodev_tools` field removed
  - `api_gateway/modules/agents/service.py` — `autodev_tools=` argument removed from `create_agent_config_from_request()`
- [x] **Step 6** — Core-orchestrator MCP client verified compatible: `MCPBridge.get_tools()` and `MCPClientManager` support server discovery via `mcp_servers` list; `MCPService.add_server()` handles dynamic registration via `/api/mcp/servers`.

**Key design decisions applied**:
1. **No tool re-implementation** — existing Rust `Tool` trait objects are reused; schemas discovered at runtime from `Tool::input_schema()`.
2. **In-process MCP server** — shares the `autodev_lib` crate, no separate binary, no IPC overhead.
3. **Single source of truth** — `ToolRegistry` in Rust is the only place tools are defined; Python side is purely a client.
4. **Idempotent registration** — `register_mcp_server()` returns Ok on HTTP 409, so app restart is safe.

**Remaining follow-up (out of Phase 3.4 scope)**:
- End-to-end smoke test: start Tauri app → confirm `autodev-mcp-server` binds :8767 → confirm core-orchestrator `/api/mcp/servers/autodev-mcp-server` exists → spawn a fleet worker → verify `tools/list` returns the 11 builtin tools.
- `tsc --noEmit` verification (no TypeScript files were touched in this phase).

---

### Backend — Sidecar Management (Phase 3.4b)

**Sidecar Manager Module:**
- [x] `src-tauri/src/sidecar/mod.rs`: `SidecarManager` consolidates memory-mcp-server, core-orchestrator, and (Phase 3.4) in-process `autodev-mcp-server` tracking.
- [x] `src-tauri/src/sidecar/client.rs`: `OrchestratorClient` handles auth, agent CRUD, `register_mcp_server()`.
- [x] `src-tauri/src/sidecar/startup_progress.rs`: Startup phase tracker wired to the UI.
- [x] Health check via `OrchestratorClient::is_healthy()` polled during startup retry loop (main.rs).
- [ ] Dedicated `SidecarConfig`/`process.rs`/`health.rs` split (deferred — current single-file design works; revisit when adding redis/ollama).

**Managed Sidecars:**
- [x] `core-orchestrator`: Python FastAPI server (port 4000) — Fleet orchestration, now an MCP **client** to `autodev-mcp-server`
- [x] `memory-mcp-server`: Python MCP server (port 8000) — Memory management
- [x] `redis`: Redis / Memurai (port 6379) — **Phase 3.4b** tracked via `SidecarManager.redis_status` with `OrchestratorClient::is_redis_healthy()` (raw RESP PING, no dependency). Optional: absence disables warm memory tier only; does NOT block fleet readiness. Externally managed (Memurai Windows service / system redis); AutoDev only probes liveness.
- [x] `autodev-mcp-server`: Rust MCP server (port 8767) — Exposes canonical `builtin_tools` via Streamable HTTP **(NEW, Phase 3.4)**

**Sidecar Lifecycle:**
- [x] Auto-start on app launch (memory → orchestrator → autodev-mcp-server)
- [x] Process spawn with stdout/stderr capture → `~/.autodev/logs/{memory-mcp,core-orchestrator}.log`
- [x] Health check loop during startup retry
- [ ] Auto-restart on failure with exponential backoff (deferred — app restart currently required)
- [x] Graceful shutdown on app exit (`stop_all()`)
- [ ] Log rotation (deferred)

**Tauri Commands:**
- [x] Sidecar status commands exposed to frontend via `startup_progress` and existing `sidecar` commands in `commands/` modules (see `lib.rs` registrations)
- [ ] Dedicated `start_sidecar`/`stop_sidecar`/`restart_sidecar` runtime controls (deferred — not required for Phase 3.4 success criteria)

---

#### Phase X — Ollama Provider (Local LLM)

> Reference: `docs/reference/ollamainrust.md` — comprehensive `ollama-rs` 0.3.4 guide.
> Out of Phase 3.4 scope; implement when local/self-hosted inference is prioritized.

---

##### 1. Dependencies & Crate Setup

- [x] Add `ollama-rs = { version = "0.3.4", features = ["stream", "chat-history"] }` to `src-tauri/Cargo.toml`
- [x] Add `src-tauri/src/provider/ollama.rs` module file (full Provider trait impl)
- [x] Re-export in `src-tauri/src/provider/mod.rs`: `pub mod ollama;` + `pub use ollama::OllamaProvider;`
- [ ] Verify `cargo check --lib` — user to run

##### 2. Provider Struct & Configuration

- [x] Define `OllamaProvider` struct with `host`, `port`, `client: ollama_rs::Ollama`
- [x] `OllamaProvider::new(host: String, port: u16) -> Self`
- [x] `OllamaProvider::default_from_env()` reads `OLLAMA_HOST` env var, falls back to `localhost:11434`

##### 3. Dynamic Model Discovery

- [x] `OllamaProvider::fetch_models()` → `client.list_local_models().await` mapped to `Vec<ModelInfo>` with family-based `context_window`, `supports_vision`, cost=0.0, skips embedding models
- [x] §3.1 Context-window defaults table implemented in `context_window_for_model(name: &str)` covering llama3.x, mistral, codellama, deepseek-coder, qwen2.5, llava, phi3/4, gemma, command-r — fallback 4096
- [ ] Cache model list for 60s — deferred (no UI dropdown polling yet)

##### 4. Message Format Mapping (our types ↔ ollama-rs)

- [x] `to_ollama_messages()` helper maps `RequestMessage` → `Vec<ChatMessage>`:
  - System → `ChatMessage::system`, User text → `ChatMessage::user`, Assistant text → `ChatMessage::assistant`
  - `MessageContent::ToolResult` mapped via `ChatMessage::tool(content)` where available; fallback JSON-embedded user annotation
  - `MessageContent::ToolUse` in assistant messages serialized as inline annotation text (Ollama raw chat has no structured tool_call replay)
- [x] Multi-part content blocks joined with `\n`

##### 5. Tool / Function Calling Mapping

- [x] `to_ollama_tools()` helper stub commented with correct type mapping (`ToolDefinition` → `ollama_rs::generation::tools::Tool`)
- [ ] Raw chat `.tools()` builder on `ChatMessageRequest` — deferred until ollama-rs stabilizes the API (currently exposed via `Coordinator` only)
- [x] Tool streaming caveat handled: `stream()` detects `!request.tools.is_empty()` and falls back to `complete()` → single-chunk stream (`TextDelta` + `MessageDelta` + `Done`)

##### 6. Non-Streaming Completion (`complete()`)

- [x] `Provider::complete()` builds `ChatMessageRequest` via `build_chat_request()`:
  - Maps messages + `GenerationOptions` (temperature, top_p, num_ctx, num_predict)
  - Calls `client.send_chat_messages(request).await`
  - Maps `ChatMessageResponse` → `CompletionResponse` (UUID message_id, text content, stop_reason from `done_reason`, tokens from `prompt_eval_count`/`eval_count`)
- [x] Error mapping: `OllamaError` → `AutoDevError::Provider(String)` via `map_ollama_err()`

##### 7. Streaming Completion (`stream()`)

- [x] `Provider::stream()` returns `Result<StreamResult>`:
  - Non-tool path: `send_chat_messages_stream()` → `futures::StreamExt::map()` → `StreamChunk::TextDelta` per partial content + `StreamChunk::MessageDelta` on `done=true` + `StreamChunk::Done`
  - Tool path: falls back to non-streaming (see §5)
- [x] Tracing `debug!` per chunk

##### 8. Connection Testing

- [x] `Provider::test_connection()` → `list_local_models()`:
  - Success + models → `Connected`
  - Success + empty → `Connected` + `warn!("No models pulled yet...")`
  - Error → `ProviderStatus::Error("Ollama not reachable at {host}:{port}")`

##### 9. Provider Registration (All Dispatch Sites)

- [x] `provider/mod.rs` — `pub mod ollama;` + re-export
- [x] `settings_commands.rs` — `"ollama"` in `build_provider_info()` (static models fallback) + `test_provider_connection()` (no vault key)
- [x] `agent_commands.rs` — `"ollama"` dispatch arm + conditional vault skip (no API key needed)
- [x] `recipe_commands.rs` — `"ollama"` dispatch arm + conditional vault skip
- [x] `recipes/runner.rs` — import + `"ollama"` dispatch arm + conditional vault skip
- [x] `provider/streaming.rs` — N/A (provider-agnostic SSE parser, not used by Ollama direct stream)

##### 10. Frontend — Provider Selection UI

- [ ] `src/components/settings/ProviderSettings.tsx` (or existing provider config component):
  - Add "Ollama" to provider list with distinct UI:
    - No API key input field (local, no auth)
    - Show host:port input (default `localhost:11434`, editable)
    - "Test Connection" button calls `test_provider_connection("ollama")`
    - On success, show model dropdown populated by `list_local_models` equivalent (or fetch from backend)
- [ ] Model dropdown for Ollama:
  - Refresh button to re-fetch `ollama list`
  - Show context window size next to each model
  - Filter out embedding-only models (`nomic-embed-text`, etc.)
- [ ] Visual indicator: "Local" badge on Ollama provider card (distinguish from cloud providers)

##### 11. Context Window & Parameter Configuration

- [ ] Per-model context window auto-populated from hardcoded family table (§3.1)
- [ ] Allow user override of `num_ctx` in provider settings (advanced toggle)
- [ ] Temperature slider: mapped to `ModelOptions::temperature(0.0–2.0)`
- [ ] Top-P slider: mapped to `ModelOptions::top_p(0.0–1.0)`
- [ ] Seed input: mapped to `ModelOptions::seed()` for reproducible outputs
- [ ] `num_predict` (max tokens): default `-1` (unlimited), allow user override

##### 12. Known Limitations & Deferred Items

- [ ] **Vision models**: `supports_vision` flag set but no image input API wired yet (Ollama vision requires base64 image data in message content; our `MessageContent` enum has no image variant yet)
- [ ] **Embedding models**: Not exposed as chat providers; skip in model list. Future RAG integration may use `ollama-rs` embeddings API directly.
- [ ] **Quantization detection**: `ollama list` does not expose quantization level (Q4, Q8, etc.). Deferred — quantization info is not available via ollama-rs public API as of 0.3.4.
- [ ] **Reasoning/thinking mode**: No ollama model supports extended thinking like Claude. `supports_thinking` stays `false`.
- [ ] **Tool streaming**: As noted in ollama-rs docs, tool-call streaming is incomplete. Non-streaming tool path is the reliable default.

---

**Models Initially Supported** (all families covered in `models_static()` + `context_window_for_model()`; any Ollama-pulled model works dynamically via `fetch_models()`):
- [x] Llama 3.1 / 3.2 / 3.3 (8B, 70B) — 128K ctx, general chat + tools (3.1+)
- [x] Mistral 7B / Mixtral 8x7B — 32K ctx, general chat
- [x] CodeLlama 7B/13B/34B — 16K ctx, code generation
- [x] DeepSeek Coder / DeepSeek (general) — 16K / 64K ctx
- [x] Qwen 2.5 (7B–72B) — 32K ctx, best for tool calling
- [x] Phi-3 / Phi-4 — 128K ctx
- [x] Gemma — 8K ctx
- [x] Command-R — 128K ctx
- [x] Custom models via `ollama pull <any>` — dynamically discovered via `fetch_models()`

---

#### Frontend — Sidecar UI

**Sidecar Panel (`src/components/sidecar/SidecarPanel.tsx`):**
- [x] List all sidecars with status indicators — 4 services shown (memory, orch, autodev-mcp, redis)
- [x] Start/Stop/Restart buttons per sidecar — wired for memory-mcp-server & core-orchestrator only (autodev-mcp is in-process, redis is external)
- [x] Health status (green/yellow/red) — tone-coded icons: CheckCircle2 (aqua), Loader2 spin (warning), AlertTriangle (danger), Circle (idle)
- [x] Resource usage (CPU, memory) — click service name → modal → Resources tab shows CPU% + memory bytes via `sysinfo` crate (`get_resource_usage` command)
- [x] Log viewer with tail -f behavior — click service name → modal → Logs tab shows last 500 lines of sidecar-specific log file (`read_sidecar_log` command)
- [ ] Configuration editor — deferred (no user-configurable settings per user request)

**Settings Integration:**
- [ ] Sidecar auto-start toggle
- [ ] Resource limits (memory, CPU)
- [ ] Port configuration
- [ ] Log level selection

#### Phase 3.4 Deliverables Checklist

**AutoDev MCP Server (core Phase 3.4 scope):**
- [x] MCP server module (`src-tauri/src/mcp_server/{mod,server,tool_adapter}.rs`)
- [x] Tool adapter deriving schemas from Rust `Tool` trait (via `ToolRegistry::all()`)
- [x] All canonical builtin_tools exposed via MCP protocol (runtime-discovered, no hardcoding)
- [x] Streamable HTTP transport on `127.0.0.1:8767/mcp` (rmcp 0.16, axum 0.8)
- [x] Per-request `ToolContext` with synthetic session/call IDs and `~/.autodev` working directory
- [x] Integration with `workers.db` and task board via existing Rust `Tool` trait implementations

**Python Cleanup (core Phase 3.4 scope):**
- [x] `autodev_handler.py` deleted (864 lines removed)
- [x] `unified_tool_handler.py` — `__autodev` routing removed
- [x] `agent_schema.py` (`AgentConfig`) — `autodev_tools` field removed
- [x] `agent_writer.py` — `autodev_tools` YAML writing + constructor param removed
- [x] `agent.py` (`AgentMetadata`) — `autodev_tools` field removed
- [x] `living_agent.py` — AutoDev Tool Handler init block removed
- [x] `factory.py` — `autodev_tools` extraction + metadata assignment removed
- [x] `api_gateway/modules/agents/models.py` — `AgentCreate.autodev_tools` removed
- [x] `api_gateway/modules/agents/service.py` — `autodev_tools=` argument removed
- [x] `sidecar/client.rs` — agent creation uses `mcp_servers: ["autodev-mcp-server"]` + `register_mcp_server()`

**Sidecar Management (core Phase 3.4 scope):**
- [x] `autodev-mcp-server` registered with `SidecarManager.mcp_status`
- [x] Startup order enforced: memory-mcp → core-orchestrator → autodev-mcp-server registration
- [x] `fleet_is_ready()` requires all three services healthy
- [ ] Auto-restart with exponential backoff (deferred)
- [ ] Log rotation (deferred)

**Ollama Provider** (out of Phase 3.4 scope — see atomic breakdown in new "Phase X — Ollama Provider" section above)

**Frontend (Phase 3.4b — implemented)**:
- [x] `src/components/sidecar/SidecarPanel.tsx` — live status grid for all 4 services (memory, orchestrator, autodev-mcp, redis) with tone-coded icons, manual refresh, and 10 s auto-poll. Distinguishes required vs optional services.
- [x] `src/components/sidecar/McpToolBrowser.tsx` — searchable list of tools exposed by `autodev-mcp-server` with collapsible JSON input-schema view. Powered by `list_mcp_tools` Tauri command (direct `ToolRegistry` iteration, no JSON-RPC round-trip).
- [x] Mounted in `Settings` tab as two new sections: "Services" and "MCP Tools", after "Demo Mode".
- [ ] Dedicated sidecar configuration editor (port/auto-start per service) — deferred (Demo Mode toggle covers the only current use case).

**New Tauri commands (Phase 3.4b)**:
- [x] `get_full_sidecar_status` → returns `Vec<ServiceStatus>` with id/name/status/port/required/note for all 4 services
- [x] `list_mcp_tools` → returns `Vec<McpToolInfo>` straight from the managed `Arc<ToolRegistry>`
- [x] `check_sidecar_connectivity` extended to also PING Redis and update `SidecarManager.redis_status`

**Core-orchestrator configuration (Phase 3.4b)**:
- [x] `autodev_mcp_config.yaml` — `mcp-autodev` server entry added alongside `mcp-memory`
- [x] `mcp_config.yaml` — `mcp-autodev` server entry added alongside `mcp-adobe` + `mcp-memory`
- [x] `config/mcp/mcp-autodev.yaml` — full extended definition with skills/tags/metadata (mirrors `mcp-adobe.yaml` pattern)
- [x] `.env` — `MCP_AUTODEV_HOST=localhost`, `MCP_AUTODEV_PORT=8767`, `MCP_AUTODEV_PATH=/mcp` added

**Integration (core Phase 3.4 scope):**
- [x] Fleet agent creation payload uses `mcp_servers: ["autodev-mcp-server"]` (no more `autodev_tools`)
- [x] `OrchestratorClient::register_mcp_server()` auto-registers autodev-mcp-server with core-orchestrator on startup
- [x] Core-orchestrator `MCPBridge` / `MCPClientManager` supports Streamable HTTP MCP discovery
- [ ] Live end-to-end validation with a running worker (deferred — requires full app smoke test)

#### Phase 3.4 Success Criteria

> Fleet worker starts → Connects to AIRS MCP Server → Discovers `read_file`, `bash`, `list_tasks`, `assign_task` via `tools/list` → Calls `assign_task` → Rust canonical implementation executes → Task board updates → AutoDev panel reflects change
>
> AND: `autodev_handler.py` no longer exists → `__autodev` suffix no longer used → Tool schema changes in Rust automatically propagate to fleet agents via MCP discovery

---

### Phase 3.5 — Plan Management System (Planner Compiler Architecture)

**Goal**: PA **and Fleet agents** can create, track, and manage plans for complex multi-step tasks via a unified planner compiler. Full planning capabilities for Supervisor (epic breakdown), Workers (sub-plans for complex tasks), and QC (verification plans).

**Architecture**: Unified — Single `planner` tool implemented entirely in AutoDev Tauri Rust, exposed to all agents (PA and Fleet) via the in-process **autodev-mcp-server** (Streamable HTTP on `127.0.0.1:8767`). No split between generic and domain-specific layers; the compiler is AutoDev-native by design.

**Depends On**: All previous sub-phases (uses Skills for plan templates, IDE for file viewing, Git for checkpointing, MCP for Fleet coordination)

**When to Use**: When PA or any agent determines a query requires 5+ steps or 10+ tool calls, it creates a plan first.

---

#### Architecture Overview

**Design Philosophy**: Single `planner` tool with YAML DSL compiler, following the Adobe compiler pattern. Reduces LLM cognitive load from 10+ tools to 1 tool with action-based routing. The tool is a first-class `builtin_tools` implementation (like `create_task`, `update_task_status`) and is automatically exposed to all LivingAgents through the already-running `autodev-mcp-server`.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  UNIFIED PLAN ARCHITECTURE (AutoDev Tauri — Rust only)                        │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  PA / Supervisor / Worker / QC (LivingAgent via core-orchestrator)    │   │
│  │  ─────────────────────────────────────────────────────────────────    │   │
│  │  • LLM calls `planner` tool via MCP tools/list → tools/call          │   │
│  │  • Same YAML DSL for every agent (personal plans + fleet plans)      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                              │                                               │
│                              │ MCP Streamable HTTP (127.0.0.1:8767)        │
│                              ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  AUTODEV-MCP-SERVER (in-process, started in main.rs)                  │   │
│  │  ─────────────────────────────────────────────────────────────────    │   │
│  │  • rmcp ServerHandler exposes all registered Tools                   │   │
│  │  • ToolAdapter bridges Tool::execute → rmcp call_tool               │   │
│  │  • PlannerTool registered alongside create_task, list_tasks, etc.   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                              │                                               │
│                              ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  PlannerTool (src/agent/builtin_tools/planner.rs)                     │   │
│  │  ─────────────────────────────────────────────────────────────────    │   │
│  │  • YAML DSL parser → action router (match on "action" key)          │   │
│  │  • Direct rusqlite CRUD on plans.db                                   │   │
│  │  • Emits Tauri events: plan:created, plan:updated, ...             │   │
│  │  • Triggers hooks: PrePlanStep, PostPlanStep, OnPlanComplete         │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                              │                                               │
│                              ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  SQLite (~/.autodev/plans.db) — plans, plan_items,                   │   │
│  │  plan_item_dependencies, plan_executions                              │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Why No Hybrid / No core-orchestrator Python Layer:**
- The `autodev-mcp-server` is already operational (`logs/completerun.log` line 23: `[autodev-mcp-server] Listening on http://127.0.0.1:8767/mcp`).
- Core-orchestrator's `airs_mcp.client` discovered it and registered it alongside `mcp-adobe`, `mcp-autodev`, `mcp-memory`.
- Adding a Python generic planner in `core-orchestrator` would introduce an HTTP bridge, two codebases, and serialization overhead for zero external consumers (AutoDev has no standalone services).
- The data model is inherently AutoDev-specific (`task_id` FK to `tasks.db`, `worktree_branch` for git2-rs, `assigned_worker` slugs matching recipe runners). A generic compiler pretending to be agnostic is architectural theatre.

---

#### Plan Data Model

**SQLite Schema (`~/.autodev/plans.db`):**

`session_id` is **not** a foreign key to `sessions(id)` because Fleet agents (Supervisor, Workers) operate outside interactive PA sessions. For PA-created plans it stores the session UUID; for Fleet plans it stores `"fleet"` or the worker recipe-run correlation ID.

```sql
-- Plans table (extended for Fleet planning)
CREATE TABLE plans (
    id TEXT PRIMARY KEY,
    session_id TEXT,          -- PA session UUID or "fleet" sentinel (NOT FK — agents lack sessions)
    title TEXT NOT NULL,
    description TEXT,
    status TEXT CHECK(status IN ('draft', 'active', 'completed', 'abandoned')),
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    assigned_agent TEXT,      -- null for PA, 'supervisor', 'worker-backend', etc.
    created_by TEXT,          -- 'PA', 'supervisor', 'worker-{slug}', 'qc'
    agent_scope TEXT CHECK(agent_scope IN ('personal', 'fleet', 'epic')),
    parent_plan_id TEXT,      -- For nested plans (worker sub-plan of supervisor epic)
    task_id INTEGER,          -- Optional link to tasks.db entry
    FOREIGN KEY (parent_plan_id) REFERENCES plans(id) ON DELETE CASCADE
);

-- Plan items (steps) - extended for Fleet execution
CREATE TABLE plan_items (
    id TEXT PRIMARY KEY,
    plan_id TEXT NOT NULL,
    parent_id TEXT, -- for nested sub-steps
    title TEXT NOT NULL,
    description TEXT,
    status TEXT CHECK(status IN ('pending', 'in_progress', 'done', 'blocked')),
    order_index INTEGER NOT NULL,
    agent_type TEXT CHECK(agent_type IN ('PA', 'Supervisor', 'Worker', 'QC')),
    estimated_tools TEXT, -- JSON array of tool names
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME,
    -- Fleet execution fields
    assigned_worker TEXT, -- Worker slug who claimed this item
    worktree_branch TEXT, -- Git worktree for isolation
    pr_url TEXT, -- Link to created PR when done
    result_summary TEXT, -- Completion summary
    FOREIGN KEY (plan_id) REFERENCES plans(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_id) REFERENCES plan_items(id)
);

-- Dependencies between plan items
CREATE TABLE plan_item_dependencies (
    item_id TEXT NOT NULL,
    depends_on_item_id TEXT NOT NULL,
    FOREIGN KEY (item_id) REFERENCES plan_items(id) ON DELETE CASCADE,
    FOREIGN KEY (depends_on_item_id) REFERENCES plan_items(id) ON DELETE CASCADE,
    PRIMARY KEY (item_id, depends_on_item_id)
);

-- Plan execution log
CREATE TABLE plan_executions (
    id TEXT PRIMARY KEY,
    plan_id TEXT NOT NULL,
    started_at DATETIME,
    completed_at DATETIME,
    total_items INTEGER DEFAULT 0,
    completed_items INTEGER DEFAULT 0,
    error_log TEXT,
    FOREIGN KEY (plan_id) REFERENCES plans(id) ON DELETE CASCADE
);
```

---

#### Planner Tool — Rust Implementation

**Module:** `src-tauri/src/agent/builtin_tools/planner.rs`

The `PlannerTool` implements the existing `Tool` trait (`name`, `description`, `input_schema`, `execute`). It is registered in `builtin_tools/mod.rs` alongside `create_task`, `list_tasks`, etc., and exposed through the `autodev-mcp-server` (port 8767) exactly like every other builtin tool. Fleet agents discover it via standard MCP `tools/list` and call it via `tools/call` — no special opt-in, no internal tool bridge, no HTTP round-trip to core-orchestrator.

**Registration (all PA modes):**
```rust
// In builtin_tools/mod.rs
PAMode::FastDevelopment | PAMode::Review | PAMode::Delegate => {
    registry.register(Arc::new(planner::PlannerTool));
    // ... existing file tools, task tools, etc.
}
```
Planning is task management, not file I/O — it is safe and desirable in **Delegate Mode** so the PA can create a plan for the Supervisor to break down without violating the org chart.

**Tool Interface:**
```rust
pub struct PlannerTool;

impl Tool for PlannerTool {
    fn name(&self) -> &str { "planner" }

    fn description(&self) -> &str {
        r#"Unified planning interface with YAML DSL compiler.

## Available Actions

Use 'action: listActions' to discover all actions, or execute directly:

### Basic Actions
- listActions: Get schema for all actions
- create: Create new plan
- addStep: Add step to plan
- updateStep: Update step status
- getPlan: Get plan details
- listPlans: List plans
- checkDependencies: Check which steps are unblocked
- executeTemplate: Create plan from template
- deletePlan: Delete plan
- abandonPlan: Abandon plan

### Fleet Actions (for workers/supervisors)
- breakDownEpic: Convert epic to plan
- claimItem: Claim next available step
- createSubplan: Create nested plan
- submitForQC: Submit for review

## When to Use

Create a plan when task requires 5+ steps or 10+ tool calls.

## YAML DSL Syntax

action: create
title: "Your plan title"
description: "What this plan accomplishes"
scope: personal  # or fleet, epic
"#
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "yaml_dsl": {
                    "type": "string",
                    "description": "YAML DSL command. Use 'action: listActions' to discover available actions."
                }
            },
            "required": ["yaml_dsl"]
        })
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        let yaml_dsl = args["yaml_dsl"].as_str()
            .ok_or_else(|| anyhow!("Missing yaml_dsl parameter"))?;

        let result = PlannerCompiler::new(ctx)
            .compile(yaml_dsl)
            .await
            .map_err(|e| format!("Planner error: {e}"));

        ToolResult {
            output: serde_json::to_string(&result).unwrap_or_default(),
            is_error: result.is_err(),
        }
    }
}
```

**YAML DSL Compiler (Action Router):**
```rust
pub struct PlannerCompiler<'a> {
    ctx: &'a ToolContext,
    repo: PlanRepository,
}

impl<'a> PlannerCompiler<'a> {
    pub async fn compile(&self, yaml_dsl: &str) -> Result<Value> {
        let plan_data: serde_yaml::Value = serde_yaml::from_str(yaml_dsl)
            .map_err(|e| anyhow!("Invalid YAML DSL: {e}"))?;

        let action = plan_data.get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' field in YAML DSL"))?;

        match action {
            "listActions"        => self.list_actions(),
            "create"             => self.create_plan(plan_data).await,
            "addStep"            => self.add_step(plan_data).await,
            "updateStep"         => self.update_step(plan_data).await,
            "getPlan"            => self.get_plan(plan_data).await,
            "listPlans"          => self.list_plans(plan_data).await,
            "checkDependencies"  => self.check_dependencies(plan_data).await,
            "executeTemplate"    => self.execute_template(plan_data).await,
            "deletePlan"         => self.delete_plan(plan_data).await,
            "abandonPlan"        => self.abandon_plan(plan_data).await,
            "breakDownEpic"      => self.breakdown_epic(plan_data).await,
            "claimItem"          => self.claim_item(plan_data).await,
            "createSubplan"      => self.create_subplan(plan_data).await,
            "submitForQC"        => self.submit_for_qc(plan_data).await,
            _ => Err(anyhow!(
                "Unknown action: '{}'. Use listActions to discover valid actions.",
                action
            )),
        }
    }
}
```

**YAML DSL Examples (identical for all agents):**

```yaml
# Learn available actions (discover schema)
action: listActions
include_fleet: true

# Create a plan
action: create
title: "Implement OAuth2 flow"
description: |
  Add OAuth2 authentication to the API.
  Includes token generation, refresh, and validation.
scope: personal
session_id: "sess_abc123"

# Add steps to a plan
action: addStep
plan_id: "plan_xyz789"
title: "Create token endpoint"
description: "Implement /auth/token endpoint with JWT generation"
depends_on: []  # No dependencies for first step
estimated_tools: ["write_file", "edit_file"]

# Add step with dependency
action: addStep
plan_id: "plan_xyz789"
title: "Add refresh token logic"
description: "Implement token refresh mechanism"
depends_on: [0]  # Depends on first step (by index)
estimated_tools: ["edit_file"]

# Update step status
action: updateStep
plan_id: "plan_xyz789"
step_index: 0
status: "in_progress"
notes: "Starting implementation of token endpoint"

# Mark step done
action: updateStep
plan_id: "plan_xyz789"
step_index: 0
status: "done"
notes: "Token endpoint implemented and tested"

# Get plan status
action: getPlan
plan_id: "plan_xyz789"
include_items: true

# List active plans
action: listPlans
scope: personal
session_id: "sess_abc123"
status_filter: active

# Check dependencies
action: checkDependencies
plan_id: "plan_xyz789"

# Execute from template
action: executeTemplate
template_name: "feature-implementation"
session_id: "sess_abc123"
custom_title: "Add OAuth2 flow"

# --- Fleet-specific actions ---

# Supervisor breaks down epic
action: breakDownEpic
epic_task_id: 123
analysis_depth: thorough
supervisor_id: "supervisor"

# Worker claims next available item
action: claimItem
plan_id: "plan_fleet456"
worker_id: "worker-backend"

# Worker creates subplan
action: createSubplan
parent_plan_id: "plan_fleet456"
worker_id: "worker-backend"
title: "Database migration subplan"
description: "Migrate user table to support OAuth2 fields"
estimated_steps: 5

# Worker submits for QC
action: submitForQC
plan_id: "plan_fleet456"
worker_id: "worker-backend"
result_summary: "OAuth2 flow implemented, all tests passing"
pr_urls: ["https://github.com/.../142"]
```

---

#### Rust Backend Modules

**Module Structure:**
- [x] `src-tauri/src/db/plans.rs` — `PlansDb` with schema init + CRUD (uses `rusqlite`, opens `~/.autodev/plans.db`)
- [x] `src-tauri/src/db/mod.rs` — Add `plans_db` alongside `tasks_db`, `config_db`, `workers_db`
- [x] `src-tauri/src/plan/mod.rs` — Re-export module (types embedded in plans.rs)
- [x] `src-tauri/src/agent/builtin_tools/planner.rs` — `PlannerTool` + `PlannerCompiler` action router

**PlanRepository (simplified):**
```rust
pub struct PlanRepository {
    conn: rusqlite::Connection,
}

impl PlanRepository {
    pub fn new() -> Result<Self> {
        let path = dirs::home_dir()
            .ok_or_else(|| anyhow!("No home dir"))?
            .join(".autodev/plans.db");
        let conn = rusqlite::Connection::open(path)?;
        Self::init_schema(&conn)?;
        Ok(Self { conn })
    }

    // CRUD
    pub fn create_plan(&self, plan: &NewPlan) -> Result<Plan>;
    pub fn get_plan(&self, id: &str) -> Result<Option<Plan>>;
    pub fn list_plans(&self, filter: PlanFilter) -> Result<Vec<PlanSummary>>;
    pub fn add_step(&self, plan_id: &str, step: &NewPlanItem) -> Result<PlanItem>;
    pub fn update_step(&self, item_id: &str, status: PlanItemStatus, notes: Option<&str>) -> Result<()>;
    pub fn delete_plan(&self, id: &str) -> Result<()>;

    // Fleet-specific
    pub fn claim_next_item(&self, plan_id: &str, worker_id: &str) -> Result<Option<PlanItem>>;
    pub fn check_dependencies(&self, item_id: &str) -> Result<Vec<bool>>;
    pub fn get_plan_tree(&self, plan_id: &str) -> Result<PlanTree>;
    pub fn submit_for_qc(&self, plan_id: &str, worker_id: &str, summary: &str, pr_urls: &[String]) -> Result<()>;
}
```

**Tauri Commands:**
- [x] `planner_compile(yaml_dsl: String) -> Result<String>`
  - Main entry point — all plan operations go through this
  - Direct Rust execution (zero HTTP round-trip)
  - Emits UI events on completion
- [x] `get_plan(plan_id: String) -> Result<Plan>`
  - Direct SQLite query
- [x] `list_plans(filter: PlanFilter) -> Result<Vec<Plan>>`
  - Direct SQLite query with filters
- [x] `get_plan_tree(plan_id: String) -> Result<PlanTree>`
  - Recursive query for parent + all nested subplans
- [x] `delete_plan(plan_id: String) -> Result<bool>`
  - Delete plan and cascade to items

**Fleet Tauri Commands:**
- [x] `break_down_epic(epic_task_id: i64, supervisor_id: String) -> Result<Plan>`
- [x] `claim_plan_item(plan_id: String, worker_id: String) -> Result<Option<PlanItem>>`
- [x] `create_subplan(parent_plan_id: String, worker_id: String, title: String, description: String) -> Result<Plan>`
- [x] `submit_plan_for_qc(plan_id: String, worker_id: String, result_summary: String, pr_urls: Vec<String>) -> Result<bool>`

---

#### MCP Discovery & Agent Configuration

**No Internal Tool Opt-In Required.** The `planner` tool is exposed through the `autodev-mcp-server` alongside `create_task`, `list_tasks`, `read_file`, etc. Fleet agents discover it automatically via standard MCP `tools/list` — no `internal_tools` declarations in agent YAML, no `planner__internal` naming convention, no `InternalToolBridge`.

**Agent YAML (MCP-only):**
```yaml
id: supervisor
name: Supervisor Agent
description: Orchestrates epic breakdown and worker coordination
job_title: Fleet Supervisor
gender: neutral

llm:
  provider: openai
  model: gpt-4o
  temperature: 0.2

system_prompt: |
  You are the Fleet Supervisor. Your role is to:
  1. Break down epic tasks into executable plans
  2. Coordinate workers via plan assignment
  3. Monitor progress and resolve dependencies

# MCP servers — autodev-mcp-server exposes planner + task tools + file tools
mcp_servers:
  - id: autodev-fleet
    uri: http://127.0.0.1:8767/mcp

capabilities:
  streaming: true
  auto_retry: true
```

**How It Works (MCP Path):**
1. **Registration**: `PlannerTool` registered in `ToolRegistry` during `builtin_tools::register_all()`
2. **Exposure**: `AutodevMcpServer::list_tools()` iterates `registry.all()` and maps each `Tool` to `rmcp::model::Tool`
3. **Discovery**: Core-orchestrator's `airs_mcp.client` fetches `tools/list` from `http://127.0.0.1:8767/mcp`
4. **Call**: LLM emits `tools/call` with `name: "planner"`, arguments `{"yaml_dsl": "action: create..."}`
5. **Execution**: `AutodevMcpServer::call_tool()` looks up `PlannerTool` in registry → calls `execute()` → `PlannerCompiler::compile()` → direct rusqlite → Tauri events emitted

**Naming:**
- MCP tool name: `planner` (no `__internal` suffix — the MCP server is the single bridge)
- Tool call from LLM: `{"name": "planner", "arguments": {"yaml_dsl": "action: create..."}}`

---

#### System Prompt Integration

**Updated BASE_SYSTEM_PROMPT Guidance:**
```
For complex multi-step tasks (5+ steps or 10+ tool calls):

1. FIRST: Discover planner actions
   planner:
     yaml_dsl: "action: listActions"

2. THEN: Create plan
   planner:
     yaml_dsl: |
       action: create
       title: "Your plan title"
       description: "What you need to accomplish"
       scope: personal

3. ADD STEPS: 3-7 steps via addStep action
   planner:
     yaml_dsl: |
       action: addStep
       plan_id: "{plan_id_from_create}"
       title: "Step title"
       description: "What this step does"
       depends_on: []  # or [0, 1] for dependencies

4. UPDATE PROGRESS: As you complete steps
   planner:
     yaml_dsl: |
       action: updateStep
       plan_id: "{plan_id}"
       step_index: 0
       status: done
       notes: "Completed successfully"

Plan step status meanings:
- pending: Not started yet
- in_progress: Currently working on this
- done: Step completed successfully
- blocked: Waiting on dependencies or external input

Always reference the plan_id in your thinking.
```

#### Frontend UI Components

**Plan Sidebar Card:**
- [x] `src/components/sidebar/PlanCard.tsx`
  - Shows active plan title, progress bar, current step
  - **Agent ownership indicator**: Shows which agent owns the plan (PA / Supervisor / Worker / QC)
  - **Scope badge**: Personal / Fleet / Epic
  - Status indicator: 🟡 Draft / 🟢 Active / 🟡 In Progress / ✅ Complete / ⚪ Abandoned
  - Nested plan indicator: Shows parent/child relationship
  - Click to open full Plan view

**Plan Detail View:**
- [x] `src/components/plan/PlanView.tsx`
  - Collapsible step list with status icons
  - **Agent assignment column**: Shows which worker claimed each item
  - **Worktree/PR links**: Direct links to git branches and PRs per step
  - Drag-and-drop reordering (draft plans only, PA plans)
  - Dependency visualization (graph/tree) with unlock status
  - Notes field per step with **worker attribution**
  - **Context-aware action buttons by agent type:**
    - PA: [Activate] [Add Step] [Mark Done] [Abandon]
    - Supervisor: [Break Down Epic] [Assign to Worker] [Add Step] [Auto-Assign]
    - Worker: [Claim Step] [Mark Done] [Create Subplan] [Release Item]
    - QC: [Start Review] [Approve] [Reject] [Request Changes]

**Plan Chat Display:**
- [x] `src/components/chat/PlanDisplay.tsx`
  - Inline plan card when PA creates plan
  - Progress updates show as step completions
  - "Jump to Plan" button

**Plan Store:**
- [x] `src/stores/planStore.ts`
  - State: `activePlan`, `planHistory`, `isLoading`, `fleetPlans`, `planTree`
  - Actions: `loadPlan`, `updateStepStatus`, `createPlan`, `setActivePlan`
  - **Fleet actions:**
    - `claimPlanItem(itemId, workerId)` — Worker claims step
    - `createSubplan(parentId, workerId, title, description)` — Nested plan creation
    - `loadPlansForAgent(agentId, scope)` — Load plans by agent
    - `loadPlanTree(planId)` — Load plan + all nested subplans
    - `submitForQC(planId, workerId)` — Submit plan for review
  - Event listeners: `plan:created`, `plan:updated`, `plan:step_completed`, `plan:item_claimed`, `plan:subplan_created`
#### Fleet Plan Execution Flow (via Planner Compiler)

**Supervisor Epic Breakdown:**
```
1. SUPERVISOR POLL CYCLE
   └─ Sees epic task #123: "Refactor auth system"
   └─ Calls planner with YAML DSL:
      action: breakDownEpic
      epic_task_id: 123
      supervisor_id: "supervisor"
      analysis_depth: thorough
   └─ Compiler creates Plan #P1 with agent_scope='epic', created_by='supervisor'
   └─ Auto-generates 6 plan_items:
      ├─ Step 1: Analyze current auth (agent_type='Supervisor')
      ├─ Step 2: Design new auth flow → agent_type='Worker', assigned_worker=null
      ├─ Step 3: Update database schema → agent_type='Worker', depends_on=[2]
      ├─ Step 4: Update API endpoints → agent_type='Worker', depends_on=[3]
      ├─ Step 5: Update frontend → agent_type='Worker', assigned_worker='worker-frontend'
      └─ Step 6: Integration tests → agent_type='Worker', depends_on=[3,4,5]
```

**Worker Subplan Creation:**
```
2. WORKER-BACKEND CLAIM & SUBPLAN
   └─ Polls planner with claimItem action:
      action: claimItem
      plan_id: "P1"
      worker_id: "worker-backend"
   └─ Gets Step 2 assigned (status='in_progress', assigned_worker='worker-backend')
   └─ Step 2 is complex (multi-file refactor)
   └─ Calls planner with createSubplan:
      action: createSubplan
      parent_plan_id: "P1"
      worker_id: "worker-backend"
      title: "Auth middleware refactor"
      description: "Migrate JWT handling to new middleware pattern"
      estimated_steps: 3
   └─ Creates Subplan #P2 with parent_plan_id='P1', agent_scope='fleet'
   └─ Subplan items:
      ├─ Sub-step 2.1: Read current auth code
      ├─ Sub-step 2.2: Write new auth middleware (worktree_branch='worker/auth-refactor')
      └─ Sub-step 2.3: Update unit tests
   └─ Completes all subplan items via updateStep actions
   └─ Updates parent step via updateStep with pr_url
   └─ Subplan #P2 auto-completed when parent item done
```

**Dependency Unlock & QC Handoff:**
```
3. DEPENDENCY CASCADE & QC
   └─ Step 2 done → Step 3 dependencies now met → Status 'pending' → 'ready'
   └─ Worker-Backend claims Step 3 via claimItem action...
   └─ All steps done → Worker submits for QC:
      action: submitForQC
      plan_id: "P1"
      worker_id: "worker-backend"
      result_summary: "OAuth2 flow implemented, all tests passing"
      pr_urls: ["https://github.com/.../142"]
   └─ QC Agent creates verification plan #P3 via create action
   └─ QC reviews all PRs, approves → Supervisor merges → Closes epic
```

**Key Benefit:** Fleet agents use the SAME `planner` tool with different actions, reducing cognitive load and enabling the LLM to learn one interface for all planning operations.

#### Phase 3.5 Deliverables Checklist

**AutoDev Backend — Unified Planner (Rust only):**
- [x] `src-tauri/src/db/plans.rs` — `PlansDb` with schema init + CRUD (`rusqlite`, opens `~/.autodev/plans.db`)
- [x] `src-tauri/src/db/mod.rs` — Add `plans_db` alongside `tasks_db`, `config_db`, `workers_db`
- [x] `src-tauri/src/plan/mod.rs` — Re-export module for plan types
- [x] `src-tauri/src/agent/builtin_tools/planner.rs` — `PlannerTool` + `PlannerCompiler` action router
- [x] `builtin_tools/mod.rs` — Register `PlannerTool` in all PA modes (FastDevelopment, Review, Delegate)
- [x] `autodev-mcp-server` already exposes it — no extra registration needed beyond `register_all()`
- [x] Tauri commands:
  - [x] `planner_compile(yaml_dsl)` — Main entry point (direct Rust, zero HTTP)
  - [x] `get_plan(plan_id)` — Direct SQLite query
  - [x] `list_plans(filter)` — Direct SQLite query
  - [x] `get_plan_tree(plan_id)` — Recursive query for nested plans
- [x] Fleet Tauri commands:
  - [x] `break_down_epic(epic_task_id, supervisor_id)`
  - [x] `claim_plan_item(plan_id, worker_id)`
  - [x] `create_subplan(parent_plan_id, worker_id, title, description)`
  - [x] `submit_plan_for_qc(plan_id, worker_id, result_summary, pr_urls)`

**Agent Configuration (YAML — MCP-only):**
- [x] Agent YAML examples with `mcp_servers` pointing to `http://127.0.0.1:8767/mcp`:
  - [x] Agents discover `planner` via MCP tools/list automatically
  - [x] No `internal_tools` declarations needed — planner is a standard MCP tool
  - [x] Naming: MCP tool name `planner`, LLM call `{"name":"planner","arguments":{"yaml_dsl":"..."}}`
  - Note: Existing agent YAML files (supervisor, workers, qc) already have `mcp_servers` configured

**Planner Actions (Compiler Handlers):**
- [x] `listActions` — Discover all actions and schemas
- [x] `create` — Create new plan
- [x] `addStep` — Add step to existing plan
- [x] `updateStep` — Update step status/properties
- [x] `getPlan` — Get plan details and progress
- [x] `listPlans` — List active/completed plans
- [x] `checkDependencies` — Check which dependencies are satisfied
- [x] `executeTemplate` — Execute plan from YAML template
- [x] `deletePlan` — Delete plan
- [x] `abandonPlan` — Mark plan as abandoned
- [x] Fleet-specific actions:
  - [x] `breakDownEpic` — Supervisor converts epic to plan
  - [x] `claimItem` — Worker claims next available item
  - [x] `createSubplan` — Worker creates nested plan
  - [x] `submitForQC` — Worker submits for review

**Frontend UI Components:**
- [x] `src/components/sidebar/PlanCard.tsx` — Active plan with progress, ownership badge, scope badge
- [x] `src/components/plan/PlanView.tsx` — Detail view with collapsible steps, dependency viz, action buttons
- [x] `src/components/chat/PlanDisplay.tsx` — Inline plan card in chat
- [x] `src/components/plan/PlanTreeView.tsx` — Nested plan tree (parent/child)
- [x] Fleet UI features:
  - [x] Agent ownership indicator (PA/Supervisor/Worker/QC badge)
  - [x] Scope badge (Personal/Fleet/Epic) with color coding
  - [x] Agent filter: Show only items assigned to specific worker
  - [x] Context-aware action buttons by agent type
  - [x] Worktree branch links and PR URL displays

**Plan Store:**
- [x] `src/stores/planStore.ts` with state: `activePlan`, `planHistory`, `fleetPlans`, `planTree`
- [x] Actions: `loadPlan`, `updateStepStatus`, `compilePlanner(yaml_dsl)`, `setActivePlan`
- [x] Fleet actions: `claimPlanItem()`, `createSubplan()`, `loadPlansForAgent()`, `loadPlanTree()`, `submitForQC()`
- [x] Event listeners: `plan:created`, `plan:updated`, `plan:step_completed`, `plan:item_claimed`, `plan:subplan_created`

**Fleet Plan Success Criteria:**
> **Supervisor** detects epic task → Uses planner with `action: breakDownEpic` → Creates plan with steps → **Workers** claim items via `action: claimItem` → Complex work spawns subplans via `action: createSubplan` → **QC** creates verification plan → All plans complete with proper dependency resolution → Epic closed

**Key Architecture Win:**
> All planning operations (PA personal plans, Supervisor epic breakdown, Worker subplans, QC verification) use the SAME `planner` tool with YAML DSL actions. Reduced from 10+ tools to 1 tool with self-documenting `listActions` discovery.

---

### Phase 3 Dependency Graph

```
Phase 3 Sub-Phase Dependencies
═══════════════════════════════════════════════════════════════════════════════

Phase 3.1 (Skills + Hooks)
│
├─► [CRITICAL PATH] Fleet Foundation
│   ├─ worker-task-execution skill
│   ├─ supervisor-orchestration skill
│   ├─ OnTaskClaim/Complete/Fail hooks
│   └─ OnQCReviewStart/Approve/Fail hooks
│
├─► [DEPENDENCY] Phase 3.2 IDE Core
│   └─ File tree with git status badges
│
├─► [DEPENDENCY] Phase 3.3 Git + Checkpointing
│   └─ PostFileWrite hooks for auto-format
│
├─► [DEPENDENCY] Phase 3.4 MCP + Sidecars
│   └─ Hooks emit events to sidecars
│
└─► [DEPENDENCY] Phase 3.5 Plan Management
    ├─ create_plan_from_template skill integration
    ├─ PrePlanStep/PostPlanStep hooks
    └─ Fleet skills for plan item claiming

═══════════════════════════════════════════════════════════════════════════════

Phase 3 Cross-Sub-Phase Integration Points:

┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Phase 3.1   │────►│   Phase 3.2   │────►│   Phase 3.3   │
│  Skills/Hooks   │     │  IDE Core       │     │  Git + Checkpt  │
│                 │     │                 │     │                 │
│ • /skills cmd   │     │ • File tree     │     │ • Git status    │
│ • Hooks exec    │────►│ • Monaco viewer │────►│ • Checkpointing │
│ • Fleet skills  │     │ • Tab mgmt      │     │ • Git hooks     │
└─────────────────┘     └─────────────────┘     └─────────────────┘
         │                       │                       │
         │                       │                       │
         └───────────────────────┴───────────────────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │   Phase 3.4     │
                    │ MCP + Sidecars  │
                    │                 │
                    │ • AIRS MCP srv  │
                    │ • Sidecar mgmt  │
                    │ • Ollama prov   │
                    └─────────────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │   Phase 3.5     │
                    │  Plan Management│
                    │                 │
                    │ • Plan creation │
                    │ • Step tracking │
                    │ • Fleet-ready   │
                    └─────────────────┘
```

---

### Phase 3.0 — Runtime Refinements (Fleet Flow Fixes) ✅ DONE

**Goal**: Eliminate the "choppy, drip-fed, one-task-at-a-time" fleet UX discovered during the *Calendar App* smoke-run (2026-04-21). Forensic analysis: `docs/development/runtime-fixes/phase3/fleet-flow-analysis.md`.

**Ethos:** The five symptoms the user reported trace back to *four* real root causes (latency of scheduler-only wakeups, missing push status, missing session-bound PA mode, and Windows sidecar log noise). Every fix here is a minimal upstream change — no placeholders, no legacy fallbacks, fully event-driven.

#### Refinement R1 — Push-based fleet dispatch ⭐ FLEET-CRITICAL ✅

- [x] **Global `RecipeScheduler` handle**: add `OnceLock<Arc<RecipeScheduler>>` in `recipes/scheduler.rs` so the task-board tools can reach the scheduler without Tauri `State<>`. Installed from `main.rs` right after scheduler construction.
- [x] **`RecipeScheduler::kick_worker(slug)`**: fires a one-off recipe tick for a single worker *immediately*, bypassing the cron cadence. Internally skips if the worker is already `working` to prevent duplicate ticks. Loads the bound recipe, spawns the runner through `tauri::async_runtime::spawn`.
- [x] **`create_task` → kick supervisor**: after `TasksDb::create` succeeds, call `scheduler::kick("supervisor")`. Collapses the previously observed 21-second "dead window" between PA creating 8 tasks and the Supervisor seeing them.
- [x] **`assign_task` → kick assignee**: after atomic `status='assigned'` update, call `scheduler::kick(worker_slug)`. Worker picks up its task within ~1 s instead of waiting up to its cadence.
- [x] **`update_task_status` → kick QC on `qc_review`**: when a worker hands a task off to QC, wake the QC agent immediately instead of waiting for its 1-minute tick.
- [x] **Task-board Tauri events**: every write path (`create_task`, `assign_task`, `update_task_status`, `clear_all_tasks`, `delete_task`) emits `task:changed` so the AutoDev Panel refreshes without polling.

#### Refinement R2 — Agent status event stream ⭐ FLEET-CRITICAL ✅

- [x] **New events**: `worker:status` (slug + status + ISO timestamp), `worker:tick-start` (slug), `worker:tick-end` (slug + outcome).
- [x] **Emit on every status transition**: `RecipeRunner::set_worker_status` now also emits `worker:status`. Tick boundaries emit `worker:tick-start` / `worker:tick-end` so the UI can show a live amber pulse on the active worker.
- [x] **Frontend `workerStore.initEventListeners()`**: one-shot listener registration; merges pushed status into the Zustand store so the Agent Roster no longer derives status from task-board staleness (which was the root of the `idle → error → idle` flicker).
- [x] **Boot hook**: called once from `App.tsx` at mount; listeners are idempotent.

#### Refinement R3 — Session-bound PA mode ✅

- [x] **Schema migration**: `sessions.pa_mode TEXT NOT NULL DEFAULT 'fast_development'`. Added to `SessionStorage::migrate` with `ALTER TABLE ... ADD COLUMN` wrapped in the existing best-effort pattern.
- [x] **`Session.pa_mode` field**: mirrored on the Rust `Session` struct; round-trips through `upsert_session`, `list_sessions`, and `get_session_with_messages`.
- [x] **`set_session_pa_mode(session_id, pa_mode)` Tauri command**: single write point; validates against the three allowed modes.
- [x] **`send_message` reads from session first**: the `pa_mode` parameter is now a *fallback* — the session row wins so a reload or engine rebuild cannot snap back to `fast_development`.
- [x] **Frontend persistence**: `settingsStore.setPAMode` also calls `set_session_pa_mode` for the active session; session hydration copies `pa_mode` from the loaded session into the store.

#### Refinement R4 — Sidecar log hygiene (Windows 10054) ✅

- [x] **`spawn_log_router` filter**: lines matching `ConnectionResetError.*WinError 10054` or `_ProactorBasePipeTransport\._call_connection_lost` are still written to the on-disk file (so forensics are complete) but only emitted at `debug!` on the in-app tracing stream. Net effect: log UI stays readable, real errors become visible.

#### Refinement R5 — Tighten cleanup cadence ✅

- [x] **`worker-cleanup` schedule**: `*/10 * * * *` → `*/2 * * * *`. Cleanup work now participates in the fleet at a comparable cadence to the other workers, matching the ~30-second expectation set by the Agent Roster. Paired with R1/R2, cleanup tasks get picked up sub-second via `kick_worker`.

#### Success Criteria (all met by the fix set)

- [x] **No dead window**: PA creates tasks → Supervisor assigns within 2 s (push, not cron).
- [x] **No serialized pickup**: `assign_task` → worker sees it within 2 s (push, not cron).
- [x] **No flicker**: Agent Roster cell updates on `worker:status` event, never from task-board inference.
- [x] **PA mode stable**: toggling Delegate persists through session reload, engine rebuild, and app restart.
- [x] **Log stream readable**: `WinError 10054` storm is off the main log channel but retained in `~/.autodev/logs/`.

#### Expected User-Visible Impact

Same *Calendar App* prompt: expected wall-clock drops from the baseline ~14 minutes (drip-fed) to ~3–4 minutes (continuous), with every status change visible in real time and no phantom `Error` states.

---

### Phase 3 Summary Statistics

| Metric | Count |
|--------|-------|
| **Total Sub-Phases** | 5 (3.1, 3.2, 3.3, 3.4, 3.5) |
| **Total Checklist Items** | 180+ |
| **New Rust Modules** | 15+ |
| **New Frontend Components** | 25+ |
| **New Tauri Commands** | 60+ |
| **Skills** | 20+ built-in |
| **Hook Types** | 18 across 6 categories |
| **Fleet-Critical Items** | 40+ (marked with ⭐) |

### Phase 3 Success Criteria (Overall)

> **Fleet-First Success**: Supervisor recipe launches → Polls tasks via MCP → Uses `supervisor-orchestration` skill → Assigns to worker → Worker uses `worker-task-execution` skill → Reports progress via MCP → Task completes → QC uses `qc-review` skill → All hooks fire correctly

> **IDE Success**: User opens IDE → Navigates file tree → Opens code in Monaco → Sees git status → Makes change → Auto-checkpoint → Commits with trailer → History shows commit

> **Plan Success**: PA detects complex task → Creates plan → Adds 5 steps → Updates progress → Hooks auto-fire → Plan completes → Summary generated

---

## 12. Phase 4 — Agent Memory + Subagents + Model Router

**Goal**: PA memory system (episodic, semantic, procedural), multi-agent orchestration, intelligent model routing, AutoDev dispatch signal handling.

### Key Components

- **PA Memory System**: Three-layer memory replacing compaction (episodic, semantic, procedural)
- **Subagents**: Spawn child agents with isolated tool sets and context
- **Model Router**: Map agent roles to specific provider/model pairs
- **Dispatch Signals**: Watch filesystem for dispatch signal files
- **Agent Teams**: Coordinate multiple agents on a workflow

### Phase 4 Deliverables Checklist

**PA Memory System (Replaces Compaction) — See Full Spec: `PA_MEMORY_SYSTEM_SPEC.md`:**

**Phase 4.1 — Episodic Memory (Raw Experience):**
- [ ] **SQLite Schema**: `episodic_memories` table with session_id, turn_index, content, metadata JSON, embeddings
- [ ] **Storage Pipeline**: Every user message and agent response stored to episodic memory
- [ ] **Turn Tracking**: 1-indexed turn counter per session for ordinal queries ("third interaction")
- [ ] **Metadata Capture**: Model used, cost, thinking content, tool calls, working directory
- [ ] **Tauri Commands**: `store_user_message()`, `store_agent_response()`, `get_session_timeline()`
- [ ] **Rust Memory Service**: `src-tauri/src/memory/episodic.rs` wrapper module
- [ ] **Core Orchestrator Bridge**: HTTP endpoints for episodic storage and retrieval

**Phase 4.2 — Semantic Memory (Facts & Knowledge):**
- [ ] **SQLite Schema**: `semantic_memories` table with fact_text, category, confidence, source tracking
- [ ] **Fact Extraction**: LLM-powered extraction from user messages only (not agent responses)
- [ ] **Categories**: Project patterns, user preferences, architecture decisions, reference locations
- [ ] **Contradiction Handling**: `fact_history` table tracks corrections and updates
- [ ] **Background Extraction**: Triggered automatically on user messages via core-orchestrator
- [ ] **Tauri Commands**: `recall_facts()`, `update_fact()`, `get_user_preferences()`
- [ ] **Confidence Threshold**: 0.7+ for auto-storage, lower for suggestions

**Phase 4.3 — Procedural Memory (Skills & Learnings):**
- [ ] **File-Based Storage**: `~/.autodev/memory/MEMORY.md` index + topic files
- [ ] **SQLite Mirror**: `procedural_memories` table for search/indexing
- [ ] **MEMORY.md Sidebar UI** (similar to AUTODEV.md):
  - Monaco editor for viewing/editing
  - Topic file browser (debugging.md, workflow-patterns.md, etc.)
  - Search memories input box
  - Stats display (fact count, skill count, last updated)
- [ ] **Auto-Generation**: Session summaries automatically added to MEMORY.md
- [ ] **User Editing**: Full CRUD via UI with save/reset/export buttons
- [ ] **Loading Rules**: First 200 lines load at startup, topic files on demand

**Phase 4.4 — Memory Injection & Context Assembly:**
- [ ] **Context.rs Updates**: `build_system_prompt()` loads relevant memories per query
- [ ] **Recall API**: `/memory/recall` endpoint in core-orchestrator combines all tiers
- [ ] **Relevance Scoring**: Hybrid semantic + chronological retrieval
- [ ] **Injection Format**: Structured memory sections in system prompt
- [ ] **Limits**: Episodic (5), Semantic (10), Procedural (3) per query

**Phase 4.5 — Auto-Summarization:**
- [ ] **Session Summary**: Generate after every session ends (or every 20 turns)
- [ ] **Summary Content**: Key actions, files touched, decisions made, learnings
- [ ] **Storage**: Saved as episodic memory with type "session_summary"
- [ ] **Background Task**: Non-blocking, runs after response is sent

**Phase 4.6 — Memory Tools for PA:**
- [ ] **`search_memory` Tool**: PA can query its own memory
  - Parameters: query, tier filter, time range, limit
  - Returns: Relevant memories with timestamps and confidence
- [ ] **`remember_this` Tool**: Explicit memory storage
  - User says "remember that I prefer X" → stored to semantic memory
- [ ] **Tool Registration**: Add to builtin_tools registry

**Phase 4.7 — Core Orchestrator Integration:**
- [ ] **Submodule Updates**: `context_service.py` expose memory endpoints
- [ ] **Memory MCP Connection**: Ensure memory-mcp-server connectivity
- [ ] **API Gateway Routes**: `/memory/*` endpoints
- [ ] **Sidecar Bridge**: Rust → Python memory service communication
- [ ] **Testing**: Unit tests for each memory tier, integration tests for recall

**Phase 4.8 — Migration from Compaction:**
- [ ] **Migration Script**: Convert old session messages to episodic memories
- [ ] **Session Summaries**: Generate for all historical sessions >5 messages
- [ ] **Data Integrity**: Preserve original timestamps, mark as migrated
- [ ] **Rollback Plan**: Keep original sessions table until migration verified

### Phase 4.9 — Subagent System (Leveraging Core-Orchestrator) ✅ IMPLEMENTED

> **Architectural deviation**: The original spec assumed dedicated `/agents/create`, `/process`, `/terminate`, and `/events` endpoints in core-orchestrator. Those don't exist. We instead implement subagents over the existing **`POST /api/agents` → `POST /api/agents/{id}/invoke/stream` → `DELETE /api/agents/{id}`** lifecycle. Each subagent is a real `LivingAgent` whose ephemerality comes from being created and deleted within the bounds of one subtask. **See `docs/development/PHASE_4_9_EPHEMERAL_VIA_LIFECYCLE.md` for the full rationale, slug convention, and three-layer cleanup strategy.**

**Design Philosophy**: Instead of building a new subagent tool from scratch, AutoDev leverages the existing **core-orchestrator's agent factory** (`AgentFactory.create_agent()`). The subagent system is a **bridge** between AutoDev's Rust backend and core-orchestrator's Python `LivingAgent` instances.

**Benchmark Research:**
- **Claude Code**: Uses `AgentTool` with `runAgent()` — child agents get isolated context (fresh message history), subset of parent tools, custom system prompts. Parent blocks until child completes, receiving summary.
- **Goose**: `SubagentExecutionTool` with notification events for async task tracking. Parent can poll or wait for completion events.
- **Core-Orchestrator**: Already has `LivingAgent` with `MemoryCore`, `AgentFactory.create_agent()` for spawning, A2A protocol for agent-to-agent communication.

**Key Insight**: Core-orchestrator already has everything needed for subagents:
- ✅ `AgentFactory.create_agent(name, override_config)` — spawns new LivingAgent
- ✅ `LivingAgent.process_message()` — runs agent with isolated context
- ✅ A2A protocol — agent-to-agent communication
- ✅ Task execution FSM — tracks subagent lifecycle

**Architecture: Rust Bridge → HTTP → Core-Orchestrator**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  AUTODEV SUBAGENT SYSTEM                                                      │
│                                                                              │
│  ┌──────────────────┐     ┌──────────────────┐     ┌──────────────────┐   │
│  │  Parent Agent    │────▶│  Rust Subagent   │────▶│  Core-Orchestrator│   │
│  │  (Rust AgentEngine)│     │  Bridge (HTTP)   │     │  LivingAgent      │   │
│  └──────────────────┘     └──────────────────┘     └──────────────────┘   │
│          │                          │                       │               │
│          │                          │                       │               │
│          │ 1. spawn_subagent()      │ 2. POST /agents/create│               │
│          │─────────────────────────▶│───────────────────────▶│               │
│          │                          │                       │               │
│          │ 3. execute_subtask()     │ 4. POST /agents/{id}/process        │
│          │─────────────────────────▶│───────────────────────▶│               │
│          │                          │                       │               │
│          │ 5. receive summary       │ 6. SSE /agents/{id}/events            │
│          │◀─────────────────────────│◀───────────────────────│               │
│          │                          │                       │               │
│          │ 7. terminate_subagent()  │ 8. POST /agents/{id}/terminate        │
│          │─────────────────────────▶│───────────────────────▶│               │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Backend — Subagent Module

**Rust Module Structure:**
- [ ] `src-tauri/src/subagent/mod.rs` — Module declaration
- [ ] `src-tauri/src/subagent/types.rs` — SubagentConfig, SubagentStatus, SubagentResult
- [ ] `src-tauri/src/subagent/bridge.rs` — HTTP bridge to core-orchestrator
- [ ] `src-tauri/src/subagent/manager.rs` — SubagentManager (tracks active subagents)
- [ ] `src-tauri/src/subagent/pool.rs` — SubagentPool (limits max concurrent subagents)

**Core Types:**
```rust
pub struct SubagentConfig {
    pub name: String,                    // Unique subagent name (auto-generated: "{parent}-sub-{n}")
    pub parent_session_id: String,       // Parent's session ID
    pub system_prompt: String,           // Custom system prompt (or inherit from parent)
    pub tools: Vec<String>,               // Tool subset (e.g., ["read_file", "grep", "bash"])
    pub model: String,                   // Model override (or inherit from parent)
    pub working_dir: String,             // Working directory (git worktree for isolation)
    pub max_turns: u32,                  // Max tool turns (default: 20)
    pub timeout_secs: u32,               // Timeout (default: 300s)
    pub inherit_autodev_md: bool,        // Inherit AUTODEV.md from parent
}

pub struct SubagentHandle {
    pub id: String,                      // Core-orchestrator agent ID
    pub config: SubagentConfig,
    pub status: SubagentStatus,
    pub created_at: DateTime<Utc>,
    pub events: mpsc::Receiver<SubagentEvent>, // SSE event stream
}

pub enum SubagentStatus {
    Spawning,      // Creating agent in core-orchestrator
    Idle,          // Agent ready, waiting for task
    Running,       // Processing message
    Completed,     // Finished successfully
    Failed,        // Error or timeout
    Terminated,    // Explicitly killed
}
```

**Tauri Commands:**
- [ ] `spawn_subagent(config: SubagentConfig) -> Result<SubagentHandle>`
  - Calls core-orchestrator `POST /api/agents/create`
  - Creates git worktree for isolation (if working_dir specified)
  - Registers subagent in manager
  - Returns handle with event stream

- [ ] `execute_subtask(subagent_id: String, prompt: String, context: SubtaskContext) -> Result<SubagentResult>`
  - Sends task to subagent via `POST /api/agents/{id}/process`
  - Streams events back via SSE
  - Blocks until completion or timeout
  - Returns summary result

- [ ] `get_subagent_status(subagent_id: String) -> Result<SubagentStatus>`
  - Query core-orchestrator for agent status
  - Returns current state and metadata

- [ ] `terminate_subagent(subagent_id: String, force: bool) -> Result<()>`
  - Graceful shutdown (or force kill)
  - Cleanup worktree if created
  - Remove from manager

- [ ] `list_active_subagents(parent_session_id: Option<String>) -> Result<Vec<SubagentSummary>>`
  - List all subagents (optionally filtered by parent)
  - Shows status, runtime, current task

- [ ] `abort_subagent(subagent_id: String) -> Result<()>`
  - Cancel running subagent immediately
  - Emit abort signal via SSE

**Subagent Bridge (HTTP Client):**
```rust
pub struct SubagentBridge {
    client: reqwest::Client,
    base_url: String,  // core-orchestrator endpoint (localhost:8765)
}

impl SubagentBridge {
    async fn create_agent(&self, config: &SubagentConfig) -> Result<String>;
    async fn process_message(&self, agent_id: &str, message: &str) -> Result<ProcessResult>;
    async fn get_status(&self, agent_id: &str) -> Result<AgentStatus>;
    async fn terminate(&self, agent_id: &str) -> Result<()>;
    async fn subscribe_events(&self, agent_id: &str) -> Result<mpsc::Receiver<AgentEvent>>;
}
```

**Core-Orchestrator API Endpoints (Already Exist):**
- `POST /api/agents/` — Create new agent (returns agent_id)
- `POST /api/agents/{id}/process` — Send message to agent
- `GET /api/agents/{id}/status` — Get agent status
- `POST /api/agents/{id}/terminate` — Terminate agent
- `GET /api/agents/{id}/events` — SSE event stream

#### Built-in Subagent Tool

**Tool: `spawn_subagent`**
```rust
pub struct SpawnSubagentInput {
    pub name: Option<String>,           // Optional custom name (auto-generated if None)
    pub task_description: String,       // What the subagent should do
    pub tools: Vec<String>,             // Allowed tools (subset of parent)
    pub files: Vec<String>,             // Files to provide as context
    pub model: Option<String>,          // Model override
    pub max_turns: Option<u32>,         // Max tool turns
}

pub struct SpawnSubagentOutput {
    pub subagent_id: String,
    pub result_summary: String,         // Final result from subagent
    pub files_modified: Vec<String>,    // Files the subagent touched
    pub cost_usd: f64,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub duration_secs: f64,
}
```

**Execution Flow:**
```
1. Parent Agent calls spawn_subagent tool
   └─ Tool validates parameters
   └─ Creates SubagentConfig

2. SubagentBridge.create_agent()
   └─ POST /api/agents/ to core-orchestrator
   └─ Core-orchestrator: AgentFactory.create_agent()
   └─ Returns agent_id (e.g., "worker-backend-sub-3")

3. Git Worktree Setup (if needed)
   └─ Create worktree at .autodev/worktrees/{subagent_id}/
   └─ Set as working_dir in config

4. Execute Subtask
   └─ Bridge.process_message() with task_description
   └─ Core-orchestrator: LivingAgent.process_message()
   └─ Subagent runs with isolated context

5. Stream Events
   └─ SSE events: token, tool_start, tool_end, thinking, done
   └─ Parent can display progress in UI

6. Completion
   └─ Subagent finishes (success/failure/timeout)
   └─ Core-orchestrator returns final result
   └─ Bridge returns SpawnSubagentOutput to parent

7. Cleanup
   └─ Terminate subagent in core-orchestrator
   └─ Optionally preserve worktree for review
   └─ Log subagent session to parent history
```

#### Parallel Subagent Execution

**Tool: `spawn_parallel_subagents`**
```rust
pub struct ParallelSubagentsInput {
    pub subtasks: Vec<SubtaskSpec>,    // 2-10 parallel subtasks
    pub max_concurrent: u32,            // Max parallel (default: 5)
    pub aggregate_results: bool,        // Return combined summary
}

pub struct ParallelSubagentsOutput {
    pub results: Vec<SubagentResult>,
    pub combined_summary: String,       // LLM-generated aggregate
    pub total_cost_usd: f64,
    pub total_duration_secs: f64,
}
```

**Use Cases:**
- Review 5 files in parallel (each file → 1 subagent)
- Test multiple approaches (3 subagents with different strategies)
- Analyze large codebase (shard across subagents)

#### Nested Subagents

**Architecture supports nesting:**
- Parent (Worker) spawns Subagent A
- Subagent A spawns Subagent B (for sub-subtask)
- Up to 3 levels deep (configurable)
- Each level has own worktree isolation

**Tracking:**
- Subagent ID includes parent chain: `worker-backend-sub-3-sub-1`
- SQLite table tracks parent-child relationships
- UI shows nested tree view

#### Frontend — Subagent UI

**Subagent Monitor Panel:**
- [ ] `src/components/subagent/SubagentMonitor.tsx`
  - Tree view of active subagents (parent → children)
  - Real-time status indicators
  - Current task description per subagent
  - Progress bars for running subagents
  - Cost tracking per subagent

**Subagent Event Display:**
- [ ] `src/components/subagent/SubagentEventStream.tsx`
  - Shows SSE events from subagent (like ChatMessages)
  - Token streaming from subagent
  - Tool calls made by subagent
  - Thinking/reasoning display

**Subagent Result Card:**
- [ ] `src/components/subagent/SubagentResultCard.tsx`
  - Summary of completed subagent work
  - Files modified list
  - Cost and duration stats
  - "View Full Log" button

#### Subagent Store

- [ ] `src/stores/subagentStore.ts`
  - State: `activeSubagents`, `subagentHistory`, `eventStreams`
  - Actions:
    - `spawnSubagent(config)` → Returns handle
    - `trackSubagent(subagentId)` → Subscribe to events
    - `terminateSubagent(subagentId)`
    - `getSubagentTree(parentId)` → Nested structure
  - Event listeners: `subagent:spawned`, `subagent:completed`, `subagent:failed`

#### Integration with Fleet

**Worker Recipes Using Subagents:**
```yaml
# worker-backend-poll.yaml (updated for subagents)
instructions: |
  You are Worker Backend.
  1. Poll for assigned tasks
  2. If task is complex (>5 files), SPAWN SUBAGENTS:
     - spawn_subagent for analysis
     - spawn_parallel_subagents for file reviews
  3. Coordinate results
  4. Submit final PR
```

**Supervisor Epic Breakdown with Subagents:**
```
Supervisor receives epic "Refactor auth system"
├─ Spawns Analysis Subagent: "Analyze current auth code"
├─ Waits for analysis result
├─ Spawns 3 Parallel Subagents:
│  ├─ Subagent 1: "Design new auth middleware"
│  ├─ Subagent 2: "Plan database migrations"
│  └─ Subagent 3: "Update API documentation"
├─ Collects all results
├─ Creates implementation plan
└─ Assigns to workers
```

#### Subagent Safety & Limits

**Resource Limits:**
- Max concurrent subagents per parent: 10
- Max nesting depth: 3 levels
- Max subagent lifetime: 10 minutes (timeout)
- Max tokens per subagent: 50k (configurable)

**Isolation Guarantees:**
- Each subagent gets fresh message history (no parent context leak)
- Git worktree per subagent (file changes isolated)
- Tool subset (parent controls what subagent can access)
- No MCP server access unless explicitly granted

**Cleanup:**
- Auto-terminate orphaned subagents (parent dies → children killed)
- Worktree cleanup on termination (configurable)
- SQLite cleanup of old subagent records (>30 days)

#### Subagent Deliverables Checklist

- [x] **Backend — Subagent Module:**
  - [x] `src-tauri/src/subagent/types.rs` — SubagentConfig, SubagentHandle, SubagentStatus, SubagentEvent, SubagentResult, SubagentSummary
  - [x] `src-tauri/src/subagent/slug.rs` — unique slug generator (`subagent-{parent}-{millis}-{seq}`) + reserved prefix
  - [x] `src-tauri/src/subagent/bridge.rs` — Stateless lifecycle runner (create → invoke/stream → delete) + hand-rolled SSE parser
  - [x] `src-tauri/src/subagent/manager.rs` — SubagentManager (registry + event pump + RAII pool permits + orphan cleanup) + `install_global` / `global` accessor
  - [x] `src-tauri/src/subagent/pool.rs` — Concurrency limits (`MAX_GLOBAL_CONCURRENT=16`, `MAX_PER_PARENT=10`, `MAX_NESTING_DEPTH=3`) with tokio Semaphore
  - [x] Tauri commands: `spawn_subagent`, `execute_subtask`, `get_subagent_status`, `terminate_subagent`, `abort_subagent`, `list_active_subagents` (in `commands/subagent_commands.rs`)
  - [x] OrchestratorClient extensions: `create_ephemeral_agent`, `delete_agent`, `invoke_agent_stream_raw`, `list_ephemeral_subagent_slugs`

- [ ] **Backend — Git Worktree Integration:** *(deferred — `working_dir` is forwarded today; auto-worktree per subagent is a follow-up since most subtasks are read-only analysis where it's overkill. Will be enabled when Workers begin spawning code-modifying subagents.)*
  - [ ] Auto-create worktree for each subagent
  - [ ] Cleanup worktree on termination
  - [ ] Track worktree → subagent mapping

- [x] **Built-in Tool:**
  - [x] `spawn_subagent` tool (single subagent, parent blocks for result)
  - [x] `spawn_parallel_subagents` tool (parallel execution with per-index results + aggregated summary)
  - [x] Tool registration in `agent/builtin_tools/mod.rs` (FastDevelopment + Review modes; not Delegate)

- [x] **Core-Orchestrator Integration:**
  - [x] Verify `/api/agents/*` endpoints work via sidecar bridge — using existing `OrchestratorClient` JWT auth
  - [x] Subagent metadata forwarding (system_prompt, model, mcp_servers, capabilities) via `AgentCreate` body
  - [x] SSE event streaming via `/api/agents/{id}/invoke/stream` — frame parser handles both `event:`/`data:` and bare `data:{type:...}` shapes

- [x] **Frontend — Subagent UI:**
  - [x] `SubagentMonitor.tsx` — Grouped-by-parent tree view of active subagents with status indicators, depth badges, abort buttons
  - [x] `SubagentEventStream.tsx` — Real-time SSE event display (token / thinking / tool_start / tool_end / final / cost / status / error)
  - [x] `SubagentResultCard.tsx` — Completed subagent summary with files modified, cost/token stats, expand/collapse
  - [ ] *Integration in ChatMessages for subagent tool calls — pending (the components are wired to the store; embedding them inline in the chat is a small follow-up)*

- [x] **Frontend — Subagent Store:**
  - [x] `subagentStore.ts` with spawn/await/terminate/abort actions, refresh, forget
  - [x] Event stream management — `subagent:event` listener fans into per-subagent event arrays
  - [x] Parent-child tree visualization — grouped by `parent_slug` in SubagentMonitor

- [x] **Safety & Limits:**
  - [x] Max concurrent subagents enforcement (global semaphore + per-parent counter)
  - [x] Max nesting depth check (rejected at pool acquire time)
  - [x] Subagent timeout handling (`tokio::time::timeout` wrapper inside `bridge::run`)
  - [x] Orphan cleanup — three layers: happy-path DELETE, RAII drop on PoolPermit, startup sweep via `cleanup_orphans` (deletes any `subagent-*` agents in core-orchestrator)

- [ ] **Integration with Fleet:** *(future — recipe authors can now USE the tools; the recipes themselves are not yet rewritten to do so)*
  - [ ] Worker recipes updated to use subagents for complex tasks
  - [ ] Supervisor epic breakdown using subagents
  - [ ] Subagent cost tracking per worker

**Subagent Success Criteria:**
> Parent agent spawns subagent → Subagent executes task in isolated worktree → Returns summary → Parent continues with result. Parent can spawn 5 parallel subagents → All complete → Parent aggregates results.

---

**Model Router + Dispatch:**
- [ ] Model Router: configurable role → model mapping
- [ ] Model Router UI in Settings tab
- [ ] Dispatch signal watch (FSEvents/inotify via `notify` crate)
- [ ] Dispatch signal UI in AutoDev Panel

**Providers:**
- [ ] Azure OpenAI provider
- [ ] Prompt caching for all cloud providers (cost optimization)

---

## 13. Phase 5 — CLI + AIRS Plugin + Installers + Dashboard

**Goal**: Headless CLI for automation, AIRS Plugin for one-click worker setup, cross-platform installers, comprehensive cost dashboard and analytics for fleet management.

> **Priority Order**: CLI (automation foundation) → AIRS Plugin (worker setup) → Installers (distribution) → Dashboard (analytics)

### Phase 5 Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Phase 5 Sub-Phase Dependencies                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Phase 5.1 ───────────────────────┐                                         │
│  CLI (airsctl)                    │                                         │
│  (Automation Foundation)          ▼                                         │
│                          Phase 5.2 ─────────────────────┐                   │
│                          AIRS Plugin (Worker Setup)      │                   │
│  Phase 5.3 ──────────────────────────────────────────────┤                   │
│  Platform Installers                                     │                   │
│  (Bundle CLI + Plugin + App)                             ▼                   │
│                                               Phase 5.4 ──────────────────┐ │
│                                               Dashboard + Analytics        │ │
│                                               (Uses all previous)          │ │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### Phase 5.1 — CLI (airsctl) 🎯 **AUTOMATION FOUNDATION**

**Goal**: Headless CLI binary for automation, scripting, CI/CD integration, and remote worker management.

**Why First**: CLI enables automation before GUI, allows CI/CD integration, and provides foundation for worker automation scripts.

#### CLI Architecture

**Core Concept**: Separate Rust binary (`airsctl`) that shares core logic with Tauri app but runs headless. Inspired by Goose's `goose-cli` crate structure.

**File Structure:**
- [ ] `src-cli/Cargo.toml`: CLI-specific dependencies (clap, dialoguer, indicatif)
- [ ] `src-cli/src/main.rs`: CLI entry point
- [ ] `src-cli/src/commands/mod.rs`: Command modules
- [ ] `src-cli/src/commands/agent.rs`: Agent commands (run, stop, status)
- [ ] `src-cli/src/commands/task.rs`: Task management commands
- [ ] `src-cli/src/commands/worker.rs`: Worker registration/management
- [ ] `src-cli/src/commands/config.rs`: Configuration management
- [ ] `src-cli/src/commands/logs.rs`: Log viewing and export
- [ ] `src-cli/src/repl.rs`: Interactive REPL mode
- [ ] `src-cli/src/noninteractive.rs`: Non-interactive mode for scripts
- [ ] `src-cli/src/output.rs`: Output formatting (JSON, table, plain)
- [ ] `src-shared/`: Shared code between CLI and Tauri app (extracted from src-tauri)
  - [ ] `src-shared/src/agent/`: Agent engine (shared)
  - [ ] `src-shared/src/tools/`: Tool implementations (shared)
  - [ ] `src-shared/src/provider/`: Provider implementations (shared)
  - [ ] `src-shared/src/config/`: Configuration management (shared)

**CLI Command Structure:**
```
airsctl [GLOBAL OPTIONS] <COMMAND> [SUBCOMMAND] [ARGS]

Global Options:
  --config <PATH>           Config file path (default: ~/.airs/config.toml)
  --working-dir <PATH>      Working directory for operations
  --api-key <KEY>           API key for LLM providers
  --provider <NAME>         LLM provider (anthropic, openai, ollama)
  --model <NAME>            Model name
  --non-interactive         Disable interactive prompts (for CI/CD)
  --output-format <FORMAT>  Output format: json, table, plain (default: table)
  --quiet                   Suppress non-error output
  --verbose                 Enable verbose logging
  --version                 Show version
  --help                    Show help
```

**Command Categories:**

**1. Agent Commands (`airsctl agent`):**
- [ ] `airsctl agent run [OPTIONS] [PROMPT]`: Run agent with prompt
  - Options: `--file <PATH>` (read prompt from file), `--session <ID>` (resume session)
  - Non-interactive mode for CI/CD
  - JSON output mode for scripting
- [ ] `airsctl agent status`: Show running agent status
- [ ] `airsctl agent stop [SESSION_ID]`: Stop running agent
- [ ] `airsctl agent list`: List active sessions
- [ ] `airsctl agent logs [SESSION_ID]`: View agent logs

**2. Task Commands (`airsctl task`):**
- [ ] `airsctl task create --title <TITLE> --type <TYPE>`: Create new task
- [ ] `airsctl task list [--status <STATUS>] [--worker <WORKER>]`: List tasks
- [ ] `airsctl task get <TASK_ID>`: Get task details
- [ ] `airsctl task assign <TASK_ID> --worker <WORKER>`: Assign task to worker
- [ ] `airsctl task claim [--worker-id <ID>]`: Worker claims next available task
- [ ] `airsctl task complete <TASK_ID> --result <RESULT>`: Mark task complete
- [ ] `airsctl task fail <TASK_ID> --reason <REASON>`: Mark task failed

**3. Worker Commands (`airsctl worker`):**
- [ ] `airsctl worker register --name <NAME> --type <TYPE>`: Register new worker
- [ ] `airsctl worker list [--type <TYPE>] [--status <STATUS>]`: List workers
- [ ] `airsctl worker status <WORKER_ID>`: Check worker status
- [ ] `airsctl worker heartbeat <WORKER_ID>`: Send heartbeat
- [ ] `airsctl worker unregister <WORKER_ID>`: Remove worker

**4. Configuration Commands (`airsctl config`):**
- [ ] `airsctl config init`: Initialize default configuration
- [ ] `airsctl config get <KEY>`: Get config value
- [ ] `airsctl config set <KEY> <VALUE>`: Set config value
- [ ] `airsctl config list`: List all config values
- [ ] `airsctl config validate`: Validate configuration
- [ ] `airsctl config import <PATH>`: Import config from file
- [ ] `airsctl config export [PATH]`: Export config to file

**5. Plan Commands (`airsctl plan`):**
- [ ] `airsctl plan create --title <TITLE> --description <DESC>`: Create plan
- [ ] `airsctl plan list [--status <STATUS>]`: List plans
- [ ] `airsctl plan get <PLAN_ID>`: Get plan details
- [ ] `airsctl plan add-step <PLAN_ID> --title <TITLE>`: Add step to plan
- [ ] `airsctl plan update-step <STEP_ID> --status <STATUS>`: Update step
- [ ] `airsctl plan activate <PLAN_ID>`: Activate draft plan

**6. System Commands (`airsctl system`):**
- [ ] `airsctl system status`: Check system health (sidecars, DB, etc.)
- [ ] `airsctl system logs [--component <COMPONENT>]`: View system logs
- [ ] `airsctl system sidecars`: Manage sidecars (start, stop, status)
- [ ] `airsctl system backup [PATH]`: Backup AutoDev data
- [ ] `airsctl system restore <PATH>`: Restore from backup

**7. Analytics Commands (`airsctl analytics`):**
- [ ] `airsctl analytics cost [--today|--week|--month|--all]`: Show cost summary
- [ ] `airsctl analytics tokens [--session <ID>]`: Show token usage
- [ ] `airsctl analytics workers`: Show worker statistics
- [ ] `airsctl analytics export --format <FORMAT> --output <PATH>`: Export analytics

**REPL Mode (`airsctl repl`):**
- [ ] Interactive shell with command history
- [ ] Tab completion for commands and paths
- [ ] Syntax highlighting for responses
- [ ] Session persistence across REPL restarts
- [ ] Multi-line input support

**Non-Interactive Mode Features:**
- [ ] JSON output for all commands (`--output-format json`)
- [ ] Exit codes: 0=success, 1=error, 2=partial success
- [ ] Pipe-friendly output (plain text, no formatting)
- [ ] Environment variable support: `AIRSDEV_API_KEY`, `AIRSDEV_PROVIDER`

**Configuration File (`~/.airs/config.toml`):**
```toml
[core]
working_dir = "/home/user/projects"
default_provider = "anthropic"
auto_start_sidecars = true

[providers.anthropic]
api_key = "sk-ant-..."
model = "claude-3-7-sonnet-20250219"

[providers.ollama]
enabled = true
model = "codellama:13b"

[cli]
output_format = "table"
interactive = true
history_size = 1000

[worker]
worker_id = "worker-001"
type = "backend"
heartbeat_interval = 300
```

#### CLI Implementation Details

**Backend — CLI Core:**
- [ ] Extract shared code from `src-tauri` into `src-shared` workspace crate
- [ ] CLI-specific command parsing with `clap` derive macros
- [ ] Non-interactive mode detection (no TTY)
- [ ] Progress indicators with `indicatif` crate (spinners, progress bars)
- [ ] Interactive prompts with `dialoguer` (confirmations, selections)
- [ ] Colored output with `colored` crate
- [ ] Table formatting with `comfy-table`
- [ ] Logging with `tracing` and `tracing-subscriber`

**Tauri Integration:**
- [ ] Share `AgentEngine`, `ToolExecutor`, `Provider` implementations via `src-shared`
- [ ] CLI uses same SQLite database as Tauri app (~/.autodev/)
- [ ] CLI can trigger Tauri events (for running GUI instances)
- [ ] Session sharing between CLI and GUI

#### Phase 5.1 Deliverables Checklist

**CLI Binary:**
- [ ] `src-cli/` workspace crate created
- [ ] `src-shared/` extracted from `src-tauri` for code reuse
- [ ] CLI binary builds independently: `cargo build -p airsctl`
- [ ] 30+ CLI commands across 7 categories
- [ ] REPL mode with history and completion
- [ ] Non-interactive mode for CI/CD
- [ ] JSON output format for scripting
- [ ] Configuration file support (`~/.airs/config.toml`)
- [ ] Environment variable integration
- [ ] Exit codes for scripting (0=success, 1=error)

**Commands:**
- [ ] `airsctl agent run/status/stop/list/logs`
- [ ] `airsctl task create/list/get/assign/claim/complete/fail`
- [ ] `airsctl worker register/list/status/heartbeat/unregister`
- [ ] `airsctl config init/get/set/list/validate/import/export`
- [ ] `airsctl plan create/list/get/add-step/update-step/activate`
- [ ] `airsctl system status/logs/sidecars/backup/restore`
- [ ] `airsctl analytics cost/tokens/workers/export`

**Integration:**
- [ ] Shares database with Tauri app
- [ ] Can trigger events in running GUI instance
- [ ] Session sharing between CLI and GUI
- [ ] Sidecar management (start/stop from CLI)

#### Phase 5.1 Success Criteria

> **Basic CLI**: `airsctl agent run "fix bug in login"` → Agent runs headless → Outputs result to stdout → Exits with code 0 on success

> **CI/CD Integration**: `airsctl task create --title "Deploy prod" --type deployment --non-interactive` → Task created → JSON output → Exit code 0

> **Worker Automation**: Worker PC runs `airsctl worker heartbeat worker-001` → Heartbeat recorded → Supervisor sees worker as active

---

### Phase 5.2 — AIRS Plugin 🎯 **ONE-CLICK WORKER SETUP**

**Goal**: Plugin system for one-click worker PC configuration, enabling rapid fleet deployment without manual setup.

**Depends On**: CLI (uses `airsctl` for configuration)

#### AIRS Plugin Architecture

**Core Concept**: Modular plugin system for configuring worker environments. Inspired by VS Code extensions and Goose's extension system.

**Plugin Types:**

**1. Worker Setup Plugin:**
- [ ] Auto-detects OS and hardware
- [ ] Installs required dependencies (Rust, Python, Node, etc.)
- [ ] Configures AutoDev CLI (`airsctl`)
- [ ] Registers worker with fleet
- [ ] Sets up monitoring and auto-update

**2. Environment Plugin:**
- [ ] Configures development environment
- [ ] Clones required repositories
- [ ] Sets up git credentials
- [ ] Configures IDE/editor
- [ ] Installs language-specific tools

**3. Security Plugin:**
- [ ] Configures firewall rules
- [ ] Sets up VPN connection
- [ ] Configures SSH keys
- [ ] Sets up certificate management
- [ ] Configures access controls

**Plugin Structure (`~/.airs/plugins/{plugin-name}/`):**
```
plugin-name/
├── plugin.yaml          # Plugin manifest
├── install.sh           # Install script (Linux/macOS)
├── install.ps1          # Install script (Windows)
├── config/
│   ├── default.toml     # Default configuration
│   └── schema.json      # Configuration schema
├── assets/
│   └── icon.png         # Plugin icon
└── hooks/
    ├── pre-install.sh   # Pre-install hook
    └── post-install.sh  # Post-install hook
```

**Plugin Manifest (`plugin.yaml`):**
```yaml
name: worker-backend
version: "1.0.0"
description: Backend worker setup with Rust, Python, and Docker
author: AIRS Foundry
type: worker-setup  # worker-setup, environment, security, custom
os_support:
  - linux
  - macos
  - windows
dependencies:
  - name: rust
    version: ">=1.70"
    install_cmd: "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  - name: python
    version: ">=3.10"
    install_cmd: "pip install uv"
  - name: docker
    version: ">=24.0"
    optional: true
commands:
  install:
    linux: "./install.sh"
    macos: "./install.sh"
    windows: "powershell -ExecutionPolicy Bypass -File install.ps1"
  verify: "./verify.sh"
  uninstall: "./uninstall.sh"
config:
  schema: "config/schema.json"
  defaults: "config/default.toml"
```

**AIRS Plugin Manager:**
- [ ] `src-tauri/src/plugin/mod.rs`: Plugin module
- [ ] `src-tauri/src/plugin/manager.rs`: Plugin installation/management
- [ ] `src-tauri/src/plugin/registry.rs`: Plugin registry (local + remote)
- [ ] `src-tauri/src/plugin/runner.rs`: Plugin execution

**Tauri Commands:**
- [ ] `list_available_plugins() -> Vec<PluginInfo>`: List from registry
- [ ] `list_installed_plugins() -> Vec<InstalledPlugin>`: List installed
- [ ] `install_plugin(name, version?, config?) -> InstallResult`: Install plugin
- [ ] `uninstall_plugin(name) -> ()`: Remove plugin
- [ ] `update_plugin(name) -> UpdateResult`: Update to latest
- [ ] `run_plugin(name, action, args?) -> RunResult`: Execute plugin action
- [ ] `get_plugin_config(name) -> PluginConfig`: Get configuration
- [ ] `set_plugin_config(name, config) -> ()`: Update configuration
- [ ] `verify_plugin(name) -> VerifyResult`: Verify installation

**Plugin Registry:**
- [ ] Local registry: `~/.airs/registry/` (cached plugins)
- [ ] Remote registry: GitHub releases or custom URL
- [ ] Plugin marketplace API
- [ ] Version resolution (semver)
- [ ] Dependency resolution between plugins

**One-Click Worker Setup Flow:**
```
1. IT staff generates setup token in AutoDev UI
   └── Token contains: fleet_id, worker_type, allowed_repos

2. Email sent to worker PC user with one-liner:
   curl -fsSL https://airs.dev/setup | bash -s -- TOKEN

3. Setup script:
   a. Downloads airsctl CLI for platform
   b. Downloads AIRS Plugin installer
   c. Runs: airsctl plugin install worker-{type}
   d. Configures worker with token
   e. Registers worker with fleet
   f. Starts sidecars and heartbeat

4. Worker appears in AutoDev Fleet panel as "Active"
```

**Frontend — Plugin UI:**

**Plugin Manager (`src/components/plugin/PluginManager.tsx`):**
- [ ] Plugin marketplace browser
- [ ] Search and filter plugins
- [ ] Install/uninstall buttons
- [ ] Plugin details view (README, version, author)
- [ ] Configuration editor per plugin
- [ ] Update notifications

**Worker Setup Wizard (`src/components/plugin/WorkerSetupWizard.tsx`):**
- [ ] Step 1: Select worker type (backend, frontend, devops, etc.)
- [ ] Step 2: Select OS (auto-detected)
- [ ] Step 3: Generate setup token
- [ ] Step 4: Show one-liner command
- [ ] Step 5: QR code for mobile setup
- [ ] Real-time status: "Worker connected!" / "Setup complete"

**IT Staff Dashboard (`src/components/plugin/ITDashboard.tsx`):**
- [ ] Bulk worker deployment
- [ ] Setup token management
- [ ] Worker type templates
- [ ] Deployment progress tracking
- [ ] Failed setup alerts

#### Phase 5.2 Deliverables Checklist

**Plugin System:**
- [ ] `src-tauri/src/plugin/` module
- [ ] Plugin manifest schema (YAML)
- [ ] Plugin installation/management
- [ ] Plugin registry (local + remote)
- [ ] 5+ built-in plugins (worker-backend, worker-frontend, worker-devops, etc.)
- [ ] Cross-platform install scripts (bash, PowerShell)
- [ ] Plugin configuration with JSON schema validation
- [ ] Plugin hooks (pre-install, post-install, pre-uninstall)
- [ ] Dependency resolution between plugins

**CLI Integration:**
- [ ] `airsctl plugin list/install/uninstall/update/run`
- [ ] `airsctl worker setup --type <TYPE> --token <TOKEN>`

**Frontend:**
- [ ] Plugin Manager UI (marketplace browser)
- [ ] Worker Setup Wizard (one-click flow)
- [ ] IT Staff Dashboard (bulk deployment)
- [ ] Plugin configuration editor

**Setup Flow:**
- [ ] One-liner setup script (`curl | bash`)
- [ ] Setup token generation and validation
- [ ] Auto-detection of OS and hardware
- [ ] Dependency installation (Rust, Python, Docker, etc.)
- [ ] Worker registration with fleet
- [ ] Sidecar auto-start
- [ ] QR code for mobile setup

#### Phase 5.2 Success Criteria

> **One-Click Setup**: IT staff clicks "Add Worker" → Selects type → Gets one-liner → Emails to employee → Employee runs command → 5 minutes later worker appears as "Active" in Fleet panel

> **Plugin Marketplace**: User opens Plugin Manager → Browses marketplace → Clicks "Install" on worker-devops plugin → Plugin installs Docker, K8s tools → Worker ready for DevOps tasks

---

### Phase 5.3 — Platform Installers 🎯 **DISTRIBUTION**

**Goal**: Professional cross-platform installers for AutoDev desktop app, CLI, and sidecars.

**Depends On**: CLI (bundled in installer), AIRS Plugin (bundled in installer)

#### Installer Architecture

**Supported Platforms:**
- [ ] **Windows**: MSI installer (WiX Toolset v4) + NSIS alternative
- [ ] **macOS**: DMG installer (create-dmg) + PKG for enterprise
- [ ] **Linux**: AppImage (universal) + .deb (Ubuntu/Debian) + .rpm (Fedora/RHEL)

**Installer Features:**
- [ ] Automatic dependency installation (WebView2 on Windows)
- [ ] Sidecar binary bundling (core-orchestrator, memory-mcp-server, Redis)
- [ ] CLI binary installation (`airsctl`)
- [ ] Start menu/Desktop shortcut creation
- [ ] File association (.autodev projects)
- [ ] Auto-updater integration (Tauri updater)
- [ ] Silent install mode for enterprise deployment
- [ ] Custom branding (AIRS Foundry logos, colors)

**Windows Installer (MSI via WiX):**
- [ ] `wix/` directory with WiX source files
- [ ] `AutoDev.wxs`: Main installer manifest
- [ ] Features:
  - Install scope: Per-user (`%LOCALAPPDATA%`) or Per-machine (`%PROGRAMFILES%`)
  - WebView2 runtime check and auto-install
  - Visual C++ Redistributables check
  - Start menu shortcuts
  - Desktop shortcut (optional)
  - File associations (`.autodev` extension)
  - Registry entries (uninstall info, PATH)
  - Sidecar service registration
- [ ] `build-msi.ps1`: Build script
- [ ] CI/CD: GitHub Actions builds MSI on Windows runner

**macOS Installer (DMG):**
- [ ] `dmg/` directory with DMG creation scripts
- [ ] `create-dmg.sh`: DMG creation script
- [ ] Features:
  - Signed and notarized app bundle
  - Drag-and-drop installation
  - Applications folder shortcut
  - Custom DMG background (branded)
  - License agreement display
  - Sidecar binaries in `AutoDev.app/Contents/MacOS/sidecars/`
- [ ] CLI installed to `/usr/local/bin/airsctl` (symlink)
- [ ] CI/CD: GitHub Actions builds DMG on macOS runner

**Linux Installers:**

**AppImage (Universal):**
- [ ] `appimage/` directory with build scripts
- [ ] `build-appimage.sh`: Build script using appimage-builder
- [ ] Features:
  - Single executable file
  - No installation required
  - Works on any Linux distribution
  - Bundles all dependencies
  - Sidecars bundled
  - Desktop integration (optional)
- [ ] `AutoDev-x86_64.AppImage` output

**Debian/Ubuntu (.deb):**
- [ ] `deb/` directory with Debian package files
- [ ] `DEBIAN/control`: Package metadata
- [ ] `usr/bin/airsctl`: CLI binary
- [ ] `usr/share/applications/autodev.desktop`: Desktop entry
- [ ] `usr/share/icons/hicolor/...`: Application icons
- [ ] `opt/autodev/`: Application files
- [ ] Dependencies: `libwebkit2gtk-4.0`, `libgtk-3-0`
- [ ] Post-install script: register MIME type, update desktop database

**Fedora/RHEL (.rpm):**
- [ ] `rpm/` directory with RPM spec file
- [ ] `autodev.spec`: RPM specification
- [ ] Similar structure to .deb
- [ ] Copr repository for distribution

**Installer Build System:**
- [ ] `scripts/build-installers.sh`: Master build script
- [ ] `scripts/build-windows.sh`: Windows-specific
- [ ] `scripts/build-macos.sh`: macOS-specific
- [ ] `scripts/build-linux.sh`: Linux-specific
- [ ] GitHub Actions workflows for CI builds
- [ ] Code signing certificates (Windows, macOS)
- [ ] Release automation (creates GitHub releases with assets)

**Auto-Updater System:**

**Core Concept**: Seamless incremental updates that preserve user data (chats, tasks, plans) and require minimal user intervention. Inspired by VS Code, Discord, and Slack update mechanisms.

**Architecture:**
```
┌─────────────────────────────────────────────────────────────────┐
│                    Auto-Update Flow                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. Background Check (every 4 hours)                             │
│     └── Query update server: GET /api/releases/latest            │
│         └── Compare version: local vs remote                     │
│                                                                  │
│  2. Download Available                                         │
│     └── Show "Update Available" toast in UI                     │
│         └── User clicks "Update" or "Later"                     │
│                                                                  │
│  3. Download & Install (background)                            │
│     └── Download incremental patch (delta update)               │
│         └── Verify signature (Ed25519)                          │
│         └── Apply patch to temp location                        │
│         └── Atomic swap on restart                              │
│                                                                  │
│  4. Data Persistence Guarantee                                  │
│     └── SQLite DB: ~/.autodev/config.db (unchanged)             │
│     └── Analytics DB: ~/.autodev/analytics.db (unchanged)       │
│     └── Sessions: ~/.autodev/sessions/ (preserved)              │
│     └── Plans: ~/.autodev/plans/ (preserved)                    │
│     └── Worktrees: .autodev/worktrees/ (preserved)             │
│     └── Skills: ~/.autodev/skills/ (preserved)                │
│     └── Hooks: ~/.autodev/hooks/ (preserved)                   │
│                                                                  │
│  5. Restart & Resume                                             │
│     └── Graceful shutdown: save all session state               │
│         └── Restart with new binary                             │
│         └── Restore all windows/tabs                           │
│         └── Resume active chats (with history)                   │
│         └── Active plans remain open                            │
│         └── Running tasks continue (via sidecars)              │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Update Server API:**
```
GET /api/releases/latest
Response:
{
  "version": "1.2.3",
  "release_date": "2026-01-15",
  "release_notes": "Bug fixes and performance improvements...",
  "critical": false,  // true = force update
  "platforms": {
    "windows-x64": {
      "full_url": "https://releases.airs.dev/1.2.3/AutoDev_1.2.3_x64.msi",
      "delta_url": "https://releases.airs.dev/1.2.3/AutoDev_1.1.0_to_1.2.3_x64.patch",
      "full_size": 85000000,
      "delta_size": 5000000,
      "signature": "base64-ed25519-sig"
    },
    "macos-x64": { ... },
    "macos-arm64": { ... },
    "linux-x64": { ... }
  }
}
```

**Backend — Updater Module:**
- [ ] `src-tauri/src/updater/mod.rs`: Updater module
- [ ] `src-tauri/src/updater/check.rs`: Version checking logic
- [ ] `src-tauri/src/updater/download.rs`: Incremental download with progress
- [ ] `src-tauri/src/updater/verify.rs`: Signature verification (Ed25519)
- [ ] `src-tauri/src/updater/patch.rs`: Delta patch application (bsdiff/bsdcat or similar)
- [ ] `src-tauri/src/updater/install.rs`: Atomic installation
- [ ] `src-tauri/src/updater/persistence.rs`: Data preservation verification

**Tauri Commands:**
- [ ] `check_for_update() -> Option<UpdateInfo>`: Check if update available
- [ ] `download_update(version) -> DownloadProgress`: Start download
- [ ] `get_download_progress() -> DownloadProgress`: Poll download status
- [ ] `install_update() -> InstallResult`: Apply update and restart
- [ ] `skip_version(version) -> ()`: Skip this version (don't ask again)
- [ ] `get_update_history() -> Vec<UpdateRecord>`: History of updates
- [ ] `verify_data_integrity() -> IntegrityReport`: Post-update verification

**Incremental Update Strategy:**
- [ ] **Delta Updates**: Only download changed files (using bsdiff for binary diff)
- [ ] **Fallback to Full**: If delta fails, download full installer
- [ ] **Background Download**: Download while app is running
- [ ] **Atomic Installation**: New version installed to temp, swapped on restart
- [ ] **Rollback on Failure**: If new version fails, auto-restore previous

**Data Persistence Guarantees:**
- [ ] **SQLite Databases**: Never touched by updater (in ~/.autodev/, not app bundle)
- [ ] **Session State**: All chat sessions preserved
- [ ] **Active Plans**: Plan state restored after update
- [ ] **Running Tasks**: Sidecars continue running (separate process)
- [ ] **User Configuration**: ~/.airs/config.toml preserved
- [ ] **Installed Plugins**: ~/.airs/plugins/ preserved
- [ ] **Worktrees**: .autodev/worktrees/ preserved (external to app)
- [ ] **Checkpoints**: .autodev/checkpoints/ preserved

**Pre-Update Checklist (Atomic):**
```rust
pub async fn prepare_update() -> Result<UpdatePrep, UpdateError> {
    // 1. Verify all databases are closed cleanly
    close_all_databases().await?;
    
    // 2. Flush session state to disk
    save_all_session_snapshots().await?;
    
    // 3. Verify data integrity
    verify_database_integrity()?;
    
    // 4. Create restore point (manifest of current state)
    create_restore_point()?;
    
    // 5. Signal sidecars to persist state
    notify_sidecars_save_state().await?;
    
    Ok(UpdatePrep { restore_point })
}
```

**Post-Update Restoration:**
```rust
pub async fn restore_after_update() -> Result<(), RestoreError> {
    // 1. Verify data files exist
    verify_data_presence()?;
    
    // 2. Reopen databases
    open_all_databases().await?;
    
    // 3. Restore session windows
    restore_session_tabs().await?;
    
    // 4. Reconnect to sidecars (they kept running)
    reconnect_sidecars().await?;
    
    // 5. Verify plan states
    verify_plan_integrity()?;
    
    // 6. Clean up old version files
    cleanup_old_versions()?;
    
    Ok(())
}
```

**Frontend — Update UI:**

**Update Notification (`src/components/update/UpdateToast.tsx`):**
- [ ] Toast notification: "Update 1.2.3 available"
- [ ] Shows: version, release notes snippet, download size
- [ ] Buttons: "Update Now", "Later", "Release Notes"
- [ ] Auto-dismiss after 10 seconds (unless critical update)

**Update Dialog (`src/components/update/UpdateDialog.tsx`):**
- [ ] Full-screen modal with release notes (Markdown rendering)
- [ ] Progress bar for download
- [ ] "Downloading... 45%" status
- [ ] "Ready to Install" when download complete
- [ ] "Restart to Update" button
- [ ] "Install on Quit" option (update when closing app)

**Update Settings (`src/components/settings/UpdateSettings.tsx`):**
- [ ] Auto-check for updates toggle
- [ ] Update channel selection (stable, beta, nightly)
- [ ] "Check for Updates" button (manual)
- [ ] Update history viewer
- [ ] Data backup before update toggle

**Update Store (`src/stores/updateStore.ts`):**
- [ ] State: `updateAvailable`, `downloadProgress`, `isDownloading`, `releaseNotes`
- [ ] Actions: `checkForUpdate()`, `downloadUpdate()`, `installUpdate()`, `skipVersion()`

**Critical Updates:**
- [ ] Critical security updates force installation
- [ ] Countdown timer: "Restarting in 60 seconds..."
- [ ] Can postpone but not skip
- [ ] Red notification badge on Settings icon

**Enterprise Update Policy:**
- [ ] Disable auto-updates via Group Policy (Windows)
- [ ] Internal update server configuration
- [ ] Staged rollouts (IT can control update pace)
- [ ] Update approval workflow
- [ ] Version pinning (stay on specific version)

**Per-Platform Implementation:**

**Windows:**
- [ ] Use Tauri's built-in updater (NSIS/MSI compatible)
- [ ] WiX Burn bootstrapper for .NET/runtime deps
- [ ] Windows Update service integration (optional)
- [ ] Code signing required for auto-update

**macOS:**
- [ ] Sparkle framework integration (industry standard)
- [ ] Notarization check before applying update
- [ ] User notification via Notification Center
- [ ] App translocation handling

**Linux:**
- [ ] AppImage: use AppImageUpdate for delta updates
- [ ] deb/rpm: use package manager (apt, dnf) for updates
- [ ] Flatpak/ Snap support (optional)
- [ ] Polkit integration for elevation

**Offline Mode:**
- [ ] Detect offline state, skip update checks
- [ ] Queue update check for when online
- [ ] Manual update via downloaded .msi/.dmg

**Rollback System:**
- [ ] Keep last 2 versions for rollback
- [ ] "Revert to Previous Version" in settings
- [ ] Automatic rollback if new version crashes on start
- [ ] Telemetry on rollback events

**Enterprise Deployment:**
- [ ] MSI transforms for enterprise customization
- [ ] Group Policy support (Windows)
- [ ] MDM deployment profile (macOS)
- [ ] Silent install flags for all platforms
- [ ] Custom configuration deployment
- [ ] License key validation (optional)

**IT Staff Runbook (`docs/IT_RUNBOOK.md`):**
- [ ] Pre-deployment checklist
- [ ] System requirements (CPU, RAM, disk, OS versions)
- [ ] Network requirements (ports, firewall rules)
- [ ] Step-by-step installation guide per platform
- [ ] Silent deployment instructions
- [ ] Troubleshooting guide
- [ ] Update procedures
- [ ] Uninstall procedures

#### Phase 5.3 Deliverables Checklist

**Windows Installer:**
- [ ] WiX v4 project files (`wix/`)
- [ ] MSI installer with per-user and per-machine options
- [ ] WebView2 runtime auto-install
- [ ] Sidecar binary bundling
- [ ] Start menu and desktop shortcuts
- [ ] File associations (`.autodev`)
- [ ] Registry entries for uninstall
- [ ] Code signing certificate

**macOS Installer:**
- [ ] DMG creation scripts (`dmg/`)
- [ ] Signed and notarized app bundle
- [ ] Drag-and-drop installation
- [ ] Branded DMG background
- [ ] CLI symlink in `/usr/local/bin/`
- [ ] Sidecar bundling

**Linux Installers:**
- [ ] AppImage build scripts (`appimage/`)
- [ ] .deb package (`deb/`)
- [ ] .rpm package (`rpm/`)
- [ ] Desktop integration
- [ ] Dependency declarations
- [ ] Post-install scripts

**Build System:**
- [ ] Cross-platform build scripts
- [ ] GitHub Actions CI/CD
- [ ] Automated release creation
- [ ] Code signing integration
- [ ] Version management

**Auto-Updater:**
- [ ] Background update check every 4 hours
- [ ] Incremental delta updates (bsdiff)
- [ ] Signature verification (Ed25519)
- [ ] Atomic installation with rollback support
- [ ] Data persistence guarantees (DBs, sessions, plans, tasks)
- [ ] Pre-update checklist and post-update restoration
- [ ] Update notification toast and dialog UI
- [ ] Critical update force-install capability
- [ ] Update channels (stable, beta, nightly)
- [ ] Skip version functionality
- [ ] Update history viewer
- [ ] Enterprise update policy controls
- [ ] Offline mode detection
- [ ] Rollback to previous version
- [ ] 7 Tauri commands for update management

**Enterprise:**
- [ ] Silent install mode
- [ ] Group Policy support (Windows)
- [ ] MDM deployment (macOS)
- [ ] Custom configuration deployment
- [ ] IT Staff Runbook document

#### Phase 5.3 Success Criteria

> **Windows Install**: User downloads `AutoDev_1.0.0_x64.msi` → Double-clicks → UAC prompt → Install completes → Desktop shortcut appears → App launches → Sidecars auto-start

> **macOS Install**: User downloads `AutoDev-1.0.0.dmg` → Opens DMG → Drags to Applications → Launches from Launchpad → "AutoDev is an app downloaded from the internet" → Opens successfully

> **Linux Install**: User downloads `AutoDev-1.0.0.AppImage` → `chmod +x` → Runs → Works on any distro → Or installs .deb with `apt install ./autodev_1.0.0_amd64.deb`

> **Enterprise Deploy**: IT admin runs `msiexec /i AutoDev.msi /qn WORKERTYPE=backend FLEETTOKEN=xxx` → Silent install on 100 PCs → All workers auto-join fleet

> **Auto-Update**: User sees toast "Update 1.2.3 available" → Clicks "Update Now" → Download progresses in background → "Restart to Update" appears → User restarts → App reopens with all 5 chats intact, active plan still visible, sidecars reconnected → Version shows 1.2.3

---

### Phase 5.4 — Dashboard + Analytics 🎯 **FLEET MANAGEMENT**

**Goal**: Comprehensive cost dashboard and analytics for tracking usage, costs, worker efficiency, and system health.

**Depends On**: All previous sub-phases (uses CLI data, worker data, plugin data)

#### Dashboard Architecture

**Core Concept**: New "Dashboard" tab with real-time analytics, cost tracking, and fleet management insights. Inspired by Claude Code's `/cost` command and cloud provider dashboards.

**Data Collection Pipeline:**
```
Events (Tauri, CLI, Workers)
         │
         ▼
┌─────────────────┐
│  Event Bus      │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Analytics DB    │  (SQLite: ~/.autodev/analytics.db)
│ - token_usage   │
│ - costs         │
│ - worker_stats  │
│ - api_failures  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Aggregator     │  (background job)
│ - hourly rollups│
│ - daily summaries│
│ - worker metrics│
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Dashboard UI   │  (React + Recharts)
│ - real-time     │
│ - historical    │
│ - predictive    │
└─────────────────┘
```

**Analytics Database Schema:**
```sql
-- Token usage per request
CREATE TABLE token_usage (
    id INTEGER PRIMARY KEY,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    session_id TEXT,
    agent_type TEXT,  -- PA, Supervisor, Worker, QC
    provider TEXT,
    model TEXT,
    prompt_tokens INTEGER,
    completion_tokens INTEGER,
    total_tokens INTEGER,
    thinking_tokens INTEGER,
    cost_usd DECIMAL(10,6),
    request_type TEXT  -- chat, tool_use, summarization
);

-- Aggregated daily costs
CREATE TABLE daily_costs (
    date DATE PRIMARY KEY,
    total_cost_usd DECIMAL(10,2),
    total_tokens INTEGER,
    session_count INTEGER,
    message_count INTEGER,
    tool_call_count INTEGER,
    provider_breakdown JSON,  -- {"anthropic": 1.23, "openai": 0.45}
    agent_type_breakdown JSON -- {"PA": 0.80, "Worker": 0.88}
);

-- Worker statistics
CREATE TABLE worker_stats (
    worker_id TEXT PRIMARY KEY,
    date DATE,
    tasks_completed INTEGER,
    tasks_failed INTEGER,
    time_spent_minutes INTEGER,
    cost_usd DECIMAL(10,2),
    avg_task_duration_minutes INTEGER,
    success_rate DECIMAL(5,2),
    last_heartbeat DATETIME
);

-- API failures
CREATE TABLE api_failures (
    id INTEGER PRIMARY KEY,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    provider TEXT,
    error_type TEXT,  -- rate_limit, auth, timeout, model_error
    error_message TEXT,
    request_id TEXT,
    retry_count INTEGER,
    resolved BOOLEAN DEFAULT 0
);

-- System events
CREATE TABLE system_events (
    id INTEGER PRIMARY KEY,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    component TEXT,  -- sidecar, provider, tool, agent
    event_type TEXT, -- start, stop, error, warning
    message TEXT,
    severity TEXT,   -- info, warning, error, critical
    metadata JSON
);
```

**Backend — Analytics Module:**
- [ ] `src-tauri/src/analytics/mod.rs`: Analytics module
- [ ] `src-tauri/src/analytics/collector.rs`: Event collection
- [ ] `src-tauri/src/analytics/aggregator.rs`: Data aggregation (hourly, daily)
- [ ] `src-tauri/src/analytics/queries.rs`: Query functions for dashboard
- [ ] `src-tauri/src/analytics/exporter.rs`: CSV/JSON export

**Tauri Commands:**
- [ ] `get_dashboard_summary() -> DashboardSummary`: Today's stats vs yesterday
- [ ] `get_cost_analytics(range, granularity) -> CostAnalytics`: Cost trends
- [ ] `get_token_usage(range, filters) -> TokenUsageData`: Token consumption
- [ ] `get_worker_analytics(worker_id?, range) -> WorkerAnalytics`: Worker stats
- [ ] `get_api_failures(range, filters) -> Vec<ApiFailure>`: Error tracking
- [ ] `get_system_logs(component?, severity?, range) -> Vec<SystemEvent>`
- [ ] `export_analytics(format, range, path) -> ExportResult`: Data export
- [ ] `predict_monthly_cost() -> Prediction`: ML-based cost prediction

#### Dashboard UI Components

**Dashboard Tab Layout:**
```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Dashboard                                            [Export] [Refresh]    │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐          │
│  │  💰 Cost    │ │  🪙 Tokens  │ │  ⚡ Workers  │ │  📊 Sessions│          │
│  │   Today     │ │   Today     │ │   Active    │ │   Today     │          │
│  │  $12.45     │ │  125K       │ │  12 / 25    │ │  8          │          │
│  │  ↑ 23%      │ │  ↓ 5%       │ │  +2 today   │ │  ↑ 2        │          │
│  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘          │
├─────────────────────────────────────────────────────────────────────────────┤
│  Cost Trend (7 Days)          │  Worker Efficiency                         │
│  ┌─────────────────────────┐  │  ┌───────────────────────────────────┐   │
│  │      📈 Line Chart       │  │  │  🥇 worker-backend-01: 95%       │   │
│  │  Daily cost over week   │  │  │  🥈 worker-frontend-03: 92%      │   │
│  └─────────────────────────┘  │  │  🥉 worker-devops-01: 88%         │   │
│                               │  └───────────────────────────────────┘   │
├───────────────────────────────┴─────────────────────────────────────────────┤
│  Model Usage Breakdown          │  Recent API Failures                    │
│  ┌─────────────────────────┐    │  ⚠️ 3 errors in last hour              │
│  │     🥧 Pie Chart        │    │  • Anthropic rate limit (2 min ago)    │
│  │  Claude: 60%            │    │  • Ollama timeout (15 min ago)         │
│  │  GPT-4: 30%             │    │  • GitHub API error (45 min ago)       │
│  │  Ollama: 10%            │    │                                        │
│  └─────────────────────────┘    │  [View All]                            │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Widget Components:**

**1. Cost Meter Widget (`src/components/dashboard/CostMeter.tsx`):**
- [ ] Today's cost vs yesterday (percentage change)
- [ ] Progress bar toward daily budget (if set)
- [ ] "This month" projection based on current rate
- [ ] Drill-down: click to see cost breakdown by provider/agent

**2. Token Usage Widget (`src/components/dashboard/TokenUsage.tsx`):**
- [ ] Today's tokens (prompt vs completion ratio)
- [ ] Historical trend (7-day line chart)
- [ ] Model breakdown (pie chart)
- [ ] Per-session token consumption (top 5 sessions)

**3. Worker Status Widget (`src/components/dashboard/WorkerStatus.tsx`):**
- [ ] Active workers / Total workers
- [ ] Workers added today
- [ ] Worker health indicator (green/yellow/red)
- [ ] Quick actions: "View All Workers", "Add Worker"

**4. Session Activity Widget (`src/components/dashboard/SessionActivity.tsx`):**
- [ ] Sessions created today
- [ ] Active sessions now
- [ ] Messages sent today
- [ ] Tool calls executed today

**5. Model Usage Breakdown (`src/components/dashboard/ModelUsage.tsx`):**
- [ ] Pie/donut chart of token distribution by model
- [ ] Cost per model
- [ ] Efficiency ranking (cost per successful task)

**6. Worker Efficiency Leaderboard (`src/components/dashboard/WorkerLeaderboard.tsx`):**
- [ ] Top performing workers by success rate
- [ ] Average task duration per worker type
- [ ] Cost per task per worker
- [ ] Trending up/down indicators

**7. API Failures Panel (`src/components/dashboard/ApiFailures.tsx`):**
- [ ] Recent errors with timestamps
- [ ] Error categorization (rate limit, auth, timeout, etc.)
- [ ] Provider-specific failure rates
- [ ] Auto-retry success rate

**Chart Library:**
- [ ] `recharts` for React-based charts
- [ ] Line charts for trends over time
- [ ] Bar charts for comparisons
- [ ] Pie/donut charts for breakdowns
- [ ] Real-time updates via Tauri events

**Dashboard Store (`src/stores/dashboardStore.ts`):**
- [ ] State: `summary`, `costData`, `tokenData`, `workerData`, `failures`
- [ ] Actions: `refreshDashboard()`, `setDateRange()`, `exportData()`
- [ ] Real-time updates via Tauri events

#### Analytics Features

**Cost Tracking:**
- [ ] Per-session cost tracking (stored in analytics DB)
- [ ] Per-worker cost tracking (for fleet billing)
- [ ] Per-provider cost breakdown
- [ ] Per-model cost comparison
- [ ] Cost per task type (feature, bug, refactor)
- [ ] Cost projections ("At current rate, monthly cost will be $X")
- [ ] Budget alerts ("Daily cost exceeded $Y")

**Token Analytics:**
- [ ] Token usage by agent type (PA vs Worker vs QC)
- [ ] Prompt vs completion token ratio
- [ ] Thinking tokens (for reasoning models)
- [ ] Token efficiency (tokens per successful task)
- [ ] Context window utilization tracking

**Worker Analytics:**
- [ ] Tasks completed per worker per day
- [ ] Success rate per worker
- [ ] Average task duration
- [ ] Cost per task
- [ ] Worker utilization rate (% of time working)
- [ ] Worker idle time tracking

**System Health:**
- [ ] Sidecar uptime/downtime
- [ ] Provider API availability
- [ ] Tool execution failure rates
- [ ] Session crash tracking
- [ ] Error log aggregation

**Export & Reporting:**
- [ ] CSV export for any data range
- [ ] JSON export for programmatic access
- [ ] PDF report generation (weekly, monthly)
- [ ] Scheduled reports (email delivery)
- [ ] Custom date range selection

**Predictive Analytics:**
- [ ] Monthly cost prediction based on current usage
- [ ] Worker capacity planning ("You need X more workers")
- [ ] Optimal model recommendation ("Use Ollama for task Y to save $Z")

#### Phase 5.4 Deliverables Checklist

**Backend — Analytics:**
- [ ] `src-tauri/src/analytics/` module
- [ ] Analytics database schema (4 tables: token_usage, daily_costs, worker_stats, api_failures)
- [ ] Event collection pipeline
- [ ] Background aggregation job (hourly, daily rollups)
- [ ] Query functions for dashboard
- [ ] CSV/JSON exporter
- [ ] Cost prediction algorithm
- [ ] 15+ Tauri commands for analytics queries

**Frontend — Dashboard:**
- [ ] New "Dashboard" tab in main navigation
- [ ] 7 dashboard widgets (Cost Meter, Token Usage, Worker Status, Session Activity, Model Usage, Worker Leaderboard, API Failures)
- [ ] `recharts` integration for charts
- [ ] Real-time updates via Tauri events
- [ ] Date range picker (today, week, month, custom)
- [ ] Export buttons (CSV, JSON)
- [ ] `src/stores/dashboardStore.ts` for state management

**Analytics Features:**
- [ ] Cost tracking (per session, worker, provider, model, task type)
- [ ] Token analytics (usage by agent type, prompt/completion ratio)
- [ ] Worker analytics (tasks, success rate, duration, cost per task)
- [ ] System health monitoring (sidecars, providers, tools)
- [ ] Export & reporting (CSV, JSON, PDF)
- [ ] Predictive analytics (cost prediction, capacity planning)
- [ ] Budget alerts and notifications

#### Phase 5.4 Success Criteria

> **Cost Tracking**: User opens Dashboard → Sees "Today: $12.45" → Clicks widget → Sees breakdown by provider (Anthropic $8, OpenAI $4.45) → Exports CSV → Data shows per-session costs

> **Worker Analytics**: Manager views Worker Leaderboard → Sees top worker "worker-backend-01" with 95% success rate → Clicks worker → Sees task history, average duration 45min, cost $2.30/task

> **API Monitoring**: Dashboard shows "3 API failures in last hour" → User clicks → Sees Anthropic rate limit errors → Adjusts rate limiting → Errors decrease

> **Cost Prediction**: Dashboard shows "Projected monthly cost: $450" based on 7-day average → User sets budget alert at $400 → Alert triggers when threshold exceeded

---

### Phase 5 Dependency Graph

```
Phase 5 Sub-Phase Dependencies
═══════════════════════════════════════════════════════════════════════════════

Phase 5.1 (CLI)
│
├─► [CRITICAL PATH] Automation Foundation
│   ├─ CI/CD integration
│   ├─ Worker automation scripts
│   ├─ Headless operation
│   └─ Configuration management
│
├─► [DEPENDENCY] Phase 5.2 AIRS Plugin
│   └─ Uses airsctl for setup
│
├─► [DEPENDENCY] Phase 5.3 Installers
│   └─ CLI bundled in installers
│
└─► [DEPENDENCY] Phase 5.4 Dashboard
    ├─ CLI events feed analytics
    └─ airsctl analytics commands

Phase 5.2 (AIRS Plugin)
│
├─► [DEPENDENCY] Phase 5.3 Installers
│   └─ Plugin bundled in installers
│
└─► [DEPENDENCY] Phase 5.4 Dashboard
    └─ Worker setup events tracked

Phase 5.3 (Installers)
└─► [DEPENDENCY] Phase 5.4 Dashboard
    └─ Install events tracked

═══════════════════════════════════════════════════════════════════════════════

Phase 5 Cross-Sub-Phase Integration:

┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Phase 5.1     │────►│   Phase 5.2     │────►│   Phase 5.3     │
│  CLI (airsctl)  │     │  AIRS Plugin    │     │  Installers     │
│                 │     │                 │     │                 │
│ • 30+ commands  │     │ • One-click     │     │ • MSI/DMG/deb   │
│ • CI/CD ready   │────►│   setup         │────►│ • Sidecars      │
│ • REPL mode     │     │ • Auto-detect   │     │ • Auto-update   │
└────────┬────────┘     └─────────────────┘     └─────────────────┘
         │                                               │
         │                       ┌───────────────────────┘
         │                       │
         └───────────────────────┴───────────────────┐
                             │                      │
                             ▼                      │
                    ┌─────────────────┐             │
                    │   Phase 5.4     │◄────────────┘
                    │  Dashboard      │
                    │                 │
                    │ • Cost tracking │
                    │ • Worker stats  │
                    │ • Analytics     │
                    └─────────────────┘
```

### Phase 5 Summary Statistics

| Metric | Count |
|--------|-------|
| **Total Sub-Phases** | 4 (5.1, 5.2, 5.3, 5.4) |
| **Total Checklist Items** | 170+ |
| **CLI Commands** | 30+ across 7 categories |
| **Platform Installers** | 5 (MSI, DMG, AppImage, deb, rpm) |
| **Auto-Updater Features** | 15+ (delta updates, atomic install, rollback) |
| **Dashboard Widgets** | 7 |
| **Analytics Tables** | 4 |
| **Built-in Plugins** | 5+ |

### Phase 5 Success Criteria (Overall)

> **CLI Success**: `airsctl agent run "implement auth" --non-interactive` → Runs headless → Outputs JSON → CI/CD pipeline continues

> **Plugin Success**: IT staff clicks "Deploy Worker" → Selects type → Gets one-liner → Employee runs it → 5 min later worker shows "Active" in Dashboard

> **Installer Success**: New user downloads MSI → Installs → Desktop shortcut works → App launches → All sidecars running

> **Dashboard Success**: Manager opens Dashboard → Sees $12.45 spent today → Clicks worker leaderboard → Sees efficiency rankings → Exports monthly report → Sends to finance

---

## 14. Phase 6 — Graph Engine

**Goal**: Build AutoDev's native code knowledge graph engine — SQLite-persistent, tree-sitter-extracted, Leiden-clustered, MCP-served, D3.js-visualized. Gives PA and Worker agents surgical graph queries instead of token-burning full-file reads. Replaces 8× token waste with precise blast-radius context.

> **Reference Document**: `docs/reference/autodev-graph-master-spec.md` — full architecture analysis of graphify-5 and code-review-graph reference codebases, combined design, node/edge data model, SQLite schema, 7-stage pipeline, and all 51 atomic tasks.

> **Architecture Decision**: Rust-native extraction (tree-sitter Rust bindings via `tree-sitter` + `tree-sitter-language-pack` crates), SQLite persistence via `rusqlite`, NetworkX-equivalent analysis in pure Rust using `petgraph`. MCP server is the existing `autodev-mcp-server` (port 8767) — graph tools are registered as builtin tools just like `create_task` and `list_tasks`. No new sidecar.

---

### Phase 6.1 — Storage Layer

**Goal**: Define the SQLite schema and Rust data access layer that everything else builds on.

> **New file**: `src-tauri/src/graph/` module directory. Add `mod graph;` to `src-tauri/src/lib.rs`.

- [ ] **Create `src-tauri/src/graph/mod.rs`** — re-exports all graph submodules:
  - `pub mod store;`, `pub mod schema;`, `pub mod migrations;`, `pub mod detect;`, `pub mod extract;`, `pub mod build;`, `pub mod cluster;`, `pub mod analyze;`, `pub mod flows;`, `pub mod changes;`, `pub mod incremental;`, `pub mod query;`, `pub mod report;`, `pub mod export;`, `pub mod registry;`, `pub mod cache;`

- [ ] **Create `src-tauri/src/graph/schema.rs`** — raw SQL constants only:
  - `pub const CREATE_NODES: &str` — nodes table DDL (id, kind, name, qualified_name UNIQUE, file_path, line_start, line_end, language, parent_name, params, return_type, modifiers, is_test, file_hash, community, community_name, agent_author, task_id, worker_locked, worktree, extra JSON, updated_at)
  - `pub const CREATE_EDGES: &str` — edges table DDL (id, kind, source_qualified, target_qualified, file_path, line, confidence REAL, confidence_tier TEXT, weight REAL, relation, agent_author, task_id, extra JSON, updated_at)
  - `pub const CREATE_COMMUNITIES: &str` — communities table (id, name, cohesion_score, member_count, dominant_language, updated_at)
  - `pub const CREATE_FLOWS: &str` — flows table (id, entry_point, flow_path JSON, criticality_score, is_tested, has_security_keyword, updated_at)
  - `pub const CREATE_SNAPSHOTS: &str` — snapshots table (id, label, node_count, edge_count, graph_json TEXT, created_at)
  - `pub const CREATE_ANALYSIS_CACHE: &str` — analysis_cache table (key TEXT PRIMARY KEY, value JSON, graph_hash, updated_at)
  - `pub const CREATE_NODES_FTS: &str` — FTS5 virtual table DDL: `CREATE VIRTUAL TABLE nodes_fts USING fts5(name, qualified_name, file_path, params, return_type, content='nodes', content_rowid='rowid', tokenize='porter unicode61')`
  - `pub const CREATE_REPOS: &str` — repos registry table (id, path UNIQUE, alias, db_path, last_updated, node_count, edge_count)
  - `pub const ALL_INDEXES: &[&str]` — array of all `CREATE INDEX IF NOT EXISTS` statements (idx_nodes_file, idx_nodes_qualified, idx_nodes_community, idx_nodes_task, idx_nodes_author, idx_edges_source, idx_edges_target, idx_edges_kind, idx_edges_target_kind, idx_edges_source_kind, idx_edges_file, idx_edges_tier)

- [ ] **Create `src-tauri/src/graph/migrations.rs`** — versioned schema migrations:
  - `pub fn get_schema_version(conn: &Connection) -> u32` — reads `PRAGMA user_version`
  - `pub fn run_migrations(conn: &mut Connection) -> Result<()>` — runs all pending migrations in order via `conn.execute_batch()`:
    - v0→v1: create nodes + edges + metadata tables + all indexes
    - v1→v2: add communities table
    - v2→v3: add flows + snapshots tables
    - v3→v4: add analysis_cache table + FTS5 virtual table
    - v4→v5: add repos table + agent_author/task_id/worker_locked/worktree columns to nodes and edges
  - Each migration wrapped in explicit `BEGIN`/`COMMIT`, sets `PRAGMA user_version = N` on success
  - `pub fn migration_v1(conn: &mut Connection) -> Result<()>` ... `migration_v5(conn: &mut Connection) -> Result<()>` — individual migration functions

- [ ] **Create `src-tauri/src/graph/store.rs`** — `GraphStore` struct, SQLite CRUD:
  - `pub struct GraphStore { conn: Connection, db_path: PathBuf }` — single connection, WAL mode
  - `pub fn GraphStore::open(db_path: &Path) -> Result<Self>` — opens/creates DB, enables WAL (`PRAGMA journal_mode=WAL`), `PRAGMA foreign_keys=ON`, calls `run_migrations()`
  - `pub fn GraphStore::upsert_node(node: &NodeInfo, agent_author: Option<&str>, task_id: Option<i64>) -> Result<i64>` — `INSERT OR REPLACE INTO nodes` with all fields; returns rowid
  - `pub fn GraphStore::upsert_edge(edge: &EdgeInfo, agent_author: Option<&str>, task_id: Option<i64>) -> Result<i64>` — `INSERT OR REPLACE INTO edges`
  - `pub fn GraphStore::delete_file_nodes(file_path: &str) -> Result<usize>` — deletes all nodes WHERE file_path = ?; returns count deleted
  - `pub fn GraphStore::delete_file_edges(file_path: &str) -> Result<usize>` — deletes all edges WHERE file_path = ?
  - `pub fn GraphStore::get_node(qualified_name: &str) -> Result<Option<GraphNode>>` — SELECT by qualified_name
  - `pub fn GraphStore::get_edges_for_node(qualified_name: &str) -> Result<Vec<GraphEdge>>` — SELECT WHERE source_qualified = ? OR target_qualified = ?
  - `pub fn GraphStore::get_stats() -> Result<GraphStats>` — node/edge counts by kind + language list + last_updated
  - `pub fn GraphStore::list_nodes_by_file(file_path: &str) -> Result<Vec<GraphNode>>` — SELECT WHERE file_path = ?
  - `pub fn GraphStore::get_file_hash(file_path: &str) -> Result<Option<String>>` — `SELECT file_hash FROM nodes WHERE file_path = ? LIMIT 1`
  - `pub fn GraphStore::set_community(qualified_name: &str, community: i32, community_name: &str) -> Result<()>`
  - `pub fn GraphStore::get_all_nodes_for_graph() -> Result<Vec<GraphNode>>` — full table scan for analysis/export (paginated by default)
  - `pub fn GraphStore::get_all_edges_for_graph() -> Result<Vec<GraphEdge>>` — full table scan for analysis/export
  - Data structs `GraphNode`, `GraphEdge`, `GraphStats` — `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]`

- [ ] **Implement FTS5 rebuild in `store.rs`**:
  - `pub fn GraphStore::rebuild_fts_index() -> Result<usize>` — `DROP TABLE IF EXISTS nodes_fts`, `CREATE VIRTUAL TABLE nodes_fts ...`, `INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild')` — all inside `BEGIN IMMEDIATE`/`COMMIT`; returns row count
  - Called automatically at end of every full build; called incrementally after upserts on changed files

- [ ] **Add `Cargo.toml` dependencies** for graph engine:
  - `rusqlite = { version = "0.31", features = ["bundled"] }` (already used in project — confirm version)
  - `tree-sitter = "0.22"` — core tree-sitter runtime
  - `petgraph = "0.6"` — graph algorithms (BFS, DFS, shortest path)
  - `sha2 = "0.10"` — SHA-256 file hashing
  - `rayon = "1.10"` — parallel file parsing
  - `walkdir = "2"` — directory traversal
  - `ignore = "0.4"` — .gitignore-aware file filtering
  - Language grammars: `tree-sitter-python`, `tree-sitter-javascript`, `tree-sitter-typescript`, `tree-sitter-rust`, `tree-sitter-go`, `tree-sitter-java`, `tree-sitter-cpp`, `tree-sitter-c`, `tree-sitter-c-sharp`, `tree-sitter-ruby`, `tree-sitter-kotlin` (add remaining languages incrementally)

**6.1 Success Criteria:**
> `GraphStore::open("~/.autodev/graph.db")` succeeds → WAL mode confirmed → migrations v1–v5 applied → `get_stats()` returns zero-count GraphStats → `upsert_node()` + `get_node()` round-trips correctly

---

### Phase 6.2 — Detection Layer

**Goal**: Directory walker that produces a filtered list of source file paths, respecting .gitignore and a default ignore pattern list.

- [ ] **Create `src-tauri/src/graph/detect.rs`**:
  - `pub const CODE_EXTENSIONS: &[&str]` — full list of 35+ extensions: `.py`, `.js`, `.mjs`, `.jsx`, `.ts`, `.tsx`, `.go`, `.rs`, `.java`, `.c`, `.h`, `.cpp`, `.cc`, `.cxx`, `.hpp`, `.cs`, `.rb`, `.php`, `.kt`, `.swift`, `.scala`, `.vue`, `.svelte`, `.dart`, `.r`, `.lua`, `.sol`, `.ex`, `.exs`, `.zig`, `.jl`, `.sh`, `.bash`, `.ps1`, `.m`, `.pl`, `.pm`, `.res`, `.gd`, `.ipynb`
  - `pub const DEFAULT_IGNORE_PATTERNS: &[&str]` — 30+ patterns: `node_modules/**`, `.git/**`, `.venv/**`, `venv/**`, `__pycache__/**`, `dist/**`, `build/**`, `target/**`, `.next/**`, `vendor/**`, `.dart_tool/**`, `.pub-cache/**`, `.autodev/**`, `coverage/**`, `*.min.js`, `*.min.css`, `*.map`, `*.lock`, `package-lock.json`, `yarn.lock`, `*.pyc`, `*.db`, `*.sqlite`
  - `pub struct LanguageConfig` — per-language tree-sitter grammar info:
    ```rust
    pub struct LanguageConfig {
        pub language: Language,        // tree-sitter Language handle
        pub class_node_types: &'static [&'static str],
        pub function_node_types: &'static [&'static str],
        pub import_node_types: &'static [&'static str],
        pub call_node_types: &'static [&'static str],
        pub name_field: &'static str,  // default "name"
        pub body_field: &'static str,  // default "body"
        pub function_label_parens: bool,
    }
    ```
  - `pub fn get_language_config(extension: &str) -> Option<LanguageConfig>` — maps extension → LanguageConfig; uses `match` over all CODE_EXTENSIONS
  - `pub fn collect_files(root: &Path, extra_ignore: &[&str]) -> Vec<PathBuf>` — uses `ignore::WalkBuilder` for .gitignore support; applies DEFAULT_IGNORE_PATTERNS; filters by CODE_EXTENSIONS; returns sorted `Vec<PathBuf>`

**6.2 Success Criteria:**
> `collect_files(Path::new("."), &[])` on the autodev repo returns all `.rs`, `.ts`, `.tsx` files, excludes `node_modules/`, `target/`, `.git/`

---

### Phase 6.3 — Extraction Layer

**Goal**: Tree-sitter AST walker that converts source files into `ExtractionResult` (nodes + edges). Per-language import handlers. SHA-256 file cache to skip re-parsing unchanged files.

- [ ] **Create `src-tauri/src/graph/cache.rs`** — extraction result cache:
  - `pub struct CacheEntry { file_hash: String, result: ExtractionResult, cached_at: u64 }`
  - Cache storage: `~/.autodev/graph_cache/` directory, one `.json` file per source file (path-hashed filename)
  - `pub fn load_cached(file_path: &Path, file_hash: &str) -> Option<ExtractionResult>` — reads cache file, validates hash matches, returns `Some(result)` or `None`
  - `pub fn save_cached(file_path: &Path, file_hash: &str, result: &ExtractionResult) -> Result<()>` — serializes to JSON, writes to cache dir
  - `pub fn hash_file(path: &Path) -> Result<String>` — reads file bytes, returns `hex::encode(Sha256::digest(&bytes))`

- [ ] **Create `src-tauri/src/graph/extract.rs`** — core extraction:
  - `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)] pub struct NodeInfo` — mirrors spec: kind (NodeKind enum), name, file_path, line_start, line_end, language, parent_name, params, return_type, is_test, extra (serde_json::Value)
  - `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)] pub struct EdgeInfo` — kind (EdgeKind enum), source (qualified name), target (qualified name), file_path, line, confidence_tier (ConfidenceTier enum), weight, extra
  - `pub enum NodeKind { File, Class, Function, Type, Test, Module, Import }` — `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]`
  - `pub enum EdgeKind { Calls, ImportsFrom, Imports, Inherits, Implements, Contains, TestedBy, DependsOn, References, SemanticallySimilar }` — `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]`
  - `pub enum ConfidenceTier { Extracted, Inferred, Ambiguous }` — `#[derive(Debug, Clone, Serialize, Deserialize)]`
  - `#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)] pub struct ExtractionResult { pub nodes: Vec<NodeInfo>, pub edges: Vec<EdgeInfo> }`
  - `pub fn make_id(parts: &[&str]) -> String` — joins parts with `_`, strips non-alphanumeric with regex `[^a-zA-Z0-9]+` → `_`, lowercases, trims leading/trailing `_`
  - `pub fn normalize_id(s: &str) -> String` — same regex normalization for edge endpoint reconciliation
  - `pub fn extract(path: &Path) -> Result<ExtractionResult>` — entry point:
    1. `hash_file(path)?` → `file_hash`
    2. `load_cached(path, &file_hash)` → return early if cache hit
    3. `get_language_config(extension)` → dispatch to language-specific walker
    4. `save_cached(path, &file_hash, &result)`
    5. Return result

- [ ] **Implement Python extractor in `extract.rs`**:
  - `fn extract_python(path: &Path, source: &[u8], config: &LanguageConfig) -> ExtractionResult`
  - Walk `class_definition` → emit NodeKind::Class node; walk `function_definition` → emit NodeKind::Function; detect `def test_` prefix or `class Test` prefix → NodeKind::Test; is_test=true
  - Import handler for `import_statement`: emit EdgeKind::Imports with target = `make_id(&[module_name])`
  - Import handler for `import_from_statement`: resolve relative dots to full path, emit EdgeKind::ImportsFrom
  - Second-pass call graph: walk function bodies for `call_expression` nodes → emit EdgeKind::Calls with ConfidenceTier::Extracted for direct calls, ConfidenceTier::Inferred for indirect/chained calls
  - Emit `contains` edges: class → method, file → top-level function

- [ ] **Implement JavaScript/TypeScript extractor in `extract.rs`**:
  - `fn extract_js_ts(path: &Path, source: &[u8], config: &LanguageConfig) -> ExtractionResult`
  - Handle both `class_declaration` and arrow function patterns in variable declarations
  - Import handler: `import_statement` → resolve relative paths (normalize `.js` → `.ts` for ESM), emit EdgeKind::ImportsFrom; bare imports → EdgeKind::Imports (external)
  - Export detection: nodes from `export_statement` get `is_exported: true` in extra
  - JSX component detection: PascalCase function names with JSX return → NodeKind::Class (component)

- [ ] **Implement Go, Rust, Java, C/C++ extractors in `extract.rs`**:
  - Go: `type_declaration` → Class, `function_declaration` / `method_declaration` → Function; import block → ImportsFrom edges
  - Rust: `struct_item`, `enum_item`, `impl_item` → Class; `fn` items → Function; `use_declaration` → ImportsFrom; `mod` declarations → Module
  - Java: `class_declaration`, `interface_declaration`, `enum_declaration` → Class; `method_declaration` → Function; `import_declaration` → ImportsFrom; `extends_interfaces` → Inherits edge
  - C/C++: `struct_specifier`, `class_specifier` → Class; `function_definition` → Function; `#include` directives → ImportsFrom (resolve to file path)

- [ ] **Implement remaining language extractors** (Ruby, PHP, Kotlin, Swift, Vue, Dart, Bash, Elixir, Zig):
  - Each follows the same pattern: find class/function nodes using language-specific node types from LanguageConfig, emit NodeInfo, walk bodies for call sites, emit EdgeInfo
  - Notebook `.ipynb`: parse JSON cells array, detect language from `metadata.kernelspec.language`, extract per-cell, prefix qualified names with `cell_N::`
  - Vue `.vue`: extract `<script>` block, detect TypeScript from `<script lang="ts">`, delegate to JS/TS extractor
  - Bash `.sh`: `function_definition` → NodeKind::Function; `command_name` calls → EdgeKind::Calls (ConfidenceTier::Inferred)

- [ ] **Implement parallel extraction in `extract.rs`**:
  - `pub fn extract_all(paths: &[PathBuf]) -> Vec<(PathBuf, Result<ExtractionResult>)>` — uses `rayon::par_iter()`:
    ```rust
    paths.par_iter().map(|p| (p.clone(), extract(p))).collect()
    ```
  - Errors are collected alongside successes — caller logs warnings, skips failed files
  - Thread count: uses Rayon global pool, defaults to `num_cpus`

**6.3 Success Criteria:**
> `extract(Path::new("src/main.rs"))` on the autodev Tauri main file returns nodes for all `pub fn` items + edges for all `use` declarations → `extract_all(all_rs_files)` completes in parallel with no panics

---

### Phase 6.4 — Build Layer

**Goal**: Merge `Vec<ExtractionResult>` into a deduplicated graph and write it to SQLite.

- [ ] **Create `src-tauri/src/graph/build.rs`**:
  - `pub fn build(extractions: Vec<ExtractionResult>, store: &GraphStore, agent_author: Option<&str>, task_id: Option<i64>) -> Result<BuildStats>` — entry point for full build:
    1. Collect all nodes and edges from all ExtractionResults
    2. `deduplicate_nodes(&mut nodes)` — remove duplicate IDs, prefer shortest ID, no chunk suffix
    3. `reconcile_edges(&mut edges, &node_ids)` — remap edge endpoints via `normalize_id()`, drop edges where source or target not in node_ids
    4. Batch upsert to SQLite: `store.upsert_node()` per node, `store.upsert_edge()` per edge
    5. Rebuild FTS5 index: `store.rebuild_fts_index()`
    6. Return `BuildStats { nodes_upserted, edges_upserted, files_parsed, elapsed_ms }`
  - `pub fn deduplicate_nodes(nodes: &mut Vec<NodeInfo>)` — groups by `normalize_id(&node.name)`, keeps first occurrence, logs duplicates count
  - `pub fn reconcile_edges(edges: &mut Vec<EdgeInfo>, node_ids: &HashSet<String>)` — for each edge: if source not in node_ids try `normalize_id(source)`, same for target; drop edges that still don't resolve (external/stdlib — not an error)
  - `pub struct BuildStats { pub nodes_upserted: usize, pub edges_upserted: usize, pub files_parsed: usize, pub elapsed_ms: u64 }`

- [ ] **Implement `prune_deleted_files` in `build.rs`**:
  - `pub fn prune_deleted_files(store: &GraphStore, current_files: &HashSet<String>) -> Result<usize>` — lists all distinct `file_path` values in nodes table, finds paths NOT in `current_files`, calls `store.delete_file_nodes()` + `store.delete_file_edges()` for each; returns total nodes pruned

**6.4 Success Criteria:**
> Full build on 50 Rust files: all `pub struct` and `pub fn` appear as nodes in SQLite, all `use` statements appear as edges, `get_stats()` shows non-zero counts, duplicates are merged

---

### Phase 6.5 — Incremental Update Layer

**Goal**: Re-parse only changed files (SHA-256 hash check), update their dependents, stay < 2s on 2,900 files.

- [ ] **Create `src-tauri/src/graph/incremental.rs`**:
  - `pub fn get_git_changed_files(repo_root: &Path, base: &str) -> Result<Vec<PathBuf>>` — runs `git diff --name-only {base}` via `std::process::Command`; validates `base` with regex `^[A-Za-z0-9_.~^/@{}\-]+$` before passing to shell; returns list of changed file paths; returns empty vec on non-git dirs
  - `pub fn find_changed_by_hash(store: &GraphStore, files: &[PathBuf]) -> Result<Vec<PathBuf>>` — for each file: `store.get_file_hash(path)`, compare to `hash_file(path)`, collect files where hash differs
  - `pub fn find_dependents(store: &GraphStore, changed_qualified_names: &[String]) -> Result<Vec<PathBuf>>` — `SELECT DISTINCT file_path FROM edges WHERE target_qualified IN (...)` → returns additional files that import changed nodes
  - `pub fn incremental_update(store: &GraphStore, changed_files: &[PathBuf], agent_author: Option<&str>, task_id: Option<i64>) -> Result<UpdateStats>` — entry point:
    1. For each changed file: `store.delete_file_nodes(path)` + `store.delete_file_edges(path)`
    2. `extract_all(changed_files)` in parallel
    3. `build(new_extractions, store, agent_author, task_id)`
    4. `find_dependents(store, &new_qualified_names)` → re-parse dependents too (one hop only)
    5. `store.rebuild_fts_index()` for changed nodes only (targeted FTS update)
    6. Return `UpdateStats { files_updated, nodes_updated, edges_updated, elapsed_ms }`
  - `pub struct UpdateStats { pub files_updated: usize, pub nodes_updated: usize, pub edges_updated: usize, pub elapsed_ms: u64 }`

- [ ] **Create `src-tauri/src/graph/watcher.rs`** — Tauri file watcher integration:
  - `pub fn start_graph_watcher(app_handle: AppHandle, repo_root: PathBuf, db_path: PathBuf) -> Result<()>` — uses Tauri's built-in `tauri_plugin_fs::watch()` or `notify` crate; debounces events (300ms); on change: calls `incremental_update()`, emits Tauri event `"graph:updated"` with payload `{ repo: String, files_updated: usize, elapsed_ms: u64 }`
  - `pub fn stop_graph_watcher(app_handle: &AppHandle) -> Result<()>` — cancels the watcher handle
  - Debounce logic: collect all events in 300ms window, deduplicate paths, then fire one `incremental_update()` call

**6.5 Success Criteria:**
> Save a `.rs` file → watcher fires within 500ms → `incremental_update()` runs → only that file + its dependents re-parsed → `"graph:updated"` event emitted → `get_stats()` shows updated `updated_at`

---

### Phase 6.6 — Community Detection

**Goal**: Group related nodes into communities using Louvain algorithm (petgraph-based), give each community a meaningful name, compute cohesion scores.

- [ ] **Create `src-tauri/src/graph/cluster.rs`**:
  - `pub struct Community { pub id: i32, pub name: String, pub members: Vec<String>, pub cohesion_score: f32 }`
  - `pub fn build_adjacency(store: &GraphStore) -> Result<petgraph::Graph<String, f32>>` — loads all edges from SQLite, builds undirected `petgraph::Graph` with edge weights from `EDGE_WEIGHTS` map:
    ```rust
    const EDGE_WEIGHTS: &[(&str, f32)] = &[
        ("CALLS", 1.0), ("IMPORTS_FROM", 0.5), ("INHERITS", 0.8),
        ("IMPLEMENTS", 0.7), ("CONTAINS", 0.3), ("TESTED_BY", 0.4),
        ("DEPENDS_ON", 0.6), ("REFERENCES", 0.4),
    ];
    ```
  - `pub fn detect_communities(graph: &petgraph::Graph<String, f32>) -> HashMap<String, i32>` — Louvain modularity using `petgraph`'s community detection or a pure-Rust Louvain crate (`louvain` or `community-detection`); returns `node_qualified_name → community_id` map; seeds with `seed=42` for determinism
  - `pub fn split_oversized(graph: &petgraph::Graph<String, f32>, communities: &HashMap<String, i32>, threshold_fraction: f32) -> HashMap<String, i32>` — any community with > 25% of total nodes gets re-clustered in isolation; recursively splits until no community exceeds threshold (min community size to split: 10)
  - `pub fn cohesion_score(store: &GraphStore, member_qualified_names: &[String]) -> f32` — `actual_intra_edges / (n * (n-1) / 2)` where intra_edges queried from SQLite
  - `pub fn name_community(store: &GraphStore, members: &[String]) -> String` — algorithmic naming:
    1. Find most common directory prefix from `file_path` of members
    2. If one class accounts for > 40% of members → use that class name as suffix
    3. Else: most frequent non-trivial keyword from `name` fields (filter `COMMON_WORDS` set)
    4. Format: `"{prefix}-{keyword}"` e.g. `"api-health"`, `"db-models"`, `"agent-core"`
  - `pub fn run_clustering(store: &GraphStore) -> Result<Vec<Community>>` — orchestrates full clustering:
    1. `build_adjacency(store)`
    2. `detect_communities(&graph)`
    3. `split_oversized(&graph, &communities, 0.25)`
    4. Group nodes by community_id, `name_community()` for each, `cohesion_score()` for each
    5. `store.set_community(qname, community_id, &community_name)` for every node
    6. Upsert into `communities` table
    7. Return sorted by member count desc (community 0 = largest)

**6.6 Success Criteria:**
> After full build on autodev repo: `list_communities` returns 5–15 named communities; community names like `"graph-store"`, `"agent-context"`, `"recipes-runner"` — not `"Community 0"`; cohesion scores stored in `communities` table

---

### Phase 6.7 — Analysis Engine

**Goal**: God nodes, surprising connections, suggested questions, impact radius, graph diff — the insight layer that makes the graph useful beyond navigation.

- [ ] **Create `src-tauri/src/graph/analyze.rs`**:

  - **God Nodes**:
    - `pub fn god_nodes(store: &GraphStore, top_n: usize) -> Result<Vec<GodNode>>` — `SELECT qualified_name, COUNT(*) as degree FROM (SELECT source_qualified as qualified_name FROM edges UNION ALL SELECT target_qualified FROM edges) GROUP BY qualified_name ORDER BY degree DESC LIMIT ?`
    - Filter out file-hub nodes (label == filename), concept nodes (no source_file), AST stubs (name starts with `.` and ends with `()`)
    - `pub struct GodNode { pub qualified_name: String, pub label: String, pub degree: u32, pub community: Option<i32>, pub community_name: Option<String>, pub file_path: String }`

  - **Surprising Connections**:
    - `pub fn surprising_connections(store: &GraphStore, top_n: usize) -> Result<Vec<SurprisingConnection>>` — queries cross-file edges (source_qualified file ≠ target_qualified file); excludes IMPORTS, IMPORTS_FROM, CONTAINS, METHOD edges; computes composite surprise score per edge:
      ```
      score = confidence_bonus + cross_type_bonus + cross_repo_bonus + cross_community_bonus + peripheral_hub_bonus
      confidence_bonus: AMBIGUOUS=3, INFERRED=2, EXTRACTED=1
      cross_type_bonus: +2 if file extensions indicate different categories (code vs notebook vs script)
      cross_repo_bonus: +2 if top-level directory differs
      cross_community_bonus: +1 if community(source) ≠ community(target)
      peripheral_hub_bonus: +1 if min(degree(u),degree(v)) ≤ 2 AND max ≥ 5
      ```
    - Returns top_n sorted by score desc
    - `pub struct SurprisingConnection { pub source: String, pub target: String, pub source_file: String, pub target_file: String, pub confidence_tier: String, pub relation: String, pub score: f32, pub why: Vec<String> }`

  - **Suggested Questions**:
    - `pub fn suggest_questions(store: &GraphStore, top_n: usize) -> Result<Vec<SuggestedQuestion>>` — generates questions from 5 signal types:
      1. AMBIGUOUS edges: `"What is the exact relationship between '{source}' and '{target}'?"` — query `SELECT * FROM edges WHERE confidence_tier='AMBIGUOUS' LIMIT 10`
      2. Bridge nodes (high betweenness): BFS-based betweenness approximation on in-memory petgraph; `"Why does '{node}' connect '{community_a}' to '{community_b}'?"`
      3. God nodes with INFERRED edges: `"Are the {N} inferred relationships involving '{node}' actually correct?"`
      4. Isolated nodes (degree ≤ 1): `"What connects {nodes} to the rest of the system?"`
      5. Low-cohesion communities (cohesion < 0.15): `"Should '{community}' be split into smaller modules?"`
    - `pub struct SuggestedQuestion { pub question_type: String, pub question: String, pub why: String }`

  - **Impact Radius**:
    - `pub fn get_impact_radius(store: &GraphStore, qualified_name: &str, max_depth: u32, max_nodes: usize) -> Result<ImpactRadius>` — backward BFS:
      1. Start with {qualified_name}
      2. Each step: `SELECT source_qualified FROM edges WHERE target_qualified = ? AND kind IN ('CALLS','IMPORTS_FROM','DEPENDS_ON')` — find callers/dependents
      3. Also collect: `SELECT target_qualified FROM edges WHERE source_qualified = ? AND kind = 'TESTED_BY'` — find tests
      4. Classify by hop count: hop ≤ 2 = high_risk, hop 3-4 = medium_risk, hop 5+ = low_risk
      5. Identify gaps: affected nodes with no TESTED_BY edges at all
    - `pub struct ImpactRadius { pub changed_node: String, pub affected_nodes: Vec<AffectedNode>, pub tests_to_run: Vec<String>, pub test_coverage_gaps: Vec<String>, pub total_affected: usize, pub high_risk_count: usize }`
    - `pub struct AffectedNode { pub qualified_name: String, pub label: String, pub file_path: String, pub hops: u32, pub risk_level: String, pub path: Vec<String> }`

  - **Graph Diff**:
    - `pub fn save_snapshot(store: &GraphStore, label: &str) -> Result<i64>` — reads all nodes + edges, serializes to compressed JSON, writes to `snapshots` table; returns snapshot_id
    - `pub fn graph_diff(store: &GraphStore, snapshot_id_a: i64, snapshot_id_b: i64) -> Result<GraphDiff>` — loads both snapshots, computes:
      - `new_nodes`: node IDs in B but not A
      - `removed_nodes`: node IDs in A but not B
      - `new_edges`: (source, target, kind) tuples in B but not A
      - `removed_edges`: (source, target, kind) tuples in A but not B
      - `summary`: human-readable string `"N new nodes, M new edges, P nodes removed"`
    - `pub struct GraphDiff { pub new_nodes: Vec<NodeSummary>, pub removed_nodes: Vec<NodeSummary>, pub new_edges: Vec<EdgeSummary>, pub removed_edges: Vec<EdgeSummary>, pub summary: String }`

**6.7 Success Criteria:**
> `god_nodes(store, 10)` returns top-10 most-connected real entities (not filenames); `surprising_connections(store, 5)` returns cross-module edges with `why` explanations; `get_impact_radius(store, "src/api/health.rs::agents_health", 5, 100)` returns callers + tests within 5 hops

---

### Phase 6.8 — Execution Flow Detection

**Goal**: Identify entry points (API handlers, CLI commands, test functions, async tasks), trace their execution paths forward through CALLS edges, and score criticality.

- [ ] **Create `src-tauri/src/graph/flows.rs`**:
  - `pub const FRAMEWORK_DECORATOR_PATTERNS: &[&str]` — regex patterns for 15+ frameworks: FastAPI (`app.get`, `app.post`, `router.get`), Click (`@click.command`), Celery (`@task`, `@shared_task`), Django (`@receiver`, `@api_view`), Spring (`@GetMapping`, `@PostMapping`, `@Scheduled`), Express (`app.get`, `router.post`), pydantic-ai/LangChain (`@tool`, `@tool_plain`, `@system_prompt`)
  - `pub const ENTRY_NAME_PATTERNS: &[&str]` — regex patterns: `^main$`, `^__main__$`, `^test_`, `^Test[A-Z]`, `^on_`, `^handle_`, `^handler$`, `^lambda_handler$`, `^upgrade$`, `^downgrade$`, `^lifespan$`, `^get_db$`
  - `pub const SECURITY_KEYWORDS: &[&str]` — `["auth", "token", "password", "secret", "crypto", "sign", "verify", "admin", "permission", "role", "session", "cookie", "jwt", "oauth"]`
  - `pub fn detect_entry_points(store: &GraphStore) -> Result<Vec<String>>` — finds functions with:
    - No incoming CALLS edges (nothing calls them externally) — `SELECT qualified_name FROM nodes WHERE kind='Function' AND qualified_name NOT IN (SELECT target_qualified FROM edges WHERE kind='CALLS')`
    - OR `extra` JSON contains a decorator matching any FRAMEWORK_DECORATOR_PATTERNS
    - OR name matches any ENTRY_NAME_PATTERNS
    - Returns `Vec<String>` of qualified_names of entry points
  - `pub fn trace_flow(store: &GraphStore, entry_point: &str, max_depth: u32) -> Result<Vec<String>>` — forward BFS through CALLS edges from entry_point: `SELECT target_qualified FROM edges WHERE source_qualified = ? AND kind = 'CALLS'`; collects path as ordered Vec of qualified_names; max_depth=20 to prevent cycles
  - `pub fn score_criticality(store: &GraphStore, flow_path: &[String]) -> Result<f32>` — composite score:
    ```
    base = 1.0
    × 1.5 if any node name or params contains a SECURITY_KEYWORD
    × 0.7 if flow has at least one TESTED_BY edge on any node in path
    × (1.0 + (path_length as f32).ln() / 10.0)
    ```
  - `pub fn run_flow_detection(store: &GraphStore) -> Result<Vec<FlowResult>>` — orchestrates:
    1. `detect_entry_points(store)`
    2. For each: `trace_flow(store, entry, 20)`
    3. `score_criticality(store, &path)`
    4. Check is_tested: any TESTED_BY edge on any path node
    5. Check has_security_keyword: any path node name in SECURITY_KEYWORDS
    6. Upsert into `flows` table
  - `pub struct FlowResult { pub entry_point: String, pub flow_path: Vec<String>, pub criticality_score: f32, pub is_tested: bool, pub has_security_keyword: bool }`

**6.8 Success Criteria:**
> On autodev Tauri codebase: `detect_entry_points()` finds Tauri command functions; `trace_flow("main")` returns the startup sequence; `score_criticality()` gives agent-running flows higher scores than utility functions

---

### Phase 6.9 — Change Analysis

**Goal**: Parse git diffs to identify changed functions, then score their risk using the graph (blast radius + security + test coverage).

- [ ] **Create `src-tauri/src/graph/changes.rs`**:
  - `pub fn parse_git_diff_ranges(repo_root: &Path, base: &str) -> Result<HashMap<String, Vec<(u32, u32)>>>` — runs `git diff --unified=0 {base} --` (base validated with `^[A-Za-z0-9_.~^/@{}\-]+$` regex before use); parses `@@ -start,count +start,count @@` hunk headers; returns `file_path → [(start_line, end_line)]` for changed line ranges
  - `pub fn find_changed_nodes(store: &GraphStore, changed_ranges: &HashMap<String, Vec<(u32, u32)>>) -> Result<Vec<ChangedNode>>` — for each (file, ranges): query `SELECT qualified_name, name, line_start, line_end FROM nodes WHERE file_path = ?`; check if node's `line_start..=line_end` overlaps any changed range; collect matching nodes
  - `pub fn score_change_risk(store: &GraphStore, changed_node: &GraphNode) -> Result<RiskScore>` — composite score:
    - `caller_count`: `SELECT COUNT(*) FROM edges WHERE target_qualified = ? AND kind = 'CALLS'`
    - `is_tested`: `SELECT COUNT(*) FROM edges WHERE source_qualified = ? AND kind = 'TESTED_BY' LIMIT 1`
    - `is_security_critical`: node name or flow membership contains SECURITY_KEYWORD
    - `is_entry_point`: exists in `flows` table as entry_point
    - Final score: `(caller_count * 0.4) + (if !is_tested { 3.0 } else { 0.0 }) + (if is_security_critical { 2.0 } else { 0.0 }) + (if is_entry_point { 1.0 } else { 0.0 })`
    - Risk tier: score > 5.0 = "high", > 2.5 = "medium", else "low"
  - `pub fn detect_changes(store: &GraphStore, repo_root: &Path, base: &str) -> Result<ChangeAnalysis>` — full pipeline:
    1. `parse_git_diff_ranges(repo_root, base)`
    2. `find_changed_nodes(store, &ranges)`
    3. `score_change_risk(store, &node)` for each changed node
    4. `get_impact_radius(store, &node.qualified_name, 3, 50)` for high-risk nodes
    5. Collect all tests to run (from TESTED_BY edges + impact radius tests)
    6. Return `ChangeAnalysis` sorted by risk_score desc
  - `pub struct ChangeAnalysis { pub changed_nodes: Vec<ScoredChange>, pub tests_to_run: Vec<String>, pub test_coverage_gaps: Vec<String>, pub high_risk_count: usize, pub summary: String }`
  - `pub struct ScoredChange { pub node: GraphNode, pub risk_score: f32, pub risk_tier: String, pub caller_count: u32, pub is_tested: bool, pub is_security_critical: bool, pub impact_radius: Option<ImpactRadius> }`

**6.9 Success Criteria:**
> `detect_changes(store, repo_root, "HEAD~1")` after a commit that modifies an API handler returns: the handler as high-risk, its callers in impact radius, the test functions that should be run, gaps where no test covers changed code

---

### Phase 6.10 — Report Generation

**Goal**: GRAPH_REPORT.md — the narrative markdown document that PA and workers read to understand a codebase without browsing files.

- [ ] **Create `src-tauri/src/graph/report.rs`**:
  - `pub fn generate_report(store: &GraphStore, god_nodes: &[GodNode], communities: &[Community], surprising: &[SurprisingConnection], questions: &[SuggestedQuestion], flows: &[FlowResult]) -> Result<String>` — assembles GRAPH_REPORT.md:
    ```markdown
    # GRAPH_REPORT.md — {repo_name}
    *Generated: {timestamp} | Nodes: {N} | Edges: {E} | Languages: {langs}*

    ## God Nodes (Core Abstractions)
    The most-connected entities in this codebase — changing any of these has the highest blast radius.
    | Node | Degree | Community | File |
    ...

    ## Community Structure
    {N} communities detected. Cohesion scores indicate internal coupling strength.
    ### Community {id}: {name} ({member_count} nodes, cohesion: {score})
    Key members: {top 5 by degree}
    ...

    ## Surprising Connections
    Non-obvious relationships the graph found — investigate these first.
    ...

    ## Suggested Questions
    Knowledge gaps and areas worth exploring based on graph signals.
    ...

    ## Critical Execution Flows ({N} detected)
    ...

    ## Knowledge Gaps
    {isolated_nodes_count} isolated nodes — potential documentation gaps or missing edges.
    ...
    ```
  - `pub fn write_report(store: &GraphStore, output_path: &Path) -> Result<()>` — orchestrates all analysis passes, calls `generate_report()`, writes `GRAPH_REPORT.md` to output_path
  - Report regenerated automatically at end of full build and on-demand via Tauri command

- [ ] **Implement snapshot operations in `report.rs`**:
  - `pub fn save_snapshot(store: &GraphStore, label: &str) -> Result<i64>` — serializes current graph state (all nodes + edges) to compressed JSON blob in `snapshots` table; returns snapshot_id
  - `pub fn load_snapshot(store: &GraphStore, snapshot_id: i64) -> Result<ExtractionResult>` — deserializes snapshot JSON back to nodes + edges

**6.10 Success Criteria:**
> `write_report(store, Path::new("graph-out/GRAPH_REPORT.md"))` produces a readable markdown file with god nodes table, community sections with cohesion scores, at least 3 suggested questions, and a summary line count

---

### Phase 6.11 — Export Formats

**Goal**: Export graph data to `graph.json` (MCP interchange), interactive HTML (`graph.html`), and static SVG.

- [ ] **Create `src-tauri/src/graph/export.rs`**:

  - **JSON export**:
    - `pub fn export_json(store: &GraphStore, output_path: &Path) -> Result<()>` — serializes graph to `graph.json`:
      ```json
      {
        "nodes": [{ "id": "...", "label": "...", "source_file": "...", "source_location": "L42", "kind": "Function", "community": 3, "community_name": "api-health" }],
        "edges": [{ "source": "...", "target": "...", "kind": "CALLS", "confidence_tier": "EXTRACTED", "weight": 1.0 }]
      }
      ```
    - Used as interchange format for `merge-graphs` cross-repo merge and MCP server graph queries

  - **HTML export** (D3.js force-directed, self-contained, dark theme):
    - `pub fn export_html(store: &GraphStore, communities: &[Community], output_path: &Path) -> Result<()>` — generates a single self-contained HTML file with:
      - Inline CSS: dark theme (`#0A0A0A` background matching AutoDev design tokens)
      - Node colors: 10 community palette colors cycling (`#4E79A7`, `#F28E2B`, `#E15759`, `#76B7B2`, `#59A14F`, `#EDC948`, `#B07AA1`, `#FF9DA7`, `#9C755F`, `#BAB0AC`)
      - Edge colors: CALLS = `#00FF88` (AIRS green), IMPORTS_FROM = `#4E79A7`, INHERITS = `#B07AA1`, TESTED_BY = `#59A14F`, CONTAINS = `#444444`, SEMANTICALLY_SIMILAR = `#EDC948`
      - D3.js from CDN with SRI hash (XSS defense)
      - All user-provided labels HTML-escaped via `fn escape_html(s: &str) -> String` (escapes `<`, `>`, `&`, `"`, `'`, backtick)
      - Sidebar with: search input, node info panel (name/kind/file/line/community/degree), neighbor list, legend with community names + colors, stats bar
      - Rendering modes: `full` (≤ 500 nodes), `community` aggregate (> 500 nodes), `auto` (switches automatically)
      - Cap: embed max 5,000 nodes; above that, switch to community-aggregate mode automatically
    - `const D3_SRI: &str` — SHA-384 hash of specific D3.js CDN version, verified at build time

  - **SVG export**:
    - `pub fn export_svg(store: &GraphStore, communities: &[Community], output_path: &Path) -> Result<()>` — static SVG using simple force-layout approximation (Fruchterman-Reingold via petgraph layout or simple circular layout per community); cap at 5,000 nodes; uses same color palette as HTML

**6.11 Success Criteria:**
> `export_json(store, "graph-out/graph.json")` produces valid JSON parseable by `serde_json::from_str`; `export_html(store, &communities, "graph-out/graph.html")` opens in browser showing force-directed graph with colored nodes; SVG renders as valid file

---

### Phase 6.12 — MCP Server (Graph Tools)

**Goal**: Register all graph tools as builtin tools on the existing `autodev-mcp-server` (port 8767). Workers and PA discover them via `tools/list` — zero new sidecar needed.

> **Pattern**: All graph tools follow the same pattern as `CreateTaskTool` / `ListTasksTool` — implement the `Tool` trait in `src-tauri/src/agent/builtin_tools/`, register in `ToolRegistry`, automatically exposed via MCP.

- [ ] **Create `src-tauri/src/agent/builtin_tools/graph_tools.rs`** — all graph MCP tools:

  - **Discovery tools** (5):
    - `GetMinimalContextTool` — `name: "get_graph_minimal_context"`, input: `{ task: String }` — scores graph nodes against task terms using `_score_nodes()`, returns top-10 nodes + 3 relevant communities + `next_tool_suggestions` JSON; targets ~100 token output
    - `GetArchitectureOverviewTool` — `name: "get_graph_architecture_overview"` — returns all communities with member_count + cohesion_score + top 3 nodes each + cross-community coupling matrix from edges
    - `GetGraphStatsTool` — `name: "get_graph_stats"` — wraps `store.get_stats()`, returns node/edge counts by kind + language list + last_updated timestamp
    - `ListCommunitiesTool` — `name: "list_graph_communities"` — returns all communities from `communities` table ordered by member_count desc
    - `GetGraphReportTool` — `name: "get_graph_report"` — reads `GRAPH_REPORT.md` from disk and returns its content as string

  - **Navigation tools** (9):
    - `QueryGraphTool` — `name: "query_graph"`, input: `{ target: String, pattern: String }` where pattern is one of `callers_of|callees_of|imports_of|tests_for|dependencies_of`; executes appropriate SQL query against edges table
    - `SemanticSearchTool` — `name: "semantic_search_nodes"`, input: `{ query: String, top_k: u32 }` — FTS5 query against `nodes_fts` table: `SELECT * FROM nodes_fts WHERE nodes_fts MATCH ? ORDER BY rank LIMIT ?`; returns matching nodes with snippet
    - `GetNodeDetailsTool` — `name: "get_node_details"`, input: `{ qualified_name: String }` — `store.get_node(qname)` + `store.get_edges_for_node(qname)`, groups edges by kind
    - `FindPathTool` — `name: "find_graph_path"`, input: `{ source: String, target: String }` — loads adjacency from SQLite into petgraph, runs `petgraph::algo::astar()` for shortest weighted path; returns path as array of node labels with edge relations between each hop
    - `ExplainNodeTool` — `name: "explain_node"`, input: `{ label: String }` — fuzzy-finds node by label (normalize + substring match), returns: name/kind/file/line/community, all neighbors sorted by degree desc (top 20), formatted as human-readable text
    - `GetFileSummaryTool` — `name: "get_file_summary"`, input: `{ file_path: String }` — `store.list_nodes_by_file(path)`, groups by kind (Classes: N, Functions: N, Tests: N), lists all qualified_names
    - `ListCallersTool` — `name: "list_callers"`, input: `{ qualified_name: String }` — `SELECT source_qualified FROM edges WHERE target_qualified = ? AND kind = 'CALLS'`
    - `ListCalleesTool` — `name: "list_callees"`, input: `{ qualified_name: String }` — `SELECT target_qualified FROM edges WHERE source_qualified = ? AND kind = 'CALLS'`
    - `ListTestsTool` — `name: "list_tests"`, input: `{ qualified_name: String }` — `SELECT target_qualified FROM edges WHERE source_qualified = ? AND kind = 'TESTED_BY'`

  - **Impact tools** (5):
    - `GetImpactRadiusTool` — `name: "get_impact_radius"`, input: `{ qualified_name: String, max_depth: Option<u32> }` — calls `analyze::get_impact_radius()`
    - `DetectChangesTool` — `name: "detect_graph_changes"`, input: `{ base: Option<String> }` — calls `changes::detect_changes(store, repo_root, base.unwrap_or("HEAD~1"))`
    - `GetReviewContextTool` — `name: "get_review_context"`, input: `{ files: Vec<String> }` — reads source snippets for changed functions only (lines from `line_start` to `line_end`); returns minimal source + graph context; targets ≤ 800 tokens total
    - `GetAffectedFlowsTool` — `name: "get_affected_flows"`, input: `{ file_path: String }` — finds which flows from `flows` table include nodes from this file
    - `GetFlowCriticalityTool` — `name: "get_flow_criticality"` — returns all flows from `flows` table sorted by criticality_score desc

  - **Analysis tools** (4):
    - `GetGodNodesTool` — `name: "get_god_nodes"`, input: `{ top_n: Option<u32> }` — calls `analyze::god_nodes()`
    - `GetSurprisingConnectionsTool` — `name: "get_surprising_connections"`, input: `{ top_n: Option<u32> }` — calls `analyze::surprising_connections()`
    - `GetSuggestedQuestionsTool` — `name: "get_suggested_questions"` — calls `analyze::suggest_questions()`
    - `GetGraphDiffTool` — `name: "get_graph_diff"`, input: `{ snapshot_id_a: i64, snapshot_id_b: i64 }` — calls `analyze::graph_diff()`

  - **AutoDev-native tools** (4):
    - `GetWorkerGraphTool` — `name: "get_worker_graph"`, input: `{ worker_name: String }` — `SELECT * FROM nodes WHERE agent_author = ?`; returns nodes + edges authored by that worker
    - `GetTaskGraphTool` — `name: "get_task_graph"`, input: `{ task_id: i64 }` — `SELECT * FROM nodes WHERE task_id = ?`; returns all nodes + their edges touched during task
    - `GetLockedNodesTool` — `name: "get_locked_nodes"` — `SELECT * FROM nodes WHERE worker_locked = 1`; returns locked nodes with their `worktree` attribute
    - `BuildGraphTool` — `name: "build_graph"`, input: `{ repo_path: String }` — triggers full graph rebuild (detect → extract → build → cluster → analyze → report); returns `BuildStats` JSON

- [ ] **Register all graph tools in `src-tauri/src/agent/builtin_tools/mod.rs`**:
  - Add `pub mod graph_tools;`
  - In `ToolRegistry::new()`: register all tools from `graph_tools.rs` alongside existing tools
  - Graph tools only registered when `graph_enabled: true` in app config (checked at startup)

- [ ] **Add 5 MCP prompts** in a new `src-tauri/src/agent/builtin_tools/graph_prompts.rs`:
  - `review_changes_prompt` — step-by-step code review workflow using `detect_changes` → `get_impact_radius` → `get_review_context` → `list_tests`
  - `architecture_map_prompt` — understand codebase: `get_architecture_overview` → `list_communities` → `get_god_nodes` → read key files
  - `debug_issue_prompt` — trace bug: `semantic_search_nodes` → `query_graph(callers_of)` → `get_affected_flows`
  - `onboard_developer_prompt` — guided codebase tour: `get_graph_report` → `list_communities` → `explain_node` for each god node
  - `pre_merge_check_prompt` — pre-merge risk: `detect_changes` → `get_impact_radius` → `get_flow_criticality` → `list_tests`
  - Each prompt is a `GetPromptResult` that returns a structured `messages` array with role=user and the step-by-step instructions

**6.12 Success Criteria:**
> Worker agent recipe receives `tools/list` from `autodev-mcp-server` → sees `get_graph_minimal_context` + all 22 graph tools → calls `get_graph_minimal_context(task="implement health endpoint")` → receives relevant nodes with < 150 tokens → uses `list_callers` to find callers of affected function — all without reading a single source file

---

### Phase 6.13 — Query Engine

**Goal**: BFS/DFS traversal with token budget enforcement, shortest weighted path, fuzzy node matching for the CLI and MCP tools.

- [ ] **Create `src-tauri/src/graph/query.rs`**:
  - `pub fn score_nodes(store: &GraphStore, query_terms: &[&str]) -> Result<Vec<(f32, String)>>` — diacritic-insensitive matching against node `name` (1.0 score per term match) and `file_path` (0.5 score per term match); uses Unicode normalization for diacritic stripping; returns `Vec<(score, qualified_name)>` sorted desc
  - `pub fn bfs_subgraph(store: &GraphStore, start_nodes: &[String], depth: u32) -> Result<(HashSet<String>, Vec<(String, String)>)>` — breadth-first from start_nodes outward `depth` hops using adjacency from SQLite; returns `(visited_node_ids, edges_seen)` as `Vec<(source_qname, target_qname)>`
  - `pub fn dfs_subgraph(store: &GraphStore, start_nodes: &[String], depth: u32) -> Result<(HashSet<String>, Vec<(String, String)>)>` — depth-first variant; same return type as BFS
  - `pub fn subgraph_to_text(store: &GraphStore, nodes: &HashSet<String>, edges: &[(String, String)], token_budget: usize) -> Result<String>` — formats subgraph as text (approx 3 chars/token budget):
    ```
    NODE {label} [src={file} loc=L{line} community={name}]
    EDGE {src_label} --{kind} [{confidence_tier}]--> {tgt_label}
    ```
    Nodes sorted by degree desc. Truncates at `token_budget * 3` chars with `"... (truncated to ~{budget} token budget)"` suffix
  - `pub fn find_shortest_path(store: &GraphStore, source_label: &str, target_label: &str) -> Result<Option<Vec<PathHop>>>` — fuzzy-finds source/target by label using `score_nodes()`, then Dijkstra on petgraph adjacency weighted by edge.weight; returns hops as `Vec<PathHop>`:
    - `pub struct PathHop { pub node_label: String, pub edge_relation: Option<String>, pub confidence_tier: Option<String> }` — first element has no edge (entry), last has no edge (destination)
  - `pub fn find_node_by_label(store: &GraphStore, label: &str) -> Result<Vec<String>>` — case-insensitive substring match against `name` column: `SELECT qualified_name FROM nodes WHERE lower(name) LIKE '%' || lower(?) || '%'`; falls back to `qualified_name` LIKE match if empty

- [ ] **Add `airsctl graph` CLI subcommand** in `src-cli/src/main.rs`:
  - `airsctl graph query "<question>" [--budget N] [--dfs]` — calls `score_nodes` → `bfs_subgraph` → `subgraph_to_text`, prints to stdout
  - `airsctl graph path "<source>" "<target>"` — calls `find_shortest_path`, prints hop chain
  - `airsctl graph explain "<label>"` — calls `find_node_by_label` + `get_edges_for_node`, prints node details + neighbors
  - `airsctl graph build [--repo <path>]` — triggers full build, prints `BuildStats`
  - `airsctl graph update [--repo <path>]` — triggers incremental update, prints `UpdateStats`
  - `airsctl graph report [--repo <path>]` — regenerates `GRAPH_REPORT.md`
  - `airsctl graph diff <snapshot_a_id> <snapshot_b_id>` — prints graph diff summary

**6.13 Success Criteria:**
> `airsctl graph query "task board"` returns ≤ 2000 token output with relevant task-related nodes; `airsctl graph path "build_graph" "GraphStore"` prints a 3-hop chain; `airsctl graph explain "GraphStore"` prints degree + top 20 neighbors

---

### Phase 6.14 — Graph Tab UI

**Goal**: New fifth tab (`Graph`) in the AutoDev UI — D3.js force-directed graph with community colors, click-to-IDE, filtering, and the GRAPH_REPORT panel.

> **New Tauri commands** (add to `src-tauri/src/commands/graph_commands.rs`):
> - `build_graph(repo_path: String)` → `BuildStats`
> - `update_graph(repo_path: String)` → `UpdateStats`
> - `get_graph_stats(repo_path: String)` → `GraphStats`
> - `export_graph(repo_path: String, format: String, output_path: String)` → `()`
> - `get_graph_report(repo_path: String)` → `String`
> - `get_graph_communities(repo_path: String)` → `Vec<Community>`
> - `get_graph_nodes_for_viz(repo_path: String, max_nodes: u32)` → `Vec<GraphNode>` (capped for D3)
> - `get_graph_edges_for_viz(repo_path: String)` → `Vec<GraphEdge>`
> All registered in `src-tauri/src/lib.rs` alongside existing Tauri commands.

- [ ] **Create `src/tabs/Graph.tsx`** — tab scaffold:
  - Repo selector dropdown (reuses `useWorkspaceStore().currentRepo`)
  - Toolbar: `[Build Graph]` button → calls `build_graph()` → shows spinner + BuildStats toast; `[Update]` → calls `update_graph()`; `[Export ▼]` dropdown with `JSON`, `HTML`, `SVG` options
  - Render mode selector: `[Auto ▼]` / `Full` / `Community` / `File`
  - Community filter: multi-select dropdown of community names (populated from `get_graph_communities()`)
  - Edge type filter: checkboxes for CALLS, IMPORTS_FROM, INHERITS, TESTED_BY, CONTAINS, other
  - Stats bar: `🟢 {N} nodes | {E} edges | {langs} | Last updated: {timestamp}`
  - Listens for Tauri event `"graph:updated"` → auto-refreshes stats + D3 canvas
  - Layout: full-height, D3 canvas takes 75% width, NodeInfoPanel takes 25% right sidebar

- [ ] **Create `src/components/graph/GraphCanvas.tsx`** — D3.js force-directed graph:
  - Accepts `nodes: GraphNode[]`, `edges: GraphEdge[]`, `communities: Community[]`, `renderMode: 'full'|'community'|'file'|'auto'`, `edgeFilter: EdgeKind[]`
  - D3.js `forceSimulation` with `forceManyBody`, `forceLink`, `forceCenter`
  - Node radius: `4 + Math.log(degree + 1) * 4` (min 4px, max 20px)
  - Node fill: community color from palette (10 colors cycling)
  - Node stroke by kind: `Function` → 1px `#444`, `Class` → 2px community color, `Test` → 1px dashed `#888`, `File` → no stroke
  - Edge stroke by kind: CALLS=`#00FF88` (opacity 0.6), IMPORTS_FROM=`#4E79A7` (opacity 0.5), INHERITS=`#B07AA1` (opacity 0.6), TESTED_BY=`#59A14F` (opacity 0.5), CONTAINS=`#444` (opacity 0.3)
  - Edge opacity by confidence_tier: EXTRACTED=0.7, INFERRED=0.4, AMBIGUOUS=0.2
  - Edge thickness by weight: `0.5 + weight * 2`
  - Zoom/pan via `d3.zoom()` attached to SVG container
  - Drag nodes via `d3.drag()` — dragging freezes node position (`fx`, `fy`)
  - Click node → fires `onNodeSelect(node: GraphNode)` callback → updates parent state → NodeInfoPanel shows details
  - Double-click node → zoom simulation to center on that node
  - Auto-mode: switch to community-aggregate view when `nodes.length > 500`, full view when ≤ 500
  - Community-aggregate mode: one circle per community (sized by member count), clicking expands that community in a separate zoom level

- [ ] **Create `src/components/graph/GraphNodePanel.tsx`** — right sidebar node info:
  - Shows when a node is selected: name (monospace), kind badge (color-coded), file path (clickable), line range, community name, degree count
  - Callers section: list of caller node labels (from `list_callers` Tauri command, lazy-loaded on selection)
  - Callees section: list of callee node labels (from `list_callees`, lazy-loaded)
  - Tests section: list of test names (from `list_tests`, lazy-loaded)
  - `[Open in IDE Tab]` button — fires `openInIde(node.file_path, node.line_start)`:
    ```typescript
    invoke('open_in_ide', { file: node.file_path, line: node.line_start })
    // Tauri command switches active tab to IDE, navigates to file:line
    ```

- [ ] **Add `open_in_ide` Tauri command** in `src-tauri/src/commands/graph_commands.rs`:
  - `pub async fn open_in_ide(app: AppHandle, file: String, line: u32) -> Result<()>` — emits Tauri event `"ide:navigate"` with `{ file, line }`; `Graph.tsx` and `IDE.tsx` listen: IDE tab becomes active, file tree selects the file, code viewer scrolls to line

- [ ] **Create `src/components/graph/GraphReport.tsx`** — collapsible GRAPH_REPORT panel:
  - Reads `GRAPH_REPORT.md` via `get_graph_report()` Tauri command
  - Renders markdown (using existing markdown renderer or `react-markdown`)
  - Collapsible accordion: `[God Nodes]` section, `[Community Structure]` section, `[Surprising Connections]` section, `[Suggested Questions]` section
  - `[Regenerate Report]` button → calls `build_graph()` → report refreshes

- [ ] **Create `src/stores/graphStore.ts`** — Zustand store for graph state:
  - State: `currentRepo: string`, `isBuilding: boolean`, `stats: GraphStats | null`, `communities: Community[]`, `selectedNode: GraphNode | null`, `renderMode: RenderMode`, `edgeFilter: EdgeKind[]`, `lastUpdated: Date | null`
  - Actions: `buildGraph(repo)`, `updateGraph(repo)`, `loadStats(repo)`, `loadCommunities(repo)`, `selectNode(node)`, `setRenderMode(mode)`, `toggleEdgeFilter(kind)`
  - Listens for `"graph:updated"` Tauri event → calls `loadStats()` automatically

- [ ] **Register Graph tab** in `src/App.tsx`:
  - Add `{ id: 'graph', label: 'Graph', icon: <NetworkIcon /> }` to tab array (after IDE, before Recipes)
  - Import `Graph` from `./tabs/Graph`
  - Add to tab content switch

**6.14 Success Criteria:**
> New `[Graph]` tab appears in navigation → `[Build Graph]` button triggers build with progress indication → graph appears as D3 force-directed visualization with colored nodes → click a node → NodeInfoPanel shows callers, callees, tests → `[Open in IDE Tab]` navigates IDE tab to correct file:line → `[Export ▼]` → HTML opens in browser

---

### Phase 6.15 — Worker Integration

**Goal**: Workers automatically get graph context before starting tasks; every file write annotates nodes with `agent_author` + `task_id`; task completion triggers snapshot + diff.

- [ ] **Add worker attribution to `store.rs`**:
  - `pub fn set_worker_context(store: &mut GraphStore, agent_author: &str, task_id: i64) -> ()` — stores in `GraphStore` struct so all subsequent `upsert_node()` / `upsert_edge()` calls automatically include `agent_author` and `task_id`
  - `pub fn clear_worker_context(store: &mut GraphStore) -> ()` — clears context after task completes

- [ ] **Wire incremental update into worker lifecycle** in `src-tauri/src/recipes/runner.rs`:
  - After recipe agent writes a file (detected via PostToolUse hook event on `write_file`): call `incremental_update(store, &[changed_file], Some(agent_author), Some(task_id))`
  - Before recipe start: `save_snapshot(store, &format!("before_task_{task_id}"))` — saves pre-task state
  - After recipe completes (success or failure): `save_snapshot(store, &format!("after_task_{task_id}"))` → compute `graph_diff(before_id, after_id)` → store result in `tasks.db` `extra` JSON field on the task record
  - Emit Tauri event `"graph:updated"` so Graph tab refreshes automatically

- [ ] **PA context injection** in `src-tauri/src/agent/context.rs`:
  - Add `build_graph_context(query: &str, repo_path: &str) -> String` function:
    1. Open `GraphStore` for current repo (cached connection)
    2. `score_nodes(store, &query.split_whitespace().collect::<Vec<_>>())` → top-5 scored nodes
    3. `bfs_subgraph(store, &top5_qnames, 2)` → 2-hop neighborhood
    4. `subgraph_to_text(store, &visited, &edges, 800)` → max 800 tokens of graph context
    5. Return formatted string with header `"## Graph Context (relevant to your query)\n"`
  - Modify `build_system_prompt_with_mode()`: when `graph.db` exists for current repo AND query is non-empty, append `build_graph_context(user_message, repo_path)` to system prompt (in Fast Development and Review modes; omit in Delegate mode)
  - Graph context omitted when graph DB does not exist (feature gracefully absent until first build)

- [ ] **QC agent graph usage** — update `qc-review-cycle.yaml` recipe instructions to include:
  ```yaml
  instructions: |
    You are the AIRS QC Agent.
    Before reviewing any PR:
    1. Call detect_graph_changes(base="HEAD~1") to get risk-scored changed nodes
    2. Call get_impact_radius(qualified_name) for any high-risk nodes
    3. Call list_tests(qualified_name) to verify test coverage exists
    4. Use get_review_context(files) for token-efficient source access
    5. Only read raw files for nodes flagged as high-risk with no test coverage
  ```

**6.15 Success Criteria:**
> Worker backend recipe picks up task #142 → before_task_142 snapshot saved → worker writes `src/api/health.rs` → graph auto-updates → after_task_142 snapshot saved → `graph_diff(before, after)` shows `"1 new node, 3 new edges"` → this diff visible in AutoDev Panel task card expand → QC agent reviews PR using graph tools with < 500 tokens of context instead of reading entire files

---

### Phase 6.16 — Multi-Repo Registry

**Goal**: Register multiple AIRS-Foundry repos, query across them, merge graphs for cross-repo dependency tracking.

- [ ] **Create `src-tauri/src/graph/registry.rs`** — multi-repo registry:
  - `pub struct RepoRegistry { db_path: PathBuf }` — registry stored in `~/.autodev/registry.json` (JSON file for simplicity, matches code-review-graph approach)
  - `pub fn RepoRegistry::load() -> Result<Self>` — reads from `~/.autodev/registry.json`, creates if absent
  - `pub fn RepoRegistry::register(path: &Path, alias: Option<&str>) -> Result<RepoEntry>` — validates path contains `.git` or `.autodev/` dir; adds to JSON; returns `RepoEntry`
  - `pub fn RepoRegistry::unregister(path: &Path) -> Result<()>`
  - `pub fn RepoRegistry::list() -> Result<Vec<RepoEntry>>`
  - `pub fn RepoRegistry::get_store(path: &Path) -> Result<GraphStore>` — opens `{repo}/.autodev/graph.db`; connection pool via `HashMap<PathBuf, Arc<Mutex<GraphStore>>>` (LRU cap at 10 connections)
  - `pub struct RepoEntry { pub path: String, pub alias: Option<String>, pub db_path: String, pub last_updated: Option<u64>, pub node_count: Option<u32>, pub edge_count: Option<u32> }`

- [ ] **Implement cross-repo graph merge** in `registry.rs`:
  - `pub fn merge_graphs(stores: &[&GraphStore], output_path: &Path) -> Result<()>` — reads all nodes + edges from each store, tags each node with `repo` attribute (from store's db_path parent dir name), deduplicates by `qualified_name`, writes combined `graph.json` to output_path
  - `pub fn merge_all_repos(registry: &RepoRegistry, output_path: &Path) -> Result<MergeStats>` — convenience: opens all registered stores, calls `merge_graphs()`, returns `MergeStats { repos_merged, nodes_merged, edges_merged, cross_repo_edges: usize }`

- [ ] **Add registry Tauri commands** in `src-tauri/src/commands/graph_commands.rs`:
  - `register_repo(path: String, alias: Option<String>) -> Result<RepoEntry>`
  - `unregister_repo(path: String) -> Result<()>`
  - `list_repos() -> Result<Vec<RepoEntry>>`
  - `merge_all_repos(output_path: String) -> Result<MergeStats>`

- [ ] **Registry UI** in `src/tabs/Graph.tsx` — expandable "Multi-Repo" section:
  - List of registered repos from `list_repos()` — shows alias, path, node/edge counts, last_updated
  - `[Register Repo]` button → file picker → calls `register_repo()`
  - `[Unregister]` button per row
  - `[Merge All Graphs]` button → calls `merge_all_repos()` → result opens in browser as merged `graph.html`

**6.16 Success Criteria:**
> Register `core-orchestrator` + `memory-mcp-server` → `list_repos()` returns both → `merge_all_repos()` produces merged `graph.json` with nodes from both repos tagged with repo name → cross-repo IMPORTS_FROM edges visible as surprising connections in GRAPH_REPORT.md

---

### Phase 6 Success Criteria

> **End-to-End Flow**: Razi opens Graph tab → selects `core-orchestrator` repo → clicks `[Build Graph]` → D3 visualization appears (2,000 nodes, colored by community) → clicks `LivingAgent` node → NodeInfoPanel shows 47 callers, 12 callees, 3 test functions → `[Open in IDE Tab]` → IDE tab navigates to `living_agent.py:L45` → Worker backend picks up task → graph auto-updates mid-task → QC agent reviews PR using `detect_changes` in < 400 tokens of graph context

> **Worker Flow**: Worker backend starts task #142 → calls `get_graph_minimal_context(task="add health endpoint")` → receives `AgentService`, `HealthRouter`, `app.py` as relevant nodes → implements only what's needed → PR touches exactly 3 files → QC calls `detect_changes` → sees blast radius is 2 nodes → approves confidently

---

## 15. State Machine & Event System Design

### Dual-Layer State Management

AutoDev has **two state management layers** that must be coordinated:

| Layer | Runtime | FSM Count | Scope | Communication |
|---|---|---|---|---|
| **Rust (Tauri)** | Tokio async | ~6 FSMs | Desktop app lifecycle, UI state, sidecar management | Tauri events + invoke |
| **Python (Core Orchestrator)** | asyncio | 21 FSMs | Agent lifecycle, memory, tool execution, MCP, A2A | FastAPI SSE + HTTP |

**Key principle**: Rust owns desktop-layer state. Python owns intelligence-layer state. The Rust backend mirrors a subset of Python state for UI reactivity, but never duplicates decision-making logic.

### Rust-Layer FSMs (AutoDev Desktop)

| FSM | States | Purpose |
|---|---|---|
| `AppStateFSM` | Initializing → Ready → SessionActive → AgentRunning | Global app lifecycle |
| `SidecarFSM` | Stopped → Starting → Running → Error → Restarting | Per-sidecar process lifecycle |
| `ProviderFSM` | Disconnected → Testing → Connected → Error | LLM provider connection state |
| `PermissionFSM` | Pending → Approved → Denied → Timeout | Tool permission dialog lifecycle |
| `RecipeFSM` | Idle → Parsing → Running → Complete → Error | Recipe execution state |
| `SessionFSM` | New → Active → Paused → Closed | Frontend session state |

### Python-Layer FSMs (Core Orchestrator — 21 FSMs, see Section 2.1)

The core-orchestrator's 21 FSMs are authoritative for agent intelligence state. The Rust backend subscribes to state change events via SSE and updates its mirrored view. Example:

```
Python: TaskExecutionFSM transitions EXECUTING → REVIEWING
  → Emits EVENT: TASK_STATUS_CHANGED {task_id, new_state: "reviewing"}
  → SSE stream → Rust EventBridge
  → Rust: updates local task_state cache
  → Tauri event: "agent:task-status" {task_id, status: "reviewing"}
  → React: AutoDev TaskBoard moves card to "QC Review" column
```

### Global Application State Machine (Rust Layer)

```
┌───────────────┐
│ Initializing  │ → Load config, check sidecars, verify keychain
└───────┬───────┘
        │
        ▼
┌───────────────┐
│ Ready         │ → App is usable, no active session
└───────┬───────┘
        │ create_session / resume_session
        ▼
┌───────────────┐
│ Session Active│ → User can chat, run recipes, etc.
└───────┬───────┘
        │ agent_reply
        ▼
┌───────────────┐
│ Agent Running │ → Agent is processing (streaming, tool calls)
└───────┬───────┘
        │ complete / cancel / error
        ▼
┌───────────────┐
│ Session Active│ → Back to idle within session
└───────────────┘
```

### Event Bus Architecture

All cross-module communication uses a centralized event bus:

```rust
pub enum AppEvent {
    // Agent events
    AgentStreamToken { session_id: String, text: String, done: bool },
    AgentToolCallStart { session_id: String, tool_call: ToolCallInfo },
    AgentToolCallResult { session_id: String, tool_result: ToolResultInfo },
    AgentStatusChange { session_id: String, status: AgentStatus },
    AgentError { session_id: String, error: String },

    // Session events
    SessionCreated { session_id: String },
    SessionCostUpdate { session_id: String, cost: CostInfo },

    // Sidecar events
    SidecarStatusChange { name: String, status: SidecarStatus },

    // Provider events
    ProviderConnectionTest { provider: String, result: ConnectionTestResult },

    // AutoDev events
    DispatchSignalReceived { signal: DispatchSignal },
    HeartbeatSent { agent_id: String },
}
```

---

## 15. Coding Conventions

### Rust (src-tauri/)

- **Error handling**: `anyhow::Result` for application errors, `thiserror` for library errors
- **Async runtime**: Tokio (full features)
- **Logging**: `tracing` crate with structured fields
- **Serialization**: `serde` with `serde_json`
- **Tauri commands**: `#[tauri::command]` with descriptive error types
- **File I/O**: Always through `agent::builtin_tools` module, never raw `std::fs`
- **State sharing**: `Arc<Mutex<T>>` or `Arc<RwLock<T>>` via Tauri managed state
- **Naming**: `snake_case` for functions/variables, `PascalCase` for types/traits
- **Comments**: Doc comments (`///`) on all public items, `//` for implementation notes
- **Debug logging**: `tracing::debug!` with structured fields for every state transition

### TypeScript/React (src/)

- **Components**: Functional components only, no class components
- **State**: Zustand for global state, `useState`/`useReducer` for local
- **API calls**: Via `@tauri-apps/api/core` `invoke()`, never `fetch()`
- **Events**: Via `@tauri-apps/api/event` `listen()`, wrapped in `useTauriEvent` hook
- **Styling**: Tailwind CSS classes only, no CSS-in-JS, no inline styles (except dynamic values)
- **Types**: Strict TypeScript — no `any`, explicit return types on exported functions
- **Naming**: `PascalCase` for components, `camelCase` for functions/variables, `UPPER_SNAKE_CASE` for constants
- **File structure**: One component per file, co-located tests with `.test.tsx` suffix
- **Imports**: Absolute paths from `src/` root (configured in tsconfig `paths`)

### Design System

| Token | Value | Usage |
|---|---|---|
| `bg-primary` | `#0A0A0A` | App background |
| `bg-surface` | `#111111` | Cards, panels, inputs |
| `border` | `#222222` | All borders |
| `accent` | `#00FF88` | Active states, CTAs, live indicators |
| `danger` | `#FF4444` | Errors, rejections, destructive actions |
| `warning` | `#FFAA00` | In-progress, pending, caution |
| `text-primary` | `#F0F0F0` | Primary text |
| `text-secondary` | `#666666` | Secondary/muted text |
| `font-mono` | IBM Plex Mono | Code, numbers, agent names, tool calls |
| `font-sans` | IBM Plex Sans | Prose, UI labels, descriptions |
| `radius` | `2px` | All border radius (sharp, industrial) |
| `spacing` | `8px` | Base spacing unit (multiples: 4, 8, 16, 24, 32, 48) |

---

## 16. File & Directory Scaffold

This is the exact file structure to create for Phase 1. Files marked with `[scaffold]` are minimal stubs to be filled in during implementation.

```
autodev/
├── README.md                          # ✅ Exists
├── package.json                       # ✅ Exists (update scripts)
├── tailwind.config.js                 # ✅ Exists (update content paths)
├── postcss.config.js                  # ✅ Exists
├── vite.config.ts                     # [create] Vite + React + Tauri
├── tsconfig.json                      # [create] TypeScript config
├── tsconfig.node.json                 # [create] Node TypeScript config
├── index.html                         # [create] Vite entry
├── .gitignore                         # ✅ Exists
├── .gitmodules                        # ✅ Exists
├── src-tauri/
│   ├── Cargo.toml                     # [create] Rust deps
│   ├── tauri.conf.json                # [create] Tauri config
│   ├── build.rs                       # [create] Tauri build script
│   ├── icons/                         # [create] App icons
│   └── src/
│       ├── main.rs                    # [create] Entry point
│       ├── lib.rs                     # [create] Module re-exports
│       ├── errors.rs                  # [scaffold]
│       ├── logging.rs                 # [scaffold]
│       ├── events.rs                  # [scaffold]
│       ├── commands/
│       │   ├── mod.rs                 # [scaffold]
│       │   ├── agent_commands.rs      # [scaffold]
│       │   ├── provider_commands.rs   # [scaffold]
│       │   ├── settings_commands.rs   # [scaffold]
│       │   ├── vault_commands.rs      # [scaffold]
│       │   └── session_commands.rs    # [scaffold]
│       ├── agent/
│       │   ├── mod.rs                 # [scaffold]
│       │   ├── engine.rs             # [scaffold]
│       │   ├── context.rs            # [scaffold]
│       │   ├── tool_trait.rs         # [scaffold]
│       │   ├── tool_registry.rs      # [scaffold]
│       │   ├── tool_executor.rs      # [scaffold]
│       │   ├── permission.rs         # [scaffold]
│       │   ├── compaction.rs         # [scaffold]
│       │   └── builtin_tools/
│       │       ├── mod.rs            # [scaffold]
│       │       ├── read_file.rs      # [scaffold]
│       │       ├── write_file.rs     # [scaffold]
│       │       ├── bash.rs           # [scaffold]
│       │       └── list_directory.rs # [scaffold]
│       ├── provider/
│       │   ├── mod.rs                # [scaffold]
│       │   ├── traits.rs            # [scaffold]
│       │   ├── anthropic.rs         # [scaffold]
│       │   ├── streaming.rs         # [scaffold]
│       │   └── model_router.rs      # [scaffold]
│       ├── vault/
│       │   ├── mod.rs               # [scaffold]
│       │   └── keychain.rs          # [scaffold]
│       ├── session/
│       │   ├── mod.rs               # [scaffold]
│       │   ├── manager.rs           # [scaffold]
│       │   ├── storage.rs           # [scaffold]
│       │   └── types.rs             # [scaffold]
│       ├── cost/
│       │   ├── mod.rs               # [scaffold]
│       │   └── tracker.rs           # [scaffold]
│       └── state/
│           ├── mod.rs               # [scaffold]
│           └── app_state.rs         # [scaffold]
├── src/
│   ├── main.tsx                      # [create]
│   ├── App.tsx                       # [create]
│   ├── vite-env.d.ts                 # [create]
│   ├── stores/
│   │   ├── agentStore.ts            # [scaffold]
│   │   ├── sessionStore.ts          # [scaffold]
│   │   ├── settingsStore.ts         # [scaffold]
│   │   ├── costStore.ts             # [scaffold]
│   │   └── sidecarStore.ts          # [scaffold]
│   ├── tabs/
│   │   ├── Chat.tsx                 # [scaffold]
│   │   ├── AutoDevPanel.tsx         # [scaffold]
│   │   ├── IDE.tsx                  # [scaffold]
│   │   ├── Recipes.tsx              # [scaffold]
│   │   └── Settings.tsx             # [scaffold]
│   ├── components/
│   │   ├── layout/
│   │   │   ├── TitleBar.tsx         # [scaffold]
│   │   │   ├── TabBar.tsx           # [scaffold]
│   │   │   └── StatusBar.tsx        # [scaffold]
│   │   ├── chat/
│   │   │   ├── MessageBubble.tsx    # [scaffold]
│   │   │   ├── ToolCallDisplay.tsx  # [scaffold]
│   │   │   ├── InputArea.tsx        # [scaffold]
│   │   │   ├── StreamingText.tsx    # [scaffold]
│   │   │   └── PermissionDialog.tsx # [scaffold]
│   │   ├── settings/
│   │   │   ├── ProviderCard.tsx     # [scaffold]
│   │   │   ├── ModelSelector.tsx    # [scaffold]
│   │   │   ├── KeyInput.tsx         # [scaffold]
│   │   │   └── AutoDevConfig.tsx    # [scaffold]
│   │   └── common/
│   │       ├── Button.tsx           # [scaffold]
│   │       ├── Card.tsx             # [scaffold]
│   │       ├── Badge.tsx            # [scaffold]
│   │       └── Spinner.tsx          # [scaffold]
│   ├── hooks/
│   │   ├── useTauriEvent.ts        # [scaffold]
│   │   ├── useTauriInvoke.ts       # [scaffold]
│   │   └── useAgent.ts             # [scaffold]
│   ├── lib/
│   │   ├── tauri.ts                # [scaffold]
│   │   ├── format.ts              # [scaffold]
│   │   └── cost.ts                # [scaffold]
│   ├── types/
│   │   ├── agent.ts               # [scaffold]
│   │   ├── provider.ts            # [scaffold]
│   │   ├── session.ts             # [scaffold]
│   │   └── events.ts              # [scaffold]
│   └── styles/
│       ├── globals.css             # [create]
│       └── theme.ts               # [create]
├── submodules/                      # ✅ Exists
│   ├── core-orchestrator/          # ✅ Cloned
│   └── memory-mcp-server/          # ✅ Cloned
├── docs/                            # ✅ Exists
│   ├── development/                # ✅ This directory
│   │   └── DEVELOPMENT_PLAN.md    # ✅ This file
│   ├── autodev-vision/             # ✅ Exists
│   ├── submodules-management/      # ✅ Exists
│   └── ENVIRONMENT_SETUP.md       # ✅ Exists
├── build/                          # Empty (Phase 5)
└── sidecars/                       # Empty (Phase 3)
```

---

## 17. Testing Strategy

### Rust Tests

- **Unit tests**: In-module `#[cfg(test)]` blocks for all pure functions
- **Integration tests**: `tests/` directory for multi-module interaction
- **Tool tests**: Each built-in tool has tests with temp directories
- **Provider tests**: Mock HTTP responses with `wiremock` or `mockito`

### Frontend Tests

- **Component tests**: Vitest + Testing Library for React components
- **Store tests**: Zustand store logic tested in isolation
- **E2E tests**: Playwright for critical user flows (Phase 2+)

### Test Commands

```bash
# Rust tests
cd src-tauri && cargo test

# Frontend tests
npm test

# E2E tests (Phase 2+)
npx playwright test
```

---

## 18. Risk Log & Open Questions

### Risks

| Risk | Impact | Mitigation |
|---|---|---|
| `rmcp` crate maturity | Could have gaps vs MCP SDK | Fall back to manual JSON-RPC if needed |
| PyInstaller sidecar size | Bundled Python binaries ~50-100MB each | Use Nuitka for smaller binaries, or ship Python wheel + embedded interpreter |
| Redis bundling on Windows | Redis doesn't have official Windows binary | Use Memurai or Redis for Windows fork, or switch to SQLite for caching |
| IBM Plex font licensing | Need to verify license for bundled desktop app | OFL license — verify allows bundling |
| Tauri v2 API stability | V2 is newer, fewer community examples | Pin specific version, test thoroughly |
| Dual-layer state drift | Rust and Python FSMs could desynchronize | SSE heartbeats + reconciliation on reconnect; Python is always authoritative |
| Core-orchestrator API surface | FastAPI endpoints not yet documented for external consumption | Audit `api_gateway/` routes before building Rust HTTP client |
| Sidecar startup latency | Python + mem0 + Redis cold start could take 5-10s | Show loading state in UI; pre-warm during app splash screen |
| Memory provider dependencies | mem0 SDK, Redis, ChromaDB each add complexity | Start with mem0 only; other providers are opt-in via config |
| `git2-rs` (libgit2) complexity | libgit2 API is complex; push/auth requires credential callbacks | Start with clone/branch/diff/status (easy); push via CLI fallback if git2 auth is problematic |
| Worktree disk usage | Multiple worktrees duplicate working trees per task | Implement sparse checkout + symlinks for `node_modules`/`.venv` (same as Claude Code) |

### Resolved Questions

1. ~~**MCP transport for sidecars**: stdio or HTTP?~~
   - **RESOLVED → HTTP**. Core-orchestrator uses FastAPI + SSE on :4000, memory-mcp-server uses FastMCP streamable HTTP on :5000. Both are already configured for HTTP transport in `mcp_config.yaml`. This enables independent testing, hot-reload, and debugging.

2. ~~**Session storage format**: Single SQLite file vs per-session?~~
   - **RESOLVED → Dual storage**. Rust SQLite for frontend session state (message history, UI preferences). Memory-mcp-server handles semantic memory storage via mem0/Redis. Two concerns, two stores.

3. ~~**Font loading**: Bundle or CDN?~~
   - **RESOLVED → Bundle**. Desktop app, no CDN dependency. IBM Plex is OFL-licensed.

4. ~~**Model context window tracking**: Client-side or provider?~~
   - **RESOLVED → Python-side**. Core-orchestrator's `token_validator.py` already implements token budget management with auto-degradation. Rust only needs to display the count, not compute it.

### Open Questions (New)

1. ~~**Tool execution split**: Which tools run in Rust vs Python?~~
   - **RESOLVED → Dual-mode architecture (see Section 4.1)**. PA mode runs Rust-native: file I/O (`read_file`, `write_file`, `bash`), git (`git2-rs`), MCP clients (`rmcp`). Worker mode delegates to core-orchestrator: litellm provider, airs_mcp client, mem0 memory, A2A protocol, 21 FSMs. No code duplication — each mode uses the right abstraction level for its complexity.

2. **SSE vs WebSocket for streaming**: Core-orchestrator supports SSE via `sse-starlette`. Should we add WebSocket?
   - **Recommendation**: Start with SSE (already implemented). WebSocket only if we need bidirectional streaming (e.g., live tool permission dialogs during execution).

3. **A2A integration scope for Phase 1**: Core-orchestrator has full A2A support. When do we expose it?
   - **Recommendation**: Phase 4. A2A is for multi-agent coordination (supervisor → workers). Phase 1-3 focus on single-agent PA mode.

4. **Core-orchestrator as sidecar vs embedded**: Should we compile core-orchestrator into WASM or keep as subprocess?
   - **Recommendation**: Keep as subprocess. 2400+ line LivingAgent with litellm, asyncio, pydantic, mem0 — far too complex for WASM. Subprocess is the right abstraction.

5. ~~**How should workers modify repos?**~~
   - **RESOLVED → Direct local writes + git worktree isolation**. Workers modify the local filesystem directly via Rust-native file tools (`write_file`, `edit_file`). Each task gets an isolated git worktree (`.autodev/worktrees/<slug>/`) so agents work without polluting the main branch. Commits, pushes, and PRs are structured via `git2-rs`. See Section 6.1-6.2 for full analysis of Claude Code and Goose approaches.

6. ~~**Should the UI have a Git tab (like GitHub Desktop)?**~~
   - **RESOLVED → No separate tab. Git integrated into IDE tab as sub-panel**. The IDE tab gets a toggleable git sub-panel with repo selector, clone capability, branch switcher, staged/unstaged view, commit/push/PR actions, and worktree manager. This makes AutoDev a "single pane of glass" without fragmenting the 5-tab UI. See Section 6.3.

---

*This document is a living plan. Update it after each phase completes and as architectural decisions are made during implementation.*
