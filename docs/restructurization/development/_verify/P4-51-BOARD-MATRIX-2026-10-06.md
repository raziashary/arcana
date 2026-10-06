# P4-51 / TG4-25 board-tool matrix

Date: 2026-10-06

## Scope and constraints

Test-only success/failure proof for `create_task`, `list_tasks`, `assign_task`, `assign_tasks`, `update_task_status`, and `claim_task` through the real FastDevelopment registry and `ToolExecutor::execute_batch`. No production API/schema/dependency changes. No tests, build, formatting, Git mutation, live data, network, paid calls, or Calendar activity in this task; the coordinator will run the exact filter.

## Progress

- Created this report before investigation as requested.
- Investigation underway: read TG4-25, exact handler/DB APIs, executor and fixture conventions, plus the scheduler boundary for create_task.

## Governing contract and checkout notes

- TG4-25 requires real production `ToolExecutor` success/failure calls, bounded versioned JSON failures returned as `is_error=true`, stable identity/correlation, private-input exclusion, and injected handler/database outcomes. This unit is one board-tool slice only.
- The six requested tools are registered in FastDevelopment in `builtin_tools/mod.rs` under their exact runtime names. `create_task` requests `scheduler::kick("supervisor")` after its durable insert. Its database success/readback can be proved in the temp home; external scheduler installation is outside scope and will not be added.
- Shared checkout already has a modified `send_worker_message.rs`, owned by the concurrent ACP unit/coordinator. It will not be touched.
- Next: read the six tool contracts and existing board fixture helpers, then add only a test module to `create_task.rs`.

## Handler and fixture findings

- `create_task` persists a ready task then calls the existing `scheduler::kick`; `kick` is explicitly a no-op apart from a warning when no global scheduler has been installed. The test will not install or start a scheduler.
- `list_tasks` has optional inputs and no validation failure path, so its failed case will come from an obstruction of only the temporary `tasks.db`, after success/readback.
- `assign_task` and `assign_tasks` require a non-system implementation worker with a runnable roster status (`idle`, `active`, `idle_no_provider`, or `error`) for explicit overrides. Batch validation failures are intentionally represented as per-entry `ok=false` values while the overall ToolResult is success; the test will verify this contract and separately exercise a top-level failed ToolResult.
- A safe worker lifecycle is grounded: create ready tasks, assign singly/in a batch, atomically claim as the assigned worker, then transition the claimed task to `failed`. The test avoids `qc_review`, which has persisted build-evidence guards.
- `claim_task` persists current process PID and host via `claim_task_with_pid`; success assertions will reopen/read the row and verify those fields.
- Planned failures: SQLite abort triggers for create/assignment/claim/status writes, wrong-worker claim refusal, malformed batch request, and the temp-only DB obstruction for all six tools including optional-input `list_tasks`. Failures will be asserted as bounded executor JSON with identity/correlation/privacy and `is_error=true`, except the explicitly documented batch per-entry outcome which is `is_error=false` by the handler contract.
- Next: implement the one focused `cfg(test)` matrix in `create_task.rs` and append exact coverage/handoff details here.

## Implemented board matrix

Added `p4_51_board_matrix_tests::board_live_fastdevelopment_registry_has_success_and_failure_proof` only as a `cfg(test)` module in `create_task.rs`. It builds the actual `PAMode::FastDevelopment` registry, fetches all six concrete registered handlers, and invokes them through `ToolExecutor::execute_batch` with an isolated temporary home/workspace, real worker/task/run rows, and correlated call IDs.

| Tool | Success evidence | Failure evidence |
| --- | --- | --- |
| `create_task` | Creates a ready task and reads it back from TasksDb; `list_tasks` also consumes it | SQLite insert abort trigger with planted title/description/criteria; later temporary DB obstruction |
| `list_tasks` | Reads the tool-created task and full acceptance criteria | Tasks DB obstruction confined to the temporary home |
| `assign_task` | Supervisor assigns a ready task to a real runnable worker; DB readback proves assigned status and owner | SQLite status-update abort trigger; state remains ready; later temp DB obstruction |
| `assign_tasks` | Batch assigns one task and records a second invalid task ID in its per-entry error row; readback proves the successful assignment | SQLite abort is represented as `failure_count=1` and per-entry `ok=false` while outer `ToolResult.is_error=false`, matching the handler's partial-batch contract; empty-batch input and later temp DB obstruction return failed tool results |
| `claim_task` | Assigned worker claims task; result/readback preserve task ID, `in_progress`, process PID, host, and worker owner | SQLite claim abort trigger and wrong-worker refusal; task stays assigned; later temp DB obstruction |
| `update_task_status` | The owning worker moves its claimed task to legal `failed` status; durable status and cleared claim PID are read back | SQLite status-update abort trigger with planted reason; later temp DB obstruction |

All executor-level failures assert `is_error=true`, bounded JSON output, schema version, `ok=false`, stable error code/category, operation/tool identity, retry fields, bounded message/cause/action, session/agent/task/run/call correlation, timestamp, and exclusion of planted argument markers. The batch tool intentionally reports per-entry failures in a successful outer result; the test explicitly distinguishes that from failed ToolExecutor results and covers both semantics.

## Limits and coordinator handoff

The unit covers these six board tools only; it does not close TG4-25/P4-51 or prove permission denial, timeout/cancellation, unknown-tool/parse failures, provider failures, or other builtin inventories. `create_task` executes its shipped `scheduler::kick("supervisor")`; the test does not install a scheduler and relies on the documented early-boot/test no-scheduler boundary. No global scheduler, network, or paid service was started.

No test/build/formatter command was run. Exact coordinator filter:

`cargo test -p arcana-core --lib agent::builtin_tools::create_task::p4_51_board_matrix_tests::board_live_fastdevelopment_registry_has_success_and_failure_proof --locked -- --exact --test-threads=1`

Changed files for this unit are limited to the `cfg(test)` addition in `create_task.rs` and this report. The concurrent `send_worker_message.rs` edit was left untouched.

## Source review status

The implementation asserts the returned batch tuple retains the model's original tool-use ID, while the normalized error envelope carries the executor-authored `${session_id}-${tool_use_id}` correlation. The real worker run ID is generated in the isolated RunsDb and threaded through each executor; no fixed or guessed run identity is used. The test's host assertion accepts any non-empty persisted host (including the production fallback `unknown`) while pinning PID to the current process.

This source review is not compilation or execution evidence. The coordinator owns the requested test filter.

Main fixture correction: the successful failed-status transition returned the expected status and cleared ownership/claim metadata. The draft incorrectly expected the assigned worker to remain. UpdateTaskStatusTool forwards the requested nullable assignment into the existing transition API; this input requests no owner. The assertion now pins no assigned worker and no claim PID after failure, preserving the real lifecycle contract. Earlier write-failure/privacy checks reached that point successfully; final filter is rerunning.

## Coordinator verification — 2026-10-06

The corrected exact registered-executor matrix passes on Windows1/1 (1.27s) and Linux1/1 (2.06s). It preserves actual assignment/claim/transition semantics and checks durable task readers, injected writer failures and temporary storage obstruction. Logs: target/p4-ci-checks/final-board_live_fastdevelopment_registry_has_success_and_failure_proof-{windows,linux}-2026-10-06.log. No production board handler changed. Core cargo check and formatting pass. The full P4-51/TG4-25 gate remains open.