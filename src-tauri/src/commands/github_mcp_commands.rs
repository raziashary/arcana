// GitHub MCP configuration commands.
// Phase 2.3: T3 — GitHub MCP integration: store server config (org, repos, enabled).
// PAT is stored separately in the OS keychain via vault_commands.
// See: DEVELOPMENT_PLAN.md Phase 2.3 — GitHub MCP Integration

use crate::db::autodev_dir;
use crate::errors::{AutoDevError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitHubMcpConfig {
    pub enabled: bool,
    pub org: Option<String>,        // GitHub org / username to scope to
    pub repos: Vec<String>,         // explicit repo slugs (empty = all accessible)
    pub server_url: Option<String>, // custom MCP server URL (None = stdio default)
}

// ─── Storage helpers (config.db) ─────────────────────────────────────────────

const KEY: &str = "github_mcp_config";

// `config.db` is opened through `db::app_settings`, the single owner of the
// `app_settings` table (P1-15). This file used to declare its own
// `CREATE TABLE IF NOT EXISTS` — one of three that disagreed about whether
// the `updated_at` trigger was part of the schema, so first caller won.

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Retrieve the current GitHub MCP server configuration.
/// Returns a default (disabled) config if none has been saved.
#[tauri::command]
pub fn get_github_mcp_config() -> Result<GitHubMcpConfig> {
    let conn = crate::db::app_settings::open()?;
    let row: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![KEY],
            |r| r.get(0),
        )
        .optional()
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
    enabled: bool,
    org: Option<String>,
    repos: Vec<String>,
    server_url: Option<String>,
) -> Result<()> {
    let cfg = GitHubMcpConfig {
        enabled,
        org,
        repos,
        server_url,
    };
    let json = serde_json::to_string(&cfg)
        .map_err(|e| AutoDevError::Agent(format!("serialize github mcp config: {e}")))?;

    let conn = crate::db::app_settings::open()?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![KEY, json],
    )
    .map_err(|e| AutoDevError::Agent(format!("set_github_mcp_config: {e}")))?;
    Ok(())
}
