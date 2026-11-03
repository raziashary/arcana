# Groq provider addition — evidence receipt (2026-10-17)

Owner direction: Razi, 2026-10-17 — add Groq as the fourth provider, keyed on
`qwen/qwen3.8-27b`, key in `.env`; free provider verification authorized, no
paid completions authorized. Logged as `ADR-LOG-RESTRUCT-0395`.

## Verified wire facts

| Fact | Value | Source |
|---|---|---|
| Wire model id | `qwen/qwen3.8-27b` | console.groq.com/docs/model/qwen/qwen3.8-27b |
| Predecessor | `qwen/qwen3.6-27b` deprecated 2026-09-14, replaced by this id | console.groq.com/docs/deprecations |
| Base URL | `https://api.groq.com/openai/v1` (OpenAI-compatible) | console.groq.com/docs/openai |
| Context window | 131,072 | console.groq.com/docs/models |
| Max completion tokens | 16,384 | console.groq.com/docs/models |
| Price | $0.80 input / $4.00 output per 1M (published 2026-08-27) | console.groq.com/docs/models |
| Tool use / JSON mode | Documented | console.groq.com/docs/vision (model page) |
| `reasoning_effort` | `none`/`low`/`medium`/`high` accepted for this model; `high` → native `xhigh` | console.groq.com/docs/api-reference |
| `stream_options.include_usage` | Documented; usage frame may carry usage under `x_groq.usage` | console.groq.com/docs/api-reference + Groq SSE AsyncAPI spec |

## Live verification (free endpoints only — no paid run)

- `GET /openai/v1/models` with `GROQ_API_KEY` from repo `.env` (not exported to
  the session — §0.8/house rule): key accepted, `qwen/qwen3.8-27b` present in
  the vendor's served list.
- Run command: `cargo test -p arcana-core --features live-smoke --test
  live_provider_smoke -- p4_groq --nocapture`
- Result: _pending run_ (fillable below after the suite runs).

| Check | Result |
|---|---|
| `test_connection` → Connected | ✅ 2026-10-17 (`p4_groq…` green, 0.60s) |
| Catalogued id in vendor `/models` | ✅ `qwen/qwen3.8-27b` present in the served list |

Housekeeping: the owner's `GROQ_API_KEY` line in `.env` carried a space before
`=` (`GROQ_API_KEY = …`), which the repo's own `key_from_dotenv` parser rejects;
normalized to `NAME=value` to match the `GOOGLE_API_KEY` line's shape. `.env`
stays git-ignored and unstaged (verified `git check-ignore` before touching it).

## Static gates (fixture suite, no credentials)

- `groq_tests.rs`: body shape (no `prompt_cache_key`, `max_completion_tokens`,
  `reasoning_effort` passthrough, `stream_options`), SSE fixtures (text delta,
  Qwen `reasoning` spelling, tool fragments, `[DONE]`, usage under
  `x_groq.usage` AND top-level `usage`, shared-subtraction inclusive rule),
  unary response with reasoning/tool call, catalogue default model.
- `conformance_tests.rs`: Groq joins the output-ceiling conformance row.
- `factory.rs`: four-provider set + unknown-refusal + id-echo tests.
- `routing.rs`: `qwen/` routes to groq; bare `qwen3-max` still refuses (slash
  distinction pinned by test).
- `catalogue.rs` / `cost/pricing.rs`: entry equality + published-rate test +
  priced-or-`UNPRICED` invariant.
- `ci_gate.rs::tg_p2_05`: Rust catalogue ↔ `src/data/models.ts` equality is
  exercised by the standing gate without modification (id shapes conform).

## Deliberately NOT claimed

- **`parallel_tools: false`** until parallel-call coordination is measured.
- **`CacheSupport::None`** until Groq documents a cached-token rate; cached
  reads are priced at full input until then (deficit-safe direction).
- **No paid completions were run** (no second paid provider run is authorized
  under the standing P4 checkpoint).
