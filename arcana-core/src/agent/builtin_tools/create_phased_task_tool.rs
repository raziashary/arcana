// create_phased_task — Ruflo Feature 5. Adds a subtask into an epic with full
// phase metadata (phase name, order, milestone) and an optional blocker list.
// Tasks with blockers start as needs_info; they auto-transition to ready when
// all their blockers complete (handled by unblock_ready_tasks in the DB layer).
// Available in FastDevelopment | Review modes (Supervisor / PA).

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::{CreatePhasedTaskInput, TasksDb};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct CreatePhasedTaskTool;

#[async_trait]
impl Tool for CreatePhasedTaskTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "create_phased_task"
    }

    fn description(&self) -> &str {
        "Add a subtask to an existing Epic with phase metadata and optional blocker dependencies. \
         Tasks with blocked_by entries start as needs_info and auto-unlock to ready when their \
         dependencies complete. phase_order controls sequencing — lower numbers run first."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "epic_id": {
                    "type": "string",
                    "description": "ID of the parent Epic task (returned by create_epic)"
                },
                "phase": {
                    "type": "string",
                    "description": "Phase name, e.g. 'foundation', 'core', 'integration', 'qa'"
                },
                "phase_order": {
                    "type": "integer",
                    "description": "Ordering within the epic — lower numbers run first (1=first phase)"
                },
                "title": {
                    "type": "string",
                    "description": "Short, specific task title (max 120 chars)"
                },
                "description": {
                    "type": "string",
                    "description": "Detailed description of the work (optional)"
                },
                "priority": {
                    "type": "string",
                    "enum": ["p0", "p1", "p2", "p3"],
                    "description": "p0=critical, p1=high, p2=medium (default), p3=low"
                },
                "task_type": {
                    "type": "string",
                    "enum": ["feature", "bug", "refactor", "docs", "chore"],
                    "description": "Type of work"
                },
                "acceptance_criteria": {
                    "type": "string",
                    "description": "Specific, verifiable criteria for completion"
                },
                "blocked_by": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Task IDs that must reach status=done before this task becomes ready. Empty array = no blockers."
                },
                "milestone": {
                    "type": "string",
                    "description": "Optional milestone label for this phase (e.g. 'MVP', 'v1.0')"
                }
            },
            "required": ["epic_id", "phase", "phase_order", "title", "priority", "task_type", "acceptance_criteria"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let epic_id = match input["epic_id"].as_str() {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => return ToolResult::error("'epic_id' is required"),
        };
        let phase = match input["phase"].as_str() {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => return ToolResult::error("'phase' is required"),
        };
        let phase_order = match input["phase_order"].as_i64() {
            Some(n) => n as i32,
            None => return ToolResult::error("'phase_order' must be an integer"),
        };
        let title = match input["title"].as_str() {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => return ToolResult::error("'title' is required"),
        };
        if title.len() > 120 {
            return ToolResult::error("title must be 120 characters or less");
        }
        let acceptance_criteria = match input["acceptance_criteria"].as_str() {
            Some(ac) if !ac.trim().is_empty() => ac.trim().to_string(),
            _ => return ToolResult::error("'acceptance_criteria' is required"),
        };

        let priority = input["priority"].as_str().unwrap_or("p2").to_string();
        let task_type = input["task_type"].as_str().unwrap_or("feature").to_string();
        let description = input["description"].as_str().map(|s| s.to_string());
        let milestone = input["milestone"].as_str().map(|s| s.to_string());
        let blocked_by: Vec<String> = input["blocked_by"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

        // ── Validate epic_id ─────────────────────────────────────────────────
        // epic_id must parse as an integer AND refer to a task with type='epic'.
        let epic_row_id: i64 = match epic_id.parse::<i64>() {
            Ok(id) if id > 0 => id,
            _ => {
                return ToolResult::error(&format!(
                    "'epic_id' must be a positive integer task ID (got '{epic_id}'). \
                 Use create_epic to create an epic first, then pass its returned task_id here."
                ))
            }
        };
        match db.get_by_id(epic_row_id) {
            Ok(Some(ref epic)) if epic.task_type == crate::db::tasks::TaskType::Epic => {}
            Ok(Some(_)) => {
                return ToolResult::error(&format!(
                    "Task #{epic_row_id} exists but is not an Epic. \
                 Pass the task_id returned by create_epic, not a regular task ID."
                ))
            }
            Ok(None) => {
                return ToolResult::error(&format!(
                    "Epic #{epic_row_id} not found. Create it first with create_epic, \
                 then pass the returned task_id as epic_id."
                ))
            }
            Err(e) => {
                return ToolResult::error(&format!("Failed to look up epic #{epic_row_id}: {e}"))
            }
        }

        // ── Validate blocked_by IDs ──────────────────────────────────────────
        // Every ID in blocked_by must refer to an existing task.
        if !blocked_by.is_empty() {
            let mut invalid: Vec<String> = Vec::new();
            for blocker_id_str in &blocked_by {
                match blocker_id_str.parse::<i64>() {
                    Ok(bid) if bid > 0 => match db.get_by_id(bid) {
                        Ok(None) => invalid.push(format!("#{bid} (not found)")),
                        Err(e) => invalid.push(format!("#{bid} (lookup error: {e})")),
                        Ok(Some(_)) => {}
                    },
                    _ => invalid.push(format!("'{blocker_id_str}' (not a valid task ID)")),
                }
            }
            if !invalid.is_empty() {
                return ToolResult::error(&format!(
                    "blocked_by contains invalid task IDs: {}. \
                     Use list_tasks or get_task to find the correct IDs.",
                    invalid.join(", ")
                ));
            }
        }

        let task = match db.create_phased_task(CreatePhasedTaskInput {
            epic_id: epic_id.clone(),
            phase: phase.clone(),
            phase_order,
            title: title.clone(),
            description,
            priority: priority.clone(),
            task_type: task_type.clone(),
            acceptance_criteria: acceptance_criteria.clone(),
            blocked_by: blocked_by.clone(),
            milestone: milestone.clone(),
            created_by: Some(ctx.session_id.clone()),
        }) {
            Ok(t) => t,
            Err(e) => return ToolResult::error(&format!("Failed to create phased task: {e}")),
        };

        let status_str = task.status.as_str().to_string();
        crate::events::emit_task_changed(crate::events::TaskChangedPayload {
            task_id: Some(task.id),
            kind: "created".into(),
            status: Some(status_str.clone()),
            assigned_worker: None,
        });

        let blocked_msg = if !blocked_by.is_empty() {
            format!(
                " Blocked by tasks: [{}]. Will auto-unlock when they complete.",
                blocked_by.join(", ")
            )
        } else {
            String::new()
        };

        ToolResult::success(
            json!({
                "task_id":    task.id,
                "epic_id":    epic_id,
                "phase":      phase,
                "phase_order": phase_order,
                "title":      title,
                "status":     status_str,
                "message": format!(
                    "✅ Task #{} created in phase '{}' (order {}) of epic #{}.{}",
                    task.id, phase, phase_order, epic_id, blocked_msg
                )
            })
            .to_string(),
        )
    }
}
