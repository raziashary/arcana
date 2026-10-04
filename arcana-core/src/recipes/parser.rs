// Recipe parser — reads YAML recipe files from ~/.arcana/recipes/
// Uses serde_yaml for parsing. Each .yaml file maps to one Recipe struct.

use super::types::Recipe;
use crate::errors::{ArcanaError, Result};
use std::path::Path;

/// Parse a single recipe from a YAML string.
pub fn parse_recipe(yaml: &str) -> Result<Recipe> {
    let recipe: Recipe = serde_yaml::from_str(yaml)
        .map_err(|e| ArcanaError::Agent(format!("Failed to parse recipe YAML: {e}")))?;
    if let Some(description) = &recipe.description {
        crate::platform::shell_portability::validate_persisted_agent_guidance(description)
            .map_err(|e| ArcanaError::Agent(format!("Recipe '{}' description {e}", recipe.id)))?;
    }
    crate::platform::shell_portability::validate_persisted_agent_guidance(&recipe.instructions)
        .map_err(|e| ArcanaError::Agent(format!("Recipe '{}' {e}", recipe.id)))?;
    Ok(recipe)
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
    if let Some(description) = &recipe.description {
        crate::platform::shell_portability::validate_persisted_agent_guidance(description)
            .map_err(|e| ArcanaError::Agent(format!("Recipe '{}' description {e}", recipe.id)))?;
    }
    crate::platform::shell_portability::validate_persisted_agent_guidance(&recipe.instructions)
        .map_err(|e| ArcanaError::Agent(format!("Recipe '{}' {e}", recipe.id)))?;
    let yaml = serde_yaml::to_string(recipe)
        .map_err(|e| ArcanaError::Agent(format!("Failed to serialize recipe: {e}")))?;
    let path = recipes_dir.join(format!("{}.yaml", recipe.id));
    std::fs::write(&path, yaml)?;
    Ok(())
}

#[cfg(test)]
mod portability_tests {
    use super::*;
    use crate::recipes::types::RecipePermissionMode;

    fn recipe(instructions: &str) -> Recipe {
        Recipe {
            version: "1.0.0".into(),
            id: "custom-worker".into(),
            title: "Custom worker".into(),
            description: None,
            schedule: None,
            permission_mode: RecipePermissionMode::Autonomous,
            instructions: instructions.into(),
            agent_role: None,
            model: None,
            is_system: false,
        }
    }

    #[test]
    fn recipe_parse_and_save_reject_custom_posix_shell_guidance() {
        let yaml = "version: '1.0.0'\nid: custom-worker\ntitle: Custom worker\npermission_mode: autonomous\ninstructions: |\n  ```bash\n  npm test && npm lint\n  ```\nis_system: false\n";
        assert!(
            parse_recipe(yaml).is_err(),
            "saved recipe reader must reject it"
        );

        let prose_yaml = serde_yaml::to_string(&recipe("Write output under /tmp/build")).unwrap();
        assert!(
            parse_recipe(&prose_yaml).is_err(),
            "persisted recipe reader must reject POSIX temp paths even in prose"
        );

        let directory = tempfile::tempdir().unwrap();
        assert!(save_recipe(directory.path(), &recipe("Write output under /tmp/build")).is_err());
        assert!(!directory.path().join("custom-worker.yaml").exists());
    }

    #[test]
    fn portable_custom_recipe_roundtrips_through_production_reader() {
        let directory = tempfile::tempdir().unwrap();
        save_recipe(
            directory.path(),
            &recipe("Use project_command for verification."),
        )
        .unwrap();
        let saved = std::fs::read_to_string(directory.path().join("custom-worker.yaml")).unwrap();
        assert_eq!(
            parse_recipe(&saved).unwrap().instructions,
            "Use project_command for verification."
        );
    }
}
