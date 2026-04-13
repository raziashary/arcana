# Goose Agent System - Deep Dive

> **Part of**: Goose Architecture Documentation  
> **Focus**: Agent implementation and core loop  
> **Crate**: `goose` (core)  
> **Generated**: 2025

---

## Table of Contents

1. [Agent Architecture](#agent-architecture)
2. [Agent Structure](#agent-structure)
3. [Reply Flow](#reply-flow)
4. [Context Management](#context-management)
5. [Tool Execution](#tool-execution)
6. [Security System](#security-system)
7. [Retry Logic](#retry-logic)
8. [Extension Management](#extension-management)
9. [Event System](#event-system)

---

## Agent Architecture

### High-Level Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    Agent.reply()                             │
│                                                              │
│  Input: Vec<Message>                                        │
│  Output: BoxStream<Result<Message>>                         │
│                                                              │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              1. Prepare Reply Context                          │
│                                                              │
│  - Get system prompt                                         │
│  - Load extensions                                           │
│  - Get available tools                                     │
│  - Set goose mode                                           │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              2. Run Agent Loop                                 │
│                                                              │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐            │
│  │  Start   │───►│ Provider │───►│ Stream   │            │
│  │          │    │ complete │    │ Response │            │
│  └──────────┘    └────┬─────┘    └────┬─────┘            │
│                        │              │                     │
│                        │              ▼                     │
│                        │        ┌──────────┐              │
│                        │        │ Content  │              │
│                        │        │ or Tool  │              │
│                        │        │ Request  │              │
│                        │        └────┬─────┘              │
│                        │             │                     │
│                        │        ┌────┴────┐               │
│                        │        │         │                │
│                        │        ▼         ▼                │
│                        │   ┌────────┐ ┌──────────┐       │
│                        │   │ Text   │ │ Tool     │       │
│                        │   │        │ │ Request  │       │
│                        │   └────────┘ └────┬─────┘       │
│                        │                  │               │
│                        │                  ▼               │
│                        │        ┌──────────────────┐      │
│                        │        │ Security Checks  │      │
│                        │        │ Permission Check │      │
│                        │        └────────┬─────────┘      │
│                        │                 │                │
│                        │        ┌────────┴────────┐     │
│                        │        │                  │     │
│                        │        ▼                  ▼     │
│                        │   ┌────────┐        ┌────────┐ │
│                        │   │ Execute│        │Confirm │ │
│                        │   │ Tool   │        │ User   │ │
│                        │   └────┬───┘        └────┬───┘ │
│                        │        │                  │     │
│                        │        └────────┬─────────┘     │
│                        │                 │                │
│                        │        ┌────────┴────────┐     │
│                        │        │   Tool Result     │     │
│                        │        └────────┬─────────┘     │
│                        │                 │                │
│                        │                 ▼                │
│                        │        ┌──────────────────┐      │
│                        └──────►│ Add to Messages  │      │
│                                 │ Continue Loop    │      │
│                                 └──────────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

---

## Agent Structure

**File**: `crates/goose/src/agents/agent.rs`

### Core Agent Struct

```rust
/// The main goose Agent - manages conversation flow and tool execution
pub struct Agent {
    // ═══════════════════════════════════════════════════════
    // Core LLM Provider
    // ═══════════════════════════════════════════════════════
    
    /// Shared provider for LLM calls
    provider: SharedProvider,
    
    /// Provider retry configuration
    provider_retry_config: RetryConfig,
    
    // ═══════════════════════════════════════════════════════
    // Configuration
    // ═══════════════════════════════════════════════════════
    
    /// Agent configuration
    pub config: AgentConfig,
    
    /// Current operating mode (Chat, Agent, Plan)
    current_goose_mode: Mutex<GooseMode>,
    
    // ═══════════════════════════════════════════════════════
    // Extension Management
    // ═══════════════════════════════════════════════════════
    
    /// Manages extensions (tools)
    pub extension_manager: Arc<ExtensionManager>,
    
    /// Built-in extensions
    builtin_extensions: Mutex<Vec<String>>,
    
    /// Extension malware checker
    malware_checker: MalwareChecker,
    
    // ═══════════════════════════════════════════════════════
    // Tool Management
    // ═══════════════════════════════════════════════════════
    
    /// Final output tool (for structured responses)
    final_output_tool: Arc<Mutex<Option<FinalOutputTool>>>,
    
    /// Frontend tools (UI integration)
    frontend_tools: Mutex<HashMap<String, FrontendTool>>,
    
    /// Frontend instructions
    frontend_instructions: Mutex<Option<String>>,
    
    /// Tool confirmation router
    tool_confirmation_router: ToolConfirmationRouter,
    
    /// Tool result channel
    tool_result_tx: mpsc::Sender<(String, ToolResult<CallToolResult>)>,
    
    /// Tool result receiver
    tool_result_rx: Mutex<mpsc::Receiver<(String, ToolResult<CallToolResult>)>>,
    
    // ═══════════════════════════════════════════════════════
    // Prompt Management
    // ═══════════════════════════════════════════════════════
    
    /// Prompt manager for system prompts
    prompt_manager: Mutex<PromptManager>,
    
    /// Recipe settings (if running a recipe)
    recipe_settings: Mutex<Option<Settings>>,
    
    // ═══════════════════════════════════════════════════════
    // Session Management
    // ═══════════════════════════════════════════════════════
    
    /// Session manager
    session_manager: Arc<SessionManager>,
    
    /// Current session
    current_session: Mutex<Option<Session>>,
    
    /// Session context
    session_context: SessionContext,
    
    /// Container for isolation
    container: Arc<Container>,
    
    // ═══════════════════════════════════════════════════════
    // Security & Inspection
    // ═══════════════════════════════════════════════════════
    
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
    
    // ═══════════════════════════════════════════════════════
    // Action & Retry
    // ═══════════════════════════════════════════════════════
    
    /// Action required manager
    action_required_manager: Arc<ActionRequiredManager>,
    
    /// Retry manager
    retry_manager: RetryManager,
    
    /// Cancellation token
    cancellation_token: CancellationToken,
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
    
    /// Maximum turns (iterations)
    pub max_turns: u32,
    
    /// Enable tool cache
    pub enable_tool_cache: bool,
    
    /// Context compaction threshold
    pub compaction_threshold: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GooseMode {
    /// Chat-only mode (no tools)
    Chat,
    
    /// Full agent mode with tools
    Agent,
    
    /// Plan mode - step by step execution
    Plan,
    
    /// Automatic mode - minimal confirmations
    Auto,
    
    /// Goose mode (deprecated)
    #[deprecated]
    Goose,
}

#[derive(Clone, Debug)]
pub enum GoosePlatform {
    GooseDesktop,
    GooseCli,
}
```

### Creating an Agent

```rust
impl Agent {
    /// Create a new agent with configuration
    pub async fn new(
        provider: Box<dyn Provider>,
        config: AgentConfig,
    ) -> Result<Self> {
        let provider = Arc::new(provider);
        
        // Create extension manager
        let extension_manager = Arc::new(
            ExtensionManager::new(config.permission_manager.clone()).await?
        );
        
        // Create channel for tool results
        let (tool_result_tx, tool_result_rx) = mpsc::channel(100);
        
        // Initialize security inspectors
        let security_inspectors: Vec<Box<dyn SecurityInspector>> = vec![
            Box::new(RepetitionInspector::new()),
            Box::new(EgressInspector::new()),
            Box::new(AdversaryInspector::new()),
        ];
        
        Ok(Self {
            provider,
            provider_retry_config: RetryConfig::default(),
            config,
            current_goose_mode: Mutex::new(GooseMode::Agent),
            extension_manager,
            builtin_extensions: Mutex::new(Vec::new()),
            malware_checker: MalwareChecker::new(),
            final_output_tool: Arc::new(Mutex::new(None)),
            frontend_tools: Mutex::new(HashMap::new()),
            frontend_instructions: Mutex::new(None),
            tool_confirmation_router: ToolConfirmationRouter::new(),
            tool_result_tx,
            tool_result_rx: Mutex::new(tool_result_rx),
            prompt_manager: Mutex::new(PromptManager::new()),
            recipe_settings: Mutex::new(None),
            session_manager: config.session_manager.clone(),
            current_session: Mutex::new(None),
            session_context: SessionContext::default(),
            container: Arc::new(Container::new()),
            tool_inspection_manager: ToolInspectionManager::new(),
            repetition_inspector: Arc::new(RepetitionInspector::new()),
            security_inspectors,
            adversary_inspector: Arc::new(AdversaryInspector::new()),
            egress_inspector: Arc::new(EgressInspector::new()),
            permission_inspector: Arc::new(PermissionInspector::new(
                config.permission_manager.clone()
            )),
            action_required_manager: Arc::new(ActionRequiredManager::new()),
            retry_manager: RetryManager::new(),
            cancellation_token: CancellationToken::new(),
        })
    }
}
```

---

## Reply Flow

### Main Entry Point

```rust
impl Agent {
    /// Main entry point for agent responses
    /// 
    /// Takes user messages and returns a stream of responses
    pub async fn reply(
        &self,
        messages: Vec<Message>,
    ) -> Result<BoxStream<'_, Result<Message, anyhow::Error>>> {
        // Prepare context for reply
        let context = self.prepare_reply_context(messages).await?;
        
        // Get available tools
        let tools = self.get_prefixed_tools().await?;
        
        // Get system prompt
        let system_prompt = self.get_system_prompt().await?;
        
        // Run the agent loop
        let stream = self.run_agent_loop(context, tools, system_prompt).await?;
        
        Ok(stream)
    }
}
```

### Reply Context Preparation

```rust
pub struct ReplyContext {
    /// Conversation state
    pub conversation: Conversation,
    
    /// Available tools
    pub tools: Vec<Tool>,
    
    /// Toolshim tools (for providers without native tool support)
    pub toolshim_tools: Vec<Tool>,
    
    /// System prompt
    pub system_prompt: String,
    
    /// Current goose mode
    pub goose_mode: GooseMode,
    
    /// Tool call cutoff index
    pub tool_call_cut_off: usize,
    
    /// Initial messages
    pub initial_messages: Vec<Message>,
    
    /// Current turn count
    pub turn_count: Arc<AtomicU32>,
    
    /// Total token count
    pub total_tokens: Arc<AtomicUsize>,
}

impl Agent {
    async fn prepare_reply_context(
        &self,
        messages: Vec<Message>,
    ) -> Result<ReplyContext> {
        // Get or create session
        let session = self.get_or_create_session().await?;
        
        // Get enabled extensions
        let extensions = get_enabled_extensions(&session).await?;
        
        // Add extensions to manager
        for ext in extensions {
            self.add_extension(ext).await?;
        }
        
        // Get tools from extensions
        let tools = self.list_tools().await?;
        
        // Check for toolshim
        let toolshim_tools = if self.provider.supports_tool_shim() {
            tools.clone()
        } else {
            vec![]
        };
        
        // Build system prompt
        let system_prompt = self.build_system_prompt().await?;
        
        // Create conversation
        let conversation = Conversation::new(messages);
        
        // Get current mode
        let goose_mode = *self.current_goose_mode.lock().await;
        
        Ok(ReplyContext {
            conversation,
            tools,
            toolshim_tools,
            system_prompt,
            goose_mode,
            tool_call_cut_off: 0,
            initial_messages: messages.clone(),
            turn_count: Arc::new(AtomicU32::new(0)),
            total_tokens: Arc::new(AtomicUsize::new(0)),
        })
    }
}
```

### Agent Loop Implementation

```rust
impl Agent {
    async fn run_agent_loop(
        &self,
        context: ReplyContext,
        tools: Vec<Tool>,
        system_prompt: String,
    ) -> Result<BoxStream<'_, Result<Message, anyhow::Error>>> {
        let provider = self.provider.clone();
        let extension_manager = self.extension_manager.clone();
        let tool_confirmation_router = self.tool_confirmation_router.clone();
        let session_manager = self.session_manager.clone();
        let action_required_manager = self.action_required_manager.clone();
        let cancellation_token = self.cancellation_token.clone();
        
        // Create the streaming response
        let stream = async_stream::try_stream! {
            let mut conversation = context.conversation;
            let mut messages = context.initial_messages;
            let mut turn_count = 0;
            
            loop {
                // Check cancellation
                if cancellation_token.is_cancelled() {
                    yield Message::system("Operation cancelled");
                    break;
                }
                
                // Check max turns
                turn_count += 1;
                if turn_count > DEFAULT_MAX_TURNS {
                    yield Message::system("Maximum turns reached");
                    break;
                }
                
                // Check if compaction is needed
                if check_if_compaction_needed(&messages) {
                    yield Message::thinking(COMPACTION_THINKING_TEXT);
                    messages = compact_messages(messages).await?;
                }
                
                // Call provider
                let response = provider.complete(
                    &ModelConfig::default(),
                    Some(system_prompt.clone()),
                    messages.clone(),
                    tools.clone(),
                ).await;
                
                // Handle provider response
                match response {
                    Ok(provider_response) => {
                        // Yield content messages
                        for content in &provider_response.content {
                            match content {
                                MessageContent::Text(text) => {
                                    yield Message::assistant(text);
                                }
                                MessageContent::ToolRequest(request) => {
                                    // Handle tool request
                                    yield Message::tool_request(&request.name, &request.arguments);
                                    
                                    // Execute tool
                                    let result = execute_tool_with_security(
                                        &extension_manager,
                                        &tool_confirmation_router,
                                        request,
                                        &action_required_manager,
                                    ).await?;
                                    
                                    yield Message::tool_result(&result);
                                    
                                    // Add to messages for next iteration
                                    messages.push(Message::assistant_with_tool(
                                        request,
                                    ));
                                    messages.push(Message::tool_result_message(
                                        &request.id,
                                        &result,
                                    ));
                                }
                                MessageContent::Thinking(thinking) => {
                                    yield Message::thinking(thinking);
                                }
                            }
                        }
                        
                        // Check if done (no tool requests)
                        if !provider_response.content.iter().any(|c| matches!(c, MessageContent::ToolRequest(_))) {
                            break;
                        }
                    }
                    Err(e) => {
                        // Retry logic
                        if let Some(retry_result) = retry_with_backoff(e).await {
                            continue; // Retry the iteration
                        }
                        
                        yield Message::error(&format!("Provider error: {}", e));
                        break;
                    }
                }
            }
            
            // Save session
            session_manager.save_session(&conversation).await?;
        };
        
        Ok(Box::pin(stream))
    }
}
```

---

## Context Management

### Conversation Compaction

**File**: `crates/goose/src/context_mgmt.rs`

```rust
/// Check if conversation needs compaction
pub fn check_if_compaction_needed(messages: &[Message]) -> bool {
    let total_tokens = estimate_token_count(messages);
    total_tokens > DEFAULT_COMPACTION_THRESHOLD
}

/// Compact conversation by summarizing older messages
pub async fn compact_messages(messages: Vec<Message>) -> Result<Vec<Message>> {
    // Keep recent messages (last N)
    const KEEP_RECENT: usize = 10;
    
    if messages.len() <= KEEP_RECENT {
        return Ok(messages);
    }
    
    let (older, recent) = messages.split_at(messages.len() - KEEP_RECENT);
    
    // Summarize older messages
    let summary = summarize_messages(older).await?;
    
    // Create new message list: summary + recent
    let mut compacted = vec![
        Message::system(&format!("Previous conversation summary: {}", summary)),
    ];
    compacted.extend_from_slice(recent);
    
    Ok(compacted)
}

async fn summarize_messages(messages: &[Message]) -> Result<String> {
    // Use a smaller/cheaper model for summarization
    let summarizer = create_summarizer_provider().await?;
    
    let prompt = format!(
        "Summarize the following conversation in 2-3 sentences, \
         focusing on the main topics and any decisions made:\n\n{}",
        format_messages(messages)
    );
    
    let response = summarizer.complete(
        &ModelConfig::default(),
        None,
        vec![Message::user(&prompt)],
        vec![],
    ).await?;
    
    Ok(response.content_text())
}
```

---

## Tool Execution

### Tool Execution Flow

```rust
/// Execute a tool with full security and permission checks
async fn execute_tool_with_security(
    extension_manager: &ExtensionManager,
    confirmation_router: &ToolConfirmationRouter,
    request: &ToolRequest,
    action_required_manager: &ActionRequiredManager,
) -> Result<CallToolResult> {
    // 1. Parse tool name
    let (extension_name, tool_name) = parse_tool_name(&request.name)?;
    
    // 2. Security inspection
    security_inspector.check_tool_request(request).await?;
    
    // 3. Permission check
    let permission = permission_inspector.check_permission(request).await?;
    
    match permission {
        PermissionCheckResult::Allowed => {
            // Execute directly
        }
        PermissionCheckResult::NeedsConfirmation => {
            // Request user confirmation
            let confirmed = action_required_manager
                .request_confirmation(&request.name, &request.arguments)
                .await?;
            
            if !confirmed {
                return Ok(CallToolResult::declined());
            }
        }
        PermissionCheckResult::Denied => {
            return Ok(CallToolResult::error("Tool execution denied"));
        }
    }
    
    // 4. Execute tool via extension manager
    let result = extension_manager.call_tool(request.clone()).await?;
    
    // 5. Post-execution security check
    security_inspector.check_tool_result(&result).await?;
    
    Ok(result)
}
```

### Tool Confirmation Router

```rust
pub struct ToolConfirmationRouter {
    /// Rules for auto-confirmation
    auto_confirm_rules: Vec<ConfirmationRule>,
    
    /// Rules for requiring confirmation
    require_confirm_rules: Vec<ConfirmationRule>,
    
    /// Denied tools
    denied_tools: HashSet<String>,
}

impl ToolConfirmationRouter {
    /// Determine if a tool needs confirmation
    pub fn needs_confirmation(&self, tool_name: &str, arguments: &Value) -> bool {
        // Check deny list first
        if self.denied_tools.contains(tool_name) {
            return true; // Will be denied
        }
        
        // Check auto-confirm rules
        for rule in &self.auto_confirm_rules {
            if rule.matches(tool_name, arguments) {
                return false; // Auto-confirm
            }
        }
        
        // Check require-confirm rules
        for rule in &self.require_confirm_rules {
            if rule.matches(tool_name, arguments) {
                return true; // Require confirmation
            }
        }
        
        // Default: require confirmation for write operations
        is_write_tool(tool_name)
    }
}
```

---

## Security System

### Security Inspectors

```rust
/// Trait for security inspectors
pub trait SecurityInspector: Send + Sync {
    /// Check a tool request before execution
    async fn check_request(&self, request: &ToolRequest) -> Result<SecurityCheckResult>;
    
    /// Check a tool result after execution
    async fn check_result(&self, result: &CallToolResult) -> Result<SecurityCheckResult>;
    
    /// Inspector name
    fn name(&self) -> &'static str;
}

/// Security check result
pub enum SecurityCheckResult {
    /// Pass - allow execution
    Pass,
    
    /// Warn - allow but warn user
    Warn(String),
    
    /// Block - deny execution
    Block(String),
}
```

### Egress Inspector

**File**: `crates/goose/src/security/egress_inspector.rs`

```rust
/// Inspects network egress from tool execution
pub struct EgressInspector {
    /// Allowed domains
    allowed_domains: Vec<String>,
    
    /// Blocked domains
    blocked_domains: Vec<String>,
    
    /// Allowed protocols
    allowed_protocols: Vec<String>,
}

impl SecurityInspector for EgressInspector {
    async fn check_request(&self, request: &ToolRequest) -> Result<SecurityCheckResult> {
        // Check if tool makes network requests
        if let Some(urls) = extract_urls_from_arguments(&request.arguments) {
            for url in urls {
                if self.is_blocked(&url) {
                    return Ok(SecurityCheckResult::Block(
                        format!("Access to {} is blocked", url)
                    ));
                }
            }
        }
        
        Ok(SecurityCheckResult::Pass)
    }
    
    fn name(&self) -> &'static str {
        "egress"
    }
}
```

### Adversary Inspector

**File**: `crates/goose/src/security/adversary_inspector.rs`

```rust
/// Detects potentially malicious tool usage
pub struct AdversaryInspector {
    /// Known attack patterns
    attack_patterns: Vec<Regex>,
    
    /// Suspicious argument patterns
    suspicious_patterns: Vec<Regex>,
}

impl SecurityInspector for AdversaryInspector {
    async fn check_request(&self, request: &ToolRequest) -> Result<SecurityCheckResult> {
        let args_str = serde_json::to_string(&request.arguments)?;
        
        // Check for attack patterns
        for pattern in &self.attack_patterns {
            if pattern.is_match(&args_str) {
                return Ok(SecurityCheckResult::Block(
                    "Potentially malicious request detected".to_string()
                ));
            }
        }
        
        // Check for suspicious patterns
        for pattern in &self.suspicious_patterns {
            if pattern.is_match(&args_str) {
                return Ok(SecurityCheckResult::Warn(
                    "Suspicious pattern detected".to_string()
                ));
            }
        }
        
        Ok(SecurityCheckResult::Pass)
    }
}
```

---

## Retry Logic

**File**: `crates/goose/src/agents/retry.rs`

```rust
/// Retry configuration
pub struct RetryConfig {
    /// Maximum retry attempts
    pub max_retries: u32,
    
    /// Base delay in milliseconds
    pub base_delay_ms: u64,
    
    /// Maximum delay in milliseconds
    pub max_delay_ms: u64,
    
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    
    /// Retryable error predicates
    pub retryable_errors: Vec<ErrorPredicate>,
}

/// Retry manager
pub struct RetryManager {
    config: RetryConfig,
}

impl RetryManager {
    /// Execute an operation with retry
    pub async fn retry<T, F, Fut>(
        &self,
        operation: F,
    ) -> Result<T, RetryError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, ProviderError>>,
    {
        let mut attempts = 0;
        let mut delay = self.config.base_delay_ms;
        
        loop {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    attempts += 1;
                    
                    // Check if we should retry
                    if attempts >= self.config.max_retries {
                        return Err(RetryError::MaxRetriesExceeded(error));
                    }
                    
                    if !self.is_retryable(&error) {
                        return Err(RetryError::NotRetryable(error));
                    }
                    
                    // Wait with exponential backoff
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    delay = (delay as f64 * self.config.backoff_multiplier) as u64;
                    delay = delay.min(self.config.max_delay_ms);
                }
            }
        }
    }
    
    fn is_retryable(&self, error: &ProviderError) -> bool {
        match error {
            ProviderError::RateLimited => true,
            ProviderError::ServerError(_) => true,
            ProviderError::NetworkError(_) => true,
            ProviderError::Timeout => true,
            _ => false,
        }
    }
}
```

---

## Extension Management

**File**: `crates/goose/src/agents/extension_manager.rs`

### Extension Manager

```rust
pub struct ExtensionManager {
    /// Active extension clients
    clients: Arc<Mutex<HashMap<String, ExtensionClient>>>,
    
    /// Cached tools
    tool_cache: Arc<RwLock<Vec<Tool>>>,
    
    /// Tool cache timestamp
    tool_cache_time: Arc<RwLock<Option<Instant>>>,
    
    /// Permission manager
    permission_manager: Arc<PermissionManager>,
    
    /// Platform extension manager
    platform_extension_manager: PlatformExtensionManager,
}

impl ExtensionManager {
    /// Add a new extension
    pub async fn add_extension(
        &self,
        config: ExtensionConfig,
    ) -> Result<ExtensionResult> {
        // Validate extension
        self.validate_extension(&config).await?;
        
        // Create client based on transport type
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
        
        // Test connection
        client.ping().await?;
        
        // Store client
        let name = config.name.clone();
        self.clients.lock().await.insert(name.clone(), client);
        
        // Invalidate tool cache
        self.invalidate_tool_cache();
        
        Ok(ExtensionResult {
            name,
            success: true,
            error: None,
        })
    }
    
    /// List all available tools
    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        // Check cache
        if let Some(cached) = self.get_cached_tools().await {
            return Ok(cached);
        }
        
        // Collect tools from all extensions
        let mut all_tools = Vec::new();
        let clients = self.clients.lock().await;
        
        for (name, client) in clients.iter() {
            match client.list_tools().await {
                Ok(tools) => {
                    // Prefix tools with extension name
                    for tool in tools {
                        let prefixed_tool = Tool {
                            name: format!("{}__{}", name, tool.name),
                            ..tool
                        };
                        all_tools.push(prefixed_tool);
                    }
                }
                Err(e) => {
                    warn!("Failed to list tools for {}: {}", name, e);
                }
            }
        }
        
        // Update cache
        self.update_tool_cache(all_tools.clone()).await;
        
        Ok(all_tools)
    }
    
    /// Call a tool
    pub async fn call_tool(
        &self,
        request: ToolRequest,
    ) -> Result<ToolResult<CallToolResult>> {
        let (extension_name, tool_name) = parse_tool_name(&request.name)?;
        
        // Get client
        let clients = self.clients.lock().await;
        let client = clients
            .get(&extension_name)
            .ok_or_else(|| anyhow!("Extension '{}' not found", extension_name))?;
        
        // Update request with unprefixed tool name
        let mut inner_request = request.clone();
        inner_request.name = tool_name;
        
        // Execute with timeout
        let result = tokio::time::timeout(
            Duration::from_secs(request.timeout_secs.unwrap_or(60)),
            client.call_tool(inner_request),
        ).await??;
        
        Ok(result)
    }
}
```

---

## Event System

### Agent Events

```rust
#[derive(Debug, Clone)]
pub enum AgentEvent {
    // ═════════════════════════════════════════════════════
    // Content Events
    // ═════════════════════════════════════════════════════
    
    /// Text content from LLM
    Text(String),
    
    /// Thinking/reasoning content
    Thinking(String),
    
    // ═════════════════════════════════════════════════════
    // Tool Events
    // ═════════════════════════════════════════════════════
    
    /// Tool request detected
    ToolRequest(ToolRequest),
    
    /// Tool result received
    ToolResult(ToolResult<CallToolResult>),
    
    /// Tool confirmation needed
    ToolConfirmation {
        tool_name: String,
        request: ToolRequest,
    },
    
    /// Tool execution started
    ToolStarted {
        tool_name: String,
        request_id: String,
    },
    
    /// Tool execution completed
    ToolCompleted {
        tool_name: String,
        request_id: String,
        duration_ms: u64,
    },
    
    // ═════════════════════════════════════════════════════
    // Session Events
    // ═════════════════════════════════════════════════════
    
    /// Session updated
    SessionUpdate(Session),
    
    /// Session saved
    SessionSaved(String),
    
    /// Conversation compaction starting
    CompactionStarting,
    
    /// Conversation compaction complete
    CompactionComplete {
        original_tokens: usize,
        compacted_tokens: usize,
    },
    
    // ═════════════════════════════════════════════════════
    // Action Events
    // ═════════════════════════════════════════════════════
    
    /// Action required
    ActionRequired(ActionRequiredData),
    
    /// Action completed
    ActionCompleted(String),
    
    // ═════════════════════════════════════════════════════
    // Task Events
    // ═════════════════════════════════════════════════════
    
    /// Task started
    TaskStarted {
        task_id: String,
        description: String,
    },
    
    /// Task progress
    TaskProgress {
        task_id: String,
        progress: f32,
        message: String,
    },
    
    /// Task completed
    TaskCompleted {
        task_id: String,
        result: String,
    },
    
    /// Task failed
    TaskFailed {
        task_id: String,
        error: String,
    },
    
    // ═════════════════════════════════════════════════════
    // Error Events
    // ═════════════════════════════════════════════════════
    
    /// Error occurred
    Error(String),
    
    /// Provider error
    ProviderError(ProviderError),
    
    /// Tool error
    ToolError {
        tool_name: String,
        error: String,
    },
    
    // ═════════════════════════════════════════════════════
    // Lifecycle Events
    // ═════════════════════════════════════════════════════
    
    /// Reply started
    ReplyStarted,
    
    /// Reply completed
    ReplyCompleted,
    
    /// Reply cancelled
    ReplyCancelled,
}
```

---

## Summary

The Goose Agent system provides:

1. **Structured Agent Loop**: Clear flow from user input → LLM → tool execution → response
2. **Context Management**: Automatic conversation compaction for long sessions
3. **Security-First**: Multiple layers of security inspection
4. **Permission System**: Configurable confirmation levels
5. **Retry Logic**: Automatic retry with exponential backoff
6. **Event-Driven**: Rich event system for UI integration
7. **Extension-Based**: Tools loaded via MCP extensions
8. **Multi-Platform**: Supports CLI and Desktop modes

Key files:
- `crates/goose/src/agents/agent.rs` - Main agent (103KB, 2471 lines)
- `crates/goose/src/agents/extension_manager.rs` - Tool management (81KB)
- `crates/goose/src/agents/retry.rs` - Retry logic
- `crates/goose/src/security/` - Security inspectors
- `crates/goose/src/context_mgmt.rs` - Context compaction
