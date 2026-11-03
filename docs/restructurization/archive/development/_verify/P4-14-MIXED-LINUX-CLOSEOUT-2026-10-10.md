# P4-14/P4-15 mixed-project production verification — 2026-10-10

## Authorized bounded change

Three existing Windows-only real ProjectCommand Cargo/Python fixtures now run on Linux too. Windows retains its signed-runtime preparation and repository-local ACL workspace. Linux uses native temporary storage and the previously approved isolated Python environment. No production command, sandbox, evidence, retry policy or schema changes. ADR0354 records the fixture decision.

## Actual consumer checks

- `production_mixed_project_batch_requires_every_child_to_pass_before_qc`: real Rust pass plus Python nonzero compileall creates a failed complete batch; the status tool refuses QC. Repairing Python and rerunning both creates a complete passing batch and the same tool accepts QC.
- `production_mixed_batch_timeout_cannot_be_hidden_by_passing_sibling`: real Python pass plus selected Cargo timeout yields expected2/observed1/passing1 and incomplete outcome after TasksDb reopen; no fabricated exit or retry increment; status tool refuses QC.
- `production_mixed_batch_stops_after_third_observed_failure`: real Rust nonzero hits the third failure, preventing Python launch. Worker submits diagnosis through registered request tool; Supervisor grants one batch through the registered resolution tool. Repair and one authorized real Cargo/Python batch produce two passing exits and QC acceptance.
- Existing `production_multi_project_spawn_failure_cannot_be_hidden_by_passing_sibling`: actual first Cargo pass and later removed selected project create incomplete evidence; reopened API and direct SQL reject QC, no false exit/retry count.
- Existing `cancelling_production_project_command_stops_descendant_and_closes_batch`: actual first Cargo pass followed by build-script descendant heartbeat; aborted future closes incomplete evidence and stops heartbeat. Reopened API/direct SQL reject QC.

## Receipts

Linux mixed group3/3,18.27s; exact spawn-failure1/1,1.89s; cancellation1/1,1.16s. Windows mixed counterpart3/3,266.56s. This whole-fixture timing includes Windows runtime/sandbox setup and does not isolate a performance cause or establish a comparable cold/warm benchmark. Credential variables removed per command; isolated DB profiles and owned temporary workspaces only. Child operations finish or are killed; no server/listener or paid provider run is involved.

Ignored logs: `target/p4-ci-checks/p4-mixed-batch-{linux,windows}-2026-10-10.log`, plus exact spawn/cancellation Linux receipt names.

Core/CLI `cargo check --offline -p arcana-core -p arcanactl --locked` passes (36.25s); workspace formatting, staged whitespace and secret-pattern scans pass. Independent Luna High source review finds no actionable defect; see `P4-14-MIXED-LINUX-REVIEW-2026-10-10.md`. The reviewer did not execute tests; root receipts above provide execution evidence.

## Limits

This closes a specific Linux production corpus gap, not P4-14/P4-15/TG4-07/TG4-08 wholesale. The current Linux launcher provides the older workspace write confinement; these results do not prove the stricter external-read/network TG4-31 boundary. macOS XPC execution remains absent, full supported-host corpus and independent closure disposition remain open. P4 stays41/59 and TG419/33. Owner Calendar separate; no live Fleet completion claim.
