# Goose Provider System - Deep Dive

> **Part of**: Goose Architecture Documentation  
> **Focus**: LLM provider implementations  
> **Crate**: `goose` (providers module)  
> **Generated**: 2025

---

## Table of Contents

1. [Provider Architecture](#provider-architecture)
2. [Provider Trait](#provider-trait)
3. [Provider Registry](#provider-registry)
4. [Model Configuration](#model-configuration)
5. [Provider Implementations](#provider-implementations)
6. [Tool Shim](#tool-shim)
7. [Authentication](#authentication)
8. [Error Handling](#error-handling)
9. [Retry System](#retry-system)

---

## Provider Architecture

### Overview

Goose supports 50+ LLM providers through a unified trait system. This allows the agent to work with any LLM backend while maintaining consistent behavior.

```
┌─────────────────────────────────────────────────────────────┐
│                    Provider Architecture                     │
│                                                              │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                    Agent                               │  │
│  │  - Uses Provider trait                                 │  │
│  │  - Provider-agnostic logic                             │  │
│  └─────────────────────┬─────────────────────────────────┘  │
│                        │                                      │
│                        ▼                                      │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              Provider Trait                            │  │
│  │                                                      │  │
│  │  - complete()           - Non-streaming call         │  │
│  │  - stream_complete()    - Streaming call             │  │
│  │  - supports_tool_shim() - Tool compatibility         │  │
│  └─────────────────────┬─────────────────────────────────┘  │
│                        │                                      │
│          ┌─────────────┼─────────────┐                       │
│          │             │             │                       │
│          ▼             ▼             ▼                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                   │
│  │Anthropic│  │ OpenAI   │  │  Azure   │    50+ Providers   │
│  │ Claude  │  │  GPT-4   │  │ OpenAI   │                   │
│  └──────────┘  └──────────┘  └──────────┘                   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Provider Registry Pattern

```
┌─────────────────────────────────────────────────────────────┐
│                  Provider Registry                           │
│                                                              │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  HashMap<String, Box<dyn ProviderFactory>>             │  │
│  │                                                      │  │
│  │  "anthropic"  -> AnthropicFactory                    │  │
│  │  "openai"     -> OpenAiFactory                       │  │
│  │  "azure"      -> AzureFactory                        │  │
│  │  "databricks" -> DatabricksFactory                   │  │
│  │  ... (50+ more providers)                            │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                              │
│  create_provider(name, config):                              │
│    1. Look up factory by name                               │
│    2. Call factory.create(config)                           │
│    3. Return Box<dyn Provider>                              │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## Provider Trait

**File**: `crates/goose/src/providers/base.rs`

### Core Trait Definition

```rust
/// Core trait for LLM providers
#[async_trait]
pub trait Provider: Send + Sync {
    /// Get the provider name
    fn name(&self) -> String;
    
    /// Get the provider's model configuration
    fn get_model_config(&self) -> &ModelConfig;
    
    /// Perform a non-streaming completion
    /// 
    /// Returns the complete response in one call
    async fn complete(
        &self,
        model_config: &ModelConfig,
        system: Option<String>,
        messages: Vec<Message>,
        tools: Vec<Tool>,
    ) -> Result<ProviderResponse, ProviderError>;
    
    /// Perform a streaming completion
    /// 
    /// Returns a stream of response chunks
    async fn stream_complete(
        &self,
        model_config: &ModelConfig,
        system: Option<String>,
        messages: Vec<Message>,
        tools: Vec<Tool>,
    ) -> Result<BoxStream<'_, Result<ProviderStreamChunk, ProviderError>>, ProviderError>;
    
    /// Check if provider supports tool shim
    /// 
    /// Some providers don't support native tool calling, so we
    /// use a "shim" that prompts the model to output tool JSON
    fn supports_tool_shim(&self) -> bool {
        false
    }
    
    /// Estimate usage (cost) for tokens
    fn estimate_usage(
        &self,
        input_tokens: usize,
        output_tokens: usize,
    ) -> ProviderUsage;
    
    /// Get available models from provider
    async fn get_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(vec![])
    }
    
    /// Validate configuration
    async fn validate_config(&self) -> Result<(), ProviderError> {
        Ok(())
    }
}

/// Type alias for shared provider
pub type SharedProvider = Arc<dyn Provider>;
```

### Provider Response

```rust
/// Response from a provider completion
#[derive(Debug, Clone)]
pub struct ProviderResponse {
    /// Message content blocks
    pub content: Vec<MessageContent>,
    
    /// Token usage information
    pub usage: ProviderUsage,
    
    /// Model that actually generated the response (may differ from requested)
    pub model: String,
    
    /// Provider-specific metadata
    pub metadata: Option<Value>,
}

/// Streaming response chunk
#[derive(Debug, Clone)]
pub enum ProviderStreamChunk {
    /// Text content
    Text(String),
    
    /// Tool request (detected in stream)
    ToolRequest(ToolRequest),
    
    /// Thinking/reasoning content
    Thinking(String),
    
    /// Usage information (typically at end)
    Usage(ProviderUsage),
    
    /// Stream complete
    Done,
    
    /// Error in stream
    Error(String),
}

/// Token usage information
#[derive(Debug, Clone, Default)]
pub struct ProviderUsage {
    /// Input tokens consumed
    pub input_tokens: usize,
    
    /// Output tokens generated
    pub output_tokens: usize,
    
    /// Total tokens
    pub total_tokens: usize,
    
    /// Estimated cost in USD
    pub estimated_cost: Option<f64>,
}
```

---

## Provider Registry

**File**: `crates/goose/src/providers/provider_registry.rs`

### Registry Implementation

```rust
/// Factory for creating providers
pub trait ProviderFactory: Send + Sync {
    fn create(&self, config: Value) -> Result<Box<dyn Provider>>;
}

/// Registry of available providers
pub struct ProviderRegistry {
    factories: HashMap<String, Box<dyn ProviderFactory>>,
    
    /// Default provider name
    default_provider: String,
}

impl ProviderRegistry {
    /// Create a new registry with all built-in providers
    pub fn new() -> Self {
        let mut registry = Self {
            factories: HashMap::new(),
            default_provider: "anthropic".to_string(),
        };
        
        // Register all built-in providers
        registry.register::<AnthropicProvider>("anthropic");
        registry.register::<OpenAiProvider>("openai");
        registry.register::<AzureProvider>("azure_openai");
        registry.register::<OllamaProvider>("ollama");
        registry.register::<DatabricksProvider>("databricks");
        registry.register::<BedrockProvider>("bedrock");
        registry.register::<VertexAIProvider>("gcp_vertex_ai");
        registry.register::<GoogleProvider>("google");
        registry.register::<GitHubCopilotProvider>("github_copilot");
        registry.register::<LiteLLMProvider>("litellm");
        registry.register::<OpenRouterProvider>("openrouter");
        registry.register::<NanoGPTProvider>("nanogpt");
        registry.register::<TetrateProvider>("tetrate");
        registry.register::<VeniceProvider>("venice");
        registry.register::<XAiProvider>("xai");
        registry.register::<SnowflakeProvider>("snowflake");
        registry.register::<CodexProvider>("codex");
        registry.register::<ChatGPTCodexProvider>("chatgpt_codex");
        registry.register::<ClaudeCodeProvider>("claude_code");
        registry.register::<CursorAgentProvider>("cursor_agent");
        
        #[cfg(feature = "local-inference")]
        registry.register::<LocalInferenceProvider>("local_inference");
        
        // ACP providers (Agent Communication Protocol)
        registry.register::<AmpAcpProvider>("amp_acp");
        registry.register::<ClaudeAcpProvider>("claude_acp");
        registry.register::<CodexAcpProvider>("codex_acp");
        registry.register::<CopilotAcpProvider>("copilot_acp");
        registry.register::<PiAcpProvider>("pi_acp");
        
        // Additional providers...
        // (50+ total providers registered)
        
        registry
    }
    
    /// Register a provider factory
    fn register<P: Provider + Default>(&mut self, name: &str) {
        self.factories.insert(
            name.to_string(),
            Box::new(GenericProviderFactory::<P>::new()),
        );
    }
    
    /// Create a provider by name
    pub fn create_provider(
        &self,
        name: &str,
        config: Value,
    ) -> Result<Box<dyn Provider>> {
        let factory = self.factories
            .get(name)
            .ok_or_else(|| anyhow!("Unknown provider: {}", name))?;
        
        factory.create(config)
    }
    
    /// Get list of available providers
    pub fn list_providers(&self) -> Vec<String> {
        self.factories.keys().cloned().collect()
    }
    
    /// Check if provider exists
    pub fn has_provider(&self, name: &str) -> bool {
        self.factories.contains_key(name)
    }
}
```

---

## Model Configuration

### ModelInfo

```rust
/// Information about a model's capabilities
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct ModelInfo {
    /// The name of the model
    pub name: String,
    
    /// The maximum context length this model supports
    pub context_limit: usize,
    
    /// Cost per token for input in USD (optional)
    pub input_token_cost: Option<f64>,
    
    /// Cost per token for output in USD (optional)
    pub output_token_cost: Option<f64>,
    
    /// Currency for the costs (default: "$")
    pub currency: Option<String>,
    
    /// Whether this model supports cache control
    pub supports_cache_control: Option<bool>,
    
    /// Whether this model supports tool calling
    pub supports_tools: Option<bool>,
    
    /// Whether this model supports vision/image input
    pub supports_vision: Option<bool>,
    
    /// Model family (e.g., "gpt-4", "claude")
    pub family: Option<String>,
}

impl ModelInfo {
    /// Create basic model info
    pub fn new(name: impl Into<String>, context_limit: usize) -> Self {
        Self {
            name: name.into(),
            context_limit,
            input_token_cost: None,
            output_token_cost: None,
            currency: None,
            supports_cache_control: None,
            supports_tools: None,
            supports_vision: None,
            family: None,
        }
    }
    
    /// Create with cost info
    pub fn with_cost(
        name: impl Into<String>,
        context_limit: usize,
        input_cost: f64,
        output_cost: f64,
    ) -> Self {
        Self {
            name: name.into(),
            context_limit,
            input_token_cost: Some(input_cost),
            output_token_cost: Some(output_cost),
            currency: Some("$".to_string()),
            supports_cache_control: None,
            supports_tools: None,
            supports_vision: None,
            family: None,
        }
    }
    
    /// Estimate cost for usage
    pub fn estimate_cost(&self, input_tokens: usize, output_tokens: usize) -> Option<f64> {
        let input_cost = self.input_token_cost? * input_tokens as f64;
        let output_cost = self.output_token_cost? * output_tokens as f64;
        Some(input_cost + output_cost)
    }
}
```

### ModelConfig

```rust
/// Runtime configuration for a model
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// Model name/identifier
    pub model_name: String,
    
    /// Temperature (0.0 - 2.0)
    pub temperature: Option<f32>,
    
    /// Maximum tokens to generate
    pub max_tokens: Option<usize>,
    
    /// Top-p sampling
    pub top_p: Option<f32>,
    
    /// Stop sequences
    pub stop_sequences: Vec<String>,
    
    /// Timeout for requests
    pub timeout: Duration,
    
    /// Additional parameters
    pub extra_params: HashMap<String, Value>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            model_name: "default".to_string(),
            temperature: Some(0.7),
            max_tokens: Some(4096),
            top_p: None,
            stop_sequences: vec![],
            timeout: Duration::from_secs(60),
            extra_params: HashMap::new(),
        }
    }
}
```

---

## Provider Implementations

### Anthropic Provider

**File**: `crates/goose/src/providers/anthropic.rs` (10KB)

```rust
/// Anthropic Claude provider
pub struct AnthropicProvider {
    /// API client
    client: ApiClient,
    
    /// Model configuration
    model_config: ModelConfig,
    
    /// API key
    api_key: String,
    
    /// Base URL
    base_url: String,
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> String {
        "anthropic".to_string()
    }
    
    fn get_model_config(&self) -> &ModelConfig {
        &self.model_config
    }
    
    async fn complete(
        &self,
        model_config: &ModelConfig,
        system: Option<String>,
        messages: Vec<Message>,
        tools: Vec<Tool>,
    ) -> Result<ProviderResponse, ProviderError> {
        // Build request body
        let body = json!({
            "model": model_config.model_name,
            "messages": convert_messages(&messages),
            "system": system,
            "tools": if tools.is_empty() { None } else { Some(convert_tools(&tools)) },
            "max_tokens": model_config.max_tokens,
            "temperature": model_config.temperature,
        });
        
        // Make request
        let response = self.client
            .post(&format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(ProviderError::from)?;
        
        // Parse response
        let response_data: AnthropicResponse = response
            .json()
            .await
            .map_err(ProviderError::from)?;
        
        // Convert to ProviderResponse
        let content = convert_anthropic_content(response_data.content);
        let usage = convert_anthropic_usage(response_data.usage);
        
        Ok(ProviderResponse {
            content,
            usage,
            model: response_data.model,
            metadata: None,
        })
    }
    
    async fn stream_complete(
        &self,
        model_config: &ModelConfig,
        system: Option<String>,
        messages: Vec<Message>,
        tools: Vec<Tool>,
    ) -> Result<BoxStream<'_, Result<ProviderStreamChunk, ProviderError>>, ProviderError> {
        let body = json!({
            "model": model_config.model_name,
            "messages": convert_messages(&messages),
            "system": system,
            "tools": if tools.is_empty() { None } else { Some(convert_tools(&tools)) },
            "max_tokens": model_config.max_tokens,
            "temperature": model_config.temperature,
            "stream": true,
        });
        
        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();
        
        let stream = async_stream::try_stream! {
            let response = reqwest::Client::new()
                .post(&format!("{}/v1/messages", base_url))
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&body)
                .send()
                .await
                .map_err(ProviderError::from)?;
            
            let mut stream = response.bytes_stream();
            
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(ProviderError::from)?;
                let text = String::from_utf8_lossy(&chunk);
                
                for line in text.lines() {
                    if line.starts_with("data: ") {
                        let data = &line[6..];
                        if data == "[DONE]" {
                            yield ProviderStreamChunk::Done;
                            break;
                        }
                        
                        let event: AnthropicStreamEvent = serde_json::from_str(data)
                            .map_err(|e| ProviderError::ParseError(e.to_string()))?;
                        
                        match event {
                            AnthropicStreamEvent::ContentBlockDelta { delta, .. } => {
                                match delta {
                                    AnthropicDelta::Text(text) => {
                                        yield ProviderStreamChunk::Text(text);
                                    }
                                    AnthropicDelta::ToolUse { name, input } => {
                                        yield ProviderStreamChunk::ToolRequest(ToolRequest {
                                            name,
                                            arguments: input,
                                        });
                                    }
                                }
                            }
                            AnthropicStreamEvent::MessageStop => {
                                yield ProviderStreamChunk::Done;
                            }
                            _ => {}
                        }
                    }
                }
            }
        };
        
        Ok(Box::pin(stream))
    }
}
```

### OpenAI Provider

**File**: `crates/goose/src/providers/openai.rs` (29KB)

```rust
/// OpenAI GPT provider
pub struct OpenAiProvider {
    client: ApiClient,
    model_config: ModelConfig,
    api_key: String,
    base_url: String,
    organization: Option<String>,
}

#[async_trait]
impl Provider for OpenAiProvider {
    fn name(&self) -> String {
        "openai".to_string()
    }
    
    async fn complete(
        &self,
        model_config: &ModelConfig,
        system: Option<String>,
        messages: Vec<Message>,
        tools: Vec<Tool>,
    ) -> Result<ProviderResponse, ProviderError> {
        // Convert messages to OpenAI format
        let mut openai_messages = Vec::new();
        
        if let Some(system) = system {
            openai_messages.push(json!({
                "role": "system",
                "content": system,
            }));
        }
        
        for msg in messages {
            openai_messages.push(convert_message_to_openai(&msg));
        }
        
        // Build request
        let mut body = json!({
            "model": model_config.model_name,
            "messages": openai_messages,
            "max_tokens": model_config.max_tokens,
            "temperature": model_config.temperature,
            "top_p": model_config.top_p,
        });
        
        // Add tools if provided
        if !tools.is_empty() {
            let openai_tools: Vec<Value> = tools.iter()
                .map(|t| json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }
                }))
                .collect();
            
            body["tools"] = json!(openai_tools);
            body["tool_choice"] = json!("auto");
        }
        
        // Make request
        let response = self.client
            .post(&format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(ProviderError::from)?;
        
        let response_data: OpenAIResponse = response
            .json()
            .await
            .map_err(ProviderError::from)?;
        
        // Convert response
        let choice = response_data.choices
            .into_iter()
            .next()
            .ok_or_else(|| ProviderError::NoResponse)?;
        
        let content = match choice.finish_reason.as_str() {
            "tool_calls" => {
                // Tool call response
                choice.message.tool_calls
                    .into_iter()
                    .map(|tc| MessageContent::ToolRequest(ToolRequest {
                        name: tc.function.name,
                        arguments: serde_json::from_str(&tc.function.arguments)
                            .unwrap_or_default(),
                    }))
                    .collect()
            }
            _ => {
                // Text response
                vec![MessageContent::Text(choice.message.content.unwrap_or_default())]
            }
        };
        
        Ok(ProviderResponse {
            content,
            usage: ProviderUsage {
                input_tokens: response_data.usage.prompt_tokens,
                output_tokens: response_data.usage.completion_tokens,
                total_tokens: response_data.usage.total_tokens,
                estimated_cost: None,
            },
            model: response_data.model,
            metadata: None,
        })
    }
    
    async fn get_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let response = self.client
            .get(&format!("{}/v1/models", self.base_url))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(ProviderError::from)?;
        
        let data: OpenAIModelsResponse = response
            .json()
            .await
            .map_err(ProviderError::from)?;
        
        let models: Vec<ModelInfo> = data.data
            .into_iter()
            .map(|m| ModelInfo {
                name: m.id,
                context_limit: get_context_limit_for_model(&m.id),
                input_token_cost: get_cost_for_model(&m.id, "input"),
                output_token_cost: get_cost_for_model(&m.id, "output"),
                currency: Some("$".to_string()),
                supports_cache_control: None,
                supports_tools: Some(supports_tools(&m.id)),
                supports_vision: Some(supports_vision(&m.id)),
                family: Some(get_family(&m.id)),
            })
            .collect();
        
        Ok(models)
    }
}
```

### Codex Provider

**File**: `crates/goose/src/providers/codex.rs` (48KB)

```rust
/// OpenAI Codex provider
pub struct CodexProvider {
    client: ApiClient,
    model_config: ModelConfig,
    api_key: String,
}

impl CodexProvider {
    /// Create with configuration
    pub fn new(api_key: String, model_config: ModelConfig) -> Self {
        Self {
            client: ApiClient::new(),
            model_config,
            api_key,
        }
    }
}

#[async_trait]
impl Provider for CodexProvider {
    fn name(&self) -> String {
        "codex".to_string()
    }
    
    fn get_model_config(&self) -> &ModelConfig {
        &self.model_config
    }
    
    async fn complete(
        &self,
        model_config: &ModelConfig,
        system: Option<String>,
        messages: Vec<Message>,
        tools: Vec<Tool>,
    ) -> Result<ProviderResponse, ProviderError> {
        // Codex-specific request format
        let body = CodexRequest {
            model: model_config.model_name.clone(),
            messages: convert_to_codex_messages(&messages, system),
            tools: if tools.is_empty() { None } else { Some(tools) },
            max_tokens: model_config.max_tokens,
            temperature: model_config.temperature,
            top_p: model_config.top_p,
        };
        
        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(ProviderError::from)?;
        
        let response_data: CodexResponse = response
            .json()
            .await
            .map_err(ProviderError::from)?;
        
        // Convert to standard format
        Ok(convert_codex_response(response_data))
    }
}
```

---

## Tool Shim

### Purpose

Some providers don't support native tool calling. The tool shim wraps these providers by prompting the model to output tool calls in a specific JSON format.

**File**: `crates/goose/src/providers/toolshim.rs`

```rust
/// Tool shim for providers without native tool support
pub struct ToolShim<P: Provider> {
    inner: P,
}

impl<P: Provider> ToolShim<P> {
    pub fn new(inner: P) -> Self {
        Self { inner }
    }
    
    fn wrap_system_prompt(&self, system: Option<String>, tools: &[Tool]) -> String {
        let mut prompt = system.unwrap_or_default();
        
        if !tools.is_empty() {
            prompt.push_str("\n\nYou have access to the following tools:\n");
            
            for tool in tools {
                prompt.push_str(&format!(
                    "\n## {}\n{}
\nParameters: {}",
                    tool.name,
                    tool.description,
                    serde_json::to_string_pretty(&tool.input_schema).unwrap_or_default()
                ));
            }
            
            prompt.push_str("\n\nWhen you need to use a tool, output a JSON block:\n");
            prompt.push_str(r#"```tool
{
  "tool": "tool_name",
  "arguments": { ... }
}
```"#);
        }
        
        prompt
    }
    
    fn parse_tool_calls(&self, text: &str) -> Vec<ToolRequest> {
        let mut requests = Vec::new();
        
        // Find all ```tool blocks
        let pattern = Regex::new(r"```tool\s*\n(.*?)\n```").unwrap();
        
        for cap in pattern.captures_iter(text) {
            if let Ok(json) = serde_json::from_str::<Value>(&cap[1]) {
                if let (Some(name), Some(args)) = (
                    json.get("tool").and_then(|v| v.as_str()),
                    json.get("arguments"),
                ) {
                    requests.push(ToolRequest {
                        name: name.to_string(),
                        arguments: args.clone(),
                    });
                }
            }
        }
        
        requests
    }
}

#[async_trait]
impl<P: Provider> Provider for ToolShim<P> {
    fn name(&self) -> String {
        format!("{}_shim", self.inner.name())
    }
    
    fn get_model_config(&self) -> &ModelConfig {
        self.inner.get_model_config()
    }
    
    fn supports_tool_shim(&self) -> bool {
        true
    }
    
    async fn complete(
        &self,
        model_config: &ModelConfig,
        system: Option<String>,
        messages: Vec<Message>,
        tools: Vec<Tool>,
    ) -> Result<ProviderResponse, ProviderError> {
        let wrapped_system = self.wrap_system_prompt(system, &tools);
        
        let response = self.inner
            .complete(model_config, Some(wrapped_system), messages, vec![])
            .await?;
        
        // Parse tool calls from text
        let mut content = Vec::new();
        for block in &response.content {
            if let MessageContent::Text(text) = block {
                // Split text and tool calls
                let tool_calls = self.parse_tool_calls(text);
                if tool_calls.is_empty() {
                    content.push(block.clone());
                } else {
                    // Add text before first tool
                    if let Some(idx) = text.find("```tool") {
                        if idx > 0 {
                            content.push(MessageContent::Text(text[..idx].trim().to_string()));
                        }
                    }
                    
                    // Add tool requests
                    for call in tool_calls {
                        content.push(MessageContent::ToolRequest(call));
                    }
                }
            } else {
                content.push(block.clone());
            }
        }
        
        Ok(ProviderResponse {
            content,
            usage: response.usage,
            model: response.model,
            metadata: response.metadata,
        })
    }
}
```

---

## Authentication

### OAuth Implementation

**File**: `crates/goose/src/providers/oauth.rs`

```rust
/// OAuth flow handler
pub struct OAuthHandler {
    client: BasicClient,
    pkce_verifier: Option<PkceCodeVerifier>,
}

impl OAuthHandler {
    /// Create new OAuth handler
    pub fn new(
        client_id: String,
        client_secret: Option<String>,
        auth_url: String,
        token_url: String,
    ) -> Result<Self> {
        let client = BasicClient::new(
            ClientId::new(client_id),
            client_secret.map(ClientSecret::new),
            AuthUrl::new(auth_url)?,
            Some(TokenUrl::new(token_url)?),
        );
        
        Ok(Self {
            client,
            pkce_verifier: None,
        })
    }
    
    /// Generate authorization URL
    pub fn authorize(&mut self) -> Result<(Url, CsrfToken)> {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        self.pkce_verifier = Some(pkce_verifier);
        
        let (auth_url, csrf_token) = self.client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge)
            .url();
        
        Ok((auth_url, csrf_token))
    }
    
    /// Exchange code for token
    pub async fn exchange_code(&self, code: String) -> Result<TokenResponse> {
        let pkce_verifier = self.pkce_verifier
            .as_ref()
            .ok_or_else(|| anyhow!("PKCE verifier not set"))?;
        
        let token = self.client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier.clone())
            .request_async(async_http_client)
            .await?;
        
        Ok(TokenResponse {
            access_token: token.access_token().secret().clone(),
            refresh_token: token.refresh_token().map(|t| t.secret().clone()),
            expires_at: token.expires_in().map(|d| Instant::now() + d),
        })
    }
}
```

---

## Error Handling

### ProviderError

**File**: `crates/goose/src/providers/errors.rs`

```rust
/// Provider-specific errors
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),
    
    /// Rate limited
    #[error("Rate limited. Retry after: {retry_after:?}")]
    RateLimited {
        retry_after: Option<Duration>,
    },
    
    /// Server error (5xx)
    #[error("Server error: {status}")]
    ServerError {
        status: u16,
        message: String,
    },
    
    /// Authentication error
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),
    
    /// Invalid request
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    
    /// Context length exceeded
    #[error("Context length exceeded: {used}/{limit}")]
    ContextLengthExceeded {
        used: usize,
        limit: usize,
    },
    
    /// Model not found
    #[error("Model not found: {0}")]
    ModelNotFound(String),
    
    /// Timeout
    #[error("Request timeout")]
    Timeout,
    
    /// Parse error
    #[error("Failed to parse response: {0}")]
    ParseError(String),
    
    /// No response
    #[error("No response from provider")]
    NoResponse,
    
    /// Other error
    #[error("Provider error: {0}")]
    Other(String),
}

impl From<reqwest::Error> for ProviderError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            ProviderError::Timeout
        } else if err.is_connect() {
            ProviderError::NetworkError(err.to_string())
        } else {
            ProviderError::Other(err.to_string())
        }
    }
}

impl ProviderError {
    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ProviderError::RateLimited { .. }
                | ProviderError::ServerError { .. }
                | ProviderError::NetworkError(_)
                | ProviderError::Timeout
        )
    }
}
```

---

## Retry System

**File**: `crates/goose/src/providers/retry.rs`

```rust
/// Retry configuration for providers
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    
    /// Initial backoff duration
    pub initial_backoff: Duration,
    
    /// Maximum backoff duration
    pub max_backoff: Duration,
    
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    
    /// Jitter factor (0.0 - 1.0)
    pub jitter: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(1000),
            max_backoff: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: 0.1,
        }
    }
}

/// Retry a provider operation with exponential backoff
pub async fn retry_operation<T, F, Fut>(
    config: &RetryConfig,
    operation: F,
) -> Result<T, ProviderError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, ProviderError>>,
{
    let mut attempts = 0;
    let mut backoff = config.initial_backoff;
    let mut operation = operation;
    
    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                attempts += 1;
                
                // Check if we should retry
                if attempts > config.max_retries || !err.is_retryable() {
                    return Err(err);
                }
                
                // Calculate jitter
                let jitter_ms = if config.jitter > 0.0 {
                    let max_jitter = (backoff.as_millis() as f64 * config.jitter) as u64;
                    rand::random::<u64>() % max_jitter
                } else {
                    0
                };
                
                let sleep_duration = backoff + Duration::from_millis(jitter_ms);
                
                tracing::warn!(
                    "Provider operation failed (attempt {}/{}), retrying in {:?}: {}",
                    attempts,
                    config.max_retries,
                    sleep_duration,
                    err
                );
                
                tokio::time::sleep(sleep_duration).await;
                
                // Increase backoff
                backoff = Duration::from_millis(
                    (backoff.as_millis() as f64 * config.backoff_multiplier) as u64,
                );
                backoff = backoff.min(config.max_backoff);
            }
        }
    }
}
```

---

## Summary

The Goose Provider system provides:

1. **Unified Trait**: Single `Provider` trait for all LLM backends
2. **50+ Providers**: Anthropic, OpenAI, Azure, Ollama, Databricks, and more
3. **Streaming Support**: Native streaming for real-time responses
4. **Tool Shim**: Compatibility layer for non-tool providers
5. **Retry Logic**: Automatic retry with exponential backoff
6. **Error Handling**: Rich error types with retry classification
7. **Authentication**: OAuth and API key support
8. **Model Management**: Model info, costs, and capabilities

Key files:
- `crates/goose/src/providers/base.rs` - Provider trait (38KB)
- `crates/goose/src/providers/provider_registry.rs` - Registry
- `crates/goose/src/providers/anthropic.rs` - Anthropic provider
- `crates/goose/src/providers/openai.rs` - OpenAI provider
- `crates/goose/src/providers/codex.rs` - Codex provider (48KB)
- `crates/goose/src/providers/claude_code.rs` - Claude Code provider (57KB)
- `crates/goose/src/providers/errors.rs` - Error handling
- `crates/goose/src/providers/retry.rs` - Retry logic
- `crates/goose/src/providers/toolshim.rs` - Tool compatibility
