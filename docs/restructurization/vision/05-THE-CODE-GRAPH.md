# 05 — The Code Graph

| | |
|---|---|
| **Parent** | `00-…-MASTER-PLAN.md` §Part 1B (M-17, M-18, M-19), §Part 3.5 |
| **Locks** | Hands **ADR-01-09** (path/data root) to `00 §Part 8`. Contributes M-18 to **ADR-01-06** |
| **Status** | PLANNED |
| **Grounding** | `_grounding/G1 §A.10, §E.1` · `G2a §4` (the reader question) · `G3d` (F-20…F-23, F-31, F-42) · `G5b` (the rendering half) · `docs/reference/autodev-graph-master-spec.md` (via `G7`) |

---

## What this is

A queryable model of the user's codebase — 5,623 LOC across 17 files, tree-sitter extraction, community clustering, incremental update, an FTS index and a versioned migration ladder. It is the most substantial single subsystem in `src-tauri` after the agent, and it is **the only one with a real migration ladder**.

It is also, today, almost entirely unread.

**The subtle failures this subsystem is prone to:**

1. **Building something nothing consumes.** The graph is built, stored, indexed — and no consumer reads it for context. INV-8 exists because of subsystems exactly like this one.
2. **Code that isn't in the binary.** The two modules that would expose the graph to an agent have no `mod` declaration. They compile in an editor and do not exist at runtime.
3. **A key that isn't canonical.** The DB path is a hash of a raw path string, so four spellings of the same directory produce four databases.
4. **An "update" that destroys.** Incremental update drops cross-file edges and never re-clusters, so the graph degrades every time it is refreshed.

---

## 1. The three problems, in order

### 1.1 It is not in the binary (M-17)

`agent/builtin_tools/graph_tools.rs` (840 LOC) and `graph_prompts.rs` (78 LOC) have **no `mod` declaration anywhere in `src-tauri`** (`G2a T32`). 918 lines that have never been compiled. The agent has no graph access at all — not even opt-in.

The same defect independently hides `db/checkpoints.rs`, which is why undo/rollback is not in the shipped app (`G3d F-44`).

**The fix is two lines. The gate is what matters:** a test that asserts every `.rs` file under `src/` is reachable from the crate root. Without it this recurs, silently, and the only symptom is a feature that "doesn't seem to work".

### 1.2 It has no reader (M-18)

`context.rs` has no graph import. The only `crate::graph` consumers are the UI command layer and the uncompiled tool module. So "workers are not using it for token management" is exactly right, and understated.

**The mechanism** (spec `03 §3`): given a task and the graph, produce a ranked, budgeted set of files and symbols by import/call adjacency to the task's named entities, community membership, and change recency. `MATH`, not `ORGAN`.

**The gate** (`G-GRAPH-SCOPE`): with a graph present, context selection sends measurably fewer files than without — and the right ones.

### 1.3 It corrupts itself on update (M-19)

Four confirmed defects:

| ID | Defect | Effect |
|---|---|---|
| `F-22` | `INSERT OR REPLACE` combined with `ON DELETE CASCADE` | Silently deletes edges **graph-wide** |
| `F-23` | `reconcile_edges` drops every cross-file edge on incremental update | The graph loses exactly the edges that make it useful |
| `F-21`/`F-20` | Community wipe with no re-clustering — `incremental.rs` never calls `run_clustering` | Everything added by ↻ Update has `community = NULL` forever, and **never appears in Graph City at all** |
| `F-42` | A single unreadable directory aborts the whole build on Windows | One permission-denied folder = no graph |

Plus `F-31`: **migration v5 uses `ADD COLUMN IF NOT EXISTS`, which is not valid SQLite** — a syntax error, silently swallowed, that can never retry. The one real migration ladder in the codebase is broken at its head and **must be repaired before v6 is added**.

---

## 2. Storage and identity

### 2.1 Two competing locations

| Scheme | Used by | Path |
|---|---|---|
| **A** | all 13 store-touching commands (`graph_commands.rs:74-81`) | `~/.autodev/graphs/<sha256(repo_path)[..12]>.db` |
| **B** | `RepoRegistry` (`registry.rs:62,123`) | `<repo_path>/.autodev/graph.db` |

Scheme B explains the stray `src-tauri/.autodev/graph.db` observed on disk. **The registry DB is created by `get_store` but never populated**, so `merge_all_repos_cmd` always merges empty stores and returns `Ok` — a silent no-op.

**Resolution: scheme A only, with a canonical path.**

### 2.2 The key must be canonical

The path is hashed **verbatim**, so `C:\p`, `C:/p`, `c:\p` and `C:\p\` each produce a different database. On a case-insensitive filesystem these are the same directory.

**The rule: normalize before hashing** — canonicalize, resolve symlinks, unify separators, case-fold where the platform is case-insensitive. This is the same discipline as the checkpoint retention key (`G3b GI-28`) and the path-lock key (spec `06`); it should be one shared helper, not three implementations.

### 2.3 Schema

`nodes`, `edges`, `communities`, `flows`, `snapshots`, `analysis_cache`, `repos`, `nodes_fts`. Migration head **v5** (`PRAGMA user_version`, `graph/migrations.rs:12-45`) — repair required before extension.

---

## 3. The rendering contract (with spec 07)

Three fields the backend has and does not send, each disabling a visual encoding (`G5b M1/M2/M3`):

| Missing | Consumer | Effect |
|---|---|---|
| `line_start` / `line_end` | building height, treemap weight | `loc` falls back to a constant `10` — **every building is the same height**; the city's primary encoding is dead flat |
| `confidence_tier` | CALLS arc opacity/radius | every arc renders identically; the confidence encoding is dead |
| — (`community = -1`) | every colour map (`ARR[id % len]`) | communities with <3 members are bucketed to `id = -1`; `COMMUNITY_COLORS[-1]` is `undefined`, producing **black-on-black nodes in 2D and a white slab in the city** |

**Aggregation has no escape hatch.** Above `NODE_RENDER_LIMIT = 2000` the backend returns only community super-nodes, and there is **no `max_nodes` or `force_full` parameter** — so the UI's own banner telling the user to "Switch to Full … to inspect individual files" is a no-op (`G5b B3`). Either the parameter is added or the banner is removed; shipping a control that does nothing is worse than shipping neither.

---

## 4. Determinism balance

Entirely `DET` (extraction, storage, incremental update) and `MATH` (clustering, scoring, selection). **No model call anywhere in this subsystem.**

---

## 5. Test gates

| Gate | Asserts | Catches |
|---|---|---|
| **G-REACHABLE** | Every `.rs` under `src/` is reachable from the crate root | `graph_tools`, `graph_prompts`, `db/checkpoints` |
| **G-GRAPH-SCOPE** | Context selection with a graph sends fewer, better files than without | The reader-less graph |
| **G-INCREMENTAL** | Build → edit one file → incremental update: cross-file edges survive, communities are re-assigned, no edge is deleted elsewhere | F-21, F-22, F-23 |
| **G-PATHKEY** | `C:\p`, `C:/p`, `c:\p`, `C:\p\` all resolve to **one** database | Verbatim hashing |
| **G-MIGRATE** | The ladder applies cleanly from empty and from v5; a bad statement **fails loudly** | F-31's swallowed syntax error |
| **G-BUILD-RESILIENT** | A build over a tree containing an unreadable directory completes and reports what it skipped | F-42 |
| **G-VIZ-CONTRACT** | The visualization payload carries `line_start`, `line_end`, `confidence_tier`; no community id is negative in a rendered payload | M1, M2, M3 |

---

> No ADR decision is recorded in this spec. It hands **ADR-01-09** to `00 §Part 8` and contributes M-18 to **ADR-01-06**.
