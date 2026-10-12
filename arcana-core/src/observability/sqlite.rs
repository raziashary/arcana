//! Privacy-safe timing for production SQLite statements.
//!
//! rusqlite's 0.31 profile callback receives expanded SQL, including bound
//! values. This module deliberately discards that string and records only a
//! fixed statement category, the database's logical filename, and elapsed
//! time. It never persists SQL, parameters, or a query fingerprint.

use rusqlite::Connection;
use std::path::Path;
use std::time::Duration;

fn statement_kind(sql: &str) -> &'static str {
    match sql
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase()
        .as_str()
    {
        "SELECT" | "WITH" => "read",
        "INSERT" | "REPLACE" => "insert",
        "UPDATE" => "update",
        "DELETE" => "delete",
        "CREATE" | "ALTER" | "DROP" => "schema",
        "PRAGMA" => "pragma",
        "BEGIN" | "COMMIT" | "END" | "ROLLBACK" | "SAVEPOINT" | "RELEASE" => "transaction",
        _ => "other",
    }
}

macro_rules! profiler {
    ($name:ident, $database:literal) => {
        fn $name(sql: &str, elapsed: Duration) {
            let database = $database;
            let operation = statement_kind(sql);
            let duration_ns = elapsed.as_nanos() as u64;
            // The profile hook observes every pragma and statement; emitting
            // all of them at INFO turned a short Fleet startup into thousands
            // of log rows, most reporting 0 ns. Keep complete detail available
            // at DEBUG and surface only materially slow statements by default.
            if elapsed >= Duration::from_millis(10) {
                tracing::info!(
                    target: "arcana::telemetry",
                    event_type = "database_statement",
                    database,
                    operation,
                    duration_ns,
                    // SQLite's profile callback receives elapsed time and SQL
                    // text, but no execution result. Do not imply the statement
                    // succeeded when this callback cannot observe its result.
                    outcome = "duration_observed",
                    "SQLite statement finished"
                );
            } else {
                tracing::debug!(
                    target: "arcana::telemetry",
                    event_type = "database_statement",
                    database,
                    operation,
                    duration_ns,
                    outcome = "duration_observed",
                    "SQLite statement finished"
                );
            }
        }
    };
}

profiler!(profile_analytics, "analytics");
profiler!(profile_annotations, "annotations");
profiler!(profile_arcana, "arcana");
profiler!(profile_config, "config");
profiler!(profile_graph, "graph");
profiler!(profile_memory, "memory");
profiler!(profile_plans, "plans");
profiler!(profile_runs, "runs");
profiler!(profile_tasks, "tasks");
profiler!(profile_workers, "workers");
profiler!(profile_other, "other");
profiler!(profile_backup_snapshot, "backup_snapshot");

/// Attach statement timing to a production SQLite connection.
pub fn install_profile(conn: &mut Connection, path: &Path) {
    let database = logical_database_name(path);
    install_profile_for(conn, database);
}

fn logical_database_name(path: &Path) -> &str {
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if filename == "graph.db" {
        return "graph.db";
    }

    let is_repo_graph = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        == Some("graphs")
        && filename.strip_suffix(".db").is_some_and(|stem| {
            stem.len() == 12 && stem.bytes().all(|byte| byte.is_ascii_hexdigit())
        });
    if is_repo_graph {
        "graph.db"
    } else {
        filename
    }
}

/// Attach statement timing when a SQLite connection's role is more specific
/// than its filename (for example, an integrity-checked backup snapshot).
pub(crate) fn install_profile_for(conn: &mut Connection, database: &str) {
    #[cfg(test)]
    crate::db::test_clock::install(conn).expect("install isolated SQLite test clock");
    let callback = match database {
        "analytics.db" => profile_analytics as fn(&str, Duration),
        "annotations.db" => profile_annotations,
        "arcana.db" => profile_arcana,
        "config.db" => profile_config,
        "graph.db" => profile_graph,
        "memory.db" => profile_memory,
        "plans.db" => profile_plans,
        "runs.db" => profile_runs,
        "tasks.db" => profile_tasks,
        "workers.db" => profile_workers,
        "backup_snapshot" => profile_backup_snapshot,
        _ => profile_other,
    };
    conn.profile(Some(callback));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    static PROFILED_STATEMENTS: AtomicUsize = AtomicUsize::new(0);
    static ZERO_DURATION_SAMPLES: AtomicUsize = AtomicUsize::new(0);
    static NONZERO_DURATION_SAMPLES: AtomicUsize = AtomicUsize::new(0);

    fn test_profile(_sql: &str, _elapsed: Duration) {
        PROFILED_STATEMENTS.fetch_add(1, Ordering::SeqCst);
    }

    fn sample_profile_resolution(_sql: &str, elapsed: Duration) {
        if elapsed.is_zero() {
            ZERO_DURATION_SAMPLES.fetch_add(1, Ordering::SeqCst);
        } else {
            NONZERO_DURATION_SAMPLES.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn run_select_fixture(conn: &Connection, iterations: usize) {
        let mut statement = conn
            .prepare("SELECT ?1 + 1")
            .expect("prepare benchmark query");
        for value in 0..iterations {
            let result: i64 = statement
                .query_row([value as i64], |row| row.get(0))
                .expect("run benchmark query");
            assert_eq!(result, value as i64 + 1);
        }
    }

    #[test]
    fn statement_kind_never_returns_sql_or_parameter_content() {
        assert_eq!(statement_kind("SELECT 'private memory text'"), "read");
        assert_eq!(
            statement_kind("INSERT INTO memories VALUES ('secret')"),
            "insert"
        );
        assert_eq!(
            statement_kind("UPDATE tasks SET title = 'sensitive'"),
            "update"
        );
        assert_eq!(statement_kind("DELETE FROM memories"), "delete");
        assert_eq!(statement_kind("PRAGMA journal_mode"), "pragma");
        assert_eq!(statement_kind("-- comment\nSELECT ?1"), "other");
    }

    #[test]
    fn per_repository_graph_files_keep_the_logical_graph_database_identity() {
        assert_eq!(
            logical_database_name(Path::new("C:/Users/example/.arcana/graphs/abcdef123456.db")),
            "graph.db"
        );
        assert_eq!(
            logical_database_name(Path::new("C:/Users/example/.arcana/other/abcdef123456.db")),
            "abcdef123456.db"
        );
        assert_eq!(logical_database_name(Path::new("tasks.db")), "tasks.db");
    }

    #[test]
    fn sqlite_profile_observes_executed_statement_without_retaining_sql() {
        let before = PROFILED_STATEMENTS.load(Ordering::SeqCst);
        let mut conn = Connection::open_in_memory().expect("open isolated SQLite");
        conn.profile(Some(test_profile));
        let _: String = conn
            .query_row("SELECT ?1", ["private memory text"], |row| row.get(0))
            .expect("query fixture");
        conn.profile(None);
        assert!(
            PROFILED_STATEMENTS.load(Ordering::SeqCst) > before,
            "SQLite profile callback should observe the statement"
        );
    }

    #[test]
    fn production_profile_emits_structured_identity_and_duration_without_sql_or_values() {
        struct SharedWriter(Arc<Mutex<Vec<u8>>>);

        impl Write for SharedWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0
                    .lock()
                    .expect("captured SQLite telemetry")
                    .extend_from_slice(bytes);
                Ok(bytes.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let output = Arc::new(Mutex::new(Vec::new()));
        let writer_output = Arc::clone(&output);
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(move || SharedWriter(Arc::clone(&writer_output)))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let mut conn = Connection::open_in_memory().expect("isolated SQLite connection");
            install_profile_for(&mut conn, "tasks.db");
            let value: String = conn
                .query_row("SELECT ?1", ["private bound value"], |row| row.get(0))
                .expect("profiled query succeeds");
            conn.profile(None);
            assert_eq!(value, "private bound value");
        });

        let output = String::from_utf8(output.lock().expect("captured output").clone())
            .expect("JSON telemetry output");
        let event = output
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSON event"))
            .find(|event| event["fields"]["event_type"] == "database_statement")
            .expect("production SQLite event");
        let fields = &event["fields"];
        assert_eq!(fields["database"], "tasks");
        assert_eq!(fields["operation"], "read");
        assert_eq!(fields["outcome"], "duration_observed");
        assert!(fields["duration_ns"].as_u64().is_some());
        assert!(!output.contains("SELECT ?1"));
        assert!(!output.contains("private bound value"));
    }

    #[test]
    fn measure_profile_callback_overhead_and_duration_resolution() {
        const ITERATIONS: usize = 20_000;
        use std::time::Instant;

        let baseline = Connection::open_in_memory().expect("open baseline SQLite");
        let started = Instant::now();
        run_select_fixture(&baseline, ITERATIONS);
        let baseline_elapsed = started.elapsed();

        let mut profiled = Connection::open_in_memory().expect("open profiled SQLite");
        profiled.profile(Some(sample_profile_resolution));
        let started = Instant::now();
        run_select_fixture(&profiled, ITERATIONS);
        let callback_elapsed = started.elapsed();
        profiled.profile(None);

        let zeros = ZERO_DURATION_SAMPLES.swap(0, Ordering::SeqCst);
        let nonzeros = NONZERO_DURATION_SAMPLES.swap(0, Ordering::SeqCst);
        assert_eq!(zeros + nonzeros, ITERATIONS);

        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::DEBUG)
            .with_current_span(true)
            .with_span_list(true)
            .with_writer(std::io::sink)
            .finish();
        let mut production_profiled =
            Connection::open_in_memory().expect("open production fixture");
        install_profile_for(&mut production_profiled, "tasks.db");
        let production_elapsed = tracing::subscriber::with_default(subscriber, || {
            let started = Instant::now();
            run_select_fixture(&production_profiled, ITERATIONS);
            started.elapsed()
        });
        production_profiled.profile(None);

        eprintln!(
            "sqlite_profile_measurement iterations={ITERATIONS} baseline_total_ns={} callback_total_ns={} production_event_total_ns={} zero_duration_samples={zeros} nonzero_duration_samples={nonzeros}",
            baseline_elapsed.as_nanos(),
            callback_elapsed.as_nanos(),
            production_elapsed.as_nanos(),
        );
    }
}
