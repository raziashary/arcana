# Goose Source - Complete Architecture Dissection

> **Project**: `goose-main`  
> **Type**: Rust CLI Application (Cargo Workspace)  
> **Crates**: 9 workspace crates  
> **Total Files**: ~1,500+ source files  
> **Generated**: 2025

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Workspace Architecture](#workspace-architecture)
3. [Core Crate (`goose`)](#core-crate-goose)
4. [Agent System](#agent-system)
5. [Provider System](#provider-system)
6. [Extension/Tool System](#extensiontool-system)
7. [Configuration System](#configuration-system)
8. [CLI Crate](#cli-crate)
9. [MCP Crate](#mcp-crate)
10. [Server Crate](#server-crate)
11. [Key Dependencies](#key-dependencies)
12. [Architecture Decisions](#architecture-decisions)

---

## Executive Summary

### Project Identity

**Goose** is a sophisticated, production-grade agentic AI assistant implemented in Rust. It serves as a development companion that can:

- Execute shell commands with safety controls
- Read and modify files
- Interface with external tools via MCP (Model Context Protocol)
- Support 50+ LLM providers
- Run in CLI or server mode
- Manage development sessions with context awareness

### Architecture Philosophy

| Principle | Implementation |
|-----------|---------------|
| **Modularity** | 9 separate crates with clear boundaries |
| **Extensibility** | Provider trait for any LLM, Extension trait for any tool |
| **Safety** | Permission system, security inspectors, tool confirmation |
| **Performance** | Async Rust with Tokio, streaming responses |
| **Standards** | MCP protocol for tool interoperability |

---

## Workspace Architecture

### Workspace Manifest

**File**: `Cargo.toml`

```toml
[workspace]
members = [
    "crates/goose",
    "crates/goose-cli",
    "crates/goose-server",
    "crates/goose-mcp",
    "crates/goose-acp",
    "crates/goose-acp-macros",
    "crates/goose-sdk",
    "crates/goose-test",
    "crates/goose-test-support",
]

[workspace.package]
version = "1.0.0"
edition = "2021"
authors = ["Block <ai-eng@block.xyz>"]
license = "Apache-2.0"
```

### Crate Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │ goose-cli   │  │ goose-server│  │ goose-sdk           │ │
│  │ 58 files    │  │ 46 files    │  │ 4 files             │ │
│  │ CLI app     │  │ HTTP API    │  │ Library SDK         │ │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘ │
│         │                │                    │            │
└─────────┼────────────────┼────────────────────┼────────────┘
          │                │                    │
          └────────────────┴────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                      Core Library                            │
│                     goose (287 files)                        │
│                                                              │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐              │
│  │   agents   │ │  providers │ │   config   │              │
│  │   25 files │ │  50+ files │ │  12 files  │              │
│  └────────────┘ └────────────┘ └────────────┘              │
│                                                              │
└──────────────────────────┬───────────────────────────────────┘
                           │
          ┌────────────────┼────────────────┐
          │                │                │
          ▼                ▼                ▼
┌─────────────────┐ ┌────────────┐ ┌────────────────────┐
│  goose-mcp      │ │ goose-acp  │ │ External deps      │
│  40 files       │ │ 36 files   │ │ rmcp, tokio, etc │
│  MCP protocol   │ │ ACP protocol│ │                  │
└─────────────────┘ └────────────┘ └────────────────────┘
```

---

## Core Crate (`goose`)

### Module Structure

**File**: `crates/goose/src/lib.rs`

```rust
pub mod acp;
pub mod action_required_manager;
pub mod agents;           // Agent implementation
pub mod builtin_extension;
pub mod config;           // Configuration system
pub mod context_mgmt;     // Context window management
pub mod conversation;     // Message/conversation handling
pub mod dictation;
pub mod doctor;
pub mod download_manager;
pub mod execution;
pub mod gateway;
pub mod goose_apps;
pub mod hints;
pub mod logging;
pub mod mcp_utils;
pub mod model;
pub mod oauth;
pub mod permission;       // Permission system
pub mod prompt_template;
pub mod providers;        // LLM providers (50+)
pub mod recipe;
pub mod recipe_deeplink;
pub mod scheduler;
pub mod security;         // Security inspectors
pub mod session;
pub mod session_context;
pub mod slash_commands;
pub mod subprocess;
pub mod token_counter;
pub mod tool_inspection;
pub mod tool_monitor;
pub mod tracing;
pub mod utils;
```

### Key Modules by Size

| Module | Files | Largest File | Purpose |
|--------|-------|--------------|---------|
| `agents/` | 25 | `agent.rs` (103KB) | Agent loop, extensions |
| `providers/` | 50+ | `codex.rs` (48KB) | LLM provider implementations |
| `config/` | 12 | `base.rs` (68KB) | Configuration management |
| `security/` | 8 | `security_inspector.rs` | Security checks |
| `conversation/` | 6 | `message.rs` | Message types |
| `session/` | 5 | `mod.rs` | Session management |

---

## Agent System

### Agent Structure

**File**: `crates/goose/src/agents/agent.rs` (2471 lines, 103KB)

```rust
/// The main goose Agent
pub struct Agent {
    /// LLM provider
    provider: SharedProvider,
    
    /// Agent configuration
    config: AgentConfig,
    
    /// Current operating mode (chat, agent, etc.)
    current_goose_mode: Mutex<GooseMode>,
    
    /// Extension manager for tools
    extension_manager: Arc<ExtensionManager>,
    
    /// Final output tool
    final_output_tool: Arc<Mutex<Option<FinalOutputTool>>>,
    
    /// Frontend tools (UI integration)
    frontend_tools: Mutex<HashMap<String, FrontendTool>>,
    
    /// Frontend instructions
    frontend_instructions: Mutex<Option<String>>,
    
    /// Prompt manager for system prompts
    prompt_manager: Mutex<PromptManager>,
    
    /// Tool confirmation routing
    tool_confirmation_router: ToolConfirmationRouter,
    
    /// Tool result channel
    tool_result_tx: mpsc::Sender<(String, ToolResult<CallToolResult>)>,
    
    /// Action required manager
    action_required_manager: Arc<ActionRequiredManager>,
    
    /// Retry manager
    retry_manager: RetryManager,
    
    /// Tool inspection manager
    tool_inspection_manager: ToolInspectionManager,
    
    /// Repetition inspector
    repetition_inspector: Arc<RepetitionInspector>,
    
    /// Security inspectors
    security_inspectors: Vec<Box<dyn SecurityInspector>>,
    
    /// Adversary inspector
    adversary_inspector: Arc<AdversaryInspector>,
    
    /// Egress inspector
    egress_inspector: Arc<EgressInspector>,
    
    /// Permission inspector
    permission_inspector: Arc<PermissionInspector>,
    
    /// Container for isolation
    container: Arc<Container>,
}
```

### Agent Configuration

```rust
#[derive(Clone)]
pub struct AgentConfig {
    /// Session management
    pub session_manager: Arc<SessionManager>,
    
    /// Permission management
    pub permission_manager: Arc<PermissionManager>,
    
    /// Optional scheduler service
    pub scheduler_service: Option<Arc<dyn SchedulerTrait>>,
    
    /// Operating mode
    pub goose_mode: GooseMode,
    
    /// Disable automatic session naming
    pub disable_session_naming: bool,
    
    /// Platform (CLI vs Desktop)
    pub goose_platform: GoosePlatform,
}

#[derive(Clone, Debug)]
pub enum GooseMode {
    Chat,      // Chat-only mode
    Agent,     // Full agent mode with tools
    Plan,      // Plan mode (step-by-step)
    Auto,      // Automatic mode
}
```

### Agent Reply Flow

```rust
impl Agent {
    /// Main entry point for agent responses
    pub async fn reply(
        &self,
        messages: Vec<Message>,
    ) -> Result<BoxStream<'_, Result<Message, anyhow::Error>>> {
        // 1. Prepare reply context
        let context = self.prepare_reply_context(messages).await?;
        
        // 2. Get available tools
        let tools = self.get_prefixed_tools().await?;
        
        // 3. Start agent loop
        let stream = self.run_agent_loop(context, tools).await?;
        
        Ok(stream)
    }
    
    /// Core agent loop
    async fn run_agent_loop(
        &self,
        context: ReplyContext,
        tools: Vec<Tool>,
    ) -> Result<BoxStream<'_, Result<Message, anyhow::Error>>> {
        // Streaming implementation that:
        // - Sends messages to provider
        // - Handles tool calls
        // - Manages context compaction
        // - Applies security checks
        // - Returns streaming responses
    }
}
```

### Agent Events

```rust
pub enum AgentEvent {
    /// Text content from LLM
    Text(String),
    
    /// Tool request detected
    ToolRequest(ToolRequest),
    
    /// Tool result received
    ToolResult(ToolResult<CallToolResult>),
    
    /// Tool confirmation needed
    ToolConfirmation {
        tool_name: String,
        request: ToolRequest,
    },
    
    /// Thinking/reasoning content
    Thinking(String),
    
    /// Error occurred
    Error(String),
    
    /// Session updated
    SessionUpdate(Session),
    
    /// Conversation compaction starting
    CompactionStarting,
    
    /// Conversation compaction complete
    CompactionComplete,
    
    /// Action required
    ActionRequired(ActionRequiredData),
    
    /// Task started
    TaskStarted { task_id: String, description: String },
    
    /// Task completed
    TaskCompleted { task_id: String, result: String },
}
```

---

## Provider System

### Provider Trait

**File**: `crates/goose/src/providers/base.rs` (1161 lines, 38KB)

```rust
/// Core trait for LLM providers
#[async_trait]
pub trait Provider: Send + Sync {
    /// Get provider name
    fn name(&self) -> String;
    
    /// Get model configuration
    fn get_model_config(&self) -> &ModelConfig;
    
    /// Non-streaming completion
    async fn complete(
        &self,
        model_config: &ModelConfig,
        system: Option<String>,
        messages: Vec<Message>,
        tools: Vec<Tool>,
    ) -> Result<ProviderResponse, ProviderError>;
    
    /// Streaming completion
    async fn stream_complete(
        &self,
        model_config: &ModelConfig,
        system: Option<String>,
        messages: Vec<Message>,
        tools: Vec<Tool>,
    ) -> Result<BoxStream<'_, Result<ProviderStreamChunk, ProviderError>>, ProviderError>;
    
    /// Provider-specific tool shimming (if needed)
    fn supports_tool_shim(&self) -> bool {
        false
    }
    
    /// Get usage estimate
    fn estimate_usage(
        &self,
        input_tokens: usize,
        output_tokens: usize,
    ) -> ProviderUsage;
}
```

### Model Info

```rust
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct ModelInfo {
    /// Model name
    pub name: String,
    
    /// Maximum context length
    pub context_limit: usize,
    
    /// Input token cost (USD per token)
    pub input_token_cost: Option<f64>,
    
    /// Output token cost (USD per token)
    pub output_token_cost: Option<f64>,
    
    /// Currency (default: "$")
    pub currency: Option<String>,
    
    /// Supports cache control
    pub supports_cache_control: Option<bool>,
}
```

### Provider Registry

**File**: `crates/goose/src/providers/provider_registry.rs`

```rust
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn ProviderFactory>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            providers: HashMap::new(),
        };
        
        // Register built-in providers
        registry.register::<AnthropicProvider>("anthropic");
        registry.register::<OpenAiProvider>("openai");
        registry.register::<AzureProvider>("azure");
        registry.register::<OllamaProvider>("ollama");
        registry.register::<DatabricksProvider>("databricks");
        registry.register::<BedrockProvider>("bedrock");
        registry.register::<VertexAIProvider>("gcp_vertex_ai");
        // ... 50+ more providers
        
        registry
    }
    
    pub fn create_provider(&self, name: &str, config: Value) -> Result<Box<dyn Provider>> {
        let factory = self.providers
            .get(name)
            .ok_or_else(|| anyhow!("Unknown provider: {}", name))?;
        
        factory.create(config)
    }
}
```

### Supported Providers (50+)

| Provider | File | Description |
|----------|------|-------------|
| **Anthropic** | `anthropic.rs` | Claude models |
| **OpenAI** | `openai.rs` | GPT-4, GPT-3.5 |
| **OpenAI Codex** | `codex.rs` | Codex models (48KB) |
| **ChatGPT Codex** | `chatgpt_codex.rs` | ChatGPT Codex (43KB) |
| **Azure** | `azure.rs` | Azure OpenAI |
| **Azure Auth** | `azureauth.rs` | Azure authentication |
| **Databricks** | `databricks.rs` | Databricks models |
| **Bedrock** | `bedrock.rs` | AWS Bedrock |
| **Ollama** | `ollama.rs` | Local inference |
| **Vertex AI** | `gcpvertexai.rs` | Google Cloud |
| **Gemini** | `google.rs` | Google Gemini |
| **Gemini CLI** | `gemini_cli.rs` | Gemini CLI |
| **GitHub Copilot** | `githubcopilot.rs` | Copilot |
| **LiteLLM** | `litellm.rs` | LiteLLM proxy |
| **OpenRouter** | `openrouter.rs` | OpenRouter |
| **NanoGPT** | `nanogpt.rs` | NanoGPT |
| **Tetrate** | `tetrate.rs` | Tetrate |
| **Venice** | `venice.rs` | Venice |
| **xAI** | `xai.rs` | xAI/Grok |
| **Snowflake** | `snowflake.rs` | Snowflake |
| **Claude Code** | `claude_code.rs` | Claude Code provider (57KB) |
| **Cursor** | `cursor_agent.rs` | Cursor |
| **Local Inference** | `local_inference.rs` | Local models |
| **Test Provider** | `testprovider.rs` | Testing |
| **And more...** | | 50+ total |

---

## Extension/Tool System

### Extension Manager

**File**: `crates/goose/src/agents/extension_manager.rs` (83468 bytes)

```rust
/// Manages extensions (tool providers)
pub struct ExtensionManager {
    /// Active extension clients
    clients: HashMap<String, ExtensionClient>,
    
    /// Cached tools from all extensions
    tool_cache: Vec<Tool>,
    
    /// Cache timestamp
    tool_cache_time: Option<Instant>,
    
    /// Extension configurations
    configs: HashMap<String, ExtensionConfig>,
    
    /// Platform extension manager
    platform_extension_manager: PlatformExtensionManager,
}

impl ExtensionManager {
    /// Add a new extension
    pub async fn add_extension(&mut self, config: ExtensionConfig) -> Result<ExtensionResult>;
    
    /// Remove an extension
    pub async fn remove_extension(&mut self, name: &str) -> Result<()>;
    
    /// List all available tools
    pub async fn list_tools(&mut self) -> Result<Vec<Tool>>;
    
    /// Call a tool
    pub async fn call_tool(&self, request: ToolRequest) -> Result<ToolResult<CallToolResult>>;
    
    /// Get tools for a specific extension
    pub async fn get_extension_tools(&self, name: &str) -> Result<Vec<Tool>>;
}
```

### Extension Configuration

```rust
pub struct ExtensionConfig {
    /// Unique name
    pub name: String,
    
    /// Display name
    pub display_name: String,
    
    /// Extension URI/endpoint
    pub uri: String,
    
    /// Transport type
    pub transport: TransportType,
    
    /// Additional configuration
    pub config: HashMap<String, Value>,
    
    /// Timeout for tool calls
    pub timeout_secs: u64,
    
    /// Environment variables
    pub env: HashMap<String, String>,
}

pub enum TransportType {
    /// MCP stdio transport
    Stdio,
    
    /// MCP SSE transport
    Sse,
    
    /// HTTP REST API
    Http,
    
    /// Built-in extension
    Builtin,
}
```

### Tool Information

```rust
pub struct ToolInfo {
    /// Tool name
    pub name: String,
    
    /// Tool description (for LLM)
    pub description: String,
    
    /// Input schema (JSON Schema)
    pub input_schema: Value,
    
    /// Output schema
    pub output_schema: Option<Value>,
    
    /// Whether tool needs confirmation
    pub needs_confirmation: bool,
    
    /// Tool category
    pub category: ToolCategory,
}

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
    
    /// Platform tools
    Platform,
}
```

---

## Configuration System

### Config Structure

**File**: `crates/goose/src/config/base.rs` (69901 bytes)

```rust
/// Main configuration structure
pub struct Config {
    /// Active provider
    pub current_provider: String,
    
    /// Provider configurations
    pub providers: HashMap<String, ProviderConfig>,
    
    /// Extension configurations
    pub extensions: Vec<ExtensionConfig>,
    
    /// Goose mode
    pub goose_mode: GooseMode,
    
    /// Permission settings
    pub permission: PermissionSettings,
    
    /// UI settings
    pub ui: UiSettings,
    
    /// Session settings
    pub session: SessionSettings,
    
    /// Security settings
    pub security: SecuritySettings,
    
    /// Logging settings
    pub logging: LoggingSettings,
    
    /// Telemetry settings
    pub telemetry: TelemetrySettings,
}

impl Config {
    /// Load configuration from disk
    pub fn load() -> Result<Self, ConfigError>;
    
    /// Save configuration to disk
    pub fn save(&self) -> Result<(), ConfigError>;
    
    /// Get provider configuration
    pub fn get_provider_config(&self, name: &str) -> Option<&ProviderConfig>;
    
    /// Set active provider
    pub fn set_provider(&mut self, name: String);
    
    /// Add extension
    pub fn add_extension(&mut self, config: ExtensionConfig);
    
    /// Remove extension
    pub fn remove_extension(&mut self, name: &str);
}
```

### Permission Settings

```rust
pub struct PermissionSettings {
    /// Global permission mode
    pub mode: PermissionMode,
    
    /// Always allow these tools
    pub always_allow: Vec<String>,
    
    /// Always ask for these tools
    pub always_ask: Vec<String>,
    
    /// Never allow these tools
    pub never_allow: Vec<String>,
    
    /// Allow list patterns
    pub allow_patterns: Vec<String>,
    
    /// Deny list patterns
    pub deny_patterns: Vec<String>,
}

pub enum PermissionMode {
    /// Always ask for confirmation
    Ask,
    
    /// Use rules to decide
    Smart,
    
    /// Allow all (dangerous)
    AllowAll,
    
    /// Deny all
    DenyAll,
}
```

---

## CLI Crate

### CLI Structure

**File**: `crates/goose-cli/src/cli.rs` (63702 bytes)

```rust
/// CLI entry point
#[derive(Parser)]
#[command(name = "goose")]
#[command(about = "An AI agent for software development")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start an interactive session
    Session {
        /// Resume existing session
        #[arg(long)]
        resume: bool,
        
        /// Session name
        name: Option<String>,
    },
    
    /// Run a recipe
    Run {
        /// Recipe name or path
        recipe: String,
        
        /// Recipe arguments
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    
    /// Configure goose
    Configure {
        #[command(subcommand)]
        command: ConfigureCommands,
    },
    
    /// Manage extensions
    Extension {
        #[command(subcommand)]
        command: ExtensionCommands,
    },
    
    /// Manage providers
    Provider {
        #[command(subcommand)]
        command: ProviderCommands,
    },
    
    /// Manage sessions
    SessionManager {
        #[command(subcommand)]
        command: SessionCommands,
    },
    
    /// Run a command
    RunCommand {
        /// Command to run
        command: Vec<String>,
    },
    
    /// Check system health
    Doctor,
    
    /// Update goose
    Update,
    
    /// Display version
    Version,
}
```

### Session Commands

```rust
#[derive(Subcommand)]
pub enum SessionCommands {
    /// List all sessions
    List,
    
    /// Delete a session
    Delete { name: String },
    
    /// Rename a session
    Rename { old_name: String, new_name: String },
    
    /// Archive a session
    Archive { name: String },
    
    /// Show session info
    Info { name: String },
}
```

---

## MCP Crate

### MCP Integration

**File**: `crates/goose/src/agents/mcp_client.rs` (33828 bytes)

```rust
/// MCP client wrapper
pub struct MCPClient {
    /// Underlying rmcp client
    client: rmcp::Client,
    
    /// Extension configuration
    config: ExtensionConfig,
    
    /// Transport handle
    transport_handle: Option<JoinHandle<()>>,
}

impl MCPClient {
    /// Create new MCP client
    pub async fn new(config: ExtensionConfig) -> Result<Self>;
    
    /// Initialize connection
    pub async fn initialize(&mut self) -> Result<()>;
    
    /// List available tools
    pub async fn list_tools(&self) -> Result<Vec<Tool>>;
    
    /// Call a tool
    pub async fn call_tool(&self, request: CallToolRequest) -> Result<CallToolResult>;
    
    /// Get a prompt
    pub async fn get_prompt(&self, name: &str) -> Result<GetPromptResult>;
    
    /// List resources
    pub async fn list_resources(&self) -> Result<Vec<Resource>>;
    
    /// Read a resource
    pub async fn read_resource(&self, uri: &str) -> Result<ResourceContent>;
    
    /// Shutdown connection
    pub async fn shutdown(self) -> Result<()>;
}
```

---

## Server Crate

### HTTP Server

**File**: `crates/goose-server/src/main.rs`

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    init_logging();
    
    // Create server
    let app = create_app().await?;
    
    // Start server
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn create_app() -> Result<Router> {
    let app = Router::new()
        // Session endpoints
        .route("/sessions", get(list_sessions).post(create_session))
        .route("/sessions/:id", get(get_session).delete(delete_session))
        .route("/sessions/:id/messages", post(send_message))
        .route("/sessions/:id/stream", get(stream_response))
        
        // Extension endpoints
        .route("/extensions", get(list_extensions).post(add_extension))
        .route("/extensions/:name", delete(remove_extension))
        
        // Provider endpoints
        .route("/providers", get(list_providers))
        .route("/providers/:name/models", get(list_models))
        
        // Configuration
        .route("/config", get(get_config).put(update_config))
        
        // Health
        .route("/health", get(health_check));
    
    Ok(app)
}
```

---

## Key Dependencies

### Cargo.toml

```toml
[dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }

# HTTP client
reqwest = { version = "0.12", features = ["json", "stream"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
anyhow = "1.0"
thiserror = "1.0"

# Async utilities
futures = "0.3"
async-trait = "0.1"

# CLI
clap = { version = "4", features = ["derive"] }

# MCP
rmcp = { version = "0.8", features = ["client"] }

# Tracing
tracing = "0.1"
tracing-subscriber = "0.3"

# Authentication
oauth2 = "4.0"
keyring = "2.0"

# HTTP server
axum = "0.7"
tower = "0.5"

# OpenAPI (for server)
utoipa = "4.0"

# Testing
tokio-test = "0.4"
mockall = "0.12"
```

---

## Architecture Decisions

### 1. Workspace Structure

**Decision**: Split into 9 crates

**Rationale**:
- Clear separation of concerns
- CLI, server, and SDK can evolve independently
- MCP and ACP as standalone protocols
- Testing utilities separate

### 2. Provider Trait

**Decision**: Trait-based provider system

**Rationale**:
- Easy to add new LLM providers
- Consistent interface across 50+ providers
- Supports both streaming and non-streaming
- Provider-specific optimizations possible

### 3. MCP for Tools

**Decision**: Use MCP (Model Context Protocol) for tool integration

**Rationale**:
- Industry standard protocol
- Decouples tools from core
- External tools can be added dynamically
- Transport-agnostic (stdio, SSE, HTTP)

### 4. Async Rust

**Decision**: Use Tokio async runtime

**Rationale**:
- Efficient concurrent I/O
- Streaming responses from LLMs
- Multiple tool calls in parallel
- Non-blocking user interface

### 5. Streaming Architecture

**Decision**: Stream-based responses throughout

**Rationale**:
- Real-time LLM output
- Progressive tool results
- Better user experience
- Lower memory footprint

### 6. Permission System

**Decision**: Multi-layer permission checking

**Rationale**:
- Security inspectors for all tool calls
- User confirmation for dangerous operations
- Configurable permission modes
- Pattern-based allow/deny lists

### 7. Context Compaction

**Decision**: Automatic conversation compaction

**Rationale**:
- Handle long conversations
- Stay within context limits
- Summarize older messages
- Maintain conversation flow

---

## Summary

Goose is a sophisticated Rust implementation of an agentic AI assistant with:

- **9 workspace crates** for modularity
- **50+ LLM providers** via trait system
- **MCP integration** for external tools
- **Streaming architecture** for real-time responses
- **Security-first** with permission system
- **Production-ready** with session management

**Key Files**:
- `crates/goose/src/agents/agent.rs` - Core agent (103KB)
- `crates/goose/src/providers/base.rs` - Provider trait (38KB)
- `crates/goose/src/config/base.rs` - Configuration (68KB)
- `crates/goose/src/agents/extension_manager.rs` - Tools (81KB)
- `crates/goose-cli/src/cli.rs` - CLI (64KB)

**Documentation**: See focused documents for deep dives:
- [01-agent-system-deep-dive.md](01-agent-system-deep-dive.md)
- [02-provider-system.md](02-provider-system.md)
- [03-mcp-integration.md](03-mcp-integration.md)
