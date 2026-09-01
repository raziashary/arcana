// Unified error types for all AutoDev backend modules.
// Uses thiserror for ergonomic error derivation.
// All Tauri commands serialize errors as strings for the frontend.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AutoDevError {
    // Vault / keychain errors
    #[error("Vault error: {0}")]
    Vault(String),

    // Provider / LLM API errors
    #[error("Provider error: {0}")]
    Provider(String),

    // Agent / tool execution errors
    #[error("Agent error: {0}")]
    Agent(String),

    // Budget circuit breaker — Ruflo Feature 4
    #[error("Budget exceeded: {0}")]
    BudgetExceeded(String),

    // Tool execution errors
    #[error("Tool execution error [{tool}]: {message}")]
    ToolExecution { tool: String, message: String },

    // Permission denied
    #[error("Permission denied for tool '{tool}'")]
    PermissionDenied { tool: String },

    // Session / SQLite errors
    #[error("Session error: {0}")]
    Session(String),

    // Git / libgit2 errors (Phase 3.3)
    #[error("Git error: {0}")]
    Git(String),

    // IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // SQLite errors
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    // HTTP / reqwest errors
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    // Serde errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    // Generic anyhow passthrough
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, AutoDevError>;

// Required to serialize errors across Tauri IPC boundary.
impl serde::Serialize for AutoDevError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}
