# 05 — Mode 1 Fallback Check

Log: `logs/autodev-.2026-05-14..log` (11:14:10Z → 11:28:28Z).

## Summary

**Yes, Mode 1 fallback triggered — once, at the very end of the run, and it was justified (orch process actually died).** Before that, the run stayed on Mode 2 throughout, and the TCP-alive fallback fired exactly when expected.

## Mode 1 transition timeline

The fleet ran healthy on Mode 2 from 11:14:13Z until 11:27:22Z. Then this sequence:

1. L900–905 `11:27:22.636Z` — six WARNs: `failed to push model update for '<worker>' (HTTP 500 Internal Server Error)`. Sidecar started returning 500s.
2. L939 `11:27:24.534Z` — `LivingAgent invoke failed for 'worker-backend': error sending request for url (http://127.0.0.1:4000/api/agents/worker-backend/invoke). Aborting tick (NOT falling back to Mode 1 — risk of double-execution).` — connection now refused.
3. L942–947 — supervisor, worker-frontend, qc-agent all hit the same invoke failure, retry once, fail again.
4. L954–955 `11:27:35.275Z` — `health check failed during startup grace (1/6) … staying optimistic` (startup-grace mode, in another launch attempt).
5. L960–962 `11:27:45.908Z` — health check failed 3/6, 4/6, 5/6 in rapid succession.
6. **L963 `11:27:47.975Z` — `health check failed 6 consecutive times AND TCP dead: error sending request for url (http://127.0.0.1:4000/health) — declaring unhealthy, switching workers to Mode 1 fallback`** ← the unhealthy crossing.
7. L971–996 `11:28:04Z` — every worker tick now logs `core-orchestrator=unreachable → Mode 1 (Rust Engine)` and immediately skips because **`no LLM provider configured. Waiting for core-orchestrator (Mode 2).`** Mode 1 is selected but cannot do useful work, so this is effectively a graceful idle, not a re-run on the Rust engine.

## Was it justified?

Yes — TCP and HTTP both dead simultaneously. Note the new code path triggered correctly:

- The earlier 6× /health failure at L389 `11:17:16.448Z` stayed on Mode 2: `/health failed 6× but TCP :4000 is alive — keeping Mode 2; orchestrator is busy, not dead.` This is the new TCP-alive fallback you mentioned — it worked exactly once during the run and behaved correctly (orch was busy serving a long supervisor tick around then).
- The fatal crossing at L963 explicitly says "AND TCP dead," meaning both probes failed — not a premature decision.

## Did `is_healthy()` return false at any other point?

No. Only L963 crosses the threshold. All other warns are 1–5/6 (transient retries) and self-recover.

## TCP-alive fallback firings

Only one: L389 `11:17:16.448Z` (`/health failed 6× but TCP :4000 is alive — keeping Mode 2`). Surrounding events confirm orch was busy (supervisor LivingAgent tick completed at L398 `11:17:58.701Z`, 42 seconds after the /health timeouts started). This is the canonical "busy event loop" pattern and the fallback worked.

## Did workers ever fail ensure_agent / invoke_worker?

Yes — but only during the orch crash window:

- L939 worker-backend invoke failure
- L945 supervisor invoke failure (retry failed)
- L946 worker-frontend invoke failure (retry failed)
- L947 qc-agent invoke failure (retry failed)
- L940, L948, L950, L952 ERROR scheduler "Startup tick failed for '…'"

The retries (L942–944, "retrying in 100ms") followed the new code path. None succeeded — because the orch was really down.

Before the crash window: zero invoke failures across the entire run.

## Was there an unrecoverable crash?

Yes. Evidence pointing to a clean orch restart attempt:
- L795 `11:27:12.648Z` — fresh `AutoDev logging initialised` line (logging banner) inside the same log file. This is the host process re-initializing.
- L816 `11:27:13.282Z` — `[StartupProgress] init (0/13)` from scratch.
- L827 `11:27:13.598Z` — `[StartupProgress] ready (12/13)`.
- L821 `11:27:13.596Z` — `core-orchestrator confirmed healthy — fleet dispatch latch released` (so it briefly looked healthy).
- L840 `11:27:13.838Z` — `[OrphanDetector] Released orphaned task #11 'Integrate backend API calls with React components': host process PID 20440 exited` — the orphan-detector confirms the previous host process died.

So a host restart happened at 11:27:12. The new orch came up but rapidly began returning 500s and then went unreachable (TCP dead by L963). No Python traceback appears in the captured Rust log — Python output would be in a separate sidecar log.

## Verdict

- **Mode 1 fallback fired exactly once (L963), and was justified** — TCP truly dead.
- **TCP-alive fallback fired once (L389), and correctly kept Mode 2** through a busy-event-loop window.
- **No premature fallback occurred.** HEALTH_FAIL_THRESHOLD=6 + TCP probe combination behaved as designed.
- The Mode 1 path is non-functional because Mode 1 has `no LLM provider configured` — but that is by design, not a regression.

The crash itself (orch restart at 11:27:12, then full TCP death by 11:27:48) is a separate root cause to investigate — possibly related to the agent config update returning 500 (L900–905) immediately after restart.

---

## Atomic implementation tasks

> This doc is mostly INFO — fallback behaviour was correct. These tasks are defensive improvements rather than bug fixes.

### Phase 1: Better orch-crash diagnostics — ✅ COMPLETE 14 May 2026

- [x] Watchdog → Tauri event wiring — `src-tauri/src/sidecar/mod.rs` circuit-breaker-open branch + new `EVT_SIDECAR_CRASH` in `events.rs` + `SidecarCrashPayload` + `emit_sidecar_crash_global`. When the breaker opens, the captured stderr tail (last 40 lines) is forwarded along with `restart_count`, `window_secs`, and an RFC-3339 timestamp.
- [x] New `src/components/SidecarCrashBanner.tsx` — subscribes to `sidecar:crash`. Renders a danger-tinted banner with the failure summary and folds the stderr tail into a `<details>` block so the Python traceback is one click away. Mounted at the top of `App.tsx` between the workspace banners.
- [x] "Copy crash report" button — bundles the stderr tail with the sidecar id, restart count, window, ISO timestamp, and the host's user-agent string into a markdown-formatted report; writes via `navigator.clipboard.writeText`. Confirmation tick (✓ Copied) flips for 2s.

### Phase 2: Mode 1 graceful degradation — ✅ COMPLETE 14 May 2026

- [x] New `docs/development/MODE1_FALLBACK_RECIPES.md` documents the two-policy taxonomy (`mode2_only` / `mode1_capable`), why most recipes are `mode2_only` (MCP tool dependency), and a 4-item checklist for promoting a recipe to `mode1_capable`. Cross-references `defaults.rs` and `LOG_ERROR_KINDS.md`.
- [x] `src-tauri/src/recipes/runner.rs` — new structured log line on the Mode 1 selection branch: `info!(mode = "mode1", mode1_capable = true|false, provider = …, …)`. `grep 'mode1_capable=true'` over a run shows which workers actually ran on the Rust engine.
- [x] `src/stores/agentLiveStore.ts` `worker:status` listener — explicit mapping for the new statuses `mode1_idle_no_provider` / `fallback_idle` / `mode1_active` → frontend enum. `src/types/live.ts` `AgentLiveStatus` extended with `fallback_idle` and `mode1_active`. `src/components/autodev/AgentLiveCard.tsx` `STATUS_DOT_COLORS` paints both with `var(--warning)` (amber) so they're visually distinct from ordinary blue idle.

### Phase 3 (optional): Per-worker fallback policy — ✅ COMPLETE 14 May 2026

- [x] `src-tauri/src/recipes/types.rs` — new `FallbackPolicy` enum (`Mode2Only` default, `Mode1Capable`). Added to `Recipe` struct with `#[serde(default)]` so existing on-disk YAML deserializes as `Mode2Only` without migration.
- [x] `src-tauri/src/recipes/defaults.rs` — every recipe literal now declares `fallback_policy`. 8 recipes default to `Mode2Only` (supervisor, qc-agent, qc-dispatcher, fleet-smoke, worker-backend/frontend/platform/mcp); `worker-cleanup` is the lone `Mode1Capable` per the doc's recommendation.
- [x] `src-tauri/src/recipes/runner.rs` — after the Mode 2 attempt fails / orch is unreachable, the runner checks `recipe.fallback_policy`. `Mode2Only` recipes emit `worker:tick-end` with `outcome="fallback_skipped_policy"`, set the worker status to `fallback_idle`, log `Mode 1 fallback skipped per policy`, and return without invoking the Rust engine. `Mode1Capable` recipes fall through to the existing provider-resolution code path unchanged.
