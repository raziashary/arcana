# P4-51 Probe Planner Matrix — 2026-10-09

## Scope

Investigate whether a focused `cfg(test)` matrix can exercise existing `e2e_probe` and planner registrations through the real FastDevelopment registry and production executor, without external providers, network, subprocesses, or live systems. Ownership is limited to `arcana-core/src/agent/builtin_tools/e2e_probe.rs` test additions and this report.

## Progress

- [x] Created this report before source investigation.
- [x] Inspected the owned probe implementation and adjacent deterministic test seams.
- [x] Inspected planner registration and execution path for a real-reader matrix.
- [x] Added focused planner success/readback and probe/planner failure coverage; probe success is not deterministic under the stated constraints.
- [x] Recorded the exact test filter and findings for parent-run verification.

## Findings

- `e2e_probe` is registered in FastDevelopment in `arcana-core/src/agent/builtin_tools/mod.rs` as `E2eProbeTool`.
- The probe's success path is intrinsically process and network dependent: `execute` validates the input and workspace, then calls `run_probe`; `run_probe` spawns `cmd /C` or `sh -c` children and `http_probe` issues a reqwest GET. A success test cannot meet the no-external-process/no-network constraint without replacing production behavior or using an invalid seam.
- Existing probe tests cover serialization (`probe_report_serializes`), URL policy (`the_probe_still_works_for_what_it_is_for`), child-output capture (`a_child_s_output_is_captured_as_evidence`), and bounded logs (`the_captured_tail_is_bounded`). None exercises registered-tool success through `ToolExecutor` with an actual response reader.
- `planner` is registered in the same FastDevelopment registry. Its success path is local: `PlannerCompiler::new` opens `PlansDb`, `create` writes a plan, and `getPlan` reads the plan back. This supports an isolated home/workspace executor matrix without providers, network, or subprocesses.
- The existing executor matrix helpers in `tool_executor.rs` demonstrate that executor-returned call IDs preserve the input ID while normalized failure envelope context uses `<session>-<call>`.

## Implementation

- Added `p4_51_probe_planner_matrix_tests::probe_and_planner_registered_matrix_uses_executor_and_real_reader` in the owned `e2e_probe.rs` file.
- The test builds the actual FastDevelopment registry and calls tools through `ToolExecutor::execute_batch` with an isolated Arcana home and workspace.
- Planner success creates a plan, then reads it back through the registered `planner` `getPlan` action and checks the persisted ID and title.
- Planner malformed-YAML and probe empty-command failures must arrive as bounded version-1 JSON error envelopes, with `is_error`, stable fields, and normalized `<session>-<original-call-id>` context. The helper separately asserts `execute_batch` preserves the exact original call ID. Both failures plant markers that must not appear in output.
- The probe failure exits before URL validation, process creation, or HTTP. No production code, planner source, registry/schema, dependencies, or shared JSON reader was changed.

## Handoff

Parent-run test filter: `p4_51_probe_planner_matrix_tests::probe_and_planner_registered_matrix_uses_executor_and_real_reader`.

Full `e2e_probe` success remains unproven under the delegated constraints because its implementation requires an OS subprocess and loopback HTTP request. Existing helper-level test seams provide serialization, URL-policy, and child-output evidence; none provides the requested registered-tool success plus actual reader. A deterministic executor success seam would require production injection/refactoring or a real local process/server, outside this ownership and prohibition.

No tests, builds, formatting, or Git mutation commands were run in this delegated task.

## Coordinator verification

The unchanged registered-tool matrix passed on Windows (1/1, 0.53s) and Linux WSL (1/1, 0.84s). Logs: target/p4-ci-checks/verified-probe_and_planner_registered_matrix_uses_executor_and_real_reader-windows-2026-10-09.log and approved-probe_and_planner_registered_matrix_uses_executor_and_real_reader-linux-2026-10-09.log. The actual planner create/getPlan reader is now covered in both success and failure. The probe has only pre-launch rejection coverage here and is not counted as successful. Deterministic registered-tool success/failure coverage is therefore 39/47; P4-51/TG4-25 remain open. No provider, user database, server or Calendar was used.
