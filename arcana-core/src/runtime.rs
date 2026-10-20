//! Host-independent production runtime composition shared by desktop and CLI.
//!
//! Hosts install their event sink and tracing subscriber before calling
//! [`initialize`]. This module owns the core runtime services both hosts need;
//! it does not start UI monitors or choose host logging and shutdown policy.

use std::sync::{Arc, Mutex, OnceLock};

#[cfg(feature = "offline-fleet-test")]
use crate::provider::{
    types::{
        CompletionRequest, CompletionResponse, MessageContent, ModelInfo, ProviderStatus,
        StreamChunk,
    },
    usage::Usage,
    Provider,
};
#[cfg(feature = "offline-fleet-test")]
use async_trait::async_trait;
#[cfg(feature = "offline-fleet-test")]
use futures::stream;
#[cfg(feature = "offline-fleet-test")]
use serde_json::{json, Value};

use crate::agent::permission::{new_pending_permissions, PendingPermissions};
use crate::analytics::AnalyticsManager;
use crate::errors::{ArcanaError, Result};
use crate::recipes::runner::RecipeRunner;
use crate::recipes::scheduler::RecipeScheduler;
use crate::session::manager::SessionManager;
use crate::state::app_state::{self, AppState};
use crate::state::lock::LockTolerant;
use crate::subagent::manager::SubagentManager;

#[cfg(feature = "offline-fleet-test")]
const OFFLINE_FLEET_SYNTHETIC_USAGE: Usage = Usage {
    input: 64,
    cached_read: 0,
    cache_write: 0,
    output: 16,
    reasoning: 0,
    tool: 0,
};

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

    #[cfg(feature = "offline-fleet-test")]
    let offline_provider: Option<Arc<dyn Provider>> = offline_fleet_test_scenario()?
        .map(|scenario| Arc::new(OfflineFleetProvider { scenario }) as Arc<dyn Provider>);

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
        #[cfg(any(test, feature = "offline-fleet-test"))]
        provider_override: {
            #[cfg(test)]
            {
                None
            }
            #[cfg(all(not(test), feature = "offline-fleet-test"))]
            {
                offline_provider.clone()
            }
        },
    });

    let analytics = Arc::new(AnalyticsManager::new().await?);
    let recipe_runner = RecipeRunner::new_with_workspace_override(
        state.clone(),
        sessions.clone(),
        pending_permissions.clone(),
        working_dir,
    )
    .with_supervisor_budget_override(options.supervisor_budget_override.clone())
    .with_fleet_correlation_id(options.fleet_correlation_id.clone())
    .with_host_approval_available(options.host_approval_available);
    #[cfg(feature = "offline-fleet-test")]
    let recipe_runner = match offline_provider {
        Some(provider) => recipe_runner.with_provider_for_offline_fleet_test(provider),
        None => recipe_runner,
    };
    let recipe_runner = Arc::new(recipe_runner);
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

/// Whether the opt-in CLI test build has a valid disposable-profile marker.
/// Ordinary builds always return false and ignore the test environment.
pub fn offline_fleet_test_active() -> Result<bool> {
    #[cfg(feature = "offline-fleet-test")]
    {
        return offline_fleet_test_scenario().map(|scenario| scenario.is_some());
    }
    #[cfg(not(feature = "offline-fleet-test"))]
    {
        Ok(false)
    }
}

#[cfg(feature = "offline-fleet-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OfflineFleetScenario {
    Success,
    Deadline,
}

#[cfg(feature = "offline-fleet-test")]
const OFFLINE_FLEET_CHILD_TITLE: &str = "Offline Fleet Cargo/QC child";
#[cfg(feature = "offline-fleet-test")]
const OFFLINE_FLEET_SECRET_PATTERN: &str = r"(?i)(password|api_key|secret|token)\s*=";
/// A real, checkable fact the scripted worker proposes so the run exercises the
/// long-term memory gateway and retrieval rather than only task/build tools.
#[cfg(feature = "offline-fleet-test")]
const OFFLINE_FLEET_MEMORY_FACT: &str = "The offline Fleet fixture is built from fleet-fixture with a locked Cargo.lock and no network access.";

#[cfg(feature = "offline-fleet-test")]
fn offline_fleet_test_scenario() -> Result<Option<OfflineFleetScenario>> {
    use std::path::Path;

    let Some(scenario) = std::env::var_os("ARCANA_OFFLINE_FLEET_SCENARIO") else {
        return Ok(None);
    };
    let scenario = scenario.to_string_lossy();
    let scenario = match scenario.as_ref() {
        "success" => OfflineFleetScenario::Success,
        "deadline" => OfflineFleetScenario::Deadline,
        _ => {
            return Err(ArcanaError::Agent(
                "Invalid offline Fleet test scenario".into(),
            ))
        }
    };
    let token = std::env::var("ARCANA_OFFLINE_FLEET_MARKER_ID")
        .map_err(|_| ArcanaError::Agent("Offline Fleet test marker is missing".into()))?;
    if token.len() != 36 || !token.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return Err(ArcanaError::Agent(
            "Offline Fleet test marker is invalid".into(),
        ));
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(std::path::PathBuf::from)
        .map_err(|_| ArcanaError::Agent("Offline Fleet test home is unavailable".into()))?
        .canonicalize()?;
    let temp = std::env::temp_dir().canonicalize()?;
    if !home.starts_with(&temp)
        || !home
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("arcana-cli-fleet-offline-"))
    {
        return Err(ArcanaError::Agent(
            "Offline Fleet test profile is not disposable".into(),
        ));
    }
    let data_dir = Path::new(&home).join(".arcana");
    let marker = data_dir.join("offline-fleet-test.marker");
    for (path, directory) in [(&data_dir, true), (&marker, false)] {
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|_| ArcanaError::Agent("Offline Fleet test marker is missing".into()))?;
        #[cfg(windows)]
        let redirected = {
            use std::os::windows::fs::MetadataExt;
            metadata.file_attributes() & 0x400 != 0
        };
        #[cfg(not(windows))]
        let redirected = metadata.file_type().is_symlink();
        if redirected || (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
            return Err(ArcanaError::Agent(
                "Offline Fleet test profile contains a redirected marker path".into(),
            ));
        }
    }
    let contents = std::fs::read_to_string(marker)
        .map_err(|_| ArcanaError::Agent("Offline Fleet test marker is missing".into()))?;
    if contents.trim() != token {
        return Err(ArcanaError::Agent(
            "Offline Fleet test marker does not match".into(),
        ));
    }
    Ok(Some(scenario))
}

#[cfg(feature = "offline-fleet-test")]
struct OfflineFleetProvider {
    scenario: OfflineFleetScenario,
}

#[cfg(feature = "offline-fleet-test")]
#[async_trait]
impl Provider for OfflineFleetProvider {
    fn id(&self) -> &str {
        "google"
    }
    fn name(&self) -> &str {
        "Offline Fleet Test Provider"
    }
    fn models(&self) -> Vec<ModelInfo> {
        crate::provider::catalogue::models_for("google")
    }
    async fn input_token_upper_bound(
        &self,
        _request: &CompletionRequest,
    ) -> crate::errors::Result<u64> {
        // Scripted turns carry no real prompt token usage. Keep the synthetic
        // reservation inside the same durable run-budget path without asking
        // the production provider default to reserve Google's full context.
        Ok(4_096)
    }
    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> crate::errors::Result<CompletionResponse> {
        Ok(CompletionResponse {
            message_id: "offline-fleet-test".into(),
            content: vec![MessageContent::Text {
                text: "offline scripted response".into(),
            }],
            stop_reason: "stop".into(),
            usage: Usage::default(),
            model: request.model,
        })
    }
    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> crate::errors::Result<crate::provider::traits::StreamResult> {
        let system = request.system.as_deref().unwrap_or_default();
        let prior_tools = request
            .messages
            .iter()
            .flat_map(|message| &message.content)
            .filter_map(|content| {
                if let MessageContent::ToolResult { name, .. } = content {
                    Some(name.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let next = scripted_fleet_tool(&request, self.scenario);
        let actor_code = if system.contains("You are the Arcana Supervisor Agent.") {
            0u8
        } else if system.contains("You are the Arcana QC Dispatcher.") {
            1
        } else if system.contains("You are an AIRS QC Reviewer") {
            2
        } else {
            3
        };
        let step_code = next.as_ref().map_or(0u8, |(name, _)| match name.as_str() {
            "list_tasks" => 1,
            "create_phased_task" => 2,
            "assign_tasks" => 3,
            "claim_task" => 4,
            "project_command" => 5,
            "update_task_status" => 6,
            "list_worker_messages" => 7,
            "spawn_qc_review" => 8,
            "get_task" => 9,
            "read_file" => 10,
            "grep" => 11,
            _ => 0,
        });
        let decision_code = if actor_code == 2 && next.is_none() {
            scripted_reviewer_decision_code(&request)
        } else {
            0
        };
        tracing::info!(
            target: "arcana::telemetry",
            event_type = "offline_fleet_test_step",
            actor_code,
            step_code,
            scenario_code = u8::from(self.scenario == OfflineFleetScenario::Deadline),
            prior_tool_count = prior_tools.len(),
            tool_count = request.tools.len(),
            decision_code,
            "Offline Fleet scripted provider step"
        );
        tracing::warn!(
            target: "arcana::offline_fleet_test",
            scenario = ?self.scenario,
            supervisor = system.contains("You are the Arcana Supervisor Agent."),
            qc_dispatcher = system.contains("You are the Arcana QC Dispatcher."),
            qc_reviewer = system.contains("You are an AIRS QC Reviewer"),
            tool_count = request.tools.len(),
            prior_tool_results = ?prior_tools,
            next_tool = next.as_ref().map(|(name, _)| name.as_str()).unwrap_or("none"),
            "scripted offline Fleet provider received a turn"
        );
        let chunks = match next {
            Some((name, input)) => vec![
                Ok(StreamChunk::ToolUseStart {
                    id: format!("offline-{}", name),
                    name,
                    block_index: 0,
                    opaque: None,
                }),
                Ok(StreamChunk::ToolInputDelta {
                    block_index: 0,
                    partial_json: input.to_string(),
                }),
                Ok(StreamChunk::MessageDelta {
                    stop_reason: "tool_use".into(),
                    usage: OFFLINE_FLEET_SYNTHETIC_USAGE,
                }),
                Ok(StreamChunk::Done),
            ],
            None if scripted_fleet_turn_is_terminal(&request, self.scenario) => vec![
                Ok(StreamChunk::TextDelta {
                    text: "Offline Fleet scenario response.".into(),
                    block_index: 0,
                }),
                Ok(StreamChunk::MessageDelta {
                    stop_reason: "stop".into(),
                    usage: OFFLINE_FLEET_SYNTHETIC_USAGE,
                }),
                Ok(StreamChunk::Done),
            ],
            None => {
                let completed_tools = request
                    .messages
                    .iter()
                    .flat_map(|message| &message.content)
                    .filter_map(|content| {
                        if let MessageContent::ToolResult {
                            name,
                            content,
                            is_error,
                            ..
                        } = content
                        {
                            let result = serde_json::from_str::<Value>(content).ok();
                            let code = result
                                .as_ref()
                                .and_then(|value| value.pointer("/error/code"))
                                .and_then(Value::as_str)
                                .unwrap_or("none");
                            let category = result
                                .as_ref()
                                .and_then(|value| value.pointer("/error/category"))
                                .and_then(Value::as_str)
                                .unwrap_or("none");
                            let diagnostic_codes = result
                                .as_ref()
                                .and_then(|value| value.pointer("/error/diagnostics"))
                                .and_then(Value::as_array)
                                .map(|rows| {
                                    rows.iter()
                                        .filter_map(|row| row.get("code").and_then(Value::as_str))
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default();
                            Some(json!({
                                "name": name,
                                "is_error": is_error,
                                "code": code,
                                "category": category,
                                "diagnostic_codes": diagnostic_codes,
                            }))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                return Err(crate::errors::ArcanaError::Agent(format!(
                    "offline Fleet script did not match turn (supervisor={}, qc_dispatcher={}, qc_reviewer={}, worker={}, tool_results={})",
                    system.contains("You are the Arcana Supervisor Agent."),
                    system.contains("You are the Arcana QC Dispatcher."),
                    system.contains("You are an AIRS QC Reviewer"),
                    is_offline_worker_prompt(system),
                    serde_json::to_string(&completed_tools).unwrap_or_else(|_| "[]".into()),
                )));
            }
        };
        Ok(Box::pin(stream::iter(chunks)))
    }
    async fn test_connection(&self) -> crate::errors::Result<ProviderStatus> {
        Ok(ProviderStatus::Connected)
    }
}

#[cfg(feature = "offline-fleet-test")]
fn scripted_fleet_tool(
    request: &CompletionRequest,
    scenario: OfflineFleetScenario,
) -> Option<(String, Value)> {
    let supervisor = request
        .system
        .as_deref()
        .is_some_and(|text| text.contains("You are the Arcana Supervisor Agent."));
    let results = request
        .messages
        .iter()
        .flat_map(|message| &message.content)
        .filter_map(|content| {
            if let MessageContent::ToolResult {
                name,
                content,
                is_error,
                ..
            } = content
            {
                Some((name.as_str(), content.as_str(), *is_error))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if request
        .system
        .as_deref()
        .is_some_and(|text| text.contains("You are the Arcana QC Dispatcher."))
    {
        let last_protocol_result = results.iter().rfind(|(name, _, _)| {
            matches!(
                *name,
                "list_worker_messages" | "list_tasks" | "spawn_qc_review"
            )
        });
        return match last_protocol_result {
            None => Some(("list_worker_messages".into(), json!({}))),
            Some(("list_worker_messages", content, is_error)) if !is_error => {
                let inbox: Value = serde_json::from_str(content).ok()?;
                if inbox["count"].as_u64()? != 0 || !inbox["messages"].as_array()?.is_empty() {
                    None
                } else {
                    Some((
                        "list_tasks".into(),
                        json!({"status": "qc_review", "compact": false}),
                    ))
                }
            }
            Some(("list_tasks", content, is_error)) if !is_error => {
                let tasks: Value = serde_json::from_str(content).ok()?;
                let task = task_rows(&tasks)?
                    .iter()
                    .find(|task| task["status"] == "qc_review")?;
                Some((
                    "spawn_qc_review".into(),
                    json!({"task_id": task["id"], "timeout_secs": 60}),
                ))
            }
            Some(("spawn_qc_review", _, true))
                if results
                    .iter()
                    .filter(|(name, _, _)| *name == "spawn_qc_review")
                    .count()
                    == 1 =>
            {
                // A failed spawn is reconciled from the durable inbox and task
                // board before one bounded retry; success remains terminal.
                Some(("list_worker_messages".into(), json!({})))
            }
            _ => None,
        };
    }
    if request
        .system
        .as_deref()
        .is_some_and(|text| text.contains("You are an AIRS QC Reviewer"))
    {
        if !results.iter().any(|(name, _, _)| *name == "get_task") {
            let prompt = request.system.as_deref()?;
            let after = prompt.split("Task under review: #").nth(1)?;
            let task_id = after
                .split(|c: char| !c.is_ascii_digit())
                .next()?
                .parse::<i64>()
                .ok()?;
            return Some(("get_task".into(), json!({"id": task_id})));
        }
        let task_result = results.iter().find(|(name, _, _)| *name == "get_task")?;
        if task_result.2 {
            return None;
        }
        let task: Value = serde_json::from_str(task_result.1).ok()?;
        if task["status"] != "qc_review"
            || task["assigned_worker"] != "worker-backend"
            || task["acceptance_criteria"]
                != "The locked Cargo fixture builds and the QC reviewer accepts the task."
        {
            return None;
        }
        if !results.iter().any(|(name, _, _)| *name == "read_file") {
            return Some((
                "read_file".into(),
                json!({"path": "fleet-fixture/Cargo.toml"}),
            ));
        }
        let manifest = results.iter().find(|(name, _, _)| *name == "read_file")?;
        if manifest.2 || !manifest.1.contains("name = \"fleet-fixture\"") {
            return None;
        }
        if results
            .iter()
            .filter(|(name, _, _)| *name == "read_file")
            .count()
            < 2
        {
            return Some((
                "read_file".into(),
                json!({"path": "fleet-fixture/src/lib.rs"}),
            ));
        }
        let source = results
            .iter()
            .filter(|(name, _, _)| *name == "read_file")
            .nth(1)?;
        if source.2 || !source.1.contains("offline_fixture") {
            return None;
        }
        if !results.iter().any(|(name, _, _)| *name == "grep") {
            return Some((
                "grep".into(),
                json!({
                    "pattern": OFFLINE_FLEET_SECRET_PATTERN,
                    "path": "fleet-fixture",
                    "context_lines": 0,
                    "max_results": 20
                }),
            ));
        }
        let security_scan = results.iter().find(|(name, _, _)| *name == "grep")?;
        if !scripted_grep_is_clean(security_scan.1, security_scan.2) {
            return None;
        }
        if !results
            .iter()
            .any(|(name, _, _)| *name == "update_task_status")
        {
            return Some((
                "update_task_status".into(),
                json!({
                    "id": task["id"],
                    "status": "done",
                    "assigned_worker": "worker-backend"
                }),
            ));
        }
        return None;
    }

    if supervisor {
        if !results.iter().any(|(name, _, _)| *name == "list_tasks") {
            return Some(("list_tasks".into(), json!({"compact": false})));
        }
        if scenario == OfflineFleetScenario::Deadline {
            return None;
        }
        if !results
            .iter()
            .any(|(name, _, _)| *name == "create_phased_task")
        {
            let tasks = results.iter().find(|(name, _, _)| *name == "list_tasks")?.1;
            let tasks: Value = serde_json::from_str(tasks).ok()?;
            let epic = task_rows(&tasks)?
                .iter()
                .find(|task| task["task_type"] == "epic" && task["status"] == "ready")?;
            let epic_id = epic["id"].as_i64()?;
            if task_rows(&tasks)?
                .iter()
                .any(|task| task["title"] == OFFLINE_FLEET_CHILD_TITLE)
            {
                // The CLI may wake the Supervisor again as an existing child moves
                // through assignment and QC. Reconcile the durable board before
                // creating work so the test provider stays idempotent like the
                // production Supervisor contract.
                return None;
            }
            return Some((
                "create_phased_task".into(),
                json!({
                    "epic_id": epic_id.to_string(), "phase": "offline-proof", "phase_order": 1,
                    "title": OFFLINE_FLEET_CHILD_TITLE, "description": "Build the locked Cargo fixture through the worker and pass the real QC review.",
                    "priority": "p2", "task_type": "chore", "acceptance_criteria": "The locked Cargo fixture builds and the QC reviewer accepts the task.", "blocked_by": []
                }),
            ));
        }
        if !results.iter().any(|(name, _, _)| *name == "assign_tasks") {
            let created = results
                .iter()
                .find(|(name, _, _)| *name == "create_phased_task")?
                .1;
            let created: Value = serde_json::from_str(created).ok()?;
            let id = created.get("task_id")?.as_i64()?;
            return Some((
                "assign_tasks".into(),
                json!({"assignments": [{
                    "task_id": id,
                    "worker_slug": "worker-backend",
                    "override_reason": "Deterministic offline Fleet integration proof",
                }]}),
            ));
        }
        return None;
    }

    if !results.iter().any(|(name, _, _)| *name == "list_tasks") {
        return Some(("list_tasks".into(), json!({"compact": false})));
    }
    let tasks = results.iter().find(|(name, _, _)| *name == "list_tasks")?.1;
    let tasks: Value = serde_json::from_str(tasks).ok()?;
    let assigned = task_rows(&tasks)?
        .iter()
        .find(|task| task["status"] == "assigned" && task["assigned_worker"].as_str().is_some())?;
    let task_id = assigned["id"].as_i64()?;
    if !results.iter().any(|(name, _, _)| *name == "claim_task") {
        return Some(("claim_task".into(), json!({"task_id": task_id})));
    }
    // Memory gateway + retrieval before the build, so a real Fleet run covers
    // the memory tier and its telemetry, not only task and command work.
    if !results.iter().any(|(name, _, _)| *name == "propose_memory") {
        return Some((
            "propose_memory".into(),
            json!({
                "content": OFFLINE_FLEET_MEMORY_FACT,
                "scope": "project",
                "confidence": 0.5,
                "source_file": "fleet-fixture/Cargo.toml",
                "source_line": 1
            }),
        ));
    }
    if !results.iter().any(|(name, _, _)| *name == "search_memory") {
        return Some((
            "search_memory".into(),
            json!({"query": "offline Fleet fixture locked Cargo.lock", "limit": 5}),
        ));
    }
    if !results
        .iter()
        .any(|(name, _, _)| *name == "project_command")
    {
        return Some((
            "project_command".into(),
            json!({"intent": "build", "toolchain": "cargo", "project_path": "fleet-fixture", "timeout_secs": 60}),
        ));
    }
    let build = results
        .iter()
        .find(|(name, _, _)| *name == "project_command")?;
    if !build.2
        && build.1.contains("Exit code: 0")
        && !results
            .iter()
            .any(|(name, _, _)| *name == "update_task_status")
    {
        return Some((
            "update_task_status".into(),
            json!({
                "id": task_id,
                "status": "qc_review",
                "assigned_worker": "worker-backend"
            }),
        ));
    }
    None
}

#[cfg(feature = "offline-fleet-test")]
fn scripted_fleet_turn_is_terminal(
    request: &CompletionRequest,
    scenario: OfflineFleetScenario,
) -> bool {
    let system = request.system.as_deref().unwrap_or_default();
    let results = request
        .messages
        .iter()
        .flat_map(|message| &message.content)
        .filter_map(|content| {
            if let MessageContent::ToolResult {
                name,
                content,
                is_error,
                ..
            } = content
            {
                Some((name.as_str(), content.as_str(), *is_error))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if system.contains("You are the Arcana Supervisor Agent.") {
        if scenario == OfflineFleetScenario::Deadline
            || results
                .iter()
                .any(|(name, _, is_error)| *name == "assign_tasks" && !is_error)
        {
            return true;
        }
        let Some((_, task_json, is_error)) =
            results.iter().rfind(|(name, _, _)| *name == "list_tasks")
        else {
            return false;
        };
        if *is_error {
            return false;
        }
        let Ok(tasks) = serde_json::from_str::<Value>(task_json) else {
            return false;
        };
        let Some(rows) = task_rows(&tasks) else {
            return false;
        };
        if !rows
            .iter()
            .any(|task| task["task_type"] == "epic" && task["status"] == "ready")
        {
            return false;
        }
        return rows
            .iter()
            .any(|task| task["title"] == OFFLINE_FLEET_CHILD_TITLE);
    }
    if system.contains("You are the Arcana QC Dispatcher.") {
        if let Some((_, _, is_error)) = results
            .iter()
            .rfind(|(name, _, _)| *name == "spawn_qc_review")
        {
            return !*is_error;
        }
        let Some((_, task_json, _)) = results.iter().find(|(name, _, _)| *name == "list_tasks")
        else {
            return false;
        };
        let Ok(tasks) = serde_json::from_str::<Value>(task_json) else {
            return false;
        };
        return !task_rows(&tasks)
            .is_some_and(|tasks| tasks.iter().any(|task| task["status"] == "qc_review"));
    }
    if system.contains("You are an AIRS QC Reviewer") {
        return results
            .iter()
            .rfind(|(name, _, _)| *name == "update_task_status")
            .is_some_and(|(_, _, is_error)| !is_error);
    }
    if is_offline_worker_prompt(system) {
        if results
            .iter()
            .rfind(|(name, _, _)| *name == "update_task_status")
            .is_some_and(|(_, _, is_error)| !is_error)
        {
            return true;
        }
        let Some((_, task_json, _)) = results.iter().find(|(name, _, _)| *name == "list_tasks")
        else {
            return false;
        };
        let Ok(tasks) = serde_json::from_str::<Value>(task_json) else {
            return false;
        };
        return !task_rows(&tasks).is_some_and(|tasks| {
            tasks.iter().any(|task| {
                task["status"] == "assigned" && task["assigned_worker"].as_str().is_some()
            })
        });
    }
    false
}

#[cfg(feature = "offline-fleet-test")]
fn task_rows(value: &Value) -> Option<&Vec<Value>> {
    value.as_array().or_else(|| value.get("tasks")?.as_array())
}

#[cfg(feature = "offline-fleet-test")]
fn scripted_grep_is_clean(content: &str, is_error: bool) -> bool {
    if is_error {
        return false;
    }
    if let Ok(scan) = serde_json::from_str::<Value>(content) {
        return scan["total_matches"].as_u64() == Some(0) && scan["truncated"] == false;
    }
    content.trim().starts_with("No matches found for pattern '")
}

#[cfg(feature = "offline-fleet-test")]
fn scripted_reviewer_decision_code(request: &CompletionRequest) -> u8 {
    let results = request
        .messages
        .iter()
        .flat_map(|message| &message.content)
        .filter_map(|content| {
            if let MessageContent::ToolResult {
                name,
                content,
                is_error,
                ..
            } = content
            {
                Some((name.as_str(), content.as_str(), *is_error))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let Some((_, task_content, task_error)) =
        results.iter().find(|(name, _, _)| *name == "get_task")
    else {
        return 1;
    };
    if *task_error {
        return 2;
    }
    let Ok(task) = serde_json::from_str::<Value>(task_content) else {
        return 3;
    };
    if task["status"] != "qc_review"
        || task["assigned_worker"] != "worker-backend"
        || task["acceptance_criteria"]
            != "The locked Cargo fixture builds and the QC reviewer accepts the task."
    {
        return 4;
    }
    let manifest_count = results
        .iter()
        .filter(|(name, _, _)| *name == "read_file")
        .count();
    if manifest_count == 0 {
        return 5;
    }
    let Some(manifest) = results.iter().find(|(name, _, _)| *name == "read_file") else {
        return 5;
    };
    if manifest.2 || !manifest.1.contains("name = \"fleet-fixture\"") {
        return 6;
    }
    if manifest_count < 2 {
        return 7;
    }
    let Some(source) = results
        .iter()
        .filter(|(name, _, _)| *name == "read_file")
        .nth(1)
    else {
        return 7;
    };
    if source.2 || !source.1.contains("offline_fixture") {
        return 8;
    }
    let Some((_, grep_content, grep_error)) = results.iter().find(|(name, _, _)| *name == "grep")
    else {
        return 9;
    };
    if *grep_error {
        return 10;
    }
    if !scripted_grep_is_clean(grep_content, *grep_error) {
        if let Ok(scan) = serde_json::from_str::<Value>(grep_content) {
            return if scan["total_matches"].as_u64() == Some(0) && scan["truncated"] == false {
                0
            } else {
                12
            };
        }
        return 11;
    }
    if results
        .iter()
        .any(|(name, _, _)| *name == "update_task_status")
    {
        return 13;
    }
    0
}

#[cfg(feature = "offline-fleet-test")]
fn is_offline_worker_prompt(system: &str) -> bool {
    [
        "You are the Arcana Backend Worker Agent.",
        "You are the Arcana Frontend Worker Agent.",
        "You are the Arcana Platform Worker Agent.",
        "You are the Arcana MCP Worker Agent.",
        "You are the Arcana Cleanup Worker Agent.",
    ]
    .iter()
    .any(|role| system.contains(role))
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

    #[cfg(feature = "offline-fleet-test")]
    fn supervisor_request(
        messages: Vec<crate::provider::types::RequestMessage>,
    ) -> crate::provider::types::CompletionRequest {
        crate::provider::types::CompletionRequest {
            model: "gemini-3.8-flash".into(),
            messages,
            system: Some("You are the Arcana Supervisor Agent.".into()),
            tools: Vec::new(),
            max_tokens: 128,
            temperature: None,
            top_p: None,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            cache: crate::provider::usage::CachePolicy::Off,
            clear_tool_uses: None,
        }
    }

    #[cfg(feature = "offline-fleet-test")]
    fn qc_dispatcher_request(
        messages: Vec<crate::provider::types::RequestMessage>,
    ) -> crate::provider::types::CompletionRequest {
        let mut request = supervisor_request(messages);
        request.system = Some("You are the Arcana QC Dispatcher.".into());
        request
    }

    #[cfg(feature = "offline-fleet-test")]
    #[tokio::test]
    async fn offline_fleet_provider_emits_agent_engine_tool_call_round_trip() {
        use crate::provider::{
            traits::Provider,
            types::{MessageContent, RequestMessage, StreamChunk},
            usage::{StopReason, Usage},
        };
        use futures::StreamExt;

        let provider = super::OfflineFleetProvider {
            scenario: super::OfflineFleetScenario::Success,
        };
        let mut stream = provider
            .stream(supervisor_request(Vec::new()))
            .await
            .expect("stream scripted Supervisor turn");
        let mut tool_name = None;
        let mut tool_input = String::new();
        let mut stop_reason = None;
        let mut observed_usage = Usage::default();
        while let Some(chunk) = stream.next().await {
            match chunk.expect("scripted provider chunk") {
                StreamChunk::ToolUseStart { name, .. } => tool_name = Some(name),
                StreamChunk::ToolInputDelta { partial_json, .. } => {
                    tool_input.push_str(&partial_json)
                }
                StreamChunk::MessageDelta {
                    stop_reason: reason,
                    usage,
                } => {
                    stop_reason = Some(reason);
                    observed_usage.add(&usage);
                }
                StreamChunk::TextDelta { .. }
                | StreamChunk::ThinkingDelta { .. }
                | StreamChunk::ThinkingSignature { .. }
                | StreamChunk::Done => {}
            }
        }
        assert_eq!(tool_name.as_deref(), Some("list_tasks"));
        assert_eq!(
            StopReason::parse("google", stop_reason.as_deref().unwrap()),
            StopReason::ToolUse
        );
        assert_eq!(observed_usage.total_input(), 64);
        assert_eq!(observed_usage.output, 16);

        let follow_up = supervisor_request(vec![RequestMessage {
            role: "assistant".into(),
            content: vec![MessageContent::ToolResult {
                tool_use_id: "offline-list_tasks".into(),
                name: "list_tasks".into(),
                content: r#"[{"id":7,"task_type":"epic","status":"ready"}]"#.into(),
                is_error: false,
            }],
        }]);
        let (tool, input) =
            super::scripted_fleet_tool(&follow_up, super::OfflineFleetScenario::Success)
                .expect("script second Supervisor tool call");
        assert_eq!(tool, "create_phased_task");
        assert_eq!(input["epic_id"], "7");

        let existing_child = supervisor_request(vec![RequestMessage {
            role: "assistant".into(),
            content: vec![MessageContent::ToolResult {
                tool_use_id: "offline-list-existing-child".into(),
                name: "list_tasks".into(),
                content: format!(
                    r#"{{"count":2,"tasks":[{{"id":7,"title":"Epic","task_type":"epic","status":"ready"}},{{"id":8,"title":{},"task_type":"chore","status":"qc_review"}}]}}"#,
                    serde_json::to_string(super::OFFLINE_FLEET_CHILD_TITLE).unwrap()
                ),
                is_error: false,
            }],
        }]);
        assert!(
            super::scripted_fleet_tool(&existing_child, super::OfflineFleetScenario::Success)
                .is_none(),
            "a board list without epic_id must not cause duplicate child creation"
        );
        assert!(super::scripted_fleet_turn_is_terminal(
            &existing_child,
            super::OfflineFleetScenario::Success,
        ));
    }

    #[cfg(feature = "offline-fleet-test")]
    #[test]
    fn offline_qc_dispatcher_drains_inbox_and_reports_failed_review_spawns() {
        use crate::provider::types::{MessageContent, RequestMessage};

        let empty = qc_dispatcher_request(Vec::new());
        assert_eq!(
            super::scripted_fleet_tool(&empty, super::OfflineFleetScenario::Success)
                .as_ref()
                .map(|(name, _)| name.as_str()),
            Some("list_worker_messages"),
            "the shipped QC recipe requires inbox drain as its first operation"
        );

        let inbox = RequestMessage {
            role: "assistant".into(),
            content: vec![MessageContent::ToolResult {
                tool_use_id: "offline-inbox".into(),
                name: "list_worker_messages".into(),
                content: r#"{"count":0,"messages":[]}"#.into(),
                is_error: false,
            }],
        };
        let after_inbox = qc_dispatcher_request(vec![inbox.clone()]);
        assert_eq!(
            super::scripted_fleet_tool(&after_inbox, super::OfflineFleetScenario::Success)
                .as_ref()
                .map(|(name, _)| name.as_str()),
            Some("list_tasks")
        );

        let mut after_queue = after_inbox.messages.clone();
        after_queue.push(RequestMessage {
            role: "assistant".into(),
            content: vec![MessageContent::ToolResult {
                tool_use_id: "offline-queue".into(),
                name: "list_tasks".into(),
                content: r#"{"count":1,"tasks":[{"id":7,"status":"qc_review"}]}"#.into(),
                is_error: false,
            }],
        });
        let after_queue = qc_dispatcher_request(after_queue);
        let (name, input) =
            super::scripted_fleet_tool(&after_queue, super::OfflineFleetScenario::Success)
                .expect("dispatch the eligible review task");
        assert_eq!(name, "spawn_qc_review");
        assert_eq!(input["task_id"], 7);

        let failed_spawn = qc_dispatcher_request(vec![RequestMessage {
            role: "assistant".into(),
            content: vec![MessageContent::ToolResult {
                tool_use_id: "offline-review".into(),
                name: "spawn_qc_review".into(),
                content: r#"{"error":{"code":"review_spawn_failed"}}"#.into(),
                is_error: true,
            }],
        }]);
        assert!(!super::scripted_fleet_turn_is_terminal(
            &failed_spawn,
            super::OfflineFleetScenario::Success,
        ));
        let successful_spawn = qc_dispatcher_request(vec![RequestMessage {
            role: "assistant".into(),
            content: vec![MessageContent::ToolResult {
                tool_use_id: "offline-review".into(),
                name: "spawn_qc_review".into(),
                content: r#"{"status":"dispatched"}"#.into(),
                is_error: false,
            }],
        }]);
        assert!(super::scripted_fleet_turn_is_terminal(
            &successful_spawn,
            super::OfflineFleetScenario::Success,
        ));
    }

    #[cfg(feature = "offline-fleet-test")]
    #[test]
    fn offline_reviewer_accepts_grep_tool_plain_text_for_zero_matches() {
        assert!(super::scripted_grep_is_clean(
            "No matches found for pattern '(?i)(password|api_key|secret|token)\\s*='",
            false,
        ));
        assert!(super::scripted_grep_is_clean(
            "No matches found for pattern '(?i)(password|api_key|secret|token)\\\\s*='",
            false,
        ));
        assert!(!super::scripted_grep_is_clean(
            "No matches found for pattern '(?i)(password|api_key|secret|token)\\s*='",
            true,
        ));
        assert!(super::scripted_grep_is_clean(
            r#"{"total_matches":0,"truncated":false}"#,
            false,
        ));
        assert!(!super::scripted_grep_is_clean(
            r#"{"total_matches":1,"truncated":false}"#,
            false,
        ));
    }

    #[cfg(feature = "offline-fleet-test")]
    #[test]
    fn offline_reviewer_diagnostic_codes_are_numeric_and_content_free() {
        use crate::provider::types::{MessageContent, RequestMessage};

        let mut request = supervisor_request(vec![RequestMessage {
            role: "assistant".into(),
            content: vec![
                MessageContent::ToolResult {
                    tool_use_id: "get-task".into(),
                    name: "get_task".into(),
                    content: serde_json::json!({
                        "status": "qc_review",
                        "assigned_worker": "worker-backend",
                        "acceptance_criteria": "The locked Cargo fixture builds and the QC reviewer accepts the task."
                    })
                    .to_string(),
                    is_error: false,
                },
                MessageContent::ToolResult {
                    tool_use_id: "manifest".into(),
                    name: "read_file".into(),
                    content: "name = \"fleet-fixture\"".into(),
                    is_error: false,
                },
                MessageContent::ToolResult {
                    tool_use_id: "source".into(),
                    name: "read_file".into(),
                    content: "pub fn offline_fixture() -> bool { true }".into(),
                    is_error: false,
                },
                MessageContent::ToolResult {
                    tool_use_id: "grep".into(),
                    name: "grep".into(),
                    content: "No matches found for pattern '(?i)(password|api_key|secret|token)\\s*='"
                        .into(),
                    is_error: false,
                },
            ],
        }]);
        request.system = Some("You are an AIRS QC Reviewer".into());
        assert_eq!(super::scripted_reviewer_decision_code(&request), 0);

        let MessageContent::ToolResult { content, .. } = request
            .messages
            .last_mut()
            .unwrap()
            .content
            .last_mut()
            .unwrap()
        else {
            panic!("fixture ends with a grep result")
        };
        *content = "private scan result".into();
        assert_eq!(super::scripted_reviewer_decision_code(&request), 11);
    }

    #[test]
    fn shared_startup_hydrates_persisted_spend_and_discards_yesterday() {
        use super::{hydrate_state, AppState, Arc, Mutex};
        use crate::db::app_settings;

        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let today = chrono::Local::now().date_naive();
        let mut previous_process = AppState::new();
        previous_process.add_cost(0.75, 31, 7);
        drop(previous_process);

        let state = Arc::new(Mutex::new(AppState::new()));
        hydrate_state(&state);
        {
            let restored = state.lock().unwrap();
            assert_eq!(restored.cost_day, Some(today));
            assert_eq!(restored.cost_today_usd, 0.75);
            assert_eq!(restored.tokens_in_today, 31);
            assert_eq!(restored.tokens_out_today, 7);
        }

        app_settings::set("cost_day", "2020-01-01").expect("stamp old counters");
        let restarted = Arc::new(Mutex::new(AppState::new()));
        hydrate_state(&restarted);
        let restored = restarted.lock().unwrap();
        assert_eq!(restored.cost_day, Some(today));
        assert_eq!(restored.cost_today_usd, 0.0);
        assert_eq!(restored.tokens_in_today, 0);
        assert_eq!(restored.tokens_out_today, 0);
    }

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
