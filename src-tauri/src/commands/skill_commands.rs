// Tauri commands — Skills system. Phase 3.1
// list_skills, get_skill, search_skills, skills_by_category,
// execute_skill, validate_skill_parameters, reload_skills, import_skill
//
// SkillRegistry is managed as Arc<SkillRegistry> — thread-safe via internal RwLock.

use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;

use crate::errors::{AutoDevError, Result};
use crate::skills::types::{
    Skill, SkillCategory, SkillExecutionResult, SkillSummary, ValidationResult,
};
use crate::skills::{SkillEngine, SkillRegistry};

// ── list_skills ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_skills(
    category: Option<String>,
    registry: State<'_, Arc<SkillRegistry>>,
) -> Result<Vec<SkillSummary>> {
    let cat = category.as_deref().map(SkillCategory::from_str);
    Ok(registry.list(cat))
}

// ── get_skill ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_skill(name: String, registry: State<'_, Arc<SkillRegistry>>) -> Result<Skill> {
    registry.get(&name)
}

// ── search_skills ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn search_skills(
    query: String,
    registry: State<'_, Arc<SkillRegistry>>,
) -> Result<Vec<SkillSummary>> {
    Ok(registry.search(&query))
}

// ── skills_by_category ────────────────────────────────────────────────────────

#[tauri::command]
pub fn skills_by_category(
    registry: State<'_, Arc<SkillRegistry>>,
) -> Result<HashMap<String, Vec<SkillSummary>>> {
    Ok(registry.by_category())
}

// ── validate_skill_parameters ─────────────────────────────────────────────────

#[tauri::command]
pub fn validate_skill_parameters(
    name: String,
    parameters: serde_json::Map<String, serde_json::Value>,
    registry: State<'_, Arc<SkillRegistry>>,
) -> Result<ValidationResult> {
    let skill = registry.get(&name)?;
    Ok(SkillEngine::validate(&skill, &parameters))
}

// ── execute_skill ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn execute_skill(
    name: String,
    parameters: serde_json::Map<String, serde_json::Value>,
    registry: State<'_, Arc<SkillRegistry>>,
) -> Result<SkillExecutionResult> {
    let skill = registry.get(&name)?;

    let validation = SkillEngine::validate(&skill, &parameters);
    if !validation.valid {
        let msgs: Vec<String> = validation
            .errors
            .iter()
            .map(|e| format!("{}: {}", e.field, e.message))
            .collect();
        return Err(AutoDevError::Agent(format!(
            "Skill parameter validation failed: {}",
            msgs.join("; ")
        )));
    }

    Ok(SkillEngine::render(&skill, &parameters))
}

// ── reload_skills ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn reload_skills(registry: State<'_, Arc<SkillRegistry>>) -> Result<usize> {
    registry.reload()
}

// ── import_skill ──────────────────────────────────────────────────────────────

#[tauri::command]
pub fn import_skill(source: String, registry: State<'_, Arc<SkillRegistry>>) -> Result<Skill> {
    use crate::db::autodev_dir;
    use crate::skills::loader::SkillLoader;
    use std::path::Path;

    let path = Path::new(&source);
    if !path.exists() {
        return Err(AutoDevError::Agent(format!("File not found: {source}")));
    }

    let category_hint = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("custom");

    let loader = SkillLoader::new()?;
    let skill = loader.load_file(path, category_hint)?;

    let dest_dir = autodev_dir()?.join("skills").join(skill.category.as_str());
    std::fs::create_dir_all(&dest_dir)?;
    std::fs::copy(path, dest_dir.join(path.file_name().unwrap()))?;

    registry.reload()?;
    Ok(skill)
}
