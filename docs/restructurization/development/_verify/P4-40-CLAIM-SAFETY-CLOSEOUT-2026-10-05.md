# P4-40 — claim metadata and stale-safe release

Status: **P4-40 proven**, 2026-10-05.

## Contract and production writers

Only an atomic task claim may enter `in_progress` with worker, PID and host together. Other lifecycle transitions clear PID/host. A release must match the observed status, worker, PID, host and update timestamp; a replacement claim must survive stale automatic or manual recovery.

| Writer / consumer | Enforced behavior |
|---|---|
| TasksDb migration 6 insert/update guards | Reject claimed inserts, partial claim metadata, metadata on nonclaimed states, changes to an existing claim's owner/PID/host, and nonclaim exits retaining metadata. All SQLite connections are covered. |
| `claim_task_with_pid` / `CLAIM_TASK_WITH_PID_SQL` | Assigned owner, non-epic kind and one-active-task restriction are predicates of the same update that records status/owner/PID/host. |
| `claim_next_ready` / `CLAIM_NEXT_READY_SQL` | Self-service oldest-ready selection and claim are one guarded update; second claims cannot leave a half-assigned task. |
| Production `ClaimTaskTool::claim_from_db` | Checks run identity and unresolved effects, then supplies this process PID to the atomic API. Returned task budget is read from the claimed row. |
| CLI `task claim` | Calls `claim_next_ready` with this process PID. The retired src-shared task mirror has no remaining writer. |
| Shared status API; agent status tool; Tauri task commands; RecipeRunner reconciliation | Preserve metadata only for the unchanged existing in-progress owner; clear it on other transitions. Generic status writes cannot fabricate a fresh claim. Tool description now states this boundary. |
| Routing assignment | Clears claim metadata atomically with assignment. |
| Budget exhaustion / dependency unblocking | Needs-info/ready transitions clear claim fields. |
| Epic creation/completion; task creation/deletion | Epics and new tasks never claim; completion concerns nonclaimed epics, deletion removes the whole record. |
| Retry, diff, budget accounting and QC-review ownership writes | Do not change task claim identity; SQLite guards remain active. QC subagent ownership is a separate field. |
| RecipeScheduler interrupted-run quarantine | `hold_interrupted_claims_for_worker_run` checks exact run worker/PID/host before setting needs-info and clearing metadata. |
| Automatic orphan reconciler | Uses the observed task snapshot with `release_orphaned_task_if_unchanged`; stale false is skipped, database errors propagate. |
| Registered Tauri `force_release_task` | Checks the caller's observed snapshot, then the same database CAS; task-change event occurs only after release succeeds. |

A fresh search of core, desktop, CLI and shared Rust found no production task SQL mutation outside TasksDb. Matches in orphan/project-command/SQLite instrumentation/migration files are test-only modules. `release_in_progress_claims_for_worker` is also test-only. The React `OrphanAlert` consumes automatic release events; it does **not** invoke manual release. The registered manual endpoint and actual task-board/orphan read commands are exercised directly below; no manual UI-button proof is claimed.

## Behavior proof

- Raw independent SQLite writer: `p4_claim_triggers_reject_inconsistent_metadata_from_independent_sql_writer` attacks partial claims, forged claimed inserts, replacement owner/PID/host, NULL fields and exit-with-old-metadata. Every refusal leaves the previous row intact; shared failed transition clears metadata. `p4_claim_` group passes Windows/Linux 3/3 each.
- Existing concurrent claim tests: two tasks for one worker and two workers for one task each permit exactly one winner; Windows 1/1 each. Stale orphan snapshot preserves a newer claim, Windows/Linux 1/1 each.
- Real Tauri manual commands: three tests cover stale preflight, reassignment **after** preflight and successful release. A tracing Layer observes the production release span and replaces the claim via a separate connection before CAS; the command returns stale and preserves worker B's PID/host. Successful release is read back through production task/orphan commands, with metadata cleared and audited reason preserved. Windows 3/3 (0.45s).
- Production claim tool: isolated tasks and runs stores, owner mismatch refusal, successful owner/current-PID/nonempty-host readback, and unknown-effect reconciliation refusal. Windows 2/2 (0.37s); Linux 2/2 (0.95s).

- Real rebuilt CLI smoke: six child processes create two tasks, claim the first with matching actual process PID/host, refuse a second claim without any row mutation, transition the first to failed and read it back. The failed transition clears owner/PID/host. Profile and JSON summary: `%TEMP%/arcana-p4-claim-cli-7wh1_62v/claim-smoke-summary.json`. No provider requests or listeners.

## Validation and limits

Workspace `cargo check --workspace --locked` passes; format check passes. No credentials or Calendar directory are used. Linux command-level desktop proof awaits the existing host matrix; SQLite and core claim behavior are portable and locally exercised on Windows/Linux. P4-40 closes its metadata/CAS invariant only; recovery-through-full-coding-completion and platform process sandboxing remain separate open tasks.
