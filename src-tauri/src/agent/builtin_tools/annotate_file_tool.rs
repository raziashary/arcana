// annotate_file — MCP tool for the QC Agent and other fleet workers.
// Writes an inline review note to a specific line range in a workspace file.
// Notes are persisted in annotations.db and rendered as gutter decorations
// in the IDE CodeViewer when the user opens the file.

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::annotations::{AnnotationsDb, CreateAnnotationInput};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct AnnotateFileTool;

#[async_trait]
impl Tool for AnnotateFileTool {
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

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let file_path = match input["file_path"].as_str() {
            Some(p) if !p.is_empty() => p.to_string(),
            _ => {
                return ToolResult::error("'file_path' is required and must be a non-empty string")
            }
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
                    if note.len() > 80 { &note[..80] } else { &note }
                )),
                Err(e) => ToolResult::error(format!("Failed to create annotation: {e}")),
            },
            Err(e) => ToolResult::error(format!("Failed to open annotations DB: {e}")),
        }
    }
}
