// suggest_worker — MCP tool that returns the recommended worker for a task.
// Available to all MCP workers; the Supervisor recipe calls it before assign_task
// to make data-driven routing decisions based on historical performance.

use async_trait::async_trait;
use serde_json::{json, Value};
use crate::db::tasks::TasksDb;
use crate::routing::TaskRouter;
use super::super::tool_trait::{Tool, ToolContext, ToolResult};

pub struct SuggestWorkerTool;

#[async_trait]
impl Tool for SuggestWorkerTool {
    fn name(&self) -> &str { "suggest_worker" }

    fn description(&self) -> &str {
        "Get the recommended worker for a task based on historical performance and \
        static keyword patterns. Returns worker_slug, score (0.0-1.0), confidence, \
        and method ('static' | 'historical' | 'default'). Call this before assign_task \
        to make data-driven routing decisions."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "integer",
                    "description": "Numeric task ID from the internal board"
                }
            },
            "required": ["task_id"]
        })
    }

    fn is_read_only(&self) -> bool { true }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let task_id = match input["task_id"].as_i64() {
            Some(id) if id > 0 => id,
            _ => return ToolResult::error("'task_id' must be a positive integer"),
        };

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(format!("Failed to open tasks database: {e}")),
        };

        let task = match db.get_by_id(task_id) {
            Ok(Some(t)) => t,
            Ok(None)    => return ToolResult::error(format!("Task #{task_id} not found")),
            Err(e)      => return ToolResult::error(format!("Failed to fetch task #{task_id}: {e}")),
        };

        match TaskRouter::suggest_worker(&task) {
            Ok(score) => ToolResult::success(json!({
                "task_id":     task_id,
                "task_title":  task.title,
                "task_type":   task.task_type.as_str(),
                "worker_slug": score.worker_slug,
                "score":       score.score,
                "confidence":  score.confidence,
                "method":      score.method,
                "message": format!(
                    "Recommended worker: {} (score={:.2}, method={})",
                    score.worker_slug, score.score, score.method
                )
            }).to_string()),
            Err(e) => ToolResult::error(format!("Failed to compute worker suggestion: {e}")),
        }
    }
}
