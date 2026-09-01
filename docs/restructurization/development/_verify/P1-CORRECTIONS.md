# P1 — plan corrections from the §0.2m adversarial pass

**Written 2026-09-01, before the first line of P1 code.** Two read-only refutation
verifiers (`V4` groups A/B, `V5` groups C/D) were briefed to *refute* and to default
to REFUTED on ambiguity. Every load-bearing result below was re-verified in the main
session, as §0.2m requires.

This file **amends** `MDP §7`. Where a task line there disagrees with this file, this
file wins until §7 is regenerated at P1 closeout.

**Headline:** of 16 verified claims, **6 confirmed as written**, **7 confirmed but
misstated in a way that changes the work**, **2 refuted as stated**, and **1 task is
impossible as written and would have produced a green gate with zero effect.**

---

## A · Razi's ruling

**`P1-05` — full extraction, break the cycles.** Asked because the naive "move five
directories" is refuted. Razi chose the complete job over the narrow one: break all
four crate cycles, then move `agent/` as well. Recorded as `ADR-LOG-RESTRUCT-0011`.

---

## B · Corrections that change what gets built

### `P1-11` — **IMPOSSIBLE AS WRITTEN.** The repair must be a new `migration_v6`
Editing `migration_v5` in place changes nothing. Every existing graph DB is already
stamped `user_version = 5`, and `graph/migrations.rs:37` gates on `if current < 5`, so
the corrected code is never invoked.

Worse, the reason it is stamped: all six statements in v5 are `let _ = …`, so the
invalid `ADD COLUMN IF NOT EXISTS` **failed silently**, the migration stamped
`user_version = 5`, committed, and logged "✅ complete". Inert-but-green — the exact
class this programme exists to remove, sitting inside the migration system itself.

→ Write **`migration_v6`** that adds the columns v5 failed to add, idempotently. Fixing
v5 in place is the textbook "fixed it, gates green, nothing changed" outcome.

### `P1-19` — **MOVED TO FIRST.** The backup helper is a precondition, not a footnote
`P1-11` and `P1-14` both carry "⚠ back up first" and `P1-19` is listed *after* both.
There is no SQLite backup capability in the tree at all: no `VACUUM INTO`, no
`Connection::backup`, and rusqlite's `backup` feature is not enabled.

Two existing helpers do **not** count: `backup_recipe_file` is real but YAML-only, and
`create_restore_point` writes a size/mtime manifest that **nothing ever reads**.

Compounding it: `~/.airs/autodev.db` is a 30 May file carrying a **4.1 MB
uncheckpointed WAL from 1 June**. A naive `fs::copy` in `P1-14` silently discards that
window. Any backup helper must checkpoint or copy the `-wal` alongside the `.db`.

*(The manual §0.8 backup taken at the start of this phase does capture the WAL —
verified byte-identical by checksum, not by size.)*

### `P1-02` — **REVERSED.** Do not declare `db/checkpoints.rs`; delete it
Its stated justification — *"the undo feature is currently absent from the shipped
app"* — is **false**. Undo ships: `pub mod checkpoint;` at `git/mod.rs:21`, five
commands registered at `lib.rs:386-390`, `src/stores/gitStore.ts:238` calls
`list_checkpoints`, and a snapshot is taken before every agent write
(`edit_file.rs:177`, `write_file.rs:124`).

`db/checkpoints.rs` is genuinely uncompiled and has **zero callers**, so
`checkpoints.db` does not exist today. Declaring it would *create* the second
checkpoint store that `P1-16` then has to merge — manufacturing the problem in order
to solve it two tasks later. Right now `P1-16` is a one-way delete, and **that window
closes the moment `P1-02` lands as written.**

→ Delete `db/checkpoints.rs`. `P1-16` collapses to "confirm one owner and remove the
dead one". Recorded as `ADR-LOG-RESTRUCT-0012`.

Retraction of the false claim, which I had propagated into my own gate code and four
documents: `ADR-LOG-RESTRUCT-0010`.

### `P1-01` — **SCOPE MADE EXPLICIT.** Compiling is not wiring
Counts corrected: `graph_tools.rs` has **6** E0609 errors and 1 warning, not 2, and the
first five are at `:296-300`, not `:272`. It defines **27** structs, not 22.

None of the 27 implements `Tool`, and nothing registers them. So `P1-01` + `P1-03`
produce 840 lines that **compile and are still unreachable by the agent** — the
reachability gate goes green while the capability remains absent.

→ Declare and fix the compile errors (the task's `CODE-PRESENT` class), and record in
the same step that **the graph tools are still not available to the agent**; wiring
them is P3/BUG-GRAPH. A green gate must not be mistaken for a working feature.

### `P1-05` — **REFUTED AS STATED**, now scoped by Razi's ruling
Tauri coupling is **6 lines in 2 files** (`agent/engine.rs:9,40,57`,
`agent/tool_executor.rs:7,30,47`); all 255 `#[tauri::command]` attributes live in
`commands/`; `provider/`, `memory/`, `platform/` have **zero**.

The real blocker is **four crate cycles**, which Cargo rejects outright:

| Cycle | Refs |
|---|---|
| `agent` ↔ `recipes` | 13 out / 9 back |
| `agent` ↔ `subagent` | 5 / 11 |
| `agent` ↔ `commands` | 1 / 38 — `agent/builtin_tools/analyze_diff_tool.rs:92` calls `crate::commands::diff_commands::generate_branch_diff_pub` |
| `agent` → `diff_analysis` → `provider` | — |

`agent/` also holds **114 references to 17 modules that would stay behind**.

**`security/` does not exist.** `P1-05` says move it; `P1-20` *creates* it later in the
same phase. It cannot be part of the move.

### `P1-06` — **ALL THREE NUMBERS WRONG**
The MDP says "132 `emit_*` sites across 27 files, 11 inside `builtin_tools/`".
Measured: **71 call sites in 24 files, 21 of them in `builtin_tools/` across 11 files**.
The 132/27 counts `events.rs`'s own definitions as call sites, and the "11" is a file
count sitting in a list of site counts. The *mechanism* — a global
`OnceLock<AppHandle>` — is confirmed.

### `P1-09` — **MIXED: overstated for `tasks`, understated for `workers`**
`tasks` is drifted but **compatible**; its 18 `ALTER`s self-heal. `plans` differs in
primary-key *type* (INTEGER AUTOINCREMENT vs TEXT/UUID) — genuinely incompatible.

**`workers` has already failed in production.** `src-shared/src/db/workers.rs` selects
`last_seen`/`notes` at `:85,:93,:106,:108,:110,:112` and updates `last_seen` at `:143`;
the live `~/.autodev/workers.db`, written by `src-tauri`, has **neither column**. Every
`airsctl` worker list/get/heartbeat call errors today. This is not a latent risk.

### `P1-13` — **MISDIRECTED.** One site, not "3+", and the stated cause is wrong
Seven `parse_from_rfc3339` sites exist, but **only one** reads a `datetime('now')`
column: `orphan/mod.rs:183`. It fails **100% of the time**, and `.ok()?` turns the
failure into *"not an orphan"* — which is why the orphan TTL can never fire, a defect
already in the register from a different direction.

And the mechanism in the plan is wrong: chrono **does** accept the space separator; it
is the **missing timezone offset** that breaks it. The five analytics sites work today
because they read columns written by chrono itself.

### `P1-14` — **CANNOT BE COMPLETED AS DESCRIBED**
`~/.airs/plugins/` and `~/.airs/fleet_secret` **do not exist** — they are code-only
paths. The real payload is **1 session and 14 messages**, nothing written since 1 June.

Two traps:
- `src-shared/src/config.rs:14` **`create_dir_all`s `~/.airs/` on every call**, so the
  root cannot simply be moved and deleted — it will be recreated. *(Found by V5's own
  mid-task self-correction, after it first reported the opposite.)*
- The live `messages` table has `tool_calls` and `tool_results` columns that the code's
  current `CREATE TABLE` **no longer declares**. A migration written from the source
  schema silently **drops real content**.

### `P1-15` / `P1-16` — **MIS-ORDERED.** One-owner steps must precede the ladders
A `user_version` ladder on `config.db`, which has **seven independent migrators**, is
inert-but-green. Every "collapse to one owner" step (`P1-09`, `P1-15`, `P1-16`) runs
**before** `P1-10`.

`app_settings` has exactly three creators. They agree on all three columns and
**disagree on the `updated_at` trigger** — only `github_mcp_commands.rs` creates it, so
first-caller-wins decides whether the trigger exists.

### `P1-20` — **COUNT WRONG, AND HALF THE PORT IS MISSING**
`SENSITIVE_KEYS` has **22** entries, not 20. And Python ships a **second** redactor for
PII that the task does not mention. Porting only the first is the same half-a-verdict
error that lost redaction in the first place (`L-1`).

---

## C · Confirmed as written

`P1-03` · `P1-04` · `P1-07` (with the caveat that `agent/` drags Tauri into the CLI
until the cycles are broken — it would red `cli-checks`, which installs no GTK/webkit)
· `P1-10` (**1 of 11** databases has a ladder; the rest report `user_version = 0`) ·
`P1-12` · `P1-17` (one query at `analytics/queries.rs:35`, no `CREATE TABLE` anywhere,
absent from the live DB, and `.unwrap_or(-1)` masks it permanently) · `P1-18` ·
`P1-21` · `P1-22`.

`P1-08` is a doc task and stands.

---

## D · Revised execution order

The MDP's order is unsafe in three places. Corrected:

1. **`P1-19` — backup helper FIRST.** Everything destructive depends on it.
2. **`P1-A`**: `P1-01`, `P1-02` (delete, not declare), `P1-03`, `P1-04`.
   *Before `P1-05`* — `reachability_gate.rs` asserts `KNOWN_UNREACHABLE` paths exist
   under `src-tauri/src/`, so moving `agent/` first fails that test with a misleading
   message.
3. **`P1-B`**: break the four cycles → `P1-05` full extraction → `P1-06` → `P1-07`.
   *`P1-06` before `P1-07`*, or `cli-checks` goes red.
4. **`P1-C`**: one-owner steps (`P1-09`, `P1-15`, `P1-16`) → `P1-10` ladders →
   `P1-11` as **v6** → `P1-12`, `P1-13`, `P1-14`, `P1-17`, `P1-18`.
5. **`P1-D`**: `P1-20` (both redactors) → `P1-21` → `P1-22`.

---

## E · Destructive-risk ranking (§0.8)

| Risk | Task | What could be lost |
|---|---|---|
| **HIGH** | `P1-14` | The 4.1 MB uncheckpointed WAL, and the `tool_calls`/`tool_results` columns a source-derived migration would drop |
| **HIGH** | `P1-11` | The live graph DB, if v6 is written without idempotency |
| **MED** | `P1-16` | Checkpoint history in `config.db` if the wrong owner is deleted |
| **MED** | `P1-10` | Any DB whose ladder is authored against a schema that differs from what is on disk |
| **LOW** | `P1-09`, `P1-15` | Schema-only, no data movement |

**Backup taken and checksum-verified before any of this**:
`~/autodev-backups/p1-20260901-113442` — 8 databases plus the 4.1 MB WAL, all
byte-identical to source.
