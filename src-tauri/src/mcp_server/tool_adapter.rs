// tool_adapter.rs — Bridges the existing `Tool` trait to rmcp's `ServerHandler`.
//
// Every tool registered in `agent::tool_registry::ToolRegistry` is dynamically
// exposed via MCP `tools/list`. Calls are dispatched back to the canonical
// Rust implementation via `Tool::execute()`.

use std::path::Path;
use std::sync::Arc;

use tracing::{debug, info, warn};
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
use crate::agent::builtin_tools::path_lock;

// Bug B — per-path write-serialisation. The registry itself lives in
// `agent::builtin_tools::path_lock` so PA mode (write_file / edit_file /
// append_file calling `path_lock::acquire_for` directly) and Mode 2 (this
// adapter acquiring the same key on behalf of the tool) contend on a
// single mutex per file. See path_lock.rs for the design notes.
//
// The adapter's job here is to acquire the lock at the *MCP entry boundary*
// for any non-read-only tool that exposes a `path` argument. Tools that
// already call `path_lock::acquire_for` inside their `execute()` will then
// re-enter the same async mutex from the same task, which is safe because
// tokio::sync::Mutex is not reentrant — so we ONLY hold the adapter-level
// guard for tools where the lock isn't taken inside the tool itself (e.g.
// a future write tool that hasn't been migrated). For the canonical
// write/edit/append trio the inner acquisition is the source of truth and
// we intentionally skip the adapter-level acquisition for them by name.
const PATH_LOCKED_INSIDE_TOOL: &[&str] = &["write_file", "edit_file", "append_file"];

/// Resolve an absolute key for the per-path lock. Returns None if the args
/// don't carry a string `path` field.
fn extract_path_arg(args: &serde_json::Map<String, serde_json::Value>) -> Option<&str> {
    args.get("path").and_then(|v| v.as_str())
}

/// MCP server that wraps the canonical AutoDev `ToolRegistry`.
/// Implements `ServerHandler` so rmcp can serve it over stdio or HTTP.
#[derive(Clone)]
pub struct AutodevMcpServer {
    registry: Arc<ToolRegistry>,
    working_dir: String,
}

impl AutodevMcpServer {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        // Doc 07 Phase 2 — prefer the Tauri-stored configured workspace
        // over `std::env::current_dir()`. The latter is `src-tauri/` during
        // `tauri dev`, which caused the 14 May calendar run's file-escape
        // bug (worker wrote `server.js` etc. into the repo's src-tauri/).
        //
        // Resolution order, with a startup log line documenting which
        // branch fired:
        //   1. config.db's `workspace_dir` (set via Settings → Workspace).
        //   2. `std::env::current_dir()`.
        //   3. "." with a loud warn — process can't determine its own cwd.
        let (working_dir, source) = match crate::commands::workspace_commands::load_workspace_dir() {
            ws if !ws.trim().is_empty() => (ws, "tauri-stored"),
            _ => match std::env::current_dir() {
                Ok(p) => {
                    let s = p.to_string_lossy().to_string();
                    warn!(
                        "[autodev-mcp-server] No configured workspace_dir; \
                         falling back to current_dir={s}. Fleet workers MUST \
                         pass __working_dir on every tool call to target the \
                         correct project root."
                    );
                    (s, "env-cwd-fallback")
                }
                Err(_) => {
                    warn!(
                        "[autodev-mcp-server] Cannot determine cwd for default \
                         working_dir. Tools called without __working_dir will \
                         resolve against '.'."
                    );
                    (".".to_string(), "dot-fallback")
                }
            },
        };
        // Doc 07 Phase 2 — startup assertion log. A log scan can spot when
        // the load-bearing fallback degrades from tauri-stored → env-cwd.
        info!(
            "[autodev-mcp-server] working_dir resolved: source={source} path={working_dir}"
        );
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

        // L.5 — Accept __agent_role so fleet workers can identify themselves.
        // Required for request_supervisor_action to record the correct worker_slug
        // instead of falling back to "unknown" for all MCP-originated calls.
        let agent_role = args.remove("__agent_role")
            .and_then(|v| v.as_str().map(String::from));

        // L.3 — Accept __session_id so all tool calls from one worker task share
        // a session, grouping checkpoints for possible rollback.
        //
        // Bug G defensive: when the caller omits __session_id we previously
        // minted a bare uuid which, in the (theoretical) worst case where two
        // unrelated agents happened to mint colliding ids, would interleave
        // their checkpoint streams. We now (a) log a warn so the missing
        // meta-arg is visible, and (b) seed the synthetic id with the
        // agent_role when we know it, guaranteeing per-agent isolation even
        // if the entropy in uuid generation degenerates.
        let session_id = match args.remove("__session_id")
            .and_then(|v| v.as_str().map(String::from))
        {
            Some(s) if !s.trim().is_empty() => s,
            _ => {
                let synthetic = match &agent_role {
                    Some(role) => format!("mcp-{role}-{}", uuid::Uuid::new_v4()),
                    None       => format!("mcp-anon-{}",  uuid::Uuid::new_v4()),
                };
                warn!(
                    "[autodev-mcp-server] tool '{}' invoked without __session_id; \
                     using synthetic id '{}'. Caller (orchestrator MCP bridge) \
                     should forward __session_id on every tool call so \
                     checkpoint streams stay per-agent.",
                    tool_name, synthetic
                );
                synthetic
            }
        };

        // L.6 — Accept __depth so subagents can propagate their nesting depth to
        // SpawnSubagentTool's guard (`ctx.depth >= MAX_SUBAGENT_DEPTH`).
        // Without this, MCP calls always see depth=0 and the depth guard never
        // fires, allowing unbounded subagent nesting until the pool hard-caps hit.
        // Subagents receive their depth in the [AUTODEV WORKSPACE] header injected
        // by SubagentBridge and are instructed to forward it on every tool call.
        let depth: u32 = args.remove("__depth")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32)
            .unwrap_or(0);

        let Some(tool) = self.registry.get(&tool_name) else {
            return Err(McpError::invalid_params(
                format!("Tool '{}' not found in autodev-mcp-server", tool_name),
                None,
            ));
        };

        // Bug B — acquire the per-path mutex for write tools that name a
        // file *and* don't acquire the lock themselves. The canonical
        // write/edit/append tools call path_lock::acquire_for inside
        // their execute() bodies (see PATH_LOCKED_INSIDE_TOOL); for those
        // we deliberately skip the adapter-level acquisition to avoid
        // attempting a non-reentrant re-lock on the same key.
        //
        // For every other non-read-only tool that names a `path`, we hold
        // the lock here for the duration of `tool.execute`. Tools without
        // a `path` argument (bash, subagent spawns, task-board ops) skip
        // the lock and rely on their own coordination model.
        let _path_guard = if !tool.is_read_only()
            && !PATH_LOCKED_INSIDE_TOOL.contains(&tool_name.as_ref())
        {
            match extract_path_arg(&args) {
                Some(raw) => {
                    let target = Path::new(raw);
                    let key = path_lock::lock_key(target, &working_dir);
                    debug!(
                        "[autodev-mcp-server] acquiring per-path lock for tool='{}' path='{}'",
                        tool_name, key.display()
                    );
                    Some(path_lock::acquire_owned(&key).await)
                }
                None => None,
            }
        } else {
            None
        };

        let args = serde_json::Value::Object(args);

        // Build a synthetic ToolContext using the per-call overrides.
        let ctx = ToolContext {
            session_id,
            working_dir,
            call_id: uuid::Uuid::new_v4().to_string(),
            depth,
            agent_role,
        };

        let result = tool.execute(args, &ctx).await;
        Ok(Self::wrap_result(result))
    }
}
