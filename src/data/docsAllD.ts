// Documentation — Sections 12–14
// Full content faithful to AUTODEV_DOCUMENTATION.md

import type { DocSection } from './docs'

export const D_SECTIONS: DocSection[] = [

  /* ── 12: Technical Internals ────────────────────────────────────── */
  {
    id: 'internals', num: '12', title: 'Technical Internals', category: 'Internals', icon: 'cpu',
    lead: 'Deep-dive into the Rust backend systems: cron scheduler, recipe runner, sidecar management, fleet management, orphan detection, tool execution, context building, and memory integration.',
    anchors: ['Cron Scheduler', 'Recipe Runner', 'Sidecar Management', 'Fleet Management', 'Orphan Detection', 'Tool Execution', 'Context Building', 'Memory Integration'],
    blocks: [
      { t: 'h2', text: 'Cron Scheduler' },
      { t: 'prose', text: 'Implementation: src-tauri/src/recipes/scheduler.rs' },
      { t: 'list', items: [
        'Uses tokio tasks for background execution',
        'cron crate for POSIX cron expression parsing',
        'Arc<RecipeScheduler> stored in Tauri managed state (lives for app lifetime)',
        'HashMap<String, JoinHandle<()>> maps worker_slug → running task',
      ]},
      { t: 'h3', text: 'Startup Sequence' },
      { t: 'list', items: [
        '1. Wait for core-orchestrator to be ready (polls every 2s for up to 120s)',
        '2. Fire immediate startup tick (regardless of cron schedule)',
        '3. If startup was Mode 1 (orch not ready), fire Mode 2 catch-up tick when orch becomes ready',
        '4. Enter cron loop',
      ]},
      { t: 'prose', text: 'Mode 2 Catch-Up Tick: handles race where some workers start before orchestrator. Ensures all workers get Mode 2 ticks and create their YAML configs in core-orchestrator. Prevents the "missing from fleet" issue.' },
      { t: 'h2', text: 'Recipe Runner' },
      { t: 'prose', text: 'Implementation: src-tauri/src/recipes/runner.rs' },
      { t: 'code', lang: 'text', code: `Dispatch Decision:
Health check → core-orchestrator reachable?
    ↓ Yes → Mode 2 (LivingAgent via HTTP POST /api/v1/agent/invoke)
    ↓ No  → Mode 1 (Rust AgentEngine, direct LLM API)` },
      { t: 'list', items: [
        'No-Op Short-Circuit (Mode 1 only): if worker has no assigned/in_progress tasks, skip tick. Prevents LLM from hallucinating work on empty board. NOT applied in Mode 2 (always dispatch to ensure_agent() runs).',
        'Model Resolution priority: worker.model (DB) > recipe.model (YAML) > active_model (Settings) > default',
        'Workspace Directory priority: configured workspace > sandbox fallback (~/.autodev/agents/<slug>/)',
        'Ephemeral Sessions: each tick creates fresh session ID, not persisted to session DB',
      ]},
      { t: 'h2', text: 'Sidecar Management' },
      { t: 'prose', text: 'Implementation: src-tauri/src/sidecar/mod.rs' },
      { t: 'h3', text: 'Startup Sequence' },
      { t: 'list', items: [
        '1. Start Redis (if not already running on port 6379)',
        '2. Start Memory MCP Server (waits for log pattern "Qdrant" or "Using Memory")',
        '3. Start Core Orchestrator (waits for log patterns confirming ready)',
        '4. Emit startup progress events to UI',
      ]},
      { t: 'h3', text: 'Startup Progress FSM (12 phases)' },
      { t: 'code', lang: 'text', code: `Init → Skills → MemoryStarting → VectorDb → MemoryReady
→ OrchStarting → Routers → Agents → LivingAgents
→ OrchReady → Workers → Ready` },
      { t: 'h3', text: 'Health Checks' },
      { t: 'list', items: [
        'HTTP GET /health on core-orchestrator',
        'TCP connection test on memory-mcp-server',
        'Process existence check for Redis',
      ]},
      { t: 'h3', text: 'Log Router' },
      { t: 'prose', text: 'Captures stdout/stderr from all sidecars, parses log patterns to detect startup phases, emits Tauri events for UI progress updates.' },
      { t: 'h2', text: 'Fleet Management' },
      { t: 'prose', text: 'Worker Registry: ~/.autodev/workers.db (SQLite). Seeds 7 system workers on first open. CRUD via Tauri commands.' },
      { t: 'table', headers: ['Worker Status', 'Meaning'], rows: [
        ['Created', 'Seeded in DB, not yet started'],
        ['Idle', 'Recipe cron active, polling, no task currently'],
        ['Active', 'Recipe cron loop running'],
        ['Working', 'Actively executing a task'],
        ['Paused', 'User manually paused'],
        ['Error', 'Recipe crashed or errored'],
        ['Deleted', 'Soft-deleted, no longer active'],
      ]},
      { t: 'h3', text: 'Task Assignment Flow' },
      { t: 'list', items: [
        'Supervisor recipe polls task board, calls assign_task to assign a worker',
        'Worker recipe polls for tasks assigned to it',
        'Worker claims task by writing a lock file (current_tasks/issue-N.lock)',
        'Worker communicates via worker_requests table using request_supervisor_action tool',
      ]},
      { t: 'h2', text: 'Orphan Detection' },
      { t: 'prose', text: 'Implementation: src-tauri/src/orphan/mod.rs. Runs automatically before each Supervisor tick. Also available as a Tauri command.' },
      { t: 'list', items: [
        'Detection: query tasks with status in_progress, check if lock file exists for each',
        'If no lock file → task is orphaned (worker crashed)',
        'Resolution: release task back to assigned status',
        'Emits orphans_released event → OrphanAlert widget shows list of released tasks',
      ]},
      { t: 'h2', text: 'Tool Execution' },
      { t: 'prose', text: 'Implementation: src-tauri/src/agent/tool_executor.rs' },
      { t: 'table', headers: ['Execution Type', 'Tools', 'Concurrency'], rows: [
        ['Read-only', 'read_file, grep, glob, list_directory', 'Concurrent (max 10 via Semaphore)'],
        ['Write', 'write_file, edit_file, bash', 'Serial (one at a time)'],
      ]},
      { t: 'table', headers: ['Permission Mode', 'Behavior'], rows: [
        ['Autonomous', 'No prompts — used for worker agents in fleet'],
        ['Manual Approval', 'Prompt user for every write operation'],
        ['Smart Approve', 'Auto-approve read-only, prompt for write operations'],
      ]},
      { t: 'h3', text: 'Tool Registry' },
      { t: 'list', items: [
        'Filters tools based on PA mode (Delegate: only create_task + MCP read-only)',
        'Fast Development: all builtin_tools + all MCP tools',
        'Review: all tools + permission wrapper',
        'Workers (Mode 2): tools provided by core-orchestrator AIRS MCP server',
      ]},
      { t: 'h2', text: 'Context Building' },
      { t: 'prose', text: 'Implementation: src-tauri/src/agent/context.rs' },
      { t: 'h3', text: 'System Prompt Assembly Order' },
      { t: 'list', items: [
        '1. Base system prompt (mode-specific: BASE/REVIEW/DELEGATE)',
        '2. Global AUTODEV.md (from SQLite primary, filesystem fallback)',
        '3. Repo-specific AUTODEV.md (from SQLite or .autodev/autodev.md in workspace)',
        '4. Memory context (if session_id attached to living agent)',
      ]},
      { t: 'prose', text: 'AUTODEV.md Storage: Primary = SQLite (~/.autodev/config.db). Fallback = ~/.autodev/config/autodev_global.md or .autodev/autodev.md. UI-managed via AUTODEV.md editor in Settings.' },
      { t: 'h2', text: 'Memory Integration' },
      { t: 'prose', text: 'Implementation: src-tauri/src/memory/' },
      { t: 'table', headers: ['Tier', 'Type', 'Description'], rows: [
        ['Tier 1', 'Episodic', 'Conversation history, tool executions'],
        ['Tier 2', 'Semantic', 'Vector embeddings, retrieval-augmented generation'],
        ['Tier 3', 'Procedural', 'Skills, recipes, workflows'],
      ]},
      { t: 'list', items: [
        'build_memory_context() — recalls relevant memories with token-aware budget management',
        'Intent detection for targeted recall (retrieves relevant context only)',
        'Memory Providers: mem0 (primary), Redis (warm cache), MongoDB, ChromaDB, Vertex AI (alternatives)',
      ]},
    ],
  },

  /* ── 13: Submodules ─────────────────────────────────────────────── */
  {
    id: 'submodules', num: '13', title: 'Submodules', category: 'Internals', icon: 'git-branch',
    lead: 'AutoDev uses git submodules for the Python sidecar components (core-orchestrator and memory-mcp-server). This allows independent development and versioning of the orchestrator and memory systems.',
    anchors: ['Structure', 'Core Orchestrator', 'Memory MCP Server', 'Submodule Workflow', 'Submodule Status', 'Troubleshooting'],
    blocks: [
      { t: 'h2', text: 'Submodule Structure' },
      { t: 'code', lang: 'text', code: `autodev/
├── .gitmodules              # Submodule configuration (branch tracking → main)
├── submodules/
│   ├── core-orchestrator/   # Python agent orchestration (port 4000)
│   └── memory-mcp-server/   # Memory MCP server (port 5000)` },
      { t: 'h2', text: 'Core Orchestrator' },
      { t: 'prose', text: 'Python-based agent runtime providing LivingAgent, FSMs, A2A protocol. Runtime: Python 3.13+ / FastAPI / uvicorn on port 4000.' },
      { t: 'table', headers: ['File', 'Size', 'Description'], rows: [
        ['src/airs/agent/living_agent.py', '2425 lines', 'LivingAgent — central agent class with 21 FSMs'],
        ['src/airs/memory/memory_core.py', '2654 lines', 'MemoryCore — 3-tier memory management'],
        ['src/airs/state/', '21 files', 'Finite state machines for lifecycle management'],
        ['src/airs/bridges/', 'N files', 'Unified tool handler, MCP bridge, memory compiler'],
        ['src/airs/mcp/', 'N files', 'MCP client system for external tool servers'],
      ]},
      { t: 'h2', text: 'Memory MCP Server' },
      { t: 'prose', text: 'FastMCP-based server exposing 6 memory tools. Runtime: Python 3.13+ / FastMCP / Starlette on port 5000. Circuit breaker pattern with automatic failover to alternative memory providers.' },
      { t: 'table', headers: ['File', 'Size', 'Description'], rows: [
        ['src/mcp_server/server.py', '1093 lines', 'FastMCP server with 6 memory tools'],
        ['src/memory_providers/', 'N files', 'Multi-provider memory manager (mem0/Redis/Qdrant)'],
      ]},
      { t: 'h2', text: 'Submodule Workflow' },
      { t: 'h3', text: 'Editing a Submodule' },
      { t: 'code', lang: 'bash', code: `cd submodules/core-orchestrator
git checkout main
git pull origin main
git checkout -b fix/your-feature
# Make changes, test locally
git add .
git commit -m "fix: your changes"
git push origin fix/your-feature` },
      { t: 'h3', text: 'Updating Pointer in Parent Repo' },
      { t: 'code', lang: 'bash', code: `cd ../..   # back to autodev root
git add submodules/core-orchestrator
git commit -m "chore: bump core-orchestrator (fix included)"` },
      { t: 'h3', text: 'Updating to Latest' },
      { t: 'code', lang: 'bash', code: `git submodule update --remote --merge` },
      { t: 'callout', text: 'Important: Submodules are separate git repos with their own history. Parent repo tracks commit SHA pointers, NOT file contents. Always commit in the submodule first, then update the pointer in the parent. Do NOT run uv sync in the main autodev repo — it is not a Python project. Only run uv sync inside submodules when developing them locally.' },
      { t: 'h2', text: 'Submodule Status' },
      { t: 'code', lang: 'bash', code: `git submodule status
# Output: [+/-/U] <SHA> submodules/<name> (<branch>)
# + = different from expected commit
# U = unmerged conflicts` },
      { t: 'h2', text: 'Troubleshooting' },
      { t: 'table', headers: ['Problem', 'Fix'], rows: [
        ['Detached HEAD', 'cd submodules/core-orchestrator && git checkout main'],
        ['Submodule not initialized', 'git submodule update --init --recursive'],
        ['Wrong commit / out of sync', 'cd submodules/core-orchestrator && git fetch origin && git checkout origin/main && cd ../.. && git add submodules/core-orchestrator && git commit -m "chore: reset submodule to main"'],
      ]},
    ],
  },

  /* ── 14: Glossary ───────────────────────────────────────────────── */
  {
    id: 'glossary', num: '14', title: 'Glossary', category: 'Internals', icon: 'book-open',
    lead: 'Complete reference of terms, acronyms, and concepts used throughout AutoDev documentation and codebase.',
    anchors: ['A–D', 'E–H', 'I–M', 'N–R', 'S–Z'],
    blocks: [
      { t: 'h2', text: 'A–D' },
      { t: 'table', headers: ['Term', 'Definition'], rows: [
        ['A2A Protocol', 'Agent-to-Agent communication protocol used by core-orchestrator for supervisor ↔ worker coordination'],
        ['AUTODEV.md', 'Proprietary AIRS instruction system, stored in SQLite, editable from UI, injected into system prompt'],
        ['Bash Tool', 'Built-in tool for executing shell commands (tests, linting, git operations)'],
        ['Class Node', 'Graph node representing a class definition'],
        ['Community', 'Group of related nodes in the knowledge graph (clustered via Leiden algorithm)'],
        ['Core Orchestrator', 'Python sidecar providing LivingAgent, FSMs, A2A protocol (port 4000)'],
        ['Delegate Mode', 'PA mode where agent creates tasks instead of executing code directly (workers are live)'],
      ]},
      { t: 'h2', text: 'E–H' },
      { t: 'table', headers: ['Term', 'Definition'], rows: [
        ['Edit File Tool', 'Built-in tool for targeted file edits (preferred over write_file for modifications)'],
        ['Fast Development Mode', 'PA mode with full tool access and no permission prompts'],
        ['File Node', 'Graph node representing a source file'],
        ['Force-Directed Graph', 'D3.js simulation where nodes repel each other and edges pull connected nodes together'],
        ['FSM', 'Finite State Machine — core-orchestrator has 21 FSMs governing agent lifecycle'],
        ['Git Worktree', 'Isolated working directory per task — prevents main branch pollution during worker execution'],
        ['GitHub MCP', 'MCP server providing GitHub Issues and Pull Request tools'],
        ['Glob Tool', 'Built-in tool for finding files matching glob patterns'],
        ['Grep Tool', 'Built-in tool for searching file contents with regex patterns'],
        ['Hook', 'Script or action that fires at lifecycle events (pre/post tool call, session start/end, git ops, etc.)'],
      ]},
      { t: 'h2', text: 'I–M' },
      { t: 'table', headers: ['Term', 'Definition'], rows: [
        ['Internal Task Board', 'SQLite database (~/.autodev/tasks.db) for autonomous task assignment between agents'],
        ['Leiden Algorithm', 'Graph clustering algorithm used to detect communities in the code knowledge graph'],
        ['LivingAgent', 'Core orchestrator\'s central agent class with 21 FSMs, mem0 memory, and A2A integration'],
        ['Lock File', 'current_tasks/issue-N.lock — worker writes this before starting a task to prevent duplicate work'],
        ['MCP', 'Model Context Protocol — open standard for connecting AI tools to external services and data'],
        ['Memory MCP Server', 'Python sidecar providing 6 memory tools via FastMCP protocol (port 5000)'],
        ['Memory Core', '3-tier memory system: episodic (history), semantic (vector embeddings), procedural (skills/recipes)'],
        ['Mode 1', 'Rust-native execution via AgentEngine — used for PA interactive mode and recipe fallback'],
        ['Mode 2', 'Python core-orchestrator LivingAgent execution — used for worker fleet recipes'],
      ]},
      { t: 'h2', text: 'N–R' },
      { t: 'table', headers: ['Term', 'Definition'], rows: [
        ['Node', 'Graph entity (file, class, function, test, type, module, import)'],
        ['Orphan', 'Task stuck in in_progress without an active lock file (worker crashed)'],
        ['PA Mode', 'Personal Assistant mode — Rust-native execution for interactive development'],
        ['PA Mode Toggle', 'Dropdown control in chat header to switch between Fast Development, Review, and Delegate modes'],
        ['Permission Mode', 'Autonomous / Manual Approval / Smart Approve / Chat Only — controls write operation gating'],
        ['Recipe', 'YAML-defined workflow with cron scheduling for autonomous agent execution'],
        ['Recipe Runner', 'Rust module that executes recipes as background agent sessions'],
        ['Recipe Scheduler', 'Rust module that manages cron-based recipe execution for worker fleet'],
        ['Redis', 'Standalone binary for warm-tier memory caching (port 6379)'],
        ['Review Mode', 'PA mode with explicit permission checkpoints before write operations'],
      ]},
      { t: 'h2', text: 'S–Z' },
      { t: 'table', headers: ['Term', 'Definition'], rows: [
        ['Sidecar', 'Python process bundled with AutoDev (core-orchestrator on :4000, memory-mcp-server on :5000)'],
        ['Skill', 'Reusable markdown workflow template with YAML frontmatter, invokable via slash commands'],
        ['Subagent', 'Isolated child agent with its own context, tools, and MCP servers'],
        ['Supervisor', 'Fleet agent that polls task board every 2 min, assigns workers, reviews QC reports, merges PRs'],
        ['Task Board', 'Kanban board showing tasks by status: Ready → In Progress → QC Review → Done'],
        ['Tool Registry', 'Maps tool names to implementations, filtered by PA mode (Delegate gets only create_task)'],
        ['Tool Result', 'Structured output from tool execution with output, is_error, and hint fields'],
        ['Vector DB', 'Qdrant or similar for semantic memory storage and similarity-based retrieval'],
        ['Worker', 'Autonomous agent that executes assigned tasks (backend, frontend, platform, cleanup, QC)'],
        ['Worker Request', 'Message from worker to Supervisor (decompose_audit, reassign, needs_info, report_issue, request_review)'],
        ['Write File Tool', 'Built-in tool for writing complete file contents — use only for new files or major rewrites'],
      ]},
    ],
  },
]
