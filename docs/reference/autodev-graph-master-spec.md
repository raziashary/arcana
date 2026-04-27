# AutoDev Graph Engine — Master Specification
### Phase 6 Reference Document: Code Knowledge Graph for AIRS Foundry

**Document Status:** Reference v1.0  
**Author:** Razi (Programming Lead, AIRS Foundry)  
**Date:** April 2026  
**Inputs:** graphify v5, code-review-graph (code-review-graph-main)  
**Classification:** Internal — AutoDev Phase 6

---

## Table of Contents

1. [Why a Code Graph](#1-why-a-code-graph)
2. [Reference Codebase Analysis: graphify-5](#2-reference-codebase-analysis-graphify-5)
3. [Reference Codebase Analysis: code-review-graph](#3-reference-codebase-analysis-code-review-graph)
4. [Head-to-Head Comparison](#4-head-to-head-comparison)
5. [AutoDev Graph Architecture (Combined Best)](#5-autodev-graph-architecture-combined-best)
6. [Data Model: Nodes and Edges](#6-data-model-nodes-and-edges)
7. [Pipeline: The Seven Stages](#7-pipeline-the-seven-stages)
8. [Storage Layer: SQLite Schema](#8-storage-layer-sqlite-schema)
9. [Analysis Engine](#9-analysis-engine)
10. [MCP Tool Surface](#10-mcp-tool-surface)
11. [Visualization: The Graphify Tab](#11-visualization-the-graphify-tab)
12. [Incremental Updates](#12-incremental-updates)
13. [Worker Integration](#13-worker-integration)
14. [Atomic Build Tasks (Phase 6 Breakdown)](#14-atomic-build-tasks-phase-6-breakdown)

---

## 1. Why a Code Graph

AI agents read your entire codebase on every task. This burns tokens, hits context limits, and produces worse results than targeted context. A code knowledge graph fixes this.

**The core idea**: parse the codebase once into a structural map of nodes (files, classes, functions, types, tests) and edges (imports, calls, inheritance, test coverage), persist it in SQLite, and query it at task time to surface only the nodes the agent actually needs.

**Token reduction observed in reference projects**:
- graphify-5 reports: graph subgraph query vs full-corpus naive scan
- code-review-graph reports: **8.2x average token reduction** across 6 real repositories

For AutoDev specifically, the graph serves three audiences:
1. **PA (Razi)**: "What does this file depend on? What tests cover it?"
2. **Workers**: "Before I implement feature X, which existing nodes do I need to read?"
3. **QC Agent**: "What is the blast radius of this PR? Which tests should I run?"

---

## 2. Reference Codebase Analysis: graphify-5

### 2.1 Overview

graphify is a Python library + Claude Code skill that converts any folder of code, docs, or papers into a **NetworkX graph** persisted as `graphify-out/graph.json`. It's designed primarily as a navigation aid: build the graph once, then let the AI agent traverse it instead of grepping raw files.

**Repository**: `graph-references/graphify-5/`  
**Primary module**: `graphify/` package  
**CLI entry point**: `graphify/__main__.py`

### 2.2 Core Pipeline

```
detect() → extract() → build() → cluster() → analyze() → report() → export()
```

Each stage is a **single pure function in its own module**. No shared state. No side effects outside `graphify-out/`. Stages communicate through Python dicts and NetworkX graphs.

| Stage | Module | Function | Input → Output |
|-------|--------|----------|----------------|
| 1. Detect | `detect.py` | `collect_files(root)` | directory → `[Path]` filtered list |
| 2. Extract | `extract.py` | `extract(path)` | file path → `{nodes, edges}` dict |
| 3. Build | `build.py` | `build_from_json(extraction)` | extraction dict → `nx.Graph` |
| 4. Cluster | `cluster.py` | `cluster(G)` | graph → `{community_id: [node_ids]}` |
| 5. Analyze | `analyze.py` | `god_nodes(G)`, `surprising_connections(G, communities)` | graph → analysis dict |
| 6. Report | `report.py` | `generate(G, communities, ...)` | graph + analysis → `GRAPH_REPORT.md` string |
| 7. Export | `export.py` | `to_json()`, `to_html()`, `to_svg()` | graph → files in `graphify-out/` |

### 2.3 Extraction System (`extract.py`)

**Language support** (tree-sitter based):
- Python, JavaScript, TypeScript, TSX, Go, Rust, Java, C, C++, C#, Ruby, PHP, Kotlin, Swift, Scala, Bash, and more

**LanguageConfig dataclass** — each language is described as a config object:
```python
@dataclass
class LanguageConfig:
    ts_module: str                  # tree-sitter grammar package
    class_types: frozenset          # AST node types for classes
    function_types: frozenset       # AST node types for functions
    import_types: frozenset         # AST node types for imports
    call_types: frozenset           # AST node types for call expressions
    import_handler: Callable        # per-language import edge builder
    extra_walk_fn: Callable         # language-specific extras (arrow fns, C# namespaces)
```

**Node ID generation** — stable, deterministic:
```python
def _make_id(*parts: str) -> str:
    combined = "_".join(p.strip("_.") for p in parts if p)
    cleaned = re.sub(r"[^a-zA-Z0-9]+", "_", combined)
    return cleaned.strip("_").lower()
```

**Import edge resolution** — per-language handlers:
- Python: resolves relative imports (dots) to full file paths, `import_from_statement` produces `imports_from` edges
- JavaScript/TypeScript: resolves `./relative` paths, normalizes `.js` → `.ts` for ESM
- Java: walks `scoped_identifier` nodes recursively

**Three-layer node deduplication**:
1. Within file: `seen_ids` set in extractor — each node emitted once per file
2. Between files: `nx.Graph.add_node()` is idempotent — last write wins (semantic over AST)
3. Semantic merge: caller deduplicates by label before `build()` call

**ID normalization** for mismatched edges:
```python
def _normalize_id(s: str) -> str:
    cleaned = re.sub(r"[^a-zA-Z0-9]+", "_", s)
    return cleaned.strip("_").lower()

# norm_to_id dict lets edges survive LLM-generated IDs with different punctuation
norm_to_id: dict[str, str] = {_normalize_id(nid): nid for nid in node_set}
```

### 2.4 Extraction Output Schema

```json
{
  "nodes": [
    {
      "id": "unique_stable_string",
      "label": "Human-readable name",
      "source_file": "path/to/file.py",
      "source_location": "L42",
      "file_type": "python"
    }
  ],
  "edges": [
    {
      "source": "node_id_a",
      "target": "node_id_b",
      "relation": "calls|imports|imports_from|contains|method|inherits|semantically_similar_to",
      "confidence": "EXTRACTED|INFERRED|AMBIGUOUS",
      "weight": 1.0
    }
  ],
  "hyperedges": []
}
```

**Confidence tiers** (critical design pattern — AutoDev keeps this):

| Label | Meaning |
|-------|---------|
| `EXTRACTED` | Explicitly stated in source (import statement, direct call) |
| `INFERRED` | Reasonable deduction (call-graph second pass, co-occurrence) |
| `AMBIGUOUS` | Uncertain; flagged for human review in report |

### 2.5 Build Graph (`build.py`)

- Produces `nx.Graph` (undirected) or `nx.DiGraph` (directed) — `directed=True` for direction-aware traversal
- Preserves edge direction even in undirected graphs via `_src`/`_tgt` attributes
- `build_merge()` — load existing `graph.json`, merge new chunks, prune deleted-file nodes, safety check prevents silent shrinkage
- `deduplicate_by_label()` — merges nodes sharing normalized label, rewrites edge references, drops self-loops

### 2.6 Community Detection (`cluster.py`)

**Algorithm**: Leiden (via `graspologic`) with Louvain (NetworkX built-in) fallback.

```python
def cluster(G: nx.Graph) -> dict[int, list[str]]:
    # 1. Separate isolates (handled separately - Leiden drops them)
    # 2. Run Leiden or Louvain on connected subgraph
    # 3. Split oversized communities (> 25% of nodes, min 10) via second Leiden pass
    # 4. Re-index by size descending for deterministic ordering
```

**Cohesion score**: `actual_intra_edges / max_possible_edges` — ratio of how tightly connected a community is.

**Key design**: communities >25% of graph nodes get recursively split. This prevents one "mega-community" dominating the report.

**Windows compatibility**: graspologic emits ANSI escape sequences; graphify suppresses `stdout/stderr` via `contextlib.redirect_stdout(io.StringIO())` to prevent PowerShell 5.1 scroll buffer corruption.

### 2.7 Analysis Engine (`analyze.py`)

**God Nodes**: Top-N most-connected real entities. Excludes:
- File-level hub nodes (labels matching the filename — they accumulate mechanical import edges)
- Concept nodes (empty `source_file`, no file extension)
- AST method stubs (labels starting with `.` and ending with `()`)

**Surprising Connections**: Cross-file edges ranked by composite surprise score:

```python
def _surprise_score(G, u, v, data, node_community, u_source, v_source):
    score = 0
    # 1. Confidence weight: AMBIGUOUS=3, INFERRED=2, EXTRACTED=1
    # 2. Cross file-type bonus: code↔paper or code↔image = +2
    # 3. Cross-repo bonus: different top-level directory = +2
    # 4. Cross-community bonus: Leiden says structurally distant = +1
    # 4b. Semantic similarity bonus: ×1.5 if relation == "semantically_similar_to"
    # 5. Peripheral→hub: low-degree node reaching high-degree = +1
    return score, reasons
```

**Suggested Questions** — generated from graph signals:
1. AMBIGUOUS edges → "What is the exact relationship between X and Y?"
2. Bridge nodes (high betweenness) → "Why does X connect community A to community B?"
3. God nodes with INFERRED edges → "Are the N inferred relationships for X correct?"
4. Isolated nodes (degree ≤ 1) → "What connects X to the rest of the system?"
5. Low-cohesion communities (score < 0.15) → "Should X be split into smaller modules?"

**Graph Diff** (`graph_diff(G_old, G_new)`):
```python
{
  "new_nodes": [...],
  "removed_nodes": [...],
  "new_edges": [...],
  "removed_edges": [...],
  "summary": "3 new nodes, 5 new edges, 1 node removed"
}
```

### 2.8 MCP Server (`serve.py`)

Exposes graph as MCP stdio server with tools:

- `query_graph(question, budget=2000)` — BFS/DFS traversal, returns scored subgraph as text
- `get_node(label)` — node details + neighborhood
- `shortest_path(source, target)` — nx.shortest_path with human-readable output
- `list_communities()` — community summary
- `get_report()` — return GRAPH_REPORT.md content

**BFS scoring**:
```python
def _score_nodes(G, terms):
    # Scores by: term in label (1.0), term in source_file (0.5)
    # Diacritic-insensitive matching
```

**Token budget enforcement**:
```python
def _subgraph_to_text(G, nodes, edges, token_budget=2000):
    char_budget = token_budget * 3  # ~3 chars/token approximation
    # Format: "NODE <label> [src=<file> loc=<location> community=<id>]"
    # Format: "EDGE <src> --<relation> [<confidence>]--> <tgt>"
```

### 2.9 Export System (`export.py`)

- `to_json()` → `graphify-out/graph.json` (networkx node_link format)
- `to_html()` → `graphify-out/graph.html` (D3.js force-directed, dark theme, sidebar, community colors)
- `to_svg()` → `graphify-out/graph.svg` (static SVG, ~5000 node cap)
- `to_canvas()` → Obsidian-compatible vault structure
- `to_wiki()` → markdown wiki from community structure

**HTML visualization** features:
- Dark theme: `#0f0f1a` background, `#1a1a2e` sidebar
- Community-colored nodes (10 colors: `#4E79A7`, `#F28E2B`, `#E15759`, ...)
- Sidebar: search results, node info panel, neighbor links, legend, stats
- Cap: 5,000 nodes max before truncating

### 2.10 Incremental Update System

- `graphify update <path>` — re-extracts code files, no LLM, updates `graph.json`
- `graphify watch <path>` — file watcher, rebuilds on code changes
- `graphify check-update <path>` — checks `needs_update` flag (cron-safe)
- `graphify cluster-only <path>` — re-runs clustering on existing graph.json

**Cache system** (`cache.py`): semantic extraction results cached by file hash. Avoids re-calling LLM for unchanged files.

### 2.11 Multi-Repo Support

- `graphify merge-graphs g1.json g2.json --out merged.json` — compose multiple graphs with `nx.compose_all()`, tags each node with `repo` attribute
- `graphify clone <github-url>` — clones to `~/.graphify/repos/<owner>/<repo>`, reuses if exists

### 2.12 Integration Layer (hooks, CLAUDE.md, AGENTS.md)

graphify integrates with 15+ platforms via platform-specific install:
- **Claude Code**: PreToolUse hook on `Glob|Grep` → injects graph reminder into context; CLAUDE.md section
- **Cursor**: `.cursor/rules/graphify.mdc` with `alwaysApply: true`
- **Gemini**: `GEMINI.md` section + BeforeTool hook in `.gemini/settings.json`
- **OpenCode**: `AGENTS.md` + `.opencode/plugins/graphify.js` (`tool.execute.before`)
- **Codex**: `AGENTS.md` + `.codex/hooks.json` PreToolUse hook
- **Kiro**: `.kiro/skills/graphify/SKILL.md` + `.kiro/steering/graphify.md` (always-on)

For AutoDev specifically, hooks integrate into `AUTODEV.md` and AutoDev's own settings system.

### 2.13 graphify Weaknesses

| Weakness | Impact |
|----------|--------|
| In-memory NetworkX + JSON — no SQLite persistence | Graph must be loaded from disk on every query; slow for large codebases |
| No real-time incremental (file watching rebuilds graph) | Agents can't query mid-edit |
| No TESTED_BY edges | Cannot answer "what tests cover this function?" |
| No blast-radius / impact analysis | Cannot trace "what breaks if I change X?" |
| No execution flow detection | Cannot identify entry points or critical paths |
| No FTS5 search | Keyword search requires graph traversal, not index |
| No multi-repo registry | Cross-repo queries require manual `merge-graphs` |
| Community names are numeric (Community 0, 1, 2) | Poor UX in reports |
| No database migrations | Schema changes break existing graphs |
| No parallel parsing | Sequential extraction on large codebases |

---

## 3. Reference Codebase Analysis: code-review-graph

### 3.1 Overview

code-review-graph is a **FastMCP server** backed by a **SQLite persistent graph** designed specifically for token-efficient code review. Where graphify is navigation-first, code-review-graph is **code-review-first** — its core value proposition is blast-radius analysis and risk-scored change detection.

**Repository**: `graph-references/code-review-graph-main/`  
**Primary module**: `code_review_graph/` package  
**Database**: `.code-review-graph/graph.db` (SQLite, WAL mode)  
**CLI entry**: `code_review_graph/cli.py`  
**MCP server**: `code_review_graph/main.py` (FastMCP, stdio transport)

### 3.2 Architecture

```
CodeParser (tree-sitter) → GraphStore (SQLite) → Analysis Modules → MCP Tools / CLI
                                  ↑
                         IncrementalUpdater (git-diff + hash)
```

Unlike graphify's stateless pipeline, code-review-graph maintains a **persistent database** that is updated incrementally. The MCP server reads from this database at query time. No full rebuild needed for queries.

### 3.3 Parser (`parser.py`)

**Language support**: 19+ languages including several graphify misses:
- Vue SFC (`.vue`) — parsed as TypeScript for script blocks
- Solidity (`.sol`)
- Dart (`.dart`)
- R (`.r`, `.R`)
- Perl (`.pl`, `.pm`, `.t`, `.xs` as C)
- Lua (`.lua`, `.luau`)
- Objective-C (`.m`)
- Bash/Zsh/Ksh (`.sh`, `.bash`, `.zsh`, `.ksh`)
- Elixir (`.ex`, `.exs`)
- Zig (`.zig`)
- PowerShell (`.ps1`, `.psm1`, `.psd1`)
- Svelte (`.svelte`)
- Julia (`.jl`)
- ReScript (`.res`, `.resi`) — regex-based since no tree-sitter grammar
- GDScript (`.gd`)
- Jupyter notebooks (`.ipynb`) — parsed per-cell with language detection
- Databricks notebooks

**Node types**: `File`, `Class`, `Function`, `Type`, `Test`

**Edge types**: `CALLS`, `IMPORTS_FROM`, `INHERITS`, `IMPLEMENTS`, `CONTAINS`, `TESTED_BY`, `DEPENDS_ON`, `REFERENCES`

**NodeInfo dataclass**:
```python
@dataclass
class NodeInfo:
    kind: str           # File, Class, Function, Type, Test
    name: str
    file_path: str
    line_start: int
    line_end: int
    language: str
    parent_name: Optional[str]   # enclosing class/module
    params: Optional[str]
    return_type: Optional[str]
    modifiers: Optional[str]
    is_test: bool
    extra: dict
```

**EdgeInfo dataclass**:
```python
@dataclass
class EdgeInfo:
    kind: str           # CALLS, IMPORTS_FROM, INHERITS, etc.
    source: str         # qualified name or path
    target: str         # qualified name or path
    file_path: str
    line: int
    extra: dict
```

**Parallel parsing**: `ThreadPoolExecutor` with configurable worker count:
```python
_MAX_PARSE_WORKERS = int(os.environ.get("CRG_PARSE_WORKERS", str(min(os.cpu_count() or 4, 8))))
```

### 3.4 Graph Store (`graph.py`)

**SQLite schema** (core tables):

```sql
CREATE TABLE nodes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,           -- File, Class, Function, Type, Test
    name TEXT NOT NULL,
    qualified_name TEXT NOT NULL UNIQUE,
    file_path TEXT NOT NULL,
    line_start INTEGER,
    line_end INTEGER,
    language TEXT,
    parent_name TEXT,
    params TEXT,
    return_type TEXT,
    modifiers TEXT,
    is_test INTEGER DEFAULT 0,
    file_hash TEXT,               -- SHA-256 of file at parse time
    extra TEXT DEFAULT '{}',
    updated_at REAL NOT NULL
);

CREATE TABLE edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    source_qualified TEXT NOT NULL,
    target_qualified TEXT NOT NULL,
    file_path TEXT NOT NULL,
    line INTEGER DEFAULT 0,
    extra TEXT DEFAULT '{}',
    confidence REAL DEFAULT 1.0,
    confidence_tier TEXT DEFAULT 'EXTRACTED',  -- EXTRACTED|INFERRED|AMBIGUOUS
    updated_at REAL NOT NULL
);

CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
```

**Key indexes**:
```sql
CREATE INDEX idx_nodes_file      ON nodes(file_path);
CREATE INDEX idx_nodes_qualified ON nodes(qualified_name);
CREATE INDEX idx_edges_source    ON edges(source_qualified);
CREATE INDEX idx_edges_target    ON edges(target_qualified);
CREATE INDEX idx_edges_target_kind ON edges(target_qualified, kind);
CREATE INDEX idx_edges_source_kind ON edges(source_qualified, kind);
```

**WAL mode + threading**:
```python
self._conn = sqlite3.connect(
    str(self.db_path), timeout=30, check_same_thread=False,
    isolation_level=None  # Disable implicit transactions
)
self._conn.execute("PRAGMA journal_mode=WAL")
```

**Database migrations** (`migrations.py`): versioned schema migrations v1→v5. `get_schema_version()` + `run_migrations()` called on every startup. Enables forward-compatible schema evolution.

### 3.5 Impact Radius (`graph.py` BFS)

**Core capability that graphify lacks**:

```python
def get_impact_radius(
    self, 
    qualified_name: str,
    max_depth: int = MAX_IMPACT_DEPTH,  # default: 5
    max_nodes: int = MAX_IMPACT_NODES   # default: 100
) -> dict[str, Any]:
    # BFS from changed node through:
    # - Callers (CALLS edges in reverse)
    # - Dependents (IMPORTS_FROM, DEPENDS_ON in reverse)  
    # - Tests (TESTED_BY edges)
    # Returns: affected nodes + paths + test coverage gaps
```

**FlowAdjacency** — in-memory adjacency structure for flow tracing:
```python
@dataclass
class FlowAdjacency:
    calls_out: dict[str, list[str]]    # outgoing CALLS
    has_tested_by: set[str]            # nodes with TESTED_BY edges
    nodes_by_qn: dict[str, GraphNode]  # fast lookup by qualified_name
    nodes_by_id: dict[int, GraphNode]  # fast lookup by id
```

### 3.6 Execution Flow Detection (`flows.py`)

**Entry point detection** — framework-aware, covers 15+ frameworks:
```python
_FRAMEWORK_DECORATOR_PATTERNS = [
    # Python web: @app.get, @router.post, @blueprint.route
    # CLI: @click.command, @mygroup.command
    # Tasks: @celery.task, @shared_task
    # Django: @receiver, @api_view, @action
    # Testing: @pytest.fixture, @pytest.mark
    # Java Spring: @GetMapping, @PostMapping, @Scheduled, @Bean
    # JS/TS: @Component, @Injectable, @Controller, @Module
    # AI/agent: @tool, @tool_plain, @system_prompt (pydantic-ai, langchain)
    # Middleware: @middleware, @exception_handler
]

_ENTRY_NAME_PATTERNS = [
    "^main$", "^__main__$", "^test_", "^Test[A-Z]",
    "^on_", "^handle_", "^handler$", "^lambda_handler$",
    "^upgrade$", "^downgrade$",          # Alembic
    "^lifespan$", "^get_db$",            # FastAPI
    "^on(Create|Start|Resume|...)$",     # Android lifecycle
]
```

**Criticality scoring**: each flow gets a score based on:
- Security keywords in the path (auth, token, password, admin, etc.)
- Whether the flow is tested
- Flow length (longer = more complex = higher risk)
- Number of callers

### 3.7 Community Detection (`communities.py`)

**Algorithm**: Leiden via igraph (preferred) with file-based grouping fallback.

**Edge weights** for community detection:
```python
EDGE_WEIGHTS = {
    "CALLS": 1.0,
    "IMPORTS_FROM": 0.5,
    "INHERITS": 0.8,
    "IMPLEMENTS": 0.7,
    "CONTAINS": 0.3,
    "TESTED_BY": 0.4,
    "DEPENDS_ON": 0.6,
}
```

**Community naming** (algorithmic, unlike graphify's numeric IDs):
1. Find most common module/file prefix among members
2. If dominant class exists (>40% of nodes), use its name
3. Most frequent keyword from function/class names
4. Format: `"{prefix}-{keyword}"` (e.g. `"api-auth"`, `"db-models"`)

**Architecture overview** — describes the whole codebase in a compact summary including:
- All communities with member counts
- Dominant language per community
- Key classes/functions per community
- Cross-community coupling matrix

### 3.8 Change Analysis (`changes.py`)

**Git diff parsing**:
```python
def parse_git_diff_ranges(repo_root, base="HEAD~1") -> dict[str, list[tuple[int, int]]]:
    # git diff --unified=0 → changed line ranges per file
    # Validates git ref with regex: r"^[A-Za-z0-9_.~^/@{}\-]+$"
```

**SVN support** (graphify has no SVN):
```python
def parse_svn_diff_ranges(repo_root, rev_range=None)
```

**Risk scoring**: each changed function gets a composite risk score based on:
- Number of callers (blast radius)
- Whether it's in a security-critical flow (auth, crypto, payments)
- Test coverage (no TESTED_BY edges = higher risk)
- Change size (lines changed)
- Is it a public API / entry point?

### 3.9 Incremental Update (`incremental.py`)

**The critical feature that graphify lacks properly**:

1. `git diff HEAD~1` → list of changed files
2. SHA-256 hash check on each file → skip unchanged files
3. Re-parse only changed files + their dependents (via IMPORTS_FROM/DEPENDS_ON edges)
4. Update SQLite in-place — no full rebuild
5. **Result**: 2,900-file project re-indexes in < 2 seconds

**Parallel parsing**:
```python
with concurrent.futures.ThreadPoolExecutor(max_workers=_MAX_PARSE_WORKERS) as executor:
    futures = {executor.submit(parser.parse_file, path): path for path in changed_files}
```

**Ignore patterns**: comprehensive `.gitignore`-style pattern list including:
- `node_modules/**`, `.venv/**`, `vendor/**`, `dist/**`, `build/**`
- `*.min.js`, `*.map`, `*.lock`, `*.db`, `*.pyc`
- Framework-specific: `.next/**`, `target/**`, `.dart_tool/**`, `.pub-cache/**`

**Daemon mode** (`daemon.py`): background process that watches for file changes and updates the graph automatically.

### 3.10 Hybrid Search (`search.py`)

**FTS5 (Full-Text Search)** with BM25 ranking:
```sql
CREATE VIRTUAL TABLE nodes_fts USING fts5(
    name, qualified_name, file_path, signature,
    content='nodes', content_rowid='rowid',
    tokenize='porter unicode61'
)
```

**Query kind boosting**:
- PascalCase query (e.g. `MyClass`) → boost `Class/Type` results by 1.5×
- snake_case query (e.g. `get_users`) → boost `Function` results by 1.5×

**Vector embeddings** (optional, `embeddings.py`):
- Local: `sentence-transformers` (no API cost)
- Cloud: Google Gemini text-embedding-004
- MiniMax

**Reciprocal Rank Fusion (RRF)** combines FTS5 and vector results:
```python
# RRF score = 1 / (rank + k) for each result list
# Merge FTS5 ranks and vector ranks, take top-N
```

### 3.11 Visualization (`visualization.py`)

**D3.js force-directed graph** — dark theme, self-contained HTML:

**Rendering modes**:
- `full` — every node (default; up to practical limits)
- `community` — aggregate by community, double-click to drill down
- `file` — aggregate by file, each file is one node
- `auto` — chooses community mode when node count exceeds threshold

**Node colors by kind**:
- `File` → blue
- `Class` → purple  
- `Function` → green
- `Test` → orange
- `Type` → teal

**Features**:
- HTML escaping via `escH()` on all user content (XSS defense)
- SRI hash on D3.js CDN script tag
- Tooltips with kind, file, line, qualified_name
- Zoom, pan, drag nodes
- Community toggle (click legend = hide/show community)
- Collapsible file clusters

### 3.12 Multi-Repo Registry (`registry.py`)

```python
# ~/.code-review-graph/registry.json
{
  "repos": [
    {"path": "/path/to/repo", "alias": "my-repo"},
    {"path": "/path/to/other-repo", "alias": "api"}
  ]
}
```

**Connection pool**: LRU cache of SQLite connections per repo, thread-safe via `threading.Lock`.

### 3.13 MCP Tool Surface (22 tools)

```
get_minimal_context(task)      → ~100 tokens, full picture upfront
query_graph(target, pattern)   → callers_of, callees_of, imports_of, tests_for, dependencies_of
get_impact_radius(qualified_name) → blast radius of a change
detect_changes(base)           → git diff → risk-scored affected nodes
get_review_context(files)      → token-efficient source snippets for review
get_affected_flows(file)       → execution paths impacted by a change
get_architecture_overview()    → community map + key nodes summary
list_communities()             → community list with stats
semantic_search_nodes(query)   → FTS5 + optional vector search
get_node_details(qname)        → full node info + neighbors
list_callers(qname)            → who calls this function?
list_callees(qname)            → what does this function call?
list_tests(qname)              → which tests cover this?
get_file_summary(file_path)    → all nodes in a file
refactor_tool(action, target)  → rename preview, dead code, suggestions
get_graph_stats()              → node/edge counts, languages, last updated
build_graph()                  → trigger full rebuild
update_graph()                 → trigger incremental update
get_communities()              → community details
get_execution_flows()          → all detected execution flows
get_flow_criticality()         → flow risk scores
find_dead_code()               → unreachable functions/classes
```

**5 MCP Prompts**:
- `review_changes` — code review workflow
- `architecture_map` — understand high-level structure
- `debug_issue` — trace a bug through the graph
- `onboard_developer` — explain the codebase to a new developer
- `pre_merge_check` — risk assessment before merging

### 3.14 code-review-graph Weaknesses

| Weakness | Impact |
|----------|--------|
| No god-node analysis | Cannot identify the most central architectural abstractions |
| No surprising connections report | Misses non-obvious cross-module couplings |
| No suggested questions generation | No automated knowledge-gap discovery |
| Community names are algorithmic (may miss intent) | Less meaningful than LLM-labeled names |
| No multi-graph merge | Cross-repo queries require separate registry approach |
| No `path "A" "B"` CLI traversal | Cannot find shortest dependency path via CLI |
| No `explain "X"` command | Must use MCP tools, not CLI |
| No confidence-level filtering | Cannot filter to AMBIGUOUS edges only |
| Less polished graph narrative (GRAPH_REPORT.md) | Report is less insight-driven |
| No graph diff visualization | Cannot show what changed between builds |

---

## 4. Head-to-Head Comparison

| Capability | graphify-5 | code-review-graph | Winner |
|-----------|-----------|-------------------|--------|
| **Storage** | In-memory + JSON files | SQLite (WAL mode, indexes, migrations) | code-review-graph |
| **Query speed** | Load JSON + traverse in-memory | Index-optimized SQL queries | code-review-graph |
| **Persistence** | Manual load from disk | Always-on persistent DB | code-review-graph |
| **Incremental updates** | Full rebuild or `graphify update` | SHA-256 hash, < 2s on 2900 files | code-review-graph |
| **Language support** | ~12 languages | 19+ languages + notebooks, Vue, Solidity | code-review-graph |
| **TESTED_BY edges** | ❌ No | ✅ Yes | code-review-graph |
| **Blast-radius analysis** | ❌ No | ✅ Yes | code-review-graph |
| **Execution flows** | ❌ No | ✅ Yes (framework-aware) | code-review-graph |
| **Risk-scored changes** | ❌ No | ✅ Yes (git diff → risk) | code-review-graph |
| **FTS5 hybrid search** | ❌ No | ✅ Yes (BM25 + vectors) | code-review-graph |
| **Vector embeddings** | ❌ No | ✅ Optional (local or cloud) | code-review-graph |
| **Multi-repo registry** | Manual merge-graphs | ~/.code-review-graph/registry.json | code-review-graph |
| **Parallel parsing** | ❌ Sequential | ✅ ThreadPoolExecutor | code-review-graph |
| **Schema migrations** | ❌ No | ✅ v1→v5 | code-review-graph |
| **God nodes analysis** | ✅ Yes | ❌ No | graphify |
| **Surprising connections** | ✅ Composite scored | ❌ No | graphify |
| **Suggested questions** | ✅ 5 signal types | ❌ No | graphify |
| **Graph diff** | ✅ `graph_diff()` | ❌ No | graphify |
| **Multi-graph merge** | ✅ `merge-graphs` | ❌ No | graphify |
| **CLI traversal** | ✅ `path A B`, `explain X`, `query "..."` | ❌ Must use MCP | graphify |
| **Confidence tiers** | ✅ EXTRACTED/INFERRED/AMBIGUOUS | ✅ Same | tie |
| **Community detection** | ✅ Leiden (named by id) | ✅ Leiden (named by content) | code-review-graph |
| **D3.js visualization** | ✅ Basic force-directed | ✅ Dark theme + modes + drilldown | code-review-graph |
| **MCP server** | ✅ Basic (5-6 tools) | ✅ Rich (22 tools + 5 prompts) | code-review-graph |
| **Daemon mode** | ❌ No | ✅ Yes | code-review-graph |
| **SVN support** | ❌ No | ✅ Yes | code-review-graph |
| **GRAPH_REPORT.md** | ✅ Narrative-rich | ❌ No equivalent | graphify |
| **Refactoring tools** | ❌ No | ✅ Rename, dead code | code-review-graph |

**Verdict**: code-review-graph wins on persistence, performance, and analysis depth. graphify wins on insight generation, graph narratives, and flexible traversal.

---

## 5. AutoDev Graph Architecture (Combined Best)

AutoDev takes the best of both worlds and adds a third dimension: **worker-awareness**.

### 5.1 Design Principles

1. **SQLite-persistent** (code-review-graph style) — no full reload on every query
2. **Pipeline-clean** (graphify style) — each stage is a pure function, testable in isolation
3. **Worker-annotated** (AutoDev-native) — every node/edge carries the agent that created it
4. **MCP-first** (code-review-graph style) — 20+ tools with `next_tool_suggestions`
5. **Analysis-rich** (graphify style) — god nodes, surprising connections, suggested questions
6. **Insight-generating** (graphify style) — GRAPH_REPORT.md with narrative analysis
7. **Rust-native extraction** (AutoDev-native) — tree-sitter Rust bindings, no Python subprocess
8. **React/D3 in the Graphify Tab** (AutoDev-native) — interactive Tauri UI, click-to-IDE

### 5.2 AutoDev Graph Stack

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        AUTODEV GRAPHIFY TAB (React)                     │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  D3.js Force-Directed Graph (dark theme, interactive)            │   │
│  │  Node click → jump to IDE Tab at file:line                       │   │
│  │  Community filter, zoom/pan, edge type filter                    │   │
│  │  Rendering modes: full | community | file | auto                 │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│  ┌──────────────┐  ┌────────────────────────┐  ┌──────────────────┐   │
│  │ Graph Report │  │ Impact Radius Panel    │  │ Graph Diff Panel │   │
│  │ (narrative)  │  │ blast radius view       │  │ before/after     │   │
│  └──────────────┘  └────────────────────────┘  └──────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
                                    │ Tauri invoke()
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                     AUTODEV GRAPH ENGINE (Rust)                         │
│  ┌──────────────────────────────────────────────────────────────┐      │
│  │  Extraction Layer                                            │      │
│  │  • tree-sitter-rust bindings (no Python subprocess)          │      │
│  │  • 20+ language configs (LanguageConfig struct)              │      │
│  │  • Parallel parsing (Rayon or Tokio thread pool)             │      │
│  └──────────────────────────────────────────────────────────────┘      │
│  ┌──────────────────────────────────────────────────────────────┐      │
│  │  Build + Dedup Layer                                          │      │
│  │  • ID normalization (same algorithm as graphify)             │      │
│  │  • 3-layer deduplication                                     │      │
│  │  • Directed edge preservation (_src/_tgt)                    │      │
│  └──────────────────────────────────────────────────────────────┘      │
│  ┌──────────────────────────────────────────────────────────────┐      │
│  │  SQLite Persistence (rusqlite)                                │      │
│  │  • WAL mode, indexed schema                                  │      │
│  │  • Schema migrations (v1→vN)                                 │      │
│  │  • Worker attribution columns                                │      │
│  └──────────────────────────────────────────────────────────────┘      │
│  ┌──────────────────────────────────────────────────────────────┐      │
│  │  Analysis Layer                                              │      │
│  │  • God nodes (degree-based, excludes hub/concept nodes)      │      │
│  │  • Surprising connections (composite surprise score)         │      │
│  │  • Suggested questions (5 signal types)                      │      │
│  │  • Leiden community detection                                │      │
│  │  • Execution flow tracing + criticality                       │      │
│  │  • Blast-radius impact analysis                              │      │
│  │  • Graph diff (snapshots before/after)                       │      │
│  └──────────────────────────────────────────────────────────────┘      │
│  ┌──────────────────────────────────────────────────────────────┐      │
│  │  Query Layer                                                  │      │
│  │  • BFS/DFS traversal with token budget                       │      │
│  │  • Shortest path (weighted)                                  │      │
│  │  • FTS5 keyword search                                        │      │
│  │  • Subgraph extraction for system prompt injection           │      │
│  └──────────────────────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    │                               │
        ┌───────────▼────────────┐    ┌────────────▼──────────┐
        │  MCP Server (stdio)    │    │  Tauri Command API    │
        │  20+ graph tools       │    │  For Graphify Tab UI  │
        │  5 MCP prompts         │    │  and Worker agents    │
        └────────────────────────┘    └───────────────────────┘
```

### 5.3 Key Innovations Over Both References

| Innovation | Description |
|-----------|-------------|
| **Worker attribution** | Every node/edge stores `agent_author` (worker-backend, pa, etc.) |
| **Task annotation** | `task_id` on edges — which task created this relationship |
| **Lock-file awareness** | Nodes in `current_tasks/*.lock` files marked as `worker_locked: true` |
| **Worktree-scoped graphs** | Each worker's worktree has its own in-flight graph, merged on PR |
| **Real-time updates** | Tauri file watcher triggers incremental update as workers write files |
| **Click-to-IDE** | Tauri event: click node → IDE tab opens file at line |
| **PA context injection** | Relevant subgraph injected into system prompt via `build_system_prompt()` |
| **QC integration** | QC agent uses `detect_changes` + `get_impact_radius` for PR review |
| **Cross-repo by default** | Multi-repo registry built-in for all AIRS-Foundry repos |
| **AUTODEV.md reflection** | Graph infers AUTODEV.md instructions from patterns it finds |

---

## 6. Data Model: Nodes and Edges

### 6.1 Node Kinds

| Kind | Description | Example |
|------|-------------|---------|
| `File` | Source file hub node | `src/api/health.py` |
| `Class` | Class definition | `LivingAgent` |
| `Function` | Function or method | `process_message()` |
| `Type` | Type alias, interface, enum | `AgentConfig` |
| `Test` | Test function or class | `test_health_endpoint()` |
| `Module` | Package/module (Python __init__, etc.) | `src.api` |
| `Import` | External import (stdlib, npm) | `fastapi` |

### 6.2 Edge Kinds

| Kind | Direction | Description | Weight |
|------|-----------|-------------|--------|
| `CALLS` | A → B | Function A calls function B | 1.0 |
| `IMPORTS_FROM` | A → B | File/module A imports from B | 0.5 |
| `IMPORTS` | A → B | A imports external module B | 0.3 |
| `INHERITS` | A → B | Class A inherits from class B | 0.8 |
| `IMPLEMENTS` | A → B | Class A implements interface B | 0.7 |
| `CONTAINS` | A → B | File/class A contains function B | 0.3 |
| `TESTED_BY` | A → B | Function A is tested by test function B | 0.4 |
| `DEPENDS_ON` | A → B | Module A depends on module B | 0.6 |
| `REFERENCES` | A → B | A references B (type annotation, etc.) | 0.4 |
| `SEMANTICALLY_SIMILAR` | A ↔ B | A and B are conceptually related | 0.5 |

### 6.3 Confidence Tiers

| Tier | Meaning | Source |
|------|---------|--------|
| `EXTRACTED` | Explicitly in source (import statement, direct call) | AST extractor |
| `INFERRED` | Reasonable deduction (call-graph second pass, co-occurrence) | Analysis pass |
| `AMBIGUOUS` | Uncertain — flagged for human review | LLM extraction or heuristic |

### 6.4 AutoDev-Specific Node Attributes

```json
{
  "id": "stable_unique_id",
  "label": "function_name()",
  "source_file": "src/api/health.py",
  "source_location": "L42-L67",
  "file_type": "python",
  "language": "python",
  "kind": "Function",
  "qualified_name": "src/api/health.py::agents_health",
  "params": "(request: Request)",
  "return_type": "JSONResponse",
  "is_test": false,
  "file_hash": "sha256:abc123...",
  "community": 3,
  "community_name": "api-health",
  "agent_author": "worker-backend",
  "task_id": 142,
  "worker_locked": false,
  "worktree": null,
  "updated_at": 1745000000.0
}
```

### 6.5 AutoDev-Specific Edge Attributes

```json
{
  "source": "src/api/health.py::agents_health",
  "target": "src/services/agent_service.py::AgentService",
  "kind": "CALLS",
  "confidence": "EXTRACTED",
  "confidence_tier": "EXTRACTED",
  "weight": 1.0,
  "source_location": "L58",
  "agent_author": "worker-backend",
  "task_id": 142,
  "updated_at": 1745000000.0
}
```

---

## 7. Pipeline: The Seven Stages

### Stage 1: Detect

**Input**: root directory path  
**Output**: filtered list of source file paths

```rust
// src-tauri/src/graph/detect.rs
pub fn collect_files(root: &Path, ignore_patterns: &[&str]) -> Vec<PathBuf> {
    // Walk directory tree
    // Apply .gitignore patterns + default ignores
    // Filter by CODE_EXTENSIONS
    // Skip: node_modules, .venv, build, dist, .git, __pycache__
}
```

**Extension whitelist** (combined from both references):
- Python: `.py`
- JavaScript: `.js`, `.mjs`, `.jsx`
- TypeScript: `.ts`, `.tsx`
- Go: `.go`
- Rust: `.rs`
- Java: `.java`
- C/C++: `.c`, `.h`, `.cpp`, `.cc`, `.hpp`, `.cxx`
- C#: `.cs`
- Ruby: `.rb`
- PHP: `.php`
- Kotlin: `.kt`
- Swift: `.swift`
- Scala: `.scala`
- Vue: `.vue`
- Svelte: `.svelte`
- Dart: `.dart`
- R: `.r`, `.R`
- Lua: `.lua`, `.luau`
- Solidity: `.sol`
- Elixir: `.ex`, `.exs`
- Zig: `.zig`
- Julia: `.jl`
- Bash: `.sh`, `.bash`, `.zsh`
- PowerShell: `.ps1`, `.psm1`
- Objective-C: `.m`
- Perl: `.pl`, `.pm`
- GDScript: `.gd`
- ReScript: `.res`, `.resi`
- Notebooks: `.ipynb`

### Stage 2: Extract

**Input**: file path  
**Output**: `ExtractionResult { nodes: Vec<NodeInfo>, edges: Vec<EdgeInfo> }`

```rust
// src-tauri/src/graph/extract.rs
pub fn extract(path: &Path) -> Result<ExtractionResult> {
    let language = detect_language(path)?;
    let source = fs::read(path)?;
    let file_hash = sha256(&source);
    
    // Check cache: if hash matches last parse, return cached result
    if let Some(cached) = cache::load(path, &file_hash) {
        return Ok(cached);
    }
    
    let parser = get_parser(language)?;
    let tree = parser.parse(&source, None)?;
    let result = walk_tree(&tree, &source, path, language);
    
    cache::save(path, &file_hash, &result);
    Ok(result)
}
```

**Two-pass extraction**:
1. **AST pass**: Walk tree-sitter nodes, emit `EXTRACTED` edges (direct calls, import statements)
2. **Inference pass**: Scan function bodies for call patterns, emit `INFERRED` edges

### Stage 3: Build

**Input**: `Vec<ExtractionResult>`  
**Output**: in-memory graph structure (for analysis) + SQLite write

**Deduplication sequence**:
1. Within-file: `seen_ids` HashSet in extractor
2. Label normalization: merge nodes with same normalized label (`[^a-z0-9 ]` stripped)
3. ID normalization: remap edge endpoints using `[^a-zA-Z0-9]+` → `_` normalization

**SQLite upsert** (conflict on `qualified_name`):
```sql
INSERT INTO nodes (kind, name, qualified_name, file_path, ...)
VALUES (?, ?, ?, ?, ...)
ON CONFLICT(qualified_name) DO UPDATE SET
    file_hash = excluded.file_hash,
    line_start = excluded.line_start,
    updated_at = excluded.updated_at;
```

### Stage 4: Cluster

**Input**: graph (NetworkX or adjacency from SQLite)  
**Output**: `HashMap<usize, Vec<String>>` — community_id → node_ids

```rust
// src-tauri/src/graph/cluster.rs
// Calls petgraph-based Leiden implementation
// OR: delegates to Python core-orchestrator via HTTP for heavy analysis
pub fn cluster(adjacency: &Adjacency) -> HashMap<usize, Vec<String>> {
    // Option A: Rust-native Louvain (fast, good quality)
    // Option B: HTTP call to core-orchestrator /api/v1/graph/cluster (full Leiden)
}
```

**Community naming** (algorithmic, content-aware):
1. Find most common directory prefix among members
2. Check for dominant class (>40%)
3. Most frequent non-trivial keyword from names
4. Format: `"{dir}-{keyword}"` (e.g. `"api-health"`)

**Community splitting**: communities >25% of total nodes re-clustered recursively.

### Stage 5: Analyze

**Input**: graph + communities  
**Output**: analysis report (god nodes, surprises, questions, flows)

Five analysis passes run in parallel:
1. **God nodes** — top-10 by degree, excludes hub/stub/concept nodes
2. **Surprising connections** — composite surprise score (confidence × cross-file × cross-community × peripheral→hub)
3. **Suggested questions** — 5 signal types (AMBIGUOUS edges, bridge nodes, INFERRED god nodes, isolated nodes, low cohesion)
4. **Execution flows** — entry point detection + forward BFS + criticality scoring
5. **Impact radius** — pre-computed for all high-degree nodes

### Stage 6: Report

**Input**: analysis results  
**Output**: `GRAPH_REPORT.md` — narrative markdown document

Structure:
```markdown
# GRAPH_REPORT.md — {repo_name}

## Graph Statistics
- Nodes: {N} | Edges: {E} | Languages: {langs}

## God Nodes (Core Abstractions)
| Node | Degree | Community | File |
|------|--------|-----------|------|
...

## Community Structure
### Community 0: api-health ({N} nodes, cohesion: 0.42)
...

## Surprising Connections
...

## Suggested Questions
...

## Knowledge Gaps
...
```

### Stage 7: Export

**Input**: graph + communities + analysis  
**Output**: multiple export formats

| Format | Path | Purpose |
|--------|------|---------|
| `graph.json` | `graphify-out/graph.json` | MCP server source, merge-graphs input |
| `graph.html` | `graphify-out/graph.html` | Standalone interactive visualization |
| `graph.svg` | `graphify-out/graph.svg` | Static image for docs |
| `GRAPH_REPORT.md` | `graphify-out/GRAPH_REPORT.md` | Narrative report for PA/workers |
| SQLite DB | `.autodev/graph.db` | Persistent store for incremental queries |

---

## 8. Storage Layer: SQLite Schema

Complete schema for AutoDev's graph database:

```sql
-- Version tracking for schema migrations
CREATE TABLE schema_version (version INTEGER NOT NULL);

-- Core nodes table
CREATE TABLE nodes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    qualified_name TEXT NOT NULL UNIQUE,
    file_path TEXT NOT NULL,
    line_start INTEGER,
    line_end INTEGER,
    language TEXT,
    parent_name TEXT,
    params TEXT,
    return_type TEXT,
    modifiers TEXT,
    is_test INTEGER DEFAULT 0,
    file_hash TEXT,
    community INTEGER,
    community_name TEXT,
    -- AutoDev extensions
    agent_author TEXT,            -- 'pa', 'worker-backend', 'worker-frontend', etc.
    task_id INTEGER,              -- Internal task board ID
    worker_locked INTEGER DEFAULT 0,
    worktree TEXT,                -- git worktree path if node is in-flight
    extra TEXT DEFAULT '{}',
    updated_at REAL NOT NULL
);

-- Core edges table
CREATE TABLE edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    source_qualified TEXT NOT NULL,
    target_qualified TEXT NOT NULL,
    file_path TEXT NOT NULL,
    line INTEGER DEFAULT 0,
    confidence REAL DEFAULT 1.0,
    confidence_tier TEXT DEFAULT 'EXTRACTED',
    weight REAL DEFAULT 1.0,
    relation TEXT,                -- graphify-style: 'calls', 'imports', etc.
    -- AutoDev extensions
    agent_author TEXT,
    task_id INTEGER,
    extra TEXT DEFAULT '{}',
    updated_at REAL NOT NULL
);

-- Community table (persisted results of clustering)
CREATE TABLE communities (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    cohesion_score REAL,
    member_count INTEGER,
    dominant_language TEXT,
    updated_at REAL NOT NULL
);

-- Execution flows table
CREATE TABLE flows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entry_point TEXT NOT NULL,    -- qualified_name of entry function
    flow_path TEXT NOT NULL,      -- JSON array of qualified_names in order
    criticality_score REAL,
    is_tested INTEGER DEFAULT 0,
    has_security_keyword INTEGER DEFAULT 0,
    updated_at REAL NOT NULL
);

-- Graph snapshots for graph_diff
CREATE TABLE snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    label TEXT NOT NULL,          -- 'before_task_142', 'after_pr_87', etc.
    node_count INTEGER,
    edge_count INTEGER,
    graph_json TEXT,              -- compressed JSON of the snapshot
    created_at REAL NOT NULL
);

-- Analysis cache (god nodes, surprises, questions)
CREATE TABLE analysis_cache (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,          -- JSON
    graph_hash TEXT,              -- invalidated when graph changes
    updated_at REAL NOT NULL
);

-- FTS5 virtual table for keyword search
CREATE VIRTUAL TABLE nodes_fts USING fts5(
    name, qualified_name, file_path, params, return_type,
    content='nodes', content_rowid='rowid',
    tokenize='porter unicode61'
);

-- Multi-repo registry
CREATE TABLE repos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    alias TEXT,
    db_path TEXT NOT NULL,
    last_updated REAL,
    node_count INTEGER,
    edge_count INTEGER
);

-- Indexes
CREATE INDEX idx_nodes_file      ON nodes(file_path);
CREATE INDEX idx_nodes_kind      ON nodes(kind);
CREATE INDEX idx_nodes_qualified ON nodes(qualified_name);
CREATE INDEX idx_nodes_community ON nodes(community);
CREATE INDEX idx_nodes_task      ON nodes(task_id);
CREATE INDEX idx_nodes_author    ON nodes(agent_author);
CREATE INDEX idx_edges_source    ON edges(source_qualified);
CREATE INDEX idx_edges_target    ON edges(target_qualified);
CREATE INDEX idx_edges_kind      ON edges(kind);
CREATE INDEX idx_edges_target_kind ON edges(target_qualified, kind);
CREATE INDEX idx_edges_source_kind ON edges(source_qualified, kind);
CREATE INDEX idx_edges_file      ON edges(file_path);
CREATE INDEX idx_edges_tier      ON edges(confidence_tier);
```

---

## 9. Analysis Engine

### 9.1 God Node Detection

```
Algorithm:
1. Compute degree for all nodes
2. Sort by degree descending
3. Skip: file hub nodes (label == filename), concept nodes (no source_file),
         AST stubs (label starts with "." and ends with "()")
4. Return top-10 real entities

Output: [{ id, label, degree, community, file }]
```

**Why degree matters**: high-degree nodes are the architectural abstractions that everything else depends on. Changing them has the highest blast radius.

### 9.2 Surprising Connection Scoring

```
Composite surprise score for edge (u, v):

base = {AMBIGUOUS: 3, INFERRED: 2, EXTRACTED: 1}[confidence]
+2 if u.file_type ≠ v.file_type (code↔doc, code↔notebook)
+2 if top_level_dir(u.source_file) ≠ top_level_dir(v.source_file)  (cross-repo)
+1 if community(u) ≠ community(v)  (cross-community)
×1.5 if relation == "SEMANTICALLY_SIMILAR"
+1 if min(degree(u), degree(v)) ≤ 2 AND max(degree(u), degree(v)) ≥ 5  (peripheral→hub)
```

### 9.3 Community Cohesion Score

```
cohesion = actual_intra_edges / (n × (n-1) / 2)

Where n = number of nodes in community.
Score of 1.0 = fully connected (clique).
Score < 0.15 = weakly connected = candidate for splitting.
```

### 9.4 Blast Radius

```
For a changed node X:
1. BFS backwards through CALLS edges → find all callers (depth-limited)
2. BFS backwards through IMPORTS_FROM, DEPENDS_ON → find all dependents
3. Collect all TESTED_BY edges from affected nodes → test coverage
4. Classify each affected node by risk: high (≤2 hops), medium (3-4 hops), low (5+ hops)
5. Identify test coverage gaps: affected nodes with no TESTED_BY edges

Output:
{
  "changed_node": "src/api/health.py::agents_health",
  "affected_nodes": [{ "node", "hops", "risk_level", "path" }],
  "tests_to_run": ["test_health_endpoint", "test_agent_service"],
  "test_coverage_gaps": ["src/api/health.py::HealthResponse"],
  "total_affected": 12,
  "high_risk_count": 3
}
```

### 9.5 Execution Flow Criticality

```
For each detected entry point (framework decorator or name pattern):
1. Forward BFS through CALLS edges to trace the full execution path
2. Score:
   base = 1.0
   × 1.5 if any node in path has security keyword (auth, token, password, admin, crypto)
   × 0.7 if flow has TESTED_BY coverage
   × (1 + log(path_length) / 10)  -- longer flows are more complex

Criticality tiers:
   > 1.5 = Critical
   > 1.0 = High
   > 0.7 = Medium
   else  = Low
```

### 9.6 Graph Diff

```
diff(G_before, G_after):
  new_nodes     = G_after.nodes - G_before.nodes
  removed_nodes = G_before.nodes - G_after.nodes
  new_edges     = G_after.edges - G_before.edges  (keyed by source+target+kind)
  removed_edges = G_before.edges - G_after.edges
  
  summary = "N new nodes, M new edges, P nodes removed, Q edges removed"
```

Used by: Workers after completing a task (snapshot before/after to show what they added), QC agent for PR review context.

---

## 10. MCP Tool Surface

AutoDev exposes the graph via MCP with 22 tools (matching code-review-graph's richness) plus AutoDev-native tools:

### 10.1 Discovery Tools

| Tool | Description | Returns |
|------|-------------|---------|
| `get_minimal_context(task)` | ~100 tokens, full picture upfront | god_nodes + relevant communities + suggested_tools |
| `get_architecture_overview()` | Community map + key nodes | communities + cross-coupling matrix |
| `get_graph_stats()` | Node/edge counts, languages, last update | GraphStats |
| `list_communities()` | All communities with stats | [CommunityInfo] |
| `get_graph_report()` | Full GRAPH_REPORT.md content | markdown string |

### 10.2 Navigation Tools

| Tool | Description | Parameters |
|------|-------------|------------|
| `query_graph(target, pattern)` | Trace relationships | target: qname, pattern: callers_of|callees_of|imports_of|tests_for|dependencies_of |
| `semantic_search_nodes(query, top_k)` | FTS5 keyword search | query string |
| `get_node_details(qualified_name)` | Full node info + neighbors | qname string |
| `find_path(source, target)` | Shortest dependency path | two qnames or labels |
| `explain_node(label)` | Node summary + all edges | label string |
| `get_file_summary(file_path)` | All nodes in a file | file path |
| `list_callers(qualified_name)` | Who calls this? | qname |
| `list_callees(qualified_name)` | What does this call? | qname |
| `list_tests(qualified_name)` | Which tests cover this? | qname |

### 10.3 Impact Analysis Tools

| Tool | Description | Parameters |
|------|-------------|------------|
| `get_impact_radius(qualified_name, depth)` | Blast radius of a change | qname, max_depth=5 |
| `detect_changes(base)` | Git diff → risk-scored affected nodes | git ref, default HEAD~1 |
| `get_review_context(files)` | Token-efficient source snippets for review | [file_path] |
| `get_affected_flows(file_path)` | Execution paths impacted | file path |
| `get_flow_criticality()` | All flows with risk scores | — |

### 10.4 Analysis Tools

| Tool | Description | Returns |
|------|-------------|---------|
| `get_god_nodes(top_n)` | Most-connected real entities | [GodNode] |
| `get_surprising_connections(top_n)` | Non-obvious cross-module edges | [SurprisingConnection] |
| `get_suggested_questions()` | Knowledge-gap questions from graph | [Question] |
| `get_graph_diff(snapshot_a, snapshot_b)` | What changed between two snapshots | GraphDiff |

### 10.5 Refactoring Tools

| Tool | Description |
|------|-------------|
| `refactor_tool(action, target)` | Rename preview, find dead code, suggest splits |
| `find_dead_code()` | Functions/classes with no callers or importers |

### 10.6 AutoDev-Native Tools

| Tool | Description |
|------|-------------|
| `get_worker_graph(worker_name)` | All nodes authored by a specific worker |
| `get_task_graph(task_id)` | All nodes and edges associated with a task |
| `get_locked_nodes()` | Nodes currently locked by workers |
| `get_worktree_diff(worktree_path)` | In-flight changes in a worker's worktree |
| `annotate_for_worker(task_id, worker)` | Mark subgraph context for a worker's system prompt |

### 10.7 MCP Prompts (5)

| Prompt | Trigger | Description |
|--------|---------|-------------|
| `review_changes` | QC agent | Full code review workflow using graph context |
| `architecture_map` | PA or any agent | Explain high-level codebase structure |
| `debug_issue` | Worker | Trace a bug through the call graph |
| `onboard_developer` | PA | Explain codebase to new team member |
| `pre_merge_check` | QC agent | Risk assessment before merging PR |

---

## 11. Visualization: The Graphify Tab

### 11.1 Tab Layout

```
┌─── Graphify ─────────────────────────────────────────────────────────────┐
│  [core-orchestrator ▼] [Build Graph] [Update] [Export ▼]    🟢 5,247 nodes│
│  Render: [Auto ▼]  Filter: [All ▼]  Community: [All ▼]                    │
│  ─────────────────────────────────────────────────────────────────────   │
│  ┌─────────────────────────────────────────────────────┐  ┌───────────┐  │
│  │                                                     │  │ Node Info │  │
│  │          D3.js FORCE-DIRECTED GRAPH                 │  │           │  │
│  │                                                     │  │ Name:     │  │
│  │   [nodes colored by community]                      │  │ Kind:     │  │
│  │   [edges colored by type]                           │  │ File:     │  │
│  │   [click node = select]                             │  │ Line:     │  │
│  │   [double-click = zoom to node]                     │  │ Community:│  │
│  │   [drag = reposition]                               │  │           │  │
│  │   [scroll = zoom]                                   │  │ Callers:  │  │
│  │                                                     │  │ • foo()   │  │
│  └─────────────────────────────────────────────────────┘  │ • bar()   │  │
│                                                             │           │  │
│  ┌───────────────────────────────────────────────────────┐ │ [Open in │  │
│  │ GRAPH REPORT              IMPACT RADIUS   GRAPH DIFF  │ │  IDE Tab]│  │
│  │ [collapsible panel: god nodes, surprises, questions]  │ └───────────┘  │
│  └───────────────────────────────────────────────────────┘                │
└────────────────────────────────────────────────────────────────────────────┘
```

### 11.2 Rendering Modes

| Mode | When | Description |
|------|------|-------------|
| `full` | < 500 nodes | Every node as a circle |
| `file` | 500–2000 nodes | Aggregate by file; each file is one node |
| `community` | 2000–10000 nodes | Aggregate by community; double-click to expand |
| `auto` | always | Automatically picks based on node count |

### 11.3 Node Visual Encoding

```
Color by community (10 palette colors, cycling)
Size by degree (min 4px, max 20px, log scale)
Border by kind:
  Function → thin border
  Class → thick border
  Test → dashed border
  File → no border (hub node)
Opacity by confidence:
  EXTRACTED → 1.0
  INFERRED → 0.7
  AMBIGUOUS → 0.4
```

### 11.4 Edge Visual Encoding

```
Color by kind:
  CALLS → #00FF88 (AIRS green)
  IMPORTS_FROM → #4E79A7 (blue)
  INHERITS → #B07AA1 (purple)
  TESTED_BY → #59A14F (green)
  CONTAINS → #666666 (grey)
  SEMANTICALLY_SIMILAR → #EDC948 (yellow)

Opacity by confidence:
  EXTRACTED → 0.7
  INFERRED → 0.4
  AMBIGUOUS → 0.2

Thickness by weight (min 0.5px, max 3px)
```

### 11.5 Click-to-IDE Integration

When user clicks a node, Tauri fires an event:
```typescript
// Graphify tab React component
onClick={(node) => {
  invoke('open_in_ide', {
    file: node.source_file,
    line: node.line_start,
    repo: currentRepo
  });
}}

// Tauri command handler
#[tauri::command]
async fn open_in_ide(file: String, line: u32, repo: String, state: ...) {
    // Switch to IDE tab
    // Set repo selector to repo
    // Navigate file tree to file
    // Scroll code view to line
}
```

---

## 12. Incremental Updates

AutoDev uses code-review-graph's SHA-256 hash approach with Tauri file watcher integration:

### 12.1 Update Triggers

| Trigger | Source | Action |
|---------|--------|--------|
| Worker writes file | Tauri FSEvents watcher | Incremental update for that file |
| Git commit hook | post-commit shell hook | Incremental update for changed files |
| `graphify update` CLI | User command | Full re-extract of code files |
| Worker completes task | Tauri event from recipe | Snapshot + incremental merge |
| QC requests review | QC recipe | `detect_changes` from PR branch |

### 12.2 Update Algorithm

```
1. Hash all files in changed set (SHA-256)
2. Compare to stored file_hash in nodes table
3. For each changed file:
   a. Delete old nodes + edges with source_qualified LIKE 'changed_file%'
   b. Parse changed file → new NodeInfo/EdgeInfo
   c. Upsert nodes (ON CONFLICT qualified_name DO UPDATE)
   d. Upsert edges
4. Find dependents:
   SELECT DISTINCT file_path FROM edges WHERE target_qualified IN (changed_qnames)
5. Repeat steps 3a-3d for dependent files
6. Rebuild FTS5 index for changed nodes
7. Invalidate analysis_cache (update graph_hash)
8. Emit Tauri event: "graph:updated" {repo, changed_count, elapsed_ms}
```

### 12.3 Performance Targets (from code-review-graph benchmarks)

| Repo Size | Full Build | Incremental Update |
|-----------|-----------|-------------------|
| 100 files | < 1s | < 0.1s |
| 500 files | ~5s | < 0.5s |
| 2,900 files | ~30s | < 2s |
| 10,000 files | ~90s | < 5s |

---

## 13. Worker Integration

This is AutoDev's unique contribution to the graph concept.

### 13.1 Worker-as-Graph-Author

Every node/edge written by a worker carries attribution:

```sql
-- When Worker Backend implements task #142:
UPDATE nodes SET agent_author='worker-backend', task_id=142
WHERE file_path LIKE '%/src/api/health.py%'
AND updated_at > task_start_time;
```

This enables queries like:
- "What did Worker Backend add in this task?" → `get_worker_graph('worker-backend')`
- "What nodes are currently locked?" → `get_locked_nodes()`
- "Show me everything task #142 touched" → `get_task_graph(142)`

### 13.2 Worktree-Scoped Graphs

Each worker operates in its own git worktree: `.autodev/worktrees/worker+backend+142/`

The graph has two layers:
1. **Main graph** (`.autodev/graph.db`): stable, merged from all completed tasks
2. **Worktree delta** (in-memory or temp DB): in-flight changes from active worker

When worker completes:
1. Take snapshot of main graph (`before_task_142`)
2. Merge worktree nodes/edges into main graph
3. Take second snapshot (`after_task_142`)
4. Store `graph_diff(before, after)` as task metadata
5. Show diff in QC agent's review context

### 13.3 PA Context Injection

`context.rs` `build_system_prompt()` injects relevant graph subgraph:

```rust
pub fn build_graph_context(query: &str, repo: &str) -> String {
    // 1. Score nodes against query terms (graphify _score_nodes algorithm)
    // 2. BFS from top-5 scored nodes, depth=2
    // 3. Format as: "NODE <label> [src=<file> community=<name>]"
    //               "EDGE <src> --<kind> [<confidence>]--> <tgt>"
    // 4. Cap at 2000 tokens (char_budget = 6000)
    // 5. Prepend to system prompt in PA mode
}
```

Workers get graph context via their recipe's system prompt:
```yaml
# worker-backend-poll.yaml
instructions: |
  You are the AIRS Backend Worker.
  Before starting task, call get_minimal_context(task="<task_title>")
  to understand which existing functions and classes are relevant.
  This saves token waste from reading raw files.
```

### 13.4 QC Agent Graph Usage

QC agent uses the graph for every PR review:

```
1. detect_changes(base="HEAD~1")
   → risk-scored list of changed functions
   
2. get_impact_radius(changed_function)
   → blast radius: callers, dependents, test gaps
   
3. get_affected_flows(changed_file)
   → execution paths impacted
   
4. list_tests(changed_function)
   → which tests to run
   
5. get_review_context([changed_files])
   → minimal source snippets for review
```

This gives QC a precise, token-efficient review context — no reading entire files.

---

## 14. Atomic Build Tasks (Phase 6 Breakdown)

Each task is a single unit of work. No task should exceed one session.

### 14.1 Storage Layer (4 tasks)

| # | Task | Output | Dependencies |
|---|------|--------|--------------|
| 6.1.1 | Define SQLite schema for AutoDev graph (nodes, edges, communities, flows, snapshots, analysis_cache, repos, schema_version) | `src-tauri/src/graph/schema.rs` | None |
| 6.1.2 | Implement schema migrations runner (`run_migrations()`, `get_schema_version()`) with v1 baseline schema | `src-tauri/src/graph/migrations.rs` | 6.1.1 |
| 6.1.3 | Implement GraphStore struct (SQLite CRUD operations, WAL mode, indexes, parameterized queries only) | `src-tauri/src/graph/store.rs` | 6.1.2 |
| 6.1.4 | Implement FTS5 virtual table + `rebuild_fts_index()` for keyword search | `src-tauri/src/graph/store.rs` | 6.1.3 |

### 14.2 Detection Layer (2 tasks)

| # | Task | Output | Dependencies |
|---|------|--------|--------------|
| 6.2.1 | Implement `collect_files(root, ignore_patterns)` — directory walker with .gitignore support, extension whitelist, default ignore patterns | `src-tauri/src/graph/detect.rs` | None |
| 6.2.2 | Define `LanguageConfig` struct + build extension-to-language mapping for all 25+ languages | `src-tauri/src/graph/detect.rs` | None |

### 14.3 Extraction Layer (6 tasks)

| # | Task | Output | Dependencies |
|---|------|--------|--------------|
| 6.3.1 | Implement SHA-256 file hasher + extraction cache (path → hash → cached result) | `src-tauri/src/graph/cache.rs` | None |
| 6.3.2 | Implement generic tree-sitter walker: `walk_tree(tree, source, path, config)` → `ExtractionResult` | `src-tauri/src/graph/extract.rs` | 6.2.2 |
| 6.3.3 | Implement Python, JavaScript, TypeScript language extractors (import handlers, call resolution) | `src-tauri/src/graph/extract.rs` | 6.3.2 |
| 6.3.4 | Implement Go, Rust, Java, C/C++ language extractors | `src-tauri/src/graph/extract.rs` | 6.3.2 |
| 6.3.5 | Implement remaining language extractors (Ruby, PHP, Kotlin, Swift, Vue, Dart, Bash, Elixir, etc.) | `src-tauri/src/graph/extract.rs` | 6.3.2 |
| 6.3.6 | Implement parallel extraction via Rayon: `extract_all(paths) → Vec<ExtractionResult>` | `src-tauri/src/graph/extract.rs` | 6.3.1, 6.3.3, 6.3.4, 6.3.5 |

### 14.4 Build Layer (3 tasks)

| # | Task | Output | Dependencies |
|---|------|--------|--------------|
| 6.4.1 | Implement node ID generation (`make_id`) + ID normalization + 3-layer deduplication | `src-tauri/src/graph/build.rs` | None |
| 6.4.2 | Implement `build(extractions) → Graph` — merge extractions, normalize IDs, resolve dangling edges | `src-tauri/src/graph/build.rs` | 6.4.1 |
| 6.4.3 | Implement SQLite upsert from ExtractionResult — `upsert_nodes()`, `upsert_edges()`, `prune_deleted_file()` | `src-tauri/src/graph/build.rs` | 6.1.3, 6.4.2 |

### 14.5 Incremental Update Layer (3 tasks)

| # | Task | Output | Dependencies |
|---|------|--------|--------------|
| 6.5.1 | Implement `parse_git_diff(repo_root, base)` — extract changed file paths from `git diff --name-only` | `src-tauri/src/graph/incremental.rs` | None |
| 6.5.2 | Implement incremental update algorithm: hash check → changed files only → update dependents → rebuild FTS5 | `src-tauri/src/graph/incremental.rs` | 6.3.6, 6.4.3, 6.5.1 |
| 6.5.3 | Wire Tauri FSEvents file watcher to incremental update → emit `graph:updated` Tauri event | `src-tauri/src/graph/watcher.rs` | 6.5.2 |

### 14.6 Community Detection (2 tasks)

| # | Task | Output | Dependencies |
|---|------|--------|--------------|
| 6.6.1 | Implement Louvain community detection in Rust (using `petgraph` or HTTP call to core-orchestrator) | `src-tauri/src/graph/cluster.rs` | 6.4.2 |
| 6.6.2 | Implement community naming algorithm (prefix + dominant class + keyword) + cohesion score | `src-tauri/src/graph/cluster.rs` | 6.6.1 |

### 14.7 Analysis Engine (5 tasks)

| # | Task | Output | Dependencies |
|---|------|--------|--------------|
| 6.7.1 | Implement `god_nodes(top_n)` — degree sort, exclude file-hubs/stubs/concepts | `src-tauri/src/graph/analyze.rs` | 6.4.2 |
| 6.7.2 | Implement `surprising_connections(top_n)` — composite surprise score (confidence × cross-file × cross-community × peripheral→hub) | `src-tauri/src/graph/analyze.rs` | 6.6.2 |
| 6.7.3 | Implement `suggest_questions(top_n)` — 5 signal types (AMBIGUOUS, bridge nodes, INFERRED god nodes, isolated, low cohesion) | `src-tauri/src/graph/analyze.rs` | 6.7.1, 6.7.2 |
| 6.7.4 | Implement `get_impact_radius(qname, depth)` — backward BFS through callers/dependents, collect test gaps | `src-tauri/src/graph/analyze.rs` | 6.1.3 |
| 6.7.5 | Implement `graph_diff(snapshot_a, snapshot_b)` — node/edge set diff with summary string | `src-tauri/src/graph/analyze.rs` | 6.1.3 |

### 14.8 Execution Flow Detection (2 tasks)

| # | Task | Output | Dependencies |
|---|------|--------|--------------|
| 6.8.1 | Implement entry point detection (framework decorator patterns + name patterns for 15+ frameworks) | `src-tauri/src/graph/flows.rs` | 6.1.3 |
| 6.8.2 | Implement flow tracing (forward BFS from entry points) + criticality scoring (security keywords, test coverage, path length) | `src-tauri/src/graph/flows.rs` | 6.8.1 |

### 14.9 Change Analysis (2 tasks)

| # | Task | Output | Dependencies |
|---|------|--------|--------------|
| 6.9.1 | Implement `parse_git_diff_ranges(repo_root, base)` — `git diff --unified=0` → changed line ranges per file | `src-tauri/src/graph/changes.rs` | None |
| 6.9.2 | Implement `detect_changes(base)` — git diff → affected nodes → risk scoring (callers + security + test coverage) | `src-tauri/src/graph/changes.rs` | 6.9.1, 6.7.4 |

### 14.10 Report Generation (2 tasks)

| # | Task | Output | Dependencies |
|---|------|--------|--------------|
| 6.10.1 | Implement `generate_report(G, communities, analysis)` → GRAPH_REPORT.md markdown string | `src-tauri/src/graph/report.rs` | 6.7.1, 6.7.2, 6.7.3, 6.6.2 |
| 6.10.2 | Implement snapshot storage + retrieval (`save_snapshot()`, `load_snapshot()`, store in SQLite as compressed JSON) | `src-tauri/src/graph/report.rs` | 6.1.3 |

### 14.11 Export Formats (3 tasks)

| # | Task | Output | Dependencies |
|---|------|--------|--------------|
| 6.11.1 | Implement `export_json(G) → graph.json` using networkx node_link format schema | `src-tauri/src/graph/export.rs` | 6.4.2 |
| 6.11.2 | Implement `export_html(G, communities)` → self-contained dark-theme D3.js HTML | `src-tauri/src/graph/export.rs` | 6.6.2 |
| 6.11.3 | Implement `export_svg(G, communities)` → static SVG (cap at 5000 nodes) | `src-tauri/src/graph/export.rs` | 6.6.2 |

### 14.12 MCP Server (4 tasks)

| # | Task | Output | Dependencies |
|---|------|--------|--------------|
| 6.12.1 | Implement MCP stdio server scaffold + 5 discovery tools (`get_minimal_context`, `get_architecture_overview`, `get_graph_stats`, `list_communities`, `get_graph_report`) | `src-tauri/src/graph/mcp.rs` | 6.7.1, 6.6.2, 6.10.1 |
| 6.12.2 | Implement 9 navigation tools (`query_graph`, `semantic_search_nodes`, `get_node_details`, `find_path`, `explain_node`, `get_file_summary`, `list_callers`, `list_callees`, `list_tests`) | `src-tauri/src/graph/mcp.rs` | 6.12.1 |
| 6.12.3 | Implement 5 impact analysis tools (`get_impact_radius`, `detect_changes`, `get_review_context`, `get_affected_flows`, `get_flow_criticality`) | `src-tauri/src/graph/mcp.rs` | 6.8.2, 6.9.2, 6.12.1 |
| 6.12.4 | Implement 5 AutoDev-native tools (`get_worker_graph`, `get_task_graph`, `get_locked_nodes`, `get_worktree_diff`, `annotate_for_worker`) + 5 MCP prompts | `src-tauri/src/graph/mcp.rs` | 6.12.1 |

### 14.13 Query Engine (2 tasks)

| # | Task | Output | Dependencies |
|---|------|--------|--------------|
| 6.13.1 | Implement `_score_nodes(query_terms)` (diacritic-insensitive) + BFS/DFS traversal with token budget + `_subgraph_to_text()` formatter | `src-tauri/src/graph/query.rs` | 6.1.3 |
| 6.13.2 | Implement `find_path(source, target)` shortest path (Dijkstra with edge weights) | `src-tauri/src/graph/query.rs` | 6.4.2 |

### 14.14 Graphify Tab UI (6 tasks)

| # | Task | Output | Dependencies |
|---|------|--------|--------------|
| 6.14.1 | Create `Graphify.tsx` tab scaffold: repo selector, build/update/export buttons, render mode selector, stats bar | `src/tabs/Graphify.tsx` | None |
| 6.14.2 | Implement D3.js force-directed graph component in React: nodes (size by degree, color by community), edges (color by kind, opacity by confidence) | `src/components/GraphCanvas.tsx` | 6.14.1 |
| 6.14.3 | Implement node interaction: click to select (show NodeInfo sidebar), double-click to zoom, drag to reposition | `src/components/GraphCanvas.tsx` | 6.14.2 |
| 6.14.4 | Implement NodeInfo sidebar: name, kind, file, line, community, callers list, callees list, "Open in IDE Tab" button that fires Tauri event | `src/components/GraphNodePanel.tsx` | 6.14.3 |
| 6.14.5 | Implement Graph Report panel (collapsible, markdown-rendered): god nodes table, surprising connections, suggested questions | `src/components/GraphReport.tsx` | 6.10.1 |
| 6.14.6 | Implement community filter, edge-type filter, search input (FTS5-backed), export dropdown (JSON/HTML/SVG) | `src/tabs/Graphify.tsx` | 6.14.2 |

### 14.15 Worker Integration (3 tasks)

| # | Task | Output | Dependencies |
|---|------|--------|--------------|
| 6.15.1 | Implement worker attribution system: tag nodes/edges with `agent_author` and `task_id` when writing via worker recipe context | `src-tauri/src/graph/store.rs` | 6.1.3 |
| 6.15.2 | Wire incremental graph update into worker lifecycle: post-file-write triggers update, task completion takes snapshot + computes diff | `src-tauri/src/agent/` | 6.5.2, 6.7.5 |
| 6.15.3 | Implement PA context injection: `build_graph_context(query, repo)` → relevant subgraph as text → prepend to system prompt in `context.rs` | `src-tauri/src/agent/context.rs` | 6.13.1 |

### 14.16 Multi-Repo Registry (2 tasks)

| # | Task | Output | Dependencies |
|---|------|--------|--------------|
| 6.16.1 | Implement multi-repo registry: `~/.autodev/registry.json` + `register(path, alias)`, `list_repos()`, `unregister(path)` | `src-tauri/src/graph/registry.rs` | None |
| 6.16.2 | Implement cross-repo graph merge: load multiple `graph.json` files → `compose_all()` → merged graph with `repo` attribute on each node | `src-tauri/src/graph/registry.rs` | 6.11.1 |

---

**Total atomic tasks: 51**

**Recommended build order**:
1. Storage layer (6.1.x) — everything else depends on this
2. Detection layer (6.2.x) — independent, do in parallel with storage
3. Extraction layer (6.3.x) — depends on detection, needs storage for cache
4. Build layer (6.4.x) — depends on extraction
5. Incremental layer (6.5.x) — depends on build + git
6. Analysis engine (6.7.x), Community (6.6.x), Flows (6.8.x), Changes (6.9.x) — depends on storage + build, can parallelize
7. MCP server (6.12.x) — depends on all analysis
8. Report (6.10.x) + Export (6.11.x) — depends on analysis
9. Query engine (6.13.x) — depends on storage + build
10. UI (6.14.x) — depends on MCP/Tauri commands being ready
11. Worker integration (6.15.x) + Registry (6.16.x) — final integration

---

*This document is the ground truth for AutoDev Phase 6: Graph Engine. Each subsection maps directly to implementation tasks. Update after each task group completes.*
