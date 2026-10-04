// SkillLoader — scans ~/.arcana/skills/{category}/{name}.md on startup.
// Each file has a YAML frontmatter block (--- ... ---) followed by the markdown template.
// Phase 3.1

use std::path::{Path, PathBuf};
use tracing::{debug, warn};

use super::types::{Skill, SkillCategory, SkillFrontmatter};
use crate::db::arcana_dir;
use crate::errors::{ArcanaError, Result};

pub struct SkillLoader {
    skills_dir: PathBuf,
}

impl SkillLoader {
    pub fn new() -> Result<Self> {
        let dir = arcana_dir()?.join("skills");
        std::fs::create_dir_all(&dir)?;
        Ok(Self { skills_dir: dir })
    }

    /// Path to the skills root directory: ~/.arcana/skills/
    pub fn skills_dir(&self) -> &Path {
        &self.skills_dir
    }

    /// Load all skills by walking the skills directory tree.
    pub fn load_all(&self) -> Vec<Skill> {
        let mut skills = Vec::new();
        let categories = ["coding", "debugging", "devops", "fleet", "custom"];

        for cat in &categories {
            let cat_dir = self.skills_dir.join(cat);
            if !cat_dir.exists() {
                continue;
            }

            let entries = match std::fs::read_dir(&cat_dir) {
                Ok(e) => e,
                Err(e) => {
                    warn!("[SkillLoader] read_dir {cat}: {e}");
                    continue;
                }
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    match self.load_file(&path, cat) {
                        Ok(skill) => {
                            debug!("[SkillLoader] loaded {}", skill.name);
                            skills.push(skill);
                        }
                        Err(e) => {
                            warn!("[SkillLoader] failed {}: {e}", path.display());
                        }
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
            return Err(ArcanaError::Agent(format!(
                "Skill file {} is missing YAML frontmatter (--- block)",
                path.display()
            )));
        }

        // Find closing ---
        let after_open = &trimmed[3..];
        let close_pos = after_open.find("\n---").ok_or_else(|| {
            ArcanaError::Agent(format!(
                "Skill file {} has unclosed frontmatter",
                path.display()
            ))
        })?;

        let frontmatter_str = &after_open[..close_pos].trim_start_matches('\n');
        let template = after_open[close_pos + 4..]
            .trim_start_matches('\n')
            .to_string();

        let fm: SkillFrontmatter = serde_yaml::from_str(frontmatter_str).map_err(|e| {
            ArcanaError::Agent(format!(
                "Skill frontmatter parse error in {}: {e}",
                path.display()
            ))
        })?;
        arcana_core::platform::shell_portability::validate_persisted_agent_guidance(
            &fm.description,
        )
        .and_then(|_| {
            arcana_core::platform::shell_portability::validate_persisted_agent_guidance(&template)
        })
        .map_err(|e| ArcanaError::Agent(format!("Skill file {} {e}", path.display())))?;

        let category = if fm.category.is_empty() {
            SkillCategory::from_str(category_hint)
        } else {
            SkillCategory::from_str(&fm.category)
        };

        // Detect whether this is a builtin skill (lives in the builtin skills dir)
        let is_builtin = self.is_builtin_path(path);

        Ok(Skill {
            name: fm.name,
            description: fm.description,
            category,
            parameters: fm.parameters,
            version: fm.version,
            author: fm.author.unwrap_or_else(|| "Arcana".into()),
            tags: fm.tags.unwrap_or_default(),
            template,
            file_path: path.to_string_lossy().into_owned(),
            is_builtin,
            requires_capability: fm.requires_capability,
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

#[cfg(test)]
mod portability_tests {
    use super::*;

    fn loader() -> SkillLoader {
        SkillLoader {
            skills_dir: std::env::temp_dir().join("arcana-skill-portability-tests"),
        }
    }

    #[test]
    fn saved_custom_skill_is_refused_by_loader_when_shell_guidance_is_posix_only() {
        let loader = loader();
        let path = loader.skills_dir.join("custom").join("portable-check.md");
        let content = "---\nname: check\ndescription: portable check\ncategory: custom\nversion: '1'\n---\n```bash\nnpm test && npm lint\n```\n";
        assert!(loader.parse_skill_file(&path, content, "custom").is_err());
    }

    #[test]
    fn saved_custom_skill_is_refused_when_persisted_guidance_mentions_posix_temp_path() {
        let loader = loader();
        let path = loader.skills_dir.join("custom").join("portable-check.md");
        let content = "---\nname: check\ndescription: portable check\ncategory: custom\nversion: '1'\n---\nSave temporary output under /tmp/build.\n";
        let error = loader
            .parse_skill_file(&path, content, "custom")
            .expect_err("persisted skills must reject hardcoded POSIX temporary paths");
        assert!(error.to_string().contains("hardcoded POSIX temporary path"));
    }

    #[test]
    fn saved_custom_skill_with_portable_guidance_loads() {
        let loader = loader();
        let path = loader.skills_dir.join("custom").join("portable-check.md");
        let content = "---\nname: check\ndescription: portable check\ncategory: custom\nversion: '1'\n---\nUse project_command to verify the project.\n";
        assert!(loader.parse_skill_file(&path, content, "custom").is_ok());
    }
}
