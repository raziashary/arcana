# G6 — Agent SDKs & Agent-Native API Features: Anthropic / OpenAI / Google / xAI

**Verified on 2026-08-31.** Every factual claim below carries a source URL. Claims that could not be
confirmed against a primary (vendor) source are marked **UNVERIFIED** in bold and must not be built on.

**Purpose.** The desktop app (Tauri, Rust backend) is retiring its Python sidecar. All agent capability
moves in-house into a **native Rust agent runtime that is SDK-shaped** — it conforms to the semantics of
the vendors' agent SDKs — and **adopts server-side / API-native agent features wherever they genuinely
help**, since those features are reachable over plain HTTP without the Python/TS SDKs. This document is
the evidence base for deciding *which* server-side features to adopt.

**Method.** Vendor documentation was fetched live on 2026-08-31 and preferred over blog posts. Crate
versions and release dates were read from the crates.io API on 2026-08-31, not from memory. Where a
vendor README and the registry disagreed (this happened with `rig`), the registry is cited.

**How to read the verdicts.** Section C of each provider ends in an ADOPT / ADOPT-LATER / SKIP verdict per
feature. The cross-provider synthesis (Part 5) ranks them. The single highest-leverage finding is in
§5.3 line 1.

---

## Table of contents

- Part 1 — Anthropic (Claude)
- Part 2 — OpenAI
- Part 3 — Google (Gemini)
- Part 4 — xAI (Grok)
- Part 5 — Cross-provider synthesis (common denominator, divergence, adoption shortlist, traps)
- Part 6 — Rust ecosystem reality check and the crate-vs-hand-roll recommendation

---

# PART 1 — ANTHROPIC (CLAUDE)

## 1.A The official agent SDK

Anthropic ships **three distinct surfaces**, and conflating them is the first trap. They differ on two
axes: *who supplies the agent harness* and *who supplies the deployment*.

| Surface | Package | Languages | What it gives you |
|---|---|---|---|
| **Client SDKs** (Messages API) | `anthropic`, `@anthropic-ai/sdk`, … | Python, TypeScript, C#, Go, Java, PHP, Ruby | Plain Messages API client. You write the tool loop. |
| **Tool Runner** (beta helper inside the Client SDK) | same packages, `client.beta.messages.tool_runner` | all seven | Runs the tool-call loop over *your* tools. Harness only; you host. |
| **Claude Agent SDK** (separate product — Claude Code as a library) | `claude-agent-sdk` (PyPI), `@anthropic-ai/claude-agent-sdk` (npm) | **Python and TypeScript only** | Full Claude Code harness: built-in tools, subagents, hooks, permissions, sessions, context management. Harness only; you host. |
| **Claude Managed Agents** (REST, beta) | `/v1/agents`, `/v1/sessions`, `/v1/environments` | any HTTP client | Anthropic runs the loop **and** hosts a per-session container. |

Sources:
[CLI, SDKs and libraries](https://platform.claude.com/docs/en/cli-sdks-libraries/overview) ·
[Agent SDK overview](https://code.claude.com/docs/en/agent-sdk/overview) ·
[Managed Agents overview](https://platform.claude.com/docs/en/managed-agents/overview)

**Rust: no.** Anthropic's official client SDK list is explicitly "Python, TypeScript, C#, Go, Java, PHP,
and Ruby" — Rust is absent
([source](https://platform.claude.com/docs/en/cli-sdks-libraries/overview)). The Claude Agent SDK page
states plainly: *"The SDK is available as a library for Python and TypeScript only. To drive the same agent
loop from another language, run the CLI as a subprocess with the `-p` flag and `--output-format json`"*
([source](https://code.claude.com/docs/en/agent-sdk/overview)).

**Verdict for the Rust team:** there is **no first-party Rust path**. Community Rust crates exist and are
audited in Part 6 (`anthropic-ai-sdk` 0.2.27, `clust` 0.9.0, `misanthropy` 0.0.8, `anthropic-sdk` 0.1.5) —
all are low-download, none is current with the 2026 API surface. The Rust runtime must speak
`POST /v1/messages` directly. Everything Anthropic-side in §1.C **is** reachable over plain HTTP, which is
the good news.

Agent SDK package versions as of Aug 2026: Python `claude-agent-sdk` **0.2.139**, TypeScript
`@anthropic-ai/claude-agent-sdk` **0.3.233**, both on weekly release cadence — **UNVERIFIED against
PyPI/npm directly**; sourced from a secondary write-up
([npm page](https://www.npmjs.com/package/@anthropic-ai/claude-agent-sdk)). Treat the exact numbers as
indicative; the *languages* claim is verified from vendor docs.

## 1.B Agent-loop semantics — what "SDK-shaped" means for Anthropic

### 1.B.1 The core loop

The loop is driven entirely by `stop_reason`. The Messages API is **stateless**: you resend the full
`messages` array every turn ([source](https://platform.claude.com/docs/en/agents-and-tools/tool-use/overview)).

```
loop {
  resp = POST /v1/messages { model, max_tokens, system, tools, messages }
  append { role: "assistant", content: resp.content } to messages      // FULL content, not just text
  match resp.stop_reason {
    "tool_use"  => execute every tool_use block; append ONE user message
                   containing ALL tool_result blocks; continue
    "pause_turn" => resend as-is to resume (server tools, long-running turns)
    "end_turn" | "stop_sequence" => done
    "max_tokens" => output was truncated — raise max_tokens or stream
    "refusal"    => safety decline; read resp.stop_details
  }
}
```

`stop_reason` values: `end_turn`, `max_tokens`, `stop_sequence`, `tool_use`, `pause_turn`, `refusal`
([source](https://platform.claude.com/docs/en/build-with-claude/handling-stop-reasons)). `stop_details`
is populated **only** when `stop_reason == "refusal"` and is `null` otherwise — guard before reading it
(skill reference `claude-api`, § Other API Surfaces; corroborated by the streaming/stop-reason docs).

**Iteration limits: there are none server-side.** Anthropic does not impose a turn cap. The cap is yours.
The nearest server-side concept is **task budgets** (beta `task-budgets-2026-03-13`, Opus 5 / Fable 5 /
Sonnet 5 / Opus 4.8 / 4.7): `output_config.task_budget = { type: "tokens", total: N }`, minimum
`total` 20,000. The server injects a countdown marker the model can see, so it *paces itself and finishes
gracefully* rather than being cut off. This is **advisory**, not enforced — unlike `max_tokens`, which is a
hard per-response ceiling the model is unaware of. Leave `remaining` unset in a normal loop; pass it only
when you rewrite history between requests
([source](https://platform.claude.com/docs/en/about-claude/models/migration-guide)).

Design note for the Rust port: keep an explicit `max_iterations` counter in the runtime (the Python sidecar
already had one — see G4 §1.8) **and** set a task budget. They solve different problems: the counter is a
runaway guard, the task budget is a quality/cost shaping signal.

### 1.B.2 Tool definition and the tool-call → tool-result wire shape

Tool definition (client tool):

```json
{
  "name": "get_weather",
  "description": "Get the current weather for a given location.",
  "input_schema": {
    "type": "object",
    "properties": { "location": { "type": "string", "description": "City and state" } },
    "required": ["location"]
  },
  "strict": true,                       // optional; guarantees schema conformance
  "defer_loading": false,               // optional; used with the tool-search tool
  "allowed_callers": ["code_execution_20260120"]   // optional; programmatic tool calling
}
```

Assistant emits:

```json
{ "type": "tool_use", "id": "toolu_01A09q90qw90lq917835lq9",
  "name": "get_weather", "input": { "location": "San Francisco, CA" } }
```

You reply in a **user** message:

```json
{ "type": "tool_result", "tool_use_id": "toolu_01A09q90qw90lq917835lq9",
  "content": "15 degrees Celsius, partly cloudy", "is_error": false }
```

`tool_choice`: `{"type":"auto"}` (default), `{"type":"any"}`, `{"type":"tool","name":"..."}`,
`{"type":"none"}`. `disable_parallel_tool_use: true` nests **inside** the `tool_choice` object, not at top
level ([source](https://platform.claude.com/docs/en/agents-and-tools/tool-use/overview)).

Note the asymmetry that bites porters: the correlation key is named `id` on the way out and `tool_use_id`
on the way back, and results go in a **user** message, not a dedicated `tool` role.

### 1.B.3 Parallel tool calls

On by default. One assistant message may carry several `tool_use` blocks. **All** corresponding
`tool_result` blocks must be returned in a **single** user message — splitting them across multiple user
messages silently trains Claude to stop making parallel calls. A failed tool still gets a `tool_result`
with `is_error: true`; dropping it breaks the pairing
([source](https://platform.claude.com/docs/en/agents-and-tools/tool-use/parallel-tool-use)).

### 1.B.4 Handoffs / sub-agents

The Messages API has **no** handoff primitive. Multi-agent delegation exists in two other places:

- **Claude Agent SDK subagents** — spawn specialised agents for focused subtasks
  ([source](https://code.claude.com/docs/en/agent-sdk/subagents)). Python/TS only.
- **Managed Agents multi-agent sessions** — a roster including `{"type":"self"}` or another agent ID; the
  orchestrator delegates reading-heavy sub-tasks to cheaper workers
  ([source](https://platform.claude.com/docs/en/managed-agents/multiagent-orchestration)).
- **Advisor tool** (server tool) — a faster executor model consults a higher-intelligence advisor
  mid-generation. The advisor's `model` must be at least as capable as the request's top-level `model`, or
  the API returns 400
  ([source](https://platform.claude.com/docs/en/agents-and-tools/tool-use/advisor-tool)).

For the Rust runtime, sub-agents are a **runtime-side construct**: a sub-agent is just a nested loop with
its own `messages`/`tools` and its result summarised back into the parent's tool_result. Nothing in the
wire protocol needs to change.

### 1.B.5 Sessions / conversation state

Messages API: **none**. Stateless, full history every request. The Anthropic answer to token blowup is not
server-side session state — it is **prompt caching + context editing + compaction** (§1.C).

Managed Agents does have server-side sessions with lifecycle states (`running`, `idle`, `rescheduling`,
`terminated`), an SSE event stream, and per-session containers
([source](https://platform.claude.com/docs/en/managed-agents/sessions)). Billed at **$0.08 per
session-hour** of `running` time on top of tokens
([source](https://platform.claude.com/docs/en/about-claude/pricing)).

### 1.B.6 Structured output / JSON schema enforcement

`output_config.format` — **GA, no beta header** (the older `output_format` param and the
`structured-outputs-2025-11-13` header are deprecated):

```json
{ "output_config": { "format": {
    "type": "json_schema",
    "schema": { "type":"object", "properties": {...}, "required": [...], "additionalProperties": false }
}}}
```

Supported on every current model (Fable 5, Opus 5, Opus 4.8/4.7/4.6/4.5, Sonnet 5, Sonnet 4.6/4.5,
Haiku 4.5, Mythos 5).

**Schema subset — this is the part that will bite.** Supported: object/array/string/integer/number/
boolean/null, `enum` (scalars only), `const`, `anyOf`, `allOf` (but **not** `allOf` with `$ref`), internal
`$ref`/`$defs`, `required`, `additionalProperties: false`, string `format` (`date-time`, `time`, `date`,
`duration`, `email`, `hostname`, `uri`, `ipv4`, `ipv6`, `uuid`), array `minItems` (only 0 or 1).
**Not** supported: **recursive schemas**, complex types inside `enum`, external `$ref`, numeric
constraints (`minimum`/`maximum`/`multipleOf`), string constraints (`minLength`/`maxLength`), array
constraints beyond `minItems`, `additionalProperties` set to anything but `false`. Unsupported features
return **400** ([source](https://platform.claude.com/docs/en/build-with-claude/structured-outputs)).

`strict: true` on a tool definition is a **separate** feature that enforces the same schema discipline on
tool *parameters*. The two compose. Grammar compilation adds latency on the first request; compiled
grammars are cached for 24 hours, invalidated by schema-structure or tool-set changes. Changing
`output_config.format` invalidates the **prompt cache**; changing only the schema `name`/`description`
does not ([same source](https://platform.claude.com/docs/en/build-with-claude/structured-outputs)).

Structured outputs are **incompatible with document `citations`** — combining them returns 400.

### 1.B.7 Streaming event taxonomy

Set `"stream": true`. SSE, each event carries an SSE event name and a matching `type` in the data.
Canonical order ([source](https://platform.claude.com/docs/en/build-with-claude/streaming)):

1. `message_start` — a `Message` object with **empty** `content`; **`usage` including cache fields lands here**
2. per content block, in `index` order:
   `content_block_start` → 0..n `content_block_delta` → `content_block_stop`
3. one or more `message_delta` — top-level changes (`stop_reason`, `stop_sequence`) and **cumulative** `usage`
4. `message_stop`

Plus `ping` (any number, anywhere) and `error` (e.g. `overloaded_error`, the streaming form of HTTP 529).

`content_block_delta` delta types:

| `delta.type` | Field | Notes |
|---|---|---|
| `text_delta` | `text` | plain text |
| `input_json_delta` | `partial_json` | **partial JSON string fragments**; the final `tool_use.input` is an object. Accumulate and parse at `content_block_stop`. Current models emit one complete key/value at a time, so expect stalls between events. |
| `thinking_delta` | `thinking` | only when `display: "summarized"` |
| `signature_delta` | `signature` | sent once, immediately before `content_block_stop` on a thinking block |
| `compaction_delta` | `content` | the whole summary in one delta (no incremental streaming) |

Two edge cases the Rust parser must handle:
- With `display: "omitted"` (the **default** on Fable 5 / Mythos 5 / Opus 5 / 4.8 / 4.7 / Sonnet 5) a
  thinking block opens, receives **only** a `signature_delta`, and closes — zero `thinking_delta`s. A UI
  that waits for text sees a long silent pause.
- During server-side fallback a `fallback` content block arrives as a `content_block_start` /
  `content_block_stop` **pair with no deltas between them**.

The docs explicitly warn that new event types may be added and that clients must handle unknown event
types gracefully. In Rust: `#[serde(other)]` catch-all variants on every event enum, non-negotiable.

### 1.B.8 Guardrails / interception / middleware

Nothing on the Messages API wire. The Tool Runner exposes per-turn hooks (approval gates, error
interception, result modification such as attaching `cache_control`, retries)
([source](https://platform.claude.com/docs/en/agents-and-tools/tool-use/tool-runner)); the Agent SDK has
`hooks` and `permissions` ([source](https://code.claude.com/docs/en/agent-sdk/hooks)); Managed Agents has
**permission policies** with `allow` / `deny` / `confirm` per tool
([source](https://platform.claude.com/docs/en/managed-agents/permission-policies)).

For the Rust runtime this is entirely a local concern: a `ToolInterceptor` trait around tool dispatch
covering pre-call approval, post-call result rewrite, and error mapping reproduces all of it.

### 1.B.9 Memory / state primitives

- **Memory tool** — Anthropic-schema *client* tool: `{"type": "memory_20250818", "name": "memory"}`, no
  `input_schema`. Claude issues file-like commands; **your** code implements the backing store
  ([source](https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool)).
- **Managed Agents memory stores** — server-side, with `memory_sync_interval` / `memory_sync_deletes`
  ([source](https://platform.claude.com/docs/en/managed-agents/memory)).

The memory tool is the interesting one for this app: it is a wire contract Anthropic trained the model on,
and the storage is ours. That maps cleanly onto the existing three-tier memory (G4).

### 1.B.10 Retries, error taxonomy, rate limits

Errors: 400 `invalid_request_error`, 401, 403, 404, 413, 429 `rate_limit_error`, 500, **529
`overloaded_error`** ([source](https://platform.claude.com/docs/en/api/errors)). Retryable: 408, 409, 429,
5xx, connection errors — that is the official SDKs' own retry set (default `max_retries` = 2). Note that
timeouts are retried, so worst-case wall clock is `timeout × (max_retries + 1)`.

Rate-limit response headers ([source](https://platform.claude.com/docs/en/api/rate-limits)):

```
retry-after                                    (seconds; ABSENT on the spend-cap 429)
anthropic-ratelimit-requests-limit|-remaining|-reset
anthropic-ratelimit-tokens-limit|-remaining|-reset
anthropic-ratelimit-input-tokens-limit|-remaining|-reset
anthropic-ratelimit-output-tokens-limit|-remaining|-reset
anthropic-priority-input-tokens-*  / anthropic-priority-output-tokens-*   (Priority Tier only)
```
`*-reset` values are **RFC 3339 timestamps**, not seconds. Token remaining counts are rounded to the
nearest thousand.

**The single most important rate-limit fact for this app:** for all current models,
`cache_read_input_tokens` do **not** count toward ITPM. Only `input_tokens + cache_creation_input_tokens`
do. With a 2,000,000 ITPM limit and an 80% cache hit rate you can push ~10,000,000 total input tokens per
minute. (Only retired Claude Haiku 3.5 counts cache reads toward ITPM.) `max_tokens` does **not** factor
into OTPM, so there is no rate-limit reason to lowball it
([source](https://platform.claude.com/docs/en/api/rate-limits)).

Two 429s that look identical and must be told apart: a real rate limit (has `retry-after`, retry works) and
a spend-cap hit (`error.details.error_code == "enforced_spend_limit_reached"`, **no** `retry-after`,
retrying fails until the next month). A blind exponential-backoff loop will spin forever on the second one
([source](https://platform.claude.com/docs/en/api/rate-limits)).

## 1.C Server-side / API-native features usable over plain HTTP

### 1.C.1 Prompt caching — **ADOPT, first, above everything else**

All of it is plain HTTP on `POST /v1/messages`; the only headers needed are `x-api-key`,
`anthropic-version: 2023-06-01`, `content-type`
([source](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)).

Two invocation modes:
- **Automatic** — a single top-level `"cache_control": {"type":"ephemeral"}` on the request; the API manages
  breakpoints as the conversation grows. Recommended starting point.
- **Explicit** — `cache_control` on individual content blocks, max **4 breakpoints** per request. A 5th
  (including an automatic one on top of 4 explicit) returns 400.

TTL and price:

| Operation | Multiplier vs base input | Duration |
|---|---|---|
| 5-minute cache write (`ttl` omitted or `"5m"`) | **1.25×** | 5 min, refreshed on each hit |
| 1-hour cache write (`"ttl": "1h"`) | **2.0×** | 1 hour |
| Cache read (hit) | **0.1×** | — |

Break-even: 5m cache pays off after **one** read; 1h cache after **two**
([source](https://platform.claude.com/docs/en/about-claude/pricing)). Mixing TTLs: 1-hour entries must
appear **before** 5-minute entries in the `tools → system → messages` order.

Minimum cacheable prefix, per model — **shorter prefixes silently do not cache and return no error**:

| Model | Minimum tokens |
|---|---|
| Claude Opus 5 / Fable 5 / Mythos 5 | 512 |
| Claude Opus 4.8 | 1,024 |
| Claude Opus 4.7 | 2,048 |
| Claude Opus 4.6 / 4.5 | 4,096 |
| Claude Sonnet 5 / 4.6 / 4.5 | 1,024 |
| Claude Haiku 4.5 | 4,096 |

Invalidation. Caching is a **prefix match over `tools → system → messages`**; a change at any level
invalidates that level and every level after it. Specific triggers documented: tool definitions (kills all
three), web-search toggle / citations toggle / `speed` setting (kills tools onward), `tool_choice` and
images (kill system onward), thinking and `effort` settings (model-specific). Requires **100% exact byte
match**. Caches are workspace- and organisation-scoped. A **20-block lookback window** applies: the system
checks at most 20 blocks before your breakpoint for a hit.

Verification — the only honest check:
```
usage.input_tokens                  // tokens AFTER the last cache breakpoint (small when caching works)
usage.cache_creation_input_tokens   // written
usage.cache_read_input_tokens       // read
usage.cache_creation.ephemeral_5m_input_tokens / .ephemeral_1h_input_tokens   // 1h TTL breakdown
total_input_tokens = cache_read + cache_creation + input_tokens
```
If `cache_read_input_tokens` is zero across repeated same-prefix requests, a silent invalidator is at work
(a timestamp in the system prompt, non-deterministic JSON key order, a varying tool list).

Extras worth wiring: **pre-warming** with `max_tokens: 0` writes the cache and bills zero output tokens;
**cache diagnostics** (beta `cache-diagnosis-2026-04-07`) returns `response.diagnostics` when you pass
`diagnostics: {previous_message_id: ...}`.

> **Verdict: ADOPT — highest priority.** Direct 90% cut on the repeated prefix *and* cache reads are free
> against ITPM. For an app with a token-blowup problem this is the single largest lever available on any of
> the four providers.

### 1.C.2 Server-side conversation state — **SKIP** (Messages API) / **SKIP for now** (Managed Agents)

The Messages API has none. Managed Agents does, but it is a different product with its own control plane
(`/v1/agents`, `/v1/environments`, `/v1/sessions`), Anthropic-hosted containers, and a $0.08/session-hour
runtime charge. It replaces the Rust runtime's job rather than helping it, and it is Claude-only.

> **Verdict: SKIP.** Anthropic's token-blowup answer is caching + context editing, not session state.

### 1.C.3 Server-hosted tools — **ADOPT selectively**

Declared in `tools`, results come back as content blocks in the same response; no client execution
([source](https://platform.claude.com/docs/en/agents-and-tools/tool-use/overview)).

| Tool | `type` | Cost beyond tokens |
|---|---|---|
| Web search | `web_search_20260209` (older: `web_search_20250305`) | **$10 / 1,000 searches** |
| Web fetch | `web_fetch_20260209` (older: `web_fetch_20250910`) | **free** |
| Code execution | `code_execution_20260521` (also `code_execution_20260120`) | **free when used with web search/fetch**; otherwise 1,550 free container-hours/org/month, then $0.05/hour/container, 5-minute minimum |
| Tool search | `tool_search_tool_regex_20251119`, `..._bm25_20251119` | tokens only |
| Advisor | server tool | tokens only |
| MCP connector | `mcp_servers` + `{"type":"mcp_toolset"}`, beta `mcp-client-2025-11-20` | tokens only |
| Computer use / browser use | `computer_toolset_20260801`, `browser_toolset_20260801` | ~4,500 / ~6,600 input tokens of toolset overhead per request |

Prices from [Pricing § Specific tool pricing](https://platform.claude.com/docs/en/about-claude/pricing).

Two gotchas. (1) `web_search_20260209` / `web_fetch_20260209` run code execution *internally* for dynamic
filtering — do **not** also declare `code_execution` or the model gets two execution environments. (2)
**Server-tool errors do not raise.** They return HTTP 200 with a result block whose `content` is an error
object (`{error_code: "max_uses_exceeded"}`). For web search a success `content` is a *list* and an error
`content` is an *object* — branch on that before indexing.

> **Verdict: ADOPT web_fetch** (free, removes a whole class of local HTTP-fetching tool code) and
> **web_search** where the $10/1k is acceptable. **ADOPT tool_search + `defer_loading`** if the tool
> catalogue grows past a few dozen — it is a direct attack on tool-definition token bloat. **SKIP
> computer/browser use** for now (4.5–6.6k tokens of overhead per request).
> Constraint: at least one tool must have `defer_loading` unset, and the search tool itself must not be
> deferred, or the API returns 400 `All tools have defer_loading set`.

### 1.C.4 Batch API — **ADOPT for offline work only**

`POST /v1/messages/batches` with `requests: [{custom_id, params}]`, poll `processing_status` until
`"ended"`, then stream results. **50% off both input and output**, and it stacks with prompt caching.
Results arrive in **any order** — key by `custom_id`, never by position. Limits: 100,000 requests per
batch; 200k/300k/500k in-queue depending on tier
([source](https://platform.claude.com/docs/en/about-claude/pricing) ·
[rate limits](https://platform.claude.com/docs/en/api/rate-limits)). Not compatible with fast mode; the
`fallbacks` parameter is rejected on batches.

> **Verdict: ADOPT** for any non-interactive path (bulk summarisation, indexing, evals). **SKIP** for the
> interactive agent loop — it is asynchronous by construction.

### 1.C.5 Context management: editing and compaction — **ADOPT both**

These are two *different* features and the docs are emphatic about not confusing them.

**Context editing** (*clears*, does not summarise). Beta header `context-management-2025-06-27`, all
current models ([source](https://platform.claude.com/docs/en/build-with-claude/context-editing)):

```json
{"context_management": {"edits": [
  {"type": "clear_thinking_20251015", "keep": {"type": "thinking_turns", "value": 2}},
  {"type": "clear_tool_uses_20250919",
   "trigger":        {"type": "input_tokens", "value": 30000},   // default 100,000
   "keep":           {"type": "tool_uses",    "value": 3},       // default 3
   "clear_at_least": {"type": "input_tokens", "value": 5000},
   "exclude_tools":  ["web_search"],
   "clear_tool_inputs": false}
]}}
```
`clear_thinking_20251015` must be **first** in the `edits` array when both are used. Response carries
`context_management.applied_edits[]` with `cleared_tool_uses`, `cleared_thinking_turns`,
`cleared_input_tokens`; on streaming it lands in the final `message_delta`.

**Cache interaction — this is the trap.** Clearing tool results **invalidates the cached prefix** and
incurs a cache write. `clear_at_least` exists precisely to make sure a clear is worth the re-write. Keeping
thinking blocks preserves the cache; clearing them invalidates at the clear point.

**Compaction** (*summarises*). Beta header `compact-2026-01-12`; edit type `compact_20260112`; models
Fable 5, Mythos 5/preview, Opus 5, Opus 4.8/4.7/4.6, Sonnet 5, Sonnet 4.6
([source](https://platform.claude.com/docs/en/build-with-claude/compaction)). Trigger defaults to
`{"type":"input_tokens","value":150000}`; only `input_tokens` is a valid trigger type and the minimum is
50,000. The response contains a `{"type":"compaction","content":"…"}` block; you **must append the whole
`response.content`** (not just the text) back into `messages`, and the API then drops everything before the
compaction block automatically.

Streaming: `content_block_start` with `type: "compaction"`, one `content_block_delta` with
`type: "compaction_delta"` carrying the entire summary, then `content_block_stop`.

Billing changes shape: `usage.iterations[]` appears with per-iteration entries (`{"type":"compaction",…}`,
`{"type":"message",…}`). **Top-level `input_tokens`/`output_tokens` reflect only the non-compaction
iterations** — cost tracking that reads only the top-level fields will under-report. Re-applying an
existing compaction block costs nothing extra.

> **Verdict: ADOPT BOTH.** `clear_tool_uses` is the direct fix for the classic agent failure mode (a
> 200-line file dump from turn 3 still being resent at turn 30). Compaction is the safety net for
> long-running sessions. Order matters: cache first, then clear, then compact.

### 1.C.6 Context windows and long context — **no action needed**

Fable 5 / Opus 5 / Sonnet 5: **1M tokens**, 128K max output. Haiku 4.5: 200K / 64K. Claude 4.6 and later
include the **full 1M window at standard pricing** — a 900k request bills at the same per-token rate as a
9k one; caching and batch discounts apply across the whole window
([source](https://platform.claude.com/docs/en/about-claude/pricing) ·
[models overview](https://platform.claude.com/docs/en/about-claude/models/overview)). On the Batch API,
Opus 5 / Sonnet 5 / Opus 4.8 / 4.7 / 4.6 / Sonnet 4.6 support up to **300k output tokens** with beta
header `output-300k-2026-03-24`.

**Tokenizer warning.** Claude 4.7 and later use a newer tokenizer producing **~30% more tokens** for the
same text than Sonnet 4.6 and earlier. Do not carry token estimates across that boundary; re-count
([source](https://platform.claude.com/docs/en/build-with-claude/token-counting)).

### 1.C.7 Reasoning / thinking controls and token accounting — **ADOPT**

Current models use **adaptive thinking**: `thinking: {type: "adaptive", display: "summarized"|"omitted"}`.
`budget_tokens` is **removed** on Fable 5 / Opus 5 / 4.8 / 4.7 / Sonnet 5 — sending it returns **400**. It
is deprecated (still functional) on Opus 4.6 / Sonnet 4.6 only. Depth is controlled by
`output_config.effort` ∈ `low | medium | high | xhigh | max`, default `high`.

- Thinking tokens are billed as **output tokens** and count the same under every `display` setting; the raw
  chain of thought is never returned.
- `display` defaults to **`"omitted"`** on Fable 5 / Mythos 5 / Opus 5 / 4.8 / 4.7 / Sonnet 5 — a silent
  change from Opus 4.6 / Sonnet 4.6 where it was `"summarized"`.
- Thinking blocks must be **echoed back unchanged** when continuing on the same model. Other models drop
  them silently and unbilled.
- Thinking blocks from *previous* assistant turns do **not** count toward input tokens; the *current*
  turn's does ([source](https://platform.claude.com/docs/en/build-with-claude/token-counting)).
- Sampling parameters (`temperature`, `top_p`, `top_k`) are **removed** — 400 — on Fable 5, Opus 5, 4.8,
  4.7, Sonnet 5.

Sources: [adaptive thinking](https://platform.claude.com/docs/en/build-with-claude/adaptive-thinking) ·
[effort](https://platform.claude.com/docs/en/build-with-claude/effort).

> **Verdict: ADOPT `effort` as a per-role routing knob.** It is the first quality-for-cost lever after
> caching, it is free to use, and low effort on a current model often beats high effort on a prior one.
> **Trap: a mid-conversation `effort` change invalidates the messages cache** — pick effort per route and
> hold it for the session.

### 1.C.8 Files API — **ADOPT-LATER**

Beta header `files-api-2025-04-14` on **both** the upload and the referencing `messages.create`. Upload to
`POST /v1/files`, reference as `{"type":"document","source":{"type":"file","file_id":"..."}}` (or `image`
for images — the block type must match the MIME type)
([source](https://platform.claude.com/docs/en/build-with-claude/files)). Files API has its own
per-organisation rate limit, separate from Messages. Inline base64 PDFs need no beta: 32 MB request cap,
600 pages (100 for 200k-context models), and the base64 string must contain no newlines.

> **Verdict: ADOPT-LATER.** Worth it once the same document is sent across many requests; not on the
> critical path for the agent loop.

### 1.C.9 Token counting — **ADOPT**

`POST /v1/messages/count_tokens`, same body shape as `/v1/messages` (system, tools, images, PDFs,
`context_management`). Returns `{"input_tokens": N}`. **Free**, with its own RPM limit (2,000 / 4,000 /
8,000 by tier) that is independent of the Messages limit. It does **not** apply caching logic — passing
`cache_control` there changes nothing
([source](https://platform.claude.com/docs/en/build-with-claude/token-counting)).

With `context-management-2025-06-27` it also returns `context_management.original_input_tokens` alongside
the post-clearing `input_tokens`, which is how you preview an edit strategy before committing to it.

> **Verdict: ADOPT.** Free, exact, tokenizer-correct across the 4.7 tokenizer change, and the only
> defensible way to do pre-flight budget checks and model routing. Do **not** use `tiktoken-rs` for Claude.

## 1.D Models and pricing (2026-08-31)

[Models overview](https://platform.claude.com/docs/en/about-claude/models/overview) ·
[Pricing](https://platform.claude.com/docs/en/about-claude/pricing)

| Model | API ID | Context | Max out | Input $/MTok | 5m write | 1h write | Cache read | Output $/MTok |
|---|---|---|---|---|---|---|---|---|
| Claude Fable 5 | `claude-fable-5` | 1M | 128K | 10 | 12.50 | 20 | 1.00 | 50 |
| Claude Mythos 5 (limited) | `claude-mythos-5` | 1M | 128K | 10 | 12.50 | 20 | 1.00 | 50 |
| **Claude Opus 5** | `claude-opus-5` | 1M | 128K | **5** | 6.25 | 10 | **0.50** | **25** |
| Claude Opus 4.8 | `claude-opus-4-8` | 1M | 128K | 5 | 6.25 | 10 | 0.50 | 25 |
| Claude Opus 4.7 | `claude-opus-4-7` | 1M | 128K | 5 | 6.25 | 10 | 0.50 | 25 |
| Claude Opus 4.6 | `claude-opus-4-6` | 1M | 128K | 5 | 6.25 | 10 | 0.50 | 25 |
| **Claude Sonnet 5** | `claude-sonnet-5` | 1M | 128K | **2** | 2.50 | 4 | **0.20** | **10** |
| Claude Sonnet 4.6 | `claude-sonnet-4-6` | 1M | 128K | 3 | 3.75 | 6 | 0.30 | 15 |
| **Claude Haiku 4.5** (cheap/fast tier) | `claude-haiku-4-5` | **200K** | 64K | **1** | 1.25 | 2 | **0.10** | **5** |

Batch = exactly 50% of base on both directions (e.g. Opus 5 $2.50 / $12.50; Sonnet 5 $1.00 / $5.00;
Haiku 4.5 $0.50 / $2.50). Fast mode (research preview, Opus 5 / Opus 4.8 only, Claude API only):
$10 / $50 with up to ~2.5× output tokens/sec; it has its own rate-limit pool, and **switching `speed`
invalidates the prompt cache**. `inference_geo: "us"` applies a **1.1× multiplier** on every token
category.

Notable for routing: **Sonnet 5's $2/$10 introductory pricing became the standard price** — the September
1, 2026 increase to $3/$15 will not happen
([source](https://platform.claude.com/docs/en/about-claude/pricing)). Haiku 4.5 is the only current model
with a 200K (not 1M) window and no `effort` support, and it needs a **4,096-token** prefix to cache.

Retirement floors: Fable 5 ≥ 2027-06-09, Opus 5 ≥ 2027-07-24, Sonnet 5 ≥ 2027-06-30,
Haiku 4.5 ≥ **2026-10-15** (soonest — plan the cheap-tier replacement).

## 1.E Rust ecosystem for Anthropic

No official SDK. See Part 6 for the full table; summary: `anthropic-ai-sdk` 0.2.27 (2026-01-11, 44k
downloads) is the most current community crate and is already 7+ months stale against an API surface that
shipped compaction, structured outputs GA, adaptive thinking, and task budgets in that window. `clust`
0.9.0 (2024-06-30) and `anthropic-sdk` 0.1.5 (2024-07-23) are abandoned.

---

# PART 2 — OPENAI

## 2.A The official agent SDK

**OpenAI Agents SDK**, available in **Python and TypeScript only**
([source](https://developers.openai.com/api/docs/guides/agents)):

| Package | Registry | Latest version |
|---|---|---|
| `openai-agents` | PyPI | **0.22.0** (requires Python ≥ 3.10) — [pypi](https://pypi.org/project/openai-agents/) |
| `@openai/agents` | npm | **0.17.0** — [npm](https://www.npmjs.com/package/@openai/agents) |

Client SDKs (not the agent framework) exist for JavaScript/TypeScript, Python, .NET/C#, Java, Go, Ruby,
plus a CLI ([source](https://developers.openai.com/api/docs/libraries)).

**Rust: no official SDK.** OpenAI's own libraries page lists `async-openai` (by
[64bit](https://github.com/64bit/async-openai)) as a **community** library and states OpenAI "does not
verify the correctness or security of these projects"
([source](https://developers.openai.com/api/docs/libraries)). `async-openai` 0.41.3 (2026-07-31, 7.68M
downloads) is by a wide margin the healthiest LLM crate in the Rust ecosystem — see Part 6.

The Agents SDK builds on the **Responses API**, which is the surface a Rust client actually talks to. The
docs are explicit about the split: use the Responses API "when you need direct control over model
interactions and custom orchestration"; use the Agents SDK "when you want the SDK to manage the agent loop"
([source](https://developers.openai.com/api/docs/guides/agents)). The Rust runtime takes the first branch.

## 2.B Agent-loop semantics

### 2.B.1 The core loop

From the Agents SDK runner ([source](https://openai.github.io/openai-agents-python/running_agents/)):

1. Call the LLM for the current agent with the current input.
2. Classify the output:
   - **final output** (text of the desired type, no tool calls) → loop ends, return result
   - **handoff** → switch the current agent, re-run
   - **tool calls** → execute, append results, re-run
3. If `max_turns` is exceeded → raise `MaxTurnsExceeded`.

`max_turns` is a parameter of `Runner.run` / `run_sync` / `run_streamed`; passing `max_turns=None` disables
the cap. The **default value is contested across sources** (5 for the Runner vs a library-level
`DEFAULT_MAX_TURNS` of 10) and the vendor page does not state it — **UNVERIFIED**; the Rust runtime should
set its own explicit limit rather than inherit an ambiguous default.

`RunConfig` fields worth mirroring: `model`, `model_settings`, `session_settings`, `input_guardrails`,
`output_guardrails`, `handoff_input_filter`, `nest_handoff_history`, `handoff_history_mapper`,
`call_model_input_filter` (edit the fully prepared model input immediately before the call),
`reasoning_item_id_policy`, `tool_execution`, `tool_not_found_behavior`, `tool_name_collision_policy`,
`tool_error_formatter`, plus tracing fields.

### 2.B.2 Tool definition and wire shape (Responses API)

Tool definition ([source](https://developers.openai.com/api/docs/guides/function-calling)):

```json
{ "type": "function", "name": "get_weather", "description": "…",
  "parameters": { "type": "object", "properties": {...},
                  "required": [...], "additionalProperties": false },
  "strict": true }
```

Model output item:
```json
{ "type": "function_call", "id": "…", "call_id": "call_abc123",
  "name": "get_weather", "arguments": "{\"location\":\"SF\"}" }   // arguments is a JSON *string*
```

Your reply, appended to the next request's `input` array:
```json
{ "type": "function_call_output", "call_id": "call_abc123", "output": "…" }
```

Note there are **two** identifiers on a function call — `id` (the output item's own id) and `call_id` (the
correlation key). Only `call_id` goes back. `arguments` is a **string** containing JSON, not an object —
the opposite of Anthropic, where `input` is an object.

`tool_choice`: `"auto"` | `"required"` | `"none"` | `{"type":"function","name":"…"}` |
`{"type":"allowed_tools","mode":"auto","tools":[…]}`. `parallel_tool_calls: false` forces exactly zero or
one tool call. Custom (grammar-constrained) tools also exist:
`{"type":"custom","name":"…","format":{"type":"grammar","syntax":"lark"|"regex","definition":"…"}}`,
answered with a `custom_tool_call` carrying `input` (raw text, not JSON).

### 2.B.3 Parallel tool calls

On by default; controlled by the top-level `parallel_tool_calls` boolean. Unlike Anthropic, results are
individual `function_call_output` items in the `input` array — there is no "one message with all results"
rule to violate.

### 2.B.4 Handoffs / multi-agent

This is where OpenAI's SDK is genuinely richer than Anthropic's Messages API: **handoffs are a first-class
primitive**. A handoff is surfaced to the model as a tool; when the model calls it, the runner switches the
current agent and continues the same loop
([source](https://openai.github.io/openai-agents-python/handoffs/)). `handoff_input_filter`,
`nest_handoff_history`, and `handoff_history_mapper` control what history the receiving agent sees. There is
also "agents-as-tools" — a sub-agent invoked as a tool without transferring control.

Important for the port: **none of this is server-side.** A handoff is a normal function tool plus a
client-side agent switch. The Rust runtime can reproduce it exactly.

### 2.B.5 Sessions / conversation state — four mechanisms

([source](https://developers.openai.com/api/docs/guides/conversation-state) ·
[running agents](https://openai.github.io/openai-agents-python/running_agents/))

| Mechanism | Where state lives | Notes |
|---|---|---|
| Manual `input` array | your app | Fully stateless. For reasoning models, **preserve every item in the response's `output` array**, including encrypted reasoning items and assistant `phase` values. |
| SDK Sessions | your storage (e.g. `SQLiteSession`) | Client-side convenience. |
| **Conversations API** | OpenAI | `POST /v1/conversations`; then pass `conversation: "<id>"` on `/v1/responses`. Stores messages, tool calls, tool outputs. **No 30-day TTL** — conversation objects persist indefinitely. |
| **`previous_response_id`** | OpenAI | Cheapest response-to-response chaining. Response objects are stored 30 days by default when `store: true`. |

**The critical fact, and it is easy to get wrong:** *"Even when using `previous_response_id`, all previous
input tokens for responses in the chain are billed as input tokens in the API."*
([source](https://developers.openai.com/api/docs/guides/conversation-state)). Server-side state saves you
*bandwidth and bookkeeping*, **not tokens**. Anyone adopting it as a token-blowup fix will be disappointed.
The actual token lever on OpenAI is caching + compaction.

Also: SDK session persistence **cannot** be combined with server-managed conversation settings in the same
run.

### 2.B.6 Structured output

`text.format` on the Responses API
([source](https://developers.openai.com/api/docs/guides/structured-outputs)):

```json
{ "text": { "format": { "type": "json_schema", "name": "my_schema",
                        "schema": { … }, "strict": true } } }
```

Strict-mode schema rules: every field in `properties` must appear in `required`;
`additionalProperties: false` is mandatory; optional fields are expressed as `"type": ["string","null"]`.
Supported: the scalar types, `enum`, nested objects/arrays, **recursive schemas via `$ref`** (note: the
opposite of Anthropic, which forbids recursion). Not supported: pattern/format string constraints, complex
conditional schemas, very large schemas.

Refusals surface as a dedicated item: `{"type":"refusal","refusal":"…"}` — programmatically detectable,
and it will not match your schema.

### 2.B.7 Streaming event taxonomy

SSE with **named, dotted event types** — structurally very different from Anthropic's block-index model.
Documented set ([source](https://developers.openai.com/api/docs/guides/streaming-responses)):

```
response.created            response.in_progress          response.failed        response.completed
response.output_item.added  response.output_item.done
response.content_part.added response.content_part.done
response.output_text.delta  response.output_text.annotation_added   response.text.done
response.refusal.delta      response.refusal.done
response.function_call_arguments.delta   response.function_call_arguments.done
response.file_search.in_progress | .searching | .completed
response.code_interpreter.in_progress | .call_code.delta | .call_code.done
    | .call_interpreting | .call_completed
error
```
Also documented elsewhere in the guides: `response.reasoning_summary_text.delta` (reasoning summaries).
The docs give the event *set* but **do not specify a strict arrival order** — some events fire once per
response lifecycle, others repeat during generation. Treat ordering as: `response.created` first,
`response.completed` / `.failed` / `error` last, everything else keyed by `output_index` /
`content_index`.

For a provider-agnostic Rust event enum this is the awkward one: OpenAI names the *item kind* in the event
type, whereas Anthropic names a generic `content_block_delta` and puts the kind in `delta.type`.

### 2.B.8 Guardrails / middleware

The Agents SDK has **input guardrails, output guardrails, and tool guardrails**, which raise
`InputGuardrailTripwireTriggered` / `OutputGuardrailTripwireTriggered` when tripped, plus resumable
approval flows ([source](https://openai.github.io/openai-agents-python/running_agents/)). `RunConfig`
additionally exposes `call_model_input_filter` — an interception point on the fully prepared model input
immediately before the call. All client-side; all reproducible in Rust.

### 2.B.9 Memory primitives

No first-class memory tool comparable to Anthropic's `memory_20250818`. The nearest equivalents are the
hosted **file_search** tool (vector-store retrieval) and SDK Sessions.

### 2.B.10 Retries, errors, rate limits

Rate-limit headers ([source](https://developers.openai.com/api/docs/guides/rate-limits)):

```
x-ratelimit-limit-requests    x-ratelimit-remaining-requests    x-ratelimit-reset-requests
x-ratelimit-limit-tokens      x-ratelimit-remaining-tokens      x-ratelimit-reset-tokens
retry-after            (seconds)
retry-after-ms         (429 only — the server's own recommendation; the most useful header for retries)
```
`*-reset-*` values are **duration strings** like `6s` or `1m30s`, **not** timestamps — the exact opposite
of Anthropic's RFC 3339 resets. Two distinct 429s share the status code:
`rate_limit_exceeded` (wait and retry succeeds) and `insufficient_quota` (out of credits; retrying never
helps) — branch on the error `code`, not the status
([source](https://help.openai.com/en/articles/5955604-how-can-i-solve-429-too-many-requests-errors)).

## 2.C Server-side / API-native features over plain HTTP

### 2.C.1 Prompt caching — **ADOPT**

([source](https://developers.openai.com/api/docs/guides/prompt-caching))

- **Automatic (implicit) by default** on supported models; OpenAI places breakpoints at model-dependent
  intervals. No parameter needed to get the discount.
- **Explicit** mode: `prompt_cache_options.mode = "explicit"` plus
  `prompt_cache_breakpoint: {"mode":"explicit"}` on content blocks.
- **Minimum prefix: 1,024 tokens** on GPT-5.6 and later; **2,048 visible input tokens** on earlier models
  (some may cache shorter). Hidden OpenAI system tokens don't count toward the minimum.
- **TTL:** GPT-5.6+ uses `prompt_cache_options.ttl`, whose only supported value is `"30m"` (default) —
  a prefix stays eligible for 30 minutes after its most recent write *or reuse*. Earlier models use
  `prompt_cache_retention`: `"in_memory"` (~5–10 min idle, up to 1 hour) or `"24h"` (~30 min typical, up to
  24 hours). **Default depends on Zero Data Retention**: orgs without ZDR default to `"24h"`; orgs with ZDR
  default to `"in_memory"`.
- `prompt_cache_key` influences routing so related requests land on the same cache. It **does not pin** a
  machine or guarantee a hit — but for a desktop app with per-user sessions, setting it per conversation is
  free and materially improves hit rate.
- **Usage fields:** `usage.input_tokens_details.cached_tokens` and
  `usage.input_tokens_details.cache_write_tokens`.
- **Pricing:** on GPT-5.6+, cache **writes are 1.25×** the uncached input rate and **reads are 0.1×** —
  the same shape as Anthropic. Earlier models use a model-dependent cached-input rate (the published
  per-model "cached input" column, generally 10% of input; see §2.D).
- **Invalidation:** the entire *rendered prefix* must match. Prefix-breaking settings include model
  selection, tool definitions and their ordering, `parallel_tool_calls`, output format, reasoning settings,
  verbosity, and context compaction.

> **Verdict: ADOPT.** Same 90% read discount as Anthropic, and it is on by default — the work is
> *prefix hygiene* (stable system prompt, deterministic tool order, volatile content last) plus setting
> `prompt_cache_key` per conversation and, on GPT-5.6+, being aware the window is a flat 30 minutes.

### 2.C.2 Server-side conversation state — **ADOPT for bookkeeping, not for cost**

Two mechanisms (`previous_response_id`, Conversations API) described in §2.B.5. Both are plain HTTP.

> **Verdict: ADOPT-LATER, and never as a cost measure.** Previous input tokens are billed regardless. It
> is worth adopting only if the runtime wants OpenAI to own reasoning-item continuity (see §2.C.7) rather
> than shuttling encrypted reasoning blobs itself. Given the runtime is provider-agnostic and Anthropic
> has no equivalent, the cheaper architectural choice is to keep history client-side for all providers.

### 2.C.3 Server-hosted tools — **ADOPT selectively**

The Responses API ships **web search, file search, code interpreter, computer use, image generation, and
remote MCP servers** as built-in tools
([source](https://developers.openai.com/api/docs/guides/tools) ·
[OpenAI announcement](https://openai.com/index/new-tools-and-features-in-the-responses-api/)). All are
declared in `tools` and executed server-side; the streaming taxonomy has dedicated events for file_search
and code_interpreter progress (§2.B.7).

> **Verdict: ADOPT web_search and file_search** where they replace local infrastructure — `file_search`
> in particular is a hosted vector store, which is a whole subsystem the Rust app would otherwise own.
> **SKIP computer use** for a desktop app that already has OS access.

### 2.C.4 Batch API — **ADOPT for offline work**

`POST /v1/batches` with `input_file_id`, `endpoint`, and `completion_window` — the only accepted value is
`"24h"`. **50% off**, separate and much higher rate-limit pool, up to 50,000 requests per file. If OpenAI
cannot finish inside the window the batch is marked `expired` and unprocessed requests are refunded
([source](https://developers.openai.com/api/docs/guides/batch)).

> **Verdict: ADOPT** for the same offline paths as Anthropic's. Same 50%, same asynchronous constraint.

### 2.C.5 Context compaction — **ADOPT**

Released 2026-02-11. Two forms
([source](https://developers.openai.com/api/docs/guides/compaction)):

- **Automatic**, inside a normal create call:
  ```json
  { "context_management": [ { "type": "compaction", "compact_threshold": 200000 } ] }
  ```
  When the rendered token count crosses the threshold the server compacts. No separate call needed.
- **Explicit**: `POST /v1/responses/compact` — send the full context window, receive a compacted one.
  The docs are emphatic: *"do not prune `/responses/compact` output. The returned window is the canonical
  next context window."*

The compaction item is **opaque and not human-interpretable**. Carrying it forward: with stateless input
arrays, append all output items (including the compaction item) to the next request's `input`; with
`previous_response_id`, pass only the new user message and the compaction context is carried automatically.

Model support and token/pricing accounting are **not stated** on the guide page (examples reference
`gpt-5.3-codex` and `gpt-5.6`) — **UNVERIFIED**; measure before relying on cost figures.

Note `context_management` is an **array** on OpenAI and an **object with an `edits` array** on Anthropic —
similar names, different shapes.

> **Verdict: ADOPT.** With `compact_threshold` it is one field on the request and it directly bounds
> context growth. This is the OpenAI counterpart to Anthropic's `compact_20260112`.

### 2.C.6 Context windows — **no action needed**

GPT-5.6 sol / terra / luna all: **1.05M context, 128K max output**, knowledge cutoff 2026-02-16
([source](https://developers.openai.com/api/docs/models)).

### 2.C.7 Reasoning controls and token accounting — **ADOPT, with a stateless caveat**

([source](https://developers.openai.com/api/docs/guides/reasoning))

- `reasoning.effort` ∈ `none | minimal | low | medium | high | xhigh | max`.
- `reasoning.summary` ∈ `concise | detailed | auto`.
- GPT-5.6 additionally has a reasoning **mode**: `standard` | `pro` (pro does more work, costs more).
- **Reasoning tokens are billed as output tokens**, reported at
  `usage.output_tokens_details.reasoning_tokens`.
- **Stateless continuity:** when `store` is `false` (or under ZDR), reasoning items carry an
  `encrypted_content` property by default; pass those items back verbatim on subsequent requests to keep
  reasoning continuity. The conversation-state guide is explicit: *"For stateless reasoning-model
  requests, preserve every item in the response's `output` array"*, including encrypted reasoning items and
  assistant `phase` values.

> **Verdict: ADOPT.** Effort is the same routing knob as Anthropic's. The design consequence is concrete:
> **the Rust conversation representation must be able to carry opaque, provider-specific blobs** —
> OpenAI encrypted reasoning items, Anthropic thinking-block `signature`s, Gemini thought signatures — and
> replay them byte-for-byte. Model it as `ProviderOpaque(Vec<u8>)` in the message item enum from day one;
> retrofitting it later means rewriting the history type.

### 2.C.8 Files API — **ADOPT-LATER**

Standard `/v1/files` upload, referenced by `file_id`; also the backing store for the Batch API's JSONL
input and for `file_search` vector stores
([source](https://developers.openai.com/api/docs/guides/batch)).

### 2.C.9 Token counting — **ADOPT**

OpenAI has a **server-side input-token counting endpoint** for the Responses API: it accepts the same input
format (messages, images, files, tools, conversations) and returns `{"object":"response.input_tokens",
"input_tokens": N}` ([guide](https://developers.openai.com/api/docs/guides/token-counting) ·
[reference](https://developers.openai.com/api/reference/python/resources/responses/subresources/input_tokens/methods/count)).
The docs state plainly why local `tiktoken` is insufficient: images and files aren't supported, tools and
schemas add tokens that are hard to count locally, and model-specific behaviour (reasoning, caching)
changes tokenization. The server count includes structural/formatting tokens (roles, boundaries).

> **Verdict: ADOPT the server endpoint; do not rely on `tiktoken-rs` for budget decisions.** Keep
> `tiktoken-rs` only as an offline, no-network heuristic for UI hints.

## 2.D Models and pricing (2026-08-31)

[Pricing](https://developers.openai.com/api/docs/pricing) · [Models](https://developers.openai.com/api/docs/models)

| Model | Context | Max out | Input $/MTok | Cached input $/MTok | Output $/MTok |
|---|---|---|---|---|---|
| `gpt-5.6-sol` (flagship, max reasoning) | 1.05M | 128K | 4.00 | 0.40 | 20.00 |
| `gpt-5.6-terra` (balanced) | 1.05M | 128K | 2.00 | 0.20 | 12.00 |
| **`gpt-5.6-luna`** (cost-optimised / high volume) | 1.05M | 128K | **0.20** | **0.02** | **1.20** |
| `gpt-5.5` | — | — | 5.00 | 0.50 | 30.00 |
| `gpt-5.4` | — | — | 2.50 | 0.25 | 15.00 |
| `gpt-5.4-mini` | — | — | 0.75 | 0.075 | 4.50 |
| `gpt-5.4-nano` | — | — | 0.20 | 0.02 | 1.25 |
| `gpt-5.2` | — | — | 1.75 | 0.175 | 14.00 |
| `gpt-5.1` / `gpt-5` | — | — | 1.25 | 0.125 | 10.00 |
| `gpt-5-mini` | — | — | 0.25 | 0.025 | 2.00 |
| **`gpt-5-nano`** | — | — | **0.05** | **0.005** | **0.40** |
| `o3` | — | — | 2.00 | 0.50 | 8.00 |
| `o3-mini` | — | — | 1.10 | 0.55 | 4.40 |
| `o1` | — | — | 15.00 | 7.50 | 60.00 |

Context/max-output for the pre-5.6 rows was not on the models page — **UNVERIFIED**; the page details only
the GPT-5.6 family and lists older models in navigation, suggesting they are superseded. All three GPT-5.6
models support function calling, web search, file search, and computer use, and share a Feb 16 2026
knowledge cutoff. Batch = **50% off** across models (e.g. `gpt-5.6-sol` batch is $2.00 / $10.00).

Routing note: `gpt-5.6-luna` at $0.20/$1.20 is the cheapest current-generation flagship-family tier across
all four providers examined, and its cached-input rate of $0.02/MTok is the lowest number in this entire
document.


---

# PART 3 — GOOGLE (GEMINI)

**Verified on 2026-08-31.** Vendor docs (`ai.google.dev`, `adk.dev`, `docs.cloud.google.com`) fetched live;
crate facts read from the crates.io API on 2026-08-31. Anything not confirmed against a primary source is
marked **UNVERIFIED**.

**The one thing to internalise before reading further:** Google now has **two incompatible generation wire
formats** live at the same time — the legacy `generateContent` API and the newer **Interactions API**, which
Google calls "the best way to build with Gemini models and agents" and which is **GA for all new projects**
as of June 2026 ([source](https://ai.google.dev/gemini-api/docs/interactions)). They differ in tool shape,
streaming taxonomy, state model, and structured-output config. Every Rust crate in the ecosystem targets the
*legacy* one. This is the single largest Gemini-side integration risk in this document.

## 3.A The official agent SDK

Google ships four distinct surfaces, and — as with Anthropic — conflating them is the first trap.

| Surface | Package / endpoint | Languages | What it gives you |
|---|---|---|---|
| **Google Gen AI SDK** (client SDK) | `google-genai` (PyPI), `@google/genai` (npm), Go, Java | Python, TS/JS, Go, Java | Plain Gemini API client. You write the tool loop. |
| **Agent Development Kit (ADK)** | `google-adk` (PyPI), `@google/adk` (npm), `google.golang.org/adk/v2`, `com.google.adk:google-adk`, `com.google.adk:google-adk-kotlin-core` | **Python, TypeScript, Go, Java, Kotlin** | Full agent harness: agents, sub-agents, tools, sessions, memory, callbacks, runners, eval. Harness only; you host. |
| **Gemini Enterprise Agent Platform / Vertex AI Agent Engine** | Google Cloud managed runtime | via ADK or REST | Google runs the deployment, sessions, and (optionally) a sandbox. |
| **Interactions API** (REST) | `POST .../interactions` | any HTTP client | Server-side conversation state, server-side tool orchestration, execution steps. |

Sources: [ADK home](https://adk.dev/) · [Interactions API](https://ai.google.dev/gemini-api/docs/interactions) ·
[ADK on Gemini Enterprise Agent Platform](https://docs.cloud.google.com/gemini-enterprise-agent-platform/build/adk)

**Rust: no.** ADK's own homepage lists exactly five languages — Python, TypeScript, Go, Java, Kotlin — and
**Rust is not among them** ([source](https://adk.dev/)). Go was the most recent addition
([Google Developers Blog](https://developers.googleblog.com/announcing-the-agent-development-kit-for-go-build-powerful-ai-agents-with-your-favorite-languages/)),
which tells you Google is willing to expand the language set but has not chosen Rust.

**The near-miss to be aware of: `ADK-Rust` exists and is *not* Google's.** The crates `adk-core`,
`adk-agent`, `adk-runner`, `adk-session`, `adk-model`, `adk-gemini`, `adk-telemetry`, `adk-plugin`,
`adk-artifact`, `adk-skill` (and ~33 more) are all **v2.1.0, published 2026-08-25**, from
`github.com/zavora-ai/adk-rust` (crates.io API, 2026-08-31). Its own README says it is "not a Google
product… a community project developed by Zavora AI"
([README](https://raw.githubusercontent.com/zavora-ai/adk-rust/main/README.md)). It is the closest thing to
a Rust agent SDK that exists for any of the four providers — see §3.E and Part 6 for the honest assessment.

**Verdict for the Rust team:** no first-party Rust path; the Rust runtime must speak the Gemini REST API
directly. That is fine — every feature in §3.C is plain HTTP with an `x-goog-api-key` header.

## 3.B Agent-loop semantics — what "SDK-shaped" means for Google

### 3.B.1 The core loop

ADK's runner concept is agent-centric rather than `stop_reason`-centric, but at the REST level the loop is
driven by **whether the response contains a client-executable function call**.

Legacy `generateContent` loop:

```
loop {
  resp = POST /v1beta/models/{model}:generateContent { contents, tools, toolConfig, systemInstruction,
                                                       generationConfig, safetySettings }
  cand = resp.candidates[0]
  append { role: "model", parts: cand.content.parts } to contents   // FULL parts, incl. thoughtSignature
  if any part has .functionCall  => execute all; append ONE { role: "user", parts: [functionResponse...] }
  else match cand.finishReason {
    "STOP"                     => done
    "MAX_TOKENS"               => output truncated — raise maxOutputTokens
    "SAFETY" | "RECITATION"    => blocked; read cand.safetyRatings / resp.promptFeedback
    "MALFORMED_FUNCTION_CALL"  => model emitted an unparseable call — retry or tighten schema
  }
}
```

`finishReason` values `STOP`, `MAX_TOKENS`, `SAFETY`, `RECITATION`, `MALFORMED_FUNCTION_CALL`
([source](https://ai.google.dev/gemini-api/docs/troubleshooting)). Note `MALFORMED_FUNCTION_CALL` — none of
the other three providers has a distinct terminal state for "the model produced a tool call I could not
parse". It must be a first-class arm in the Rust `StopReason` enum.

Interactions API loop: the response is an **`Interaction` object containing the complete turn — execution
steps, tool calls, and model output** ([source](https://ai.google.dev/gemini-api/docs/interactions)). Server
-side (hosted) tools are resolved *inside one API call*: "the entire process happens within one API call"
([source](https://ai.google.dev/gemini-api/docs/interactions/tools)). Only **client** function calls come
back to you as `function_call` steps to execute. So the client loop iterates far less often than
Anthropic's, but each iteration can hide many server-side steps.

**Iteration limits: none server-side** on either surface — **UNVERIFIED** as an explicit negative statement,
but no turn cap appears anywhere in the Gemini REST reference. Keep your own `max_iterations`.

### 3.B.2 Tool definition and the tool-call → tool-result wire shape

**This is where the two APIs diverge hardest.** A Rust client must pick one and commit, or abstract both.

*Legacy `generateContent`* ([source](https://ai.google.dev/gemini-api/docs/generate-content/function-calling)):

```json
// request
"tools": [{ "functionDeclarations": [{
  "name": "get_weather",
  "description": "Get the current weather for a location.",
  "parameters": { "type": "object",
                  "properties": { "location": { "type": "string", "description": "City" } },
                  "required": ["location"] } }] }],
"toolConfig": { "functionCallingConfig": { "mode": "AUTO" } }

// model output part
{ "functionCall": { "id": "call_123", "name": "get_weather", "args": { "location": "London" } } }

// client reply — role "user", NOT a dedicated tool role
{ "role": "user", "parts": [
  { "functionResponse": { "id": "call_123", "name": "get_weather",
                          "response": { "result": "17C, cloudy" } } } ] }
```

*Interactions API* ([source](https://ai.google.dev/gemini-api/docs/function-calling)):

```json
// tool declaration — flat, OpenAI-Responses-shaped
{ "type": "function", "name": "get_weather", "description": "...",
  "parameters": { "type": "object", "properties": { ... }, "required": [...] } }

// model output item
{ "type": "function_call", "id": "call_123", "name": "get_weather",
  "arguments": { "location": "London" } }

// client reply item — note call_id (not id), and a typed content array
{ "type": "function_result", "name": "get_weather", "call_id": "call_123",
  "result": [ { "type": "text", "text": "17C, cloudy" } ] }
```

Four traps in that diff, each of which will silently break a naive port:
1. `functionDeclarations` nested under `tools[]` (legacy) vs a flat `{"type":"function"}` item (Interactions).
2. `args` (object) vs `arguments` (object) — and **note that Gemini passes arguments as a JSON *object*, not
   as a JSON *string* the way OpenAI and xAI do.** Parsing code is not portable.
3. The result correlation field renames: `id` → `call_id`.
4. The result payload changes from a free-form `response` object to a **typed content array** that supports
   multimodal returns (`{"type":"image","mime_type":...,"data":...}`)
   ([source](https://ai.google.dev/gemini-api/docs/function-calling)). Gemini is the only one of the four
   that can take an *image* back from a tool.

Tool-choice modes: `AUTO`, `ANY`, `NONE`, `VALIDATED` (legacy `toolConfig.functionCallingConfig.mode`;
Interactions `generation_config.tool_choice` = `auto` / `any` / `none` / `validated`, plus
`{"allowed_tools":{"mode":"any","tools":[...]}}`). **`VALIDATED` has no analogue at Anthropic, OpenAI, or
xAI** — it makes the model adhere to the declared schema and is the documented default when tools are
combined ([source](https://ai.google.dev/gemini-api/docs/generate-content/function-calling)). Schema root
must be an object.

### 3.B.3 Parallel tool calls

Supported: multiple independent `function_call` steps come back in a single turn
([source](https://ai.google.dev/gemini-api/docs/function-calling)). Gemini also documents **compositional
(sequential) calling** — the model chains a second call off the first result — and, in Gemini 3, allows
**structured outputs, built-in tools, and custom functions in the same request**
([source](https://ai.google.dev/gemini-api/docs/gemini-3)), which Anthropic and OpenAI restrict more
tightly. Executor design implication: same as Anthropic — fan out, join, return all results in one message.

### 3.B.4 Handoffs / sub-agents

**Nothing at the REST layer.** Sub-agents, `AgentTool`, sequential/parallel/loop workflow agents and
Agent2Agent (A2A) orchestration are **ADK library constructs**, not API features
([ADK](https://adk.dev/) · [InfoWorld on A2A + Go](https://www.infoworld.com/article/4089550/google-agent-development-kit-adds-go-language-support.html)).
Consequence for the Rust runtime: handoff is *entirely yours to implement*, exactly as it is for Anthropic.
This is the strongest argument in the whole document for the "SDK-shaped native runtime" decision — three of
four providers put multi-agent structure in the client library, so a Rust orchestrator is not swimming
against the tide.

### 3.B.5 Sessions / conversation state

- Legacy `generateContent`: **stateless**. Resend `contents` every turn.
- Interactions API: **stateful by default**. `store=true` is the default; retention is **55 days (paid) / 1
  day (free)**; continue by passing the prior interaction `id` as `previous_interaction_id`, and the server
  reconstructs history. Interaction-scoped parameters (`tools`, `system_instruction`, `generation_config`)
  **must be re-sent on every call** even in stateful mode
  ([source](https://ai.google.dev/gemini-api/docs/interactions)).

That last clause is a trap: "stateful" does not mean "send less config", it means "send less history".

### 3.B.6 Structured output / JSON schema enforcement

Interactions API ([source](https://ai.google.dev/gemini-api/docs/interactions/structured-output)):

```json
"response_format": { "type": "text", "mime_type": "application/json", "schema": { /* JSON Schema */ } }
```

Supported schema subset: `string`/`number`/`integer`/`boolean`/`object`/`array`/`null`; `title`,
`description`; `properties`, `required`, `additionalProperties`; `enum`, `format` (date-time/date/time);
`minimum`/`maximum`; `items`, `prefixItems`, `minItems`/`maxItems`; `anyOf`. Output is guaranteed
**syntactically valid JSON**, and the docs still tell you to "always validate values in your application".
Very large or deeply nested schemas **may be rejected**. Whether this is true constrained decoding is **not
stated — UNVERIFIED**; treat it as weaker than OpenAI's `strict: true`.

### 3.B.7 Streaming event taxonomy

Interactions API SSE — named events, `event: <type>` + `data: <json>`
([source](https://ai.google.dev/gemini-api/docs/interactions/streaming)):

```
interaction.created
  step.start          (repeats per step, carries step type)
  step.delta   × N
  step.stop
interaction.completed   (carries usage)
done
```

`step.delta.delta.type` is one of: `text`, `image` (base64 + `mime_type`), **`arguments_delta`** (partial
JSON *string* fragments you must accumulate and only then parse), `thought_summary`, `thought_signature`.
The docs explicitly instruct clients to **log and skip unknown event types rather than error**
([source](https://ai.google.dev/gemini-api/docs/interactions/streaming)) — build the Rust enum with a
`#[serde(other)] Unknown` arm from day one.

Legacy `streamGenerateContent` has no named-event taxonomy: it emits successive partial
`GenerateContentResponse` JSON objects. Two entirely different stream parsers.

### 3.B.8 Guardrails / interception / middleware

- **API-level:** `safetySettings` — four adjustable categories (harassment, hate speech, sexually explicit,
  dangerous) with thresholds `OFF` / `BLOCK_NONE` / `BLOCK_ONLY_HIGH` / `BLOCK_MEDIUM_AND_ABOVE` /
  `BLOCK_LOW_AND_ABOVE`; blocking surfaces as `promptFeedback.blockReason` (input blocked) or
  `finishReason: SAFETY` with `safetyRatings` (output blocked); child-safety protections are always on and
  non-adjustable ([source](https://ai.google.dev/gemini-api/docs/safety-settings)).
- **Library-level:** ADK callbacks/plugins (before/after agent, model, tool). Not an API feature — the Rust
  runtime supplies its own equivalent.

Practical note: Gemini is the only one of the four with a **request-side** block signal (`promptFeedback`)
distinct from a response-side one. The Rust error type needs both.

### 3.B.9 Memory / state primitives

- **File Search** — Google-hosted RAG: index documents, model retrieves at generation time
  ([source](https://ai.google.dev/gemini-api/docs/interactions/tools)).
- **Interactions store** — 55-day server-side transcript retention (§3.B.5).
- **Files API** — 20 GB/project, 2 GB/file, **48-hour** TTL, **free**
  ([source](https://ai.google.dev/gemini-api/docs/files)).
- ADK `SessionService` / `MemoryService` are library constructs.

There is **no `memory` tool** analogous to Anthropic's, and no cross-session memory primitive in the REST
API. Long-term memory remains the app's job.

### 3.B.10 Retries, error taxonomy, rate limits

([source](https://ai.google.dev/gemini-api/docs/troubleshooting) ·
[rate limits](https://ai.google.dev/gemini-api/docs/rate-limits))

| Status | Meaning | Retry? |
|---|---|---|
| 400 `INVALID_ARGUMENT` | bad syntax/params | **No** |
| 403 | bad key / unauthorised | **No** |
| 408 | request timeout | Yes, backoff |
| 429 `RESOURCE_EXHAUSTED` | rate limit | Yes, backoff |
| 500 / 503 `UNAVAILABLE` / 504 | server-side | Yes, backoff |

Documented policy: exponential backoff starting ~1 s (2, 4, 8…) **with jitter**, capped attempts; Google's
own Python SDK retries up to **4 times**, initial ~1 s, max 60 s. Rate limits are enforced on **RPM, TPM,
RPD** (plus IPM/TPD for some model classes), tiered by spend (Free → Tier 1 at billing enabled → Tier 2 at
$100 + 3 days → Tier 3 at $1,000 + 30 days).

**Gemini does not document rate-limit response headers** — unlike Anthropic and OpenAI, you cannot read
remaining quota off a successful response. **UNVERIFIED as an absolute negative**, but nothing in the rate-
limit or troubleshooting pages describes such headers. Implication: the Rust client's Gemini adapter must
maintain a **local** token-bucket estimate rather than reconciling against server headers. Design the
rate-limit abstraction so header data is `Option<...>`.

## 3.C Server-side / API-native features usable over plain HTTP

Everything below is reachable with `POST` + `x-goog-api-key`. No Google SDK required.

### 3.C.1 Context caching — **ADOPT (implicit), ADOPT-LATER (explicit)**

Two mechanisms, and which one you get **depends on which API you call**.

**Implicit caching** — automatic, on by default for **all Gemini 2.5 and newer models**, no setup, no
storage cost, discount applied automatically on hit
([source](https://ai.google.dev/gemini-api/docs/caching)).

**Explicit caching** — you create a `cachedContents` object and reference it. It gives a guaranteed discount
and control, and costs **storage per hour**. Critically: *"Explicit caching (manually creating and managing
cache objects) is **not supported in the Interactions API**"* — it requires the legacy `generateContent` API
([source](https://ai.google.dev/gemini-api/docs/interactions/caching.md.txt)). Default TTL 60 minutes,
updatable ([source](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/context-cache/context-cache-overview)).

Minimum cacheable prefix — **below this, nothing caches and you get no error**
([source](https://ai.google.dev/gemini-api/docs/caching)):

| Model | Minimum tokens |
|---|---|
| Gemini 3.7 / 3.6 / 3.5 Flash, 3.1 Pro Preview | **4,096** |
| Gemini 2.5 Flash / 2.5 Pro | **2,048** |

Price effect: cache reads are billed at roughly **10% of the input rate** on 3.x/2.5 models (e.g. 3.7 Flash
$0.75 → **$0.075**; 3.1 Pro ≤200k $2.00 → **$0.20**), i.e. a **90% discount**; on legacy 2.0 models the
explicit-cache discount is 75%
([pricing](https://ai.google.dev/gemini-api/docs/pricing) ·
[Vertex context-cache overview](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/context-cache/context-cache-overview)).
Explicit caching adds **storage rent**: $0.50/hr (3.7/3.6 Flash), $1.00/hr (3.5 Flash, 3.5 Flash-Lite,
3-Flash-Preview, 2.5 Flash), **$4.50/hr** (3.1 Pro Preview, 2.5 Pro)
([pricing](https://ai.google.dev/gemini-api/docs/pricing)). Implicit caching has **no** storage cost.

Invalidation / hit-maximisation rules (all documented, all identical in spirit to the other three):
prefix-match from the start of the prompt; put large static content **first**; send similar-prefix requests
**close together in time** ([source](https://ai.google.dev/gemini-api/docs/caching)).

Verification field — the only honest check:
- Interactions API: `usage.total_cached_tokens`
- legacy: `usageMetadata.cachedContentTokenCount` — **UNVERIFIED** field name against the current reference;
  the Interactions field name *is* verified.

> **Verdict: ADOPT implicit caching immediately — it costs nothing to use and is on by default, so the only
> work is *not defeating it*** (stable prefix ordering, no timestamps in the system instruction,
> deterministic tool-JSON key order). **ADOPT-LATER explicit caching**, and only for a genuinely hot, large,
> long-lived prefix: at $4.50/hr, a 3.1-Pro explicit cache costs ~$108/day whether you hit it or not, so it
> only wins at high request density. Note the coupling: choosing explicit caching **forces you onto the
> legacy `generateContent` API**, which costs you the Interactions API's server-side state and step
> observability. That is a real architectural fork, not a flag.

### 3.C.2 Server-side conversation state — **ADOPT (Interactions API)**

`store=true` (default) + `previous_interaction_id`; 55-day retention on paid tier; `store=false` opts out
([source](https://ai.google.dev/gemini-api/docs/interactions)). Google explicitly claims a **cost** benefit,
not just convenience: *"optional server-side state management enables more efficient context caching across
turns, reducing token costs."*

> **Verdict: ADOPT** — this is the opposite call from Anthropic (§1.C.2), because unlike Anthropic's
> Managed Agents this is not a separate hosted-runtime product; it is a field on the normal generation call.
> Two caveats to design around: (a) you still re-send `tools`/`system_instruction`/`generation_config` every
> turn, so it is not a total-token silver bullet; (b) a desktop app that keeps its own local transcript now
> has **two** sources of truth — treat the server copy as a cache, keep the local transcript authoritative,
> and be able to fall back to full-history mode when `store=false` is required for privacy.

### 3.C.3 Server-hosted tools — **ADOPT selectively**

Declared in `tools`; executed on Google's servers; *"the entire process happens within one API call"*
([source](https://ai.google.dev/gemini-api/docs/interactions/tools)).

| Tool | What it does | Cost beyond tokens |
|---|---|---|
| Google Search grounding | grounds answers in live web | **$14 / 1,000 requests** (after free tier) |
| Google Maps grounding | location-aware answers | **$14 / 1,000 queries** |
| Code execution | writes and runs Python server-side | tokens only (**UNVERIFIED** — no separate line on the pricing page) |
| URL context | fetches and reads specific URLs | tokens only (**UNVERIFIED**) |
| File search | hosted RAG over your documents | **UNVERIFIED** |
| Computer use (Preview) | drives a browser UI | **UNVERIFIED** |

([tools](https://ai.google.dev/gemini-api/docs/interactions/tools) ·
[pricing](https://ai.google.dev/gemini-api/docs/pricing))

> **Verdict: ADOPT code execution and URL context** (server-side, no local sandbox to build, no per-call
> fee found). **ADOPT-LATER Google Search grounding** — at $14/1k it is the most expensive search of the
> four providers examined (Anthropic $10/1k, xAI $5/1k), so route search to the cheapest provider rather
> than the current model's vendor. **SKIP computer use** (Preview) and **SKIP file search** for now: the
> desktop app already owns the filesystem, and hosted RAG means shipping user code to Google.

### 3.C.4 Batch API — **ADOPT for offline work only**

`POST .../v1beta/models/{model}:batchGenerateContent`; **50% of standard cost**; target turnaround 24 h,
jobs expire after 48 h pending/running; inline requests capped at **20 MB**, file inputs up to **2 GB**
([source](https://ai.google.dev/gemini-api/docs/batch-api)). **Batch supports only `generateContent` — not
the Interactions API.**

> **Verdict: ADOPT for offline work only** (bulk indexing, evals, background summarisation). Useless for
> interactive agent turns, and the generateContent-only restriction means batch jobs must be written against
> the *legacy* wire format even if the interactive path uses Interactions. Budget for maintaining both
> encoders if batch is adopted.

### 3.C.5 Context management / automatic compaction — **SKIP (no such feature)**

Google documents no server-side compaction or context-editing primitive equivalent to Anthropic's context
editing or OpenAI's compaction. The nearest things are implicit caching, `previous_interaction_id`, and the
advice to *reduce media resolution* when requests exceed the context limit
([source](https://ai.google.dev/gemini-api/docs/gemini-3)). **UNVERIFIED as an absolute negative**; nothing
was found across the Interactions, caching, or Gemini-3 guides.

> **Verdict: SKIP — nothing to adopt.** The Rust runtime's own compaction strategy must therefore be
> **provider-independent and client-side**, because it is the only strategy that works on all four
> providers. Do not design compaction as a thin wrapper over Anthropic's server feature.

### 3.C.6 Context windows and long context — **no action needed, but watch the price cliff**

Gemini 3 models: **1,048,576 input tokens / 65,536 output tokens** (verified on the 3.7 Flash and 3.5
Flash-Lite model cards) ([3.7 Flash](https://ai.google.dev/gemini-api/docs/models/gemini-3.7-flash) ·
[3.5 Flash-Lite](https://ai.google.dev/gemini-api/docs/models/gemini-3.5-flash-lite) ·
[Gemini 3 guide](https://ai.google.dev/gemini-api/docs/gemini-3)).

**The important operational fact is not the size, it is the tier boundary:** on the Pro models, prompts
**above 200k tokens** are billed at roughly **2× input and 1.5× output**
($2.00/$12.00 → $4.00/$18.00 on 3.1 Pro Preview) ([pricing](https://ai.google.dev/gemini-api/docs/pricing)).
Crossing 200k tokens on a Pro model silently doubles the input bill. The Rust runtime should treat 200k as a
**hard budget alarm**, not a soft one. Secondary reports of a 2M-token window on 3.1 Pro are
**UNVERIFIED** — the vendor guide says 1M.

### 3.C.7 Reasoning / thinking controls and token accounting — **ADOPT**

`thinking_level`: `minimal` / `low` / `medium` / `high`, with **per-model defaults and per-model supported
sets** — 3.7 Flash defaults to `medium` and does **not** support `minimal`; the Gemini 3 guide describes
`high` as the family default ([thinking](https://ai.google.dev/gemini-api/docs/thinking) ·
[Gemini 3](https://ai.google.dev/gemini-api/docs/gemini-3) ·
[3.7 Flash card](https://ai.google.dev/gemini-api/docs/models/gemini-3.7-flash)). Treat the supported set as
**per-model data in a table**, not as a global enum.

Thought summaries: off by default, enabled via `thinking_summaries` (`"none"` disables); may come back empty.

**Thought signatures are the trap.** They are encrypted representations of internal reasoning state. In
stateful mode (`previous_interaction_id`) the server manages them. **In stateless mode you must resend all
thought blocks exactly as received** ([source](https://ai.google.dev/gemini-api/docs/thinking)). This is the
direct analogue of Anthropic's thinking-block signature rule and of xAI's `reasoning_content` rule (§4) —
all three providers punish you for dropping opaque reasoning state, and xAI names it the **top cause of
cache misses**.

Billing: **thinking tokens are billed as output tokens**; reported as `total_thought_tokens` in usage
([source](https://ai.google.dev/gemini-api/docs/thinking)).

> **Verdict: ADOPT.** `thinking_level` is the cheapest cost lever after caching — dropping a routine turn
> from `high` to `low` cuts billed output directly. Track `total_thought_tokens` separately in telemetry;
> an unexplained cost spike is usually thinking, not answers.

### 3.C.8 Files API — **ADOPT-LATER**

Resumable upload via `POST /upload/v1beta/files` with `X-Goog-Upload-*` headers; `GET/LIST/DELETE
/v1beta/files`; **20 GB per project, 2 GB per file, 48-hour TTL, free in all regions**
([source](https://ai.google.dev/gemini-api/docs/files)). Referenced by `uri` + `mime_type`.

> **Verdict: ADOPT-LATER.** Free and generous, and the 48 h TTL suits a desktop session. But it is a
> multimodal-attachment mechanism, not a token-reduction mechanism — it does not attack the cost problem.
> Do it when the app needs PDF/audio/video input, not before.

### 3.C.9 Token counting — **ADOPT**

`POST https://generativelanguage.googleapis.com/v1beta/models/{model}:countTokens` returns input token count
and accounts for text, images (258 tokens ≤384 px; larger images tile into 768×768 blocks at 258 each),
video (263 tok/s), audio (32 tok/s), **tools/functions**, and system instructions
([source](https://ai.google.dev/gemini-api/docs/tokens)). Whether `countTokens` is free is **not stated in
the docs — UNVERIFIED** (it is widely believed to be free; do not build a cost model on that belief).

Post-generation usage fields (Interactions API): `total_input_tokens`, `total_output_tokens`,
`total_thought_tokens`, `total_cached_tokens`, `total_tool_use_tokens`, `total_tokens`
([source](https://ai.google.dev/gemini-api/docs/tokens)). Note `total_tool_use_tokens` — Gemini is the only
provider of the four that breaks out tool-use tokens as a separate accounting line. Use it: it tells you
directly whether tool schemas are the source of the blowup.

> **Verdict: ADOPT.** Same reasoning as Anthropic and OpenAI: local tokenizers cannot count images, tools,
> or cache interactions. Use the endpoint for pre-flight budget checks and routing.

## 3.D Models and pricing (2026-08-31)

[Pricing](https://ai.google.dev/gemini-api/docs/pricing) · [Models](https://ai.google.dev/gemini-api/docs/models)

All prices per 1M tokens, USD, paid tier.

| Model | Model code | Context | Max out | Input | Cache read | Cache storage | Output |
|---|---|---|---|---|---|---|---|
| **Gemini 3.7 Flash** (current default) | `gemini-3.7-flash` | 1,048,576 | 65,536 | **0.75** | **0.075** | 0.50/hr | **3.75** |
| Gemini 3.6 Flash | `gemini-3.6-flash` | 1M (UNVERIFIED) | 64K (UNVERIFIED) | 0.75 | 0.075 | 0.50/hr | 3.75 |
| Gemini 3.5 Flash | `gemini-3.5-flash` | 1M (UNVERIFIED) | 64K (UNVERIFIED) | 1.50 | 0.15 | 1.00/hr | 9.00 |
| **Gemini 3.5 Flash-Lite** (cheap/fast) | `gemini-3.5-flash-lite` | 1,048,576 | 65,536 | **0.30** | **0.03** | 1.00/hr | **2.50** |
| Gemini 3.1 Flash-Lite | `gemini-3.1-flash-lite` | 1M (UNVERIFIED) | — | 0.25 (audio 0.50) | — | — | 1.50 |
| Gemini 3.1 Pro Preview ≤200k | `gemini-3.1-pro-preview` | 1M | 64K | 2.00 | 0.20 | 4.50/hr | 12.00 |
| Gemini 3.1 Pro Preview **>200k** | same | — | — | **4.00** | 0.40 | 4.50/hr | **18.00** |
| Gemini 3 Flash Preview | `gemini-3-flash-preview` | — | — | 0.50 (audio 1.00) | 0.05 | 1.00/hr | 3.00 |
| Gemini 2.5 Pro ≤200k | `gemini-2.5-pro` | 1M (UNVERIFIED) | — | 1.25 | 0.125 | 4.50/hr | 10.00 |
| Gemini 2.5 Pro >200k | same | — | — | 2.50 | 0.25 | 4.50/hr | 15.00 |
| Gemini 2.5 Flash | `gemini-2.5-flash` | 1M (UNVERIFIED) | — | 0.30 (audio 1.00) | 0.03 | 1.00/hr | 2.50 |
| **Gemini 2.5 Flash-Lite** (cheapest) | `gemini-2.5-flash-lite` | 1M (UNVERIFIED) | — | **0.10** (audio 0.30) | **0.01** | — | **0.40** |

Modifiers ([pricing](https://ai.google.dev/gemini-api/docs/pricing)):
- **Batch = 50% off** all token categories (cache storage is **not** discounted).
- **Priority tier = 1.8× standard.**
- **Free tier exists** for most models — but free-tier usage is used for product improvement; paid tier opts
  out. For a desktop app handling user code, the free tier is **not usable**.
- **Prices rise on 2027-01-01** — the 3.x introductory rates roughly **double** (3.7 Flash goes to $1.50 in
  / $7.50 out per secondary reporting — the doubling is on the vendor pricing page; the exact post-increase
  output figure is **UNVERIFIED**). Any cost model built on today's Gemini prices has a **four-month shelf
  life**.
- Grounding: Google Search **$14/1,000 requests**, Google Maps **$14/1,000 queries**, after free tier.

Routing note: **Gemini 3.5 Flash-Lite at $0.30/$2.50 with a 1M window and a $0.03 cache read** is the best
context-window-per-dollar in this document. Gemini 2.5 Flash-Lite at $0.10/$0.40 is cheaper still and
remains the floor price across all four providers for a 1M-class window.

## 3.E Rust ecosystem for Google

No official SDK (§3.A). Community, from the crates.io API on 2026-08-31:

| Crate | Version | Last release | Downloads | Assessment |
|---|---|---|---|---|
| `adk-core` + ~42 sibling crates (`adk-agent`, `adk-runner`, `adk-session`, `adk-model`, `adk-gemini`, …) | **2.1.0** | **2026-08-25** | 47k (core), 500k+ across the family | **The only Rust *agent framework* in this document.** Community (`zavora-ai`), not Google. Multi-provider (Gemini default; OpenAI, Anthropic, xAI via OpenAI-compat), `#[tool]` macro, MCP, SSE streaming, session backends (SQL/Redis/Mongo/Firestore/Neo4j/Vertex). README claims caching "via Anthropic client integration" — i.e. **no evidence of Gemini implicit/explicit cache support**. |
| `gemini-rust` | 2.0.0 | 2026-07-10 | 47k | Focused Gemini client, actively maintained. Interactions API support **UNVERIFIED**. |
| `google-ai-rs` | 0.3.0 | 2025-10-07 | 7.0k | ~11 months stale; predates Gemini 3 entirely. |
| `google-generative-ai-rs` | 0.3.4 | 2024-12-23 | 37k | Abandoned (20 months). |
| `googleapis-tonic-google-ai-generativelanguage-v1beta` | 0.42.0 | 2026-06-20 | 59k | Generated gRPC/tonic bindings. Current-ish, but it is protobuf plumbing, not an SDK — and the Interactions API is a REST/SSE surface. |

**Assessment:** every one of these targets the **legacy `generateContent`** surface. Nothing in the Rust
ecosystem was found to implement the Interactions API, its named SSE event taxonomy, `previous_interaction_id`,
or explicit `cachedContents` management. Treated as a verified gap for `google-ai-rs` and
`google-generative-ai-rs` (both predate the API); **UNVERIFIED but strongly implied** for `gemini-rust` and
`adk-rust`, whose release dates are recent enough that support is possible but undocumented.

---

# PART 4 — xAI (GROK)

**Verified on 2026-08-31.** All facts from `docs.x.ai` fetched live; crate facts from the crates.io API on
2026-08-31. Unconfirmed items are marked **UNVERIFIED**.

**Headline finding, stated plainly because the plan depends on it:**
**xAI ships no agent SDK in any language.** There is an official *client* SDK (Python, gRPC-based), a
coding-agent *product* (Grok Build — a CLI/TUI, not a library), and a set of *server-side agentic features*
(hosted tools, a multi-agent model, stateful responses, a compaction API). What does not exist is an
`openai-agents` / `claude-agent-sdk` / ADK equivalent: no Runner, no handoffs, no guardrail hooks, no
session abstraction shipped as a library. For a Rust team this is *good* news — there is no SDK semantics to
conform to, only a wire protocol, and that wire protocol is **OpenAI's**.

## 4.A The official SDK — and how OpenAI-compatible xAI actually is

| Surface | What it is | Languages | Rust? |
|---|---|---|---|
| **`xai-sdk`** (official client SDK) | **gRPC-based** Python library, sync + async clients | **Python ≥ 3.10 only** | No |
| **OpenAI-compatible REST** | `https://api.x.ai/v1` with `Authorization: Bearer` | any HTTP client / any OpenAI SDK | **Yes — this is the Rust path** |
| **Grok Build** | Hosted coding agent, exposed as CLI + TUI + **Agent Client Protocol (ACP)** | Bash/CLI, Python, JS | No library API |
| **Agents SDK** | **Does not exist** | — | — |

Sources: [xai-sdk on PyPI](https://pypi.org/project/xai-sdk/) ·
[xai-sdk-python](https://github.com/xai-org/xai-sdk-python) ·
[REST API reference](https://docs.x.ai/developers/api-reference) ·
[Grok Build overview](https://docs.x.ai/build/overview)

`xai-sdk` is at **1.17.0** (PyPI listing, mid-2026) — **UNVERIFIED against the PyPI JSON API directly**; the
*language* claim (Python only) is verified from the repository and PyPI page. Note that the official SDK
being **gRPC-based** is itself a finding: it means the Python SDK is *not* the reference implementation of
the REST surface, and REST is a first-class, separately documented interface rather than a generated
by-product.

### How compatible is "OpenAI-compatible", precisely?

The API reference states the Inference REST API "offers advanced AI capabilities with **full compatibility
with the OpenAI REST API**" ([source](https://docs.x.ai/developers/api-reference)). Fetched page-by-page,
the compatibility decomposes as follows:

| Area | Compatible with OpenAI? | Evidence |
|---|---|---|
| Base URL swap | **Yes** — set `base_url` to `https://api.x.ai/v1`, `Authorization: Bearer <key>` | [overview](https://docs.x.ai/docs/overview) |
| `POST /v1/chat/completions` | **Yes** | [api-reference](https://docs.x.ai/developers/api-reference) |
| `POST /v1/responses` | **Yes**, incl. `input`, `store`, `previous_response_id`, `include:["reasoning.encrypted_content"]` | [chat guide](https://docs.x.ai/docs/guides/chat) |
| Function/tool calling | **Yes** — same `{"type":"function", ...}` shape, `tool_choice` = `auto`/`required`/`none`/named, `parallel_tool_calls` | [function calling](https://docs.x.ai/docs/guides/function-calling) |
| Streaming (chat completions) | **Yes** — `chat.completion.chunk`, `choices[0].delta.content`, terminating `data: [DONE]` | [streaming](https://docs.x.ai/docs/guides/streaming-response) |
| Structured outputs | **Yes** — `response_format: {type:"json_schema", json_schema:{name, schema, strict:true}}` | [structured outputs](https://docs.x.ai/docs/guides/structured-outputs) |
| Realtime / speech-to-speech | **Yes** — `wss://api.x.ai/v1/realtime`, OpenAI Realtime-compatible | [speech-to-speech](https://docs.x.ai/developers/model-capabilities/audio/speech-to-speech) |
| Batch API | **Divergent** — xAI's own create-batch/add-request/poll model, not OpenAI's JSONL-file batch | [batch](https://docs.x.ai/developers/advanced-api-usage/batch-api) |
| Cache steering | **Divergent** — `x-grok-conv-id` **HTTP header** (no OpenAI analogue) alongside `prompt_cache_key` | [caching](https://docs.x.ai/developers/advanced-api-usage/prompt-caching/maximizing-cache-hits) |
| Context compaction | **xAI-only** — `POST /v1/responses/compact` | [compaction](https://docs.x.ai/developers/advanced-api-usage/context-compaction) |
| Server-side tools | **Divergent names** — `web_search`, `x_search`, `code_interpreter`, remote MCP, collections search | [tools](https://docs.x.ai/docs/guides/tools/overview) |
| Deferred completions | **xAI-only** — HTTP `202` + `GET /v1/chat/deferred-completion/{request_id}` | [debugging](https://docs.x.ai/developers/debugging) |
| Multi-agent | **xAI-only** — `grok-4.20-multi-agent-0309`, `agent_count`, `use_encrypted_content` | [multi-agent](https://docs.x.ai/developers/model-capabilities/text/multi-agent) |
| Tokenizer endpoint | **xAI-only** — `/v1/tokenize-text` | [api-reference](https://docs.x.ai/developers/api-reference) |
| Semantic document search | **xAI-only** — `/v1/documents/search` over Collections | [collections](https://docs.x.ai/developers/files/collections) |

**Practical conclusion:** the *core generation path* — chat completions, responses, tools, streaming,
structured outputs — is genuinely a base-URL swap. Everything that makes xAI interesting for cost
(compaction, cache steering, deferred, batch) is **outside** the OpenAI-compatible envelope and needs
hand-written request types. Design the Rust xAI adapter as "OpenAI transport + an xAI extension trait",
**not** as a separate protocol implementation and **not** as a pure alias of the OpenAI adapter.

**Verdict for the Rust team:** xAI is the *easiest* of the four to reach from Rust, because `async-openai`
(7.69M downloads, v0.41.3, 2026-07-31) with an overridden base URL covers the core path today. Nothing else
in this document has that property.

## 4.B Agent-loop semantics

### 4.B.1 The core loop

Identical in shape to OpenAI's, because it is OpenAI's wire format:

```
loop {
  resp = POST https://api.x.ai/v1/responses  { model, input, tools, store, previous_response_id,
                                               reasoning_effort, include:["reasoning.encrypted_content"] }
  for item in resp.output {
    match item.type {
      "message"        => emit text
      "reasoning"      => KEEP IT (encrypted_content must be resent — see 4.B.10 / 4.C.7)
      "function_call"  => execute; append a function_call_output item
      // web_search / x_search / code_interpreter items are already RESOLVED server-side
    }
  }
  if no function_call items => done
}
```

The distinguishing property versus OpenAI: **xAI runs the tool loop server-side for its own hosted tools.**
*"Once you enable those tools in the request, the server will perform the agent loop to invoke those tools on
the server side based on your query until the final answer is generated"*
([source](https://docs.x.ai/developers/model-capabilities/text/multi-agent)); built-in tools "run on xAI's
servers—you provide the tool configuration, and the API handles execution and returns results"
([source](https://docs.x.ai/docs/guides/tools/overview)). Only client-side function tools come back for you
to execute, and they can be mixed with server-side tools in the same request.

Stop conditions: absence of `function_call` items. Explicit `finish_reason` enumeration (`stop`, `length`,
`tool_calls`, `content_filter`) is **not documented on the debugging page — UNVERIFIED**; by OpenAI
compatibility it should be the OpenAI set, but do not build the truncation detector on that assumption
without testing (see §5.5, trap 6).

Iteration limits: none documented — **UNVERIFIED as an absolute negative**. Keep your own counter.

### 4.B.2 Tool definition and the tool-call → tool-result wire shape

([source](https://docs.x.ai/docs/guides/function-calling))

```json
// tool definition
{ "type": "function", "name": "get_temperature",
  "description": "Get current temperature for a location",
  "parameters": { "type": "object",
                  "properties": { "location": {"type":"string"},
                                  "unit": {"type":"string","enum":["celsius","fahrenheit"]} },
                  "required": ["location"] } }
```

Constraint worth writing down: *"The root of a `parameters` schema must be an object"* — or a union of
objects via `oneOf`/`anyOf`. Same restriction Gemini imposes.

The model returns a `tool_call` carrying the function name and **arguments as a JSON string you must
parse**; you reply with a tool-result message carrying the original call identifier
([source](https://docs.x.ai/docs/guides/function-calling)). That is the OpenAI contract exactly, and it is
the **opposite** of Gemini, which hands you an already-parsed object (§3.B.2). A provider-agnostic Rust
`ToolCall` type must therefore hold arguments as `serde_json::Value` and let each adapter decide whether to
`from_str` on the way in.

### 4.B.3 Parallel tool calls

**On by default** — *"the model can request multiple tool calls in a single response"* — disabled with
`parallel_tool_calls: false` ([source](https://docs.x.ai/docs/guides/function-calling)). Same executor
design as the other three: fan out, join, return all results together.

### 4.B.4 Handoffs / sub-agents

xAI is the only one of the four with a **server-side multi-agent primitive**, and it is a model, not an API:
`grok-4.20-multi-agent-0309` (beta). Multiple agents are launched to collaborate; a leader agent synthesises
the answer. Agent count is set by `agent_count` (4 or 16) in the xAI SDK, or — via the OpenAI-compatible
path — by `reasoning.effort` (`low`/`medium` → 4 agents; `high`/`xhigh` → 16). By default only the leader's
tool calls and final response are returned; sub-agent reasoning, tool calls and outputs come back **only**
when `use_encrypted_content: true`, encrypted and opaque
([source](https://docs.x.ai/developers/model-capabilities/text/multi-agent)).

**All tokens from leader and sub-agents are billed**, including server-side tool calls — the docs warn of
"potentially significantly higher usage than single-agent requests."

> For an app with a token-blowup problem this is a **cost hazard disguised as a feature**. It is also
> semantically *not* a handoff (no agent identity, no instruction swap, no control return) — it is
> ensemble reasoning. It cannot implement the app's multi-agent structure. Treat it as a premium
> single-call quality knob to be used deliberately, never as the default model.

### 4.B.5 Sessions / conversation state

Two modes ([source](https://docs.x.ai/docs/guides/chat)):
- `/v1/chat/completions` — stateless; resend everything.
- `/v1/responses` with `store: true` — xAI stores prior inputs, reasoning content, and outputs; continue via
  `previous_response_id`. **Responses remain retrievable for 30 days**; `store: false` opts out.

Retention comparison worth noting: xAI 30 days, Gemini 55 days (paid). Both are longer than a desktop
session, and both are a privacy consideration for an app that handles user source code — `store: false`
must be a first-class, user-visible setting.

### 4.B.6 Structured output / JSON schema enforcement

([source](https://docs.x.ai/docs/guides/structured-outputs))

```json
"response_format": { "type": "json_schema",
  "json_schema": { "name": "my_schema", "schema": { /* JSON Schema */ }, "strict": true } }
```

Guarantee: *"When using supported schema features, the response is guaranteed to match your schema"* —
structural enforcement, i.e. genuine constrained decoding, stronger than Gemini's stated guarantee. But the
enforcement has **numeric ceilings**, and this is the sharp edge:

- `maxLength` enforced **up to 2,048 characters**
- `maxItems` enforced **up to 256 items**
- `maxProperties` enforced **up to 64 fields**
- `minimum`/`maximum` — no limit
- **Best-effort only** (not guaranteed): `not`, `if`/`then`/`else`, `allOf` with multiple subschemas,
  non-standard `format` values
- **400-rejected**: empty enums, boolean property schemas, array-based item tuples

Supported on the Grok 4 family. A schema that exceeds a ceiling does not error — it degrades to
best-effort. Validate output anyway.

### 4.B.7 Streaming event taxonomy

Two taxonomies, matching the two endpoints:

**Chat Completions** ([source](https://docs.x.ai/docs/guides/streaming-response)) — byte-compatible with
OpenAI: a sequence of `chat.completion.chunk` objects with `choices[0].delta.content` (plus id, model,
system fingerprint, and usage counters including cached tokens), terminated by `data: [DONE]`.

**Responses API** — OpenAI-style named events. Verified event names include
`response.reasoning_text.delta` and `response.reasoning_summary_text.delta`
([source](https://docs.x.ai/developers/model-capabilities/text/reasoning)). The full ordered taxonomy
(`response.created` → `response.output_item.added` → `response.output_text.delta` →
`response.function_call_arguments.delta` → `response.completed`) is **UNVERIFIED** — xAI does not publish a
consolidated event list, and the reasoning page only exhibits the two reasoning events. Implement the Rust
parser with a catch-all `Unknown(String)` arm and log unrecognised events; do not enumerate exhaustively
from OpenAI's list and assume parity.

### 4.B.8 Guardrails / interception / middleware

**None.** No safety-settings equivalent (unlike Gemini), no guardrail hooks (unlike the OpenAI Agents SDK
and ADK), no moderation endpoint documented. `content_filter` as a finish reason is **UNVERIFIED**. All
input/output guarding is the client's responsibility. For the Rust runtime this means the guardrail layer
must be **provider-independent and client-side** — which, combined with §3.C.5, is now true for two of the
four providers and is the right default anyway.

### 4.B.9 Memory / state primitives

- **Collections** — persistent document storage with an embedding index; semantic/hybrid search across
  multiple collections via `POST /v1/documents/search`; also exposed to the model as a
  **collections search tool** ([source](https://docs.x.ai/developers/files/collections)).
- **Files** — attach documents to a conversation for immediate context; distinct from Collections.
- **Stored responses** — 30-day transcript retention (§4.B.5).
- **Compaction blobs** — an `encrypted_content` compaction item you can persist in *your own* database and
  replay later (§4.C.5). This is the closest thing to a portable memory primitive any of the four providers
  offers, and it is opaque and vendor-locked.

No cross-session memory tool.

### 4.B.10 Retries, error taxonomy, rate limits

Errors ([source](https://docs.x.ai/developers/debugging)):

| Status | Meaning | Retry? |
|---|---|---|
| 202 | **Deferred request queued** — poll `GET /v1/chat/deferred-completion/{request_id}` | n/a (poll) |
| 400 | invalid request structure | No |
| 401 | missing/invalid API key | No |
| 403 | insufficient permissions | No |
| 404 | resource/endpoint not found | No |
| 405 | wrong HTTP method | No |
| 415 | invalid content type | No |
| 422 | malformed payload | No |
| 429 | rate limit exceeded | Yes, backoff |

Note **202 as a non-terminal success** — xAI is the only one of the four that returns a 2xx meaning "come
back later". A Rust client that treats `status.is_success()` as "parse the body as a completion" will break
on deferred requests.

Rate limits ([source](https://docs.x.ai/developers/rate-limits)): two dimensions, **RPS** (derived as
RPM/60, so bursts are punished) and **TPM** — and TPM counts *"prompt, completion, reasoning, and cached
prompt tokens"*. **Cached tokens still consume TPM even though they cost less.** Caching saves money at xAI
but does **not** buy you throughput headroom — the opposite of Anthropic, where cache reads are free against
ITPM (§1.C.1). Tiers auto-upgrade on cumulative spend since 2026-01-01 (Tier 0 $0 → Tier 1 $50 → Tier 2
$250 → Tier 3 $1,000 → Tier 4 $5,000 → Enterprise), and *"tiers never downgrade."* Exceeding a limit
returns 429; documented remedy is exponential backoff.

**No rate-limit response headers are documented** — same gap as Gemini. Two of four providers give you no
server-side quota signal, which settles the design: the Rust rate-limit layer must be a local governor with
optional header reconciliation.

## 4.C Server-side / API-native features usable over plain HTTP

All plain HTTP against `https://api.x.ai/v1` with a bearer token.

### 4.C.1 Prompt caching — **ADOPT (it is automatic), but budget it as unreliable**

Mechanics ([how it works](https://docs.x.ai/developers/advanced-api-usage/prompt-caching/how-it-works) ·
[maximizing hits](https://docs.x.ai/developers/advanced-api-usage/prompt-caching/maximizing-cache-hits) ·
[what breaks it](https://docs.x.ai/developers/advanced-api-usage/prompt-caching/multi-turn) ·
[usage & pricing](https://docs.x.ai/developers/advanced-api-usage/prompt-caching/usage-and-pricing)):

- **Automatic. No configuration, no `cache_control` blocks, no cache objects.** The API checks how many
  messages at the start of your array match a previous request exactly; that prefix is served from cache.
- **Granularity is per-message, not per-token.**
- **Steering is a server-affinity problem, not a cache-key problem.** Cache entries are stored **per
  server**, so you must set the `x-grok-conv-id` HTTP header (or `prompt_cache_key` on the Responses API) to
  a stable UUID/session id so requests route to the same machine. Without it, hit rate collapses for reasons
  that have nothing to do with your prompt.
- **Not guaranteed.** Entries are evicted under memory pressure and requests may be routed elsewhere.
- **TTL: not documented. Minimum cacheable prefix: not documented.** Both **UNVERIFIED** — a real gap
  versus Anthropic (explicit table) and Gemini (explicit table).

What breaks it: editing, removing, or reordering any earlier message. And the one that will actually bite —
*"omitting `reasoning_content` from previous responses"* is named **the top cause of cache misses** for
reasoning models. Fixes: resend the encrypted reasoning content, or use `previous_response_id`.

Verification:
- Chat Completions: `usage.prompt_tokens_details.cached_tokens`
- Responses API: `usage.input_tokens_details.cached_tokens`
- `0` = miss; equal to `prompt_tokens` = full hit (rare); a healthy multi-turn conversation shows the number
  climbing turn over turn.

Price effect (see §4.D): grok-4.6 $2.00 → **$0.50** (75% off); grok-4.5 $2.00 → **$0.30** (85%); grok-4.3
and the 4.20 family $1.25 → **$0.20** (84%); grok-build-0.1 $1.00 → **$0.20** (80%).

> **Verdict: ADOPT — but the work is different from Anthropic's.** There is nothing to *enable*; the
> engineering is (a) always send `x-grok-conv-id`/`prompt_cache_key`, (b) never mutate history, (c) always
> round-trip `reasoning_content`, (d) instrument `cached_tokens` and alarm when it stays at 0. Discount is
> 75–85% versus Anthropic's 90%, with no write premium and no minimum-size table — cheaper to adopt,
> less deterministic to rely on. **Do not model xAI cache savings as guaranteed in the cost plan.**

### 4.C.2 Server-side conversation state — **ADOPT (with `store:false` as a user setting)**

`/v1/responses` + `store: true` + `previous_response_id`; 30-day retention
([source](https://docs.x.ai/docs/guides/chat)).

> **Verdict: ADOPT.** It is the documented fix for the reasoning-content cache-miss problem — the server
> keeps the encrypted reasoning so you do not have to replay it correctly. That makes it a *caching*
> feature as much as a convenience feature. Same caveat as Gemini: local transcript stays authoritative,
> and `store: false` must be exposed for privacy-sensitive work.

### 4.C.3 Server-hosted tools — **ADOPT (best price of the four)**

([tools overview](https://docs.x.ai/docs/guides/tools/overview) ·
[pricing](https://docs.x.ai/developers/pricing))

| Tool | Type string | Cost beyond tokens |
|---|---|---|
| Web search | `web_search` | **$5 / 1,000 calls** |
| X (Twitter) search | `x_search` | **$5 / 1,000 calls** |
| Code interpreter | `code_interpreter` | **$5 / 1,000 calls** (listed as "code execution") |
| Collections search | collections search tool | **UNVERIFIED** |
| Remote MCP | MCP server URL + config | **UNVERIFIED** |

Executed on xAI's servers inside one API call; combinable with client-side function tools in the same
request.

> **Verdict: ADOPT.** $5/1k is half Anthropic's $10/1k and a third of Google's $14/1k. If the runtime needs
> web search at all, route it here regardless of which model is answering. Remote MCP support is a genuine
> differentiator worth a follow-up spike — it lets a hosted model reach the app's own MCP tools without a
> client round trip. **SKIP `x_search`** unless the product has a social-listening use case.

### 4.C.4 Batch API — **SKIP (the discount is not worth the divergence)**

([source](https://docs.x.ai/developers/advanced-api-usage/batch-api) ·
[pricing](https://docs.x.ai/developers/pricing))

- Discount is **20%**, and only for `grok-4.3`, `grok-4.20-0309` (reasoning and non-reasoning), and
  `grok-4.20-multi-agent-0309`. **grok-4.6 and grok-4.5 get no batch discount at all.**
- Supports `/v1/chat/completions`, `/v1/responses`, and the image/video endpoints; server-side and
  client-side tools both allowed.
- Limits: 25 MB per request, 200 MB per file, 50,000 requests/file, 1,000 add-request calls per 30 s per
  team, 2 batch creations/second; result URLs are signed and expire after **1 hour**.
- Turnaround "typically within 24 hours", explicitly **best effort and not guaranteed**.

> **Verdict: SKIP.** 20% on second-tier models only, versus **50%** at all three other providers on their
> current models. The bespoke create/add/poll protocol is real implementation work for the weakest payoff
> in this document. Revisit only if xAI raises the discount or extends it to 4.6.

### 4.C.5 Context management / compaction — **ADOPT — this is xAI's headline feature**

`POST /v1/responses/compact` ([source](https://docs.x.ai/developers/advanced-api-usage/context-compaction)).

```json
// request
{ "model": "grok-4.6",
  "input": [ {"role":"system","content":"..."}, {"role":"user","content":"..."},
             {"role":"assistant","content":"..."} ] }

// response
{ "id": "cmp_<uuid>", "object": "response.compaction", "created_at": 1748895600, "model": "grok-4.6",
  "output": [ { "type": "compaction", "id": "cmp_<uuid>", "encrypted_content": "<opaque blob>" } ],
  "usage": { "input_tokens": 12000, "output_tokens": 800, "total_tokens": 12800,
             "dropped_message_count": 45 } }

// reuse — paste the item back verbatim as the first input item
{ "model": "grok-4.6",
  "input": [ { "type": "compaction", "id": "cmp_abc123", "encrypted_content": "<paste unchanged>" },
             { "role": "user", "content": "new message" } ] }
```

Rules: treat `encrypted_content` as **opaque** — do not parse or modify it; you may store the blob in your
own database and replay it unchanged; **at most one compaction item per request**; the conversation must
still fit the context window *before* you compact; re-compacting a compacted conversation is allowed.
Compaction itself **bills input and output tokens**, and the docs advise using a smaller/cheaper model to do
the compacting if you compact often. `grok-4.6` is the documented supported model; the full support list is
**UNVERIFIED**.

> **Verdict: ADOPT — highest-priority xAI feature, and the single best token-blowup answer of the four
> providers *per unit of implementation effort*.** It is one extra endpoint, one opaque string to persist,
> and it directly collapses the input tokens of a long agent loop — `dropped_message_count` even tells you
> how much history was folded in, giving you a measurable, loggable saving. Two design consequences:
> (1) compact with a **cheap model**, not the answering model — the compaction call is billed separately;
> (2) the blob is **vendor-locked and unreadable**, so the local transcript must remain the source of truth
> and the runtime must be able to rebuild a full-history request if a blob is rejected or the provider
> changes underneath you.

### 4.C.6 Context windows and long context — **watch the 200k price cliff**

grok-4.6: **500,000-token context**, knowledge cutoff January 2026, and the model card states *"no text
output limit"* ([source](https://docs.x.ai/developers/grok-4-6)). grok-4.3 and the 4.20 family: **1M**.
grok-build-0.1: 256k ([models](https://docs.x.ai/docs/models)).

**Every xAI model doubles its per-token price above 200k prompt tokens** — grok-4.6 goes $2.00 → $4.00 in
and $6.00 → $12.00 out ([pricing](https://docs.x.ai/developers/pricing)). Identical cliff to Gemini Pro
(§3.C.6), identical threshold. **200,000 tokens is the universal budget alarm across Google and xAI** and
should be a constant in the Rust runtime, not a per-provider config.

### 4.C.7 Reasoning controls and token accounting — **ADOPT**

([source](https://docs.x.ai/developers/model-capabilities/text/reasoning))

- `reasoning_effort`: `low` / `medium` / `high` (**default**) / `xhigh` (grok-4.6 and later). Supported on
  grok-4.6 and grok-4.5.
- **The default is `high`** — the most expensive setting is what you get if you say nothing. This is the
  opposite of a safe default for a cost-constrained app, and it is different from Gemini 3.7 Flash, which
  defaults to `medium`. **Set `reasoning_effort` explicitly on every xAI request.**
- On `grok-4.20-multi-agent`, `reasoning_effort` does not mean effort at all — it selects **how many agents
  collaborate** (4 vs 16). Same parameter name, completely different cost semantics. A generic
  "effort" abstraction that maps onto this without a special case will multiply the bill by ~4×.
- Reasoning summaries are exposed on grok-4.6 via streaming (`response.reasoning_text.delta`,
  `response.reasoning_summary_text.delta`).
- **Encrypted reasoning content** via `include: ["reasoning.encrypted_content"]` must be resent to preserve
  context — and omitting it is the documented top cause of cache misses (§4.C.1).
- Billing: reasoning tokens are billed as part of total consumption and appear in the usage object; they
  also count against **TPM** (§4.B.10).

> **Verdict: ADOPT, and treat "set effort explicitly" as a hard rule.** Between the `high` default and the
> multi-agent overload, xAI's reasoning controls are the most expensive footgun in this document.

### 4.C.8 Files and Collections — **ADOPT-LATER (Files) / SKIP (Collections)**

Files attach documents to a conversation for immediate context; Collections give persistent storage with an
embedding index and hybrid semantic search via `POST /v1/documents/search`, plus a model-facing collections
search tool ([source](https://docs.x.ai/developers/files/collections)). Storage limits, retention, and
pricing are **UNVERIFIED**.

> **Verdict: ADOPT-LATER for Files** (same reasoning as Gemini: a multimodal-input convenience, not a cost
> lever). **SKIP Collections** — it is a hosted RAG index that would require uploading the user's codebase
> to xAI, duplicating an index the desktop app can hold locally, with unverified pricing. Wrong trade for a
> local-first Tauri app.

### 4.C.9 Token counting — **ADOPT**

`POST /v1/tokenize-text` ([source](https://docs.x.ai/developers/api-reference)). Cost and exact
request/response shape are **UNVERIFIED** — the endpoint is listed in the REST reference navigation but its
detail page was not retrieved.

> **Verdict: ADOPT** (subject to a 15-minute verification spike). All four providers now offer a server-side
> tokenizer; using each provider's own is the only defensible way to do pre-flight budget checks, and it is
> the only way to know how much your *tool schemas* cost.

## 4.D Models and pricing (2026-08-31)

[Pricing](https://docs.x.ai/developers/pricing) · [Models](https://docs.x.ai/docs/models)

Per 1M tokens, USD. Each cell is **standard / long-context (>200k prompt tokens)**.

| Model | Context | Input | Cached input | Output | Batch |
|---|---|---|---|---|---|
| **grok-4.6** (recommended, code + chat) | 500k | **2.00** / 4.00 | **0.50** / 1.00 | **6.00** / 12.00 | none |
| grok-4.5 | 500k | 2.00 / 4.00 | **0.30** / 0.60 | 6.00 / 12.00 | none |
| **grok-4.3** (extended context) | **1M** | **1.25** / 2.50 | **0.20** / 0.40 | **2.50** / 5.00 | −20% |
| grok-4.20-0309 (reasoning) | 1M | 1.25 / 2.50 | 0.20 / 0.40 | 2.50 / 5.00 | −20% |
| grok-4.20-0309 (non-reasoning, fast tier) | 1M | 1.25 / 2.50 | 0.20 / 0.40 | 2.50 / 5.00 | −20% |
| grok-4.20-multi-agent-0309 (beta) | 1M | 1.25 / 2.50 | 0.20 / 0.40 | 2.50 / 5.00 | −20% |
| **grok-build-0.1** (cheapest, coding agent) | 256k | **1.00** / 2.00 | **0.20** / 0.40 | **2.00** / 4.00 | none |

Tools: web search, X search, and code execution are each **$5 / 1,000 calls**
([pricing](https://docs.x.ai/developers/pricing)).

Routing notes:
- **grok-4.3 at $1.25/$2.50 with a 1M window** is xAI's value pick and the only xAI model that is both
  large-context and batch-discounted.
- **grok-4.5 has a cheaper cache read ($0.30) than grok-4.6 ($0.50)** despite identical base pricing — for a
  heavily cached workload, the older model is the cheaper one. Worth an A/B before defaulting to 4.6.
- xAI has **no sub-$1 tier**. Its floor ($1.00 in / $2.00 out) is 3–10× above Gemini 2.5/3.5 Flash-Lite and
  above OpenAI's `gpt-5.6-luna` ($0.20/$1.20). **xAI is not the provider for cheap high-volume calls.**
- Grok-4.6 has **no documented max output limit** — attractive for long generations, but it means
  `max_tokens` truncation detection cannot lean on a known ceiling.

## 4.E Rust ecosystem for xAI

| Crate | Version | Last release | Downloads | Assessment |
|---|---|---|---|---|
| `xai-rs` | **0.0.8** | **2025-12-10** | **318 total, 33 recent** | Effectively dead. Pre-1.0, no meaningful adoption, ~9 months stale, predates grok-4.5/4.6, compaction, and the Responses API. **Do not use.** |
| `async-openai` | 0.41.3 | 2026-07-31 | 7.69M | **The actual answer.** Base-URL override + bearer token covers chat completions, responses, tools, streaming, structured outputs. Does not know about `x-grok-conv-id`, `/v1/responses/compact`, deferred completions, or `/v1/tokenize-text`. |
| `rig-core` | 0.42.0 | 2026-08-17 | 2.45M | Multi-provider; xAI support claimed via OpenAI-compat. Feature depth **UNVERIFIED** — see Part 6. |
| `genai` | 0.7.0-**beta.20** | 2026-08-30 | 338k | Very actively maintained multi-provider crate, but still beta. xAI coverage **UNVERIFIED**. |
| `adk-rust` family | 2.1.0 | 2026-08-25 | 500k+ | README lists xAI among OpenAI-compatible providers ([README](https://raw.githubusercontent.com/zavora-ai/adk-rust/main/README.md)). |

**Assessment:** xAI is the one provider where an off-the-shelf Rust crate genuinely covers the core path
today — because that path is OpenAI's. Everything xAI-specific and cost-relevant (compaction, cache
steering header, deferred, tokenize) is unimplemented anywhere in the Rust ecosystem and must be
hand-rolled regardless of which crate is chosen. That is a small, well-bounded amount of code — perhaps
300 lines — and it is the highest-value 300 lines in the xAI adapter.

---

# PART 5 — CROSS-PROVIDER SYNTHESIS

**Verified on 2026-08-31.** Every claim below is carried forward from Parts 1–4 and is sourced there; this
Part adds no new external facts, only the design conclusions drawn from them.

## 5.1 The common denominator — what a provider-agnostic Rust trait must express

These fourteen properties hold for **all four** providers. They are the contract; everything else is an
adapter concern.

| # | Property | Anthropic | OpenAI | Google | xAI |
|---|---|---|---|---|---|
| 1 | Loop driven by "did the assistant emit client-executable tool calls?" | ✔ (`stop_reason: tool_use`) | ✔ (`function_call` items) | ✔ (`functionCall` / `function_call`) | ✔ (`function_call` items) |
| 2 | No server-side iteration cap; the client owns `max_iterations` | ✔ | ✔ | ✔ | ✔ |
| 3 | Tools declared as JSON Schema with an **object root** | ✔ | ✔ | ✔ | ✔ |
| 4 | Parallel tool calls **on by default**, disableable | ✔ | ✔ | ✔ | ✔ |
| 5 | Multi-turn works by **replaying full history** (server state is optional everywhere) | ✔ | ✔ | ✔ | ✔ |
| 6 | Structured output via a JSON-Schema subset + a `strict`-style flag | ✔ | ✔ | ✔ | ✔ |
| 7 | Streaming is **SSE**, and every vendor tells you to tolerate unknown event types | ✔ | ✔ | ✔ | ✔ |
| 8 | A reasoning/thinking **effort** knob, billed as output tokens | ✔ | ✔ | ✔ | ✔ |
| 9 | Opaque reasoning state that must be **replayed byte-for-byte** when stateless | signature | `encrypted_content` | thought signature | `reasoning_content` |
| 10 | **Automatic/implicit prompt caching** on a prefix match, no code change to benefit | ✔ | ✔ | ✔ | ✔ |
| 11 | `usage` reports **cached tokens** as a distinct field | ✔ | ✔ | ✔ | ✔ |
| 12 | 429 + documented exponential backoff with jitter | ✔ | ✔ | ✔ | ✔ |
| 13 | A **server-side token-counting endpoint** | ✔ | ✔ | ✔ | ✔ |
| 14 | **Handoffs/sub-agents are client-side** (or a proprietary non-handoff) | ✔ | ✔ (SDK-side) | ✔ (ADK-side) | ✔ (ensemble ≠ handoff) |

Property 14 is the load-bearing one for the owner's decision: **no provider offers a wire-level handoff.**
A native Rust orchestrator is not reimplementing something the vendors give away — it is implementing the
thing all four vendors implement in their own client libraries.

### The trait, concretely

```rust
/// One provider adapter. Implemented four times; the runtime knows only this.
#[async_trait]
pub trait ModelProvider: Send + Sync + 'static {
    /// Provider-specific request extensions the runtime must not interpret
    /// (Anthropic betas, Gemini safety settings, xAI `x-grok-conv-id`, …).
    type Extensions: Default + Send + Sync;

    fn id(&self) -> ProviderId;
    /// Per-MODEL, not per-provider: effort sets, cache minimums, ceilings, tier cliffs.
    fn capabilities(&self, model: &ModelId) -> &ModelCapabilities;

    async fn complete(&self, req: &Request) -> Result<Turn, ProviderError>;
    async fn stream(&self, req: &Request)
        -> Result<BoxStream<'static, Result<TurnEvent, ProviderError>>, ProviderError>;

    /// Server-side counting. Never a local tokenizer.
    async fn count_input_tokens(&self, req: &Request) -> Result<u64, ProviderError>;

    /// Mutates the request in place to express the cache policy. A no-op where
    /// caching is purely implicit (Gemini, xAI) beyond setting the affinity key.
    fn apply_cache_policy(&self, req: &mut Request, policy: &CachePolicy);

    /// Server-side compaction where it exists; `Err(Unsupported)` drives the
    /// runtime's client-side fallback (required for Google).
    async fn compact(&self, conv: &Conversation, target: TokenBudget)
        -> Result<CompactionArtifact, ProviderError>;

    /// Some providers publish quota headers, two of four publish none.
    fn read_rate_limit(&self, headers: &HeaderMap) -> Option<RateLimitState>;
    fn classify_error(&self, status: StatusCode, body: &[u8]) -> ErrorClass;
}
```

Associated / supporting types — these are where the divergence in §5.2 is absorbed:

```rust
pub struct Request {
    pub model: ModelId,
    pub system: Vec<ContentBlock>,        // adapters decide top-level vs first message
    pub history: Vec<Item>,
    pub tools: Vec<ToolDef>,
    pub tool_choice: ToolChoice,          // Auto | Required | None | Named(String) | Allowed(Vec<String>)
    pub max_output_tokens: Option<u32>,
    pub effort: Effort,                   // None|Minimal|Low|Medium|High|XHigh -> mapped per model
    pub output_format: Option<SchemaSpec>,
    pub cache: CachePolicy,
    pub state: StateMode,
    pub ext: ProviderExtensions,
}

pub enum Item {
    User(Vec<ContentBlock>),
    Assistant(Vec<ContentBlock>),
    ToolCall  { id: ToolCallId, name: String, args: serde_json::Value },
    ToolResult{ id: ToolCallId, content: Vec<ContentBlock>, is_error: bool },
    /// Thinking signatures, encrypted reasoning, compaction blobs. NEVER inspected,
    /// always replayed verbatim, tagged with the provider that minted it.
    Opaque    { provider: ProviderId, kind: OpaqueKind, bytes: Vec<u8> },
}

pub enum StopReason { EndTurn, ToolUse, MaxTokens, StopSequence,
                      Refusal, Safety, Recitation, MalformedToolCall,
                      PauseTurn, Other(String) }

pub enum TurnEvent { TurnStart{ usage: PartialUsage }, ItemStart{ idx: u32, kind: ItemKind },
                     TextDelta{ idx: u32, text: String },
                     ToolArgsDelta{ idx: u32, partial_json: String },   // always a STRING fragment
                     ThinkingDelta{ idx: u32, text: String },
                     ItemStop{ idx: u32 }, TurnStop{ stop: StopReason, usage: Usage },
                     Unknown(String) }                                   // mandatory

pub enum CachePolicy { Off, Automatic { affinity_key: Option<String> },
                       Explicit { breakpoints: Vec<BreakpointHint>, ttl: CacheTtl },
                       Handle { id: String } }        // Gemini cachedContents

pub enum StateMode { LocalOnly, ServerAssisted { last_id: Option<String> } }

pub enum ErrorClass { Retryable{ after: Option<Duration> }, Overloaded, QuotaExhausted,
                      Deferred{ poll_url: String }, Fatal }

pub struct Usage { pub input: u64, pub cached_read: u64, pub cache_write: u64,
                   pub output: u64, pub reasoning: u64, pub tool: u64 }
```

Three non-obvious decisions embedded above, each paid for by a finding in Parts 1–4:

- **`Item::Opaque` exists from day one.** Four providers, four kinds of unreadable state that must survive a
  round trip (§1.B.7 signatures, §2.C.7 encrypted reasoning, §3.C.7 thought signatures, §4.C.7
  `reasoning_content` — plus §2.C.5 and §4.C.5 compaction blobs). Retrofitting this means rewriting the
  history type.
- **`ToolCall.args` is `serde_json::Value`, never `String`.** Gemini hands you an object; OpenAI and xAI
  hand you a JSON string; Anthropic hands you an object. Only the adapter does `from_str`.
- **`capabilities()` is keyed by *model*, not provider.** Cache minimums (512–4,096 tokens), effort sets
  (Haiku 4.5 has none; 3.7 Flash has no `minimal`; xAI adds `xhigh`), context cliffs, and batch eligibility
  are all per-model facts. A per-provider capability struct will be wrong within a month.

## 5.2 Divergence — and how one Rust abstraction absorbs each

| Axis | The four shapes | Absorption strategy |
|---|---|---|
| **Tool schema** | Anthropic `input_schema` inside a tool object · OpenAI/xAI flat `{"type":"function", …, "strict":true}` · Gemini **two** shapes: legacy `tools[].functionDeclarations[]` vs Interactions flat item | Canonical `ToolDef { name, description, schema: Value, strict: bool }`; each adapter owns a `fn serialize_tools()`. **Serialize with deterministic key order** — tool JSON is part of every cache prefix (§5.5 trap 3). |
| **Tool arguments encoding** | Object: Anthropic, Gemini · JSON **string**: OpenAI, xAI | `args: serde_json::Value` in core; adapters parse on ingest, re-serialize on egress. Never leak a `String` into the runtime. |
| **Tool result shape** | Anthropic: **all** `tool_result` blocks in **one** user message · OpenAI: individual `function_call_output` items · xAI: tool-role message · Gemini: `functionResponse` part with role `"user"` (legacy) / `function_result` with a **typed content array** supporting images (Interactions) | `Item::ToolResult` with `Vec<ContentBlock>`. The Anthropic adapter must *batch* — the one-message rule is a real behavioural trap (§1.B.3). Gemini's image-returning results degrade to text elsewhere; put that downgrade in the adapter, not in tool code. |
| **Call-id semantics** | Anthropic `tool_use.id` · OpenAI **two** ids (`id` + `call_id`; only `call_id` returns) · Gemini `id` → renamed `call_id` on reply · xAI OpenAI-style | `ToolCallId(String)` newtype + an adapter-local map from runtime id → provider id(s). The runtime never invents ids. |
| **Streaming taxonomy** | Anthropic: block-index model (`content_block_delta` + `delta.type`) · OpenAI: dotted event names naming the item kind · Gemini: `interaction.created`/`step.start`/`step.delta`/`step.stop`/`interaction.completed`/`done` · xAI: OpenAI `chat.completion.chunk` + `[DONE]`, or OpenAI-style dotted names on `/v1/responses` | Normalize to `TurnEvent` keyed by `(index, kind)` — the Anthropic model is the *lowest common denominator* and the other three collapse onto it cleanly. `Unknown(String)` arm is mandatory; all four vendors explicitly warn about new event types. |
| **Tool-argument streaming** | All four stream **partial JSON strings**, not partial objects | `ToolArgsDelta{ partial_json: String }`, accumulate, parse **once** at `ItemStop`. Never parse a fragment. |
| **Caching mechanics** | Anthropic: explicit `cache_control` breakpoints (max 4), TTL 5m/1h, **write premium** 1.25×/2×, per-model minimum, 20-block lookback · OpenAI: implicit by default + explicit mode, 30m TTL on GPT-5.6+, `prompt_cache_key` for routing, 1,024-token minimum · Google: implicit free (no storage cost) + explicit `cachedContents` with **hourly rent** and a legacy-API-only restriction · xAI: fully automatic, **per-message** granularity, server affinity via `x-grok-conv-id`, no documented TTL or minimum | `CachePolicy` enum + `apply_cache_policy()`. Anthropic is the only one that needs breakpoint placement; the other three need an **affinity/cache key** and prefix hygiene. The runtime's job is identical everywhere: **build the prefix in a stable order and never mutate it.** Make the prefix builder a single shared function, not four. |
| **System prompt handling** | Anthropic: top-level `system` array, cacheable, ordered `tools → system → messages` · OpenAI: system/developer message inside `input` · Gemini: `system_instruction`, **must be re-sent every turn even in stateful mode** · xAI: system-role message in `input` | `Request.system: Vec<ContentBlock>`; adapters place it. The Gemini re-send rule is an adapter invariant, not a runtime concern. |
| **Multi-turn state** | Stateless: Anthropic Messages, Gemini `generateContent`, xAI chat completions · Server-stored: OpenAI `previous_response_id` (30d) + Conversations (indefinite), Gemini Interactions (55d/1d), xAI Responses (30d) | `StateMode`. **The local transcript is always authoritative**; server state is a cache with a TTL that differs per provider and can vanish. Every adapter must be able to rebuild a full-history request on demand. |
| **Does server state save tokens?** | OpenAI: **no** — "all previous input tokens … are billed" · Gemini: **claims yes** (better cross-turn caching) · xAI: **indirectly yes** (keeps `reasoning_content` so you stop missing cache) · Anthropic: n/a | Never model server state as a token saving in the cost planner. Model it as a *cache-hit-rate improvement* on Gemini and xAI, and measure it. |
| **Reasoning / thinking accounting** | Billed as output tokens on all four. Names differ: `usage.output_tokens_details.reasoning_tokens` (OpenAI) · `total_thought_tokens` (Google) · in-usage reasoning tokens (xAI) · thinking in output (Anthropic). Defaults differ dangerously: **xAI defaults to `high`**, Gemini 3.7 Flash to `medium`, OpenAI/Anthropic per-model | Single `Effort` enum + a **per-model mapping table** that also carries the *default*. The runtime **always sends effort explicitly** — never relies on a provider default. Special-case xAI's multi-agent model, where `reasoning_effort` selects **agent count** (4 vs 16), not depth. |
| **Structured output enforcement** | Anthropic: strict subset, **no recursion**, unsupported features → 400 · OpenAI: strict, requires `additionalProperties:false` + all fields in `required`, **supports `$ref` recursion** · xAI: constrained decoding with **numeric ceilings** (maxLength 2048, maxItems 256, maxProperties 64) and best-effort keywords beyond them · Gemini: documented subset, guarantee stated only as "syntactically correct JSON" | `SchemaSpec` + a **per-provider schema lint pass that runs at build time**, failing loudly on unsupported constructs rather than at runtime. And validate the parsed output on every provider — none of the four guarantees semantic validity. |
| **Error / rate-limit shape** | Headers: Anthropic full set with **RFC 3339** resets · OpenAI full set with **duration strings** (`6s`, `1m30s`) + `retry-after-ms` · Google **none documented** · xAI **none documented**. Statuses: Anthropic **529 overloaded** · xAI **202 deferred (a 2xx that is not a result)** · Gemini 503. Two distinct 429s at both Anthropic (spend cap, no `retry-after`) and OpenAI (`insufficient_quota`) | `ErrorClass` + `Option<RateLimitState>`. **Design for the absence of headers** — half the providers give none, so the governor must be a local token bucket that *optionally* reconciles. Branch retries on `ErrorClass`, never on the HTTP status alone. |
| **Context management** | Anthropic: context **editing** (`clear_tool_uses`, `clear_thinking`) **and** compaction (`compact_20260112`) · OpenAI: `context_management:[{type:"compaction",compact_threshold}]` **plus** `POST /v1/responses/compact` · xAI: `POST /v1/responses/compact` returning a replayable opaque item · Google: **nothing** | `compact()` returning `CompactionArtifact`, with `Err(Unsupported)` triggering a **client-side summarise-and-replace** fallback. Because Google has nothing, the client-side path must exist and be good — so build it first and treat the server-side variants as accelerators. |

## 5.3 Ranked adoption shortlist

Ranked by expected effect on the token-blowup/cost problem per unit of implementation effort. "HTTP" =
callable from a plain `reqwest` client with no vendor SDK.

| # | Feature | Providers | Concrete benefit | HTTP? |
|---|---|---|---|---|
| **1** | **Prompt/context caching + a shared prefix-hygiene discipline** | **all four** | **~90% off the repeated prefix** (Anthropic 0.1×, OpenAI 0.1×, Gemini 0.1×, xAI 0.75–0.85× off). On Anthropic, cache reads additionally **do not count against ITPM**, multiplying effective throughput. This is the largest single lever in the document and it is available on every provider. | **Yes** |
| **2** | **Server-side compaction, with a client-side fallback** | Anthropic, OpenAI, xAI (**not** Google) | Bounds input growth in long agent loops instead of letting it compound. xAI's `dropped_message_count` makes the saving directly measurable. One endpoint or one request field each. | **Yes** |
| **3** | **Anthropic context editing (`clear_tool_uses`)** | Anthropic | Kills the canonical agent failure: a 200-line file dump from turn 3 still being re-sent at turn 30. Configurable trigger/keep, and `clear_at_least` guards the cache re-write. Nothing equivalent exists elsewhere — it is a reason to route long tool-heavy sessions to Claude. | **Yes** |
| **4** | **Explicit reasoning-effort on every request** | all four | Reasoning bills as output. **xAI defaults to `high`** and Gemini to `medium`; sending nothing is choosing the expensive option. A one-field change with immediate output-token reduction on routine turns. | **Yes** |
| **5** | **Cheap-tier model routing** | all four | Not an API feature but the biggest absolute number here: `gemini-2.5-flash-lite` $0.10/$0.40 and `gpt-5.6-luna` $0.20/$1.20 versus $5/$25 for Opus 5 — **25–50×**. Requires `capabilities()` per model and honest per-task routing. Note xAI has **no** sub-$1 tier. | **Yes** |
| **6** | **Server-side token counting before dispatch** | all four | The only tokenizer-correct pre-flight check; local tokenizers cannot count images, tool schemas, or cache interaction. Enables budget gates and routing decisions *before* spending. Gemini additionally breaks out `total_tool_use_tokens`, which tells you directly whether tool schemas are the blowup. | **Yes** |
| **7** | **Server-side conversation state — Gemini Interactions + xAI Responses only** | Google, xAI | Not a token saving at OpenAI (explicitly billed anyway) but at xAI it is the documented fix for the `reasoning_content` cache-miss problem, and Google claims better cross-turn caching. Adopt as a **cache-hit-rate** feature; keep the local transcript authoritative. | **Yes** |
| **8** | **Hosted web search, routed to the cheapest provider** | xAI **$5**/1k · Anthropic $10/1k · Google $14/1k | 2.8× price spread for the same capability. Routing search to xAI regardless of the answering model is free money, and it removes a scraping subsystem from the Rust app. | **Yes** |
| **9** | **Hosted code execution** | Anthropic, OpenAI, Google, xAI ($5/1k) | Avoids building and sandboxing a local interpreter. Google's carries no separate fee found. Adopt where a sandbox would otherwise be built; skip if the desktop app already runs code locally under its own permissions. | **Yes** |
| **10** | **Batch API — offline paths only** | Anthropic 50%, OpenAI 50%, Google 50%, **xAI 20% on second-tier models only** | Halves cost on evals, bulk indexing, background summarisation. Useless for interactive turns. Google's batch is `generateContent`-only, so adopting it means maintaining the legacy Gemini encoder too. | **Yes** |
| 11 | Files APIs | all four | Multimodal attachment convenience (Gemini's is free, 20 GB, 48 h). Does **not** attack cost. Adopt when PDFs/audio/video input is needed. | Yes |
| 12 | Hosted RAG / vector stores (OpenAI `file_search`, Gemini File Search, xAI Collections) | 3 of 4 | Would mean uploading the user's codebase to a vendor and duplicating an index a local-first Tauri app can own. **SKIP.** | Yes |
| 13 | Managed agent runtimes (Anthropic Managed Agents $0.08/session-hour, Vertex Agent Engine, Grok Build) | 3 of 4 | These **replace** the Rust runtime rather than help it, and each is single-vendor. **SKIP** — directly contrary to the in-house decision. | Yes |
| 14 | xAI multi-agent model (`grok-4.20-multi-agent`) | xAI | Bills every sub-agent's tokens; the docs warn of significantly higher usage. Not a handoff. **SKIP as a default**; a deliberate quality knob at most. | Yes |

**Everything in rows 1–10 is plain HTTP.** Not one of the cost-relevant features requires a vendor SDK. That
is the empirical justification for the hand-roll decision in §5.4 / Part 6.

## 5.4 Crate vs hand-roll — the recommendation

> **Recommendation: hand-roll the four provider adapters over `reqwest` + `serde` + `eventsource-stream`,
> inside the runtime's own `ModelProvider` trait (§5.1). Do not adopt a multi-provider crate as the
> abstraction layer.** Use `async-openai` **tactically** — as the fastest route to the OpenAI *and* xAI core
> paths via base-URL override — behind your own trait, and be prepared to drop it.

The full crate-by-crate evidence, versions, and the four disqualifying gaps are in **Part 6**, which follows.
The short form: the crates are healthy on *chat*, uniformly thin on *the features that cost money*
(caching mechanics, compaction, context editing, per-model cache minimums, quota headers), and none of them
models the four-provider divergence table in §5.2 without lossy flattening — which is precisely where the
runtime's value lies.

## 5.5 Traps — provider quirks that bite implementers

**1. Silent cache misses. The most expensive failure mode in this document.**
No provider errors when caching fails. Anthropic silently does not cache a prefix below the per-model
minimum (512 / 1,024 / 2,048 / 4,096 tokens depending on model). Gemini silently does not cache below
2,048–4,096. xAI's cache is explicitly "not 100% guaranteed" and evicts under memory pressure. The **only**
detection is instrumenting the usage field on every single call — `cache_read_input_tokens` (Anthropic),
`input_tokens_details.cached_tokens` (OpenAI, xAI), `total_cached_tokens` (Gemini) — and alarming when it
stays at zero across repeated same-prefix requests. Build that alarm before building anything else.

**2. Cache invalidation is a *prefix* property, and the prefix includes things you forgot are in it.**
Anthropic invalidates `tools → system → messages` in cascade: change a tool definition and all three levels
die; toggle web search, citations, `speed`, `tool_choice`, images, thinking, or `effort` and you invalidate
from that level onward. OpenAI's rendered prefix includes tool definitions **and their ordering**,
`parallel_tool_calls`, output format, reasoning settings, verbosity, and compaction. xAI breaks on any edit,
removal, or reorder of an earlier message. A timestamp in the system prompt, a `HashMap`-ordered tool list,
or a "current file" header rotated per turn will destroy the cache and produce no error anywhere.

**3. Non-deterministic JSON serialization silently defeats every cache.**
This is the Rust-specific form of trap 2. `serde_json` with a `HashMap` emits keys in arbitrary order;
caching requires a **100% exact byte match** (Anthropic's wording). Use `BTreeMap` or `preserve_order`
throughout the request builder, and make the prefix builder one shared function so the ordering rule is
enforced in one place rather than four.

**4. Dropping opaque reasoning state breaks continuity *and* caching.**
Anthropic thinking-block `signature`s, OpenAI reasoning items' `encrypted_content`, Gemini thought
signatures, xAI `reasoning_content` — all four must be replayed verbatim in stateless mode. xAI names
omitting `reasoning_content` **the top cause of cache misses**; OpenAI's guide says to preserve *every* item
in the `output` array, including assistant `phase` values. A history type that keeps only text and tool
calls is not merely lossy — it converts every turn into a cache miss.

**5. Tool-call id mismatches and the one-message rule.**
OpenAI carries **two** ids on a function call (`id` and `call_id`) and only `call_id` goes back. Gemini
renames `id` → `call_id` between the call and the result in the Interactions API, while the legacy API uses
`id` on both. Anthropic requires **all** `tool_result` blocks for a parallel batch to be returned in a
**single** user message — splitting them across messages does not error; it silently trains the model to
stop making parallel calls. A failed tool still needs a `tool_result` with `is_error: true`; dropping it
breaks the pairing.

**6. `max_tokens` truncation is detected differently on every provider — and one provider has no ceiling.**
Anthropic: `stop_reason == "max_tokens"`. Gemini: `finishReason == "MAX_TOKENS"`. OpenAI/xAI: OpenAI's
`length` finish reason — and xAI's `finish_reason` set is **not documented** (UNVERIFIED), while grok-4.6
advertises **no text output limit** at all, so there is no ceiling to reason about. Truncation detection
must be a per-adapter function, and the runtime must treat "truncated" as a distinct outcome from "done" —
silently returning half a JSON document as a success is the classic downstream corruption.

**7. Streaming edge cases that produce a silent, empty-looking stream.**
Anthropic: with `display: "omitted"` (the **default** on Opus 5 / Sonnet 5 / Fable 5 / Mythos 5 / Opus 4.8 /
4.7) a thinking block opens, emits **only** a `signature_delta`, and closes — zero text deltas, so a UI
waiting for text sees a long silence; server-side fallback emits a `content_block_start`/`stop` pair with
nothing between. Gemini requires you to accumulate `arguments_delta` string fragments before parsing.
Anthropic emits one complete key/value at a time in `input_json_delta`, so partial-JSON parsers stall.
All four vendors warn that new event types will appear: **a non-exhaustive match is a hard requirement, not
a nicety.**

**8. Rate-limit headers exist on half the providers; the 429s are not interchangeable.**
Anthropic resets are **RFC 3339 timestamps**; OpenAI resets are **duration strings** (`6s`, `1m30s`) plus
`retry-after-ms`; Google and xAI document **no headers at all**. Anthropic's spend-cap 429 carries **no**
`retry-after` and retrying never succeeds (`enforced_spend_limit_reached`); OpenAI's `insufficient_quota`
429 is the same trap. A blind exponential-backoff loop spins forever on both. Also: Anthropic returns **529
`overloaded_error`**, which is outside the usual 5xx handling, and xAI returns **202** for deferred
completions — a 2xx that is *not* a result body.

**9. Cached tokens count against xAI's TPM but not against Anthropic's ITPM.**
The same optimisation buys throughput headroom at Anthropic and buys none at xAI, where TPM explicitly
counts "prompt, completion, reasoning, and cached prompt tokens". Capacity planning cannot be shared across
providers.

**10. The 200,000-token price cliff.**
Google Pro models and **every** xAI model roughly double their input price above 200k prompt tokens (Gemini
3.1 Pro $2 → $4 in, $12 → $18 out; grok-4.6 $2 → $4 in, $6 → $12 out). Crossing it is silent. 200k must be a
hard alarm in the runtime, not a per-provider footnote. (Anthropic, by contrast, includes the full 1M window
at standard pricing on 4.6+.)

**11. Feature coupling that forces API choice.**
Gemini explicit caching requires the **legacy** `generateContent` API; the Interactions API supports only
implicit caching. Gemini's Batch API is likewise `generateContent`-only. Choosing explicit caching or batch
therefore costs you server-side state and the step-level streaming taxonomy — an architectural fork, not a
flag. Anthropic has its own version: structured outputs are **incompatible with document citations** (400),
and clearing tool results invalidates the cached prefix.

**12. Provider defaults are not neutral.**
xAI's `reasoning_effort` defaults to `high`. OpenAI's cache retention default flips based on whether the org
has Zero Data Retention. Gemini's `store` defaults to `true` (55-day retention of user data). xAI's `store`
retains 30 days. For a desktop app handling user source code, **two of the four providers retain
conversation content by default** — `store: false` must be a first-class, user-visible setting, not a
constant buried in an adapter.

---

# PART 6 — RUST ECOSYSTEM REALITY CHECK AND THE CRATE-VS-HAND-ROLL RECOMMENDATION

**Verified on 2026-08-31.** Every version, release date, and download count below was read from the
**crates.io API on 2026-08-31**, not from memory or from a README. Where a project's README and the registry
disagree, the registry is cited — this happens with `rig`, whose README still advertises 0.36.0 while
crates.io serves **0.42.0**.

## 6.A Provider-specific crates

| Crate | Version | Last release | Downloads (total / recent) | Verdict |
|---|---|---|---|---|
| **Anthropic** | | | | |
| `anthropic-ai-sdk` | 0.2.27 | 2026-01-11 | 44,016 / 3,918 | Most current community Anthropic crate — and **7½ months stale** against an API surface that shipped compaction, structured-outputs GA, adaptive thinking, and task budgets in that window. |
| `anthropic-sdk` | 0.1.5 | 2024-07-23 | 77,643 / 8,181 | Abandoned (25 months). High historical downloads, do not be fooled. |
| `clust` | 0.9.0 | 2024-06-30 | 14,638 / 243 | Abandoned. |
| `misanthropy` | 0.0.8 | 2025-06-08 | 12,731 / 332 | Abandoned; pre-0.1. |
| **OpenAI** | | | | |
| `async-openai` | **0.41.3** | **2026-07-31** | **7,687,332 / 2,394,137** | **The healthiest LLM crate in Rust by an order of magnitude.** Listed on OpenAI's own libraries page as a *community* library, with the explicit caveat that OpenAI "does not verify the correctness or security of these projects". |
| **Google** | | | | |
| `gemini-rust` | 2.0.0 | 2026-07-10 | 47,227 / 15,510 | Active, focused Gemini client. Interactions API support **UNVERIFIED**. |
| `google-ai-rs` | 0.3.0 | 2025-10-07 | 6,985 / — | ~11 months stale; predates Gemini 3. |
| `google-generative-ai-rs` | 0.3.4 | 2024-12-23 | 37,394 / — | Abandoned (20 months). |
| `googleapis-tonic-google-ai-generativelanguage-v1beta` | 0.42.0 | 2026-06-20 | 59,483 / — | Generated tonic/gRPC bindings — protobuf plumbing, not an SDK, and the Interactions API is REST/SSE. |
| **xAI** | | | | |
| `xai-rs` | **0.0.8** | **2025-12-10** | **318 / 33** | Effectively dead. **Do not use.** The OpenAI-compatible path via `async-openai` is strictly better. |

## 6.B Multi-provider crates and agent frameworks

| Crate | Version | Last release | Downloads (total / recent) | Providers | Streaming | Tools | Caching |
|---|---|---|---|---|---|---|---|
| `rig-core` | **0.42.0** | 2026-08-17 | 2,454,484 / 1,413,493 | 24+ incl. all four (Anthropic, OpenAI, Gemini, xAI) | ✔ (`streaming` module) | ✔ (`tool` module, `#[rig_tool]`) | **Partial — verified `CacheControl` + `CacheTtl` in the Anthropic provider module; nothing equivalent found for the other three.** |
| `genai` | **0.7.0-beta.20** | **2026-08-30** | 337,858 / 120,742 | 26+ incl. all four | ✔ | ✔ (provider-neutral tool hints) | **Best in class for a crate — README claims OpenAI `prompt_cache_key` + cache retention and Anthropic tool-level caching, plus `ReasoningEffort` up to `Max`/`XHigh`.** Still **beta**, with fallible-constructor breaking changes landing in 0.7.0. |
| `adk-rust` family (`adk-core` + ~42 crates) | 2.1.0 | 2026-08-25 | 47,332 core / 500k+ family | Gemini (default), OpenAI, Anthropic, xAI via OpenAI-compat, Bedrock, Azure, local | ✔ SSE | ✔ `#[tool]` macro + MCP | Claimed "via Anthropic client integration" only. |
| `llm_adapter` | 0.2.23 | 2026-08-26 | 125,666 / 68,085 | multi | ✔ | ✔ | **UNVERIFIED** |
| `liter-llm` | 1.18.4 | 2026-08-28 | 54,104 / 37,519 | claims 165 | ✔ SSE + AWS EventStream | ✔ parallel tools, structured outputs | **Not mentioned in the README — UNVERIFIED.** Note: its "caching" claim is a *proxy-side response cache*, which is a different thing from provider prompt caching. |
| `llm` | 1.3.8 | 2026-04-19 | 116,358 / 29,529 | multi | ✔ | ✔ | **UNVERIFIED** |
| `allms` | 0.42.0 | 2026-05-27 | 49,013 / 702 | multi | — | — | Low recent adoption (702). |
| `langchain-rust` | 4.6.0 | 2024-10-06 | 153,121 / 12,222 | multi | — | — | **Abandoned (22 months).** |
| `openai-agents` | 0.1.1 | 2026-03-02 | 3,721 | OpenAI | — | — | Toy-scale adoption; not a viable dependency. |

Two observations that matter more than any individual row:

1. **The ecosystem is healthy on *chat* and thin on *cost*.** Every serious crate does streaming and tool
   calling. Only `genai` and (partially) `rig` model prompt caching at all, and **nothing** in the ecosystem
   models context compaction, Anthropic context editing, per-model cache minimums, cache-diagnostic fields,
   or quota headers. Those are exactly rows 1–4 of the §5.3 shortlist — i.e. the crates are strongest
   precisely where the problem isn't and absent precisely where it is.

2. **`adk-rust` is a genuine surprise and still not the answer.** It is the only Rust *agent framework* with
   real structure (agents, runners, sessions, artifacts, plugins, skills, telemetry, MCP) and real adoption
   (500k+ downloads across the family, released 2026-08-25). But it is a single-vendor-shaped community
   project (`zavora-ai`) with **43 crates** of surface area, no Google affiliation, and no evidence of
   caching or compaction support for three of the four providers. Adopting it means inheriting somebody
   else's agent semantics — the precise thing the owner's decision rejects.

## 6.C The recommendation

> **Hand-roll the four provider adapters over `reqwest` + `serde` + an SSE decoder, behind the runtime's own
> `ModelProvider` trait (§5.1). Use `async-openai` tactically for the OpenAI *and* xAI core paths, behind
> that trait, as an implementation detail you can drop. Do not adopt `rig`, `genai`, `llm`, `liter-llm`, or
> `adk-rust` as the abstraction layer.**

Four reasons, in order of weight:

**1. The features that solve the actual problem are unimplemented in every crate.**
The app's problem is token blowup. §5.3 rows 1–4 — caching mechanics, compaction, context editing, explicit
effort — are the fix. `genai` covers part of row 1; nothing covers rows 2 or 3 for any provider. Adopting a
crate therefore does not save the work; it adds a layer between the runtime and the work, and every
provider-specific cost feature has to be plumbed through somebody else's type system as an escape hatch.

**2. Every cost-relevant feature is plain HTTP.** This is the decisive fact and it is empirical, not
aesthetic: of the fourteen shortlist rows in §5.3, **all fourteen** are reachable with a bearer token and a
JSON body. No vendor SDK is required for any of them, on any of the four providers. The usual reason to
adopt a client crate — "the SDK does something the HTTP API cannot" — simply does not apply here.

**3. A multi-provider crate must flatten §5.2, and the flattening is where the value leaks.**
Any crate that presents one `Message` type across four providers has already decided how to handle
tool-arg encoding (object vs string), the tool-result batching rule, dual call-ids, four streaming
taxonomies, and — critically — **opaque reasoning state**. `Item::Opaque` (§5.1) is a design the runtime
needs and a generic crate has little incentive to expose faithfully. Dropping a thinking signature is
invisible in a type system and expensive on the invoice (§5.5 trap 4).

**4. Stability risk is real and asymmetric.** `rig` warns in its own README: *"Here be dragons! … future
updates will contain breaking changes"* — with a README/registry version skew of six minor versions
(0.36.0 vs 0.42.0). `genai` is at `0.7.0-beta.20` with fallible constructors landing as a breaking change.
`adk-rust` is 43 crates from one author. Against that, `reqwest` + `serde` are the two most stable
dependencies in the Rust ecosystem, and a hand-rolled adapter breaks only when the vendor's wire format
breaks — which you must respond to anyway.

**Why `async-openai` is the one exception.** It is the healthiest crate here by two orders of magnitude
(7.69M downloads, 0.41.3 on 2026-07-31), OpenAI itself links it, and — verified on docs.rs — its
`CreateResponse` already carries `prompt_cache_key`, `prompt_cache_retention`, `previous_response_id`,
`store`, `reasoning`, `tools`, and `stream`. Because xAI's core path *is* OpenAI's wire format (§4.A), one
crate covers two of the four providers' request/response types with a base-URL override, and it is
configurable for compatible providers by design. Its known gaps are exactly the ones you were going to
hand-write anyway: no `context_management`/compaction field, no `x-grok-conv-id`, no
`/v1/responses/compact`, no `/v1/tokenize-text`. Keep it **behind** `ModelProvider` so those gaps are
patched in one adapter rather than leaking into the runtime.

### Cost of the hand-roll, stated honestly

Four adapters, each roughly: request/response types, an SSE decoder mapping onto `TurnEvent`, an error
classifier, a cache-policy applier, and a capabilities table. The shared work — prefix builder, tool
executor, retry governor, compaction fallback, budget accounting — is written once and is the same code a
crate-based design would need anyway. The genuinely provider-specific, genuinely unavoidable surface is
small: on xAI it is ~300 lines (§4.E); Anthropic and OpenAI are larger because their cost features are
richer; Gemini is the largest because of the two-wire-format fork (§3, preamble).

That is a bounded, one-time cost against a permanent dependency on somebody else's flattening of the
divergence table — and against an ecosystem that, as of 2026-08-31, does not implement the features the
app is adopting these providers *for*.
