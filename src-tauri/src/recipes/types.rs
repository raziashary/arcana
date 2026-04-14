// Recipe types — YAML-defined workflow definitions for the Worker Fleet.
// Recipes are used by Workers, Supervisor, and QC — NOT by the PA.
// PA uses AUTODEV.md for its own instructions.
// See: DEVELOPMENT_PLAN.md Phase 2.1 — Recipes System

use serde::{Deserialize, Serialize};

/// Permission mode for recipe execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RecipePermissionMode {
    /// Agent executes all actions without prompting
    Autonomous,
    /// Agent prompts before destructive operations
    Supervised,
}

impl Default for RecipePermissionMode {
    fn default() -> Self { Self::Autonomous }
}

/// A single Recipe — stored as YAML in ~/.autodev/recipes/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub version:         String,                   // "1.0.0"
    pub id:              String,                   // slug used as file name
    pub title:           String,
    pub description:     Option<String>,
    pub schedule:        Option<String>,           // cron expression (*/120 * * * * = every 2 min)
    pub permission_mode: RecipePermissionMode,
    pub instructions:    String,                   // system prompt injected into agent
    pub agent_role:      Option<String>,           // "supervisor" | "worker-backend" | "qc"
    pub model:           Option<String>,           // model override (default from settings)
    pub is_system:       bool,                     // system recipes cannot be deleted by users
}

/// Execution state of a recipe run
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RecipeRunStatus {
    Idle,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// A record of a recipe execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeRun {
    pub id:         String,        // UUID
    pub recipe_id:  String,
    pub status:     RecipeRunStatus,
    pub started_at: String,
    pub ended_at:   Option<String>,
    pub error:      Option<String>,
}
