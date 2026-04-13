# Claude Code Tool System - Deep Dive

> **Part of**: Claude Code Architecture Documentation  
> **Focus**: Tool system implementation details  
> **Generated**: 2025

---

## Table of Contents

1. [Tool System Overview](#tool-system-overview)
2. [Tool Interface](#tool-interface)
3. [Tool Registry](#tool-registry)
4. [Tool Execution](#tool-execution)
5. [Permission System](#permission-system)
6. [Concurrency Model](#concurrency-model)
7. [Built-in Tools](#built-in-tools)
8. [MCP Tools](#mcp-tools)
9. [Tool UI Components](#tool-ui-components)

---

## Tool System Overview

Claude Code's tool system is the core mechanism that allows the LLM to interact with the system. It's designed with:

- **Security First**: Every tool has permission checking
- **Concurrency Control**: Read-only tools run in parallel, write tools run serially
- **Extensibility**: Easy to add new tools
- **MCP Integration**: External tools via Model Context Protocol

### Architecture Diagram

```
┌─────────────────┐
│   LLM Response  │
│  (tool_use)     │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  ToolRegistry   │◄──────┐
│  (find tool)    │       │
└────────┬────────┘       │
         │                │
         ▼                │
┌─────────────────┐      │
│Permission Check │      │
│ (should allow?) │      │
└────────┬────────┘      │
         │                │
    ┌────┴────┐          │
    │         │          │
    ▼         ▼          │
┌────────┐ ┌────────┐    │
│ ALLOW  │ │  DENY  │    │
└───┬────┘ └───┬────┘    │
    │          │         │
    ▼          ▼         │
┌─────────────────┐      │
│   Tool.call()   │      │
│   (execute)     │      │
└────────┬────────┘      │
         │                │
         ▼                │
┌─────────────────┐       │
│  ToolResult     │───────┘
│  (return to LLM)│
└─────────────────┘
```

---

## Tool Interface

### Core Type Definition

**File**: `src/Tool.ts`

```typescript
export interface Tool<TInput = unknown, TOutput = unknown> {
  // Identification
  name: string
  description: string
  
  // Input validation
  inputSchema: ToolInputJSONSchema
  
  // Execution
  call(input: TInput, context: ToolContext): Promise<ToolResult<TOutput>>
  
  // Permission checking
  checkPermissions(
    input: TInput, 
    context: ToolContext
  ): Promise<PermissionResult>
  
  // Concurrency control
  isConcurrencySafe(): boolean
  
  // UI rendering
  render?(
    input: TInput, 
    result: TOutput
  ): React.ReactElement | null
}
```

### Tool Context

```typescript
export interface ToolContext {
  // Core app state access
  getAppState(): AppState
  setAppState(updater: (prev: AppState) => AppState): void
  
  // Session-scoped infrastructure
  setAppStateForTasks?: (updater: (prev: AppState) => AppState) => void
  
  // Message history
  messages: Message[]
  
  // Available tools
  tools: Tool[]
  
  // MCP clients
  mcpClients: MCPServerConnection[]
  
  // Abort signal
  abortController: AbortController
  
  // File state cache
  readFileState: FileStateCache
  
  // Query source for analytics
  querySource?: QuerySource
}
```

### Tool Result

```typescript
export interface ToolResult<TOutput = unknown> {
  // Output data
  output: TOutput
  
  // Error flag
  isError?: boolean
  
  // Error message (if isError)
  errorMessage?: string
  
  // Tool-specific data
  data?: Record<string, unknown>
}
```

---

## Tool Registry

**File**: `src/tools.ts`

### Registry Implementation

```typescript
// Global tool registry
const TOOLS: Map<string, Tool> = new Map()

export function registerTool(tool: Tool): void {
  TOOLS.set(tool.name, tool)
}

export function findToolByName(name: string): Tool | undefined {
  return TOOLS.get(name)
}

export function getAllTools(): Tool[] {
  return Array.from(TOOLS.values())
}
```

### Tool Registration

Tools are registered at module load time:

```typescript
// In tools.ts or tool modules
registerTool(new BashTool())
registerTool(new FileReadTool())
registerTool(new FileEditTool())
registerTool(new AgentTool())
registerTool(new MCPTool())
// ... etc
```

---

## Tool Execution

### Tool Orchestration

**File**: `src/utils/toolOrchestration.ts`

```typescript
export async function runTools(
  toolCalls: ToolCall[],
  context: ToolContext
): Promise<ToolResult[]> {
  // 1. Partition calls by concurrency safety
  const { concurrentCalls, serialCalls } = partitionToolCalls(
    toolCalls, 
    context
  )
  
  // 2. Execute concurrent calls in parallel
  const concurrentResults = await Promise.all(
    concurrentCalls.map(call => executeToolCall(call, context))
  )
  
  // 3. Execute serial calls sequentially
  const serialResults: ToolResult[] = []
  for (const call of serialCalls) {
    const result = await executeToolCall(call, context)
    serialResults.push(result)
  }
  
  // 4. Combine and return results
  return [...concurrentResults, ...serialResults]
}
```

### Partitioning Logic

```typescript
function partitionToolCalls(
  toolCalls: ToolCall[],
  context: ToolContext
): { concurrentCalls: ToolCall[]; serialCalls: ToolCall[] } {
  const concurrentCalls: ToolCall[] = []
  const serialCalls: ToolCall[] = []
  
  for (const call of toolCalls) {
    const tool = findToolByName(call.name)
    if (!tool) {
      serialCalls.push(call) // Unknown tools default to serial
      continue
    }
    
    if (tool.isConcurrencySafe()) {
      concurrentCalls.push(call)
    } else {
      serialCalls.push(call)
    }
  }
  
  return { concurrentCalls, serialCalls }
}
```

### Streaming Tool Executor

```typescript
export async function* StreamingToolExecutor(
  toolCalls: ToolCall[],
  context: ToolContext
): AsyncGenerator<ToolProgressEvent, ToolResult[], unknown> {
  for (const call of toolCalls) {
    const tool = findToolByName(call.name)
    if (!tool) {
      yield { type: 'error', callId: call.id, error: 'Unknown tool' }
      continue
    }
    
    // Yield start event
    yield { type: 'tool_start', callId: call.id, toolName: tool.name }
    
    try {
      // Execute tool
      const result = await tool.call(call.input, context)
      
      // Yield end event
      yield { type: 'tool_end', callId: call.id, result }
    } catch (error) {
      yield { type: 'error', callId: call.id, error: String(error) }
    }
  }
}
```

---

## Permission System

### Permission Context

**File**: `src/types/permissions.ts`

```typescript
export interface ToolPermissionContext {
  // Current permission mode
  mode: PermissionMode
  
  // Additional working directories
  additionalWorkingDirectories: Map<string, AdditionalWorkingDirectory>
  
  // Rule sets
  alwaysAllowRules: ToolPermissionRulesBySource
  alwaysDenyRules: ToolPermissionRulesBySource
  alwaysAskRules: ToolPermissionRulesBySource
  
  // Feature flags
  isBypassPermissionsModeAvailable: boolean
  isAutoModeAvailable?: boolean
  
  // Control flags
  shouldAvoidPermissionPrompts?: boolean
  awaitAutomatedChecksBeforeDialog?: boolean
}
```

### Permission Modes

```typescript
export type PermissionMode = 
  | 'default'        // Normal permission checking
  | 'bypass'         // Skip all permission checks
  | 'auto'           // Auto-approve based on rules
  | 'plan'           // Plan mode (special handling)
```

### Permission Checking Flow

```typescript
export async function checkToolPermission(
  tool: Tool,
  input: unknown,
  context: ToolContext
): Promise<PermissionResult> {
  const { toolPermissionContext: permContext } = context.getAppState()
  
  // 1. Check mode
  if (permContext.mode === 'bypass') {
    return { type: 'allow', reason: 'bypass_mode' }
  }
  
  // 2. Check always-deny rules
  if (matchesRule(tool, input, permContext.alwaysDenyRules)) {
    return { type: 'deny', reason: 'rule_denied' }
  }
  
  // 3. Check always-allow rules
  if (matchesRule(tool, input, permContext.alwaysAllowRules)) {
    return { type: 'allow', reason: 'rule_allowed' }
  }
  
  // 4. Check always-ask rules
  if (matchesRule(tool, input, permContext.alwaysAskRules)) {
    return { type: 'ask', reason: 'rule_ask' }
  }
  
  // 5. Delegate to tool-specific permission check
  return tool.checkPermissions(input, context)
}
```

---

## Concurrency Model

### Read vs Write Tools

| Type | Examples | Concurrency | Max Parallel |
|------|----------|-------------|--------------|
| **Read** | read, glob, grep, web_search | Concurrent | 10 |
| **Write** | write, edit, bash (write) | Serial | 1 |

### Concurrency Safety Determination

```typescript
// Tool declares if it's concurrency safe
class FileReadTool implements Tool {
  isConcurrencySafe(): boolean {
    return true // Reading files is safe to parallelize
  }
}

class FileWriteTool implements Tool {
  isConcurrencySafe(): boolean {
    return false // Writing files must be serial
  }
}

class BashTool implements Tool {
  isConcurrencySafe(): boolean {
    return false // Bash commands are never concurrent safe
  }
}
```

### Execution Order

```
Concurrent Batch (max 10 parallel):
├── read_file("src/utils.ts")
├── read_file("src/main.ts")
├── grep("function", "src/")
├── glob("**/*.ts")
└── web_search("typescript tips")

Serial Queue (executed one by one):
├── write_file("src/new.ts", content)
├── edit_file("src/existing.ts", old, new)
├── bash("npm install")
└── write_file("package.json", updated)
```

---

## Built-in Tools

### File Tools

#### FileReadTool

```typescript
class FileReadTool implements Tool<FileReadInput, FileReadOutput> {
  name = 'read'
  description = 'Read file contents with optional offset and limit'
  
  inputSchema = {
    type: 'object',
    properties: {
      file_path: { type: 'string' },
      offset: { type: 'number' },
      limit: { type: 'number' }
    },
    required: ['file_path']
  }
  
  isConcurrencySafe(): boolean { return true }
  
  async call(input: FileReadInput, context: ToolContext): Promise<ToolResult> {
    const content = await fs.readFile(input.file_path, 'utf-8')
    const lines = content.split('\n')
    const slice = lines.slice(input.offset, input.offset + input.limit)
    
    return {
      output: {
        content: slice.join('\n'),
        total_lines: lines.length
      }
    }
  }
  
  async checkPermissions(
    input: FileReadInput, 
    context: ToolContext
  ): Promise<PermissionResult> {
    // Check if path is within allowed directories
    const isAllowed = await isPathAllowed(input.file_path, context)
    return isAllowed 
      ? { type: 'allow' }
      : { type: 'ask', prompt: `Read ${input.file_path}?` }
  }
}
```

#### FileWriteTool

```typescript
class FileWriteTool implements Tool<FileWriteInput, FileWriteOutput> {
  name = 'write'
  description = 'Write content to a file'
  
  isConcurrencySafe(): boolean { return false }
  
  async call(input: FileWriteInput): Promise<ToolResult> {
    await fs.writeFile(input.file_path, input.content)
    return { output: { success: true } }
  }
}
```

#### FileEditTool

```typescript
class FileEditTool implements Tool<FileEditInput, FileEditOutput> {
  name = 'edit'
  description = 'Edit a file by replacing text'
  
  isConcurrencySafe(): boolean { return false }
  
  async call(input: FileEditInput): Promise<ToolResult> {
    const content = await fs.readFile(input.file_path, 'utf-8')
    
    if (!content.includes(input.old_string)) {
      return {
        isError: true,
        errorMessage: 'Old string not found in file'
      }
    }
    
    const newContent = content.replace(input.old_string, input.new_string)
    await fs.writeFile(input.file_path, newContent)
    
    return { output: { success: true } }
  }
}
```

### Search Tools

#### GlobTool

```typescript
class GlobTool implements Tool<GlobInput, GlobOutput> {
  name = 'glob'
  description = 'Find files matching a pattern'
  
  isConcurrencySafe(): boolean { return true }
  
  async call(input: GlobInput): Promise<ToolResult> {
    const files = await glob(input.pattern, {
      cwd: input.cwd,
      ignore: input.ignore
    })
    return { output: { files } }
  }
}
```

#### GrepTool

```typescript
class GrepTool implements Tool<GrepInput, GrepOutput> {
  name = 'grep'
  description = 'Search for text patterns in files'
  
  isConcurrencySafe(): boolean { return true }
  
  async call(input: GrepInput): Promise<ToolResult> {
    const results = await grep(input.pattern, input.path, {
      caseSensitive: input.case_sensitive
    })
    return { output: { results } }
  }
}
```

### Bash Tool

**File**: `src/tools/BashTool/BashTool.tsx` (160KB)

The Bash tool is the most complex built-in tool with extensive security features:

```typescript
class BashTool implements Tool<BashInput, BashOutput> {
  name = 'bash'
  description = 'Execute bash commands'
  
  isConcurrencySafe(): boolean { return false }
  
  async call(input: BashInput, context: ToolContext): Promise<ToolResult> {
    // 1. Validate command
    const validation = validateCommand(input.command)
    if (!validation.valid) {
      return { isError: true, errorMessage: validation.error }
    }
    
    // 2. Check if should use sandbox
    const useSandbox = shouldUseSandbox(input.command)
    
    // 3. Execute (sandboxed or direct)
    if (useSandbox) {
      return executeInSandbox(input.command)
    } else {
      return executeDirectly(input.command)
    }
  }
  
  async checkPermissions(
    input: BashInput, 
    context: ToolContext
  ): Promise<PermissionResult> {
    // Extensive permission checking
    const security = analyzeCommandSecurity(input.command)
    
    if (security.destructive) {
      return { 
        type: 'ask', 
        prompt: `Allow destructive command: ${input.command}?` 
      }
    }
    
    if (security.readOnly) {
      return { type: 'allow', reason: 'read_only' }
    }
    
    return { 
      type: 'ask', 
      prompt: `Allow command: ${input.command}?` 
    }
  }
}
```

### Agent Tool (Subagent)

**File**: `src/tools/AgentTool/runAgent.ts`

```typescript
class AgentTool implements Tool<AgentInput, AgentOutput> {
  name = 'agent'
  description = 'Create a subagent with isolated context'
  
  async call(input: AgentInput, context: ToolContext): Promise<ToolResult> {
    // Create isolated context
    const subagentContext = createSubagentContext(context, {
      messages: [], // Fresh history
      tools: input.tools || context.tools,
      systemPrompt: input.system_prompt
    })
    
    // Run subagent
    const result = await runAgent({
      definition: input.definition,
      prompt: input.prompt,
      context: subagentContext
    })
    
    return { output: result }
  }
}

async function runAgent(params: RunAgentParams): Promise<AgentResult> {
  // Initialize subagent
  const messages: Message[] = [
    { role: 'system', content: params.context.systemPrompt },
    { role: 'user', content: params.prompt }
  ]
  
  // Run query loop
  const generator = query(messages, {
    tools: params.context.tools,
    mcpClients: params.context.mcpClients,
    // ... other options
  })
  
  // Collect results
  const results: QueryResult[] = []
  for await (const result of generator) {
    results.push(result)
  }
  
  return {
    output: results,
    finalMessage: results[results.length - 1]
  }
}
```

---

## MCP Tools

### MCPTool Implementation

**File**: `src/tools/MCPTool/MCPTool.tsx`

```typescript
class MCPTool implements Tool<MCPToolInput, MCPToolOutput> {
  name = 'mcp'
  description = 'Call tools from MCP servers'
  
  async call(input: MCPToolInput, context: ToolContext): Promise<ToolResult> {
    // Get MCP client
    const client = context.mcpClients.find(
      c => c.serverName === input.server_name
    )
    
    if (!client) {
      return {
        isError: true,
        errorMessage: `MCP server '${input.server_name}' not found`
      }
    }
    
    // Call tool via MCP
    const result = await client.callTool({
      name: input.tool_name,
      arguments: input.arguments
    })
    
    return { output: result }
  }
  
  async checkPermissions(): Promise<PermissionResult> {
    // MCP tools generally require explicit permission
    return { type: 'ask' }
  }
}
```

### MCP Client

**File**: `src/services/mcp/client.ts`

```typescript
export class MCPClient {
  private client: Client
  private transport: Transport
  
  constructor(config: MCPClientConfig) {
    this.transport = createTransport(config.transport)
    this.client = new Client({
      name: 'claude-code',
      version: getVersion()
    })
  }
  
  async connect(): Promise<void> {
    await this.client.connect(this.transport)
  }
  
  async listTools(): Promise<Tool[]> {
    const response = await this.client.listTools()
    return response.tools
  }
  
  async callTool(request: CallToolRequest): Promise<CallToolResult> {
    return this.client.callTool(request)
  }
  
  async listResources(): Promise<Resource[]> {
    return this.client.listResources()
  }
  
  async readResource(uri: string): Promise<ReadResourceResult> {
    return this.client.readResource({ uri })
  }
}
```

---

## Tool UI Components

### Tool Result Rendering

Tools can provide custom UI components for their results:

```typescript
class BashTool implements Tool<BashInput, BashOutput> {
  // ... implementation
  
  render(input: BashInput, result: BashOutput): React.ReactElement {
    return (
      <BashResultMessage
        command={input.command}
        stdout={result.stdout}
        stderr={result.stderr}
        exitCode={result.exit_code}
      />
    )
  }
}
```

### Bash Result Component

**File**: `src/tools/BashTool/BashToolResultMessage.tsx`

```typescript
export function BashToolResultMessage({
  command,
  stdout,
  stderr,
  exitCode
}: BashResultProps): React.ReactElement {
  return (
    <Box flexDirection="column">
      <Text color="gray">$ {command}</Text>
      
      {stdout && (
        <Box marginTop={1}>
          <Text>{stdout}</Text>
        </Box>
      )}
      
      {stderr && (
        <Box marginTop={1}>
          <Text color="red">{stderr}</Text>
        </Box>
      )}
      
      <Text 
        color={exitCode === 0 ? 'green' : 'red'}
        dimColor
      >
        exit code: {exitCode}
      </Text>
    </Box>
  )
}
```

---

## Tool Development Guide

### Creating a New Tool

```typescript
// 1. Define input/output types
interface MyToolInput {
  param: string
}

interface MyToolOutput {
  result: string
}

// 2. Implement tool class
class MyTool implements Tool<MyToolInput, MyToolOutput> {
  name = 'my_tool'
  description = 'Description for LLM'
  
  inputSchema = {
    type: 'object',
    properties: {
      param: { type: 'string', description: 'Parameter description' }
    },
    required: ['param']
  }
  
  isConcurrencySafe(): boolean {
    return true // or false
  }
  
  async call(
    input: MyToolInput, 
    context: ToolContext
  ): Promise<ToolResult<MyToolOutput>> {
    // Implementation
    const result = await doSomething(input.param)
    
    return {
      output: { result }
    }
  }
  
  async checkPermissions(
    input: MyToolInput,
    context: ToolContext
  ): Promise<PermissionResult> {
    // Permission logic
    return { type: 'allow' }
  }
}

// 3. Register tool
registerTool(new MyTool())
```

### Tool Testing

```typescript
import { test, expect } from 'bun:test'
import { MyTool } from './MyTool'
import { createMockContext } from '../../utils/testing'

test('MyTool.call', async () => {
  const tool = new MyTool()
  const context = createMockContext()
  
  const result = await tool.call({ param: 'test' }, context)
  
  expect(result.isError).toBeFalsy()
  expect(result.output.result).toBe('expected')
})

test('MyTool.checkPermissions', async () => {
  const tool = new MyTool()
  const context = createMockContext()
  
  const result = await tool.checkPermissions({ param: 'test' }, context)
  
  expect(result.type).toBe('allow')
})
```

---

## Summary

The Claude Code tool system provides:

1. **Security**: Permission checking at multiple levels
2. **Performance**: Concurrency control for read/write operations
3. **Extensibility**: Easy to add new tools
4. **MCP Integration**: External tools via standard protocol
5. **UI Integration**: Custom rendering for tool results

Key files:
- `src/Tool.ts` - Type definitions
- `src/tools.ts` - Registry
- `src/utils/toolOrchestration.ts` - Execution
- `src/tools/BashTool/` - Complex tool example
- `src/tools/AgentTool/` - Subagent tool
- `src/tools/MCPTool/` - MCP integration
