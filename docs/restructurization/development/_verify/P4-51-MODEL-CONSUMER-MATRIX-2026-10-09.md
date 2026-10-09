# P4-51 Model Consumer Matrix — 2026-10-09

Status: verified on Windows and Linux; full P4-51 remains open. Scope is limited to the named `cfg(test)` AgentEngine fixture/helpers in `arcana-core/src/agent/engine.rs` and this report. No tests/builds/process launches/network/environment reads/.env/Vault/user DB/Git mutation will be performed here; coordinator runs exact filters.

## Progress log

- Report created before source investigation, per task instruction.

### Source baseline

- The live `register_for_mode(..., FastDevelopment)` arm registers the execution/file tools, task board, communication, planner/subagent/QC/spec/probe, memory, routing/claim, diff/annotation/budget, epic/phase, worker-request and ACP tools. I am mapping each exact `ToolRegistry` name to its handler behavior before extending the consumer fixture.
- Existing model consumer fixture covers 17 names and uses each real registered handler with missing required input. It verifies the next `ScriptedProvider` request contains correlated, bounded `ToolResult` error JSON and omits planted prompt/source/key/OS-error/path canaries. No test execution is authorized here.

### Fixture correction and safe-coverage boundary

- The existing 17-case fixture supplied the literal `run-builtin-failure` via `with_worker_run_id` without creating its durable `worker_run_state`. For mutation effects this could yield `operation_intent_unavailable` before handler execution. Those 17 rows are therefore not currently established as handler failures; the fixture will now create an isolated `RunsDb::begin_worker_tick` row per session and assert the operation-intent guard was not the returned failure. The coordinator has separately flagged this same defect and is recording its correction in ADR0351; this report retracts any prior treatment of the old rows as handler proof.
- Bash will be included only as a public `PendingPermissions` entry answered with `PermissionResponse::Deny`; its shell command will never be allowed. `e2e_probe` will use valid schema input and the current `sandbox_violation` fail-closed response, which returns before process/HTTP. No MCP request will be invoked.
- Some live registrations are read-only listing tools with no invalid-input failure path (empty input is a valid query); these cannot meet this fixture's controlled-failure criterion without database sabotage or new seams. I am identifying exact handler bodies and will leave such names explicit as gaps instead of inventing invalid behavior.

### Expanded AgentEngine consumer matrix authored (not run)

The only source edit is the existing `#[cfg(test)]` fixture `builtin_handler_failures_reach_model_as_correlated_private_error_results` and its local test setup in `arcana-core/src/agent/engine.rs`. It asserts the sorted live FastDevelopment registry equals the explicit 47-name inventory and that each name appears exactly once in the cases. Every case uses a fresh `RunsDb::begin_worker_tick("worker-backend", session_id)` under `db::test_support::with_home`; the expected run ID is the durable value and `operation_intent_unavailable` is explicitly rejected. Each failure is consumed from the next scripted provider request and checked for `MessageContent::ToolResult` with matching original tool-use ID/name, `is_error=true`, `ok=false`, schema version, operation/tool, run/call correlation, bounded message/cause/output, retryability, timestamp, suggested action, and planted canary omission.

| Live registrations | Controlled consumer failure used |
|---|---|
| `read_file`, `write_file`, `edit_file`, `append_file`, `list_directory`, `grep`, `glob`, `check_file` | Missing required path/content/pattern input; `list_directory` also plants a path-escape argument. |
| `bash` | Host-agnostic marker-writing command answered with explicit `PermissionResponse::Deny` through the public one-use pending map; asserts no marker file and drained map. No shell is approved or launched. |
| `git_commit`, `project_command` | Missing required commit/intent input; neither reaches Git mutation or child process. |
| `e2e_probe` | Valid required schema fields and loopback URL; asserts the registered handler's existing `sandbox_violation` before any process/HTTP attempt. |
| `get_task`, `create_task`, `assign_task`, `assign_tasks`, `update_task_status`, `claim_task`, `suggest_worker`, `set_task_budget`, `create_epic`, `create_phased_task`, `get_epic_progress`, `get_ready_tasks_in_phase`, `planner`, `request_supervisor_action`, `resolve_worker_request` | Missing required task/board/action fields, so each real handler rejects before a state mutation. |
| `spawn_subagent`, `spawn_parallel_subagents`, `spawn_qc_review` | Missing required task descriptions/subtasks/task ID; child/manager code is not reached. |
| `write_interface_spec`, `analyze_diff`, `annotate_file`, `propose_memory`, `search_memory`, `get_memory`, `send_worker_message`, `reconcile_worker_operation`, `acknowledge_worker_message` | Missing required input fields; no artifact, memory, ACP, or operation mutation is attempted. |
| `list_memories` | Empty-query read is forced to fail at `MemoryDb::open` by making only the isolated `memory.db` path a temporary directory; the directory is removed before the next case. |
| `list_tasks`, `list_worker_messages`, `list_worker_requests` | Empty-query reads are forced to fail at `TasksDb::open` by a temporary directory at only the isolated `tasks.db` path; the directory is removed before the next case. This prevents `list_worker_messages` from marking any row read. |
| `mcp_call_tool` | Missing remote name/arguments rejected before authorization or connect. |
| `mcp_list_tools` | Isolated profile has no egress consent; `authorize` returns before `connect`. No remote discovery request or Vault lookup occurs. |

No live user database can be reached: all test database resolution occurs under the isolated `with_home` guard, and the temporary directory blockers are asserted absent before creation and removed immediately after each handler returns. The list-tool failure source is database-open failure, not fabricated `ToolResult` output. The MCP source ordering is `mcp_call_tool` argument validation before `authorize/connect`, and `mcp_list_tools` `authorize` before `connect`; the fixture additionally checks those expected causes. The e2e probe fixture is intentionally a registered failing consumer row, not the missing positive probe success required for 47/47 deterministic success/failure coverage.

The matrix still does not prove that each handler emits a long OS diagnostic; `os_error` remains a planted input marker, not an emitted error. The compiler privacy tests also remain narrower formatter/direct-handler evidence. The broader provider IO timeout/cancellation and retry-safety audit remain separately pending under P4-51/TG4-25.

Initial author handoff (superseded by coordinator results below). Exact coordinator filter: `cargo test -p arcana-core --lib agent::engine::procedural_memory_tests::builtin_handler_failures_reach_model_as_correlated_private_error_results --locked -- --test-threads=1`. This source/report change was not executed here, so all assertions and host behavior remain unverified until the coordinator runs that exact filter. No gate or task count is closed.


## Coordinator execution and correction

- Exact all-registration filter passes Windows1/1 (6.13s) and Linux1/1 (7.82s). All47 names are matched against the live registry and every next model request consumes a correlated failed result. The fixture now sorts its expected names and finishes each real worker tick before beginning the next case. Initial inventory ordering and active-worker errors were corrected in the fixture without changing production guards.
- Surrounding `agent::engine::procedural_memory_tests` passes Windows15/15 (12.36s) and Linux15/15 (17.30s). Windows first reproduced a pre-existing fixture transport race: an accepted nonblocking socket returned WouldBlock. ADR0352 records the explicit blocking-mode setup correction before the existing bounded read timeout. Retry/no-replay, malformed arguments, MCP, reservation and cancellation checks remain intact.
- Logs: ignored `target/p4-ci-checks/model-consumer-matrix-{windows,linux}-2026-10-09.log` and `model-consumer-regression-{windows,linux}-2026-10-09.log`. Owned HTTP fixtures finish and release their listeners; denial cases launch no process. No provider credentials or live data are used.
- This supersedes ADR0351's pending model-consumer correction. Successful registration coverage remains46/47: confined smoke execution, actual compiler/private OS diagnostics and provider IO follow-up remain separate. No P4/TG4 checkbox or readiness score changes.
