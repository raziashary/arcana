use async_trait::async_trait;
use serde_json::json;
use std::path::Path;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

pub struct ListDirectoryTool;

#[async_trait]
impl Tool for ListDirectoryTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Read
    }

    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List files and subdirectories at a path. Returns structured JSON with name, path, is_directory, size_bytes."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type":        "string",
                    "description": "Directory path to list. Defaults to working directory."
                },
                "recursive": {
                    "type":        "boolean",
                    "description": "If true, list recursively (max depth 3). Default: false."
                },
                "max_entries": {
                    "type":        "integer",
                    "description": "Maximum number of entries to return. Default: 500. Use a lower value on large repos."
                }
            },
            "required": []
        })
    }


    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let path_str = input["path"].as_str().unwrap_or(&ctx.working_dir);
        let recursive = input["recursive"].as_bool().unwrap_or(false);
        let max_entries = input["max_entries"].as_u64().unwrap_or(500) as usize;

        let raw = if Path::new(path_str).is_absolute() {
            Path::new(path_str).to_path_buf()
        } else {
            Path::new(&ctx.working_dir).join(path_str)
        };
        // ⚠ P3-14. This tool did not call the jail and was auto-approved.
        // Reading outside the workspace is an **exfiltration primitive**, not a
        // harmless read: the model names a path, the contents enter its
        // context, and nothing asked the user. Six tools bypassed the jail;
        // two of them acted on the path rather than merely reading it.
        let base = match super::path_jail::validate_workspace_path(&raw, &ctx.working_dir) {
            Ok(canonical) => canonical,
            Err(e) => return ToolResult::error(e),
        };

        if !base.exists() {
            return ToolResult::error(format!("Path does not exist: {}", base.display()));
        }
        if !base.is_dir() {
            return ToolResult::error(format!("Not a directory: {}", base.display()));
        }

        let mut entries: Vec<serde_json::Value> = Vec::new();
        collect_entries(
            &base,
            &base,
            0,
            if recursive { 3 } else { 1 },
            max_entries,
            &mut entries,
        );

        // ⚠ This was `== max_entries`, which is `false` **exactly when the
        // cap was overshot** — so the one signal that would have revealed the
        // bug above reported clean at 100,000 entries. `>=` is exact now that
        // the loop cannot overshoot, and it stays honest if it ever can again.
        let truncated = entries.len() >= max_entries;
        let output = json!({
            "path":      base.display().to_string(),
            "entries":   entries,
            "truncated": truncated,
        });

        if truncated {
            ToolResult::success_with_hint(
                output.to_string(),
                format!(
                    "Output capped at {} entries. Use a more specific path, set recursive=false, \
                     or lower max_entries to focus on a subdirectory.",
                    max_entries
                ),
            )
        } else {
            ToolResult::success(output.to_string())
        }
    }
}

// Skip common noise directories
const SKIP: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".venv",
    "venv",
    "dist",
    "build",
    ".airs",
    ".arcana",
];

fn collect_entries(
    root: &Path,
    path: &Path,
    depth: usize,
    max_depth: usize,
    max_entries: usize,
    out: &mut Vec<serde_json::Value>,
) {
    if depth >= max_depth {
        return;
    }
    if out.len() >= max_entries {
        return;
    }

    let Ok(read) = std::fs::read_dir(path) else {
        return;
    };

    let mut dir_entries: Vec<_> = read.filter_map(|e| e.ok()).collect();
    dir_entries.sort_by_key(|e| {
        let is_dir = e.path().is_dir();
        (!is_dir, e.file_name())
    });

    for entry in dir_entries {
        // 🔴 P3-29. The cap was checked ONLY at function entry, never here —
        // so a single directory pushed every one of its children regardless.
        // With `recursive=false` (`max_depth = 1`) there is exactly one call
        // and one loop, which made the cap **fully inert in the common case**.
        if out.len() >= max_entries {
            return;
        }

        let name = entry.file_name();
        let name_str = name.to_string_lossy().to_string();
        if SKIP.iter().any(|s| *s == name_str) {
            continue;
        }

        let entry_path = entry.path();
        let is_dir = entry_path.is_dir();
        let size_bytes: Option<u64> = if is_dir {
            None
        } else {
            entry.metadata().ok().map(|m| m.len())
        };

        // Path relative to root for display
        let rel = entry_path
            .strip_prefix(root)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| entry_path.display().to_string());

        let mut obj = json!({
            "name":         name_str,
            "path":         rel,
            "is_directory": is_dir,
        });

        if let Some(sz) = size_bytes {
            obj["size_bytes"] = json!(sz);
        }

        out.push(obj);

        if is_dir {
            // 🔴 T-27 — do not follow a link out of the jail.
            //
            // `read_dir` resolves a symlinked directory transparently, so a
            // link inside the workspace listed whatever it pointed at —
            // anywhere on the disk — and reported it under a workspace-relative
            // path, which concealed where the content came from. The jail was
            // applied to the ARGUMENT and never to what the walk reached.
            //
            // ⚠ Canonicalise and compare, rather than testing `is_symlink`:
            // Windows junctions report `false` for that, and a `..`
            // component would escape without any link at all.
            let inside = std::fs::canonicalize(&entry_path)
                .ok()
                .zip(std::fs::canonicalize(root).ok())
                .map(|(child, base)| child.starts_with(&base))
                .unwrap_or(false);
            if !inside {
                // The entry itself was already listed above, so the escape is
                // recorded by NOT descending. Annotating the pushed object
                // would mean holding it past the push; naming it in the
                // listing is `T-27`'s follow-up if a caller needs the signal.
                continue;
            }
            collect_entries(root, &entry_path, depth + 1, max_depth, max_entries, out);
        }
    }
}

#[cfg(test)]
mod p3_29_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn a_flat_directory_larger_than_the_cap_is_actually_capped() {
        // 🔴 THE DEFECT. `collect_entries` checked `out.len() >= max_entries`
        // only at FUNCTION ENTRY, never in the push loop. With
        // `recursive=false` (`max_depth = 1`) there is exactly one call and one
        // loop — so the cap was fully inert in the common case, and a directory
        // with 100,000 files put all 100,000 into the model's context.
        let dir = TempDir::new().unwrap();
        for i in 0..250 {
            std::fs::write(dir.path().join(format!("f{i:04}.txt")), "x").unwrap();
        }

        let mut out = Vec::new();
        collect_entries(dir.path(), dir.path(), 0, 1, 50, &mut out);

        assert_eq!(
            out.len(),
            50,
            "the cap did not hold: {} entries returned for a cap of 50",
            out.len()
        );
    }

    #[test]
    fn the_truncation_signal_fires_when_the_cap_is_reached() {
        // ⚠ The old signal was `entries.len() == max_entries`, which is FALSE
        // exactly when the cap was overshot — so the one thing that would have
        // revealed the bug above reported clean at 100,000 entries.
        let dir = TempDir::new().unwrap();
        for i in 0..60 {
            std::fs::write(dir.path().join(format!("f{i:04}.txt")), "x").unwrap();
        }

        let mut out = Vec::new();
        collect_entries(dir.path(), dir.path(), 0, 1, 25, &mut out);

        assert!(
            out.len() >= 25,
            "expected the cap to be reached, got {}",
            out.len()
        );
    }

    #[test]
    fn a_directory_under_the_cap_is_returned_whole() {
        // The no-fire half: capping the common small listing would be a
        // regression of its own.
        let dir = TempDir::new().unwrap();
        for i in 0..7 {
            std::fs::write(dir.path().join(format!("f{i}.txt")), "x").unwrap();
        }

        let mut out = Vec::new();
        collect_entries(dir.path(), dir.path(), 0, 1, 100, &mut out);

        assert_eq!(out.len(), 7);
    }
}
