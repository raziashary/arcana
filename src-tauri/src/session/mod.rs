// Session module — SQLite-backed conversation persistence.
// Distinct from core-orchestrator's memory storage: this holds UI-level
// session history (messages, metadata) for the desktop app.

pub mod types;
pub mod storage;
pub mod manager;

pub use types::{Session, Message, MessageRole, ToolCall, ToolResult};
pub use manager::SessionManager;
