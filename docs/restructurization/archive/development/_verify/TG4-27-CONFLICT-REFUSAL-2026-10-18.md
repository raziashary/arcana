# TG4-27 / P4-53 conflict-refusal clause — 2026-10-18

Two focused tests added to
`arcana-core/src/agent/builtin_tools/reconcile_worker_operation.rs` (test module only; strictly
additive) driving the registered Supervisor tool through a real ToolExecutor:

1. `git_state_change_between_intent_and_handoff_refuses_reconciliation` — a real `git2` repo
   advances HEAD after the operation intent is journalled (`RecoverySnapshot::capture` holds the
   seed HEAD/branch/index digests) and the task is interrupted to `needs_info`. The Supervisor's
   `reconciled` and `retry_safe` decisions are both refused with
   `error.code = "operation_reconciliation_rejected"`, `error.category = "recovery_conflict"`
   (via `RecoverySnapshot::verify_state`). The unknown intent stays `Unresolved`
   (`operation_journal_page` length 1, actor/reason `None`; `unresolved_task_side_effect`
   returns the cursor), and two retry-barrier shapes are proven:
   - a plain same-task write is refused with `operation_reconciliation_required` /
     `reconciliation_required` before any file write or new intent row; and
   - a write carrying the unresolved `recovery_intent_cursor` is also refused — with
     `operation_intent_unavailable` / `durability`, because no one-use retry grant was ever
     appended (the cursor path bypasses the barrier; the refusal outcome is the same).
2. `untampered_workspace_accepts_supervisor_verified_reconciled_decision` — differential control
   in the same fixture: restoring the recorded Git state (real `git reset --hard`) lets the same
   tool accept the decision (`retry_permitted = true` / `operation_complete = true` variants with
   audited supervisor actor + reason), proving refusal was specific to the Git change.

Validated: `cargo test -p arcana-core --lib --locked -- reconcile_worker_operation -- --test-threads=1` → 5/5, exit 0 (2026-10-18). `rustfmt` clean on the file.

**Flagged for owner judgment (not fixed):** `do_not_retry` reconciliation decisions do not run
`verify_state` at all (`runs.rs` reconciliation append only verifies for continuation decisions),
so a Git-tampered workspace still records the hold — safe (intent stays unresolved), but the
TG4-27 "conflict refusal" wording is currently enforced only for continuation paths. Related
cosmetic gap: a lost/never-granted retry admission surfaces as the generic
`operation_intent_unavailable` durability envelope rather than naming the missing grant.

**Suite-hygiene flag (pre-existing, not this batch):** the three shipped lost-ack tests
(`recipes/runner.rs::production_git_commit_recipe_tests`) each pass 1/1 (their 2026-10-14
receipts) but mutually interfere in one process — running any two together breaks the other's
replacement-worker build spawn (`command_spawn_failed`) on this Windows host. The interference
reproduces on a clean HEAD worktree as a distinct missing-untracked-`native/` artifact error, so
the receipts' individual-run proof stands; treat the pair/trio co-run behavior as an unproven
suite case until root-caused (suspect: leaked process-global launch/sandbox state between tests).
