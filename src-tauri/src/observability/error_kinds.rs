// Doc 08 Phase 2 — structured error kind taxonomy.
//
// Every ERROR-level tracing event SHOULD carry a `kind = "<value>"` field
// so `tail -f | grep 'kind='` is grep-able for triage. The constants below
// are the canonical set. New kinds must be added here (and to
// docs/development/LOG_ERROR_KINDS.md) before they appear at a call site —
// keeps the set bounded.
//
// Usage:
//   use crate::observability::error_kinds;
//   error!(kind = error_kinds::KIND_ORCH_CRASH, "orch died: {e}");

/// Core-orchestrator (Python sidecar) process died or returned 500s.
/// Triage owner: sidecar team. See
/// `src-tauri/src/sidecar/watchdog.rs::is_healthy` for the detection point.
pub const KIND_ORCH_CRASH: &str = "orch_crash";

/// Watchdog circuit breaker opened: 6 consecutive /health failures AND TCP
/// dead. The Rust runner stops attempting Mode 2 invokes until the breaker
/// closes again. Triage: same root as orch_crash, with the additional fact
/// that the auto-retry policy gave up.
pub const KIND_WATCHDOG_RESTART: &str = "watchdog_restart";

/// An MCP server AutoDev depends on became unresponsive.
///
/// This used to mean AutoDev's OWN in-process server on :8767, which was
/// deleted in P0. It now covers the external MCP servers AutoDev connects to
/// as a client (see `mcp/health.rs`).
pub const KIND_MCP_UNREACHABLE: &str = "mcp_unreachable";

/// A builtin tool returned a hard error (non-zero exit / panic / bad input).
/// Bash, write_file, edit_file, etc. all surface failures through here.
pub const KIND_TOOL_FAILED: &str = "tool_failed";

/// Budget Circuit Breaker (Ruflo Feature 4) blocked a tool call — the task
/// exceeded token / cost / turn budget. Distinct from tool_failed because
/// the cause is a policy decision, not a runtime fault.
pub const KIND_BUDGET_EXCEEDED: &str = "budget_exceeded";

/// LivingAgent invoke failed at the connection level (network blip / orch
/// restart). The retry policy at `client.rs::try_invoke` already handles
/// the transient case; emit this kind when retry also fails.
pub const KIND_INVOKE_FAILED: &str = "invoke_failed";

/// Path-jail (Doc 07) refused a write because the target escaped the
/// workspace. Useful as a tripwire for misbehaving prompts; the operation
/// is always blocked before this kind fires.
pub const KIND_WORKSPACE_ESCAPE: &str = "workspace_escape";

/// QC reviewer subagent reached a terminal failed state. Emitted from
/// `spawn_qc_review.rs`'s completion handler so SOC dashboards can plot
/// reviewer failure rates over time.
pub const KIND_QC_REVIEWER_FAILED: &str = "qc_reviewer_failed";

/// Analytics write to the SQLite analytics DB failed. Caught by the
/// runner's `if let Err(e) = …` blocks (Phase 1 audit fix). Surfaces the
/// rare schema / disk-full / lock-timeout cases.
pub const KIND_ANALYTICS_WRITE_FAILED: &str = "analytics_write_failed";

#[cfg(test)]
mod tests {
    use super::*;

    /// Catch typos / re-use within the constants — every kind must be
    /// unique, and snake_case-only.
    #[test]
    fn kinds_are_unique_and_snake_case() {
        let kinds = [
            KIND_ORCH_CRASH,
            KIND_WATCHDOG_RESTART,
            KIND_MCP_UNREACHABLE,
            KIND_TOOL_FAILED,
            KIND_BUDGET_EXCEEDED,
            KIND_INVOKE_FAILED,
            KIND_WORKSPACE_ESCAPE,
            KIND_QC_REVIEWER_FAILED,
            KIND_ANALYTICS_WRITE_FAILED,
        ];
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for k in &kinds {
            assert!(seen.insert(*k), "duplicate kind: {k}");
            assert!(
                k.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "kind '{k}' must be snake_case ASCII"
            );
        }
    }
}
