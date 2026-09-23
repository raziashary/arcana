// Session module — SQLite-backed conversation persistence.
// Distinct from the agent memory stores: this holds UI-level
// session history (messages, metadata) for the desktop app.

pub mod manager;
pub mod storage;
pub mod types;

pub use manager::SessionManager;
pub use types::{Message, MessageRole, Session, ToolCall, ToolResult};
