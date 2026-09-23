// Recipe parser — reads YAML recipe files from ~/.arcana/recipes/
// Uses serde_yaml for parsing. Each .yaml file maps to one Recipe struct.

use super::types::Recipe;
use crate::errors::{ArcanaError, Result};
use std::path::Path;

/// Parse a single recipe from a YAML string.
pub fn parse_recipe(yaml: &str) -> Result<Recipe> {
    serde_yaml::from_str(yaml)
        .map_err(|e| ArcanaError::Agent(format!("Failed to parse recipe YAML: {e}")))
}

/// Load all recipes from the recipes directory.
/// Returns (valid_recipes, errors) — partial failure is OK.
pub fn load_all_recipes(recipes_dir: &Path) -> (Vec<Recipe>, Vec<String>) {
    let mut recipes = Vec::new();
    let mut errors = Vec::new();

    let entries = match std::fs::read_dir(recipes_dir) {
        Ok(e) => e,
        Err(e) => {
            errors.push(format!("Cannot read recipes dir {:?}: {e}", recipes_dir));
            return (recipes, errors);
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .map(|e| e == "yaml" || e == "yml")
            .unwrap_or(false)
        {
            match std::fs::read_to_string(&path) {
                Ok(yaml) => {
                    match parse_recipe(&yaml) {
                        Ok(mut recipe) => {
                            // Derive id from filename if not set
                            if recipe.id.is_empty() {
                                recipe.id = path
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                            }
                            recipes.push(recipe);
                        }
                        Err(e) => errors.push(format!("{:?}: {e}", path)),
                    }
                }
                Err(e) => errors.push(format!("Failed to read {:?}: {e}", path)),
            }
        }
    }

    (recipes, errors)
}

/// Load a single recipe by its id from the recipes directory.
pub fn load_recipe_by_id(recipe_id: &str) -> Result<Option<Recipe>> {
    let dir = crate::recipes::recipes_dir()?;
    let path = dir.join(format!("{recipe_id}.yaml"));
    if !path.exists() {
        return Ok(None);
    }
    let yaml = std::fs::read_to_string(&path)?;
    let recipe = parse_recipe(&yaml)?;
    Ok(Some(recipe))
}

/// Write a recipe to a YAML file.
pub fn save_recipe(recipes_dir: &Path, recipe: &Recipe) -> Result<()> {
    let yaml = serde_yaml::to_string(recipe)
        .map_err(|e| ArcanaError::Agent(format!("Failed to serialize recipe: {e}")))?;
    let path = recipes_dir.join(format!("{}.yaml", recipe.id));
    std::fs::write(&path, yaml)?;
    Ok(())
}
