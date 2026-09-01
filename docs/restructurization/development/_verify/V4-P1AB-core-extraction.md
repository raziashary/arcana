# V4 — Adversarial verification: P1 groups A & B (core extraction)

**Verifier stance:** REFUTE by default. Ambiguous evidence = REFUTED.
**Repo:** `C:\AIRS Foundry\autodev\autodev`
**Date:** 2026-09-01
**Status:** COMPLETE.

**Standing caveat on provenance:** C1–C4 restate, almost word for word, the `KNOWN_UNREACHABLE`
comments in `src-tauri/src/reachability_gate.rs:25-47`. They are not independent observations of
the code, and where the gate is wrong the claims inherit the error. Three of the gate's own
factual statements are wrong: the tool count (22 vs **27**), the `git/mod.rs` line number (`:31`
vs **`:21`**), and "undo does not exist in the shipped binary" (**it does**).

---

## Verdict table

| Claim | Verdict | One line |
|---|---|---|
| **C1** | **CONFIRMED** | Zero `mod` declarations; the only `mod graph_tools` in the workspace is inside a `//` comment. LOC 840 / 78 exact. |
| **C2** | **CONFIRMED, understated** | Both defects real, but there are **6** E0609s + 1 warning, not 2 — and the first five land at `:296-300`, not `:272`. |
| **C3** | **CONFIRMED, count wrong** | No `impl Tool for`, no registration — but there are **27** structs, not 22, and the trait gap is 5 changes per struct, not 1. |
| **C4** | **PARTIALLY CONFIRMED / OVERSTATED** | Undeclared ✓, 5 commands ✓, different DBs ✓ — but `git/mod.rs:21` not `:31`, **undo already ships**, and declaring it would double up **nothing**. |
| **C5** | **REFUTED as stated** | `security/` doesn't exist; Tauri coupling is **6 lines in 2 files**; the real blocker is **4 crate cycles** and 114 refs to modules that stay behind. |
| **C6** | **Mechanism CONFIRMED, all 3 numbers WRONG** | Real: **71** call sites in **24** files, **21** of them in `builtin_tools/` across 11 files. "132/27" counts `events.rs`'s own definitions; "11" is a file count in a list of site counts. |
| **C7** | **PARTIALLY CONFIRMED** | `airsctl` links no Tauri today (verified by `cargo tree`). `provider`/`memory`/`platform` are safe; `agent/` drags Tauri in and would red the `cli-checks` CI job. |

---

## Already done, or already impossible

**Already done:**
- The reachability guard P1-04 wants to see pass **already passes.** I reimplemented its logic
  read-only: the orphan set is exactly `{graph_tools.rs, graph_prompts.rs, db/checkpoints.rs}`,
  which is exactly `KNOWN_UNREACHABLE`. The gate cannot distinguish "declared" from "listed as
  known-unreachable", so P1-04 proves nothing unless P1-01/02 also *remove* the list entries.
- The undo feature P1-02 says is missing **is shipped and wired end to end** — see C4.
- `platform/` is already structured for the move and says so in its own header
  (`platform/mod.rs:4-5`).

**Already impossible:**
- P1-05 cannot move `security/`: it does not exist and is created later, by P1-20
  (`MASTER_DEVELOPMENT_PLAN.md:535`).
- P1-05 cannot move `agent/` at all in its current form — four crate cycles (C5).
- P1-01 alone cannot "make the invisible visible": declaring `graph_tools` compiles 27 structs
  that no registry ever sees. The capability needs P3, exactly as the gate says — but the gate's
  own guard would then read green over a module that still does nothing.

**Not a task, but load-bearing:** `platform/` has **zero consumers** anywhere in the workspace.
375 LOC, 9 passing tests, never called.

---

## Per-claim detail

### C1 — graph_tools.rs / graph_prompts.rs undeclared

**CONFIRMED.**

Evidence:

1. `src-tauri/src/agent/builtin_tools/mod.rs` declares **42** sub-modules (`grep -c "^pub mod "`,
   lines 6–72), while the directory holds **44** `.rs` files besides `mod.rs`. The two-file gap is
   exactly `graph_tools.rs` and `graph_prompts.rs`. Full declared set (line numbers):
   `assign_task:6, assign_tasks_batch:7, bash:8, create_task:9, edit_file:10, get_task_tool:11,
   glob:12, grep:13, list_directory:14, list_tasks_tool:15, planner:16, read_file:17,
   spawn_parallel_subagents:18, spawn_subagent:19, update_task_status_tool:20, write_file:21,
   spawn_qc_review:26, write_interface_spec:29, append_file:32, check_file:33, e2e_probe:34,
   remember_this:35, request_supervisor_action:36, search_memory:37, validate:38, path_jail:43,
   path_lock:48, analyze_diff_tool:52, annotate_file_tool:53, build_evidence:54,
   claim_task_tool:55, set_task_budget_tool:56, suggest_worker_tool:57, create_epic_tool:59,
   create_phased_task_tool:60, get_epic_progress_tool:61, get_ready_tasks_in_phase_tool:62,
   list_worker_requests_tool:64, resolve_worker_request_tool:65, acknowledge_worker_message:70,
   list_worker_messages:71, send_worker_message:72`.

2. Workspace-wide grep for the literal declaration returns exactly ONE hit, and it is inside a
   `//` comment:

   `src-tauri/src/reachability_gate.rs:12`
   ```
   // "Add `pub mod graph_tools;`" and declares the section COMPLETE. It never
   ```

   Command: `grep -rn "mod graph_tools\|mod graph_prompts\|mod checkpoints" src-tauri src-cli src-shared --include=*.rs`
   → 1 result, the comment above. No `mod checkpoints` anywhere either (relevant to C4).

3. LOC counts in the claim are exact:
   - `src-tauri/src/agent/builtin_tools/graph_tools.rs` = **840** lines (25,082 bytes)
   - `src-tauri/src/agent/builtin_tools/graph_prompts.rs` = **78** lines (4,396 bytes)

4. Both files are stale relative to the rest of the directory: `graph_prompts.rs` mtime
   **2026-04-28**, `graph_tools.rs` mtime **2026-05-11**, while every declared sibling in that
   directory has mtime 2026-09-01. Consistent with "never touched since written, because nothing
   ever compiled them."

**Adversarial note (important for the caller):** C1–C4 are *near-verbatim restatements of the
repo's own comments* in `src-tauri/src/reachability_gate.rs:25-47` (`KNOWN_UNREACHABLE`). The
claims did not originate from an independent read of the code. I verified each against the code
anyway; where the gate's prose and the code disagree, I report the code. See C2 and C4 — the
gate's prose is materially incomplete on C2 and partially wrong on C4.

**Already done / partially done:** a guard for exactly this failure already exists and is wired:
`src-tauri/src/lib.rs:37-38`
```
#[cfg(test)]
mod reachability_gate;
```
The gate test `every_source_file_is_reachable_from_the_crate_root` will FAIL the moment any of
the three files is deleted-from-list-without-repair, and its sibling
`every_known_unreachable_file_still_exists` fails if a listed file is declared or deleted without
updating `KNOWN_UNREACHABLE`. **This means P1-01/02 cannot be "just add a `mod` line" — the
`KNOWN_UNREACHABLE` entry must be removed in the same commit or the test suite goes red.**

### C2 — graph_tools.rs does not compile

**CONFIRMED — both cited defects are real. But the line attribution for the first is wrong, and
the claim understates the error count: 6 hard errors, not 2 (plus 1 warning).**

I did NOT run the compiler on this — the brief forbids editing any source file to test it, and
`cargo check` cannot see an undeclared module. Everything below is static type-checking against
the actual signatures, each of which I read. See "What I could not determine" for the residual.

#### Defect 1 — `Option<GraphNode>` used as `GraphNode`. REAL, but the errors land at 296–300, not 272.

`src-tauri/src/agent/builtin_tools/graph_tools.rs:271-272`:
```rust
    pub fn execute(store: &GraphStore, qualified_name: String) -> Result<Value> {
        let node = store.get_node(&qualified_name)?;
```
`src-tauri/src/graph/store.rs:304`:
```rust
    pub fn get_node(&self, qualified_name: &str) -> Result<Option<GraphNode>> {
```
So after `?`, `node: Option<GraphNode>`. `Option<T>` does not `Deref` to `T`, so every field
access is E0609. The five accesses are at **`graph_tools.rs:296-300`**, not `:272`:
```rust
        Ok(json!({
            "node": {
                "qualified_name": node.qualified_name,
                "name": node.name,
                "kind": node.kind,
                "file": node.file_path,
                "line": node.line_start
```
→ **5 × E0609** `no field `qualified_name` / `name` / `kind` / `file_path` / `line_start` on type
`Option<GraphNode>`` at `:296, :297, :298, :299, :300`. The claim's "E0609 x5" count is right;
its `:272` is the *binding* site, which is where a reader must look but not where rustc points.

#### Defect 2 — `confidence_tier` on `SuggestedQuestion`. REAL.

`graph_tools.rs:689-697`:
```rust
        let questions = suggest_questions(store, 5)?;

        let questions_json: Vec<_> = questions
            .iter()
            .map(|q| json!({
                "question": q.question,
                "type": q.question_type,
                "confidence": q.confidence_tier
            }))
```
`src-tauri/src/graph/analyze.rs:205`:
```rust
pub fn suggest_questions(store: &GraphStore, top_n: usize) -> Result<Vec<SuggestedQuestion>> {
```
`src-tauri/src/graph/analyze.rs:30-35` — the whole struct:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedQuestion {
    pub question_type: String,
    pub question: String,
    pub why: String,
}
```
Three fields. No `confidence_tier`. → **1 × E0609** at `graph_tools.rs:696`.
(Contrast `GraphEdge`, `store.rs:44`, which *does* have `pub confidence_tier: String` — that is
why `graph_tools.rs:283` and `:289` are fine. The author copied the field name across types.)

#### Full error list if the module were declared

**Hard errors: 6 total, in 2 functions.**

| # | `path:line` | Code | Message shape |
|---|---|---|---|
| 1 | `graph_tools.rs:296` | E0609 | no field `qualified_name` on type `Option<GraphNode>` |
| 2 | `graph_tools.rs:297` | E0609 | no field `name` on type `Option<GraphNode>` |
| 3 | `graph_tools.rs:298` | E0609 | no field `kind` on type `Option<GraphNode>` |
| 4 | `graph_tools.rs:299` | E0609 | no field `file_path` on type `Option<GraphNode>` |
| 5 | `graph_tools.rs:300` | E0609 | no field `line_start` on type `Option<GraphNode>` |
| 6 | `graph_tools.rs:696` | E0609 | no field `confidence_tier` on type `SuggestedQuestion` |

**Warnings: 1.**

| # | `path:line` | Lint | Detail |
|---|---|---|---|
| 7 | `graph_tools.rs:8` | `unused_imports` | `use crate::graph::flows::detect_entry_points;` — `grep -n detect_entry_points graph_tools.rs` returns only line 8. The symbol resolves (`flows.rs:34`), it is simply never called. |

Not an error, and not a warning either: every struct and fn in the file is `pub`, and the path
`lib.rs:7 pub mod agent;` → `agent/mod.rs:7 pub mod builtin_tools;` is public all the way down, so
the 27 unused structs would be externally reachable and `dead_code` would **not** fire.

**Everything else in the file type-checks.** I verified each cross-module call and every field
read against the live definitions:

- `store.get_all_nodes_for_graph(i64::MAX, 0)` / `get_all_edges_for_graph` — `store.rs:503` and
  `store.rs:542`, both `(&self, limit: i64, offset: i64)`. 19 call sites, all correct.
- `store.get_edges_for_node` — `store.rs:343` → `Result<Vec<GraphEdge>>`. Used at `:273`, `:367`.
- `query_by_name` / `query_dependents` / `query_dependencies` — `query.rs:32, :88, :160`, all
  `(&GraphStore, &str) -> Result<QueryResult>`; `QueryResult` (`query.rs:9-13`) has exactly the
  `nodes` / `edges` / `summary` read at `:210-212`, `:442-443`, `:459-460`.
- `god_nodes` (`analyze.rs:80`) → `GodNode` (`analyze.rs:9-16`) has `label`, `qualified_name`,
  `degree`, `file_path` — all four read at `:640-643` exist.
- `surprising_connections` (`analyze.rs:120`) → `SurprisingConnection` (`analyze.rs:19-28`) has
  `source`, `target`, `score`, `why` — all four read at `:667-670` exist.
- `get_impact_radius` (`analyze.rs:285-290`, `(store, &str, u32, usize)`) — call at `:503` passes
  `max_depth.unwrap_or(5)` (`u32`) and `100` (infers `usize`). `ImpactRadius` (`analyze.rs:48-55`)
  has all six fields read at `:506-511`.
- `graph_diff` (`analyze.rs:486`) → `GraphDiff` (`analyze.rs:72-78`) has all five fields read at
  `:719-723`.
- `detect_changes` (`changes.rs:136-140`, `(store, &str, &str)`) — `&base.unwrap_or_else(…)` is a
  `&String` that deref-coerces. `ChangeAnalysis` (`changes.rs:20-25`) has all four fields read at
  `:527-530`.
- `crate::errors::Result<T>` is `std::result::Result<T, AutoDevError>` (`errors.rs:62`) and
  `AutoDevError::Agent(String)` exists (`errors.rs:19`), so `:365`'s `.ok_or_else(…)?` is fine.
- `chrono::Local::now()` at `:125` and `:837` — `chrono = { version = "0.4", features = ["serde"] }`
  with default features on, so the `clock` feature and `Local` are available.
- Every `graph::` sub-module used is `pub mod` (`graph/mod.rs:5-21`) — no E0603.
- I specifically checked the ten `json!({ "x": v, "count": v.len() })` sites (`:162, :258, :443,
  :486, :618, :649, :676, :702, :761, :789, :817`) for use-after-move. They are **fine**:
  serde_json 1.0.149 (the locked version, `Cargo.lock:5199-5200`) expands the value arm as
  `macros.rs:278-280`:
  ```rust
      ($other:expr) => {
          $crate::to_value(&$other).unwrap()
      };
  ```
  i.e. it borrows.

#### On `graph_prompts.rs` "compiles cleanly" — TRUE, but it is wrong in a way nobody has noticed

The whole file is five `pub fn …() -> serde_json::Value` returning `json!` literals plus
`all_prompts()` at `:70-78`. Only dependency is `use serde_json::json;` (`:3`). It compiles.

But `graph_prompts.rs:38` instructs the model to:
```
1. Call semantic_search_nodes(query, top_k=10) to find relevant code
```
**There is no `semantic_search_nodes` tool.** The nearest thing in `graph_tools.rs` is
`KeywordSearchTool`, whose `name()` at `:220-222` returns `"keyword_search_nodes"`. So even after
P3 repairs and wires everything, `debug_issue_prompt` step 1 names a tool that will not exist.
Nothing in either claim mentions this.

### C3 — 22 tool structs do not impl Tool, nothing registers them

**CONFIRMED on substance, but the count "22" is WRONG — there are 27.**

`grep -c "^pub struct" src-tauri/src/agent/builtin_tools/graph_tools.rs` → **27**.
The file's own header lies too — `graph_tools.rs:1`:
```
// Graph MCP tools — all 22 graph analysis and navigation tools
```
but its own section banners sum to 27: DISCOVERY (5) `:12`, NAVIGATION (9) `:190`, IMPACT (5)
`:492`, ANALYSIS (4) `:624`, AUTODEV-NATIVE (4) `:735` = 27. Struct list with line numbers:
`GetMinimalContextTool:15, GetArchitectureOverviewTool:65, GetGraphStatsTool:99,
ListCommunitiesTool:130, GetGraphReportTool:167, QueryGraphTool:193, KeywordSearchTool:217,
GetNodeDetailsTool:264, FindPathTool:308, ExplainNodeTool:352, GetFileSummaryTool:393,
ListCallersTool:431, ListCalleesTool:448, ListTestsTool:465, GetImpactRadiusTool:495,
DetectChangesTool:516, GetReviewContextTool:535, GetAffectedFlowsTool:565,
GetFlowCriticalityTool:589, GetGodNodesTool:627, GetSurprisingConnectionsTool:654,
GetSuggestedQuestionsTool:681, GetGraphDiffTool:707, GetWorkerGraphTool:738,
GetTaskGraphTool:766, GetLockedNodesTool:794, GetGraphStatusTool:822`.

The "22" appears in `reachability_gate.rs:5` and `:28` and is repeated by the plan. **Any task
sized off "22 tools" is sized 23% low.**

`grep -n "impl Tool for" src-tauri/src/agent/builtin_tools/graph_tools.rs` → **zero hits**. Every
struct is a unit struct with two *inherent* associated fns, e.g. `graph_tools.rs:15-22`:
```rust
pub struct GetMinimalContextTool;

impl GetMinimalContextTool {
    pub fn name() -> &'static str {
        "get_graph_minimal_context"
    }

    pub fn execute(store: &GraphStore, task: String) -> Result<Value> {
```
Note the signature shape: `execute` is **synchronous**, takes `&GraphStore` plus *typed*
positional params (`task: String`, `top_k: u32`, `max_depth: Option<u32>`,
`files: Vec<String>`, `snapshot_id_a: i64`…). The real trait is async and takes a JSON blob —
see below. No struct is constructible-as-a-Tool without a full rewrite of all 27 `execute` fns.

The live trait is in `src-tauri/src/agent/tool_trait.rs:109-127` (NOT `tool_registry.rs` —
that file only holds `ToolRegistry`, `tool_registry.rs:9-46`):
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;

    /// Read-only tools can run concurrently; write tools run serially.
    /// Pattern from Claude Code's isConcurrencySafe().
    fn is_read_only(&self) -> bool {
        false
    }

    /// Requires explicit user approval before execution.
    fn requires_permission(&self) -> bool {
        !self.is_read_only()
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult;
}
```
Every gap between the two shapes is real per-struct work — five of them:
1. `fn name() -> &'static str` (no `&self`) vs `fn name(&self) -> &str`.
2. `description()` — **does not exist anywhere in `graph_tools.rs`**; 27 must be authored.
   (This is presumably what `graph_prompts.rs` is for — see C2 note.)
3. `input_schema()` — does not exist; 27 JSON schemas must be authored.
4. sync `execute(store: &GraphStore, task: String) -> Result<Value>` vs
   `async execute(&self, input: Value, ctx: &ToolContext) -> ToolResult`. Note the return type:
   the trait returns a **bare `ToolResult`**, not `Result<…>`, so every `?` in all 27 bodies
   must be converted to an explicit `ToolResult::error(...)` branch, and every `Ok(json!(…))`
   into `ToolResult::success(...)` over a serialized string.
5. `ToolContext` (`tool_trait.rs:49-63`) carries `session_id`, `working_dir`, `call_id`, `depth`,
   `agent_role` — **no `GraphStore` and no handle that yields one**. Every tool must locate and
   open the store itself from `working_dir`, or the context must be widened. This is the single
   largest hidden cost and it is not mentioned in any claim.

Registration: `src-tauri/src/agent/builtin_tools/mod.rs` has three registration entry points —
`register_all` (`mod.rs:78`), `register_for_mcp_worker` (`mod.rs:96`), `register_for_mode`
(`mod.rs:186`). `grep -n "graph" src-tauri/src/agent/builtin_tools/mod.rs` returns
**zero hits**. Nothing anywhere registers any struct from `graph_tools.rs`.

**Verdict: declaring the module would (after the C2 repairs) add 27 unit structs and 54 inherent
fns to the build that nothing constructs or calls.** The agent gains no graph tools. C3's
conclusion is right; its count is not.

One correction to a tempting inference: this dead weight would compile **silently**. Because
`lib.rs:7` is `pub mod agent;` and `agent/mod.rs:7` is `pub mod builtin_tools;`, every `pub` item
in `graph_tools.rs` is externally reachable from the `rlib`, so `dead_code` does not fire. There
is no compiler signal that the tools are unreachable *as tools*. That is exactly the class of
failure `reachability_gate.rs` was written for, and the gate cannot catch it either: it checks
only that a `mod` name exists somewhere, never that a `Tool` is registered. **Declaring the
module actually makes the situation harder to detect, because it satisfies the only existing
guard while changing nothing about the shipped capability.**

### C4 — db/checkpoints.rs undeclared + duplicates git/checkpoint.rs

**PARTIALLY CONFIRMED / OVERSTATED.** Undeclared: yes. `git/checkpoint.rs` live and wired to
five commands: yes. Different databases: yes, and more different than claimed. But the claimed
consequence is wrong on two counts — the `git/mod.rs` line number is wrong, and the underlying
"undo does not exist in the shipped binary" is **FALSE**.

#### Undeclared — CONFIRMED
`src-tauri/src/db/mod.rs:10-15` declares six sub-modules:
```rust
pub mod annotations;
pub mod autodev_instructions;
pub mod model_configs;
pub mod plans;
pub mod tasks;
pub mod workers;
```
`checkpoints` is absent. Workspace grep for `mod checkpoints` → zero hits. 280 LOC dark.

#### Is `git/checkpoint.rs` really live and wired? — YES, end to end. Five commands.

**The claim's `git/mod.rs:31` is WRONG.** That line is `pub mod worktree;`. The real declaration
is `src-tauri/src/git/mod.rs:21`:
```rust
pub mod checkpoint;
```
(The gate repeats the same wrong line number at `reachability_gate.rs:42`.)

Five Tauri commands, all confirmed at all three layers:

| Command | `#[tauri::command]` fn | delegates to | in `generate_handler!` | frontend caller |
|---|---|---|---|---|
| `create_checkpoint` | `commands/git_commands.rs:210` | `checkpoint::create` (`git/checkpoint.rs:66`) | `lib.rs:386` | **none** |
| `restore_checkpoint` | `commands/git_commands.rs:219` | `checkpoint::restore` (`:112`) | `lib.rs:387` | `src/stores/gitStore.ts:347` |
| `list_checkpoints` | `commands/git_commands.rs:224` | `checkpoint::list` (`:136`) | `lib.rs:388` | `src/stores/gitStore.ts:238` |
| `delete_checkpoint` | `commands/git_commands.rs:232` | `checkpoint::delete` (`:179`) | `lib.rs:389` | `src/stores/gitStore.ts:352` |
| `compare_checkpoints` | `commands/git_commands.rs:237` | `checkpoint::compare` (`:202`) | `lib.rs:390` | **none** |

And snapshots are actually being *written* on the agent path — `git/checkpoint.rs:256` defines
`pub fn auto_checkpoint_before_write(session_id: &str, file_path: &str)`, called from exactly two
live, declared, registered tools:
- `src-tauri/src/agent/builtin_tools/edit_file.rs:177`
- `src-tauri/src/agent/builtin_tools/write_file.rs:124`

both `crate::git::checkpoint::auto_checkpoint_before_write(`.

**Therefore `reachability_gate.rs:7-8` is factually wrong:**
```
// clippy, or any test. `db/checkpoints.rs` is in the same state, which is why
// undo does not exist in the shipped binary.
```
Undo **does** exist in the shipped binary: snapshot-on-write (`edit_file.rs:177`,
`write_file.rs:124`) → `restore_checkpoint` command (`lib.rs:387`) → UI call
(`gitStore.ts:347`). The dark file is a *duplicate of a working feature*, not the reason a
feature is missing. Any plan item justified by "undo is missing" is justified by a false premise.

#### Do the two use DIFFERENT databases? — YES, and different tables and different storage models.

| | `db/checkpoints.rs` (dark) | `git/checkpoint.rs` (live) |
|---|---|---|
| DB file | `~/.autodev/checkpoints.db` (`db/checkpoints.rs:67`) | `~/.autodev/config.db` (`git/checkpoint.rs:30`) |
| Table | `file_checkpoints` (`:80`) | `checkpoints` (`:33`) |
| Blob storage | **full file text inline in SQLite** — `file_content TEXT NOT NULL` (`:84`) | **files on disk** at `~/.autodev/checkpoints/<session_id>/<ts>_<basename>`, DB holds only `snapshot_path` (`:5`, `:37`) |
| Correlation key | `message_id` (`:82`) | `session_id` (`:35`) |
| Retention | none | `RETENTION_PER_FILE = 50`, evicted after each snapshot (`:25`, `:219-248`) |
| Op metadata | `operation_type` create/write/edit (`:86`) | `reason` free text (`:39`) |
| Restore | **not implemented** — no restore/undo fn exists in the file at all | `restore` (`:112`), `compare` (`:202`) |

Note the last row: the dark implementation is not merely a duplicate, it is a **less capable**
duplicate — it has 13 CRUD methods (`create`, `get_by_id`, `list_by_message_id`,
`list_by_file_path`, `get_latest_by_file_path`, `list_by_operation_type`, `list_all`, `delete`,
`delete_by_message_id`, `delete_by_file_path`, `count`, `count_by_message_id`,
`count_by_file_path`) and **no restore path**. There is nothing here to salvage that the live one
lacks. The "needs an owner decision on which implementation survives" framing in
`reachability_gate.rs:44-45` presents this as a genuine two-way choice. It is not: on the
evidence, the only defensible outcomes are *delete* `db/checkpoints.rs`, or keep it dark.

#### What would break or double up if `db/checkpoints.rs` were simply declared?

**Nothing would break, and nothing would double up.** The claim that declaring it "would stand up
a SECOND checkpoint implementation against a second database" (`reachability_gate.rs:42-44`) is
**OVERSTATED**. Declaring a module compiles code; it does not run it.

- **Compile:** it should compile clean. Every import is used (`OptionalExtension` at `:124`,
  `:165`; `AutoDevError` at `:115`; `params`, `Connection`, `Result` throughout);
  `super::autodev_dir` resolves (`db/mod.rs:21`); and every `?` on a `rusqlite::Error` is covered
  by `errors.rs:46-47`:
  ```rust
      #[error("Database error: {0}")]
      Database(#[from] rusqlite::Error),
  ```
- **Runtime:** the DB is created inside `CheckpointsDb::open()` (`db/checkpoints.rs:66-72`), and
  **nothing calls it**. `~/.autodev/checkpoints.db` would still never be created. No second
  writer, no divergence, no doubled snapshots.
- **Name collision:** none — `crate::db::checkpoints` vs `crate::git::checkpoint` are distinct
  paths (note also singular vs plural).
- **Test suite:** `cargo test --workspace --all-targets` (`.github/workflows/ci.yml:79`) would
  newly run one trivial test, `db/checkpoints.rs:269-279` `test_operation_type_conversion`, which
  passes by inspection.
- **Clippy:** CI runs `cargo clippy --workspace --all-targets` (`ci.yml:74`) **without
  `-D warnings`**, so new lints do not fail the build. The one likely new lint is
  `clippy::should_implement_trait` on the inherent `pub fn from_str` at `db/checkpoints.rs:31`.

So the real cost of declaring it is *zero runtime risk and 280 lines of compiled-but-uncalled
code* — which is precisely the state `reachability_gate.rs` exists to prevent someone from
mistaking for "done".

### C5 — autodev-core extraction feasibility

**REFUTED as stated.** P1-05 ("Create the crate; move `agent/`, `provider/`, `memory/`,
`platform/`, `security/`") cannot be executed as written, for two independent reasons:

1. **`security/` does not exist.** It is *created later in the same phase* by P1-20.
2. **Moving `agent/` creates four crate-level dependency cycles.** Rust forbids circular crate
   dependencies. This is not a difficulty; it is a compile-time impossibility.

And the framing in the brief — "how many files reference `tauri::` types" — is measuring the
wrong thing. Tauri coupling in these directories is **almost nil**. The blocker is `crate::`
coupling to modules that stay behind.

#### Does `security/` exist? — NO.

`ls src-tauri/src` returns 29 directories; `security` is not among them. `find src-tauri -type d
-name security` → nothing. And the MDP itself says why —
`docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md:535`:
```
- [ ] **P1-20** Port `redact_dict` + the 20-entry `SENSITIVE_KEYS` set into `autodev-core/security/`. → `UNIT-PROVEN`
```
So P1-05 (§P1-B) instructs a move of a directory that P1-20 (§P1-D, later in the same phase)
creates. **P1-05 is partly impossible as written and must drop `security/`.**

#### Exact Tauri coupling counts, per directory

| directory | .rs files | LOC | files w/ `tauri::` | `#[tauri::command]` | `AppHandle` | `State<` | `Emitter` | `Manager` | `Window` | files w/ `crate::events::` |
|---|---|---|---|---|---|---|---|---|---|---|
| `agent/` | 53 | 12,901 | **2** | **0** | **2** | 0 | 0 | 0 | 0 | **13** |
| `provider/` | 8 | 1,974 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `memory/` | 9 | 1,640 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `platform/` | 2 | 375 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `security/` | — | — | — | — | — | — | — | — | — | **DOES NOT EXIST** |

**Every one of the 255 real `#[tauri::command]` attributes in the crate lives in `commands/`** —
zero in the moving set, zero anywhere else. (A raw grep returns 257; the two extra hits,
`lib.rs:49` and `main.rs:182`, are `//` comments mentioning the macro, not attributes.) `State<`
appears at 75 sites confined to
`commands/*`, `events.rs`, `main.rs`, `recipes/scheduler.rs`, `subagent/manager.rs` — again zero
in the moving set. `Emitter`, `Manager`, `Window`: zero in the moving set.

The complete list of `tauri::`/`AppHandle` occurrences in all four directories — **six lines, two
files**:
```
agent/engine.rs:9:use tauri::AppHandle;
agent/engine.rs:40:    app: AppHandle,
agent/engine.rs:57:        app: AppHandle,
agent/tool_executor.rs:7:use tauri::AppHandle;
agent/tool_executor.rs:30:    app: AppHandle,
agent/tool_executor.rs:47:        app: AppHandle,
```
i.e. one import + one struct field + one `new()` parameter, twice. `agent/engine.rs:34-50` and
`agent/tool_executor.rs:27-41` are the two structs. That is a genuinely small, mechanical change.

#### The actual blocker: `crate::` fan-out and four dependency cycles

`grep -roh "crate::[a-z_][a-z_0-9]*"` per directory:

| directory | distinct `crate::` modules referenced | refs to modules that would **stay behind** |
|---|---|---|
| `agent/` | **20** | **114** |
| `provider/` | 1 (`crate::errors`) | 6 |
| `memory/` | 2 (`crate::errors`, `crate::session`) | 9 |
| `platform/` | **0** | **0** |

`agent/`'s outbound references, by module (only `agent`, `provider`, `memory` are in the moving
set):
```
 31 crate::db          27 crate::events      18 crate::agent      13 crate::recipes
 11 crate::provider     7 crate::hooks        6 crate::memory      6 crate::errors
  5 crate::subagent     5 crate::graph        5 crate::budget      3 crate::session
  3 crate::routing      2 crate::git          2 crate::analytics   1 crate::vault
  1 crate::state        1 crate::diff_analysis 1 crate::cost        1 crate::commands
```

Now the reverse direction. Modules that would **stay** in `src-tauri` and reference the moving
set:
```
recipes        ->  4 crate::agent,  5 crate::provider          (9)
subagent       ->  4 crate::agent,  7 crate::provider         (11)
diff_analysis  ->                   2 crate::provider          (2)
commands       ->  8 crate::agent, 24 crate::provider, 6 crate::memory  (38)
```

**Four cycles.** Each is `core → tauri → core`, which cargo rejects outright:

| # | cycle | evidence |
|---|---|---|
| 1 | `agent`(core) → `recipes`(tauri) → `agent`(core) | agent→recipes 13 refs; `recipes/runner.rs` uses `crate::agent` |
| 2 | `agent`(core) → `subagent`(tauri) → `agent`(core) | agent→subagent 5 refs; `subagent/native_runner.rs` uses `crate::agent` |
| 3 | `agent`(core) → `commands`(tauri) → `agent`(core) | `agent/builtin_tools/analyze_diff_tool.rs:92` calls `crate::commands::diff_commands::generate_branch_diff_pub(` — **a builtin tool reaching up into the Tauri command layer** |
| 4 | `agent`(core) → `diff_analysis`(tauri) → `provider`(core) | agent→diff_analysis 1 ref; diff_analysis→provider 2 refs |

Cycle 3 is the single worst offender by `path:line`:
`src-tauri/src/agent/builtin_tools/analyze_diff_tool.rs:92`
```rust
        let patch = match crate::commands::diff_commands::generate_branch_diff_pub(
```

Second worst, because it is the one P1-06 is nominally about:
`src-tauri/src/agent/engine.rs:30`
```rust
use crate::state::AppState;
```
consumed at `agent/engine.rs:41` (`state: Arc<Mutex<AppState>>`) and `:58`.

Third: the 27 `crate::events` references spread over **13** `agent/` files (`engine.rs`,
`tool_executor.rs`, and 11 files in `builtin_tools/`).

Fourth, and the one nobody has costed: **31 `crate::db` references across 25 `agent/` files** —
23 of them in `builtin_tools/`. The task-board tools (`create_task`, `assign_task`,
`update_task_status_tool`, `claim_task_tool`, `list_tasks_tool`, …) are written directly against
`crate::db::tasks`. Moving `agent/` without moving `db/` breaks 25 files. Moving `db/` too is
exactly P1-09, which is scheduled **after** P1-05 in §P1-C.

#### What is genuinely easy

- **`platform/` — zero `crate::` references, zero `tauri::` references, 2 files, 375 LOC.** It is
  already written to be moved; `src-tauri/src/platform/mod.rs:1-6` says so in a comment:
  ```
  // Moves wholesale into `autodev-core/platform/` in P1-05, which is why it is a
  // directory module from the start rather than a single file.
  ```
  Nothing depends on it in either direction that I could find via `crate::platform` from the
  staying modules (0 hits). Movable today, in isolation.
- **`provider/` — 8 files, 1,974 LOC, one outbound dependency (`crate::errors`), no cycle.**
  Nothing in `provider/` references anything that stays behind except `errors`. Move `errors.rs`
  with it (or into a lower crate) and `provider/` moves cleanly. 31 inbound references
  (`commands` 24, `subagent` 7… plus recipes 5, diff_analysis 2) all point *into* the new crate,
  which is the correct direction.
- **`memory/` — 9 files, 1,640 LOC, two outbound (`crate::errors`, `crate::session`).** The
  `session` dependency is exactly two lines:
  ```
  memory/migration.rs:14:use crate::session::manager::SessionManager;
  memory/migration.rs:15:use crate::session::types::MessageRole;
  ```
  and `session/` itself has **zero** `tauri::` references and only `crate::errors` outbound — so
  `session/` can move down with it. No cycle.

#### What is genuinely hard

`agent/` is the whole problem, and the difficulty is **not** Tauri (6 lines) — it is that
`agent/` sits in the *middle* of the dependency graph, not at the bottom. It depends on 17
modules that would stay behind and is depended on by 4 of them. Any plan that says "move five
directories" and budgets for the Tauri types has mis-costed the task by an order of magnitude.

### C6 — events.rs OnceLock<AppHandle> + 132 emit sites / 27 files / 11 in builtin_tools

**Mechanism CONFIRMED. All three numbers are WRONG — and they are wrong in a way that matters,
because two of them are measuring different things and the claim presents them as one series.**

#### The mechanism — CONFIRMED, verbatim

`src-tauri/src/events.rs:5-25`:
```rust
use serde::Serialize;
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};

// Global AppHandle installed from main.rs after setup. Lets non-Tauri
// subsystems (task-board tools, recipes, hooks) emit fleet events without
// having to thread `AppHandle` through every call site. Fleet tools run in
// background tokio tasks with no Tauri `State<>` access, so this is the
// minimal indirection required for Phase 3.0 R1/R2.
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Install the global AppHandle exactly once. Called from main.rs `.setup()`.
pub fn install_app_handle(app: AppHandle) {
    let _ = APP_HANDLE.set(app);
}

/// Access the global AppHandle. Returns None if emission is attempted before
/// setup completes (tests, early boot). Callers must no-op on None.
pub fn app_handle() -> Option<&'static AppHandle> {
    APP_HANDLE.get()
}
```
Installed once, `main.rs:204`: `autodev_lib::events::install_app_handle(app.handle().clone());`
`events.rs` is 533 lines and defines **34** `pub fn emit_*` (`grep -c "^pub fn emit_" events.rs`),
of which **15** are `emit_*_global` wrappers (`events.rs:258, 271, 284, 299, 321, 346, 359, 382,
410, 431, 452, 466, 492, 516, 522`).

#### My own counts

| Metric | My count | Method |
|---|---|---|
| `emit_*(` **call sites**, excluding `events.rs` | **73** | `grep -rn "emit_[a-z_]*(" --include=*.rs \| grep -v events.rs \| wc -l` |
| …of which **false positives** | **2** | `graph/extract.rs:1366, :1378` call `emit_go_import`, a local closure declared at `graph/extract.rs:1322` — nothing to do with Tauri events |
| **Real** emit call sites | **71** | 73 − 2 |
| **Files** containing real call sites | **24** | 25 minus `graph/extract.rs` |
| Real call sites in `agent/builtin_tools/` | **21** | across **11** files |
| `emit_*_global(` call sites (no AppHandle param) | **41** | across **21** files |
| threaded `emit_*(app, …)` call sites | **30** | 32 − the 2 false positives |

#### Where the claim's numbers came from — and why each is wrong

- **"132 sites"** → the closest real figure is `grep -rn "emit_" --include=*.rs \| wc -l` =
  **131 grep-lines**, or `grep -roh "emit_" \| wc -l` = **141 substring hits**. Neither is a call
  site count. The 131 figure **includes `events.rs` itself** — i.e. it counts the 34 definitions
  and 11 doc-comment mentions (`grep -c "emit_" events.rs` = 45) as if they were call sites, plus
  the 2 `emit_go_import` false positives. **The real number is 71, roughly 54% of the claim.**
- **"27 files"** → this is exactly `grep -rl "emit_" --include=*.rs \| wc -l` = **27**, but that
  set includes `events.rs` (the definition file), `graph/extract.rs` (false positive), and
  `hooks/executor.rs`, whose *only* `emit_` is a comment — `hooks/executor.rs:2`:
  ```
  // Supports: shell, http, notification, log, block, emit_event.
  ```
  **The real number is 24 files.**
- **"11 inside builtin_tools/"** → **this is a FILE count sitting in a list of SITE counts.**
  `agent/builtin_tools/` has **21 call sites across 11 files**. Presented next to "132 sites",
  "11" reads as 8% of the emit surface when the real share is 21/71 = **30%**.

#### The part of C6 that is right, and undersold

The claim identifies the right coupling but describes the wrong shape of it. The important
structural fact — which no claim states — is the **41/30 split**:

- **41 of the 71 call sites (58%), across 21 files, go through `emit_*_global(payload)` and take
  no `AppHandle` argument at all.** Every one of the 21 `agent/builtin_tools/` sites is in this
  group (e.g. `agent/builtin_tools/planner.rs` has 10, all `emit_plan_changed_global(...)`).
  For these, the *call sites* are already Tauri-free in signature; only the `crate::events::`
  path they resolve through is not.
- **30 sites thread an `AppHandle` explicitly**, and they live in exactly four files:
  `recipes/runner.rs` (13), `agent/tool_executor.rs` (7), `agent/engine.rs` (5),
  `commands/plan_commands.rs` (5).

So inside the five directories proposed for extraction, only **12** emit sites (`tool_executor.rs`
7 + `engine.rs` 5) actually take an `AppHandle` parameter. That is the real blast radius of the
threaded path, and it is small. The 41 global sites need a trait/channel indirection but no
signature surgery at the call site. **This makes the events coupling more tractable than C6
implies — and it is still not the thing that blocks C5. See C5.**

### C7 — src-cli can link autodev-core

**PARTIALLY CONFIRMED — depends entirely on what goes into the crate. `provider/` + `memory/` +
`platform/`: yes. `agent/` as it stands today: NO, it drags Tauri in and breaks a CI job.**

#### What `src-cli` depends on today

`src-cli/Cargo.toml` declares exactly one path dependency:
```toml
[dependencies]
autodev-shared = { path = "../src-shared" }
```
plus clap, dialoguer, indicatif, colored, comfy-table, rustyline, serde, serde_json, toml, tokio,
futures, reqwest, chrono, uuid, anyhow, tracing, tracing-subscriber, zip, base64. **No Tauri.**

Verified with the dependency graph, not the manifest:
- `cargo tree -p airsctl --edges normal --prefix none | grep -i "^tauri"` → **zero matches**
  (282 unique dependency lines total).
- `cargo tree -p autodev --edges normal --prefix none | grep -ci "^tauri "` → **4** (sanity check
  that the grep works).

`autodev-shared` itself is Tauri-free (serde, serde_json, rusqlite, thiserror, anyhow, chrono,
uuid, toml, reqwest, tokio).

**Side finding, unasked:** `src-shared`'s own description says *"Shared types and database access
between AutoDev Tauri app and airsctl CLI"* — but **`src-tauri/Cargo.toml` does not depend on
`autodev-shared` at all** (grep for `autodev-shared`/`autodev_shared` in `src-tauri/Cargo.toml`
and `src-tauri/src/*.rs` → no hits). It is a CLI-only crate whose stated purpose is being shared
with a consumer that never links it. That is the mechanism behind P1-09's "four hand-mirrored
tables": the mirror exists because nothing forces the two to agree.

#### Would pulling in agent/provider/memory drag Tauri into the CLI?

| moved unit | drags Tauri? | why |
|---|---|---|
| `platform/` | **No** | only external `use` is `std` (1). Zero `crate::` refs. |
| `provider/` | **No** | externals are `futures`, `async_trait`, `tracing`, `std`, `serde_json`, `reqwest`, `ollama_rs`, `serde`. |
| `memory/` | **No** | externals are `tracing`, `rusqlite`, `chrono`, `uuid`, `std`, `serde`. |
| `agent/` | **YES** | `agent/engine.rs:9` and `agent/tool_executor.rs:7` are `use tauri::AppHandle;`, and 13 `agent/` files reach `crate::events`, whose header is `events.rs:7` `use tauri::{AppHandle, Emitter};` |

So the drag is real but narrow — 2 direct files plus the `crate::events` path. **Unless P1-06
lands first, P1-07 cannot ship `agent/` to the CLI.**

#### The consequence is concrete, not theoretical: it breaks a CI job

`.github/workflows/ci.yml:112-122`, the `cli-checks` job, installs only:
```
          sudo apt-get install -y libssl-dev pkg-config lld
```
whereas the `rust-checks` job (`ci.yml:44-54`) installs `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`,
`libayatana-appindicator3-dev`, `librsvg2-dev` on top of those. `cli-checks` then builds with
`--manifest-path src-cli/Cargo.toml` (`ci.yml:135, 140, 146`). **If `airsctl` transitively linked
`tauri`, `cli-checks` would fail on missing webkit2gtk/GTK system libraries**, while
`rust-checks` (which has them) stayed green — a confusing, half-red CI. Anyone doing P1-07 must
either keep Tauri out of the core crate or add the UI system deps to `cli-checks`, and the second
option defeats the point of a headless CLI.

---

## Ordering hazards

### H-1 (BLOCKING) — P1-05 before P1-01/02/03 makes the test suite go red

`src-tauri/src/reachability_gate.rs:160-175`:
```rust
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
```
`KNOWN_UNREACHABLE` (`:25-47`) lists `agent/builtin_tools/graph_tools.rs` and
`agent/builtin_tools/graph_prompts.rs`. `src` is fixed to the **`autodev` package's**
`CARGO_MANIFEST_DIR/src`. **If `agent/` moves to `autodev-core` first, both paths stop existing
under `src-tauri/src` and this test fails**, with an error message that actively misleads the
next reader ("If it was declared or deleted, remove the entry" — it was neither).

**Order: P1-01 → P1-02 → P1-03 → P1-04 must all land before P1-05.** Doing A before B defuses
this completely: once the two entries are removed from `KNOWN_UNREACHABLE`, only
`db/checkpoints.rs` remains, and `db/` is not in the moving set.

### H-2 (BLOCKING) — P1-05 before P1-09 breaks 25 files

`agent/` holds **31 `crate::db` references across 25 files**, 23 of them in `builtin_tools/`
(`create_task.rs`, `assign_task.rs`, `claim_task_tool.rs`, `update_task_status_tool.rs`,
`list_tasks_tool.rs`, `spawn_qc_review.rs`, …). P1-09 ("move the task/worker/plan schema into
`autodev-core`") is scheduled in §P1-C, **after** P1-05 in §P1-B. Moving `agent/` while `db/`
stays behind orphans all 25. **P1-09's schema move must precede, or be merged into, any move of
`agent/`.**

### H-3 (BLOCKING) — P1-07 before P1-06 breaks the `cli-checks` CI job

See C7. `agent/` transitively pulls `tauri`; `.github/workflows/ci.yml:119-122` gives the CLI job
no GTK/webkit system libraries. **P1-06 (event sink trait) must complete before P1-07 ships
`agent/` to `src-cli`.**

### H-4 — P1-05 leaves the moved code permanently unguarded

`reachability_gate.rs:112` walks only `CARGO_MANIFEST_DIR/src` for the `autodev` package. After
the move, a new orphan file inside `autodev-core` would be invisible to every gate. Nothing in
P1 says to move or generalise the gate. (The gate's own `assert!(files.len() > 100)` survives:
`src-tauri/src` has **238** `.rs` files today and the moving set is **72**, leaving 166.)

### H-5 — layout-coupled test that travels with the move

`src-tauri/src/agent/tool_trait.rs:170-172`:
```rust
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/agent/tool_executor.rs");
        let source = std::fs::read_to_string(&path).expect("read tool_executor.rs");
```
This survives the move **only if `autodev-core` reproduces the `src/agent/…` sub-path**. A
flattened layout (`autodev-core/src/tool_executor.rs`) makes it panic on `.expect(…)`. Same class
of hazard for `ci_gate.rs:18-23`, whose `repo_root()` is `CARGO_MANIFEST_DIR.parent()` — correct
for `src-tauri/`, wrong for any nested crate path like `crates/autodev-core/`. `ci_gate` stays
put, so it is safe *provided* the new crate is a sibling of `src-tauri`, not a child of a new
`crates/` directory.

### H-6 — P1-02 makes P1-16 strictly harder, and rests on a false premise

P1-02 ("Declare `db/checkpoints.rs` — **the undo feature is currently absent from the shipped
app**") and P1-16 ("Collapse the two checkpoint systems to one") are in tension. The premise is
false (C4: undo ships and works). Declaring the dark duplicate in P1-02 promotes a 280-line dead
file into a compiled second implementation that P1-16 then has to un-do. **P1-02 should be
resolved by P1-16's decision, not before it.** On the evidence in C4 the right resolution is
deletion, which would let P1-02 and P1-16 collapse into one task.

### H-7 — the `security/` reference inside P1-05 is unsatisfiable

P1-05 names `security/`; it does not exist and is created by P1-20 in §P1-D. Either P1-05 drops
it or P1-20 moves ahead of it. This is a documentation defect, not a build hazard, but it will
stop whoever executes P1-05 cold.

---

## Smallest workable sequence for the core extraction

The naive "move five directories" cannot compile (four crate cycles) and cannot even start
(`security/` absent). The graph does have a clean bottom, though — three of the four existing
directories are already near-leaves. Sequence, smallest first, each step independently
compilable and committable:

**Step 0 — correct the plan text (no code).** Drop `security/` from P1-05; note that P1-20
creates it. Fix `git/mod.rs:31` → `:21` in P1-02's supporting text and in
`reachability_gate.rs:42`. Retract "undo does not exist in the shipped binary"
(`reachability_gate.rs:7-8`, MDP P1-02). Replace P1-06's "132 sites / 27 files / 11" with the
measured 71 / 24 / 21-in-11.

**Step 1 — create `autodev-core` containing only `errors.rs`.** Every candidate directory depends
on it (`provider/` 6 refs, `memory/` 7, `agent/` 6) and it depends on nothing in-crate
(`errors.rs` imports only `thiserror`, and derives `From` for `std::io`, `rusqlite`, `reqwest`,
`serde_json`, `anyhow`). `src-tauri` re-exports it as `pub use autodev_core::errors;` so no call
site changes. This is the whole foundation and it is **72 lines**.

**Step 2 — move `platform/`.** Zero `crate::` references out, zero `tauri::`, **zero consumers
anywhere** (`grep -rn "platform::" src-tauri/src` → 0 hits). It cannot break anything because
nothing calls it. Its 9 unit tests (`platform/command.rs:224-338`) move with it and keep passing.
This is the free proof that the crate boundary works.

**Step 3 — move `provider/`.** One outbound dependency, already satisfied by Step 1. 8 files,
1,974 LOC, no Tauri. All 38 inbound references (`commands` 24, `subagent` 7, `recipes` 5,
`diff_analysis` 2) now point `src-tauri → autodev-core`, the correct direction. **This alone
satisfies the useful half of P1-07** — `src-cli` can link `autodev-core` for provider types
without any Tauri risk.

**Step 4 — move `session/` then `memory/`.** `session/` has zero `tauri::` and one outbound
(`crate::errors`), so it moves on Step 1's foundation. `memory/`'s only non-`errors` dependency
is two lines (`memory/migration.rs:14-15`, `crate::session::{manager::SessionManager,
types::MessageRole}`), satisfied once `session/` is below it. 9 files, 1,640 LOC.

**At this point `autodev-core` is real, both binaries can link it, and P1-07's headline is true —
without touching `agent/` at all.**

**Step 5 — break the four cycles, before moving anything else.** These are prerequisites, not
part of the move:
- 5a. `agent/builtin_tools/analyze_diff_tool.rs:92` — a builtin tool calls
  `crate::commands::diff_commands::generate_branch_diff_pub(`. Push that function down out of the
  command layer. **One call site; do this first, it is the cheapest cycle to kill.**
- 5b. `agent ⇄ recipes` (13 out / 9 in) and `agent ⇄ subagent` (5 out / 11 in) — invert both with
  traits defined in `autodev-core` and implemented in `src-tauri`, same shape as 5c.
- 5c. `agent ⇄ diff_analysis → provider` — dies automatically once `provider/` is in core
  (Step 3), since `diff_analysis`'s only involvement is 2 `crate::provider` refs.

**Step 6 — P1-06, the event sink.** Define `trait EventSink` in `autodev-core`; implement it over
`AppHandle` in `src-tauri`. Scope, measured: **41 `emit_*_global(` call sites in 21 files** become
sink calls (mechanical — they take no `AppHandle` today), and **12 threaded sites** in exactly two
`agent/` files (`tool_executor.rs` 7, `engine.rs` 5) lose their `AppHandle` parameter. The two
struct fields are `agent/engine.rs:40` and `agent/tool_executor.rs:30`; the two constructor
parameters are `agent/engine.rs:57` and `agent/tool_executor.rs:47`. That is the entire Tauri
surface inside the moving set.

**Step 7 — P1-09's schema move, then `agent/`.** Only after `db/tasks|workers|plans` are in core
does moving `agent/` stop orphaning 25 files. `agent/engine.rs:30`'s `use crate::state::AppState;`
is the last item and is what P1-06 nominally names.

**Net:** Steps 1–4 deliver a real `autodev-core` that `src-cli` links, with ~4,000 LOC moved, no
cycles, no Tauri, and no gate breakage. Steps 5–7 are where the actual difficulty is, and they
belong after P1-09, not before it.

---

## What I could not determine

1. **Whether `graph_tools.rs` compiles after fixing the 6 E0609s — I could not execute it.** The
   brief forbids editing source to test, and `cargo check` cannot see an undeclared module. My
   error list is static analysis against read signatures. It is complete *for the categories I
   checked* (field existence, arity, argument types, visibility, imports, move semantics), but I
   cannot rule out an inference-site error that only rustc would surface — e.g. a
   `partial_cmp`/closure type inference failure at `:44`, `:243`, `:369`, `:610-614`, or a
   `json!` value that fails to satisfy `Serialize` in a way I mis-read. **Treat "6 errors" as a
   floor, not a ceiling.**
2. **I did not run `cargo check` at all**, despite it being permitted. `src-tauri/gen/` exists and
   is **not** in `.gitignore` (I checked lines matching `gen|target|schemas`), and `tauri-build`
   writes generated schemas there. Running it would have written to the repository, which the
   READ-ONLY constraint forbids. Consequence: **I have not independently confirmed the workspace
   currently compiles green.** Every "would compile" statement is static reasoning.
3. **I did not run `cargo test`.** It is outside the permitted command list, and the suite touches
   live SQLite under `~/.autodev/` (`db/checkpoints.rs:67`, `git/checkpoint.rs:30`,
   `db/mod.rs:21`), so running it blind would have risked live data. I therefore **reimplemented
   `reachability_gate`'s logic read-only** (walk `src/`, collect `^(pub )?mod NAME;`, subtract
   roots) and got an orphan set of exactly three files — identical to `KNOWN_UNREACHABLE`. That
   is strong evidence the gate passes today, but it is my reimplementation, not the gate itself.
4. **Where a repaired graph tool would get its `GraphStore`.** `ToolContext`
   (`tool_trait.rs:49-63`) carries no store and no handle to one. `GraphStore::open(&Path)`
   exists (`graph/store.rs:66`) and `graph/registry.rs` exists, but I did not trace how the live
   `graph_commands.rs` obtains a store, so I cannot size the injection work for C3. **This is the
   single largest unpriced item in P3/BUG-GRAPH.**
5. **Whether `provider/`'s `ollama_rs` dependency is CLI-safe.** `provider/` uses `ollama_rs` (3
   `use` sites) and `reqwest` (3). I confirmed neither pulls Tauri via `cargo tree -p airsctl`
   only in the negative direction (airsctl has no Tauri today); I did not build a hypothetical
   core crate, so I cannot promise `ollama-rs 0.3.4` links cleanly into `airsctl`'s existing
   282-crate graph without a version conflict.
6. **Whether the four cycles are the *only* cycles.** I enumerated reverse references by grepping
   `crate::(agent|provider|memory|platform)` from each staying module. A cycle routed through a
   `pub use` re-export (e.g. `graph/mod.rs:23-32` re-exports) or through `autodev_lib::` rather
   than `crate::` would not appear in that grep. I did check `autodev_lib::` and found it only in
   `main.rs`. **Medium confidence the list is complete; not high.**
7. **The frontend side of the checkpoint feature.** I confirmed three of the five commands are
   invoked (`src/stores/gitStore.ts:238, 347, 352`) and two are not (`create_checkpoint`,
   `compare_checkpoints`). I did not determine whether the UI surfaces restore to the user or
   whether `create_checkpoint` being uncalled from the UI is intentional (the agent path calls
   `auto_checkpoint_before_write` instead, so it may simply be redundant).
8. **`platform/` has zero consumers — I could not determine whether that is intended.** 375 LOC,
   4 public functions, 9 passing unit tests, and `grep -rn "platform::"` → **0 hits** across
   `src-tauri/src`, `src-cli/src`, `src-shared/src`. It is compiled, tested, and never called.
   This is the same "green but inert" failure class as `graph_tools.rs`, one level up, and **no
   existing gate can see it** — `reachability_gate` checks only that a `mod` name exists. Whether
   this is pre-positioned work for a later phase or a second dropped feature needs the owner.
