//! TG1-06 — two writers on one database file both succeed.
//!
//! This is the gate the restated `INV-1` makes necessary rather than optional:
//! `arcanactl` and the desktop app are two binaries linking one agent core, so
//! two processes contending for `~/.arcana/tasks.db` is the *normal* case, not
//! an edge one.
//!
//! **What these tests do and do not show, measured rather than assumed.** Each
//! was re-run with `open_tuned` forced to `journal_mode = DELETE`:
//!
//!   * `two_writers_on_one_file_both_commit` — passes under **both** modes.
//!     Concurrent *writers* are handled by `busy_timeout`, not by WAL, so this
//!     proves the contention is survivable but says nothing about the journal.
//!   * `a_reader_is_not_blocked_by_an_open_write_transaction` — also passes
//!     under both. A `BEGIN IMMEDIATE` writer holds a RESERVED lock, and
//!     readers may read through RESERVED even in rollback-journal mode; the
//!     blocking window is the EXCLUSIVE lock taken at COMMIT, which this test
//!     does not span.
//!   * `open_tuned_actually_puts_the_file_in_wal_mode` — the **only** one that
//!     fails under DELETE, and therefore the one that actually pins the
//!     pragma.
//!
//! An earlier version of this comment asserted that `delete` mode "blocks a
//! reader outright". That is wrong, and the falsification run is what showed
//! it. WAL's benefit here is narrower than claimed: it removes the COMMIT-time
//! exclusive window and lets a long read proceed against a writer. The reason
//! `analytics.db`, `annotations.db` and `workers.db` sitting in `delete` mode
//! mattered is consistency of behaviour across databases, not a reader that
//! could never run.
//!
//! Threads rather than processes: the contention is at the SQLite file-lock
//! level, which is shared across both, and a test that spawns processes cannot
//! run in this harness. The limitation is stated rather than hidden — what this
//! proves is that the pragmas are set and effective, not that a second OS
//! process was launched.

use std::sync::{Arc, Barrier};

use arcana_core::db::open_tuned;

fn sandbox(tag: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("arcana-concurrent-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).expect("create unique isolated test directory");
    dir.join("contended.db")
}

#[test]
fn two_writers_on_one_file_both_commit() {
    let path = sandbox("writers");
    {
        let setup = open_tuned(&path).expect("open");
        setup
            .execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, who TEXT NOT NULL);")
            .expect("schema");
    }

    const WRITERS: usize = 4;
    const PER_WRITER: usize = 25;

    // Every writer starts at the same instant, so the contention is real
    // rather than incidental.
    let barrier = Arc::new(Barrier::new(WRITERS));
    let mut handles = Vec::new();

    for w in 0..WRITERS {
        let path = path.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || -> Result<(), String> {
            let conn = open_tuned(&path).map_err(|e| format!("writer {w} open: {e}"))?;
            barrier.wait();
            for i in 0..PER_WRITER {
                conn.execute(
                    "INSERT INTO t (who) VALUES (?1)",
                    rusqlite::params![format!("writer-{w}-{i}")],
                )
                .map_err(|e| format!("writer {w} insert {i}: {e}"))?;
            }
            Ok(())
        }));
    }

    let mut failures = Vec::new();
    for h in handles {
        match h.join().expect("thread panicked") {
            Ok(()) => {}
            Err(e) => failures.push(e),
        }
    }

    assert!(
        failures.is_empty(),
        "a contended write failed instead of waiting — check busy_timeout and \
         journal_mode: {failures:?}"
    );

    // Every row must actually be there. A writer that silently wrote nothing
    // would satisfy the assertion above.
    let conn = open_tuned(&path).expect("reopen");
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
        .expect("count");
    assert_eq!(
        n,
        (WRITERS * PER_WRITER) as i64,
        "rows were lost under contention"
    );

    drop(conn);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn a_reader_is_not_blocked_by_an_open_write_transaction() {
    // Note: this passes under `delete` mode too — see the module header. It is
    // kept because it pins real behaviour (a reader sees committed data and not
    // uncommitted data during a write transaction), not because it
    // discriminates between journal modes.
    let path = sandbox("reader");
    {
        let setup = open_tuned(&path).expect("open");
        setup
            .execute_batch(
                "CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT);
                 INSERT INTO t (v) VALUES ('committed');",
            )
            .expect("schema");
    }

    let writer = open_tuned(&path).expect("writer");
    writer.execute_batch("BEGIN IMMEDIATE").expect("begin");
    writer
        .execute("INSERT INTO t (v) VALUES ('uncommitted')", [])
        .expect("insert");

    // With the write transaction still open, a reader must still see the
    // committed row rather than erroring with "database is locked".
    let reader = open_tuned(&path).expect("reader");
    let seen: String = reader
        .query_row("SELECT v FROM t WHERE v = 'committed'", [], |r| r.get(0))
        .expect("a reader must not be blocked by an open write transaction");
    assert_eq!(seen, "committed");

    // And it must not see the uncommitted row.
    let uncommitted: i64 = reader
        .query_row("SELECT COUNT(*) FROM t WHERE v = 'uncommitted'", [], |r| {
            r.get(0)
        })
        .expect("count");
    assert_eq!(uncommitted, 0, "a reader saw an uncommitted write");

    writer.execute_batch("COMMIT").expect("commit");

    drop(reader);
    drop(writer);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn open_tuned_actually_puts_the_file_in_wal_mode() {
    // The discriminating assertion. Forcing `open_tuned` to DELETE fails this
    // test and only this test — verified by doing it.
    let path = sandbox("mode");
    let conn = open_tuned(&path).expect("open");
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .expect("journal_mode");
    assert_eq!(mode.to_lowercase(), "wal");
    drop(conn);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
