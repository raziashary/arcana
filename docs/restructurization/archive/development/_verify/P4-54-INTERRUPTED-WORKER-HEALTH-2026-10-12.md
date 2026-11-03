# P4-54 Interrupted Worker Health Readback — 2026-10-12

Status: investigation started. This report is updated as evidence is gathered.

## Requirement and reader contract

P4-54 and TG4-28 require Supervisor-visible persisted lifecycle, task ownership/dependency, operation/recovery state, heartbeat, failure, usage/activity and checkpoint information, while proving each field through its production writer and reader. The assigned fixture is one actual backend worker and one task; it has no dependency/parent-child setup and uses a scripted provider, so this extension will not invent usage or claim two-worker recovery.

`list_worker_health` is registered for FastDevelopment. Its output reads the latest worker run, task health, heartbeat/progress, operation journal, active operation, session usage, and task/session-scoped checkpoint count. The actual tool must be called through a FastDevelopment `ToolRegistry` and `ToolExecutor.execute_batch` under Supervisor identity. Checkpoint health exposes count and latest timestamp; checkpoint IDs and task/session links can be verified through the public checkpoint reader alongside the Supervisor-visible count. RunsDb's `active_worker_operation` intentionally only reports an operation while the run remains open and in `running`; after this fixture fails, health should say `idle` while the journal separately exposes the unresolved unknown effect and recovery-required flag.

The extension will read health after the worker failure and DB reopen, then compare its safe values to the existing public RunsDb/TasksDb/checkpoint readers. It will assert the real failed run and task ownership/status, the acknowledged and unresolved journal cursors/outcomes, checkpoint linkage/count, and terminal current-operation semantics. Dependency fields are checked only for the existing no-dependency fixture. No source arguments, private task prose, or synthetic usage values will be sent through telemetry or asserted.

## First prescribed validation attempt

The exact locked command failed during compilation at the heartbeat assertion: the Supervisor JSON reader returns `Option<&str>` from `Value::as_str`, while the persisted worker run has a non-optional timestamp. This is a test assertion type mismatch. I am aligning the comparison to `Some(run.heartbeat_at.as_str())` and will rerun only the same prescribed command.

## Root continuation after agent usage limit

Root runs the exact fixture. The first run fails because the assertion assumes absent dependencies serialize as an empty array; the current TasksDb optional blocked_by value is null. Replace the invented shape with exact equality to json!(task.blocked_by), preserving comparison with the real reopened task reader. This is a test expectation correction, not a production workaround. The run remains recorded as failed; only a later passing receipt may establish proof.

## Accepted real-consumer receipt

Root's exact credential-stripped selected test passes1/1 in1.33s. The actual shipped backend worker performs two writes, loses one durable acknowledgement and fails. After reopened persisted readers, the real FastDevelopment-registered list_worker_health is invoked through ToolExecutor under Supervisor identity. It exposes the error worker, failed run/failure class/heartbeat, real needs_info task/assignment/dependency value, terminal idle operation state, two task/session-associated checkpoints, and exact acknowledged/unknown journal cursor/path/recovery-required values. The subsequent scheduler kick still issues no provider call or repeated mutation. Current normal core/CLI check and formatter/whitespace checks pass.

The agent's report/edit survived its Luna usage limit; root completed review and execution. No new production API/schema/default, synthetic usage record or health writer is introduced. One-worker proof does not establish two-worker startup/mid-run reassignment, parent-child recovery, every field producer or whole TG4-28. P4-54 remains open.
