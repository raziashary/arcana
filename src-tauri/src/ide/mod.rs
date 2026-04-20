// IDE module — file tree, file reading, search, symbol extraction, workspace state.
// Phase 3.2 — backend services for the IDE tab.
//
// Architecture:
//   file_tree.rs    → recursive directory listing + git status overlay
//   file_reader.rs  → encoding-aware file content reading with pagination
//   file_search.rs  → fuzzy file-name search + grep-like content search
//   symbols.rs      → language-aware symbol extraction (functions, classes, …)
//   workspace.rs    → workspace root resolution + open-tab persistence
//   types.rs        → shared DTOs serialized to the frontend
//
// All modules operate within the configured workspace_dir (see workspace_commands.rs).
// Commands are wired in commands/ide_commands.rs and registered in lib.rs.

pub mod types;
pub mod file_tree;
pub mod file_reader;
pub mod file_search;
pub mod symbols;
pub mod workspace;
