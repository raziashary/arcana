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
- Used by: `Chat.tsx`, `MessageBubble.tsx`, `ToolCallDisplay.tsx`

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

**Purpose**: Provider configuration and system prompt management.

**Key Components**:
```typescript
interface SettingsState {
  providers: Provider[]
  activeProvider: string | null
  activeModel: string | null
  systemPrompt: string | null
  defaultSystemPrompt: string
  isLoadingProviders: boolean
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  loadProviders: async () => {...}
  setActiveProvider: async (id) => {...}
  setSystemPrompt: async (prompt) => {...}
  loadDefaultSystemPrompt: async () => {...}
  resetSystemPrompt: async () => {...}
}))
```

**Relationships**:
- Calls Tauri commands: `list_providers`, `set_active_provider`, `get_system_prompt`, `set_system_prompt`, `get_default_system_prompt`, `vault_get`, `vault_set`
- Used by: `Settings.tsx`, `StatusBar.tsx`

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
- Displays in: `StatusBar.tsx`

---

### Tabs

#### `src/tabs/Chat.tsx`

**Purpose**: Main chat interface with streaming and tool display.

**Key Components**:
```typescript
export default function ChatTab() {
  const messages = useAgentStore(s => s.messages)
  const isStreaming = useAgentStore(s => s.isStreaming)
  
  const handleSend = async () => {
    // 1. Ensure session exists
    // 2. Set up event listeners
    // 3. Add user message to store
    // 4. Invoke send_message command
  }
  
  const handleCancel = async () => {
    await invoke('cancel_agent', { sessionId })
  }
}
```

**Layout**:
- Left: `SessionBar` (200px)
- Center: Message list (`MessageBubble` components)
- Bottom: Input area with send/cancel buttons

**Relationships**:
- Uses: `useAgentStore`, `useSessionStore`, `useSettingsStore`, `useCostStore`
- Calls: `send_message`, `cancel_agent` commands
- Renders: `MessageBubble`, `PermissionDialog`, `SessionBar`

---

#### `src/tabs/Settings.tsx`

**Purpose**: Provider configuration and system prompt editing.

**Key Components**:
```typescript
export default function SettingsTab() {
  const store = useSettingsStore()
  
  return (
    <div>
      <SectionHeader title="Providers" />
      {store.providers.map(p => <ProviderCard key={p.id} providerId={p.id} />)}
      
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
- System prompt editor with save/reset

**Relationships**:
- Uses: `useSettingsStore`

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

export function parseToolOutput(toolName: string, output: string): ToolOutputType
export function detectLanguageFromPath(path: string): string
export function formatFileSize(bytes: number): string
```

**Relationships**:
- Used by: `ToolCallDisplay.tsx` to render appropriate component
- Supports: Syntax highlighting decisions

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

### Backend (Rust)

| Module | Files | Purpose |
|--------|-------|---------|
| Core | 5 | lib, main, errors, events, logging |
| Agent | 9 | Tool system, execution engine, context |
| Built-in Tools | 8 | File, bash, grep, glob tools |
| Commands | 5 | Tauri command handlers |
| Provider | 7 | LLM API abstractions |
| Session | 4 | Persistence layer |
| State | 2 | Runtime state |
| Vault | 2 | Secure storage |
| Cost | 2 | Usage tracking |

### Frontend (TypeScript)

| Module | Files | Purpose |
|--------|-------|---------|
| Core | 3 | App, main, Vite config |
| Stores | 4 | Zustand state management |
| Tabs | 3 | Chat, Settings, IDE |
| Components | 12 | UI components |
| Utils | 1 | Tool output parsing |

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

## Future Extensions (Phase 2+)

### Planned Modules

1. **Task Database** (`src-tauri/src/db/tasks.rs`)
   - SQLite table for worker task management
   - `create_task` tool for delegate mode

2. **Git Module** (`src-tauri/src/git/`)
   - Worktree management
   - Branch operations
   - Diff/status via git2-rs

3. **Sidecar Integration**
   - HTTP client to core-orchestrator
   - Worker mode execution
   - Recipe management

4. **IDE Tab** (`src/tabs/IDE.tsx`)
   - File explorer
   - Monaco editor integration
   - Git panel

---

## Conclusion

This document provides a comprehensive map of the AutoDev codebase. Key architectural principles:

1. **Separation of Concerns**: Each module has a clear, focused responsibility
2. **Type Safety**: Heavy use of TypeScript and Rust's type systems
3. **Event-Driven**: Frontend and backend communicate via typed events
4. **Extensible Tools**: Tool trait makes adding new capabilities straightforward
5. **Secure by Default**: API keys in OS keychain, permission system for tools

For questions or contributions, refer to the implementation files directly - this document is a guide, but the code is the source of truth.
