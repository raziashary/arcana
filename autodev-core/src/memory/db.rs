// Memory SQLite database — ~/.autodev/memory.db
// All 3 tiers live in one database: episodic, semantic, procedural.
// Migration is purely additive via CREATE TABLE IF NOT EXISTS so new columns
// can be appended in later schema revisions without breaking old installs.

use crate::db::migrate::{self, Migration};
use crate::errors::{AutoDevError, Result};
use rusqlite::Connection;
use std::path::PathBuf;

pub struct MemoryDb {
    pub conn: Connection,
}

impl MemoryDb {
    /// This database's schema version, as recorded by the migration ladder.
    ///
    /// Exposed so a migration can be *verified* rather than assumed — see
    /// `examples/migrate_check.rs`. Reading it through the same handle the
    /// application uses is the point: a version read from a fresh connection
    /// would prove the file's state, not that the shipped `open()` path put it
    /// there.
    pub fn schema_version(&self) -> Result<i32> {
        migrate::user_version(&self.conn)
    }

    pub fn open() -> Result<Self> {
        // ⚠ P3-30. BEFORE `open_tuned`, deliberately. `sqlite3_auto_extension`
        // installs a hook that only affects connections opened AFTER it runs —
        // registering after the open would leave THIS connection without
        // `vec0`, and the failure would surface later as `no such module`,
        // far from its cause. Idempotent, so calling it here as well as at
        // startup costs nothing and removes the ordering hazard entirely.
        crate::memory::vector::register_once();

        let path = db_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = crate::db::open_tuned(&path)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// `memory.db`'s ladder. `v1` is the schema as it stands, created idempotently,
    /// so an existing database is stamped without being altered.
    pub(crate) const LADDER: &[Migration] = &[Migration {
        version: 1,
        name: "memory-baseline",
        sql: r#"

            -- ─────────────────────────────────────────────────────────────
            -- Tier 1: Episodic Memory (raw conversation experience)
            -- ─────────────────────────────────────────────────────────────
            CREATE TABLE IF NOT EXISTS episodic_memories (
                id           TEXT PRIMARY KEY,
                session_id   TEXT NOT NULL,
                turn_index   INTEGER NOT NULL DEFAULT 1,
                role         TEXT NOT NULL,       -- 'user' | 'agent'
                content      TEXT NOT NULL,
                model        TEXT,
                cost_usd     REAL NOT NULL DEFAULT 0.0,
                tokens_in    INTEGER NOT NULL DEFAULT 0,
                tokens_out   INTEGER NOT NULL DEFAULT 0,
                thinking     TEXT,                -- agent extended thinking
                tool_calls   TEXT,                -- JSON array of tool call objects
                working_dir  TEXT,
                summary_type TEXT,               -- NULL | 'session_summary'
                metadata     TEXT,               -- JSON blob
                created_at   TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_epi_session
                ON episodic_memories(session_id, turn_index);
            CREATE INDEX IF NOT EXISTS idx_epi_created
                ON episodic_memories(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_epi_summary
                ON episodic_memories(summary_type)
                WHERE summary_type IS NOT NULL;

            -- ─────────────────────────────────────────────────────────────
            -- Tier 2: Semantic Memory (extracted facts + knowledge)
            -- ─────────────────────────────────────────────────────────────
            CREATE TABLE IF NOT EXISTS semantic_memories (
                id             TEXT PRIMARY KEY,
                fact_text      TEXT NOT NULL,
                category       TEXT NOT NULL DEFAULT 'preference',
                confidence     REAL NOT NULL DEFAULT 0.7,
                source_type    TEXT NOT NULL DEFAULT 'extracted',
                source_session TEXT,
                working_dir    TEXT,
                tags           TEXT NOT NULL DEFAULT '[]',
                is_active      INTEGER NOT NULL DEFAULT 1,
                created_at     TEXT NOT NULL,
                updated_at     TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_sem_category
                ON semantic_memories(category, is_active);
            CREATE INDEX IF NOT EXISTS idx_sem_confidence
                ON semantic_memories(confidence DESC)
                WHERE is_active = 1;

            -- Fact history — contradiction tracking (Phase 4.2)
            CREATE TABLE IF NOT EXISTS fact_history (
                id         TEXT PRIMARY KEY,
                fact_id    TEXT NOT NULL REFERENCES semantic_memories(id),
                old_text   TEXT NOT NULL,
                new_text   TEXT NOT NULL,
                changed_at TEXT NOT NULL
            );

            -- ─────────────────────────────────────────────────────────────
            -- Tier 3: Procedural Memory (skills + patterns — SQLite mirror)
            -- Primary storage is the MEMORY.md file tree; this table enables
            -- fast full-text search and tag-based filtering.
            -- ─────────────────────────────────────────────────────────────
            CREATE TABLE IF NOT EXISTS procedural_memories (
                id         TEXT PRIMARY KEY,
                topic      TEXT NOT NULL UNIQUE,  -- e.g. 'debugging', 'workflow-patterns'
                title      TEXT NOT NULL,
                content    TEXT NOT NULL,          -- full markdown body
                tags       TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_proc_topic
                ON procedural_memories(topic);
            CREATE INDEX IF NOT EXISTS idx_proc_updated
                ON procedural_memories(updated_at DESC);
        "#,
    },
    // ═════════════════════════════════════════════════════════════════════
    // v2 — the unified retrieval index (P3-30 / P3-31 / P3-38 / P3-42)
    // ═════════════════════════════════════════════════════════════════════
    //
    // ⚠ A NEW RUNG, not an edit to v1. `L-C13`: a database already at v1 would
    // never receive columns added to the baseline, and only a *fresh* database
    // would look correct — the defect would be invisible to exactly the test
    // most likely to be written.
    //
    // ── Why a separate index rather than columns on the three tier tables ──
    //
    // Spec `03 §4.5` states the target shape: *"The schema uses a tier
    // discriminator value, not a table per tier."* Retrieval also has to rank
    // all three tiers against each other in one pool (`§4.2`), which a
    // per-table design cannot do without a union at every query.
    //
    // The tier tables keep their tier-specific payload (episodic keeps cost and
    // tokens, semantic keeps category and tags, procedural keeps topic). This
    // table carries only what RETRIEVAL needs, and `ref_id` points back.
    // Everything is written through one seam so the two cannot diverge.
    Migration {
        version: 2,
        name: "memory-retrieval-index",
        sql: r#"
            CREATE TABLE IF NOT EXISTS memories (
                id            TEXT PRIMARY KEY,
                -- The discriminator. 'episodic' | 'semantic' | 'procedural',
                -- and 'working' later without a migration (OQ-7).
                tier          TEXT NOT NULL,

                -- ── Scope (P3-31) ────────────────────────────────────────
                -- A MANDATORY retrieval filter, not a nicety: the sidecar's
                -- episodic recall never forwarded `agent_id` despite the callee
                -- supporting it, and procedural recall had no type filter at
                -- all. Scope closes that structurally.
                scope         TEXT NOT NULL DEFAULT 'global',
                scope_ref     TEXT NOT NULL DEFAULT '',
                -- The vec0 PARTITION KEY. Denormalised from the pair above
                -- because vec0 partition keys are exact-match and TEXT/INTEGER
                -- only (measured), so the pair has to be one comparable value.
                scope_key     TEXT NOT NULL DEFAULT 'global',

                content       TEXT NOT NULL,

                -- ── Provenance (P3-38) ───────────────────────────────────
                -- What makes a contradiction resolvable later instead of
                -- needing a migration to add. It also separates "the user told
                -- me" from "I observed it" — a fact from a guess.
                source        TEXT NOT NULL DEFAULT 'conversation',
                source_file   TEXT,
                source_line   INTEGER,

                -- ── Reranker columns (P3-42) ─────────────────────────────
                -- The anchor's §10 terms that the competitive model does NOT
                -- carry. Six of its eight are absent there, which is why this
                -- is its own work item with its own columns rather than a port.
                importance       REAL    NOT NULL DEFAULT 0.5,
                confidence       REAL    NOT NULL DEFAULT 0.7,
                access_count     INTEGER NOT NULL DEFAULT 0,
                last_accessed_at TEXT,

                -- Back-pointer to the tier payload row.
                ref_id        TEXT,
                created_at    TEXT NOT NULL,
                updated_at    TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_mem_scope
                ON memories(scope_key, tier);
            CREATE INDEX IF NOT EXISTS idx_mem_created
                ON memories(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_mem_ref
                ON memories(tier, ref_id);

            -- ── Lexical leg: FTS5 over the same rows ────────────────────
            -- External-content, so the index cannot hold a second copy that
            -- drifts. The triggers make desync structurally impossible rather
            -- than a thing the write path must remember.
            CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                content,
                content='memories',
                content_rowid='rowid'
            );

            CREATE TRIGGER IF NOT EXISTS memories_fts_ai AFTER INSERT ON memories BEGIN
                INSERT INTO memories_fts(rowid, content) VALUES (new.rowid, new.content);
            END;
            CREATE TRIGGER IF NOT EXISTS memories_fts_ad AFTER DELETE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, content)
                VALUES ('delete', old.rowid, old.content);
            END;
            CREATE TRIGGER IF NOT EXISTS memories_fts_au AFTER UPDATE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, content)
                VALUES ('delete', old.rowid, old.content);
                INSERT INTO memories_fts(rowid, content) VALUES (new.rowid, new.content);
            END;

            -- ── Backfill: the rows that already exist ───────────────────
            -- 39 episodic rows on the live database at the time this was
            -- written. Their `working_dir` becomes a project scope where it is
            -- present, so existing history is scoped rather than dumped into
            -- 'global' where every project would recall it.
            INSERT OR IGNORE INTO memories (
                id, tier, scope, scope_ref, scope_key, content,
                source, importance, confidence, ref_id, created_at, updated_at
            )
            SELECT
                'epi-' || e.id,
                'episodic',
                CASE WHEN COALESCE(e.working_dir, '') = '' THEN 'global' ELSE 'project' END,
                COALESCE(e.working_dir, ''),
                CASE WHEN COALESCE(e.working_dir, '') = '' THEN 'global'
                     ELSE 'project:' || e.working_dir END,
                e.content,
                CASE WHEN e.role = 'user' THEN 'user' ELSE 'agent_observation' END,
                0.5,
                0.7,
                e.id,
                e.created_at,
                e.created_at
            FROM episodic_memories e
            WHERE e.content IS NOT NULL AND e.content <> '';

            INSERT OR IGNORE INTO memories (
                id, tier, scope, scope_ref, scope_key, content,
                source, importance, confidence, ref_id, created_at, updated_at
            )
            SELECT
                'sem-' || f.id,
                'semantic',
                CASE WHEN COALESCE(f.working_dir, '') = '' THEN 'global' ELSE 'project' END,
                COALESCE(f.working_dir, ''),
                CASE WHEN COALESCE(f.working_dir, '') = '' THEN 'global'
                     ELSE 'project:' || f.working_dir END,
                f.fact_text,
                CASE WHEN f.source_type = 'user_explicit' THEN 'user' ELSE 'agent_observation' END,
                0.5,
                f.confidence,
                f.id,
                f.created_at,
                f.updated_at
            FROM semantic_memories f
            WHERE f.is_active = 1;

            INSERT OR IGNORE INTO memories (
                id, tier, scope, scope_ref, scope_key, content,
                source, importance, confidence, ref_id, created_at, updated_at
            )
            SELECT
                'prc-' || p.id,
                'procedural',
                'global',
                '',
                'global',
                p.title || ': ' || p.content,
                'agent_observation',
                0.5,
                0.7,
                p.id,
                p.created_at,
                p.updated_at
            FROM procedural_memories p;
        "#,
    }];

    fn migrate(&self) -> Result<()> {
        // ⚠ `sqlite3_auto_extension` only affects connections opened AFTER it
        // runs, so registering here — before the ladder, on a connection this
        // function is about to migrate — is not sufficient on its own; the
        // connection is already open. `MemoryDb::open` therefore registers
        // BEFORE it opens, and `main.rs` registers at startup. This call is the
        // third belt: idempotent, and it makes the ordering requirement
        // impossible to miss when reading the migration in isolation.
        crate::memory::vector::register_once();

        // PRAGMAs are per-connection and are not carried by a migration.
        self.conn
            .execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
            .map_err(|e| AutoDevError::Agent(format!("memory.db pragmas: {e}")))?;
        migrate::run_ladder(&self.conn, "memory.db", Self::LADDER)?;
        Ok(())
    }
}

pub fn db_path() -> Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| AutoDevError::Agent("Cannot determine home directory for memory.db".into()))?;
    Ok(home.join(".autodev").join("memory.db"))
}

#[cfg(test)]
mod p3_30_tests {
    use super::*;
    use rusqlite::Connection;

    /// Run the real ladder against an in-memory database.
    fn laddered() -> Connection {
        crate::memory::vector::register_once();
        let c = Connection::open_in_memory().expect("open");
        c.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        crate::db::migrate::run_ladder(&c, "memory.db(test)", MemoryDb::LADDER)
            .expect("the ladder must apply cleanly to a fresh database");
        c
    }

    #[test]
    fn a_fresh_database_reaches_v2_with_all_three_legs() {
        let c = laddered();
        assert_eq!(crate::db::migrate::user_version(&c).unwrap(), 2);

        // The relational leg.
        let cols: Vec<String> = c
            .prepare("SELECT name FROM pragma_table_info('memories')")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for required in [
            "tier", "scope", "scope_ref", "scope_key", "content",
            "source", "source_file", "source_line",
            "importance", "confidence", "access_count", "last_accessed_at",
        ] {
            assert!(
                cols.iter().any(|c| c == required),
                "`memories` is missing `{required}` — it has {cols:?}"
            );
        }

        // The lexical leg.
        c.execute_batch(
            "INSERT INTO memories(id,tier,scope,scope_ref,scope_key,content,created_at,updated_at)
             VALUES ('m1','semantic','global','','global','the quick brown fox','t','t');",
        )
        .unwrap();
        let hits: i64 = c
            .query_row(
                "SELECT count(*) FROM memories_fts WHERE memories_fts MATCH 'brown'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1, "the FTS5 trigger did not index the insert");

        // …and it stays in sync on update and delete, which is the whole
        // reason the triggers exist rather than a write-path convention.
        c.execute_batch("UPDATE memories SET content='a lazy dog' WHERE id='m1';")
            .unwrap();
        let stale: i64 = c
            .query_row(
                "SELECT count(*) FROM memories_fts WHERE memories_fts MATCH 'brown'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stale, 0, "FTS5 kept the OLD content after an update");
        c.execute_batch("DELETE FROM memories WHERE id='m1';").unwrap();
        let after_delete: i64 = c
            .query_row(
                "SELECT count(*) FROM memories_fts WHERE memories_fts MATCH 'lazy'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(after_delete, 0, "FTS5 kept a deleted row");
    }

    /// 🔴 `L-C13`. A database already at v1 must receive v2 too.
    ///
    /// The register's own instance of this class was a migration branch that
    /// only *existing* databases take, so live-data proof could not see it.
    /// This is the mirror: a rung only *fresh* databases would take. The v1
    /// baseline was therefore left untouched and v2 appended — and this test
    /// is what proves the live database at v1 actually traverses it.
    #[test]
    fn a_database_already_at_v1_is_carried_to_v2_and_its_rows_are_indexed() {
        crate::memory::vector::register_once();
        let c = Connection::open_in_memory().unwrap();

        // Build a v1 database exactly as the shipped baseline does, and stamp
        // it — this is the state the live `memory.db` was in.
        c.execute_batch(MemoryDb::LADDER[0].sql).unwrap();
        c.pragma_update(None, "user_version", 1).unwrap();

        // Give it history, the way the live database has 39 episodic rows.
        c.execute_batch(
            "INSERT INTO episodic_memories(id,session_id,turn_index,role,content,working_dir,created_at)
             VALUES ('e1','s1',1,'user','remember the deploy key rotation','/work/proj','t1'),
                    ('e2','s1',2,'agent','noted','', 't2');",
        )
        .unwrap();

        crate::db::migrate::run_ladder(&c, "memory.db(test)", MemoryDb::LADDER).unwrap();
        assert_eq!(crate::db::migrate::user_version(&c).unwrap(), 2);

        // Pre-existing rows are indexed, not stranded.
        let n: i64 = c
            .query_row("SELECT count(*) FROM memories WHERE tier='episodic'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2, "existing episodic rows were not backfilled");

        // …and scoped by their working_dir rather than dumped into 'global',
        // where every other project would recall them.
        let scoped: String = c
            .query_row("SELECT scope_key FROM memories WHERE ref_id='e1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(scoped, "project:/work/proj");
        let global: String = c
            .query_row("SELECT scope_key FROM memories WHERE ref_id='e2'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(global, "global", "a row with no working_dir must be global");

        // Backfilled rows are searchable — the backfill runs INSERT, so the
        // FTS5 trigger fires for it too.
        let hits: i64 = c
            .query_row(
                "SELECT count(*) FROM memories_fts WHERE memories_fts MATCH 'rotation'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1, "backfilled rows were not indexed for lexical search");
    }

    #[test]
    fn the_ladder_is_idempotent() {
        // A second run must be a no-op, not a duplicate backfill.
        let c = laddered();
        c.execute_batch(
            "INSERT INTO episodic_memories(id,session_id,turn_index,role,content,created_at)
             VALUES ('e9','s1',1,'user','hello','t');",
        )
        .unwrap();
        crate::db::migrate::run_ladder(&c, "memory.db(test)", MemoryDb::LADDER).unwrap();
        crate::db::migrate::run_ladder(&c, "memory.db(test)", MemoryDb::LADDER).unwrap();
        assert_eq!(crate::db::migrate::user_version(&c).unwrap(), 2);
    }
}
