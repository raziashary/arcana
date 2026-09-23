// path_lock.rs — Process-wide per-file async mutex registry.
//
// Purpose
// -------
// Prevent two callers — the assistant and a fleet worker, two workers, or
// two subagents inside the same parent agent — from writing the same
// file at the same time. The fleet's task-board atomicity guarantees only
// one worker owns a *task*; this registry guarantees only one writer is
// inside a *file's* read-modify-write cycle.
//
// Design
// ------
// One async `tokio::sync::Mutex` per resolved absolute path. Acquired by
// `acquire(path)` and held in the caller's scope; released on drop. The
// registry itself is guarded by a synchronous `std::sync::Mutex` because
// it only does HashMap inserts (fast, contention is negligible).
//
// Key normalisation: paths are lexically normalised (`.` and `..` stripped)
// against an already-absolutised key, *not* `Path::canonicalize`d, because
// the file may not exist yet — `write_file` is the canonical "create a
// fresh file" entry point. Two callers passing equivalent paths
// (`workspace/src/x.rs` vs an absolute path that resolves to the same
// location) hash to the same slot.
//
// Lifetime
// --------
// Entries are not GC'd. One entry per path the fleet has ever touched.
// For any realistic project (thousands of files) this is well below the
// cost of taking a global lock to garbage-collect. Switch to a `Weak<>`
// pattern with a periodic sweep if memory ever becomes a concern.
//
// Tools that should call this
// ---------------------------
// * `write_file`   — full overwrite (or refuse-on-exists, with the new
//                    overwrite flag).
// * `edit_file`    — read-modify-write; the lock spans the read AND the
//                    write so a concurrent writer can't slip in between.
// * `append_file`  — append; lock prevents a concurrent overwrite from
//                    truncating mid-append.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};
use crate::state::lock::LockTolerant;

type Registry = StdMutex<HashMap<PathBuf, Arc<AsyncMutex<()>>>>;

fn registry() -> &'static Registry {
    static REG: OnceLock<Registry> = OnceLock::new();
    REG.get_or_init(|| StdMutex::new(HashMap::new()))
}

/// Lexically normalise a path: strip `CurDir`, collapse `ParentDir`. Does
/// NOT touch the filesystem — safe for not-yet-existing paths.
fn lexically_normalise(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => continue,
            Component::ParentDir => {
                out.pop();
            }
            _ => out.push(c.as_os_str()),
        }
    }
    out
}

/// Compute the lock key for a given target path.
///
/// * Absolute paths are normalised in place.
/// * Relative paths are joined onto `working_dir` first, then normalised.
///
/// Returns the canonical lock key. Two callers passing equivalent paths
/// resolve to the same key.
pub fn lock_key(target: &Path, working_dir: &str) -> PathBuf {
    let joined: PathBuf = if target.is_absolute() {
        target.to_path_buf()
    } else {
        Path::new(working_dir).join(target)
    };

    // ⚠ P3-16 / C-7. This was `lexically_normalise`, which resolves `.` and
    // `..` and **nothing else** — no canonicalisation, no case-folding. So
    // `C:.rs` and `c:.rs` took two mutexes for one file, on Windows
    // and macOS. The same missing normalisation exists in the graph DB key and
    // checkpoint retention: three subsystems, one helper, fixed once.
    PathBuf::from(crate::security::path_key::path_key(&joined))
}

/// Acquire the per-path mutex for `key`. Awaits if another caller currently
/// holds it. The returned guard releases the lock on drop.
///
/// The mutex registry itself is a synchronous `std::sync::Mutex` over a
/// HashMap; we hold it only long enough to look up / insert the per-path
/// Arc, then release before awaiting the path-level mutex. That keeps
/// registry contention to microseconds even under heavy fleet write load.
pub async fn acquire_owned(key: &Path) -> OwnedMutexGuard<()> {
    let lock: Arc<AsyncMutex<()>> = {
        let mut reg = registry().lock_tolerant();
        if let Some(existing) = reg.get(key) {
            existing.clone()
        } else {
            let fresh = Arc::new(AsyncMutex::new(()));
            reg.insert(key.to_path_buf(), fresh.clone());
            fresh
        }
    };
    lock.lock_owned().await
}

/// Convenience wrapper: compute the key from (target, working_dir) and
/// acquire the lock in one call.
pub async fn acquire_for(target: &Path, working_dir: &str) -> OwnedMutexGuard<()> {
    let key = lock_key(target, working_dir);
    acquire_owned(&key).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equivalent_paths_hash_to_same_key() {
        let workspace = "/work";
        let absolute = Path::new("/work/src/x.rs");
        let relative = Path::new("src/x.rs");
        let with_dots = Path::new("./src/./x.rs");

        let k1 = lock_key(absolute, workspace);
        let k2 = lock_key(relative, workspace);
        let k3 = lock_key(with_dots, workspace);

        assert_eq!(k1, k2);
        assert_eq!(k2, k3);
    }

    #[test]
    fn parent_dir_collapses() {
        let workspace = "/work";
        let with_parent = Path::new("src/foo/../x.rs");
        let canonical = Path::new("src/x.rs");
        assert_eq!(
            lock_key(with_parent, workspace),
            lock_key(canonical, workspace)
        );
    }

    #[tokio::test]
    async fn second_acquire_waits_for_first() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;

        let key = Path::new("/tmp/path-lock-test-file.rs");
        let counter = Arc::new(AtomicUsize::new(0));

        let c1 = counter.clone();
        let h1 = tokio::spawn(async move {
            let _g = acquire_owned(Path::new("/tmp/path-lock-test-file.rs")).await;
            // Hold the lock for a measurable interval; observer should see
            // counter stay at 1 until we release.
            c1.store(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(50)).await;
            c1.store(2, Ordering::SeqCst);
        });

        // Give h1 time to grab the lock.
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "h1 should be holding the lock"
        );

        // h2 tries to acquire; should block until h1 finishes.
        let _g2 = acquire_owned(key).await;
        // When we get here, h1 has released, so counter must be 2.
        assert_eq!(
            counter.load(Ordering::SeqCst),
            2,
            "h2 must wait for h1 to release"
        );

        h1.await.unwrap();
    }
}

#[cfg(test)]
mod p3_16_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn all_spellings_of_one_file_take_one_lock_slot() {
        // 🔴 The defect: `lock_key` normalised only `.` and `..`, so two
        // spellings of one file took two mutexes — and two mutexes is no
        // mutual exclusion at all.
        let dir = TempDir::new().unwrap();
        let ws = dir.path().to_string_lossy().to_string();
        fs::write(dir.path().join("File.rs"), "x").unwrap();

        let direct = lock_key(&dir.path().join("File.rs"), &ws);
        let dotted = lock_key(&dir.path().join("./File.rs"), &ws);
        let relative = lock_key(std::path::Path::new("File.rs"), &ws);

        assert_eq!(direct, dotted, "`./` produced a second lock slot");
        assert_eq!(direct, relative, "a relative path produced a second lock slot");
    }

    #[test]
    fn case_variants_share_a_slot_where_the_platform_shares_the_file() {
        let dir = TempDir::new().unwrap();
        let ws = dir.path().to_string_lossy().to_string();
        fs::write(dir.path().join("File.rs"), "x").unwrap();

        let upper = lock_key(&dir.path().join("File.rs"), &ws);
        let lower = lock_key(&dir.path().join("file.rs"), &ws);

        if crate::security::path_key::platform_is_case_insensitive() {
            assert_eq!(
                upper, lower,
                "on this platform these are ONE file; two lock slots is the lost-update race"
            );
        } else {
            // The no-fire half: on Linux they are two files, and merging their
            // locks would be a correctness bug in the other direction.
            assert_ne!(upper, lower);
        }
    }

    #[test]
    fn the_pre_jail_and_post_jail_forms_agree() {
        // The race that was actually live: `write_file` locked the pre-jail
        // path and `edit_file` the post-jail canonical one. Both forms must
        // land in the same slot, or the two tools do not exclude each other.
        let dir = TempDir::new().unwrap();
        let ws = dir.path().to_string_lossy().to_string();
        fs::write(dir.path().join("shared.rs"), "x").unwrap();

        let pre_jail = dir.path().join("shared.rs");
        let post_jail =
            crate::agent::builtin_tools::path_jail::validate_workspace_path(&pre_jail, &ws)
                .expect("inside the workspace");

        assert_eq!(
            lock_key(&pre_jail, &ws),
            lock_key(&post_jail, &ws),
            "write_file and edit_file must contend on the same slot"
        );
    }
}
