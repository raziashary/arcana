//! Shared helpers for tests that must redirect the data root.
//!
//! `HOME` and `USERPROFILE` are **process-global**, and `cargo test` runs the
//! unit tests of a crate as threads in one process. So any two tests that swap
//! them race: whichever sets them last wins, and the other reads a sandbox it
//! did not create.
//!
//! That is not hypothetical here. `db/migrate.rs` gained two tests in the same
//! change — one proving a destructive migration takes a backup, one proving it
//! refuses when the backup fails — and the second creates a *file* where the
//! first expects a *directory*. Run in parallel they failed with
//! `Cannot create a file when that file already exists`, and the failure looked
//! like a bug in the code under test rather than in the harness.
//!
//! A per-module mutex does not fix it: `legacy_root.rs` already had one, and it
//! did not serialise against `migrate.rs`. The lock has to be crate-wide, which
//! is why it lives here.

use std::sync::{Mutex, MutexGuard, OnceLock};

fn lock() -> &'static Mutex<()> {
    static HOME_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    HOME_LOCK.get_or_init(|| Mutex::new(()))
}

/// Holds the data root redirected for as long as it is alive, and restores the
/// previous values on drop — including on a panic, which a manual
/// set/restore pair does not.
pub struct HomeGuard {
    _guard: MutexGuard<'static, ()>,
    previous_user: Option<String>,
    previous_home: Option<String>,
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        restore("USERPROFILE", self.previous_user.take());
        restore("HOME", self.previous_home.take());
    }
}

fn restore(key: &str, value: Option<String>) {
    match value {
        Some(v) => std::env::set_var(key, v),
        None => std::env::remove_var(key),
    }
}

/// Point `~` at `dir` for the lifetime of the returned guard.
///
/// Blocks until any other test holding the redirect has finished, so two tests
/// never see each other's sandbox. A poisoned lock is recovered rather than
/// propagated: a panic in one test must not cascade into every other test that
/// needs the data root.
pub fn with_home(dir: &std::path::Path) -> HomeGuard {
    let guard = lock().lock().unwrap_or_else(|e| e.into_inner());
    let previous_user = std::env::var("USERPROFILE").ok();
    let previous_home = std::env::var("HOME").ok();
    std::env::set_var("USERPROFILE", dir);
    std::env::set_var("HOME", dir);
    HomeGuard {
        _guard: guard,
        previous_user,
        previous_home,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_redirect_is_restored_on_drop() {
        let dir =
            std::env::temp_dir().join(format!("arcana-home-guard-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).expect("create unique isolated directory");
        {
            let _g = with_home(&dir);
            assert_eq!(std::env::var("HOME").ok().as_deref(), dir.to_str());
        }

        // Asserted as "no longer the sandbox" rather than "equal to the value
        // sampled before". Reading that earlier value would have to happen
        // *outside* the lock, where another test may hold the redirect — so the
        // stricter-looking assertion is the racy one, and it failed exactly
        // that way when first written.
        assert_ne!(
            std::env::var("HOME").ok().as_deref(),
            dir.to_str(),
            "the redirect outlived its guard"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The property the crate-wide lock exists for: a second acquisition waits
    /// rather than trampling the first.
    #[test]
    fn two_redirects_do_not_overlap() {
        let a = std::env::temp_dir().join(format!("arcana-home-guard-a-{}", uuid::Uuid::new_v4()));
        let b = std::env::temp_dir().join(format!("arcana-home-guard-b-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&a).expect("create unique isolated directory a");
        std::fs::create_dir(&b).expect("create unique isolated directory b");

        let g = with_home(&a);
        assert_eq!(std::env::var("HOME").ok().as_deref(), a.to_str());
        drop(g);

        let g = with_home(&b);
        assert_eq!(std::env::var("HOME").ok().as_deref(), b.to_str());
        drop(g);

        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }
}
