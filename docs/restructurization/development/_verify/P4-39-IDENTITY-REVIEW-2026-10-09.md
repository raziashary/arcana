# P4-39 identity and budget-resolution review

Scope: dirty implementation in the requested task/database, builtin tool, and recipe files against MDP P4-39 and ADR0333/0336. Review is read-only except this report. No tests, builds, installs, network, credentials, database access, or Git mutation.

Progress: report created before source investigation. Initial diff inspection complete; governing plan and consumer tracing in progress. Findings will be recorded as verified.

## Findings

1. **P2 — Retained history has no production reader after its task is deleted.** `get_task` calls `db.get_by_id(id)` first and returns `Task #… not found` on `Ok(None)` (`arcana-core/src/agent/builtin_tools/get_task_tool.rs:52-53,91-93`); its history query runs only inside `Ok(Some(t))` (`:54`). The other production references to the history getter are resolution readback and the active-task Supervisor loop; after deletion those routes are no longer available either. Thus the rows survive in SQLite and `TasksDb::get_task_budget_resolution_history` can select them by numeric ID, but the shipped Supervisor/user-facing task reader cannot inspect their identity, reason, or limits once Clear Board/deletion removes the parent. The new deletion test validates only the internal DB method (`arcana-core/src/db/tasks.rs:9688-9694,9711-9717`), so it does not prove the retained audit is consumable after deletion. Add/extend a real audit reader that accepts the retained identity/ID without requiring a current task, or otherwise expose deleted-task audit history through the intended production surface.

2. **P3 — Clear Board documentation still promises task-ID reuse.** The changed implementation now intentionally preserves `tasks` AUTOINCREMENT state by deleting only the `worker_requests` sequence (`arcana-core/src/db/tasks.rs:2879-2884`), matching ADR-LOG-RESTRUCT-0336. But the adjacent `clear_all` docs still say it “reset[s] the auto-increment counter,” “reset[s] both auto-increment sequences,” and the next insert “starts from id 1 again” (`arcana-core/src/db/tasks.rs:2837-2846`); the backup explanation repeats that both sequences reset (`:2847-2848`). These statements are now false and directly contradict the ID-preservation contract, which can mislead future callers/operators about retained audit keys. Update the comments to describe preservation of task IDs and reset of worker-request IDs.

## Scope and limits

Read the governing P4-39 entry and ADR-LOG-RESTRUCT-0333/0336, prior P4-39 audit/consumer reports, the scoped dirty diff, the migration/resolver/delete/clear/history code, shipped `get_task` and budget tool behavior, Supervisor recipe guidance, scheduler’s captured production-path test, and all source references to the history API. No tests or builds were run per the delegated scope.

Review status: source and consumer tracing complete. Two actionable findings remain; report created/updated only, with no source edits or validation runs.


## Main disposition

Both findings are resolved under owner ADR0340: registered get_task now has an explicit include_deleted_audit option and returns real retained identity/history after delete/Clear Board; default missing semantics remain intact. The two reader cases pass Windows/Linux2/2. Clear Board comments now accurately state retained task sequence/audit and reset worker-request sequence. Full TasksDb70/70 and saved-Fleet completion1/1 pass on both hosts. See P4-39-CLOSEOUT-2026-10-09.md.
