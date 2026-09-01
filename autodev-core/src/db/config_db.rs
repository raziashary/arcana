// `~/.autodev/config.db` — the schema owner for the whole file.
//
// Three modules keep tables here: `app_settings`, `autodev_instructions` and
// `model_configs`. Their tables are disjoint, so for a long time each declared
// its own `CREATE TABLE IF NOT EXISTS` and nothing collided.
//
// A `user_version` does collide. It belongs to the *file*, not to a table, so
// three modules each running their own ladder over `config.db` would each see a
// version the other two had written and either skip their own work or re-run
// it. That is why this module exists: one connection, one ladder, one version
// for the file, and the three table modules open through it.
//
// See MDP `§7 P1-10`, `P1-15` and `INV-10`.

use rusqlite::Connection;

use crate::db::autodev_dir;
use crate::db::migrate::{self, Migration};
use crate::errors::{AutoDevError, Result};

/// Everything `config.db` contains.
///
/// One rung, because this is the first version that carries a stamp — the
/// tables themselves predate it and are created here idempotently, so an
/// existing `config.db` is stamped without being altered.
const LADDER: &[Migration] = &[Migration {
    version: 1,
    name: "config-baseline",
    sql: r#"
        CREATE TABLE IF NOT EXISTS app_settings (
            key        TEXT PRIMARY KEY,
            value      TEXT NOT NULL,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TRIGGER IF NOT EXISTS update_app_settings_ts
            AFTER UPDATE ON app_settings
            BEGIN
                UPDATE app_settings SET updated_at = CURRENT_TIMESTAMP WHERE key = NEW.key;
            END;

        CREATE TABLE IF NOT EXISTS autodev_instructions (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            scope       TEXT    NOT NULL CHECK(scope IN ('global', 'repo')),
            repo_path   TEXT,
            content     TEXT    NOT NULL DEFAULT '',
            is_active   INTEGER NOT NULL DEFAULT 1,
            created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT    NOT NULL DEFAULT (datetime('now')),
            CONSTRAINT repo_path_required CHECK (
                (scope = 'global' AND repo_path IS NULL) OR
                (scope = 'repo'   AND repo_path IS NOT NULL)
            ),
            UNIQUE(scope, repo_path)
        );
        CREATE INDEX IF NOT EXISTS idx_autodev_scope_repo
            ON autodev_instructions(scope, repo_path);
        CREATE TRIGGER IF NOT EXISTS autodev_instructions_updated_at
        AFTER UPDATE ON autodev_instructions
        BEGIN
            UPDATE autodev_instructions SET updated_at = datetime('now') WHERE id = NEW.id;
        END;

        CREATE TABLE IF NOT EXISTS model_configs (
            model_id          TEXT PRIMARY KEY,
            temperature       REAL,
            top_p             REAL,
            reasoning_effort  TEXT,
            thinking_enabled  INTEGER NOT NULL DEFAULT 0,
            thinking_budget   INTEGER,
            updated_at        DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TRIGGER IF NOT EXISTS update_model_configs_ts
            AFTER UPDATE ON model_configs
            BEGIN
                UPDATE model_configs SET updated_at = CURRENT_TIMESTAMP WHERE model_id = NEW.model_id;
            END;
    "#,
}];

/// The tables this file owns. Used by the one-owner gate and by tests.
pub const TABLES: &[&str] = &["app_settings", "autodev_instructions", "model_configs"];

/// Open `~/.autodev/config.db`, migrated and ready.
pub fn open() -> Result<Connection> {
    open_at(&autodev_dir()?.join("config.db"))
}

/// As `open`, against an explicit path, so tests never touch the user's real
/// `config.db`.
pub fn open_at(path: &std::path::Path) -> Result<Connection> {
    let conn =
        Connection::open(path).map_err(|e| AutoDevError::Agent(format!("config.db open: {e}")))?;
    // Per-connection, so it is set here rather than in a migration rung.
    conn.execute_batch("PRAGMA journal_mode = WAL;")
        .map_err(|e| AutoDevError::Agent(format!("config.db journal_mode: {e}")))?;
    migrate::run_ladder(&conn, "config.db", LADDER)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("autodev-configdb-test-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir.join("config.db")
    }

    fn table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
            [name],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
            == 1
    }

    #[test]
    fn one_open_creates_every_table_the_file_owns() {
        let path = temp("fresh");
        let conn = open_at(&path).expect("open");
        for t in TABLES {
            assert!(table_exists(&conn, t), "missing table {t}");
        }
        assert_eq!(migrate::user_version(&conn).unwrap(), 1);
        drop(conn);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// The reason this module exists.
    ///
    /// Three modules used to migrate this file independently. Whichever ran
    /// first decided the schema, and none of them could have owned the
    /// `user_version` without the other two fighting it. Opening twice must
    /// converge, not oscillate.
    #[test]
    fn reopening_is_stable_rather_than_re_migrating() {
        let path = temp("stable");
        let first = open_at(&path).expect("first");
        assert_eq!(migrate::user_version(&first).unwrap(), 1);
        drop(first);

        let second = open_at(&path).expect("second");
        assert_eq!(migrate::user_version(&second).unwrap(), 1);
        for t in TABLES {
            assert!(table_exists(&second, t), "missing table {t} after reopen");
        }
        drop(second);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// A `config.db` that predates the ladder — tables present, version 0 —
    /// is the shape every live installation is in. It must be stamped without
    /// losing its rows.
    #[test]
    fn a_pre_ladder_config_db_is_stamped_without_losing_data() {
        let path = temp("legacy");
        {
            let conn = Connection::open(&path).expect("create");
            conn.execute_batch(
                "CREATE TABLE app_settings (
                     key TEXT PRIMARY KEY, value TEXT NOT NULL,
                     updated_at DATETIME DEFAULT CURRENT_TIMESTAMP);
                 INSERT INTO app_settings (key, value) VALUES ('workspace_dir', 'C:/live');",
            )
            .expect("legacy schema");
            assert_eq!(migrate::user_version(&conn).unwrap(), 0);
        }

        let conn = open_at(&path).expect("migrate");
        assert_eq!(migrate::user_version(&conn).unwrap(), 1);
        let v: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='workspace_dir'",
                [],
                |r| r.get(0),
            )
            .expect("the existing row must survive");
        assert_eq!(v, "C:/live");
        for t in TABLES {
            assert!(table_exists(&conn, t), "missing table {t}");
        }
        drop(conn);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
