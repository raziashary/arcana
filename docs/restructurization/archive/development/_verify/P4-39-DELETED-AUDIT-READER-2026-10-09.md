# P4-39 deleted-audit reader verification

## Progress

- Started: required report created before investigation.
- Source review complete: inspected get_task, task identity and resolution readers, deletion and Clear Board behavior, test home isolation, and executor registration patterns.
- Implementation complete: added opt-in deleted audit output and registered-executor coverage in the owned source file.
- Validation deferred: parent task owner is running integrated checks; no builds, tests, or formatting were run here.

## Source findings

- TasksDb::get_by_id is the live task reader. Task has immutable task_identity, but current get_task omitted it from JSON.
- TasksDb::get_task_budget_resolution_history(id, 20) is the existing bounded reader; the database clamps limits to 20. Migration 22 stores task identity on immutable resolution rows, preserving the original numeric ID and UUID after row deletion or Clear Board.
- The existing behavior errors when the task row is absent. The additive option can use the same production reader and distinguish audit-present from no-history without synthesizing task fields.
- Tests redirect TasksDb::open() through the crate-wide isolated-home guard. ToolRegistry and ToolExecutor provide the registered production path.

## Implementation

- Added optional include_deleted_audit boolean, default false. Missing active task IDs remain errors unless the option is true and retained budget-resolution history exists.
- Active task results now include task_identity. Deleted audit output contains only task_deleted=true, the original numeric id, stored task_identity, and at most 20 budget_resolutions. It has no task title or status.
- Database failures return generic errors without raw database text. An identity mismatch across retained rows fails closed.
- Added isolated production-reader tests that use register_for_mode(FastDevelopment), retrieve get_task from ToolRegistry, and call it through ToolExecutor::execute_batch. Coverage includes reads after row deletion and Clear Board, default missing-task failure, missing-without-history failure, ID/UUID provenance, and active-task identity.

## Validation

- Not run by instruction: parent task owner has active build/test checks, and this agent was explicitly told not to run builds, tests, or formatting.
- Source review confirms test database access is redirected to a fresh temporary Arcana home; tests use no user database or credentials.

## Remaining

- Parent task owner to run integrated validation and decide whether further changes are needed.


## Main integrated proof

Both real registered-reader tests pass Windows2/2 (0.67s), Linux2/2 (1.58s). Full TasksDb70/70 and saved-Fleet ordinary/resolved completion1/1 pass on both hosts; SQL registry8/8 and CLI/Tauri/core checks pass. The first telemetry-label filter ran zero tests and is not evidence; the corrected observability::telemetry_labels:: selection passes1/1 (4.72s). See P4-39-CLOSEOUT-2026-10-09.md.
