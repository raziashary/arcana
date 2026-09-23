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

// 🔴 `KIND_ORCH_CRASH` and `KIND_WATCHDOG_RESTART` are DELETED (`P8-60`).
// Both named the health of a second runtime and its watchdog, and both had
// **zero emitters** — their detection point was
// `src-tauri/src/sidecar/watchdog.rs`, removed in `P8-B`.
//
// ⚠ **Finding, not fixed here.** Of the nine kinds this taxonomy declared,
// exactly ONE has an emitter (`KIND_INVOKE_FAILED`, at
// `recipes/scheduler.rs`). The rest — `KIND_MCP_UNREACHABLE`,
// `KIND_TOOL_FAILED`, `KIND_BUDGET_EXCEEDED` and the others below — name
// live concepts and are still never logged, so the triage vocabulary a
// responder would grep for does not appear in any log. That is out of `P8`'s
// scope (they name nothing deleted) and is reported rather than deleted:
// the fix is to emit them, not to remove the names.

/// An MCP server Arcana depends on became unresponsive.
///
/// This used to mean Arcana's OWN in-process server on :8767, which was
/// deleted in P0. It now covers the external MCP servers Arcana connects to
/// as a client (see `mcp/health.rs`).
pub const KIND_MCP_UNREACHABLE: &str = "mcp_unreachable";

/// A builtin tool returned a hard error (non-zero exit / panic / bad input).
/// Bash, write_file, edit_file, etc. all surface failures through here.
pub const KIND_TOOL_FAILED: &str = "tool_failed";

/// Budget Circuit Breaker (Ruflo Feature 4) blocked a tool call — the task
/// exceeded token / cost / turn budget. Distinct from tool_failed because
/// the cause is a policy decision, not a runtime fault.
pub const KIND_BUDGET_EXCEEDED: &str = "budget_exceeded";

/// A recipe run failed at the point of invocation — the provider rejected
/// the call, a tool errored, or the recipe itself is malformed. Emitted by
/// `recipes/scheduler.rs` for a failed start-up tick.
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
    ///
    /// ⚠ The population is **derived from this file's own source**, not
    /// retyped. It used to be a hand-written array of all nine names, and
    /// deleting two constants in `P8-60` broke the test rather than the test
    /// noticing — which is the wrong direction: a list that must agree with the
    /// declarations will disagree with them, and the one nobody updates is the
    /// copy (`L-19`). Adding a constant now covers it automatically; forgetting
    /// to is no longer possible.
    #[test]
    fn kinds_are_unique_and_snake_case() {
        let src = include_str!("error_kinds.rs");
        let kinds: Vec<&str> = src
            .lines()
            .filter_map(|l| l.trim().strip_prefix("pub const KIND_"))
            .filter_map(|rest| rest.split_once('"'))
            .filter_map(|(_, v)| v.split('"').next())
            .collect();

        assert!(
            kinds.len() >= 5,
            "only {} kind constant(s) parsed out of this file — the extractor \
             stopped matching the declaration shape, so this test is checking \
             almost nothing: {kinds:?}",
            kinds.len()
        );

        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for k in &kinds {
            assert!(seen.insert(k), "duplicate kind: {k}");
            assert!(
                k.chars().all(|c: char| c.is_ascii_lowercase() || c == '_'),
                "kind '{k}' must be snake_case ASCII"
            );
        }
    }
}
