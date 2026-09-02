# P3-D / P3-E adversarial verification — subagents and context strategy

Verifier: fresh-context adversarial review. Mandate: refute each claim; default REFUTED when
evidence is ambiguous. All code quoted verbatim with `path:line`.

Repo: `C:\AIRS Foundry\autodev\autodev` · read-only pass · date 2026-09-02.

Status: COMPLETE. No source file was modified; this report is the only file written.

## Verdicts at a glance

| Claim | Verdict | One line |
|---|---|---|
| P3-22 | **REFUTED** (already-done) | In-process tokio tasks; `PoolPermit` is an RAII drop-guard; 16 is `MAX_GLOBAL_CONCURRENT`; the leak was closed by audit F-02. Residual timing defect → C-1. |
| P3-23 | **REFUTED** | Depth is threaded from `AgentIdentity` through `ToolContext`; two hard caps exist and the pool's runs before any provider work. |
| P3-24 | **PARTIALLY CONFIRMED** | Timeout exists (300 s default, `native_runner.rs:221`). Parent→child cancellation genuinely does not propagate — children are detached `tokio::spawn` tasks. |
| P3-25 | **PARTIALLY CONFIRMED** | Cached-prefix, live-history and compaction layers all exist and are reached; `clear_tool_uses` does not exist at any layer, including the wire. |
| P3-26 | **ALREADY-DONE** | Reads `ModelInfo::context_window` (`engine.rs:343-353`); 200k survives only as the unknown-model fallback, and the lookup resolves for every shipped model. Two caveats → C-2, C-3. |
| P3-27 | **REFUTED** | `history = compacted;` (`engine.rs:371`). The value that *was* discarded — compaction's `usage` — is now folded into the turn total. |
| P3-28 | **REFUTED as stated** | `clear_tool_uses` / `clear_at_least` do not exist, and the claim's economic premise fails here: no message-region breakpoint is ever set, so a clear forces no cache write. |
| P3-29 | **CONFIRMED** (worse than stated) | grep bounds match count, never bytes; `list_directory`'s cap is inert on the non-recursive path and its `truncated` flag reports `false` exactly when the cap was exceeded. |

Ten additional defects are listed under **Claims I would add** (C-1 … C-10). The ones I would act on
first are **C-2** (every default subagent sends an Anthropic model id to whatever provider is
active), **C-8** (`tools` subset is documented, written everywhere, read nowhere — a containment
claim the code does not honour) and **C-1** (permit release runs after the blocking acquire).

---

## P3-22 · The subagent runner is not in-process on tokio tasks; its concurrency permit is not released by `Drop`, leaking until all spawning deadlocks after 16
**Verdict:** REFUTED (all three sub-claims), with a **residual live defect** in the release *timing* — see "Claims I would add" C-1.

**Evidence:**

The runner *is* in-process on tokio tasks:
```rust
// autodev-core/src/subagent/manager.rs:106-116
        let task =
            tokio::spawn(
                async move { native_runner::run(run_deps, run_id, run_cfg, prompt, tx).await },
            );

        // Launch the event pump (consumes rx, fans out to global bus + state).
        let pump_manager = self.clone();
        let pump_id = id.clone();
        tokio::spawn(async move {
            pump_manager.pump_events(pump_id, rx).await;
        });
```
```rust
// autodev-core/src/subagent/native_runner.rs:1-6
// Native in-process subagent runner.
//
// Replaces `subagent/bridge.rs`, which drove a create → invoke(SSE) → delete
// lifecycle against the Python `core-orchestrator` sidecar. Subagents now run
// on the same Rust `AgentEngine` as every other agent, inside this process.
```

The permit **is** released by `Drop`, on all three of its axes:
```rust
// autodev-core/src/subagent/pool.rs:45-72
/// Token returned by `SubagentPool::acquire`. Releases its slots on drop.
pub struct PoolPermit {
    _global: OwnedSemaphorePermit,
    ...
    _qc: Option<OwnedSemaphorePermit>,
    pool: Arc<SubagentPool>,
    parent: String,
}

impl Drop for PoolPermit {
    fn drop(&mut self) {
        let pool = self.pool.clone();
        let parent = self.parent.clone();
        ...
        tokio::task::spawn(async move {
            let mut counts = pool.per_parent.lock().await;
            if let Some(c) = counts.get_mut(&parent) {
                *c = c.saturating_sub(1);
```
`_global` / `_qc` are `OwnedSemaphorePermit`s — tokio returns those to the semaphore in their own `Drop`; the per-parent counter is decremented by the hand-written `Drop` above.

`16` is the right number, and it is a *back-pressure* cap, not a rejection cap:
```rust
// autodev-core/src/subagent/pool.rs:26
pub const MAX_GLOBAL_CONCURRENT: usize = 16;
```
```rust
// autodev-core/src/subagent/pool.rs:140
        let global_permit = match self.global.clone().acquire_owned().await {
```

The leak the claim describes was **found and fixed before this phase**, and the fix is documented in place:
```rust
// autodev-core/src/subagent/manager.rs:266-276
                        // Stamp the terminal time HERE as well as in
                        // `finalise()`. `prune_terminal` only evicts entries
                        // with `terminated_at` set, and `finalise` runs only
                        // from `await_result` — so a subagent that completed
                        // but was never awaited stayed in the registry forever
                        // and, because `_permit` is a drop-guard held by the
                        // entry, never released its pool slot. Found by the P0
                        // post-phase audit (F-02).
                        if status.is_terminal() && entry.terminated_at.is_none() {
                            entry.terminated_at = Some(Instant::now());
                        }
```

**Reasoning:** every clause of the claim is contradicted by current code. The runner is a `tokio::spawn`ed in-process `AgentEngine` run; the permit is an RAII drop-guard over two tokio semaphores plus a manual counter decrement; 16 is `MAX_GLOBAL_CONCURRENT`; and the specific leak (terminal-but-never-awaited entries pinning permits forever) was already closed by the P0 audit's F-02 fix. Verdict REFUTED / effectively ALREADY-DONE.

**If the fix were applied as written:** rewriting the runner "to be in-process on tokio tasks" and "adding a `Drop` impl to the permit" is a no-op at best; at worst a second release path double-releases the semaphore (hand-calling `add_permits` alongside an `OwnedSemaphorePermit` drop inflates the cap permanently, silently removing the concurrency limit). Do not apply.

**Residual:** the permit is not released when the run ends — it is released when the *registry entry* is dropped, which happens only inside `prune_terminal`, reachable only from `spawn()` (AFTER the blocking `acquire`) and from `list()`. The claim's *symptom* ("spawning stalls once 16 are outstanding") can therefore still occur, for a different reason than the claim states. See C-1.

---

## P3-23 · `depth + 1` is not taken from the parent's real context, and there is no hard cap on nesting
**Verdict:** REFUTED — both halves are false in current code, and both were fixed with named regression tests.

**Evidence:** depth comes from the parent's threaded identity, not from a literal and not from the model:
```rust
// autodev-core/src/agent/tool_executor.rs:129-134
        let ctx = ToolContext {
...
            depth: self.identity.depth,
            agent_role: self.identity.role.clone(),
```
(three `ToolContext` construction sites, all identical in this respect — `tool_executor.rs:129`, `:318`, `:411`)

```rust
// autodev-core/src/agent/builtin_tools/spawn_subagent.rs:173
            depth: ctx.depth + 1, // K.11 — increment nesting depth
```
```rust
// autodev-core/src/agent/builtin_tools/spawn_parallel_subagents.rs:175
                depth: ctx.depth + 1, // K.11 — increment nesting depth
```

The child then runs at exactly that depth (no second increment):
```rust
// autodev-core/src/subagent/native_runner.rs:306-319
fn identity_for(_subagent_id: &str, config: &SubagentConfig) -> AgentIdentity {
...
    AgentIdentity {
        role,
        depth: config.depth,
    }
}
```

There are **two** hard caps, and the pool one is reached before any provider work:
```rust
// autodev-core/src/subagent/pool.rs:32
pub const MAX_NESTING_DEPTH: u32 = 3;
```
```rust
// autodev-core/src/subagent/pool.rs:96-100
        if depth > MAX_NESTING_DEPTH {
            return Err(AutoDevError::Agent(format!(
                "subagent rejected: depth {depth} exceeds MAX_NESTING_DEPTH={MAX_NESTING_DEPTH}"
            )));
        }
```
```rust
// autodev-core/src/agent/builtin_tools/spawn_subagent.rs:85-91
        const MAX_SUBAGENT_DEPTH: u32 = 3;
        if ctx.depth >= MAX_SUBAGENT_DEPTH {
            return ToolResult::error(format!(
                "Subagent depth limit ({MAX_SUBAGENT_DEPTH}) reached — ...
```
`manager.rs:78` calls `self.pool.acquire(&config.parent_slug, config.depth).await?` as the first statement of `spawn`, so the pool guard is not dead code on the depth axis.

**Reasoning:** the claim describes the *pre-restructure* design, which the code documents as removed:
```rust
// autodev-core/src/subagent/native_runner.rs:9-14
//   1. The nesting guard becomes real. The old path carried the child's depth
//      in its PROMPT — `bridge.rs` injected "Include `__depth: {n}` in every
//      tool call" ... so `MAX_NESTING_DEPTH` was enforced by asking the model to
//      report its own recursion depth honestly.
```
Both properties the claim denies are now enforced in Rust and covered by regression tests (`native_runner.rs:349-391`, `tool_trait.rs:129-211`).

**If the fix were applied as written:** adding another `+1` at the runner (the usual shape of "take depth from the parent's real context") re-introduces audit finding F-21 verbatim — a depth-0 parent's child runs at depth 2 and the cap fires after half the intended generations. The regression test `a_subagent_runs_at_exactly_the_depth_its_config_names` (`native_runner.rs:350`) would fail. Do not apply.

**One real defect sits next to this claim** (C-4): the guards disagree. All three spawn tools refuse at `ctx.depth >= 3` on the *parent's* depth (`spawn_subagent.rs:86`, `spawn_parallel_subagents.rs`, `spawn_qc_review.rs:80`), while the pool refuses at `depth > 3` on the *child's* depth (`pool.rs:96`) — so the tool guard is strictly tighter and the pool's depth branch can never fire through any builtin tool. (I checked the obvious gap and it is not there: `spawn_qc_review` does carry its own depth guard, at `spawn_qc_review.rs:80`.)

---

## P3-24 · Subagents have no timeout and no propagated cancellation
**Verdict:** PARTIALLY CONFIRMED — the timeout exists and is reached (that half REFUTED); parent→child cancellation genuinely does not propagate (that half CONFIRMED).

**Evidence — timeout exists, is defaulted, and wraps the run:**
```rust
// autodev-core/src/subagent/native_runner.rs:221-234
    let history = match timeout(
        Duration::from_secs(config.timeout_secs.max(1) as u64),
        run_future,
    )
    .await
    {
        Ok(result) => result?,
        Err(_) => {
            return Err(AutoDevError::Agent(format!(
                "Subagent exceeded its {}s timeout",
                config.timeout_secs
            )))
        }
    };
```
```rust
// autodev-core/src/subagent/manager.rs:84-86
        if config.timeout_secs == 0 {
            config.timeout_secs = SubagentConfig::DEFAULT_TIMEOUT_SECS;
        }
```
```rust
// autodev-core/src/subagent/types.rs:67
    pub const DEFAULT_TIMEOUT_SECS: u32 = 300;
```
Dropping `run_future` on timeout cancels the child engine's turn loop at its next await point, so this is a real stop, not merely an error return. A second ceiling also applies (`native_runner.rs:206`, `.with_budget(crate::budget::TaskBudget::default_worker())` = 100k tokens / $0.50 / 20 turns).

**Evidence — cancellation does not propagate from parent to child:**
The child runs on a **detached** `tokio::spawn` (`manager.rs:106`). The parent's tool call only awaits the `JoinHandle`:
```rust
// autodev-core/src/agent/builtin_tools/spawn_subagent.rs:183
        let result: SubagentResult = match mgr.await_result(&handle.id).await {
```
Dropping that future (parent turn aborted, parent engine timed out, user cancels the parent) drops the `JoinHandle`, which in tokio **detaches** the task rather than aborting it. Nothing in the parent path calls `abort`. The only aborts on the whole subagent surface are user-initiated:
```rust
// autodev-core/src/subagent/manager.rs:216-221
        if let Some(t) = task {
            if force {
                t.abort();
            }
```
reachable only from the Tauri commands `terminate_subagent` / `abort_subagent` (`src-tauri/src/commands/subagent_commands.rs:96-117`). `spawn_qc_review` compounds it by design — it detaches a *second* task that takes ownership of the join handle:
```rust
// autodev-core/src/agent/builtin_tools/spawn_qc_review.rs:221-223
        tokio::spawn(async move {
            match mgr_for_bg.await_result(&subagent_id_for_bg).await {
```
so a QC reviewer has no cancellation path at all short of the UI command, and its parent (the dispatcher tick) has returned long before it finishes.

**Reasoning:** "no timeout" is false. "No propagated cancellation" is true: children are detached tasks, and a cancelled or aborted parent leaves them running to their own 300 s timeout, still spending against the daily cap.

**If the fix were applied as written:** adding a timeout duplicates an existing one — harmless if identical, actively wrong if the new one is shorter, because it would silently pre-empt the `timeout_secs` the caller/LLM set. Adding propagated cancellation is worth doing and is not a no-op: it needs a `CancellationToken` (or a parent-scoped guard that aborts its children on drop) threaded from the parent engine through `SubagentManager::spawn`. The lineage field it needs already exists (`config.parent_session_id`, used today only as a `list()` filter at `manager.rs:186-190`); nothing indexes the registry by it, so a `terminate_children(parent_session_id)` helper is the smallest correct addition.

---

## P3-25 · There is no layered context assembly (cached prefix → live history → `clear_tool_uses` → compaction)
**Verdict:** PARTIALLY CONFIRMED — three of the four layers exist and are reached on every run; the `clear_tool_uses` layer does not exist anywhere in the repo.

**Evidence — the cached-prefix layer exists and is taken:**
```rust
// autodev-core/src/agent/engine.rs:147-161
        let (stable_prompt, volatile_prompt) = match system_prompt_override {
            Some(override_str) => (override_str.to_string(), String::new()),
            None => context_builder.build_system_prompt_parts(self.pa_mode, working_dir),
        };

        // What actually goes on the wire, in the order the cache requires:
        // stable first, volatile last.
        let system_prompt = if volatile_prompt.is_empty() {
            stable_prompt.clone()
        } else {
            format!("{stable_prompt}\n\n---\n\n{volatile_prompt}")
        };
```
```rust
// autodev-core/src/agent/engine.rs:171-187
        let caps = self.provider.capabilities(model);
        let cache_prefix = crate::cache::prefix::build(
            &tool_defs
                .iter()
                .map(|t| (t.name.clone(), t.input_schema.to_string()))
                .collect::<Vec<_>>(),
            ...
            &stable_prompt,
            &[],
            std::slice::from_ref(&volatile_prompt),
        );
```
The prefix builder is the single assembler for the cached region (`autodev-core/src/cache/prefix.rs:74-116`), sorts tools for byte stability (`prefix.rs:89-96`), and the policy is decided once per run and applied per turn (`engine.rs:191-222`, `request.cache = cache_policy.clone()` at `engine.rs:418`). The breakpoint reaches the wire (`autodev-core/src/provider/anthropic.rs:169-176`).

**Evidence — the live-history layer and the compaction layer exist:**
`history` is threaded through the loop and cloned into each request (`engine.rs:390`, `messages: history.clone()`), and compaction runs at the top of every turn against the model's real window (`engine.rs:343-380`, quoted under P3-26/27).

**Evidence — `clear_tool_uses` does not exist:**
```
$ grep -rn "clear_tool_uses\|clear_at_least\|ClearToolUses" --include=*.rs --include=*.ts .   (node_modules excluded)
(no matches)
```
Nor is there any wire-level support for it: the Anthropic adapter sends no `anthropic-beta` header and no `context_management` block —
```
$ grep -n "cache_control\|CachePolicy\|breakpoints\|context_management\|anthropic-beta\|beta" autodev-core/src/provider/anthropic.rs
6:use super::usage::CachePolicy;
152:            // Anthropic accepts both, but `cache_control` can only be attached
170:            if let CachePolicy::Explicit { ref ttl, .. } = req.cache {
171:                block["cache_control"] = json!({
```

**Reasoning:** stated as "there is no layered context assembly", the claim is false — the layering, ordering rationale and cache-safety of the prefix/volatile/history stack is the substance of P2-13/14/16 and it is live. Stated as "the `clear_tool_uses` stage is missing", it is true and total: neither the helper, nor a caller, nor the provider-side beta that would carry it exists.

**If the fix were applied as written:** rebuilding "layered context assembly" wholesale would re-open the exact defect P2 closed twice (`engine.rs:134-146` documents D-12: the split existed, was unit-tested, and was never taken in production, so memory sat inside the cached region). The only additive part worth doing is the tool-output clearing stage — and see P3-28 for why its economics in *this* codebase are not what the plan assumes.

---

## P3-26 · The compaction trigger reads a hardcoded 200,000 rather than `ModelInfo::context_window`
**Verdict:** ALREADY-DONE — fixed by P2-57, and the lookup that P2-57 depended on was *itself* repaired by P2-31 (the catalogue). It now succeeds for every model the app ships.

**Evidence — what it reads now:**
```rust
// autodev-core/src/agent/engine.rs:343-357
            let model_context = self
                .provider
                .models()
                .iter()
                .find(|m| m.id == model)
                .map(|m| m.context_window)
                // A model the adapter does not list falls back to the smallest
                // window we ship rather than the largest: compacting early is
                // wasteful, compacting late overflows the request and fails the
                // turn outright.
                .unwrap_or(200_000);
            if CompactionEngine::needs_compaction(
                CompactionEngine::estimate_tokens(&history),
                model_context,
            ) {
```
200,000 survives only as the *fallback for an unknown model*, and the threshold applies to whatever window is found:
```rust
// autodev-core/src/agent/compaction.rs:44-46
    pub fn needs_compaction(estimated_tokens: u32, context_window: u32) -> bool {
        estimated_tokens as f32 / context_window as f32 > COMPACTION_THRESHOLD
    }
```

**Evidence — the lookup actually succeeds for shipped models.** The chain is `provider.models()` → `models_static()` → the shared catalogue:
```rust
// autodev-core/src/provider/anthropic.rs:205-207
    fn models(&self) -> Vec<ModelInfo> {
        Self::models_static()
    }
```
```rust
// autodev-core/src/provider/anthropic.rs:45-47   (google.rs:42-44 and openai.rs:43-45 identical but for the string)
    fn models_static() -> Vec<ModelInfo> {
        crate::provider::catalogue::models_for("anthropic")
    }
```
```rust
// autodev-core/src/provider/catalogue.rs:244-248
pub fn models_for(provider: &str) -> Vec<ModelInfo> {
    CATALOGUE
        .iter()
        .filter(|e| e.provider == provider)
        .map(|e| {
```
and the ids in `CATALOGUE` are exactly the ids the picker offers (`src/data/models.ts` `MODEL_GROUPS`: `gemini-3.1-pro-preview`, `gemini-3.7-flash`, `claude-opus-5`, `claude-sonnet-5`, `claude-fable-5`, `claude-haiku-4-5`, `gpt-5.5`, `gpt-5.4`, `gpt-5.4-mini`, `gpt-5.4-nano`), so `find(|m| m.id == model)` hits. Gemini's 1,000,000-token window is reachable (`catalogue.rs:90`, `:104`), which is the precise failure P2-57 was written for and did not originally fix:
```
// autodev-core/src/provider/catalogue.rs:20-25
//   * `P2-57` reads `context_window` from `models()` so compaction stops
//     hardcoding 200k. Against the stale list it found no row for
//     `gemini-3.1-pro` and fell through to the 200k default — so the fix was
//     inert for every model we ship ... **A fix that lands in the right place
//     and is never reached is not a fix**
```
The decorator every subagent call goes through forwards `models()` unchanged, so the wrapper does not break the lookup:
```rust
// autodev-core/src/provider/capped.rs:115-117
    fn models(&self) -> Vec<ModelInfo> {
        self.inner.models()
    }
```

**Reasoning:** the claim describes code that no longer exists. Both the read and the table it reads from are current, and I verified the wrapper in the subagent path does not silently flatten it.

**Two caveats that keep this from being a clean "no action":**
1. The lookup is **provider-scoped**, while the identical fact is available **model-keyed** and already in scope four lines earlier as `caps` (`engine.rs:171`, `self.provider.capabilities(model)` → `catalogue::capabilities_for(model)`, `traits.rs:34-36`). If the active provider ever does not serve the selected model, `find` misses and compaction silently reverts to 200k — for Gemini that is compaction at 16% of the real window, the original bug, reachable again through a different door. See C-2, which shows that mismatch is not hypothetical.
2. `ModelCapabilities::context_window` (`catalogue.rs:279`, `:289`) has **zero production readers** — `grep -rn "\.context_window"` over `autodev-core/src` + `src-tauri/src` returns exactly one non-catalogue, non-assert hit: `engine.rs:348`, which reads `ModelInfo`, not `ModelCapabilities`. The catalogue test that claims to guard this behaviour asserts on the *unread* field:
```rust
// autodev-core/src/provider/catalogue.rs:374-380
    fn geminis_full_window_is_reachable() {
        // The bug this table was built for: `P2-57` reads the window from the
        // catalogue ...
        assert_eq!(capabilities_for("gemini-3.1-pro-preview").context_window, 1_000_000);
```
Both fields are fed from the same `CATALOGUE` row, so the test is not *wrong* — but it does not exercise the production path (`provider.models().find(id == model)`) at all, so a break in that path (a provider filter change, a model/provider mismatch, a wrapper that stops delegating) leaves it green. See C-3.

**If the fix were applied as written:** re-plumbing the trigger to `ModelInfo::context_window` is a literal no-op — that is what line 348 already reads. Time is better spent on caveat 1 (switch the read to `caps.context_window`, one line, removes the provider-scoping hazard and gives `ModelCapabilities::context_window` its first reader).

---

## P3-27 · The caller discards the compacted history rather than using it
**Verdict:** REFUTED — the caller assigns it, and also folds in the usage that *used* to be discarded.

**Evidence:**
```rust
// autodev-core/src/agent/engine.rs:368-379
                let (compacted, compaction_usage) =
                    CompactionEngine::compact_with_summary(history, self.provider.as_ref(), model)
                        .await;
                history = compacted;
                if compaction_usage.total() > 0 {
                    info!(
                        "Compaction cost {} input / {} output tokens",
                        compaction_usage.total_input(),
                        compaction_usage.output
                    );
                    total_usage.add(&compaction_usage);
                }
```
`compact_with_summary` takes `messages` **by value** (`compaction.rs:113-117`), so the pre-compaction `history` is moved and cannot be used by accident; the only binding left is the compacted one. `history` is then what each request carries (`engine.rs:390`, `messages: history.clone()`).

**Reasoning:** the claim is false on its face for the history. It is worth recording that the *discard* the claim describes did exist, on the adjacent value, and was fixed in the same phase:
```rust
// autodev-core/src/agent/compaction.rs:99-104
    /// This performs a real, billed `complete()` call, and its `usage` was
    /// **discarded** — the response was consumed for its text and dropped. So
    /// compaction spent money that never reached the cost tracker, never
    /// counted toward the daily ceiling, and never appeared in analytics.
```
That is the most likely origin of this claim, and it is closed: the usage is returned (`compaction.rs:117`), recorded via `usage_recorded` (`compaction.rs:177`), and added to the turn total (`engine.rs:378`).

**If the fix were applied as written:** a no-op at best. A re-assignment added in the wrong place (e.g. reassigning `history` after the loop body already mutated it) would drop the turn's tool results.

---

## P3-28 · `clear_at_least` is untuned, such that a clear may not exceed the cache-write it forces
**Verdict:** REFUTED as stated — neither `clear_tool_uses` nor `clear_at_least` exists in this repo, so there is no parameter to tune. The underlying capability is genuinely absent (that part of P3-25 stands), but the economic premise the claim reasons from does **not** hold in this codebase.

**Evidence — the symbols do not exist:**
```
$ grep -rn "clear_tool_uses\|clear_at_least\|clear_tool\|ClearToolUses" --include=*.rs --include=*.ts .   (node_modules excluded)
(no matches — exit 0, empty)
```
Nothing in `autodev-core/src/cache/` implements or references tool-output clearing; `cache/` contains exactly `mod.rs` (17 lines), `monitor.rs` (403) and `prefix.rs` (319), and `prefix.rs` assembles **tools + system** only:
```rust
// autodev-core/src/cache/prefix.rs:74-79
pub fn build(
    tool_names_and_schemas: &[(String, String)],
    system_base: &str,
    instructions: &[String],
    volatile_blocks: &[String],
) -> CachePrefix {
```

**Evidence — the message region is not cached at all today, so a clear forces no cache write:**
```rust
// autodev-core/src/agent/engine.rs:194-205
                    crate::provider::usage::CachePolicy::Explicit {
                        // One breakpoint, at the end of the system block. The
                        // four-breakpoint budget is worth spending only where
                        // there are distinct regions with different lifetimes;
                        // here there is one.
                        breakpoints: vec![0],
```
and the adapter puts that one breakpoint on the system block, never on a message:
```rust
// autodev-core/src/provider/anthropic.rs:169-176
            let mut block = json!({ "type": "text", "text": sys });
            if let CachePolicy::Explicit { ref ttl, .. } = req.cache {
                block["cache_control"] = json!({
                    "type": "ephemeral",
                    "ttl": ttl.as_wire_str(),
                });
            }
            body["system"] = json!([block]);
```
(Note the destructure: `breakpoints` is *ignored* here. `grep -rn "breakpoints"` finds one production write — `engine.rs:199` — two test writes, and **no reader**. See C-6.)

**Reasoning:** the claim assumes a design where clearing tool results invalidates a cached *messages* prefix and therefore forces a re-write that must be paid back by the clear. In the shipped design nothing after the system block is ever marked for caching, so clearing tool outputs costs zero cache-writes and the sizing question the claim raises does not arise — *until* someone also adds message-region breakpoints. Verifying "tuning" of a non-existent constant is not possible; verifying the premise is, and the premise is false here.

**If the fix were applied as written:** "tune `clear_at_least`" has nothing to act on and would be recorded as done against no code. If the intent is to *add* tool-output clearing, the honest sizing input for this codebase is the estimator and the compaction threshold, not a cache-write cost: `estimate_tokens` is ±29% (`cache/prefix.rs:51-65`, measured against `count_tokens` on 2026-09-02) and compaction fires at 0.80 of the window (`compaction.rs:31`). Any `clear_at_least` chosen here must exceed the estimator error on the history it measures, or the clear can fire without measurably moving the trigger it is meant to postpone.

---

## P3-29 · Tool outputs are unbounded: grep does not bound per line, and `list_directory` does not actually bound entries despite appearing to
**Verdict:** CONFIRMED — both halves, and the `list_directory` half is worse than the claim says: the cap is fully inert on the non-recursive path, and the `truncated` flag it sets is *false* precisely when the cap has been exceeded.

**Evidence — grep bounds the match COUNT, never the bytes:**
```rust
// autodev-core/src/agent/builtin_tools/grep.rs:242-255
                    file_matches.push(GrepMatch {
                        line_number: line_num + 1,
                        line_content: line.to_string(),
                        match_positions,
                        context_before,
                        context_after,
                    });

                    total_matches += 1;

                    if total_matches >= params.max_results as usize {
                        truncated = true;
                        break;
                    }
```
`line_content` is the whole line, verbatim. So are both context arrays:
```rust
// autodev-core/src/agent/builtin_tools/grep.rs:233-240
                    let context_before: Vec<String> = lines[ctx_start..line_num]
                        .iter()
                        .map(|s| s.to_string())
                        .collect();
                    let context_after: Vec<String> = lines[line_num + 1..ctx_end]
                        .iter()
                        .map(|s| s.to_string())
                        .collect();
```
and so is the position list, one tuple per occurrence on the line:
```rust
// autodev-core/src/agent/builtin_tools/grep.rs:223-226
                let all_matches: Vec<(usize, usize)> = regex
                    .find_iter(line)
                    .map(|m| (m.start(), m.end()))
                    .collect();
```
The default cap is 100 matches (`grep.rs:55-57`). One minified bundle, one generated `.lock`, one embedded-data line — the repo has a 522 KB `package-lock.json` and a 209 KB `Cargo.lock` at its root — and a single match returns that line three times over (before-context, line, after-context). There is no per-line, per-file or total-byte cap anywhere in the tool, and no file-size precondition before `fs::read_to_string(entry.path())` (`grep.rs:209`).

**Evidence — `list_directory`'s cap is not applied where entries are added:**
```rust
// autodev-core/src/agent/builtin_tools/list_directory.rs:116-121
    if depth >= max_depth {
        return;
    }
    if out.len() >= max_entries {
        return;
    }
```
```rust
// autodev-core/src/agent/builtin_tools/list_directory.rs:133-168
    for entry in dir_entries {
        ...
        out.push(obj);

        if is_dir {
            collect_entries(root, &entry_path, depth + 1, max_depth, max_entries, out);
        }
    }
```
The only cap test is at *function entry*. The `for` loop that pushes has none. Consequences:
* **Non-recursive (`recursive=false` → `max_depth = 1`): the cap cannot fire at all.** `collect_entries` is called once, the entry test passes on an empty `out`, and every child of the directory is pushed. `max_entries` is inert — a 100,000-entry directory returns 100,000 entries.
* **Recursive:** reaching the cap only stops *descent*; every not-yet-iterated entry in each ancestor directory is still pushed, so the overshoot is the sum of the unvisited siblings along the current path.

And the flag that is supposed to say so tests equality, not "at or over":
```rust
// autodev-core/src/agent/builtin_tools/list_directory.rs:72
        let truncated = entries.len() == max_entries;
```
So the moment the output overshoots — the only case that matters — `truncated` is **false**, the `success_with_hint` branch (`list_directory.rs:79-87`) is skipped, and the model is told the listing is complete. The guard and its warning both fail in the same direction.

Concretely on this repository: `list_directory(path=".", recursive=true, max_entries=50)` walks 395 entries at depth 3 under this tool's own SKIP list (measured: `find . -maxdepth 3 -mindepth 1 <SKIP dirs pruned> | wc -l` → 395). It returns well over 50 of them with `truncated: false`.

**Evidence that this is a per-tool omission, not a house style** — the sibling tool caps inside its collection loop, exactly as `list_directory` does not:
```rust
// autodev-core/src/agent/builtin_tools/glob.rs:186-188
                if file_entries.len() >= params.max_results {
                    break;
                }
```
and `bash` is the only tool with a byte cap:
```rust
// autodev-core/src/agent/builtin_tools/bash.rs:13
const MAX_OUTPUT_BYTES: usize = 50_000;
```
`read_file` bounds both ways (256 KB file ceiling at `read_file.rs:107-108`, 2,000-line page at `read_file.rs:18`). There is **no shared output-bounding helper**: a search for a truncation utility across `autodev-core/src/agent/` returns only these per-tool ad-hoc sites, and `ToolExecutor` applies no cap of its own (`tool_executor.rs` has no length check on `ToolResult::output`).

**Reasoning:** both halves of the claim hold on the current code, and the `list_directory` half is a textbook "guard that cannot fire" — the parameter is documented in the schema (`list_directory.rs:31-34`), echoed in the hint text, and never enforced on the path that produces the entries.

**If the fix were applied as written:** it would work and is small, but the two obvious one-line versions are both wrong in a way worth naming.
* For `list_directory`, adding `if out.len() >= max_entries { return; }` *inside* the loop is correct only if `truncated` also becomes `>=`; fixing the loop alone leaves the flag reporting `false` for every over-cap listing (`len` will now equal `max_entries`, so in fact the `==` test starts working — but only by accident, and it breaks again the moment any caller pushes after the check).
* For `grep`, the per-line bound must be applied on **character** boundaries, not bytes. Two existing sites in this tree already make that mistake on adjacent code — `annotate_file_tool.rs:109` (`if note.len() > 80 { &note[..80] }`) and `remember_this.rs:101` (`&fact[..120]`) — and the identical bug in `subagent/manager.rs` is documented at `manager.rs:363-369` as having silently killed the event pump (audit finding F-03). See C-7.

---

## What I could not determine

1. **Whether the permit-release stall (C-1) has ever bitten a real session.** The window is real but conditional: `manager::list` (which prunes) is polled every 5 s by `SubagentMonitor.tsx:42-44` and every 5 s by `QcRoiPanel.tsx:61` — but only while those components are mounted, and the orphan reconciler's `list` call is behind an early return when no QC claims exist (`orphan/mod.rs:79-81`). *Measurement that would settle it:* add a counter to `SubagentPool::global_available()` sampled on a timer, or log `global_available()` at each `spawn`, and drive 20 sequential `spawn_subagent` calls from a fleet worker with the Subagent Monitor closed. If the 17th blocks for more than a second, the ordering is the cause.
2. **Whether a subagent under a Google or OpenAI active provider actually fails (C-2).** I verified statically that the model string handed to the child engine is `claude-haiku-4-5` regardless of provider, and that `build_provider` selects the adapter from `active_provider`. Confirming the 404 requires a live call, which is out of scope here. *Measurement:* one live-smoke spawn with `active_provider = "google"` and no explicit model, asserting on the provider error — or simply assert in a unit test that `run_inner`'s chosen model routes to the same provider as `provider_id` via `routing::provider_for_model`.
3. **Whether `PoolPermit::drop` can run outside a tokio runtime.** `Drop` calls `tokio::task::spawn` (`pool.rs:62`), which panics with no reactor. Every drop path I traced is inside an async context (`prune_terminal`, `terminate`, and `orphan::reconcile`'s `block_on` which runs on a scheduler thread). Shutdown ordering is the unknown. *Measurement:* a unit test that constructs a pool inside a runtime, moves the permit out, drops the runtime, then drops the permit — it should either panic (confirming) or not.
4. **Whether the 0.80 threshold and the 4-chars-per-token estimator interact badly at the top of a 1M-token Gemini window.** `estimate_tokens` was measured 29% low on the base prompt and 5% high on repetitive text (`cache/prefix.rs:51-65`). At 200k the absolute error is ~50k tokens; at 1M it is ~250k. *Measurement:* run `estimate_tokens` against a captured 1M-token history and compare with the provider's `count_tokens`, which `P2-65` already plans to wire.
5. **Whether `SubagentConfig.tools` was ever meant to restrict the child's registry** (C-8) or was always aspirational. The field has writers and no readers; the docs claim it restricts. Settling which one is wrong is a decision, not a measurement.

---

## Claims I would add

**C-1 · The pool permit is released by pruning, and the prune runs AFTER the blocking acquire — so spawning can stall at 16 with nothing running.** `manager.rs:78` acquires (awaiting the global semaphore, no timeout) and `manager.rs:120` prunes. A permit is only returned when its registry entry is dropped in `prune_terminal` (`manager.rs:304-318`), which is reachable from `spawn` (after the acquire) and `list` — nowhere else. With 16 terminal-but-unpruned entries and no `list` caller (Subagent Monitor closed, QC ROI panel unmounted, no outstanding QC claims), the 17th spawn waits indefinitely. Fix: move `self.prune_terminal().await;` above the `acquire` — one line, and it also makes `TERMINAL_RETENTION` mean what it says.

**C-2 · Every subagent that does not name a model runs `claude-haiku-4-5` on whatever provider is active, and the `active_model` fallback that would have prevented it is unreachable.** `manager.rs:87-89` fills `config.model` with `SubagentConfig::DEFAULT_MODEL` before the config is cloned into the runner at `manager.rs:105`, so `native_runner.rs:163-167`'s `if config.model.is_empty()` branch — the one that reads the user's `active_model` — is dead code. `build_provider` (`native_runner.rs:246-268`) builds the adapter from `active_provider`. On a Google-primary install (`src/data/models.ts` names Gemini the primary provider for rollout 1) every default subagent sends an Anthropic model id to Google. It also poisons the compaction window lookup (`engine.rs:347`, `find(|m| m.id == model)` over the *Google* catalogue rows) — the P3-26 defect, re-entered through the model/provider mismatch rather than through a hardcoded constant.

**C-3 · `ModelCapabilities::context_window` has zero production readers, and the test that guards the compaction fix asserts on it rather than on the path compaction uses.** `catalogue.rs:279`/`:289` populate it; `grep -rn "\.context_window"` over `autodev-core/src` + `src-tauri/src` yields exactly one non-catalogue, non-assert reader — `engine.rs:348`, which reads `ModelInfo`. The test `geminis_full_window_is_reachable` (`catalogue.rs:374-384`) therefore exercises a field the product never consults. Both fields come from the same `CATALOGUE` row so the test is not lying today, but it cannot fail for the failure it names (a break in `provider.models()` lookup, a provider/model mismatch, a wrapper that stops delegating). Fix both at once: read `caps.context_window` at `engine.rs:343-353` — `caps` is already in scope from `engine.rs:171` and is model-keyed, not provider-scoped.

**C-4 · The two nesting guards disagree, and the pool's depth branch is unreachable from the builtin tools.** `spawn_subagent.rs:86` refuses when `ctx.depth >= 3` (parent depth), so the deepest child config it can build is `depth: 3`; `pool.rs:96` refuses when `depth > 3` (child depth). The tool guard is strictly tighter, so `MAX_NESTING_DEPTH` can only ever fire for a caller that bypasses the tools — today just the Tauri command `spawn_subagent`, which takes `depth` from the frontend request (`subagent_commands.rs:39`, `:55`). Two constants, two comparison operators, one of them dead; they should be one constant with one operator.

**C-5 · The QC path picks an Anthropic model id regardless of the active provider — C-2's defect on a second path.** `spawn_qc_review.rs:155-163` falls back to `crate::db::workers::DEFAULT_MODEL`, which is `"claude-sonnet-5"` (`db/workers.rs:103`), while the adapter is still built from `active_provider` (`native_runner.rs:246-268`). On a Google-primary install every QC review that does not name a model explicitly sends `claude-sonnet-5` to Google. The comment above that line records the *previous* iteration of the same bug ("Was a hardcoded `claude-sonnet-4-5` — retired in February 2026, so every QC review that did not name a model explicitly ran against an id no provider would serve") — the id was refreshed, the provider-independence was not.
*(Checked and NOT a defect: this tool does carry its own depth guard, `spawn_qc_review.rs:80`, `if ctx.depth >= subagent::pool::MAX_NESTING_DEPTH`. It uses `>=` on the parent's depth like the other two, so C-4 applies to it as well.)*

**C-6 · `CachePolicy::Explicit::breakpoints` is written and never read.** `engine.rs:199` sets `breakpoints: vec![0]` with a comment explaining the choice; the Anthropic adapter destructures `CachePolicy::Explicit { ref ttl, .. }` (`anthropic.rs:170`) and places the breakpoint on the system block unconditionally. `grep -rn "breakpoints"` finds one production write, two test writes, no reader. Harmless today because the intended value and the hardcoded behaviour agree — but it is a field that will silently ignore its first real use.

**C-7 · Two tool paths truncate a `&str` by BYTE index and will panic on non-ASCII input.** `annotate_file_tool.rs:109` (`if note.len() > 80 { &note[..80] }`) and `remember_this.rs:101` (`&fact[..120]`). Both slice model- or user-authored free text: a note over 80 bytes whose 80th byte lands mid-character panics inside `Tool::execute`. This is the same defect class as audit finding F-03, whose fix and post-mortem sit at `subagent/manager.rs:363-377` ("a panic kills the pump silently and every later subagent event is lost"). The fix there — `s.chars().take(max)` — is the fix here.

**C-8 · `SubagentConfig.tools` is documented as the child's allowed tool subset, has writers everywhere, and has no reader at all.** Declared at `types.rs:41-43` ("Tool subset the subagent is allowed to use … Empty vec = no `internal_tools`"), advertised to the model in the tool schema (`spawn_subagent.rs:45-49`), populated by all three spawn tools and by the Tauri command (`subagent_commands.rs:49`) — and `native_runner::run_inner` never reads it. The child engine is built with the full registry (`native_runner.rs:195-206`). A parent that asks for a read-only child gets a child that can write files and run `bash`. This is a containment claim the code does not honour, which makes it more than cosmetic.

**C-9 · `SubagentConfig.max_turns` is defaulted, validated and discarded.** `manager.rs:81-83` fills the default, all three spawn tools set it, and `run_inner` never passes it to the engine — the child is bounded by `TaskBudget::default_worker()` (`native_runner.rs:206`, 20 turns / 100k tokens / $0.50) instead. So `max_turns: 30` in `spawn_qc_review.rs:180` is silently 20, and a caller lowering it to 5 gets 20. Either thread it into the budget (`TaskBudget { max_turns: Some(config.max_turns), ..default_worker() }`) or delete the field; leaving it is a knob wired to nothing.

**C-10 · `SubagentResult` reports `cost_usd: 0.0`, `tokens_in: 0`, `tokens_out: 0` for every run — and the QC hourly cost ceiling is built on that number, so it cannot trip on realised spend.** The runner is honest about it in comments (`native_runner.rs:108-114`), but three consumers treat the zeros as measurements:
* `spawn_parallel_subagents.rs` sums them into `total_cost_usd` / `total_tokens_in` / `total_tokens_out` and returns them to the model as fact.
* `spawn_qc_review.rs:230-240` — `add_qc_cost` is gated on `cost > 0.0` so the task board's QC cost column is never written, and `qc_cost_guard::record_dispatch_complete(cost)` is then called *unconditionally* with that 0.0.
* That guard is an hourly ceiling: `check_and_record_dispatch_intent` (`spawn_qc_review.rs:400-415`) rejects a dispatch when `sum + PROVISIONAL_COST_USD > CEILING_USD_PER_HOUR`, and `record_dispatch_complete` (`:424-437`) removes the provisional entry and pushes the realised one. Fed a constant 0.0, every completed reviewer erases its own provisional cost, so the rolling sum only ever reflects **in-flight** reviewers — a bound `MAX_CONCURRENT_QC = 6` already enforces. The $/hour ceiling is, in practice, a second concurrency limit.

The daily cap is unaffected (it is enforced inside `CappedProvider` against real spend), but every per-subagent number surfaced to the model, to the task board, to analytics and to the QC ROI panel is a zero presented as a measurement.
