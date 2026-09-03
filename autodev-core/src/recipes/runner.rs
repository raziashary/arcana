// RecipeRunner — executes a single recipe as a background agent session.
// Called by RecipeScheduler on each cron tick for autonomous worker agents.
//
// **One engine.** A tick resolves a provider, builds an `AgentEngine`, and runs
// the recipe's instructions on it, in this process. There is no dispatch
// decision to make and no second runtime to be healthy.
//
// ⚠ This header used to describe a two-engine architecture with a routing
// priority between them, and it survived the removal of the second engine by
// several phases — so the first thing anyone read about this file was a
// dispatch rule it had stopped implementing. The history is in
// `ADR-LOG-RESTRUCT-0008` and `P8-48`; it does not belong at the top of the
// file that no longer does it (`P8-60`).

use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::agent::engine::AgentEngine;
use crate::agent::permission::{PendingPermissions, PermissionMode};
use crate::recipes::types::RecipePermissionMode;
use crate::agent::PAMode;
use crate::db::workers::WorkersDb;
use crate::errors::{AutoDevError, Result};
use crate::events::{
    emit_worker_log_line, emit_worker_status, emit_worker_tick_end, emit_worker_tick_start,
    WorkerLogLinePayload, WorkerStatusPayload, WorkerTickEndPayload, WorkerTickStartPayload,
};
use crate::hooks::ecosystem::Ecosystem;
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::google::GoogleProvider;
use crate::provider::openai::OpenAIProvider;
use crate::provider::traits::Provider;
use crate::recipes::types::Recipe;
use crate::session::manager::SessionManager;
use crate::state::AppState;
use crate::vault::keychain::{provider_key, VaultManager};
use crate::state::lock::LockTolerant;

/// A recipe that is scheduled AND asks for prompts, described.
///
/// Returns `Some(reason)` when the combination cannot work, `None` otherwise.
///
/// Split out of `run_recipe` so it can be tested without standing up an
/// `AppState`, a `SessionManager` and a provider — an inline check that needs
/// the whole runtime to exercise is an untested check.
pub(crate) fn unanswerable_prompt_config(recipe: &Recipe) -> Option<String> {
    if recipe.schedule.is_none() {
        return None;
    }
    if !matches!(recipe.permission_mode, RecipePermissionMode::Supervised) {
        return None;
    }
    Some(format!(
        "Recipe '{}' is scheduled ({}) and also set to 'supervised'. A scheduled \
         run has nobody present to answer a permission prompt, so it would stall \
         for {}s and then be denied. Set permission_mode to 'autonomous', or \
         remove the schedule and trigger it manually.",
        recipe.id,
        recipe.schedule.as_deref().unwrap_or("?"),
        crate::agent::tool_executor::PERMISSION_TIMEOUT.as_secs(),
    ))
}

/// Initial trigger message sent to the worker agent at each cron execution.
const RECIPE_TRIGGER: &str =
    "Begin your scheduled execution. Check the task board and proceed with your responsibilities.";

// ─── RecipeRunner ─────────────────────────────────────────────────────────────

/// Executes a recipe as an autonomous agent session.
/// Each call creates a fresh ephemeral session (not stored in session DB).
pub struct RecipeRunner {
    state: Arc<Mutex<AppState>>,
    _session_mgr: Arc<SessionManager>, // retained for future SSE bridging (not attached to recipe sessions)
    pending: PendingPermissions,
}

impl RecipeRunner {
    pub fn new(
        state: Arc<Mutex<AppState>>,
        session_mgr: Arc<SessionManager>,
        pending: PendingPermissions,
    ) -> Self {
        Self {
            state,
            _session_mgr: session_mgr,
            pending,
        }
    }


    /// Execute a recipe for the given worker slug.
    /// Updates worker status in DB: idle → working → idle/error.
    pub async fn run_recipe(&self, recipe: &Recipe, worker_slug: &str) -> Result<()> {
        // ── P3-65 · a schedule and a prompt are mutually exclusive ──────────
        //
        // `TG-P3-65` holds this for the nine shipped recipes, but users author
        // their own: `RecipeEditor.tsx` exposes `permission_mode` as a
        // dropdown alongside the schedule field, so the invalid combination is
        // two clicks away and no source gate can reach it.
        //
        // ⚠ Refusing is the only honest option here. Silently downgrading to
        // Autonomous would run the recipe unprompted after the user asked for
        // prompts — the exact defect P3-65 fixes, reintroduced as a
        // "convenience". Honouring it would block every Execute/Network/Board
        // tool for the full 300s `PERMISSION_TIMEOUT` with nobody there to
        // answer, then deny. Both silently do something other than what was
        // asked; this says so instead.
        if let Some(msg) = unanswerable_prompt_config(recipe) {
            warn!("[RecipeRunner] {msg}");
            return Err(AutoDevError::Agent(msg));
        }

        // ⚠ A start-up latch and a health probe stood here. The latch held
        // every tick until a second runtime answered a probe, so on a machine
        // that did not have one the latch never opened and **every worker tick
        // returned early reporting "idle"** — a fleet that was dead and said it
        // was resting (P0 audit `F-10`). The probe that replaced it cost a
        // round-trip per tick to fill a variable nothing read (`P8-48`).
        //
        // Both are gone. A tick now starts by doing the work.
        info!(
            "[RecipeRunner] tick dispatch — worker='{}' (native engine)",
            worker_slug
        );

        // ── Deterministic no-op short-circuit ──────────────────────────────────
        //
        // For "worker-*" slugs (backend / frontend / platform / mcp / cleanup),
        // the recipe is meaningful ONLY when there is an actual task on the
        // board assigned to this worker in a runnable state
        // (`assigned` or `in_progress`). Historically the LLM was spun up on
        // every cron tick regardless and — finding an empty board — would
        // hallucinate work, drift through random files, and churn until the
        // hard `MAX_TOOL_TURNS` cap (20). That produced the "18 loops for a
        // simple markdown" behaviour seen in `logs/demo.txt`.
        //
        // ⚠ This was once conditional on which engine the tick would take.
        // The condition was always true (`P8-48`), so it has only ever meant
        // what it now says: every `worker-*` slug, every tick.
        if worker_slug.starts_with("worker-") {
            if !Self::has_runnable_task(worker_slug) {
                info!(
                    "[RecipeRunner] worker '{}' has no assigned/in_progress tasks — skipping tick",
                    worker_slug
                );
                self.set_worker_status(worker_slug, "idle");
                return Ok(());
            }
        }

        // ── Model resolution ───────────────────────────────────────────────
        // Priority: worker.model (DB) > recipe.model (YAML) > active_model > default.
        let worker_model: Option<String> = WorkersDb::open()
            .ok()
            .and_then(|db| db.get_by_slug(worker_slug).ok().flatten())
            .map(|w| w.model)
            .filter(|m| !m.is_empty());

        let model = {
            let s = self.state.lock_tolerant();
            worker_model
                .or_else(|| recipe.model.clone())
                .unwrap_or_else(|| {
                    // P2-31. The literal here was `claude-sonnet-4-5`, retired
                    // in February 2026 — so a run with no worker model, no
                    // recipe model and no active selection reached for a model
                    // that does not exist.
                    //
                    // `DEFAULT_MODEL` is the one place a fallback model id is
                    // written down, and `TG-P2-09` holds every such id to the
                    // catalogue.
                    s.active_model
                        .clone()
                        .unwrap_or_else(|| crate::db::workers::DEFAULT_MODEL.to_string())
                })
        };

        // Ephemeral session id — used by Mode 1 if reached
        let session_id = format!("recipe-{}-{}", worker_slug, Uuid::new_v4());

        // Working directory for recipe agents.
        //
        // Priority:
        //   1. Configured workspace dir (set via Settings → Workspace Folder).
        //      This is the SAME root the Personal Assistant uses, so the worker
        //      fleet and the PA share a consistent view of the project.
        //   2. Fallback sandbox ~/.autodev/agents/<slug>/ when no workspace is set,
        //      so background ticks can still run on a fresh install without
        //      polluting a random cwd.
        let workspace_dir = crate::db::app_settings::load_workspace_dir();
        let working_dir = if !workspace_dir.trim().is_empty() {
            workspace_dir
        } else {
            crate::db::autodev_dir()
                .map(|p| {
                    let agent_dir = p.join("agents").join(worker_slug);
                    let _ = std::fs::create_dir_all(&agent_dir);
                    agent_dir.to_string_lossy().into_owned()
                })
                .unwrap_or_else(|_| {
                    std::env::var("USERPROFILE")
                        .or_else(|_| std::env::var("HOME"))
                        .unwrap_or_else(|_| ".".to_string())
                })
        };

        // ⚠ Two dead branches stood here and were deleted in `P8-48` and
        // `P8-47`: a dispatch arm into a second runtime, unreachable by
        // construction since `P0`, and a policy guard that skipped any recipe
        // carrying the serde DEFAULT policy — so the shipped recipes never ran
        // and the fleet reported itself **idle rather than broken**.
        //
        // The first also held every `worker:tick-start` / `worker:tick-end`
        // emitter in this file, which is why the Live Monitor showed nothing at
        // all rather than something stale. `P8-42` moved them onto the live
        // path in the same change.

        // ── The engine ──────────────────────────────────────────────────
        // A provider and key are required: without one there is nothing to run
        // the recipe on.
        let provider_id = {
            let s = self.state.lock_tolerant();
            s.active_provider.clone()
        };

        let provider_id = match provider_id {
            Some(id) => id,
            None => {
                // No LLM provider configured — there is nothing to run on.
                // 🔴 `P8-43`. This branch used to say the worker would stay
                // idle until a second runtime finished starting and picked the
                // recipe up, and logged that it was waiting for one.
                // **Nothing is coming.** There is one engine, and a worker with
                // no provider configured is not waiting for a peer — it is
                // blocked until the user adds a key. Saying otherwise turned a
                // fixable configuration gap into a wait with no end.
                //
                // Still not `"error"`: the user has simply not finished setting
                // up, which is a state to report, not a failure to log.
                info!(
                    "[RecipeRunner] Worker '{}' idle: no LLM provider configured. \
                     Add a provider key in Settings — nothing else will pick this up.",
                    worker_slug
                );
                self.set_worker_status(worker_slug, "idle_no_provider");
                return Ok(());
            }
        };
        // ⚠ The `mode` / `mode1_capable` fields are gone with the second
        // engine: every worker runs here, so a field that was true for all of
        // them carried no information and a `grep` for it selected everything.
        info!(
            provider = provider_id.as_str(),
            "[RecipeRunner] Worker '{}' active (provider='{}')",
            worker_slug,
            provider_id,
        );

        let api_key = VaultManager::get(&provider_key(&provider_id))?.ok_or_else(|| {
            AutoDevError::Agent(format!(
                "No API key stored for provider '{provider_id}' — set it in Settings"
            ))
        })?;

        let inner: Arc<dyn Provider> = match provider_id.as_str() {
            "anthropic" => Arc::new(AnthropicProvider::new(api_key)),
            "openai" => Arc::new(OpenAIProvider::new(api_key)),
            "google" => Arc::new(GoogleProvider::new(api_key)),
            other => {
                return Err(AutoDevError::Agent(format!(
                    "Unsupported provider: {other}"
                )))
            }
        };

        // Every worker call passes the daily ceiling (P2-21).
        let provider =
            crate::provider::capped::CappedProvider::wrap(inner, self.state.clone());

        self.set_worker_status(worker_slug, "working");
        info!(
            "[RecipeRunner] Starting Rust AgentEngine for '{}' (session: {})",
            worker_slug, session_id
        );

        // ⚠ P3-65. This comment used to read "Build autonomous engine — workers
        // never wait for user approval", and the line below hardcoded
        // `PermissionMode::Autonomous`. That was true of every recipe
        // regardless of what it declared, which is why four recipes could ask
        // for `Supervised` and get none of it.
        //
        // A recipe on a cron schedule still must not prompt — there is nobody
        // there to answer, and the 300s `PERMISSION_TIMEOUT` would stall the
        // fleet. That constraint is now enforced where it belongs, on the
        // *recipe declarations* (and gated by `TG-P3-65`), rather than by
        // ignoring the field here.
        //
        // NOTE: session_mgr is intentionally NOT attached. Recipe sessions are ephemeral
        // (session_id never inserted into sessions table), so append_message would fail
        // with FOREIGN KEY constraint. In-memory context is sufficient for recipe runs.
        let engine = AgentEngine::new(
            provider,
            PermissionMode::from(&recipe.permission_mode),
            PAMode::FastDevelopment,
            self.state.clone(),
            self.pending.clone(),
            // The worker's own slug is its identity. This is what the three
            // worker-messaging tools, claim_task's orphan detection and the
            // qc_review gate all read — previously it was None on this path.
            crate::agent::tool_trait::AgentIdentity::worker(worker_slug),
        )
        // P2-48. A worker runs unattended, so it runs under a ceiling. Until
        // 2026-09-02 nothing ever called this and the guard in `engine.run`
        // was unreachable — a worker could loop indefinitely and spend without
        // limit. The task's own budget, when it claims one, refines this; the
        // default is what stops an unclaimed or unbudgeted run.
        .with_budget(crate::budget::TaskBudget::default_worker());

        // P8-42 — the Live Monitor's tick events, emitted around the NATIVE run.
        //
        // 🔴 Every `worker:tick-start` / `worker:tick-end` emitter used to live
        // inside a branch that `P0` made unreachable. So the Live Monitor
        // showed nothing for every worker from then on — not stale data, **no
        // data** — and deleting that branch without moving these would have
        // turned an accident into a structural impossibility. This is the half
        // that keeps the capability.
        //
        // ⚠ `tick_duration_ms` is measured from an `Instant` taken here, not
        // computed from the two timestamps: the timestamps are RFC3339 strings
        // for the UI, and deriving a duration from them would inherit
        // `P8-31`'s parsing problem.
        let tick_started = std::time::Instant::now();
        emit_worker_tick_start(WorkerTickStartPayload {
            slug: worker_slug.to_string(),
            // One engine left, so one mode. The field stays because the
            // payload is a published event shape; `agentLiveStore.ts` types it
            // and does not read it.
            mode: "native".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });

        // Run agent with recipe instructions as system prompt override
        let result = engine
            .run(
                &session_id,
                &working_dir,
                &model,
                vec![],
                RECIPE_TRIGGER,
                Some(&recipe.instructions),
            )
            .await;

        // P8-42 — one end event per tick, whatever the outcome. `last_outcome`
        // is what the Live Monitor renders, so it carries the real result
        // rather than a constant: a monitor that always says "ok" is a reader
        // with no writer (`W-7`).
        emit_worker_tick_end(WorkerTickEndPayload {
            slug: worker_slug.to_string(),
            outcome: match &result {
                Ok(_) => "completed".to_string(),
                // ⚠ The prefix is load-bearing. `agentLiveStore.ts:349` maps a
                // tick to the error state with `outcome.startsWith('error')`,
                // so "failed: ..." would have rendered every failed tick as
                // IDLE -- a green monitor over a broken worker, which is the
                // exact failure this task exists to end.
                Err(e) => format!("error: {e}"),
            },
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
        let _tick_duration_ms = tick_started.elapsed().as_millis() as u64;

        // Update worker status based on outcome
        match &result {
            Ok(_) => {
                info!(
                    "[RecipeRunner] Rust AgentEngine completed for worker '{}'",
                    worker_slug
                );
                self.set_worker_status(worker_slug, "idle");
                // Phase 7.5 — Record successful Mode 1 tick in analytics DB.
                // Token usage is already recorded inside engine.rs per-session;
                // record the worker-level completion so dashboards can show task counts.
                let _ = crate::analytics::global().collector().record_worker_stats(
                    worker_slug.to_string(),
                    1,   // tasks_completed
                    0,   // tasks_failed
                    0,   // time_spent_minutes — not yet timed
                    0.0, // cost — recorded per-token inside engine.rs
                    0,   // avg_task_duration_minutes
                );
            }
            Err(e) => {
                error!(
                    "[RecipeRunner] Rust AgentEngine failed for worker '{}': {e}",
                    worker_slug
                );
                self.set_worker_status(worker_slug, "error");
                // Phase 7.5 — Record failed Mode 1 tick in analytics DB.
                let _ = crate::analytics::global().collector().record_worker_stats(
                    worker_slug.to_string(),
                    0, // tasks_completed
                    1, // tasks_failed
                    0,
                    0.0,
                    0,
                );
            }
        }

        // ── Post-session reconciliation ────────────────────────────────────────
        //
        // If a session ends (naturally, cancelled, or at MAX_TOOL_TURNS) with a
        // task still in `in_progress` for this worker, that means the worker
        // did NOT reach an explicit terminal state (qc_review / failed / done).
        // Left alone, subsequent ticks re-enter the same in_progress task with
        // a fresh session and the LLM redoes work it already completed —
        // observed at 2026-04-20T13:03:41 where worker-backend wrote the file
        // at turn 5 but dawdled on bash verifications until hitting max_turns.
        //
        // Auto-promote stranded in_progress tasks to `qc_review`. QC will then
        // verify whether the work product exists. If it does → mark done.
        // If not → bounce back to `assigned` for a retry. Either way the task
        // escapes the in_progress limbo and the board keeps moving.
        //
        // Run reconciliation BEFORE the sentinel guard so a stranded task that
        // completed but forgot to update status is promoted to qc_review
        // instead of being incorrectly marked build_failed.
        if worker_slug.starts_with("worker-") {
            Self::reconcile_stranded_task(worker_slug);
        }

        // Fix 8c: sentinel-file guard after successful tick.
        // Only apply when the engine actually completed (not on error).
        if result.is_ok() {
            Self::guard_and_validate(worker_slug, std::path::Path::new(&working_dir));
        }

        result.map(|_| ())
    }

    /// If a task remains `in_progress` for this worker after the session
    /// ends, promote it to `qc_review` so QC can verify work state. See
    /// comment at the call site for motivation.
    fn reconcile_stranded_task(worker_slug: &str) {
        let db = match crate::db::tasks::TasksDb::open() {
            Ok(db) => db,
            Err(_) => return,
        };
        let tasks = match db.list(None) {
            Ok(v) => v,
            Err(_) => return,
        };
        for t in tasks {
            let is_mine = t.assigned_worker.as_deref() == Some(worker_slug);
            let is_in_progress = matches!(t.status, crate::db::tasks::TaskStatus::InProgress);
            if is_mine && is_in_progress {
                match db.update_status(t.id, "qc_review", Some(worker_slug)) {
                    Ok(()) => {
                        info!(
                            "[RecipeRunner] Reconciled stranded in_progress task #{} → qc_review \
                             (worker '{}' session ended without explicit completion)",
                            t.id, worker_slug
                        );
                    }
                    Err(e) => {
                        warn!(
                            "[RecipeRunner] Failed to reconcile stranded task #{}: {e}",
                            t.id
                        );
                    }
                }
            }
        }
    }

    /// Returns true when the local task board has at least one task assigned to
    /// `worker_slug` in a runnable state (`assigned` or `in_progress`).
    ///
    /// Used by `run_recipe` to skip LLM invocation on empty ticks so workers
    /// can't fabricate work when there is literally nothing queued. Reads
    /// `~/.autodev/tasks.db` via `TasksDb`; any DB failure is treated as
    /// "no work" (fail-safe: never spin up the LLM when we can't confirm work
    /// exists).
    fn has_runnable_task(worker_slug: &str) -> bool {
        let db = match crate::db::tasks::TasksDb::open() {
            Ok(db) => db,
            Err(e) => {
                warn!("[RecipeRunner] tasks.db open failed ({e}); treating as no-work");
                return false;
            }
        };
        let all = match db.list(None) {
            Ok(v) => v,
            Err(e) => {
                warn!("[RecipeRunner] tasks.db list failed ({e}); treating as no-work");
                return false;
            }
        };
        all.iter().any(|t| {
            t.assigned_worker.as_deref() == Some(worker_slug)
                && matches!(
                    t.status,
                    crate::db::tasks::TaskStatus::Assigned
                        | crate::db::tasks::TaskStatus::InProgress
                )
        })
    }

    fn set_worker_status(&self, slug: &str, status: &str) {
        self.set_worker_status_ex(slug, status, None, None);
    }

    fn set_worker_status_ex(
        &self,
        slug: &str,
        status: &str,
        tick_duration_ms: Option<u64>,
        last_outcome: Option<String>,
    ) {
        let previous = if let Ok(db) = WorkersDb::open() {
            if let Ok(Some(worker)) = db.get_by_slug(slug) {
                let prev = worker.status.clone();
                let _ = db.update_status(worker.id, status);
                Some(prev)
            } else {
                None
            }
        } else {
            None
        };
        // Phase 3.0 R2 — push the single source-of-truth transition to the UI.
        // The Agent Roster subscribes via `workerStore.initEventListeners()`
        // and mutates local state in place, eliminating the derived-status
        // flicker where rows went `idle → error → idle` between ticks.
        // Fix 9a: expanded payload with previous_status, reason, cooldown_seconds.
        let reason: Option<String> = if status == "error" {
            Some("Invoke failed — see logs for details".into())
        } else {
            None
        };
        let cooldown_seconds: Option<u32> = if status == "error" { Some(30) } else { None };
        emit_worker_status(WorkerStatusPayload {
            slug: slug.to_string(),
            status: status.to_string(),
            previous_status: previous,
            reason,
            cooldown_seconds,
            timestamp: chrono::Utc::now().to_rfc3339(),
            tick_duration_ms,
            last_outcome,
        });
    }

    /// Fix 8c — Sentinel-file guard.
    /// After a worker tick succeeds, verify the generated project has its
    /// dependencies installed by checking the ecosystem sentinel file.
    /// If sentinel is missing, update the active in_progress task to
    /// `build_failed` so QC can inspect it.
    fn guard_and_validate(worker_slug: &str, working_dir: &std::path::Path) {
        use crate::db::tasks::{TaskStatus, TasksDb};
        use std::fs;

        // Only guard for workers (not supervisor, etc.)
        if !worker_slug.starts_with("worker-") {
            return;
        }

        // Find active task for this worker
        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(_) => return,
        };
        let tasks = match db.list(None) {
            Ok(v) => v,
            Err(_) => return,
        };
        let active_task = tasks.into_iter().find(|t| {
            t.assigned_worker.as_deref() == Some(worker_slug)
                && matches!(t.status, TaskStatus::InProgress)
        });
        let Some(task) = active_task else { return };

        // Detect ecosystem
        let Some(ecosystem) = Ecosystem::detect(working_dir) else {
            return;
        };
        let sentinel: std::path::PathBuf = working_dir.join(ecosystem.sentinel_path());

        if sentinel.exists() {
            info!(
                "[RecipeRunner] Sentinel found for '{}' ({:?}) — build guard passed",
                worker_slug, sentinel
            );
            return;
        }

        // Sentinel missing → mark build_failed
        warn!(
            "[RecipeRunner] Sentinel missing for '{}' ({:?}) — marking task #{} build_failed",
            worker_slug, sentinel, task.id
        );

        // Write a guard log note in the workspace
        let guard_log = working_dir.join("build_guard.log");
        let note = format!(
            "[AutoDev Build Guard] {:?} dependencies not installed.\nExpected sentinel: {:?}\nRun '{}' and '{}' in this directory, then retry.\n",
            ecosystem,
            sentinel,
            ecosystem.install_cmd(),
            ecosystem.validate_cmd(),
        );
        let _ = fs::write(&guard_log, note);

        let _ = db.update_status(task.id, "build_failed", Some(worker_slug));
    }
}

#[cfg(test)]
mod p3_65_runner_tests {
    use super::*;
    use crate::recipes::types::RecipePermissionMode;

    fn recipe(schedule: Option<&str>, mode: RecipePermissionMode) -> Recipe {
        Recipe {
            version: "1.0.0".into(),
            id: "test-recipe".into(),
            title: "Test".into(),
            description: None,
            schedule: schedule.map(String::from),
            permission_mode: mode,
            instructions: "do the thing".into(),
            agent_role: None,
            model: None,
            is_system: false,
        }
    }

    #[test]
    fn a_scheduled_supervised_recipe_is_refused_with_a_reason() {
        // `TG-P3-65` covers the nine SHIPPED recipes. This covers the ones
        // users author: `RecipeEditor.tsx` puts `permission_mode` and
        // `schedule` on the same form, so the invalid pair is two clicks away
        // and no source gate can reach it.
        let msg = unanswerable_prompt_config(&recipe(Some("*/1 * * * *"), RecipePermissionMode::Supervised))
            .expect("a scheduled supervised recipe cannot work and must say so");

        assert!(msg.contains("test-recipe"), "name the recipe: {msg}");
        assert!(msg.contains("*/1 * * * *"), "name the schedule: {msg}");
        assert!(
            msg.contains("autonomous"),
            "an error the user cannot act on is half an error: {msg}"
        );
    }

    #[test]
    fn a_manual_supervised_recipe_is_exactly_what_supervision_is_for() {
        // The no-fire half, and the whole point of wiring the field up: with
        // no schedule, a human triggered it and is present to answer.
        assert!(
            unanswerable_prompt_config(&recipe(None, RecipePermissionMode::Supervised)).is_none(),
            "a manually-triggered supervised recipe must be allowed — refusing \
             it would leave the control as dead as it was before"
        );
    }

    #[test]
    fn a_scheduled_autonomous_recipe_is_the_normal_fleet_case() {
        assert!(
            unanswerable_prompt_config(&recipe(Some("*/5 * * * *"), RecipePermissionMode::Autonomous))
                .is_none(),
            "the entire fleet runs this way; refusing it would stop everything"
        );
    }
}
