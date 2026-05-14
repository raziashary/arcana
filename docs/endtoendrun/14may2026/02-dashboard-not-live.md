# Dashboard not real-time + Refresh button broken

## TL;DR

`analytics::install_global` is never called from `main.rs`. Every call to `analytics::global()` therefore falls into the OnceLock pre-init fallback, which builds a brand-new `AnalyticsManager` backed by an **in-memory SQLite connection with NO schema applied** — and it does so on *every call*. Every `record_token_usage` write goes to a throw-away connection that doesn't even have a `token_usage` table; every read returns empty. The manual Refresh button works perfectly — it just queries an empty void. `~/.autodev/analytics.db` does not exist on disk, which proves the real DB was never opened.

## Investigation

### Frontend refresh handler

`src/components/dashboard/Dashboard.tsx:127` wires the Refresh button to `refreshDashboard` from `useDashboardStore`. The store action (`src/stores/dashboardStore.ts:163-245`) calls five Tauri commands in series: `get_dashboard_summary`, `get_cost_analytics`, `get_token_usage`, `get_api_failures`, `predict_monthly_cost`. Each `invoke` is wrapped in `.catch(() => ({ ...zeroed defaults }))` so a backend failure is silently swallowed and the UI receives an all-zero payload — exactly the symptom the user reports. The frontend is not the problem.

### Backend Tauri command

`src-tauri/src/commands/analytics_commands.rs:10` defines `get_dashboard_summary`. It reads two in-process counters from `AppState` (`cost_today_usd`, `tokens_in_today + tokens_out_today`) and forwards them to `analytics::global().queries().get_dashboard_summary(live_cost_usd, live_tokens)`. The queries layer (`src-tauri/src/analytics/queries.rs:25-119`) uses the live counters for today's cost/tokens, then queries the DB for yesterday's rollup, worker counts, and today's activity. The other four commands (`get_cost_analytics`, `get_token_usage`, `get_api_failures`, `predict_monthly_cost`) read **exclusively** from the analytics DB via `manager.queries()`.

### Where Mode 2 data SHOULD be recorded

`src-tauri/src/recipes/runner.rs:376-388` calls `analytics::global().collector().record_token_usage(...)` after every successful LivingAgent tick, passing `tokens_in`, `tokens_out`, computed `cost.cost_usd`, and a synthesised `session_id` like `mode2-supervisor-1715680000123`. Lines 361-364 also push the same numbers into the shared `AppState` via `s.add_cost(...)`, and lines 392-399 emit a `cost:update` Tauri event. `record_worker_stats` is called at line 415-424 (with hardcoded `cost=0.0, time_spent=0` — that's a separate cosmetic issue, not the root cause). Mode 1 has an analogous call at `src-tauri/src/agent/engine.rs:375`. The system_event recorder is wired into `src-tauri/src/recipes/scheduler.rs:222`.

### Where Mode 2 data actually goes

Nowhere durable. `src-tauri/src/analytics/mod.rs:105-117` defines `analytics::global()` with this fallback:

```rust
pub fn global() -> Arc<AnalyticsManager> {
    if let Some(m) = ANALYTICS_MANAGER.get() { return m.clone(); }
    // Pre-init fallback — fresh in-memory DB on EVERY call, no schema.
    Arc::new(AnalyticsManager {
        db: Arc::new(AnalyticsDb { conn: Mutex::new(Connection::open_in_memory().unwrap()) }),
        collector: Arc::new(AnalyticsCollector::new(Arc::new(AnalyticsDb { conn: Mutex::new(Connection::open_in_memory().unwrap()) }))),
        ...
    })
}
```

`AnalyticsManager::new()` (mod.rs:38) is **never invoked** anywhere in the codebase. `install_global` is **never called** — grep over `src-tauri/src` shows zero call sites. `main.rs` does not contain the string "analytics" anywhere. As a direct consequence:

1. Every Mode 2 tick's `record_token_usage` call hits a brand-new `Connection::open_in_memory()` that has no `token_usage` table. The insert returns `Err(SqliteFailure)` (no such table), but the call site uses `let _ =` so the error is discarded.
2. Even if a schema existed, the collector's DB Arc is **different** from the queries' DB Arc (the fallback constructs four separate in-memory DBs), so a hypothetical successful write would be invisible to reads anyway.
3. `~/.autodev/analytics.db` is never created. Confirmed: `dir C:\Users\Razi Ashary\.autodev` lists `annotations.db`, `config.db`, `memory.db`, `tasks.db`, `workers.db` — but **no `analytics.db`**.
4. The today-running log `logs/autodev-.2026-05-14..log` contains zero lines mentioning `analytics`, `token_usage`, `insert_token`, or `record_token`. It contains many `LivingAgent tick complete for 'supervisor' in 35506ms` lines — meaning ticks fired, finished, but never logged a write because writes go to a black hole and use `let _`.

The in-process `AppState.cost_today_usd` *is* being updated correctly by runner.rs:363, so the Sidebar/StatusBar cost meter (which reads that field directly) would show non-zero. But the Dashboard's `summary.today_tokens` is computed from `tokens_in_today + tokens_out_today` (analytics_commands.rs:18) — those *should* also be non-zero. If the user sees zero there too, it suggests the `s.add_cost(...)` path itself is never reached because `extract_usage` returned (0, 0) — possible if `tokens_used` was None and the char/4 multiplier produced zero for short responses, but more likely the `cost_today_usd` is genuinely positive and the dashboard's zeros come purely from the DB-backed widgets.

### Why even the live event listeners can't help

The `cost:update` listener in `dashboardStore.ts:358-397` only mutates `summary.today_cost` and `summary.today_tokens` optimistically — but `summary` starts as `null` (initial state at line 148) and is only populated by a successful `refreshDashboard()`. If the initial refresh returns the all-zero fallback payload (because any invoke threw), `summary` is set but with zeros. The cost-update listener then adds deltas to those zeros, but **the other widgets** (`CostAnalytics.data_points`, `tokenUsage.by_model`, `apiFailures`) come from analytics-DB-backed queries that will always return empty arrays. The 60-second polling fallback (`Dashboard.tsx:43-45`) re-queries the same broken pipeline. The `agent:done` and `task:changed` listeners trigger another `refreshDashboard()` — same empty DB, same zeros. No matter which code path fires (manual click, polling, event-driven), the destination is the same: an unmigrated in-memory SQLite connection.

## Root cause

`analytics::install_global` is never called during Tauri startup. `main.rs` constructs and manages `app_state`, `session_mgr`, `pending_permissions`, `sidecar_mgr`, `orch_client`, `subagent_mgr`, `skill_registry`, `mcp_registry`, `updater_state`, and `mcp_health_monitor`, but it never builds an `AnalyticsManager`. Every `analytics::global()` consumer therefore falls back to a schemaless in-memory DB created fresh per call, so every write is silently dropped and every read is empty. The Dashboard's Refresh button works end-to-end; the underlying data store simply has no rows because the production database object was never wired up.

## Atomic implementation tasks

### Phase 1: Wire up the analytics singleton

- [ ] Task 1.1 — `src-tauri/src/main.rs:120` (right after `subagent::manager::install_global`) — add `let analytics_mgr = tauri::async_runtime::block_on(autodev_lib::analytics::AnalyticsManager::new()).expect("Failed to initialise analytics DB"); autodev_lib::analytics::install_global(Arc::new(analytics_mgr));` so the OnceLock holds a real disk-backed manager before any Tauri command, RecipeRunner tick, or scheduler hook fires.
- [ ] Task 1.2 — `src-tauri/src/analytics/mod.rs:105` — replace the silent fallback in `global()` with a panic-on-miss (`expect("analytics::install_global not called")`). Silent fallbacks have already cost us one full audit cycle; loud failure is cheaper.
- [ ] Task 1.3 — `src-tauri/src/analytics/db.rs:51` — drop the `FOREIGN KEY(session_id) REFERENCES sessions(id)` clause. The `sessions` table lives in `autodev.db`, not `analytics.db`, so the FK references a non-existent target. Even with the FK removed, `token_usage.session_id` remains a plain `TEXT NOT NULL` index column, which is all the queries need.
- [ ] Task 1.4 — `src-tauri/src/analytics/mod.rs:75` (`start_aggregation_job`) — call this from `main.rs` immediately after `install_global` so the hourly + daily rollups actually run.

### Phase 2: Verify the write path

- [ ] Task 2.1 — `src-tauri/src/recipes/runner.rs:376` — replace the `let _ =` with explicit `if let Err(e) = ... { warn!("[Mode2] record_token_usage failed: {e}"); }` so future DB failures are visible in `logs/`.
- [ ] Task 2.2 — `src-tauri/src/recipes/runner.rs:415-424` — populate the `record_worker_stats` call with real values: pass `cost.cost_usd` instead of `0.0`, and `(elapsed_ms / 60_000) as i32` instead of `0` for `time_spent_minutes` and `avg_task_duration_minutes`. Without this, the leaderboard widget always shows zero per-worker spend even after the DB is wired.
- [ ] Task 2.3 — `src-tauri/src/sidecar/client.rs:50-83` (`extract_usage`) — log the chosen branch at debug level so we can verify whether real `tokens_used` dicts are arriving from core-orchestrator or whether the char/4 fallback is doing all the work.
- [ ] Task 2.4 — `src-tauri/src/analytics/collector.rs:18` — emit a `tracing::debug!` line on every insert with `(session_id, model, total_tokens, cost_usd)`. Without that breadcrumb the next audit will again be unable to tell write-side from read-side failures.

### Phase 3 (optional): UI polish

- [ ] Task 3.1 — `src/stores/dashboardStore.ts:166-228` — drop the per-invoke `.catch(() => zeroed)` swallowers and surface errors on a single banner so silent backend failures become visible to the user instead of rendering a misleading all-zero dashboard.
- [ ] Task 3.2 — `src/components/dashboard/Dashboard.tsx:127` — disable the Refresh button while `isLoading` AND show a last-refreshed-at timestamp so the user can tell whether a click actually round-tripped.
- [ ] Task 3.3 — `src/components/dashboard/Dashboard.tsx:144-194` — add an explicit "No data yet" empty state per widget (instead of rendering "$0.00" + "0 tokens" + "0 workers") so a freshly initialised DB looks distinct from a broken pipeline.
- [ ] Task 3.4 — `src-tauri/src/commands/analytics_commands.rs:10` — add a one-shot `tracing::info!` on first invocation logging `(analytics_db_path, row_counts)` to help future on-call debugging.
