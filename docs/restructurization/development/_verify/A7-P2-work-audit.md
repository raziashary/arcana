# A7 · P2 work audit — claim-to-code correspondence

**Auditor:** independent post-phase agent, no sight of the implementer's reasoning.
**Scope:** all 74 P2 tasks in `MASTER_DEVELOPMENT_PLAN.md` `§8`, plus the S2 smoke claims
where they carry the evidence for a task.
**Method:** every `- [x]` is treated as a claim to be refuted against the code. A fix that
exists but cannot be reached (dead code, a lookup that never hits, a type nothing
constructs) is recorded as **NOT-DONE**, per the audit mandate.
**Constraints honoured:** read-only on source; no `live-smoke` run; no keychain or
`~/.autodev/` access; no API-key environment variable set.

**Commits under audit** (`git log --oneline -12`):

```
fe2a6db docs(brief): P2 closed — readiness 8.0% to 22%, and what the live test found
414e102 docs(P2): closeout — matrix, gates register, and the six lessons P2 paid for
1f1d6f3 fix(budget): S2-06 — the daily cap reset to zero on every app launch
dfd145d fix(google): Gemini tool calls were broken against the live API — thoughtSignature
7d63db8 fix(models): P2-31 — four places named models that do not exist, one of them always
07727be feat(settings): P2-26/27/32/50 — settings that are written are now also read
8ec4003 feat(vault): P2-23/24/33/70/71/72/75 — BYOK, and stop claiming protections we do not implement
7257629 feat(provider): P2-67/68 — error classification, and the two missing SSE suites
9316e22 feat(provider): P2-16/28/65/69 — the live smoke, and the four defects it found
7a18955 feat(cache): P2-12..15 — the alarm first, then one prefix builder, then the wire
77a6382 feat(provider): P2-03/09/10 — one model catalogue, capabilities by model, normalised stop reasons
a0b426a docs(§0.16): record L-14 — never chain an action after a check with &&
```

---

## Tally

18 findings. Full per-task verdict tables at the foot of the file.

| Verdict | Count of the 74 tasks |
|---|---|
| CONFIRMED-DONE | 54 |
| PARTIAL | 17 |
| NOT-DONE | 2 — **P2-14**, **P2-23** (Google path) |
| OVERSTATED-EVIDENCE | 1 — **P2-32** (plus S2-04, S2-05, S2-07) |
| N/A, verified struck | 1 — P2-07 |

**Headline.** 20 of the 74 tasks are not what the plan says they are, and 3 of the 12 S2 rows
report evidence that does not match their own criterion. The two hardest failures are both the
class this phase has already paid for twice — **a fix that exists and is never reached**:
`P2-14`'s stable/volatile prompt split is rejoined by its only caller before the engine sees it
(F-12), and the daily spend ceiling `P2-21`/`P2-22` built correctly has no user-facing control
that can arm it (F-15). A third, F-8, is a BYOK hole the gate cited for it cannot see.

**Index of findings**

| # | One line | Task(s) | Verdict |
|---|---|---|---|
| F-1 | The app's `DEFAULT_MODEL` for new workers is a retired id | P2-31 | PARTIAL |
| F-2 | `TG-P2-05` is cited as a build-failing gate and does not exist | P2-03/31 | OVERSTATED-EVIDENCE |
| F-3 | Google key validation calls a retired model and can refuse a valid key | P2-23, P2-31 | NOT-DONE |
| F-4 | P2-36's id-collision fix landed on the streaming path only | P2-36 | PARTIAL |
| F-5 | `ErrorClass` is computed then flattened back into a string | P2-68 | PARTIAL |
| F-6 | P2-52's zero-usage alarm cannot fire on the commonest turn shape | P2-52 | PARTIAL |
| F-7 | OpenAI's SSE parser still returns one chunk per frame — the P2-35 shape | P2-67/68 | PARTIAL |
| F-8 | The shipped binary loads the dev `.env`; keys are ambient in every subprocess | P2-32, S2-04/07 | OVERSTATED-EVIDENCE |
| F-9 | `TG-P2-04`'s detector is narrower than its claim in three ways | P2-21 | gate |
| F-10 | `airsctl` does not share the provider factory | S2-07 | OVERSTATED-EVIDENCE |
| F-11 | An unpriced model still spends invisibly against the daily ceiling | P2-18/21 | PARTIAL |
| F-12 | 🔴 P2-14 unreachable — volatile memory still inside the cached region | P2-14 | **NOT-DONE** |
| F-13 | P2-64's single-use-prefix alarm cannot fire at its only call site | P2-64 | PARTIAL |
| F-14 | Vault enumeration and backend disclosure stop at the IPC boundary | P2-33, P2-71, P2-24 | PARTIAL |
| F-15 | 🔴 The daily cap cannot be set or enabled from the UI | P2-50, P2-22 | PARTIAL |
| F-16 | `max_output` was added to clamp `max_tokens` and nothing reads it | P2-08/45 | PARTIAL |
| F-17 | P2-30 closed the sleep half and left the "yesterday" half of its own statement | P2-30 | PARTIAL |
| F-18 | Residual Ollama references; no gate implements `TG2-14` | P2-11/73/74/75 | CONFIRMED-DONE |

---

# Findings

## F-1 · "No shipped code names a model that does not exist" — the app's DEFAULT model is a retired id
**Task:** P2-31 (also touches P2-18, S2-01)
**Verdict:** PARTIAL

**What the plan claims:**
> **P2-31** **18 hardcoded model strings** … → `GATE-PROVEN` *(`TG-P2-09`, falsified. Four live defects …)*

and the gate's own docstring: *"Every model id written into shipped code must be one the app offers."*

**What the code does:**
- `src/data/models.ts:77` — `export const DEFAULT_MODEL = 'gemini-3.5-flash'`
- `src/components/workers/WorkerCreateModal.tsx:24` — `const [model, setModel] = useState(DEFAULT_MODEL)`; the value is submitted at `:53` into `CreateWorkerInput`.
- `gemini-3.5-flash` is **not** in `MODEL_GROUPS` (`src/data/models.ts`, Google group is `['gemini-3.1-pro-preview', 'gemini-3.7-flash']`), so it is not in `ALL_KNOWN_MODELS` and is not an `<option>` in the picker at `WorkerCreateModal.tsx:107`.
- `autodev-core/src/provider/catalogue.rs:364` lists `gemini-3.5-flash` in `the_retired_generation_is_gone` with the assertion *"{dead} is retired and must not be offered"*.
- `autodev-core/src/db/workers.rs:302` ships a migration `UPDATE workers SET model = 'gemini-3.7-flash' WHERE model = 'gemini-3.5-flash';` — the tree already knows the id is dead.
- `src/data/models.ts` also documents `providerForModel` as returning `undefined` for an unknown id; `DEFAULT_MODEL` is exactly such an id.

**Why the gate missed it.** `TG-P2-09` (`src-tauri/src/ci_gate.rs:1186`) walks only `autodev-core/src`, `src-tauri/src`, `src-cli/src` and skips anything not ending `.rs` — **TypeScript is outside its reach entirely**. `TG-P2-02` (`ci_gate.rs:1077`) does read `models.ts`, but slices `&rest[..to]` from `MODEL_GROUPS` to `] as const`; `DEFAULT_MODEL` sits at line 77, *after* that slice, so it is never scanned. The two gates between them leave a hole exactly the shape of this constant.

**Why it matters:** a user who opens "New Worker" and does not touch the model dropdown creates a worker pinned to a model no provider will answer. The React `<select>` is controlled with a `value` that matches no `<option>`, so the UI gives no signal that the state is invalid. Under `P2-18`'s no-silent-fallback rule the model is also unpriceable, so the failure surfaces as a pricing error or a 404 rather than as "you picked a retired model". This is the same defect class `P2-31` was written to close, still live, in the single most-travelled creation path.

---

## F-2 · `TG-P2-05` is cited as a build-failing gate and does not exist
**Task:** P2-03 / P2-31 (evidence infrastructure)
**Verdict:** OVERSTATED-EVIDENCE

**What the plan claims:** the catalogue is the single model table; `TESTING_GATES.md:43` records P2 gates as *"`TG2-01…17` (implemented as `TG-P2-01…09`)"*.

**What the code does:**
- `autodev-core/src/provider/catalogue.rs:59` — *"⚠ **This list must equal `MODEL_GROUPS` in `src/data/models.ts` exactly** — `TG-P2-05` fails the build if it does not."*
- `autodev-core/src/provider/catalogue.rs:29` — *"`TG-P2-05` holds it against `models.ts` so the two lists cannot drift apart again."*
- `grep -rn "TG-P2-05"` over the whole repo returns **exactly those two comment lines**. `src-tauri/src/ci_gate.rs` implements `TG-P2-01, -02, -03, -04, -06, -07, -08, -09` — **there is no `TG-P2-05`**.

**Why it matters:** the one invariant the catalogue module was created to protect ("these two lists cannot drift apart again") is asserted by nothing. It has already drifted — see F-1, where `models.ts` names a model the catalogue calls retired. A docstring promising a gate that does not exist is worse than no docstring: the next reader will not re-derive the check, because the comment says it is already there.

---
## F-3 · BYOK key validation for Google calls a retired model, and can reject a valid key
**Task:** P2-23 (validation on the save path), P2-31 (no dead model ids)
**Verdict:** NOT-DONE (as a *working* validation path for Google) / gate escape for P2-31

**What the plan claims:**
> **P2-23** Key entry → **live validation** (models-list or token-count call) → keychain. → `SMOKE-PROVEN` *(Validation moved onto the SAVE path: `set_provider_key` builds the provider and calls `test_connection` before storing, and refuses to store a key the vendor rejects.)*

**What the code does:**
- `autodev-core/src/provider/google.rs:317-320` — `test_connection` posts to
  `"{}/v1beta/models/gemini-2.5-flash:generateContent"`.
- `autodev-core/src/provider/catalogue.rs:357` lists `gemini-2.5-flash` among the ids in
  `the_retired_generation_is_gone`, asserting *"{dead} is retired and must not be offered"*.
- `src-tauri/src/commands/vault_commands.rs:149-155` — a non-`Connected` status is a hard
  refusal: *"That key was not accepted by {provider}, so it has NOT been saved"*. A 404 on a
  retired model id lands in `Ok(r) => Ok(ProviderStatus::Error(format!("HTTP {code}")))`
  (`google.rs:341-344`), i.e. **`Error`, not the `Err(e)` network-failure branch** — so it is
  refused, not stored-with-a-warning.
- By contrast `anthropic.rs:312` and `openai.rs:369` both validate against the free
  `/models` list endpoint and name no model at all. Google has the same endpoint
  (`GET /v1beta/models`); the adapter uses a **billed `generateContent`** call instead, which
  the task text explicitly did not ask for (*"models-list or token-count call"*).

**Why `TG-P2-09` did not catch it.** The gate (`src-tauri/src/ci_gate.rs:1272-1280`) splits each
line on `"` and tests `token.starts_with("gemini-")`. Here the quoted token is the whole format
string `{}/v1beta/models/gemini-2.5-flash:generateContent`, which starts with `{`. **A dead model
id embedded in a URL template is invisible to this gate** — and every adapter builds its URLs
exactly this way, so the blind spot is systematic, not incidental.

**Why the smoke evidence does not cover it.** `autodev-core/tests/live_provider_smoke.rs`
contains 12 `#[tokio::test]`s and **none of them calls `test_connection`**. The one path the
`SMOKE-PROVEN` annotation names is the one path the smoke never executes.

**Why it matters:** this is the first thing a new user does. If Google no longer serves
`gemini-2.5-flash` — and the project's own registry says the id is retired, and the live smoke
already caught the same class of 404 on `gemini-3.1-pro` — then a perfectly valid Google API
key is refused with "That key was not accepted by google", on the provider the master plan
names as primary for rollout 1. The narrative success criterion for P2 ("a newcomer can enter a
key for any of the three providers, watch it validate…") fails at step one.

---

## F-4 · P2-36's id-collision fix landed on the streaming path only; the unary path still collides
**Task:** P2-36
**Verdict:** PARTIAL

**What the plan claims:**
> **P2-36** `google.rs:347` hardcodes `block_index: 0` … and `:345` builds `id: format!("gemini-{}", name)` (collides when one tool is called twice). → `UNIT-PROVEN`
> ✅ **2026-09-02** — Block index from the part position, id includes it — two calls to one tool no longer collide.

**What the code does:**
- Streaming path — **fixed**: `autodev-core/src/provider/google.rs:429` `let block_index = idx as u64;` and `:448` `id: format!("gemini-{name}-{block_index}")`.
- Unary path — **unchanged**: `autodev-core/src/provider/google.rs:527` still builds
  `id: format!("gemini-{}", name)` inside `parse_completion_response`, with no index of any
  kind. Two `functionCall` parts for the same tool in one non-streaming response produce two
  `MessageContent::ToolUse` values carrying **the same `id`**.

**Why it matters:** the blast radius is smaller than the streaming path (the agent loop uses
`stream()`), but `complete()` is live — `agent/compaction.rs`, `diff_analysis/classifier.rs`,
`src-tauri/src/commands/recipe_commands.rs` and the live smoke all call it. Any consumer that
correlates a `ToolResult.tool_use_id` back to a `ToolUse.id` on that path sees one of two
parallel calls. The annotation reads as though the defect is closed everywhere; it is closed in
one of the two places the defect exists, and the plan's own line numbers (`:345`, `:347`) point
only at the streaming half, so the second half was never in scope and never flagged.

---

## F-5 · `ErrorClass` is computed and then flattened back into a string; no typed consumer exists
**Task:** P2-68 (and the `ErrorClass` half of P2-67)
**Verdict:** PARTIAL

**What the plan claims:**
> `ErrorClass` had **no constructor** until now: defined, unit-tested against itself, produced by nothing, so a spend-cap 429 was the same string as a rate-limit 429. `classify()` reads the body, `TG-P2-06` holds every HTTP error path to it, falsified.

**What the code does:**
- `classify()` is genuinely called on all six HTTP error paths: `anthropic.rs:230`, `:265`;
  `openai.rs:290`, `:324`; `google.rs:235`, `:275`. `TG-P2-06`
  (`src-tauri/src/ci_gate.rs:1504`) counts `if !status.is_success()` blocks against
  `ErrorClass::classify` occurrences per adapter, so the gate is real.
- **But every one of the six sites does the same thing with the value:** interpolates it into
  the message — e.g. `google.rs:240` `AutoDevError::Provider(format!("Google {status} [{class:?}]: {text}"))`.
  `AutoDevError::Provider` carries a `String`. The class does not survive as a value.
- `ErrorClass::is_retryable()` (`usage.rs:229`) has **zero callers** outside `usage.rs`'s own
  test module. `grep -rn "is_retryable\|ErrorClass" autodev-core/src src-tauri/src src-cli/src`
  returns only the six `classify` call sites and the gate's own text.

**Why it matters:** the stated defect was *"a spend-cap 429 was the same string as a rate-limit
429."* After the fix it is a **different string** — `[QuotaExhausted]` versus `[Retryable]` —
which a future retry loop must recover by substring-matching a `Debug` rendering. That is the
same shape of defect one notch further along, and it is the shape `P2-46` was written to
punish elsewhere ("written by four sites and read by none"). The module docstring is honest
that no retry loop exists yet; the plan line is not, because `GATE-PROVEN` here proves the
*producer* runs, not that any consumer can act on it. A struct field or a dedicated
`AutoDevError` variant would have made it load-bearing; a format string does not.

---
## F-6 · P2-52's zero-usage alarm cannot fire on the most common turn shape
**Task:** P2-52
**Verdict:** PARTIAL

**What the plan claims:**
> **P2-52** `.unwrap_or(0)` guards all eight usage extractions. A provider schema change degrades to "this call was free" with no log line — and with `P2-48` unfixed, to unbounded silent spend. → `UNIT-PROVEN`

and `TESTING_GATES.md:179` (`TG2-09`, marked **GREEN**): *"A turn that returns content and reports zero tokens is an **explicit error**, not a $0 turn (P2-52)."*

**What the code does:**
- The eight `.unwrap_or(0)` sites are **all still there** — `streaming.rs:51-53`, `:119-123`;
  `google.rs:468-472`, `:499-504`; `openai.rs:431-438`, `:511-518`, `:541-549`. That is
  deliberate and defensible (the module note says so), so the invariant was moved up to the
  engine instead.
- The engine guard is `autodev-core/src/agent/engine.rs:642-651`:
  ```rust
  if total_usage.total() == 0 && turn > 0 {
      error!("Provider '{}' returned content but reported ZERO tokens …");
  }
  ```
- **`turn` counts tool-executing iterations, not turns.** `let mut turn = 0;` at `engine.rs:252`;
  the only `turn += 1` is at `engine.rs:615`, at the **end** of the tool-execution branch. The
  no-tool exit at `engine.rs:526-530` (`if tool_calls.is_empty() { break; }`) leaves the loop
  **before** that increment.

**Why it matters:** a session where the model answers once and calls no tool — the single most
common completed turn, and the exact shape of `s2_01/02/03` in the live smoke — exits with
`turn == 0`, so `turn > 0` is false and the alarm is suppressed. The guard fires only when the
model made at least one tool call *and* every usage field across every frame was missing. The
comment above it states the intended condition correctly ("a turn which produced output
reported nothing"); the code expresses a different one. `total_usage.total() == 0 && !history.is_empty()`,
or a loop-iteration counter distinct from the tool-turn counter, would express what the comment
says.

Two secondary notes on the same task:
- The consequence is an `error!` log line only — no `Err`, no event, no surfacing. `TG2-09`'s
  register text calls it an *"explicit error"*, which overstates a log call.
- Nothing tests this guard. `anthropic_tests.rs:190` tests the adapter's tolerance of a
  usage-less frame and explicitly defers the invariant to "the engine level"; the live smoke's
  `assert!(r.usage.total() > 0)` (`live_provider_smoke.rs:642`) asserts against a provider
  response, not against the engine guard. So the one condition that is wrong is the one
  condition nothing exercises.

---

## F-7 · The OpenAI SSE parser still returns at most one chunk per frame — the defect P2-35 fixed on Google
**Task:** P2-35 / P2-67 / P2-68 (and `TG2-03`, `TG2-04`)
**Verdict:** PARTIAL

**What the plan claims:**
> **P2-35** Google streaming returns on the **first** matching part … so parallel `functionCall` parts are dropped. → `GATE-PROVEN` ✅ The parser returns `Vec<StreamChunk>` and walks every part …

`TESTING_GATES.md:163` marks `TG2-04` **GREEN** on the strength of parallel-tool assertions across all three adapters.

**What the code does:**
- Google was genuinely converted: `parse_google_chunk` returns `Result<Vec<StreamChunk>>`
  (`google.rs:395`) and walks every part.
- **OpenAI was not.** `parse_openai_chunk` still returns `Result<Option<StreamChunk>>`
  (`openai.rs:419`). Inside `for tc in tool_calls { … }` (`openai.rs:465-496`) both branches
  `return` on the **first** array element, so a delta frame whose `tool_calls` array carries
  more than one entry loses every entry after the first.
- Two adjacent instances of the same shape: `openai.rs:453` returns on `delta.content`, so a
  frame carrying both content and `tool_calls` drops the tool calls; and `openai.rs:473` returns
  the `ToolUseStart` before reaching the arguments branch at `:485`, so a frame carrying `id`,
  `name` **and** non-empty `arguments` together loses the arguments.

**Why the fixture suite cannot expose it.** `openai_tests.rs:120 parallel_tool_calls_keep_separate_indices`
sends the two calls as **two separate SSE frames**, each with a one-element `tool_calls` array,
and asserts only that the indices differ. The multi-element-array shape — the only shape the
`Option` return cannot represent — appears in no fixture. The suite is shaped around what the
parser can do.

**Why it matters:** this is the same structural defect the phase spent `P2-35` fixing on Google,
left standing on OpenAI, under a gate marked GREEN. It is latent rather than certain (OpenAI
commonly emits one entry per frame), but "the vendor usually does not do that" is the position
`P2-41` and `P2-51` were both written to reject. If it does fire, a parallel tool call is
silently dropped and the loop proceeds with a partial batch.

---
## F-8 · The shipped desktop binary loads the dev `.env` — provider keys are ambient in the app and every subprocess it spawns
**Task:** P2-32, S2-04, S2-07 (and `TG-P2-07` / `TG2-11` / `INV-12`)
**Verdict:** OVERSTATED-EVIDENCE — the gate's literal assertion holds; the claim built on top of it does not

**What the plan claims:**
> **P2-32** … → `GATE-PROVEN` *(… **The dev `.env` is outside this, not an exception to it**: the live smoke reads that file directly, never the environment, behind a cargo feature.)*
>
> **S2-04** With the keychain empty and every provider env var set, the app **refuses to run a turn**. → **PASS, by construction.**

**What the code does:**
- `src-tauri/src/main.rs:27` — the **first statement of the shipped `main()`** is
  `let dotenv_loaded = dotenvy::dotenv().ok();`, unconditional, not behind any feature or
  `debug_assertions`. `dotenvy::dotenv()` walks up from CWD and loads the **entire** file into
  the process environment.
- The repo `.env` (present, gitignored) declares — key names only, values not read —
  `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GOOGLE_API_KEY`, alongside `AWS_SECRET_ACCESS_KEY`,
  `AWS_ACCESS_KEY_ID`, `MCP_VAULT_KEY`, `TAVILY_API_KEY`, `REGISTRATION_ADMIN_KEY` and eight
  further vendor keys. It is the same file `autodev-core/tests/live_provider_smoke.rs:53
  key_from_dotenv` reads.
- The tree already knows this happens: `src-tauri/src/sidecar/mod.rs:86-93
  strip_inherited_orch_env` exists to remove vars *"inherited … from the parent dotenvy load"*.
  Its strip list (`ORCH_ONLY_ENV_VARS`, `:70-85`) contains `AIRS_API_KEY`, `DATABASE_URL`,
  `REDIS_URL`, `MEM0_API_KEY` — and **none of the three provider keys**.
- Subprocesses inherit that environment. `std::process::Command` inherits by default and there
  is no `env_clear` anywhere in the tree (`grep -rn "env_clear"` → zero hits). The spawners
  include `autodev-core/src/agent/builtin_tools/bash.rs:98,102` — **the agent's own `bash`
  tool, driven by the model** — plus `hooks/executor.rs:60`, `plugin/runner.rs:54,61` and
  `builtin_tools/validate.rs`.

**Why `TG-P2-07` cannot see it.** The gate (`ci_gate.rs:1440-1447`) requires
`reads_env && names_a_provider_key` **on the same source line**. `main.rs` reads the whole file
and names no key, so it is invisible. The same same-line requirement means
`env::var(format!("{}_API_KEY", id))` or a two-line `let name = "ANTHROPIC_API_KEY"; env::var(name)`
would also pass. The gate proves *"no line both calls `env::var` and spells a provider key"*,
which is narrower than the invariant it is cited for.

**Why it matters:**
1. `P2-32`'s sentence *"The dev `.env` is outside this, not an exception to it"* is false of the
   shipped binary. The smoke's discipline is real; `main()`'s is the opposite of it.
2. `INV-12` ("credential provenance is BYOK, single-sourced") does not hold on any machine with
   a `.env` on the CWD walk-up path. The keys are not *used* by a provider — no code reads them
   by name, which is why `S2-04`'s conclusion survives — but they are **present in the process
   and inherited by every child process**, including a shell the language model itself chooses
   the contents of. "No reader exists today" is the same protection `P2-46` and `P2-48` were
   written to reject as insufficient.
3. Blast radius is developer machines and any deployment where a `.env` sits above the CWD,
   not a clean end-user install. That bounds it; it does not close it. The one-line fix shape
   already exists next door in `strip_inherited_orch_env`.

---

## F-9 · `TG-P2-04`'s detector misses any file that constructs a single provider, and skips `src-cli`
**Task:** P2-21 (the ceiling in the one place every call passes)
**Verdict:** CONFIRMED-DONE for the code; OVERSTATED-EVIDENCE for the gate

**What the plan claims:**
> **P2-21** **M-42 — daily spend ceiling in the provider layer**, the one place every call passes. → `GATE-PROVEN` … `TG-P2-04`, **falsified**.
> and the gate's own text: *"A hole in a spend cap is not a partial cap; it is the path spend will take. This asserts there are none."*

**What the code does — the good half.** The decorator is real and correctly placed.
`autodev-core/src/provider/capped.rs:119-127` gates `complete` and `stream`;
`autodev-core/src/provider/factory.rs:48` wraps on construction. Every direct construction site
in the tree is wrapped: `recipes/runner.rs:754`, `subagent/native_runner.rs:271`,
`commands/agent_commands.rs:146`, `commands/diff_commands.rs:43`,
`commands/recipe_commands.rs:232`, with `settings_commands.rs` exempted by name and with a
stated reason. I found **no unwrapped construction site**. `P2-21` itself is done.

**What the code does — the gate.** `src-tauri/src/ci_gate.rs:932-933`:
```rust
let builds = production.contains("AnthropicProvider::new(")
    && production.contains("GoogleProvider::new(");
```
- **`&&`, not `||`.** A file that constructs only one provider — say a future Anthropic-only
  helper, or any single-vendor path — is not recognised as a construction site at all and is
  skipped silently. The check is only as wide as "a file with a full three-arm match".
- **Scan roots are `["autodev-core/src", "src-tauri/src"]`** (`ci_gate.rs:885`). `src-cli/src`
  is absent, although `TG-P2-07` and `TG-P2-09` both include it.
- `tg_p2_04_the_desktop_binary_publishes_its_state` checks **only** `src-tauri/src/main.rs`.
  `wrap_global` returns the provider *unwrapped* when no state is published
  (`capped.rs:55-60`), so any other binary is uncapped by default and nothing says so.
- There is no floor assertion in this gate — unlike `TG-P2-07` (`scanned > 100`) and
  `TG-P2-09` (`scanned > 100`, `offered.len() >= 8`). A scan that matches nothing passes.

**Why it matters:** the code is right today, so nothing is currently leaking. But the gate is
what is supposed to keep it right, and its detector is narrower than its stated claim in three
independent ways. The next single-provider construction site added anywhere, or any provider
work in `src-cli`, passes this gate without being wrapped.

---

## F-10 · `airsctl` does not share the provider factory; S2-07 passes for a different reason than the one recorded
**Task:** S2-07 (evidence for P2-32)
**Verdict:** OVERSTATED-EVIDENCE

**What the plan claims:**
> **S2-07** `airsctl` refuses to run a turn from an environment variable with the keychain empty. → **PASS.** Same construction as S2-04 — **`airsctl` shares the provider factory**, and no path reads a key from the environment.

**What the code does:** `src-cli` constructs **no provider at all**.
`grep -rn "Provider::new\|factory::build\|CappedProvider" src-cli/src` returns nothing.
`src-cli/src/commands/agent.rs:136-165` builds an HTTP client and POSTs to
`{orchestrator_url}/api/agents/invoke` — the whole turn is delegated to
`submodules/core-orchestrator`, a Python service outside the Rust workspace
(`Cargo.toml` members are `autodev-core`, `src-tauri`, `src-cli`, `src-shared`).

**Why it matters:** the outcome recorded is right — `airsctl` cannot run a turn off an
environment variable, because it cannot run a turn at all without the orchestrator. But the
*mechanism* recorded is wrong, and it points the reader at a shared factory that `src-cli`
never touches. The real question S2-07 should be asking — whether the orchestrator this command
delegates to honours BYOK — is not asked anywhere, and that component is outside every P2
gate's scan roots. A closeout note that says "shares the provider factory" will stop the next
person from asking it.

---
## F-11 · An unpriced model still spends invisibly against the daily ceiling — the exact hazard the price table documents
**Task:** P2-18 / P2-21 (interaction)
**Verdict:** PARTIAL

**What the plan claims:**
> **P2-18** Per-model price table with **no silent fallback** — an unknown model is an explicit error. → `UNIT-PROVEN`

and the module's own reasoning at `autodev-core/src/cost/pricing.rs:90-93`, given as the reason
two models were withdrawn from the picker:
> *"an unpriced model bills at $0 — invisible to the daily cap, so it runs uncapped. Removed from the catalogue and the picker rather than shipped unmeterable."*

**What the code does:**
- `CostTracker::calculate` correctly returns `Result` and errors on an unknown model
  (`autodev-core/src/cost/tracker.rs:29-35`, `cost/pricing.rs:251`). That half is done.
- `autodev-core/src/agent/engine.rs:666-675`:
  ```rust
  let cost_usd = match CostTracker::calculate(model, &total_usage) {
      Ok(c) => c.cost_usd,
      Err(e) => { error!("Cost unavailable for model '{model}': {e}"); 0.0 }
  };
  { let mut state = self.state.lock().unwrap();
    state.add_cost(cost_usd, total_usage.total_input(), total_usage.output); }
  ```
- `autodev-core/src/recipes/runner.rs:443-458` does the same thing on the Mode 2 path.
- `CappedProvider::check` reads `state.cost_today()` (`capped.rs:68-73`), which is the figure
  `add_cost` maintains. So an unpriced model contributes **$0.00** to the ceiling on every turn.
- The engine already got this right one branch earlier — `engine.rs:449-459` deliberately does
  **not** fold `0.0` into `budget_status.cost_usd`, with the comment *"adding 0.0 would
  under-report spend and, once the cap lands, let an unpriced model spend past it silently."*
  The same author, forty lines apart, did the thing the comment forbids.

**Why it matters:** the mitigation chosen for this hazard was administrative — withdraw
unpriced models from the picker — and `TG-P2-02` enforces it for `MODEL_GROUPS`. F-1 shows that
enforcement has a hole (`DEFAULT_MODEL`), and a worker can carry any model string from the DB.
When an unpriced model does reach a turn, spend accrues in tokens, is logged as an error, and
advances the daily ceiling by nothing. The task is done at the price table and undone at the
two places that consume it. Refusing the turn, or adding a sentinel that forces the cap to
`Exceeded`, are both closer to the stated intent than `0.0`.

---
## F-12 · 🔴 P2-14 is unreachable — the volatile memory block is still inside the cached region on the live path
**Task:** P2-14 (and it undermines P2-13, P2-16, P2-12)
**Verdict:** NOT-DONE

**What the plan claims:**
> **P2-14** Remove volatile content from the system prompt (it is not byte-stable today). → `UNIT-PROVEN`

and, in the adapter, `autodev-core/src/provider/anthropic.rs:164`:
> *"Volatile content is not here to be marked around: `cache::prefix` keeps it out of this region entirely (P2-14)."*

**What the code does — the machinery exists.** `ContextBuilder::build_system_prompt_parts`
(`autodev-core/src/agent/context.rs:78-142`) genuinely returns `(stable, volatile)`, with the
memory block from `crate::memory::build_memory_context` correctly placed in the volatile half
and a careful docstring explaining why the order is load-bearing.

**What the code does — nothing consumes it.**
1. `build_system_prompt_parts` has **exactly one caller**: `build_system_prompt_with_mode`
   (`context.rs:54-62`), which immediately **rejoins** the two halves:
   `format!("{stable}\n\n---\n\n{volatile}")`.
2. `engine.rs:136` calls `build_system_prompt_with_mode` — the joined form.
3. `engine.rs:148-155` calls the prefix builder with that joined string as `system_base` **and
   an empty volatile list**:
   ```rust
   let cache_prefix = crate::cache::prefix::build(
       &tool_defs.iter().map(...).collect::<Vec<_>>(),
       &system_prompt,   // ← the JOINED string, memory included
       &[],
       &[],              // ← volatile_blocks: always empty
   );
   ```
   So `CachePrefix.volatile` is `""` at the only production call site, and `CachePrefix.stable`
   contains the memory block.
4. `engine.rs:355` sends `system: Some(system_prompt.clone())` — again the joined string.
5. `anthropic.rs:169-177` wraps that whole string in **one** `{"type":"text"}` block and hangs
   `cache_control` on it. There is exactly one block, so the breakpoint is after the memory.

**Why it matters:** this is the defect the task was written to remove, still present, with the
fix built and left unplugged. Every time the agent learns a fact, `build_memory_context` returns
different bytes, the whole multi-KB `tools → system` prefix in front of it is invalidated, and
the next request pays full input price **plus** the 1.25× cache-write surcharge — the exact
"naive caching costs more than no caching" outcome `P2-64` documents. The `anthropic.rs` comment
asserting the opposite is the kind of docstring `§0`'s Rule 5 classes as a bug rather than a
style issue.

**Why the evidence did not catch it:**
- `UNIT-PROVEN` is satisfiable by testing `build_system_prompt_parts` in isolation; the split is
  correct, so such a test passes while the live path ignores it.
- `P2-16`'s `BENCH-PROVEN` live measurement used `live_provider_smoke.rs:120 long_system_prompt()`
  — a synthetic, genuinely stable string — not the engine's assembled prompt. It proves the
  adapter can cache; it says nothing about whether the engine's prefix is stable between turns.
- `TG2-06` in `TESTING_GATES.md:169` is honestly marked **AMBER** ("not asserted over 100
  invocations, and not across processes"), but even its green half tests
  `cache::prefix::build`'s determinism given identical inputs — and the inputs are what differ
  here.

**Smallest correct fix:** have `engine.rs` call `build_system_prompt_parts`, pass `stable` as
`system_base` and `volatile` as `volatile_blocks`, and give the adapter two system blocks with
`cache_control` on the first. The prefix builder's `volatile` field already exists for this and
is presently write-only.

---

## F-13 · P2-64's single-use-prefix alarm cannot fire at its only production call site
**Task:** P2-64 (the monitoring half)
**Verdict:** PARTIAL

**What the plan claims:**
> **P2-64** **Hoist per-run identifiers out of cached prompts.** … → `BENCH-PROVEN` *(… Protocol is now a const with per-run facts appended; 6 reviewers share one key …)*

and `cache/monitor.rs:161`: *"Warn when a prefix is single-use across many requests (`P2-64`)."*

**What the code does — the prompt half is real.** `recipes/defaults.rs:1573` holds
`QC_REVIEWER_PROTOCOL` as a const with per-run facts appended after it, and
`builtin_tools/spawn_subagent.rs:111` carries the same ordering comment. That part is done and
`defaults.rs:2067` even asserts six reviewers hash to one key.

**What the code does — the alarm half is inert.** `CacheMonitor::report_reuse`
(`cache/monitor.rs:167-181`) fires when `requests >= 4 && distinct * 2 > requests`. Its only
production caller is `engine.rs:620` `cache_monitor.report_reuse(turn as u32)`, and at that
point:
- `distinct_prefixes()` is **structurally 1**. `cache_prefix` is computed **once**, before the
  loop (`engine.rs:148`), and the same `cache_prefix.hash` is passed to every `observe` call
  (`engine.rs:409`). One run can only ever register one prefix.
- The monitor itself is constructed per run (`engine.rs:194 CacheMonitor::new()`), so it never
  sees a second run's prefix either — which is the six-concurrent-QC-reviewers scenario the
  warning was written for.

So the condition reduces to `turn >= 4 && 2 > turn`, which is unsatisfiable. The warning cannot
be emitted by any execution of the shipped code.

Two smaller notes on the same monitor:
- `CacheVerdict::BelowMinimum` is likewise unreachable from the engine: `observe` is only called
  when `cache_policy != Off` (`engine.rs:403`), and the policy is `Off` precisely when
  `will_cache` returned false for being below the minimum. The below-minimum warning is
  delivered by `prefix::will_cache` instead, so nothing is lost — but a verdict variant that
  production cannot produce is worth knowing about.
- Because the monitor is per-run, the cross-run silent-invalidator case — the one F-12 creates —
  always reads as `FirstSighting` and never alarms. The instrument built first (`P2-12`) cannot
  see the defect that is actually present.

**Why it matters:** the measurement in the `BENCH-PROVEN` annotation is real and the prompt fix
is real. The standing detector that was supposed to catch a *recurrence* is not. A process-level
monitor, or seeding the per-run monitor from a shared store, is what the warning's own docstring
describes.

---
## F-14 · P2-33's orphan enumeration and P2-71's backend disclosure both stop at the IPC boundary — no UI calls either
**Task:** P2-33, P2-71 (and P2-24's "tell the user which store is in use", G-06)
**Verdict:** PARTIAL (both), and P2-71's replacement doc text repeats the defect it was fixing

**What the plan claims:**
> **P2-33** **The vault has no `enumerate`** — keys for removed providers linger in the OS keychain **with no way to find them**. → `UNIT-PROVEN`
>
> **P2-71** The credential store in use is **never disclosed** … → `UNIT-PROVEN` *(`VaultBackend` with `label()` and `security_note()`, **surfaced through** `VaultEntryStatus` and the new `vault_backend_info` command …)*
>
> **P2-24** … **tell the user which store is in use** (G-06) … *(… **the UI says so**.)*

**What the code does — the Rust side is real and correctly wired.**
`VaultManager::enumerate` (`autodev-core/src/vault/keychain.rs:208-224`) unions a name-only
index with the fallback vault's keys, and it **is** called — at
`src-tauri/src/commands/vault_commands.rs:291`, inside `list_vault_entries`, which merges it
with `PROBED_PROVIDERS` and marks unrecognised entries orphaned. `VaultBackend::label()` /
`security_note()` (`keychain.rs:34-74`) are real, and `vault_backend_info`
(`vault_commands.rs:90`) returns them. Both commands are registered in
`src-tauri/src/lib.rs:185, 238-239`.

**What the code does — nothing on the other side of the bridge calls them.**
`grep -rn "list_vault_entries\|vault_backend_info" src/` returns **zero hits**. The frontend
invokes `list_providers`, `set_provider_key`, `delete_provider_key`,
`test_provider_connection`, `has_github_pat`, `set_github_pat`, `delete_github_pat` — and
neither of these two. There is no screen, panel or badge anywhere in `src/` that renders a
vault entry list or a backend name.

**Why it matters, per task:**
- **P2-33.** The stated defect is *"keys for removed providers linger … with no way to find
  them."* A Tauri command with no caller is not a way for a user to find them. The Rust
  capability exists; the user-visible symptom is unchanged. `Settings.tsx` renders only the
  three current providers, so an orphaned `xai`/`grok`/`ollama` key remains exactly as
  invisible as before — which is the entire point of `PROBED_PROVIDERS` keeping those ids.
- **P2-71 / P2-24.** Worse than unfinished: `src/data/docsMore.ts:161` now tells the user
  *"**Settings shows which store is actually in use**, because the two are not equivalent."*
  Settings does not, and cannot — nothing invokes `vault_backend_info`. `P2-71`'s stated defect
  was *"`docsMore.ts:160` claims behaviour the code does not implement"*; the corrected text
  makes a **new** claim the code does not implement. The annotation's word "surfaced" is doing
  work the code does not do.
- Adjacently, `src/tabs/Settings.tsx:543` still reads *"Stored securely in your OS keychain"*
  for the GitHub PAT, and `:485` reports *"PAT saved to OS keychain"* unconditionally — both
  false whenever `VaultBackend::EncryptedFile` is the active store, which is the whole scenario
  `P2-24` exists for.

**Not in dispute:** `P2-70` (`VaultManager::exists` zeroizes, two call sites fixed —
`vault_commands.rs:185`, `:38`) and `P2-72` (`Settings.tsx:1096-1117`, all three of the nine-line
defects genuinely addressed, with validation now coming from the backend save path) are
**CONFIRMED-DONE**.

---
## F-15 · 🔴 The daily cap cannot be set or enabled from the UI — `setDailyBudget` still has no caller
**Task:** P2-50 (and it makes P2-21/P2-22/S2-05 unreachable in the shipped product)
**Verdict:** PARTIAL — and the third of the three defects the task names is untouched

**What the plan claims:**
> **P2-50** `dailyBudget` is unsettable — `dashboardStore.ts:173` and `CostMeter.tsx:20` hardcode 50, **and `setDailyBudget` has no caller**. → `UNIT-PROVEN` *(There were **three** disconnected numbers, not two … **All three now read `get_daily_cap`**.)*

and the phase's **narrative success criterion**:
> *"And they can set a daily cap and watch it actually stop."*

**What the code does — two of three are fixed.** `CostMeter.tsx:10-11,25` now takes
`dailyBudget` as a **required** prop with no default, `Dashboard.tsx:227` passes it, and
`dashboardStore.loadDailyCap` (`:332-342`) reads the real ceiling via `get_daily_cap` and is
called on mount (`Dashboard.tsx:95`).

**What the code does — the third is not.** `grep -rn "setDailyBudget" src/` returns exactly two
hits: the interface declaration (`dashboardStore.ts:55`) and the implementation (`:344-354`).
**No component calls it.** `grep -rn "set_daily_cap" src/` returns one hit — inside that same
uncalled function. There is no input, slider, toggle or settings row anywhere in `src/` that
writes a daily cap:
- `dailyBudgetEnabled` (`dashboardStore.ts:57,185`) is only ever *assigned from* a backend
  response; nothing in the UI toggles it.
- `DailyCap::default()` is `enabled: false` (`autodev-core/src/budget/daily.rs:61-66`), and
  `DailyCap::load()` returns `enabled: false` when the settings row is absent (`:77-81`).
- `CappedProvider::check` therefore evaluates to `CapState::Disabled` on a fresh install and
  stays there forever (`provider/capped.rs:75-76`).

**Why it matters:**
1. The enforcement machinery of `P2-21` and `P2-22` — the decorator, `pause_fleet`, the
   `BudgetExceeded` event — is complete, correct, and **unreachable in the shipped product**,
   because the only switch that arms it has no user-facing control. This is the same
   "≈100 lines of enforcement with no entry point" shape that `P2-48` was written to fix, one
   layer up.
2. `CostMeter.tsx:91` renders `Budget: $X / $50.00` unconditionally. `dailyBudgetEnabled` is
   loaded and **never read in the render**, so the dashboard displays a ceiling that is not
   enforcing, with nothing to say so. That is a fourth disconnected number in place of the three
   removed — a displayed budget with no enforcement behind it.
3. The phase's own narrative success criterion cannot be executed by a user.

**Secondary — S2-05's evidence does not match its criterion.**
> **S2-05** Exceeding the daily cap **pauses the fleet with a visible message**. → **PASS.** The ceiling fires **at** the limit (`>=`) … a disabled cap never reports `Exceeded` … the meter saturates at 1.0 …

The three facts reported are all true and all proven — `autodev-core/tests/s2_spend_ceilings.rs:17-60`
tests `DailyCap::evaluate` and `fraction_used` directly. **None of them is the criterion.**
Nothing in that file, or anywhere else, exercises `pause_fleet`, `CappedProvider::check`, or any
surfacing of the pause. The gate register's `TG2-17` is correctly left unchecked; the S2 row is
not.

---
## F-16 · `max_output` was added to clamp `max_tokens` and nothing reads it
**Task:** P2-08 / P2-45
**Verdict:** PARTIAL

**What the plan claims:**
> **P2-08** Honour `max_tokens` on OpenAI and Google (silently dropped today). → `UNIT-PROVEN`
> **P2-45** `maxOutputTokens` is sent nowhere; `max_tokens` is read only at `anthropic.rs:134`. → `UNIT-PROVEN`

**What the code does — the stated defect is fixed.** `openai.rs:233` sends
`max_completion_tokens`, `google.rs:176` sends `maxOutputTokens` unconditionally, and
`conformance_tests.rs:65 every_adapter_sends_an_output_ceiling` asserts it on the built request
body rather than the struct. That is genuinely done.

**What the code does — the field added alongside it is dead.**
`autodev-core/src/provider/catalogue.rs:46-49`:
```rust
/// Maximum tokens the model will emit in one response. Used to clamp
/// `max_tokens` (`P2-08`) rather than let the provider reject the request.
pub max_output: u32,
```
`grep -rn "max_output" --include=*.rs .` outside `catalogue.rs` returns only the
`ModelCapabilities` field declaration (`usage.rs:396`) and three test literals
(`monitor.rs:194`, `prefix.rs:199`, `usage.rs:560`). **No production code reads it.** Every
model's value — 64,000 / 65,536 / 128,000 — is carried through `capabilities_for()` and
discarded, while `engine.rs:354` hardcodes `max_tokens: 8192` for every model.

**Why it matters:** the docstring states a behaviour the code does not perform, and cites the
task number while doing it — the class §0 Rule 5 treats as a bug rather than a style issue. It
also means the clamp that would prevent a provider-side rejection (`max_tokens` above a model's
ceiling) does not exist, in a field created to provide it.

---

## F-17 · P2-30 fixed the sleep and left the "aggregates only yesterday" half of its own defect statement unaddressed
**Task:** P2-30
**Verdict:** PARTIAL

**What the plan claims:**
> **P2-30** **The daily rollup sleeps an hour before its first run and aggregates only *yesterday*** (`G3c IN-56`) … → `UNIT-PROVEN`
> ✅ **2026-09-02** — The rollup runs immediately, then hourly — it slept an hour **before** its first run … `eprintln!` → `tracing::error!`.

**What the code does:**
- The sleep half is fixed: `autodev-core/src/analytics/mod.rs:92-102` runs
  `run_daily_rollup()` **before** the `sleep(3600)`, and logs through `tracing::error!`. Both
  claims in the annotation are true.
- The second half of the stated defect is untouched: `analytics/aggregator.rs:37-42` still
  computes `let yesterday = today - chrono::Duration::days(1);` and aggregates
  `get_token_usage_by_date(yesterday)` only.

**Why it matters:** it may well be correct — `analytics/queries.rs:62,75` document a deliberate
split where today is read live from `token_usage` and yesterday from the `daily_costs` rollup,
which makes "aggregates only yesterday" the intended design rather than a bug. But the task text
names it as part of the defect, the evidence line addresses only the sleep, and nothing records
that the other half was reconsidered and dismissed. Under the plan's own no-stale rule, a task
whose statement is half-wrong should have the statement corrected rather than silently
half-closed — the same discipline `P2-74` applied to `P2-11`.

---

## F-18 · Residual Ollama references and no gate for `TG2-14`
**Task:** P2-11 / P2-73 / P2-74 / P2-75
**Verdict:** CONFIRMED-DONE (with residue)

The Ollama removal is genuinely complete where it counts. `ollama-rs` is absent from **both**
manifests (verified: no `ollama` dependency line in `src-tauri/Cargo.toml`, `autodev-core/Cargo.toml`,
or `Cargo.lock`). `factory::build("ollama", ..)` is refused and tested (`factory.rs:85`);
`list_providers` offers exactly anthropic/openai/google (`settings_commands.rs:27-31`);
`PROBED_PROVIDERS` deliberately retains `ollama` so an orphaned key stays findable
(`vault_commands.rs:35`, with `is_supported_provider("ollama") == false` asserted at `:334`);
`src/` contains **no** Ollama reference at all. `P2-73`, `P2-74`, `P2-75` are all confirmed.

Two residues worth a line rather than a verdict change:
- `src-tauri/Cargo.toml:87-88` still carries the two-line comment *"# Ollama local LLM provider
  — Phase X / # Task D3: Keep 0.3.4 …"* describing a dependency that no longer exists.
  `TG2-14`'s stated scope explicitly includes `Cargo.toml`.
- **No `TG-P2-nn` implements `TG2-14`.** It is correctly left `- [ ]` in `TESTING_GATES.md:194`,
  so nothing is overclaimed — but the removal has no standing guard, and `TG2-14`'s text also
  still says *"Exactly four providers exist: Anthropic, OpenAI, Google, xAI"*, stale since the
  `P2-07` xAI strike.

---
# Summary

## Verdict table — all 74 P2 tasks

| Task | Verdict | Finding / evidence |
|---|---|---|
| P2-01 | CONFIRMED-DONE | Six-field `Usage`, `CachePolicy`, `ErrorClass`, `ModelCapabilities` all present, 22 unit tests in `usage.rs`. Naming note: the trait is `Provider`, not `ModelProvider`, and `Item::Opaque` ships as `MessageContent::ToolUse::opaque` / `StreamChunk::ToolUseStart::opaque`. Substance matches; the plan's names do not exist as written. |
| P2-02 | CONFIRMED-DONE | Four variants present (`usage.rs:159-174`). `Handle` is constructed nowhere — correctly, since the task is to keep the shape. |
| P2-03 | CONFIRMED-DONE | `Provider::capabilities(&self, model)` defaults to `catalogue::capabilities_for` (`traits.rs:32-36`), keyed by model id. |
| P2-04 | CONFIRMED-DONE | |
| P2-05 | CONFIRMED-DONE | |
| P2-06 | CONFIRMED-DONE | Arguments survive on both Google paths. |
| P2-07 | N/A (verified struck) | No xAI adapter; `factory::build("xai", ..)` refused and tested (`factory.rs:80-83`); `xai`/`grok` retained in `PROBED_PROVIDERS` as stated. |
| P2-08 | **PARTIAL** | F-16 |
| P2-09 | CONFIRMED-DONE | Three conformance tests, one per wire shape (`conformance_tests.rs:321, 356, 383`). |
| P2-10 | CONFIRMED-DONE | `StopReason::parse` + `is_truncated`, consumed at `engine.rs:428`. |
| P2-11 | CONFIRMED-DONE | F-18 (residue only) |
| P2-12 | **PARTIAL** | F-13 — the alarm exists and is consulted, but is per-run, so the cross-run invalidator F-12 creates always reads as `FirstSighting`. |
| P2-13 | **PARTIAL** | F-12 — one builder exists; the engine hands it the joined prompt, so its `volatile` output is always empty. |
| P2-14 | **NOT-DONE** | F-12 |
| P2-15 | CONFIRMED-DONE | `will_cache` + `MINIMUM_MARGIN` (`prefix.rs:153-176`), consulted at `engine.rs:160`. |
| P2-16 | **PARTIAL** | F-12 — wired on all three and measured live, but the live measurement used a synthetic stable prompt, not the engine's assembled one. |
| P2-17 | CONFIRMED-DONE | `streaming.rs:44-60`. |
| P2-18 | CONFIRMED-DONE | Exact-id table, `Result` return, `_ => (1.0, 3.0)` gone. Its two consumers are F-11. |
| P2-19 | CONFIRMED-DONE | Anthropic as-reported; OpenAI/Google via `from_inclusive_input`. |
| P2-20 | CONFIRMED-DONE | `PriceTier` on `gemini-3.1-pro-preview` including the cached-read tier (`pricing.rs:138-145`). |
| P2-21 | CONFIRMED-DONE (code) | F-9 (gate is narrower than its claim), F-15 (unreachable in the product) |
| P2-22 | **PARTIAL** | F-15 — `pause_fleet` is real and correct; nothing can arm the cap that calls it. |
| P2-23 | **NOT-DONE** (Google path) | F-3 |
| P2-24 | **PARTIAL** | F-14 — the fallback vault is real; "tell the user which store is in use" is not delivered. |
| P2-25 | CONFIRMED-DONE | Namespaced provider keys, PAT outside the namespace, tested (`keychain.rs:315-321`). |
| P2-26 | CONFIRMED-DONE | `hydrate_active_provider` called at `main.rs:143`; retired models discarded. |
| P2-27 | CONFIRMED-DONE | `engine.rs:200-222` reads `ModelConfigsDb`; `engine.rs:359-373` puts all four fields on the request; `TG-P2-08` gates it. |
| P2-28 | CONFIRMED-DONE | `#![cfg(feature = "live-smoke")]` at `live_provider_smoke.rs:36`. 394 test attributes total, 382 outside the feature — reconciles with the claimed count exactly. |
| P2-29 | CONFIRMED-DONE | `cost_day` + `roll_day_if_needed` + `cost_today()`; readers go through it. |
| P2-30 | **PARTIAL** | F-17 |
| P2-31 | **PARTIAL** | F-1 |
| P2-32 | **OVERSTATED-EVIDENCE** | F-8 |
| P2-33 | **PARTIAL** | F-14 |
| P2-34 | CONFIRMED-DONE | `ToolInputDelta` emitted at `google.rs:458`. |
| P2-35 | CONFIRMED-DONE | `parse_google_chunk` returns `Vec` and walks every part. The same defect shape survives on OpenAI — F-7, filed under P2-67/68. |
| P2-36 | **PARTIAL** | F-4 |
| P2-37 | CONFIRMED-DONE | `MessageContent::ToolResult::name` + `google.rs:113`; tested at `conformance_tests.rs:160`. |
| P2-38 | CONFIRMED-DONE | `openai.rs:97-101`, `google.rs:119-123`; tested at `conformance_tests.rs:126`. |
| P2-39 | CONFIRMED-DONE | `openai.rs:111-131`; tested at `conformance_tests.rs:109`. |
| P2-40 | CONFIRMED-DONE | Both adapters coerce and warn; tested at `conformance_tests.rs:189`. |
| P2-41 | CONFIRMED-DONE | `openai.rs:499-504` + `openai_tests.rs:171`. |
| P2-42 | CONFIRMED-DONE | Both edits present — `openai.rs:253` and `:428-448` — and both tested. |
| P2-43 | CONFIRMED-DONE | See P2-17. |
| P2-44 | CONFIRMED-DONE | Both early returns gone; usage emitted whenever `finishReason` is present. |
| P2-45 | **PARTIAL** | F-16 |
| P2-46 | CONFIRMED-DONE | `stop_reason` out-parameter, truncation warns (`engine.rs:430-437`). |
| P2-47 | CONFIRMED-DONE | `engine.rs:463-479`; `let _ =` gone, failure logged at error level. |
| P2-48 | CONFIRMED-DONE | `with_budget` has two real callers — `recipes/runner.rs:782`, `subagent/native_runner.rs:206` — and `TG-P2-03` counts callers, not just presence. |
| P2-49 | CONFIRMED-DONE | `emit_cost_update` now called from `engine.rs:971` (Mode 1) as well as `runner.rs:498`. |
| P2-50 | **PARTIAL** | F-15 |
| P2-51 | CONFIRMED-DONE | Exact-id matching makes the ordering trap unrepresentable. |
| P2-52 | **PARTIAL** | F-6 |
| P2-53 | CONFIRMED-DONE | `provider::routing::provider_for_model` / `provider_for_analytics`; all three former ladders delegate. Separate note: `factory.rs`'s header says provider *construction* is "in one place", but the three-arm `match provider_id` is still written out at `agent_commands.rs:137`, `diff_commands.rs:37`, `recipe_commands.rs:222`, `runner.rs:742`, `native_runner.rs:260` and `settings_commands.rs:63` — six copies. Outside P2-53's task text, but it leaves the factory's own docstring stale. |
| P2-54 | CONFIRMED-DONE | Failure branch records `(0, 1)` at `runner.rs:603-614`. |
| P2-55 | CONFIRMED-DONE | Stub deleted, with the reason recorded at `aggregator.rs:18-34`. |
| P2-56 | CONFIRMED-DONE | `thoughtsTokenCount` read on both Google paths, billed at the output rate. |
| P2-57 | CONFIRMED-DONE | `engine.rs:308-312` reads `context_window` from `models()`; `catalogue.rs:378` asserts Gemini's 1M. |
| P2-58 | CONFIRMED-DONE | `model` read at `engine.rs:790`. |
| P2-59 | CONFIRMED-DONE | Both accumulators are `BTreeMap<u64, _>` keyed by the wire block index; `assemble_in_wire_order` at `engine.rs:1022`. |
| P2-60 | CONFIRMED-DONE | `TextDelta.block_index` carried and used in assembly. |
| P2-61 | CONFIRMED-DONE | Results walked in `tool_calls` order (`engine.rs:583-598`); stragglers reported loudly. |
| P2-62 | CONFIRMED-DONE | `build_system_prompt` deleted; `context.rs:30-43` records why. |
| P2-63 | CONFIRMED-DONE | `body["system"] = json!([block])` (`anthropic.rs:177`); tested at `conformance_tests.rs:446`. |
| P2-64 | **PARTIAL** | F-13 — prompt restructuring done and measured; the standing detector cannot fire. |
| P2-65 | CONFIRMED-DONE | Measured live via `count_tokens`; `MINIMUM_MARGIN` exists because of it. |
| P2-66 | CONFIRMED-DONE | `compact_with_summary` returns usage; `usage_recorded` helper; folded into the turn total at `engine.rs:336-345`. |
| P2-67 | **PARTIAL** | F-7 — three suites of 9 exist and are real, but the parallel-tool fixture is shaped so the one frame shape OpenAI's parser cannot handle never appears. |
| P2-68 | **PARTIAL** | F-5, F-7 |
| P2-69 | CONFIRMED-DONE | Feature-gated, reads `.env` directly, no environment fallback. Minor: **12** `#[tokio::test]`s, not the 9 claimed. |
| P2-70 | CONFIRMED-DONE | `VaultManager::exists` zeroizes; both call sites converted; the honest limit is stated rather than glossed. |
| P2-71 | **PARTIAL** | F-14 |
| P2-72 | CONFIRMED-DONE | All three defects addressed at `Settings.tsx:1096-1117`. |
| P2-73 | CONFIRMED-DONE | F-18 (comment residue only) |
| P2-74 | CONFIRMED-DONE | Measurements recorded and corrected. |
| P2-75 | CONFIRMED-DONE | Verified independently, not assumed. |

## Verdict table — S2 smoke rows

| Row | Verdict | Note |
|---|---|---|
| S2-01 | CONFIRMED-DONE | `every_catalogued_model_is_actually_callable` exists and iterates the catalogue. |
| S2-02 | CONFIRMED-DONE | Measured on both Anthropic and OpenAI. Caveat: a synthetic prompt, not the engine's — F-12. |
| S2-03 | CONFIRMED-DONE | Recomputed against the price table to 1e-9. |
| S2-04 | **OVERSTATED-EVIDENCE** | F-8 — the conclusion holds, but "by construction" rests on a gate narrower than the invariant, and the shipped binary loads the dev `.env`. |
| S2-05 | **OVERSTATED-EVIDENCE** | F-15 — the three facts reported are proven; none of them is the row's own criterion (fleet pauses, message visible). |
| S2-06 | CONFIRMED-DONE | Persist + date-stamp + hydrate, wired at `main.rs:140`, with a behavioural stale-day test. |
| S2-07 | **OVERSTATED-EVIDENCE** | F-10 — right outcome, wrong mechanism; `airsctl` builds no provider at all. |
| S2-08 | CONFIRMED-DONE | Real parallel Gemini calls carrying real arguments. |
| S2-09 | CONFIRMED-DONE | |
| S2-10 | CONFIRMED-DONE | This is what found the `thoughtSignature` breakage. |
| S2-11 | CONFIRMED-DONE | Honestly labelled "PASS, structurally"; the test is a source-text assertion and says so. |
| S2-12 | CONFIRMED-DONE | |

## Final tally

| Verdict | Count | Task IDs |
|---|---|---|
| **CONFIRMED-DONE** | 54 | P2-01…06, 09, 10, 11, 15, 17, 18, 19, 20, 21, 25, 26, 27, 28, 29, 34, 35, 37…44, 46…49, 51, 53…63, 65, 66, 69, 70, 72, 73, 74, 75 |
| **PARTIAL** | 17 | P2-08, 12, 13, 16, 22, 24, 30, 31, 33, 36, 45, 50, 52, 64, 67, 68, 71 |
| **NOT-DONE** | 2 | **P2-14**, **P2-23** (Google path) |
| **OVERSTATED-EVIDENCE** | 1 task (+3 S2 rows) | **P2-32**; S2-04, S2-05, S2-07 |
| N/A (verified struck) | 1 | P2-07 |

74 tasks accounted for. Of the 74, **20 are not what the plan says they are** — 17 partial, 2 not
done, 1 overstated — and 3 of the 12 S2 rows report evidence that does not match their own
criterion.

## The five that matter most

1. **F-12 · P2-14 is unreachable.** The stable/volatile split exists, is well documented, and is
   rejoined by its only caller before the engine ever sees it. Memory sits inside the cached
   region on every Anthropic request, and `anthropic.rs:164` asserts the opposite in a comment.
   This is the "fix that lands in the right place and is never reached" failure the phase has
   already paid for twice.
2. **F-15 · the daily cap has no UI.** `setDailyBudget` still has no caller — the third of the
   three defects `P2-50` names, and the annotation says all three were fixed. Everything
   downstream (`CappedProvider`, `pause_fleet`, `BudgetExceeded`) is correct and cannot be
   armed. P2's narrative success criterion — "they can set a daily cap and watch it actually
   stop" — is not executable by a user.
3. **F-8 · the shipped binary loads the dev `.env`.** `main.rs:27 dotenvy::dotenv()` puts three
   provider keys plus an AWS secret into the process environment, which every subprocess —
   including the model-driven `bash` tool — inherits. `TG-P2-07`'s same-line pattern cannot see
   it, and `P2-32` states the opposite in as many words.
4. **F-1 / F-3 · two dead model ids in shipped code**: one is the app's `DEFAULT_MODEL` for every
   new worker, the other sits inside Google's key-validation URL where it can refuse a valid key
   on the primary provider. Both escape `TG-P2-09` structurally — TypeScript is outside its scan
   roots, and an id inside a format string does not match its `starts_with` test — so this is a
   gate-design problem, not two oversights.
5. **F-6 / F-13 · two alarms that cannot fire.** `P2-52`'s zero-usage check is gated on a
   tool-turn counter that is 0 for the commonest turn shape; `P2-64`'s single-use-prefix warning
   has a condition that is unsatisfiable at its only production call site. Both were built as
   the standing detector for a defect that had already been paid for once.

## Cross-cutting observation: the gates are narrower than their claims

Four of the eight implemented P2 gates assert something materially narrower than the invariant
they are cited for, and in three cases a live defect sits in the gap:

| Gate | Asserts | Gap |
|---|---|---|
| `TG-P2-04` | a file containing **both** `AnthropicProvider::new(` **and** `GoogleProvider::new(` also contains `CappedProvider::wrap` | single-provider construction sites invisible; `src-cli` not scanned; no floor assertion |
| `TG-P2-06` | occurrence count of `ErrorClass::classify` ≥ count of `if !status.is_success()` | says nothing about the classification being usable by a consumer (F-5) |
| `TG-P2-07` | no **single line** both calls `env::var` and spells a provider key | misses `dotenvy::dotenv()`, dynamic names, and two-line reads (F-8) |
| `TG-P2-09` | quoted tokens in `.rs` production code that `starts_with("gemini-"/"claude-"/"gpt-")` are in the catalogue | misses TypeScript entirely (F-1) and any id inside a format string (F-3) |

And `TG-P2-05` — cited twice in `catalogue.rs` as the gate that "fails the build" when the
catalogue and `models.ts` disagree — **does not exist** (F-2). That is the one invariant the
catalogue module was created to protect, and the lists have already drifted.

## Method notes and what this audit did not do

- **`cargo test --workspace` was not run.** `autodev-core/src/vault/keychain.rs:324
  the_backend_probe_treats_no_entry_as_a_reachable_keychain` calls `VaultManager::backend()`,
  which opens a `keyring::Entry` and calls `get_password()` — it touches the OS credential
  store, which this audit was instructed not to do. Instead: `cargo check --workspace
  --all-targets` was run and **completed clean** (0 errors, warnings only), proving every test
  target compiles; and test attributes were counted statically at **394**, of which 12 sit
  behind `#![cfg(feature = "live-smoke")]` — leaving **382**, matching the claimed count exactly.
- `cargo test --features live-smoke` was not run (it spends real money), so every `BENCH-PROVEN`
  and live `SMOKE-PROVEN` number is taken at face value. What was audited is whether the test
  that produced a number exercises what the claim says it exercises — which is where S2-04,
  S2-05 and S2-07 came apart.
- No source file was modified. No API key environment variable was set. `.env` was inspected for
  **key names only** (`grep -oE "^[A-Z_]+="`); no value was read, printed, or stored.
