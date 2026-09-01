# V5 — P1 Groups C & D — Adversarial Verification: Schema Duplication, Migrations, Redaction

**Verifier stance:** REFUTE by default. Ambiguous evidence = REFUTED.
**Mode:** READ-ONLY on repo and all databases. No source file edited, no test suite run, no build,
no git write. Databases opened only via `sqlite3` URI `?mode=ro`.
**Date:** 2026-09-01.

---

## Verdict table

| Claim | Verdict | One line |
|---|---|---|
| **C1** schema duplication (`tasks`, `workers`, `plans`, `plan_steps`; "two incompatible designs") | **CONFIRMED** (understated for `plans`/`workers`; **overstated for `tasks`**) | Both crates compute the same `~/.autodev` and race on `CREATE TABLE IF NOT EXISTS`; `plans` differs in PK *type* (INTEGER vs TEXT) and scope column name, `workers` differs by `last_seen`/`notes` — **and that one has already fired: `airsctl`'s worker list/get/heartbeat are broken on this machine today.** `tasks` is drifted but compatible. |
| **C2** "give EVERY database `user_version` + a ladder" | **CONFIRMED** | Exactly **1 of 11** databases has a ladder (the graph DB, live `user_version = 5`); every other live DB reports **0**, and the whole Rust tree contains six `user_version` references, all in one file. |
| **C3** graph v5 migration is broken | **CONFIRMED on syntax, REFUTED on consequence** | `ADD COLUMN IF NOT EXISTS` is a syntax error (proven empirically) — but all six statements are `let _ =`, so it **swallows every failure, stamps `user_version = 5`, commits, and logs "✅ complete"**. Inert-but-green, not aborting. Harmless today only because v1's `CREATE_NODES` already has the columns. |
| **C4** two data roots | **PARTIALLY CONFIRMED / OVERSTATED** | Two roots exist and `sessions`/`messages` do live in `~/.airs/autodev.db` (**1 session, 14 messages**) — but `~/.airs/plugins/` and `~/.airs/fleet_secret` **do not exist**; the "plugin/fleet-secret tree" is code-only, and nothing has been *written* to `~/.airs` since 1 June 2026. **Scope is wider than the claim in one direction:** `~/.airs/config.toml` (airsctl's config) is a fourth path, and `src-shared/src/config.rs:14` **recreates `~/.airs/` on every call**, so deleting the root does not retire it. |
| **C5** `app_settings` has three independent creators | **CONFIRMED** | Exactly three, in three modules. They agree on all three columns; they **disagree on the `updated_at` trigger** (only `github_mcp_commands.rs` creates it). Whichever runs first wins — here creator 3 did. |
| **C6** `hourly_costs` queried but never created | **CONFIRMED** (both halves, with live proof) | One source hit repo-wide (`analytics/queries.rs:35`); no `CREATE TABLE` anywhere; the live `analytics.db` has no such table. `.unwrap_or(-1)` masks it forever. |
| **C7** timestamp bug, "3+ known sites" | **PARTIALLY CONFIRMED / MISDIRECTED** | 7 call sites, but **only one** reads a `datetime('now')` column: `orphan/mod.rs:183`. It fails **100% of the time** and `.ok()?` silently means "not an orphan" — the orphan TTL fallback is dead code that has never fired. The 5 analytics sites read Rust-written RFC3339 and work today. Also: chrono *does* accept the space; the **missing timezone offset** is what breaks it. |
| **C8** redaction never ported | **CONFIRMED on substance, OVERSTATED on count** | Zero redaction/scrubbing/secret-masking anywhere in `src-tauri`/`src-shared`/`src-cli`. `SENSITIVE_KEYS` has **22** entries, not 20 — and the Python side has a **second** redactor (PII) the claim doesn't mention. |
| **C9** no backup helper exists | **PARTIALLY CONFIRMED / OVERSTATED as written** | Two helpers *do* exist (`backup_recipe_file` — real, YAML-only; `create_restore_point` — a size/mtime manifest that is never read and cannot restore). **No SQLite backup capability exists at all** (no `VACUUM INTO`, no `Connection::backup`, rusqlite's `backup` feature not enabled) and **not one destructive DB path is guarded.** The operative claim stands. |

**Three most important findings**
1. **P1-11 as written is already impossible.** Fixing `migration_v5` in place changes nothing: every existing graph DB is stamped `user_version = 5` and `migrations.rs:37` gates on `if current < 5`, so the corrected code will never run. The repair must be a **new `migration_v6`**.
2. **P1-19 (the backup helper) is listed last, but P1-11 and P1-14 both say "⚠ back up first".** The helper does not exist until after the steps that require it — and there is no SQLite backup capability in the tree at all. P1-19 must move to the front of P1-C.
3. **C1 is not latent — it has already fired.** `src-shared/src/db/workers.rs` selects `last_seen`/`notes` in six places and updates `last_seen` in a seventh; the live `~/.autodev/workers.db` (written by `src-tauri`) has neither column. Every `airsctl` worker command errors today.

---

## Per-claim detail

### Live database census (read-only, `PRAGMA user_version` + `sqlite_master`)

Method: Python `sqlite3.connect("file:...?mode=ro", uri=True)` — read-only URI, no writes, no
vacuum, no migration. Script lived in the scratchpad, not the repo.

| DB file | `user_version` | tables (rows) |
|---|---|---|
| `~/.autodev/analytics.db` | **0** | api_failures(0), daily_costs(0), system_events(62), token_usage(259), worker_stats(10) |
| `~/.autodev/annotations.db` | **0** | annotations(8) |
| `~/.autodev/config.db` | **0** | app_settings(4), autodev_instructions(1), checkpoints(108), hook_logs(0), hooks(0), model_configs(2) |
| `~/.autodev/memory.db` | **0** | episodic_memories(39), fact_history(0), procedural_memories(0), semantic_memories(0) |
| `~/.autodev/tasks.db` | **0** | routing_history(220), tasks(0), worker_messages(0), worker_requests(0) |
| `~/.autodev/workers.db` | **0** | workers(7) |
| `~/.autodev/graphs/d0ef18f6ad8b.db` | **5** | analysis_cache(0), communities(0), edges(0), flows(0), nodes(0), nodes_fts*(fts5 shadow), repos(0), snapshots(0) |
| `~/.airs/autodev.db` | **0** | messages(14), sessions(1) |
| `~/.autodev/plans.db` | **DOES NOT EXIST** | — |

`~/.autodev/graph_cache/` holds ~90 `*.json` files, not databases (each one fails to open as
SQLite). It is a JSON blob cache, not a DB root.

Filesystem mtimes at time of audit:
- `~/.airs/autodev.db` = May 30 16:17, `autodev.db-wal` = Jun 1 15:19 (4.1 MB), `autodev.db-shm` = **Sep 1 10:42**
- `~/.autodev/workers.db` = **Sep 1 10:42**; analytics/config/tasks.db = Jun 1 15:31

---

### C1 — schema duplication between `src-shared` and `src-tauri`

**VERDICT: CONFIRMED, and the `plans` case is worse than "incompatible" — it is a latent
hard-failure collision. But it has never fired on this machine (no `plans.db` exists).**

Both crates compute the *same* directory:

- `src-shared/src/db/mod.rs:10-18` — `pub fn autodev_dir()` → `USERPROFILE`/`HOME` + `.autodev`
- `src-tauri/src/db/mod.rs:21-29` — `pub fn autodev_dir()` → `USERPROFILE`/`HOME` + `.autodev`

Byte-for-byte the same resolution logic, differing only in the error type. So
`src-shared::db::plans::PlansDb::open()` (`src-shared/src/db/plans.rs:38`) and
`src-tauri::db::plans::PlansDb::open()` (`src-tauri/src/db/plans.rs:205`) open **the same file**,
`~/.autodev/plans.db`. Same story for `tasks.db` (`src-shared/src/db/tasks.rs:156` vs
`src-tauri/src/db/tasks.rs:312`) and `workers.db` (`src-shared/src/db/workers.rs:37` vs
`src-tauri/src/db/workers.rs:117`).

Every one of these migrations is `CREATE TABLE IF NOT EXISTS`. **Whichever process opens the file
first wins the schema; the loser silently accepts the winner's table and then fails at
INSERT/SELECT time.** There is no `user_version` check, no schema fingerprint, no reconciliation.

`src-shared/src/db/plans.rs:1` even says so in a comment:
`// Plans database — mirrors the schema in src-tauri/src/db/plans.rs.`
That comment is **false** — see below. A comment promising one behaviour while the code does
another is itself a bug.

#### `plans` — genuinely incompatible (different primary-key TYPE)

`src-shared/src/db/plans.rs:48-57`:
```sql
CREATE TABLE IF NOT EXISTS plans (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    title       TEXT    NOT NULL,
    description TEXT,
    status      TEXT    NOT NULL DEFAULT 'draft',
    scope       TEXT    NOT NULL DEFAULT 'personal',
    created_by  TEXT    NOT NULL DEFAULT 'airsctl',
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);
```

`src-tauri/src/db/plans.rs:218-232`:
```sql
CREATE TABLE IF NOT EXISTS plans (
    id              TEXT PRIMARY KEY,
    session_id      TEXT,
    title           TEXT    NOT NULL,
    description     TEXT,
    status          TEXT    NOT NULL DEFAULT 'draft',
    created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    assigned_agent  TEXT,
    created_by      TEXT,
    agent_scope     TEXT    NOT NULL DEFAULT 'personal',
    parent_plan_id  TEXT,
    task_id         INTEGER,
    FOREIGN KEY (parent_plan_id) REFERENCES plans(id) ON DELETE CASCADE
);
```

Differences that make them incompatible, not merely duplicated:

| | src-shared | src-tauri |
|---|---|---|
| `id` | `INTEGER PRIMARY KEY AUTOINCREMENT` (rowid alias, DB-generated) | `TEXT PRIMARY KEY` (UUIDv4 generated in Rust, `src-tauri/src/db/plans.rs:292`) |
| scope column | `scope` | `agent_scope` |
| `created_by` | `NOT NULL DEFAULT 'airsctl'` | nullable, no SQL default (`unwrap_or("pa")` in Rust, `:295`) |
| extra cols | — | `session_id`, `assigned_agent`, `parent_plan_id`, `task_id` + self-FK |
| index `idx_plans_status` | `ON plans(status)` | `ON plans(status)` — same name, same target, collides harmlessly |

Concrete failure if src-tauri creates the table first and `airsctl` then calls
`PlansDb::create(...)` (`src-shared/src/db/plans.rs:81-84`):
`INSERT INTO plans (title, description, scope, created_by)` → **`no such column: scope`**, a hard
rusqlite error propagated straight out. In the reverse order, src-tauri's
`INSERT INTO plans (id, session_id, …, agent_scope, parent_plan_id, task_id)`
(`src-tauri/src/db/plans.rs:297-298`) fails the same way — and additionally its `id` is a UUID
string, which the src-shared table would coerce into an INTEGER-affinity rowid column. Neither
side has a fallback path or a schema check.

#### `plan_steps` vs `plan_items` — not a name collision, a *modelling* fork

src-shared calls the child table `plan_steps` (`src-shared/src/db/plans.rs:58-67`, flat, INTEGER
`plan_id`); src-tauri calls it `plan_items` (`src-tauri/src/db/plans.rs:240-258`, TEXT ids,
self-referencing `parent_id` for a tree, plus `agent_type`, `estimated_tools`, `assigned_worker`,
`worktree_branch`, `pr_url`, `result_summary`). src-tauri additionally creates
`plan_item_dependencies` (`:264`) and `plan_executions` (`:272`), which src-shared knows nothing
about. So `plans.db` really does hold **two different designs** — a flat 2-table CLI model and a
hierarchical 5-table app model — sharing one filename and one table name (`plans`).
The claim's phrase "two incompatible designs" is accurate and, if anything, understated.

One mitigating fact: because the two child tables have *different names*, they coexist. Only the
`plans` table itself actually collides. That single collision is enough to break both writers.

#### `tasks` — duplicated and drifted, but NOT incompatible

`src-shared/src/db/tasks.rs:167-180` and `src-tauri/src/db/tasks.rs:325-340` agree on all 12
src-shared columns exactly (same names, types, defaults, `INTEGER PRIMARY KEY AUTOINCREMENT`).
src-tauri adds `rejection_reason TEXT` and `retry_count INTEGER NOT NULL DEFAULT 0` inline, then
adds **18 more** columns by best-effort `ALTER TABLE` (`src-tauri/src/db/tasks.rs:391-458`):
`claimed_by_pid`, `claim_host`, `diff_analysis`, seven `budget_*`, five phase columns
(`phase`, `phase_order`, `milestone`, `epic_id`, `blocked_by`), `under_review_by`,
`task_qc_cost_usd`. All are nullable or defaulted, so src-shared's
`INSERT INTO tasks (title, description, priority, task_type, acceptance_criteria, created_by)`
(`src-shared/src/db/tasks.rs:194-195`) still succeeds against the src-tauri table.
Conversely, if `airsctl` creates `tasks.db` first, src-tauri's ALTERs at `:391-396` add the two
missing columns — it self-heals.
**This one is duplication + drift, not incompatibility.** The claim overstates `tasks`.

Note also: both crates create a trigger named `tasks_updated_at`
(`src-shared/src/db/tasks.rs:183-187`, `src-tauri/src/db/tasks.rs:346-350`) with the same body.
`IF NOT EXISTS` makes that a silent no-op, not an error — but it is one more copy to keep in sync.

#### `workers` — duplicated, and broken in the direction that actually happened

`src-shared/src/db/workers.rs:47-59`:
```sql
CREATE TABLE IF NOT EXISTS workers (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    slug        TEXT    NOT NULL UNIQUE,
    name        TEXT    NOT NULL,
    worker_type TEXT    NOT NULL DEFAULT 'custom',
    status      TEXT    NOT NULL DEFAULT 'created',
    model       TEXT    NOT NULL DEFAULT 'claude-haiku-4-5',
    last_seen   TEXT,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    description TEXT,
    notes       TEXT
);
```

`src-tauri/src/db/workers.rs:131-145`:
```sql
CREATE TABLE IF NOT EXISTS workers (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    name              TEXT NOT NULL,
    slug              TEXT NOT NULL UNIQUE,
    worker_type       TEXT NOT NULL DEFAULT 'custom',
    status            TEXT NOT NULL DEFAULT 'created',
    model             TEXT NOT NULL DEFAULT 'claude-sonnet-4-5',
    primary_repos     TEXT NOT NULL DEFAULT '[]',
    polling_interval  INTEGER NOT NULL DEFAULT 120,
    recipe_id         TEXT,
    is_system_defined INTEGER NOT NULL DEFAULT 0,
    description       TEXT,
    created_at        DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at        DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

Differences:
- src-shared has **`last_seen`** and **`notes`**; src-tauri has neither, and never ALTERs them in.
- src-tauri has `primary_repos`, `polling_interval`, `recipe_id`, `is_system_defined`; src-shared
  has none of them — but all are defaulted, so src-shared INSERTs still work.
- **default `model` differs**: `claude-haiku-4-5` (shared) vs `claude-sonnet-4-5` (tauri). A worker
  created through the CLI vs the app gets a different default model *for the same operation*.
- `created_at`/`updated_at`: `TEXT NOT NULL DEFAULT (datetime('now'))` vs
  `DATETIME DEFAULT CURRENT_TIMESTAMP`. Both render `YYYY-MM-DD HH:MM:SS`; the declared type and
  the NOT NULL differ. Cosmetic in practice.
- Trigger names differ — src-tauri's is `update_workers_timestamp`
  (`src-tauri/src/db/workers.rs:148-152`); src-shared defines none for `workers`.
- src-tauri also runs `migrate_model_names()` (`:168-179`) and `seed_defaults()` (`:183`) on every
  open; src-shared does neither.

**This one has already fired.** The live `~/.autodev/workers.db` was written by src-tauri (its
`workers` table carries `primary_repos`/`polling_interval`/`recipe_id`/`is_system_defined` and
has no `last_seen`/`notes` — see the live-schema dump below), so any `airsctl` code path that
SELECTs or UPDATEs `last_seen` or `notes` on this machine errors with `no such column`.

#### PROOF that the `workers` collision has already fired on this machine

Live `~/.autodev/workers.db` (read-only `PRAGMA table_info(workers)` + `sqlite_master`):

```sql
CREATE TABLE workers (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                name              TEXT NOT NULL,
                slug              TEXT NOT NULL UNIQUE,
                worker_type       TEXT NOT NULL DEFAULT 'custom',
                status            TEXT NOT NULL DEFAULT 'created',
                model             TEXT NOT NULL DEFAULT 'claude-sonnet-4-5',
                primary_repos     TEXT NOT NULL DEFAULT '[]',
                polling_interval  INTEGER NOT NULL DEFAULT 120,
                recipe_id         TEXT,
                is_system_defined INTEGER NOT NULL DEFAULT 0,
                description       TEXT,
                created_at        DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at        DATETIME DEFAULT CURRENT_TIMESTAMP
            );
CREATE TRIGGER update_workers_timestamp
                AFTER UPDATE ON workers
                BEGIN
                    UPDATE workers SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
                END;
```

13 columns. **No `last_seen`. No `notes`.** This is verbatim the src-tauri definition; src-tauri
won the race and the file has been that way since it was created. 7 seeded rows
(supervisor, qc-agent, worker-backend/-frontend/-platform/-mcp/-cleanup).

Now `src-shared/src/db/workers.rs` — every read path names both missing columns:

- `:85`  `SELECT id,slug,name,worker_type,status,model,last_seen,created_at,updated_at,description,notes` (`get`)
- `:93`  same column list (`get_by_slug`)
- `:106`, `:108`, `:110`, `:112` — all four `list()` variants, same column list
- `:143` `"UPDATE workers SET last_seen=datetime('now'), status='active', updated_at=datetime('now') WHERE slug=?1 AND status!='deleted'"` (`heartbeat`)
- `:173` `last_seen: row.get(6)?`, `:177` `notes: row.get(10)?` in `row_to_worker`

Against the live file every one of these raises rusqlite `SqliteFailure … no such column: last_seen`.
**So the `airsctl` worker list / get / heartbeat commands are broken today on the developer's own
machine, and this is not hypothetical drift — it is a live outage caused precisely by the
duplication C1 describes.** `create()` at `:68` happens to survive (it only INSERTs
slug/name/worker_type/model/description) but its follow-up `self.get(id)` at `:79` then fails, so
even create is broken end-to-end.

---

### C2 — migration state: does every DB have `PRAGMA user_version` and a numbered ladder?

**VERDICT: CONFIRMED (the premise is true). Exactly ONE database in the whole product has a
`user_version` ladder — the graph DB. Every other database has `user_version = 0` and no ladder.**

Exhaustive `user_version` search over the whole Rust tree:

```
src-tauri/src/graph/migrations.rs:8:    let version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
src-tauri/src/graph/migrations.rs:63:    tx.execute("PRAGMA user_version = 1", [])?;
src-tauri/src/graph/migrations.rs:75:    tx.execute("PRAGMA user_version = 2", [])?;
src-tauri/src/graph/migrations.rs:87:    tx.execute("PRAGMA user_version = 3", [])?;
src-tauri/src/graph/migrations.rs:104:    tx.execute("PRAGMA user_version = 4", [])?;
src-tauri/src/graph/migrations.rs:141:    tx.execute("PRAGMA user_version = 5", [])?;
```

That is the complete result set — six hits, all in one file. No `application_id` anywhere either
(live `application_id = 0` on every DB checked).

Database inventory, module → file → versioning strategy:

| DB file | opened by | ladder? | live `user_version` |
|---|---|---|---|
| `~/.autodev/analytics.db` | `src-tauri/src/analytics/db.rs:23`; `src-shared/src/db/analytics.rs:52` (read-only consumer) | no — `CREATE TABLE IF NOT EXISTS` only | 0 |
| `~/.autodev/annotations.db` | `src-tauri/src/db/annotations.rs:51` | no | 0 |
| `~/.autodev/config.db` | **5 different modules** — `db/autodev_instructions.rs:34`, `db/model_configs.rs:44`, `hooks/db.rs:18`, `ide/workspace.rs:31`, `commands/workspace_commands.rs:19`, `commands/github_mcp_commands.rs:26`, `git/checkpoint.rs:30` | no | 0 |
| `~/.autodev/checkpoints.db` | `src-tauri/src/db/checkpoints.rs:67` | no | **file does not exist** |
| `~/.autodev/memory.db` | `src-tauri/src/memory/db.rs:120` | no | 0 |
| `~/.autodev/plans.db` | `src-tauri/src/db/plans.rs:205`; `src-shared/src/db/plans.rs:38` | no | **file does not exist** |
| `~/.autodev/tasks.db` | `src-tauri/src/db/tasks.rs:312`; `src-shared/src/db/tasks.rs:156` | no — 20 best-effort `ALTER TABLE` calls with results discarded | 0 |
| `~/.autodev/workers.db` | `src-tauri/src/db/workers.rs:117`; `src-shared/src/db/workers.rs:37` | no | 0 |
| `~/.autodev/graphs/<hash>.db` | `src-tauri/src/commands/graph_commands.rs:82` → `graph/store.rs:75` | **YES — v1…v5** | **5** |
| `<repo>/.autodev/graph.db` | `src-tauri/src/graph/registry.rs:65`, `:129` → same `GraphStore::open` | YES (same ladder) | not present in `~` |
| `~/.airs/autodev.db` | `src-tauri/src/session/storage.rs:327` | no — `CREATE TABLE IF NOT EXISTS` + swallowed `ALTER TABLE`s | 0 |

So "give EVERY database a `user_version` and a numbered ladder" is 1 of 11 done. The other ten use
one of two ad-hoc idioms:

1. `CREATE TABLE IF NOT EXISTS` — which silently accepts *any* pre-existing table with that name,
   including one written by a different crate with a different shape (that is exactly the C1 bug);
2. `let _ = conn.execute_batch("ALTER TABLE … ADD COLUMN …")` — result discarded, so a genuine
   failure (disk full, locked DB, syntax error) is indistinguishable from the expected
   "duplicate column name" no-op. 20 such calls in `src-tauri/src/db/tasks.rs:391-461` alone.

Neither idiom can express a *destructive* or *transforming* migration (rename, type change,
backfill, split), which is why the `plans` fork in C1 has no path out.

**A second-order finding the claim does not mention:** `src-tauri/src/db/checkpoints.rs:67` opens
`~/.autodev/checkpoints.db`, but that file **does not exist** on the developer's machine, while
`src-tauri/src/git/checkpoint.rs:30` opens `~/.autodev/config.db` and its `checkpoints` table holds
**108 live rows**. So there are two checkpoint stores in two different files, and only one of them
has ever been written. See the P1-16 note in Ordering hazards.

---

### C3 — "the graph v5 migration is broken: `ADD COLUMN IF NOT EXISTS` is not valid SQLite"

**VERDICT: CONFIRMED on the syntax, but the consequence is the OPPOSITE of "the migration
aborts". Every failure is explicitly swallowed by `let _ =`, the migration then stamps
`user_version = 5` and commits. It is INERT-BUT-GREEN: it reports success while doing nothing.**

Exact statements, `src-tauri/src/graph/migrations.rs:110-145`:

```rust
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
```

Six statements, at `migrations.rs:115-116`, `:117-118`, `:119-121`, `:122-123`, `:126-127`,
`:128-129`. All six are prefixed `let _ =`.

**Proof the syntax is invalid** (run against an in-memory scratch DB, no live data touched):

```
python sqlite lib version: 3.50.4
ERR  : ALTER TABLE nodes ADD COLUMN IF NOT EXISTS agent_author TEXT DEFAULT NULL
       -> OperationalError near "EXISTS": syntax error
ERR  : ALTER TABLE nodes ADD COLUMN IF NOT EXISTS worker_locked BOOLEAN DEFAULT 0
       -> OperationalError near "EXISTS": syntax error
cols after: ['id', 'name']
```

SQLite has never supported `IF NOT EXISTS` on `ALTER TABLE … ADD COLUMN` in any version, and the
app bundles its own: `rusqlite = { version = "0.31", features = ["bundled"] }`
(`src-tauri/Cargo.toml:42`, `src-shared/Cargo.toml:10`) → `libsqlite3-sys 0.28.0` in `Cargo.lock`.
The grammar is identical. So this fails on every machine, every run, always.

**Has it ever run successfully? Yes — vacuously.** The live graph DB
`~/.autodev/graphs/d0ef18f6ad8b.db` reports `user_version = 5`. Lines `:132-141` are NOT prefixed
`let _`: the two `CREATE INDEX IF NOT EXISTS` and `PRAGMA user_version = 5` use `?`, they all
succeed, and `tx.commit()` at `:142` lands. So the function returns `Ok(())`, the caller
`GraphStore::open` (`src-tauri/src/graph/store.rs:93`) proceeds, and
`tracing::info!("[Migrations] ✅ migration_v5 complete")` (`migrations.rs:40`) prints a green
checkmark for a migration that added zero columns.

**Why nothing has broken yet:** the four "v5" node columns and two "v5" edge columns are *already
in the v1 base DDL*:

- `src-tauri/src/graph/schema.rs:57-60` — `agent_author TEXT, task_id INTEGER, worker_locked BOOLEAN DEFAULT 0, worktree TEXT` inside `CREATE_NODES`
- `src-tauri/src/graph/schema.rs:78-79` — `agent_author TEXT, task_id INTEGER` inside `CREATE_EDGES`

and the two indexes v5 "creates" are already in `ALL_INDEXES`
(`src-tauri/src/graph/schema.rs:159-160`). Verified against the live DB: its `nodes` table does
carry all four columns and its `edges` table both. **So migration_v5 is dead code that happens to
be harmless on any DB born after those columns entered `CREATE_NODES`.**

**The actual latent bug** — and it is worse than the claim states: a graph DB created *before*
those columns were added to `CREATE_NODES` would have `CREATE TABLE IF NOT EXISTS` no-op in
`migration_v1` (`migrations.rs:51`), the six ALTERs would silently fail, `user_version` would be
stamped 5, and the DB would then be **permanently mislabelled as v5 while missing six columns**.
Every subsequent read of `agent_author`/`worker_locked`/`worktree` would raise
`no such column`, and because `user_version` is already 5, `run_migrations` (`:37`) will never
re-attempt the fix. There is no repair path. Whether any such DB exists in the wild I could not
determine — the only graph DB on this machine is a fresh one and is intact.

Same class of bug in `migration_v2` (`:69-79`) and `migration_v3` (`:81-91`): both are pure no-ops
whose comments admit it (`// v2: communities table already in v1, just ensure it exists`,
`// v3: add flows table (already in schema)`). And `migration_v4` (`:93-108`) does
`DROP TABLE IF EXISTS nodes_fts` then rebuilds the FTS5 index — **that one is genuinely
destructive** and is discussed in the risk ranking.

---

### C4 — two data roots (`~/.airs` and `~/.autodev`)

**VERDICT: PARTIALLY CONFIRMED / OVERSTATED. Two roots do exist and `sessions`/`messages` really
do live in `~/.airs/autodev.db`. But the "plugin/fleet-secret tree" is code-only — neither
`~/.airs/plugins/` nor `~/.airs/fleet_secret` exists on this machine, so there is nothing to move.
And `~/.airs/autodev.db` has not been *written* since 1 June 2026.**

Complete list of `.airs` references in the Rust tree (`rg '\.airs\b' src-tauri src-shared src-cli`):

| `path:line` | what it opens |
|---|---|
| `src-tauri/src/session/storage.rs:327` | `Ok(home.join(".airs").join("autodev.db"))` — **the only DB** |
| `src-tauri/src/session/storage.rs:2` | comment `// Database path: ~/.airs/autodev.db` |
| `src-tauri/src/db/mod.rs:8` | comment: `// Session data lives in the existing ~/.airs/autodev.db` |
| `src-tauri/src/memory/migration.rs:2` | comment — reads sessions from `~/.airs/autodev.db`, backfills `~/.autodev/memory.db` |
| `src-tauri/src/plugin/token.rs:107-109` | `PathBuf::from(home).join(".airs").join("fleet_secret")` |
| `src-tauri/src/plugin/registry.rs:17` | `PathBuf::from(home).join(".airs").join("plugins")` |
| `src-tauri/src/updater/persistence.rs:41` | `("Plugins dir", home.join(".airs").join("plugins"), false)` |
| `src-tauri/src/updater/persistence.rs:44` | `home.join(".airs").join("fleet_secret")` |
| `src-tauri/src/agent/builtin_tools/list_directory.rs:104` | `".airs"` in an ignore list |
| `src-tauri/src/agent/context.rs:41` | comment `// LEGACY: ~/.airs/CLAUDE.md — will be replaced with SQLite query` |

~~Nothing in `src/` (the TypeScript front-end), `src-shared`, or `src-cli` touches `.airs` at all.~~ **← RETRACTED, see the correction at the end of this section.**

**Live contents of `~/.airs/`** — the directory holds exactly three files and no subdirectories:

```
-rw-r--r--  409600  2026-05-30 16:17:52  autodev.db
-rw-r--r--   32768  2026-09-01 11:37:22  autodev.db-shm
-rw-r--r-- 4124152  2026-06-01 15:19:51  autodev.db-wal
```

`~/.airs/autodev.db` (`user_version = 0`) contains exactly two tables:

```sql
CREATE TABLE sessions (
                id          TEXT PRIMARY KEY,
                title       TEXT NOT NULL,
                working_dir TEXT NOT NULL,
                provider    TEXT NOT NULL,
                model       TEXT NOT NULL,
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL
            , pa_mode TEXT NOT NULL DEFAULT 'fast_development');

CREATE TABLE messages (
                id           TEXT PRIMARY KEY,
                session_id   TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                role         TEXT NOT NULL,
                content      TEXT NOT NULL,
                tool_calls   TEXT NOT NULL DEFAULT '[]',
                tool_results TEXT NOT NULL DEFAULT '[]',
                tokens_in    INTEGER NOT NULL DEFAULT 0,
                tokens_out   INTEGER NOT NULL DEFAULT 0,
                cost_usd     REAL NOT NULL DEFAULT 0.0,
                created_at   TEXT NOT NULL
            , tool_name TEXT, tool_input TEXT, tool_call_id TEXT, is_error INTEGER NOT NULL DEFAULT 0);
```

**Row counts: `sessions` = 1, `messages` = 14.** That is the entire payload of the second data
root. (Note the live `messages` table carries `tool_calls`/`tool_results` columns that the current
`CREATE TABLE messages` at `src-tauri/src/session/storage.rs:68-81` does **not** define — this file
predates the "V2 schema" branch. Because the table already exists, the `if !table_exists` branch at
`:64` never runs; only the four swallowed `ALTER TABLE`s at `:89-101` did, which is why
`tool_name`/`tool_input`/`tool_call_id`/`is_error` are appended at the end. Another instance of the
same no-ladder pattern from C2.)

**"Is anything in `~/.airs/` still being written today?"**
- `autodev.db` mtime = 2026-05-30, `autodev.db-wal` mtime = 2026-06-01 → **no session or message
  has been written since 1 June 2026.** The 4.1 MB uncheckpointed WAL is the residue of that
  session.
- `autodev.db-shm` was 2026-09-01 10:42 *before I touched anything*, which proves an AutoDev
  process **opened** the DB read-write today (`SessionManager::new()` →
  `SessionStorage::open()` → `migrate()`, `src-tauri/src/session/manager.rs:15`). It opened it and
  wrote nothing.
- **Disclosure:** the `-shm` mtime is now 2026-09-01 11:37:22 because my own read-only inspection
  updated the shared-memory read-mark block. `autodev.db` (2026-05-30 16:17) and `autodev.db-wal`
  (2026-06-01 15:19) are byte-unchanged. No data was written.
- `~/.airs/plugins/` — **does not exist**. `~/.airs/fleet_secret` — **does not exist**.
  `src-tauri/src/plugin/token.rs:112-133` `load_or_create_fleet_secret()` would create it on first
  use; it never has been. `~/.airs/CLAUDE.md` — does not exist either.

**So the consolidation P1-14 moves 1 session + 14 messages and two paths that hold nothing.**
The claim that it "moves sessions/messages plus a plugin/fleet-secret tree" describes the *code*
correctly and the *data* incorrectly. Corrected scope for planning: one 400 KB DB with a 4 MB WAL,
15 rows total, and two never-materialised path constants.

**Related defect found in passing** — `src-tauri/src/updater/persistence.rs:29-47`
`verify_data_integrity()` is the pre-update data-integrity gate. Its checklist is:

```rust
    let checks_to_run = vec![
        ("Config DB", autodev_dir.join("config.db"), true),
        ("Analytics DB", autodev_dir.join("analytics.db"), false),
        ("Sessions dir", autodev_dir.join("sessions"), false),
        ("Plans dir", autodev_dir.join("plans"), false),
        ("Skills dir", autodev_dir.join("skills"), false),
        ("Hooks dir", autodev_dir.join("hooks"), false),
        ("Updates dir", autodev_dir.join("updates"), false),
        ("Plugins dir", home.join(".airs").join("plugins"), false),
        ("Fleet secret", home.join(".airs").join("fleet_secret"), false),
    ];
```

It checks `~/.autodev/sessions` (a directory that does not exist and never did — sessions are a
SQLite DB at `~/.airs/autodev.db`), `~/.autodev/plans` and `~/.autodev/hooks` and
`~/.autodev/updates` (none exist; plans/hooks are tables inside `plans.db`/`config.db`). It does
**not** check `~/.airs/autodev.db`, `tasks.db`, `workers.db`, `memory.db`, `annotations.db`, or any
graph DB. Only `config.db` is marked critical. So the "pre-update integrity check" verifies one of
the eight real databases and five paths that are structurally incapable of existing. It reports
"All data verified" regardless.


#### ⚠ RETRACTION — correction to this section's `.airs` inventory

An earlier line in this section read:

> ~~"Nothing in `src/` (the TypeScript front-end), `src-shared`, or `src-cli` touches `.airs` at all."~~

**That claim is WRONG and is retracted.** The `src/` half is correct (0 hits). The `src-shared` and
`src-cli` half is not: my first search was scoped in a way that only surfaced `src-tauri` hits, and
I generalised from an incomplete result set. A wider sweep found **11 more references in the two
crates I cleared**, and they change P1-14's scope:

| `path:line` | what it opens |
|---|---|
| `src-shared/src/config.rs:1` | `// ~/.airs/config.toml — airsctl configuration file types and I/O.` |
| `src-shared/src/config.rs:9-16` | `pub fn config_path()` → `PathBuf::from(home).join(".airs")` … `dir.join("config.toml")` |
| `src-shared/src/config.rs:63`, `:74`, `:82` | load / persist / create-default doc comments for `~/.airs/config.toml` |
| `src-cli/src/main.rs:22` | `/// Path to config file (default: ~/.airs/config.toml)` |
| `src-cli/src/commands/config.rs:8` | `/// Create default config at ~/.airs/config.toml` |
| `src-cli/src/commands/plugin.rs:540` | `// Read ~/.airs/plugins/ for installed plugins` |
| `src-cli/src/commands/plugin.rs:643` | `scan_installed_names()` → `PathBuf::from(home).join(".airs").join("plugins")` |
| `src-cli/src/commands/plugin.rs:659` | `plugin_install_dir()` → `.join(".airs").join("plugins").join(name)` |

**Two consequences for P1-14 that the corrected inventory exposes:**

1. **`~/.airs/config.toml` is a third thing living in the second root**, and it is *airsctl's own
   configuration file* (`working_dir`, `default_provider`, `auto_start_sidecars`,
   `orchestrator_url` — `src-shared/src/config.rs:18-28`). It is not a database, and it appears
   neither in the claim, nor in the MDP's P1-14 text, nor in `updater/persistence.rs`'s integrity
   checklist. It does **not** exist on this machine (`ls ~/.airs/config.toml` → No such file), so
   there is nothing to migrate today — but the *code path* must be redirected, or `airsctl` will go
   on reading its config from the root P1-14 is meant to retire.

2. **`src-shared` CREATES `~/.airs/` unconditionally.** `src-shared/src/config.rs:13-14`:
   ```rust
       let dir = PathBuf::from(home).join(".airs");
       std::fs::create_dir_all(&dir)?;
   ```
   `create_dir_all` runs on *every* call to `config_path()`, before any file is read. So **P1-14
   cannot be completed by moving data and deleting `~/.airs/`** — the next `airsctl` invocation
   recreates the directory. The consolidation must change `src-shared/src/config.rs:13` and
   `src-cli/src/commands/plugin.rs:643`/`:659` in the same step, or the "one data root" outcome is
   cosmetic. **Add to P1-14's acceptance criteria: after the migration, run `airsctl` and assert
   `~/.airs/` is not recreated.**

The rest of this section stands: `~/.airs/plugins/` and `~/.airs/fleet_secret` still do not exist,
and `~/.airs/autodev.db` (1 session, 14 messages) is still the only *data* in the root. The verdict
for C4 is unchanged — but the migration is a **four-path job** (`autodev.db`, `config.toml`,
`plugins/`, `fleet_secret`) spanning **three crates**, not a one-DB job inside `src-tauri`.

**Mechanism of my error, recorded so it is not repeated:** I ran one pattern against `src-tauri`
and a separate one against `src/`, then wrote a universal negative ("nothing in `src-shared` or
`src-cli`") that neither search had covered. A negative claim needs a search whose scope equals the
negative's scope.

---

### C5 — `app_settings` has three independent creators

**VERDICT: CONFIRMED on the count — exactly three. On the schema they agree on columns and
disagree on the trigger.**

1. `src-tauri/src/ide/workspace.rs:34-41`
```sql
CREATE TABLE IF NOT EXISTS app_settings (
     key        TEXT PRIMARY KEY,
     value      TEXT NOT NULL,
     updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
 );
```

2. `src-tauri/src/commands/workspace_commands.rs:22-29`
```sql
CREATE TABLE IF NOT EXISTS app_settings (
     key        TEXT PRIMARY KEY,
     value      TEXT NOT NULL,
     updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
 );
```

3. `src-tauri/src/commands/github_mcp_commands.rs:29-39`
```sql
CREATE TABLE IF NOT EXISTS app_settings (
     key   TEXT PRIMARY KEY,
     value TEXT NOT NULL,
     updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
 );
 CREATE TRIGGER IF NOT EXISTS update_app_settings_ts
     AFTER UPDATE ON app_settings
     BEGIN
         UPDATE app_settings SET updated_at = CURRENT_TIMESTAMP WHERE key = NEW.key;
     END;
```

All three are private `fn open_config_db() -> Result<Connection>` helpers, each in its own module,
each opening `~/.autodev/config.db` (`:31`, `:19`, `:26` respectively) and running its own
`execute_batch` on every call. No shared helper, no import between them.

**Column agreement: yes** — identical three columns, identical types, identical PK, identical
default. The only textual difference is whitespace alignment.

**Behavioural disagreement: yes, on the `updated_at` trigger.** Creators 1 and 2 do not create
`update_app_settings_ts`. Creator 3 does. So whether an `UPDATE app_settings SET value = …` bumps
`updated_at` depends on whether the GitHub-MCP module has ever been called in this DB's lifetime.
On this machine it has: live `config.db` triggers are
`['autodev_instructions_updated_at', 'update_app_settings_ts', 'update_model_configs_ts']`, and the
live `app_settings` DDL is verbatim creator 3's (the `key   TEXT PRIMARY KEY` three-space form). So
creator 3 won here, and the trigger exists — but that is luck of call order, not design.

Live `app_settings` contents (4 rows): `workspace_dir`, `ide_open_tabs`, `ide_expanded_dirs`,
`ide_active_tab` — all last touched 2026-06-01.

Note that only creators 1 and 2 write those four keys, and both of them *would have* created the
table without the trigger. The consolidation is genuinely a 3-way merge, not a 2-way one.

---

### C6 — `hourly_costs` is queried but never created

**VERDICT: CONFIRMED, both halves, with live proof.**

The query, `src-tauri/src/analytics/queries.rs:23-43`:

```rust
    pub fn row_counts_for_diagnostic(&self) -> Result<std::collections::HashMap<String, i64>> {
        let mut out = std::collections::HashMap::new();
        let conn = self.db.conn.lock()…;
        for table in &[
            "token_usage",
            "api_failures",
            "system_events",
            "worker_stats",
            "daily_costs",
            "hourly_costs",
        ] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap_or(-1);
            out.insert((*table).to_string(), count);
        }
        Ok(out)
    }
```

`hourly_costs` is at `queries.rs:35`; the mask is `.unwrap_or(-1)` at `queries.rs:39`.

**Proof of non-creation:** `rg 'hourly_costs'` over the entire repository returns exactly **one**
source hit — `src-tauri/src/analytics/queries.rs:35`. Every other hit is in `docs/`. There is no
`CREATE TABLE hourly_costs` anywhere in `src-tauri`, `src-shared`, `src-cli`, the TypeScript tree,
the sidecars, or the submodules. The `AnalyticsDb::migrate()` batch at
`src-tauri/src/analytics/db.rs:52-120` creates exactly five tables: `token_usage` (`:52`),
`daily_costs` (`:73`), `worker_stats` (`:85`), `api_failures` (`:99`), `system_events` (`:110`).

**Live proof:** `~/.autodev/analytics.db` `sqlite_master` table list is
`['token_usage', 'sqlite_sequence', 'daily_costs', 'worker_stats', 'api_failures', 'system_events']`.
No `hourly_costs`. So on every run, the *one diagnostic written to answer "is the dashboard hitting
the right database"* prints `hourly_costs: -1`, indistinguishable from "table exists but is empty"
(which would be `0`) and from "query errored".

Note this diagnostic is doubly weak: `-1` is also what `daily_costs` would report if it were
broken, and `daily_costs` legitimately reads `0` today. The mask is the bug, the missing table is
just the trigger.

---

### C7 — the timestamp bug: `parse_from_rfc3339` on a `datetime('now')` column

**VERDICT: PARTIALLY CONFIRMED / MISDIRECTED. There are 7 `parse_from_rfc3339` call sites, but
only ONE of them reads a `datetime('now')` column — and that one is broken 100% of the time and
silently reports "healthy". The five analytics sites the claim probably means are reading columns
that Rust writes as real RFC3339, so they work today; they are landmines, not live bugs.**

First, the mechanism, verified against the vendored chrono source
(`chrono 0.4.44`, per `Cargo.lock`; `~/.cargo/registry/src/index.crates.io-…/chrono-0.4.44/src/format/parse.rs:192-256`):

- line `:214` — `if !matches!(bytes.get(10), Some(&b't' | &b'T' | &b' ')) { return Err(INVALID); }`
  → chrono **does** accept a space separator, so that half of the usual folklore is wrong.
- line `:247` — `let offset = try_consume!(scan::timezone_offset(s, |s| scan::char(s, b':'), true, false, true));`
  → the **timezone offset is mandatory and is not optional**. `datetime('now')` emits exactly 19
  characters, `"2026-09-01 03:42:00"`, with nothing after the seconds, so `timezone_offset("")`
  returns `TOO_SHORT` and the whole parse fails. Also `:193` `if bytes.len() < 19 { TOO_SHORT }`.

So the claim's conclusion (parse fails) is right; the stated reason (the space) is not the
operative one — the missing offset is.

#### The one site that is actually broken: `src-tauri/src/orphan/mod.rs:183`

```rust
    // TTL fallback for tasks without a recorded PID (pre-PID tasks or
    // tasks assigned via direct DB edits / UI drag-drop) AND for tasks
    // claimed on a different host where we can't validate the PID locally.
    let updated = chrono::DateTime::parse_from_rfc3339(&task.updated_at).ok()?;
    let age_hours = chrono::Utc::now()
        .signed_duration_since(updated.with_timezone(&chrono::Utc))
        .num_hours();
    if age_hours > STALE_TTL_HOURS {
        return Some(format!(
            "no local PID validation possible and task is {}h stale (TTL fallback)",
            age_hours
        ));
    }

    None
```

`task.updated_at` comes from `tasks.db`'s `tasks.updated_at`, which is written **only** by SQLite:

- `src-tauri/src/db/tasks.rs:338` — `updated_at TEXT NOT NULL DEFAULT (datetime('now')),`
- `src-tauri/src/db/tasks.rs:346-350` — trigger `tasks_updated_at`:
  `UPDATE tasks SET updated_at = datetime('now') WHERE id = NEW.id;`
- and ~10 explicit statements `… updated_at = datetime('now') …` at `tasks.rs:624`, `:631`, `:643`,
  `:983`, `:1030`, `:1143`, `:1163`, `:1184`, `:1194`, `:1489`, `:1512`.

`rg 'updated_at' src-tauri/src/db/tasks.rs` shows **no** path that writes it as RFC3339. So
`task.updated_at` is always `YYYY-MM-DD HH:MM:SS`, the parse always errors, `.ok()?` always
short-circuits, and the enclosing function always returns `None`.

**What happens on failure: it silently means "not an orphan."** `None` is the "task is healthy"
answer. The consequence is that the TTL fallback branch — the *only* orphan check for tasks with no
`claimed_by_pid`, and the *only* check for tasks claimed on a different host (`orphan/mod.rs:163-164`
deliberately falls through to it) — **is dead code that has never fired and cannot fire.** A task
stranded by a direct DB edit, a UI drag-drop assignment, or a dead worker on another machine is
never released. There is no log line on the failure path; `.ok()?` discards the error.

#### The site that is fine: `src-tauri/src/routing/mod.rs:93`

```rust
            .and_then(|h| chrono::DateTime::parse_from_rfc3339(&h.assigned_at).ok())
```
`routing_history.assigned_at` is declared `TEXT NOT NULL` with **no default**
(`src-tauri/src/db/tasks.rs:478`) and is written from Rust at `src-tauri/src/routing/mod.rs:57`:
`assigned_at: chrono::Utc::now().to_rfc3339(),`. Live sample from
`~/.autodev/tasks.db` (220 rows): `'2026-05-14T09:32:17.418759+00:00'`. **Parses correctly.**

#### The five analytics sites: currently correct, structurally fragile

`src-tauri/src/analytics/db.rs:183`, `:392`, `:435`, `:496`, `:729`. All read columns declared
`DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP` (`analytics/db.rs:54`, `:101`, `:112`) or
`last_heartbeat DATETIME` (`:93`) — i.e. the *declared default* is SQLite's space-separated form.
But every INSERT in the codebase supplies the column explicitly from Rust:
`record.timestamp.to_rfc3339()` at `db.rs:149`, `:470`, `:758` and
`record.last_heartbeat.map(|dt| dt.to_rfc3339())` at `:341`. `rg 'INSERT.*(token_usage|system_events|api_failures|worker_stats|daily_costs)'`
finds exactly four INSERT statements, all of which name the timestamp column.

Live proof that the parse works today:
```
token_usage.timestamp   = '2026-05-30T09:13:04.375083100+00:00'
system_events.timestamp = '2026-05-14T14:10:37.633760300+00:00'
worker_stats.last_heartbeat = '2026-05-30T13:21:08.285814+00:00'
```

**Failure behaviour if the default ever does fire** (a raw `INSERT` from a migration, a manual
`sqlite3` fix-up, or a future rollup job that omits the column):
- `db.rs:185`, `:498`, `:731` — `.unwrap_or_else(|_| Utc::now())` → the row's timestamp becomes
  **now**, silently. A May row is reported as today's. This is worse than an error: it corrupts
  cost-by-day and event-timeline reporting with no signal.
- `db.rs:393`, `:436` — `.ok()` → `last_heartbeat` becomes `None`, i.e. "never seen".

So the claim's "3+ known sites" is numerically right (7 sites) but points at the wrong ones. The
correct statement is: **one live, always-failing site with a silent-healthy default
(`orphan/mod.rs:183`), and five latent sites whose failure mode is silent timestamp fabrication.**

---

### C8 — redaction was never ported to Rust

**VERDICT: CONFIRMED on the substance (there is NO secret redaction anywhere in the Rust tree),
OVERSTATED on one detail — `SENSITIVE_KEYS` has 22 entries, not 20.**

#### The Rust side: nothing

Searches run over `src-tauri`, `src-shared`, `src-cli` (case-insensitive, whole tree):

| pattern | hits in `src-shared` | hits in `src-cli` | hits in `src-tauri` |
|---|---|---|---|
| `redact` | 0 | 0 | 0 |
| `scrub` | 0 | 0 | 0 |
| `REDACTED` | 0 | 0 | 0 |
| `SENSITIVE` (as an identifier) | 0 | 0 | 0 |
| `sanitiz` | 0 | 0 | 3 (all unrelated, below) |
| `mask` | 0 | 0 | 1 (unrelated, below) |

**There is no `redact_dict`, no `redact_string`, no `redact_args`, no `safe_repr`, no
`SENSITIVE_KEYS`, and no equivalent by any other name, anywhere in the Rust tree.** The claim is
correct.

The three `sanitiz`/`mask` hits are all something else and none of them redacts a secret:

1. `src-tauri/src/analytics/db.rs:704-708` — a **SQL-injection** filter for two WHERE-clause
   literals, not a secret redactor:
```rust
        let sanitize = |s: &str| -> String {
            s.chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                .collect()
        };
```
2. `src-tauri/src/memory/procedural.rs:235` — `// Sanitize: only allow alphanumeric, dash, underscore.`
   — a **filename** sanitizer for a path segment.
3. `src-tauri/src/commands/vault_commands.rs:66-75` — `masked_last4()`, a UI **display** helper that
   returns the last four characters of a key the user has just typed, so the settings screen can
   confirm the paste. It masks *for display of a value already in hand*; it never touches a log,
   a DB write, or a tool argument.

`src-tauri/src/vault/keychain.rs` stores secrets in the OS keychain and
`src-tauri/src/commands/vault_commands.rs:3-16` documents a genuine inbound-only IPC invariant
(INV-12) — that is real security work, but it is *storage* hygiene, orthogonal to redacting values
that have already left the vault and are travelling through logs, tool arguments, and message rows.

**Concrete exposure surface this leaves open** (worth naming so the port has a target):
`~/.airs/autodev.db`'s `messages` table has a `tool_input TEXT` column and a
`tool_calls TEXT NOT NULL DEFAULT '[]'` column — verbatim tool arguments are persisted to disk with
no filter of any kind. `analytics.db`'s `system_events.metadata TEXT` and `config.db`'s `hook_logs`
are the same shape. In the Python sidecar those paths would go through `redact_args()`.

#### The Python original

`submodules/core-orchestrator/src/airs/security/redaction.py:9-19`, verbatim:

```python
# Sensitive key patterns (case-insensitive)
SENSITIVE_KEYS = {
    "api_key", "apikey", "api-key",
    "secret", "secret_key", "secretkey",
    "password", "pass", "pwd",
    "token", "access_token", "refresh_token", "auth_token",
    "authorization", "auth",
    "private_key", "privatekey",
    "client_secret", "clientsecret",
    "bearer",
    "credential", "credentials"
}
```

**Counted programmatically: 22 entries, 22 unique.**
`['api_key','apikey','api-key','secret','secret_key','secretkey','password','pass','pwd','token','access_token','refresh_token','auth_token','authorization','auth','private_key','privatekey','client_secret','clientsecret','bearer','credential','credentials']`

The claim's "20-entry" is wrong by two. It matters slightly for the port spec: matching is
**substring, not equality** (`redaction.py:42`
`is_sensitive_key = any(sk in key_lower for sk in SENSITIVE_KEYS)`), so `"pass"` also catches
`passphrase`/`bypass`, and `"auth"` catches `oauth`/`author`. A Rust port that uses exact-match
`HashSet::contains` would be materially weaker than the original — and one that copies the
substring behaviour will redact `author` fields. Either way, port the *semantics* deliberately, not
just the list.

The module is more than a key list — a faithful port needs all four functions:

- `redact_dict(data, redact_keys=True, redact_patterns=True)` (`:22-58`) — recursive over dicts and
  lists; sensitive-keyed values → `"[REDACTED]"`; every *other* string value is still passed through
  `redact_string`.
- `redact_string(text)` (`:61-85`) — two regexes:
  `re.sub(r'Bearer\s+[\w\-\.]+', 'Bearer [REDACTED]', text, flags=re.IGNORECASE)` and
  `re.sub(r'\b[A-Za-z0-9_\-]{32,}\b', '[REDACTED_KEY]', text)`. Note the second one is aggressive —
  any 32+ char alphanumeric token becomes `[REDACTED_KEY]`, which will also eat SHA-256 hashes,
  UUIDs-without-dashes, and base64 blobs. Porting it verbatim will change log content broadly.
- `redact_args(arguments)` (`:88-100`) — the tool-argument wrapper.
- `safe_repr(obj, max_length=200)` (`:103-126`) — redact + truncate for logging.

**A second Python redactor also exists and the claim does not mention it:**
`submodules/core-orchestrator/src/airs/logging/centralized_collector.py:193` —
`def redact(self, text: str) -> str:`, a regex-pattern PII redactor on a class with a
`PATTERNS` dict, `enabled` flag and `custom_patterns` override, wired into
`CentralizedLogCollector(… enable_pii_redaction: bool = True …)`. So the Python side has **two**
independent redaction facilities (secret-key redaction and PII redaction) and Rust has zero. Any
port scoped only to `redact_dict` will still leave the PII half unported.

---

### C9 — no backup helper exists

**VERDICT: PARTIALLY CONFIRMED / OVERSTATED as written. Two backup-ish mechanisms DO exist in the
Rust tree — but neither backs up a database, and no destructive DB path is guarded by anything.
The claim's operative point ("no backup wired into destructive paths") holds.**

#### What DOES exist

1. **`backup_recipe_file`** — `src-tauri/src/commands/recipe_commands.rs:19-37`. A real
   copy-before-overwrite: `backups_dir()` = `~/.autodev/recipes/backups`, destination
   `format!("{recipe_id}-{ts}.yaml")` (`:33`). It IS wired into its destructive paths —
   `update_recipe` calls it at `:132`, `delete_recipe` at `:160`, and `restore_recipe_backup` backs
   up the current version before restoring at `:176`. `list_recipe_backups` (`:62`, newest 10) and
   `restore_recipe_backup` (`:167`) complete the loop. This is the one place in the product that
   gets it right — **it is a YAML-file backup, and it covers recipes only.**

2. **`create_restore_point`** — `src-tauri/src/updater/persistence.rs:91-135`. Called once, from
   `src-tauri/src/commands/updater_commands.rs:244-245` before an app update. It is **not a
   backup**: it writes `~/.autodev/restore_point.json` containing, for `config.db` and
   `analytics.db` only, `{path, size, modified}` — a size-and-mtime manifest. No checksum despite
   the doc comment saying "checksums", no copy of the bytes. `verify_post_update`
   (`persistence.rs:138-149`) then does **nothing with it**: it checks the file exists and returns
   `Ok(report)` without comparing a single field. And `cleanup_after_update` (`:152-159`) deletes
   the manifest afterwards. So the "restore point" cannot restore anything and is not even read.

`rg 'VACUUM INTO'` → 0 hits. `rg 'Connection::backup|rusqlite::backup'` → 0 hits. rusqlite's
`backup` feature is not enabled in either `Cargo.toml`. **There is no mechanism anywhere that can
copy a live SQLite database safely.**

#### Destructive paths, and what guards them: nothing

| `path:line` | operation | guard |
|---|---|---|
| `src-tauri/src/db/tasks.rs:674-683` | `clear_all()` — `DELETE FROM worker_requests; DELETE FROM tasks; DELETE FROM sqlite_sequence …` in one transaction. Doc comment: *"Used by the UI 'Clear Board' action to start fresh."* | **none** |
| `src-tauri/src/commands/graph_commands.rs:810-829` | `delete_graph()` — `std::fs::remove_file(&db_path)?` deletes an entire graph database file | **none** |
| `src-tauri/src/graph/store.rs:741` | `DROP TABLE IF EXISTS nodes_fts` | **none** |
| `src-tauri/src/graph/migrations.rs:100-102` | `DROP TABLE IF EXISTS nodes_fts` then rebuild — runs automatically inside `migration_v4` on any DB with `user_version < 4` | **none** |
| `src-tauri/src/session/storage.rs:319` | `DELETE FROM sessions WHERE id=?1` — cascades to `messages` via `ON DELETE CASCADE` | **none** |
| `src-tauri/src/graph/store.rs:294`, `:300` | `DELETE FROM nodes/edges WHERE file_path = ?` | **none** |
| `src-tauri/src/db/plans.rs:372` | `DELETE FROM plans WHERE id = ?1` — cascades to `plan_items`, `plan_item_dependencies`, `plan_executions` | **none** |
| `src-tauri/src/db/tasks.rs:660`, `src-shared/src/db/tasks.rs:308` | `DELETE FROM tasks WHERE id=…` | **none** |
| `src-tauri/src/plugin/registry.rs:123`, `plugin/manager.rs:66`, `:107` | `remove_dir_all` on a plugin install dir | **none** |
| `src-tauri/src/git/worktree.rs:130` | `remove_dir_all(wt_path_norm)` | **none** |
| `src-tauri/src/git/checkpoint.rs:190`, `:247` | `remove_file` on a checkpoint snapshot + `DELETE FROM checkpoints` | **none** |

Not one of these calls `backup_recipe_file`, `create_restore_point`, or any equivalent, and none of
them is preceded by a copy. So: a helper *exists*, in one corner, for YAML files. **For every
database in the product there is no backup helper, and every destructive database operation runs
unguarded.** That is the claim's substance, and it stands.

---

## Ordering hazards among P1-09 … P1-19

### The headline: the plan's own ordering is inverted in two places

**H1 — P1-19 (the backup helper) is listed LAST, but P1-11 and P1-14 both carry
"⚠ Back up … first (§0.8)".** The helper that would perform those backups does not exist until
after the two steps that require it. Verified in `MASTER_DEVELOPMENT_PLAN.md` §P1-C: P1-11 says
*"⚠ Back up `~/.autodev/graphs/*.db` first (§0.8)"*, P1-14 says *"⚠ Back up both roots first."*,
and P1-19 — *"Add a backup helper and wire it into every destructive path"* — is the last item in
the group. **P1-19 must move to the top of P1-C.** It is also the cheapest item in the group
(a `VACUUM INTO` wrapper plus a call site list; there is no `Connection::backup` or `VACUUM INTO`
anywhere today, and rusqlite's `backup` feature is not enabled — see C9).

**H2 — "collapse to one owner" steps must precede the ladder step, and two of them don't.**
A `PRAGMA user_version` ladder is only authoritative if the database has exactly one migrator.
Today `config.db` has **seven** modules each running their own `CREATE TABLE IF NOT EXISTS` on
open (C2), and `app_settings` alone has three (C5). If P1-10 lands a ladder on `config.db` while
those seven still exist, the ladder is **inert-but-green**: on a fresh install any of the seven can
still win the race and create a table the ladder then declares itself already past. That is
precisely the C3 failure mode reproduced at a larger scale.

- P1-09 (one owner for task/worker/plan schema) → **correctly** placed before P1-10.
- **P1-15** (one owner for `app_settings`) → currently *after* P1-10. **Must move before it.**
- **P1-16** (collapse the two checkpoint systems) → currently *after* P1-10, and it decides the
  final shape of a table that lives in `config.db`. **Must move before it**, or at minimum its
  *decision* (which of the two schemas wins) must be made before P1-10 writes `config.db`'s ladder,
  or `config.db` needs a second rung immediately.

### The specific question asked: P1-10 vs P1-14 and P1-16

**P1-10 must come BEFORE P1-14.** Reasons, strongest first:

1. **P1-14 is a one-time data move and has no idempotency mechanism today.** Live
   `~/.airs/autodev.db` has `user_version = 0`; so would any destination. Without a version stamp
   the only "have I already run" signals available are "does the source file still exist" (fragile —
   the user may restore a backup, or the §0.8 backup itself may be a copy at the old path) or
   row-level dedup. A ladder rung gives it one authoritative bit.
2. **The consolidation should BE a ladder rung, not a separate act.** The correct decomposition:
   P1-10 gives the *destination* DB a ladder; "import sessions+messages from `~/.airs/autodev.db`
   if present, then rename the source aside" is rung N of that ladder. Expressed that way it is
   re-runnable, resumable, and logged.
3. **Caveat that makes the ordering non-obvious, and the right answer to it:** stamping
   `user_version = 1` on `~/.airs/autodev.db` in P1-10 and then deleting that file in P1-14 wastes
   the stamp. So **exclude `~/.airs/autodev.db` from P1-10's rollout** — treat it as a legacy
   source pinned at implicit version 0 — rather than reordering. This is the only part of P1-10
   that should NOT precede P1-14.
4. The counter-argument ("do P1-14 first so there are fewer DBs to ladder") is an efficiency claim,
   not a safety one, and it is weak on the merits: the consolidation does not reduce the DB count
   much (the destination still needs a ladder) and it is the single riskiest step in the group.

**P1-10 must also come BEFORE P1-16**, for reason (1) above — P1-16 is likewise a data move
(108 live rows plus 74 on-disk snapshots, see the risk ranking) — **but with a hard precondition:
P1-16's schema decision must be made before P1-10 authors `config.db`'s ladder.** Sequence:
decide P1-16 (design only) → P1-15 (single `app_settings` owner) → P1-10 (ladder for `config.db`
written against the *live* schema) → P1-16 executes as a rung.

### Cross-group hazard the MDP does not flag

**H3 — P1-02 creates the second checkpoint store that P1-16 then has to collapse.**
Right now the collapse is nearly free: `src-tauri/src/git/checkpoint.rs` writes `checkpoints` in
`config.db` (**108 live rows**, 74 snapshot files, 667 KB under `~/.autodev/checkpoints/`), while
`src-tauri/src/db/checkpoints.rs` writes `file_checkpoints` in `~/.autodev/checkpoints.db` — a file
that **does not exist**, because `db/checkpoints.rs` is **not declared in
`src-tauri/src/db/mod.rs`** (the module list is `annotations, autodev_instructions, model_configs,
plans, tasks, workers` — no `checkpoints`). It is dead code that has never executed. That is
exactly what P1-02 exists to change: *"Declare `db/checkpoints.rs` — the undo feature is currently
absent from the shipped app."*

The moment P1-02/P1-03 land in group A and the app is run, `CheckpointsDb::open()`
(`src-tauri/src/db/checkpoints.rs:66-72`) creates `checkpoints.db` and a **second live store**
starts accumulating rows. P1-16 then becomes a real two-way merge instead of a delete.

And the two schemas do not map onto each other:

| | `checkpoints` (live, `config.db`) | `file_checkpoints` (never created, `checkpoints.db`) |
|---|---|---|
| source | `src-tauri/src/git/checkpoint.rs:33-45` | `src-tauri/src/db/checkpoints.rs:80-87` |
| id | `TEXT PRIMARY KEY` (uuid) | `INTEGER PRIMARY KEY AUTOINCREMENT` |
| keyed by | `session_id TEXT NOT NULL` | `message_id TEXT NOT NULL` |
| content | `snapshot_path TEXT NOT NULL` — a **file on disk** | `file_content TEXT NOT NULL` — **inline** |
| time | `created_ms INTEGER NOT NULL` — epoch ms | `timestamp TEXT NOT NULL DEFAULT (datetime('now'))` |
| other | `reason TEXT`, `size_bytes INTEGER` | `operation_type TEXT NOT NULL DEFAULT 'write'` |

One stores content by path keyed by session; the other stores content inline keyed by message; the
time representations are incompatible (integer ms vs a `datetime('now')` string — which also drags
P1-13 in). **Recommendation: either pull P1-16 forward to sit immediately after P1-02/P1-03, or
have P1-02 declare the module while leaving the store disabled behind a flag until P1-16 picks the
winner.** Today, with `checkpoints.db` non-existent, P1-16 is a one-way delete of dead code; after
P1-02 it is not.

### Other dependencies

- **P1-11 is independent of P1-10** — the graph DB is the one database that already has a ladder
  (C2/C3), so P1-11 can run in parallel with everything else. **But it cannot be done by editing
  `migration_v5`** — see "already impossible" below.
- **P1-12 before P1-09.** P1-09 leaves two processes (the app and `airsctl`) sharing the same
  schema owner and the same files. With no `busy_timeout` anywhere in the tree (verified: zero
  occurrences), concurrent access returns `SQLITE_BUSY` immediately rather than retrying. Not a
  correctness dependency, but it will make P1-09's verification flaky if left until after.
- **P1-12 slightly RAISES P1-14's risk.** Switching `analytics.db`/`annotations.db`/`workers.db`
  from `journal_mode=delete` to `wal` gives them `-wal` companions, and a naive `fs::copy` of a
  WAL database silently omits uncommitted-to-main content. If P1-12 lands before P1-14, P1-14 must
  use `VACUUM INTO` / the SQLite backup API, never a file copy. (This is not hypothetical — see
  risk item 1.)
- **P1-13's direction determines whether it needs P1-10.** If the fix is *reader-tolerant* (a
  `parse_db_timestamp` helper that accepts both `YYYY-MM-DD HH:MM:SS` and RFC3339), it needs no
  migration and is independent — and it fixes `orphan/mod.rs:183` immediately. If the fix
  *normalises stored data* to RFC3339, it is a backfill and needs a ladder rung → after P1-10. It
  also then has a sort-order hazard: `tasks.rs:1015` does `ORDER BY updated_at ASC`, and mixed
  formats sort wrong (space `0x20` < `T` `0x54`, so every legacy row sorts before every new row
  regardless of actual time). **Recommend the reader-tolerant direction**; it is strictly less
  destructive and unblocks the one live bug.
- **P1-18 (run/audit ledger) after P1-10** — it is a new table and should arrive as a rung, not as
  a twelfth independent `CREATE TABLE IF NOT EXISTS`.
- **P1-17 is fully independent** and can be done at any point; it is a one-line deletion.

### Recommended order for P1-C

`P1-19` → `P1-16 (decide only)` → `P1-15` → `P1-09` → `P1-12` → `P1-10` (excluding `~/.airs`) →
`P1-16 (execute)` → `P1-14` → `P1-18` → `P1-13` → `P1-11` (any time) → `P1-17` (any time).

---

## Destructive-risk ranking (P1-09 … P1-19)

Live data actually at stake, measured read-only:

| store | size | contents |
|---|---|---|
| `~/.airs/autodev.db` + WAL | 400 KB + **4.1 MB uncheckpointed WAL** | sessions(1), messages(14) |
| `~/.autodev/config.db` | 135 KB | **checkpoints(108)**, app_settings(4), model_configs(2), autodev_instructions(1), hooks(0), hook_logs(0) |
| `~/.autodev/checkpoints/` | **667 KB, 74 entries** | snapshot files backing the 108 checkpoint rows |
| `~/.autodev/analytics.db` | 180 KB | token_usage(259), system_events(62), worker_stats(10) |
| `~/.autodev/tasks.db` | 164 KB | routing_history(220), tasks(0) |
| `~/.autodev/memory.db` | 100 KB | episodic_memories(39) |
| `~/.autodev/workers.db` | 24 KB | workers(7) |
| `~/.autodev/annotations.db` | 20 KB | annotations(8) |
| `~/.autodev/graphs/d0ef18f6ad8b.db` | 124 KB | **every table 0 rows** |
| `~/.autodev/graph_cache/` | 87 JSON files | regenerable cache |

**1. P1-14 — data-root consolidation. HIGHEST RISK.** Two independent loss mechanisms, both silent:
   - *(a) the WAL.* `~/.airs/autodev.db` is a **30 May** file; its `-wal` is **1 June** and holds
     4.1 MB. Any migration that does `fs::copy(autodev.db, dest)` — or that reads the DB while the
     `-wal`/`-shm` are left behind — moves a stale snapshot and loses everything written in that
     window. Must use `VACUUM INTO`, the SQLite backup API, or `PRAGMA wal_checkpoint(TRUNCATE)`
     first. Nothing in the tree can do any of those today (C9).
   - *(b) schema drift between the live table and the code's DDL.* The live `messages` table has
     `tool_calls TEXT NOT NULL DEFAULT '[]'` and `tool_results TEXT NOT NULL DEFAULT '[]'` —
     columns the current `CREATE TABLE messages` at `src-tauri/src/session/storage.rs:68-81` does
     **not** declare (C4). A migration that creates the destination from the code's DDL and copies
     "the columns the code knows about" drops two columns of real message content, silently.
     **The migration must be written from `PRAGMA table_info` on the live file, not from the source.**
   - What could be lost: 1 session, 14 messages, plus the WAL's contents. Small in row count,
     irreplaceable in kind (it is the only conversation history).

**2. P1-16 — checkpoint collapse. HIGH.** 108 rows in `config.db.checkpoints`, each pointing at one
   of **74 snapshot files (667 KB)** under `~/.autodev/checkpoints/`. The target schema
   (`file_checkpoints`) stores content **inline** rather than by path, so the collapse must read
   all 74 files and inline them — a step that, if it half-succeeds, leaves rows whose content is
   neither on disk nor in the DB. If it goes the other direction, the 108 rows must be rewritten
   from `created_ms INTEGER` to `timestamp TEXT`. Either way it touches `config.db`, the only DB
   the (non-functional) integrity check marks *critical*. **What could be lost: the entire undo
   history for real file edits.**

**3. P1-09 — delete `src-shared`'s mirrored tables. HIGH-MEDIUM.** The code deletion is safe; the
   *schema unification* is not. `workers.db` holds 7 rows in src-tauri's 13-column shape, and the
   unified schema must gain `last_seen`/`notes` (C1). Adding them is a safe `ALTER`. But changing
   `created_at DATETIME DEFAULT CURRENT_TIMESTAMP` to
   `TEXT NOT NULL DEFAULT (datetime('now'))`, or changing the default model, requires SQLite's
   12-step table rebuild — and a mistake there loses the 7 worker rows and the trigger.
   `tasks.db` carries 220 `routing_history` rows in the blast radius of the same file.
   **Mitigating fact: `plans.db` does not exist**, so the `plans` half — the genuinely incompatible
   one — has zero data at risk *today*. That window closes the moment either `PlansDb` is opened.

**4. P1-19 — backup helper. LOW to implement, but its LATE POSITION is the single highest
   systemic risk in the group** (see H1). Implementing it is additive and destroys nothing.

**5. P1-10 — user_version ladders. LOW-MEDIUM.** Stamping a version is non-destructive. The risk is
   in rung 1: it must be written against **what is on disk**, not what the code's `CREATE TABLE`
   says, because live evidence shows they differ (`messages` has two extra columns; `workers` is
   missing two the CLI expects; `config.db`'s `app_settings` was created by the variant that also
   installs a trigger). A rung that "normalises" a live table to the code's DDL drops columns.
   This report's live-schema dumps are the correct source for those rungs.

**6. P1-13 — timestamp helper. LOW if reader-tolerant; MEDIUM if it backfills.** A backfill would
   rewrite `tasks.updated_at` (0 rows today — free) but must **not** touch `routing_history`
   (220 rows) or the analytics tables (331 rows), which already hold correct RFC3339.

**7. P1-12 — WAL + busy_timeout. LOW.** `analytics.db`, `annotations.db` and `workers.db` are live
   at `journal_mode=delete` (verified read-only); `config.db`, `memory.db`, `tasks.db`,
   `~/.airs/autodev.db` and the graph DB are already `wal`. The switch is in-place and supported.
   Its only risk is the second-order one on P1-14 noted above.

**8. P1-11 — graph v5 repair. LOW, and lower than the MDP assumes.** The MDP says
   *"⚠ Back up `~/.autodev/graphs/*.db` first"*. Verified: the only graph DB on this machine has
   **0 rows in nodes, edges, communities, flows, snapshots, repos and analysis_cache**. There is
   nothing to lose. The genuine risk is elsewhere and is a *correctness* risk, not a data one —
   see "already impossible".

**9. P1-15 — one `app_settings` owner. LOW.** 4 rows: a workspace path and three IDE tab lists,
   all trivially reconstructible by the user.

**10. P1-17 — `hourly_costs`. NIL.** Deleting one string from an array at
   `src-tauri/src/analytics/queries.rs:35`. Note the plan's "delete **or create**" — creating it is
   the wrong branch: nothing writes it, so a created table reports `0` forever, which is exactly as
   uninformative as the `-1` it replaces. **Delete.**

**11. P1-18 — run/audit ledger. NIL to existing data** (new table only), provided it arrives as a
   ladder rung rather than a twelfth independent `CREATE TABLE IF NOT EXISTS`.

---

## Already done, or already impossible

**Already done (fully or in part):**
- **P1-10 is 1 of 11 done.** The graph DB has a real `user_version` ladder (v1…v5) and reports
  `user_version = 5` live. No other database has one; all report 0.
- **P1-12 is ~70% done on the WAL half, 0% on `busy_timeout`.** `PRAGMA journal_mode = WAL` is
  issued in 11 places (`db/autodev_instructions.rs:44`, `db/checkpoints.rs:77`, `db/plans.rs:215`,
  `db/tasks.rs:322`, `graph/store.rs:84`, `hooks/db.rs:21`, `memory/db.rs:29`,
  `session/storage.rs:29`, `src-shared/src/db/{plans.rs:47,tasks.rs:165,workers.rs:46}`).
  Live confirmation: `config.db`, `memory.db`, `tasks.db`, `graphs/*.db`, `~/.airs/autodev.db` are
  `wal`; **`analytics.db`, `annotations.db`, `workers.db` are `delete`** — those three are the gap.
  `busy_timeout` / `busy_handler`: **zero occurrences in the entire Rust tree.**
- **P1-16 is currently far cheaper than the plan assumes** — one of the two checkpoint systems has
  never executed (`db/checkpoints.rs` is not declared in `db/mod.rs`; `~/.autodev/checkpoints.db`
  does not exist). It is a delete, not a merge — until P1-02 lands.
- **P1-09's `plans` half has no data at risk** — `~/.autodev/plans.db` does not exist. It is a pure
  code decision today.
- **P1-14's "plugin/fleet-secret tree" needs no migration** — `~/.airs/plugins/` and
  `~/.airs/fleet_secret` do not exist. Only `autodev.db` needs moving.

**Already impossible as written:**
- **P1-11 cannot be achieved by editing `migration_v5`.** Every graph DB that has ever been opened
  is already stamped `user_version = 5` (live proof: `d0ef18f6ad8b.db` = 5), and
  `src-tauri/src/graph/migrations.rs:37` gates the call on `if current < 5`. A corrected v5 will
  **never be invoked** on any existing database. The repair must be a **new `migration_v6`** that
  adds the columns conditionally (check `PRAGMA table_info(nodes)`, or issue a plain
  `ALTER TABLE … ADD COLUMN` and tolerate the duplicate-column error explicitly — not with a bare
  `let _ =`). Editing v5 in place would be the textbook "fixed it, gates green, nothing changed"
  outcome. **This is the most important single correction in this report for P1-C.**
- **P1-20's "20-entry `SENSITIVE_KEYS`" is wrong: it is 22 entries** (counted programmatically from
  `submodules/core-orchestrator/src/airs/security/redaction.py:9-19`). The plan text needs the
  correction, and the port needs to preserve the module's **substring** matching semantics
  (`redaction.py:42`), not exact-match.
- **P1-13's "3+ known sites" is both an undercount and a misdirection.** There are 7
  `parse_from_rfc3339` sites; **only one** (`src-tauri/src/orphan/mod.rs:183`) reads a
  `datetime('now')` column, and that one fails on every call and silently reports "not an orphan".
  The five analytics sites read Rust-written RFC3339 and work correctly today. Fixing "3 sites"
  without naming `orphan/mod.rs:183` would leave the only live bug in place.
- **P1-09's "four hand-mirrored tables … incompatible" is right for `plans`/`plan_steps` and
  `workers`, but `tasks` is compatible** (all src-tauri additions are nullable or defaulted, and
  its `ALTER TABLE` block self-heals a CLI-created table). Treating `tasks` as an incompatibility
  will produce an unnecessary rebuild of the table that holds `routing_history`'s sibling data.

**One live bug found in passing that is not in P1-C at all:** the `airsctl` worker commands are
broken **right now** on this machine. `src-shared/src/db/workers.rs` SELECTs `last_seen` and
`notes` at `:85`, `:93`, `:106`, `:108`, `:110`, `:112` and UPDATEs `last_seen` at `:143`; the live
`~/.autodev/workers.db` — created by src-tauri — has neither column. Every one of those calls
raises `no such column: last_seen`. This is C1's failure mode already fired, and it belongs in
P1-09's acceptance criteria as a regression test.

---

## What I could not determine

- **Whether any graph DB in the wild is mislabelled `user_version = 5` while missing the six
  "v5" columns.** The only graph DB on this machine was created after `agent_author`/`task_id`/
  `worker_locked`/`worktree` entered `CREATE_NODES` (`graph/schema.rs:57-60`), so it has them. A DB
  created before that change would be permanently broken with no repair path. I have no access to
  another machine's data and did not run the app to create one.
- **Whether `~/.airs/autodev.db`'s 4.1 MB WAL contains rows beyond the 1 session / 14 messages I
  counted.** My read-only connection reads through the WAL, so the counts *should* include it —
  but 4.1 MB for 15 rows is anomalous and I could not explain it without writing (a
  `wal_checkpoint` is a write and was out of scope). It may be page churn from repeated
  rewrites, or it may indicate deleted rows. **Whoever executes P1-14 must checkpoint and re-count
  before and after.** Treat my 15-row figure as a floor, not a fact.
- **Whether the app was running during my inspection**, and therefore whether any count moved under
  me. The `~/.airs/autodev.db-shm` and `~/.autodev/workers.db` mtimes of 2026-09-01 10:42 (both
  predating my first command) show an AutoDev process opened those files today. I did not enumerate
  processes.
- **Whether `hourly_costs` was ever created by a version of the code no longer in the tree.** I
  checked the live `analytics.db` (it is absent) and the current source (no `CREATE TABLE`), but I
  did not read git history — the repo has no `.git` reachable from the working directory per the
  environment banner, and history commands were out of scope.
- **Whether any TypeScript/front-end code opens or writes these databases directly.** I searched
  `src/` for `.airs` (0 hits) but did not exhaustively audit the front-end for SQLite access; the
  Tauri IPC surface makes direct access unlikely but I did not prove it.
- **The exact byte-level effect of a `journal_mode` switch on the three `delete`-mode DBs.** I did
  not test it, because testing it requires writing to live databases.
- **Whether `redact_string`'s 32+-char rule (`redaction.py:83`) is safe to port verbatim.** It will
  also match SHA-256 hashes, undashed UUIDs and base64 blobs. Whether that breaks existing log
  consumers is a product decision I could not evaluate from the code.

