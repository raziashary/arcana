# P4-61 Workspace Write Lease Review — 2026-10-12

Scope: read-only review of `platform/workspace_write.rs` and its actual `write_file` consumer, including lock identity/path security, cancellation, hook reentrancy/deadlock risk, and material regressions. Subprocess/staging integration is explicitly out of scope. No tests, builds, or Git mutations are in scope.

Status: source review complete; no builds or tests were run.

## Findings

### P2 — Cancellation while waiting for the lease leaves a false unknown outcome

`execute_batch` persists the operation intent before calling `execute_handler` (`arcana-core/src/agent/tool_executor.rs:1100-1105`). The new handler first awaits `WorkspaceWriteLease::acquire` and only invokes `tool.execute` after acquisition (`tool_executor.rs:498-522`; the lease can wait up to 30 seconds at `arcana-core/src/platform/workspace_write.rs:20-25, 61-76`). If session cancellation wins the outer `select!` while that acquisition is pending, `execute_batch` returns `operation_cancelled_outcome_unknown` and deliberately skips `finish_operation_journal` (`tool_executor.rs:1146-1163`), even though the tool handler never ran and the dropped acquisition future releases its file handle. For task-scoped runs, the unresolved intent then activates the recovery barrier: `RunsDb::record_operation_intent` rejects subsequent non-Board effects for that task (`arcana-core/src/db/runs.rs:795-823`). Distinguish cancellation before handler start from cancellation after execution may have begun, and record the former as a known non-execution outcome; retain `unknown` once the handler may have produced effects.

### P2 — Detached `PostFileWrite` shell hooks can mutate outside the workspace lease

`WriteFileTool` spawns `HookRegistry::fire(PostFileWrite, ...)` and does not await it (`arcana-core/src/agent/builtin_tools/write_file.rs:145-167`). `HookExecutor` supports configured `HookAction::Shell` commands (`arcana-core/src/hooks/executor.rs:42-106`) and does not acquire the workspace lease. Such a hook can therefore write the same workspace after the file-tool lease is dropped, concurrently with another managed writer. The detachment avoids a nested-lease deadlock, but means the lease does not serialize this write-capable callback. Either keep mutating hook work inside the coordinated boundary with a reentrancy-safe design, or make the lease contract explicitly exclude hook subprocess effects and ensure the write hook path cannot be treated as serialized.

## Reviewed without findings

- Lock identity is based on the canonical workspace path (`arcana-core/src/platform/workspace.rs:49-66`) and hashed into a stable filename; the lease retains the lock file handle and rejects a non-directory lock directory / non-file lock target (`platform/workspace_write.rs:27-59`). On Unix, creation requests mode `0600`, opens without following the lock-file symlink, and rejects multiple hard links (`platform/workspace_write.rs:37-59`). No concrete lock-identity or path-escape defect was found in this source review.
- `PreToolUse` runs before journal/lease acquisition, while `PostToolUse` is reached after `execute_handler` returns; the `PostFileWrite` hook is detached. I found no synchronous hook re-entry that waits on the same lease, so no direct deadlock was evident (`tool_executor.rs:1063-1105, 1142-1163`; `write_file.rs:145-167`).

Limits at initial review: source inspection only; no runtime tests/builds. This review does not assess subprocess/staging integration or claim the lease coordinates arbitrary external writers.

## Follow-up verification — 2026-10-12

The admission-cancellation finding above is resolved in current source. `ToolExecutor::execute_batch` waits for selected-workspace write admission before `begin_operation_journal`; if session cancellation wins while waiting, it returns `operation_cancelled` and never records a mutation intent. `cancelling_write_lease_admission_creates_no_unknown_mutation_intent` passes on Windows and reopens the isolated RunsDb to verify both an empty operation page and no unresolved task side effect.

The detached PostFileWrite shell-hook path is also routed through the selected workspace and a lease acquired inside the hook, so it does not nest under the file tool's lease. Windows hook routing/admission tests pass 5/5; retained-child cleanup tests prove the lease remains unavailable until the OS reports the job empty. Linux/macOS refuse this hook capability before spawn.

Residual boundary: `ToolExecutor::acquire_write_lease` currently leases `ToolEffect::Write`; it does not serialize the shell hook with concurrent `Execute` or `FreeformShell` handlers. This remains a production integration gap for P4-18/P4-61 and must not be described as full same-workspace process-write serialization. The current tests cover the reviewed cancellation path and the Windows hook-owned process lifecycle, not the complete cross-tool/process matrix.

A fresh read-only Luna High review found no confirmed scoped security defect. It noted that the cleanup owner has no production shutdown/drain path; job-close termination is configured, but exclusion of another Arcana process until shutdown cleanup finishes is not proven. Treat this as a lifecycle proof gap.

