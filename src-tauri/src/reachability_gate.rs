// Reachability gate — every .rs file in the workspace must be reachable from
// its own crate's root.
//
// Since P1 the application spans four crates (`arcana-core`, `src-tauri`,
// `src-cli`, `src-shared`), so this gate walks all four. It lives in
// `src-tauri` only because that is where it was written; it is a workspace-wide
// check and covers every crate equally.
//
// This exists because of 918 lines that were never compiled. `graph_tools.rs`
// (840 LOC, 27 tool structs — the plan said 22) and `graph_prompts.rs` (78 LOC)
// had no `mod` declaration anywhere, so `cargo build` never saw them — and
// neither did clippy, or any test. `db/checkpoints.rs` was in the same state.
//
// ✅ RESOLVED IN P1. Both graph modules are declared and compile; the six
// E0609s they surfaced are fixed. `db/checkpoints.rs` was deleted rather than
// declared. The exception list below is now EMPTY.
//
// ⚠ CORRECTED 2026-09-01. This comment used to end "...which is why undo does
// not exist in the shipped binary". That is FALSE, and it contradicted this
// file's own KNOWN_UNREACHABLE entry twenty lines below, which correctly calls
// `git/checkpoint.rs` "the live, shipped" implementation. Undo ships: declared
// at `git/mod.rs:21`, five Tauri commands registered (`lib.rs:386-390`), called
// from `src/stores/gitStore.ts:238`, and a snapshot is taken automatically
// before every agent write (`edit_file.rs:177`, `write_file.rs:124`).
//
// What IS true, and is a separate defect: `auto_checkpoint_before_write`
// returns `()` and logs a failure at `debug!` level only
// (`git/checkpoint.rs:256-260`), so a checkpoint that fails is invisible and
// the overwrite proceeds regardless.
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
///
/// **It is empty, and that is the goal state.** P1 resolved all three entries:
/// `graph_tools.rs` and `graph_prompts.rs` are declared and compiling (P1-01,
/// P1-03), and `db/checkpoints.rs` was deleted rather than declared, because
/// its justification — "undo does not exist" — was false and declaring it would
/// have created the second checkpoint store P1-16 then had to merge
/// (ADR-LOG-RESTRUCT-0012).
///
/// ⚠ **Compiling is not wiring.** The 27 structs in `graph_tools.rs` still do
/// not implement `Tool` and nothing registers them, so the agent has no graph
/// tools. This gate proves the code is *built*, never that a capability
/// *exists* — that is P3 (BUG-GRAPH). A green gate here must not be read as a
/// working feature.
const KNOWN_UNREACHABLE: &[(&str, &str)] = &[];

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

/// Every crate in the workspace, with the smallest file count that is
/// plausible for it.
///
/// The floors exist because this gate is only as wide as the tree it walks.
/// Before P1 it walked `src-tauri/src` alone and asserted `> 100` files — which
/// was true then and became false the moment P1 moved 182 files into
/// `arcana-core`. Had the floor simply been lowered, the gate would have gone
/// green while silently protecting 84 files instead of 280
/// (ADR-LOG-RESTRUCT-0015). A per-crate floor makes that failure loud: moving a
/// crate's contents elsewhere trips its own floor rather than shrinking the
/// gate's reach unnoticed.
fn crate_roots() -> Vec<(&'static str, PathBuf, usize)> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri always has a workspace parent")
        .to_path_buf();
    vec![
        ("arcana-core", workspace.join("arcana-core/src"), 150),
        ("src-tauri", workspace.join("src-tauri/src"), 50),
        ("src-cli", workspace.join("src-cli/src"), 8),
        ("src-shared", workspace.join("src-shared/src"), 5),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_source_file_is_reachable_from_its_crate_root() {
        let known: HashSet<&str> = KNOWN_UNREACHABLE.iter().map(|(p, _)| *p).collect();
        let mut orphans = Vec::new();
        let mut walked = 0usize;

        for (krate, src, floor) in crate_roots() {
            assert!(
                src.is_dir(),
                "{krate}: expected a source tree at {}",
                src.display()
            );
            let files = all_rust_files(&src);
            assert!(
                files.len() >= floor,
                "{krate}: walked only {} files, below the floor of {floor}. Either the \
                 tree moved — in which case update crate_roots() so the gate follows \
                 it — or this gate is now protecting far less than it claims.",
                files.len()
            );
            walked += files.len();

            // Declarations are collected per crate: a `mod` in `src-tauri` does
            // not make a file in `arcana-core` reachable.
            let declared = declared_module_names(&files);

            for file in &files {
                let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                // Crate roots and directory modules are reachable by
                // construction: `mod.rs` is named by its parent directory,
                // checked via that directory's own name.
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
                orphans.push(format!("{krate}/src/{rel}"));
            }
        }

        assert!(
            orphans.is_empty(),
            "these .rs files are never compiled — no `mod` declaration names them, \
             so cargo, clippy and every test are blind to them:\n  {}\n\n\
             Declare the module, delete the file, or add it to KNOWN_UNREACHABLE \
             with a reason and an owning phase.",
            orphans.join("\n  ")
        );

        // A floor per crate still permits the whole workspace shrinking toward
        // its floors, so the total is asserted too.
        assert!(
            walked > 250,
            "walked only {walked} files across the workspace; this gate has lost reach"
        );
    }

    /// The exception list is a debt register, so it must not quietly accumulate
    /// entries for files that no longer exist — or shrink by deletion instead of
    /// by repair.
    #[test]
    fn every_known_unreachable_file_still_exists() {
        for (rel, reason) in KNOWN_UNREACHABLE {
            let found = crate_roots()
                .iter()
                .any(|(_, src, _)| src.join(rel).exists());
            assert!(
                found,
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
