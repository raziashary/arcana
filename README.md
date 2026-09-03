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
- 🧠 **Memory across sessions:** episodic, semantic and procedural stores with relevance ranking, compaction and staleness tracking
- 🎯 **A worker fleet:** supervisor, five workers and QC, coordinated through a SQLite task board
- 📝 **AUTODEV.md system:** SQLite-backed instruction system, editable from UI, no manual file editing required
- 🔌 **MCP servers:** External MCP servers can be registered and are health-checked in Settings. AutoDev does not yet call them, and it does not host one — the in-app MCP server was removed because it was an unauthenticated local endpoint exposing `bash`, `write_file` and `spawn_subagent`.
- 🛠️ **Skills & Recipes:** Reusable YAML workflows with cron scheduling for fleet orchestration
- 🚀 **One installer, nothing to run alongside it:** no interpreters, no cache server, no Docker
- 🎭 **One engine:** the assistant, every fleet worker and every subagent run on the same in-process Rust engine, with the same tools and the same permission policy
- 🔄 **Git integration:** structured git operations via git2-rs. Workers share the configured workspace and commit to a shared branch; no worktree is created per task
- 📊 **Cost tracking:** Real-time token spend per session with per-agent, per-task reporting
- 🛡️ **Workspace path-jail:** Every writing tool enforces that file paths and shell working directories stay inside the user's configured workspace
- 🧯 **Budget circuit breaker:** Per-task token / USD / turn caps, pre-emptively halting runaway loops and escalating to the Supervisor
- 🧪 **Build evidence gate:** Workers can't push tasks to QC review without a recent successful build on record
- 👥 **Headless QC dispatcher:** A fleet of short-lived reviewer subagents reviews completed work in parallel with a concurrency cap and a rolling cost guard
- 🧭 **Semantic task routing:** Supervisor learns which Worker performs best for which task type from historical outcomes
- 🩺 **MCP health monitor:** per-server liveness pings, TCP-alive checks and auto-reconnect for the MCP servers you connect
- 🪦 **PID-based orphan detection:** Tasks abandoned by a crashed Worker are released within the next Supervisor poll (~2 min) instead of waiting 4 hours
- 📐 **Backend↔Frontend contract:** INTERFACE.md authored by the Supervisor and validated against both sides, with structured drift detection in diff analysis
- 🗺️ **Phase-based decomposition:** Epics broken into ordered phases with blockers, milestones, and a live Epic Phase View
- 🔎 **Diff risk scoring:** `analyze_diff` tool gives the QC agent a structured risk score, classification, summary, and review notes per PR

---

## Architecture

AutoDev is a **Tauri v2** desktop application — a Rust backend and a React front
end, in one process. Every agent it runs, from the assistant you chat with to a
fleet worker on a cron beat, runs on the same engine inside that process. There
is nothing to install alongside it and no service that has to be healthy first.

```
┌──────────────────────────────────────────────────────────────┐
│                        AutoDev — one process                     │
├─────────────────────┬──────────────────────────────────────────┤
│  React + TypeScript  │            Rust                          │
│  src/                │            src-tauri/ + autodev-core/    │
│                      │                                          │
│  Chat                │   Tauri commands                         │
│  AutoDev panel       │    agent · provider · vault · settings     │
│  IDE                 │    tasks · workers · recipes · mcp         │
│  Recipes             │                                          │
│  Graph               │   AgentEngine                            │
│  Docs                │    the tool loop every agent runs on      │
│  Settings            │    assistant · fleet worker · subagent    │
│                      │                                          │
│  Zustand stores      │   agent/    tools, prompts, permissions   │
│    ↕ invoke          │   recipes/  YAML + cron scheduler        │
│    ↕ events          │   memory/   episodic, semantic, proc.    │
│                      │   provider/ Anthropic · OpenAI · Google   │
│                      │   mcp/      client for your MCP servers   │
│                      │   git/      git2-rs                       │
│                      │   graph/    knowledge graph               │
│                      │   db/       SQLite                        │
│                      │                                          │
│                      │   ~/.autodev/  — SQLite on your disk      │
│                      │    tasks · workers · sessions · memory     │
│                      │    analytics · recipes · instructions     │
└─────────────────────┴──────────────────────────────────────────┘
```

The only things outside this process are the LLM provider you chose and any MCP
servers you connected yourself.

### One engine

The assistant and the fleet take the same path. A turn is a function call, not
an HTTP request to something else on your machine.

```
You / a cron beat
      ↓
AgentEngine — builds the prompt, calls the provider, runs the tools
      ↓
built-in tools: read_file, write_file, edit_file, bash, grep, glob, …
      ↓
the task board, memory, git — all SQLite and git2-rs, in-process
      ↓
events → the UI
```

**What that buys:**

- **No round-trip** — nothing to serialise, nothing to wait on.
- **Nothing to start** — the app works the moment it opens.
- **One place to look** — a misbehaving agent is one process and one log.
- **Headless too** — `airsctl -p "..."` runs the same engine from a terminal.
- **Local-first** — sessions, memory and the task board are all on your disk.

### What every agent is given

An agent's prompt is assembled in one place, in this order:

1. **The invariant core** — the prohibitions, the git house rules, and the fact
   that work is coordinated through the task board. Every agent gets this,
   whatever its role, and a role cannot drop it.
2. **Its own instructions** — the assistant's prompt, or a worker's recipe:
   role, protocol and what counts as done.
3. **Your `AUTODEV.md`** — global and per-repository. The most specific voice in
   the room, so it outranks the defaults above it.
4. **Memory** — rebuilt each turn from what this agent has learned.

The first three are stable across a turn and are what the provider's prompt
cache holds; memory is volatile and comes last, so a new fact does not
invalidate the rest.

### The fleet

A supervisor, five workers (backend, frontend, platform, MCP, cleanup) and QC.
Each has a recipe and a cron schedule, and they coordinate through the **SQLite
task board** — not GitHub Issues, and not lockfiles on disk.

```
Supervisor tick → reads the board → assigns work
                                        ↓
Worker tick → claims a task → implements → runs the project's tests
                                        ↓
                    commits to the fleet branch, moves it to qc_review
                                        ↓
QC → reviews → done, or rejected with a reason
```

Workers commit to a shared `autodev/fleet` branch, staging the paths they named
and reading the diff before committing. **They never push, open a pull request
or merge one** — that stays with you.

### Three assistant modes

The chat header switches the assistant between three modes.

| Mode | Tools | Prompt | Use it when |
|------|-------|--------|-------------|
| 🔧 Fast Development | Every built-in tool | `BASE_SYSTEM_PROMPT` | You are coding and want it to keep up |
| ✋ Review | **The same tools** | `REVIEW_SYSTEM_PROMPT` | You want deliberation over speed |
| 📋 Delegate | `create_task`, `planner`, memory — no file or shell access | `DELEGATE_SYSTEM_PROMPT` | The fleet should do the work |

⚠ **Review does not give you a permission checkpoint that Fast Development
lacks.** The assistant already asks before anything that is not a pure read, in
both modes — `ManualApproval` is a literal, not a per-mode setting. What Review
changes is posture: explore read-only first, say what you are about to do before
doing it, one change at a time, and report what you could not verify. The README
claimed the checkpoint was the difference for a long time; it never was.

**Delegate mode, end to end:**

1. You: *"add a CSV export to the sessions list"*
2. The assistant decomposes it and files task #142 on the board, with acceptance
   criteria a reviewer could check without asking you
3. Supervisor (every 2 min): sees #142 `ready`, assigns it to Worker Frontend
4. Worker Frontend: claims it, implements it, runs the project's tests, commits
   to the fleet branch, moves it to `qc_review`
5. QC: reviews and either marks it `done` or rejects it with a reason
6. The AutoDev panel shows each transition as it happens


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
| **Git Integration** | git2-rs (libgit2) | Structured git operations |
| **Agent engine** | Rust (`autodev-core`) | The tool loop every agent runs on |
| **Memory** | Rust + SQLite | Episodic, semantic and procedural stores with relevance ranking |
| **Keychain** | keyring crate | OS keychain integration for API keys |
| **Database** | SQLite (rusqlite) | Session persistence, task board, worker registry |

---

## Project Structure

```
autodev/
├── src/                        React + TypeScript front end
│   ├── tabs/                   Chat, AutoDev, IDE, Recipes, Graph, Docs, Settings
│   ├── components/             UI, grouped by feature
│   ├── stores/                 Zustand state
│   ├── data/                   Docs-tab content, model registry
│   └── types/                  Shared TypeScript types
│
├── autodev-core/               The engine. Everything an agent does lives here.
│   └── src/
│       ├── agent/              Tool loop, built-in tools, prompts, permissions
│       ├── recipes/            YAML recipes, cron scheduler, the worker runner
│       ├── subagent/           In-process subagents, pool and nesting limits
│       ├── memory/             Episodic, semantic and procedural stores
│       ├── provider/           Anthropic, OpenAI, Google; routing and retry
│       ├── db/                 SQLite: tasks, workers, sessions, plans, settings
│       ├── git/                git2-rs operations and diffing
│       ├── graph/              Knowledge graph
│       ├── budget/             Per-task budgets and the daily spend ceiling
│       ├── security/           Redaction, environment scrubbing
│       └── …                   analytics, cache, hooks, plan, plugin, vault …
│
├── src-tauri/                  The desktop shell
│   └── src/
│       ├── commands/           Tauri commands — the front end's whole API surface
│       ├── mcp/                MCP client and health
│       ├── updater/            Signed auto-update
│       ├── ci_gate.rs          Spec-invariant gates, run by `cargo test`
│       └── main.rs / lib.rs    Start-up and command registration
│
├── src-cli/                    `airsctl` — the same engine, headless
├── src-shared/                 Types and config shared by the app and the CLI
│
├── installers/                 msi / dmg / deb / rpm packaging
├── scripts/                    Build and smoke scripts
├── e2e/                        End-to-end tests
└── docs/                       Specifications, plans and decision records
```

Runtime state lives outside the repository, in `~/.autodev/`: the task board,
the worker registry, sessions, memory, analytics, recipes and your
`AUTODEV.md`. Nothing an agent produces is written inside the app's own
install.

## Quick Start

### Prerequisites

- **Rust 1.90+** — `rustup` toolchain
- **Node.js 20+** and npm
- **Git**

That is the whole list. AutoDev bundles no runtimes and starts no services.

```bash
rustc --version   # 1.90+
node --version    # 20+
```

### Installation

```bash
git clone https://github.com/AIRS-Foundry/autodev.git
cd autodev

npm install
npm run tauri dev
```

Open **Settings → Providers**, add a key for Anthropic, OpenAI or Google, and
pick a model. Keys go to your OS keychain — never to a config file, and never
into the process environment.

---

## Development

### Running Locally

```bash
npm run tauri dev
```

That is the whole loop. There is no second terminal and no service to start
first.

⚠ **Do not run `uv sync` here.** AutoDev is a Tauri app — Rust and Node. It has
no Python and no `pyproject.toml`.

### Building for Production

```bash
npm run tauri build
```

The installer lands in `src-tauri/target/release/bundle/`. It contains the app
and nothing else — no bundled interpreters, no downloaded binaries, no
post-install steps.

### Checks

```bash
cargo check --workspace     # compiles
cargo test  --workspace     # unit tests + the CI gates
npx tsc --noEmit            # frontend types
npx vitest run              # frontend tests
```

The `ci_gate` tests are spec invariants, not unit tests: they assert properties
the codebase must keep holding — that no agent runs without the invariant core,
that every frontend event listener has a Rust emitter, that no shipped source
names a deleted subsystem. They run with `cargo test`.

---

## Documentation

| Document | Description |
|----------|-------------|
| [`docs/autodev-vision/autodev-master-plan.md`](docs/autodev-vision/autodev-master-plan.md) | **Living product spec** - single source of truth for vision, features, architecture, UI/UX |
| [`docs/development/DEVELOPMENT_PLAN.md`](docs/development/DEVELOPMENT_PLAN.md) | **Master development plan** - phased build plan, module breakdown, coding conventions |
| [`docs/ENVIRONMENT_SETUP.md`](docs/ENVIRONMENT_SETUP.md) | Complete environment setup guide |

---

## Key Concepts

### One engine, three assistant modes

There is one execution path. The assistant, every fleet worker and every
subagent run on the same in-process Rust engine, with the same tool registry
and the same permission policy. What differs between them is their
instructions, not their machinery — see **Architecture** above.

Within the assistant, three modes change what it can reach: **Fast
Development** (everything), **Review** (the same tools, a more deliberate
posture) and **Delegate** (no file or shell access at all — it files work on
the board instead).

### The task board, not GitHub Issues

Every agent coordinates through SQLite at `~/.autodev/tasks.db`.

- **Why:** one app, no external service to be reachable, no API rate limit
  between a worker and its next task. It works offline and you own the data.
- **Flow:** a task is filed → the Supervisor assigns it → a worker claims and
  implements it → QC reviews → `done`, or `rejected` with a reason.
- **Statuses:** `ready`, `assigned`, `in_progress`, `qc_review`, `done`,
  `rejected`, `failed`, `build_failed`, `needs_info`, `cancelled`.
- Agents do **not** write lockfiles and do **not** create git worktrees. An
  earlier multi-machine fleet used both; this one uses the board plus a
  PID-based orphan detector, so a lockfile here would be an artifact nothing
  reads.

### What the fleet may do to your repository

Workers are held to the same git rules you would hold a contractor to, and the
rules are part of the invariant core every agent receives:

- **Branch before the first commit** — they share `autodev/fleet` and never
  commit onto `main` or `master`.
- **Stage the paths they named** — never `git add -A`, which sweeps in build
  output and anything your ignore rules did not anticipate.
- **Read the staged diff before committing**, and refuse it if it carries a
  credential.
- **Never push, open a pull request, or merge one.** The branch is the
  deliverable; what happens to it is your decision.
- **Never force-push, rewrite published history, or delete a branch, a stash or
  a remote ref.**

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
- **Live Monitor:** worker and subagent tool calls streamed in real time; lazy slug registration so events are never dropped

**Safety & Reliability**
- **Workspace Path-Jail:** `write_file` / `edit_file` / `append_file` / `bash` (cwd + redirect targets) all enforce that paths stay inside the configured workspace
- **Build Evidence Gate:** Session-keyed ring of recent `EXIT:0` builds; Workers cannot transition tasks to `qc_review` without recent successful build evidence
- **Budget Circuit Breaker:** Per-task token / USD / turn caps with pre-emptive halt and `needs_info` escalation; per-task `BudgetBar` widget; daily budget summary
- **PID-Based Orphan Detection:** Workers record their OS PID + hostname when claiming; orphans released within ~2 min via the Supervisor's poll
- **Backend↔Frontend Interface Contract:** `INTERFACE.md` authored via `write_interface_spec`; Tier 2.6 API-contract validation; structured drift detection in `analyze_diff`
- **Tier 2.5 Export Cross-Reference:** Pure-Rust import↔export matcher catches missing/renamed exports before the slow compiler does
- **Fleet Smoke Recipe + `e2e_probe`:** Scheduled end-to-end check that boots the app and confirms it serves an expected response

**Visibility & Diagnostics**
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
- **MCP Tools:** browse the servers you have connected, the tools each exposes, and their liveness
- **Browser-Test Skill:** Playwright-driven visual regression and E2E checks invoked by the QC agent on frontend PRs

---

## Development Phases

> **These are delivery records, not a description of the app as it stands.** A
> phase below may name something that was built at the time and has since been
> removed — most notably the two bundled Python services, deleted in Phase 8.
> For what AutoDev is today, read **Architecture** above; this section is kept
> because a changelog that is edited to match the present is no longer a
> changelog.


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
- ✅ Rust stable (via rustup)
- ✅ Node.js v22.1.0 / npm 10.7.0
- ✅ Git 2.45.0+
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

**Q: What do I need installed?**  
A: Rust, Node and Git. That is the whole list — no Docker, no Python, no cache server, and nothing that has to be running before you open the app.

**Q: Can I use different LLM providers?**  
A: Yes. AutoDev supports Anthropic, OpenAI, Gemini, Ollama, Azure OpenAI, and Google Bedrock via the provider abstraction layer.

**Q: Is this compatible with my existing CLAUDE.md files?**  
A: Yes. AutoDev has an AUTODEV.md system (SQLite-backed) that is separate from CLAUDE.md. For migration, you can import CLAUDE.md content into AUTODEV.md via the UI.

**Q: Why Node v22.1.0?**  
A: Vite 8 prefers v22.12+, but v22.1.0 works fine. Warnings can be ignored. The version is pinned for compatibility.

**Q: Do I need to run `uv sync`?**  
A: No. AutoDev is a Tauri app — Rust and Node. There is no Python in it and no `pyproject.toml`.

**Q: What is the difference between the assistant and a worker?**  
A: Their instructions, not their machinery. Both run the same engine in the same process, with the same tools and the same permission policy. The assistant is interactive and takes its prompt from the mode you chose; a worker runs on a cron beat and takes its prompt from its recipe. Both also receive the invariant core and your `AUTODEV.md`.

**Q: Will the fleet push to my repository?**  
A: No. Workers commit to a shared `autodev/fleet` branch, staging only the paths they named and reading the diff first. They never push, open a pull request, or merge one — and they never force-push or delete anything. Those rules are in the invariant core every agent receives, so a recipe cannot opt out of them.

**Q: How does the internal task board work?**  
A: AutoDev uses an internal SQLite task board (`~/.autodev/tasks.db`) instead of GitHub Issues — faster, offline-capable, and yours. A task moves `ready` → `assigned` → `in_progress` → `qc_review` → `done`, and can land in `rejected`, `failed`, `build_failed`, `needs_info` or `cancelled` instead.

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

**Q: What happens if my provider is unreachable, or I have not set a key?**
A: The tick fails cleanly and says why. With no provider configured at all, the worker reports `idle_no_provider` rather than `error` — nothing is broken, you have simply not finished setting up — and the Live Monitor paints it amber rather than as ordinary idle, so "needs me" is distinguishable from "fine". It is scheduled again on its next beat, so adding a key is enough to bring it back.

**Q: How does the Supervisor decide who gets a task?**
A: Two layers. First a small set of hand-written static patterns (keywords like "fastapi", "react", "docker" → worker-backend/-frontend/-platform); if those are ambiguous or disagree with history, the Supervisor calls the `suggest_worker` tool, which scores Workers by historical success rate × duration ranking for that task type (`routing_history` table). A Worker Affinity Stats panel exposes the per-Worker per-type success rates.

---

**For detailed setup instructions, see [`docs/ENVIRONMENT_SETUP.md`](docs/ENVIRONMENT_SETUP.md)**

**For the complete product specification, see [`docs/autodev-vision/autodev-master-plan.md`](docs/autodev-vision/autodev-master-plan.md)**

**For the master development plan, see [`docs/development/DEVELOPMENT_PLAN.md`](docs/development/DEVELOPMENT_PLAN.md)**
