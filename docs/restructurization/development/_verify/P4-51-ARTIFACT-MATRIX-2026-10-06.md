# P4-51 Artifact Matrix — 2026-10-06

Scope: bounded `cfg(test)` coverage for FastDevelopment `check_file`, `annotate_file`, and `write_interface_spec` through `ToolExecutor::execute_batch`. This report is an evidence log; no tests, formatters, Git mutations, live credentials, network calls, or production changes are in scope.

## Progress
- Report initialized before investigation.
- TG4-25 and the exact handler paths reviewed; test implementation complete, execution remains coordinator-owned.

## Investigation log
- The requested `arcana-core/src/agent/builtin_tools/annotate_file.rs` did not appear in the first targeted file inventory; `check_file.rs` and `write_interface_spec.rs` exist. Confirming the live path before any test edit.
- Current plan evidence says P4-51/TG4-25 remain open for a complete builtin/error-class matrix; this slice is limited to the three named artifacts and should not be reported as task/gate closure.
- Memory and current planning records both identify `ToolExecutor::execute_batch` as the production execution boundary and require real consumer behavior evidence. The prior Arcana evidence also cautions that symbol or adjacent unit-test presence alone is insufficient.

- TG4-25 requires each builtin through production ToolExecutor with deterministic success plus injected failure; structured error fields include bounded/versioned envelope, category/stable code, operation/tool identity, retryability/delay, corrective action, bounded context, available correlation IDs, timestamp, failed-result semantics, and planted-input privacy. The scoped test cannot close TG4-25.
- Live source path is `annotate_file_tool.rs`; there is no `annotate_file.rs` in the current module inventory. The parent confirmed the intended owner is the existing `annotate_file_tool.rs` test section.
- Source review: `check_file` has deterministic in-process JSON syntax checking (`tier1_syntax` dispatches `.json` to `serde_json`); an unrecognized-only check list yields a direct handler failure suitable for private-input filtering. `annotate_file` persists into `AnnotationsDb::open()` and `list_for_file` is the production readback. `write_interface_spec` requires a current claimed task and calls `auto_checkpoint_before_write` before writing; its prior file content enables checkpoint readback.

## Implemented bounded slice

- Extended only the existing `#[cfg(test)]` module in `arcana-core/src/agent/builtin_tools/annotate_file_tool.rs`; no production code, dependencies, or schema changed.
- The test registers the actual FastDevelopment registry and invokes `check_file`, `annotate_file`, and `write_interface_spec` using `ToolExecutor::execute_batch`.
- `check_file`: valid isolated JSON file passes the production `json_parse` syntax check; an unknown-only check selection produces a failed result.
- `annotate_file`: valid annotation persists and reads back through `AnnotationsDb::list_for_file`, including prompt, range, source, and severity; malformed line input creates no extra row.
- `write_interface_spec`: valid route writes `INTERFACE.md`; the pre-write checkpoint is read from the production checkpoint store and its snapshot is checked against the prior file contents. Malformed route input leaves the artifact and checkpoint count unchanged. The test creates and claims a task through `TasksDb` because the live handler requires a current claim.
- Each malformed result asserts `is_error`, bounded versioned JSON, `ok=false`, stable error/tool operation identity, category/code/message/cause/action, retry fields, timestamp, and task/agent/session/call correlation. Distinct planted path, source, prompt, route, and convention markers must be absent from the result.

## Handoff

- Exact coordinator filter: `cargo test -p arcana-core --lib agent::builtin_tools::annotate_file_tool::workspace_tests::artifact_builtins_have_registered_executor_success_and_failure_contracts --locked -- --exact --test-threads=1`.
- Test execution, formatting, and Git operations were not performed here; the coordinator owns those checks and any fixture correction.
- Coverage is limited to these three artifact builtins and malformed-input failure paths. It does not cover other builtin registrations or permission, provider, handler/database injection, timeout, cancellation, or broader consumer/telemetry categories. P4-51 and TG4-25 remain open.


## Coordinator verification — 2026-10-06

The exact registered-executor matrix passes on Windows1/1 (0.68s) and Linux1/1 (0.98s). Annotation and checkpoint readers consume the stored results; malformed calls leave durable artifacts unchanged. Logs: target/p4-ci-checks/final-artifact_builtins_have_registered_executor_success_and_failure_contracts-{windows,linux}-2026-10-06.log. No production artifact handler changed. Core cargo check and formatting pass. The remaining P4-51/TG4-25 inventory and error categories stay open.