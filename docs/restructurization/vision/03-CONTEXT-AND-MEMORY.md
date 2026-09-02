# 03 — Context and Memory

| | |
|---|---|
| **Parent** | `00-…-MASTER-PLAN.md` §Part 1 (INV-7, INV-8), §Part 1B (M-04, M-05, M-10, M-18), §Part 3.3 |
| **Locks** | Hands **ADR-01-06**, **ADR-01-07**, **ADR-01-08**, **ADR-05-01**…**ADR-05-05**, **OQ-7** to `00 §Part 8` / §Part 9 |
| **Also governed by** | `docs/restructurization/memory-anchor.md` — adopted as a reference by Razi, 2026-08-31. Its **principles** apply in full; its §21 storage recommendation (PostgreSQL + pgvector) is re-instantiated for the desktop per ADR-05-01 |
| **Status** | PLANNED |
| **Grounding** | `_grounding/G4c` (the full port spec, formulas included) · `G4b §C.4, §C.5` · `G2a §2` (BUG-TOKENS) · `G6 §1.C.1, §1.C.5` |

---

## What this is

Everything that decides **which bytes are sent to the model** on a given turn. It is the direct answer to BUG-TOKENS, and it is the subsystem where the port is most likely to go wrong — because the thing being ported is itself defective in ways that look like features.

**The subtle failures this subsystem is prone to:**

1. **Faithfully porting a bug.** The sidecar's semantic recall **ignores its `query` parameter entirely** while its docstring says otherwise, making recall identical for every turn in a session. Its episodic cache key ignores the query too, so two different questions share one entry. Its HITL writer **always throws**, so an entire memory subtype has never once been written. A faithful port reproduces all three.
2. **Optimising a thing nobody reads.** Memory recall must be *proven* to reach the prompt (INV-8). Today's Rust recall does reach it — but with a hardcoded empty query, which makes it recency, not relevance.
3. **Mechanisms that fight each other.** Clearing tool results **invalidates the cached prefix and forces a cache write**. A naive "clear early, clear often" policy costs more than it saves.
4. **Changing behaviour while calling it a port.** The three tiers are budgeted *competitively*, not by quota. Assigning per-tier quotas is a redesign — a defensible one, but it must be a decision, not an accident.

---

## 1. First principles

**Context is assembled, not accumulated.** The current engine does `messages: history.clone()` every turn (`engine.rs:196`) and appends two messages per turn across up to 20 tool turns, so turn *k* re-bills turns 1..k-1. Assembly means: choose what goes in, under a budget, every turn.

**The prefix is stable; the tail is live.** Everything cacheable goes first, in a byte-stable order (spec `02 §4.1`). Everything volatile goes after.

**Memory is a reader-proved subsystem.** Every tier must have a test asserting a recalled item appears in the outbound prompt bytes.

---

## 2. The layered strategy

Precedence order — this is the substance of ADR-01-06.

```
┌─ cached prefix ─────────────────────────────┐
│ tools → system → AUTODEV.md → recalled       │   ← 0.1× on read
└──────────────────────────────────────────────┘
┌─ live history ──────────────────────────────┐
│ recent turns, tool calls and results          │   ← full price
└──────────────────────────────────────────────┘
        │ above threshold
        ▼
  clear_tool_uses            ← drops old tool results
        │ still growing
        ▼
  compaction                 ← summarises, last resort
```

### 2.1 Why this order

**Cache first** because it is the only mechanism that is pure gain — it changes price, not content.

**Clear second, and carefully.** `clear_tool_uses` is the direct fix for the classic failure: a 200-line file dump from turn 3 still being resent at turn 30. But clearing **invalidates the cached prefix from the clear point onward and incurs a cache write**. `clear_at_least` exists precisely so the clear is worth the re-write, and it must be tuned against the cache-write cost, not set by feel.

**Compact last** because it costs an extra full-context model call and it is lossy. It is the long-session backstop, not the primary mechanism.

### 2.2 Current state, for contrast

⚠ **Updated 2026-09-02 after P2 (§0.3).** This table described the pre-P2 codebase. Two of the
four layers have since landed, and leaving the old text would have had P3 rebuild them.

| Layer | Today |
|---|---|
| Cached prefix | ✅ **Landed in P2.** `CachePolicy` on `CompletionRequest`, wired on all three adapters, and **proven live**: Anthropic wrote 11,923 tokens and read back 11,923; OpenAI went 8,659 input → 211 + 8,448 cached. The alarm that detects a *broken* cache (`cache::monitor`) shipped **before** the caching, per `§0.2e`. ⚠ One caveat P3 inherits: the audit found the stable/volatile split built and **not wired** — the engine passed the joined prompt, so the memory block sat inside the cached region. Fixed, and `TG-P2-11` now holds it |
| Live history | Full clone every turn — **unchanged**, and `P3-25` owns it |
| `clear_tool_uses` | **Still not implemented.** `P3-25`/`P3-28` own it, and `P3-28`'s tuning problem is real: clearing invalidates the prefix from the clear point and forces a cache write, so the saving must exceed that cost |
| Compaction | ✅ **Trigger fixed in P2** (`P2-57`): reads `ModelInfo::context_window` from the shared catalogue, not a hardcoded 200,000. ⚠ **The first fix was inert** — the adapters' model lists had drifted a generation behind the picker, so the lookup found nothing for eleven of twelve shipped models and fell through to the 200k default anyway; the catalogue landed to make the lookup hit (`L-18`). The compacted history **is now used** by the caller, and compaction's own usage is folded into the turn total, which it previously discarded entirely |

**What P3 still owns here:** the live-history layer, `clear_tool_uses` and its cost tuning, and
the layered *assembly* that orders all four. The two landed layers are inputs to that work, not
work to redo.

### 2.3 Tool-output caps

What enters history in the first place (`G2a Q6`, all confirmed by reading bodies): bash 50 KB · `read_file` 2000 lines / 256 KB · grep 100 matches (**no per-line cap**) · `list_directory`'s `max_entries` **does not actually bound a directory**.

Two of those are defects rather than policy, and both belong to spec `06`. The relevant point here: **caps are the first line of context defence**, and an unbounded grep line or directory listing defeats every layer above.

---

## 3. Graph-scoped selection (M-18)

The code graph exists, is queryable, and **nothing reads it for context** (`G2a §4`). Worse, the two modules that would expose it to an agent are not compiled at all (spec `05`).

**The mechanism:** given a task and the graph, produce a ranked, budgeted set of files and symbols likely to matter — by import/call adjacency to the task's named entities, by community membership, and by recency of change. This is `MATH`, not `ORGAN`.

**Why it belongs here:** it is the difference between "send the repo" and "send the twelve files that matter". It is the single largest content-side lever, as caching is the largest price-side lever.

---

## 4. The three tiers

Full mechanics, formulas and thresholds are in `_grounding/G4c`. This section carries the decisions and the deltas.

### 4.1 What each tier is

| Tier | A record is | Written when | Read how |
|---|---|---|---|
| **Episodic** | one message (a turn produces two) | every turn, verbatim, no inference | hybrid: relevance search + recency tail, deduped |
| **Semantic** | one extracted fact about the user/project | fire-and-forget after a **user** message only | vector/lexical search over facts |
| **Procedural** | one learning event (tool success/failure, user correction) | on tool outcomes and corrections | search, filtered by subtype for the auxiliary readers |

**Only user messages produce semantic facts** — agent responses are stored without inference, deliberately, to prevent circular reasoning. That constraint is worth preserving.

**`user_correction` is the highest-authority record class** in the scoring model (authority 0.85, and `correction_boost` 1.0). That is a good design and it survives.

### 4.2 Budgeting — the decision that changes behaviour

The sidecar unifies all three tiers into **one pool** and ranks them against each other by a two-stage utility function; a tier's share is *emergent* from `type_bonus`, `authority` and `session_anchor` (`G4c §4.4`).

```
Stage 1 — absolute utility (weights sum to 1.0)
U_abs(c) = clamp01( w_rel·relevance + w_auth·authority + w_rec·recency
                  + w_type·type_bonus + w_prio·priority + w_corr·correction
                  + w_sess·session_anchor + w_proc·procedural_fit
                  − w_len·length_penalty )

Stage 2 — marginal utility, greedy selection
U_marg(c|S) = max(0, U_abs(c) − 0.30·redundancy(c,S)
                            + 0.25·gap_fill(c,F)
                            − 0.20·contradiction(c,S))

stop when best U_marg < 0.15, or the token budget is exhausted
```

with a **14-day half-life recency decay** (`exp(−ln2/14 · age_days)`) and seven archetype weight presets selected by first-match keyword detection.

**ADR-01-08 asks whether to port this or use per-tier quotas.** The honest framing: this is the sophisticated part of the design, it is fully specified, and it is directly relevant to BUG-TOKENS — but the recall path it ranks is itself defective (§4.3), so its value has never actually been demonstrated in this product.

### 4.3 Defects to fix, not reproduce

`G4c §5.4` — each of these is in the code being ported:

| # | Defect | Consequence |
|---|---|---|
| 1 | **Semantic recall drops its `query`** — the vector query is a fixed template string, despite a docstring saying otherwise | Recall is **query-independent**; identical for every turn until the cache expires |
| 2 | **Episodic cache key ignores the query** (`episodic:{user}:{session}`), with a comment claiming it hashes the query | Two different questions in one session share a cache entry |
| 3 | **`learn_hitl_pattern` always throws** — it passes a `metadata` argument the callee does not accept; the `TypeError` is swallowed | **No record with `subtype == "hitl_interaction"` has ever been written**, and its reader can only ever return `[]` |
| 4 | **Procedural recall has no type filter** and its cold fetch has **no circuit breaker** — the constructed breaker is never used | Cross-tier bleed by construction |
| 5 | **Episodic recall never forwards `agent_id`** despite the callee supporting it as documented "defence in depth" | Recall is unscoped by agent — a cross-agent leakage risk with a defence that is present and unused |
| 6 | **The negative cache never fires** — every tier gates its return on `if not strict_lossless`, and the config sets `strict_lossless: true` | Negative markers are written on every empty result and never honoured |
| 7 | **Semantic cache is not invalidated on write** | A newly extracted fact is invisible for up to 30 minutes |

**These are listed in the spec so the port has an explicit fix-list, not so the port has an excuse.**

### 4.4 Storage and retrieval — as decided

`~/.autodev/memory.db` already has the four tables and is **already Rust-owned** — the sidecar's memory service is a *client* of it and says so in its own source (`G4b §C.3`). There is nothing to migrate; there is a Python wrapper to delete.

**The substrate** (ADR-05-01): **SQLite + `sqlite-vec` + FTS5**, behind a retrieval-backend trait.

| Anchor requirement (`memory-anchor.md` §9) | Implementation |
|---|---|
| Dense vector search | `sqlite-vec` — Rust crate compiles the C source and statically links it; no extension loading, no runtime dependency |
| Keyword / BM25 | **FTS5**, built into SQLite |
| Metadata filtering | SQL columns, including scope |
| Structured metadata + relational queries | SQLite, already statically bundled (`Cargo.toml:41`) |

One file. No server, no port, no second process — INV-1 holds.

**Embeddings** (ADR-05-02, final): **local, via `fastembed-rs`, model BGE-small-en-v1.5 — 384 dimensions, ~130 MB.**

*(An earlier answer chose OpenAI `text-embedding-3-small` at 1536 dims and was retracted the same day. The `sqlite-vec` column width is **384**, not 1536; the ledger records the correction.)*

Local embedding removes the BYOK coupling entirely — no user is forced to hold an OpenAI key to get semantic memory on a Claude-only or Gemini-only setup — and memory writes depend on neither the network nor the user's inference budget.

**Three design constraints:**

1. **The model download is a first-run surface.** ~130 MB, once. It is the single heavyweight step in an otherwise light onboarding, so it must show progress, survive interruption, and be **skippable** — skipping degrades retrieval rather than blocking the app (M-35).
2. **Degradation, not failure.** Before the model is present — mid-download, or if skipped — retrieval falls back to **FTS5/BM25 + metadata filtering**, two of the anchor's three legs (§9). Memory works, less well, and reports that it is degraded.
3. **`fastembed` is synchronous and Tokio-free.** Inference runs on a blocking thread pool (`spawn_blocking`), never on the async runtime, or it stalls every other task on that worker. Embedding remains **asynchronous with respect to a turn**: a write enqueues and embeds behind it, and recall works lexically in the interim.

**One honest limitation:** BGE-small-en-v1.5 is English-centric, so recall on non-English content will be weaker than a multilingual model would give. The backend is behind a trait and the model is a config value, so this is a swap rather than a redesign if it matters later.

### 4.5 Scope, provenance, and the gateway

Three additions from the anchor, all decided.

**Scopes** (ADR-05-03) become first-class columns and a **mandatory retrieval filter**: `global` (user / agent) · `project` · `task`. This is not a nice-to-have — it structurally closes a defect we observed: sidecar episodic recall **never forwards `agent_id`** despite the callee supporting it as documented "defence in depth" (`G4c §5.4.5`), and procedural recall has no type filter at all. The anchor's framing is exact: *"Scope is critical because it prevents irrelevant memories from contaminating retrieval."*

*Working memory* as a fourth persisted tier is **deferred** (OQ-7). The schema uses a tier **discriminator value**, not a table per tier, so adding it later needs no migration.

**Provenance** (ADR-05-05, pulled forward from anchor Phase 4) — every memory carries a `source`: `user` · `agent_observation` · `repository` · `tool_result` · `test_result` · `conversation`, with a file/line where one applies. One column, and it is what makes contradictions *resolvable later* instead of requiring a migration to add. It also lets the agent distinguish *"I know this because the user told me"* from *"I believe this because I observed it"* — which is the difference between a fact and a guess.

**The gateway** (ADR-05-04) — agents get `search_memory`, `get_memory`, `list_memories` and **`propose_memory`**. `remember_this`'s unrestricted write is **removed**. A deterministic gateway applies the anchor's §18 evaluation — importance, novelty, duplicate check, confidence — and decides what is persisted.

This is the doctrine boundary from `00 §Part 2` in its clearest form:

| Act | Balance |
|---|---|
| Judging *what was learned* from a completed task | `ORGAN` |
| Deciding *whether it is worth persisting* | `DET` — importance/novelty/duplication policy |
| Storing, embedding, indexing | `DET` |
| Ranking at recall | `MATH` |

### 4.6 Scope of delivery

**Anchor Phases 1–3 plus provenance** (ADR-05-05). In: store/search/get/list; hybrid retrieval with reranking; quality (importance, confidence, recency, dedup); provenance. Out, with a named follow-up: consolidation, contradiction resolution, autonomous memory creation, knowledge graphs, multi-hop retrieval.

Composes with ADR-01-08 — simple per-tier budgeting ships first; the anchor's §10 reranker (which *is* the competitive model) arrives in Phase 2.

### 4.7 What not to port

`G4c §6`, condensed: **Redis as a warm tier** (a second cache tier exists to share state between processes; after the restructure there is one process) · **mem0 + Qdrant** · the `[MEMORY_TYPE: …]` marker protocol and its defensive strip-on-read (an artifact of routing three tiers through one prompt — with a real schema, tier is a column) · `PredictiveStager`, the BLAKE3 event-journal WAL, and `ContinuityPacket` (server infrastructure) · `TokenValidator` and the `token_policy` trim/compaction machinery (already unreachable) · the two dead config blocks · **the second parallel recall path** in `build_agent_context` with its own hardcoded limits.

---

## 5. Determinism balance

| Step | Balance |
|---|---|
| Prefix assembly | `DET` |
| Graph-scoped selection | `MATH` |
| Memory recall + scoring | `MATH` (+ `EMB` if ADR-01-07 chooses it) |
| `clear_tool_uses` | `DET` — a provider-side rule |
| Compaction | `ORGAN` — one extra call, rarely |
| Semantic fact extraction | `ORGAN` — one per user message, or removed by ADR-01-07 |

---

## 6. Test gates

| Gate | Asserts | Catches |
|---|---|---|
| **G-CTX-BOUND** | A 30-turn session's input tokens do **not** grow linearly with turn count | The full-history clone |
| **G-CTX-CACHE** | From turn 2, `cached_read > 0` on every provider that supports it | Zero caching |
| **G-CTX-STABLE** | The prefix builder is byte-identical across repeated identical inputs | The non-byte-stable system prompt |
| **G-COMPACT-FIRES** | With a small context window, compaction triggers, and the **compacted history is what the next turn sends** | The unreachable trigger *and* the discarded result |
| **G-CLEAR-WORTH** | `clear_tool_uses` fires only when the saving exceeds the forced cache write | Clearing that costs more than it saves |
| **G-MEM-READ** (INV-8) | A memory known to be recalled appears in the outbound prompt **bytes** | Recall that reaches nothing |
| **G-MEM-QUERY** | Two different queries in one session produce **different** recall sets | Defects 1 and 2 |
| **G-MEM-SCOPE** | Agent A cannot recall agent B's episodic memory; project A cannot recall project B's facts | Defect 5 |
| **G-MEM-SUBTYPES** | Every declared memory subtype has at least one successful write in a full run | Defect 3 — a subtype never once written |
| **G-GRAPH-SCOPE** | With a graph present, context selection sends measurably fewer files than without, and the right ones | Graph with no reader |
| **G-TOOLCAP** | Every tool's output is bounded; grep bounds per line; `list_directory` bounds entries | Unbounded context entry |
| **G-MEM-DEGRADE** | With the embedding model **absent or still downloading**, recall still returns results via FTS5/BM25 and reports itself degraded — it never returns empty and never errors | The skipped/incomplete model download |
| **G-MEM-ASYNC** | Embedding never delays a turn; the write lands and embeds behind it. Inference runs off the async runtime (`spawn_blocking`) and does not starve other tasks | Embedding on the critical path; a sync model call blocking a Tokio worker |
| **G-MEM-DOWNLOAD** | The first-run model download shows progress, resumes after interruption, and can be skipped — skipping yields a working, degraded app | A ~130 MB silent stall at first launch |
| **G-MEM-SCOPED** | Scope is a mandatory filter: a `project`-scoped memory is unreachable from another project; an agent-scoped one from another agent | The unscoped-recall contamination class |
| **G-MEM-PROVENANCE** | Every stored memory carries a source; a repository-sourced memory carries file and line | Unauditable memory |
| **G-MEM-GATEWAY** | `remember_this` no longer exists; a proposal that fails the importance/novelty/duplicate check is **not** stored, and the agent is told why | Unrestricted agent writes |
| **G-MEM-RECALL-QUALITY** | Recall@5, Recall@10 and MRR over a fixed labelled corpus meet thresholds set at Phase 2 | The anchor's §23 retrieval suite |
| **G-MEM-VALUE** | The anchor's real metric: over a repeated task, the number of **rediscovered facts** falls | Memory that exists but prevents no work |
| **G-REDACT** | ⚠ **ADDED at rev 0.9 (M-40).** A file containing a recognised secret, read by a tool, appears **redacted** in the prompt, in the logs, and in the memory store | The dropped redaction port — and `G4:486`'s unredacted tool arguments reaching procedural memory, which is now permanent |
| **G-MEM-FORGET** | ⚠ **ADDED at rev 0.9 (G-07).** A user can list what is stored, export it, and delete a single memory or everything for a project — and a deleted memory is **not** returned by any later recall | Memory that is permanent and invisible |

---

> No ADR decision is recorded in this spec. It hands **ADR-01-06**, **ADR-01-07**, **ADR-01-08** and **ADR-05-01…ADR-05-05** to `00 §Part 8`, and **OQ-7** to §Part 9.
