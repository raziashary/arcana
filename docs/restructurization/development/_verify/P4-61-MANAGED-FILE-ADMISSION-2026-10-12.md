# Managed file-write admission proof — 2026-10-12

## Complete unit

The approved workspace write lease uses an OS file lock keyed by the canonical selected workspace. The handle remains held through a ToolEffect::Write handler. Separate workspaces do not share a lock; a dropped/cancelled waiting future releases its handle. Lock files remain to avoid replacing a locked inode. The production ToolExecutor admits the lease before appending an operation intent, so cancellation before admission returns a known non-execution result and cannot quarantine a task for a mutation that never started.

Owners: arcana-core/src/platform/workspace_write.rs, platform/mod.rs and agent/tool_executor.rs. This adds no dependency or schema. ADR0363 and ADR0365 record the approved boundary and cancellation correction.

## Existing verified current-source evidence

The lease tests pass Windows3/3, with an ignored child helper invoked by the real cross-process contention test. They verify same-workspace contention, release after drop/cancellation, and independent workspaces. Actual executor consumer/context/panic checks pass1/1 each, as recorded in the 2026-10-12 checkpoint. The exact current cancellation regression cancelling_write_lease_admission_creates_no_unknown_mutation_intent passed again before this commit: it cancels while a real lease is held, then reopens runs.db and reads an empty journal plus no unresolved task side effect. No new source changes were made after those checks. Normal core/CLI cargo check and workspace formatting passed in the subsequent CLI unit validation; staged whitespace and secret-pattern checks are required before commit.

## Limits

This is cooperative managed file-tool serialization. Execute/FreeformShell tools, subprocesses and external editors are not covered. It does not complete staged copy-back, host shutdown cleanup, task snapshots, checkpoint coverage for all process writes, or any whole P4 task/gate. Windows shell-hook lifecycle code remains a separate uncommitted unit. No user database, paid provider, Calendar or server was touched.
