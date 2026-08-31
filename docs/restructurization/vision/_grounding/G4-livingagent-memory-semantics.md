# G4 — LivingAgent & Three-Tier Memory: Behavioral Specification for the Rust Port

**Source system:** `submodules/core-orchestrator` (Python 3 / FastAPI sidecar, being retired)
**Purpose:** This is the specification source for reimplementing the agent loop and memory model in Rust in-house. It documents *behavior and contracts*, not file inventory.
**Method:** Code bodies were read, not docstrings. Where a docstring and the code disagree, the **code** is reported and the discrepancy is flagged.
**Citation form:** `path:line`, paths relative to `submodules/core-orchestrator/`.

> **Reading note on provenance.** Several capabilities the Python code advertises in docstrings and class names are not wired to anything. Those are flagged inline as **DEAD** or **UNVERIFIED** and collected in the final assessment. Do not port a capability on the strength of its name.

---

# PART 1 — LivingAgent

`src/airs/agent/living_agent.py` (2424 lines)

## 1.1 The single most important structural fact

**The agentic tool-calling loop is NOT in `LivingAgent`.** `LivingAgent` runs a *linear, single-pass* orchestration; the iterative LLM↔tool loop lives one layer down, inside the LiteLLM provider:

- `src/llm/providers/litellm_provider.py:2097` — `while iteration_count < max_tool_iterations:` (streaming path)
- `src/llm/providers/litellm_provider.py:1553` — the same loop in the non-streaming `complete()` path

`LivingAgent` calls `complete_stream()` once and consumes an async chunk generator; every tool round-trip happens invisibly inside that generator. This matters enormously for the port: **the Rust design must decide whether to keep this split** (agent = orchestration, provider = loop) or hoist the loop into the agent. Recommendation is in Part 6.

## 1.2 Construction (`__init__`, `living_agent.py:177-324`)

Signature (`living_agent.py:177-191`):

```
LivingAgent(
    name: str,
    description: str,
    implementation_func: Callable,          # may be None -> default LLM path
    context_service: Any,                   # backing store for memory
    mcp_client_manager: Any,
    llm_name: str = "gpt-4o",
    model_config: Optional[Any] = None,
    prompt_template: Optional[PromptTemplate] = None,
    agent_list_provider: Optional[AgentListProvider] = None,
    redis_client: Optional[Any] = None,     # async Redis, warm tier
    metadata: Optional[AgentMetadata] = None,
    **kwargs
)
```

Construction order and what is built:

| Step | Line | What |
|---|---|---|
| 1 | `:214-220` | `metadata` defaulted to empty `AgentMetadata()`. If `instance_id` empty → assign a **ULID** ("birth certificate"). If `birth_timestamp` empty → `datetime.now(timezone.utc).isoformat()`. |
| 2 | `:225-229` | `MemoryCore(context_service, agent_id=name, redis_client=redis_client)` — **memory is constructed unconditionally, before anything else.** |
| 3 | `:236` | `UnifiedToolHandler()` — a name→handler router. |
| 4 | `:239-247` | `mcp_bridge`, `memory_compiler`, `adobe_compiler`, `internal_bridge`, `tool_executor` all set to `None`; they are built later in async `initialize()`. |
| 5 | `:250-256` | `MemoryAwareFailureDetector(memory_core)` then `UnifiedDecisionEngine(memory_core, failure_detector)`. **Both are constructed and neither is used on the default execution path** — see §1.12. |
| 6 | `:259-261` | `ThoughtProcessor(memory_core)`. |
| 7 | `:268-276` | `AgentMetadataManager(name, initial_metadata)`, `AgentActivityLogger(name)`. |
| 8 | `:279` | `current_task_fsm = None` (created per task). |
| 9 | `:285-296` | If `llm_name` truthy → `LiteLLMProvider(llm_name, model_config, **kwargs)` and `enable_llm = True`; else provider `None`, `enable_llm = False`. |
| 10 | `:299-307` | `AIRSClientManager()`, A2A `AgentCard`, `InMemoryTaskStore()`, `LivingAgentExecutor(self)`, and `_setup_a2a()` which builds a **full A2A Starlette application per agent instance** (`:351-361`). |
| 11 | `:311-317` | Best-effort self-registration into the process-global `AgentRegistry`. |

**Port note:** steps 10 and 11 are sidecar-shaped. An in-process Rust agent needs neither a per-agent HTTP app nor a global mutable registry singleton.

### Async `initialize()` (`living_agent.py:363-374`)

Ordered, and **all failures are swallowed** (each sub-step is individually try/except'd, `:466`, `:501`, `:558`, `:601`, `:634`):
1. `client_manager.initialize()` (A2A)
2. `_initialize_agent_bridge()` — `AgentBridge(agent_instance=self)`
3. `_initialize_bridges_with_unified_handler()` — tool discovery, below

**Flag:** an agent whose entire tool discovery failed still reports initialized and will happily run with zero tools. There is no readiness gate. (`:634-635` logs the error and returns normally.)

## 1.3 Tool registration and description to the model

`_initialize_bridges_with_unified_handler()` (`living_agent.py:405-635`).

**Server categorization is by substring match on the server name** (`:423-425`):
```python
memory_servers = [s for s in mcp_servers if "memory" in s.lower()]
adobe_servers  = [s for s in mcp_servers if "adobe"  in s.lower()]
other_servers  = [s for s in mcp_servers if s not in memory_servers and s not in adobe_servers]
```
This is a string-sniffing dispatch, not a declared capability. Flagged as fragile.

Four discovery paths, each with a hard timeout:

| Path | Line | Timeout | Handler registered as |
|---|---|---|---|
| Memory compiler | `:445-448` | **60.0 s** | `register_memory_handler`, tool names built as `f"{tool}_{server}"` for each tool in `MemoryCompiler.ALLOWED_TOOLS` × each memory server (`:454-458`) |
| Adobe compiler | `:483-486` | **60.0 s** | `register_adobe_handler`, single tool name `"adobe_interface"` (`:493`) |
| Generic MCP | `:523-526` | **45.0 s** | `register_mcp_handler` with actual discovered names (`:532`) |
| Internal tools | `:573` (sync) | none | `register_internal_handler`, names carry a `__internal` suffix (`:582-588`) |

Internal tools are **opt-in per agent** via agent YAML `internal_tools: [{id: web_search}]` (`:563`, `:565`).

`add_tools()` (`living_agent.py:641-712`) — validation and provider configuration:
- Accepts only objects with `type == "function"` and a `function` dict containing `name` (`:675-685`). Anything else is warned and dropped.
- On any non-empty valid set (`:699-704`):
  - `model_config.tool_choice = "auto"`
  - `model_config.parallel_tool_calls = True`  ← **parallel tool calls are always enabled when any tool exists**; there is no per-agent switch.
- Exceptions are caught and **not re-raised** (`:711-713`, explicit comment "Don't re-raise to prevent initialization failure").

Tools reach the model verbatim as the accumulated `model_config.tools` list (`_aggregate_tools_for_call`, `:715-731`) — LivingAgent does no filtering, ranking, or budgeting of tool schemas. **There is no tool-count cap and no token accounting for tool schemas anywhere.**

## 1.4 The lifecycle, precisely

`process_message()` (`:845-923`) is a thin wrapper: it records `start_time`, extracts multimodal content, delegates to `_process_message_internal`, and on exception transitions the task FSM to FAILED, emits `AGENT_INVOCATION_FAILED`, and **re-raises** (`:923`).

`_process_message_internal()` (`:925-1267`) — the actual sequence:

```
1.  TaskExecutionFSM created per task            :946-951
      task_description = message_text[:200]
    FSM.assign()  → CREATED → PLANNING           :954
2.  emit AGENT_INVOCATION_REQUESTED              :965
3.  StructuredResponseBuilder created            :997
    emit AGENT_INVOCATION_STARTED,
         AGENT_INSTANCE_ACTIVE                   :1007, :1019
4.  RECALL
      retrieval_mode, kwargs = _detect_retrieval_intent(text)   :1036
      consciousness, degrade_msg =
        memory.recall_with_budget(...)                          :1041-1051
5.  THINK
      enhanced_prompt = _build_conscious_prompt(...)            :1082
6.  FSM.plan(subtask="llm_execution") → EXECUTING              :1093
7.  EXECUTE
      response, clarifications, message_ulid =
        _execute_with_consciousness(...)                        :1101
8.  FSM.complete_step() → REVIEWING                             :1118
9.  REMEMBER
      memory.remember(Interaction(...))                         :1161-1177
      + one remember() per accumulated clarification            :1183-1201
10. fallback thinking_end emission if broker still "thinking"   :1208-1218
11. emit AGENT_INVOCATION_COMPLETED, AGENT_INSTANCE_IDLE        :1226, :1240
12. FSM.complete(result=response_text[:500]) → COMPLETED        :1256
    current_task_fsm = None
13. return response (an A2A Message)
```

**There is exactly one LLM turn per `process_message` call from LivingAgent's point of view.** Multi-step tool work is entirely absorbed inside step 7.

**Bug flagged:** `start_time` is captured at `:878` in `process_message`, then **shadowed and reset** at `:982` inside `_process_message_internal`. The `duration_ms` reported on the success path (`:1225`) therefore excludes the FSM setup and event emission before `:982`, while the failure path (`:906`) uses the outer one. The two duration figures are not comparable.

## 1.5 System-prompt assembly — what goes in, in what order

There are **two distinct prompt layers**, assembled in different places. Both must be reproduced.

### Layer A — the system message (static per agent, rendered per turn)

Built by `src/airs/config/extended_prompts_loader.py:355-431` at agent-construction time, then stored as a `PromptTemplate` and re-rendered per invocation at `living_agent.py:1574-1584`.

Assembly algorithm (`extended_prompts_loader.py:378-431`):
1. Select prompts matching the agent's **tags**. Empty `tags: []` in the YAML means *all agents*; otherwise include if the agent has **any** matching tag (`:359-361`).
2. `prepend` prompts, **sorted ascending by `priority`** (`:378-381`).
3. The agent's own system prompt (`:400`).
4. `append` prompts, **sorted ascending by `priority`** (`:383-386`, `:403-408`).
5. Dynamic prompts supplied by the caller, already resolved — sourced from **Redis** (`:411-429`, event `PROMPT_CONTEXT_LOADED` with `source="redis"`).
6. Joined with the literal separator **`"\n\n---\n\n"`** (`:431`).

Variable substitution is naive string replace of `{var}` (`:396`, and `src/airs/prompts/template.py:63-72`) — **not** a real template engine; no escaping, no error on missing variable (missing vars are silently left as literal `{var}`).

The catalogue lives in `config/extended_prompts.yaml` (499 lines). Sections, in priority order:

| name | position | priority | tags |
|---|---|---|---|
| `agent_identity` | prepend | 0 | `[]` (all) |
| `minimal_environmental_context` | prepend | 1 | `[]` |
| `base_guidelines` | prepend | 1 | `[]` |
| `safety_guidelines` | prepend | 2 | `[]` |
| `memory_awareness` | prepend | 3 | `[memory_core]` |
| `agent_to_agent_communication` | append | 4 | `[a2a_communication]` |
| `system_api_documentation` | append | 6 | `[system_access]` |
| `orchestration_guidelines` | append | 7 | `[orchestration]` |
| `system_monitoring` | append | 7 | `[orchestration]` |
| `human_interaction_guidelines` | append | 8 | `[human_input]` |
| `tool_usage_guidelines` | append | 9 | `[mcp_tools]` |
| `adobe_rules` | append | 9 | `["tools:mcp-adobe"]` |
| `output_format` | append | 10 | `[]` |

Identity block, verbatim (`config/extended_prompts.yaml:22-30`) — this is the load-bearing persona anchor:

> ```
> ## Who You Are
>
> **Name:** {agent_name}
> **Role:** {job_title}
> **Gender:** {gender}
> **Your Purpose:** {agent_description}
> **Birth Certificate:** {instance_id}
> **Born:** {birth_timestamp}
> ```

Only `agent_list` is re-rendered per invocation (`living_agent.py:1581-1583`); the identity fields are baked in at creation time (comment at `:1578-1580`). `agent_list` excludes the agent itself (`:1576`).

### Layer B — thinking instructions, appended to the system message

`ThoughtProcessor.inject_instructions()` (`src/airs/execution/thought_processor.py:116-157`), called at `living_agent.py:1734` **after** the system message is built. It appends to `messages[0]["content"]` if role is system, else inserts a new system message (`:147-155`).

Mode selection is driven by `blocks_mode` (see §1.9):
- `reasoning` / `thinking` → `THINKING_INSTRUCTIONS_NATIVE` (`thought_processor.py:33-58`), five memory-guided questions, **no XML format demanded**.
- `xml` → `THINKING_INSTRUCTIONS_XML` (`thought_processor.py:61-105`), the same five questions **plus** a hard format contract:

> ```
> ⚠️ IMPORTANT: You MUST use BOTH <thinking> and <content> blocks in your response.
> ...
> CRITICAL: Do NOT put your final answer outside the <content> block. The user will only see what's inside <content>.
> ```

### Layer C — the user message (`_build_conscious_prompt`, `living_agent.py:1312-1485`)

Memory is injected into the **user message**, not the system message. Fixed section order:

```
## Current Time: {%A, %B %d, %Y at %H:%M:%S UTC}      :1336-1338
## <experiences header, mode-dependent>                :1345-1359
   ...at most 5 experiences                            :1360
## Facts & Knowledge:                                  :1418
   ...at most 5 facts                                  :1419
## Lessons & Experience:                               :1427
   ...ALL learnings, no cap                             :1428
## Current Request:
{message_text}                                          :1440-1441
```
Joined with `"\n".join(parts)` (`:1443`).

The experiences header and per-item format vary by retrieval mode (`:1345-1396`):

| mode | header | item format |
|---|---|---|
| `timeline` | `## Chronological Record:` | `- [{relative_time}] {content}` |
| `verbatim` | `## Exact Evidence (word-for-word):` | `> "{content}"` then `  _(said {relative_time})_` |
| `hybrid` | `## Recent Context:` | `- ({relative_time}): {content}` |

Learnings are prefixed by subtype (`:1430-1436`): `⚠️  ` if `metadata.subtype ∈ {"tool_failure", "user_correction"}`, else `• `.

**Cross-agent persona-leak guard (`:1398-1413`)** — important behavior:
```python
exp_agent_id = exp.get("agent_id") or exp.get("metadata", {}).get("agent_id", "")
is_own_experience = exp_agent_id == self.name or exp_agent_id == getattr(self, 'agent_id', None)
if thinking and is_own_experience:
    parts.append(f"  💭 **My Reasoning**: {thinking}")
```
A recalled experience's `thinking` block is injected **only if the experience belongs to this agent**. Otherwise it is dropped, to stop Agent A adopting Agent B's first-person reasoning. **This guard must be ported.**

### Budgeting / truncation of the prompt

There is **no truncation in `_build_conscious_prompt`.** Budgeting happens *upstream* inside `memory.recall_with_budget()` (`:1041`); `_build_conscious_prompt` then applies its own hard caps of **top-5 experiences** and **top-5 facts** (`:1360`, `:1419`) on whatever survived. Learnings are uncapped.

Token counting in this file is a **character heuristic only** — `estimated_tokens = len(final_prompt) // 4` (`:1449`) — and it is used **for logging only**, never to gate anything. Do not mistake it for a budget enforcement point.

## 1.6 Retrieval-intent detection (`_detect_retrieval_intent`, `living_agent.py:775-843`)

Returns `(mode, kwargs)` with three modes. Keywords come from `config/memory_config.yaml` via `get_intent_keywords()`.

Normalization pipeline (`:794-807`):
1. Strip `@mentions`: `re.sub(r"@[\w\-_]+\s*", "", s)`
2. Lowercase, strip
3. Strip punctuation: `re.sub(r"[\.,!?:;\-–—’‘\"']+", "", s)`
4. Collapse whitespace

Matching rules, in order:
1. **timeline** — normalized query must be an **exact member** of the timeline keyword set (`:816`). Sort direction: `asc = True` if any of `first`, `oldest`, `earliest`, `beginning` appear in the query (`:818`), else the config default.
2. **recap** — **substring** containment of any recap keyword (`:827-834`). Routes to `timeline` mode with `timeline_asc = False` (newest first).
3. **verbatim** — normalized query must be an **exact member** of the verbatim set (`:837`); passes `verbatim_phrase = query` (the *original*, un-normalized query).
4. Default → **hybrid**, no kwargs.

**Flag:** exact-match gating on modes 1 and 3 means the specialized retrieval paths fire only for a small fixed phrasebook. Any paraphrase falls to hybrid. This makes timeline/verbatim modes nearly unreachable in practice — treat their value as UNVERIFIED before porting.

## 1.7 The execute step (`_execute_with_consciousness`, `living_agent.py:1524-2050`)

Two paths.

### Path A — custom `implementation_func` (`:2006-2050`)
Called with a fixed keyword set (`:2009-2022`): `message`, `message_text` (the *enhanced* prompt), `task_id`, `context`, `agent`, `call_llm`, `tool_executor`, `decision_engine`, `thought_processor`, `user_id`, `session_id`, `agent_id`. Handles both sync and async. Returns `(message, [], None)` — **no streaming, no clarifications, no message ULID**.

### Path B — default LLM path (`:1556-2004`), the one config-based agents use

1. **Guard** (`:1557-1560`): no implementation and no LLM → returns a plain error message.
2. **System message** rendered from template + agent list (`:1574-1584`); final prompt saved to disk for observability via `structured_logger.save_final_prompt` (`:1591-1604`).
3. **User message**, multimodal if `original_message.metadata['file_attachments']` is non-empty (`:1609`):
   - Always: a text block containing the enhanced prompt, then a literal appended section (`:1638`):
     `"\n\n**Attached Files (use these URLs for tool operations):**\n"` + one `- **{filename}** ({content_type}): \`{url}\`` per attachment.
     Rationale in the code comment (`:1622-1627`): vision lets the model *see* an image but does not give it the URL string needed for a tool call.
   - PDFs → `{"type": "file", "file": {"file_id": url}}` **only if** `provider.supports_files()` (`:1647-1656`).
   - Images → `{"type": "image_url", "image_url": {"url": url}}` **only if** `provider.supports_vision()` (`:1657-1666`).
   - `.psd` → URL in text only (`:1667-1669`).
   - Text-like files (MIME/extension allowlist at `:1498-1521`) → **downloaded over HTTP with `httpx`, 30 s timeout** (`:1674-1675`) and inlined as
     `"\n\n--- File: {filename} ---\n{file_content}\n--- End of {filename} ---"` (`:1682`).
     **No size cap on the inlined file.** A large text file goes into the prompt whole. Flagged.
4. **`blocks_mode` detection** (`:1697-1731`) — see §1.9.
5. **Thinking instructions injected** (`:1734`).
6. **Streaming call** `call_llm_streaming(...)` (`:1768-1774`).
7. **A `message_ulid` is generated once per agent turn** (`:1784`) and threaded through both SSE and memory storage so the frontend can de-duplicate on hydration.
8. `StreamBroker` constructed (`:1785-1793`) with `blocks_mode`, the SSE broker, session/agent ids, `calling_agent_id` (non-null ⇒ this is an A2A call), and `message_id`.
9. **Chunk loop** (`:1796-1812`): every chunk → `stream_broker.process_chunk(chunk)`. If a chunk carries `tool_call_history`, each entry is flattened into `accumulated_tool_calls` with keys `tool_name`, `tool_call_id` (from `tool_id`), `arguments`, `result`, `success`, `error`, `iteration`.
10. `stream_broker.finalize()` (`:1815`), then `get_accumulated_response()` → `{full_response, thinking, content, usage}`.
11. **Thinking extraction** `thought_processor.process_response(full_response, session_id)` (`:1830-1833`) — extracts `<thinking>…</thinking>`, strips `<content>` tags and `<storing_memory>` blocks, and **writes the thinking to procedural memory** (`thought_processor.py:275-278`).
12. **Delegation parsing** (`:1839-1903`): `DelegationHandler(max_call_depth=3)` (`:1844`). Parsed `DELEGATE` intents are executed, and `actual_response` is replaced by `delegation_result['modified_response']` **whether or not** the delegations succeeded (`:1868-1880`). Successful delegations are recorded as A2A messages on the response builder.
13. **Response assembly** (`:1905-1952`): tool-narration sanitization, thinking added (visibility from `Settings.SHOW_THINKING_IN_UI`, default `True` at `:1922`), tool calls added (visibility from `Settings.SHOW_TOOL_CALLS_IN_UI`, default `False` at `:1927`), metadata set with `message_ulid` and usage, then `.build()`.
14. `agent_stop` emitted with a metadata payload carrying `internal_operations` counts and the full structured response (`:1959-1982`). **`memory_recalls` is hardcoded to `0` with a TODO (`:1969`).**
15. Returns `(message, accumulated_clarifications, message_ulid)`; the A2A message carries `metadata['thinking']` and `metadata['structured']` (`:1989-1990`).

**Flag:** `accumulated_clarifications` is initialized to `[]` at `:1568` and **never appended to** anywhere in this file. The clarification-storage block at `:1182-1202` is therefore unreachable on this path. Either the human-input path writes to it elsewhere (UNVERIFIED — no writer found in `living_agent.py`) or this is dead.

**Flag:** `_thinking_content`, `_in_thinking_block`, `_in_memory_block`, `_stream_usage`, `stream_model_name` (`:1753-1765`) are initialized and never meaningfully used; `stream_model_name` stays `None` and the ternary at `:1947` therefore always resolves to the provider model name.

## 1.8 The real tool loop (`litellm_provider.py`)

`complete_stream(messages, max_tool_iterations=25, session_context=None, **kwargs)` (`:2053-2058`).

```
formatted_messages = await _prepare_messages(messages.copy())        :2085
iteration_count = 0
while iteration_count < max_tool_iterations:                          :2097
    iteration_count += 1
    params = _prepare_params(formatted_messages, **kwargs); params["stream"]=True
    response = await acompletion(**params)                            :2111
    <consume stream, accumulate content / thinking blocks / tool_calls>
    if no tool_calls:
        append {"role":"assistant","content":accumulated_content}; BREAK   :2709-2717
    if tool_calls but none valid after filter:
        append assistant message; BREAK                                :2700-2708
    append assistant message (provider-shaped, see below)              :2540
    yield chunk: "\n\n[Executing {n} tool(s)...]\n\n"                  :2543-2547
    for each tool_call:                    # SEQUENTIAL, not parallel  :2550
        parse args (JSON; on failure -> {"error":"Invalid JSON in arguments"})  :2556-2558
        if session_context.tool_executor:
            tool_executor.execute(tool_name, arguments, user_id, session_id, response_builder)  :2580
        else:
            self.tool_handler(tool_call_dict)                          :2618
        record into tool_call_history (see schema below)
        yield chunk: "[{name} completed]" or "[{name} failed: {err}]"
        append tool-result message (provider-shaped)                   :2680
    yield chunk(thinking_resumption=True, resumption_reason="tool_results")  :2688-2694
    continue
# after loop
if tool_calls_executed or iteration_count >= max_tool_iterations:      :2720
    yield final chunk(is_final=True, tool_call_history=...)
```

**Key numbers and behaviors:**

- **`max_tool_iterations` default = 25** (`:2056`, and `:1517` for `complete()`). **LivingAgent never overrides it** — `call_llm_streaming` (`living_agent.py:2146`) passes no such kwarg. So 25 is the effective cap.
- **`parallel_tool_calls=True` is sent to the provider** (`living_agent.py:702`) so the *model* may emit several tool calls per turn, but **the executor runs them strictly sequentially in a `for` loop** (`:2550`). There is no concurrency. The `parallel_tool_calls` flag only affects what the model is allowed to request.
- **On limit hit there is no distinct signal.** `:2720` uses `or`, so the final chunk is identical whether the loop ended by natural break or by exhausting 25 iterations. The model is never told it was cut off, and no error is surfaced. **This is a real defect to fix in the port.**
- **Tool errors are non-fatal and are fed back to the model.** A raised exception becomes `serialized_result = {"error": str(tool_error)}` (`:2641`), a history entry with `success: False`, a visible `[{name} failed: …]` chunk, and a normal tool-result message. The loop continues.
- **An error inside the outer per-tool-call try (e.g. malformed tool_call structure) is `continue`d and silently skipped** (`:2683-2685`) — the model gets **no** tool-result message for that call, which can leave an unanswered `tool_use` block. Flagged as a correctness hazard on strict providers.

`tool_call_history` entry schema (`:2607-2614`, `:2644-2652`):
```
{ tool_name, arguments, result, success: bool, tool_id, iteration, error? }
```

### Provider-specific branching in result formatting

Two shapes, selected at `:2664`:

```python
if (anthropic_thinking or model_config.block == 'reasoning') and supports_structured_blocks_stream:
    {"role": "user", "content": f"Tool result for {name}: {json.dumps(result, indent=2)}"}
else:
    {"tool_call_id": tool_id, "role": "tool", "name": name, "content": tool_content}
```
i.e. **Anthropic-thinking and Google-reasoning models receive tool results as plain `user` messages**, not as `role: "tool"`. Assistant messages are correspondingly built via `_build_anthropic_assistant_content` with structured thinking blocks when applicable (`:2518-2531`).

Other provider branches:
- **Anthropic image-size recovery** (`:2117-2159`): `ANTHROPIC_MAX_IMAGE_SIZE_MB = 5.0` (`:219`). On a size error, oversized images/base64 files are extracted and **replaced with GPT-4.1-generated textual descriptions**, then the call is retried. This is a hard cross-provider dependency (Anthropic calls fall back onto OpenAI).
- **Anthropic native thinking** (`:1056-1066`): when `blocks_mode == 'thinking'` and the model name contains `anthropic`/`claude` → `params['thinking'] = {"type": "enabled", "budget_tokens": budget}` where `budget = int(os.getenv('ANTHROPIC_THINKING_BUDGET', '2048'))`, and **`temperature` is forced to `1`** (`:1060`).
- **Empty tools array** (`:1030-1037`): sent for all providers *except* `sealion` and `groq`, which misinterpret `[]` as "enable tool-calling mode".
- **`max_tokens` is immutable** — explicitly filtered out of kwargs so callers cannot override the model config (`:1042-1051`).

### Context-window management — the honest answer

- `trim_formatted_messages(messages, max_total_length=12_000)` exists at `:1480-1512` (keep first 3, keep last 15, collapse dropped `role:"tool"` messages to a 120-char summary). **It is never called.** Verified: the only occurrence of the identifier in the entire non-vendored tree is its own definition.
- The **only** live trimming is on an *error-recovery* path in the non-streaming `complete()`: `formatted_messages = formatted_messages[:3] + formatted_messages[-15:]` (`:1946-1947`). The streaming path — the one LivingAgent actually uses — has **no trimming at all**.
- There is no summarization and no compaction of the working conversation. Overflow protection relies entirely on `context_window_fallback` / `fallbacks` being passed to LiteLLM (`:1020-1021`), i.e. on falling back to a bigger model rather than shrinking the input.

**Conclusion for the port: the running conversation is unmanaged.** Long tool loops grow the message array monotonically until the provider refuses. The memory system's careful budgeting (Part 2) governs only the *first* user message; everything appended during the 25 iterations is unbudgeted. This is the largest single correctness gap in the system.

Model context windows are declared per model in `src/llm/providers/model_registry.py` (73 `ModelConfig` entries) — e.g. `context_window=256000` for GPT-5 family (`:77`), `128000` (`:102`), `1000000` for GPT-4.1 family (`:175`), `16385` for older chat models (`:206`), `200000` for Anthropic-class entries (`:230`). Providers present: `openai` (37), `anthropic` (10), `google` (6), `deepseek` (4), `groq` (4), `moonshot` (3), `nvidia` (3), `sealion` (3), `xai` (2), `dashscope` (1). `block` is declared as `reasoning` on 27 entries and `thinking` on 7; the rest have none and fall back to `xml`.

## 1.9 `blocks_mode` resolution (`living_agent.py:1696-1731`)

Per-invocation (deliberately, to pick up dynamic agent updates). Algorithm:
1. Take `llm_provider.model_name`.
2. Normalize by stripping any provider prefix before `/` (`:1706-1707`).
3. Scan **`MODEL_CONFIGS.values()`** matching on the config's `name` field, not the dict key (the comment at `:1702-1703` notes keys are aliases while LiteLLM uses full names).
4. Among candidates, **prefer one that declares a `block`** (`:1717-1719`).
5. `blocks_mode = config.block.lower()` if present, else `"xml"`.
6. Hard assertion: `blocks_mode in ["reasoning", "thinking", "xml"]` (`:1730-1731`) — an **`assert`**, so it vanishes under `python -O`. In Rust make this a typed enum.

## 1.10 Streaming and event emission

Streaming is a two-stage pipeline: `LiteLLMProvider.complete_stream` yields `LLMStreamChunk`s → `StreamBroker.process_chunk` classifies and buffers them → SSE broker emits to the frontend.

`src/airs/streaming/` (1708 lines total): `stream_broker.py` (648), `xml_detector.py` (309), `stream_state.py` (282), `buffer_manager.py` (232), `native_detector.py` (200).

`StreamState` is an FSM (`stream_state.py:26-32`) over phases `SEEK_THINK_START → IN_THINK → SEEK_CONTENT_START → IN_CONTENT → DONE`, with a **thinking-resumption transition `IN_CONTENT → IN_THINK`** for Anthropic extended thinking (`stream_state.py:48`). It emits `STREAM_THINKING_START`, `STREAM_THINKING_END`, `STREAM_CONTENT_START`, `STREAM_CONTENT_END` telemetry events on transition.

Buffer cadence (`buffer_manager.py:143`, `:165-171`, `:208-209`):
- Initial content buffer: `max_delay_ms = 20`, `max_size = 128` chars.
- **Adaptive relaxation:** after `adaptive_threshold_seconds = 2.0` of streaming, the buffer relaxes to `max_delay_ms = 60`, `max_size = 256`.
- Reset returns to 20 ms / 128.

Agent-level events emitted by LivingAgent (all via `emit_event_safe`, all individually swallowed on failure):
`AGENT_TOOLS_UPDATED` (`:545`), `AGENT_INVOCATION_REQUESTED` (`:966`), `AGENT_INVOCATION_STARTED` (`:1008`), `AGENT_INSTANCE_ACTIVE` (`:1020`), `AGENT_INVOCATION_COMPLETED` (`:1227`), `AGENT_INSTANCE_IDLE` (`:1241`), `AGENT_INVOCATION_FAILED` (`:908`).

SSE broker calls: `emit_thinking_end` (`:1211`), `emit_agent_stop` (`:1974`), plus `emit_tool_call_start` / `emit_tool_call_complete` from the tool executor (`tool_executor.py:349`, `:420`, `:490`).

The SSE broker import is **optional** — `try: from api_gateway.modules.stream.streams import broker / except ImportError: broker = None` (`living_agent.py:49-54`). Every emission site is guarded by `if sse_available and session_id and broker`. The agent runs correctly with streaming entirely absent.

## 1.11 Memory read/write seam

**Read** (`living_agent.py:1041-1051`):
```python
consciousness, degrade_msg = await self.memory.recall_with_budget(
    user_id, session_id, query=message_text,
    model_name=self.llm_name,
    system_prompt="",            # <-- see flag
    use_cache=True,
    response_builder=response_builder,
    retrieval_mode=retrieval_mode, **retrieval_kwargs)
```

**Flag (real bug):** `system_prompt=""` is passed with an in-code `# TODO: Add system prompt if available` (`:1046`). The token budget is computed **as though the system prompt were empty**, but the actual system prompt is the full assembled extended-prompt stack (typically thousands of tokens). **The memory budget is systematically over-generous by the size of the system prompt.** This must be fixed in the port — pass the real rendered system prompt.

`AgentConsciousness` (`src/airs/memory/memory_core.py:106-116`):
```python
experiences: List[Dict[str, Any]]   # episodic
facts:       List[Dict[str, Any]]   # semantic
learnings:   List[Dict[str, Any]]   # procedural
query:       str
```

**Write** (`living_agent.py:1161-1177`), once per turn, after the response is complete:
```python
await self.memory.remember(Interaction(...), response_builder=response_builder)
```

`Interaction` (`memory_core.py:120-142`):
```python
user_message: str
agent_response: str
user_id: str
session_id: str
agent_id: Optional[str] = None      # for A2A: which agent was involved
task_id: Optional[str] = None
thinking: Optional[str] = None
tool_calls: Optional[List[Dict[str, Any]]] = None
image_urls: Optional[List[str]] = None
file_urls: Optional[List[str]] = None
file_types: Optional[List[str]] = None
metadata: Optional[Dict[str, Any]] = None
```

Metadata written by LivingAgent (`:1146-1158`): `message_id` (= `message_ulid`, the assistant message id, used for hydration de-duplication), `user_message_id` (pulled from the inbound message metadata at `:1140-1141`), `assistant_message_id`, `has_images`, `has_files`, and `delegation_id` when the task was delegated.

There are **three additional write sites**:
1. Per clarification (`:1184-1201`) with `metadata.type = "clarification"` — currently unreachable, see §1.7.
2. Per A2A exchange (`send_message_to_agent`, `:2379-2395`) with `metadata.type = "a2a"`, storing the **target** agent's thinking and tool calls, and messages prefixed `[A2A to {agent}]` / `[A2A from {agent}]`.
3. Per tool call, inside the executor: `memory.learn_from_tool(...)` on both success and failure (`tool_executor.py:399-407`, `:467-475`).

## 1.12 Memory-aware tool execution (`src/airs/execution/tool_executor.py`, 563 lines)

`MemoryAwareToolExecutor.execute()` (`:255-533`) — the memory-learning wrapper. Three phases:

**Phase 1 — pre-flight memory check (`:280-309`), gated by `check_memory=True` (default):**
```python
past_failures = await memory.procedural.check_tool_history(tool_name, arguments)
if past_failures:
    for failure in past_failures:
        resolution = failure["metadata"].get("resolution")
        if resolution:
            return ToolResult(success=False, from_memory=True, suggested_fix=resolution)
    # else: log a warning and proceed anyway
```
**This short-circuits the tool without executing it** and returns a suggested fix instead. Powerful and dangerous: a stale learned "resolution" permanently blocks a tool that may now work. There is no recency check, no confidence threshold, and no retry-anyway escape hatch. Flagged.

**Phase 2 — execution (`:311-392`):**
- A `ToolExecutionFSM` per call with states `CREATED → CHECKING_MEMORY → PREPARING → EXECUTING → LEARNING → COMPLETE|FAILED` (`:318-322`, `:340-372`, `:395`, `:410`, `:478`).
- Registered in a process-global singleton `ToolExecutionRegistry` (`:50-141`) tracking active calls plus `completed_count`, `failed_count`, `success_rate`, `avg_duration_ms`.
- Arguments are **redacted** via `redact_args` before logging, before the registry summary (truncated to 200 chars, `:335`), before SSE emission (`:353`), and before the response builder (`:448`). The **unredacted** args are what actually reach the tool (`:379`).
- `__session_id` is injected into the arguments (`:379`) so the Rust `autodev-mcp-server` tool adapter can route checkpoint streams per agent lane. **This is a live Rust-side contract — preserve the key name.**
- Result is serialized by `serialize_mcp_result` (`:161-218`), a recursive Pydantic-v2/v1/dataclass/list/dict flattener with a 500-char string fallback on failure.

**Phase 3 — learning (`:399-407` success, `:467-475` failure):** `memory.learn_from_tool(tool_name, success, context={args, result|error}, session_id)` on **both** paths. `context` carries the **unredacted** arguments — a secret passed as a tool argument lands in procedural memory verbatim. Flagged as a security issue to fix in the port.

`execute_multiple()` (`:535-563`) is a plain sequential `for` loop; no concurrency.

## 1.13 Tool-narration sanitization (`living_agent.py:733-773`)

If `tool_calls` is non-empty the response is returned unchanged (`:746-747`). Otherwise, and only if `sanitization_config.tool_narration.enabled`, each configured regex is applied with `re.IGNORECASE` to strip bracketed narration (`:757-759`), then runs of 3+ newlines are collapsed to `max_consecutive_newlines` (`:762-763`) and the result is stripped. Purpose: remove "[Executing 3 tool(s)...]"-style text the model hallucinated when it did not actually call tools.

## 1.14 State and persistence

| Thing | Where it lives | Survives restart? |
|---|---|---|
| Episodic / semantic / procedural memory | via `context_service` → Postgres (Part 5) | **Yes** |
| Warm-tier memory cache | Redis (`redis_client` param, `:228`) | Yes, TTL-bound |
| `AgentRegistry` entries | process-global class state (`registry.py`) | **No** |
| A2A `InMemoryTaskStore` | in-process (`:304`) | **No** |
| `current_task_fsm` | instance attribute, per task, nulled on completion (`:1265`) | **No** |
| `ToolExecutionRegistry` | process-global singleton (`tool_executor.py:58`) | **No** |
| Rendered final prompts | disk, via `structured_logger.save_final_prompt` (`:1591`) | Yes (observability only) |
| Conversation message array | **local variable only**, discarded each turn (`:1571`) | **No** |

**Critical:** the conversation is not persisted as a conversation. Continuity between turns is reconstructed **entirely from the memory tiers**. There is no message-history table backing the agent loop. That is the architectural bet of this system, and the port must either keep it or replace it deliberately.

## 1.15 Termination conditions — complete list

| Condition | Where | Result |
|---|---|---|
| Model returns no tool calls | `litellm_provider.py:2709-2717` | Normal completion |
| Tool calls present but all filtered as invalid | `:2700-2708` | Normal completion |
| 25 iterations reached | `:2097` | Loop exits; **indistinguishable from normal completion** |
| Provider exception | `:2728-2735` | Yields a final chunk with `content=f"Error: {e}"`; **no raise** — the error text becomes the agent's answer |
| Exception anywhere in `_process_message_internal` | `living_agent.py:890-923` | FSM→FAILED, event emitted, **re-raised** to the caller |
| No implementation and no LLM | `:1557-1560` | Static error string returned |
| Task cancellation | `:151-158` | **Not supported** — logs a warning and does nothing |

---

## 1.16 Agent sibling modules

