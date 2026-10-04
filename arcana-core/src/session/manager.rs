// SessionManager — high-level session lifecycle management.
// Wraps SessionStorage with business logic for create/list/get/delete.

use super::storage::SessionStorage;
use super::types::{Message, Session};
use crate::errors::Result;
use crate::state::lock::LockTolerant;
use std::sync::{Arc, Mutex};

pub struct SessionManager {
    storage: Arc<Mutex<SessionStorage>>,
}

impl SessionManager {
    pub fn new() -> Result<Self> {
        let storage = SessionStorage::open()?;
        Ok(Self {
            storage: Arc::new(Mutex::new(storage)),
        })
    }

    pub fn create_session(
        &self,
        working_dir: &str,
        provider: &str,
        model: &str,
    ) -> Result<Session> {
        let workspace = crate::platform::workspace::resolve_workspace_dir(Some(working_dir))?;
        let session = Session::new(&workspace.to_string_lossy(), provider, model);
        let storage = self.storage.lock_tolerant();
        storage.upsert_session(&session)?;
        Ok(session)
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        self.storage.lock_tolerant().list_sessions()
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        self.storage
            .lock()
            .unwrap()
            .get_session_with_messages(session_id)
    }

    pub fn get_session_messages(&self, session_id: &str) -> Result<Vec<Message>> {
        match self
            .storage
            .lock()
            .unwrap()
            .get_session_with_messages(session_id)?
        {
            Some(session) => Ok(session.messages),
            None => Ok(vec![]),
        }
    }

    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        self.storage.lock_tolerant().delete_session(session_id)
    }

    pub fn append_message(&self, msg: &Message) -> Result<()> {
        self.storage.lock_tolerant().insert_message(msg)
    }

    pub fn update_message(&self, msg: &Message) -> Result<()> {
        self.storage.lock_tolerant().update_message(msg)
    }

    pub fn update_session_title(&self, session: &Session) -> Result<()> {
        self.storage.lock_tolerant().upsert_session(session)
    }

    /// Phase 3.0 R3 — persist the PA mode toggle for a session.
    pub fn set_pa_mode(&self, session_id: &str, pa_mode: &str) -> Result<()> {
        self.storage
            .lock()
            .unwrap()
            .update_session_pa_mode(session_id, pa_mode)
    }
}

#[cfg(test)]
mod workspace_resolution_tests {
    use super::*;

    #[test]
    fn sessions_require_and_store_the_canonical_workspace() {
        let home =
            std::env::temp_dir().join(format!("arcana-session-workspace-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&home).expect("create isolated Arcana home");
        let _home = crate::db::test_support::with_home(&home);
        let manager = SessionManager::new().expect("open isolated session storage");
        assert!(
            manager
                .create_session("", "google", "gemini-2.5-flash")
                .is_err(),
            "unset workspace must not fall back to CWD"
        );

        let configured = home.join("project");
        let explicit = home.join("explicit-project");
        std::fs::create_dir_all(&configured).expect("create configured project");
        std::fs::create_dir_all(&explicit).expect("create explicit project");
        crate::db::app_settings::set(
            crate::db::app_settings::WORKSPACE_KEY,
            &configured.to_string_lossy(),
        )
        .expect("save configured project");

        let configured_session = manager
            .create_session("", "google", "gemini-2.5-flash")
            .expect("create session from configured workspace");
        assert_eq!(
            configured_session.working_dir,
            configured
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        );
        let explicit_session = manager
            .create_session(&explicit.to_string_lossy(), "google", "gemini-2.5-flash")
            .expect("create session from explicit workspace");
        assert_eq!(
            explicit_session.working_dir,
            explicit
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        );
        assert!(
            manager
                .create_session("relative/path", "google", "gemini-2.5-flash")
                .is_err(),
            "relative paths must not be resolved against CWD"
        );

        drop(manager);
        drop(_home);
        std::fs::remove_dir_all(home).expect("remove isolated Arcana home");
    }
}
