use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

use arcana_core::db::tasks::{CreateTaskInput, TasksDb};
use arcana_core::recipes::runner::RecipeRunner;
use arcana_core::session::manager::SessionManager;
use arcana_core::state::AppState;

struct IsolatedHome(PathBuf);

impl Drop for IsolatedHome {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove owned isolated retry profile");
    }
}

// This integration target has one test and its own process: installing the real
// process-global logger and HOME cannot interfere with another test's subscriber.
#[tokio::test]
async fn permanent_worker_startup_event_survives_redaction_and_cli_export() {
    let home = IsolatedHome(std::env::temp_dir().join(format!(
        "arcana-startup-retry-logs-{}",
        uuid::Uuid::new_v4()
    )));
    std::fs::create_dir(&home.0).expect("create isolated retry profile");
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
    let guards = arcana_core::observability::logging::init();
    let tasks = TasksDb::open().expect("open isolated task store");
    let task = tasks
        .create(CreateTaskInput {
            title: "fail before provider work".into(),
            description: None,
            priority: "p2".into(),
            task_type: "feature".into(),
            acceptance_criteria: "safe startup status reaches Logs".into(),
            created_by: Some("startup-observability-test".into()),
        })
        .unwrap();
    tasks
        .update_status(task.id, "assigned", Some("worker-backend"))
        .unwrap();
    let sessions = Arc::new(SessionManager::new().unwrap());
    let recipes = home.0.join(".arcana/recipes");
    arcana_core::recipes::defaults::ensure_default_recipes(&recipes).unwrap();
    // Read the saved shipped recipe through its production loader, not a
    // separately hand-authored test protocol.
    let recipe = arcana_core::recipes::list()
        .expect("load saved recipes")
        .into_iter()
        .find(|recipe| recipe.id == "worker-backend-poll")
        .expect("saved backend recipe");
    let missing = home.0.join("missing-selected-workspace");
    let runner = RecipeRunner::new_with_workspace_override(
        Arc::new(Mutex::new(AppState::default())),
        sessions,
        arcana_core::agent::permission::new_pending_permissions(),
        Some(missing.to_string_lossy().into_owned()),
    );
    assert!(runner.run_recipe(&recipe, "worker-backend").await.is_err());
    let run = arcana_core::db::runs::RunsDb::open()
        .unwrap()
        .latest_worker_tick("worker-backend")
        .unwrap()
        .unwrap();
    assert_eq!(run.phase, "failed");
    assert_eq!(
        tasks.get_by_id(task.id).unwrap().unwrap().status,
        arcana_core::db::tasks::TaskStatus::Assigned
    );
    drop(guards);

    let exported = home.0.join("logs-export.txt");
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
    let content = std::fs::read_to_string(exported).unwrap();
    let retries = content
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|event| event["fields"]["event_type"] == "worker_startup_retry")
        .collect::<Vec<_>>();
    assert_eq!(
        retries.len(),
        1,
        "permanent failure must emit once and never retry"
    );
    let fields = &retries[0]["fields"];
    assert_eq!(fields["worker"], "worker-backend");
    assert_eq!(fields["attempt"], 1);
    assert_eq!(fields["max_attempts"], 4);
    assert_eq!(fields["outcome"], "permanent_failure");
    assert!(fields["elapsed_ms"].as_u64().is_some());
    assert!(
        !content.contains("missing-selected-workspace"),
        "raw workspace error must not leak"
    );
}
