// SkillRegistry — in-memory index of loaded skills, organised by category.
// Reload from disk is triggered on startup and via reload_skills command.
// Phase 3.1

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::info;

use super::loader::SkillLoader;
use super::types::{Skill, SkillCategory, SkillSummary};
use crate::errors::{ArcanaError, Result};
use arcana_core::state::lock::RwLockTolerant;

#[derive(Debug, Default)]
struct RegistryInner {
    skills: HashMap<String, Skill>, // skill.name → Skill
}

#[derive(Debug, Clone)]
pub struct SkillRegistry {
    inner: Arc<RwLock<RegistryInner>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RegistryInner::default())),
        }
    }

    /// Load (or reload) all skills from disk.
    pub fn reload(&self) -> Result<usize> {
        let loader = SkillLoader::new()?;
        let loaded = loader.load_all();
        let count = loaded.len();

        let mut inner = self.inner.write_tolerant();
        inner.skills.clear();
        for skill in loaded {
            inner.skills.insert(skill.name.clone(), skill);
        }

        info!("[SkillRegistry] reloaded {count} skills");
        Ok(count)
    }

    /// Get a single skill by name (exact match).
    pub fn get(&self, name: &str) -> Result<Skill> {
        self.inner
            .read()
            .unwrap()
            .skills
            .get(name)
            .cloned()
            .ok_or_else(|| ArcanaError::Agent(format!("Skill '{name}' not found")))
    }

    /// List all skills, optionally filtered by category.
    pub fn list(&self, category_filter: Option<SkillCategory>) -> Vec<SkillSummary> {
        self.inner
            .read()
            .unwrap()
            .skills
            .values()
            .filter(|s| {
                category_filter
                    .as_ref()
                    .map(|c| &s.category == c)
                    .unwrap_or(true)
            })
            .map(SkillSummary::from)
            .collect()
    }

    /// Search skills by name or tag prefix (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<SkillSummary> {
        let q = query.to_lowercase();
        self.inner
            .read()
            .unwrap()
            .skills
            .values()
            .filter(|s| {
                s.name.to_lowercase().contains(&q)
                    || s.description.to_lowercase().contains(&q)
                    || s.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .map(SkillSummary::from)
            .collect()
    }

    /// Total count of loaded skills.
    pub fn count(&self) -> usize {
        self.inner.read_tolerant().skills.len()
    }

    /// Return names grouped by category (for palette sidebar).
    pub fn by_category(&self) -> HashMap<String, Vec<SkillSummary>> {
        let mut map: HashMap<String, Vec<SkillSummary>> = HashMap::new();
        for skill in self.inner.read_tolerant().skills.values() {
            map.entry(skill.category.as_str().to_string())
                .or_default()
                .push(SkillSummary::from(skill));
        }
        map
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}
