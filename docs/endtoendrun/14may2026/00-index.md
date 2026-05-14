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

### Phase 1 — Load-bearing safety fixes (must ship before next E2E run)

Without these, the next end-to-end test will eat the dev server again or generate useless dashboard numbers.

| Task | File | What |
|---|---|---|
| Path-jail helper | new `src-tauri/src/agent/builtin_tools/path_jail.rs` | Canonicalize both paths, reject target outside `ctx.working_dir`, return self-correcting error message |
| Wire jail into `write_file` | [`builtin_tools/write_file.rs:111-114`](src-tauri/src/agent/builtin_tools/write_file.rs:111) | Call jail BEFORE fs::write |
| Wire jail into `edit_file` | [`builtin_tools/edit_file.rs:79-83`](src-tauri/src/agent/builtin_tools/edit_file.rs:79) | Same |
| Wire jail into `append_file` | [`builtin_tools/append_file.rs:62-66`](src-tauri/src/agent/builtin_tools/append_file.rs:62) | Same |
| Wire jail into `bash` cwd | [`builtin_tools/bash.rs:152`](src-tauri/src/agent/builtin_tools/bash.rs:152) | Reject if requested cwd escapes workspace |
| Prefer configured workspace over `current_dir()` | [`mcp_server/tool_adapter.rs:33-42`](src-tauri/src/mcp_server/tool_adapter.rs:33) | Read from `workspace_commands::load_workspace_dir()` first, only fall back to `current_dir()` |
| **Wire `install_global` into startup** | [`src-tauri/src/main.rs`](src-tauri/src/main.rs) (early in setup) | Call `analytics::install_global(autodev_dir)` once, before any recipe scheduler starts |
| Drop the broken FK | [`analytics/db.rs:51`](src-tauri/src/analytics/db.rs:51) | Remove `FOREIGN KEY(session_id) REFERENCES sessions(id)` — `sessions` lives in another DB |
| Fail loudly on analytics write errors | [`analytics/collector.rs`](src-tauri/src/analytics/collector.rs) | Replace `let _ = ...` callers' silence with at least a `warn!` so we see future regressions |

Full atomic task lists in [02](02-dashboard-not-live.md) Phase 1 and [07](07-workspace-escape-rootcause.md) Phase 1.

### Phase 2 — UI un-stuck (visible improvements during next run)

Workers run, but the user can't see them.

| Task | File | What |
|---|---|---|
| Lazy slug registration | [`src/stores/agentLiveStore.ts:200`](src/stores/agentLiveStore.ts:200) (`handleStatusChange` and siblings) | Auto-create agent record if missing instead of silent `return state` |
| Earlier `initAgents` call | [`src/tabs/AutoDevPanel.tsx`](src/tabs/AutoDevPanel.tsx) | Move `initAgents()` to mount, before `subscribeToEvents()` could fire |
| Bridge `worker:tick-end` to Dashboard widgets | [`src/stores/dashboardStore.ts`](src/stores/dashboardStore.ts) | Subscribe to `worker:tick-end` and refresh DB-backed summary |
| Worker Requests state split | [`src/tabs/AutoDevPanel.tsx:565-714`](src/tabs/AutoDevPanel.tsx:565) | Split `expandedId` into `panelOpen: boolean` and `openRequestId: number \| null` |
| Render payload JSON in expanded row | [`src/tabs/AutoDevPanel.tsx`](src/tabs/AutoDevPanel.tsx) | `<pre>` block when row open |
| Acknowledge / Reject buttons | [`src/tabs/AutoDevPanel.tsx`](src/tabs/AutoDevPanel.tsx) | Wire to existing `resolve_worker_request` Tauri command |

Full task lists in [01](01-live-monitor-stale.md), [02](02-dashboard-not-live.md) Phase 2, [03](03-worker-requests-dropdown.md), [06](06-qc-agent-no-visibility.md).

### Phase 3 — Per-task headless QC (the user's idea, big throughput win)

Subagent infra is already there; we just need a dispatcher.

| Task | File | What |
|---|---|---|
| Replace `qc-agent` recipe with `qc-dispatcher` | [`src-tauri/src/recipes/defaults.rs`](src-tauri/src/recipes/defaults.rs) (qc_agent_recipe section) | Dispatcher reads `qc_review` queue, spawns N ephemeral qc-reviewer subagents in parallel |
| QC reviewer system prompt template | [`src-tauri/src/recipes/defaults.rs`](src-tauri/src/recipes/defaults.rs) | Constant string with the canonical QC review protocol (currently embedded in qc-agent recipe) |
| Subagent spawn for QC | [`src-tauri/src/subagent/manager.rs`](src-tauri/src/subagent/manager.rs) usage from new dispatcher | Use existing `spawn` API; pass `system_prompt` = QC template, `model` = sonnet, `mcp_servers` = `["autodev-mcp-server"]` |
| QC pool sub-cap | [`src-tauri/src/subagent/pool.rs`](src-tauri/src/subagent/pool.rs) | Optional separate cap for QC subagents (~6) so they don't crowd out worker-spawned subagents |
| UI: QC subagent cards | [`src/tabs/AutoDevPanel.tsx`](src/tabs/AutoDevPanel.tsx) Live Monitor section | Show each active QC reviewer with its target task #N |

Full task list in [04](04-qc-backlog.md).

### Phase 4 — Work-product quality gates (close the loop on bad output)

Make bad work fail loudly *before* reaching qc_review.

| Task | File | What |
|---|---|---|
| Extend `validate.rs` Tier 1 to TypeScript | [`src-tauri/src/agent/builtin_tools/validate.rs`](src-tauri/src/agent/builtin_tools/validate.rs) | Add `.ts` / `.tsx` syntax check via `tsc --noEmit` or quick parser |
| New `tier2_imports` pass | [`src-tauri/src/agent/builtin_tools/write_file.rs`](src-tauri/src/agent/builtin_tools/write_file.rs) post-write hook | After write, check imports resolve to existing files / package.json deps |
| New `tier2_5_exports` pass | new file in `src-tauri/src/agent/builtin_tools/` | Detect empty `module.exports = {}` files that other files import from |
| Flip `check_file.rs` defaults | [`src-tauri/src/agent/builtin_tools/check_file.rs`](src-tauri/src/agent/builtin_tools/check_file.rs) | Run by default after write, not opt-in |
| Fix `worker_frontend_recipe` Vite layout bug | [`src-tauri/src/recipes/defaults.rs:594`](src-tauri/src/recipes/defaults.rs:594) | Recipe currently prescribes `public/index.html` — Vite expects root. This is a **recipe bug**, not LLM failure. |
| Enforce build before `qc_review` | [`src-tauri/src/agent/builtin_tools/update_task_status_tool.rs`](src-tauri/src/agent/builtin_tools/update_task_status_tool.rs) | Workers must run `npm run build` / `cargo check` and capture output before transitioning to qc_review |
| New `write_interface_spec` tool | new file in `src-tauri/src/agent/builtin_tools/` | Worker writes contract YAML (route paths, request/response shapes) to `INTERFACE.md` for downstream workers to consume |
| New `e2e_probe.rs` builtin | new file in `src-tauri/src/agent/builtin_tools/` | Fleet smoke test: start servers, hit endpoints, assert responses match interface spec |

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
