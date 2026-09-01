// Documentation data — sections 8–14 (continued from docs.ts).

import type { DocSection } from './docs'

export const MORE_SECTIONS: DocSection[] = [
  {
    id: 'settings', num: '08', title: 'Settings Panel', category: 'Interface',
    icon: 'settings',
    lead: 'The Settings panel is the central configuration hub for AutoDev — providers, models, workers, GitHub, MCP tools, hooks, and more.',
    anchors: ['Workspace', 'Services', 'Providers', 'GitHub', 'Workers', 'Hooks'],
    blocks: [
      { t: 'h2', text: 'Workspace Folder' },
      { t: 'prose', text: 'Set the active workspace root. AutoDev\'s file tools, git integration, and graph extraction all operate relative to this path. Changing the workspace restarts the agent context.' },
      { t: 'h2', text: 'Services' },
      { t: 'prose', text: 'Shows the live status of AutoDev\'s two Python sidecars: core-orchestrator (port 4000) and memory-mcp-server (port 3001). Green = running, red = offline. Provides start/stop/restart controls.' },
      { t: 'h2', text: 'Providers & Models' },
      { t: 'prose', text: 'Configure LLM providers. Each provider requires an API key stored in the OS keychain (Tauri vault). Select the default model for PA and for each worker. Available providers:' },
      { t: 'list', items: ['Google (gemini-3.5-pro, gemini-3.5-flash)', 'Anthropic (claude-opus-5, claude-sonnet-5, claude-fable-5, claude-haiku-4-5)', 'OpenAI (gpt-5.5, gpt-5.4, gpt-5.3-codex)'] },
      { t: 'h2', text: 'GitHub Configuration' },
      { t: 'prose', text: 'Provide a Personal Access Token (PAT) with repo + read:org scopes. AutoDev uses this for: reading Issues, creating PRs, and the QC agent merging approved branches.' },
      { t: 'h2', text: 'Workers (Fleet)' },
      { t: 'prose', text: 'Enable/disable individual worker recipes, override their model, and set their schedule. Changes take effect on the next scheduler tick without a restart.' },
      { t: 'h2', text: 'System Prompt' },
      { t: 'prose', text: 'Edit the AUTODEV.md content that is injected into every PA prompt. Also edit the three mode-specific system prompts: BASE, REVIEW, DELEGATE. Content is stored in SQLite (config.db), not as a file.' },
      { t: 'h2', text: 'Hooks' },
      { t: 'prose', text: 'Create, edit, enable/disable, and delete hooks from the HooksSettings component. Each hook shows its event, action, conditions, and last-fired timestamp.' },
    ],
  },

  {
    id: 'dashboard', num: '09', title: 'Dashboard Panel (AutoDev Panel)', category: 'Interface',
    icon: 'dashboard',
    lead: 'The Dashboard is the internal task board and agent roster — a live Kanban view of everything the fleet is working on.',
    anchors: ['Agent Roster', 'Task Board', 'Live Monitor', 'Worker Requests'],
    blocks: [
      { t: 'h2', text: 'Agent Roster' },
      { t: 'prose', text: 'The top section shows all configured workers with their current status, mode (LivingAgent / Rust Engine), assigned task, and elapsed time. Badges indicate: idle, working, failed, offline.' },
      { t: 'h2', text: 'Task Board (Kanban)' },
      { t: 'prose', text: 'Six columns represent the full task lifecycle. Cards show task ID, title, type, priority, and assigned worker. Click any card to expand its acceptance criteria and full details.' },
      { t: 'table', headers: ['Column', 'Description'], rows: [
        ['Backlog', 'Tasks created but not yet assigned'],
        ['Assigned', 'Assigned to a worker, waiting to start'],
        ['In Progress', 'Worker currently executing'],
        ['QC Review', 'Completed — QC agent or human reviewing'],
        ['Done', 'Merged, accepted'],
        ['Failed', 'Build failed or blocked'],
      ]},
      { t: 'h2', text: 'Live Monitor' },
      { t: 'prose', text: 'The Live Monitor widget streams real-time tool calls and agent output from the active worker. Powered by Tauri event channels (worker:stream events).' },
      { t: 'h2', text: 'Worker Requests Widget' },
      { t: 'prose', text: 'Shows pending worker_requests that need Supervisor or human attention. Each request shows: type badge, worker slug, task ID, payload preview. Actions: Acknowledge/Complete or Decline.' },
      { t: 'h2', text: 'Orphan Alert' },
      { t: 'prose', text: 'Warns when tasks are stuck in_progress without an active lock file (worker crashed or was killed). Click to auto-release orphans back to assigned state.' },
    ],
  },

  {
    id: 'ide', num: '10', title: 'IDE Panel', category: 'Interface',
    icon: 'code',
    lead: 'The IDE panel is a full read-only code explorer with file tree, Monaco editor, symbol outline, and an integrated Git visualization panel. All in one tab.',
    anchors: ['File Tree', 'Code Viewer', 'Git Panel', 'Symbol Outline', 'Checkpoints'],
    blocks: [
      { t: 'h2', text: 'Layout' },
      { t: 'code', lang: 'text', code: `┌──────────────┬────────────────────────────┬────────────┐
│ File Tree    │ Tab Bar          [Git ⌥]   │ Git Panel  │
│ (260px)      ├────────────────────────────┤ (360px)    │
│              │ Breadcrumb Nav             │ Changes    │
│ git badges   ├────────────────────────────┤ History    │
│ context menu │ Monaco Code Viewer         │ Branches   │
│ drag & drop  │ (read-only)                │ Stash      │
├──────────────│                            │ Undo       │
│ Symbol       ├────────────────────────────┤            │
│ Outline      │ IDE Status Bar             │            │
└──────────────┴────────────────────────────┴────────────┘` },
      { t: 'h2', text: 'File Tree' },
      { t: 'prose', text: 'Collapsible folder browser with real-time git status badges. Right-click any file or folder for: Open, Copy Path, Rename, Delete, New File, New Folder. Drag files onto folders to move them.' },
      { t: 'h2', text: 'Code Viewer (Monaco)' },
      { t: 'prose', text: 'Read-only Monaco Editor (VS Code\'s editor engine). AutoDev never edits files from the UI — agents use write_file/edit_file tools. The viewer applies a custom AutoDev Dark theme tuned to the industrial-dark palette.' },
      { t: 'callout', text: 'Read-only is intentional: it avoids stale-buffer bugs and prevents UI edits from conflicting with concurrent agent operations.' },
      { t: 'h2', text: 'Symbol Outline' },
      { t: 'prose', text: 'Tree-sitter-powered symbol extractor. Shows functions, classes, methods, types, and modules for the active file. Filter symbols by name; click to jump Monaco to the exact declaration line.' },
      { t: 'h2', text: 'Git Panel (5 Tabs)' },
      { t: 'table', headers: ['Tab', 'Features'], rows: [
        ['Changes', 'Staged / unstaged / untracked files with diff preview, stage/unstage/discard, commit form'],
        ['History', 'Commit log with SHA, author, date, branch badges, click to view diff'],
        ['Branches', 'Local + remote branches, create/delete/checkout, ahead/behind indicators'],
        ['Stash', 'Stash list, push/apply/drop/pop operations'],
        ['Undo', 'Auto-snapshot checkpoint timeline — restore to any pre-agent state'],
      ]},
      { t: 'h2', text: 'Checkpoints (Undo System)' },
      { t: 'prose', text: 'AutoDev automatically creates git checkpoints before agent file operations. The Undo tab shows a timeline of these snapshots. Click any checkpoint to restore the repo to that exact state — your most important safety net when agents make mistakes.' },
      { t: 'h2', text: 'Keyboard Shortcuts' },
      { t: 'table', headers: ['Shortcut', 'Action'], rows: [
        ['Ctrl/Cmd+P', 'Open fuzzy file search overlay'],
        ['Ctrl/Cmd+F', 'Find in file (Monaco built-in)'],
        ['Arrow keys', 'Navigate search results / file tree'],
        ['Escape', 'Close overlay / modal'],
      ]},
    ],
  },

  {
    id: 'graph', num: '11', title: 'Graph Panel', category: 'Interface',
    icon: 'chart-dots-3',
    lead: 'The Graph panel renders a D3.js force-directed visualization of the code knowledge graph — nodes are code entities, edges are relationships.',
    anchors: ['Nodes & Edges', 'Visualization', 'Communities', 'Use Cases'],
    blocks: [
      { t: 'h2', text: 'Nodes & Edges' },
      { t: 'table', headers: ['Node Type', 'Symbol'], rows: [
        ['File', '◆'],
        ['Class', '■'],
        ['Function', '●'],
        ['Test', '△'],
        ['Type/Interface', '◇'],
        ['Module/Package', '◈'],
        ['Import', '○'],
      ]},
      { t: 'table', headers: ['Edge Type', 'Meaning'], rows: [
        ['CALLS', 'Function A calls function B'],
        ['IMPORTS_FROM', 'File imports from another file'],
        ['INHERITS', 'Class extends another class'],
        ['IMPLEMENTS', 'Class implements interface'],
        ['TESTED_BY', 'Code covered by a test'],
        ['DEPENDS_ON', 'Module dependency'],
        ['CONTAINS', 'File contains symbol'],
      ]},
      { t: 'h2', text: 'Building the Graph' },
      { t: 'prose', text: 'Click "Build Graph" to trigger tree-sitter extraction on the current workspace. Extraction runs incrementally — only changed files are re-parsed. Progress is shown via a build progress event stream.' },
      { t: 'h2', text: 'Communities' },
      { t: 'prose', text: 'The Leiden community detection algorithm clusters nodes into logical groups. Each community is color-coded in the visualization. Hover a node to see its community, file path, and edge count.' },
      { t: 'h2', text: 'Use Cases' },
      { t: 'list', items: [
        'Impact analysis — select a node, see all dependents before making a change',
        'Dead code detection — nodes with no incoming edges',
        'Test coverage — highlight nodes with/without TESTED_BY edges',
        'Circular dependency identification — find cycles in the graph',
        'Refactoring — identify opportunities to extract cohesive modules',
        'Onboarding — visually understand a new codebase\'s structure at a glance',
      ]},
    ],
  },

  {
    id: 'internals', num: '12', title: 'Technical Internals', category: 'Internals',
    icon: 'cpu',
    lead: 'Key implementation details of the AutoDev Rust/Python backend — scheduler, recipe runner, sidecar management, and the tool execution engine.',
    anchors: ['Scheduler', 'Recipe Runner', 'Sidecar Management', 'Tool Execution'],
    blocks: [
      { t: 'h2', text: 'Cron Scheduler' },
      { t: 'prose', text: 'RecipeScheduler (src-tauri/src/recipes/scheduler.rs) uses tokio tasks + the cron crate. An Arc<RecipeScheduler> lives for the app lifetime in Tauri managed state. A HashMap<String, JoinHandle> maps worker_slug → running task handle.' },
      { t: 'h2', text: 'Recipe Runner' },
      { t: 'prose', text: 'RecipeRunner (src-tauri/src/recipes/runner.rs) fires per tick. Priority: if core-orchestrator is healthy → dispatch to LivingAgent (Mode 2). Otherwise → Rust AgentEngine (Mode 1). Worker status is emitted as Tauri events consumed by the dashboard.' },
      { t: 'h2', text: 'Sidecar Management' },
      { t: 'prose', text: 'SidecarManager (src-tauri/src/sidecar/) starts core-orchestrator and memory-mcp-server as child processes bundled with the app. Reads stdout/stderr for health detection. Restart is automatic on crash.' },
      { t: 'h2', text: 'Tool Execution Engine' },
      { t: 'prose', text: 'AgentEngine (src-tauri/src/agent/engine.rs) implements the ReAct loop: LLM call → tool parse → tool dispatch → result injection → repeat. Tool hints are appended inline to large results (grep > 50 matches, glob > 100 results) to guide the agent without token explosion.' },
      { t: 'h2', text: 'MCP Client' },
      { t: 'prose', text: 'AutoDev does not currently call external MCP servers. src-tauri/src/mcp/ registers user-configured servers and polls them for liveness, surfacing the result in Settings; nothing consumes their tools yet. AutoDev also no longer HOSTS an MCP server — it previously ran one on localhost so sidecar-hosted workers could reach its tools, which was an unauthenticated local endpoint exposing bash, write_file and spawn_subagent, and it was removed. Fleet workers now call those tools in-process.' },
      { t: 'h2', text: 'Vault (OS Keychain)' },
      { t: 'prose', text: 'API keys are stored via Tauri\'s vault module — macOS Keychain, Windows Credential Manager, or Linux SecretService. Never stored in plain files or SQLite. The vault is unlocked per-session with the OS credentials.' },
    ],
  },

  {
    id: 'submodules', num: '13', title: 'Submodules', category: 'Internals',
    icon: 'git-branch',
    lead: 'AutoDev bundles two Python components as git submodules: core-orchestrator (LivingAgent FSMs + A2A) and memory-mcp-server (mem0 memory system).',
    anchors: ['Core Orchestrator', 'Memory MCP', 'Workflow', 'Troubleshooting'],
    blocks: [
      { t: 'h2', text: 'Structure' },
      { t: 'code', lang: 'text', code: `autodev/
├── .gitmodules
└── submodules/
    ├── core-orchestrator/   # Python 3.13 + FastAPI + litellm
    │   └── src/airs/
    │       ├── agent/living_agent.py   # 21 FSMs
    │       ├── bridges/                # Tool handlers
    │       └── models/                 # Agent schema
    └── memory-mcp-server/   # Python 3.13 + mem0
        └── src/
            └── main.py                # MCP server entry` },
      { t: 'h2', text: 'Core Orchestrator' },
      { t: 'prose', text: 'Runs at port 4000. Provides LivingAgent (21 FSMs governing agent lifecycle), A2A protocol for worker coordination, and litellm for model-agnostic LLM calls. Workers are created via POST /api/agents; ticks are fired via POST /api/agents/{id}/run.' },
      { t: 'h2', text: 'Memory MCP Server' },
      { t: 'prose', text: 'Runs at port 3001. Provides a 3-tier memory system (episodic / semantic / procedural) via mem0. Agents connect as an MCP client and use add_memory, search_memory, get_memories tools to maintain context across sessions.' },
      { t: 'h2', text: 'Submodule Workflow' },
      { t: 'code', lang: 'bash', code: `# Edit submodule code
cd submodules/core-orchestrator
git checkout main
# ... make changes ...
git add . && git commit -m "fix: description"
git push origin main

# Update parent repo pointer
cd ../..
git add submodules/core-orchestrator
git commit -m "chore: bump core-orchestrator"

# Update all submodules to latest
git submodule update --remote --merge` },
      { t: 'callout', text: 'The parent repo tracks commit SHA pointers, not file contents. Always commit inside the submodule first, then update the pointer in the parent repo.' },
    ],
  },

  {
    id: 'glossary', num: '14', title: 'Glossary', category: 'Internals',
    icon: 'alphabet-latin',
    lead: 'Key terms and concepts used throughout AutoDev.',
    anchors: ['A–F', 'G–L', 'M–R', 'S–Z'],
    blocks: [
      { t: 'h2', text: 'A–F' },
      { t: 'table', headers: ['Term', 'Definition'], rows: [
        ['A2A Protocol', 'Agent-to-Agent communication — how Supervisor coordinates Workers'],
        ['AgentEngine', 'Rust-native ReAct loop (PA Mode 1)'],
        ['AUTODEV.md', 'Proprietary AIRS instruction file stored in SQLite, injected into every prompt'],
        ['Builtin Tool', 'Rust-implemented tool (read_file, write_file, bash, grep, …)'],
        ['Checkpoint', 'Auto-snapshot git stash created before agent file operations'],
        ['core-orchestrator', 'Python sidecar providing LivingAgent FSMs and A2A protocol'],
        ['Delegate Mode', 'PA sub-mode with only create_task — all work dispatched to fleet'],
        ['Fast Development', 'PA sub-mode with full tool access and no confirmation prompts'],
      ]},
      { t: 'h2', text: 'G–L' },
      { t: 'table', headers: ['Term', 'Definition'], rows: [
        ['git2-rs', 'Rust bindings to libgit2 — AutoDev\'s git engine'],
        ['Hook', 'Event-driven callback fired at agent lifecycle points'],
        ['LivingAgent', 'core-orchestrator\'s FSM-based agent (Mode 2 workers)'],
        ['Lock File', 'tasks/{id}.lock file that prevents double-processing of a task'],
      ]},
      { t: 'h2', text: 'M–R' },
      { t: 'table', headers: ['Term', 'Definition'], rows: [
        ['MCP', 'Model Context Protocol — standard for connecting AI to external tools'],
        ['mem0', 'Memory library used by memory-mcp-server for 3-tier agent memory'],
        ['PA Mode', 'Personal Assistant Mode — interactive, user-facing agent'],
        ['Recipe', 'YAML-defined agentic workflow with optional cron schedule'],
        ['RecipeRunner', 'Rust module that dispatches recipe ticks to Mode 1 or Mode 2'],
        ['RecipeScheduler', 'Rust cron daemon that fires recipe ticks on schedule'],
        ['MCP', 'Model Context Protocol. AutoDev registers and health-checks external MCP servers; it neither calls them nor hosts one.'],
      ]},
      { t: 'h2', text: 'S–Z' },
      { t: 'table', headers: ['Term', 'Definition'], rows: [
        ['Skill', 'Reusable markdown workflow template invocable via slash command'],
        ['Supervisor', 'Fleet orchestrator recipe — assigns tasks, handles worker requests'],
        ['Tauri', 'Rust + WebView desktop framework used for the AutoDev shell'],
        ['tasks.db', 'SQLite database at ~/.autodev/tasks.db holding all fleet tasks'],
        ['Vault', 'OS keychain integration (macOS Keychain / Windows Credential Manager)'],
        ['Worker', 'Autonomous fleet agent running a recipe on a schedule'],
        ['Worktree', 'Git worktree branch (task/{id}-{slug}) isolating per-worker changes'],
        ['Zustand', 'Lightweight React state management library used throughout the frontend'],
      ]},
    ],
  },
]
