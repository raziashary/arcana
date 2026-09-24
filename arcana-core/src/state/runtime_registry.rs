//! Ephemeral admission state for native agent runs.
//!
//! Engines are created per turn/tick. This registry retains only their
//! `(agent_id, session_id)` ownership, never an engine or persisted messages.

use crate::errors::{ArcanaError, Result};
use crate::state::lock::LockTolerant;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const MAX_RUNTIMES: usize = 500;
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(600);
pub const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct RuntimeKey {
    agent_id: String,
    session_id: String,
}

struct RuntimeEntry {
    active: bool,
    last_touched: Instant,
}

/// Tracks native run ownership without extending an `AgentEngine`'s lifetime.
#[derive(Default)]
pub struct RuntimeRegistry {
    entries: Mutex<HashMap<RuntimeKey, RuntimeEntry>>,
}

pub struct RuntimeLease {
    registry: Arc<RuntimeRegistry>,
    key: RuntimeKey,
    released: bool,
}

impl RuntimeRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Admit one run. A second active run for the same pair is never allowed.
    pub fn admit(self: &Arc<Self>, agent_id: &str, session_id: &str) -> Result<RuntimeLease> {
        self.admit_at(agent_id, session_id, Instant::now())
    }

    fn admit_at(
        self: &Arc<Self>,
        agent_id: &str,
        session_id: &str,
        now: Instant,
    ) -> Result<RuntimeLease> {
        let key = RuntimeKey {
            agent_id: agent_id.to_owned(),
            session_id: session_id.to_owned(),
        };
        let mut entries = self.entries.lock_tolerant();
        entries.retain(|_, entry| {
            entry.active || now.duration_since(entry.last_touched) < IDLE_TIMEOUT
        });

        if let Some(entry) = entries.get_mut(&key) {
            if entry.active {
                return Err(ArcanaError::Agent(format!(
                    "agent '{agent_id}' is already running in session '{session_id}'"
                )));
            }
            entry.active = true;
            entry.last_touched = now;
        } else {
            if entries.len() >= MAX_RUNTIMES {
                return Err(ArcanaError::Agent(format!(
                    "native runtime limit of {MAX_RUNTIMES} reached"
                )));
            }
            entries.insert(
                key.clone(),
                RuntimeEntry {
                    active: true,
                    last_touched: now,
                },
            );
        }
        Ok(RuntimeLease {
            registry: Arc::clone(self),
            key,
            released: false,
        })
    }

    /// Reap idle ownership; persisted sessions and active runs are untouched.
    pub fn reap_idle(&self) -> usize {
        self.reap_at(Instant::now())
    }

    fn reap_at(&self, now: Instant) -> usize {
        let mut entries = self.entries.lock_tolerant();
        let before = entries.len();
        entries.retain(|_, entry| {
            entry.active || now.duration_since(entry.last_touched) < IDLE_TIMEOUT
        });
        before - entries.len()
    }

    pub fn stats(&self) -> (usize, usize) {
        let entries = self.entries.lock_tolerant();
        let active = entries.values().filter(|entry| entry.active).count();
        (active, entries.len() - active)
    }
}

impl RuntimeLease {
    fn release_at(&mut self, now: Instant) {
        if self.released {
            return;
        }
        if let Some(entry) = self.registry.entries.lock_tolerant().get_mut(&self.key) {
            entry.active = false;
            entry.last_touched = now;
        }
        self.released = true;
    }
}

impl Drop for RuntimeLease {
    fn drop(&mut self) {
        self.release_at(Instant::now());
    }
}

static GLOBAL: OnceLock<Arc<RuntimeRegistry>> = OnceLock::new();

pub fn global() -> Arc<RuntimeRegistry> {
    Arc::clone(GLOBAL.get_or_init(RuntimeRegistry::new))
}

/// Start the periodic idle reaper on the host's existing Tokio runtime.
pub fn spawn_reaper() -> tokio::task::JoinHandle<()> {
    let registry = global();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(CLEANUP_INTERVAL);
        loop {
            interval.tick().await;
            registry.reap_idle();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_are_isolated_and_an_active_pair_cannot_run_twice() {
        let registry = RuntimeRegistry::new();
        let now = Instant::now();
        let mut first = registry.admit_at("worker-a", "shared", now).unwrap();
        let second = registry.admit_at("worker-b", "shared", now).unwrap();
        assert!(registry.admit_at("worker-a", "shared", now).is_err());
        assert_eq!(registry.stats(), (2, 0));
        first.release_at(now);
        assert_eq!(registry.stats(), (1, 1));
        assert_eq!(registry.reap_at(now + IDLE_TIMEOUT), 1);
        assert_eq!(registry.stats(), (1, 0));
        drop(second);
    }

    #[test]
    fn cap_is_enforced_and_idle_reap_opens_admission() {
        let registry = RuntimeRegistry::new();
        let now = Instant::now();
        let mut leases: Vec<_> = (0..MAX_RUNTIMES)
            .map(|i| registry.admit_at("worker", &i.to_string(), now).unwrap())
            .collect();
        assert!(registry.admit_at("worker", "overflow", now).is_err());
        leases[0].release_at(now);
        assert_eq!(registry.reap_at(now + IDLE_TIMEOUT), 1);
        let _new = registry
            .admit_at("worker", "overflow", now + IDLE_TIMEOUT)
            .unwrap();
        assert_eq!(registry.stats().0, MAX_RUNTIMES);
    }

    #[test]
    fn unwinding_releases_activity() {
        let registry = RuntimeRegistry::new();
        let probe = Arc::clone(&registry);
        let _ = std::panic::catch_unwind(move || {
            let _lease = probe.admit("worker", "session").unwrap();
            panic!("simulated agent panic");
        });
        assert_eq!(registry.stats(), (0, 1));
    }
}
