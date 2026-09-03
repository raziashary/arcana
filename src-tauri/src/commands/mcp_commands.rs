// mcp_commands.rs — AutoDev as an MCP *client*, and its own tool surface.
//
// 🔴 **Why this file exists (`P8-51`).** These three commands lived in
// `sidecar_commands.rs`, and `P8-06` says *"delete `sidecar_commands.rs` and
// all 17 commands"*. Executing that literally would have deleted a **live
// feature with a live UI**: none of the three has anything to do with the
// Python sidecar.
//
//   * `list_mcp_tools`     — reads the in-process `ToolRegistry`
//   * `get_mcp_health`     — reads `mcp::health::McpHealthMonitor`
//   * `force_reconnect_mcp` — reconnects a named **external** MCP server
//
// They were only ever neighbours of the sidecar code, and `mcp/mod.rs:1-14`
// already records relocating a file *"by address rather than deleting by
// association"* — this is the same mistake in reverse, caught by the `§0.2m`
// refutation before the deletion ran.
//
// ⚠ **The command names are unchanged.** The frontend invokes them by string;
// renaming during a move would be a silent breakage that no compiler catches.

use std::sync::Arc;

use tauri::State;

use autodev_core::agent::tool_registry::ToolRegistry;

/// One tool, as the MCP tool browser renders it.
///
/// Queried directly from the in-process `ToolRegistry` rather than over MCP
/// `tools/list`, which avoids a JSON-RPC round-trip for data we already hold.
#[derive(serde::Serialize)]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Every tool AutoDev exposes, for the MCP tool browser.
///
/// Lets a developer inspect the available capabilities without connecting an
/// external MCP client.
#[tauri::command]
pub fn list_mcp_tools(registry: State<'_, Arc<ToolRegistry>>) -> Vec<McpToolInfo> {
    let mut tools: Vec<McpToolInfo> = registry
        .all()
        .into_iter()
        .map(|t| McpToolInfo {
            name: t.name().to_string(),
            description: t.description().to_string(),
            input_schema: t.input_schema(),
        })
        .collect();
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    tools
}

/// Health of every external MCP server AutoDev is connected to.
#[tauri::command]
pub async fn get_mcp_health(
    monitor: State<'_, Arc<crate::mcp::health::McpHealthMonitor>>,
) -> Result<Vec<crate::mcp::health::McpServerHealth>, String> {
    Ok(monitor.get_all_health().await)
}

/// Manually trigger a reconnect for a named external MCP server.
///
/// Called when the user clicks "Reconnect" in the MCP health panel.
#[tauri::command]
pub async fn force_reconnect_mcp(
    server_name: String,
    monitor: State<'_, Arc<crate::mcp::health::McpHealthMonitor>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    monitor.force_reconnect(&server_name, &app).await;
    Ok(())
}
