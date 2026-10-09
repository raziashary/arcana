use super::*;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::agent::permission::{new_pending_permissions, PermissionMode};
use crate::agent::tool_executor::ToolExecutor;
use crate::agent::tool_registry::ToolRegistry;
use crate::db::tasks::{CreateTaskInput, TasksDb};
use crate::provider::traits::StreamResult;
use crate::provider::types::{
    CompletionRequest, CompletionResponse, MessageContent, ModelInfo, ProviderStatus, StreamChunk,
};
use crate::provider::usage::Usage;

struct NativeFixtureProvider {
    task_id: i64,
    requests: Mutex<Vec<CompletionRequest>>,
}

#[async_trait]
impl Provider for NativeFixtureProvider {
    fn id(&self) -> &str {
        "google"
    }
    fn name(&self) -> &str {
        "Native child fixture"
    }
    fn models(&self) -> Vec<ModelInfo> {
        crate::provider::catalogue::models_for("google")
    }
    async fn input_token_upper_bound(&self, _: &CompletionRequest) -> Result<u64> {
        Ok(10)
    }
    async fn complete(&self, _: CompletionRequest) -> Result<CompletionResponse> {
        unreachable!("native AgentEngine must stream")
    }
    async fn test_connection(&self) -> Result<ProviderStatus> {
        unreachable!("isolated native runner must not validate credentials")
    }
    async fn stream(&self, request: CompletionRequest) -> Result<StreamResult> {
        assert_eq!(request.model, "gemini-3.8-flash");
        let qc = request.messages.iter().flat_map(|message| &message.content).any(|content| {
            matches!(content, MessageContent::Text { text } if text.contains("Begin QC review of task #"))
        });
        let task_result = request
            .messages
            .iter()
            .flat_map(|message| &message.content)
            .find_map(|content| match content {
                MessageContent::ToolResult {
                    name,
                    content,
                    is_error,
                    ..
                } if name == "get_task" => {
                    assert!(!is_error, "actual child get_task failed: {content}");
                    Some(
                        serde_json::from_str::<Value>(content)
                            .expect("real task reader returns JSON"),
                    )
                }
                _ => None,
            });
        if let Some(task) = &task_result {
            assert_eq!(task["id"], self.task_id);
            assert_eq!(task["status"], "qc_review");
        }
        let mut chunks = if qc && task_result.is_none() {
            assert!(request.tools.iter().any(|tool| tool.name == "get_task"));
            vec![
                StreamChunk::ToolUseStart {
                    id: "qc-task-read".into(),
                    name: "get_task".into(),
                    block_index: 0,
                    opaque: None,
                },
                StreamChunk::ToolInputDelta {
                    block_index: 0,
                    partial_json: json!({"id":self.task_id}).to_string(),
                },
            ]
        } else {
            vec![StreamChunk::TextDelta {
                text: "native child completed".into(),
                block_index: 0,
            }]
        };
        chunks.push(StreamChunk::MessageDelta {
            stop_reason: if qc && task_result.is_none() {
                "tool_use"
            } else {
                "end_turn"
            }
            .into(),
            usage: Usage {
                input: 10,
                output: 3,
                ..Usage::default()
            },
        });
        chunks.push(StreamChunk::Done);
        self.requests.lock().unwrap().push(request);
        Ok(Box::pin(futures::stream::iter(chunks.into_iter().map(Ok))))
    }
}

// Install process-global manager/analytics only in an explicit fresh invocation.
// Ordinary no-manager negative fixtures must never run alongside this test.
#[tokio::test]
#[ignore = "requires an isolated test process for native manager OnceLock"]
async fn registered_subagent_tools_execute_children_and_read_back_qc_accounting() {
    let home = tempfile::tempdir().unwrap();
    let _home = crate::db::test_support::with_home(home.path());
    let analytics = Arc::new(crate::analytics::AnalyticsManager::new().await.unwrap());
    crate::analytics::install_global(analytics);
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(
        workspace.path().join("Cargo.toml"),
        "[workspace]\nmembers=[]\n",
    )
    .unwrap();
    let workspace_path = workspace.path().canonicalize().unwrap();
    crate::egress_consent::WorkspaceEgressConsentDb::open()
        .unwrap()
        .grant(&workspace_path, "provider:google")
        .unwrap();

    let db = TasksDb::open().unwrap();
    let task = db
        .create(CreateTaskInput {
            title: "Native QC reader fixture".into(),
            description: None,
            priority: "p2".into(),
            task_type: "feature".into(),
            acceptance_criteria: "Read actual task and release reviewer claim".into(),
            created_by: Some("native-fixture".into()),
        })
        .unwrap();
    db.update_status(task.id, "assigned", Some("worker-fixture"))
        .unwrap();
    db.claim_task_with_pid(task.id, "worker-fixture", std::process::id())
        .unwrap();
    // Synthetic passing evidence prepares QC tool inputs only. It is not a
    // compiler execution proof; project-command integration owns that gate.
    let path = workspace_path.to_string_lossy().into_owned();
    let fingerprint = crate::platform::workspace_fingerprint::fingerprint(&workspace_path).unwrap();
    let batch = db
        .begin_project_build_batch_for_worker(
            "worker-fixture",
            "native-fixture",
            1,
            &path,
            Some(&fingerprint),
        )
        .unwrap()
        .unwrap();
    db.finish_project_build_batch_for_worker(
        batch.id,
        "worker-fixture",
        "native-fixture",
        &[(
            "cargo build".into(),
            0,
            "cargo".into(),
            Some(path.clone()),
            Some(fingerprint),
        )],
        None,
        true,
    )
    .unwrap()
    .unwrap();
    db.update_status_with_workspace_and_run_id(
        task.id,
        "qc_review",
        Some("worker-fixture"),
        None,
        None,
        &workspace_path,
    )
    .unwrap();

    let provider = Arc::new(NativeFixtureProvider {
        task_id: task.id,
        requests: Mutex::new(Vec::new()),
    });
    let mut state = AppState::new();
    state.active_provider = Some("google".into());
    state.active_model = Some("gemini-3.8-flash".into());
    let manager = crate::subagent::SubagentManager::new(RunnerDeps {
        state: Arc::new(Mutex::new(state)),
        pending: new_pending_permissions(),
        provider_override: Some(provider.clone()),
    });
    assert!(crate::subagent::manager::global().is_none());
    crate::subagent::manager::install_global(manager.clone());
    let mut registry = ToolRegistry::new();
    crate::agent::builtin_tools::register_for_mode(&mut registry, PAMode::FastDevelopment);
    let mut executor = ToolExecutor::new(
        PermissionMode::Autonomous,
        new_pending_permissions(),
        "native-parent".into(),
        path,
        AgentIdentity::supervisor(),
    );
    let cases = [
        (
            "spawn_subagent",
            json!({"task_description":"Finish a focused child", "tools":["get_task"], "max_turns":3, "timeout_secs":15}),
        ),
        (
            "spawn_parallel_subagents",
            json!({"subtasks":[{"task_description":"First child"},{"task_description":"Second child"}], "timeout_secs":15}),
        ),
        (
            "spawn_qc_review",
            json!({"task_id":task.id, "timeout_secs":15}),
        ),
    ];
    let mut child_ids = Vec::new();
    for (name, input) in cases {
        let call_id = format!("native-{name}");
        let result = tokio::time::timeout(
            Duration::from_secs(25),
            executor.execute_batch(vec![(registry.get(name).unwrap(), input, call_id.clone())]),
        )
        .await
        .unwrap()
        .unwrap()
        .pop()
        .unwrap();
        assert_eq!(result.0, call_id);
        assert!(!result.1.is_error, "{name}: {}", result.1.output);
        let value: Value = serde_json::from_str(&result.1.output).unwrap();
        match name {
            "spawn_subagent" => {
                assert_eq!(value["status"], "completed");
                assert_eq!(value["result_summary"], "native child completed");
                assert_eq!(value["tokens_in"], 10);
                assert_eq!(value["tokens_out"], 3);
                assert!(value["cost_usd"].as_f64().unwrap() > 0.0);
                child_ids.push(value["subagent_id"].as_str().unwrap().to_string());
            }
            "spawn_parallel_subagents" => {
                let results = value["results"].as_array().unwrap();
                assert_eq!(results.len(), 2);
                for child in results {
                    assert_eq!(child["status"], "completed");
                    assert_eq!(child["result_summary"], "native child completed");
                    child_ids.push(child["subagent_id"].as_str().unwrap().to_string());
                }
                assert_eq!(value["total_tokens_in"], 20);
                assert_eq!(value["total_tokens_out"], 6);
            }
            _ => {
                assert_eq!(value["status"], "dispatched");
                let id = value["subagent_id"].as_str().unwrap().to_string();
                tokio::time::timeout(Duration::from_secs(20), async {
                    loop {
                        let persisted = db.get_by_id(task.id).unwrap().unwrap();
                        if manager.get_status(&id).await.unwrap() == SubagentStatus::Completed
                            && persisted.under_review_by.is_none()
                            && persisted.task_qc_cost_usd > 0.0
                        {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
                .await
                .expect(
                    "QC background completion must release claim and attribute real child cost",
                );
                child_ids.push(id);
            }
        }
    }
    let summaries = manager.list(Some("native-parent")).await;
    assert_eq!(summaries.len(), 4);
    for id in child_ids {
        assert!(summaries
            .iter()
            .any(|child| child.id == id && child.status == SubagentStatus::Completed));
    }
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 5, "one single, two parallel, two QC turns");
    assert!(requests.iter().any(|request| request.messages.iter().flat_map(|message| &message.content)
        .any(|content| matches!(content, MessageContent::ToolResult { name, is_error: false, .. } if name == "get_task"))));
}
