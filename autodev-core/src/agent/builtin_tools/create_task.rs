// create_task — Delegate Mode only. Writes a new task to ~/.autodev/tasks.db.
// PA calls this when in Delegate Mode instead of directly editing files.
// The task enters the board with status="ready" for Supervisor to pick up.
// See: DEVELOPMENT_PLAN.md Phase 2.1 — PA Mode Toggle / create_task Tool

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::{CreateTaskInput, TasksDb};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct CreateTaskTool;

#[async_trait]
impl Tool for CreateTaskTool {
    fn name(&self) -> &str {
        "create_task"
    }

    fn description(&self) -> &str {
        "Create a new task on the internal AutoDev task board. Use this in Delegate Mode to assign work to the Worker fleet. The task enters with status 'ready' and will be picked up by the Supervisor within 2 minutes. Always include specific, testable acceptance criteria."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short, specific task title (max 120 chars). E.g. 'Add GET /agents/health endpoint'"
                },
                "description": {
                    "type": "string",
                    "description": "Detailed description of what needs to be done and why (optional)"
                },
                "priority": {
                    "type": "string",
                    "enum": ["p0", "p1", "p2", "p3"],
                    "description": "p0=critical/blocking, p1=high, p2=medium (default), p3=low"
                },
                "task_type": {
                    "type": "string",
                    "enum": ["feature", "bug", "refactor", "docs", "chore"],
                    "description": "Type of work. feature=new functionality, bug=fix existing, refactor=restructure, docs=documentation, chore=maintenance"
                },
                "acceptance_criteria": {
                    "type": "string",
                    "description": "Specific, verifiable criteria that define when this task is done. List each criterion on a new line. Be precise — the QC agent will check these."
                }
            },
            "required": ["title", "priority", "task_type", "acceptance_criteria"]
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let title = match input["title"].as_str() {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => return ToolResult::error("'title' is required and must not be empty"),
        };
        if title.len() > 120 {
            return ToolResult::error("title must be 120 characters or less");
        }

        let priority = input["priority"].as_str().unwrap_or("p2").to_string();
        let task_type = input["task_type"].as_str().unwrap_or("feature").to_string();
        let acceptance_criteria = match input["acceptance_criteria"].as_str() {
            Some(ac) if !ac.trim().is_empty() => ac.trim().to_string(),
            _ => {
                return ToolResult::error("'acceptance_criteria' is required and must not be empty")
            }
        };
        let description = input["description"].as_str().map(|s| s.to_string());

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

        let task = match db.create(CreateTaskInput {
            title: title.clone(),
            description,
            priority: priority.clone(),
            task_type: task_type.clone(),
            acceptance_criteria: acceptance_criteria.clone(),
            created_by: Some("pa".to_string()),
        }) {
            Ok(t) => t,
            Err(e) => return ToolResult::error(&format!("Failed to create task: {e}")),
        };

        // Phase 3.0 R1 — push-based dispatch. Wake the Supervisor immediately
        // so it dispatches this task on the next event-loop tick instead of
        // waiting up to one cron cycle (previously ~21 s dead window).
        crate::recipes::scheduler::kick("supervisor");

        // Phase 3.0 R1 — notify the UI (AutoDev Panel kanban) that the board
        // changed without requiring a poll. The frontend `taskStore` listens
        // to `task:changed` and re-fetches `list_tasks` on any event.
        crate::events::emit_task_changed(crate::events::TaskChangedPayload {
            task_id: Some(task.id),
            kind: "created".into(),
            status: Some("ready".into()),
            assigned_worker: None,
        });

        ToolResult::success(json!({
            "task_id": task.id,
            "title": task.title,
            "priority": priority,
            "task_type": task_type,
            "status": "ready",
            "message": format!(
                "✅ Task #{} created on the AutoDev board. Status: ready. The Supervisor will pick this up within 2 minutes and assign it to a worker.",
                task.id
            )
        }).to_string())
    }
}
