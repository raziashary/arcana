// PA Memory System — Phase 4.1-4.8
// 3-tier persistent memory architecture:
//
//   Tier 1 — Episodic  (episodic.rs)
//     Raw conversation storage: every user message + agent response,
//     turn-indexed per session. Session summaries enable cross-session recall.
//
//   Tier 2 — Semantic  (semantic.rs)
//     Extracted facts: user preferences, architecture decisions, patterns,
//     reference locations. Rule-based extraction + the `propose_memory`
//     gateway (P3-39; `remember_this` was removed).
//
//   Tier 3 — Procedural (procedural.rs)
//     Skills and patterns stored as MEMORY.md + topic/*.md files on disk,
//     mirrored to SQLite for full-text search.
//
// Storage:  ~/.arcana/memory.db  (SQLite, WAL mode)
// Files:    ~/.arcana/memory/MEMORY.md  +  topics/<topic>.md
//
// See: DEVELOPMENT_PLAN.md Phase 4.1-4.8

pub mod backend;
pub mod db;
pub mod embed;
pub mod episodic;
pub mod gateway;
pub mod manager;
pub mod migration;
pub mod procedural;
pub mod rank;
pub mod recall;
pub mod semantic;
pub mod summarizer;
pub mod types;
pub mod vector;

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

/// Apply `memory.db`'s real migration ladder to `conn`.
///
/// For integration tests only — but it runs the SHIPPED ladder, so a test can
/// never pass against a schema the product does not build.
pub fn run_ladder_for_tests(conn: &rusqlite::Connection) -> crate::errors::Result<i32> {
    db::MemoryDb::run_ladder_on(conn)
}
