// SQLite backup helper.
//
// Every destructive database operation goes through this first (GAP-11, §0.8).
//
// Why this exists at all: before P1 there was no SQLite backup capability
// anywhere in the tree — no `VACUUM INTO`, no `Connection::backup`, and
// rusqlite's `backup` feature was not enabled. Two helpers looked like backups
// and were not: `backup_recipe_file` is YAML-only, and `create_restore_point`
// writes a size/mtime manifest that nothing ever reads.
//
// Why `VACUUM INTO` and not `fs::copy`:
//
//   A SQLite database in WAL mode keeps recent commits in a `-wal` sidecar
//   until a checkpoint folds them back. Copying only the `.db` file silently
//   discards everything committed since the last checkpoint. That is not
//   theoretical here — `~/.airs/autodev.db` is a 30 May file carrying a **4.1 MB
//   uncheckpointed WAL from 1 June**, so a copy-based helper would have backed
//   up the older data and reported success.
//
//   `VACUUM INTO` asks SQLite itself for a consistent snapshot: it includes
//   uncheckpointed WAL content, it does not mutate the source (unlike
//   `wal_checkpoint(TRUNCATE)`), and the result is a compact, fully-formed
//   database rather than a byte image that may be mid-write.
//
// See ADR-LOG-RESTRUCT-0014.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::errors::{AutoDevError, Result};

/// Where backups are kept: `~/.autodev/backups/`.
///
/// Deliberately inside the data root rather than a temp directory — a backup
/// that a cleaner can delete is not a backup.
pub fn backup_root() -> Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| AutoDevError::Agent("No home directory to store backups in".into()))?;
    let dir = PathBuf::from(home).join(".autodev").join("backups");
    std::fs::create_dir_all(&dir)
        .map_err(|e| AutoDevError::Agent(format!("Cannot create {}: {e}", dir.display())))?;
    Ok(dir)
}

/// A completed backup. Returned so callers can name it in an error message or
/// a log line — a backup nobody can find is not much better than none.
#[derive(Debug, Clone)]
pub struct Backup {
    /// Absolute path of the snapshot.
    pub path: PathBuf,
    /// The database it was taken from.
    pub source: PathBuf,
    /// Why it was taken, e.g. `"graph-migration-v6"`.
    pub reason: String,
    pub bytes: u64,
}

impl Backup {
    pub fn describe(&self) -> String {
        format!(
            "{} ({} bytes) backed up from {} before {}",
            self.path.display(),
            self.bytes,
            self.source.display(),
            self.reason
        )
    }
}

/// Snapshot `db_path` before a destructive operation.
///
/// `reason` becomes part of the filename so a directory of backups is
/// self-describing. `timestamp` is supplied by the caller rather than read from
/// the clock, so the name is deterministic in tests.
///
/// Fails loudly. A destructive operation must not proceed on the belief that a
/// backup happened — that is the whole point of the helper, and it is why this
/// returns `Result` rather than logging and carrying on the way
/// `auto_checkpoint_before_write` does.
pub fn backup_database(db_path: &Path, reason: &str, timestamp: &str) -> Result<Backup> {
    backup_database_in(&backup_root()?, db_path, reason, timestamp)
}

/// As `backup_database`, with the destination directory supplied.
///
/// Exists so tests never write into the user's real `~/.autodev/backups/`.
/// The first version of this module did, and a failing test leaked a snapshot
/// that then made every subsequent run fail with "Backup already exists" —
/// an order-dependent suite that also touched live user data.
pub fn backup_database_in(
    root: &Path,
    db_path: &Path,
    reason: &str,
    timestamp: &str,
) -> Result<Backup> {
    if !db_path.exists() {
        return Err(AutoDevError::Agent(format!(
            "Refusing to back up a database that does not exist: {}",
            db_path.display()
        )));
    }

    let stem = db_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("database");
    let safe_reason: String = reason
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();

    std::fs::create_dir_all(root)
        .map_err(|e| AutoDevError::Agent(format!("Cannot create {}: {e}", root.display())))?;
    let dest = root.join(format!("{stem}-{timestamp}-{safe_reason}.db"));
    if dest.exists() {
        return Err(AutoDevError::Agent(format!(
            "Backup already exists, refusing to overwrite: {}",
            dest.display()
        )));
    }

    // Read-only on the source: VACUUM INTO neither checkpoints nor writes to it.
    let conn = Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| AutoDevError::Agent(format!("Cannot open {}: {e}", db_path.display())))?;

    // `VACUUM INTO` takes a string literal, not a bound parameter. The path is
    // ours (backup_root + a sanitised stem), never user input, but the quotes
    // are escaped anyway so a home directory containing one cannot break out.
    let escaped = dest.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{escaped}'"))
        .map_err(|e| {
            AutoDevError::Agent(format!(
                "VACUUM INTO failed for {} -> {}: {e}",
                db_path.display(),
                dest.display()
            ))
        })?;

    let bytes = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    if bytes == 0 {
        return Err(AutoDevError::Agent(format!(
            "Backup of {} produced an empty file — treating as failure",
            db_path.display()
        )));
    }

    // Prove the snapshot is a readable database before telling the caller it is
    // safe to proceed. An unopenable backup that reports success is worse than
    // no backup, because it is acted upon.
    {
        let check = Connection::open_with_flags(&dest, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| AutoDevError::Agent(format!("Backup is not readable: {e}")))?;
        let ok: String = check
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(|e| AutoDevError::Agent(format!("Backup integrity check failed: {e}")))?;
        if ok != "ok" {
            return Err(AutoDevError::Agent(format!(
                "Backup of {} failed its integrity check: {ok}",
                db_path.display()
            )));
        }
    }

    let backup = Backup {
        path: dest,
        source: db_path.to_path_buf(),
        reason: reason.to_string(),
        bytes,
    };
    tracing::info!("[backup] {}", backup.describe());
    Ok(backup)
}

/// Timestamp for a backup filename: `YYYYMMDD-HHMMSS`.
pub fn timestamp_now() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// Convenience for call sites that have no reason to control the clock.
pub fn backup_before(db_path: &Path, reason: &str) -> Result<Backup> {
    backup_database(db_path, reason, &timestamp_now())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("autodev-backup-test-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    fn seed(path: &Path, rows: i32) -> Connection {
        let conn = Connection::open(path).expect("open");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS t (id INTEGER PRIMARY KEY, v TEXT NOT NULL);",
        )
        .expect("create");
        for i in 0..rows {
            conn.execute("INSERT INTO t (v) VALUES (?1)", [format!("row-{i}")])
                .expect("insert");
        }
        conn
    }

    fn count(path: &Path) -> i64 {
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("open backup");
        conn.query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .expect("count")
    }

    #[test]
    fn a_backup_contains_the_rows_of_its_source() {
        let dir = temp_dir("basic");
        let db = dir.join("source.db");
        let conn = seed(&db, 7);
        drop(conn);

        let b = backup_database_in(&dir, &db, "unit-test", "20260101-000000").expect("backup");
        assert!(b.path.exists());
        assert_eq!(count(&b.path), 7);
        assert_eq!(count(&db), 7, "the source must be untouched");

        let _ = std::fs::remove_file(&b.path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The regression this helper exists for.
    ///
    /// In WAL mode, commits live in a `-wal` sidecar until a checkpoint folds
    /// them in. A backup built on `fs::copy` of the `.db` would capture the
    /// pre-WAL state and report success — which is exactly what would have
    /// happened to `~/.airs/autodev.db` and its 4.1 MB uncheckpointed WAL.
    #[test]
    fn a_backup_captures_uncheckpointed_wal_content() {
        let dir = temp_dir("wal");
        let db = dir.join("wal.db");

        let conn = Connection::open(&db).expect("open");
        conn.pragma_update(None, "journal_mode", "WAL")
            .expect("wal");
        conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT NOT NULL);")
            .expect("create");
        for i in 0..50 {
            conn.execute("INSERT INTO t (v) VALUES (?1)", [format!("wal-{i}")])
                .expect("insert");
        }
        // Deliberately NOT checkpointing, and deliberately holding the
        // connection open — this is the state a naive copy gets wrong.
        let wal = db.with_extension("db-wal");
        assert!(
            wal.exists(),
            "expected a -wal sidecar to exist for this test to mean anything"
        );

        let b = backup_database_in(&dir, &db, "wal-test", "20260101-000001").expect("backup");
        assert_eq!(
            count(&b.path),
            50,
            "the backup must contain rows that live only in the uncheckpointed WAL"
        );

        drop(conn);
        let _ = std::fs::remove_file(&b.path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn backing_up_a_missing_database_is_an_error_not_a_silent_skip() {
        let dir = temp_dir("missing");
        let err = backup_database_in(&dir, &dir.join("nope.db"), "x", "20260101-000002")
            .expect_err("must fail");
        assert!(
            err.to_string().contains("does not exist"),
            "unexpected error: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_existing_backup_is_never_overwritten() {
        let dir = temp_dir("dup");
        let db = dir.join("dup.db");
        drop(seed(&db, 2));

        let first = backup_database_in(&dir, &db, "dup", "20260101-000003").expect("first");
        let err =
            backup_database_in(&dir, &db, "dup", "20260101-000003").expect_err("second must fail");
        assert!(err.to_string().contains("already exists"), "got: {err}");

        let _ = std::fs::remove_file(&first.path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_reason_is_sanitised_into_the_filename() {
        let dir = temp_dir("name");
        let db = dir.join("n.db");
        drop(seed(&db, 1));

        let b =
            backup_database_in(&dir, &db, "graph migration/v6", "20260101-000004").expect("backup");
        let name = b.path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.contains("graph-migration-v6"), "got {name}");
        assert!(
            !name.contains('/') && !name.contains(' '),
            "unsafe filename: {name}"
        );

        let _ = std::fs::remove_file(&b.path);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
