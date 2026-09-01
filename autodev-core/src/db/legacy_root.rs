// Moving off the old `~/.airs` data root onto `~/.autodev`.
//
// Two roots existed. `~/.autodev` holds nearly everything — tasks, workers,
// config, memory, analytics, annotations, graphs — while `~/.airs` held only
// the session store, `autodev.db`. Code also *referred* to `~/.airs/plugins/`
// and `~/.airs/fleet_secret`, but neither has ever existed on disk: they are
// code-only paths, so there is nothing to move for them.
//
// **The migration copies the file; it does not re-create the schema.** That
// distinction is the whole design. `messages` on disk carries `tool_calls` and
// `tool_results`, two columns the current `CREATE TABLE messages` no longer
// declares — so a migration that built the destination from the source code and
// then `INSERT … SELECT`-ed into it would silently drop them. Copying the
// database preserves every column, every row and every index by construction,
// whatever the schema happens to be, and whatever a future version adds.
//
// `VACUUM INTO` is used rather than `fs::copy` for the reason set out in
// `db/backup.rs`: in WAL mode, recent commits live in a `-wal` sidecar until a
// checkpoint folds them in, and copying only the `.db` discards them. `VACUUM
// INTO` asks SQLite for a consistent snapshot that includes uncheckpointed WAL
// content, and does not mutate the source.
//
// The source is left in place. Deleting a user's only copy of their session
// history immediately after moving it is not a migration, it is a gamble; the
// old root is removed in a later phase once the new one has been running.
//
// See MDP `§7 P1-14`.

use std::path::{Path, PathBuf};

use crate::errors::{AutoDevError, Result};

/// The retired data root, `~/.airs`.
///
/// Deliberately does **not** create the directory. `src-shared`'s
/// `config_path()` used to `create_dir_all` this on every call, which meant the
/// old root could not be retired: anything that removed it would find it
/// recreated by the next CLI invocation.
pub fn legacy_dir() -> Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| AutoDevError::Agent("Cannot determine home directory".into()))?;
    Ok(PathBuf::from(home).join(".airs"))
}

/// What a migration attempt did, so a caller can log it honestly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Migrated {
    /// Copied from the legacy root to the new one.
    Moved { from: PathBuf, to: PathBuf },
    /// The destination already existed; the legacy file was left untouched.
    AlreadyPresent,
    /// There was nothing at the legacy path.
    NothingToDo,
}

/// Copy `name` from `~/.airs` to `dest` if, and only if, the destination does
/// not already exist.
///
/// Never overwrites. A destination that already exists is the newer, live
/// database — the legacy file is by definition older, and copying over it would
/// destroy exactly the data the user has been creating since the move.
pub fn migrate_sqlite_once(name: &str, dest: &Path) -> Result<Migrated> {
    let src = legacy_dir()?.join(name);

    if !src.exists() {
        return Ok(Migrated::NothingToDo);
    }
    if dest.exists() {
        return Ok(Migrated::AlreadyPresent);
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AutoDevError::Agent(format!("cannot create {}: {e}", parent.display())))?;
    }

    // Read-only on the source: `VACUUM INTO` neither checkpoints nor writes.
    let conn = rusqlite::Connection::open_with_flags(
        &src,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| AutoDevError::Agent(format!("cannot open {}: {e}", src.display())))?;

    let escaped = dest.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{escaped}'"))
        .map_err(|e| {
            AutoDevError::Agent(format!(
                "migrating {} -> {} failed: {e}",
                src.display(),
                dest.display()
            ))
        })?;

    // Prove the copy is a readable database before reporting success. A
    // migration that reports success over an unopenable file is worse than one
    // that fails, because it is acted upon.
    {
        let check =
            rusqlite::Connection::open_with_flags(dest, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|e| AutoDevError::Agent(format!("migrated file is not readable: {e}")))?;
        let ok: String = check
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(|e| AutoDevError::Agent(format!("migrated file failed its check: {e}")))?;
        if ok != "ok" {
            // Remove the bad copy so the next start retries rather than
            // adopting it.
            let _ = std::fs::remove_file(dest);
            return Err(AutoDevError::Agent(format!(
                "migrated {} failed its integrity check: {ok}",
                src.display()
            )));
        }
    }

    tracing::info!(
        "[data-root] migrated {} -> {} (the original is left in place)",
        src.display(),
        dest.display()
    );
    Ok(Migrated::Moved {
        from: src,
        to: dest.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn sandbox(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("autodev-legacyroot-test-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".airs")).expect("mkdir airs");
        std::fs::create_dir_all(dir.join(".autodev")).expect("mkdir autodev");
        dir
    }

    /// Build a database shaped like the live one: `messages` carrying the two
    /// legacy columns the current source no longer declares.
    fn seed_legacy(path: &Path, rows: i32) {
        let c = Connection::open(path).expect("open");
        c.execute_batch(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, title TEXT NOT NULL);
             CREATE TABLE messages (
                 id TEXT PRIMARY KEY, session_id TEXT NOT NULL, role TEXT NOT NULL,
                 content TEXT NOT NULL, tool_calls TEXT, tool_results TEXT);
             INSERT INTO sessions (id, title) VALUES ('s1', 'New Session');",
        )
        .expect("schema");
        for i in 0..rows {
            c.execute(
                "INSERT INTO messages (id, session_id, role, content, tool_calls, tool_results)
                 VALUES (?1, 's1', 'user', ?2, '[]', '[]')",
                rusqlite::params![format!("m{i}"), format!("message {i}")],
            )
            .expect("insert");
        }
    }

    /// Point `~` at `dir` for the duration.
    ///
    /// Uses the **crate-wide** lock in `db::test_support`. This module had its
    /// own private mutex, which serialised its own tests and nothing else — so
    /// it did not protect against `db::migrate`'s tests swapping `HOME` at the
    /// same time. A per-module lock on a process-global is not a lock.
    fn with_home<T>(dir: &Path, f: impl FnOnce() -> T) -> T {
        let _guard = crate::db::test_support::with_home(dir);
        f()
    }

    /// The property the whole design rests on: columns the current source does
    /// not declare must survive, because the file is copied rather than rebuilt.
    #[test]
    fn legacy_columns_the_current_schema_omits_are_preserved() {
        let dir = sandbox("columns");
        seed_legacy(&dir.join(".airs").join("autodev.db"), 14);
        let dest = dir.join(".autodev").join("autodev.db");

        let outcome =
            with_home(&dir, || migrate_sqlite_once("autodev.db", &dest)).expect("migrate");
        assert!(matches!(outcome, Migrated::Moved { .. }), "{outcome:?}");

        let c = Connection::open(&dest).expect("open migrated");
        let cols: Vec<String> = {
            let mut st = c.prepare("PRAGMA table_info(messages)").unwrap();
            let r = st.query_map([], |r| r.get::<_, String>(1)).unwrap();
            r.map(|x| x.unwrap()).collect()
        };
        for legacy in ["tool_calls", "tool_results"] {
            assert!(
                cols.iter().any(|c| c == legacy),
                "{legacy} was dropped; the migration rebuilt the schema instead of copying it"
            );
        }
        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 14, "every message must survive");

        drop(c);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_existing_destination_is_never_overwritten() {
        let dir = sandbox("nooverwrite");
        seed_legacy(&dir.join(".airs").join("autodev.db"), 3);
        let dest = dir.join(".autodev").join("autodev.db");
        seed_legacy(&dest, 99); // the live database, with more in it

        let outcome =
            with_home(&dir, || migrate_sqlite_once("autodev.db", &dest)).expect("migrate");
        assert_eq!(outcome, Migrated::AlreadyPresent);

        let c = Connection::open(&dest).expect("open");
        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            n, 99,
            "the newer database must not be clobbered by the older one"
        );

        drop(c);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_original_is_left_in_place() {
        let dir = sandbox("keepsource");
        let src = dir.join(".airs").join("autodev.db");
        seed_legacy(&src, 5);
        let dest = dir.join(".autodev").join("autodev.db");

        with_home(&dir, || migrate_sqlite_once("autodev.db", &dest)).expect("migrate");
        assert!(
            src.exists(),
            "the legacy file must survive the move; deleting a user's only copy \
             immediately is not a migration"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nothing_to_do_when_there_is_no_legacy_file() {
        let dir = sandbox("empty");
        let dest = dir.join(".autodev").join("autodev.db");
        let outcome =
            with_home(&dir, || migrate_sqlite_once("autodev.db", &dest)).expect("migrate");
        assert_eq!(outcome, Migrated::NothingToDo);
        assert!(!dest.exists(), "nothing should have been created");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `legacy_dir` must not create the directory it names, or the old root can
    /// never be retired.
    #[test]
    fn legacy_dir_does_not_create_the_directory() {
        let dir = sandbox("nocreate");
        let airs = dir.join(".airs");
        std::fs::remove_dir_all(&airs).expect("remove");

        let resolved = with_home(&dir, || legacy_dir()).expect("legacy_dir");
        assert_eq!(resolved, airs);
        assert!(
            !airs.exists(),
            "legacy_dir recreated the retired root — this is exactly what \
             src-shared's config_path() did, and why the old root kept coming back"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
