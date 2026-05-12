// Documentation data — single source of truth for the Docs panel.
// Content is structured as typed blocks so the renderer can apply
// AutoDev-native styling without parsing markdown.

export type Block =
  | { t: 'h2';      text: string }
  | { t: 'h3';      text: string }
  | { t: 'prose';   text: string }
  | { t: 'callout'; text: string; variant?: 'warn' | 'info' }
  | { t: 'list';    items: string[] }
  | { t: 'table';   headers: string[]; rows: string[][] }
  | { t: 'code';    lang: string; code: string }

export interface DocSection {
  id:       string
  num:      string
  title:    string
  category: string
  icon:     string
  lead:     string
  anchors:  string[]
  blocks:   Block[]
}

export const NAV_CATEGORIES: { label: string; ids: string[] }[] = [
  { label: 'Overview',    ids: ['what', 'arch'] },
  { label: 'Agent Modes', ids: ['pa-mode', 'fleet'] },
  { label: 'Automation',  ids: ['recipes', 'skills', 'hooks'] },
  { label: 'Interface',   ids: ['settings', 'dashboard', 'ide', 'graph'] },
  { label: 'Internals',   ids: ['internals', 'submodules', 'glossary'] },
]

export const SECTIONS: DocSection[] = [
  {
    id: 'what', num: '01', title: 'What is AutoDev?', category: 'Overview',
    icon: 'info-circle',
    lead: 'A downloadable desktop application combining Claude Code and Goose into a single, model-agnostic AI developer agent platform. Purpose-built for AIRS Foundry\'s worker pipeline.',
    anchors: ['Vision', 'Why Build This?', 'Key Features', 'Tech Stack'],
    blocks: [
      { t: 'h2', text: 'Vision' },
      { t: 'callout', text: 'One app. Any model. Full autonomy.' },
      { t: 'prose', text: 'Any AIRS team member or AutoDev worker agent can download one binary, enter their LLM API key (any provider), select their model, and start executing agentic development tasks with the full capability stack: AUTODEV.md, Skills, Recipes, Subagents, MCP servers, Hooks, and the AutoDev worker pipeline.' },
      { t: 'h2', text: 'Why Build This?' },
      { t: 'h3', text: 'Claude Code Limitations' },
      { t: 'list', items: ['Locked to Anthropic subscriptions — rate limits cripple 24/7 agents', '512,000-line source leak (March 31 2026) exposed vulnerabilities', 'No model flexibility — cannot route workers to cheaper models', 'Proprietary — cannot audit, fork, or adapt'] },
      { t: 'callout', text: 'March 2026 source map leak confirmed — building a custom harness reduces dependency on opaque binaries.', variant: 'warn' },
      { t: 'h3', text: 'AutoDev Advantage' },
      { t: 'list', items: ['Model-agnostic by design (Anthropic / OpenAI / Gemini / Ollama)', 'AUTODEV.md-native with SQLite-backed instruction system', 'AutoDev-native GitHub Issues board understanding', 'Secure and auditable — AIRS controls the harness', 'Downloadable single installer — no Python/Docker for end users'] },
      { t: 'h2', text: 'Key Features' },
      { t: 'table', headers: ['Feature', 'Detail', 'Status'], rows: [
        ['Model-agnostic', 'Anthropic, OpenAI, Gemini, Ollama, Azure, Bedrock', 'live'],
        ['Memory system', '3-tier episodic/semantic/procedural with context scoring', 'live'],
        ['Agent orchestration', '21 FSMs + A2A protocol via core-orchestrator', 'live'],
        ['AUTODEV.md system', 'SQLite-backed, editable from UI', 'live'],
        ['MCP native', 'rmcp crate — GitHub, Slack, custom MCP servers', 'beta'],
        ['Skills & Recipes', 'Reusable YAML workflows with cron scheduling', 'live'],
        ['Cost tracking', 'Real-time token spend per session per agent', 'live'],
        ['Git integration', 'git2-rs with worktree isolation per task', 'live'],
      ]},
      { t: 'h2', text: 'Tech Stack' },
      { t: 'code', lang: 'toml', code: `# Desktop layer
desktop_shell = "Tauri v2 (Rust)"
frontend      = "React 19 + TypeScript + Vite 8"
state         = "Zustand"
styling        = "Tailwind CSS 3"

# Intelligence layer
orchestrator  = "Python 3.13 + FastAPI + litellm"
memory        = "Python 3.13 + mem0"
cache         = "Redis 7+ (standalone binary)"
mcp_client    = "rmcp crate (Rust-native)"
git           = "git2-rs (libgit2)"` },
    ],
  },

  {
    id: 'arch', num: '02', title: 'Core Architecture', category: 'Overview',
    icon: 'topology-star',
    lead: 'AutoDev is built on Tauri v2 (Rust + React) and bundles two Python sidecars as git submodules. It operates in dual mode: PA (Rust-native) and Worker (Python core-orchestrator).',
    anchors: ['Dual-Mode', 'PA Mode Path', 'Worker Mode Path', 'Module Map'],
    blocks: [
      { t: 'h2', text: 'Dual-Mode Architecture' },
      { t: 'prose', text: 'AutoDev operates in TWO modes with fundamentally different execution paths. The mode is chosen per session based on who is executing and what context they need.' },
      { t: 'table', headers: ['Mode', 'Who uses it', 'Execution path', 'Speed'], rows: [
        ['PA — Rust-native', 'Razi, team members', 'Tauri → direct LLM API → built-in tools', 'Fast — no sidecar overhead'],
        ['Worker — Python', 'Fleet workers, Supervisor, QC', 'Sidecar → LivingAgent FSM → core-orchestrator', 'Managed — full FSM lifecycle'],
      ]},
      { t: 'h2', text: 'PA Mode Path' },
      { t: 'code', lang: 'text', code: `User → Tauri (Rust) → Direct LLM API (Anthropic/OpenAI/etc.)
                ↓
          Built-in tools (read_file, write_file, bash, git2-rs)
                ↓
          MCP clients (GitHub MCP, Slack MCP) via rmcp crate
                ↓
          Response → UI` },
      { t: 'h2', text: 'Worker Mode Path' },
      { t: 'code', lang: 'text', code: `RecipeScheduler (Rust cron)
        ↓
  core-orchestrator (Python FastAPI, port 4000)
        ↓
  LivingAgent (21 FSMs + A2A protocol)
        ↓
  LLM API (via litellm) + MCP tools (AIRS MCP Server)
        ↓
  Task completion → tasks.db → Tauri events → UI` },
      { t: 'h2', text: 'Module Map' },
      { t: 'table', headers: ['Layer', 'Module', 'Purpose'], rows: [
        ['Rust', 'agent/', 'Tool execution loop, engine, context building'],
        ['Rust', 'mcp/', 'MCP client via rmcp, tool discovery'],
        ['Rust', 'recipes/', 'YAML parser, cron scheduler, recipe runner'],
        ['Rust', 'git/', 'git2-rs bindings, checkpoint system'],
        ['Rust', 'graph/', 'Code knowledge graph, tree-sitter extraction'],
        ['Rust', 'db/', 'SQLite: tasks.db, config.db, graph.db'],
        ['Python', 'core-orchestrator/', 'LivingAgent FSMs, A2A protocol, litellm'],
        ['Python', 'memory-mcp-server/', 'mem0 memory system MCP server'],
        ['React', 'tabs/', 'Chat, AutoDev, IDE, Recipes, Graph, Docs, Settings'],
        ['React', 'stores/', 'Zustand state (settings, tasks, workers, costs)'],
      ]},
    ],
  },

  {
    id: 'pa-mode', num: '03', title: 'Personal Assistant (PA) Mode', category: 'Agent Modes',
    icon: 'robot',
    lead: 'PA Mode is the interactive agent that Razi (and team) use directly. It has three sub-modes that control which tools are available and how cautiously it operates.',
    anchors: ['Mode Toggle', 'Fast Development', 'Review Mode', 'Delegate Mode', 'Interaction Flow'],
    blocks: [
      { t: 'h2', text: 'Mode Toggle' },
      { t: 'prose', text: 'The PA mode toggle appears in the Chat header. Switching modes changes the tool registry and system prompt injected for the next conversation.' },
      { t: 'table', headers: ['Mode', 'Tools', 'System Prompt', 'Use Case'], rows: [
        ['⚡ Fast Development', 'All 11 built-in tools', 'BASE_SYSTEM_PROMPT', 'Direct coding, full autonomy'],
        ['✋ Review Mode', 'All tools + permission checks', 'REVIEW_SYSTEM_PROMPT', 'Cautious editing, human approval'],
        ['📋 Delegate Mode', 'create_task only', 'DELEGATE_SYSTEM_PROMPT', 'Workers are live — PA just dispatches'],
      ]},
      { t: 'h2', text: 'Fast Development' },
      { t: 'prose', text: 'The default mode. PA has access to all built-in tools and acts autonomously. No confirmation dialogs. Best for rapid iteration and development.' },
      { t: 'list', items: ['read_file, write_file, edit_file, append_file', 'bash (shell commands, test runners, linters)', 'grep, glob, list_directory', 'git operations (via git2-rs)', 'MCP tools (GitHub, Slack, custom)'] },
      { t: 'h2', text: 'Review Mode' },
      { t: 'prose', text: 'Same tools as Fast Development but the system prompt instructs PA to pause and seek confirmation before destructive operations. Ideal for production changes.' },
      { t: 'callout', text: 'Review Mode does NOT disable any tools — it changes agent behavior through the system prompt. The agent will ask before writing files or running bash commands.' },
      { t: 'h2', text: 'Delegate Mode' },
      { t: 'prose', text: 'When the worker fleet is live, switch PA to Delegate Mode. PA can only call create_task — it has zero file editing tools. All actual work is dispatched to the fleet.' },
      { t: 'code', lang: 'text', code: `Razi speaks → PA (Delegate Mode) → create_task → tasks.db
                                              ↓
                            Supervisor recipe (polls every 2 min)
                                              ↓
                            Supervisor assigns to Worker recipes
                                              ↓
                            Workers execute via core-orchestrator
                                              ↓
                            QC reviews → Supervisor merges` },
      { t: 'h2', text: 'Interaction Flow' },
      { t: 'prose', text: 'The system prompt is built from three constants in context.rs: BASE_SYSTEM_PROMPT, REVIEW_SYSTEM_PROMPT, DELEGATE_SYSTEM_PROMPT. The AUTODEV.md content is injected from SQLite (config.db) into every prompt regardless of mode.' },
    ],
  },

  {
    id: 'fleet', num: '04', title: 'Fleet & Delegate Mode', category: 'Agent Modes',
    icon: 'users',
    lead: 'The fleet is a set of autonomous worker agents orchestrated by the Supervisor recipe. Workers run 24/7 via the core-orchestrator Python sidecar.',
    anchors: ['Fleet Architecture', 'Worker Types', 'Task Lifecycle', 'Lock Files', 'Branches'],
    blocks: [
      { t: 'h2', text: 'Fleet Architecture' },
      { t: 'prose', text: 'The fleet is NOT a separate service. It runs inside AutoDev as recipe-scheduled workers. The Supervisor recipe polls tasks.db, assigns work to workers, and manages the full lifecycle.' },
      { t: 'table', headers: ['Role', 'Recipe', 'Responsibility'], rows: [
        ['Supervisor', 'supervisor.yaml', 'Polls tasks, assigns workers, handles worker requests'],
        ['Frontend Worker', 'worker-frontend.yaml', 'React/TS UI implementation'],
        ['Backend Worker', 'worker-backend.yaml', 'Rust/Python backend implementation'],
        ['QC Agent', 'qc-agent.yaml', 'Code review, diff analysis, approve/reject'],
        ['Docs Worker', 'worker-docs.yaml', 'Documentation generation and updates'],
      ]},
      { t: 'h2', text: 'Task Lifecycle' },
      { t: 'code', lang: 'text', code: `backlog → assigned → in_progress → qc_review → done
                                ↓              ↓
                          build_failed      failed` },
      { t: 'prose', text: 'Tasks live in tasks.db (SQLite at ~/.autodev/tasks.db). Each status transition is atomic. Workers acquire a lock file (tasks/{id}.lock) to prevent double-processing.' },
      { t: 'h2', text: 'Worker Requests' },
      { t: 'prose', text: 'Workers can request Supervisor action via the request_supervisor_action tool. This writes to the worker_requests table and kicks the Supervisor to process it on its next tick.' },
      { t: 'table', headers: ['Request Type', 'Worker sends', 'Supervisor does'], rows: [
        ['decompose_audit', 'audit_summary + suggested_tasks[]', 'create_task each → assign_task each'],
        ['reassign', 'target_worker + reason', 'assign_task(id, target_worker)'],
        ['needs_info', 'question + what_you_tried', 'Acknowledged — human PA must answer'],
        ['report_issue', 'issue_description + severity', 'Set task failed/build_failed'],
        ['request_review', 'completion_summary + files_changed', 'Set task qc_review'],
      ]},
      { t: 'h2', text: 'Worktree Isolation' },
      { t: 'prose', text: 'Each worker operates in a git worktree branch named task/{id}-{slug}. This ensures parallel workers never conflict on the same files. The Supervisor merges approved worktrees.' },
    ],
  },

  {
    id: 'recipes', num: '05', title: 'Recipes System', category: 'Automation',
    icon: 'clock-play',
    lead: 'Recipes are YAML-defined agentic workflows that run on a cron schedule or on demand. They power the entire AutoDev fleet — Supervisor, Workers, QC — as well as custom user automations.',
    anchors: ['YAML Structure', 'Cron Scheduling', 'Recipe Runner', 'Default Recipes'],
    blocks: [
      { t: 'h2', text: 'YAML Structure' },
      { t: 'code', lang: 'yaml', code: `title: "My Recipe"
description: "What this recipe does"
system_prompt: |
  You are a specialized agent. Your role is to...
agent_role: "my-worker"           # slug used for lock files
schedule: "*/5 * * * *"           # POSIX cron (optional)
model: claude-sonnet-4-5          # overrides global setting
permission_mode: "autonomous"     # or "supervised"
instructions: |
  Step 1: Do this first.
  Step 2: Then do this.
  Stop when: Condition is met.` },
      { t: 'h2', text: 'Cron Scheduling' },
      { t: 'prose', text: 'The RecipeScheduler (src-tauri/src/recipes/scheduler.rs) starts a tokio background task per worker. Each tick fires the recipe runner. Workers with no schedule only run on-demand.' },
      { t: 'table', headers: ['Schedule', 'Meaning'], rows: [
        ['*/2 * * * *', 'Every 2 minutes (Supervisor)'],
        ['*/15 * * * *', 'Every 15 minutes'],
        ['0 * * * *', 'Every hour on the hour'],
        ['0 9 * * 1-5', 'Weekdays at 9:00 AM'],
        ['(none)', 'On-demand only'],
      ]},
      { t: 'h2', text: 'Recipe Runner' },
      { t: 'prose', text: 'The RecipeRunner (src-tauri/src/recipes/runner.rs) dispatches each tick to Mode 2 (core-orchestrator LivingAgent) when healthy, or falls back to Mode 1 (Rust AgentEngine) if the sidecar is down.' },
      { t: 'h2', text: 'Default Recipes' },
      { t: 'list', items: [
        'supervisor — Polls tasks.db every 2 min, assigns work, handles worker requests',
        'worker-frontend — Implements React/TS tasks from the board',
        'worker-backend — Implements Rust/Python tasks from the board',
        'qc-agent — Reviews PRs, validates acceptance criteria, approves/rejects',
        'worker-docs — Generates and updates documentation',
      ]},
      { t: 'callout', text: 'Default recipes are seeded from src-tauri/src/recipes/defaults.rs on first launch. User-created recipes live in ~/.autodev/recipes/ as YAML files.' },
    ],
  },

  {
    id: 'skills', num: '06', title: 'Skills System', category: 'Automation',
    icon: 'wand',
    lead: 'Skills are reusable markdown workflow templates invocable via slash commands from the Chat UI or programmatically by fleet workers during execution.',
    anchors: ['What are Skills?', 'Categories', 'File Structure', 'Parameters', 'Invocation'],
    blocks: [
      { t: 'h2', text: 'What are Skills?' },
      { t: 'prose', text: 'Skills extend the PA and workers with reusable, parameterizable instructions. A skill is a markdown file stored in ~/.autodev/skills/{category}/{skill-name}.md. When invoked, its content is injected into the agent context as a high-priority instruction.' },
      { t: 'h2', text: 'Storage Location' },
      { t: 'code', lang: 'text', code: `~/.autodev/skills/
  ├── code/
  │   ├── refactor.md
  │   └── add-tests.md
  ├── review/
  │   └── security-audit.md
  ├── docs/
  │   └── generate-readme.md
  └── git/
      └── commit-message.md` },
      { t: 'h2', text: 'File Structure' },
      { t: 'code', lang: 'markdown', code: `---
name: refactor
description: Refactors code to improve readability and structure
parameters:
  - name: target
    description: File or module to refactor
    required: true
  - name: style
    description: Style guide to apply
    default: "AIRS standard"
---

# Refactor Instructions

You are performing a focused refactor of {{target}}.

Apply the {{style}} style guide:
1. Extract functions longer than 20 lines
2. Rename variables to be descriptive
...` },
      { t: 'h2', text: 'Invocation' },
      { t: 'prose', text: 'From the Chat UI, type a slash command matching the skill name. Parameters are provided inline or via a prompt dialog.' },
      { t: 'list', items: [
        '/refactor target=src/agent/engine.rs',
        '/security-audit',
        '/generate-readme style="technical"',
        'Skills can also be invoked by workers via the invoke_skill tool',
      ]},
      { t: 'h2', text: 'Skills UI (SkillsPalette)' },
      { t: 'prose', text: 'The Skills Palette (src/components/skills/SkillsPalette.tsx) provides a searchable list of all installed skills. Click any skill to invoke it with a parameter form, or manage skills (create/edit/delete) from Settings.' },
    ],
  },

  {
    id: 'hooks', num: '07', title: 'Hooks System', category: 'Automation',
    icon: 'webhook',
    lead: 'Hooks are event-driven callbacks that fire at key lifecycle points. They allow you to extend AutoDev behavior — run bash scripts, send webhooks, log events — without modifying any source code.',
    anchors: ['Event Types', 'Actions', 'Conditions', 'Templates', 'Best Practices'],
    blocks: [
      { t: 'h2', text: 'Event Types' },
      { t: 'table', headers: ['Category', 'Events'], rows: [
        ['Agent Lifecycle', 'agent:start, agent:stop, agent:error, agent:tool_call, agent:tool_result, agent:message'],
        ['File System', 'fs:write, fs:read, fs:delete, fs:create, fs:rename'],
        ['Git', 'git:commit, git:push, git:branch_create, git:merge, git:checkpoint_create'],
        ['Plan', 'plan:create, plan:update, plan:step_complete, plan:complete'],
        ['Fleet', 'worker:start, worker:stop, worker:task_assign, worker:request, worker:complete'],
        ['Session', 'session:start, session:end, session:message'],
      ]},
      { t: 'h2', text: 'Actions' },
      { t: 'table', headers: ['Action', 'Purpose'], rows: [
        ['bash', 'Execute a shell command with event data as env vars'],
        ['webhook', 'POST JSON payload to an HTTP endpoint'],
        ['log', 'Append event data to a log file'],
        ['notify', 'Show a desktop notification'],
        ['skip', 'Cancel the triggering operation (before hooks only)'],
      ]},
      { t: 'h2', text: 'Conditions' },
      { t: 'prose', text: 'Hooks can be conditionally fired using the conditions field. Conditions are evaluated against the event payload using simple comparisons.' },
      { t: 'code', lang: 'json', code: `{
  "event": "fs:write",
  "action": "bash",
  "command": "npx tsc --noEmit",
  "conditions": [
    { "field": "file_path", "op": "ends_with", "value": ".ts" }
  ]
}` },
      { t: 'h2', text: 'Best Practices' },
      { t: 'list', items: [
        'Use bash hooks for CI-style checks after file writes',
        'Use webhook hooks to notify Slack on task completion',
        'Use log hooks for audit trails of agent actions',
        'Keep hook scripts idempotent — they may run multiple times',
        'Test hooks manually before enabling them on agent:tool_call (high frequency)',
      ]},
      { t: 'h2', text: 'Storage' },
      { t: 'prose', text: 'Hooks are stored in ~/.autodev/config.db (SQLite) in the hooks table. They are executed by the HookExecutor (src-tauri/src/hooks/executor.rs) which resolves conditions and dispatches the action asynchronously.' },
    ],
  },
]
