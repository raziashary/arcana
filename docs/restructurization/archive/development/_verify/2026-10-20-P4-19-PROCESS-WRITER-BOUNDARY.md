# P4-19 / P4-20 / TG4-23 — process-writer coverage on the attributable recovery boundary — 2026-10-20

Owner ruling recorded as `ADR-LOG-RESTRUCT-0403` (option A): P4-19/P4-20 close on the
**boundary contract** for process writers; per-file content reversal of a file a process
rewrites is not claimed here and stays with P4-53/TG4-27's repository-snapshot clause.

This unit adds test coverage only. No production behaviour changed, no schema change, no
new dependency, no new mechanism.

## What was already true (and where it is proven)

| Fact | Proof |
|---|---|
| The four file tools checkpoint before mutating (`write_file`, `edit_file`, `append_file`, `write_interface_spec`) and refuse when the checkpoint write fails | `production_tool_executor_claim_identity_blocks_write_after_checkpoint_failure` (2026-10-04) |
| Every non-Board tool effect, including `FreeformShell` and `Execute`, passes `ToolExecutor::begin_operation_journal` → a task-attributed intent + `RecoverySnapshot` (workspace fingerprint, Git HEAD/branch/index digest, root identity, the task's checkpoint ids) before the handler runs, and is refused (`operation_intent_unavailable`, `side_effect_outcome_unknown`) when that evidence cannot be persisted | `operation_journal_intent_failure_blocks_the_mutating_handler` — but only with a `Write`-effect stub |
| `bash` refuses a worker process before a task claim | `worker_process_is_refused_before_a_task_claim` |
| The executor's task identity is set from the host's claim response in the full fleet loop | `engine.rs` `tool_executor.set_task_id(active_task_id)` — single production site; asserted in the shipped-worker flow by `dual_worker_failures_persist_distinct_classes_visible_to_supervisor_without_duplicate_dispatch` (`journal task_id == claimed task`, task-attributed checkpoint) |
| Restoring task A's checkpoint cannot revert task B's writes, and a stale cross-task restore is refused | `task_scoped_checkpoints_isolate_restore_between_tasks` (2026-10-18) |

## What was genuinely open, and what this unit closes

1. **The boundary was never proven for a real process effect** — only for a stub declaring
   `ToolEffect::Write`. Three tests now drive the **registered** `bash` tool through the
   production `ToolExecutor`, including the per-call host approval a freeform shell requires
   in Autonomous mode, under a real claim (`worker-backend`, PID claim on an isolated
   `TasksDb`) and a durable worker run (`RunsDb::begin_worker_tick`):

   - `agent::builtin_tools::bash::process_sandbox_tests::shell_mutation_without_a_persistable_recovery_boundary_never_runs`
     — a SQLite trigger rejects every `operation_journal` intent insert. Asserts the sentinel
     file the shell would create does **not** exist, the refusal is
     `operation_intent_unavailable`, and no intent row survives that could later read as a
     duplicate effect.
   - `agent::builtin_tools::bash::process_sandbox_tests::unresolved_prior_shell_effect_blocks_a_new_shell_mutation`
     — a prior `bash` intent on the same task is left unresolved and its run closed as
     `interrupted`; a replacement run's shell is refused with
     `operation_reconciliation_required` and writes nothing.
   - `agent::builtin_tools::bash::process_sandbox_tests::shell_mutation_records_a_task_attributed_recovery_boundary`
     — a real shell write succeeds and persists exactly one journal entry whose `task_id`
     **and** `before_snapshot.task_id()` are the claimed task (not the session),
     `operation = "bash"`, `effect = "FreeformShell"`.

2. **The second production process writer had no named claim-refusal proof.**
   `agent::builtin_tools::project_command::tests::project_command_process_is_refused_before_a_task_claim`
   covers both arms against a detectable Cargo fixture: no claim → "Worker mutations require a
   task claim"; a task claimed by another worker → "no longer owns the in-progress task"; and
   the foreign task has no `project_build_batches` row afterwards (the refusal happens before
   any batch row, sandbox admission or child spawn).

`git_commit`, `check_file` and `e2e_probe` share the same executor gate by **effect**, not by a
per-tool copy (`ToolEffect::Execute`), so they are covered by the same seam rather than by a
name-specific test.

## Falsification (each test fails when the gate it claims is weakened)

| Probe | Result |
|---|---|
| `begin_operation_journal`: `record_operation_intent_with_snapshot(..)?` → `.unwrap_or(0)` (swallow the store failure) | Both boundary tests failed; the first failed at *"the child must never spawn when its task-attributed boundary is unpersistable"* — the shell ran and created its sentinel, and the post-run outcome write returned `side_effect_outcome_unknown`. Probe reverted. |
| `journal_task_id`: return `None` for `bash` | `shell_mutation_records_a_task_attributed_recovery_boundary` failed with `left: None, right: Some(1)` on *"a process write is attributed to the claimed task, not its session"*. Probe reverted. |
| `project_command`: wrap `require_current_task_claim()` in `if false` | `project_command_process_is_refused_before_a_task_claim` failed on *"an unclaimed worker may not build"* — the request proceeded into the build path. Probe reverted. |

`git diff` after revert contains no probe remnants (`FALSIFICATION` / `if false` grep is empty).

## Test runs

- `cargo test -p arcana-core --lib --locked -- builtin_tools::bash --test-threads=1` → **15/15**
  (`test result: ok ... finished in 8.94s`), including the three new tests.
- `cargo test -p arcana-core --lib --locked -- agent::tool_executor --test-threads=1` → **22/22**
  (`finished in 21.28s`).
- `cargo test -p arcana-core --lib --locked -- shell_mutation unresolved_prior_shell_effect project_command_process_is_refused_before_a_task_claim --test-threads=1`
  → **4/4**, `exit=0`.
- `cargo test -p arcana-core --lib --locked -- builtin_tools::project_command --test-threads=1`
  → **38 passed / 0 failed / 2 ignored** (`finished in 738.42s`, `exit=0`). This module is the
  slow one: it runs real Cargo/Node/Python builds through the production launcher.
- `cargo test -p arcana-core --lib --locked -- --test-threads=4` → **1181 passed / 1 failed /
  3 ignored** (`finished in 2138.28s`). The single failure is the documented pre-existing
  concurrency coupling, not this unit: `project_command::tests::checked_in_ecosystem_corpora_run_real_intents_through_the_production_launcher`
  panics with *"Python staging failed and no user-owned fallback is selected: runtime files were
  not found"* only in the parallel run, while the serial module run above shows it `... ok`. That
  pair was already the known `--test-threads=4` failure set before this unit (W5 owns the
  order/concurrency coupling); this unit adds no shared state beyond the `with_home` pattern the
  touched modules already use.

## Deliberately not claimed (residual, named)

- **Process write content reversal.** No checkpoint row is created for a file a shell rewrote or
  created, so a task restore cannot return those bytes or remove a process-created file. The
  task-attributed `RecoverySnapshot` records the boundary (workspace fingerprint, Git
  HEAD/branch/index digest) so the change is *detectable and attributable*, not silently lost.
  Content-level capture belongs to P4-53/TG4-27 ("repository state: HEAD, branch/worktree,
  staged/unstaged and relevant untracked contents"), which is still open.
- **Taskless interactive sessions.** `begin_operation_journal` attributes an intent to a task only
  when `ctx.task_id` is set (worker runs). A user-attended PA shell has no task to attribute to;
  that path is covered by the per-call host approval prompt, not by a task-scoped checkpoint.
- **Observation, not changed:** `project_command`'s pinned-Ruff `install` intent returns at
  `install_pinned_ruff_for_host()` *before* `require_current_task_claim()`. It is a
  `ToolEffect::HostApproval` per-call-approved host-tooling install that does not mutate the
  workspace, so its missing claim gate is an approval-bypass question for P4-56/TG4-30, not a
  workspace-mutation hole. Recorded here so it is not rediscovered as new; changing an
  approved-consent flow is a P4-56 decision.
- Windows/Linux only. The three bash tests are `#[cfg(any(windows, target_os = "linux"))]`
  because they spawn a real sandboxed child; macOS production proof remains P4-18/TG4-31.

## Gate mapping

- **P4-19** — checked: verified task identity reaches every production file mutation path
  (four file tools + five effect-routed process tools), and a failed task-associated checkpoint
  blocks the write.
- **P4-20** — checked: failed snapshots block writes; task-level restore is isolated
  (A's restore does not revert B; stale restore refused); the process path's mutation snapshot is
  the task-attributed `RecoverySnapshot` (ADR-0403 residual named above).
- **TG4-23** — checked. Save, production reader, failed-save veto (file **and** process), task
  attribution, single `config.db` store (TG1-04/TG1-05 owner; unchanged here), `arcana/fleet`
  kept (no branch switching in restore), isolated restore.
- **TG4-30** — stays open (P4-56 owns the full attack matrix). This unit closes its
  *"worker file/process writes cannot begin without a task claim and attributable recovery
  boundary"* clause, with the negative cases above.
