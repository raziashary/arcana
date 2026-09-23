// Annotation commands — CRUD for ~/.arcana/annotations.db
// Used by: IDE CodeViewer (display), AnnotateFileTool (agent writes), frontend store

use crate::db::annotations::{Annotation, AnnotationsDb, CreateAnnotationInput};
use crate::errors::Result;

#[tauri::command]
pub fn create_annotation(input: CreateAnnotationInput) -> Result<String> {
    AnnotationsDb::open()?.create(&input)
}

#[tauri::command]
pub fn list_annotations_for_file(file_path: String) -> Result<Vec<Annotation>> {
    AnnotationsDb::open()?.list_for_file(&file_path)
}

#[tauri::command]
pub fn resolve_annotation(id: String, response: Option<String>) -> Result<()> {
    AnnotationsDb::open()?.resolve(&id, response.as_deref())
}

/// Write the PA's response text back to an annotation without resolving it.
/// Called after "Ask PA" completes so the answer is visible in the annotation
/// panel alongside the original code selection.
#[tauri::command]
pub fn update_annotation_response(id: String, response: String) -> Result<()> {
    AnnotationsDb::open()?.update_response(&id, Some(&response))
}

/// Link a fleet task to an annotation after Delegate mode dispatches the work.
/// Stored as `task_id` on the annotation row; surfaced as a task badge in the
/// AnnotationPanel so the user can navigate directly to the created task.
#[tauri::command]
pub fn update_annotation_task_id(id: String, task_id: String) -> Result<()> {
    AnnotationsDb::open()?.update_task_id(&id, &task_id)
}

#[tauri::command]
pub fn delete_annotation(id: String) -> Result<()> {
    AnnotationsDb::open()?.delete(&id)
}
