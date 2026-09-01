# P2 pre-build refutation — prompt caching

**Read-only adversarial pass, 2026-09-02.** Dispatched under `§0.2m`. Same dispatch defect as
`P2-REFUTE-cost.md`: the agent was given `§0.2i`'s incremental-write clause and a read-only
agent type, so it could not write its own file. Recorded, not hidden.

---

## Summary

| Claim | Verdict |
|---|---|
| 1 — no caching exists today | **CONFIRMED** |
| 2 — the system prompt is not byte-stable | 🔴 **REFUTED as stated** — all four named causes are absent |
| 3 — cache usage fields are not parsed | **CONFIRMED**, all providers, both paths |
| 4 — no single prefix builder | **REFUTED for the live path**, overstated overall |
| 5 — Anthropic needs a 100% byte match and has silent per-model minimums | **CONFIRMED as a vendor fact**; the code holds no assumption — but the consequence is unflagged |

---

## Claim 1 — no caching exists (CONFIRMED)

`cache_control|cache_read|cached_read|cache_creation|ephemeral|prompt_cach|cachedContent|cached_tokens|anthropic-beta|prompt-caching`, case-insensitive, all extensions: **zero hits in any `.rs` file.** Every hit is prose in `_grounding/G6-agents-sdk-research.md`.

The Anthropic body (`anthropic.rs:132`) carries `model`, `max_tokens`, `messages`, optional
`tools` and `system`. Headers (`anthropic.rs:178`) are `x-api-key`, `anthropic-version`,
`content-type` — no beta header. `CompletionRequest` (`types.rs:59-69`) has no cache field and
no `CachePolicy` type exists.

⚠ **`body["system"] = json!(sys)` emits a bare string, not an array of blocks** — there is no
block structure onto which a breakpoint could be hung without a shape change.

---

## Claim 2 — byte-stability 🔴 **REFUTED AS STATED**

**This is the correction that matters most, because P2-13 and P2-14 were written against it.**

### The prompt is built ONCE per `run()` and cloned

`engine.rs:109-114` builds `system_prompt` and `tool_defs` **above** the loop at `engine.rs:143`;
`engine.rs:205-208` re-uses them by `.clone()`. Across every tool-loop iteration of a single
`run()` — up to `MAX_TOOL_TURNS = 20` (`engine.rs:31`), which is exactly where caching pays —
the system string and tool list are **byte-identical by construction, not by luck**.

### All four named causes are absent

| Alleged cause | Finding |
|---|---|
| timestamps | Zero `Utc::now()`/`Local::now()` in `context.rs`, `recall.rs`, `defaults.rs`. The only date reaching a prompt is a stored row's, day-granularity (`recall.rs:46`) — it does not tick |
| changing paths | `working_dir`/`repo_path` are **lookup keys only** (`context.rs:63,113,117`), never interpolated. `parts.push` in `context.rs` yields only three literal headers and the memory block |
| counts | The only count-bearing text is in **messages**, not system (`compaction.rs:64`) |
| HashMap ordering | `parts` is a `Vec` joined in source order (`context.rs:76,137`). The registry **is** a HashMap (`tool_registry.rs:10`) but `to_definitions()` **sorts** (`tool_registry.rs:43`). Tool descriptions cannot vary — the trait returns `&str` (`tool_trait.rs:112`) and no impl uses `format!`. `serde_json` is declared without features (`Cargo.toml:9`), so `Map` is a `BTreeMap` and schema key order is alphabetical |

### What IS unstable — narrower and real

Between **separate `run()` calls**, the prompt is rebuilt and the memory block re-queried
(`context.rs:131-135`). The three backing queries are deterministic *given DB state*
(`episodic.rs:171`, `semantic.rs:161`, `procedural.rs:184`) but state mutates mid-session via
`remember_this` (`remember_this.rs:87`) and `Summarizer::summarize_session`
(`summarizer.rs:38`). Any write reorders or extends the block, invalidating the system level at
the next user message.

⚠ `ProceduralStore::search` is called with `""` (`recall.rs:35`), so the LIKE pattern is `%%` —
it matches everything and returns the N most-recently-updated rows, the most churn-prone
ordering available.

**Settled by:** hashing `build_system_prompt_with_mode`'s output at the top of every `run()`
and comparing consecutive hashes.

---

## Claim 3 — no instrument (CONFIRMED, all providers, both paths)

Anthropic non-streaming `anthropic.rs:287`; Anthropic streaming `streaming.rs:71-78`; OpenAI
`openai.rs:392,415`; Google `google.rs:356,381`. None reads `cache_read_input_tokens`,
`cache_creation_input_tokens`, `prompt_tokens_details.cached_tokens` or
`cachedContentTokenCount`. The carrier types (`types.rs:71-79`, `:101-105`) have no slot.

Downstream blindness is total: `engine.rs:474` accumulates two numbers;
`CostTracker::calculate` has no cache-rate parameter. **Even if caching were switched on
tomorrow, cost would be over-reported and the hit rate unobservable.**

---

## Claim 4 — prefix builder (REFUTED for the live path)

**tools** → `to_definitions()` (`tool_registry.rs:33`), sorted, fixed literal registration per
mode (`builtin_tools/mod.rs:188-268`). **system** → `build_system_prompt_with_mode`
(`context.rs:83`) with **exactly one call site**, `engine.rs:113`.

Only four `CompletionRequest` sites exist repo-wide; the other three are one-shot calls with
`tools: vec![]` and a tiny or absent system prompt (`classifier.rs:139`, `compaction.rs:111`,
`recipe_commands.rs:257`) — not on the cacheable prefix at all.

**Grain of truth:** three system-prompt origins reach `engine.run()` — the builder
(`engine.rs:113`), recipes (`runner.rs:743`, bypassing it), and subagents
(`native_runner.rs:206`, composed by the caller).

---

## Claim 5 — minimums (CONFIRMED as vendor fact; consequence unflagged)

`G6-agents-sdk-research.md:341` states minimums are **silent**: *"shorter prefixes silently do
not cache and return no error"* — Opus 4.6/4.5 → 4,096; Sonnet 5/4.6/4.5 → 1,024; Haiku 4.5 →
4,096. `:352` adds that the match runs over `tools → system → messages`, that a change at any
level invalidates that level **and every level after it**, and that there is a 20-block
lookback window.

**The code holds no assumption to check** — no minimum table, no `capabilities()`, no
threshold, no warning. `models_static()` (`anthropic.rs:34-64`) has no cache field.

🔴 **The unflagged consequence.** Measured prompt sizes (`wc -c` over the consts in
`context.rs`): `BASE_SYSTEM_PROMPT` 7,397 B (~1,850 tok) · `REVIEW_SYSTEM_PROMPT` 1,464 B
(~366) · `DELEGATE_SYSTEM_PROMPT` 1,844 B (~461). **`PAMode::Delegate` registers only four
tools** (`builtin_tools/mod.rs:260-266`). That combined prefix lands in the low four figures at
best — **below the 4,096 minimum on two of three shipped models, failing silently.** `P2-15`
treats this as a runtime edge case; it is a design-time certainty for Delegate mode.

---

## What the plan gets wrong

- **W-1** — `MASTER_DEVELOPMENT_PLAN.md:139` says *"the system prompt is not byte-stable."* Flatly false within a `run()`. The real defect is memory-block drift **across** `run()` calls. Ironically, `§0.2L-2`'s lesson (*an inference is not a measurement*) is undercut by the fact that this instability was itself inferred.
- **W-2** — The four enumerated causes are all absent. `NOVELTY_LEDGER.md:31` promises *"one byte-stable prefix builder with ordered maps"*; the tools half **already has** ordered maps. Budget P2 effort at the memory block, not at map ordering.
- **W-3** — "No single prefix builder" is wrong for the live path: one live builder, one dead divergent twin, two override channels.
- **W-4** — 🔴 `TESTING_GATES.md:158` (`TG2-06`) targets *"non-deterministic key order"*, which is **already absent**. The gate **will pass on day one and prove nothing** — the programme's own "inert-but-green" failure. Re-scope to the full `tools + system + messages` prefix.
- **W-6** — `NOVELTY_LEDGER.md:30` cites the 512–4,096 range without noting that two of three shipped models sit at the ceiling and Delegate mode cannot reach it.

---

## Unlisted findings

**U-1 · Dead divergent builder.** `context.rs:37` `build_system_prompt` has **zero callers**. It reads AUTODEV.md from the filesystem only, never queries `AutodevInstructionsDb`, never injects memory — it would emit a *different* prefix from the live builder. A trap for whoever implements P2's prefix builder. Delete it or make it the single implementation.

**U-2 · 🔴 HashMap iteration DOES break the prefix — at the MESSAGES level.** This is the real instance of the mechanism Claim 2 misattributes to the system prompt.

`engine.rs:431` declares `tool_use_map: HashMap<String, (String, String)>`; `engine.rs:486-492` iterates it to build content blocks. `std::collections::HashMap` iterates in an order derived from a per-process `RandomState` seed. **When the model emits two or more parallel tool_use blocks in one message, the order written into `history` (`engine.rs:283-286`) is arbitrary and differs between processes** — and that reordered assistant message is replayed on every subsequent turn, permanently invalidating the messages level from that point on.

Independently, `text_buffer` is pushed **unconditionally first**, so text the model emitted *after* a tool_use is reordered ahead of it.

**Both are correctness bugs on the wire regardless of caching.** The fix is available: `block_index` is already carried on `StreamChunk::ToolUseStart` (`types.rs:92-96`) — preserve it and use an ordered container.

**U-3 · Subagent prompts stamp per-task identifiers, making every prefix single-use.** `spawn_subagent.rs:112` interpolates `working_dir` into a system prompt. Worse, `spawn_qc_review.rs:152` builds `qc_reviewer_system_prompt(task_id, repo_path)`, and `defaults.rs:1583-1587` stamps `#{task_id}` **four times** plus `Repository under review: {p}` at `:1576`. Every QC reviewer gets a unique, single-use prefix; with a 5-minute TTL and one `run()` each, they can never be reused and every cache write is pure 1.25× overhead. **Enabling caching naively across subagent spawns would *increase* cost on the QC fan-out path (concurrency 6).** Structural fix: hoist the invariant protocol body into the cached block and push `#{task_id}`/`repo_path` into the first *user* message, past the breakpoint.

**U-4 · Compaction destroys the cached prefix with no accounting.** `compaction.rs:154-163` rebuilds history as `messages[0] + summary + tail`. Every compaction forces a full messages-level cache write, plus an extra full-context `provider.complete()` (`compaction.rs:124`) that itself runs `system: None, tools: vec![]` — an uncacheable shape. `needs_compaction` fires at a hardwired `0.80` against a hardwired `let model_context = 200_000u32;` (`engine.rs:188`), **ignoring `ModelInfo::context_window` which is already populated** (`anthropic.rs:39/48/57`). P3-28 addresses `clear_tool_uses`; nothing addresses compaction's own forced rewrite.

**U-5 · Tool results are assembled in a different order from the tool calls.** `engine.rs:320-340` emits unknown-tool errors first, then batch results, whereas calls were collected in order at `engine.rs:277-280`. With U-2, neither half of a tool round-trip preserves wire order.

**U-6 · `cached_tokens` already arrives and is dropped.** `sidecar/client.rs:39` and `:944` document that the orchestrator returns `{prompt_tokens, completion_tokens, total_tokens, cached_tokens}`; `extract_usage` (`client.rs:47-80`) reads three of four. Since core-orchestrator fronts LiteLLM, this is the one place a cache signal is *already arriving* — reading it is one line and gives `P2-12` a free early data point. *(Note: this path is deleted in P8, so the value is short-lived.)*
