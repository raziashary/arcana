# Claude Code Source Architecture Dissection

> **Project**: Claude Code (claude-code-source-main)  
> **Type**: Node.js/TypeScript CLI Application (Ink-based React for CLI)  
> **Generated**: Comprehensive architectural documentation  
> **Last Updated**: 2025

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture Patterns](#architecture-patterns)
3. [Entry Points](#entry-points)
4. [Core Systems](#core-systems)
5. [Tool System](#tool-system)
6. [MCP Integration](#mcp-integration)
7. [Subagent System](#subagent-system)
8. [State Management](#state-management)
9. [Services](#services)
10. [UI/Components](#uicomponents)
11. [Utilities](#utilities)
12. [Key Relationships](#key-relationships)

---

## Overview

Claude Code is a **Node.js/TypeScript CLI tool** (not a desktop app) built with **Ink** (React for CLI). It's a sophisticated agentic coding assistant that runs in the terminal.

### Technology Stack

- **Framework**: Node.js with Bun bundler
- **UI**: Ink (React for CLI terminals)
- **State**: Zustand-like custom store pattern
- **MCP**: Full Model Context Protocol SDK client
- **Build**: Bun with feature flags for dead code elimination

### Key Architectural Features

1. **Agent Loop**: Main query.ts runs the agentic loop - sends messages to LLM, receives tool_use blocks, executes them
2. **Tool System**: Tools are objects with name, description, inputSchema, call(), checkPermissions(), isConcurrencySafe()
3. **Tool Orchestration**: Read-only tools run concurrently (max 10), write tools run serially
4. **MCP Client**: Full MCP SDK implementation with stdio/sse/http transports
5. **Subagents**: Child agents with isolated contexts, own tools, own MCP servers
6. **Hooks System**: PreToolUse, PostToolUse, SessionStart, SubagentStart hooks
7. **Skills System**: Prompt-based reusable workflows loaded from files

---

## Architecture Patterns

### 1. Tool System Pattern

Tools are objects implementing the Tool interface:

```typescript
interface Tool {
  name: string
  description: string
  inputSchema: JSONSchema
  call(input: unknown, context: ToolContext): Promise<ToolResult>
  checkPermissions(input: unknown, context: ToolContext): Promise<PermissionResult>
  isConcurrencySafe(): boolean
}
```

### 2. Agent Loop Pattern

```
query.ts
  ↓
Send message to LLM
  ↓
Receive tool_use blocks
  ↓
PartitionToolCalls() → Read-only (concurrent) vs Write (serial)
  ↓
StreamingToolExecutor or runTools (toolOrchestration.ts)
  ↓
Yield results back to loop
```

### 3. MCP Client Pattern

- Multiple transports: stdio, sse, http, ws, sdk
- Server configs from multiple scopes: project, user, enterprise, dynamic, claudeai, managed
- Tools discovered via `tools/list`, called via `call_tool`
- OAuth support for authenticated servers

### 4. State Management Pattern

```typescript
// Zustand-like pattern
const [state, setState] = useState(initialState)
const getAppState = () => state
const setAppState = (updater) => setState(updater)
```

### 5. Subagent Pattern

- runAgent.ts creates child agents
- Isolated contexts
- Own tools and MCP servers
- AgentTool dispatches to subagents

---

## Entry Points

### `src/main.tsx`

**Purpose**: Main entry point for the application (803KB file - very large)

**Key Functions**:
- Bootstraps the application
- Sets up React with Ink renderer
- Initializes AppStateProvider
- Loads plugins if --plugin-dir specified
- Handles trust dialog
- Starts the main REPL or runs initial command

**Dependencies**:
- Uses: `init.ts`, `AppStateProvider`, `MailboxProvider`, `VoiceProvider`
- Imports: Dynamic imports for various subsystems

---

### `src/entrypoints/cli.tsx`

**Purpose**: CLI bootstrap entrypoint - checks for special flags before loading full CLI

**Key Features**:
- Fast-path for --version (zero imports)
- Fast-path for --dump-system-prompt
- Handles various entry modes:
  - `--claude-in-chrome-mcp`
  - `--chrome-native-host`
  - `--computer-use-mcp`
  - `--daemon-worker`
  - `remote-control` / `bridge`
  - `daemon`
  - `ps|logs|attach|kill` (background sessions)

**Fast Path Optimization**:
```typescript
// Dynamic imports to minimize module evaluation
const { enableConfigs } = await import('../utils/config.js')
const { bridgeMain } = await import('../bridge/bridgeMain.js')
```

---

### `src/entrypoints/init.ts`

**Purpose**: Application initialization with memoization

**Key Functions**:
```typescript
export const init = memoize(async (): Promise<void> => {
  // 1. Enable configs
  // 2. Apply safe environment variables
  // 3. Setup graceful shutdown
  // 4. Initialize 1P event logging
  // 5. Configure global mTLS
  // 6. Configure global HTTP agents (proxy)
  // 7. Preconnect to Anthropic API
  // 8. Setup git-bash for Windows
  // 9. Register cleanup handlers
})
```

**Key Services Initialized**:
- Config system (`enableConfigs()`)
- Environment variables (`applySafeConfigEnvironmentVariables()`)
- mTLS (`configureGlobalMTLS()`)
- Proxy agents (`configureGlobalAgents()`)
- Analytics (`initialize1PEventLogging()`)
- OAuth account info
- JetBrains IDE detection
- GitHub repository detection
- Remote managed settings
- Policy limits
- Telemetry (lazy-loaded)

---

### `src/entrypoints/mcp.ts`

**Purpose**: MCP server entry point

**Exports**:
- `runClaudeInChromeMcpServer()`
- `runComputerUseMcpServer()`

---

## Core Systems

### `src/query.ts` (68KB)

**Purpose**: Core agentic loop implementation - sends messages to LLM and processes tool calls

**Key Components**:
```typescript
export async function* query(
  messages: Message[],
  options: QueryOptions
): AsyncGenerator<QueryResult, void, unknown>
```

**Agent Loop Flow**:
1. Send messages to LLM via Anthropic SDK
2. Stream response chunks
3. Detect tool_use blocks
4. Partition tool calls (concurrent vs serial)
5. Execute tools via StreamingToolExecutor
6. Yield results for UI display
7. Continue until completion or error

**Key Types**:
```typescript
interface QueryOptions {
  commands: Command[]
  debug: boolean
  mainLoopModel: string
  tools: Tools
  verbose: boolean
  thinkingConfig: ThinkingConfig
  mcpClients: MCPServerConnection[]
  agentDefinitions: AgentDefinitionsResult
  maxBudgetUsd?: number
  customSystemPrompt?: string
}
```

---

### `src/QueryEngine.ts` (47KB)

**Purpose**: Query execution engine - handles LLM API interaction and streaming

**Key Components**:
```typescript
export class QueryEngine {
  async executeQuery(request: QueryRequest): Promise<QueryResult>
  async streamQuery(request: QueryRequest): AsyncIterable<StreamChunk>
  private handleToolUse(toolCall: ToolCall): Promise<ToolResult>
}
```

**Features**:
- Handles streaming from Anthropic API
- Tool use detection and routing
- Message history management
- Context window management
- Cost tracking integration

---

### `src/commands.ts` (25KB)

**Purpose**: Command definitions for slash commands in the REPL

**Key Components**:
```typescript
export interface Command {
  type: string
  name: string
  description: string
  execute(args: string[], context: CommandContext): Promise<void>
}
```

**Available Commands**:
- `/clear` - Clear conversation
- `/cost` - Show cost summary
- `/debug` - Toggle debug mode
- `/compact` - Compact conversation history
- `/model` - Switch models
- `/mcp` - MCP server management
- `/agent` - Agent management
- `/exit` or `/quit` - Exit

---

## Tool System

### `src/tools.ts` (17KB)

**Purpose**: Tool registry and lookup functions

**Key Exports**:
```typescript
export function findToolByName(name: string): Tool | undefined
export function registerTool(tool: Tool): void
export function getAllTools(): Tool[]
export const TOOLS: Record<string, Tool>
```

**Tool Categories**:
1. **File Tools**: Bash, FileRead, FileEdit, FileWrite, Glob, Grep
2. **Agent Tools**: AgentTool (subagent dispatch)
3. **MCP Tools**: MCPTool (external MCP servers)
4. **Web Tools**: WebSearch, WebFetch
5. **Task Tools**: TaskCreate, TaskGet, TaskList, TaskOutput
6. **Team Tools**: TeamCreate, TeamDelete
7. **Utility Tools**: TodoWrite, Sleep, AskUserQuestion
8. **Mode Tools**: EnterPlanMode, ExitPlanMode, EnterWorktree, ExitWorktree
9. **REPL Tools**: REPLTool
10. **Config Tools**: ConfigTool, BriefTool

---

### Tool Directory Structure

**Location**: `src/tools/` (184 items total)

| Tool | Files | Purpose |
|------|-------|---------|
| **BashTool** | 17 | Execute shell commands |
| **AgentTool** | 20 | Create and manage subagents |
| **FileReadTool** | 5 | Read file contents |
| **FileEditTool** | 6 | Edit files with string replacement |
| **FileWriteTool** | 3 | Write new files |
| **GlobTool** | 3 | Find files by pattern |
| **GrepTool** | 3 | Search file contents |
| **MCPTool** | 4 | MCP server tool calls |
| **WebSearchTool** | 3 | Web search via API |
| **WebFetchTool** | 5 | Fetch web content |
| **Task*Tools** | 12 | Task management (create, get, list, output, stop, update) |
| **Team*Tools** | 8 | Team/swarm management |
| **REPLTool** | 2 | Read-Eval-Print Loop |
| **LSPTool** | 6 | Language Server Protocol |
| **PowerShellTool** | 14 | Windows PowerShell execution |
| **NotebookEditTool** | 4 | Jupyter notebook editing |
| **TodoWriteTool** | 3 | Todo list management |
| **AskUserQuestionTool** | 2 | Interactive user prompts |
| **ScheduleCronTool** | 5 | Cron job scheduling |
| **SkillTool** | 4 | Skill/prompt workflows |
| **SendMessageTool** | 4 | Message sending |
| **RemoteTriggerTool** | 3 | Remote trigger execution |
| **ConfigTool** | 5 | Configuration management |
| **BriefTool** | 5 | Project brief management |
| **Enter/Exit PlanMode** | 8 | Plan mode entry/exit |
| **Enter/Exit Worktree** | 8 | Worktree management |
| **SyntheticOutputTool** | 1 | Synthetic output generation |
| **SleepTool** | 1 | Delay execution |
| **McpAuthTool** | 1 | MCP authentication |
| **ListMcpResourcesTool** | 3 | List MCP resources |
| **ReadMcpResourceTool** | 3 | Read MCP resources |
| **ToolSearchTool** | 3 | Search available tools |

---

### Key Tool Implementation: BashTool

**Location**: `src/tools/BashTool/` (17 files, ~500KB total)

**Key Files**:
- `BashTool.tsx` (160KB) - Main tool implementation
- `bashSecurity.ts` (102KB) - Security validation
- `bashPermissions.ts` (98KB) - Permission checking
- `pathValidation.ts` (43KB) - Path security validation
- `readOnlyValidation.ts` (68KB) - Read-only command detection
- `sedEditParser.ts` (9KB) - Sed command parsing
- `sedValidation.ts` (21KB) - Sed command validation

**Key Components**:
```typescript
export class BashTool implements Tool {
  name = 'bash'
  description = 'Execute bash commands'
  
  async call(input: BashInput, context: ToolContext): Promise<ToolResult>
  async checkPermissions(input: BashInput, context: ToolContext): Promise<PermissionResult>
  isConcurrencySafe(): boolean { return false }
  
  // Security features
  private validateCommand(command: string): ValidationResult
  private checkDestructive(command: string): boolean
  private shouldUseSandbox(command: string): boolean
}
```

---

### Key Tool Implementation: AgentTool

**Location**: `src/tools/AgentTool/` (20 files)

**Purpose**: Creates and dispatches to subagents

**Key Files**:
- `runAgent.ts` - Main subagent runner
- `AgentTool.ts` - Tool implementation
- `loadAgentsDir.ts` - Load agent definitions from directory
- `agentPrompt.tsx` - Agent prompt rendering

**Key Components**:
```typescript
export async function runAgent(
  definition: AgentDefinition,
  input: AgentInput,
  context: SubagentContext
): Promise<AgentResult>

export class AgentTool implements Tool {
  name = 'agent'
  description = 'Create a subagent with its own context and tools'
  
  async call(input: AgentInput, context: ToolContext): Promise<ToolResult> {
    const subagentContext = createSubagentContext(context, input)
    return runAgent(input.definition, input, subagentContext)
  }
}
```

---

## MCP Integration

### `src/services/mcp/` (23 items)

**Purpose**: Full Model Context Protocol client implementation

**Key Files**:
- `types.ts` - MCP type definitions
- `client.ts` - MCP client implementation
- `transport/` - Transport implementations (stdio, sse, http, ws)
- `auth/` - OAuth authentication
- `resources.ts` - Resource management
- `prompts.ts` - Prompt management

### MCP Types (`src/services/mcp/types.ts`)

**Config Scopes**:
```typescript
export type ConfigScope = 
  | 'local' 
  | 'user' 
  | 'project' 
  | 'dynamic' 
  | 'enterprise' 
  | 'claudeai' 
  | 'managed'
```

**Transport Types**:
```typescript
export type Transport = 'stdio' | 'sse' | 'sse-ide' | 'http' | 'ws' | 'sdk'
```

**Server Configs**:
- `McpStdioServerConfig` - Local command-based servers
- `McpSSEServerConfig` - Server-Sent Events servers
- `McpHTTPServerConfig` - HTTP-based servers
- `McpWebSocketServerConfig` - WebSocket servers

---

### MCPTool (`src/tools/MCPTool/`)

**Purpose**: Tool for calling MCP server tools

```typescript
export class MCPTool implements Tool {
  name = 'mcp'
  description = 'Call tools from MCP servers'
  
  async call(input: MCPInput, context: ToolContext): Promise<ToolResult> {
    const client = getMcpClient(input.serverName)
    const result = await client.callTool(input.toolName, input.arguments)
    return { output: result }
  }
}
```

---

## Subagent System

### Overview

The subagent system allows creating child agents with:
- Isolated contexts (separate message history)
- Own tool sets (can be subset of parent tools)
- Own MCP servers
- Different system prompts
- Nested subagents (agents creating agents)

### Key Files

**Location**: `src/tools/AgentTool/`

| File | Purpose |
|------|---------|
| `runAgent.ts` | Main subagent runner |
| `AgentTool.ts` | Tool definition |
| `createSubagentContext.ts` | Context creation |
| `loadAgentsDir.ts` | Load agent definitions |
| `agentPrompt.tsx` | Prompt template |
| `AgentToolResult.tsx` | Result display |
| `types.ts` | Type definitions |
| `agentToolParam.ts` | Parameter handling |
| `resolveAgent.ts` | Agent resolution |
| `parseAgentSyntax.ts` | Syntax parsing |

### Context Creation

```typescript
export function createSubagentContext(
  parentContext: ToolContext,
  input: AgentInput
): SubagentContext {
  return {
    ...parentContext,
    messages: [], // Fresh message history
    tools: input.tools || parentContext.tools,
    mcpClients: input.mcpClients || parentContext.mcpClients,
    systemPrompt: input.systemPrompt,
    depth: parentContext.depth + 1,
    // Shared state access for session-scoped infrastructure
    setAppStateForTasks: parentContext.setAppStateForTasks
  }
}
```

---

## State Management

### Overview

Claude Code uses a **custom Zustand-like store pattern** for state management, not Redux or standard Zustand.

### Key Files

**Location**: `src/state/` (6 files)

| File | Purpose | Size |
|------|---------|------|
| `AppState.tsx` | React provider component | 23KB |
| `AppStateStore.ts` | Store definition and types | 21KB |
| `store.ts` | Generic store factory | 0.8KB |
| `onChangeAppState.ts` | State change handler | 6KB |
| `selectors.ts` | Memoized selectors | 2KB |
| `teammateViewHelpers.ts` | Teammate view utilities | 4KB |

### Store Pattern (`src/state/store.ts`)

```typescript
type Listener = () => void
type OnChange<T> = (args: { newState: T; oldState: T }) => void

export type Store<T> = {
  getState: () => T
  setState: (updater: (prev: T) => T) => void
  subscribe: (listener: Listener) => () => void
}

export function createStore<T>(
  initialState: T,
  onChange?: OnChange<T>,
): Store<T>
```

### AppState Structure (`src/state/AppStateStore.ts`)

```typescript
export interface AppState {
  // Conversation state
  messages: Message[]
  pendingMessages: PendingMessage[]
  
  // Tool execution state
  toolUse: ToolUseState
  toolResults: ToolResult[]
  pendingPermissions: PendingPermission[]
  
  // Configuration
  verbose: boolean
  debug: boolean
  mainLoopModel: string
  
  // MCP state
  mcpServers: MCPServerConfig[]
  mcpClients: MCPServerConnection[]
  
  // Agent state
  agentMode: AgentMode
  agentDefinitions: AgentDefinitionsResult
  
  // Permission context
  toolPermissionContext: ToolPermissionContext
  
  // UI state
  theme: Theme
  sidebarVisible: boolean
  
  // Session infrastructure
  sessionHooks: SessionHooks
  speculation: SpeculationState
  
  // Background tasks
  backgroundTasks: BackgroundTask[]
}
```

### Usage Pattern

```typescript
// Get state
const state = getAppState()
const { verbose, messages } = state

// Set state (functional update)
setAppState(prev => ({
  ...prev,
  verbose: !prev.verbose
}))

// In React components
const verbose = useAppState(s => s.verbose)
const setVerbose = useSetAppState()
```

---

## Services

### Overview

Services provide domain-specific functionality across the application.

### Service Directory (`src/services/` - 130 items)

| Service | Files | Purpose |
|---------|-------|---------|
| **api** | 20 | LLM API client (Anthropic) |
| **mcp** | 23 | MCP client implementation |
| **compact** | 11 | Context window compaction |
| **analytics** | 9 | Telemetry and analytics |
| **lsp** | 7 | Language Server Protocol |
| **oauth** | 5 | OAuth authentication |
| **plugins** | 3 | Plugin system |
| **policyLimits** | 2 | Organization policy limits |
| **remoteManagedSettings** | 5 | Remote configuration |
| **PromptSuggestion** | 2 | AI prompt suggestions |
| **SessionMemory** | 3 | Session-based memory |
| **AgentSummary** | 1 | Agent conversation summaries |
| **MagicDocs** | 2 | Documentation system |
| **extractMemories** | 2 | Memory extraction |
| **teamMemorySync** | 5 | Team memory synchronization |
| **toolUseSummary** | 1 | Tool use summarization |
| **settingsSync** | 2 | Settings synchronization |
| **tips** | 3 | User tips |
| **voice** | 1 | Voice mode |
| **voiceStreamSTT** | 1 | Voice streaming STT |

### Key Service: API (`src/services/api/`)

**Purpose**: Anthropic API client with streaming support

**Key Files**:
- `anthropic.ts` - Main API client
- `streaming.ts` - Streaming response handling
- `types.ts` - API types
- `cache.ts` - Response caching
- `rateLimit.ts` - Rate limit handling

### Key Service: Compact (`src/services/compact/`)

**Purpose**: Context window management and message compaction

**Key Files**:
- `compact.ts` - Main compaction logic
- `strategies/` - Different compaction strategies
- `summarize.ts` - Summarization for compaction
- `types.ts` - Type definitions

**Compaction Triggers**:
- Token count approaching limit
- User manually requests compaction
- Model indicates context is too long

**Strategies**:
1. Summarize older messages
2. Remove system messages that are no longer relevant
3. Compact tool results to summaries

---

## UI/Components

### Overview

Claude Code uses **Ink** - a React renderer for terminals. Components are React components that render to the terminal.

### Component Directory (`src/components/` - 389 items)

**Major Component Categories**:

| Category | Count | Purpose |
|----------|-------|---------|
| **chat/** | ~50 | Chat UI components |
| **sidebar/** | ~30 | Sidebar components |
| **permissions/** | ~20 | Permission dialogs |
| **tool/** | ~40 | Tool output displays |
| **mcp/** | ~15 | MCP-related UI |
| **agent/** | ~25 | Agent/subagent UI |
| **input/** | ~20 | Input handling |
| **modals/** | ~15 | Modal dialogs |
| **toasts/** | ~10 | Toast notifications |
| **Spinner** | ~5 | Loading spinners |
| **Logo** | ~5 | Logo/branding |

### Key Components

#### `src/components/Spinner.tsx`

**Purpose**: Loading indicator component

```typescript
export type SpinnerMode = 'dots' | 'line' | 'arrow'

export function Spinner({ mode, text }: SpinnerProps): JSX.Element
```

#### Chat Components

- `ChatMessage.tsx` - Individual message display
- `ChatInput.tsx` - User input field
- `ChatHistory.tsx` - Message list
- `StreamingMessage.tsx` - Streaming text display

#### Tool Display Components

- `ToolUseMessage.tsx` - Tool call display
- `ToolResultMessage.tsx` - Tool result display
- `BashToolResultMessage.tsx` - Bash output display
- `FileEditResult.tsx` - File edit display

---

## Utilities

### Overview

Utilities provide cross-cutting concerns used throughout the application.

### Utils Directory (`src/utils/` - 564 items)

**Major Utility Categories**:

| Category | Files | Purpose |
|----------|-------|---------|
| **permissions/** | ~50 | Permission checking |
| **config/** | ~30 | Configuration management |
| **files/** | ~40 | File operations |
| **git/** | ~30 | Git operations |
| **markdown/** | ~20 | Markdown processing |
| **shell/** | ~25 | Shell command handling |
| **errors/** | ~15 | Error handling |
| **debug/** | ~10 | Debugging utilities |
| **types/** | ~15 | Type utilities |
| **testing/** | ~20 | Test utilities |

### Key Utilities

#### `src/utils/debug.ts`

**Purpose**: Debugging and logging utilities

```typescript
export function logForDebugging(message: string, meta?: object): void
export function logForDiagnosticsNoPII(level: string, message: string, meta?: object): void
```

#### `src/utils/errors.ts`

**Purpose**: Error handling utilities

```typescript
export function errorMessage(error: unknown): string
export function isFatalError(error: unknown): boolean
```

#### `src/utils/config.ts`

**Purpose**: Configuration system

```typescript
export function enableConfigs(): void
export function getConfig(key: string): unknown
export function setConfig(key: string, value: unknown): void
```

#### `src/utils/permissions/`

**Purpose**: Permission checking system

Key files:
- `permissionSetup.ts` - Permission context setup
- `checkPermissions.ts` - Generic permission checking
- `denialTracking.ts` - Track permission denials
- `files/` - File-specific permissions

---

## Key Relationships

### Dependency Graph (Simplified)

```
Entry Points
├── cli.tsx (fast paths)
├── main.tsx (full app)
│   ├── init.ts (initialization)
│   ├── AppStateProvider (state)
│   ├── MailboxProvider (messages)
│   └── query.ts (main loop)
│
Core Systems
├── query.ts
│   ├── QueryEngine.ts
│   ├── commands.ts
│   └── tools.ts
│
Tool System
├── tools.ts (registry)
├── tools/
│   ├── BashTool/
│   ├── AgentTool/
│   ├── File*Tool/
│   ├── MCPTool/
│   └── ...
│
MCP Integration
├── services/mcp/
│   ├── types.ts
│   ├── client.ts
│   └── transport/
│
State Management
├── state/
│   ├── store.ts (generic store)
│   ├── AppStateStore.ts (types)
│   └── AppState.tsx (provider)
│
Services
├── services/
│   ├── api/ (LLM API)
│   ├── compact/ (context compaction)
│   ├── mcp/ (MCP client)
│   └── ...
│
UI Components
├── components/
│   ├── Spinner.tsx
│   ├── chat/
│   ├── tool/
│   └── permissions/
│
Utilities
└── utils/
    ├── debug.ts
    ├── errors.ts
    ├── config.ts
    └── permissions/
```

---

## File Counts

### By Directory

| Directory | Files | Purpose |
|-----------|-------|---------|
| `src/` | 1902 | All source code |
| `src/utils/` | 564 | Utilities |
| `src/components/` | 389 | UI components |
| `src/commands/` | 207 | Slash commands |
| `src/tools/` | 184 | Tool implementations |
| `src/services/` | 130 | Services |
| `src/hooks/` | 104 | React hooks |
| `src/ink/` | 96 | Ink (React for CLI) |
| `src/entrypoints/` | 8 | Entry points |

### By Category

| Category | Files |
|----------|-------|
| Core/Entry | 5 |
| State Management | 6 |
| Tools | 184 |
| Commands | 207 |
| Services | 130 |
| Components | 389 |
| Hooks | 104 |
| Utils | 564 |
| Other | 313 |
| **Total** | **1902** |

---

## Architecture Decisions

### 1. Ink (React for CLI)

**Decision**: Use Ink for terminal UI instead of traditional CLI libraries.

**Rationale**:
- Familiar React patterns
- Component-based architecture
- Easy to test and compose
- JSX for declarative UI

### 2. Custom Store (not Redux/Zustand)

**Decision**: Use custom Zustand-like store pattern.

**Rationale**:
- Full control over implementation
- No external dependencies for core state
- Easy to customize for CLI-specific needs
- Smaller bundle size

### 3. Full MCP SDK Client

**Decision**: Implement full MCP client with all transports.

**Rationale**:
- First-class MCP support
- Multiple server types (stdio, sse, http)
- OAuth support for authenticated servers
- Enterprise-ready

### 4. Tool Partitioning

**Decision**: Partition tool calls into concurrent (read) and serial (write).

**Rationale**:
- Performance: read-only tools run in parallel (max 10)
- Safety: write tools run serially to avoid conflicts
- Pattern from Claude Code's isConcurrencySafe()

### 5. Subagent System

**Decision**: Support nested subagents with isolated contexts.

**Rationale**:
- Complex workflows need task decomposition
- Isolation prevents context pollution
- Shared state access for infrastructure
- Reusable agent definitions

### 6. Skills System

**Decision**: Prompt-based reusable workflows (skills) loaded from files.

**Rationale**:
- Easy to create new capabilities
- Version controlled
- No code changes needed for new skills
- User-customizable

---

## Conclusion

This document provides a comprehensive architectural overview of the Claude Code source codebase. The architecture follows several key patterns:

1. **React for CLI**: Ink provides a component-based UI framework
2. **Agent Loop**: query.ts drives the LLM interaction and tool execution
3. **Tool System**: Standardized tool interface with security and concurrency controls
4. **MCP First**: Full MCP client implementation for extensibility
5. **Subagents**: Hierarchical agent system for complex workflows
6. **Custom State**: Purpose-built store pattern for CLI needs

The codebase is organized into clear layers:
- **Entry Points** (cli.tsx, main.tsx, init.ts)
- **Core Systems** (query.ts, commands.ts, tools.ts)
- **Tools** (184 files in tools/)
- **Services** (130 files in services/)
- **UI** (389 files in components/)
- **State** (6 files in state/)
- **Utils** (564 files in utils/)

For detailed file-level documentation, refer to individual source files. This document serves as a high-level architectural map.

---

## Additional Documentation Files

Due to the size of this codebase, additional documentation files are organized by topic:

- `01-overview.md` - This file (main overview)
- `02-entry-points.md` - Detailed entry point documentation
- `03-core-systems.md` - query.ts, QueryEngine, commands
- `04-tool-system.md` - All tool implementations
- `05-mcp-integration.md` - MCP client details
- `06-subagent-system.md` - Subagent architecture
- `07-state-management.md` - State system deep dive
- `08-services.md` - Service layer documentation
- `09-ui-components.md` - Component architecture
- `10-utilities.md` - Utility functions
- `codebase_graph.json` - Complete dependency graph
