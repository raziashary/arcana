// Memory types — DTOs for all 3 memory tiers + recall assembler.
// Phase 4.1-4.6 — PA Memory System.
// See: DEVELOPMENT_PLAN.md Phase 4.1-4.6

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ─── Tier enum ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTier {
    Episodic,   // Tier 1: raw conversation experience
    Semantic,   // Tier 2: extracted facts and knowledge
    Procedural, // Tier 3: skills, patterns, learnings
}

// ─── Tier 1: Episodic Memory ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpisodicRole {
    User,
    Agent,
}

impl EpisodicRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            EpisodicRole::User  => "user",
            EpisodicRole::Agent => "agent",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicMemory {
    pub id:           String,
    pub session_id:   String,
    /// 1-indexed turn counter within the session. Both the user message and the
    /// agent response that immediately follows share the same turn_index.
    pub turn_index:   u32,
    pub role:         EpisodicRole,
    pub content:      String,
    pub model:        Option<String>,
    pub cost_usd:     f64,
    pub tokens_in:    u32,
    pub tokens_out:   u32,
    /// Agent extended thinking (Claude extended-thinking feature).
    pub thinking:     Option<String>,
    /// JSON array of tool call objects logged for this turn.
    pub tool_calls:   Option<serde_json::Value>,
    pub working_dir:  Option<String>,
    /// `None` for normal messages; `"session_summary"` for auto-generated summaries.
    pub summary_type: Option<String>,
    /// Freeform JSON metadata bucket (model config, capabilities, etc.).
    pub metadata:     Option<serde_json::Value>,
    pub created_at:   DateTime<Utc>,
}

// ─── Tier 2: Semantic Memory ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactCategory {
    Preference,   // user preferences — "prefers tabs over spaces"
    Architecture, // architectural decisions — "use rusqlite not sea-orm"
    Pattern,      // coding patterns the PA should follow
    Reference,    // important file/symbol locations
    UserProfile,  // facts about the user themselves
}

impl FactCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            FactCategory::Preference   => "preference",
            FactCategory::Architecture => "architecture",
            FactCategory::Pattern      => "pattern",
            FactCategory::Reference    => "reference",
            FactCategory::UserProfile  => "user_profile",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "architecture" => FactCategory::Architecture,
            "pattern"      => FactCategory::Pattern,
            "reference"    => FactCategory::Reference,
            "user_profile" => FactCategory::UserProfile,
            _              => FactCategory::Preference,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFact {
    pub id:             String,
    pub fact_text:      String,
    pub category:       String,
    /// 0.0–1.0. Auto-extracted facts start at 0.7; user-explicit facts start at 1.0.
    pub confidence:     f64,
    /// "user_explicit" | "extracted" | "inferred"
    pub source_type:    String,
    pub source_session: Option<String>,
    pub working_dir:    Option<String>,
    pub tags:           Vec<String>,
    pub is_active:      bool,
    pub created_at:     DateTime<Utc>,
    pub updated_at:     DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactHistory {
    pub id:         String,
    pub fact_id:    String,
    pub old_text:   String,
    pub new_text:   String,
    pub changed_at: DateTime<Utc>,
}

// ─── Tier 3: Procedural Memory ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProceduralMemory {
    pub id:         String,
    pub topic:      String, // filename key: "debugging", "workflow-patterns"
    pub title:      String,
    pub content:    String, // full markdown body
    pub tags:       Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ─── Recall ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallRequest {
    pub session_id:  String,
    pub query:       String, // free-text query for relevance scoring
    pub working_dir: Option<String>,
    pub tier_filter: Option<Vec<MemoryTier>>,
    pub limit:       Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallResult {
    pub tier:       MemoryTier,
    pub id:         String,
    pub content:    String,
    pub score:      f64, // relevance 0.0–1.0
    pub created_at: DateTime<Utc>,
}

// ─── Stats ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub episodic_count:   u64,
    pub semantic_count:   u64,
    pub procedural_count: u64,
    /// Distinct sessions present in the episodic table.
    pub session_count:    u64,
    pub last_updated:     Option<DateTime<Utc>>,
}
