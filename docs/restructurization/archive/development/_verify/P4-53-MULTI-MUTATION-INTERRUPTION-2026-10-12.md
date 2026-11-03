# P4-53 Multi-Mutation Interruption Proof — 2026-10-12

Status: focused proof implemented and validated; limitations recorded below.


## Requirement and existing evidence

P4-53 requires task-attributed execution state, reconciliation before resuming, and explicit handling of non-idempotent mutations; its owner-expanded paragraph marks repository snapshot/checkpoint comparison, complete mutation capture, multi-mutation interruption, replacement-worker continuation, and duplicate-free resume as open. TG4-27 asks for real worker mutations followed by panic/cancellation/process loss, durable checkpoint/repository state and lost-ack reconciliation. This requested proof is intentionally narrower: it covers fail-safe interruption and scheduler quarantine, not full snapshot/resume closure.

The existing `shipped_supervisor_recipe_keeps_uncertain_operation_quarantined` test exercises the persisted Supervisor recipe through RecipeRunner/AgentEngine and the real reconciliation tool, then reopens RunsDb/TasksDb and proves a `do_not_retry` decision leaves the task in `needs_info` and the task effect barrier active. Existing `production_git_commit_recipe_tests` provides the actual scheduler → shipped `worker-backend-poll.yaml` → RecipeRunner/AgentEngine → `git_commit` host builtin fixture and verifies task and Git readback. The requested test must extend the latter execution path with two real file mutations and an isolated persistence fault.

Memory lookup found the Arcana navigation record; current governing docs and source are being used as authority. No implementation or gate status change is in scope.

## Test added

Added `shipped_worker_preserves_multiple_mutations_and_quarantines_lost_acknowledgement` in `arcana-core/src/recipes/runner.rs` only. It uses `test_support::with_home`, an isolated workspace and real `RunsDb`/`TasksDb`, the shipped backend polling recipe, a scripted streaming provider, and the actual scheduler/RecipeRunner/AgentEngine. The provider claims the assigned task, calls registered `write_file` twice on distinct files, verifies the first success and the second non-retry-safe unknown result, then returns a provider error to interrupt the run.

An isolated test-only SQLite trigger rejects the outcome append associated with `lost-ack.txt`, after its intent exists. The test reopens task/run/checkpoint readers and the workspace, checks both file contents, confirms the first journal outcome is acknowledged and the second intent remains `Unknown`/unresolved with the same run/task IDs, confirms both checkpoint rows carry the actual task ID, and starts a fresh scheduler/runner for a later kick. That kick must make no additional provider request and leaves the second file unchanged. Journal assertions inspect only bounded labels/path metadata; file payloads are not asserted through telemetry.

Validation is pending. The fixture is designed to prove interruption containment and durable readback only; it does not claim Git snapshot comparison, checkpoint restore safety, complete continuation state, a safe retry/reconciled-complete branch, or duplicate-free resumed execution after reconciliation.

## First prescribed run

The exact focused command compiled successfully and started the new test, then failed at the fixture assertion that treated the first historical `write_file` result as the second call's result. The request contains both prior tool results, so the provider script must select the result for its second call before evaluating the expected lost acknowledgement. This is a test-fixture selection error; the run has not yet established whether the trigger is reached.

The provider fixture now selects the second result by its `tool_use_id` (`write-lost-ack`), preventing the earlier acknowledged result in conversation history from satisfying the assertion. Rerunning the same prescribed command.

The second prescribed run correctly selected `write-lost-ack`, but still received a success result. This shows the trigger predicate did not intercept that outcome in the isolated ledger; it is still fixture wiring, not evidence of a production defect. I am revising the test-only trigger to target the second `write_file` intent by counting those intents in the fresh database.

The third prescribed run reached durable journal checks, which means the second write outcome was rejected as intended and the prior assertions (actual file readback, task quarantine, and task-attributed checkpoints) passed. It then failed because the assertion assumed a workspace-relative journal target string. I am making the lookup accept the persisted path suffix and will keep the exact file-content/readback checks.

Public reader output now identifies the production gap precisely: both task-attributed `write_file` intents have `target_paths=[]`; the acknowledged first outcome consequently has `changed_paths=[]`. The actual filesystem writes and task-attributed checkpoint references are present. Since production code is out of scope, the proof asserts the two ordered journal outcomes and task/run identity, and separately checks checkpoint file references and task IDs. This leaves complete changed-path capture explicitly unproved and reported.

## Final validation and result

Prescribed command (the listed variables were removed from the Cargo process environment before launch):

```powershell
cargo test --offline -p arcana-core --lib shipped_worker_preserves_multiple_mutations_and_quarantines_lost_acknowledgement -- --nocapture --test-threads=1
```

Result: **passed, 1 test; 0 failed; 1,114 filtered out** (Windows, 1.17s test runtime). Compilation reported 45 existing warnings; no new test warning was reported.

The passing test proves the shipped worker recipe claims the real assigned task through RecipeRunner/AgentEngine and runs two registered `write_file` handlers. A test-only trigger in the isolated `runs.db` rejects the outcome insert for the second `write_file` intent. The provider sees the non-retry-safe unknown result and is then interrupted. After reopening the workspace and public DB readers, both actual file payloads remain, both checkpoint records identify the real task and file references, the worker run and both journal entries carry the real task/run identity, the first outcome is `Ok`, and the second remains `Unknown`/`Unresolved`. The task is `needs_info` with its assignment preserved and claim metadata cleared. A fresh scheduler and runner receive a later actual kick; the provider request count does not increase and the second file remains unchanged.

## Initial finding and final status

The first runs exposed empty journal target paths on Windows. The coordinator traced the mismatch to the canonical `\\?\` workspace root versus the DOS-form jailed target and fixed normalization in the separately owned production path; ADR0372 records that change. The final focused test retains strict `target_paths` and `changed_paths` assertions and passes against the corrected path.

This is interruption containment and persistence evidence. It does not prove repository HEAD/branch/index/worktree comparison, snapshot reconciliation or task-scoped restore, continuation after reconciliation, or duplicate-free resumed execution. P4-53/TG4-27 remain open; no task/gate status was changed.
## Root integration verification

The exact same credential-free focused test passes under root1/1 (1.26s test runtime;2.00s cached compilation) after the production path repair. Normal core/CLI cargo check passes11.27s. The new test received a mechanical rustfmt pass; workspace cargo fmt --all -- --check and git diff --check pass. Exact receipt: ignored target/p4-ci-checks/p4-multi-mutation-final-2026-10-12.log. The independent Luna High scoped review finds no confirmed defect; the new real-consumer case uses a local Windows workspace, so UNC journal extraction is not directly executed by this case (shared prefix conversion already has UNC helper coverage). No whole task/gate is closed, and the pending snapshot/reconciliation contract has not been implemented.
