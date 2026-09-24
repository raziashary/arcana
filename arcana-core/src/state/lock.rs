// Poison-tolerant locking (P3-03).
//
// ── The defect ───────────────────────────────────────────────────────────────
//
// `P3-03` said *"a panicking worker takes down the app"*. It does not: every
// agent path runs inside `tokio::spawn`, and unwinding is on. What actually
// happens is worse to diagnose and just as fatal in practice.
//
// When a thread panics while holding a `std::sync::Mutex`, the mutex is
// **poisoned**, and every subsequent `.lock().unwrap()` on it panics too. This
// tree had **98 `.lock().unwrap()` sites against 5 poison-tolerant** ones. So a
// single panic anywhere in a turn converts into a cascade of unrelated panics at
// every later lock of the same state — a permanent, restart-only failure that
// presents as something else entirely, several layers away from its cause.
//
// `catch_unwind` at the turn boundary (`P3-02`) contains the *first* panic. It
// does nothing about the poison the panic already left behind. Both are needed,
// and neither is sufficient alone.
//
// ── Why ignoring poison is the right call HERE, and where it would not be ────
//
// Poisoning is not noise. It means *"a panic happened partway through a mutation,
// so this data may violate its own invariants"*, and discarding that signal
// blindly is how corruption propagates. That objection is real and this module
// does not wave it away.
//
// It does not apply to what these mutexes actually hold. `AppState` is counters,
// flags, an active-model string and a list of running sessions — **fields with
// no cross-field invariant**. A panic mid-update leaves one of them stale, and
// the next write corrects it. Weighed against the alternative — the application
// becoming permanently unusable until restarted, with every later lock panicking
// and the original cause long gone — a possibly-stale counter is plainly the
// lesser harm.
//
// ⚠ **Where this would be the wrong tool:** a structure with an invariant
// spanning several fields (an index that must agree with the collection it
// indexes, a balance split across two accounts). There, the correct response to
// poison is to rebuild or to fail loudly — not to read through it. If such a
// structure is ever added, it must not use this trait, and the reason should be
// written at its definition.

use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Lock without propagating poison.
pub trait LockTolerant<T> {
    /// Acquire the lock, recovering the guard if a previous holder panicked.
    ///
    /// Prefer this to `.lock().unwrap()` everywhere in this codebase. `TG-P3-03`
    /// enforces it.
    fn lock_tolerant(&self) -> MutexGuard<'_, T>;
}

impl<T> LockTolerant<T> for Mutex<T> {
    fn lock_tolerant(&self) -> MutexGuard<'_, T> {
        match self.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                // Logged, not silent. The poison is a real event — a panic
                // happened while this lock was held — and swallowing it without
                // a trace would make the *original* failure unfindable, which is
                // the problem this module exists to solve.
                tracing::warn!(
                    "recovered a poisoned lock: a previous holder panicked. The panic \
                     itself was reported where it happened; this is the after-effect."
                );
                poisoned.into_inner()
            }
        }
    }
}

/// The same for `RwLock`, which poisons on write-panic exactly as `Mutex` does.
pub trait RwLockTolerant<T> {
    fn read_tolerant(&self) -> RwLockReadGuard<'_, T>;
    fn write_tolerant(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T> RwLockTolerant<T> for RwLock<T> {
    fn read_tolerant(&self) -> RwLockReadGuard<'_, T> {
        match self.read() {
            Ok(g) => g,
            Err(poisoned) => {
                tracing::warn!("recovered a poisoned RwLock (read)");
                poisoned.into_inner()
            }
        }
    }

    fn write_tolerant(&self) -> RwLockWriteGuard<'_, T> {
        match self.write() {
            Ok(g) => g,
            Err(poisoned) => {
                tracing::warn!("recovered a poisoned RwLock (write)");
                poisoned.into_inner()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn a_poisoned_mutex_is_still_usable() {
        // The whole point, expressed as the scenario that actually happens: a
        // turn panics while holding the state lock, and every later turn must
        // still be able to read it.
        let state = Arc::new(Mutex::new(41));

        let poisoner = Arc::clone(&state);
        let _ = std::thread::spawn(move || {
            let mut g = poisoner.lock().unwrap();
            *g = 42;
            panic!("a tool panicked mid-update");
        })
        .join();

        assert!(
            state.lock().is_err(),
            "the mutex must genuinely be poisoned"
        );

        // The tolerant path recovers, and sees the write the panicking thread
        // completed before it died.
        let g = state.lock_tolerant();
        assert_eq!(*g, 42);
    }

    #[test]
    fn an_unpoisoned_mutex_behaves_identically() {
        let m = Mutex::new(vec![1, 2, 3]);
        {
            let mut g = m.lock_tolerant();
            g.push(4);
        }
        assert_eq!(*m.lock_tolerant(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn a_poisoned_rwlock_is_still_readable_and_writable() {
        let lock = Arc::new(RwLock::new(String::from("before")));

        let poisoner = Arc::clone(&lock);
        let _ = std::thread::spawn(move || {
            let mut g = poisoner.write().unwrap();
            g.push_str("-during");
            panic!("panic under a write lock");
        })
        .join();

        assert!(
            lock.read().is_err(),
            "the rwlock must genuinely be poisoned"
        );
        assert_eq!(&*lock.read_tolerant(), "before-during");

        lock.write_tolerant().push_str("-after");
        assert_eq!(&*lock.read_tolerant(), "before-during-after");
    }

    #[test]
    fn the_cascade_this_prevents_is_real() {
        // Demonstrates the failure mode in the small: with `.lock().unwrap()`,
        // one panic makes every subsequent access panic — which is what turns a
        // single failed turn into a restart-only application.
        let state = Arc::new(Mutex::new(0u32));

        let poisoner = Arc::clone(&state);
        let _ = std::thread::spawn(move || {
            let _g = poisoner.lock().unwrap();
            panic!("first failure");
        })
        .join();

        // Three "later turns". Under the old pattern all three die.
        let mut would_have_panicked = 0;
        for _ in 0..3 {
            if state.lock().is_err() {
                would_have_panicked += 1;
            }
            // The tolerant path survives all three.
            let mut g = state.lock_tolerant();
            *g += 1;
        }

        assert_eq!(would_have_panicked, 3, "each later access hits the poison");
        assert_eq!(*state.lock_tolerant(), 3, "and each still did its work");
    }
}
