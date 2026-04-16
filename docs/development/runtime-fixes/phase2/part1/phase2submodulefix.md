# AutoDev — Error Fix Guide (Refined)
**Based on:** `submodulestesting.txt` run — 2026-04-15  
**Codebase Analysis Date:** 2026-04-16  
**Errors covered:** E-01 through E-11 (10 actionable fixes + 1 info)  

> **⚠️ CRITICAL:** This document has been refined against the ACTUAL codebase. All file paths and line numbers have been verified.

---

## Executive Summary

After analyzing `submodulestesting.txt` and cross-referencing with the actual codebase across `autodev/`, `core-orchestrator/`, and `memory-mcp-server/`, I identified **11 distinct errors** causing agent failures. Below are **atomic fix tasks** organized by priority and dependency chain.

**Dependency Graph (Corrected — verified against codebase):**
```
E-01 (Tool Wiring) ──► Agent Execution Works   ← INDEPENDENT of E-02

E-02 (Redis) ──► E-07 (Recall Latency)         ← Redis only affects warm-tier cache

E-11 (Unicode) ──► Fixes Log Loss

E-03 (Memory Contamination) ──► Isolated fix, no dependencies

E-04 (Startup Race) ──► Isolated fix in RecipeScheduler

E-05 (SSE Spam) ──► Isolated fix in StreamManager

E-06 (Circuit Breaker) ──► Isolated config fix

E-08 (asyncio) ──► memory-mcp-server only, low priority

E-09 (API Key Leak) ──► Partially fixed, needs verification
```

> **⚠️ IMPORTANT CORRECTION:** Previous analysis incorrectly stated E-02 (Redis) causes E-01 (tool handlers). After deep codebase trace, **E-01 is completely independent of Redis**. `UnifiedToolHandler`, `AutoDevToolHandler`, and all bridge initialization have zero Redis dependency. The true cause of E-01 is a broken configuration pipeline from Tauri to `factory.py`. See FIX-1 below.

---

## Critical Path Fixes (Do These First)

### 🔴 FIX-1: E-01 — Tool Handlers Never Registered (`total_tools: 0`)

**Error:** All 5 handlers `registered: False`, `total_tools: 0` for every agent  
**Root Cause:** Broken configuration pipeline across two gaps — verified by reading actual YAML on disk and tracing Tauri → API → factory → metadata chain  
**Impact:** 🔴 CRITICAL — Agents have zero tools; every recipe tick is a no-op text generation  

---

#### Gap A: `autodev_tools` never wired from Tauri → agent config → factory → metadata

**Evidence files:**
- `src-tauri/src/sidecar/client.rs:254-264` — `ensure_agent` POST body has NO `autodev_tools` key
- `api_gateway/modules/agents/models.py:25-46` — `AgentCreate` model has NO `autodev_tools` field
- `src/airs/config/agent_schema.py` — `AgentConfig` has NO `autodev_tools` field
- `src/airs/config/agent_writer.py:319-428` — `create_agent_config_from_request` has NO `autodev_tools` param
- `src/airs/agent/factory.py:440-446` — `AgentMetadata` built WITHOUT setting `autodev_tools`
- `src/airs/agent/living_agent.py:612-613` — `if autodev_tool_names:` gate is ALWAYS `False`
- `config/agents/supervisor-q8e3t5sx.yaml` — no `autodev_tools` field on disk (confirmed)

**The `AutoDevToolHandler` (`src/airs/bridges/autodev_handler.py`) IS fully implemented** with 8 tool schemas (`read_file__autodev`, `write_file__autodev`, `edit_file__autodev`, `bash__autodev`, `list_directory__autodev`, `grep__autodev`, `glob__autodev`, `list_workers__autodev`). The handler works. The problem is no agent is ever told to activate it.

**Atomic Tasks — Gap A:**

- [ ] **T1.1:** Add `autodev_tools` field to `AgentCreate` API model  
  **File:** `submodules/core-orchestrator/api_gateway/modules/agents/models.py:41`  
  After the `internal_tools` field, add:
  ```python
  autodev_tools: List[str] = Field(
      default_factory=list,
      description="AutoDev tool names to activate (e.g. ['read_file__autodev', 'bash__autodev'])"
  )
  ```

- [ ] **T1.2:** Add `autodev_tools` parameter to `create_agent_config_from_request`  
  **File:** `submodules/core-orchestrator/src/airs/config/agent_writer.py:319-428`  
  Add param and pass through to `AgentConfig`. Decide: store in YAML metadata dict or as a dedicated field.

- [ ] **T1.3:** Add `autodev_tools` to `AgentConfig` schema  
  **File:** `submodules/core-orchestrator/src/airs/config/agent_schema.py`  
  Add field after `internal_tools`:
  ```python
  autodev_tools: List[str] = Field(
      default_factory=list,
      description="AutoDev tool IDs forwarded from Tauri for fleet workers"
  )
  ```

- [ ] **T1.4:** Read `autodev_tools` from `AgentConfig` in `factory.py` and set on metadata  
  **File:** `submodules/core-orchestrator/src/airs/agent/factory.py:440-446`  
  After building `internal_tool_ids`, add:
  ```python
  autodev_tool_names = (
      agent_config.autodev_tools
      if agent_config.autodev_tools
      else []
  )
  metadata = AgentMetadata(
      tags=agent_tags,
      capabilities=[],
      version=agent_config.version,
      mcp_servers=mcp_server_ids,
      internal_tools=internal_tool_ids,
      autodev_tools=autodev_tool_names,  # ADD THIS
  )
  ```

- [ ] **T1.5:** Add `autodev_tools` to Tauri `ensure_agent` POST body  
  **File:** `src-tauri/src/sidecar/client.rs:254-264`  
  In `find_or_create_agent`, add the full tool list to `create_body`:
  ```rust
  let create_body = serde_json::json!({
      "name":          slug,
      "job_title":     job_title,
      "gender":        "male",
      "description":   format!("AutoDev fleet worker agent — {slug}"),
      "llm_model":     model,
      "system_prompt": system_prompt,
      "mcp_servers":   ["mcp-memory"],
      "internal_tools": [],
      "autodev_tools": [               // ← ADD THIS
          "read_file__autodev",
          "write_file__autodev",
          "edit_file__autodev",
          "bash__autodev",
          "list_directory__autodev",
          "grep__autodev",
          "glob__autodev",
          "list_workers__autodev",
      ],
      "capabilities":  ["task_management", "code_generation", "autodev_fleet"],
  });
  ```

---

#### Gap A — Secondary Bug: `get_tool_specs()` returns plain dicts, `add_tools()` rejects them

**Verified:** Even after T1.1–T1.5 populate `autodev_tool_names`, `add_tools()` at `living_agent.py:714` uses `hasattr(tool, 'type')` — this returns `False` for plain Python dicts. All 8 tools are silently dropped as invalid. Additionally `living_agent.py:628` calls `s.function.get('name')` which raises `AttributeError` on a dict. Both require a fix to `get_tool_specs()` to return proper `ToolSpec` Pydantic objects.

**Atomic Tasks — Gap A Secondary:**

- [ ] **T1.6:** Fix `get_tool_specs()` to return `ToolSpec` objects instead of dicts  
  **File:** `submodules/core-orchestrator/src/airs/bridges/autodev_handler.py:152-173`  
  At the top of the method, import the ToolSpec type. Replace the dict append with a ToolSpec construction:
  ```python
  def get_tool_specs(self, tool_names: List[str]) -> List[Any]:
      from src.llm.providers.schema import ToolSpec, ToolFunction  # import ToolSpec here
      specs = []
      for name in tool_names:
          base = self._strip_suffix(name)
          schema = self.TOOL_SCHEMAS.get(base)
          if schema:
              specs.append(ToolSpec(
                  type="function",
                  function=ToolFunction(
                      name=name,
                      description=schema["description"],
                      parameters=schema["parameters"],
                  ),
              ))
          else:
              logger.warning(f"No schema for AutoDev tool: {name} (base={base})")
      return specs
  ```
  **Note:** If `ToolSpec` / `ToolFunction` are not importable from `src.llm.providers.schema`, inspect the actual schema module for the correct type names and adjust the import accordingly. Do NOT use a dict.

- [ ] **T1.7:** Verify after fix  
  Check log: `"✅ AutoDev Tool Handler initialized for {name} with tools: [...]"` (`living_agent.py:636-639`)  
  Check log: `Handler status: {'autodev_handler': {'registered': True, 'tools': [...]}, 'total_tools': 8}`

---

#### Cleanup: Delete stale agent YAMLs that lack the new fields

- [ ] **T1.10:** Delete stale agent config files on disk so next `ensure_agent` recreates them with the updated body  
  **Directory:** `submodules/core-orchestrator/config/agents/`  
  Delete all `supervisor-*.yaml`, `worker-backend-*.yaml`, `worker-frontend-*.yaml`, `worker-mcp-*.yaml`, `qc-agent-*.yaml`  
  On next run, Tauri will call `ensure_agent` → `POST /api/agents` → new YAML with `autodev_tools` + `mcp_servers`

---

### 🔴 FIX-2: E-02 — Redis/Memurai Warm Tier

**Error:** Redis not running → warm tier unavailable → memory recall latency, agent card store fails  
**Location:** `api_gateway/app.py:128-194` (auto-start logic exists but may fail)  
**Impact:** 🔴 HIGH — Warm tier disabled, agent card store unavailable, log spam  
**Note:** Does NOT cause E-01. Fix independently.

#### Atomic Tasks:

- [ ] **T2.1:** Verify Memurai service status on Windows
  ```powershell
  Get-Service -Name Memurai | Select-Object Name, Status, StartType
  ```
  **Verify:** Status = Running, StartType = Automatic

- [ ] **T2.2:** Test Redis connectivity from Python
  ```python
  import redis.asyncio as async_redis
  client = async_redis.from_url("redis://127.0.0.1:6379", socket_connect_timeout=2)
  await client.ping()
  ```

- [ ] **T2.3:** If Memurai not installed, implement Tauri Redis sidecar (long-term)  
  See FIX-11 for strategic path

- [ ] **T2.4:** Verify warm tier connection in `factory.py:220-248`
  - Confirm `redis_client.ping()` succeeds
  - Check log: `"🔥 [REDIS] Connected for warm tier caching"`

---

### 🔴 FIX-3: E-11 — UnicodeEncodeError in Logging

**Error:** `UnicodeEncodeError: 'utf-8' codec can't encode characters in position 63-64: surrogates not allowed`  
**Location:** Python logging `__init__.py:1153` (stream.write)  
**Impact:** 🔴 HIGH — Log data loss, may hide root cause of other errors  

#### Atomic Tasks:

- [ ] **T3.1:** Verify file handler fix is already applied — NO ACTION NEEDED  
  **File:** `src/airs/logging/centralized_collector.py:274-280`  
  **STATUS: ALREADY FIXED** — `_get_file_handle()` already opens files with `errors="replace"` and `_sanitize_string()` is called at lines 350-352 before every write. This covers all file-based log output. No changes required here.
  ```python
  # Already present — confirm only:
  self.file_handles[key] = open(
      file_path, mode, 
      encoding="utf-8", 
      errors="replace",  # ← Already there
      buffering=1
  )
  ```

- [ ] **T3.2:** Add console stream reconfiguration in core-orchestrator startup
  **File:** `api_gateway/app.py` (in lifespan startup, after logger setup)
  **Add after line 196:**
  ```python
  import sys
  if sys.platform == 'win32':
      sys.stdout.reconfigure(encoding='utf-8', errors='replace')
      sys.stderr.reconfigure(encoding='utf-8', errors='replace')
  ```

- [ ] **T3.3:** Set PYTHONIOENCODING in Tauri sidecar spawn
  **File:** `src-tauri/src/sidecar/mod.rs` (in `start()` method)
  **Add env var:**
  ```rust
  .env("PYTHONIOENCODING", "utf-8:replace")
  ```

- [ ] **T3.4:** Verify `_sanitize_string()` is called before all log writes
  **File:** `src/airs/logging/centralized_collector.py:350-352`
  ```python
  # Confirm this exists:
  text_output = self._sanitize_string(entry.to_text())
  json_output = self._sanitize_string(entry.to_json())
  ```

---

## High Priority Fixes (Do After Critical Path)

### 🟠 FIX-4: E-03 — Cross-Agent Episodic Memory Contamination

**Error:** Ephemeral `headless-{uuid}` session IDs generated per tick — every worker starts every tick with amnesia AND unscoped memories from all other workers bleed into every recall  
**Primary location:** `submodules/core-orchestrator/api_gateway/modules/agents/service.py:1315-1318`  
**Secondary locations:** `stream_manager.py:140`, `mem0_adapter.py:864-867`, `context_service.py:1826-1835`  
**Impact:** 🟠 HIGH — Worker identity corruption, zero episodic continuity across ticks  

**Root cause (architectural):** The core-orchestrator memory system uses `(user_id, session_id)` pairs as memory lanes — by design. Recipe ticks arrive headless (`session_id=None`), and `service.py` assigns a fresh random UUID on every single tick. This breaks both isolation (no stable lane per worker) and continuity (memories from tick N are unreachable on tick N+1).

---

#### PRIMARY FIX — Stable Per-Worker Session ID

- [ ] **T4.0:** Change headless session_id generation from random UUID to stable per-worker identity  
  **File:** `submodules/core-orchestrator/api_gateway/modules/agents/service.py:1318`  
  **Current (ephemeral — new UUID every tick):**
  ```python
  headless = session_id is None
  if headless:
      import uuid
      session_id = f"headless-{uuid.uuid4()}"   # ← amnesia + bleed source
  ```
  **Fixed (stable — deterministic per-worker memory lane):**
  ```python
  headless = session_id is None
  if headless:
      session_id = f"worker-{agent_id}"   # agent_id already resolved at line 1305
  ```
  `agent_id` is the registry key (e.g. `worker-backend-b5vsadgp`) available at the call site.
  No changes to Tauri `client.rs` — `session_id` continues to arrive as `None`, triggering the headless path unchanged.

  **Why this works:**
  - `worker-backend-b5vsadgp` always writes and reads its own Mem0 lane — structurally impossible to collide with `worker-mcp-ch0cayz4`
  - Workers accumulate episodic memory across ticks — a worker can recall what it did on the previous tick
  - `headless = True` is still set, so SSE validation is still bypassed — PA chat sessions are **not affected**

  **Compatibility check — no breakages:**

  | Scenario | Effect | Safe? |
  |---|---|---|
  | PA chat sessions | Real `session_id` from UI → `headless = False` → SSE validation runs normally | ✅ Zero change |
  | Episodic memory (`_get_cached_experiences`) | Stable session → each worker builds its own timeline | ✅ Improved |
  | Semantic facts (`_get_cached_facts`) | Scoped by `user_id` only, not `session_id` | ✅ Unaffected |
  | Procedural learnings (`_get_cached_learnings`) | Scoped by `agent_id` — already isolated | ✅ Unaffected |
  | Continuity packets | Stored/loaded by `agent_id` in Redis | ✅ Unaffected |
  | `RuntimeManager` session reuse | `get_runtime(session_id="worker-{slug}", create_if_missing=True)` → same warm runtime reused per worker across ticks | ✅ Beneficial |
  | Global/unscoped memory (future) | After T4.2: memories without `session_id` excluded from scoped queries, accessible via `session_id=None` only — clean design | ✅ |

---

#### REQUIRED FOLLOW-UP — E-05 Guard Extension

- [ ] **T4.1:** Extend `stream_manager.py` headless guard to also skip `worker-*` sessions  
  **File:** `submodules/core-orchestrator/api_gateway/core/stream_manager.py:140`  
  **Current:**
  ```python
  if session_id.startswith("headless-"):
      return
  ```
  **Fixed:**
  ```python
  if session_id.startswith("headless-") or session_id.startswith("worker-"):
      return
  ```
  Without this update, E-05 warnings continue firing — same volume, just with a `worker-` prefix instead of `headless-`. This fix is a **required companion** to T4.0.

---

#### DEFENSE-IN-DEPTH — Bug 1: Permissive Session Filter

- [ ] **T4.2:** Fix permissive session filter in `list_memories_async`  
  **File:** `memory-mcp-server/src/memory_providers/adapters/mem0_adapter.py:864-867`  
  **Current (passes unscoped memories through):**
  ```python
  if item_session_id is not None and item_session_id != session_id:
      continue
  ```
  **Fixed:**
  ```python
  if item_session_id != session_id:
      continue
  ```
  Memories stored before T4.0 was applied have no `session_id` in Mem0 metadata. This filter closes the legacy data bleed gap. Also enables the "global memory" pattern: memories stored without `session_id` are only accessible when caller passes `session_id=None`.

- [ ] **T4.3:** Verify the Mem0 `get_all()` filter parameter is passed correctly  
  **File:** `memory-mcp-server/src/memory_providers/adapters/mem0_adapter.py:812-814`  
  Confirm `params["filters"] = {"metadata.session_id": session_id}` is set when `session_id` is provided. This passes the filter hint to Qdrant; T4.2 is the authoritative post-retrieval safety net.

---

#### DEFENSE-IN-DEPTH — Bug 2: Missing `agent_id` Scope in context_service

- [ ] **T4.4:** Add `agent_id` to the `filters` dict in `_retrieve_recent_conversation`  
  **File:** `submodules/core-orchestrator/api_gateway/services/context_service.py:1826-1835`  
  **Current (no agent scoping):**
  ```python
  "filters": {
      "metadata.type": "episodic"
  }
  ```
  **Fixed:**
  ```python
  "filters": {
      "metadata.type": "episodic",
      "agent_id": agent_id,  # ADD — second isolation layer
  }
  ```
  Thread `agent_id` into `_retrieve_recent_conversation` from `get_context_for_agent()` which already holds it. This provides a second isolation layer independent of session_id.

- [ ] **T4.5:** Apply same `agent_id` scoping to `_retrieve_relevant_conversation` and `get_timeline_messages`  
  **Files:**  
  - `context_service.py:1871-1941` (`_retrieve_relevant_conversation`) — add `"agent_id": agent_id` to filters  
  - `context_service.py:2017-2155` (`get_timeline_messages`) — add `"agent_id": agent_id` to filters  

- [ ] **T4.6:** Verify `agent_id` filter passthrough in `mem0_adapter.get_all()`  
  **File:** `memory-mcp-server/src/memory_providers/adapters/mem0_adapter.py:817-821`  
  Confirm existing code already handles it:
  ```python
  if filters:
      if "agent_id" in filters:
          params["agent_id"] = filters["agent_id"]  # ← Already there — no change needed
  ```

---

#### CLEANUP — Wipe Contaminated Test Memories

- [ ] **T4.7:** After applying T4.0–T4.6, wipe all unscoped memories from previous test runs  
  ```python
  from mem0 import Memory
  m = Memory.from_config(config)
  all_memories = m.get_all(user_id='service_autodev')
  for mem in all_memories.get('results', []):
      m.delete(mem['id'])
  ```
  Run once after deployment; not needed on subsequent clean starts.

---

### 🟠 FIX-5: E-04 — RecipeScheduler First-Tick Failures

**Error:** RecipeScheduler fires at t+12s but core-orchestrator ready at t+26s  
**Location:** `src-tauri/src/recipes/scheduler.rs:77-119` (schedule loop)  
**Impact:** 🟠 MEDIUM — First recipe ticks fail on cold start  

#### Atomic Tasks:

- [ ] **T5.1:** Add health check gate before first tick
  **File:** `src-tauri/src/recipes/scheduler.rs`
  **Insert before line 77 (spawn block):**
  ```rust
  async fn wait_for_orchestrator_ready(
      orch_client: &OrchestratorClient,
      timeout_secs: u64
  ) -> bool {
      let deadline = tokio::time::Instant::now() + 
          tokio::time::Duration::from_secs(timeout_secs);
      loop {
          if orch_client.is_healthy().await {
              return true;
          }
          if tokio::time::Instant::now() >= deadline {
              return false;
          }
          tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
      }
  }
  ```

- [ ] **T5.2:** Call health check before entering cron loop
  **File:** `src-tauri/src/recipes/scheduler.rs:77-85`
  **After line 77 (inside spawn):**
  ```rust
  // Wait for orchestrator before first tick
  if !wait_for_orchestrator_ready(&runner.orch_client, 45).await {
      warn!("[RecipeScheduler] Orchestrator not ready after 45s, starting anyway");
  }
  
  for next_fire in schedule.upcoming(Utc) {
      // ... existing loop
  }
  ```

- [ ] **T5.3:** Alternative: Add startup_delay_secs to recipe config
  **File:** `src-tauri/src/recipes/types.rs` (Recipe struct)
  **Add field:**
  ```rust
  pub startup_delay_secs: Option<u64>,
  ```
  
  **File:** `src-tauri/src/recipes/scheduler.rs:80-85`
  **Before cron loop:**
  ```rust
  if let Some(delay) = recipe.startup_delay_secs {
      info!("[RecipeScheduler] Delaying {}s for startup", delay);
      tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
  }
  ```

---

## Medium Priority Fixes (Background Tasks)

### 🟡 FIX-6: E-05 — SSE Broadcast Spam for Headless Sessions

**Error:** `[SSE] Cannot broadcast to session headless-...: session not found` warnings  
**Location:** `api_gateway/core/stream_manager.py:140-155`  
**Impact:** 🟡 LOW — Log noise, not functional issue  

#### Atomic Tasks:

- [ ] **T6.1:** Add headless session guard in `broadcast_to_session`
  **File:** `api_gateway/core/stream_manager.py:140-155`
  **Modify line 153:**
  ```python
  async def broadcast_to_session(self, session_id: str, data: Union[str, Dict[str, Any]]) -> None:
      # Skip silently for headless sessions (no UI)
      if session_id.startswith("headless-"):
          return
      
      if session_id not in self.session_owners:
          # Only warn for non-headless sessions
          logger.warning(f"⚠️ [SSE] Cannot broadcast to session {session_id}: session not found")
          return
  ```

- [ ] **T6.2:** Apply same guard in `UnifiedEventBroker`
  **File:** `api_gateway/core/stream_manager.py:607-644` (emit_agent_start)
  **Files:** All emit_* methods in UnifiedEventBroker
  **Quick fix:** Add guard at top of each emit method

---

### 🟡 FIX-7: E-06 — Log Circuit Breaker Threshold

**Error:** LOG-CIRCUIT-BREAKER OPEN triggered during normal startup (1000 logs/10s)  
**Location:** `src/airs/logging/centralized_collector.py:102-165`  
**Impact:** 🟡 LOW — Log suppression during startup  

#### Atomic Tasks:

- [ ] **T7.1:** Verify — threshold is already 2500, NO CHANGE NEEDED  
  **File:** `src/airs/logging/centralized_collector.py:111`  
  **STATUS: ALREADY FIXED** — `threshold: int = 2500` with `cooldown_seconds: float = 2.0` (labeled `# Issue 8 fix`). Do NOT raise threshold further — the problem is log source volume, not the breaker sensitivity.

- [ ] **T7.2:** Silence LiteLLM debug logging
  **File:** `src/llm/providers/litellm_provider.py:1559-1568`
  **Or add in `api_gateway/app.py` startup:**
  ```python
  import logging
  import litellm
  litellm.suppress_debug_info = True
  logging.getLogger("LiteLLM").setLevel(logging.WARNING)
  ```

- [ ] **T7.3:** Set environment variable for LiteLLM
  **File:** `.env` or Tauri spawn env
  ```
  LITELLM_LOG=WARNING
  ```

---

### 🟡 FIX-8: E-07 — Slow Memory Recall (1398ms)

**Error:** Memory recall taking 1398ms due to cold Qdrant + no Redis warm tier  
**Location:** `src/airs/memory/memory_core.py:1185-1448` (recall method)  
**Impact:** 🟡 MEDIUM — Slow agent response times  

#### Atomic Tasks:

- [ ] **T8.1:** Verify Redis fix from T1 is complete
  - Warm tier should reduce recall to < 200ms after cache populates

- [ ] **T8.2:** If still slow, optimize Qdrant path
  **File:** `memory-mcp-server` Qdrant configuration
  - Set stable persistent path (not `/tmp/qdrant`)
  - Enable payload indexing for `agent_id` field

- [ ] **T8.3:** Verify continuity packet fast boot
  **File:** `src/airs/memory/memory_core.py:1243-1271`
  - Confirm FAST BOOT path is taken
  - Check log: `"⚡ [CONTINUITY-SEED] Loaded continuity packet as seed"`

---

## Lower Priority / Next Sprint

### 🟢 FIX-9: E-09 — API Keys Exposed in Debug Logs

**Error:** OpenAI API key visible in plaintext in log file  
**Location:** `memory-mcp-server/src/memory_providers/adapters/mem0_adapter.py:112-118`  
**Impact:** 🟢 MEDIUM — Security risk  

#### Atomic Tasks:

- [ ] **T9.1:** Mask API keys in Mem0 adapter logging
  **File:** `memory-mcp-server/src/memory_providers/adapters/mem0_adapter.py:112-118`
  **Current:**
  ```python
  logger.info(
      "Building OSS config - DEBUG",
      config_values={k: v for k, v in self.config.items() if k not in ['api_key']}
  )
  ```
  
  **Verify:** `'api_key'` is already in the exclusion list
  **If still leaking:** Check other log sites

- [ ] **T9.2:** Verify LiteLLM logging fix from T7 also fixes key exposure
  - If LiteLLM DEBUG is disabled, full request payloads won't be logged

- [ ] **T9.3:** Rotate exposed key if needed
  - The `sk-proj-...` key from log should be rotated in OpenAI dashboard

---

### 🟢 FIX-10: E-08 — asyncio DeprecationWarning

**Error:** `DeprecationWarning: There is no current event loop` on Python 3.12+  
**Location:** `memory-mcp-server/main.py:581`  
**Impact:** 🟢 LOW — Warning only, becomes error in Python 3.12+  

#### Atomic Tasks:

- [ ] **T10.1:** Replace `asyncio.get_event_loop()` with `asyncio.new_event_loop()`
  **File:** `memory-mcp-server/main.py:581`
  ```python
  # Current (deprecated)
  loop = asyncio.get_event_loop()
  
  # Fixed
  try:
      loop = asyncio.get_running_loop()
  except RuntimeError:
      loop = asyncio.new_event_loop()
      asyncio.set_event_loop(loop)
  ```

- [ ] **T10.2:** Check all usages in mem0_adapter.py
  **Search:** `asyncio.get_event_loop()` in `memory-mcp-server/src/`
  **Files with usages:**
  - `base.py:539, 545` (health_check_async)
  - `mem0_adapter.py:319, 349, 447, 596, 605, etc.`
  
  **Fix pattern:** Replace with `asyncio.get_running_loop()` or use `asyncio.run()`

---

## Long-Term / Strategic Fixes

### ⬛ FIX-11: Redis Auto-Spawn Sidecar

**Goal:** Ship Redis as Tauri sidecar, eliminate Memurai dependency  
**Effort:** Half-day  
**Reference:** `AutoDev_Redis_AutoSpawn_Research` doc  

#### Atomic Tasks:

- [ ] **T11.1:** Place Redis binaries in `src-tauri/binaries/`
  - Windows: `redis-server-x86_64-pc-windows-msvc.exe`
  - macOS: `redis-server-aarch64-apple-darwin` (Apple Silicon)
  - macOS: `redis-server-x86_64-apple-darwin` (Intel)
  - Linux: `redis-server-x86_64-unknown-linux-gnu`

- [ ] **T11.2:** Create `src-tauri/src/redis_manager.rs`
  ```rust
  pub async fn spawn_redis(app: &AppHandle) -> Result<(), String>
  pub async fn is_redis_alive() -> bool
  ```

- [ ] **T11.3:** Register in `tauri.conf.json`
  ```json
  {
    "bundle": {
      "externalBin": ["binaries/redis-server"]
    }
  }
  ```

- [ ] **T11.4:** Integrate into startup sequence in `main.rs`
  - Spawn Redis before memory-mcp-server
  - Wait for `:6379` to be ready

---

## Verification Checklist

After applying all fixes, run a fresh cold start and verify:

```
✅ Handler status: {'autodev_handler': {'registered': True}, 'total_tools': 8+}   ← FIX-1 T1.7
✅ [REDIS] Ready.                                                                   ← FIX-2 T2.1
✅ No UnicodeEncodeError in logs                                                    ← FIX-3 T3
✅ No cross-agent memory bleed                                                      ← FIX-4 T4
✅ No "Recipe run failed" on first tick                                              ← FIX-5 T5
✅ No "[SSE] Cannot broadcast to headless-..." warnings                              ← FIX-6 T6
✅ No LOG-CIRCUIT-BREAKER OPEN during startup                                       ← FIX-7 T7
✅ Slow recall < 300ms after warm tier active                                        ← FIX-8 T8
✅ No api_key values visible in log output                                           ← FIX-9 T9
✅ No DeprecationWarning in memory-mcp-server                                        ← FIX-10 T10
```

**End-to-end validation:**
```
[RecipeScheduler] Firing 'supervisor-poll'
[core-orch] list_tasks called → returned N tasks
[core-orch] update_task_status: task_XYZ → in_progress
[RecipeScheduler] Recipe run completed successfully
```

---

## Quick Reference: File Locations Summary

| Error | Primary Files | Lines |
|-------|---------------|-------|
| E-01 (Gap A) | `src-tauri/src/sidecar/client.rs` | 254-264 (Tauri create body) |
| E-01 (Gap A) | `api_gateway/modules/agents/models.py` | 25-46 (AgentCreate schema) |
| E-01 (Gap A) | `src/airs/config/agent_schema.py` | (AgentConfig schema) |
| E-01 (Gap A) | `src/airs/config/agent_writer.py` | 319-428 (config builder) |
| E-01 (Gap A) | `src/airs/agent/factory.py` | 430-446 (metadata build) |
| E-01 (Gap A) | `src/airs/agent/living_agent.py` | 612-613 (handler gate) |
| E-01 (Secondary) | `src/airs/bridges/autodev_handler.py` | 152-173 (get_tool_specs returns dicts) |
| E-02 | `api_gateway/app.py` | 128-194 |
| E-03 (Primary) | `api_gateway/modules/agents/service.py` | 1315-1318 (headless session_id) |
| E-03 (Follow-up) | `api_gateway/core/stream_manager.py` | 140 (headless guard — extend for worker-*) |
| E-03 (Def-in-depth Bug 1) | `memory-mcp-server/src/memory_providers/adapters/mem0_adapter.py` | 864-867 (session filter) |
| E-03 (Def-in-depth Bug 2) | `api_gateway/services/context_service.py` | 1826-1835 (_retrieve_recent_conversation) |
| E-04 | `src-tauri/src/recipes/scheduler.rs` | 77-119 |
| E-05 | `api_gateway/core/stream_manager.py` | 140-155 |
| E-06 | `src/airs/logging/centralized_collector.py` | 102-165 (threshold already 2500) |
| E-07 | `src/airs/memory/memory_core.py` | 1185-1448 |
| E-08 | `memory-mcp-server/main.py` | 581 |
| E-09 | `memory-mcp-server/src/memory_providers/adapters/mem0_adapter.py` | 112-118 |
| E-11 (File) | `src/airs/logging/centralized_collector.py` | 274-280, 350-352 (ALREADY FIXED) |
| E-11 (Console) | `api_gateway/app.py` | startup — add sys.stdout.reconfigure |

---

*Refined fix guide version: 1.4 — 2026-04-16 | E-03 root cause elevated to architectural: ephemeral headless-{uuid} session IDs cause both worker amnesia and cross-agent bleed simultaneously. Primary fix is one-line change in service.py:1318 (stable per-worker session `f"worker-{agent_id}"`). Required companion: extend stream_manager E-05 guard to also skip `worker-*` prefix. Bug 1 (mem0_adapter:864) and Bug 2 (context_service:1826) remain as defense-in-depth. Full compatibility analysis added — PA chat sessions and all memory tiers unaffected.*
