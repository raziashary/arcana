// Doc 07 Phase 3 — stray-file scan.
//
// On host startup, walk `src-tauri/` (and `src/`) for files NOT owned by
// the Arcana repo itself. The 14 May calendar run left three stray files
// in `src-tauri/`:
//   • ARCANA_PROGRESS.md  (worker progress log)
//   • server.js            (Express backend for the calendar project)
//   • src/components/AddEventModal.tsx (React component for the calendar)
// All three were the worker-frontend recipe writing files with a leaked
// working_dir = `src-tauri/`. The path-jail (Phase 1) plus the
// tool-adapter fallback (Phase 2) now block this at write time, but
// pre-existing strays from before those landed will still be on disk.
//
// The scan emits a structured warning per stray with a copy-paste delete
// command and forwards a `workspace:strays-detected` Tauri event so the
// SidecarCrashBanner / a dedicated startup-warnings panel can surface the
// list inline.
//
// Heuristics: files NOT matching the Arcana repo's tracked-file pattern.
// We deliberately keep this list short and conservative — false negatives
// (a stray we miss) are cheaper than false positives (warning on a real
// repo file).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tracing::{info, warn};

/// One observed stray file. Forwarded to the frontend via the
/// `workspace:strays-detected` event so the UI can render a dismissible
/// banner with "move to workspace" / "delete" affordances per file.
#[derive(Debug, Clone, Serialize)]
pub struct StrayFile {
    pub absolute_path: String,
    pub relative_to_repo: String,
    pub size_bytes: u64,
    pub reason: &'static str,
}

/// Scan known-problem subdirectories of the Arcana repo for stray
/// worker-generated files. Returns the list of strays found; emits a WARN
/// per file with a self-correcting "rm" hint.
///
/// `repo_root` should be the Arcana repo's top-level directory (the one
/// that contains both `src/` and `src-tauri/`). Pass None to default to
/// `current_dir()` — convenient at startup before any path config is
/// loaded.
pub fn scan(repo_root: Option<&Path>) -> Vec<StrayFile> {
    let root = match repo_root {
        Some(p) => p.to_path_buf(),
        None => match std::env::current_dir() {
            Ok(p) => p,
            Err(e) => {
                warn!("[stray_files] cannot determine current_dir: {e} — skipping scan");
                return Vec::new();
            }
        },
    };

    let mut found: Vec<StrayFile> = Vec::new();
    let known_filenames = stray_filename_blocklist();

    // Subdirectories to walk. `src/` and `src-tauri/` are the only ones a
    // worker would conceivably write into when the workspace fallback
    // degrades. `node_modules/` and `target/` are intentionally NOT walked
    // — they contain millions of files and zero workers ever write there.
    let subdirs = ["src-tauri", "src"];

    for sub in subdirs {
        let dir = root.join(sub);
        if !dir.is_dir() {
            continue;
        }
        scan_recursive(&dir, &root, &known_filenames, &mut found, 0);
    }

    if !found.is_empty() {
        warn!(
            "[stray_files] {} stray file(s) detected in src-tauri/ or src/ — \
             likely written by a previous worker run with a leaked working_dir. \
             Review and delete before the next E2E test. See \
             docs/endtoendrun/14may2026/07-workspace-escape-rootcause.md §4 \
             for evidence of past occurrences.",
            found.len()
        );
        for s in &found {
            warn!(
                "[stray_files]   {} ({} bytes) — reason: {}",
                s.absolute_path, s.size_bytes, s.reason
            );
        }
    } else {
        info!("[stray_files] no stray files detected");
    }

    found
}

fn scan_recursive(
    dir: &Path,
    root: &Path,
    blocklist: &HashSet<&'static str>,
    found: &mut Vec<StrayFile>,
    depth: u32,
) {
    // Cap depth so a runaway directory tree can't lock up startup.
    if depth > 4 {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        // Skip canonical Arcana folders / hidden dirs.
        if matches!(
            name,
            "target" | "node_modules" | ".git" | "dist" | "build" | ".cache" | ".turbo" | "out"
        ) {
            continue;
        }

        if path.is_dir() {
            scan_recursive(&path, root, blocklist, found, depth + 1);
            continue;
        }
        if !path.is_file() {
            continue;
        }

        // Match against the blocklist by filename. Conservative: only flag
        // files whose names match patterns workers historically produced.
        if blocklist.contains(name) {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            found.push(StrayFile {
                absolute_path: path.to_string_lossy().to_string(),
                relative_to_repo: rel,
                size_bytes: size,
                reason: "matches known worker-output filename",
            });
            continue;
        }

        // .tsx components dropped into src-tauri are always strays — the
        // Tauri host has no React tree.
        let in_src_tauri = path.to_string_lossy().contains("src-tauri");
        if in_src_tauri && (name.ends_with(".tsx") || name.ends_with(".jsx")) {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            found.push(StrayFile {
                absolute_path: path.to_string_lossy().to_string(),
                relative_to_repo: rel,
                size_bytes: size,
                reason: ".tsx/.jsx inside src-tauri/ — Tauri host has no React tree",
            });
        }
    }
}

/// Filenames historically produced by misbehaving workers. Kept short and
/// specific: a real repo file would not have these names.
fn stray_filename_blocklist() -> HashSet<&'static str> {
    [
        "ARCANA_PROGRESS.md",
        "build.log",
        "smoke.log",
        "build_guard.log",
        "install.log",
        // Calendar run's specific outputs
        "server.js",
    ]
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocklist_contains_known_strays() {
        let bl = stray_filename_blocklist();
        assert!(bl.contains("ARCANA_PROGRESS.md"));
        assert!(bl.contains("server.js"));
        assert!(bl.contains("build.log"));
        // Negatives — must not flag real repo files
        assert!(!bl.contains("Cargo.toml"));
        assert!(!bl.contains("package.json"));
        assert!(!bl.contains("main.rs"));
    }

    #[test]
    fn scan_returns_empty_for_clean_root() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let strays = scan(Some(tmp.path()));
        assert!(strays.is_empty(), "expected no strays in empty dir");
    }
}
