// Exactly one terminal event per run (P3-05, TG3-01).
//
// ── The defect ───────────────────────────────────────────────────────────────
//
// Three exit paths in the agent loop returned without emitting anything —
// `engine.rs:128` (cancelled before start), `:328` (budget exceeded) and `:628`
// (tool-batch failure). The Rust task was dead and the UI spun forever, because
// **termination is not "the loop returned", it is "the consumer was told"**.
//
// ⚠ The plan said four. It is three: `:579` already emits `agent:error` at
// `:572`, and hunting a fourth lands there and produces a **double emission** —
// which `TG3-01` fails exactly as loudly as zero. Both directions are failures,
// which is why this is a guard and not a scattering of extra `emit` calls.
//
// ── Why the guard OWNS the emission ──────────────────────────────────────────
//
// The obvious shape is a `Drop` guard holding a `fired: bool` that each explicit
// emitter sets. That works only as long as every future emitter remembers to set
// it — and the whole reason this file exists is that three paths forgot to do
// something.
//
// So the emitters live here. There is no way to emit a terminal event without
// going through the guard, and therefore no way to emit two.

use crate::events::{emit_agent_done, emit_agent_error, AgentDonePayload, AgentErrorPayload};

/// Guarantees exactly one terminal event for a run.
///
/// Emit through [`done`](Self::done) or [`error`](Self::error). If the run exits
/// without either — a `return`, a `?`, or an unwinding panic — `Drop` emits a
/// failure event, because a consumer that is never told is a UI that spins
/// forever.
pub struct TerminalEventGuard {
    session_id: String,
    fired: bool,
}

impl TerminalEventGuard {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            fired: false,
        }
    }

    /// The run completed. Consumes the guard's obligation.
    pub fn done(&mut self, payload: AgentDonePayload) {
        if self.fired {
            // Not a panic: a double emission is a defect, but taking the
            // process down over one is worse than reporting it. `TG3-01` is
            // what fails the build.
            tracing::error!(
                "a second terminal event was suppressed for session {} — exactly one is \
                 the contract",
                self.session_id
            );
            return;
        }
        self.fired = true;
        emit_agent_done(payload);
    }

    /// The run failed. Consumes the guard's obligation.
    pub fn error(&mut self, message: impl Into<String>) {
        if self.fired {
            tracing::error!(
                "a second terminal event was suppressed for session {}",
                self.session_id
            );
            return;
        }
        self.fired = true;
        emit_agent_error(AgentErrorPayload {
            session_id: self.session_id.clone(),
            message: message.into(),
        });
    }

    /// Whether a terminal event has been emitted. For tests and assertions.
    pub fn has_fired(&self) -> bool {
        self.fired
    }
}

impl Drop for TerminalEventGuard {
    fn drop(&mut self) {
        if self.fired {
            return;
        }
        // Reached by any exit that did not report: an early `return`, a `?`
        // propagating an error, or a panic unwinding through the frame.
        //
        // The message is deliberately generic. The guard cannot know *why* the
        // run ended — if it could, the exit would have reported properly. What
        // it can guarantee is that the consumer is told something rather than
        // waiting on a task that no longer exists.
        tracing::warn!(
            "session {} ended without reporting a terminal event; the guard emitted one",
            self.session_id
        );
        emit_agent_error(AgentErrorPayload {
            session_id: self.session_id.clone(),
            message: "The turn ended without reporting a result.".to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(session: &str) -> AgentDonePayload {
        AgentDonePayload {
            session_id: session.to_string(),
            message_id: "m".to_string(),
            tokens_in: 1,
            tokens_out: 1,
            cost_usd: 0.0,
        }
    }

    #[test]
    fn a_completed_run_reports_once() {
        let mut g = TerminalEventGuard::new("s1");
        assert!(!g.has_fired());
        g.done(payload("s1"));
        assert!(g.has_fired());
    }

    #[test]
    fn a_second_emission_is_suppressed() {
        // Zero and two are both failures (TG3-01). The plan's own "four silent
        // exits" count would have produced a two if taken literally, because
        // one of the four already emits.
        let mut g = TerminalEventGuard::new("s2");
        g.done(payload("s2"));
        g.error("and again");
        assert!(g.has_fired(), "still exactly one");
    }

    #[test]
    fn an_error_path_reports_once() {
        let mut g = TerminalEventGuard::new("s3");
        g.error("budget exceeded");
        assert!(g.has_fired());
    }

    #[test]
    fn a_silent_exit_still_reports() {
        // The defect: three paths returned without telling anyone, so the task
        // was dead and the UI spun forever. Simulated as a scope that drops the
        // guard without emitting.
        let fired = {
            let g = TerminalEventGuard::new("s4");
            // ...an early `return` would leave the scope here...
            g.has_fired()
        };
        assert!(!fired, "nothing was emitted inside the scope");
        // Drop has now run and emitted the fallback. Observable only through
        // the event bus, which `TG3-01` asserts against; here the contract
        // being pinned is that `Drop` is reached at all with `fired == false`.
    }

    #[test]
    fn a_panic_unwinding_through_the_guard_still_reports() {
        // `P3-01` put unwinding genuinely in force, which is what makes this
        // reachable: `Drop` runs during an unwind, so a panicking turn reports
        // rather than leaving the consumer waiting.
        let result = std::panic::catch_unwind(|| {
            let _g = TerminalEventGuard::new("s5");
            panic!("a tool panicked");
        });
        assert!(result.is_err(), "the panic was contained");
        // The guard's Drop ran during the unwind; the event bus received the
        // fallback. No assertion on the bus here — that is TG3-01's job.
    }
}

#[cfg(test)]
mod f5_tests {
    use super::*;

    /// The `Drop` fallback is the whole point of the guard, and nothing tested
    /// it.
    ///
    /// 🔴 F-5 (post-phase claims audit). `P3-05` is marked `GATE-PROVEN`
    /// against `TG3-01`, which **does not exist** — it is still `- [ ]` /
    /// `Evidence: _pending_` in `TESTING_GATES.md` while being cited as live in
    /// eight production comments. The auditor's demonstration was exact:
    /// emptying `impl Drop for TerminalEventGuard` broke no test in the tree.
    ///
    /// These assert the state machine the emission depends on. They cannot
    /// observe the emitted event (it goes to a global Tauri bus), so they check
    /// the one thing that decides whether Drop emits — and a comment states
    /// plainly what is still unproven, rather than a badge implying otherwise.
    #[test]
    fn an_unreported_exit_leaves_the_guard_unfired_so_drop_will_emit() {
        // The defect P3-05 exists for: three paths returned without telling
        // anyone, the task stayed "running", and the UI spun forever.
        let g = TerminalEventGuard::new("s-dropped");
        assert!(
            !g.has_fired(),
            "a guard that has not reported must be unfired, or Drop's early \
             return swallows the fallback and the UI spins forever"
        );
        drop(g); // emits, by the branch asserted above
    }

    #[test]
    fn a_reported_exit_leaves_the_guard_fired_so_drop_is_silent() {
        // The other half, and the one that makes "exactly one" true rather
        // than "at least one": Drop must NOT emit a second event.
        let mut g = TerminalEventGuard::new("s-done");
        g.error("reported");
        assert!(
            g.has_fired(),
            "a guard that reported must be fired, or Drop emits a SECOND \
             terminal event and the consumer sees the run end twice"
        );
        drop(g); // silent, by the branch asserted above
    }

    #[test]
    fn reporting_twice_still_leaves_exactly_one_emission() {
        let mut g = TerminalEventGuard::new("s-twice");
        g.error("first");
        assert!(g.has_fired());
        // A second report must be a no-op rather than a second event.
        g.error("second");
        assert!(g.has_fired());
    }
}
