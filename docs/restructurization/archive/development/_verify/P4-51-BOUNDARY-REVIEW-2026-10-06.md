# P4-51 Boundary Review — 2026-10-06

Review scope: current uncommitted production diff in `arcana-core/src/agent/engine.rs` and related tests in that file, plus direct tool/provider contracts. Read-only review; no tests, credentials, live data, network, or Git mutations.

## Result

No material correctness, privacy, replay, or tool-call correlation defects found in the reviewed changes.

- Streamed malformed or non-object arguments are detected before execution (`engine.rs:1749-1755`, dispatch check at `1005`); the result is an error with a stable code and executor-built context (`1238-1254`). The helper used for assistant transcript assembly substitutes `{}` for invalid wire input (`2020-2055`), so malformed text is not replayed to providers.
- Error output is fixed and does not include raw arguments. `ToolResult::with_execution_context` sanitizes/bounds fields and attaches the call ID (`tool_trait.rs:183-200`, `290`). The new tests plant private markers and assert they are absent (`engine.rs:2810-2950`, `2960-3046`).
- Provider failures are recorded once on terminal establishment/stream errors, and a retried call is marked resolved only after stream consumption and response assembly (`engine.rs:1501-1535`, `1579-1587`, `1749-1768`). Error classification uses bounded class tags; stored messages contain only the class and attempt count. The tests cover terminal adapter errors and a retry followed by a stream-body failure (`engine.rs:3100-3338`).
- Correlation for malformed tool calls uses the same `<session>-<tool-use-id>` convention as the adjacent unknown-tool path and includes `session_id`, role, optional task, call ID, and run ID through `with_execution_context` (`engine.rs:1229-1254`; `tool_trait.rs:200-290`).

## Non-blocking limitation

The new durable provider-failure rows pass `request_id: None` (`engine.rs:1501-1524`). They therefore identify provider/class/time/retry outcome but cannot be joined back to the originating agent session from this table. The provider API does not expose a request identifier through the reviewed stream contract; I did not treat this as a defect without a requirement to add session-level correlation to analytics.

## Limits

This was a source review only. I did not run tests or inspect unrelated changed files. The conclusions are limited to the scoped diff and the direct contracts listed above.
