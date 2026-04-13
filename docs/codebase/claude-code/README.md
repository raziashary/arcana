# Claude Code Source - Architecture Documentation

> **Project**: `claude-code-source-main`  
> **Type**: Node.js/TypeScript CLI Application (Ink/React)  
> **Total Files**: 1,902  
> **Generated**: 2025

---

## Documentation Index

This comprehensive documentation is organized into multiple files for easier navigation:

| Document | Purpose | Size |
|----------|---------|------|
| **[README.md](README.md)** | This file - index and navigation | - |
| **[claude-code-codebase-dissected.md](claude-code-codebase-dissected.md)** | Complete architectural overview | ~15KB |
| **[codebase_graph.json](codebase_graph.json)** | Machine-readable dependency graph | ~25KB |
| **[01-tool-system-deep-dive.md](01-tool-system-deep-dive.md)** | Tool system implementation details | ~15KB |
| **[02-mcp-integration.md](02-mcp-integration.md)** | Model Context Protocol integration | ~15KB |
| **[03-agent-loop-and-state.md](03-agent-loop-and-state.md)** | Agent loop and state management | ~15KB |

---

## Quick Start

### For High-Level Overview

Start with **[claude-code-codebase-dissected.md](claude-code-codebase-dissected.md)** for the complete architectural overview.

### For Specific Topics

| If you want to learn about... | Read this document |
|------------------------------|-------------------|
| Tool system implementation | [01-tool-system-deep-dive.md](01-tool-system-deep-dive.md) |
| MCP client and external tools | [02-mcp-integration.md](02-mcp-integration.md) |
| Agent loop and state management | [03-agent-loop-and-state.md](03-agent-loop-and-state.md) |
| Dependencies and file relationships | [codebase_graph.json](codebase_graph.json) |

---

## Project Overview

### What is Claude Code?

Claude Code is a **Node.js/TypeScript CLI application** that provides an agentic coding assistant experience in the terminal. It's built with:

- **Ink**: React for CLI (terminal UI)
- **Anthropic SDK**: Direct LLM API integration
- **MCP**: Model Context Protocol for external tools
- **Custom Store**: Zustand-like state management

### Architecture Highlights

```
┌─────────────────────────────────────────────────────────────┐
│                      Claude Code                             │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │   Entry      │  │   Agent      │  │    MCP       │     │
│  │   Points     │  │   Loop       │  │   Client     │     │
│  │  (cli.tsx)   │  │ (query.ts)   │  │(services/mcp)│    │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘     │
│         │                 │                  │              │
│         └─────────────────┼──────────────────┘              │
│                           │                                │
│                           ▼                                │
│  ┌─────────────────────────────────────────────────────┐  │
│  │                   Tool System                        │  │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐     │  │
│  │  │  Bash   │ │  File   │ │  Agent  │ │  MCP    │ ... │  │
│  │  │  Tool   │ │  Tools  │ │  Tool   │ │  Tool   │     │  │
│  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘     │  │
│  └─────────────────────────────────────────────────────┘  │
│                           │                                │
│                           ▼                                │
│  ┌─────────────────────────────────────────────────────┐  │
│  │              State Management                        │  │
│  │         (Custom Store Pattern)                      │  │
│  └─────────────────────────────────────────────────────┘  │
│                           │                                │
│                           ▼                                │
│  ┌─────────────────────────────────────────────────────┐  │
│  │                   UI Layer                           │  │
│  │              (Ink/React Components)                  │  │
│  └─────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## Directory Structure

```
claude-code-source-main/
├── src/
│   ├── entrypoints/          # CLI entry points (5 files)
│   │   ├── cli.tsx           # Main CLI bootstrap
│   │   ├── init.ts           # Application initialization
│   │   ├── mcp.ts            # MCP entry points
│   │   └── ...
│   │
│   ├── core/                 # Core systems (7 files, ~200KB)
│   │   ├── main.tsx          # Main entry (803KB)
│   │   ├── query.ts          # Agent loop (68KB)
│   │   ├── QueryEngine.ts    # LLM abstraction (47KB)
│   │   ├── tools.ts          # Tool registry (17KB)
│   │   ├── Tool.ts           # Tool types (29KB)
│   │   └── commands.ts       # Slash commands (25KB)
│   │
│   ├── tools/                # Tool implementations (184 directories)
│   │   ├── BashTool/         # Bash execution (17 files, ~500KB)
│   │   ├── AgentTool/        # Subagent system (20 files)
│   │   ├── FileReadTool/     # File reading (5 files)
│   │   ├── FileEditTool/     # File editing (6 files)
│   │   ├── MCPTool/          # MCP integration (4 files)
│   │   ├── WebSearchTool/    # Web search (3 files)
│   │   └── ...               # 40+ other tools
│   │
│   ├── state/                # State management (6 files)
│   │   ├── store.ts          # Store factory
│   │   ├── AppStateStore.ts  # State types (21KB)
│   │   ├── AppState.tsx      # React provider (23KB)
│   │   └── ...
│   │
│   ├── services/             # Services layer (130 directories)
│   │   ├── api/              # Anthropic API client (20 files)
│   │   ├── mcp/              # MCP client (23 files)
│   │   ├── compact/          # Context compaction (11 files)
│   │   ├── lsp/              # Language Server Protocol (7 files)
│   │   └── ...
│   │
│   ├── components/           # UI components (389 files)
│   │   ├── Spinner.tsx       # Loading indicator
│   │   ├── chat/             # Chat UI (~50 files)
│   │   ├── tool/             # Tool displays (~40 files)
│   │   └── ...
│   │
│   ├── commands/             # Slash commands (207 files)
│   │   └── ...
│   │
│   ├── hooks/                # React hooks (104 files)
│   │   ├── useAppState.ts
│   │   ├── useCanUseTool.ts
│   │   └── ...
│   │
│   ├── utils/                # Utilities (564 files)
│   │   ├── permissions/      # Permission checking (~50 files)
│   │   ├── config.ts         # Configuration
│   │   ├── debug.ts          # Debugging
│   │   └── ...
│   │
│   ├── types/                # Type definitions (11 files)
│   ├── constants/            # Constants (21 files)
│   └── ink/                  # Ink renderer (96 files)
│
└── ...
```

---

## File Statistics

### By Category

| Category | Count | Purpose |
|----------|-------|---------|
| **Entry Points** | 5 | Application bootstrap |
| **Core Systems** | 7 | Agent loop, tools, commands |
| **Tools** | 184 | Tool implementations |
| **Commands** | 207 | Slash command handlers |
| **Services** | 130 | API clients, MCP, LSP |
| **Components** | 389 | UI components |
| **State** | 6 | State management |
| **Hooks** | 104 | React hooks |
| **Utils** | 564 | Utility functions |
| **Types** | 11 | Type definitions |
| **Constants** | 21 | Application constants |
| **Ink** | 96 | Ink renderer |
| **Tests** | 52 | Test files |
| **Other** | 683 | Various |
| **Total** | **1,902** | All source files |

### Largest Files

| File | Size | Purpose |
|------|------|---------|
| `src/main.tsx` | 785 KB | Main application entry |
| `src/tools/BashTool/BashTool.tsx` | 157 KB | Bash tool implementation |
| `src/tools/BashTool/bashSecurity.ts` | 100 KB | Bash security |
| `src/tools/BashTool/bashPermissions.ts` | 96 KB | Bash permissions |
| `src/tools/BashTool/readOnlyValidation.ts` | 67 KB | Read-only detection |
| `src/query.ts` | 67 KB | Agent loop |
| `src/QueryEngine.ts` | 46 KB | LLM abstraction |
| `src/Tool.ts` | 29 KB | Tool type definitions |
| `src/state/AppState.tsx` | 23 KB | State provider |
| `src/state/AppStateStore.ts` | 21 KB | State types |

---

## Key Architectural Patterns

### 1. Tool System Pattern

```typescript
// Tools implement standardized interface
interface Tool {
  name: string
  description: string
  inputSchema: JSONSchema
  call(input: unknown, context: ToolContext): Promise<ToolResult>
  checkPermissions(input: unknown, context: ToolContext): Promise<PermissionResult>
  isConcurrencySafe(): boolean
}

// Concurrency control
// - Read-only tools: Run concurrently (max 10)
// - Write tools: Run serially
```

**Documentation**: [01-tool-system-deep-dive.md](01-tool-system-deep-dive.md)

### 2. Agent Loop Pattern

```typescript
// Generator-based async loop
async function* query(messages: Message[], options: QueryOptions) {
  // 1. Send to LLM
  // 2. Stream response
  // 3. Detect tool_use blocks
  // 4. Execute tools (concurrent/serial)
  // 5. Yield results
  // 6. Continue or complete
}
```

**Documentation**: [03-agent-loop-and-state.md](03-agent-loop-and-state.md)

### 3. MCP Integration Pattern

```typescript
// Full MCP client with multiple transports
- stdio: Local command-based servers
- sse: Server-Sent Events
- http: Direct HTTP
- ws: WebSocket
- sdk: In-process

// Dynamic tool discovery
const tools = await mcpClient.listTools()
```

**Documentation**: [02-mcp-integration.md](02-mcp-integration.md)

### 4. State Management Pattern

```typescript
// Custom store (Zustand-like)
const store = createStore(initialState, onChange)

// Functional updates
setAppState(prev => ({ ...prev, value: newValue }))

// Selector-based subscriptions
const value = useAppState(s => s.value)
```

**Documentation**: [03-agent-loop-and-state.md](03-agent-loop-and-state.md)

---

## External Dependencies

### Core Dependencies

| Package | Purpose |
|---------|---------|
| `@anthropic-ai/sdk` | Anthropic API client |
| `@modelcontextprotocol/sdk` | MCP protocol implementation |
| `react` | UI framework |
| `ink` | React for CLI |

### Build & Development

| Package | Purpose |
|---------|---------|
| `bun` | Runtime and bundler |
| `typescript` | Type checking |

### Utilities

| Package | Purpose |
|---------|---------|
| `lodash-es` | Utility functions |
| `zod` | Schema validation |
| `chalk` | Terminal colors |
| `glob` | File globbing |

---

## Reading Order

### For New Contributors

1. Start with this README
2. Read [claude-code-codebase-dissected.md](claude-code-codebase-dissected.md) for overview
3. Dive into specific topics:
   - Tool system → [01-tool-system-deep-dive.md](01-tool-system-deep-dive.md)
   - MCP integration → [02-mcp-integration.md](02-mcp-integration.md)
   - State management → [03-agent-loop-and-state.md](03-agent-loop-and-state.md)
4. Reference [codebase_graph.json](codebase_graph.json) for dependencies

### For Quick Reference

| Topic | Document | Section |
|-------|----------|---------|
| Entry points | Main docs | Entry Points |
| Tool implementation | [01-tool-system-deep-dive.md](01-tool-system-deep-dive.md) | Tool Interface |
| MCP client | [02-mcp-integration.md](02-mcp-integration.md) | MCP Client |
| State hooks | [03-agent-loop-and-state.md](03-agent-loop-and-state.md) | State Management |
| Agent loop | [03-agent-loop-and-state.md](03-agent-loop-and-state.md) | Agent Loop |

---

## Key Files Cheat Sheet

| Purpose | File |
|---------|------|
| CLI entry | `src/entrypoints/cli.tsx` |
| App init | `src/entrypoints/init.ts` |
| Main entry | `src/main.tsx` |
| Agent loop | `src/query.ts` |
| LLM client | `src/QueryEngine.ts` |
| Tool registry | `src/tools.ts` |
| Tool types | `src/Tool.ts` |
| Bash tool | `src/tools/BashTool/BashTool.tsx` |
| Agent tool | `src/tools/AgentTool/runAgent.ts` |
| MCP types | `src/services/mcp/types.ts` |
| MCP client | `src/services/mcp/client.ts` |
| State factory | `src/state/store.ts` |
| State types | `src/state/AppStateStore.ts` |
| State provider | `src/state/AppState.tsx` |
| Commands | `src/commands.ts` |

---

## Contributing to Documentation

When adding to this documentation:

1. Update the relevant focused document (01-, 02-, 03-)
2. Update this README index if adding new documents
3. Update the main [claude-code-codebase-dissected.md](claude-code-codebase-dissected.md) for high-level changes
4. Update [codebase_graph.json](codebase_graph.json) for dependency changes

---

## License & Attribution

This documentation was generated for the `claude-code-source-main` repository. All code references belong to their respective owners.

---

## Summary

This documentation provides:

- **Main Overview**: [claude-code-codebase-dissected.md](claude-code-codebase-dissected.md)
- **Dependency Graph**: [codebase_graph.json](codebase_graph.json) (machine-readable)
- **Tool System**: [01-tool-system-deep-dive.md](01-tool-system-deep-dive.md)
- **MCP Integration**: [02-mcp-integration.md](02-mcp-integration.md)
- **Agent Loop & State**: [03-agent-loop-and-state.md](03-agent-loop-and-state.md)

Total documentation size: ~85KB across 5 files covering 1,902 source files.
