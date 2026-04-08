// SessionManager — high-level session lifecycle management.
// Wraps SessionStorage with business logic for create/list/get/delete.

use std::sync::{Arc, Mutex};
use crate::errors::Result;
use super::storage::SessionStorage;
use super::types::{Message, Session};

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
        let session = Session::new(working_dir, provider, model);
        let storage = self.storage.lock().unwrap();
        storage.upsert_session(&session)?;
        Ok(session)
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        self.storage.lock().unwrap().list_sessions()
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        self.storage.lock().unwrap().get_session_with_messages(session_id)
    }

    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        self.storage.lock().unwrap().delete_session(session_id)
    }

    pub fn append_message(&self, msg: &Message) -> Result<()> {
        self.storage.lock().unwrap().insert_message(msg)
    }

    pub fn update_message(&self, msg: &Message) -> Result<()> {
        self.storage.lock().unwrap().update_message(msg)
    }

    pub fn update_session_title(&self, session: &Session) -> Result<()> {
        self.storage.lock().unwrap().upsert_session(session)
    }
}
