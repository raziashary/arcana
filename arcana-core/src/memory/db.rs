// Memory SQLite database — ~/.arcana/memory.db
// All 3 tiers live in one database: episodic, semantic, procedural.
// Migration is purely additive via CREATE TABLE IF NOT EXISTS so new columns
// can be appended in later schema revisions without breaking old installs.

use crate::db::migrate::{self, Migration};
use crate::errors::{ArcanaError, Result};
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

    /// Apply this database's ladder to an arbitrary connection.
    ///
    /// Exposed for integration tests, which must run the **real** ladder rather
    /// than a hand-written copy of the schema — a fixture that drifts from the
    /// migration proves the fixture, not the product.
    pub fn run_ladder_on(conn: &Connection) -> Result<i32> {
        crate::memory::vector::register_once();
        migrate::run_ladder(conn, "memory.db", Self::LADDER)
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
    pub(crate) const LADDER: &[Migration] = &[
        Migration {
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
        //
        // 🔴 This comment claimed "everything is written through one seam so the
        // two cannot diverge". **That was false when written.** Only the gateway
        // wrote here; `episodic.rs` and `procedural.rs` wrote to their own tables
        // and nothing else, so every NEW turn and topic was invisible to recall —
        // the index was frozen at whatever the v2 backfill captured. Closed by v3,
        // which makes the indexing a database trigger rather than a convention the
        // write paths have to remember.
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
        },
        // ═════════════════════════════════════════════════════════════════════
        // v3 — keep the retrieval index in step with the tier tables
        // ═════════════════════════════════════════════════════════════════════
        //
        // 🔴 v2 backfilled the existing rows and stopped there. New episodic turns
        // and new procedural topics went into their own tables and **nowhere else**,
        // so the retrieval index was frozen at migration time: every conversation
        // after the upgrade was unrecallable, silently, with no error and nothing to
        // notice but recall that felt oddly stale.
        //
        // ⚠ Triggers rather than calls in the write paths, for the same reason the
        // FTS5 index uses them: a convention the write path must remember is one a
        // new write path will forget, and there are already four of them across
        // three files. The database enforces this now.
        //
        // The scope and source expressions mirror v2's backfill exactly, so a row
        // written today and the same row backfilled yesterday land identically.
        Migration {
            version: 3,
            name: "memory-index-triggers",
            sql: r#"
            -- Episodic: one row per turn.
            CREATE TRIGGER IF NOT EXISTS episodic_index_ai
            AFTER INSERT ON episodic_memories
            WHEN NEW.content IS NOT NULL AND NEW.content <> ''
            BEGIN
                INSERT OR IGNORE INTO memories (
                    id, tier, scope, scope_ref, scope_key, content,
                    source, importance, confidence, ref_id, created_at, updated_at
                ) VALUES (
                    'epi-' || NEW.id,
                    'episodic',
                    CASE WHEN COALESCE(NEW.working_dir,'') = '' THEN 'global' ELSE 'project' END,
                    COALESCE(NEW.working_dir,''),
                    CASE WHEN COALESCE(NEW.working_dir,'') = '' THEN 'global'
                         ELSE 'project:' || NEW.working_dir END,
                    NEW.content,
                    CASE WHEN NEW.role = 'user' THEN 'user' ELSE 'agent_observation' END,
                    0.5, 0.7, NEW.id, NEW.created_at, NEW.created_at
                );
            END;

            -- Semantic: the legacy direct-write path. The gateway writes to
            -- `memories` itself, so this covers anything that still writes a
            -- fact the old way rather than leaving it unindexed.
            CREATE TRIGGER IF NOT EXISTS semantic_index_ai
            AFTER INSERT ON semantic_memories
            WHEN NEW.is_active = 1
            BEGIN
                INSERT OR IGNORE INTO memories (
                    id, tier, scope, scope_ref, scope_key, content,
                    source, importance, confidence, ref_id, created_at, updated_at
                ) VALUES (
                    'sem-' || NEW.id,
                    'semantic',
                    CASE WHEN COALESCE(NEW.working_dir,'') = '' THEN 'global' ELSE 'project' END,
                    COALESCE(NEW.working_dir,''),
                    CASE WHEN COALESCE(NEW.working_dir,'') = '' THEN 'global'
                         ELSE 'project:' || NEW.working_dir END,
                    NEW.fact_text,
                    CASE WHEN NEW.source_type = 'user_explicit' THEN 'user'
                         ELSE 'agent_observation' END,
                    0.5, NEW.confidence, NEW.id, NEW.created_at, NEW.updated_at
                );
            END;

            -- Procedural: `save_topic` upserts on the UNIQUE topic, so the
            -- index has to follow BOTH halves or an edited topic keeps its old
            -- text in search forever.
            CREATE TRIGGER IF NOT EXISTS procedural_index_ai
            AFTER INSERT ON procedural_memories
            BEGIN
                INSERT OR IGNORE INTO memories (
                    id, tier, scope, scope_ref, scope_key, content,
                    source, importance, confidence, ref_id, created_at, updated_at
                ) VALUES (
                    'prc-' || NEW.id, 'procedural', 'global', '', 'global',
                    NEW.title || ': ' || NEW.content,
                    'agent_observation', 0.5, 0.7, NEW.id, NEW.created_at, NEW.updated_at
                );
            END;

            CREATE TRIGGER IF NOT EXISTS procedural_index_au
            AFTER UPDATE ON procedural_memories
            BEGIN
                UPDATE memories
                   SET content    = NEW.title || ': ' || NEW.content,
                       updated_at = NEW.updated_at
                 WHERE tier = 'procedural' AND ref_id = NEW.id;
            END;

            -- ── Edits and removals ───────────────────────────────────
            --
            -- 🔴 Insert triggers alone leave two defects standing, and the
            -- second is the serious one:
            --
            --   1. an EDITED fact serves its pre-edit text from the index for
            --      ever, so the contradiction-resolution machinery updates a
            --      column nothing reads back; and
            --   2. **"forget this" does not forget.** `deactivate_fact` sets
            --      `is_active = 0` and `delete_topic` removes the row, but the
            --      index copy is untouched, still in scope, and still selected
            --      by `LexicalBackend::search` — so a memory the user asked to
            --      remove keeps being injected into the system prompt on every
            --      turn, permanently, with no surface anywhere that admits it
            --      is still there.
            --
            -- A deletion path that silently retains the data is worse than
            -- having no deletion path at all, because the user is told it
            -- worked.

            CREATE TRIGGER IF NOT EXISTS semantic_index_au
            AFTER UPDATE ON semantic_memories
            WHEN NEW.is_active = 1
            BEGIN
                -- OR IGNORE first, so a fact REACTIVATED after a deactivation
                -- comes back rather than updating a row that is no longer
                -- there. The UPDATE then carries the current text either way,
                -- and `access_count` survives the common edit path.
                INSERT OR IGNORE INTO memories (
                    id, tier, scope, scope_ref, scope_key, content,
                    source, importance, confidence, ref_id, created_at, updated_at
                ) VALUES (
                    'sem-' || NEW.id,
                    'semantic',
                    CASE WHEN COALESCE(NEW.working_dir,'') = '' THEN 'global' ELSE 'project' END,
                    COALESCE(NEW.working_dir,''),
                    CASE WHEN COALESCE(NEW.working_dir,'') = '' THEN 'global'
                         ELSE 'project:' || NEW.working_dir END,
                    NEW.fact_text,
                    CASE WHEN NEW.source_type = 'user_explicit' THEN 'user'
                         ELSE 'agent_observation' END,
                    0.5, NEW.confidence, NEW.id, NEW.created_at, NEW.updated_at
                );
                UPDATE memories
                   SET content    = NEW.fact_text,
                       confidence = NEW.confidence,
                       updated_at = NEW.updated_at
                 WHERE tier = 'semantic' AND ref_id = NEW.id;
            END;

            -- Deactivation IS deletion, as far as retrieval is concerned.
            CREATE TRIGGER IF NOT EXISTS semantic_index_deactivate
            AFTER UPDATE ON semantic_memories
            WHEN NEW.is_active = 0
            BEGIN
                DELETE FROM memories WHERE tier = 'semantic' AND ref_id = NEW.id;
            END;

            CREATE TRIGGER IF NOT EXISTS semantic_index_ad
            AFTER DELETE ON semantic_memories
            BEGIN
                DELETE FROM memories WHERE tier = 'semantic' AND ref_id = OLD.id;
            END;

            CREATE TRIGGER IF NOT EXISTS episodic_index_ad
            AFTER DELETE ON episodic_memories
            BEGIN
                DELETE FROM memories WHERE tier = 'episodic' AND ref_id = OLD.id;
            END;

            CREATE TRIGGER IF NOT EXISTS procedural_index_ad
            AFTER DELETE ON procedural_memories
            BEGIN
                DELETE FROM memories WHERE tier = 'procedural' AND ref_id = OLD.id;
            END;

            -- ⚠ Nothing extra is needed to keep `memories_fts` in step: v2's
            -- `memories_fts_ad` / `_au` already fire on `memories` itself, so
            -- these deletes propagate. That is the point of indexing ONE table
            -- rather than three.

            -- Catch up anything written between v2 and v3.
            INSERT OR IGNORE INTO memories (
                id, tier, scope, scope_ref, scope_key, content,
                source, importance, confidence, ref_id, created_at, updated_at
            )
            SELECT 'epi-' || e.id, 'episodic',
                   CASE WHEN COALESCE(e.working_dir,'') = '' THEN 'global' ELSE 'project' END,
                   COALESCE(e.working_dir,''),
                   CASE WHEN COALESCE(e.working_dir,'') = '' THEN 'global'
                        ELSE 'project:' || e.working_dir END,
                   e.content,
                   CASE WHEN e.role = 'user' THEN 'user' ELSE 'agent_observation' END,
                   0.5, 0.7, e.id, e.created_at, e.created_at
            FROM episodic_memories e
            WHERE e.content IS NOT NULL AND e.content <> '';

            -- ⚠ And drop index rows whose subject was deactivated while the
            -- database sat at v2 — the window during which `deactivate_fact`
            -- was a no-op against retrieval. Catching up must include the
            -- removals, or the upgrade re-serves exactly the memories the user
            -- had already asked to forget.
            DELETE FROM memories
             WHERE tier = 'semantic'
               AND ref_id IN (SELECT id FROM semantic_memories WHERE is_active = 0);
        "#,
        },
    ];

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
            .map_err(|e| ArcanaError::Agent(format!("memory.db pragmas: {e}")))?;
        migrate::run_ladder(&self.conn, "memory.db", Self::LADDER)?;
        Ok(())
    }
}

pub fn db_path() -> Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| ArcanaError::Agent("Cannot determine home directory for memory.db".into()))?;
    Ok(home.join(".arcana").join("memory.db"))
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
        // The ladder's HEAD, not a literal — a rung added later must not make
        // this test fail for a reason that has nothing to do with what it checks.
        let head = MemoryDb::LADDER.last().unwrap().version;
        assert_eq!(crate::db::migrate::user_version(&c).unwrap(), head);

        // The relational leg.
        let cols: Vec<String> = c
            .prepare("SELECT name FROM pragma_table_info('memories')")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for required in [
            "tier",
            "scope",
            "scope_ref",
            "scope_key",
            "content",
            "source",
            "source_file",
            "source_line",
            "importance",
            "confidence",
            "access_count",
            "last_accessed_at",
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
        c.execute_batch("DELETE FROM memories WHERE id='m1';")
            .unwrap();
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
        // The ladder's HEAD, not a literal — a rung added later must not make
        // this test fail for a reason that has nothing to do with what it checks.
        let head = MemoryDb::LADDER.last().unwrap().version;
        assert_eq!(crate::db::migrate::user_version(&c).unwrap(), head);

        // Pre-existing rows are indexed, not stranded.
        let n: i64 = c
            .query_row(
                "SELECT count(*) FROM memories WHERE tier='episodic'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 2, "existing episodic rows were not backfilled");

        // …and scoped by their working_dir rather than dumped into 'global',
        // where every other project would recall them.
        let scoped: String = c
            .query_row(
                "SELECT scope_key FROM memories WHERE ref_id='e1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(scoped, "project:/work/proj");
        let global: String = c
            .query_row(
                "SELECT scope_key FROM memories WHERE ref_id='e2'",
                [],
                |r| r.get(0),
            )
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
        assert_eq!(
            hits, 1,
            "backfilled rows were not indexed for lexical search"
        );
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
        // The ladder's HEAD, not a literal — a rung added later must not make
        // this test fail for a reason that has nothing to do with what it checks.
        let head = MemoryDb::LADDER.last().unwrap().version;
        assert_eq!(crate::db::migrate::user_version(&c).unwrap(), head);
    }
}

#[cfg(test)]
mod p3_30_v3_tests {
    use super::*;
    use rusqlite::Connection;

    fn laddered() -> Connection {
        crate::memory::vector::register_once();
        let c = Connection::open_in_memory().unwrap();
        crate::db::migrate::run_ladder(&c, "memory.db(test)", MemoryDb::LADDER).unwrap();
        c
    }

    /// 🔴 The defect v3 closes, and it was mine.
    ///
    /// v2 backfilled the existing rows and stopped. New episodic turns went to
    /// `episodic_memories` and **nowhere else**, so the retrieval index was
    /// frozen at migration time — every conversation after the upgrade was
    /// unrecallable, silently. My own v2 comment claimed the opposite
    /// ("written through one seam so the two cannot diverge"), which is the
    /// `L-28` shape: a claim that looks verified.
    #[test]
    fn a_new_episodic_turn_is_indexed_for_retrieval() {
        let c = laddered();
        c.execute(
            "INSERT INTO episodic_memories(id,session_id,turn_index,role,content,working_dir,created_at)
             VALUES ('e-new','s1',1,'user','the release key lives in the ops vault','/proj','t1')",
            [],
        )
        .unwrap();

        let n: i64 = c
            .query_row(
                "SELECT count(*) FROM memories WHERE ref_id='e-new' AND tier='episodic'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            n, 1,
            "a new turn was not indexed — recall would be frozen at whatever the \
             migration backfilled"
        );

        // …and it is searchable, so the FTS5 trigger fired on the trigger's own
        // insert. A chain of triggers that stops halfway is worse than none.
        let hits: i64 = c
            .query_row(
                "SELECT count(*) FROM memories_fts WHERE memories_fts MATCH 'vault'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1, "the indexed row is not lexically searchable");

        // …and scoped from its working_dir, not dumped into global.
        let scope: String = c
            .query_row(
                "SELECT scope_key FROM memories WHERE ref_id='e-new'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(scope, "project:/proj");
    }

    #[test]
    fn a_new_procedural_topic_is_indexed_and_an_edit_follows_it() {
        let c = laddered();
        c.execute(
            "INSERT INTO procedural_memories(id,topic,title,content,tags,created_at,updated_at)
             VALUES ('p1','deploys','Deploy runbook','run make deploy','[]','t1','t1')",
            [],
        )
        .unwrap();
        let hits: i64 = c
            .query_row(
                "SELECT count(*) FROM memories_fts WHERE memories_fts MATCH 'runbook'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1, "a new topic was not indexed");

        // `save_topic` UPSERTs on the unique topic. If the index does not
        // follow the update, an edited topic keeps its OLD text in search
        // forever — stale in exactly the way that is hardest to notice.
        c.execute(
            "UPDATE procedural_memories SET title='Deploy guide',
                    content='run just deploy', updated_at='t2' WHERE id='p1'",
            [],
        )
        .unwrap();

        let stale: i64 = c
            .query_row(
                "SELECT count(*) FROM memories_fts WHERE memories_fts MATCH 'runbook'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            stale, 0,
            "the index kept the topic's OLD text after an edit"
        );
        let fresh: i64 = c
            .query_row(
                "SELECT count(*) FROM memories_fts WHERE memories_fts MATCH 'guide'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fresh, 1, "the edit did not reach the index");
    }

    #[test]
    fn a_legacy_direct_semantic_write_is_still_indexed() {
        // The gateway writes to `memories` itself. This covers anything that
        // still writes a fact the old way rather than leaving it invisible.
        let c = laddered();
        c.execute(
            "INSERT INTO semantic_memories(id,fact_text,category,confidence,source_type,
                                           working_dir,tags,is_active,created_at,updated_at)
             VALUES ('s1','the api listens on 8080','preference',0.9,'extracted',
                     '/proj','[]',1,'t1','t1')",
            [],
        )
        .unwrap();
        let n: i64 = c
            .query_row("SELECT count(*) FROM memories WHERE ref_id='s1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(n, 1, "a direct semantic write was not indexed");
    }

    fn add_fact(c: &Connection, id: &str, text: &str) {
        c.execute(
            "INSERT INTO semantic_memories(id,fact_text,category,confidence,source_type,
                                           working_dir,tags,is_active,created_at,updated_at)
             VALUES (?1,?2,'preference',0.9,'user_explicit','/proj','[]',1,'t1','t1')",
            rusqlite::params![id, text],
        )
        .unwrap();
    }

    fn indexed(c: &Connection, ref_id: &str) -> i64 {
        c.query_row(
            "SELECT count(*) FROM memories WHERE ref_id = ?1",
            [ref_id],
            |r| r.get(0),
        )
        .unwrap()
    }

    /// 🔴 **"Forget this" did not forget.**
    ///
    /// `deactivate_fact` sets `is_active = 0`, and the tier table honours it.
    /// The index copy has no `is_active` column, was never updated, and is
    /// still selected by `LexicalBackend::search` — so a fact the user asked to
    /// remove kept being injected into the system prompt on every turn, for
    /// ever, while every surface reported the removal as done.
    ///
    /// A deletion path that silently retains the data is worse than no deletion
    /// path, because the user is told it worked.
    #[test]
    fn deactivating_a_fact_removes_it_from_retrieval() {
        let c = laddered();
        add_fact(&c, "s1", "the staging password is in the ops vault");
        assert_eq!(indexed(&c, "s1"), 1, "the fixture must start indexed");

        c.execute(
            "UPDATE semantic_memories SET is_active = 0, updated_at = 't2' WHERE id = 's1'",
            [],
        )
        .unwrap();

        assert_eq!(
            indexed(&c, "s1"),
            0,
            "a deactivated fact is STILL retrievable — the user asked to forget it"
        );
        let hits: i64 = c
            .query_row(
                "SELECT count(*) FROM memories_fts WHERE memories_fts MATCH 'vault'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            hits, 0,
            "it is gone from `memories` but still in the search index"
        );
    }

    /// The other direction, or the trigger above is just breaking recall.
    #[test]
    fn reactivating_a_fact_brings_it_back() {
        let c = laddered();
        add_fact(&c, "s1", "the api gateway listens on port 8080");
        c.execute(
            "UPDATE semantic_memories SET is_active = 0 WHERE id = 's1'",
            [],
        )
        .unwrap();
        assert_eq!(indexed(&c, "s1"), 0);

        c.execute(
            "UPDATE semantic_memories SET is_active = 1 WHERE id = 's1'",
            [],
        )
        .unwrap();
        assert_eq!(
            indexed(&c, "s1"),
            1,
            "reactivation left the fact unretrievable — the UPDATE had no row to hit"
        );
    }

    #[test]
    fn an_edited_fact_does_not_serve_its_old_text() {
        let c = laddered();
        add_fact(&c, "s1", "the deploy key rotates every quarter");
        c.execute(
            "UPDATE semantic_memories SET fact_text = 'the deploy key rotates every fortnight',
                    updated_at = 't2' WHERE id = 's1'",
            [],
        )
        .unwrap();

        let content: String = c
            .query_row(
                "SELECT content FROM memories WHERE ref_id = 's1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            content, "the deploy key rotates every fortnight",
            "the index served the PRE-EDIT sentence; contradiction resolution \
             would be updating a column nothing reads back"
        );
    }

    #[test]
    fn deleting_a_row_from_any_tier_removes_its_index_entry() {
        let c = laddered();
        add_fact(&c, "s1", "a semantic fact worth deleting later");
        c.execute(
            "INSERT INTO episodic_memories(id,session_id,turn_index,role,content,created_at)
             VALUES ('e1','s1',1,'user','an episodic turn worth deleting later','t1')",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO procedural_memories(id,topic,title,content,tags,created_at,updated_at)
             VALUES ('p1','t','A topic','worth deleting later','[]','t1','t1')",
            [],
        )
        .unwrap();
        for r in ["s1", "e1", "p1"] {
            assert_eq!(indexed(&c, r), 1, "{r} was not indexed to begin with");
        }

        c.execute("DELETE FROM semantic_memories WHERE id = 's1'", [])
            .unwrap();
        c.execute("DELETE FROM episodic_memories WHERE id = 'e1'", [])
            .unwrap();
        c.execute("DELETE FROM procedural_memories WHERE id = 'p1'", [])
            .unwrap();

        for r in ["s1", "e1", "p1"] {
            assert_eq!(indexed(&c, r), 0, "{r} outlived its own row in the index");
        }
    }

    /// The upgrade must not resurrect what was already forgotten.
    ///
    /// A fact deactivated while the database sat at v2 — the window in which
    /// `deactivate_fact` was a no-op against retrieval — is still `is_active=0`
    /// in the tier table when v3 lands. If the catch-up only inserted, the
    /// upgrade would re-serve exactly the memories the user had removed.
    #[test]
    fn the_v3_catch_up_does_not_resurrect_a_deactivated_fact() {
        crate::memory::vector::register_once();
        let c = Connection::open_in_memory().unwrap();
        crate::db::migrate::run_ladder(&c, "memory.db(test)", &MemoryDb::LADDER[..2]).unwrap();
        add_fact(&c, "s1", "a fact the user asked to forget while at v2");
        // v2's own backfill ran before this row existed, so put it in the index
        // the way the v2 backfill would have, then deactivate it.
        c.execute(
            "INSERT INTO memories (id,tier,scope,scope_ref,scope_key,content,source,
                                   importance,confidence,ref_id,created_at,updated_at)
             VALUES ('sem-s1','semantic','project','/proj','project:/proj',
                     'a fact the user asked to forget while at v2','user',
                     0.5,0.9,'s1','t1','t1')",
            [],
        )
        .unwrap();
        c.execute(
            "UPDATE semantic_memories SET is_active = 0 WHERE id = 's1'",
            [],
        )
        .unwrap();
        assert_eq!(
            indexed(&c, "s1"),
            1,
            "the v2 window is the point of this fixture"
        );

        crate::db::migrate::run_ladder(&c, "memory.db(test)", MemoryDb::LADDER).unwrap();
        assert_eq!(
            indexed(&c, "s1"),
            0,
            "the upgrade re-served a memory the user had already forgotten"
        );
    }

    #[test]
    fn v3_catches_up_rows_written_between_v2_and_v3() {
        // A database that ran v2, accumulated turns, then upgraded must not
        // leave that window unindexed — `L-C13`'s shape one rung along.
        crate::memory::vector::register_once();
        let c = Connection::open_in_memory().unwrap();
        crate::db::migrate::run_ladder(&c, "memory.db(test)", &MemoryDb::LADDER[..2]).unwrap();
        // Deliberately stopped at v2 — this test is about the window BETWEEN
        // rungs, so here the literal is the point rather than a hazard.
        assert_eq!(crate::db::migrate::user_version(&c).unwrap(), 2);

        c.execute(
            "INSERT INTO episodic_memories(id,session_id,turn_index,role,content,created_at)
             VALUES ('gap','s1',1,'user','written in the gap between rungs','t1')",
            [],
        )
        .unwrap();
        let before: i64 = c
            .query_row(
                "SELECT count(*) FROM memories WHERE ref_id='gap'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(before, 0, "the fixture must actually be unindexed at v2");

        crate::db::migrate::run_ladder(&c, "memory.db(test)", MemoryDb::LADDER).unwrap();
        let after: i64 = c
            .query_row(
                "SELECT count(*) FROM memories WHERE ref_id='gap'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(after, 1, "v3 did not catch up rows written under v2");
    }
}
