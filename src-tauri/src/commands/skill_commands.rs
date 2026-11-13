// Tauri commands — Skills system. Phase 3.1
// list_skills, get_skill, search_skills, skills_by_category,
// execute_skill, validate_skill_parameters, reload_skills, import_skill
//
// SkillRegistry is managed as Arc<SkillRegistry> — thread-safe via internal RwLock.

use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;

use crate::errors::{ArcanaError, Result};
use crate::skills::types::{Skill, SkillExecutionResult, SkillSummary, ValidationResult};
use crate::skills::{SkillEngine, SkillRegistry};

// ── list_skills ───────────────────────────────────────────────────────────────

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
        return Err(ArcanaError::Agent(format!(
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
