// Annotation commands — CRUD for ~/.autodev/annotations.db
// Used by: IDE CodeViewer (display), AnnotateFileTool (agent writes), frontend store

use crate::errors::Result;
use crate::db::annotations::{Annotation, AnnotationsDb, CreateAnnotationInput};

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

#[tauri::command]
pub fn delete_annotation(id: String) -> Result<()> {
    AnnotationsDb::open()?.delete(&id)
}
