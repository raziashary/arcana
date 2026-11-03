# 02 — Providers and the SDK-Native Surface

| | |
|---|---|
| **Parent** | `00-…-MASTER-PLAN.md` §Part 1 (INV-2, INV-7, INV-12), §Part 1B (M-03, M-04, M-06, M-07, M-30), §Part 3.2, §Part 5 (S2) |
| **Locks** | Hands **ADR-01-04**, **ADR-01-05**, **ADR-02-05**, **ADR-03-01**, **ADR-03-05** to `00 §Part 8` |
| **Status** | PLANNED |
| **Grounding** | `_grounding/G6` (all five parts, verified 2026-08-31) · `G2b` (62 findings, provider capability matrix) · `G4b §A.1` (the LiteLLM design being replaced) · `G1 §D.2` |

---

## What this is

The layer between the agent runtime and four vendor HTTP APIs: **Anthropic, OpenAI, Google (Gemini), xAI (Grok)**. It is also where the restructure's largest cost lever lives.

The current layer is not salvageable by patching. `G2b`'s capability matrix found three of four providers unusable for real agent work: OpenAI and Google **silently drop `max_tokens`**; Gemini **discards tool-call arguments** (`google.rs:313` `let _args = ...`) and the empty `{}` is laundered downstream into a valid-looking tool call; parallel tool calls take first-only on both; structured output exists nowhere; there is **no Grok provider at all**, and the factory's `match` falls through to a silent `return` from inside `tokio::spawn`, leaving a phantom running agent. `ModelInfo` carries only input and output prices, so **the type cannot represent cache pricing** — which is why ADR-01-04 recommends replacement over repair.

**The subtle failures this subsystem is prone to:**

1. **Caching that silently does nothing.** No provider errors when a prefix falls below its cache minimum. One volatile byte — a timestamp, a HashMap iteration order — defeats it completely, and the only evidence is a usage field nobody is watching.
2. **Losing opaque reasoning state.** It is invisible in the types, breaks turn continuity, and shows up as a bigger invoice rather than an error.
3. **Estimating what the provider already told you.** Every estimate is a number that will disagree with the bill.
4. **Flattening the divergence too early.** The four vendors differ in ways that matter; an abstraction that erases the difference erases the feature.

---

## 1. First principles

**Adapters own all vendor JSON.** No vendor type escapes into the runtime — the one deliberate exception being `Item::Opaque`, whose bytes the runtime carries without interpreting.

**Normalize to the OpenAI-shaped tool-call wire format.** This is the design the sidecar's LiteLLM layer already proved (`G4b §A.1`), and it is what three of the four vendors are closest to natively.

**Capabilities are keyed by model, not provider** (`G6 §5.1`). The four vendors vary per model — context window, cache minimum, reasoning support, output ceiling — so `capabilities(&ModelId)` is the only correct shape. A provider-level capability table is wrong on its first day.

**Keys are the user's** (INV-12). See §6.

---

## 2. The trait

```rust
#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn id(&self) -> ProviderId;
    fn capabilities(&self, model: &ModelId) -> &ModelCapabilities;

    async fn complete(&self, req: Request) -> Result<Response, ProviderError>;
    async fn stream(&self, req: Request)
        -> Result<BoxStream<'static, TurnEvent>, ProviderError>;

    async fn count_input_tokens(&self, req: &Request) -> Result<u32, ProviderError>;

    fn apply_cache_policy(&self, req: &mut Request, policy: &CachePolicy);
    async fn compact(&self, req: &Request) -> Result<CompactionArtifact, ProviderError>;

    fn read_rate_limit(&self, headers: &HeaderMap) -> Option<RateLimitState>;
    fn classify_error(&self, status: StatusCode, body: &[u8]) -> ErrorClass;
}
```

```rust
pub enum CachePolicy {
    Off,
    Automatic { affinity_key: String },        // xAI: x-grok-conv-id
    Explicit  { breakpoints: Vec<usize>, ttl: CacheTtl },   // Anthropic: ≤4
    Handle    { id: String },                  // server-side state
}

pub struct Usage {
    pub input: u32, pub cached_read: u32, pub cache_write: u32,
    pub output: u32, pub reasoning: u32, pub tool: u32,
}

pub enum ErrorClass { Retryable, Overloaded, QuotaExhausted, Deferred, Fatal }
```

**`CachePolicy` has four variants because the vendors genuinely differ**, not for generality's sake: Anthropic is explicit with ≤4 breakpoints and per-model minimums; **xAI is fully automatic** with no `cache_control` at all, steered by an `x-grok-conv-id` header at per-*message* prefix granularity; the others differ again. Collapsing this to a boolean loses the feature on at least two providers.

**`Usage` has six fields, not two.** Cache-read and cache-write are priced differently from input, and reasoning tokens are billed. A four-way split is the minimum that can produce a correct invoice.

---

## 3. The divergence table

What the abstraction must absorb (`G6 §5.2`), and how.

| Dimension | Divergence | Absorption |
|---|---|---|
| **Caching** | Anthropic explicit (≤4 breakpoints, minimums 512–4,096 **per model**, 5m at 1.25× / 1h at 2.0× write, 0.1× read); xAI fully automatic (75–85% off, **no documented TTL or minimum**, "not 100% guaranteed"); others differ | `CachePolicy` 4-variant + `apply_cache_policy` in the adapter |
| **Tool-call ids** | OpenAI has **two** ids, only `call_id` returns; Gemini renames `id`→`call_id`; Anthropic uses its own | `ToolCallId` newtype; adapters map both directions |
| **Parallel tool results** | **Anthropic requires all parallel `tool_result`s in one user message** or it silently stops calling in parallel | Adapter batches; the runtime always hands over the full set |
| **Truncation** | `max_tokens` / `MAX_TOKENS` / `length`; **xAI's finish-reason set is undocumented** and grok-4.6 advertises no output ceiling | `StopReason::MaxTokens`, mapped per adapter; unknown reasons surface as `Unknown(String)`, never silently as `EndTurn` |
| **Reasoning state** | 4 different names for unreplayable state | `Item::Opaque { provider, kind, bytes }` |
| **Rate limits** | Headers only on Anthropic (RFC 3339) and OpenAI (duration strings); **Google and xAI publish none** | `read_rate_limit -> Option<_>`; absence is a legitimate answer |
| **Errors** | Anthropic returns **529** for overload; **xAI returns 202 as a non-result success**; spend-cap 429s carry no `retry-after` and never succeed on retry | `ErrorClass`, with `QuotaExhausted` explicitly non-retryable |
| **Server state** | Gemini + xAI make it a cache-hit feature; **OpenAI bills prior tokens regardless**, so there is no saving there | `CachePolicy::Handle`, adopted only where it actually saves |
| **Price cliffs** | A **200k-token price cliff** at Google Pro and on every xAI model | Pricing is a function of token count, not a flat pair |
| **Gemini API fork** | Explicit caching and batch force you onto the legacy `generateContent` API | Adapter-internal; invisible above the trait |

---

## 4. Prompt caching — the largest lever

**Adopt first, everywhere** (ADR-01-05 row 1). Anthropic cache reads cost **0.1× base input** and do not count against ITPM; xAI gives 75–85% automatically.

### 4.1 The prefix contract

**One builder produces the cached prefix, and it is byte-stable.**

```
tools → system → AUTODEV.md → recalled memory        ← cached
────────────────────────────────────────────
live history                                          ← not cached
```

Requirements, each of which is a real failure mode:
- **Deterministic serialization.** `BTreeMap` / `preserve_order`. Anthropic requires a **100% byte match**; non-deterministic JSON key order defeats caching with no error (`G6 §5.5`).
- **No volatile content.** No timestamps, no "files changed since", no varying tool list. Today's system prompt is rebuilt per run and is **not byte-stable** (`G2a Q4`), so caching would not work even once it is wired.
- **Prefix-order stability.** Caching is a prefix match over `tools → system → messages`; a change at any level invalidates that level *and every level after it*.
- **1-hour entries precede 5-minute entries** when TTLs are mixed.

### 4.2 The alarm comes first

**Build the cache-hit alarm before the cache** (`00 §Part 9` R-2). The only honest check is the usage field.

⚠ **CORRECTED 2026-08-31 — the arithmetic below is Anthropic's, and it is not universal.**

```
Anthropic:            total_input = cache_read + cache_creation + input      ← input EXCLUDES cached
OpenAI / Gemini:      cached tokens are reported INSIDE the prompt total     ← do NOT add
```

Applying Anthropic's formula to OpenAI or Gemini **double-counts cached tokens, over-reports cost, and makes the cache-hit alarm's arithmetic wrong on three of the four providers**. Reported by the buildability review (`B-09`); this is exactly the divergence `Usage`'s six fields exist to absorb, and the adapter — not the runtime — owns the normalisation.

**The invariant the alarm actually needs**, stated provider-neutrally: *across repeated requests with an identical prefix, `cached_read` must be non-zero from the second request onward.* If it is zero, a silent invalidator is at work. The runtime logs a warning and the dashboard carries cache-hit rate as a first-class metric — otherwise caching can be "adopted", deliver nothing indefinitely, and show no symptom but the bill.

### 4.3 Per-model minimums

Below the minimum, **nothing caches and no error is returned**. The adapter's `capabilities(&ModelId)` carries the number, and the prefix builder warns when the assembled prefix falls short (Anthropic minimums range 512–4,096 tokens by model; xAI documents none).

---

## 5. Cost and usage

**Provider-reported only. No estimates.** (INV-7.)

Four current defects this replaces:
- **`message_start` is not handled in streaming**, so Anthropic `input_tokens` is always 0 — and with the OpenAI/Google equivalents, **input tokens are 0 for all three cloud providers in streaming** (`G2b P-30`).
- The price table's `_ => (1.0, 3.0)` fallback catches **7 of 11 selectable models**: Gemini 2.5 Flash billed **13× high**, a free local model billed at $1/1M, GPT-4.1 under-billed 5× (`C-02`).
- `cost_today_usd` is never reset — "today" means "since process start" (`G3c IN-50`).
- The daily rollup sleeps an hour before its first run and only aggregates *yesterday*, so a desktop app used in sub-hour sessions **never completes a single rollup** (`IN-56`).

**The rule: an unknown model is an explicit error, never a default price.** A wrong cost number is worse than no cost number, because it is believed.

Pricing is a **function**, not a pair — it takes model, token counts and the four-way split, and it must handle the 200k price cliffs.

---

## 6. BYOK — key lifecycle

**Decided direction (Razi, 2026-08-31): keys are the user's.** No key ships with AutoDev.

```
user types key in UI
   → invoke("set_provider_key", provider, key)     ← inbound only
   → Rust validates it against the provider (a cheap models/count call)
   → on success: OS keychain (keyring crate)
   → UI receives: { ok: true, masked_last4, models_available }
```

**Asymmetric by construction** (ADR-03-01): there is an inbound command and **no outbound one**. `get_provider_key` and `get_github_pat` are removed. The UI never needs to read a key back — only to know one is present and works.

**Validation on save, not on first use.** Otherwise the failure surfaces during a real turn, which is exactly where it is most confusing (spec `07`, M-35).

**Per provider, independently configurable.** The UI shows which of the four are live. The vault needs an `enumerate` it does not currently have — today keys for removed providers linger in the OS keychain forever with no way to find them (`G3c IN-49`).

**The development path** (ADR-03-05): a repo `.env` is read **only by the integration-test harness**, which drives the adapters directly and is not linked into the app. There is no runtime env fallback in any build configuration (INV-12). Today `airsctl` violates this — it takes a key as a CLI flag (leaking into shell history and the process table) and falls back to `ANTHROPIC_API_KEY` from the environment.

---

## 7. Runtime model switching

**Resolution is already per-request** (`agent_commands.rs:32-40`), so a switch takes effect on the next message and in-flight runs keep their model. That part is right.

Three defects to fix (M-07):
- **`set_active_provider` is in-memory only with no startup hydration** — the choice dies on restart, and the first message after a restart errors with "No active provider set".
- **`ModelConfigsDb` is a writer with no reader** — temperature, top_p, reasoning effort and thinking are persisted by the UI and every `CompletionRequest` hardcodes them to `None`.
- **`analyze_diff_tool.rs:219-222` bypasses selection entirely** via env vars set nowhere, falling back to a hardcoded `claude-sonnet-4-6` — an id in no provider catalogue. There are 18 hardcoded model strings in total (`G2a §3`).

---

## 8. Ollama removal

**Clean** (`G2b`): delete `ollama.rs` (506 lines), edit 10 sites across 8 files, drop the `ollama-rs` dependency. Zero references in `routing/`, `budget/`, `cost/`, `subagent/`, `session/` or `memory/`. It also removes the only stubbed tool-calling path and one class of wrong pricing. The 8 frontend references are **documentation copy only**, in 4 files.

---

## 9. Implementation shape

**Hand-rolled adapters on `reqwest` + `serde` + an SSE parser, behind the trait** (ADR-01-04), using `async-openai` tactically inside the OpenAI *and* xAI adapters — xAI is OpenAI-wire-compatible with a base-URL override, making it the easiest of the four to implement from Rust.

**No first-party Rust SDK exists for any of the four**, and **xAI ships no agent SDK in any language**. `G6 §5.4` finds no crate implements compaction, context editing, or per-model cache minimums; `genai` covers part of caching, `rig` has `CacheControl` for Anthropic only. All 14 cost-relevant features are plain HTTP.

---

## 10. Determinism balance

Entirely `DET` except the provider call itself, which is the runtime's single `ORGAN` (spec `01 §7`). Cost math is `MATH`.

---

## 11. Test gates

| Gate | Asserts | Catches |
|---|---|---|
| **G-CONFORM** | Per provider × model: streaming, tools, **parallel** tools, structured output, system prompt, vision (where claimed), reasoning, usage completeness, truncation detection, error classification | The `G2b` matrix — every ⚠️ and ❌ in it |
| **G-MAXTOK** | `max_tokens` is honoured and truncation is reported as `MaxTokens` | OpenAI and Google silently dropping it |
| **G-TOOLARGS** | Tool arguments survive the round trip byte-identically, including nested objects and unicode | `google.rs:313` discarding args; the laundered empty `{}` |
| **G-PARALLEL** | Two tools requested in one turn both execute and both results return in one message | First-only on OpenAI and Google; Anthropic's single-message requirement |
| **G-CACHE-HIT** | A 5-turn same-prefix session shows non-zero `cached_read` from turn 2 on every provider that supports it | The whole of §4 |
| **G-CACHE-STABLE** | The prefix builder is byte-identical across 100 invocations with identical inputs | Non-deterministic key order |
| **G-CACHE-MIN** | A prefix below the model's minimum produces a warning | Silent non-caching |
| **G-OPAQUE** | A reasoning-carrying turn replayed to the provider is accepted; opaque bytes survive persistence | Dropped signatures; xAI cache misses |
| **G-USAGE** | Every response yields all six `Usage` fields; streaming yields non-zero input tokens | `P-30` |
| **G-PRICE** | Known models price correctly incl. across the 200k cliff; **an unknown model errors** | The `_ => (1.0, 3.0)` fallback |
| **G-KEY-ASYM** | Static check: no command returns key material. Runtime: a saved key validates before storage | `IN-47` |
| **G-KEY-NOENV** | With the keychain empty and every provider env var set, the app **refuses to run a turn** | A dev `.env` becoming a runtime fallback (INV-12) |
| **G-MODEL-PERSIST** | Select a model, restart, send a message — the same model serves it | `set_active_provider` not persisted |
| **G-MODEL-CONFIG** | Temperature/top_p/reasoning set in the UI appear in the outbound request | `ModelConfigsDb` with no reader |
| **G-NO-OLLAMA** | No reference to Ollama remains in Rust or in shipped UI copy | Incomplete removal |
| **G-SPEND-CAP** | ⚠ **ADDED at rev 0.9 (G-04, M-42).** A session exceeding the daily ceiling is **stopped**, the fleet pauses, and the user is told. The kill switch halts all agent work immediately | No ceiling, no off switch; per-task budget inert |
| **G-KEY-LINUX** | ⚠ **ADDED at rev 0.9 (G-06).** On a Linux box with no D-Bus Secret Service, a key can still be stored and read, and **the UI states which store is in use** | Keychain-only BYOK having no Linux story — and CI having no credential path at all |

---

> No ADR decision is recorded in this spec. It hands **ADR-01-04**, **ADR-01-05**, **ADR-02-05**, **ADR-03-01** and **ADR-03-05** to `00 §Part 8`.
