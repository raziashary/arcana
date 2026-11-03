# P8-R3 — Adversarial verification of task group P8-I ("conversion" / old-architecture removal)

**Verifier role:** adversarial pre-build verifier. Default verdict is REFUTED when evidence is ambiguous.
**Repo:** `C:\AIRS Foundry\autodev\autodev`
**Scope:** P8-45 … P8-65 (task group P8-I).
**Mode:** read-only. This file is the only file written.

Report is written incrementally; sections appear in claim order as each is investigated.

---

## Status log

- Report file created. Beginning investigation.

---

## P8-45 — "Re-point TG-P4-31 first"

### VERDICT: **PARTIALLY CONFIRMED** — mechanism exact, trigger point wrong

**The mechanism is confirmed verbatim.** `src-tauri/src/ci_gate.rs`:

```
2810	    #[test]
2811	    fn tg_p4_31_no_recipe_is_mode2only_while_mode2_is_inert() {
2812	        let runner = read_source(&repo_root().join("autodev-core/src/recipes/runner.rs"));
2813	
2814	        // Is Mode 2 actually reachable? The runner discards the probe result
2815	        // and pins the flag.
2816	        let mode2_inert = runner
2817	            .lines()
2818	            .filter(|l| !l.trim_start().starts_with("//"))
2819	            .any(|l| l.contains("let orch_healthy = false"));
2820	
2821	        if !mode2_inert {
...
2825	            panic!(
2826	                "`orch_healthy` is no longer pinned to false, so Mode 2 may be ... live again. Re-point TG-P4-31 deliberately rather than letting it pass by default."
2827	            );
2828	        }
```

The cited span `2811-2828` is exact: the `fn` header is at 2811 and the `if !mode2_inert { … panic!(…) }` block closes at 2828. The literal read is at 2819. The line it looks for still exists — `autodev-core/src/recipes/runner.rs:174: let orch_healthy = false;`.

**Worse than stated in one respect:** the helper is

```
1237	    fn read_source(path: &std::path::Path) -> String {
1238	        std::fs::read_to_string(path)
1239	            .unwrap_or_default()
```

`unwrap_or_default()` — so if `runner.rs` were *deleted* the gate would also panic, with the same misleading message. There is no "file missing" branch.

### ⚠ REFUTED sub-claim: "fails CI on the first commit of `P8-B`"

`P8-B` is `P8-04` … `P8-12` (MDP §P8-B). Read them: `P8-04` binds `NoOrchestrator` in `main.rs`; `P8-05`–`P8-08` delete `src-tauri/src/sidecar/`, `sidecar_commands.rs`, the `lib.rs` registrations and `main.rs`'s `SidecarManager`; `P8-09`–`P8-12` touch `startup_commands.rs`, `worker_commands.rs`, `budget_commands.rs`, `runtime_config.rs`, `updater/*`, `mcp/mod.rs`, `logging.rs`, `events.rs`, `RuntimeConfig`, `src-cli/.../system.rs`.

**None of those files is `autodev-core/src/recipes/runner.rs`.** The gate reads only `runner.rs` and `defaults.rs`. `let orch_healthy = false;` is removed by **`P8-48`**, and `FallbackPolicy::Mode1Capable` (the `declared >= 9` floor at `ci_gate.rs:2847-2852`) by **`P8-47`** — both in `P8-I`, both listed *after* `P8-45`.

So the stated failure cannot occur where the task says it will. The **ordering constraint is still correct** (`P8-45` before `P8-47`/`P8-48`), but its stated justification is not, and a builder who trusts the justification will look for a CI break in the wrong commit and conclude the gate is fine.

Current state of the second half of the gate (so the remedy is sized correctly):
- `grep -c "FallbackPolicy::Mode2Only" autodev-core/src/recipes/defaults.rs` → **0**
- `grep -c "FallbackPolicy::Mode1Capable" autodev-core/src/recipes/defaults.rs` → **9**

So today `stranded == 0`, `declared == 9`, gate green. `P8-47` takes `declared` to 0 → the `declared >= 9` assert fires. That is the *real* first failure, and it is the one the ordering constraint must actually name.

### Would the remedy work?

"Rewrite it as a permanent invariant, then retire it in `P8-53`" — yes, but note the remedy is self-cancelling as written: after `P8-47` deletes `FallbackPolicy` entirely there is no invariant left for TG-P4-31 to hold, because the property it asserts ("no shipped recipe declares `Mode2Only`") becomes unstateable. The honest rewrite is not "permanent invariant" but "assert the enum and the field are gone" — i.e. the same shape as `TG-P3-39` at `ci_gate.rs:2877-2886`, which asserts a file *does not exist*. Retiring it in `P8-53` then makes sense; keeping it as a "permanent invariant" between `P8-47` and `P8-53` does not.

### Could not determine
Whether CI actually runs this test today. `ci_gate.rs` is `mod ci_gate;` at `src-tauri/src/lib.rs:42` and its header says it runs under `cargo test --all-targets`. I did not run the suite (read-only mandate) and did not open `.github/workflows/ci.yml` to confirm the invocation. Settle by reading the workflow's cargo test line.

---

## P8-46 — "PAMode is live; FallbackPolicy is the inert half"

### VERDICT: **CONFIRMED** (every cited line still says what the task says)

**`PAMode` selects the prompt** — `autodev-core/src/agent/context.rs:89-93`, exact:

```
89	        let base = match mode {
90	            super::PAMode::FastDevelopment => BASE_SYSTEM_PROMPT,
91	            super::PAMode::Review => REVIEW_SYSTEM_PROMPT,
92	            super::PAMode::Delegate => DELEGATE_SYSTEM_PROMPT,
93	        };
```

**`PAMode` selects the registry** — `autodev-core/src/agent/builtin_tools/mod.rs:190-284`, exact (`pub fn register_for_mode` opens at 190; the `match` closes at 284, the fn at 285):

```
190	pub fn register_for_mode(registry: &mut ToolRegistry, mode: PAMode) {
191	    match mode {
192	        PAMode::FastDevelopment | PAMode::Review => {
...
269	        PAMode::Delegate => {
270	            registry.register(Arc::new(create_task::CreateTaskTool));
...
282	            registry.register(Arc::new(propose_memory::ProposeMemoryTool));
283	        }
284	    }
```

**Both are reached on the interactive path** — `autodev-core/src/agent/engine.rs:82` `builtin_tools::register_for_mode(&mut registry, pa_mode);` and `engine.rs:231` `context_builder.build_system_prompt_parts(self.pa_mode, working_dir, user_message)`.

**`sessions.pa_mode` is the source of truth** — `autodev-core/src/session/storage.rs:67` `pa_mode     TEXT NOT NULL DEFAULT 'fast_development',` and `src-tauri/src/commands/agent_commands.rs:116-127`:

```
116	    // Phase 3.0 R3 — session row is the source of truth for PA mode. The
117	    // optional `pa_mode` argument from the frontend is accepted only as a
118	    // last-resort fallback …
122	    let session_mode = session.pa_mode.clone();
123	    let effective_mode_str: &str = if !session_mode.is_empty() {
```

**The warned-of harm is real and understated.** Deleting `PAMode` would not merely "hand the PA file and bash tools": the `Delegate` arm registers exactly **6** tools (`create_task`, `planner`, `search_memory`, `get_memory`, `list_memories`, `propose_memory`) against the FastDevelopment/Review arm's ~38 — so a name-match sweep grants `bash`, `write_file`, `edit_file`, `spawn_subagent`, `spawn_parallel_subagents` and `e2e_probe` in the mode whose own enum doc (`autodev-core/src/agent/mod.rs:31-32`) reads `/// No file tools, only create_task for delegating to workers`.

**Remedy adequacy:** `DOC-GATE` is the right evidence class, but a doc gate is not a guard. Nothing in `ci_gate.rs` today asserts that `Delegate` registers no file/execution tool. And note the hazard is not actually created by P8's own tooling: `P8-65` sweeps the strings `Mode 2` / `LivingAgent` / `core-orchestrator` / `memory-mcp-server`, none of which is a `PAMode` variant name. The warning is correct but guards against a human doing a manual "Mode 1 / Mode 2" cleanup, not against the phase's gate.

---

## P8-47 — Delete `FallbackPolicy`, `Recipe.fallback_policy`, the `Mode2Only` guard

### VERDICT: **CONFIRMED on location — INCOMPLETE on inventory**

Line numbers are exact. `autodev-core/src/recipes/runner.rs:717-737`:

```
717	        if matches!(
718	            recipe.fallback_policy,
719	            crate::recipes::types::FallbackPolicy::Mode2Only
720	        ) {
...
735	            self.set_worker_status(worker_slug, "fallback_idle");
736	            return Ok(());
737	        }
```

`autodev-core/src/recipes/types.rs:64-70` (enum, `#[default] Mode2Only`) and `types.rs:87-88` (`#[serde(default)] pub fallback_policy: FallbackPolicy,`).

### Sites the task does not name (all of which must change in the same commit)

1. **`autodev-core/src/recipes/runner.rs:1116`** — `fallback_policy: Default::default(),` in the in-file test `Recipe` builder. A construction site; the deletion breaks the test module, not just shipped code.
2. **Nine shipped literals** — `defaults.rs:232, 408, 677, 916, 1043, 1226, 1399, 1971, 2064`, all `fallback_policy: super::types::FallbackPolicy::Mode1Capable,`.
3. **Eight rationale comment blocks** in `defaults.rs` at `:221, :397, :666, :905, :1032, :1388, :1960, :2053` beginning `// why this recipe never ran. runner.rs hardcodes` `orch_healthy =` — these are `P8-59`'s "eight duplicated `P4-31` comment blocks" (verified below), and they die with this field, not with the recipe prose.
4. **`src-tauri/src/ci_gate.rs:2838-2859`** — `TG-P4-31`'s second half counts `FallbackPolicy::Mode2Only` / `Mode1Capable` in `defaults.rs`. `declared >= 9` fails the instant these literals go. This is the CI break that `P8-45` should have named.

### The knock-on the task misses: `fallback_idle` loses its only writer

`runner.rs:735` is the **only** Rust site that writes the worker status string `"fallback_idle"`. What I searched: `set_worker_status` across `autodev-core/src/recipes/runner.rs` (16 hits, listed with their literals) and a repo-wide ripgrep for `fallback_idle` restricted to `*.rs,*.ts,*.tsx,*.yaml,*.yml`. The frontend consumes it as a first-class state:
- `src/types/live.ts:55` — `| 'fallback_idle' | 'mode1_active'`
- `src/components/autodev/AgentLiveCard.tsx:39` — `fallback_idle: 'var(--warning)',`
- `src/stores/agentLiveStore.ts:402-403` — remaps `'mode1_idle_no_provider'` → `'fallback_idle'`

After `P8-47` the amber "fallback engaged but skipped per policy" state is only ever produced by the *remap* of `mode1_idle_no_provider` (`runner.rs:766`), which means something else. `P8-43` names `mode1_active` as a reader-with-no-writer; it does not name this. **Fold it into `P8-43` explicitly**, or the amber dot silently changes meaning.

### One thing the remedy gets right and should be recorded

`Recipe` is `#[derive(Debug, Clone, Serialize, Deserialize)]` with **no** `deny_unknown_fields` (`types.rs:73`). Existing on-disk `~/.autodev/recipes/*.yaml` carrying `fallback_policy: mode1_capable` will still deserialize after the field is removed, so no data migration is needed. Load-bearing and unrecorded.

---

## P8-50 — Delete `register_for_mcp_worker`

### VERDICT: **CONFIRMED** — zero callers, and the two comments are exactly two

**Line numbers exact.** `autodev-core/src/agent/builtin_tools/mod.rs:100` is `pub fn register_for_mcp_worker(registry: &mut ToolRegistry) {`; the fn closes at `:180`. That is **81 lines**; "~80" is fine.

**What I searched for callers:** ripgrep for the bare identifier `register_for_mcp_worker` across the entire repository, all file types, no path filter. Code-file hits, complete:

- `autodev-core/src/agent/builtin_tools/mod.rs:100` — the definition
- `autodev-core/src/agent/builtin_tools/set_task_budget_tool.rs:7` — comment
- `autodev-core/src/agent/builtin_tools/request_supervisor_action.rs:2` — comment

Every other hit is under `docs/`. **No call site anywhere in `autodev-core/`, `src-tauri/`, `src-cli/`, `src/`, or any `tests/` tree.** Zero callers: CONFIRMED.

**The two comments, verbatim:**

```
autodev-core/src/agent/builtin_tools/request_supervisor_action.rs:2
// Available to fleet workers via MCP (register_for_mcp_worker) so that Mode 2
```
```
autodev-core/src/agent/builtin_tools/set_task_budget_tool.rs:7
// Registered in register_for_mcp_worker() — available to Supervisor (and any
```

Both are `//` line comments, not `///` doc-comments. Loose but harmless.

**`pub` so no dead-code warning: CONFIRMED via the visibility chain** — `autodev-core/src/lib.rs:15 pub mod agent;` → `autodev-core/src/agent/mod.rs:7 pub mod builtin_tools;` → `pub fn`. Reachable from the crate root, so `dead_code` cannot fire. That is exactly why the function survived.

**⚠ Comments the task under-counts.** The function's own doc-comment (`mod.rs:86-99`) and an in-body block (`mod.rs:144-147`, "Exposed at the MCP-worker layer because Mode 2 LivingAgents reach all tools through the MCP server") die with the function, fine. But `set_task_budget_tool.rs:1` — `// set_task_budget — Supervisor MCP tool for assigning token/cost budgets to tasks.` — and `:8` — `// MCP-connected agent that needs budget management). NOT available in Delegate mode.` — survive, and are wrong in the same way as line 7. Fixing only line 7 leaves the same false claim one line above and one line below it.

---

## P8-48 — Delete the probe, the `orch_healthy` pin, and the unreachable branch

### VERDICT: **PARTIALLY CONFIRMED** — the structure is exact, the endpoint named in the task is wrong

**Lines 173-174 are exact.** `autodev-core/src/recipes/runner.rs`:

```
173	        let _orch_reachable = self.orch_client.is_healthy().await;
174	        let orch_healthy = false;
```

**"discarded on every worker tick" — CONFIRMED.** What I searched: `grep -rn "_orch_reachable" autodev-core/src src-tauri/src src-cli/src`. **Exactly one hit — line 173, the binding itself.** It is never read. `run_recipe` is the per-tick entry point (`runner.rs:114 pub async fn run_recipe(&self, recipe: &Recipe, worker_slug: &str)`), so the await is paid once per worker per tick and thrown away.

**"unreachable branch … lines 276-702, ~430 lines" — CONFIRMED, exactly 427 lines.**

```
276	        if orch_healthy {
702	            }
703	        } else {
```

`orch_healthy` is bound to the literal `false` eleven lines earlier and never reassigned, so the `if` body is statically dead. `awk 'NR>=276 && NR<=702' | wc -l` → **427**. The plan says ~430. Fine.

### ⚠ REFUTED sub-claim: it is not the `/api/system/health` probe

The task calls it "the awaited `/api/system/health` probe". The request actually goes to **`/health`**:

```
src-tauri/src/sidecar/client.rs:121
const HEALTH_PATH: &str = "/health";
```
```
src-tauri/src/sidecar/client.rs:456-457
    pub async fn is_healthy(&self) -> bool {
        let url = format!("{}{}", self.base_url, HEALTH_PATH);
```

And `client.rs:114-120` says so explicitly and deliberately:

```
114	// Phase 8.3 — Use the lightweight /health route (api_gateway/modules/ops/router.py)
115	// instead of the heavy /api/system/health route which calls
116	// SystemMCPServer._get_system_health() and competes with active LivingAgent
```

The plan inherited `/api/system/health` from a **stale comment**, `autodev-core/src/recipes/runner.rs:20-22`:

```
20	// The only legitimate Mode 1 path is `orch_healthy == false` at tick entry,
21	// which means the orchestrator process genuinely did not respond to the
22	// /api/system/health check after the grace-window retry policy in is_healthy().
```

That comment has been wrong since Phase 8.3 and the plan repeats it as fact. Small in itself, but it is exactly the failure mode `P8-60` is about — a comment describing a mechanism that isn't there — and here it propagated into the plan.

Practical consequence worth recording: on a machine with no orchestrator this is a `reqwest` GET to `http://127.0.0.1:4000/health` with `HEALTH_TIMEOUT_SECS = 10` (`client.rs:111`) **per worker per tick**. The cost claim is if anything understated.

### Would the remedy work?

Yes, and the cross-reference to `P8-42` is not merely prudent — it is mandatory. Every `emit_worker_tick_start` / `emit_worker_tick_end` call site in the runner is inside the region this task deletes or inside the region `P8-47` deletes:

| line | emitter | region |
|---|---|---|
| 287 | `emit_worker_tick_start` | inside 276-702 (`P8-48`) |
| 316, 348, 391, 415, 552, 634, 694 | `emit_worker_tick_end` | inside 276-702 (`P8-48`) |
| 727 | `emit_worker_tick_end` | inside 717-737 (`P8-47`) |

After `P8-47` + `P8-48` there are **zero** emitters left in `runner.rs`. `dashboardStore.ts:426` still cites `emit_worker_tick_end` as its source. So `P8-42` is a hard co-requisite of `P8-47` as well as of `P8-48`, and the plan only names `P8-48`.

### What I could not determine
Whether `#[allow(dead_code)]` or a clippy allow currently suppresses a warning on the dead branch — irrelevant to the deletion, but it would tell you whether the compiler has been silently flagging this. Settle with `cargo clippy --workspace --all-targets` (not run: read-only mandate).

---

## P8-49 — Delete `recipes/orchestrator.rs`, `is_orchestrator_ready`, the `orch_client` field

### VERDICT: **CONFIRMED** — and the "supersedes `P8-04`" note is right for a stronger reason than stated

**`is_orchestrator_ready` — zero callers: CONFIRMED.** What I searched: ripgrep for `is_orchestrator_ready` across the whole repository, all file types. Hits:

- `autodev-core/src/recipes/runner.rs:108` — the definition
- `docs/restructurization/_verify/A14-p8-legacy-sweep-tasks.md:61`, `docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md:2333` — planning prose
- `docs/development/DEVELOPMENT_PLAN.md:2379` and `:7466` — **stale**: `:7466` asserts *"`RecipeScheduler` also calls `runner.is_orchestrator_ready()` at startup-gate and immediate-startup-tick (lines 95 and 109)"*. It does not. There is no hit in `autodev-core/src/recipes/scheduler.rs` or anywhere else in any source tree.

Its own doc-comment still claims a caller — `runner.rs:106-107`:
```
106	    /// Returns true when the core-orchestrator is reachable and ready.
107	    /// Used by RecipeScheduler to gate the first tick (T5.1 — E-04 fix).
```
That is a false statement in shipped source, i.e. a `P8-60`-class comment the task list does not name.

**`NoOrchestrator` — zero production users: CONFIRMED, which strengthens the "supersedes `P8-04`" note.** Searched `NoOrchestrator` across `autodev-core`, `src-tauri/src`, `src-cli`, `e2e`. All five hits are in `autodev-core/src/recipes/orchestrator.rs` itself — the struct (`:67`), the impl (`:70`), one comment (`:15`), and its own two `#[cfg(test)]` tests (`:100`, `:105`). **Nothing binds it today.** So `P8-04` ("`main.rs` binds `NoOrchestrator` in place of `OrchestratorClient`") is not merely superseded — it would be *new work introduced and then deleted* inside the same phase. Doing `P8-04` at all is wasted motion; the plan should strike it, not just note the supersession.

**`orch_client` field and constructor parameter — exact.**
```
88	    orch_client: Arc<dyn OrchestratorProbe>, // Mode 2: see recipes::orchestrator
96	        orch_client: Arc<dyn OrchestratorProbe>,
102	            orch_client,
```
Sole construction site: `src-tauri/src/main.rs:359 let runner = Arc::new(RecipeRunner::new(`. One caller to update.

**`WorkerInvokeResult` has a live re-export that must die in the same commit** — `src-tauri/src/sidecar/client.rs:34 pub use autodev_core::recipes::orchestrator::WorkerInvokeResult;` and uses at `client.rs:893, 959, 987, 999, 1117, 1122`. Those all sit inside `src-tauri/src/sidecar/`, which `P8-05` deletes, so the ordering works — but only if `P8-05` lands first or in the same commit. If `P8-49` lands before `P8-05`, `src-tauri` will not compile. **The plan states no ordering between `P8-49` and `P8-05`, and its stated ordering constraint (`P8-45` before `P8-B`) is the wrong one.**

**Stale gate message left behind** — `src-tauri/src/ci_gate.rs:513`:
```
"autodev-core references Tauri in code. Core depends on traits the host implements — see events::EventSink and recipes::orchestrator — never on the host's framework: {}"
```
After `P8-49` that assertion message names a module that does not exist. It does not fail the gate (it is a message string), which is why nothing will catch it. `P8-65` scans for `core-orchestrator`, not `recipes::orchestrator`, so it will not catch it either.

---

## P8-51 — Split `sidecar_commands.rs` before deleting it

### VERDICT: **PARTIALLY CONFIRMED** — two of the three are what the task says; the third is misclassified, and the destination file does not exist

**"all 17" is right.** `grep -c '#\[tauri::command\]' src-tauri/src/commands/sidecar_commands.rs` → **17**, in a 570-line file.

**`get_mcp_health` and `force_reconnect_mcp` — CONFIRMED external-MCP-client, live.**
```
src-tauri/src/commands/sidecar_commands.rs:553-557
#[tauri::command]
pub async fn get_mcp_health(
    monitor: State<'_, Arc<crate::mcp::health::McpHealthMonitor>>,
) -> Result<Vec<crate::mcp::health::McpServerHealth>, String> {
```
```
src-tauri/src/commands/sidecar_commands.rs:562-568
pub async fn force_reconnect_mcp(
    server_name: String,
    monitor: State<'_, Arc<crate::mcp::health::McpHealthMonitor>>,
```
The monitor is real, installed and polling: `src-tauri/src/main.rs:226 let mcp_health_monitor = Arc::new(McpHealthMonitor::new(30, 3, 10));` and `:253 .manage(mcp_health_monitor)`, with emitters throughout `src-tauri/src/mcp/health.rs` (`:180 emit_serialized(EVT_MCP_HEALTH_UPDATE …)`, `:280, :295, :334, :400, :433, :440`). Frontend consumers: `src/stores/sidecarStore.ts:127` and `src/components/sidecar/McpHealthPanel.tsx:171`.

### ⚠ REFUTED sub-claim: `list_mcp_tools` is not an external-MCP-client command

```
src-tauri/src/commands/sidecar_commands.rs:274-277
#[tauri::command]
pub fn list_mcp_tools(registry: State<'_, Arc<ToolRegistry>>) -> Vec<McpToolInfo> {
    let mut tools: Vec<McpToolInfo> = registry
        .all()
```
The `State` it reads is the **in-process builtin tool registry**, built at `src-tauri/src/main.rs:144-145`:
```
144	    let mut mcp_registry = ToolRegistry::new();
145	    builtin_tools::register_all(&mut mcp_registry);
```
Nothing about an external MCP server enters this command. Its own frontend consumer says so explicitly — `src/components/sidecar/McpToolBrowser.tsx:3-6`:
```
3	// Lists every builtin tool AutoDev exposes to its agents (the canonical Rust
4	// ToolRegistry). Data comes from the `list_mcp_tools` Tauri command, which
5	// iterates the in-process ToolRegistry — NOT a JSON-RPC round-trip through
6	// the MCP server …
```
whereas the Rust doc-comment above it is stale in the opposite direction — `sidecar_commands.rs:271-273`:
```
271	/// Returns every tool exposed by `autodev-mcp-server`. The frontend renders
272	/// this list in the MCP tool browser so developers can inspect available
273	/// capabilities without connecting an external MCP client.
```
`autodev-mcp-server` was deleted in P0. So the command is a **builtin-tool browser wearing an MCP name**. It is live and must be kept — but filing it under "AutoDev's *client* connections to external MCP servers" is wrong, and moving it into `mcp_commands.rs` (a module whose header, `src-tauri/src/mcp/mod.rs:1-4`, states *"AutoDev connects to user-configured EXTERNAL MCP servers … This module owns that surface"*) **relocates the mislabelling instead of fixing it** — precisely the error `mcp/mod.rs:9-12` records having already been made once.

**Correct disposition:** `get_mcp_health` + `force_reconnect_mcp` → the external-MCP module. `list_mcp_tools` → a tool-registry command surface (or rename to `list_builtin_tools` and keep the old name as an alias, since the task's own constraint is "keeping the command names identical").

### ⚠ The destination file does not exist

`ls src-tauri/src/commands/` — there is **no `mcp_commands.rs`**. The nearest is `github_mcp_commands.rs`. The task's "Move them to `mcp_commands.rs`" reads as a move into an existing file; it is a file creation, and a new `mod mcp_commands;` in `src-tauri/src/commands/mod.rs`. Small, but a builder following the text will look for a file that is not there.

### ⚠ P8-51 and P8-54 contradict each other — see the next section

---

## P8-54 — Delete the sidecar-only frontend

### VERDICT: **PARTIALLY CONFIRMED** — `SystemHealth.tsx` is exactly as described; the rest of the prescription takes live UI down with it

**`SystemHealth.tsx` — "no importer": CONFIRMED.** What I searched: ripgrep for `SystemHealth` across all of `src/`. Every hit is inside `src/components/dashboard/SystemHealth.tsx` itself (`:1` header comment, `:14` `interface SystemHealthData`, `:22` `const SystemHealth: React.FC`, `:329` `export default SystemHealth;`). **No `import … SystemHealth` anywhere.** 329 lines, zero importers.

**The hardcoded mock — CONFIRMED verbatim**, `src/components/dashboard/SystemHealth.tsx:25-43`:
```
25	  // Mock system health data
26	  const healthData: SystemHealthData = {
...
31	        name: 'Memory MCP Server',
32	        status: 'healthy',
33	        uptime_percent: 99.9,
...
38	        name: 'Core Orchestrator',
39	        status: 'healthy',
40	        uptime_percent: 99.8,
```
So the plan's *"'Core Orchestrator — healthy — 99.8% uptime'"* is an accurate paraphrase of lines 38-40. It also fabricates Anthropic/OpenAI/Google API uptimes (`:45-60`), which the task does not mention and which are a worse liability than the orchestrator row, because those services still exist.

**The three named components exist and their mounts are real:**
- `src/components/sidecar/SidecarPanel.tsx:82` → mounted `src/tabs/Settings.tsx:55`
- `src/components/settings/SubmoduleStatusPanel.tsx:18` → mounted `src/tabs/Settings.tsx:58`
- `src/components/SidecarCrashBanner.tsx:20` → mounted `src/App.tsx:101`

### 🔴 The collision: deleting `SidecarPanel.tsx` deletes the only mount of `McpHealthPanel`

```
src/components/sidecar/SidecarPanel.tsx:25
import { McpHealthPanel } from './McpHealthPanel'
src/components/sidecar/SidecarPanel.tsx:408
      <McpHealthPanel />
```
`McpHealthPanel` is mounted **nowhere else** (searched `McpHealthPanel` across all of `src/`: the definition at `McpHealthPanel.tsx:156`, the import and mount above, and one comment in `sidecarStore.ts:214`). `McpHealthPanel.tsx:171` is the **only** caller of `force_reconnect_mcp` — the command `P8-51` preserves on the express grounds that it is *"a live feature with a live UI"*.

So as written, `P8-51` keeps a command whose UI `P8-54` deletes. **Neither task mentions `McpHealthPanel.tsx`.** The fix is small — re-mount `McpHealthPanel` directly in `Settings.tsx` where `SidecarPanel` was — but nothing in the plan says to, and the failure is silent: the app compiles, the command stays registered, the panel is simply gone.

`McpToolBrowser` is safe: `Settings.tsx:61` mounts it independently of `SidecarPanel`.

### ⚠ A second `SidecarCrashBanner`

There are **two** components with that name:
```
src/components/SidecarCrashBanner.tsx:20   export function SidecarCrashBanner()          → App.tsx:101
src/components/sidecar/SidecarPanel.tsx:585 function SidecarCrashBanner({ service })     → SidecarPanel.tsx:257
```
The second is a file-local helper inside `SidecarPanel.tsx` and dies with it. The task names only "`SidecarCrashBanner.tsx`". Not a defect, but a name collision in a delete-by-name task is worth writing down.

### ⚠ Trap check — `src/design/` is not touched by this task
`src/design/` uses "sidecar" in the AutoDesign sense (`src/design/contract/sidecar.ts:1 // PD0-08 — the sidecar invocation model.`, `SidecarIo`, `dist-sidecar/design/contract/main.js`, `npm run build:sidecar` at `src/design/contract/gates.test.ts:28,174,303`). **`P8-54`'s prescription names specific files and none of them is under `src/design/`, so this task is clean.** The exposure is elsewhere (see the closing section).

---

## P8-55 — Split `sidecarStore.ts`

### VERDICT: **REFUTED on the split line** — the task assigns the `startup:progress` listener to the wrong half, and omits the live half's actual contents

The premise ("two unrelated concerns") is right. The **partition the task states is wrong in the one place it matters**.

**`startup:progress` is the DEAD half, not the live one.** `src/stores/sidecarStore.ts:148-157`:
```
148	      const unlisten = await listen<StartupProgressPayload>(
149	        "startup:progress",
150	        (event) => {
151	          const { phase } = event.payload;
152	          // When orchestrator reaches these phases, mark sidecar as running
153	          if (phase === "orch_ready" || phase === "fleet_ready" || phase === "ready") {
154	            set({ status: "running", isConnected: true });
155	          }
156	        }
157	      );
```
It sets `status`/`isConnected` — the sidecar-boot state — from orchestrator phases. `P8-52` deletes `startup_progress.rs`, the emitter. Extracting this into `mcpHealthStore.ts`, as the task literally instructs, moves a dead listener for an event nothing will emit into the store that is supposed to hold the live feature.

**The live half is six `mcp:*` listeners plus `mcpDisconnected`, none of which the task names:**

| line | listener | purpose |
|---|---|---|
| 177 | `mcp:health-update` | full snapshot each ping cycle |
| 186 | `mcp:connection-lost` | |
| 201 | `mcp:reconnected` | |
| 217 | `mcp:connection-degraded` | |
| 234 | `mcp:reconnect-failed` | |
| 258 | `mcp:requires-restart` | stdio server needs manual restart |

plus state field `sidecarStore.ts:54 mcpDisconnected: string[];  // server names currently disconnected`, which is consumed by `src/components/layout/StatusBar.tsx:29 const mcpDisconnected = useSidecarStore(s => s.mcpDisconnected)`.

Following the task's wording — "extract `McpServerHealth`, `loadMcpHealth`, the `startup:progress` listener … **delete the rest**" — deletes all six `mcp:*` listeners and `mcpDisconnected`. That is the entire live feature. **This is the most dangerous single sentence in the group.**

### The importer list is incomplete: six, not four

What I searched: ripgrep `sidecarStore` across `src/`. Importers:

| file | what it uses | half |
|---|---|---|
| `src/components/layout/StatusBar.tsx:28,29` | `status`, `mcpDisconnected` | **both** |
| `src/components/layout/Sidebar.tsx:22` | `status` | dead |
| `src/tabs/Chat.tsx:104` | `status` | dead |
| `src/tabs/AutoDevPanel.tsx:1007` | `status`, `isConnected` | dead |
| **`src/App.tsx:26,58`** | `initEventListeners` | **both — and the only caller** |
| **`src/components/sidecar/McpHealthPanel.tsx:15,157`** | `McpServerHealth`, `McpConnectionState`, `mcpServerHealth`, `loadMcpHealth` | **live** |

`App.tsx:58 const initSidecarEvents = useSidecarStore(s => s.initEventListeners)` is the sole entry point for **every** listener in the file, live and dead alike. If the store is deleted without repointing `App.tsx`, the MCP health events stop being subscribed and `McpHealthPanel` renders whatever the one-shot `loadMcpHealth` returned. `McpHealthPanel.tsx` is the live half's only view. **Both are missing from the task's "update `StatusBar`, `Sidebar`, `Chat` and `AutoDevPanel`".**

### Would the remedy work?
Not as written. The corrected prescription is: `mcpHealthStore.ts` takes `McpConnectionState`, `McpServerHealth`, `mcpServerHealth`, `mcpDisconnected`, `loadMcpHealth`, and the **six `mcp:*` listeners** (renaming `initEventListeners` accordingly); everything keyed on `SidecarStatusStr` / `isConnected` / the `startup:progress` listener / the 5-second `checkConnectivity` poll (`:162-172`) is deleted; and the update list is **`App.tsx`, `StatusBar`, `Sidebar`, `Chat`, `AutoDevPanel`, `McpHealthPanel`**.
