# G2b — Bug Audit: Provider / Routing / Budget / Cost / Subagent / Session / Memory

Read-only audit. Every load-bearing claim carries `path:line`. Labels: CONFIRMED (I read
the body) vs SUSPECTED (inference not fully traced). All paths relative to
`C:\AIRS Foundry\autodev\autodev\src-tauri\src\`.

Status: COMPLETE.

---

## 1. Findings table

| ID | file:line | sev | class | what the code does | why it is wrong | failure scenario | status |
|----|-----------|-----|-------|--------------------|-----------------|------------------|--------|
| P-01 | `provider/` (whole dir) | **CRITICAL** | missing capability | No `cache_control`, no `ephemeral`, no `cache_creation_input_tokens`, no `cached_tokens` anywhere in `provider/`. Only hit for `cached_tokens` in the repo is `sidecar/client.rs:42,899` (the Python path being deleted). | Anthropic prompt caching is the single largest cost lever for an agent loop that resends a large system prompt + tool schemas + full history every turn. Absent entirely. | 40-turn agent session re-bills the full system prompt + tool defs at full input price every turn. At Opus input $15/1M with a 20k-token preamble that is ~$12 of avoidable spend per session (90% of it cacheable at 0.1×). | CONFIRMED |
| P-02 | `provider/openai.rs:165-186` | **CRITICAL** | dropped parameter | `build_body` never writes `max_tokens` or `max_completion_tokens`. `req.max_tokens` (`provider/types.rs:54`) is silently discarded. | The caller's output cap is ignored; OpenAI applies its own model default. Cost control and truncation control are both lost. | Agent sets `max_tokens: 1024` for a cheap classification call; OpenAI generates up to the model max instead. Budget guard sees the bill after the fact. | CONFIRMED |
| P-03 | `provider/google.rs:117-144` | **CRITICAL** | dropped parameter | `build_body` never writes `maxOutputTokens` into `generationConfig`. `req.max_tokens` discarded. | Same as P-02 for Gemini. | Same. | CONFIRMED |
| P-04 | `provider/ollama.rs:230-240` | HIGH | dropped parameter | `build_chat_request` builds `ChatMessageRequest::new(model, messages)` and returns — no tools, no `max_tokens`, no temperature, no top_p. Comment at 235-238 admits tools are "Deferred". | `stream()` at `ollama.rs:149-164` branches on `!request.tools.is_empty()`, logs "falling back to non-streaming path", calls `complete()` — which builds a request **with no tools**. The model is never told the tools exist. `parse_chat_response` (309-340) has a TODO at 322-323 and never extracts `tool_calls`. | Agent gives Ollama 12 tools; Ollama replies with prose. The agent loop sees no `ToolUse` block, concludes the model chose not to use tools, and loops or gives up. Looks like a model quality problem; it is a wiring stub. | CONFIRMED |
| P-05 | `provider/ollama.rs:168-189` | MED | inert guard / doc-vs-body | `catch_unwind(AssertUnwindSafe(\|\| self.client.send_chat_messages_stream(chat_request)))` — the closure only *constructs* the future. `.await` happens at line 175, **outside** the `catch_unwind`. | Comment claims "Wrap Ollama stream in catch_unwind to prevent panics ... from crashing the app". Any panic inside `ollama-rs` occurs during poll, i.e. outside the guard. The guard cannot fire. | Malformed `OLLAMA_HOST` causes a `RelativeUrlWithoutBase` panic during the await; the tokio task aborts and the guard's friendly error never appears. | CONFIRMED |
| P-06 | `provider/openai.rs:359-372` | **CRITICAL** | usage never reported | `MessageDelta` is emitted **only if** `v["usage"]` is present in the chunk. `build_body` (165-186) never sets `stream_options: {"include_usage": true}`. | OpenAI's Chat Completions stream omits `usage` entirely unless `include_usage` is requested. Therefore in streaming mode this branch is dead: no `MessageDelta` is ever emitted → `stop_reason` never surfaced, `tokens_in`/`tokens_out` never recorded. | Every streamed OpenAI turn records 0 input and 0 output tokens. Cost tracker and budget guard both see $0 for all OpenAI streaming traffic. Also `finish_reason == "length"` (max_tokens truncation) is never detected. | CONFIRMED |
| P-07 | `provider/openai.rs:308-374` | HIGH | parallel tool calls lost | `parse_openai_chunk` `return`s on the **first** thing it finds. Inside the `for tc in tool_calls` loop (327-356) both branches `return`. | OpenAI emits `tool_calls` as an array with `index` for parallel calls; a single delta chunk can carry more than one entry, and the *first* delta for a call carries `id` + `name` **and** the opening `arguments` fragment in one object. That fragment is dropped (the `id` branch returns first). | Parallel tool calling: model requests `read_file` and `grep` in one turn; only one `ToolUseStart` is emitted, the sibling is silently discarded. Separately, the first `arguments` fragment of every tool call is lost, so the accumulated JSON is malformed (`ile.rs"}` instead of `{"path":"file.rs"}`) → tool invocation fails to parse. | CONFIRMED |
| P-08 | `provider/google.rs:313` | **CRITICAL** | tool args discarded | `let _args = func_call.get("args").cloned().unwrap_or(json!({}));` — the function-call arguments are bound to `_args` and thrown away. The emitted `StreamChunk::ToolUseStart` carries only `id` + `name`; no `ToolInputDelta` is ever emitted for Gemini. | Comment on 315 says "Gemini sends complete function calls, not deltas" — correct observation, wrong action: the complete args should be forwarded as a `ToolInputDelta`. They are not. | Gemini streaming tool call arrives with an empty argument object. The agent calls `write_file` with no path and no content. Streaming + tools on Gemini is non-functional. | CONFIRMED |
| P-09 | `provider/google.rs:299-338` | HIGH | usage never reported (streaming) | `parse_google_chunk` returns on the first text part (301-305). Gemini's terminal SSE chunk normally carries **both** a text part and `finishReason` + `usageMetadata`. | Because the text branch returns first, the `finishReason`/`usageMetadata` block at 325-338 is unreachable for the common case. | Every streamed Gemini turn records 0 tokens; no `MessageDelta`, and no `Done` is ever emitted either (Gemini SSE has no `[DONE]` sentinel and `parse_google_sse_line:269-282` has no `[DONE]` handling). A consumer waiting for `StreamChunk::Done` waits for stream end instead — see P-10. | CONFIRMED |
| P-10 | `provider/google.rs:269-282` | HIGH | missing terminator | No `[DONE]` handling and no `message_stop` equivalent; `StreamChunk::Done` is never produced on the Google path. | `StreamChunk::Done` is a variant of the shared contract (`provider/types.rs:81`) that Anthropic (`streaming.rs:73`), OpenAI (`openai.rs:300`) and Ollama (`ollama.rs:363`) all emit. Google alone does not. | Any consumer that terminates on `Done` rather than on stream exhaustion hangs or mis-finalizes for Gemini. | CONFIRMED |
| P-11 | `provider/google.rs:85-90` | HIGH | wrong protocol field | `MessageContent::ToolResult` maps to `"functionResponse": {"name": tool_use_id, ...}`. | Gemini's `functionResponse.name` must be the **function name**, matching a prior `functionCall.name`. Passing the tool-use id makes the response unmatchable. | Multi-turn tool loop on Gemini: the model cannot correlate the result with its call; it re-issues the same call or errors 400. | CONFIRMED |
| P-12 | `provider/google.rs:317`, `google.rs:376` | MED | id collision | Tool-use id is synthesised as `format!("gemini-{name}")`. | Two parallel calls to the same tool produce identical ids; the agent's tool-result routing keys collide. | Model calls `read_file` twice in one turn; both results carry `tool_use_id: "gemini-read_file"` and one overwrites the other. | CONFIRMED |
| P-13 | `provider/google.rs:158-161`, `184-187`, `235-238` | HIGH | secret in URL | API key is interpolated into the query string: `...:generateContent?key={api_key}`. | Query strings land in proxy logs, crash dumps, and `reqwest` error `Display` output. Note `errors` wrap `AutoDevError::Http` — a connection error's `Display` includes the URL. Gemini also accepts `x-goog-api-key` as a header. | A logged transport error (`error!` at `google.rs:262`) writes the user's Gemini key to the app log file in plaintext. | CONFIRMED |
| P-14 | `provider/anthropic.rs:31`, `openai.rs:29`, `google.rs:29` | HIGH | no timeout | `Client::new()` with no `.timeout()`, no `.connect_timeout()`, no `.pool_idle_timeout()`. | `reqwest::Client::new()` has **no** default request timeout. A hung TLS handshake or a stalled SSE stream blocks the spawned task forever. | Provider stalls mid-stream (common on 429-adjacent conditions); the agent task never returns, the session shows "running" indefinitely, and the `ActiveAgent` entry in app state (`commands/agent_commands.rs:56-62`) is never cleared. | CONFIRMED |
| P-15 | `provider/` (all four) | HIGH | no retry / no backoff | Grep for `retry`, `backoff`, `Retry-After`, `429` across `provider/` returns nothing beyond a `401`/`403` check in `test_connection`. Non-2xx is converted straight to `AutoDevError::Provider(format!("... {status}: {text}"))` — `anthropic.rs:165-168`, `openai.rs:209-212`, `google.rs:171-174`, and the stream variants at `anthropic.rs:188-191`, `openai.rs:230-233`, `google.rs:196-199`. | No retry on 429, 500, 502, 503, 529. `Retry-After` is **not** read. Anthropic's `overloaded_error` (529) is routine under load. | A single 429 on turn 30 of a long agent run kills the whole run. No jitter, no backoff, no idempotency. | CONFIRMED |
| P-16 | `provider/streaming.rs:25`, `openai.rs:304`, `google.rs:280` | MED | fatal on malformed chunk | A JSON parse failure on one SSE `data:` line returns `Err`, which the `flat_map` (e.g. `anthropic.rs:215-221`) turns into a terminal `Err` item in the stream. | One corrupt or unexpectedly-shaped line aborts the whole response rather than being skipped. Also the error message embeds the raw `data` payload (`{data}`) which for a tool-result echo can contain user file contents. | A vendor adds a new event type with a payload the parser chokes on → every stream fails; and the log now contains file contents. | CONFIRMED |
| P-17 | `provider/anthropic.rs:202`, `openai.rs:243`, `google.rs:209` | MED | lock across boundary / panic path | `buffer.lock().unwrap()` on a `std::sync::Mutex` inside a stream combinator. | The `unwrap()` panics on a poisoned mutex; a poison can only arise from a prior panic while the guard is held — but the `unwrap()` is nonetheless a reachable panic in the streaming hot path. The `std::sync::Mutex` is not held across an `.await` here (the closure is sync), so this is a panic risk, not a deadlock. | Any panic in `parse_sse_line` poisons the buffer; the next chunk panics the task with an unrelated message. | CONFIRMED |
| P-18 | `provider/anthropic.rs:250-256` | MED | truncation not surfaced | `parse_completion_response` reads `stop_reason` into the struct but nothing in `provider/` inspects it. Grep for `max_tokens` as a *stop reason* value: zero comparisons anywhere. Same for `"length"` (OpenAI) and `"MAX_TOKENS"` (Gemini). | `max_tokens` truncation is a silent data-loss condition — the model's answer is cut mid-sentence and the agent treats it as a complete turn. | Model hits the cap while writing a file via a tool call; the tool-input JSON is truncated, the tool errors on parse, and the agent retries the same prompt in a loop. | CONFIRMED |
| P-19 | `provider/types.rs:5-14` | HIGH | model registry design | `ModelInfo` carries exactly two price fields, `cost_in_per_1m` and `cost_out_per_1m`. There is **no** cache-read price, no cache-write price, no reasoning-token price. | Even if caching were added, the type system cannot represent its pricing. Cache-write is 1.25× input and cache-read is 0.1× input on Anthropic; folding them into one input rate makes cost figures wrong by up to 10×. | Post-caching, a cached-heavy session is billed at full input rate in the UI — a 10× overstatement — or, if the provider reports only uncached input, a large understatement. | CONFIRMED |
| P-20 | `provider/anthropic.rs:38-63`, `openai.rs:36-70`, `google.rs:36-61`, `ollama.rs:413-488` | MED | hardcoded model ids + prices | 18 hardcoded model entries with hardcoded USD prices compiled into the binary. Full list in §4. | Prices change; models retire. A hardcoded table is stale the day it ships and requires a rebuild to fix. `google.rs:36` pins `gemini-2.0-flash-thinking-exp-01-21` — an experimental preview id. | User sees a cost estimate computed from prices that no longer exist; or a 404 from a retired model id. | CONFIRMED |
| P-21 | `provider/openai.rs:119-153` | MED | silent message drop | The message loop is `if role == "user" { } else if role == "assistant" { }` with **no `else`**. Any other role is dropped. | A `system` or `tool` role appearing inside `req.messages` (rather than in `req.system`) vanishes with no log line. | History replayed from a session with a stored `system` message loses it; the model answers without its instructions. | CONFIRMED |
| P-22 | `provider/openai.rs:87-118` | MED | mixed-content loss | When a user message contains **both** tool results and text, only the tool results are emitted (the `if !tool_results.is_empty()` branch discards the sibling text at 93-103). | The user's accompanying text is lost. | User approves a tool and adds "but use the other file" — the added instruction is discarded on the OpenAI path only. | CONFIRMED |
| P-23 | `provider/openai.rs:173-182` | MED | invalid params for o-series | `temperature` and `top_p` are always forwarded when present, alongside `reasoning_effort`. | OpenAI reasoning models (`o4-mini`, listed at `openai.rs:62-70`) reject `temperature`/`top_p` with a 400. | Selecting `o4-mini` with any temperature configured produces `400 Unsupported parameter`. | SUSPECTED (depends on caller always leaving temperature None for o-series; nothing in `provider/` enforces it) |
| P-24 | `provider/google.rs:233-252` | LOW | paid connection test | `test_connection` issues a real `generateContent` call with `"Hi"`. | Costs money and quota on every settings-panel connection test; Anthropic and OpenAI use the free `/models` endpoint instead (`anthropic.rs:229`, `openai.rs:270`). | Repeated key testing burns quota. | CONFIRMED |
| P-25 | `provider/types.rs:29-33` | HIGH | no multimodal representation | `MessageContent` has exactly three variants: `Text`, `ToolUse`, `ToolResult`. There is **no** `Image` / `Document` variant. Yet `ModelInfo.supports_vision` (`types.rs:10`) is set `true` for 9 of the 18 models. | `supports_vision` is a claim the request type cannot honour. Vision is advertised and unimplementable through this trait. | UI offers image attachment against a vision-capable model; there is no wire format to send it. | CONFIRMED |
| P-26 | `provider/types.rs:49-59` | HIGH | no structured output | `CompletionRequest` has no `response_format` / `json_schema` / `tool_choice` field. `openai.rs:171` hardcodes `tool_choice: "auto"`; Anthropic and Google set no `tool_choice` at all. | Callers cannot force a tool, forbid tools, or request JSON-schema-constrained output. Every "give me JSON" call is prompt-and-pray. | A classifier subagent asked for strict JSON returns prose; the parse fails; the caller `unwrap_or_default()`s it (see §6). | CONFIRMED |
| P-27 | `provider/anthropic.rs:80-88` | LOW | model allowlist by match | `build_body` rewrites model ids through a hardcoded `match`. Unknown ids pass through via `other => other`. | Fine, but the comment (68-79) documents a past production incident of 9 × 404s. The mapping table must be maintained by hand for every new Claude model. | A new model id ships; nothing breaks, but the historical class of bug (slug drift) recurs whenever a caller stores a proxy-format name. | CONFIRMED |
| P-28 | `provider/` — no file | **CRITICAL** | missing provider | There is **no** xAI/Grok provider. The only occurrence of "grok" in the whole `src-tauri/src` tree is `commands/worker_commands.rs:327-328`, a string mapping `model.starts_with("grok") → "xai"` used to name a *LiteLLM* provider for the Python sidecar's model registry. | One of the four post-restructure providers does not exist. | Selecting a Grok model in the UI falls through the provider factory `match` (`commands/agent_commands.rs:130-137`) to `_ => { tracing::error!("Unsupported provider"); return; }` — a **silent return from a spawned task**. The UI shows an agent that started and never produced anything. | CONFIRMED |
| P-29 | `commands/agent_commands.rs:134-137` | HIGH | silent failure | Unsupported provider id logs an error and `return`s from inside `tokio::spawn`. The `ActiveAgent` registered at `agent_commands.rs:56-62` with `is_running: true` is never cleared, and no event is emitted to the frontend. | A user-visible failure is downgraded to a log line and a leaked state entry. | See P-28. Same pattern repeats at `commands/diff_commands.rs:38`, `commands/recipe_commands.rs:212`, `recipes/runner.rs:653`, `agent/builtin_tools/analyze_diff_tool.rs:235`. | CONFIRMED |

| P-30 | `provider/streaming.rs:29-74` | **CRITICAL** | input tokens always zero | `parse_anthropic_event` handles four event types: `content_block_delta`, `content_block_start`, `message_delta`, `message_stop`. **There is no `message_start` handler.** Anthropic reports `usage.input_tokens` in the `message_start` event; `message_delta.usage` carries `output_tokens` (and cache counters), not the input count. `streaming.rs:65` reads `usage["input_tokens"].as_u64().unwrap_or(0)`, which is therefore **always 0**. | Input tokens — the dominant cost in an agent loop that resends the full history every turn — are never recorded on the streaming path for Anthropic either. Combined with P-06 and P-09, **input tokens are 0 for all three cloud providers in streaming mode**; only Ollama (free) reports them. | `CostTracker::calculate(model, 0, tokens_out)` (`agent/engine.rs:219,355`) computes output-only cost. An Opus session that really cost $9 (mostly input) is displayed as $2.25. The error grows with conversation length — exactly the direction that matters. | CONFIRMED |
| P-31 | `provider/streaming.rs:65-66` + `agent/engine.rs:456-457` | HIGH | double-counting risk | `streaming.rs:65-66` reads Anthropic's `message_delta.usage` values, which are **cumulative for the message**, and `engine.rs:456-457` then does `*total_tokens_in += tokens_in`. Anthropic emits one `message_delta` per message, so per-message this is currently correct — but the semantics are mismatched (cumulative value fed into an accumulator). | If Anthropic ever emits more than one `message_delta`, or if a provider is added that emits incremental deltas, the totals silently double-count with no test to catch it. | Latent. Flagged because the two halves of the contract disagree about whether the value is cumulative or incremental. | CONFIRMED (mismatch); SUSPECTED (current-day impact) |

### 1b. Cost / budget / routing findings

| ID | file:line | sev | class | what the code does | why it is wrong | failure scenario | status |
|----|-----------|-----|-------|--------------------|-----------------|------------------|--------|
| B-01 | `agent/engine.rs:73` | **CRITICAL** | dead guard / writer-no-reader | `AgentEngine::with_task_budget` has **zero callers**. `grep -rn "with_task_budget"` over the whole `src-tauri/src` tree returns only its own definition (`engine.rs:73`) and its doc comment (`engine.rs:45,71`). The two `AgentEngine::new` sites — `commands/agent_commands.rs:140` and `recipes/runner.rs:664` — never chain it. `engine.rs:62` initialises `task_budget: None`. | Therefore `self.task_budget` is **always `None`**, the guard at `engine.rs:150-165` never enters, `escalate_budget_exceeded` (`engine.rs:480`) is unreachable, and `emit_budget_exceeded` (`events.rs:490`) never fires from the engine. **The Budget Circuit Breaker does not exist at runtime.** | An agent runs away and spends unbounded money. The only surviving cap is `MAX_TOOL_TURNS` at `engine.rs:167`. The UI shows a budget panel (`commands/budget_commands.rs`) fed by DB columns that no enforcement path reads. | CONFIRMED |
| B-02 | `agent/builtin_tools/set_task_budget_tool.rs:79` | HIGH | writer with no reader | The `set_task_budget` tool validates input and writes `budget_max_tokens/usd/turns` to the `tasks` row, then returns `"✅ Budget set for task #N: ... max."` | Nothing reads those columns for enforcement (B-01). `db/tasks.rs:1114 get_task_budget_status` reads only the *usage* columns, and its sole caller is `commands/budget_commands.rs:38` (a display query). | The agent tells the user, in a green check mark, that a $0.50 cap is in force. It is not. This is worse than having no feature. | CONFIRMED |
| B-03 | `budget/mod.rs:3`, `agent/engine.rs:148-165` | HIGH | doc contradicts body | Module header says "**Pre-emptive** per-task token and cost caps enforced inside the AgentEngine loop." The check runs at the **top** of a turn against totals accumulated from *previous* turns (`engine.rs:216-220` updates after the stream completes). | It is post-hoc, not pre-emptive. A single turn can blow through the cap by a full context window of input plus 8192 output tokens before the next iteration notices. `check()` uses `>=` (`budget/mod.rs:75-83`), so the trip happens strictly *after* the limit is reached. | A $0.50 cap on Opus is breached by one turn costing $1.20 (60k input × $15/1M + 8k out × $75/1M). The breaker reports "halted at $1.20" against a $0.50 limit. | CONFIRMED |
| B-04 | `agent/engine.rs:217` | HIGH | usage lost on failure | `if let Ok(ref _r) = stream_result { budget_status.tokens_used = ... }` | Tokens consumed by a stream that errored mid-flight (P-15: any 429/500/disconnect) are never accumulated. The provider still bills them. | Ten consecutive failed streams cost real money and register as zero usage; the (inert) budget would not have caught them either. | CONFIRMED |
| B-05 | P-06 + P-09 × `agent/engine.rs:218` | **CRITICAL** | compounding | `budget_status.tokens_used` and `cost_usd` derive entirely from `total_tokens_in/out`, which are filled only by `StreamChunk::MessageDelta`. Per P-06 (OpenAI) and P-09 (Google), that chunk is **never emitted** on those two providers' streaming paths. | Even if B-01 were fixed, token and USD budgets would read 0 forever on OpenAI and Gemini. Only the turn cap would function. | User sets a $5 daily cap, runs GPT-4.1 all day, dashboard shows $0.00 spent, real bill is $80. | CONFIRMED |
| C-01 | `cost/tracker.rs:54-55` | HIGH | unreachable match arm + wrong price | `m if m.contains("gpt-4o") => (2.5, 10.0)` precedes `m if m.contains("gpt-4o-mini") => (0.15, 0.6)`. Every `gpt-4o-mini` string contains `gpt-4o`. | The mini arm is **dead code**. `gpt-4o-mini` is priced at full `gpt-4o` rates: 16.7× overcharge on input, 16.7× on output. | Cost dashboard reports $16.70 for a session that actually cost $1.00. | CONFIRMED |
| C-02 | `cost/tracker.rs:57` | **CRITICAL** | silent wrong-default | Fallback `_ => (1.0, 3.0) // Safe default`. | It is not safe, it is silently wrong for **most models the app ships**. Models that hit this fallback: `gpt-4.1` (real 5.0/15.0 per `openai.rs:41-42`), `gpt-4.1-mini` (1.0/3.0 — coincidentally right), `o4-mini` (1.10/4.40), `gemini-2.5-pro` (1.25/5.0 per `google.rs:50-51`), `gemini-2.5-flash` (0.075/0.30), `gemini-2.0-flash-thinking-exp-01-21` (0.10/0.40), and **every Ollama model** (0.0/0.0). Note `cost/tracker.rs:56` matches only `gemini-1.5-pro` — a model the app does not offer — so no Gemini model the user can actually pick is priced correctly. | Gemini 2.5 Flash is billed at 13× its real input price and 10× its real output price. Local Ollama inference — free — is billed at $1/1M in and $3/1M out. GPT-4.1 is under-billed 5×. The cost figure shown to the user is wrong for 7 of the 11 selectable models. | CONFIRMED |
| C-03 | `cost/tracker.rs:49-58` vs `provider/*.rs models_static()` | HIGH | duplicate divergent source of truth | Two independent hardcoded pricing tables exist: `ModelInfo.cost_in_per_1m/cost_out_per_1m` in each provider file, and `model_pricing()` in `cost/tracker.rs`. The file header at `cost/tracker.rs:2` claims "Pricing data mirrors provider model definitions for correctness." | It does not mirror them — see C-02. The claim is false as written. Nothing keeps them in sync; nothing reads `ModelInfo`'s price fields for actual cost computation (grep: `cost_in_per_1m` is only ever *written*, never read outside serialization to the frontend). | Editing the provider table to fix a price changes the UI's model list but not the billed figure. | CONFIRMED |
| C-04 | `cost/tracker.rs:18-23` | MED | no cache / reasoning dimension | `calculate(model, tokens_in, tokens_out)` — three arguments. There is no parameter for cache-read tokens, cache-write tokens, or reasoning tokens. | Structurally cannot price a cached or reasoning request correctly. Unit handling itself is correct (`/1_000_000.0` per-million — verified `tracker.rs:20-21`); the arithmetic is fine, the inputs and the table are not. | Post-caching, cost figures are off by up to 10×. Reasoning-model output tokens (billed as output) are at least counted, since `completion_tokens` includes them — but only when usage is reported at all (P-06). | CONFIRMED |
| C-05 | `state/app_state.rs:26,29,30,57-60` | HIGH | never-reset counters | `cost_today_usd`, `tokens_in_today`, `tokens_out_today` are incremented in `add_cost` (`app_state.rs:57-60`, called from `agent/engine.rs:356-357`). `grep -n "reset" state/app_state.rs` returns nothing. | The "today" suffix is a lie: these are process-lifetime totals. Confirms the sibling audit's finding. | A desktop app left running for a week shows a week's spend as "today". | CONFIRMED |
| C-06 | `budget/mod.rs:120-130`, `db/tasks.rs:1145-1167` | MED | UTC/local date boundary | `DailyBudgetSummary` is computed by SQL `WHERE date(created_at) = date('now')`. This one **is** correctly date-scoped (unlike C-05). | But `date('now')` is UTC in SQLite while `created_at` is written by `datetime('now')` (also UTC) — consistent — yet the user's "today" is their local day. For a user at UTC+8 the daily summary rolls over at 08:00 local. | Budget summary resets mid-morning. Cosmetic but confusing. | CONFIRMED (behaviour); SUSPECTED (whether any `created_at` writer uses local time) |
| C-07 | `agent/engine.rs:172` | HIGH | hardcoded context window | `let model_context = 200_000u32;` — used for compaction thresholding regardless of the selected model. | `gpt-4o` is 128k (`openai.rs:58`); `llama3.2` via Ollama defaults to 4096 (`ollama.rs:409`). `ModelInfo.context_window` exists and is never consulted here. | On GPT-4o the engine lets history grow to 200k estimated tokens before compacting; the request 400s at 128k. On a small Ollama model it overruns catastrophically. | CONFIRMED |
| C-08 | `agent/engine.rs:196` | MED | hardcoded max_tokens | `max_tokens: 8192` hardcoded in the request the engine builds. | Not model-aware, not configurable, and dropped entirely on OpenAI/Google (P-02/P-03) so it does not even take effect there. | Inconsistent output caps across providers with no way to tune. | CONFIRMED |
| R-01 | `routing/` (whole dir) | **CRITICAL** | spec feature absent | `routing/` routes **tasks to workers** (`worker-backend`, `worker-frontend`, … — `patterns.rs:14,20,26,32,38`), by keyword match plus historical success rate. It does not select models. `grep -rn "model" routing/` returns nothing relevant. | The product spec's **Model Router (role + priority + budget → model)** is **not implemented anywhere**. Model selection is: `state.active_model` read once in `commands/agent_commands.rs:37` (a global UI setting), or a `model` field on a worker YAML. There is no role-aware, priority-aware, or budget-aware model choice. | Every PA-mode turn — a trivial file listing and a complex refactor alike — runs on whatever model the user last picked in the dropdown. No cheap-model fallback, no escalation. This is the single biggest missing cost lever after prompt caching. | CONFIRMED |
| R-02 | `routing/patterns.rs:30-34` | MED | over-broad keywords | The `worker-mcp` pattern keys on `"tool"`, `"server"`, `"protocol"`, `"plugin"`. | `"tool"` and `"server"` appear in an enormous fraction of software task descriptions. Two hits fires the pattern at confidence 0.80. | A backend task titled "Add a server-side tool for CSV export" routes to `worker-mcp`. | CONFIRMED |
| R-03 | `routing/patterns.rs:54` | MED | substring matching | `haystack.contains(**kw)` — raw substring, no word boundary. | `"ui"` (patterns.rs:18) matches "build", "guide", "requirement", "quick". `"orm"` (patterns.rs:13) matches "format", "perform", "platform", "transform". `"api"` matches "capital", "rapid". | A task "Reformat the build guide" scores 2 hits on `worker-frontend` (`ui` in "guide" + … ) and on `worker-backend` (`orm` in "Reformat"). Routing is effectively noise for prose descriptions. | CONFIRMED |
| R-04 | `routing/scorer.rs:61-63` | MED | degenerate normalisation | When `avg_durations` is empty, `min_dur = f64::INFINITY` and `max_dur = 0.0`; `dur_range = (0.0 - INFINITY).max(1.0) = 1.0`. Then at `scorer.rs:77`, `1.0 - ((avg - INFINITY)/1.0)` → `+INFINITY` as f32. | The `>= 3` gate at `scorer.rs:70` and the `dur_samples > 0` filter at 57 make the empty case reachable only when a worker has ≥3 outcomes but zero positive durations *and* another worker qualifies — but the `min_dur = INFINITY` propagation is a real NaN/Inf hazard. `score.clamp(0.0,1.0)` at 86 masks it. | Scores silently clamp to 1.0 for every historical worker; the ranking becomes arbitrary (stable sort on equal keys). | SUSPECTED (reachability depends on data shape; the arithmetic hazard is CONFIRMED) |
| R-05 | `routing/mod.rs:42`, `mod.rs:91` | MED | unbounded-window heuristic | `get_routing_history(200)` — a fixed 200-row window. `record_outcome_auto` (`mod.rs:86-99`) *scans* those 200 rows in Rust to find the pending row for a task, with the comment "200 is generous". | With >200 assignments between a task's assignment and its completion, the pending row falls out of the window, `duration_ms` silently becomes `0` (`mod.rs:97` `.unwrap_or(0)`), and the scorer's duration term is poisoned. In a fleet run, 200 rows is not generous. | Long-running tasks systematically record duration 0, which `scorer.rs:47` filters out — so slow workers look like they have no timing data and get the neutral 0.5. | CONFIRMED |
| R-06 | `agent/builtin_tools/assign_task.rs:114`, `assign_tasks_batch.rs:190`, `commands/task_commands.rs:53,56,60` | MED | silent failure | Five `let _ = crate::routing::TaskRouter::record_{assignment,outcome_auto}(...)` calls. | Every routing-history write is discarded on error with no log. The affinity scorer's entire input can silently stop being written (e.g. DB locked) and routing degrades to "default 0.5 for everyone" with no signal. | Routing quality quietly collapses; the affinity panel shows stale numbers forever. | CONFIRMED |

---

## 2. Provider capability matrix

Legend: ✅ implemented · ⚠️ partial/broken · ❌ absent · n/a not applicable

| Capability | Anthropic | OpenAI | Google (Gemini) | Ollama | **Grok (xAI)** |
|---|---|---|---|---|---|
| Provider exists | ✅ `anthropic.rs` | ✅ `openai.rs` | ✅ `google.rs` | ✅ `ollama.rs` | **❌ ABSENT** |
| Streaming | ✅ `anthropic.rs:174` | ⚠️ `openai.rs:218` (no usage, no stop_reason — P-06) | ⚠️ `google.rs:180` (no args, no usage, no Done — P-08/09/10) | ⚠️ `ollama.rs:143` (falls back to non-stream when tools present) | ❌ |
| Tool / function calling | ✅ `anthropic.rs:109-122` | ⚠️ `openai.rs:156-172` (args corrupted in stream — P-07) | ⚠️ `google.rs:107-115` (args discarded — P-08; wrong result field — P-11) | ❌ never sent — `ollama.rs:235-238` (P-04) | ❌ |
| Parallel tool calls | ✅ (Anthropic emits multiple `content_block_start`; `streaming.rs:51-61` handles each) | ❌ first-only (P-07) | ❌ first-part-only (P-09) + id collision (P-12) | ❌ | ❌ |
| Structured output / JSON schema | ❌ no request field (P-26) | ❌ | ❌ | ❌ | ❌ |
| `tool_choice` control | ❌ | ⚠️ hardcoded `"auto"` `openai.rs:171` | ❌ | ❌ | ❌ |
| System prompts | ✅ `anthropic.rs:123-125` | ✅ `openai.rs:78-83` | ✅ `google.rs:100-104` | ✅ `ollama.rs:248-250` | ❌ |
| Multi-modal input (vision) | ❌ no wire format (P-25) | ❌ | ❌ | ❌ | ❌ |
| Reasoning / thinking | ✅ `anthropic.rs:133-137` + `ThinkingDelta` `streaming.rs:39-42` | ⚠️ `reasoning_effort` sent `openai.rs:180-182`; no reasoning-token accounting; no reasoning delta parsing | ⚠️ `thinkingConfig` sent `google.rs:137-139`; no thought parts parsed | ❌ hardcoded `supports_thinking: false` | ❌ |
| **Prompt caching** | ❌ | ❌ | ❌ | n/a | ❌ |
| Usage reporting (non-stream) | ✅ `anthropic.rs:254-255` | ✅ `openai.rs:380-381` | ✅ `google.rs:352-353` | ✅ `ollama.rs:327-330` | ❌ |
| Usage reporting (stream) | ⚠️ output only — `input_tokens` always 0, no `message_start` handler (P-30) | ❌ dead branch (P-06) | ❌ unreachable branch (P-09) | ✅ `ollama.rs:353-356` | ❌ |
| Cache-token accounting | ❌ type cannot express it (P-19) | ❌ | ❌ | n/a | ❌ |
| `max_tokens` honoured | ✅ `anthropic.rs:117` | ❌ dropped (P-02) | ❌ dropped (P-03) | ❌ dropped (P-04) | ❌ |
| Truncation (`max_tokens`) detected | ❌ (P-18) | ❌ | ❌ | ❌ | ❌ |
| Retry / backoff / `Retry-After` | ❌ (P-15) | ❌ | ❌ | ❌ | ❌ |
| HTTP timeout | ❌ (P-14) | ❌ | ❌ | ❌ (ollama-rs default) | ❌ |

### What implementing Grok would require

The trait is small — `provider/traits.rs:12-31`: `id`, `name`, `models`,
`complete`, `stream`, `test_connection`. xAI's API is OpenAI-Chat-Completions-compatible
at `https://api.x.ai/v1`, so the cheapest correct path is to **parameterise
`OpenAIProvider` with a base URL + id/name** rather than write a fourth copy of the same
SSE parser. Concretely:

1. `OpenAIProvider` currently hardcodes `OPENAI_API_BASE` (`openai.rs:18`) and returns
   literal `"openai"` / `"OpenAI"` (`openai.rs:192-193`). Add `base_url: String`,
   `id: String`, `name: String` fields.
2. Register `"xai" => Arc::new(OpenAIProvider::with_base(api_key, "https://api.x.ai/v1", "xai", "xAI"))`
   at the **five** provider factory sites (P-29 list).
3. Add a Grok `models_static()` table — which inherits P-19 (no cache pricing) and P-20
   (hardcoded prices).
4. Add `"xai"` to `commands/settings_commands.rs:34-45` so the settings panel lists it.
5. Every OpenAI-path bug (P-02, P-06, P-07, P-21, P-22) is inherited verbatim. **Fix
   those first, or Grok ships pre-broken.**

---

## 3. Prompt caching verdict

**There is no prompt caching in AutoDev's Rust provider layer. None. Not partial, not
disabled-by-default — the concept does not appear in the code.**

Evidence: `grep -rn "cache_control\|ephemeral\|cache_creation\|cached_tokens\|cache_read"`
over `src-tauri/src` returns **zero** hits inside `provider/`. The only cache-adjacent
hits are (a) `sidecar/client.rs:42` and `sidecar/client.rs:899`, which merely document
that the *Python sidecar's* usage dict contains a `cached_tokens` key — a field that is
being deleted with the sidecar; and (b) unrelated uses of the English word "ephemeral"
for short-lived subagents (`subagent/bridge.rs:56`, `slug.rs:19`, etc.).

`AnthropicProvider::build_body` (`anthropic.rs:67-142`) emits `system` as a bare string
(`anthropic.rs:124`), not as a content-block array, so there is **no place to hang a
`cache_control` breakpoint** without changing the body shape. Tool definitions
(`anthropic.rs:109-113`) likewise carry no `cache_control`. The response parser
(`anthropic.rs:250-256`) reads only `usage.input_tokens` / `usage.output_tokens` and
would silently ignore `cache_creation_input_tokens` / `cache_read_input_tokens` even if
the API returned them. The streaming parser has the same gap (`streaming.rs:62-72`).

**Cost consequence.** An agent loop resends the entire system prompt, every tool schema,
and the full conversation history on every turn. Without caching, all of it is billed at
full input rate every time. This is a headline finding: it is the largest single
avoidable cost in the product, and fixing it requires changes to three places —
`build_body`'s system/tools serialisation, `ModelInfo`'s price fields (P-19), and the
usage parsers in both `anthropic.rs` and `streaming.rs`.

---

## 4. Pricing and cost-math audit

### 4a. Every hardcoded price and model id

| file:line | model id | ctx | $/1M in | $/1M out |
|---|---|---|---|---|
| `provider/anthropic.rs:38-44` | `claude-opus-4-5` | 200k | 15.0 | 75.0 |
| `provider/anthropic.rs:47-53` | `claude-sonnet-4-5` | 200k | 3.0 | 15.0 |
| `provider/anthropic.rs:56-62` | `claude-haiku-4-5` | 200k | 0.25 | 1.25 |
| `provider/openai.rs:36-42` | `gpt-4.1` | 1M | 5.0 | 15.0 |
| `provider/openai.rs:44-51` | `gpt-4.1-mini` | 1M | 1.0 | 3.0 |
| `provider/openai.rs:53-60` | `gpt-4o` | 128k | 2.50 | 10.0 |
| `provider/openai.rs:62-70` | `o4-mini` | 200k | 1.10 | 4.40 |
| `provider/google.rs:36-43` | `gemini-2.0-flash-thinking-exp-01-21` | 1M | 0.10 | 0.40 |
| `provider/google.rs:44-52` | `gemini-2.5-pro` | 1M | 1.25 | 5.0 |
| `provider/google.rs:53-61` | `gemini-2.5-flash` | 1M | 0.075 | 0.30 |
| `provider/ollama.rs:415-487` | `llama3.2`, `llama3.1`, `mistral`, `mixtral`, `codellama`, `deepseek-coder`, `qwen2.5`, `custom` | various | 0.0 | 0.0 |

Additional hardcoded model ids outside the price tables:
`provider/anthropic.rs:82-84` (six slug aliases), `provider/google.rs:236`
(`gemini-2.5-flash` in `test_connection`).

Notes:
- `provider/google.rs:36` is an **experimental preview id** (`-exp-01-21`) presented as a
  first-class model. Preview ids are withdrawn on a schedule.
- Gemini 2.5 Pro is tiered by prompt length (>200k tokens costs more); the flat 1.25/5.0
  is wrong for long-context calls on a 1M-context model. SUSPECTED (depends on current
  vendor pricing, which I did not fetch — but a single flat rate cannot express a tiered
  schedule regardless).
- Claude Haiku 4.5 at 0.25/1.25 is the Haiku **3.5** price shape. SUSPECTED.
- **No cache-read or cache-write price exists for any model** (P-19).

### 4b. Second pricing table — `cost/tracker.rs`

This is the table that actually produces the dollar figures. It is **not** the provider
table (C-03).

| `cost/tracker.rs:line` | match key | $/1M in | $/1M out | verdict |
|---|---|---|---|---|
| `:51` | `contains("claude-opus")` | 15.0 | 75.0 | matches provider table |
| `:52` | `contains("claude-sonnet")` | 3.0 | 15.0 | matches |
| `:53` | `contains("claude-haiku")` | 0.25 | 1.25 | matches provider table (both possibly stale — §4a) |
| `:54` | `contains("gpt-4o")` | 2.5 | 10.0 | matches, but shadows `:55` |
| `:55` | `contains("gpt-4o-mini")` | 0.15 | 0.6 | **DEAD ARM** — C-01 |
| `:56` | `contains("gemini-1.5-pro")` | 1.25 | 5.0 | model not offered by the app — dead in practice |
| `:57` | `_` fallback | 1.0 | 3.0 | **wrong for 7 of 11 selectable models** — C-02 |

### 4c. Cost math traced end to end

1. **Token source.** `agent/engine.rs:213-215` passes `&mut total_tokens_in/out` into
   `stream_response`, which accumulates from `StreamChunk::MessageDelta`. Values come
   from the provider, not estimated — **but only Anthropic and Ollama ever emit that
   chunk** (P-06, P-09). OpenAI and Google contribute 0.
2. **Accumulation shape.** `MessageDelta` carries *cumulative* `tokens_in`/`tokens_out`
   for Anthropic (`streaming.rs:65-66` reads `usage.input_tokens`/`output_tokens` from
   `message_delta`, which Anthropic reports as running totals for the message). Whether
   `stream_response` *assigns* or *adds* determines correctness across multi-turn loops —
   `engine.rs:218` reads `(total_tokens_in + total_tokens_out)` as a running total across
   all turns, so an assignment per turn would under-count. **SUSPECTED** — I did not read
   `stream_response`'s accumulation body line by line; flag for follow-up at
   `agent/engine.rs` `stream_response`.
3. **Unit conversion.** `cost/tracker.rs:20-21` — `(tokens as f64 / 1_000_000.0) * price_per_1m`.
   **Correct.** Per-million, not per-token. No unit error.
4. **Both directions counted.** Input and output are both included (`tracker.rs:20-21`).
   **Correct.**
5. **Float accumulation.** `state/app_state.rs:58` `self.cost_today_usd += cost_usd` — f64
   accumulation of small values. Drift is negligible at f64 for realistic call counts.
   Not a finding.
6. **The failure is upstream of the arithmetic.** The math is right; the price table
   (C-01, C-02) and the token inputs (P-06, B-05) are wrong. A correct formula over wrong
   operands is still a wrong number.

**Net verdict on cost:** dollar figures shown to the user are wrong for the majority of
selectable models, and are structurally 0 for all streaming OpenAI and Gemini traffic.



---

### 1c. Subagent / session / memory findings

| ID | file:line | sev | class | what the code does | why it is wrong | failure scenario | status |
|----|-----------|-----|-------|--------------------|-----------------|------------------|--------|
| S-01 | `subagent/bridge.rs:99,130,221` | **CRITICAL** | sidecar coupling | The bridge's entire lifecycle is three `OrchestratorClient` calls: `create_ephemeral_agent` (`:99`), `invoke_agent_stream_raw` (`:221`), `delete_agent` (`:130`). `OrchestratorClient` is `crate::sidecar::client` (imported `bridge.rs:32`). | **When the Python sidecar is deleted, every subagent stops working.** `bridge.rs` has no in-process execution path. It never touches `provider/` or `AgentEngine`. | `spawn_subagent`, `spawn_qc_review`, and `spawn_parallel_subagents` all fail at `create_ephemeral_agent`. The QC review pipeline and Delegate mode both die. | CONFIRMED |
| S-02 | `subagent/bridge.rs:281-298`, `:315-395` | HIGH | shared parser removed with sidecar | `find_frame_end` and `parse_event_frame` are `pub(crate)` and, per their own comments (`:278-280`, `:311-314`), are reused by `sidecar::client::invoke_worker_streaming` — the **main Worker-mode SSE bridge**. | The frame parser is coupled in both directions: the bridge needs the sidecar, and the sidecar's worker path needs the bridge's parser. Deleting the sidecar orphans half of `bridge.rs`; keeping `bridge.rs` keeps a dead dependency. | Worker-mode streaming loses its framing logic if `bridge.rs` is deleted wholesale. Plan the two deletions together. | CONFIRMED |
| S-03 | `subagent/manager.rs:287-295`, `:299-313`, `:124-130` | **CRITICAL** | permit leak / deadlock | `terminated_at` is set **only** in `finalise`, which is called **only** from `await_result` (`:149,153,157`). `pump_events` (`:263-282`) updates `handle.status` on a terminal `Status` event but never sets `terminated_at`. `prune_terminal` (`:302-312`) keeps any entry with `terminated_at == None` forever, so the `PoolPermit` (`ManagerEntry._permit`, `:45`) is never dropped. | A subagent that is spawned but never awaited leaks its global-semaphore slot **permanently**. `commands/subagent_commands.rs:73` is exactly that path: the Tauri `spawn_subagent` command returns the handle without awaiting; awaiting is a separate command (`:82`) the frontend may never call. `spawn_qc_review.rs:205` does await, but in a detached background task that can itself be dropped. | After 16 unawaited spawns the global semaphore (`pool.rs:26`, `MAX_GLOBAL_CONCURRENT = 16`) is exhausted and `acquire_owned().await` (`pool.rs:138`) blocks forever. **All subagent spawning deadlocks with no error, no timeout, and no log line.** The registry also grows unboundedly. | CONFIRMED |
| S-04 | `subagent/manager.rs:214` vs `bridge.rs:107-108,130` | HIGH | wrong id on cleanup | `terminate` calls `self.client.delete_agent(id)` where `id` is the **AutoDev slug** (`slug::generate`, `manager.rs:93`). The bridge deletes using `agent_id` — the value **returned by** `create_ephemeral_agent` (`bridge.rs:107-108`). `sidecar/client.rs:1492` logs `"created ephemeral subagent '{slug}' → {id}"`, i.e. slug and id are distinct values. | `terminate`/`abort` issue a DELETE against an identifier the orchestrator does not know, so the upstream agent is **not** deleted. The failure is swallowed into a `warn!` (`manager.rs:215`). | Force-aborting a subagent leaves a live agent in core-orchestrator, still consuming a worker slot, until the next startup orphan sweep. | CONFIRMED |
| S-05 | `subagent/manager.rs:354-356` | HIGH | reachable panic | `fn truncate(s, max) { ... format!("{}…", &s[..max]) }` — **no `is_char_boundary` check**, unlike the otherwise-identical helpers at `memory/recall.rs:178-187` and `memory/summarizer.rs:147-153` which do check. | Called from `short_label` (`manager.rs:350`) on `SubagentEvent::Error { message }`, whose text originates from the sidecar's SSE `error` frame — arbitrary, potentially non-ASCII text. | A subagent error message containing an emoji or accented character at byte 60 panics the `pump_events` task, killing event forwarding for that subagent and leaving it stuck in the registry (compounding S-03). | CONFIRMED |
| S-06 | `subagent/pool.rs:56-69` | HIGH | doc-vs-body + panic risk | `Drop for PoolPermit` spawns a **tokio task** to do the per-parent decrement asynchronously. The comment (`:60-61`) claims "Synchronous decrement — uses the std-Mutex-equivalent via blocking_lock". | The body does the opposite of the comment. Worse: `tokio::task::spawn` **panics** if called outside a runtime, and `Drop` can run during runtime shutdown or from a non-async context. The deferred decrement also makes the per-parent count lag, so a tight spawn loop can transiently exceed `MAX_PER_PARENT`. | Panic-on-drop during shutdown; or a rogue parent briefly exceeding its cap. | CONFIRMED |
| S-07 | `subagent/manager.rs:80-82` + `bridge.rs:99-105` | MED | dead config | `config.max_turns` is defaulted to 20 (`manager.rs:80-82`, `types.rs:60`) and **never read again**. `create_ephemeral_agent` is called with `(subagent_id, system_prompt, model, mcp_servers, capabilities)` — no `max_turns`. | A per-subagent turn cap the user/LLM can set has no effect. | A runaway subagent ignores its declared `max_turns` and is bounded only by `timeout_secs` (`bridge.rs:195`, default 300s). | CONFIRMED |
| S-08 | `subagent/bridge.rs:195-211` | MED | partial timeout | `timeout()` wraps `consume_stream` only. `create_ephemeral_agent` (`:99`) and `delete_agent` (`:130`) are un-timed. | A hung create or delete blocks the bridge task indefinitely; the manager's `await_result` blocks with it, and (per S-03) the permit is never released. | Sidecar stalls during agent creation → one permanently-wedged pool slot per attempt. | CONFIRMED |
| S-09 | `subagent/bridge.rs:232-269` | MED | unbounded buffer | `buf` accumulates every byte until a `\n\n`/`\r\n\r\n` separator appears (`:239` `else { break }`). There is no size cap and no eviction. | A malformed or separator-less stream grows `buf` without limit. | A sidecar that streams a large blob without frame separators OOMs the desktop app. | CONFIRMED |
| S-10 | `subagent/bridge.rs:156-174` | MED | error laundered into success | The failure branch returns `Ok(SubagentResult { status: Failed, cost_usd: 0.0, ... })` — an `Err` from `run_invoke_phase` becomes an `Ok`. | Any caller checking `Result::is_ok()` treats a failed subagent as a success. And a subagent that burned real tokens before failing is recorded at `cost_usd: 0.0`. | Parallel-spawn aggregation (`spawn_parallel_subagents.rs:177`) counts failures as completions; cost accounting under-reports. | CONFIRMED |
| SE-01 | `commands/agent_commands.rs:243-256` | **CRITICAL** | history reconstruction loses tools | `build_history` maps stored `Message`s to `RequestMessage`s. `MessageRole::Tool => return None` (`:249`) with the comment "Tool results folded into user messages" — **nothing folds them anywhere**. Assistant messages become a single `MessageContent::Text` from `msg.content` (`:253`), and `msg.content` for an assistant turn was populated at `:173-176` by extracting **only** `Text` blocks. | On session reload the model is handed a conversation in which every `tool_use` block and every `tool_result` has vanished. The comment is false. | Reload a session in which the agent read 8 files and wrote 3. The replayed history says only "I'll read the files." The agent re-reads everything (wasting tokens) or asserts it already did work whose results it cannot see. A pure-tool-use assistant turn replays as an **empty** text block, which Anthropic rejects with a 400. | CONFIRMED |
| SE-02 | `commands/agent_commands.rs:173-176` | HIGH | multi-block text loss | `for block in &last_msg.content { if Text { assistant_msg.content = text.clone(); } }` — **assignment inside a loop**, not append. | With more than one `Text` block in the response, only the last survives. Anthropic emits separate text blocks around tool_use blocks routinely. | The assistant's preamble ("Here's what I found: …") is discarded; only its closing sentence is persisted and shown on reload. | CONFIRMED |
| SE-03 | `commands/agent_commands.rs:169` | MED | wrong message picked | `updated_history.last()` is checked for `role == "assistant"`; if the engine's returned history ends with a tool-result (user-role) message, the whole block is skipped and **nothing is persisted**. | The assistant's final turn is silently lost. | A run that ends on a tool result saves no assistant message and no episodic row. | SUSPECTED (depends on the engine's exact return shape at loop exit) |
| SE-04 | `session/storage.rs:198-202` | HIGH | hardcoded LIMIT 50 | `list_sessions` is `... ORDER BY updated_at DESC LIMIT 50` with no pagination parameter. | Sessions 51+ are invisible to the UI **and** to `MemoryMigration::run` (`memory/migration.rs:34` calls `list_sessions()`), whose own docstring claims it migrates "all existing sessions" (`migration.rs:2,20`). | A user with 200 sessions has 150 of them permanently excluded from episodic backfill and from the session list. | CONFIRMED |
| SE-05 | `memory/summarizer.rs:46-57` | MED | dead code + false doc | `on_session_end` has **zero callers** (`grep -rn "on_session_end"` returns only its definition). Its docstring says "called from Tauri command delete_session". Its body also contradicts its own inline comment: `:51-52` says "Only generate a final summary if there isn't one already", then `:53` summarises unconditionally. | Two false claims and one dead function. Deleting a session produces no final summary, so the last N turns before deletion never reach cross-session recall. | Session summaries only ever come from the every-20-turns trigger; short sessions are never summarised at all. | CONFIRMED |
| SE-06 | `session/storage.rs:284-287` | MED | FK-dependent cascade | `delete_session` deletes only the `sessions` row and relies on `ON DELETE CASCADE` (`:67`) plus `PRAGMA foreign_keys = ON` (`:30`). The `messages` table is only guaranteed to carry that FK when it was created by the `if !table_exists` branch (`:61-82`); the migration branch (`:83-100`) adds columns to a pre-existing table whose FK is unknown. | On any install whose `messages` table predates the V2 schema, deleting a session orphans all of its messages permanently. | Unbounded orphan-row growth in `~/.airs/autodev.db`. | SUSPECTED |
| M-01 | `memory/semantic.rs:236-237`, `:247-248` | HIGH | reachable panic (byte index) | `content.to_lowercase().find(pat)` returns a byte offset **into the lowercased string**, which is then used to slice the **original**: `content[pos..]`. | Lowercasing changes byte lengths for many non-ASCII characters (e.g. `İ` U+0130 → two bytes lowercased; `ẞ` → `ß`). The offset is then wrong for the original, and `&content[pos..]` panics on a non-char-boundary index. | A user message containing certain non-ASCII characters before the phrase "i prefer " panics the memory-extraction thread spawned at `commands/agent_commands.rs:75-84`. That thread is detached, so the panic is invisible except in the log — but the fact is lost. | CONFIRMED |
| M-02 | `memory/recall.rs:31` + `agent/context.rs:118` | HIGH | relevance is a no-op in production | `build_memory_context(session_id, query, working_dir)` is called from the **only** production site with `query = ""` (`context.rs:118`). `ProceduralStore::search("", 3)` (`procedural.rs:169-178`) builds `pattern = "%%"`, which `LIKE` matches against **every row**, ordered `updated_at DESC LIMIT 3`. | The module docstring (`recall.rs:8`) promises "3 most-relevant topic excerpts (keyword match on query)". In practice it returns the 3 most-recently-edited topics regardless of relevance. `_working_dir` is also `_`-prefixed and unused (`recall.rs:31`), despite the header's per-project framing. | Procedural memory injection is recency, not relevance. A user asking about deployment gets whatever topic they last edited. | CONFIRMED |
| M-03 | `memory/recall.rs:34` + `semantic.rs:136-148` | HIGH | cross-project fact leakage | `SemanticStore::recall_facts(limit)` has **no `working_dir` filter** — `WHERE is_active = 1 AND confidence >= 0.7 ORDER BY confidence DESC`. The `working_dir` column exists (`db.rs:70`) and is written (`semantic.rs:83`) but is never used in any read. | Facts learned in project A are injected into project B's system prompt as "What I know about you and this project" (`recall.rs:54`). | The PA tells you your Rust project "uses SQLAlchemy" because you said so in an unrelated Python repo. Writer-with-no-reader on the `working_dir` column. | CONFIRMED |
| M-04 | `memory/recall.rs:162-176` | HIGH | case-sensitivity bug in scorer | `keyword_score` lowercases the **text** (`:166`) but not the **query words** (`:167-170`). It then tests `text_lower.contains(*w)` with the raw-case word. | Every capitalised query term fails to match. Queries are natural language, so this silently zeroes relevance for proper nouns, acronyms, and sentence-initial words. | Searching memory for "Tauri IPC" scores 0.0 against a memory literally containing "tauri ipc", and `search_all` (`:90`) drops the episodic hit entirely because `score > 0.0` is false. | CONFIRMED |
| M-05 | `commands/agent_commands.rs:183-186` | HIGH | writer with no real data | Every agent response is stored via `store_agent_response(&sid2, &agent_text, None, 0.0, 0, 0, None, None, Some(&wdir2), None)` — model `None`, cost `0.0`, tokens `0,0`, thinking `None`, tool_calls `None`, all **hardcoded literals**. | The `episodic_memories` columns `model`, `cost_usd`, `tokens_in`, `tokens_out`, `thinking`, `tool_calls` (`db.rs:41-46`) are permanently zero/NULL on the live path. That in turn makes two blocks of `summarizer.rs` dead: `tools_used` (`:97-117`) is always empty, and `total_cost` (`:120-125`) is always `0.0`. | Session summaries never mention which tools were used or what the session cost, though the code appears to compute both. The Memory UI shows $0.00 for every turn. | CONFIRMED |
| M-06 | `memory/db.rs:14-24` | HIGH | connection-per-call + migrate-per-call | `MemoryDb::open()` opens a **new** `Connection` and runs the full `migrate()` `execute_batch` — including `PRAGMA journal_mode = WAL` and every `CREATE TABLE`/`CREATE INDEX IF NOT EXISTS` — on **every single call**. There are 15+ call sites across `episodic.rs`, `semantic.rs`, `procedural.rs`, `recall.rs`. `build_memory_context` alone opens the DB at least 3 times per turn. | No pooling, no caching, no shared handle. Contrast `session/manager.rs:10`, which holds one connection in an `Arc<Mutex<_>>`. | Per-turn latency overhead and file-lock churn on every prompt assembly. | CONFIRMED |
| M-07 | `memory/db.rs:26-111` | HIGH | no `busy_timeout` | The migration batch sets `journal_mode = WAL` and `foreign_keys = ON` but **no `PRAGMA busy_timeout`**. Same omission in `session/storage.rs:27-43`. | With M-06's connection-per-call pattern plus the two detached `std::thread::spawn` writers at `commands/agent_commands.rs:73` and `:182`, concurrent writes are routine. Without `busy_timeout`, SQLite returns `SQLITE_BUSY` **immediately** rather than retrying. | A memory write racing the summarizer fails with `database is locked`. On the write paths that error is swallowed (`semantic.rs:206` `let _ =`; `agent_commands.rs:78` logs a warn) — the fact is silently lost. | CONFIRMED |
| M-08 | `memory/db.rs:93` vs `procedural.rs:169-178`, `semantic.rs:166-179` | MED | doc contradicts body | The schema comment claims the `procedural_memories` table "enables fast full-text search". There is **no FTS5 virtual table** anywhere in `memory/`. Search is `LOWER(content) LIKE '%q%'` — an unindexable full-table scan. Same for `semantic.rs:174`. | "Fast full-text search" is false. No index can serve a leading-wildcard `LIKE`. | Search cost grows linearly with memory size; there is no vector/embedding search of any kind in `memory/` (grep for `embedding`, `vector`, `cosine`: zero hits). | CONFIRMED |
| M-09 | `memory/semantic.rs:44-52,:80-84` | MED | dedup compares mismatched forms | The near-duplicate probe compares `LOWER(SUBSTR(fact_text,1,80))` (the **raw stored** column) against `LOWER(SUBSTR(?2,1,80))` where `?2` is `normalize(fact_text)` — whitespace-collapsed. The subsequent INSERT (`:82`) stores the **raw** `fact_text`, not the normalized form. | The two sides of the comparison are normalized differently, so dedup misses whenever the incoming text has non-single-space whitespace. | Repeating a preference across sessions accumulates near-duplicate rows. Combined with M-10, semantic memory grows without bound. | CONFIRMED |
| M-10 | `memory/` (whole module) | MED | unbounded growth, no eviction | `episodic_memories` stores every user message and agent response forever. `semantic_memories` only soft-deletes (`semantic.rs:91-99`). There is no retention policy, no pruning job, no size cap, and no `VACUUM`. | The only bounded structure is the subagent registry's `TERMINAL_RETENTION`. Memory DB grows monotonically. | `~/.autodev/memory.db` grows without limit; M-08's full-scan searches get slower every session. | CONFIRMED |
| M-11 | `memory/migration.rs:59-90` | MED | non-atomic, self-poisoning | The per-message loop uses `?` (`:67`, `:83`). A single failed insert aborts `run()` mid-session, leaving that session **partially** migrated. The skip check (`:42-43`) is `get_turn_count(&session.id) > 0`, so the half-migrated session is skipped forever on subsequent runs. | The docstring (`:6-7`) claims idempotency "via content-hash equality check" — the body does a per-session existence check, not a content hash. Doc contradicts body. | One transient `SQLITE_BUSY` (M-07) permanently truncates a session's episodic history. | CONFIRMED |
| M-12 | `memory/episodic.rs:223`, `semantic.rs:292`, `procedural.rs:192`, `session/storage.rs:215,280` | MED | silent row drops | Five `.filter_map(\|r\| r.ok())` calls over `query_map` results. | A row that fails to deserialize (bad timestamp, NULL in a NOT NULL-typed field) is dropped with no log. | Memory or messages silently disappear from recall with no diagnostic. | CONFIRMED |
| M-13 | `memory/episodic.rs:218-220`, `semantic.rs:284-289`, `procedural.rs:188-189`, `session/storage.rs:211,278` | MED | timestamp corruption | `row.get::<_,String>(n)?.parse().unwrap_or_default()` — an unparseable timestamp silently becomes `DateTime::default()` (1970-01-01T00:00:00Z). | Ordering and display are corrupted rather than erroring. | A single bad `created_at` sorts a memory to the beginning of time; `get_recent_summaries` ordering (`episodic.rs:134`) is silently wrong. | CONFIRMED |
| M-14 | `memory/summarizer.rs:87` | LOW | ineffective dedup | `keywords.dedup()` on an **unsorted** `Vec` — `Vec::dedup` removes only *consecutive* duplicates. | Duplicate keywords survive across non-adjacent positions, and `truncate(8)` then keeps repeats. | Summary "Topics:" line reads "tauri, memory, tauri, memory, …". | CONFIRMED |
| M-15 | `memory/db.rs:115-122`, `procedural.rs:213-219`, `agent/context.rs:132-135` | MED | cross-platform home resolution | Home is resolved as `USERPROFILE` then `HOME`, hand-rolled in three separate places (plus a fourth, `session/storage.rs:297-303`, defining a local module literally named `dirs_next` with the comment "we can't add a dep mid-build"). | Windows-first ordering. On macOS/Linux `USERPROFILE` is normally unset so the `HOME` fallback works — but a shell that exports `USERPROFILE` (WSL interop does this) silently redirects the DB path. No `%LOCALAPPDATA%` use on Windows, which is where per-user app data belongs. | Under WSL interop, `~/.autodev/memory.db` resolves to a Windows path; two AutoDev installs disagree about where memory lives. | CONFIRMED |
| G-01 | `provider/ollama.rs:12-13,170` | LOW | `catch_unwind` on non-`UnwindSafe` closure | See P-05. | — | — | CONFIRMED |
| G-02 | `agent/engine.rs:467-471` | HIGH | silent tool-input corruption | `serde_json::from_str(&json_str).unwrap_or(Value::Object(Map::new()))` — a malformed accumulated tool-input JSON becomes an **empty object**, and the tool is executed anyway. | Directly downstream of P-07 (OpenAI drops the first `arguments` fragment) and P-08 (Gemini discards args entirely). The corruption is laundered into a plausible-looking empty call. | `write_file` is invoked with `{}` — no path, no content — instead of failing loudly. The agent sees a tool error about a missing argument and blames itself. | CONFIRMED |
| G-03 | `agent/engine.rs:467` | MED | nondeterministic ordering | `for (id, (name, json_str)) in tool_use_map` iterates a `HashMap`, so parallel tool calls are appended to `content_blocks` in **random** order. | Anthropic requires `tool_result` blocks to correspond to the prior `tool_use` blocks; scrambled ordering also makes runs non-reproducible. | Two parallel tool calls execute in a different order on every run; debugging is non-deterministic. | CONFIRMED |
| G-04 | `agent/engine.rs:427` | MED | one bad chunk kills the turn | `match chunk? { ... }` inside the `while let` — the first `Err` from the provider stream propagates out of `stream_response` and ends the turn. | Compounds P-16: a single malformed SSE line discards all text and tool calls accumulated so far in that turn. | Partial work is thrown away and the tokens are still billed (B-04 means they aren't even counted). | CONFIRMED |

---

## 5. Is Rust-side memory wired into prompt assembly?

**Yes — it is wired in, on exactly one path, with three defects that make what arrives
nearly useless.** This is not a written-never-read module, but it is close to a
read-but-not-usefully-read one.

### The wiring, with evidence

1. `agent/engine.rs:96-97` — the engine builds `ContextBuilder::new(working_dir).with_session(session_id)`.
2. `agent/engine.rs:101` — calls `context_builder.build_system_prompt_with_mode(self.pa_mode, working_dir)`.
3. `agent/context.rs:117-121` —
   ```rust
   if let Some(ref sid) = self.session_id {
       if let Some(memory_block) = crate::memory::build_memory_context(sid, "", repo_path) {
           parts.push(memory_block);
       }
   }
   ```
4. `agent/context.rs:123` — `parts.join("\n\n---\n\n")` produces the system prompt.
5. `agent/engine.rs:196` — that string becomes `CompletionRequest.system`, which reaches
   Anthropic at `provider/anthropic.rs:124`, OpenAI at `openai.rs:78-83`, Google at
   `google.rs:100-104`.

So the chain is complete and the memory block does reach the model. `build_memory_context`
is the **only** consumer of `EpisodicStore::get_recent_summaries`, `SemanticStore::recall_facts`,
and `ProceduralStore::search` on a prompt path.

### The three defects that hollow it out

- **The query is empty.** `context.rs:118` hardcodes `""`. Procedural recall therefore
  degenerates to "3 most recently edited topics" (M-02); the whole relevance mechanism is
  bypassed on the only production call.
- **`system_prompt_override` bypasses memory entirely.** `agent/engine.rs:98-102`:
  ```rust
  let system_prompt = if let Some(override_str) = system_prompt_override {
      override_str.to_string()          // ← build_system_prompt_with_mode never runs
  } else { context_builder.build_system_prompt_with_mode(...) };
  ```
  The override is a user-facing settings-panel field (`commands/settings_commands.rs:125-131`,
  read at `commands/agent_commands.rs:102-105`). **Any user who customises their system
  prompt silently loses all memory recall**, with no warning anywhere in the UI or logs.
- **It is Mode-1 PA only.** `context.rs:116` states it plainly: "Skipped when no session_id
  is set (e.g. MCP server, subagent, recipe mode)." Recipes (`recipes/runner.rs:664`),
  subagents, and the MCP server get no memory.

### Maturity of the three tiers

| Tier | Real or stubbed? | Storage | Retrieval | Verdict |
|---|---|---|---|---|
| Episodic | **Real** — writes on every turn (`agent_commands.rs:77`, `:183`) | `episodic_memories` (`db.rs:35-58`), 3 indexes | `get_recent_summaries` — SQL `ORDER BY created_at DESC LIMIT 5`, no scoring | Works, but 6 of its 14 columns are always zero/NULL (M-05) |
| Semantic | **Real but crude** — regex-free substring rules over 12 phrases (`semantic.rs:225-234,245`) | `semantic_memories` + `fact_history`, 2 indexes | `recall_facts` — `ORDER BY confidence DESC LIMIT 10`, **no query, no project scoping** (M-03) | Extraction has a reachable panic (M-01); dedup is broken (M-09) |
| Procedural | **Real** — file tree + SQLite mirror, both written by `save_topic` (`procedural.rs:125-154`) | `procedural_memories`, 2 indexes | `LIKE '%q%'` scan (M-08); with `q=""` it returns everything (M-02) | Storage is solid; retrieval is recency-only in practice |

### Embeddings / vector search

**None.** `grep -rn "embedding\|vector\|cosine\|faiss\|hnsw\|sqlite-vss\|sqlite-vec"` over
`src-tauri/src/memory/` returns **zero hits**. All three tiers are keyword/substring or
plain ordering. `routing/scorer.rs:2` states the same for routing: "No vector embeddings;
pure arithmetic."

---

## 6. Per-module narratives

---

## 6. Per-module narratives

### 6a. Provider layer

The four providers are four hand-written copies of the same shape. `streaming.rs` is
named as if it were the shared SSE layer, but it only contains the **Anthropic** parser
(`parse_anthropic_event`, `streaming.rs:29`) — OpenAI and Google each carry a private
duplicate of the line-buffering loop (`openai.rs:236-262`, `google.rs:202-228`, versus
`anthropic.rs:193-221`). The three loops are byte-identical; the three parsers diverge in
quality, and the two that were copied last are the two that are broken. This is the
structural cause of P-06 through P-10: a bug fixed in one parser does not reach the
others.

The buffering loop itself has a quadratic reallocation (`*buf = buf[pos + 1..].to_string()`
at `anthropic.rs:208`, `openai.rs:249`, `google.rs:215`) — every parsed line copies the
remainder of the buffer. Harmless at SSE line sizes; worth noting if a single tool-result
line grows to megabytes.

### 6b. `routing/`

`routing/` is a **task→worker** router, not a model router. It has three parts:
`patterns.rs` (5 hardcoded keyword lists → 5 hardcoded worker slugs), `scorer.rs`
(success-rate 0.6 + speed 0.4 over the last 200 `routing_history` rows), and `mod.rs`
(stateless facade that reopens `TasksDb` and `WorkersDb` per call, `mod.rs:41,44,65,87,103`).

It is genuinely implemented and wired: `agent/builtin_tools/suggest_worker_tool.rs:57`
exposes it as a tool, `commands/routing_commands.rs:11` as a Tauri command, and outcomes
are recorded from `commands/task_commands.rs:53-60`. Quality is limited by naive
substring matching (R-02, R-03).

**The Model Router from the product spec does not exist.** Model choice is a single
global UI setting (`commands/agent_commands.rs:33-40` reads `state.active_provider` /
`state.active_model`) plus per-worker YAML `model` fields. There is no role→model map, no
priority tier, no budget-aware downgrade, and no escalation path. `subagent/types.rs:65`
does hardcode `DEFAULT_MODEL = "claude-haiku-4-5"` for subagents — the one place in the
codebase where a cheap model is chosen for a cheap role, and it is a constant, not a
router.

### 6c. `budget/` and `cost/`

`budget/mod.rs` is 130 lines of well-formed, well-validated **types** — `TaskBudget`,
`BudgetStatus`, `BudgetExceededReason`, three presets, a ceiling-checked `validate_budget`.
The types are fine. The enforcement is not connected (B-01): `with_task_budget` has no
callers, so `task_budget` is always `None` and the guard at `agent/engine.rs:150-165` is
unreachable. The DB columns, the Tauri commands, the tool, the escalation helper, and the
`BudgetExceeded` event variant all exist and none of them are load-bearing.

`cost/tracker.rs` is 59 lines. The arithmetic is correct; the pricing table is not (C-01,
C-02) and it is a second, divergent source of truth from the provider tables (C-03).

### 6d. `subagent/`

`pool.rs` is the most carefully built file in the audited set: a global semaphore, a QC
sub-cap acquired before the global one, per-parent counts with rollback on both rejection
paths, depth limiting, and two unit tests. Its one defect is the deferred `Drop`
(S-06).

`manager.rs` is a clean registry with retention-based pruning — undermined by the fact
that `terminated_at` is only ever set from `await_result` (S-03), which turns the
carefully-built pool into a resource that leaks to deadlock on the fire-and-forget path.

`bridge.rs` is entirely a sidecar client (S-01). Its SSE framing (`find_frame_end`,
`parse_event_frame`) is more robust than the provider-layer parsers — it handles CRLF and
LF, multi-line `data:`, both `event:`-named and `type`-discriminated frames, and 6 event
kinds with aliases — and it is exercised by 5 unit tests (`bridge.rs:401-441`). It is also
the only tested parser in the whole audited surface.

### 6e. `session/`

**Lifecycle.** Create (`manager.rs:21-31`) → upsert row → append messages one at a time
(`manager.rs:52-54`) → delete (`manager.rs:48-50`). No explicit "close"; no final summary
(SE-05).

**What is saved.** `sessions`: id, title, working_dir, provider, model, pa_mode, timestamps.
`messages`: id, session_id, role, content (text), tool_name, tool_input, tool_call_id,
is_error, tokens_in/out, cost_usd, created_at. Tool calls **are** persisted as separate
`Tool`-role rows by `agent/tool_executor.rs:67,290`.

**What is lost on reload.** Everything tool-shaped (SE-01): `build_history` discards Tool
rows and reduces assistant turns to a single text string. So the data is in the DB and the
reconstruction throws it away. Also lost: extended thinking (never persisted at all — no
column), and all but the last text block of a multi-block assistant turn (SE-02).

**Token/cost on messages.** `Message.tokens_in/out/cost_usd` exist and are written as
literal zeros by `Message::new_assistant` (`types.rs:77-79`); `update_message` could fill
them in, but the live path at `agent_commands.rs:171-192` never calls it. Another
writer-with-no-real-data, matching M-05.

### 6f. `memory/`

Covered in §5. Structurally the most complete module in the audit — real schema, real
indexes, real dual file/DB storage for Tier 3, WAL enabled — and simultaneously the one
with the most silent-failure surface (`let _ =`, `.ok()`, `unwrap_or_default()`,
`filter_map(ok)` at 15+ sites).

---

## 7. Cross-platform portability

Target is a Tauri desktop app; Windows is the development platform (see the Windows-first
env-var ordering everywhere).

| Concern | file:line | Windows | macOS / Linux | Severity |
|---|---|---|---|---|
| Home dir resolution | `memory/db.rs:116-121`, `memory/procedural.rs:214-217`, `agent/context.rs:133-134`, `session/storage.rs:297-303` | OK (`USERPROFILE`) | Falls through to `HOME` — OK, **unless** `USERPROFILE` is exported (WSL interop does this), which silently redirects the data dir | MED (M-15) |
| Hand-rolled `dirs_next` | `session/storage.rs:296-303` | works | works | LOW — 4 copies of the same logic, comment admits "we can't add a dep mid-build" |
| App-data location | `~/.autodev/`, `~/.airs/` | Non-idiomatic (should be `%LOCALAPPDATA%`) | Idiomatic | LOW |
| Two separate DB roots | `~/.airs/autodev.db` (`session/storage.rs:293`) vs `~/.autodev/memory.db` (`memory/db.rs:122`) | same issue both platforms | same | LOW — inconsistent, easy to back up one and not the other |
| Path separators | `PathBuf::join` used throughout (`procedural.rs:218,222,226,235`) | OK | OK | none |
| Filename sanitisation | `memory/procedural.rs:229-236` — maps anything not `[A-Za-z0-9_-]` to `-` | OK; also blocks traversal | OK | none — this one is correct |
| Reserved Windows filenames | `procedural.rs:229-236` does **not** reject `CON`, `PRN`, `AUX`, `NUL`, `COM1`… | A topic named `con` creates `con.md` → open fails on Windows | fine | LOW, SUSPECTED |
| `canonicalize()` | `session/types.rs:164` | Produces `\\?\C:\...` UNC-prefixed paths on Windows | Produces clean paths | MED, SUSPECTED — the `\\?\` prefix leaks into `working_dir`, which is embedded verbatim into subagent system prompts (`subagent/bridge.rs:85`) and used for path comparison |
| Line endings in SSE | `provider/*.rs` `trim_end_matches('\r')` (`anthropic.rs:207`, `openai.rs:248`, `google.rs:214`); `bridge.rs:281-294` handles CRLF and LF | OK | OK | none |
| Ollama default host | `provider/ollama.rs:80` `http://localhost:11434` | OK | OK | n/a — being removed |
| `std::thread::spawn` for memory writes | `commands/agent_commands.rs:75`, `:182` | OK | OK | none (portable), but see M-07 for the locking consequence |

---

## 8. Ollama removal impact

Ollama is **cleanly removable**. It implements the shared `Provider` trait and nothing
else depends on it structurally: it exports no types the trait or other providers use, it
appears in no shared enum, and `provider/streaming.rs` (the only shared parser) contains
no Ollama code.

### Files to delete

- `src-tauri/src/provider/ollama.rs` (506 lines, whole file)

### Edits required — 10 sites across 8 files

| # | file:line | change |
|---|---|---|
| 1 | `provider/mod.rs:11` | remove `pub mod ollama;` |
| 2 | `provider/mod.rs:15` | remove `pub use ollama::OllamaProvider;` |
| 3 | `commands/agent_commands.rs:13` | remove `use crate::provider::ollama::OllamaProvider;` |
| 4 | `commands/agent_commands.rs:42-48` | remove the `if provider_id != "ollama"` key-skip branch; the key becomes unconditionally required |
| 5 | `commands/agent_commands.rs:133` | remove the `"ollama" => Arc::new(OllamaProvider::default_from_env()),` factory arm |
| 6 | `commands/diff_commands.rs:12,27,38` | same three edits |
| 7 | `commands/recipe_commands.rs:188,201,212` | same three edits |
| 8 | `recipes/runner.rs:45,640,653` | same three edits |
| 9 | `agent/builtin_tools/analyze_diff_tool.rs:12,224,235` | same three edits |
| 10 | `commands/settings_commands.rs:10,34,45,59-62` | remove the import, the `build_provider_info("ollama", "Ollama (Local)")` entry, the `models()` arm, and the whole "Ollama does not require an API key" early-return block |

### Dependency and behaviour changes

- **`Cargo.toml`**: `ollama-rs` becomes unused. Remove it. It is the only dependency
  pulled in solely by this provider (`ollama.rs:25-27` are its only import sites —
  `grep -rn "ollama_rs" src-tauri/src` returns matches only in `ollama.rs`). SUSPECTED
  on the exact crate-feature impact; I did not read `Cargo.toml`.
- **The `OLLAMA_HOST` env var** (`ollama.rs:54`) stops being read. If it is documented
  anywhere or set in a launch script, that becomes stale.
- **The "no API key required" special case disappears** from all five factory sites. This
  is a small simplification win: the `if provider_id != "ollama" { … } else { String::new() }`
  pattern is repeated verbatim five times and can collapse to a plain vault lookup.
- **Zero-cost models disappear**, which is a *good* thing for `cost/tracker.rs`: it
  currently prices every Ollama model at the $1.00/$3.00 fallback (C-02), so removing
  Ollama removes a class of wrong cost numbers.
- **The frontend must drop the Ollama entry** from its provider list (out of this audit's
  scope, but `commands/settings_commands.rs:34` is what feeds it).

### What is *not* affected

- `provider/traits.rs`, `provider/types.rs`, `provider/streaming.rs` — no Ollama
  references.
- `routing/`, `budget/`, `cost/`, `subagent/`, `session/`, `memory/` — zero references
  (`grep -rni "ollama"` over those directories returns nothing).

### Net

Removing Ollama deletes 506 lines, one dependency, five copies of a special case, and
one class of wrong pricing. It removes the **only** provider whose tool calling is a
non-functional stub (P-04) and the only one with an inert panic guard (P-05). **This is
the cheapest and highest-yield deletion in the restructure.** Meanwhile, the replacement
provider — xAI/Grok — does not exist at all (P-28), and adding it correctly means fixing
the OpenAI path first, since Grok is best implemented as a re-based `OpenAIProvider`.

---

## 9. Priority order for the restructure

1. **Prompt caching** (P-01, P-19) — the largest recurring cost, and it needs a type
   change (`ModelInfo` price fields) plus body-shape changes, so it must be designed in,
   not bolted on.
2. **Fix streaming usage reporting** (P-06, P-09, and Anthropic's missing `message_start`
   handler) — nothing downstream of it can be correct until it is: not cost, not budget,
   not analytics.
3. **One shared SSE layer** (§6a) — the three duplicated buffering loops and three
   divergent parsers are the structural cause of P-06 through P-12.
4. **Wire the budget guard** (B-01, B-02) — the feature is fully built and simply not
   connected. Cheapest large win in the audit.
5. **Fix `build_history`** (SE-01) — tool history loss on reload is invisible, expensive,
   and makes long sessions behave erratically.
6. **Then** add Grok (P-28) onto a repaired OpenAI path.

---

## 10. Coverage note

Read in full: all 8 files in `provider/`, all 3 in `routing/`, `budget/mod.rs`, both
`cost/` files, `subagent/{bridge,manager,pool}.rs`, all 4 `session/` files, all 9
`memory/` files. Read in part (for consumer tracing only): `agent/engine.rs`,
`agent/context.rs`, `commands/agent_commands.rs`, `commands/worker_commands.rs`,
`db/tasks.rs`, `state/app_state.rs`, `agent/builtin_tools/set_task_budget_tool.rs`.
Not read: `subagent/{types,slug}.rs` beyond targeted greps, `Cargo.toml`, the frontend.
No code was modified; nothing was built or run.

