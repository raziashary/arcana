//! Open every database through core's own code and report its schema version.
//!
//! A migration is only real if the code that ships actually applies it. This
//! runs the same `open()` paths the app and the CLI use, then prints what each
//! database ended up at — so "every database has a ladder" is a measurement
//! rather than a claim (MDP `§7 P1-10`).
//!
//! It respects `USERPROFILE`/`HOME`, so it can be pointed at a copy of a real
//! data directory instead of the live one:
//!
//! ```text
//! HOME=/path/to/copy cargo run -p arcana-core --example migrate_check
//! ```
//!
//! Exits non-zero if any database fails to open or is left unstamped.

fn main() {
    let mut failures: Vec<String> = Vec::new();
    let mut rows: Vec<(String, String)> = Vec::new();

    macro_rules! check {
        ($label:expr, $open:expr) => {
            match $open {
                Ok(db) => match db.schema_version() {
                    Ok(0) => {
                        failures.push(format!("{}: opened but still at user_version 0", $label));
                        rows.push(($label.to_string(), "0  UNSTAMPED".into()));
                    }
                    Ok(n) => rows.push(($label.to_string(), format!("v{n}"))),
                    Err(e) => {
                        failures.push(format!("{}: {e}", $label));
                        rows.push(($label.to_string(), format!("ERROR {e}")));
                    }
                },
                Err(e) => {
                    failures.push(format!("{}: {e}", $label));
                    rows.push(($label.to_string(), format!("OPEN FAILED {e}")));
                }
            }
        };
    }

    check!("workers.db", arcana_core::db::workers::WorkersDb::open());
    check!("tasks.db", arcana_core::db::tasks::TasksDb::open());
    check!("plans.db", arcana_core::db::plans::PlansDb::open());
    check!("runs.db", arcana_core::db::runs::RunsDb::open());
    check!(
        "annotations.db",
        arcana_core::db::annotations::AnnotationsDb::open()
    );
    check!("memory.db", arcana_core::memory::db::MemoryDb::open());
    // config.db has no wrapper type; its version is read directly.
    match arcana_core::db::config_db::open() {
        Ok(conn) => match arcana_core::db::migrate::user_version(&conn) {
            Ok(0) => {
                failures.push("config.db: opened but still at user_version 0".into());
                rows.push(("config.db".into(), "0  UNSTAMPED".into()));
            }
            Ok(n) => rows.push(("config.db".into(), format!("v{n}"))),
            Err(e) => {
                failures.push(format!("config.db: {e}"));
                rows.push(("config.db".into(), format!("ERROR {e}")));
            }
        },
        Err(e) => {
            failures.push(format!("config.db: {e}"));
            rows.push(("config.db".into(), format!("OPEN FAILED {e}")));
        }
    }

    // The session store, which P1-14 moved off the retired `~/.airs` root.
    // Opening it performs that migration if it has not happened yet.
    match arcana_core::session::storage::SessionStorage::open() {
        Ok(st) => match st.schema_version() {
            Ok(n) => rows.push(("arcana.db (sessions)".into(), format!("v{n}"))),
            Err(e) => {
                failures.push(format!("arcana.db: {e}"));
                rows.push(("arcana.db (sessions)".into(), format!("ERROR {e}")));
            }
        },
        Err(e) => {
            failures.push(format!("arcana.db: {e}"));
            rows.push(("arcana.db (sessions)".into(), format!("OPEN FAILED {e}")));
        }
    }

    // Analytics is async and keeps its connection behind a mutex.
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    match rt.block_on(arcana_core::analytics::db::AnalyticsDb::new()) {
        Ok(db) => {
            let conn = db.conn.lock().expect("analytics lock");
            match arcana_core::db::migrate::user_version(&conn) {
                Ok(0) => {
                    failures.push("analytics.db: opened but still at user_version 0".into());
                    rows.push(("analytics.db".into(), "0  UNSTAMPED".into()));
                }
                Ok(n) => rows.push(("analytics.db".into(), format!("v{n}"))),
                Err(e) => {
                    failures.push(format!("analytics.db: {e}"));
                    rows.push(("analytics.db".into(), format!("ERROR {e}")));
                }
            }
        }
        Err(e) => {
            failures.push(format!("analytics.db: {e}"));
            rows.push(("analytics.db".into(), format!("OPEN FAILED {e}")));
        }
    }

    // Per-repo graph stores live under `graphs/` and run their own ladder,
    // which reaches v6 (P1-11). They are discovered rather than named, because
    // there is one per repository the user has analysed.
    let graphs_dir = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(|h| std::path::PathBuf::from(h).join(".arcana").join("graphs"))
        .unwrap_or_default();
    if let Ok(entries) = std::fs::read_dir(&graphs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("db") {
                continue;
            }
            let label = format!("graphs/{}", path.file_name().unwrap().to_string_lossy());
            match arcana_core::graph::store::GraphStore::open(&path) {
                Ok(store) => match store.schema_version() {
                    Ok(n) if n >= 6 => rows.push((label, format!("v{n}"))),
                    Ok(n) => {
                        failures.push(format!("{label}: still at v{n}, expected v6"));
                        rows.push((label, format!("v{n}  BEHIND")));
                    }
                    Err(e) => {
                        failures.push(format!("{label}: {e}"));
                        rows.push((label, format!("ERROR {e}")));
                    }
                },
                Err(e) => {
                    failures.push(format!("{label}: {e}"));
                    rows.push((label, format!("OPEN FAILED {e}")));
                }
            }
        }
    }

    for (name, state) in &rows {
        println!("  {name:<18} {state}");
    }

    if failures.is_empty() {
        println!(
            "\nall {} databases opened and carry a schema version",
            rows.len()
        );
    } else {
        eprintln!("\n{} database(s) failed:", failures.len());
        for f in &failures {
            eprintln!("  - {f}");
        }
        std::process::exit(1);
    }
}
