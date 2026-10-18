// Database module — houses all SQLite repositories beyond the session store.
// Each sub-module manages its own connection and migration.
// Databases, all under `~/.arcana/` unless noted:
//   tasks.db        — Task board (Supervisor ↔ Workers)
//   workers.db      — Worker registry (Phase 2.2)
//   runs.db         — Worker runs, run events, and the operation journal
//   plans.db        — Plan documents
//   config.db       — Settings and configuration
//   annotations.db  — File annotations
//   analytics.db    — Token/cost telemetry and fleet health
//   memory.db       — Memory records
//   arcana.db       — Session store (opened in `session/storage.rs`)
//   `<project>/.arcana/graph.db` — Per-project code graph (`graph/registry.rs`)
//
// Session data lives in ~/.arcana/arcana.db. It was ~/.airs/arcana.db until
// P1-14 consolidated the two data roots.

pub mod annotations;
pub mod app_settings;
pub mod arcana_instructions;
pub mod backup;
pub mod config_db;
pub mod migrate;
pub mod model_configs;
pub mod plans;
pub mod runs;
pub mod tasks;
#[cfg(test)]
pub mod test_support;
pub mod timestamps;
pub mod workers;

use crate::errors::{ArcanaError, Result};
use std::path::PathBuf;

/// How long a connection waits for a lock before giving up.
///
/// **Measured, not assumed:** SQLite's own default is zero — a busy database
/// returns `SQLITE_BUSY` immediately — but `rusqlite` overrides that and sets
/// 5000 ms on every connection it opens. Verified by reading
/// `PRAGMA busy_timeout` from a plain `Connection::open`, which reports 5000
/// with no configuration at all.
///
/// So this constant does **not** fix a bug. It makes the value ours and
/// explicit instead of inherited from a dependency's default, which can change
/// between versions without anything here noticing. The genuine defect
/// `open_tuned` fixes is `journal_mode` — see below.
pub const BUSY_TIMEOUT_MS: u64 = 5_000;

#[cfg(test)]
pub(crate) mod test_clock;

/// Open a SQLite database with the settings every connection here should have.
///
/// The real fix is `journal_mode`. Before P1 it was set at some open sites and
/// not others: `analytics.db`, `annotations.db` and `workers.db` were all
/// sitting in `journal=delete` on disk, and five of the six places
/// `graph/store.rs` opens a connection set nothing. Under `delete`, a reader
/// and a writer cannot proceed at once — which is the shape of nearly every
/// access here, the UI reading while the fleet writes.
///
/// Both pragmas are applied per-connection. `journal_mode` happens to persist
/// as a property of the file; `busy_timeout` does not and must be set on every
/// connection, which is why it belongs in one place rather than scattered.
///
/// A caller needing different flags (read-only, URI) opens directly; this is
/// the ordinary read-write path.
pub fn open_tuned(path: &std::path::Path) -> Result<rusqlite::Connection> {
    let mut operation =
        crate::observability::timing::OperationTimer::start("database_connection_open")
            .with_database("sqlite");
    let mut conn = rusqlite::Connection::open(path)
        .map_err(|e| ArcanaError::Agent(format!("opening {}: {e}", path.display())))?;
    crate::observability::sqlite::install_profile(&mut conn, path);
    conn.busy_timeout(std::time::Duration::from_millis(BUSY_TIMEOUT_MS))
        .map_err(|e| ArcanaError::Agent(format!("busy_timeout on {}: {e}", path.display())))?;
    // Concurrent first opens can receive SQLITE_BUSY during the journal-mode
    // upgrade without SQLite invoking its busy handler. Retry only lock errors
    // within the same bounded timeout; all other failures still propagate.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(BUSY_TIMEOUT_MS);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        conn.busy_timeout(remaining)
            .map_err(|e| ArcanaError::Agent(format!("busy_timeout on {}: {e}", path.display())))?;
        match conn.pragma_update(None, "journal_mode", "WAL") {
            Ok(()) => break,
            Err(rusqlite::Error::SqliteFailure(error, _))
                if matches!(
                    error.code,
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                ) && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(
                    deadline
                        .saturating_duration_since(std::time::Instant::now())
                        .min(std::time::Duration::from_millis(10)),
                );
            }
            Err(e) => {
                return Err(ArcanaError::Agent(format!(
                    "journal_mode on {}: {e}",
                    path.display()
                )))
            }
        }
    }
    conn.busy_timeout(std::time::Duration::from_millis(BUSY_TIMEOUT_MS))
        .map_err(|e| ArcanaError::Agent(format!("busy_timeout on {}: {e}", path.display())))?;
    operation.succeed();
    Ok(conn)
}

/// Returns the ~/.arcana directory, creating it if needed.
pub fn arcana_dir() -> Result<PathBuf> {
    let base = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| ArcanaError::Agent("Cannot determine home directory".into()))?;
    let dir = base.join(".arcana");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_tuned_sets_both_pragmas_that_matter() {
        let dir =
            std::env::temp_dir().join(format!("arcana-open-tuned-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).expect("create unique isolated directory");
        let path = dir.join("t.db");

        let conn = open_tuned(&path).expect("open");

        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .expect("journal_mode");
        assert_eq!(mode.to_lowercase(), "wal");

        // Read back on the same handle, because `busy_timeout` is
        // per-connection and is not stored in the file.
        //
        // Note this assertion is weak on its own: `rusqlite` already defaults
        // to 5000 ms, so it would pass even if `open_tuned` set nothing. It is
        // here to pin the value we intend, not to prove we caused it — see the
        // note on BUSY_TIMEOUT_MS.
        let busy: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |r| r.get(0))
            .expect("busy_timeout");
        assert_eq!(busy, BUSY_TIMEOUT_MS as i64);

        drop(conn);

        // `journal_mode` persists as a property of the file. This is the half
        // that was genuinely broken: analytics.db, annotations.db and
        // workers.db were all in `delete` mode on disk before P1.
        let plain = rusqlite::Connection::open(&path).expect("plain open");
        let mode2: String = plain
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode2.to_lowercase(), "wal", "journal_mode must persist");

        drop(plain);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tuned_connection_open_records_success_and_failure_without_paths() {
        let dir =
            std::env::temp_dir().join(format!("arcana-open-tuned-span-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).expect("create isolated directory");
        let valid_path = dir.join("valid.db");
        let invalid_path = dir.join("missing").join("invalid.db");
        let ((opened, failed), events) = crate::observability::timing::capture_json_events(|| {
            let opened = open_tuned(&valid_path).is_ok();
            let failed = open_tuned(&invalid_path).is_err();
            (opened, failed)
        });
        assert!(opened && failed);
        let spans = events
            .iter()
            .filter(|event| event["fields"]["operation"] == "database_connection_open")
            .collect::<Vec<_>>();
        assert_eq!(spans.len(), 2);
        assert!(spans
            .iter()
            .any(|event| event["fields"]["outcome"] == "success"));
        assert!(spans
            .iter()
            .any(|event| event["fields"]["outcome"] == "error"));
        let serialized = events.iter().map(ToString::to_string).collect::<String>();
        assert!(!serialized.contains(valid_path.to_string_lossy().as_ref()));
        assert!(!serialized.contains(invalid_path.to_string_lossy().as_ref()));
        std::fs::remove_dir_all(dir).expect("remove isolated fixture");
    }
}
