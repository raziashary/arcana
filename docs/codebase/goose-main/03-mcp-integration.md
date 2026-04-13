# Goose MCP Integration - Deep Dive

> **Part of**: Goose Architecture Documentation  
> **Focus**: Model Context Protocol integration  
> **Crate**: `goose` + `goose-mcp`  
> **Generated**: 2025

---

## Table of Contents

1. [MCP Architecture](#mcp-architecture)
2. [Extension Manager](#extension-manager)
3. [MCP Client](#mcp-client)
4. [Transport Types](#transport-types)
5. [Tool Discovery](#tool-discovery)
6. [Tool Execution](#tool-execution)
7. [Resource Management](#resource-management)
8. [Prompt Management](#prompt-management)
9. [Security](#security)

---

## MCP Architecture

### Overview

Goose uses the **Model Context Protocol (MCP)** to integrate with external tools. MCP is an open protocol that standardizes how AI agents connect to tools, resources, and prompts.

```
┌─────────────────────────────────────────────────────────────┐
│                    MCP Architecture                          │
│                                                              │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │    Agent     │    │   Extension  │    │   External   │  │
│  │              │◄──►│   Manager    │◄──►│    Tools     │  │
│  │              │    │   (MCP)      │    │   (MCP)      │  │
│  └──────────────┘    └──────┬───────┘    └──────────────┘  │
│                              │                              │
│          ┌───────────────────┼───────────────────┐          │
│          │                   │                   │          │
│          ▼                   ▼                   ▼          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  stdio       │  │  SSE         │  │  HTTP        │      │
│  │  transport   │  │  transport   │  │  transport   │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│          │                   │                   │          │
│          ▼                   ▼                   ▼          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Local        │  │ Remote       │  │ REST         │      │
│  │ Process      │  │ Server       │  │ API          │      │
│  │ (npx, uvx)   │  │ (SSE)        │  │              │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### MCP Flow

```
┌─────────────────────────────────────────────────────────────┐
│                     Tool Call Flow                             │
│                                                              │
│  1. LLM detects tool need                                    │
│     │                                                        │
│     ▼                                                        │
│  2. Agent receives tool request                            │
│     │                                                        │
│     ▼                                                        │
│  3. Parse tool name (extension__tool)                      │
│     │                                                        │
│     ▼                                                        │
│  4. Route to ExtensionManager                              │
│     │                                                        │
│     ▼                                                        │
│  5. ExtensionManager finds MCP client                      │
│     │                                                        │
│     ▼                                                        │
│  6. MCP client sends call_tool request                     │
│     │                                                        │
│     ▼                                                        │
│  7. Transport layer delivers to external server            │
│     │                                                        │
│     ▼                                                        │
│  8. External server executes tool                          │
│     │                                                        │
│     ▼                                                        │
│  9. Result flows back through MCP                          │
│     │                                                        │
│     ▼                                                        │
│  10. Agent receives result and sends to LLM                │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## Extension Manager

**File**: `crates/goose/src/agents/extension_manager.rs` (83468 bytes)

### ExtensionManager Structure

```rust
/// Manages MCP extensions and their tools
pub struct ExtensionManager {
    /// Active extension clients (name -> client)
    clients: Arc<Mutex<HashMap<String, ExtensionClient>>>,
    
    /// Cached tools from all extensions
    tool_cache: Arc<RwLock<Vec<Tool>>>,
    
    /// Tool cache timestamp
    tool_cache_time: Arc<RwLock<Option<Instant>>>,
    
    /// Permission manager for tool access control
    permission_manager: Arc<PermissionManager>,
    
    /// Platform extension manager (built-in tools)
    platform_extension_manager: PlatformExtensionManager,
    
    /// Cache TTL
    cache_ttl: Duration,
}

/// Extension client wrapper
pub enum ExtensionClient {
    /// MCP-based client
    Mcp(MCPClient),
    
    /// Built-in extension
    Builtin(BuiltinExtension),
    
    /// HTTP-based extension
    Http(HttpExtensionClient),
}

impl ExtensionClient {
    /// List available tools
    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        match self {
            ExtensionClient::Mcp(client) => client.list_tools().await,
            ExtensionClient::Builtin(ext) => ext.list_tools().await,
            ExtensionClient::Http(client) => client.list_tools().await,
        }
    }
    
    /// Call a tool
    pub async fn call_tool(&self, request: ToolRequest) -> Result<CallToolResult> {
        match self {
            ExtensionClient::Mcp(client) => client.call_tool(request).await,
            ExtensionClient::Builtin(ext) => ext.call_tool(request).await,
            ExtensionClient::Http(client) => client.call_tool(request).await,
        }
    }
    
    /// Ping to check connectivity
    pub async fn ping(&self) -> Result<()> {
        match self {
            ExtensionClient::Mcp(client) => client.ping().await,
            ExtensionClient::Builtin(_) => Ok(()),
            ExtensionClient::Http(client) => client.ping().await,
        }
    }
}
```

### Adding Extensions

```rust
impl ExtensionManager {
    /// Add a new extension
    pub async fn add_extension(
        &self,
        config: ExtensionConfig,
    ) -> Result<ExtensionResult> {
        info!("Adding extension: {}", config.name);
        
        // Validate extension
        self.validate_extension(&config).await?;
        
        // Check for malware (if applicable)
        if let Some(risk) = self.check_malware(&config).await? {
            warn!("Extension {} flagged: {}", config.name, risk);
            // Depending on risk level, may block or warn
        }
        
        // Create client based on transport
        let client = match config.transport {
            TransportType::Stdio => {
                self.create_stdio_client(&config).await?
            }
            TransportType::Sse => {
                self.create_sse_client(&config).await?
            }
            TransportType::Http => {
                self.create_http_client(&config).await?
            }
            TransportType::Builtin => {
                self.create_builtin_client(&config).await?
            }
        };
        
        // Test connectivity
        if let Err(e) = client.ping().await {
            return Ok(ExtensionResult {
                name: config.name,
                success: false,
                error: Some(format!("Connection failed: {}", e)),
            });
        }
        
        // Store client
        let name = config.name.clone();
        {
            let mut clients = self.clients.lock().await;
            clients.insert(name.clone(), client);
        }
        
        // Invalidate tool cache
        self.invalidate_tool_cache();
        
        info!("Extension {} added successfully", name);
        
        Ok(ExtensionResult {
            name,
            success: true,
            error: None,
        })
    }
    
    /// Remove an extension
    pub async fn remove_extension(&self, name: &str) -> Result<()> {
        let mut clients = self.clients.lock().await;
        
        if let Some(client) = clients.remove(name) {
            // Graceful shutdown
            match client {
                ExtensionClient::Mcp(mut c) => c.shutdown().await?,
                _ => {}
            }
        }
        
        self.invalidate_tool_cache();
        
        Ok(())
    }
    
    /// List all available tools from all extensions
    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        // Check cache first
        if let Some(cached) = self.get_cached_tools().await {
            return Ok(cached);
        }
        
        // Collect tools from all extensions
        let mut all_tools = Vec::new();
        let clients = self.clients.lock().await;
        
        for (name, client) in clients.iter() {
            match client.list_tools().await {
                Ok(mut tools) => {
                    // Prefix tool names with extension name
                    for tool in &mut tools {
                        tool.name = format!("{}__{}", name, tool.name);
                    }
                    all_tools.extend(tools);
                }
                Err(e) => {
                    warn!("Failed to list tools for {}: {}", name, e);
                }
            }
        }
        
        // Add platform tools
        let platform_tools = self.platform_extension_manager.list_tools().await?;
        all_tools.extend(platform_tools);
        
        // Update cache
        self.update_tool_cache(all_tools.clone()).await;
        
        Ok(all_tools)
    }
    
    /// Call a tool by name
    pub async fn call_tool(
        &self,
        request: ToolRequest,
    ) -> Result<ToolResult<CallToolResult>> {
        // Parse tool name (format: extension__tool)
        let parts: Vec<&str> = request.name.splitn(2, "__").collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid tool name format: {}", request.name));
        }
        
        let extension_name = parts[0];
        let tool_name = parts[1];
        
        // Find extension
        let clients = self.clients.lock().await;
        let client = clients
            .get(extension_name)
            .ok_or_else(|| anyhow!("Extension '{}' not found", extension_name))?;
        
        // Check permission
        let has_permission = self.permission_manager
            .check_tool_permission(extension_name, tool_name)
            .await?;
        
        if !has_permission {
            return Ok(ToolResult::PermissionDenied);
        }
        
        // Create unprefixed request
        let mut inner_request = request.clone();
        inner_request.name = tool_name.to_string();
        
        // Execute with timeout
        let result = tokio::time::timeout(
            Duration::from_secs(request.timeout_secs.unwrap_or(60)),
            client.call_tool(inner_request),
        ).await;
        
        match result {
            Ok(Ok(result)) => Ok(ToolResult::Success(result)),
            Ok(Err(e)) => Ok(ToolResult::Error(e.to_string())),
            Err(_) => Ok(ToolResult::Timeout),
        }
    }
}
```

---

## MCP Client

**File**: `crates/goose/src/agents/mcp_client.rs` (33828 bytes)

### MCP Client Implementation

```rust
use rmcp::{
    client::{Client, ClientBuilder},
    model::{CallToolRequest, CallToolResult, Tool, Resource, GetPromptResult},
    transport::{StdioTransport, SseTransport},
};

/// MCP client wrapper for rmcp
pub struct MCPClient {
    /// Underlying rmcp client
    client: Client,
    
    /// Extension configuration
    config: ExtensionConfig,
    
    /// Transport handle (to keep alive)
    _transport_handle: Option<JoinHandle<()>>,
}

impl MCPClient {
    /// Create a new MCP client from configuration
    pub async fn new(config: ExtensionConfig) -> Result<Self> {
        let client = match config.transport {
            TransportType::Stdio => {
                Self::create_stdio_client(&config).await?
            }
            TransportType::Sse => {
                Self::create_sse_client(&config).await?
            }
            _ => {
                return Err(anyhow!("Unsupported transport for MCP client"));
            }
        };
        
        Ok(client)
    }
    
    /// Create stdio-based MCP client
    async fn create_stdio_client(config: &ExtensionConfig) -> Result<Self> {
        // Parse command and args from URI
        let (cmd, args) = parse_stdio_command(&config.uri)?;
        
        // Create stdio transport
        let transport = StdioTransport::new(&cmd, &args, config.env.clone())
            .map_err(|e| anyhow!("Failed to create stdio transport: {}", e))?;
        
        // Build MCP client
        let client = ClientBuilder::new(transport)
            .build()
            .await
            .map_err(|e| anyhow!("Failed to build MCP client: {}", e))?;
        
        // Initialize
        client.initialize().await
            .map_err(|e| anyhow!("Failed to initialize MCP client: {}", e))?;
        
        Ok(Self {
            client,
            config: config.clone(),
            _transport_handle: None,
        })
    }
    
    /// Create SSE-based MCP client
    async fn create_sse_client(config: &ExtensionConfig) -> Result<Self> {
        let transport = SseTransport::new(&config.uri)
            .map_err(|e| anyhow!("Failed to create SSE transport: {}", e))?;
        
        let client = ClientBuilder::new(transport)
            .build()
            .await
            .map_err(|e| anyhow!("Failed to build MCP client: {}", e))?;
        
        client.initialize().await
            .map_err(|e| anyhow!("Failed to initialize MCP client: {}", e))?;
        
        Ok(Self {
            client,
            config: config.clone(),
            _transport_handle: None,
        })
    }
    
    /// List available tools
    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        let response = self.client
            .list_tools(None, None)
            .await
            .map_err(|e| anyhow!("Failed to list tools: {}", e))?;
        
        Ok(response.tools)
    }
    
    /// Call a tool
    pub async fn call_tool(&self, request: ToolRequest) -> Result<CallToolResult> {
        let mcp_request = CallToolRequest {
            name: request.name,
            arguments: request.arguments,
        };
        
        let result = self.client
            .call_tool(mcp_request)
            .await
            .map_err(|e| anyhow!("Tool call failed: {}", e))?;
        
        Ok(result)
    }
    
    /// List available resources
    pub async fn list_resources(&self) -> Result<Vec<Resource>> {
        let response = self.client
            .list_resources(None, None)
            .await
            .map_err(|e| anyhow!("Failed to list resources: {}", e))?;
        
        Ok(response.resources)
    }
    
    /// Read a resource
    pub async fn read_resource(&self, uri: &str) -> Result<ResourceContent> {
        let response = self.client
            .read_resource(rmcp::model::ReadResourceRequest {
                uri: uri.to_string(),
            })
            .await
            .map_err(|e| anyhow!("Failed to read resource: {}", e))?;
        
        Ok(response.contents.into_iter().next())
    }
    
    /// Get a prompt
    pub async fn get_prompt(&self, name: &str) -> Result<GetPromptResult> {
        let result = self.client
            .get_prompt(rmcp::model::GetPromptRequest {
                name: name.to_string(),
                arguments: None,
            })
            .await
            .map_err(|e| anyhow!("Failed to get prompt: {}", e))?;
        
        Ok(result)
    }
    
    /// Ping to check connectivity
    pub async fn ping(&self) -> Result<()> {
        // Try to list tools as a ping
        self.client
            .list_tools(Some(1), None)
            .await
            .map_err(|e| anyhow!("Ping failed: {}", e))?;
        
        Ok(())
    }
    
    /// Graceful shutdown
    pub async fn shutdown(mut self) -> Result<()> {
        self.client.close().await
            .map_err(|e| anyhow!("Shutdown failed: {}", e))?;
        
        Ok(())
    }
}
```

---

## Transport Types

### ExtensionConfig

**File**: `crates/goose/src/agents/extension.rs`

```rust
/// Extension configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionConfig {
    /// Unique extension name
    pub name: String,
    
    /// Display name for UI
    pub display_name: String,
    
    /// Description of extension
    pub description: Option<String>,
    
    /// URI for connecting (command, URL, etc.)
    pub uri: String,
    
    /// Transport type
    pub transport: TransportType,
    
    /// Timeout for tool calls (seconds)
    pub timeout_secs: Option<u64>,
    
    /// Environment variables
    pub env: HashMap<String, String>,
    
    /// Additional configuration
    pub config: HashMap<String, Value>,
    
    /// Whether extension is enabled
    pub enabled: bool,
    
    /// Extension type
    pub extension_type: ExtensionType,
}

/// Transport types for extensions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransportType {
    /// Standard I/O (stdio) transport for local processes
    Stdio,
    
    /// Server-Sent Events (SSE) transport
    Sse,
    
    /// HTTP REST API transport
    Http,
    
    /// Built-in extension (in-process)
    Builtin,
}

/// Extension types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExtensionType {
    /// Standard extension
    Standard,
    
    /// Platform extension (built-in)
    Platform,
    
    /// Recipe-specific extension
    Recipe,
}
```

### Stdio Transport

```rust
/// Create stdio-based extension client
async fn create_stdio_client(&self, config: &ExtensionConfig) -> Result<ExtensionClient> {
    // Parse URI format: "npx -y @anthropic/mcp-github"
    // or: "uvx mcp-server-fetch"
    
    let parts: Vec<&str> = config.uri.split_whitespace().collect();
    if parts.is_empty() {
        return Err(anyhow!("Empty command"));
    }
    
    let cmd = parts[0];
    let args = &parts[1..];
    
    // Create MCP client with stdio transport
    let mcp_client = MCPClient::from_stdio(cmd, args, config.env.clone()).await?;
    
    Ok(ExtensionClient::Mcp(mcp_client))
}

/// Example stdio extensions:
/// - "npx -y @anthropic/mcp-github" (GitHub MCP)
/// - "uvx mcp-server-fetch" (Fetch MCP)
/// - "/usr/local/bin/my-mcp-server" (Custom binary)
```

### SSE Transport

```rust
/// Create SSE-based extension client
async fn create_sse_client(&self, config: &ExtensionConfig) -> Result<ExtensionClient> {
    // URI format: "https://mcp.example.com/events"
    
    let mcp_client = MCPClient::from_sse(&config.uri).await?;
    
    Ok(ExtensionClient::Mcp(mcp_client))
}
```

### HTTP Transport

```rust
/// Create HTTP-based extension client
async fn create_http_client(&self, config: &ExtensionConfig) -> Result<ExtensionClient> {
    let client = HttpExtensionClient::new(
        config.uri.clone(),
        config.config.clone(),
    );
    
    Ok(ExtensionClient::Http(client))
}
```

---

## Tool Discovery

### Tool Information

```rust
/// Tool metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    /// Tool name
    pub name: String,
    
    /// Tool description (shown to LLM)
    pub description: String,
    
    /// Input schema (JSON Schema)
    pub input_schema: Value,
    
    /// Output schema (optional)
    pub output_schema: Option<Value>,
    
    /// Whether tool needs confirmation
    pub needs_confirmation: bool,
    
    /// Tool category
    pub category: ToolCategory,
    
    /// Extension that provides this tool
    pub extension: String,
}

/// Tool categories for permission management
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    /// Read-only tools
    Read,
    
    /// Write/modify tools
    Write,
    
    /// System/shell tools
    System,
    
    /// Development tools
    Dev,
    
    /// Communication tools
    Comm,
    
    /// Platform tools (built-in)
    Platform,
}
```

### Tool Discovery Flow

```rust
impl ExtensionManager {
    /// Refresh and cache tools from all extensions
    pub async fn refresh_tools(&self) -> Result<Vec<Tool>> {
        let mut all_tools = Vec::new();
        
        let clients = self.clients.lock().await;
        
        for (ext_name, client) in clients.iter() {
            match client.list_tools().await {
                Ok(tools) => {
                    for tool in tools {
                        let tool_info = ToolInfo {
                            name: format!("{}__{}", ext_name, tool.name),
                            description: tool.description,
                            input_schema: tool.input_schema,
                            output_schema: None,
                            needs_confirmation: self.infer_needs_confirmation(&tool),
                            category: self.infer_category(&tool),
                            extension: ext_name.clone(),
                        };
                        all_tools.push(tool_info);
                    }
                }
                Err(e) => {
                    warn!("Failed to list tools from {}: {}", ext_name, e);
                }
            }
        }
        
        // Update cache
        self.update_tool_cache(all_tools.clone()).await;
        
        Ok(all_tools)
    }
    
    fn infer_needs_confirmation(&self, tool: &Tool) -> bool {
        // Infer based on tool name/description
        let name = tool.name.to_lowercase();
        let desc = tool.description.to_lowercase();
        
        // Write operations typically need confirmation
        let write_keywords = ["write", "edit", "delete", "create", "modify", "exec", "run"];
        
        for keyword in &write_keywords {
            if name.contains(keyword) || desc.contains(keyword) {
                return true;
            }
        }
        
        false
    }
    
    fn infer_category(&self, tool: &Tool) -> ToolCategory {
        let name = tool.name.to_lowercase();
        
        if name.contains("read") || name.contains("get") || name.contains("list") {
            ToolCategory::Read
        } else if name.contains("write") || name.contains("edit") || name.contains("delete") {
            ToolCategory::Write
        } else if name.contains("exec") || name.contains("run") || name.contains("shell") {
            ToolCategory::System
        } else if name.contains("git") || name.contains("build") || name.contains("test") {
            ToolCategory::Dev
        } else {
            ToolCategory::Comm
        }
    }
}
```

---

## Tool Execution

### Execution Flow

```rust
/// Execute a tool with full lifecycle management
pub async fn execute_tool(
    &self,
    request: ToolRequest,
    context: ToolExecutionContext,
) -> Result<ToolExecutionResult> {
    let start_time = Instant::now();
    
    // 1. Parse and validate
    let (ext_name, tool_name) = parse_tool_name(&request.name)?;
    
    // 2. Permission check
    let permission = self.permission_manager
        .check_permission(&ext_name, &tool_name, &request.arguments)
        .await?;
    
    match permission {
        PermissionResult::Denied => {
            return Ok(ToolExecutionResult {
                status: ToolExecutionStatus::Denied,
                result: None,
                error: Some("Permission denied".to_string()),
                duration_ms: start_time.elapsed().as_millis() as u64,
            });
        }
        PermissionResult::NeedsConfirmation => {
            // Return confirmation required
            return Ok(ToolExecutionResult {
                status: ToolExecutionStatus::NeedsConfirmation,
                result: None,
                error: None,
                duration_ms: 0,
                confirmation_data: Some(ConfirmationData {
                    tool_name: request.name.clone(),
                    arguments: request.arguments.clone(),
                }),
            });
        }
        PermissionResult::Allowed => {
            // Continue to execution
        }
    }
    
    // 3. Find extension
    let clients = self.clients.lock().await;
    let client = clients.get(&ext_name)
        .ok_or_else(|| anyhow!("Extension not found: {}", ext_name))?;
    
    // 4. Execute with timeout
    let timeout = Duration::from_secs(
        request.timeout_secs.unwrap_or(60)
    );
    
    let result = tokio::time::timeout(
        timeout,
        client.call_tool(ToolRequest {
            name: tool_name.to_string(),
            ..request
        })
    ).await;
    
    let duration_ms = start_time.elapsed().as_millis() as u64;
    
    // 5. Handle result
    match result {
        Ok(Ok(call_result)) => {
            // Success
            Ok(ToolExecutionResult {
                status: ToolExecutionStatus::Success,
                result: Some(call_result),
                error: None,
                duration_ms,
            })
        }
        Ok(Err(e)) => {
            // Tool execution error
            Ok(ToolExecutionResult {
                status: ToolExecutionStatus::Error,
                result: None,
                error: Some(e.to_string()),
                duration_ms,
            })
        }
        Err(_) => {
            // Timeout
            Ok(ToolExecutionResult {
                status: ToolExecutionStatus::Timeout,
                result: None,
                error: Some(format!("Timeout after {:?}", timeout)),
                duration_ms: timeout.as_millis() as u64,
            })
        }
    }
}
```

---

## Resource Management

### MCP Resources

```rust
/// Resource from MCP server
#[derive(Debug, Clone)]
pub struct ResourceInfo {
    /// Resource URI
    pub uri: String,
    
    /// Resource name
    pub name: String,
    
    /// MIME type
    pub mime_type: Option<String>,
    
    /// Description
    pub description: Option<String>,
    
    /// Size in bytes (if known)
    pub size: Option<usize>,
    
    /// Extension providing this resource
    pub extension: String,
}

impl ExtensionManager {
    /// List all available resources from all extensions
    pub async fn list_resources(&self) -> Result<Vec<ResourceInfo>> {
        let mut all_resources = Vec::new();
        let clients = self.clients.lock().await;
        
        for (ext_name, client) in clients.iter() {
            if let ExtensionClient::Mcp(mcp) = client {
                match mcp.list_resources().await {
                    Ok(resources) => {
                        for res in resources {
                            all_resources.push(ResourceInfo {
                                uri: res.uri,
                                name: res.name,
                                mime_type: res.mime_type,
                                description: res.description,
                                size: None,
                                extension: ext_name.clone(),
                            });
                        }
                    }
                    Err(e) => {
                        debug!("Extension {} doesn't support resources: {}", ext_name, e);
                    }
                }
            }
        }
        
        Ok(all_resources)
    }
    
    /// Read a resource by URI
    pub async fn read_resource(&self, uri: &str) -> Result<ResourceContent> {
        // Find extension that owns this resource
        let clients = self.clients.lock().await;
        
        for (ext_name, client) in clients.iter() {
            if let ExtensionClient::Mcp(mcp) = client {
                // Try to read - if it works, this is the right extension
                match mcp.read_resource(uri).await {
                    Ok(content) => return Ok(content),
                    Err(_) => continue,
                }
            }
        }
        
        Err(anyhow!("Resource not found: {}", uri))
    }
}
```

---

## Prompt Management

### MCP Prompts

```rust
/// Prompt from MCP server
#[derive(Debug, Clone)]
pub struct PromptInfo {
    /// Prompt name
    pub name: String,
    
    /// Prompt description
    pub description: Option<String>,
    
    /// Required arguments
    pub arguments: Vec<PromptArgument>,
    
    /// Extension providing this prompt
    pub extension: String,
}

/// Prompt argument
#[derive(Debug, Clone)]
pub struct PromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}

impl ExtensionManager {
    /// List all available prompts
    pub async fn list_prompts(&self) -> Result<Vec<PromptInfo>> {
        let mut all_prompts = Vec::new();
        let clients = self.clients.lock().await;
        
        for (ext_name, client) in clients.iter() {
            if let ExtensionClient::Mcp(mcp) = client {
                // Note: list_prompts may not be in all MCP implementations
                // This is optional functionality
            }
        }
        
        Ok(all_prompts)
    }
    
    /// Get a prompt
    pub async fn get_prompt(
        &self,
        ext_name: &str,
        prompt_name: &str,
    ) -> Result<GetPromptResult> {
        let clients = self.clients.lock().await;
        
        let client = clients.get(ext_name)
            .ok_or_else(|| anyhow!("Extension not found: {}", ext_name))?;
        
        if let ExtensionClient::Mcp(mcp) = client {
            let result = mcp.get_prompt(prompt_name).await?;
            Ok(result)
        } else {
            Err(anyhow!("Extension doesn't support prompts"))
        }
    }
}
```

---

## Security

### Extension Malware Check

**File**: `crates/goose/src/agents/extension_malware_check.rs` (16024 bytes)

```rust
/// Checks extensions for potential security risks
pub struct MalwareChecker {
    /// Known malicious signatures
    malicious_signatures: Vec<String>,
    
    /// Suspicious patterns
    suspicious_patterns: Vec<Regex>,
    
    /// Trusted publishers
    trusted_publishers: Vec<String>,
}

impl MalwareChecker {
    /// Check an extension for risks
    pub async fn check_extension(&self, config: &ExtensionConfig) -> Option<RiskLevel> {
        // Check command against known bad signatures
        if self.is_known_malicious(&config.uri) {
            return Some(RiskLevel::Critical);
        }
        
        // Check for suspicious patterns
        let risk_score = self.calculate_risk_score(config);
        
        if risk_score > 80 {
            Some(RiskLevel::High)
        } else if risk_score > 50 {
            Some(RiskLevel::Medium)
        } else if risk_score > 20 {
            Some(RiskLevel::Low)
        } else {
            None
        }
    }
    
    fn is_known_malicious(&self, uri: &str) -> bool {
        let lower = uri.to_lowercase();
        
        for signature in &self.malicious_signatures {
            if lower.contains(signature) {
                return true;
            }
        }
        
        false
    }
    
    fn calculate_risk_score(&self, config: &ExtensionConfig) -> u32 {
        let mut score = 0;
        
        // Direct binary execution is higher risk
        if config.transport == TransportType::Stdio {
            if !config.uri.starts_with("npx") && 
               !config.uri.starts_with("uvx") &&
               !config.uri.starts_with("npm") {
                score += 30; // Direct binary execution
            }
        }
        
        // Network access
        if config.uri.contains("http://") || 
           config.uri.contains("https://") {
            score += 10;
        }
        
        // Suspicious environment variables
        for (key, _) in &config.env {
            if self.is_suspicious_env_var(key) {
                score += 20;
            }
        }
        
        // Unknown publisher
        if !self.is_trusted_publisher(&config.uri) {
            score += 15;
        }
        
        score
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}
```

### Tool Confirmation

```rust
/// Router for tool confirmation decisions
pub struct ToolConfirmationRouter {
    /// Auto-allow rules
    auto_allow: Vec<ConfirmationRule>,
    
    /// Require confirmation rules
    require_confirm: Vec<ConfirmationRule>,
    
    /// Never allow list
    deny_list: HashSet<String>,
}

impl ToolConfirmationRouter {
    /// Check if a tool call needs confirmation
    pub fn needs_confirmation(&self, tool_name: &str, args: &Value) -> bool {
        // Check deny list first
        if self.deny_list.contains(tool_name) {
            return true; // Will be denied
        }
        
        // Check auto-allow rules
        for rule in &self.auto_allow {
            if rule.matches(tool_name, args) {
                return false;
            }
        }
        
        // Check require-confirm rules
        for rule in &self.require_confirm {
            if rule.matches(tool_name, args) {
                return true;
            }
        }
        
        // Default: require confirmation for write operations
        self.is_write_operation(tool_name)
    }
}
```

---

## Summary

Goose's MCP integration provides:

1. **Extension Management**: Dynamic loading/unloading of MCP servers
2. **Multiple Transports**: stdio, SSE, HTTP, and built-in
3. **Tool Discovery**: Automatic tool listing and caching
4. **Resource Access**: Read resources from MCP servers
5. **Security**: Malware checking and permission management
6. **Error Handling**: Timeout and retry support
7. **Performance**: Tool caching and connection pooling

Key files:
- `crates/goose/src/agents/extension_manager.rs` - Extension management (81KB)
- `crates/goose/src/agents/mcp_client.rs` - MCP client (34KB)
- `crates/goose/src/agents/extension.rs` - Extension types
- `crates/goose/src/agents/extension_malware_check.rs` - Security checks
- `crates/goose-mcp/` - MCP protocol crate (40 files)

Integration with rmcp crate (Rust MCP implementation) provides protocol-level MCP support.
