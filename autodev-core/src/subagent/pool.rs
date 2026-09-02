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

/// Doc 04 Phase 3 — sub-cap on concurrent QC reviewer subagents (those
/// spawned by `qc-dispatcher` via `spawn_qc_review`). Sits between the
/// per-parent cap and the global cap: even though `MAX_PER_PARENT = 10`,
/// QC reviewers compete with worker-spawned subagents for the global cap,
/// so we hold QC to 6 to leave headroom under burst load.
pub const MAX_CONCURRENT_QC: usize = 6;

/// Parent slug that triggers the QC sub-cap. Single constant so the
/// dispatcher recipe + tool + reconciler all agree on the canonical name.
pub const QC_DISPATCHER_SLUG: &str = "qc-dispatcher";

/// Token returned by `SubagentPool::acquire`. Releases its slots on drop.
pub struct PoolPermit {
    _global: OwnedSemaphorePermit,
    /// Doc 04 Phase 3 — held only when `parent == QC_DISPATCHER_SLUG`.
    /// Released-on-drop alongside the global permit so the QC sub-cap
    /// behaves identically to the rest of the pool.
    _qc: Option<OwnedSemaphorePermit>,
    pool: Arc<SubagentPool>,
    parent: String,
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
                if *c == 0 {
                    counts.remove(&parent);
                }
            }
        });
    }
}

pub struct SubagentPool {
    global: Arc<Semaphore>,
    /// Doc 04 Phase 3 — dedicated semaphore for QC reviewer subagents.
    /// Acquired BEFORE the global semaphore so the QC cap stays tight under
    /// pressure (rejecting a QC spawn rather than back-pressuring everything).
    qc: Arc<Semaphore>,
    per_parent: Mutex<HashMap<String, usize>>,
}

impl SubagentPool {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            global: Arc::new(Semaphore::new(MAX_GLOBAL_CONCURRENT)),
            qc: Arc::new(Semaphore::new(MAX_CONCURRENT_QC)),
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

        // Doc 04 Phase 3 — QC sub-cap. Acquire BEFORE the global sem so a
        // burst of QC spawns rejects fast instead of starving worker
        // subagents. `try_acquire_owned` is used (not `acquire`) so QC
        // failures do not back-pressure the whole dispatcher tick.
        let qc_permit = if parent_slug == QC_DISPATCHER_SLUG {
            match self.qc.clone().try_acquire_owned() {
                Ok(p) => Some(p),
                Err(_) => {
                    // Roll back the per-parent increment so a rejected
                    // claim doesn't leak the counter.
                    let mut counts = self.per_parent.lock().await;
                    if let Some(c) = counts.get_mut(parent_slug) {
                        *c = c.saturating_sub(1);
                    }
                    return Err(AutoDevError::Agent(format!(
                        "QC reviewer rejected: already at MAX_CONCURRENT_QC={MAX_CONCURRENT_QC} \
                         in-flight reviewers — wait for the next dispatcher tick"
                    )));
                }
            }
        } else {
            None
        };

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
        if parent_slug == QC_DISPATCHER_SLUG && self.qc.available_permits() < 1 {
            warn!("[SubagentPool] approaching QC concurrency limit ({MAX_CONCURRENT_QC})");
        }

        Ok(PoolPermit {
            _global: global_permit,
            _qc: qc_permit,
            pool: self.clone(),
            parent: parent_slug.to_string(),
        })
    }

    /// Doc 04 Phase 3.3 — snapshot of available QC reviewer slots. Used by
    /// the Subagent Monitor UI to render "QC: 3/6 in flight" without
    /// re-walking the registry.
    pub fn qc_available(&self) -> usize {
        self.qc.available_permits()
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

#[cfg(test)]
mod p3_22_tests {
    use super::*;

    /// Releasing a permit must restore global capacity — and holding one past
    /// termination must consume it.
    ///
    /// 🔴 `P3-22`. The manager held each subagent's permit inside its registry
    /// entry for the whole `TERMINAL_RETENTION` window and beyond. Retention
    /// exists so the UI can show a final result; it has nothing to do with
    /// capacity. So 16 **completed** subagents consumed the entire global cap,
    /// and the only thing that would free them — `prune_terminal` — was
    /// reachable from `spawn` **only after** the `acquire` that was waiting on
    /// them, or from `list()`, which only the UI calls. With the subagent
    /// monitor panel closed, the 17th spawn blocked permanently.
    #[tokio::test]
    async fn the_depth_boundary_is_where_it_says_it_is() {
        // P3-58. The three builtin-tool guards rejected at `>= 3` on the
        // PARENT while this pool rejects at `> 3` on the CHILD (= parent + 1).
        // Parent >= 3 fires whenever child > 3 would, and one generation
        // earlier -- so the pool's depth branch was unreachable from any
        // builtin tool: a guard that could not fire.
        //
        // One boundary, asserted on one side. A child exactly AT the limit is
        // allowed; one past it is not.
        let pool = SubagentPool::new();

        assert!(
            pool.acquire("at-the-limit", MAX_NESTING_DEPTH).await.is_ok(),
            "a child at exactly MAX_NESTING_DEPTH={MAX_NESTING_DEPTH} must be              allowed, or the effective limit is one less than the constant says"
        );
        assert!(
            pool.acquire("past-the-limit", MAX_NESTING_DEPTH + 1)
                .await
                .is_err(),
            "a child past MAX_NESTING_DEPTH must be rejected"
        );
    }

    #[tokio::test]
    async fn a_released_permit_restores_capacity() {
        let pool = SubagentPool::new();

        // Fill the global cap. Each spawn uses a distinct parent so the
        // per-parent cap (10) is not what we are measuring.
        let mut held = Vec::new();
        for i in 0..MAX_GLOBAL_CONCURRENT {
            held.push(
                pool.acquire(&format!("parent-{i}"), 0)
                    .await
                    .unwrap_or_else(|e| panic!("permit {i} of {MAX_GLOBAL_CONCURRENT}: {e}")),
            );
        }

        // The next one must wait — this is the state 16 finished-but-unreleased
        // subagents put the pool in.
        let blocked = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            pool.acquire("parent-overflow", 0),
        )
        .await;
        assert!(
            blocked.is_err(),
            "the global cap did not hold at {MAX_GLOBAL_CONCURRENT}; this test \
             cannot detect the deadlock it exists to detect"
        );

        // Terminating one subagent releases its permit — which is what
        // `finalise` now does, and what its docstring always claimed it did.
        drop(held.pop().expect("one held permit"));

        let recovered = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            pool.acquire("parent-after-release", 0),
        )
        .await;
        assert!(
            recovered.is_ok(),
            "capacity was not restored by releasing a permit — a finished \
             subagent still holds its slot, which is the deadlock"
        );
        assert!(recovered.unwrap().is_ok());
    }
}

#[cfg(test)]
mod f21_tests {
    use super::*;

    /// The two predicates the P3-58 "fix" swapped between are the SAME.
    ///
    /// 🔴 F-21 (post-phase claims audit). The task claimed the tool guard
    /// (`parent >= 3`) was "strictly tighter" than the pool's
    /// (`child > 3`) and fired a generation earlier. For `u32`,
    /// `d >= 3` ⟺ `d + 1 > 3` — identical for every input. The premise was
    /// false and my rewrite was arithmetically a no-op that claimed otherwise.
    ///
    /// This test exists so the arithmetic is checked rather than reasoned
    /// about, which is what nobody did the first two times.
    #[test]
    fn the_two_spellings_of_the_depth_boundary_are_one_predicate() {
        for parent in 0u32..8 {
            let tool_side = parent >= MAX_NESTING_DEPTH;
            let pool_side = parent + 1 > MAX_NESTING_DEPTH;
            assert_eq!(
                tool_side, pool_side,
                "at parent depth {parent} the two spellings disagree — if this \
                 ever becomes true, one of them is the real boundary and the \
                 other is a second, silent one"
            );
        }
    }

    #[tokio::test]
    async fn the_pool_is_now_the_only_thing_that_decides_depth() {
        // With the three builtin-tool guards deleted, the pool's own branch is
        // what refuses — reachable at last, and with a message the caller
        // surfaces to the model.
        let pool = SubagentPool::new();

        let text = match pool.acquire("f21", MAX_NESTING_DEPTH + 1).await {
            Err(e) => e.to_string(),
            Ok(_) => panic!("a child past the limit must be refused"),
        };

        assert!(
            text.contains("MAX_NESTING_DEPTH"),
            "the refusal must name the limit so the model can act on it: {text}"
        );
        assert!(
            pool.acquire("f21-ok", MAX_NESTING_DEPTH).await.is_ok(),
            "a child exactly at the limit must still be allowed"
        );
    }
}
