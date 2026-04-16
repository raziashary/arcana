// Model config commands — get/set per-model temperature, top_p, reasoning_effort, thinking.
// Phase 2.3: T4a/T4b — Reasoning Effort + Temperature/Top-P per-model config.
// See: DEVELOPMENT_PLAN.md Phase 2.3 — Provider Support & Model Enhancements

use crate::errors::Result;
use crate::db::model_configs::{ModelConfig, ModelConfigsDb};

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Fetch the stored model config for a given model_id.
/// Returns None if no config has been saved (caller should use provider defaults).
#[tauri::command]
pub fn get_model_config(model_id: String) -> Result<Option<ModelConfig>> {
    ModelConfigsDb::open()?.get(&model_id)
}

/// Persist a model config.  All fields are optional — pass None to keep using defaults.
#[tauri::command]
pub fn set_model_config(
    model_id:         String,
    temperature:      Option<f32>,
    top_p:            Option<f32>,
    reasoning_effort: Option<String>,
    thinking_enabled: bool,
    thinking_budget:  Option<u32>,
) -> Result<()> {
    let cfg = ModelConfig {
        model_id,
        temperature,
        top_p,
        reasoning_effort,
        thinking_enabled,
        thinking_budget,
    };
    ModelConfigsDb::open()?.upsert(&cfg)
}

/// Delete a model's stored config (reverts to provider defaults).
#[tauri::command]
pub fn delete_model_config(model_id: String) -> Result<()> {
    ModelConfigsDb::open()?.delete(&model_id)
}
