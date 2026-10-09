# P4-62 Command Logs Reader Integration Proof

Date: 2026-10-09
Scope: Add a bounded integration test proving project-command stages are persisted through the JSONL writer and exported by the real `arcanactl logs` reader, including evidence persistence and TasksDb batch readback.

## Progress
- [x] Created this report before investigation.
- [x] Inspect relevant rule/spec and public interfaces.
- [x] Draft assertions from the contract, then implement the assigned integration test.
- [x] Record source claims, exact test filter, constraints, and unresolved limits.

## Findings
Investigation completed; source claims are recorded below.

### Contract-derived assertions drafted before fixture-source inspection
Initial assertion draft before source inspection: run the public `ProjectCommandTool` path for a task-attributed Python build, persist tracing output through the configured JSONL writer, invoke the real `arcanactl logs export` consumer, and read the batch through the public TasksDb reader. Source review corrected the stage labels and correlation field names below; the implemented assertions also cover missing and wrong-owner claim refusals.

### Source-of-truth reviewed
- `_verify/P4-62-NEXT-PROOF-2026-10-09.md`, “Remaining exact acceptance clauses” item 2 says current capture-only tests do not prove configured rolling JSONL writer plus shipped Logs reader; requested proof is ProjectCommandTool + actual export + all five correlated stage rows + persisted build-batch reader.
- `TESTING_GATES.md`, TG4-32 requires separate sandbox-preparation, process-wait, output-drain, and build-evidence timings correlated by tool call, alongside cold/warm measurements; the new test is specifically the missing writer-to-consumer integration proof, not a measurement claim.
- `MASTER_DEVELOPMENT_PLAN.md`, P4-62 remains partial; the fresh evidence does not change any gate status.

## Progress
- [x] Created this report before investigation.
- [x] Read the acceptance clause and drafted assertions before fixture-source inspection.
- [x] Inspect public interfaces and existing subprocess test pattern.
- [x] Implement the assigned integration test.
- [x] Record source claims, exact test filter, constraints, and unresolved limits.
- Public consumer pattern confirmed: `startup_retry_logs.rs` initializes `arcana_core::observability::logging::init()`, sets an isolated HOME/USERPROFILE before logger init, then runs `env!("CARGO_BIN_EXE_arcanactl") logs export <path> --component arcana-events` and parses JSONL. `logging::init` and `init_with_filter` are public; the test is a separate integration target with one test so process-global logger/env state is isolated.
- The core command type is public and implements `Tool::execute`; `evidence_persistence` is emitted in the production command path, while timer helper/tests are private to the core crate. Existing core test `p4_62_measure_linux_task_build_stages_with_evidence` has the closest fixture structure, but its `#[cfg(test)]` internals cannot be called from the integration target. I will reproduce it using only public APIs and flag anything unavailable.
- Corrected the draft to match emitted JSON fields: the five stages are `runtime_prepare`, `sandbox_prepare`, `process_wait`, `output_drain`, and `evidence_persistence`; correlation is emitted as `call_id` plus string `task_id` (there is no `tool_call_id` field on these stage records). The timer initializes outcome as `incomplete` and marks `success` only after stage work completes.
- The real stage integration requires isolated HOME/USERPROFILE and a profile-local selected workspace. On Windows the production path invokes the existing local Python runtime staging step; it does not use the Ruff install path or a network dependency. The test will use only `intent=build`, `toolchain=python`.
- Plan includes two refusal cases before the successful call: missing task identity and mismatched worker identity. Both must return errors and produce no stage rows; then the correctly claimed worker must produce the five successful rows and a passing TasksDb batch.

## Implementation progress
- Added one isolated integration test in `src-cli/tests/project_command_stage_logs.rs`. It sets a profile-local HOME/USERPROFILE, asserts the configured writer exists, creates a minimal Python project, creates/assigns/atomically claims a task, and calls the public ProjectCommandTool API.
- The test exercises two invalid claim guards (missing task ID and wrong worker) and requires both to fail before any stage events. The valid claimed build must succeed and return a passed, complete one-child batch via `TasksDb::latest_project_build_batch_status`.
- It drops the async logging guards to flush, invokes `CARGO_BIN_EXE_arcanactl logs export --component arcana-events`, parses the exported JSONL, and requires exactly one successful correlated event for each of the five production stage names with a u64 duration.
- No build, test, formatter, or git command has been run.


## Source claims and test contract

- `ProjectCommandTool` is public and implements the public `Tool::execute` contract (`arcana-core/src/agent/builtin_tools/project_command.rs:25,34,89`). Worker claim validation happens before the process stage (`project_command.rs:160`; `agent/tool_trait.rs:728`). `TasksDb::claim_task_with_pid` performs the real assigned-to-in-progress claim (`db/tasks.rs:3371`), and `latest_project_build_batch_status` reads the persisted batch (`db/tasks.rs:2251`).
- The production stages are named `runtime_prepare`, `sandbox_prepare`, `process_wait`, `output_drain`, and `evidence_persistence` (`project_command.rs:587,1024-1055,1199,1226,1235`; the Linux task fixture lists the same stages at `project_command.rs:3065-3069`). Stage records carry `call_id`, string `task_id`, `duration_ns`, and `outcome` (`project_command.rs:945-980`). Their success state is set only after the work completes.
- Python Build resolves to the standard-library `-m compileall -q ... .` command (`arcana-core/src/platform/command.rs:159-167`), matching the temporary source fixture. The Windows launcher invokes the existing Python runtime staging path during the real call (`project_command.rs:1024-1043`); runtime staging selects a locally available signed interpreter and copies it into the isolated Arcana profile (`arcana-core/src/platform/python_runtime.rs:336-394`). This path does not call the network-enabled Ruff install function.
- `startup_retry_logs.rs` establishes the repository's public integration pattern: isolated profile, standard logging initialization, actual `CARGO_BIN_EXE_arcanactl logs export`, and exported JSONL parsing. `logging::init` and `LoggingGuards::writer_count` are public (`observability/logging.rs:69,83`); CLI `logs export` reads component-selected files and writes the export (`src-cli/src/commands/logs.rs:212-239`).
- The new test adds the actual ProjectCommandTool consumer and both task-claim guard refusals to that pattern. It uses the repository-local `target/p4-62-stage-log-workspaces/<uuid>` path for the child workspace, following the existing Windows process fixture convention; the profile and unique workspace are owned and removed by their guards. The parent directory may remain empty.

Exact test filter for the coordinator to run later:
`cargo test -p arcanactl --test project_command_stage_logs claimed_python_build_stages_survive_jsonl_writer_and_cli_export --locked -- --nocapture`

Not run here, per the dispatch: no build, test, formatter, or git command. No provider, credentials, persistent user DB, or network path is used by the test. Windows execution depends on a locally available signed Python interpreter accepted by the production staging path; the test intentionally does not download or install one. This is an environment prerequisite, not a reason to substitute an in-memory capture.

Environment note: the Windows `stage_selected_python` path also requires the selected runtime to contain the pytest closure before it will stage, even though this test's command is only Build. The focused test therefore needs the same configured local signed Python environment as the existing staged-runtime production fixture; it does not fetch pytest. This prerequisite should be checked by the coordinator before running the exact filter.


## Coordinator implementation and actual proof — 2026-10-09

The positive case now uses the actual FastDevelopment registration through ToolExecutor, not a direct handler call. The executor forms a compound session/provider call ID. Owner ADR-0343 approves mapping only designated Arcana telemetry call fields to domain-separated SHA-256-derived canonical UUIDs; arbitrary strings, sensitive fields and foreign targets retain normal redaction. Canonical UUIDs retain identity. Provider result IDs remain unchanged. The test requires the real JSONL writer and CLI export to return all five stage records and reads the passing task batch. Refused-call assertions compare derived IDs too, so redaction cannot hide a forbidden stage.

Windows: logging writer10/10 (1.49s), actual command/export1/1 (111.34s). Linux: writer10/10 (2.27s), actual command/export1/1 (13.65s). Raw outputs are the ignored `target/p4-ci-checks/correlation-writer-{windows,linux}-2026-10-09.log` and `project-command-stage-logs-{windows,linux}-2026-10-09.log`. No fixture profile/workspace or child process is retained. No provider request or live credentials were used. These fixture durations include setup and persistence and are not compiler-speed claims. The earlier bridge and lost-correlation failures are resolved on these hosts; remaining acceptance gaps in P4-62-NEXT-PROOF-2026-10-09.md remain open.
