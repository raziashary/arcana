// GitHub MCP configuration commands.
// Phase 2.3: T3 — GitHub MCP integration: store server config (org, repos, enabled).
// PAT is stored separately in the OS keychain via vault_commands.
// See: DEVELOPMENT_PLAN.md Phase 2.3 — GitHub MCP Integration

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use crate::errors::{AutoDevError, Result};
use crate::db::autodev_dir;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitHubMcpConfig {
    pub enabled:       bool,
    pub org:           Option<String>,             // GitHub org / username to scope to
    pub repos:         Vec<String>,                // explicit repo slugs (empty = all accessible)
    pub server_url:    Option<String>,             // custom MCP server URL (None = stdio default)
}

// ─── Storage helpers (config.db) ─────────────────────────────────────────────

const KEY: &str = "github_mcp_config";

fn open_config_db() -> Result<Connection> {
    let path = autodev_dir()?.join("config.db");
    let conn = Connection::open(&path)
        .map_err(|e| AutoDevError::Agent(format!("config.db open: {e}")))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_settings (
             key   TEXT PRIMARY KEY,
             value TEXT NOT NULL,
             updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TRIGGER IF NOT EXISTS update_app_settings_ts
             AFTER UPDATE ON app_settings
             BEGIN
                 UPDATE app_settings SET updated_at = CURRENT_TIMESTAMP WHERE key = NEW.key;
             END;",
    ).map_err(|e| AutoDevError::Agent(format!("app_settings migrate: {e}")))?;
    Ok(conn)
}

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Retrieve the current GitHub MCP server configuration.
/// Returns a default (disabled) config if none has been saved.
#[tauri::command]
pub fn get_github_mcp_config() -> Result<GitHubMcpConfig> {
    let conn = open_config_db()?;
    let row: Option<String> = conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![KEY],
        |r| r.get(0),
    ).optional()
     .map_err(|e| AutoDevError::Agent(format!("get_github_mcp_config: {e}")))?;

    match row {
        Some(json) => serde_json::from_str(&json)
            .map_err(|e| AutoDevError::Agent(format!("parse github mcp config: {e}"))),
        None => Ok(GitHubMcpConfig::default()),
    }
}

/// Persist GitHub MCP configuration.
#[tauri::command]
pub fn set_github_mcp_config(
    enabled:    bool,
    org:        Option<String>,
    repos:      Vec<String>,
    server_url: Option<String>,
) -> Result<()> {
    let cfg = GitHubMcpConfig { enabled, org, repos, server_url };
    let json = serde_json::to_string(&cfg)
        .map_err(|e| AutoDevError::Agent(format!("serialize github mcp config: {e}")))?;

    let conn = open_config_db()?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![KEY, json],
    ).map_err(|e| AutoDevError::Agent(format!("set_github_mcp_config: {e}")))?;
    Ok(())
}
