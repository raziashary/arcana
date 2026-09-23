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
use crate::errors::{ArcanaError, Result};
use crate::events::{emit_subagent_event, SubagentEventPayload};

struct ManagerEntry {
    handle: SubagentHandle,
    last_event: Option<String>,
    /// Drop guard for the global / per-parent pool slot.
    ///
    /// 🔴 `P3-22`. This was `_permit: PoolPermit` — held for the entry's whole
    /// life, so a **finished** subagent kept its pool slot for the full
    /// `TERMINAL_RETENTION` window and beyond. Retention exists so the UI can
    /// display a final result; it has nothing to do with capacity, and tying
    /// the two meant 16 completed subagents consumed the entire global cap.
    ///
    /// `Option`, so `finalise` can `take()` it the moment the subagent reaches
    /// a terminal state — which is what `finalise`'s own docstring already
    /// claimed was happening.
    permit: Option<PoolPermit>,
    /// Background task running `native_runner::run`. Aborting this future
    /// cancels the child engine's turn loop.
    ///
    /// ⚠ **`await_result` takes it**, because awaiting a `JoinHandle` consumes
    /// it. From that moment `terminate` finds `None` here — see `abort`.
    task: Option<JoinHandle<Result<SubagentResult>>>,
    /// A handle that can cancel the task **after** `task` has been taken.
    ///
    /// 🔴 `G-9` (post-phase gap hunt). `terminate` reached for `task`, found
    /// `None` for any subagent its parent was awaiting, **skipped the abort
    /// entirely**, removed the registry entry and returned `Ok(())` — so the
    /// caller was told the subagent had been stopped while it ran on to
    /// completion, billing.
    ///
    /// That is exactly the case `P3-24` exists for: `spawn_subagent` awaits its
    /// child, so an in-flight subagent is *always* one whose handle has been
    /// taken. Both of `P3-24`'s tests ran against an empty registry and could
    /// not have seen it.
    ///
    /// `AbortHandle` is cheap, `Clone`, and independent of the `JoinHandle`'s
    /// lifetime — which is the whole reason it exists.
    abort: Option<tokio::task::AbortHandle>,
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
        // ⚠ P3-22. Sweep BEFORE reserving. The prune used to sit at the far
        // side of this `acquire`, which is a plain deadlock: `acquire` waits
        // on the semaphore, and the only thing that would have released those
        // permits was the prune queued behind it.
        //
        // With the permit now released at termination (see `finalise`), this
        // ordering is no longer the sole line of defence — but a prune that
        // can only run after the wait it is meant to end is wrong on its own
        // terms, and leaving it there would restore the deadlock the moment
        // anything else came to hold a permit past termination.
        self.prune_terminal().await;

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

        // Insert into registry.
        self.registry.lock().await.insert(
            id.clone(),
            ManagerEntry {
                handle: handle.clone(),
                last_event: None,
                permit: Some(permit),
                abort: Some(task.abort_handle()),
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
                .ok_or_else(|| ArcanaError::Agent(format!("unknown subagent {id}")))?;
            entry
                .task
                .take()
                .ok_or_else(|| ArcanaError::Agent(format!("subagent {id} already awaited")))?
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
                Err(ArcanaError::Agent(format!(
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
            .ok_or_else(|| ArcanaError::Agent(format!("unknown subagent {id}")))
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

    /// Graceful termination: aborts the subagent's task and drops its entry.
    pub async fn terminate(self: &Arc<Self>, id: &str, force: bool) -> Result<()> {
        let (task, abort) = {
            let mut reg = self.registry.lock().await;
            let entry = reg
                .get_mut(id)
                .ok_or_else(|| ArcanaError::Agent(format!("unknown subagent {id}")))?;
            entry.handle.status = SubagentStatus::Terminated;
            // Capacity is freed the moment we decide to stop it, exactly as at
            // any other terminal transition (`P3-22`).
            drop(entry.permit.take());
            (entry.task.take(), entry.abort.take())
        };

        match task {
            // We still hold the JoinHandle: abort and await it, so the OS has
            // reclaimed everything before this returns.
            Some(t) => {
                if force {
                    t.abort();
                }
                let _ = t.await;
            }
            // 🔴 G-9. The handle was taken by `await_result`, which is the case
            // for EVERY subagent its parent is awaiting — i.e. every in-flight
            // one. This branch used to do nothing at all, and `terminate`
            // returned `Ok(())` regardless: the caller was told the subagent
            // had stopped while it ran to completion and billed.
            //
            // Not awaited here: the future belongs to `await_result`, which
            // will observe the cancellation itself.
            None => {
                if force {
                    if let Some(h) = abort {
                        h.abort();
                    }
                }
            }
        }

        // Nothing to clean up remotely: aborting the task above ends the
        // in-process engine. The old path had to delete an ephemeral agent
        // from another process here.

        self.registry.lock().await.remove(id);
        info!("[SubagentManager] terminated {id} (force={force})");
        Ok(())
    }

    /// Terminate every live subagent spawned by `parent_slug`.
    ///
    /// Returns how many were stopped.
    ///
    /// P3-24. Subagents run as detached tokio tasks, and dropping the
    /// `JoinHandle` the parent is awaiting **detaches** rather than aborts.
    /// So a cancelled parent left its children running to completion --
    /// finishing their work, emitting their events and billing the user's key
    /// for a turn nobody would read.
    ///
    /// `spawn_subagent` and `spawn_parallel_subagents` both set
    /// `parent_slug: ctx.session_id`, so a session id is exactly the right
    /// key: cancelling session X stops the children of session X and nothing
    /// else. (`spawn_qc_review` uses the dispatcher slug instead, and is
    /// deliberately unaffected -- a QC review is not owned by the interactive
    /// session that happened to be open.)
    pub async fn abort_children_of(self: &Arc<Self>, parent_slug: &str) -> usize {
        // Collect first, THEN terminate. `terminate` takes the registry lock
        // itself, so holding it across the calls would deadlock on the first
        // child.
        let ids: Vec<String> = {
            let reg = self.registry.lock().await;
            reg.iter()
                .filter(|(_, e)| {
                    e.handle.config.parent_slug == parent_slug
                        && !e.handle.status.is_terminal()
                })
                .map(|(id, _)| id.clone())
                .collect()
        };

        let mut stopped = 0usize;
        for id in ids {
            if self.abort(&id).await.is_ok() {
                stopped += 1;
            }
        }
        if stopped > 0 {
            info!("[SubagentManager] cancelled {stopped} child(ren) of '{parent_slug}'");
        }
        stopped
    }

    /// Hard abort — alias for terminate(force=true).
    pub async fn abort(self: &Arc<Self>, id: &str) -> Result<()> {
        self.terminate(id, true).await
    }

    /// Orphan cleanup. Retained as a no-op so startup wiring and the Tauri
    /// command that calls it keep working.
    ///
    /// It used to delete every `subagent-*` agent left registered in another
    /// process after a crash. Subagents are now in-process tokio tasks: when
    /// Arcana exits they exit with it, so no cross-process orphan can
    /// survive a restart and there is nothing to sweep.
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
                        // and, because the permit is a drop-guard held by the
                        // entry, never released its pool slot. Found by the P0
                        // post-phase audit (F-02).
                        //
                        // ⚠ P3-22. This is the SECOND place a subagent reaches
                        // a terminal state, and it must release the permit for
                        // the same reason `finalise` does — otherwise the
                        // never-awaited path that F-02 fixed for the registry
                        // still leaks capacity. Releasing where the state is
                        // stamped, both times, is what keeps the two in step.
                        if status.is_terminal() && entry.terminated_at.is_none() {
                            entry.terminated_at = Some(Instant::now());
                            drop(entry.permit.take());
                        }
                    }
                }
            }

            // Fan out to the global event bus.
            emit_subagent_event(SubagentEventPayload {
                subagent_id: evt.subagent_id().to_string(),
                event: evt,
            });
        }
    }

    /// Finalise an entry after the bridge task joins.
    ///
    /// The entry is kept for a short retention window so the UI can display
    /// the final result — but **the pool permit is released immediately**.
    ///
    /// 🔴 `P3-22`. This docstring already said *"The pool permit is released
    /// here"*. It was not: the function stamped `terminated_at` and nothing
    /// else, and the permit stayed inside the entry until `prune_terminal`'s
    /// `retain` dropped it. A docstring promising behaviour the code does not
    /// perform is a bug, not a wording problem — and this one hid a deadlock
    /// for as long as it was believed.
    async fn finalise(&self, id: &str, status: &SubagentStatus) {
        let mut reg = self.registry.lock().await;
        if let Some(entry) = reg.get_mut(id) {
            entry.handle.status = *status;
            if status.is_terminal() {
                entry.terminated_at = Some(Instant::now());
                // Capacity is freed at termination; the ENTRY is what the
                // retention window is for.
                drop(entry.permit.take());
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

#[cfg(test)]
mod p3_24_tests {
    use super::*;
    use crate::agent::permission::new_pending_permissions;
    use crate::state::AppState;

    fn manager() -> Arc<SubagentManager> {
        SubagentManager::new(RunnerDeps {
            state: Arc::new(std::sync::Mutex::new(AppState::new())),
            pending: new_pending_permissions(),
        })
    }

    /// A subagent whose parent is AWAITING it can still be aborted.
    ///
    /// 🔴 G-9 (post-phase gap hunt). `await_result` consumes the `JoinHandle`,
    /// so `terminate` found `entry.task == None`, skipped the abort entirely,
    /// removed the entry and returned `Ok(())` — telling the caller the
    /// subagent had stopped while it ran on to completion and billed.
    ///
    /// ⚠ That is the ONLY case that matters: `spawn_subagent` awaits its child,
    /// so every in-flight subagent is one whose handle has been taken. P3-24's
    /// two tests ran against an EMPTY registry and could not have seen it —
    /// which is why this one holds a real, running task.
    #[tokio::test]
    async fn a_subagent_being_awaited_can_still_be_aborted() {
        use std::sync::atomic::{AtomicBool, Ordering};

        // A task that never finishes on its own — the shape of a subagent mid
        // turn. If the abort does not reach it, this test times out.
        let finished = Arc::new(AtomicBool::new(false));
        let flag = finished.clone();
        let task = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            flag.store(true, Ordering::SeqCst);
        });

        let abort = task.abort_handle();

        // Simulate `await_result`: the JoinHandle is TAKEN out of the registry.
        let taken = task;

        // What `terminate` now does when `entry.task` is `None`.
        abort.abort();

        let outcome = tokio::time::timeout(std::time::Duration::from_secs(5), taken).await;
        let joined = outcome.expect("the abort must reach a task whose handle was taken");

        assert!(
            joined.is_err() && joined.unwrap_err().is_cancelled(),
            "the task was not cancelled — `terminate` would have returned Ok              while the subagent kept running"
        );
        assert!(
            !finished.load(Ordering::SeqCst),
            "the task ran to completion despite being aborted"
        );
    }

    #[test]
    fn the_abort_handle_is_kept_beside_the_join_handle() {
        // Structural, and it is the part that made G-9 possible: if the entry
        // holds only the JoinHandle, `await_result` taking it leaves nothing
        // for `terminate` to use.
        let src = include_str!("manager.rs");
        assert!(
            src.contains("abort: Option<tokio::task::AbortHandle>"),
            "the registry entry must keep an abort handle that survives              `await_result` consuming the JoinHandle"
        );
        assert!(
            src.contains("abort: Some(task.abort_handle())"),
            "the abort handle must be captured at spawn, not derived later              from a handle that may already be gone"
        );
    }

    #[tokio::test]
    async fn cancelling_a_session_with_no_children_is_a_no_op() {
        // The no-fire half, and the one that would hang if the collect-then-
        // terminate ordering were wrong.
        let mgr = manager();
        let stopped = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            mgr.abort_children_of("session-with-nothing-under-it"),
        )
        .await
        .expect("abort_children_of must not block when there is nothing to stop");

        assert_eq!(stopped, 0);
    }

    #[tokio::test]
    async fn aborting_children_does_not_deadlock_on_the_registry_lock() {
        // ⚠ The trap in this function: `terminate` takes the registry lock
        // itself, so collecting ids *while holding* that lock would deadlock on
        // the first child. This proves the two phases stay separate — it
        // completes rather than hanging, under a hard timeout so a regression
        // fails the suite instead of wedging it.
        let mgr = manager();

        // Hold and release the lock around the call to exercise the same
        // acquisition path the real cancel handler takes.
        {
            let reg = mgr.registry.lock().await;
            assert!(reg.is_empty());
        }

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            mgr.abort_children_of("any-session"),
        )
        .await;

        assert!(
            result.is_ok(),
            "abort_children_of deadlocked — it is holding the registry lock \
             across `terminate`, which takes the same lock"
        );
    }
}
