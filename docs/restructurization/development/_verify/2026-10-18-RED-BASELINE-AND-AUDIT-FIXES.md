# 2026-10-18 — red-baseline repairs and the audit-derived owning-layer fixes

**Scope:** every code fix made in this session that was *not* covered by the TG4-24/TG4-27
receipts. One row per logical unit, each with the evidence that can fail and the audit that
found it. All commits are local; nothing here has been pushed.

## 1. What was red, and why it mattered

Closing P4 needed a trustworthy baseline, so the pre-existing reds were triaged first. Each of
the following was confirmed to fail identically on a clean worktree of the previous HEAD (i.e.
red on `main`, not a regression from this session's work):

| Red | Count | Owning layer |
|---|---|---|
| `project_command` suite | 4 | test fixtures (two of them) + a superseded test + a process-global dependency |
| `recipes::runner::production_git_commit_recipe_tests::shipped_worker_*` | 3 (co-run only) | **production**: `recipes/scheduler.rs` |
| `db::runs` legacy fixture | 1 | test fixture disagreeing with the real ladder |
| production telemetry labels unregistered | 1 | `observability/telemetry_labels.rs` allow-list |
| `write_file` checkpoint-failure tests | 2 | test injected failure at the wrong layer |

## 2. Fixes, in commit order

| Commit | Unit | Proof |
|---|---|---|
| `49e87e0` | Register the 7 real production telemetry labels at their sorted positions | `fixed_production_telemetry_labels_are_registered_and_sorted` 1/1 |
| `eea621e` | Derive the `db::runs` legacy fixture from the real ladder (`run_ladder(&conn, "runs", &LADDER[..3])`) instead of a hand-written "v3" schema that skipped rung 2 | `db::runs` tests green |
| `c00a6f0` | Inject the checkpoint failure at the checkpoint layer (a `BEFORE INSERT` trigger on `checkpoints` raising ABORT) rather than corrupting `.arcana/config.db`, which ADR-0380's fail-closed hook reader denies first | both `write_file` checkpoint tests 2/2 |
| `ce909be` | Three `project_command` reds repaired where the defect lived (see §3) | `cargo test -p arcana-core --lib --locked -- --test-threads=1 project_command` → 40 passed, 0 failed, 2 ignored, exit 0 |
| `a2e7aa8` | **Production defect**: release the process-wide worker slot after a successful kick (`KickSlotGuard::skip_recovery`) | see `_verify/TG4-27-SLOT-LEAK-REPEATABILITY-2026-10-18.md`; trio 3/3 in one process |
| `1e03e08` | Prove telemetry persistence/export failures cannot harm core work (TG4-24 clause) | see `_verify/TG4-24-TELEMETRY-FAILURE-ISOLATION-2026-10-18.md` |
| `894bbe0` | **Production defect**: one provider construction site (`factory::build_inner`) + a source-scan guard for the drift class | `provider::` 153/153; `provider::factory::tests` 5/5 |
| `5953f8c` | `memory.db`/`arcana.db` reported `database="other"` in migration telemetry (a TG4-24 identity clause) | `db::migrate` 19/19 |
| `60c5534` | Delete the dead `mod dirs_next`; correct the db module map | `db::migrate session::storage` 24/24; both crates compile clean |

## 3. Notable judgements (recorded, not silently taken)

- **`project_command` fixtures sat inside Arcana's own cargo workspace.** Two fixtures created
  their cargo project under `arcana-core/target/`, so cargo refused ("current package believes
  it's in a workspace when it's not") and exited before rustc ran: the diagnostics test saw
  `BUILD_FAILED` instead of the planted `E0425`, and the timeout test saw `failed` instead of
  `incomplete`. Fixed at the fixture (`[workspace]` table), matching the already-correct
  checked-in `tests/fixtures/fx-repo-rust`. Proved directly: the same project shape with
  `[workspace]` emits `E0425`.
- **A superseded test was repointed, not deleted.**
  `windows_batch_package_manager_shim_runs_with_fixed_argv` asserted pre-guard behaviour (a
  user-owned `.cmd` shim granted to project code). The guard deliberately refuses that. The test
  now pins the refusal/guidance **through the production launcher**, so the launcher boundary is
  still exercised; the fixed-argv batch path stays covered by the checked-in node corpus, whose
  `npm.cmd` lives under `Program Files` and takes the same `cmd.exe /d /s /c` branch.
- **The corpus test's Python leg was order-dependent.** `python_runtime::stage()` is a
  process-global cache and the test never primed it, so it passed only when another test in the
  same process had staged first. It now pre-stages, as its mixed-batch sibling does.
- **The lost-ack trio's co-run failure was a production bug, not test interference.** Isolating
  tests per-process would have made the suite green while leaving a worker permanently
  un-dispatchable after one successful run. Fixed in production; recorded in ADR-0397.

## 4. Kept open deliberately

- `executor.rs` reconciliation envelope omits `outcome_unknown`; `journal_target_paths` returns
  empty for both "failed" and "genuinely pathless" (P4-53 clauses).
- Hook infrastructure failure is reported as a policy denial (P4-51 clause) — needs an
  infra-vs-policy distinction in `hooks/registry.rs`, which is an app-visible behaviour change.
- `do_not_retry` reconciliation skips `RecoverySnapshot::verify_state` (safe, but conflict
  wording is enforced only on continuation paths).
- 16 `HookType` variants have no firing site; `Tool::is_read_only` and `fire_with_logs` have no
  callers. Consolidating these is part of the refactor plan, not a drive-by edit.

## 5. Cost of being wrong here

Each of these was reproduced by execution before the fix and re-run after it. Where a claim
could not be made to fail on demand it is recorded as a limitation instead of a pass (e.g. the
mid-write export failure, the live-Fleet telemetry injection).
