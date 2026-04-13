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
11. [Phase 3 — IDE + Skills + Hooks + MCP Server](#11-phase-3--ide--skills--hooks--mcp-server)
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
   - "Open in IDE" button (Phase 3 integration)

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

**Recipes System (Minimal):**
- [ ] YAML recipe parser with validation — Parse recipe YAML into Recipe struct
- [ ] Recipe execution engine (run recipe steps as agent messages) — Core engine
- [ ] Cron scheduler for recurring recipes — `tokio-cron-scheduler` to poll every 2 min
- [ ] Hardcoded Supervisor recipe (`~/.autodev/recipes/supervisor-poll.yaml`):
  - Polls `~/.autodev/tasks.db` every 2 min for tasks with status='ready'
  - Assigns to hardcoded `worker-backend` (single worker for 2.1)
  - Updates task: `assigned_worker = 'worker-backend'`, `status = 'assigned'`
- [ ] Hardcoded Worker recipe (`~/.autodev/recipes/worker-backend-poll.yaml`):
  - Polls for tasks assigned to 'worker-backend'
  - Claims task via lock file, executes (bash/write_file), marks 'done'
  ```yaml
  version: "1.0.0"
  title: Worker Backend — Polling Loop
  schedule: "*/120 * * * *"
  permission_mode: autonomous
  instructions: |
    You are the AIRS Backend Worker.
    1. Poll internal task board for tasks assigned to 'worker-backend'
    2. If found: claim via lock file, execute task, test, mark done
    3. If nothing: exit cleanly
  ```
- [ ] Recipe browser UI (read-only for 2.1) — List and view recipe YAML

**AutoDev Panel & Internal Board (Minimal Viable):**
- [ ] Internal Task Board database:
  - SQLite database at `~/.autodev/tasks.db`
  - Schema: tasks table with id, title, priority, type, status, acceptance_criteria, assigned_worker, timestamps
  - Rust module: `src-tauri/src/db/tasks.rs` with `rusqlite` crate
  - Tauri commands: `create_task`, `list_tasks`, `update_task_status`, `get_task_by_id`
- [ ] AutoDev Panel: **View-only** task board (4-column kanban: Ready, In Progress, QC Review, Done Today)
- [ ] TaskBoard component reads from SQLite via Tauri commands
- [ ] Task status updates propagate to UI in real-time via Tauri events
- [ ] AutoDev Panel: **Hardcoded** agent roster showing:
  - Supervisor (always present, shows status)
  - worker-backend (hardcoded for 2.1, shows status: Active/Working/Idle)
  - QC Agent (optional for 2.1, can defer to 2.2)
  - Status indicator: 🟢 Active / 🟡 Working / 🔵 Idle (no Error/Paused states for 2.1)

**PA Mode Toggle (Full Implementation):**
- [ ] **UI Components:**
  - ChatHeader dropdown with 3 modes: 🔧 Fast Development, ✋ Review Mode, 📋 Delegate Mode
  - Mode indicator with colored dot (green/yellow/blue)
  - Mode state persisted in `settingsStore.ts`
  - Mode selection triggers system prompt reload + tool registry rebuild
- [ ] **Backend - System Prompt Variants:**
  - `src-tauri/src/agent/context.rs`: Define 3 system prompt constants:
    - `BASE_SYSTEM_PROMPT` (current, for Fast Development)
    - `REVIEW_SYSTEM_PROMPT` (adds permission-checking instructions)
    - `DELEGATE_SYSTEM_PROMPT` (removes file tool descriptions, adds task decomposition guidance)
  - `ContextBuilder::build_system_prompt()` accepts `PAMode` enum, selects appropriate prompt
  - `AUTODEV.md` is stored in SQLite (`~/.autodev/config/autodev_global.md` + per-repo entries), editable from UI sidebar
  - Settings panel: add "System Prompts" section with 3 text areas for editing each variant
  - Tauri commands: `get_system_prompt_variant(mode)`, `set_system_prompt_variant(mode, content)`
- [ ] **Backend - Tool Registry Filtering:**
  - `src-tauri/src/agent/tools/mod.rs`: Add `filter_tools_by_mode(tools, mode) -> Vec<Tool>`
  - Fast Development: all builtin_tools (read_file, write_file, edit_file, bash, grep, glob)
  - Review Mode: all builtin_tools + wrap write operations with permission_check
  - Delegate Mode: only `create_task` tool (zero file tools)
  - Tool registry built conditionally before LLM call based on current PA mode
- [ ] **Backend - create_task Tool:**
  - New builtin tool: `src-tauri/src/agent/builtin_tools/create_task.rs`
  - Input schema: title (string), priority (enum: p0/p1/p2/p3), type (enum: feature/bug/refactor/docs), acceptance_criteria (string)
  - Writes to `~/.autodev/tasks.db` with status="ready"
  - Returns task ID and confirmation message
  - Only registered in Delegate Mode

**AUTODEV.md UI-Managed Instruction System (Phase 2.1.2):**

> **Core Principle**: AUTODEV.md is NOT a file you manually create. It is a first-class UI feature — created, edited, and stored entirely within the AutoDev app. SQLite-backed, not filesystem-scattered.

**Backend — Database Layer:**
- [ ] **SQLite Schema** (`src-tauri/src/db/schema.sql`):
  ```sql
  CREATE TABLE autodev_instructions (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      scope TEXT NOT NULL CHECK (scope IN ('global', 'repo')),
      repo_path TEXT,  -- NULL for global, absolute path for repos
      content TEXT NOT NULL DEFAULT '',
      created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
      updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
      is_active BOOLEAN DEFAULT 1,
      CONSTRAINT repo_path_required CHECK (
          (scope = 'global' AND repo_path IS NULL) OR
          (scope = 'repo' AND repo_path IS NOT NULL)
      )
  );
  CREATE INDEX idx_autodev_instructions_scope_repo ON autodev_instructions(scope, repo_path);
  CREATE TRIGGER update_autodev_instructions_timestamp 
  AFTER UPDATE ON autodev_instructions
  BEGIN
      UPDATE autodev_instructions SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
  END;
  ```
- [ ] **Rust Repository Module** (`src-tauri/src/db/autodev_instructions.rs`):
  - `AutodevInstructionsRepo::get(conn, scope, repo_path) -> Result<Option<String>>`
  - `AutodevInstructionsRepo::set(conn, scope, repo_path, content) -> Result<()>`
  - `AutodevInstructionsRepo::delete(conn, repo_path) -> Result<()>`
  - `AutodevInstructionsRepo::exists(conn, scope, repo_path) -> Result<bool>`
  - `AutodevInstructionsRepo::char_count(conn, scope, repo_path) -> Result<usize>`
- [ ] **Tauri Commands** (`src-tauri/src/commands/autodev_instructions.rs`):
  - `get_autodev_instructions(scope, repo_path) -> Result<String, String>` — Returns content or empty string
  - `set_autodev_instructions(scope, repo_path, content) -> Result<(), String>` — Insert/update with 40,000 char limit validation
  - `delete_autodev_instructions(repo_path) -> Result<(), String>` — Soft delete for repo scope
  - `check_autodev_instructions_exists(scope, repo_path) -> Result<bool, String>`
  - `get_autodev_instructions_char_count(scope, repo_path) -> Result<usize, String>`

**Frontend — UI Components:**
- [ ] **Sidebar Card** (`src/components/sidebar/AutodevMdCard.tsx`):
  - Shows "AUTODEV.md" header with status indicator (🟢 Active / 🟡 Modified / ⚪ Empty)
  - Quick action buttons: "Edit Global" | "Edit This Repo"
  - Character count preview: "1,247 / 40,000"
  - "Create" button if doesn't exist yet
  - Collapsible section in persistent sidebar
- [ ] **Editor Component** (`src/components/autodev/AutodevMdEditor.tsx`):
  - Monaco Editor (@monaco-editor/react) with Markdown syntax highlighting
  - Tab switching: [Global] [This Repo]
  - Live character counter with 40,000 limit warning (enforce at save)
  - Auto-save toggle (default: ON)
  - Manual Save / Reset buttons
  - "Preview System Prompt" button — shows full prompt that will be injected
  - Close confirmation if unsaved changes
- [ ] **Zustand Store** (`src/store/autodevMdStore.ts`):
  - State: `globalContent`, `repoContents` (Map), `modifiedScopes` (Set), `charCounts` (Map)
  - Actions: `loadGlobal()`, `loadRepo(repoPath)`, `saveGlobal(content)`, `saveRepo(repoPath, content)`, `setModified(scope, isModified)`
- [ ] **Settings Panel Section** (`src/components/settings/AutodevMdSettings.tsx`):
  - "Filesystem Sync" toggle (default: OFF) — when ON, syncs to `{repo}/.autodev/autodev.md`
  - "Auto-save" toggle (default: ON)
  - Display global path: `~/.autodev/config/autodev_global.md` (read-only)
  - "Import from CLAUDE.md" migration button

**Integration:**
- [ ] **ContextBuilder Integration** (`src-tauri/src/agent/context.rs`):
  - Replace legacy file-reading with `AutodevInstructionsRepo::get()` calls
  - `build_system_prompt(mode, repo_path)` fetches both global and repo content from SQLite
  - Remove all CLAUDE.md and AGENTS.md fallbacks (proprietary system only)
- [ ] **Filesystem Sync** (optional feature):
  - When "Filesystem Sync" is enabled in settings:
    - On `set_autodev_instructions`: also write to `{repo_path}/.autodev/autodev.md`
    - On load: check if file exists and import if DB is empty
  - Create `.autodev/` directory if doesn't exist when syncing
- [ ] **Migration Tool**:
  - `migrate_from_claude_md(repo_path)` command:
    - Check `~/.claude/CLAUDE.md` → import as Global AUTODEV.md
    - Check `{repo}/CLAUDE.md` → import as Repo AUTODEV.md
    - Check `{repo}/.claude/CLAUDE.md` → import as Repo AUTODEV.md
    - Check `{repo}/AGENTS.md` → import as Repo AUTODEV.md (Goose compatibility)
  - UI: Settings → "Import from Claude Code/Goose"

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

---

#### Phase 2.2 — "Production Worker Orchestration" 🚀 **Scale to Multiple Workers + GitHub**

**Goal:** Production-grade worker management, multiple worker types, GitHub integration, full submodule bridge.

**Worker Management System (Full Implementation):**
- [ ] **Workers Database:**
  - SQLite database at `~/.autodev/workers.db`
  - Schema: workers table (see Module Breakdown § 8 for full schema)
  - Rust module: `src-tauri/src/db/workers.rs` with `rusqlite` crate
  - Tauri commands:
    - `create_worker(name, type, primary_repos, model, polling_interval)` → Returns worker_id
    - `list_workers()` → Returns all workers (system + user-created)
    - `update_worker_status(worker_id, status)` → Updates worker state
    - `delete_worker(worker_id)` → Soft delete (sets status='deleted')
    - `get_worker_by_slug(slug)` → Retrieves worker config
    - `start_worker_recipe(worker_id)` → Starts recipe cron job
    - `stop_worker_recipe(worker_id)` → Stops recipe cron job
- [ ] **Pre-Defined Workers Seeded on First Launch:**
  - On app first run, seed `workers.db` with 5 pre-defined workers:
    - `worker-backend` (type: backend, model: claude-sonnet-4-6, status: created)
    - `worker-frontend` (type: frontend, model: claude-sonnet-4-6, status: created)
    - `worker-platform` (type: platform, model: claude-sonnet-4-6, status: created)
    - `worker-mcp` (type: mcp, model: claude-opus-4-6, status: created)
    - `worker-cleanup` (type: cleanup, model: claude-haiku-4-5, status: created)
  - Also seed system agents:
    - `supervisor` (is_system_defined: 1, cannot delete)
    - `qc-agent` (is_system_defined: 1, cannot delete)
- [ ] **Worker Creation UI (Settings → Workers Tab):**
  - "Create Worker" button → Opens modal
  - Worker creation form fields:
    - Worker Name (auto-generates slug from name)
    - Worker Type dropdown (Backend, Frontend, Platform, MCP Builder, Cleanup, Custom)
    - Primary Repos multi-select (fetches from GitHub org via PAT)
    - Model dropdown (lists available models from providers)
    - Polling Interval slider (30s - 10min, default 2min)
    - Recipe Template: Auto-generate or Custom YAML
  - On submit:
    - Creates worker in `workers.db`
    - Generates recipe YAML at `~/.autodev/recipes/worker-{slug}-poll.yaml`
    - Updates Agent Roster dynamically
  - Validation:
    - Worker name must be unique
    - Slug must be unique (auto-sanitized from name)
    - Primary repos must exist in GitHub org
- [ ] **Worker Management UI (Settings → Workers Tab):**
  - Table showing all workers:
    - Columns: Name, Type, Status, Model, Primary Repos, Actions
    - System agents (Supervisor, QC) shown with 🔒 lock icon, cannot delete
    - Pre-defined workers can be edited/deleted
    - Custom workers can be edited/deleted
  - Worker row actions:
    - Start/Stop/Pause (toggles recipe execution)
    - Edit (opens edit modal)
    - Delete (soft delete, confirms first)
  - Worker status indicators (full set):
    - 🟢 Active (recipe running, polling)
    - 🟡 Working (executing task)
    - 🔵 Idle (created but not started)
    - 🔴 Error (recipe crashed)
    - ⏸️ Paused (user paused)
- [ ] **Agent Roster Dynamic Updates:**
  - Agent Roster reads from `workers.db` instead of hardcoded list
  - Real-time updates when worker status changes
  - Shows queue count for workers in 'queued' state
- [ ] **Recipe Auto-Generation:**
  - Template system for worker recipes
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

**Enhanced Supervisor Logic (Full Implementation):**
- [ ] **Supervisor Recipe Logic (Production):**
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
- [ ] GitHub PAT storage in vault — Secure keychain storage
- [ ] GitHub MCP integration: list repos, create PR, review PR — Workers use this to create PRs after completing tasks

**Provider Support:**
- [ ] Additional providers: OpenAI, Gemini — Extend beyond Anthropic

**Submodule Bridge (core-orchestrator integration):**
- [ ] **[SUBMODULE]** `unified_tool_handler.py`: Add `register_autodev_handler()` + routing logic
- [ ] **[SUBMODULE]** `autodev_handler.py`: New file with AutoDevToolHandler class
- [ ] **[SUBMODULE]** `living_agent.py`: Wire up autodev_handler in `initialize_bridges()`
- [ ] **[SUBMODULE]** `tool_executor.py`: Handle `__autodev` suffix in execution tracking
- [ ] **[SUBMODULE]** Unit tests for autodev tool routing
- [ ] **[SUBMODULE]** Integration tests for worker recipes with AutoDev tools

**2.2 Success Criteria:**
> Create custom worker "worker-api" via UI → Assign it specific repos → Task routes to it → Worker creates PR on GitHub → QC reviews → Supervisor merges

---

### Phase 2 Dependency Graph

```
Phase 2.1 Dependencies:
  create_task tool → tasks.db → TaskBoard UI
  PA Mode Toggle → System Prompt Variants → Tool Registry Filtering
  Cron Scheduler → Hardcoded Supervisor Recipe → Hardcoded Worker Recipe

Phase 2.2 Dependencies (requires 2.1):
  workers.db → Worker Management UI → Recipe Auto-Generation
  GitHub PAT → GitHub MCP → Workers create PRs
  Submodule Bridge → core-orchestrator integration
```

---

## 11. Phase 3 — IDE + Skills + Hooks + MCP Server

**Goal**: IDE tab with file tree, Skills system, Hooks system, AIRS MCP server, sidecar management.

### Key Components

- **IDE Tab**: File tree (recursive dir listing), code viewer with syntax highlighting, **git sub-panel** (see Section 6.3)
- **Git Integration**: `git2-rs` module with clone, branch, diff, status, commit, push, worktree management (see Section 6.2)
- **File Checkpointing**: Snapshot before/after each file write, undo support (see Section 6.2)
- **Repo Management**: Clone AIRS Foundry repos from within the IDE tab, multi-repo selector
- **Skills**: `.md` files in `.airs/skills/` loaded as reusable prompt templates
- **Hooks**: Event-driven extensibility (PreToolUse, PostToolUse, SessionStart)
- **AIRS MCP Server**: Expose `airs_get_tasks`, `airs_claim_task`, `airs_report_progress`, etc.
- **Sidecar Manager**: Start/stop/monitor `core-orchestrator` and `memory-mcp-server`

### Hooks Event System

```
┌──────────────┐      ┌──────────────────┐      ┌───────────────┐
│ Agent Engine │─emit─►│ Event Dispatcher │─────►│ Hook Registry │
└──────────────┘      └──────────────────┘      └───────┬───────┘
                                                        │
                                                   ┌────┴────┐
                                                   ▼         ▼
                                             ┌─────────┐ ┌─────────┐
                                             │ Hook A  │ │ Hook B  │
                                             │ (file)  │ │ (shell) │
                                             └─────────┘ └─────────┘
```

### Phase 3 Deliverables Checklist

- [ ] IDE tab: file tree browser with icons
- [ ] IDE tab: code viewer with syntax highlighting
- [ ] IDE tab: git sub-panel (toggle via [Git ▼] button)
- [ ] IDE tab: repo selector dropdown + "Clone Repo" button
- [ ] Git: `git2-rs` repo open/clone/status/diff/branch/commit/push
- [ ] Git: worktree create/enter/exit/remove/list
- [ ] Git: file checkpointing per message (undo support)
- [ ] Git: commit attribution trailer (`Generated-by: AutoDev/<role>`)
- [ ] Git: clone repos from AIRS Foundry org via GitHub API
- [ ] Skills system: load `.md` skills, expose as slash commands
- [ ] Hooks system: PreToolUse, PostToolUse, SessionStart events
- [ ] Hooks config: `.airs/hooks.json` per-repo
- [ ] AIRS MCP server: 5+ tools (get_tasks, claim_task, etc.)
- [ ] Sidecar manager: start/stop core-orchestrator
- [ ] Sidecar manager: start/stop memory-mcp-server
- [ ] Sidecar manager: start/stop Redis
- [ ] Sidecar health checks + auto-restart
- [ ] Additional tools: `glob`, `grep`, `web_search`
- [ ] Ollama provider (local models)

---

## 12. Phase 4 — Subagents + Model Router + Dispatch

**Goal**: Multi-agent orchestration, intelligent model routing, AutoDev dispatch signal handling.

### Key Components

- **Subagents**: Spawn child agents with isolated tool sets and context
- **Model Router**: Map agent roles to specific provider/model pairs
- **Dispatch Signals**: Watch filesystem for dispatch signal files
- **Agent Teams**: Coordinate multiple agents on a workflow

### Phase 4 Deliverables Checklist

- [ ] Subagent spawning with isolated context
- [ ] Model Router: configurable role → model mapping
- [ ] Model Router UI in Settings tab
- [ ] Dispatch signal watch (FSEvents/inotify via `notify` crate)
- [ ] Dispatch signal UI in AutoDev Panel
- [ ] Agent team coordination (supervisor → workers pattern)
- [ ] Lock file management for task claiming
- [ ] Heartbeat reporting (every 5 min to status file)
- [ ] Azure OpenAI provider

---

## 13. Phase 5 — Plugin + CLI + Installers

**Goal**: One-click worker configuration, headless CLI, platform installers.

### Phase 5 Deliverables Checklist

- [ ] `airsctl` CLI binary (separate Rust binary in `src-cli/`)
- [ ] AIRS Plugin for one-click worker PC setup
- [ ] Windows installer (MSI via WiX or NSIS)
- [ ] macOS installer (DMG)
- [ ] Linux installer (AppImage + .deb)
- [ ] IT staff runbook document
- [ ] Sidecar binaries bundled in installers

---

## 14. State Machine & Event System Design

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
