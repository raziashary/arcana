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

    // Rebuild FTS5 index
    tx.execute_batch("DROP TABLE IF EXISTS nodes_fts")?;
    tx.execute_batch(CREATE_NODES_FTS)?;
    tx.execute_batch("INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild')")?;

    tx.execute("PRAGMA user_version = 4", [])?;
    tx.commit()?;

    Ok(())
}

fn migration_v5(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;

    // v5: add agent_author, task_id, worker_locked, worktree columns to nodes
    // Use IF NOT EXISTS to handle idempotency (columns may already exist)
    let _ = tx
        .execute_batch("ALTER TABLE nodes ADD COLUMN IF NOT EXISTS agent_author TEXT DEFAULT NULL");
    let _ =
        tx.execute_batch("ALTER TABLE nodes ADD COLUMN IF NOT EXISTS task_id INTEGER DEFAULT NULL");
    let _ = tx.execute_batch(
        "ALTER TABLE nodes ADD COLUMN IF NOT EXISTS worker_locked BOOLEAN DEFAULT 0",
    );
    let _ =
        tx.execute_batch("ALTER TABLE nodes ADD COLUMN IF NOT EXISTS worktree TEXT DEFAULT NULL");

    // v5: add agent_author, task_id columns to edges
    let _ = tx
        .execute_batch("ALTER TABLE edges ADD COLUMN IF NOT EXISTS agent_author TEXT DEFAULT NULL");
    let _ =
        tx.execute_batch("ALTER TABLE edges ADD COLUMN IF NOT EXISTS task_id INTEGER DEFAULT NULL");

    // Create indexes for new columns
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
