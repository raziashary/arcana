// Documentation — Sections 08–11
// Full content faithful to AUTODEV_DOCUMENTATION.md

import type { DocSection } from './docs'

export const C_SECTIONS: DocSection[] = [

  /* ── 08: Settings Panel ─────────────────────────────────────────── */
  {
    id: 'settings', num: '08', title: 'Settings Panel', category: 'Interface', icon: 'settings',
    lead: 'The Settings panel is the central configuration hub for AutoDev. It manages providers, models, workers, GitHub integration, system prompts, hooks, and more. Location: src/tabs/Settings.tsx',
    anchors: ['Workspace', 'Demo Mode', 'Services', 'Submodules', 'MCP Tools', 'Providers', 'Model Parameters', 'GitHub', 'Workers', 'System Prompt', 'Hooks', 'Updates'],
    blocks: [
      { t: 'h2', text: '1. Workspace Folder' },
      { t: 'prose', text: 'Sets the root directory for all AutoDev agents (PA, Supervisor, Workers). All agents operate inside this folder. @path/to/file references in chat are relative to workspace. Default: None (agents use sandbox directories). Storage: workspaceStore.ts (localStorage).' },
      { t: 'h2', text: '2. Demo Mode' },
      { t: 'prose', text: 'Enables demo mode for testing without API keys. Mocks LLM responses, no actual API calls. For UI testing and demos.' },
      { t: 'h2', text: '3. Services (Sidecar Panel)' },
      { t: 'prose', text: 'Monitor and control Python sidecar processes. Status: green dot = running, red dot = stopped, yellow dot = starting. Actions: Start/Stop each sidecar independently.' },
      { t: 'table', headers: ['Sidecar', 'Port', 'Description'], rows: [
        ['Core Orchestrator', '4000', 'FastAPI agent runtime (LivingAgent, FSMs, A2A)'],
        ['Memory MCP Server', '5000', 'FastMCP memory server (mem0, 6 memory tools)'],
        ['Redis', '6379', 'Cache server for warm-tier memory'],
      ]},
      { t: 'h2', text: '4. Submodules' },
      { t: 'prose', text: 'Display git submodule status and versions. Shows: commit SHA, branch, last updated, remote URL.' },
      { t: 'h2', text: '5. MCP Tools' },
      { t: 'prose', text: 'Browse and manage MCP server connections. Features: list all configured MCP servers, show available tools from each server, add/remove MCP server configurations, test MCP server connectivity.' },
      { t: 'h2', text: '6. Providers & Models' },
      { t: 'prose', text: 'Configure LLM provider API keys and settings. Per-provider: API key (stored in OS keychain), base URL (for custom endpoints), model selection, test connection button. Storage: OS keychain via keyring crate.' },
      { t: 'table', headers: ['Provider', 'Models'], rows: [
        ['Anthropic', 'claude-haiku-4-5, claude-sonnet-4-5, claude-opus-4-5'],
        ['OpenAI', 'gpt-4o, gpt-4o-mini, o1, o3-mini'],
        ['Google Gemini', 'gemini-2.5-pro, gemini-2.5-flash'],
        ['Ollama', 'Local models with custom endpoint'],
        ['Azure OpenAI', 'Custom deployment URL + key'],
        ['AWS Bedrock', 'Regional Claude/Titan models'],
      ]},
      { t: 'h2', text: '7. Model Parameters' },
      { t: 'table', headers: ['Parameter', 'Range', 'Purpose'], rows: [
        ['Temperature', '0.0 – 2.0', 'Controls randomness of responses'],
        ['Top P', '0.0 – 1.0', 'Nucleus sampling probability'],
        ['Thinking Budget', 'Tokens', 'Extended thinking budget for Claude models'],
        ['Reasoning Effort', 'low/medium/high', 'Reasoning effort for OpenAI o1/o3 models'],
      ]},
      { t: 'h2', text: '8. GitHub Configuration' },
      { t: 'prose', text: 'GitHub PAT Card: Store GitHub Personal Access Token (used for GitHub Issues API, stored in OS keychain). GitHub MCP Card: configure GitHub MCP server (URL, auth, tool list: create_issue, list_issues, etc.).' },
      { t: 'h2', text: '9. Workers' },
      { t: 'prose', text: 'Manage the worker fleet. Worker table: list all workers (system + custom), status indicators (idle/working/error/paused), start/stop buttons, edit configuration, delete workers (except system-defined). Storage: ~/.autodev/workers.db (SQLite).' },
      { t: 'table', headers: ['Worker Config Field', 'Description'], rows: [
        ['name, slug, type', 'Identity fields'],
        ['model', 'Model preference (overrides global)'],
        ['primary_repositories', 'Repos this worker focuses on'],
        ['polling_interval', 'How often to poll task board'],
        ['bound_recipe', 'YAML recipe this worker runs'],
        ['description', 'Human-readable description'],
      ]},
      { t: 'h2', text: '10. System Prompt' },
      { t: 'prose', text: 'Edit the three PA mode system prompts. Storage: ~/.autodev/config.db (SQLite). AUTODEV.md Integration: global AUTODEV.md injected into all modes; repo-specific AUTODEV.md injected when in workspace; system prompt + AUTODEV.md = full prompt.' },
      { t: 'table', headers: ['Prompt', 'Used In'], rows: [
        ['Base System Prompt', 'Fast Development mode'],
        ['Review System Prompt', 'Review mode (adds permission consciousness)'],
        ['Delegate System Prompt', 'Delegate mode (task creation focused)'],
      ]},
      { t: 'h2', text: '11. Hooks' },
      { t: 'prose', text: 'Configure lifecycle hooks. Hook Types: PreToolUse/PostToolUse, SessionStart/SessionEnd, AgentLoopStart/AgentLoopEnd, OnToolError/OnError. Actions: Script, Modify, HTTP. UI: list/enable/disable/add/test/delete hooks, view execution logs.' },
      { t: 'h2', text: '12. Updates' },
      { t: 'prose', text: 'Check for and apply AutoDev updates. Features: check for updates, download and apply updates, view update history.' },
    ],
  },

  /* ── 09: Dashboard Panel (AutoDev Panel) ───────────────────────── */
  {
    id: 'dashboard', num: '09', title: 'Dashboard Panel (AutoDev Panel)', category: 'Interface', icon: 'dashboard',
    lead: 'The AutoDev Panel is the central dashboard for monitoring the worker fleet and task board. It displays real-time agent status, task progress, and fleet metrics. Location: src/tabs/AutoDevPanel.tsx',
    anchors: ['Layout', 'Agent Roster', 'Task Board', 'Metrics Footer', 'Live Monitor', 'Worker Requests', 'Orphan Alert'],
    blocks: [
      { t: 'h2', text: 'Layout' },
      { t: 'code', lang: 'text', code: `┌─────────────────────────────────────────────────────────────┐
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
│  └─────────┴─────────────┴───────────┴─────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│  Metrics Footer                                            │
│  Cost Today: $2.34 | Tokens: 45.2K | Closed: 12 | QC: 85% │
└─────────────────────────────────────────────────────────────┘` },
      { t: 'h2', text: 'Agent Roster' },
      { t: 'prose', text: 'Real-time status of all fleet workers. Refreshes every second (elapsed timers). Pulls live tool/file info from agentLiveStore. Warns when a tool runs > 2 minutes.' },
      { t: 'table', headers: ['Column', 'Description'], rows: [
        ['Status Dot', 'Green = working, gray = idle, red = error. Glow effect when actively executing'],
        ['Mode Badge', 'LA (green) = Mode 2 LivingAgent via core-orchestrator; M1 (yellow) = Mode 1 Rust Engine fallback'],
        ['Agent Name', 'Uppercase slug (e.g., SUPERVISOR, WORKER-BACK)'],
        ['Current Task', 'Task ID + title if working, status message if idle'],
        ['Elapsed Time', 'MM:SS since task started or last tick'],
      ]},
      { t: 'h2', text: 'Task Board (Kanban)' },
      { t: 'table', headers: ['Column', 'Tasks Shown'], rows: [
        ['Ready', 'Tasks waiting for Supervisor assignment (backlog)'],
        ['In Progress', 'Worker is actively working — has lock file'],
        ['QC Review', 'Worker completed, PR opened, QC agent reviewing'],
        ['Done Today', 'Completed tasks filtered by today\'s date'],
      ]},
      { t: 'h3', text: 'Task Cards Show:' },
      { t: 'list', items: [
        'Task ID (#N) and title',
        'Priority badge (p0/p1/p2/p3)',
        'Type badge (feature/bug/refactor/docs/chore)',
        'Assigned worker (if assigned)',
        'Acceptance criteria (click to expand)',
        'Delete button (if in Ready state)',
        'Reassign button (if assigned)',
      ]},
      { t: 'h2', text: 'Metrics Footer' },
      { t: 'table', headers: ['Metric', 'Description'], rows: [
        ['Cost Today', 'Total LLM spend for the day'],
        ['Tokens', 'Total tokens consumed (input + output)'],
        ['Closed', 'Number of tasks completed today'],
        ['QC Pass Rate', 'Percentage of QC reviews that passed'],
      ]},
      { t: 'h2', text: 'Live Monitor' },
      { t: 'prose', text: 'Real-time streaming output from active agents. Shows live tool execution, file operations, bash command output. Auto-scrolls to latest. Filter by agent. Powered by Tauri event channels (worker:stream events).' },
      { t: 'h2', text: 'Worker Requests Widget' },
      { t: 'prose', text: 'Displays pending worker → Supervisor requests needing attention. Auto-refreshes on worker:request event and every 60s. Actions: Acknowledge/Complete, Decline, add resolution note.' },
      { t: 'h2', text: 'Orphan Alert' },
      { t: 'prose', text: 'Warns when tasks are stuck in in_progress without an active lock file (worker crashed or killed). Click to auto-release orphans back to assigned state for re-claiming.' },
    ],
  },

  /* ── 10: IDE Panel ──────────────────────────────────────────────── */
  {
    id: 'ide', num: '10', title: 'IDE Panel', category: 'Interface', icon: 'code',
    lead: 'The IDE panel provides a full-featured code explorer with file tree, Monaco code viewer, symbol outline, and integrated Git visualization. Location: src/tabs/IDE.tsx',
    anchors: ['Layout', 'File Tree', 'Code Viewer', 'Tab Bar', 'Symbol Outline', 'File Search', 'Status Bar', 'Git Panel', 'Keyboard Shortcuts', 'Use Cases'],
    blocks: [
      { t: 'h2', text: 'Layout' },
      { t: 'code', lang: 'text', code: `┌──────────────────┬─────────────────────────────┬────────────┐
│ File Tree        │ Tab Bar          [Git ⌥]    │ Git Panel  │
│ (260px)          ├─────────────────────────────┤ (360px)    │
│                  │ Breadcrumb Nav              │ Changes    │
│ git badges       ├─────────────────────────────┤ History    │
│ context menu     │                             │ Branches   │
│ drag & drop      │  Monaco Code Viewer         │ Stash      │
│                  │  (read-only)                │ Undo       │
├──────────────────│                             │ (checkpts) │
│ Symbol Outline   ├─────────────────────────────┤            │
│ (tree-sitter)    │ IDE Status Bar              │            │
└──────────────────┴─────────────────────────────┴────────────┘
FileSearch overlay (Ctrl/Cmd+P) — fuzzy file search` },
      { t: 'h2', text: 'File Tree' },
      { t: 'prose', text: 'Location: src/components/ide/FileTree.tsx. Collapsible folder browser with auto-refresh when files change on disk. Keyboard: arrow keys to navigate, Enter to open.' },
      { t: 'table', headers: ['Feature', 'Detail'], rows: [
        ['Git status badges', 'Green = Modified, Yellow = Staged, Blue = Untracked, Red = Deleted'],
        ['Right-click context menu', 'Open, Copy Path, Rename, Delete, New File, New Folder'],
        ['Drag-and-drop', 'Move files by dragging onto a folder'],
      ]},
      { t: 'h2', text: 'Code Viewer (Monaco)' },
      { t: 'prose', text: 'Location: src/components/ide/CodeViewer.tsx. Full Monaco Editor (VS Code\'s editor engine), read-only. Features: syntax highlighting, line numbers with active line highlighting, minimap, binary file detection, encoding detection, language auto-detection, goto line support.' },
      { t: 'callout', text: 'Read-only is intentional: AutoDev agents edit files via write_file and edit_file tools. The UI viewer is for exploration. Avoids stale-buffer bugs and prevents conflicts with agent operations.' },
      { t: 'h3', text: 'Custom AutoDev Dark Theme Colors' },
      { t: 'table', headers: ['Element', 'Color'], rows: [
        ['Background', '#01111D'],
        ['Foreground', '#D6DEEB'],
        ['Comments', '#637777 (italic)'],
        ['Keywords', '#82AAFF'],
        ['Strings', '#ECC48D'],
        ['Numbers', '#F78C6C'],
        ['Types', '#7FDBCA'],
        ['Functions', '#82AAFF'],
      ]},
      { t: 'h2', text: 'Tab Bar' },
      { t: 'prose', text: 'Location: src/components/ide/TabBar.tsx. Multiple tabs for simultaneously open files. Tab close button, active tab highlight, tab overflow scroll, git modified badge per tab, click to switch.' },
      { t: 'h2', text: 'Breadcrumb Navigation' },
      { t: 'prose', text: 'Location: src/components/ide/BreadcrumbNav.tsx. Shows file path as clickable breadcrumbs (navigate hierarchy). Workspace-relative display with compact truncation for long paths.' },
      { t: 'h2', text: 'Symbol Outline' },
      { t: 'prose', text: 'Location: src/components/ide/SymbolOutline.tsx. Tree-sitter parsing extracts symbols. Click any symbol to jump Monaco to its declaration line. Supported languages: Rust, TypeScript/JavaScript, Python, Go, Java, C/C++, more via tree-sitter.' },
      { t: 'table', headers: ['Symbol Kind', 'Icon', 'Description'], rows: [
        ['Function', 'Code2', 'Functions and methods'],
        ['Variable', 'Hash', 'Variables and constants'],
        ['Class', 'Layers', 'Class definitions'],
        ['Module', 'Package', 'Modules and packages'],
        ['Type/Interface', 'TypeIcon', 'Type aliases and interfaces'],
      ]},
      { t: 'h2', text: 'File Search' },
      { t: 'prose', text: 'Location: src/components/ide/FileSearch.tsx. Keyboard shortcut: Ctrl/Cmd+P. Fuzzy search — finds files by name. Shows recent files when query is empty. 120ms debounce. Max 40 results. Keyboard navigation: arrows to navigate, Enter to open, Escape to close.' },
      { t: 'h2', text: 'IDE Status Bar' },
      { t: 'prose', text: 'Location: src/components/ide/IdeStatusBar.tsx. Shows: file path, encoding (UTF-8/UTF-16), line count, file size (B/KB/MB), detected language (colored accent), cursor position (reserved for future).' },
      { t: 'h2', text: 'Git Panel (5 Tabs)' },
      { t: 'prose', text: 'Location: src/components/git/GitPanel.tsx. Toggle: click "Git" button in tab header. Width: 360px (min 280px).' },
      { t: 'table', headers: ['Tab', 'Key Features'], rows: [
        ['Changes', 'Staged/unstaged/untracked files, diff preview (DiffViewer), stage/unstage/discard, commit form with message + trailer'],
        ['History', 'Full commit log, SHA/author/date/message, branch badges, relative time, click to see diff, Fetch/Push/Pull buttons'],
        ['Branches', 'Local + remote branches, current branch highlighted, create/delete/checkout, ahead/behind indicators'],
        ['Stash', 'Stash list, push/apply/drop/pop operations'],
        ['Undo (Checkpoints)', 'Auto-snapshot timeline before agent ops, restore to any checkpoint state, checkpoint metadata (agent/task/operation)'],
      ]},
      { t: 'h2', text: 'Keyboard Shortcuts' },
      { t: 'table', headers: ['Shortcut', 'Action'], rows: [
        ['Ctrl/Cmd+P', 'Open fuzzy file search overlay'],
        ['Ctrl/Cmd+F', 'Find in file (Monaco built-in)'],
        ['Ctrl/Cmd+G', 'Find next (Monaco built-in)'],
        ['Escape', 'Close overlay/modal'],
      ]},
      { t: 'h2', text: 'Use Cases' },
      { t: 'list', items: [
        'PA Mode: explore codebase, read files for context, navigate to lines via "Open in IDE from chat", review agent changes via Git panel, undo agent mistakes via Checkpoints',
        'Fleet Workers: workers don\'t use IDE directly (headless). Git ops done via tools (git2-rs). Checkpoints created automatically. Supervisor can review diffs via Git panel.',
        'Developers: full code exploration without external IDE, integrated git, visual diff review, symbol navigation, quick file search',
      ]},
      { t: 'h2', text: 'Future Enhancements' },
      { t: 'list', items: [
        'Cursor tracking — show cursor position in status bar',
        'Find in files — global search across workspace',
        'Refactoring support — rename symbols, extract functions',
        'Multi-file diff — compare multiple files',
        'Blame view — git blame per line',
        'Merge conflict resolution — visual merge tool',
        'Terminal integration — embedded terminal panel',
        'Debug integration — breakpoints, step-through',
      ]},
    ],
  },

  /* ── 11: Graph Panel ────────────────────────────────────────────── */
  {
    id: 'graph', num: '11', title: 'Graph Panel', category: 'Interface', icon: 'chart-dots-3',
    lead: 'The Graph panel provides a D3.js force-directed visualization of the code knowledge graph. It enables visual exploration of code structure, dependencies, and relationships. Location: src/tabs/Graph.tsx',
    anchors: ['What is the Graph', 'Building the Graph', 'Visualization Controls', 'Node & Edge Visualization', 'Interactions', 'Community View', 'Analysis Buttons', 'Use Cases'],
    blocks: [
      { t: 'h2', text: 'What is the Graph?' },
      { t: 'prose', text: 'The code knowledge graph is a structured representation of your codebase as nodes and edges.' },
      { t: 'table', headers: ['Node Type', 'Shape', 'Represents'], rows: [
        ['File', 'Diamond ◆', 'Source files'],
        ['Class', 'Square ■', 'Class definitions'],
        ['Function/Method', 'Circle ●', 'Functions and methods'],
        ['Test', 'Triangle △', 'Test functions'],
        ['Type/Interface', 'Star ★', 'Types and interfaces'],
        ['Module/Package', 'Wye ⑂', 'Modules and packages'],
        ['Import', 'Cross ✚', 'Import statements'],
      ]},
      { t: 'table', headers: ['Edge Type', 'Color', 'Meaning'], rows: [
        ['CALLS', '#00FF88 green', 'Function calls another function'],
        ['IMPORTS_FROM', '#4477FF blue', 'File imports from another'],
        ['INHERITS', '#B07AA1 purple', 'Class extends another class'],
        ['IMPLEMENTS', '#76B7B2 teal', 'Class implements interface'],
        ['TESTED_BY', '#F28E2B orange', 'Code is covered by a test'],
        ['DEPENDS_ON', '#EDC948 yellow', 'Module dependency'],
        ['REFERENCES', '#888888 gray', 'Symbol references'],
        ['CONTAINS', '#2A2A2A dark gray', 'File contains symbol (no arrowhead)'],
        ['SEMANTICALLY_SIMILAR', '#9C755F brown', 'Semantic similarity (no arrowhead)'],
      ]},
      { t: 'h2', text: 'Building the Graph' },
      { t: 'prose', text: 'Click "Build Graph" to trigger extraction. Process runs incrementally — only changed files are re-parsed. Progress bar shows phases.' },
      { t: 'list', items: [
        '1. Detection — scan workspace for code files',
        '2. Extraction — parse files with tree-sitter extractors',
        '3. Clustering — group related nodes into communities (Leiden algorithm)',
        '4. Analysis — compute graph metrics (density, centrality, cohesion)',
        '5. Storage — save to SQLite (graph.db)',
      ]},
      { t: 'h2', text: 'Visualization Controls' },
      { t: 'h3', text: 'Toolbar Row 1' },
      { t: 'list', items: [
        'Workspace path display',
        'Build button — trigger full graph build',
        'Update button — incremental update (changed files only)',
        'Export dropdown — JSON, GEXF (Gephi format), SVG image',
        'Fit button — reset zoom and center graph',
        'Delete button — clear graph data',
      ]},
      { t: 'h3', text: 'Toolbar Row 2' },
      { t: 'list', items: [
        'Render Mode: Full (all nodes) / Community (aggregated groups) / Auto (switch by count)',
        'Edge Filters — toggle each edge type on/off',
        'Search — find nodes by name or file path',
        'Min Degree — filter nodes by minimum connections (default: 1)',
      ]},
      { t: 'h2', text: 'Node Visualization Details' },
      { t: 'prose', text: 'Size based on degree: nodeRadius(degree) = min(22, 4 + log(1+degree)*3.5). More connections = larger node. Colors are community-based (10-color palette — nodes in same community share color). God Nodes: nodes with degree ≥ 15 get outer glow ring.' },
      { t: 'h2', text: 'Stats Bar' },
      { t: 'list', items: ['Total nodes', 'Total edges', 'Total communities', 'Average degree', 'Graph density', 'Last updated timestamp'] },
      { t: 'h2', text: 'Interactions' },
      { t: 'table', headers: ['Interaction', 'Result'], rows: [
        ['Click Node', 'Focus mode — dims non-neighbors, highlights 1-hop ring'],
        ['Right-click Node', 'Context menu: Open in IDE, Show incoming/outgoing edges, Find in codebase, Copy node ID'],
        ['Drag Node', 'Reposition node (force simulation adjusts)'],
        ['Scroll', 'Zoom in/out'],
        ['Click + drag background', 'Pan the view'],
        ['Hover Node', 'Tooltip: name, kind, file, community, degree'],
      ]},
      { t: 'h2', text: 'Community Visualization' },
      { t: 'prose', text: 'Community Hulls: colored polygons around nodes in same community. Cohesion Score: green (> 0.5 high), yellow (0.25–0.5 medium), red (< 0.25 low). Community Panel (right sidebar): list all communities, show member count and cohesion, click to filter to community.' },
      { t: 'h2', text: 'Node Inspector' },
      { t: 'prose', text: 'Right sidebar for selected node. Shows: name, kind, language, file path, community, degree. Incoming edges (nodes that reference this), outgoing edges (nodes this references), edge kind badges, click to navigate.' },
      { t: 'h2', text: 'Analysis Buttons' },
      { t: 'list', items: [
        'Centrality — find most central nodes (high betweenness centrality)',
        'Bridges — find nodes that connect communities',
        'Clusters — find tightly-knit clusters',
        'Isolates — find disconnected nodes',
      ]},
      { t: 'h2', text: 'Use Cases' },
      { t: 'list', items: [
        'Impact Analysis: select a node → see all dependents before making a change',
        'Dead code detection: nodes with no incoming edges',
        'Test coverage: highlight nodes without TESTED_BY edges',
        'Circular dependency identification: find cycles in the graph',
        'Architecture review: identify tightly-coupled modules (high degree), find community boundaries',
        'Refactoring: identify opportunities to extract cohesive modules',
        'Onboarding: visually understand a new codebase at a glance',
      ]},
    ],
  },
]
