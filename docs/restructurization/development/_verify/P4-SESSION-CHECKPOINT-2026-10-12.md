# P4 session checkpoint — 2026-10-12

## Current status

The active goal is to complete every P4 item and TG4 gate in MASTER_DEVELOPMENT_PLAN.md and TESTING_GATES.md, maintain READINESS_MATRIX.md, then read and complete P4_EXPANDED_GOAL.md. P5 and P6 remain downstream; they have not started.

Live checklist count: **41/59 P4 tasks checked (18 open)** and **19/33 TG4 gates checked (14 open)**. No checklist item closed during this continuation. P4-49 is the owner-run Calendar desktop acceptance; the other 17 open P4 tasks remain engineering work.

Open P4 tasks: P4-11, P4-14, P4-15, P4-18, P4-19, P4-20, P4-21, P4-36, P4-50, P4-51, P4-53, P4-54, P4-56, P4-57, P4-60, P4-61, P4-62, P4-49.

Open TG4 gates: TG4-07, TG4-08, TG4-09, TG4-16, TG4-18, TG4-19, TG4-23, TG4-24, TG4-25, TG4-27, TG4-28, TG4-30, TG4-31, TG4-32.

## New verified work

- The feature-gated offline production CLI Fleet harness passes 4/4. It exercises actual CLI startup, scripted workers and QC, a Rust fixture build, durable task/epic completion, deadline/shutdown behavior, log and analytics export readback, and unmarked-profile denial. Exact evidence and command: P4-50-CLI-OFFLINE-FLEET-2026-10-12.md. This is not live-provider success and does not close P4-50/TG4-24.
- Windows shell hooks now run through the selected-workspace write lease and existing experimental SBOX launcher. Focused tests pass 5/5 for shell-hook routing/admission, 1/1 for failed-stop lease retention/retry, and 1/1 for dropped-child ownership until the OS confirms an empty process job. Provider/database environment variables were removed. Rust formatting and diff checks pass.
- On failed process-tree confirmation, the bounded cleanup owner retains the child, cleanup admission and write lease; the workspace remains quarantined until the OS confirms the job is empty. Unsupported non-Windows shell-hook execution stays fail-closed.
- Detailed shell-hook evidence: P4-61-SHELL-HOOK-BOUNDARY-2026-10-12.md. P4-61/TG4-31 remain open because complete command/corpus coverage, guarded staging/copy-back, locked dependency staging, filesystem/network negatives, Linux/macOS production runners, and loopback smoke capability are not yet proven.
- MASTER_DEVELOPMENT_PLAN.md, TESTING_GATES.md, READINESS_MATRIX.md, GOAL.md and PROJECT_MEMORY.md now include this continuation state.

- Linux staging planner correctness: root/same-parent metadata changes from unrelated host entries no longer cause false conflicts, while a replaced root and permission change still refuse. Ubuntu WSL staging module passes 8/8; production stage lifetime, apply/recovery, and ProjectCommand integration remain absent. Evidence: P4-61-COPYBACK-WIRING-NEXT-2026-10-12.md. Counts unchanged.
## Remaining constraints and next work

The worktree is on main with extensive mixed P4 modifications and untracked files from prior work. Do not use git add -A or commit unrelated/unverified WIP. Verified offline CLI unit97f4a17 is now committed and pushed to origin/main; other mixed P4 code remains uncommitted. Commit coherent verified phases after reviewing the complete staged diff; main push authorization was already granted for this chat.

The offline CLI proof does not authorize another paid provider run. No second live-provider Fleet attempt is authorized in the current state. The owner Calendar acceptance remains the final P4 action after engineering gates are ready. Do not access C:Playgroundarcana-calendar or claim desktop Calendar acceptance.

Continue from the exact open tasks and current evidence records above. Do not rerun successful offline tests absent code changes. P4_EXPANDED_GOAL.md remains unread until the core P4 plan and gates close, as directed.
- P4-57 closure-gap reconciliation: the named annual worker-history proof is already present in source and has Windows/Linux evidence (4,400 leap-year/DST instants plus legacy row through monthly rollup/readback). The closure-gap report now records that this specific pair is resolved; P4-57/TG4-19 remain open for the complete pair inventory and whole-contract coverage. No checkbox changed.

## Continuation after the 2026-10-12 checkpoint

- Verified the write-admission cancellation path after the source-only lease review: `ToolExecutor::execute_batch` acquires the selected-workspace lease before persisting an operation intent. `cancelling_write_lease_admission_creates_no_unknown_mutation_intent` passes on Windows and reopens `runs.db` to confirm there is no journal entry and no unresolved task side effect when cancellation wins before admission.
- The Windows cleanup owner now reports its retrying phase while retaining the process, lease and admission. `failed_stop_keeps_real_write_lease_and_retry_releases_it_after_confirmation` passes and asserts `Retrying`, then `Confirmed`; the real sandbox-child drop test and the five shell-hook routing/admission tests pass again.
- A fresh Luna High review found no confirmed scoped security defect. It identified a shutdown proof gap: the process-local cleanup owner has no production drain; Windows job close is configured to kill descendants, but cross-process lease exclusion during that termination window is not established.
- These are production-slice proofs, not P4 task or TG4 gate closures. Current tally remains P4 **41/59** and TG4 **19/33**. The complete P4-61 sandbox acceptance remains open; P4-18/P4-19/P4-53 and their gates also retain their broader outcomes.

## Closure sequence after owner direction to stop repeated platform checks

The active goal and safety/platform requirements are unchanged. P4 has17 engineering tasks and the separate owner Calendar acceptance. Complete shared outcomes rather than treat these as18 independent projects:

| Outcome | Open tasks it owns | Remaining completion proof |
|---|---|---|
| Task continuity and recovery | P4-19/20/36/53/54 | Real multi-mutation interruption, complete task/repository/checkpoint snapshot, replacement-worker reconciliation, safe no-duplicate resume and Supervisor health readback. |
| Untrusted project execution | P4-11/14/15/18/61 | Production runtime allowlist/dependency staging, recoverable staged copy-back, all child cleanup paths, positive confined smoke, and required Windows/Linux/macOS command/security matrix. |
| Observability and failure contracts | P4-50/51/56/57/62 | Remaining producer-to-UI/token-result/cost/failure proofs, successful smoke tool, complete timestamp pair inventory/annual coverage, and final cancellation/security evidence. |
| Desktop handoff and consent | P4-21/60 | Real desktop IPC producer/event/IDE/run and consent/revocation boundaries; current Windows driver attachment still lacks a working endpoint. |
| Owner final acceptance | P4-49 | Calendar scenario only after engineering readiness; owner runs it and returns logs. |

Actual offline CLI Fleet now passes5/5 Windows and normal builds execute0 test-seam cases; core/CLI check passes. Current suite and two review corrections supersede the earlier4/4 receipt. Two redundant Linux P4-14/15 checks passed again (11.87s/8.18s), but ADR0354 already established them; the stale P4-15/TG4-08 wording is corrected, no new platform proof is claimed. Timestamp pair inventory is now assigned as the missing required P4-57 deliverable to a Luna High agent; do not repeat the already-proved monthly-history test. Counts remain41/59 and19/33. Do not access Calendar or spend another provider budget.

## Current continuation handoff

- Pushed97f4a17: opt-in offline CLI Fleet proof and review corrections. Windows5/5; ordinary integration target0 tests; normal core/CLI check passes. The two original Windows review findings are resolved. Unix harness-overrun descendant cleanup is still a limit, not a passing claim.
- P4-57 production pair inventory is complete for TasksDb, RunsDb, SessionStorage, ConfigDb/Hooks, Analytics/events, Memory, WorkersDb, PlanDb and GraphStore: _verify/P4-57-PRODUCTION-PAIR-INVENTORY-2026-10-12.md. This is the missing inventory deliverable, not annual execution or a closed gate.
- ADR0370 proposes an isolated test-only clock fixture for generated-now APIs. The owner question is pending; do not build that shared testing pattern until answered. Existing annual Session/Analytics/hook-log proof must be reused; the new inventory lists exactly which writer/reader pairs still need year-wide proof.
- All resumed Windows/WSL test commands finished. No live provider request, owner Calendar, server or listener was launched. No task/gate count changes:41/59 P4,19/33 TG4. Goal remains active, core P4 precedes the still-unread expanded goal.
