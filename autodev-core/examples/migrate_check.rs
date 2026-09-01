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
//! HOME=/path/to/copy cargo run -p autodev-core --example migrate_check
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

    check!("workers.db", autodev_core::db::workers::WorkersDb::open());
    check!("tasks.db", autodev_core::db::tasks::TasksDb::open());
    check!("plans.db", autodev_core::db::plans::PlansDb::open());
    check!(
        "annotations.db",
        autodev_core::db::annotations::AnnotationsDb::open()
    );
    check!("memory.db", autodev_core::memory::db::MemoryDb::open());
    // config.db has no wrapper type; its version is read directly.
    match autodev_core::db::config_db::open() {
        Ok(conn) => match autodev_core::db::migrate::user_version(&conn) {
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

    // Analytics is async and keeps its connection behind a mutex.
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    match rt.block_on(autodev_core::analytics::db::AnalyticsDb::new()) {
        Ok(db) => {
            let conn = db.conn.lock().expect("analytics lock");
            match autodev_core::db::migrate::user_version(&conn) {
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
