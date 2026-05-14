# End-to-end run audit — 14 May 2026

User ran AutoDev end-to-end with the task *"build a simple Google-Calendar-style app"*. Run produced calendar code in `C:\AIRS Foundry\autodev\testing\` and surfaced multiple architecture-level issues. This folder is the deep-audit set; **108 atomic implementation tasks across 9 docs.**

---

## TL;DR — what worked, what didn't

### Working as designed ✅

- **Mode 2 dispatch held the entire run.** Only one Mode 1 fallback, fully justified (orch crash). See [05](05-mode1-fallback-check.md).
- **TCP-alive fallback fired and correctly kept Mode 2** during a busy-event-loop window. The protection we shipped last round is proven.
- **Batch `assign_tasks` shipped and in use.** Supervisor dispatch is meaningfully faster.
- **No tool execution failures, no LivingAgent timeouts, no budget breaches.** Run ended on orch crash, not Rust-side bug. See [08](08-log-error-scan.md).
- **Calendar backend is solid in isolation** — parameterized SQL, clean REST surface.

### Broken — full fix-list ordered by blast radius

| # | Concern | Doc | Severity | Real root cause (corrected) |
|---|---|---|---|---|
| 7 | Worker wrote files into `src-tauri/` instead of workspace, dev server restarted | [07](07-workspace-escape-rootcause.md) | **CRITICAL** | No path-jail in `write_file`/`edit_file`/`append_file`/`bash` + Python MCPBridge never forwards `__working_dir` |
| 2 | Dashboard always shows zero, even on manual Refresh | [02](02-dashboard-not-live.md) | **CRITICAL** | `analytics::install_global` is NEVER called from `main.rs`. Every read/write goes to a throw-away in-memory DB with no schema. `let _ =` swallows the errors. |
| 4 | Tasks pile up in QC review | [04](04-qc-backlog.md) | HIGH | Single shared qc-agent on sonnet takes ~5 min per tick; subagent infra ALREADY supports per-task fan-out — just unused |
| 1 | Live Monitor stays STALE/IDLE even when workers are working | [01](01-live-monitor-stale.md) | HIGH | `handleStatusChange` drops events for slugs not pre-registered. `initAgents()` races with `subscribeToEvents()` |
| 9 | Calendar app shipped non-functional — AutoDev gaps that allowed it | [09](09-work-product-audit.md) | HIGH | Workers can mark `qc_review` without proving the artifact runs; no backend↔frontend contract handshake; `worker_frontend_recipe` literally prescribes a Vite-broken layout |
| 3 | Worker Requests dropdown collapses panel | [03](03-worker-requests-dropdown.md) | HIGH | `expandedId` state used for two orthogonal interactions |
| 6 | QC agent has no Live Monitor visibility | [06](06-qc-agent-no-visibility.md) | MEDIUM | Same root as #1 + Mode 2 doesn't emit `agent:tool-call-*` events |
| 5 | Mode 1 fallback check — was it justified? | [05](05-mode1-fallback-check.md) | INFO | One fallback, fully justified. All recent protections worked. |
| 8 | Log error scan | [08](08-log-error-scan.md) | INFO | 4 ERROR lines, all downstream of the orch crash; some log hygiene wins available |

---

## The 3 stray files — confirmed worker-generated, SAFE TO DELETE

```
C:\AIRS Foundry\autodev\autodev\src-tauri\AUTODEV_PROGRESS.md
C:\AIRS Foundry\autodev\autodev\src-tauri\server.js
C:\AIRS Foundry\autodev\autodev\src-tauri\src\components\AddEventModal.tsx
```

Evidence in [07](07-workspace-escape-rootcause.md) §4. **Delete before the next test run** or they'll trigger another HMR restart.

---

## Two new root-cause findings worth highlighting

### A. Dashboard is *architecturally* broken — `install_global` never called

Previous audit blamed missing event subscriptions. **That was wrong.** The Refresh button works perfectly: it round-trips to the backend, which calls `analytics::global()`, which falls into a pre-init fallback path at [`src-tauri/src/analytics/mod.rs:105-117`](src-tauri/src/analytics/mod.rs:105) and builds a **brand-new in-memory SQLite connection with no schema applied** on every call. Every `record_token_usage` write hits a connection without a `token_usage` table; every read returns empty. `~/.autodev/analytics.db` does not exist on disk — proof the real DB was never opened.

The `record_token_usage` failures are all swallowed by `let _ = ...` calls. The DB-backed widgets (cost trend, model breakdown, worker leaderboard, API failures, monthly prediction) will be zero **forever** until `install_global` is wired into `main.rs`. Three-line fix at the top of the next pass.

### B. QC is fixable using the subagent system that already exists

User suggested *"every task in the QC panel fires a headless QC agent so they run in parallel"*. The subagent infrastructure already supports exactly this:

- [`SubagentManager::spawn`](src-tauri/src/subagent/manager.rs:74) — full create→invoke(stream)→delete lifecycle, already production.
- [`OrchestratorClient::create_ephemeral_agent`](src-tauri/src/sidecar/client.rs:1273) — POSTs to `/api/agents` with custom `system_prompt`, `llm_model`, `mcp_servers`, `capabilities`.
- [`SubagentPool`](src-tauri/src/subagent/pool.rs:26) — `MAX_GLOBAL_CONCURRENT=16`, `MAX_PER_PARENT=10`, `MAX_NESTING_DEPTH=3` caps already enforced.
- [`subagent/bridge.rs:64`](src-tauri/src/subagent/bridge.rs:64) — fresh subagents auto-attach to `autodev-mcp-server`. Full QC tool access (analyze_diff, annotate_file, bash, update_task_status) on creation.

Recommended approach in [04](04-qc-backlog.md): replace the single qc-agent with a **qc-dispatcher** that on each tick checks the qc_review queue and spawns one ephemeral qc-reviewer subagent per task. 6 tasks → 6 parallel reviews instead of 6 serial ones. No submodule changes required.

---

## Next-steps: detailed implementation roadmap

Phases are ordered by **dependency** and **blast radius**, not by doc number. Each task names the file to edit.

### Phase 0 — Cleanup (do BEFORE next test run, ~5 min)

- [x] Delete the 3 stray files listed above
- [x] Confirm no other Tauri HMR-triggering files in `src-tauri/` or `src/` from a previous run

### Phase 1 — Load-bearing safety fixes (must ship before next E2E run) ✅ COMPLETE — 14 May 2026

Without these, the next end-to-end test will eat the dev server again or generate useless dashboard numbers.

| Task | File | What | Status |
|---|---|---|---|
| Path-jail helper | new [`src-tauri/src/agent/builtin_tools/path_jail.rs`](../../../src-tauri/src/agent/builtin_tools/path_jail.rs) | Canonicalize both paths, reject target outside `ctx.working_dir`, return self-correcting error message + 7 unit tests | ✅ |
| Wire jail into `write_file` | [`builtin_tools/write_file.rs:49-57`](../../../src-tauri/src/agent/builtin_tools/write_file.rs) | Call jail BEFORE fs::write | ✅ |
| Wire jail into `edit_file` | [`builtin_tools/edit_file.rs:79-90`](../../../src-tauri/src/agent/builtin_tools/edit_file.rs) | Same | ✅ |
| Wire jail into `append_file` | [`builtin_tools/append_file.rs:62-74`](../../../src-tauri/src/agent/builtin_tools/append_file.rs) | Same | ✅ |
| Wire jail into `bash` cwd + redirect heuristic | [`builtin_tools/bash.rs:47-72`, `bash.rs:189-280`](../../../src-tauri/src/agent/builtin_tools/bash.rs) | Reject if requested cwd escapes workspace; best-effort scan for `>` / `>>` / `tee` outside workspace | ✅ |
| **Wire `install_global` into startup** | [`src-tauri/src/main.rs:127-141`](../../../src-tauri/src/main.rs) (right after subagent install_global) | Call `AnalyticsManager::new()` + `install_global` + `start_aggregation_job` once, before any recipe scheduler starts | ✅ |
| Drop the broken FK | [`analytics/db.rs:51`](../../../src-tauri/src/analytics/db.rs) | Removed `FOREIGN KEY(session_id) REFERENCES sessions(id)` — `sessions` lives in another DB | ✅ |
| Panic loud (not silent fallback) in `analytics::global()` | [`analytics/mod.rs:96-119`](../../../src-tauri/src/analytics/mod.rs) | Replaced in-memory fallback with `expect()` so silent regressions are impossible | ✅ |
| Fail loudly on analytics write errors | [`recipes/runner.rs:376`, `agent/engine.rs:373`](../../../src-tauri/src/recipes/runner.rs) | Replaced `let _ = ...` with `if let Err(e) = ... { warn!/error!(...) }` so future regressions surface in log | ✅ |

**Verification:**
- `cargo check` clean (only pre-existing warnings)
- `cargo test --lib agent::builtin_tools::path_jail::tests` → **7 passed, 0 failed**
- `npx tsc --noEmit` clean

> The "Prefer configured workspace over `current_dir()` in tool_adapter" task originally listed here belongs to **Phase 2** of doc [07](07-workspace-escape-rootcause.md). The Phase 1 path-jail covers the escape regardless of which workspace defaulting strategy is in effect, so it was deferred. Full Phase 2 list in [07](07-workspace-escape-rootcause.md).

Full atomic task lists in [02](02-dashboard-not-live.md) Phase 1 and [07](07-workspace-escape-rootcause.md) Phase 1.

### Phase 2 — UI un-stuck (visible improvements during next run) ✅ COMPLETE — 14 May 2026

Workers run, AND the user can now see them.

| Task | File | What | Status |
|---|---|---|---|
| Lazy slug registration | [`src/stores/agentLiveStore.ts`](../../../src/stores/agentLiveStore.ts) `handleStatusChange` / `handleToolCallStart` / `handleToolCallResult` | All three handlers auto-create the agent record via `makeInitialAgent(id)` on first event; STARTUP `worker:tick-start` events are no longer dropped | ✅ |
| Earlier `initAgents` call | [`src/tabs/AutoDevPanel.tsx`](../../../src/tabs/AutoDevPanel.tsx) (top-level `AutoDevTab` effect, slug-set key) | Moved from deep `AgentRoster` mount up to the top-level `AutoDevTab` effect; dep uses sorted slug-set hash so renames/replacements re-seed | ✅ |
| Bridge Mode 2 subagent tool-call events | [`src/stores/agentLiveStore.ts`](../../../src/stores/agentLiveStore.ts) `subscribeToEvents` | Translates `subagent:event` (tagged enum `tool_start`/`tool_end`/`status`/`error`) into the same `handleToolCallStart` / `handleToolCallResult` / `handleStatusChange` / `handleError` paths so subagent activity lights up Live Monitor cards | ✅ |
| Real values in `record_worker_stats` | [`src-tauri/src/recipes/runner.rs`](../../../src-tauri/src/recipes/runner.rs) Mode 2 success branch | `cost.cost_usd` instead of `0.0`, `(elapsed_ms+59_999)/60_000` (ceiling) instead of `0` for both time_spent and avg_duration; wrapped in `if let Err … warn!` | ✅ |
| Debug-log `extract_usage` chosen branch | [`src-tauri/src/sidecar/client.rs:50-86`](../../../src-tauri/src/sidecar/client.rs) | Every branch (`dict_split` / `dict_total_70_30` / `int_total_70_30` / `char_fallback`) logs which one fired with its numbers, so future audits can tell estimate vs. real | ✅ |
| Debug-log every analytics insert | [`src-tauri/src/analytics/collector.rs`](../../../src-tauri/src/analytics/collector.rs) `record_token_usage` | One `debug!` per insert with session/model/prompt/completion/total/cost — separates write-side from read-side failures in the next audit | ✅ |
| Bridge `worker:tick-end` to Dashboard widgets | [`src/stores/dashboardStore.ts`](../../../src/stores/dashboardStore.ts) `startRealtimeUpdates` | New listener on `worker:tick-end` calling `refreshDashboard()`; DB-backed widgets refresh within ~1s of a Mode 2 tick instead of waiting up to 60s | ✅ |
| Worker Requests state split | [`src/tabs/AutoDevPanel.tsx`](../../../src/tabs/AutoDevPanel.tsx) `WorkerRequestsWidget` | `expandedId` removed; replaced with `panelOpen: boolean` + `openRequestId: number\|null`. Clicking a row no longer unmounts the list. | ✅ |
| Render payload JSON in expanded row | [`src/tabs/AutoDevPanel.tsx`](../../../src/tabs/AutoDevPanel.tsx) `WorkerRequestsWidget` | Expanded row shows `<pre>` with `JSON.stringify(parsed, null, 2)`; falls back to raw string on parse error. Collapsed row keeps the truncated single-line preview. | ✅ |
| Acknowledge / Reject buttons | [`src/tabs/AutoDevPanel.tsx`](../../../src/tabs/AutoDevPanel.tsx) `WorkerRequestsWidget` | Both buttons call `resolveWorkerRequest(id, 'completed'\|'rejected', note?)` → `invoke('resolve_worker_request', …)`. Per-row pending state, inline error message on failure, row stays open so user can retry. | ✅ |
| QC agent slug parity (read-only check) | [`src-tauri/src/db/workers.rs:178`](../../../src-tauri/src/db/workers.rs), [`src-tauri/src/recipes/defaults.rs:1112`](../../../src-tauri/src/recipes/defaults.rs) | Confirmed: both sides use canonical `qc-agent`. No mismatch. QC card will register correctly once the lazy-create path is exercised. | ✅ |

**Verification:**
- `cargo check` clean (only pre-existing warnings)
- `cargo test --lib agent::builtin_tools::path_jail::tests` → **7 passed, 0 failed** (Phase 1 tests still green)
- `npx tsc --noEmit` clean (exit 0)

Full task lists in [01](01-live-monitor-stale.md), [02](02-dashboard-not-live.md) Phase 2, [03](03-worker-requests-dropdown.md), [06](06-qc-agent-no-visibility.md).

**Deferred items from Phase 2 — ALL COMPLETED 14 May 2026:**

| Originally deferred | Now landed in | Notes |
|---|---|---|
| Doc 01 Phase 3 — model badge | `src/components/autodev/AgentLiveCard.tsx` + new `agentLiveStore.setAgentModel` | `shortModelName()` compresses registry keys to friendly chips; full id in `title` |
| Doc 01 Phase 3 — recent-tools tail | `src/components/autodev/AgentLiveCard.tsx` | Last 3 distinct tool names, shown only when nothing is in flight |
| Doc 01 Phase 3 — gray vs blue idle dot | `src/types/live.ts` + `agentLiveStore.ts` + `AgentLiveCard.tsx` | New `has_ever_been_active` flag; gray when never seen alive, info-blue when intentionally idle |
| Doc 06 Phase 3 — reviewing #N | `agentLiveStore.parseTaskId` + `AgentLiveCard.tsx` | Parses `get_task` / `update_task_status` args; writes `agent.task_id`; rendered on QC card |
| Doc 06 Phase 3 — idle breadcrumb | `src-tauri/src/events.rs` (new `EVT_WORKER_LOG_LINE`) + `recipes/runner.rs` + `agentLiveStore.ts` (`worker:log-line` listener) | Distinct messages for "no qc_review tasks" vs "only build_failed tasks" |
| Doc 06 Phase 3 — qc_review count chip | `AgentLiveCard.tsx` | QC-card-only chip, live count via `useTaskStore`, click jumps to column |
| Doc 06 Phase 3 — link to qc_review column | `uiStore.taskColumnPulse` + `AutoDevPanel.tsx` effect + `LiveMonitor.css` keyframe | scrollIntoView + 1.5s accent-glow pulse on the existing kanban column |
| Mode 2 *main-worker* SSE tool-call streaming | `OrchestratorClient::invoke_worker_streaming` (`src-tauri/src/sidecar/client.rs`); `recipes/runner.rs` swap; `subagent::bridge::{find_frame_end, parse_event_frame}` made `pub(crate)` | Worker tick now emits per-tool frames on `subagent:event` keyed by worker slug; Live Monitor card body fills in real time |

> Doc 03 Phase 3's "locate the Tauri command" task remained intentionally a no-op: `src-tauri/src/commands/task_commands.rs:172` already defines `resolve_worker_request` with the signature the frontend invokes. Verified during Phase 2 and re-confirmed here.

**Verification (Phase 3 follow-up):**
- `cargo check` clean (only pre-existing warnings)
- `cargo test --lib agent::builtin_tools::path_jail::tests` → **7 passed, 0 failed**
- `cargo test --lib subagent::bridge::tests` → **5 passed, 0 failed**
- `npx tsc --noEmit` clean (exit 0)

### Phase 3 — Per-task headless QC ✅ COMPLETE — 14 May 2026

Subagent infra was already there; the dispatcher is now wired up.

| Task | File | What | Status |
|---|---|---|---|
| Replaced `qc-agent` recipe with `qc-dispatcher` | [`src-tauri/src/recipes/defaults.rs`](../../../src-tauri/src/recipes/defaults.rs) `qc_dispatcher_recipe` + workers.db seed | Dispatcher (haiku, 30s cron) reads `qc_review` queue, caps at 6, calls `spawn_qc_review(task_id)` per task. Slug stays `qc-agent` for backward compat; recipe is what changed | ✅ |
| QC reviewer system prompt template | [`src-tauri/src/recipes/defaults.rs`](../../../src-tauri/src/recipes/defaults.rs) `qc_reviewer_system_prompt(task_id, repo_path)` | Per-task, task-id-scoped variant of the legacy QC protocol (Phases 0–5). Subagent Protocol removed (no nested QC). Exposed `pub fn` so the builtin tool can consume it | ✅ |
| `spawn_qc_review` builtin tool | new [`src-tauri/src/agent/builtin_tools/spawn_qc_review.rs`](../../../src-tauri/src/agent/builtin_tools/spawn_qc_review.rs) | Atomically claims the task via `try_claim_for_qc_review`, spawns sonnet subagent, upgrades the claim to the real subagent_id, schedules background cost-attribution and claim-release | ✅ |
| `under_review_by` + cost column | [`src-tauri/src/db/tasks.rs`](../../../src-tauri/src/db/tasks.rs) | New columns: `under_review_by TEXT`, `task_qc_cost_usd REAL DEFAULT 0.0`. Idempotent ALTERs + index on `under_review_by`. Atomic primitives: `try_claim_for_qc_review` / `upgrade_qc_claim` / `release_qc_claim` / `add_qc_cost` / `list_qc_claims` | ✅ |
| Stranded-reviewer reconciler | [`src-tauri/src/orphan/mod.rs`](../../../src-tauri/src/orphan/mod.rs) `reconcile_stranded_qc_claims` | Piggy-backs on existing orphan reconciler tick. Real subagent_id → exact match against `SubagentManager::list`; synthetic claim token → 90s age fallback. Empty registry on startup sweeps every stale claim | ✅ |
| `MAX_CONCURRENT_QC = 6` pool sub-cap | [`src-tauri/src/subagent/pool.rs`](../../../src-tauri/src/subagent/pool.rs) | Dedicated `qc: Semaphore` acquired with `try_acquire_owned` BEFORE the global cap; rejects QC bursts fast instead of back-pressuring worker subagents. `QC_DISPATCHER_SLUG` constant single-sources the trigger | ✅ |
| Live QC reviewer count Tauri cmd | [`src-tauri/src/commands/subagent_commands.rs`](../../../src-tauri/src/commands/subagent_commands.rs) `get_qc_reviewer_stats` + `lib.rs` registration | Returns `{ in_flight, max_concurrent }` — frontend QC ROI panel polls every 5s + on every subagent:event | ✅ |
| QC ROI UI panel | new [`src/components/dashboard/QcRoiPanel.tsx`](../../../src/components/dashboard/QcRoiPanel.tsx) | Top 8 tasks by QC cost; columns `#id / title / QC$ / WORKER$ / QC/W%`. Suspicious ratios (> 100%) tinted red. Header chip shows live reviewer count. Mounted in AutoDev panel below the Live Monitor | ✅ |
| qc-agent.yaml submodule update | [`submodules/core-orchestrator/config/agents/qc-agent.yaml`](../../../submodules/core-orchestrator/config/agents/qc-agent.yaml) | Persona switched to dispatcher role; model changed to haiku; system prompt rewritten to describe the fan-out role | ✅ |
| Hard cost guard ($5/hour ceiling) | [`spawn_qc_review.rs`](../../../src-tauri/src/agent/builtin_tools/spawn_qc_review.rs) `qc_cost_guard` module | Rolling 1h ring of (timestamp, cost) entries with $0.10 provisional reserve per in-flight reviewer. Trips BEFORE the atomic claim so a denied spawn doesn't leak claim state. Realised cost replaces the provisional entry on completion | ✅ |
| Analytics dispatch events | [`spawn_qc_review.rs`](../../../src-tauri/src/agent/builtin_tools/spawn_qc_review.rs) `record_analytics_event` | `qc_review_dispatched` / `_complete` / `_failed` events flow through `analytics::global().collector().record_system_event` with task_id, subagent_id, cost_usd, duration_secs, success. Queryable via the analytics DB | ✅ |

**Verification:**
- `cargo check` clean (only pre-existing warnings)
- `cargo test --lib agent::builtin_tools::path_jail::tests` → **7 passed, 0 failed**
- `cargo test --lib subagent` → **11 passed, 0 failed** (bridge + slug + pool)
- `npx tsc --noEmit` clean (exit 0)

Full task list in [04](04-qc-backlog.md).

### Phase 4 — Work-product quality gates ✅ COMPLETE — 14 May 2026

Bad work now fails loudly *before* reaching qc_review.

| Task | File | What | Status |
|---|---|---|---|
| T1.1 — tier2_imports on every write | [`builtin_tools/write_file.rs`](../../../src-tauri/src/agent/builtin_tools/write_file.rs) | Post-write runs Tier 1 + Tier 2 (imports) + Tier 2.5 (exports) + Tier 2.6 (api_contract) back-to-back | ✅ |
| T1.2 — Tier-1 TypeScript via tsc | [`builtin_tools/validate.rs`](../../../src-tauri/src/agent/builtin_tools/validate.rs) `check_tsc_syntax` | `--noEmit --allowJs --isolatedModules`. Graceful warning when tsc isn't on PATH | ✅ |
| T1.3 — tier2_5_exports cross-reference | [`builtin_tools/validate.rs`](../../../src-tauri/src/agent/builtin_tools/validate.rs) `tier2_5_exports` | Pure-Rust regex pass that verifies every relative `import { X } from './foo'` has a matching export in the target file | ✅ |
| T1.4 — Flip check_file defaults | [`builtin_tools/check_file.rs`](../../../src-tauri/src/agent/builtin_tools/check_file.rs) | Defaults to `["syntax", "imports", "exports", "contract"]`; types still opt-in | ✅ |
| T2.1 — Vite layout fix | [`recipes/defaults.rs`](../../../src-tauri/src/recipes/defaults.rs) `worker_frontend_recipe` | `index.html` moved to project root with inline "Doc 09 T2.1 — common Vite bug" warning | ✅ |
| T2.2 — Build precondition with EXIT echo | [`recipes/defaults.rs`](../../../src-tauri/src/recipes/defaults.rs) both worker recipes | Mandatory `bash('<cmd> 2>&1 | tee build.log; echo "EXIT:$?"')` form, echoed in same tick as qc_review | ✅ |
| T2.3 — Smoke probe step | [`recipes/defaults.rs`](../../../src-tauri/src/recipes/defaults.rs) both worker recipes | `timeout 5 npm run dev` smoke + SMOKE_OK gate before qc_review | ✅ |
| T2.4 — qc_review build-evidence gate | new [`builtin_tools/build_evidence.rs`](../../../src-tauri/src/agent/builtin_tools/build_evidence.rs) + [`update_task_status_tool.rs`](../../../src-tauri/src/agent/builtin_tools/update_task_status_tool.rs) | Module-level per-session ring tracks bash invocations; tool rejects qc_review without recent successful build. QC reviewers exempted via agent_role check. 6 unit tests | ✅ |
| T3.1 — write_interface_spec builtin | new [`builtin_tools/write_interface_spec.rs`](../../../src-tauri/src/agent/builtin_tools/write_interface_spec.rs) | Backend writes `<workspace>/INTERFACE.md` with METHOD / PATH / request / response / status codes / auth per route. overwrite + merge modes. worker-backend recipe Step 3.5 mandates calling before qc_review on routing tasks | ✅ |
| T3.2 — Frontend reads INTERFACE.md first | [`recipes/defaults.rs`](../../../src-tauri/src/recipes/defaults.rs) `worker_frontend_recipe` Step 3.0 | New discovery-budget-exempt step before any consumer write | ✅ |
| T3.3 — tier2_6_api_contract validation | [`builtin_tools/validate.rs`](../../../src-tauri/src/agent/builtin_tools/validate.rs) `tier2_6_api_contract` | Extracts fetch/apiClient URLs, normalises `${template}`+query, substring-matches against INTERFACE.md | ✅ |
| T3.4 — interface_drift in analyze_diff | [`builtin_tools/analyze_diff_tool.rs`](../../../src-tauri/src/agent/builtin_tools/analyze_diff_tool.rs) `detect_interface_drift` | Structured `{ status: "consumer_only"\|"contract_only"\|"both"\|"neither", spec_files, consumer_files }` field injected into JSON output. QC reviewer recipe flags `consumer_only`/`contract_only` as hard-fail | ✅ |
| T4.1 — fleet_smoke_recipe | [`recipes/defaults.rs`](../../../src-tauri/src/recipes/defaults.rs) `fleet_smoke_recipe` | New on-demand recipe (haiku, no cron). Installs deps, builds, calls e2e_probe, marks the epic done/rejected/build_failed | ✅ |
| T4.2 — e2e_probe builtin | new [`builtin_tools/e2e_probe.rs`](../../../src-tauri/src/agent/builtin_tools/e2e_probe.rs) | Spawns start_cmds with kill_on_drop, waits boot_wait_s, reqwest GET probe_url, asserts expected_text. Returns structured ProbeReport JSON. Total timeout wrapped; teardown always runs. 1 unit test | ✅ |
| T4.3 — QC reviewer requires e2e_probe | [`recipes/defaults.rs`](../../../src-tauri/src/recipes/defaults.rs) `qc_reviewer_system_prompt` Phase 4 | Phase 4 rewritten: discover start_cmds/probe_url/expected_text, run e2e_probe, verdict branches, hard-fail on consumer_only/contract_only drift | ✅ |

**Verification (full Phase 4 run):**
- `cargo check` clean (only pre-existing warnings)
- `cargo test --lib agent::builtin_tools::path_jail::tests` → **7 passed, 0 failed**
- `cargo test --lib agent::builtin_tools::build_evidence` → **6 passed, 0 failed** (new)
- `cargo test --lib agent::builtin_tools::e2e_probe` → **1 passed, 0 failed** (new)
- `cargo test --lib subagent` → **11 passed, 0 failed**
- `npx tsc --noEmit` clean (exit 0)

Full task list in [09](09-work-product-audit.md).

### Phase 5 — Observability & log hygiene (optional polish)

| Task | File | What |
|---|---|---|
| Consolidate `runtime_config` reload | search for `runtime_config: [runtime_config] loaded` (4×/run) | One load, share via `OnceLock` |
| Demote sub-5s cron-slip warns | [`src-tauri/src/recipes/scheduler.rs`](src-tauri/src/recipes/scheduler.rs) cron-slip warn site | Log at DEBUG below 5s; WARN only when slip > 5s |
| Structured error `kind=` tags | new `src-tauri/src/error_kinds.rs` module | Taxonomy: `orch_crash`, `watchdog_restart`, `mcp_unreachable`, `tool_failed`, `budget_exceeded`. Add to all ERROR log lines. |
| Workspace-mismatch banner | [`src/components/sidecar/SidecarPanel.tsx`](src/components/sidecar/SidecarPanel.tsx) | If `current_dir()` differs from configured workspace at startup, show banner |

Full task list in [05](05-mode1-fallback-check.md) Phase 1-3, [07](07-workspace-escape-rootcause.md) Phase 3-4, [08](08-log-error-scan.md).

---

## What to ship first (if you only do one thing)

**Phase 0 + Phase 1.** Without those:
- Phase 0 (delete strays): next test will trigger HMR restart again
- Phase 1 path jail: another file escape is one tool call away
- Phase 1 `install_global`: every Dashboard metric stays at zero forever, no matter how many widgets you wire up

Everything else is improvement on top of a working baseline. Those two are *required* for the baseline to exist.

---

## Run statistics

- **Duration:** ~15 min (11:14 → 11:28 local time, ended on orch crash)
- **Tasks created:** 11
- **Tasks reaching `qc_review`:** 4 (3 via reconciler at session end, not worker self-promotion)
- **Mode 1 fallbacks:** 1 (justified — orch genuinely dead)
- **TCP-alive fallback uses:** 1 (kept Mode 2 alive during busy event loop ✅)
- **Atomic implementation tasks documented:** **108** across 9 docs
- **Stray files in wrong directory:** 3 (delete before next run)
- **`~/.autodev/analytics.db` exists?** No. The DB was never opened.
