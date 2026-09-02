// PA Memory System — Phase 4.1-4.8
// 3-tier persistent memory architecture:
//
//   Tier 1 — Episodic  (episodic.rs)
//     Raw conversation storage: every user message + agent response,
//     turn-indexed per session. Session summaries enable cross-session recall.
//
//   Tier 2 — Semantic  (semantic.rs)
//     Extracted facts: user preferences, architecture decisions, patterns,
//     reference locations. Rule-based extraction + explicit `remember_this` tool.
//
//   Tier 3 — Procedural (procedural.rs)
//     Skills and patterns stored as MEMORY.md + topic/*.md files on disk,
//     mirrored to SQLite for full-text search.
//
// Storage:  ~/.autodev/memory.db  (SQLite, WAL mode)
// Files:    ~/.autodev/memory/MEMORY.md  +  topics/<topic>.md
//
// See: DEVELOPMENT_PLAN.md Phase 4.1-4.8

pub mod backend;
pub mod vector;
pub mod db;
pub mod episodic;
pub mod migration;
pub mod procedural;
pub mod recall;
pub mod semantic;
pub mod summarizer;
pub mod types;

pub use db::MemoryDb;
pub use episodic::EpisodicStore;
pub use procedural::ProceduralStore;
pub use recall::{build_memory_context, get_stats, search_all};
pub use semantic::SemanticStore;
pub use summarizer::Summarizer;
pub use types::{
    EpisodicMemory, EpisodicRole, FactCategory, MemoryStats, MemoryTier, ProceduralMemory,
    RecallRequest, RecallResult, SemanticFact,
};
