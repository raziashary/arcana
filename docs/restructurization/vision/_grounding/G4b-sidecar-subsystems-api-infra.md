# G4b — core-orchestrator: subsystem survey, HTTP API surface, infrastructure dependencies

**Scope:** `submodules/core-orchestrator` — everything *except* the three memory tiers (those are in `G4-livingagent-memory-semantics.md`)
**Purpose:** decide, per subsystem, what must be **PORTED** into Rust and what **DIES** with the sidecar
**Date:** 2026-08-31
**Status:** COMPLETE

> **Path correction:** the `llm/` package is **not** under `src/airs/`. It lives at `src/llm/`. `src/airs/llm/` contains exactly one file, `enrichment_service.py` (755 lines), an MCP-metadata enrichment helper, not an LLM client.

> **The single most important number in this document:** the Rust app consumes **9 endpoints** out of ~129 built. **The port surface is ~7% of the API**, and really only three endpoints matter.

---

## SECTION A — Subsystem survey

### A.1 `src/llm/` — the LLM provider layer — **PORT**

Wraps LiteLLM into `LiteLLMProvider`: completion, streaming completion, iterative tool-calling loops, prompt caching, vision/file input, embeddings.

**Entry points**
- `LiteLLMProvider` — `src/llm/providers/litellm_provider.py:76`
  - `complete(messages, max_tool_iterations=25, ...)` — `:1514`
  - `complete_stream(messages, max_tool_iterations=25, ...)` — `:2053`
  - `get_embeddings(texts, **kwargs)` — `:2737`
  - `register_tool_handler(fn)` — `:2774`; `validate_model()` — `:2854`
  - `_prepare_params()` — `:968`; `_prepare_messages()` — `:897`
- `LLMResponse` `:49`, `LLMStreamChunk` `:62` (both carry `reasoning_content` and `thinking_blocks`)
- `MODEL_CONFIGS: Dict[str, ModelConfig]` — `src/llm/providers/model_registry.py:71`
- `get_model_by_intelligence_rank(rank)` `:889` (1=Flash … 10=Strongest), `get_all_ranked_models()` `:934`

**Providers supported (11)** — resolved in `_get_api_key_and_base()` `:466-590`:

| provider | env var | base URL override |
|---|---|---|
| `openai` | `OPENAI_API_KEY` | none `:499` |
| `anthropic` | `ANTHROPIC_API_KEY` | none `:512` |
| `xai` | `XAI_API_KEY` | none `:525` |
| `azure` | `AZURE_API_KEY` | `AZURE_API_BASE` `:532` |
| `google` | `GOOGLE_API_KEY` | none `:540` |
| `groq` | `GROQ_API_KEY` | none `:547` |
| `nvidia` | `NVIDIA_NIM_API_KEY` | none `:554` |
| `moonshot` | `MOONSHOT_API_KEY` | none `:561` |
| `deepseek` | `DEEPSEEK_API_KEY` | none `:570` |
| `dashscope` (Qwen) | `DASHSCOPE_API_KEY` | `https://dashscope-intl.aliyuncs.com/compatible-mode/v1` `:580` |
| `sealion` | `SEALION_API_KEY_TRIAL` | `https://api.sea-lion.ai/v1` `:487` |

**Keys are passed per-request, never written to `os.environ`** — `_setup_environment_keys()` is a documented no-op (`:592-598`) to avoid cross-provider env pollution. **This is the correct pattern and must survive the port.**

**Per-model defaults** in `ModelConfig` (`schema.py:20`): `timeout=600` s, `temperature=0.7`, `drop_params=True`, `cache_min_tokens=1024`, `cache_breakpoint_locations=["system"]`.

**The three streaming modes** — declared by `ModelConfig.block`, documented at `model_registry.py:1-64`:
1. `"reasoning"` → native `chunk.reasoning_content` (GPT-5, o1/o3/o4, Gemini thinking, deepseek-reasoner)
2. `"thinking"` → native `chunk.thinking_blocks` (Claude extended thinking)
3. `None` → XML fallback: model emits `<thinking>…</thinking>` inline, parsed by the DFA detector in `src/airs/streaming/`

**Tool-call normalization.** Everything is normalized to the **OpenAI function-calling shape** (`{"id","type":"function","function":{"name","arguments"}}`) — built at `:1726-1740`. Two divergences handled explicitly:
- **Anthropic thinking + tools**: `_build_anthropic_assistant_content(...)` (`:659`) rebuilds the assistant turn as structured content blocks and re-attaches `signature` fields (`_safe_add_signature`, `:645`), because Anthropic rejects thinking blocks without signatures when tools are in play.
- **Prompt caching**: `_add_anthropic_cache_markers()` (`:822`), gated by `_should_enable_caching()` (`:733`).

Tool results are stringified per provider by `_format_tool_result_for_llm()` (`:1248`).

**External deps:** `litellm==1.81.16` (the only LLM SDK — no direct `openai`/`anthropic` imports anywhere), `httpx[http2]`, `tenacity`.

**PORT verdict:** the Rust replacement must reproduce: the model registry table (names/providers/context windows/`block` mode/fallback chains), per-request key injection with no env mutation, the ≤25-iteration tool loop, OpenAI-shape tool-call normalization, and the Anthropic thinking-block/signature special case. **The Rust side already consumes `GET /api/llm/models` purely to translate registry keys to canonical LiteLLM names (`src-tauri/src/sidecar/client.rs:1140`), so the registry table is the concrete artifact to port first.**

> ⚠️ **Model IDs in this registry are stale relative to the restructure.** They list `claude-sonnet-4-5`, `gemini-3-pro`, `grok-3-mini-beta`, `gpt-5` etc. Re-ground every model ID against `G6-agents-sdk-research.md` before porting the table. The *structure* ports; the *contents* must be refreshed.

### A.2 `src/airs/execution/` — **PORT** (executor + registry), **DELETE** (ThoughtProcessor's injection half)

Executes tools discovered through the MCP bridge while recording every execution into procedural memory; maintains a process-global registry of in-flight tool calls.

Entry points (`__init__.py:17-32`): `MemoryAwareToolExecutor` (`tool_executor.py:232`) with `execute()` `:255`, `execute_multiple()` `:535`; `ToolExecutionRegistry` `:50` (singleton via `__new__` `:64`) with `register/update_state/unregister/get_active_by_session/get_active_by_agent/get_metrics`; `ThoughtProcessor` (`thought_processor.py:18`).

`ThoughtProcessor.inject_instructions()` exists to coax non-native models into emitting `<thinking>` XML — only needed if the Rust port keeps XML-mode models. With the provider list narrowed to four (all of which have native reasoning/thinking), **it can be dropped.**

### A.3 `src/airs/mcp/` — **DELETE**

Redis-backed stores for AI-generated "agent cards" and dynamic prompts, background regeneration services, a config deleter, a refresh coordinator.

Entry points: `DynamicAgentCardStore` (`agent_card_store.py:29`, keys `agent_cards:{agent_id}`, **no TTL**), `DynamicPromptStore`, `get_card_regeneration_service()`, `get_prompt_hot_reload_service()`, `get_refresh_coordinator()`, `MCPDefaults` (`defaults.py:22`: `timeout=30.0`, `max_retries=3`, cache `ttl_seconds=300`, `max_items=100`).

**DELETE verdict:** the Rust app already runs its own in-process MCP server (`src-tauri/src/mcp_server/`, port 8767 with fallback) and its own health monitor. This package's value is Redis-backed prompt/card generation for a multi-tenant server, which a single-user desktop app does not need. The one salvageable idea — refresh debounce (`MCP_REFRESH_DEBOUNCE=60`, `MCP_REFRESH_CONCURRENCY=3`) — is a handful of constants, not a port.

### A.4 `src/airs/session/` — **PORT**

"Multiverse" architecture: each `(agent_id, session_id)` pair gets an isolated `AgentRuntime` while sharing universal memory. Lazily created, idle-reaped.

Entry points: `AgentRuntime` (`runtime_manager.py:27`, `touch()` `:48`, `is_idle(timeout_seconds=600.0)` `:52`, `cleanup()` `:56`); `SessionRuntimeManager` `:210` with `get_runtime()` `:307`, `cleanup_session()` `:493`, `cleanup_idle_runtimes()` `:543`, `invalidate_agent()` `:635`, `get_stats()` `:686`.

**Numbers:** `MAX_RUNTIMES = 500` hard circuit-breaker (`:225`); `idle_timeout_seconds=600.0` (10 min); `cleanup_interval_seconds=60.0`.

**PORT verdict:** a desktop app runs many concurrent worker sessions. The isolation model, idle reaping, and runtime cap map cleanly onto a Rust `HashMap<(AgentId, SessionId), Runtime>` behind a `RwLock` with a reaper task.

### A.5 `src/airs/state/` — **PORT selectively (~5 of 20)**

20 hand-written FSMs, each emitting events on every transition and rejecting invalid transitions. All exported from `state/__init__.py:43-62`.

**PORT:** `ToolExecutionFSM`, `SessionLifecycleFSM`, `AgentLifecycleFSM`, `TaskExecutionFSM`, `SystemStartupFSM`/`SystemShutdownFSM` — these guard real invariants the desktop app has.
**DELETE:** `A2AServerFSM`, `A2ARequestFSM`, `AgentSubprocessFSM`, `MCPServerFSM`, `CacheTierFSM`, `PromptReloadFSM`, `AuthenticationFSM` — server-side concerns that die with the sidecar.

> Rust's type system does this better: enum + `impl` transition methods, illegal states unrepresentable. Port the *invariants*, not the Python class hierarchy.

### A.6 `src/airs/streaming/` — **PORT** (genuinely good engineering)

"STREAM V2" — a zero-copy, CPU-light pipeline turning raw LLM chunks into self-describing thinking/content boundary signals.

Entry points (`__init__.py:23-36`): `StreamBroker` (`stream_broker.py:51`) — `process_chunk()` `:165`, `_process_native_chunk()` `:194`, `_process_xml_chunk()` `:313`, `finalize()` `:517`, `get_stats()` `:637`; `NativeThinkingDetector(blocks_mode)` (`native_detector.py:31`); `XMLThinkingDetector(ring_capacity=4096)` (`xml_detector.py:158`) with `ByteRing` `:96` and KMP failure tables `:66`; `BufferManager`/`TokenBuffer`; `StreamState`/`StreamPhase`.

**Numbers:** ring buffer **4096 bytes**; design target **TTFS < 250 ms**; linear O(n) KMP scan, no string accumulation. **No external deps** — pure Python/bytes.

**PORT verdict:** dependency-free engineering that Rust will run strictly better (`bytes::BytesMut` ring, `memchr` scanning). **The three-mode contract is the spec: keep it verbatim.**

### A.7 `src/airs/prompts/` + `config/extended_prompts.yaml` — **PORT the YAML, DELETE the Python**

`PromptTemplate` (`template.py:11`) is 79 lines of `str.replace` with regex `r'\{(\w+)\}'`. `AgentListProvider` (`agent_list_provider.py:18`) is a cached, event-invalidated provider of the "Available Agents" list.

**The genuine asset is `config/extended_prompts.yaml`** — 13 tag-gated templates:

| name | position | priority | tags (gate) | line |
|---|---|---|---|---|
| `agent_identity` | prepend | 0 | all | `:14` |
| `minimal_environmental_context` | prepend | 1 | all | `:32` |
| `base_guidelines` | prepend | 1 | all | `:48` |
| `safety_guidelines` | prepend | 2 | all | `:79` |
| `memory_awareness` | prepend | 3 | `memory_core` | `:120` |
| `agent_to_agent_communication` | append | 4 | `a2a_communication` | `:137` |
| `system_api_documentation` | append | 6 | `system_access` | `:291` |
| `orchestration_guidelines` | append | 7 | `orchestration` | `:233` |
| `system_monitoring` | append | 7 | `orchestration` | `:260` |
| `human_interaction_guidelines` | append | 8 | `human_input` | `:356` |
| `tool_usage_guidelines` | append | 9 | `mcp_tools` | `:331` |
| `adobe_rules` | append | 9 | `tools:mcp-adobe` | `:390` |
| `output_format` | append | 10 | all | `:101` |

Global config `:490-499`: `enabled: true`, `log_prompt_application: true`, `tag_based_inclusion: true`.

**Load-bearing fragments to port:**
- **Delegation contract**, `:154-158` — the syntax is literally `DELEGATE[@agent-id]: Your task instructions here.` with `**Available Agents:** {agent_list}` at `:145` as the only authoritative roster.
- **Roster staleness guard**, `:196` — *"the ONLY authoritative source of who you can collaborate with RIGHT NOW"* … *"Do NOT mention or attempt to delegate to anyone not in the current list, even if they appear in conversation history."*
- **Tool honesty**, `:339-341` — *"Only reference actions you truly performed."* / *"Don't narrate tool execution; focus on results."*

**Drop `adobe_rules`** (creative-suite specific; `autodev_mcp_config.yaml` already excludes the Adobe MCP).

### A.8 `src/airs/a2a/` — **DELETE**

Google A2A protocol: a per-agent HTTP server, a port allocator (`PortManager(start_port=5001, end_port=5099)`, persisting to `.port_allocations.json`), a client manager (`ClientConfig` defaults `timeout=60.0`, `max_retries=5`), a streaming adapter. Deps: `a2a-sdk>=0.3.5`, `httpx`, **~99 localhost TCP ports**.

**DELETE verdict:** A2A means spawning one HTTP server per agent on ports 5001–5099 to let two Python objects in the same process talk. **In a Rust desktop app that is an `mpsc::channel` or a direct function call.** The parent repo has no A2A client code at all. Keep only the *semantics* of `AgentBridge.MAX_CALL_DEPTH = 3` and the circular-dependency call graph (`bridges/agent_bridge.py:47,51`).

### A.9 `src/airs/intelligence/` — **PORT the decision engine, DELETE the failure detector**

Entry points (`__init__.py:16-31`): `UnifiedDecisionEngine`, `DecisionContext`, `ActionPlan`, `NextAction`, `RetryDecision` (`decision_engine.py`); `MemoryAwareFailureDetector`, `TaskFailureResult`, `FailureType` (`failure_detector.py`); `MemoryAwareHITLOrchestrator` (`hitl_orchestrator.py`).

Module header notes retry was deliberately moved here out of `execution/` because *"retry is a DECISION, not an EXECUTION"* (`execution/__init__.py:13-14`) — **that framing is worth carrying over.**

`failure_detector.py` is ~500 lines of regex error classification tuned for Adobe Firefly image errors (`r"(?i)invalid\s+(size|dimensions|format)"` `:87`, image-dimension extraction `:512`) — **irrelevant to a coding fleet. DELETE.**

### A.10 `src/airs/lifecycle/` — **DELETE**

`StartupOrchestrator` (`startup_orchestrator.py:106`) with topological sort (`:289`), `execute()` `:345`, `_rollback_completed_steps()` `:545`; `ShutdownCoordinator` (`shutdown_coordinator.py:86`) with graceful/force paths and signal handling.

**DELETE verdict:** an orchestration harness for a Python process with ~20 async singletons. Tauri already has its own `setup()` lifecycle and `SidecarManager`. Once the sidecar is gone, **the 18-second boot it exists to sequence goes with it** (the Rust side independently observes "core-orchestrator takes ~18s to boot", `sidecar/client.rs:~155`). Keep the *idea* of rollback-on-partial-startup if the Rust side ever grows comparable complexity.

### A.11 `src/airs/events/` — **PORT the bus, DELETE the schema breadth**

Typed pub/sub event bus (v2) with ~25 Pydantic event families, per-event state machines, a dead-letter queue, ring-buffer replay, correlation-ID propagation, pluggable transports.

Entry points: `EventBus`/`get_event_bus()` (`unified_bus.py`, 756 lines); `EventType`, `EventCategory`, `BaseEvent` and family models (`schema.py`, **1986 lines**); `StateMachine`/`Transition`; `EventStore`; transports: `InternalTransport` (`transports.py:66`), `SSETransport` `:81`, `WebSocketTransport` `:288` (**stub**), `RedisPubSubTransport` `:304` (**stub — "future implementation"**).

**Numbers:** `EVENT_HANDLER_TIMEOUT=5.0`s, `EVENT_HISTORY_SIZE=10000` ring buffer, `EVENT_DLQ_SIZE=1000`.

**PORT verdict:** a typed internal event bus with correlation IDs is the right backbone and maps to `tokio::sync::broadcast` + a `#[derive(Serialize)] enum Event`. But 1986 lines of schema covering `MCPWizardAuthEvent`, `CollaborationEvent`, `AnalyticsEvent` is server-product surface. **Port the ~8 families the desktop app actually renders** (agent, tool, stream, session, task, system, file, memory). Drop the WebSocket and Redis transport stubs entirely. Note the Redis transport being a stub means **no runtime Redis dependency here.**

### A.12 `src/airs/bridges/` — **SPLIT**

- **`MCPBridge`** (`mcp_bridge.py:40`, 815 lines) — builds `ToolSpec` from MCP tool schemas, drives `MCPServerFSM`/`MCPToolDiscoveryFSM`. **This is the tool-calling seam. PORT.**
- **`AgentBridge`** (`agent_bridge.py:37`, 1076 lines) — parses `DELEGATE[@id]:` out of LLM output and dispatches over A2A. `MAX_CALL_DEPTH = 3` `:47`; `_call_graph` cycle detection `:51`; `A2A_TIMEOUT=300.0`s `:32`; `MAX_AGENT_RESPONSE_LENGTH=8000` `:36`. **PORT the parser and the depth/cycle guards; reimplement dispatch in-process.**
- **`UnifiedToolHandler`** (201 lines) — **PORT.**
- **`AdobeCompiler`** (`adobe_compiler.py`) — **2874 lines, the single largest file in the repo. DELETE** (the AutoDev MCP config explicitly excludes the Adobe server).
- **`MemoryCompiler`** (418 lines) — **DELETE** (superseded by the Rust-owned `memory.db`).

### A.13 `src/airs/cache/` — **DELETE**

`UnifiedCacheManager` (`unified_cache_manager.py:101`) over five caches: `MEMORY_HOT`, `REDIS_WARM`, `MCP_PROMPTS`, `MCP_RESOURCES`, `AGENT_CARDS`. Redis key namespaces cleared: `session:history:*`, `continuity:*`, `mcp:prompt:*`, `agent:card:*`.

**DELETE verdict:** four of five managed caches are Redis namespaces that die with the sidecar; the fifth is `MemoryCore`'s in-process LRU, which the Rust port will own directly. **The reusable artifact is the TTL table in `config/memory_config.yaml` (see C.5), not this coordinator.**

### A.14 `src/airs/security/` — **PORT redaction, REPLACE SecretStore**

- `redact_dict(...)` (`redaction.py:22`) with a 20-entry `SENSITIVE_KEYS` set (`:9-20`). **126 lines, directly reusable — the `SENSITIVE_KEYS` list is the spec. PORT.**
- `SecretStore` (`secret_store.py:18`) — Fernet-encrypted blobs in Redis, **hard-fails at construction** without both `MCP_VAULT_KEY` and `REDIS_URL`. **Its design — encrypted blobs in Redis with the key sitting in plaintext `.env` — must not be carried over.** The Rust replacement uses the OS keychain (`keyring` crate), removing both the Redis and the `MCP_VAULT_KEY` dependencies at once. `src-tauri/src/vault/keychain.rs` already does this correctly.

### A.15 `src/airs/llm/enrichment_service.py` — **DELETE**

755 lines calling `gpt-4o-mini` to generate agent-card "powers", extended-prompt hints, and job descriptions from MCP tool lists. Pure server-product UX (the "MCP wizard"). **No Rust caller.**

---

## SECTION B — HTTP API surface

Server: FastAPI, `uvicorn.run(app, host="0.0.0.0", port=4000)` (`api_gateway/app.py:741`). Routers auto-discovered from `api_gateway/modules/*/router.py` via `ROUTER_META`.

> ⚠️ **`openapi.json` / `openapi-pretty.json` are STALE.** They contain 113 paths but omit the entire `/api/memory/*`, `/api/internal-tools/*`, `/api/auth/service-login`, and several `/api/sessions/fsm-*` routes that exist in code. The enumeration below is from the routers, which are authoritative.

**Auth.** `AuthMiddleware` (`api/middleware/auth.py:44-58`) protects all `/api/*` except `OPTIONS`, `/health`, `/api/system/health`, `/api/auth/*`, `/api/client/logs`, `/api/client-logs`, `/api/debug/*`. **`DEBUG_AUTH_BYPASS=True` in `.env:40`.**

### B.1 SSE frame vocabulary (emitted by `api_gateway/core/stream_manager.py`)
`connected` `:313`, `heartbeat` `:387`, `agent_start` `:616`, `agent_stop` `:708`, `agent_pause` `:789`, `content_start` `:844`, `content_end` `:897`, `thinking_start` `:960`, `thinking_end` `:1028`, `token` `:1092`, `tool_call_start` `:1183`, `tool_call_complete` `:1248`, `a2a_delegation_start` `:1320`, `a2a_delegation_complete` `:1354`, `end` `:89`, `error` `:303`.

### B.2 **WHAT THE RUST APP ACTUALLY CONSUMES (9 endpoints)**

The **only** consumer is `src-tauri/src/sidecar/client.rs` (1656 lines). Constants: `BASE_URL = "http://127.0.0.1:4000"` `:93`, `MEMORY_URL = "http://127.0.0.1:8000"` `:94`, **`API_KEY = "dev-key-123"` `:95`** (hardcoded), `HEALTH_PATH = "/health"` `:108`, `HEALTH_TIMEOUT_SECS = 10`, `INVOKE_TIMEOUT_SECS = 600`, `HEALTH_FAIL_THRESHOLD = 6`.

**The TypeScript frontend never calls the sidecar.** Every `/api/core` and `/api/event` hit in `src/**/*.tsx` is a `@tauri-apps/api/core` import, not an HTTP path. **All sidecar traffic is Rust → 127.0.0.1:4000.**

| Method | Path | Rust call site | Purpose |
|---|---|---|---|
| GET | `/health` | `client.rs:390`, `:429` | liveness; 6-failure debounce → Mode 1 fallback |
| POST | `/api/auth/service-login` | `client.rs:215` | API key → JWT, cached in `jwt_token` |
| GET | `/api/agents` | `client.rs:302, 352, 682, 1202` | slug → agent_id lookup, model drift detection |
| POST | `/api/agents` | `client.rs:764` | register worker agent |
| PUT | `/api/agents/{id}/config` | `client.rs:649`, `:797` | push `llm_model` change; force-load an existing agent |
| DELETE | `/api/agents/{id}` | `client.rs:~1500` | remove agent |
| POST | `/api/agents/{id}/invoke` | `client.rs:860`, `:1092` | unary worker dispatch |
| POST | `/api/agents/{id}/invoke/stream?session_id=worker-{id}` | `client.rs:1546`, `subagent/bridge.rs`, `events.rs:87` | **SSE**; body `{prompt, file_ids:[], parameters:null}` |
| GET | `/api/llm/models` | `client.rs:1140` | registry key → canonical LiteLLM name map |
| POST | `/api/mcp/servers` | `client.rs:1269` | register the Rust in-process MCP server at `http://127.0.0.1:{mcp_port}/mcp` |

Also probed but not part of the sidecar API: `GET http://127.0.0.1:8000/health` (memory-mcp-server, `client.rs:533`) and a raw RESP `PING` to `127.0.0.1:6379` (`client.rs:565`).

**⚠️ SSE frame-name mismatch (UNVERIFIED which side translates):** the Rust parser (`src-tauri/src/subagent/bridge.rs:354-372`) recognizes `token`|`text`|`delta` → `Token`; `tool_start`|`tool_call` → `ToolStart`; `tool_end`|`tool_result` → `ToolEnd`; `final`|`done`|`complete`|`completed` → `Final`. **The Python side emits `tool_call_start`/`tool_call_complete`** (`stream_manager.py:1183,1248`) — neither of which the Rust matcher lists. Either an adapter exists somewhere or **tool events are silently dropped from worker streams.** Worth confirming during the port; it is a candidate contributor to BUG-ASSIGN's invisibility symptoms.

### B.3 EXISTS BUT UNUSED (~120 endpoints)

Entire router groups with **zero** Rust or TS callers: `/api/memory/*` (14), `/api/system/*` (20), `/api/analytics/*` (7), `/api/collaboration/*` (5), `/api/orchestrate/*` (5), `/api/files/*` (9), `/api/human-input/*` (4), `/api/sessions/*` and `/api/chat/*` (~20), `/api/stream/{session_id}` (browser-only SSE), `/api/internal-tools/*` (4), `/api/mcp/wizard/*` (8), `/api/prompts/extended`, `/api/logs/*`, `/api/debug/memory/*`, `/api/metrics/snapshot`, all of `/api/auth/*` except `service-login`, and all agent endpoints beyond the five listed above.

> **The port surface is ~7% of the built API. Nine endpoints, of which really three matter: `POST /invoke`, `POST /invoke/stream`, `GET /api/llm/models`.**

---

## SECTION C — Storage / infrastructure dependencies

### C.1 Redis / Memurai — port 6379
- Config: `REDIS_URL=redis://127.0.0.1:6379/0` (`.env:68`); fallback `redis://localhost:6379`. Client: `redis>=5.0.0`, always `redis.asyncio`.
- **Data stored:** `session:history:*` (TTL **604800** s = 7 days), `continuity:*`, `mcp:prompt:*`, `agent_cards:{agent_id}` (**no TTL**), `vault:{name}` (Fernet-encrypted secrets). Warm-tier memory cache TTL **7200** s.
- **Required or optional?** Nominally required and hard-failing. `agent/factory.py:154-155` raises `RuntimeError` when `cache.warm.enabled` and Redis is unreachable. `app.py:_ensure_warm_tier_ready()` pings, and **on Windows shells out to `powershell Start-Service -Name Memurai`** `:161-172`, retries 5× at 1 s, then honors `REDIS_STRICT` (default `"false"`). With `REDIS_STRICT` unset it degrades. `SecretStore` is unconditionally hard-fail. The Rust side pings Redis directly at `client.rs:565`.
- **Rust replacement must provide:** a local KV with TTL for session history (7 d), a persistent store for generated agent metadata, and an OS-keychain-backed secret vault. **`redb`/`sled` or plain SQLite tables cover all three; the whole Memurai/Docker-Redis apparatus (and its two setup docs) can be deleted outright.**

### C.2 SQLite — `api_gateway_data.sqlite` (40 KB, repo root)
- Path: `AIRS_DB_PATH=./api_gateway_data.sqlite` (`.env:45`).
- **Schema — exactly one table**, created at `asset_repo.py:46-59`: `assets(asset_id PK, key, filename, content_type, size, status, uploaded_at, last_modified, created_by, metadata, policy)` with two indexes.
- Required only if `ENABLE_FILES=True`. Backs S3 asset bookkeeping.
- **Rust replacement: nothing.** The `/api/files/*` routes have zero Rust callers.

### C.3 SQLite — `~/.autodev/memory.db` — **ALREADY RUST-OWNED**
- Path: `AUTODEV_DIR` env override, else `~/.autodev/memory.db` (`modules/memory/service.py:36-39`). `PRAGMA journal_mode=WAL`, `foreign_keys=ON`, `timeout=10`.
- Tables: `episodic_memories`, `semantic_memories`, `fact_history`, `procedural_memories`.
- **The file states outright:** *"Schema contract mirrors src-tauri/src/memory/db.rs"* (`service.py:9`), *"the same SQLite database created by the Rust memory module"* (`:4-5`).
- **The sidecar is a *client* of the Rust database here.** Relevance scoring is a naive keyword-overlap score (`service.py:57-64`) explicitly mirroring `recall.rs`. **Nothing to port — delete the Python wrapper.**

### C.4 Vector store / embeddings — **effectively absent**
- **No pgvector, Postgres, Qdrant, Chroma, Weaviate, FAISS, or Pinecone is imported anywhere** in `src/`, `api_gateway/`, or `airs_mcp/`. The only hits are a stale `CLEAR_QDRANT_ON_STARTUP` description string, a `'postgres'` docstring example, and TODOs.
- The **only** embedding path is `LiteLLMProvider.get_embeddings()` (`litellm_provider.py:2737`), defaulting to `"text-embedding-ada-002"` `:2751`. **No dimension is configured anywhere and no vector index consumes the output** — retrieval is BM25 (`memory/bm25_search.py`) plus hand-rolled scoring.
- **Rust replacement:** if semantic recall is wanted, **it must be built new** (e.g. `fastembed-rs` + `usearch`/`sqlite-vec`). **There is no existing index to migrate.** Porting BM25 + the scoring heuristics is the lower-risk path and matches what actually runs today.

> ⚠️ Note the contradiction with the memory-mcp-server config, which *does* declare Qdrant at `collection_name: "mem0"`, `embedding_model_dims: 1536`, `text-embedding-3-small` (see `G4-livingagent-memory-semantics.md` §0). **The two submodules disagree about whether a vector store exists.** Resolve by ADR.

### C.5 Memory tier TTLs — `config/memory_config.yaml` — **the artifact worth keeping**

| tier | entity | size | TTL |
|---|---|---|---|
| hot (in-proc LRU) | episodic | 100 | 300 s (5 min) `:19-21` |
| hot | semantic | 100 | 1800 s (30 min) `:22-24` |
| hot | procedural | 50 | 3600 s (60 min) `:25-27` |
| warm (Redis) | all | — | 7200 s (2 h) `:30-32` |
| negative | episodic / semantic / procedural | — | 15 / 30 / 60 s `:40-43` |

Retrieval limits `:50-57`: semantic 10, episodic_relevant 5, episodic_recent 5, procedural 5, timeline 10, verbatim 20. Hybrid weights `:67-69`: semantic 0.7 / chronological 0.3. Token policy `strategy: tail_trim`, trim order episodic→semantic→procedural, compaction `target_ratio: 0.6`.

**This YAML is portable as-is.**

### C.6 AWS S3 — `airs-alpha-storage` — **DELETE**
`boto3>=1.34.0`; `AwsStorageService` (`api_gateway/core/aws_storage.py:37`). Config: `AWS_REGION=ap-southeast-2`, `AWS_S3_BUCKET=airs-alpha-storage`, `SIGN_TTL_SECONDS=600`, `MAX_UPLOAD_MB=100`. Required only with `ENABLE_FILES=True`. **Zero Rust callers. A desktop app writes to the local filesystem.**

### C.7 MCP servers (external processes)
Per `autodev_mcp_config.yaml`:
- **`autodev-mcp-server`** — Streamable HTTP at the Rust in-process server (port 8767 with fallback). `request_timeout: 300.0` s, `connection_timeout: 30.0`, `max_retries: 3`, cache **disabled**. Exposes **26 tools**: `read_file, write_file, edit_file, append_file, check_file, list_directory, grep, glob, bash, list_tasks, get_task, create_task, assign_task, update_task_status, claim_task, request_supervisor_action, suggest_worker, analyze_diff, annotate_file, set_task_budget, get_epic_progress, get_ready_tasks_in_phase, spawn_subagent, spawn_parallel_subagents, planner`.
- **`mcp-memory`** — HTTP, bearer `dev`, port 8000; six memory tools.
- Connection pool: `max_pool_size 10`, `max_connection_per_server 3`, `idle_timeout 300.0`, `max_lifetime 3600.0`.

**Rust replacement:** **the AutoDev MCP server already IS Rust — this is a round-trip** (Rust tools → HTTP MCP → Python bridge → LLM → back). **Removing the sidecar collapses it to a direct call.** `mcp-memory` is likewise redundant with `memory.db`.

### C.8 Tavily — web search — optional
`tavily-python>=0.7.21`; `TavilySuiteTool` (`src/internal_tools/web/tavily_suite.py:325`) imports lazily and errors cleanly if absent. **If web search is wanted, call the Tavily REST API directly with `reqwest`. Nothing stateful to migrate.**

### C.9 A2A ports 5001–5099 — **DELETE** (in-process channels)

### C.10 OpenTelemetry — optional
`opentelemetry-api/sdk==1.33.0`, used lightly (`observability/metrics.py`, 87 lines). Rust equivalent: `tracing` + `tracing-opentelemetry` if desired.

### C.11 Other Python-only runtime dependencies — **all DELETE**
`pymupdf`, `pdf2image` (needs system poppler), `python-magic` (needs libmagic), `pillow` — only for `/api/files` thumbnails. `bcrypt` + `PyJWT` — only for `/api/auth`. `watchdog` — config hot reload. `psutil` — `/api/system/*`. `sse-starlette`. `a2a-sdk`, `protobuf<6`.

### C.12 ⚠️ SECRETS POSTURE — FLAG BEFORE ARCHIVING

**`.env` carries live plaintext credentials in the working tree:**
- AWS access key + secret (`:6-7`)
- the Fernet `MCP_VAULT_KEY` (`:67`)
- provider API keys (`:49-59`)
- `REGISTRATION_ADMIN_KEY` (`:79`)
- `API_KEYS=["dev-key-123",…]` (`:24`) — **hardcoded on the Rust side at `src-tauri/src/sidecar/client.rs:95`**
- `DEBUG_AUTH_BYPASS=True` (`:40`) disables `/api/*` auth entirely

**Rotate all of these regardless of the port.** This couples to `G0 §3.1`: the same tree is declared as a Tauri bundle resource, so a locally-built installer ships these credentials to end users.

---

## PORT / DELETE SUMMARY

| Subsystem | Verdict | One-line reason |
|---|---|---|
| `src/llm/providers` | **PORT** | Model registry + tool loop + provider key routing **is** the product |
| `src/llm/sampling` | DELETE | MCP server-initiated sampling; no Rust caller |
| `execution/` | **PORT** | Tool dispatch + in-flight registry |
| `streaming/` | **PORT** | 3-mode thinking detector; Rust does it strictly better |
| `session/` | **PORT** | Per-session runtime isolation + idle reap (600 s / 500 cap) |
| `state/` | PARTIAL (5 of 20) | Rust enums subsume the pattern |
| `prompts/` | PARTIAL | Port the YAML, drop the templater |
| `intelligence/` | PARTIAL | Keep decision engine, drop Adobe-tuned failure regexes |
| `bridges/` | PARTIAL | Keep `MCPBridge` + DELEGATE parser; drop the 2874-line Adobe compiler |
| `events/` | PARTIAL | Keep the bus, cut 1986 lines of schema to ~8 families |
| `security/` | PARTIAL | Keep redaction; replace vault with OS keychain (already done in Rust) |
| `mcp/` | DELETE | Redis-backed prompt/card generation for a server product |
| `a2a/` | DELETE | 99 HTTP ports to replace a function call |
| `lifecycle/` | DELETE | Boot sequencer for a process that ceases to exist |
| `cache/` | DELETE | 4 of 5 caches are Redis namespaces; keep the TTL table only |
| `airs/llm/enrichment_service.py` | DELETE | MCP wizard UX |
| **AWS S3 / files** | DELETE | Desktop app writes to the local filesystem |
| **Redis / Memurai** | DELETE | Replaced by SQLite/`redb` + OS keychain |
| **Postgres / Qdrant** | N/A | Never actually wired up (C.4) |
