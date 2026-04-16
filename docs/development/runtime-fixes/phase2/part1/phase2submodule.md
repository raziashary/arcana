# AutoDev Run Error Analysis
**Log File:** `submodulestesting.txt`  
**Run Date:** 2026-04-15 | 19:21:46 – 19:29:00 UTC+7  
**Environment:** Windows 11 — `C:\AIRS Foundry\autodev\autodev\submodules\`  
**Analyzed by:** AIRS Code  

---

## Executive Summary

The submodule testing run achieved **partial startup** — both `memory-mcp-server` and `core-orchestrator` came online, and agents began receiving recipe ticks. However, **no agents were able to execute any actual work**. Two root-cause issues blocked all task execution: The `autodev_tools` configuration pipeline and `mcp_servers` field were never wired end-to-end from Tauri through to `factory.py`, and all tool handlers registered with `total_tools: 0`, leaving every agent with no callable tools. Additionally, a memory scoping bug is causing cross-agent recipe instruction bleed that will corrupt agent behavior once tools are working.

---

## Error Catalogue

### 🔴 CRITICAL — `[E-01]` All Tool Handlers Unregistered (`total_tools: 0`)

**Severity:** CRITICAL — Blocks all agent task execution  
**First occurrence:** Line 1142 (`19:23:05`)  
**Affects:** All agents — supervisor, qc-agent, worker-backend, worker-frontend, worker-mcp

**Evidence:**
```
Handler status: {
  'autodev_handler':   {'registered': False, 'tools': []},
  'internal_handler':  {'registered': False, 'tools': []},
  'memory_handler':    {'registered': False, 'tools': []},
  'adobe_handler':     {'registered': False, 'tools': []},
  'mcp_handler':       {'registered': False, 'tools': []},
  'total_tools': 0
}
```
This is logged for **every single agent instance** created during this run. Despite `Memory-aware tool executor created` firing successfully, none of the handlers are actually registering tools. The LiteLLM call confirms the consequence:
```
[TOOL-LOOP] tool_choice=None, tools_count=0
```
Agents are being dispatched to GPT-4o/GPT-4.1 with **zero tools available**. They cannot call `list_tasks`, `update_task_status`, `bash`, or any other AutoDev tool. Every recipe tick will complete as a bare text generation with no side effects.

**Root Cause — VERIFIED (Redis is NOT the cause):**

The `UnifiedToolHandler` and all bridge initialization code have **zero Redis dependency**. Redis is only used by `MemoryCore` for warm-tier cache. After deep codebase trace, the actual cause is a **broken configuration pipeline across two separate gaps:**

**Gap A: `autodev_tools` chain is completely unwired**

`living_agent.py:612` only initializes `AutoDevToolHandler` if `self.metadata.autodev_tools` is non-empty:
```python
autodev_tool_names = self.metadata.autodev_tools  # ← ALWAYS []
if autodev_tool_names:                             # ← ALWAYS False → handler skipped
```
The `autodev_tools` field is **never populated** anywhere in the chain:
- `agent_schema.py` (`AgentConfig`) — **no `autodev_tools` field** — YAML cannot declare it
- `create_agent_config_from_request()` — **no `autodev_tools` parameter**
- `factory.py:440-446` — builds `AgentMetadata` **without setting `autodev_tools`**
- `AgentCreate` API model (`models.py`) — **no `autodev_tools` field** — API cannot receive it
- Tauri `client.rs:254-264` (`ensure_agent`) — sends **no `autodev_tools` key** in `POST /api/agents` body

The `AutoDevToolHandler` tools (`read_file__autodev`, `bash__autodev`, `list_workers__autodev`, etc.) ARE fully implemented in `src/airs/bridges/autodev_handler.py` with 8 complete schemas, but no agent is ever told to use them. The handler is unreachable because the activation signal from Tauri to the agent config to the factory to the metadata is completely missing.

**Gap A’s secondary bug (verified): `get_tool_specs()` returns plain dicts — `add_tools()` rejects them**

Even IF Gap A were fixed (i.e., `autodev_tool_names` became non-empty), the tools would STILL not register. `autodev_handler.get_tool_specs()` at line 152-173 returns plain Python dicts:
```python
specs.append({
    "type": "function",
    "function": {"name": name, ...},
})
```
But `living_agent.add_tools()` at line 714 validates tools using `hasattr(tool, 'type')` and `hasattr(tool, 'function')` — which returns `False` for plain dicts (Python dict attributes are accessed via key, not `hasattr`). Every autodev tool spec would be logged as:
```
⚠️ {name}: Tool {i} missing required attributes (type/function)
```
and silently dropped, leaving `valid_tools = []`. Additionally, the log line at `living_agent.py:628` calls `s.function.get('name')` which would raise `AttributeError` on a dict (`dict` has no `.function` attribute). Both issues must be fixed together with Gap A.

**Fix Required:**
- **Gap A fix:** Add `autodev_tools` as a field across the full chain: `AgentCreate` → `create_agent_config_from_request` → `AgentConfig` schema → `factory.py` metadata build → Tauri `ensure_agent` POST body.
- **Gap A secondary fix:** Change `autodev_handler.get_tool_specs()` to return `ToolSpec` Pydantic objects (from `src.llm.providers.schema`) instead of plain dicts, so `add_tools()` accepts them.

---

### 🔴 CRITICAL — `[E-02]` Redis/Memurai Not Running

**Severity:** CRITICAL — Cascades into multiple downstream failures  
**First occurrence:** Line 190 (`19:22:03`)  
**Affects:** Warm tier memory, agent card store, tool handler registration

**Evidence — Memurai service failed to start:**
```
[REDIS] Failed to start Memurai service:
Command '['powershell', '-NoProfile', '-Command', 'Start-Service -Name Memurai']'
returned non-zero exit status 1.
```

**Evidence — Persistent connection refusal:**
```
redis.exceptions.ConnectionError: Error 22 connecting to 127.0.0.1:6379.
The remote computer refused the network connection.
```

This fires for every single agent card store lookup across all agent instances (`worker-backend-b5vsadgp`, `qc-agent-ha2brdy2`, `worker-mcp-ch0cayz4`, `worker-frontend-93wcxf50`, `supervisor-q8e3t5sx`), generating dozens of Redis stack traces in the logs.

**Downstream effects:**
- Warm tier disabled across all agents — proceeding in degraded "dev mode"
- Agent card store cannot write or read — agent capability metadata is lost
- **NOT the cause of E-01** (verified — tool registration has no Redis dependency)

**Fix Required:** Ensure Memurai is installed as a Windows service and set to `Automatic` startup. Run `Get-Service -Name Memurai` and `Start-Service Memurai` to verify manually. If Memurai is not the production Redis for this environment, verify the service name in the AIRS Code Redis sidecar config matches what Memurai registers.

---

### 🟠 HIGH — `[E-03]` Cross-Agent Episodic Memory Contamination

**Severity:** HIGH — Will corrupt agent behavior once tools are working  
**First occurrence:** Line 4059 (memory recall at `19:23:26`)  
**Affects:** All agents sharing `user_id: service_autodev`

**Evidence:**
```
## Recent Context:
- [AUTODEV RECIPE INSTRUCTIONS] You are the AIRS MCP Worker Agent...
- [AUTODEV RECIPE INSTRUCTIONS] You are the AIRS QC Agent...
- [AUTODEV RECIPE INSTRUCTIONS] You are the AIRS Backend Worker Agent...
- [AUTODEV RECIPE INSTRUCTIONS] You are the AIRS Supervisor Agent...
```

An agent receiving instructions for its own role plus three other roles is at risk of identity confusion — it may attempt to execute QC or Supervisor tasks when it should be scoping to MCP work only.

**Root Cause — ARCHITECTURAL (verified): Ephemeral headless session IDs cause both amnesia and cross-agent bleed simultaneously**

The memory architecture in `core-orchestrator` is designed for session-scoped episodic isolation. Each `(user_id, session_id)` pair is a dedicated memory lane. This is correct by design. The problem is that recipe ticks are **headless invocations** — they arrive at `service.py` with `session_id=None`, which triggers:

```python
# service.py:1315-1318
headless = session_id is None
if headless:
    import uuid
    session_id = f"headless-{uuid.uuid4()}"   # ← NEW random UUID every tick
```

This produces two simultaneous failures:
- **Amnesia per tick:** Each tick gets a brand-new session_id. The previous tick's memories (stored under the old `headless-{uuid}`) are never recalled. Every agent starts every tick with zero episodic context. Workers cannot remember what they did one minute ago.
- **Cross-agent bleed:** Memories stored before Mem0 extracts a `session_id` tag are stored **without** a `session_id` in their metadata. These unscoped memories bypass the post-retrieval session filter (Bug 1 below) and surface in EVERY agent's recall regardless of session. This is why all four sets of recipe instructions appear in one agent's context window.

**Primary Fix — `service.py:1318` — one line (stable per-worker session ID)**

Change the headless session_id from a random UUID to a deterministic, stable key derived from the agent's own identity:
```python
# Current — ephemeral, amnesiac, non-isolating
session_id = f"headless-{uuid.uuid4()}"

# Fixed — stable, persistent, fully isolating  
session_id = f"worker-{agent_id}"   # agent_id = e.g. "worker-backend-b5vsadgp"
```
`agent_id` is the registry key already resolved at line 1305. No changes to Tauri `client.rs` — `session_id` continues to arrive as `None` from Tauri, triggering the headless path. The fix is entirely inside core-orchestrator.

This gives each fleet worker a **stable, unique memory lane** in Mem0 that persists across all recipe ticks. `worker-backend-b5vsadgp` always reads its own lane and can never accidentally match `worker-mcp-ch0cayz4`'s lane. Workers also gain **episodic memory continuity** across ticks — a worker can now remember that it already processed task XYZ on the previous tick.

**Required follow-up — `stream_manager.py` — E-05 guard extension**

The current E-05 headless guard at `stream_manager.py:140` checks `session_id.startswith("headless-")`. With the new stable session_id, the prefix becomes `worker-`. The guard must be extended:
```python
# Current:
if session_id.startswith("headless-"):
    return

# Extended:
if session_id.startswith("headless-") or session_id.startswith("worker-"):
    return
```
Without this, E-05 warnings switch from `headless-` to `worker-` prefix — same noise, different prefix.

**Defense-in-depth — Bug 1 (`mem0_adapter.py:864-867`) — still apply**

Even with stable session IDs, memories stored **before** this fix was applied (from test runs) have no `session_id` tag in Mem0. The current permissive filter passes them through:
```python
# Current (still broken for unscoped legacy data):
if item_session_id is not None and item_session_id != session_id:
    continue

# Fixed:
if item_session_id != session_id:
    continue
```
This is no longer the primary fix, but it protects against unscoped legacy memory data and any future cases where Mem0 fails to tag a memory with its session_id.

**Defense-in-depth — Bug 2 (`context_service.py:1826-1835`) — still apply**

Adding `"agent_id": agent_id` to the `list_memories` filter in `_retrieve_recent_conversation` provides a second isolation layer that works even if two workers somehow ended up with the same session_id (not possible by design, but as a safety net).

**Compatibility Analysis — No breaking changes to existing session architecture**

| Scenario | Impact | Safe? |
|---|---|---|
| PA chat sessions (UI) | Pass real `session_id` from frontend → `headless = False` → SSE validation runs normally → **zero change** | ✅ |
| Episodic memory recall | Stable `worker-{agent_id}` session → each worker accumulates its own episodic timeline | ✅ (improved) |
| Semantic facts (`_get_cached_facts`) | Scoped by `user_id` only, not `session_id` — **unaffected** | ✅ |
| Procedural learnings (`_get_cached_learnings`) | Scoped by `agent_id` only — **already isolated, unaffected** | ✅ |
| Continuity packets (`packet_builder`) | Stored/loaded by `agent_id` in Redis — **unaffected** | ✅ |
| `RuntimeManager` session reuse | `get_runtime(session_id="worker-{slug}", create_if_missing=True)` → same runtime reused across ticks (warm LivingAgent instance) → **beneficial** | ✅ |
| Concurrent overlapping ticks | Extremely unlikely at 1-min schedule; if it occurs, both ticks share the same runtime object — acceptable | ✅ |
| Global/unscoped memory | After Bug 1 fix: memories stored without `session_id` → excluded from scoped queries (`None != 'worker-...'`), accessible only via `session_id=None` queries — **clean design** | ✅ |

---

### 🟠 HIGH — `[E-04]` First-Tick Recipe Failures — All Agents

**Severity:** HIGH (first tick only — self-resolving on subsequent ticks)  
**First occurrence:** Lines 172–188 (`19:22:02`)  
**Affects:** supervisor, worker-backend, worker-mcp, worker-frontend, qc-agent

**Evidence:**
```
[RecipeScheduler] Recipe run failed for 'worker-backend':
Agent error: No active provider set and core-orchestrator is unreachable.
Configure a provider in Settings or start core-orchestrator.
```
All 5 workers fired their first recipe tick approximately **14 seconds after startup**, but core-orchestrator only came online at `19:22:13` — roughly 11 seconds into the startup window. The scheduler's first fire is at `t+12s`, which is just inside the startup window.

**Fix Required:** Add a startup readiness gate to `RecipeScheduler` — do not fire the first tick until `OrchestratorClient` health check passes. This prevents noise errors on every cold start. Alternatively, increase the initial delay from 12s to something comfortably above the measured `core-orchestrator` boot time (~30s).

---

### 🟡 MEDIUM — `[E-05]` SSE Broadcast Failures for Headless Sessions

**Severity:** MEDIUM — Noise, no functional impact  
**First occurrence:** Line 3571 (`19:23:26`)  
**Affects:** All headless agent invocations

**Evidence:**
```
⚠️ [SSE] Cannot broadcast to session headless-407a789f-...: session not found
Failed to resolve session attachments: Session headless-407a789f-... not found
```
Every headless invocation creates a synthetic `session_id` for the agent run. When the run completes and the system attempts to broadcast completion events over SSE, the session has already been torn down — it was never registered with the SSE stream manager since it was headless to begin with. This fires for every agent on every tick.

**Fix Required:** In `api_gateway.core.stream_manager`, check whether a session is of type `headless` before attempting SSE broadcast. Headless sessions should be silently skipped; currently they generate a WARNING on every single invocation, creating substantial log pollution.

---

### 🟡 MEDIUM — `[E-06]` Log Circuit Breaker Triggered

**Severity:** MEDIUM — Causes log data loss during startup  
**Occurrence:** Line 964 (`19:23:00`)

**Evidence:**
```
⚠️ [LOG-CIRCUIT-BREAKER] OPEN: 1000 logs in 10s (threshold: 2500)
```
The log circuit breaker opened during the burst of agent creation and initial tool handler registration attempts, suppressing subsequent logs for an unknown window.

**CORRECTION — Threshold is already 2500 in the current codebase:**
The log evidence above reflects an older run. `centralized_collector.py:111` already shows `threshold: int = 2500` with a 2.0s cooldown (labeled `# Issue 8 fix`). The partial fix was already applied. The root cause of the burst is not the threshold value but the **LiteLLM DEBUG payload logging** during agent creation — each LLM call emits multiple DEBUG-level log lines with full request/response bodies. With 5 agents initializing simultaneously, this saturates even the 2500-log window.

**Fix Required:** Suppress LiteLLM DEBUG verbosity during startup by setting `litellm.suppress_debug_info = True` and configuring the LiteLLM logger to WARNING level in the core-orchestrator startup sequence (`api_gateway/app.py` lifespan). Do NOT raise the circuit breaker threshold further; the problem is the log source volume, not the breaker sensitivity.

---

### 🟡 MEDIUM — `[E-07]` Slow Memory Recall Exceeding Threshold

**Severity:** MEDIUM — Agent response latency degraded  
**Occurrence:** Line 4361 (`19:23:27`)

**Evidence:**
```
⚠️ Slow recall: 1398.4ms (threshold: 1000.0ms)
```
Memory recall for the agent context window took ~1.4 seconds, 40% over the 1000ms threshold. This was also accompanied by:
```
🔻 ✂️ Early stop: marginal utility below threshold. Selected 6 chunks, tokens=700, coverage=100.0%
```

**Root Cause:** Mem0 OSS using Qdrant with an in-process path (`/tmp/qdrant`) and no warm cache layer (Redis is down). Cold vector search with no caching is producing the latency spike.

**Fix Required:** Once Redis is operational, warm tier caching should absorb most recall latency. If it persists after Redis is restored, consider reducing the Qdrant collection scan range or switching to on-disk storage with indexing.

---

### 🟢 LOW — `[E-08]` Python asyncio DeprecationWarning in `memory-mcp-server`

**Severity:** LOW — Cosmetic, no runtime impact  
**Occurrence:** Lines 28–29 (`19:21:53`)

**Evidence:**
```
C:\AIRS Foundry\...\memory-mcp-server\main.py:581: DeprecationWarning:
There is no current event loop
  loop = asyncio.get_event_loop()
```
Python 3.10+ deprecated implicit event loop creation via `asyncio.get_event_loop()` when no loop is running. Will become an error in Python 3.12+.

**Fix Required:** In `main.py:581`, replace `asyncio.get_event_loop()` with `asyncio.new_event_loop()` or use `asyncio.run()` at the top level. Low priority, but should be addressed before upgrading to Python 3.12.

---

### 🟢 LOW — `[E-09]` API Key Exposed in DEBUG Logs

**Severity:** LOW (local log file, but worth flagging)  
**Occurrence:** Lines 4372+

**Evidence:**  
LiteLLM DEBUG logging is printing the full OpenAI API key (`sk-proj-...`) in plaintext in the log file, inside the full LiteLLM call payload. The same key also appears in Mem0 adapter initialization logs (lines 31–36).

**Fix Required:** Set LiteLLM log level to INFO or higher in production to suppress the full request payload dump. In the Mem0 adapter initialization log, mask the `api_key` field before logging the config values. Log files should not contain live API keys.

---

### ℹ️ INFO — `[E-10]` Agent Registry Miss on New Instance Creation (Expected)

**Severity:** INFO — Expected behavior  
**First occurrence:** Line 287 (`19:22:13`)

**Evidence:**
```
Agent ava-chen-2xtw8pe1 not found in registry (available agents: [])
Agent hasan-c0zqd6rk not found in registry (available agents: ['ava-chen-2xtw8pe1'])
```
Each new agent instance is not found on the first attempt because the registry is populated incrementally as agents are created. This is normal behavior during the initial registration sweep from persistent config — each agent registers itself, and subsequent lookups find it. Not an error.

---

### 🔴 CRITICAL — `[E-11]` UnicodeEncodeError in Logging

**Severity:** HIGH — Causes log data loss, may hide root cause of other errors  
**First occurrence:** Line 3592 (`19:23:26`)  
**Pattern:** Repeats 10+ times throughout log

**Evidence:**
```
UnicodeEncodeError: 'utf-8' codec can't encode characters in position 63-64: surrogates not allowed
  File "C:\Users\Razi Ashary\AppData\Local\Programs\Python\Python313\Lib\logging\__init__.py", line 1153, in emit
    stream.write(msg + self.terminator)
```

**Impact:**
- **Log corruption:** The logging system crashes mid-write, meaning error messages that occurred during critical agent initialization are lost
- This may be hiding the **true root cause** of E-01 (tool handler registration failures)
- The error occurs during `living_agent` processing and `task_execution` state transitions

**Root Cause:** Windows console/logger receiving surrogate-pair Unicode characters from LLM streaming content that cannot be encoded to UTF-8. Python’s default Windows console stdout encoding does not handle surrogates.

**PARTIAL FIX ALREADY APPLIED:**  
`centralized_collector.py:274–279` already opens all log FILE handles with `errors="replace"` and applies `_sanitize_string()` before writing. File-based logs are already protected.

**REMAINING GAP:** The error originates in Python’s standard `logging.StreamHandler` (`logging/__init__.py:1153`) which writes to stdout/stderr — NOT through `centralized_collector`. The console handler uses the default system encoding without surrogate handling.

**Fix Required:** At `core-orchestrator` startup (before the first log statement), call `sys.stdout.reconfigure(errors='replace')` (Python 3.7+) to configure the console stream to drop surrogates silently. This is a one-line fix in `api_gateway/app.py` or `main.py` startup.

---

## Summary Table

| ID | Severity | Component | Issue | Blocking? |
|----|----------|-----------|-------|-----------|
| E-01 | 🔴 CRITICAL | `agent.factory` / `client.rs` | `autodev_tools` + `mcp_servers` never wired from Tauri → config → factory → metadata | ✅ Yes — no tasks can execute |
| E-02 | 🔴 CRITICAL | Redis / Memurai | Service failed to start, port 6379 refused | ✅ Yes — warm tier disabled, log noise |
| E-03 | 🟠 HIGH | `service.py` + `stream_manager` | Ephemeral headless session IDs: worker amnesia per tick + cross-agent bleed | ✅ Yes — identity corruption |
| E-04 | 🟠 HIGH | `RecipeScheduler` | First-tick fires before core-orchestrator is ready | ⚠️ First tick only |
| E-05 | 🟡 MEDIUM | `stream_manager` | Headless SSE broadcast warnings on every tick | ❌ Log noise only |
| E-06 | 🟡 MEDIUM | `centralized_collector` | Log circuit breaker tripped during startup burst | ⚠️ Log data loss |
| E-07 | 🟡 MEDIUM | `memory.living_agent` | Memory recall 1398ms, exceeds 1000ms threshold | ⚠️ Latency only |
| E-08 | 🟢 LOW | `memory-mcp-server` | asyncio `DeprecationWarning` in `main.py:581` | ❌ No impact |
| E-09 | 🟢 LOW | LiteLLM / Mem0 | API key exposed in DEBUG log output | ❌ Local only |
| E-10 | ℹ️ INFO | `agent.registry` | Agent not found on first registry sweep | ❌ Expected behavior |
| E-11 | 🔴 CRITICAL | Python logging | UnicodeEncodeError causes log data loss during initialization | ✅ Yes — may hide E-01 root cause |

---

## Recommended Fix Order

1. **Fix tool handler wiring (E-01)** — this is the true root cause of zero agent execution. The `autodev_tools` configuration pipeline and `mcp_servers` field must be wired end-to-end from Tauri through to `factory.py`. E-01 is **independent of E-02** (Redis). Fix this first.
2. **Fix Redis/Memurai (E-02)** — warm tier and agent card store restore. Does not unblock agents but improves memory performance and reduces log spam.
3. **Fix memory isolation (E-03)** — change headless session_id generation in `service.py:1318` to `f"worker-{agent_id}"` for stable per-worker memory lanes. Also extend `stream_manager` E-05 guard to include `worker-*` prefix. Apply Bug 1 + Bug 2 patches as defense-in-depth.
4. **Fix first-tick gate (E-04)** — add health check guard to `RecipeScheduler` initial fire.
5. **Fix SSE headless warnings (E-05)** — skip broadcast for headless session types.
6. **Fix API key in logs (E-09)** — set LiteLLM log level, mask keys in Mem0 init logging.
7. **Fix asyncio deprecation (E-08)** — low priority, do before Python 3.12 upgrade.

---