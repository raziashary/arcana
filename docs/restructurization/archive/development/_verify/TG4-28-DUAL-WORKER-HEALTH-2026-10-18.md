# TG4-28 / P4-54 dual-worker failure visibility + TG4-23/P4-20 restore smoke — 2026-10-18

One new module `recipes::runner::dual_worker_failure_health_tests` in
`arcana-core/src/recipes/runner.rs` (test additions only; strictly additive), driven by a real
scripted provider through production RecipeRunner/AgentEngine/executors:

`dual_worker_failures_persist_distinct_classes_visible_to_supervisor_without_duplicate_dispatch`
covers the four test-work TG4-28 clauses:
1. **Two workers, distinct failure classes, both persisted:** worker A fails mid-run after a
   real journaled write (terminal non-retryable provider failure → persisted
   `failure_class="permanent"`, journal intent + task-attributed checkpoint + task held at
   `needs_info`); worker B fails at startup with exhausted retries
   (persistent `failure_class="io"`, zero provider dispatches, observed `phase = retrying`).
2. **Supervisor-visible readback through the real registered `list_worker_health`** (via the
   FastDevelopment registry + production ToolExecutor): per-worker run record, task health
   (`needs_info`), journal outcome, checkpoints count, and the failure-class distinction are
   exercised behaviorally, not by grep.
3. **Paused/non-runnable exclusion:** a paused worker appears in the roster read but cannot be
   selected by the production routing path and is never dispatched.
4. **Duplicate-execution prevention:** a non-owner worker's claim attempt on another worker's
   assigned task is refused; the owner's recompliant retry is refused by the status gate.

`task_scoped_checkpoints_isolate_restore_between_tasks` covers the P4-20/TG4-23 fleet
write/restore smoke clause: two worker tasks each write workspace files under
task-attributed auto-checkpoints; restoring task A's checkpoints returns A's files to their
pre-A state while task B's later write survives untouched; a stale cross-task restore is
refused ("another task or session checkpointed this file afterward") — the enforced fail-closed
rule now has the fleet-flow exercising test.

Validated: `cargo test -p arcana-core --lib --locked -- dual_worker -- --test-threads=1` → 2/2,
exit 0 (2026-10-18); `rustfmt --edition 2021 --check` clean on the file.

**Contract notes recorded during the build:** the mid-run fixture uses a terminal provider
failure to guarantee a persisted class — a retryable 503-shaped overload would retry instead of
persisting, so the literal failure-class values exercised are `"permanent"` (mid-run) and `"io"`
(startup) rather than a 503 end-to-end scenario. Post-failure landing is the production
`recover_interrupted_worker_run` behavior (`needs_info` + released PID claim), asserted, not
assumed. `write_file` in this fixture used `overwrite: true` where required by production
policy. What remains open for P4-54/TG4-28: worker parent/child links and the bounded
temporary-worker creation flow (both code-work, owner-scope flagged).
