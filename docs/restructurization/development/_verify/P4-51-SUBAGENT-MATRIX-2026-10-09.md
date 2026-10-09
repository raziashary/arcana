# P4-51 subagent executor matrix

Scope: `spawn_subagent`, `spawn_parallel_subagents`, and `spawn_qc_review` registered-tool coverage.

- [x] Created report before investigation.
- [x] Traced governing requirements and executor matrix patterns.
- [x] Added scoped failure coverage.
- [x] Recorded exact filter and coverage limits.

## Grounded findings

- MDP P4-51 requires versioned structured failures across handler, parse, permission, timeout, cancellation, and infrastructure classes, with stable operation/tool identity, bounded safe fields, optional correlation IDs, and no raw credentials, prompts, source, arguments, or unbounded OS/provider details. TG4-25 continues to require the production tool/provider matrix and planted-input privacy/retry proof.
- Existing matrix tests use `register_for_mode(FastDevelopment)`, real `ToolExecutor::execute_batch`, isolated home/workspace, `RunsDb` correlation, `schema_version=1` / `ok=false`, and bounded error field assertions.
- All three registrations use `subagent::manager::global()` from a process-wide `OnceLock`. `RunnerDeps` has only `AppState` and pending permissions; native runner builds the provider from the Vault and has no provider override. `RecipeRunner::with_provider_for_test` applies only to RecipeRunner, not SubagentManager.
- A safe successful child completion cannot be produced through the existing seam without a real Vault key/provider call or a production/test-seam change outside this assignment. This slice exercises the actual FastDevelopment registrations through ToolExecutor and proves their manager-unavailable failures; successful child-result consumer readback remains explicitly open.
- The test uses fresh process-local global state, isolated home/workspace, and `Autonomous` permission mode so no interactive permission request or provider/network call is possible. Exact filter: `agent::builtin_tools::spawn_subagent::p4_51_subagent_executor_matrix_tests`.

## Test implementation

- Added one exact-filter test in `arcana-core/src/agent/builtin_tools/spawn_subagent.rs` under `p4_51_subagent_executor_matrix_tests`. It registers actual FastDevelopment tools and invokes `spawn_subagent`, `spawn_parallel_subagents`, and `spawn_qc_review` via production `ToolExecutor::execute_batch` with valid family-specific inputs.
- The fixture uses a redirected temporary Arcana home, a temporary workspace, an isolated `RunsDb` run, Autonomous mode, stable agent/task/session/call IDs, and a fresh-process empty-manager precondition. It cannot reach Vault/provider construction or network.
- Each call must return `is_error=true`; assertions require a bounded JSON failure envelope, schema 1, `ok=false`, stable tool and `tool.<name>` operation, error classification, bounded non-empty message/cause/action, retry fields, timestamp, top-level and nested run/agent/task/session/tool-call correlation, and no success payload fields. Planted credential/prompt/source/argument/path values must be absent.

## Exact verification and limits

- Exact filter for the coordinator: `cargo test -p arcana-core --lib agent::builtin_tools::spawn_subagent::p4_51_subagent_executor_matrix_tests --locked -- --test-threads=1`. This subagent did not run tests, per dispatch.
- This adds concrete failure coverage for the three registrations. It does not prove successful child execution, provider failure classes, permission timeout, cancellation, scheduler orchestration, QC claim/cost consumer readback, or telemetry readback. Existing SubagentManager has no test-only provider injection seam, so successful in-process scripted child runs would require a seam outside assigned ownership or a real credential/provider; neither was used.
- No production code, schema, dependencies, build/test execution, formatter, git operation, environment inspection, or external provider/network activity was performed.


## Coordinator verification — 2026-10-09

The unchanged three-registration failure fixture passes Windows1/1 (0.47s) and Linux1/1 (0.54s). Each actual registered tool returns a bounded correlated failed result with planted inputs omitted. A source search found no core-test installation of SubagentManager; production installation remains in the shared runtime initializer. Formatting passes. This is failure-only coverage; it does not increase the 39/47 success/failure tally or close P4-51/TG4-25.
