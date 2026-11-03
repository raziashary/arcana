# A8 · P2 Area Audit — Adversarial Defect Review

**Reviewer:** independent adversarial agent (has not seen the implementer's reasoning)
**Date:** 2026-09-02
**Scope:** `autodev-core/src/provider/`, `cache/`, `cost/pricing.rs`, `budget/daily.rs`,
`vault/`, `state/app_state.rs`, `agent/engine.rs`, `agent/compaction.rs`,
`src-tauri/src/commands/{vault,settings,budget}_commands.rs`

**Method:** read-only source review. `cargo test --workspace` run once. No live-smoke,
no keychain writes, no `~/.autodev/` writes, no API keys set.

---

## Count by severity — 14 findings

| Severity | Count | Findings |
|---|---|---|
| CRITICAL | 1 | D-1 |
| HIGH | 5 | D-2, D-3, D-7, D-10, D-12 |
| MEDIUM | 5 | D-4, D-5, D-6, D-8, D-9 |
| LOW | 3 | D-11, D-13, D-14 |

### The four that matter most

1. **D-1** — a Mode 1 run that fails or hits its task budget records **$0** against the daily ceiling,
   however much it spent. `add_cost` is reachable only when the turn loop exits by `break`; three
   `return` paths skip it.
2. **D-12** — the P2-14 stable/volatile cache split is built, documented, unit-tested, and **never
   wired**. The engine calls the wrapper that rejoins the halves, so the volatile memory block sits
   inside the cached region and every turn pays the cache-write surcharge for a prefix that can never
   be read back.
3. **D-7** — an unpriced model bills real money and records $0. The catalogue gate covers models that
   come *from* the catalogue; four of the five paths that choose a model never consult it.
4. **D-10** — Google key validation calls `gemini-2.5-flash`, which the repo's own test asserts is
   retired, via a **billed completion** that `capped.rs` exempts from the ceiling on the written
   grounds that it is free.

Three findings are defects of the *same* shape: a fix implemented correctly in one place and not
connected at the point of use (D-12 the prefix split, D-13 the breakpoints field, D-6 the redaction
helper). D-5 and D-13 are values computed and discarded.

---

## Findings

## D-1 · A failed or budget-halted Mode 1 agent run records $0 against the daily ceiling, however much it spent
**Severity:** CRITICAL
**File:** `autodev-core/src/agent/engine.rs:672-675` (the only `add_cost` call), vs. early exits at `:292`, `:510`, `:552`
**The defect:** `AppState::add_cost` — the *only* writer of the figure `CappedProvider::check()` reads — is
called once, after the turn loop, and is reachable only when the loop exits via `break`. Three paths return
out of the loop body and skip it entirely, discarding the whole run's accumulated `total_usage`:

- `:292` `return Err(AutoDevError::BudgetExceeded(reason_str))` — the per-task budget trips.
- `:510` `return Err(e)` — `stream_response` failed on any turn.
- `:552` `tool_executor.execute_batch(tool_batch).await?` — the `?` propagates a tool-executor error.

`total_usage` at those points already holds every billed turn of the run (plus any compaction call folded
in at `:343`). None of it reaches `add_cost`, so none of it reaches `cost_today()`, so none of it counts
toward the daily ceiling.

**Failure scenario:** Daily cap enabled at $50. A Mode 1 run does 18 turns on a large context (say $9 of
real spend), then turn 19's stream fails on a 529 / connection reset. `:510` returns `Err`. `cost_today_usd`
is unchanged at $0.00. The user retries; the same thing happens four more times. $45 of genuinely billed
API usage has occurred and `CappedProvider::check()` still evaluates `0.0 >= 50.0` → `Ok`. The ceiling has
not moved at all. The `:292` variant is worse in kind: the run that hits the per-task budget is by
construction the *most expensive* run, and it is the one guaranteed to record nothing.

**Why it survives today:** Every unit test of the ceiling tests `DailyCap::evaluate` against a
hand-supplied `spent` figure — `budget/daily.rs` tests, `capped.rs` tests, `tests/s2_spend_ceilings.rs`
(which greps `add_cost` for the string `persist_today` rather than exercising the path). Nothing tests
that a *failed run's* spend reaches `add_cost`. The happy path works, so a manual smoke shows the cap
working. Note the asymmetry with Mode 2: `recipes/runner.rs:457` calls `add_cost` **inside** its loop,
per turn, so the recipe path is not affected — which is likely why this was not noticed.

**Suggested fix:** Record incrementally rather than at the end. Move the `CostTracker::calculate` +
`state.add_cost` pair to just after the per-turn usage is known (near `:448`, using
`total_usage.delta_since(&usage_before_turn)`), matching what `recipes/runner.rs` already does. Failing
that, wrap the loop body so every exit path funnels through a single cost-recording epilogue (e.g. extract
the loop into an inner closure/fn and record on both `Ok` and `Err` before returning).

## D-2 · A key rotation can silently not take effect, leaving AutoDev using the old (possibly leaked) key
**Severity:** HIGH
**File:** `autodev-core/src/vault/keychain.rs:98-112` (`store`) vs `:115-135` (`get`)
**The defect:** `store()` writes to the OS keychain, and on **any** keychain error falls back to writing
the encrypted file — without removing or invalidating the keychain entry. `get()` reads the keychain
**first** and returns its value whenever that read succeeds. The two form a write-to-B / read-from-A pair,
so a fallback write is invisible to every subsequent read as long as the keychain still holds the old
entry. The `Err(e) =>` arm at `:104` is a catch-all: it fires for a transient error, a locked keychain, a
denied user prompt or a D-Bus hiccup — not only for "no keychain exists on this machine".

**Failure scenario:** A user's Anthropic key leaks. They paste a new key into Settings. The macOS Keychain
happens to be locked (or Windows Credential Manager returns a transient error). `store()` logs a `warn!`
nobody reads, writes the **new** key into `~/.autodev/config/vault.enc`, and returns `Ok(())` — the UI
reports success. Every subsequent `get()` hits the keychain, which still holds the **old, leaked** key, and
returns it. AutoDev keeps billing the compromised key indefinitely while the user believes they rotated it.
The fallback file now also holds a second, divergent copy of a live credential.

**Why it survives today:** The failure needs a keychain error at exactly one moment, which never happens on
a developer machine. `store()` returns `Ok`, the UI confirms, and there is no read-back verification.
`VaultManager::backend()` (P2-71) does not help: it probes *at display time*, so once the keychain is
reachable again it reports `OsKeychain` — the disclosure is wrong for precisely the case it exists to
cover. No test exercises a keychain-error store followed by a keychain-success get.

**Suggested fix:** On the fallback path, first attempt `entry.delete_password()` so a stale keychain entry
cannot shadow the file. Also return where the secret actually landed (`Result<VaultBackend>` from `store`)
so the UI can report the downgrade instead of silently reporting plain success.

## D-3 · A crash or a concurrent write during a fallback-vault save destroys every stored credential
**Severity:** HIGH
**File:** `autodev-core/src/vault/fallback.rs:67-93` (`write_owner_only`), `:163-183` (`save`), `:199-213`
**The defect:** `save()` rewrites the whole vault in place. `write_owner_only` opens the real `vault.enc`
with `.truncate(true)` and then `write_all`s — no temp-file-plus-rename, no `sync_all()`, no lock. The file
is therefore zero-length or partial for the duration of every write. AES-GCM cannot decrypt a partial blob,
so a truncated `vault.enc` is not degraded, it is **total loss of every key in the vault**.
`store`/`delete` are also unsynchronised read-modify-write cycles (`load()` -> mutate -> `save()`), so two
concurrent callers lose one of the two updates. The non-unix branch (`std::fs::write`) truncates the same
way.

**Failure scenario (loss):** User adds a Google key. `store()` truncates `vault.enc` and begins writing.
The machine loses power, the process is killed, or the disk returns ENOSPC mid-`write_all`. On restart
`load()` returns either "too short to contain a nonce" or "could not be decrypted", and the Anthropic and
OpenAI keys that were in that file are gone permanently — no backup, no prior copy.
**Failure scenario (lost update):** The desktop app and the CLI are both open. Each does `load()` -> insert
-> `save()`. Whichever writes second reverts the other's key, and the UI reported success for both.

**Why it survives today:** The write is a few hundred bytes and completes in microseconds, so the window is
tiny — until it is not. The disk path is deliberately not exercised by `cargo test` (see the comment at
`fallback.rs:232-237`), so nothing about the file handling is tested at all; every test in the module
operates on an in-memory `BTreeMap` or on the cipher directly.

**Suggested fix:** Write to `vault.enc.tmp` (created `0o600`), `sync_all()`, then `std::fs::rename` over
the real path — rename is atomic within a directory. Serialise `store`/`delete` behind a process-level
mutex, and use a lockfile for the cross-process case.

## D-4 · Losing (or failing to copy) `vault.key` bricks the vault permanently, with no recovery path
**Severity:** MEDIUM
**File:** `autodev-core/src/vault/fallback.rs:96-120` (`load_or_create_key`), called from `:137` and `:164`
**The defect:** `load()` calls `load_or_create_key()`, which **creates and writes a brand-new random key**
whenever `vault.key` is absent — including when `vault.enc` exists and is non-empty. The decrypt then fails,
`load()` returns `Err`, and because `store()`, `get()`, `delete()` and `keys()` all begin with `load()?`,
every vault operation now fails forever. The user cannot even store a *replacement* key, because storing
requires loading first. The error names the cause and no remedy.

**Failure scenario:** A user restores `~/.autodev` from a backup, or syncs their dotfiles, and `vault.enc`
comes across while `vault.key` does not — it is a 32-byte binary blob that a backup filter or `.gitignore`
will plausibly exclude, and nothing marks the two files as a pair. On next launch the app writes a fresh
`vault.key`, then answers every vault call with "the vault could not be decrypted — the key file and the
vault file do not match, or one of them is corrupt". Settings cannot save a new API key. The app is
unusable and nothing says the fix is to move `vault.enc` aside.

**Why it survives today:** It needs the two files to be separated, which does not happen in development.
The error message is accurate about the cause and silent about the remedy, so it reads as informative
rather than as a dead end.

**Suggested fix:** Do not create key material inside the read path. Give `load()` a `load_key()` that errors
when `vault.enc` exists but `vault.key` does not, and state the recovery in the message. Optionally let
`store()` offer to re-key when the existing vault is provably undecryptable and the user confirms.

## D-5 · `fallback::delete` discards the "was anything removed?" answer, and a test comment claims it does not
**Severity:** MEDIUM
**File:** `autodev-core/src/vault/fallback.rs:209-213`; consumed at `keychain.rs:180-183`; claim at `fallback.rs:336-338`
**The defect:** `apply_delete` returns a `bool` saying whether an entry existed. `fallback::delete` calls it
and drops the value on the floor (`apply_delete(&mut entries, key);` — result unused), returning `Ok(())`
either way. `VaultManager::delete` then sets `removed_anywhere = true` merely because `fallback::is_present()`
was true and the call did not error. So deleting a key that exists nowhere returns success whenever a vault
file exists at all, defeating the `no stored secret for {key}` error at `keychain.rs:190`. The test at
`fallback.rs:336-338` asserts the bool and comments *"`VaultManager::delete` surfaces that to the caller"* —
which is false; the only consumer discards it. This is a guard nothing calls, plus a comment promising
behaviour the code does not implement.

**Failure scenario:** The frontend deletes `provider.google.api_key` twice, or deletes a key that was never
stored. Both return success, the row disappears, and the UI reports a credential deleted having verified
nothing. Every such no-op also re-encrypts and rewrites the entire vault file — which, under D-3, is a fresh
opportunity to lose it.

**Why it survives today:** The test that would catch it exercises `apply_delete` (the pure helper) instead
of `fallback::delete` (the function with the defect), and its comment asserts the wiring rather than
testing it. From the UI a double-delete is indistinguishable from a real one.

**Suggested fix:** Make it `pub fn delete(key: &str) -> Result<bool>` returning `apply_delete`'s answer and
skipping `save()` entirely when nothing was removed; have `keychain::delete` derive `removed_anywhere` from
it. Then correct or remove the claim in the test comment.

## D-6 · The transport-error redaction fix was applied to one adapter out of three
**Severity:** MEDIUM
**File:** `autodev-core/src/provider/google.rs:345-355` (has it) vs `anthropic.rs:325-328` and `openai.rs:382-385` (do not)
**The defect:** Google's `test_connection` runs the transport error through
`security::redaction::redact_text` before logging it *and* before putting it in the returned
`ProviderStatus::Error`, with a comment stating the reason: *"this value crosses the Tauri IPC boundary
into the renderer (`Settings.tsx`) ... an error string that reaches the UI is not a place to trust a
transport library's formatting."* Anthropic and OpenAI do the identical thing with the raw error:
```rust
error!("Anthropic connection test failed: {e}");
Ok(ProviderStatus::Error(e.to_string()))
```
A grep confirms `redact_text` has exactly one caller in the whole `provider/` tree (google.rs:352). The
stated invariant is implemented for one third of the surface it was written for.

**Failure scenario:** Whatever `reqwest` decides to include in an `Error`'s `Display` for the Anthropic or
OpenAI endpoint goes verbatim into `ProviderStatus::Error`, over IPC, into the renderer, and into whatever
the renderer does with it (toast, log, bug report). Today that is the request URL, which for these two
adapters carries no key — so **this is a latent hole, not an active key leak, and I am saying so rather
than inflating it.** It stops being latent the moment a proxy URL with embedded credentials, a redirect
target, or a vendor-specific error detail enters that string. The `error!` half is partly covered by
`src-tauri/src/logging.rs:40`, which redacts at the log sink; the IPC return value has no such backstop.

**Why it survives today:** The redaction is invisible when it is not needed, and the two unredacted
adapters produce a perfectly innocuous string on every error anyone has actually seen. Nothing asserts the
invariant across adapters — `tests/redaction_boundaries.rs` exists but the provider layer is not among its
subjects.

**Suggested fix:** Redact in all three `test_connection` error arms (better: put it in one shared helper
that every adapter's error path calls, so a fourth adapter cannot forget), and add a conformance-suite
assertion that no adapter returns a raw `reqwest::Error` string across the boundary.

## D-7 · An unpriced model still records $0 and runs uncapped — the catalogue gate covers only one of five ways a model is chosen
**Severity:** HIGH
**File:** `autodev-core/src/agent/engine.rs:665-671` and `autodev-core/src/recipes/runner.rs:443-450`
(the `Err => 0.0`); validation present only at `src-tauri/src/commands/settings_commands.rs:124`
**The defect:** `cost::pricing` correctly refuses to invent a price, and `catalogue.rs:315` gates that
every **catalogued** model is priced. But the enforcement point converts that refusal straight back into a
zero:
```rust
let cost_usd = match CostTracker::calculate(model, &total_usage) {
    Ok(c) => c.cost_usd,
    Err(e) => { error!("Cost unavailable for model '{model}': {e}"); 0.0 }
};
state.add_cost(cost_usd, ...);
```
so an unpriced model bills real money to the vendor and adds **$0.00** to `cost_today_usd` — invisible to
the daily ceiling, exactly the failure `pricing.rs`'s header names as the reason `gpt-5.5-pro` and
`gpt-5.3-codex` were removed from the product. The catalogue gate only holds if every model that reaches
these lines came from the catalogue. A repo-wide grep shows `catalogue::entry(...)` is called in exactly
**one** place outside `catalogue.rs` itself — `hydrate_active_provider` (settings_commands.rs:124). Four
other paths choose a model with no check at all:

| Path | Source of the model string | Validated? |
|---|---|---|
| `set_active_provider` (settings_commands.rs:81-102) | IPC argument, persisted immediately | **no** |
| `hydrate_active_provider` (:114-141) | the same settings row, at startup | yes |
| `worker.model` (recipes/runner.rs:173-177, priority 1) | workers DB column | **no** |
| `recipe.model` (runner.rs:182, recipes/types.rs:55) | user-authored YAML | **no** |
| subagent config model (subagent/manager.rs:88) | subagent config | **no** |

The asymmetry between rows 1 and 2 is the tell: the *reader* of the setting validates and the *writer*
does not, so a bad value is used for the whole session and only discarded on the next launch.

**Failure scenario:** A worker row (or a recipe YAML) names `gpt-5.5-pro` — a model OpenAI still serves and
that AutoDev deliberately stopped offering *because* it is unpriced. `runner.rs` picks `worker.model` at
priority 1, `factory::build("openai", key)` builds it, the call succeeds and is billed by OpenAI.
`CostTracker::calculate` errors, `cost_usd = 0.0`, `add_cost(0.0, …)`. The worker ticks every N minutes,
all day, and `cost_today_usd` never leaves $0.00 — the daily ceiling never fires, no matter how it is
configured. The only trace is one `error!` line per tick.

**Why it survives today:** The catalogue gate is real and passes, so the invariant *looks* enforced; the
gate proves "every model in `CATALOGUE` is priced", which is a different statement from "every model that
reaches `add_cost` is priced". The `error!` line is honest but nothing acts on it, and the run continues
normally, producing correct output at an apparently free price.

**Suggested fix:** Make an unpriced model refuse the call rather than bill zero — the clean place is
`CappedProvider::check()` or `factory::build`, both of which see every call: if `!pricing::is_priced(model)`
and the daily cap is enabled, return `Err` instead of proceeding. At minimum, validate against
`catalogue::entry` in `set_active_provider`, in the worker-model setter, and when a recipe is loaded, so
the four unvalidated rows above become one enforced rule.

## D-8 · A network error raises a false "caching is broken" alarm, because the monitor is consulted before the stream result is checked
**Severity:** MEDIUM
**File:** `autodev-core/src/agent/engine.rs:403-411` (the call) vs `:485-512` (where the error is finally handled)
**The defect:** The turn runs, then `cache_monitor.observe(...)` is called with
`total_usage.delta_since(&usage_before_turn)` — **and only afterwards**, at `:485`, is `stream_result`
inspected and the error returned. When the stream failed there is no usage, so the delta is all zeros, so
`usage.cached_read == 0`. If that prefix has been seen before in this run, `observe` classifies it as
`SilentMiss`, increments `alarms`, and emits the module's loudest message: *"CACHE MISS on a repeated
prefix … The prefix is being invalidated between calls — every request is paying full price plus the cache
write surcharge."* No such request was ever billed. The same happens for a turn that returns zero usage for
the P2-52 reason (a renamed vendor field), where the cache is not the problem either.

**Failure scenario:** Turn 1 of a run succeeds and registers the prefix (`FirstSighting`). Turn 2's stream
dies on a 529 or a dropped connection. `observe` runs on a zero usage delta, sees `seen_before == 1`,
raises the alarm and logs the invalidation message. `alarm_count()` — which `monitor.rs:147-149` says "`S2`
and the readiness matrix read; a non-zero count after a caching smoke means caching is broken regardless of
what the code looks like" — now reports 1. A transient network fault has been recorded as a caching defect
in the readiness evidence.

**Why it survives today:** The alarm fires into the log during a turn that is *also* about to report an
error, so the two look related. The monitor's own unit tests all pass a well-formed `Usage`, so the
zero-usage-because-the-call-failed case is never constructed. It contradicts the module's stated design
principle at `monitor.rs:26-28` — *"An alarm that fires when nothing is wrong gets muted, and a muted
alarm is worse than none."*

**Suggested fix:** Move the `observe` call inside the `Ok` arm of the `stream_result` match (or guard it
with `if stream_result.is_ok()`), so a request that never completed is never judged. Optionally add a
`Usage::is_empty()` guard in `observe` itself returning a new `NoUsage` verdict, so a caller cannot make
this mistake again.

## D-9 · Anthropic streaming sums the input side from two frames; two in-repo comments contradict each other about whether the vendor sends it twice
**Severity:** MEDIUM (uncertain — see below)
**File:** `autodev-core/src/provider/streaming.rs:116-127` vs `autodev-core/src/provider/anthropic_tests.rs:24-28`; accumulation at `agent/engine.rs:819-823`
**The defect:** `parse_anthropic_event` reads `input_tokens`, `cache_read_input_tokens` and
`cache_creation_input_tokens` from **both** `message_start` (`:44-60`) and `message_delta` (`:116-127`),
and the engine **adds** every `MessageDelta` chunk into the running total (`total_usage.add(&usage)`).
Addition makes "the vendor sends it in one frame or the other" and "the vendor sends it in both" produce
answers that differ by exactly 2× — and the repo states both, in the same feature:

- `streaming.rs:112-115`: *"`input` is read here as well as in `message_start` because the vendor may
  report it in either frame; the engine accumulates, so a field absent from one frame contributes zero."*
- `anthropic_tests.rs:26-28`: *"It is the **ONLY** frame carrying `input_tokens`,
  `cache_read_input_tokens` and `cache_creation_input_tokens` — `message_delta` reports output only."*

Exactly one of these is true, and the fixture suite was written from the second, so it cannot discriminate:
`anthropic_tests.rs:121` and `:197` both encode `message_delta` with `"usage":{"output_tokens":42}` and no
input fields. A test written from the same assumption as the code passes while documenting the bug.

**Failure scenario (if the code comment is the correct one):** every Anthropic streaming call — which is
every agent turn — counts `input`, `cached_read` and `cache_write` twice. `total_input()` doubles, so
`cost_usd` roughly doubles on the input side, so `add_cost` writes roughly 2× the real spend and the daily
ceiling fires at about half the configured limit. The user sees AutoDev report $50 spent on a day
Anthropic's console shows $28. This over-reports rather than under-reports, so it is not a spend *hole* —
but it is a wrong number that a ceiling enforces on, which `pricing.rs`'s own header calls the worse kind:
*"A wrong cost number is worse than no cost number, because it is believed."*

**Why it survives today:** The fixtures encode the assumption, so the suite is green either way; the live
smoke exercises the path but (as far as I can see) asserts no absolute token figure against the vendor's
own reporting. Doubling is not obviously wrong from the inside — the numbers are plausible, monotonic and
internally consistent.

**Uncertainty, stated plainly:** I could not settle this from the repository, and I did not run the live
smoke (it spends real money). I did check the bundled Anthropic API reference, whose raw-SSE examples show
`message_delta` as `{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":12}}`
— **output only**, siding with the test comment and against the code comment. Those examples may be
abridged, so this is evidence, not proof. **What would settle it:** capture one real `message_delta` frame
from an Anthropic streaming call with a cached prefix (the existing live smoke already makes such a call —
logging the raw frame costs nothing extra) and check whether it carries the three input fields.

**Suggested fix:** Regardless of which way it resolves, `add` is the wrong combinator for a
cumulative-per-message field: it is correct for neither "one frame" nor "both frames" in general. Take the
input-side fields with a last-non-zero-wins or `max` rule (they are per-message totals, not deltas), keep
`output` additive, and then reconcile the two comments so the repo states one wire format.

## D-10 · Google key validation calls a model the repo's own test asserts is retired, and it is a billed completion exempted from the ceiling on the grounds that it is free
**Severity:** HIGH
**File:** `autodev-core/src/provider/google.rs:314-336`; claims at `src-tauri/src/commands/vault_commands.rs:132-134` and `autodev-core/src/provider/capped.rs:129-135`; retirement asserted at `autodev-core/src/provider/catalogue.rs:346-370`
**The defect:** Three things go wrong together in Google's `test_connection`.

1. **It calls a retired model.** The URL is hardcoded to
   `.../v1beta/models/gemini-2.5-flash:generateContent`. `catalogue.rs:358` lists `gemini-2.5-flash` in
   `the_retired_generation_is_gone`, whose own comment says *"Selecting one is a call that cannot succeed,
   not a degraded choice."* The repository asserts in one test that this id is dead and calls it in the
   key-validation path.
2. **It is a completion, not a models list.** It POSTs
   `{"contents":[{"role":"user","parts":[{"text":"Hi"}]}]}` — a real, billed inference request. But
   `vault_commands.rs:132-134` states: *"The validation call is `test_connection`, which each adapter
   implements as its cheapest authenticated request — **a models list, not a completion**. It costs
   nothing."* True for Anthropic and OpenAI (both `GET /v1/models`); false for Google.
3. **That false claim is load-bearing for the spend ceiling.** `capped.rs:129-135` exempts
   `test_connection` from the cap **because** *"Validating a key is a free control-plane call that spends
   nothing."* So Google's validation is a billed LLM call that is deliberately exempt from the daily
   ceiling, and its cost is recorded nowhere — no `add_cost`, no analytics. `gemini-2.5-flash` is also
   absent from `PRICES`, so it could not be priced even if it were metered.

**Failure scenario (the live one):** A user pastes a valid Google API key. `set_provider_key` builds the
provider and calls `test_connection`. Google returns 404 for the retired id. The match at `google.rs:341`
falls to `Ok(r) => ProviderStatus::Error("HTTP 404")`, and `vault_commands.rs:151-155` refuses the write:
*"That key was not accepted by google, so it has NOT been saved."* **A correct key is rejected and the user
cannot configure Google at all** — the primary provider for the first rollout, per `models.ts`. The 403
arm (`:340`) only catches genuinely invalid keys, so every outcome here reads as the user's fault.
**Failure scenario (the spend one):** if the id is still alive, each validation is an unmetered, uncapped
billed call; a user re-testing a key repeatedly spends money that no counter sees.

**Why it survives today:** Only a live call against Google can distinguish "404 because the model is
retired" from "the key is bad", and the fixture suite cannot make that call. The cost is a few tokens, so
even when it does succeed nothing looks wrong. The two claims that make it dangerous (`vault_commands.rs`
and `capped.rs`) are prose, and prose is not gated.

**Suggested fix:** Change Google's `test_connection` to `GET {base}/v1beta/models` with the
`x-goog-api-key` header — an actual control-plane call, free and version-stable, matching what the other
two adapters do and what both comments already claim. Then add a conformance assertion that no adapter's
`test_connection` issues a generation request, so the `capped.rs` exemption stays true.

## D-11 · `set_provider_key` returns the raw transport error to the renderer, giving D-6 a live conduit
**Severity:** LOW (as an amplifier of D-6, not an independent defect)
**File:** `src-tauri/src/commands/vault_commands.rs:151-155`
**The defect:** The refusal message interpolates `{why}` — the `ProviderStatus::Error` string — straight
into an `AutoDevError::Vault` that Tauri serialises to the frontend. For Anthropic and OpenAI that string
is an unredacted `reqwest::Error` (see D-6). This is the concrete path by which the unredacted transport
error reaches the IPC boundary that `google.rs:346-351` names as the reason redaction exists. It is listed
separately only because fixing D-6 fixes this; fixing this without D-6 leaves the log half open.

**Failure scenario:** A user behind a proxy pastes a key; the transport error carries whatever `reqwest`
puts in it, and it is rendered in a Settings error toast the user may screenshot into a bug report.

**Why it survives today:** Same as D-6 — today's string is innocuous.

**Suggested fix:** Covered by D-6's shared redaction helper.

## D-12 · The stable/volatile cache split is fully built and never used — the engine calls the joining wrapper, putting the volatile memory block back inside the cached region
**Severity:** HIGH
**File:** `autodev-core/src/agent/engine.rs:133-156`; the split it bypasses is `autodev-core/src/agent/context.rs:78-142`; the consumer it starves is `autodev-core/src/cache/prefix.rs:74-116`
**The defect:** P2-14's whole point — stated in `prefix.rs:19-29` and again in `context.rs:131-134` — is that
the memory block *"is rebuilt from `get_recent_summaries` and `recall_facts` and therefore changes whenever
the agent learns anything"*, so it must sit **after** the cache breakpoint. Both halves of that fix are
implemented correctly:

- `context.rs:78-142` `build_system_prompt_parts` returns `(stable, volatile)` with memory in `volatile`.
- `cache/prefix.rs:74-116` `build(tools, system_base, instructions, volatile_blocks)` keeps the two apart
  and hashes only `stable`.

They are never connected. `context.rs:54-61` `build_system_prompt_with_mode` takes the split pair and
**rejoins it into one string**, and that joined wrapper is what the engine calls:
```rust
let system_prompt = context_builder.build_system_prompt_with_mode(self.pa_mode, working_dir);  // :136 — joined
let cache_prefix = crate::cache::prefix::build(
    &tool_defs..., &system_prompt, &[], &[],                                                    // :148-156
);                                          //  ^ whole prompt as system_base, both extras empty
```
So the memory block travels inside `system_base`, lands in `stable`, and is hashed as part of the cached
prefix. Confirmed by grep: `build_system_prompt_parts` has exactly one caller (the wrapper that undoes it);
`prefix::build` has two callers and **both** pass `&[], &[]`; `CachePrefix::volatile` is read nowhere
outside a unit test (`prefix.rs:232`). The split is built, documented, unit-tested, and unreachable.

**Failure scenario:** A Mode 1 session on Anthropic with memory enabled. Turn 1 writes the whole system
prompt — base + AUTODEV.md + memory — into the cached region and pays the cache-write surcharge. The agent
learns one fact, so `build_memory_context` returns different bytes. Turn 2's system block differs, the
prefix does not match, nothing is read from cache, and the write surcharge is paid again. Repeat every
turn. The result is the `P2-64` failure the monitor exists to name: *"caching COST more than it saves —
each write is billed above the input rate and never read back"* — arising not from a per-run identifier but
from the one fix that was supposed to prevent it not being wired in.

**Why it survives today:** Everything looks right in isolation, and the test suite actively reassures.
`cargo test -p autodev-core --lib cache::` passes 17/17, and one of them is
`cache::prefix::tests::volatile_content_is_kept_out_of_the_hashed_region` — a green test proving the
function honours a split that no production caller ever asks it to perform. A grep across
`autodev-core/tests/*.rs` and `src-tauri/src/ci_gate.rs` for `volatile`, `prefix::build`, `cache_prefix`
or `build_system_prompt` returns **nothing**: no gate asserts the wiring.
`context.rs`'s tests prove the parts are correct; the engine compiles and runs; `will_cache` returns true
because the joined prompt is comfortably over the minimum; `cache_control` appears in the request exactly
as review would expect. The only symptom is the bill — which is precisely the condition `monitor.rs:5-15`
opens by describing. The monitor *would* catch it (repeated `SilentMiss`), but D-8 means network errors
also produce `SilentMiss`, so the true signal arrives mixed with false ones.

**Suggested fix:** Have the engine call `build_system_prompt_parts` and pass the halves through:
`prefix::build(&tools, &stable, &[], &[volatile])`, then build the request so the breakpoint falls between
them (Anthropic: two `system` blocks, `cache_control` on the first). Then delete or `#[deprecated]` the
joining wrapper for the caching path so the next caller cannot repeat the mistake — the same reasoning that
justified deleting `build_system_prompt` in P2-62.

## D-13 · `CachePolicy::Explicit::breakpoints` is constructed and never read by any adapter
**Severity:** LOW
**File:** `autodev-core/src/provider/usage.rs:167-170` (the field), `autodev-core/src/agent/engine.rs:168` (the only production construction), `autodev-core/src/provider/anthropic.rs:170` (where it is discarded)
**The defect:** The Anthropic adapter destructures the policy as
`if let CachePolicy::Explicit { ref ttl, .. } = req.cache` — `breakpoints` goes into the `..` and is
dropped. It then unconditionally attaches `cache_control` to the single system block. A grep shows
`breakpoints` is constructed in five places (one production, four tests) and read in none. The engine's
comment at `:164-168` reasons carefully about *"One breakpoint, at the end of the system block. The
four-breakpoint budget is worth spending only where there are distinct regions"* — a decision that has no
effect, because the value is not consulted.

**Failure scenario:** Not a live failure today: with one system block, "attach `cache_control` to it" and
"put a breakpoint at index 0" coincide. It becomes one the moment D-12 is fixed — the fix needs two system
blocks with the breakpoint between them, and the field that is supposed to say where it goes is ignored, so
the adapter would cache the wrong region while the policy said otherwise.

**Why it survives today:** The two behaviours are indistinguishable in the one-block case that is all the
code currently produces, and the conformance tests construct `breakpoints: vec![0]` without asserting
anything about where the resulting `cache_control` lands.

**Suggested fix:** Either honour `breakpoints` in the adapter (place `cache_control` on the indicated
blocks) or delete the field and let `Explicit` carry only `ttl`. Do this before D-12, since D-12's fix
depends on the answer.

## D-14 · `analyze_diff_tool` pairs a defaulted provider with an independently-read model, with no consistency check
**Severity:** LOW
**File:** `autodev-core/src/agent/builtin_tools/analyze_diff_tool.rs:246-270`
**The defect:** `provider_id` is read from `active_provider` and **defaults to `"anthropic"`** when absent;
`model` is read from `active_model` and filtered only by `catalogue::entry(m).is_some()` — never by whether
that model is served by `provider_id`. `hydrate_active_provider` (settings_commands.rs:131-136) performs
exactly this consistency check and refuses the pair when it fails; this call site does not. The two
settings rows are also written by two non-atomic `app_settings::set` calls
(settings_commands.rs:95-96), so they can genuinely diverge.

**Failure scenario:** `active_model` is `gemini-3.7-flash` and `active_provider` is missing or stale.
`provider_id` defaults to `"anthropic"`, the model passes the catalogue filter, and the tool builds an
Anthropic provider and sends it a Gemini model id — a 404 that reads as an outage. This is the same defect
class the file's own header comment describes having just fixed ("it named a model that does not exist.
Every call through this tool failed at the provider with a 404 that looked like an outage").

**Why it survives today:** Requires the two settings rows to disagree, which the normal write path avoids.

**Suggested fix:** Derive the provider from the model via `routing::provider_for_model(&model)` rather than
reading and defaulting it separately — the model is the more specific fact and routing already owns the
mapping.

---

## Checked and found sound (no defect filed)

Stated so the absence is a finding rather than a gap in coverage:

- **`DailyCap` arithmetic** (`budget/daily.rs`) — `>=` at the boundary, `NaN`/`inf`/`<=0` refused at
  `save`, off by default, `fraction_used` clamped. The tests exercise the boundary in both directions.
  No defect.
- **Day rollover** (`state/app_state.rs`) — local date, rolls on *any* date inequality (so a backwards
  clock rolls rather than merging days), `cost_today()` rolls before every read, `hydrate_today` discards
  a stale day and filters a non-finite/negative stored figure. `main.rs:124,140` publishes state and
  hydrates before anything can call a provider. No defect.
- **`Usage` normalisation** (`provider/usage.rs`) — `from_inclusive_input` saturates, `total_input()`
  is consistent across vendors, `delta_since` saturates. Sound *given* the wire format — which is the
  open question in D-9, filed there rather than here.
- **`CacheMonitor` concurrency** — two independent `Mutex`es with no cross-lock invariant (no deadlock
  path), and every lock uses `unwrap_or_else(|e| e.into_inner())` so a poisoned lock degrades rather
  than panicking. Sound. (Note `CappedProvider::check` at `capped.rs:71` uses a bare
  `.lock().unwrap()` instead — inconsistent, but a poisoned `AppState` lock is a process-wide problem
  either way, so not filed.)
- **`assemble_in_wire_order`** (`engine.rs:915`) — orders by block index via `BTreeMap`, and the
  tool-result loop at `:584-607` walks `tool_calls` order with leftovers reported loudly. Correct.
- **IPC secret asymmetry** (`vault_commands.rs`) — no command returns key material, and
  `no_command_returns_key_material` is a genuine check that can fail (it scans return types across
  every command file and asserts a closed command set). The `masked_last4` helper is char-safe. Sound.
- **The price table** — exact-match lookup, no substring fallthrough, unknown model is an error,
  four-way token split, the tier applies to cached reads as well as input, and
  `a_scheduled_price_change_has_not_silently_passed` is a real time-bomb guard. The one gap is what
  happens *after* the error reaches the caller, which is D-7. (Minor, unfiled: `cache_write` is not
  tiered; harmless today because the only tiered model has `cache_write: 0.0`.)
- **`airsctl`** does not construct providers in-process — it posts to the `core-orchestrator` Python
  sidecar — so `wrap_global`'s "unwrapped when no global state" concession is not reachable from a
  second Rust binary. Worth noting separately that work sent to that sidecar is outside the Rust daily
  ceiling entirely, but sidecar removal is already a tracked constraint, so no defect is filed here.

**Severities with no findings:** none — every severity band from LOW to CRITICAL has at least one entry.

---

## Method and limits

- Read-only review; no source file was modified.
- Ran `cargo test -p autodev-core --lib cache::` once (17 passed) to check the D-12 claim against
  execution rather than reading. Did **not** run `cargo test --workspace`: the full run includes
  integration tests whose blast radius against `~/.autodev/` and the local databases I could not
  establish from the tree, and the prohibition on touching those was the more specific instruction.
  Nothing in this report rests on a workspace-wide run.
- No live-smoke, no keychain access, no `~/.autodev/` writes, no API key set, no migration or
  database-reset tests.
- **D-9 is the one finding I could not settle.** It is filed with its uncertainty stated and with the
  single cheap experiment that would resolve it.
