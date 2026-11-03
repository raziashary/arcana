# G3d — Bug Audit: graph / commands / db / plan

Read-only audit. Status: COMPLETE.

Scope: `src-tauri/src/graph/**`, `src-tauri/src/commands/**`, `src-tauri/src/db/**`,
`src-tauri/src/plan/**`, `lib.rs`, `main.rs`, `events.rs`, `errors.rs`, `logging.rs`,
`runtime_config.rs`.

---

## 1. Findings table

| ID | file:line | sev | class | what the code does | why it is wrong | concrete failure scenario | status |
|----|-----------|-----|-------|--------------------|-----------------|---------------------------|--------|
| F-02 | `agent/builtin_tools/bash.rs:404-437` | CRIT | cross-platform | rewrites `\| head`/`\| grep` into PowerShell `Select-Object`/`Select-String` | the executor two functions up (`:89-97`) is `cmd /C`, which has neither | any piped build command fails with "is not recognized" | CONFIRMED |
| F-03/F-05 | `bash.rs:114` + `update_task_status_tool.rs:133-162` | CRIT | logic | records the shell exit code as build evidence, then hard-rejects `qc_review` on a non-zero code | on cmd.exe the pipeline's exit code is `tee`'s failure, not the build's | every task is permanently blocked before qc_review — **BUG-COMPILE** | CONFIRMED |
| F-06 | `build_evidence.rs:63-66,125-136` | HIGH | logic | evidence keyed on `ctx.session_id`, in-process only | a subagent's build is invisible to the parent's status update; restart wipes it | gate rejects on a correctly-built task | CONFIRMED |
| F-08 | `validate.rs:269,753` | HIGH | cross-platform | `Command::new("tsc")` / `("pyright")` | Rust resolves bare names to `.exe`; npm/pip ship `.cmd` shims | every .ts/.tsx written unvalidated on Windows; result downgraded to a warning | CONFIRMED |
| F-10 | `commands/graph_commands.rs:74-81` | HIGH | path | `sha256(repo_path)[..12]` over the raw string | no canonicalise/case-fold/separator normalisation | `C:\p` and `C:/p` produce two different graph DBs — "my graph went empty" | CONFIRMED |
| F-11 | `commands/graph_commands.rs:404` | HIGH | path | `output_path.unwrap_or_else(\|\| format!("graph.{}", format))` | bare relative filename | export lands in the process CWD (`src-tauri/` in dev) | CONFIRMED |
| F-21 | `graph/store.rs:176-181` | HIGH | SQL | `INSERT OR REPLACE INTO nodes` omitting `community`, `community_name`, `worker_locked`, `worktree` | REPLACE deletes + reinserts, so those columns reset | every incremental update strips community assignments | CONFIRMED |
| F-22 | `graph/schema.rs:82-83` + `store.rs:176` | HIGH | SQL | FK `ON DELETE CASCADE` + `foreign_keys=ON` + `INSERT OR REPLACE` | SQLite's REPLACE fires the cascade | re-upserting a node deletes every edge in the graph that references it | CONFIRMED |
| F-23 | `graph/build.rs:51-57,134-146` | HIGH | logic | `reconcile_edges` keeps only edges whose endpoints are in *this batch* | an incremental batch is one file | one-file update permanently deletes that file's cross-file edges | CONFIRMED |
| F-20 | `graph/incremental.rs:100-177` | HIGH | logic | no `run_clustering` call | nodes touched incrementally keep `community = NULL` | graph view degrades to unclustered after any incremental update | CONFIRMED |
| F-26 | `graph/changes.rs:87` | HIGH | logic | requires node fully **contained** in the diff hunk, with `--unified=0` | should test overlap | `detect_changes` returns zero changed nodes for essentially every real edit, reports risk 0.0 | CONFIRMED |
| F-31 | `graph/migrations.rs:115-134` | HIGH | SQL | `ALTER TABLE … ADD COLUMN IF NOT EXISTS`, each wrapped in `let _ =` | SQLite has no such clause — every statement is a syntax error, discarded | v5 is inert; `user_version` still bumps to 5, so it never retries | CONFIRMED |
| F-42 | `graph/detect.rs:54-55` | HIGH | error handling | `for entry in fs::read_dir(dir)? { let entry = entry?; … }` | one bad entry aborts the whole walk | a single ACL-protected or >MAX_PATH dir kills `build_graph` | CONFIRMED |
| F-44 | `db/checkpoints.rs` (whole file) | HIGH | dead code | 280-line `CheckpointsDb` with no `mod` declaration in `db/mod.rs:10-15` | not compiled, not referenced | the advertised file rollback/undo/audit feature does not exist | CONFIRMED |
| F-45 | `commands/task_commands.rs:133-137` | HIGH | logic | parses `updated_at` with `DateTime<Utc>::from_str` | SQLite wrote `datetime('now')` — no `T`, no offset | every qc_review task reports age 0 s; stalled-review alert never fires | CONFIRMED |
| F-45b | `orphan/mod.rs:183` | HIGH | logic | `parse_from_rfc3339(&task.updated_at).ok()?` | same mismatch | the 4-hour TTL orphan fallback is unreachable | CONFIRMED |
| F-55 | `db/tasks.rs:895-903` | HIGH | logic | claim `UPDATE … WHERE id=? AND status='assigned'` | never checks `assigned_worker` | any worker can steal a task assigned to another and overwrite the owner — **BUG-ASSIGN** | CONFIRMED |
| F-DEL | `recipes/runner.rs:175-190` + `commands/workspace_commands.rs:85-98` | HIGH | path | with no configured workspace, workers run in `~/.autodev/agents/<slug>/` | that is not the user's project and nothing hands the result back | agent output stranded in the home config dir — **BUG-DELIVER** (files observed on disk) | CONFIRMED |
| F-EV1 | `commands/plan_commands.rs:67,94,117,152,174` vs `src/stores/planStore.ts:74-90` | HIGH | contract | Rust emits `plan:changed`; the UI listens for 5 different names | total name mismatch | the plan UI never live-updates | CONFIRMED |
| F-13 | `commands/graph_commands.rs:650,663,737` | MED | API | `NODE_RENDER_LIMIT = 2000` with no override parameter | caller cannot request detail | any repo over 2000 nodes only ever renders community super-nodes | CONFIRMED |
| F-18/F-19 | `graph/incremental.rs:31-41`, `graph/changes.rs:148-161` | MED | error handling | blocking `git diff`, no timeout, non-zero exit → `warn!` + empty result | failure is indistinguishable from "no changes" | `update_graph` reports success and does nothing; hung git parks a tokio thread | CONFIRMED |
| F-14 | `commands/graph_commands.rs:278` | MED | logic | diff base hardcoded to `HEAD~1` | fails on 1-commit repos, shallow clones, unborn HEAD | silent no-op update | CONFIRMED |
| F-29/F-47 | `graph/store.rs:98-102`, `db/tasks.rs:286-292` | MED | SQL | new `Connection` per call, WAL on, **no `busy_timeout`** | concurrent writers get `SQLITE_BUSY` immediately | fleet tick storms produce sporadic write failures | CONFIRMED |
| F-30 | `graph/store.rs:643-669` | MED | SQL | raw user string into `nodes_fts MATCH ?` | FTS5 has its own grammar; LIKE fallback only covers a missing table | searching `foo(`, `a-b`, `::` returns a hard error | CONFIRMED |
| F-27 | `graph/changes.rs:79,108,184,186` | MED | perf | full-table loads inside per-file loops | O(files × nodes) + O(nodes × edges) | change analysis on a large graph takes minutes | CONFIRMED |
| F-24 | `graph/incremental.rs:151-156` | MED | dead code | `find_dependents` result assigned to `all_files_to_extract`, never used | documented one-hop dependency re-extraction does not happen | stale dependents keep old edges; full edge scan wasted every update | CONFIRMED |
| F-33/F-43 | `graph/cache.rs`, `graph/watcher.rs` | MED | dead code | 264 lines total, never constructed | `QueryCache` and `GraphWatcher` are unreachable | the graph never auto-updates on file change | CONFIRMED |
| F-34 | `graph/build.rs:21-24`, `graph/registry.rs:202-205` | MED | stub | `// TODO … Ok(())` | success-shaped no-ops, publicly re-exported | a caller believes the pipeline ran | CONFIRMED |
| F-REG | `graph/registry.rs:62,123` vs `graph_commands.rs:74-81` | MED | path | two DB-path schemes | the registry path is created but never populated | `merge_all_repos_cmd` writes `{"nodes":[],"edges":[]}` and reports success | CONFIRMED |
| F-56 | `agent/builtin_tools/assign_task.rs:96-99` | MED | logic | `assigned` is a dispatchable state | silent re-route while a claim may be in flight | worker #1 keeps working on a task now owned by worker #2 | CONFIRMED |
| F-57 | `db/tasks.rs:1199-1206` + `assign_task.rs` | MED | logic | epics inserted `status='ready'`; no `task_type` filter on assign | an epic is dispatchable like a task | a worker is handed "the epic" as a unit of work | CONFIRMED |
| F-50 | `db/tasks.rs:1530-1531,1352-1355` | MED | silent failure | malformed `blocked_by` JSON → `None` → `continue` | the task is never unblocked and nothing is logged | permanent deadlock on one corrupt row | CONFIRMED |
| F-52 | `db/tasks.rs:163-172` | MED | logic | `_ => Self::Ready` in `from_str` | any unknown status becomes a dispatchable ready task | a bad write re-queues finished work | CONFIRMED |
| F-54 | `commands/task_commands.rs:27-43` | MED | contract | UI command performs none of the tool's validation | two divergent implementations of one transition | UI can push a task to qc_review with no build; no hooks fire | CONFIRMED |
| F-17 | `commands/graph_commands.rs:620` | MED | security | `merge_all_repos_cmd(output_path: String)` | no workspace jail on a frontend-supplied write path | arbitrary-path write from the renderer | CONFIRMED |
| F-UNW | `commands/agent_commands.rs:34,58,103,159,214,231`, `settings_commands.rs:91-129`, `analytics_commands.rs:38`, `diff_commands.rs:19`, `recipe_commands.rs:193` | MED | panic | `state.lock().unwrap()` | one panic poisons the mutex forever | every later call on that state panics — the app half-dies | CONFIRMED |
| F-40 | `graph/detect.rs:68-73` | LOW | cross-platform | case-sensitive extension match | `.TS`/`.PY` invisible | files silently missing from the graph | CONFIRMED |
| F-41 | `graph/detect.rs:40-66` | LOW | cross-platform | unbounded recursion following symlinks/junctions | no depth cap, no visited set | stack overflow aborts the process | CONFIRMED |
| F-09 | `agent/builtin_tools/e2e_probe.rs:35,60-78,105-107` | LOW | cross-platform | fixed 8 s boot wait; kills the `cmd` wrapper only | grandchild `node`/`vite` survives | leaked dev server holds the port; false FAILs | CONFIRMED |
| F-36 | `graph/registry.rs:128-132` | LOW | docstring/body | comment says LRU; code evicts `keys().next()` | arbitrary hash-order eviction | cache thrash | CONFIRMED |
| F-37 | `graph/registry.rs:42,64,82,88,94` | LOW | concurrency | unlocked read-modify-write of `registry.json`; `parent().unwrap()` | lost updates | concurrent register/unregister silently drops entries | CONFIRMED |
| F-15 | `commands/graph_commands.rs:126,204,329,338,345,373,404,502,…` | LOW | panic | `.expect("db_path always has parent")` in a command | panic at the IPC boundary | (unreachable in practice) | CONFIRMED |
| F-28 | `graph/changes.rs:64` | LOW | dead code | `.max(0)` on a `u32` | no-op | none | CONFIRMED |
| F-35 | `graph/build.rs:152` | LOW | dead code | `get_stats()?` assigned and unused | 4 wasted aggregate scans per prune | slow prune | CONFIRMED |
| F-38 | `graph/registry.rs:195` | LOW | stub | `cross_repo_edges: 0, // TODO` | always zero | merge stats lie | CONFIRMED |
| F-51 | `db/tasks.rs:1356` | — | **REFUTED** | `blockers.contains(&completed_str)` | it is `Vec<String>::contains`, not `str::contains` | no substring false positive | REFUTED |

Sections 2–9 follow the raw findings log below (see **MAIN REPORT**).

---

## APPENDIX A — running findings log (raw, appended as read)

### A.1 — build-evidence / bash layer (Priority 1)

- **F-01 CONFIRMED** `src-tauri/src/agent/builtin_tools/bash.rs:89-97` — the shell is
  `cmd /C` on Windows, `sh -c` elsewhere.
- **F-02 CONFIRMED** `src-tauri/src/agent/builtin_tools/bash.rs:404-437`
  (`rewrite_for_windows`) rewrites Unix pipe stages into **PowerShell** cmdlets
  (`| head -n N` → `| Select-Object -First N`, `| grep X` → `| Select-String X`)
  and then hands the result to **cmd.exe**, which has no such commands. The
  rewriter produces a command that is guaranteed to fail in the shell that runs
  it. Docstring says "Unix → PowerShell command pre-processor for Windows"; the
  executor two functions up is `cmd /C`. Severity: HIGH.
- **F-03 CONFIRMED** `bash.rs:114` records evidence with the *rewritten* command and
  the shell's exit code. `tee`, `timeout`, `;`, `$?` are all absent from cmd.exe,
  so the recipe-mandated form
  `npm run build 2>&1 | tee build.log; echo "EXIT:$?"` exits non-zero
  (`'tee' is not recognized` → errorlevel 9009 / 1) even when `npm run build`
  itself succeeded — cmd pipelines return the **last** stage's status.
- **F-04 CONFIRMED** `agent/builtin_tools/build_evidence.rs:74-120` `classify()`
  matches on substring, so the failing pipeline above still classifies as
  `Node` build evidence, recorded with the failing exit code.
- **F-05 CONFIRMED** `agent/builtin_tools/update_task_status_tool.rs:133-162` —
  the gate. `status == "qc_review"` + `ctx.agent_role.is_some()` + role not
  starting with `qc-`/`subagent-qc-dispatcher` ⇒ requires
  `most_recent_build(session_id)`; `None` ⇒ hard error, `exit_code != 0` ⇒ hard
  error. Combined with F-02/F-03 this makes `qc_review` unreachable on Windows.
- **F-06 CONFIRMED** `build_evidence.rs:63-66,125-136` — the evidence ring is a
  process-global `HashMap<session_id, Vec<..>>` keyed on `ctx.session_id`. If
  the bash call and the status update run under different session ids
  (subagent vs parent), the gate sees `None` and rejects. No persistence: an
  app restart between build and status update loses all evidence.
- **F-07 CONFIRMED** `build_evidence.rs:26` TTL is 15 min of **`Instant`**
  (monotonic). Fine, but combined with F-06 it means evidence is process-local
  only — nothing is written to any DB or table. There is **no `build_evidence`
  schema anywhere**; `grep` across `src-tauri/src` finds no table, no column,
  no migration.
- **F-08 CONFIRMED** `agent/builtin_tools/validate.rs:269` runs
  `Command::new("tsc")`, and `validate.rs:753` runs `Command::new("pyright")`.
  On Windows these are npm/pip `.cmd` shims; Rust's `Command` resolves a
  bare program name against `PATHEXT`-less `.exe` search, so spawn fails with
  `NotFound` and the code takes the `Err(_)` arm, which downgrades the result
  to a **`warning`** ("TypeScript not installed") and lets the write succeed.
  Every .ts/.tsx file is therefore written unvalidated on Windows while the
  agent is told validation "was not run". Severity: HIGH (Windows).
- **F-09 CONFIRMED** `agent/builtin_tools/e2e_probe.rs:60-78` — the probe also
  shells `cmd /C` on Windows, and `PROBE_DEFAULT_BOOT_WAIT_S = 8`
  (`e2e_probe.rs:35`) is a fixed sleep with no readiness poll; a slow Vite/
  cargo boot yields a false FAIL. `e2e_probe.rs:106` `let _ = c.kill().await;`
  swallows teardown errors, and on Windows killing the `cmd` wrapper does NOT
  kill the grandchild `node`/`vite` process — the dev server leaks and holds
  the port for the next probe.

### A.2 — graph_commands.rs (Priority 2)

- **F-10 CONFIRMED** `commands/graph_commands.rs:74-81` `repo_db_path()` is the
  ONLY DB-path function in the command layer:
  `~/.autodev/graphs/<first-12-hex-of-sha256(repo_path)>.db`. The repo path is
  hashed **verbatim** — no canonicalisation, no case folding, no separator
  normalisation. On Windows `C:\proj`, `C:/proj`, `c:\proj`, and `C:\proj\`
  all hash differently ⇒ **four different graph databases for one repo**.
  Severity: HIGH (Windows). This is the likely mechanism behind "the graph is
  empty after I rebuilt it".
- **F-11 CONFIRMED** `commands/graph_commands.rs:404`
  `let out_path = output_path.unwrap_or_else(|| format!("graph.{}", format));`
  — a bare relative filename. `export_graph` with no `output_path` writes
  `graph.json` / `graph.html` / `graph.svg` into the **process CWD** (the
  Tauri binary's launch directory, i.e. `src-tauri/` in dev). CWD-relative
  write #1.
- **F-12 CONFIRMED** `commands/graph_commands.rs:663-672`
  `get_graph_visualization_data` opens the store **without** the
  `create_dir_all` that every sibling command performs
  (`:329, :338, :345, :373, :404, :502…`), so it is the one command that
  errors instead of silently creating an empty DB.
- **F-13 CONFIRMED** `commands/graph_commands.rs:650` `NODE_RENDER_LIMIT = 2000`
  and `:663` `get_graph_visualization_data(repo_path)` takes **no** override
  parameter. Above 2000 project nodes the command unconditionally returns
  community super-nodes. `is_aggregated`/`total_node_count` are reported, but
  there is no way to ask for detail. (Raw detail is reachable only via the
  separate `get_graph_nodes_for_viz(repo_path, max_nodes)` +
  `get_graph_edges_for_viz(repo_path)` pair, which return a *different*,
  degree-less JSON shape and no communities.)
- **F-14 CONFIRMED** `commands/graph_commands.rs:278` `update_graph` hardcodes
  the diff base `"HEAD~1"`. On a repo with a single commit, a shallow clone,
  or a detached/unborn HEAD, `git diff HEAD~1` fails; per F-2x below that
  failure is converted into an empty file list, and `update_graph` reports
  `files_changed: 0` and emits `graph:updated` — a **successful-looking
  no-op**.
- **F-15 CONFIRMED** `commands/graph_commands.rs:126,204,329,338,345,373,404,
  502,…` `db_path.parent().expect("db_path always has parent")` — `.expect()`
  inside `#[tauri::command]`. Unreachable in practice (the path is always
  built by `repo_db_path`), but it is a panic in a command boundary.
- **F-16 CONFIRMED** every `app.emit(...)` in this file is `let _ = app.emit(...)`
  (`:119, :140, :148, :153, :183, :191, :200, :208, :216, :224, :225, :245,
  :253, :282, :287, :299, :306, :311, :312, :862`). A frontend that never
  receives `graph:build_progress` is indistinguishable from one that does.
- **F-17 CONFIRMED** `commands/graph_commands.rs:621` `merge_all_repos_cmd`
  takes `output_path: String` straight from the frontend with no workspace
  jail — an arbitrary-path write reachable from any renderer code.

### A.3 — graph module internals (Priority 2)

- **F-18 CONFIRMED** `graph/incremental.rs:31-41` — `std::process::Command::new("git").arg("diff").arg("--name-only").arg(base_ref).output()` — **blocking** std Command (not tokio), **no timeout**, and `if !output.status.success()` -> `tracing::warn!` + `Ok(Vec::new())`. A failed git invocation is indistinguishable from "nothing changed". Called from the `async` `update_graph` command (`commands/graph_commands.rs:278`), so it also blocks a tokio worker thread on a hung `git`.
- **F-19 CONFIRMED** `graph/changes.rs:148-161` — identical pattern for `git diff --unified=0 <base>`: blocking, no timeout, non-zero exit -> `warn!` + an empty `ChangeAnalysis` with `total_risk_score: 0.0`. A caller cannot tell "no risk" from "git isn't installed / bad ref".
- **F-20 CONFIRMED** `graph/incremental.rs:100-177` — `incremental_update` **never calls `run_clustering`**. Steps are: delete file nodes/edges -> extract -> find_dependents -> `build::build` -> `rebuild_fts_index`. Confirmed: nodes added or re-upserted by an incremental update carry `community = NULL` until a full `build_graph` re-runs clustering (`commands/graph_commands.rs:214`).
- **F-21 CONFIRMED — worse than F-20** `graph/store.rs:176-181` — the batch node upsert is `INSERT OR REPLACE INTO nodes (...)` and the column list **omits `community`, `community_name`, `worker_locked`, `worktree`**. `INSERT OR REPLACE` deletes the conflicting row and inserts a fresh one, so every re-upsert **resets those columns to NULL/default**. An incremental update therefore doesn't merely fail to cluster new nodes — it strips the existing community assignment off every node in every changed file.
- **F-22 CONFIRMED (SQLite semantics)** `graph/schema.rs:82-83` — `edges` has `FOREIGN KEY (source_qualified) REFERENCES nodes(qualified_name) ON DELETE CASCADE` (and the same for target), and every connection sets `PRAGMA foreign_keys=ON` (`store.rs:85,100,172,215,259`). Under SQLite, `INSERT OR REPLACE` performs a **DELETE then INSERT**, and with foreign keys enabled that DELETE fires `ON DELETE CASCADE`. So re-upserting node X (F-21) cascade-deletes **every edge in the whole graph that references X**, including edges owned by files that were not part of this update and which are therefore never rebuilt. Mechanism for progressive edge loss across incremental updates. The code path is unambiguous; quantifying it needs a runtime check.
- **F-23 CONFIRMED** `graph/build.rs:51-57` + `:134-146` — `reconcile_edges` drops any edge whose source **or target** is not in `node_ids`, and `node_ids` is built only from **the nodes in this batch** (`build.rs:51-54`). In an incremental update the batch is just the changed files, so every cross-file edge is discarded before insert. Combined with `delete_file_edges` at `incremental.rs:129`, a one-file incremental update permanently deletes that file's outbound cross-file edges.
- **F-24 CONFIRMED dead code** `graph/incremental.rs:151-156` — `dependent_files` is computed via `find_dependents` (a full edge-table scan) and merged into `all_files_to_extract`, which is then **never used**. The documented "STEP 3: Find dependent files (one hop)" behaviour does not happen; the expensive scan runs anyway.
- **F-25 CONFIRMED silent failure** `graph/incremental.rs:140-147` — `for (_, result) in extractions { if let Ok(extraction) = result { ... } }`. Extraction errors are dropped with no counter and no log. The full-build path at `commands/graph_commands.rs:174-177` at least warns and counts.
- **F-26 CONFIRMED logic bug** `graph/changes.rs:87` — `if node_start >= *start && node_end <= *end` requires the graph node to be **fully contained inside the diff hunk**. The diff is taken with `--unified=0` (`changes.rs:149`), so hunks are only the changed lines. A one-line edit inside a 40-line function produces hunk `(120,120)` and the function node `(100,140)` never matches. `detect_changes` therefore returns an empty `changed_nodes` for essentially every real edit — and then reports `total_risk_score: 0.0`. The test should be range *overlap*, not containment.
- **F-27 CONFIRMED perf/N+1** `graph/changes.rs:79` — `find_changed_nodes` calls `store.get_all_nodes_for_graph(i64::MAX, 0)` (whole node table into memory) and is invoked **twice per changed file** in the loop at `changes.rs:184,186`. `score_change_risk` (`:108`) loads the whole edge table again, and `get_impact_radius` is called once per changed node (`:198`). O(files x nodes) + O(nodes x edges) on every change analysis.
- **F-28 CONFIRMED** `graph/changes.rs:64` `let end = start + count.saturating_sub(1).max(0);` — `.max(0)` on a `u32` is a no-op; dead expression.
- **F-29 CONFIRMED** `graph/store.rs` sets **no `busy_timeout`** on any connection (`store.rs:75-86` open, `:98-102` get_conn, `:171,214,258` batch paths). Every operation opens a **brand-new `Connection`**, so the file watcher, the agent tools and the UI can collide; with WAL a writer conflict returns `SQLITE_BUSY` immediately rather than waiting. `PRAGMA journal_mode=WAL` is set only in `open()` (persistent in the file, so that part is fine).
- **F-30 CONFIRMED** `graph/store.rs:643-669` — `search_nodes` passes the raw user string straight into `nodes_fts MATCH ?`. FTS5 has its own query grammar: `foo(`, `a-b`, `::`, `->`, an unbalanced `"` all raise `fts5: syntax error`, which propagates out of `search_graph_nodes` (`commands/graph_commands.rs:562`) as a failed command. The LIKE fallback (`store.rs:687`) triggers only when the FTS *table* is missing, never on a MATCH parse error. Searching for a Rust path or a call expression breaks graph search.
- **F-31 CONFIRMED** `graph/migrations.rs:115-134` — migration v5 uses `ALTER TABLE nodes ADD COLUMN **IF NOT EXISTS** ...`. **SQLite has no `IF NOT EXISTS` clause on `ADD COLUMN`** — every one of these six statements is a syntax error, and every one is wrapped in `let _ = ...` so the error is discarded. `tx.execute("PRAGMA user_version = 5")` at `:140` then commits, so the migration can never be retried. Fresh databases survive only because `CREATE_NODES`/`CREATE_EDGES` (`schema.rs:39-85`) already declare those columns; any database created before that happened is permanently missing them and every `SELECT ... agent_author ...` against it fails. Severity: HIGH latent.
- **F-32 CONFIRMED** `graph/migrations.rs:12-45` — no "version is newer than I understand" guard and no down-migrations; an older binary opening a newer DB proceeds silently.
- **F-33 CONFIRMED dead code** `graph/cache.rs` (115 lines) — `QueryCache` is never constructed anywhere in the crate (`grep -rn QueryCache` finds only its own definition). The `analysis_cache` SQL table (`schema.rs:121-128`) is likewise never read or written.
- **F-34 CONFIRMED stubs returning Ok** `graph/build.rs:21-24` `GraphBuilder::build_graph` -> `// TODO: Implement full graph build pipeline; Ok(())`, re-exported at `graph/mod.rs:27`. `graph/registry.rs:202-205` `GraphRegistry::manage_repos` -> `// TODO; Ok(())`, re-exported at `graph/mod.rs:31`.
- **F-35 CONFIRMED** `graph/build.rs:152` — `prune_deleted_files` calls `store.get_stats()?` (four full aggregate scans) and assigns the result to `stats`, which is never used.
- **F-36 CONFIRMED** `graph/registry.rs:128-132` — the comment says "Limit cache to 10 entries (LRU)" but the code evicts `cache.keys().next()`, an arbitrary `HashMap` bucket-order key. Not LRU.
- **F-37 CONFIRMED** `graph/registry.rs:64,82` and `:88,94` — `register` and `unregister` do an unlocked read-modify-write of `~/.autodev/registry.json`; concurrent calls silently lose entries. `registry.rs:42` `registry_path.parent().unwrap()`.
- **F-38 CONFIRMED** `graph/registry.rs:195` `cross_repo_edges: 0, // TODO` — `merge_all_repos` always reports zero cross-repo edges regardless of input.
- **F-39 CONFIRMED** `agent/builtin_tools/graph_tools.rs:176` `Path::new(".autodev/GRAPH_REPORT.md")` — CWD-relative read; and `graph_tools.rs:524` `detect_changes(store, ".", ...)` — the repo root passed to `git diff` is the literal `"."`, i.e. the **process CWD**, not the agent's workspace. CWD-relative reads/execs #2 and #3.

### A.4 — THE GRAPH DB LOCATION, RESOLVED

Two independent, non-communicating path schemes exist in the same crate:

1. **`~/.autodev/graphs/<sha256(repo_path)[..12]>.db`** — `commands/graph_commands.rs:74-81` (`repo_db_path`). Used by **every one of the 13 graph `#[tauri::command]`s** that touches a store: `:131, :275, :331, :368, :382, :406, :434, :460, :489, :516, :540, :576, :670`. This is where the real graph lives.
2. **`<repo_path>/.autodev/graph.db`** — `graph/registry.rs:62` (`RepoRegistry::register`) and `graph/registry.rs:123` (`RepoRegistry::get_store`). Reached from the commands `register_repo` (`graph_commands.rs:602`) and `merge_all_repos_cmd` (`graph_commands.rs:621` -> `merge_all_repos` -> `get_store`).

`GraphStore::open` creates the parent dir and the file (`store.rs:70-75`), so `get_store` **creates an empty DB** at location 2. Nothing ever writes nodes there — no build path uses the registry. Consequences, both CONFIRMED:

- The observed `src-tauri/.autodev/graph.db` (196 KB, dated Apr 28) is location 2 for `repo_path = <...>/src-tauri`. On disk today `~/.autodev/graphs/` holds exactly one DB (`d0ef18f6ad8b.db`, 127 KB, May 30) and `~/.autodev/registry.json` does not exist at all — so `RepoRegistry::load()` would recreate an empty registry and `list_repos` would return `[]`.
- `merge_all_repos_cmd` merges location-2 stores, which the builder never populates, so it writes `{"nodes":[],"edges":[]}` and reports `nodes_merged: 0` while returning `Ok`. A stub-shaped success.

### A.5 — remaining graph files, detect/watcher

- **F-40 CONFIRMED** `graph/detect.rs:68-73` — `is_code_file` builds `format!(".{}", ext)` and looks it up in `CODE_EXTENSIONS` (`detect.rs:7-12`), which are all lowercase. The comparison is **case-sensitive**, so `Foo.TS`, `MAIN.PY`, `Bar.RS` are not detected as code. Windows filesystems are case-insensitive and mixed-case extensions do occur.
- **F-41 CONFIRMED** `graph/detect.rs:40-66` — `walk_dir` recurses on `path.is_dir()`, which **follows symlinks / Windows junctions**, with no depth cap and no visited-set. A directory symlink loop (or a junction like `Documents and Settings`) recurses until the stack overflows, which aborts the process — a panic a `#[tauri::command]` cannot catch.
- **F-42 CONFIRMED** `graph/detect.rs:54-55` — `for entry in fs::read_dir(dir)? { let entry = entry?; ... }`. A **single** unreadable subdirectory (permissions, a path over MAX_PATH, a locked file) makes `detect_files` return `Err`, which aborts the entire `build_graph` command at `commands/graph_commands.rs:138`. There is no per-entry skip. On Windows this is the single most likely reason "the graph build just fails".
- **F-43 CONFIRMED dead code** `graph/watcher.rs` (149 lines) — `GraphWatcher` is referenced only by its own `pub use` at `graph/mod.rs:32`. It is never constructed or started; the debounce thread, the `notify` subscription and the `stop()`/`Drop` handling are all unreachable. The graph therefore never auto-updates on file change; every refresh is a manual `build_graph`/`update_graph` command.

### A.6 — db/ layer

- **F-44 CONFIRMED orphaned module** `src-tauri/src/db/checkpoints.rs` (280 lines,
  `CheckpointsDb`, `file_checkpoints` table, 14 public methods) is **not declared in
  `db/mod.rs`** (`db/mod.rs:10-15` declares only `tasks, autodev_instructions,
  model_configs, plans, workers, annotations`) and `CheckpointsDb` is referenced
  nowhere in the crate. The file does not compile into the binary. Its header
  claims "Used by: AgentEngine to record file states after write/edit
  operations … Enable rollback, undo, and audit trail" — that feature does not
  exist in the shipped build. (`commands/git_commands.rs:186 list_checkpoints`
  is a *different*, git-based mechanism.)
- **F-45 CONFIRMED timestamp-format mismatch #2** `commands/task_commands.rs:133-137` —
  `get_qc_queue` computes review age with
  `t.updated_at.parse::<chrono::DateTime<chrono::Utc>>()`, but `updated_at` is
  written by SQLite as `datetime('now')` (`db/tasks.rs:313,324,556,562,899,945,…`),
  which yields `"2026-08-31 12:34:56"` — no `T`, no offset. `DateTime<Utc>`'s
  `FromStr` requires an offset, so the parse always fails and `.unwrap_or(0)`
  makes **every qc_review task report an age of 0 seconds**. The "QC backlog
  age / stalled review" alert can never fire. Exactly the same class as the
  orphan-reconciler bug at `orphan/mod.rs:183`
  (`chrono::DateTime::parse_from_rfc3339(&task.updated_at).ok()?` → always
  `None` → the 4-hour `STALE_TTL_HOURS` fallback at `orphan/mod.rs:13,183-190`
  is unreachable).
- **F-46 CONFIRMED mixed formats in one table** `db/tasks.rs:987` writes
  `routing_history.outcome_at` with SQLite `datetime('now')`, while
  `routing/mod.rs:59` writes `routing_history.assigned_at` with
  `chrono::Utc::now().to_rfc3339()`. Two different formats in adjacent columns
  of the same table; `get_routing_history` (`db/tasks.rs:996`) orders by
  `assigned_at` as a *string*.
- **F-47 CONFIRMED** `db/tasks.rs:286-292` `TasksDb::open()` opens a fresh
  `Connection` per call with `PRAGMA journal_mode=WAL` but **no
  `busy_timeout`**. Every command and every agent tool calls `TasksDb::open()`
  independently (`commands/task_commands.rs:13,20,28,70,112,128,148,166,177,188,196,217`
  alone), and the recipe scheduler writes concurrently. Under contention SQLite
  returns `SQLITE_BUSY` immediately instead of waiting. Same for
  `db/workers.rs`, `db/plans.rs`, `db/annotations.rs`.
- **F-48 CONFIRMED** `db/tasks.rs:367-412` — the entire migration strategy is
  `let _ = self.conn.execute_batch("ALTER TABLE tasks ADD COLUMN …");` repeated
  ~20 times with the error discarded. There is no version counter. This is
  benign for "column already exists" but **indistinguishable from a real
  failure** (disk full, locked DB, typo in DDL): the app then runs against a
  table missing the column and every positional `SELECT` in the file breaks.
- **F-49 CONFIRMED** `db/tasks.rs:1497-1537` `row_to_task` is a 32-column
  **positional** mapper, and 12 of the fields use `row.get(N).unwrap_or(None)` /
  `.unwrap_or(0)`. Any change in a SELECT's column order silently yields
  `None`/`0` rather than an error — including for `blocked_by`,
  `budget_exceeded`, `under_review_by`. Eight separate SELECT statements must
  keep that order by hand.
- **F-50 CONFIRMED** `db/tasks.rs:1530-1531` — `blocked_by` is parsed with
  `serde_json::from_str::<Vec<String>>(&s).ok()`; a malformed blob becomes
  `None`. `unblock_ready_tasks` (`db/tasks.rs:1352-1355`) then hits
  `None => continue`, so the task is **never unblocked** — a permanent
  deadlock with no log line.
- **F-51 REFUTED (checked, no bug)** `db/tasks.rs:1356` `blockers.contains(&completed_str)`
  is `Vec<String>::contains`, not `str::contains`, because `Task.blocked_by` is
  `Option<Vec<String>>` (`db/tasks.rs:210`). No substring false-positive.
- **F-52 CONFIRMED** `db/tasks.rs:163-172` `TaskStatus::from_str` maps **any**
  unrecognised string to `Ready` (`_ => Self::Ready`). A row whose status was
  written by a path that isn't in the enum (or corrupted) silently re-enters
  the dispatch queue as a fresh ready task. Same shape at `:105-107` (priority
  → P2), `:129-135` (type → Feature), `:32-40` (request status → Pending).
- **F-53 CONFIRMED contract drift** `db/tasks.rs:140-151` defines **10**
  statuses including `needs_info`, but the agent tool's JSON enum
  (`agent/builtin_tools/update_task_status_tool.rs:37`) lists only **9** —
  `needs_info` is missing, so no agent can move a task into or out of it by
  name. `needs_info` is also overloaded: it is both "blocked by a dependency"
  (`db/tasks.rs:1219`) and "budget exceeded" (`db/tasks.rs:1107`).
- **F-54 CONFIRMED** `commands/task_commands.rs:27-43` — the `update_task_status`
  **Tauri command** performs *none* of the validation the identically-named
  agent tool performs: no `ready`+worker guard, no FLEET_WORKERS check, no
  `rejection_reason` requirement, and **no build-evidence gate**. It also fires
  no lifecycle hooks and emits no `qc:decision`. Two divergent implementations
  of the same transition.

### A.7 — BUG-ASSIGN mechanics

- **F-55 CONFIRMED — the misroute** `db/tasks.rs:895-903` — `claim_task_with_pid`
  is `UPDATE tasks SET status='in_progress', assigned_worker = ?1, … WHERE id = ?4 AND status = 'assigned'`.
  The WHERE clause guards on **status only** — it never checks that
  `assigned_worker` already equals the claiming worker. Any fleet worker that
  sees an `assigned` task can claim it and **overwrite `assigned_worker` with
  its own slug**. Supervisor assigns #12 to `worker-backend`; `worker-frontend`
  ticks first, claims #12, and the board now shows #12 owned by
  `worker-frontend` with no record that it was ever routed elsewhere. The
  routing_history row still says `worker-backend`, so affinity learning is
  poisoned too.
- **F-56 CONFIRMED** `agent/builtin_tools/assign_task.rs:96-99` — `assigned` is
  in the dispatchable set, so `assign_task` silently re-routes a task that is
  already assigned (and already `kick`ed) to a different worker, with no
  notification to the first worker and no guard against a claim in flight.
- **F-57 CONFIRMED** `agent/builtin_tools/assign_task.rs` never checks
  `task_type`. `db/tasks.rs:1199-1206` `create_epic` inserts the epic itself
  with `task_type='epic'` and **`status='ready'`**, so epics sit in the READY
  column and can be dispatched to a worker as if they were units of work.
- **F-58 CONFIRMED** `db/tasks.rs:873-892` — the anti-double-claim pre-flight is
  a `SELECT COUNT(*)` followed by a separate `UPDATE`, with `.unwrap_or(0)` on
  the count (`:877-882`). It is **not atomic** (no transaction), and a failing
  count is treated as "zero active tasks", i.e. the guard fails open.
- **F-59 CONFIRMED** `db/tasks.rs:1386-1394` — epic auto-completion does
  `epic_id.parse::<i64>()` inside `if let Ok(...)`; a non-numeric epic id (a
  slug like `EPIC-AUTH`) silently skips the completion with no log. The
  `epic_id` column is `TEXT` and nothing enforces that it is a task id.
- **F-60 CONFIRMED** `commands/task_commands.rs:234` — `create_phased_task`
  emits a `task:changed` payload whose `status` is *inferred*
  (`if task.epic_id.is_some() { "needs_info" } else { "ready" }`) rather than
  read from `task.status`. `db/tasks.rs:1219` actually chooses the status from
  `blocked_by.is_empty()`, not from `epic_id`. An epic subtask with no blockers
  is created `ready` but announced to the UI as `needs_info`.

---

# MAIN REPORT

## 2. BUG-COMPILE — the build-evidence mechanism

### 2.1 What it is, where it lives, what reads it

There is **no build-evidence schema**. Nothing is persisted. The mechanism is a
single process-global in-memory map:

| Piece | Location |
|---|---|
| Storage | `src-tauri/src/agent/builtin_tools/build_evidence.rs:63-66` — `static RING: OnceLock<Mutex<HashMap<String /*session_id*/, Vec<BuildEvidence>>>>` |
| Record shape | `build_evidence.rs:28-35` — `{ session_id, command (first 200 chars), exit_code: i32, recorded_at: Instant, kind }` |
| Kinds | `build_evidence.rs:37-49` — `Node \| Cargo \| Python \| Go \| Smoke` |
| TTL | `build_evidence.rs:26` — `EVIDENCE_TTL = 15 min`, swept lazily on read (`:145`, `:157`) |
| The only writer | `agent/builtin_tools/bash.rs:114` — `build_evidence::record(&ctx.session_id, &command, exit_code)` after every bash call |
| Classifier | `build_evidence.rs:74-120` — case-insensitive **substring** match against 14 Node forms, 4 Cargo, 7 Python, 3 Go, plus a `timeout …` smoke form |
| The only readers | `update_task_status_tool.rs:141` (`most_recent_build`); `most_recent_smoke` (`build_evidence.rs:153`) has **no caller** |

### 2.2 The exact guard

`src-tauri/src/agent/builtin_tools/update_task_status_tool.rs:133-162`:

```
if status == "qc_review" && ctx.agent_role.is_some() {
    let role = ctx.agent_role.as_deref().unwrap_or("");
    let is_qc_reviewer = role.starts_with("qc-") || role.starts_with("subagent-qc-dispatcher");
    if !is_qc_reviewer {
        match build_evidence::most_recent_build(&ctx.session_id) {
            None                            => return ToolResult::error(...),  // hard reject
            Some(ev) if ev.exit_code != 0   => return ToolResult::error(...),  // hard reject
            Some(_)                         => { /* ok */ }
        }
    }
}
```

`most_recent_build` (`build_evidence.rs:141-150`) filters out `Smoke` entries and
returns the newest surviving record for that **session id**.

### 2.3 Is there a Rust-native compile/verify step?

**No — plainly, no.** Every verification path in the crate ends in a subprocess
that the LLM (or a fixed command string) has to get right:

- `agent/builtin_tools/validate.rs` Tier 1 shells `python -m py_compile`
  (`:186`), `node --check` (`:221`), `tsc` (`:269`); Tier 3 shells `pyright`
  (`:753`) and friends via `run_checker` (`:816`).
- `agent/builtin_tools/e2e_probe.rs:60-78` shells `cmd /C` / `sh -c` and then
  does an HTTP GET.
- `build_evidence` itself only *observes* bash calls — it compiles nothing.

Nothing links, type-checks, or parses the produced code in-process. Even the
validators degrade to a **warning** when the tool is missing
(`validate.rs:111-121` for `tsc`, `:52-62` for `node`, `:18-28` for python), so a
machine without the toolchain writes unvalidated code and reports success.

### 2.4 VERDICT

**CONFIRMED — on Windows the `qc_review` gate is unreachable by construction, and
the command/db layer is the second half of the cause.**

1. `bash.rs:89-97` runs everything through **`cmd /C`**, but `bash.rs:404-437`
   `rewrite_for_windows` rewrites Unix pipe stages into **PowerShell** cmdlets
   (`Select-Object`, `Select-String`). cmd.exe has neither, so the rewriter
   actively converts a working command into a failing one.
2. The recipe-mandated evidence form
   (`npm run build 2>&1 | tee build.log; echo "EXIT:$?"`, quoted verbatim in the
   gate's own error message at `update_task_status_tool.rs:143-151`) uses `tee`,
   `;` and `$?` — none of which exist in cmd.exe. cmd returns the **last** pipe
   stage's status, so the recorded `exit_code` is the failure of `tee`, not the
   result of `npm run build`.
3. `build_evidence::classify` (`:94`) still matches `"npm run build"` as a
   substring, so the record is created — with a non-zero exit code.
4. `update_task_status_tool.rs:152` then rejects every `qc_review` transition
   with "the most recent build invocation exited with code N". The worker cannot
   advance the task, retries, fails identically, and the board stalls.

Corroborating on-disk evidence: `~/.autodev/build.log` (written by a worker run)
reads *"Build validation failed: Unable to execute npm commands due to Windows
path handling issues… Error: 'The filename, directory name, or volume label
syntax is incorrect'"* — the canonical cmd.exe malformed-path error.

Two secondary failure modes in the same mechanism:

- **Session-scoping (F-06).** Evidence is keyed on `ctx.session_id`. A build run
  in a subagent session and a status update issued from the parent session do not
  share a key, so the gate sees `None` and rejects even on a healthy platform.
- **No persistence (F-07).** An app restart between the build and the status
  update erases all evidence; nothing is written to any DB.

**Also note the bypass:** the Tauri command `update_task_status`
(`commands/task_commands.rs:27`) performs none of this validation, so the same
transition succeeds from the UI. Two implementations, one gated, one not.

---

## 3. BUG-DELIVER — where agent output lands

### 3.1 The path from "agent writes a file" to "user has an artifact"

1. The workspace root is a string in `~/.autodev/config.db`'s `app_settings`
   table under key `workspace_dir` (`commands/workspace_commands.rs:14,18-30`).
2. `load_workspace_dir()` (`commands/workspace_commands.rs:85-98`) reads it and
   returns **`""`** on "not set" *and* on any DB error —
   `.ok().flatten().unwrap_or_default()`. Its docstring says "Never fails."
3. Consumers each invent their own fallback:

| Consumer | Fallback when workspace_dir is empty | Location |
|---|---|---|
| Recipe fleet (all workers, supervisor, QC) | `~/.autodev/agents/<worker_slug>/` — created on the spot | `recipes/runner.rs:175-190` |
| MCP tool adapter | `std::env::current_dir()` — i.e. **`src-tauri/`** under `tauri dev` — then `"."` | `mcp_server/tool_adapter.rs:68-90` |
| Stray-file scanner | `std::env::current_dir()` | `observability/stray_files.rs:49-59` |
| Startup mismatch check | skipped entirely when `configured` is empty | `main.rs:214-234` |

4. **There is no run / launch / handoff step at all.** Nothing in the crate opens,
   reveals, or starts the produced artifact. The closest things are
   `e2e_probe` (`agent/builtin_tools/e2e_probe.rs`), which boots the app,
   HTTP-GETs it, and then **kills it** (`:105-107`), and `open_in_ide`
   (`commands/graph_commands.rs:851-862`), which emits `ide:navigate` — an event
   with **zero frontend listeners** (see §8). No `tauri_plugin_shell` open, no
   opener, no "reveal in Explorer".

### 3.2 VERDICT

**CONFIRMED. When no workspace is configured, worker output lands in
`~/.autodev/agents/<slug>/` — a hidden directory in the user's home — and when
the MCP adapter path is taken it lands in the AutoDev repo's own `src-tauri/`.
Neither is the user's project, and no step ever hands the result back.**

Live on-disk evidence in this environment:

```
~/.autodev/agents/worker-backend/count_words.py
~/.autodev/agents/worker-backend/backend/
~/.autodev/agents/worker-backend/docs/
~/.autodev/agents/worker-frontend/README.md
~/.autodev/agents/worker-frontend/docs/
~/.autodev/build.log            <- a worker's build output
~/.autodev/package-lock.json    <- an `npm install` ran with cwd = ~/.autodev
<repo>/src-tauri/.autodev/graph.db
```

The `src-tauri/` half of this is already documented inside the codebase itself —
`observability/stray_files.rs:1-17` names the exact three files a past run left
in `src-tauri/` — but the *home-directory sandbox* half (`recipes/runner.rs:180-190`)
is not treated as a bug anywhere: it is the documented, intended fallback.

### 3.3 Every CWD-relative path operation found

| # | Location | What | Effect |
|---|---|---|---|
| 1 | `commands/graph_commands.rs:404` | `output_path.unwrap_or_else(\|\| format!("graph.{}", format))` | `export_graph` writes `graph.json`/`.html`/`.svg` into the process CWD |
| 2 | `agent/builtin_tools/graph_tools.rs:176` | `Path::new(".autodev/GRAPH_REPORT.md")` | report read resolves against process CWD |
| 3 | `agent/builtin_tools/graph_tools.rs:524` | `detect_changes(store, ".", …)` | `git diff` is run with `current_dir(".")` — the process CWD, not the workspace |
| 4 | `mcp_server/tool_adapter.rs:70-88` | `std::env::current_dir()` then `"."` | default tool working_dir = `src-tauri/` in dev |
| 5 | `observability/stray_files.rs:52` | `std::env::current_dir()` | scan root |
| 6 | `runtime_config.rs:81` | `PathBuf::from(".")` when neither `USERPROFILE` nor `HOME` resolves | `./.autodev/runtime_config.json` |
| 7 | `session/types.rs:150,157` | `env::current_dir().unwrap_or_else(\|_\| PathBuf::from("."))` | default session working_dir |
| 8 | `sidecar/mod.rs:158` | `.unwrap_or_else(\|_\| PathBuf::from("."))` | sidecar spawn root |
| 9 | `main.rs:497` | `.unwrap_or_else(\|_\| PathBuf::from("."))` | — |
| 10 | `updater/install.rs:124,176,334` | `PathBuf::from(".")` / `PathBuf::from("autodev")` | install target fallback |
| 11 | `commands/graph_commands.rs:621` | `merge_all_repos_cmd(output_path: String)` | frontend-supplied path, **no workspace jail** — relative values land in CWD, absolute ones anywhere |

Note also `graph/registry.rs:62,123`: `<repo_path>/.autodev/graph.db` — not
CWD-relative, but it writes *inside the user's repo*, which is what produced the
observed `src-tauri/.autodev/graph.db`.

---

## 4. The graph subsystem

**Q1 — Does the build succeed? What breaks it?**
The happy path is coherent: `build_graph` (`commands/graph_commands.rs:94-240`)
does detect → extract → build → cluster → emit, and it does call `run_clustering`
(`:214`). Its realistic failure modes:

- `graph/detect.rs:54-55` — one unreadable directory entry aborts the whole scan
  with `Err`, killing the build (F-42). No per-entry skip. On Windows this is
  the most likely "build just fails" cause (permissions, >MAX_PATH, locked files).
- `graph/detect.rs:40-66` — symlink/junction loops recurse without a depth cap
  or visited set → stack overflow → process abort (F-41).
- `graph/detect.rs:68-73` — extension matching is case-sensitive, so `.TS`/`.PY`
  files are invisible (F-40).
- Extraction failures are **counted and warned** on this path
  (`commands/graph_commands.rs:174-177`) but never surfaced beyond a progress
  string; `BuildStats` has no error field, so a build where every file failed to
  parse returns `Ok` with zeros.
- Empty repo → `Ok(BuildStats{0,0,0})` plus a `graph:updated` event (`:146-160`)
  — success-shaped.
- Every progress emit is `let _ = app.emit(...)` (F-16), so a broken UI channel
  is invisible.

**Q2 — Where is the graph DB stored?** See §A.4. Definitively **two** schemes:
`~/.autodev/graphs/<sha256(repo_path)[..12]>.db` for all 13 store-touching
commands, and `<repo_path>/.autodev/graph.db` for the `RepoRegistry` path
(`register_repo`, `merge_all_repos_cmd`). The registry path is created but never
populated. Additionally the hash is taken over the **raw** repo string
(`graph_commands.rs:78`), so `C:\proj` vs `C:/proj` vs `c:\proj` vs a trailing
separator each produce a different database (F-10).

**Q3 — does `incremental.rs` skip clustering?** **CONFIRMED.**
`graph/incremental.rs:100-177` contains no `run_clustering` call and no
`set_communities_batch` call; the module does not even import `cluster`
(`incremental.rs:1-10`). Worse, `graph/store.rs:176-181`'s `INSERT OR REPLACE`
omits the `community` column, so an incremental update also *clears* the
community of every node it touches (F-21), and the `ON DELETE CASCADE` on
`edges` (`graph/schema.rs:82-83`) means the replace cascades away edges pointing
at those nodes (F-22).

**Q4 — the `git diff` shell-outs.** **CONFIRMED** at both sites:
`graph/changes.rs:148-161` and `graph/incremental.rs:31-41`. Both use blocking
`std::process::Command`, neither sets a timeout, and both convert a non-zero exit
into an empty result behind a single `tracing::warn!`. Assessment: this is a
silent-success failure mode — `update_graph` (`commands/graph_commands.rs:278`,
which hardcodes base `"HEAD~1"`) reports `files_changed: 0` and emits
`graph:updated`, so a repo with one commit, a shallow clone, an unborn HEAD, or
no `git` on PATH looks identical to "nothing changed". Because `update_graph` is
`async` and the call is blocking, a hung `git` also parks a tokio worker thread
indefinitely.

**Q5 — schema, migrations, indexes.** There is a real versioned mechanism
(`graph/migrations.rs:7-45`, `PRAGMA user_version`, v1→v5, each in a
transaction). Two defects: **migration v5 is inert** — it uses
`ALTER TABLE … ADD COLUMN IF NOT EXISTS`, which SQLite does not support, and every
statement is wrapped in `let _ =` (F-31) — and there is no guard against a
future version (F-32). Indexes (`graph/schema.rs:155-168`) actually cover the hot
paths well: `edges(source_qualified)`, `edges(target_qualified)`, the two
composite `(qualified, kind)` indexes, `nodes(file_path)`, `nodes(community)`.
The gap is not indexing but query shape: `get_all_nodes_for_graph(i64::MAX, 0)` /
`get_all_edges_for_graph(i64::MAX, 0)` load whole tables and are called in loops
(F-27, `graph/build.rs:156`, `graph/registry.rs:146-147,175-176`,
`graph/changes.rs:79,108`, `graph/incremental.rs:79`). And no connection anywhere
sets `busy_timeout` (F-29).

**Q6 — `NODE_RENDER_LIMIT`.** **CONFIRMED** at `commands/graph_commands.rs:650`
(`const NODE_RENDER_LIMIT: usize = 2000;`), enforced at `:737`
(`if total_code_count > NODE_RENDER_LIMIT`). `get_graph_visualization_data`
(`:663`) takes only `repo_path` — no limit, no `force_detail` flag, no
pagination. The response does carry `is_aggregated` and `total_node_count`
(`:60-62`), so the UI can *tell*, but cannot ask for more. The only way to full
detail is the unrelated pair `get_graph_nodes_for_viz(repo_path, max_nodes)`
(`:694`) + `get_graph_edges_for_viz(repo_path)` (`:723`), which return a
different JSON shape with `"degree": 0` hardcoded (`:715`) and no community data.

---

## 5. BUG-ASSIGN — task assignment and the task state machine

### 5.1 The state machine

Status values are declared **twice** and the two lists disagree:

| Source | Values |
|---|---|
| `db/tasks.rs:140-151` (`enum TaskStatus`) | ready, assigned, in_progress, qc_review, done, build_failed, failed, cancelled, rejected, **needs_info** (10) |
| `agent/builtin_tools/update_task_status_tool.rs:37` (tool JSON enum) | ready, assigned, in_progress, qc_review, done, failed, build_failed, cancelled, rejected (9 — **no `needs_info`**) |
| `agent/builtin_tools/list_tasks_tool.rs:30-31` (filter enum) | same 9 |
| `commands/task_commands.rs:10` (docstring) | 7 — omits build_failed, rejected, needs_info |

`TaskStatus::from_str` (`db/tasks.rs:163-172`) maps **anything unrecognised to
`Ready`**, so a status the enum doesn't know silently becomes a fresh dispatchable
task.

There is **no transition table anywhere.** The legal transitions are whatever the
individual guards happen to allow:

| Guard | Where | Rule |
|---|---|---|
| `ready` must have no worker | `update_task_status_tool.rs:82-88` | rejected with a message pointing at `assign_task` |
| `assigned`/`in_progress` must have a worker | `update_task_status_tool.rs:89-94` | hard error |
| worker slug must be in `FLEET_WORKERS` | `update_task_status_tool.rs:97-105`, `assign_task.rs:71-76` | hard error |
| `rejected` needs a non-empty reason | `update_task_status_tool.rs:106-114` | hard error |
| `qc_review` needs fresh, non-failing build evidence | `update_task_status_tool.rs:133-162` | hard error (see §2) |
| assignable states | `assign_task.rs:96-99` | `ready \| assigned \| failed \| rejected` |
| claim requires `status='assigned'` | `db/tasks.rs:895-903` (SQL `WHERE`) | atomic |
| one in-progress task per worker | `db/tasks.rs:877-885` | pre-flight `SELECT COUNT(*)`, non-atomic |
| QC reviewer claim | `db/tasks.rs:1414-1422` | atomic `UPDATE … WHERE under_review_by IS NULL` |
| blocked → ready | `db/tasks.rs:1343-1379` | all blockers `done` |

**None of these guards exist on the Tauri command path** (`commands/task_commands.rs:27`) —
see F-54.

### 5.2 The claim mechanism

`claim_task_with_pid` (`db/tasks.rs:873-908`):

- `claimed_by_pid` = the worker process's PID (`u32` cast to `i64`).
- `claim_host` = `COMPUTERNAME` → `HOSTNAME` → `"unknown"` (`:886-888`).
- The atomic step is `UPDATE tasks SET status='in_progress', assigned_worker=?1,
  claimed_by_pid=?2, claim_host=?3 WHERE id=?4 AND status='assigned'`.

Reconciliation lives in `orphan/mod.rs`: PID liveness first, then a 4-hour TTL
(`orphan/mod.rs:13,142-190`) — and the TTL branch is **dead** because
`parse_from_rfc3339` cannot parse SQLite's `datetime('now')` output (F-45). A
task claimed by a process on another host (`claim_host` mismatch,
`orphan/mod.rs:164`) is therefore never released at all.

### 5.3 What can silently drop or misroute a task

1. **Worker theft (F-55, the headline).** The claim's `WHERE` clause guards on
   `status='assigned'` only — it never compares `assigned_worker`. Any fleet
   worker whose tick fires can claim a task assigned to someone else and
   overwrite `assigned_worker` with its own slug. The board shows no trace; the
   `routing_history` row still names the original worker, so affinity learning
   is trained on a fiction.
2. **Silent re-route (F-56).** `assign_task` accepts `assigned` as a dispatchable
   state, so a second `assign_task` moves the task to a different worker while
   the first worker may already be mid-claim. Nothing notifies worker #1.
3. **Epics dispatched as work (F-57).** `create_epic` (`db/tasks.rs:1199-1206`)
   inserts the epic row itself with `status='ready'`, and nothing in
   `assign_task` filters `task_type='epic'`.
4. **Blocked forever on malformed JSON (F-50).** `blocked_by` parse failure →
   `None` → `unblock_ready_tasks` skips the row permanently, with no log.
5. **Non-numeric epic id (F-59).** Epic auto-completion is inside
   `if let Ok(epic_id.parse::<i64>())`; a slug epic id silently never completes.
6. **The TTL reconciler cannot fire (F-45).**
7. **`SQLITE_BUSY` (F-47).** No `busy_timeout` on `tasks.db`; every command and
   tool opens its own connection. Under a fleet tick storm, a write simply
   errors out; `commands/task_commands.rs:53,56,60,71` discard several such
   errors with `let _ =` / `if let Ok`.
8. **Two divergent implementations (F-54).** The UI's `update_task_status` skips
   every guard, fires no hooks, and emits no `qc:decision`.

### 5.4 Epic / subtask decomposition (`plan/`)

`src-tauri/src/plan/mod.rs` is **four lines** — `pub use crate::db::plans::*;`.
The real implementation is `db/plans.rs` (517 lines) plus
`commands/plan_commands.rs`. There are therefore **two parallel decomposition
systems**:

- **Task-board epics** — `tasks.epic_id / phase / phase_order / milestone /
  blocked_by` (`db/tasks.rs:1199-1300`), driven by `create_epic`,
  `create_phased_task`, `get_phase_summary`, `unblock_ready_tasks`.
- **Plans** — a separate `plans.db` with `Plan`/`PlanItem`/`PlanTree`, driven by
  `break_down_epic` (`commands/plan_commands.rs:78`), `claim_plan_item`
  (`:104`), `create_subplan` (`:130`), `submit_plan_for_qc` (`:162`).

They share nothing but the word "epic", and the plan half emits `plan:changed`
into a frontend that listens for five other names (§8) — so the plan UI is
permanently stale.

---

## 6. Per-module narratives

**`graph/`** — The build pipeline (detect → extract → build → cluster) is real and
works on the happy path, but the *incremental* pipeline is structurally
destructive: it deletes a file's nodes and edges, re-inserts only what it can see
in that one batch (`build.rs:51-57` drops every cross-file edge), re-upserts nodes
with `INSERT OR REPLACE` (wiping `community` and cascading edge deletes), and never
re-clusters. Two "engine" structs (`GraphBuilder`, `GraphRegistry`) are TODO stubs
returning `Ok(())`. Two whole modules — `cache.rs` (115 lines) and `watcher.rs`
(149 lines) — are dead. The DB path scheme is split between the commands layer and
the registry. Change detection (`changes.rs`) is effectively a no-op because of the
containment test at `:87`. `extract.rs` (1630 lines) and `analyze.rs`/`flows.rs`/
`query.rs`/`report.rs`/`export.rs` were not read line-by-line for this pass and
should be treated as unaudited.

**`commands/`** — 252 Tauri commands across 39 files. The dominant patterns are
(a) `state.lock().unwrap()` — a single panic anywhere poisons the mutex and turns
every later call on that state into a panic; (b) `let _ = app.emit(...)`
everywhere; (c) per-call `TasksDb::open()` / `GraphStore::open()` with no
connection pooling and no `busy_timeout`; (d) duplicated business logic between a
command and a same-named agent tool that do *not* agree (`update_task_status`).

**`db/`** — Six modules are wired (`tasks, autodev_instructions, model_configs,
plans, workers, annotations`); `checkpoints.rs` is a seventh that is **not
declared in `mod.rs`** and does not compile into the binary despite claiming to
back file rollback/undo. Migrations are "run `ALTER TABLE ADD COLUMN` and discard
the error" with no version tracking (`db/tasks.rs:367-412`). Timestamps mix SQLite
`datetime('now')` with Rust `to_rfc3339()` and are parsed with RFC3339 parsers in
at least two places where they were written by SQLite (F-45).

**`plan/`** — a 4-line re-export shim.

**Top level** — `errors.rs` is a clean `thiserror` enum serialised to a plain
string across IPC, which means the frontend can only pattern-match on message
text. `events.rs` centralises 30+ event-name constants and a global `OnceLock<AppHandle>`;
`emit_*_global` silently no-ops when the handle isn't installed yet
(`events.rs:23-25,415-418`). `runtime_config.rs` caches config in a
`OnceLock<Mutex<..>>` and falls back to `PathBuf::from(".")` when neither
`USERPROFILE` nor `HOME` resolves (`:81`).

---

## 7. Tauri command inventory

**252 `#[tauri::command]` functions.** Sidecar-dependent (die with the Python
sidecars — core-orchestrator / memory-mcp): **29**.

Sidecar-dependent set:

| Command | file:line | Sidecar |
|---|---|---|
| `get_sidecar_status` | `commands/sidecar_commands.rs:26` | manager |
| `check_sidecar_connectivity` | `commands/sidecar_commands.rs:37` | both |
| `get_full_sidecar_status` | `commands/sidecar_commands.rs:101` | both |
| `get_mcp_port_status` | `commands/sidecar_commands.rs:148` | mcp |
| `get_sidecar_stderr_tail` | `commands/sidecar_commands.rs:157` | both |
| `start_sidecar` | `commands/sidecar_commands.rs:170` | both |
| `stop_sidecar` | `commands/sidecar_commands.rs:230` | both |
| `restart_sidecar` | `commands/sidecar_commands.rs:246` | both |
| `list_mcp_tools` | `commands/sidecar_commands.rs:312` | memory-mcp |
| `start_memory_mcp` | `commands/sidecar_commands.rs:334` | memory-mcp |
| `stop_memory_mcp` | `commands/sidecar_commands.rs:359` | memory-mcp |
| `start_orchestrator` | `commands/sidecar_commands.rs:369` | core-orchestrator |
| `stop_orchestrator` | `commands/sidecar_commands.rs:397` | core-orchestrator |
| `get_resource_usage` | `commands/sidecar_commands.rs:420` | both |
| `read_sidecar_log` | `commands/sidecar_commands.rs:460` | both |
| `get_submodule_paths` | `commands/sidecar_commands.rs:508` | both |
| `get_mcp_health` | `commands/sidecar_commands.rs:584` | mcp |
| `force_reconnect_mcp` | `commands/sidecar_commands.rs:593` | mcp |
| `list_workers` | `commands/worker_commands.rs:50` | core-orchestrator (`OrchestratorClient`) |
| `update_worker_model_sync` | `commands/worker_commands.rs:153` | core-orchestrator |
| `spawn_subagent` | `commands/subagent_commands.rs:68` | core-orchestrator |
| `execute_subtask` | `commands/subagent_commands.rs:78` | core-orchestrator |
| `get_subagent_status` | `commands/subagent_commands.rs:87` | core-orchestrator |
| `terminate_subagent` | `commands/subagent_commands.rs:96` | core-orchestrator |
| `abort_subagent` | `commands/subagent_commands.rs:106` | core-orchestrator |
| `list_active_subagents` | `commands/subagent_commands.rs:115` | core-orchestrator |
| `get_qc_reviewer_stats` | `commands/subagent_commands.rs:130` | core-orchestrator |
| `get_startup_phase` | `commands/startup_commands.rs:26` | reads the sidecar startup FSM |
| `check_playwright_available` / `install_playwright_browsers` | `commands/browser_commands.rs:30,99` | not the Python sidecar, but shells out to a Node toolchain |

The remaining ~223 are pure Rust (SQLite, libgit2, filesystem, HTTP). The full
list with `file:line` for every command is reproduced in Appendix B.

---

## 8. Event contract drift

Rust emits 38 named events (constants at `events.rs:29-130` plus ad-hoc strings).
Cross-checking against `listen(...)` calls in `src/`:

### Emitted but never listened for

| Event | Emitter | Consequence |
|---|---|---|
| `ide:navigate` | `commands/graph_commands.rs:862` (`open_in_ide`) | **the graph view's "open in IDE" action does nothing** — the command returns `Ok(())` and no listener exists |
| `plan:changed` (`EVT_PLAN_CHANGED`, `events.rs:56`) | `commands/plan_commands.rs:67,94,117,152,174` | **the entire plan UI never live-updates** — see below |
| `agent:thinking` (`events.rs:30`) | agent loop | no consumer |
| `host:port` | startup | no consumer |
| `mcp:connection-degraded`, `mcp:reconnect-failed`, `mcp:requires-restart` (`events.rs:76,78,79`) | `McpHealthMonitor` | degraded/failed/restart states are invisible; only `health-update`, `connection-lost`, `reconnected` are consumed |

### Listened for but never emitted

| Event | Listener | Consequence |
|---|---|---|
| `plan:created` | `src/stores/planStore.ts:74` | never fires |
| `plan:updated` | `src/stores/planStore.ts:78` | never fires |
| `plan:step_completed` | `src/stores/planStore.ts:82` | never fires |
| `plan:item_claimed` | `src/stores/planStore.ts:86` | never fires |
| `plan:subplan_created` | `src/stores/planStore.ts:90` | never fires |
| `agent:stage-change`, `agent:status-change`, `agent:token-update`, `agent:tool-call-start`, `agent:tool-call-result` | agent UI | never fire; Rust emits `agent:token`, `agent:tool-start`, `agent:tool-end` |

**The `plan:*` mismatch is total**: five listeners, zero matching emitters, and
the one event Rust does emit has zero listeners. Every plan mutation
(`break_down_epic`, `claim_plan_item`, `create_subplan`, `submit_plan_for_qc`,
`delete_plan`) is invisible to the UI until a manual refetch.

Secondary: `events.rs:23-25` — `emit_*_global` no-ops when the global
`AppHandle` has not been installed, and every call site discards the result
(`let _ = app.emit(...)`), so a dropped event is undetectable from Rust too.

---

## 9. Cross-platform portability table

| Issue | Location | Windows behaviour | Severity |
|---|---|---|---|
| PowerShell rewrite fed to cmd.exe | `agent/builtin_tools/bash.rs:404-437` vs `:89-97` | `Select-Object`/`Select-String` are not cmd commands; command fails | **HIGH** |
| Recipe-mandated POSIX evidence form | `update_task_status_tool.rs:143-151` | `tee`, `;`, `$?`, `timeout` absent in cmd; exit code is `tee`'s failure | **HIGH** |
| `Command::new("tsc")` / `("pyright")` | `agent/builtin_tools/validate.rs:269,753` | npm/pip shims are `.cmd`, Rust appends `.exe` only → spawn NotFound → downgraded to a warning, file written unvalidated | **HIGH** |
| Repo path hashed verbatim | `commands/graph_commands.rs:78` | `C:\p`, `C:/p`, `c:\p`, `C:\p\` → four different graph DBs | **HIGH** |
| `fs::read_dir(dir)?` aborts the whole walk | `graph/detect.rs:54-55` | one ACL-protected dir, one >MAX_PATH path, or one locked file kills `build_graph` | **HIGH** |
| No `\\?\` / long-path handling anywhere | crate-wide (no `canonicalize` normalisation of UNC prefixes except `path_jail`) | paths over 260 chars fail opaquely | MED |
| Case-sensitive extension match | `graph/detect.rs:68-73` | `.TS`, `.PY`, `.RS` files invisible on a case-insensitive FS | MED |
| Symlink/junction recursion, no depth cap | `graph/detect.rs:40-66` | junction loop → stack overflow → process abort | MED |
| `cmd /C` child kill doesn't kill grandchildren | `agent/builtin_tools/e2e_probe.rs:60-78,105-107` | `node`/`vite` survives the probe and holds the port | MED |
| `COMPUTERNAME` → `HOSTNAME` → `"unknown"` | `db/tasks.rs:886-888` | fine on Windows; `claim_host` mismatch disables orphan release cross-host | LOW |
| Two different home-dir mechanisms | `db/mod.rs:22-25` (`USERPROFILE`/`HOME`) vs `commands/graph_commands.rs:76` & `graph/registry.rs:35` (`dirs::home_dir()`) | usually identical, can diverge under a redirected profile | LOW |
| `PathBuf::from(".")` home fallback | `runtime_config.rs:81` | config written into the CWD | LOW |
| Fixed 8 s boot wait | `agent/builtin_tools/e2e_probe.rs:35` | slower Windows toolchains yield false FAIL | LOW |

---

## Appendix B — full Tauri command list

(Generated from `#[tauri::command]` attributes; 252 entries.)
| `send_message` | `commands/agent_commands.rs:23` |
| `cancel_agent` | `commands/agent_commands.rs:210` |
| `respond_to_permission` | `commands/agent_commands.rs:221` |
| `get_dashboard_summary` | `commands/analytics_commands.rs:31` |
| `get_cost_analytics` | `commands/analytics_commands.rs:47` |
| `get_token_usage` | `commands/analytics_commands.rs:63` |
| `get_api_failures` | `commands/analytics_commands.rs:78` |
| `predict_monthly_cost` | `commands/analytics_commands.rs:85` |
| `export_costs_csv` | `commands/analytics_commands.rs:92` |
| `export_costs_json` | `commands/analytics_commands.rs:108` |
| `get_worker_analytics` | `commands/analytics_commands.rs:124` |
| `get_system_logs` | `commands/analytics_commands.rs:236` |
| `create_annotation` | `commands/annotation_commands.rs:8` |
| `list_annotations_for_file` | `commands/annotation_commands.rs:13` |
| `resolve_annotation` | `commands/annotation_commands.rs:18` |
| `update_annotation_response` | `commands/annotation_commands.rs:26` |
| `update_annotation_task_id` | `commands/annotation_commands.rs:34` |
| `delete_annotation` | `commands/annotation_commands.rs:39` |
| `get_autodev_instructions` | `commands/autodev_instructions_commands.rs:11` |
| `set_autodev_instructions` | `commands/autodev_instructions_commands.rs:19` |
| `delete_autodev_instructions` | `commands/autodev_instructions_commands.rs:26` |
| `check_autodev_instructions_exists` | `commands/autodev_instructions_commands.rs:33` |
| `get_autodev_instructions_char_count` | `commands/autodev_instructions_commands.rs:40` |
| `check_playwright_available` | `commands/browser_commands.rs:30` |
| `install_playwright_browsers` | `commands/browser_commands.rs:99` |
| `analyze_diff` | `commands/diff_commands.rs:47` |
| `analyze_pr_diff` | `commands/diff_commands.rs:57` |
| `search_workspace_files` | `commands/file_commands.rs:60` |
| `git_open_repo` | `commands/git_commands.rs:17` |
| `git_clone` | `commands/git_commands.rs:22` |
| `git_init` | `commands/git_commands.rs:29` |
| `git_status` | `commands/git_commands.rs:36` |
| `git_diff` | `commands/git_commands.rs:41` |
| `git_stage` | `commands/git_commands.rs:48` |
| `git_unstage` | `commands/git_commands.rs:53` |
| `git_discard` | `commands/git_commands.rs:58` |
| `git_commit` | `commands/git_commands.rs:63` |
| `git_push` | `commands/git_commands.rs:80` |
| `git_pull` | `commands/git_commands.rs:87` |
| `git_fetch` | `commands/git_commands.rs:94` |
| `git_branch_list` | `commands/git_commands.rs:103` |
| `git_branch_create` | `commands/git_commands.rs:108` |
| `git_branch_delete` | `commands/git_commands.rs:113` |
| `git_checkout` | `commands/git_commands.rs:118` |
| `git_worktree_list` | `commands/git_commands.rs:125` |
| `git_worktree_add` | `commands/git_commands.rs:130` |
| `git_worktree_remove` | `commands/git_commands.rs:135` |
| `git_log` | `commands/git_commands.rs:142` |
| `git_blame` | `commands/git_commands.rs:147` |
| `git_stash_list` | `commands/git_commands.rs:154` |
| `git_stash_push` | `commands/git_commands.rs:159` |
| `git_stash_apply` | `commands/git_commands.rs:164` |
| `git_stash_drop` | `commands/git_commands.rs:169` |
| `create_checkpoint` | `commands/git_commands.rs:176` |
| `restore_checkpoint` | `commands/git_commands.rs:181` |
| `list_checkpoints` | `commands/git_commands.rs:186` |
| `delete_checkpoint` | `commands/git_commands.rs:191` |
| `compare_checkpoints` | `commands/git_commands.rs:196` |
| `get_github_mcp_config` | `commands/github_mcp_commands.rs:49` |
| `set_github_mcp_config` | `commands/github_mcp_commands.rs:67` |
| `build_graph` | `commands/graph_commands.rs:95` |
| `update_graph` | `commands/graph_commands.rs:243` |
| `get_graph_stats` | `commands/graph_commands.rs:323` |
| `get_graph_communities` | `commands/graph_commands.rs:361` |
| `get_graph_report` | `commands/graph_commands.rs:375` |
| `export_graph` | `commands/graph_commands.rs:394` |
| `get_graph_nodes_for_viz` | `commands/graph_commands.rs:424` |
| `get_graph_edges_for_viz` | `commands/graph_commands.rs:453` |
| `get_god_nodes` | `commands/graph_commands.rs:482` |
| `get_surprising_connections` | `commands/graph_commands.rs:506` |
| `get_suggested_questions` | `commands/graph_commands.rs:533` |
| `search_graph_nodes` | `commands/graph_commands.rs:562` |
| `register_repo` | `commands/graph_commands.rs:602` |
| `unregister_repo` | `commands/graph_commands.rs:608` |
| `list_repos` | `commands/graph_commands.rs:614` |
| `merge_all_repos_cmd` | `commands/graph_commands.rs:620` |
| `delete_graph` | `commands/graph_commands.rs:630` |
| `get_graph_visualization_data` | `commands/graph_commands.rs:663` |
| `open_in_ide` | `commands/graph_commands.rs:860` |
| `list_hooks` | `commands/hook_commands.rs:14` |
| `enable_hook` | `commands/hook_commands.rs:21` |
| `disable_hook` | `commands/hook_commands.rs:26` |
| `add_hook` | `commands/hook_commands.rs:33` |
| `remove_hook` | `commands/hook_commands.rs:40` |
| `test_hook` | `commands/hook_commands.rs:47` |
| `get_hook_logs` | `commands/hook_commands.rs:65` |
| `read_file_tree` | `commands/ide_commands.rs:33` |
| `read_file_content` | `commands/ide_commands.rs:50` |
| `get_file_metadata` | `commands/ide_commands.rs:60` |
| `search_files` | `commands/ide_commands.rs:68` |
| `search_file_contents` | `commands/ide_commands.rs:85` |
| `get_symbol_tree` | `commands/ide_commands.rs:102` |
| `create_file` | `commands/ide_commands.rs:111` |
| `create_directory` | `commands/ide_commands.rs:125` |
| `delete_file` | `commands/ide_commands.rs:135` |
| `rename_file` | `commands/ide_commands.rs:150` |
| `move_file` | `commands/ide_commands.rs:171` |
| `resolve_symbol` | `commands/ide_commands.rs:216` |
| `find_symbol_references` | `commands/ide_commands.rs:228` |
| `get_ide_workspace_state` | `commands/ide_commands.rs:239` |
| `set_ide_workspace_state` | `commands/ide_commands.rs:244` |
| `list_log_files` | `commands/log_commands.rs:42` |
| `read_log_file` | `commands/log_commands.rs:78` |
| `read_log_file_full` | `commands/log_commands.rs:134` |
| `clear_log_files` | `commands/log_commands.rs:157` |
| `get_session_timeline` | `commands/memory_commands.rs:32` |
| `recall_facts` | `commands/memory_commands.rs:40` |
| `store_fact_cmd` | `commands/memory_commands.rs:46` |
| `update_fact_cmd` | `commands/memory_commands.rs:66` |
| `deactivate_fact` | `commands/memory_commands.rs:76` |
| `get_user_preferences` | `commands/memory_commands.rs:82` |
| `search_semantic` | `commands/memory_commands.rs:88` |
| `get_memory_md` | `commands/memory_commands.rs:96` |
| `save_memory_md` | `commands/memory_commands.rs:102` |
| `list_memory_topics` | `commands/memory_commands.rs:108` |
| `get_memory_topic` | `commands/memory_commands.rs:114` |
| `save_memory_topic` | `commands/memory_commands.rs:120` |
| `delete_memory_topic` | `commands/memory_commands.rs:136` |
| `search_procedural` | `commands/memory_commands.rs:142` |
| `search_memory` | `commands/memory_commands.rs:150` |
| `get_memory_stats` | `commands/memory_commands.rs:168` |
| `summarize_session_cmd` | `commands/memory_commands.rs:174` |
| `get_model_config` | `commands/model_config_commands.rs:13` |
| `set_model_config` | `commands/model_config_commands.rs:19` |
| `delete_model_config` | `commands/model_config_commands.rs:40` |
| `get_orphaned_tasks` | `commands/orphan_commands.rs:10` |
| `force_release_task` | `commands/orphan_commands.rs:18` |
| `planner_compile` | `commands/plan_commands.rs:28` |
| `get_plan` | `commands/plan_commands.rs:37` |
| `list_plans` | `commands/plan_commands.rs:43` |
| `get_plan_tree` | `commands/plan_commands.rs:53` |
| `delete_plan` | `commands/plan_commands.rs:64` |
| `break_down_epic` | `commands/plan_commands.rs:78` |
| `claim_plan_item` | `commands/plan_commands.rs:104` |
| `create_subplan` | `commands/plan_commands.rs:130` |
| `submit_plan_for_qc` | `commands/plan_commands.rs:162` |
| `list_plugins` | `commands/plugin_commands.rs:17` |
| `list_installed_plugins` | `commands/plugin_commands.rs:24` |
| `get_plugin` | `commands/plugin_commands.rs:31` |
| `install_plugin` | `commands/plugin_commands.rs:40` |
| `install_plugin_from_yaml` | `commands/plugin_commands.rs:47` |
| `uninstall_plugin` | `commands/plugin_commands.rs:54` |
| `update_plugin` | `commands/plugin_commands.rs:63` |
| `verify_plugin` | `commands/plugin_commands.rs:72` |
| `run_plugin` | `commands/plugin_commands.rs:81` |
| `get_plugin_config` | `commands/plugin_commands.rs:90` |
| `set_plugin_config` | `commands/plugin_commands.rs:97` |
| `generate_setup_token` | `commands/plugin_commands.rs:107` |
| `validate_setup_token` | `commands/plugin_commands.rs:121` |
| `list_recipes` | `commands/recipe_commands.rs:47` |
| `get_recipe` | `commands/recipe_commands.rs:53` |
| `list_recipe_backups` | `commands/recipe_commands.rs:60` |
| `create_recipe` | `commands/recipe_commands.rs:93` |
| `update_recipe` | `commands/recipe_commands.rs:108` |
| `delete_recipe` | `commands/recipe_commands.rs:133` |
| `restore_recipe_backup` | `commands/recipe_commands.rs:158` |
| `generate_recipe_yaml` | `commands/recipe_commands.rs:179` |
| `suggest_worker_for_task` | `commands/routing_commands.rs:11` |
| `get_worker_affinity_stats` | `commands/routing_commands.rs:23` |
| `list_sessions` | `commands/session_commands.rs:11` |
| `get_session` | `commands/session_commands.rs:18` |
| `get_session_messages` | `commands/session_commands.rs:26` |
| `delete_session` | `commands/session_commands.rs:34` |
| `create_session` | `commands/session_commands.rs:60` |
| `update_session_title` | `commands/session_commands.rs:93` |
| `set_session_pa_mode` | `commands/session_commands.rs:111` |
| `list_providers` | `commands/settings_commands.rs:27` |
| `test_provider_connection` | `commands/settings_commands.rs:58` |
| `set_active_provider` | `commands/settings_commands.rs:86` |
| `get_active_provider` | `commands/settings_commands.rs:98` |
| `get_cost_today` | `commands/settings_commands.rs:109` |
| `get_system_prompt` | `commands/settings_commands.rs:117` |
| `set_system_prompt` | `commands/settings_commands.rs:125` |
| `get_default_system_prompt` | `commands/settings_commands.rs:135` |
| `get_sidecar_status` | `commands/sidecar_commands.rs:26` |
| `check_sidecar_connectivity` | `commands/sidecar_commands.rs:37` |
| `get_full_sidecar_status` | `commands/sidecar_commands.rs:101` |
| `get_mcp_port_status` | `commands/sidecar_commands.rs:148` |
| `get_sidecar_stderr_tail` | `commands/sidecar_commands.rs:157` |
| `start_sidecar` | `commands/sidecar_commands.rs:170` |
| `stop_sidecar` | `commands/sidecar_commands.rs:230` |
| `restart_sidecar` | `commands/sidecar_commands.rs:246` |
| `list_mcp_tools` | `commands/sidecar_commands.rs:312` |
| `start_memory_mcp` | `commands/sidecar_commands.rs:334` |
| `stop_memory_mcp` | `commands/sidecar_commands.rs:359` |
| `start_orchestrator` | `commands/sidecar_commands.rs:369` |
| `stop_orchestrator` | `commands/sidecar_commands.rs:397` |
| `get_resource_usage` | `commands/sidecar_commands.rs:420` |
| `read_sidecar_log` | `commands/sidecar_commands.rs:460` |
| `get_submodule_paths` | `commands/sidecar_commands.rs:508` |
| `get_mcp_health` | `commands/sidecar_commands.rs:584` |
| `force_reconnect_mcp` | `commands/sidecar_commands.rs:593` |
| `list_skills` | `commands/skill_commands.rs:18` |
| `get_skill` | `commands/skill_commands.rs:29` |
| `search_skills` | `commands/skill_commands.rs:39` |
| `skills_by_category` | `commands/skill_commands.rs:49` |
| `validate_skill_parameters` | `commands/skill_commands.rs:58` |
| `execute_skill` | `commands/skill_commands.rs:70` |
| `reload_skills` | `commands/skill_commands.rs:93` |
| `import_skill` | `commands/skill_commands.rs:100` |
| `get_startup_phase` | `commands/startup_commands.rs:26` |
| `get_runtime_config` | `commands/startup_commands.rs:37` |
| `set_skip_sidecars` | `commands/startup_commands.rs:47` |
| `spawn_subagent` | `commands/subagent_commands.rs:68` |
| `execute_subtask` | `commands/subagent_commands.rs:78` |
| `get_subagent_status` | `commands/subagent_commands.rs:87` |
| `terminate_subagent` | `commands/subagent_commands.rs:96` |
| `abort_subagent` | `commands/subagent_commands.rs:106` |
| `list_active_subagents` | `commands/subagent_commands.rs:115` |
| `get_qc_reviewer_stats` | `commands/subagent_commands.rs:130` |
| `list_tasks` | `commands/task_commands.rs:12` |
| `get_task` | `commands/task_commands.rs:19` |
| `update_task_status` | `commands/task_commands.rs:27` |
| `delete_task` | `commands/task_commands.rs:111` |
| `get_qc_queue` | `commands/task_commands.rs:126` |
| `clear_all_tasks` | `commands/task_commands.rs:147` |
| `list_worker_requests` | `commands/task_commands.rs:165` |
| `resolve_worker_request` | `commands/task_commands.rs:172` |
| `get_epic_tasks` | `commands/task_commands.rs:187` |
| `get_epic_phase_summary` | `commands/task_commands.rs:195` |
| `create_phased_task` | `commands/task_commands.rs:205` |
| `check_for_update` | `commands/updater_commands.rs:21` |
| `get_cached_update` | `commands/updater_commands.rs:43` |
| `download_update` | `commands/updater_commands.rs:54` |
| `get_download_progress` | `commands/updater_commands.rs:161` |
| `install_update` | `commands/updater_commands.rs:171` |
| `skip_version` | `commands/updater_commands.rs:213` |
| `get_update_history` | `commands/updater_commands.rs:228` |
| `verify_data_integrity` | `commands/updater_commands.rs:236` |
| `store_provider_key` | `commands/vault_commands.rs:7` |
| `get_provider_key` | `commands/vault_commands.rs:12` |
| `delete_provider_key` | `commands/vault_commands.rs:17` |
| `store_github_pat` | `commands/vault_commands.rs:22` |
| `get_github_pat` | `commands/vault_commands.rs:27` |
| `delete_github_pat` | `commands/vault_commands.rs:32` |
| `list_workers` | `commands/worker_commands.rs:50` |
| `get_worker_by_slug` | `commands/worker_commands.rs:85` |
| `create_worker` | `commands/worker_commands.rs:90` |
| `update_worker_status` | `commands/worker_commands.rs:115` |
| `update_worker_model` | `commands/worker_commands.rs:139` |
| `update_worker_model_sync` | `commands/worker_commands.rs:150` |
| `update_worker_repos` | `commands/worker_commands.rs:201` |
| `bind_worker_recipe` | `commands/worker_commands.rs:206` |
| `delete_worker` | `commands/worker_commands.rs:211` |
| `start_worker_recipe` | `commands/worker_commands.rs:218` |
| `stop_worker_recipe` | `commands/worker_commands.rs:230` |
| `list_worker_messages_for_task` | `commands/worker_message_commands.rs:14` |
| `list_recent_worker_messages` | `commands/worker_message_commands.rs:23` |
| `count_unread_worker_messages_for` | `commands/worker_message_commands.rs:30` |
| `get_workspace_dir` | `commands/workspace_commands.rs:37` |
| `set_workspace_dir` | `commands/workspace_commands.rs:52` |
| `pick_workspace_dir` | `commands/workspace_commands.rs:72` |
