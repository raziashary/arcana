# P4-62 Windows Timing Review — 2026-10-12

Scope: current diff in `arcana-core/src/agent/builtin_tools/project_command.rs` and `_verify/P4-62-WINDOWS-TASK-EVIDENCE-TIMING-2026-10-12.md`.

## Findings

Review in progress.

## Scoped review and proof mapping

No confirmed defect found in the inspected measurement helper. `assert_task_build_timing_sample` reads the persisted latest batch and latest evidence through TasksDb, checks the selected workspace by canonical `workspace_identity`, requires one observed/passing build and one valid evidence row, and matches the evidence fingerprint to the batch (`project_command.rs:3230-3269`). It requires exactly one successful event for each of the five requested stages with the current call/task IDs, and ensures summed stage durations do not exceed measured tool wall time (`3271-3300`). The Windows fixture uses three fresh profile/workspace pairs, one first cold call plus five warm calls per profile, resetting the Arcana-owned process stage cache before each profile; median and min/max range are computed over three cold and fifteen warm observations (`3401-3542`). The even/odd median selection is correct for these sample counts, and the report explicitly retains raw samples and distinguishes tool wall time from compiler time.

The Linux ignored measurement continues to use the shared assertion and retains its prior three-workspace, six-call loop (`project_command.rs:3309-3393`). Its current source shape is compatible with the new reader assertions for successful batches/evidence and five correlated stage events. The verification record cites the earlier Linux eighteen-build measurement; I did not run the Linux fixture or validate that result in this review. The existing path-jail/app identity comparisons use `workspace_identity`, avoiding Windows case-only spelling mismatches.

## Normative task/gate scope versus older status notes

The exact MDP P4-62 row (line 1541) and TG4-32 row (`TESTING_GATES.md:538`) require the selected lower timeout and cancellation/reconciliation, a paused-clock operation over ten minutes but below the cap, separate five-stage timing, matched cold/warm fixtures with variance, and clean finite CLI deadline behavior. They do not require a paid-vendor benchmark or a macOS timing run. The available records map to those clauses: selected-timeout proof is recorded for Windows and Linux in `_verify/P4-62-SELECTED-TIMEOUT-2026-10-09.md`; the paused-clock worker deadline and reconciliation are recorded in the MDP/TG4-32 history; this Windows record supplies all five stages and raw cold/warm samples; and the feature-gated actual offline CLI Fleet test record reports the success/deadline/shutdown scenarios passing 5/5 on Windows. These records are citations only; I did not execute them.

Older notes that list paid-provider CLI execution as open belong to P4-50's separate live-provider/observability acceptance. macOS sandbox support and launcher proof belong to P4-61/TG4-31. The Testing Gates contract defines `PLATFORM-TRIPLE` as a multiplier for gates tagged `[portability]`; TG4-32 is tagged `[reliability/performance]`, so that rule does not make a macOS timing benchmark part of TG4-32. The broader Part 6 exit line (`TESTING_GATES.md:390`) separately requires an end-to-end task on all three OSes; that remains a phase-level criterion and does not add a cross-platform benchmark clause to P4-62/TG4-32.

Remaining review limit: this was source/document inspection only. No test, build, or end-to-end run was performed, and no gate or phase status is asserted here.

Scope note: the instruction prohibited version-control operations, so I reviewed the checked-out source and named evidence/plan records without comparing the file to its Git base. Findings above are about the live source behavior; a changed-lines-only audit was not possible under that constraint.
