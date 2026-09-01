// create_epic — Ruflo Feature 5. Creates an Epic task (task_type='epic') that
// acts as a parent container for phased work. Subtasks are created separately
// via create_phased_task and reference this epic's ID.
// Available in FastDevelopment | Review modes (Supervisor / PA).

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct CreateEpicTool;

#[async_trait]
impl Tool for CreateEpicTool {
    fn name(&self) -> &str {
        "create_epic"
    }

    fn description(&self) -> &str {
        "Create an Epic — a parent task that groups phased subtasks. \
         Use this first, then call create_phased_task with the returned epic_id \
         to add individual subtasks with phase metadata and blocker relationships. \
         The epic auto-completes when all its subtasks reach status=done."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short, descriptive epic title (max 120 chars). \
                                    E.g. 'Implement OAuth2 Login Flow'"
                },
                "description": {
                    "type": "string",
                    "description": "High-level description of the epic goal and scope (optional)"
                }
            },
            "required": ["title"]
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let title = match input["title"].as_str() {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => return ToolResult::error("'title' is required and must not be empty"),
        };
        if title.len() > 120 {
            return ToolResult::error("title must be 120 characters or less");
        }
        let description = input["description"].as_str().map(|s| s.to_string());
        let created_by = ctx.session_id.as_str();

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

        let epic = match db.create_epic(&title, description.as_deref(), created_by) {
            Ok(e) => e,
            Err(e) => return ToolResult::error(&format!("Failed to create epic: {e}")),
        };

        crate::events::emit_task_changed(crate::events::TaskChangedPayload {
            task_id: Some(epic.id),
            kind: "created".into(),
            status: Some("ready".into()),
            assigned_worker: None,
        });

        ToolResult::success(
            json!({
                "epic_id": epic.id,
                "title":   epic.title,
                "status":  "ready",
                "message": format!(
                    "✅ Epic #{} created. Now call create_phased_task with epic_id={} \
                     to add subtasks for each phase.",
                    epic.id, epic.id
                )
            })
            .to_string(),
        )
    }
}
