# A3 — P1 Work Audit

**Auditor:** A3 (P1 Work Auditor)
**Date:** 2026-09-01
**Scope:** MASTER_DEVELOPMENT_PLAN.md §7 · Phase P1 — tasks P1-01…P1-22, smokes S1-01…S1-04
**Mode:** READ-ONLY on codebase. Only this file written — `git status` over `autodev-core src-tauri src-cli src-shared src docs .github Cargo.toml Cargo.lock` shows no modified tracked file.
**Status:** COMPLETE. All 26 units audited.

---

## 1 · Headline

I audited all **26** P1 units — tasks `P1-01`…`P1-22` and smokes `S1-01`…`S1-04` — against the evidence each one claims, re-running `cargo check`, `cargo test --workspace --all-targets` (185 passed, 0 failed), `cargo clippy`, `cargo fmt`, `cargo tree`, `cargo build` per crate, and reading all ten live databases directly. **17 verdicts are CONFIRMED, 6 OVERSTATED, 3 REFUTED**, and I raise **16 findings — 2 BLOCKER, 6 MAJOR, 8 MINOR**. Most of the phase is genuinely built and unusually honestly documented; the arithmetic in three task notes does not survive recounting, but nothing structural was faked. **The single most serious finding is W-10: `P1-21`'s redaction is wired into `ToolExecutor::execute`, a function with *no production caller*. The agent engine calls `ToolExecutor::execute_batch` (`autodev-core/src/agent/engine.rs:314`), which redacts nothing — so a secret read by `read_file` still reaches the model's conversation, the `messages` table on disk, and the `tool_end` UI event verbatim. `S1-04` passes because the test itself calls the dead path, and `TG-P1-06` passes because it only greps the file for the string `security::redaction`. M-40 is inert-but-green: the exact defect class this phase's own learnings register was written to prevent.** Second is W-13: `delete_graph` (`src-tauri/src/commands/graph_commands.rs:824`) `fs::remove_file`s an entire database from a UI button with no backup, so `P1-19`'s "wired into every destructive path" is false.

---

## 2 · Per-task verdicts

| id | claim (abbreviated) | verdict | what settles it |
|---|---|---|---|
| **P1-01** | graph modules declared and compile; 27 structs; 0 implement `Tool`; nothing registers them | **CONFIRMED** | `autodev-core/src/agent/builtin_tools/mod.rs:13-14`; `grep -c "^pub struct"` → 27; `grep "impl .*Tool"` → 27 *inherent* impls, 0 `impl Tool for`; register-grep for graph → exit 1 |
| **P1-02** | reversed — `db/checkpoints.rs` deleted; undo ships via `git/checkpoint.rs` | **CONFIRMED** | file absent; `autodev-core/src/git/mod.rs:21`; `src-tauri/src/lib.rs:386-390` (five commands); `src/stores/gitStore.ts:238` |
| **P1-03** | six E0609s fixed; `cargo check --workspace` exits 0 | **CONFIRMED** | `cargo check --workspace --all-targets` → `[exited with code 0]` |
| **P1-04** | `KNOWN_UNREACHABLE` empty; gate widened to four crates | **CONFIRMED** | `src-tauri/src/reachability_gate.rs:57` `= &[];`; `:133-141` four crate roots; `cargo test -p autodev --lib reachability` → `2 passed` |
| **P1-05** | 182 files moved; 0 Tauri edges in core, 28 for desktop | **CONFIRMED** | `git show eab345c --name-status -M \| grep autodev-core/src \| wc -l` → **182** (5 A + 177 R); `cargo tree -p autodev-core \| grep -icE "tauri\|gtk\|webkit"` → **0**; `-p autodev` → **28** |
| **P1-06** | "19 `emit_X`/`emit_X_global` **pairs** collapsed to one each" | **OVERSTATED** | pre-P1 `events.rs`: 34 emit fns = 19 app-taking + 15 `_global`, over **25** base names, of which only **9** are true pairs; post-P1 = 26 fns. Mechanism confirmed (`events.rs:32,45,58`; `src-tauri/src/main.rs:206`; `src-cli/src/main.rs:130`; 4 tests) — **W-01** |
| **P1-07** | both binaries link core; CLI builds the runtime with no UI | **CONFIRMED** | `src-cli/tests/core_runtime.rs` (3 tests, pass); `cargo tree -p airsctl` UI-edge count 0 |
| **P1-08** | INV-1 restated runtime-and-language-scoped | **CONFIRMED** | `docs/restructurization/vision/00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md:120` and `:1151`, with the unsatisfied P8 half inline |
| **P1-09** | mirrors deleted (22/19/23 write statements); airsctl on core | **OVERSTATED** | deletion + repoint confirmed (`src-shared/src/` is now 5 files; `src-cli/src/commands/{task,worker,plan}.rs:3`). The counts are not reproducible: `workers.rs` has **7** write statements under every metric tried, and 7 at every revision in its history — **W-02** |
| **P1-10** | every DB has `PRAGMA user_version` and a numbered ladder; 7 of 7 at v1 | **CONFIRMED** | 9 production `run_ladder` sites + the graph ladder = 10; live read: 9 × `v1`, graph `v6`; `migrate.rs:80,88-93,127-150,174-211` |
| **P1-11** | v5's dead statements removed; defensive `migration_v6`; premise retracted | **CONFIRMED** | `graph/migrations.rs:115-150` (six ALTERs gone), `:167-221` (v6 via `ensure_columns`); retraction verified against `graph/schema.rs:57-60,78-79`; 4 tests; live graph store at **v6** |
| **P1-12** | one `open_tuned`, all 17 production sites; WAL everywhere | **OVERSTATED** | **17 exactly** in core (18 hits − 1 in `db/mod.rs`'s test mod); all 10 live DBs read `journal_mode=wal`. But `src-shared/src/db/analytics.rs:55` opens the live `analytics.db` with a plain `Connection::open`, outside `TG-P1-05`'s reach, contradicting `TG1-06`'s "every connection" — **W-03** |
| **P1-13** | one timestamp helper; orphan TTL can fire; fresh-schema guard "verified against the real file" | **OVERSTATED** | helper + 7 callers + orphan fix confirmed (`db/timestamps.rs:57,84,93`; `orphan/mod.rs:195,271`). The guard compares against the hardcoded `EXPECTED_COLUMNS` (`db/tasks.rs:1775-1808`, 32 entries), **not** the live file — **W-05**; misattached doc comment at `:1809-1818` — **W-09** |
| **P1-14** | "All **nine** `~/.airs` path references repointed" | **OVERSTATED** | every *path* is repointed (`src-shared/src/config.rs:13`, `plugin/token.rs:108`, `plugin.rs:644`, `updater/persistence.rs:44`) and `legacy_root.rs` is sound. But **7 doc strings still say `~/.airs`**, two of them clap help text users see — **W-06** |
| **P1-15** | one owner for `app_settings`; `config_db.rs` owns the whole file | **OVERSTATED** | `app_settings` half confirmed (sole creator `db/config_db.rs:30`; three callers repointed; `TG-P1-04` at `ci_gate.rs:628` with a working falsifier; live `config.db` **has** `update_app_settings_ts`). The file-owner half is false: live `config.db` has **6** tables; `git/checkpoint.rs:30-46` and `hooks/db.rs:17-58` create the other three outside the ladder — **W-07** |
| **P1-16** | one checkpoint system; zero `checkpoints.db` references | **CONFIRMED** | `grep -rn "checkpoints.db"` over `.rs/.ts/.tsx` in all crates + `src/` → **exit 1, no output**; no such file on disk |
| **P1-17** | `hourly_costs` deleted, not created | **CONFIRMED** | `rg hourly_costs` → zero code hits; `DIAGNOSTIC_TABLES` (`analytics/queries.rs:22-28`) equals the live `analytics.db`'s five tables exactly |
| **P1-18** | run ledger live in both binaries; **"nothing is stamped yet"** | **CONFIRMED** | live `runs.db`: 3 rows, 2 `ok` + 1 `failed`, all closed — exactly as claimed. **No table in any of the ten live DBs has a `run_id` column.** ⚠ but `reap_abandoned` (`db/runs.rs:233-251`) will mark a *live* concurrent peer's run `abandoned` — **W-08** |
| **P1-19** | backup helper wired to **the two** wholesale operations | **REFUTED** | the two wirings are real (`migrate.rs:115`, `tasks.rs:733`) and well tested; but `delete_graph` (`src-tauri/src/commands/graph_commands.rs:824`, UI-invoked at `src/tabs/Graph.tsx:691`) `fs::remove_file`s a whole database with no backup — **W-13**; and `is_destructive` misses `DROP COLUMN`/`UPDATE`, while the graph ladder bypasses `run_ladder` altogether — **W-04** |
| **P1-20** | both Python redactors ported; 22 `SENSITIVE_KEYS` | **CONFIRMED** | `security/redaction.rs:65-88` — I counted **22**; all `PIIRedactor` rules at `:108-155` with the ordering fix; 11 tests incl. `:311` and `:352` pinning the free-text deviation |
| **P1-21** | redaction "at **every** boundary" | **REFUTED** | complete inventory: 12 production `security::redaction` lines in 4 files. Memory ✓. Prompts ✗ (redaction lives in `ToolExecutor::execute`, **zero production callers**) — **W-10**. Logs ✗, analytics ✗, egress ✗ — `grep -rln redact` over `observability/`, `analytics/`, `provider/` all exit 1 — **W-11** |
| **P1-22** | `TG8-02` exists in `TESTING_GATES.md §11` | **CONFIRMED** | `TESTING_GATES.md:513-515`, correctly left `- [ ]` as a P8 gate. ⚠ its tenth-row evidence cites `S1-04`, which W-10 invalidates |
| **S1-01** | `cargo build` clean, all four crates | **CONFIRMED** | four separate `cargo build -p …` runs, all `Finished` + `exit=0` |
| **S1-02** | `airsctl --version` → `airsctl 1.0.0`, exit 0; core edge, zero UI edges | **CONFIRMED** | `./target/debug/airsctl.exe --version` → `airsctl 1.0.0`, `exit=0`; `cargo tree -p airsctl` line 3 = `autodev-core v1.0.0`; UI-edge count 0 |
| **S1-03** | all 10 DBs open, stamped, survive a restart | **CONFIRMED** (partial) | `examples/migrate_check.rs:43-140` covers 9 named + every `graphs/*.db`, exiting 1 on any `v0`; live read gives 9 × `v1` + graph `v6`. **"run twice, byte-identical" is UNVERIFIABLE by me** — my mandate forbids running it |
| **S1-04** | a secret read by a tool is redacted in the prompt and in memory | **REFUTED** | the test (`autodev-core/tests/s1_04_secret_never_reaches_the_prompt.rs:60`) calls `executor.execute(…)`, which nothing in production calls. Memory half genuinely proven by `tests/redaction_boundaries.rs` — **W-10** |

---

## 3 · Findings

### W-10 · `BLOCKER` — redaction is wired into a code path that never runs

**Claimed** (`MASTER_DEVELOPMENT_PLAN.md`, P1-21): *"Wired at `agent/tool_executor.rs::execute` — the single funnel **every** tool result passes through, so it covers the conversation, the `messages` table and the UI event at once."*

**Actually true.** `ToolExecutor` is constructed in exactly two places workspace-wide: `autodev-core/src/agent/engine.rs:125` and the S1-04 test. Production calls **only** `execute_batch`:

```
autodev-core/src/agent/engine.rs:314:                tool_executor.execute_batch(tool_batch).await?
```

`ToolExecutor::execute` (`autodev-core/src/agent/tool_executor.rs:94`) is the only method containing redaction:

```
autodev-core/src/agent/tool_executor.rs:218:        result.output = crate::security::redaction::redact_text(&result.output);
autodev-core/src/agent/tool_executor.rs:219:        let redacted_input = crate::security::redaction::redact_args(&input);
```

It has **no production caller**. `execute_batch` (`:246`) redacts nothing — its read-only concurrent branch emits raw input at `:311`, raw output at `:322`, persists raw output to the `messages` table at `:344-352`, and returns the raw result into the conversation at `:356`; the write branch repeats it at `:469-484`. `read_file` is read-only, so `read_file(".env")` takes exactly that branch.

**Both proofs are inert-but-green.** `S1-04` calls `executor.execute(…)` — the dead path — and its own header asserts the false premise *"because every tool result funnels through one place."* `TG-P1-06` (`src-tauri/src/ci_gate.rs:802`) tests `production.contains("security::redaction")` **per file**, which `tool_executor.rs` satisfies via `execute`.

**Severity: BLOCKER.** M-40 / `G-REDACT` ("appears redacted in the prompt, in the logs, and in the memory store", `docs/restructurization/vision/03-CONTEXT-AND-MEMORY.md:245`) is unmet on the shipped path.
**Resolution:** redact in `execute_batch`'s two branches (or make `execute_batch` delegate to `execute` per tool), and rewrite `S1-04` to drive the engine's actual call so the smoke exercises the path the agent uses.

### W-13 · `BLOCKER` — a third wholesale-destruction path, unwired to the backup helper

**Claimed** (`MASTER_DEVELOPMENT_PLAN.md`, P1-19): *"Wired to **the two** genuinely wholesale operations, deliberately not to row-level deletes."*

**Actually true.** There is a third, and it destroys more than either:

```
src-tauri/src/commands/graph_commands.rs:811   pub async fn delete_graph(repo_path: Option<String>) -> Result<()> {
src-tauri/src/commands/graph_commands.rs:824       std::fs::remove_file(&db_path)?;
```

This deletes the **entire** per-repo graph database file (`~/.autodev/graphs/d0ef18f6ad8b.db` — 126 KB, 12 tables, `user_version = 6`). It is registered (`src-tauri/src/lib.rs:85`, `:430`) and reachable from the UI in one click (`src/tabs/Graph.tsx:691` — `await invoke('delete_graph', { repoPath: workspaceDir })`). No `backup_before`. `fs::remove_file` is unrecoverable.

**Severity: BLOCKER** for `P1-19` as written ("wire it into **every** destructive path (GAP-11)") — a two-item enumeration presented as exhaustive that is not.
**Resolution:** call `db::backup::backup_before` in `delete_graph` before `remove_file`, failing the command if the backup fails; then re-word `P1-19` to say three paths and state the enumeration method used.

### W-07 · `MAJOR` — `config_db.rs` is not the owner of `config.db`

**Claimed** (`autodev-core/src/db/config_db.rs:1,3`): *"the schema owner for the whole file"*, *"**Three** modules keep tables here"*; and P1-10: *"`db/config_db.rs` now owns the connection, the schema and the version."*

**Actually true.** The live `config.db` holds **six** tables: `app_settings, autodev_instructions, checkpoints, hook_logs, hooks, model_configs`. Two further modules open the same file directly and create the other three outside the ladder:

- `autodev-core/src/git/checkpoint.rs:30-46` — `crate::db::open_tuned(&autodev_dir()?.join("config.db"))` then `conn.execute_batch("CREATE TABLE IF NOT EXISTS checkpoints (…)")`
- `autodev-core/src/hooks/db.rs:17-58` — same open, then `CREATE TABLE IF NOT EXISTS hooks (…)` / `hook_logs (…)`

Neither consults or advances `user_version` — the very "`CREATE TABLE IF NOT EXISTS` in an `open()` path is not a migration" pattern that `autodev-core/src/db/migrate.rs:5-6` names as the defect P1-10 exists to remove. And `TG-P1-04` iterates a hardcoded three-table list (`src-tauri/src/ci_gate.rs:629`), so those three tables are ungated.

**Severity: MAJOR** — "one owner per schema" is two-thirds met for this file, and the gate cannot see the gap.
**Resolution:** move the `checkpoints`/`hooks`/`hook_logs` DDL into `config_db.rs`'s ladder as `v2`, have both modules open through `config_db::open()`, and add all six names to `TG-P1-04`.

### W-08 · `MAJOR` — `reap_abandoned` closes a *live* peer's run

**Claimed** (P1-18): *"`reap_abandoned` runs at startup and closes rows belonging to a pid/host that is not this process."* — accurate as a description, and that is the problem.

**Actually true.** `autodev-core/src/db/runs.rs:236-245`:

```
"UPDATE runs SET ended_at = ?1, outcome = ?2 \
 WHERE outcome IS NULL AND NOT (pid = ?3 AND host = ?4)"
```

There is no liveness check. Desktop app running (run A open, pid X) → the user runs any `airsctl` command → `airsctl`'s startup reap marks run A `abandoned` → the app's `finish` hits `rows == 0` and returns "no open run … to close". A normal session is recorded as abandoned and the app logs a spurious warning. Two binaries writing one database at once is not an edge case: `TESTING_GATES.md:128` (`TG1-06`) makes it the requirement.

The correct primitive already exists in the same crate: `autodev-core/src/orphan/pid_checker.rs:7 pub fn is_pid_alive(pid: u32) -> bool`, used at `autodev-core/src/orphan/mod.rs:165`. `runs.rs` does not call it. The test `a_run_left_open_by_a_dead_process_is_reaped` (`autodev-core/src/db/runs.rs:339`) plants `host='other-host', pid=999999`, so it passes identically with or without a liveness check.

**Severity: MAJOR.**
**Resolution:** gate the reap on `!is_pid_alive(pid)` for same-host rows, keeping the host-mismatch rule (or a staleness window) for other hosts.

### W-11 · `MAJOR` — `TG-P1-06` names five boundaries and gates two of them

`src-tauri/src/ci_gate.rs:779` says *"Each entry is a boundary named by `P1-21`: prompts, memory writes, **logs, analytics, egress**"*, but `BOUNDARIES` (`:780-793`) has four entries: `agent/tool_executor.rs` and the three memory modules. Measured:

```
$ grep -rln redact autodev-core/src/provider/        → exit 1   (egress)
$ grep -rln redact autodev-core/src/analytics/       → exit 1   (analytics)
$ grep -rln redact autodev-core/src/observability/ src-tauri/src/commands/log_commands.rs → exit 1  (logs)
```

Three of the five named boundaries have no gate entry and no redaction, so the gate **cannot** fail for them.

**Severity: MAJOR.**
**Resolution:** either wire those three and add them to `BOUNDARIES`, or delete them from the gate's comment and record them as deferred with an owning phase. A gate whose comment claims coverage it does not have is worse than an honest gap.

### W-06 · `MAJOR` — `airsctl --help` still tells users their config is in `~/.airs`

P1-14 claims *"All nine `~/.airs` path references repointed."* True of paths; false of two **clap doc comments**, which become user-visible help text:

- `src-cli/src/main.rs:22` — `/// Path to config file (default: ~/.airs/config.toml)` on `Cli::config`
- `src-cli/src/commands/config.rs:8` — `/// Create default config at ~/.airs/config.toml` on `ConfigCmd::Init`

`src-shared/src/config.rs:13` in fact resolves `.join(".autodev")`. Five more stale strings are internal comments: `src-shared/src/config.rs:63,74,82`, `src-cli/src/commands/plugin.rs:540`, `autodev-core/src/plugin/token.rs:7`. A sixth, `autodev-core/src/agent/context.rs:41` (`// LEGACY: ~/.airs/CLAUDE.md`), labels code that reads `~/.autodev/config/autodev_global.md` (`:44-47`) — historical in intent but now mislabelling the line beneath it.

**Severity: MAJOR** for the two help strings (a docstring promising different behaviour than the code is a bug), MINOR for the rest.
**Resolution:** replace `~/.airs` with `~/.autodev` in all seven.

### W-04 · `MAJOR` — the automatic-backup net has two holes

`autodev-core/src/db/migrate.rs:56-70`:

```rust
["drop table","drop index","drop view","drop trigger",
 "delete from","rename to","rename column","truncate"]
```

1. **`ALTER TABLE … DROP COLUMN` is absent**, as is a bare `UPDATE … SET` rewrite — yet `TESTING_GATES.md:120` (`TG1-04`) states the rule covers migrations that "drop, truncate, **rewrite or type-change** a column".
2. **The graph ladder never reaches `is_destructive`.** `autodev-core/src/graph/migrations.rs:12 run_migrations` is hand-rolled, not `migrate::run_ladder`. `migration_v4` (`autodev-core/src/graph/migrations.rs:105`) executes `DROP TABLE IF EXISTS nodes_fts` with no backup. Harmless on today's `v6` store, but any database at `v3` and every future graph migration inherit the gap.

**Severity: MAJOR.**
**Resolution:** add `"drop column"` (and consider `"update "`) to `is_destructive`; move the graph ladder onto `migrate::run_ladder`.

### W-14 · `MAJOR` — `TG1-01` is ticked while its own named minima are unmet

`MASTER_DEVELOPMENT_PLAN.md:534` states `TG1-01` is *"recorded as short of its registration clause **rather than ticked**"*. `TESTING_GATES.md:109` reads `- [x] **TG1-01**`. The gate's named minima require the graph tools' "tools/commands are registered"; `TESTING_GATES.md:112` concedes that half is unmet. Two governing documents disagree about a checkbox, and the checkbox says done.

**Severity: MAJOR** — a green gate over an unmet clause is the failure mode `§0.2e` exists to stop.
**Resolution:** flip `TG1-01` to `- [ ]` with the met half recorded, or split the registration clause into a P3-owned gate and re-scope `TG1-01` to reachability only — then correct whichever document is wrong.

### W-01 · `MINOR` — `P1-06`'s "19 pairs" is not a count the source can produce

Pre-P1 `src-tauri/src/events.rs` had **34** `pub fn emit_` functions: **19** taking `app: &AppHandle` and **15** suffixed `_global`, spanning **25** base names. Only **9** base names had both forms — those are the pairs (`budget_exceeded, plan_changed, qc_decision, subagent_event, task_changed, worker_log_line, worker_message, worker_request, worker_status`). Post-P1 there are **26** functions (25 + the new `emit_serialized`). "19 pairs collapsed to one function each" would leave 19.

**Severity: MINOR.**
**Resolution:** restate as *"34 emit functions (19 `&AppHandle`-taking + 15 `_global`) collapsed to 25, of which 9 were true duplicate pairs."*

### W-02 · `MINOR` — `P1-09`'s "22 / 19 / 23 write statements" is not reproducible

Against `eb9be2b~1`, `src-shared/src/db/workers.rs` (179 lines) contains **13 SQL statements**, of which **7** are writes (1 `CREATE TABLE`, 2 `CREATE INDEX`, 1 `INSERT`, 3 `UPDATE`). Other metrics tried: 17 SQL-keyword occurrences, 14 rusqlite calls, 10 functions. `git log --follow` shows the file never exceeded 7 write statements at any revision. `tasks.rs` gives 10 (claim 22), `plans.rs` 8 (claim 23).

**Severity: MINOR** — decorative figure; the substantive claim (mirrors deleted, airsctl repointed) is confirmed.
**Resolution:** recount with a stated method or drop the numbers. Commit `eb9be2b`'s message carries them too, so both need correcting.

### W-03 · `MINOR` — one production connection is outside the tuned path and outside the gate

`src-shared/src/db/analytics.rs:55` — `let conn = Connection::open(&path)?;` — opens the live `~/.autodev/analytics.db` (`:52`) with no `busy_timeout` and no `journal_mode`. `TG-P1-05` walks `autodev-core/src` only. `TESTING_GATES.md:128` claims "`busy_timeout` is set on **every** connection". Its comment also reads *"the Tauri app is responsible for the schema"* — stale since P1-10 gave `analytics.db` a ladder in core.

**Severity: MINOR** — WAL is persisted in the file header and rusqlite defaults `busy_timeout` to 5000 ms, so there is no live symptom.
**Resolution:** route it through `autodev_core::db::open_tuned`, or widen `TG-P1-05` to all four crates and record the exemption.

### W-05 · `MINOR` — "verified against the real file" describes an authoring act, not the guard

`a_fresh_database_has_every_column_an_existing_one_has` (`autodev-core/src/db/tasks.rs:1879`) compares a fresh DB's `PRAGMA table_info(tasks)` against the hardcoded `EXPECTED_COLUMNS` constant (`autodev-core/src/db/tasks.rs:1775-1808`, 32 entries), not against `~/.autodev/tasks.db`. If the live schema drifts the test stays green.

**Severity: MINOR.**
**Resolution:** reword to "asserts a fresh `tasks.db` matches the 32-column list transcribed from the live file", or add an opt-in live-comparison test.

### W-09 · `MINOR` — a doc comment is attached to the wrong test

`autodev-core/src/db/tasks.rs:1809-1818` explains the *fresh-database column regression* but sits immediately above `clearing_the_board_backs_up_first_and_the_backup_holds_the_tasks` (`:1821`), and runs straight into that test's own sentence at `:1819`. The test it actually describes (`:1879`) has no doc comment.

**Severity: MINOR.**
**Resolution:** move the paragraph to `:1878`.

### W-12 · `MINOR` — P1 left 15 dead-code warnings and the clippy gate cannot fail on them

`.github/workflows/ci.yml:74` runs `cargo clippy --workspace --all-targets` with no `-D warnings`, so it exits 0 over roughly 54 warnings. Fifteen are P1's own leftovers: unused `crate::db::autodev_dir` / `Connection` imports at `src-tauri/src/commands/github_mcp_commands.rs:6,8`, `src-tauri/src/commands/workspace_commands.rs:10,12`, `autodev-core/src/db/autodev_instructions.rs:6`, `autodev-core/src/db/model_configs.rs:5`, `autodev-core/src/ide/workspace.rs:14` (from P1-15/P1-10 routing through the owner — `git show 14aa9c9~1` shows `autodev_dir` had 3 uses in `github_mcp_commands.rs`, now 1); unused `app: AppHandle` at `src-tauri/src/commands/plan_commands.rs:67,81,110,145,177` and `src-tauri/src/commands/agent_commands.rs:100` (from P1-06's EventSink — that file had 10 `app` references pre-P1, 5 now); `autodev-core/src/orphan/mod.rs:225`; `autodev-core/src/session/storage.rs:351`.

**Severity: MINOR.**
**Resolution:** `cargo fix` the unused imports, prefix the unused params, and consider `-D warnings` on the clippy step.

### W-15 · `MINOR` — the "280 files" figure was wrong when written

`TESTING_GATES.md:110` ("all four crates (280 files)") and `src-tauri/src/reachability_gate.rs:126` ("84 files instead of 280"). `git log -S"280 files"` dates both to **`fd46044`**, where

```
$ git ls-tree -r --name-only fd46044 | grep -E "^(autodev-core|src-tauri|src-cli|src-shared)/src/.*\.rs$" | wc -l
292
```

Today it is also 292. The `84` for `src-tauri` is correct; the workspace total is 12 short.

**Severity: MINOR.**
**Resolution:** change 280 → 292.

### W-16 · `MINOR` — literal space runs inside user-visible strings

A formatter artifact left in string literals, two of them in P1 code that reaches users or logs:
`src-tauri/src/main.rs:211`, `autodev-core/src/db/migrate.rs:119`, `autodev-core/src/db/tasks.rs:1899`, `src-tauri/src/ci_gate.rs:734` (two more pre-date P1: `autodev-core/src/agent/builtin_tools/acknowledge_worker_message.rs:58`, `send_worker_message.rs:144`).

**Severity: MINOR.**
**Resolution:** collapse each run to a single space.

---

## 4 · What I could not determine

1. **`S1-03`'s "run twice, output byte-identical, exit 0 both times."** My mandate forbids running `examples/migrate_check`. I verified what the example *would* report by reading `user_version` from copies of all ten live databases, but the idempotence-across-two-runs claim is unreproduced. *(Note for whoever closes this: the example honours `USERPROFILE`/`HOME` — `autodev-core/examples/migrate_check.rs:8-13` documents pointing it at a copy — so it can be re-verified safely. I did not, because the instruction was absolute.)*
2. **`P1-04`'s falsification** ("planting an orphan in `autodev-core`: caught and named") and the equivalent plant-a-violation falsifications for `TG-P1-01`, `TG-P1-04`, `TG-P1-05`. Each requires editing the codebase, which I am forbidden to do. The gates' *detector* unit tests exist and pass, which is the next best evidence, but I did not re-run the end-to-end plants.
3. **`P1-09`'s live-DB assertions** — "7 rows preserved", "`worker list` exits 0", "heartbeat persists a timestamp `status` reads back", "unknown slug exits 1". Transient command results against the live `workers.db`. `~/autodev-backups/p1-09-preflight/` exists with a matching timestamp and the live `workers.db` opens at `v1`/`wal`, but I did not run `airsctl worker …`.
4. **Whether `execute_batch` was always the engine's call**, or whether `execute` was on the live path at some point during P1. I established the current state definitively; I did not bisect. It does not change the verdict.
5. **The intended semantics of `is_destructive` for `UPDATE`.** I flag the gap against `TG1-04`'s wording; whether the author meant to include column rewrites is a judgement for Razi, not something I can measure.
6. **Whether the two `~/.airs` clap strings surface anywhere beyond `airsctl --help`** (packaged man pages, docs site). I confirmed the source; I did not enumerate downstream consumers.
7. **Whether `P1-14`'s "nine `~/.airs` path references" is exactly nine.** The commit diff removes nine `.airs` lines from code, which is consistent, but "reference" is not defined precisely enough for me to call the number confirmed or refuted. I neither counted it as a finding nor as confirmed.

---

## 5 · What held up

Claims I attacked and could not break. Recording them so the phase gets credit where it is due.

1. **`P1-01`'s double negative.** I looked hard for a `Tool` impl or a registration for the 27 graph structs — `impl Tool for`, any `registry.register(...Graph...)`, any `graph_tools::` reference across four crates. There is none. The task's refusal to claim wiring is exactly right, and the pre-P1 LOC arithmetic (840 + 78 = 918) checks out against `6c67ebb~1`.
2. **`P1-05`'s numbers, to the digit.** 182 files (5 added + 177 renamed) and 0 / 28 Tauri edges both reproduce exactly on the default `cargo tree` invocation.
3. **`P1-11`'s retraction.** The claim that the six "lost" columns were never lost is verifiable straight from `autodev-core/src/graph/schema.rs:57-60,78-79` — `CREATE_NODES` declares all four, `CREATE_EDGES` both. A correction that was itself checked against state, and it holds.
4. **`P1-12`'s "17 production connection sites".** Exactly 17, after correctly excluding the one inside `db/mod.rs`'s own test module. A precise count, precisely stated.
5. **`P1-18`'s honesty clause.** "Nothing is stamped yet — no other table carries a `run_id` column" is easy to write and easy to falsify. I checked it two independent ways — grep over every `.rs` in the workspace, and `PRAGMA table_info` over every table of all ten live databases. **NONE.** It is true.
6. **The live ledger matches its description exactly.** Three rows, two `ok`, one `failed`, all closed, none open. A claim about state, verified against state.
7. **The `busy_timeout` correction.** "rusqlite overrides SQLite's default to 5000 ms, so this fixed nothing" is a self-deflating claim, recorded rather than hidden. Python's `sqlite3` also reports 5000 on these files, consistent with it.
8. **`P1-20` is simply good work.** 22 keys counted not trusted; the bearer-before-assignment ordering leak found *because the test asserted the token was gone rather than that something had changed*; the free-text/structured split for the 32-character rule stated as a known limit with a test pinning it. Nothing in it overclaims.
9. **The test suite does not touch live data.** I md5-summed all ten live databases before and after `cargo test --workspace --all-targets` — byte-identical. `--all-targets` does build `examples/migrate_check`, but as a libtest harness (`0 passed`), so its `main()` never runs. Recorded so the next auditor need not re-derive it.
10. **"185 tests" is exact.** `fd46044` claims 185; I measured 185 passed, 0 failed, 0 `FAILED` lines. `clippy` exits 0 with no errors, `fmt --check` is clean. Every commit's per-step test count is monotone and lands on the final figure.
11. **`P1-02` / `P1-16`'s self-retraction.** Every element of the "undo does ship" correction — module declaration, five registered commands, UI call sites, auto-snapshot before every agent write — checks out at the cited lines, and zero `checkpoints.db` references remain anywhere in four crates plus `src/`.

---

## Appendix · audit log, in investigation order

*(Written incrementally during the audit; the sections above were composed from it at the end.)*



### P1-01 — graph modules declared, 27 structs, 0 `Tool` impls

**Claim (MDP §7 P1-A):** `pub mod graph_prompts;` and `pub mod graph_tools;` declared and compiling; **27** structs; **0 of the 27** implement `Tool`; nothing registers them.

Measured:

- `autodev-core/src/agent/builtin_tools/mod.rs:13-14`
  ```
  13:pub mod graph_prompts;
  14:pub mod graph_tools;
  ```
- Struct count — `grep -c "^pub struct \|^struct " autodev-core/src/agent/builtin_tools/graph_tools.rs` → `27`; same on `graph_prompts.rs` → `0`. **27 confirmed.**
- `impl Tool for` blocks: `grep -n "impl .*Tool" graph_tools.rs graph_prompts.rs` returns **27 inherent `impl XTool {` blocks and zero `impl Tool for`**. Verbatim first line: `autodev-core/src/agent/builtin_tools/graph_tools.rs:18:impl GetMinimalContextTool {` — inherent, not a trait impl.
- Registration: `grep -n "register" autodev-core/src/agent/builtin_tools/mod.rs | grep -i graph` → **exit 1, no output**. There are 76 `registry.register(` calls in that file; none names a graph tool.
- External references: `grep -rn "graph_tools::\|graph_prompts::" --include=*.rs autodev-core src-tauri src-cli src-shared` → **exit 1, no output**.
- LOC arithmetic checks out: pre-P1 `git show 6c67ebb~1:src-tauri/src/agent/builtin_tools/graph_tools.rs | wc -l` = **840**, `graph_prompts.rs` = **78** → 918. Today 899 + 78 = 977 (file grew during the E0609 fix).

**Verdict: CONFIRMED (both halves).** The declaration is real, and the honesty about non-wiring is accurate — I attacked it and could not find a registration.

### P1-02 / P1-16 — `db/checkpoints.rs` deleted; one checkpoint owner

**Claim:** task reversed — file deleted, not declared. Undo *does* ship: `pub mod checkpoint;` in `git/mod.rs`, five commands in `lib.rs`, `src/stores/gitStore.ts` calls `list_checkpoints`, auto-snapshot before every agent write. P1-16 collapses to a one-way confirmation: zero references to `checkpoints.db`, `git/checkpoint.rs` sole owner.

Measured:

- `ls autodev-core/src/db/checkpoints.rs src-tauri/src/db/checkpoints.rs` → both **No such file or directory**. `git show 6c67ebb --stat` shows `src-tauri/src/db/checkpoints.rs | 280 ---------` (deleted).
- `autodev-core/src/git/mod.rs:21:pub mod checkpoint;` — present.
- Five commands, `src-tauri/src/lib.rs:386-390`: `create_checkpoint, restore_checkpoint, list_checkpoints, delete_checkpoint, compare_checkpoints` — **five, confirmed**, all inside the `tauri::generate_handler!` list.
- `src/stores/gitStore.ts:238` `invoke<CheckpointInfo[]>('list_checkpoints', {`; `:347` `restore_checkpoint`; `:352` `delete_checkpoint`. UI wiring confirmed.
- Auto-snapshot: `autodev-core/src/agent/builtin_tools/edit_file.rs:177` and `.../write_file.rs:124` both call `crate::git::checkpoint::auto_checkpoint_before_write(`.
- `grep -rn "checkpoints.db" --include=*.rs --include=*.ts --include=*.tsx autodev-core src-tauri src-cli src-shared src` → **exit 1, no output**. Zero references confirmed.

**Verdict: CONFIRMED** for both P1-02 and P1-16. The self-retraction is accurate in every particular I could check.

### P1-03 — six E0609s fixed, `cargo check --workspace` exits 0

- `cargo check --workspace --all-targets --message-format short` → **`Finished dev profile ... [exited with code 0]`**. Warnings only (11 in `autodev`, 7 in `airsctl`, 37 in `autodev-core` lib test) — no errors.
- `git show 6c67ebb` message records "five E0609 … one E0609" = six; the MDP text says "Six `E0609`s … at `:296-300` and one later". The commit and the plan agree.

**Verdict: CONFIRMED.**

### P1-04 — reachability gate passes; `KNOWN_UNREACHABLE` empty; widened to four crates

- `src-tauri/src/reachability_gate.rs:57` — `const KNOWN_UNREACHABLE: &[(&str, &str)] = &[];` **empty, verbatim.**
- `crate_roots()` at `src-tauri/src/reachability_gate.rs:133-141` returns four crates with per-crate floors: `autodev-core` 150, `src-tauri` 50, `src-cli` 8, `src-shared` 5; plus `assert!(walked > 250, …)` at the end of the test.
- Live file counts: `autodev-core` 189, `src-tauri` 84, `src-cli` 14, `src-shared` 5, total **292**.
- `cargo test -p autodev --lib reachability` → `test result: ok. 2 passed; 0 failed`.

⚠ Two observations, not refutations:
1. `src-shared` sits at **exactly** its floor (5 files, floor 5). One more file moved out of it and the gate fails — which is the intended behaviour, but it is at zero margin today.
2. The gate is a **name-set**, not a reachability walk — `declared_module_names` unions every `mod x;` in the crate regardless of whether the declaring file is itself reachable. The file documents this limitation in its own doc comment ("A name-set rather than a true graph walk: it cannot prove a declared module is itself reachable"). Honest, but it means a green gate does not prove reachability, only declaredness.

I did **not** re-run the falsification (planting an orphan) — that would modify the codebase, which my mandate forbids. The falsification claim is therefore taken on the commit message, unverified by me.

**Verdict: CONFIRMED** (gate green, list empty, four crates walked) with the falsification half **UNVERIFIED by me**.

### P1-05 — 182 files moved; zero Tauri edges in core

**Claim:** create the crate, move 182 files; `cargo tree` reports **0** Tauri/GTK/webkit edges for `autodev-core`, **28** for the desktop crate; gates `TG-P1-01/02/03`.

Measured (counted, not read):

```
$ git show eab345c --name-status -M | grep -E "autodev-core/src" | wc -l
182
$ git show eab345c --name-status -M | grep -E "autodev-core/src" | awk '{print substr($1,1,1)}' | sort | uniq -c
      5 A
    177 R
```
The 5 `A` files under `autodev-core/src` are `db/app_settings.rs`, `errors.rs`, `events.rs`, `lib.rs`, `recipes/orchestrator.rs`; 177 are renames. **182 confirmed exactly.**

```
$ cargo tree -p autodev-core | grep -icE "tauri|gtk|webkit"
0
$ cargo tree -p autodev      | grep -icE "tauri|gtk|webkit"
28
$ cargo tree -p airsctl      | grep -icE "tauri|gtk|webkit"
0
```
**0 / 28 confirmed to the digit.** (Note: with `-e normal` the desktop count is 18; the 28 figure includes build/dev edges, i.e. the default `cargo tree`. The claim does not qualify which, and the default invocation reproduces it.)

Gate existence:
- `TG-P1-01` — `src-tauri/src/ci_gate.rs:437 fn tg_p1_01_autodev_core_does_not_depend_on_tauri()` — reads `autodev-core/Cargo.toml` **as text**.
- `TG-P1-02` — `src-tauri/src/ci_gate.rs:488 fn tg_p1_02_no_core_source_file_references_tauri_in_code()` — walks core's `.rs`, asserts `files.len() > 150`.
- `TG-P1-03` — **not a Rust test.** It exists only as a CI step: `.github/workflows/ci.yml:81` `# TG-P1-03 - the crate boundary, checked transitively.` and the step `Crate boundary - autodev-core must not depend on Tauri`, which runs `cargo tree -p "$crate" | awk 'tolower($0) ~ /tauri|gtk|webkit|glib/'` for `autodev-core` and `airsctl`, plus a self-falsification against `autodev`. Legitimate, but it means `cargo test --workspace` alone does **not** run TG-P1-03 — only CI does.

**Verdict: CONFIRMED.**

### P1-06 — `OnceLock<AppHandle>` replaced with an injected `EventSink`

**Claim:** "the live figures were **19 `emit_X(&app, …)` / `emit_X_global(…)` pairs** collapsed to one function each, rewritten across **50 call sites in 21 files**, plus 30 handle-passing argument lines removed."

Measured:

```
$ git show eab345c~1:src-tauri/src/events.rs | grep -c "^pub fn emit_"          # 34
$ git show eab345c~1:src-tauri/src/events.rs | grep -c "^pub fn emit_.*_global" # 15
$ git show eab345c~1:src-tauri/src/events.rs | grep "^pub fn emit_" | grep -c "app" # 19
$ grep -c "^pub fn emit_" autodev-core/src/events.rs                            # 26
```

Names, pre-P1 (34) vs post (26). The 34 resolve to **25 unique base names**. Base names carrying **both** a plain and a `_global` form — i.e. actual *pairs* — are exactly **nine**:
`budget_exceeded`, `plan_changed`, `qc_decision`, `subagent_event`, `task_changed`, `worker_log_line`, `worker_message`, `worker_request`, `worker_status`.
Six were `_global`-only (`orphans_released`, `phase_complete`, `sidecar_crash`, `task_unblocked`, `workspace_mismatch`, `workspace_strays`) and ten were plain-only (`agent_done`, `agent_error`, `cost_update`, `permission_request`, `thinking`, `token`, `tool_end`, `tool_start`, `worker_tick_end`, `worker_tick_start`). 9·2 + 6 + 10 = 34 ✓.

⚠ **The word "pairs" does not survive the count.** 19 is the number of **`&AppHandle`-taking** emit functions, not the number of pairs; there were **9** pairs. And "collapsed to one function each" would leave 19 functions, whereas 25 base names + the new `emit_serialized` helper = the 26 that exist. The correct statement is: *34 emit functions (19 app-taking + 15 `_global`) collapsed to 25, of which 9 were true duplicate pairs.* → **Finding W-01, MINOR.**

The substantive half **holds**:
- `autodev-core/src/events.rs:32 pub trait EventSink: Send + Sync`, `:45 pub struct NullEventSink;`, `:58 pub fn install_sink(sink: Arc<dyn EventSink>) -> bool`.
- Desktop: `src-tauri/src/events.rs:21 impl EventSink for TauriEventSink`, `:36 install_sink(Arc::new(TauriEventSink { app }))`; `src-tauri/src/main.rs:206` checks the return and `tracing::error!`s on double-installation — **reports rather than swallows, confirmed**.
- CLI: `src-cli/src/main.rs:130 autodev_core::events::install_sink(std::sync::Arc::new(autodev_core::events::NullEventSink));` — explicit, confirmed.
- Four unit tests, `autodev-core/src/events.rs:527/533/541/552`: `a_null_sink_discards_without_panicking`, `an_uninstalled_sink_still_returns_something_usable`, `a_recorder_sees_name_and_payload`, `a_payload_that_cannot_serialise_is_dropped_not_propagated`. **4 confirmed.**
- No `OnceLock<AppHandle>` remains: the only hit is a *comment* at `autodev-core/src/events.rs:6`.

**Verdict: OVERSTATED** (mechanism confirmed; the headline count is misdescribed).

### P1-07 — both binaries link `autodev-core`

- `src-cli/tests/core_runtime.rs` exists (4,639 bytes) and contains three tests: `the_agent_runtime_constructs_in_a_process_with_no_ui`, `the_cli_can_build_the_same_builtin_tool_set_the_desktop_app_runs` (asserts `read_file`, `write_file`, `bash` are present, not merely non-empty), `emitting_without_a_host_sink_is_a_no_op_not_a_panic`.
- `cargo tree -p airsctl | grep -icE "tauri|gtk|webkit"` → **0**.

The ⚠ half — "Linking is not adoption" — is honest and was itself closed by P1-09 (below).

**Verdict: CONFIRMED.**

### P1-08 — INV-1 restated (DOC-GATE)

- `docs/restructurization/vision/00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md:120` carries the restated INV-1 verbatim: "**No agent work happens in a foreign runtime or language** (`G-02`). One agent runtime, written in Rust, linked by every binary that runs agent work… *Restated in P1 (`P1-08`)…*" — with the P1 evidence **and** the unsatisfied half inline: "Not yet satisfied: `main.rs` still spawns `memory-mcp-server` and `core-orchestrator` (P8)."
- `:1151` also carries the `G-02` restatement.
- I verified the unsatisfied half is honest — the desktop still spawns the two Python sidecars.

**Verdict: CONFIRMED.** (The MDP cites the file as `§Part 1`; the text lives in the invariants table. Locatable, not misfiled.)

### P1-09 — `src-shared` mirrors deleted; airsctl adopts core

**Structural half — CONFIRMED:**
- `find src-shared/src -name "*.rs"` → exactly five files: `config.rs`, `db/analytics.rs`, `db/mod.rs`, `errors.rs`, `lib.rs`. `tasks.rs`, `workers.rs`, `plans.rs` are gone.
- `src-cli/src/commands/task.rs:3 use autodev_core::db::tasks::{CreateTaskInput, Task, TasksDb};`, `worker.rs:3 use autodev_core::db::workers::{Worker, WorkersDb, DEFAULT_MODEL};`, `plan.rs:3 use autodev_core::db::plans::{…}`.
- `src-shared/src/db/analytics.rs`: `grep -cE "INSERT|UPDATE|DELETE|CREATE"` → **0**. "Holds zero write statements" — **confirmed**.

**⚠ Count half — NOT REPRODUCIBLE.** The claim (MDP and commit `eb9be2b`) is "**22 / 19 / 23** write statements between them". Against `eb9be2b~1`:

| file | write-SQL stmts¹ | all SQL keywords² | rusqlite calls³ | fns | LOC | **claimed** |
|---|---|---|---|---|---|---|
| `tasks.rs`   | 10 | 21 | 16 | 19 | 328 | 22 |
| `workers.rs` |  7 | 17 | 14 | 10 | 179 | 19 |
| `plans.rs`   |  8 | 18 | 16 | 12 | 208 | 23 |

¹ `grep -oiE "INSERT INTO|UPDATE [a-z_]+ SET|DELETE FROM|CREATE TABLE|CREATE INDEX|ALTER TABLE"` ² `grep -oiE "\b(SELECT|INSERT|UPDATE|DELETE|CREATE|ALTER)\b"` ³ `.execute( .execute_batch( .prepare( .query_row( .query_map(`

Hand-enumerating `workers.rs` (179 lines) gives **13 SQL statements total** — 1 `CREATE TABLE`, 2 `CREATE INDEX`, 1 `INSERT`, 6 `SELECT`, 3 `UPDATE` — of which **7 are writes**. There is no reading on which that file contains 19 write statements. I also walked the file's whole history (`git log --follow`): its maximum write-statement count at any revision is **7**. → **Finding W-02, MINOR.**

**Live-outage half — CONFIRMED by reading the deleted source:** `git show eb9be2b~1:src-shared/src/db/workers.rs:45-61` is a `CREATE TABLE IF NOT EXISTS workers (…)` declaring `last_seen`/`notes`, and `:85` selects `…,last_seen,…` — against a table `src-tauri` had already created without them. `IF NOT EXISTS` makes the create a no-op, so the select could only ever fail. The mechanism is real.

**Live-DB half — PARTIALLY VERIFIABLE.** `~/autodev-backups/p1-09-preflight/` **exists** on disk (dir mtime 13:28, matching commit `eb9be2b` at 13:30). The "7 rows preserved / worker list exits 0 / heartbeat round-trip / unknown slug exits 1" assertions are transient command results I cannot re-derive without running `airsctl` against the live DB, which my mandate forbids. **Not refuted; not independently re-proven.**

**Verdict: OVERSTATED** (structure and mechanism confirmed; the headline statement count is not reproducible).

### P1-10 — every database has a numbered ladder

**Ladders found (`grep -rn "run_ladder(" --include=*.rs`, production call sites only):**

| db | site |
|---|---|
| `analytics.db` | `autodev-core/src/analytics/db.rs:140` |
| `annotations.db` | `autodev-core/src/db/annotations.rs:104` |
| `config.db` | `autodev-core/src/db/config_db.rs:95` |
| `plans.db` | `autodev-core/src/db/plans.rs:305` |
| `runs.db` | `autodev-core/src/db/runs.rs:113` |
| `tasks.db` | `autodev-core/src/db/tasks.rs:534` |
| `workers.db` | `autodev-core/src/db/workers.rs:161` |
| `memory.db` | `autodev-core/src/memory/db.rs:130` |
| `autodev.db` (session) | `autodev-core/src/session/storage.rs:131` |

Nine, plus the per-repo graph store on its own `run_migrations` ladder = **ten**. The live root confirms this exactly: `~/.autodev/` holds 9 `.db` files (`analytics, annotations, autodev, config, memory, plans, runs, tasks, workers`) and `~/.autodev/graphs/d0ef18f6ad8b.db` is the tenth. **P1-10's "7 of 7" is consistent** — at 13:52, `autodev.db` (joined at P1-14, 14:21) and `runs.db` (created at P1-18, 14:27) were not yet ladder members: 9 − 2 = 7. ✓

**`run_ladder` mechanics — CONFIRMED by reading `autodev-core/src/db/migrate.rs`:**
- Rung + stamp in one transaction: `:127` `BEGIN`, `:130-132` `execute_batch(m.sql).and_then(|_| execute_batch("PRAGMA user_version = {version}"))`, `:136` `COMMIT`, `:145` `ROLLBACK` on error and returns `Err`. **Confirmed.**
- Refuses a newer database: `:88-93` `if current > target { return Err(… "refusing to run against it.") }`. **Confirmed.**
- Rejects a non-consecutive ladder before touching anything: `validate_ladder` at `:216-238`, called on line `:80` — the **first** statement of `run_ladder`. **Confirmed.**
- `ensure_columns` at `:174` probes `sqlite_master` then `PRAGMA table_info` and **propagates** the `ALTER` error (`:203-205`) rather than discarding it. **Confirmed.**

**`~/autodev-backups/p1-10-preflight/` exists** (dir mtime 13:50, commit `8dd27f6` at 13:52). ✓

**Verdict: CONFIRMED.** (The self-reported `ensure_columns`-before-ladder defect and its `P1-13` fix are audited under P1-13 below.)

### P1-11 — graph v5 dead statements removed, v6 added

- `autodev-core/src/graph/migrations.rs:115 fn migration_v5` — the six `ADD COLUMN IF NOT EXISTS` statements are **gone**; what remains is two `CREATE INDEX IF NOT EXISTS` and the `PRAGMA user_version = 5` stamp, and the comment records why.
- The retraction is **correct**, verified against the schema, not the prose: `autodev-core/src/graph/schema.rs` `CREATE_NODES` declares `agent_author` (`:57`), `task_id` (`:58`), `worker_locked` (`:59`), `worktree` (`:60`); `CREATE_EDGES` declares `agent_author` (`:78`), `task_id` (`:79`). **All six were always in v1's schema.** The "columns were never lost" claim holds.
- `migration_v6` at `:167` calls `crate::db::migrate::ensure_columns` for both tables and propagates errors (`?`), then stamps v6 at `:219`.
- Four tests, `autodev-core/src/graph/migrations.rs`: `:239 a_fresh_graph_database_reaches_the_ladder_head_with_every_column`, `:261 a_v5_database_that_never_got_the_columns_is_repaired_by_v6`, `:300 a_v5_database_that_already_has_the_columns_is_advanced_untouched`, `:315 migrations_are_idempotent`. **4 confirmed**, and the two named in the claim are the two that matter.
- `~/autodev-backups/p1-11-preflight/` exists (mtime 13:53, commit `98b5fc1` at 13:58). ✓
- Live graph store present at `~/.autodev/graphs/d0ef18f6ad8b.db`.

**Verdict: CONFIRMED.**

⚠ **Carried forward to P1-19:** the graph ladder runs through its **own** `run_migrations`, not `migrate::run_ladder`, so it does **not** inherit `run_ladder`'s automatic destructive-migration backup. Non-issue for v6 (pure `ADD COLUMN`), a real gap for any future graph migration. See W-04.

### Workspace-wide test run (evidence base for the rest of this audit)

```
$ cargo test --workspace --all-targets
...
     Running unittests src\main.rs (airsctl)                 → 0 passed
     Running tests\core_runtime.rs                           → 3 passed
     Running unittests src\lib.rs (autodev_lib)              → 44 passed
     Running unittests src\main.rs (autodev)                 → 0 passed
     Running unittests src\lib.rs (autodev_core)             → 130 passed
     Running tests\concurrent_writers.rs                     → 3 passed
     Running tests\redaction_boundaries.rs                   → 4 passed
     Running tests\s1_04_secret_never_reaches_the_prompt.rs  → 1 passed
     Running unittests examples\migrate_check.rs             → 0 passed
     Running unittests src\lib.rs (autodev_shared)           → 0 passed
exit=0 · TOTAL passed=185 failed=0 · zero "FAILED" lines
```

**Live-data safety, measured not assumed.** I md5-summed all ten live databases (`~/.autodev/*.db`, `~/.autodev/graphs/*.db`) and `~/.airs/autodev.db` **before and after** the run. Every hash is byte-identical. Notably `--all-targets` *does* build and run `examples/migrate_check.rs`, but as a libtest harness (`0 passed`), so `main()` — which migrates the live root — never executes. **The suite is safe to run with live credentials/paths present.** Recording this because a future auditor will otherwise have to re-derive it.

### P1-12 — one tuned connection path

**"All 17 production connection sites in core" — CONFIRMED exactly.** `grep -rn "open_tuned(" autodev-core/src` returns 18 call sites; one (`autodev-core/src/db/mod.rs:88`) is inside that file's `#[cfg(test)] mod tests` (which begins at `:77`). 18 − 1 = **17**:

`analytics/db.rs:30` · `db/annotations.rs:64` · `db/config_db.rs:91` · `db/plans.rs:218` · `db/runs.rs:111` · `db/tasks.rs:333` · `db/workers.rs:155` · `git/checkpoint.rs:30` · `graph/store.rs:72,85,112,184,227,497` · `hooks/db.rs:19` · `memory/db.rs:32` · `session/storage.rs:38`

- `open_tuned` at `autodev-core/src/db/mod.rs:56-64` sets `busy_timeout` then `journal_mode=WAL`. Unit test `open_tuned_sets_both_pragmas_that_matter` at `:82` passes.
- Every remaining direct open in core is either `open_in_memory` or `open_with_flags(..., SQLITE_OPEN_READ_ONLY | …)` — I checked the flags on all five `open_with_flags` sites (`analytics/queries.rs:108`, `db/backup.rs:133`, `:162`, `:220`, `db/legacy_root.rs:80`, `:99`); **all are READ_ONLY**, so the exemption is sound today.
- `TG-P1-05` at `src-tauri/src/ci_gate.rs:700` walks `autodev-core/src` with a `files.len() > 150` floor, skips `db/mod.rs`, and scans only `text.split("#[cfg(test)]").next()`. Falsification test at `:742`.

⚠ **Two real limits on the gate, neither disclosed by the claim:**
1. **It walks `autodev-core` only.** `src-shared/src/db/analytics.rs:55` is a **production** connection to the live `~/.autodev/analytics.db` opened with a plain `Connection::open(&path)?` — no `busy_timeout`, no `journal_mode`. `TG1-06`'s gate text in `TESTING_GATES.md:128` reads "`busy_timeout` is set on **every** connection", which this contradicts. → **Finding W-03.**
2. The `#[cfg(test)]` split means everything after a file's **first** `#[cfg(test)]` is invisible to the gate. I checked all core files: the earliest first-occurrence is at 43% of the file (`db/legacy_root.rs:126`, `db/migrate.rs:238`), so no production code is currently hidden — but one `#[cfg(test)] fn helper` placed near the top of a file would silently blind the gate for the rest of it.

The `busy_timeout` correction (rusqlite defaults to 5000 ms, so this fixed nothing) is stated in `autodev-core/src/db/mod.rs:32` as a comment and is **honest**.

**Verdict: CONFIRMED for core, OVERSTATED as a workspace claim** (`TG1-06`'s "every connection").

### P1-13 — one timestamp helper; the orphan TTL can fire

- `autodev-core/src/db/timestamps.rs` exists; API is `parse_db_timestamp` (`:57`), `age_hours` (`:84`), `now_rfc3339` (`:93`).
- **Seven** production callers, matching "one broken + six that worked": `analytics/db.rs:193, :408, :448, :507, :747`, `routing/mod.rs:93`, `orphan/mod.rs:195`. Zero ad-hoc `parse_from_rfc3339` remains outside the helper (`autodev-core/src/db/timestamps.rs:64`) and test/comment lines.
- The measurement-not-reading claim is itself pinned by a test: `autodev-core/src/db/timestamps.rs:122 fn parse_from_rfc3339_alone_cannot_read_a_sqlite_column`, asserting `"2026-09-01 06:23:58"` is `is_err()` and `"…+00:00"` is `is_ok()`. **Good practice; confirmed.**
- The orphan fix at `autodev-core/src/orphan/mod.rs:195`, and its test at `:271` asserts the raw column really is unparseable before requiring the verdict. **Confirmed.**
- Baseline repair: `tasks.db`'s `LADDER` baseline now declares **32** columns (14 original + 18 later), and `EXPECTED_COLUMNS` (`autodev-core/src/db/tasks.rs:1775-1808`) has exactly **32** entries.

⚠ **"verified against the real file" does not describe the guard.** `a_fresh_database_has_every_column_an_existing_one_has` (`autodev-core/src/db/tasks.rs:1879`) compares a fresh DB's `PRAGMA table_info(tasks)` against the **hardcoded `EXPECTED_COLUMNS` constant**, not against `~/.autodev/tasks.db`. The constant may well have been transcribed from the live file once, but the *test* cannot detect live-schema drift. The MDP's wording ("verified against the real file") reads as an ongoing property; it is a one-time authoring act. → **Finding W-05, MINOR.**

⚠ **Misattached doc comment (real, trivial).** The paragraph explaining the fresh-database regression sits at `autodev-core/src/db/tasks.rs:1809-1818` — attached to `clearing_the_board_backs_up_first_and_the_backup_holds_the_tasks` (`:1821`), the *backup* test — and runs straight into the backup test's own sentence at `:1819`. The test it actually describes (`:1879`) carries no doc comment. → **Finding W-09, MINOR.**

**Verdict: CONFIRMED with one overstated evidence phrase.**

### P1-14 — one data root

**Confirmed:**
- `autodev-core/src/db/legacy_root.rs` exists (289 lines added in `a29c73b`). `legacy_dir()` at `:39-44` returns `PathBuf::from(home).join(".airs")` and **does not** `create_dir_all` — guarded by `legacy_dir_does_not_create_the_directory` at `:274`.
- `migrate_sqlite_once` at `:63` uses `VACUUM INTO` (`:86`) from a **READ_ONLY** source connection (`:78-81`), refuses to overwrite an existing destination (`:69`), and verifies `PRAGMA integrity_check` on the copy, deleting the copy and erroring if it is not `ok` (`:97-110`).
- Wired: `autodev-core/src/session/storage.rs:26 match crate::db::legacy_root::migrate_sqlite_once("autodev.db", &db_path)?`.
- The **actual path resolution** in every repointed site now uses `.autodev`: `src-shared/src/config.rs:13 let dir = PathBuf::from(home).join(".autodev");`, `autodev-core/src/plugin/token.rs:108 .join(".autodev").join("fleet_secret")`, `src-cli/src/commands/plugin.rs:644 .join(".autodev").join("plugins")`, `src-tauri/src/updater/persistence.rs:44 home.join(".autodev").join("fleet_secret")`.
- `~/.airs` still exists on disk holding `autodev.db` + a **0-byte** `-wal` — matching the stated "deliberately not deleted" and the "the 4.1 MB WAL no longer exists" note. Both **confirmed against the filesystem**.
- `~/autodev-backups/p1-14-preflight/` exists (mtime 14:15; commit at 14:21).

⚠ **"All nine `~/.airs` path references repointed" is true of code and false of documentation.** Seven surviving `~/.airs` strings describe behaviour the code no longer has:

| location | text | user-visible? |
|---|---|---|
| `src-cli/src/main.rs:22` | `/// Path to config file (default: ~/.airs/config.toml)` | **YES** — clap doc comment on `Cli::config`, printed by `airsctl --help` |
| `src-cli/src/commands/config.rs:8` | `/// Create default config at ~/.airs/config.toml` | **YES** — clap doc comment on `ConfigCmd::Init`, printed by `airsctl config --help` |
| `src-shared/src/config.rs:63` | `/// Load from \`~/.airs/config.toml\`…` | no |
| `src-shared/src/config.rs:74` | `/// Persist to \`~/.airs/config.toml\`.` | no |
| `src-shared/src/config.rs:82` | `/// Create a default config file at \`~/.airs/config.toml\`.` | no |
| `src-cli/src/commands/plugin.rs:540` | `// Read ~/.airs/plugins/ for installed plugins` | no |
| `autodev-core/src/plugin/token.rs:7` | `…stored in ~/.airs/fleet_secret.` | no |

The two clap ones ship to users: `airsctl --help` tells the user their config lives at `~/.airs/config.toml` while `config_path()` writes `~/.autodev/config.toml`. → **Finding W-06, MAJOR** (user-facing wrong documentation; the rest MINOR).

**Verdict: OVERSTATED.**

### Live-database state, read directly (evidence for P1-10/11/12/15/17/18 and S1-03)

I copied all ten live databases to a scratch directory and opened the **copies** read-only with Python's `sqlite3` (no write ever reached `~/.autodev/`; there were no `-wal`/`-shm` files, so the copies are faithful):

```
== analytics.db:   user_version=1 journal_mode=wal   tables(5): api_failures, daily_costs, system_events, token_usage, worker_stats
== annotations.db: user_version=1 journal_mode=wal   tables(1): annotations          triggers: update_annotations_timestamp
== autodev.db:     user_version=1 journal_mode=wal   tables(2): messages, sessions
== config.db:      user_version=1 journal_mode=wal   tables(6): app_settings, autodev_instructions, checkpoints, hook_logs, hooks, model_configs
                                                     triggers: autodev_instructions_updated_at, update_app_settings_ts, update_model_configs_ts
== graph_store.db: user_version=6 journal_mode=wal   tables(12): analysis_cache, communities, edges, flows, nodes, nodes_fts*, repos, snapshots
== memory.db:      user_version=1 journal_mode=wal   tables(4): episodic_memories, fact_history, procedural_memories, semantic_memories
== plans.db:       user_version=1 journal_mode=wal   tables(4): plan_executions, plan_item_dependencies, plan_items, plans
== runs.db:        user_version=1 journal_mode=wal   tables(1): runs
== tasks.db:       user_version=1 journal_mode=wal   tables(4): routing_history, tasks, worker_messages, worker_requests   triggers: tasks_updated_at
== workers.db:     user_version=1 journal_mode=wal   tables(1): workers                triggers: update_workers_timestamp

run_id columns found anywhere: NONE
```

This settles, **against state rather than code**:
- **P1-12 · WAL** — all ten are `wal`. ✓ (`busy_timeout` is per-connection and not persisted, so it cannot be read from a file; that half remains code-only.)
- **P1-10 / P1-11 / S1-03 · versions** — nine at `v1`, the graph store at **`v6`**. ✓
- **P1-17 · `hourly_costs`** — absent from `analytics.db`; its five tables are exactly `DIAGNOSTIC_TABLES` (`autodev-core/src/analytics/queries.rs:22-28`). ✓
- **P1-15 · the trigger** — `update_app_settings_ts` **is** present in the live `config.db`, so creating it in the owner matches every existing install. ✓
- **P1-18 · "nothing is stamped yet"** — **no table in any live database has a `run_id` column**, and `grep -rn "run_id" --include=*.rs` over the workspace finds the identifier only inside `db/runs.rs` (`parent_run_id`, self-referential), `src-cli/src/main.rs:162`, `src-tauri/src/main.rs:529` (local variables) and `subagent/manager.rs:104` (an unrelated local). **The honesty claim is accurate.** ✓

### P1-15 — one owner for `app_settings`

- Sole production creator: `autodev-core/src/db/config_db.rs:30 CREATE TABLE IF NOT EXISTS app_settings (`. The second hit at `:163` is inside that file's `#[cfg(test)]` block (which starts at `:99`).
- The three former creators now call the owner: `src-tauri/src/commands/github_mcp_commands.rs:36,70`, `src-tauri/src/commands/workspace_commands.rs:29,46`, `autodev-core/src/ide/workspace.rs:58,76` — all `crate::db::app_settings::open()`. Their now-dead `use crate::db::autodev_dir` / `use rusqlite::…Connection` imports show up as compiler warnings, which is independent corroboration that they no longer open the file themselves.
- `TG-P1-04` at `src-tauri/src/ci_gate.rs:628`, with a working falsification test at `:645`, and a detector that skips comments and `#[cfg(test)]`.

**Verdict: CONFIRMED for `app_settings`.**

⚠ **But the *file*-owner claim is false.** `autodev-core/src/db/config_db.rs:1` opens "`~/.autodev/config.db` — **the schema owner for the whole file**" and `:3` says "**Three** modules keep tables here". The live `config.db` holds **six** tables. The other three are created by two modules that open `config.db` **directly, outside the ladder**:

- `autodev-core/src/git/checkpoint.rs:30-46` — `crate::db::open_tuned(&autodev_dir()?.join("config.db"))` then `conn.execute_batch("CREATE TABLE IF NOT EXISTS checkpoints (…); CREATE INDEX …")`
- `autodev-core/src/hooks/db.rs:17-58` — same open, then `migrate()` running `CREATE TABLE IF NOT EXISTS hooks (…); CREATE TABLE IF NOT EXISTS hook_logs (…)`

Neither consults or advances `user_version`; both are exactly the "`CREATE TABLE IF NOT EXISTS` in an `open()` path is not a migration" pattern that `autodev-core/src/db/migrate.rs:5-6` names as the defect P1-10 exists to remove. And `TG-P1-04` (`src-tauri/src/ci_gate.rs:629`) iterates a **hardcoded three-table list** — `["app_settings", "autodev_instructions", "model_configs"]` — so `checkpoints`, `hooks` and `hook_logs` are outside the one-owner gate entirely. → **Finding W-07, MAJOR.**

### P1-17 — `hourly_costs`

- `rg hourly_costs` over the repo: **zero** code references. The only surviving mentions are prose in `autodev-core/src/analytics/queries.rs:17` and `:48` and in planning/audit documents.
- `DIAGNOSTIC_TABLES` at `autodev-core/src/analytics/queries.rs:22-28` lists exactly the five tables the live `analytics.db` has.
- Test `every_diagnostic_table_actually_exists` at `autodev-core/src/analytics/queries.rs:324` builds a fresh DB from the real migration and checks each name.
- `.unwrap_or(-1)` is still at `:51`, which the task text discloses and justifies.

**Verdict: CONFIRMED.**

### P1-18 — the run/audit ledger

**Confirmed against the live ledger** (`~/.autodev/runs.db`, read from a copy):

```
['id','kind','actor','started_at','ended_at','outcome','host','pid','parent_run_id','detail']
(…865, 'cli','airsctl', 07:25:49.467…, 07:25:49.472…, 'ok',     'LAPTOP-3K58S1BQ',  9564, None, None)
(…ff1, 'cli','airsctl', 07:25:49.514…, 07:25:49.528…, 'ok',     'LAPTOP-3K58S1BQ', 29512, None, None)
(…35e, 'cli','airsctl', 07:25:49.580…, 07:25:49.587…, 'failed', 'LAPTOP-3K58S1BQ', 33572, None, None)
count 3 · by outcome [('failed',1), ('ok',2)]
```
Three invocations, two `ok`, one `failed`, **all three closed, none left open** — exactly the claim, digit for digit.

- Schema at `autodev-core/src/db/runs.rs:85-99` includes `parent_run_id TEXT REFERENCES runs(id)`. Nesting proven by `runs_can_be_nested_so_a_turn_points_at_its_launch` (`:371`).
- `finish` at `:161-176` uses `WHERE id = ?3 AND outcome IS NULL` and **errors when `rows == 0`**. Confirmed, with test `closing_an_unknown_or_already_closed_run_is_an_error` (`:316`).
- Both binaries wired: `src-cli/src/main.rs:161-179`, `src-tauri/src/main.rs:529-557`.
- **"nothing is stamped yet"** — verified two ways (grep over all `.rs`, and `PRAGMA table_info` on every table of all ten live databases). **NONE.** The honesty is accurate.

⚠ **`reap_abandoned` cannot tell a dead process from a live one, and will corrupt a concurrent peer's run.** `autodev-core/src/db/runs.rs:233-251`:
```
"UPDATE runs SET ended_at = ?1, outcome = ?2 \
 WHERE outcome IS NULL AND NOT (pid = ?3 AND host = ?4)"
```
The only predicate is *"not this process"*. So: desktop app launches (opens run A, pid X, `outcome IS NULL`); the user then runs any `airsctl` command; `airsctl`'s startup `reap_abandoned` marks the app's **still-live** run A as `abandoned`; when the app exits, `finish` hits `rows == 0` and returns the error "no open run … to close". The ledger then shows a normal session as `abandoned`, and the app logs a spurious warning. That two-binaries-at-once case is not hypothetical — `TESTING_GATES.md:128` (`TG1-06`) makes it *the requirement*.

The correct primitive already exists **in the same crate and is used elsewhere**: `autodev-core/src/orphan/pid_checker.rs:7 pub fn is_pid_alive(pid: u32) -> bool` (used at `autodev-core/src/orphan/mod.rs:165`). `reap_abandoned` does not call it. The test `a_run_left_open_by_a_dead_process_is_reaped` (`:339`) inserts `host='other-host', pid=999999` — a row that is unreachable-by-construction, so it passes identically whether or not liveness is checked. → **Finding W-08, MAJOR.**

⚠ Minor: the `LIVE-DB-PROVEN` evidence covers the **CLI** half only — the live ledger contains no `kind='app'` row. The desktop wiring is `CODE-PRESENT`.

**Verdict: CONFIRMED as claimed, with a MAJOR latent defect in new P1 code that the claim does not mention.**

### P1-19 — the backup helper and its wiring  ← **the most serious finding in this audit**

**The two claimed wirings are real:**
- `autodev-core/src/db/migrate.rs:115 match crate::db::backup::backup_before(&path, &reason)`, inside `if is_destructive(m.sql)`, and a failed backup **returns `Err`** (`:117-121`) so the migration stops.
- `autodev-core/src/db/tasks.rs:733 let backup = crate::db::backup::backup_before(&path, "clear-board").map_err(…)?;` — before the `DELETE FROM worker_requests; DELETE FROM tasks; DELETE FROM sqlite_sequence …` batch at `:742-747`. Reached from the UI via `src-tauri/src/commands/task_commands.rs:151 pub fn clear_all_tasks()`, registered at `src-tauri/src/lib.rs:244`.
- `grep -rn "backup_before\|db::backup::"` across all four crates returns **exactly those two production callers** (plus a doc comment). So "the helper had zero callers, now it has two" is accurate.

**⚠ There is a third wholesale-destruction path, and it is NOT wired.**

```
src-tauri/src/commands/graph_commands.rs:811
    #[tauri::command]
    pub async fn delete_graph(repo_path: Option<String>) -> Result<()> {
        …
        let db_path = repo_db_path(&path)?;
        if db_path.exists() {
            std::fs::remove_file(&db_path)?;                       // :824
            tracing::info!("[GraphDelete] Deleted graph DB: {:?}", db_path);
        }
```
This **deletes an entire database file** — the per-repo code graph (`~/.autodev/graphs/d0ef18f6ad8b.db`, 126 KB, 12 tables, `user_version = 6`). It is more wholesale than `clear_all`, which only empties two tables. It is registered (`src-tauri/src/lib.rs:85`, `:430`) and **reachable from the UI in one click**: `src/tabs/Graph.tsx:691 await invoke('delete_graph', { repoPath: workspaceDir })`. No `backup_before`, no confirmation in the Rust layer, and `fs::remove_file` is unrecoverable.

The MDP's phrasing — "Wired to the **two** genuinely wholesale operations, deliberately not to row-level deletes" — presents a two-item enumeration as exhaustive. It is not. → **Finding W-13, BLOCKER for the task as written.**

**⚠ Two further gaps in `is_destructive`'s coverage** (`autodev-core/src/db/migrate.rs:56-70`):

```rust
["drop table","drop index","drop view","drop trigger",
 "delete from","rename to","rename column","truncate"]
```
1. **`ALTER TABLE … DROP COLUMN` is not in the list.** SQLite has supported it since 3.35, and it destroys a column's data outright. `TESTING_GATES.md:120` (`TG1-04`) states the gate covers "A migration that **drops**, truncates, **rewrites or type-changes a column**" — a bare `UPDATE … SET` rewrite is likewise undetected. The implementation is narrower than the gate it is cited for.
2. **The graph ladder never reaches `is_destructive` at all.** `autodev-core/src/graph/migrations.rs:12 run_migrations` is a hand-rolled ladder, not `migrate::run_ladder`, so no graph migration gets the automatic backup. That is not theoretical: `autodev-core/src/graph/migrations.rs:105` (`migration_v4`) executes `DROP TABLE IF EXISTS nodes_fts`. On today's live store (already at v6) it will not re-run, but any database still at v3 would be dropped-and-rebuilt with no backup, and any future graph migration inherits the gap. → **Finding W-04, MAJOR.**

Also unwired, for completeness (lower severity, derived data): `autodev-core/src/graph/store.rs:751 tx.execute_batch("DROP TABLE IF EXISTS nodes_fts")` in `rebuild_fts_index` — the FTS index is regenerated from `nodes` in the same transaction, so nothing unique is lost.

**Verdict: OVERSTATED → REFUTED on the exhaustiveness claim.** The helper is correct and its two wirings are real and well-tested; the statement that these are *the* wholesale operations is false.

### P1-20 — both Python redactors ported

- `autodev-core/src/security/{mod.rs,redaction.rs}` exist. `SENSITIVE_KEYS` at `autodev-core/src/security/redaction.rs:65-88` has **exactly 22** entries — I counted them: `api_key, apikey, api-key, secret, secret_key, secretkey, password, pass, pwd, token, access_token, refresh_token, auth_token, authorization, auth, private_key, privatekey, client_secret, clientsecret, bearer, credential, credentials`. **22 confirmed**, and the file says so at `:63` ("The plan says 20; they were counted, not trusted").
- `PIIRedactor`'s rules are all present in `TEXT_PATTERNS` (`:108-155`): bearer, `sk-`/`pk-`, `gh[pousr]_`, `AKIA|ASIA`, the assignment rule, email, SSN, card, phone — with an explicit ordering comment at `:101-107` explaining the bearer-before-assignment leak.
- The deliberate deviation is real and asserted: `LONG_TOKEN` (`:161-162`, `\b[A-Za-z0-9_\-]{32,}\b`) is applied only to dictionary values, never in `redact_text`. Tests `free_text_does_not_destroy_long_identifiers` (`:311`) and `a_bare_token_in_prose_is_not_caught_by_free_text_redaction` (`:352`) pin both halves.
- 11 unit tests in the module, including `the_ported_key_list_has_the_count_the_source_has` (`:228`) and `a_bearer_token_in_a_header_line_does_not_survive` (`:300`).

**Verdict: CONFIRMED.** This task is well done and honestly documented.

### P1-21 — "Apply at every boundary"  ← **BLOCKER**

**The complete, measured inventory.** `grep -rn "security::redaction\|use crate::security" --include=*.rs` over `autodev-core/src src-tauri/src src-cli/src` returns **twelve** production lines, in **four** files:

| file | lines |
|---|---|
| `autodev-core/src/agent/tool_executor.rs` | `:117`, `:218`, `:219` — **all three inside `fn execute`** |
| `autodev-core/src/memory/episodic.rs` | `:32`, `:33`, `:77`, `:82`, `:84`, `:85`, `:127` |
| `autodev-core/src/memory/procedural.rs` | `:129` |
| `autodev-core/src/memory/semantic.rs` | `:41` |

Against the boundaries the plan names (`03-CONTEXT-AND-MEMORY.md:245` `G-REDACT`: "in the prompt, in the logs, and in the memory store"; `R3-gaps-and-policy.md:356`: "outbound provider payloads, memory writes through the gateway, and log/event emission"; and `ci_gate.rs:780`'s own list "prompts, memory writes, logs, analytics, egress"):

| boundary | wired? | evidence |
|---|---|---|
| memory writes | **YES** | the four paths above, behaviourally proven by `autodev-core/tests/redaction_boundaries.rs` (4 tests, all pass) |
| prompts / tool results | **NO — see below** | the only redaction is in a function with no production caller |
| logs / event emission | **NO** | `grep -rln redact autodev-core/src/observability/ src-tauri/src/commands/log_commands.rs` → **exit 1, no files** |
| analytics | **NO** | `grep -rln redact autodev-core/src/analytics/` → **exit 1, no files** |
| egress / outbound provider payload | **NO** | `grep -rln redact autodev-core/src/provider/` → **exit 1, no files** |

**#### The prompt boundary is redacted in dead code.**

`ToolExecutor` is constructed in exactly two places in the entire workspace:
```
autodev-core/src/agent/engine.rs:125:        let tool_executor = ToolExecutor::new(
autodev-core/tests/s1_04_secret_never_reaches_the_prompt.rs:43:    let mut executor = ToolExecutor::new(
```
The production one calls **only**:
```
autodev-core/src/agent/engine.rs:314:                tool_executor.execute_batch(tool_batch).await?
```
`ToolExecutor::execute` (`autodev-core/src/agent/tool_executor.rs:94`) — the sole redacting method — **has no production caller.** `execute_batch` (`:246`) redacts nothing. In its read-only concurrent branch:

```
autodev-core/src/agent/tool_executor.rs:311   emit_tool_start(ToolStartPayload { … input: input.clone() });   // raw
:317                                            let result = tool.execute(input.clone(), &ctx).await;
:322   emit_tool_end(ToolEndPayload { … output: result.output.clone(), … });                        // raw
:344-352 Message::new_tool(&self.session_id, &call_id, &tool_name, input, &result.output, …)
         session_mgr.append_message(&tool_msg)                                                       // raw → messages table
:356                                            results[idx] = (tool_use_id, result);                // raw → conversation
```
and the write branch does the same at `:469-484` (`self.save_tool_message(&call_id, tool.name(), input, &result.output, …)` then `emit_tool_end(… output: result.output.clone() …)`).

Compare `execute`, which does redact:
```
autodev-core/src/agent/tool_executor.rs:218:        result.output = crate::security::redaction::redact_text(&result.output);
autodev-core/src/agent/tool_executor.rs:219:        let redacted_input = crate::security::redaction::redact_args(&input);
```

`read_file` is read-only, so `read_file(".env")` takes the concurrent branch: the secret reaches the model's conversation, the `messages` table in `~/.autodev/autodev.db`, and the `tool_end` UI event, **verbatim**. → **Finding W-10, BLOCKER.**

**#### And both proofs are inert-but-green.**

- `S1-04` (`autodev-core/tests/s1_04_secret_never_reaches_the_prompt.rs:60-66`) calls `executor.execute(read_file, …)` — the dead path. Its own header asserts the premise that is false: *"because every tool result funnels through one place."* There are two entry points and the engine uses the other one.
- `TG-P1-06` (`src-tauri/src/ci_gate.rs:777-812`) checks only `production.contains("security::redaction")` **per file**. `agent/tool_executor.rs` contains that string (in `execute`), so the gate is green while the live path is unredacted. Its own doc comment concedes it "asserts the boundary file references `security::redaction`, which a caller could satisfy while discarding the result" and defers to behaviour — but no behavioural test covers the tool path as the engine calls it.
- `TG-P1-06`'s `BOUNDARIES` list is four entries — `tool_executor` plus the three memory files — while the comment immediately above it (`:779`) says "Each entry is a boundary named by `P1-21`: prompts, memory writes, **logs, analytics, egress**". Three of the five named boundaries have no entry at all, so the gate cannot fail for them. → **Finding W-11, MAJOR.**

**Verdict: REFUTED.** "Applied at every boundary" is false for four of the five named boundaries, and the one non-memory boundary that is wired is wired to a code path nothing executes.

### P1-22 — the P8 gate exists (DOC-GATE)

- `docs/restructurization/development/TESTING_GATES.md:513` — `- [ ] **TG8-02** \`[governance]\` — … The deletion phase **cannot complete while any capability grounding marked \`PORT\` remains unported.** The gate reads the grounding reports' PORT verdicts as its checklist and asserts a named, tested destination in the Rust tree for each — the ported-capability count is **ten, not nine**, and the tenth is `M-40` redaction (`TG3-38`).`
- `:514` records the supply of the tenth entry by P1-20/P1-21.
- It sits in `§11` and is correctly left `- [ ]` (it is a P8 gate).

**Verdict: CONFIRMED as present.** ⚠ But `:514` now reads *"proven end to end by `S1-04`"*, which W-10 shows is not a proof of the live path — so this gate's own tenth-row evidence is compromised by the P1-21 defect.

### Smoke S1-01 … S1-04

**S1-01 — `cargo build` clean across all four crates.** Run individually:
```
autodev-core:    Finished `dev` profile … in 22.82s   exit=0
autodev:         Finished `dev` profile … in 56.89s   exit=0
airsctl:         Finished `dev` profile … in 36.75s   exit=0
autodev-shared:  Finished `dev` profile … in  2.33s   exit=0
```
**CONFIRMED, all four exit 0.**

**S1-02 — `airsctl --version` runs and links `autodev-core`.**
```
$ ./target/debug/airsctl.exe --version
airsctl 1.0.0
exit=0
$ cargo tree -p airsctl | grep autodev-core
3:├── autodev-core v1.0.0 (C:\AIRS Foundry\autodev\autodev\autodev-core)
$ cargo tree -p airsctl | grep -icE "tauri|gtk|webkit"
0
```
**CONFIRMED to the character** (`airsctl 1.0.0`, exit 0, core edge present, zero UI edges).

**S1-03 — every DB opens, reports its `user_version`, survives a restart.** I did **not** run `examples/migrate_check` (my mandate forbids it — it opens the live root). Verified two other ways:
- *Structurally:* `autodev-core/examples/migrate_check.rs` checks `workers, tasks, plans, runs, annotations, memory, config, autodev(sessions), analytics` by name (`:43-107`) plus every `*.db` discovered under `~/.autodev/graphs/` (`:110-140`), requiring graph stores to be `>= 6`. With one graph store present that is **10**, matching the claim. It exits non-zero on any unstamped or unopenable database (`:155`).
- *By state, independently:* reading `PRAGMA user_version` from copies of all ten live files gives 9 × `v1` + graph `v6`, with no `0/UNSTAMPED`. That is the same measurement the example prints, obtained without running it.

⚠ **Not verified:** "run twice, output byte-identical, exit 0 both times." That is a transient result I cannot reproduce under my constraints. **UNVERIFIABLE by me.**

**S1-04 — a secret read by a tool appears redacted in the prompt and in memory.** The test exists and passes (`1 passed`), and every assertion it makes is sound *for the function it calls*. But it calls `ToolExecutor::execute`, which **no production code calls**; the engine calls `execute_batch`, which does not redact. See **W-10**. The memory half is genuinely proven by `redaction_boundaries.rs` (4 tests). **REFUTED as a smoke of the shipped path.**

### Gate re-runs (independent)

```
$ cargo test --workspace --all-targets   → exit 0, 185 passed, 0 failed, 0 "FAILED" lines
$ cargo clippy --workspace --all-targets → exit 0, 0 errors
$ cargo fmt --all -- --check             → exit 0, no diff
```
Commit `fd46044` claims "185 tests" — **exact match**. The per-commit progression (106 → 131 → 135 → 138 → 150 → 160 → 164 → 185) is internally consistent with the final count.

⚠ The clippy gate is `cargo clippy --workspace --all-targets` with **no `-D warnings`** (`.github/workflows/ci.yml:74`), so it exits 0 while the workspace carries ~54 warnings. **Fifteen of them are demonstrably introduced by P1** and are dead code the phase left behind:
`src-tauri/src/commands/github_mcp_commands.rs:6,8` and `workspace_commands.rs:10,12` (unused `crate::db::autodev_dir`, `Connection` — left over when P1-15 routed them through `app_settings::open()`; `git show 14aa9c9~1` shows `autodev_dir` had 3 uses there before, now 1, the import) · `autodev-core/src/db/autodev_instructions.rs:6`, `model_configs.rs:5`, `ide/workspace.rs:14` (same cause) · `src-tauri/src/commands/plan_commands.rs:67,81,110,145,177` and `agent_commands.rs:100` (unused `app: AppHandle` / `app_clone`, left over from P1-06's EventSink; the file had 10 `app` references before P1, 5 now) · `autodev-core/src/orphan/mod.rs:225` · `autodev-core/src/session/storage.rs:351` (`home_dir` never used, from P1-14).
→ **Finding W-12, MINOR.**

### Two further document-level checks

**`TG1-01` is ticked while its own named minima are unmet.** The MDP (`MASTER_DEVELOPMENT_PLAN.md:534`) states: *"This is exactly why `TG1-01` is recorded as short of its registration clause **rather than ticked**."* But `TESTING_GATES.md:109` reads `- [x] **TG1-01**`. It **is** ticked, with the unmet half recorded only in the prose beneath it (`:112`: *"The registration half of this gate is still unmet and is not claimed"*). The two governing documents contradict each other about the checkbox state. → **Finding W-14, MAJOR.**

**The "280 files" figure is wrong, and was wrong when written.** `TESTING_GATES.md:110` ("walking **all four** crates (280 files)") and `src-tauri/src/reachability_gate.rs:126` ("protecting 84 files instead of 280"). `git log -S"280 files"` puts its introduction at **`fd46044`**, and `git ls-tree -r --name-only fd46044 | grep -E "^(autodev-core|src-tauri|src-cli|src-shared)/src/.*\.rs$" | wc -l` = **292**. The `84` for `src-tauri` is right; the workspace total is 12 short. → **Finding W-15, MINOR.**

**Two log/error strings carry a run of literal spaces mid-sentence** — a formatter artifact, but they ship in user-visible output:
- `src-tauri/src/main.rs:211` — `"[events] a sink was already installed before setup() —                          events may be delivered to a stale window"`
- `autodev-core/src/db/migrate.rs:119` — `"…refusing to apply destructive migration v{} ({})                              because no backup could be taken: {e}"`
(also `autodev-core/src/db/tasks.rs:1899` and `src-tauri/src/ci_gate.rs:734`; two more at `autodev-core/src/agent/builtin_tools/acknowledge_worker_message.rs:58` and `send_worker_message.rs:144` predate P1). → **Finding W-16, MINOR.**

