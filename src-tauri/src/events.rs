// Tauri event emission helpers.
// All agent → frontend communication uses named Tauri events.
// The frontend subscribes to these via @tauri-apps/api listen().

use serde::Serialize;
use tauri::{AppHandle, Emitter};

// ── Event name constants ──────────────────────────────────────────────────────

pub const EVT_AGENT_TOKEN:       &str = "agent:token";
pub const EVT_AGENT_THINKING:    &str = "agent:thinking";
pub const EVT_AGENT_TOOL_START:  &str = "agent:tool-start";
pub const EVT_AGENT_TOOL_END:    &str = "agent:tool-end";
pub const EVT_AGENT_PERMISSION:  &str = "agent:permission-request";
pub const EVT_AGENT_DONE:        &str = "agent:done";
pub const EVT_AGENT_ERROR:       &str = "agent:error";
pub const EVT_COST_UPDATE:       &str = "cost:update";

// ── Payload types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct TokenPayload {
    pub session_id: String,
    pub text:       String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ThinkingPayload {
    pub session_id: String,
    pub text:       String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ToolStartPayload {
    pub session_id: String,
    pub call_id:    String,
    pub tool_name:  String,
    pub input:      serde_json::Value,
}

#[derive(Debug, Serialize, Clone)]
pub struct ToolEndPayload {
    pub session_id: String,
    pub call_id:    String,
    pub tool_name:  String,
    pub output:     String,
    pub is_error:   bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct PermissionPayload {
    pub session_id: String,
    pub call_id:    String,
    pub tool_name:  String,
    pub input:      serde_json::Value,
    pub description: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct AgentDonePayload {
    pub session_id:    String,
    pub message_id:    String,
    pub tokens_in:     u32,
    pub tokens_out:    u32,
    pub cost_usd:      f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct AgentErrorPayload {
    pub session_id: String,
    pub message:    String,
}

#[derive(Debug, Serialize, Clone)]
pub struct CostUpdatePayload {
    pub session_id:   String,
    pub tokens_in:    u32,
    pub tokens_out:   u32,
    pub cost_usd:     f64,
    pub total_today:  f64,
}

// ── Emit helpers ──────────────────────────────────────────────────────────────

pub fn emit_token(app: &AppHandle, payload: TokenPayload) {
    let _ = app.emit(EVT_AGENT_TOKEN, payload);
}

pub fn emit_thinking(app: &AppHandle, payload: ThinkingPayload) {
    let _ = app.emit(EVT_AGENT_THINKING, payload);
}

pub fn emit_tool_start(app: &AppHandle, payload: ToolStartPayload) {
    let _ = app.emit(EVT_AGENT_TOOL_START, payload);
}

pub fn emit_tool_end(app: &AppHandle, payload: ToolEndPayload) {
    let _ = app.emit(EVT_AGENT_TOOL_END, payload);
}

pub fn emit_permission_request(app: &AppHandle, payload: PermissionPayload) {
    let _ = app.emit(EVT_AGENT_PERMISSION, payload);
}

pub fn emit_agent_done(app: &AppHandle, payload: AgentDonePayload) {
    let _ = app.emit(EVT_AGENT_DONE, payload);
}

pub fn emit_agent_error(app: &AppHandle, payload: AgentErrorPayload) {
    let _ = app.emit(EVT_AGENT_ERROR, payload);
}

pub fn emit_cost_update(app: &AppHandle, payload: CostUpdatePayload) {
    let _ = app.emit(EVT_COST_UPDATE, payload);
}
