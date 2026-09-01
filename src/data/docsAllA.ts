// Documentation — Sections 01–04
// Full content faithful to AUTODEV_DOCUMENTATION.md

import type { DocSection } from './docs'

export const A_SECTIONS: DocSection[] = [

  /* ── 01: What is AutoDev? ──────────────────────────────────────── */
  {
    id: 'what', num: '01', title: 'What is AutoDev?', category: 'Overview', icon: 'info-circle',
    lead: 'A downloadable desktop application combining Claude Code and Goose into a single, branded, model-agnostic AI developer agent platform. Purpose-built for AIRS Foundry\'s AutoDev worker pipeline.',
    anchors: ['Vision', 'Why Build This?', 'Key Features', 'Tech Stack'],
    blocks: [
      { t: 'h2', text: 'Vision' },
      { t: 'callout', text: 'One app. Any model. Full autonomy.' },
      { t: 'prose', text: 'Any AIRS team member or AutoDev worker agent can: download one binary, enter their LLM API key (any provider), select their model, and start executing agentic development tasks with the full capability stack: AUTODEV.md, Skills, Recipes, Subagents, MCP servers, Hooks, and the AutoDev worker pipeline.' },
      { t: 'h2', text: 'Why Build This?' },
      { t: 'h3', text: 'Claude Code Limitations' },
      { t: 'list', items: ['Locked to Anthropic subscriptions — rate limits cripple 24/7 worker agents', '512,000-line source leak (March 31, 2026) exposed vulnerabilities', 'No model flexibility — cannot route workers to cheaper models', 'Proprietary — cannot audit, fork, or adapt'] },
      { t: 'h3', text: 'Goose Limitations' },
      { t: 'list', items: ['AGENTS.md ecosystem is newer and less mature', 'Not tailored to AutoDev\'s Supervisor → Worker → QC pipeline', 'Generic tool with no AIRS-specific automation'] },
      { t: 'h3', text: 'AutoDev Advantage' },
      { t: 'list', items: ['Model-agnostic by design (Google Gemini, Anthropic, or OpenAI)', 'AUTODEV.md-native with SQLite-backed instruction system', 'AutoDev-native GitHub Issues board understanding', 'Secure and auditable (AIRS controls the harness)', 'Downloadable and distributable single installer'] },
      { t: 'h2', text: 'Key Features' },
      { t: 'list', items: [
        'Model-agnostic: Google Gemini, Anthropic, or OpenAI',
        'Advanced memory system: 3-tier (episodic/semantic/procedural) with context scoring, compression, staleness tracking',
        'Agent orchestration: 21 finite state machines governing lifecycle + A2A protocol',
        'AUTODEV.md system: SQLite-backed, editable from UI, no manual file editing required',
        'MCP native: Connect to GitHub, Slack, Adobe, and custom MCP servers',
        'Skills & Recipes: Reusable YAML workflows with cron scheduling for fleet orchestration',
        'Single installer: No Python, Redis, or Docker setup required for end users',
        'Dual-mode execution: PA mode (Rust-native for speed) + Worker mode (orchestrated via core-orchestrator)',
        'Git integration: Structured git operations via git2-rs with worktree isolation per task',
        'Cost tracking: Real-time token spend per session with per-agent, per-task reporting',
      ]},
      { t: 'h2', text: 'Tech Stack' },
      { t: 'table', headers: ['Layer', 'Technology', 'Purpose'], rows: [
        ['Desktop Shell', 'Tauri v2 (Rust)', 'Cross-platform native wrapper (~15MB base)'],
        ['Frontend', 'React 19 + TypeScript + Vite 8', 'Agent UI, chat, code editor, task board'],
        ['State Management', 'Zustand', 'Global state (sessions, config, agent status)'],
        ['Styling', 'Tailwind CSS 3', 'AIRS design system (industrial-minimal, #00FF88 accent)'],
        ['Backend', 'Rust (Tauri commands)', 'Code editing tools, file I/O, command execution'],
        ['Provider Layer', 'Anthropic, OpenAI, Google', 'Multi-provider LLM abstraction'],
        ['MCP Health', 'reqwest', 'Liveness probes for user-configured external MCP servers'],
        ['Git Integration', 'git2-rs (libgit2)', 'Structured git operations, worktrees'],
        ['Orchestrator', 'Python 3.13 + FastAPI + litellm', 'Agent lifecycle, FSMs, A2A protocol'],
        ['Memory', 'Python 3.13 + mem0', 'Context storage, scoring, synopsis evolution'],
        ['Cache', 'Redis 7+', 'Warm-tier memory caching'],
        ['Keychain', 'keyring crate', 'OS keychain integration for API keys'],
        ['Database', 'SQLite (rusqlite)', 'Session persistence, task board, worker registry'],
      ]},
    ],
  },

  /* ── 02: Core Architecture ─────────────────────────────────────── */
  {
    id: 'arch', num: '02', title: 'Core Architecture', category: 'Overview', icon: 'topology-star',
    lead: 'AutoDev is built on Tauri v2 (Rust + React) and bundles two Python sidecars as git submodules. It operates in dual mode: PA (Rust-native) and Worker (Python core-orchestrator).',
    anchors: ['High-Level Diagram', 'Mode 1: PA', 'Mode 2: Worker', 'Tech Stack'],
    blocks: [
      { t: 'h2', text: 'High-Level Architecture Diagram' },
      { t: 'code', lang: 'text', code: `┌─────────────────────────────────────────────────────────────────┐
│                     AutoDev Desktop App                         │
├──────────────────────┬──────────────────────────────────────────┤
│   React/TS Frontend  │         Tauri v2 Rust Backend            │
│   (src/)             │         (src-tauri/)                     │
│                      │                                          │
│   ┌─────────────┐    │    ┌──────────────────────────────────┐  │
│   │ Zustand      │◄──invoke──►│ Tauri Commands              │  │
│   │ Store        │    │    │  ├── agent::*                    │  │
│   ├─────────────┤    │    │  ├── provider::*                 │  │
│   │ Tab Views    │    │    │  ├── vault::*  ├── settings::*  │  │
│   │ ├── Chat     │    │    │  └── sidecar::*                 │  │
│   │ ├── AutoDev  │    │    ├──────────────────────────────────┤  │
│   │ ├── IDE      │    │    │ Core Modules                    │  │
│   │ ├── Recipes  │    │    │  ├── agent/   (tool exec loop)  │  │
│   │ ├── Graph    │    │    │  ├── mcp/     (MCP client)       │  │
│   │ └── Settings │    │    │  ├── recipes/ (YAML + cron)     │  │
│   └─────────────┘    │    │  ├── git/     (git2-rs)         │  │
│                      │    │  ├── graph/   (knowledge graph) │  │
│                      │    │  └── db/      (SQLite CRUD)      │  │
│                      │    └──────────────────────────────────┘  │
│                      │    ┌──────────────────────────────────┐  │
│                      │    │     Sidecar Manager               │  │
│                      │    │  core-orchestrator (Python :4000) │  │
│                      │    │  memory-mcp-server (Python :5000) │  │
│                      │    │  Redis (standalone binary  :6379) │  │
│                      │    └──────────────────────────────────┘  │
└──────────────────────┴──────────────────────────────────────────┘` },
      { t: 'h2', text: 'Mode 1: PA (Personal Assistant) — Rust-Native' },
      { t: 'prose', text: 'Used by: Razi, team members, developers using the desktop app interactively.' },
      { t: 'code', lang: 'text', code: `User → Tauri (Rust) → Direct LLM API (Anthropic/OpenAI/etc.)
                    ↓
              Built-in tools (read_file, write_file, bash, git2-rs)
                    ↓
              External MCP servers — configured and health-checked, not yet called
                    ↓
              Response → UI` },
      { t: 'h3', text: 'Why NOT delegate to core-orchestrator for PA:' },
      { t: 'list', items: [
        'Lower latency — no HTTP round-trip (100–500ms saved per turn)',
        'Simpler deployment — works even if Python sidecars fail to start',
        'Settings UI — test provider API keys without starting core-orchestrator',
        'Headless CLI — airsctl -p "..." runs standalone, no Python dependency',
        'Local-first — sessions, memory and the task board are all on-device',
      ]},
      { t: 'callout', text: 'Limitations: No A2A, no mem0 semantic memory, no 21 FSMs, no context scoring — intentionally kept simple.' },
      { t: 'h2', text: 'Mode 2: Worker Agent — Orchestrated via Core Orchestrator' },
      { t: 'prose', text: 'Used by: AutoDev worker agents (backend, frontend, cleanup, etc.), Supervisor, QC agent running recipes.' },
      { t: 'code', lang: 'text', code: `Recipe trigger → Tauri → HTTP POST /api/v1/agent/invoke → core-orchestrator
                                                         ↓
                                                   LivingAgent (21 FSMs)
                                                         ↓
                                                   litellm provider
                                                         ↓
                                                   airs_mcp client → external MCPs
                                                         ↓
                                                   mem0 memory → Redis
                                                         ↓
                                                   A2A protocol → SSE stream → UI` },
      { t: 'h3', text: 'Why delegate to core-orchestrator for workers:' },
      { t: 'list', items: [
        'A2A protocol — supervisor ↔ workers coordination',
        'mem0 semantic memory — long-term context, retrieval-augmented generation',
        '21 FSMs — agent lifecycle, tool execution tracking, memory management',
        'Context scoring — relevance ranking, smart compaction',
        'Feedback-driven synopsis — conversation evolution',
        'Too complex to reimplement in Rust; Python ecosystem advantage',
      ]},
    ],
  },

  /* ── 03: Personal Assistant (PA) Mode ─────────────────────────── */
  {
    id: 'pa-mode', num: '03', title: 'Personal Assistant (PA) Mode', category: 'Agent Modes', icon: 'robot',
    lead: 'The PA is the primary interface for interactive AI-assisted development. Runs in Mode 1 (Rust-Native) for low-latency, direct LLM API access. Three sub-modes control tool availability and system prompt behavior.',
    anchors: ['Mode Toggle', 'Fast Development', 'Review Mode', 'Delegate Mode', 'Interaction Flow', 'Switching Modes'],
    blocks: [
      { t: 'h2', text: 'Mode Toggle' },
      { t: 'table', headers: ['Mode', 'Enum Variant', 'Tool Registry', 'System Prompt', 'Use Case'], rows: [
        ['⚡ Fast Development', 'PAMode::FastDevelopment', 'All builtin_tools + All MCP tools', 'BASE_SYSTEM_PROMPT', 'Razi coding directly, quick fixes'],
        ['✋ Review Mode', 'PAMode::Review', 'All builtin_tools + permission_check wrapper + MCP', 'REVIEW_SYSTEM_PROMPT', 'Reviewing changes, cautious editing'],
        ['📋 Delegate Mode', 'PAMode::Delegate', 'Only create_task + MCP read-only', 'DELEGATE_SYSTEM_PROMPT', 'Workers are live, PA delegates work'],
      ]},
      { t: 'h2', text: 'Fast Development Mode' },
      { t: 'prose', text: 'System Prompt: BASE_SYSTEM_PROMPT (~200 lines comprehensive). Full file and execution access, no explicit permission prompts. Agents read before writing, verify after changes. Surgical edits preferred over wholesale rewrites.' },
      { t: 'h3', text: 'Available Tools' },
      { t: 'list', items: [
        'Reading & Discovery: read_file, grep, glob, list_directory',
        'Writing & Editing: edit_file (preferred), write_file (new files/major rewrites)',
        'Execution: bash (tests, linting, git operations)',
        'Git: All git operations (status, diff, commit, push, pull, worktree)',
        'MCP Tools: GitHub, Slack, and any configured MCP servers',
      ]},
      { t: 'h2', text: 'Review Mode' },
      { t: 'prose', text: 'System Prompt: REVIEW_SYSTEM_PROMPT (adds permission consciousness). Same tool set as Fast Development plus explicit permission checkpoint before write operations.' },
      { t: 'list', items: [
        'Adds explicit permission checkpoint before write operations',
        'Requires reasoning through implications before changes',
        'One change at a time — no batching',
        'Logs reasoning for each write operation',
      ]},
      { t: 'h2', text: 'Delegate Mode' },
      { t: 'prose', text: 'System Prompt: DELEGATE_SYSTEM_PROMPT (task creation focused). NO file access tools (no read_file, write_file, edit_file, bash). Decomposes user requests into atomic tasks and creates them on the internal AutoDev board.' },
      { t: 'list', items: [
        'create_task (writes to internal AutoDev task board)',
        'MCP tools (read-only only — GitHub MCP for context, no write tools)',
        'NO direct code execution whatsoever',
      ]},
      { t: 'h2', text: 'Delegate Mode Interaction Flow' },
      { t: 'code', lang: 'text', code: `1. User: "Add /health endpoint to orchestrator"
2. PA (Delegate Mode): Understands intent → decomposes → creates task #142
3. PA confirms: "Task #142 created. Supervisor will assign to Worker Backend on next poll."
4. Supervisor recipe (every 2 min): Sees task #142, assigns to Worker Backend
5. Worker Backend recipe: Executes via core-orchestrator, implements endpoint, opens PR
6. QC Agent recipe: Reviews PR, updates task status to "qc-approved"
7. Supervisor: Sees QC approval, instructs worker to merge, closes task #142
8. PA (if monitoring AutoDev Panel): Shows task completion notification` },
      { t: 'h2', text: 'Switching PA Modes' },
      { t: 'prose', text: 'Location: Chat header dropdown (3-button segmented control). State Management: settingsStore.ts persists paMode to localStorage. Effect: Changes tool registry and system prompt for the next message sent. Persistence: Mode preference saved across sessions.' },
    ],
  },

  /* ── 04: Fleet & Delegate Mode ─────────────────────────────────── */
  {
    id: 'fleet', num: '04', title: 'Fleet & Delegate Mode', category: 'Agent Modes', icon: 'users',
    lead: 'The AutoDev fleet is a collection of autonomous worker agents that execute tasks assigned by a Supervisor agent. This multi-agent pipeline enables 24/7 autonomous development.',
    anchors: ['Fleet Architecture', 'Worker Types', 'Internal Task Board', 'Task Lifecycle', 'Worker Requests', 'Lock Files', 'Branch Naming', 'Orphan Detection'],
    blocks: [
      { t: 'h2', text: 'Fleet Architecture' },
      { t: 'code', lang: 'text', code: `┌─────────────┐
│  Supervisor │  ← Polls task board every 2 min
│   Recipe    │  ← Breaks down epics, assigns workers
└──────┬──────┘
       │
       ├──→ Worker Backend Recipe    (polls every 2 min)
       ├──→ Worker Frontend Recipe   (polls every 2 min)
       ├──→ Worker Platform Recipe   (polls every 5 min)
       ├──→ Worker MCP Recipe        (polls every 5 min)
       ├──→ Worker Cleanup Recipe    (polls every 10 min)
       └──→ QC Agent Recipe         (polls every 2 min)` },
      { t: 'h2', text: 'Worker Types' },
      { t: 'table', headers: ['Worker', 'Schedule', 'Responsibility'], rows: [
        ['Supervisor', '*/2 * * * *', 'Poll task board, break down epics, assign workers, review QC reports, merge PRs'],
        ['Worker Backend', '*/2 * * * *', 'Backend development (Python, Rust, Go)'],
        ['Worker Frontend', '*/2 * * * *', 'Frontend development (React, TypeScript)'],
        ['Worker Platform', '*/5 * * * *', 'Platform/infra work (Docker, CI/CD, deployment)'],
        ['Worker MCP', '*/5 * * * *', 'MCP server development and testing'],
        ['Worker Cleanup', '*/10 * * * *', 'Code cleanup, refactoring, technical debt'],
        ['QC Agent', '*/2 * * * *', 'PR review, test execution, QC reports'],
      ]},
      { t: 'h2', text: 'Internal Task Board' },
      { t: 'prose', text: 'AutoDev uses an internal SQLite task board (~/.autodev/tasks.db). Why not GitHub Issues directly? Faster (no API calls), offline support, full control over data model. GitHub is still used for PRs, but task assignment happens internally.' },
      { t: 'code', lang: 'sql', code: `CREATE TABLE tasks (
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
);` },
      { t: 'h2', text: 'Task Lifecycle' },
      { t: 'code', lang: 'text', code: `Ready (created by PA or Supervisor)
    ↓
Assigned (Supervisor assigns to worker)
    ↓
InProgress (Worker claims task, creates lock file)
    ↓
QC Review (Worker completes, opens PR, QC reviews)
    ↓
Done (QC approved, Supervisor merges, closes task)
    │
    ├─→ Failed (Worker cannot complete)
    ├─→ Build Failed (Dependencies/build validation fails)
    └─→ Rejected (QC rejects, worker must rework)` },
      { t: 'h2', text: 'Worker Request Queue' },
      { t: 'prose', text: 'Workers communicate with Supervisor via the worker_requests table using the request_supervisor_action tool.' },
      { t: 'table', headers: ['Request Type', 'Worker Sends', 'Supervisor Does'], rows: [
        ['decompose_audit', 'audit_summary + suggested_tasks[]', 'create_task each → assign_task each'],
        ['reassign', 'target_worker + reason', 'assign_task(id, target_worker)'],
        ['needs_info', 'question + what_you_tried', 'Mark acknowledged — human PA must answer'],
        ['report_issue', 'issue_description + severity', 'Mark acknowledged, set task failed/build_failed'],
        ['request_review', 'completion_summary + files_changed', 'Set task qc_review'],
      ]},
      { t: 'code', lang: 'text', code: `Worker (request_supervisor_action tool) → worker_requests table
                                          ↓
Supervisor recipe (every 2 min) → processes requests
                                          ↓
Creates tasks / reassigns / marks acknowledged` },
      { t: 'h2', text: 'Lock Files' },
      { t: 'prose', text: 'Location: current_tasks/issue-N.lock. Format: ISO 8601 timestamp (UTC). Workers write lock file before starting work. Workers delete lock file only on clean completion. Never delete lock file mid-task on failure. Lock files are sacred — prevent concurrent work on same task.' },
      { t: 'h2', text: 'Branch Naming Convention' },
      { t: 'code', lang: 'text', code: `worker/<role>/<issue-number>

Examples:
  worker/backend/142
  worker/frontend/87
  worker/cleanup/201` },
      { t: 'list', items: [
        'Isolation — each task gets a clean working directory',
        'Parallel work — multiple workers can work simultaneously',
        'No main branch pollution — changes stay in worktrees',
        'Easy cleanup — worktree removed after task completion',
      ]},
      { t: 'h2', text: 'Orphan Detection' },
      { t: 'prose', text: 'The orphan module detects tasks that are stuck in in_progress but have no active lock file (worker crashed). These are automatically released back to assigned state so they can be re-claimed.' },
    ],
  },
]
