# P4 Production-Path Audit

- Scope: Independent source audit of P4 fleet workflows against the active MDP,
  TG4 gates, and production readers/consumers.
- Date: 2026-09-28
- Review: GPT-6 Luna High subagent; findings were re-opened against current
  source by the coordinating agent before recording.
- Limit: Source review only. No tests, live DB/provider, app, or service were
  accessed by the reviewer. This report does not close or reopen plan rows.

## Findings

### High — QC promotion paths can bypass build evidence

`UpdateTaskStatusTool::execute` checks task-scoped passing evidence only when
the caller is not exempted by `promotes_on_behalf_of_others`. The exemption
includes the Supervisor. `RecipeRunner::reconcile_stranded_task` directly
promotes an in-progress task with `TasksDb::update_status`, and the Tauri
`update_task_status` command also calls the DB transition without requiring
passing evidence. The task-state trigger enforces legal transitions, not build
evidence.

- Plan/gates: P4-14, TG4-07.
- Source: `arcana-core/src/agent/builtin_tools/update_task_status_tool.rs`
  (`execute`, `promotes_on_behalf_of_others`); `arcana-core/src/recipes/runner.rs`
  (`reconcile_stranded_task`); `src-tauri/src/commands/task_commands.rs`
  (`update_task_status`).
- Required proof: each production route, including Supervisor promotion, refuses
  missing, unattributable, or failing task-scoped evidence and accepts matching
  passing evidence. Owner must settle the shared enforcement boundary before a
  cross-module change.

### High — persisted task budgets are not loaded by worker execution

`SetTaskBudgetTool` writes task-specific limits through `TasksDb`, but
`RecipeRunner::run_recipe` constructs the engine with only
`TaskBudget::default_worker()`. `AgentEngine::with_task_budget` exists and has
no production caller, so per-task settings are inert while the worker-level cap
remains active.

- Plan/gates: P4-29, TG4-15.
- Source: `arcana-core/src/agent/builtin_tools/set_task_budget_tool.rs`;
  `arcana-core/src/recipes/runner.rs` (`run_recipe`);
  `arcana-core/src/agent/engine.rs` (`with_budget`, `with_task_budget`).
- Required proof: load the claimed task's saved budget on the actual worker
  execution path, demonstrate that a low task limit trips its breaker, and
  prove the daily ceiling remains independently enforced. Runtime load timing
  and budget-channel questions remain owner-directed.

### Medium — normal assignment is not controlled by the deterministic scorer

The Supervisor recipe directs a worker matrix choice and uses `suggest_worker`
only for ambiguous tasks. `AssignTasksBatchTool::execute` validates and persists
the supplied worker but does not call the scorer. `TaskRouter::suggest_worker`
being available therefore does not prove that recorded outcomes affect normal
assignment.

- Plan/gates: P4-32, TG4-13.
- Source: `arcana-core/src/recipes/defaults.rs` (Supervisor dispatch protocol);
  `arcana-core/src/agent/builtin_tools/assign_tasks_batch.rs` (`execute`);
  `arcana-core/src/routing/mod.rs` (`suggest_worker`).
- Required proof: recorded outcomes measurably change an ordinary next
  assignment without model worker selection; any override is auditable. The
  routing override persistence contract remains owner-directed.

### Medium — unrestricted Bash remains alongside validated project commands

`register_for_mode(FastDevelopment)` exposes both `BashTool` and
`ProjectCommandTool`; Bash executes model-authored commands. Consequently the
deterministic command interface does not yet own all build/test execution, and
a Bash-authored build must not be treated as passing QC evidence.

- Plan/gates: P4-15, TG4-08.
- Source: `arcana-core/src/agent/builtin_tools/mod.rs`
  (`register_for_mode`); `arcana-core/src/agent/builtin_tools/bash.rs`
  (`execute`); `arcana-core/src/agent/builtin_tools/project_command.rs`.
- Required proof: settle the Bash/QC boundary, prove shell output cannot mint
  build evidence, and exercise the prescribed retry/exit-code contract. This
  audit does not infer owner approval for changing the shipped toolset.

### Medium — Windows `.cmd` manager shim needs a real process test

Manager discovery honors `packageManager`, then lockfile precedence, then the
documented npm fallback when metadata is absent. The unresolved proof is that a
resolved `.cmd` shim runs through `ProjectCommandTool` on Windows; source passes
the selected program to `tokio::process::Command::new`, but the required spawned
process behavior is not established by the existing unit tests.

- Plan/gates: P4-11, TG4-18.
- Source: `arcana-core/src/platform/command.rs` (`detect_node_manager`,
  `resolve_program`, `run_resolved_command`);
  `arcana-core/src/agent/builtin_tools/project_command.rs`.
- Required proof: create a local `.cmd` fixture and invoke it through the
  production tool on Windows, asserting the observed exit code. If it fails,
  repair the platform-owned launcher and rerun the behavior test.

### Medium — ACP recovery/idempotent side effects remain a proof gap

The durable message reader returns `pending` and `read` rows with stable IDs
until acknowledgement; the scheduler considers them recoverable. The audit did
not establish a confirmed defect in this behavior, but the production recipe
has not yet been shown to recover after interruption before ack without
duplicating side effects.

- Plan/gates: P4-23, P4-42, TG4-10, TG4-11, S4-04.
- Source: `arcana-core/src/db/tasks.rs` (worker-message reader/ack);
  `arcana-core/src/recipes/scheduler.rs` (inbox/recovery eligibility).
- Required proof: interrupt the real consumer before acknowledgement, let the
  scheduler redispatch, show the stable ID is replayed and the consumer handles
  it idempotently.

### Confirmed current wiring with remaining gate scope

The P4 production wiring for consent-aware QC and fleet procedural recall is
present in `AnalyzeDiffTool`, the provider wrapper, `RecipeRunner`,
`ToolExecutor`, and `AgentEngine`; this audit found no additional defect there.
TG3-26/TG4-16 still require the repository-diff and full outbound-consent
consumer proof. Project manager precedence/fallback also matches the owner's
direction. P5 graph consumers and P6/P7 interface/onboarding work remain later
phase ownership, not P4 defects.

## Coordinator follow-up (2026-09-28)

The Windows `.cmd` gap was verified by adding a fixture test through
`run_resolved_command`, the same process launcher called by
`ProjectCommandTool::execute`. The first run failed because the `cmd.exe /S /C`
command-line string did not preserve the quoted path when the temp directory
contained spaces; the outer quote pair was corrected. The passing focused test
`windows_batch_package_manager_shim_runs_with_fixed_argv` proves the shim sees
`run build`, prints it, and returns exit code 17. This resolves the audit's
Windows shim concern on the current host only. TG4-18 remains open for the
complete Rust/Node/Python corpora and Linux/macOS.

The P4-14, P4-29, and P4-32 findings were verified in current source and remain
open pending their shared-boundary/runtime/routing contracts and behavior
proofs. The Bash/QC enforcement and failure-streak choice for P4-15 is also
still pending. No cross-phase P1/P2/P3 gate was marked complete by this audit.

### Reconciliation after the production-path fixes — 2026-09-28

The preceding coordinator disposition is stale against later current-tree
changes; the original findings remain as historical evidence from the audit
snapshot, but their present status is superseded as follows:

- **P4-14:** SQLite migration 7 now rejects every direct `qc_review` promotion
  without the latest attributable passing build record. Focused tests cover
  direct SQL, `TasksDb`, RecipeRunner reconciliation, and Tauri status updates.
  P4-14 is checked in the MDP. TG4-07 remains open for process-exit and
  cross-platform worker evidence.
- **P4-32:** `AssignTasksBatchTool::execute` now invokes the deterministic
  `TaskRouter::select_worker_for_dispatch` path for automatic assignments;
  persisted outcomes affect the selected worker and audited overrides are
  excluded from learning. P4-32/TG4-13 are proven in the MDP and gate record.
- **P4-15:** the owner chose observed nonzero Rust-owned `project_command`
  build exits, with an observed pass resetting the streak; Bash remains for
  general work and cannot authorize QC. The stale “owner decision pending”
  wording above is superseded by ADR-LOG-RESTRUCT-0093/0094 and the partial proof at
  MDP P4-15/TG4-08; worker-provider diagnosis and retry remain open.
- **P4-29:** the owner chose activation after claim (ADR-LOG-RESTRUCT-0098).
  The saved task limit now
  enters the real `AgentEngine` run after a successful production claim, and
  the focused test proves it blocks/persists the next request; broad TG4-15
  interaction proof remains open.

These dispositions preserve TG4-07/TG4-08/TG4-15's remaining scopes. They do
not close the P4 phase or any unrelated gate.

### Independent checkpoint review (2026-09-28)

A separate GPT-6 Luna High read-only review traced P4-19/P4-20 through the
checkpoint schema, every direct file writer, runtime task identity, restore
commands, and TG4-23. It confirmed three implementation gaps: checkpoints are
session-associated rather than task-associated; pre-write snapshot errors are
logged and ignored; and restore overwrites the current file without checking
whether another task changed it afterward. Append and interface-spec writes
also lack the checkpoint hook, and retention currently groups snapshots by
file without task ownership.

Recommended target: persist verified task ownership and whether the file
existed before the write; fail the direct write if its snapshot fails; scope
retention by task; and refuse stale restores when the current file no longer
matches the state produced by that task. Do not derive task identity from
`session_id`. This design is not implemented yet. Its broader process boundary
depends on the pending P4-15 decision because Bash and `project_command` can
mutate files outside these direct tool hooks. P4-19, P4-20, and TG4-23 remain
open until that boundary and all required behavior tests are resolved.

### Separate IDE mutation-path finding (2026-09-28)

The P4-18 source sweep confirmed `write_file`, `edit_file`, `append_file`, and
`write_interface_spec` resolve paths through the shared workspace jail. It
also found a separate issue in Tauri IDE file operations: `rename_file` joined
its destination directly to the workspace, and `move_file` did the same for
its destination directory. The old `resolve_safe` helper only folded path
components, without resolving symlink ancestors. These commands are not agent
tools and therefore do not satisfy or invalidate P4-18 by themselves, but a
traversal path could make an IDE mutation escape the selected workspace.

The coordinator changed IDE mutations to use the shared canonical path jail
for source and destination paths, including the final move destination.
Focused tests prove traversal refusal and valid new/existing workspace paths
on Windows; the Unix symlink case is covered by a platform-gated test. This
repair does not expand P4-18's agent/process boundary or close its pending
gate.

### P4-11 platform corpus follow-up (2026-09-28)

The required `fx-repo-{rust,node,python}` corpora were not present in the tree,
and the existing three-OS CI build matrix did not execute project-command
intents. Checked-in fixtures now exercise Rust build/test/clippy/run, npm
build/test/lint/start, and Python pytest/ruff through `platform::command` and
the production `run_resolved_command` launcher. CI runs the focused test on
Windows, Linux and macOS with the required runtimes/tools installed. The exact
test passed on this Windows host, including its real `npm.cmd` route. This
closes the missing local fixture/CI wiring, but TG4-18 and P4-11 remain open
until Linux/macOS CI and the production worker/build path are evidenced.
