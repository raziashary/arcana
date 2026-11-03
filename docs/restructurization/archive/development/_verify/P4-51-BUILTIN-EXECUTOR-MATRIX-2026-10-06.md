# P4-51 Builtin Executor Matrix — 2026-10-06

## Progress
- Created this report before repository investigation, as requested.
- Memory quick pass: no Arcana/P4-51/TG4-25 matches in the registry; no prior notes used.
- Next: read the governing P4-51/TG4-25 requirements, draft assertion targets from that text, then inspect the public executor/registry surface and existing test fixtures.

## Findings
- Pending.

## Assertion draft from the governing contract (before test implementation inspection)

From MDP P4-51 and TG4-25, this test unit should establish through the production executor boundary that a registered builtin failure is visibly a failure and remains useful and safe for the agent. For each scoped live-registry tool family selected here, assertions should cover a real valid invocation whose output is successful, plus a malformed invocation that yields a structured failed result. The failure assertions should verify JSON/schema and bounded message/cause/action/context, `ok=false` and `is_error=true`, stable operation/tool identity, available call correlation, and absence of planted private argument values and oversized handler detail. Successful output must be checked as real handler output, not merely absence of an error. Registry enumeration should report the exact tools covered and uncovered so additions cannot be mistaken for coverage. This unit does not claim to cover permission, timeout, cancellation, provider, scheduler, or all-infrastructure categories unless those are directly exercised here.

## Harness findings

- `builtin_tools::register_for_mode` builds the actual `FastDevelopment` registry; `ToolRegistry::all()` exposes its registered `Arc<dyn Tool>` values, and each tool exposes `name`, `input_schema`, `effect`, and `execute`. The test can therefore enumerate live registrations and pass those concrete handlers directly to `ToolExecutor::execute_batch`.
- The executor test module already imports the required runtime types and uses `tempfile::tempdir()` plus `db::test_support::with_home` for isolated Arcana state. `ToolContext` has explicit session, workspace, call, run/task, permission, role, depth and budget fields.
- The selected inventory is broader than one handler: file/workspace, task-board, epic/phase, and budget registrations need deterministic input fixtures. Exact per-tool valid and malformed inputs are still being mapped from the real handlers.

## Implemented test unit

`agent::tool_executor::p8_execution_registry_tests::builtin_file_task_epic_budget_matrix_uses_live_fastdevelopment_registry` is the focused test. It constructs the production FastDevelopment registry, derives covered/uncovered names from `ToolRegistry::all()`, and invokes each covered concrete tool through `ToolExecutor::execute_batch`. Success paths use a separate temporary workspace and home/TasksDb; malformed inputs exercise handler validation and a workspace escape guard. Failure assertions cover JSON schema version, `ok=false`, normalized error code/category, tool/operation identity, session/agent/call correlation, retry fields, bounded message/cause/action and output size, plus planted argument omission. Success assertions inspect returned file/grep/glob, task, epic/phase, and persisted budget state.

### Current FastDevelopment inventory

The current `register_for_mode(FastDevelopment)` registrations were enumerated in `builtin_tools/mod.rs`. The test computes this partition from the runtime registry on execution; test execution is intentionally deferred to the coordinator.

Covered (13): `read_file`, `write_file`, `edit_file`, `append_file`, `list_directory`, `grep`, `glob`, `get_task`, `create_epic`, `create_phased_task`, `get_epic_progress`, `get_ready_tasks_in_phase`, `set_task_budget`.

Uncovered (34): `bash`, `git_commit`, `project_command`, `mcp_list_tools`, `mcp_call_tool`, `check_file`, `list_tasks`, `update_task_status`, `assign_task`, `assign_tasks`, `create_task`, `request_supervisor_action`, `planner`, `spawn_subagent`, `spawn_parallel_subagents`, `spawn_qc_review`, `write_interface_spec`, `e2e_probe`, `search_memory`, `get_memory`, `list_memories`, `propose_memory`, `suggest_worker`, `claim_task`, `analyze_diff`, `annotate_file`, `list_worker_requests`, `resolve_worker_request`, `send_worker_message`, `list_worker_messages`, `list_stale_worker_messages`, `list_worker_health`, `reconcile_worker_operation`, `acknowledge_worker_message`.

### Limits and blockers

This is a bounded first slice, not the complete P4-51/TG4-25 matrix. `list_tasks` has only optional fields and defaults malformed types rather than producing a failure, so it needs a separate injected DB failure if it is to have both success and failure rows. `create_task` invokes the global `scheduler::kick("supervisor")`; this isolated unit does not start or poke a global scheduler, so it remains uncovered pending a scheduler-isolated consumer fixture. Other uncovered categories need their owning provider, scheduler, permission, ACP, Memory, or network fixtures. No production code/contracts or dependencies changed. No tests were run; the coordinator owns the exact filter execution.

## Handoff status

- Drafted the test and its coverage partition; no production files outside the assigned executor test module were changed.
- Exact coordinator-owned filter: `cargo test -p arcana-core --lib agent::tool_executor::p8_execution_registry_tests::builtin_file_task_epic_budget_matrix_uses_live_fastdevelopment_registry --locked -- --exact --test-threads=1`.
- This agent did not execute the filter or any other tests. The runtime-computed inventory and all assertions still require the coordinator's prescribed execution.

Inventory guard: the test also pins the current 47-name FastDevelopment registry and compares it with `ToolRegistry::all()`. A registration addition/removal therefore fails the focused test until its covered/uncovered disposition is reviewed.

## Continuation — file mutation matrix

- Added real `write_file`, `edit_file`, and `append_file` calls through `execute_batch`, each with malformed-input failure assertions. Their valid calls mutate only temporary `mutation-proof.txt`; direct filesystem readback checks the expected content after each success and confirms each malformed call leaves it unchanged.
- Updated the coverage inventory to 13 covered / 34 uncovered tools, preserving the main agent's `assign_tasks` name and `BTreeSet` partition fixes. The expected live registry list remains pinned at 47 names.
- No tests or other commands were run. The coordinator still owns the exact filter execution; no gate is marked closed.

Main-session inventory correction: the pinned registry contains 47 names. The earlier prose included unregistered assign_tasks_batch, overcounting the complement by one. The executable inventory already uses actual assign_tasks and compares the full registry set; the correct file/task/epic/budget partition is 13 covered and 33 outside this slice. MCP's separate executor success/revocation fixture adds two concrete tools; Memory and ACP are separate pending units.

Runtime inventory correction: the actual ToolRegistry contains BOTH assign_task and assign_tasks. The draft expected set omitted assign_task and the runtime equality assertion failed on both hosts. The source pin is corrected to the observed 47 names; the actual complement is 34. The preceding 46-name inference is retracted. The nonexistent assign_tasks_batch prose entry remains removed. This preserves a failing completeness check rather than weakening it.

Final main verification: p8_execution_registry_tests passes Windows18/18 (2.61s) and Linux18/18 (3.11s), including the corrected actual47-name registry equality, thirteen successes/failures, exact mutation readback and unchanged artifacts after refusals. engine module passes26/26 on both hosts, including seventeen real builtin failure results consumed by the model. Separate MCP executor success/revocation fixture passes in external_mcp module9/9 on both hosts. This unit covers15 of47 registered tools; other family candidates and the complete gate remain open.
