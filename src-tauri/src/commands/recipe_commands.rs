// Recipe commands — Tauri commands for listing/reading recipes.
// Phase 2.1: read-only (list + get). Create/Edit/Delete deferred to Phase 2.2.

use crate::errors::Result;
use crate::recipes::{self, Recipe};

/// List all recipes in ~/.autodev/recipes/
#[tauri::command]
pub fn list_recipes() -> Result<Vec<Recipe>> {
    recipes::list()
}

/// Get a specific recipe by id (filename stem).
#[tauri::command]
pub fn get_recipe(id: String) -> Result<Option<Recipe>> {
    let all = recipes::list()?;
    Ok(all.into_iter().find(|r| r.id == id))
}
