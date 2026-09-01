// A numbered migration ladder, shared by every SQLite database in the tree.
//
// Before P1 exactly one database had a ladder — the per-repo graph store — and
// every other one reported `user_version = 0` while carrying a schema that had
// been extended many times by hand. `CREATE TABLE IF NOT EXISTS` in an `open()`
// path is not a migration: it creates a table that is absent and does nothing
// at all to a table that exists but is missing a column. That is not a
// theoretical distinction. It is exactly how `airsctl` broke — `src-shared`
// declared `last_seen` in its `CREATE TABLE IF NOT EXISTS workers`, the table
// already existed without it, the statement did nothing, and every worker
// command failed with *no such column* from the day it was written
// (`ADR-LOG-RESTRUCT-0016`).
//
// Two rules this enforces, both bought with real defects:
//
//   * **A migration that fails is an error.** The graph store's v5 wrapped all
//     six of its statements in `let _ = …`, so an invalid
//     `ADD COLUMN IF NOT EXISTS` failed silently, the ladder stamped
//     `user_version = 5`, committed, and logged "✅ complete". Inert-but-green,
//     inside the migration system itself (`ADR-LOG-RESTRUCT-0013`).
//   * **The stamp and the work are one transaction.** Applying DDL and then
//     failing before the stamp leaves a database that re-runs the same step
//     forever; stamping first and then failing leaves one that never runs it
//     again. Both are silent.
//
// See MDP `§7 P1-10` and `INV-10`.

use rusqlite::Connection;

use crate::errors::{AutoDevError, Result};

/// One rung of a ladder.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    /// The `user_version` this migration brings the database to. Must be
    /// consecutive from 1 within a ladder.
    pub version: i32,
    /// Short identifier, used in errors and logs so a failure names the step.
    pub name: &'static str,
    /// SQL to run. Executed as a batch, so it may contain several statements.
    pub sql: &'static str,
}

/// Read a database's schema version.
pub fn user_version(conn: &Connection) -> Result<i32> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| AutoDevError::Agent(format!("reading user_version: {e}")))
}

/// Statements that can destroy data.
///
/// The first version of this list missed `DROP COLUMN` (valid and destructive
/// on SQLite 3.35+), `UPDATE`, and `INSERT OR REPLACE` — so a migration using
/// any of them would have run with **no backup attempted**. Found by the
/// post-phase audit.
const DESTRUCTIVE: &[&str] = &[
    "drop table",
    "drop index",
    "drop view",
    "drop trigger",
    "drop column",
    "delete from",
    "rename to",
    "rename column",
    "truncate",
    "insert or replace",
    "update ",
];

/// Remove `CREATE TRIGGER … END;` bodies before scanning for destructive verbs.
///
/// Every baseline rung in this tree defines an `updated_at` trigger, and a
/// trigger body *is* an `UPDATE`. Without this, adding `UPDATE` to the list
/// above would mark **every** baseline destructive — so a fresh install would
/// copy each database immediately after creating it, and the "baselines are
/// skipped" property that keeps first run cheap would silently invert.
fn strip_trigger_bodies(sql: &str) -> String {
    let lower = sql.to_ascii_lowercase();
    let mut out = String::with_capacity(sql.len());
    let mut cursor = 0usize;
    while let Some(rel) = lower[cursor..].find("create trigger") {
        let start = cursor + rel;
        out.push_str(&sql[cursor..start]);
        // A trigger body ends at `end;`. An unterminated one drops the rest,
        // which is the safe direction: it can only make this function *less*
        // likely to report destructive, and SQLite would not parse it anyway.
        match lower[start..].find("end;") {
            Some(e) => cursor = start + e + "end;".len(),
            None => return out,
        }
    }
    out.push_str(&sql[cursor..]);
    out
}

/// Whether a migration's SQL can destroy data.
///
/// `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX` cannot. This decides whether
/// `run_ladder` takes a backup before applying a rung, so the safety net is
/// automatic rather than something each future migration's author has to
/// remember.
fn is_destructive(sql: &str) -> bool {
    let lower = strip_trigger_bodies(sql).to_ascii_lowercase();
    DESTRUCTIVE.iter().any(|needle| lower.contains(needle))
}

/// Apply every migration in `ladder` whose version is above the database's
/// current `user_version`, in order, and return the version reached.
///
/// `db_name` appears in errors and logs; it is a label, not a path.
///
/// Each rung runs inside its own transaction together with its version stamp,
/// so a database is never left having done the work without recording it, or
/// having recorded it without doing the work.
pub fn run_ladder(conn: &Connection, db_name: &str, ladder: &[Migration]) -> Result<i32> {
    validate_ladder(db_name, ladder)?;

    let current = user_version(conn)?;
    let target = ladder.last().map(|m| m.version).unwrap_or(0);

    // A database from a newer build. Running an older ladder over it would
    // apply nothing and report success, so the process would carry on against
    // a schema it does not understand.
    if current > target {
        return Err(AutoDevError::Agent(format!(
            "{db_name} is at schema version {current}, but this build only knows up to {target}. \
             It was written by a newer version of AutoDev; refusing to run against it."
        )));
    }

    for m in ladder.iter().filter(|m| m.version > current) {
        // GAP-11 / §0.8: a migration that can destroy data is backed up first,
        // automatically. A baseline rung is `CREATE … IF NOT EXISTS` only and
        // is skipped, so the ordinary first-run path does not copy every
        // database on every start.
        //
        // A backup that cannot be taken **stops the migration**. Proceeding
        // with a destructive change on the belief that a backup happened is the
        // exact failure this is here to prevent.
        if is_destructive(m.sql) {
            // An in-memory database reports `Some("")` rather than `None`, and
            // is never user data — there is nothing to back up and nothing to
            // lose, so it proceeds.
            if let Some(path) = conn
                .path()
                .filter(|p| !p.is_empty())
                .map(std::path::PathBuf::from)
            {
                let reason = format!("{db_name}-v{}-{}", m.version, m.name);
                match crate::db::backup::backup_before(&path, &reason) {
                    Ok(b) => tracing::info!("[migrate] {}", b.describe()),
                    Err(e) => {
                        return Err(AutoDevError::Agent(format!(
                            "{db_name}: refusing to apply destructive migration v{} ({}) because no backup could be taken: {e}",
                            m.version, m.name
                        )))
                    }
                }
            }
        }

        conn.execute_batch("BEGIN")
            .map_err(|e| AutoDevError::Agent(format!("{db_name} v{}: begin: {e}", m.version)))?;

        let applied = conn
            .execute_batch(m.sql)
            .and_then(|_| conn.execute_batch(&format!("PRAGMA user_version = {}", m.version)));

        match applied {
            Ok(()) => {
                conn.execute_batch("COMMIT").map_err(|e| {
                    AutoDevError::Agent(format!("{db_name} v{}: commit: {e}", m.version))
                })?;
                tracing::info!("[migrate] {db_name}: applied v{} ({})", m.version, m.name);
            }
            Err(e) => {
                // Roll back before reporting, or the connection stays inside a
                // failed transaction and every later statement fails with a
                // confusing secondary error.
                let _ = conn.execute_batch("ROLLBACK");
                return Err(AutoDevError::Agent(format!(
                    "{db_name}: migration v{} ({}) failed and was rolled back: {e}",
                    m.version, m.name
                )));
            }
        }
    }

    user_version(conn)
}

/// Add any of `columns` that `table` does not already have.
///
/// Each entry is `(column_name, full ALTER statement)`. Existence is checked
/// with `PRAGMA table_info` rather than by attempting the `ALTER` and ignoring
/// the error, because that idiom cannot tell "already there" from "the
/// statement was wrong". `tasks.db` had twenty `let _ = ALTER TABLE …` lines
/// doing exactly that, and the graph store's v5 lost six real failures the
/// same way (`ADR-LOG-RESTRUCT-0013`).
///
/// `ADD COLUMN IF NOT EXISTS` is not valid SQLite, so this check is the
/// mechanism, not a convenience.
///
/// Returns the columns actually added. A missing table is not an error — a
/// fresh database has its columns created by the ladder's baseline rung —
/// so callers may run this before the ladder.
pub fn ensure_columns(
    conn: &Connection,
    table: &str,
    columns: &[(&str, &str)],
) -> Result<Vec<String>> {
    let table_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
            [table],
            |r| r.get(0),
        )
        .map_err(|e| AutoDevError::Agent(format!("probing {table}: {e}")))?;
    if table_exists == 0 {
        return Ok(Vec::new());
    }

    let existing: Vec<String> = {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(|e| AutoDevError::Agent(format!("table_info({table}) prepare: {e}")))?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(|e| AutoDevError::Agent(format!("table_info({table}) query: {e}")))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AutoDevError::Agent(format!("table_info({table}) collect: {e}")))?
    };

    let mut added = Vec::new();
    for (name, ddl) in columns {
        if existing.iter().any(|c| c == name) {
            continue;
        }
        conn.execute_batch(ddl).map_err(|e| {
            AutoDevError::Agent(format!("{table}: adding column {name} failed: {e}"))
        })?;
        added.push((*name).to_string());
    }
    if !added.is_empty() {
        tracing::info!("[migrate] {table}: added columns {added:?}");
    }
    Ok(added)
}

/// Reject a ladder that cannot be reasoned about, before it touches anything.
///
/// A duplicated or out-of-order version is a programming error that would
/// otherwise show up as a database silently skipping a step.
fn validate_ladder(db_name: &str, ladder: &[Migration]) -> Result<()> {
    for (i, m) in ladder.iter().enumerate() {
        let expected = i as i32 + 1;
        if m.version != expected {
            return Err(AutoDevError::Agent(format!(
                "{db_name}: ladder is not consecutive from 1 — entry {i} declares v{} where v{expected} was expected ({})",
                m.version, m.name
            )));
        }
        if m.sql.trim().is_empty() {
            return Err(AutoDevError::Agent(format!(
                "{db_name}: migration v{} ({}) has no SQL",
                m.version, m.name
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        Connection::open_in_memory().expect("in-memory db")
    }

    const LADDER: &[Migration] = &[
        Migration {
            version: 1,
            name: "create-t",
            sql: "CREATE TABLE t (id INTEGER PRIMARY KEY, a TEXT);",
        },
        Migration {
            version: 2,
            name: "add-b",
            sql: "ALTER TABLE t ADD COLUMN b TEXT;",
        },
    ];

    fn columns(conn: &Connection, table: &str) -> Vec<String> {
        let mut st = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("table_info");
        let rows = st.query_map([], |r| r.get::<_, String>(1)).expect("query");
        rows.map(|r| r.expect("col")).collect()
    }

    #[test]
    fn ensure_columns_adds_only_what_is_missing_and_is_idempotent() {
        let c = mem();
        c.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, a TEXT);")
            .unwrap();

        let cols: &[(&str, &str)] = &[
            ("a", "ALTER TABLE t ADD COLUMN a TEXT;"),
            ("b", "ALTER TABLE t ADD COLUMN b TEXT;"),
        ];
        let added = ensure_columns(&c, "t", cols).expect("first");
        assert_eq!(
            added,
            vec!["b".to_string()],
            "must skip the existing column"
        );

        let again = ensure_columns(&c, "t", cols).expect("second");
        assert!(again.is_empty(), "second run must add nothing");
        assert_eq!(columns(&c, "t"), vec!["id", "a", "b"]);
    }

    #[test]
    fn ensure_columns_reports_a_bad_statement_instead_of_swallowing_it() {
        // The whole point: `let _ = ALTER …` could not distinguish this from
        // "the column was already there".
        let c = mem();
        c.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY);")
            .unwrap();
        let err = ensure_columns(&c, "t", &[("x", "ALTER TABLE t ADD COLUMN x NOT_A_TYPE(;")])
            .expect_err("invalid DDL must surface");
        assert!(err.to_string().contains("adding column x"), "got: {err}");
    }

    #[test]
    fn ensure_columns_on_a_missing_table_is_not_an_error() {
        let c = mem();
        let added = ensure_columns(
            &c,
            "absent",
            &[("x", "ALTER TABLE absent ADD COLUMN x TEXT;")],
        )
        .expect("a fresh database has no table yet");
        assert!(added.is_empty());
    }

    #[test]
    fn destructive_sql_is_recognised_and_creation_is_not() {
        assert!(is_destructive("DROP TABLE nodes_fts;"));
        assert!(is_destructive("delete from tasks where id = 1"));
        assert!(is_destructive("ALTER TABLE t RENAME TO u"));
        assert!(is_destructive("ALTER TABLE t RENAME COLUMN a TO b"));
        // The four the first version missed.
        assert!(is_destructive("ALTER TABLE t DROP COLUMN secret;"));
        assert!(is_destructive("UPDATE tasks SET status = 'done';"));
        assert!(is_destructive("INSERT OR REPLACE INTO t VALUES (1);"));
        assert!(is_destructive("TRUNCATE t;"));

        // The shape every baseline rung has. If this were flagged, the first
        // run of a fresh install would copy every database it just created.
        assert!(!is_destructive(
            "CREATE TABLE IF NOT EXISTS t (id INTEGER PRIMARY KEY);
             CREATE INDEX IF NOT EXISTS i ON t(id);"
        ));
        assert!(!is_destructive("ALTER TABLE t ADD COLUMN b TEXT;"));
    }

    /// An `updated_at` trigger body contains an `UPDATE`, and every baseline
    /// rung in this tree defines one. Without the trigger-stripping, adding
    /// `UPDATE` to the destructive list would have made every first run copy
    /// every database it had just created.
    #[test]
    fn a_trigger_body_containing_update_does_not_make_a_baseline_destructive() {
        let baseline = "CREATE TABLE IF NOT EXISTS t (
                            id INTEGER PRIMARY KEY,
                            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
                        );
                        CREATE TRIGGER IF NOT EXISTS t_ts
                            AFTER UPDATE ON t
                            BEGIN
                                UPDATE t SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
                            END;";
        assert!(
            !is_destructive(baseline),
            "a baseline with an updated_at trigger must not be treated as destructive"
        );

        // But a real UPDATE outside a trigger still is.
        let real = format!("{baseline}\nUPDATE t SET updated_at = NULL;");
        assert!(
            is_destructive(&real),
            "an UPDATE outside a trigger is destructive"
        );
    }

    /// The refusal branch, which was previously untested — so "a failed backup
    /// stops the operation" was an unproven claim.
    #[test]
    fn a_destructive_migration_refuses_to_run_when_the_backup_fails() {
        let dir = std::env::temp_dir().join("autodev-migrate-refuse-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let db_path = dir.join("victim.db");

        let c = Connection::open(&db_path).expect("open");
        c.execute_batch(
            "CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT);
             INSERT INTO t (v) VALUES ('precious');
             PRAGMA user_version = 1;",
        )
        .expect("seed");

        // Crate-wide lock: `HOME` is process-global and these tests run as
        // threads. Without it this test and the refusal test below raced, and
        // one created a file where the other expected a directory.
        let _home = crate::db::test_support::with_home(&dir);

        // Make the backup impossible: put a DIRECTORY where the backup file
        // needs to be written, so `VACUUM INTO` cannot create it.
        let backups = dir.join(".autodev").join("backups");
        std::fs::create_dir_all(&backups).expect("mkdir backups");
        // `backup_before` names the file `<stem>-<timestamp>-<reason>.db`; the
        // timestamp is the clock, so block the whole directory instead by
        // making it read-only is unreliable on Windows. Occupy the exact name
        // is impossible without the clock — so remove write access by making
        // `backups` a FILE rather than a directory.
        std::fs::remove_dir_all(&backups).expect("rm");
        std::fs::write(&backups, b"not a directory").expect("write blocker");

        const D: &[Migration] = &[
            Migration {
                version: 1,
                name: "baseline",
                sql: "CREATE TABLE IF NOT EXISTS t (id INTEGER PRIMARY KEY, v TEXT);",
            },
            Migration {
                version: 2,
                name: "destroy",
                sql: "DELETE FROM t;",
            },
        ];
        let outcome = run_ladder(&c, "victim.db", D);

        let err = outcome.expect_err("a destructive migration must not run without a backup");
        assert!(
            err.to_string().contains("no backup could be taken"),
            "unexpected error: {err}"
        );

        // The whole point: the data is still there.
        let left: i64 = c
            .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 1, "the row was deleted despite the backup failing");
        assert_eq!(
            user_version(&c).unwrap(),
            1,
            "the version must not advance past a refused migration"
        );

        drop(c);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A destructive rung must not run when its backup cannot be taken.
    ///
    /// Proven with an in-memory database, which has no path — so
    /// `backup_before` cannot be reached and the guard's *other* branch is
    /// exercised in `a_destructive_migration_backs_up_first`.
    #[test]
    fn a_destructive_rung_on_a_pathless_database_still_applies() {
        // In-memory databases cannot be backed up and are never user data.
        // The guard must not deadlock the ladder for them.
        let c = mem();
        const D: &[Migration] = &[
            Migration {
                version: 1,
                name: "make",
                sql: "CREATE TABLE t (id INTEGER PRIMARY KEY);",
            },
            Migration {
                version: 2,
                name: "drop-it",
                sql: "DROP TABLE t;",
            },
        ];
        assert_eq!(run_ladder(&c, "mem.db", D).expect("ladder"), 2);
    }

    /// A destructive rung against a real file takes a backup before running,
    /// and the backup contains the data the migration is about to remove.
    #[test]
    fn a_destructive_migration_backs_up_first() {
        let dir = std::env::temp_dir().join("autodev-migrate-backup-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let db_path = dir.join("victim.db");

        let c = Connection::open(&db_path).expect("open");
        c.execute_batch(
            "CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT);
             INSERT INTO t (v) VALUES ('precious');
             PRAGMA user_version = 1;",
        )
        .expect("seed");

        // Point the backup root at the sandbox so the test never writes to
        // the user's real ~/.autodev/backups.
        // Crate-wide lock: `HOME` is process-global and these tests run as
        // threads. Without it this test and the refusal test below raced, and
        // one created a file where the other expected a directory.
        let _home = crate::db::test_support::with_home(&dir);

        const D: &[Migration] = &[
            Migration {
                version: 1,
                name: "baseline",
                sql: "CREATE TABLE IF NOT EXISTS t (id INTEGER PRIMARY KEY, v TEXT);",
            },
            Migration {
                version: 2,
                name: "destroy",
                sql: "DELETE FROM t;",
            },
        ];
        let reached = run_ladder(&c, "victim.db", D).expect("ladder");

        assert_eq!(reached, 2);
        let left: i64 = c
            .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0, "the migration did run");

        // The row it removed must be recoverable.
        let backups = dir.join(".autodev").join("backups");
        let found: Vec<_> = std::fs::read_dir(&backups)
            .expect("backup directory must exist")
            .flatten()
            .map(|e| e.path())
            .collect();
        assert_eq!(found.len(), 1, "expected exactly one backup, got {found:?}");

        let restored = Connection::open(&found[0]).expect("open backup");
        let saved: String = restored
            .query_row("SELECT v FROM t", [], |r| r.get(0))
            .expect("the backup must contain the row the migration deleted");
        assert_eq!(saved, "precious");

        drop(restored);
        drop(c);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_fresh_database_runs_the_whole_ladder() {
        let c = mem();
        assert_eq!(user_version(&c).unwrap(), 0);

        let reached = run_ladder(&c, "test.db", LADDER).expect("ladder");
        assert_eq!(reached, 2);
        assert_eq!(user_version(&c).unwrap(), 2);
        assert_eq!(columns(&c, "t"), vec!["id", "a", "b"]);
    }

    #[test]
    fn re_running_a_ladder_applies_nothing() {
        let c = mem();
        run_ladder(&c, "test.db", LADDER).expect("first");
        // v2 is `ALTER TABLE … ADD COLUMN`, which errors on a second run. If
        // the version check were wrong this would fail with "duplicate column".
        let reached = run_ladder(&c, "test.db", LADDER).expect("second must be a no-op");
        assert_eq!(reached, 2);
    }

    #[test]
    fn a_partially_migrated_database_resumes_from_its_version() {
        let c = mem();
        run_ladder(&c, "test.db", &LADDER[..1]).expect("v1 only");
        assert_eq!(user_version(&c).unwrap(), 1);
        assert_eq!(columns(&c, "t"), vec!["id", "a"]);

        run_ladder(&c, "test.db", LADDER).expect("resume");
        assert_eq!(user_version(&c).unwrap(), 2);
        assert_eq!(columns(&c, "t"), vec!["id", "a", "b"]);
    }

    /// The v5 failure mode, as a test.
    ///
    /// A rung whose SQL is invalid must leave the version untouched and report
    /// the error. The graph store's v5 did the opposite: it swallowed six
    /// failures, stamped the version and logged success.
    #[test]
    fn a_failing_migration_neither_stamps_nor_half_applies() {
        let c = mem();
        const BAD: &[Migration] = &[
            Migration {
                version: 1,
                name: "good",
                sql: "CREATE TABLE t (id INTEGER PRIMARY KEY);",
            },
            Migration {
                version: 2,
                name: "invalid-sqlite",
                // Valid in Postgres, not in SQLite — the exact statement that
                // silently failed six times in graph v5.
                sql: "CREATE TABLE u (id INTEGER); ALTER TABLE u ADD COLUMN IF NOT EXISTS x TEXT;",
            },
        ];

        let err = run_ladder(&c, "test.db", BAD).expect_err("v2 must fail");
        assert!(err.to_string().contains("invalid-sqlite"), "got: {err}");
        assert!(err.to_string().contains("rolled back"), "got: {err}");

        assert_eq!(
            user_version(&c).unwrap(),
            1,
            "a failed migration must not stamp its version"
        );
        let leftover: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='u'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            leftover, 0,
            "the successful half of a failed migration must be rolled back"
        );
    }

    #[test]
    fn a_database_from_a_newer_build_is_refused_rather_than_ignored() {
        let c = mem();
        run_ladder(&c, "test.db", LADDER).expect("ladder");
        c.execute_batch("PRAGMA user_version = 99").expect("bump");

        let err = run_ladder(&c, "test.db", LADDER).expect_err("must refuse");
        assert!(err.to_string().contains("99"), "got: {err}");
        assert!(err.to_string().contains("newer version"), "got: {err}");
    }

    #[test]
    fn a_non_consecutive_ladder_is_rejected_before_touching_the_database() {
        let c = mem();
        const GAPPED: &[Migration] = &[
            Migration {
                version: 1,
                name: "one",
                sql: "CREATE TABLE a (id INTEGER);",
            },
            Migration {
                version: 3,
                name: "three",
                sql: "CREATE TABLE b (id INTEGER);",
            },
        ];
        let err = run_ladder(&c, "test.db", GAPPED).expect_err("must reject");
        assert!(err.to_string().contains("consecutive"), "got: {err}");

        let tables: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 0, "validation must happen before any DDL runs");
    }
}
