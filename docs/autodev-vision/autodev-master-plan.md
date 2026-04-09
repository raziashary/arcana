# AutoDev — Product Specification & Build Prompt
### A Custom Desktop AI Agent Platform for AIRS Foundry

**Document Status:** Draft v1.0
**Author:** Razi (Programming Lead, AIRS Foundry)
**Date:** April 2026
**Classification:** Internal — AutoDev Project

---

## Table of Contents

1. [Vision](#1-vision)
2. [Why Build This](#2-why-build-this)
3. [Feature Inventory — Claude Code vs Goose vs AutoDev](#3-feature-inventory)
4. [Architecture Overview](#4-architecture-overview)
5. [UI/UX Design Spec](#5-uiux-design-spec)
6. [Core Features — Deep Spec](#6-core-features-deep-spec)
7. [Tech Stack Decision](#7-tech-stack-decision)
8. [File & Config Structure](#8-file--config-structure)
9. [AutoDev Integration Layer](#9-autodev-integration-layer)
10. [Build Prompt — Start Coding](#10-build-prompt--start-coding)
11. [Phased Build Plan](#11-phased-build-plan)

---

## 1. Vision

**AutoDev** is a downloadable desktop + CLI application — purpose-built for AIRS Foundry — that combines the best capabilities of Claude Code and Goose into a single, branded, model-agnostic AI developer agent platform.

Any AIRS team member or AutoDev worker agent can:
- Download one binary
- Enter their LLM API key (any provider)
- Select their model
- Start executing agentic development tasks with the full capability stack: CLAUDE.md/AGENTS.md, Skills, Recipes, Subagents, MCP servers, Hooks, and the AutoDev worker pipeline

**The tagline:** *One app. Any model. Full autonomy.*

---

## 2. Why Build This

### The Problem with Claude Code Alone
- Locked to Anthropic subscriptions with **rate limits that cripple 24/7 worker agents**
- **512,000-line source leak (March 31, 2026)** exposed vulnerabilities via npm supply chain — the harness is now a known attack surface
- No model flexibility — cannot route workers to cheaper models for lower-stakes tasks
- Proprietary — cannot audit, fork, or adapt the harness for AIRS-specific needs

### The Problem with Goose Alone
- AGENTS.md ecosystem is newer and less mature than CLAUDE.md
- Not tailored to AutoDev's Supervisor → Worker → QC pipeline
- Generic tool — no AIRS-specific automation, no built-in task board awareness

### The Case for AutoDev
- **Model-agnostic by design**: use Anthropic API directly (no rate-limited subscriptions), or OpenAI, Gemini, or local Ollama on DGX Sparks in Year 2
- **CLAUDE.md-compatible**: workers keep their existing instruction files — zero migration cost
- **AutoDev-native**: the app understands AIRS's GitHub Issues board, dispatch signals, and lock file patterns natively — no extra scripting
- **Secure and auditable**: AIRS controls the harness, not a third-party binary from npm
- **Downloadable and distributable**: IT staff provisions worker PCs by installing one `.exe` / `.AppImage` / `.dmg`

---

## 3. Feature Inventory

### What We're Taking from Claude Code

| Feature | Description | Why We Keep It |
|---|---|---|
| **CLAUDE.md** | Per-repo instruction files that define agent behavior, domain, constraints | The entire AutoDev system is built around this — core primitive |
| **Headless `-p` mode** | Run agent from CLI with a single prompt, no interactive session | The polling scripts (`poll-worker-backend.sh`) depend on this exactly |
| **`--dangerously-skip-permissions`** | Allow agent to take actions without per-step confirmation | Required for 24/7 unattended worker operation |
| **Subagents** | Spawn isolated sub-agents for specialized tasks; results returned as summary | Up to 10 parallel subagents — critical for Supervisor breaking down epics |
| **Agent Teams** | Multiple independent Claude sessions that coordinate and message each other | The Supervisor ↔ Worker architecture is literally this pattern |
| **Hooks** | Scripts that fire at lifecycle events (pre/post tool call, session start/end) | Used to enforce linting, run tests, post Slack notifications automatically |
| **Skills** | Reusable `.md` files that define slash-command workflows | `/review-pr`, `/run-tests`, `/write-spec` — shareable across all workers |
| **MCP client** | Connect to external services (GitHub, Slack, Sentry) via MCP protocol | GitHub MCP is the nervous system of AutoDev |
| **`/loop` scheduled tasks** | Cron-like background agent loops | Replaces external Task Scheduler — workers can self-schedule |
| **Plugins** | Bundle Skills + MCP servers + Hooks into installable packages | AIRS plugin = one install to configure any worker PC |
| **Context compaction** | Auto-compress stale context to prevent context window overflow | Essential for long-running worker sessions |
| **Plan mode** | Agent maps the work before executing — halves token waste | Used by Supervisor when breaking down epics |
| **Git worktree integration** | Each task gets an isolated git worktree branch via `git2-rs`; file checkpointing per message (undo support); commit attribution trailers | The `worker/backend/<issue>` branching pattern; agents work in isolation without polluting main |
| **`@mention` subagents** | Reference specific subagents by name in conversation | Supervisor can address workers by name |
| **HTTP hooks** | POST JSON to external services on lifecycle events | Send task completion events to AIRS dashboard |
| **`defer` permission** | Pause headless session, wait for external signal to resume | Multi-agent coordination between Supervisor and Workers |

### What We're Taking from Goose

| Feature | Description | Why We Keep It |
|---|---|---|
| **Model-agnostic provider layer** | Plug in any LLM: Anthropic, OpenAI, Gemini, Ollama, Azure, Bedrock | Pay direct API rates, no subscription caps; DGX Spark local inference in Year 2 |
| **Recipes (YAML workflows)** | Portable, shareable YAML workflow definitions with sub-recipes | AIRS worker polling loop becomes a Recipe — versioned, shareable, reproducible |
| **Recipe scheduler (cron)** | Schedule recipes with cron expressions | `0 */2 * * * worker-backend-poll` — cleaner than Windows Task Scheduler |
| **Recipe parameters** | Dynamic inputs to recipes — typed, required, described | Pass issue numbers, repo names, priority flags into worker recipes |
| **MCP-UI components** | Extensions render interactive UI widgets, not just text | AIRS dashboard panels inside the app — agent status, token costs, PR queue |
| **Session export (JSON)** | Export sessions with full token usage, model config, timestamps | Cost tracking per worker, per task — feeds into AutoDev financial model |
| **Approval modes** | Autonomous / Manual Approval / Smart Approval / Chat Only | Workers run Autonomous; Supervisor uses Smart Approval for destructive ops |
| **`.goosehints`** | Per-directory instruction injection (like CLAUDE.md but directory-scoped) | Complementary to CLAUDE.md for fine-grained per-path context |
| **OS-level sandbox** | `sandbox-exec` restricts file and network access per session | Security boundary around worker agents — prevent scope creep |
| **Custom distributions** | Build your own branded Goose distro with preconfigured providers and branding | AutoDev IS a custom distribution — this is the build model |
| **ACP (Agent Client Protocol)** | Connect to IDE plugins — JetBrains, VS Code, Cursor | AIRS developers use the IDE panel while workers use CLI/headless mode |
| **Per-session cost audit** | Token usage, cache hits, model config all logged per session | Feed directly into AutoDev cost dashboard |
| **Headless recipe runner** | Run recipes as CLI one-shots — `airsctl run recipe worker-backend` | The worker polling pattern as a first-class CLI command |

### What AutoDev Adds Natively (New)

| Feature | Description |
|---|---|
| **AutoDev Panel** | Built-in tab showing live GitHub Issues board, agent status, QC queue, and token costs |
| **Agent Roster** | UI view of all 7 agents (Supervisor, 5 Workers, QC) with their current task, status, and last heartbeat |
| **Key Vault** | Secure local storage for API keys — per-provider, encrypted, never transmitted |
| **Model Router** | Route different task types to different models (Opus for Supervisor/QC, Sonnet for Workers, Haiku for Cleanup) |
| **AIRS Plugin** | One-click plugin that pre-configures any worker PC with the right CLAUDE.md, MCP servers, GitHub PAT scopes, and recipe set for their role |
| **Cost Meter** | Real-time token spend display per agent per session, with 30-day rolling totals |
| **IDE Tab** | Built-in file tree, code viewer, and **git sub-panel** — read-only for workers, edit mode for Razi |
| **Git Sub-Panel** | Embedded in IDE tab: repo selector, clone AIRS repos, branch switcher, staged/unstaged view, commit/push/PR, worktree manager |
| **Dispatch Signal UI** | Visual button to send a dispatch signal to any worker immediately from the UI |
| **PA Mode Toggle** | Fast Development ↔ Delegate Mode switch in Chat tab. Fast = full tools; Delegate = creates GitHub issues + dispatches to workers, zero file tools |

---

## 4. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          AutoDev DESKTOP APP                          │
│                                                                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐  │
│  │  Chat    │  │  AutoDev │  │  IDE     │  │ Recipes  │  │Settings │  │
│  │  Tab     │  │  Panel   │  │  Tab     │  │  Tab     │  │  Tab    │  │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬────┘  │
│       │              │              │              │              │       │
│       └──────────────┴──────────────┴──────────────┴─────────────┘       │
│                                    │                                      │
│                           ┌────────▼────────┐                            │
│                           │   AIRS CORE     │                            │
│                           │  Agent Engine   │                            │
│                           │  (Rust + TS)    │                            │
│                           └────────┬────────┘                            │
│                                    │                                      │
│         ┌──────────────────────────┼──────────────────────────┐          │
│         │                          │                           │          │
│  ┌──────▼──────┐          ┌────────▼──────┐         ┌─────────▼──────┐  │
│  │  Provider   │          │   MCP Client  │         │  Recipe Engine │  │
│  │  Layer      │          │   + Server    │         │  + Scheduler   │  │
│  │             │          │               │         │                │  │
│  │ • Anthropic │          │ • GitHub MCP  │         │ • YAML Recipes │  │
│  │ • OpenAI    │          │ • Slack MCP   │         │ • Sub-recipes  │  │
│  │ • Gemini    │          │ • Sentry MCP  │         │ • Cron loops   │  │
│  │ • Ollama    │          │ • Custom MCP  │         │ • Parameters   │  │
│  │ • Azure     │          │               │         │                │  │
│  └─────────────┘          └───────────────┘         └────────────────┘  │
│                                                                           │
│         ┌──────────────────────────────────────────────────────┐         │
│         │                   AIRS FEATURES                       │         │
│         │  CLAUDE.md · Skills · Hooks · Subagents · Plugins   │         │
│         │  Key Vault · Model Router · Cost Meter · Sandbox     │         │
│         └──────────────────────────────────────────────────────┘         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 4.1 Sidecar Architecture & Submodule Integration Strategy

AutoDev bundles **Core Orchestrator** and **Memory MCP Server** as managed sidecars rather than rebuilding their capabilities. This preserves upstream updates while delivering a single-installer desktop experience.

#### The Sidecar Pattern

```
AutoDev.app (or .exe)
├── autodev (Tauri/Rust binary)         ← UI + agent runtime
├── sidecars/
│   ├── core-orchestrator                  ← PyInstaller bundle of FastAPI orchestrator
│   ├── memory-mcp-server                  ← PyInstaller bundle of Memory MCP
│   └── redis-server                       ← Redis standalone binary (~5MB)
└── resources/
    └── [Python stdlib + deps bundled]
```

Tauri manages all sidecar lifecycles — the user never touches Python, Redis, or Docker.

#### Startup Sequence

When AutoDev launches:

1. **Check Redis** on `localhost:6379` → if not running, launch `sidecars/redis-server`
2. **Wait for Redis ready** (ping loop, max 3s timeout)
3. **Launch Core Orchestrator** on `localhost:8765`
   - Env vars: `REDIS_URL=localhost:6379`, `DATA_DIR=~/.airs/data`
4. **Launch Memory MCP Server** on `localhost:8766`
5. **Health check both** → `/health` endpoints, 10s timeout
6. **Signal frontend** → show main UI
7. **On app exit** → `SIGTERM` all sidecars → 3s grace → `SIGKILL` if needed

The user sees a 2-3 second loading screen on first launch. Subsequent launches are faster (Redis warm).

#### What We Keep vs Strip from Core Orchestrator

**Keep (the intelligence):**
- 21 finite state machines governing agent lifecycle
- A2A (Agent-to-Agent) protocol implementation
- SSE streaming engine → pipes to AutoDev chat UI
- MCP client manager with connection pooling
- Tool execution FSM tracking
- 3-tier memory system (hot/warm/cold)
- Episodic/semantic/procedural memory separation
- Context scoring and relevance ranking
- Memory algorithm system
- Feedback-driven synopsis evolution
- MCP Wizard logic (UI provided by AutoDev)

**Strip (server infrastructure, not needed locally):**
- Nginx reverse proxy
- Let's Encrypt / Certbot SSL
- S3 / presigned URLs → replace with local filesystem paths
- Docker Compose (Tauri replaces this as process manager)
- YAML auth + JWT sessions → desktop app IS the session
- OpenTelemetry → optional, debug builds only

**Adapt:**
- **Redis:** Ship as sidecar binary, runs on `localhost:6379`, zero config
- **FastAPI:** Runs on `localhost:8765`, AutoDev Tauri backend talks HTTP to it
- **mem0 graph store:** Requires local-only mode (verify no cloud dependency)

#### Dual-Mode Execution: When to Use Rust vs Core Orchestrator

> **Critical architectural pattern**: AutoDev has TWO execution modes. This explains why the Rust codebase implements provider abstraction and MCP client capabilities even though core-orchestrator already has these via `litellm` and `airs_mcp`.

**Mode 1: PA (Personal Assistant)** — Rust-native, lightweight
- **Who uses it:** Razi, team members, developers using the desktop app interactively
- **Execution:** Tauri Rust → Direct LLM API → Built-in tools (file I/O, git, MCP via `rmcp`)
- **Why NOT delegate:** Lower latency (no HTTP round-trip), works offline with Ollama, simpler deployment
- **Tradeoffs:** No A2A, no mem0 semantic memory, no 21 FSMs — intentionally simple

**PA Mode Toggle** (UI dropdown in ChatHeader):
PA mode has three sub-modes that control tool availability:

| Mode | Tools Available | Use Case |
|------|----------------|----------|
| **🔧 Fast Development** | Full suite: read_file, write_file, edit_file, bash, grep, glob, git | Razi doing direct coding, quick fixes, exploring code |
| **✋ Review Mode** | Same tools, but permission prompt before any write operation | Reviewing changes, cautious editing |
| **📋 Delegate Mode** | **Zero file tools**. Only: github_create_issue, airs_dispatch_signal | Workers are live — PA creates tasks and dispatches, doesn't touch files |

In **Delegate Mode**, the PA acts as a task creator:
1. User: "Add /health endpoint to orchestrator"
2. PA: Creates GitHub issue #142 with acceptance criteria via `github_create_issue`
3. PA: "Issue created. Dispatch to Worker Backend?"
4. User: "Yes"
5. PA: Writes `.autodev/dispatch/worker_backend_20260409_1642.signal` via `airs_dispatch_signal`
6. Worker Backend picks up signal and executes

This prevents the PA from accidentally editing files while workers are processing the task queue.

**Mode 2: Worker Agent** — Orchestrated via core-orchestrator
- **Who uses it:** Worker agents (backend, frontend, cleanup), Supervisor, QC running recipes
- **Execution:** Tauri → HTTP POST to core-orchestrator → LivingAgent (21 FSMs) → litellm → airs_mcp → mem0 → A2A
- **Why delegate:** Needs full intelligence layer (FSMs, memory, context scoring, A2A protocol)
- **When:** Recipe execution, multi-agent coordination, long-running autonomous tasks

**Result:** Zero duplication — PA mode uses lightweight Rust implementations; Worker mode leverages the full Python intelligence stack.

#### Git Submodule Strategy

Core Orchestrator and Memory MCP Server are tracked as **git submodules**, not copied code:

```
autodev/
├── src-tauri/                    ← Rust/Tauri desktop shell
├── src/                          ← React frontend (AutoDev UI)
├── submodules/
│   ├── core-orchestrator/        ← git submodule (AIRS-Foundry/core-orchestrator)
│   └── memory-mcp-server/        ← git submodule (AIRS-Foundry/memory-mcp-server)
├── build/
│   ├── bundle-core.sh            ← PyInstaller script for core-orchestrator
│   ├── bundle-memory-mcp.sh      ← PyInstaller script for memory-mcp-server
│   └── fetch-redis.sh            ← Downloads pre-built Redis binary for target OS
├── sidecars/                     ← Output of build scripts (gitignored)
│   ├── core-orchestrator         ← Compiled standalone binary
│   ├── memory-mcp-server         ← Compiled standalone binary
│   └── redis-server              ← Redis binary (win/mac/linux)
└── tauri.conf.json               ← Declares all sidecars
```

**Submodule tracking configuration** (`.gitmodules`):

```ini
[submodule "submodules/core-orchestrator"]
    path = submodules/core-orchestrator
    url = https://github.com/AIRS-Foundry/core-orchestrator.git
    branch = main

[submodule "submodules/memory-mcp-server"]
    path = submodules/memory-mcp-server
    url = https://github.com/AIRS-Foundry/memory-mcp-server.git
    branch = main
```

This ensures `git submodule update --remote` always pulls the latest `main` of both repos.

#### Tool Integration Architecture

```
┌─────────────────────────────────────────────────────┐
│         AutoDev (Tauri Frontend/UI)               │
└─────────────────────┬───────────────────────────────┘
                      │
        ┌─────────────┴─────────────┐
        │                           │
        ▼                           ▼
┌──────────────────┐      ┌──────────────────────┐
│  Code Edit Tools │      │  Core Orchestrator   │
│  (Tauri Rust)    │◄────►│   (Python FastAPI)   │
│                  │      │   localhost:8765     │
│ • edit_file      │      └──────────┬───────────┘
│ • read_file      │                 │
│ • apply_diff     │      ┌──────────▼───────────┐
│ • grep_search    │      │  MCP Integration     │
│ • list_dir       │      │  Layer               │
│ • run_command    │      │                      │
└────────┬─────────┘      │ • MCP client pool    │
         │                │ • Tool discovery     │
         │                │ • Health checks      │
         │                │ • FSM tracking       │
         │                └──────────┬───────────┘
         │                           │
         │                ┌──────────▼───────────┐
         │                │   Memory MCP Server  │
         │                │   localhost:8766     │
         │                │                      │
         │                │ • 3-tier storage     │
         │                │ • Context scoring    │
         │                │ • Synopsis evolution │
         └────────────────┴──────────────────────┘
                     Unified Tool Registry
```

### Tool Flow: Tauri → Core Orchestrator → LivingAgent → Execution

> **Complete data flow**: How AutoDev tools pass from Tauri to worker agents and execute

#### Step 1: Tool Definition in Tauri (PA Mode)

**PA mode tools** are defined in Rust and executed locally:
```rust
// src-tauri/src/agent/builtin_tools/
├── read_file.rs       → ToolSpec { name: "read_file", ... }
├── write_file.rs      → ToolSpec { name: "write_file", ... }
├── bash.rs            → ToolSpec { name: "bash", ... }
└── git.rs             → ToolSpec { name: "git_status", "git_commit", ... }

// These are passed directly to LLM in PA mode
llm.chat(messages, tools=[read_file, write_file, bash, git_status, ...])
```

#### Step 2: Tool Registration in Core Orchestrator (Worker Mode)

**Worker mode tools** come from three sources:

**A. Internal Tools** (defined in core-orchestrator):
```python
# src/internal_tools/__init__.py (import time)
from src.internal_tools.web import WEB_TOOLS
from src.internal_tools.flora import FLORA_TOOLS

_bridge = InternalToolBridge()
for _tool in [*WEB_TOOLS, *FLORA_TOOLS, ...]:
    _bridge.register(_tool)  # Stores by tool.name
```

**B. MCP Tools** (from external MCP servers):
```python
# src/airs/bridges/mcp_bridge.py
mcp_client.list_tools()  # Discover tools from GitHub MCP, Slack MCP, etc.
# Returns: [ToolSpec { name: "github_create_issue", ... }, ...]
```

**C. AutoDev Tools** (from Tauri via HTTP):
```python
# When Tauri sends POST /api/v1/agent/invoke
# Payload includes (with __autodev suffix for collision-free routing):
{
    "tools": [
        {"name": "read_file__autodev", "description": "...", "inputSchema": {...}},
        {"name": "write_file__autodev", "description": "...", "inputSchema": {...}},
        {"name": "bash__autodev", "description": "...", "inputSchema": {...}},
        {"name": "git_status__autodev", "description": "...", "inputSchema": {...}},
        ...
    ]
}
```

#### Step 3: Tool Registration in LivingAgent

**When worker recipe triggers:**
```python
# src/airs/agent/living_agent.py

class LivingAgent:
    def __init__(self, config: AgentConfig):
        # 1. Get internal tools from config
        internal_tool_ids = config.internal_tools  # ["web_search", "flora_tools"]
        internal_specs = internal_bridge.get_tools(internal_tool_ids)
        
        # 2. Get MCP tools from bridge
        mcp_specs = mcp_bridge.get_tools()
        
        # 3. Get AutoDev tools from request
        autodev_specs = request.tools  # Passed from Tauri
        
        # 4. Combine all tools
        all_tools = internal_specs + mcp_specs + autodev_specs
        
        # 5. Register with LLM
        self.llm.add_tools(all_tools)
        
        # 6. Register handlers with UnifiedToolHandler
        unified_handler.register_internal_handler(
            self.internal_bridge.handle_tool_call,
            tools=internal_specs
        )
        unified_handler.register_mcp_handler(
            self.mcp_bridge.handle_tool_call,
            tools=mcp_specs
        )
        unified_handler.register_autodev_handler(
            self.autodev_handler.handle_tool_call,
            tools=autodev_specs
        )
```

#### Step 4: LLM Calls Tool

```python
# LivingAgent.process_message()
response = await self.llm.complete(
    messages=conversation,
    tools=all_tools  # Includes internal + MCP + AutoDev
)

# LLM returns:
# {
#     "type": "tool_use",
#     "name": "read_file",  # or "web_search__internal" or "github_create_issue"
#     "arguments": {"path": "/src/main.py"}
# }
```

#### Step 5: Tool Execution via UnifiedToolHandler

```python
# src/airs/bridges/unified_tool_handler.py

class UnifiedToolHandler:
    async def handle_tool_call(self, tool_call: ToolCall) -> Dict[str, Any]:
        tool_name = tool_call.name
        
        # Route based on tool name suffix/prefix pattern
        if tool_name.endswith("__internal"):
            # Internal tool (e.g., "web_search__internal")
            return await self.internal_handler(tool_call)
        
        elif tool_name.endswith("__autodev"):
            # AutoDev tool (e.g., "read_file__autodev", "write_file__autodev")
            return await self.autodev_handler(tool_call)
        
        elif tool_name.startswith("github_") or tool_name.startswith("slack_"):
            # MCP tool (e.g., "github_create_issue", "slack_send_message")
            return await self.mcp_handler(tool_call)
        
        else:
            # Unknown tool
            return {"error": f"Unknown tool: {tool_name}"}
```

#### Step 6: Tool Execution Tracking

```python
# src/airs/execution/tool_executor.py

class MemoryAwareToolExecutor:
    async def execute(self, tool_call: ToolCall) -> Dict[str, Any]:
        # 1. Create FSM for this execution
        fsm = ToolExecutionFSM(tool_name=tool_call.name)
        
        # 2. Register in global registry (for observability)
        registry = ToolExecutionRegistry()
        registry.register(fsm, ToolExecutionInfo(...))
        
        # 3. Check memory: has this tool failed before?
        past_failures = await memory.retrieve(
            f"tool_failures:{tool_call.name}"
        )
        
        # 4. Execute via appropriate handler
        result = await unified_handler.handle_tool_call(tool_call)
        
        # 5. Store outcome in memory
        await memory.store(
            f"tool_execution:{tool_call.name}",
            {"success": result.get("error") is None, "result": result}
        )
        
        # 6. Update FSM state
        fsm.mark_complete()
        
        return result
```

#### Step 7: Result Back to LLM

```python
# LivingAgent continues conversation
messages.append({
    "role": "assistant",
    "content": [{"type": "tool_use", "name": "read_file", ...}]
})

messages.append({
    "role": "user",
    "content": [{"type": "tool_result", "tool_use_id": "...", "content": "file contents..."}]
})

# LLM sees the result and decides next action
response = await self.llm.complete(messages, tools=all_tools)
```

---

**Tool Routing Logic:**

- **Local tools** (PA mode, Tauri Rust native):
  - `edit_file`, `read_file`, `apply_diff`, `grep_search`, `run_command`, `git_*`
  - No HTTP overhead, direct filesystem access (Tauri sandboxed)

- **Internal tools** (Worker mode, in core-orchestrator):
  - `web_search__internal`, `flora_tools__internal`, etc.
  - Registered at import time, routed by `__internal` suffix
  - Opt-in per agent via YAML `internal_tools: [web_search]`

- **MCP tools** (Worker mode, external servers):
  - `github_create_issue`, `slack_send_message`, etc.
  - Discovered via MCP client, routed by prefix (github_, slack_)
  - Pooled connections, cached capabilities

- **AutoDev tools** (Worker mode, from Tauri):
  - `read_file__autodev`, `write_file__autodev`, `bash__autodev`, `git_status__autodev`, etc.
  - Passed via HTTP POST from Tauri to core-orchestrator with `__autodev` suffix
  - Routed by suffix, executed locally in core-orchestrator (not proxied back to Tauri)

**Agent execution loop** (Python pseudocode):

```python
async def process_message(self, user_message: str):
    # 1. Add user message to conversation
    messages.append({"role": "user", "content": user_message})
    
    # 2. Call LLM with all tools (internal + MCP + AutoDev)
    response = await llm.complete(messages, tools=all_tools)
    
    # 3. Stream response to UI via SSE
    for chunk in response.stream():
        await sse_broker.emit("agent:stream-token", chunk)
    
    # 4. If tool_use in response, execute it
    for tool_call in response.tool_calls:
        result = await tool_executor.execute(tool_call)
        messages.append({"role": "user", "content": [{"type": "tool_result", "content": result}]})
    
    # 5. Continue until no more tool calls
    # 6. Store final message in memory + SQLite
```

#### Editing Submodule Code

Submodules are independent git repos. To fix bugs or add features:

**Inside the submodule:**
```bash
cd submodules/core-orchestrator
git checkout main

# Make changes, test, commit
git add .
git commit -m "fix: implement stdio MCP transport"
git push origin main
```

**Back in parent repo:**
```bash
cd ../..
git add submodules/core-orchestrator
git commit -m "chore: bump core-orchestrator to include stdio fix"
git push
```

The parent repo tracks the **commit SHA** of the submodule. When you update and push, all clones pull the updated version.

#### Build & Release Pipeline

**Local development:**
1. Work in submodule → commit → push to upstream
2. Bump submodule pointer in `autodev`
3. Run `build/bundle-core.sh` → regenerates `sidecars/core-orchestrator`
4. Test locally

**CI/CD (GitHub Actions matrix):**
- **Windows runner:** Builds Windows sidecar binaries + Tauri `.exe`
- **macOS runner:** Builds macOS sidecar binaries + Tauri `.dmg`
- **Linux runner:** Builds Linux sidecar binaries + Tauri `.AppImage`

PyInstaller bundles are OS-specific and can't cross-compile. This is standard — Cursor, Ollama, Postgres.app all do the same.

**Update story:**
- New release → new PyInstaller build of updated submodule → bundled in Tauri update
- User clicks "Update AutoDev" → entire package updates (UI + sidecars)
- The submodule pin in `autodev` controls which commit of `core-orchestrator` each release ships

#### Tradeoffs & Constraints

**Installer size:** ~120-200MB (vs ~15MB pure Tauri)
- Acceptable — same size as Claude Desktop, VS Code
- PyInstaller bundles entire Python runtime per sidecar

**Startup time:** 2-3 seconds (Redis + Python sidecars)
- Fast enough for desktop app
- Subsequent launches faster (Redis already warm)

**Offline mode:** Limited
- Core Orch requires Redis → can't work fully offline
- Fallback: implement memory cache to local SQLite when Redis unavailable

**mem0 dependency:** Must verify local-only mode
- If mem0 requires cloud API, need self-hosted alternative
- If purely in-process, no issue

---

## 5. UI/UX Design Spec

### Design Language

- **Aesthetic:** Industrial-minimal. Dark backgrounds, sharp lines, monospace accents, status-board density. Think mission control meets terminal — professional, not playful.
- **Primary color:** `#0A0A0A` (near-black background)
- **Surface color:** `#111111` (panel surfaces)
- **Accent:** `#00FF88` (AIRS green — used for active status, actions, highlights)
- **Danger:** `#FF4444` (errors, QC rejections, blocked status)
- **Warning:** `#FFAA00` (in-progress, pending)
- **Text primary:** `#F0F0F0`
- **Text secondary:** `#666666`
- **Font — UI:** `IBM Plex Mono` for all monospace elements (agent names, issue numbers, tokens, cost) + `IBM Plex Sans` for prose
- **Border radius:** 2px (sharp, minimal rounding)
- **Borders:** 1px `#222222`

### Window Layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│  ● ● ●  AutoDev  v1.0.0                              [_] [□] [×]      │
├─────────────────────────────────────────────────────────────────────────┤
│ [Chat] [AutoDev] [IDE] [Recipes] [Settings]          🟢 claude-sonnet-4 │
│          [active tab underlined]                     $0.47 today        │
├────────────────────────────────────────────────────────────┬────────────┤
│                                                             │            │
│                   MAIN CONTENT AREA                        │  CONTEXT   │
│                   (changes per tab)                        │  SIDEBAR   │
│                                                             │            │
│                                                             │ CLAUDE.md  │
│                                                             │ Active MCP │
│                                                             │ Skills     │
│                                                             │ Hooks      │
│                                                             │            │
├────────────────────────────────────────────────────────────┴────────────┤
│ ▶ [______________________________________________________________] [Send]│
│   Attach · /skill · @agent · #recipe         Tokens: 12,450  Cost: $0.02│
└─────────────────────────────────────────────────────────────────────────┘
```

### Tab 1 — Chat (Primary)

The main conversational interface for Razi (PA mode) or any team member.

```
┌─── Chat ─────────────────────────────────────────────────────────────────┐
│                                                                            │
│  ┌─ AutoDev ─────────────────────────────────────────────────────────┐  │
│  │ Running as: Personal Assistant (PA) mode                            │  │
│  │ Model: claude-opus-4-6  |  MCP: github, slack  |  Skills: 4 loaded │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
│                                                                            │
│  [Message bubbles — user right, agent left]                               │
│                                                                            │
│  ┌─ AutoDev ───────────────────────────────────────────────────────┐    │
│  │ 🔧 Using tool: github_create_issue                                │    │
│  │ ├── title: "Add GET /agents/health endpoint"                      │    │
│  │ ├── labels: worker-backend, p2-medium, feature, status-ready      │    │
│  │ └── ✅ Issue #142 created                                          │    │
│  └───────────────────────────────────────────────────────────────────┘    │
│                                                                            │
│  ┌─ You ─────────────────────────────────────────────────────────────┐    │
│  │ Add a /health endpoint to the orchestrator that returns agent     │    │
│  │ status, last active time, and memory usage                        │    │
│  └───────────────────────────────────────────────────────────────────┘    │
│                                                                            │
│  ▶ [What do you want to build today?_____________________________] [⏎]   │
└────────────────────────────────────────────────────────────────────────────┘
```

### Tab 2 — AutoDev Panel

Live view of the entire AutoDev pipeline. The "mission control" tab.

```
┌─── AutoDev Panel ────────────────────────────────────────────────────────┐
│                                                                            │
│  AGENT ROSTER                                    [Refresh] [New Task]     │
│  ─────────────────────────────────────────────────────────────────────   │
│  ● SUPERVISOR      🟢 Active   Polling every 2m   0 tasks queued          │
│  ● WORKER-BACKEND  🟡 Working  Issue #142 · 00:14 elapsed                 │
│  ● WORKER-FRONTEND 🔵 Idle     Waiting for tasks                          │
│  ● WORKER-PLATFORM 🔵 Idle     Waiting for tasks                          │
│  ● WORKER-MCP      🔵 Idle     Waiting for tasks                          │
│  ● WORKER-CLEANUP  🟡 Working  Issue #138 · 01:42 elapsed                 │
│  ● QC AGENT        🟡 Working  Reviewing PR #87                           │
│  ─────────────────────────────────────────────────────────────────────   │
│                                                                            │
│  TASK BOARD (GitHub Issues → airs-task-board)                             │
│  ─────────────────────────────────────────────────────────────────────   │
│  READY (3)          IN PROGRESS (2)    QC REVIEW (1)    DONE TODAY (5)   │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐                  │
│  │ #145 p1      │   │ #142 backend │   │ PR #87       │                  │
│  │ Fix SSE hook │   │ /health ep   │   │ health ep    │                  │
│  │ frontend     │   │ ████░░ 60%   │   │ QC: 🟡       │                  │
│  ├──────────────┤   ├──────────────┤   └──────────────┘                  │
│  │ #146 p2      │   │ #138 cleanup │                                      │
│  │ Docstrings   │   │ Dead code    │   [Dispatch Signal ▼]               │
│  ├──────────────┤   │ ████████ 80% │                                      │
│  │ #147 p3      │   └──────────────┘                                      │
│  │ Update deps  │                                                          │
│  └──────────────┘                                                          │
│                                                                            │
│  COST TODAY: $2.14  |  TOKENS: 1.2M  |  TASKS CLOSED: 5  |  QC PASS: 80%│
└────────────────────────────────────────────────────────────────────────────┘
```

### Tab 3 — IDE (with Git Sub-Panel)

A lightweight read/edit view of the active codebase with integrated git operations.

```
┌─── IDE ──────────────────────────────────────────────────────────────────┐
│  [core-orchestrator ▼] [Clone Repo]  /src/api/health.py  [Edit] [Git ▼] │
│  ─────────────────────────────────────────────────────────────────────   │
│  ├─ 📁 src/                    │  1 │ from fastapi import APIRouter      │
│  │  ├─ 📁 api/                 │  2 │ from ..services import ...         │
│  │  │  ├─ health.py  M         │  3 │                                    │
│  │  │  └─ routes.py            │  4 │ router = APIRouter()               │
│  │  ├─ 📁 agents/              │  5 │                                    │
│  │  ├─ 📁 services/            │  6 │+ @router.get("/agents/health")    │
│  │  └─ main.py                 │  7 │+ async def agents_health():       │
│  ├─ 📁 tests/                  │  8 │+     """Return agent status."""   │
│  ├─ CLAUDE.md                  │  9 │+     svc = get_agent_service()    │
│  └─ pyproject.toml             │ 10 │+     return await svc.health()   │
│                                ├────┴──────────────────────────────────┤
│                                │ GIT PANEL (toggle via [Git ▼])        │
│                                │ Branch: worker/backend/142             │
│                                │ ┌ Staged ───────────────────────────┐ │
│                                │ │ (none)                             │ │
│                                │ ├ Unstaged ─────────────────────────┤ │
│                                │ │  M  src/api/health.py  (+5 -0)    │ │
│                                │ │  ?  src/api/new_file.py            │ │
│                                │ └───────────────────────────────────┘ │
│                                │ [Stage All] [Commit] [Push] [PR]     │
│  ─────────────────────────────────────────────────────────────────────   │
│  pytest: 142✅ 0❌  · mypy: clean · worktree: worker/backend/142         │
└──────────────────────────────────────────────────────────────────────────┘
```

Git is integrated as a **sub-panel within the IDE tab**, not a separate tab. The repo selector + "Clone Repo" button enables cloning AIRS Foundry repos directly, making AutoDev a complete single pane of glass.

### Tab 4 — Recipes

Create, browse, and run YAML workflow recipes.

```
┌─── Recipes ──────────────────────────────────────────────────────────────┐
│  [My Recipes] [AIRS Library] [Community]          [+ New Recipe]          │
│  ─────────────────────────────────────────────────────────────────────   │
│  AIRS LIBRARY                                                              │
│  ┌────────────────────────────────────┐  ┌──────────────────────────────┐ │
│  │ 🔧 worker-backend-poll             │  │ RECIPE DETAIL                │ │
│  │ Backend worker polling loop        │  │                              │ │
│  │ Schedule: */2 * * * *              │  │ worker-backend-poll          │ │
│  │ [Run Now] [Edit] [Schedule]        │  │                              │ │
│  ├────────────────────────────────────┤  │ Instructions:                │ │
│  │ 🔧 supervisor-dispatch             │  │ Poll GitHub for              │ │
│  │ Supervisor task routing loop       │  │ worker-backend issues,       │ │
│  │ Schedule: */2 * * * *              │  │ claim, execute, PR.          │ │
│  │ [Run Now] [Edit] [Schedule]        │  │                              │ │
│  ├────────────────────────────────────┤  │ Extensions:                  │ │
│  │ 🔍 qc-review-cycle                 │  │ • github-mcp                 │ │
│  │ QC agent PR review loop            │  │ • developer                  │ │
│  │ Schedule: */3 * * * *              │  │                              │ │
│  │ [Run Now] [Edit] [Schedule]        │  │ Parameters:                  │ │
│  ├────────────────────────────────────┤  │ • repo: string (required)    │ │
│  │ 📋 create-task-from-spec           │  │ • worker: string (required)  │ │
│  │ PA → GitHub Issue creation         │  │                              │ │
│  │ Manual only                        │  │ [▶ Run Now]  [Schedule]      │ │
│  └────────────────────────────────────┘  └──────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────┘
```

### Tab 5 — Settings

```
┌─── Settings ─────────────────────────────────────────────────────────────┐
│  [General] [Providers] [MCP Servers] [Permissions] [AIRS AutoDev]         │
│  ─────────────────────────────────────────────────────────────────────   │
│                                                                            │
│  PROVIDERS                                                                 │
│  ─────────────────────────────────────────────────────────────────────   │
│  Provider          API Key              Model               Status         │
│  Anthropic         sk-ant-●●●●●●●●     claude-sonnet-4-6   🟢 Active      │
│  OpenAI            [Add key...]         —                   ○  Inactive    │
│  Ollama            localhost:11434      —                   ○  Inactive    │
│  [+ Add Provider]                                                          │
│                                                                            │
│  MODEL ROUTER                                                              │
│  ─────────────────────────────────────────────────────────────────────   │
│  Agent Role          → Model                  Reason                       │
│  Supervisor          → claude-opus-4-6        Complex routing decisions    │
│  Worker Backend      → claude-sonnet-4-6      Balanced quality/cost        │
│  Worker Frontend     → claude-sonnet-4-6      Balanced quality/cost        │
│  Worker Platform     → claude-sonnet-4-6      MCP-heavy, mid complexity   │
│  Worker MCP Builder  → claude-opus-4-6        New repo scaffolding         │
│  Worker Cleanup      → claude-haiku-4-5       Simple refactors, cheap      │
│  QC Agent            → claude-opus-4-6        High-stakes review           │
│  [Edit routing rules]                                                      │
│                                                                            │
│  AIRS AUTODEV                                                              │
│  ─────────────────────────────────────────────────────────────────────   │
│  GitHub Org:          AIRS-Foundry                                         │
│  Task Board Repo:     airs-task-board                                      │
│  GitHub PAT:          ghp_●●●●●●●●●●●●  [Rotate]                         │
│  Agent identity:      [Supervisor ▼]  (this machine's role)               │
│  Lock file path:      current_tasks/                                       │
│  Dispatch watch dir:  dispatch/                                            │
└────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Core Features — Deep Spec

### 6.1 Agent Engine

The core execution loop. Implements:
- **Tool system:** Bash, ReadFile, WriteFile, EditFile, Glob, Grep, ListDirectory, LSP, WebFetch, GitWorktree
- **Context management:** CLAUDE.md injection on session start; auto-compaction at 80% context fill
- **Headless mode:** `airsctl -p "..." --skip-permissions` — drop-in replacement for `claude -p`
- **Permission modes:** Autonomous / SmartApproval / ManualApproval / ChatOnly
- **Checkpoint system:** Save/restore agent state mid-session
- **Working directory:** Per-session `working_dir` stored in SQLite; all tools resolve relative paths against it; changes when entering a git worktree

#### How Workers Modify the Codebase

Workers (and the PA) modify repos through **direct local filesystem writes + git worktree isolation**:

1. **File tools run in Rust natively** — `write_file`, `edit_file`, `read_file`, `bash`, `grep`, `glob`, `list_directory`. No subprocess overhead, direct filesystem access via Tauri sandbox.

2. **Git operations via `git2-rs`** (libgit2 bindings) — not shell-based like Claude Code/Goose. Provides:
   - Structured diff output (feeds IDE tab's syntax-highlighted diff view)
   - Branch/status/commit without subprocess overhead
   - Worktree lifecycle as first-class operations
   - Commit attribution: auto-appends `Generated-by: AutoDev/<agent-role>` trailer

3. **Worktree isolation per task** — the `worker/backend/<issue>` branching pattern maps to git worktrees:
   ```
   Agent claims task #142
     → git2::worktree::create("worker/backend/142")
     → Working copy at .autodev/worktrees/worker+backend+142/
     → Session working_dir switches to worktree path
     → Agent writes, tests, commits, pushes within isolation
     → Agent creates PR via GitHub MCP
     → On complete: worktree removed or kept for review
   ```

4. **File checkpointing** — snapshots of file state per message boundary (like Claude Code). Enables undo/redo and per-turn diff visualization in the IDE tab. Capped at 100 checkpoints per session.

### 6.2 CLAUDE.md / AGENTS.md Support

- Parse and inject CLAUDE.md from the following scopes (Claude Code precedence order):
  1. System-level (`~/.airs/CLAUDE.md`) — global AIRS conventions
  2. Repo-level (`{repo}/CLAUDE.md`) — per-repo agent instructions
  3. Directory-level (`.claude/CLAUDE.md`) — subdirectory-specific context
- Also parse `.goosehints` for Goose-compatible deployments
- **Live edit:** Changes to CLAUDE.md take effect on next session start without restart

### 6.3 Skills System

- Skills stored in `.airs/skills/` or `.claude/skills/` (both paths supported)
- Each skill = a directory with `SKILL.md` + optional helper scripts
- Frontmatter controls: auto-invoke, slash-command visibility, subagent execution
- AIRS built-in skills library (shipped with the app):
  - `/review-pr` — QC checklist run on a PR diff
  - `/run-tests` — Execute test suite, format results
  - `/write-spec` — Generate acceptance criteria from task description
  - `/claim-issue` — Write lock file + update GitHub label to in-progress
  - `/open-pr` — Create PR with standard AutoDev description template
  - `/check-dispatch` — Scan dispatch directory for signal files

### 6.4 Hooks System

Hooks fire at lifecycle events. Defined in `settings.json`:

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Write",
        "hooks": [{"type": "command", "command": "cd {repo} && uv run ruff check --fix"}]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{"type": "command", "command": "airsctl check-sandbox-violation '{command}'"}]
      }
    ],
    "SessionEnd": [
      {
        "matcher": "*",
        "hooks": [{"type": "http", "url": "https://airs.internal/agent-telemetry", "method": "POST"}]
      }
    ]
  }
}
```

AIRS-specific hooks shipped by default:
- **Post-Write:** Auto-lint Python (ruff) and TypeScript (eslint)
- **Post-Write:** Delete lock file if session ends cleanly
- **Pre-Bash:** Block `git push origin main` (enforce PR flow)
- **Session-End:** POST session cost + task summary to AIRS cost dashboard

### 6.5 Subagents + Agent Teams

- **Subagents:** Spawn isolated context loops, return summary. Used by Supervisor for epic decomposition.
- **Agent Teams:** Multiple AutoDev instances coordinate via GitHub Issues + dispatch signals (the existing AutoDev protocol maps directly to Agent Teams semantics)
- **@mention syntax:** In Supervisor sessions, `@worker-backend` sends a dispatch signal to Worker 1's recipe loop
- **Concurrency:** Up to 10 parallel subagents per session

### 6.6 Recipes Engine

YAML recipes are the primary workflow unit for AutoDev:

```yaml
# .airs/recipes/worker-backend-poll.yaml
version: "1.0.0"
title: Worker Backend — Polling Loop
description: Poll GitHub for worker-backend tasks, claim, implement, PR
schedule: "*/2 * * * *"
permission_mode: autonomous

parameters:
  - name: repo
    type: string
    required: true
    default: "core-orchestrator"

instructions: |
  You are the AIRS Backend Worker. Follow your CLAUDE.md exactly.
  1. Check dispatch/ for signal files first.
  2. Poll GitHub Issues: filter worker-backend + status-ready.
  3. If found: claim, branch, implement, test, PR.
  4. If nothing: exit cleanly.

extensions:
  - github-mcp
  - developer

sub_recipes:
  - source: check-dispatch.yaml
    priority: true
```

### 6.7 MCP Client + Server

- **Client:** Connect to any MCP server (GitHub, Slack, Sentry, custom)
- **Server:** AutoDev can itself act as an MCP server, exposing AIRS-specific tools to other agents
- **AIRS MCP Tools (built-in server):**
  - `airs_get_tasks(worker, status)` → Current GitHub Issues filtered by worker
  - `airs_claim_task(issue_number, worker)` → Write lock file + update label
  - `airs_complete_task(issue_number, pr_url)` → Delete lock file + update label
  - `airs_dispatch_signal(worker, issue_number)` → Write dispatch signal file
  - `airs_agent_status()` → Return all agents' current task and last heartbeat

### 6.8 Model Router

The Model Router selects the appropriate provider + model per session based on:
- **Agent role** (configured in Settings → AIRS AutoDev)
- **Task priority** (p0-critical → always Opus; p3-low → always Haiku)
- **Cost budget** (if daily spend exceeds threshold, downgrade model tier)

```
Role: worker-cleanup
Priority: p3-low
Budget: under daily limit
→ Route to: claude-haiku-4-5-20251001 (cheapest, fast, sufficient for refactors)

Role: supervisor  
Priority: p0-critical epic decomposition
Budget: any
→ Route to: claude-opus-4-6 (best reasoning for architectural decisions)
```

### 6.9 Key Vault

- All API keys encrypted at rest using OS keychain (Windows Credential Manager / macOS Keychain / Linux libsecret)
- Keys never written to plain text config files
- Rotation reminders: 90-day PAT expiry warning in UI
- Keys are scoped: GitHub PAT per agent role, API key per provider

### 6.10 IDE Tab (with Git Sub-Panel)

- **File tree:** Browse any repo cloned on this machine
- **Code viewer:** Syntax highlighting, monospace, read-only by default
- **Edit mode:** Toggle for Razi's PA machine — full edit with save + auto-commit option
- **Git status:** Show branch, changed files, test status inline
- **Worker context:** When a worker has a task active, IDE auto-opens the relevant branch and files being modified
- **Repo selector:** Dropdown of locally cloned repos; switch between multiple AIRS Foundry repos
- **Clone Repo button:** Lists repos from AIRS-Foundry GitHub org (using PAT from Settings), clone with one click
- **Git sub-panel** (togglable): Embedded in the IDE tab, provides:
  - Branch switcher (view, switch, create)
  - Staged/unstaged changes view with inline diff stats
  - Stage/unstage individual files or all
  - Commit with message (auto-attribution trailer: `Generated-by: AutoDev/<agent-role>`)
  - Push current branch to origin
  - Create PR via GitHub MCP (one-click)
  - Worktree manager: list active worktrees, which agent owns them, navigate
  - Full file diff viewer with syntax highlighting

> **Design decision**: Git is integrated as a sub-panel within the IDE tab, NOT as a separate 6th tab. The 5-tab structure (Chat, AutoDev, IDE, Recipes, Settings) maps to distinct workflows. Git is part of the IDE workflow. VS Code, JetBrains, and all major IDEs integrate git this way. The "Clone Repo" capability makes AutoDev a complete "single pane of glass" — clone, browse, edit, commit, push, PR — without leaving the app.

---

## 7. Tech Stack Decision

| Layer | Technology | Rationale |
|---|---|---|
| Desktop shell | **Tauri v2** (Rust + WebView) | Cross-platform (Windows/Mac/Linux), lightweight binary (~15MB vs Electron's 150MB+), Rust memory safety, Goose itself uses Rust |
| UI framework | **React + TypeScript** | Team familiarity, fast iteration, same stack as AIRS frontend |
| UI styling | **Tailwind CSS** | Utility-first, consistent with AIRS frontend patterns |
| Agent engine | **TypeScript** (compiled to WASM or Node sidecar) | Leverage Claude Code leak learnings; proven architecture; can port Goose recipe engine |
| CLI interface | **Rust** (`clap` crate) | `airsctl` — fast, single binary, same process as Tauri |
| MCP client | **TypeScript MCP SDK** (`@modelcontextprotocol/sdk`) | Official SDK, most mature |
| Database | **SQLite** (via `rusqlite`) | Local session history, cost tracking, recipe state |
| Key vault | **OS keychain** (`keytar` for Node / `keyring` for Rust) | Secure, no plaintext keys |
| YAML parser | **`serde_yaml`** (Rust) for recipes | Fast, type-safe |
| Git | **`git2-rs`** (libgit2 bindings) | Worktree creation, branch management, PR prep |

---

## 8. File & Config Structure

```
~/.airs/
├── config.yaml              # Global AutoDev config (providers, model router rules)
├── CLAUDE.md                # System-level CLAUDE.md (applied to all sessions)
├── skills/                  # Global skills library
│   ├── review-pr/
│   ├── run-tests/
│   ├── write-spec/
│   ├── claim-issue/
│   ├── open-pr/
│   └── check-dispatch/
├── recipes/                 # Global recipes
│   ├── worker-backend-poll.yaml
│   ├── worker-frontend-poll.yaml
│   ├── worker-platform-poll.yaml
│   ├── worker-mcp-poll.yaml
│   ├── worker-cleanup-poll.yaml
│   ├── supervisor-dispatch.yaml
│   └── qc-review-cycle.yaml
├── plugins/                 # Installed plugins
│   └── airs-autodev/        # The official AIRS plugin
├── sessions/                # Session history (SQLite)
│   └── sessions.db
└── logs/
    └── autodev.log
```

**Per-repo (in addition to existing CLAUDE.md):**
```
{repo}/
├── CLAUDE.md               # Existing worker instructions (unchanged)
├── .goosehints             # Optional: Goose-compatible hints
├── .airs/
│   ├── hooks.json          # Repo-level hooks
│   └── skills/             # Repo-specific skills
├── current_tasks/          # Lock files (existing)
│   └── .gitkeep
└── dispatch/               # Dispatch signals (existing)
    └── .gitkeep
```

---

## 9. AutoDev Integration Layer

AutoDev is not just a generic agent tool — it has native AutoDev support baked into the core:

### AutoDev Mode

When a machine is configured as an AutoDev agent (Settings → AIRS AutoDev → Agent identity), AutoDev:
1. **Auto-starts** the appropriate recipe on boot
2. **Watches** the dispatch directory for signal files in real time (inotify/FSEvents, not polling)
3. **Displays** its current task in the Agent Roster on all other AutoDev instances connected to the same org
4. **Reports** heartbeats every 5 minutes to a shared GitHub status file (or lightweight AIRS status endpoint)
5. **Auto-resets** stale lock files after 4 hours (Supervisor behavior)

### GitHub Issues as the Command Layer

AutoDev has a native GitHub Issues widget in the AutoDev Panel that:
- Reads issues without leaving the app
- Creates new issues via the PA interface
- Displays the full kanban board live
- Lets Razi send a dispatch signal with one click

### Cost Dashboard

Every session logs:
- Agent role, model used, task (issue number), tokens in/out, cache hits, total cost
- All data stored in local SQLite, queryable via `airsctl cost report --since 7d`
- AutoDev Panel shows: daily total, per-agent totals, 30-day trend
- Export to CSV for feeding the financial model

---

## 10. Build Prompt — Start Coding

Copy this prompt into your AutoDev session (ironic, building AutoDev with AutoDev) or any capable coding agent to begin the build:

---

```
You are building AutoDev — a cross-platform desktop + CLI AI agent application 
for AIRS Foundry. This is a Tauri v2 application (Rust backend, React/TypeScript 
frontend) that combines the best features of Anthropic's Claude Code and Block's 
Goose into a branded, model-agnostic AI developer agent platform.

## Repository Setup
Create a new repository called `autodev` under the AIRS-Foundry GitHub org with 
the following structure:

```
autodev/
├── src-tauri/           # Rust Tauri backend
│   ├── src/
│   │   ├── main.rs      # Tauri entry point
│   │   ├── agent/       # Core agent engine (tool execution, context management)
│   │   ├── mcp/         # MCP client + AIRS MCP server
│   │   ├── recipes/     # YAML recipe engine + cron scheduler
│   │   ├── vault/       # OS keychain integration
│   │   ├── git/         # git2-rs: clone, branch, diff, status, commit, push, worktree, checkpoint
│   │   ├── github/      # GitHub Issues + labels + PRs integration
│   │   └── autodev/     # AutoDev-specific logic (lock files, dispatch, heartbeat)
├── src/                 # React/TypeScript frontend
│   ├── App.tsx
│   ├── tabs/
│   │   ├── Chat.tsx         # Main chat interface
│   │   ├── AutoDevPanel.tsx # Agent roster + task board
│   │   ├── IDE.tsx          # File tree + code viewer + git sub-panel
│   │   ├── Recipes.tsx      # Recipe browser + runner
│   │   └── Settings.tsx     # Provider + model router + AutoDev config
│   ├── components/
│   │   ├── AgentStatus.tsx
│   │   ├── TaskCard.tsx
│   │   ├── CostMeter.tsx
│   │   ├── MessageBubble.tsx
│   │   ├── ToolCallDisplay.tsx
│   │   └── ContextSidebar.tsx
│   └── styles/
│       └── theme.css        # Dark industrial design tokens
├── src-cli/             # airsctl CLI (Rust, separate binary)
│   └── main.rs
├── claude.md            # This repo's own CLAUDE.md
└── AGENTS.md            # Goose-compatible equivalent
```

## Phase 1 — What to Build First

Focus on these four components in order:

### 1. Tauri App Shell + Navigation
- Window with title bar, tab navigation: Chat / AutoDev / IDE / Recipes / Settings
- Dark theme: background #0A0A0A, surface #111111, accent #00FF88
- IBM Plex Mono for code elements, IBM Plex Sans for prose
- Status bar: current model name, today's cost display
- Responsive to window resize; minimum 1000×700

### 2. Provider Layer (Settings → Providers tab)
- Support providers: Anthropic, OpenAI, Gemini, Ollama (local), Azure OpenAI
- API key input → store in OS keychain via `keytar` (never in config files)
- Test connection button: send a minimal completion request, show latency + success
- Default model selector per provider
- Active provider badge in status bar

### 3. Chat Tab — Core Agent Loop
Implement the agent execution loop:
a. User types a message → append to conversation
b. Build request: system prompt (CLAUDE.md content) + conversation history + user message
c. Send to selected provider via streaming API
d. Stream response tokens into the UI in real time
e. Parse tool calls from the stream — display as collapsible tool call blocks:
   - Tool name + arguments (formatted JSON)
   - Tool result (truncated if >500 chars, with expand button)
   - Success/error indicator
f. Loop: tool result → next model call → until no more tool calls → final response
g. Display final response as a styled message bubble

Built-in tools to implement first:
- `read_file(path)` — read a local file
- `write_file(path, content)` — write to a local file
- `bash(command)` — execute a shell command, capture stdout/stderr
- `list_directory(path)` — list files and subdirectories

Permission mode: implement a simple allow/deny dialog for bash commands in 
ManualApproval mode. In Autonomous mode, all tools execute without confirmation.

### 4. Settings Tab — AIRS AutoDev Config
Fields:
- GitHub Org (text input)
- Task Board Repo (text input, default: airs-task-board)
- GitHub PAT (password input → keychain)
- Agent identity (dropdown: Personal Assistant / Supervisor / Worker Backend / Worker Frontend / Worker Platform / Worker MCP Builder / Worker Cleanup / QC Agent)
- Lock file path (default: current_tasks/)
- Dispatch watch dir (default: dispatch/)
- Model Router table: editable rows of (Agent Role → Provider → Model)

## Code Style + Conventions

### Rust (src-tauri)
- Use `anyhow` for error handling
- Async with Tokio
- Tauri commands exposed via `#[tauri::command]`
- All file I/O through the `agent::filesystem` module (not raw std::fs)
- Log with `tracing` crate

### TypeScript/React (src/)
- Functional components only
- State management: Zustand for global state (sessions, config, agent status)
- API calls to Tauri backend via `@tauri-apps/api/core` invoke()
- No useEffect for data fetching — use Zustand actions
- Tailwind for all styling, no CSS-in-JS

### Design System
Apply the AutoDev design language:
- Background: #0A0A0A
- Surface: #111111  
- Border: #222222
- Accent: #00FF88 (active states, CTAs, live indicators)
- Danger: #FF4444 (errors, rejections)
- Warning: #FFAA00 (in-progress, pending)
- Text primary: #F0F0F0
- Text secondary: #666666
- Font: IBM Plex Mono (code, numbers, agent names) + IBM Plex Sans (prose)
- Border radius: 2px
- Spacing: 8px base unit

## What NOT to Do
- Do not use Electron — Tauri only
- Do not store API keys in config.yaml or any plain text file
- Do not hardcode model names — always read from the provider config
- Do not build the IDE tab yet (Phase 2)
- Do not build the Recipes engine yet (Phase 2)
- Do not add authentication — AutoDev is a local app, no cloud login

## Deliverables for Phase 1
1. Working Tauri app that opens with the 5-tab layout
2. Settings tab: add/test/save Anthropic API key
3. Chat tab: send a message, get a streaming response with tool call display
4. Tool execution: read_file, write_file, bash, list_directory working in ManualApproval mode
5. CLAUDE.md injection: on session start, read ~/.airs/CLAUDE.md + {cwd}/CLAUDE.md and prepend to system prompt
6. Status bar: show active model + today's token cost (tracked in local SQLite)

Start with `src-tauri/src/main.rs` and the Tauri app scaffold. Then move to 
the React shell. Build the Provider layer before the Chat tab.
```

---

## 11. Phased Build Plan

| Phase | Duration | Deliverables |
|---|---|---|
| **Phase 1** | Days 1–3 | App shell, Provider layer, Chat tab with tool execution, CLAUDE.md injection, Settings + AIRS AutoDev config, Cost tracking |
| **Phase 2** | Days 4–6 | Recipes engine + YAML parser + cron scheduler, AutoDev Panel (agent roster + task board), GitHub Issues integration |
| **Phase 3** | Days 7–9 | IDE tab (file tree + viewer), Skills system, Hooks system, AIRS MCP server (airs_get_tasks, airs_claim_task, etc.) |
| **Phase 4** | Days 10–12 | Subagents + Agent Teams, Model Router, Dispatch Signal UI, AutoDev native mode (boot-start recipe, FSEvents dispatch watch) |
| **Phase 5** | Days 13–14 | AIRS Plugin (one-click worker PC configuration), Headless CLI (`airsctl`), installers for Windows/Mac/Linux, IT staff runbook |

### Definition of Done
AutoDev is production-ready when:
- Any AIRS worker PC can be configured by IT staff in under 10 minutes by installing the app and entering a GitHub PAT
- The AutoDev polling loops run as Recipes with zero external scripting
- All API keys are stored in the OS keychain — no `.env` files on worker machines
- The cost dashboard shows per-agent, per-task token spend
- Razi can speak to the PA tab on his machine and watch the task move through the pipeline to completion on the AutoDev Panel — without opening GitHub

---

*This document is the ground truth for AutoDev. Update after each phase completes.*
