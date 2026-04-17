// Skills system types — Phase 3.1
// Skills are reusable .md workflows with YAML frontmatter, invocable via slash commands
// or programmatically by Fleet workers. Stored in ~/.autodev/skills/{category}/{name}.md

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// ── Category ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SkillCategory {
    Coding,
    Debugging,
    DevOps,
    Fleet,
    Custom,
}

impl SkillCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Coding    => "coding",
            Self::Debugging => "debugging",
            Self::DevOps    => "devops",
            Self::Fleet     => "fleet",
            Self::Custom    => "custom",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "coding"    => Self::Coding,
            "debugging" => Self::Debugging,
            "devops"    => Self::DevOps,
            "fleet"     => Self::Fleet,
            _           => Self::Custom,
        }
    }
}

// ── Parameter ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillParamType {
    String,
    Boolean,
    Integer,
    Enum,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillParameter {
    pub name:        String,
    #[serde(rename = "type")]
    pub param_type:  SkillParamType,
    pub required:    bool,
    pub description: String,
    pub default:     Option<serde_json::Value>,
    pub values:      Option<Vec<String>>,    // Valid values for Enum type
}

// ── Frontmatter (YAML header of each .md skill file) ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFrontmatter {
    pub name:        String,
    pub description: String,
    pub category:    String,
    pub parameters:  Vec<SkillParameter>,
    pub version:     String,
    pub author:      Option<String>,
    pub tags:        Option<Vec<String>>,
}

// ── Full Skill (frontmatter + template body) ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name:        String,
    pub description: String,
    pub category:    SkillCategory,
    pub parameters:  Vec<SkillParameter>,
    pub version:     String,
    pub author:      String,
    pub tags:        Vec<String>,
    pub template:    String,             // Raw markdown body with {{param}} placeholders
    pub file_path:   String,             // Absolute path to .md file on disk
    pub is_builtin:  bool,
}

// ── Summary (for list API — omits template for efficiency) ────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSummary {
    pub name:        String,
    pub description: String,
    pub category:    SkillCategory,
    pub tags:        Vec<String>,
    pub version:     String,
    pub is_builtin:  bool,
    pub param_count: usize,
}

impl From<&Skill> for SkillSummary {
    fn from(s: &Skill) -> Self {
        Self {
            name:        s.name.clone(),
            description: s.description.clone(),
            category:    s.category.clone(),
            tags:        s.tags.clone(),
            version:     s.version.clone(),
            is_builtin:  s.is_builtin,
            param_count: s.parameters.len(),
        }
    }
}

// ── Execution ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecutionResult {
    pub skill_name:  String,
    pub rendered:    String,             // Template with parameters substituted
    pub executed_at: DateTime<Utc>,
    pub parameters:  serde_json::Map<String, serde_json::Value>,
}

// ── Validation ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub field:   String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid:  bool,
    pub errors: Vec<ValidationError>,
}

impl ValidationResult {
    pub fn ok() -> Self {
        Self { valid: true, errors: vec![] }
    }

    pub fn fail(errors: Vec<ValidationError>) -> Self {
        Self { valid: false, errors }
    }
}
