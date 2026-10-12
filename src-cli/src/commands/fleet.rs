//! Production Fleet execution for headless hosts.
//!
//! This deliberately enters through the same core initializer and scheduler
//! as the desktop. It creates a durable epic task, starts the configured
//! runnable worker recipes, and waits on the task database for the actual
//! Supervisor/worker/QC lifecycle to finish.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, Context, Result};
use arcana_core::{
    analytics::AnalyticsManager,
    db::{
        tasks::{Task, TaskStatus, TasksDb},
        workers::{WorkerStatus, WorkersDb},
    },
    runtime::{self, CoreRuntimeOptions},
};
use clap::{Args, Subcommand};
use uuid::Uuid;

use crate::output::{OutputContext, OutputFormat};

#[derive(Subcommand)]
pub enum FleetCmd {
    /// Create an epic and run it through Arcana's production worker scheduler.
    Run(RunArgs),
}

#[derive(Args)]
pub struct RunArgs {
    /// Short task title shown to the Supervisor and worker fleet.
    #[arg(long)]
    pub title: String,

    /// Concrete end-to-end implementation request.
    #[arg(long)]
    pub description: String,

    /// Observable acceptance criteria the fleet must satisfy.
    #[arg(long)]
    pub acceptance_criteria: String,

    /// Maximum wall-clock time to wait for the epic to reach a terminal state.
    #[arg(long, default_value_t = 7200, value_parser = clap::value_parser!(u64).range(1..=86400))]
    pub timeout_secs: u64,

    /// Per-run Supervisor token cap (hard ceiling: 1,000,000).
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=1_000_000))]
    pub supervisor_max_tokens: Option<u64>,

    /// Per-run Supervisor cost cap in USD (hard ceiling: 100).
    #[arg(long)]
    pub supervisor_max_usd: Option<f64>,

    /// Per-run Supervisor turn cap (hard ceiling: 100).
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=100))]
    pub supervisor_max_turns: Option<u32>,
}

pub async fn run(cmd: FleetCmd, ctx: OutputContext) -> Result<()> {
    match cmd {
        FleetCmd::Run(args) => run_epic(args, ctx).await,
    }
}

async fn run_epic(args: RunArgs, ctx: OutputContext) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();
    let supervisor_budget_override = resolve_supervisor_budget(
        args.supervisor_max_tokens,
        args.supervisor_max_usd,
        args.supervisor_max_turns,
    )?;
    let runtime = runtime::initialize(CoreRuntimeOptions {
        provider: ctx.provider.clone(),
        model: ctx.model.clone(),
        working_dir: ctx.working_dir.clone(),
        // Fleet is a long-running host, so it owns the same immediate
        // retention maintenance as the desktop for the lifetime of this run.
        start_analytics_rollup: true,
        supervisor_budget_override,
        fleet_correlation_id: Some(correlation_id.clone()),
        // The CLI has no prompt responder today. Install intents are denied
        // immediately instead of waiting for the desktop's permission timeout.
        host_approval_available: false,
    })
    .await
    .context("Could not initialize Arcana's production runtime")?;
    let _analytics_maintenance = CliAnalyticsMaintenanceGuard(runtime.analytics.clone());
    let offline_test = runtime::offline_fleet_test_active()?;

    let provider_id = runtime
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .active_provider
        .clone()
        .ok_or_else(|| anyhow!("No provider is selected; configure Gemini in Arcana Settings or pass --provider google"))?;
    if !offline_test {
        let key_available = arcana_core::vault::keychain::VaultManager::get(
            &arcana_core::vault::keychain::provider_key(&provider_id),
        )?
        .is_some();
        if !key_available {
            bail!(
                "No API key is stored for provider '{provider_id}'. Configure it in Arcana Settings before running the production Fleet"
            );
        }
    }
    let workspace = ctx
        .working_dir
        .as_deref()
        .ok_or_else(|| {
            anyhow!("Pass --working-dir so Fleet egress is bound to an explicit workspace")
        })?
        .to_owned();
    let workspace = std::path::Path::new(&workspace)
        .canonicalize()
        .context("Could not establish the canonical Fleet workspace")?;
    let destination = arcana_core::egress_consent::provider_destination(&provider_id)?;
    let consent_db = arcana_core::egress_consent::WorkspaceEgressConsentDb::open()
        .context("Could not open Arcana's workspace-consent store")?;
    if !consent_db
        .is_granted(&workspace, &destination)
        .context("Could not check workspace egress consent")?
    {
        bail!(
            "Provider egress is not authorized for this workspace. Grant it explicitly with `arcanactl consent grant --workspace \"{}\" --provider {provider_id}`",
            workspace.display()
        );
    }

    let workers = WorkersDb::open()
        .context("Could not load the Fleet roster")?
        .list()?;
    let startable = workers
        .iter()
        // A fresh roster seeds workers in `created`. They are not routing
        // targets yet, but `start_worker` promotes them to `idle` before the
        // scheduler admits their first turn. This mirrors desktop startup,
        // which starts recipe-bound Created workers, while preserving the
        // rule that task routing itself only targets runnable states.
        .filter(|worker| worker.recipe_id.is_some() && should_start_worker(&worker.status))
        .map(|worker| worker.slug.clone())
        .collect::<Vec<_>>();
    if startable.is_empty() {
        bail!("No non-paused workers with bound recipes are available to run the Fleet");
    }

    // Capture the latest durable Supervisor run before this CLI invocation
    // creates its epic or starts any worker loops. A timestamp comparison is
    // racy: the startup tick may be recorded before the CLI finishes starting
    // all seven schedule loops even though it is the tick processing this run.
    let previous_supervisor_run_id = arcana_core::db::runs::RunsDb::open()
        .context("Could not read the prior Supervisor run state")?
        .latest_worker_tick("supervisor")?
        .map(|tick| tick.run_id);

    let task_db = TasksDb::open().context("Could not open the task board")?;
    let epic = task_db.create_epic(
        &args.title,
        Some(&format!(
            "{}\n\nAcceptance criteria:\n{}",
            args.description, args.acceptance_criteria
        )),
        "arcanactl",
    )?;
    tracing::info!(
        target: "arcana::telemetry",
        event_type = "fleet_cli_run_started",
        correlation_id = %correlation_id,
        task_id = epic.id,
        worker_count = startable.len(),
        timeout_secs = args.timeout_secs,
        "CLI Fleet execution started"
    );
    ctx.success(&format!(
        "Created Fleet epic #{}; starting {} worker recipes",
        epic.id,
        startable.len()
    ));

    for slug in &startable {
        if let Err(error) = runtime.recipe_scheduler.start_worker(slug) {
            runtime.recipe_scheduler.shutdown().await;
            return Err(anyhow!("Could not start worker '{slug}': {error}"));
        }
    }
    // Worker schedules perform an immediate startup scan, but the CLI also
    // issues the durable task outside Tauri's event-to-kick bridge. Explicitly
    // wake the Supervisor after the epic exists; the scheduler's existing
    // in-flight guard coalesces this with any startup scan already underway.
    runtime.recipe_scheduler.kick_worker("supervisor");

    let started = Instant::now();
    let deadline = Duration::from_secs(args.timeout_secs);
    let mut previous_child_snapshot = Vec::new();
    let result = async {
        loop {
            let current = task_db
                .get_by_id(epic.id)?
                .ok_or_else(|| anyhow!("Fleet epic #{} disappeared from the task board", epic.id))?;
            if let Some(timeout_error) = cli_epic_timeout_error(
                &task_db,
                &current,
                started,
                deadline,
                args.timeout_secs,
                &correlation_id,
            )? {
                break Err(timeout_error);
            }
            match current.status {
                arcana_core::db::tasks::TaskStatus::Done => break Ok(current),
                arcana_core::db::tasks::TaskStatus::Failed
                | arcana_core::db::tasks::TaskStatus::Cancelled
                | arcana_core::db::tasks::TaskStatus::BuildFailed
                | arcana_core::db::tasks::TaskStatus::NeedsInfo => {
                    let children = task_db.get_epic_tasks(&epic.id.to_string())?;
                    tracing::warn!(
                        target: "arcana::telemetry",
                        event_type = "fleet_cli_run_blocked",
                        correlation_id = %correlation_id,
                        task_id = epic.id,
                        status = current.status.as_str(),
                        child_count = children.len(),
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "CLI Fleet execution reached a blocked or failed epic state"
                    );
                    break Err(anyhow!(
                        "Fleet epic #{} reached {} after {}s",
                        epic.id,
                        current.status.as_str(),
                        started.elapsed().as_secs()
                    ));
                }
                arcana_core::db::tasks::TaskStatus::Ready => {
                    let children = task_db.get_epic_tasks(&epic.id.to_string())?;
                    let snapshot = children
                        .iter()
                        .map(|task| {
                            (
                                task.id,
                                task.status.as_str().to_string(),
                                task.assigned_worker.clone(),
                                task.phase.clone(),
                                task.blocked_by.clone(),
                            )
                        })
                        .collect::<Vec<_>>();
                    if snapshot != previous_child_snapshot {
                        // The CLI has no Tauri task-event bridge. Wake workers
                        // when decomposition or a child transition changes
                        // runnable work, instead of waiting for their polling
                        // interval after the initial startup scan.
                        for slug in startable.iter().filter(|slug| *slug != "supervisor") {
                            runtime.recipe_scheduler.kick_worker(slug);
                        }
                        previous_child_snapshot = snapshot;
                    }

                    let supervisor = arcana_core::db::runs::RunsDb::open()
                        .context("Could not read the Supervisor's durable run state")?
                        .latest_worker_tick("supervisor")?;
                    let failed_supervisor = supervisor.as_ref().is_some_and(|tick| {
                        is_new_supervisor_failure(
                            &tick.run_id,
                            &tick.phase,
                            previous_supervisor_run_id.as_deref(),
                        )
                    });
                    if failed_supervisor {
                        let failure_code = supervisor
                            .and_then(|tick| tick.failure_code)
                            .unwrap_or_else(|| "unknown".into());
                        let children = task_db.get_epic_tasks(&epic.id.to_string())?;
                        tracing::error!(
                            target: "arcana::telemetry",
                            event_type = "fleet_cli_supervisor_failed",
                            correlation_id = %correlation_id,
                            task_id = epic.id,
                            failure_code = %failure_code,
                            child_count = children.len(),
                            elapsed_ms = started.elapsed().as_millis() as u64,
                            "CLI Fleet Supervisor failed while processing the epic"
                        );
                        break Err(anyhow!(
                            "Fleet Supervisor failed while processing the epic (failure code: {failure_code}, {} child task(s) persisted); inspect the durable task/run state before retrying",
                            children.len()
                        ));
                    }
                }
                _ => {}
            }
            let remaining = deadline.saturating_sub(started.elapsed());
            if !remaining.is_zero() {
                tokio::time::sleep(Duration::from_secs(2).min(remaining)).await;
            }
        }
    }
    .await;

    // Stop persistent scheduler loops on both success and failure. The Tokio
    // host exits after this command, so in-flight tasks are cancelled by host
    // shutdown and the normal worker recovery path reconciles durable claims.
    runtime.recipe_scheduler.shutdown().await;

    match result {
        Ok(epic) => {
            let children = task_db.get_epic_tasks(&epic.id.to_string())?;
            tracing::info!(
                target: "arcana::telemetry",
                event_type = "fleet_cli_run_completed",
                correlation_id = %correlation_id,
                task_id = epic.id,
                child_count = children.len(),
                elapsed_ms = started.elapsed().as_millis() as u64,
                "CLI Fleet execution completed"
            );
            ctx.success(&format!(
                "Fleet epic #{} completed with {} child task(s) in {}s",
                epic.id,
                children.len(),
                started.elapsed().as_secs()
            ));
            if ctx.format == OutputFormat::Json {
                ctx.print_json(&serde_json::json!({
                    "task_id": epic.id,
                    "status": epic.status.as_str(),
                    "child_tasks": children,
                    "elapsed_ms": started.elapsed().as_millis(),
                    "correlation_id": correlation_id,
                }));
            } else {
                ctx.print_table(
                    &["ID", "Title", "Status", "Worker", "Phase"],
                    children
                        .iter()
                        .map(|task| {
                            vec![
                                task.id.to_string(),
                                task.title.clone(),
                                task.status.as_str().to_owned(),
                                task.assigned_worker.clone().unwrap_or_else(|| "-".into()),
                                task.phase.clone().unwrap_or_else(|| "-".into()),
                            ]
                        })
                        .collect(),
                );
            }
            Ok(())
        }
        Err(error) => Err(error),
    }
}

struct CliAnalyticsMaintenanceGuard(Arc<AnalyticsManager>);

impl Drop for CliAnalyticsMaintenanceGuard {
    fn drop(&mut self) {
        self.0.stop_aggregation_job();
    }
}

fn cli_epic_timeout_error(
    task_db: &TasksDb,
    epic: &Task,
    started: Instant,
    deadline: Duration,
    timeout_secs: u64,
    correlation_id: &str,
) -> Result<Option<anyhow::Error>> {
    let terminal = matches!(
        epic.status,
        TaskStatus::Done
            | TaskStatus::Failed
            | TaskStatus::Cancelled
            | TaskStatus::BuildFailed
            | TaskStatus::NeedsInfo
    );
    if terminal || started.elapsed() < deadline {
        return Ok(None);
    }

    let children = task_db.get_epic_tasks(&epic.id.to_string())?;
    tracing::error!(
        target: "arcana::telemetry",
        event_type = "fleet_cli_run_timeout",
        correlation_id = %correlation_id,
        task_id = epic.id,
        status = epic.status.as_str(),
        child_count = children.len(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        "CLI Fleet execution reached its bounded deadline"
    );
    Ok(Some(anyhow!(
        "Fleet epic #{} did not finish within {}s; its durable task and worker state were preserved",
        epic.id,
        timeout_secs
    )))
}

#[cfg(test)]
mod run_correlation_tests {
    use super::{
        cli_epic_timeout_error, is_new_supervisor_failure, is_runnable_worker, should_start_worker,
    };
    use arcana_core::db::tasks::{TaskStatus, TasksDb};
    use std::time::{Duration, Instant};

    #[test]
    fn cli_failure_detection_uses_durable_run_identity_not_clock_order() {
        assert!(is_new_supervisor_failure(
            "new-run",
            "failed",
            Some("old-run")
        ));
        assert!(!is_new_supervisor_failure(
            "old-run",
            "failed",
            Some("old-run")
        ));
        assert!(!is_new_supervisor_failure(
            "new-run",
            "running",
            Some("old-run")
        ));
        assert!(is_new_supervisor_failure("new-run", "failed", None));
    }

    #[test]
    fn fleet_start_excludes_workers_that_are_not_runnable() {
        use arcana_core::db::workers::WorkerStatus;

        for status in [
            WorkerStatus::Idle,
            WorkerStatus::Active,
            WorkerStatus::IdleNoProvider,
            WorkerStatus::Error,
        ] {
            assert!(is_runnable_worker(&status));
        }
        for status in [
            WorkerStatus::Created,
            WorkerStatus::Working,
            WorkerStatus::Paused,
            WorkerStatus::Deleted,
        ] {
            assert!(!is_runnable_worker(&status));
        }
    }

    #[test]
    fn fleet_bootstraps_created_workers_before_they_become_routing_targets() {
        use arcana_core::db::workers::WorkerStatus;

        assert!(should_start_worker(&WorkerStatus::Created));
        assert!(!is_runnable_worker(&WorkerStatus::Created));
        assert!(is_runnable_worker(&WorkerStatus::Idle));
    }

    #[test]
    fn ready_epic_expires_at_cli_deadline_without_losing_durable_state() {
        let directory = std::env::temp_dir().join(format!(
            "arcana-cli-fleet-deadline-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&directory).expect("create unique test-owned directory");
        let db = TasksDb::open_at(&directory.join("tasks.db")).expect("open isolated task DB");
        let epic = db
            .create_epic("bounded CLI run", Some("test deadline"), "test")
            .expect("create durable epic");
        assert_eq!(epic.status, TaskStatus::Ready);

        let error = cli_epic_timeout_error(
            &db,
            &epic,
            Instant::now() - Duration::from_secs(2),
            Duration::from_secs(1),
            1,
            "deadline-test-correlation",
        )
        .expect("query child state")
        .expect("ready epic must expire");
        assert!(error
            .to_string()
            .contains("durable task and worker state were preserved"));
        assert_eq!(
            db.get_by_id(epic.id).unwrap().unwrap().status,
            TaskStatus::Ready
        );
        assert!(db.get_epic_tasks(&epic.id.to_string()).unwrap().is_empty());

        let done = TaskStatus::Done;
        assert!(cli_epic_timeout_error(
            &db,
            &arcana_core::db::tasks::Task {
                status: done,
                ..epic
            },
            Instant::now() - Duration::from_secs(2),
            Duration::from_secs(1),
            1,
            "deadline-test-correlation",
        )
        .unwrap()
        .is_none());

        drop(db);
        std::fs::remove_dir_all(directory).expect("remove test-owned temporary directory");
    }
}

fn is_new_supervisor_failure(run_id: &str, phase: &str, previous_run_id: Option<&str>) -> bool {
    phase == "failed" && previous_run_id != Some(run_id)
}

fn is_runnable_worker(status: &WorkerStatus) -> bool {
    matches!(
        status,
        WorkerStatus::Idle
            | WorkerStatus::Active
            | WorkerStatus::IdleNoProvider
            | WorkerStatus::Error
    )
}

fn should_start_worker(status: &WorkerStatus) -> bool {
    matches!(status, WorkerStatus::Created) || is_runnable_worker(status)
}

fn resolve_supervisor_budget(
    max_tokens: Option<u64>,
    max_usd: Option<f64>,
    max_turns: Option<u32>,
) -> Result<Option<arcana_core::budget::TaskBudget>> {
    if max_tokens.is_none() && max_usd.is_none() && max_turns.is_none() {
        return Ok(None);
    }
    let defaults = arcana_core::budget::TaskBudget::default_supervisor();
    let budget = arcana_core::budget::validate_budget(
        max_tokens.or(defaults.max_tokens),
        max_usd.or(defaults.max_usd),
        max_turns.or(defaults.max_turns),
    )
    .map_err(|code| anyhow!("Invalid Supervisor run budget: {code}"))?;
    Ok(Some(budget))
}

#[cfg(test)]
mod tests {
    use super::resolve_supervisor_budget;

    #[test]
    fn cli_budget_override_changes_only_requested_caps_and_is_bounded() {
        assert_eq!(resolve_supervisor_budget(None, None, None).unwrap(), None);
        let budget = resolve_supervisor_budget(Some(180_000), Some(2.0), Some(30))
            .unwrap()
            .unwrap();
        assert_eq!(budget.max_tokens, Some(180_000));
        assert_eq!(budget.max_usd, Some(2.0));
        assert_eq!(budget.max_turns, Some(30));

        let partial = resolve_supervisor_budget(Some(90_000), None, None)
            .unwrap()
            .unwrap();
        assert_eq!(partial.max_tokens, Some(90_000));
        assert_eq!(partial.max_usd, Some(0.25));
        assert_eq!(partial.max_turns, Some(10));
        assert!(resolve_supervisor_budget(Some(1_000_001), None, None).is_err());
        assert!(resolve_supervisor_budget(None, Some(101.0), None).is_err());
        assert!(resolve_supervisor_budget(None, None, Some(101)).is_err());
    }
}
