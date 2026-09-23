// One canonical key for one path — used by three subsystems (P3-16, C-7).
//
// ── The defect, in three places ──────────────────────────────────────────────
//
// A path used as a **map key** must have exactly one spelling, or two names for
// one file become two entries. Three subsystems each rolled their own and each
// got it wrong the same way:
//
//   * `path_lock` — `lock_key` normalises only `.` and `..`. It never
//     canonicalises and never case-folds, so `C:\a\b.rs` and `c:\a\b.rs` take
//     **two different mutexes for one file**, on Windows *and* macOS.
//   * the graph DB path key (spec `05 §2.2`)
//   * checkpoint retention (`G3b GI-28`)
//
// ── The live race this fixes ─────────────────────────────────────────────────
//
// Worse than case: the three write tools locked **different forms of the path**.
// `write_file` locked the **pre-jail** path; `edit_file` and `append_file`
// locked the **post-jail canonical** one. Different strings, different slots —
// so the lost-update race that `edit_file`'s own comment documents was live
// between `write_file` and `edit_file` today.
//
// `write_file` even carried a comment asserting *"lock_key normalises both forms
// to the same slot"*. It does not: lexical normalisation cannot resolve symlinks
// or case, which is exactly what distinguishes the two forms.
//
// ⚠ So changing the key alone would not have fixed it. The callers had to agree
// on **which path** they hand in, and they now all hand in the jailed one.

use std::path::{Path, PathBuf};

/// Whether this platform's filesystem is case-insensitive.
///
/// Windows and macOS are (by default); Linux is not. Folding case on Linux
/// would merge `Foo.rs` and `foo.rs`, which are genuinely two files there — so
/// this is a platform question, not a preference.
pub const fn platform_is_case_insensitive() -> bool {
    cfg!(any(target_os = "windows", target_os = "macos"))
}

/// The canonical map key for `path`.
///
/// Resolves symlinks and `..` where the path exists, strips the Windows
/// verbatim prefix, and case-folds where the platform is case-insensitive.
///
/// ⚠ **Falls back to lexical normalisation for a path that does not exist yet**
/// — a fresh-file write is the common case, and refusing to produce a key there
/// would mean no lock at all for exactly the operation that most needs one.
pub fn path_key(path: &Path) -> String {
    let resolved = path
        .canonicalize()
        .map(|c| strip_verbatim(&c))
        .unwrap_or_else(|_| lexical(path));

    let text = resolved.to_string_lossy().to_string();

    if platform_is_case_insensitive() {
        text.to_lowercase()
    } else {
        text
    }
}

/// Strip Windows' `\\?\` verbatim prefix, preserving a UNC root.
///
/// ⚠ Stripping it blindly from `\\?\UNC\server\share\…` yields a **relative**
/// path — the same corruption `P3-15` fixed in the jail. Kept consistent here
/// so the key and the jail agree about what a path is.
fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        return PathBuf::from(stripped);
    }
    p.to_path_buf()
}

/// Resolve `.` and `..` textually, for a path that does not exist yet.
fn lexical(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn two_spellings_of_one_file_take_one_key() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("File.rs"), "x").unwrap();

        let a = path_key(&dir.path().join("File.rs"));
        let b = path_key(&dir.path().join("./File.rs"));
        assert_eq!(a, b, "`./` must not create a second key");
    }

    #[test]
    fn case_folds_only_where_the_platform_does() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("File.rs"), "x").unwrap();

        let upper = path_key(&dir.path().join("File.rs"));
        let lower = path_key(&dir.path().join("file.rs"));

        if platform_is_case_insensitive() {
            assert_eq!(
                upper, lower,
                "on a case-insensitive filesystem these are ONE file and must take one \
                 mutex — two keys is the lost-update race"
            );
        } else {
            // On Linux they are genuinely two files, and folding would merge
            // two distinct locks into one — a correctness bug in the other
            // direction.
            assert_ne!(upper, lower);
        }
    }

    #[test]
    fn a_dotdot_resolves_to_the_same_key_as_the_direct_path() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("a/b")).unwrap();
        fs::write(dir.path().join("a/target.rs"), "x").unwrap();

        let direct = path_key(&dir.path().join("a/target.rs"));
        let indirect = path_key(&dir.path().join("a/b/../target.rs"));
        assert_eq!(direct, indirect);
    }

    #[test]
    fn a_file_that_does_not_exist_yet_still_gets_a_stable_key() {
        // The fresh-file write is the common case and the one that most needs
        // a lock. Refusing to key it would mean no serialisation at all there.
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("not-created-yet.rs");

        let a = path_key(&target);
        let b = path_key(&target);
        assert_eq!(a, b, "the key must be stable across calls");
        assert!(!a.is_empty());
    }

    #[test]
    fn lexical_resolution_handles_dotdot_without_the_filesystem() {
        let out = lexical(Path::new("/a/b/../c/./d.rs"));
        assert_eq!(out, PathBuf::from("/a/c/d.rs"));
    }

    #[test]
    fn a_verbatim_unc_key_keeps_its_root() {
        // Consistent with the jail's fix: stripping the prefix blindly would
        // make this relative, and a relative key collides with anything of the
        // same name under a different root.
        let bs = std::path::MAIN_SEPARATOR;
        let input = format!("{bs}{bs}?{bs}UNC{bs}server{bs}share{bs}f.rs");
        let out = strip_verbatim(Path::new(&input));
        let text = out.to_string_lossy();
        assert!(!text.starts_with("UNC"), "got {text}");
    }
}
