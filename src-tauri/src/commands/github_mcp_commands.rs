// GitHub MCP configuration commands.
// Phase 2.3: T3 — GitHub MCP integration: store server config (org, repos, enabled).
// PAT is stored separately in the OS keychain via vault_commands.
// See: DEVELOPMENT_PLAN.md Phase 2.3 — GitHub MCP Integration

use crate::errors::{ArcanaError, Result};
use rusqlite::{params, OptionalExtension};
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
    let operation =
        arcana_core::observability::timing::OperationTimer::start("github_mcp_config_read")
            .with_database("config");
    let conn = crate::db::app_settings::open()?;
    let row: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![KEY],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| ArcanaError::Agent(format!("get_github_mcp_config: {e}")))?;

    match row {
        Some(json) => {
            let config = serde_json::from_str(&json)
                .map_err(|e| ArcanaError::Agent(format!("parse github mcp config: {e}")))?;
            operation.with_result_count(1).succeed();
            Ok(config)
        }
        None => {
            operation.with_result_count(0).set_outcome("empty");
            Ok(GitHubMcpConfig::default())
        }
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
    let operation =
        arcana_core::observability::timing::OperationTimer::start("github_mcp_config_write")
            .with_database("config");
    let cfg = GitHubMcpConfig {
        enabled,
        org,
        repos,
        server_url,
    };
    let json = serde_json::to_string(&cfg)
        .map_err(|e| ArcanaError::Agent(format!("serialize github mcp config: {e}")))?;

    let conn = crate::db::app_settings::open()?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![KEY, json],
    )
    .map_err(|e| ArcanaError::Agent(format!("set_github_mcp_config: {e}")))?;
    operation.with_result_count(1).succeed();
    Ok(())
}

#[cfg(test)]
mod observability_tests {
    use super::*;
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .expect("capture GitHub config telemetry")
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn github_mcp_configuration_operations_emit_private_result_spans() {
        let directory = tempfile::tempdir().expect("isolated config home");
        let _home = crate::test_support::isolate_home(directory.path());
        let output = Arc::new(Mutex::new(Vec::new()));
        let writer_output = Arc::clone(&output);
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || SharedWriter(Arc::clone(&writer_output)))
            .finish();

        let (observed, ()) = tracing::subscriber::with_default(subscriber, || {
            let empty = get_github_mcp_config().expect("read missing configuration");
            set_github_mcp_config(
                true,
                Some("private-org-marker".into()),
                vec!["private-repo-marker".into()],
                Some("https://private-server-marker.example".into()),
            )
            .expect("write configuration");
            let populated = get_github_mcp_config().expect("read configuration");
            let conn = crate::db::app_settings::open().expect("open isolated settings");
            crate::db::app_settings::set_in(&conn, KEY, "private-invalid-json-marker")
                .expect("write malformed configuration fixture");
            let malformed = get_github_mcp_config().is_err();
            conn.execute_batch(
                "CREATE TRIGGER fail_github_config_update
                 BEFORE UPDATE OF value ON app_settings WHEN OLD.key = 'github_mcp_config'
                 BEGIN SELECT RAISE(ABORT, 'private-config-sql-error-marker'); END;",
            )
            .expect("install isolated config failure trigger");
            let failed_write = set_github_mcp_config(false, None, Vec::new(), None).is_err();
            ((empty, populated, malformed, failed_write), ())
        });
        assert!(!observed.0.enabled);
        assert_eq!(observed.1.org.as_deref(), Some("private-org-marker"));
        assert!(observed.2);
        assert!(observed.3);

        let output = String::from_utf8(output.lock().expect("captured telemetry").clone())
            .expect("UTF-8 telemetry");
        let events: Vec<serde_json::Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).expect("JSON event"))
            .collect();
        let operations: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        for (operation, outcome) in [
            ("github_mcp_config_read", "empty"),
            ("github_mcp_config_read", "success"),
            ("github_mcp_config_read", "error"),
            ("github_mcp_config_write", "success"),
            ("github_mcp_config_write", "error"),
        ] {
            assert!(
                operations.iter().any(|event| {
                    event["fields"]["operation"] == operation
                        && event["fields"]["outcome"] == outcome
                        && event["fields"]["database"] == "config"
                }),
                "missing {operation} outcome {outcome}"
            );
        }
        let serialized = serde_json::to_string(&operations).expect("serialize config spans");
        for private_value in [
            "private-org-marker",
            "private-repo-marker",
            "private-server-marker",
            "private-invalid-json-marker",
            "private-config-sql-error-marker",
        ] {
            assert!(
                !serialized.contains(private_value),
                "GitHub configuration telemetry leaked {private_value}"
            );
        }
    }
}
