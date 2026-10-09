use std::path::PathBuf;
use std::process::Command;

use arcana_core::agent::builtin_tools::project_command::ProjectCommandTool;
use arcana_core::agent::permission::{new_pending_permissions, PermissionMode};
use arcana_core::agent::tool_executor::ToolExecutor;
use arcana_core::agent::tool_registry::ToolRegistry;
use arcana_core::agent::tool_trait::{AgentIdentity, Tool, ToolContext};
use arcana_core::agent::{builtin_tools, PAMode};
use arcana_core::db::tasks::{CreateTaskInput, TasksDb};
use serde_json::Value;

struct IsolatedHome(PathBuf);

impl Drop for IsolatedHome {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove owned project-command profile");
    }
}

struct IsolatedWorkspace(PathBuf);

impl Drop for IsolatedWorkspace {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove owned project-command workspace");
    }
}

fn context(
    session_id: &str,
    call_id: &str,
    working_dir: &std::path::Path,
    task_id: Option<i64>,
    agent_role: &str,
) -> ToolContext {
    ToolContext {
        session_id: session_id.into(),
        working_dir: working_dir.to_string_lossy().into_owned(),
        call_id: call_id.into(),
        run_id: None,
        task_id,
        permission_mode: PermissionMode::Autonomous,
        depth: 0,
        agent_role: agent_role.into(),
        run_token_budget: None,
    }
}

// This integration target contains one test because logging initialization and
// HOME/USERPROFILE are process-global. It proves the production command's
// records survive the JSONL writer and are read back by the shipped CLI.
#[tokio::test]
async fn claimed_python_build_stages_survive_jsonl_writer_and_cli_export() {
    let home = IsolatedHome(std::env::temp_dir().join(format!(
        "arcana-project-command-stage-logs-{}",
        uuid::Uuid::new_v4()
    )));
    std::fs::create_dir(&home.0).expect("create isolated project-command profile");
    std::env::set_var("HOME", &home.0);
    std::env::set_var("USERPROFILE", &home.0);
    for key in [
        "GOOGLE_API_KEY",
        "GEMINI_API_KEY",
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "DATABASE_URL",
    ] {
        std::env::remove_var(key);
    }
    let logging = arcana_core::observability::logging::init();
    assert!(
        logging.writer_count() > 0,
        "the configured JSONL writer must be active"
    );

    let workspace_parent = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("p4-62-stage-log-workspaces");
    std::fs::create_dir_all(&workspace_parent).expect("create fixture workspace parent");
    let workspace = IsolatedWorkspace(workspace_parent.join(uuid::Uuid::new_v4().to_string()));
    std::fs::create_dir(&workspace.0).expect("create selected workspace");
    std::fs::write(
        workspace.0.join("pyproject.toml"),
        "[project]\nname = \"arcana-stage-log-fixture\"\nversion = \"0.1.0\"\n",
    )
    .expect("write Python project manifest");
    std::fs::write(
        workspace.0.join("valid.py"),
        "def valid():\n    return True\n",
    )
    .expect("write valid Python source");

    let tasks = TasksDb::open().expect("open isolated TasksDb");
    let task = tasks
        .create(CreateTaskInput {
            title: "persist project command stage logs".into(),
            description: None,
            priority: "p2".into(),
            task_type: "feature".into(),
            acceptance_criteria:
                "a claimed Python build persists all command stages and build evidence".into(),
            created_by: Some("p4-62-stage-log-integration".into()),
        })
        .expect("create task");
    tasks
        .update_status(task.id, "assigned", Some("worker-backend"))
        .expect("assign task to worker");
    tasks
        .claim_task_with_pid(task.id, "worker-backend", std::process::id())
        .expect("claim task for worker");

    let invalid_context = context(
        "stage-log-session",
        "stage-log-no-claim",
        &workspace.0,
        None,
        "worker-backend",
    );
    let invalid = ProjectCommandTool
        .execute(
            serde_json::json!({"intent": "build", "toolchain": "python"}),
            &invalid_context,
        )
        .await;
    assert!(
        invalid.is_error,
        "a worker without a task claim must be refused"
    );

    let wrong_owner_context = context(
        "stage-log-session",
        "stage-log-wrong-owner",
        &workspace.0,
        Some(task.id),
        "worker-frontend",
    );
    let wrong_owner = ProjectCommandTool
        .execute(
            serde_json::json!({"intent": "build", "toolchain": "python"}),
            &wrong_owner_context,
        )
        .await;
    assert!(
        wrong_owner.is_error,
        "a different worker must not use another worker's task claim"
    );

    let session_id = uuid::Uuid::new_v4().to_string();
    let provider_call_id = format!("stage-log-valid-{}", uuid::Uuid::new_v4());
    let raw_call_id = format!("{session_id}-{provider_call_id}");
    let call_id = arcana_core::observability::logging::telemetry_call_id(&raw_call_id);
    let mut registry = ToolRegistry::new();
    builtin_tools::register_for_mode(&mut registry, PAMode::FastDevelopment);
    let mut executor = ToolExecutor::new(
        PermissionMode::Autonomous,
        new_pending_permissions(),
        session_id,
        workspace.0.to_string_lossy().into_owned(),
        AgentIdentity::worker("worker-backend"),
    );
    executor.set_task_id(Some(task.id));
    let mut results = executor
        .execute_batch(vec![(
            registry
                .get("project_command")
                .expect("actual project_command registration"),
            serde_json::json!({"intent": "build", "toolchain": "python"}),
            provider_call_id.clone(),
        )])
        .await
        .expect("production executor returns the project command result");
    assert_eq!(results.len(), 1);
    let (returned_call_id, result) = results.pop().expect("one build result");
    assert_eq!(returned_call_id, provider_call_id);
    assert!(!result.is_error, "Python build failed: {}", result.output);
    drop(executor);

    let batch = tasks
        .latest_project_build_batch_status(task.id)
        .expect("read persisted build batch")
        .expect("build batch must be persisted");
    assert_eq!(batch.0, "passed");
    assert_eq!((batch.1, batch.2, batch.3), (1, 1, 1));
    drop(tasks);
    drop(logging);

    let exported = home.0.join("project-command-logs-export.txt");
    let output = Command::new(env!("CARGO_BIN_EXE_arcanactl"))
        .args(["--quiet", "logs", "export"])
        .arg(&exported)
        .args(["--component", "arcana-events"])
        .env("HOME", &home.0)
        .env("USERPROFILE", &home.0)
        .output()
        .expect("run actual CLI Logs export reader");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let content = std::fs::read_to_string(exported).expect("read exported JSONL logs");
    assert!(
        !content.contains(&raw_call_id),
        "opaque tool-call identity must not bypass redaction"
    );
    let events = content
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    let stage_events = events
        .iter()
        .filter(|event| {
            event["fields"]["event_type"] == "project_command_stage"
                && event["fields"]["call_id"] == call_id
        })
        .collect::<Vec<_>>();
    let expected_stages = [
        "runtime_prepare",
        "sandbox_prepare",
        "process_wait",
        "output_drain",
        "evidence_persistence",
    ];
    assert_eq!(
        stage_events.len(),
        expected_stages.len(),
        "the CLI reader must return exactly the five persisted stages for this call"
    );
    for stage in expected_stages {
        let matching = stage_events
            .iter()
            .filter(|event| event["fields"]["stage"] == stage)
            .collect::<Vec<_>>();
        assert_eq!(matching.len(), 1, "one exported {stage} event");
        let fields = &matching[0]["fields"];
        assert_eq!(fields["call_id"], call_id);
        assert_eq!(fields["task_id"], task.id.to_string());
        assert_eq!(fields["outcome"], "success");
        assert!(
            fields["duration_ns"].as_u64().is_some(),
            "{stage} must export a nonnegative duration"
        );
    }
    for refused_call_id in ["stage-log-no-claim", "stage-log-wrong-owner"] {
        assert!(
            events.iter().all(|event| {
                event["fields"]["event_type"] != "project_command_stage"
                    || event["fields"]["call_id"]
                        != arcana_core::observability::logging::telemetry_call_id(refused_call_id)
            }),
            "refused call {refused_call_id} must not emit a stage event"
        );
    }
}
