// path_jail.rs — Workspace-relative path validation for write/edit/append/bash tools.
//
// Without this, an agent that emits a relative path like
// `ARCANA_PROGRESS.md` or `server.js` resolves it against
// `std::env::current_dir()` (= `src-tauri/` during `npm run tauri dev`), and
// the tool writes the file there instead of in the configured workspace.
// That actually happened on 14 May 2026 — see docs/endtoendrun/14may2026/07.
//
// Contract:
//   `validate_workspace_path(target, workspace)` canonicalizes both inputs and
//   returns Ok(canonical_target) ONLY when the canonical target is inside the
//   canonical workspace. Otherwise it returns an Err with a self-correcting
//   message the LLM can read and retry against.
//
// This is a Rust-side load-bearing safety net. It must remain in place even
// after the Python MCPBridge starts forwarding `__working_dir` correctly,
// because a misbehaving model can still emit absolute paths outside the
// workspace.

use std::path::{Path, PathBuf};

/// Canonicalize a workspace path. The workspace MUST exist on disk for the
/// jail check to be meaningful — if it doesn't exist there is no jail to
/// enforce, and we surface that as an error so the caller doesn't silently
/// degrade into a no-op.
fn canonicalize_workspace(workspace: &Path) -> Result<PathBuf, String> {
    workspace.canonicalize().map_err(|e| {
        format!(
            "Workspace path '{}' could not be canonicalized: {e}. \
         This is a configuration problem in Arcana — the worker has no valid workspace.",
            workspace.display()
        )
    })
}

/// Canonicalize a *target* path, including the case where the target file
/// does not yet exist (e.g. `write_file` creating a fresh file). Walks up
/// from the target until it finds an existing ancestor, canonicalizes that,
/// then re-joins the non-existent tail. Returns the same form used by
/// `canonicalize_workspace` so the two can be compared via `starts_with`.
fn canonicalize_target(target: &Path) -> Result<PathBuf, String> {
    if target.exists() {
        return target.canonicalize().map_err(|e| {
            format!(
                "Path '{}' exists but could not be canonicalized: {e}",
                target.display()
            )
        });
    }

    // ── P3-15 · a path that cannot be normalised is REFUSED ──────────────
    //
    // `..` inside a non-existent tail used to be **silently deleted and the
    // path rewritten**, and the rewritten path was returned as `Ok` — so
    // `write_file.rs` used it as the write destination. Measured:
    // `validate_workspace_path(<ws>/newdir/sub/../x.txt)` returned
    // `Ok(<ws>/newdir/sub/x.txt)`, a different file, accepted.
    //
    // The mechanism is subtle and worth naming: `Path::file_name()` returns
    // **`None`** for a `..` component, so the `if let Some(name)` below skipped
    // it and the rejoin simply lost it. Nothing failed; the path just changed.
    //
    // Corruption is worse than rejection because rejection is visible. A
    // refused call tells the model to retry with a real path; a rewritten one
    // writes to somewhere nobody asked for.
    //
    // The existing part of the path is canonicalised by the OS, which resolves
    // `..` correctly. Only the **non-existent tail** is unresolvable, and there
    // a `..` has no meaning we can honour.
    for component in target.components() {
        if matches!(component, std::path::Component::ParentDir) {
            // Only a problem when it falls in the part we cannot canonicalise.
            // If the whole path exists, the branch above already returned.
            if !target.exists() {
                return Err(format!(
                    "Path '{}' contains '..' in a part that does not exist yet, so it \
                     cannot be resolved. This call has been refused rather than rewritten \
                     — retry with a path that has no '..' segments.",
                    target.display()
                ));
            }
        }
    }

    // Walk up to the first existing ancestor. Track the tail components we
    // skipped so we can rejoin them after canonicalizing the existing part.
    let mut tail: Vec<&std::ffi::OsStr> = Vec::new();
    let mut cursor: &Path = target;
    let canonical_existing = loop {
        match cursor.parent() {
            Some(parent) => {
                match cursor.file_name() {
                    Some(name) => tail.push(name),
                    // `file_name()` is `None` for `..` and for a root. Either
                    // way the component cannot be reproduced, and skipping it
                    // is what silently rewrote the path. Refuse.
                    None => {
                        return Err(format!(
                            "Path '{}' has a component that cannot be resolved. Refused \
                             rather than rewritten.",
                            target.display()
                        ))
                    }
                }
                if parent.exists() {
                    break parent.canonicalize().map_err(|e| {
                        format!(
                            "Ancestor '{}' of target '{}' could not be canonicalized: {e}",
                            parent.display(),
                            target.display()
                        )
                    })?;
                }
                cursor = parent;
            }
            None => {
                // Reached filesystem root without finding an existing ancestor.
                // This only happens when target is on a drive that doesn't exist.
                return Err(format!(
                    "Path '{}' has no existing ancestor on this filesystem — \
                     cannot validate against workspace.",
                    target.display()
                ));
            }
        }
    };
    // Re-join the tail components in reverse (we pushed leaf-first).
    let mut canonical = canonical_existing;
    for component in tail.iter().rev() {
        canonical.push(component);
    }
    Ok(canonical)
}

/// Strip the Windows `\\?\` verbatim-path prefix that `canonicalize()`
/// returns on Windows. Without this, a workspace canonicalized to
/// `\\?\C:\AIRS Foundry\...` would fail to match a target without the
/// prefix even though they refer to the same location.
fn strip_verbatim_prefix(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();

    // ⚠ P3-15, second corruption. `\\?\UNC\server\share\…` is a UNC path in
    // verbatim form. Stripping the `\\?\` blindly yields `UNC\server\share\…`
    // — a **relative** path — and that was returned as the validated write
    // target. A relative path resolves against the process CWD, so a network
    // share became a write somewhere else entirely.
    //
    // The verbatim prefix is only safe to strip when what follows is a drive
    // path (`\\?\C:\…`). For the UNC form the correct de-verbatim spelling is
    // `\\server\share\…`, so the `UNC\` is replaced rather than dropped.
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        p.to_path_buf()
    }
}

/// On Windows, NTFS paths are case-insensitive; the user might write
/// `C:\AIRS Foundry\...` in their config and Rust's canonicalize returns
/// `c:\airs foundry\...` (or vice-versa depending on disk metadata).
/// Compare lowercased components on Windows; exact-match elsewhere.
fn paths_match_prefix(target: &Path, workspace: &Path) -> bool {
    #[cfg(windows)]
    {
        let t = target.to_string_lossy().to_lowercase();
        let w = workspace.to_string_lossy().to_lowercase();
        // Trailing-separator tolerance — `starts_with` on strings can be wrong
        // when one has trailing `\` and the other doesn't, so normalize.
        let w = w.trim_end_matches(['/', '\\']).to_string();
        let t_prefix = if t.len() >= w.len() {
            &t[..w.len()]
        } else {
            return false;
        };
        if t_prefix != w {
            return false;
        }
        // Next char must be a separator OR end-of-string — otherwise the
        // workspace prefix `C:\foo` would falsely accept `C:\foobar`.
        match t.as_bytes().get(w.len()) {
            None => true,
            Some(&b'\\') | Some(&b'/') => true,
            _ => false,
        }
    }
    #[cfg(not(windows))]
    {
        // Use Path::starts_with which compares by component, not by string,
        // so `/foo` does not falsely match `/foobar`.
        target.starts_with(workspace)
    }
}

/// Validate that `target` resolves to a path inside `workspace`. Returns the
/// canonical target on success; an error message safe to surface to the LLM
/// on failure.
///
/// The error message is deliberately structured so the agent can self-correct:
/// it names both paths, says explicitly that the call was refused, and tells
/// the agent what shape of path WOULD have worked.
pub fn validate_workspace_path(target: &Path, workspace: &str) -> Result<PathBuf, String> {
    let workspace_path = Path::new(workspace);
    let canonical_workspace = strip_verbatim_prefix(&canonicalize_workspace(workspace_path)?);
    let canonical_target = strip_verbatim_prefix(&canonicalize_target(target)?);

    if paths_match_prefix(&canonical_target, &canonical_workspace) {
        Ok(canonical_target)
    } else {
        Err(format!(
            "Path '{}' is outside the workspace '{}'. \
             Use a workspace-relative path (e.g. 'src/foo.ts' or 'package.json') \
             or an absolute path under '{}'. \
             This call has been refused — retry with a path inside the workspace.",
            canonical_target.display(),
            canonical_workspace.display(),
            canonical_workspace.display(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn workspace_fixture() -> (TempDir, String) {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().to_string_lossy().to_string();
        (dir, path)
    }

    #[test]
    fn accepts_relative_path_inside_workspace() {
        let (dir, ws) = workspace_fixture();
        let target = dir.path().join("src/foo.ts");
        let result = validate_workspace_path(&target, &ws);
        assert!(result.is_ok(), "got: {:?}", result);
    }

    #[test]
    fn accepts_absolute_path_inside_workspace() {
        let (dir, ws) = workspace_fixture();
        fs::create_dir_all(dir.path().join("nested")).unwrap();
        let target = dir.path().join("nested").join("file.txt");
        let result = validate_workspace_path(&target, &ws);
        assert!(result.is_ok(), "got: {:?}", result);
    }

    #[test]
    fn accepts_existing_file_inside_workspace() {
        let (dir, ws) = workspace_fixture();
        let target = dir.path().join("existing.txt");
        fs::write(&target, b"hi").unwrap();
        let result = validate_workspace_path(&target, &ws);
        assert!(result.is_ok(), "got: {:?}", result);
    }

    #[test]
    fn rejects_absolute_path_outside_workspace() {
        let (_dir, ws) = workspace_fixture();
        let outside = std::env::temp_dir().join("arcana-jail-test-outside-XYZ.txt");
        let result = validate_workspace_path(&outside, &ws);
        // The outside file may or may not exist; either way it must be rejected
        // because it's not inside the workspace tempdir.
        assert!(result.is_err(), "expected rejection, got: {:?}", result);
        let err = result.unwrap_err();
        assert!(err.contains("outside the workspace"), "msg: {err}");
        assert!(err.contains("refused"), "msg: {err}");
    }

    #[test]
    fn rejects_parent_dir_escape() {
        let (dir, ws) = workspace_fixture();
        // dir.path() = /tmp/xyz. Target = /tmp/xyz/../../../etc/passwd
        let target = dir.path().join("..").join("..").join("escape.txt");
        let result = validate_workspace_path(&target, &ws);
        assert!(result.is_err(), "expected rejection, got: {:?}", result);
    }

    #[test]
    fn rejects_workspace_sibling_with_shared_prefix() {
        // /tmp/foo workspace must NOT accept /tmp/foobar/file.txt.
        let parent = TempDir::new().expect("tempdir");
        let ws_dir = parent.path().join("foo");
        let sibling = parent.path().join("foobar");
        fs::create_dir_all(&ws_dir).unwrap();
        fs::create_dir_all(&sibling).unwrap();
        let target = sibling.join("file.txt");
        let result = validate_workspace_path(&target, &ws_dir.to_string_lossy());
        assert!(
            result.is_err(),
            "shared-prefix sibling must be rejected, got: {:?}",
            result
        );
    }

    #[test]
    fn rejects_when_workspace_does_not_exist() {
        let target = std::env::temp_dir().join("anything.txt");
        let bogus_workspace = std::env::temp_dir()
            .join("arcana-nonexistent-workspace-ABCDEF")
            .to_string_lossy()
            .to_string();
        let result = validate_workspace_path(&target, &bogus_workspace);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod p3_15_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn ws() -> (TempDir, String) {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().to_string_lossy().to_string();
        (dir, path)
    }

    #[test]
    fn a_dotdot_in_an_unresolvable_tail_is_refused_not_rewritten() {
        // 🔴 The measured defect. This returned
        // `Ok(<ws>/newdir/sub/x.txt)` — a DIFFERENT path — and `write_file`
        // used that return value as its write destination.
        //
        // The mechanism: `Path::file_name()` yields `None` for `..`, so the
        // tail-rejoin loop skipped it and the path silently changed.
        let (dir, workspace) = ws();
        let target = dir
            .path()
            .join("newdir")
            .join("sub")
            .join("..")
            .join("x.txt");

        let result = validate_workspace_path(&target, &workspace);

        assert!(
            result.is_err(),
            "an unresolvable `..` must be refused, not rewritten — got {result:?}"
        );
        let msg = result.unwrap_err();
        assert!(
            msg.contains("refused") || msg.contains("Refused"),
            "the refusal must say so: {msg}"
        );
    }

    #[test]
    fn a_dotdot_that_the_os_can_resolve_is_still_accepted() {
        // ⚠ The no-fire half (`TG3-28`): a jail that refuses everything passes
        // a refusal-only test and breaks the product. When the whole path
        // exists, the OS canonicalises `..` correctly and the call is fine.
        let (dir, workspace) = ws();
        fs::create_dir_all(dir.path().join("a/b")).unwrap();
        fs::write(dir.path().join("a/target.txt"), "x").unwrap();

        let target = dir.path().join("a").join("b").join("..").join("target.txt");
        let result = validate_workspace_path(&target, &workspace);

        assert!(
            result.is_ok(),
            "a resolvable `..` inside the workspace must still be accepted — got {result:?}"
        );
    }

    #[test]
    fn ordinary_paths_are_unaffected() {
        // The rest of the no-fire corpus: every legitimate shape must still
        // pass, or the fix has broken the product to close the hole.
        let (dir, workspace) = ws();
        fs::create_dir_all(dir.path().join("src/components")).unwrap();

        for rel in [
            "file.txt",
            "src/main.rs",
            "src/components/App.tsx",
            "deep/new/dir/f.ts",
        ] {
            let target = dir.path().join(rel);
            assert!(
                validate_workspace_path(&target, &workspace).is_ok(),
                "{rel} must be accepted"
            );
        }
    }

    #[test]
    fn a_verbatim_unc_path_does_not_become_relative() {
        // The second corruption. A verbatim UNC path had its prefix stripped
        // blindly, leaving a path that begins with the literal text "UNC" -- a
        // RELATIVE path -- and that was returned as the validated write target,
        // so it resolved against the process CWD instead of the share.
        //
        // Built by concatenation rather than as a backslash-heavy literal: the
        // first version of this test failed because the LITERAL was mangled in
        // transit while the code under test was correct. Concatenation removes
        // that ambiguity.
        let bs = std::path::MAIN_SEPARATOR;
        let input = format!("{bs}{bs}?{bs}UNC{bs}server{bs}share{bs}dir{bs}f.txt");
        let out = strip_verbatim_prefix(std::path::Path::new(&input));
        let text = out.to_string_lossy().to_string();

        assert!(
            text.starts_with(&format!("{bs}{bs}")),
            "a de-verbatimised UNC path must keep its network root, got {text}"
        );
        assert!(
            !text.starts_with("UNC"),
            "the UNC marker must become the network root, not be dropped: {text}"
        );
        assert!(text.contains("server"), "the server must survive: {text}");
    }

    #[test]
    fn a_verbatim_drive_path_still_strips_cleanly() {
        // The no-fire half: the common Windows case must be unchanged, or every
        // path on Windows stops matching its own workspace.
        let bs = std::path::MAIN_SEPARATOR;
        let input = format!("{bs}{bs}?{bs}C:{bs}work{bs}repo{bs}src{bs}main.rs");
        let out = strip_verbatim_prefix(std::path::Path::new(&input));

        assert_eq!(
            out.to_string_lossy(),
            format!("C:{bs}work{bs}repo{bs}src{bs}main.rs")
        );
    }
}
