# P2 pre-build refutation — BYOK and adapter conformance

**Read-only adversarial pass, 2026-09-02.** Dispatched under `§0.2m`. Same dispatch defect as
the other two reports: read-only agent type, so it could not write its own file.

---

## Summary

| Claim | Verdict |
|---|---|
| 1 — parallel tool calls broken on OpenAI and Google | **PARTIALLY CONFIRMED, mis-scoped** |
| 2 — `StopReason::MaxTokens` not distinguishable | **CONFIRMED in effect; the named type does not exist** |
| 3 — keys stored without live validation | **CONFIRMED** |
| 4 — active provider/model dies on restart | **CONFIRMED — never persisted at all** |
| 5 — no Linux encrypted-file fallback, no store disclosure | **CONFIRMED**, with one correction |
| 6 — `ollama.rs` is dead weight, ~506 LOC, 10 sites, 8 files | 🔴 **REFUTED — every number wrong, and it is NOT dead** |

---

## 🔴 The finding that outranks the claims — a live credential leak

**`google.rs` puts an unredacted API key on an IPC surface reaching the renderer.**

Google takes its key in the **URL query string** (`google.rs:178-181`, `205-208`, `259-262`:
`...?key={}`) — the one pattern the privacy rule warns about. Anthropic (`x-api-key` header)
and OpenAI (`Authorization` header) do this correctly.

On a transport failure, `reqwest` 0.12.28's `Error` Display **includes the full URL**. That
string is returned at `google.rs:288` as `Ok(ProviderStatus::Error(e.to_string()))`, crosses
the Tauri boundary via `test_provider_connection`, and is rendered verbatim at
`Settings.tsx:1110`.

**Redaction does not save it, for two independent reasons:**

1. `SENSITIVE_KEYS` (`redaction.rs:88-111`) contains `api_key` / `apikey` / `api-key` but
   **not bare `key`**, so `?key=AIza…` is not matched by `redact_assignments`. The 32-char
   `LONG_TOKEN` rule is deliberately excluded from `redact_text` (`redaction.rs:301-317`).
2. **No redaction is applied on the IPC path at all.**

🔴 **And the console log layer is unredacted.** `logging.rs:109-122` wraps only the **file**
appender in `RedactingWriter`; `console_layer` (`:59-63`) is registered raw at `:121`. So
`google.rs:287` — `error!("Google connection test failed: {e}")` — prints the key-bearing URL
to stderr.

*This is a direct miss in P1's redaction work: the file appender was wrapped and the console
layer was not. Same shape as `A-08a` — one side of a boundary fixed, the other left open.*

---

## Claim 1 — parallel tool calls (PARTIALLY CONFIRMED, mis-scoped)

**Non-streaming: REFUTED for both.** `openai.rs:438-448` iterates every tool call;
`google.rs:386-411` pushes every `functionCall` part. A fix rewriting either
`parse_completion_response` is wasted work.

**OpenAI streaming: OVERSTATED.** Every branch inside the `tool_calls` loop
(`openai.rs:356-385`) returns, so a second element of one array is dropped — but OpenAI
carries a distinct `index` per call, normally one entry per delta, and it is threaded as
`block_index` (`openai.rs:367`) into the engine's correlation map (`engine.rs:459`). Parallel
calls arriving in separate deltas **do** accumulate. The claim would send an implementer to fix
a path that mostly works.

**Google streaming: CONFIRMED, and worse than claimed.** `google.rs:326-350` returns on the
first matching part, and Gemini emits parallel calls as multiple `functionCall` parts of one
candidate. Three compounding defects:

1. `google.rs:341` — `let _args = ...` discards arguments, and **no `ToolInputDelta` is ever
   emitted for Google**. The engine assembles from an empty accumulator (`engine.rs:490-493`),
   `from_str("")` fails, `.unwrap_or` substitutes `{}`. 🔴 **Every Google tool call executes
   with empty input — not just the parallel ones.**
2. `google.rs:347` — `block_index: 0` hardcoded, so two calls collide in `index_to_id`.
3. `google.rs:345` — `id: format!("gemini-{}", name)` collides when a tool is called twice.

**Anthropic single-message sub-claim: REFUTED — the code is correct.** `engine.rs:342-346` is
one `push` of one user message holding every result.

---

## Claim 2 — stop reason (CONFIRMED in effect; type does not exist)

**There is no `StopReason` enum anywhere** — repo-wide grep returns zero. It is
`pub stop_reason: String` (`types.rs:75`). Anyone told to fix "the enum" will find nothing.

The string *is* produced by all three adapters (`anthropic.rs:282`, `openai.rs:420`,
`google.rs:374`) and **destroyed at the consumer**: `engine.rs:474-481` destructures
`MessageDelta { tokens_in, tokens_out, .. }` — the `..` discards it. Loop termination is decided
purely by absence of tool calls (`engine.rs:288-292`), so **a `max_tokens` truncation and a
clean `end_turn` are literally the same code path.** Measured: written by four sites, read by
**zero**.

**(a)** OpenAI streaming can never report a stop reason at all — `openai.rs:388-406` gates
`MessageDelta` behind a `usage` object, and `stream_options`/`include_usage` appear **nowhere
in the repo**. A guard that cannot fire, which also zeroes tokens and therefore cost.

**(b)** `max_tokens` is dead on two of three adapters — read only at `anthropic.rs:134`.
`maxOutputTokens`/`max_output_tokens` appear **nowhere**. The engine's `max_tokens: 8192`
(`engine.rs:210`) applies to Anthropic only.

---

## Claim 3 — no live validation (CONFIRMED)

`vault_commands.rs:89-102` checks non-emptiness and stores. No format check, no provider call.
`settingsStore.ts:110-117` invokes `set_provider_key` then `loadProviders()`, never
`testProvider`.

**The UI is worse than "no validation":** `Settings.tsx:1082-1092` clears the prior test result
**and promotes the unvalidated key to active** (`setActiveProvider`). `test_provider_connection`
exists (`settings_commands.rs:55-84`) and every adapter implements `test_connection`, but it is
reachable only via a separate manual button.

---

## Claim 4 — provider persistence (CONFIRMED — never persisted)

`settings_commands.rs:86-107` writes `active_provider`/`active_model` into process memory only.
`AppState` is `#[derive(Default)]` and `AppState::new()` leaves both `None`. No `app_settings`
read exists at any of the 10 Rust hit sites.

**The silent post-restart behaviour is "you are on Anthropic Sonnet"**, regardless of choice —
`Chat.tsx:268-269`, `SessionBar.tsx:20-21` and `AnnotationModal.tsx:82-83` all fall back to
`?? 'anthropic'` / `?? 'claude-sonnet-4-5'`. `system_prompt_override` has the same defect.
PA mode *is* persisted (`settingsStore.ts:152`), so the machinery exists and was not used here.

---

## Claim 5 — Linux keychain (CONFIRMED, with a correction)

`keyring = "2"` in both manifests, resolving to 2.3.3 with `linux-keyutils`, `secret-service`,
`security-framework`, `windows-sys`. **No encrypted-file backend exists in keyring 2.x**, and
`vault/keychain.rs` is 49 lines with a single unconditional path.

**Correction:** it does **not** fail silently — the keyring error propagates through
`AutoDevError::Vault` to the UI (`Settings.tsx:1093-1102`). The accurate defect is (a) no
fallback and (b) **no backend disclosure**: `VaultEntryStatus` carries `key`, `label`,
`present`, `orphaned` — no backend identity. Meanwhile `docsMore.ts:160` claims keys are
"unlocked per-session with the OS credentials", describing behaviour this code does not
implement.

**Undetermined:** whether keyring 2.3.3 resolves to `secret-service` or `linux-keyutils` here —
the latter is kernel-memory-backed and **does not survive reboot**, a distinct failure mode.
Settled by `cargo tree -e features -p keyring --target x86_64-unknown-linux-gnu`.

---

## Claim 6 — Ollama 🔴 **REFUTED**

**Every number in the plan is wrong, and the framing is wrong.**

| Plan says | Measured |
|---|---|
| ~506 lines | **537** |
| ~10 sites | **31 Rust occurrences** (42 total incl. manifests, frontend, lockfile) |
| 8 files | **10 Rust files** (16 total) |

**It is not dead weight — it is a live, user-facing provider.**
`settings_commands.rs:28-33` enumerates it as `"Ollama (Local)"`; `Settings.tsx:67-68` renders
the list unfiltered; it has a dedicated no-key branch (`settings_commands.rs:57-61`); and
**five** provider-construction switches route to it (`agent_commands.rs:142`,
`diff_commands.rs:43`, `recipe_commands.rs:228`, `analyze_diff_tool.rs:259`,
`native_runner.rs:254`). **Removing it is a user-visible feature removal, not a cleanup.**

**Missed by the plan:** `ollama_rs::` is imported by **one file only**, yet `ollama-rs` is a
direct dependency **twice** — `autodev-core/Cargo.toml:18` (used) and `src-tauri/Cargo.toml:89`
(**unused**; src-tauri reaches Ollama through the `autodev_core` re-export). The src-tauri
declaration is removable today with no code change.

⚠ **The tree already contradicts itself about Ollama**, half of it from 2026-09-01:
`PROBED_PROVIDERS` marks it retired and `is_supported_provider` excludes it, so a stored Ollama
key reports `orphaned: true` — while `settings_commands.rs:32` presents Ollama as a live
provider on the same screen.

---

## Unlisted findings

**Discarded results**
- `google.rs:341` — tool arguments computed and dropped (Claim 1).
- `engine.rs:237` — `let _ = db.update_task_budget_usage(...)` swallows a DB failure on the budget path, in a file that logs loudly for the analogous failures at `:402`, `:548`, `:551`.
- `engine.rs:231` — `if let Ok(ref _r)` binds and ignores.

**Dead parameters / unread config**
- `engine.rs:422` — `_model: &str` never read, passed live at `:223`.
- `openai.rs:138-140` — `.enumerate()` then `filter_map(|(_idx, c)|)`.
- `model_configs` persisted, shown in the UI, applied nowhere — every request site hardcodes the fields (`engine.rs:211-214`, `compaction.rs:117-120`, `classifier.rs:148`, `recipe_commands.rs:266`).

**Guards that cannot fire**
- `openai.rs:391` — `if let Some(usage)` on the stream path; `include_usage` never sent.
- `openai.rs:389` — `finish_reason != "null"` compares against the four-character *string*; JSON `null` yields `None` from `as_str()` and never reaches the line. Inert.

**Adapter conformance**
- **Tool errors invisible on OpenAI and Google** — `openai.rs:97-101` and `google.rs:87-91` destructure `is_error: _`; neither wire format carries it. A failed tool is indistinguishable from a successful one on two of three providers. Anthropic passes it (`anthropic.rs:112`).
- **Google's `functionResponse.name` receives a tool-use id, not a name** (`google.rs:93`), and with `id: format!("gemini-{}", name)` the round trip sends `"gemini-read_file"` where the API expects `"read_file"`.
- **OpenAI drops text sharing a message with tool results** — `openai.rs:94-109`, the `else` producing the text message is skipped.
- **Unrecognised roles vanish on OpenAI** (`openai.rs:85-163`, no `else`); Google coerces them to `"user"` (`google.rs:70-74`). Two adapters, two silent and different behaviours.
- **Tool-call ordering non-deterministic** — `engine.rs:490` iterates a `HashMap`.
- **Tool results reordered relative to their calls** — `engine.rs:320-340` puts unknown-tool errors first.
- 🔴 **Zero test coverage in every adapter.** No `#[test]` of any kind across `anthropic.rs`, `openai.rs`, `google.rs`, `ollama.rs` — **1,733 lines of provider code with no unit test.**

**Vault surface**
- `vault_commands.rs:38` — `build_provider_info` calls `VaultManager::get` per provider on every `list_providers`, decrypting a secret into process memory purely to compute a boolean, then dropping it unzeroized. A presence probe would avoid materialising it.
