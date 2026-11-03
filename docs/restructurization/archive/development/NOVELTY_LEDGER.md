# AutoDev Restructure — Novelty Ledger

> **Governance for every externally-inspired mechanic.** No paper, vendor document or web finding becomes product truth on its own. At final closeout **every row must be `ADOPTED-PROVEN` or `REJECTED`** — nothing ships as intent.

| | |
|---|---|
| **Parents** | `MASTER_DEVELOPMENT_PLAN.md` · `TESTING_GATES.md` · `../vision/_grounding/G4`, `G4b`, `G4c`, `G6` · `../memory-anchor.md` |
| **Rev** | 1.0 — 2026-08-31 |
| **Statuses** | `TERRAIN` (surveyed, undecided) · `ADOPTED-INTENT` → `ADOPTED-PROVEN` (a named gate passed) · `REJECTED` · `REJECTED-AND-REPLACED` |

**The rejection column is often the valuable half.** It records what we deliberately refuse and why — which is what stops a later session from "helpfully" adding it back.

---

## Provider and runtime mechanics
> ### Provider scope changed 2026-09-01
>
> The provider set is **Anthropic, OpenAI, Google** — three, not four.
> xAI/Grok was removed on Razi's ruling and `P2-07` is struck.
>
> Rows below that describe the **external market** still name xAI, because
> those statements remain true about the world. Rows stating **our adoption
> scope** have been corrected to three. Where a rejection existed only
> because xAI existed, it is marked moot rather than silently deleted — a
> decision that no longer applies is not the same as one never made.


### N-01 · Prompt caching
- **Source pattern** — vendor prompt/context caching across all four providers (`G6 §1.C.1`, `§2.C.1`, `§3.C`, `§4.C`)
- **Known external shape** — Anthropic: explicit `cache_control`, ≤4 breakpoints, per-model minimums 512–4,096 tokens, 5m@1.25× / 1h@2.0× write, **0.1× read**. xAI: fully automatic, no `cache_control`, steered by `x-grok-conv-id`, 75–85% off. OpenAI/Gemini differ again.
- **We adopt** — all three in scope, behind a three-variant `CachePolicy`; one byte-stable prefix builder with ordered maps.
- **We reject** — treating caching as a boolean. Collapsing the mechanisms to one flag loses the feature on at least two providers.
- **Unique addition** — **the cache-hit alarm is built before the cache.** No provider errors below its minimum; the only evidence is a usage field. A cache that silently does nothing has no symptom but the bill.
- **Proof** — `TG-P2` `G-CACHE-HIT`, `G-CACHE-STABLE`, `G-CACHE-MIN`
- **Status** — `ADOPTED-INTENT`

### N-02 · Server-side context editing (`clear_tool_uses`)
- **Source** — Anthropic context editing (`G6 §1.C.5`) · **External shape** — trigger/keep/`clear_at_least`/`exclude_tools`, clears rather than summarises
- **We adopt** — as the layer between live history and compaction.
- **We reject** — clear-early-clear-often, ~~because clearing invalidates the cached prefix and forces a cache write~~. ⚠ **That reason was WRONG, and the measurement is what established it (2026-09-02).** Our cache breakpoint is on the **system block only** (`anthropic.rs`); clearing edits the **message region**, where no breakpoint is set — so clearing invalidates nothing and forfeits no cache write. The rejection stands on a different footing: clearing costs **tool-result content the model may still need**, which is why `keep = 3` retains a read → edit → verify cycle and `clear_at_least = 5%` refuses a clear that frees almost nothing.
- **Unique addition** — ~~the threshold is derived from measured cache-write cost~~ → **the threshold is derived from the LAYER ORDERING, and the cache premise was tested rather than assumed.** `CLEAR_TRIGGER_FRACTION = 0.60` sits below `COMPACTION_THRESHOLD = 0.80`, and that inequality is asserted: the cheap server-side layer must get its chance before the expensive summarisation call. ⚠ Recorded at the site: **if a message-region breakpoint is ever added, these must be re-derived** — the original trade-off becomes real at that moment.
- **Proof** — `p3_25_bench_the_clear_layer_actually_reduces_input_tokens` (live) + `p3_25_28_tests` + `p3_25_wire_tests` · **Status** — **`ADOPTED-PROVEN`** — measured against `api.anthropic.com`, six tool round-trips: **11,310 input tokens without the layer, 2,750 with it — 8,560 freed, 75.7%**, `applied_edits` reporting `cleared_input_tokens: 8515, cleared_tool_uses: 5`. The wire shape was likewise established by execution: the identical body **without** `anthropic-beta: context-management-2025-06-27` returns **400 `context_management: Extra inputs are not permitted`**.

### N-03 · Server-side compaction
- **Source** — Anthropic, OpenAI, xAI (**Google has none**) · **External shape** — a summarisation block appended to history; billing shape changes to `usage.iterations[]`
- **We adopt** — as a long-session backstop only, with a client-side fallback for Google.
- **We reject** — compaction as the primary context strategy. It costs an extra full-context call and is lossy; caching and clearing come first.
- **Unique addition** — the billing trap is handled explicitly: top-level `input_tokens` reflects only non-compaction iterations, so naive cost tracking under-reports.
- **Proof** — `TG-P3` `G-COMPACT-FIRES` · **Status** — `ADOPTED-INTENT`

### N-04 · The vendor agent-SDK loop shape
- **Source** — Anthropic Claude Agent SDK, OpenAI Agents SDK, Google ADK (`G6` Parts 1–3)
- **External shape** — Runner, handoffs, sessions, guardrails, structured output, streaming event taxonomy
- **We adopt** — the **semantics**: the turn loop, tool wire shape, parallel tool calls, streaming taxonomy, stop reasons.
- **We reject** — the SDKs themselves. **No first-party Rust SDK exists for any of the three**, and hosting a Node/Python runtime to get one is a sidecar by another name — the exact defect this programme removes. **xAI ships no agent SDK in any language at all.**
- **Unique addition** — `Item::Opaque` from day one, carrying every in-scope vendor's unreplayable reasoning state without interpreting it.
- **Proof** — `TG-P2` `G-CONFORM`, `G-OPAQUE` · **Status** — `ADOPTED-INTENT`

### N-05 · LiteLLM's normalised provider abstraction
- **Source** — `core-orchestrator`'s `LiteLLMProvider` (`G4b §A.1`) · **External shape** — 11 providers behind one interface, normalised to OpenAI-shaped tool calls, per-request key injection with no env mutation
- **We adopt** — the normalisation target, the ≤25-iteration tool loop, per-request keys, and the Anthropic thinking-block/signature special case.
- **We reject** — the 11-provider breadth (three are mandated) and the model registry's contents, which are stale.
- **Unique addition** — `capabilities()` keyed by **model**, not provider, because the vendors vary per model.
- **Proof** — `TG-P2` `G-CONFORM` · **Status** — `ADOPTED-INTENT`

### N-06 · Provider-hosted tools
- **Source** — hosted web search, file search, code execution across all four (`G6 §5.3`)
- **We adopt** — **web search only**, via Google (OQ-4).
- **We reject** — **hosted file search**, because it *is* hosted RAG, which ADR-01-05 rows 12–14 skip, and it ships user code to a vendor against the local-first non-goal. **Hosted code execution**, because executing code on the user's machine is the product. **Managed agent runtimes** (Anthropic Managed Agents, OpenAI hosted state), because they re-host our runtime in a vendor cloud, per-vendor.
- **Unique addition** — egress consent is a property of **workspace + destination**, recorded and revocable, not a per-call prompt.
- **Proof** — `TG-P3` `G-PERM-NETWORK` · **Status** — `ADOPTED-INTENT` (web search) / `REJECTED` (the rest)

### N-07 · Batch APIs
- **Source** — 50% discount at Anthropic, OpenAI, Google; 20% at xAI
- **We adopt** — for offline work only (cleanup workers, bulk analysis).
- **We reject** — batch for anything interactive. *(The line here also rejected xAI's 20% tier; xAI left the provider set 2026-09-01, so that rejection is now moot rather than a decision.)*
- **Proof** — deferred; no gate until a batch consumer exists · **Status** — `TERRAIN`

---

## Memory mechanics

### N-08 · The three-tier memory model
- **Source** — `core-orchestrator` LivingAgent (`G4c`), reinforced by `memory-anchor.md` §4
- **External shape** — Episodic / Semantic / Procedural, one physical store, tier carried in metadata
- **We adopt** — the tier separation, the write/read paths, `user_correction` as the highest-authority record class, age-based compression of episodic only.
- **We reject** — **seven inherited defects, explicitly**: query-independent semantic recall · a cache key that ignores the query · an always-throwing HITL writer · unfiltered procedural recall · unscoped episodic recall · an inert negative cache · an uninvalidated semantic cache. A faithful port would reproduce all seven.
- **Unique addition** — the fix-list is a task group (`P3-43`), not a footnote.
- **Proof** — `TG-P3` `G-MEM-QUERY`, `G-MEM-SCOPE`, `G-MEM-SUBTYPES` · **Status** — `ADOPTED-INTENT`

### N-09 · Competitive two-stage memory budgeting
- **Source** — `G4c §4.4` · **External shape** — all tiers unified into one pool; absolute utility (9 weighted factors, 7 archetype presets) then marginal utility with redundancy/gap/contradiction terms; greedy selection with an early stop
- **We adopt** — eventually. Per-tier quotas ship first (ADR-01-08), interface-compatible.
- **We reject** — shipping it first. Its value is unproven *in this product*, because the recall path it ranks is itself defective.
- **Unique addition** — **⚠ the plan initially claimed this and the anchor's §10 reranker were the same model. They are not — 6 of 8 terms are disjoint.** The false equivalence had left the reranker with no work item. Corrected as G-13; they are now two rows, N-09 and N-10.
- **Proof** — `TG-P3` (Phase 2 of the memory programme) · **Status** — `TERRAIN`

### N-10 · The memory-anchor reranker
- **Source** — `memory-anchor.md` §10 · **External shape** — `semantic + lexical + importance + recency + access_frequency + scope_match + task_similarity + confidence`, weights determined experimentally
- **We adopt** — as the Phase-2 target, with **its own schema columns** (`access_count`, `last_accessed_at`, `importance`, `confidence`).
- **We reject** — treating the weights as settled. The anchor says "determined experimentally"; they are tuned against the eval corpus, not copied.
- **Unique addition** — the anchor's own evaluation suite (§23) becomes gates: Recall@5/@10 and MRR over a labelled corpus.
- **Proof** — `TG-P3` `G-MEM-RECALL-QUALITY` · **Status** — `ADOPTED-INTENT`

### N-11 · RRF fusion of BM25 and vector search
- **Source** — `G4c §4.4` · **External shape** — `RRF(d) = 0.3/(60 + rank_bm25) + 0.7/(60 + rank_semantic)`, k=60; BM25 with k1=1.5, b=0.75
- **We adopt** — verbatim as the starting point. It is standard, correct, and better than a linear blend.
- **We reject** — nothing. This is the cleanest thing in the ported design.
- **Unique addition** — FTS5 provides BM25 natively, so the lexical leg costs no new dependency and survives when embeddings are unavailable.
- **Proof** — `TG-P3` `G-MEM-DEGRADE` · **Status** — `ADOPTED-INTENT`

### N-12 · mem0 + Qdrant
- **Source** — `memory-mcp-server` · **External shape** — mem0 OSS over Qdrant, collection `mem0`, 1536-dim `text-embedding-3-small`, LLM fact extraction
- **We reject entirely and replace.** A separate vector service for a single-user desktop app is unjustified, and the two submodules **disagree with each other** about whether it is even in the live path — `core-orchestrator` wires no vector store and retrieves via BM25.
- **Replaced by** — SQLite + `sqlite-vec` + FTS5, statically linked (ADR-05-01).
- **Unique addition** — verified there is **no existing index to migrate**, so the replacement carries no data-loss risk.
- **Status** — `REJECTED-AND-REPLACED`

### N-13 · PostgreSQL + pgvector
- **Source** — `memory-anchor.md` §21, the anchor's own recommended first implementation
- **We reject the storage recommendation while adopting the principles.** Bundling PostgreSQL means a server process, a port, a lifecycle, a watchdog and ~200 MB — recreating the exact defect this programme removes (INV-1). The anchor anticipates this: *"The architecture should remain modular enough that the retrieval backend can later be replaced."*
- **Replaced by** — SQLite + `sqlite-vec` + FTS5 **behind a retrieval-backend trait**, so a hosted edition could still adopt it.
- **Proof** — `TG-P3` `G-NO-SERVER` · **Status** — `REJECTED-AND-REPLACED` (acknowledged divergence, ADR-05-01)

### N-14 · Local ONNX embeddings
- **Source** — `fastembed-rs`, BGE-small-en-v1.5 · **External shape** — local inference, synchronous, no Tokio, model downloaded once then offline; 384 dimensions
- **We adopt** — after the owner **retracted** an initial choice of OpenAI `text-embedding-3-small` the same day. Local removes the BYOK coupling: Anthropic ships no embeddings endpoint, so provider embeddings would force an OpenAI key on a Claude-only user.
- **We reject** — provider embeddings, and the default `ort` feature set, which **downloads the ONNX runtime from a third-party CDN at build time** — breaking offline and reproducible builds.
- **Unique addition** — the runtime is pinned and vendored; the model cache path is set **explicitly under the app data root** rather than `fastembed`'s CWD-relative default, which would violate INV-5 on the newest subsystem.
- **Proof** — `TG-P3` `G-MEM-DOWNLOAD`, `G-NO-CWD-WRITE` · **Status** — `ADOPTED-INTENT`

### N-15 · The Memory Gateway and its permission boundary
- **Source** — `memory-anchor.md` §16–§18
- **External shape** — all access behind a gateway; read-only agents get `search`/`get`/`list`, a trusted component gets `store`/`update`/`delete`; §18's selectivity criteria before creating a memory
- **We adopt** — agents read and **propose**; a deterministic gateway evaluates importance, novelty, duplication and confidence.
- **We reject** — `remember_this`'s unrestricted agent write, which has no evaluation gate at all.
- **Unique addition** — **sensitivity joins the gateway's criteria**, and M-40's redaction runs on the write path, so a secret read from a file cannot enter the store. The anchor does not cover this; the review found it.
- **Proof** — `TG-P3` `G-MEM-GATEWAY`, `G-REDACT` · **Status** — `ADOPTED-INTENT`

---

## Infrastructure mechanics

### N-16 · The three-mode thinking detector (STREAM V2)
- **Source** — `core-orchestrator/src/airs/streaming/` (`G4b §A.6`) · **External shape** — native reasoning · native thinking blocks · XML DFA fallback with a 4096-byte KMP ring buffer, TTFS < 250 ms
- **We adopt** — the three-mode contract verbatim. Dependency-free, and Rust runs it strictly better.
- **We reject** — the XML fallback branch's prompt-injection half (`ThoughtProcessor.inject_instructions`), needed only for models without native reasoning. All three mandated providers have it.
- **Proof** — `TG-P2` `G-CONFORM` (streaming taxonomy) · **Status** — `ADOPTED-INTENT`

### N-17 · Twenty finite state machines
- **Source** — `core-orchestrator/src/airs/state/` (`G4b §A.5`)
- **We adopt** — **five of twenty**: tool execution, session lifecycle, agent lifecycle, task execution, startup/shutdown.
- **We reject** — the other fifteen. `A2AServerFSM`, `MCPServerFSM`, `CacheTierFSM`, `AuthenticationFSM` and the rest model server-side concerns that die with the sidecar.
- **Unique addition** — Rust enums make illegal states unrepresentable, so the *pattern* is absorbed by the type system rather than reimplemented. **With the caveat the review caught: an enum only helps inside Rust — `workers.status` is unconstrained `TEXT` written by a second process, so `FromStr` must fail loudly and a `CHECK` constraint guards the column.**
- **Proof** — `TG-P4` `G-DISPATCH-EXHAUSTIVE` · **Status** — `ADOPTED-INTENT`

### N-18 · A2A (Agent-to-Agent) protocol
- **Source** — Google A2A via `a2a-sdk` (`G4b §A.8`) · **External shape** — one HTTP server per agent across ports 5001–5099
- **We reject entirely.** Ninety-nine localhost ports to let two objects in the same process talk. In Rust that is an `mpsc::channel`.
- **We keep only the semantics** — `MAX_CALL_DEPTH = 3`, cycle detection over the call graph, and the `DELEGATE[@agent-id]:` grammar.
- **Replaced by** — in-process ACP messaging over the `worker_messages` table.
- **Proof** — `TG-P4` `G-ACP-IDENTITY` · **Status** — `REJECTED-AND-REPLACED`

### N-19 · MCP (Model Context Protocol)
- **Source** — the MCP ecosystem · **External shape** — a client connects to servers and surfaces their tools to an agent
- **We reject the server** (it existed solely to serve the sidecar and is an unauthenticated local RCE). **We defer the client**, which does not exist at all — `rmcp` is compiled server-features-only and cannot dial out.
- **The blocker, recorded so it is not rediscovered** — `ToolRegistry` is frozen after construction with no dynamic registration path anywhere, so admitting third-party tools means changing the agent core, and there is no permission story for untrusted tools.
- **Unique addition** — the false UI claim that AutoDev "connects to the GitHub MCP server" is deleted, because a control that lies is worse than an absent one.
- **Proof** — `TG-P0` `G-NO-MCP-SERVER` · **Status** — `REJECTED` (server) / `TERRAIN` (client, ADR-05-06)

### N-20 · The Claude Code / Goose feature inventory
- **Source** — the original product vision (`docs/autodev-vision/autodev-master-plan.md` §3)
- **We adopt** — AUTODEV.md instructions, skills, recipes, hooks, subagents, headless mode, permission modes, worktree isolation, cost audit, model-agnostic providers.
- **We reject** — the dual-mode execution model (Rust PA + Python worker), which this entire restructure exists to collapse; GitHub Issues as the command layer; and the TypeScript agent engine the vision's §7 specified.
- **Proof** — the headline acceptance test · **Status** — `ADOPTED-INTENT`

---

## Design mechanics

### N-21 · Writing to another product's capability-contract conventions

| Field | Value |
|---|---|
| **source_pattern** | AutoDesign's `@airs/auto-design-capability-contracts@0.2.0` — a sibling product's shipped, versioned contract package |
| **known_external_shape** | Per-family namespaced version literals (`ad-<name>/0.2.0`) pinned so an unknown version fails decode; `.strict()` objects that refuse undeclared fields; a `fixtures/v<semver>/{valid,invalid,races}` corpus; artifact transfer by reference with `sha256` + `storageRef` + `createdAt`/`expiresAt`; a nine-category error taxonomy with a `retryable` flag; and a three-valued outcome `inspection_ready \| partial \| held` in which `partial` must be evidenced |
| **airs_adoption** | The **conventions**, not the types. Namespaced per-family versions; fail-closed decode with unknown-field refusal; reference-not-payload; the artifact field set verbatim so their existing decoder reads ours; their `category` and `retryable` verbatim; and their outcome triple, renaming only `inspection_ready` → `complete` for a reader outside the PDF context |
| **airs_rejection** | **Their types, necessarily.** `capabilityIdSchema` is a closed 18-member `z.enum` of AutoDesign's own capabilities, the host manifest declares `productOwner: z.literal('airs_auto_design')`, and the catalog must contain every canonical capability exactly once — so AutoDev **cannot** publish a conforming manifest, and "write our contract to their shape" was never executable as stated. Also rejected: their 16 KiB input envelope as a *payload* limit (we carry a reference, so it never binds), their internal nine-status job vocabulary (we mirror the public one), and any event type we do not actually emit — three of theirs have no producer |
| **airs_unique_addition** | **`FidelityReport.method`.** A fidelity score must state how it was obtained, and `unmeasured` may not carry a number. Nothing in AutoDesign's contract has this, and it exists because the organisation's own adaptation engine reported a fidelity score for a year that nothing measured — a penalty tally over the boxes the engine had just produced, with no rasterisation and no comparison anywhere in the pipeline. A contract that carries a `score` and no account of its provenance invites exactly that |
| **proof_or_eval** | `TGD0-01`…`TGD0-08`, all in `npm test`; four falsified by planting a compiling defect and confirming the failure names its own reason. `SD0-01` spawns the sidecar as a real child process — nine cases, five of them refusals |
| **status** | **ADOPTED-PROVEN** |

⚠ **The design programme's other three rows are owed.** `PD1` is grounded in an
external converter (`apps/psdconversions`), and `PD2`/`PD4` carry ~33 tasks
derived from the AAE post-mortem — all externally-inspired, none yet in this
register. Recorded here so they are not lost: `PD1`, `PD2` and `PD4` each open
their own row at their own closeout.

---

## Ledger discipline

- **Closeout rule** — at final closeout every row is `ADOPTED-PROVEN` or `REJECTED`. A row still at `ADOPTED-INTENT` blocks the capstone.
- **Promotion** — `ADOPTED-INTENT` → `ADOPTED-PROVEN` requires the **named gate to have passed**, with its evidence class recorded here.
- **New rows** — any mechanic adopted from an external source mid-build gets a row **before** it is implemented, not after.
- **Reversals** — a status change is a new dated line in the row's history, never an edit. *(N-14 already carries one: the owner retracted a provider-embeddings choice the same day, which also corrected a vector dimension from 1536 to 384.)*

| Date | Rev | Change |
|---|---|---|
| 2026-08-31 | 1.0 | Created. Twenty rows: 13 `ADOPTED-INTENT`, 3 `TERRAIN`, 4 `REJECTED`/`REJECTED-AND-REPLACED`. Two acknowledged divergences from governing anchors recorded (N-13 storage, N-19 client deferral). N-09/N-10 split after the review disproved the plan's claim that they were the same model. |
