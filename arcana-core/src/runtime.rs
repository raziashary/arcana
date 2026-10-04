//! Host-independent production runtime composition shared by desktop and CLI.
//!
//! Hosts install their event sink and tracing subscriber before calling
//! [`initialize`]. This module owns the core runtime services both hosts need;
//! it does not start UI monitors or choose host logging and shutdown policy.

use std::sync::{Arc, Mutex, OnceLock};

use crate::agent::permission::{new_pending_permissions, PendingPermissions};
use crate::analytics::AnalyticsManager;
use crate::errors::{ArcanaError, Result};
use crate::recipes::runner::RecipeRunner;
use crate::recipes::scheduler::RecipeScheduler;
use crate::session::manager::SessionManager;
use crate::state::app_state::{self, AppState};
use crate::state::lock::LockTolerant;
use crate::subagent::manager::SubagentManager;

/// The core-owned services initialized for one Arcana process.
///
/// The desktop and CLI retain this value for their process lifetime. The
/// process-wide accessors used by core tools point to the corresponding
/// managers after [`initialize`] succeeds.
pub struct CoreRuntime {
    pub state: Arc<Mutex<AppState>>,
    pub sessions: Arc<SessionManager>,
    pub pending_permissions: PendingPermissions,
    pub subagents: Arc<SubagentManager>,
    pub analytics: Arc<AnalyticsManager>,
    pub recipe_runner: Arc<RecipeRunner>,
    pub recipe_scheduler: Arc<RecipeScheduler>,
    options: CoreRuntimeOptions,
}

/// Optional host overrides applied only to this process's runtime.
///
/// The provider and model values are validated against the shipped catalogue.
/// An explicit working directory is canonicalized and must already exist.
/// None of these overrides are persisted.
#[derive(Clone, Debug, PartialEq)]
pub struct CoreRuntimeOptions {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub working_dir: Option<String>,
    /// Whether the host owns the long-lived daily analytics rollup thread.
    /// Bounded one-shot hosts should disable this background service.
    pub start_analytics_rollup: bool,
    /// Per-host override for the Supervisor's bounded run budget. Worker and
    /// saved task budgets remain independent and can impose stricter limits.
    pub supervisor_budget_override: Option<crate::budget::TaskBudget>,
    /// Optional correlation ID for one host-launched Fleet run. It is threaded
    /// into worker traces so CLI and desktop lifecycle events can be followed
    /// through provider, tool, and command work.
    pub fleet_correlation_id: Option<String>,
    /// Whether this process has a live host capable of answering interactive
    /// per-call approval prompts. Desktop builds do; headless hosts must fail
    /// closed without waiting for a UI responder that does not exist.
    pub host_approval_available: bool,
}

impl Default for CoreRuntimeOptions {
    fn default() -> Self {
        Self {
            provider: None,
            model: None,
            working_dir: None,
            start_analytics_rollup: true,
            supervisor_budget_override: None,
            fleet_correlation_id: None,
            host_approval_available: true,
        }
    }
}

static CORE_RUNTIME: OnceLock<Arc<CoreRuntime>> = OnceLock::new();
static INITIALIZATION_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Initialize Arcana's shared production runtime.
///
/// Call after the host installs its event sink and tracing subscriber, from
/// inside an active Tokio runtime. Initialization is idempotent within the
/// process: concurrent or subsequent callers receive the same handles.
///
/// Core-owned recipe defaults are seeded on a best-effort basis, matching the
/// desktop's existing startup policy. Skills remain host-owned because their
/// registry and default files currently live in the Tauri crate.
pub async fn initialize(options: CoreRuntimeOptions) -> Result<Arc<CoreRuntime>> {
    let _guard = INITIALIZATION_LOCK.lock().await;
    if let Some(runtime) = CORE_RUNTIME.get() {
        if runtime.options != options {
            return Err(ArcanaError::Agent(
                "Arcana core runtime is already initialized with different host overrides".into(),
            ));
        }
        return Ok(runtime.clone());
    }

    let working_dir = options
        .working_dir
        .as_deref()
        .map(|path| crate::platform::workspace::resolve_workspace_dir(Some(path)))
        .transpose()?
        .map(|path| path.to_string_lossy().into_owned());
    let selection = resolve_selection(options.provider.as_deref(), options.model.as_deref())?;

    if let Err(error) = crate::recipes::init() {
        tracing::warn!("Failed to initialise recipes: {error}");
    }

    let state = Arc::new(Mutex::new(AppState::new()));
    hydrate_state(&state);
    if let Some((provider, model)) = selection {
        let mut state = state.lock_tolerant();
        state.active_provider = Some(provider);
        state.active_model = Some(model);
    }

    let sessions = Arc::new(SessionManager::new()?);
    let pending_permissions = new_pending_permissions();

    let subagents = SubagentManager::new(crate::subagent::native_runner::RunnerDeps {
        state: state.clone(),
        pending: pending_permissions.clone(),
    });

    let analytics = Arc::new(AnalyticsManager::new().await?);
    let recipe_runner = Arc::new(
        RecipeRunner::new_with_workspace_override(
            state.clone(),
            sessions.clone(),
            pending_permissions.clone(),
            working_dir,
        )
        .with_supervisor_budget_override(options.supervisor_budget_override.clone())
        .with_fleet_correlation_id(options.fleet_correlation_id.clone())
        .with_host_approval_available(options.host_approval_available),
    );
    let recipe_scheduler = Arc::new(RecipeScheduler::new(recipe_runner.clone()));

    // Publish globals only after all fallible database construction succeeds.
    crate::subagent::manager::install_global(subagents.clone());
    crate::analytics::install_global(analytics.clone());
    crate::recipes::scheduler::install_global(recipe_scheduler.clone());
    app_state::set_global(state.clone());
    if options.start_analytics_rollup {
        analytics.start_aggregation_job();
    }

    let runtime = Arc::new(CoreRuntime {
        state,
        sessions,
        pending_permissions,
        subagents,
        analytics,
        recipe_runner,
        recipe_scheduler,
        options,
    });

    // INITIALIZATION_LOCK prevents another initializer from racing this set.
    let _ = CORE_RUNTIME.set(runtime.clone());
    Ok(runtime)
}

fn resolve_selection(
    provider_override: Option<&str>,
    model_override: Option<&str>,
) -> Result<Option<(String, String)>> {
    let provider_override = provider_override
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let model_override = model_override
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if provider_override.is_none() && model_override.is_none() {
        return Ok(None);
    }

    let model_provider = model_override.and_then(crate::provider::routing::provider_for_model);
    if let Some(model) = model_override {
        if model_provider.is_none() || crate::provider::catalogue::entry(model).is_none() {
            return Err(ArcanaError::Agent(format!(
                "Unknown provider model override '{model}'"
            )));
        }
    }

    let provider = match (provider_override, model_provider) {
        (Some(provider), Some(model_provider)) if provider != model_provider => {
            return Err(ArcanaError::Agent(format!(
                "Model override does not belong to provider '{provider}'"
            )));
        }
        (Some(provider), _) => provider,
        (None, Some(provider)) => provider,
        (None, None) => unreachable!("an override was checked above"),
    };

    if crate::provider::catalogue::default_model_for(provider).is_none() {
        return Err(ArcanaError::Agent(format!(
            "Unknown provider override '{provider}'"
        )));
    }
    let model = model_override
        .map(str::to_owned)
        .or_else(|| crate::provider::catalogue::default_model_for(provider).map(str::to_owned))
        .expect("every supported provider has a catalogue default");
    Ok(Some((provider.to_owned(), model)))
}

const ACTIVE_PROVIDER_KEY: &str = "active_provider";
const ACTIVE_MODEL_KEY: &str = "active_model";

fn hydrate_state(state: &Arc<Mutex<AppState>>) {
    {
        let mut state = state.lock_tolerant();
        state.hydrate_today();
    }

    let provider = crate::db::app_settings::get(ACTIVE_PROVIDER_KEY)
        .ok()
        .flatten();
    let model = crate::db::app_settings::get(ACTIVE_MODEL_KEY)
        .ok()
        .flatten();

    let (provider, model) = match (provider, model) {
        (Some(provider), Some(model)) => (provider, model),
        (None, None) => {
            let mut state = state.lock_tolerant();
            state.active_provider = Some("google".into());
            state.active_model = Some("gemini-3.8-flash".into());
            return;
        }
        _ => return,
    };

    // Retain the desktop migration from the former Google default while
    // preserving any explicit model choice.
    let model = if provider == "google" && model == "gemini-3.7-flash" {
        if let Err(error) = crate::db::app_settings::set(ACTIVE_MODEL_KEY, "gemini-3.8-flash") {
            tracing::warn!("Could not update the saved Google model default: {error}");
            model
        } else {
            "gemini-3.8-flash".to_owned()
        }
    } else {
        model
    };

    if crate::provider::catalogue::entry(&model).is_none() {
        tracing::warn!(
            "The saved model '{model}' is no longer offered, so it has not been restored. Choose a model in Settings."
        );
        return;
    }

    if crate::provider::routing::provider_for_model(&model) != Some(provider.as_str()) {
        tracing::warn!(
            "The saved selection ({provider} / {model}) is inconsistent: that model is not served by that provider, so it has not been restored."
        );
        return;
    }

    let mut state = state.lock_tolerant();
    state.active_provider = Some(provider);
    state.active_model = Some(model);
}

#[cfg(test)]
mod tests {
    use super::resolve_selection;

    #[test]
    fn no_overrides_leave_the_hydrated_desktop_selection_unchanged() {
        assert_eq!(resolve_selection(None, None).unwrap(), None);
    }

    #[test]
    fn provider_override_selects_its_catalogue_default() {
        assert_eq!(
            resolve_selection(Some("openai"), None).unwrap(),
            Some(("openai".into(), "gpt-5.5".into()))
        );
    }

    #[test]
    fn model_override_selects_its_provider_without_persisting() {
        assert_eq!(
            resolve_selection(None, Some("gemini-3.8-flash")).unwrap(),
            Some(("google".into(), "gemini-3.8-flash".into()))
        );
    }

    #[test]
    fn mismatched_or_unknown_overrides_fail_closed() {
        assert!(resolve_selection(Some("anthropic"), Some("gpt-4.1")).is_err());
        assert!(resolve_selection(Some("not-a-provider"), None).is_err());
        assert!(resolve_selection(None, Some("retired-model")).is_err());
    }
}
