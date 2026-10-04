// `~/.arcana/config.db` — the schema owner for the whole file.
//
// Modules keep tables here: `app_settings`, `arcana_instructions`,
// `model_configs`, `git/checkpoint.rs` (`checkpoints`), `hooks/db.rs`
// (`hooks`, `hook_logs`), and workspace egress consent/audit. Their tables are disjoint, so for a long time each
// declared its own `CREATE TABLE IF NOT EXISTS` and nothing collided.
//
// ⚠ P1-15 originally said "three". That was wrong, and the error is instructive:
// I searched for `CREATE TABLE … app_settings` — the *claim* — instead of for
// `config.db` — the *file*. The two extra writers predate P1, so this was never
// a regression, but the ladder below was declared the file's schema owner while
// three of its six tables sat outside it, and `INV-10` was scored GREEN on that
// count. Found by the post-phase audit (D-27 / W-07 / A-??), corrected in v2.
//
// A `user_version` does collide. It belongs to the *file*, not to a table, so
// three modules each running their own ladder over `config.db` would each see a
// version the other two had written and either skip their own work or re-run
// it. That is why this module exists: one connection, one ladder, one version
// for the file, and the three table modules open through it.
//
// See MDP `§7 P1-10`, `P1-15` and `INV-10`.

use rusqlite::Connection;

use crate::db::arcana_dir;
use crate::db::migrate::{self, Migration};
use crate::errors::{ArcanaError, Result};

/// Everything `config.db` contains.
///
/// The ladder adopts the pre-existing tables, then adds new consent/audit
/// tables through a separate versioned rung.
const LADDER: &[Migration] = &[
    Migration {
        version: 1,
        name: "config-baseline",
        sql: r#"
        CREATE TABLE IF NOT EXISTS app_settings (
            key        TEXT PRIMARY KEY,
            value      TEXT NOT NULL,
            updated_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE TRIGGER IF NOT EXISTS update_app_settings_ts
            AFTER UPDATE ON app_settings
            BEGIN
                UPDATE app_settings SET updated_at = CURRENT_TIMESTAMP WHERE key = NEW.key;
            END;

        CREATE TABLE IF NOT EXISTS arcana_instructions (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            scope       TEXT    NOT NULL CHECK(scope IN ('global', 'repo')),
            repo_path   TEXT,
            content     TEXT    NOT NULL DEFAULT '',
            is_active   INTEGER NOT NULL DEFAULT 1,
            created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            updated_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            CONSTRAINT repo_path_required CHECK (
                (scope = 'global' AND repo_path IS NULL) OR
                (scope = 'repo'   AND repo_path IS NOT NULL)
            ),
            UNIQUE(scope, repo_path)
        );
        CREATE INDEX IF NOT EXISTS idx_arcana_scope_repo
            ON arcana_instructions(scope, repo_path);
        CREATE TRIGGER IF NOT EXISTS arcana_instructions_updated_at
        AFTER UPDATE ON arcana_instructions
        BEGIN
            UPDATE arcana_instructions SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = NEW.id;
        END;

        CREATE TABLE IF NOT EXISTS model_configs (
            model_id          TEXT PRIMARY KEY,
            temperature       REAL,
            top_p             REAL,
            reasoning_effort  TEXT,
            thinking_enabled  INTEGER NOT NULL DEFAULT 0,
            thinking_budget   INTEGER,
            updated_at        TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE TRIGGER IF NOT EXISTS update_model_configs_ts
            AFTER UPDATE ON model_configs
            BEGIN
                UPDATE model_configs SET updated_at = CURRENT_TIMESTAMP WHERE model_id = NEW.model_id;
            END;
    "#,
    },
    Migration {
        version: 2,
        name: "config-adopt-strays",
        sql: V2_ADOPT_STRAYS,
    },
    Migration {
        version: 3,
        name: "workspace-egress-consent",
        sql: r#"
        CREATE TABLE IF NOT EXISTS workspace_egress_consents (
            workspace_path TEXT NOT NULL,
            destination   TEXT NOT NULL,
            granted_at    TEXT NOT NULL,
            revoked_at    TEXT,
            PRIMARY KEY (workspace_path, destination)
        );
        CREATE TABLE IF NOT EXISTS workspace_egress_audit (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            workspace_path TEXT NOT NULL,
            destination   TEXT NOT NULL,
            operation     TEXT NOT NULL,
            outcome       TEXT NOT NULL,
            occurred_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_workspace_egress_audit_scope
            ON workspace_egress_audit(workspace_path, destination, occurred_at);
    "#,
    },
    Migration {
        version: 4,
        name: "checkpoint-task-association",
        sql: r#"
        ALTER TABLE checkpoints ADD COLUMN task_id INTEGER;
        CREATE INDEX idx_checkpoints_task ON checkpoints(task_id);
    "#,
    },
    Migration {
        version: 5,
        name: "checkpoint-creation-rollback-marker",
        sql: r#"
        ALTER TABLE checkpoints ADD COLUMN file_existed INTEGER NOT NULL DEFAULT 1
            CHECK (file_existed IN (0, 1));
    "#,
    },
    Migration {
        version: 6,
        name: "canonical-utc-timestamp-writers",
        sql: r#"
        DROP TRIGGER IF EXISTS update_app_settings_ts;
        CREATE TRIGGER update_app_settings_ts AFTER UPDATE ON app_settings
        BEGIN UPDATE app_settings SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE key = NEW.key; END;
        DROP TRIGGER IF EXISTS arcana_instructions_updated_at;
        CREATE TRIGGER arcana_instructions_updated_at AFTER UPDATE ON arcana_instructions
        BEGIN UPDATE arcana_instructions SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = NEW.id; END;
        DROP TRIGGER IF EXISTS update_model_configs_ts;
        CREATE TRIGGER update_model_configs_ts AFTER UPDATE ON model_configs
        BEGIN UPDATE model_configs SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE model_id = NEW.model_id; END;
        CREATE TRIGGER IF NOT EXISTS config_app_settings_timestamp_default AFTER INSERT ON app_settings
        WHEN NEW.updated_at = datetime('now')
        BEGIN UPDATE app_settings SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE key = NEW.key; END;
        CREATE TRIGGER IF NOT EXISTS config_instructions_timestamp_default AFTER INSERT ON arcana_instructions
        WHEN NEW.created_at = datetime('now') OR NEW.updated_at = datetime('now')
        BEGIN UPDATE arcana_instructions SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = NEW.id; END;
        CREATE TRIGGER IF NOT EXISTS config_model_timestamp_default AFTER INSERT ON model_configs
        WHEN NEW.updated_at = datetime('now')
        BEGIN UPDATE model_configs SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE model_id = NEW.model_id; END;
        CREATE TRIGGER IF NOT EXISTS config_egress_audit_timestamp_default AFTER INSERT ON workspace_egress_audit
        WHEN NEW.occurred_at = datetime('now')
        BEGIN UPDATE workspace_egress_audit SET occurred_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = NEW.id; END;
        CREATE TRIGGER IF NOT EXISTS config_hooks_timestamp_default AFTER INSERT ON hooks
        WHEN NEW.created_at = datetime('now') OR NEW.updated_at = datetime('now')
        BEGIN UPDATE hooks SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = NEW.id; END;
        CREATE TRIGGER IF NOT EXISTS config_hook_logs_timestamp_default AFTER INSERT ON hook_logs
        WHEN NEW.executed_at = datetime('now')
        BEGIN UPDATE hook_logs SET executed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = NEW.id; END;
        "#,
    },
    Migration {
        version: 7,
        name: "atomic-daily-budget-reservations",
        sql: r#"
        CREATE TABLE IF NOT EXISTS daily_budget_totals (
            local_date TEXT PRIMARY KEY,
            spent_micros INTEGER NOT NULL CHECK (spent_micros >= 0)
        );
        CREATE TABLE IF NOT EXISTS daily_budget_reservations (
            id TEXT PRIMARY KEY,
            local_date TEXT NOT NULL,
            amount_micros INTEGER NOT NULL CHECK (amount_micros >= 0),
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_daily_budget_reservations_date
            ON daily_budget_reservations(local_date);
    "#,
    },
];

/// v2 — adopt the tables that `git/checkpoint.rs` and `hooks/db.rs` used to
/// create for themselves.
///
/// A separate rung rather than an edit to v1: every live `config.db` is already
/// stamped `user_version = 1`, so a v1 edit would never run. The statements are
/// `IF NOT EXISTS` and the tables already exist, so on every database that
/// exists today this rung creates nothing and only advances the version — which
/// is the point. From here on the file's `user_version` describes the whole
/// file, which is what `INV-10` actually asks for.
const V2_ADOPT_STRAYS: &str = r#"
    CREATE TABLE IF NOT EXISTS checkpoints (
        id             TEXT PRIMARY KEY,
        session_id     TEXT NOT NULL,
        file_path      TEXT NOT NULL,
        snapshot_path  TEXT NOT NULL,
        created_ms     INTEGER NOT NULL,
        reason         TEXT NOT NULL,
        size_bytes     INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_checkpoints_file    ON checkpoints(file_path);
    CREATE INDEX IF NOT EXISTS idx_checkpoints_session ON checkpoints(session_id);

    CREATE TABLE IF NOT EXISTS hooks (
        id          TEXT PRIMARY KEY,
        name        TEXT NOT NULL,
        hook_type   TEXT NOT NULL,
        enabled     INTEGER NOT NULL DEFAULT 1,
        priority    INTEGER NOT NULL DEFAULT 100,
        conditions  TEXT NOT NULL DEFAULT '{}',
        action      TEXT NOT NULL,
        created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    CREATE TABLE IF NOT EXISTS hook_logs (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        hook_id     TEXT NOT NULL,
        hook_name   TEXT NOT NULL,
        hook_type   TEXT NOT NULL,
        outcome     TEXT NOT NULL,
        output      TEXT,
        error       TEXT,
        executed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        duration_ms INTEGER NOT NULL DEFAULT 0
    );
"#;

/// Every table this file owns. Used by the one-owner gate and by tests.
pub const TABLES: &[&str] = &[
    "app_settings",
    "arcana_instructions",
    "model_configs",
    "checkpoints",
    "hooks",
    "hook_logs",
    "workspace_egress_consents",
    "workspace_egress_audit",
    "daily_budget_totals",
    "daily_budget_reservations",
];

/// Open `~/.arcana/config.db`, migrated and ready.
pub fn open() -> Result<Connection> {
    open_at(&arcana_dir()?.join("config.db"))
}

/// As `open`, against an explicit path, so tests never touch the user's real
/// `config.db`.
pub fn open_at(path: &std::path::Path) -> Result<Connection> {
    let conn = crate::db::open_tuned(path)?;
    // Per-connection, so it is set here rather than in a migration rung.
    conn.execute_batch("PRAGMA journal_mode = WAL;")
        .map_err(|e| ArcanaError::Agent(format!("config.db journal_mode: {e}")))?;
    migrate::run_ladder(&conn, "config.db", LADDER)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "arcana-configdb-test-{tag}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&dir).expect("create unique isolated directory");
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
    fn populated_pre_retention_config_upgrade_preserves_hook_logs_for_the_reader() {
        let directory = tempfile::tempdir().expect("owned legacy config profile");
        let path = directory.path().join("config.db");
        let conn = Connection::open(&path).expect("legacy config file");
        migrate::run_ladder(&conn, "config.db", &LADDER[..2])
            .expect("production pre-retention config schema");
        conn.execute(
            "INSERT INTO hook_logs (hook_id,hook_name,hook_type,outcome,output,error,executed_at,duration_ms)
             VALUES ('legacy-hook','legacy fixture','pre_tool_use','success','fixture output',NULL,'2026-07-06 12:00:00',27)",
            [],
        ).expect("populate historical hook log");
        drop(conn);
        let db = crate::hooks::db::HooksDb::open_at(&path)
            .expect("production hook opener migrates config");
        let rows = db.get_logs(None, 10).expect("production hook log reader");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].hook_id, "legacy-hook");
        assert_eq!(rows[0].output.as_deref(), Some("fixture output"));
        assert!(rows[0].error.is_none());
        assert_eq!(rows[0].executed_at, "2026-07-06 12:00:00");
        assert_eq!(rows[0].duration_ms, 27);
        let exported = db
            .get_logs_since("2026-07-06T00:00:00Z")
            .expect("production retained export reader");
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].id, rows[0].id);
    }

    #[test]
    fn one_open_creates_every_table_the_file_owns() {
        let path = temp("fresh");
        let _home = crate::db::test_support::with_home(path.parent().unwrap());
        let conn = open_at(&path).expect("open");
        for t in TABLES {
            assert!(table_exists(&conn, t), "missing table {t}");
        }
        assert_eq!(migrate::user_version(&conn).unwrap(), 7);
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
        let _home = crate::db::test_support::with_home(path.parent().unwrap());
        let first = open_at(&path).expect("first");
        assert_eq!(migrate::user_version(&first).unwrap(), 7);
        drop(first);

        let second = open_at(&path).expect("second");
        assert_eq!(migrate::user_version(&second).unwrap(), 7);
        for t in TABLES {
            assert!(table_exists(&second, t), "missing table {t} after reopen");
        }
        drop(second);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn checkpoint_task_association_survives_database_reopen() {
        let path = temp("checkpoint-task");
        let _home = crate::db::test_support::with_home(path.parent().unwrap());
        let conn = open_at(&path).expect("open config DB");
        conn.execute(
            "INSERT INTO checkpoints
             (id, session_id, task_id, file_path, snapshot_path, created_ms, reason, size_bytes)
             VALUES ('cp-1', 'worker-session', 42, 'C:/repo/src/lib.rs', 'C:/snap/lib.snap', 1, 'pre_write', 10)",
            [],
        )
        .expect("write task-associated checkpoint");
        drop(conn);

        let reopened = open_at(&path).expect("reopen config DB");
        let (session_id, task_id): (String, Option<i64>) = reopened
            .query_row(
                "SELECT session_id, task_id FROM checkpoints WHERE id='cp-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read checkpoint association");
        assert_eq!(session_id, "worker-session");
        assert_eq!(task_id, Some(42));
        drop(reopened);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// A `config.db` that predates the ladder — tables present, version 0 —
    /// is the shape every live installation is in. It must be stamped without
    /// losing its rows.
    #[test]
    fn a_pre_ladder_config_db_is_stamped_without_losing_data() {
        let path = temp("legacy");
        let _home = crate::db::test_support::with_home(path.parent().unwrap());
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
        assert_eq!(migrate::user_version(&conn).unwrap(), 7);
        let v: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='workspace_dir'",
                [],
                |r| r.get(0),
            )
            .expect("the existing row must survive");
        assert_eq!(v, "C:/live");
        let legacy_timestamp: String = conn
            .query_row(
                "SELECT updated_at FROM app_settings WHERE key='workspace_dir'",
                [],
                |r| r.get(0),
            )
            .expect("legacy timestamp remains persisted");
        assert_eq!(legacy_timestamp.len(), 19);
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('new-setting', 'value')",
            [],
        )
        .expect("production default writer");
        let new_timestamp: String = conn
            .query_row(
                "SELECT updated_at FROM app_settings WHERE key='new-setting'",
                [],
                |r| r.get(0),
            )
            .expect("read inserted setting");
        assert_eq!(new_timestamp.len(), 24);
        assert!(new_timestamp.ends_with('Z'));
        conn.execute(
            "UPDATE app_settings SET value='changed' WHERE key='workspace_dir'",
            [],
        )
        .expect("production update trigger");
        let updated_timestamp: String = conn
            .query_row(
                "SELECT updated_at FROM app_settings WHERE key='workspace_dir'",
                [],
                |r| r.get(0),
            )
            .expect("read triggered update");
        assert_eq!(updated_timestamp.len(), 24);
        assert!(updated_timestamp.ends_with('Z'));
        for t in TABLES {
            assert!(table_exists(&conn, t), "missing table {t}");
        }
        drop(conn);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
