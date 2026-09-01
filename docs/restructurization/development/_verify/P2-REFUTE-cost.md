# P2 pre-build refutation — cost, tokens, spend

**Read-only adversarial pass, 2026-09-02.** Dispatched under `§0.2m`. Verdicts are the
agent's; every `path:line` below was quoted verbatim from the tree.

⚠ **Dispatch defect, recorded rather than hidden.** This agent was given the incremental-write
clause required by `§0.2i` and then dispatched as a **read-only** agent type with no write
tool, so it *could not comply*. The report survived only because it fitted in the completion
notification. That is `L-C8` — a recorded lesson not applied — and the fix is that a subagent
required to write a file must be dispatched with a tool that can write one.

---

## Summary

| Claim | Verdict | Reason |
|---|---|---|
| 1 — input tokens 0; no initial-usage handling | **PARTIALLY CONFIRMED** | Outcome right and **worse**; mechanism wrong for 2 of 3 providers |
| 2 — silent price fallback over most models | **CONFIRMED** | Cite exact; **9 of 12** offered models fall through, including `DEFAULT_MODEL` |
| 3 — provider-blind, wrong in different directions | **OVERSTATED** | Provider-blind yes; different directions no — `cache_control` absent tree-wide |
| 4 — no daily ceiling, no kill switch | **CONFIRMED, understated** | No cap at *any* scope; the per-task breaker is unreachable |
| 5 — nothing handles the 200k price cliff | **CONFIRMED** | `model_pricing(&str) -> (f64, f64)` cannot express a threshold |
| 6 — `ModelConfigsDb` writer with no reader | **PARTIALLY CONFIRMED** | Read only by the form that wrote it; never reaches a request |

---

## Claim 1 — input tokens are zero

**The plan's mechanism is wrong for two of three providers, and the damage is larger than
claimed.** There is no single "streaming path" — three adapters have three parsers with three
independent bugs. A task scoped to one file leaves two providers broken.

**Anthropic — mechanism confirmed.** `message_start` appears **zero times in any `.rs` file**.
`streaming.rs:85-86` dispatches `_ => Ok(None)`, so it is swallowed. Usage can only arrive via
`message_delta`, read at `streaming.rs:77-78` with `.unwrap_or(0)` — a missing field yields 0
silently.

**OpenAI — mechanism refuted, outcome worse.** There is no usage message *to* handle: the
request never asks for one. `stream_options` / `include_usage` appear **zero times in the
tree**; the body at `openai.rs:199-201` sets only `stream`. So `usage` is never present and
`StreamChunk::MessageDelta` is **never constructed** — `tokens_out` is zero too, not just
input. A **second, independent** blocker: the usage frame arrives with `choices: []`, and
`openai.rs:342-344` returns `None` on empty choices before any usage read. Two edits required.

**Google — mechanism refuted, outcome confirmed.** `usageMetadata` arrives on the *final*
chunk and a handler exists at `google.rs:353-366`, but **both routes to it are blocked by
earlier returns**: `google.rs:321-324` returns `None` when a chunk has no `parts`, and
`google.rs:326-332` returns `TextDelta` on the first text part. The handler is reachable only
by a chunk with non-empty `parts` where no element has `text` or `functionCall`.

**Undetermined:** whether Anthropic's `message_delta` under `anthropic-version: 2023-06-01`
(`anthropic.rs:19`) carries `input_tokens`. **Settled by:** one raw SSE capture per provider,
or `SELECT prompt_tokens FROM token_usage` against the analytics DB.

---

## Claim 2 — the price fallback

`cost/tracker.rs:61` is exact, character for character: `_ => (1.0, 3.0), // Safe default`.

**9 of 12 offered models fall through**, measured against `src/data/models.ts`:

| Falls through | Matches an arm |
|---|---|
| `gemini-3.5-pro` · `gemini-3.5-flash` · `claude-fable-5` · `gpt-5.5` · `gpt-5.5-pro` · `gpt-5.4` · `gpt-5.4-mini` · `gpt-5.4-nano` · `gpt-5.3-codex` | `claude-opus-5` · `claude-sonnet-5` · `claude-haiku-4-5` |

**Every OpenAI model (6/6) and every Google model (2/2) is billed at an invented rate** — and
`DEFAULT_MODEL` is `gemini-3.5-flash`, so every worker created without an explicit choice is
priced at $1.00/$3.00 per 1M.

**The three "matches" are not safe either.** `contains()` is a substring test, so
`claude-opus-5` inherits Opus 4.5's `(15.0, 75.0)`. That is *confidently* wrong rather than
obviously wrong — it will never look suspicious.

**None of the six `gpt-*`/`gemini-*` arms corresponds to an offered model.** `gpt-4o`,
`gpt-4o-mini`, `gemini-1.5-pro` are all retired. Three of the table's arms are dead code.

---

## Claim 3 — normalisation (OVERSTATED)

**Confirmed half:** there is no normalisation layer. `types.rs:101-105` carries two untyped
`u32`s with no provenance; `streaming.rs:77`, `openai.rs:392-395` and `google.rs:356` collapse
three different source fields into `tokens_in`; `engine.rs:479-480` adds them blind.
`cache_creation_input_tokens`, `cache_read_input_tokens`, `prompt_tokens_details`,
`cachedContentTokenCount` and `thoughtsTokenCount` appear **zero times in the tree**.

**Refuted half:** `cache_control` appears **zero times in any `.rs` file**. The app never opts
into Anthropic caching, so there are no Anthropic cached tokens to exclude — the Anthropic
direction is *neutral*, not negative. OpenAI and Google would both err in the **same**
direction. "Wrong in different directions" does not describe what this code can produce.

🔴 **Sequencing constraint:** this defect is **masked by Claim 1**. A cached-token
misattribution inside a value that is always zero has no effect on the bill. It becomes live
only after Claim 1 lands.

**Missed by the claim:** Gemini thinking tokens are never counted. `google.rs:357` reads only
`candidatesTokenCount`; `thoughtsTokenCount` is never read, and `thinking_tokens` is hard-passed
as `None` at `engine.rs:398` and `recipes/runner.rs:470` although `collector.rs:27` accepts it.

---

## Claim 4 — no spend ceiling (CONFIRMED, understated)

🔴 **The largest finding of the pass. There is no enforced spend limit at any scope — not
per-task, not per-day, not per-fleet — and the per-task breaker that appears to exist is dead
code.**

`AgentEngine.task_budget` is `Option<TaskBudget>`, initialised `None` at `engine.rs:72` and set
only by `with_task_budget` (`engine.rs:84-86`) — which has **zero callers** across `.rs`, `.ts`
and `.tsx`. The only two hits in the repo are its own definition and a comment naming it. So
`task_budget` is permanently `None` and `engine.rs:164-180` is unreachable, along with
`escalate_budget_exceeded` (`engine.rs:503-563`), `TaskBudget::default_worker` (`budget/mod.rs:52-58`,
`max_usd: Some(0.50)`), `AutoDevError::BudgetExceeded`, and the `set_task_budget` MCP tool that
writes budgets to `tasks.db`. **~100 lines of enforcement machinery with no entry point.**

**No daily ceiling in Rust.** `DailyBudgetSummary` (`budget/mod.rs:130-138`) is six report
fields with no limit member and no `check()`. The only running total is an unbounded
accumulator with no guard: `app_state.rs:57-61`.

**The only "kill switch" is a frontend toast**, and it cannot see Mode 1 spend:
- `src/stores/dashboardStore.ts:366-376` pushes a `BudgetAlert` and calls nothing else.
- The threshold is a literal — `dashboardStore.ts:173` `dailyBudget: 50,`; `setDailyBudget`
  (`:320-322`) **has no caller**. `CostMeter.tsx:20` independently re-defaults to 50.
- It listens on `'cost:update'`, and `emit_cost_update` has **one** caller —
  `recipes/runner.rs:481`, the Mode 2 tick. Mode 1 emits `emit_agent_done` (`engine.rs:406`),
  so **the entire Mode 1 agent path is invisible to the one warning that exists**.
- Frontend-only: closing the window, or running `src-cli`, removes even the toast.

🔴 **Remediation order changes.** `P2-21` cannot be "add a daily cap" — a cap layered on a
breaker that cannot fire inherits the same dead-code failure. **Wire `with_task_budget` first.**

---

## Claim 5 — the 200k price cliff (CONFIRMED, structurally)

`cost/tracker.rs:53` is `fn model_pricing(model: &str) -> (f64, f64)` — it **takes no token
count**, so a threshold cannot be expressed, let alone selected. Its caller multiplies two flat
scalars (`tracker.rs:19-21`). `ModelInfo` (`provider/types.rs:5-14`) has one input and one
output rate and no tier fields.

**Related:** the single `200_000` in this area is unrelated to pricing — `engine.rs:188`
`let model_context = 200_000u32;` is a hardcoded compaction trigger, **wrong for the offered
1M-context Gemini models, which are compacted at 20% of their real window.**

---

## Claim 6 — `ModelConfigsDb` (PARTIALLY CONFIRMED)

**As literally worded it is false**: `ModelConfigsDb::get` *is* called, from
`model_config_commands.rs:13-14`, consumed at `modelConfigStore.ts:69` and rendered by
`Settings.tsx:1276-1284`. It round-trips to redisplay its own form.

**The substance is correct**: neither `ModelConfigsDb` nor `ModelConfig` appears in `agent/`,
`provider/` or `recipes/`. Every `CompletionRequest` hardcodes the fields —
`engine.rs:211-214` (`None`×4), `compaction.rs:117-120`, `classifier.rs:148-151`,
`recipe_commands.rs:266-269`. **The adapters would honour them** (`anthropic.rs:143-154`,
`openai.rs:189-198`, `google.rs:142-153`); the wire is cut at request construction.
**Setting a temperature in Settings changes nothing about any API call.**

**Additional:** `THINKING_MODELS` and `REASONING_EFFORT_MODELS` (`modelConfigStore.ts:20-35`)
both have an **empty intersection** with the 12 offered ids, so `Settings.tsx:1286-1287` gates
are permanently false. Three columns of `model_configs` are unwritable *and* unread.

---

## What the plan gets wrong

1. Claim 1's mechanism describes **Anthropic only**. Fixing "handle the initial usage message"
   fixes **zero of three** providers.
2. Claim 1 understates the blast radius — `tokens_out` is zero too on OpenAI and Google.
3. "The streaming path" is **three** parsers with three bugs.
4. Claim 3's "different directions" is unsupported, and the defect is masked until Claim 1 lands.
5. Claim 4 implies per-task enforcement works. It does not — remediation starts with wiring.
6. Claim 6 is falsifiable as worded; state it as "read only by the form that wrote it".
7. Claim 2's cite is the only one verified **exact**, with no drift.

---

## Unlisted findings

| # | Finding |
|---|---|
| **U1** | `engine.rs:237` `let _ = db.update_task_budget_usage(` — the only persistence of running budget consumption, silently discarded. Every other DB call in the same function logs its error; this one is the outlier, and it is the one that matters for spend. |
| **U2** | Dead guard — `with_task_budget` zero callers. See Claim 4. |
| **U3** | Dead parameter — `engine.rs:422` `_model: &str` in `stream_response`; per-model streaming cannot be implemented without a signature change, adjacent to where per-model config would apply. |
| **U4** | `sidecar/client.rs:1119` `let _ = cost_usd;` — the orchestrator's own authoritative cost frame, parsed at `:1080`, discarded for a locally recomputed value from the broken table. |
| **U5** | The comment justifying U4 (`client.rs:1117-1118`) names `crate::analytics::cost::compute_cost`, which **does not exist** — there is no `analytics/cost.rs` and no such function. |
| **U6** | 🔴 Unreachable match arm — `tracker.rs:58-59` tests `contains("gpt-4o")` **before** `contains("gpt-4o-mini")`, so a mini would bill at 16.7× its rate. Latent now, but **`gpt-5.4` and `gpt-5.4-mini` are both offered**: the trap is armed for the next arm added. |
| **U7** | `cost_today_usd` never reset (`app_state.rs:26,57-61`); surfaced as the dashboard's "today" via `queries.rs:72` and `analytics_commands.rs:40`. A week-long session reports a week as today. |
| **U8** | Mode 1 spend never reaches the budget alert — `emit_cost_update` has one caller, `recipes/runner.rs:481` (Mode 2). |
| **U9** | `dailyBudget` unchangeable — two hardcoded fifties, `setDailyBudget` with no caller. |
| **U10** | `.unwrap_or(0)` on all eight usage extractions. A provider schema change degrades to "this call was free" with no log line — and with U2, to unbounded silent spend. |
| **U11** | Provider inference by substring, duplicated at `engine.rs:379-387` and `runner.rs:441-452`; `o4-mini` matches none and is filed `"unknown"`, corrupting the per-provider breakdown at `aggregator.rs:62-64`. |
| **U12** | `runner.rs:516-517` passes `1, 0` as literals, so `collector.rs:130-134` always computes `success_rate = 100.0` from that path. |
| **U13** | `analytics/aggregator.rs:20-23` — the hourly rollup is a stub returning `Ok(())` on a schedule. Logs no error, looks healthy, produces nothing. |
| **U14** | `sidecar/client.rs:67-89` invents a 70/30 token split when only a total is reported; `:1109-1113` falls back to chars/4 with a 4× multiplier. Mode 2 cost is an estimate of an estimate priced with a fallback rate, shown to the user to four decimal places. |
