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

use std::panic::{catch_unwind, AssertUnwindSafe};
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::Instrument;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::agent::engine::AgentEngine;
use crate::agent::permission::{PendingPermissions, PermissionMode};
use crate::agent::PAMode;
use crate::db::workers::{WorkerStatus, WorkersDb};
use crate::errors::{ArcanaError, Result};
use crate::events::{
    emit_worker_status, emit_worker_tick_end, emit_worker_tick_start, WorkerStatusPayload,
    WorkerTickEndPayload, WorkerTickStartPayload,
};
use crate::hooks::ecosystem::Ecosystem;
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::google::GoogleProvider;
use crate::provider::openai::OpenAIProvider;
use crate::provider::traits::Provider;
use crate::recipes::types::Recipe;
use crate::recipes::types::RecipePermissionMode;
use crate::session::manager::SessionManager;
use crate::state::lock::LockTolerant;
use crate::state::AppState;
use crate::vault::keychain::{provider_key, VaultManager};

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

const STARTUP_MAX_ATTEMPTS: u32 = 4;
const STARTUP_RETRY_BASE_MS: u64 = 200;
const STARTUP_RETRY_MAX_DELAY: Duration = Duration::from_millis(2_200);
// Admission window for another setup attempt, not an interrupting deadline
// for synchronous SQLite/keychain work. Every setup overrun is reported.
const STARTUP_RETRY_ADMISSION_WINDOW: Duration = Duration::from_secs(2);
const SUPERVISOR_OUTPUT_TOKEN_LIMIT: u32 = 6_144;

fn worker_run_budget(
    worker_slug: &str,
    supervisor_override: Option<&crate::budget::TaskBudget>,
) -> crate::budget::TaskBudget {
    if worker_slug == "supervisor" {
        supervisor_override
            .cloned()
            .unwrap_or_else(crate::budget::TaskBudget::default_supervisor)
    } else {
        crate::budget::TaskBudget::default_worker()
    }
}

fn worker_output_token_limit(worker_slug: &str) -> Option<u32> {
    (worker_slug == "supervisor").then_some(SUPERVISOR_OUTPUT_TOKEN_LIMIT)
}

fn startup_retry_delay(retry_number: u32, jitter_seed: u128) -> Duration {
    let exponent = retry_number.saturating_sub(1).min(63);
    let window_ms = STARTUP_RETRY_BASE_MS
        .saturating_mul(1u64 << exponent)
        .min(STARTUP_RETRY_MAX_DELAY.as_millis() as u64);
    let delay_ms = (jitter_seed % u128::from(window_ms.saturating_add(1))) as u64;
    Duration::from_millis(delay_ms)
}

fn retryable_startup_error(error: &ArcanaError) -> bool {
    match error {
        ArcanaError::Io(error) => matches!(
            error.kind(),
            std::io::ErrorKind::Interrupted
                | std::io::ErrorKind::WouldBlock
                | std::io::ErrorKind::TimedOut
        ),
        ArcanaError::Database(rusqlite::Error::SqliteFailure(error, _)) => matches!(
            error.code,
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
        ),
        // Vault errors currently use a string-only error variant, so the
        // runner cannot distinguish transient keychain service failures from
        // corrupt or undecryptable persistent data. Fail once until that
        // boundary exposes typed transient errors.
        ArcanaError::Vault(_) => false,
        _ => false,
    }
}

fn startup_error_class(error: &ArcanaError) -> &'static str {
    match error {
        ArcanaError::Io(_) => "io",
        ArcanaError::Database(_) => "database",
        ArcanaError::Vault(_) => "keychain",
        ArcanaError::Provider(message) if message.contains("[QuotaExhausted]") => {
            "provider_quota_exhausted"
        }
        ArcanaError::Provider(message) if message.contains("[Overloaded]") => "provider_overloaded",
        ArcanaError::Provider(message) if message.contains("[Retryable]") => "provider_retryable",
        ArcanaError::Provider(_) => "provider_fatal",
        ArcanaError::Cancelled => "cancelled",
        _ => "permanent",
    }
}

#[cfg(test)]
mod provider_failure_class_tests {
    use super::startup_error_class;
    use crate::errors::ArcanaError;

    #[test]
    fn provider_failure_records_preserve_the_provider_retry_class() {
        assert_eq!(
            startup_error_class(&ArcanaError::Provider(
                "Google 503 [Overloaded]: busy".into()
            )),
            "provider_overloaded"
        );
        assert_eq!(
            startup_error_class(&ArcanaError::Provider(
                "Google 429 [QuotaExhausted]: capped".into()
            )),
            "provider_quota_exhausted"
        );
        assert_eq!(
            startup_error_class(&ArcanaError::Provider(
                "Google 500 [Retryable]: unavailable".into()
            )),
            "provider_retryable"
        );
        assert_eq!(
            startup_error_class(&ArcanaError::Provider("Google 400 [Fatal]: invalid".into())),
            "provider_fatal"
        );
    }
}

#[cfg(test)]
async fn retry_startup<T, F>(worker_slug: &str, run_id: &str, mut prepare: F) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    retry_startup_with_state(worker_slug, run_id, &mut prepare, |_| {}).await
}

async fn retry_startup_with_state<T, F, R>(
    worker_slug: &str,
    run_id: &str,
    prepare: F,
    on_retry: R,
) -> Result<T>
where
    F: FnMut() -> Result<T>,
    R: FnMut(u32),
{
    retry_startup_with_state_and_delay(worker_slug, run_id, prepare, on_retry, |attempt| {
        startup_retry_delay(attempt, Uuid::new_v4().as_u128())
    })
    .await
}

async fn retry_startup_with_state_and_delay<T, F, R, D>(
    worker_slug: &str,
    run_id: &str,
    mut prepare: F,
    mut on_retry: R,
    mut delay_for_attempt: D,
) -> Result<T>
where
    F: FnMut() -> Result<T>,
    R: FnMut(u32),
    D: FnMut(u32) -> Duration,
{
    let started = Instant::now();
    for attempt in 1..=STARTUP_MAX_ATTEMPTS {
        let result = catch_unwind(AssertUnwindSafe(|| prepare()));
        let elapsed = started.elapsed();
        if elapsed > STARTUP_RETRY_ADMISSION_WINDOW {
            tracing::warn!(
                target: "arcana::telemetry",
                event_type = "worker_startup_retry",
                worker = %worker_slug,
                run_id = %run_id,
                attempt,
                max_attempts = STARTUP_MAX_ATTEMPTS,
                elapsed_ms = elapsed.as_millis() as u64,
                admission_window_ms = STARTUP_RETRY_ADMISSION_WINDOW.as_millis() as u64,
                setup_succeeded = matches!(&result, Ok(Ok(_))),
                outcome = "setup_overrun",
                "Synchronous worker setup exceeded the retry admission window"
            );
        }
        let (value, error, retryable, error_class) = match result {
            Ok(Ok(value)) => (Some(value), None, false, "none"),
            Ok(Err(error)) => {
                let retryable = retryable_startup_error(&error);
                let class = startup_error_class(&error);
                (None, Some(error), retryable, class)
            }
            Err(_) => (
                None,
                Some(ArcanaError::Agent("worker startup panicked".into())),
                true,
                "panic",
            ),
        };

        if let Some(value) = value {
            if attempt > 1 {
                tracing::info!(
                    target: "arcana::telemetry",
                    event_type = "worker_startup_retry",
                    worker = %worker_slug,
                    run_id = %run_id,
                    attempt,
                    max_attempts = STARTUP_MAX_ATTEMPTS,
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    outcome = "recovered",
                    "Worker startup recovered before provider work"
                );
            }
            return Ok(value);
        }

        let error = error.expect("failed startup attempt has an error");
        let delay = delay_for_attempt(attempt);
        let elapsed = started.elapsed();
        if !retryable
            || attempt == STARTUP_MAX_ATTEMPTS
            || elapsed.saturating_add(delay) >= STARTUP_RETRY_ADMISSION_WINDOW
        {
            tracing::warn!(
                target: "arcana::telemetry",
                event_type = "worker_startup_retry",
                worker = %worker_slug,
                run_id = %run_id,
                attempt,
                max_attempts = STARTUP_MAX_ATTEMPTS,
                error_class,
                outcome = if retryable { "exhausted" } else { "permanent_failure" },
                elapsed_ms = elapsed.as_millis() as u64,
                "Worker startup stopped before provider work"
            );
            return Err(error);
        }

        tracing::warn!(
            target: "arcana::telemetry",
            event_type = "worker_startup_retry",
            worker = %worker_slug,
            run_id = %run_id,
            attempt,
            max_attempts = STARTUP_MAX_ATTEMPTS,
            next_delay_ms = delay.as_millis() as u64,
            error_class,
            outcome = "retry_scheduled",
            "Retrying worker setup before provider work"
        );
        on_retry(attempt);
        tokio::time::sleep(delay).await;
        // A scheduled delay may resume late. Recheck before admitting setup,
        // preserving the original error and never starting a late attempt.
        if started.elapsed() >= STARTUP_RETRY_ADMISSION_WINDOW {
            tracing::warn!(
                target: "arcana::telemetry",
                event_type = "worker_startup_retry",
                worker = %worker_slug,
                run_id = %run_id,
                attempt,
                max_attempts = STARTUP_MAX_ATTEMPTS,
                error_class,
                outcome = "exhausted",
                elapsed_ms = started.elapsed().as_millis() as u64,
                "Worker retry admission expired during backoff"
            );
            return Err(error);
        }
    }
    unreachable!("bounded startup attempts always return")
}

/// Select a worker/recipe model only when it belongs to the active provider.
/// Seeded worker rows can outlive a provider switch, so an incompatible saved
/// id must not be sent to the newly selected provider.
fn model_for_provider(
    provider: Option<&str>,
    worker_model: Option<&str>,
    recipe_model: Option<&str>,
    active_model: Option<&str>,
) -> Option<String> {
    let Some(provider) = provider else {
        return worker_model
            .or(recipe_model)
            .or(active_model)
            .map(str::to_owned);
    };
    [worker_model, recipe_model, active_model]
        .into_iter()
        .flatten()
        .find(|model| {
            crate::provider::catalogue::entry(model).is_some_and(|entry| entry.provider == provider)
        })
        .map(str::to_owned)
        .or_else(|| crate::provider::catalogue::default_model_for(provider).map(str::to_owned))
}

// ─── RecipeRunner ─────────────────────────────────────────────────────────────

/// Executes a recipe as an autonomous agent session.
/// Each call creates a fresh ephemeral session (not stored in session DB).
pub struct RecipeRunner {
    state: Arc<Mutex<AppState>>,
    _session_mgr: Arc<SessionManager>, // retained for future SSE bridging (not attached to recipe sessions)
    pending: PendingPermissions,
    workspace_override: Option<String>,
    supervisor_budget_override: Option<crate::budget::TaskBudget>,
    fleet_correlation_id: Option<String>,
    host_approval_available: bool,
    #[cfg(any(test, feature = "offline-fleet-test"))]
    provider_override: Option<Arc<dyn Provider>>,
    #[cfg(test)]
    startup_failures_for_test: Arc<AtomicUsize>,
}

impl RecipeRunner {
    pub fn new(
        state: Arc<Mutex<AppState>>,
        session_mgr: Arc<SessionManager>,
        pending: PendingPermissions,
    ) -> Self {
        Self::new_with_workspace_override(state, session_mgr, pending, None)
    }

    /// Construct a runner with a host-selected, already validated workspace.
    /// The override is process-local and leaves the user's configured
    /// workspace untouched.
    pub fn new_with_workspace_override(
        state: Arc<Mutex<AppState>>,
        session_mgr: Arc<SessionManager>,
        pending: PendingPermissions,
        workspace_override: Option<String>,
    ) -> Self {
        Self {
            state,
            _session_mgr: session_mgr,
            pending,
            workspace_override,
            supervisor_budget_override: None,
            fleet_correlation_id: None,
            host_approval_available: true,
            #[cfg(any(test, feature = "offline-fleet-test"))]
            provider_override: None,
            #[cfg(test)]
            startup_failures_for_test: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Replace provider construction in production-path tests while leaving
    /// scheduler dispatch, recipe resolution, AgentEngine, and tools intact.
    #[cfg(test)]
    pub(crate) fn with_provider_for_test(mut self, provider: Arc<dyn Provider>) -> Self {
        self.provider_override = Some(provider);
        self
    }

    /// Supply the deterministic provider for the opt-in arcanactl integration build.
    #[cfg(feature = "offline-fleet-test")]
    pub fn with_provider_for_offline_fleet_test(mut self, provider: Arc<dyn Provider>) -> Self {
        self.provider_override = Some(provider);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_startup_failures_for_test(mut self, count: usize) -> Self {
        self.startup_failures_for_test = Arc::new(AtomicUsize::new(count));
        self
    }

    /// Apply a bounded host-scoped override to each Supervisor run. Worker
    /// caps remain unchanged; any saved task budget is still intersected by
    /// the engine after a task is claimed.
    pub fn with_supervisor_budget_override(
        mut self,
        budget: Option<crate::budget::TaskBudget>,
    ) -> Self {
        self.supervisor_budget_override = budget;
        self
    }

    /// Correlate every production worker tick with the host's Fleet run.
    pub fn with_fleet_correlation_id(mut self, correlation_id: Option<String>) -> Self {
        self.fleet_correlation_id = correlation_id;
        self
    }

    /// Set whether this host can answer one-call approval prompts.
    pub fn with_host_approval_available(mut self, available: bool) -> Self {
        self.host_approval_available = available;
        self
    }

    /// Execute a recipe for the given worker slug.
    /// Updates worker status in DB: idle → working → idle/error.
    #[tracing::instrument(
        target = "arcana::telemetry",
        skip_all,
        fields(
            event_type = "trace_span",
            operation = "fleet_worker_tick",
            fleet_correlation_id = self.fleet_correlation_id.as_deref().unwrap_or(""),
            worker = %worker_slug
        )
    )]
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
            return Err(ArcanaError::Agent(msg));
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
        // Every persistent worker is provider-gated by actual board work. This
        // includes the Supervisor's READY/request queues and QC's inbox/review
        // queue, so neither can spend on an empty startup, event, or recovery
        // tick. A failed DB read is an error, never evidence that the queue is
        // empty.
        if !crate::recipes::scheduler::worker_has_recoverable_work(worker_slug)? {
            info!(
                "[RecipeRunner] worker '{}' has no recoverable task or message work — skipping provider turn",
                worker_slug
            );
            self.set_worker_status(worker_slug, WorkerStatus::Idle);
            return Ok(());
        }

        // ── Model resolution ───────────────────────────────────────────────
        // Priority among provider-compatible ids: worker.model (DB) >
        // recipe.model (YAML) > active_model > that provider's catalogue default.
        let worker_model: Option<String> = WorkersDb::open()
            .ok()
            .and_then(|db| db.get_by_slug(worker_slug).ok().flatten())
            .map(|w| w.model)
            .filter(|m| !m.is_empty());

        let (active_provider, active_model) = {
            let s = self.state.lock_tolerant();
            (s.active_provider.clone(), s.active_model.clone())
        };
        let model = model_for_provider(
            active_provider.as_deref(),
            worker_model.as_deref(),
            recipe.model.as_deref(),
            active_model.as_deref(),
        )
        .unwrap_or_else(|| crate::db::workers::DEFAULT_MODEL.to_string());

        // Ephemeral session id — used by Mode 1 if reached
        let session_id = format!("recipe-{}-{}", worker_slug, Uuid::new_v4());
        let runs_db = crate::db::runs::RunsDb::open()?;
        let worker_run_id = runs_db.begin_worker_tick(worker_slug, &session_id)?;
        // Track the native run under its worker/session pair for its entire
        // lifetime. This does not retain the engine or persist the session.
        let _runtime_lease =
            crate::state::runtime_registry::global().admit(worker_slug, &session_id)?;

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

        // Prepare all local startup state under one bounded retry boundary.
        // Workspace validation stays before the no-provider return, preserving
        // the fail-closed worker contract. A retry can only happen before
        // `engine.run`, so it cannot replay a provider request, tool call, task
        // claim, or other run side effect.
        let retry_run_id = worker_run_id.clone();
        let retry_worker_slug = worker_slug.to_owned();
        let prepared = retry_startup_with_state(worker_slug, &worker_run_id, || {
            #[cfg(test)]
            if self
                .startup_failures_for_test
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok()
            {
                return Err(ArcanaError::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "injected transient startup failure before provider work",
                )));
            }

            // Every agent runs against the same canonical configured project.
            // Missing, unreadable, relative, or invalid workspace configuration
            // fails the tick instead of redirecting work to CWD or an Arcana folder.
            let working_dir = crate::platform::workspace::resolve_workspace_dir(
                self.workspace_override.as_deref(),
            )?
                .to_string_lossy()
                .into_owned();

            let Some(provider_id) = active_provider.as_deref() else {
                return Ok(None);
            };

            let inner: Arc<dyn Provider> = {
                #[cfg(any(test, feature = "offline-fleet-test"))]
                if let Some(provider) = &self.provider_override {
                    provider.clone()
                } else {
                    let api_key = VaultManager::get(&provider_key(provider_id))?.ok_or_else(|| {
                        ArcanaError::Agent(format!(
                            "No API key stored for provider '{provider_id}' — set it in Settings"
                        ))
                    })?;
                    match provider_id {
                        "anthropic" => Arc::new(AnthropicProvider::new(api_key)),
                        "openai" => Arc::new(OpenAIProvider::new(api_key)),
                        "google" => Arc::new(GoogleProvider::new(api_key)),
                        other => {
                            return Err(ArcanaError::Agent(format!("Unsupported provider: {other}")))
                        }
                    }
                }
                #[cfg(not(any(test, feature = "offline-fleet-test")))]
                {
                    let api_key = VaultManager::get(&provider_key(provider_id))?.ok_or_else(|| {
                        ArcanaError::Agent(format!(
                            "No API key stored for provider '{provider_id}' — set it in Settings"
                        ))
                    })?;
                    match provider_id {
                        "anthropic" => Arc::new(AnthropicProvider::new(api_key)),
                        "openai" => Arc::new(OpenAIProvider::new(api_key)),
                        "google" => Arc::new(GoogleProvider::new(api_key)),
                        other => {
                            return Err(ArcanaError::Agent(format!("Unsupported provider: {other}")))
                        }
                    }
                }
            };

            // Every worker call passes the daily ceiling (P2-21).
            let provider = crate::provider::capped::CappedProvider::wrap(inner, self.state.clone());

            // NOTE: session_mgr is intentionally NOT attached. Recipe sessions
            // are ephemeral and are not inserted into the sessions table.
            let engine = AgentEngine::new(
                provider,
                PermissionMode::from(&recipe.permission_mode),
                PAMode::FastDevelopment,
                self.state.clone(),
                self.pending.clone(),
                crate::agent::tool_trait::AgentIdentity::worker(worker_slug),
            )
            .with_workspace_egress_consent()
            .with_host_approval_available(self.host_approval_available)
            .with_procedural_recall(true)
            .with_fleet_correlation_id(self.fleet_correlation_id.clone())
            // P2-48: unattended runs always have a worker-level spending cap.
                .with_budget(worker_run_budget(
                    worker_slug,
                    self.supervisor_budget_override.as_ref(),
                ));
            let engine = match worker_output_token_limit(worker_slug) {
                Some(limit) => engine.with_output_token_limit(limit),
                None => engine,
            };

            Ok(Some((engine, working_dir, provider_id.to_owned())))
        }, |attempt| {
            let update = crate::db::runs::RunsDb::open().and_then(|db| {
                db.update_worker_tick_phase(&retry_run_id, "retrying", None, None)
            });
            if let Err(error) = update {
                warn!(target: "arcana::telemetry", event_type = "worker_startup_retry_state_failed", worker = %retry_worker_slug, run_id = %retry_run_id, attempt, error_class = "runs_db", "Could not persist startup retry phase");
                let _ = error;
            }
        })
        .await;

        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                let class = startup_error_class(&error);
                let _ = runs_db.finish_worker_tick(
                    &worker_run_id,
                    "failed",
                    crate::db::runs::outcome::FAILED,
                    Some(class),
                );
                self.set_worker_status(worker_slug, WorkerStatus::Error);
                return Err(error);
            }
        };

        let Some((engine, working_dir, provider_id)) = prepared else {
            // No LLM provider configured — there is nothing to run on.
            // `P8-43`: report the blocked configuration state rather than
            // claiming another runtime will eventually pick the recipe up.
            info!(
                "[RecipeRunner] Worker '{}' idle: no LLM provider configured. \
                 Add a provider key in Settings — nothing else will pick this up.",
                worker_slug
            );
            runs_db.finish_worker_tick(
                &worker_run_id,
                "blocked",
                crate::db::runs::outcome::OK,
                Some("provider_not_configured"),
            )?;
            self.set_worker_status(worker_slug, WorkerStatus::IdleNoProvider);
            return Ok(());
        };

        runs_db.update_worker_tick_phase(&worker_run_id, "running", None, None)?;
        let engine = engine.with_worker_run_id(worker_run_id.clone());

        // ⚠ The `mode` / `mode1_capable` fields are gone with the second
        // engine: every worker runs here, so a field that was true for all of
        // them carried no information and a `grep` for it selected everything.
        info!(
            provider = provider_id.as_str(),
            "[RecipeRunner] Worker '{}' active (provider='{}')", worker_slug, provider_id,
        );

        self.set_worker_status(worker_slug, WorkerStatus::Working);
        info!(
            "[RecipeRunner] Starting Rust AgentEngine for '{}' (session: {})",
            worker_slug, session_id
        );

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
            timestamp: crate::db::timestamps::now_rfc3339(),
        });

        // Run agent with recipe instructions as system prompt override
        let fleet_span = tracing::info_span!(
            target: "arcana::telemetry",
            "fleet_worker_tick",
            event_type = "trace_span",
            worker = %worker_slug,
            session_id = %session_id,
            worker_id = %worker_slug,
        );
        let engine_run = engine
            .run(
                &session_id,
                &working_dir,
                &model,
                vec![],
                RECIPE_TRIGGER,
                Some(&recipe.instructions),
            )
            .instrument(fleet_span);
        tokio::pin!(engine_run);
        let mut heartbeat = tokio::time::interval_at(
            tokio::time::Instant::now() + Duration::from_secs(15),
            Duration::from_secs(15),
        );
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let result = loop {
            tokio::select! {
                result = &mut engine_run => break result,
                _ = heartbeat.tick() => {
                    if let Err(error) = WorkersDb::open().and_then(|db| db.heartbeat(worker_slug)) {
                        warn!(target: "arcana::telemetry", event_type = "worker_heartbeat_failed", worker = %worker_slug, error_class = "workers_db", "Could not refresh worker heartbeat");
                        let _ = error;
                    }
                    if let Err(error) = runs_db.heartbeat_worker_tick(&worker_run_id) {
                        warn!(target: "arcana::telemetry", event_type = "worker_heartbeat_failed", worker = %worker_slug, error_class = "runs_db", "Could not refresh durable run heartbeat");
                        let _ = error;
                    }
                }
            }
        };

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
            timestamp: crate::db::timestamps::now_rfc3339(),
        });
        let _tick_duration_ms = tick_started.elapsed().as_millis() as u64;
        info!(
            target: "arcana::fleet",
            worker = %worker_slug,
            session_id = %session_id,
            elapsed_ms = _tick_duration_ms,
            outcome = if result.is_ok() { "completed" } else { "error" },
            "worker recipe tick finished"
        );
        tracing::info!(
            target: "arcana::telemetry",
            event_type = "fleet_worker_tick",
            worker = %worker_slug,
            session_id = %session_id,
            duration_ns = tick_started.elapsed().as_nanos() as u64,
            outcome = if result.is_ok() { "completed" } else { "error" },
            "Fleet worker tick completed"
        );

        // Update worker status based on outcome
        match &result {
            Ok(_) => {
                info!(
                    "[RecipeRunner] Rust AgentEngine completed for worker '{}'",
                    worker_slug
                );
                self.set_worker_status(worker_slug, WorkerStatus::Idle);
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
                if matches!(&e, ArcanaError::Cancelled) {
                    info!(
                        "[RecipeRunner] Worker '{}' run was cancelled; preserving task state",
                        worker_slug
                    );
                    self.set_worker_status_ex(
                        worker_slug,
                        WorkerStatus::Paused,
                        None,
                        Some("cancelled".into()),
                    );
                    // Cancellation is a terminal run outcome, not an ordinary
                    // failed tick that should be retried or promoted to QC.
                    // Leave claim ownership untouched here; the scheduler's
                    // run guard will move the exact interrupted claim to
                    // needs_info, where side effects must be reconciled before
                    // a retry is considered.
                } else {
                    error!(
                        "[RecipeRunner] Rust AgentEngine failed for worker '{}': {e}",
                        worker_slug
                    );
                    self.set_worker_status(worker_slug, WorkerStatus::Error);
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
        if worker_slug.starts_with("worker-") && result.is_ok() {
            Self::reconcile_stranded_task(worker_slug, std::path::Path::new(&working_dir));
        }

        // Fix 8c: sentinel-file guard after successful tick.
        // Only apply when the engine actually completed (not on error).
        if result.is_ok() {
            Self::guard_and_validate(worker_slug, std::path::Path::new(&working_dir));
        }

        let (phase, outcome, failure_code) = match &result {
            Ok(_) => ("completed", crate::db::runs::outcome::OK, None),
            Err(ArcanaError::Cancelled) => (
                "cancelled",
                crate::db::runs::outcome::CANCELLED,
                Some("cancelled"),
            ),
            Err(error) => (
                "failed",
                crate::db::runs::outcome::FAILED,
                Some(startup_error_class(error)),
            ),
        };
        if let Err(error) = runs_db.finish_worker_tick(&worker_run_id, phase, outcome, failure_code)
        {
            warn!(target: "arcana::telemetry", event_type = "worker_run_close_failed", worker = %worker_slug, error_class = "runs_db", "Could not close durable worker run");
            let _ = error;
        }

        result.map(|_| ())
    }

    /// If a task remains `in_progress` for this worker after the session
    /// ends, promote it to `qc_review` so QC can verify work state. See
    /// comment at the call site for motivation.
    fn reconcile_stranded_task(worker_slug: &str, workspace_path: &std::path::Path) {
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
                match db.update_status_with_workspace_and_run_id(
                    t.id,
                    "qc_review",
                    Some(worker_slug),
                    None,
                    None,
                    workspace_path,
                ) {
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

    fn set_worker_status(&self, slug: &str, status: WorkerStatus) {
        self.set_worker_status_ex(slug, status, None, None);
    }

    fn set_worker_status_ex(
        &self,
        slug: &str,
        status: WorkerStatus,
        tick_duration_ms: Option<u64>,
        last_outcome: Option<String>,
    ) {
        let previous = if let Ok(db) = WorkersDb::open() {
            if let Ok(Some(worker)) = db.get_by_slug(slug) {
                let prev = worker.status.as_str().to_string();
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
        let reason: Option<String> = if status == WorkerStatus::Error {
            Some("Invoke failed — see logs for details".into())
        } else {
            None
        };
        let cooldown_seconds: Option<u32> = if status == WorkerStatus::Error {
            Some(30)
        } else {
            None
        };
        emit_worker_status(WorkerStatusPayload {
            slug: slug.to_string(),
            status: status.as_str().to_string(),
            previous_status: previous,
            reason,
            cooldown_seconds,
            timestamp: crate::db::timestamps::now_rfc3339(),
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
            "[Arcana Build Guard] {:?} dependencies not installed.\nExpected sentinel: {:?}\nRun '{}' and '{}' in this directory, then retry.\n",
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
        let msg = unanswerable_prompt_config(&recipe(
            Some("*/1 * * * *"),
            RecipePermissionMode::Supervised,
        ))
        .expect("a scheduled supervised recipe cannot work and must say so");

        assert!(msg.contains("test-recipe"), "name the recipe: {msg}");
        assert!(msg.contains("*/1 * * * *"), "name the schedule: {msg}");
        assert!(
            msg.contains("autonomous"),
            "an error the user cannot act on is half an error: {msg}"
        );
    }

    #[tokio::test]
    async fn empty_supervisor_and_qc_queues_exit_before_provider_resolution() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workers = WorkersDb::open().expect("seed isolated worker roster");
        let runner = RecipeRunner::new(
            Arc::new(Mutex::new(AppState::default())),
            Arc::new(SessionManager::new().expect("open isolated session DB")),
            crate::agent::permission::new_pending_permissions(),
        );

        for slug in ["supervisor", "qc-agent"] {
            runner
                .run_recipe(
                    &recipe(Some("*/5 * * * *"), RecipePermissionMode::Autonomous),
                    slug,
                )
                .await
                .expect("empty queue should be an idle turn");
            assert_eq!(
                workers.get_by_slug(slug).unwrap().unwrap().status,
                WorkerStatus::Idle,
                "{slug} should skip model resolution without work"
            );
        }
    }

    #[test]
    fn stranded_task_reconciliation_cannot_bypass_qc_build_evidence() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let db = crate::db::tasks::TasksDb::open().expect("open isolated task DB");
        let task = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "stranded task QC evidence gate".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "reconciliation cannot skip build evidence".into(),
                created_by: Some("tg4-07-test".into()),
            })
            .expect("create task");
        db.update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign task");
        db.claim_task_with_pid(task.id, "worker-backend", 991)
            .expect("claim task");

        let workspace = tempfile::tempdir().expect("create selected workspace");
        let workspace_path = workspace
            .path()
            .canonicalize()
            .expect("canonical workspace");
        RecipeRunner::reconcile_stranded_task("worker-backend", &workspace_path);
        assert_eq!(
            db.get_by_id(task.id).unwrap().unwrap().status,
            crate::db::tasks::TaskStatus::InProgress,
            "reconciliation must leave a task in progress if no build evidence exists"
        );

        let fingerprint = crate::platform::workspace_fingerprint::fingerprint(&workspace_path)
            .expect("fingerprint selected workspace");
        let batch = db
            .begin_project_build_batch_for_worker(
                "worker-backend",
                "test-session",
                1,
                &workspace_path.to_string_lossy(),
                Some(&fingerprint),
            )
            .expect("begin selected workspace batch")
            .expect("claimed task batch");
        db.finish_project_build_batch_for_worker(
            batch.id,
            "worker-backend",
            "test-session",
            &[(
                "cargo test".into(),
                0,
                "cargo".into(),
                Some(workspace_path.to_string_lossy().into_owned()),
                Some(fingerprint),
            )],
            None,
            true,
        )
        .expect("record complete passing build batch");
        RecipeRunner::reconcile_stranded_task("worker-backend", &workspace_path);
        assert_eq!(
            db.get_by_id(task.id).unwrap().unwrap().status,
            crate::db::tasks::TaskStatus::QcReview,
            "the same production reconciliation path may promote after a passing build"
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
            unanswerable_prompt_config(&recipe(
                Some("*/5 * * * *"),
                RecipePermissionMode::Autonomous
            ))
            .is_none(),
            "the entire fleet runs this way; refusing it would stop everything"
        );
    }

    #[test]
    fn recipe_model_resolution_never_sends_a_saved_foreign_model_to_the_provider() {
        assert_eq!(
            model_for_provider(
                Some("google"),
                Some("claude-sonnet-5"),
                None,
                Some("gemini-3.7-flash"),
            ),
            Some("gemini-3.7-flash".into())
        );
        assert_eq!(
            model_for_provider(Some("google"), None, None, None),
            crate::provider::catalogue::default_model_for("google").map(str::to_owned)
        );
        assert_eq!(
            model_for_provider(
                Some("google"),
                Some("claude-sonnet-5"),
                Some("claude-haiku-4-5"),
                None,
            ),
            crate::provider::catalogue::default_model_for("google").map(str::to_owned)
        );
    }

    #[tokio::test]
    async fn worker_run_refuses_to_fall_back_when_workspace_is_unconfigured() {
        let home = std::env::temp_dir().join(format!("arcana-runner-workspace-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&home).expect("create isolated Arcana home");
        let _home = crate::db::test_support::with_home(&home);
        let workers = WorkersDb::open().expect("open isolated worker DB");
        assert!(workers
            .registered_slugs()
            .unwrap()
            .iter()
            .any(|slug| slug == "worker-backend"));
        let tasks = crate::db::tasks::TasksDb::open().expect("open isolated task DB");
        let task_id = tasks
            .create(crate::db::tasks::CreateTaskInput {
                title: "workspace resolver worker fixture".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "worker must require the configured workspace".into(),
                created_by: Some("tg4-09-test".into()),
            })
            .unwrap()
            .id;
        tasks
            .update_status(task_id, "assigned", Some("worker-backend"))
            .expect("assign task so worker dispatch reaches its workspace resolver");

        let runner = RecipeRunner::new(
            Arc::new(Mutex::new(AppState::default())),
            Arc::new(SessionManager::new().expect("open isolated session DB")),
            crate::agent::permission::new_pending_permissions(),
        );
        let error = runner
            .run_recipe(
                &recipe(Some("*/5 * * * *"), RecipePermissionMode::Autonomous),
                "worker-backend",
            )
            .await
            .expect_err("worker must refuse to run without a selected project");
        assert!(
            error
                .to_string()
                .contains("Workspace folder is not configured"),
            "report the missing project rather than falling back: {error}"
        );
        assert_eq!(
            tasks.get_by_id(task_id).unwrap().unwrap().status,
            crate::db::tasks::TaskStatus::Assigned,
            "workspace resolution failure must not claim or mutate the task"
        );
        drop(runner);
        drop(tasks);
        drop(workers);
        drop(_home);
        std::fs::remove_dir_all(home).expect("remove isolated Arcana home");
    }
}

#[cfg(test)]
mod production_git_commit_recipe_tests {
    use super::*;
    use crate::db::tasks::{CreateTaskInput, TasksDb};
    use crate::provider::traits::{Provider, StreamResult};
    use crate::provider::types::{
        CompletionRequest, CompletionResponse, MessageContent, ModelInfo, ProviderStatus,
        StreamChunk,
    };
    use crate::provider::usage::Usage;
    use async_trait::async_trait;
    use futures::stream;
    use git2::{Repository, Signature};
    use serde_json::{json, Value};
    use std::sync::Mutex;

    struct CommitPathProvider {
        task_id: i64,
        fail_after_commit: bool,
        turn: Mutex<usize>,
        requests: Mutex<Vec<CompletionRequest>>,
    }

    impl CommitPathProvider {
        fn tool_call(id: &str, name: &str, input: Value) -> Vec<StreamChunk> {
            vec![
                StreamChunk::ToolUseStart {
                    id: id.into(),
                    name: name.into(),
                    block_index: 0,
                    opaque: None,
                },
                StreamChunk::ToolInputDelta {
                    block_index: 0,
                    partial_json: input.to_string(),
                },
                StreamChunk::MessageDelta {
                    stop_reason: "tool_use".into(),
                    usage: Usage::default(),
                },
                StreamChunk::Done,
            ]
        }

        fn final_answer() -> Vec<StreamChunk> {
            vec![
                StreamChunk::TextDelta {
                    text: "The task change was committed.".into(),
                    block_index: 0,
                },
                StreamChunk::MessageDelta {
                    stop_reason: "end_turn".into(),
                    usage: Usage::default(),
                },
                StreamChunk::Done,
            ]
        }
    }

    #[async_trait]
    impl Provider for CommitPathProvider {
        fn id(&self) -> &str {
            "google"
        }

        fn name(&self) -> &str {
            "Production git commit path fixture"
        }

        fn models(&self) -> Vec<ModelInfo> {
            crate::provider::catalogue::models_for("google")
        }

        async fn input_token_upper_bound(
            &self,
            _request: &CompletionRequest,
        ) -> crate::errors::Result<u64> {
            Ok(10)
        }

        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> crate::errors::Result<CompletionResponse> {
            unreachable!("RecipeRunner must use AgentEngine streaming")
        }

        async fn stream(&self, request: CompletionRequest) -> crate::errors::Result<StreamResult> {
            assert!(request.tools.iter().any(|tool| tool.name == "claim_task"));
            assert!(request.tools.iter().any(|tool| tool.name == "git_commit"));
            self.requests
                .lock()
                .expect("provider request capture")
                .push(request.clone());

            let turn = {
                let mut turn = self.turn.lock().expect("provider turn lock");
                let current = *turn;
                *turn += 1;
                current
            };
            let chunks = match turn {
                0 => Self::tool_call("claim-task", "claim_task", json!({"task_id": self.task_id})),
                1 => {
                    let claim = request
                        .messages
                        .iter()
                        .flat_map(|message| &message.content)
                        .find_map(|content| match content {
                            MessageContent::ToolResult { name, is_error, .. }
                                if name == "claim_task" =>
                            {
                                Some(*is_error)
                            }
                            _ => None,
                        })
                        .expect("model turn must receive the production task-claim result");
                    assert!(
                        !claim,
                        "the assigned worker must successfully claim its task"
                    );
                    Self::tool_call(
                        "commit-change",
                        "git_commit",
                        json!({
                            "paths": ["change.txt"],
                            "message": "fix(core): commit through the worker host\n\nProve model-produced commits use the host-validated production path."
                        }),
                    )
                }
                2 => {
                    let commit = request
                        .messages
                        .iter()
                        .flat_map(|message| &message.content)
                        .find_map(|content| match content {
                            MessageContent::ToolResult { name, is_error, .. }
                                if name == "git_commit" =>
                            {
                                Some(*is_error)
                            }
                            _ => None,
                        })
                        .expect("model turn must receive the host git_commit result");
                    assert!(
                        !commit,
                        "the host commit tool should accept the safe named file"
                    );
                    if self.fail_after_commit {
                        return Err(ArcanaError::Agent(
                            "synthetic failure after committed effect".into(),
                        ));
                    }
                    Self::final_answer()
                }
                _ => panic!("unexpected provider request after scripted task completion"),
            };
            Ok(Box::pin(stream::iter(chunks.into_iter().map(Ok))))
        }

        async fn test_connection(&self) -> crate::errors::Result<ProviderStatus> {
            Ok(ProviderStatus::Connected)
        }
    }

    #[tokio::test]
    async fn model_produced_commit_runs_through_recipe_runner_and_host_guard() {
        run_commit_recipe(false).await;
    }

    #[tokio::test]
    async fn failure_after_host_commit_does_not_restart_or_replay_the_effect() {
        run_commit_recipe(true).await;
    }

    struct MultiMutationInterruptionProvider {
        task_id: i64,
        panic_after_lost_ack: bool,
        turn: Mutex<usize>,
        requests: Mutex<Vec<CompletionRequest>>,
    }

    impl MultiMutationInterruptionProvider {
        fn tool_call(id: &str, name: &str, input: Value) -> Vec<StreamChunk> {
            vec![
                StreamChunk::ToolUseStart {
                    id: id.into(),
                    name: name.into(),
                    block_index: 0,
                    opaque: None,
                },
                StreamChunk::ToolInputDelta {
                    block_index: 0,
                    partial_json: input.to_string(),
                },
                StreamChunk::MessageDelta {
                    stop_reason: "tool_use".into(),
                    usage: Usage::default(),
                },
                StreamChunk::Done,
            ]
        }
    }

    #[async_trait]
    impl Provider for MultiMutationInterruptionProvider {
        fn id(&self) -> &str {
            "google"
        }

        fn name(&self) -> &str {
            "Multi-mutation interruption fixture"
        }

        fn models(&self) -> Vec<ModelInfo> {
            crate::provider::catalogue::models_for("google")
        }

        async fn input_token_upper_bound(
            &self,
            _request: &CompletionRequest,
        ) -> crate::errors::Result<u64> {
            Ok(10)
        }

        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> crate::errors::Result<CompletionResponse> {
            unreachable!("RecipeRunner must use AgentEngine streaming")
        }

        async fn stream(&self, request: CompletionRequest) -> crate::errors::Result<StreamResult> {
            assert!(request.tools.iter().any(|tool| tool.name == "claim_task"));
            assert!(request.tools.iter().any(|tool| tool.name == "write_file"));
            self.requests
                .lock()
                .expect("provider request capture")
                .push(request.clone());
            let turn = {
                let mut turn = self.turn.lock().expect("provider turn lock");
                let current = *turn;
                *turn += 1;
                current
            };
            let chunks = match turn {
                0 => Self::tool_call(
                    "claim-task",
                    "claim_task",
                    json!({ "task_id": self.task_id }),
                ),
                1 => {
                    let claim = request
                        .messages
                        .iter()
                        .flat_map(|message| &message.content)
                        .find_map(|content| match content {
                            MessageContent::ToolResult { name, is_error, .. }
                                if name == "claim_task" =>
                            {
                                Some(*is_error)
                            }
                            _ => None,
                        })
                        .expect("worker must receive the real task claim result");
                    assert!(!claim, "assigned worker must claim its task");
                    Self::tool_call(
                        "write-acknowledged",
                        "write_file",
                        json!({
                            "path": "acknowledged.txt",
                            "content": "first real mutation\n"
                        }),
                    )
                }
                2 => {
                    let first_write = request
                        .messages
                        .iter()
                        .flat_map(|message| &message.content)
                        .find_map(|content| match content {
                            MessageContent::ToolResult { name, is_error, .. }
                                if name == "write_file" =>
                            {
                                Some(*is_error)
                            }
                            _ => None,
                        })
                        .expect("worker must receive the first write result");
                    assert!(
                        !first_write,
                        "first host file mutation must be acknowledged"
                    );
                    Self::tool_call(
                        "write-lost-ack",
                        "write_file",
                        json!({
                            "path": "lost-ack.txt",
                            "content": "second real mutation\n"
                        }),
                    )
                }
                3 => {
                    let second_write = request
                        .messages
                        .iter()
                        .flat_map(|message| &message.content)
                        .find_map(|content| match content {
                            MessageContent::ToolResult {
                                tool_use_id,
                                name,
                                content,
                                is_error,
                                ..
                            } if tool_use_id == "write-lost-ack" && name == "write_file" => {
                                Some((content.clone(), *is_error))
                            }
                            _ => None,
                        })
                        .expect("worker must receive the second write result");
                    assert!(
                        second_write.1,
                        "lost durable acknowledgement must be an error"
                    );
                    let result: Value =
                        serde_json::from_str(&second_write.0).expect("unknown result envelope");
                    assert_eq!(result["error"]["code"], "side_effect_outcome_unknown");
                    assert_eq!(result["error"]["retry_safe"], false);
                    if self.panic_after_lost_ack {
                        panic!("injected provider panic after lost mutation acknowledgement");
                    }
                    return Err(ArcanaError::Agent(
                        "scripted provider interruption after lost mutation acknowledgement".into(),
                    ));
                }
                4 => Self::tool_call(
                    "resume-claim",
                    "claim_task",
                    json!({"task_id":self.task_id}),
                ),
                5 => Self::tool_call(
                    "resume-instructions",
                    "get_task",
                    json!({"id":self.task_id}),
                ),
                6 => {
                    assert!(request.messages.iter().flat_map(|message| &message.content).any(|content|
                        matches!(content, MessageContent::ToolResult {name, content, is_error:false, ..}
                            if name == "get_task" && content.contains("continuation.txt"))),
                        "replacement reads persisted remaining-work instructions");
                    Self::tool_call(
                        "read-acknowledged",
                        "read_file",
                        json!({"path":"acknowledged.txt"}),
                    )
                }
                7 => Self::tool_call("read-verified", "read_file", json!({"path":"lost-ack.txt"})),
                8 => {
                    for expected in ["first real mutation", "second real mutation"] {
                        assert!(request.messages.iter().flat_map(|message| &message.content).any(|content|
                            matches!(content, MessageContent::ToolResult {name, content, is_error:false, ..}
                                if name == "read_file" && content.contains(expected))),
                            "replacement consumes persisted artifacts");
                    }
                    Self::tool_call(
                        "write-remaining",
                        "write_file",
                        json!({"path":"continuation.txt","content":"remaining work complete\n"}),
                    )
                }
                9 => Self::tool_call(
                    "verify-resumed-build",
                    "project_command",
                    json!({"intent":"build","toolchain":"cargo","timeout_secs":60}),
                ),
                10 => {
                    let build = request
                        .messages
                        .iter()
                        .flat_map(|message| &message.content)
                        .find_map(|content| match content {
                            MessageContent::ToolResult {
                                name,
                                content,
                                is_error,
                                ..
                            } if name == "project_command" => Some((content, is_error)),
                            _ => None,
                        })
                        .expect("replacement receives build result");
                    assert!(!*build.1, "replacement build failed: {}", build.0);
                    Self::tool_call(
                        "submit-resumed-qc",
                        "update_task_status",
                        json!({"id":self.task_id,"status":"qc_review"}),
                    )
                }
                11 => {
                    let qc = request
                        .messages
                        .iter()
                        .flat_map(|message| &message.content)
                        .find_map(|content| match content {
                            MessageContent::ToolResult {
                                name,
                                content,
                                is_error,
                                ..
                            } if name == "update_task_status" => Some((content, is_error)),
                            _ => None,
                        })
                        .expect("replacement receives QC result");
                    assert!(!*qc.1, "replacement QC refused: {}", qc.0);
                    vec![
                        StreamChunk::TextDelta {
                            text: "Reconciled artifacts preserved; remaining work build-verified."
                                .into(),
                            block_index: 0,
                        },
                        StreamChunk::MessageDelta {
                            stop_reason: "end_turn".into(),
                            usage: Usage::default(),
                        },
                        StreamChunk::Done,
                    ]
                }
                _ => panic!("unexpected provider request after resumed completion"),
            };
            Ok(Box::pin(stream::iter(chunks.into_iter().map(Ok))))
        }

        async fn test_connection(&self) -> crate::errors::Result<ProviderStatus> {
            Ok(ProviderStatus::Connected)
        }
    }

    async fn exercise_shipped_worker_lost_ack_continuation(panic_after_lost_ack: bool) {
        let expected_phase = if panic_after_lost_ack {
            "interrupted"
        } else {
            "failed"
        };
        let expected_outcome = if panic_after_lost_ack {
            "abandoned"
        } else {
            "failed"
        };
        let expected_worker_status = if panic_after_lost_ack {
            "idle"
        } else {
            "error"
        };
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let analytics = Arc::new(
            crate::analytics::AnalyticsManager::new()
                .await
                .expect("initialize isolated analytics DB"),
        );
        crate::analytics::install_global(analytics);
        let workspace = tempfile::tempdir().expect("isolated selected workspace");
        std::fs::create_dir(workspace.path().join("src")).unwrap();
        std::fs::write(workspace.path().join("Cargo.toml"),
            "[package]\nname = \"recovered-worker-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").unwrap();
        std::fs::write(
            workspace.path().join("src/lib.rs"),
            "pub fn ready() -> bool { true }\n",
        )
        .unwrap();
        let lock = std::process::Command::new("cargo")
            .current_dir(workspace.path())
            .env_remove("GOOGLE_API_KEY")
            .env_remove("GEMINI_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove("DATABASE_URL")
            .env_remove("DIRECT_URL")
            .args(["generate-lockfile", "--offline"])
            .output()
            .expect("generate isolated fixture lockfile offline");
        assert!(
            lock.status.success(),
            "fixture lockfile generation failed: {}",
            String::from_utf8_lossy(&lock.stderr)
        );
        crate::egress_consent::WorkspaceEgressConsentDb::open()
            .expect("open isolated workspace consent store")
            .grant(workspace.path(), "provider:google")
            .expect("grant scripted provider for isolated workspace");

        let tasks = TasksDb::open().expect("open isolated task database");
        let task_id = tasks
            .create(CreateTaskInput {
                title: "Preserve mutations across lost acknowledgement".into(),
                description: Some(
                    "Run two production write_file calls before provider interruption.".into(),
                ),
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "An unknown mutation remains quarantined. After host reconciliation, read both artifacts, write continuation.txt without replaying either write, build and submit QC.".into(),
                created_by: Some("tg4-27-test".into()),
            })
            .expect("create task")
            .id;
        tasks
            .update_status(task_id, "assigned", Some("worker-backend"))
            .expect("assign task to backend worker");
        let workers = crate::db::workers::WorkersDb::open().expect("seed worker roster");
        let backend = workers
            .get_by_slug("worker-backend")
            .expect("read backend worker")
            .expect("backend worker exists");
        workers
            .update_status(backend.id, WorkerStatus::Active)
            .expect("make worker runnable");

        // This isolated test-only trigger rejects only the second write_file
        // outcome row. The real ToolExecutor first inserts its intent,
        // then runs the registered builtin, then encounters this failed append.
        crate::db::runs::RunsDb::open().expect("initialize isolated run ledger");
        let runs_path = home.path().join(".arcana").join("runs.db");
        let trigger_db = rusqlite::Connection::open(&runs_path)
            .expect("open isolated runs.db for the test trigger");
        trigger_db
            .execute_batch(
                "CREATE TRIGGER test_fail_lost_ack_outcome
                 BEFORE INSERT ON operation_journal
                 WHEN NEW.record_type = 'outcome'
                   AND (
                     SELECT COUNT(*) FROM operation_journal
                     WHERE record_type = 'intent' AND operation = 'write_file'
                   ) = 2
                 BEGIN
                   SELECT RAISE(ABORT, 'test injected second mutation outcome failure');
                 END;",
            )
            .expect("install isolated outcome failure trigger");
        drop(trigger_db);

        let provider = Arc::new(MultiMutationInterruptionProvider {
            task_id,
            panic_after_lost_ack,
            turn: Mutex::new(0),
            requests: Mutex::new(Vec::new()),
        });
        let make_runner = || {
            RecipeRunner::new_with_workspace_override(
                Arc::new(Mutex::new(AppState {
                    active_provider: Some("google".into()),
                    active_model: Some("gemini-3.8-flash".into()),
                    ..AppState::default()
                })),
                Arc::new(SessionManager::new().expect("open isolated session manager")),
                crate::agent::permission::new_pending_permissions(),
                Some(workspace.path().to_string_lossy().into_owned()),
            )
            .with_provider_for_test(provider.clone())
            .with_startup_failures_for_test(1)
        };
        let recipes_dir = home.path().join(".arcana").join("recipes");
        crate::recipes::defaults::ensure_default_recipes(&recipes_dir)
            .expect("persist the shipped worker recipes");
        let recipe: Recipe = serde_yaml::from_str(
            &std::fs::read_to_string(recipes_dir.join("worker-backend-poll.yaml"))
                .expect("read shipped backend recipe"),
        )
        .expect("parse shipped backend recipe");
        assert_eq!(recipe.id, "worker-backend-poll");

        let scheduler = crate::recipes::scheduler::RecipeScheduler::new(Arc::new(make_runner()));
        scheduler.kick_worker("worker-backend");
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if crate::db::runs::RunsDb::open()
                    .expect("open worker run ledger")
                    .latest_worker_tick("worker-backend")
                    .expect("read worker tick")
                    .is_some_and(|run| run.phase == expected_phase)
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("provider interruption must finish the worker tick");
        scheduler.shutdown().await;

        let task = TasksDb::open()
            .expect("reopen isolated task database")
            .get_by_id(task_id)
            .expect("read interrupted task")
            .expect("task remains persisted");
        assert_eq!(task.status, crate::db::tasks::TaskStatus::NeedsInfo);
        assert_eq!(task.assigned_worker.as_deref(), Some("worker-backend"));
        assert!(task.claimed_by_pid.is_none() && task.claim_host.is_none());
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("acknowledged.txt"))
                .expect("reopen first file mutation"),
            "first real mutation\n"
        );
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("lost-ack.txt"))
                .expect("reopen second file mutation"),
            "second real mutation\n"
        );

        let checkpoints = crate::git::checkpoint::list(None, None, Some(task_id))
            .expect("read checkpoints through the public reader");
        assert_eq!(checkpoints.len(), 2, "each real write has a checkpoint");
        assert!(checkpoints
            .iter()
            .all(|checkpoint| checkpoint.task_id == Some(task_id)));
        assert!(checkpoints
            .iter()
            .any(|checkpoint| checkpoint.file_path.ends_with("acknowledged.txt")));
        assert!(checkpoints
            .iter()
            .any(|checkpoint| checkpoint.file_path.ends_with("lost-ack.txt")));
        let runs = crate::db::runs::RunsDb::open().expect("reopen isolated run ledger");
        let run = runs
            .latest_worker_tick("worker-backend")
            .expect("read persisted worker tick")
            .expect("worker run remains persisted");
        assert_eq!(run.task_id, Some(task_id));
        assert_eq!(run.phase, expected_phase);
        let journal = runs
            .operation_journal_page(&run.run_id, 0, 20)
            .expect("read task-attributed mutation journal");
        let writes = journal
            .entries
            .iter()
            .filter(|entry| entry.operation == "write_file")
            .collect::<Vec<_>>();
        assert_eq!(writes.len(), 2);
        assert!(writes[0].cursor < writes[1].cursor);
        assert!(writes.iter().all(|entry| {
            entry.task_id == Some(task_id) && entry.run_id == run.run_id && entry.effect == "Write"
        }));
        let acknowledged = writes[0];
        assert_eq!(acknowledged.target_paths, ["acknowledged.txt"]);
        assert_eq!(acknowledged.outcome, crate::db::runs::OperationOutcome::Ok);
        assert_eq!(acknowledged.changed_paths, ["acknowledged.txt"]);
        let unknown = writes[1];
        assert_eq!(unknown.target_paths, ["lost-ack.txt"]);
        assert_eq!(unknown.outcome, crate::db::runs::OperationOutcome::Unknown);
        assert_eq!(
            unknown.reconciliation,
            crate::db::runs::OperationReconciliation::Unresolved
        );
        assert!(unknown.changed_paths.is_empty());
        assert!(runs
            .unresolved_task_side_effect(task_id)
            .expect("read unresolved mutation barrier")
            .is_some());
        let session_checkpoints =
            crate::git::checkpoint::list(None, Some(&run.session_id), Some(task_id))
                .expect("read run-scoped checkpoints through the public reader");
        assert_eq!(session_checkpoints.len(), 2);
        let checkpoint_ids = session_checkpoints
            .iter()
            .map(|checkpoint| checkpoint.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(
            checkpoint_ids.len(),
            2,
            "checkpoint references are distinct"
        );
        assert!(session_checkpoints.iter().all(|checkpoint| {
            checkpoint.task_id == Some(task_id) && checkpoint.session_id == run.session_id
        }));

        let mut registry = crate::agent::tool_registry::ToolRegistry::new();
        crate::agent::builtin_tools::register_for_mode(
            &mut registry,
            crate::agent::PAMode::FastDevelopment,
        );
        let health_tool = registry
            .get("list_worker_health")
            .expect("FastDevelopment registers Supervisor health reader");
        let mut health_executor = crate::agent::tool_executor::ToolExecutor::new(
            crate::agent::permission::PermissionMode::Autonomous,
            crate::agent::permission::new_pending_permissions(),
            "p4-54-health-readback".into(),
            home.path().to_string_lossy().into_owned(),
            crate::agent::tool_trait::AgentIdentity::supervisor(),
        );
        let health_results = health_executor
            .execute_batch(vec![(
                health_tool,
                json!({ "journal_limit": 50 }),
                "interrupted-worker-health".into(),
            )])
            .await
            .expect("registered Supervisor health read returns a result");
        assert_eq!(health_results.len(), 1);
        let (health_call_id, health_result) = health_results
            .into_iter()
            .next()
            .expect("health read result");
        assert_eq!(health_call_id, "interrupted-worker-health");
        assert!(!health_result.is_error, "{}", health_result.output);
        let health: Value =
            serde_json::from_str(&health_result.output).expect("Supervisor health JSON");
        let backend_health = health["workers"]
            .as_array()
            .expect("Supervisor worker roster")
            .iter()
            .find(|worker| worker["slug"] == "worker-backend")
            .expect("Supervisor-visible backend worker");
        assert_eq!(backend_health["status"], expected_worker_status);
        let supervisor_run = &backend_health["run"];
        assert_eq!(supervisor_run["run_id"], run.run_id);
        assert_eq!(supervisor_run["phase"], expected_phase);
        assert_eq!(supervisor_run["task_id"], task_id);
        assert_eq!(supervisor_run["outcome"], expected_outcome);
        assert_eq!(
            supervisor_run["failure_code"].as_str(),
            run.failure_code.as_deref()
        );
        assert!(
            run.failure_code.is_some(),
            "failed run records its failure class"
        );
        assert_eq!(
            supervisor_run["heartbeat_at"].as_str(),
            Some(run.heartbeat_at.as_str())
        );

        let supervisor_task = &supervisor_run["task"];
        assert_eq!(supervisor_task["state"], "known");
        assert_eq!(supervisor_task["status"], "needs_info");
        assert_eq!(supervisor_task["assigned_worker"], "worker-backend");
        assert_eq!(supervisor_task["blocked_by"], json!(task.blocked_by));
        assert_eq!(
            task.status,
            crate::db::tasks::TaskStatus::NeedsInfo,
            "direct TasksDb read and Supervisor view agree"
        );

        assert_eq!(supervisor_run["active_operation"]["state"], "idle");
        let supervisor_journal = &supervisor_run["operation_journal"];
        assert_eq!(supervisor_journal["state"], "known");
        let supervisor_writes = supervisor_journal["entries"]
            .as_array()
            .expect("Supervisor-visible operation journal")
            .iter()
            .filter(|entry| entry["operation"] == "write_file")
            .collect::<Vec<_>>();
        assert_eq!(supervisor_writes.len(), 2);
        assert_eq!(supervisor_writes[0]["cursor"], acknowledged.cursor);
        assert_eq!(supervisor_writes[0]["run_id"], run.run_id);
        assert_eq!(supervisor_writes[0]["task_id"], task_id);
        assert_eq!(supervisor_writes[0]["target_paths"][0], "acknowledged.txt");
        assert_eq!(supervisor_writes[0]["outcome"], "ok");
        assert_eq!(supervisor_writes[0]["changed_paths"][0], "acknowledged.txt");
        assert_eq!(supervisor_writes[0]["recovery_required"], false);
        assert_eq!(supervisor_writes[1]["cursor"], unknown.cursor);
        assert_eq!(supervisor_writes[1]["run_id"], run.run_id);
        assert_eq!(supervisor_writes[1]["task_id"], task_id);
        assert_eq!(supervisor_writes[1]["target_paths"][0], "lost-ack.txt");
        assert_eq!(supervisor_writes[1]["outcome"], "unknown");
        assert_eq!(supervisor_writes[1]["reconciliation"], "unresolved");
        assert_eq!(supervisor_writes[1]["recovery_required"], true);
        assert_eq!(supervisor_run["checkpoints"]["state"], "known");
        assert_eq!(supervisor_run["checkpoints"]["count"], 2);

        assert!(provider.requests.lock().expect("provider requests").len() == 4);

        // Recreate the scheduler and runner to ensure the later kick consults
        // persisted task/journal state rather than an in-memory run latch.
        let requests_before_kick = provider.requests.lock().unwrap().len();
        let restarted_scheduler =
            crate::recipes::scheduler::RecipeScheduler::new(Arc::new(make_runner()));
        restarted_scheduler.kick_worker("worker-backend");
        tokio::time::sleep(Duration::from_millis(150)).await;
        restarted_scheduler.shutdown().await;
        assert_eq!(
            provider.requests.lock().unwrap().len(),
            requests_before_kick,
            "unresolved mutation must prevent blind worker redispatch"
        );
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("lost-ack.txt")).unwrap(),
            "second real mutation\n",
            "the later scheduler kick must not repeat the second write"
        );

        let mut recovery_executor = crate::agent::tool_executor::ToolExecutor::new(
            crate::agent::permission::PermissionMode::Autonomous,
            crate::agent::permission::new_pending_permissions(),
            "verified-worker-recovery".into(),
            workspace.path().to_string_lossy().into_owned(),
            crate::agent::tool_trait::AgentIdentity::supervisor(),
        );
        let recovered = recovery_executor
            .execute_batch(vec![(
                registry.get("reconcile_worker_operation").unwrap(),
                json!({"run_id":run.run_id,"intent_cursor":unknown.cursor,
                "decision":"reconciled","reason_code":"effect_verified_complete"}),
                "verify-lost-ack-file".into(),
            )])
            .await
            .unwrap();
        assert!(!recovered[0].1.is_error, "{}", recovered[0].1.output);
        assert!(runs.unresolved_task_side_effect(task_id).unwrap().is_none());
        // Retire only the isolated injected fault, preserving all actual guards.
        rusqlite::Connection::open(&runs_path)
            .unwrap()
            .execute_batch("DROP TRIGGER test_fail_lost_ack_outcome;")
            .unwrap();
        let released = recovery_executor
            .execute_batch(vec![(
                registry
                    .get("update_task_status")
                    .expect("Supervisor lifecycle tool"),
                json!({"id":task_id,"status":"ready"}),
                "release-verified-task".into(),
            )])
            .await
            .unwrap();
        assert!(!released[0].1.is_error, "{}", released[0].1.output);
        let frontend = workers
            .get_by_slug("worker-frontend")
            .unwrap()
            .expect("replacement frontend worker exists");
        workers
            .update_status(frontend.id, WorkerStatus::Active)
            .expect("make replacement worker runnable");
        let handoff = recovery_executor
            .execute_batch(vec![(
                registry
                    .get("assign_task")
                    .expect("Supervisor assignment tool"),
                json!({"id":task_id,"worker_slug":"worker-frontend",
                    "override_reason":"Host verified both effects and approved remaining work"}),
                "approved-manual-handoff".into(),
            )])
            .await
            .unwrap();
        assert!(!handoff[0].1.is_error, "{}", handoff[0].1.output);
        assert_eq!(
            TasksDb::open()
                .unwrap()
                .get_by_id(task_id)
                .unwrap()
                .unwrap()
                .assigned_worker
                .as_deref(),
            Some("worker-frontend")
        );
        let handoff_audit: (String, String) =
            rusqlite::Connection::open(home.path().join(".arcana").join("tasks.db"))
                .unwrap()
                .query_row(
                    "SELECT assignment_method, override_reason FROM routing_history
             WHERE task_id = ?1 AND worker_slug = 'worker-frontend'
             ORDER BY assigned_at DESC LIMIT 1",
                    [task_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("manual handoff remains auditable after reopen");
        assert_eq!(handoff_audit.0, "override");
        assert_eq!(
            handoff_audit.1,
            "Host verified both effects and approved remaining work"
        );
        let resumed_scheduler =
            crate::recipes::scheduler::RecipeScheduler::new(Arc::new(make_runner()));
        resumed_scheduler.kick_worker("worker-frontend");
        tokio::time::timeout(Duration::from_secs(90), async {
            loop {
                let latest = crate::db::runs::RunsDb::open()
                    .unwrap()
                    .latest_worker_tick("worker-frontend")
                    .unwrap();
                if latest.is_some_and(|latest| latest.outcome.is_some()) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("replacement worker settles");
        resumed_scheduler.shutdown().await;
        let tasks = TasksDb::open().unwrap();
        let resumed_task = tasks.get_by_id(task_id).unwrap().unwrap();
        let observed_run = crate::db::runs::RunsDb::open()
            .unwrap()
            .latest_worker_tick("worker-frontend")
            .unwrap()
            .unwrap();
        assert_eq!(
            provider.requests.lock().unwrap().len(),
            12,
            "replacement stopped before scripted completion: outcome={:?} failure={:?}",
            observed_run.outcome,
            observed_run.failure_code
        );
        assert_eq!(resumed_task.task_identity, task.task_identity);
        assert_eq!(
            resumed_task.status,
            crate::db::tasks::TaskStatus::QcReview,
            "resumed worker uses observed build before QC"
        );
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("continuation.txt")).unwrap(),
            "remaining work complete\n"
        );
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("acknowledged.txt")).unwrap(),
            "first real mutation\n"
        );
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("lost-ack.txt")).unwrap(),
            "second real mutation\n"
        );
        let resumed_runs = crate::db::runs::RunsDb::open().unwrap();
        let resumed = resumed_runs
            .latest_worker_tick("worker-frontend")
            .unwrap()
            .unwrap();
        assert_eq!(
            resumed_runs
                .latest_worker_tick("worker-backend")
                .unwrap()
                .unwrap()
                .run_id,
            run.run_id,
            "the original worker is not redispatched after handoff"
        );
        assert_ne!(resumed.run_id, run.run_id);
        assert_ne!(
            resumed.session_id, run.session_id,
            "continuation reconstructs fresh session"
        );
        let resumed_journal = resumed_runs
            .operation_journal_page(&resumed.run_id, 0, 50)
            .unwrap();
        let remaining_writes = resumed_journal
            .entries
            .iter()
            .filter(|entry| entry.operation == "write_file")
            .collect::<Vec<_>>();
        assert_eq!(
            remaining_writes.len(),
            1,
            "prior effects are never replayed"
        );
        assert_eq!(remaining_writes[0].target_paths, ["continuation.txt"]);
        assert_eq!(
            remaining_writes[0].outcome,
            crate::db::runs::OperationOutcome::Ok
        );
        assert_eq!(provider.requests.lock().unwrap().len(), 12);
        assert_eq!(
            crate::git::checkpoint::list(None, None, Some(task_id))
                .unwrap()
                .len(),
            3
        );
    }

    #[tokio::test]
    async fn shipped_worker_preserves_multiple_mutations_and_quarantines_lost_acknowledgement() {
        exercise_shipped_worker_lost_ack_continuation(false).await;
    }

    #[tokio::test]
    async fn shipped_worker_panic_after_lost_ack_requires_reconciliation_before_handoff() {
        exercise_shipped_worker_lost_ack_continuation(true).await;
    }

    #[tokio::test]
    async fn exhausted_startup_preserves_assigned_task_budget_and_daily_reservation() {
        use crate::budget::daily::{DailyCap, ReservationDecision};
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated workspace");
        let tasks = TasksDb::open().unwrap();
        let task = tasks
            .create(CreateTaskInput {
                title: "preserve startup identity".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "no effect during failed setup".into(),
                created_by: Some("startup-budget-test".into()),
            })
            .unwrap();
        tasks
            .update_status(task.id, "assigned", Some("worker-backend"))
            .unwrap();
        tasks
            .set_task_budget(task.id, Some(1000), Some(0.75), Some(5))
            .unwrap();
        tasks
            .update_task_budget_usage(task.id, 37, 0.125, 1)
            .unwrap();
        let before = tasks.get_by_id(task.id).unwrap().unwrap();
        let mut app_state = AppState {
            active_provider: Some("google".into()),
            active_model: Some("gemini-3.8-flash".into()),
            ..AppState::default()
        };
        app_state.add_cost(0.25, 31, 7);
        DailyCap {
            enabled: true,
            limit_usd: 2.0,
        }
        .save()
        .unwrap();
        let hold = crate::budget::daily::reserve_provider_call("existing-startup-hold".into(), 0.1)
            .unwrap();
        assert!(matches!(hold, ReservationDecision::Granted(_)));
        let budget_snapshot = || {
            let db = crate::db::config_db::open().unwrap();
            let spent: Vec<(String, i64)> = db
                .prepare(
                    "SELECT local_date, spent_micros FROM daily_budget_totals ORDER BY local_date",
                )
                .unwrap()
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap();
            let holds: Vec<(String, String, i64, String)> = db.prepare("SELECT id, local_date, amount_micros, created_at FROM daily_budget_reservations ORDER BY id").unwrap()
                .query_map([], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap();
            (spent, holds)
        };
        let before_daily = budget_snapshot();
        let state = Arc::new(Mutex::new(app_state));
        let provider = Arc::new(CommitPathProvider {
            task_id: task.id,
            fail_after_commit: false,
            turn: Mutex::new(0),
            requests: Mutex::new(Vec::new()),
        });
        let runner = RecipeRunner::new_with_workspace_override(
            state.clone(),
            Arc::new(SessionManager::new().unwrap()),
            crate::agent::permission::new_pending_permissions(),
            Some(workspace.path().to_string_lossy().into_owned()),
        )
        .with_provider_for_test(provider.clone())
        .with_startup_failures_for_test(STARTUP_MAX_ATTEMPTS as usize);
        let recipes = home.path().join(".arcana/recipes");
        crate::recipes::defaults::ensure_default_recipes(&recipes).unwrap();
        let recipe: Recipe = serde_yaml::from_str(
            &std::fs::read_to_string(recipes.join("worker-backend-poll.yaml")).unwrap(),
        )
        .unwrap();
        assert!(runner.run_recipe(&recipe, "worker-backend").await.is_err());
        assert!(
            provider.requests.lock().unwrap().is_empty(),
            "startup cannot dispatch the provider"
        );
        assert_eq!(*provider.turn.lock().unwrap(), 0);
        let after = tasks.get_by_id(task.id).unwrap().unwrap();
        assert_eq!(after.status, before.status);
        assert_eq!(after.assigned_worker, before.assigned_worker);
        assert_eq!(
            (after.claimed_by_pid, after.claim_host),
            (before.claimed_by_pid, before.claim_host)
        );
        assert_eq!(
            (
                after.budget_max_tokens,
                after.budget_max_usd,
                after.budget_max_turns
            ),
            (
                before.budget_max_tokens,
                before.budget_max_usd,
                before.budget_max_turns
            )
        );
        assert_eq!(
            (
                after.budget_tokens_used,
                after.budget_cost_usd,
                after.budget_turns_used,
                after.budget_exceeded
            ),
            (
                before.budget_tokens_used,
                before.budget_cost_usd,
                before.budget_turns_used,
                before.budget_exceeded
            )
        );
        assert_eq!(
            budget_snapshot(),
            before_daily,
            "same durable reservation and spend after retries"
        );
        let state = state.lock().unwrap();
        assert_eq!(
            (
                state.cost_today_usd,
                state.tokens_in_today,
                state.tokens_out_today
            ),
            (0.25, 31, 7)
        );
        let runs = crate::db::runs::RunsDb::open().unwrap();
        let run = runs.latest_worker_tick("worker-backend").unwrap().unwrap();
        assert_eq!(run.phase, "failed");
        assert!(runs
            .operation_journal_page(&run.run_id, 0, 20)
            .unwrap()
            .entries
            .is_empty());
        assert!(std::fs::read_dir(workspace.path())
            .unwrap()
            .next()
            .is_none());
    }

    async fn run_commit_recipe(fail_after_commit: bool) {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let analytics = Arc::new(
            crate::analytics::AnalyticsManager::new()
                .await
                .expect("initialize isolated analytics DB"),
        );
        crate::analytics::install_global(analytics);

        let workspace = tempfile::tempdir().expect("isolated selected workspace");
        crate::egress_consent::WorkspaceEgressConsentDb::open()
            .expect("open isolated workspace consent store")
            .grant(workspace.path(), "provider:google")
            .expect("grant test provider to the isolated workspace");
        let repository = Repository::init(workspace.path()).expect("initialize Git repository");
        let signature = Signature::now("Arcana Test", "arcana-test@example.invalid")
            .expect("fixture commit identity");
        repository
            .config()
            .expect("open repository config")
            .set_str("user.name", "Arcana Test")
            .expect("set fixture name");
        repository
            .config()
            .expect("open repository config")
            .set_str("user.email", "arcana-test@example.invalid")
            .expect("set fixture email");
        std::fs::write(workspace.path().join("README.md"), "base\n")
            .expect("write tracked fixture file");
        let mut index = repository.index().expect("open initial index");
        index
            .add_path(std::path::Path::new("README.md"))
            .expect("stage initial file");
        index.write().expect("write initial index");
        let tree_id = index.write_tree().expect("write initial tree");
        let tree = repository.find_tree(tree_id).expect("load initial tree");
        let initial = repository
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                "chore: initialize worker fixture\n\nCreate the isolated repository baseline.",
                &tree,
                &[],
            )
            .expect("commit repository baseline");
        drop(tree);
        repository
            .branch(
                "arcana/fleet",
                &repository.find_commit(initial).unwrap(),
                false,
            )
            .expect("create fleet branch");
        repository
            .set_head("refs/heads/arcana/fleet")
            .expect("select fleet branch");
        std::fs::write(workspace.path().join("change.txt"), "worker change\n")
            .expect("write the task change");
        drop(repository);

        let tasks = TasksDb::open().expect("open isolated task database");
        let task_id = tasks
            .create(CreateTaskInput {
                title: "Commit through production worker host".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "a model-generated commit is host validated".into(),
                created_by: Some("tg8-13".into()),
            })
            .expect("create task")
            .id;
        tasks
            .update_status(task_id, "assigned", Some("worker-backend"))
            .expect("assign task to backend worker");
        tasks
            .set_task_budget(task_id, Some(100_000), Some(5.0), Some(10))
            .expect("persist the assigned task budget before worker startup");

        let workers = crate::db::workers::WorkersDb::open().expect("seed worker roster");
        let backend = workers
            .get_by_slug("worker-backend")
            .expect("read backend worker")
            .expect("backend worker exists");
        workers
            .update_status(backend.id, WorkerStatus::Active)
            .expect("make worker runnable");

        let provider = Arc::new(CommitPathProvider {
            task_id,
            fail_after_commit,
            turn: Mutex::new(0),
            requests: Mutex::new(Vec::new()),
        });
        let state = Arc::new(Mutex::new(AppState {
            active_provider: Some("google".into()),
            active_model: Some("gemini-3.8-flash".into()),
            ..AppState::default()
        }));
        let runner = RecipeRunner::new_with_workspace_override(
            state,
            Arc::new(SessionManager::new().expect("open isolated session manager")),
            crate::agent::permission::new_pending_permissions(),
            Some(workspace.path().to_string_lossy().into_owned()),
        )
        .with_provider_for_test(provider.clone())
        .with_startup_failures_for_test(1);
        let recipes_dir = home.path().join(".arcana").join("recipes");
        crate::recipes::defaults::ensure_default_recipes(&recipes_dir)
            .expect("persist the shipped worker recipes");
        let recipe: Recipe = serde_yaml::from_str(
            &std::fs::read_to_string(recipes_dir.join("worker-backend-poll.yaml"))
                .expect("read the shipped backend recipe"),
        )
        .expect("parse the shipped backend recipe");
        assert_eq!(recipe.id, "worker-backend-poll");
        assert!(recipe.instructions.contains("git_commit"));

        let scheduler = crate::recipes::scheduler::RecipeScheduler::new(Arc::new(runner));
        scheduler.kick_worker("worker-backend");
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let completed = crate::db::runs::RunsDb::open()
                    .expect("open worker run ledger")
                    .latest_worker_tick("worker-backend")
                    .expect("read worker tick")
                    .is_some_and(|run| {
                        run.phase
                            == if fail_after_commit {
                                "failed"
                            } else {
                                "completed"
                            }
                    });
                if completed {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("scheduler must finish the scripted worker run before timeout");
        scheduler.shutdown().await;

        let committed_repo = Repository::open(workspace.path()).expect("reopen worker repository");
        assert_eq!(
            committed_repo.head().unwrap().shorthand(),
            Some("arcana/fleet")
        );
        let committed = committed_repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(committed.parent_count(), 1);
        assert!(committed
            .message()
            .unwrap()
            .starts_with("fix(core): commit through the worker host"));
        let parent = committed.parent(0).expect("read fixture baseline commit");
        let diff = committed_repo
            .diff_tree_to_tree(
                Some(&parent.tree().unwrap()),
                Some(&committed.tree().unwrap()),
                None,
            )
            .expect("compare the committed tree to its parent");
        let changed_paths = diff
            .deltas()
            .filter_map(|delta| {
                delta
                    .new_file()
                    .path()
                    .map(|path| path.to_string_lossy().into_owned())
            })
            .collect::<Vec<_>>();
        assert_eq!(changed_paths, ["change.txt"]);
        let task = tasks.get_by_id(task_id).unwrap().unwrap();
        assert_eq!(
            task.status,
            if fail_after_commit {
                crate::db::tasks::TaskStatus::NeedsInfo
            } else {
                crate::db::tasks::TaskStatus::InProgress
            }
        );
        assert_eq!(task.assigned_worker.as_deref(), Some("worker-backend"));
        assert!(
            task.budget_turns_used > 0 && task.budget_turns_used <= 3,
            "the saved task budget must activate when the single engine workflow claims it; used {} turns",
            task.budget_turns_used
        );
        assert!(!task.budget_exceeded);
        let requests = provider
            .requests
            .lock()
            .expect("captured provider requests");
        assert_eq!(requests.len(), 3, "claim, commit and final response turns");
        assert!(requests
            .iter()
            .all(|request| request.tools.iter().any(|tool| tool.name == "git_commit")));
        drop(requests);
        let runs = crate::db::runs::RunsDb::open().expect("open worker run ledger");
        let latest = runs
            .latest_worker_tick("worker-backend")
            .expect("read latest worker tick")
            .expect("production runner should persist a tick");
        assert_eq!(
            latest.phase,
            if fail_after_commit {
                "failed"
            } else {
                "completed"
            }
        );
        let journal = runs
            .operation_journal_page(&latest.run_id, 0, 20)
            .expect("read commit operation journal");
        assert_eq!(
            journal
                .entries
                .iter()
                .filter(|entry| {
                    entry.operation == "git_commit"
                        && entry.outcome == crate::db::runs::OperationOutcome::Ok
                })
                .count(),
            1,
            "exactly one durable committed effect"
        );
        assert_eq!(
            committed.parent(0).unwrap().id(),
            initial,
            "no replayed commit"
        );
        if fail_after_commit {
            assert!(task.claimed_by_pid.is_none() && task.claim_host.is_none());
            let before = provider.requests.lock().unwrap().len();
            scheduler.kick_worker("worker-backend");
            tokio::time::sleep(Duration::from_millis(100)).await;
            scheduler.shutdown().await;
            assert_eq!(
                provider.requests.lock().unwrap().len(),
                before,
                "held task must not automatically restart after its effect"
            );
        }
    }
}

#[cfg(test)]
mod supervisor_build_retry_recipe_tests {
    use super::*;
    use crate::db::tasks::{CreateTaskInput, TasksDb};
    use crate::provider::traits::{Provider, StreamResult};
    use crate::provider::types::{
        CompletionRequest, CompletionResponse, ModelInfo, ProviderStatus, StreamChunk,
    };
    use crate::provider::usage::Usage;
    use async_trait::async_trait;
    use futures::stream;
    use serde_json::{json, Value};
    use std::sync::Mutex;

    struct SupervisorRetryProvider {
        request_id: i64,
        turn: Mutex<usize>,
        requests: Mutex<Vec<CompletionRequest>>,
    }

    impl SupervisorRetryProvider {
        fn tool_call(id: &str, name: &str, input: Value) -> Vec<StreamChunk> {
            vec![
                StreamChunk::ToolUseStart {
                    id: id.into(),
                    name: name.into(),
                    block_index: 0,
                    opaque: None,
                },
                StreamChunk::ToolInputDelta {
                    block_index: 0,
                    partial_json: input.to_string(),
                },
                StreamChunk::MessageDelta {
                    stop_reason: "tool_use".into(),
                    usage: Usage::default(),
                },
                StreamChunk::Done,
            ]
        }

        fn final_answer() -> Vec<StreamChunk> {
            vec![
                StreamChunk::TextDelta {
                    text: "The failed build was reviewed and one retry was approved.".into(),
                    block_index: 0,
                },
                StreamChunk::MessageDelta {
                    stop_reason: "end_turn".into(),
                    usage: Usage::default(),
                },
                StreamChunk::Done,
            ]
        }
    }

    #[async_trait]
    impl Provider for SupervisorRetryProvider {
        fn id(&self) -> &str {
            "google"
        }

        fn name(&self) -> &str {
            "Scripted Supervisor build retry decision"
        }

        fn models(&self) -> Vec<ModelInfo> {
            crate::provider::catalogue::models_for("google")
        }

        async fn input_token_upper_bound(
            &self,
            _request: &CompletionRequest,
        ) -> crate::errors::Result<u64> {
            Ok(1_000)
        }

        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> crate::errors::Result<CompletionResponse> {
            unreachable!("RecipeRunner must use AgentEngine streaming")
        }

        async fn stream(&self, request: CompletionRequest) -> crate::errors::Result<StreamResult> {
            self.requests
                .lock()
                .expect("captured provider requests")
                .push(request.clone());
            let turn = {
                let mut turn = self.turn.lock().expect("provider turn lock");
                let current = *turn;
                *turn += 1;
                current
            };
            let chunks = match turn {
                0 => {
                    assert!(request.tools.iter().any(|tool| tool.name == "list_tasks"));
                    Self::tool_call("list-board", "list_tasks", json!({"status": "all"}))
                }
                1 => {
                    let board_read = request
                        .messages
                        .iter()
                        .flat_map(|message| &message.content)
                        .find_map(|content| match content {
                            crate::provider::types::MessageContent::ToolResult {
                                name,
                                is_error,
                                ..
                            } if name == "list_tasks" => Some(*is_error),
                            _ => None,
                        })
                        .expect("Supervisor must read the task board before requests");
                    assert!(!board_read, "board read should succeed");
                    assert!(request
                        .tools
                        .iter()
                        .any(|tool| tool.name == "list_worker_requests"));
                    Self::tool_call(
                        "list-escalations",
                        "list_worker_requests",
                        json!({"status": "pending"}),
                    )
                }
                2 => {
                    let pending = request
                        .messages
                        .iter()
                        .flat_map(|message| &message.content)
                        .find_map(|content| match content {
                            crate::provider::types::MessageContent::ToolResult {
                                name,
                                content,
                                is_error,
                                ..
                            } if name == "list_worker_requests" => {
                                Some((content.clone(), *is_error))
                            }
                            _ => None,
                        })
                        .expect("Supervisor must inspect the pending escalation");
                    assert!(!pending.1, "pending request read should succeed");
                    assert!(pending.0.contains(&self.request_id.to_string()));
                    assert!(pending.0.contains("build_retry_approval"));
                    assert!(pending.0.contains("diagnosis"));
                    assert!(request
                        .tools
                        .iter()
                        .any(|tool| tool.name == "resolve_worker_request"));
                    Self::tool_call(
                        "approve-retry",
                        "resolve_worker_request",
                        json!({
                            "id": self.request_id,
                            "action": "approve_build_retry",
                            "note": "The diagnosis is actionable; authorize one bounded verification batch."
                        }),
                    )
                }
                3 => {
                    let approval = request
                        .messages
                        .iter()
                        .flat_map(|message| &message.content)
                        .find_map(|content| match content {
                            crate::provider::types::MessageContent::ToolResult {
                                name,
                                content,
                                is_error,
                                ..
                            } if name == "resolve_worker_request" => {
                                Some((content.clone(), *is_error))
                            }
                            _ => None,
                        })
                        .expect("provider must observe the production approval result");
                    assert!(!approval.1, "Supervisor approval should succeed");
                    assert!(approval.0.contains("exactly one"));
                    Self::final_answer()
                }
                _ => panic!("unexpected provider turn after Supervisor approval"),
            };
            Ok(Box::pin(stream::iter(chunks.into_iter().map(Ok))))
        }

        async fn test_connection(&self) -> crate::errors::Result<ProviderStatus> {
            Ok(ProviderStatus::Connected)
        }
    }

    #[tokio::test]
    async fn shipped_supervisor_recipe_approves_one_diagnosed_build_retry() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let analytics = Arc::new(
            crate::analytics::AnalyticsManager::new()
                .await
                .expect("initialize isolated analytics DB"),
        );
        crate::analytics::install_global(analytics);
        let workspace = tempfile::tempdir().expect("isolated selected workspace");
        crate::egress_consent::WorkspaceEgressConsentDb::open()
            .expect("open isolated workspace consent store")
            .grant(workspace.path(), "provider:google")
            .expect("grant scripted provider for the isolated workspace");

        let tasks = TasksDb::open().expect("open isolated TasksDb");
        let task_id = tasks
            .create(CreateTaskInput {
                title: "Repair a repeatedly failing build".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "Supervisor reviews diagnosis before another build".into(),
                created_by: Some("tg4-08-test".into()),
            })
            .expect("create task")
            .id;
        tasks
            .update_status(task_id, "assigned", Some("worker-backend"))
            .expect("assign task to backend worker");
        tasks
            .claim_task_with_pid(task_id, "worker-backend", std::process::id())
            .expect("claim task for the worker");
        let fingerprint = "test-workspace-fingerprint";
        for index in 0..3 {
            let session_id = format!("failed-build-{index}");
            let batch = tasks
                .begin_project_build_batch_for_worker(
                    "worker-backend",
                    &session_id,
                    1,
                    &workspace.path().to_string_lossy(),
                    Some(fingerprint),
                )
                .expect("begin observed failed build batch")
                .expect("claimed task should own a build batch");
            tasks
                .finish_project_build_batch_for_worker(
                    batch.id,
                    "worker-backend",
                    &session_id,
                    &[(
                        "cargo test".into(),
                        101,
                        "cargo".into(),
                        Some(workspace.path().to_string_lossy().into_owned()),
                        Some(fingerprint.into()),
                    )],
                    Some(0),
                    true,
                )
                .expect("persist observed failed build exit");
        }
        let diagnosis = json!({
            "diagnosis": [{"code": "RUST_COMPILER_ERROR", "location": "src/lib.rs:12"}]
        })
        .to_string();
        let request = tasks
            .create_build_retry_request_for_worker("worker-backend", task_id, &diagnosis)
            .expect("worker creates typed diagnosis request");

        let workers = crate::db::workers::WorkersDb::open().expect("seed worker roster");
        let supervisor = workers
            .get_by_slug("supervisor")
            .expect("read Supervisor")
            .expect("seeded Supervisor exists");
        workers
            .update_status(supervisor.id, WorkerStatus::Active)
            .expect("make Supervisor runnable");
        let provider = Arc::new(SupervisorRetryProvider {
            request_id: request.id,
            turn: Mutex::new(0),
            requests: Mutex::new(Vec::new()),
        });
        let state = Arc::new(Mutex::new(AppState {
            active_provider: Some("google".into()),
            active_model: Some("gemini-3.8-flash".into()),
            ..AppState::default()
        }));
        let runner = RecipeRunner::new_with_workspace_override(
            state,
            Arc::new(SessionManager::new().expect("open isolated session manager")),
            crate::agent::permission::new_pending_permissions(),
            Some(workspace.path().to_string_lossy().into_owned()),
        )
        .with_provider_for_test(provider.clone());
        let recipes_dir = home.path().join(".arcana").join("recipes");
        crate::recipes::defaults::ensure_default_recipes(&recipes_dir)
            .expect("persist shipped recipes");
        let recipe: Recipe = serde_yaml::from_str(
            &std::fs::read_to_string(recipes_dir.join("supervisor-poll.yaml"))
                .expect("read shipped Supervisor recipe"),
        )
        .expect("parse shipped Supervisor recipe");

        runner
            .run_recipe(&recipe, "supervisor")
            .await
            .expect("scripted Supervisor run should process the build retry request");

        drop(tasks);
        let tasks = TasksDb::open().expect("reopen task DB after Supervisor run");
        let request_after = tasks
            .get_worker_request_by_id(request.id)
            .expect("read resolved request")
            .expect("request remains auditable");
        assert_eq!(
            request_after.status,
            crate::db::tasks::WorkerRequestStatus::Completed
        );
        drop(tasks);
        let database_path = home.path().join(".arcana").join("tasks.db");
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let workspace_path = workspace.path().to_string_lossy().into_owned();
        let workers = (0..2)
            .map(|index| {
                let barrier = Arc::clone(&barrier);
                let database_path = database_path.clone();
                let workspace_path = workspace_path.clone();
                std::thread::spawn(move || {
                    let tasks = TasksDb::open_at(&database_path)
                        .expect("open independent TasksDb connection");
                    barrier.wait();
                    tasks.begin_project_build_batch_for_worker(
                        "worker-backend",
                        &format!("approved-retry-race-{index}"),
                        1,
                        &workspace_path,
                        Some(fingerprint),
                    )
                })
            })
            .collect::<Vec<_>>();
        let outcomes = workers
            .into_iter()
            .map(|worker| {
                worker
                    .join()
                    .expect("concurrent batch consumer must not panic")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| outcome.as_ref().is_ok_and(|batch| batch
                    .as_ref()
                    .is_some_and(|batch| batch.supervisor_retry_approved)))
                .count(),
            1,
            "exactly one concurrent build batch may consume the approval"
        );
        assert_eq!(
            outcomes.iter().filter(|outcome| outcome.is_err()).count(),
            1,
            "the losing consumer must fail closed after the atomic claim"
        );
        let reopened = TasksDb::open().expect("reopen task DB after concurrent consumption");
        let retry_count = reopened
            .get_by_id(task_id)
            .expect("read task after batch race")
            .expect("task remains persisted")
            .retry_count;
        assert_eq!(retry_count, 3, "starting a retry does not invent an exit");

        let provider_requests = provider
            .requests
            .lock()
            .expect("captured provider requests");
        assert_eq!(provider_requests.len(), 4);
        assert!(provider_requests.iter().all(|request| {
            request
                .tools
                .iter()
                .any(|tool| tool.name == "resolve_worker_request")
        }));
    }
}

#[cfg(test)]
mod supervisor_operation_recovery_recipe_tests {
    use super::*;
    use crate::db::tasks::{CreateTaskInput, TasksDb};
    use crate::provider::traits::{Provider, StreamResult};
    use crate::provider::types::{
        CompletionRequest, CompletionResponse, ModelInfo, ProviderStatus, StreamChunk,
    };
    use crate::provider::usage::Usage;
    use async_trait::async_trait;
    use futures::stream;
    use serde_json::{json, Value};
    use std::sync::Mutex;

    struct SupervisorOperationRecoveryProvider {
        run_id: String,
        task_id: i64,
        intent_cursor: i64,
        turn: Mutex<usize>,
    }

    impl SupervisorOperationRecoveryProvider {
        fn tool_call(id: &str, name: &str, input: Value) -> Vec<StreamChunk> {
            vec![
                StreamChunk::ToolUseStart {
                    id: id.into(),
                    name: name.into(),
                    block_index: 0,
                    opaque: None,
                },
                StreamChunk::ToolInputDelta {
                    block_index: 0,
                    partial_json: input.to_string(),
                },
                StreamChunk::MessageDelta {
                    stop_reason: "tool_use".into(),
                    usage: Usage::default(),
                },
                StreamChunk::Done,
            ]
        }

        fn final_answer() -> Vec<StreamChunk> {
            vec![
                StreamChunk::TextDelta {
                    text: "The interrupted operation remains held because its side effect is uncertain.".into(),
                    block_index: 0,
                },
                StreamChunk::MessageDelta {
                    stop_reason: "end_turn".into(),
                    usage: Usage::default(),
                },
                StreamChunk::Done,
            ]
        }
    }

    #[async_trait]
    impl Provider for SupervisorOperationRecoveryProvider {
        fn id(&self) -> &str {
            "google"
        }

        fn name(&self) -> &str {
            "Scripted Supervisor operation recovery"
        }

        fn models(&self) -> Vec<ModelInfo> {
            crate::provider::catalogue::models_for("google")
        }

        async fn input_token_upper_bound(
            &self,
            _request: &CompletionRequest,
        ) -> crate::errors::Result<u64> {
            Ok(1_000)
        }

        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> crate::errors::Result<CompletionResponse> {
            unreachable!("RecipeRunner must use AgentEngine streaming")
        }

        async fn stream(&self, request: CompletionRequest) -> crate::errors::Result<StreamResult> {
            let turn = {
                let mut turn = self.turn.lock().expect("provider turn lock");
                let current = *turn;
                *turn += 1;
                current
            };
            let chunks = match turn {
                0 => {
                    assert!(request.tools.iter().any(|tool| tool.name == "list_tasks"));
                    Self::tool_call("recovery-board", "list_tasks", json!({ "compact": false }))
                }
                1 => {
                    let board = request
                        .messages
                        .iter()
                        .flat_map(|message| &message.content)
                        .find_map(|content| match content {
                            crate::provider::types::MessageContent::ToolResult {
                                name,
                                content,
                                is_error,
                                ..
                            } if name == "list_tasks" => Some((content.clone(), *is_error)),
                            _ => None,
                        })
                        .expect("Supervisor must read the board first");
                    assert!(!board.1, "task board read should succeed");
                    assert!(board.0.contains(&self.task_id.to_string()));
                    assert!(board.0.contains("worker recovery"));
                    assert!(request
                        .tools
                        .iter()
                        .any(|tool| tool.name == "list_worker_requests"));
                    Self::tool_call(
                        "recovery-requests",
                        "list_worker_requests",
                        json!({ "status": "pending" }),
                    )
                }
                2 => {
                    let requests = request
                        .messages
                        .iter()
                        .flat_map(|message| &message.content)
                        .find_map(|content| match content {
                            crate::provider::types::MessageContent::ToolResult {
                                name,
                                is_error,
                                ..
                            } if name == "list_worker_requests" => Some(*is_error),
                            _ => None,
                        })
                        .expect("Supervisor must inspect its request queue");
                    assert!(!requests, "pending request read should succeed");
                    assert!(request
                        .tools
                        .iter()
                        .any(|tool| tool.name == "list_worker_messages"));
                    Self::tool_call(
                        "recovery-inbox",
                        "list_worker_messages",
                        json!({ "unread_only": true }),
                    )
                }
                3 => {
                    let inbox = request
                        .messages
                        .iter()
                        .flat_map(|message| &message.content)
                        .find_map(|content| match content {
                            crate::provider::types::MessageContent::ToolResult {
                                name,
                                content,
                                is_error,
                                ..
                            } if name == "list_worker_messages" => {
                                Some((content.clone(), *is_error))
                            }
                            _ => None,
                        })
                        .expect("Supervisor must drain its ACP inbox");
                    assert!(!inbox.1, "inbox read should succeed");
                    assert!(inbox.0.contains("Inbox empty"));
                    assert!(request
                        .tools
                        .iter()
                        .any(|tool| tool.name == "list_worker_health"));
                    Self::tool_call(
                        "recovery-health",
                        "list_worker_health",
                        json!({ "limit": 100, "journal_limit": 50 }),
                    )
                }
                4 => {
                    let health = request
                        .messages
                        .iter()
                        .flat_map(|message| &message.content)
                        .find_map(|content| match content {
                            crate::provider::types::MessageContent::ToolResult {
                                name,
                                content,
                                is_error,
                                ..
                            } if name == "list_worker_health" => Some((content.clone(), *is_error)),
                            _ => None,
                        })
                        .expect("Supervisor must inspect durable worker health");
                    assert!(!health.1, "health read should succeed");
                    let health: Value = serde_json::from_str(&health.0).expect("health JSON");
                    assert_eq!(
                        health["workers"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|worker| worker["slug"] == "worker-backend")
                            .unwrap()["run"]["run_id"],
                        self.run_id
                    );
                    assert_eq!(
                        health["workers"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|worker| worker["slug"] == "worker-backend")
                            .unwrap()["run"]["operation_journal"]["entries"][0]["cursor"],
                        self.intent_cursor
                    );
                    assert_eq!(
                        health["workers"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|worker| worker["slug"] == "worker-backend")
                            .unwrap()["run"]["operation_journal"]["entries"][0]["outcome"],
                        "unknown"
                    );
                    assert!(request.tools.iter().any(|tool| tool.name == "get_task"));
                    Self::tool_call("recovery-task", "get_task", json!({ "id": self.task_id }))
                }
                5 => {
                    let task = request
                        .messages
                        .iter()
                        .flat_map(|message| &message.content)
                        .find_map(|content| match content {
                            crate::provider::types::MessageContent::ToolResult {
                                name,
                                content,
                                is_error,
                                ..
                            } if name == "get_task" => Some((content.clone(), *is_error)),
                            _ => None,
                        })
                        .expect("Supervisor must inspect the held task before deciding");
                    assert!(!task.1, "task detail read should succeed");
                    let task: Value = serde_json::from_str(&task.0).expect("task JSON");
                    assert_eq!(task["status"], "needs_info");
                    assert!(task["rejection_reason"]
                        .as_str()
                        .unwrap()
                        .starts_with("[worker recovery]"));
                    assert!(request
                        .tools
                        .iter()
                        .any(|tool| tool.name == "reconcile_worker_operation"));
                    Self::tool_call(
                        "reconcile-uncertain",
                        "reconcile_worker_operation",
                        json!({
                            "run_id": self.run_id,
                            "intent_cursor": self.intent_cursor,
                            "decision": "do_not_retry",
                            "reason_code": "unsafe_to_repeat"
                        }),
                    )
                }
                6 => {
                    let decision = request
                        .messages
                        .iter()
                        .flat_map(|message| &message.content)
                        .find_map(|content| match content {
                            crate::provider::types::MessageContent::ToolResult {
                                name,
                                content,
                                is_error,
                                ..
                            } if name == "reconcile_worker_operation" => {
                                Some((content.clone(), *is_error))
                            }
                            _ => None,
                        })
                        .expect("provider must observe the production reconciliation result");
                    assert!(!decision.1, "Supervisor decision should persist");
                    assert!(decision.0.contains("do_not_retry"));
                    assert!(decision.0.contains("unsafe_to_repeat"));
                    Self::final_answer()
                }
                _ => panic!("unexpected Supervisor recovery provider turn"),
            };
            Ok(Box::pin(stream::iter(chunks.into_iter().map(Ok))))
        }

        async fn test_connection(&self) -> crate::errors::Result<ProviderStatus> {
            Ok(ProviderStatus::Connected)
        }
    }

    #[tokio::test]
    async fn shipped_supervisor_recipe_keeps_uncertain_operation_quarantined() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let analytics = Arc::new(
            crate::analytics::AnalyticsManager::new()
                .await
                .expect("initialize analytics"),
        );
        crate::analytics::install_global(analytics);
        let workspace = tempfile::tempdir().expect("isolated selected workspace");
        crate::egress_consent::WorkspaceEgressConsentDb::open()
            .expect("open isolated workspace consent store")
            .grant(workspace.path(), "provider:google")
            .expect("grant scripted provider for isolated workspace");

        let tasks = TasksDb::open().expect("open isolated TasksDb");
        let task_id = tasks
            .create(CreateTaskInput {
                title: "Inspect interrupted file mutation".into(),
                description: Some(
                    "The worker stopped after an unacknowledged file mutation.".into(),
                ),
                priority: "p1".into(),
                task_type: "feature".into(),
                acceptance_criteria: "Do not replay uncertain mutations.".into(),
                created_by: Some("tg4-53-test".into()),
            })
            .expect("create recovery task")
            .id;
        tasks
            .update_status(task_id, "assigned", Some("worker-backend"))
            .expect("assign task");
        tasks
            .claim_task_with_pid(task_id, "worker-backend", std::process::id())
            .expect("claim task");
        let host = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "unknown".to_string());
        tasks
            .hold_interrupted_claims_for_worker_run(
                "worker-backend",
                std::process::id() as i64,
                &host,
                "provider acknowledgement was lost",
            )
            .expect("quarantine interrupted worker claim");

        let runs = crate::db::runs::RunsDb::open().expect("open run ledger");
        let run_id = runs
            .begin_worker_tick("worker-backend", "operation-recovery-session")
            .expect("begin worker run");
        runs.update_worker_tick_phase(&run_id, "running", Some(task_id), None)
            .expect("attribute run to task");
        let intent_cursor = runs
            .record_operation_intent(
                &run_id,
                Some(task_id),
                "uncertain-file-write-1",
                "write_file",
                "FileSystem",
                &["src/calendar.rs".into()],
            )
            .expect("record unknown file mutation intent");

        let workers = crate::db::workers::WorkersDb::open().expect("seed worker roster");
        let supervisor = workers
            .get_by_slug("supervisor")
            .expect("read Supervisor")
            .expect("seeded Supervisor");
        workers
            .update_status(supervisor.id, WorkerStatus::Active)
            .expect("make Supervisor runnable");
        let provider = Arc::new(SupervisorOperationRecoveryProvider {
            run_id: run_id.clone(),
            task_id,
            intent_cursor,
            turn: Mutex::new(0),
        });
        let state = Arc::new(Mutex::new(AppState {
            active_provider: Some("google".into()),
            active_model: Some("gemini-3.8-flash".into()),
            ..AppState::default()
        }));
        let runner = RecipeRunner::new_with_workspace_override(
            state,
            Arc::new(SessionManager::new().expect("open isolated session manager")),
            crate::agent::permission::new_pending_permissions(),
            Some(workspace.path().to_string_lossy().into_owned()),
        )
        .with_supervisor_budget_override(Some(crate::budget::TaskBudget {
            max_tokens: Some(100_000),
            max_usd: None,
            max_turns: Some(12),
        }))
        .with_provider_for_test(provider);
        let recipes_dir = home.path().join(".arcana").join("recipes");
        crate::recipes::defaults::ensure_default_recipes(&recipes_dir)
            .expect("persist shipped recipes");
        let recipe: Recipe = serde_yaml::from_str(
            &std::fs::read_to_string(recipes_dir.join("supervisor-poll.yaml"))
                .expect("read shipped Supervisor recipe"),
        )
        .expect("parse shipped Supervisor recipe");

        runner
            .run_recipe(&recipe, "supervisor")
            .await
            .expect("scripted recovery tick");

        drop(runs);
        let runs = crate::db::runs::RunsDb::open().expect("reopen journal after Supervisor run");
        let page = runs
            .operation_journal_page(&run_id, 0, 10)
            .expect("read reconciled journal");
        let entry = page
            .entries
            .iter()
            .find(|entry| entry.cursor == intent_cursor)
            .expect("intent retained");
        assert_eq!(entry.outcome, crate::db::runs::OperationOutcome::Unknown);
        assert_eq!(
            entry.reconciliation,
            crate::db::runs::OperationReconciliation::DoNotRetry
        );
        assert_eq!(entry.reconciliation_actor.as_deref(), Some("supervisor"));
        assert_eq!(
            entry.reconciliation_reason.as_deref(),
            Some("unsafe_to_repeat")
        );
        assert_eq!(
            runs.unresolved_task_side_effect(task_id)
                .expect("read retry barrier")
                .unwrap()
                .2,
            intent_cursor
        );
        runs.finish_worker_tick(&run_id, "interrupted", "interrupted", None)
            .expect("close the interrupted worker run");
        let replacement_run = runs
            .begin_worker_tick("worker-backend", "recovery-replacement-session")
            .expect("start replacement worker run after interruption");
        assert!(matches!(
            runs.record_operation_intent(
                &replacement_run,
                Some(task_id),
                "must-not-replay-uncertain-effect",
                "write_file",
                "FileSystem",
                &["src/calendar.rs".into()],
            ),
            Err(crate::errors::ArcanaError::OperationReconciliationRequired { .. })
        ));
        let held = TasksDb::open()
            .expect("reopen task board")
            .get_by_id(task_id)
            .expect("read held task")
            .unwrap();
        assert_eq!(held.status, crate::db::tasks::TaskStatus::NeedsInfo);
        assert!(held
            .rejection_reason
            .as_deref()
            .unwrap_or_default()
            .starts_with("[worker recovery]"));
    }
}

#[cfg(test)]
mod startup_retry_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Notify;

    fn transient_startup_error() -> ArcanaError {
        ArcanaError::Io(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "transient setup failure",
        ))
    }

    #[test]
    fn supervisor_run_budget_can_be_overridden_without_raising_worker_caps() {
        let override_budget = crate::budget::TaskBudget {
            max_tokens: Some(180_000),
            max_usd: Some(2.0),
            max_turns: Some(30),
        };
        assert_eq!(
            worker_run_budget("supervisor", Some(&override_budget)),
            override_budget
        );
        assert_eq!(
            worker_run_budget("supervisor", None),
            crate::budget::TaskBudget::default_supervisor()
        );
        assert_eq!(
            worker_run_budget("worker-backend", Some(&override_budget)),
            crate::budget::TaskBudget::default_worker(),
            "a Supervisor CLI override must not widen worker run caps"
        );
        assert_eq!(worker_output_token_limit("supervisor"), Some(6_144));
        assert_eq!(worker_output_token_limit("worker-backend"), None);
    }

    #[test]
    fn startup_backoff_uses_bounded_exponential_full_jitter() {
        let expected_windows = [200, 400, 800, 1_600];
        for (index, window_ms) in expected_windows.into_iter().enumerate() {
            let retry = index as u32 + 1;
            let low = startup_retry_delay(retry, 0).as_millis() as u64;
            let middle = startup_retry_delay(retry, u128::from(window_ms / 2)).as_millis() as u64;
            let high = startup_retry_delay(retry, u128::from(window_ms)).as_millis() as u64;
            assert_eq!(low, 0, "retry {retry} must allow full lower jitter bound");
            assert_eq!(middle, window_ms / 2, "retry {retry} midpoint");
            assert_eq!(
                high, window_ms,
                "retry {retry} must reach its exponential ceiling"
            );
            assert!(high <= STARTUP_RETRY_MAX_DELAY.as_millis() as u64);
        }
    }

    #[tokio::test]
    async fn transient_setup_failure_recovers_within_startup_boundary() {
        let mut attempts = 0;
        let prepared = retry_startup("test-worker", "test-run", || {
            attempts += 1;
            if attempts == 1 {
                return Err(transient_startup_error());
            }
            Ok("prepared")
        })
        .await
        .expect("second setup attempt should recover");

        assert_eq!(prepared, "prepared");
        assert_eq!(attempts, 2);
    }

    #[test]
    fn startup_retry_events_include_the_durable_worker_run_id() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("create current-thread runtime");
        let mut attempts = 0;
        let (_, events) = crate::observability::timing::capture_json_events(|| {
            runtime.block_on(retry_startup_with_state_and_delay(
                "test-worker",
                "durable-run-42",
                || {
                    attempts += 1;
                    if attempts == 1 {
                        Err(transient_startup_error())
                    } else {
                        Ok(())
                    }
                },
                |_| {},
                |_| Duration::ZERO,
            ))
        });

        assert_eq!(attempts, 2);
        let retry_events = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "worker_startup_retry")
            .collect::<Vec<_>>();
        assert_eq!(retry_events.len(), 2, "scheduled and recovered events");
        assert!(retry_events
            .iter()
            .all(|event| event["fields"]["run_id"] == "durable-run-42"));
        assert!(retry_events.iter().any(|event| {
            event["fields"]["outcome"] == "retry_scheduled" && event["fields"]["attempt"] == 1
        }));
        assert!(retry_events.iter().any(|event| {
            event["fields"]["outcome"] == "recovered" && event["fields"]["attempt"] == 2
        }));
        assert!(retry_events
            .iter()
            .all(|event| event["fields"]["max_attempts"] == STARTUP_MAX_ATTEMPTS));
    }

    #[test]
    fn synchronous_setup_overruns_are_reported_without_another_attempt() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("create current-thread runtime");
        for succeeds in [true, false] {
            let mut attempts = 0;
            let (result, events) = crate::observability::timing::capture_json_events(|| {
                runtime.block_on(retry_startup_with_state_and_delay(
                    "test-worker",
                    "overrun-run",
                    || {
                        attempts += 1;
                        std::thread::sleep(
                            STARTUP_RETRY_ADMISSION_WINDOW + Duration::from_millis(20),
                        );
                        if succeeds {
                            Ok("prepared")
                        } else {
                            Err(transient_startup_error())
                        }
                    },
                    |_| panic!("overrun must not schedule another attempt"),
                    |_| Duration::ZERO,
                ))
            });
            assert_eq!(result.is_ok(), succeeds);
            assert_eq!(attempts, 1);
            let overruns = events
                .iter()
                .filter(|event| {
                    event["fields"]["event_type"] == "worker_startup_retry"
                        && event["fields"]["outcome"] == "setup_overrun"
                })
                .collect::<Vec<_>>();
            assert_eq!(overruns.len(), 1);
            assert_eq!(overruns[0]["fields"]["run_id"], "overrun-run");
            assert_eq!(overruns[0]["fields"]["setup_succeeded"], succeeds);
            assert!(
                overruns[0]["fields"]["elapsed_ms"].as_u64().unwrap()
                    >= STARTUP_RETRY_ADMISSION_WINDOW.as_millis() as u64
            );
        }
    }

    #[tokio::test]
    async fn delayed_backoff_does_not_admit_setup_after_the_window() {
        let mut attempts = 0;
        let mut blocker = None;
        let result = retry_startup_with_state_and_delay(
            "test-worker",
            "late-backoff-run",
            || {
                attempts += 1;
                Err::<(), _>(transient_startup_error())
            },
            |_| {
                blocker = Some(tokio::spawn(async {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    std::thread::sleep(STARTUP_RETRY_ADMISSION_WINDOW + Duration::from_millis(20));
                }));
            },
            |_| Duration::from_millis(100),
        )
        .await;
        blocker.unwrap().await.unwrap();
        assert!(result.is_err());
        assert_eq!(attempts, 1, "late wake-up must not start another setup");
    }

    #[tokio::test]
    async fn permanent_setup_failure_is_not_retried() {
        let mut attempts = 0;
        let result = retry_startup::<(), _>("test-worker", "test-run", || {
            attempts += 1;
            Err(ArcanaError::Agent("invalid workspace configuration".into()))
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts, 1);
    }

    #[tokio::test]
    async fn vault_setup_failure_is_not_retried_without_a_typed_transient_cause() {
        let mut attempts = 0;
        let result = retry_startup::<(), _>("test-worker", "test-run", || {
            attempts += 1;
            Err(ArcanaError::Vault("vault data is corrupt".into()))
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts, 1);
    }

    #[tokio::test]
    async fn exhausted_transient_startup_returns_after_the_attempt_limit() {
        let mut attempts = 0;
        let started = Instant::now();
        let result = retry_startup::<(), _>("test-worker", "test-run", || {
            attempts += 1;
            Err(transient_startup_error())
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts, STARTUP_MAX_ATTEMPTS);
        assert!(started.elapsed() <= STARTUP_RETRY_ADMISSION_WINDOW);
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_during_backoff_stops_before_another_attempt() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let retry_scheduled = Arc::new(Notify::new());
        let task = tokio::spawn({
            let attempts = Arc::clone(&attempts);
            let retry_scheduled = Arc::clone(&retry_scheduled);
            async move {
                retry_startup_with_state_and_delay(
                    "test-worker",
                    "test-run",
                    move || {
                        attempts.fetch_add(1, Ordering::SeqCst);
                        Err::<(), _>(transient_startup_error())
                    },
                    move |_| retry_scheduled.notify_one(),
                    |_| Duration::from_secs(1),
                )
                .await
            }
        });

        retry_scheduled.notified().await;
        task.abort();
        assert!(task.await.expect_err("aborted retry task").is_cancelled());
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            1,
            "cancellation during the retry sleep must prevent another setup"
        );
    }

    #[tokio::test]
    async fn startup_panic_is_contained_and_retried_before_provider_work() {
        let mut attempts = 0;
        let result = retry_startup("test-worker", "test-run", || {
            attempts += 1;
            if attempts == 1 {
                panic!("synthetic startup panic");
            }
            Ok("prepared")
        })
        .await;

        assert_eq!(
            result.expect("second startup attempt should recover"),
            "prepared"
        );
        assert_eq!(attempts, 2);
    }
}
