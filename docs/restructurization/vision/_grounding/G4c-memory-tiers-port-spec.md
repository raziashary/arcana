# G4c — The Three-Tier Memory Model: port specification

**Source:** `submodules/core-orchestrator/src/airs/memory/` + `config/memory_config.yaml` + `api_gateway/services/context_service.py`
**Purpose:** the authoritative reimplementation spec for Episodic / Semantic / Procedural memory in Rust
**Date:** 2026-08-31
**Status:** COMPLETE

> **Companion documents:** `G4-livingagent-memory-semantics.md` (the LivingAgent loop, §1.1–1.16 — note that file is TRUNCATED at §1.16) · `G4b-sidecar-subsystems-api-infra.md` (everything else in the sidecar).

**Files read in full:** `memory_core.py` (2654), `episodic.py` (496), `semantic.py` (116), `procedural.py` (453), `memory_config_loader.py` (492), `memory/__init__.py` (169), `config/memory_config.yaml` (485), `api_gateway/services/context_service.py` (2798), plus `context_scorer.py`, `relevance_scorer.py`, `bm25_search.py`, `token_validator.py`, `staleness_tracker.py`, `compression_engine.py`, `ordinal_intent.py`, `query_expander.py`, `paradigm_integration.py`, `continuity_packet.py`, and the memory-mcp-server configs.

---

## 0. Architectural frame — READ THIS FIRST

**There is no SQL in this service.** Grep for `CREATE TABLE` across the repo returns only `assets` and test fixtures — nothing memory-related. There are no Alembic migrations. **All three tiers are stateless façades over one external MCP server** (`memory-mcp-server`, Streamable HTTP, port 8000), which wraps **mem0 OSS** over **Qdrant**.

Everything the three tiers do reduces to exactly **three MCP tool calls**:

| MCP tool | Used by | Semantics |
|---|---|---|
| `store_memory` | all three tiers (write) | mem0 `add()`; `infer` flag controls LLM fact extraction |
| `search_memories` | episodic-relevant, semantic, procedural | mem0 vector search (cosine over Qdrant) |
| `list_memories` | episodic-recent, timeline, verbatim | non-ranked enumeration; sorting done client-side in Python |

**Single physical store, logical tiers.** All three tiers write into the *same* Qdrant collection. **Tier identity is carried purely in `metadata.type ∈ {"episodic","semantic","procedural"}` plus a `tags` array (`"tier:1" | "tier:2" | "tier:3"`). There is no per-tier table, index, or namespace.**

### Storage backend (exact)
`submodules/memory-mcp-server/config/production.yaml:25-46`

```yaml
vector_store:
  provider: "qdrant"
  config:
    collection_name: "mem0"
    embedding_model_dims: 1536
    host: "localhost"      # QDRANT_HOST
    port: 6333             # QDRANT_PORT
    on_disk: true
llm_provider: "openai"
llm_config: { model: "gpt-4o-mini", temperature: 0.1, max_tokens: 4000 }
embedder_provider: "openai"
embedder_config: { model: "text-embedding-3-small" }
```

- **Embedding model:** `text-embedding-3-small`, **dimension 1536**, one shared vector space for all three tiers.
- **Collection:** `mem0` (single collection).
- **Index definition:** none declared — Qdrant default HNSW, cosine distance. **UNVERIFIED** whether payload indexes exist on `metadata.type` (the config never declares any, so `filters` are almost certainly unindexed payload filters).
- **Graph store:** `enable_graph: False` (`production.yaml:88`). Neo4j config present but disabled.
- **Extraction LLM:** `gpt-4o-mini`, temp 0.1, max_tokens 4000, driven by a `custom_fact_extraction_prompt` (`production.yaml:50-85`) that branches on a `[MEMORY_TYPE: ...]` marker:
  - `memory:episodic` → store remaining content **100% verbatim as ONE fact** (no summarization)
  - `memory:semantic` → extract discrete facts/preferences
  - `memory:procedural` → extract patterns formatted `"When X, then Y"` / `"Tool Z fails when W"`

> ⚠️ **This contradicts `G4b §C.4`,** which found no vector store wired up anywhere in `core-orchestrator`. The two submodules disagree about whether Qdrant exists in the live path. **Resolve by ADR before porting.**

### Circuit breakers (per agent) — hardcoded, NOT from config
`memory_core.py:249-272`:

| Breaker | failure_threshold | timeout_seconds | half_open_successes |
|---|---|---|---|
| `episodic_{agent_id}` | 3 | 30.0 | 2 |
| `semantic_{agent_id}` | 3 | 30.0 | 2 |
| `procedural_{agent_id}` | 3 | 30.0 | 2 |
| `redis_{agent_id}` | 5 | 10.0 | 2 |

> **Discrepancy:** `memory_config.yaml:350-353` declares `circuit_breaker: {failure_threshold: 5, recovery_timeout: 30, …}` and the loader parses it into `CircuitBreakerConfig`. **`memory_core.py` never reads it.** The config block is dead.

---

## 1. Episodic Memory (Tier 1)

`src/airs/memory/episodic.py` — class `EpisodicMemory` (line 20).

### 1.1 Record schema
A "memory" is **one message** (not one turn). Every interaction produces **two** episodic records: one `user_message`, one `agent_response`.

Written payload (`context_service.py:315-327` user, `:442-454` agent):

| Field | Type | Value / notes |
|---|---|---|
| `content` | `str` | Raw message text, **verbatim, no truncation** |
| `user_id` / `session_id` / `agent_id` | `str` | |
| `metadata.type` | `str` | `"episodic"` |
| `metadata.subtype` | `str` | `"user_message"` \| `"agent_response"` (retrieval also admits `"a2a_inbound"`, `"a2a_outbound"`, `"conversation"`) |
| `metadata.timestamp` | ISO-8601 UTC | `datetime.now(timezone.utc).isoformat()` |
| `metadata.turn_index` | `int` | **1-indexed** ordinal within session. User message increments; **agent response reuses the same index** (`:292`, `:386`, `:393`) |
| `metadata.has_thinking` / `.thinking` | `bool` / `str?` | agent only, full CoT text |
| `metadata.has_tool_calls` / `.tool_count` / `.tool_calls` | `bool` / `int` / `list[dict]?` | agent only |
| `metadata.message_id` / `assistant_message_id` / `user_message_id` | `str?` | correlation ids (`episodic.py:367-370, 390-394, 405-410`) |
| `metadata.image_urls` / `file_urls` / `file_types` | `list[str]?` | presigned URLs (`episodic.py:373-382`) |
| `tags` | `list[str]` | `["memory:episodic", "source:user"\|"source:agent", "session:{sid}", "agent:{aid}", "user:{uid}", "category:conversation", "tier:1"]` plus `"has:thinking"`, `"has:tool_calls"`, `"used_tool:{name}"` per tool |
| `infer` | `bool` | **`False`** for both — episodic writes never trigger LLM extraction |

**The in-process hot-cache record has a DIFFERENT shape** (`memory_core.py:449-470`) — a single fused turn:
```python
"content": f"[USER:{user_id}] {user_message}\n\n[AGENT:{agent_id}] {agent_response}"
```
with `role: "assistant"`, `timestamp`, and a 14-key `metadata` dict. **The Rust port must reproduce this fused-string format** — it is what `_format_context()` speaker attribution expects and what prevents cross-agent persona leakage (comment at `:445-447`).

### 1.2 Write path
Trigger: `MemoryCore.remember()` → background task → `EpisodicMemory.store(interaction)` (`episodic.py:354`).

1. Merge `interaction.metadata`; hoist `image_urls`/`file_urls`/`file_types` into metadata with `has_*` booleans (`:373-382`).
2. `store_user_message(...)` — **idempotency guard**: if `metadata.message_id` is a non-empty str, fetch the last **200** timeline messages (`asc=False`) and short-circuit `return True` if any `subtype=="user_message"` has a matching `message_id` (`context_service.py:270-289`). **This is the only dedup on the write path.**
3. Increment `turn_index`; `store_memory(..., infer=False)` via `_call_memory_with_retry`.
4. **Fire-and-forget** `asyncio.create_task(_safe_extract())` → semantic extraction (§2.2).
5. `store_agent_response(...)` — same, `infer=False`, `turn_index` **not** incremented.

**Contradiction handling** happens one level up in `MemoryCore.remember()` (`memory_core.py:1978-2015`): `ContradictionDetector.detect_contradictions(new_memory, existing_memories=<facts from first 5 semantic cache entries, ≤20 each>, scope_tags=["user:{uid}","session:{sid}","agent:{aid}"], confidence_threshold=0.5)`. **Conflicts are logged only** (top 3) and recorded as a metric — they never block, merge, or supersede a write.

**Embeddings** are generated server-side by mem0. No embedding is computed in this service on the write path.

### 1.3 Read / recall path
`EpisodicMemory.recall(user_id, session_id, query, timeout_per_source=3.0)` (`episodic.py:55`).

**Step 1 — ordinal routing** (`:88-101`). `detect_ordinal_intent(query)` matches 5 regex families over `ORDINAL_WORDS` (`ordinal_intent.py:42-61`): `first..fifteenth`/`1st..15th` → 1..15, `last`/`latest` → **-1**, `previous` → **-2**; against `TYPE_KEYWORDS` (`:64-79`) which also sets a `role_filter` (`question|request|query` → `"user"`; `response|answer|reply` → `"assistant"`).

If matched → `_recall_by_ordinal` (`:142`):
- Negative ordinal: `recall_timeline(limit=20, offset=0, asc=False, role, agent_id)` with `wait_for(timeout=3.0)`; `actual_index = abs(ordinal) - 1`.
- Positive ordinal: `recall_timeline(limit=max(ordinal + 2, 10), offset=0, asc=True)`; `target_index = ordinal - 1`.
- Result assembled by `_build_ordinal_result` (`:267`): **1 message before + target + 1 message after**, annotated `_role ∈ {context_before, primary_target, context_after}`, `_turn_index`, `_is_requested_ordinal`, `_timeline_summary` on element 0.
- Miss → `_format_timeline_for_ordinal_miss` attaches `_ordinal_miss {requested, available, message}`.

**Step 2 — standard hybrid recall** (`:105-126`). `asyncio.gather` of two sources, each wrapped in `with_timeout_and_fallback(timeout=3.0, fallback=[])`:

**(a) Relevant** → `_retrieve_relevant_conversation` (`context_service.py:1879`):
```python
call_tool("search_memories", {
    "query": current_query,
    "user_id": user_id,
    "session_id": session_id,
    "limit": limit * 2,                       # 5*2 = 10
    "filters": {"metadata.type": "episodic"}
})
```
then `_filter_episodic_conversation()` then `memories[:limit]` → **top-5**. Ranking is mem0's raw cosine order; no re-ranking here.

**(b) Recent** → `_retrieve_recent_conversation` (`:1806`):
```python
call_tool("list_memories", {
    "user_id": user_id, "session_id": session_id,
    "limit": limit * 3,                       # 5*3 = 15
    "filters": {"metadata.type": "episodic"}
})
```
then filter, then **client-side sort ascending by** `metadata.timestamp or created_at`, then `memories[-limit:]` → **last 5**.

**Step 3 — dedup**: `_deduplicate_messages(relevant, recent)` (`:1614`) — `hash(content.strip().lower())`, relevant first (higher priority), recent appended if unseen. Order preserved: semantic block then chronological block.

`_filter_episodic_conversation` (`:1742`) admits `allowed_subtypes = {"user_message","agent_response","a2a_inbound","a2a_outbound","conversation"}` where `metadata.type == "episodic"`, **and unconditionally admits any memory with empty/absent metadata** (`:1780-1786`) because mem0 OSS omits metadata on search results.

**Other read modes:**
- `recall_timeline(session_id, user_id, limit=10, offset=0, asc=True, role=None, agent_id=None)` (`episodic.py:418`) → `get_timeline_messages` (`context_service.py:2028`): `list_memories` with `fetch_limit = (limit + offset) * 3`; role filter maps `"assistant"` → subtype `"agent_response"`; **dedup by key `f"{turn_index}:{subtype}"` preferring the record carrying `tool_calls` or `thinking`**; sort key `(turn_index or 999999, timestamp)` with `reverse=not asc`; slice `[offset:offset+limit]`.
- `recall_exact(session_id, user_id, phrase, agent_id=None)` (`:462`) → `get_exact_match` (`:2172`): `list_memories` with **hardcoded `limit: 50`**, then case-insensitive substring. No ranking, no result limit.

### 1.4 Caching, decay, eviction
- **Hot:** in-process `OrderedDict` LRU, key `f"episodic:{user_id}:{session_id}"` (`memory_core.py:346`), **size 100, TTL 300 s**.
- **Write-through cap: 50 experiences per session key**, newest prepended (`:479-483`).
- **Warm:** Redis `SETEX` same key, **TTL 7200 s** (2 h), value = JSON of the whole list, guarded by the Redis breaker with a **0.1 s** timeout (`:531-539`).
- **Negative cache TTL:** 15 s for episodic — **but see §5.3, it never fires.**
- **Staleness probe on hot hit** (`:766-829`): if `optimization.staleness_check` and Redis present, `GET` warm and replace hot when `warm_len > hot_len` **or** (`warm_len == hot_len` and `max(warm.timestamp) > max(hot.timestamp)`).
- **Age-based compression, applied ONLY to episodic** (`:1348` → `_apply_compression_to_memories:2419`):
  ```
  age_days ∈ [0,1)  → L0   0%     reduction
  age_days ∈ [1,3)  → L1  30-40% reduction
  age_days ∈ [3,7)  → L2  50-60% reduction
  age_days ≥ 7      → L3  70-80% reduction
  ```
  (`compression_engine.py:19-24`, `:331-337`). Diminishing returns: L0→L1 = 50 % of max savings (+50 ms), L1→L2 = 25 % (+25 ms), L2→L3 = 12 % (+12 ms).
- **No TTL, no eviction, no deletion in the durable store.** Episodic memories persist in Qdrant forever.

---

## 2. Semantic Memory (Tier 2)

`src/airs/memory/semantic.py` — class `SemanticMemory` (line 19). **The thinnest tier: 115 lines, one real method.**

### 2.1 Record schema
A "memory" is **one LLM-extracted fact string** about the user, produced by mem0's `infer=True` path. **The orchestrator never authors a fact directly.**

Written envelope (`context_service.py:1164-1195`):

| Field | Value |
|---|---|
| `content` | The **raw user message** — mem0 decomposes it into facts server-side |
| `metadata.type` | `"semantic"` |
| `metadata.source` | `source_type`, e.g. `"user_message"` |
| `metadata.extracted_at` | ISO-8601 UTC |
| `metadata.confidence` | **`0.95` if `source_type == "user_message"` else `0.85`** (`:1168`) |
| `tags` | `["memory:semantic","extracted:facts","source:{t}","session:{sid}","agent:{aid}","user:{uid}","category:knowledge","tier:2"]` |
| `infer` | **`True`** |

Note this write carries **no `subtype`**, which matters for the read-side filter (§5.4).

### 2.2 Write path
**`SemanticMemory.extract_and_store(interaction)` (`semantic.py:91`) is a NO-OP.** Its entire body is a `logger.debug` inside a `try/except` (`:104-115`). The comment states extraction "already" happened.

The real trigger is inside the *episodic* write: `store_user_message` spawns `asyncio.create_task(_safe_extract())` → `_extract_semantic_knowledge(..., source_type="user_message")` (`context_service.py:332-344`, `:1139`).

**Consequences to preserve or deliberately change in the port:**
- **Only user messages produce semantic facts.** Agent responses are stored `infer=False` — deliberate, to prevent circular reasoning (`semantic.py:29-30`, `context_service.py:451`).
- Extraction is **fire-and-forget**, unawaited, un-retried; failure swallowed at `debug` level.
- **The same user text is written TWICE to Qdrant:** once as episodic (`infer=False`), once as semantic (`infer=True`).
- **Dedup / contradiction / update is entirely delegated to mem0's ADD/UPDATE/DELETE fact reconciliation.** This service implements none of it for semantic facts.
- `store_tool_failure` and `store_user_correction` also pass `infer=True`, so procedural writes additionally spawn extracted facts.

### 2.3 Read / recall path
`SemanticMemory.recall(user_id, query, timeout=3.0)` (`semantic.py:51`) → `_retrieve_user_facts` (`context_service.py:1955`):

```python
call_tool("search_memories", {
    "query": f"all information and facts about user {user_id}. "
             f"Such as preferences, likes, dislikes, history, etc.",
    "user_id": user_id,
    "limit": limit          # 10 from config
    # NOTE: "filters": {"type": "semantic"} was REMOVED - mem0 doesn't support this!
})
```

- **top-k = 10.**
- **No type filter, no session filter, no agent filter.** Semantic recall returns whatever mem0 ranks highest for that boilerplate query across *all* memory types for the user — episodic and procedural rows included.
- **No post-filtering.** The result is returned as-is.

> **Load-bearing discrepancy.** `semantic.py:60-66` documents `query` as "Current query (for relevance matching)" and `:73` comments *"Pass the query through so semantic search can match user preferences"* — **the code never passes `query`** (`:74-81` forwards only `user_id` and `limit`). The vector query is the fixed template string above.
>
> **Report the CODE: semantic recall is query-independent.** Semantic results are identical for every turn in a session until the cache expires — consistent with the cache key ignoring the query (§2.4).

### 2.4 Caching
- Hot key: `f"semantic:{user_id}"` (`:350`) — **no session, no query**. Size **100**, TTL **1800 s** (30 min).
- Warm: Redis, TTL 7200 s, breaker-guarded at 0.1 s.
- Negative TTL: 30 s.
- **Semantic cache is NOT invalidated on write** — comment at `:564-566`: *"semantic cache stays valid until TTL (30min)"*. **A newly extracted fact is invisible for up to 30 minutes.**
- No compression (only episodic is compressed). No TTL/eviction in Qdrant.

---

## 3. Procedural Memory (Tier 3)

`src/airs/memory/procedural.py` — class `ProceduralMemory` (line 19).

### 3.1 Record schema
A "memory" is **one learning event**, always authored as a formatted natural-language block. Five writers, distinguished by `metadata.subtype`.

**tool_success** (`context_service.py:1204-1258`)
```
content:  f"Successfully used {tool_name} for: {context}\nResult: {result_summary}"
user_id:  "system"                      # <- procedural rows are owned by user "system"
metadata: {type:"procedural", subtype:"tool_success", tool, outcome:"success", timestamp}
tags:     ["memory:procedural","outcome:success","tool:{name}","agent:{aid}",
           "session:{sid}","category:learning","tier:3","reusable:true"]
infer:    False
```

**tool_failure** (`:1267-1357`) — multi-line reflection:
```
Tool: {tool_name}
Status: Failed
Context: {context}
Error: {error}
[Attempted Solution: ...]
[Resolution: ...]
[User Correction: ...  + "IMPORTANT: User indicated I was wrong. Learn from this!"]
Lesson: Never repeat this mistake. Remember this failure.
```
tags add `"priority:high"`, `"avoid:true"`; `session_id` **defaults to the literal string `"procedural"`**; `infer: True`.

**user_correction** (`:1366-1438`) — **the highest-authority record in the whole system:**
```
User Correction - I was WRONG:
What I said: {incorrect_response}
User's correction: {user_correction}
[Correct approach: {corrected_approach}]
CRITICAL: Remember this correction. Never make this mistake again.
```
tags include `"priority:critical"`, `"must_remember:true"`; `infer: True`.

**retry_resolution** — not a distinct subtype; `learn_retry_pattern` (`procedural.py:139`) reuses tool_success/failure with `tool_name="retry_resolution"`.
**reasoning** — `store_reasoning` (`:425`) → `store_tool_success(tool_name="reasoning", ...)`.
**task_failure** — `learn_failure_pattern` (`:222`) → `store_tool_failure(tool_name=f"task_failure_{failure_type}", context=context[:500], error=error)`.

**Truncations to preserve:** `result_summary` → **200 chars** (`:120`); failure context → **500** (`:246`); HITL prompt → **500** (`:281`).

### 3.2 Write path
Six public learn methods, all `async`, **all swallow exceptions**:

| Method | line | Fan-out |
|---|---|---|
| `learn_tool_pattern(tool_name, success, context, session_id)` | 93 | tool_success / tool_failure |
| `learn_retry_pattern(error, succeeded, resolution, session_id)` | 139 | as above with `tool_name="retry_resolution"` |
| `learn_from_correction(...)` | 183 | `store_user_correction` |
| `learn_failure_pattern(error, failure_type, context, session_id)` | 222 | `store_tool_failure` |
| `learn_hitl_pattern(input_type, prompt, user_response, session_id, user_id)` | 255 | `store_tool_success(tool_name=f"hitl_{input_type}", ..., metadata={...})` |
| `store_reasoning(thinking, session_id)` | 425 | `store_tool_success(tool_name="reasoning", ...)` |

**No dedup. No contradiction check. No idempotency key. Every tool call writes a new row.**

### 3.3 Read / recall path
`ProceduralMemory.recall(query, timeout=3.0)` (`:54`) → `_retrieve_agent_learnings` (`context_service.py:1989`):

```python
call_tool("search_memories", {
    "query": query,          # the caller's primary/expanded query
    "user_id": "system",
    "agent_id": agent_id,
    "limit": limit           # 5 from config
    # REMOVED: "filters": {"type": "procedural"} - mem0 doesn't support this!
})
```
**top-k = 5**, no filter, no post-filter, raw mem0 cosine order.

**Three auxiliary readers DO post-filter** — the only places subtype filtering happens:
- `check_tool_history(tool_name, arguments)` (`:297`): query `f"tool {tool_name} failed error arguments {str(arguments)[:100]}"`, **limit 3**, keep `subtype ∈ {"tool_failure","user_correction"}`.
- `find_similar_failures(error, tool=None)` (`:336`): query `f"{tool or 'task'} failed error {error}"`, **limit 5**, keep `subtype ∈ {"tool_failure","user_correction","retry_resolution"}`.
- `find_similar_hitl_interactions(input_type, context, user_id=None)` (`:376`): **limit 5**, keep `subtype == "hitl_interaction"`.

### 3.4 Caching
- Hot key: `f"procedural:{agent_id}"` (`:354`) — **no query, no session**. Size **50**, TTL **3600 s**.
- **Unlike the other two tiers, the procedural warm path uses bare `await self._redis.get(...)` / `setex` with NO circuit breaker and NO timeout** (`memory_core.py:1137`, `:1179`).
- **The procedural cold fetch is also unprotected**: `learnings = await self.procedural.recall(query)` at `:1160` — no breaker call, unlike episodic (`:905`) and semantic (`:1064`). **The `_procedural_breaker` constructed at `:261` is never used.**
- Negative TTL: 60 s.

---

## 4. `memory_core.py` — the orchestrator

### 4.1 Data contracts
```python
@dataclass
class AgentConsciousness:      # memory_core.py:105
    experiences: List[Dict[str, Any]]   # episodic
    facts:       List[Dict[str, Any]]   # semantic
    learnings:   List[Dict[str, Any]]   # procedural
    query:       str

@dataclass
class Interaction:             # memory_core.py:119
    user_message: str
    agent_response: str
    user_id: str
    session_id: str
    agent_id / task_id / thinking / tool_calls / image_urls / file_urls / file_types / metadata  # all Optional
```

### 4.2 Public API

| Signature | Line | Behavior |
|---|---|---|
| `__init__(context_service, agent_id, …12 optional overrides…, redis_client=None)` | 162 | Loads YAML config; constructs 3 tiers, 3 LRU caches, 4 breakers, negative cache, `CompressionEngine`, `StalenessTracker`, `QueryExpander`, `HybridSearch`, `MemoryMetricsCollector`, `EventJournal`, `ContradictionDetector`, `ContinuityPacketBuilder`, `MemoryQueryPlanner`, `PredictiveStager`, `MemoryLogger` |
| `async recall(user_id, session_id, query, use_cache=True, response_builder=None) -> AgentConsciousness` | 1185 | Raw 3-tier parallel recall + continuity merge (§4.3) |
| `async recall_with_budget(…) -> Tuple[AgentConsciousness, Optional[str]]` | 1450 | **The production entry point** (§4.4) |
| `update_consciousness_with_tool_result(...)` | 1852 | **Sync.** Appends an ephemeral learning `{metadata:{…, ephemeral:True}}`. Not persisted |
| `async remember(interaction, response_builder=None)` | 1915 | 4-step write (§4.5) |
| `async clear_cache(cache_type=None)` | 2134 | `"hot"`/`"warm"`/`"all"` |
| `async get_cache_stats()` | 2202 | |
| `async shutdown()` | 2255 | Harvests ≤10 episodic keys × 5, ≤5 semantic × 10, ≤5 procedural × 5; `predictive_stager.predict(top_k=3)`; builds a `ContinuityPacket`; `store_to_redis(ttl_seconds=604800)` (7 days) |
| `learn_from_{tool,retry,correction,failure,hitl_interaction}` | 2506-2611 | Delegate to `procedural.*` |

Module-level helpers: `_sanitize_memory_content(content)` (68) strips `^\[MEMORY_TYPE:\s*[^\]]+\]\s*\n*` (case-insensitive); `_sanitize_memories(list)` (89) mutates in place.

### 4.3 `recall()` pipeline order
1. `MemoryRecallFSM(query_id=f"recall_{session_id}_{ms}")`, `start(query)` → emits `MEMORY_RECALL_STARTED`.
2. **Continuity seed** (`:1243-1271`): if `use_cache and redis`, `packet_builder.load_from_redis(agent_id, redis)`. **It is a seed, not an early return** — the packet is agent-level and may be stale.
3. `recall_fsm.check_hot_cache(cache_hit=False)`, `check_warm_cache(cache_hit=False)` — **always hardcoded `False`**; telemetry stubs.
4. **Parallel fan-out** (`:1292-1297`): `asyncio.gather(_get_cached_experiences, _get_cached_facts, _get_cached_learnings, return_exceptions=True)`. Per-branch exceptions become empty lists; if *all three* fail → `recall_fsm.fail()` and return an empty `AgentConsciousness`.
5. `_sanitize_memories` on all three.
6. `_apply_compression_to_memories(experiences)` — **episodic only**.
7. **Continuity merge** via `_merge_with_dedup(primary=fresh, secondary=packet)` (`:651`), content-hash dedup, fresh wins.
8. `_log_memory_temperatures` × 3, sampling the **first 5** of each list.
9. `processing_complete` → `MEMORY_RECALL_COMPLETED`.

**Per-tier cache ladder** (identical in all three `_get_cached_*`):
`HOT (in-proc LRU) → WARM (Redis) → NEGATIVE → COLD (MCP)`.
The negative-cache probe is deliberately **deferred until after HOT and WARM** so a stale negative cannot shadow fresh data (`:716-720`, `:881-891`). On warm hit → promote to hot. On cold: empty → write negative marker; non-empty → `_clean_cache` (TTL sweep) → `_evict_oldest` (LRU `popitem(last=False)`) → hot store → warm `SETEX`.

### 4.4 `recall_with_budget()` — **the formula the port must reproduce**

**Step 0 — Query expansion.** `query_expander.expand_query(query, max_expansions=3)`. `primary_query = queries[0]`. Decay weights (`query_expander.py:29-43`): `base 0.85`, `entity 0.90`, `temporal 0.80`, `synonym 0.85`, `decomposed 0.82`, `original_boost 1.0`.

**Step 1 — Mode routing** (`:1514-1568`):
- `"timeline"` → `episodic.recall_timeline(limit=10, offset=0, asc=True)`. **Bypasses the cache ladder entirely.**
- `"verbatim"` → `episodic.recall_exact(phrase=verbatim_phrase or query)`.
- `"hybrid"` (default) → `self.recall(...)`.

**Step 2 — BM25 hybrid re-rank** (`:1571-1586`, **episodic only**):
```python
hybrid_search(query=primary_query, semantic_results=experiences,
              top_k=len(experiences), bm25_weight=0.3, semantic_weight=0.7)
```
Fusion is **Reciprocal Rank Fusion, NOT a linear blend** (`bm25_search.py:255-268`), with `k = 60`:
```
RRF(d) = 0.3 / (60 + rank_bm25(d))  +  0.7 / (60 + rank_semantic(d))
```
BM25 candidate depth = `top_k * 2`. Parameters `k1 = 1.5`, `b = 0.75`:
```
score(q,d) = Σ_t IDF(t) · [ tf·(k1+1) ] / [ tf + k1·(1 - b + b·(|d| / avgdl)) ]
```

**Step 3 — Token budget** (`:1594-1597`) — **the only token budget in the system:**
```
budget            = TokenBudget.from_model(model_name)
total             = MODEL_CONFIGS[model].context_window,  default 8192
reserved_response = floor(total * 0.40)
reserved_system   = floor(total * 0.10)
available_context = total - reserved_response - reserved_system      # ≈ 0.50 · total
system_tokens     = len(system_prompt) // 4
query_tokens      = len(query) // 4
max_context_tokens = available_context - system_tokens - query_tokens
```
(`token_validator.py:74-87`). **It is shared — there is no per-tier allotment.** Token estimation elsewhere is `len(text) // 4`.

**Step 4 — Paradigm pre-scoring** (`:1618-1647`). **Skipped for `timeline` and `verbatim`** to preserve order. `MIN_RELEVANCE_THRESHOLD = 0.0` (`paradigm_integration.py:62`) — **i.e. the filter is disabled by default.**

Paradigm score (`relevance_scorer.py:836-841`, `:929-944`):
```
query_score(m, qi)  = match_weight(type) · match_score · context_score · decay^level
final_score(m)      = max_i( query_score(m, qi) )
```
`match_weight` (`:80-88`): `exact 1.00, phrase 0.85, semantic 0.75, broad 0.60, fuzzy 0.50, partial 0.35, none 0.10`.

`context_score` (`:1035-1067`), weights mirroring `memory_config.yaml:390-400`:
```
raw = 0.18·recency + 0.12·authority + 0.15·priority_class + 0.12·session_anchor
    + 0.10·correction_boost + 0.08·procedural_fit + 0.08·completeness + 0.07·specificity
    - 0.05·length_penalty - 0.05·staleness_penalty

max_score = 0.90 ; min_score = -0.10
context_score = clamp01( (raw + 0.10) / 1.00 )
```
Score bands: `≥0.90 CRITICAL`, `≥0.70 HIGH`, `≥0.50 MEDIUM`, `≥0.30 CONTEXT`, else `LOW`. `min_inclusion_score = 0.20`, `min_context_guarantee = 3`.

**Step 5 — Must-include set** (`:1650-1657`): any chunk whose `metadata.tags` contains `"priority:critical"` or `"safety_rule"`.

**Step 6 — Session anchors** (`:1660-1663`): first **5** experiences whose `metadata.session_id == session_id`.

**Step 7 — Two-stage selection** (`:1678-1719`), skipped for timeline/verbatim.

`create_scorer_for_query(...)` auto-detects an archetype from `memory_config.yaml:180-213` (**first match wins**) and loads the matching weight preset.

**Stage 1 — absolute utility** (`context_scorer.py:687-697`), weights normalized to sum 1.0 first:
```
U_abs(c) = clamp01(
      w_rel · relevance(c,q)  + w_auth · authority(c)   + w_rec · recency(c)
    + w_type · type_bonus(c)  + w_prio · priority_class(c)
    + w_corr · correction_boost(c) + w_sess · session_anchor(c)
    + w_proc · procedural_fit(c,q) - w_len · length_penalty(c) )
```

DEFAULT weights (`context_scorer.py:134-142` == `memory_config.yaml:152-161`):
`relevance 0.25, authority 0.15, recency 0.12, type_bonus 0.08, priority_class 0.10, correction_boost 0.08, session_anchor 0.10, length_penalty 0.04, procedural_fit 0.08` (sum = 1.00).

**Archetype presets** (`context_scorer.py:182-220`), order `[rel, auth, rec, type, prio, corr, sess, len, proc | redund, gap, contra]`:

| Archetype | rel | auth | rec | type | prio | corr | sess | len | proc | redund | gap | contra |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| TOOL_OPERATION | 0.20 | 0.20 | 0.10 | 0.05 | 0.10 | 0.05 | 0.08 | 0.02 | 0.20 | 0.35 | 0.20 | 0.25 |
| DEBUGGING | 0.25 | 0.15 | 0.15 | 0.05 | 0.08 | 0.10 | 0.12 | 0.02 | 0.18 | 0.30 | 0.25 | 0.20 |
| QA_RETRIEVAL | 0.30 | 0.20 | 0.10 | 0.08 | 0.08 | 0.06 | 0.06 | 0.04 | 0.08 | 0.25 | 0.30 | 0.15 |
| CONVERSATION | 0.20 | 0.10 | 0.18 | 0.08 | 0.10 | 0.08 | 0.16 | 0.02 | 0.08 | 0.35 | 0.20 | 0.15 |
| TEACHING | 0.25 | 0.18 | 0.08 | 0.10 | 0.08 | 0.08 | 0.08 | 0.05 | 0.10 | 0.20 | 0.35 | 0.15 |
| PLANNING | 0.22 | 0.15 | 0.12 | 0.08 | 0.10 | 0.08 | 0.10 | 0.03 | 0.12 | 0.25 | 0.30 | 0.20 |
| DEFAULT | 0.25 | 0.15 | 0.12 | 0.08 | 0.10 | 0.08 | 0.10 | 0.04 | 0.08 | 0.30 | 0.25 | 0.20 |

**Factor functions** (all `→ [0,1]`):
- `relevance` (`:739-755`): `chunk["score"]` if present, else `metadata["similarity"]`, else keyword-overlap `min(1, |Q∩C| / |Q|)`; `0.3` when either keyword set is empty.
- `authority` (`:757-786`): `tool_spec`/`api_doc` → **1.0**; `tool_success`/`outcome:success` → **0.9**; `user_correction`/`has:user_correction` → **0.85**; SEMANTIC → **0.75**; PROCEDURAL → **0.6**; EPISODIC → **0.4**; else 0.5.
- `recency` (`:788-814`): **exponential decay, 14-day half-life**
  ```
  decay_rate = ln(2) / 14.0            ≈ 0.049510
  recency    = exp(-decay_rate · max(0, age_days))
  ```
  `0.5` if no parseable timestamp.
- `type_bonus` (`:816-845`), archetype-conditional: `TOOL_OPERATION {proc 0.9, sem 0.6, epi 0.4}`; `CONVERSATION {epi 0.8, sem 0.7, proc 0.4}`; `QA_RETRIEVAL {sem 0.85, proc 0.6, epi 0.5}`; default `{sem 0.7, epi 0.6, proc 0.6}`.
- `priority_class` (`:847-869`): `priority ≥ 3 → 1.0`, `≥2 → 0.85`, `≥1 → 0.7`; tags `priority:critical`/`pinned` → 1.0; `priority:high` → 0.85; `must_remember:true` → 0.8; else 0.5.
- `correction_boost` (`:871-883`): `subtype=="user_correction"` → 1.0; tag `has:user_correction` → 0.9; literal `"correction"` in `content[:100]` → 0.6; else **0.0**.
- `session_anchor` (`:885-906`): no session ids → 0.3; **different session → 0.0**; in last-N turns at reverse index `i` → `max(0.5, 1.0 - 0.1·i)`; same session not in last-N → 0.4.
- `length_penalty` (`:908-916`): `min(1.0, est_tokens / 600)`.
- `procedural_fit` (`:918-951`): non-procedural → 0.3; query has no tool keyword → 0.4; else `0.5 + 0.3·[subtype ∈ {tool_success, tool_failure, retry_resolution}] + 0.1·[shared tool keyword]`, capped 1.0. Tool keyword set: `{tool, function, api, mcp, call, execute, run, command, script, code, error, fix, debug, retry}`.

**Stage 2 — marginal utility** (`context_scorer.py:701-737`):
```
U_marg(c | S) = max(0,  U_abs(c)
                      - w_redundancy   · redundancy(c, S)
                      + w_gap_fill     · gap_fill(c, F)
                      - w_contradiction· contradiction(c, S) )
```
Defaults `w_redundancy = 0.30`, `w_gap_fill = 0.25`, `w_contradiction = 0.20`. First chunk selected has `U_marg = U_abs`. `redundancy` uses embedding cosine when `use_embedding_similarity = true`, else Jaccard; `redundancy_similarity_threshold = 0.70`.

**Greedy selection loop** (`:441-505`): recompute `U_marg` for all remaining candidates each iteration, sort descending, take the best. **Early stop when `best.marginal_score < marginal_utility_threshold = 0.15`** → `stopped_reason = "threshold"`; stop on `max_tokens` → `"budget"`; else `"exhausted"`.

**Step 8 — Re-partition** (`:1693-1700`): selected chunks split back into `{episodic, semantic, procedural}` by `chunk_type.value`.

> ### ⚠️ THE CENTRAL PORT DECISION
> **The three tiers are budgeted COMPETITIVELY, not proportionally.** They are unified into one pool (`_unify_chunks`, `:546`) and ranked against one another by the single formula above. **A tier's effective share is emergent from `type_bonus`, `authority`, and `session_anchor`.**
>
> **Any Rust port that assigns fixed per-tier token quotas is *changing* the behavior.** This is an explicit ADR, not an implementation detail.

### 4.5 `remember()` write pipeline (`:1915-2132`) — order matters
1. `MemoryStoreFSM(...)`, `start(content, user_id)` → `MEMORY_STORE_STARTED`.
2. **Contradiction check** (`:1978-2015`) against up to `5 cache keys × 20 facts = 100` cached semantic facts. **Log-only.**
3. **Event journal** (`:2018-2042`): append-only WAL with a content hash (BLAKE3 per the init banner).
4. `store_fsm.validate()`.
5. **Hot cache write-through, SYNCHRONOUS** (`:2058` → `:422`). Prepend fused turn, cap 50, refresh timestamp, clear the negative marker, best-effort warm `SETEX` at 0.1 s.
6. `store_fsm.store(use_cache=True)`, `.cache()`.
7. **Background MCP persist** (`:2065`): `asyncio.create_task(...)` → `gather(episodic.store, semantic.extract_and_store, return_exceptions=True)`.
8. `store_fsm.persist()`; metrics.

> **Note:** `remember()` fires `log_store_completed(cached=True, persisted=True)` at `:2098-2104` **before** the background persist has finished — `persisted=True` is **optimistic, not observed**. `ProceduralMemory` is not part of `remember()` at all.

### 4.6 Concurrency
- All caches are plain `OrderedDict`/`dict` with **no locks**. Correct only under a single-threaded event loop; **a Rust port must decide explicitly** (e.g. `DashMap` or an actor).
- `PredictiveStager(enable_background_worker=True, predictions_per_second=2.0)` runs a background worker.
- Metrics alert thresholds: `recall_latency_ms 1000.0`, `failure_rate 0.05`, `degradation_rate 0.10`, `cache_hit_rate 0.50`.

---

## 5. Config, dead keys, and live defects

### 5.1 `get_memory_config(reload=False)` (`:434`)
A **process-global singleton**. Path resolves to `config/memory_config.yaml`. **Missing file raises `FileNotFoundError`** — no built-in fallback.

Key values are tabulated in `G4b §C.5`. The additional live/dead notes below matter for the port.

### 5.2 Keys present in YAML that the loader NEVER parses
`_parse_config` (`:304-425`) handles only: `cache`, `retrieval`, `timeouts`, `intent_detection`, `sanitization`, `continuity_packet`, `circuit_breaker`, `compression`, `optimization`, `logging`, `retrieval.context_scoring`. **Silently ignored:**
- `paradigm_scoring:` (`:375-439`) — `match_weights`, `context_weights`, `query_expansion`, `score_bands`, `min_inclusion_score: 0.20`, `min_context_guarantee: 3`, `aggregation: "max"`, 11 `formatting` keys.
- `cache_population:` (`:442-473`) — thresholds, capacity, decay, prewarm, eviction.

**These values are duplicated as hardcoded dataclass defaults** in `relevance_scorer.py:80-145` and `cache_populator.py`. They currently agree numerically, so **the drift is latent**. → **In the Rust port, either wire these blocks into the loader or delete them from the config — do not silently re-create the fork.**

### 5.3 LIVE DEFECT: the negative cache never fires
All three tiers gate the negative-cache return on `if not strict_lossless:` (`:886-891`, `:1055-1059`, `:1152-1156`). The config sets `strict_lossless: true` (`yaml:38`) and the dataclass default is also `True`. **Therefore negative markers are written on every empty result but never honored, and `ttl_by_type` is unreachable.** The 60/15/30 s TTLs are inert. **Decide deliberately in Rust: keep it inert, or invert the gate.**

### 5.4 Other code/doc discrepancies to carry into the port
1. **`SemanticMemory.recall` drops `query`** — §2.3. Docstring and code disagree; **the code wins.**
2. **`_get_episodic_key` docstring lies** (`:344-346`): comment says *"Query hash to distinguish different queries"* but the key is `f"episodic:{user_id}:{session_id}"` with `query` unused. **Two different questions in the same session share one cache entry.**
3. **`learn_hitl_pattern` ALWAYS throws.** `procedural.py:278-290` calls `store_tool_success(..., metadata={...})`, but `context_service.store_tool_success` (`:1204-1211`) **has no `metadata` parameter** → `TypeError`, swallowed at `:294-295`. Consequently **no record with `subtype == "hitl_interaction"` is ever written**, and `find_similar_hitl_interactions` can only ever return `[]`.
4. **Procedural tier has no filter on its primary read** — §3.3. Cross-tier bleed is by construction.
5. **Episodic tier never forwards `agent_id`.** The `context_service` functions accept an `agent_id` that adds `filters["metadata.agent_id"]` (the "E-03 defense-in-depth" comments), but `EpisodicMemory.recall` (`:106-124`) and `recall_timeline` (`:449-457`) **do not pass it**. **The memory_core path is therefore unscoped by agent.**
6. **`_procedural_breaker` is constructed but never used** (`:261` vs `:1160`); procedural warm-tier Redis calls bypass `_redis_breaker`.
7. **Init banner logs the raw constructor args, not resolved values** (`:323-326`) — when built from config the log reads `None entries, None TTL`.
8. **`get_exact_match` hardcodes `limit: 50`** while `retrieval.limits.verbatim` is 20.
9. **`TokenValidator` is dead code.** Only `TokenBudget` is imported (`:25`); nothing instantiates `TokenValidator` (`token_validator.py:101`). **Its degradation levels and the `token_policy` trimming/compaction are unreachable** — which means `trim_order: [episodic, semantic, procedural]` in the YAML is decorative.
10. **`build_agent_context` uses different limits than `MemoryCore`** (`context_service.py:1486-1527`): recent=3, relevant=5, facts=5, learnings=**2**. **A second, parallel recall path with its own hardcoded numbers.**

### 5.5 Numbers not otherwise stated
- **Staleness relevance** (`staleness_tracker.py:244-255`):
  ```
  half_life_days = 30.0
  recency_score  = exp(-(ln 2 / 30.0) · age_days)
  relevance      = clamp01( 0.7 · recency_score + 0.3 · frequency_score )
  ```
  Access history retained **90 days**; "refresh to fresh" if **≥3 accesses in the last 30 days**.
- **Continuity packet:** target **≤ 2048 bytes**; beliefs truncated to **100 chars** each, capped at **5**; Redis TTL **604800 s**.
- **Ordinal recall fetch depths:** `limit=20` for negative ordinals, `limit=max(ordinal+2, 10)` for positive.
- **Shutdown harvest caps:** 10 episodic keys × 5, 5 semantic keys × 10, 5 procedural keys × 5.
- **Contradiction check corpus cap:** 5 semantic cache keys × 20 facts.

---

## 6. Honest assessment for the port

### What is genuinely good and should be ported
- **The three-tier separation itself** (what happened / what I know / what I learned) — a clean, defensible model.
- **The competitive two-stage selection** (absolute utility → marginal utility with redundancy/gap/contradiction terms). This is the sophisticated part and it directly attacks BUG-TOKENS.
- **RRF fusion of BM25 + vector** at `k=60`, 0.3/0.7 — standard, correct, and better than a linear blend.
- **The 14-day half-life recency decay** and the archetype weight presets.
- **The cache ladder shape** (hot → warm → negative → cold) with the negative probe deliberately deferred.
- **`user_correction` as the highest-authority record class.**
- **Age-based compression of episodic only.**
- **The TTL/limit table** in `memory_config.yaml` — portable as-is.

### What is over-built and should NOT be ported
- **Redis as a warm tier.** A desktop app has one process; an in-memory LRU plus SQLite is the whole story. Drop the second cache tier and its four circuit breakers.
- **mem0 + Qdrant.** A 1536-dim vector DB as a separate service for a single-user desktop app is unjustified. Either BM25-only (matching what actually runs, per `G4b §C.4`) or an embedded `sqlite-vec`/`usearch` index.
- **The `[MEMORY_TYPE: …]` marker protocol** and its defensive strip-on-read — an artifact of routing three tiers through one mem0 prompt. With a real schema, tier is a column.
- **`PredictiveStager` background worker**, `EventJournal` WAL with BLAKE3 hashing, `ContinuityPacket` — infrastructure for a multi-tenant server, not a desktop app.
- **The two dead config blocks** (`paradigm_scoring`, `cache_population`) and every dead key in §5.2.
- **`TokenValidator`** and the `token_policy` trim/compaction machinery (already unreachable, §5.4.9).
- **The second parallel recall path** in `build_agent_context` with its own hardcoded limits (§5.4.10) — one recall path, one set of numbers.

### Defects to fix rather than faithfully reproduce
- The negative cache that never fires (§5.3) — **decide**.
- `SemanticMemory.recall` ignoring the query (§5.4.1) — almost certainly a bug; the docstring states the intent.
- `learn_hitl_pattern` always throwing (§5.4.3) — an entire subtype that has never been written.
- Episodic recall unscoped by agent (§5.4.5) — a cross-agent leakage risk the code has a defense for and does not use.
- Procedural's missing circuit breaker and timeout (§5.4.6).
- The episodic cache key ignoring the query (§5.4.2) — two different questions share one entry.
