// Recipes module — YAML workflow definitions for the Worker Fleet.
// Recipes live in ~/.autodev/recipes/ and are managed by RecipeEngine.
// See: DEVELOPMENT_PLAN.md Phase 2.1 — Recipes System

pub mod defaults;
pub mod parser;
pub mod runner;
pub mod scheduler;
pub mod types;

use crate::db::autodev_dir;
use crate::errors::Result;
use std::path::PathBuf;

pub use types::{Recipe, RecipePermissionMode, RecipeRun, RecipeRunStatus};

/// Returns the recipes directory path, ensuring it exists.
pub fn recipes_dir() -> Result<PathBuf> {
    let dir = autodev_dir()?.join("recipes");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Initialize the recipes system — called at app startup.
/// Ensures the recipes directory exists and writes default system recipes.
pub fn init() -> Result<()> {
    let dir = recipes_dir()?;
    defaults::ensure_default_recipes(&dir)?;
    tracing::info!("Recipes initialized at {:?}", dir);
    Ok(())
}

/// List all recipes in the recipes directory.
pub fn list() -> Result<Vec<Recipe>> {
    let dir = recipes_dir()?;
    let (recipes, errors) = parser::load_all_recipes(&dir);
    for err in &errors {
        tracing::warn!("Recipe load error: {err}");
    }
    Ok(recipes)
}
