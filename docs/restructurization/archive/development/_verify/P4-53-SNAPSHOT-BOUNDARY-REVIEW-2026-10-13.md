# P4-53 Snapshot and Reconciliation Boundary Review — 2026-10-13

## Review record

- Scope: assigned P4-53 snapshot and worker-operation reconciliation boundary, limited to the named Rust modules and the requested MDP/ADR sources.
- Checkout: `main` at `origin/main`; target modules include concurrent uncommitted edits. This review is read-only and will assess the present working-tree content.
- Constraints honored: no tests, builds, process launches, network, database, environment-value reads, dependency edits, or Git mutation.
- Progress: source and contract trace complete; findings recorded below.

### Contract checked

The MDP P4-53 clause (MASTER_DEVELOPMENT_PLAN.md, P4-53 row) requires task-attributed assignment/dependency identity, operation outcomes/cursor, artifact/checkpoint and continuation references, repository/worktree/index/untracked state, and reconciliation before same/replacement-worker resume. The host-snapshot receipt explicitly requires refusing live runs, other workspace/task, changed owner/dependency/Git state, absent/malformed snapshots, and unprovable effects; file completion must derive from sealed host expectations, and arbitrary command/network effects cannot be inferred from unchanged files. ADR-0375 authorizes implementation but does not close gates; ADR-0378 repeats these exact boundaries.

## Initial review finding (superseded by latest source)

### Medium — retry-safe proof omitted targets under generated directories

- Reachability: the production ToolExecutor captures a `write_file` intent and seals the requested content digest, then returns `retry_permitted: true` for a host-approved `RetrySafe` reconciliation. `RecoverySnapshot::verify` accepts that decision when the current whole-workspace fingerprint equals the captured fingerprint (`arcana-core/src/agent/recovery_snapshot.rs:247-253`); it does not inspect the target file in this branch. The path jail permits workspace-relative paths, and `write_file` accepts those paths (`arcana-core/src/agent/builtin_tools/write_file.rs:59-71`).
- Root cause: `workspace_fingerprint::fingerprint` prunes entire directories named `target`, `dist`, `node_modules`, `.venv`, etc. (`arcana-core/src/platform/workspace_fingerprint.rs:8-21, 160-175`). A valid write target under one of these directories, for example `target/generated.txt`, is absent from both the before and current workspace fingerprints.
- Reproduction conditions: a task invokes `write_file` under an excluded directory; the process loses the acknowledgement; after interruption but before Supervisor reconciliation, that target is created or changed by another actor; the Supervisor chooses `retry_safe`. The host comparison still succeeds because the excluded subtree is invisible, and the tool reports `retry_permitted: true`. Retrying may overwrite the intervening file (especially when the original input used `overwrite: true`).
- Impact: a concurrent/local workspace change can be overwritten under a false idempotency proof. This is a data-integrity defect in the retry authorization boundary, not an observed remote-code execution path.
- Bounded remediation: for `RetrySafe`, compare the intended target directly against a sealed pre-operation state (existence plus bounded digest/metadata) and refuse if it changed, even when the general fingerprint excludes its directory. Alternatively, disallow replay when the target or any ancestor is within an excluded generated directory. Keep `Reconciled`'s exact content check separate.

## Hardening suggestion — unbounded fingerprint work

The serialized snapshot is capped at 32 KiB and caps dependency/checkpoint counts, but capture computes full workspace fingerprints before and after every task-attributed write (`recovery_snapshot.rs:158`, `workspace_fingerprint.rs:50-64, 96-125`). The traversal has no file-count, aggregate-byte, or elapsed-work limit and streams every non-excluded file to completion. Since this synchronous capture is on the ToolExecutor admission/completion path (`tool_executor.rs:1179-1184` and the outcome path), a very large or adversarial workspace can make each mutation consume unbounded host I/O/CPU and delay cancellation. This is a hardening concern rather than a demonstrated confidentiality or authorization bypass. Bound scan work and fail closed when the cap is reached; preserve a distinct unavailable-evidence result.

## Known owner-requested reader correction

`RecoverySnapshot::no_effect_hooks` treats `HooksDb::list_all()` as complete (`recovery_snapshot.rs:116-119`), while `HooksDb::list_all` and `list_by_type` discard every row decode error with `filter_map(|r| r.ok())` (`hooks/db.rs:38-63`). Thus malformed persisted hook rows are silently absent from the hook-absence decision. This is the known pending owning-reader correction requested by the main agent. I am recording it once as a fail-closed proof gap, not asserting that the skipped malformed row is currently executable: the hook runner uses the same list-by-type reader.

## Checks that currently fail closed

- The Supervisor tool checks a host-supplied role; replay decisions take a matching workspace write lease, reject open runs, require the exact task-attributed snapshot and reject legacy/malformed metadata (`reconcile_worker_operation.rs:49-56, 92-105`; `runs.rs:1012-1049`). The current tree also ties the originating run actor to the snapshot's assigned worker and holds the TasksDb transaction while comparing (`runs.rs:1039-1047`); task identity, NeedsInfo/unclaimed status, assignment, dependency states, canonical workspace/root identity, Git HEAD/branch/index, and retained checkpoint IDs are compared (`recovery_snapshot.rs:227-245`).
- Snapshot serialization is strict-versioned, rejects unknown fields and is capped at 32 KiB; dependency/session and checkpoint counts are bounded (`recovery_snapshot.rs:9-12, 127-162, 204-219`). Captured fields contain hashes, IDs, status/ownership labels and a workspace-relative expected path, not task prose, file contents, credentials, remotes or raw Git paths (`recovery_snapshot.rs:13-42`).
- Decision flags match the durable meanings: only `RetrySafe` returns `retry_permitted`, only `Reconciled` returns `operation_complete`; the DB keeps `do_not_retry` in the task barrier (`reconcile_worker_operation.rs:125-134`, `runs.rs:1150-1171`). These are source-path findings only; no tests or gate closure are claimed.

## Disposition

At the initial review snapshot, one Medium data-integrity defect and two proof gaps were present. The latest-source dispositions are recorded in the recheck below. No broader sandbox finding or task/gate closure is asserted.


## Latest working-tree recheck

The previous findings were rechecked against the current source. Three are addressed in this tree:

- The generated-directory issue is fixed at the reconciliation instant: `ExpectedFile.before` seals the target's absence or file digest/identity/permissions (`recovery_snapshot.rs:38-45, 56-112, 259-274`), and `RetrySafe` now requires both the original workspace fingerprint and the target pre-state to match (`recovery_snapshot.rs:348-353`).
- Recovery fingerprint scans now fail closed above 50,000 entries, 256 MiB read, or 10 seconds (`workspace_fingerprint.rs:94-110, 166-178`); the recovery path and `fingerprint_without_file` use the bounded mode.
- Hook presence no longer depends on row decoding: `HooksDb::has_enabled_hooks` uses `SELECT EXISTS(... enabled IS NOT 0)` so malformed/nonzero enabled values block replay (`hooks/db.rs:17-28`; called from `recovery_snapshot.rs:194-196`). The previously recorded known reader gap is resolved in the present source.

The current reconciliation path also reuses the guarded TasksDb connection via `capture_with_tasks` while comparing and appending the decision (`runs.rs:1065-1070`, `recovery_snapshot.rs:212-223, 319-329`). This source review does not independently validate the pending three-test rerun reported by the root agent.

### Confirmed Medium — retry authorization can go stale before replay

- Reachability: Supervisor reconciliation acquires a workspace lease locally, verifies the snapshot, and persists `retry_safe` before returning (`reconcile_worker_operation.rs:92-124`; `runs.rs:1004-1055`). The lease is held only by that tool call, so it is released when the call returns (`workspace_write.rs:10-13`). The durable task barrier treats only `unresolved` and `do_not_retry` as blocking (`runs.rs:1185-1198`), and `claim_task` proceeds when that query returns no blocker (`claim_task_tool.rs:91-116`). A later write call obtains a new lease and records a fresh intent/snapshot, but `begin_operation_journal` does not bind it to or revalidate the prior retry authorization (`tool_executor.rs:107-183, 1175-1184`).
- Reproduction conditions: after a `write_file` lost acknowledgement, the host verifies the sealed pre-state and records `retry_safe`; after that call releases the lease, another managed workspace writer changes the target; the task is later reassigned/claimed and the worker repeats the write with `overwrite: true`. The ordinary file tool permits that explicit overwrite (`write_file.rs:63, 100-118`), and no code rechecks the old recovery snapshot before the new mutation.
- Impact: a state that was proven safe at reconciliation can change before execution, allowing a resumed worker to overwrite intervening content. The new target pre-state comparison closes the earlier same-instant/generated-directory bypass but does not make the authorization valid through replay.
- Bounded remediation: bind `retry_safe` to the exact operation and expected arguments/state, then consume and revalidate that authorization under the next write lease immediately before dispatch. If the baseline changed, keep the task quarantined and require a fresh host decision. A decision stored durably must not serve as an unexpired blanket permit after workspace state changes.

## Updated disposition

The earlier generated-directory target, unbounded-scan, and malformed-hook-reader findings are resolved by the current source. One confirmed Medium time-of-check/time-of-use integrity defect remains: retry authorization is released durably while its workspace lease is not held through the eventual replay. No tests were run in this review, and no task/gate closure is claimed.




## Final focused disposition — latest source review

The previously reported stale retry-authorization/TOCTOU finding is **disabled for this first unit, not resolved by a one-use admission implementation**. In the current production path, `RunsDb::reconcile_operation_as_supervisor` explicitly rejects `OperationReconciliation::RetrySafe` with “retry admission revalidation is unavailable; leave this operation held” before any reconciliation append (`arcana-core/src/db/runs.rs`, around lines 1031–1034). That bounds the first unit by preventing a durable retry permission from being recorded through this path.

The `Reconciled` route remains usable after host comparison, but the tool maps it to `retry_permitted: false` (`arcana-core/src/agent/builtin_tools/reconcile_worker_operation.rs`, around lines 132–133). Thus a lost-ack reconciliation does not authorize retry. Keep retry admission held until the separately approved one-use admission/revalidation capability is implemented and proven; do not describe the TOCTOU as resolved by such an implementation today.

The root reports that the latest three registered-tool tests passed in 0.75 seconds, covering unchanged retry denial/hold and successful reconciliation of an actual lost acknowledgement with `retry_permitted: false`. This reviewer did not run or independently verify those tests. No task or gate closure is implied.
