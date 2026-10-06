// annotate_file — MCP tool for the QC Agent and other fleet workers.
// Writes an inline review note to a specific line range in a workspace file.
// Notes are persisted in annotations.db and rendered as gutter decorations
// in the IDE CodeViewer when the user opens the file.

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::annotations::{AnnotationsDb, CreateAnnotationInput};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::{Component, Path};

pub struct AnnotateFileTool;

#[async_trait]
impl Tool for AnnotateFileTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Write
    }

    fn name(&self) -> &str {
        "annotate_file"
    }

    fn description(&self) -> &str {
        "Pin an inline review note to a specific line range in a source file. The note appears \
        as a gutter decoration in the IDE code viewer so the user can see exactly which lines \
        triggered a QC finding or review comment. Used by QC Agent to attach rejection reasons \
        directly to the code rather than only in the task card."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Workspace-relative path to the file (e.g. 'src/components/Button.tsx')"
                },
                "start_line": {
                    "type": "integer",
                    "description": "First line of the annotated range (1-indexed)"
                },
                "end_line": {
                    "type": "integer",
                    "description": "Last line of the annotated range (1-indexed, inclusive). Use same as start_line for a single line."
                },
                "note": {
                    "type": "string",
                    "description": "The review finding or comment text (max 500 chars recommended)"
                },
                "severity": {
                    "type": "string",
                    "enum": ["info", "warning", "blocking"],
                    "description": "'blocking' for rejections, 'warning' for non-blocking concerns, 'info' for observations"
                }
            },
            "required": ["file_path", "start_line", "end_line", "note", "severity"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let file_path = match input["file_path"].as_str() {
            Some(p) if !p.is_empty() => p.to_string(),
            _ => {
                return ToolResult::error("'file_path' is required and must be a non-empty string")
            }
        };
        let file_path = match resolve_annotation_path(&file_path, &ctx.working_dir) {
            Ok(path) => path,
            Err(error) => return ToolResult::error(error),
        };
        let start_line = match input["start_line"].as_i64() {
            Some(n) if n >= 1 => n,
            _ => return ToolResult::error("'start_line' must be an integer >= 1"),
        };
        let end_line = match input["end_line"].as_i64() {
            Some(n) if n >= start_line => n,
            _ => return ToolResult::error("'end_line' must be >= start_line"),
        };
        let note = match input["note"].as_str() {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => return ToolResult::error("'note' is required and must be a non-empty string"),
        };
        let severity = input["severity"].as_str().unwrap_or("info").to_string();

        // Infer the source from the agent role in context
        let source = match ctx.agent_role.as_str() {
            "qc-agent" => "qc_agent",
            r if r.starts_with("worker") => "worker",
            _ => "user",
        }
        .to_string();

        let create_input = CreateAnnotationInput {
            file_path: file_path.clone(),
            start_line,
            end_line,
            selected_text: None,
            prompt: note.clone(),
            source: Some(source),
            severity: Some(severity.clone()),
            task_id: None,
        };

        match AnnotationsDb::open() {
            Ok(db) => match db.create(&create_input) {
                Ok(id) => ToolResult::success(format!(
                    "Annotation created (id={id}) on {}:{}-{} [{}]: {}",
                    file_path,
                    start_line,
                    end_line,
                    severity,
                    // P3-04: `&note[..80]` panicked on a multi-byte note.
                    crate::security::truncate::truncate_bytes(&note, 80)
                )),
                Err(e) => ToolResult::error(format!("Failed to create annotation: {e}")),
            },
            Err(e) => ToolResult::error(format!("Failed to open annotations DB: {e}")),
        }
    }
}

fn resolve_annotation_path(file_path: &str, working_dir: &str) -> Result<String, String> {
    let workspace = crate::platform::workspace::resolve_workspace_dir(Some(working_dir))
        .map_err(|error| format!("Annotation workspace is unavailable: {error}"))?;
    let supplied = Path::new(file_path);
    if supplied.is_absolute() {
        return Err("Annotation path must be relative to the configured workspace".into());
    }
    let mut normalized_parts = Vec::new();
    for component in supplied.components() {
        match component {
            Component::Normal(part) => normalized_parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("Annotation path must stay inside the configured workspace".into());
            }
        }
    }
    if normalized_parts.is_empty() {
        return Err("Annotation path must name a file inside the configured workspace".into());
    }
    let normalized_input = normalized_parts.join("/");
    super::path_jail::validate_workspace_path(
        &workspace.join(&normalized_input),
        &workspace.to_string_lossy(),
    )?;
    Ok(normalized_input)
}

#[cfg(test)]
mod workspace_tests {
    use super::*;

    #[test]
    fn annotation_paths_are_workspace_relative_and_confined() {
        let workspace = tempfile::tempdir().expect("workspace");
        assert_eq!(
            resolve_annotation_path("src/main.rs", &workspace.path().to_string_lossy()).unwrap(),
            "src/main.rs"
        );
        assert!(
            resolve_annotation_path("../outside.rs", &workspace.path().to_string_lossy()).is_err()
        );
        assert!(
            resolve_annotation_path("src/../outside.rs", &workspace.path().to_string_lossy())
                .is_err()
        );
        assert!(resolve_annotation_path(
            &workspace.path().join("outside.rs").to_string_lossy(),
            &workspace.path().to_string_lossy()
        )
        .unwrap_err()
        .contains("must be relative"));
        assert!(resolve_annotation_path("src/main.rs", ".")
            .unwrap_err()
            .contains("must be absolute"));
    }

    async fn execute_registered_artifact_tool(
        executor: &mut crate::agent::tool_executor::ToolExecutor,
        registry: &crate::agent::tool_registry::ToolRegistry,
        tool_name: &str,
        input: serde_json::Value,
        use_id: &str,
    ) -> ToolResult {
        let tool = registry
            .get(tool_name)
            .unwrap_or_else(|| panic!("FastDevelopment registry is missing {tool_name}"));
        let results = executor
            .execute_batch(vec![(tool, input, use_id.to_string())])
            .await
            .expect("registered artifact tool returns a result");
        assert_eq!(results.len(), 1, "one result for {tool_name}");
        let (returned_use_id, result) = results.into_iter().next().expect("one result");
        assert_eq!(returned_use_id, use_id);
        result
    }

    fn assert_artifact_failure(
        tool_name: &str,
        use_id: &str,
        task_id: i64,
        result: &ToolResult,
        private_arguments: &[&str],
    ) {
        assert!(
            result.is_error,
            "{tool_name} must be delivered as a failed tool result"
        );
        assert!(
            result.output.len() <= 8 * 1024,
            "bounded error for {tool_name}"
        );
        let envelope: serde_json::Value =
            serde_json::from_str(&result.output).expect("versioned JSON failure envelope");
        assert_eq!(envelope["schema_version"], 1, "{tool_name}");
        assert_eq!(envelope["ok"], false, "{tool_name} cannot look successful");
        assert_eq!(envelope["error"]["tool"], tool_name, "{tool_name}");
        assert_eq!(
            envelope["error"]["operation"],
            format!("tool.{tool_name}"),
            "{tool_name} operation identity"
        );
        assert_eq!(
            envelope["error"]["context"]["session_id"], "artifact-matrix-session",
            "{tool_name} session correlation"
        );
        assert_eq!(
            envelope["error"]["context"]["agent_id"], "worker-artifact-matrix",
            "{tool_name} agent correlation"
        );
        assert_eq!(
            envelope["error"]["context"]["task_id"], task_id,
            "{tool_name} task correlation"
        );
        assert_eq!(
            envelope["error"]["context"]["tool_call_id"],
            format!("artifact-matrix-session-{use_id}"),
            "{tool_name} call correlation"
        );
        for field in [
            "error_code",
            "category",
            "message",
            "cause",
            "suggested_action",
        ] {
            assert!(
                envelope["error"][field].as_str().is_some(),
                "{tool_name} has {field}: {envelope}"
            );
        }
        for field in ["message", "cause", "suggested_action"] {
            assert!(
                envelope["error"][field].as_str().unwrap_or_default().len() <= 512,
                "{tool_name} {field} is bounded"
            );
        }
        assert!(envelope["error"]["retryable"].is_boolean(), "{tool_name}");
        assert!(
            envelope["error"]["retry_after_ms"].is_null()
                || envelope["error"]["retry_after_ms"].is_u64(),
            "{tool_name} retry delay is optional and numeric"
        );
        assert!(
            envelope["error"]["timestamp"].as_str().is_some(),
            "{tool_name} timestamp"
        );
        assert!(
            envelope["error"]["context"].is_object(),
            "{tool_name} context"
        );
        for private in private_arguments {
            assert!(
                !result.output.contains(private),
                "{tool_name} leaked private argument {private}"
            );
        }
    }

    #[tokio::test]
    async fn artifact_builtins_have_registered_executor_success_and_failure_contracts() {
        use crate::agent::{
            builtin_tools, permission::PermissionMode, tool_executor::ToolExecutor,
            tool_registry::ToolRegistry, tool_trait::AgentIdentity, PAMode,
        };
        use crate::db::tasks::{CreateTaskInput, TasksDb};
        use std::sync::Arc;

        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated workspace");
        std::fs::write(workspace.path().join("source.rs"), "fn main() {}\n")
            .expect("seed annotation source");
        std::fs::write(workspace.path().join("valid.json"), "{\"answer\": 42}\n")
            .expect("seed valid JSON for check_file");
        let prior_interface = "# Existing interface contract\n\nPreserve this in the checkpoint.\n";
        std::fs::write(workspace.path().join("INTERFACE.md"), prior_interface)
            .expect("seed prior interface contract");

        let worker = "worker-artifact-matrix";
        let tasks = TasksDb::open().expect("open isolated task database");
        let task = tasks
            .create(CreateTaskInput {
                title: "artifact builtin executor matrix".into(),
                description: Some("temporary test task".into()),
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "exercise registered artifact tools".into(),
                created_by: Some("test".into()),
            })
            .expect("create isolated task");
        tasks
            .update_status(task.id, "assigned", Some(worker))
            .expect("assign isolated task to test worker");
        tasks
            .claim_task_with_pid(task.id, worker, std::process::id())
            .expect("claim task through production task API");
        drop(tasks);

        let mut registry = ToolRegistry::new();
        builtin_tools::register_for_mode(&mut registry, PAMode::FastDevelopment);
        for tool_name in ["check_file", "annotate_file", "write_interface_spec"] {
            assert!(
                registry.get(tool_name).is_some(),
                "{tool_name} is a live FastDevelopment registration"
            );
        }
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            "artifact-matrix-session".into(),
            workspace.path().to_string_lossy().into_owned(),
            AgentIdentity::worker(worker),
        );
        executor.set_task_id(Some(task.id));

        let checked = execute_registered_artifact_tool(
            &mut executor,
            &registry,
            "check_file",
            serde_json::json!({"path":"valid.json", "checks":["syntax"]}),
            "check-success",
        )
        .await;
        assert!(!checked.is_error, "valid JSON check: {}", checked.output);
        assert!(
            checked.output.contains("json_parse"),
            "real syntax checker ran"
        );
        assert!(
            checked.output.contains("No syntax errors"),
            "valid JSON passed"
        );

        let checked_failure = execute_registered_artifact_tool(
            &mut executor,
            &registry,
            "check_file",
            serde_json::json!({
                "path":"valid.json",
                "checks":["PRIVATE_CHECK_SELECTION_MARKER"],
                "source":"PRIVATE_CHECK_SOURCE_MARKER"
            }),
            "check-failure",
        )
        .await;
        assert_artifact_failure(
            "check_file",
            "check-failure",
            task.id,
            &checked_failure,
            &[
                "PRIVATE_CHECK_SELECTION_MARKER",
                "PRIVATE_CHECK_SOURCE_MARKER",
            ],
        );

        let annotation_success = execute_registered_artifact_tool(
            &mut executor,
            &registry,
            "annotate_file",
            serde_json::json!({
                "file_path":"source.rs",
                "start_line":1,
                "end_line":1,
                "note":"Review the entry point ownership.",
                "severity":"warning"
            }),
            "annotate-success",
        )
        .await;
        assert!(
            !annotation_success.is_error,
            "annotation creation: {}",
            annotation_success.output
        );
        assert!(annotation_success.output.contains("Annotation created"));
        let annotations = AnnotationsDb::open()
            .expect("open production annotations reader")
            .list_for_file("source.rs")
            .expect("read persisted annotation");
        assert_eq!(annotations.len(), 1, "annotation is visible to its reader");
        assert_eq!(annotations[0].prompt, "Review the entry point ownership.");
        assert_eq!(annotations[0].start_line, 1);
        assert_eq!(annotations[0].end_line, 1);
        assert_eq!(annotations[0].source, "worker");
        assert_eq!(annotations[0].severity, "warning");

        let annotation_failure = execute_registered_artifact_tool(
            &mut executor,
            &registry,
            "annotate_file",
            serde_json::json!({
                "file_path":"source.rs",
                "start_line":"PRIVATE_ANNOTATION_LINE_MARKER",
                "end_line":1,
                "note":"PRIVATE_ANNOTATION_PROMPT_MARKER",
                "severity":"blocking"
            }),
            "annotate-failure",
        )
        .await;
        assert_artifact_failure(
            "annotate_file",
            "annotate-failure",
            task.id,
            &annotation_failure,
            &[
                "PRIVATE_ANNOTATION_LINE_MARKER",
                "PRIVATE_ANNOTATION_PROMPT_MARKER",
                "source.rs",
            ],
        );
        assert_eq!(
            AnnotationsDb::open()
                .expect("reopen annotations reader")
                .list_for_file("source.rs")
                .expect("verify failed call did not add a row")
                .len(),
            1,
            "malformed annotation adds no persisted row"
        );

        let interface_success = execute_registered_artifact_tool(
            &mut executor,
            &registry,
            "write_interface_spec",
            serde_json::json!({
                "mode":"overwrite",
                "routes":[{
                    "method":"GET",
                    "path":"/api/artifacts",
                    "description":"List available artifacts.",
                    "response_shape":{"artifacts":[]},
                    "status_codes":[401,500]
                }]
            }),
            "interface-success",
        )
        .await;
        assert!(
            !interface_success.is_error,
            "interface spec write: {}",
            interface_success.output
        );
        let interface_path = workspace.path().join("INTERFACE.md");
        let interface_content =
            std::fs::read_to_string(&interface_path).expect("read persisted interface spec");
        assert!(interface_content.contains("### GET /api/artifacts"));
        assert!(interface_content.contains("List available artifacts."));
        assert!(interface_content.contains("\"artifacts\": []"));
        let checkpoints = crate::git::checkpoint::list(None, None, Some(task.id))
            .expect("read pre-write checkpoint through production store");
        assert_eq!(
            checkpoints.len(),
            1,
            "write created a task-associated checkpoint"
        );
        assert_eq!(checkpoints[0].reason, "pre_write");
        assert_eq!(
            std::fs::canonicalize(&checkpoints[0].file_path).expect("canonical checkpoint target"),
            std::fs::canonicalize(&interface_path).expect("canonical interface target")
        );
        assert_eq!(
            std::fs::read_to_string(&checkpoints[0].snapshot_path)
                .expect("read checkpoint snapshot"),
            prior_interface,
            "checkpoint retains prior interface content"
        );

        let interface_failure = execute_registered_artifact_tool(
            &mut executor,
            &registry,
            "write_interface_spec",
            serde_json::json!({
                "routes":[{
                    "method":17,
                    "path":"PRIVATE_INTERFACE_PATH_MARKER",
                    "notes":"PRIVATE_INTERFACE_NOTES_MARKER"
                }],
                "conventions":"PRIVATE_INTERFACE_CONVENTIONS_MARKER"
            }),
            "interface-failure",
        )
        .await;
        assert_artifact_failure(
            "write_interface_spec",
            "interface-failure",
            task.id,
            &interface_failure,
            &[
                "PRIVATE_INTERFACE_PATH_MARKER",
                "PRIVATE_INTERFACE_NOTES_MARKER",
                "PRIVATE_INTERFACE_CONVENTIONS_MARKER",
            ],
        );
        assert_eq!(
            std::fs::read_to_string(&interface_path).expect("interface remains persisted"),
            interface_content,
            "malformed input leaves the successful artifact unchanged"
        );
        assert_eq!(
            crate::git::checkpoint::list(None, None, Some(task.id))
                .expect("read task checkpoints after failed call")
                .len(),
            1,
            "malformed input does not create another checkpoint"
        );
    }
}
