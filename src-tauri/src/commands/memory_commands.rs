// Memory system Tauri commands — Phase 4.1-4.6
// Thin command layer over the memory module; all heavy lifting is in
// src/memory/. Commands return ArcanaError on failure (serialised to frontend).
//
// Command groups:
//   Episodic  — get_session_timeline
//   Semantic  — recall_facts, store_fact_cmd, update_fact_cmd, deactivate_fact,
//               get_user_preferences, search_semantic
//   Procedural — get_memory_md, save_memory_md, list_memory_topics,
//               get_memory_topic, save_memory_topic, delete_memory_topic,
//               search_procedural
//   Cross-tier — search_memory, get_memory_stats
//
// NOTE: store_user_message_cmd / store_agent_response_cmd are intentionally
// NOT Tauri commands — those writes happen inside agent_commands.rs (Rust-side)
// so the frontend never needs to drive the episodic store directly.

use crate::errors::Result;
use crate::memory::{
    recall, EpisodicMemory, EpisodicStore, FactCategory, MemoryStats, ProceduralMemory,
    ProceduralStore, RecallRequest, RecallResult, SemanticFact, SemanticStore,
};

// ─── Episodic commands ───────────────────────────────────────────────────────

/// Full timeline for one session — used by the Memory Sidebar timeline view.
#[tauri::command]
pub fn get_session_timeline(session_id: String) -> Result<Vec<EpisodicMemory>> {
    EpisodicStore::get_session_timeline(&session_id)
}

// ─── Semantic commands ───────────────────────────────────────────────────────

/// Active facts ordered by confidence (descending).
#[tauri::command]
pub fn recall_facts(limit: Option<u32>) -> Result<Vec<SemanticFact>> {
    SemanticStore::recall_facts(limit.unwrap_or(50))
}

/// Explicitly store a fact (user-explicit, confidence = 1.0).
#[tauri::command]
pub fn store_fact_cmd(
    fact_text: String,
    category: String,
    session_id: Option<String>,
    working_dir: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<String> {
    SemanticStore::store_fact(
        &fact_text,
        FactCategory::from_str(&category),
        1.0,
        "user_explicit",
        session_id.as_deref(),
        working_dir.as_deref(),
        tags.as_deref().unwrap_or(&[]),
    )
}

/// Update an existing fact's text (records old value in fact_history).
#[tauri::command]
pub fn update_fact_cmd(id: String, new_text: String, confidence: Option<f64>) -> Result<()> {
    SemanticStore::update_fact(&id, &new_text, confidence)
}

/// Soft-delete a fact (is_active → 0).
#[tauri::command]
pub fn deactivate_fact(id: String) -> Result<()> {
    SemanticStore::deactivate_fact(&id)
}

/// Facts in preference + user_profile categories.
#[tauri::command]
pub fn get_user_preferences() -> Result<Vec<SemanticFact>> {
    SemanticStore::get_user_preferences()
}

/// Keyword search within semantic tier.
#[tauri::command]
pub fn search_semantic(query: String, limit: Option<u32>) -> Result<Vec<SemanticFact>> {
    SemanticStore::search(&query, limit.unwrap_or(20))
}

// ─── Procedural commands ─────────────────────────────────────────────────────

/// Full MEMORY.md contents.
#[tauri::command]
pub fn get_memory_md() -> Result<String> {
    ProceduralStore::get_memory_md()
}

/// Overwrite MEMORY.md with new content.
#[tauri::command]
pub fn save_memory_md(content: String) -> Result<()> {
    ProceduralStore::save_memory_md(&content)
}

/// List all topic files from disk.
#[tauri::command]
pub fn list_memory_topics() -> Result<Vec<ProceduralMemory>> {
    ProceduralStore::list_topics()
}

/// Get a single topic file by name.
#[tauri::command]
pub fn get_memory_topic(topic: String) -> Result<ProceduralMemory> {
    ProceduralStore::get_topic(&topic)
}

/// Create or overwrite a topic file + SQLite mirror.
#[tauri::command]
pub fn save_memory_topic(
    topic: String,
    title: String,
    content: String,
    tags: Option<Vec<String>>,
) -> Result<()> {
    ProceduralStore::save_topic(&topic, &title, &content, tags.as_deref().unwrap_or(&[]))
}

/// Delete a topic file.
#[tauri::command]
pub fn delete_memory_topic(topic: String) -> Result<()> {
    ProceduralStore::delete_topic(&topic)
}

/// Keyword search within procedural tier.
#[tauri::command]
pub fn search_procedural(query: String, limit: Option<u32>) -> Result<Vec<ProceduralMemory>> {
    ProceduralStore::search(&query, limit.unwrap_or(10))
}

// ─── Cross-tier commands ─────────────────────────────────────────────────────

/// Full cross-tier search — used by the search bar in the Memory Sidebar.
#[tauri::command]
pub fn search_memory(
    session_id: String,
    query: String,
    working_dir: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<RecallResult>> {
    let req = RecallRequest {
        session_id,
        query,
        working_dir,
        tier_filter: None,
        limit,
    };
    recall::search_all(&req)
}

/// Aggregate memory statistics for the stats panel.
#[tauri::command]
pub fn get_memory_stats() -> Result<MemoryStats> {
    Ok(recall::get_stats())
}

/// Manually trigger session summarization (e.g. from "End Session" button).
#[tauri::command]
pub fn summarize_session_cmd(session_id: String, working_dir: Option<String>) -> Result<String> {
    crate::memory::Summarizer::summarize_session(&session_id, working_dir.as_deref())
}
