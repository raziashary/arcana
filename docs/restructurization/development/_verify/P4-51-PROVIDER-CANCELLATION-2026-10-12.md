# P4-51 Provider Cancellation — 2026-10-12

## Scope and proof target

Implement and prove the owner-approved cancellation path around provider token preflight, request/retry establishment, and stream-body waits. The proof must show cancellation prevents a later generation/tool dispatch and promptly drops an in-flight provider future. A cancelled remote request may already have incurred billing; this work does not claim remote cancellation. Usage already reported remains accounted. When usage is unknown, run-token and daily spend reservations remain conservatively charged.

Owned files: `arcana-core/src/agent/engine.rs` and this report only.

## Source grounding

- `AgentEngine::run` checks the existing `AppState` cancellation flag before starting and between turns, but previously awaited provider stream handling as one operation.
- `stream_response` owns preflight, stream establishment/retry, and stream chunk polling.
- `RunTokenBudgetProvider` asks for the token upper bound before dispatch, reserves before generation, and charges the conservative maximum when a dispatched call has unknown usage or its stream is dropped.
- `CappedProvider` holds a daily reservation around generation. A dropped active stream settles unknown usage at the reservation maximum.
- `AppState::cancel_agent` sets a session flag; the existing executor cancellation wait polls it every 100 ms.

## Progress

- [x] Created this report before code edits.
- [x] Read the exact run-loop, stream/retry, AppState, budget, and existing cancellation test bodies.
- [x] Implemented cancellation checks around provider preflight/establishment/retry and stream-body waits in `engine.rs`.
- [x] Added scripted local provider regressions for preflight, pending establishment, retry delay, and pending body cancellation.
- [x] Ran only the authorized focused cancellation filter with credential/database variables explicitly removed.
- [x] Recorded results, reservation/accounting observations, test limits, and cleanup evidence below.

## Implementation

`stream_response` races the existing session-cancel flag against the complete retry/establishment future and each provider body read. The poll cadence follows the existing 100 ms executor pattern. Cancellation sets the provider-call and stream-span outcome to `cancelled`, returns `ArcanaError::Cancelled`, and bypasses provider-failure recording. The existing run loop then emits its cancellation terminal result and checks cancellation before any assembled tool call can execute.

The scripted preflight test proves a cancelled token-count bound never reaches generation and creates no reservation. Establishment, retry-delay, and body tests prove the engine drops the pending provider future or stream and does not make a later attempt. The body test registers a side-effecting tool, streams only an incomplete tool call, then blocks; cancellation returns before the tool handler can start. It wraps the production `CappedProvider`, reads the isolated config database, and proves unknown usage is settled as conservative daily spend with no open reservation. Run-token ledger readback proves the 25-token input bound plus 32-token output cap (57 total) remains charged after an unknown post-dispatch outcome. The existing cancellation-during-stream test still proves already reported usage (120 input / 18 output) is persisted; its fixture now signals cancellation when the first chunk is consumed so the reported frame remains observable.

## Verification

Command, after explicitly removing `GEMINI_API_KEY`, `GOOGLE_API_KEY`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, and `DATABASE_URL` from the PowerShell process environment:

```powershell
cargo test --manifest-path Cargo.toml -p arcana-core --lib cancellation_during_provider_ -- --nocapture --test-threads=1
```

Result: **5 passed, 0 failed, 0 ignored**. The filter covered all four new tests plus the existing `cancellation_during_provider_stream_is_terminal_and_prevents_tool_side_effects` regression. Cargo compiled the Windows `arcana-core` test target successfully; 47 pre-existing crate warnings were emitted. `rustfmt --edition 2021 arcana-core/src/agent/engine.rs` and `git diff --check` passed.

The first test draft awaited the fixture notification without polling the engine future; the focused run exposed this harness defect. The fixture now runs the engine task on the current-thread test runtime while the isolated-home guard stays on the test thread, and the corrected filter passes.

## Limits and cleanup

This proves cancellation through the production `AgentEngine` against deterministic local scripted provider futures, not against a live vendor SDK or remote server. Dropping local request/stream futures does not guarantee the vendor stopped processing or billing an already dispatched request. Cancellation during token-count preflight occurs before generation reservation/dispatch; post-dispatch usage remains conservatively charged unless the provider has reported usage. Tests use isolated temporary Arcana homes and no credentials, provider spend, external network, or loopback listeners; no ports or test servers were started.

## Fresh failure-boundary review and follow-up

The independent review `P4-51-FAILURE-BOUNDARY-REVIEW-2026-10-12.md` found two gaps after the first focused proof: budget decorators settled on the first nonzero `MessageDelta`, so an input-only Anthropic frame released the unknown-output reserve before cancellation; and the stream-first biased select could starve cancellation while chunks stayed ready. A later accounting review found that raw EOF after an input-only Anthropic frame still looked complete. Adapter inspection confirms Anthropic splits input (`message_start`) and output (`message_delta`), OpenAI may send its usage-only frame after a finish-reason frame, and Google reports normalized usage with its finish-reason frame then ends by EOF. Therefore decorators settle on explicit `Done`, or at EOF only if a recognized terminal stop reason was observed. Bare EOF, unknown stop reason, error, or drop retains the conservative maximum. Ownership is extended to `provider/run_budget.rs`, `provider/capped.rs`, and their existing tests, alongside `agent/engine.rs` and this report.

- [x] Keep both reservations until `Done` or EOF after a recognized terminal stop reason; on completion reconcile aggregate reported usage, and on cancellation/error/drop/bare EOF retain the full bound.
- [x] Prioritize cancellation over ready body chunks and prove a continuously-ready stream stops after cancellation is signaled.
- [x] Add input-only usage → pending → cancellation readback for run-token and daily ledgers, plus successful split-usage completion reconciliation.
- [x] Run focused cancellation and decorator filters with credential/database variables explicitly removed; update final evidence and limits.

### Follow-up implementation and partial verification

Both production decorators now aggregate normalized `Usage` across message frames and reconcile only when they observe explicit `Done`, or EOF after `StopReason::parse` recognizes a terminal reason. An error, dropped stream, bare EOF, or unknown stop reason settles at the conservative maximum. This preserves all reported fields for reconciliation on success without treating Anthropic's input-only start frame as final usage. The engine's biased body select now polls cancellation before the next chunk. The deterministic continuously-ready fixture sets the real `AppState` cancel flag on its first body poll and asserts the engine drops the stream after exactly one poll; the shared cancellation helper measures signal-to-return latency and enforces a 250 ms ceiling against the existing 100 ms polling interval.

The exact run-token decorator filter passed **9 passed, 0 failed, 0 ignored** after tightening EOF settlement:

```powershell
cargo test --manifest-path Cargo.toml -p arcana-core --lib provider::run_budget::tests:: -- --nocapture --test-threads=1
```

The command explicitly removed `GEMINI_API_KEY`, `GOOGLE_API_KEY`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, and `DATABASE_URL` from the process environment. The input-only cancellation case read back 70 conservatively charged tokens. Bare input-only EOF and unknown-nonempty-reason EOF both also settled the full 70-token reservation. The successful split-usage cases kept 70 reserved/0 used until explicit `Done` or recognized-reason EOF, then settled 21 used/0 reserved.

The exact daily-cap decorator filter passed **9 passed, 0 failed, 0 ignored** after tightening EOF settlement:

```powershell
cargo test --manifest-path Cargo.toml -p arcana-core --lib provider::capped::tests:: -- --nocapture --test-threads=1
```

Its cancellation case read back one full pending hold and zero spend after input-only usage. After dropping the stream, it read zero open reservations and spend equal to the original full request hold. Its success case read the same hold after input and output frames, then verified `Done` settled the exact rounded cost for 17 input and 4 output tokens.

Both decorator fixtures model the Google-style successful EOF path. They keep the full reservation after split input/output usage, then reconcile to 21 tokens or the exact rounded daily cost at EOF after the recognized `end_turn` reason. Negative cases prove bare input-only EOF and unknown non-empty reason followed by EOF retain the full run-token and daily maximum charges.

The exact engine-module cancellation filter passed **6 passed, 0 failed, 0 ignored**:

```powershell
cargo test --manifest-path Cargo.toml -p arcana-core --lib agent::engine::procedural_memory_tests::cancellation_ -- --nocapture --test-threads=1
```

The four measured cancellation paths returned in 110–143 ms for preflight, establishment, retry delay, and pending body, within the 250 ms assertion against the 100 ms polling cadence. The continuously-ready body returned in about 5 ms and was polled exactly once after cancellation. The existing usage-persistence/tool-prevention test also passed: the engine returned `ArcanaError::Cancelled`, started no tool side effect, made no later provider request, and persisted 120 input plus 18 output tokens already reported by the stream. The select cancellation branch writes `cancelled` to stream and provider-call telemetry and returns before provider-failure recording.

All focused commands explicitly removed `GEMINI_API_KEY`, `GOOGLE_API_KEY`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, and `DATABASE_URL`. One initial compile attempt failed because newly added run-budget tests lacked a `StreamExt` import; after adding it, the focused filter passed. I also mistakenly invoked the broader test-name filter `cancellation_` once; it selected 10 tests across unrelated modules, all passing. I reran and use the scoped engine-module filter above as the engine evidence. No live resource or port was used.

`rustfmt --edition 2021` passed on all three owned Rust files after the EOF cases were added. An earlier `git diff --check` passed before those cases; a final direct trailing-whitespace scan across all owned files also passed. No Git operation was used beyond that earlier check. The focused test processes launched for this unit exited; this work started no servers or ports.

Parent integration: default-feature cargo check --offline -p arcana-core -p arcanactl --locked passed on Windows (43.87s) after the final EOF accounting correction. Independent final source review found no unresolved scoped defect. Linux execution remains unproved for this unit; no remote-billing guarantee or P4 gate closure.
