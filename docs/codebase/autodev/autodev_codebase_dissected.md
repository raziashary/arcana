# AutoDev Codebase Architecture Dissection

> **Generated**: Comprehensive architectural documentation of the AutoDev codebase  
> **Project**: AutoDev - AI-powered development assistant (Tauri + React/TypeScript)  
> **Last Updated**: 2025

## Table of Contents

1. [Overview](#overview)
2. [Architecture Patterns](#architecture-patterns)
3. [Backend (Rust/Tauri)](#backend-rusttauri)
   - [Core Files](#core-files)
   - [Agent Module](#agent-module)
   - [Commands Module](#commands-module)
   - [Provider Module](#provider-module)
   - [Session Module](#session-module)
   - [State Module](#state-module)
   - [Vault Module](#vault-module)
   - [Cost Module](#cost-module)
4. [Frontend (React/TypeScript)](#frontend-reacttypescript)
   - [Core Files](#frontend-core-files)
   - [Stores](#stores)
   - [Tabs](#tabs)
   - [Components](#components)
   - [Utils](#utils)
5. [Data Flow](#data-flow)
6. [Key Relationships](#key-relationships)

---

## Overview

AutoDev is a **Personal Assistant (PA) mode AI coding tool** built with:

- **Backend**: Rust with Tauri framework (desktop app shell + native capabilities)
- **Frontend**: React + TypeScript + Tailwind CSS
- **State Management**: Zustand stores
- **Communication**: Tauri events (WebSocket-like) + IPC commands

### Core Philosophy

The application follows a **dual-mode architecture**:

1. **PA Mode (Rust-native)**: Direct LLM API calls with built-in tools for low-latency interactive coding
2. **Worker Mode (Python sidecar)**: Delegates to core-orchestrator for multi-agent coordination

---

## Architecture Patterns

### 1. Event-Driven Communication

Frontend and backend communicate via Tauri events:

| Event Name | Direction | Purpose |
|------------|-----------|---------|
| `agent:token` | Backend → Frontend | Streaming LLM tokens |
| `agent:tool-start` | Backend → Frontend | Tool execution started |
| `agent:tool-end` | Backend → Frontend | Tool execution completed |
| `agent:permission-request` | Backend → Frontend | User approval needed |
| `agent:done` | Backend → Frontend | Agent response complete |
| `agent:error` | Backend → Frontend | Error occurred |

### 2. Tool System Pattern

Tools follow a standardized interface (`Tool` trait in Rust):

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    fn is_read_only(&self) -> bool { false }
    fn requires_permission(&self) -> bool { !self.is_read_only() }
    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult;
}
```

### 3. Store Pattern (Zustand)

Frontend state is organized into focused stores:

- `useAgentStore` - Messages, streaming state, tool calls
- `useSessionStore` - Session list, active session management
- `useSettingsStore` - Providers, models, system prompts
- `useCostStore` - Token usage and cost tracking

---

## Backend (Rust/Tauri)

### Core Files

#### `src-tauri/src/lib.rs`

**Purpose**: Main library entry point for Tauri application. Registers all commands and sets up the application state.

**Key Components**:
- `run()` function - Main Tauri builder setup
- Command registration via `invoke_handler!` macro
- Plugin initialization (logging, single instance)

**Registered Commands**:
- `send_message`, `cancel_agent`
- `list_sessions`, `create_session`, `delete_session`, `get_session_messages`, `update_session_title`
- `list_providers`, `set_active_provider`, `get_active_provider`, `get_active_model`, `set_active_model`
- `get_system_prompt`, `set_system_prompt`, `get_default_system_prompt`
- `provider_test_connection`, `get_provider_models`
- `vault_get`, `vault_set`, `vault_delete`, `vault_list`
- `get_cost_today`, `grant_permission`, `deny_permission`

**Relationships**:
- Uses: `commands::*` (all command modules)
- Uses: `state::AppState`
- Uses: `logging`

---

#### `src-tauri/src/main.rs`

**Purpose**: Application entry point. Initializes logging and runs the Tauri app.

**Key Components**:
- `main()` function - Calls `logging::init()` then `autodev_lib::run()`

**Relationships**:
- Uses: `logging`
- Uses: `autodev_lib` (lib.rs)

---

#### `src-tauri/src/errors.rs`

**Purpose**: Unified error type for the entire backend using `thiserror`.

**Key Components**:
```rust
pub enum AutoDevError {
    Vault(String),           // Keychain/vault errors
    Provider(String),        // LLM API errors
    Agent(String),           // Agent execution errors
    ToolExecution { tool: String, message: String },
    PermissionDenied { tool: String },
    Session(String),         // Session management errors
    Io(std::io::Error),
    Database(rusqlite::Error),
    Http(reqwest::Error),
    Serialization(serde_json::Error),
    Other(anyhow::Error),
}
```

**Relationships**:
- Implements: `serde::Serialize` (for IPC to frontend)
- Used by: All modules for consistent error handling

---

#### `src-tauri/src/events.rs`

**Purpose**: Tauri event constants and emission helpers for frontend-backend communication.

**Key Components**:
- Event constants: `EVT_AGENT_TOKEN`, `EVT_AGENT_THINKING`, `EVT_AGENT_TOOL_START`, `EVT_AGENT_TOOL_END`, `EVT_AGENT_PERMISSION`, `EVT_AGENT_DONE`, `EVT_AGENT_ERROR`, `EVT_COST_UPDATE`
- Payload structs: `TokenPayload`, `ThinkingPayload`, `ToolStartPayload`, `ToolEndPayload`, `PermissionPayload`, `DonePayload`, `ErrorPayload`
- Helper functions: `emit_token`, `emit_thinking`, `emit_tool_start`, `emit_tool_end`, `emit_permission_request`, `emit_done`, `emit_error`, `emit_cost_update`

**Relationships**:
- Used by: `agent::engine` (emits events)
- Used by: Frontend stores (listens to events)

---

#### `src-tauri/src/logging.rs`

**Purpose**: Structured logging initialization using `tracing` crate.

**Key Components**:
- `init()` function - Sets up env filter and subscriber
- Log levels: `autodev=debug,warn` by default
- Output format: Compact with target, no file info

**Relationships**:
- Called by: `main.rs` at startup

---

### Agent Module

**Location**: `src-tauri/src/agent/`

**Purpose**: Core agentic loop implementation for PA mode - manages LLM interaction, tool execution, and context.

#### `src-tauri/src/agent/mod.rs`

**Purpose**: Module declaration and re-exports.

**Exports**:
- `Tool` trait, `ToolResult`, `ToolContext` from `tool_trait`
- `ToolRegistry` from `tool_registry`
- `ToolExecutor` from `tool_executor`
- `PermissionMode`, `PermissionRequest`, `PermissionResponse`, `PendingPermissions` from `permission`
- `ContextBuilder` from `context`
- `AgentEngine` from `engine`
- `compact_messages` from `compaction`
- `register_builtin_tools` from `builtin_tools`

**Submodules**:
- `tool_trait`, `tool_registry`, `tool_executor`, `permission`, `context`, `engine`, `compaction`, `builtin_tools`

---

#### `src-tauri/src/agent/tool_trait.rs`

**Purpose**: Defines the `Tool` trait - uniform interface for all built-in tools.

**Key Components**:
```rust
pub struct ToolContext {
    pub working_dir: String,
    pub session_id: String,
}

pub type ToolResult = Result<serde_json::Value, AutoDevError>;

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    fn is_read_only(&self) -> bool;
    fn requires_permission(&self) -> bool;
    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult;
}
```

**Relationships**:
- Implemented by: All tools in `builtin_tools`
- Used by: `ToolRegistry`, `ToolExecutor`, `AgentEngine`

---

#### `src-tauri/src/agent/tool_registry.rs`

**Purpose**: Manages registration and lookup of tool implementations.

**Key Components**:
```rust
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self
    pub fn register(&mut self, tool: Arc<dyn Tool>)
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>>
    pub fn list(&self) -> Vec<Arc<dyn Tool>>
    pub fn definitions(&self) -> Vec<ToolDefinition>  // For LLM
}
```

**Relationships**:
- Contains: `Arc<dyn Tool>` instances
- Used by: `AgentEngine` to look up tools
- Used by: `builtin_tools::register_builtin_tools()` to populate

---

#### `src-tauri/src/agent/tool_executor.rs`

**Purpose**: Handles tool execution with permission checking and concurrency control.

**Key Components**:
```rust
pub struct ToolExecutor {
    mode: PermissionMode,
    pending: PendingPermissions,
    app: AppHandle,
    session_id: String,
    working_dir: String,
    always_allow: HashSet<String>,
}

impl ToolExecutor {
    pub async fn execute_tool(&self, tool: Arc<dyn Tool>, input: serde_json::Value) -> ToolResult
    pub async fn check_permission(&self, tool_name: &str, input: &serde_json::Value) -> PermissionCheck
}
```

**Key Behavior**:
- Read-only tools can run concurrently
- Write tools run serially with permission checks
- Emits `agent:permission-request` event for user approval

**Relationships**:
- Uses: `Tool` trait, `PermissionMode`, `AppHandle`
- Used by: `AgentEngine`

---

#### `src-tauri/src/agent/permission.rs`

**Purpose**: Permission system for tool execution safety.

**Key Components**:
```rust
pub enum PermissionMode {
    ManualApproval,  // Every tool requires approval
    Autonomous,      // No approval needed
    SmartApprove,    // Read-only auto-approved, write tools require approval
}

pub struct PermissionRequest {
    pub call_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub description: String,
}

pub enum PermissionResponse {
    AllowOnce,
    AllowAlways,
    Deny,
}
```

**Relationships**:
- Used by: `ToolExecutor`, `AgentEngine`
- Updated by: Frontend via `grant_permission`/`deny_permission` commands

---

#### `src-tauri/src/agent/context.rs`

**Purpose**: Builds the system prompt for LLM, injecting context from CLAUDE.md and AGENTS.md files.

**Key Components**:
```rust
pub struct ContextBuilder {
    working_dir: String,
}

impl ContextBuilder {
    pub fn new(working_dir: impl Into<String>) -> Self
    pub fn build(&self) -> String  // Returns full system prompt
    pub fn default_system_prompt() -> String  // Returns BASE_SYSTEM_PROMPT
}

pub const BASE_SYSTEM_PROMPT: &str = "...";
```

**Context Injection**:
- Global: `~/.autodev/CLAUDE.md`
- Project: `./CLAUDE.md`
- Agents: `./AGENTS.md`

**Relationships**:
- Used by: `AgentEngine` to construct prompts
- Called by: `settings_commands.rs` for `get_default_system_prompt`

---

#### `src-tauri/src/agent/engine.rs`

**Purpose**: Core agentic loop implementation. Manages LLM streaming, tool calls, and context.

**Key Components**:
```rust
pub struct AgentEngine {
    provider: Arc<dyn Provider>,
    registry: ToolRegistry,
    mode: PermissionMode,
    pending: PendingPermissions,
    app: AppHandle,
    state: Arc<Mutex<AppState>>,
}

impl AgentEngine {
    pub async fn run(&self, session_id: &str, messages: &[Message>) -> Result<RunResult, AutoDevError>
    pub fn stop(&self)
}
```

**Key Behavior**:
1. Streams LLM response
2. Detects tool_use blocks
3. Executes tools via `ToolExecutor`
4. Compacts context when needed
5. Emits events for frontend updates

**Relationships**:
- Uses: `Provider`, `ToolRegistry`, `ToolExecutor`, `ContextBuilder`, `AppState`
- Emits: Events via `events.rs`
- Called by: `agent_commands.rs` via `send_message` command

---

#### `src-tauri/src/agent/compaction.rs`

**Purpose**: Context window management - compacts message history when approaching token limits.

**Key Components**:
```rust
pub fn compact_messages(messages: &[Message]) -> Vec<Message>
```

**Relationships**:
- Used by: `AgentEngine` before sending to LLM

---

### Built-in Tools Module

**Location**: `src-tauri/src/agent/builtin_tools/`

#### `src-tauri/src/agent/builtin_tools/mod.rs`

**Purpose**: Registers all built-in tools with the registry.

**Key Components**:
```rust
pub fn register_builtin_tools(registry: &mut ToolRegistry) {
    registry.register(Arc::new(BashTool));
    registry.register(Arc::new(ReadFileTool));
    registry.register(Arc::new(WriteFileTool));
    registry.register(Arc::new(EditFileTool));
    registry.register(Arc::new(ListDirectoryTool));
    registry.register(Arc::new(GrepTool));
    registry.register(Arc::new(GlobTool));
}
```

**Relationships**:
- Calls: All tool constructors
- Called by: `lib.rs` during setup

---

#### `src-tauri/src/agent/builtin_tools/bash.rs`

**Purpose**: Execute shell commands in the working directory.

**Key Components**:
```rust
pub struct BashTool;

impl Tool for BashTool {
    fn name(&self) -> &str { "bash" }
    fn is_read_only(&self) -> bool { false }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult
}
```

**Input Schema**:
```json
{
  "command": "string"
}
```

**Output**: JSON with `stdout`, `stderr`, `exit_code`, `execution_time_ms`

**Relationships**:
- Implements: `Tool` trait
- Used by: `ToolExecutor`

---

#### `src-tauri/src/agent/builtin_tools/read_file.rs`

**Purpose**: Read file contents with optional line range.

**Key Components**:
```rust
pub struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }
    fn is_read_only(&self) -> bool { true }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult
}
```

**Input Schema**:
```json
{
  "path": "string",
  "start_line": "number (optional)",
  "end_line": "number (optional)"
}
```

**Relationships**:
- Implements: `Tool` trait
- Used by: `ToolExecutor`

---

#### `src-tauri/src/agent/builtin_tools/write_file.rs`

**Purpose**: Write content to a file (creates or overwrites).

**Key Components**:
```rust
pub struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> &str { "write_file" }
    fn is_read_only(&self) -> bool { false }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult
}
```

**Input Schema**:
```json
{
  "path": "string",
  "content": "string"
}
```

**Relationships**:
- Implements: `Tool` trait
- Used by: `ToolExecutor`

---

#### `src-tauri/src/agent/builtin_tools/edit_file.rs`

**Purpose**: Perform string replacement in a file.

**Key Components**:
```rust
pub struct EditFileTool;

impl Tool for EditFileTool {
    fn name(&self) -> &str { "edit_file" }
    fn is_read_only(&self) -> bool { false }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult
}
```

**Input Schema**:
```json
{
  "path": "string",
  "old_string": "string",
  "new_string": "string"
}
```

**Relationships**:
- Implements: `Tool` trait
- Used by: `ToolExecutor`

---

#### `src-tauri/src/agent/builtin_tools/list_directory.rs`

**Purpose**: List directory contents, optionally recursively.

**Key Components**:
```rust
pub struct ListDirectoryTool;

impl Tool for ListDirectoryTool {
    fn name(&self) -> &str { "list_directory" }
    fn is_read_only(&self) -> bool { true }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult
}
```

**Input Schema**:
```json
{
  "path": "string",
  "recursive": "boolean (default: false)"
}
```

**Relationships**:
- Implements: `Tool` trait
- Used by: `ToolExecutor`

---

#### `src-tauri/src/agent/builtin_tools/grep.rs`

**Purpose**: Search for patterns in files using ripgrep-style matching.

**Key Components**:
```rust
pub struct GrepTool;

impl Tool for GrepTool {
    fn name(&self) -> &str { "grep" }
    fn is_read_only(&self) -> bool { true }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult
}
```

**Input Schema**:
```json
{
  "pattern": "string",
  "path": "string (optional)",
  "file_pattern": "string (optional)"
}
```

**Output**: Structured JSON with matches per file including line numbers and content.

**Relationships**:
- Uses: `walkdir`, `regex`
- Implements: `Tool` trait

---

#### `src-tauri/src/agent/builtin_tools/glob.rs`

**Purpose**: Find files matching glob patterns.

**Key Components**:
```rust
pub struct GlobTool;

impl Tool for GlobTool {
    fn name(&self) -> &str { "glob" }
    fn is_read_only(&self) -> bool { true }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult
}
```

**Input Schema**:
```json
{
  "pattern": "string (e.g., 'src/**/*.rs')"
}
```

**Output**: JSON with file paths, sizes, modification dates.

**Relationships**:
- Uses: `glob` crate
- Implements: `Tool` trait

---

### Commands Module

**Location**: `src-tauri/src/commands/`

**Purpose**: Tauri command handlers - the API surface exposed to the frontend.

#### `src-tauri/src/commands/mod.rs`

**Purpose**: Module declaration for commands.

**Submodules**:
- `agent_commands`, `session_commands`, `settings_commands`, `vault_commands`

---

#### `src-tauri/src/commands/agent_commands.rs`

**Purpose**: Agent execution commands.

**Commands**:
- `send_message(session_id, user_message)` - Start agent loop
- `cancel_agent(session_id)` - Stop running agent
- `grant_permission(session_id, call_id, response)` - Approve/deny tool
- `deny_permission(session_id, call_id)` - Deny tool execution

**Relationships**:
- Uses: `AgentEngine`, `AppState`
- Called by: Frontend via `invoke()`

---

#### `src-tauri/src/commands/session_commands.rs`

**Purpose**: Chat session management commands.

**Commands**:
- `list_sessions()` - Get all sessions
- `create_session(working_dir, provider, model)` - Create new session
- `delete_session(session_id)` - Delete a session
- `get_session_messages(session_id)` - Get messages for a session
- `update_session_title(session_id, title)` - Rename session

**Relationships**:
- Uses: `SessionManager`, `Storage`
- Called by: `useSessionStore` frontend

---

#### `src-tauri/src/commands/settings_commands.rs`

**Purpose**: Settings and provider management commands.

**Commands**:
- `list_providers()` - Get available LLM providers
- `set_active_provider(provider_id)` - Set current provider
- `get_active_provider()` - Get current provider
- `get_active_model()` - Get current model
- `set_active_model(model_id)` - Set current model
- `get_system_prompt()` - Get custom system prompt
- `set_system_prompt(prompt)` - Set custom system prompt
- `get_default_system_prompt()` - Get base system prompt from Rust
- `provider_test_connection(provider_id)` - Test API key
- `get_provider_models(provider_id)` - List available models

**Relationships**:
- Uses: `ProviderManager`, `AppState`
- Called by: `useSettingsStore` frontend

---

#### `src-tauri/src/commands/vault_commands.rs`

**Purpose**: Secure credential storage commands.

**Commands**:
- `vault_get(key)` - Retrieve secret from keychain
- `vault_set(key, value)` - Store secret in keychain
- `vault_delete(key)` - Remove secret from keychain
- `vault_list()` - List all stored keys

**Relationships**:
- Uses: `KeychainVault`
- Called by: `useSettingsStore` for API key management

---

### Provider Module

**Location**: `src-tauri/src/provider/`

**Purpose**: LLM provider abstraction - Anthropic, OpenAI, Google, Ollama support.

#### `src-tauri/src/provider/mod.rs`

**Purpose**: Module re-exports.

**Exports**:
- `Provider` trait, `ProviderManager`, `ProviderConfig`
- `Message`, `ToolDefinition`, `ToolCall`, `ToolResult`, `StreamingChunk`

**Submodules**:
- `types`, `traits`, `anthropic`, `openai`, `google`, `streaming`

---

#### `src-tauri/src/provider/traits.rs`

**Purpose**: Provider trait definition.

**Key Components**:
```rust
#[async_trait]
pub trait Provider: Send + Sync {
    async fn chat(&self, messages: &[Message], tools: &[ToolDefinition]) -> Result<ProviderResponse, AutoDevError>;
    async fn chat_stream(&self, messages: &[Message], tools: &[ToolDefinition]) -> Result<StreamReceiver, AutoDevError>;
    async fn test_connection(&self) -> Result<bool, AutoDevError>;
    async fn list_models(&self) -> Result<Vec<String>, AutoDevError>;
}
```

**Relationships**:
- Implemented by: `AnthropicProvider`, `OpenAIProvider`, `GoogleProvider`, `OllamaProvider`
- Used by: `AgentEngine`

---

#### `src-tauri/src/provider/types.rs`

**Purpose**: Shared types for provider communication.

**Key Types**:
```rust
pub struct Message {
    pub role: String,  // "user" | "assistant"
    pub content: String,
}

pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

pub struct ToolCall {
    pub call_id: String,
    pub name: String,
    pub input: serde_json::Value,
}

pub struct ToolResult {
    pub call_id: String,
    pub output: String,
    pub is_error: bool,
}
```

**Relationships**:
- Used by: All provider implementations, `AgentEngine`

---

#### `src-tauri/src/provider/anthropic.rs`

**Purpose**: Anthropic Claude API provider implementation.

**Key Components**:
```rust
pub struct AnthropicProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl Provider for AnthropicProvider {
    async fn chat_stream(&self, ...) -> Result<StreamReceiver, AutoDevError>
}
```

**Features**:
- Native streaming support
- Tool use support
- Model selection (claude-sonnet-4, claude-opus-4, etc.)

---

#### `src-tauri/src/provider/openai.rs`

**Purpose**: OpenAI API provider implementation.

**Key Components**:
```rust
pub struct OpenAIProvider {
    api_key: String,
    model: String,
    base_url: String,  // Allows OpenAI-compatible APIs
}
```

---

#### `src-tauri/src/provider/google.rs`

**Purpose**: Google Gemini API provider implementation.

**Key Components**:
```rust
pub struct GoogleProvider {
    api_key: String,
    model: String,
}
```

---

#### `src-tauri/src/provider/streaming.rs`

**Purpose**: Streaming response utilities.

**Key Components**:
```rust
pub type StreamReceiver = mpsc::Receiver<StreamingChunk>;

pub enum StreamingChunk {
    Text(String),
    ToolCall(ToolCall),
    Done,
    Error(String),
}
```

**Relationships**:
- Used by: All streaming providers

---

### Session Module

**Location**: `src-tauri/src/session/`

**Purpose**: Chat session persistence and management.

#### `src-tauri/src/session/mod.rs`

**Purpose**: Module re-exports.

**Exports**: `SessionManager`, `Session`, `Message`, `ToolCallRecord`, `ToolResultRecord`

**Submodules**: `manager`, `storage`, `types`

---

#### `src-tauri/src/session/types.rs`

**Purpose**: Session data types.

**Key Components**:
```rust
pub struct Session {
    pub id: String,
    pub title: String,
    pub working_dir: String,
    pub provider: String,
    pub model: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: i64,
}

pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<ToolCallRecord>,
    pub tool_results: Vec<ToolResultRecord>,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub cost_usd: f64,
    pub created_at: String,
}
```

---

#### `src-tauri/src/session/storage.rs`

**Purpose**: SQLite persistence layer.

**Key Components**:
```rust
pub struct Storage {
    conn: Connection,
}

impl Storage {
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self, AutoDevError>
    pub fn create_session(&self, ...) -> Result<Session, AutoDevError>
    pub fn get_messages(&self, session_id: &str) -> Result<Vec<Message>, AutoDevError>
    pub fn save_message(&self, ...) -> Result<(), AutoDevError>
}
```

**Database Schema**:
- `sessions` table: id, title, working_dir, provider, model, timestamps
- `messages` table: id, session_id, role, content, tokens, cost, timestamp
- `tool_calls` table: id, message_id, call_id, tool_name, input
- `tool_results` table: id, call_id, output, is_error

**Relationships**:
- Used by: `SessionManager`

---

#### `src-tauri/src/session/manager.rs`

**Purpose**: High-level session operations.

**Key Components**:
```rust
pub struct SessionManager {
    storage: Storage,
}

impl SessionManager {
    pub fn new(storage: Storage) -> Self
    pub fn list(&self) -> Result<Vec<Session>, AutoDevError>
    pub fn create(&self, ...) -> Result<Session, AutoDevError>
    pub fn delete(&self, id: &str) -> Result<(), AutoDevError>
    pub fn get_messages(&self, session_id: &str) -> Result<Vec<Message>, AutoDevError>
    pub fn add_message(&self, ...) -> Result<(), AutoDevError>
}
```

**Relationships**:
- Uses: `Storage`
- Called by: `session_commands.rs`

---

### State Module

**Location**: `src-tauri/src/state/`

**Purpose**: In-memory runtime state management.

#### `src-tauri/src/state/mod.rs`

**Purpose**: Module declaration.

**Exports**: `AppState`

---

#### `src-tauri/src/state/app_state.rs`

**Purpose**: Central mutable application state.

**Key Components**:
```rust
pub struct AppState {
    // Current LLM configuration
    pub active_provider: Option<String>,
    pub active_model: Option<String>,
    
    // Cost tracking
    pub cost_today_usd: f64,
    pub tokens_in_today: u64,
    pub tokens_out_today: u64,
    
    // System prompt override
    pub system_prompt_override: Option<String>,
    
    // Active agent tracking
    pub active_agents: HashMap<String, AgentHandle>,
}

impl AppState {
    pub fn new() -> Self
    pub fn get_agent(&self, session_id: &str) -> Option<&AgentHandle>
    pub fn add_agent(&mut self, session_id: String, handle: AgentHandle)
    pub fn remove_agent(&mut self, session_id: &str)
    pub fn update_cost(&mut self, tokens_in: u64, tokens_out: u64, cost_usd: f64)
}
```

**Thread Safety**:
- Wrapped in `Arc<Mutex<>>` for safe concurrent access
- Managed by Tauri's State system

**Relationships**:
- Used by: All command handlers
- Managed by: Tauri app state

---

### Vault Module

**Location**: `src-tauri/src/vault/`

**Purpose**: Secure API key storage using OS keychain/keyring.

#### `src-tauri/src/vault/mod.rs`

**Purpose**: Module re-exports.

**Exports**: `KeychainVault`, `VaultError`

---

#### `src-tauri/src/vault/keychain.rs`

**Purpose**: OS-native secure storage implementation.

**Key Components**:
```rust
pub struct KeychainVault {
    service: String,
}

impl KeychainVault {
    pub fn new(service: impl Into<String>) -> Self
    pub fn get(&self, key: &str) -> Result<Option<String>, VaultError>
    pub fn set(&self, key: &str, value: &str) -> Result<(), VaultError>
    pub fn delete(&self, key: &str) -> Result<(), VaultError>
    pub fn list(&self) -> Result<Vec<String>, VaultError>
}
```

**Platform Support**:
- macOS: Keychain
- Windows: Credential Manager
- Linux: Secret Service (libsecret)

**Relationships**:
- Used by: `vault_commands.rs`
- Called by: Frontend for API key management

---

### Cost Module

**Location**: `src-tauri/src/cost/`

**Purpose**: Token usage and cost tracking.

#### `src-tauri/src/cost/mod.rs`

**Purpose**: Module re-exports.

**Exports**: `CostTracker`

---

#### `src-tauri/src/cost/tracker.rs`

**Purpose**: Cost calculation and daily tracking.

**Key Components**:
```rust
pub struct CostTracker;

impl CostTracker {
    pub fn calculate(tokens_in: u64, tokens_out: u64, model: &str) -> f64
    pub fn get_today() -> Result<(f64, u64, u64), AutoDevError>
    pub fn add_usage(tokens_in: u64, tokens_out: u64, cost_usd: f64) -> Result<(), AutoDevError>
}
```

**Model Pricing**:
- Claude Sonnet 4: $3/mTok input, $15/mTok output
- Claude Opus 4: $15/mTok input, $75/mTok output
- GPT-4: $30/mTok input, $60/mTok output

**Relationships**:
- Used by: `AppState` for cost updates
- Called by: `get_cost_today` command

---

### Database Module (Phase 2.1)

**Location**: `src-tauri/src/db/`

**Purpose**: SQLite persistence for tasks, model configs, and AUTODEV.md storage.

#### `src-tauri/src/db/mod.rs`

**Purpose**: Module declarations and re-exports.

**Exports**:
- `autodev_dir()` - Returns `~/.autodev` path
- `TasksDb`, `ModelConfigsDb`, `AutodevInstructionsDb`

**Submodules**: `tasks`, `model_configs`, `autodev_instructions`

---

#### `src-tauri/src/db/tasks.rs`

**Purpose**: Task management for Internal Board (~/.autodev/tasks.db).

**Key Components**:
```rust
pub struct TasksDb {
    conn: Connection,
}

pub struct Task {
    pub id: String,
    pub title: String,
    pub priority: TaskPriority,
    pub task_type: TaskType,
    pub status: TaskStatus,
    pub acceptance_criteria: String,
    pub assigned_worker: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

pub enum TaskStatus { Ready, Assigned, InProgress, QCReview, Done }
pub enum TaskPriority { Low, Medium, High, Critical }
pub enum TaskType { Feature, Bug, Refactor, Docs, Test, Infrastructure }
```

**Methods**:
- `create_task()` - Create new task with status="ready"
- `list_tasks()` - Get all tasks
- `get_by_status()` - Filter by status
- `update_status()` - Move through state machine with updated_at tracking
- `get_stalled_tasks()` - Find tasks stuck > 10 minutes
- `delete_task()` - Remove task

**Task State Machine**:
```
ready → assigned (supervisor) → in_progress (worker) → qc_review (worker) → done (QC)
                                                                    ↓
                                                              assigned (QC reject)
```

**Relationships**:
- Used by: `create_task_tool.rs` (Delegate Mode)
- Used by: `task_commands.rs` (AutoDev Panel)
- Used by: `recipes/runner.rs` (Recipe polling)

---

#### `src-tauri/src/db/model_configs.rs`

**Purpose**: Per-model configuration storage (~/.autodev/config.db).

**Key Components**:
```rust
pub struct ModelConfigsDb;

pub struct ModelConfig {
    pub temperature: Option<f32>,           // 0.0 - 2.0
    pub top_p: Option<f32>,               // 0.0 - 1.0
    pub reasoning_effort: Option<String>, // "low", "medium", "high"
    pub thinking_budget_tokens: Option<u32>,
}
```

**Methods**:
- `get_config(model_id)` - Retrieve config
- `set_config(model_id, config)` - Save config
- `delete_config(model_id)` - Remove config

**Relationships**:
- Used by: `model_config_commands.rs`
- Used by: Provider implementations (Anthropic, OpenAI, Google)

---

#### `src-tauri/src/db/autodev_instructions.rs`

**Purpose**: AUTODEV.md storage per project.

**Key Components**:
```rust
pub struct AutodevInstructionsDb;
```

**Methods**:
- `get(project_path)` - Retrieve AUTODEV.md content
- `set(project_path, content)` - Save content
- `delete(project_path)` - Remove entry
- `exists(project_path)` - Check existence
- `char_count(project_path)` - Get character count

**Relationships**:
- Used by: `autodev_instructions_commands.rs`
- Used by: `agent/context.rs` (injected into system prompt)

---

### Recipes Module (Phase 2.1)

**Location**: `src-tauri/src/recipes/`

**Purpose**: Recipe engine with YAML parsing, execution, and cron scheduling.

#### `src-tauri/src/recipes/mod.rs`

**Purpose**: Module declarations and re-exports.

**Exports**:
- `RecipeRunner`, `RecipeScheduler`, `RecipeParser`
- `Recipe`, `RecipeStep`, `RecipeTrigger`
- `ensure_default_recipes()`

**Submodules**: `runner`, `scheduler`, `parser`, `defaults`, `types`

---

#### `src-tauri/src/recipes/types.rs`

**Purpose**: Recipe data types.

**Key Components**:
```rust
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub permission_mode: PermissionMode,
    pub trigger: RecipeTrigger,
    pub steps: Vec<RecipeStep>,
}

pub struct RecipeStep {
    pub name: String,
    pub instruction: String,
    pub tools: Option<Vec<String>>,
    pub expected_output: Option<String>,
}

pub enum RecipeTrigger {
    Cron { schedule: String },    // "*/1 * * * *"
    Startup,
    Webhook { endpoint: String },
}

pub enum WorkerType {
    Supervisor, Backend, Frontend, Platform, MCP, Cleanup, QCAgent, Custom,
}
```

---

#### `src-tauri/src/recipes/parser.rs`

**Purpose**: YAML recipe parsing and validation.

**Key Components**:
```rust
pub struct RecipeParser;

impl RecipeParser {
    pub fn parse(yaml: &str) -> Result<Recipe, RecipeParseError>
    pub fn validate(recipe: &Recipe) -> Result<(), RecipeParseError>
    pub fn parse_file(path: &Path) -> Result<Recipe, RecipeParseError>
}
```

**Recipe Schema**:
```yaml
id: supervisor-poll
name: Supervisor Poll
version: "1.0"
description: Polls for ready tasks and assigns to workers
permission_mode: SmartApprove
trigger:
  type: cron
  schedule: "*/1 * * * *"
steps:
  - name: Poll tasks
    instruction: "Query the task database for tasks with status='ready'..."
    tools: ["list_tasks"]
```

---

#### `src-tauri/src/recipes/runner.rs`

**Purpose**: Execute recipe steps as agent prompts (in-memory context only).

**Key Components**:
```rust
pub struct RecipeRunner {
    recipe: Recipe,
    provider: Arc<dyn Provider>,
    tool_registry: ToolRegistry,
    context: RecipeContext,
}

impl RecipeRunner {
    pub async fn run(&self) -> Result<RunResult, AutoDevError>
    pub async fn execute_step(&self, step: &RecipeStep) -> Result<StepResult, AutoDevError>
}
```

**Key Behavior**:
- Polls tasks from `tasks.rs` by status (assigned, in_progress)
- Executes recipe instructions as LLM prompts
- Updates task status through state machine
- **Ephemeral sessions** - No DB persistence (removed `.with_session_manager()`)
- In-memory context only

**Task Polling**:
```rust
// Workers poll for BOTH assigned AND in_progress tasks
let tasks = tasks_db.get_by_status("assigned")?;
let in_progress = tasks_db.get_by_status("in_progress")?;
// Resume interrupted work
```

**Relationships**:
- Uses: `tasks.rs` for task CRUD
- Uses: `AgentEngine` for LLM execution
- Triggered by: `scheduler.rs`

---

#### `src-tauri/src/recipes/scheduler.rs`

**Purpose**: Cron-based recipe execution using `tokio-cron-scheduler`.

**Key Components**:
```rust
pub struct RecipeScheduler {
    sched: JobScheduler,
}

impl RecipeScheduler {
    pub fn new() -> Self
    pub fn register_recipe(&mut self, recipe: &Recipe) -> Result<Uuid, JobSchedulerError>
    pub async fn start(&self) -> Result<(), JobSchedulerError>
    pub async fn stop(&self) -> Result<(), JobSchedulerError>
}
```

**Schedule Formats**:
- `"*/1 * * * *"` - Every 1 minute (supervisor, workers, QC)
- `"*/5 * * * *"` - Every 5 minutes (platform)
- `"*/10 * * * *"` - Every 10 minutes (cleanup)

**Relationships**:
- Triggers: `runner.rs` execution
- Called by: `lib.rs` on startup

---

#### `src-tauri/src/recipes/defaults.rs`

**Purpose**: Hardcoded system recipes written on startup.

**Exports**:
- `ensure_default_recipes()` - Writes system recipes to `~/.autodev/recipes/`
- `SUPERVISOR_RECIPE`, `WORKER_BACKEND_RECIPE`, etc.

**System Recipes** (always overwritten on startup - code is source of truth):

| Recipe | Schedule | Purpose |
|--------|----------|---------|
| `supervisor-poll.yaml` | */1 min | Polls ready tasks, assigns to workers |
| `worker-backend-poll.yaml` | */1 min | Executes backend tasks |
| `worker-frontend-poll.yaml` | */1 min | Executes frontend tasks |
| `worker-mcp-poll.yaml` | */1 min | Executes MCP integration tasks |
| `worker-platform-poll.yaml` | */5 min | Platform/infrastructure tasks |
| `worker-cleanup-poll.yaml` | */10 min | Cleanup tasks |
| `qc-agent-poll.yaml` | */1 min | Quality control review |

**Relationships**:
- Called by: `lib.rs` during app initialization

---

## Frontend (React/TypeScript)

### Core Files

#### `src/App.tsx`

**Purpose**: Root React component. Initializes stores and renders main layout.

**Key Components**:
```typescript
function App() {
  // Initialize stores on mount
  useEffect(() => {
    loadSettings()
    loadSessions()
    loadDefaultSystemPrompt()
  }, [])

  return (
    <div className="app-container">
      <Sidebar />
      <MainContent />
      <StatusBar />
    </div>
  )
}
```

**Initialization Flow**:
1. Load settings from backend
2. Load session list
3. Load default system prompt from Rust

**Relationships**:
- Uses: All stores (`useSettingsStore`, `useSessionStore`, `useCostStore`)
- Renders: `Sidebar`, `ChatTab`, `SettingsTab`, `StatusBar`

---

#### `src/main.tsx`

**Purpose**: React application entry point.

**Key Components**:
```typescript
ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
)
```

**Relationships**:
- Renders: `App.tsx`

---

### Stores

#### `src/stores/agentStore.ts`

**Purpose**: Agent message state and streaming management.

**Key Components**:
```typescript
interface AgentState {
  messages: ChatMessage[]
  isStreaming: boolean
  streamingText: string
  pendingPermission: PermissionRequest | null
  sessionId: string | null
  workingDir: string
}

export const useAgentStore = create<AgentState>((set, get) => ({
  // Actions
  addUserMessage: (text) => {...}
  startStreaming: (sessionId) => {...}
  stopStreaming: () => {...}
  setupListeners: async (sessionId) => {...}  // Event listeners
}))
```

**Event Listeners**:
- `agent:token` - Append to streaming message
- `agent:tool-start` - Add pending tool call
- `agent:tool-end` - Update tool call with result
- `agent:permission-request` - Show permission dialog
- `agent:done` - Finalize message with cost
- `agent:error` - Show error message

**Relationships**:
- Listens to: Tauri events from backend
- Used by: `Chat.tsx`, `MessageBubble.tsx`, `ToolCallDisplay.tsx`, `ContextMeter.tsx`

---

#### `src/stores/sessionStore.ts`

**Purpose**: Session list and active session management.

**Key Components**:
```typescript
interface SessionState {
  sessions: Session[]
  activeSession: Session | null
  activeSessionId: string | null
  isLoading: boolean
}

export const useSessionStore = create<SessionState>((set, get) => ({
  loadSessions: async () => {...}
  createSession: async (workingDir, provider, model) => {...}
  switchSession: async (id) => {...}
  deleteSession: async (id) => {...}
  renameSession: async (id, newTitle) => {...}
}))
```

**Relationships**:
- Calls Tauri commands: `list_sessions`, `create_session`, `delete_session`, `get_session_messages`, `update_session_title`
- Used by: `SessionBar.tsx`, `Chat.tsx`

---

#### `src/stores/settingsStore.ts`

**Purpose**: Provider configuration, system prompt, and PA Mode state.

**Key Components**:
```typescript
type PAMode = 'fast' | 'review' | 'delegate';

interface SettingsState {
  providers: Provider[]
  activeProvider: string | null
  activeModel: string | null
  systemPrompt: string | null
  defaultSystemPrompt: string
  isLoadingProviders: boolean
  paMode: PAMode  // Phase 2.1: PA Mode Toggle
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  loadProviders: async () => {...}
  setActiveProvider: async (id) => {...}
  setSystemPrompt: async (prompt) => {...}
  loadDefaultSystemPrompt: async () => {...}
  resetSystemPrompt: async () => {...}
  setPAMode: (mode: PAMode) => {...}  // Phase 2.1
}))
```

**PA Mode Integration**:
- `paMode` sent to backend on every `send_message` invoke
- Backend filters tools based on mode (Delegate = only create_task)

**Relationships**:
- Calls Tauri commands: `list_providers`, `set_active_provider`, `get_system_prompt`, `set_system_prompt`, `get_default_system_prompt`, `vault_get`, `vault_set`
- Used by: `Settings.tsx`, `StatusBar.tsx`, `PaModeToggle.tsx`

---

#### `src/stores/costStore.ts`

**Purpose**: Token usage and cost tracking (today's totals).

**Key Components**:
```typescript
interface CostState {
  costTodayUsd: number
  tokensInToday: number
  tokensOutToday: number
}

export const useCostStore = create<CostState>((set) => ({
  loadFromBackend: async () => {...}
  addCost: (costUsd, tokensIn, tokensOut) => {...}
}))
```

**Relationships**:
- Listens to: `agent:done` event
- Displays in: `StatusBar.tsx`, `AutoDevPanel.tsx`

---

#### `src/stores/taskStore.ts` (Phase 2.1)

**Purpose**: Internal Board task management.

**Key Components**:
```typescript
interface TaskState {
  tasks: Task[]
  isLoading: boolean
  tasksByStatus: Record<TaskStatus, Task[]>
}

export const useTaskStore = create<TaskState>((set, get) => ({
  loadTasks: async () => {...}
  updateTaskStatus: async (id, status) => {...}
  deleteTask: async (id) => {...}
  getTasksByStatus: (status) => {...}
}))
```

**Tauri Commands**: `list_tasks`, `update_task_status`, `delete_task`

**Relationships**:
- Used by: `AutoDevPanel.tsx`, `KanbanBoard.tsx`, `TaskCard.tsx`

---

#### `src/stores/workerStore.ts` (Phase 2.1)

**Purpose**: Worker agent roster and status.

**Key Components**:
```typescript
interface WorkerState {
  workers: Worker[]
  isLoading: boolean
}

export const useWorkerStore = create<WorkerState>((set, get) => ({
  loadWorkers: async () => {...}
  updateWorker: async (worker) => {...}
  bindRecipeToWorker: async (workerId, recipeId) => {...}
}))
```

**Tauri Commands**: `list_workers`, `update_worker`, `bind_worker_recipe`

**Relationships**:
- Used by: `AgentRoster.tsx`, `WorkerTable.tsx`

---

#### `src/stores/modelConfigStore.ts` (Phase 2.3)

**Purpose**: Per-model configuration (temperature, top_p, reasoning).

**Key Components**:
```typescript
interface ModelConfigState {
  configs: Record<string, ModelConfig>
  activeConfig: ModelConfig | null
}

interface ModelConfig {
  temperature?: number          // 0.0 - 2.0
  top_p?: number                // 0.0 - 1.0
  reasoning_effort?: string     // "low", "medium", "high"
  thinking_budget_tokens?: number
}

export const useModelConfigStore = create<ModelConfigState>((set) => ({
  loadModelConfig: async (modelId) => {...}
  setModelConfig: async (modelId, config) => {...}
  resetModelConfig: async (modelId) => {...}
}))
```

**Tauri Commands**: `get_model_config`, `set_model_config`, `delete_model_config`

**Relationships**:
- Used by: `ModelConfigSection.tsx`

---

#### `src/stores/recipeStore.ts` (Phase 2.1-2.3)

**Purpose**: Recipe list and AI generation.

**Key Components**:
```typescript
interface RecipeState {
  recipes: Recipe[]
  isLoading: boolean
  selectedRecipe: Recipe | null
}

export const useRecipeStore = create<RecipeState>((set, get) => ({
  loadRecipes: async () => {...}
  selectRecipe: (recipe) => {...}
  generateRecipeWithAI: async (workerType, description) => {...}  // Phase 2.3
}))
```

**Tauri Commands**: `list_recipes`, `get_recipe`, `generate_recipe_yaml`

**Relationships**:
- Used by: `Recipes.tsx`, `RecipeEditor.tsx`, `WorkerTable.tsx`

---

#### `src/stores/ideStore.ts` (Phase 3.2)

**Purpose**: IDE Core state management.

**Key Components**:
```typescript
interface IdeState {
  openTabs: OpenTab[]
  activeTab: string | null
  fileTree: FileTreeNode[]
  expandedDirs: Set<string>
  recentFiles: string[]
  isSearchOpen: boolean
  searchQuery: string
  searchResults: FileSearchResult[]
}

export const useIdeStore = create<IdeState>((set, get) => ({
  init: async () => {...}
  refreshTree: async () => {...}
  loadDirChildren: async (path) => {...}
  toggleDir: (path) => {...}
  openFile: async (path, gotoLine?) => {...}  // Integration API
  closeTab: (path) => {...}
  closeOtherTabs: (path) => {...}
  closeAllTabs: () => {...}
  switchTab: (path) => {...}
  setSearchOpen: (open) => {...}
}))
```

**Integration API**:
```typescript
// For chat tool-result renderers
import { useIdeStore } from '@/stores/ideStore'
useIdeStore.getState().openFile('src/foo.ts', 42)
```

**Tauri Commands**:
- `read_file_tree`, `read_file_content`, `search_files`
- `get_ide_workspace_state`, `set_ide_workspace_state`

**Relationships**:
- Used by: `IDE.tsx`, `FileTree.tsx`, `TabBar.tsx`, `CodeViewer.tsx`, `FileSearch.tsx`, `SymbolOutline.tsx`

---

#### `src/stores/uiStore.ts` (Phase 3.2)

**Purpose**: Global UI state for cross-component communication.

**Key Components**:
```typescript
type TabId = 'chat' | 'ide' | 'autodev' | 'settings' | 'recipes'

interface UiState {
  activeTab: TabId
}

export const useUiStore = create<UiState>((set) => ({
  setActiveTab: (tab: TabId) => set({ activeTab: tab })
}))
```

**Integration Flow**:
```typescript
// Click file in chat → open in IDE
import { useUiStore } from '@/stores/uiStore'
import { useIdeStore } from '@/stores/ideStore'

function openInIde(path: string, gotoLine?: number) {
  useUiStore.getState().setActiveTab('ide')  // Switch to IDE tab
  useIdeStore.getState().openFile(path, gotoLine)  // Open file
}
```

**Relationships**:
- Used by: `App.tsx`, `Sidebar.tsx`, `openInIde.ts`

---

### Tabs

#### `src/tabs/Chat.tsx`

**Purpose**: Main chat interface with streaming, tool display, and PA Mode Toggle.

**Key Components**:
```typescript
export default function ChatTab() {
  const messages = useAgentStore(s => s.messages)
  const isStreaming = useAgentStore(s => s.isStreaming)
  const paMode = useSettingsStore(s => s.paMode)
  
  const handleSend = async () => {
    // 1. Ensure session exists
    // 2. Set up event listeners
    // 3. Add user message to store
    // 4. Invoke send_message command WITH paMode param
    await invoke('send_message', { sessionId, userMessage, paMode })
  }
  
  const handleCancel = async () => {
    await invoke('cancel_agent', { sessionId })
  }
}
```

**Layout**:
- Header: `PaModeToggle` (3-button segmented control)
- Left: `SessionBar` (200px)
- Center: Message list (`MessageBubble` components)
- Bottom: Input area with `ContextMeter`

**PA Mode Integration**:
- Sends `paMode` parameter to backend on every `send_message`
- Backend filters tool registry based on mode:
  - Fast Development: All 7 file tools
  - Review Mode: All tools + permission prompts
  - Delegate Mode: Only `create_task` tool

**Relationships**:
- Uses: `useAgentStore`, `useSessionStore`, `useSettingsStore`, `useCostStore`
- Calls: `send_message`, `cancel_agent` commands
- Renders: `MessageBubble`, `PermissionDialog`, `SessionBar`, `PaModeToggle`, `ContextMeter`

---

#### `src/tabs/Settings.tsx`

**Purpose**: Provider configuration, system prompt editing, GitHub settings.

**Key Components**:
```typescript
export default function SettingsTab() {
  const store = useSettingsStore()
  
  return (
    <div>
      <SectionHeader title="Providers" />
      {store.providers.map(p => <ProviderCard key={p.id} providerId={p.id} />)}
      
      <SectionHeader title="Model Configuration" />
      <ModelConfigSection />  {/* Phase 2.3: Temperature, Top-P, Reasoning */}
      
      <SectionHeader title="GitHub Integration" />
      <GitHubPATCard />
      <GitHubMCPCard />  {/* Phase 2.3: MCP Server config */}
      
      <SectionHeader title="System Prompt" />
      <SystemPromptEditor />
    </div>
  )
}
```

**Features**:
- Provider list with connection testing
- API key input (masked, stored in vault)
- Model selection dropdown
- **ModelConfigSection** (Phase 2.3): Temperature, Top-P, reasoning effort
- **GitHubMCPCard** (Phase 2.3): MCP server configuration
- System prompt editor with save/reset

**Relationships**:
- Uses: `useSettingsStore`, `useModelConfigStore`

---

#### `src/tabs/AutoDevPanel.tsx` (Phase 2.1)

**Purpose**: Internal Task Board with Kanban view.

**Key Components**:
```typescript
export default function AutoDevPanel() {
  const { tasks, loadTasks } = useTaskStore()
  const { workers } = useWorkerStore()
  
  // Auto-refresh every 60 seconds
  useEffect(() => {
    const interval = setInterval(() => {
      loadTasks()
      loadCost()
      loadWorkers()
    }, 60_000)
    return () => clearInterval(interval)
  }, [])
  
  return (
    <div className="autodev-panel">
      <Header title="AutoDev Board" />
      <AgentRoster workers={workers} />
      <KanbanBoard tasks={tasks} />
      <MetricsFooter />
    </div>
  )
}
```

**Layout**:
- Header: Title + Agent Roster toggle
- Main: 4-column Kanban (Ready | In Progress | QC Review | Done Today)
- Footer: Cost Today | Tokens | Closed | QC Pass Rate

**Features**:
- Task cards with expand for acceptance criteria
- Drag-and-drop status changes
- **Agent roster** with 7 hardcoded agents + status dots
- **Auto-refresh** every 60 seconds (Phase 2.6)
- Metrics footer with live calculations

**Task State Machine Display**:
- `Ready` column: Merges `ready` + `assigned` status
- `In Progress` column: Shows progress indicator
- `QC Review` column: Tasks awaiting quality control
- `Done Today` column: Completed today only

**Relationships**:
- Uses: `useTaskStore`, `useWorkerStore`, `useCostStore`
- Renders: `KanbanBoard`, `TaskCard`, `AgentRoster`

---

#### `src/tabs/Recipes.tsx` (Phase 2.1-2.3)

**Purpose**: Recipe browser and management.

**Key Components**:
```typescript
export default function RecipesTab() {
  const { recipes, loadRecipes } = useRecipeStore()
  
  return (
    <div className="recipes-tab">
      <h1>Recipes</h1>
      <div className="recipe-list">
        {recipes.map(recipe => (
          <RecipeCard key={recipe.id} recipe={recipe} />
        ))}
      </div>
      <RecipeEditor />  {/* Phase 2.3: AI generation panel */}
    </div>
  )
}
```

**Features**:
- Recipe list (system + user recipes)
- Expandable recipe cards with YAML preview
- **[Run Now] button** (disabled in 2.1, coming in 2.2)
- **Generate with AI panel** (Phase 2.3):
  - Worker type selector
  - Description textarea
  - YAML preview with Apply to Form

**Relationships**:
- Uses: `useRecipeStore`
- Renders: `RecipeCard`, `RecipeEditor`

---

## Phase 3.3 — Git Integration + Checkpointing

Full git integration built on `git2-rs` (libgit2), plus a file-level
checkpointing system used by every file-mutating agent tool so edits are
always undoable.

### Backend: `src-tauri/src/git/`

Module composition (all files live under `src-tauri/src/git/`):

| File | Responsibility |
|---|---|
| `mod.rs`        | Module declaration + doc comments |
| `types.rs`      | DTOs: `RepoInfo`, `GitStatusReport`, `StatusEntry`, `FileStatus`, `DiffResult`, `CommitInfo`, `BranchInfo`, `WorktreeInfo`, `StashInfo`, `PullResult`, `BlameLine`, `CheckpointInfo`, `RestoreResult` |
| `repo.rs`       | `open / info / init / clone_repo` plus `build_info` helper |
| `status.rs`     | `status()` — categorises into staged / unstaged / untracked / conflicted + ahead/behind |
| `diff.rs`       | Unified-diff generation for workdir↔index, index↔HEAD, and in-memory blob-to-blob |
| `commit.rs`     | `stage / unstage / commit / discard_changes` — commit message trailer injection (`Generated-by: AutoDev/<role>`) |
| `branch.rs`     | `list / create / delete / checkout` with upstream + ahead/behind resolution |
| `remote.rs`     | `fetch / push / pull` with ssh-agent + credential-helper callbacks; pull enforces fast-forward |
| `worktree.rs`   | `list / add / remove` for per-task worktree isolation (used later by Fleet) |
| `history.rs`    | `log` (optionally path-filtered) + `blame` |
| `stash.rs`      | `list / push / apply / drop` |
| `checkpoint.rs` | Snapshot store under `~/.autodev/checkpoints/<session_id>/` with SQLite index in `config.db`. `RETENTION_PER_FILE = 50`. Public `auto_checkpoint_before_write()` used by `write_file` / `edit_file` tools. |

### Tauri commands — `src-tauri/src/commands/git_commands.rs`

29 commands across repository, status/diff, staging, commit, remote, branch, worktree, history/blame, stash, and checkpoint. Long-running network ops (`git_clone`, `git_push`, `git_pull`, `git_fetch`) run on a blocking task via `tokio::task::spawn_blocking`.

### Auto-checkpoint integration

Every call to the `write_file` and `edit_file` builtin tools calls
`crate::git::checkpoint::auto_checkpoint_before_write(session_id, file_path)`
immediately before the filesystem write. Errors are logged but never propagate — checkpointing must never block the edit.

### Frontend — `src/components/git/` + `src/stores/gitStore.ts`

`gitStore.ts` is a Zustand store mirroring every DTO and wrapping every Tauri command. Components (all dark-theme inline styles, consistent with the rest of the IDE):

- `GitPanel.tsx` — togglable right-side panel with 5 tabs: **Changes / History / Branches / Stash / Undo**. Mounted in `tabs/IDE.tsx`, opened via the **Git** button in the tab header. Renders sub-components for each section.
- `DiffViewer.tsx` — renders a unified-diff patch string with line-origin colouring (additions green, deletions red, hunk headers lavender).
- `BranchManager.tsx` — local + remote branch lists, create-and-checkout input, hover-delete.
- `StashManager.tsx` — list + create (with include-untracked) + apply / pop / drop per-row.
- `CheckpointViewer.tsx` — timeline grouped by file path, restore with inline confirmation, delete per row.

### IDE integration — `src/tabs/IDE.tsx`

A Git toggle button sits at the right edge of the TabBar row. The GitPanel mounts in the flex row after the main editor column, with a fixed width (360px default, 280px min).

### Data flow

```
UI action
  ↓
useGitStore.action()
  ↓ invoke('git_*')
Tauri IPC
  ↓
commands::git_commands::*
  ↓
crate::git::<module>
  ↓
git2-rs (libgit2)
```

For checkpoints:

```
agent tool (write_file / edit_file)
  ↓ synchronous
git::checkpoint::auto_checkpoint_before_write()
  ↓ insert row
config.db `checkpoints` table
  ↓ write blob
~/.autodev/checkpoints/<session>/<stamp>_<basename>.<uuid>.snap
```

### Key architectural decisions

1. **libgit2 via `git2` crate, not shelling out.** All git ops are in-process for performance and error fidelity.
2. **Pull is fast-forward only.** Non-ff merges return an error; a full merge driver (`git/merge.rs`) is deferred. This keeps the UX safe — agents never silently resolve conflicts.
3. **Checkpoints are filesystem-backed, SQLite-indexed.** Fast random access via the DB, but the snapshots themselves are plain files the user can inspect/copy.
4. **Unified-diff rendering in pure React.** We get the patch string from libgit2 and render it ourselves; no Monaco DiffEditor dependency, which keeps the Git panel lightweight.
5. **Trailer-based attribution.** Commits created from AutoDev get a `Generated-by: AutoDev/<role>` trailer — machine-greppable and preserves normal commit flow.

---

#### `src/tabs/IDE.tsx` (Phase 3.2)

**Purpose**: IDE Core with file explorer and code viewer.

**Key Components**:
```typescript
export default function IDETab() {
  const { fileTree, openTabs, activeTab } = useIdeStore()
  
  return (
    <div className="ide-layout">
      <aside className="sidebar">
        <FileTree tree={fileTree} />
        <SymbolOutline />
      </aside>
      <main className="editor-area">
        <TabBar tabs={openTabs} active={activeTab} />
        <CodeViewer />
        <IdeStatusBar />
      </main>
      <FileSearch />  {/* Ctrl/⌘+P overlay */}
    </div>
  )
}
```

**Layout**:
- Left sidebar: `FileTree` + `SymbolOutline`
- Center: `TabBar` + `CodeViewer` (Monaco)
- Right: (reserved for future git panel - Phase 3.3)
- Bottom: `IdeStatusBar`

**Features**:
- File tree with **git status badges** (modified, added, untracked)
- Tab bar with **middle-click close**
- **Monaco read-only viewer** with custom Winter-is-Coming theme
- **Quick-open file search** (Ctrl/⌘+P)
- **Symbol outline** for navigation
- **Click file in chat to open in IDE** (integration)

**Integration API**:
```typescript
// From chat tool-result renderers
import { useIdeStore } from '@/stores/ideStore'
useIdeStore.getState().openFile('src/foo.ts', 42)
```

**Relationships**:
- Uses: `useIdeStore`, `useUiStore`
- Tauri Commands: `read_file_tree`, `read_file_content`, `search_files`, `get_symbol_tree`
- Renders: `FileTree`, `TabBar`, `CodeViewer`, `FileSearch`, `SymbolOutline`, `IdeStatusBar`

---

### Components

#### `src/components/layout/StatusBar.tsx`

**Purpose**: Bottom status bar showing active model and cost.

**Key Components**:
```typescript
export default function StatusBar() {
  const costToday = useCostStore(s => s.costTodayUsd)
  const tokensIn = useCostStore(s => s.tokensInToday)
  const tokensOut = useCostStore(s => s.tokensOutToday)
  const activeModel = useSettingsStore(s => s.activeModel)
  const activeProvider = useSettingsStore(s => s.activeProvider)
  
  return (
    <div className="flex items-center justify-between">
      <div>autodev</div>
      <div>
        <span>{activeProvider}/{activeModel}</span>
        <span>${costToday} today</span>
        <span>↑{tokensIn} ↓{tokensOut}</span>
      </div>
    </div>
  )
}
```

**Relationships**:
- Uses: `useCostStore`, `useSettingsStore`

---

#### `src/components/chat/MessageBubble.tsx`

**Purpose**: Renders individual chat messages with markdown and tool calls.

**Key Components**:
```typescript
interface Props {
  message: ChatMessage
  isLast: boolean
  streaming: boolean
}

export default function MessageBubble({ message, isLast, streaming }: Props) {
  const isUser = message.role === 'user'
  const isAssistant = message.role === 'assistant'
  
  return (
    <div className={isUser ? 'items-end' : 'items-start'}>
      {/* Message content with ReactMarkdown */}
      <ReactMarkdown remarkPlugins={[remarkGfm]}>
        {message.content}
      </ReactMarkdown>
      
      {/* Tool call blocks */}
      {message.toolCalls.map(tc => <ToolCallDisplay key={tc.callId} toolCall={tc} />)}
    </div>
  )
}
```

**Relationships**:
- Uses: `react-markdown`, `remark-gfm`
- Renders: `ToolCallDisplay`

---

#### `src/components/chat/ToolCallDisplay.tsx`

**Purpose**: Renders tool call blocks with context-specific display.

**Supported Tools**:
- `read_file` → `FileContentDisplay` (syntax highlighted)
- `bash` → `BashOutputDisplay` (terminal-style)
- `grep` → `GrepResultsDisplay` (search results)
- `glob` → `GlobResultsDisplay` (file list table)
- `list_directory` → `DirectoryListingDisplay`

**Relationships**:
- Uses: `toolOutputParser.ts` for parsing
- Renders: Tool-specific display components

---

#### `src/components/chat/SessionBar.tsx`

**Purpose**: Left sidebar showing chat sessions.

**Features**:
- Session list with title, timestamp, message count
- "New Session" button
- Active session highlighting
- Click to switch sessions
- Delete and rename actions

**Relationships**:
- Uses: `useSessionStore`

---

#### `src/components/chat/PermissionDialog.tsx`

**Purpose**: Modal for tool execution approval.

**Features**:
- Shows tool name and input
- Approve once / Approve always / Deny options
- Triggered by `agent:permission-request` event

**Relationships**:
- Uses: `useAgentStore` (for `pendingPermission`)
- Calls: `grant_permission`, `deny_permission` commands

---

### Utils

#### `src/utils/toolOutputParser.ts`

**Purpose**: Parse tool output strings into structured data for display.

**Key Components**:
```typescript
export interface GrepOutput {
  pattern: string
  total_matches: number
  files_matched: number
  results: GrepFileResult[]
}

export interface GlobOutput {
  pattern: string
  total_files: number
  files: GlobFileEntry[]
}

export interface DirectoryEntry {
  name: string
  path: string
  is_directory: boolean
  size_bytes?: number
}

export type ToolOutputType = 
  | { type: 'grep'; data: GrepOutput }
  | { type: 'glob'; data: GlobOutput }
  | { type: 'bash'; data: BashOutput }
  | { type: 'file_content'; data: FileContent }
  | { type: 'directory_listing'; data: DirectoryListingOutput }
  | null

export function parseToolOutput(toolName: string, output: string): ToolOutputType
export function detectLanguageFromPath(path: string): string
export function formatFileSize(bytes: number): string
```

**Relationships**:
- Used by: `ToolCallDisplay.tsx` to render appropriate component
- Supports: Syntax highlighting decisions
- Phase 1 Extension: Added `DirectoryEntry` for `list_directory` tool

---

#### `src/utils/openInIde.ts` (Phase 3.2)

**Purpose**: Universal helper to open files in IDE from chat tool results.

**Key Components**:
```typescript
export async function openInIde(path: string, gotoLine?: number): Promise<void> {
  // 1. Normalize path
  const normalizedPath = path.startsWith('/') ? path.slice(1) : path
  
  // 2. Switch to IDE tab via uiStore
  useUiStore.getState().setActiveTab('ide')
  
  // 3. Open file via ideStore (handles line jumps)
  await useIdeStore.getState().openFile(normalizedPath, gotoLine)
}
```

**Integration Points**:
- `FileContentDisplay.tsx` - "Open in IDE" button
- `GrepResultsDisplay.tsx` - Click match line to jump
- `GlobResultsDisplay.tsx` - Click file path
- `DirectoryListingDisplay.tsx` - Click file entry

**Relationships**:
- Uses: `useUiStore` (tab switching)
- Uses: `useIdeStore` (file opening)
- Called by: Chat tool-result renderers

---

## Data Flow

### 1. Sending a Message

```
User Input
    ↓
Chat.tsx handleSend()
    ↓
ensureSession() → creates session if needed
    ↓
useAgentStore.addUserMessage() → adds to UI
    ↓
useAgentStore.startStreaming() → creates assistant placeholder
    ↓
invoke('send_message') → backend
    ↓
agent_commands.rs → AgentEngine.run()
    ↓
Provider.chat_stream() → LLM API
    ↓
Streaming chunks emitted:
    - agent:token events → update message content
    - agent:tool-start → add tool call block
    - agent:tool-end → update with result
    - agent:done → finalize with cost
```

### 2. Tool Execution Flow

```
AgentEngine detects tool_use
    ↓
ToolExecutor.check_permission()
    ↓
If SmartApprove + read-only: execute immediately
If SmartApprove + write: emit agent:permission-request
    ↓
PermissionDialog.tsx shows modal
    ↓
User clicks Approve/Deny
    ↓
invoke('grant_permission'/'deny_permission')
    ↓
ToolExecutor.execute_tool()
    ↓
Tool.execute() → file operation / bash command
    ↓
Result returned to AgentEngine
    ↓
Context updated, LLM continues
```

### 3. Session Switching Flow

```
User clicks session in SessionBar
    ↓
sessionStore.switchSession(id)
    ↓
invoke('get_session_messages', { sessionId: id })
    ↓
session_commands.rs → SessionManager.get_messages()
    ↓
Storage.get_messages() → SQLite query
    ↓
Transform to ChatMessage format
    ↓
useAgentStore.setMessages() → updates UI
```

---

## Key Relationships

### Dependency Graph (Simplified)

```
lib.rs
├── commands/
│   ├── agent_commands → agent/engine, state/app_state
│   ├── session_commands → session/manager
│   ├── settings_commands → provider/manager, state/app_state
│   └── vault_commands → vault/keychain
├── agent/
│   ├── engine → provider/traits, tool_executor, context, state/app_state
│   ├── tool_executor → tool_registry, permission, events
│   ├── tool_registry → tool_trait
│   ├── context → (builds system prompt)
│   └── builtin_tools/* → tool_trait
├── session/
│   ├── manager → storage
│   └── storage → rusqlite
├── state/
│   └── app_state → cost/tracker
├── vault/
│   └── keychain → keyring crate
├── cost/
│   └── tracker → (calculates pricing)
└── provider/
    ├── traits → types
    ├── anthropic → traits, types, streaming
    ├── openai → traits, types, streaming
    └── google → traits, types, streaming
```

### Frontend-Backend Bridge

```
Frontend (React)
├── stores/agentStore ←→ events.rs (WebSocket-like)
├── stores/sessionStore ←→ session_commands.rs (RPC)
├── stores/settingsStore ←→ settings_commands.rs (RPC)
└── stores/costStore ←→ events.rs + cost_commands.rs
```

---

## File Summary Statistics

### Backend (Rust) - 72 Files Total

| Module | Files | Purpose | Phase Added |
|--------|-------|---------|-------------|
| Core | 5 | lib, main, errors, events, logging | 1.0 |
| Agent | 10 | Tool system, engine, context, **create_task tool** | 1.0 + **2.1** |
| Built-in Tools | 9 | File, bash, grep, glob tools + **create_task** | 1.0 + **2.1** |
| Commands | 11 | Tauri command handlers + **task, recipe, model, ide, worker cmds** | 1.0 + **2.1-3.2** |
| Provider | 7 | LLM API abstractions | 1.0 |
| Session | 4 | Persistence layer | 1.0 |
| State | 2 | Runtime state | 1.0 |
| Vault | 2 | Secure storage | 1.0 |
| Cost | 2 | Usage tracking | 1.0 |
| **DB** | **4** | **tasks, model_configs, autodev_instructions** | **2.1** |
| **Recipes** | **6** | **parser, runner, scheduler, defaults, types** | **2.1** |
| **IDE** | **7** | **file_tree, file_reader, file_search, symbols, workspace** | **3.2** |

### Frontend (TypeScript) - 48 Files Total

| Module | Files | Purpose | Phase Added |
|--------|-------|---------|-------------|
| Core | 3 | App, main, Vite config | 1.0 |
| Stores | 7 | Zustand stores + **task, worker, recipe, model, ide, ui** | 1.0 + **2.1-3.2** |
| Tabs | 5 | Chat, Settings + **AutoDevPanel, Recipes, IDE** | 1.0 + **2.1-3.2** |
| Components | 28 | UI components + **autodev, workers, recipes, ide, rich tool displays** | 1.0 + **2.1-3.2** |
| Utils | 2 | Tool output parsing + **openInIde** | 1.0 + **3.2** |

---

## Architecture Decisions

### 1. Rust-Native PA Mode

**Decision**: Implement PA mode directly in Rust rather than delegating to Python sidecar.

**Rationale**:
- 100-500ms lower latency
- Works offline with Ollama
- Survives sidecar crashes
- Simpler for single-user desktop use

### 2. Event-Driven Streaming

**Decision**: Use Tauri events for streaming rather than HTTP SSE.

**Rationale**:
- Native integration with Tauri
- No separate port management
- Automatic reconnection
- Type-safe payloads

### 3. SQLite for Sessions

**Decision**: Use SQLite (via rusqlite) for session persistence.

**Rationale**:
- Single file, easy backup
- No external dependencies
- Good performance for chat history
- ACID compliance

### 4. OS Keychain for Secrets

**Decision**: Use OS native keychain rather than file-based storage.

**Rationale**:
- Platform-native security
- Encrypted at rest
- Integrates with OS credential managers
- Industry standard practice

### 5. Zustand for Frontend State

**Decision**: Use Zustand over Redux/Context.

**Rationale**:
- Minimal boilerplate
- Excellent TypeScript support
- Built-in persistence helpers
- Small bundle size

---

## Completed Phases Summary

### Phase 2.1 — Vision Core ✅ (Shipped)

**Backend**:
- `src-tauri/src/db/` - Tasks, model configs, AUTODEV.md storage
- `src-tauri/src/recipes/` - Recipe parser, runner, scheduler, defaults
- `src-tauri/src/agent/builtin_tools/create_task.rs` - Delegate Mode tool
- `src-tauri/src/commands/task_commands.rs` - Task management commands
- `src-tauri/src/commands/recipe_commands.rs` - Recipe commands

**Frontend**:
- `src/stores/taskStore.ts`, `workerStore.ts`, `recipeStore.ts`
- `src/tabs/AutoDevPanel.tsx` - Kanban board with 4 columns
- `src/components/autodev/KanbanBoard.tsx`, `TaskCard.tsx`, `AgentRoster.tsx`
- `src/components/chat/PaModeToggle.tsx` - 3-mode segmented control

### Phase 2.2 — Production Scale ⏸️ (Partial)

**Implemented**:
- Worker Management UI (`WorkerTable.tsx`, `WorkerCreateModal.tsx`)
- Recipe-Worker binding

**Pending**:
- Full recipe CRUD UI
- Real-time Tauri events for task updates

### Phase 2.3 — Polish ✅ (Shipped)

**Backend**:
- `src/db/model_configs.rs` - Per-model settings
- `src/commands/model_config_commands.rs`
- `src/commands/github_mcp_commands.rs` - MCP server config
- `src/commands/recipe_commands.rs` - `generate_recipe_yaml`

**Frontend**:
- `src/stores/modelConfigStore.ts`
- `src/components/settings/ModelConfigSection.tsx`
- `src/components/settings/GitHubMCPCard.tsx`
- `src/components/recipes/RecipeEditor.tsx` - AI generation panel
- `src/components/chat/ContextMeter.tsx` - Token usage indicator

### Phase 3.2 — IDE Core ✅ (Shipped)

**Backend** (`src-tauri/src/ide/`):
- `file_tree.rs` - Git status overlay, lazy expansion
- `file_reader.rs` - Encoding detection, pagination
- `file_search.rs` - Fuzzy search, content search
- `symbols.rs` - Regex-based symbol extraction
- `workspace.rs` - Tab persistence

**Commands** (`commands/ide_commands.rs`):
- `read_file_tree`, `read_file_content`, `get_file_metadata`
- `search_files`, `search_file_contents`
- `get_symbol_tree`
- `get_ide_workspace_state`, `set_ide_workspace_state`

**Frontend**:
- `src/stores/ideStore.ts` - IDE state management
- `src/stores/uiStore.ts` - Global tab switching
- `src/tabs/IDE.tsx` - Full IDE layout
- `src/components/ide/FileTree.tsx` - File explorer
- `src/components/ide/TabBar.tsx` - Tab bar with close buttons
- `src/components/ide/CodeViewer.tsx` - Monaco viewer
- `src/components/ide/FileSearch.tsx` - Quick-open (Ctrl/⌘+P)
- `src/components/ide/SymbolOutline.tsx` - Symbol navigation
- `src/components/ide/IdeStatusBar.tsx` - Status bar
- `src/utils/openInIde.ts` - Chat-to-IDE integration

---

## Future Extensions (Phase 3.3+)

### Planned Modules

1. **Git Module** (`src-tauri/src/git/`)
   - Worktree management
   - Branch operations
   - Diff/status via git2-rs
   - **Git panel** as IDE sub-panel (not separate tab)

2. **Sidecar Integration**
   - HTTP client to core-orchestrator
   - Worker mode execution
   - Full multi-agent orchestration

---

## IDE Module (Phase 3.2)

### Backend (`src-tauri/src/ide/`)

| File              | Purpose                                                            |
|-------------------|--------------------------------------------------------------------|
| `mod.rs`          | Module declaration                                                  |
| `types.rs`        | DTOs mirrored in `src/stores/ideStore.ts`                          |
| `file_tree.rs`    | Recursive walk + git2 status overlay; lazy expansion via `has_more` |
| `file_reader.rs`  | Encoding detection (UTF-8/16), language id, binary sniff, pagination|
| `file_search.rs`  | Fuzzy name search + substring content search                        |
| `symbols.rs`      | Regex-based extractor — Rust / TS / JS / Python / Go                |
| `workspace.rs`    | Persists open tabs + expanded dirs in `config.db::app_settings`     |

Tauri commands (in `commands/ide_commands.rs`):

- `read_file_tree(path?, depth?, ignore_patterns?) → FileTreeNode`
- `read_file_content(path, start_line?, end_line?) → FileContent`
- `get_file_metadata(path) → FileMetadata`
- `search_files(query, path?, glob?, max_results?) → Vec<FileSearchResult>`
- `search_file_contents(query, path?, max_results?) → Vec<ContentSearchResult>`
- `get_symbol_tree(path) → SymbolTree`
- `get_ide_workspace_state() / set_ide_workspace_state(state)`

All commands resolve paths against the active workspace
(`workspace_commands::load_workspace_dir`) and reject paths that
escape it.

### Frontend (`src/components/ide/` + `src/stores/ideStore.ts`)

| Component           | Responsibility                                        |
|---------------------|--------------------------------------------------------|
| `FileTree.tsx`      | Collapsible tree with git badges + refresh             |
| `TabBar.tsx`        | Tabs + close + middle-click close                      |
| `CodeViewer.tsx`    | Monaco read-only viewer, custom Winter-is-Coming theme |
| `FileSearch.tsx`    | Ctrl/⌘+P overlay with arrow-key nav                    |
| `SymbolOutline.tsx` | Click-to-jump symbol list                              |
| `IdeStatusBar.tsx`  | Path, line count, size, language, encoding             |

**Store**: `useIdeStore` (Zustand) — sole authority for tabs, active tab,
expansion state, recent files, and tree cache. Actions: `init`,
`refreshTree`, `loadDirChildren`, `toggleDir`, `openFile`, `closeTab`,
`closeOtherTabs`, `closeAllTabs`, `switchTab`, `setSearchOpen`. Every
mutation that changes persisted state calls `set_ide_workspace_state`
in the background.

**Integration API**: external callers (e.g. chat tool-result renderers)
open a file in the IDE via

```ts
import { useIdeStore } from '@/stores/ideStore'
useIdeStore.getState().openFile('src/foo.ts', 42)
```

`gotoLine` is honoured by Monaco through `revealLineInCenter` on mount
and on subsequent jumps.

See `docs/development/PHASE_3_2_IDE_NOTES.md` for the deviation log
(regex-based symbols as a first cut, `.gitignore` parsing deferred to
Phase 3.3, intentional lack of inline editing, etc.).

---

## Auto-Updater System (Phase 5.3)

### Backend: `src-tauri/src/updater/`

The updater is a fully custom Rust module (not `tauri-plugin-updater`) providing:

| File | Role |
|------|------|
| `mod.rs` | `UpdaterState` (shared Tauri managed state), `start_background_check` (4-hour ticker), re-exports |
| `check.rs` | Queries GitHub Releases API, semver comparison, channel filtering (stable/beta/nightly), `PlatformAsset` map |
| `download.rs` | Streaming HTTP download with Range header resumption, real-time progress events via mpsc channel, `~/.autodev/updates/` cache dir |
| `verify.rs` | SHA-256 checksum verification (via `sha2`), Ed25519 signature verification (via `ed25519-dalek`), compile-time embedded public key |
| `patch.rs` | Pure-Rust bsdiff4 reader for delta patches; ADEV magic format header |
| `install.rs` | Platform-specific install scheduling: Windows (RunOnce batch/.bat), macOS (LaunchAgent plist + shell script), Linux (AppImage copy / pkexec deb install); update history in `~/.autodev/update_history.json` |
| `persistence.rs` | Pre-update integrity check (config.db, analytics.db, sessions/, plans/, skills/), restore point JSON, post-update cleanup |

**Tauri events emitted:**
- `updater:available` — new version found by background check
- `updater:critical` — critical update requiring immediate action
- `updater:progress` — download progress (DownloadProgress struct)
- `updater:ready` — download + verification complete
- `updater:error` — download or verification failure

**8 Tauri commands** registered in `commands/updater_commands.rs`:
`check_for_update`, `get_cached_update`, `download_update`, `get_download_progress`, `install_update`, `skip_version`, `get_update_history`, `verify_data_integrity`

`UpdaterState` is registered as `.manage(UpdaterState::new())` in `main.rs`. Background check starts in the setup hook via `tauri::async_runtime::spawn`.

### Frontend: Update UI

| File | Role |
|------|------|
| `src/stores/updateStore.ts` | Zustand store; subscribes to all 5 Tauri events, exposes `checkForUpdate`, `startDownload`, `installUpdate`, `skipVersion`, `loadHistory`, `verifyIntegrity` |
| `src/components/update/UpdateToast.tsx` | Non-intrusive bottom-right toast; critical updates use orange variant and disable dismiss |
| `src/components/update/UpdateDialog.tsx` | Full modal: release notes, download progress bar, speed indicator, signature status, Install & Restart button |
| `src/components/settings/UpdateSettings.tsx` | Settings panel: current version, last checked, channel selector, manual check trigger, data integrity report, update history viewer |

Both `UpdateToast` and `UpdateDialog` are mounted at the App root (`App.tsx`). `useUpdateStore.init()` called on mount; `cleanup()` on unmount.

### Installer Artifacts

| Platform | Format | Build Script |
|----------|--------|-------------|
| Windows | MSI (WiX v4) | `installers/wix/build-msi.ps1` |
| Windows | NSIS EXE | Tauri built-in |
| macOS | DMG | `installers/dmg/create-dmg.sh` |
| Linux | AppImage | `installers/appimage/build-appimage.sh` |
| Linux | .deb | `installers/deb/` (control, postinst, prerm) |
| Linux | .rpm | `installers/rpm/autodev.spec` |

**Top-level scripts**: `scripts/build-installers.sh` (OS-detecting orchestrator), `scripts/build-windows.ps1`, `scripts/build-macos.sh`, `scripts/build-linux.sh`

**GitHub Actions**: `.github/workflows/build-release.yml` (multi-platform release on tag push) + `.github/workflows/ci.yml` (PR checks: Rust lint/test, frontend typecheck/lint, build smoke tests on Windows + Linux)

**Custom WiX fragment** (`installers/wix/fragments/AutoDevExtensions.wxs`): firewall exception for port 8767, `.autodev` file association, `AUTODEV_HOME` environment variable.

**Entitlements** (`installers/dmg/entitlements.plist`): WebKit JIT, unsigned executable memory, library validation disabled, network client + server, filesystem read-write, subprocess inherit.

**IT Runbook**: `docs/IT_RUNBOOK.md` covers system requirements, silent install, network requirements, user data layout, auto-updater management, proxy config, uninstall, troubleshooting, and enterprise deployment (Group Policy, MDM, pre-configured API keys).

---

## Conclusion

This document provides a comprehensive map of the AutoDev codebase. Key architectural principles:

1. **Separation of Concerns**: Each module has a clear, focused responsibility
2. **Type Safety**: Heavy use of TypeScript and Rust's type systems
3. **Event-Driven**: Frontend and backend communicate via typed events
4. **Extensible Tools**: Tool trait makes adding new capabilities straightforward
5. **Secure by Default**: API keys in OS keychain, permission system for tools
6. **Safe Updates**: Pre-update integrity check, Ed25519 signature verification, OS-native install scheduling, user data never touched by installers

For questions or contributions, refer to the implementation files directly - this document is a guide, but the code is the source of truth.
