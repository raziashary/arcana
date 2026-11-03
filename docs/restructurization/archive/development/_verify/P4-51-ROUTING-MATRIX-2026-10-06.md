# P4-51 Routing Matrix — 2026-10-06

Scope: bounded production `ToolExecutor::execute_batch` coverage for the registered FastDevelopment `suggest_worker` tool. This report is an evidence log; no production code, dependencies, schema, live resources, network calls, or unrelated files are in scope.

## Progress
- Report initialized before investigation.
- Investigation pending.
- Located the live tool in `suggest_worker_tool.rs`; FastDevelopment registration is present in `builtin_tools/mod.rs`. Reviewing its exact routing inputs and the production worker/task APIs before test design.
- Handler contract: `suggest_worker` accepts a positive `task_id`, loads the task from `TasksDb`, delegates selection to `TaskRouter::suggest_worker`, and attaches `primary_repos` from the selected worker roster. Invalid/missing IDs fail before DB access; missing-task failures include the submitted ID and therefore exercise the executor argument-redaction boundary.

## Implemented bounded slice

- Added one `#[cfg(test)]` regression in `arcana-core/src/agent/builtin_tools/suggest_worker_tool.rs`. It registers the actual FastDevelopment tool and calls it through `ToolExecutor::execute_batch`.
- The test creates a task in the temporary home, reopens it through `TasksDb::get_by_id`, marks the seeded `worker-backend` idle and other seeded implementation workers paused through `WorkersDb`, then reopens and reads the persisted roster.
- Three persisted successful outcomes on real prior task IDs deterministically route an unfamiliar feature to the only runnable `worker-backend` using historical scoring. The successful JSON is checked for the database-returned task ID/title/type, route, static method, score bounds, confidence, and `primary_repos` matching the persisted worker row. The task is re-read afterward to prove suggestion did not assign it.
- A malformed `task_id` carries planted task-ID/prompt/source markers. The result asserts `is_error`, bounded versioned JSON, `ok=false`, tool and operation identity, available task/session/agent/call correlation, bounded message/cause/action, retry metadata, timestamp, and omission of each private marker.

## Progress
- Source review and test implementation complete. Formatting and the authorized exact filter remain.
- First authorized filter compiled successfully but the assertion decoding the successful tool output failed at JSON column 17. The fixture itself created and read task/worker rows before the failure. Inspecting the actual output string in the focused test to identify and correct the assertion or surfaced output issue; no other test scope will be run.

- The scoped test is authorized with `cargo test --offline -p arcana-core --lib suggest_worker_live_registry_executor_matrix_uses_persisted_runnable_roster --locked -- --test-threads=1`, after clearing the specified provider/database variables per command.

## Handoff blocker and correction

- The historical-routing variant mentioned in an interim draft was reverted. The test retains the realistic static API/endpoint success assertion; it is not adjusted to avoid the executor behavior below.
- Exact command attempted with `GEMINI_API_KEY`, `GOOGLE_API_KEY`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, and `DATABASE_URL` cleared for the command: `cargo test --offline -p arcana-core --lib suggest_worker_live_registry_executor_matrix_uses_persisted_runnable_roster --locked -- --test-threads=1`.
- Result: compilation succeeded; the focused test failed while parsing the successful executor result. The observed output contains `"confidence":0.[CARD]`. The static f32 confidence serializes with a 16-digit decimal run, and ToolExecutor free-text credit-card redaction replaces that run inside the JSON, yielding `invalid number` at JSON column 17.
- No production change is authorized in this slice. The parent has a shared structured JSON redaction policy question pending owner direction and requested no further routing source edits or test runs until that direction arrives. This blocker is preserved; no task/gate closure or commit is claimed.


## Owner-approved correction and verified consumer, 2026-10-09

ADR0332 supersedes the pending policy choice above. JSON-aware output redaction preserves scalar types, redacts strings and masks sensitive-key values. The unchanged real registered Memory and routing reader assertions now pass on Windows/Linux (each matrix1/1); redaction22/22 and ToolExecutor18/18 also pass on both hosts. See P4-51-JSON-REDACTION-2026-10-09.md for commands and timings. This raises full registry matrix coverage to38/47 without closing P4-51/TG4-25.
