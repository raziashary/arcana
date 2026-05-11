// tool_adapter.rs — Bridges the existing `Tool` trait to rmcp's `ServerHandler`.
//
// Every tool registered in `agent::tool_registry::ToolRegistry` is dynamically
// exposed via MCP `tools/list`. Calls are dispatched back to the canonical
// Rust implementation via `Tool::execute()`.

use std::sync::Arc;

use tracing::warn;
use rmcp::{
    ErrorData as McpError,
    RoleServer,
    ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, Content, ListToolsResult, ServerInfo,
        Implementation, ServerCapabilities, Tool as McpTool,
    },
    service::RequestContext,
};

use crate::agent::{ToolContext, ToolRegistry, ToolOutput as ToolResult};

/// MCP server that wraps the canonical AutoDev `ToolRegistry`.
/// Implements `ServerHandler` so rmcp can serve it over stdio or HTTP.
#[derive(Clone)]
pub struct AutodevMcpServer {
    registry: Arc<ToolRegistry>,
    working_dir: String,
}

impl AutodevMcpServer {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        // Default to process cwd, NOT $HOME/.autodev (the AutoDev config dir).
        // Fleet workers MUST pass __working_dir on every tool call to target the
        // correct project root. This default is only a last-resort fallback.
        let working_dir = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| {
                warn!("[autodev-mcp-server] Cannot determine cwd for default working_dir. \
                       Fleet workers MUST pass __working_dir on every tool call.");
                ".".to_string()
            });
        Self { registry, working_dir }
    }

    /// Wrap a `ToolResult` (from our trait) into an rmcp `CallToolResult`.
    fn wrap_result(tool_result: ToolResult) -> CallToolResult {
        let content = vec![Content::text(tool_result.output)];
        if tool_result.is_error {
            CallToolResult::error(content)
        } else {
            CallToolResult::success(content)
        }
    }
}

impl ServerHandler for AutodevMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "autodev-mcp-server".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                ..Default::default()
            },
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }

    /// MCP `tools/list` — discover every tool the registry knows about.
    async fn list_tools(
        &self,
        _req: Option<rmcp::model::PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let tools: Vec<McpTool> = self
            .registry
            .all()
            .into_iter()
            .map(|t| {
                let input_schema = match t.input_schema() {
                    serde_json::Value::Object(map) => std::sync::Arc::new(map),
                    other => {
                        let mut map = serde_json::Map::new();
                        map.insert("__schema".to_string(), other);
                        std::sync::Arc::new(map)
                    }
                };
                McpTool {
                    name: t.name().to_string().into(),
                    title: None,
                    description: Some(t.description().to_string().into()),
                    input_schema,
                    output_schema: None,
                    annotations: None,
                    execution: None,
                    icons: None,
                    meta: None,
                }
            })
            .collect();

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    /// MCP `tools/call` — execute a single tool by name.
    async fn call_tool(
        &self,
        req: CallToolRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let tool_name = req.name;
        let mut args = req.arguments.unwrap_or_default();

        // L.1 — Accept __working_dir per-call so fleet workers target the right
        // project directory instead of the process-cwd default.
        let working_dir = args.remove("__working_dir")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| self.working_dir.clone());

        // L.3 — Accept __session_id so all tool calls from one worker task share
        // a session, grouping checkpoints for possible rollback.
        let session_id = args.remove("__session_id")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // L.5 — Accept __agent_role so fleet workers can identify themselves.
        // Required for request_supervisor_action to record the correct worker_slug
        // instead of falling back to "unknown" for all MCP-originated calls.
        let agent_role = args.remove("__agent_role")
            .and_then(|v| v.as_str().map(String::from));

        let args = serde_json::Value::Object(args);

        let Some(tool) = self.registry.get(&tool_name) else {
            return Err(McpError::invalid_params(
                format!("Tool '{}' not found in autodev-mcp-server", tool_name),
                None,
            ));
        };

        // Build a synthetic ToolContext using the per-call overrides.
        let ctx = ToolContext {
            session_id,
            working_dir,
            call_id: uuid::Uuid::new_v4().to_string(),
            depth:   0,
            agent_role,
        };

        let result = tool.execute(args, &ctx).await;
        Ok(Self::wrap_result(result))
    }
}
