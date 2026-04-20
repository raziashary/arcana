# Model Context Protocol in Rust — Complete Guide

> **Based on the official [`rmcp`](https://crates.io/crates/rmcp) SDK · MCP Spec 2025-11-25**

---

## Table of Contents

1. [What is MCP?](#1-what-is-mcp)
2. [The `rmcp` Crate](#2-the-rmcp-crate)
3. [Project Setup](#3-project-setup)
4. [Core Concepts](#4-core-concepts)
5. [Transports](#5-transports)
6. [Building a Server](#6-building-a-server)
7. [Building a Client](#7-building-a-client)
8. [Tools](#8-tools)
9. [Resources](#9-resources)
10. [Prompts](#10-prompts)
11. [Sampling](#11-sampling)
12. [Roots](#12-roots)
13. [Logging](#13-logging)
14. [Completions](#14-completions)
15. [Notifications & Progress](#15-notifications--progress)
16. [Subscriptions](#16-subscriptions)
17. [Streamable HTTP Transport](#17-streamable-http-transport)
18. [Claude Desktop Integration](#18-claude-desktop-integration)
19. [Related Crates & Ecosystem](#19-related-crates--ecosystem)

---

## 1. What is MCP?

The **Model Context Protocol (MCP)** is an open standard that lets AI assistants (clients) discover and call capabilities exposed by external processes (servers). Those capabilities are organized into three primitives:

| Primitive   | Direction         | Purpose                                               |
|-------------|-------------------|-------------------------------------------------------|
| **Tools**   | Client → Server   | Callable functions (like REST endpoints for AI)       |
| **Resources** | Client → Server | Readable data — files, database records, API payloads |
| **Prompts** | Client → Server   | Parameterized message templates for LLM interactions  |

Additionally, **Sampling** lets servers ask the *client* to run an LLM completion — flipping the usual direction — and **Roots** let clients tell servers which workspace directories are in scope.

The full specification lives at [modelcontextprotocol.io](https://modelcontextprotocol.io/specification/2025-11-25).

---

## 2. The `rmcp` Crate

`rmcp` is the **official Rust SDK** for MCP, built on Tokio's async runtime. The workspace ships two crates:

| Crate | Purpose |
|-------|---------|
| `rmcp` | Core protocol implementation — types, handlers, transports, macros |
| `rmcp-macros` | Procedural macros (`#[tool]`, `#[tool_router]`, `#[prompt]`, …) |

- **Crates.io:** <https://crates.io/crates/rmcp>
- **Docs:** <https://docs.rs/rmcp>
- **Source:** <https://github.com/modelcontextprotocol/rust-sdk>

---

## 3. Project Setup

### `Cargo.toml`

```toml
[package]
name    = "my-mcp-server"
version = "0.1.0"
edition = "2021"

[dependencies]
# Server-only build
rmcp = { version = "0.16", features = ["server"] }

# Client-only build
# rmcp = { version = "0.16", features = ["client"] }

# Both server and client with all transports
# rmcp = { version = "0.16", features = [
#     "server",
#     "client",
#     "transport-io",            # stdio
#     "transport-sse-server",    # SSE (legacy)
#     "transport-streamable-http-server", # Streamable HTTP (current)
# ]}

tokio      = { version = "1", features = ["full"] }
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow     = "1"
tracing-subscriber = "0.3"
```

### Feature Flags Summary

| Feature flag | What it enables |
|---|---|
| `server` | `ServerHandler` trait, server-side macros |
| `client` | `ClientHandler` trait, client helpers |
| `transport-io` | stdio transport (default for local tools) |
| `transport-sse-server` | Server-Sent Events transport (legacy HTTP) |
| `transport-streamable-http-server` | Streamable HTTP transport (current standard) |
| `macros` | Re-exports all proc-macro helpers |

---

## 4. Core Concepts

### The `serve()` Pattern

Everything in `rmcp` revolves around one method:

```rust
let handle = service.serve(transport).await?;
```

- `service` is anything implementing `ServerHandler` (or `ClientHandler`)
- `transport` is anything implementing `IntoTransport` (e.g. stdio pair, SSE session, child process)
- `.await?` completes the MCP initialization handshake and returns a live handle

### Lifecycle

```
Client                          Server
  |------- initialize  -------->|
  |<------ initialize_result ---|
  |------- initialized  ------->|   (notification, no reply)
  |         ... requests ...    |
  |------- shutdown  ---------->|
```

### Handler Traits

```rust
// Implement for a server process
pub trait ServerHandler: Clone + Send + Sync + 'static { ... }

// Implement for a client process
pub trait ClientHandler: Clone + Send + Sync + 'static { ... }
```

Both traits provide blanket default implementations for every method, so you only override what you need.

---

## 5. Transports

### stdio (most common — local tools, Claude Desktop)

```rust
use rmcp::transport::stdio;
use tokio::io::{stdin, stdout};

// Shorthand helper
let transport = stdio();

// Or explicitly
let transport = (stdin(), stdout());
```

### Child Process (client spawning a server)

```rust
use rmcp::transport::{TokioChildProcess, ConfigureCommandExt};
use tokio::process::Command;

let transport = TokioChildProcess::new(
    Command::new("npx").configure(|cmd| {
        cmd.arg("-y").arg("@modelcontextprotocol/server-everything");
    })
)?;
```

### Streamable HTTP (current network standard — 2025-03-26+)

Requires features `transport-streamable-http-server` and an HTTP framework like `axum`:

```toml
axum = "0.8"
rmcp  = { version = "0.16", features = ["server", "transport-streamable-http-server"] }
```

```rust
use rmcp::transport::streamable_http_server::{
    StreamableHttpService,
    session::local::LocalSessionManager,
};
use std::sync::Arc;

let service = StreamableHttpService::new(
    || Ok(MyServer::new()),
    Arc::new(LocalSessionManager::default()),
);

let router = axum::Router::new()
    .route_service("/mcp", service);

let listener = tokio::net::TcpListener::bind("127.0.0.1:7000").await?;
axum::serve(listener, router).await?;
```

### SSE (legacy — pre 2025-03-26)

```rust
use rmcp::transport::SseServer;

let server = SseServer::serve("127.0.0.1:8080".parse()?).await?;
let _ct = server.with_service(|| MyServer::new());
_ct.cancelled().await;
```

---

## 6. Building a Server

### Minimal stdio server

```rust
use rmcp::{ServerHandler, ServiceExt, model::ServerInfo, transport::stdio};

#[derive(Clone)]
struct HelloServer;

impl ServerHandler for HelloServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: rmcp::model::Implementation {
                name: "hello-server".into(),
                version: "1.0.0".into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let service = HelloServer.serve(stdio()).await?;
    service.waiting().await?;   // blocks until client disconnects
    Ok(())
}
```

### Lifecycle hooks

```rust
impl ServerHandler for MyServer {
    async fn on_initialized(&self, _ctx: NotificationContext<RoleServer>) {
        // Handshake is complete — safe to start background tasks
        println!("Client connected and ready");
    }
}
```

### Shutdown

```rust
let handle = MyServer.serve(stdio()).await?;

// Block until the client closes the connection
let reason = handle.waiting().await?;

// Or forcibly cancel from another task
let reason = handle.cancel().await?;
```

---

## 7. Building a Client

```rust
use rmcp::{ServiceExt, transport::{TokioChildProcess, ConfigureCommandExt}};
use tokio::process::Command;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Spawn a server subprocess and connect
    let client = ().serve(
        TokioChildProcess::new(
            Command::new("my-mcp-server").configure(|_| {})
        )?
    ).await?;

    // List available tools
    let tools = client.list_all_tools().await?;
    for tool in &tools {
        println!("Tool: {} — {}", tool.name, tool.description.as_deref().unwrap_or(""));
    }

    // Call a tool
    let result = client.call_tool(
        rmcp::model::CallToolRequestParams::new("add")
    ).await?;

    // Graceful shutdown
    client.cancel().await?;
    Ok(())
}
```

### Custom `ClientHandler`

```rust
use rmcp::{ClientHandler, model::*, service::{NotificationContext, RoleClient}};

#[derive(Clone, Default)]
struct MyClient;

impl ClientHandler for MyClient {
    async fn on_resource_list_changed(&self, _ctx: NotificationContext<RoleClient>) {
        println!("Server resource list changed — re-fetching…");
    }

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _ctx: NotificationContext<RoleClient>,
    ) {
        eprintln!("[{}] {}", params.level, params.data);
    }
}
```

---

## 8. Tools

Tools are functions that the client can discover via `list_tools` and invoke via `call_tool`. The `#[tool]` macro generates JSON Schema from your parameter types automatically.

### Simple server — tools only

For a server that only exposes tools, `#[tool_router(server_handler)]` wires everything in one step:

```rust
use rmcp::{tool, tool_router, ServiceExt, transport::stdio};

#[derive(Clone)]
struct Calculator;

#[tool_router(server_handler)]
impl Calculator {
    /// Add two integers
    #[tool(description = "Add two numbers together")]
    fn add(&self, #[tool(param)] a: i32, #[tool(param)] b: i32) -> String {
        (a + b).to_string()
    }

    /// Multiply two integers
    #[tool(description = "Multiply two numbers")]
    fn multiply(&self, #[tool(param)] x: i32, #[tool(param)] y: i32) -> String {
        (x * y).to_string()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let service = Calculator.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
```

### Aggregate parameters (struct input)

```rust
use rmcp::{handler::server::tool::Parameters, schemars, tool, tool_router};
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DivideRequest {
    #[schemars(description = "Dividend")]
    pub numerator: f64,
    #[schemars(description = "Divisor")]
    pub denominator: f64,
}

#[tool_router]
impl Calculator {
    #[tool(description = "Divide two numbers, returns error on division by zero")]
    async fn divide(
        &self,
        Parameters(req): Parameters<DivideRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if req.denominator == 0.0 {
            return Err(ErrorData::invalid_params("Division by zero", None));
        }
        Ok(CallToolResult::text_content(
            vec![(req.numerator / req.denominator).to_string().into()]
        ))
    }
}
```

### Server with tools + custom metadata

When you need both tools and fine-grained `ServerInfo`, split the macros:

```rust
use rmcp::{tool, tool_router, tool_handler, ServerHandler, ServiceExt};

#[derive(Clone)]
struct Calculator;

#[tool_router]
impl Calculator {
    #[tool(description = "Add two numbers")]
    fn add(&self, #[tool(param)] a: i32, #[tool(param)] b: i32) -> String {
        (a + b).to_string()
    }
}

#[tool_handler(
    name        = "calculator",
    version     = "1.0.0",
    instructions = "A simple calculator server"
)]
impl ServerHandler for Calculator {}
```

### Client-side tool calls

```rust
use rmcp::model::CallToolRequestParams;

// List
let tools = client.list_all_tools().await?;

// Call by name (args are a JSON object)
let result = client.call_tool(
    CallToolRequestParams::new("add")
        .with_arg("a", 3)
        .with_arg("b", 4)
).await?;

// Extract text from the result
for content in result.content {
    if let Some(text) = content.as_text() {
        println!("Result: {}", text.text);
    }
}
```

---

## 9. Resources

Resources expose data that the client can read — files, database rows, API responses, in-memory state.

### Server implementation

```rust
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::*, service::RequestContext, transport::stdio,
};
use serde_json::json;

#[derive(Clone)]
struct FileServer;

impl ServerHandler for FileServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_resources()
                .build(),
            ..Default::default()
        }
    }

    async fn list_resources(
        &self,
        _req: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![
                RawResource::new("file:///config.json", "App Config").no_annotation(),
                RawResource::new("memo://insights",     "Analysis Memo").no_annotation(),
            ],
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        req: ReadResourceRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let contents = match req.uri.as_str() {
            "file:///config.json" => {
                ResourceContents::text(r#"{"version": "1.0", "debug": false}"#, &req.uri)
            }
            "memo://insights" => {
                ResourceContents::text("Analysis results: revenue up 12% QoQ.", &req.uri)
            }
            _ => return Err(McpError::resource_not_found(
                "resource_not_found",
                Some(json!({ "uri": req.uri })),
            )),
        };
        Ok(ReadResourceResult { contents: vec![contents] })
    }
}
```

### Client-side resource access

```rust
// List all (handles pagination automatically)
let resources = client.list_all_resources().await?;

// Read
let result = client.read_resource(ReadResourceRequestParams {
    uri: "file:///config.json".into(),
    meta: None,
}).await?;

for content in result.contents {
    println!("{}", content.as_text().map(|t| t.text.as_str()).unwrap_or("(binary)"));
}
```

### Resource update notifications (server → client)

```rust
// Notify that the list of available resources changed
context.peer.notify_resource_list_changed().await?;

// Notify that a specific resource's content changed
context.peer.notify_resource_updated(ResourceUpdatedNotificationParam {
    uri: "file:///config.json".into(),
}).await?;
```

---

## 10. Prompts

Prompts are reusable, parameterized message templates that clients can request and inject into conversations.

### Server implementation

```rust
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::prompt::PromptRouter, wrapper::Parameters},
    model::*, prompt, prompt_handler, prompt_router,
    schemars::JsonSchema, service::RequestContext, transport::stdio,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CodeReviewArgs {
    #[schemars(description = "Programming language of the code")]
    pub language: String,
    #[schemars(description = "Focus areas for the review (optional)")]
    pub focus_areas: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct PromptServer {
    prompt_router: PromptRouter<Self>,
}

#[prompt_router]
impl PromptServer {
    fn new() -> Self {
        Self { prompt_router: Self::prompt_router() }
    }

    /// Simple prompt with no parameters
    #[prompt(name = "greeting", description = "A simple greeting message")]
    async fn greeting(&self) -> Vec<PromptMessage> {
        vec![PromptMessage::new_text(
            PromptMessageRole::User,
            "Hello! How can you help me today?",
        )]
    }

    /// Prompt with typed, schema-validated arguments
    #[prompt(name = "code_review", description = "Request a code review")]
    async fn code_review(
        &self,
        Parameters(args): Parameters<CodeReviewArgs>,
    ) -> Result<GetPromptResult, McpError> {
        let focus = args.focus_areas
            .unwrap_or_else(|| vec!["correctness".into(), "safety".into()]);

        Ok(GetPromptResult {
            description: Some(format!("{} code review", args.language)),
            messages: vec![PromptMessage::new_text(
                PromptMessageRole::User,
                format!(
                    "Please review my {} code. Focus on: {}.",
                    args.language,
                    focus.join(", ")
                ),
            )],
        })
    }
}

#[prompt_handler]
impl ServerHandler for PromptServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_prompts()
                .build(),
            ..Default::default()
        }
    }
}
```

#### Supported prompt return types

| Return type | When to use |
|---|---|
| `Vec<PromptMessage>` | Simple message list |
| `GetPromptResult` | Messages + optional description |
| `Result<T, McpError>` | Either of the above with error handling |

### Client-side prompt access

```rust
use rmcp::model::GetPromptRequestParams;

// Discover prompts
let prompts = client.list_all_prompts().await?;

// Fetch a prompt with arguments
let result = client.get_prompt(GetPromptRequestParams {
    name: "code_review".into(),
    arguments: Some(rmcp::object!({
        "language": "Rust",
        "focus_areas": ["performance", "memory safety"]
    })),
    meta: None,
}).await?;
```

---

## 11. Sampling

Sampling inverts control: the *server* asks the *client* to run an LLM completion. This is how MCP servers can leverage the AI that's already connected to them.

### Server — requesting a completion

```rust
use rmcp::model::*;

// Inside a tool handler, call the client's LLM
let response = context.peer.create_message(CreateMessageRequestParams {
    messages: vec![SamplingMessage::user_text(
        "Explain this error: connection refused on port 5432"
    )],
    model_preferences: Some(ModelPreferences {
        hints: Some(vec![ModelHint { name: Some("claude".into()) }]),
        cost_priority: Some(0.3),
        speed_priority: Some(0.8),
        intelligence_priority: Some(0.7),
    }),
    system_prompt: Some("You are a helpful debugging assistant.".into()),
    include_context: Some(ContextInclusion::None),
    temperature: Some(0.5),
    max_tokens: 256,
    stop_sequences: None,
    metadata: None,
    tools: None,
    tool_choice: None,
    task: None,
    meta: None,
}).await?;

let text = response.message.content
    .first()
    .and_then(|c| c.as_text())
    .map(|t| t.text.as_str())
    .unwrap_or("(no text)");
```

### Client — handling sampling requests

```rust
use rmcp::{ClientHandler, model::*, service::RequestContext};

#[derive(Clone, Default)]
struct MyClient;

impl ClientHandler for MyClient {
    async fn create_message(
        &self,
        params: CreateMessageRequestParams,
        _ctx: RequestContext<RoleClient>,
    ) -> Result<CreateMessageResult, ErrorData> {
        // Forward to your actual LLM here
        let reply = call_my_llm(&params.messages).await;

        Ok(CreateMessageResult {
            message: SamplingMessage::assistant_text(reply),
            model: "my-model-v1".into(),
            stop_reason: Some(CreateMessageResult::STOP_REASON_END_TURN.into()),
        })
    }
}
```

---

## 12. Roots

Roots tell a server which directories the client considers "in scope" — equivalent to workspace folders in an IDE.

### Server — querying roots

```rust
impl ServerHandler for MyServer {
    async fn call_tool(
        &self,
        req: CallToolRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        // Ask the client which directories to work in
        let roots = ctx.peer.list_roots().await?;
        for root in &roots.roots {
            println!("Workspace: {} — {}", root.uri, root.name.as_deref().unwrap_or(""));
        }
        // ...
    }

    async fn on_roots_list_changed(&self, _ctx: NotificationContext<RoleServer>) {
        // Client's workspace changed — re-fetch roots if cached
    }
}
```

### Client — declaring roots

```rust
impl ClientHandler for MyClient {
    async fn list_roots(
        &self,
        _ctx: RequestContext<RoleClient>,
    ) -> Result<ListRootsResult, ErrorData> {
        Ok(ListRootsResult {
            roots: vec![Root {
                uri:  "file:///home/user/my-project".into(),
                name: Some("My Project".into()),
            }],
        })
    }
}

// Notify when roots change
client.notify_roots_list_changed().await?;
```

---

## 13. Logging

Servers can stream structured log messages to the client. The client controls the minimum severity level.

### Server — sending log messages

```rust
impl ServerHandler for MyServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_logging()
                .build(),
            ..Default::default()
        }
    }

    async fn set_level(
        &self,
        req: SetLevelRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        // Store req.level and filter future messages accordingly
        Ok(())
    }
}

// From any handler that has access to `context.peer`
context.peer.notify_logging_message(LoggingMessageNotificationParam {
    level: LoggingLevel::Info,
    logger: Some("my-server".into()),
    data: serde_json::json!({
        "message": "Processing completed",
        "records": 1024,
    }),
}).await?;
```

**Log levels** (least → most severe):
`Debug` · `Info` · `Notice` · `Warning` · `Error` · `Critical` · `Alert` · `Emergency`

### Client — receiving log messages

```rust
impl ClientHandler for MyClient {
    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _ctx: NotificationContext<RoleClient>,
    ) {
        eprintln!("[{}] {}: {}", params.level,
            params.logger.unwrap_or_default(), params.data);
    }
}

// Set server log level from client
client.set_level(SetLevelRequestParams {
    level: LoggingLevel::Warning,
    meta: None,
}).await?;
```

---

## 14. Completions

Completions provide argument auto-complete suggestions as users fill in prompt or resource template parameters.

### Server implementation

```rust
use rmcp::{ErrorData as McpError, ServerHandler, model::*, service::RequestContext, RoleServer};

impl ServerHandler for MyServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_completions()
                .enable_prompts()
                .build(),
            ..Default::default()
        }
    }

    async fn complete(
        &self,
        req: CompleteRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<CompleteResult, McpError> {
        let all_values: Vec<&str> = match &req.r#ref {
            Reference::Prompt(p) if p.name == "sql_query" => {
                match req.argument.name.as_str() {
                    "operation" => vec!["SELECT", "INSERT", "UPDATE", "DELETE"],
                    "table"     => vec!["users", "orders", "products", "inventory"],
                    _           => vec![],
                }
            }
            _ => vec![],
        };

        // Filter by the partial value the user has typed so far
        let filtered: Vec<String> = all_values
            .into_iter()
            .filter(|v| v.to_lowercase().contains(&req.argument.value.to_lowercase()))
            .map(String::from)
            .collect();

        Ok(CompleteResult {
            completion: CompletionInfo {
                values: filtered,
                total: None,
                has_more: Some(false),
            },
        })
    }
}
```

### Client — requesting completions

```rust
let result = client.complete(CompleteRequestParams {
    r#ref: Reference::Prompt(PromptReference { name: "sql_query".into() }),
    argument: ArgumentInfo {
        name:  "operation".into(),
        value: "SEL".into(),       // partial input
    },
    context: None,
    meta: None,
}).await?;

// result.completion.values → ["SELECT"]
println!("{:?}", result.completion.values);
```

---

## 15. Notifications & Progress

Notifications are fire-and-forget messages — no reply is expected or required.

### Progress reporting

For long-running tool calls, stream progress updates back to the client:

```rust
use rmcp::model::*;

// Inside a tool handler
let total = 100u64;
for i in 0..total {
    do_work_item(i).await;

    context.peer.notify_progress(ProgressNotificationParam {
        progress_token: ProgressToken(NumberOrString::Number(i as i64)),
        progress: i as f64,
        total: Some(total as f64),
        message: Some(format!("Processing item {}/{}", i + 1, total)),
    }).await?;
}
```

### Cancellation

Either side can cancel an in-progress request:

```rust
// Send cancellation
context.peer.notify_cancelled(CancelledNotificationParam {
    request_id: the_request_id,
    reason: Some("User requested stop".into()),
}).await?;

// Handle incoming cancellation on the server
impl ServerHandler for MyServer {
    async fn on_cancelled(
        &self,
        params: CancelledNotificationParam,
        _ctx: NotificationContext<RoleServer>,
    ) {
        eprintln!("Request {:?} cancelled: {:?}", params.request_id, params.reason);
        // Signal your running task to abort
    }
}
```

### List-changed notifications

Inform the client that available capabilities have changed:

```rust
context.peer.notify_tool_list_changed().await?;
context.peer.notify_prompt_list_changed().await?;
context.peer.notify_resource_list_changed().await?;
```

---

## 16. Subscriptions

Clients can subscribe to individual resources. When a subscribed resource changes, the server notifies the client, which can then re-read it.

### Server — subscribe / unsubscribe handlers

```rust
use std::{collections::HashSet, sync::Arc};
use tokio::sync::Mutex;

#[derive(Clone)]
struct MyServer {
    subscriptions: Arc<Mutex<HashSet<String>>>,
}

impl MyServer {
    fn new() -> Self {
        Self { subscriptions: Arc::new(Mutex::new(HashSet::new())) }
    }
}

impl ServerHandler for MyServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_resources()
                .enable_resources_subscribe()
                .build(),
            ..Default::default()
        }
    }

    async fn subscribe(
        &self,
        req: SubscribeRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        self.subscriptions.lock().await.insert(req.uri);
        Ok(())
    }

    async fn unsubscribe(
        &self,
        req: UnsubscribeRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        self.subscriptions.lock().await.remove(&req.uri);
        Ok(())
    }
}

// When data changes, notify all subscribers
context.peer.notify_resource_updated(ResourceUpdatedNotificationParam {
    uri: "file:///config.json".into(),
}).await?;
```

### Client — subscribe and handle updates

```rust
// Subscribe
client.subscribe(SubscribeRequestParams {
    uri: "file:///config.json".into(),
    meta: None,
}).await?;

// Handle update notifications
impl ClientHandler for MyClient {
    async fn on_resource_updated(
        &self,
        params: ResourceUpdatedNotificationParam,
        _ctx: NotificationContext<RoleClient>,
    ) {
        // Re-read the resource
        println!("Resource updated: {}", params.uri);
    }
}

// Unsubscribe when done
client.unsubscribe(UnsubscribeRequestParams {
    uri: "file:///config.json".into(),
    meta: None,
}).await?;
```

---

## 17. Streamable HTTP Transport

The **Streamable HTTP** transport (protocol version 2025-03-26+) replaces the older HTTP+SSE transport. It exposes a single endpoint that accepts both `POST` (client → server) and `GET` (open SSE stream for server → client) requests.

### Cargo.toml

```toml
[dependencies]
rmcp  = { version = "0.16", features = [
    "server",
    "transport-streamable-http-server",
]}
axum  = "0.8"
tokio = { version = "1", features = ["full"] }
```

### Full Streamable HTTP server example

```rust
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::*, schemars, tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpService,
        session::local::LocalSessionManager,
    },
    service::RequestContext,
};
use std::sync::Arc;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EchoRequest {
    #[schemars(description = "The message to echo back")]
    pub message: String,
}

#[derive(Clone)]
struct EchoServer {
    tool_router: ToolRouter<EchoServer>,
}

#[tool_router]
impl EchoServer {
    fn new() -> Self {
        Self { tool_router: Self::tool_router() }
    }

    #[tool(description = "Echo a message back to the caller")]
    async fn echo(
        &self,
        Parameters(req): Parameters<EchoRequest>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::text_content(vec![req.message.into()]))
    }
}

#[tool_handler(name = "echo-server", version = "0.1.0")]
impl ServerHandler for EchoServer {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let service = StreamableHttpService::new(
        || Ok(EchoServer::new()),
        Arc::new(LocalSessionManager::default()),
    );

    let router = axum::Router::new()
        .route_service("/mcp", service);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:7000").await?;
    println!("MCP server listening on http://127.0.0.1:7000/mcp");
    axum::serve(listener, router).await?;
    Ok(())
}
```

**Transport security notes:**
- Always validate the `Origin` header to prevent DNS rebinding attacks.
- In production, bind to `127.0.0.1` (localhost) rather than `0.0.0.0`.
- Implement authentication (e.g. Bearer token or OAuth 2.1) for public-facing servers.

---

## 18. Claude Desktop Integration

Add your server to Claude Desktop's configuration file.

**macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`
**Windows:** `%APPDATA%\Claude\claude_desktop_config.json`

### stdio server (cargo run — development)

```json
{
  "mcpServers": {
    "my-server-dev": {
      "command": "cargo",
      "args": ["run", "--manifest-path", "/path/to/my-server/Cargo.toml"]
    }
  }
}
```

### stdio server (installed binary — production)

```bash
# Install to ~/.cargo/bin
cargo install --path .
```

```json
{
  "mcpServers": {
    "my-server": {
      "command": "my-mcp-server"
    }
  }
}
```

### Remote Streamable HTTP server

```json
{
  "mcpServers": {
    "my-remote-server": {
      "url": "http://127.0.0.1:7000/mcp"
    }
  }
}
```

Claude will automatically discover tools, invoke them as needed, and display results inline in the conversation.

---

## 19. Related Crates & Ecosystem

### Extending `rmcp`

| Crate | Purpose |
|---|---|
| [`rmcp-macros`](https://crates.io/crates/rmcp-macros) | Procedural macros bundled with `rmcp` |
| [`rmcp-actix-web`](https://crates.io/crates/rmcp-actix-web) | actix-web transport backends (SSE + Streamable HTTP) |
| [`rmcp-openapi`](https://gitlab.com/lx-industries/rmcp-openapi) | Convert OpenAPI endpoints into MCP tools |
| [`rust-mcp-sdk`](https://crates.io/crates/rust-mcp-sdk) | Alternative community SDK, MCP spec 2025-11-25 |

### Tools built with `rmcp`

| Project | Description |
|---|---|
| [goose](https://github.com/block/goose) | Open-source extensible AI agent |
| [terminator](https://github.com/mediar-ai/terminator) | Cross-platform desktop automation MCP server |
| [hyper-mcp](https://github.com/hyper-mcp-rs/hyper-mcp) | MCP server extended by WASM plugins |
| [spreadsheet-mcp](https://github.com/PSU3D0/spreadsheet-mcp) | Token-efficient spreadsheet analysis for LLM agents |
| [rmcp-proxy](https://crates.io/crates/rmcp-proxy) | Bidirectional proxy — stdio ↔ SSE/Streamable HTTP |

### Official references

- [MCP Specification 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25)
- [MCP Transports spec (2025-03-26)](https://modelcontextprotocol.io/specification/2025-03-26/basic/transports)
- [JSON Schema (2020-12)](https://json-schema.org/specification)
- [JSON-RPC 2.0](https://www.jsonrpc.org/specification)

---

*Last updated: April 2026 · rmcp v0.16 · MCP spec 2025-11-25*