# P4-14/P4-15 Named Gate Reconciliation — 2026-10-12

Scope: normative P4-14/P4-15 and TG4-07/TG4-08 clauses; relevant production consumers and specified evidence receipts.

## Findings

Review in progress.

## Governing clauses located

- MDP P4-14 (`MASTER_DEVELOPMENT_PLAN.md:1453`) requires task-scoped persisted build evidence on every production transition to `qc_review`, including direct SQL, rejecting missing, unattributable, failing, stale, or incomplete latest batches. It also specifies atomic per-selected-project completion records and the current-workspace fingerprint condition.
- MDP P4-15 (`MASTER_DEVELOPMENT_PLAN.md:1454`) requires three consecutive observed nonzero exits to trigger a worker diagnosis/stop; one explicit Supervisor grant bound to task and assigned worker, atomically consumed before spawn and pinned to the exact latest failed/incomplete diagnostic batch; Rust-owned argv/observed exits; pass resets streak; spawn failures/timeouts remain separate; Bash remains available but cannot authorize QC.
- TG4-07 (`TESTING_GATES.md:415`) additionally requires restart-surviving task-associated evidence, QC refusal without that task’s pass, no passing build misrecorded as failed, and Windows PowerShell plus `cmd` assertions against real spawned-process exit results.
- TG4-08 (`TESTING_GATES.md:419`) states the same diagnosis/approval policy and Rust-only QC evidence boundary.

The clauses above are the source of truth for the reconciliation. The attached dated status paragraphs also mention broader remaining proof; I am checking whether each comes from a separately named task/platform requirement or is an unproven conjunction inside these clauses.

## Evidence receipts read

`_verify/P4-14-QC-CLOSURE-CHECK-2026-10-09.md` maps the production writer/reader contract: atomic running/final batch persistence, observed exits only, latest-batch trigger and API checks, workspace-fingerprint binding, direct-SQL refusal, restart/cancel/spawn failure and Tauri/RecipeRunner consumers. It records an isolated Windows `cmd.exe` and PowerShell exit test at 0/17 (1/1, 3.83s, 2026-10-10). That exact TG4-07 shell clause has a named execution receipt; the report still distinguishes it from shell-integration-to-build-evidence and broader process corpus proof.

`_verify/P4-14-MIXED-LINUX-CLOSEOUT-2026-10-10.md` records Linux production mixed-project 3/3, spawn-failure 1/1, cancellation 1/1; Windows mixed counterpart 3/3; and actual TasksDb reopen/API/direct-SQL checks, with observed expected/observed/passing counts. Its limitation is explicit: this is not P4-14/P4-15/TG4-07/TG4-08 wholesale, and macOS XPC plus strict TG4-31 containment remain open.
