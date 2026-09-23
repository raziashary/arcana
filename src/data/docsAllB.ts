// Documentation — Sections 05–07
// Full content faithful to ARCANA_DOCUMENTATION.md

import type { DocSection } from './docs'

export const B_SECTIONS: DocSection[] = [

  /* ── 05: Recipes System ─────────────────────────────────────────── */
  {
    id: 'recipes', num: '05', title: 'Recipes System', category: 'Automation', icon: 'clock-play',
    lead: 'Recipes are YAML-defined workflows that schedule autonomous agent execution. They are the backbone of the worker fleet, enabling 24/7 automated development.',
    anchors: ['YAML Structure', 'Cron Scheduling', 'Recipe Execution', 'Scheduler', 'Runner', 'Default Recipes', 'Parameters', 'Recipe UI'],
    blocks: [
      { t: 'h2', text: 'Recipe YAML Structure' },
      { t: 'code', lang: 'yaml', code: `id: supervisor-poll
title: Supervisor Poll
description: Polls task board and assigns work to workers
agent_role: supervisor
schedule: "*/2 * * * *"   # POSIX cron expression
permission_mode: autonomous
model: claude-sonnet-4-5
instructions: |
  You are the Supervisor Agent. Your responsibilities:
  1. Poll the task board via list_tasks
  2. Check for worker_requests via list_worker_requests
  3. Assign tasks to workers via assign_task
  4. Monitor worker progress
  5. Review QC reports and merge approved PRs` },
      { t: 'h2', text: 'Cron Scheduling' },
      { t: 'prose', text: 'Recipes use POSIX cron expressions (5 fields: minute hour day month weekday):' },
      { t: 'table', headers: ['Expression', 'Meaning'], rows: [
        ['*/2 * * * *', 'Every 2 minutes (Supervisor, Worker Backend, Worker Frontend, QC Agent)'],
        ['*/5 * * * *', 'Every 5 minutes (Worker Platform, Worker MCP)'],
        ['*/10 * * * *', 'Every 10 minutes (Worker Cleanup)'],
        ['0 * * * *', 'Every hour on the hour'],
        ['0 0 * * *', 'Every day at midnight'],
        ['(none)', 'On-demand only — run manually from UI'],
      ]},
      { t: 'h2', text: 'How a recipe runs' },
      { t: 'prose', text: 'On its cron beat the scheduler checks the worker is in a runnable state, then hands the recipe to the engine in this process. The recipe\'s instructions become that worker\'s own instructions, appended to the invariant core every agent receives and to your ARCANA.md.' },
      { t: 'h2', text: 'Recipe Scheduler' },
      { t: 'prose', text: 'Implementation: arcana-core/src/recipes/scheduler.rs' },
      { t: 'list', items: [
        'Uses tokio tasks for background execution',
        'cron crate for POSIX cron expression parsing',
        'Arc<RecipeScheduler> stored in Tauri managed state (lives for app lifetime)',
        'HashMap<String, JoinHandle<()>> maps worker_slug → running task',
        'Idempotent start_worker() — restarts if already running',
        'Immediate startup tick (no wait for first cron fire)',
        'A worker that is paused or deleted stops its schedule; one that is still working skips the tick rather than overlapping',
        'Orphan detection runs before each Supervisor tick',
      ]},
      { t: 'h2', text: 'Recipe Runner' },
      { t: 'prose', text: 'Implementation: arcana-core/src/recipes/runner.rs' },
      { t: 'code', lang: 'text', code: `Cron beat → runnable? → resolve provider + model → AgentEngine → tick events → status` },
      { t: 'list', items: [
        'No-op short-circuit: a worker with no assigned or in_progress task skips the tick rather than spinning up the model to find an empty board',
        'Model resolution: worker.model > recipe.model > active_model > default',
        'Workspace directory: configured workspace > sandbox (~/.arcana/agents/<slug>/)',
        'Ephemeral sessions — each tick creates fresh session ID (not persisted)',
        'Status updates: idle → working → idle/error',
      ]},
      { t: 'h2', text: 'Default System Recipes' },
      { t: 'prose', text: 'Location: src-tauri/src/recipes/defaults.rs — seeded on first launch.' },
      { t: 'list', items: [
        'supervisor-poll — Supervisor agent, every 2 minutes',
        'worker-backend-poll — Backend worker, every 2 minutes',
        'worker-frontend-poll — Frontend worker (React/TypeScript), every 2 minutes',
        'worker-platform-poll — Platform/infra worker, every 5 minutes',
        'worker-mcp-poll — MCP server developer, every 5 minutes',
        'worker-cleanup-poll — Code cleanup/refactoring, every 10 minutes',
        'qc-agent-poll — QC agent (PR review), every 2 minutes',
      ]},
      { t: 'h2', text: 'Recipe Parameters' },
      { t: 'code', lang: 'yaml', code: `parameters:
  - name: issue_number
    type: integer
    required: true
    description: GitHub issue number to process
  - name: repo_name
    type: string
    required: false
    default: "arcana"` },
      { t: 'h2', text: 'Recipe UI' },
      { t: 'prose', text: 'Location: src/tabs/Recipes.tsx' },
      { t: 'list', items: [
        'List all recipes (system + user)',
        'Expandable cards showing YAML content',
        'Edit recipe via RecipeEditor modal',
        'Create new recipes',
        'System recipes marked as read-only',
        'Permission mode badge (autonomous/manual_approval)',
        'Agent role badge, schedule display, model display',
      ]},
    ],
  },

  /* ── 06: Skills System ──────────────────────────────────────────── */
  {
    id: 'skills', num: '06', title: 'Skills System', category: 'Automation', icon: 'wand',
    lead: 'Skills are reusable markdown workflow templates that can be invoked via slash commands or programmatically by fleet workers. They package common development patterns, debugging workflows, and operational procedures into shareable, parameterized templates.',
    anchors: ['What are Skills?', 'Categories', 'File Structure', 'Parameters', 'Invocation', 'Skills Palette', 'Execution Flow', 'Built-in Skills', 'Tauri Commands'],
    blocks: [
      { t: 'h2', text: 'What are Skills?' },
      { t: 'prose', text: 'Skills are .md files with YAML frontmatter stored in ~/.arcana/skills/{category}/{skill-name}.md. They combine YAML frontmatter (metadata: name, description, category, parameters, version) with a markdown template body containing {{parameter}} placeholders.' },
      { t: 'h2', text: 'Skill Categories' },
      { t: 'table', headers: ['Category', 'Description', 'Icon'], rows: [
        ['Coding', 'Development workflows (feature implementation, refactoring)', '⚡ Zap'],
        ['Debugging', 'Debugging patterns (error investigation, log analysis)', '🐛 Bug'],
        ['DevOps', 'Infrastructure and deployment workflows', '🖥️ Server'],
        ['Fleet', 'Fleet management and orchestration tasks', '👥 Users'],
        ['Custom', 'User-defined skills', '➕ Plus'],
      ]},
      { t: 'h2', text: 'Skill File Structure' },
      { t: 'code', lang: 'yaml', code: `---
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
author: Arcana
tags: [authentication, jwt, rest-api]
---

You are adding JWT-based authentication at \`{{endpoint_path}}\`.

1. Verify the project structure with list_directory and glob
2. Add the JWT dependency (check package.json or requirements.txt)
3. Implement the auth endpoint loading secret from \`{{jwt_secret_env}}\`
4. Add authentication middleware to protected routes
5. Write tests for login, invalid credentials, and token verification` },
      { t: 'h2', text: 'Parameter Types' },
      { t: 'table', headers: ['Type', 'Description', 'Example'], rows: [
        ['String', 'Text value', '"my-feature"'],
        ['Boolean', 'True/false flag', 'true'],
        ['Integer', 'Whole number', '42'],
        ['Enum', 'One of predefined values', '"production" | "staging" | "development"'],
      ]},
      { t: 'h2', text: 'Slash Command Invocation' },
      { t: 'prose', text: 'Keyboard Shortcut: Cmd/Ctrl+Shift+S opens the Skills Palette. In Chat: type / to trigger autocomplete, select a skill, fill in parameters via modal, send to execute.' },
      { t: 'code', lang: 'text', code: `User: /add-auth-endpoint
[SkillParameterModal opens]
User fills: endpoint_path="/api/auth/login", jwt_secret_env="JWT_SECRET"
[Skill renders into chat input]
User sends message → Agent executes the workflow` },
      { t: 'h2', text: 'Programmatic Invocation' },
      { t: 'prose', text: 'Fleet workers can invoke skills programmatically via Tauri commands. Use cases: Supervisor invokes skills for common patterns; workers use skills for standardized debugging workflows; QC agent uses skills for consistent review procedures.' },
      { t: 'code', lang: 'rust', code: `let result = execute_skill("add-auth-endpoint", params).await?;
// result.rendered contains the full markdown with parameters substituted` },
      { t: 'h2', text: 'Skills Palette UI' },
      { t: 'prose', text: 'Location: src/components/skills/SkillsPalette.tsx' },
      { t: 'list', items: [
        'Search across all skills',
        'Filter by category (Coding, Debugging, DevOps, Fleet, Custom)',
        'Recent skills tab (tracks last 10 used)',
        'Skill detail view with full template',
        'Parameter modal for execution',
        'Create new skill button',
        'Keyboard navigation: arrows, Enter, Escape, Tab to cycle categories',
      ]},
      { t: 'h2', text: 'Skill Execution Flow' },
      { t: 'code', lang: 'text', code: `1. User selects skill (via slash command or palette)
2. SkillParameterModal opens (if skill has parameters)
3. User fills in parameter values
4. SkillEngine renders template with parameters substituted
5. Rendered markdown inserted into chat input
6. User sends message
7. Agent executes the workflow as if user typed it directly` },
      { t: 'h2', text: 'Default Built-in Skills' },
      { t: 'prose', text: 'Arcana ships with 20+ pre-built skills. Custom skills stored in ~/.arcana/skills/custom/. User-created skills via the custom category.' },
      { t: 'table', headers: ['Category', 'Skill', 'Purpose'], rows: [
        ['Coding', 'add-auth-endpoint', 'Add JWT authentication endpoint'],
        ['Coding', 'add-rest-endpoint', 'Add REST API endpoint with tests'],
        ['Coding', 'refactor-component', 'Refactor React component'],
        ['Coding', 'add-unit-tests', 'Add unit tests for a function'],
        ['Debugging', 'investigate-error', 'Systematic error investigation'],
        ['Debugging', 'analyze-logs', 'Log file analysis workflow'],
        ['Debugging', 'debug-test-failure', 'Debug failing test step by step'],
        ['Debugging', 'profile-performance', 'Performance profiling workflow'],
        ['DevOps', 'dockerize-app', 'Containerize application'],
        ['DevOps', 'setup-ci-cd', 'Setup CI/CD pipeline'],
        ['DevOps', 'deploy-to-staging', 'Deploy to staging environment'],
        ['Fleet', 'create-worker-task', 'Create task for worker agent'],
        ['Fleet', 'monitor-fleet-health', 'Check fleet status and health'],
        ['Fleet', 'analyze-fleet-metrics', 'Analyze fleet performance metrics'],
      ]},
      { t: 'h2', text: 'Tauri Commands' },
      { t: 'list', items: [
        'list_skills() — List all skills with summaries',
        'get_skill(name) — Get full skill details',
        'search_skills(query) — Search by name/description/tags',
        'skills_by_category(category) — List skills in a category',
        'validate_skill_parameters(name, params) — Validate parameter values',
        'execute_skill(name, params) — Render and return skill',
        'reload_skills() — Reload skills from disk',
        'import_skill(skill_data) — Import a skill from JSON',
      ]},
      { t: 'callout', text: 'Skills are validated on load: YAML frontmatter must be valid, required parameters defined, parameter types valid, template must have valid {{parameter}} placeholders. Invalid skills are skipped and logged.' },
    ],
  },

  /* ── 07: Hooks System ───────────────────────────────────────────── */
  {
    id: 'hooks', num: '07', title: 'Hooks System', category: 'Automation', icon: 'webhook',
    lead: 'Hooks are event-driven extensibility points that execute custom actions at key lifecycle points. They enable automation, safety checks, notifications, logging, and custom integrations without modifying core code.',
    anchors: ['Event Types', 'Hook Actions', 'Conditions', 'Templates', 'Execution', 'Settings UI', 'Tauri Commands', 'Best Practices'],
    blocks: [
      { t: 'h2', text: 'Hook Event Types' },
      { t: 'table', headers: ['Category', 'Events'], rows: [
        ['Agent Lifecycle', 'SessionStart, SessionEnd, AgentLoopStart, AgentLoopEnd, OnError'],
        ['Tool Execution', 'PreToolUse, PostToolUse, OnToolError, OnPermissionRequest'],
        ['File System', 'PreFileRead, PostFileWrite, PreFileDelete, OnCheckpointCreate, OnCheckpointRestore'],
        ['Git', 'PreCommit, PostCommit, PrePush, OnBranchSwitch, OnMergeConflict'],
        ['Plan Management', 'PrePlanStep, PostPlanStep, OnPlanActivate, OnPlanComplete, OnPlanAbandon'],
        ['Fleet', 'OnTaskClaim, OnTaskComplete, OnTaskFail, OnQcReviewStart, OnQcApprove, OnQcFail'],
      ]},
      { t: 'h2', text: 'Hook Actions' },
      { t: 'table', headers: ['Type', 'Description', 'Key Fields'], rows: [
        ['shell', 'Execute a shell command', 'command, working_dir, timeout_secs'],
        ['script', 'Run a script file', 'path, args[]'],
        ['http', 'POST JSON to an HTTP endpoint', 'url, method, headers, body'],
        ['notification', 'Show desktop notification', 'title, message'],
        ['log', 'Append to a log file', 'file, message (supports {{variables}})'],
        ['block', 'Cancel the triggering operation (before hooks)', 'message'],
        ['modify', 'Transform tool input/output', 'transform (expression)'],
        ['emit_event', 'Emit a custom event', 'event_name, payload'],
      ]},
      { t: 'h2', text: 'Hook Conditions' },
      { t: 'prose', text: 'All conditions must match for a hook to execute. Available condition types:' },
      { t: 'list', items: [
        'tool_name — Match specific tool (e.g., "bash")',
        'file_pattern — Glob pattern for file path (e.g., "**/*.rs")',
        'command_pattern — Regex on command (e.g., "rm\\\\s+-rf")',
        'session_type — "fleet" or "pa"',
        'agent_mode — "fast_dev", "review", "delegate"',
        'time_range — start_hour, end_hour, days (e.g., weekdays 9–17)',
        'custom_condition — Rhai script expression (e.g., \'tool_name == "bash" && command.contains("rm -rf")\')',
      ]},
      { t: 'h3', text: 'Available Variables in Conditions' },
      { t: 'list', items: [
        'tool_name — Name of tool being called',
        'file_path — Path to file being operated on',
        'agent_mode — Current PA mode',
        'command — Command being executed',
        'hook_type — Current hook event type',
        'session_id — Current session ID',
        'worker_id — Worker ID (for fleet mode)',
        'task_id — Task ID (for fleet mode)',
        'is_error — Whether operation resulted in error',
      ]},
      { t: 'h2', text: 'Pre-built Hook Templates' },
      { t: 'table', headers: ['Category', 'Template', 'What It Does'], rows: [
        ['Safety', 'Block rm -rf /', 'Prevents accidental root deletion'],
        ['Safety', 'Block force push', 'Stops --force pushes to remotes'],
        ['Safety', 'Block .env file commits', 'Prevents committing secrets'],
        ['Automation', 'Auto-format Rust on save', 'Runs cargo fmt after .rs file writes'],
        ['Automation', 'Auto-format TypeScript on save', 'Runs prettier on .ts/.tsx files'],
        ['Automation', 'Clippy lint on commit', 'Runs cargo clippy before commits'],
        ['Automation', 'Webhook on plan complete', 'POSTs to webhook when plan finishes'],
        ['Notifications', 'Notify on task complete', 'Desktop notification when worker finishes'],
        ['Notifications', 'Notify on QC failure', 'Alert when QC rejects output'],
        ['Notifications', 'Notify on agent error', 'Alert when agent encounters error'],
        ['Logging', 'Log all tool calls', 'Appends tool calls to log file'],
        ['Logging', 'Log errors to file', 'Captures errors in dedicated log'],
        ['Logging', 'Log git activity', 'Records commits and pushes'],
      ]},
      { t: 'h2', text: 'Hook Execution Flow' },
      { t: 'code', lang: 'text', code: `Event occurs (e.g., PreToolUse)
    ↓
HookRegistry fires event
    ↓
For each hook registered for event (sorted by priority):
    1. Evaluate all conditions
    2. If conditions match:
       a. Execute action
       b. Log outcome (Executed/Blocked/Skipped/Failed)
       c. If action is Block → stop event propagation
    3. If conditions don't match → Skipped` },
      { t: 'prose', text: 'Priority: hooks execute in priority order (lower number = higher priority). Block action stops the event from proceeding — the agent receives an error message from the hook.' },
      { t: 'h2', text: 'Hook Execution Logs' },
      { t: 'prose', text: 'Stored in SQLite (~/.arcana/config.db). Fields: id, hook_id, hook_name, hook_type, outcome (Executed/Blocked/Skipped/Failed), output, error, executed_at, duration_ms. Retention: 30 days (configurable).' },
      { t: 'h2', text: 'Settings UI' },
      { t: 'prose', text: 'Location: src/components/settings/HooksSettings.tsx' },
      { t: 'list', items: [
        'List all hooks with enabled/disabled status',
        'Toggle hooks on/off',
        'Add new hooks from template gallery (browse by category)',
        'Create custom hooks with full configuration',
        'Test hooks with mock context',
        'View hook execution logs (filter by outcome, export as JSON)',
        'Delete hooks',
      ]},
      { t: 'h2', text: 'Tauri Commands' },
      { t: 'list', items: [
        'list_hooks() — List all hooks',
        'enable_hook(id) — Enable a hook',
        'disable_hook(id) — Disable a hook',
        'add_hook(input) — Add a new hook',
        'remove_hook(id) — Delete a hook',
        'test_hook(id, context) — Test hook with mock context',
        'get_hook_logs(filter) — Get hook execution logs',
      ]},
      { t: 'h2', text: 'Best Practices' },
      { t: 'list', items: [
        'Keep hooks fast (< 1 second for non-blocking hooks)',
        'Use specific conditions to avoid false positives',
        'Test hooks with mock context before enabling',
        'Monitor hook logs for failures',
        'Use Block action sparingly — only for truly dangerous operations',
        'Provide clear error messages when blocking',
        'Avoid expensive operations in hot paths (PreToolUse, PostFileWrite)',
        'Set appropriate timeouts for shell commands',
        'Never log sensitive data (passwords, API keys)',
        'Review hooks regularly for security risks',
      ]},
    ],
  },
]
