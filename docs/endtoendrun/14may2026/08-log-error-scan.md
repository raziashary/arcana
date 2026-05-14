# 08 — Log Error & Anomaly Sweep

Log: `logs/autodev-.2026-05-14..log` (999 lines, 11:14:10Z → 11:28:28Z).

## Severity: ERROR (fatal-tier)

All four ERRORs cluster in the post-restart crash window (11:27:24–27Z):

- L940 `11:27:24.542Z` — `[RecipeScheduler] Startup tick failed for 'worker-backend': … POST /api/agents/worker-backend/invoke: error sending request` — initial post-restart invoke failure.
- L948 `11:27:26.847Z` — `Startup tick failed for 'supervisor' … (retry failed)`.
- L950 `11:27:26.862Z` — `Startup tick failed for 'qc-agent' … (retry failed)`.
- L952 `11:27:26.869Z` — `Startup tick failed for 'worker-frontend' … (retry failed)`.

Root cause: core-orchestrator (`:4000`) crashed shortly after restart. See [05-mode1-fallback-check.md](./05-mode1-fallback-check.md). These are downstream symptoms of the orch death, not independent bugs.

## Severity: WARN (serious, non-transient)

### Sidecar config-update 500s (post-restart)

- L900 `11:27:22.636Z` — `failed to push model update for 'supervisor': agent config update failed (HTTP 500 Internal Server Error)`
- L901–905 — same pattern for `worker-frontend`, `worker-cleanup`, `worker-mcp`, `worker-platform`, `qc-agent`.

The orch returned 500 on `PUT /api/agents/<name>` for **every** agent within the same 1 ms window. Likely the same crash already in progress — but the HTTP 500 (vs connection refused) suggests the FastAPI process was up but its agent layer was broken (e.g., DB or LLM-key load failure on a freshly re-launched orch).

### Orphan release on restart

- L840 `11:27:13.838Z` — `[OrphanDetector] Released orphaned task #11 'Integrate backend API calls with React components': host process PID 20440 exited` — confirms previous host process death; **task #11** was held by a dead worker and got correctly auto-released back to `ready`.

### LivingAgent invoke connection errors

- L939 `11:27:24.534Z` (worker-backend, no retry) — `LivingAgent invoke failed: error sending request`.
- L942–944 — `invoke connection error for 'supervisor'/'worker-frontend'/'qc-agent', retrying in 100ms` (new retry path).
- L945–947 — retries failed for all three. The new `(retry failed)` suffix shows the retry code ran but the orch was actually down.
- All four logs include `Aborting tick (NOT falling back to Mode 1 — risk of double-execution). Next scheduled tick will retry Mode 2.` — the double-execution guard worked as designed.

### Health-check threshold crossing

- L963 `11:27:47.975Z` — `health check failed 6 consecutive times AND TCP dead — declaring unhealthy, switching workers to Mode 1 fallback`. Single legitimate crossing — see Doc 05.

### Cron slips (cosmetic but worth noting)

- L364 `11:16:56.424Z` (3.57s), L405 `11:17:58.714Z` (1.29s), L582 `11:21:59.436Z` (0ns), L728 `11:25:29.995Z` (0ns), L756 `11:26:22.237Z` (0ns), L763 `11:26:27.523Z` (0ns), L789 `11:27:08.960Z` (0ns) — `cron slipped, enforcing 10s floor`. Triggered when supervisor/worker-backend finish a tick right at the next cron boundary. Benign — floor protects against tick storms.

## Tool execution failures

**None observed.** All `edit_file`, `glob` DEBUG lines complete cleanly (e.g. L536, L573, L617, L750, L764, L775). No `Tool failed`, no `tool error`, no bash crash markers. The only `Failed`/`failed` matches in the log are the orch HTTP errors above.

## LivingAgent invoke timeouts

No timeout-class errors (no `timed out`, no `deadline exceeded`). The invoke failures are connection-level (`error sending request for url`), not timeouts.

## Sidecar crashes / restarts

**One full host restart at 11:27:12.648Z** (L795–L827):
- L795 `[logging] AutoDev logging initialised` (fresh banner)
- L816 `[StartupProgress] init (0/13)`
- L821 `core-orchestrator confirmed healthy — fleet dispatch latch released`
- L827 `[StartupProgress] ready (12/13)`

Previous host PID 20440 had exited (L840 orphan release confirms this). The new host launched a fresh orch which began returning 500 within ~9 seconds, then went TCP-dead within ~35 seconds. Python traceback would live in a separate orch sidecar log not in this Rust log.

## Orphan-detection releases

- L840–841 — exactly one: task #11 released after PID 20440 exit. No other releases.

## Budget-exceeded events

**None.** No `budget exceeded`, `warn_pct`, or `over budget` lines. Default budgets (100k tokens, $0.5, 20 turns) loaded multiple times (L24, L131–138, L818, L956–959) but never tripped during the run.

## Suspicious task transitions

All `Reconciled stranded in_progress task #N → qc_review` events fired correctly when worker ticks ended without explicit completion:

- L491 task #8 (worker-backend) at 11:19:53.504Z — preceded by 198s LivingAgent tick (L486).
- L581 task #3 (worker-backend) at 11:21:59.432Z — preceded by 113s tick.
- L659 task #9 (worker-frontend) at 11:23:51.999Z — preceded by **436s** tick.
- L762 task #1 (worker-backend) at 11:26:27.519Z — preceded by **254s** tick.

No `assigned → ready` skip (no orphaned ready-state without a done in between). The transitions are sane; the symptom is that the worker LivingAgent never called `update_task_status(..., 'qc_review')` itself — see Doc 04.

## Health-check warns that were benign retries (skipped per brief)

- L335, L356, L372, L376, L386, L387, L388, L396, L439, L561, L642, L690, L691, L722, L773, L782, L954, L955, L960, L961, L962 — all `health check failed (n/6) — treating as healthy until threshold reached`. Self-recovered or escalated to L963. The TCP-alive escape at L389 is the working failsafe.

## Severity ranking

1. **L963 — single Mode 1 transition (justified, orch dead).**
2. **L840 — orphan release on restart (system working correctly).**
3. **L900–905, L939–952 — post-restart orch failure cluster** (root cause = orch dying after restart; needs separate investigation of orch sidecar logs).
4. **Cron slips** — purely cosmetic.

No silent failures, no wrong task transitions, no LLM tool crashes. The dispatch layer behaved correctly throughout — the run ended because the orchestrator process itself died, not because of a defect in the Rust scheduler/runner code.

---

## Atomic implementation tasks

> This doc is INFO — no functional bugs found. These tasks are log-hygiene and observability improvements.

### Phase 1: Reduce log noise

- [ ] Investigate why `runtime_config` reloads 4× per run — `src-tauri/src/sidecar/runtime_config.rs` (and any caller in `src-tauri/src/sidecar/startup.rs`) — log L24, L131–138, L818, L956–959 show the default-budgets banner re-emitted on every reload; identify the duplicate load sites and consolidate to a single startup load + on-change watcher. Goal: one banner per host lifetime.
- [ ] Confirm `MCP server TCP probe OK :8767` stays at DEBUG — `src-tauri/src/mcp_server/probe.rs` (or wherever the probe lives) — read-only verification that the line is `tracing::debug!` not `info!`; if it has been silently promoted, demote it. No user-facing INFO churn for healthy probes.
- [ ] Demote cron-slip warnings to DEBUG when slip < 5s — `src-tauri/src/recipes/scheduler.rs` (the `cron slipped, enforcing 10s floor` emit site) — keep WARN only for slips ≥ 5s; below that they are routine and clutter the log (L364, L405, L582, L728, L756, L763, L789).

### Phase 2: Structured error categorization

- [ ] Add a `kind` field to all ERROR-level events — `src-tauri/src/recipes/scheduler.rs` (Startup tick failed site, L940/L948/L950/L952) and `src-tauri/src/sidecar/watchdog.rs` (health threshold crossing, L963) — include a `tracing` field like `kind = "orch_crash"` or `kind = "watchdog_restart"` so a `tail -f | grep 'kind='` is grep-able.
- [ ] Define the kind taxonomy in a central module — new file `src-tauri/src/observability/error_kinds.rs` — `pub const KIND_ORCH_CRASH: &str = "orch_crash";` etc., with a short doc comment per kind. Reference from all emit sites so the set stays bounded.
- [ ] Document the taxonomy — `docs/development/LOG_ERROR_KINDS.md` (new file) — table of `kind` → meaning → triage owner, linked from the main developer docs.

### Phase 3 (optional): Log retention policy

- [ ] Verify daily rotation is active — `src-tauri/src/observability/logging.rs` (or wherever `tracing-appender` is configured) — confirm `tracing_appender::rolling::daily(...)` is in use and rolls at UTC midnight.
- [ ] Document retention defaults — `docs/development/LOGGING.md` (existing or new) — record the default retention (e.g. 14 days), the rotation strategy, and how to override via env var or settings.
- [ ] Add a configurable retention cap — `src-tauri/src/observability/logging.rs` — read `AUTODEV_LOG_RETENTION_DAYS` (default 14); prune logs older than the cap on host startup with a one-line `tracing::info!` summarizing how many files were dropped.
