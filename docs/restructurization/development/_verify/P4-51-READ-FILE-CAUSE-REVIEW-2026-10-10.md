# P4-51 read_file error-cause review — 2026-10-10

## Scope and progress

- Read-only review of the isolated `read_file` filesystem-error formatting repair and its oversized-path consumer regression.
- Check actual cause propagation, canary omission, positive OS error assertion, and source-grounded scope.
- No tests/builds/network/credentials/user DB/Git/source changes or gate closures.
- Findings will be appended as the relevant production producer and test consumer are inspected.

## Review findings

- No defect found in the isolated producer correction or its registered-consumer regression. `ReadFileTool::execute` reaches `std::fs::read_to_string` after path resolution/workspace jail and its pre-read hook; the `Err` arm now returns `ToolResult::error(format!("Cannot read file: {e}"))` without interpolating the requested path (`read_file.rs:79-93,95-118,142-144`). This keeps the OS error payload while removing the path reflection that truncated before the executor could redact it.
- The regression establishes that the input is a real path whose own `std::fs::read_to_string` fails, captures `raw_os_error()`, then passes that actual long path through the registered `read_file` case in the all-registered handler fixture (`engine.rs:2792-2798,2860-2868`). The fixture executes the handler under the real `AgentEngine` registry/executor and inspects the matching `ToolResult` in the second provider request, i.e. what the next model receives (`engine.rs:3021-3077,3083-3105`). The read_file-specific assertions require a cause beginning `Cannot read file: ` and containing the independently observed OS code (`:3130-3139`); the shared marker loop rejects `planted-private-os-path` in the model-visible result (`:3145-3156`). The message/cause bounds and correlated operation/tool/call/run assertions also cover the enclosing structured error.
- The source rationale matches the scoped decision recorded under `ADR-LOG-RESTRUCT-0358` in `ADR_LOG.md:7407-7409`: retain the real filesystem cause while omitting the requested path; no shared sanitizer or schema change. The prior defect note at `:7396` records that the formerly reflected path was truncated before exact full-argument removal, matching the reproduced long-path scenario.
- Scope limitation: this proves the `read_to_string` OS-error branch for an oversized filename. Other `read_file` rejection branches still format paths (for example the full-file size guard at `read_file.rs:126-138` and line-count guard at `:151-165`); neither this producer change nor this fixture proves universal prefix sanitization or path-free diagnostics across those branches. Shared sanitizer hardening remains a separate decision as requested.

## Limits

- Read-only trace of the `read_file` producer, error-envelope conversion, registered-engine consumer fixture, and its ADR record. No tests/builds/network/credentials/user DB/Git/source edits/gate closures. Coordinator test results were accepted as supplied, not rerun.
