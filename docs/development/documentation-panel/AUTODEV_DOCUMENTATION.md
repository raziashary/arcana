# AutoDev Documentation

> Complete reference guide for AutoDev — AIRS Foundry's model-agnostic AI agent platform

---

## Table of Contents

1. [What is AutoDev?](#1-what-is-autodev)
2. [Core Architecture](#2-core-architecture)
3. [Personal Assistant (PA) Mode](#3-personal-assistant-pa-mode)
4. [Fleet & Delegate Mode](#4-fleet--delegate-mode)
5. [Recipes System](#5-recipes-system)
6. [Skills System](#6-skills-system)
7. [Hooks System](#7-hooks-system)
8. [Settings Panel](#8-settings-panel)
9. [Dashboard Panel (AutoDev Panel)](#9-dashboard-panel-autodev-panel)
10. [IDE Panel](#10-ide-panel)
11. [Graph Panel](#11-graph-panel)
12. [Technical Internals](#12-technical-internals)
13. [Submodules](#13-submodules)
14. [Glossary](#14-glossary)

---

## 1. What is AutoDev?

AutoDev is a downloadable desktop application that combines the best capabilities of Claude Code and Goose into a single, branded, model-agnostic AI developer agent platform. Purpose-built for AIRS Foundry's AutoDev worker pipeline, it delivers enterprise-grade agent orchestration with zero vendor lock-in.

### Vision

**Tagline:** *One app. Any model. Full autonomy.*

Any AIRS team member or AutoDev worker agent can:
- Download one binary
- Enter their LLM API key (any provider)
- Select their model
- Start executing agentic development tasks with the full capability stack: AUTODEV.md, Skills, Recipes, Subagents, MCP servers, Hooks, and the AutoDev worker pipeline

### Why Build This?

**Claude Code limitations:**
- Locked to Anthropic subscriptions with rate limits that cripple 24/7 worker agents
- 512,000-line source leak (March 31, 2026) exposed vulnerabilities
- No model flexibility — cannot route workers to cheaper models
- Proprietary — cannot audit, fork, or adapt

**Goose limitations:**
- AGENTS.md ecosystem is newer and less mature
- Not tailored to AutoDev's Supervisor → Worker → QC pipeline
- Generic tool with no AIRS-specific automation

**AutoDev advantage:**
- Model-agnostic by design (use Anthropic API directly or OpenAI/Gemini/Ollama)
- AUTODEV.md-native with SQLite-backed instruction system
- AutoDev-native GitHub Issues board understanding
- Secure and auditable (AIRS controls the harness)
- Downloadable and distributable single installer

### Key Features

- 🤖 **Model-agnostic:** Use any LLM (Anthropic, OpenAI, Gemini, Ollama, Azure, Bedrock)
- 🧠 **Advanced memory system:** 3-tier architecture (episodic/semantic/procedural) with context scoring, compression, staleness tracking
- 🎯 **Agent orchestration:** 21 finite state machines governing lifecycle + A2A protocol
- 📝 **AUTODEV.md system:** SQLite-backed instruction system, editable from UI, no manual file editing required
- 🔌 **MCP native:** Connect to GitHub, Slack, Adobe, and custom MCP servers
- 🛠️ **Skills & Recipes:** Reusable YAML workflows with cron scheduling for fleet orchestration
- 🚀 **Single installer:** No Python, Redis, or Docker setup required for end users
- 🎭 **Dual-mode execution:** PA mode (Rust-native for speed) + Worker mode (orchestrated via core-orchestrator)
- 🔄 **Git integration:** Structured git operations via git2-rs with worktree isolation per task
- 📊 **Cost tracking:** Real-time token spend per session with per-agent, per-task reporting

---

## 2. Core Architecture

AutoDev is built on **Tauri v2** (Rust + React) and bundles two powerful Python sidecars as git submodules.

### High-Level Architecture

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
│   └─────────────┘    │    │  ├── git/    (git2-rs)          │  │
│                      │    │  ├── github/ (Issues + PRs)     │  │
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
              MCP clients (GitHub MCP, Slack MCP) via rmcp crate
                    ↓
              Response → UI
```

**Why NOT delegate to core-orchestrator:**
- Lower latency — no HTTP round-trip (100-500ms saved per turn)
- Simpler deployment — works even if Python sidecars fail to start
- Settings UI — test provider API keys without starting core-orchestrator
- Headless CLI — `airsctl -p "..."` runs standalone, no Python dependency
- Offline-first — PA mode + Ollama works without internet

**Limitations:** No A2A, no mem0 semantic memory, no 21 FSMs, no context scoring — intentionally kept simple.

#### Mode 2: Worker Agent — Orchestrated via Core Orchestrator

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
- A2A protocol — supervisor ↔ workers coordination
- mem0 semantic memory — long-term context, retrieval-augmented generation
- 21 FSMs — agent lifecycle, tool execution tracking, memory management, etc.
- Context scoring — relevance ranking, smart compaction
- Feedback-driven synopsis — conversation evolution
- Too complex to reimplement in Rust; Python ecosystem advantage

### Tech Stack

| Layer | Technology | Purpose |
|-------|-----------|---------|
| Desktop Shell | Tauri v2 (Rust) | Cross-platform native wrapper (~15MB base) |
| Frontend | React 19 + TypeScript + Vite 8 | Agent UI, chat, code editor, task board |
| State Management | Zustand | Global state (sessions, config, agent status) |
| Styling | Tailwind CSS 3 | AIRS design system (industrial-minimal: #0A0A0A background, #00FF88 accent) |
| Backend | Rust (Tauri commands) | Code editing tools, file I/O, command execution |
| Provider Layer | Anthropic, OpenAI, Gemini, Ollama, Azure | Multi-provider LLM abstraction |
| MCP Client | rmcp crate | Native Rust MCP client integration |
| Git Integration | git2-rs (libgit2) | Structured git operations, worktrees |
| Orchestrator | Python 3.13 + FastAPI + litellm | Agent lifecycle, FSMs, A2A protocol |
| Memory | Python 3.13 + mem0 | Context storage, scoring, synopsis evolution |
| Cache | Redis 7+ | Warm-tier memory caching |
| Keychain | keyring crate | OS keychain integration for API keys |
| Database | SQLite (rusqlite) | Session persistence, task board, worker registry |

---

## 3. Personal Assistant (PA) Mode

The Personal Assistant (PA) is the primary interface for interactive AI-assisted development. It runs in **Mode 1 (Rust-Native)** for low-latency, direct LLM API access.

### PA Mode Toggle

Within PA mode, three sub-modes control tool availability and system prompt behavior:

| Mode | Enum Variant | Tool Registry | System Prompt Source | Use Case |
|------|---------------|---------------|---------------------|----------|
| 🔧 Fast Development | `PAMode::FastDevelopment` | All builtin_tools (read_file, write_file, edit_file, bash, grep, glob, git) + All MCP tools | `BASE_SYSTEM_PROMPT` | Razi coding directly, quick fixes |
| ✋ Review Mode | `PAMode::Review` | All builtin_tools + permission_check wrapper + All MCP tools | `REVIEW_SYSTEM_PROMPT` + caution instructions | Reviewing changes, cautious editing |
| 📋 Delegate Mode | `PAMode::Delegate` | Only `create_task` tool + **MCP tools (read-only)** | `DELEGATE_SYSTEM_PROMPT` | Workers are live, PA delegates work |

### Fast Development Mode

**System Prompt:** `BASE_SYSTEM_PROMPT` (comprehensive ~200 lines)

**Available Tools:**
- **Reading & Discovery:** read_file, grep, glob, list_directory
- **Writing & Editing:** edit_file (preferred), write_file (new files/major rewrites)
- **Execution:** bash (tests, linting, git operations)
- **Git:** All git operations (status, diff, commit, push, pull, worktree)
- **MCP Tools:** GitHub, Slack, and any configured MCP servers

**Use Case:** Direct coding, quick fixes, exploratory development

**Behavior:**
- Full file and execution access
- No explicit permission prompts
- Agents read before writing, verify after changes
- Surgical edits preferred over wholesale rewrites

### Review Mode

**System Prompt:** `REVIEW_SYSTEM_PROMPT` (adds permission consciousness)

**Available Tools:** Same as Fast Development + permission wrapper

**Use Case:** Code review, cautious editing, production changes

**Behavior:**
- Same tool set as Fast Development
- Adds explicit permission checkpoint before write operations
- Requires reasoning through implications before changes
- One change at a time — no batching
- Logs reasoning for each write operation

### Delegate Mode

**System Prompt:** `DELEGATE_SYSTEM_PROMPT` (task creation focused)

**Available Tools:**
- `create_task` (writes to internal AutoDev task board)
- MCP tools (read-only only — GitHub MCP for context, no write tools)

**Use Case:** Workers are live; PA should delegate work, not execute code

**Behavior:**
- NO file access tools (no read_file, write_file, edit_file, bash)
- NO direct code execution
- Decomposes user requests into atomic tasks
- Creates tasks on internal AutoDev board via `create_task` tool
- Supervisor recipe assigns tasks to workers
- Workers execute via core-orchestrator (Mode 2)

### Delegate Mode Interaction Flow

```
1. User: "Add /health endpoint to orchestrator"
2. PA (Delegate Mode): Understands intent → decomposes → creates task #142
3. PA confirms: "Task #142 created on board. Supervisor will assign to Worker Backend on next poll."
4. Supervisor recipe (polls every 2 min): Sees task #142, assigns to Worker Backend
5. Worker Backend recipe: Executes via core-orchestrator, implements endpoint, opens PR on GitHub
6. QC Agent recipe: Reviews PR, updates task status to "qc-approved"
7. Supervisor: Sees QC approval, instructs worker to merge, closes task #142
8. PA (if monitoring AutoDev Panel): Shows task completion notification
```

### Switching PA Modes

**Location:** Chat header dropdown (3-button segmented control)

**State Management:** `settingsStore.ts` persists `paMode` to localStorage

**Effect:** Changes tool registry and system prompt for the next message sent

**Persistence:** Mode preference saved across sessions

---

## 4. Fleet & Delegate Mode

The AutoDev fleet is a collection of autonomous worker agents that execute tasks assigned by a Supervisor agent. This multi-agent pipeline enables 24/7 autonomous development.

### Fleet Architecture

```
┌─────────────┐
│  Supervisor │  ← Polls task board every 2 min
│   Recipe    │  ← Breaks down epics, assigns workers
└──────┬──────┘
       │
       ├──→ Worker Backend Recipe    (polls every 2 min)
       ├──→ Worker Frontend Recipe   (polls every 2 min)
       ├──→ Worker Platform Recipe   (polls every 5 min)
       ├──→ Worker MCP Recipe        (polls every 5 min)
       ├──→ Worker Cleanup Recipe    (polls every 10 min)
       └──→ QC Agent Recipe         (polls every 2 min)
```

### Worker Types

| Worker | Recipe Schedule | Responsibility |
|--------|----------------|----------------|
| **Supervisor** | `*/2 * * * *` | Poll task board, break down epics, assign workers, review QC reports, merge PRs |
| **Worker Backend** | `*/2 * * * *` | Backend development (Python, Rust, Go) |
| **Worker Frontend** | `*/2 * * * *` | Frontend development (React, TypeScript) |
| **Worker Platform** | `*/5 * * * *` | Platform/infra work (Docker, CI/CD, deployment) |
| **Worker MCP** | `*/5 * * * *` | MCP server development and testing |
| **Worker Cleanup** | `*/10 * * * *` | Code cleanup, refactoring, technical debt |
| **QC Agent** | `*/2 * * * *` | PR review, test execution, QC reports |

### Internal Task Board

AutoDev uses an internal SQLite task board (`~/.autodev/tasks.db`) for task assignment:

**Why not GitHub Issues directly?**
- Single app = no external coordination needed
- Faster (no API calls)
- Offline support
- Full control over data model
- GitHub still used for PRs, but task assignment happens internally

**Database Schema:**
```sql
CREATE TABLE tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    description TEXT,
    priority TEXT NOT NULL,  -- p0/p1/p2/p3
    task_type TEXT NOT NULL,  -- feature/bug/refactor/docs/chore
    status TEXT NOT NULL,     -- ready/assigned/in_progress/qc_review/done/failed/cancelled/rejected
    acceptance_criteria TEXT,
    assigned_worker TEXT,
    created_at DATETIME,
    updated_at DATETIME
);
```

### Task Lifecycle

```
Ready (created by PA or Supervisor)
    ↓
Assigned (Supervisor assigns to worker)
    ↓
InProgress (Worker claims task, creates lock file)
    ↓
QC Review (Worker completes, opens PR, QC reviews)
    ↓
Done (QC approved, Supervisor merges, closes task)
    ↓
    └─→ Failed (Worker cannot complete)
    └─→ Build Failed (Dependencies/build validation fails)
    └─→ Rejected (QC rejects, worker must rework)
```

### Worker Request Queue

Workers can communicate with the Supervisor via the `worker_requests` table:

**Request Types:**
- `decompose_audit` — Worker found scope too large, suggests sub-tasks
- `reassign` — Worker cannot handle task, suggests reassignment
- `needs_info` — Worker needs clarification from human PA
- `report_issue` — Worker encountered blocking issue
- `request_review` — Worker completed task, requests QC review

**Flow:**
```
Worker (via request_supervisor_action tool) → worker_requests table
                                            ↓
Supervisor recipe (polls every 2 min) → processes requests
                                            ↓
Creates tasks / reassigns / marks acknowledged
```

### Lock Files

Workers use lock files to claim tasks and prevent conflicts:

**Location:** `current_tasks/issue-N.lock`

**Format:** ISO 8601 timestamp (UTC)

**Rules:**
- Worker writes lock file before starting work
- Worker deletes lock file only on clean completion
- Never delete lock file mid-task on failure
- Lock files are sacred — prevent concurrent work on same task

### Branch Naming Convention

Workers create git worktree branches following this pattern:

```
worker/<role>/<issue-number>
```

**Examples:**
- `worker/backend/142`
- `worker/frontend/87`
- `worker/cleanup/201`

**Why worktrees?**
- Isolation — each task gets a clean working directory
- Parallel work — multiple workers can work simultaneously
- No main branch pollution — changes stay in worktrees
- Easy cleanup — worktree removed after task completion

### Orphan Detection

The `orphan` module detects tasks that are stuck in `in_progress` but have no active lock file (e.g., worker crashed). These are automatically released back to `assigned` state so they can be re-claimed.

---

## 5. Recipes System

Recipes are YAML-defined workflows that schedule autonomous agent execution. They are the backbone of the worker fleet, enabling 24/7 automated development.

### Recipe YAML Structure

```yaml
id: supervisor-poll
title: Supervisor Poll
description: Polls task board and assigns work to workers
agent_role: supervisor
schedule: "*/2 * * * *"  # Cron expression
permission_mode: autonomous
model: claude-sonnet-4-5
instructions: |
  You are the Supervisor Agent. Your responsibilities:
  1. Poll the task board via list_tasks__autodev
  2. Check for worker_requests via list_worker_requests
  3. Assign tasks to workers via assign_task__autodev
  4. Monitor worker progress
  5. Review QC reports
  6. Merge approved PRs
```

### Cron Scheduling

Recipes use POSIX cron expressions (5 fields: minute hour day month weekday):

| Expression | Meaning |
|-----------|---------|
| `*/2 * * * *` | Every 2 minutes |
| `*/5 * * * *` | Every 5 minutes |
| `*/10 * * * *` | Every 10 minutes |
| `0 * * * *` | Every hour |
| `0 0 * * *` | Every day at midnight |

### Recipe Execution

**Two Modes:**

1. **Mode 2 (Primary)** — core-orchestrator LivingAgent
   - Full 21 FSMs
   - mem0 semantic memory
   - A2A protocol
   - Context scoring
   - Used when core-orchestrator is healthy

2. **Mode 1 (Fallback)** — Rust AgentEngine
   - Direct LLM API
   - No FSMs
   - No semantic memory
   - Used when core-orchestrator is unreachable

### Recipe Scheduler

**Implementation:** `src-tauri/src/recipes/scheduler.rs`

**Key Features:**
- Uses `tokio` tasks for background execution
- `cron` crate for schedule parsing
- Idempotent `start_worker()` — restarts if already running
- Immediate startup tick (no wait for first cron fire)
- Mode 2 catch-up tick (if startup was Mode 1, retry once when orch ready)
- Orphan detection before Supervisor tick

### Recipe Runner

**Implementation:** `src-tauri/src/recipes/runner.rs`

**Key Features:**
- Health check before dispatch (determines Mode 1 vs Mode 2)
- No-op short-circuit when worker has no runnable tasks (Mode 1 only)
- Model resolution: worker.model > recipe.model > active_model > default
- Workspace directory: configured workspace > sandbox fallback
- Ephemeral sessions (not persisted to session DB)
- Status updates: idle → working → idle/error

### Default System Recipes

**Location:** `src-tauri/src/recipes/defaults.rs`

**Recipes:**
1. **supervisor-poll** — Supervisor agent, every 2 minutes
2. **worker-backend-poll** — Backend worker, every 2 minutes
3. **worker-frontend-poll** — Frontend worker, every 2 minutes
4. **worker-platform-poll** — Platform worker, every 5 minutes
5. **worker-mcp-poll** — MCP worker, every 5 minutes
6. **worker-cleanup-poll** — Cleanup worker, every 10 minutes
7. **qc-agent-poll** — QC agent, every 2 minutes

### Recipe Parameters

Recipes support typed parameters for dynamic inputs:

```yaml
parameters:
  - name: issue_number
    type: integer
    required: true
    description: GitHub issue number to process
  - name: repo_name
    type: string
    required: false
    default: "airs-orchestrator"
```

### Recipe UI

**Location:** `src/tabs/Recipes.tsx`

**Features:**
- List all recipes (system + user)
- Expandable cards showing YAML content
- Edit recipe via RecipeEditor modal
- Create new recipes
- System recipes marked as read-only
- Permission mode badge (autonomous/manual_approval)
- Agent role badge
- Schedule display
- Model display

---

## 6. Skills System

Skills are reusable markdown workflow templates that can be invoked via slash commands or programmatically by fleet workers. They provide a way to package common development patterns, debugging workflows, and operational procedures into shareable, parameterized templates.

### What Are Skills?

Skills are `.md` files with YAML frontmatter stored in `~/.autodev/skills/{category}/{skill-name}.md`. They combine:
- **YAML frontmatter** — Metadata (name, description, category, parameters, version)
- **Markdown template** — The workflow body with `{{parameter}}` placeholders

### Skill Categories

| Category | Description | Icon |
|----------|-------------|------|
| **Coding** | Development workflows (feature implementation, refactoring) | ⚡ Zap |
| **Debugging** | Debugging patterns (error investigation, log analysis) | 🐛 Bug |
| **DevOps** | Infrastructure and deployment workflows | 🖥️ Server |
| **Fleet** | Fleet management and orchestration tasks | 👥 Users |
| **Custom** | User-defined skills | ➕ Plus |

### Skill File Structure

```yaml
---
name: add-auth-endpoint
description: Adds JWT-based authentication endpoint to a REST API
category: coding
parameters:
  - name: endpoint_path
    type: string
    required: true
    description: Path for the new auth endpoint (e.g., /api/auth/login)
  - name: jwt_secret_env
    type: string
    required: false
    default: JWT_SECRET
    description: Environment variable name for JWT secret
version: "1.0.0"
author: AIRS Foundry
tags: [authentication, jwt, rest-api]
---

You are adding JWT-based authentication to a REST API at `{{endpoint_path}}`.

1. First, verify the project structure:
   - Use `list_directory` to understand the project layout
   - Use `glob` to find existing route handlers
   - Use `read_file` to examine the current routing setup

2. Add the JWT dependency:
   - Check package.json (Node.js) or requirements.txt (Python)
   - Add the appropriate JWT library
   - Run `npm install` or `pip install` as needed

3. Implement the auth endpoint:
   - Create the endpoint at `{{endpoint_path}}`
   - Load JWT secret from environment variable `{{jwt_secret_env}}`
   - Implement login logic with credentials validation
   - Return JWT token on successful authentication

4. Add authentication middleware:
   - Create middleware to verify JWT tokens
   - Apply middleware to protected routes
   - Test the authentication flow

5. Write tests:
   - Test successful login
   - Test invalid credentials
   - Test token verification
   - Test protected route access

Use the appropriate tools for the language/framework detected in the project.
```

### Parameter Types

| Type | Description | Example |
|------|-------------|---------|
| **String** | Text value | `"my-feature"` |
| **Boolean** | True/false | `true` |
| **Integer** | Whole number | `42` |
| **Enum** | One of predefined values | `"production" | "staging" | "development"` |

### Slash Command Invocation

**Keyboard Shortcut:** `Cmd/Ctrl+Shift+S` opens the Skills Palette

**In Chat:**
- Type `/` to trigger autocomplete
- Select a skill from the dropdown
- Fill in parameters via modal
- Skill renders into chat input
- Send to execute

**Example:**
```
User: /add-auth-endpoint
[SkillParameterModal opens]
User fills: endpoint_path="/api/auth/login", jwt_secret_env="JWT_SECRET"
[Skill renders into chat input]
User sends message → Agent executes the workflow
```

### Programmatic Invocation

Fleet workers can invoke skills programmatically via Tauri commands:

```rust
let result = execute_skill("add-auth-endpoint", params).await?;
// result.rendered contains the full markdown with parameters substituted
```

**Use Cases:**
- Supervisor recipe invokes skills for common patterns
- Workers use skills for standardized debugging workflows
- QC agent uses skills for consistent review procedures

### Skills Palette UI

**Location:** `src/components/skills/SkillsPalette.tsx`

**Features:**
- Search across all skills
- Filter by category (Coding, Debugging, DevOps, Fleet, Custom)
- Recent skills tab (tracks last 10 used)
- Skill detail view with full template
- Parameter modal for execution
- Create new skill button

**Keyboard Navigation:**
- Arrow keys to navigate
- Enter to select
- Escape to close
- Tab to cycle through categories

### Skill Execution Flow

```
1. User selects skill (via slash command or palette)
2. SkillParameterModal opens (if skill has parameters)
3. User fills in parameter values
4. SkillEngine renders template with parameters substituted
5. Rendered markdown inserted into chat input
6. User sends message
7. Agent executes the workflow as if user typed it directly
```

### Skill Storage

**Location:** `~/.autodev/skills/{category}/{skill-name}.md`

**Built-in Skills:** AutoDev ships with 20+ pre-built skills covering common patterns

**Custom Skills:** Users can create skills in the `custom` category

**Hot Reload:** File watcher detects changes and reloads skills (future work)

### Tauri Commands

- `list_skills()` — List all skills with summaries
- `get_skill(name)` — Get full skill details
- `search_skills(query)` — Search skills by name/description/tags
- `skills_by_category(category)` — List skills in a category
- `validate_skill_parameters(name, params)` — Validate parameter values
- `execute_skill(name, params)` — Render and return skill
- `reload_skills()` — Reload skills from disk
- `import_skill(skill_data)` — Import a skill from JSON

### Default Built-in Skills

**Coding:**
- `add-auth-endpoint` — Add JWT authentication
- `add-rest-endpoint` — Add REST API endpoint
- `refactor-component` — Refactor React component
- `add-unit-tests` — Add unit tests for a function

**Debugging:**
- `investigate-error` — Systematic error investigation
- `analyze-logs` — Log file analysis
- `debug-test-failure` — Debug failing test
- `profile-performance` — Performance profiling

**DevOps:**
- `dockerize-app` — Containerize application
- `setup-ci-cd` — Setup CI/CD pipeline
- `deploy-to-staging` — Deploy to staging environment

**Fleet:**
- `create-worker-task` — Create task for worker
- `monitor-fleet-health` — Check fleet status
- `analyze-fleet-metrics` — Analyze fleet performance

### Skill Validation

Skills are validated on load:
- YAML frontmatter must be valid
- Required parameters must be defined
- Parameter types must be valid
- Template must contain valid `{{parameter}}` placeholders

Invalid skills are skipped and logged.

### Use Cases

**For PA Mode:**
- Quick invocation of common patterns via slash commands
- Consistent debugging workflows
- Standardized code review procedures

**For Fleet Workers:**
- Supervisor uses skills for task breakdown patterns
- Workers use skills for standardized implementation patterns
- QC agent uses skills for consistent review criteria

**For Teams:**
- Shareable workflows across team members
- Version-controlled skill library in git
- Custom skills for team-specific patterns

---

## 7. Hooks System

Hooks are event-driven extensibility points that execute custom actions at key lifecycle points throughout AutoDev. They enable automation, safety checks, notifications, logging, and custom integrations without modifying core code.

### What Are Hooks?

Hooks are stored in SQLite (`~/.autodev/config.db`) and execute at specific events. Each hook has:
- **Trigger event** — When the hook fires (e.g., PreToolUse, PostCommit)
- **Conditions** — When the hook should run (tool name, file pattern, time range, custom logic)
- **Action** — What to do (shell command, HTTP request, notification, block operation)
- **Priority** — Execution order (lower = higher priority)

### Hook Event Types

**Agent Lifecycle:**
- `SessionStart` — When a chat session begins
- `SessionEnd` — When a chat session ends
- `AgentLoopStart` — Before agent loop begins
- `AgentLoopEnd` — After agent loop completes
- `OnError` — When agent encounters an error

**Tool Execution:**
- `PreToolUse` — Before a tool is called
- `PostToolUse` — After a tool completes
- `OnToolError` — When a tool fails
- `OnPermissionRequest` — When permission is requested

**File System:**
- `PreFileRead` — Before reading a file
- `PostFileWrite` — After writing a file
- `PreFileDelete` — Before deleting a file
- `OnCheckpointCreate` — After checkpoint created
- `OnCheckpointRestore` — After checkpoint restored

**Git:**
- `PreCommit` — Before git commit
- `PostCommit` — After git commit
- `PrePush` — Before git push
- `OnBranchSwitch` — After switching branches
- `OnMergeConflict` — On merge conflict detected

**Plan Management:**
- `PrePlanStep` — Before each plan step
- `PostPlanStep` — After each plan step
- `OnPlanActivate` — When plan is activated
- `OnPlanComplete` — When plan completes
- `OnPlanAbandon` — When plan is abandoned

**Fleet:**
- `OnTaskClaim` — When worker claims a task
- `OnTaskComplete` — When worker completes a task
- `OnTaskFail` — When worker fails a task
- `OnQcReviewStart` — When QC review starts
- `OnQcApprove` — When QC approves
- `OnQcFail` — When QC rejects

### Hook Actions

**Shell:**
```json
{
  "type": "shell",
  "command": "cargo fmt",
  "working_dir": "/path/to/repo",
  "timeout_secs": 30
}
```

**Script:**
```json
{
  "type": "script",
  "path": "/path/to/script.sh",
  "args": ["--flag", "value"]
}
```

**HTTP:**
```json
{
  "type": "http",
  "url": "https://hooks.example.com/autodev",
  "method": "POST",
  "headers": {"Authorization": "Bearer token"},
  "body": {"event": "{{hook_type}}", "session_id": "{{session_id}}"}
}
```

**Notification:**
```json
{
  "type": "notification",
  "title": "Task Complete",
  "message": "A worker finished its task."
}
```

**Log:**
```json
{
  "type": "log",
  "file": "~/.autodev/logs/tools.log",
  "message": "[{{hook_type}}] {{tool_name}}: {{command}}"
}
```

**Block:**
```json
{
  "type": "block",
  "message": "Destructive operation blocked by hook"
}
```

**Modify:**
```json
{
  "type": "modify",
  "transform": "$.input.password = '***'"
}
```

**EmitEvent:**
```json
{
  "type": "emit_event",
  "event_name": "custom_event",
  "payload": {"data": "value"}
}
```

### Hook Conditions

Conditions control when a hook should execute:

**Tool Name:**
```json
{
  "tool_name": "bash"
}
```

**File Pattern:**
```json
{
  "file_pattern": "**/*.rs"
}
```

**Command Pattern:**
```json
{
  "command_pattern": "rm\\s+-rf"
}
```

**Session Type:**
```json
{
  "session_type": "fleet"
}
```

**Agent Mode:**
```json
{
  "agent_mode": "fast_dev"
}
```

**Time Range:**
```json
{
  "time_range": {
    "start_hour": 9,
    "end_hour": 17,
    "days": ["mon", "tue", "wed", "thu", "fri"]
  }
}
```

**Custom Condition (Rhai script):**
```json
{
  "custom_condition": "tool_name == \"bash\" && command.contains(\"rm -rf\")"
}
```

**Available Variables:**
- `tool_name` — Name of tool being called
- `file_path` — Path to file being operated on
- `agent_mode` — Current PA mode
- `command` — Command being executed
- `hook_type` — Current hook event type
- `session_id` — Current session ID
- `worker_id` — Worker ID (for fleet mode)
- `task_id` — Task ID (for fleet mode)
- `is_error` — Whether operation resulted in error

### Hook Templates

AutoDev includes pre-built hook templates for common use cases:

**Safety:**
- **Block rm -rf /** — Prevents accidental root deletion
- **Block force push** — Stops --force pushes to remotes
- **Block .env file commits** — Prevents committing secrets

**Automation:**
- **Auto-format Rust on save** — Runs `cargo fmt` after .rs file writes
- **Auto-format TypeScript on save** — Runs prettier on .ts/.tsx files
- **Clippy lint on commit** — Runs `cargo clippy` before commits
- **Webhook on plan complete** — POSTs to webhook when plan finishes

**Notifications:**
- **Notify on task complete** — Desktop notification when worker finishes
- **Notify on QC failure** — Alert when QC rejects output
- **Notify on agent error** — Alert when agent encounters error

**Logging:**
- **Log all tool calls** — Appends tool calls to log file
- **Log errors to file** — Captures errors in dedicated log
- **Log git activity** — Records commits and pushes

### Hooks Settings UI

**Location:** `src/components/settings/HooksSettings.tsx`

**Features:**
- List all hooks with status (enabled/disabled)
- Toggle hooks on/off
- Add new hooks from template gallery
- Create custom hooks with full configuration
- Test hooks with mock context
- View hook execution logs
- Delete hooks

**Template Gallery:**
- Browse pre-built hook templates
- One-click install from template
- Filter by category (Safety, Automation, Notifications, Logging)

**Hook Log Viewer:**
- Filter logs by outcome (Executed, Blocked, Skipped, Failed)
- Expand log entries for full details
- Export logs as JSON
- Real-time updates on new log entries

### Hook Execution

**Priority:** Hooks execute in priority order (lower number = higher priority)

**Condition Evaluation:** All conditions must match for hook to execute

**Execution Flow:**
```
Event occurs (e.g., PreToolUse)
    ↓
HookRegistry fires event
    ↓
For each hook registered for event:
    1. Evaluate conditions
    2. If conditions match:
       a. Execute action
       b. Log outcome (Executed/Blocked/Skipped/Failed)
       c. If action is Block, stop event propagation
    3. If conditions don't match: Skipped
```

**Blocking Behavior:**
- `Block` action stops the event from proceeding
- Example: PreToolUse hook blocks dangerous `rm -rf /` command
- Agent receives error message from hook

**Hook Execution Context:**
```rust
HookContext {
    hook_type: "pre_tool_use",
    timestamp: 2026-05-12T12:00:00Z,
    session_id: Some("session-123"),
    agent_mode: Some("fast_dev"),
    tool_call: Some(ToolCallContext {
        id: "tool-456",
        name: "bash",
        input: {"command": "rm -rf /"},
        result: None,
        is_error: false
    }),
    file_path: None,
    task_id: None,
    worker_id: None,
    extra: {}
}
```

### Tauri Commands

- `list_hooks()` — List all hooks
- `enable_hook(id)` — Enable a hook
- `disable_hook(id)` — Disable a hook
- `add_hook(input)` — Add a new hook
- `remove_hook(id)` — Delete a hook
- `test_hook(id, context)` — Test hook with mock context
- `get_hook_logs(filter)` — Get hook execution logs

### Hook Execution Logs

**Location:** SQLite table in `~/.autodev/config.db`

**Fields:**
- `id` — Log entry ID
- `hook_id` — Hook that executed
- `hook_name` — Hook name
- `hook_type` — Event type
- `outcome` — Executed/Blocked/Skipped/Failed
- `output` — Action output (if any)
- `error` — Error message (if failed)
- `executed_at` — Timestamp
- `duration_ms` — Execution time

**Retention:** Logs retained for 30 days (configurable)

### Use Cases

**Safety:**
- Prevent destructive operations (rm -rf /, force push)
- Block commits of sensitive files (.env, secrets)
- Require approval for dangerous tool combinations

**Automation:**
- Auto-format code on save
- Run linters before commits
- Execute deployment scripts on task completion
- Trigger CI/CD pipelines on git events

**Notifications:**
- Desktop alerts for important events (task complete, QC fail, error)
- Slack notifications for fleet status
- Email alerts for critical failures

**Logging:**
- Audit trail of all tool executions
- Centralized error logging
- Git activity tracking
- Performance monitoring

**Custom Integrations:**
- Webhooks to external services
- Custom validation logic
- Business rule enforcement
- Third-party service integration

### Best Practices

**Hook Design:**
- Keep hooks fast (< 1 second for non-blocking hooks)
- Use specific conditions to avoid false positives
- Test hooks with mock context before enabling
- Monitor hook logs for failures

**Safety Hooks:**
- Use `Block` action sparingly (only for truly dangerous operations)
- Provide clear error messages
- Document why operation was blocked
- Allow override for trusted sessions

**Performance:**
- Avoid expensive operations in hot paths (PreToolUse, PostFileWrite)
- Use background tasks for long-running operations
- Set appropriate timeouts for shell commands
- Monitor hook execution time

**Security:**
- Never log sensitive data (passwords, API keys)
- Validate all user input in custom conditions
- Use least privilege for shell commands
- Review hooks regularly for security risks

---

## 8. Settings Panel

The Settings panel is the central configuration hub for AutoDev. It manages providers, models, workers, GitHub integration, system prompts, hooks, and more.

**Location:** `src/tabs/Settings.tsx`

### Sections

#### 1. Workspace Folder

**Purpose:** Sets the root directory for all AutoDev agents (PA, Supervisor, Workers)

**Behavior:**
- All agents operate inside this folder
- `@path/to/file` references in chat are relative to workspace
- Default: None (agents use sandbox directories)
- UI: Folder picker dialog + clear button

**Storage:** `workspaceStore.ts` (localStorage)

#### 2. Demo Mode

**Purpose:** Enables demo mode for testing without API keys

**Behavior:**
- Mocks LLM responses
- No actual API calls
- For UI testing and demos

#### 3. Services (Sidecar Panel)

**Purpose:** Monitor and control Python sidecar processes

**Sidecars:**
- **Core Orchestrator** — FastAPI agent runtime (port 4000)
- **Memory MCP Server** — FastMCP memory server (port 5000)
- **Redis** — Cache server (port 6379)

**Status Indicators:**
- Green dot = running
- Red dot = stopped
- Yellow dot = starting

**Actions:** Start/Stop each sidecar independently

#### 4. Submodules

**Purpose:** Display git submodule status and versions

**Submodules:**
- `core-orchestrator` — Python agent orchestration
- `memory-mcp-server` — Memory MCP server

**Information displayed:**
- Commit SHA
- Branch
- Last updated
- Remote URL

#### 5. MCP Tools

**Purpose:** Browse and manage MCP server connections

**Features:**
- List all configured MCP servers
- Show available tools from each server
- Add/remove MCP server configurations
- Test MCP server connectivity

#### 6. Providers

**Purpose:** Configure LLM provider API keys and settings

**Supported Providers:**
- Anthropic (claude-3-5-sonnet, claude-3-5-haiku, claude-3-opus)
- OpenAI (gpt-4, gpt-4-turbo, gpt-3.5-turbo)
- Google Gemini (gemini-1.5-pro, gemini-1.5-flash)
- Ollama (local models)
- Azure OpenAI
- Bedrock

**Per-Provider Settings:**
- API key (stored in OS keychain)
- Base URL (for custom endpoints)
- Model selection
- Test connection button

**Storage:** OS keychain via `keyring` crate

#### 7. Model Parameters

**Purpose:** Fine-tune model behavior

**Parameters:**
- **Temperature** (0.0 - 2.0) — Controls randomness
- **Top P** (0.0 - 1.0) — Nucleus sampling
- **Thinking Budget** — For Claude models with extended thinking
- **Reasoning Effort** — For OpenAI o1/o3 models

**Model-Specific:**
- Some parameters only apply to certain models
- UI shows/hides based on active model

#### 8. GitHub

**Purpose:** Configure GitHub integration

**GitHub PAT Card:**
- Store GitHub Personal Access Token
- Used for GitHub Issues API
- Stored in OS keychain

**GitHub MCP Card:**
- Configure GitHub MCP server connection
- Server URL, authentication
- Tool list (create_issue, list_issues, etc.)

#### 9. Workers

**Purpose:** Manage the worker fleet

**Worker Table:**
- List all workers (system + custom)
- Status indicators (idle, working, error, paused)
- Start/Stop buttons
- Edit worker configuration
- Delete workers (except system-defined)

**Worker Configuration:**
- Name, slug, type
- Model preference
- Primary repositories
- Polling interval
- Bound recipe
- Description

**Storage:** `~/.autodev/workers.db` (SQLite)

#### 10. System Prompt

**Purpose:** Edit the three PA mode system prompts

**Prompts:**
- **Base System Prompt** — Fast Development mode
- **Review System Prompt** — Review mode (adds permission consciousness)
- **Delegate System Prompt** — Delegate mode (task creation focused)

**Storage:** `~/.autodev/config.db` (SQLite)

**AUTODEV.md Integration:**
- Global AUTODEV.md injected into all modes
- Repo-specific AUTODEV.md injected when in workspace
- System prompt + AUTODEV.md = full prompt

#### 11. Hooks

**Purpose:** Configure lifecycle hooks for extensibility

**Hook Types:**
- PreToolUse, PostToolUse
- SessionStart, SessionEnd
- SubagentStart, SubagentEnd
- AgentLoopStart, AgentLoopEnd
- OnToolError, OnError

**Actions:**
- Script (bash, PowerShell, Python)
- Modify (modify tool input/output)
- HTTP (POST to external service)

**UI:**
- List all hooks
- Enable/disable hooks
- Add new hooks
- Test hook execution
- View hook logs

#### 12. Updates

**Purpose:** Check for and apply AutoDev updates

**Features:**
- Check for updates
- Download updates
- Apply updates
- View update history

---

## 7. Dashboard Panel (AutoDev Panel)

The AutoDev Panel is the central dashboard for monitoring the worker fleet and task board. It displays real-time agent status, task progress, and fleet metrics.

**Location:** `src/tabs/AutoDevPanel.tsx`

### Layout

```
┌─────────────────────────────────────────────────────────────┐
│  Agent Roster (vertical table)                              │
│  ┌───────────────────────────────────────────────────────┐ │
│  │ ● SUPERVISOR  | Idle — awaiting tasks      | 00:00  │ │
│  │ ● WORKER-BACK | #142 — Add /health endpoint | 02:34  │ │
│  │ ● WORKER-FRONT| Idle — awaiting tasks      | —      │ │
│  └───────────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│  Task Board (Kanban)                                       │
│  ┌─────────┬─────────────┬───────────┬─────────────────┐ │
│  │ Ready   │ In Progress │ QC Review │ Done Today      │ │
│  ├─────────┼─────────────┼───────────┼─────────────────┤ │
│  │ #143    │ #142        │ #140      │ #138            │ │
│  │ #144    │             │           │ #139            │ │
│  └─────────┴─────────────┴───────────┴─────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│  Metrics Footer                                            │
│  Cost Today: $2.34 | Tokens: 45.2K | Closed: 12 | QC: 85% │
└─────────────────────────────────────────────────────────────┘
```

### Agent Roster

**Purpose:** Show real-time status of all fleet workers

**Columns:**
- **Status Dot** — Green = working, gray = idle, red = error
- **Agent Name** — Uppercase slug (e.g., SUPERVISOR, WORKER-BACK)
- **Current Task** — Task ID + title if working, status message if idle
- **Elapsed Time** — MM:SS since task started or last tick

**Mode Indicators:**
- **LA (green dot)** — Mode 2 (LivingAgent via core-orchestrator)
- **M1 (yellow dot)** — Mode 1 (Rust Engine fallback)
- Glow effect on dot when actively executing

**Live Updates:**
- Refreshes every second (elapsed timers)
- Pulls live tool/file info from `agentLiveStore`
- Shows current tool being executed
- Shows current file being edited
- Warns when tool running > 2 minutes

### Task Board (Kanban)

**Columns:**
1. **Ready** — Tasks waiting for Supervisor assignment
2. **In Progress** — Worker is actively working on task
3. **QC Review** — Worker completed, QC agent reviewing
4. **Done Today** — Completed tasks (filtered by date)

**Task Cards:**
- Task ID (#N)
- Title
- Priority badge (p0/p1/p2/p3)
- Type badge (feature/bug/refactor/docs/chore)
- Assigned worker (if assigned)
- Progress bar (for in-progress tasks)
- Acceptance criteria (expandable)

**Actions:**
- Click to expand full task details
- Delete button (if task is in Ready state)
- Reassign button (if task is assigned)

### Metrics Footer

**Metrics:**
- **Cost Today** — Total LLM spend for the day
- **Tokens** — Total tokens consumed (in/out)
- **Closed** — Number of tasks completed today
- **QC Pass Rate** — Percentage of QC reviews that passed

### Live Monitor Section

**Purpose:** Real-time streaming output from active agents

**Features:**
- Shows live tool execution
- Shows file operations
- Shows bash command output
- Auto-scroll to latest
- Filter by agent

### Worker Requests Widget

**Purpose:** Display pending worker → Supervisor requests

**Request Types:**
- `decompose_audit` — Worker suggests sub-tasks
- `reassign` — Worker suggests reassignment
- `needs_info` — Worker needs clarification
- `report_issue` — Worker encountered blocking issue
- `request_review` — Worker requests QC review

**Actions:**
- Acknowledge request
- Complete request
- Reject request
- Add resolution note

### Orphan Alert

**Purpose:** Warn when orphaned tasks are detected

**Behavior:**
- Appears when tasks are stuck in `in_progress` without lock file
- Shows list of orphaned task IDs
- Auto-releases orphans back to `assigned` state

---

## 10. IDE Panel

The IDE panel provides a full-featured code editor interface with file tree, code viewer, symbol outline, and integrated Git visualization. It enables code exploration, navigation, and version control operations without leaving AutoDev.

**Location:** `src/tabs/IDE.tsx`

### Layout

```
┌──────────────────────────────────────────────────────────────────┐
│  File Tree (260px)  │  Main Editor Area           │  Git Panel     │
│                     │                            │  (360px)       │
│  ┌───────────────┐  │  ┌──────────────────────┐  │  ┌──────────┐  │
│  │ Collapsible  │  │  │ Tab Bar              │  │  │ Changes  │  │
│  │ folder       │  │  ├──────────────────────┤  │  ├──────────┤  │
│  │ browser with │  │  │ Breadcrumb Nav       │  │  │ History  │  │
│  │ git status   │  │  ├──────────────────────┤  │  ├──────────┤  │
│  │ badges       │  │  │                      │  │  │ Branches │  │
│  └───────────────┘  │  │  Monaco Code Viewer  │  │  ├──────────┤  │
│                     │  │  (read-only)         │  │  │ Stash    │  │
│  ┌───────────────┐  │  │                      │  │  ├──────────┤  │
│  │ Symbol       │  │  │                      │  │  │ Undo     │  │
│  │ Outline      │  │  │                      │  │  │ (check-  │  │
│  │ (tree-sitter)│  │  │                      │  │  │  points) │  │
│  └───────────────┘  │  ├──────────────────────┤  │  └──────────┘  │
│                     │  │ IDE Status Bar       │  │                │
│                     │  └──────────────────────┘  │                │
└─────────────────────┴────────────────────────────┴────────────────┘

FileSearch overlay (Ctrl/Cmd+P) — fuzzy file search overlay
```

### File Tree

**Location:** `src/components/ide/FileTree.tsx`

**Features:**
- **Collapsible folder browser** — Navigate workspace directory structure
- **Git status badges** — Color-coded indicators for file status:
  - Green = Modified
  - Yellow = Staged
  - Blue = Untracked
  - Red = Deleted
- **Right-click context menu** — Actions per file/folder:
  - Open
  - Copy Path
  - Rename
  - Delete
  - New File
  - New Folder
- **Drag-and-drop** — Move files by dragging onto folder
- **Auto-refresh** — Updates when files change on disk
- **Keyboard shortcuts** — Arrow keys to navigate, Enter to open

**Git Status Integration:**
- Files show git status badges
- Modified files highlighted
- Untracked files marked
- Staged files indicated

### Code Viewer

**Location:** `src/components/ide/CodeViewer.tsx`

**Features:**
- **Monaco Editor** — Full-featured code editor (VS Code's editor)
- **Read-only** — AutoDev never edits files from UI; agents use tools instead
- **Custom AutoDev Dark theme** — Tuned to industrial-dark palette
- **Syntax highlighting** — Language-aware coloring
- **Line numbers** — With active line highlighting
- **Minimap** — Code overview on right side
- **Binary file detection** — Shows size for binary files
- **Encoding detection** — UTF-8, UTF-16, etc.
- **Language detection** — Auto-detects file language
- **Goto line support** — "Open in IDE from chat" can jump to specific line

**Theme Colors:**
- Background: `#01111D`
- Foreground: `#D6DEEB`
- Comments: `#637777` (italic)
- Keywords: `#82AAFF`
- Strings: `#ECC48D`
- Numbers: `#F78C6C`
- Types: `#7FDBCA`
- Functions: `#82AAFF`

**Why Read-Only?**
- AutoDev agents edit files via `write_file` and `edit_file` tools
- UI viewer is for exploration, not editing
- Avoids stale-buffer bugs
- Simplifies state management
- Prevents conflicts with agent operations

### Tab Bar

**Location:** `src/components/ide/TabBar.tsx`

**Features:**
- **Multiple tabs** — Open multiple files simultaneously
- **Tab close button** — Close individual tabs
- **Active tab highlight** — Visual indicator for current file
- **Tab overflow** — Scroll when many tabs open
- **Git badge on tab** — Shows modified status
- **Click to activate** — Switch between open files

### Breadcrumb Navigation

**Location:** `src/components/ide/BreadcrumbNav.tsx`

**Features:**
- **Path display** — Shows file path as clickable breadcrumbs
- **Navigate hierarchy** — Click any folder to navigate
- **Compact display** — Truncates long paths
- **Workspace-relative** — Shows path relative to workspace

### Symbol Outline

**Location:** `src/components/ide/SymbolOutline.tsx`

**Features:**
- **Tree-sitter parsing** — Extracts symbols from code
- **Symbol types** — Functions, classes, methods, types, modules
- **Filter input** — Search symbols by name
- **Click to jump** — Clicking symbol jumps to its line in Monaco
- **Language-aware** — Different symbols per language
- **Icon per kind** — Visual distinction:
  - Code2 = Function
  - Hash = Variable
  - Layers = Class
  - Package = Module
  - TypeIcon = Type/Interface

**Supported Languages:**
- Rust
- TypeScript/JavaScript
- Python
- Go
- Java
- C/C++
- More via tree-sitter

### File Search

**Location:** `src/components/ide/FileSearch.tsx`

**Keyboard Shortcut:** `Ctrl/Cmd+P`

**Features:**
- **Fuzzy search** — Find files by name
- **Recent files** — Shows recently opened files when query empty
- **Debounced** — 120ms debounce to avoid hammering backend
- **Keyboard navigation** — Arrow keys to navigate, Enter to open, Escape to close
- **Max 40 results** — Limits results for performance
- **Glob support** — Can search with patterns (future)

**Search Results:**
- File name
- Relative path
- Directory/file icon
- Click to open in tab

### IDE Status Bar

**Location:** `src/components/ide/IdeStatusBar.tsx`

**Features:**
- **File path** — Shows current file path
- **Encoding** — UTF-8, UTF-16, etc.
- **Line count** — Number of lines in file
- **File size** — Formatted (B, KB, MB)
- **Language** — Detected language (colored accent)
- **Cursor position** — Reserved for future cursor tracking

### Git Panel

**Location:** `src/components/git/GitPanel.tsx`

**Toggle:** Click "Git" button in tab header (or `GitBranch` icon)

**Width:** 360px (min 280px)

**Five Tabs:**

#### 1. Changes Tab

**Features:**
- **Staged files** — Files ready to commit
- **Unstaged files** — Modified but not staged
- **Untracked files** — New files not tracked
- **Diff preview** — Click file to see diff in DiffViewer
- **Stage button** — Stage individual files
- **Unstage button** — Unstage files
- **Discard button** — Discard changes
- **Commit form** — Commit message + trailer (default: "AutoDev/PA")
- **Commit button** — Create commit

**Diff Viewer:**
- Side-by-side diff view
- Red/green highlighting
- Line numbers
- Context lines
- Unified view option

#### 2. History Tab

**Features:**
- **Commit log** — List of all commits
- **Commit details** — SHA, author, date, message
- **Branch badges** — Shows branch for each commit
- **Relative time** — "2 hours ago", "yesterday"
- **Click to see diff** — View commit changes
- **Fetch/Push/Pull** — Sync with remote
- **Refresh** — Reload history

#### 3. Branches Tab

**Location:** `src/components/git/BranchManager.tsx`

**Features:**
- **Local branches** — List of local branches
- **Remote branches** — List of remote branches
- **Current branch** — Highlighted
- **Create branch** — New branch dialog
- **Delete branch** — Delete local branch
- **Checkout branch** — Switch to branch
- **Ahead/behind indicators** — Shows commits ahead/behind remote

#### 4. Stash Tab

**Location:** `src/components/git/StashManager.tsx`

**Features:**
- **Stash list** — All stashed changes
- **Stash message** — Shows stash description
- **Push stash** — Save current changes to stash
- **Apply stash** — Apply stashed changes
- **Drop stash** — Delete stash
- **Pop stash** — Apply and delete stash

#### 5. Undo Tab (Checkpoints)

**Location:** `src/components/git/CheckpointViewer.tsx`

**Features:**
- **Auto-snapshots** — Automatic checkpoints before agent operations
- **Timeline view** — Visual timeline of checkpoints
- **Checkpoint details** — Timestamp, description, files changed
- **Restore checkpoint** — Revert to checkpoint state
- **Delete checkpoint** — Remove checkpoint
- **Checkpoint metadata** — Agent, task, operation that created it

**Checkpoint Use Cases:**
- Undo agent mistakes
- Revert to known good state
- Compare before/after states
- Debug agent operations

### Git Integration

**Backend:** `src-tauri/src/git/` (git2-rs bindings)

**Features:**
- **Repository detection** — Auto-detects git repos
- **Status computation** — Computes git status efficiently
- **Diff generation** — Generates diffs for files
- **Commit creation** — Creates commits with metadata
- **Branch management** — List, create, delete, checkout branches
- **Stash operations** — Push, apply, drop stashes
- **Remote operations** — Fetch, pull, push
- **Checkpoint system** — Auto-snapshots for undo

**Git Store:**
- Zustand store for git state
- Caches repository handle
- Reactive updates on git operations
- Error handling for non-git repos

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl/Cmd+P` | Open file search overlay |
| `Ctrl/Cmd+S` | Not applicable (read-only editor) |
| `Ctrl/Cmd+F` | Find in file (Monaco built-in) |
| `Ctrl/Cmd+G` | Find next (Monaco built-in) |
| `Ctrl/Cmd+Shift+F` | Find in files (future) |
| `Ctrl/Cmd+B` | Toggle sidebar (future) |
| `Escape` | Close overlay/modal |

### Use Cases

**For PA Mode:**
- Explore codebase structure
- Read files to understand context
- Navigate to specific lines via "Open in IDE from chat"
- Review agent changes via Git panel
- Undo agent mistakes via Checkpoints

**For Fleet Workers:**
- Workers don't use IDE directly (headless)
- Git operations done via tools (git2-rs)
- Checkpoints created automatically before operations
- Supervisor can review diffs via Git panel

**For Developers:**
- Full code exploration without external IDE
- Git operations integrated
- Visual diff review
- Symbol outline for navigation
- Quick file search

### Future Enhancements (Planned)

- **Cursor tracking** — Show cursor position in status bar
- **Find in files** — Global search across workspace
- **Refactoring support** — Rename symbols, extract functions
- **Multi-file diff** — Compare multiple files
- **Blame view** — Git blame per line
- **Merge conflict resolution** — Visual merge tool
- **Terminal integration** — Embedded terminal panel
- **Debug integration** — Breakpoints, step-through

---

## 11. Graph Panel

The Graph panel provides a D3.js force-directed visualization of the code knowledge graph. It enables visual exploration of code structure, dependencies, and relationships.

**Location:** `src/tabs/Graph.tsx`

### What is the Graph?

The code knowledge graph is a structured representation of your codebase as nodes and edges:

**Nodes** represent code entities:
- Files
- Classes
- Functions/Methods
- Tests
- Types/Interfaces
- Modules/Packages
- Imports

**Edges** represent relationships:
- CALLS — Function calls
- IMPORTS_FROM — Import dependencies
- INHERITS — Class inheritance
- IMPLEMENTS — Interface implementation
- TESTED_BY — Test coverage
- DEPENDS_ON — Module dependencies
- REFERENCES — Symbol references
- CONTAINS — File containment
- SEMANTICALLY_SIMILAR — Semantic similarity

### Graph Building

**Trigger:** Click "Build Graph" button

**Process:**
1. **Detection** — Scan workspace for code files
2. **Extraction** — Parse files with language-specific extractors (tree-sitter)
3. **Clustering** — Group related nodes into communities (Leiden algorithm)
4. **Analysis** — Compute graph metrics (density, centrality, cohesion)
5. **Storage** — Save to SQLite database

**Progress:** Shown as progress bar with phases (scanning, extracting, clustering, analyzing)

### Visualization Controls

#### Toolbar Row 1

**Workspace Path:** Shows current workspace directory

**Build Button:** Triggers graph build process

**Update Button:** Incremental update (only changed files)

**Export Dropdown:**
- Export as JSON
- Export as GEXF (Gephi format)
- Export as SVG image

**Fit Button:** Reset zoom and center graph

**Delete Button:** Clear graph data

#### Toolbar Row 2

**Render Mode:**
- **Full** — Show all nodes
- **Community** — Show community aggregates (grouped nodes)
- **Auto** — Automatically switch based on node count

**Edge Filters:** Toggle edge types on/off
- CALLS (green)
- IMPORTS_FROM (blue)
- INHERITS (purple)
- IMPLEMENTS (teal)
- TESTED_BY (orange)
- DEPENDS_ON (yellow)
- REFERENCES (gray)
- CONTAINS (dark gray)
- SEMANTICALLY_SIMILAR (brown)

**Search:** Find nodes by name or file path

**Min Degree:** Filter nodes by minimum connections (default: 1)

### Stats Bar

**Metrics Displayed:**
- Total nodes
- Total edges
- Total communities
- Average degree
- Graph density
- Last updated timestamp

### Node Visualization

**Shapes by Kind:**
- Circle — Function/Method
- Square — Class
- Diamond — File
- Triangle — Test
- Star — Type/Interface
- Wye (Y-shape) — Module/Package
- Cross — Import

**Size:** Based on degree (number of connections)
- `nodeRadius(degree) = min(22, 4 + log(1+degree)*3.5)`
- More connections = larger node

**Colors:**
- Community-based coloring (10-color palette)
- Nodes in same community share color

**God Nodes:** Nodes with degree ≥ 15 get outer glow ring

### Edge Visualization

**Colors by Kind:**
- CALLS — #00FF88 (green)
- IMPORTS_FROM — #4477FF (blue)
- INHERITS — #B07AA1 (purple)
- IMPLEMENTS — #76B7B2 (teal)
- TESTED_BY — #F28E2B (orange)
- DEPENDS_ON — #EDC948 (yellow)
- REFERENCES — #888888 (gray)
- CONTAINS — #2A2A2A (dark gray)
- SEMANTICALLY_SIMILAR — #9C755F (brown)

**Arrowheads:**
- Most edges have arrowheads showing direction
- CONTAINS and SEMANTICALLY_SIMILAR have no arrowheads

### Interactions

**Click Node:** Focus mode — dims non-neighbors, highlights 1-hop ring

**Right-Click Node:** Context menu
- Open file in IDE
- Show incoming edges
- Show outgoing edges
- Find in codebase
- Copy node ID

**Drag Node:** Reposition node (force simulation adjusts)

**Scroll:** Zoom in/out

**Pan:** Click and drag background

**Hover Node:** Tooltip with node details (name, kind, file, community, degree)

### Community Visualization

**Community Hulls:** Colored polygons around nodes in same community

**Cohesion Score:** How tightly connected nodes are within community
- Green (> 0.5) — High cohesion
- Yellow (0.25 - 0.5) — Medium cohesion
- Red (< 0.25) — Low cohesion

**Community Panel (Right Sidebar):**
- List all communities
- Show member count
- Show cohesion score
- Click to filter to community

### Node Inspector (Right Sidebar)

**Selected Node Info:**
- Name, kind, language
- File path
- Community
- Degree

**Incoming Edges:**
- List of nodes that reference this node
- Edge kind badges
- Click to navigate to source

**Outgoing Edges:**
- List of nodes this node references
- Edge kind badges
- Click to navigate to target

### Analysis Buttons

**Centrality:** Find most central nodes (high betweenness centrality)

**Bridges:** Find bridge nodes (connect communities)

**Clusters:** Find tightly-knit clusters

**Isolates:** Find disconnected nodes

### Use Cases

**Impact Analysis:**
- Select a node → see what depends on it
- Understand ripple effects of changes

**Code Navigation:**
- Search for function → see its callers
- Navigate through call graph

**Architecture Review:**
- Identify tightly-coupled modules (high degree)
- Find architectural boundaries (communities)

**Test Coverage:**
- Filter by TESTED_BY edges
- Find untested code

**Refactoring:**
- Identify circular dependencies
- Find opportunities to extract modules

---

## 12. Technical Internals

### Cron Scheduler

**Implementation:** `src-tauri/src/recipes/scheduler.rs`

**Architecture:**
- Uses `tokio` tasks for background execution
- `cron` crate for POSIX cron expression parsing
- `Arc<RecipeScheduler>` stored in Tauri managed state (lives for app lifetime)
- `HashMap<String, JoinHandle<()>>` maps worker_slug → running task

**Startup Sequence:**
1. Wait for core-orchestrator to be ready (polls every 2s for up to 120s)
2. Fire immediate startup tick (regardless of cron schedule)
3. If startup was Mode 1 (orch not ready), fire Mode 2 catch-up tick when orch becomes ready
4. Enter cron loop

**Mode 2 Catch-Up Tick:**
- Handles race where some workers start before orchestrator
- Ensures all workers get Mode 2 ticks and create their YAML configs
- Prevents "missing from fleet" issue

**Idempotent Start:**
- `start_worker()` aborts existing task before starting new one
- Safe to call multiple times (restarts worker)

### Recipe Runner

**Implementation:** `src-tauri/src/recipes/runner.rs`

**Dispatch Decision:**
```
Health check → core-orchestrator reachable?
    ↓ Yes
Mode 2 (LivingAgent via HTTP POST)
    ↓ No
Mode 1 (Rust AgentEngine)
```

**No-Op Short-Circuit (Mode 1 only):**
- If worker has no assigned/in_progress tasks, skip tick
- Prevents LLM from hallucinating work on empty board
- NOT applied in Mode 2 (always dispatch to ensure_agent() runs)

**Model Resolution:**
Priority: worker.model (DB) > recipe.model (YAML) > active_model > default

**Workspace Directory:**
Priority: configured workspace > sandbox fallback (~/.autodev/agents/<slug>/)

**Ephemeral Sessions:**
- Each tick creates fresh session ID
- Not persisted to session DB
- No SSE bridging (future work)

### Sidecar Management

**Implementation:** `src-tauri/src/sidecar/mod.rs`

**Sidecars:**
1. **Memory MCP Server** — Python FastMCP server (port 5000)
2. **Core Orchestrator** — Python FastAPI agent runtime (port 4000)
3. **Redis** — Standalone binary (port 6379)

**Startup Sequence:**
1. Start Redis (if not running)
2. Start Memory MCP Server (waits for log pattern "Qdrant" or "Using Memory")
3. Start Core Orchestrator (waits for log patterns)
4. Emit startup progress events to UI

**Log Router:**
- Captures stdout/stderr from all sidecars
- Parses log patterns to detect startup phases
- Emits Tauri events for UI updates

**Health Checks:**
- HTTP GET /health on core-orchestrator
- TCP connection test on memory-mcp
- Process existence check for Redis

**Startup Progress FSM:**
12 phases: Init → Skills → MemoryStarting → VectorDb → MemoryReady → OrchStarting → Routers → Agents → LivingAgents → OrchReady → Workers → Ready

### Fleet Management

**Worker Registry:**
- Database: `~/.autodev/workers.db` (SQLite)
- Schema: workers table with id, name, slug, type, status, model, recipe_id, etc.
- Seeds 7 system workers on first open
- CRUD operations via Tauri commands

**Worker Status:**
- Created (seeded, not started)
- Idle (started, polling, no task)
- Active (recipe cron active)
- Working (executing task)
- Paused (user paused)
- Error (recipe crashed)
- Deleted (soft-deleted)

**Task Assignment:**
- Supervisor recipe polls task board
- Calls `assign_task__autodev` to assign worker
- Worker recipe polls for assigned tasks
- Worker claims task via lock file

**Worker Communication:**
- `worker_requests` table for worker → Supervisor messages
- `request_supervisor_action` tool for workers to request help
- Supervisor processes requests on each tick

### Orphan Detection

**Implementation:** `src-tauri/src/orphan/mod.rs`

**Purpose:** Detect tasks stuck in `in_progress` without active lock file

**Detection:**
- Query tasks with status `in_progress`
- Check if lock file exists for each task
- If no lock file, task is orphaned (worker crashed)

**Resolution:**
- Release task back to `assigned` status
- Emit `orphans_released` event to UI
- OrphanAlert widget shows released tasks

**Trigger:**
- Runs automatically before Supervisor tick
- Also available via Tauri command

### Tool Execution

**Implementation:** `src-tauri/src/agent/tool_executor.rs`

**Partitioned Execution:**
- Read-only tools (read_file, grep, glob, list_directory) execute concurrently (max 10)
- Write tools (write_file, edit_file, bash) execute serially
- Uses `tokio::sync::Semaphore` for concurrency control

**Permission Modes:**
- Autonomous — no prompts (for worker agents)
- Manual Approval — prompt user for each write
- Smart Approve — auto-approve read-only, prompt for write

**Tool Registry:**
- Filters tools based on PA mode
- Delegate mode: only `create_task` + MCP read-only
- Fast Development: all tools
- Review: all tools + permission wrapper

### Context Building

**Implementation:** `src-tauri/src/agent/context.rs`

**System Prompt Assembly:**
1. Base system prompt (mode-specific)
2. Global AUTODEV.md (from SQLite or filesystem)
3. Repo-specific AUTODEV.md (from SQLite or filesystem)
4. Memory context (if session_id attached)

**AUTODEV.md Storage:**
- Primary: SQLite database (`~/.autodev/config.db`)
- Fallback: Filesystem (`~/.autodev/config/autodev_global.md`, `.autodev/autodev.md`)
- UI-managed via AUTODEV.md editor in Settings

### Memory Integration

**Implementation:** `src-tauri/src/memory/`

**3-Tier Memory:**
1. **Episodic** — Conversation history, tool executions
2. **Semantic** — Vector embeddings, retrieval-augmented generation
3. **Procedural** — Skills, recipes, workflows

**Context Building:**
- `build_memory_context()` recalls relevant memories
- Token-aware budget management
- Intent detection for targeted recall

**Memory Providers:**
- mem0 (primary)
- Redis (warm cache)
- MongoDB (alternative)
- ChromaDB (alternative)
- Vertex AI (alternative)

---

## 13. Submodules

AutoDev uses git submodules for the Python sidecar components. This allows independent development and versioning of the orchestrator and memory systems.

### Submodule Structure

```
autodev/
├── .gitmodules              # Submodule configuration
├── submodules/
│   ├── core-orchestrator/   # Python agent orchestration
│   └── memory-mcp-server/   # Memory MCP server
```

### Core Orchestrator

**Purpose:** Python-based agent runtime with LivingAgent, FSMs, A2A protocol

**Key Components:**
- `src/airs/agent/living_agent.py` — LivingAgent (2425 lines)
- `src/airs/memory/memory_core.py` — MemoryCore (2654 lines)
- `src/airs/state/` — 21 finite state machines
- `src/airs/bridges/` — Unified tool handler, MCP bridge, memory compiler
- `src/airs/mcp/` — MCP client system

**Runtime:** Python 3.13+ / FastAPI / uvicorn on port 4000

### Memory MCP Server

**Purpose:** FastMCP-based server exposing 6 memory tools

**Key Components:**
- `src/mcp_server/server.py` — FastMCP server (1093 lines)
- `src/memory_providers/` — Multi-provider memory manager
- Circuit breaker pattern with automatic failover

**Runtime:** Python 3.13+ / FastMCP / Starlette on port 5000

### Submodule Workflow

**Editing a submodule:**
```bash
cd submodules/core-orchestrator
git checkout main
git pull origin main
git checkout -b fix/your-feature
# Make changes
git add .
git commit -m "fix: your changes"
git push origin fix/your-feature
```

**Updating pointer in parent repo:**
```bash
cd ../..  # back to autodev root
git add submodules/core-orchestrator
git commit -m "chore: bump core-orchestrator (fix included)"
```

**Updating to latest:**
```bash
git submodule update --remote --merge
```

**Important:**
- Submodules are separate git repos with their own history
- Parent repo tracks commit SHA pointers, not file contents
- Always commit in submodule first, then update pointer in parent
- Do NOT run `uv sync` in main autodev repo (not a Python project)
- Only run `uv sync` inside submodules when developing them locally

### Submodule Status

**Viewing status:**
```bash
git submodule status
```

**Output:**
- First character: `+` if different from expected commit
- Second character: `U` if unmerged
- SHA: Current commit
- Description: Branch/tag

### Troubleshooting

**Detached HEAD:**
```bash
cd submodules/core-orchestrator
git checkout main
```

**Submodule not initialized:**
```bash
git submodule update --init --recursive
```

**Wrong commit:**
```bash
cd submodules/core-orchestrator
git fetch origin
git checkout origin/main
cd ../..
git add submodules/core-orchestrator
git commit -m "chore: reset submodule to main"
```

---

## 14. Glossary

### A

**A2A Protocol** — Agent-to-Agent communication protocol used by core-orchestrator for supervisor ↔ worker coordination

**AUTODEV.md** — Proprietary AIRS instruction system, stored in SQLite, editable from UI, injected into system prompt

### B

**Bash Tool** — Built-in tool for executing shell commands (tests, linting, git operations)

### C

**Class Node** — Graph node representing a class definition

**Community** — Group of related nodes in the graph (clustered via Leiden algorithm)

**Core Orchestrator** — Python sidecar providing LivingAgent, FSMs, A2A protocol (port 4000)

### D

**Delegate Mode** — PA mode where agent creates tasks instead of executing code (workers are live)

### E

**Edit File Tool** — Built-in tool for targeted file edits (preferred over write_file)

### F

**Fast Development Mode** — PA mode with full tool access, no permission prompts

**File Node** — Graph node representing a source file

**Force-Directed Graph** — D3.js simulation where nodes repel each other and edges pull connected nodes together

**FSM** — Finite State Machine (core-orchestrator has 21 FSMs for lifecycle management)

### G

**Git Worktree** — Isolated working directory for each task (prevents main branch pollution)

**GitHub MCP** — MCP server providing GitHub Issues and PR tools

**Glob Tool** — Built-in tool for finding files matching patterns

**Grep Tool** — Built-in tool for searching files with regex patterns

### H

**Hook** — Script that fires at lifecycle events (pre/post tool call, session start/end)

### I

**Internal Task Board** — SQLite database (`~/.autodev/tasks.db`) for task assignment

### L

**Leiden Algorithm** — Graph clustering algorithm used to detect communities

**LivingAgent** — Core orchestrator's central agent class with 21 FSMs and memory integration

**Lock File** — `current_tasks/issue-N.lock` — worker writes before starting task to prevent conflicts

### M

**MCP** — Model Context Protocol — standard for connecting AI tools to external services

**Memory MCP Server** — Python sidecar providing memory tools (port 5000)

**Memory Core** — 3-tier memory system (episodic/semantic/procedural)

### N

**Node** — Graph entity (file, class, function, test, type, module, import)

### O

**Orphan** — Task stuck in `in_progress` without active lock file (worker crashed)

### P

**PA Mode** — Personal Assistant mode (Rust-native execution for interactive use)

**PA Mode Toggle** — Dropdown to switch between Fast Development, Review, and Delegate modes

**Permission Mode** — Autonomous / Manual Approval / Smart Approve / Chat Only

### R

**Recipe** — YAML-defined workflow with cron scheduling for autonomous agent execution

**Recipe Runner** — Executes recipes as background agent sessions

**Recipe Scheduler** — Manages cron-based recipe execution for worker fleet

**Redis** — Standalone binary for warm-tier memory caching (port 6379)

**Review Mode** — PA mode with permission checkpoints before write operations

### S

**Sidecar** — Python process bundled with AutoDev (core-orchestrator, memory-mcp-server)

**Subagent** — Isolated child agent with own context, tools, and MCP servers

**Supervisor** — Fleet agent that polls task board, assigns workers, reviews QC reports

### T

**Task Board** — Kanban board showing tasks by status (Ready, In Progress, QC Review, Done)

**Tool Registry** — Maps tool names to tool implementations, filtered by PA mode

**Tool Result** — Structured output from tool execution with `output`, `is_error`, `hint` fields

### V

**Vector DB** — Qdrant or similar for semantic memory storage and retrieval

### W

**Worker** — Autonomous agent that executes assigned tasks (backend, frontend, platform, etc.)

**Worker Request** — Message from worker to Supervisor (decompose_audit, reassign, needs_info, etc.)

**Write File Tool** — Built-in tool for writing complete file contents (new files or major rewrites only)

---

**Document Version:** 1.0  
**Last Updated:** 2026-05-12  
**Maintained By:** AIRS Foundry Development Team
