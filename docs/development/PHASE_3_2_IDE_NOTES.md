# Phase 3.2 — IDE Core Implementation Notes

Companion document to `DEVELOPMENT_PLAN.md` §Phase 3.2. Captures the
architectural decisions that were made during implementation and the deltas
from the original plan so future work has a single place to read before
touching the IDE tab.

## Module map

### Backend — `src-tauri/src/ide/`

| File              | Responsibility                                                    |
|-------------------|--------------------------------------------------------------------|
| `mod.rs`          | Module declaration only.                                           |
| `types.rs`        | DTOs shared with the frontend (FileTreeNode, FileContent, Symbol…) |
| `file_tree.rs`    | Recursive tree walk + git status overlay (git2).                    |
| `file_reader.rs`  | Encoding detection + pagination + language id + metadata.          |
| `file_search.rs`  | Fuzzy name search + grep-like content search.                      |
| `symbols.rs`      | Regex-based symbol extractor (Rust/TS/JS/Python/Go).               |
| `workspace.rs`    | Open-tabs / expanded-dirs persistence in `config.db`.              |

### Backend — `src-tauri/src/commands/ide_commands.rs`

Thin Tauri wrappers. All commands pull the active workspace from
`workspace_commands::load_workspace_dir` and error out when unset.

### Frontend — `src/components/ide/`

| Component             | Responsibility                                        |
|-----------------------|--------------------------------------------------------|
| `FileTree.tsx`        | Folder browser with git badges.                       |
| `TabBar.tsx`          | Tabs with close + middle-click.                       |
| `CodeViewer.tsx`      | Monaco viewer, read-only, custom Winter-is-Coming theme.|
| `FileSearch.tsx`      | Ctrl+P quick-open with keyboard nav.                  |
| `SymbolOutline.tsx`   | Click-to-jump symbol list.                            |
| `IdeStatusBar.tsx`    | Bottom strip — path, lang, size, encoding.            |

### Frontend — `src/stores/ideStore.ts`

Zustand store. Persists `openTabs`, `activeTab`, `expandedDirs` to the
backend via `set_ide_workspace_state`. Content for each tab is kept in
memory only (Monaco is read-only, so there is nothing to persist
beyond the path list).

## Deviations from the original plan

### 1. Symbol extraction: regex-first, tree-sitter later

The plan called for tree-sitter-based symbol extraction. Tree-sitter
requires per-language grammar compilation and bumps binary size materially.
To keep Phase 3.2 shippable we implemented a regex-based extractor for
Rust, TypeScript, JavaScript, Python, and Go. The Tauri API (`SymbolTree`,
`Symbol`, `SymbolKind`) is grammar-agnostic, so swapping the internals for
tree-sitter in **Phase 3.2.1** is isolated to `symbols.rs`.

**What works today:** functions, classes, structs, enums, interfaces,
traits, modules, constants, types.
**What requires tree-sitter:** `resolve_symbol` (go-to-definition) and
`find_symbol_references` (usage search). These two commands are
intentionally unregistered until that upgrade lands.

### 2. `.gitignore` parsing deferred to Phase 3.3

The file tree uses a built-in skip list of noisy directories
(`.git`, `node_modules`, `target`, …) plus a `ignore_patterns` parameter
supplied by the caller. True `.gitignore` parsing will ship alongside
the git work in Phase 3.3, which is where the `git2::Repository::statuses`
options already need revisiting to cover ignored files properly.

### 3. Inline editing: intentionally not supported

Monaco is wired in read-only mode. AutoDev mutates files exclusively
through agent tools (`write_file`, `edit_file`). Allowing the UI to edit
files would split ownership and introduce stale-buffer bugs; the
"Modified" tab indicator is therefore N/A for this phase. The checklist
flags these as deferred "if inline editing ships" rather than treating
them as debt.

### 4. Content menus and DnD deferred

Tree context menus (Copy Path, Delete, Rename, New File/Folder) and
drag-and-drop moves need `mv`/`rm`-equivalent commands in Rust that
coexist safely with git worktrees. These are polish items queued
behind Phase 3.3 so they can reuse the checkpointing guarantees landing
there.

## Integration seams

- **Chat → IDE jump.** Shipped as part of Phase 3.2 (follow-up delivery).
  - Single helper: `src/utils/openInIde.ts` — `openInIde(path, gotoLine?)`
    calls `useIdeStore.getState().openFile(path, gotoLine)` and then
    `useUiStore.getState().setActiveTab('ide')`.
  - Active top-level tab is now sourced from `src/stores/uiStore.ts`
    (promoted out of `App.tsx` local state) so any component can
    navigate between tabs without prop-drilling.
  - Wired into: `FileContentDisplay` (header path + explicit
    "Open in IDE" button), `GrepResultsDisplay` (per-match line jump +
    file-header icon), `GlobResultsDisplay` (files only), and
    `DirectoryListingDisplay` (files only). Directories remain inert
    until directory-navigation UX is designed.
  - The Monaco viewer honours `gotoLine` via
    `editor.revealLineInCenter` on mount and on subsequent jumps.
- **Persistence.** The single source of truth is `config.db`
  (`app_settings` table, keys: `ide_open_tabs`, `ide_active_tab`,
  `ide_expanded_dirs`). No separate DB or file \- reuses the same store
  that already holds `workspace_dir`.
- **Git status.** All git calls go through `git2::Repository::discover`
  starting at the workspace root, so nested repos inside the workspace
  are not re-discovered per file. The index is built once per tree
  request.

## Testing strategy

Manual verification path (matches the Success Criteria in the plan):

1. Set a workspace folder in Settings.
2. Open the IDE tab → file tree loads with git badges.
3. Click a file → Monaco opens it with syntax highlighting.
4. Press **Ctrl/⌘+P** → quick-open overlay appears; arrow keys move
   selection; Enter opens the file.
5. Click a symbol in Outline → Monaco scrolls and centres on the
   declaration line.
6. Close the app; reopen → the previously-open tabs and active tab
   are restored; the expanded directories re-expand.

Automated tests are intentionally deferred until tree-sitter arrives in
3.2.1; the regex extractor is easy to unit-test at that point.

## Known follow-ups

1. **Phase 3.2.1** — swap `symbols.rs` to tree-sitter; register
   `resolve_symbol` + `find_symbol_references`.
2. **Phase 3.2 polish** — extension-specific file icons, breadcrumb
   navigation, context menu, tab drag-reorder.
3. **Phase 3.3** — `.gitignore` parsing in file tree, git sub-panel,
   checkpointing.
