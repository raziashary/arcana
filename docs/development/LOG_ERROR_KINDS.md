# Log error `kind=` taxonomy

Doc 08 Phase 2. Every ERROR or WARN-level tracing event that represents a
*recoverable failure of a known kind* SHOULD carry a `kind = "<value>"`
field. The taxonomy is small and bounded; new kinds must be added to both
`src-tauri/src/observability/error_kinds.rs` and this doc before they
appear at a call site.

A `tail -f ~/.autodev/logs/autodev-*.log | grep 'kind='` shows exactly the
recoverable-failure stream without scanning every WARN/ERROR line.

## Canonical kinds

| `kind` value | Meaning | Triage owner | First emit site |
|---|---|---|---|
| `orch_crash` | core-orchestrator (Python sidecar) process died or returned 500s for every agent. | Sidecar team | `sidecar/watchdog.rs` (planned — currently logged without `kind=`) |
| `watchdog_restart` | Watchdog circuit breaker opened: 6 consecutive `/health` failures AND TCP dead. Mode 2 disabled until reachable. | Sidecar team | `sidecar/client.rs::is_healthy` (HEALTH_FAIL_THRESHOLD crossing) |
| `mcp_unreachable` | autodev-mcp-server on :8767 stopped responding. Different from orch_crash because MCP lives in-process. | Host runtime | `mcp_server/health_monitor.rs` (planned) |
| `tool_failed` | A builtin tool returned a hard error (non-zero exit / panic / bad input). | Builtin authors | `agent/builtin_tools/*.rs` |
| `budget_exceeded` | Budget Circuit Breaker blocked a tool call. Policy decision, not a runtime fault. | Recipe authors | `agent/budget.rs` (planned) |
| `invoke_failed` | LivingAgent invoke failed at the connection level after retries also failed. | Sidecar team | `recipes/scheduler.rs::Startup tick failed` |
| `workspace_escape` | Path-jail (Doc 07) blocked a write whose canonical target escaped the workspace. | Recipe authors / model | `agent/builtin_tools/path_jail.rs` callers (planned) |
| `qc_reviewer_failed` | A `qc-reviewer-*` subagent reached a terminal failed state. | QC infra | `agent/builtin_tools/spawn_qc_review.rs` (planned) |
| `analytics_write_failed` | A write to the analytics SQLite DB failed. | Analytics infra | `recipes/runner.rs` / `agent/engine.rs` (planned) |

## Usage convention

```rust
use crate::observability::error_kinds;

if let Err(e) = client.invoke_worker(…).await {
    error!(
        kind = error_kinds::KIND_INVOKE_FAILED,
        worker_slug = slug.as_str(),
        "invoke failed for '{slug}': {e}",
    );
}
```

The `kind` field appears as a structured key/value in the `tracing` output
alongside the human-readable message. Both grep-style scans
(`grep 'kind=invoke_failed'`) and structured-log consumers can filter on it.

## Adding a new kind

1. Add the `pub const KIND_…` line to
   `src-tauri/src/observability/error_kinds.rs`.
2. Update the unit test `kinds_are_unique_and_snake_case` (it iterates the
   set — append the new constant or it won't be checked).
3. Add a row to the table above (this file).
4. Replace the bare `error!(...)` at the call site with the keyed form.

Kinds are intentionally coarse. Resist the urge to add fine-grained variants
(`orch_crash_oom`, `orch_crash_segfault`) — the human-readable message
already carries that information, and the kind is meant to coarsely route a
log line to its triage owner, not to enumerate every failure mode.
