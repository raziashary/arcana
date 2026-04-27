// SubagentPool — enforces resource limits on subagent spawning.
//
// Limits:
//   * MAX_GLOBAL_CONCURRENT — hard cap on simultaneous live subagents
//     across the whole app (prevents one rogue parent from monopolising
//     core-orchestrator's worker pool)
//   * MAX_PER_PARENT — hard cap per parent slug (so a single agent can't
//     explode into hundreds of children)
//   * MAX_NESTING_DEPTH — how deep the parent → child → grandchild chain
//     can recurse
//
// The pool uses a tokio Semaphore for the global cap (giving back-pressure
// instead of immediate rejection) and an in-memory map for the per-parent
// cap (rejected immediately because parents that hit this limit are usually
// misbehaving — back-pressure would mask the bug).

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tracing::warn;

use crate::errors::{AutoDevError, Result};

/// Maximum live subagents across the whole AutoDev process.
pub const MAX_GLOBAL_CONCURRENT: usize = 16;

/// Maximum live subagents per parent slug.
pub const MAX_PER_PARENT: usize = 10;

/// Maximum nesting depth (grandchildren of grandchildren is too much).
pub const MAX_NESTING_DEPTH: u32 = 3;

/// Token returned by `SubagentPool::acquire`. Releases its slots on drop.
pub struct PoolPermit {
    _global: OwnedSemaphorePermit,
    pool:    Arc<SubagentPool>,
    parent:  String,
}

impl Drop for PoolPermit {
    fn drop(&mut self) {
        let pool = self.pool.clone();
        let parent = self.parent.clone();
        // Synchronous decrement — uses the std-Mutex-equivalent via blocking_lock
        // alternative is fine here because the lock is held for microseconds.
        tokio::task::spawn(async move {
            let mut counts = pool.per_parent.lock().await;
            if let Some(c) = counts.get_mut(&parent) {
                *c = c.saturating_sub(1);
                if *c == 0 { counts.remove(&parent); }
            }
        });
    }
}

pub struct SubagentPool {
    global: Arc<Semaphore>,
    per_parent: Mutex<HashMap<String, usize>>,
}

impl SubagentPool {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            global: Arc::new(Semaphore::new(MAX_GLOBAL_CONCURRENT)),
            per_parent: Mutex::new(HashMap::new()),
        })
    }

    /// Acquire a slot in the pool. Blocks (asynchronously) if the global
    /// semaphore is exhausted; rejects immediately on per-parent or depth
    /// limits.
    pub async fn acquire(self: &Arc<Self>, parent_slug: &str, depth: u32) -> Result<PoolPermit> {
        if depth > MAX_NESTING_DEPTH {
            return Err(AutoDevError::Agent(format!(
                "subagent rejected: depth {depth} exceeds MAX_NESTING_DEPTH={MAX_NESTING_DEPTH}"
            )));
        }

        // Per-parent check (fast-fail before queueing on the global sem).
        {
            let mut counts = self.per_parent.lock().await;
            let n = counts.entry(parent_slug.to_string()).or_insert(0);
            if *n >= MAX_PER_PARENT {
                return Err(AutoDevError::Agent(format!(
                    "subagent rejected: parent '{parent_slug}' already has {n} active subagents \
                     (MAX_PER_PARENT={MAX_PER_PARENT})"
                )));
            }
            *n += 1;
        }

        // Global semaphore acquire — back-pressure if too many live overall.
        let global_permit = match self.global.clone().acquire_owned().await {
            Ok(p) => p,
            Err(e) => {
                // Roll back the per-parent increment we just did.
                let mut counts = self.per_parent.lock().await;
                if let Some(c) = counts.get_mut(parent_slug) {
                    *c = c.saturating_sub(1);
                }
                return Err(AutoDevError::Agent(format!(
                    "subagent pool global semaphore closed: {e}"
                )));
            }
        };

        if self.global.available_permits() < 2 {
            warn!("[SubagentPool] approaching global concurrency limit ({MAX_GLOBAL_CONCURRENT})");
        }

        Ok(PoolPermit {
            _global: global_permit,
            pool:    self.clone(),
            parent:  parent_slug.to_string(),
        })
    }

    /// Snapshot of the current per-parent counts. For diagnostics / UI.
    pub async fn snapshot(&self) -> HashMap<String, usize> {
        self.per_parent.lock().await.clone()
    }

    pub fn global_available(&self) -> usize {
        self.global.available_permits()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_excessive_depth() {
        let pool = SubagentPool::new();
        let res = pool.acquire("worker-backend", MAX_NESTING_DEPTH + 1).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn enforces_per_parent_limit() {
        let pool = SubagentPool::new();
        let mut held = Vec::new();
        for _ in 0..MAX_PER_PARENT {
            held.push(pool.acquire("p", 0).await.unwrap());
        }
        let extra = pool.acquire("p", 0).await;
        assert!(extra.is_err(), "should have rejected (MAX_PER_PARENT)");
    }
}
