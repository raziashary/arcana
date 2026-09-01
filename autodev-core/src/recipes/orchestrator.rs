// The fleet's view of an external orchestrator process.
//
// `RecipeRunner`'s Mode 2 path drives the Python `core-orchestrator` through
// `sidecar::client::OrchestratorClient`. That client is HTTP-plus-process
// supervision owned by the desktop app, it is deleted wholesale in P8, and the
// CLI has no orchestrator at all — three reasons it does not belong in core.
//
// This is one of the two genuine host seams identified in
// ADR-LOG-RESTRUCT-0015: a capability whose behaviour actually differs between
// hosts, rather than infrastructure the engine owns. It is deliberately two
// methods wide — exactly what `RecipeRunner` calls, nothing speculative.
//
// ⚠ Mode 2 is inert as of P0 (ADR-LOG-RESTRUCT-0008): `orch_healthy` is forced
// false, so `ensure_agent` is not reached today. The seam is still wired to the
// real client rather than to `NoOrchestrator`, because ADR-LOG-RESTRUCT-0009
// records what happened last time this path was "simplified" on the assumption
// it was dead: the fleet-ready latch above it went with it and every worker
// went silently idle on machines without Python.

use async_trait::async_trait;

use crate::errors::{AutoDevError, Result};

/// Result of a Mode 2 agent invocation. Carries the response content and an
/// approximation of token usage, so the recipe runner can record analytics and
/// update the cost meter without the orchestrator having to report them.
#[derive(Debug, Clone)]
pub struct WorkerInvokeResult {
    pub content: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
    /// Model reported by the orchestrator's response metadata, if any. May be
    /// in the orchestrator's normalised key format (e.g. `anthropic_claude_sonnet`).
    pub model: Option<String>,
}

/// An orchestrator the fleet can probe and register agents with.
#[async_trait]
pub trait OrchestratorProbe: Send + Sync {
    /// Whether the orchestrator is reachable and ready to accept work.
    ///
    /// Implementations must not block indefinitely: this is called on the tick
    /// path, and a hung probe stalls the whole fleet.
    async fn is_healthy(&self) -> bool;

    /// Ensure an agent exists for `slug` running `model`, returning its id.
    async fn ensure_agent(&self, slug: &str, model: &str) -> Result<String>;

    /// Run one turn on `agent_id`, streaming progress events as it goes.
    async fn invoke_worker_streaming(
        &self,
        agent_id: &str,
        instructions: &str,
        trigger: &str,
        worker_slug: &str,
    ) -> Result<WorkerInvokeResult>;
}

/// The implementation for a host with no orchestrator — `airsctl`, and any
/// desktop build after P8 deletes the sidecar.
///
/// `is_healthy` is false rather than true so callers take their Mode 1 path,
/// and `ensure_agent` is a loud error rather than a fabricated id: a caller
/// that reaches it has already misread `is_healthy`, and silently handing back
/// a plausible-looking agent id would turn that into a much harder bug.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOrchestrator;

#[async_trait]
impl OrchestratorProbe for NoOrchestrator {
    async fn is_healthy(&self) -> bool {
        false
    }

    async fn ensure_agent(&self, slug: &str, _model: &str) -> Result<String> {
        Err(AutoDevError::Agent(format!(
            "No orchestrator is configured in this host, so no agent could be ensured for '{slug}'"
        )))
    }

    async fn invoke_worker_streaming(
        &self,
        _agent_id: &str,
        _instructions: &str,
        _trigger: &str,
        worker_slug: &str,
    ) -> Result<WorkerInvokeResult> {
        Err(AutoDevError::Agent(format!(
            "No orchestrator is configured in this host, so '{worker_slug}' could not be invoked"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn no_orchestrator_reports_unhealthy_so_callers_fall_back() {
        assert!(!NoOrchestrator.is_healthy().await);
    }

    #[tokio::test]
    async fn no_orchestrator_refuses_loudly_rather_than_inventing_an_agent_id() {
        let err = NoOrchestrator
            .ensure_agent("worker-backend", "some-model")
            .await
            .expect_err("must not fabricate an agent id");
        assert!(err.to_string().contains("worker-backend"), "got: {err}");
    }
}
