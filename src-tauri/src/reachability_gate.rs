// Reachability gate — every .rs file under src/ must be reachable from the
// crate root.
//
// This exists because of 918 lines that were never compiled. `graph_tools.rs`
// (840 LOC, 22 tool structs) and `graph_prompts.rs` (78 LOC) have no `mod`
// declaration anywhere, so `cargo build` never saw them — and neither did
// clippy, or any test. `db/checkpoints.rs` is in the same state, which is why
// undo does not exist in the shipped binary.
//
// The project's own plan records this as done: `docs/development/
// DEVELOPMENT_PLAN.md` §6.12 checks off the literal instruction
// "Add `pub mod graph_tools;`" and declares the section COMPLETE. It never
// happened. No gate existed that could tell the difference between a module
// that ships and a file that merely exists, so the plan and the binary
// disagreed for as long as anyone cared to look.
//
// Compiled only under `cfg(test)`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Files known to be unreachable, each with the reason and the phase that fixes
/// it. **This list may only shrink.** Adding to it requires a deliberate edit
/// and a reviewer asking why a new file cannot be reached.
const KNOWN_UNREACHABLE: &[(&str, &str)] = &[
    (
        "agent/builtin_tools/graph_tools.rs",
        "840 LOC, 22 tool structs. Does not currently compile: `:272` treats an \
         Option<GraphNode> as a GraphNode and `:696` reads a non-existent \
         `confidence_tier`. Declaring it also would not restore the capability — \
         the structs do not implement `Tool` and nothing registers them. Repaired \
         and wired in P3 (BUG-GRAPH).",
    ),
    (
        "agent/builtin_tools/graph_prompts.rs",
        "78 LOC. Compiles cleanly; withheld only so it lands with graph_tools in \
         P3 rather than shipping prompts for tools that do not exist.",
    ),
    (
        "db/checkpoints.rs",
        "Duplicates the live, shipped `git/checkpoint.rs` (declared at \
         `git/mod.rs:31`, wired to five Tauri commands). Declaring it would stand \
         up a SECOND checkpoint implementation against a second database \
         (~/.autodev/checkpoints.db, which no compiled code opens). Needs an \
         owner decision on which implementation survives — P1.",
    ),
];

/// Walk `src/`, returning every `.rs` file relative to it.
fn all_rust_files(src: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(src, &mut out);
    out.sort();
    out
}

/// Collect every module name declared anywhere in the crate.
///
/// A name-set rather than a true graph walk: it cannot prove a declared module
/// is itself reachable, but it catches the failure that actually happened — a
/// file nobody declared at all. `main.rs` and `lib.rs` are roots by definition.
fn declared_module_names(files: &[PathBuf]) -> HashSet<String> {
    let mut names = HashSet::new();
    for file in files {
        let Ok(source) = std::fs::read_to_string(file) else {
            continue;
        };
        for line in source.lines() {
            let line = line.trim();
            if line.starts_with("//") {
                continue;
            }
            // `mod foo;` / `pub mod foo;` / `pub(crate) mod foo;` — declaration
            // only, not `mod foo {` which is an inline module.
            let Some(rest) = line
                .split(" mod ")
                .nth(1)
                .or_else(|| line.strip_prefix("mod "))
            else {
                continue;
            };
            if let Some(name) = rest.strip_suffix(';') {
                let name = name.trim();
                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    names.insert(name.to_string());
                }
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_source_file_is_reachable_from_the_crate_root() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let files = all_rust_files(&src);
        assert!(
            files.len() > 100,
            "expected to walk the crate; found {}",
            files.len()
        );

        let declared = declared_module_names(&files);
        let known: HashSet<&str> = KNOWN_UNREACHABLE.iter().map(|(p, _)| *p).collect();

        let mut orphans = Vec::new();
        for file in &files {
            let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            // Crate roots and directory modules are reachable by construction:
            // `mod.rs` is named by its parent directory, checked via that
            // directory's own name.
            if matches!(stem, "main" | "lib" | "mod" | "build") {
                continue;
            }
            if declared.contains(stem) {
                continue;
            }

            let rel = file
                .strip_prefix(&src)
                .unwrap_or(file)
                .to_string_lossy()
                .replace('\\', "/");
            if known.contains(rel.as_str()) {
                continue;
            }
            orphans.push(rel);
        }

        assert!(
            orphans.is_empty(),
            "these .rs files are never compiled — no `mod` declaration names them, \
             so cargo, clippy and every test are blind to them:\n  {}\n\n\
             Declare the module, delete the file, or add it to KNOWN_UNREACHABLE \
             with a reason and an owning phase.",
            orphans.join("\n  ")
        );
    }

    /// The exception list is a debt register, so it must not quietly accumulate
    /// entries for files that no longer exist — or shrink by deletion instead of
    /// by repair.
    #[test]
    fn every_known_unreachable_file_still_exists() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for (rel, reason) in KNOWN_UNREACHABLE {
            let path = src.join(rel);
            assert!(
                path.exists(),
                "KNOWN_UNREACHABLE lists {rel}, which no longer exists. If it was \
                 declared or deleted, remove the entry.\n  recorded reason: {reason}"
            );
            assert!(
                reason.len() > 40,
                "{rel} needs a real reason and an owning phase, not a placeholder"
            );
        }
    }
}
