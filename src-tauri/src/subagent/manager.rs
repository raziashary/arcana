// SubagentManager — single source of truth for the live subagent registry.
//
// Responsibilities:
//   1. Track every spawned SubagentHandle (id → entry)
//   2. Forward bridge events to:
//        - Tauri global event bus  (`subagent:event`)
//        - In-memory last_event field (for SubagentSummary)
//   3. Provide list/get/abort/cleanup operations to the Tauri command layer
//   4. Orphan cleanup on process startup (Layer 3 of the cleanup story —
//      see PHASE_4_9_EPHEMERAL_VIA_LIFECYCLE.md)
//
// Concurrency: a tokio::Mutex around the registry HashMap. All operations
// are short — we never hold the lock across HTTP calls.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use chrono::Utc;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// How long a terminal (Completed/Failed/Terminated) entry is kept in the
/// registry before `prune_terminal` sweeps it out. Long enough for the UI
/// to display the final result card, short enough to prevent unbounded growth.
const TERMINAL_RETENTION: Duration = Duration::from_secs(60);

use super::native_runner::{self, RunnerDeps};
use super::pool::{PoolPermit, SubagentPool};
use super::slug;
use super::types::{
    SubagentConfig, SubagentEvent, SubagentHandle, SubagentResult, SubagentStatus, SubagentSummary,
};
use crate::errors::{AutoDevError, Result};
use crate::events::{emit_subagent_event_global, SubagentEventPayload};

struct ManagerEntry {
    handle: SubagentHandle,
    last_event: Option<String>,
    /// Drop guard — releases the global / per-parent pool slot.
    _permit: PoolPermit,
    /// Background task running `native_runner::run`. Aborting this future
    /// cancels the child engine's turn loop (used by `abort_subagent`).
    task: Option<JoinHandle<Result<SubagentResult>>>,
    /// Set when the entry reaches a terminal state. Used by `prune_terminal`
    /// to sweep entries older than TERMINAL_RETENTION.
    terminated_at: Option<Instant>,
}

pub struct SubagentManager {
    deps: RunnerDeps,
    pool: Arc<SubagentPool>,
    registry: Mutex<HashMap<String, ManagerEntry>>,
}

impl SubagentManager {
    pub fn new(deps: RunnerDeps) -> Arc<Self> {
        Arc::new(Self {
            deps,
            pool: SubagentPool::new(),
            registry: Mutex::new(HashMap::new()),
        })
    }

    /// Spawn a subagent and start its lifecycle in the background.
    /// Returns the handle immediately; the run completes asynchronously.
    ///
    /// Use `await_result(id)` to block on the final SubagentResult, or
    /// `terminate(id)` / `abort(id)` to stop early.
    pub async fn spawn(
        self: &Arc<Self>,
        mut config: SubagentConfig,
        prompt: String,
    ) -> Result<SubagentHandle> {
        // Reserve a slot before generating the slug so depth/per-parent
        // rejections happen before any HTTP work.
        let permit = self.pool.acquire(&config.parent_slug, config.depth).await?;

        // Default values when caller / LLM omitted them.
        if config.max_turns == 0 {
            config.max_turns = SubagentConfig::DEFAULT_MAX_TURNS;
        }
        if config.timeout_secs == 0 {
            config.timeout_secs = SubagentConfig::DEFAULT_TIMEOUT_SECS;
        }
        if config.model.is_empty() {
            config.model = SubagentConfig::DEFAULT_MODEL.to_string();
        }

        let id = slug::generate(&config.parent_slug);
        let handle = SubagentHandle {
            id: id.clone(),
            config: config.clone(),
            status: SubagentStatus::Spawning,
            created_at: Utc::now(),
        };

        // Internal channel: bridge → manager → (event bus + last_event)
        let (tx, rx) = mpsc::unbounded_channel::<SubagentEvent>();

        // Launch the runner task.
        let run_deps = self.deps.clone();
        let run_id = id.clone();
        let run_cfg = config.clone();
        let task =
            tokio::spawn(
                async move { native_runner::run(run_deps, run_id, run_cfg, prompt, tx).await },
            );

        // Launch the event pump (consumes rx, fans out to global bus + state).
        let pump_manager = self.clone();
        let pump_id = id.clone();
        tokio::spawn(async move {
            pump_manager.pump_events(pump_id, rx).await;
        });

        // Prune stale terminal entries before inserting the new one so the
        // registry doesn't grow unboundedly over a long session.
        self.prune_terminal().await;

        // Insert into registry.
        self.registry.lock().await.insert(
            id.clone(),
            ManagerEntry {
                handle: handle.clone(),
                last_event: None,
                _permit: permit,
                task: Some(task),
                terminated_at: None,
            },
        );

        info!(
            "[SubagentManager] spawned {id} (parent={}, depth={})",
            config.parent_slug, config.depth
        );
        Ok(handle)
    }

    /// Wait for a previously-spawned subagent to finish, returning its result.
    /// If the subagent is unknown or its task already joined, returns an error.
    pub async fn await_result(self: &Arc<Self>, id: &str) -> Result<SubagentResult> {
        let task = {
            let mut reg = self.registry.lock().await;
            let entry = reg
                .get_mut(id)
                .ok_or_else(|| AutoDevError::Agent(format!("unknown subagent {id}")))?;
            entry
                .task
                .take()
                .ok_or_else(|| AutoDevError::Agent(format!("subagent {id} already awaited")))?
        };

        match task.await {
            Ok(Ok(res)) => {
                self.finalise(id, &res.status).await;
                Ok(res)
            }
            Ok(Err(e)) => {
                self.finalise(id, &SubagentStatus::Failed).await;
                Err(e)
            }
            Err(join_err) => {
                self.finalise(id, &SubagentStatus::Failed).await;
                Err(AutoDevError::Agent(format!(
                    "subagent {id} task join error: {join_err}"
                )))
            }
        }
    }

    /// Status snapshot for a single subagent.
    pub async fn get_status(&self, id: &str) -> Result<SubagentStatus> {
        let reg = self.registry.lock().await;
        reg.get(id)
            .map(|e| e.handle.status)
            .ok_or_else(|| AutoDevError::Agent(format!("unknown subagent {id}")))
    }

    /// List all live subagents, optionally filtered by parent session.
    pub async fn list(&self, parent_session_id: Option<&str>) -> Vec<SubagentSummary> {
        self.prune_terminal().await;
        let reg = self.registry.lock().await;
        reg.values()
            .filter(|e| {
                parent_session_id
                    .map(|p| e.handle.config.parent_session_id == p)
                    .unwrap_or(true)
            })
            .map(|e| SubagentSummary {
                id: e.handle.id.clone(),
                parent_slug: e.handle.config.parent_slug.clone(),
                parent_session_id: e.handle.config.parent_session_id.clone(),
                status: e.handle.status,
                model: e.handle.config.model.clone(),
                created_at: e.handle.created_at,
                depth: e.handle.config.depth,
                last_event: e.last_event.clone(),
            })
            .collect()
    }

    /// Graceful termination: cancels the bridge task, then ensures core-orchestrator
    /// has the agent deleted.
    pub async fn terminate(self: &Arc<Self>, id: &str, force: bool) -> Result<()> {
        let task = {
            let mut reg = self.registry.lock().await;
            let entry = reg
                .get_mut(id)
                .ok_or_else(|| AutoDevError::Agent(format!("unknown subagent {id}")))?;
            entry.handle.status = SubagentStatus::Terminated;
            entry.task.take()
        };

        if let Some(t) = task {
            if force {
                t.abort();
            }
            let _ = t.await;
        }

        // Nothing to clean up remotely: aborting the task above ends the
        // in-process engine. The old path had to DELETE an ephemeral agent from
        // core-orchestrator here.

        self.registry.lock().await.remove(id);
        info!("[SubagentManager] terminated {id} (force={force})");
        Ok(())
    }

    /// Hard abort — alias for terminate(force=true).
    pub async fn abort(self: &Arc<Self>, id: &str) -> Result<()> {
        self.terminate(id, true).await
    }

    /// Orphan cleanup. Retained as a no-op so startup wiring and the Tauri
    /// command that calls it keep working.
    ///
    /// It used to DELETE every `subagent-*` agent left registered in
    /// core-orchestrator after a crash. Subagents are now in-process tokio
    /// tasks: when AutoDev exits they exit with it, so no cross-process orphan
    /// can survive a restart and there is nothing to sweep.
    pub async fn cleanup_orphans(&self) {
        debug!("[SubagentManager] orphan cleanup is a no-op: subagents are in-process");
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    /// Background task that consumes the bridge's event channel and:
    ///   1. Updates the registry's status / last_event fields
    ///   2. Forwards the event to the Tauri event bus
    async fn pump_events(
        self: Arc<Self>,
        id: String,
        mut rx: mpsc::UnboundedReceiver<SubagentEvent>,
    ) {
        while let Some(evt) = rx.recv().await {
            // Update entry's mirror fields for the Monitor UI.
            {
                let mut reg = self.registry.lock().await;
                if let Some(entry) = reg.get_mut(&id) {
                    entry.last_event = Some(short_label(&evt));
                    if let SubagentEvent::Status { status, .. } = &evt {
                        entry.handle.status = *status;
                        // Stamp the terminal time HERE as well as in
                        // `finalise()`. `prune_terminal` only evicts entries
                        // with `terminated_at` set, and `finalise` runs only
                        // from `await_result` — so a subagent that completed
                        // but was never awaited stayed in the registry forever
                        // and, because `_permit` is a drop-guard held by the
                        // entry, never released its pool slot. Found by the P0
                        // post-phase audit (F-02).
                        if status.is_terminal() && entry.terminated_at.is_none() {
                            entry.terminated_at = Some(Instant::now());
                        }
                    }
                }
            }

            // Fan out to the global event bus.
            emit_subagent_event_global(SubagentEventPayload {
                subagent_id: evt.subagent_id().to_string(),
                event: evt,
            });
        }
    }

    /// Finalise an entry after the bridge task joins. We don't drop it
    /// immediately — keeping it for a short retention window lets the UI
    /// display the final result. The pool permit is released here.
    async fn finalise(&self, id: &str, status: &SubagentStatus) {
        let mut reg = self.registry.lock().await;
        if let Some(entry) = reg.get_mut(id) {
            entry.handle.status = *status;
            if status.is_terminal() {
                entry.terminated_at = Some(Instant::now());
            }
        }
    }

    /// Remove registry entries whose terminal status has been held longer
    /// than TERMINAL_RETENTION. Safe to call frequently — O(n) walk.
    async fn prune_terminal(&self) {
        let now = Instant::now();
        let mut reg = self.registry.lock().await;
        reg.retain(|id, entry| {
            if let Some(t) = entry.terminated_at {
                let stale = now.duration_since(t) > TERMINAL_RETENTION;
                if stale {
                    debug!("[SubagentManager] pruned terminal entry {id}");
                }
                !stale
            } else {
                true // not terminal yet — keep
            }
        });
    }
}

// ── Global accessor (parallel to recipes::scheduler::install_global) ────────
//
// Builtin tools run in async tasks without Tauri `State<>` access. They
// reach the SubagentManager via this OnceLock instead of having to receive
// it via ToolContext. Installed once from main.rs after the manager is
// constructed.

static GLOBAL: OnceLock<Arc<SubagentManager>> = OnceLock::new();

/// Install the process-wide SubagentManager. Idempotent — subsequent calls
/// are no-ops (the first installation wins).
pub fn install_global(mgr: Arc<SubagentManager>) {
    let _ = GLOBAL.set(mgr);
}

/// Access the process-wide SubagentManager. Returns None during early boot
/// or in unit tests that don't install one. Tools must handle None
/// gracefully (typically by returning a "subagents unavailable" tool error).
pub fn global() -> Option<Arc<SubagentManager>> {
    GLOBAL.get().cloned()
}

/// Compact human-readable label for "last event" tooltip / list view.
fn short_label(evt: &SubagentEvent) -> String {
    match evt {
        SubagentEvent::Token { .. } => "streaming…".into(),
        SubagentEvent::Thinking { .. } => "thinking…".into(),
        SubagentEvent::ToolStart { tool, .. } => format!("tool: {tool}"),
        SubagentEvent::ToolEnd { tool, is_error, .. } => {
            if *is_error {
                format!("tool failed: {tool}")
            } else {
                format!("tool ok: {tool}")
            }
        }
        SubagentEvent::Final { .. } => "final".into(),
        SubagentEvent::Cost { .. } => "cost update".into(),
        SubagentEvent::Status { status, .. } => format!("status: {}", status.as_str()),
        SubagentEvent::Error { message, .. } => format!("error: {}", truncate(message, 60)),
    }
}

/// Truncate to at most `max` CHARACTERS.
///
/// This sliced bytes (`&s[..max]`), which panics when byte `max` is not a UTF-8
/// char boundary — and it is reached from `short_label` inside the detached
/// `pump_events` task, where a panic kills the pump silently and every later
/// subagent event is lost. A provider error message containing any non-ASCII
/// character was enough. Found by the P0 post-phase audit (F-03).
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max).collect();
        format!("{head}…")
    }
}

#[cfg(test)]
mod truncate_tests {
    use super::truncate;

    #[test]
    fn short_strings_are_returned_whole() {
        assert_eq!(truncate("hello", 60), "hello");
    }

    #[test]
    fn multibyte_input_does_not_panic_at_the_cut() {
        // Every one of these used to panic: the byte at the cut index is in
        // the middle of a multi-byte character.
        // Each of these must be LONGER than the 60-char cut, or nothing is
        // truncated and the test asserts nothing.
        for input in ["é".repeat(80), "日本語テキスト".repeat(20), "🔥".repeat(80)] {
            assert!(input.chars().count() > 60, "test input must exceed the cut");
            let out = truncate(&input, 60);
            assert!(
                out.chars().count() <= 61,
                "got {} chars",
                out.chars().count()
            );
            assert!(out.ends_with('…'));
        }
    }

    #[test]
    fn truncation_counts_characters_not_bytes() {
        // 10 two-byte chars is 20 bytes; a byte-based check would not truncate.
        let out = truncate(&"é".repeat(10), 5);
        assert_eq!(out.chars().count(), 6, "5 chars plus the ellipsis");
    }
}
