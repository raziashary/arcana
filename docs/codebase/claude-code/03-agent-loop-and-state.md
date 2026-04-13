# Claude Code Agent Loop & State Management

> **Part of**: Claude Code Architecture Documentation  
> **Focus**: Core agent loop and state management  
> **Generated**: 2025

---

## Table of Contents

1. [Agent Loop Overview](#agent-loop-overview)
2. [Query Function](#query-function)
3. [Query Engine](#query-engine)
4. [State Management](#state-management)
5. [Message Flow](#message-flow)
6. [Context Management](#context-management)
7. [Speculation System](#speculation-system)

---

## Agent Loop Overview

The agent loop is the heart of Claude Code - it drives the conversation with the LLM, processes responses, executes tools, and manages state.

### Loop Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                        Agent Loop                             │
│                                                               │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐       │
│  │   Start     │───►│   query()   │───►│   Stream    │       │
│  │  (user msg) │    │  generator  │    │   LLM Resp  │       │
│  └─────────────┘    └─────────────┘    └──────┬──────┘       │
│                                               │              │
│                         ┌─────────────────────┘              │
│                         │                                     │
│                         ▼                                     │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐       │
│  │   Return    │◄───│   Process   │◄───│   Receive   │       │
│  │   Results   │    │   Results   │    │   Content   │       │
│  └─────────────┘    └──────┬──────┘    └─────────────┘       │
│                            │                                 │
│              ┌─────────────┴─────────────┐                  │
│              │                           │                   │
│              ▼                           ▼                   │
│  ┌─────────────────┐         ┌─────────────────┐            │
│  │   Text Content  │         │   Tool Calls    │            │
│  │   (yield text)  │         │   (execute)     │            │
│  └─────────────────┘         └────────┬────────┘            │
│                                       │                      │
│                                       ▼                      │
│                         ┌─────────────────────────┐            │
│                         │   Partition & Execute   │            │
│                         │   - Concurrent (read)   │            │
│                         │   - Serial (write)      │            │
│                         └────────────┬────────────┘            │
│                                      │                       │
│                                      ▼                       │
│                         ┌─────────────────────────┐            │
│                         │   Tool Results           │            │
│                         │   (add to messages)      │            │
│                         └────────────┬────────────┘            │
│                                      │                       │
│                                      └──────────────────┐   │
│                                                         │   │
│                              ┌──────────────────────────┘   │
│                              │                            │
│                              ▼                            │
│                   ┌─────────────────────┐                  │
│                   │   Continue Loop?    │                  │
│                   │   (more tool calls) │                  │
│                   └──────────┬──────────┘                  │
│                              │                            │
│                   ┌──────────┴──────────┐                  │
│                   │                     │                  │
│                   ▼                     ▼                  │
│         ┌───────────────┐    ┌───────────────┐           │
│         │     Yes       │    │      No       │           │
│         │   (continue)  │    │    (done)     │           │
│         └───────┬───────┘    └───────┬───────┘           │
│                 │                   │                      │
│                 └───────────────────┘                      │
│                          │                                 │
│                          ▼                                 │
│                ┌───────────────────┐                       │
│                │   Complete/Done   │                       │
│                └───────────────────┘                       │
└──────────────────────────────────────────────────────────────┘
```

---

## Query Function

**File**: `src/query.ts` (68KB)

### Main Generator Function

```typescript
export async function* query(
  messages: Message[],
  options: QueryOptions
): AsyncGenerator<QueryResult, void, unknown> {
  // Initialize query context
  const context: QueryContext = {
    messages: [...messages],
    tools: options.tools,
    mcpClients: options.mcpClients,
    abortController: new AbortController(),
    // ... other context
  }
  
  let iteration = 0
  const maxIterations = 100
  
  while (iteration < maxIterations) {
    iteration++
    
    // Check if aborted
    if (context.abortController.signal.aborted) {
      yield { type: 'abort' }
      return
    }
    
    // Send to LLM
    const stream = await streamResponse(context, options)
    
    // Process stream
    const chunks: StreamChunk[] = []
    let currentToolCalls: ToolCall[] = []
    let currentContent = ''
    
    for await (const chunk of stream) {
      switch (chunk.type) {
        case 'content':
          currentContent += chunk.content
          yield {
            type: 'content',
            content: chunk.content,
            accumulated: currentContent
          }
          break
          
        case 'tool_use':
          currentToolCalls.push(chunk.toolCall)
          yield {
            type: 'tool_use',
            toolCall: chunk.toolCall
          }
          break
          
        case 'stop':
          // Stream complete
          break
      }
    }
    
    // Add assistant message to history
    context.messages.push({
      role: 'assistant',
      content: currentContent,
      tool_calls: currentToolCalls.length > 0 ? currentToolCalls : undefined
    })
    
    // If no tool calls, we're done
    if (currentToolCalls.length === 0) {
      yield { type: 'complete' }
      return
    }
    
    // Execute tools
    const toolResults = await executeToolCalls(
      currentToolCalls, 
      context, 
      options
    )
    
    // Add tool results to history
    for (const result of toolResults) {
      context.messages.push({
        role: 'tool',
        tool_call_id: result.callId,
        content: JSON.stringify(result.output)
      })
    }
    
    // Yield tool results
    yield {
      type: 'tool_results',
      results: toolResults
    }
    
    // Continue loop (next iteration will send updated messages to LLM)
  }
  
  // Max iterations reached
  yield {
    type: 'error',
    error: 'Maximum iterations reached'
  }
}
```

### Query Options

```typescript
export interface QueryOptions {
  // Commands for slash command handling
  commands: Command[]
  
  // Debug mode
  debug: boolean
  
  // Model configuration
  mainLoopModel: string
  
  // Available tools
  tools: Tool[]
  
  // Verbose output
  verbose: boolean
  
  // Thinking configuration
  thinkingConfig: ThinkingConfig
  
  // MCP clients for external tools
  mcpClients: MCPClient[]
  
  // MCP resources
  mcpResources: Record<string, Resource[]>
  
  // Session type
  isNonInteractiveSession: boolean
  
  // Agent definitions for subagents
  agentDefinitions: AgentDefinitionsResult
  
  // Budget limit
  maxBudgetUsd?: number
  
  // Custom system prompt
  customSystemPrompt?: string
  
  // Additional system prompt
  appendSystemPrompt?: string
  
  // Analytics tracking
  querySource?: QuerySource
  
  // Tool refresh callback (for dynamic tool updates)
  refreshTools?: () => Tool[]
}
```

### Query Results

```typescript
export type QueryResult =
  | { type: 'content'; content: string; accumulated: string }
  | { type: 'tool_use'; toolCall: ToolCall }
  | { type: 'tool_results'; results: ToolResult[] }
  | { type: 'thinking'; thinking: string }
  | { type: 'complete' }
  | { type: 'abort' }
  | { type: 'error'; error: string }
```

---

## Query Engine

**File**: `src/QueryEngine.ts` (47KB)

### QueryEngine Class

```typescript
export class QueryEngine {
  private client: AnthropicClient
  private config: QueryEngineConfig
  private conversationManager: ConversationManager
  private costTracker: CostTracker
  
  constructor(config: QueryEngineConfig) {
    this.client = new AnthropicClient({
      apiKey: config.apiKey,
      baseURL: config.baseURL
    })
    this.config = config
    this.conversationManager = new ConversationManager()
    this.costTracker = new CostTracker()
  }
  
  async executeQuery(
    request: QueryRequest
  ): Promise<QueryResponse> {
    // Build messages for API
    const messages = this.buildMessages(request)
    
    // Add system prompt
    const systemPrompt = this.buildSystemPrompt(request)
    
    // Call API
    const response = await this.client.messages.create({
      model: request.model,
      messages,
      system: systemPrompt,
      tools: this.formatTools(request.tools),
      max_tokens: request.maxTokens || 4096
    })
    
    // Track cost
    this.costTracker.trackUsage({
      model: request.model,
      inputTokens: response.usage.input_tokens,
      outputTokens: response.usage.output_tokens
    })
    
    // Process response
    return this.processResponse(response)
  }
  
  async *streamQuery(
    request: QueryRequest
  ): AsyncIterable<StreamChunk> {
    // Build messages
    const messages = this.buildMessages(request)
    const systemPrompt = this.buildSystemPrompt(request)
    
    // Start streaming
    const stream = await this.client.messages.create({
      model: request.model,
      messages,
      system: systemPrompt,
      tools: this.formatTools(request.tools),
      max_tokens: request.maxTokens || 4096,
      stream: true
    })
    
    // Yield chunks
    for await (const event of stream) {
      switch (event.type) {
        case 'content_block_delta':
          if (event.delta.type === 'text') {
            yield {
              type: 'content',
              content: event.delta.text
            }
          }
          break
          
        case 'content_block_start':
          if (event.content_block.type === 'tool_use') {
            yield {
              type: 'tool_use',
              toolCall: {
                id: event.content_block.id,
                name: event.content_block.name,
                input: event.content_block.input
              }
            }
          }
          break
          
        case 'message_stop':
          yield { type: 'stop' }
          break
      }
    }
  }
  
  private buildMessages(request: QueryRequest): MessageParam[] {
    // Convert internal messages to API format
    return request.messages.map(msg => ({
      role: msg.role,
      content: this.formatContent(msg)
    }))
  }
  
  private formatContent(msg: Message): MessageParam['content'] {
    if (typeof msg.content === 'string') {
      return msg.content
    }
    
    // Handle complex content (images, etc.)
    return msg.content
  }
  
  private buildSystemPrompt(request: QueryRequest): string {
    const parts: string[] = []
    
    // Base system prompt
    parts.push(getSystemPrompt())
    
    // Custom system prompt if provided
    if (request.customSystemPrompt) {
      parts.push(request.customSystemPrompt)
    }
    
    // Append additional prompt if provided
    if (request.appendSystemPrompt) {
      parts.push(request.appendSystemPrompt)
    }
    
    // Add tool descriptions
    parts.push(this.formatToolDescriptions(request.tools))
    
    return parts.join('\n\n')
  }
  
  private formatTools(tools: Tool[]): ToolDefinition[] {
    return tools.map(tool => ({
      name: tool.name,
      description: tool.description,
      input_schema: tool.inputSchema
    }))
  }
  
  private processResponse(response: Message): QueryResponse {
    const content: ContentBlock[] = []
    const toolCalls: ToolCall[] = []
    
    for (const block of response.content) {
      if (block.type === 'text') {
        content.push({ type: 'text', text: block.text })
      } else if (block.type === 'tool_use') {
        toolCalls.push({
          id: block.id,
          name: block.name,
          input: block.input
        })
      }
    }
    
    return {
      content,
      toolCalls,
      usage: response.usage
    }
  }
}
```

---

## State Management

### State Architecture

Claude Code uses a **custom store pattern** similar to Zustand but implemented internally.

```
┌─────────────────────────────────────────┐
│           State Architecture             │
│                                          │
│  ┌──────────────┐    ┌──────────────┐   │
│  │   AppState   │    │    Store     │   │
│  │   (types)    │    │  (factory)   │   │
│  └───────┬──────┘    └──────┬───────┘   │
│          │                   │           │
│          └─────────┬─────────┘             │
│                    │                       │
│                    ▼                       │
│          ┌─────────────────┐              │
│          │  AppStateStore  │              │
│          │  (instance)     │              │
│          └────────┬────────┘              │
│                   │                        │
│          ┌────────┴────────┐               │
│          │                 │               │
│          ▼                 ▼               │
│  ┌──────────────┐  ┌──────────────┐       │
│  │  useAppState │  │useSetAppState│       │
│  │   (read)     │  │   (write)    │       │
│  └──────────────┘  └──────────────┘       │
│          │                 │               │
│          └────────┬────────┘               │
│                   │                        │
│                   ▼                        │
│          ┌─────────────────┐              │
│          │   Components    │              │
│          │   (subscribe)   │              │
│          └─────────────────┘              │
└─────────────────────────────────────────┘
```

### Store Implementation

**File**: `src/state/store.ts`

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
): Store<T> {
  let state = initialState
  const listeners = new Set<Listener>()
  
  return {
    getState: () => state,
    
    setState: (updater: (prev: T) => T) => {
      const prev = state
      const next = updater(prev)
      
      // Only update if changed
      if (Object.is(next, prev)) return
      
      state = next
      
      // Notify change handler
      onChange?.({ newState: next, oldState: prev })
      
      // Notify all listeners
      for (const listener of listeners) {
        listener()
      }
    },
    
    subscribe: (listener: Listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }
}
```

### AppState Structure

**File**: `src/state/AppStateStore.ts`

```typescript
export interface AppState {
  // ═══════════════════════════════════════════════════
  // Conversation State
  // ═══════════════════════════════════════════════════
  
  /** All messages in current conversation */
  messages: Message[]
  
  /** Messages being composed but not yet sent */
  pendingMessages: PendingMessage[]
  
  // ═══════════════════════════════════════════════════
  // Tool Execution State
  // ═══════════════════════════════════════════════════
  
  /** Current tool use state */
  toolUse: ToolUseState
  
  /** Results from completed tool calls */
  toolResults: ToolResult[]
  
  /** Pending permission requests */
  pendingPermissions: PendingPermission[]
  
  // ═══════════════════════════════════════════════════
  // Configuration
  // ═══════════════════════════════════════════════════
  
  /** Verbose mode flag */
  verbose: boolean
  
  /** Debug mode flag */
  debug: boolean
  
  /** Current model */
  mainLoopModel: string
  
  /** Available commands */
  commands: Command[]
  
  // ═══════════════════════════════════════════════════
  // MCP State
  // ═══════════════════════════════════════════════════
  
  /** MCP server configurations */
  mcpServers: MCPServerConfig[]
  
  /** Active MCP client connections */
  mcpClients: MCPClient[]
  
  /** MCP resources by server */
  mcpResources: Record<string, Resource[]>
  
  // ═══════════════════════════════════════════════════
  // Agent State
  // ═══════════════════════════════════════════════════
  
  /** Current agent mode */
  agentMode: AgentMode
  
  /** Loaded agent definitions */
  agentDefinitions: AgentDefinitionsResult
  
  /** Active subagents */
  subagents: Map<string, SubagentState>
  
  // ═══════════════════════════════════════════════════
  // Permission Context
  // ═══════════════════════════════════════════════════
  
  /** Tool permission context */
  toolPermissionContext: ToolPermissionContext
  
  // ═══════════════════════════════════════════════════
  // UI State
  // ═══════════════════════════════════════════════════
  
  /** Current theme */
  theme: Theme
  
  /** Sidebar visibility */
  sidebarVisible: boolean
  
  /** Current view */
  currentView: ViewType
  
  /** Spinner state */
  spinner: SpinnerState | null
  
  /** Input state */
  input: InputState
  
  // ═══════════════════════════════════════════════════
  // Session Infrastructure
  // ═══════════════════════════════════════════════════
  
  /** Session hooks */
  sessionHooks: SessionHooks
  
  /** Speculation state */
  speculation: SpeculationState
  
  /** Query chain tracking */
  queryChain: QueryChainTracking
  
  // ═══════════════════════════════════════════════════
  // Background Tasks
  // ═══════════════════════════════════════════════════
  
  /** Running background tasks */
  backgroundTasks: BackgroundTask[]
  
  /** Completed tasks */
  completedTasks: CompletedTask[]
}
```

### React Integration

**File**: `src/state/AppState.tsx`

```typescript
export const AppStoreContext = React.createContext<AppStateStore | null>(null)

export function AppStateProvider({
  children,
  initialState,
  onChangeAppState
}: AppStateProviderProps): React.ReactElement {
  // Create store instance
  const [store] = useState(() => 
    createStore(initialState ?? getDefaultAppState(), onChangeAppState)
  )
  
  // Handle settings changes
  useEffect(() => {
    return applySettingsChange(store)
  }, [store])
  
  return (
    <AppStoreContext.Provider value={store}>
      <MailboxProvider>
        <VoiceProvider>{children}</VoiceProvider>
      </MailboxProvider>
    </AppStoreContext.Provider>
  )
}

// Hook for reading state
export function useAppState<T>(selector: (state: AppState) => T): T {
  const store = useContext(AppStoreContext)
  if (!store) {
    throw new Error('useAppState must be used within AppStateProvider')
  }
  
  const [selected, setSelected] = useState(() => 
    selector(store.getState())
  )
  
  useEffect(() => {
    return store.subscribe(() => {
      const newSelected = selector(store.getState())
      setSelected(prev => {
        // Only update if changed (Object.is comparison)
        if (Object.is(newSelected, prev)) return prev
        return newSelected
      })
    })
  }, [store, selector])
  
  return selected
}

// Hook for updating state
export function useSetAppState(): (
  updater: (prev: AppState) => AppState
) => void {
  const store = useContext(AppStoreContext)
  if (!store) {
    throw new Error('useSetAppState must be used within AppStateProvider')
  }
  
  return store.setState
}
```

### Usage Examples

```typescript
// Reading state
function MyComponent() {
  const verbose = useAppState(s => s.verbose)
  const messages = useAppState(s => s.messages)
  
  // Only re-renders when these specific values change
  return <Text>Messages: {messages.length}, Verbose: {verbose}</Text>
}

// Updating state
function ToggleVerbose() {
  const setAppState = useSetAppState()
  
  const toggle = () => {
    setAppState(prev => ({
      ...prev,
      verbose: !prev.verbose
    }))
  }
  
  return <Button onPress={toggle}>Toggle Verbose</Button>
}

// Multiple updates
function AddMessage(content: string) {
  const setAppState = useSetAppState()
  
  setAppState(prev => ({
    ...prev,
    messages: [
      ...prev.messages,
      { role: 'user', content, id: generateId() }
    ],
    // Reset input
    input: { ...prev.input, value: '' }
  }))
}
```

---

## Message Flow

### Message Types

```typescript
export type Message =
  | UserMessage
  | AssistantMessage
  | SystemMessage
  | ToolMessage
  | ProgressMessage
  | AttachmentMessage

export interface UserMessage {
  role: 'user'
  content: string
  id: string
  timestamp: Date
  attachments?: Attachment[]
}

export interface AssistantMessage {
  role: 'assistant'
  content: string
  id: string
  timestamp: Date
  thinking?: string
  tool_calls?: ToolCall[]
}

export interface ToolMessage {
  role: 'tool'
  tool_call_id: string
  content: string
  id: string
  is_error?: boolean
}
```

### Message Flow Diagram

```
User Input
    │
    ▼
┌─────────────────┐
│ Create User Msg │
└────────┬────────┘
         │
         ▼
┌─────────────────┐    ┌─────────────────┐
│ Add to Messages │───►│ Update UI       │
└────────┬────────┘    └─────────────────┘
         │
         ▼
┌─────────────────┐
│ Send to LLM     │
│ (query.ts)      │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Stream Response │
└────────┬────────┘
         │
    ┌────┴────┐
    │         │
    ▼         ▼
┌───────┐ ┌───────────┐
│ Text  │ │ Tool Call │
└───┬───┘ └─────┬─────┘
    │           │
    ▼           ▼
┌───────────┐ ┌───────────┐
│ Show Text │ │ Execute   │
│ in Chat   │ │ Tool      │
└───────────┘ └─────┬─────┘
                    │
                    ▼
            ┌───────────────┐
            │ Add Tool      │
            │ Result to Msg │
            └───────┬───────┘
                    │
                    ▼
            ┌───────────────┐
            │ Send to LLM   │
            │ (continue)    │
            └───────────────┘
```

---

## Context Management

### Context Builder

```typescript
export function buildContext(
  state: AppState,
  options: ContextOptions
): ToolContext {
  return {
    getAppState: () => state,
    setAppState: options.setAppState,
    setAppStateForTasks: options.setAppStateForTasks,
    messages: state.messages,
    tools: options.tools || state.commands.flatMap(c => c.tools || []),
    mcpClients: state.mcpClients,
    mcpResources: state.mcpResources,
    abortController: new AbortController(),
    readFileState: createFileStateCache(),
    querySource: options.querySource
  }
}
```

### Context for Subagents

```typescript
export function createSubagentContext(
  parentContext: ToolContext,
  input: SubagentInput
): ToolContext {
  return {
    ...parentContext,
    
    // Fresh message history
    messages: [],
    
    // Can use subset of tools
    tools: input.tools || parentContext.tools,
    
    // Can use subset of MCP clients
    mcpClients: input.mcpClients || parentContext.mcpClients,
    
    // Custom system prompt
    systemPrompt: input.systemPrompt,
    
    // Track depth
    depth: (parentContext.depth || 0) + 1,
    
    // Always use parent's setAppStateForTasks for infrastructure
    setAppStateForTasks: parentContext.setAppStateForTasks
  }
}
```

---

## Speculation System

### Purpose

The speculation system enables predictive actions - the agent can "speculate" about what the user might want next and prepare for it.

### Speculation State

```typescript
export interface SpeculationState {
  enabled: boolean
  currentSpeculation: Speculation | null
  speculationHistory: SpeculationResult[]
}

export interface Speculation {
  id: string
  trigger: string
  predictedIntent: string
  confidence: number
  preparedActions: PreparedAction[]
  status: 'preparing' | 'ready' | 'executed' | 'cancelled'
}

export interface PreparedAction {
  type: 'tool_call' | 'message' | 'command'
  content: unknown
  prepared: boolean
}
```

### Speculation Flow

```typescript
export async function speculate(
  state: AppState,
  lastMessage: Message
): Promise<Speculation | null> {
  // Analyze last message
  const intent = await analyzeIntent(lastMessage)
  
  // Check if we can predict next action
  if (intent.confidence < 0.7) {
    return null
  }
  
  // Prepare actions
  const actions = await prepareActions(intent)
  
  return {
    id: generateId(),
    trigger: lastMessage.content,
    predictedIntent: intent.type,
    confidence: intent.confidence,
    preparedActions: actions,
    status: 'preparing'
  }
}
```

---

## Summary

Key concepts:

1. **Agent Loop** (`query.ts`): Generator-based async loop that streams LLM responses and processes tool calls

2. **Query Engine** (`QueryEngine.ts`): Abstraction over Anthropic API with streaming support

3. **State Management**: Custom store pattern with functional updates and subscription-based reactivity

4. **Message Flow**: Well-defined flow from user input → LLM → tool execution → results → UI

5. **Context Management**: ToolContext provides consistent state access across tools

6. **Speculation**: Predictive system for preparing future actions

Key files:
- `src/query.ts` - Main agent loop
- `src/QueryEngine.ts` - LLM API abstraction
- `src/state/store.ts` - Store factory
- `src/state/AppStateStore.ts` - State types
- `src/state/AppState.tsx` - React integration
