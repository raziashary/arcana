# 2026-10-18 — ADR-LOG-RESTRUCT-0399 · provider `ErrorClass` carried as data

**Task.** Close the owner-approved escalation "carry `ErrorClass` structurally instead of
parsing it back out of the error string", which is the same defect the expanded P4 goal names
in §25 ("prefer structured contracts over parsing human-readable strings").

**Prior finding this closes.** `_verify/A9-P3-claims-refutation.md` **F-13** (`severity: high`)
recorded `P3-09` as **FALSE**, with two defects: the class was re-derived by string-matching
the message, and the code comment directly above asserted the opposite of the code three lines
below it. It also recorded a second, unaddressed defect: a **transport** failure carried no
class tag, so a dropped connection — the commonest retryable failure — was classified `Fatal`
and never retried.

## What changed

| Layer | Before | After |
|---|---|---|
| `errors.rs` | `Provider(String)` only | `+ ProviderClassified { class: ErrorClass, message: String }`, `Display` emits the tag via `{class:?}` so the rendered spelling is unchanged; `Provider(String)` kept for unclassified errors |
| adapters (`anthropic`, `google`, `groq`, `openai`) | `Provider(format!("X {status} [{class:?}]: {summary}"))` | `ProviderClassified { class, message: format!("X {status}: {summary}") }` — single carrier, no tag duplicated into text |
| `usage.rs` | `safe_transport_error` / `safe_decode_error` flattened the class into the message | carry `Retryable` / `Fatal` as data |
| `agent/engine.rs` retry closure | `let text = e.to_string(); text.contains("[QuotaExhausted]") …` under a comment saying it did not | `match e { ProviderClassified { class, .. } => *class, _ => Fatal }`; comment corrected |
| `recipes/runner.rs::startup_error_class` | same `contains("[…]")` chain | same `match`, same returned label strings |
| `src-tauri/src/ci_gate.rs` `TG-P2-06` | loop over the literal `["anthropic.rs","openai.rs","google.rs"]` | adapters enumerated from `arcana-core/src/provider/` by the presence of `if !status.is_success()`; asserts ≥ 4 discovered |

The tag stays in the rendered string **as display only**: `ArcanaError` serialises to a string
across the Tauri IPC boundary, where the type cannot survive that boundary. The standing rule
recorded in ADR-0399 is that no Rust consumer may parse it.

## Evidence

| Claim | Command | Result |
|---|---|---|
| Compiles | `cargo check -p arcana-core -p arcana --locked` | Finished, no errors |
| Provider suite | `cargo test -p arcana-core --lib --locked -- --test-threads=1 provider::` | **154 passed**, 0 failed (153 before + the new closure test) |
| Engine suite | `… --test-threads=1 agent::engine` | 31 passed, 0 failed |
| Fleet classifier suite | `… --test-threads=1 recipes::runner` | 29 passed, 0 failed |
| Gate green | `cargo test -p arcana --lib --locked -- tg_p2_06` | 1 passed |
| **Gate can fail** | injected the old `[class:?]`-in-message shape into `groq.rs`, ran `tg_p2_06` | **FAILED**, naming `groq.rs async fn complete(` — then reverted, 1 passed. `groq.rs` was never covered by the old literal list. |
| Numeric gates still green | `… -- tg_p2_05` | 1 passed |
| No text parser remains | `Select-String -Pattern 'contains\("\[(QuotaExhausted\|Overloaded\|Retryable)'` across `arcana-core`, `src-tauri`, `src-cli` | only test assertions on display text, plus the new test |
| Formatting | `cargo fmt --all -- --check` | clean except the owner's untracked WIP `process_sandbox/linux_smoke/staging.rs` (not touched) |

## Limits (stated, not glossed)

- The class is **absent** across the Tauri IPC boundary by design; the frontend receives the
  rendered string. Nothing in the frontend classifies provider errors, so this is not a gap in
  behaviour — but it is why the tag was kept in the text rather than deleted outright.
- One pre-existing gap is **recorded, not fixed**: two other `ci_gate.rs` loops
  (`:1128`, `:3845`) still use the hardcoded three-adapter literal. They belong to other gates,
  and changing a gate whose contract has not been read would be guessing; they are recorded as
  wave-1 workstream W1 items in `../AUDIT_2026-10-18_CODEBASE_REFACTOR_PLAN.md`.
- `tests/live_provider_smoke.rs` is `#![cfg(feature = "live-smoke")]` and was **not** compiled
  or run (it makes real billable calls). Its one reference to the old shape was type-checked by
  compiling a copy with the `cfg` line stripped, then deleted; the real file is compiled in CI's
  `--all-targets` run, where the feature is off.
