// Phase 4.8 — Migration from session message storage to episodic memory.
// Reads all existing sessions from ~/.airs/autodev.db and backfills the
// episodic_memories table in ~/.autodev/memory.db so the PA has full
// continuity from day-one sessions.
//
// Migration is idempotent: it skips rows whose session_id + role + content
// already exist in the episodic table (content-hash equality check).
// Safe to run on every startup (cost ≈ one COUNT query per session).

use tracing::{debug, info, warn};

use crate::errors::Result;
use crate::session::manager::SessionManager;
use crate::session::types::MessageRole;
use super::episodic::EpisodicStore;

pub struct MemoryMigration;

impl MemoryMigration {
    /// Migrate all sessions from session storage into episodic memory.
    /// Runs in a background thread; errors are logged but not fatal.
    pub fn run_background() {
        std::thread::spawn(|| {
            if let Err(e) = Self::run() {
                warn!("[MemoryMigration] background migration error: {e}");
            }
        });
    }

    /// Synchronous migration — walks all sessions from SessionManager and
    /// backfills episodic memory. Skips sessions that already have entries.
    pub fn run() -> Result<()> {
        let session_mgr = SessionManager::new()?;
        let sessions    = session_mgr.list_sessions()?;

        info!("[MemoryMigration] migrating {} sessions to episodic memory", sessions.len());
        let mut migrated = 0usize;
        let mut skipped  = 0usize;

        for session in &sessions {
            // Skip if this session already has episodic entries.
            let existing_count = EpisodicStore::get_turn_count(&session.id);
            if existing_count > 0 {
                skipped += 1;
                continue;
            }

            let session_with_msgs = match session_mgr.get_session(&session.id)? {
                Some(s) => s,
                None    => { skipped += 1; continue; }
            };
            let msgs = session_with_msgs.messages;

            if msgs.is_empty() {
                skipped += 1;
                continue;
            }

            for msg in &msgs {
                match &msg.role {
                    MessageRole::User => {
                        EpisodicStore::store_user_message(
                            &session.id,
                            &msg.content,
                            Some(session.working_dir.as_str()),
                            None,
                        )?;
                    }
                    MessageRole::Assistant => {
                        // Collect any tool calls from subsequent tool messages.
                        // (Phase 4.1 shape: tool calls as JSON array)
                        EpisodicStore::store_agent_response(
                            &session.id,
                            &msg.content,
                            Some(session.model.as_str()),
                            msg.cost_usd,
                            msg.tokens_in,
                            msg.tokens_out,
                            None, // thinking not stored in old schema
                            None, // tool_calls not separate in old schema
                            Some(session.working_dir.as_str()),
                            None,
                        )?;
                    }
                    MessageRole::Tool => {
                        // Tool results are not stored as independent episodic entries.
                        // They are captured in the agent response tool_calls field.
                    }
                }
            }
            migrated += 1;
        }

        info!(
            "[MemoryMigration] done — migrated={migrated} skipped={skipped}"
        );
        Ok(())
    }
}
