# Claude Code MCP Integration

> **Part of**: Claude Code Architecture Documentation  
> **Focus**: Model Context Protocol (MCP) integration  
> **Generated**: 2025

---

## Table of Contents

1. [MCP Overview](#mcp-overview)
2. [Architecture](#architecture)
3. [MCP Client](#mcp-client)
4. [Transport Layer](#transport-layer)
5. [Server Configuration](#server-configuration)
6. [Authentication](#authentication)
7. [Tool Integration](#tool-integration)
8. [Resource Management](#resource-management)
9. [Prompt Management](#prompt-management)

---

## MCP Overview

Model Context Protocol (MCP) is an open protocol that enables secure integration between AI assistants and external data sources and tools. Claude Code implements a full MCP client with multiple transport options.

### Supported Transports

| Transport | Protocol | Use Case |
|-----------|----------|----------|
| **stdio** | Standard I/O | Local command-based servers |
| **sse** | Server-Sent Events | HTTP streaming servers |
| **http** | HTTP/REST | Direct HTTP servers |
| **ws** | WebSocket | Real-time bidirectional |
| **sdk** | In-process | Built-in/embedded servers |

### Config Scopes

MCP servers can be configured at different scopes:

```typescript
type ConfigScope = 
  | 'local'      // Project-local .claude-mcp.json
  | 'user'       // User settings
  | 'project'    // Project configuration
  | 'dynamic'    // Runtime-added
  | 'enterprise' // Organization-wide
  | 'claudeai'   // Anthropic-hosted
  | 'managed'    // Managed by Claude
```

---

## Architecture

### MCP System Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Claude Code                           │
│                                                          │
│  ┌─────────────────┐    ┌──────────────────────────┐   │
│  │   MCPTool       │    │    MCPClientManager        │   │
│  │   (calls tools) │◄──►│    (manages connections)   │   │
│  └─────────────────┘    └───────────┬────────────────┘   │
│                                     │                    │
│                                     ▼                    │
│  ┌───────────────────────────────────────────────────┐  │
│  │              Transport Layer                       │  │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐  │  │
│  │  │  stdio  │ │   sse   │ │  http   │ │   ws    │  │  │
│  │  └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘  │  │
│  └───────┼────────────┼────────────┼────────────┼───────┘  │
│          │            │            │            │         │
└──────────┼────────────┼────────────┼────────────┼─────────┘
           │            │            │            │
           ▼            ▼            ▼            ▼
     ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
     │ Local    │ │ Remote   │ │ HTTP     │ │ WebSocket│
     │ Process  │ │ Server   │ │ Server   │ │ Server   │
     └──────────┘ └──────────┘ └──────────┘ └──────────┘
```

### Key Components

| Component | File | Purpose |
|-----------|------|---------|
| **MCPClient** | `services/mcp/client.ts` | Core MCP client |
| **MCPTool** | `tools/MCPTool/` | Tool interface |
| **types** | `services/mcp/types.ts` | Type definitions |
| **Transport** | `services/mcp/transport/` | Transport implementations |
| **Auth** | `services/mcp/auth/` | OAuth handling |

---

## MCP Client

### Client Implementation

**File**: `src/services/mcp/client.ts`

```typescript
export class MCPClient {
  private client: Client
  private transport: Transport
  private config: MCPClientConfig
  private connected = false
  private tools: Tool[] = []
  private resources: Resource[] = []
  
  constructor(config: MCPClientConfig) {
    this.config = config
    this.transport = createTransport(config)
    this.client = new Client({
      name: 'claude-code',
      version: getVersion()
    })
  }
  
  async connect(): Promise<void> {
    if (this.connected) return
    
    await this.client.connect(this.transport)
    this.connected = true
    
    // Discover capabilities
    await this.discoverCapabilities()
  }
  
  async disconnect(): Promise<void> {
    if (!this.connected) return
    
    await this.client.close()
    this.connected = false
  }
  
  private async discoverCapabilities(): Promise<void> {
    // List available tools
    const toolsResult = await this.client.listTools()
    this.tools = toolsResult.tools
    
    // List available resources
    const resourcesResult = await this.client.listResources()
    this.resources = resourcesResult.resources
  }
  
  async callTool(request: CallToolRequest): Promise<CallToolResult> {
    this.ensureConnected()
    return this.client.callTool(request)
  }
  
  async readResource(uri: string): Promise<ReadResourceResult> {
    this.ensureConnected()
    return this.client.readResource({ uri })
  }
  
  async getPrompt(name: string, args?: Record<string, string>): Promise<GetPromptResult> {
    this.ensureConnected()
    return this.client.getPrompt({ name, arguments: args })
  }
  
  private ensureConnected(): void {
    if (!this.connected) {
      throw new Error('MCP client not connected')
    }
  }
  
  getAvailableTools(): Tool[] {
    return this.tools
  }
  
  getAvailableResources(): Resource[] {
    return this.resources
  }
}
```

### Client Manager

```typescript
export class MCPClientManager {
  private clients = new Map<string, MCPClient>()
  
  async addServer(config: MCPServerConfig): Promise<MCPClient> {
    const client = new MCPClient(config)
    await client.connect()
    this.clients.set(config.name, client)
    return client
  }
  
  removeServer(name: string): void {
    const client = this.clients.get(name)
    if (client) {
      client.disconnect()
      this.clients.delete(name)
    }
  }
  
  getClient(name: string): MCPClient | undefined {
    return this.clients.get(name)
  }
  
  getAllClients(): MCPClient[] {
    return Array.from(this.clients.values())
  }
  
  async callTool(
    serverName: string, 
    toolName: string, 
    args: unknown
  ): Promise<CallToolResult> {
    const client = this.getClient(serverName)
    if (!client) {
      throw new Error(`MCP server '${serverName}' not found`)
    }
    return client.callTool({ name: toolName, arguments: args })
  }
}
```

---

## Transport Layer

### Transport Factory

```typescript
export function createTransport(config: MCPClientConfig): Transport {
  switch (config.transport.type) {
    case 'stdio':
      return new StdioTransport(config.transport)
    case 'sse':
      return new SSETransport(config.transport)
    case 'http':
      return new HTTPTransport(config.transport)
    case 'ws':
      return new WebSocketTransport(config.transport)
    case 'sdk':
      return new SDKTransport(config.transport)
    default:
      throw new Error(`Unknown transport: ${config.transport.type}`)
  }
}
```

### stdio Transport

**File**: `src/services/mcp/transport/stdio.ts`

```typescript
export class StdioTransport implements Transport {
  private process: ChildProcess
  private stdin: Writable
  private stdout: Readable
  private messageHandler?: (message: JSONRPCMessage) => void
  private errorHandler?: (error: Error) => void
  
  constructor(config: StdioTransportConfig) {
    // Spawn child process
    this.process = spawn(config.command, config.args, {
      env: { ...process.env, ...config.env },
      stdio: ['pipe', 'pipe', 'pipe']
    })
    
    this.stdin = this.process.stdin
    this.stdout = this.process.stdout
    
    // Handle stdout messages
    this.stdout.on('data', (data: Buffer) => {
      const messages = this.parseMessages(data.toString())
      messages.forEach(msg => this.messageHandler?.(msg))
    })
    
    // Handle stderr
    this.process.stderr.on('data', (data: Buffer) => {
      console.error(`MCP server stderr: ${data}`)
    })
    
    // Handle process exit
    this.process.on('exit', (code) => {
      if (code !== 0) {
        this.errorHandler?.(new Error(`MCP server exited with code ${code}`))
      }
    })
  }
  
  send(message: JSONRPCMessage): void {
    const json = JSON.stringify(message)
    this.stdin.write(json + '\n')
  }
  
  onMessage(handler: (message: JSONRPCMessage) => void): void {
    this.messageHandler = handler
  }
  
  onError(handler: (error: Error) => void): void {
    this.errorHandler = handler
  }
  
  async close(): Promise<void> {
    this.process.kill()
  }
  
  private parseMessages(data: string): JSONRPCMessage[] {
    return data
      .split('\n')
      .filter(line => line.trim())
      .map(line => JSON.parse(line))
  }
}
```

### SSE Transport

```typescript
export class SSETransport implements Transport {
  private eventSource: EventSource
  private messageHandler?: (message: JSONRPCMessage) => void
  private postEndpoint: string
  
  constructor(config: SSETransportConfig) {
    this.postEndpoint = config.url
    this.eventSource = new EventSource(config.url)
    
    this.eventSource.onmessage = (event) => {
      const message = JSON.parse(event.data)
      this.messageHandler?.(message)
    }
    
    this.eventSource.onerror = (error) => {
      console.error('SSE error:', error)
    }
  }
  
  async send(message: JSONRPCMessage): Promise<void> {
    const response = await fetch(this.postEndpoint, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        ...this.config.headers
      },
      body: JSON.stringify(message)
    })
    
    if (!response.ok) {
      throw new Error(`HTTP error: ${response.status}`)
    }
  }
  
  onMessage(handler: (message: JSONRPCMessage) => void): void {
    this.messageHandler = handler
  }
  
  async close(): Promise<void> {
    this.eventSource.close()
  }
}
```

### HTTP Transport

```typescript
export class HTTPTransport implements Transport {
  private baseUrl: string
  private headers: Record<string, string>
  private pendingResponses = new Map<string, (msg: JSONRPCMessage) => void>()
  
  constructor(config: HTTPTransportConfig) {
    this.baseUrl = config.url
    this.headers = config.headers || {}
  }
  
  async send(message: JSONRPCMessage): Promise<void> {
    const response = await fetch(`${this.baseUrl}/mcp`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        ...this.headers
      },
      body: JSON.stringify(message)
    })
    
    if (!response.ok) {
      throw new Error(`HTTP error: ${response.status}`)
    }
    
    const result = await response.json()
    this.messageHandler?.(result)
  }
  
  onMessage(handler: (message: JSONRPCMessage) => void): void {
    this.messageHandler = handler
  }
  
  async close(): Promise<void> {
    // Nothing to close for HTTP
  }
}
```

---

## Server Configuration

### Configuration Types

**File**: `src/services/mcp/types.ts`

```typescript
// Stdio server (local command)
export interface StdioServerConfig {
  type: 'stdio'
  command: string
  args?: string[]
  env?: Record<string, string>
}

// SSE server (HTTP streaming)
export interface SSEServerConfig {
  type: 'sse'
  url: string
  headers?: Record<string, string>
  oauth?: OAuthConfig
}

// HTTP server (REST API)
export interface HTTPServerConfig {
  type: 'http'
  url: string
  headers?: Record<string, string>
  oauth?: OAuthConfig
}

// WebSocket server
export interface WebSocketServerConfig {
  type: 'ws'
  url: string
  headers?: Record<string, string>
}

// SDK/in-process server
export interface SDKServerConfig {
  type: 'sdk'
  factory: () => Server
}

// OAuth configuration
export interface OAuthConfig {
  clientId?: string
  callbackPort?: number
  authServerMetadataUrl?: string
  xaa?: boolean // Cross-App Access
}
```

### Server Configuration Examples

```json
// Project config: .claude-mcp.json
{
  "mcpServers": {
    "github": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@anthropic/mcp-github"],
      "env": {
        "GITHUB_TOKEN": "${env:GITHUB_TOKEN}"
      }
    },
    "postgres": {
      "type": "sse",
      "url": "https://mcp.example.com/postgres",
      "headers": {
        "Authorization": "Bearer ${env:DB_TOKEN}"
      }
    },
    "internal": {
      "type": "http",
      "url": "https://internal.company.com/mcp",
      "oauth": {
        "clientId": "claude-code",
        "callbackPort": 3333
      }
    }
  }
}
```

### Config Loading

```typescript
export async function loadMCPConfigs(): Promise<MCPServerConfig[]> {
  const configs: MCPServerConfig[] = []
  
  // Load from all scopes
  const scopes: ConfigScope[] = [
    'enterprise',
    'managed',
    'user',
    'project',
    'local'
  ]
  
  for (const scope of scopes) {
    const scopeConfigs = await loadConfigForScope(scope)
    configs.push(...scopeConfigs)
  }
  
  // Merge and deduplicate
  return mergeConfigs(configs)
}

async function loadConfigForScope(
  scope: ConfigScope
): Promise<MCPServerConfig[]> {
  const configPath = getConfigPathForScope(scope)
  
  try {
    const content = await fs.readFile(configPath, 'utf-8')
    const parsed = JSON.parse(content)
    return parsed.mcpServers || []
  } catch {
    return []
  }
}
```

---

## Authentication

### OAuth Flow

```typescript
export async function handleOAuth(
  serverName: string,
  config: OAuthConfig
): Promise<string> {
  // 1. Start local callback server
  const callbackServer = await startCallbackServer(config.callbackPort)
  
  // 2. Build authorization URL
  const authUrl = buildAuthorizationUrl({
    clientId: config.clientId,
    redirectUri: `http://localhost:${config.callbackPort}/callback`,
    scope: 'openid profile'
  })
  
  // 3. Open browser for authorization
  await open(authUrl)
  
  // 4. Wait for callback
  const code = await callbackServer.waitForCode()
  await callbackServer.close()
  
  // 5. Exchange code for tokens
  const tokens = await exchangeCodeForTokens({
    code,
    clientId: config.clientId,
    redirectUri: `http://localhost:${config.callbackPort}/callback`
  })
  
  // 6. Store tokens securely
  await storeTokens(serverName, tokens)
  
  return tokens.accessToken
}
```

### Token Storage

```typescript
export async function storeTokens(
  serverName: string, 
  tokens: OAuthTokens
): Promise<void> {
  const key = `mcp:${serverName}:tokens`
  const encrypted = await encrypt(JSON.stringify(tokens))
  await keytar.setPassword('claude-code-mcp', key, encrypted)
}

export async function getTokens(serverName: string): Promise<OAuthTokens | null> {
  const key = `mcp:${serverName}:tokens`
  const encrypted = await keytar.getPassword('claude-code-mcp', key)
  
  if (!encrypted) return null
  
  const decrypted = await decrypt(encrypted)
  return JSON.parse(decrypted)
}
```

---

## Tool Integration

### MCP Tool Wrapper

```typescript
export class MCPTool implements Tool<MCPToolInput, MCPToolOutput> {
  name = 'mcp'
  description = 'Call tools from MCP servers'
  
  inputSchema = {
    type: 'object',
    properties: {
      server_name: { type: 'string' },
      tool_name: { type: 'string' },
      arguments: { type: 'object' }
    },
    required: ['server_name', 'tool_name']
  }
  
  async call(
    input: MCPToolInput, 
    context: ToolContext
  ): Promise<ToolResult<MCPToolOutput>> {
    // Find MCP client
    const mcpClient = context.mcpClients.find(
      c => c.serverName === input.server_name
    )
    
    if (!mcpClient) {
      return {
        isError: true,
        errorMessage: `MCP server '${input.server_name}' not found`
      }
    }
    
    try {
      // Call tool via MCP
      const result = await mcpClient.callTool({
        name: input.tool_name,
        arguments: input.arguments
      })
      
      return { output: result }
    } catch (error) {
      return {
        isError: true,
        errorMessage: String(error)
      }
    }
  }
  
  async checkPermissions(): Promise<PermissionResult> {
    // MCP tools generally require explicit permission
    return { type: 'ask' }
  }
}
```

### Dynamic Tool Discovery

```typescript
export async function discoverMCPTools(
  mcpClients: MCPClient[]
): Promise<Tool[]> {
  const tools: Tool[] = []
  
  for (const client of mcpClients) {
    const availableTools = client.getAvailableTools()
    
    for (const mcpTool of availableTools) {
      // Create wrapper tool for each MCP tool
      const wrapperTool = createMCPWrapperTool(client, mcpTool)
      tools.push(wrapperTool)
    }
  }
  
  return tools
}

function createMCPWrapperTool(
  client: MCPClient, 
  mcpTool: MCPToolDefinition
): Tool {
  return {
    name: `mcp_${client.serverName}_${mcpTool.name}`,
    description: mcpTool.description,
    inputSchema: mcpTool.inputSchema,
    
    async call(input: unknown): Promise<ToolResult> {
      const result = await client.callTool({
        name: mcpTool.name,
        arguments: input
      })
      return { output: result }
    },
    
    isConcurrencySafe(): boolean {
      // Default to false for MCP tools
      return false
    }
  }
}
```

---

## Resource Management

### Resource Discovery

```typescript
export async function listMCPResources(
  client: MCPClient
): Promise<Resource[]> {
  return client.getAvailableResources()
}
```

### Resource Reading

```typescript
export async function readMCPResource(
  client: MCPClient,
  uri: string
): Promise<ResourceContent> {
  const result = await client.readResource(uri)
  return result.contents[0]
}
```

### Resource Integration

```typescript
// Tool to read MCP resources
export class ReadMCPResourceTool implements Tool {
  name = 'read_mcp_resource'
  description = 'Read a resource from an MCP server'
  
  async call(input: { server: string; uri: string }, context: ToolContext): Promise<ToolResult> {
    const client = context.mcpClients.find(c => c.serverName === input.server)
    if (!client) {
      return { isError: true, errorMessage: 'Server not found' }
    }
    
    const content = await readMCPResource(client, input.uri)
    return { output: content }
  }
}
```

---

## Prompt Management

### Prompt Discovery

```typescript
export async function listMCPPrompts(client: MCPClient): Promise<Prompt[]> {
  const result = await client.client.listPrompts()
  return result.prompts
}
```

### Prompt Usage

```typescript
export async function getMCPPrompt(
  client: MCPClient,
  name: string,
  args?: Record<string, string>
): Promise<string> {
  const result = await client.getPrompt(name, args)
  return result.messages.map(m => m.content).join('\n')
}
```

---

## MCP Commands

### `/mcp` Command

```typescript
export const mcpCommand: Command = {
  name: 'mcp',
  description: 'Manage MCP servers',
  
  async execute(args: string[], context: CommandContext): Promise<void> {
    const subcommand = args[0]
    
    switch (subcommand) {
      case 'list':
        await listMCPServers(context)
        break
      case 'add':
        await addMCPServer(args[1], context)
        break
      case 'remove':
        await removeMCPServer(args[1], context)
        break
      case 'connect':
        await connectMCPServer(args[1], context)
        break
      case 'disconnect':
        await disconnectMCPServer(args[1], context)
        break
      default:
        console.log('Unknown subcommand. Try: list, add, remove, connect, disconnect')
    }
  }
}

async function listMCPServers(context: CommandContext): Promise<void> {
  const { mcpClients } = context.getAppState()
  
  for (const client of mcpClients) {
    const tools = client.getAvailableTools()
    console.log(`${client.serverName}: ${tools.length} tools`)
  }
}
```

---

## Error Handling

### Common Errors

```typescript
export enum MCPErrorCode {
  CONNECTION_FAILED = -32000,
  SERVER_NOT_FOUND = -32001,
  TOOL_NOT_FOUND = -32002,
  AUTHENTICATION_FAILED = -32003,
  PERMISSION_DENIED = -32004,
  TIMEOUT = -32005
}

export class MCPError extends Error {
  constructor(
    public code: MCPErrorCode,
    message: string,
    public serverName?: string
  ) {
    super(message)
  }
}
```

### Error Recovery

```typescript
export async function withMCPRetry<T>(
  operation: () => Promise<T>,
  maxRetries = 3
): Promise<T> {
  let lastError: Error | undefined
  
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await operation()
    } catch (error) {
      lastError = error as Error
      
      // Don't retry certain errors
      if (error instanceof MCPError) {
        if (error.code === MCPErrorCode.PERMISSION_DENIED) {
          throw error
        }
      }
      
      // Exponential backoff
      await delay(1000 * Math.pow(2, i))
    }
  }
  
  throw lastError
}
```

---

## Summary

Claude Code's MCP integration provides:

1. **Multiple Transports**: stdio, SSE, HTTP, WebSocket, SDK
2. **Full Protocol Support**: Tools, Resources, Prompts
3. **Authentication**: OAuth with secure token storage
4. **Flexible Configuration**: Multiple config scopes
5. **Dynamic Discovery**: Runtime tool/resource discovery
6. **Error Recovery**: Retry logic with backoff

Key files:
- `src/services/mcp/types.ts` - Type definitions
- `src/services/mcp/client.ts` - MCP client
- `src/services/mcp/transport/` - Transport implementations
- `src/services/mcp/auth/` - OAuth handling
- `src/tools/MCPTool/` - Tool interface
