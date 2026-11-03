# 08 — Data Model and Migrations

| | |
|---|---|
| **Parent** | `00-…-MASTER-PLAN.md` §Part 1 (INV-10), §Part 1B (M-16), §Part 4, §Part 5 (S5) |
| **Locks** | Hands **ADR-01-03**, **ADR-01-09**, **OQ-5** to `00 §Part 8` / §Part 9 |
| **Status** | PLANNED |
| **Grounding** | `_grounding/G1 §E` (the authoritative database inventory) · `G0 §5.4` · `G3c` (IN-54, IN-59…IN-65) · `G3d` (F-22, F-31, F-45) · `G3b` (GI-01, GI-27, GI-28) |

---

## What this is

Eleven SQLite databases, ~30 tables, and **one real migration ladder among them**. This spec sets the discipline that stops the data layer from being the place where silent corruption lives.

**The subtle failures this subsystem is prone to:**

1. **`CREATE TABLE IF NOT EXISTS` as a migration strategy.** It works until a column is added, at which point the schema depends on which binary opened the file first.
2. **A timestamp written one way and read another.** SQLite's `datetime('now')` is not RFC 3339. Parsing one as the other yields `None` or `0`, and `.unwrap_or` turns that into a plausible-looking answer.
3. **A function around an indexed column.** `WHERE DATE(timestamp) = ?` cannot use an index on `timestamp`.
4. **Growth with no retention.** Every append-only table is a future performance incident.

---

## 1. The verified live state

**11 databases** (`G1 §E.1`, `CODE-PRESENT`). Reproduced in `00 §Part 4.1`; not repeated here.

> ⚠ **Superseded by P1 (2026-09-01).** Every database now carries a ladder (`P1-10`), the graph head is repaired and advanced to v6 (`P1-11`), the three data roots are one (`P1-14`), `src-shared`'s schema mirrors are deleted (`P1-09`), and a backup helper is wired into every destructive path (`P1-19`). What follows records the state that motivated those tasks.

**The migration head is meaningful in exactly one:** `graph/` has `PRAGMA user_version` with 5 migrations, head **v5**. Every other database runs idempotent `CREATE TABLE IF NOT EXISTS` on each open. There is **no schema-version table, no down-migration, and no backup path anywhere in the crate** — `updater/persistence.rs` creates a restore point for `config.db` and `analytics.db` only, and only during an update.

And the one ladder that exists is **broken at its head**: migration v5 uses `ADD COLUMN IF NOT EXISTS`, which SQLite does not support. The error is swallowed and the migration can never retry (`G3d F-31`).

---

## 2. The seven reconciliations

Detailed in `00 §Part 4.2` (R1–R7). The two with the widest blast radius:

**R1 — three data roots.** *(Resolved by `P1-14`: the session store is now `~/.autodev/autodev.db`, copied across on first start with the original left in place; `~/.airs/plugins/` and `~/.airs/fleet_secret` were code-only paths that never existed on disk.)* Sessions lived in `~/.airs/autodev.db`; plugins and the fleet secret live under `~/.airs/`; **everything else** lives under `~/.autodev/`. The old master plan already contradicted itself on two roots (`§8` vs `§4.1`); the code has three. A user backing up "their AutoDev data" loses whichever they miss. → **ADR-01-09**, with a one-time migration.

**R5 — the hand-mirrored schema.** `src-shared/src/db/tasks.rs` states in its own header that it "mirrors the schema in `src-tauri/src/db/tasks.rs`", and it has **already drifted**: it lacks `rejection_reason`, `retry_count`, `claimed_by_pid`, `claim_host` and one index. Severity is contained — the Tauri side carries additive `ALTER TABLE` migrations that repair a table `airsctl` created — but `airsctl`'s `Task` struct **structurally cannot represent why QC rejected a task or how many times it has been retried**. → **ADR-01-03**, INV-10.

---

## 3. The discipline

### 3.1 Migrations

1. **Every database gets `PRAGMA user_version` and a numbered ladder.** New migrations are appended after the verified live head: **v5 for `graph/`** (after repair), **v0 for everything else**.
2. **A migration that fails, fails loudly.** No swallowed errors, no `IF NOT EXISTS` standing in for versioning.
3. **A backup is taken before any destructive step**, and its location is recorded. There is no backup path today.
4. **Additive-only by default.** Column drops and type changes require an explicit, reviewed migration with a backup.

### 3.2 Connections

Every connection sets:
- `journal_mode = WAL` — today only `memory.db` does
- `busy_timeout` — **today nothing does**, so a concurrent writer gets an immediate `SQLITE_BUSY` rather than waiting. With `airsctl` in the picture (ADR-01-03) this is a live multi-process concern, not a theoretical one
- `foreign_keys = ON` **only where foreign keys exist** — `analytics.db` sets it on a schema that declares none

**Connection reuse, not per-call open.** `HooksDb::open()` currently opens a connection, sets pragmas and runs two `CREATE TABLE` statements **on every hook fire** — which is every tool call, every file read, every loop iteration (`G3a SK-21`). `git/checkpoint.rs` opens a fresh unpooled connection per operation with no busy timeout (`G3b GI-27`).

### 3.3 Timestamps — one format, one helper

**The defect class:** SQLite `datetime('now')` produces `2026-08-31 14:22:05` (space separator, no offset). RFC 3339 requires `T` and an offset. Parsing one as the other always fails, and `.ok()?` / `.unwrap_or(0)` turns the failure into a plausible answer.

Three confirmed sites, each disabling a feature:

| Site | Consequence |
|---|---|
| `orphan/mod.rs:183` | The **4-hour orphan TTL can never fire** — any task claimed without a PID is stuck permanently, and there is no manual release either (`GI-01`, `GI-114`) |
| `commands/task_commands.rs:133-137` | **Every `qc_review` task reports age 0 s**, so the stalled-review alert can never fire (`F-45b`) |
| `analytics/db.rs:421`, `:614` | An RFC 3339 string compared as **text** against `datetime()` output; `T` (0x54) sorts above space (0x20), so **every row from the cutoff date passes regardless of time** — the "last hour" filters are decorative (`IN-59`, `IN-60`) |

**The rule: one timestamp type, one write helper, one read helper. No ad-hoc parsing anywhere.**

### 3.4 Timezones

`get_today_activity` builds its date from `chrono::Local` and compares it against `DATE(timestamp)`, which SQLite evaluates in **UTC**. `run_daily_rollup` does the same. For a user at UTC+10, every `daily_costs` row **omits 10 hours of that day and includes 10 hours of the next** (`IN-61`, `IN-62`).

**The rule: store UTC, convert at the presentation boundary only.**

### 3.5 Indexes

**Never wrap an indexed column in a function in a `WHERE` clause.** `DATE(timestamp)` appears at seven sites in `analytics/db.rs` and defeats `idx_token_usage_timestamp` on every one, turning each dashboard load into a stack of full table scans (`IN-63`). Use half-open range predicates on the raw column instead.

Missing indexes to add: `hooks(hook_type)` — the hot path of every hook fire, currently a full scan twice per tool call (`SK-22`).

### 3.6 Retention (OQ-5)

Unbounded, with no pruning and no `VACUUM` anywhere: `token_usage`, `system_events`, `api_failures`, `worker_stats` (`IN-64`) and `hook_logs` (`SK-23`). Combined with §3.5, a year of use produces a multi-hundred-MB database that is full-scanned on every dashboard open.

**Every append-only table needs a stated retention policy before it ships.** → OQ-5.

### 3.7 Ownership

**One owner per table** (INV-10). Two violations to resolve:
- **`app_settings` has three independent creators** — `commands/workspace_commands.rs:23`, `commands/github_mcp_commands.rs:30`, `ide/workspace.rs:35`, each with its own `CREATE TABLE IF NOT EXISTS`.
- **Two checkpoint systems** *(resolved by `P1-16`: `db/checkpoints.rs` was **deleted**, never having been compiled, so no second store was ever created — zero references to `checkpoints.db` remain and none exists on disk; `git/checkpoint.rs` is the sole owner)* — `checkpoints` in `config.db` (`git/checkpoint.rs`) and `file_checkpoints` in `checkpoints.db` (`db/checkpoints.rs`, **not compiled**). Pick one; the uncompiled one is why undo does not exist.

Also delete or create `hourly_costs` — `analytics/queries.rs:29` queries a table `migrate()` never creates and masks the failure as `-1`, in the one diagnostic written to answer "is the dashboard hitting the right database" (`IN-54`).

### 3.8 Correctness of writes

- **Multi-write operations are transactional.** `reconcile_stranded_task` issues one `update_status` per match with no transaction, so a crash leaves the board half-promoted (`SK-34`).
- **Conditional updates are one statement.** The task claim must include `assigned_worker` in its `WHERE`, not check ownership separately (spec `04`).
- **No `INSERT OR REPLACE` on a table with `ON DELETE CASCADE`** unless the cascade is intended — in `graph/` it silently deletes edges graph-wide (`F-22`).
- **No silent enum coercion.** `TaskPriority::from_str` maps every unrecognised string to `P2` via a `_` arm (`G0 §5.8`).

---

## 4. Test gates

| Gate | Asserts | Catches |
|---|---|---|
| **G-MIGRATE-FRESH** | Every ladder applies cleanly to an empty database | — |
| **G-MIGRATE-HEAD** | Every ladder applies cleanly from the live head; **a malformed statement fails the test** | F-31 |
| **G-MIGRATE-BACKUP** | A destructive migration refuses to run without a recorded backup | The absent backup path |
| **G-TIME-ROUNDTRIP** | Every timestamp written is readable by its reader; property test over a year of instants across DST boundaries | GI-01, F-45b, IN-59/60 |
| **G-TZ** | "Today" is the user's today at UTC±12 | IN-61, IN-62 |
| **G-INDEX** | `EXPLAIN QUERY PLAN` on every dashboard query shows an index, not a scan | IN-63 |
| **G-OWNER** | Static check: exactly one `CREATE TABLE` per table name across the workspace | `app_settings` × 3; the `src-shared` mirror |
| **G-CONCURRENT** | Two processes writing the same database both succeed | The missing `busy_timeout` |
| **G-RETENTION** | Every append-only table has a policy, and pruning keeps it under its cap | IN-64, SK-23 |
| **G-TXN** | A crash injected mid-multi-write leaves a consistent board | SK-34 |

---

> No ADR decision is recorded in this spec. It hands **ADR-01-03** and **ADR-01-09** to `00 §Part 8`, and **OQ-5** to §Part 9.
