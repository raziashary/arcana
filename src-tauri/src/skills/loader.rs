// SkillLoader — scans ~/.autodev/skills/{category}/{name}.md on startup.
// Each file has a YAML frontmatter block (--- ... ---) followed by the markdown template.
// Phase 3.1

use std::path::{Path, PathBuf};
use tracing::{debug, warn};

use crate::db::autodev_dir;
use crate::errors::{AutoDevError, Result};
use super::types::{Skill, SkillCategory, SkillFrontmatter};

pub struct SkillLoader {
    skills_dir: PathBuf,
}

impl SkillLoader {
    pub fn new() -> Result<Self> {
        let dir = autodev_dir()?.join("skills");
        std::fs::create_dir_all(&dir)?;
        Ok(Self { skills_dir: dir })
    }

    /// Path to the skills root directory: ~/.autodev/skills/
    pub fn skills_dir(&self) -> &Path {
        &self.skills_dir
    }

    /// Load all skills by walking the skills directory tree.
    pub fn load_all(&self) -> Vec<Skill> {
        let mut skills = Vec::new();
        let categories = ["coding", "debugging", "devops", "fleet", "custom"];

        for cat in &categories {
            let cat_dir = self.skills_dir.join(cat);
            if !cat_dir.exists() { continue; }

            let entries = match std::fs::read_dir(&cat_dir) {
                Ok(e) => e,
                Err(e) => { warn!("[SkillLoader] read_dir {cat}: {e}"); continue; }
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    match self.load_file(&path, cat) {
                        Ok(skill) => { debug!("[SkillLoader] loaded {}", skill.name); skills.push(skill); }
                        Err(e)    => { warn!("[SkillLoader] failed {}: {e}", path.display()); }
                    }
                }
            }
        }

        debug!("[SkillLoader] loaded {} skills total", skills.len());
        skills
    }

    /// Load a single skill .md file.
    pub fn load_file(&self, path: &Path, category_hint: &str) -> Result<Skill> {
        let content = std::fs::read_to_string(path)?;
        self.parse_skill_file(path, &content, category_hint)
    }

    /// Parse YAML frontmatter + markdown body from a skill file.
    fn parse_skill_file(&self, path: &Path, content: &str, category_hint: &str) -> Result<Skill> {
        // Expect file to start with ---\n
        let trimmed = content.trim_start();
        if !trimmed.starts_with("---") {
            return Err(AutoDevError::Agent(format!(
                "Skill file {} is missing YAML frontmatter (--- block)",
                path.display()
            )));
        }

        // Find closing ---
        let after_open = &trimmed[3..];
        let close_pos = after_open.find("\n---")
            .ok_or_else(|| AutoDevError::Agent(
                format!("Skill file {} has unclosed frontmatter", path.display())
            ))?;

        let frontmatter_str = &after_open[..close_pos].trim_start_matches('\n');
        let template = after_open[close_pos + 4..].trim_start_matches('\n').to_string();

        let fm: SkillFrontmatter = serde_yaml::from_str(frontmatter_str)
            .map_err(|e| AutoDevError::Agent(
                format!("Skill frontmatter parse error in {}: {e}", path.display())
            ))?;

        let category = if fm.category.is_empty() {
            SkillCategory::from_str(category_hint)
        } else {
            SkillCategory::from_str(&fm.category)
        };

        // Detect whether this is a builtin skill (lives in the builtin skills dir)
        let is_builtin = self.is_builtin_path(path);

        Ok(Skill {
            name:       fm.name,
            description: fm.description,
            category,
            parameters: fm.parameters,
            version:    fm.version,
            author:     fm.author.unwrap_or_else(|| "AutoDev".into()),
            tags:       fm.tags.unwrap_or_default(),
            template,
            file_path:  path.to_string_lossy().into_owned(),
            is_builtin,
        })
    }

    /// A skill is builtin if it was written by ensure_default_skills().
    fn is_builtin_path(&self, path: &Path) -> bool {
        path.starts_with(&self.skills_dir)
    }
}

impl Default for SkillLoader {
    fn default() -> Self {
        Self::new().expect("SkillLoader: cannot determine home directory")
    }
}
