# P4-53 / TG4-27 — repository-snapshot breadth (staged, unstaged, untracked) — 2026-10-20

Closes the TG4-27 clause *"repository state (HEAD, branch/worktree, staged/unstaged and relevant
untracked files)"* and reconciles the P4-53 plan row, which had gone stale against receipts through
2026-10-18.

## What was actually missing

`RecoverySnapshot` (`arcana-core/src/agent/recovery_snapshot.rs`) already persisted `head`,
`branch_digest`, `index_digest` and a workspace input fingerprint, and `verify_state` compared them.
But the only negative case on file was a HEAD advance, so "snapshot comparison" was proven for one
class, and nothing defined which untracked files count. The plan row still listed snapshot comparison,
cancellation/panic/process-loss, and replacement-worker continuation as open, contradicting
`_verify/P4-53-*` receipts from 2026-10-13/14/18.

## The contract, in two mechanisms

| Mechanism | Recorded | Catches |
|---|---|---|
| `GitSnapshot` | resolved HEAD, digest of the symbolic branch target, digest over every index entry (path, oid, mode, flags) | branch switch, HEAD advance, **stage-only** change (worktree bytes and HEAD unchanged) |
| workspace input fingerprint | deterministic hash over every input, excluding generated trees (`.git`, `target`, `node_modules`, venvs, caches, build output) | **unstaged** worktree edit, untracked **input** file |

"Relevant untracked files" therefore means: inputs count, generated dependency/build trees do not.
The exclusion list is the single owner of that rule (`platform/workspace_fingerprint.rs`), and it
already existed for build evidence — including those directories would make every build invalidate
its own evidence.

## Test added

`agent::builtin_tools::reconcile_worker_operation::tests::repository_worktree_and_index_changes_refuse_before_handoff`,
driving the registered `reconcile_worker_operation` tool (real ToolExecutor path, real git repo
fixture, isolated Arcana home). Each case plants the journalled effect first, so a clean workspace
would reconcile and any refusal is attributable to the tamper:

| Case | Tamper | Assertion |
|---|---|---|
| 1 unstaged edit | `seed.txt` rewritten; index asserted still equal to HEAD's tree entry | `reconciled` refused, `operation_reconciliation_rejected` / `recovery_conflict`, effect still unresolved |
| 1b retry admission | same tamper | `retry_safe` refused with the same category (separate fingerprint comparison) |
| 2 stage-only | index entry mode `100644` → `100755`; worktree bytes asserted unchanged | refused; then restoring the recorded mode **accepts** |
| 3 untracked input | `notes.txt` created; asserted untracked via `repo.statuses` | refused, effect unresolved |
| 4 generated control | `node_modules/pkg/index.js` + completed effect in `target/` | **accepted**, `operation_complete=true` |

## Falsification (one arm at a time, then reverted)

| Probe | Result |
|---|---|
| Drop the other-inputs comparison in `Reconciled` | Case 1 failed: the tampered workspace reconciled (`operation_complete:true`). |
| Drop `current.git != self.git` in `verify_state` | Case 2 failed: the staged mode change reconciled. Case 1 still passed, so the arms are independent. |
| Remove `node_modules` from `GENERATED_DIRECTORIES` | Case 4 failed: generated dependency content blocked a verified completion. |

## Validation (current tree, Windows, `--test-threads=1`)

- `reconcile_worker_operation` → **6/6** (6.5 s), including the new case.
- `workspace_fingerprint` → **2/2**.
- Re-run of the surrounding P4-53 evidence the row depends on, to avoid resting on stale status:
  - `shipped_worker_` → **4/4** (53.5 s) — error, panic and cancellation variants plus the
    recipe-claim test.
  - `dead_worker_process_keeps_unknown_write_for_supervisor_handoff` → **1/1** (0.6 s).
- `rustfmt --check` clean on all three touched files.

## Plan-row correction

The P4-53 row's open list is replaced with the verified state: snapshot/Git/checkpoint comparison,
cancellation/panic/process-loss modes, in-process replacement-worker continuation and no-duplicate
resume for this effect class are proven (citing the module results and the 2026-10-13/14 receipts).
Still open: **whole-app restart** with the shipped Supervisor consuming the restart kick, **broader
mutation classes** (`verify_state` accepts `write_file` only), one-use retry admission beyond the
file-write pair, and cross-platform proof.

## Deliberately not claimed

- Whole-app process loss across restart; the dead-process case still recovers in the parent test
  process.
- Git commit, build/process and MCP/network effects: no snapshot expectation exists, and
  `verify_state` refuses any operation other than `write_file` rather than pretending to verify it.
- File modes are not in the fingerprint; only the index digest observes a mode change.
- No live provider, no user database, no network.
