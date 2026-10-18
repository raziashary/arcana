// Unified error types for all Arcana backend modules.
// Uses thiserror for ergonomic error derivation.
// All Tauri commands serialize errors as strings for the frontend.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArcanaError {
    // Vault / keychain errors
    #[error("Vault error: {0}")]
    Vault(String),

    // Provider / LLM API errors
    #[error("Provider error: {0}")]
    Provider(String),

    /// A provider failure whose retry class the producing adapter determined from
    /// a bounded response body. The class is carried as data so no consumer has to
    /// parse it back out of the message text (ADR-LOG-RESTRUCT-0399).
    ///
    /// The `{class:?}` tag in the rendered text is **display only** — it keeps the
    /// classification legible after `ArcanaError` serialises to a string across
    /// the Tauri IPC boundary, where the type cannot survive. No Rust consumer may
    /// read it: consumers `match` on this variant.
    ///
    /// `ErrorClass` lives in `provider::usage` and is referenced here
    /// deliberately: this is intra-crate, the dependence is one-way in effect
    /// (`usage.rs` already builds `ArcanaError` values), and a separate error
    /// type per provider taxonomy would buy nothing.
    #[error("Provider error [{class:?}]: {message}")]
    ProviderClassified {
        class: crate::provider::usage::ErrorClass,
        message: String,
    },

    // Agent / tool execution errors
    #[error("Agent error: {0}")]
    Agent(String),

    #[error(
        "Operation reconciliation required for '{operation}' at journal entry {intent_cursor}"
    )]
    OperationReconciliationRequired {
        operation: String,
        intent_cursor: i64,
    },

    #[error("Operation cancelled")]
    Cancelled,

    // Budget circuit breaker — Ruflo Feature 4
    #[error("Budget exceeded: {0}")]
    BudgetExceeded(String),

    // Tool execution errors
    #[error("Tool execution error [{tool}]: {message}")]
    ToolExecution { tool: String, message: String },

    // Permission denied
    #[error("Permission denied for tool '{tool}'")]
    PermissionDenied { tool: String },

    #[error("Workspace egress denied for destination '{destination}'")]
    EgressDenied { destination: String },

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

pub type Result<T> = std::result::Result<T, ArcanaError>;

// Required to serialize errors across Tauri IPC boundary.
impl serde::Serialize for ArcanaError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}
