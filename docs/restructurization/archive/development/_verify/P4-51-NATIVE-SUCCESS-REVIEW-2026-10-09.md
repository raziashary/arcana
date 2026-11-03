# P4-51 Native Success Review — 2026-10-09

Review in progress. Scope: test-only native provider seam and registered subagent success fixture, including specified runtime consumers. No source edits, builds, tests, process/network access, or task/gate closure.

## Progress

The diff adds a `cfg(test)`-only optional provider override, wraps the injected provider in the existing `CappedProvider`, and leaves the production `cfg(not(test))` path calling `build_provider`. The ignored fixture uses all three registered spawn tools via `ToolExecutor::execute_batch`; it asserts child results and token accounting for direct/parallel runs, and polls QC manager status plus persisted claim/cost state. Source tracing confirmed the real reader and settlement paths and identified the fixture limits below.
## Review result

No actionable defect found in the reviewed seam or fixture.

- `native_runner::RunnerDeps::provider_override` and the alternate `run_inner` branch are both `cfg(test)`; test injection is still wrapped by `CappedProvider`, while non-test builds continue through `build_provider`. The runtime initializer supplies `None` in test builds. This preserves the shipped provider-selection path.
- `registered_subagent_tools_execute_children_and_read_back_qc_accounting` registers FastDevelopment builtins and calls each actual spawn registration through `ToolExecutor::execute_batch`. Single and parallel paths assert completed child summaries and returned token totals. The QC child is produced by the real `SubagentManager`/`AgentEngine`; its scripted stream issues `get_task`, then asserts the next provider request contains that registered tool's successful result for the same task in `qc_review` status.
- QC completion is observed through `SubagentManager::get_status` and the real `TasksDb::get_by_id` reader until `under_review_by` is cleared and `task_qc_cost_usd` is positive. The fixture begins with a fresh task, so the positive value is attributable to this settlement path. Engine token/cost accounting uses the existing capped provider and engine settlement paths.
- The fixture explicitly documents its synthetic successful build-batch evidence as input setup, not proof that a compiler ran. No claim of compiler execution is made.

## Scope limits

This was a static review only. The success test is `#[ignore]` because it installs process-global manager and analytics singletons; this review did not run it or independently verify its runtime behavior. The fixture proves successful child completion, the QC `get_task` consumer round-trip, and positive QC cost/claim settlement. It does not prove real-provider behavior, provider failures, permission denial/timeout, cancellation, compiler execution, or broader P4-51 failure-contract coverage. These are limits of the fixture's claim, not defects in the reviewed code.

Inputs traced: MDP P4-51; `native_runner.rs` and its new test module; `runtime.rs`; `subagent/manager.rs`; `spawn_subagent.rs`, `spawn_parallel_subagents.rs`, and `spawn_qc_review.rs`; `AgentEngine` tool registration/subset and accounting; `ToolExecutor::execute_batch`; `get_task`; and `CappedProvider`.
