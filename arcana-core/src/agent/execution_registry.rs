//! Queryable, process-local view of executing tools. Only redacted arguments
//! enter this view; the executor still passes raw arguments solely to the tool.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;

use crate::events::{publish_internal, subscribe_internal, InternalEvent};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ActiveTool {
    pub session_id: String,
    pub agent_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
}

#[derive(Default)]
struct Registry {
    active: Mutex<HashMap<String, ActiveTool>>,
}

static REGISTRY: OnceLock<Arc<Registry>> = OnceLock::new();

fn registry() -> &'static Arc<Registry> {
    REGISTRY.get_or_init(|| {
        let registry = Arc::new(Registry::default());
        let reader = Arc::clone(&registry);
        subscribe_internal(Arc::new(move |event| {
            if let InternalEvent::ToolFinished { call_id, .. } = event {
                reader
                    .active
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .remove(call_id);
            }
        }));
        registry
    })
}

pub fn active_by_session(session_id: &str) -> Vec<ActiveTool> {
    let mut tools: Vec<_> = registry()
        .active
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .values()
        .filter(|tool| tool.session_id == session_id)
        .cloned()
        .collect();
    tools.sort_by(|a, b| a.call_id.cmp(&b.call_id));
    tools
}

pub fn active_by_agent(agent_id: &str) -> Vec<ActiveTool> {
    let mut tools: Vec<_> = registry()
        .active
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .values()
        .filter(|tool| tool.agent_id == agent_id)
        .cloned()
        .collect();
    tools.sort_by(|a, b| a.call_id.cmp(&b.call_id));
    tools
}

/// A guard covers cancellation and panics as well as normal returns. It owns
/// only identifiers and redacted input; no secret-bearing tool arguments.
pub struct ExecutionGuard {
    session_id: String,
    agent_id: String,
    call_id: String,
    completed: bool,
}

impl ExecutionGuard {
    pub fn start(tool: ActiveTool) -> Self {
        let guard = Self {
            session_id: tool.session_id.clone(),
            agent_id: tool.agent_id.clone(),
            call_id: tool.call_id.clone(),
            completed: false,
        };
        registry()
            .active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(tool.call_id.clone(), tool.clone());
        publish_internal(InternalEvent::ToolStarted {
            session_id: tool.session_id,
            agent_id: tool.agent_id,
            call_id: tool.call_id,
            tool_name: tool.tool_name,
        });
        guard
    }

    pub fn finish(mut self, is_error: bool) {
        self.completed = true;
        publish_internal(InternalEvent::ToolFinished {
            session_id: self.session_id.clone(),
            agent_id: self.agent_id.clone(),
            call_id: self.call_id.clone(),
            is_error,
            cancelled: false,
        });
    }
}

impl Drop for ExecutionGuard {
    fn drop(&mut self) {
        if !self.completed {
            publish_internal(InternalEvent::ToolFinished {
                session_id: self.session_id.clone(),
                agent_id: self.agent_id.clone(),
                call_id: self.call_id.clone(),
                is_error: true,
                cancelled: true,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_filters_and_cleans_success_error_and_cancel() {
        let session = uuid::Uuid::new_v4().to_string();
        let make = |id: &str| ActiveTool {
            session_id: session.clone(),
            agent_id: "worker-a".into(),
            call_id: id.into(),
            tool_name: "read_file".into(),
            input: serde_json::json!({"path":"safe.txt"}),
        };
        let ok = ExecutionGuard::start(make("ok"));
        let err = ExecutionGuard::start(make("err"));
        let cancelled = ExecutionGuard::start(make("cancel"));
        assert_eq!(active_by_session(&session).len(), 3);
        assert!(active_by_agent("worker-a")
            .iter()
            .any(|t| t.call_id == "ok"));
        ok.finish(false);
        err.finish(true);
        drop(cancelled);
        assert!(active_by_session(&session).is_empty());
    }
}
