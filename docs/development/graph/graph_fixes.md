# Graph Engine — Fix Specification
### AutoDev Phase 6 · Defect Analysis and Atomic Repair Tasks

**Document Status:** Active — pre-implementation  
**Author:** Claude Code (analysis session, May 2026)  
**Scope:** All graph-related files: `src-tauri/src/graph/`, `src-tauri/src/commands/graph_commands.rs`, `src/tabs/Graph.tsx`

---

## Root Cause Map

Before atomic tasks, here is the causal chain explaining what you see on screen:

```
Bug A: EdgeKind serialized as "Calls" (Rust Debug)
  → stored in SQLite as "Calls"
  → sent to frontend as edge.kind = "Calls"
  → EDGE_COLORS["Calls"] = undefined   ← no match (keys are "CALLS")
  → every edge renders #555555 (fallback gray)
  → legend shows correct colors but graph is monochrome

Bug B: 10 of 11 language extractors are file-node stubs
  → JS, Rust, Go, Java, C/C++, Ruby, PHP, Kotlin, Swift, Bash emit ONLY a File node
  → no functions, no classes, no imports, no edges between files
  → graph is 1280 isolated orange squares connected by almost nothing
  → avg degree = 0.85 confirms this (only Python + its imports create real edges)

Bug C: edge.kind mismatch also breaks edge TYPE FILTER
  → edgeFilters["CALLS"] = true, but edge.kind = "Calls"
  → filter check: edgeFilters[e.kind] !== false → edgeFilters["Calls"] → undefined !== false → true
  → paradox: all edges pass the filter even when "disabled" because the key never matches

Bug D: EDGE_WEIGHTS in cluster.rs uses "Calls" (CamelCase)
  → EDGE_WEIGHTS.find(k == &edge.kind) with edge.kind from DB also "Calls"
  → actually matches correctly — clustering is internally consistent
  → BUT community hulls are nearly meaningless because most edges are CONTAINS-only (Python)
```

---

## Fix G-1 · `build.rs` — `format!("{:?}", edge.kind)` produces CamelCase; frontend expects UPPER_SNAKE_CASE

**Status:** ✅ Done — `Display` impls added to `NodeKind`, `EdgeKind`, `ConfidenceTier` in `extract.rs`; `build.rs` lines 62/83/89 changed to `format!("{}", ...)`; `cluster.rs` EDGE_WEIGHTS updated to UPPER_SNAKE_CASE

**Files:**
- `src-tauri/src/graph/build.rs` lines 83, 89
- `src-tauri/src/graph/extract.rs` lines 24–35 (EdgeKind enum definition)

**Finding 1 — Edge kind serialization at line 83 uses Rust `Debug` trait:**

```rust
// build.rs line 82-92
let schema_edges: Vec<SchemaEdgeInfo> = all_edges.into_iter().map(|edge| SchemaEdgeInfo {
    kind: format!("{:?}", edge.kind),   // ← BUG: Debug of EdgeKind::ImportsFrom = "ImportsFrom"
    ...
}).collect();
```

`format!("{:?}", EdgeKind::ImportsFrom)` produces `"ImportsFrom"`. The frontend `EDGE_COLORS` map at `Graph.tsx:16` expects `"IMPORTS_FROM"`. Every single edge kind is affected: `Calls`, `ImportsFrom`, `Inherits`, `Implements`, `Contains`, `TestedBy`, `DependsOn`, `References`, `SemanticallySimilar` — none match any key in `EDGE_COLORS`. Result: every edge renders gray (`#555555` fallback), the legend is cosmetically correct but functionally dead, and arrowhead markers are never applied because `arr-${d.kind.toLowerCase()}` produces `arr-calls` but markers were created as `arr-calls` from the `EDGE_COLORS` key `"CALLS"` → `"calls"` — actually this sub-path works because both lower to the same thing, but stroke color is still wrong.

**Finding 2 — Same pattern on line 89 for confidence tier:**

```rust
confidence_tier: format!("{:?}", edge.confidence_tier),  // "Extracted", "Inferred", "Ambiguous"
```

The spec at section 6.3 uses `"EXTRACTED"`, `"INFERRED"`, `"AMBIGUOUS"`. The DB stores mixed case. No frontend code currently uses confidence tier for rendering, but this will become a bug when opacity-by-confidence is implemented.

**Finding 3 — Node kind has the same issue at line 62:**

```rust
kind: format!("{:?}", node.kind),  // "Function", "Class", "File" etc.
```

`symbolForKind()` in `Graph.tsx:34` uses `.toLowerCase()` which saves it — `"Function".toLowerCase() === "function"` → `.includes('function')` → circle. Node shapes work accidentally. But the sidebar at line ~706 does `node.kind === "Function"` style comparisons — those fail without `.toLowerCase()`. Not exploited yet but is a latent bug.

**Required changes:**

```rust
// Option A (simplest): implement Display on EdgeKind to return UPPER_SNAKE_CASE
impl std::fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            EdgeKind::Calls             => "CALLS",
            EdgeKind::ImportsFrom       => "IMPORTS_FROM",
            EdgeKind::Imports           => "IMPORTS",
            EdgeKind::Inherits          => "INHERITS",
            EdgeKind::Implements        => "IMPLEMENTS",
            EdgeKind::Contains          => "CONTAINS",
            EdgeKind::TestedBy          => "TESTED_BY",
            EdgeKind::DependsOn         => "DEPENDS_ON",
            EdgeKind::References        => "REFERENCES",
            EdgeKind::SemanticallySimilar => "SEMANTICALLY_SIMILAR",
        };
        write!(f, "{}", s)
    }
}

impl std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            NodeKind::File     => "File",
            NodeKind::Class    => "Class",
            NodeKind::Function => "Function",
            NodeKind::Type     => "Type",
            NodeKind::Test     => "Test",
            NodeKind::Module   => "Module",
            NodeKind::Import   => "Import",
        };
        write!(f, "{}", s)
    }
}

impl std::fmt::Display for ConfidenceTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ConfidenceTier::Extracted  => "EXTRACTED",
            ConfidenceTier::Inferred   => "INFERRED",
            ConfidenceTier::Ambiguous  => "AMBIGUOUS",
        };
        write!(f, "{}", s)
    }
}

// Then in build.rs lines 62, 83, 89: change format!("{:?}", ...) to format!("{}", ...)
kind: format!("{}", node.kind),            // "File", "Class", etc.
kind: format!("{}", edge.kind),            // "CALLS", "IMPORTS_FROM", etc.
confidence_tier: format!("{}", edge.confidence_tier), // "EXTRACTED", etc.
```

**Also fix `cluster.rs` EDGE_WEIGHTS at lines 8–17** — currently uses CamelCase which accidentally matches the DB format. After this fix, the DB will store `"CALLS"` etc., so EDGE_WEIGHTS must be updated to match:

```rust
// cluster.rs lines 8-17 — BEFORE (currently matches, will break after fix):
const EDGE_WEIGHTS: &[(&str, f32)] = &[
    ("Calls", 1.0),
    ("ImportsFrom", 0.5),
    ...
];

// AFTER (must match new UPPER_SNAKE_CASE format):
const EDGE_WEIGHTS: &[(&str, f32)] = &[
    ("CALLS", 1.0),
    ("IMPORTS_FROM", 0.5),
    ("INHERITS", 0.8),
    ("IMPLEMENTS", 0.7),
    ("CONTAINS", 0.3),
    ("TESTED_BY", 0.4),
    ("DEPENDS_ON", 0.6),
    ("REFERENCES", 0.4),
];
```

> ⚠️ After this fix, existing `~/.autodev/graph.db` records will have the old CamelCase kinds. A forced **rebuild** (not update) is required to repopulate with UPPER_SNAKE_CASE. The migration version in `migrations.rs` should be bumped so the old DB is wiped or re-indexed.

---

## Fix G-2 · `extract.rs` — 10 of 11 language extractors are file-node-only stubs; graph has no real edges

**Status:** ✅ Done — All 11 language extractors fully implemented with regex two-pass (structural + import). `extract_js_ts`: classes, named/arrow functions, import-from edges, `resolve_js_import()` relative resolver probing .ts/.tsx/.js/.jsx/.mjs + index variants. `extract_rust`: fn/struct/enum/trait, `#[test]` attribute + `test_` prefix detection. `extract_go`: func (with receiver), struct/interface. `extract_java`: class/interface/enum/record, methods via access-modifier regex, package import edges. `extract_c_cpp`: class/struct, function defs at low indentation, `#include "local.h"` relative resolution + `#include <sys>` external. `extract_ruby`: class/module, def/def self, `require_relative` file resolution + `require` external. `extract_php`: class/interface/trait/enum, functions, `use` namespace, `require`/`include` relative resolution. `extract_kotlin`: class/interface/object/enum class, fun (suspend/inline/operator), typealias, package imports. `extract_swift`: class/struct/enum/protocol/extension/actor, func, typealias, framework imports. `extract_bash`: function definitions, `source`/`.` script file resolution. EXTRACTOR_VERSION bumped to 4.

**File:** `src-tauri/src/graph/extract.rs` lines 417–657

**Finding 1 — JS/TS extractor (lines 417–444) ignores `_source` entirely:**

```rust
fn extract_js_ts(path: &Path, _source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    // Only emits one File node. No functions, no imports, no classes.
    result.nodes.push(NodeInfo { kind: NodeKind::File, ... });
    Ok(result)
}
```

`_source` is a prefixed-underscore parameter, meaning the source bytes are intentionally unused. The function is a placeholder that compiles but extracts nothing. Same pattern repeated for:
- `extract_rust` (lines 446–467)
- `extract_go` (lines 469–490)
- `extract_java` (lines 492–513)
- `extract_c_cpp` (lines 515–541)
- `extract_ruby` (lines 543–564)
- `extract_php` (lines 566–587)
- `extract_kotlin` (lines 589–610)
- `extract_swift` (lines 612–633)
- `extract_bash` (lines 635–657)

**Finding 2 — Python extractor (lines 222–415) IS implemented but has no `CALLS` edge extraction:**

Python extracts classes, functions, and imports correctly. But there is no regex pass for function call sites (e.g., `foo()`, `self.bar()`, `module.func()`). So Python files produce `CONTAINS` edges (file→function, file→class) and `IMPORTS`/`IMPORTS_FROM` edges, but zero `CALLS` edges. Callers, callees, and impact radius analysis all depend on `CALLS` — none work for Python either.

**Finding 3 — `from X import Y` maps target to `module::X`, not to a real file node:**

```rust
// extract.rs line 383-394
let target = format!("module::{}", module);  // e.g. "module::fastapi"
result.edges.push(EdgeInfo {
    kind: EdgeKind::ImportsFrom,
    source_qualified: file_path.clone(),
    target_qualified: target.clone(),    // never matches another File node's qualified_name
    ...
});
```

For relative imports (`from .utils import helper`), the module resolves to `module::.utils`, which is filtered out in `graph_commands.rs:692` because it starts with `module::`. So cross-file `IMPORTS_FROM` edges between project files are completely absent — every file is an island.

**Required changes:**

For each stub language extractor, implement a regex-based two-pass extraction:
1. **Structural pass**: classes, functions/methods, type definitions
2. **Import pass**: import statements → cross-file `IMPORTS_FROM` edges (resolving relative paths to qualified_names of other File nodes)

For JS/TS specifically (highest priority for this codebase):

```rust
fn extract_js_ts(path: &Path, source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    let source_str = std::str::from_utf8(source).unwrap_or("");
    
    // File node (keep as-is)
    result.nodes.push(NodeInfo { kind: NodeKind::File, ... });
    
    // Regex patterns
    let func_re  = Regex::new(r"(?m)^(?:export\s+)?(?:async\s+)?function\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*\(").unwrap();
    let arrow_re = Regex::new(r"(?m)^(?:export\s+)?(?:const|let|var)\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*=\s*(?:async\s*)?\(").unwrap();
    let class_re = Regex::new(r"(?m)^(?:export\s+)?class\s+([A-Za-z_$][A-Za-z0-9_$]*)").unwrap();
    // import { X } from './y' and import X from './y'
    let import_re = Regex::new(r#"(?m)^\s*import\s+(?:\{[^}]*\}|[A-Za-z_$][A-Za-z0-9_$]*)\s+from\s+['"]([^'"]+)['"]"#).unwrap();
    
    // Extract functions, arrow functions, classes
    // Emit CONTAINS edges from File node
    // For imports: resolve relative path to absolute, create IMPORTS_FROM edge to target file's qualified_name
}
```

The relative import resolution must turn `'./utils'` into the absolute path of `utils.ts`/`utils/index.ts` so the edge target matches the other File node's `qualified_name`. This is the structural fix that creates the "web" the user expects to see.

**Scope note:** Full tree-sitter integration (spec section 7, stage 2) is the eventual target but is a larger effort. Regex-based extraction for the 5 most common languages (Python+calls, JS/TS, Rust, Go, Java) will make the graph immediately useful and match the reference implementation's quality.

---

## Fix G-3 · `Graph.tsx` — edge filter toggle broken due to case mismatch

**Status:** ✅ Done (self-healed) — `edgeFilters[e.kind] !== false` at line 286 of `Graph.tsx` already uses UPPER_SNAKE_CASE keys in `DEFAULT_EDGE_FILTERS`. After G-1 made all stored edge kinds UPPER_SNAKE_CASE (`CALLS`, `IMPORTS_FROM`, etc.) the filter started working with no additional code change. Verified: keys in `EDGE_COLORS` and `DEFAULT_EDGE_FILTERS` match the UPPER_SNAKE_CASE values emitted by the backend.

**File:** `src/tabs/Graph.tsx` lines 28–32, 283

**Finding — `edgeFilters` keys are UPPER_SNAKE_CASE but `edge.kind` from backend is CamelCase:**

```typescript
// Graph.tsx line 28-32
const DEFAULT_EDGE_FILTERS: Record<string, boolean> = {
  CALLS: true, IMPORTS_FROM: true, INHERITS: true, IMPLEMENTS: true,
  TESTED_BY: true, DEPENDS_ON: true, REFERENCES: false,
  CONTAINS: false, SEMANTICALLY_SIMILAR: false,
}

// Graph.tsx line 283 — the filter check:
.filter(e => {
  ...
  return nodeIds.has(s) && nodeIds.has(t) && edgeFilters[e.kind] !== false
  //                                                     ^^^^^^^^
  // e.kind = "Calls" (CamelCase from DB), edgeFilters["Calls"] = undefined
  // undefined !== false → true → ALL edges pass regardless of toggle state
})
```

This means:
- Clicking "CALLS" in the toolbar visually toggles the badge but has zero effect on the graph
- "REFERENCES" and "CONTAINS" appear disabled by default but their edges still render
- After Fix G-1 lands, this will start working correctly with no additional change needed IF the frontend filter keys match the new UPPER_SNAKE_CASE format from the backend.

This is not a separate code fix — it **self-heals when G-1 is applied**. But it must be verified after G-1.

**Verification step after G-1:**  
Confirm `edge.kind` values in `vizData.edges` returned from `get_graph_visualization_data` are `"CALLS"`, `"IMPORTS_FROM"`, etc. and match `DEFAULT_EDGE_FILTERS` keys exactly.

---

## Fix G-4 · `graph_commands.rs` — single global database regardless of workspace; multi-repo data corruption

**Status:** ✅ Done — `repo_db_path()` helper added; all 13 commands now use `~/.autodev/graphs/<sha256-12>.db` keyed by repo path; `get_graph_visualization_data` accepts `repo_path` param; frontend passes `repoPath: workspaceDir` to that call

**File:** `src-tauri/src/commands/graph_commands.rs` lines 103–113, 635–637

**Finding 1 — `build_graph` hardcodes `~/.autodev/graph.db` for all repos:**

```rust
// graph_commands.rs lines 103-113
let home_dir = dirs::home_dir()...;
let autodev_dir = home_dir.join(".autodev");
std::fs::create_dir_all(&autodev_dir)?;
let db_path = autodev_dir.join("graph.db");  // ← always the same path
```

If a user builds the graph for repo A (`C:\AIRS Foundry\autodev`) then builds for repo B (`C:\AIRS Foundry\core-orchestrator`), the second build adds to the same DB without clearing the first repo's nodes. The graph will show a merged soup of both codebases with no way to distinguish them.

**Finding 2 — `get_graph_visualization_data` (lines 635–637) also hardcodes the path, and takes no `repo_path` parameter:**

```rust
#[tauri::command]
pub async fn get_graph_visualization_data() -> Result<GraphVisualizationData> {
    let db_path = home_dir.join(".autodev").join("graph.db");  // ← ignores current workspace
```

The command signature takes no arguments. The frontend calls it without context (`invoke('get_graph_visualization_data')`). If you switch workspace in the app, the graph still shows the old repo's data.

**Finding 3 — `delete_graph` (lines 619–625) also hardcodes path:**

```rust
let db_path = autodev_dir.join("graph.db");
std::fs::remove_file(&db_path)?;
```

Deleting always hits the global DB regardless of which repo is selected.

**Required changes:**

```rust
// Helper: per-repo database path derived from repo path hash
fn repo_db_path(repo_path: &str) -> Result<PathBuf> {
    use sha2::{Sha256, Digest};
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home dir"))?;
    let slug = hex::encode(Sha256::digest(repo_path.as_bytes()))[..12].to_string();
    Ok(home.join(".autodev").join("graphs").join(format!("{}.db", slug)))
}

// All commands: replace hardcoded db_path with:
let db_path = repo_db_path(&repo_path)?;
std::fs::create_dir_all(db_path.parent().unwrap())?;

// get_graph_visualization_data must accept repo_path:
#[tauri::command]
pub async fn get_graph_visualization_data(repo_path: Option<String>) -> Result<GraphVisualizationData> {
    let repo_path = repo_path.and_then(|p| if p.is_empty() { None } else { Some(p) })
        .ok_or_else(|| anyhow::anyhow!("repo_path required"))?;
    let db_path = repo_db_path(&repo_path)?;
    ...
}

// Frontend call must pass workspaceDir:
const vizData = await invoke<GraphVisualizationData>('get_graph_visualization_data', {
    repoPath: workspaceDir  // Graph.tsx line 548
})
```

---

## Fix G-5 · `extract.rs` — `save_cached()` silently discards write errors; `load_cached` uses unwrap on home dir

**Status:** ✅ Done — `save_cached` uses `ok_or_else` instead of `unwrap` on home dir; `extract()` logs cache write failures with `tracing::warn!` instead of `let _ =`

**File:** `src-tauri/src/graph/extract.rs` lines 124–140, 111

**Finding 1 — Cache save errors are silently ignored at line 181:**

```rust
// extract.rs line 181
let _ = save_cached(path, &file_hash, &result);  // error swallowed with let _
```

If the cache directory is read-only, on a network share, or full, every file will be re-extracted on every build with no warning. On a 1000-file repo this adds significant time.

**Finding 2 — `save_cached` at line 125 uses `unwrap()` on home_dir:**

```rust
pub fn save_cached(...) -> Result<()> {
    let cache_dir = dirs::home_dir().unwrap().join(".autodev/graph_cache");  // panics on server envs
```

`load_cached` uses the safe `?` operator but `save_cached` uses `.unwrap()`. On CI or Docker containers without a home directory, this panics and crashes the build pipeline.

**Required changes:**

```rust
pub fn save_cached(file_path: &Path, file_hash: &str, result: &ExtractionResult) -> Result<()> {
    let cache_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("no home dir for cache"))?  // no unwrap
        .join(".autodev/graph_cache");
    fs::create_dir_all(&cache_dir)?;
    ...
    fs::write(&cache_file, serde_json::to_string(&entry)?)?;
    Ok(())
}

// In extract():
if let Err(e) = save_cached(path, &file_hash, &result) {
    tracing::warn!("[extract] Cache write failed for {}: {}", path.display(), e);
    // continue — cache miss is not fatal
}
```

---

## Fix G-6 · `extract.rs` — `from X import Y` for intra-project imports resolves to `module::X`, filtered out as external

**Status:** ✅ Done — `resolve_python_import()` helper added; relative imports (`.`) resolved to real file abs-paths; only unresolvable/external imports keep `module::` stubs; confidence 1.0/EXTRACTED for internal, 0.7/INFERRED for external; `EXTRACTOR_VERSION` bumped to 3; `repo_root` threaded through `extract_all` → `extract` → `extract_python`

**File:** `src-tauri/src/graph/extract.rs` lines 378–410  
**Also:** `src-tauri/src/commands/graph_commands.rs` lines 688–693

**Finding — Python relative imports (`from .utils import helper`) produce dead-end edges:**

```rust
// extract.rs line 383
let target = format!("module::{}", module);  // e.g. "module::.utils"
```

Then in graph_commands.rs:
```rust
// graph_commands.rs lines 690-693
let project_nodes: Vec<_> = all_nodes
    .iter()
    .filter(|node| !node.qualified_name.starts_with("module::"))  // ← filters out ALL module:: nodes
    .collect();
```

Any file that imports from another project file gets its edge filtered out before visualization. The graph shows zero cross-file `IMPORTS_FROM` edges even though Python has them. Files are isolated islands.

**The correct approach:** During the extraction pass, if a `from X import Y` statement resolves to a file within the same repo, the edge should point directly to that file's `qualified_name` (its absolute path), not to a `module::` stub.

**Required changes in `extract_python`:**

```rust
// Pass the repo_root into extract_python so we can resolve relative imports
fn extract_python(path: &Path, source: &[u8], file_hash: &str, repo_root: &Path) -> Result<ExtractionResult> {
    ...
    for cap in from_import_re.captures_iter(source_str) {
        let module = cap.get(1).unwrap().as_str();
        
        // Try to resolve to a real file
        let resolved = resolve_python_import(path, module, repo_root);
        let (target_qname, is_external) = match resolved {
            Some(abs_path) => (abs_path.to_string_lossy().to_string(), false),
            None => (format!("module::{}", module), true),
        };
        
        result.edges.push(EdgeInfo {
            kind: EdgeKind::ImportsFrom,
            source_qualified: file_path.clone(),
            target_qualified: target_qname.clone(),
            confidence: if is_external { 0.7 } else { 1.0 },
            confidence_tier: if is_external { ConfidenceTier::Inferred } else { ConfidenceTier::Extracted },
            ...
        });
        
        if is_external {
            // Only emit stub node for external modules
            result.nodes.push(NodeInfo { kind: NodeKind::Module, qualified_name: target_qname, ... });
        }
        // If internal: the target File node will be created when we parse that file
    }
}

fn resolve_python_import(current_file: &Path, module: &str, repo_root: &Path) -> Option<PathBuf> {
    if module.starts_with('.') {
        // Relative import: from .utils import x → sibling utils.py
        let dots = module.chars().take_while(|c| *c == '.').count();
        let rest = &module[dots..];
        let mut base = current_file.parent()?;
        for _ in 1..dots { base = base.parent()?; }
        let candidate = base.join(rest.replace('.', "/")).with_extension("py");
        if candidate.exists() { return Some(candidate); }
        let candidate_init = base.join(rest.replace('.', "/")).join("__init__.py");
        if candidate_init.exists() { return Some(candidate_init); }
    }
    None  // external/unresolvable
}
```

---

## Fix G-7 · `cluster.rs` — greedy BFS produces one "miscellaneous" mega-community (285 nodes, 22% of graph)

**Status:** ✅ Done — Replaced broken BFS-only `detect_communities` and no-op `split_oversized` with a 4-phase modularity-aware algorithm in `cluster.rs`: Phase 1 strong-edge connected components (≥0.45 weight threshold — keeps IMPORTS_FROM/CALLS/INHERITS/IMPLEMENTS/DEPENDS_ON, drops CONTAINS/0.3 and REFERENCES/0.4; excludes `module::` stubs that previously bridged all importers into one mega-component); Phase 2 oversized-component splitting (>max(15, 20% of non-module nodes), re-clustered on subgraph with ≥0.65 threshold keeping only CALLS/INHERITS/IMPLEMENTS); Phase 3 orphan re-attachment (isolated nodes assigned to best neighbor community by weighted vote through full non-module adjacency, including CONTAINS so functions attach to their parent file's community); Phase 4 module stub placement (external `module::` nodes placed in their most-frequent importer's community). Five private helper functions added: `build_filtered_adjacency`, `connected_components_bfs`, `subgraph_filtered`, `reassign_orphans`, `assign_module_nodes`. `split_oversized` removed; `run_clustering` updated to call only `detect_communities`.

**File:** `src-tauri/src/graph/cluster.rs` lines 61–100

**Finding 1 — BFS treats any connected component as a single community:**

```rust
// cluster.rs line 61-100
pub fn detect_communities(adjacency: &HashMap<String, Vec<(String, f32)>>) -> HashMap<String, i32> {
    // Simple greedy clustering: BFS-based community detection
    for node in adjacency.keys() {
        let mut queue = vec![node.clone()];
        while let Some(current) = queue.pop() {
            // BFS walks ALL reachable nodes in one pass → one community per connected component
```

A single large connected component (e.g. all Python files linked by `module::` edges) becomes one community. The screenshot shows `miscellaneous` with 285 nodes — this is the giant component of all nodes reachable through any path.

**Finding 2 — Isolated nodes (no edges) get their own community each:**

Every node with degree 0 runs the BFS loop once, creates a new community ID, and increments `community_id` even though the community has one member. This produces many single-node communities and inflates the count.

**Finding 3 — After Fix G-2 and G-6, this will improve naturally** because real inter-file edges will split the monolithic component into genuine modules. The algorithm itself is still too naive but becomes less wrong.

**Required changes (minimum viable):**

The simplest improvement is **modularity-aware splitting**: after BFS produces components, split any component larger than 25% of total nodes by recursively running BFS on the subgraph with higher edge-weight threshold. This is the same approach graphify-5 uses.

```rust
pub fn detect_communities(adjacency: &HashMap<String, Vec<(String, f32)>>) -> HashMap<String, i32> {
    // Phase 1: connected components (existing BFS)
    let components = find_connected_components(adjacency);
    let total_nodes = adjacency.len();
    let threshold = (total_nodes as f32 * 0.25) as usize;
    
    // Phase 2: split oversized components
    let mut assignments: HashMap<String, i32> = HashMap::new();
    let mut next_id = 0i32;
    
    for component in components {
        if component.len() > threshold.max(10) {
            // Re-cluster subgraph with weight cutoff
            let sub_adjacency = subgraph(adjacency, &component, 0.6); // only strong edges
            let sub_components = find_connected_components(&sub_adjacency);
            for sub in sub_components {
                for node in sub { assignments.insert(node, next_id); }
                next_id += 1;
            }
        } else {
            for node in component { assignments.insert(node, next_id); }
            next_id += 1;
        }
    }
    assignments
}
```

---

## Fix G-8 · `Graph.tsx` — node size uses fixed `size` field (always 8 or 15) instead of actual degree

**Status:** ✅ Done — Backend `size` computation replaced in `get_graph_visualization_data` (`graph_commands.rs`). Added `degree_based_size(degree: usize) -> f32` private helper mirroring the frontend `nodeRadius()` formula: `(4.0 + ln(1+d) * 3.5).min(22.0)`. A `degree_map` is precomputed from `all_edges` before building `viz_nodes`, so every node's `size` now reflects its actual connection count rather than a fixed kind-based constant.

**File:**  
- `src-tauri/src/commands/graph_commands.rs` lines 705–707  
- `src/tabs/Graph.tsx` lines 263–270, 275, 405–408

**Finding 1 — Backend sets `size` based on kind, not degree:**

```rust
// graph_commands.rs lines 705-707
let size = if node.kind.contains("File") { 15.0 } else { 8.0 };
VisualizationNode { ..., size, ... }
```

All File nodes are size 15, all others are size 8 regardless of how many connections they have.

**Finding 2 — Frontend recomputes degree from edges correctly (lines 263–270) but `node.size` is not used for `nodeRadius()`:**

```typescript
// Graph.tsx lines 263-270 — correctly computes degree
const degMap = new Map<string, number>()
graphData.edges.forEach(e => {
    degMap.set(s, (degMap.get(s) || 0) + 1)
    degMap.set(t, (degMap.get(t) || 0) + 1)
})

// Graph.tsx line 275 — attaches _degree to each node
.map(n => ({ ...n, _degree: degMap.get(n.id) || 0 }))

// Graph.tsx line 407 — nodeRadius uses _degree ✓ (this is correct)
symGen.type(symbolForKind(d.kind || '')).size(Math.PI * r * r)
```

The frontend correctly derives radius from `_degree`. The backend `size` field is set but never actually used. This is not a visual bug — but the `size` field in `VisualizationNode` is wasted bandwidth and misleading. However it does affect the **tooltip and inspector** if they display `node.size` directly.

**Required change (cleanup):** Remove the `size` computation from the backend — it's dead weight. Or align it with the frontend's degree-based formula so the field is at least meaningful.

---

## Fix G-9 · `Graph.tsx` line 348 — edge CSS class includes raw `e.kind` which causes invalid CSS selectors before G-1

**Status:** ⬜ Self-heals with G-1

**File:** `src/tabs/Graph.tsx` line 348

```typescript
.attr('class', (d: any) => `edge edge-${d.kind.toLowerCase()}`)
// Before G-1: "edge edge-importsfrom", "edge edge-calls"
// After G-1:  "edge edge-imports_from", "edge edge-calls"
```

CSS classes like `edge-imports_from` (with underscore) are valid. No action needed beyond confirming G-1 output. The CSS in Graph.css at line ~302 references `.edge` generically and doesn't use the kind-suffixed classes for styling — so this is cosmetic/unused.

---

## Fix G-10 · `graph_commands.rs` — `get_graph_visualization_data` fetches entire graph with `i64::MAX` limit; no pagination

**Status:** ✅ Done — Added `NODE_RENDER_LIMIT = 2000` constant in `graph_commands.rs`. When `total_code_count > NODE_RENDER_LIMIT`, the command returns a community-aggregated view instead of the full node list: one `VisualizationNode` per community (`kind="module"`, `id="community::{id}"`, `size=degree_based_size(member_count)`), and deduplicated cross-community `VisualizationEdge`s (direction-normalised, weight-summed per kind). Added `is_aggregated: bool` and `total_node_count: usize` fields to `GraphVisualizationData` (Rust struct + TypeScript interface). Frontend shows a `graph-aggregation-banner` strip when `is_aggregated === true`. Styled in `Graph.css`.

**File:** `src-tauri/src/commands/graph_commands.rs` lines 642–647

```rust
let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
```

For the current 1280-node graph this is fine (~2ms). For a 10,000-node codebase, this loads ~100MB of JSON into memory on the Rust side, serializes it all to the frontend, and D3 tries to render all 10k nodes simultaneously (the spec caps at 5000 for SVG, but there's no cap here).

**Required change:** Apply the `max_nodes` limit from the spec. When `total_nodes > 2000`, switch to community-aggregated view server-side before sending data.

---

## Fix G-11 · `extract.rs` — Python extractor has no `CALLS` edge detection; impact radius and flow analysis are dead for all languages

**Status:** ✅ Done — heuristic second pass added after function extraction; builds `known_funcs` map from current file, regex-scans source for `name(` tokens, emits one deduplicated `CALLS` edge per (file → callee) pair; confidence 0.8/EXTRACTED

**File:** `src-tauri/src/graph/extract.rs` lines 222–415

**Finding — No call-site extraction exists anywhere:**

Python extracts definitions (classes, functions) but does not scan function bodies for call expressions. There is no regex for `(\w+)\s*\(` that would detect `foo()`, `self.bar()`, or `obj.method()`.

Without `CALLS` edges:
- `get_impact_radius()` in `analyze.rs` will return empty results (BFS through CALLS finds nothing)
- `list_callers()` MCP tool returns empty
- `get_affected_flows()` returns empty
- The graph shows only structural containment — what files have — not how they interact

**Required changes for Python:**

```rust
// After extracting functions, do a second pass over function bodies for call detection
let call_re = Regex::new(r"(?m)([A-Za-z_][A-Za-z0-9_]*)\s*\(").unwrap();

// For each function/class, scan body lines for known function names defined in this file
// Emit CALLS edges where source = current_function, target = called_function (if known)
// Confidence = EXTRACTED for intra-file calls, INFERRED for cross-file
```

This is a heuristic (regex, not AST) but dramatically better than zero. Cross-file calls require knowing the target function's qualified_name, which requires the import resolution from Fix G-6 to be in place first.

---

## Fix G-12 · `Graph.tsx` — `loadGraphStats` ignores `isLoadingStats` guard on re-entry but `isBuilding` state cleared before visualization loads

**Status:** ✅ Done — `finally { setIsBuilding(false) }` added to both `handleBuildGraph` and `handleUpdateGraph`

**File:** `src/tabs/Graph.tsx` lines 541–560, 562–574

**Finding — `handleBuildGraph` calls `loadGraphStats()` after `invoke('build_graph')` but `setIsBuilding(false)` is never called on the success path:**

```typescript
// Graph.tsx lines 563-574
const handleBuildGraph = async () => {
    setIsBuilding(true)
    try {
        await invoke('build_graph', { repoPath: workspaceDir })
        await loadGraphStats()  // ← isBuilding remains true during this call
        // ← missing setIsBuilding(false) on success path
    } catch (err) {
        // ← missing setIsBuilding(false) on error path too
        alert(`Build failed: ${err}`)
    }
}
```

The `isBuilding` flag is only cleared by the `graph:build_progress` event listener when `phase === 'done'`. If the Tauri backend emits `done` before the frontend `invoke()` call resolves (possible with async timing), the loading overlay disappears before `loadGraphStats()` completes, creating a flash where the old empty state is visible.

More critically: if the build throws an error, `isBuilding` stays `true` permanently and the loading overlay never clears — the UI is stuck.

**Required changes:**

```typescript
const handleBuildGraph = async () => {
    if (!workspaceDir) { alert('Set a workspace directory in Settings first.'); return }
    setIsBuilding(true)
    setBuildProgress({ phase: 'init', message: 'Starting build…', percent: 5 })
    try {
        await invoke('build_graph', { repoPath: workspaceDir })
        await loadGraphStats()
    } catch (err) {
        console.error('[Graph] build_graph:', err)
        alert(`Build failed: ${err}`)
    } finally {
        setIsBuilding(false)  // ← always clear, regardless of success/error
    }
}
// Same fix applies to handleUpdateGraph
```

---

## Fix G-13 · `Graph.tsx` — `symbolForKind` uses `.includes()` which makes "File" nodes resolve to squares (same as "Class")

**Status:** ✅ Done — replaced `if/includes` chain with `switch` on exact lowercase kind; File → `symbolDiamond`, Type → `symbolStar` (was Diamond, freed up for File)

**File:** `src/tabs/Graph.tsx` lines 34–44

```typescript
function symbolForKind(kind: string): d3.SymbolType {
  const k = kind.toLowerCase()
  if (k.includes('class'))    return d3.symbolSquare   // "Class" → square
  if (k.includes('function')) return d3.symbolCircle
  if (k.includes('test'))     return d3.symbolTriangle
  if (k.includes('type'))     return d3.symbolDiamond
  if (k.includes('file'))     return d3.symbolSquare   // "File" → square (same as Class!)
  ...
}
```

`"File"` and `"Class"` both resolve to `d3.symbolSquare`. In the screenshot you can see a mix of squares — these could be either File hub nodes or Class nodes with no way to tell them apart. The spec (section 11.3) says Files should have "no border" while Classes have "thick border", but neither the border thickness nor a distinct shape differentiates them.

**Required change:** Give File nodes a distinct symbol. `d3.symbolDiamond` is currently used for Type — swap File to a larger square with a distinct visual treatment, or use a custom SVG path (e.g., a document/rect icon).

```typescript
function symbolForKind(kind: string): d3.SymbolType {
  switch (kind.toLowerCase()) {
    case 'file':     return d3.symbolDiamond2  // or keep square but handle in render
    case 'class':    return d3.symbolSquare
    case 'function': return d3.symbolCircle
    case 'test':     return d3.symbolTriangle
    case 'type':     return d3.symbolDiamond
    case 'module':   return d3.symbolWye
    case 'import':   return d3.symbolCross
    default:         return d3.symbolCircle
  }
}
```

Using `switch` instead of `.includes()` also prevents false matches if a future kind like `"typescript_interface"` accidentally matches `"type"`.

---

## Fix G-14 · `graph_commands.rs` — `run_clustering` called in commands file, not in `cluster.rs`; coupling concern

**Status:** ✅ Done (already correctly structured) — `run_clustering` is a `pub fn` in `src-tauri/src/graph/cluster.rs` and is imported into `graph_commands.rs` via `use crate::graph::cluster::{run_clustering, Community}`. The layering violation described in the finding did not exist in the current codebase; the function was already in the cluster module before this session.

**File:** `src-tauri/src/commands/graph_commands.rs` (contains `run_clustering()` helper)

**Finding — `run_clustering` is a private function inside `graph_commands.rs` rather than part of the `cluster` module:**

The commands file orchestrates the clustering call but also implements the cluster→community name→store logic. If a second command (e.g., a scheduled incremental re-cluster) needs to run clustering, it must call through `graph_commands.rs` or duplicate code. This is a layering violation: commands should call into the graph engine, not implement engine logic themselves.

**Required change:** Move `run_clustering()` into `cluster.rs` as a public function:

```rust
// cluster.rs — add:
pub fn run_clustering_and_store(store: &GraphStore) -> Result<Vec<Community>> {
    let adjacency = build_adjacency(store)?;
    let assignments = detect_communities(&adjacency);
    let communities = name_communities(&assignments, store)?;
    store.set_communities_batch(&communities)?;
    Ok(communities)
}
```

---

## Implementation Priority

| # | Fix | Impact | Effort | Do First |
|---|-----|--------|--------|----------|
| G-1 | Edge kind serialization (CamelCase→UPPER_SNAKE_CASE) | 🔴 Critical | S (30 min) | ✅ Yes |
| G-2 | Stub language extractors — implement JS/TS, Rust | 🔴 Critical | L (2–3 days) | ✅ Yes |
| G-6 | Python relative import resolution | 🔴 Critical | M (4 hours) | ✅ Yes |
| G-11 | Python CALLS edge extraction | 🔴 High | M (4 hours) | After G-6 |
| G-4 | Per-repo database (multi-repo isolation) | 🟠 High | M (2 hours) | After G-1 |
| G-12 | Build button race condition / stuck loading overlay | 🟠 High | S (30 min) | Any time |
| G-5 | Cache write panic safety | 🟡 Medium | S (15 min) | Any time |
| G-7 | Community detection BFS → modularity-aware | 🟡 Medium | M (3 hours) | After G-2 |
| G-13 | File vs Class symbol ambiguity | 🟡 Medium | S (20 min) | Any time |
| G-3 | Edge filter verification | Verify only | — | After G-1 |
| G-8 | Remove dead `size` field from backend | 🟢 Low | S (10 min) | Polish |
| G-10 | Pagination for large graphs | 🟢 Low | M (2 hours) | Later |
| G-9 | Edge CSS class format | Self-heals | — | None |
| G-14 | `run_clustering` layering | 🟢 Low | S (20 min) | Polish |

**Recommended session order:**
1. ~~G-1 + G-12 + G-5 + G-13 in one session (all small fixes, can be done in parallel)~~ ✅ Done
2. ~~G-4 in a dedicated session (DB path change, needs careful testing)~~ ✅ Done
3. ~~G-6 + G-11 together (both in `extract.rs`, Python import + call detection)~~ ✅ Done
4. ~~G-2 in a dedicated session per language (JS/TS first, then Rust, then Go)~~ ✅ Done
5. ~~G-7 after G-2 (clustering only improves once real edges exist)~~ ✅ Done
6. ~~G-3 verify, G-8, G-10, G-14 as polish~~ ✅ Done

---

## Known Unknowns (investigate before implementing)

1. **Does `store.get_all_edges_for_graph()` return `kind` as stored in DB or does it transform it?** Check `store.rs` around line 200. If the store applies any transformation, G-1 may need changes there too.
2. **Does `update_graph` (incremental) call `reconcile_edges()`?** If it skips reconciliation, edges for changed files may accumulate duplicates after multiple updates.
3. **Does the FTS5 rebuild at the end of `build()` account for the new UPPER_SNAKE_CASE kind values?** FTS5 indexes `name, qualified_name, file_path` — kind is not indexed so this is fine.
4. **After G-4 (per-repo DB), the old `~/.autodev/graph.db` will be abandoned.** Decide: migrate data, delete silently, or warn the user on first build.
