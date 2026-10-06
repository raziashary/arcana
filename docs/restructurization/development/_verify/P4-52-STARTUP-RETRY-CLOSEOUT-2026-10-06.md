# P4-52 / TG4-26 startup retry closeout — 2026-10-06

**Result: GATE-PROVEN.** Owner approved closure against the retry clauses, with successful live CLI Fleet completion retained under P4-50/TG4-24 (ADR-0319). Synchronous setup is bounded by retry admission and explicit overrun reporting, not a forcibly interrupting deadline (ADR-0302/0304). This does not close P4-53 mid-run continuity.

| Required outcome | Production-path proof |
| --- | --- |
| Exponential full jitter; attempt/admission caps and post-delay check | `startup_backoff_uses_bounded_exponential_full_jitter`, transient/exhaustion tests and slow setup/late-backoff admission case exercise the existing retry function used by RecipeRunner. |
| Cancellation interrupts retry delay | `cancellation_during_backoff_stops_before_another_attempt` verifies no subsequent attempt. |
| Permanent failure returns once with status | `permanent_setup_failure_is_not_retried`, fail-closed Vault test, and actual RecipeRunner permanent-failure JSONL read through `arcanactl logs export`. |
| Explicit synchronous setup overrun | `synchronous_setup_overruns_are_reported_without_another_attempt` covers slow success/failure and late backoff; real logger/CLI export observes `setup_overrun` and measured elapsed time. |
| One task/claim/budget identity after recovery | `model_produced_commit_runs_through_recipe_runner_and_host_guard` injects transient pre-provider setup failure, then drives the shipped recipe and AgentEngine to claim once, activate the saved task budget and perform one real guarded commit. |
| Exhaustion preserves saved task and daily budget | `exhausted_startup_preserves_assigned_task_budget_and_daily_reservation` compares assignment/claim, saved cap/usage, persisted daily spend and the exact reservation identity before/after four failed attempts; no provider dispatch. |
| Do not replay after a side effect | `failure_after_host_commit_does_not_restart_or_replay_the_effect` fails the generation after a real commit, asserts one commit and durable journal outcome, quarantines the task, then scheduler-kicks again and verifies no more provider requests. |
| Observable attempt/outcome/delay and durable run correlation | `startup_retry_events_include_the_durable_worker_run_id` asserts scheduled/recovered events, attempt fields and the same durable run. Permanent/overrun outcomes additionally pass production persisted CLI readback. |

## Current execution

On 2026-10-06, Windows and Ubuntu WSL each pass:

- `cargo test --offline -p arcana-core --lib startup_retry_tests --locked -- --test-threads=1`: **11/11**, Windows 7.12s, Linux 7.18s.
- `cargo test --offline -p arcana-core --lib production_git_commit_recipe_tests --locked -- --test-threads=1`: **3/3**, Windows 4.19s, Linux 5.59s.
- `cargo test --offline -p arcanactl --test startup_retry_logs --locked -- --test-threads=1`: **1/1**, Windows 6.75s, Linux 8.21s.

All profiles, repositories, locks and databases are disposable; providers in these tests are scripted. No paid request or Calendar access. Compiler/test harness runtime is excluded from the reported test durations. Safe ignored raw logs are under `target/p4-ci-checks`; the current Linux whole-workspace CI run 37444400127 also passes. No production retry code changed after these checks; subsequent changes are tests and documentation.

Independent GPT-6 Luna High review is retained in `_verify/P4-52-CLOSEOUT-REVIEW-2026-10-06.md`. It found an added status dependency on successful paid CLI Fleet, which the owner has now explicitly separated. Scheduled/recovered telemetry is captured-event proof; this report does not claim persisted export of every retry outcome. The gate's required permanent/overrun statuses have actual persisted reader proof.

After this and P4-59 closure: **P4 40/59; TG4 19/33**. Mid-run snapshot/reconciliation and replacement-worker continuity remain P4-53/P4-54 work. The one earlier paid CLI trial created no child task, so successful production Fleet completion remains unproven. The owner-only desktop Calendar acceptance remains pending.
