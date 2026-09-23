// Documentation — Sections 01–04
// Full content faithful to ARCANA_DOCUMENTATION.md

import type { DocSection } from './docs'

export const A_SECTIONS: DocSection[] = [

  /* ── 01: What is Arcana? ──────────────────────────────────────── */
  {
    id: 'what', num: '01', title: 'What is Arcana?', category: 'Overview', icon: 'info-circle',
    lead: 'A personal desktop workspace for multi-agent software development, with a native Rust agent engine and a React interface.',
    anchors: ['Vision', 'Why Build This?', 'Key Features', 'Tech Stack'],
    blocks: [
      { t: 'h2', text: 'Vision' },
      { t: 'callout', text: 'One app. Any model. Full autonomy.' },
      { t: 'prose', text: 'Arcana is being developed as a personal coding platform. Configure a provider key in Settings, select a model, and use chat, tasks, recipes, and coding tools as their development phases are verified.' },
      { t: 'h2', text: 'Why Build This?' },
      { t: 'h3', text: 'Claude Code Limitations' },
      { t: 'list', items: ['Locked to Anthropic subscriptions — rate limits cripple 24/7 worker agents', '512,000-line source leak (March 31, 2026) exposed vulnerabilities', 'No model flexibility — cannot route workers to cheaper models', 'Proprietary — cannot audit, fork, or adapt'] },
      { t: 'h3', text: 'Goose Limitations' },
      { t: 'list', items: ['A general-purpose agent does not provide Arcana\'s local task board', 'The Supervisor → Worker → QC flow remains an Arcana development goal'] },
      { t: 'h3', text: 'Arcana Advantage' },
      { t: 'list', items: ['Provider adapters for Google Gemini, Anthropic, and OpenAI', 'ARCANA.md instructions backed by SQLite', 'A local SQLite task board for the agent fleet', 'A native Rust tool loop shared across roles'] },
      { t: 'h2', text: 'Key Features' },
      { t: 'list', items: [
        'Model-agnostic: Google Gemini, Anthropic, or OpenAI',
        'Memory across sessions: episodic, semantic and procedural stores with relevance ranking and compaction',
        'A worker fleet: supervisor, five workers and QC, coordinated through a SQLite task board',
        'ARCANA.md: your own instructions, global and per-repository, edited in the app and given to every agent',
        'MCP client: connect to GitHub, Slack, Adobe and your own MCP servers',
        'Skills & Recipes: reusable YAML workflows with cron scheduling',
        'One installer, no services to run: no Python, no Docker, no database to stand up',
        'One engine: the assistant, every fleet worker and every subagent run on the same in-process Rust engine',
        'Git integration: structured git operations via git2-rs',
        'Cost tracking: real-time token spend per session, with per-agent and per-task reporting',
      ]},
      { t: 'h2', text: 'Tech Stack' },
      { t: 'table', headers: ['Layer', 'Technology', 'Purpose'], rows: [
        ['Desktop Shell', 'Tauri v2 (Rust)', 'Cross-platform native wrapper (~15MB base)'],
        ['Frontend', 'React 19 + TypeScript + Vite 8', 'Agent UI, chat, code editor, task board'],
        ['State Management', 'Zustand', 'Global state (sessions, config, agent status)'],
        ['Styling', 'Tailwind CSS 3', 'Arcana interface styles'],
        ['Backend', 'Rust (Tauri commands)', 'Code editing tools, file I/O, command execution'],
        ['Provider Layer', 'Anthropic, OpenAI, Google', 'Multi-provider LLM abstraction'],
        ['MCP Health', 'reqwest', 'Liveness probes for user-configured external MCP servers'],
        ['Git Integration', 'git2-rs (libgit2)', 'Structured git operations, worktrees'],
        ['Agent engine', 'Rust (arcana-core)', 'The tool loop every agent runs on — assistant, worker and subagent'],
        ['Memory', 'Rust + SQLite', 'Episodic, semantic and procedural stores with relevance ranking'],
        ['Keychain', 'keyring crate', 'OS keychain integration for API keys'],
        ['Database', 'SQLite (rusqlite)', 'Session persistence, task board, worker registry'],
      ]},
    ],
  },

  /* ── 02: Core Architecture ─────────────────────────────────────── */
  {
    id: 'arch', num: '02', title: 'Core Architecture', category: 'Overview', icon: 'topology-star',
    lead: 'Arcana is a Tauri v2 desktop application — a Rust backend and a React front end, in one process. Every agent it runs, from the interactive assistant to a fleet worker, runs on the same engine inside that process. There are no services to start and nothing to keep alive alongside it.',
    anchors: ['High-Level Diagram', 'The engine', 'The fleet', 'What runs where'],
    blocks: [
      { t: 'h2', text: 'High-Level Architecture Diagram' },
      { t: 'code', lang: 'text', code: `┌──────────────────────────────────────────────────────────────┐
│                        Arcana — one process                     │
├─────────────────────┬──────────────────────────────────────────┤
│  React + TypeScript  │            Rust (Tauri v2)               │
│  src/                │            src-tauri/ + arcana-core/    │
│                      │                                          │
│  Chat                │   Tauri commands                         │
│  Arcana panel       │    agent · provider · vault · settings     │
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
│                      │   ~/.arcana/  — SQLite on your disk      │
│                      │    tasks · workers · sessions · memory     │
│                      │    analytics · recipes · instructions     │
└─────────────────────┴──────────────────────────────────────────┘

The only things outside this process are the LLM provider you chose and any
MCP servers you connected yourself.` },
      { t: 'h2', text: 'The engine' },
      { t: 'prose', text: 'One loop, in this process, for every agent. You talking to the assistant and a fleet worker running its recipe at 3am take the same path.' },
      { t: 'code', lang: 'text', code: `User → Tauri (Rust) → Direct LLM API (Anthropic/OpenAI/etc.)
                    ↓
              Built-in tools (read_file, write_file, bash, git2-rs)
                    ↓
              External MCP servers — configured and health-checked, not yet called
                    ↓
              Response → UI` },
      { t: 'h3', text: 'What that buys you' },
      { t: 'list', items: [
        'No round-trip — a turn is a function call, not an HTTP request to something else on your machine',
        'Nothing to start — the app works the moment it opens; there is no service that can fail to come up',
        'One place to look — an agent that misbehaves is debugged in one process, with one log',
        'Headless too — arcanactl -p "..." runs the same engine from a terminal',
        'Local-first — sessions, memory and the task board are all on-device',
      ]},
      { t: 'h2', text: 'The fleet' },
      { t: 'prose', text: 'A supervisor, five workers (backend, frontend, platform, MCP, cleanup) and QC. Each runs its own recipe on a cron schedule and coordinates through the task board — not through files on disk, and not through GitHub.' },
      { t: 'code', lang: 'text', code: `Cron tick → RecipeRunner → AgentEngine (same process)
                                        ↓
                             task board: claim → in_progress
                                        ↓
                             tools: read/write/edit/bash/grep
                                        ↓
                             task board: qc_review → QC → done
                                        ↓
                             events → Live Monitor` },
      { t: 'h3', text: 'How a worker gets its instructions' },
      { t: 'list', items: [
        'An invariant core every agent receives — the prohibitions, the git rules, and how work is coordinated',
        'Your ARCANA.md, global and per-repository',
        'Its own recipe: role, protocol and what counts as done',
        'Its memory, rebuilt each turn from what it has learned',
      ]},
      { t: 'callout', variant: 'info', text: 'A worker can only do what its recipe and the shared core allow. It commits to a branch, stages the paths it named, and stops there — it never pushes, opens a pull request or merges one.' },
    ],
  },

  /* ── 03: Personal Assistant (PA) Mode ─────────────────────────── */
  {
    id: 'pa-mode', num: '03', title: 'Personal Assistant (PA) Mode', category: 'Agent Modes', icon: 'robot',
    lead: 'The assistant is the interactive half of Arcana — the one you talk to. It runs on the same engine as everything else, with direct access to your chosen provider. Three modes change what it can reach and how it works.',
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
      { t: 'prose', text: 'System Prompt: DELEGATE_SYSTEM_PROMPT (task creation focused). NO file access tools (no read_file, write_file, edit_file, bash). Decomposes user requests into atomic tasks and creates them on the internal Arcana board.' },
      { t: 'list', items: [
        'create_task (writes to internal Arcana task board)',
        'MCP tools (read-only only — GitHub MCP for context, no write tools)',
        'NO direct code execution whatsoever',
      ]},
      { t: 'h2', text: 'Delegate Mode Interaction Flow' },
      { t: 'code', lang: 'text', code: `1. You: "add a CSV export to the sessions list"
2. Assistant (Delegate): understands the intent, decomposes it, files task #142
   with acceptance criteria a reviewer could check
3. Assistant: "Task #142 is on the board. The Supervisor picks it up on its
   next tick."
4. Supervisor (every 2 min): sees #142 ready, assigns it to Worker Frontend
5. Worker Frontend: claims it, implements it, runs the project's tests,
   commits to the fleet branch, moves it to qc_review
6. QC: reviews the change and either marks it done or rejects it with a reason
7. Arcana panel: shows each transition as it happens` },
      { t: 'h2', text: 'Switching PA Modes' },
      { t: 'prose', text: 'Location: Chat header dropdown (3-button segmented control). State Management: settingsStore.ts persists paMode to localStorage. Effect: Changes tool registry and system prompt for the next message sent. Persistence: Mode preference saved across sessions.' },
    ],
  },

  /* ── 04: Fleet & Delegate Mode ─────────────────────────────────── */
  {
    id: 'fleet', num: '04', title: 'Fleet & Delegate Mode', category: 'Agent Modes', icon: 'users',
    lead: 'The Arcana fleet is a collection of autonomous worker agents that execute tasks assigned by a Supervisor agent. This multi-agent pipeline enables 24/7 autonomous development.',
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
        ['Supervisor', '*/2 * * * *', 'Poll the task board, break down epics, assign workers, review what comes back'],
        ['Worker Backend', '*/2 * * * *', 'Backend work — Rust, services, schema, APIs'],
        ['Worker Frontend', '*/2 * * * *', 'Frontend development (React, TypeScript)'],
        ['Worker Platform', '*/5 * * * *', 'Platform/infra work (Docker, CI/CD, deployment)'],
        ['Worker MCP', '*/5 * * * *', 'MCP server development and testing'],
        ['Worker Cleanup', '*/10 * * * *', 'Code cleanup, refactoring, technical debt'],
        ['QC Agent', '*/2 * * * *', 'PR review, test execution, QC reports'],
      ]},
      { t: 'h2', text: 'Internal Task Board' },
      { t: 'prose', text: 'Arcana uses an internal SQLite task board (~/.arcana/tasks.db). Why not GitHub Issues directly? Faster (no API calls), offline support, full control over data model. GitHub is still used for PRs, but task assignment happens internally.' },
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
