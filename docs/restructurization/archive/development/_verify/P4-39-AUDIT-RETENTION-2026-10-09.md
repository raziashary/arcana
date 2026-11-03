# P4-39 audit retention correction

## Source inspection: first pass

- `TasksDb::delete` issues a direct `DELETE FROM tasks WHERE id = ?1` (`tasks.rs:2739` onward).
- `TasksDb::clear_all` backs up a file-backed DB, then deletes worker requests and task rows in a transaction (`tasks.rs:2760` onward). Its docs state it is the wholesale Clear Board path.
- Unshipped migration 21 creates `task_budget_resolutions.task_id INTEGER NOT NULL REFERENCES tasks(id)` (`tasks.rs:1402` onward), so this FK prevents both task deletion paths from succeeding while audit rows exist.
- Migration 21 has immutable update/delete triggers and a task budget-clear trigger requiring the latest matching Supervisor audit row.
- Test module already includes rollback, bounded/indexed history, and audit-immutability coverage near the task-budget tests; it also has a file-backed Clear Board backup test.

Next inspection: read the full migration, resolver, history query, and production consumers before editing.

## Contract and consumer inspection

- ADR-LOG-RESTRUCT-0333 is accepted for P4-39 (2026-10-09): retain immutable audit rows with their original numeric task IDs, remove the restrictive FK, validate task existence on insertion, preserve deletion/Clear Board, and reject nonexistent resolutions.
- The resolver already performs an in-transaction `get_by_id(task_id)` and returns `Task {id} was not found` before its audit insert. Migration 21's insert trigger independently requires a matching active exceeded task. Thus removing the FK does not authorize orphan audit inserts.
- `TasksDb::delete` and `clear_all` delete only tasks (plus worker requests for clear); neither touches `task_budget_resolutions`. With the FK removed, the immutable rows remain.
- Production history readers use the shared bounded `get_task_budget_resolution_history` query: `set_task_budget_tool` reads back and validates the just-written row; `get_task_tool` attaches history to task detail; scheduler budget handling reads history after the Supervisor loop. The shared query filters by the retained numeric task ID, so existing row/history behavior is preserved. `get_task_tool` requires the task itself to exist before presenting detail; retained history remains directly queryable through TasksDb after deletion.

Next: inspect in-module fixtures and add isolated in-memory tests for row delete, board clear, audit readback/immutability, and nonexistent task rejection.

## Test design

The existing `ready_task` and budget setup APIs can create legitimate resolved audit rows. I will add one in-memory regression covering both actual delete APIs and post-delete history readback, plus a missing numeric task ID case that asserts the history remains empty. Existing tests already assert audit `UPDATE` and `DELETE` rejection; the new test will repeat immutability checks after deleting the parent task to prove deletion does not weaken audit immutability.

## Bounded implementation added

- Migration 21 now stores `task_budget_resolutions.task_id` as a required numeric value without `REFERENCES tasks(id)`. The existing Supervisor authorization, active-task existence/hold checks, audit immutability triggers, and budget-clear guard are unchanged.
- Added `task_budget_resolution_audit_survives_delete_and_clear_board` in the existing `tasks.rs` test module. It uses only an in-memory TasksDb; asserts row deletion and `clear_all` complete, each resolution remains readable by its original numeric ID, update/delete remain rejected after task deletion, and resolving `i64::MAX` fails with no audit row.
- Existing resolver code already checks task existence inside `BEGIN IMMEDIATE` before inserting the audit, so no resolver behavior change was needed.

Semantic limit surfaced to the owner/integrator: `clear_all` resets the `tasks` autoincrement sequence. A later task may reuse a deleted numeric ID, and history queries keyed only by numeric ID can then include the retained old audit. The existing clear-board sequence-reset contract is outside this approved bounded edit and is left unchanged; retention survives, but numeric IDs alone cannot distinguish a later reused ID. This needs explicit disposition before claiming unambiguous lifetime identity.

No tests, build, formatter, git mutation, environment access, live database, network, or Calendar command was run in this delegated slice.

## Exact test filter for integration

`task_budget_resolution_audit_survives_delete_and_clear_board` is the new focused test. It has not been run in this delegated slice; the integrator owns test execution and final validation.

Reader proof boundary: the new source test exercises the shared production `TasksDb::get_task_budget_resolution_history` reader used by `set_task_budget_tool`, `get_task_tool`, and scheduler budget handling. Their wrapper outputs were source-traced but not invoked here because this slice owns only `tasks.rs` and the report. In particular, `get_task_tool` intentionally returns “task not found” once the task has been deleted; the new test is intended to assert audit readback after deletion through the shared DB history API. Integrator should keep any existing actual Supervisor/tool integration readback in the final evidence.

## Owner follow-up: stable task identity and ID preservation

Owner directed adding a separate immutable task identity for provenance and preserving numeric task IDs across Clear Board. I am inspecting the existing task row/schema and UUID conventions before changing the owned `tasks.rs`; validation remains with the integrator.

### Identity and additive-upgrade findings

- `Task` is materialized by one positional `row_to_task` mapper shared by eight explicit projection constants; adding an identity requires adding it to every task projection and mapping, then consumers of serialized audit rows receive it from `TaskBudgetResolution`.
- Production task creation has three insert paths in `tasks.rs` (`create`, `create_epic`, `create_phased_task`); UUID generation is already established as `uuid::Uuid::new_v4().to_string()` in this module. Several raw SQL insert fixtures omit optional columns, so the migration needs an insert trigger to supply a stable identity for those writers too.
- `migrate::run_ladder` runs additive migrations atomically and automatically backs up migrations containing `UPDATE`; an additive next migration can backfill existing tasks and audit records, including old audit rows whose task has already been deleted.
- `clear_all` currently resets both task and worker-request sequences. The approved correction will preserve the task sequence while continuing to reset worker-request IDs.

Implementation plan: add schema migration 22 for nullable-added/backfilled UUID identity columns, a unique task identity index, insert generation and immutable-update guards, audit identity validation/backfill, and add identity to the actual history readback struct/query. Keep all changes in `tasks.rs` and this report.

### Stable identity implementation added

- Added additive migration 22. It backfills existing task rows with lowercase hyphenated UUID v4 identities, copies each identity to its matching existing budget-audit rows, and gives a unique synthetic UUID to legacy audit rows whose task was already deleted before migration.
- Added a unique task identity index, raw-insert identity generation, and an immutable task-identity update trigger. The three production task creation APIs explicitly write `uuid::Uuid::new_v4().to_string()`; all eight shared Task projections and `row_to_task` return the identity.
- Budget resolutions now persist the task UUID alongside numeric `task_id`. The shared bounded production history query and `TaskBudgetResolution` mapper return that UUID. Existing whole-row update/delete guards keep audit identities immutable; a new insert guard requires an audit identity to equal the active task identity.
- `clear_all` now resets only the worker-request sequence, preserving AUTOINCREMENT history for tasks. New tasks after Clear Board therefore receive higher numeric IDs and distinct UUIDs.
- Added in-memory source regressions for UUID uniqueness across standard, epic, phased, and legacy SQL insert paths; immutable task UUIDs; populated version-21 migration backfill and audit readback; history UUID readback; delete/Clear Board history retention; new-task non-aliasing; and missing-task refusal.

Exact new test filters: `task_budget_resolution_audit_survives_delete_and_clear_board`, `task_identity_is_unique_and_immutable_for_created_tasks`, `task_identity_upgrade_backfills_existing_tasks_and_budget_audits`. No tests/build/formatting were run in this slice.

Migration limit: a version-21 audit whose task was already deleted has no former task UUID to recover; migration 22 assigns that legacy orphan audit a unique synthetic UUID. From migration onward, newly written audit rows always carry the active task's immutable UUID. This is source-level implementation evidence only until the integrator runs the focused filters and actual Supervisor/history-reader smoke.

Coordinator corrections: populated-v21 backfill now drops/recreates the immutable audit UPDATE guard inside the existing atomic migration transaction; a moved String fixture and latest-schema22 expectations are corrected. Whole TasksDb regression initially reached67 passes and exposed3 legacy-fixture/column-inventory failures. The intentional downgrade fixtures now remove the new identity column/triggers before replaying migration22, and the schema inventory explicitly includes the approved identity column. No production migration is skipped or weakened.


## Owner-approved provenance correction, 2026-10-09

ADR0333/0336 supersede the pending retention/ID-reuse choice above. The new additive migration22 assigns/backfills immutable unique task UUIDs, includes them in retained resolution rows and task readers, and preserves numeric task AUTOINCREMENT across Clear Board. Deleted audit has no restrictive task FK; insertion still requires the active held task and authorized Supervisor. Both immutable-audit and tasks_updated_at triggers are restored inside the atomic migration, so backfill preserves historical timestamps. Older deleted legacy audit has no recoverable parent identity; it receives a distinct synthetic UUID rather than an invented link to a new task.

Final production TasksDb module passes Windows70/70 (61.36s), Linux70/70 (137.95s), including populated upgrades, immutable history, delete/clear, non-reuse, usage/other holds, rollback, direct-SQL guard and legacy timestamp checks. Windows installed Supervisor guidance1/1 (0.18s), real saved-Fleet ordinary/resolved cases1/1 (4.93s), exact SQL registry8/8 (4.82s), core and CLI/Tauri checks pass. Linux Fleet/default/query and both-host retained audit reader verification are recorded in the final closeout once completed. The new reader is an explicit opt-in under ADR0340; default get_task missing behavior stays unchanged.
