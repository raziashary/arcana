// Graph schema migrations — versioned updates with rollback safety

use super::schema::*;
use crate::errors::Result;
use rusqlite::Connection;

pub fn get_schema_version(conn: &Connection) -> Result<u32> {
    let version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    Ok(version)
}

pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    tracing::debug!("[Migrations] Getting current schema version...");
    let current = get_schema_version(conn)?;
    tracing::info!("[Migrations] Current schema version: {}", current);

    if current < 1 {
        tracing::info!("[Migrations] Running migration_v1...");
        migration_v1(conn)?;
        tracing::info!("[Migrations] ✅ migration_v1 complete");
    }
    if current < 2 {
        tracing::info!("[Migrations] Running migration_v2...");
        migration_v2(conn)?;
        tracing::info!("[Migrations] ✅ migration_v2 complete");
    }
    if current < 3 {
        tracing::info!("[Migrations] Running migration_v3...");
        migration_v3(conn)?;
        tracing::info!("[Migrations] ✅ migration_v3 complete");
    }
    if current < 4 {
        tracing::info!("[Migrations] Running migration_v4...");
        migration_v4(conn)?;
        tracing::info!("[Migrations] ✅ migration_v4 complete");
    }
    if current < 5 {
        tracing::info!("[Migrations] Running migration_v5...");
        migration_v5(conn)?;
        tracing::info!("[Migrations] ✅ migration_v5 complete");
    }
    if current < 6 {
        tracing::info!("[Migrations] Running migration_v6...");
        migration_v6(conn)?;
        tracing::info!("[Migrations] ✅ migration_v6 complete");
    }

    tracing::info!("[Migrations] All migrations complete");
    Ok(())
}

fn migration_v1(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;

    // Execute each CREATE TABLE separately to avoid SQL parsing issues
    tx.execute_batch(CREATE_NODES)?;
    tx.execute_batch(CREATE_EDGES)?;
    tx.execute_batch(CREATE_COMMUNITIES)?;
    tx.execute_batch(CREATE_REPOS)?;
    tx.execute_batch(CREATE_SNAPSHOTS)?;
    tx.execute_batch(CREATE_ANALYSIS_CACHE)?;
    tx.execute_batch(CREATE_NODES_FTS)?;

    for idx in ALL_INDEXES {
        tx.execute(idx, [])?;
    }

    tx.execute("PRAGMA user_version = 1", [])?;
    tx.commit()?;

    Ok(())
}

fn migration_v2(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;

    // v2: communities table already in v1, just ensure it exists
    tx.execute_batch(CREATE_COMMUNITIES)?;

    tx.execute("PRAGMA user_version = 2", [])?;
    tx.commit()?;

    Ok(())
}

fn migration_v3(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;

    // v3: add flows table (already in schema)
    tx.execute_batch(CREATE_FLOWS)?;

    tx.execute("PRAGMA user_version = 3", [])?;
    tx.commit()?;

    Ok(())
}

fn migration_v4(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;

    // v4: analysis_cache table already in schema
    tx.execute_batch(CREATE_ANALYSIS_CACHE)?;

    // Rebuild the FTS5 index.
    //
    // ⚠ The post-phase audit flagged this `DROP TABLE` as a destructive
    // statement outside `run_ladder`'s backup guard, which is literally true.
    // No backup is taken, and that is correct: `nodes_fts` is declared
    // `content='nodes'` — an **external-content** FTS5 table. It stores no data
    // of its own, `nodes` is the source of truth, and the `rebuild` two lines
    // below regenerates it in full. Backing up the whole graph database before
    // re-indexing would copy megabytes to protect nothing.
    //
    // If `nodes_fts` ever stops being external-content, this comment is wrong
    // and a backup becomes mandatory.
    tx.execute_batch("DROP TABLE IF EXISTS nodes_fts")?;
    tx.execute_batch(CREATE_NODES_FTS)?;
    tx.execute_batch("INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild')")?;

    tx.execute("PRAGMA user_version = 4", [])?;
    tx.commit()?;

    Ok(())
}

fn migration_v5(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;

    // v5 originally issued six `ALTER TABLE … ADD COLUMN IF NOT EXISTS`
    // statements, each wrapped in `let _ = …`.
    //
    // `ADD COLUMN IF NOT EXISTS` is not valid SQLite, so all six failed — and
    // the discarded results meant the migration stamped `user_version = 5`,
    // committed, and logged "✅ complete" regardless.
    //
    // They were also **redundant**: `CREATE_NODES` already declares
    // `agent_author`, `task_id`, `worker_locked` and `worktree`, and
    // `CREATE_EDGES` declares `agent_author` and `task_id`, so every database
    // created by `migration_v1` has had them all along. That is why the two
    // `CREATE INDEX` statements below — which do *not* discard their result —
    // have always succeeded: the columns they index were already there.
    //
    // The six statements are therefore removed rather than repaired. Databases
    // predating those columns are handled by `migration_v6`, which checks
    // before it alters. See ADR-LOG-RESTRUCT-0017.

    // Create indexes for the columns declared in v1's schema.
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_nodes_author ON nodes(agent_author)",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_nodes_task ON nodes(task_id)",
        [],
    )?;

    tx.execute("PRAGMA user_version = 5", [])?;
    tx.commit()?;

    Ok(())
}

/// v6 — guarantee the agent-attribution columns exist, whatever the database's
/// history.
///
/// Every database created by `migration_v1` already has them: they are declared
/// in `CREATE_NODES` and `CREATE_EDGES`. The live graph store was verified to
/// carry all six before this migration was written, so on it v6 is a no-op that
/// only advances the version.
///
/// It exists for databases created by a build whose `CREATE_NODES` predated
/// those columns and which were then "migrated" by the broken v5 — for them the
/// columns are genuinely absent, and nothing else would ever add them, because
/// `CREATE TABLE IF NOT EXISTS` does not alter a table that already exists.
///
/// Unlike v5, this checks `PRAGMA table_info` before issuing each `ALTER` and
/// propagates any failure instead of discarding it.
fn migration_v6(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;

    crate::db::migrate::ensure_columns(
        &tx,
        "nodes",
        &[
            (
                "agent_author",
                "ALTER TABLE nodes ADD COLUMN agent_author TEXT DEFAULT NULL",
            ),
            (
                "task_id",
                "ALTER TABLE nodes ADD COLUMN task_id INTEGER DEFAULT NULL",
            ),
            (
                "worker_locked",
                "ALTER TABLE nodes ADD COLUMN worker_locked BOOLEAN DEFAULT 0",
            ),
            (
                "worktree",
                "ALTER TABLE nodes ADD COLUMN worktree TEXT DEFAULT NULL",
            ),
        ],
    )?;

    crate::db::migrate::ensure_columns(
        &tx,
        "edges",
        &[
            (
                "agent_author",
                "ALTER TABLE edges ADD COLUMN agent_author TEXT DEFAULT NULL",
            ),
            (
                "task_id",
                "ALTER TABLE edges ADD COLUMN task_id INTEGER DEFAULT NULL",
            ),
        ],
    )?;

    // Safe to re-create: these are `IF NOT EXISTS`, and after the calls above
    // the columns they name are guaranteed to exist.
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_nodes_author ON nodes(agent_author)",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_nodes_task ON nodes(task_id)",
        [],
    )?;

    tx.execute("PRAGMA user_version = 6", [])?;
    tx.commit()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns(conn: &Connection, table: &str) -> Vec<String> {
        let mut st = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("table_info");
        let rows = st.query_map([], |r| r.get::<_, String>(1)).expect("query");
        rows.map(|r| r.expect("col")).collect()
    }

    /// A fresh database gets the full schema from v1 and ends at the head.
    #[test]
    fn a_fresh_graph_database_reaches_the_ladder_head_with_every_column() {
        let mut conn = Connection::open_in_memory().expect("mem");
        run_migrations(&mut conn).expect("migrations");

        assert_eq!(get_schema_version(&conn).unwrap(), 6);
        let nodes = columns(&conn, "nodes");
        for c in ["agent_author", "task_id", "worker_locked", "worktree"] {
            assert!(nodes.iter().any(|x| x == c), "nodes is missing {c}");
        }
        let edges = columns(&conn, "edges");
        for c in ["agent_author", "task_id"] {
            assert!(edges.iter().any(|x| x == c), "edges is missing {c}");
        }
    }

    /// The database v6 exists for.
    ///
    /// A graph store created by a build whose `CREATE_NODES` predated the
    /// agent-attribution columns, then stamped `user_version = 5` by the broken
    /// v5 without gaining them. Nothing else would ever add them:
    /// `CREATE TABLE IF NOT EXISTS` does not alter an existing table.
    #[test]
    fn a_v5_database_that_never_got_the_columns_is_repaired_by_v6() {
        let mut conn = Connection::open_in_memory().expect("mem");
        conn.execute_batch(
            "CREATE TABLE nodes (
                 id INTEGER PRIMARY KEY, kind TEXT, name TEXT, qualified_name TEXT
             );
             CREATE TABLE edges (
                 id INTEGER PRIMARY KEY, kind TEXT, source_qualified TEXT
             );
             INSERT INTO nodes (kind, name, qualified_name) VALUES ('fn','probe','m::probe');
             PRAGMA user_version = 5;",
        )
        .expect("legacy schema");

        // The fixture must actually be missing the columns, or this proves nothing.
        assert!(!columns(&conn, "nodes").iter().any(|c| c == "agent_author"));

        run_migrations(&mut conn).expect("v6 must repair it");

        assert_eq!(get_schema_version(&conn).unwrap(), 6);
        let nodes = columns(&conn, "nodes");
        for c in ["agent_author", "task_id", "worker_locked", "worktree"] {
            assert!(nodes.iter().any(|x| x == c), "nodes is still missing {c}");
        }
        let edges = columns(&conn, "edges");
        for c in ["agent_author", "task_id"] {
            assert!(edges.iter().any(|x| x == c), "edges is still missing {c}");
        }

        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1, "the existing row must survive the repair");
    }

    /// The live graph store's actual shape: version 5, columns already present.
    /// v6 must advance it without attempting an `ALTER` that would fail with
    /// *duplicate column name*.
    #[test]
    fn a_v5_database_that_already_has_the_columns_is_advanced_untouched() {
        let mut conn = Connection::open_in_memory().expect("mem");
        run_migrations(&mut conn).expect("build full schema");
        conn.execute_batch("PRAGMA user_version = 5")
            .expect("rewind");

        let before = columns(&conn, "nodes");
        run_migrations(&mut conn).expect("v6 must be a no-op here");

        assert_eq!(get_schema_version(&conn).unwrap(), 6);
        assert_eq!(columns(&conn, "nodes"), before, "schema must be unchanged");
    }

    /// The audit flagged `migration_v4`'s `DROP TABLE nodes_fts` as destruction
    /// without a backup. It is safe only because the table is external-content
    /// — this pins that property, so if the schema ever changes the assumption
    /// fails loudly here rather than silently losing an index nobody can
    /// rebuild.
    #[test]
    fn the_fts_table_is_external_content_so_dropping_it_loses_nothing() {
        assert!(
            super::super::schema::CREATE_NODES_FTS.contains("content='nodes'"),
            "nodes_fts is no longer external-content: dropping it now loses data, and migration_v4 must take a backup first"
        );

        let mut conn = Connection::open_in_memory().expect("mem");
        run_migrations(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO nodes              (kind, name, qualified_name, file_path, line_start, line_end, language, file_hash)              VALUES ('fn', 'probe', 'm::probe', 'src/m.rs', 1, 2, 'rust', 'deadbeef')",
            [],
        )
        .expect("insert");

        // Drop and rebuild exactly as v4 does, and show the content returns.
        conn.execute_batch("DROP TABLE IF EXISTS nodes_fts")
            .expect("drop");
        conn.execute_batch(super::super::schema::CREATE_NODES_FTS)
            .expect("recreate");
        conn.execute_batch("INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild')")
            .expect("rebuild");

        let hits: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM nodes_fts WHERE nodes_fts MATCH 'probe'",
                [],
                |r| r.get(0),
            )
            .expect("search");
        assert_eq!(hits, 1, "the index did not come back from its source table");
    }

    /// Re-running the whole ladder changes nothing.
    #[test]
    fn migrations_are_idempotent() {
        let mut conn = Connection::open_in_memory().expect("mem");
        run_migrations(&mut conn).expect("first");
        let before = columns(&conn, "nodes");
        run_migrations(&mut conn).expect("second");
        assert_eq!(get_schema_version(&conn).unwrap(), 6);
        assert_eq!(columns(&conn, "nodes"), before);
    }
}
