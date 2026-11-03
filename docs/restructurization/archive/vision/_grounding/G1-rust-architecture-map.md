# G1 — Rust Architecture Map & Sidecar Coupling Inventory

**Scope:** `C:\AIRS Foundry\autodev\autodev\src-tauri\` (Rust backend, 53,387 LOC across 33 top-level modules)
**Method:** Full read/grep sweep. Every load-bearing claim carries a `path:line` citation. Claims that could not be verified by reading the function body are marked **UNVERIFIED**.
**Date of sweep:** 2026-08-31
**Constraint honoured:** read-only; no builds, no tests, no edits outside this file.

---

## 0. Headline numbers

| Metric | Value | Evidence |
|---|---|---|
| Total Rust LOC under `src-tauri/src` | **53,387** | `find . -name "*.rs" \| xargs cat \| wc -l` |
| Top-level modules declared in `lib.rs` | **32** (`pub mod` lines) | `src-tauri/src/lib.rs:4-35` |
| Physical directories under `src/` | 28 + 6 top-level `.rs` files | filesystem |
| Tauri commands defined | **256** (252 × `#[tauri::command]`, 4 × `#[command]`) | see §B |
| Tauri commands registered | **256** — zero dead, zero missing | `src-tauri/src/lib.rs:167-470` |
| Built-in agent tools | **40 modules** in `agent/builtin_tools/` (9,806 LOC) | `agent/builtin_tools/mod.rs:6-72` |
| SQLite databases | **10** (+ per-repo graph DBs) | see §E |
| Files with real sidecar coupling | **13** (out of 61 that merely mention it) | see §C |
| Capabilities that must be PORTED | **9** | see §C.4 |

---

# Section A — Module map

Ordering: alphabetical by path. "Wired?" = reachable from `lib.rs`/`main.rs` at runtime.

### A.1 Top-level files

| Path | LOC | Purpose (read from code) | Public surface | Internal deps | Wired? |
|---|---|---|---|---|---|
| `src/lib.rs` | 471 | Crate root. Declares all 32 modules; `register_commands()` runs `tauri::generate_handler!` inside the lib crate so the `__cmd__*` macros are in scope. | `register_commands(Builder<Wry>) -> Builder<Wry>` (`lib.rs:41`) | all `commands::*` | **Yes — entry point** |
| `src/main.rs` | 519 | Binary entry. Loads `.env` via dotenvy (`main.rs:28`), canonicalizes `AUTODEV_SIDECAR_PATH`/`AUTODEV_MEMORY_PATH` (`main.rs:36-60`), installs panic hook, logging, recipes, skills, MCP registry, AppState, SessionManager, `SidecarManager`, `OrchestratorClient`, `SubagentManager`, analytics singleton, updater state, MCP health monitor; then the `.setup()` closure spawns the MCP server, sidecar auto-start, worker auto-start, skill watcher, and background update check. | `fn main()` | everything | **Yes** |
| `src/errors.rs` | 72 | `AutoDevError` thiserror enum with 12 variants incl. `BudgetExceeded`, plus `From` impls for io/rusqlite/reqwest/serde_json/anyhow. | `AutoDevError`, `Result<T>` | — | Yes (universal) |
| `src/events.rs` | 528 | Tauri event constants + typed payload structs + `install_app_handle` global `OnceLock<AppHandle>` so background subsystems emit without `State<>` plumbing. Includes `EVT_SIDECAR_CRASH` (`events.rs:108`), subagent events, worker tick events, cost updates, workspace strays/mismatch, MCP port conflict. | `install_app_handle`, `app_handle()`, `emit_*` fns, `*Payload` structs | `state`, `subagent::SubagentEvent` | Yes (`main.rs:194`) |
| `src/logging.rs` | 102 | `tracing_subscriber` init with daily rolling file appender into `~/.autodev/logs/`. Returns a `WorkerGuard` that main.rs must hold (`main.rs:76`). | `init() -> WorkerGuard` | — | Yes |
| `src/runtime_config.rs` | 176 | `~/.autodev/runtime_config.json`, read before Tauri state exists. Fields: `skip_sidecars` (Demo Mode) + 4 default-budget fields. Cached in a `OnceLock<Mutex<..>>` (`runtime_config.rs:98-102`). | `RuntimeConfig`, `read()`, `write()`, `invalidate()` | — | Yes (`main.rs:284`) |

### A.2 `agent/` — 1,652 LOC (excl. builtin_tools)

Purpose: the PA-mode agentic loop. Declared `agent/mod.rs:7-14`; re-exports `AgentEngine`, `Tool`, `ToolRegistry`, `PermissionMode` (`agent/mod.rs:16-19`) and the `PAMode` enum (`agent/mod.rs:24-31`).

| File | LOC | Purpose | Key surface | Deps | Wired? |
|---|---|---|---|---|---|
| `agent/engine.rs` | 537 | The loop: stream → collect tool calls → execute batch → append results → repeat, max 20 turns (`engine.rs:34`). Fires `AgentLoopStart`/`AgentLoopEnd`/`OnError` hooks, enforces budgets, compacts context, records cost + analytics, emits `agent:done`. | `AgentEngine::new/with_session_manager/with_task_budget/run` | `provider`, `hooks`, `cost`, `budget`, `analytics`, `db::tasks`, `events`, `session` | **Yes** (`commands/agent_commands.rs`, `recipes/runner.rs:663`) |
| `agent/tool_registry.rs` | 40 | `HashMap<String, Arc<dyn Tool>>` + `to_definitions()` producing sorted `Vec<ToolDefinition>`. | `ToolRegistry` | `provider::types` | Yes |
| `agent/tool_trait.rs` | 68 | `Tool` async trait: `name/description/input_schema/is_read_only/requires_permission/execute`. `ToolResult` carries `output`, `is_error`, optional `hint`. `ToolContext` carries `session_id`, `working_dir`, `call_id`, `depth`, `agent_role`. | `Tool`, `ToolResult`, `ToolContext` | — | Yes |
| `agent/tool_executor.rs` | 459 | Permission gating + partitioned concurrency: read-only tools run concurrently behind a `Semaphore(10)` (`tool_executor.rs:25,231`), write tools serially. Fires `PreToolUse`/`Post*` hooks. | `ToolExecutor::new/with_session_manager/execute/execute_batch` | `hooks`, `permission`, `events`, `session` | Yes |
| `agent/permission.rs` | 46 | `PermissionMode` (ManualApproval/Autonomous/SmartApprove), `PermissionRequest/Response`, `PendingPermissions = Arc<Mutex<HashMap<call_id, oneshot::Sender>>>`. | as listed | — | Yes (`main.rs:122`) |
| `agent/context.rs` | 302 | System-prompt assembly. `build_system_prompt_with_mode` picks one of three 100+ line prompt constants, then layers global + repo `AUTODEV.md` from `config.db` (fallback to filesystem), then the memory block. | `ContextBuilder`, `default_system_prompt()` | `db::autodev_instructions`, `memory` | Yes (`engine.rs:101`) |
| `agent/compaction.rs` | 159 | Compacts at 80% of context (`compaction.rs:13`), keeping first + last 10 messages; LLM-summary path with truncation fallback. Token estimate = chars/4. | `CompactionEngine::{needs_compaction,estimate_tokens,compact_truncate,compact_with_summary}` | `provider` | Yes (`engine.rs:175-185`) |
| `agent/builtin_tools/` | 9,806 | 40 tool modules + `path_jail.rs` (workspace escape guard), `path_lock.rs` (process-wide per-file async mutex), `build_evidence.rs` (build-ran proof for qc_review gating), `graph_tools.rs`/`graph_prompts.rs` (918 LOC, **not** registered in either registry — see A.2.1). Three registration fns: `register_all` (`mod.rs:78`), `register_for_mcp_worker` (`mod.rs:96`), `register_for_mode` (`mod.rs:174`). | 40 `*Tool` structs | `db::*`, `subagent`, `routing`, `budget`, `diff_analysis`, `memory`, `graph` | Yes |

**A.2.1 Dead code in `builtin_tools/`:** `graph_tools.rs` (840 LOC) and `graph_prompts.rs` (78 LOC) are **not declared** in `agent/builtin_tools/mod.rs:6-72` — there is no `pub mod graph_tools;` line. They are therefore not compiled into the crate at all. **918 LOC of dead files.** (Verified: `grep -rn "graph_tools" mod.rs` returns nothing.)

### A.3 `analytics/` — 1,577 LOC

Purpose: cost/token/worker/failure telemetry into `~/.autodev/analytics.db` (`analytics/mod.rs:5`).
Files: `db.rs` (658 — schema + connection, 5 tables), `collector.rs` (136 — write path), `aggregator.rs` (83 — hourly/daily rollups), `queries.rs` (282 — dashboard reads; opens `workers.db` read-only at `queries.rs:83-85`), `exporter.rs` (76 — CSV/JSON), `types.rs` (216 — DTOs).
Public surface: `AnalyticsManager` + `install_global`/`global()` (`analytics/mod.rs:112-136`). `global()` **panics** if `install_global` was not called (`analytics/mod.rs:127-136`) — a deliberate loud-failure choice documented in the doc comment.
Wired: **Yes** — `main.rs:146-152` blocks on `AnalyticsManager::new()`, installs the global, starts the rollup thread.
Note: `aggregator.rs:21` contains `// For now, this is a placeholder for future implementation` — see §D for the verdict.

### A.4 `budget/` — 130 LOC (single `mod.rs`)

Purpose: Budget Circuit Breaker types + validation. `TaskBudget{max_tokens,max_usd,max_turns}`, `BudgetStatus`, `BudgetExceededReason`, presets `default_worker()`/`default_supervisor()`/`unlimited()`, `validate_budget()` with ceilings 1M tokens / $100 / 100 turns (`budget/mod.rs:89-91`).
Wired: **partially — see FINDING F3.** `TaskBudget::check` is called from `engine.rs:151`, but only when `self.task_budget` is `Some`, and **nothing ever calls `AgentEngine::with_task_budget`** (only definition `engine.rs:73` and a comment `engine.rs:45`). The enforcement path is therefore inert in production.

### A.5 `commands/` — 5,738 LOC, 32 files

Purpose: thin Tauri command wrappers (`commands/mod.rs:1-3`). All modules declared `commands/mod.rs:5-37`. Every file's commands are registered in `lib.rs`. Full enumeration in §B.

### A.6 `cost/` — 64 LOC

`cost/tracker.rs`: `CostTracker::calculate(model, in, out) -> MessageCost` using a hardcoded 7-arm pricing table (`cost/tracker.rs:50-59`), plus `format_cost`/`format_tokens`. Wired: Yes (`engine.rs:355`, `recipes/runner.rs:382`).
**FINDING F8 (bug):** the match arms are ordered `gpt-4o` before `gpt-4o-mini` (`cost/tracker.rs:53-54`). Because both use `contains`, every `gpt-4o-mini` call is priced at **$2.50/$10.00** instead of **$0.15/$0.60** — a 16× overcharge on input. Unreachable arm.

### A.7 `db/` — 3,249 LOC

`db/mod.rs:10-15` declares six repositories; `autodev_dir()` returns `~/.autodev` (`db/mod.rs:21-29`).

| File | LOC | Owns | DB file |
|---|---|---|---|
| `db/tasks.rs` | 1,565 | `tasks`, `worker_requests`, `worker_messages`, `routing_history` | `~/.autodev/tasks.db` (`db/tasks.rs:287`) |
| `db/plans.rs` | 517 | `plans`, `plan_items`, `plan_item_dependencies`, `plan_executions` | `~/.autodev/plans.db` (`db/plans.rs:205`) |
| `db/workers.rs` | 387 | `workers` (+ seed of 7 fleet workers, `db/workers.rs:185`) | `~/.autodev/workers.db` (`db/workers.rs:111`) |
| `db/checkpoints.rs` | 280 | `file_checkpoints` | `~/.autodev/checkpoints.db` (`db/checkpoints.rs:67`) |
| `db/annotations.rs` | 217 | `annotations` | `~/.autodev/annotations.db` (`db/annotations.rs:51`) |
| `db/autodev_instructions.rs` | 135 | `autodev_instructions` | `~/.autodev/config.db` (`db/autodev_instructions.rs:34`) |
| `db/model_configs.rs` | 119 | `model_configs` | `~/.autodev/config.db` (`db/model_configs.rs:44`) |

Wired: **Yes** for all seven. **FINDING F4:** `model_configs` is written by `set_model_config` (`commands/model_config_commands.rs:19-35`) but read by nothing except `get_model_config` returning it to the frontend — see §D.

### A.8 `diff_analysis/` — 548 LOC

`mod.rs` (77) exposes `DiffAnalyzer{provider, model}` with `analyze()` composing `parser::parse_unified_diff` → `risk_scorer::compute_structural_risk` → `classifier::classify_diff` (LLM call). `parser.rs` (169), `risk_scorer.rs` (136), `classifier.rs` (166 — issues a real `CompletionRequest` at `classifier.rs:138`).
Wired: **Yes** via `commands/diff_commands.rs:42` and `agent/builtin_tools/analyze_diff_tool.rs:239`.

### A.9 `git/` — 1,882 LOC, 13 files

Synchronous `git2`/libgit2 wrapper (`git/mod.rs:3-18`): `repo`, `status`, `diff`, `commit`, `branch`, `remote`, `merge`, `worktree`, `history`, `stash`, `checkpoint`, `types`.
Wired: **Yes** — 30 commands in `commands/git_commands.rs`.
Note: `git/merge.rs:4` — conflict *resolution* is explicitly deferred; only conflict-marker writing is supported.

### A.10 `graph/` — 5,623 LOC, 17 files

Tree-sitter code knowledge graph persisted to per-repo SQLite (`graph/mod.rs:1-3`). `extract.rs` (1,630) is the tree-sitter AST extractor; `store.rs` (738) the SQLite layer; `cluster.rs` (464) real community detection; `analyze.rs` (511) real graph diffing; `schema.rs`/`migrations.rs` the DDL and 5-step migration ladder.
Wired: **Yes** — 19 commands in `commands/graph_commands.rs`, each opening a per-repo store via `repo_db_path()` (`commands/graph_commands.rs:74-80`).
**FINDING F6 (dead public API):** seven façade structs are exported from `graph/mod.rs:27-31` and each has exactly one method that ignores its arguments and returns `Ok(())`:
- `GraphBuilder::build_graph` — `graph/build.rs:21-24`
- `Clusterer::cluster_graph` — `graph/cluster.rs:460-463`
- `Analyzer::analyze_graph` — `graph/analyze.rs:507-510`
- `QueryEngine::query_graph` — `graph/query.rs:256-259`
- `GraphRegistry::manage_repos` — `graph/registry.rs:202-205`
- `Exporter::export_graph` — `graph/export.rs:187-190`
- `FlowAnalyzer::analyze_flows` — `graph/flows.rs:191-194`
None is called anywhere (`grep -rn "GraphBuilder::\|Clusterer::\|..."` returns only unrelated `DiffAnalyzer::`/`AnalyticsExporter::` hits). The real work is done by free functions in the same files. These are **inert-but-green traps**: any future caller gets a silent success and no graph.
`GraphWatcher` (`graph/watcher.rs`, 149 LOC) is exported (`graph/mod.rs:32`) but **never instantiated** — dead.

### A.11 `hooks/` — 1,001 LOC

Event-driven extensibility (`hooks/mod.rs:2-3`). `types.rs` (231 — `HookType` enum, `HookContext`), `conditions.rs` (151 — rhai expression evaluation), `executor.rs` (190 — shell exec + `{{field}}` templating), `registry.rs` (85 — `HookRegistry::fire`/`fire_with_logs`), `db.rs` (241 — `hooks`, `hook_logs` in `config.db`), `ecosystem.rs` (85 — project ecosystem detection incl. `PythonUv`).
Wired: **Yes** — fired from 14 call sites across `engine.rs`, `tool_executor.rs`, `session_commands.rs`, `read_file.rs`, `write_file.rs`, `edit_file.rs`, `assign_task.rs`, `update_task_status_tool.rs`. 7 commands.

### A.12 `ide/` — 1,456 LOC

Backend for the IDE tab (`ide/mod.rs:4-12`): `file_tree.rs` (280, lazy placeholder dirs), `file_reader.rs` (263), `file_search.rs` (240), `symbols.rs` (144 — regex-based, Rust/TS/JS/Python/Go), `symbol_nav.rs` (229), `workspace.rs` (102 — `app_settings` in `config.db`), `types.rs` (177).
Wired: **Yes** — 15 commands.
Note: `ide/symbols.rs:8` states a future phase should swap this for tree-sitter — the `graph/extract.rs` tree-sitter extractor already exists in the same crate but is not shared. Duplicated symbol extraction.

### A.13 `mcp_server/` — 1,105 LOC

Exposes Rust builtin tools over MCP Streamable HTTP so **core-orchestrator LivingAgents** can call them (`mcp_server/mod.rs:1-9`). `server.rs` (228 — axum + rmcp on 127.0.0.1:8767 with fallbacks 8768-8770, `server.rs:30,35`), `tool_adapter.rs` (286 — `Tool` trait → MCP bridge, reads `__depth`), `health_monitor.rs` (400 — polls **external** MCP servers, e.g. GitHub MCP), `port_status.rs` (172 — netstat-based conflict diagnosis).
Wired: **Yes** — spawned at `main.rs:240-244`.
**FINDING F7:** `start_mcp_server` deliberately discards the registry passed to it (`server.rs:149 let _ = registry;`) and builds its own with `register_for_mcp_worker` (`server.rs:151`). Meanwhile `main.rs:108-110` builds a *different* registry with `register_all` (= the FastDevelopment set) and manages it as Tauri state, which is what `list_mcp_tools` reads (`commands/sidecar_commands.rs:312-326`). **The MCP tool browser in the UI shows a different tool set than the server actually serves.**

### A.14 `memory/` — 1,586 LOC

3-tier PA memory in `~/.autodev/memory.db` (`memory/mod.rs:1-19`). See §D for maturity.
`db.rs` (123 — 4 tables, WAL), `episodic.rs` (226), `semantic.rs` (295), `procedural.rs` (299 — MEMORY.md + topics/*.md mirrored to SQLite), `recall.rs` (187 — cross-tier assembler with limits 5/10/3, `recall.rs:23-25`), `summarizer.rs` (154 — rule-based), `migration.rs` (99 — backfill from `~/.airs/autodev.db`), `types.rs` (164).
Wired: **Yes** — 17 commands; `build_memory_context` injected at `agent/context.rs:118`; writes from `commands/agent_commands.rs:77,83,183,190` and `agent/builtin_tools/remember_this.rs:81`.

### A.15 `observability/` — 430 LOC

`error_kinds.rs` (84 — log-error taxonomy; several variants describe the Python sidecar), `log_retention.rs` (142 — prune > N days, default 14, `main.rs:88`), `stray_files.rs` (195 — startup scan for worker-generated junk, `main.rs:202`).
Wired: **Yes.**
**FINDING (docstring):** `observability/error_kinds.rs:15` points at `src-tauri/src/sidecar/watchdog.rs::is_healthy`. **No file `sidecar/watchdog.rs` exists** — the watchdog lives in `sidecar/mod.rs:1054-1290`, and `is_healthy` is in `sidecar/client.rs:428`. Stale cross-reference.

### A.16 `orphan/` — 239 LOC

PID-based orphan task detection + stranded QC-claim sweep (`orphan/mod.rs:1-6`). `reconcile()` (`orphan/mod.rs:18`), `pid_checker.rs` (38 — `is_pid_alive`, `is_autodev_process`). Cross-host safety via `claim_host` comparison (`orphan/mod.rs:170-183`), 4-hour TTL fallback (`orphan/mod.rs:14`).
Wired: **Yes** — `recipes/scheduler.rs:130,230`; 2 commands.

### A.17 `plan/` — 4 LOC

Pure re-export shim: `pub use crate::db::plans::*;` (`plan/mod.rs:4`). No own code.

### A.18 `plugin/` — 1,503 LOC

Marketplace/installer for worker-environment plugins. `manifest.rs` (209), `builtins.rs` (258 — built-in plugin catalog with `python3 --version`, `uv --version` checks), `registry.rs` (214), `runner.rs` (269), `token.rs` (296 — HMAC setup tokens), `manager.rs` (241).
Wired: **Yes** — 13 commands; `PluginManager::new()` is constructed per-command (stateless).

### A.19 `provider/` — 1,802 LOC

LLM provider abstraction (`provider/mod.rs:1-3`). `traits.rs` (31), `types.rs` (82), `streaming.rs` (76 — Anthropic SSE parser), `anthropic.rs` (277), `openai.rs` (423), `google.rs` (392), `ollama.rs` (506). See §D for the full capability assessment.
Wired: **Yes** — `commands/settings_commands.rs:30-34`, `commands/agent_commands.rs`, `recipes/runner.rs:646-652`, `commands/diff_commands.rs`.

### A.20 `recipes/` — 3,645 LOC

YAML worker workflows in `~/.autodev/recipes/` (`recipes/mod.rs:1-3`). `types.rs` (81 — `Recipe`, `FallbackPolicy`), `parser.rs` (75), `defaults.rs` (1,966 — the seven default fleet recipes as embedded markdown/YAML), `runner.rs` (924 — **the dual-mode dispatcher**, see §C), `scheduler.rs` (558 — cron ticks, `install_global`, orphan reconcile).
Wired: **Yes** — `recipes::init()` at `main.rs:92`; `RecipeRunner`/`RecipeScheduler` built at `main.rs:292-305`; 8 commands.

### A.21 `routing/` — 333 LOC

Semantic task routing: `patterns.rs` (66 — static keyword match), `scorer.rs` (141 — historical affinity), `mod.rs` (126 — `TaskRouter::suggest_worker/record_assignment/record_outcome_auto/get_affinity_stats`).
Wired: **Yes** — `suggest_worker` from `suggest_worker_tool.rs:57` + `routing_commands.rs:18`; `record_assignment` from `assign_task.rs:114`, `assign_tasks_batch.rs:190`; `record_outcome_auto` from `task_commands.rs:53,56,60`. Full read/write loop verified.

### A.22 `session/` — 550 LOC

UI-level conversation persistence, explicitly distinct from orchestrator memory (`session/mod.rs:2-3`). `types.rs` (168), `storage.rs` (304 — `~/.airs/autodev.db`, tables `sessions`/`messages`, `session/storage.rs:293`), `manager.rs` (68 — 11 thin methods).
Wired: **Yes** — `main.rs:116`; 7 commands.

### A.23 `sidecar/` — 3,161 LOC — **THE DELETION TARGET**

| File | LOC | Purpose |
|---|---|---|
| `sidecar/mod.rs` | 1,283 | `SidecarManager`: spawn/stop/status for memory-mcp (:8000) and core-orchestrator (:4000); log routing + rotation into `~/.autodev/logs/`; stderr tail ring buffer (60 lines, `mod.rs:37`); env-var stripping for the Python children (`mod.rs:74-82`); Redis/Memurai service starter (`mod.rs:760`); path resolvers; two watchdogs with exponential backoff 2s→120s (`mod.rs:1046-1052`). |
| `sidecar/client.rs` | 1,656 | `OrchestratorClient` — the entire HTTP surface against core-orchestrator (JWT auth, agent CRUD, invoke, SSE stream, model registry, MCP registration, health probes). |
| `sidecar/startup_progress.rs` | 222 | `StartupPhase` FSM (Init → MemoryStarting → VectorDb → MemoryReady → OrchStarting → Routers → Agents → LivingAgents → OrchReady → Workers → FleetReady → Ready) driving the loading overlay. Monotonic `advance()`. |

Wired: **Yes** — `main.rs:126-127, 275, 310-461`; 18 commands + 3 startup commands.

### A.24 `skills/` — 1,569 LOC

Markdown workflow templates in `~/.autodev/skills/` (`skills/mod.rs:1-5`). `types.rs` (161), `loader.rs` (123), `engine.rs` (103 — `validate` + `render` with `{{param}}` substitution), `registry.rs` (95), `defaults.rs` (958 — embedded default skills), `watcher.rs` (112 — `notify`-based hot reload).
Wired: **Yes** — `main.rs:97-103, 499-503`; 8 commands.

### A.25 `state/` — 67 LOC

`AppState`: active agents map, active provider/model, today's cost + token counters, system-prompt override (`state/app_state.rs:14-34`). `cancel_agent`, `is_cancelled`, `add_cost`.
Wired: **Yes** — `main.rs:112`.

### A.26 `subagent/` — 1,351 LOC — **100 % SIDECAR-DEPENDENT**

`types.rs` (223), `slug.rs` (98), `bridge.rs` (442 — the create→invoke(stream)→delete lifecycle against core-orchestrator + the SSE frame parser reused by `client.rs`), `pool.rs` (205 — `MAX_GLOBAL_CONCURRENT`/`MAX_PER_PARENT`/`MAX_NESTING_DEPTH` caps), `manager.rs` (356 — registry, event pump, orphan sweep, `install_global`/`global()`).
Wired: **Yes** — `main.rs:133-134`; 7 commands; 3 builtin tools.
There is **no local execution fallback**: `SubagentManager::new` takes an `Arc<OrchestratorClient>` (`subagent/manager.rs:61`) and `bridge::run` calls `create_ephemeral_agent` → `invoke_agent_stream_raw` → `delete_agent` (`bridge.rs:99,221,130`).

### A.27 `updater/` — 1,382 LOC

GitHub-Releases auto-updater (`updater/mod.rs:1-9`). `check.rs` (223), `download.rs` (186), `verify.rs` (120), `patch.rs` (189), `install.rs` (369), `persistence.rs` (211).
Wired: **Yes** — `main.rs:155, 508-512`; 8 commands.
**FINDING F9 (security):** `updater/verify.rs:13-14` embeds a base64 string that decodes to the ASCII text `undefined-public-key-for-development-only-placeholder` (51 bytes ≠ 32). In debug builds signature verification returns `Ok(())` unconditionally when the key length is wrong (`verify.rs:54-56`) or when no signature is present (`verify.rs:39-41`). Release builds bail — so the shipped updater is *hard-broken* until `AUTODEV_UPDATE_PUBKEY` is set at build time.
**FINDING (stub):** `updater/patch.rs:135` — `// TODO: Add bzip2 = "0.4" to Cargo.toml and implement proper decompression.` `bzip2` is absent from `Cargo.toml`, so bsdiff delta patching is not functional. **UNVERIFIED** whether the delta path is reachable at runtime.

### A.28 `vault/` — 66 LOC

OS keychain wrapper via the `keyring` crate (`vault/mod.rs:1-5`): `VaultManager::{store,get,delete}` + key namers `provider_key(provider)` / `github_pat_key()`.
Wired: **Yes** — 6 commands; read from `agent_commands.rs`, `settings_commands.rs`, `recipes/runner.rs:641`.

---

# Section B — Tauri command surface

**Total: 256.** 252 use `#[tauri::command]`; 4 use the bare `#[command]` form imported at `commands/budget_commands.rs:6`. All 256 are registered in `lib.rs:167-470`. **Zero orphaned commands, zero registered-but-undefined.** (Verified by set-differencing the defined list against the registered list.)

Two additional `#[tauri::command]` grep hits are inside comments and are **not** commands: `lib.rs:39` and `main.rs:173`.

## B.1 Commands that touch the sidecar

**Direct (24):**

| Command | file:line | What it does with the sidecar |
|---|---|---|
| `check_sidecar_connectivity` | `commands/sidecar_commands.rs:38` | `is_memory_healthy` + `is_healthy` + `is_redis_healthy` (`:41-43`) |
| `get_sidecar_status` | `:26` | reads `SidecarManager` status |
| `get_full_sidecar_status` | `:101` | aggregate of all four services |
| `get_mcp_port_status` | `:148` | port conflict snapshot |
| `get_sidecar_stderr_tail` | `:157` | Python stderr ring buffer |
| `start_sidecar` | `:170` | spawns both Python processes (`:176-217`) |
| `stop_sidecar` | `:230` | kills both (`:235`) |
| `restart_sidecar` | `:246` | stop+start (`:252-286`) |
| `list_mcp_tools` | `:312` | reads the Tauri-managed `ToolRegistry` (see FINDING F7) |
| `start_memory_mcp` | `:334` | spawns memory-mcp (`:338-347`) |
| `stop_memory_mcp` | `:359` | kills memory-mcp |
| `start_orchestrator` | `:369` | spawns core-orchestrator (`:374-386`) |
| `stop_orchestrator` | `:397` | kills core-orchestrator (`:402`) |
| `get_resource_usage` | `:420` | `sysinfo` CPU/RAM of the Python PIDs |
| `read_sidecar_log` | `:460` | reads `~/.autodev/logs/core-orchestrator.log` etc. |
| `get_submodule_paths` | `:508` | resolves both submodule dirs for the Settings UI |
| `get_mcp_health` | `:584` | external MCP health (GitHub MCP) — *not* core-orchestrator |
| `force_reconnect_mcp` | `:593` | ditto |
| `get_startup_phase` | `commands/startup_commands.rs:26` | reads the sidecar-driven `StartupProgress` FSM |
| `get_runtime_config` | `:37` | exposes `skip_sidecars` |
| `set_skip_sidecars` | `:47` | writes the Demo-Mode flag that suppresses sidecar spawn |
| `list_workers` | `commands/worker_commands.rs:50` | `orch_client.is_healthy()` then `fetch_agent_models()` (`:58-59`); reads `{orch}/config/agents/{slug}.yaml` (`:74-76`) |
| `update_worker_model_sync` | `:150` | `update_agent_model_by_slug` (`:163-164`) + `write_yaml_model` into the submodule tree (`:181-182`) |
| `update_worker_model` | `:139` | writes DB; YAML/orch sync happens via the `_sync` variant |

**Transitive (5):**

| Command | file:line | Path to the sidecar |
|---|---|---|
| `spawn_subagent` | `commands/subagent_commands.rs:68` | → `SubagentManager::spawn` → `bridge::run` → `create_ephemeral_agent` (`subagent/bridge.rs:99`) |
| `execute_subtask` | `:78` | same |
| `get_subagent_status` | `:87` | registry read only; registry only ever populated by sidecar spawns |
| `terminate_subagent` | `:96` | → `client.delete_agent` (`subagent/manager.rs:214`) |
| `abort_subagent` | `:106` | same |
| `list_active_subagents` | `:115` | registry read |
| `get_qc_reviewer_stats` | `:130` | registry read |
| `start_worker_recipe` / `stop_worker_recipe` | `commands/worker_commands.rs:218,230` | → `RecipeScheduler` → `RecipeRunner::run_recipe` → Mode 2 dispatch (`recipes/runner.rs:110,209,355`) |

**Everything else (≈227 commands)** — sessions, settings, vault, tasks, recipes, git (30), IDE (15), memory (17), graph (19), analytics (9), plugins (13), updater (8), skills (8), hooks (7), plans (9), annotations (6), routing (2), orphan (2), diff (2), budget (4), browser (2), worker messages (3), workspace (3), logs (4), files (1), model config (3), github MCP (2), AUTODEV.md (5), worker CRUD (9) — is **sidecar-free**.

## B.2 Full enumeration (by file, `file:line name`)

<details><summary>256 commands</summary>

**agent_commands.rs (3):** `:23 send_message` · `:210 cancel_agent` · `:221 respond_to_permission`
**analytics_commands.rs (9):** `:31 get_dashboard_summary` · `:47 get_cost_analytics` · `:63 get_token_usage` · `:78 get_api_failures` · `:85 predict_monthly_cost` · `:92 export_costs_csv` · `:108 export_costs_json` · `:124 get_worker_analytics` · `:236 get_system_logs`
**annotation_commands.rs (6):** `:8 create_annotation` · `:13 list_annotations_for_file` · `:18 resolve_annotation` · `:26 update_annotation_response` · `:34 update_annotation_task_id` · `:39 delete_annotation`
**autodev_instructions_commands.rs (5):** `:11 get_autodev_instructions` · `:19 set_autodev_instructions` · `:26 delete_autodev_instructions` · `:33 check_autodev_instructions_exists` · `:40 get_autodev_instructions_char_count`
**browser_commands.rs (2):** `:30 check_playwright_available` · `:99 install_playwright_browsers`
**budget_commands.rs (4, `#[command]`):** `:19 cmd_set_task_budget` · `:38 get_task_budget_status` · `:48 get_daily_budget_summary` · `:59 set_default_worker_budget`
**diff_commands.rs (2):** `:47 analyze_diff` · `:57 analyze_pr_diff`
**file_commands.rs (1):** `:60 search_workspace_files`
**git_commands.rs (30):** `:17 git_open_repo` · `:22 git_clone` · `:29 git_init` · `:36 git_status` · `:41 git_diff` · `:48 git_stage` · `:53 git_unstage` · `:58 git_discard` · `:63 git_commit` · `:80 git_push` · `:87 git_pull` · `:94 git_fetch` · `:103 git_branch_list` · `:108 git_branch_create` · `:113 git_branch_delete` · `:118 git_checkout` · `:125 git_worktree_list` · `:130 git_worktree_add` · `:135 git_worktree_remove` · `:142 git_log` · `:147 git_blame` · `:154 git_stash_list` · `:159 git_stash_push` · `:164 git_stash_apply` · `:169 git_stash_drop` · `:176 create_checkpoint` · `:181 restore_checkpoint` · `:186 list_checkpoints` · `:191 delete_checkpoint` · `:196 compare_checkpoints`
**github_mcp_commands.rs (2):** `:49 get_github_mcp_config` · `:67 set_github_mcp_config`
**graph_commands.rs (19):** `:95 build_graph` · `:243 update_graph` · `:323 get_graph_stats` · `:361 get_graph_communities` · `:375 get_graph_report` · `:394 export_graph` · `:424 get_graph_nodes_for_viz` · `:453 get_graph_edges_for_viz` · `:482 get_god_nodes` · `:506 get_surprising_connections` · `:533 get_suggested_questions` · `:562 search_graph_nodes` · `:602 register_repo` · `:608 unregister_repo` · `:614 list_repos` · `:620 merge_all_repos_cmd` · `:630 delete_graph` · `:663 get_graph_visualization_data` · `:860 open_in_ide`
**hook_commands.rs (7):** `:14 list_hooks` · `:21 enable_hook` · `:26 disable_hook` · `:33 add_hook` · `:40 remove_hook` · `:47 test_hook` · `:65 get_hook_logs`
**ide_commands.rs (15):** `:33 read_file_tree` · `:50 read_file_content` · `:60 get_file_metadata` · `:68 search_files` · `:85 search_file_contents` · `:102 get_symbol_tree` · `:111 create_file` · `:125 create_directory` · `:135 delete_file` · `:150 rename_file` · `:171 move_file` · `:216 resolve_symbol` · `:228 find_symbol_references` · `:239 get_ide_workspace_state` · `:244 set_ide_workspace_state`
**log_commands.rs (4):** `:42 list_log_files` · `:78 read_log_file` · `:134 read_log_file_full` · `:157 clear_log_files`
**memory_commands.rs (17):** `:32 get_session_timeline` · `:40 recall_facts` · `:46 store_fact_cmd` · `:66 update_fact_cmd` · `:76 deactivate_fact` · `:82 get_user_preferences` · `:88 search_semantic` · `:96 get_memory_md` · `:102 save_memory_md` · `:108 list_memory_topics` · `:114 get_memory_topic` · `:120 save_memory_topic` · `:136 delete_memory_topic` · `:142 search_procedural` · `:150 search_memory` · `:168 get_memory_stats` · `:174 summarize_session_cmd`
**model_config_commands.rs (3):** `:13 get_model_config` · `:19 set_model_config` · `:40 delete_model_config`
**orphan_commands.rs (2):** `:10 get_orphaned_tasks` · `:18 force_release_task`
**plan_commands.rs (9):** `:28 planner_compile` · `:37 get_plan` · `:43 list_plans` · `:53 get_plan_tree` · `:64 delete_plan` · `:78 break_down_epic` · `:104 claim_plan_item` · `:130 create_subplan` · `:162 submit_plan_for_qc`
**plugin_commands.rs (13):** `:17 list_plugins` · `:24 list_installed_plugins` · `:31 get_plugin` · `:40 install_plugin` · `:47 install_plugin_from_yaml` · `:54 uninstall_plugin` · `:63 update_plugin` · `:72 verify_plugin` · `:81 run_plugin` · `:90 get_plugin_config` · `:97 set_plugin_config` · `:107 generate_setup_token` · `:121 validate_setup_token`
**recipe_commands.rs (8):** `:47 list_recipes` · `:53 get_recipe` · `:60 list_recipe_backups` · `:93 create_recipe` · `:108 update_recipe` · `:133 delete_recipe` · `:158 restore_recipe_backup` · `:179 generate_recipe_yaml`
**routing_commands.rs (2):** `:11 suggest_worker_for_task` · `:23 get_worker_affinity_stats`
**session_commands.rs (7):** `:11 list_sessions` · `:18 get_session` · `:26 get_session_messages` · `:34 delete_session` · `:60 create_session` · `:93 update_session_title` · `:111 set_session_pa_mode`
**settings_commands.rs (8):** `:27 list_providers` · `:58 test_provider_connection` · `:86 set_active_provider` · `:98 get_active_provider` · `:109 get_cost_today` · `:117 get_system_prompt` · `:125 set_system_prompt` · `:135 get_default_system_prompt`
**sidecar_commands.rs (18):** listed in B.1
**skill_commands.rs (8):** `:18 list_skills` · `:29 get_skill` · `:39 search_skills` · `:49 skills_by_category` · `:58 validate_skill_parameters` · `:70 execute_skill` · `:93 reload_skills` · `:100 import_skill`
**startup_commands.rs (3):** listed in B.1
**subagent_commands.rs (7):** listed in B.1
**task_commands.rs (11):** `:12 list_tasks` · `:19 get_task` · `:27 update_task_status` · `:111 delete_task` · `:126 get_qc_queue` · `:147 clear_all_tasks` · `:165 list_worker_requests` · `:172 resolve_worker_request` · `:187 get_epic_tasks` · `:195 get_epic_phase_summary` · `:205 create_phased_task`
**updater_commands.rs (8):** `:21 check_for_update` · `:43 get_cached_update` · `:54 download_update` · `:161 get_download_progress` · `:171 install_update` · `:213 skip_version` · `:228 get_update_history` · `:236 verify_data_integrity`
**vault_commands.rs (6):** `:7 store_provider_key` · `:12 get_provider_key` · `:17 delete_provider_key` · `:22 store_github_pat` · `:27 get_github_pat` · `:32 delete_github_pat`
**worker_commands.rs (11):** `:50 list_workers` · `:85 get_worker_by_slug` · `:90 create_worker` · `:115 update_worker_status` · `:139 update_worker_model` · `:150 update_worker_model_sync` · `:201 update_worker_repos` · `:206 bind_worker_recipe` · `:211 delete_worker` · `:218 start_worker_recipe` · `:230 stop_worker_recipe`
**worker_message_commands.rs (3):** `:14 list_worker_messages_for_task` · `:23 list_recent_worker_messages` · `:30 count_unread_worker_messages_for`
**workspace_commands.rs (3):** `:37 get_workspace_dir` · `:52 set_workspace_dir` · `:72 pick_workspace_dir`

</details>

---

# Section C — Sidecar coupling inventory

## C.0 Search methodology

Greps run across `src-tauri/src/**/*.rs` for: `sidecar`, `orchestrator`, `OrchestratorClient`, `orch_client`, `127.0.0.1`, `localhost`, `:4000`, `:8000`, `:8767`, `:6379`, `uv run`, `python`, `reqwest`, plus a targeted grep for every public method on `OrchestratorClient`.

61 files *mention* the sidecar. **13 files actually couple to it.** The remaining 48 are comments, prompt text, docstrings, Python-ecosystem detection, or the Ollama `localhost:11434` URL — all classified DELETE (comment-only) below.

## C.1 The client surface — every capability delegated

`sidecar/client.rs` constants: `BASE_URL = "http://127.0.0.1:4000"` (`:93`), `MEMORY_URL = "http://127.0.0.1:8000"` (`:94`), `API_KEY = "dev-key-123"` (`:95` — **a hardcoded credential in source**), `HEALTH_PATH = "/health"` (`:108`), `INVOKE_TIMEOUT_SECS = 600` (`:99`), `HEALTH_FAIL_THRESHOLD = 6` (`:116`).

| Method | client.rs line | HTTP endpoint | Capability delegated | Class |
|---|---|---|---|---|
| `authenticate` | `:214` | `POST /api/auth/service-login` (`:215`) | JWT bootstrap | **DELETE** |
| `ensure_authenticated` | `:258` | — | token cache | **DELETE** |
| `verify_fleet_ready` | `:297` | `GET /api/agents` (`:302`) | asserts all 7 `FLEET_AGENTS` (`:282`) registered | **DELETE** |
| `fetch_agent_models` | `:347` | `GET /api/agents` (`:352`) | reads per-agent model out of orch YAML | **PORT** (→ read from `workers.db`) |
| `is_healthy_strict` | `:389` | `GET /health` (`:390`) | liveness | **DELETE** |
| `tcp_liveness` | `:415` | raw TCP to :4000 (`:418-421`) | liveness | **DELETE** |
| `is_healthy` | `:428` | `GET /health` (`:429`) | debounced liveness — **the Mode 1/Mode 2 switch** | **DELETE** |
| `is_memory_healthy` | `:532` | `GET :8000/health` (`:533`) | mem0 liveness | **DELETE** |
| `is_redis_healthy` | `:558` | raw RESP `PING` to `127.0.0.1:6379` (`:565`) | Redis/Memurai warm tier | **DELETE** |
| `ensure_agent` | `:609` | list/create/config | idempotent worker-agent registration | **PORT** (→ local worker registry) |
| `update_agent_model` | `:647` | `PUT /api/agents/{id}/config` (`:649`) | push model change into orch YAML | **PORT** |
| `find_or_create_agent` | `:672` | `GET`/`POST /api/agents` (`:682,764`) | agent CRUD | **PORT** |
| **`invoke_worker`** | `:842` | `POST /api/agents/{id}/invoke` (`:860`) | **run a LivingAgent turn** (21 FSMs, mem0 recall, A2A, litellm routing, context scoring) | **PORT — the core capability** |
| **`invoke_worker_streaming`** | `:947` | `POST /api/agents/{id}/invoke/stream` | same, SSE — re-emits frames on `subagent:event` keyed by worker slug (`:1026-1029`) | **PORT** |
| `try_invoke` | `:1075` | retry wrapper for WinError 10054 | resilience | **DELETE** |
| `resolve_model_name` | `:1127` | `GET /api/llm/models` (`:1140`) | short key → canonical LiteLLM model name | **PORT** (→ local model registry) |
| `update_agent_model_by_slug` | `:1191` | `GET /api/agents` (`:1202`) + config PUT | slug-keyed model sync | **PORT** |
| `register_mcp_server` | `:1262` | `POST /api/mcp/servers` (`:1269`), `PUT .../autodev-mcp-server` (`:1331`); body carries `http://127.0.0.1:{mcp_port}/mcp` (`:1274`) | tells orch how to reach our own MCP server | **DELETE** (loop closes when the caller is in-process) |
| `mcp_server_healthy` | `:1388` | TCP to `127.0.0.1:{mcp_port}` (`:1400`) | our own MCP liveness | **ALREADY-LOCAL** (`mcp_server::is_ready()` exists at `mcp_server/server.rs:56`) |
| `create_ephemeral_agent` | `:1435` | `POST /api/agents` (`:1464`) | subagent creation | **PORT** |
| `delete_agent` | `:1500` | `DELETE /api/agents/{id}` (`:1503`) | subagent teardown | **PORT** |
| `invoke_agent_stream_raw` | `:1531` | `POST .../invoke/stream` (`:1557`) | subagent execution stream | **PORT** |
| `list_ephemeral_subagent_slugs` | `:1579` | `GET /api/agents` (`:1581`) | orphan sweep | **PORT** |
| `extract_usage` (free fn) | `:50` | — | token accounting from orch metadata with char/4 fallback (`:87-89`) | **DELETE** (real usage comes from the provider directly) |
| `to_orch_model_key` | `:1623` | — | model-name normalisation for orch | **DELETE** |
| `parse_agents_list` | `:1643` | — | response shape tolerance | **DELETE** |

## C.2 Process-lifecycle coupling — `sidecar/mod.rs`

| Coupling point | Line | What it does | Class |
|---|---|---|---|
| `ORCH_ONLY_ENV_VARS` | `:74-82` | strips `CORS_ORIGINS`, `API_KEYS`, `AIRS_API_KEY`, `DATABASE_URL`, `REDIS_URL`, `MEM0_API_KEY`, `ALLOWED_MIME` from the child env because dotenvy mangles list-typed values | **DELETE** |
| `SidecarManager::start_memory` | `:475-535`; `Command::new("uv")` `:488`; `args(["run","python","main.py","--mode","dev"])` `:489` | spawns memory-mcp-server on :8000 | **DELETE** |
| `SidecarManager::start` | `:578-632`; `Command::new("uv")` `:591`; `args(["run","python","start_orchestrator.py"])` `:596` | spawns core-orchestrator on :4000 | **DELETE** |
| `spawn_log_router` | `:218-341` | pipes Python stdout/stderr into `~/.autodev/logs/*.log` + tracing, with Windows-asyncio noise filtering (`:343-358`) and level classification (`:360`) | **DELETE** |
| `rotate_sidecar_log_if_needed` | `:176` | log rotation (the 2026-04-29 run produced 133 MB, `:125`) | **DELETE** |
| `try_start_memurai_service` | `:760-840` | starts the Windows Memurai/Redis service; falls back to a direct exe spawn (`:809`) | **DELETE** |
| `resolve_orchestrator_path` | `:888-958` | env var → exe-adjacent `sidecars/core-orchestrator` → NSIS `%LOCALAPPDATA%\AutoDev\resources\sidecars\` → legacy MSI path | **DELETE** |
| `resolve_memory_path` | `:968-1033` | same for memory-mcp-server | **DELETE** |
| `start_watchdog` / `orch_watchdog` / `memory_watchdog` | `:1054`, `:1075`, `:1210` | 10s poll, 3 strikes, exponential backoff 2→120 s, restart | **DELETE** |
| `SidecarStatus`, `StderrTail`, PID/status accessors | `:97`, `:41`, `:449-873` | UI-facing status surface | **DELETE** |

**DOCSTRING-LIE D1:** `resolve_orchestrator_path`'s doc comment (`sidecar/mod.rs:881-887`) lists a sixth resolution step — *"6. Relative candidates from the current working directory"*. The body has no such step: after the installed-location checks it goes straight to `error!` + `Err` (`:955-957`). Identical lie in `resolve_memory_path`'s doc (`:961-967` vs body ending `:1031-1033`).

## C.3 Application-level coupling

| File | Lines | What is delegated | Class |
|---|---|---|---|
| `main.rs` | `:10-12` imports; `:126-127` construct; `:133-134` SubagentManager; `:166-170,180-181,275,297,310` wiring; `:283-347` Demo-Mode branch + scripted FSM; `:349-461` two-phase health probe + auto-start + fleet verification | entire startup choreography | **DELETE** (startup) / **PORT** (the FSM's UX contract) |
| `recipes/runner.rs` | `:49,66,75-77` client injection; `:83` readiness; `:98` fleet latch; `:110` **the Mode-1/Mode-2 switch**; `:209` `ensure_agent`; `:215` `mcp_server_healthy`; `:355-362` `invoke_worker_streaming`; `:562-576` `FallbackPolicy::Mode2Only` guard | **worker fleet execution** — the single most consequential coupling | **PORT** |
| `subagent/bridge.rs` | `:32,48,99,130,182,215,221` | entire subagent lifecycle | **PORT** |
| `subagent/manager.rs` | `:31,55,61,214,235,250` | registry + orphan sweep against orch | **PORT** |
| `subagent/{types,pool,slug}.rs` | `types.rs:50,75,81,111,143,152`; `pool.rs:6`; `slug.rs:26` | DTO fields and slug sanitisation shaped by orch's API | **PORT** (reshape) |
| `commands/sidecar_commands.rs` | 18 commands, 146 sidecar mentions | lifecycle UI | **DELETE** (except `list_mcp_tools`, `get_mcp_health`, `force_reconnect_mcp` — those serve the *external* MCP surface) |
| `commands/worker_commands.rs` | `:12,51,58-59,74-76,153,163-164,181-182` | model sync + YAML read/write **into the submodule tree** at `{orch}/config/agents/{slug}.yaml` (`:74`, `:181`) | **PORT** |
| `commands/startup_commands.rs` | `:15,41-49` | Demo Mode flag + startup FSM snapshot | **DELETE**/reshape |
| `runtime_config.rs` | `:7-12,39-44` | `skip_sidecars` field + its doc | **DELETE** |
| `events.rs` | `:87,104,108,298-306` | `sidecar:crash` event + payload | **DELETE** |
| `observability/error_kinds.rs` | `:13-15,26` | taxonomy entries naming the Python sidecar (with a stale file reference) | **DELETE** |
| `mcp_server/*` (1,105 LOC) | `mcp_server/mod.rs:1-9`, `server.rs:1-8,126-144` | **exists solely so orchestrator LivingAgents can call Rust tools over HTTP** | **DELETE** — once agents run in-process, they call `ToolRegistry` directly. ~~`health_monitor.rs` + `port_status.rs` (572 LOC) serve *external* MCP servers and should be **KEPT**.~~ **⚠ RETRACTED 2026-08-31 — this claim is wrong and the main session propagated it into ADR-03-02 and spec `09` before catching it.** Only `health_monitor.rs` (400 LOC) survives. **`port_status.rs` (172 LOC) is own-port diagnostics, not external-MCP infrastructure**: `port_status.rs:17` is `use super::server::{MCP_SERVER_PORT, current_mcp_port}`, so it dies with the server and **keeping it breaks the build** — see `G12` MC-08 for the exact break. Survivor set is **484 LOC, not 572**. |
| `agent/builtin_tools/{spawn_subagent,spawn_parallel_subagents,spawn_qc_review}.rs` | `spawn_subagent.rs:92-96`, `spawn_qc_review.rs:84-88` | all three route through `subagent::manager::global()` → sidecar | **PORT** |

**Comment-only / no-op mentions (DELETE, zero code impact):** `provider/{mod,anthropic,openai,google}.rs` header comments; `session/mod.rs:2`; `analytics/types.rs:75`; `agent/builtin_tools/mod.rs:3`; `memory/summarizer.rs:10`; `recipes/types.rs:24`; `agent/builtin_tools/send_worker_message.rs:8`; `mcp_server/tool_adapter.rs:215`; `mcp_server/health_monitor.rs:4`; `observability/stray_files.rs:16`; `updater/{check,download}.rs` ("sidecar file" = `.sig`/`.sha256`, unrelated); `hooks/ecosystem.rs`, `plugin/builtins.rs`, `skills/defaults.rs`, `recipes/defaults.rs`, `graph/extract.rs`, `ide/{file_reader,symbols}.rs`, `db/workers.rs:185` (all reference Python as a *target ecosystem*, not the sidecar); `provider/ollama.rs` `localhost:11434` (Ollama, not sidecar); `agent/builtin_tools/e2e_probe.rs` `localhost:3000` (probe target, not sidecar).

**DOCSTRING-LIE D2:** `memory/summarizer.rs:10` — *"(Full LLM summarization forwarded to core-orchestrator when online — Phase 4.8)"*. `Summarizer::summarize_session` (`:38-43`) calls only `condense()`, a rule-based function (`:63+`). No LLM call, no orchestrator call, ever. Nothing to delete — but the comment misrepresents current behaviour.

**DOCSTRING-LIE D3:** `agent/builtin_tools/mod.rs:3` — *"Worker mode: tools are forwarded to core-orchestrator with `__autodev` suffix."* No `__autodev` suffix exists anywhere in the crate (`grep -rn "__autodev"` → 0 hits). Worker-mode tools are served by the in-process MCP server under their plain names.

## C.4 Capabilities that MUST be ported — the definitive list

**9 capabilities.** Everything else on the sidecar boundary is either lifecycle plumbing (DELETE) or already has a Rust equivalent.

| # | Capability | Currently provided by | Rust status today | Where the port lands |
|---|---|---|---|---|
| **P1** | **Fleet-worker agent turn execution** (the "LivingAgent" invoke) — 21 FSMs, per-agent persona, A2A messaging, context scoring | `client.invoke_worker` / `invoke_worker_streaming` (`client.rs:842,947`) | Mode 1 `AgentEngine` exists and is complete for the plain loop (`recipes/runner.rs:646-670`), but has **none** of the FSM / A2A / context-scoring layer | `agent/engine.rs` + a new fleet-agent layer |
| **P2** | **Ephemeral subagent lifecycle** (create → stream → delete) | `subagent/bridge.rs:99,221,130` | **Zero** local implementation; `SubagentManager` is a thin wrapper over the HTTP client | new in-process subagent runner reusing `AgentEngine` |
| **P3** | **Long-term semantic memory (mem0)** — vector recall across sessions | memory-mcp-server on :8000 (`client.rs:94,533`) | `memory/semantic.rs:166-179` is a **SQL `LIKE '%q%'` substring match**. **No embeddings anywhere in the crate** (`grep -rni "embedding\|vector\|cosine\|faiss\|hnsw"` → 0 hits) | `memory/` needs a real vector index |
| **P4** | **Multi-provider LLM routing (litellm)** — 10 provider families | orchestrator's litellm; the family map lives at `commands/worker_commands.rs:308-345` (anthropic, openai, google, **xai**, deepseek, moonshot, sealion, groq, dashscope, nvidia) | Rust implements **4**: anthropic, openai, google, ollama (`provider/mod.rs:8-11`). **No xAI/Grok provider module exists.** | `provider/` — 6+ new providers |
| **P5** | **Canonical model registry** (short key → API model name, with pricing) | `GET /api/llm/models` via `resolve_model_name` (`client.rs:1127-1160`) | hardcoded `models_static()` per provider + a 7-arm price table (`cost/tracker.rs:50`) with a known bug (F8) | new `provider/registry.rs` |
| **P6** | **Worker-agent registry / persona config** | orch `/api/agents` + `{orch}/config/agents/{slug}.yaml`, read/written from Rust at `commands/worker_commands.rs:74,181` | `workers.db` exists (`db/workers.rs`) and already seeds 7 fleet workers (`:185`), but persona/system-prompt lives in the YAML | fold persona into `workers.db` + `recipes/` |
| **P7** | **Fleet readiness / startup choreography** | `verify_fleet_ready` (`client.rs:297`) + the 12-phase `StartupPhase` FSM (`sidecar/startup_progress.rs`) | FSM code exists and is provider-agnostic; only the *sources* of the transitions are sidecar-bound | keep the FSM, re-source its events |
| **P8** | **Worker cost/token accounting for fleet turns** | `extract_usage` heuristics over orch metadata (`client.rs:50-91`) | Mode 1 already gets exact usage from provider `MessageDelta` (`engine.rs:455-458`) | falls out of P1 automatically — **this is a net accuracy gain** |
| **P9** | **Per-tool live streaming for the Live Monitor** | SSE frames parsed by `subagent::bridge::parse_event_frame`, re-emitted on `subagent:event` (`client.rs:1011-1029`) | `AgentEngine::stream_response` already emits `emit_token`/`emit_thinking` (`engine.rs:429-439`) but **not** per-tool start/result events on the `subagent:event` channel | emit `SubagentEvent::{ToolStart,ToolResult}` from `tool_executor.rs` |

**Not ported (DELETE, 14 capability-shaped items):** JWT service-login; the `dev-key-123` API key; health/liveness probes for :4000 and :8000; the Redis/Memurai warm tier (`client.rs:558`, `sidecar/mod.rs:760`) — no Rust code reads Redis and no `redis` crate is in `Cargo.toml`; Python process spawn/kill/watchdog/log-routing; env-var stripping; `register_mcp_server`; WinError-10054 retry; Demo Mode (`skip_sidecars`); `sidecar:crash` events; the sidecar half of `error_kinds`; the `dump.rdb` Redis snapshot (`src-tauri/dump.rdb`, 171 KB — a committed artefact of a Redis instance).

**ALREADY-LOCAL (redundant sidecar paths, 2):** `mcp_server_healthy` (`client.rs:1388`) duplicates the in-process `mcp_server::is_ready()` (`mcp_server/server.rs:56`) via a TCP round-trip to ourselves; `extract_usage`'s char/4 estimator (`client.rs:87-89`) duplicates — worse — the exact provider usage the Rust engine already receives.

---

# Section D — What already exists in-house

Verdict scale: **Complete** · **Partial** · **Stubbed** · **Shell-only**.

## D.1 `agent/` — **Complete** (for the single-agent loop)

Evidence for "complete", not aspirational:
- Real streaming loop with tool-call assembly from partial JSON deltas: `engine.rs:426-473`.
- Partitioned concurrency actually implemented: `Semaphore::new(MAX_CONCURRENT_READ_TOOLS)` at `tool_executor.rs:231`, `tokio::spawn` per read tool at `:243`.
- 40 real tools with a workspace path-jail (`builtin_tools/path_jail.rs`) and a process-wide per-file async lock (`builtin_tools/path_lock.rs`).
- Hooks fire at 6 real lifecycle points from within the loop.
- Compaction has both an LLM path and a truncation fallback (`compaction.rs:45,78`).

Real gaps:
- `MAX_TOOL_TURNS = 20` hardcoded (`engine.rs:34`); `max_tokens: 8192` hardcoded (`engine.rs:198`); `model_context = 200_000u32` hardcoded (`engine.rs:174`) — wrong for every non-Claude model.
- `temperature`, `top_p`, `reasoning_effort`, `thinking_budget_tokens` are all hardcoded `None` at the only production call site (`engine.rs:199-202`) — see FINDING F4.
- Provider inference for analytics is a string-`contains` ladder that yields `"unknown"` for ollama and everything else (`engine.rs:364-372`).
- `agent/context.rs:9,31-33,39,52` carry four TODOs saying the SQLite migration is pending — but `build_system_prompt_with_mode` **already does** the SQLite read (`:86-113`). The TODOs are stale, and the legacy `build_system_prompt` they describe (`:34-66`) is **dead code** — its only reference in the crate is its own definition (`grep -rn "build_system_prompt"` → definition + `build_system_prompt_with_mode` + one call to the `_with_mode` variant at `engine.rs:101`).

## D.2 `provider/` — **Partial**

| Question | Answer | Evidence |
|---|---|---|
| Is there **any** xAI/Grok provider? | **No.** | `provider/mod.rs:8-11` declares only anthropic, openai, google, ollama. The only `grok`/`xai` strings in the whole crate are `commands/worker_commands.rs:327-328`, which maps a model prefix to a provider **name string written into the orchestrator's YAML** — no Rust provider backs it. |
| Is the trait streaming-capable? | **Yes.** | `Provider::stream(...) -> Result<StreamResult>` where `StreamResult = Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>` (`provider/traits.rs:10,27`). |
| Tool / function calling? | **Yes for 3 of 4.** Anthropic (`anthropic.rs:67-144`), OpenAI (`openai.rs:74-190`), Google (`google.rs:105-127` builds `functionDeclarations`). **Ollama: NO.** | `provider/ollama.rs:230-241`: `build_chat_request` explicitly *does not* attach tools — *"Raw chat `.tools()` builder is not yet exposed in ollama-rs 0.3.4 public API (only available through Coordinator). Deferred."* `to_ollama_messages` (`:242-306`) flattens tool calls into **prose inside message text**. An agent run on Ollama is effectively tool-less. |
| Structured output / JSON schema? | **No.** | `CompletionRequest` (`provider/types.rs:49-59`) has no `response_format`, `json_schema`, or `tool_choice` field. `grep -rni "response_format\|json_schema"` → 0 hits. |
| Extended thinking? | Partial. | `thinking_budget_tokens` exists on the request (`types.rs:58`) and Google wires it to `thinkingConfig` (`google.rs:135-137`); Anthropic wires it in `build_body`. But nothing ever sets it (see F4). |
| Vision / images? | **No.** `MessageContent` has only `Text`/`ToolUse`/`ToolResult` (`types.rs:29-33`) despite `ModelInfo::supports_vision` existing (`types.rs:10`). |

Verdict: the **trait** is well-shaped and the three cloud providers are genuinely complete for text + tools. Ollama is **Partial (tool-less)**. The **provider roster is the single biggest functional regression** the sidecar deletion introduces.

## D.3 `memory/` — **Partial, with a misnamed tier**

- **Episodic (Tier 1) — Complete.** Real schema (`memory/db.rs:35-58`), real writes from the PA path (`commands/agent_commands.rs:77,183`), real reads (`memory_commands.rs:33`).
- **Semantic (Tier 2) — Partial/misnamed.** `SemanticStore::search` is `WHERE LOWER(fact_text) LIKE '%q%'` (`memory/semantic.rs:166-179`). There is no embedding model, no vector column, no ANN index anywhere in the crate. The `search_semantic` command (`memory_commands.rs:88`) therefore performs substring matching, not semantic retrieval. **This is the capability mem0 was providing.**
- **Procedural (Tier 3) — Complete.** File-backed MEMORY.md + topics, mirrored to SQLite (`memory/procedural.rs`).
- **Recall — Complete but crude.** `build_memory_context` (`recall.rs:31-72`) with fixed limits 5/10/3 (`:23-25`) and a naïve word-overlap scorer (`recall.rs:162-176`). It is genuinely injected into the system prompt (`agent/context.rs:117-121`) — reader verified.
- **Summarizer — Partial.** Rule-based only; see DOCSTRING-LIE D2.
- **Coverage gap:** memory writes happen **only** on the PA path (`agent_commands.rs`). `recipes/runner.rs` contains no `EpisodicStore` call — **fleet worker turns write no episodic memory at all** in either mode. Verified by grep.

## D.4 `subagent/` — **Shell-only (locally)**

The module is complete *as an HTTP client of core-orchestrator* and has **zero** local execution capability. `SubagentManager::new(client: Arc<OrchestratorClient>)` (`manager.rs:61`); the bridge is the create/invoke/delete lifecycle (`bridge.rs:99,221,130`). Delete the sidecar today and `spawn_subagent` returns an error at `bridge.rs:110-116`.
What *is* reusable and worth keeping: `SubagentPool` concurrency caps (`pool.rs:93-168`), the slug generator (`slug.rs`), the event DTOs (`types.rs`), the orphan sweep logic (`manager.rs:234-256`), and `parse_event_frame`/`find_frame_end` (which will become dead once the stream is in-process).

## D.5 `routing/` — **Complete**

Static keyword patterns + historical affinity scoring, with a closed feedback loop: `record_assignment` on assign (`assign_task.rs:114`, `assign_tasks_batch.rs:190`), `record_outcome_auto` on status change (`task_commands.rs:53,56,60`), `suggest_worker` reading `routing_history` back (`routing/mod.rs:45-51`). Writer and reader both verified. Sidecar-independent.

## D.6 `budget/` — **Complete types, INERT enforcement**

The types, validation, ceilings, and `check()` are all real (`budget/mod.rs:75-113`). `AgentEngine` has a full enforcement block: pre-emptive check (`engine.rs:150-166`), usage accumulation (`engine.rs:216-230`), and escalation into a `WorkerRequest` + `needs_info` transition (`engine.rs:480-536`).
**But it never fires.** `self.task_budget` is only ever set by `AgentEngine::with_task_budget` (`engine.rs:73`), and **nothing in the crate calls it** — the only other occurrence of the string is the comment at `engine.rs:45`. `recipes/runner.rs:653-661` constructs the engine without it. Verdict: **Partial — dead in production.** See FINDING F3.

## D.7 `cost/` — **Complete but wrong**

`CostTracker::calculate` is called on both the Mode 1 (`engine.rs:355`) and Mode 2 (`runner.rs:382`) paths. The pricing table (`cost/tracker.rs:50-59`) is hardcoded, duplicates the `cost_in_per_1m`/`cost_out_per_1m` already present on every `ModelInfo` (`provider/types.rs:12-13`), and contains the unreachable-arm bug F8. Default fallback is `(1.0, 3.0)` for anything unmatched — which includes every Claude 4.5 alias not containing the literal substrings, every Gemini 2.x, and all of Ollama.

## D.8 `session/` — **Complete**

Straightforward SQLite CRUD over `~/.airs/autodev.db`. 11 methods, all exercised by the 7 session commands. Sidecar-independent.

## D.9 `graph/` — **Complete core, dead façade**

The real pipeline is complete and substantial: tree-sitter extraction across multiple languages (`graph/extract.rs`, 1,630 LOC), SQLite store with FTS (`store.rs`, 738), community detection (`cluster.rs`, 464), incremental rebuild (`incremental.rs`), graph diffing (`analyze.rs:495-502`), god-node and surprising-connection analytics — all reachable from the 19 graph commands.
Overlaid on it are the **7 dead façade structs** of FINDING F6 and the never-instantiated `GraphWatcher`. Verdict: **Complete (free functions) + Shell-only (exported struct API).**

## D.10 `mcp_server/` — **Complete, but its reason for existing is the sidecar**

`server.rs` is a working rmcp Streamable-HTTP server with SO_REUSEADDR, port fallback, slow-retry recovery, and port-conflict diagnostics. `tool_adapter.rs` is a working `Tool`→MCP bridge. All of it exists so that **Python LivingAgents can call Rust tools** (`mcp_server/mod.rs:1-9`). Post-restructure, 533 LOC (`server.rs` + `tool_adapter.rs`) become unnecessary; 572 LOC (`health_monitor.rs` + `port_status.rs`, which serve *external* MCP servers like GitHub MCP) remain useful.

## D.11 Others (brief)

| Module | Verdict | Evidence |
|---|---|---|
| `analytics/` | **Complete** | 5 real tables, real writes from both modes, `global()` panics rather than silently no-op'ing (`analytics/mod.rs:127-136`). Exception: `aggregator.rs:21` "placeholder for future implementation" — **UNVERIFIED** which rollup arm that guards; the hourly/daily jobs do run (`main.rs:151`). |
| `git/` | **Complete** except merge-conflict resolution (`git/merge.rs:4`) |
| `hooks/` | **Complete** — 14 real fire sites |
| `ide/` | **Complete**; symbol extraction is regex-based (`ide/symbols.rs:8`) and duplicates the tree-sitter work in `graph/extract.rs` |
| `plugin/` | **Complete** |
| `skills/` | **Complete** incl. hot reload |
| `updater/` | **Partial** — signature verification is placeholder-keyed (F9), bsdiff decompression unimplemented (`patch.rs:135`) |
| `orphan/` | **Complete** |
| `diff_analysis/` | **Complete** |
| `vault/`, `state/`, `errors/`, `logging/`, `observability/` | **Complete** |
| `plan/` | **Shell** — 4-line re-export of `db::plans` |

---

# Section E — Data layer

## E.1 Databases

| # | File | Owner module | Tables | Migration mechanism |
|---|---|---|---|---|
| 1 | `~/.airs/autodev.db` | `session/storage.rs:293` | `sessions` (`:32`), `messages` (`:65`) | `CREATE TABLE IF NOT EXISTS` on open |
| 2 | `~/.autodev/tasks.db` | `db/tasks.rs:287` | `tasks` (`:300`), `worker_requests` (`:327`), `worker_messages` (`:345`), `routing_history` (`:417`) | idem |
| 3 | `~/.autodev/config.db` | shared by 6 modules | `autodev_instructions` (`db/autodev_instructions.rs:46`), `model_configs` (`db/model_configs.rs:54`), `hooks` + `hook_logs` (`hooks/db.rs:30,42`), `app_settings` (created independently at `commands/workspace_commands.rs:23`, `commands/github_mcp_commands.rs:30`, `ide/workspace.rs:35`), `checkpoints` (`git/checkpoint.rs:33`) | idem |
| 4 | `~/.autodev/workers.db` | `db/workers.rs:111` | `workers` (`:123`) + seed of 7 fleet workers (`:185`) | idem |
| 5 | `~/.autodev/plans.db` | `db/plans.rs:205` | `plans` (`:218`), `plan_items` (`:240`), `plan_item_dependencies` (`:264`), `plan_executions` (`:272`) | idem |
| 6 | `~/.autodev/memory.db` | `memory/db.rs:122` | `episodic_memories` (`:35`), `semantic_memories` (`:63`), `fact_history` (`:83`), `procedural_memories` (`:96`) | additive `CREATE TABLE IF NOT EXISTS`, WAL + FK on (`:29-30`) |
| 7 | `~/.autodev/analytics.db` | `analytics/db.rs:23` | `token_usage` (`:47`), `daily_costs` (`:68`), `worker_stats` (`:80`), `api_failures` (`:94`), `system_events` (`:105`) | idem |
| 8 | `~/.autodev/annotations.db` | `db/annotations.rs:51` | `annotations` (`:61`) | idem |
| 9 | `~/.autodev/checkpoints.db` | `db/checkpoints.rs:67` | `file_checkpoints` (`:80`) | idem |
| 10 | `~/.autodev/graphs/<12-char-sha>.db` (per repo) | `commands/graph_commands.rs:74-80`, `graph/store.rs:66` | `nodes` (`graph/schema.rs:40`), `edges` (`:67`), `communities` (`:88`), `flows` (`:99`), `snapshots` (`:111`), `analysis_cache` (`:122`), `repos` (`:131`), plus `nodes_fts` | **Versioned** — `PRAGMA user_version`, 5 migrations, **head = v5** (`graph/migrations.rs:12-45`) |

Also: `<repo>/.autodev/graph.db` written by `graph/registry.rs:62,123` (a *second*, repo-local graph path distinct from #10's `~/.autodev/graphs/` — **inconsistency worth flagging**), and a JSON registry file at `graph/registry.rs:48,82`.

**Migration head:** only `graph/` has a real migration ladder (v5). Every other database uses idempotent `CREATE TABLE IF NOT EXISTS` executed on each open. There is no schema-version table, no down-migration, and **no backup path anywhere in the crate** (`updater/persistence.rs:34-35,90-91` creates a restore point for `config.db` + `analytics.db` only, and only during an update).

## E.2 Redis

`src-tauri/dump.rdb` (171 KB, dated 2026-05-12) is a **committed Redis snapshot**. Redis is used by the Python sidecars only. The Rust side merely (a) probes `127.0.0.1:6379` with a raw RESP `PING` (`sidecar/client.rs:558-590`) and (b) tries to start the Memurai Windows service (`sidecar/mod.rs:760-840`). **No `redis` crate is in `Cargo.toml`; no Rust code reads or writes Redis data.** Both Redis touchpoints are **DELETE**; `dump.rdb` is an orphan artefact.

## E.3 Table ownership map

`tasks`/`worker_requests`/`worker_messages`/`routing_history` → `db/tasks.rs` (+ `routing/` reads `routing_history`, `orphan/` reads/writes `tasks`) · `workers` → `db/workers.rs` (+ `analytics/queries.rs:83` opens it read-only) · `plans*` → `db/plans.rs` (via `plan/`) · `episodic/semantic/procedural/fact_history` → `memory/*` · `token_usage`/`daily_costs`/`worker_stats`/`api_failures`/`system_events` → `analytics/*` · `hooks`/`hook_logs` → `hooks/db.rs` · `model_configs` → `db/model_configs.rs` · `autodev_instructions` → `db/autodev_instructions.rs` (read by `agent/context.rs:86,101`) · `app_settings` → **three independent owners** (`workspace_commands`, `github_mcp_commands`, `ide/workspace`) · `annotations` → `db/annotations.rs` · `file_checkpoints`/`checkpoints` → `db/checkpoints.rs` + `git/checkpoint.rs` (two tables, similar names, different files — **UNVERIFIED** whether this is intentional) · graph tables → `graph/store.rs`.

---

# Section F — Build & configuration

## F.1 Workspace

`autodev/Cargo.toml` — a 3-member workspace: `src-tauri`, `src-cli`, `src-shared`, resolver 2.
`.cargo/config.toml` — Linux `x86_64-unknown-linux-gnu` uses `-fuse-ld=lld` because GitHub runners OOM with GNU ld on large crates.
`src-tauri/build.rs` — 3 lines, `tauri_build::build()` only. No codegen, no sidecar step.

## F.2 `src-tauri/Cargo.toml` — dependencies of note

- **Tauri 2.11** (`devtools`) + `tauri-plugin-shell` — **only one plugin**. `tauri.conf.json:114-115` declares `"dialog": {}` and `"fs": {}` under `plugins`, but neither `tauri-plugin-dialog` nor `tauri-plugin-fs` is in `Cargo.toml` and neither is `.plugin()`-registered in `main.rs:176`. **Dead config keys.**
- `rusqlite 0.31` **bundled** — SQLite is statically linked; no system dependency. Good for cross-platform.
- `reqwest 0.12` (`json`, `stream`) — the only HTTP client; used for both providers and the sidecar.
- `rmcp 0.16` (`server`, `transport-streamable-http-server`) + `axum 0.8` + `socket2 0.5` — the MCP server stack. **Removable if `mcp_server/server.rs` goes.**
- `ollama-rs 0.3.4` — pinned; the version whose missing `.tools()` API causes the Ollama tool-calling gap (`provider/ollama.rs:236-237`).
- `git2 0.19`, `tree-sitter 0.22`, `petgraph 0.6`, `rayon`, `ignore`, `notify 6` — graph + git + watchers.
- `keyring 2` — OS keychain; the Linux path needs `libsecret`/KDE Wallet at runtime (`vault/mod.rs:5`).
- `rfd 0.14` — deliberately **without** the `tokio` feature; the comment at `Cargo.toml:83-88` documents that enabling it triggers an `ashpd` `compile_error!()`. **Do not "fix" this.**
- `ed25519-dalek 2`, `semver`, `tempfile` — updater.
- `dotenvy 0.15` — exists **solely** to load `AUTODEV_SIDECAR_PATH` / `AUTODEV_MEMORY_PATH` (`Cargo.toml:120-121`, `main.rs:22-27`). **Removable with the sidecar** unless another use is introduced.
- `sysinfo 0.33` — used only by `get_resource_usage` to measure the Python PIDs (`commands/sidecar_commands.rs:420`). **Removable with the sidecar.**
- `rhai 1` (`sync`) — hook `custom_condition` evaluator.
- `flate2`, `base64`, `sha2`, `hex` — updater + graph hashing. **`bzip2` is absent** despite `updater/patch.rs:135` requiring it.
- Release profile: `panic="abort"`, `lto=true`, `codegen-units=1`, `opt-level="s"`, `strip=true` (`Cargo.toml:127-132`). Note `panic="abort"` — the panic hook at `main.rs:67-71` logs, then the process dies.

## F.3 `tauri.conf.json` — bundling

- `productName: "AutoDev"`, `version: "1.0.0"`, identifier `com.airs-foundry.autodev`.
- **`bundle.targets: ["nsis"]`** (`:33`) — **Windows-only bundling.** The `linux` (deb/appimage) and `macOS` (dmg/entitlements/hardenedRuntime) sections at `:48-57` and `:66-90` are fully configured but **never produced**, because `targets` is not `"all"`.
- **`bundle.resources: ["resources/sidecars"]`** (`:41-43`) — this bundles **224 MB of `core-orchestrator` + 835 KB of `memory-mcp-server`** (measured: `du -sh src-tauri/resources/sidecars/*`) into every installer. Deleting the sidecar removes ~225 MB from the installer. This is the single largest build-level win of the restructure.
- CSP (`:28`) allows `connect-src` to `ipc:`, `http://ipc.localhost`, `https://api.github.com`, `https://releases.airs.dev` — note it does **not** whitelist `http://127.0.0.1:4000`; the sidecar is reached from Rust, not the webview, so this is consistent.
- Window is `decorations: false` with `additionalBrowserArgs: "--enable-gpu --use-angle=d3d11 --enable-features=Vulkan"` (`:24`) — **d3d11/ANGLE is Windows-specific**; harmless elsewhere but another Windows lean.
- `fileAssociations` for `.autodev` (`:58-65`).

## F.4 `capabilities/default.json`

Single capability file. Permissions: `core:default`, `core:event:default`, four window permissions, `shell:allow-open`. **No `fs:` or `dialog:` permissions** — consistent with those plugins not being installed, and confirming the `plugins` block in `tauri.conf.json` is vestigial.

## F.5 Cross-platform build blockers

| Blocker | Evidence | Severity |
|---|---|---|
| `bundle.targets: ["nsis"]` — only Windows installers are produced | `tauri.conf.json:33` | **High** — a one-line fix, but nothing else has been validated |
| 225 MB Python tree in `resources` | `tauri.conf.json:41-43` + `du` | **High** (goes away with the restructure) |
| Sidecar spawn uses `uv` from `PATH` with no bundled interpreter | `sidecar/mod.rs:488,591` | **High** — a fresh install on any OS fails unless the user has `uv` + a matching Python |
| Installed-path resolution is `#[cfg(target_os = "windows")]`-only | `sidecar/mod.rs:924-955`, `:996-1030` | **High** — on macOS/Linux, resolution falls through to the exe-adjacent path or fails |
| Memurai/Redis autostart is Windows-service-specific | `sidecar/mod.rs:760-840` | Medium (goes away) |
| `port_status.rs` shells out to `netstat` with Windows-format parsing | `mcp_server/port_status.rs:57-80` (`Expect: TCP 127.0.0.1:8767 ... LISTENING 27644`) | Medium |
| Home-dir resolution is `USERPROFILE` first, `HOME` second, everywhere | `db/mod.rs:22`, `memory/db.rs:116`, `runtime_config.rs:78`, `main.rs:83` | Low (works, but `dirs 5` is already a dependency and unused for this) |
| `hostname` via `COMPUTERNAME`/`HOSTNAME` env vars | `orphan/mod.rs:214-217` | Low — `HOSTNAME` is often unset on macOS; orphan detection silently degrades to `"unknown"` |
| `keyring 2` on Linux needs `libsecret`/KDE Wallet at runtime; not in the deb `depends` list | `Cargo.toml:44`, `tauri.conf.json:50` | Medium |
| `--use-angle=d3d11` in browser args | `tauri.conf.json:24` | Low |
| macOS `signingIdentity: null`, Windows `certificateThumbprint: null` | `tauri.conf.json:69,92` | Medium — unsigned builds; Gatekeeper/SmartScreen will block |

---

# Section G — Findings ledger

Ordered by consequence for the restructure.

**F1 — The subagent subsystem has no local implementation.** `subagent/` (1,351 LOC) + 3 builtin tools + 7 commands are a pure HTTP client of core-orchestrator (`manager.rs:61`, `bridge.rs:99,221,130`). Deleting the sidecar deletes subagents outright until P2 is built. *Class: PORT, blocking.*

**F2 — The Rust provider roster is 4; the sidecar's was 10.** `provider/mod.rs:8-11` vs `commands/worker_commands.rs:308-345`. **No xAI/Grok provider exists in Rust.** Also lost: deepseek, moonshot, sealion, groq, dashscope, nvidia. *Class: PORT, blocking.*

**F3 — The Budget Circuit Breaker is inert.** `AgentEngine::with_task_budget` (`engine.rs:73`) is never called; `self.task_budget` is always `None`, so the check at `engine.rs:150-166` never fires. Compounding this, `RuntimeConfig::default_worker_budget_{tokens,usd,turns}` and `budget_warn_pct` are written by `set_default_worker_budget` (`commands/budget_commands.rs:73-76`) and **read by nothing** — despite two docstrings claiming *"so the Rust recipe runner can read them when assigning tasks"* (`budget_commands.rs:55-57`, `runtime_config.rs:46-48`). **DOCSTRING-LIE.** A shipped safety feature that has never once halted a run. *Class: bug, pre-existing.*

**F4 — Per-model config is write-only.** `model_configs` (temperature, top_p, reasoning_effort, thinking) is persisted by `set_model_config` (`commands/model_config_commands.rs:19-35`) and read back only by `get_model_config` for display. The single production `CompletionRequest` hardcodes all four to `None` (`agent/engine.rs:199-202`). The user-facing model-tuning UI has no effect on any agent run. *Class: bug, pre-existing.*

**F5 — "Semantic" memory is a SQL LIKE.** `memory/semantic.rs:166-179`. No embeddings exist anywhere in the crate. The real semantic tier was mem0 on :8000. *Class: PORT (P3), blocking for parity.*

**F6 — Seven exported graph structs are silent no-ops.** `GraphBuilder::build_graph` (`build.rs:21-24`), `Clusterer::cluster_graph` (`cluster.rs:460-463`), `Analyzer::analyze_graph` (`analyze.rs:507-510`), `QueryEngine::query_graph` (`query.rs:256-259`), `GraphRegistry::manage_repos` (`registry.rs:202-205`), `Exporter::export_graph` (`export.rs:187-190`), `FlowAnalyzer::analyze_flows` (`flows.rs:191-194`) — each ignores its arguments and returns `Ok(())`. All are re-exported from `graph/mod.rs:27-31`. Zero callers today; a future caller gets green-and-empty. Plus `GraphWatcher` (149 LOC), exported and never instantiated. *Class: dead code / inert-but-green trap.*

**F7 — The MCP tool browser lies about the served tool set.** `main.rs:108-110` builds a registry with `register_all` (= FastDevelopment) and manages it as Tauri state; `list_mcp_tools` (`commands/sidecar_commands.rs:312-326`) reads *that*. But `start_mcp_server` discards it (`mcp_server/server.rs:149`) and serves `register_for_mcp_worker` instead (`:151`). The UI shows a different list than the wire. *Class: bug, pre-existing.*

**F8 — `gpt-4o-mini` is billed at 16× its input price.** `cost/tracker.rs:53-54`: `m.contains("gpt-4o")` precedes `m.contains("gpt-4o-mini")`, making the latter arm unreachable. Every mini call is priced $2.50/$10.00 instead of $0.15/$0.60. Also: the whole table duplicates `ModelInfo::cost_*_per_1m` (`provider/types.rs:12-13`), and unmatched models silently fall back to `(1.0, 3.0)`. *Class: bug, pre-existing.*

**F9 — The updater ships a placeholder signing key.** `updater/verify.rs:13-14` embeds base64 decoding to `undefined-public-key-for-development-only-placeholder` (51 bytes, not 32). Debug builds skip verification entirely (`:39-41`, `:54-56`); release builds bail. Combined with `patch.rs:135` (bzip2 dependency never added), the auto-update path is not shippable. *Class: security / incomplete, pre-existing.*

**F10 — 225 MB of Python is bundled into every installer, and only NSIS is built.** `tauri.conf.json:41-43` bundles `resources/sidecars` (measured 224 MB + 835 KB); `tauri.conf.json:33` restricts `bundle.targets` to `["nsis"]` while fully-populated macOS and Linux config sits unused at `:48-90`. *Class: build.*

**Additional DOCSTRING-LIEs recorded:**
- **D1** — `resolve_orchestrator_path` / `resolve_memory_path` doc comments list a step 6 ("relative candidates from CWD") the bodies do not implement (`sidecar/mod.rs:881-887` vs `:955-957`; `:961-967` vs `:1031-1033`).
- **D2** — `memory/summarizer.rs:10` claims LLM summarization is forwarded to core-orchestrator; the code is purely rule-based (`:38-43`, `:63+`).
- **D3** — `agent/builtin_tools/mod.rs:3` claims worker tools are "forwarded to core-orchestrator with `__autodev` suffix"; `grep -rn "__autodev"` returns 0 hits.
- **D4** — `agent/context.rs:31-33,39,52` carry four TODOs saying the SQLite migration is pending; `build_system_prompt_with_mode` already reads SQLite (`:86-113`). The function the TODOs describe, `build_system_prompt` (`:34-66`), is dead — no callers.
- **D5** — `observability/error_kinds.rs:15` cites `src-tauri/src/sidecar/watchdog.rs::is_healthy`; that file does not exist (watchdog is `sidecar/mod.rs:1054-1290`, `is_healthy` is `sidecar/client.rs:428`).

**Other dead code:** `agent/builtin_tools/graph_tools.rs` (840 LOC) and `graph_prompts.rs` (78 LOC) are **not declared** in `agent/builtin_tools/mod.rs:6-72` — 918 LOC never compiled. `agent/context.rs:34-66` `build_system_prompt` — no callers. `runtime_config::invalidate` — `#[allow(dead_code)]` (`:108`), no callers. `tauri.conf.json:114-115` `dialog`/`fs` plugin blocks — plugins not installed.

**Hardcoded credential:** `sidecar/client.rs:95` — `const API_KEY: &str = "dev-key-123";` in tracked source. Goes away with the sidecar, but should be noted in any pre-deletion audit.

---

## Appendix — Confidence notes

- All LOC figures are `wc -l` on the actual files.
- The 256 command count was derived by set-differencing every `#[tauri::command]`/`#[command]` function name against the names inside `tauri::generate_handler!` (`lib.rs:167-470`) and reconciling the four `#[command]` outliers and the one comment-suffixed registration (`get_qc_reviewer_stats`, `lib.rs:370`).
- **UNVERIFIED:** whether `analytics/aggregator.rs:21`'s "placeholder" comment guards a rollup arm that is actually reached; whether `updater/patch.rs`'s delta path is reachable at runtime; whether the co-existence of `db/checkpoints.rs::file_checkpoints` and `git/checkpoint.rs::checkpoints` is intentional; whether the two distinct graph-DB locations (`~/.autodev/graphs/<hash>.db` vs `<repo>/.autodev/graph.db`) are both live.
- No file outside this report was created, modified, or deleted. No build, test, or server command was executed.
