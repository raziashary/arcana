# P4 session checkpoint — 2026-09-26

**State:** Implementation in progress. Both source/doc refutation audits and
owner rulings are reconciled. P4 is not complete. Focused task/status gates
have passed and are checked in the governing plan; full fleet smokes and
cross-platform proofs remain open.

## Baseline and scope

- The P4 turn began on clean `main` at `9cc0433` (`docs(p8): align closeout
  smoke with coding-only scope`), pushed to `origin/main`.
- The governing scope is `MASTER_DEVELOPMENT_PLAN.md` §10, “Phase P4 — The
  Fleet,” with rules in §0; the coding-only project goal is in `GOAL.md`.
- Active P4 work comprises tasks P4-01…P4-44 and P4-24a (P4-31 was already
  checked by earlier work and needs its current proof wording reconciled),
  smokes S4-01…S4-05, and TG4-01…TG4-23 / the TG-P4 closeout. Retired design
  verification is archived and is not part of P4.
- P4's exit target remains a coding task completed on Windows, Linux, and
  macOS, yielding a build-verified, reversible artifact in the selected user
  workspace that can be run from the IDE. The five S4 smokes and gates remain
  mandatory.

## Work completed in this turn

- Re-read the supplied AGENTS instructions and checked for additional local
  `AGENTS.md` files in `C:\`, the repository, and the governing docs folders;
  none were present.
- Re-read MDP §0's audit, evidence, live-data, ADR, dual-anchor, escalation,
  and closeout rules; inspected the P4 plan, fleet vision, P4 gate register,
  readiness row, and current project goal/memory/map.
- Started two **GPT-6 Luna** read-only pre-build refutation audits, as required
  by MDP §0.2m. The user then asked to stop urgently. Both agents stopped and
  saved partial reports:
  - `P4-PREBUILD-DISPATCH-TASKS.md`
  - `P4-PREBUILD-DELIVERY-COORDINATION.md`
- Both reports were completed by GPT-6 Luna auditors. The coordinator is
  reconciling findings against current source and the governing plan; reports
  are evidence, not a P4 readiness verdict.
- This resumed turn implemented strict typed worker status and its isolated
  migration; owner-checked atomic claims, one-task-per-worker, epic guards,
  transaction-scoped dependent unblocking/epic progression and per-entry
  assignment/routing history; plus persisted workspace/provider consent, audit,
  Settings grant/revoke, provider enforcement and deterministic QC diff
  fallback. P4-01…03, P4-07…10, TG4-01, TG4-03, TG4-04 and TG4-22 are checked
  with evidence in the MDP and gate register.
- Focused validation passed: `cargo fmt --all -- --check`; six
  `db::tasks::tests::p4_*` tests; 11 `db::workers::tests`; one Tauri
  `worker_status_tests`; two `egress_consent::tests`; deterministic diff
  fallback test; `cargo check -p arcana-core --locked`,
  `cargo check -p arcana --locked`, and TypeScript `tsc --noEmit`.
- Dispatcher continuation added the §14.5 five-minute recovery cadence, idle
  task/message checks, transient status-read retry behavior, ten-minute
  push-run timeout, RAII slot cleanup, claim recovery, worker-status reset,
  and Supervisor wake after autonomous dependency unblocking.
- The latest focused tests passed: all 9 `recipes::scheduler::tests`,
  `p4_interrupted_worker_claim_release_is_atomic_and_owner_checked`, and
  `autonomous_completion_unblocks_and_wakes_supervisor`. `cargo check -p
  arcana-core --locked`, `cargo check -p arcana --locked`, and the direct
  TypeScript `tsc --noEmit` check passed after scheduler/unblock wiring.
  Existing unrelated Rust warnings remain.
- Continuation added an initial task-associated build-evidence implementation:
  `tasks.db` migration v5 creates `build_evidence`; the worker shell result
  writer resolves the single in-progress task with a claimed PID and persists
  the observed process exit; the QC status gate reads the latest evidence for
  the task being reviewed. Ambiguous multi-claim state fails closed. This is
  still partial P4-11…14 work: the production ecosystem-aware argv path,
  PowerShell/`cmd` process proofs, task-state invalidation policy, and
  `TG4-07`/live-database proof remain open. No real `~/.arcana/tasks.db`
  migration or application launch was performed.
- Validation after this partial evidence slice: `cargo fmt --all` completed;
  `cargo test -p arcana-core --lib
  agent::builtin_tools::build_evidence::tests --locked --quiet` passed 9/9;
  `cargo test -p arcana-core --lib db::tasks::tests --locked --quiet` passed
  19/19; `cargo test -p arcana-core --lib update_task_status_tool --locked
  --quiet` passed 4/4; and `cargo check -p arcana-core --locked` passed.
  The last task-attribution predicate edit was followed by the 9/9 evidence
  tests, but not by a repeat of the full check. Existing unrelated compiler
  warnings remain.
- P4-05, P4-06, P4-28 and P4-37 are now checked with unit evidence. P4-04,
  TG4-02, TG4-06 and S4-03 remain open: still required are an injected DB error
  through a scheduled tick, assignment-event-to-completion smoke, and proof
  that production timeout/panic/cancellation recovers and redispatches a task.
- A GPT-6 Luna implementation review found that autonomous dependency
  completion did not wake the Supervisor. The code now emits unblocked events
  and wakes the Supervisor, covered by the focused helper test.
- One early file-backed worker migration test attempt wrote synthetic
  temporary-DB backup snapshots into `C:\Users\razia\.arcana\backups` and
  showed same-second destination collisions. No files were inspected or
  removed, and no actual user DB content was changed. ADR-0071 records the
  safeguard; migration tests now use in-memory fixtures. The actual `tasks.db`
  was inspected read-only before any migration; no app launch or live provider
  call has occurred during this implementation turn.

## Remaining source-grounded signals to verify on resumption

These are audit leads, not closed findings:

1. **Resolved 2026-09-26:** `WorkerStatus` now governs production row hydration,
   writes, and exhaustive consumers; the v2 migration constrains status and
   rejects unknown legacy values in isolated fixtures (P4-01…03).
2. P4-12 remains relevant: `build_evidence` uses a process-local map. P4-13's
   specific old `str::contains` false-positive example appears stale because
   the current classifier tokenizes command invocations. P4-14's guard reads
   the current transient evidence; verify persistence and consumer behavior
   together.
3. **Resolved by owner ruling:** P4-20 retains shared `arcana/fleet`; task-level
   reversibility must be proved through checkpoints. Do not reintroduce
   per-task branch switching (ADR-0067).
4. P4-22's premise that identity comes only from the removed MCP adapter is
   reported stale after P8. Preserve and prove ACP messaging behavior without
   duplicating already-shipped identity work.
5. P4-27 and P4-41 overlap on epic creation and contain an obsolete MCP
   premise. Consolidate or scope them against the actual Supervisor tool
   surface before implementation.
6. P4-30's default-recipe overwrite behavior and P4-32/33 routing-affinity
   concerns were reported current. Fix recipe persistence before launching the
   app, because startup may overwrite user edits.
7. **Resolved by read-only evidence:** the actual `~/.arcana/tasks.db` passed
   `integrity_check` and has no `routing_history` table; no real user DB writes
   or migrations have run. Preserve the database and use isolated fixtures
   (ADR-0071/0072).
8. P4-31's completion narrative may be stale after P8 removed fallback-policy
   machinery. P4-11's old crate naming and P4-21's `ide:navigate` listener
   claim also need source verification.

## Resume sequence

1. Confirm Git state and inspect both completed reports before using them.
2. Coordinator reconciliation for the Luna pre-build audits is complete;
   see the disposition appendices in the two reports.
3. Continue implementation in dependency order. Owner-checked atomic claims
   now precede outcome recording and affinity changes. First resume by
   reviewing the partial build-evidence diff, then finish P4-11 and the
   P4-12…14 chain with the real platform-owned command path. Prove Windows
   PowerShell/`cmd` spawned-process outcomes and migration behavior using
   isolated fixtures; before any required live database migration, create and
   verify a uniquely named restorable backup. Then continue workspace
   writes/checkpoints, ACP, budgets/recipes, routing, and autonomous
   QC/Supervisor behavior.
4. Complete TG3-26/TG4-16 behavior tests, including the actual QC fallback and
   all exported diff/provider readers; keep shell egress explicitly outside
   persisted app-mediated consent under ADR-0073.
5. Run the application through the user-visible fleet workflow after recipe
   initialization is safe; complete S4 and TG-P4, including the available
   three-platform proof. Update `READINESS_MATRIX.md`, the goal/memory, §0.16
   learnings and public-facing docs as required by §0.6/§0.17.
6. Perform the required two post-phase GPT-6 Luna audits, resolve and log every
   finding, run the no-stale sweep, then commit and push the completed phase
   with non-empty descriptive commit bodies. The user has authorized the final
   P4 commit and push to `main`.

## Urgent stop checkpoint — 2026-09-27

**Current state:** P4 remains in progress and uncommitted. The checkout is still
`main` at `9cc0433`, tracking `origin/main`; the P4 working tree is dirty. No
commit or push has been made in this continuation. The user has authorized
committing and pushing the completed P4 phase to `main` when it is actually
complete.

### Latest continuation work and evidence

- Continued the P4-11 build-verification slice. A new, untracked
  `arcana-core/src/agent/builtin_tools/project_command.rs` is being added as a
  production tool that resolves Rust-owned build/test/lint/run commands for the
  selected project path, executes the process, and records observed
  build/test/lint outcomes. It has not yet had focused tests, end-to-end process
  proof, or a complete source review; do not count P4-11 or TG4-07 as passed.
- Registered the new tool in `arcana-core/src/agent/builtin_tools/mod.rs` and
  added `BuildEvidenceKind::Make` / database support. This is also unverified
  beyond compilation.
- Added optional nested `project_path` selection for monorepos and a path
  containment check. This addition has not yet been tested against escapes,
  symlinks, or representative real workspaces.
- Removed the prior `bash.rs` build-evidence write so arbitrary shell text
  cannot certify QC. Review the now-unused command-text classifier in
  `build_evidence.rs`; either remove/update it if truly dead or preserve any
  still-required P8 behavior with a real consumer.
- Ran `cargo fmt --all` and then `cargo check -p arcana-core --locked`; the
  check completed successfully. It emitted 43 pre-existing/ongoing compiler
  warnings, including new dead-code warning(s) around `build_evidence.rs` that
  need review. The focused build-evidence tests (9/9) passed before this new
  command-tool slice; that is not evidence for `project_command.rs`.
- No application was launched, no real user database was migrated or changed,
  and no live provider call or server/port was started.

### Resume immediately with

1. Read this checkpoint and inspect the live diff/status before editing.
2. Review `project_command.rs`, its registry wiring, the platform argv mapper,
   and build-evidence schema/writer/reader as one chain. Add focused tests for
   workspace path containment, task association, process exit attribution,
   failure handling, and evidence invalidation. Prove actual process results
   from both PowerShell and `cmd.exe` as required by TG4-07; retain the rule
   that generic shell commands cannot create QC evidence.
3. Complete P4-11…16/TG4-07 and the worker-recipe migration from shell build
   commands to the native tool. Audit and remove stale POSIX-specific prompts
   (P4-16/TG4-17) without changing permitted free-form shell egress behavior.
4. Continue in MDP dependency order through workspace writes/checkpoints,
   ACP, budgets/recipes/routing, recovery, QC/Supervisor behavior, and the
   consent exit tests. The current working tree includes broad unfinished P4
   source and documentation changes; do not discard or reset them.
5. Before any subagent fanout, create the required working-tree snapshot per
   MDP §0.2L-5. The user's standing constraint is that any subagents used must
   use **GPT-6 Luna only**. Respect the required post-phase read-only audits.
6. Run the required gates and S4 smokes, update the MDP, gate register,
   readiness matrix, goal, project memory/map, ADR/learning records, and public
   docs; complete the no-stale sweep. Only then stage deliberately, inspect the
   staged diff for secrets, commit with descriptive Conventional Commit
   subject and body, and push to `main` under the user's existing authorization.

**Do not treat this checkpoint as P4 completion.** The current last validated
state is `cargo check -p arcana-core --locked`; `project_command` behavior,
TG4-07, the remaining P4 work, full gates, smokes, and cross-platform proof are
still open.

---

## 2026-09-27 continuation — host Git safety and fleet workflow corrections

This continuation preserves the original stop record above as historical and
updates the current state below. P4 is still open.

### P8 host-enforced commit safety

- Implemented `git_commit` as a FastDevelopment built-in. It accepts explicit
  repository-relative files and requires a Conventional Commit subject plus a
  descriptive body. It refuses trunk/detached branches, pre-staged changes,
  unsafe paths, `.env` files, and staged diffs matched by Arcana's redaction
  scanner. Refused sensitive diffs restore the index.
- Bash rejects Git index/commit mutation text in command segments, including
  direct commands, common shell wrappers, and the tested subprocess form.
  This is a textual guard, not a sandbox against obfuscated executable code;
  recognized secret patterns are likewise bounded by the shared redaction
  scanner. ADR-0075 records this limit, and ADR-0076 reconciles ADR-0059's
  obsolete prose `DELEGATE` parser proof with the later structured subagent
  contract.
- TG8-13 passed:
  `cargo test -p arcana --lib ci_gate::tests::tg8_13_host_commit_safety_is_registered_and_used_by_workers --locked -- --nocapture`.
- P8-74 unit tests passed (5):
  `cargo test -p arcana-core --lib agent::builtin_tools::git_commit::tests --locked -- --nocapture`.

---

## 2026-09-28 continuation — Luna review fixes and production-path proofs

**State:** P4 remains active and uncommitted on `main` at `fff31ec`. No P4
commit or push has been made. Preserve the existing broad working-tree diff.
The product goal has been refreshed in `GOAL.md`: Arcana owns deterministic
harness work, ships a useful team workflow by default, and proves real
consumers; P4 cannot close until cross-phase audit findings are resolved or
assigned to explicit later owners with open gates.

### Implemented and freshly verified

- ACP recipient validation and broadcasts now use the persisted active worker
  roster. `deleted_system_worker_is_not_a_message_recipient_or_broadcast_target`
  and the sender ToolExecutor tests pass; the test accounts for default QC
  rows separately from custom worker fixtures.
- Existing installed Supervisor/QC prompts receive the inbox/stale-mail
  protocol through an idempotent narrow migration while retaining user text.
  `installed_supervisor_and_qc_recipes_receive_inbox_protocols_without_losing_custom_text`
  passes.
- The persisted `qc-agent` row is verified as bound to `qc-dispatcher`. The
  shipped recipe runs through AgentEngine for inbox-only work, reads and
  acknowledges the stable message ID, and leaves no pending inbox item:
  `shipped_qc_recipe_handles_inbox_only_work_through_agent_engine` passes.
  The shipped Supervisor Engine test consumes stale-mail message/task IDs and
  persists a QC nudge through `send_worker_message` before continuing dispatch;
  automatic worker redispatch after that nudge remains open.
- QC checks consent before loading a provider key. Mismatched/absent consent
  returns deterministic structural analysis without provider configuration;
  `qc_diff_without_matching_consent_uses_structural_review_without_provider_key`
  and the provider wrapper grant/revocation test pass. ADR-0081 records the
  ordering decision. Direct Tauri diff-command and complete egress caller
  proofs remain open.
- Scheduler recovery catches panic/timeout within a run, recovers cancelled
  runs through the guard, and leaves a `working` status intact when task-claim
  recovery itself fails so the five-minute recovery pass can retry. New tests
  cover panic, timeout, cancellation, task-DB failure, and a next tick entering
  the real `RecipeRunner` after a timeout. These improve P4-36 but do not close
  S4-03's automatic recovery/redispatch smoke.
- Empty Supervisor/QC queues exit through the real RecipeRunner before provider
  resolution; helper-level no-work behavior is also tested. Event-kick/cron
  call measurement and an intentional provider-turn bound remain open for
  P4-43.
- `cargo fmt --all -- --check` passes. The focused test commands for the above
  behaviors passed in isolated Arcana homes; standard existing Rust unused and
  dead-code warnings remain. No provider credentials were used and no service
  or listening port was started.
- Corrected the stale MDP register count from 390 (290 checked/100 open) to
  383 P-numbered rows (283 checked/100 open), measured from the current plan.
- Revalidated the original PC migration check: Rust 1.98.1/MSVC, Node 24.21.0,
  npm 11.19.0, and Python 3.14.7 resolve in the current shell. `python` and
  `py -3.14` both use `C:\Python314\python.exe`; `sqlite3`, `ssl`, and pip work
  with no prefix warning. No Python repair was needed. `.env.example` contains
  only the optional `GOOGLE_API_KEY` required by the explicit Gemini live-smoke
  test; no `.env` value was read or displayed.

### Independent review disposition

Two additional GPT-6 Luna High reviews covered ACP/QC consumers and run
recovery. Their findings and exact dispositions are recorded in
`_verify/2026-09-27-CROSS-PHASE-HARNESS-AUDIT.md`. Remaining open evidence
includes automatic stale-mail worker redispatch, full scheduler
provider-call measurement, actual AnalyzeDiffTool Git execution, direct Tauri
consent/diff consumers, and full S4 smokes.

### Still blocking P4 closeout

The user has been asked for the remaining contract choices on P4-15 failure
counting, P4-16 persisted user-authored portability scope, P4-40 claim metadata
enforcement, P4-14 QC evidence enforcement, P4-40 force-release semantics,
and P4-19/20 checkpoint ownership metadata. Do not implement those choices or
close dependent gates until their answers arrive. The rest of P4 still needs
its dependency-ordered tasks, TG4/TG-P4 gates, S4-01…S4-05 smokes, cross-phase
no-stale pass, current build/type/test checks, and required post-phase Luna
reviews. The user's existing permission to commit and push P4 to `main` remains
in force after all closeout proof passes.
- S8-06 isolated real-tool smoke passed:
  `cargo test -p arcana-core --test s8_git_safety --locked -- --nocapture`.
  It proved Bash refusal and a named-file commit on `arcana/fleet`, preserving
  the trunk ref and leaving `.env` and generated output uncommitted.
- TG8-06 and TG8-13 recipe/registry gates passed individually in Tauri tests.
  Bash's direct/wrapper refusal test passed. `cargo fmt --all` and
  `git diff --check` passed. Existing workspace compiler warnings remain.
- Full `arcana-core` all-target suite passed with no live credential feature:
  573 library tests plus 36 integration/example tests (609 total, 0 failed).
  Test home was redirected to `target/arcana-test-home`; no real Arcana DB was
  opened. `cargo check --workspace --locked` and `npx tsc --noEmit` passed.
- Full Tauri library suite passed: 111 tests, 0 failed. The run initially
  exposed TG8-07's stale string-literal extractor after the P4 enum migration;
  the gate now checks enum/parser round-trip, emitted UI mappings, canonical
  manual status writes, and every scheduler status arm. The updated focused
  TG8-07 test passes. The suite ran again after this correction.
- `MDP-COUNTS-CHECK.py` passes after reconciling the active register to 390
  tasks (281 checked, 109 open) and 128 testing-gate rows.
- The Supervisor's shipped decomposition instructions list `create_epic` and
  `create_phased_task` in the tools section, with a regression assertion. A
  scripted-provider turn through the real `AgentEngine` and FastDevelopment
  tool executor created an epic and dependent phased task; the test read both
  back from `TasksDb`. P4-41, P4-27 and TG4-05 are now checked on that evidence.
- Default recipe consumer corrections from earlier in this continuation use
  `claim_task`, `project_command`, explicit install intent, and host
  `git_commit`; startup preserves edited system recipes, and recipe model IDs
  are checked against the active provider. These focused tests passed in the
  earlier continuation; see terminal/test history. Full P4 consumer smokes
  remain open.

### Current state and resume order

The P8-74 portion is now recorded as passed in the MDP, TG8-13, S8-06, and the
readiness matrix. The 2026-09-27 Supervisor epic AgentEngine turn also passes
P4-27/P4-41/TG4-05. This does not close P4. The current P4 implementation still
needs full core/workspace validation and the remaining P4 exit obligations:
TG4-07/09…20, TG-P4, S4-01…05, the complete TG3-26/TG4-16 consumer proof,
three-OS command/process evidence, and full fleet dispatch/recovery smokes.
Cross-phase P1/P2/P3 findings stay assigned to their recorded phase/feature
owners; no historical checkboxes were changed based on this source audit.

Before the eventual P4 commit, inspect all current diffs (including edits that
predate this continuation), scan staged changes for secrets, and review exact
staged content. Do not use `git add -A`. Current branch is `main` with
uncommitted changes; no commits or pushes were made here.

### 2026-09-27 continuation — project-command evidence path

- The independent P4 slice review found `project_command` persisted synthetic
  exit code `-1` for spawn failures and timeouts, contrary to P4-13's rule that
  evidence requires an observed process exit. Those paths now return an error
  without writing build evidence.
- The Cargo build proof now invokes the shipped `ProjectCommandTool::execute`
  against a real offline Cargo fixture under an isolated Arcana home. It proves
  a successful process creates task-scoped evidence the QC reader accepts, and
  a subsequent failing process creates evidence the QC reader rejects, across
  DB reopen. This replaces the old test that manually invoked the lower-level
  process runner and evidence writer.
- Validation: `cargo test -p arcana-core --lib
  agent::builtin_tools::project_command::tests --locked -- --nocapture`
  passed (6 tests); the focused production-tool test passed independently.
  Formatting, `MDP-COUNTS-CHECK.py` (390 / 281 / 109 / 128), and
  `git diff --check` passed. TG4-07 remains open for Windows PowerShell/`cmd`,
  three-OS fixture runs and the full desktop worker/QC consumer smoke.
- P4-11, P4-13, TG4-07 and the readiness row now distinguish local production
  wiring/proof from the still-open platform and end-to-end obligations.

### 2026-09-27 continuation — shipped-default portability gate

- The shipped-default portion of P4-16/TG4-17 passes. The core static tests scan
  all default worker/Supervisor/QC recipes, the base/review/delegate system
  prompts, and registered tool descriptions in FastDevelopment, Review and
  Delegate. The Tauri static test scans all 17 built-in skill templates.
- `src-tauri/src/hooks.rs` only re-exports the core hook module; no shipped hook
  templates exist. User-authored hook bodies are not covered. The Bash tool description quotes `&&` only in a warning that tells
  Windows workers not to prepend a redundant `cd` command; the static test
  excludes this negative example from executable-command checks.
- Validation passed:
  `cargo test -p arcana-core --lib tg4_17 --locked -- --nocapture` (2 tests),
  `cargo test -p arcana --lib skills::defaults::tg4_17_tests --locked -- --nocapture`
  (1 test), and `cargo fmt --all`.
- P4-13/P4-14 are checked independently of the broader TG4-07 gate. The
  production integration test proves observed process exits, task-scoped
  persistence/reopen, refusal of missing/failing evidence through the shipped
  status tool, acceptance of passing evidence, and no synthetic evidence on
  spawn/timeout failures.
- The owner must decide the treatment of persisted/user-authored recipes,
  skills and hooks before P4-16/TG4-17 can be closed. The task register now has
  390 rows (283 checked, 107 open). P4 remains active;
  TG4-07's full desktop worker/QC and Windows shell proof, TG4-18's three-OS
  command proof, TG-P4 and S4-01…05 remain open. No full fleet smoke was run.

### 2026-09-28 continuation — strict workspace resolution

- P4-17 is checked. `arcana_core::platform::workspace::resolve_workspace_dir`
  is the shared strict resolver for interactive sessions, worker recipes, Tauri
  IDE/file access, diff analysis and persisted egress consent. It returns a
  canonical path and rejects unset, relative, missing, and non-directory paths.
  Session CWD normalization and RecipeRunner's Arcana-agent/home fallbacks are
  removed. This does not close P4-18 write-path enforcement or P4-21 artifact
  reveal/open/run handoff; TG4-09 stays open.
- `cargo check -p arcana --locked`, `npm run typecheck`, workspace resolver,
  session and worker no-fallback tests, six project-command behavior tests, and
  `git diff --check` passed. Pre-existing Rust warnings remain; no server or
  listener was started.
- P4-16/TG4-17 remains open while awaiting the owner's scope decision for
  persisted/user-authored recipes, skills and hooks. Current shipped-default
  static tests pass; no shipped hook templates exist.
- The active register now has 390 rows (284 checked, 106 open; 128 gates);
  `MDP-COUNTS-CHECK.py` passes. P4 remains active and no full fleet closeout
  or smoke has passed.

### 2026-09-28 continuation — P8 host commit safety closeout

- Strengthened the registered `git_commit` host tool to refuse a branch
  literally named `trunk` as well as `main`/`master`. Every error after writing
  the requested paths to the index restores the index to current HEAD.
- The isolated S8-06 smoke calls the production tool on its initialized trunk,
  verifies refusal leaves the task edit unstaged and the trunk ref unchanged,
  then switches to `arcana/fleet` and verifies only the explicitly named safe
  file is committed. `.env` and generated output stay outside the commit.
  Bash's direct Git-mutation refusal is exercised in the same smoke.
- Current-source validation passed: TG8-13 Tauri gate (1), `git_commit` unit
  tests (7), Bash mutation-guard test (1), S8-06 isolated tool smoke (1),
  `cargo fmt --all`, `git diff --check`, and `MDP-COUNTS-CHECK.py` (390 tasks:
  284 checked, 106 open; 128 gates).
- The P8 implementation, FastDevelopment/Review registration, default worker
  protocol, TG8-13 gate, and S8-06 smoke were committed and pushed to the
  authorized `main` upstream in `37a2792`, `0cdf4b7`, `29a0b91`, and `6ee463b`.
  `cargo check -p arcana --locked` and `npm run typecheck` also pass on the
  current combined working tree. P4 work and the P8 doc/readiness refinements
  remain uncommitted in the working tree; they were not staged by the P8
  code/test commits.
- P8-74/TG8-13/S8-06 remain checked with the stronger proof recorded in the
  MDP, testing gates, ADR-0075, and readiness matrix. Existing limitation
  remains: Bash mutation detection is textual, not an OS-level sandbox.
- Return to P4-18 and ACP P4-22/23 work. P4-16/TG4-17 still awaits the owner's
  persisted-content scope answer; the free-form Bash workspace-write boundary
  also remains pending. Do not close affected items before the owner response.

### 2026-09-28 continuation — native ACP consumer evidence

- P4-22 is checked with production `ToolExecutor` evidence. An actual worker
  send persists the runtime identity; self-send, worker broadcast, and a body
  over 4096 bytes are refused. The inbox reader and acknowledgement tool run
  through a newly created worker `ToolExecutor`, and FastDevelopment registry
  assertions confirm all three ACP tools are registered. Test:
  `cargo test -p arcana-core --lib
  native_tool_executor_supplies_sender_identity_and_refuses_self_messages
  --locked -- --nocapture`.
- P4-23 remains partial. Its production inbox test drops the reader before
  acknowledgement, reconstructs a recipient executor, observes the same
  message ID, acknowledges it, then observes an empty inbox. The scheduler
  recovery-work unit proves pending/read messages keep a worker runnable until
  ack. Focused inbox test:
  `cargo test -p arcana-core --lib
  production_inbox_consumer_replays_stable_id_and_ack_stops_delivery
  --locked -- --nocapture`. Full recipe dispatch after a stale message and
  idempotent side-effect handling are not yet proven; TG4-10/11 and S4-04 stay
  open pending message-storm scope and end-to-end recovery evidence.

### 2026-09-28 continuation — worker recipe claim proof and QC bypass

- P4-34 now has a real shipped-worker consumer test:
  `shipped_worker_recipes_claim_through_agent_engine_with_owner_identity`
  sends `claim_task` through each default worker recipe's AgentEngine and the
  production FastDevelopment ToolExecutor. A wrong worker is refused without
  changing the `assigned` row; the assigned worker then claims it and the test
  checks the real PID and host are stored. TG4-03 evidence and the MDP row are
  updated. The focused test passes.
- A fresh read-only audit found the `UpdateTaskStatusTool` evidence guard is
  bypassed by two production paths: RecipeRunner stranded-task reconciliation
  and the Tauri task-status command both write through `TasksDb::update_status`
  without consulting build evidence. P4-14 is reopened; TG4-07 remains open.
  Owner direction is pending on whether to enforce this invariant with a new
  tasks.db SQLite trigger (recommended), guard all current Rust writers, or
  retain the API-only guard. Do not close QC evidence gates until the chosen
  path prevents and tests the direct writer bypass.
- P8 host commit-safety code/test commits and the P8-only plan, ADR, gate and
  readiness documentation are pushed to `main`; final P8 documentation commit
  is `fff31ec`. P4 work remains uncommitted. `cargo fmt --all -- --check`,
  `cargo check -p arcana --locked`, `npm run typecheck`, MDP count validation,
  TG4-17 shipped recipe and skill-template scans, project-command tests,
  workspace resolver tests, Supervisor epic AgentEngine proof, and the new
  worker claim proof passed on the current tree. No live provider was used.

### 2026-09-28 continuation — IDE artifact navigation consumer

- Added an app-root listener for the shipped `ide:navigate` event so graph
  navigation remains consumable while the IDE tab is unmounted. The listener
  validates event shape and line number, maps absolute paths only when they are
  inside the configured workspace, and refuses traversal/outside paths.
- IDE store `revealFile` loads and expands ancestor directories, opens the file,
  and selects the requested line. The city context menu's producer now invokes
  `open_in_ide` with the correct `file` argument and required `line`.
- Behavior evidence: `npx vitest run src/utils/ideNavigation.test.ts
  src/stores/ideStore.test.ts` passed 4 tests covering the registered consumer,
  workspace boundary rejection, directory expansion, tree readback and opened
  file/line. Full `npm test` passed 21 tests across 6 files; `npm run typecheck`
  passed. No app server or listening port was started.
- MDP P4-21 and TG4-09 now record this partial consumer proof. Keep both open:
  the full agent artifact → Tauri event → IDE → deterministic project-command
  run smoke has not yet been exercised end to end.
- `npm run build` passed (Vite transformed 4,946 modules). Vite reports existing
  dynamic/static import chunking warnings for settingsStore, agentStore and
  sessionStore, and a 5.95 MB main JS chunk plus Monaco workers above 500 kB;
  this change did not address bundle splitting. The desktop WebDriver smoke was
  not runnable in this environment because `tauri-driver` and `msedgedriver` are
  absent. No app server was started and no test port was left open.

### 2026-09-28 continuation — P4-15/TG4-08 contract audit

- A focused GPT-6 Luna High source audit traced the vision decision, Supervisor
  prompt, task retry fields, `project_command`, and Bash registration. The
  owner decision in vision 04 §4.1 is clear on the main rule: Rust runs fixed
  argv and owns the verdict; only after three consecutive failures does the
  model diagnose, after which Rust reruns verification.
- Current source is incomplete: `TasksDb::increment_retry` and
  `get_retry_count` have no production callers; the Supervisor recipe branches
  on `retry_count` but neither records nor advances it. Worker prompts stop or
  retry informally and no built-in diagnosis branch exists. `project_command`
  itself accepts intent/selector/path/timeout, not command text, and records
  observed exit evidence consumed by QC.
- Broader production boundary still matters: FastDevelopment gives workers both
  `ProjectCommandTool` and `BashTool`. Bash accepts model-authored command text
  and spawns `cmd /C` or `sh -c`; its description currently says to use it for
  build/test/lint. Thus project-command evidence is deterministic, but the
  literal gate claim that model-authored build execution is unreachable is not
  established while Bash remains available.
- Asked the owner to settle two contract details before implementing P4-15:
  (1) count observed nonzero build exits only (recommended), or count any
  failed attempt/task attempt including spawn/timeout; (2) whether only
  `project_command` may authorize QC while Bash stays available for general
  work (recommended), or restrict recognized Bash build commands/remove Bash
  from workers for a stronger unreachable-execution guarantee. Keep P4-15 and
  TG4-08 open until the boundary and failure counter have real consumer tests.
- Required eventual proof: no diagnosis on failures one/two, one diagnosis on
  the third qualifying failure, a successful exit resetting the streak,
  deterministic post-diagnosis rerun deciding pass/fail, no second diagnosis
  on recovery/re-entry, and no Bash-authored result reaching QC evidence.

### 2026-09-28 continuation — P4-26/TG4-14 routing exclusion

- `suggest_worker` previously scored the entire worker registry, allowing
  Supervisor/QC to win through history. A static match could also return a slug
  not present among scored workers; empty registries returned a hard-coded
  `worker-backend` recommendation.
- The scorer now excludes system-defined workers and their history before
  computing affinity and duration normalization. Static suggestions must match
  an eligible scored worker; if no implementation workers exist, the router
  returns an error. This preserves custom/user-defined implementation workers.
- `cargo test -p arcana-core --lib routing::scorer::tests --locked` passes
  three tests. One opens the actual seeded default roster through isolated
  the seeded in-memory registry, gives every worker strong history, tries a static
  Supervisor match, and verifies only implementation workers can be selected.
  Another proves system-agent timing extremes do not alter the implementation
  ranking; the third verifies an all-system roster yields no suggestion.
- A fresh GPT-6 Luna High review caught the history-normalization bleed; the
  production change and regression test were updated and passed after review.
  Existing unrelated Rust unused/dead-code warnings remain.

### 2026-09-28 continuation — P4-33 static routing reachability

- `patterns::STATIC_PATTERNS` sets its highest confidence to 0.85, while
  `select_worker` previously required `static_conf > 0.85`; thus no shipped
  static pattern could ever be selected.
- The threshold now accepts 0.85. A focused regression test obtains the real
  shipped API/endpoint match, loads the real default team in an in-memory
  seeded worker DB, and proves the selected result is `worker-backend` with
  `method: static`.
- A previous version of the routing tests used file-backed `WorkersDb::open_at`.
  Its migration attempted to write timestamped backup artifacts in
  `C:\Users\razia\.arcana\backups`; the latest observed file is
  `workers-20260928-022918-workers-db-v2-worker-status-check.db`. I left all
  backup files untouched. The tests now use an in-memory seeded DB and do not
  create further backup artifacts.
### 2026-09-28 continuation — P4-38 assignment guard

- The per-entry assignment transaction is now guarded by a single `UPDATE`
  predicate that refuses assignment when the target worker already has another
  `in_progress` task. This is race-safe with concurrent assignment attempts and
  leaves the rejected task and routing history untouched.
- `assign_tasks` already treats the DB method's error as an entry-local failure
  and continues iterating; focused test
  `p4_38_batch_entry_rejects_worker_with_active_task_without_failing_other_entries`
  proves the loaded worker's entry is refused, the task stays ready, and a
  later assignment to another worker succeeds.
- Test uses an in-memory `TasksDb`; no file-backed migration or backup is
  triggered.
- A fresh GPT-6 Luna High implementation review correctly noted that the DB
  test alone did not prove the batch tool continues after an entry error. Added
  `loaded_worker_failure_is_reported_and_later_batch_entry_succeeds`, which
  calls the real `AssignTasksBatchTool::execute` with an isolated Arcana home.
  The busy-worker entry is reported failed, the later free-worker entry is
  successful in the same response, and the refused row stays ready. The exact
  focused test command passes; the review gap is resolved.

### 2026-09-28 continuation — escalation fairness, routing outcomes, orphan CAS

- **P4-24/TG4-12 closed.** Pending worker requests now make the Supervisor a
  recoverable-work candidate. The shipped recipe no longer stops after Step
  1a; it continues to dispatch unrelated READY work and may idle with a
  pending request surfaced for the next scheduled tick. `a_pending_escalation_keeps_supervisor_dispatch_available`
  leaves a request pending, checks Supervisor eligibility, runs the real batch
  assignment tool, and reads the unrelated task back as assigned. The recipe
  protocol test guards against reintroducing the old early stop.
- **P4-25 closed; TG4-13 remains open.** The production status tool uses the
  pre-transition task owner and atomically records the newest matching pending
  history row with status completion/reassignment. Older attempts are not
  rewritten. Missing history does not block manually assigned work from
  completing. Tauri UI outcomes are owner-scoped and update only one row; the
  legacy task-only helper also updates at most the newest pending row.
  Both tool and Tauri routes compare the observed state in the transaction.
  `autonomous_completion_closes_only_the_current_owners_routing_row` and
  `stale_task_owner_cannot_overwrite_reassignment_or_claim_its_outcome` pass.
  The remaining TG4-13/P4-32 proof is that N actual outcomes measurably change
  normal worker ranking.
- **P4-40 partial.** Automatic orphan recovery now releases only when the
  observed status, owner, PID, host and `updated_at` still match. The new
  `stale_orphan_snapshot_cannot_release_a_newer_worker_claim` test proves a
  delayed stale snapshot cannot clear a newly assigned/claimed worker. The
  SQLite-wide claim-metadata invariant and explicit Tauri force-release
  semantics remain open pending the owner's decision.
- **P4-38 reviewer follow-up.** The real batch-tool regression now asserts
  exactly one worker is kicked after refusing the busy-worker entry.
- Focused tests passed for all four behaviors above; formatting was applied.
  Existing unrelated Rust unused/dead-code warnings remain. No server or port
  was started. Owner decisions for P4-15 failure counting/Bash boundary and
  P4-16 user-authored artifact scope are pending; the current turn has also
  asked whether P4-40 metadata should be enforced in SQLite and whether
  explicit force-release remains an override. No P4 closeout or commit/push
  was made in this continuation.

### 2026-09-28 continuation — P8 safety closeout and P4 resume

- P8 host-enforced commit safety is closed and pushed to `main` at
  `fff31ec` (`origin/main`): P8-74/TG8-13/S8-06 have current focused tests and
  an isolated Git smoke. P4 work remains unstaged and open.
- Before the new read-only and implementation-agent fan-out, the dirty tree was
  archived at `C:\Users\razia\AppData\Local\Temp\arcana-p4-working-tree-20260928-050137-corrected.zip`.
- **P4-12 closed.** Ran
  `cargo test -p arcana-core --lib completed_process_is_persisted_for_the_claimed_task_and_consumed_after_reopen --locked -- --nocapture`;
  its file-backed `tasks.db` test persists a claimed worker's process result,
  closes/reopens the database, and proves the QC reader accepts matching
  evidence while refusing a task without evidence. No live user database was
  used. Updated active task totals to 383 rows (284 checked, 99 open), project
  memory/map and platform-readiness wording; TG4-07 remains open for desktop,
  PowerShell/`cmd`, and three-platform proof.
- P4-30 independent review confirms database worker rows own identity/state and
  recipe YAML owns prompt text, but the existing tests do not prove a
  customized prompt flows through a file-backed worker binding to the
  scheduled runner. Keep P4-30 open until that consumer is tested.
- P4-32 review confirms the shipped Supervisor still chooses normal
  `worker_slug` values from its static prompt matrix; `assign_tasks` does not
  call the scorer. Owner questions are pending for persistent override-audit
  fields and candidate worker statuses. Do not implement an override contract
  until answered.
- P4-44 procedural recall has a real store and ToolExecutor retrieval seam but
  no engine prompt consumer or production caller. A GPT-6 Luna High worker is
  implementing only the engine/tool-executor/recipe-runner wiring and an
  isolated scripted-provider behavior test; main agent will review and verify
  the handoff. Do not close P4-44 on seam presence alone.

### 2026-09-28 continuation — P8 verification and P4-30/P4-44 closure

- Re-ran the P8 host-safety proof on the current tree:
  - `cargo test -p arcana-core --test s8_git_safety --locked -- --nocapture` — S8-06 passed against an isolated Git repository and the real host tool.
  - `cargo test -p arcana --lib ci_gate::tests::tg8_13_host_commit_safety_is_registered_and_used_by_workers --locked -- --nocapture` — TG8-13 passed.
  - `cargo test -p arcana-core --lib agent::builtin_tools::git_commit::tests --locked -- --nocapture` — 7 passed.
  - `cargo test -p arcana-core --lib agent::builtin_tools::bash::git_mutation_guard_tests --locked -- --nocapture` — direct Git mutation refusal passed.
- Workspace `cargo fmt --all` was applied under the owner's prior approval; `cargo fmt --all -- --check` passes.
- **P4-30 closed.** The isolated file-backed worker/recipe test customizes recipe text and worker profile, repeats recipe initialization, then resolves via the same helper used by `RecipeScheduler::start_worker`. It reads the customized prompt and DB-owned profile through the persisted binding. Focused command passed: `cargo test -p arcana-core --lib startup_preserves_custom_recipe_and_scheduler_reads_it_through_file_backed_worker_binding --locked -- --nocapture`.
- **P4-44 closed.** Fleet RecipeRunner enables procedural recall; AgentEngine defaults remain off. The isolated scripted-provider test proves only matching tool recall reaches the next provider request and is not retained by unrelated tools/requests. Focused command passed: `cargo test -p arcana-core --lib worker_procedural_memory_reaches_only_the_next_matching_provider_request --locked -- --nocapture`.
- Active plan count is now 383 P-numbered tasks (286 checked, 97 open). P4 remains active; no commit or push was made in this continuation.

### 2026-09-28 continuation — audit closure and Windows shim proof

- Reopened the independent P8 host audit findings in the live tree. The
  in-process `git_commit` transaction now serializes on the shared index lock;
  path arrays reject non-string entries; credential scanning inspects exact
  staged blobs, including binary blobs; the message validator requires the
  documented blank line; and the Bash textual check covers common index/ref
  mutations. The Tauri gate was updated after its old response-string assertion
  failed on the new guard. The historical P8 audit is still retained as the
  before-fix source snapshot; the current resolution and boundaries are in
  ADR-0075/0083 and P8-74/TG8-13.
- Current-tree P8 proof rerun: ten `git_commit` unit tests, Bash mutation guard,
  TG8-13, and S8-06 all passed. `cargo fmt --all` was applied and will be
  rechecked after the remaining changes. The process-local index lock protects
  Arcana's in-process fleet; the textual Bash check is not a process sandbox.
- Independent P4 production-path audit is recorded in
  `_verify/2026-09-28-P4-PRODUCTION-AUDIT.md`. Re-opened findings: QC evidence
  promotion bypasses in the Supervisor/stranded-task/Tauri paths (P4-14), saved
  task budgets do not reach the worker engine (P4-29), normal assignment still
  lets the model select without the deterministic scorer (P4-32), and the
  Bash/QC boundary and failure-streak contract remain owner-directed (P4-15).
  ACP interruption/idempotency remains a proof gap under P4-23/TG4-10/11.
- The Windows `.cmd` path was exercised through the production
  `run_resolved_command`: a temporary shim under a path containing spaces
  received fixed `run build` argv and returned exit 17. The first run failed
  because `cmd /S /C` needed its outer quote pair; the launcher was corrected,
  and the focused rerun passed. This closes the shim execution question on
  Windows only; TG4-18's full corpora and Linux/macOS proof remain open.
- P4 remains active and its audited gaps stay open. The cross-phase P1/P2/P3/P8
  reports and their gate dispositions are separate from P4-owned closeout. No
  live DB/provider, application service, or port was used.

### 2026-09-28 continuation — fresh gate rerun and checkpoint review

- Fresh checks on the resumed tree: `cargo fmt --all -- --check`,
  `cargo check --workspace --locked`, `npm run typecheck`, and all 21 Vitest
  tests pass. P8's ten `git_commit` unit tests, Bash Git-mutation guard,
  TG8-13 host test, and S8-06 isolated real-Git smoke pass again. No live
  provider, database, application server, or port was used.
- A GPT-6 Luna High read-only review of P4-19/P4-20 found that session-only
  snapshots, swallowed snapshot failures, and blind whole-file restore do not
  prove task-level reversibility. It recommends task ownership in the
  checkpoint schema/API, a missing-file snapshot state, fail-closed coverage
  for all direct file writers (including append and interface-spec tools),
  task-aware retention, and conflict detection before restore so a newer
  task's contents cannot be overwritten. Runtime identity must come from a
  verified claim; `session_id` is not sufficient.
- The review also confirmed that arbitrary Bash and `project_command` can
  mutate files outside the direct file-tool checkpoint hooks. This depends on
  the still-pending P4-15 owner decision about the process boundary. P4-19/20
  remain open; no checkpoint implementation was started before that boundary
  is settled.
- These reruns do not close any P4 task or gate. P4 remains active, with its
  existing open task count and unresolved owner decisions unchanged.
- Added a negative/positive P4-39 assertion to
  `shipped_status_tool_completion_unblocks_a_dependent_for_supervisor_dispatch`:
  the real status-tool completion readies the ordinary dependent and leaves a
  same-blocker budget-exceeded dependent held with its reason. The focused
  test passed. Automatic Supervisor routing and worker dispatch remain open;
  the MDP and readiness matrix now record this as partial evidence.

### Cross-phase continuation note — P2 analytics date grouping

- The P1/P2 audit found analytics readers comparing UTC-stored timestamps to
  the local day's date. `analytics/db.rs` now applies SQLite local-time date
  grouping consistently, and `local_day_tests` verifies records on either side
  of local midnight via the date, today-activity, and range readers.
- `cargo fmt --all -- --check` and the focused analytics test pass on this
  continuation. TG2-15 remains open for UTC−12/0/+14, daylight-saving, and
  sub-hour cadence coverage. The correction and disposition are recorded in
  ADR-0085, `TESTING_GATES.md`, `READINESS_MATRIX.md`, and the P1/P2 audit.

### P8 closeout verification — 2026-09-28

- TG8-13 now asserts that every shipped worker recipe includes the protected
  `main`/`master`/`trunk` instruction. The targeted gate, ten host commit unit
  tests, Bash mutation guard, and S8-06 isolated real-Git smoke all pass.
- Corrected the P8 audit's stale documentation-consistency finding: both the
  live Commit Protocol and tool description already included `trunk`; the audit
  missed synchronizing that section after remediation. ADR-0086 records the
  correction and the audit retains the original statement struck through.
- P8 is closed for the supported registered host tool. Its boundary remains
  literal protected branch names, in-process serialization, and a Bash text
  heuristic; it does not claim OS process isolation or protection against
  external Git writers. P4 remains active and uncommitted.

### P1/P2 audit register corrections — 2026-09-28

- TG2-11 has been reopened as explicitly PARTIAL: Linux no-D-Bus fallback
  behavior lacks runtime proof.
- TG2-14 now checks the supported provider set (Anthropic, Google, OpenAI),
  supported ID construction, and rejection of xAI/Ollama/empty IDs. Its four
  focused provider-factory tests pass; legacy-key cleanup remains intact.
- TG2-15's local-midnight defect is fixed and tested on this host; the
  multi-zone/DST/cadence requirements remain open.

### P4-11 project-command continuation proof — 2026-09-28

- Re-ran `cargo test -p arcana-core --lib
  agent::builtin_tools::project_command::tests --locked -- --nocapture`: all
  eight tests passed. This includes mixed Rust/TypeScript discovery, nested
  selector fallback, workspace boundary refusal, process exit/evidence
  readback, bounded output, and a Windows `.cmd` shim under a spaced path.
- Keep P4-11/TG4-18 open for Linux/macOS process fixtures and actual three-OS
  worker execution.

### Independent P8/P2 review follow-up — 2026-09-28

- Reworded S8-06 so its smoke claim matches the test: it directly invokes the
  real Bash and host-commit tools in an isolated Git repository. TG8-13
  separately asserts FastDevelopment registration and shipped recipe usage;
  there is no full model-produced AgentEngine/RecipeRunner turn in S8-06.
- The P1/P2 audit caught that Settings provider IDs were hardcoded separately
  from the factory. `list_providers` now derives them from `factory::supported_ids`
  and fails closed if a supported ID lacks a display label. The Settings
  consumer regression test passed; TG2-14 records both factory and Settings
  evidence. P8 and P2 audit records were corrected without claiming a broader
  end-to-end worker smoke.

Register count verification (2026-09-28): `MDP-COUNTS-CHECK.py` reports 390
active P-numbered tasks (293 checked, 97 open) and 128 gates. The MDP header,
§5.1 count, and project-memory current count were synchronized to that result;
older checkpoint counts remain historical snapshots.

### IDE mutation path-safety continuation — 2026-09-28

- The P4-18 source sweep found that direct agent file tools use the shared jail,
  while Tauri IDE mutations had a separate traversal gap: rename destinations
  bypassed validation and move destinations used only lexical normalization.
  These UI commands are not the agent/process boundary for P4-18.
- IDE create/delete/rename/move now use
  `arcana_core::agent::builtin_tools::path_jail::validate_workspace_path` for
  the paths they mutate. New/existing workspace path and traversal tests pass
  on Windows. A Unix-only symlink escape test is included for CI.
- ADR-0088 records this scoped repair. P4-18, P4-19, P4-20 and their gates
  remain open; arbitrary process mutation coverage still depends on the
  unresolved P4-15 boundary choice.

### P4-11 command corpus continuation — 2026-09-28

- Added checked-in `fx-repo-rust`, `fx-repo-node`, and `fx-repo-python`
  corpora. One focused test discovers each project, resolves supported
  build/test/lint/run intents, and executes them through the production
  process launcher. Python build/run/install are asserted unsupported; no
  install is inferred.
- The existing Windows/Linux/macOS CI build matrix now installs Rust/Clippy,
  Node, Python, pytest, and Ruff, then runs the exact corpus test. Local
  Windows execution passed all Rust, npm, and Python intents, including the
  actual `npm.cmd` path. Linux/macOS results remain pending CI.
- This Windows host has Python 3.14, but pytest and Ruff were not installed in
  that interpreter. The verification installed those test-only tools under the
  ignored `target/p4-command-test-python` directory and used a process-scoped
  `PYTHONPATH`; it did not modify the user's global Python installation.
- P4-11/TG4-18 stay open until all runner results and production worker-path
  proof are recorded. MDP and readiness now describe that partial state; the
  stale gate count note was corrected from 126 to 128.

### P4-15 and P4-18 production tool-path continuation — 2026-09-28

- Added `autonomous_bash_requires_permission_before_shell_execution`. It
  drives a denied Bash write through `ToolExecutor::execute_batch` in
  Autonomous mode, observes the actual pending permission, verifies the
  actionable tool refusal, proves the command did not create its marker file,
  and checks the pending entry drains. The focused test passed.
- Added `third_observed_build_failure_exposes_diagnostic_guidance_on_the_tool_path`.
  It runs three real failing Cargo builds through `ProjectCommandTool::execute`
  against an isolated fixture and observes escalation on the third failure;
  task `retry_count` reaches 3. The focused test passed. The existing DB test
  proves a passing result resets the streak.
- Updated MDP P4-15/P4-18 and TG4-08 with this partial evidence. These remain
  open for provider/model diagnosis and repair retry, process failure variants,
  OS-enforced sandboxing and capability probes, process-write checkpoint
  ownership, and fleet-level proofs.
- `analyze_diff_tool_executes_repository_diff_without_consent_or_provider_key`
  now drives the actual `AnalyzeDiffTool::execute` path against a temporary Git
  repository, verifies deterministic-only output without a provider key, and
  reads back the denied egress audit. The test passed. TG4-16 now remains open
  for the Tauri PR-diff invocation and a full desktop consent proof.
- `manual_release_command_rejects_a_stale_claim_snapshot` drives the Tauri
  force-release command after a task has moved from worker A to worker B. The
  command refuses the old UI snapshot and the test verifies worker B's claim
  remains intact. The focused test passed; an injected race between the command
  preflight read and its CAS write remains open.
- Persisted custom recipe, shell hook, and skill portability rejection tests,
  built-in skill template checks, and the shared portability validator tests
  passed on Windows. No full TG4-17 corpus sweep has been run yet.
- The MDP staleness sweep exposed an outdated live Tauri command inventory
  (250 stated, 242 anchored command attributes in the source tree). Updated the
  current inventory and module map; all 21 MDP staleness checks now pass.
- The required P4 acceptance smoke remains mandatory and pending: launch the
  desktop application and use PA chat to delegate a calendar build to the
  fleet in a dedicated child directory under `C:\Playground`.

## Continuation checkpoint — 2026-09-28

- Updated `GOAL.md`, MDP P4-49 and `READINESS_MATRIX.md`: the blocking acceptance run now targets the dedicated `C:\Playground\arcana-calendar` directory. It must be a real Windows desktop PA-chat → fleet task with frontend, backend, integration, Arcana-owned verification/task evidence, and a launched/inspected runnable artifact. The directory was absent at the time of this check; no Playground files were changed.
- A first `npx tauri dev` launch exposed a production panic: Tauri's synchronous `setup` called the Tokio runtime reaper without an active runtime. `src-tauri/src/main.rs` now enters `tauri::async_runtime` before spawning it. `cargo check -p arcana --locked` passed, and the second desktop launch kept the `Arcana` process/window alive without the panic.
- The same live launch showed that the Supervisor, QC, platform and cleanup workers start, while legacy persisted backend/frontend/MCP recipes are rejected for actual nonportable `;` or `&` command examples. This is a partial launch smoke only; the full fleet is not yet runnable from this saved profile.
- A fresh source review found that a session `AllowAlways` entry bypassed `FreeformShell`'s promised per-call prompt. `tool_executor.rs` now excludes free-form shell both when checking standing approvals and when recording `AllowAlways`; `bash_standing_allowance_does_not_skip_per_call_approval` passes.
- P4-16's validator previously rejected prose mentioning `source .venv` and treated `2>&1` as nonportable. It now checks executable shell fences, inline `bash('...')` examples and hook commands; portable redirection is accepted, executable POSIX-only examples remain refused. Both `shell_portability::tests` pass. Full TG4-17 content corpus remains open.
- The fresh Windows process-security review confirms Bash and `project_command` both launch children with the current user's filesystem authority. Redirect/path checks and Job Objects are not OS write confinement. The smallest plausible approach is AppContainer for both launchers, requiring a direct Windows API dependency and a workspace/toolchain/cache access policy; no implementation or fail-closed proof exists yet. The owner has since directed both OS write containment and per-call Bash approval; selecting and proving the concrete per-OS boundary and capability policy is still required before fleet processes can safely run.
- `npx tauri dev` was stopped after the launch probe; port 1420 and the Arcana process were verified stopped. Native Windows app controls are not exposed by the current CUA runtime (`apps` inventory empty and `getApp`/`listWindows` unavailable); only browser controls are callable here. A browser view of localhost is not a valid Tauri PA-chat smoke. The actual desktop PA-to-fleet acceptance run has not been performed.
- Focused passing checks in this continuation: `cargo fmt --all`; `cargo check -p arcana --locked`; `cargo test -p arcana-core --lib shell_portability::tests --locked -- --nocapture` (2/2); `cargo test -p arcana-core --lib bash_standing_allowance_does_not_skip_per_call_approval --locked -- --nocapture` (1/1). Existing compiler warnings remain. No commit or push was made.

### P4-49 acceptance preparation — 2026-09-28

- Tightened `GOAL.md`, MDP P4-49 and `READINESS_MATRIX.md` with a concrete
  end-to-end scenario: create a calendar event, read it after refresh from
  backend persistence, edit it, delete it, and retain observed frontend request
  and backend response/storage evidence. This does not select the app's stack.
- Rechecked the existing `C:\Playground` fingerprint at
  `target/p4-49-playground-before.csv`; the dedicated
  `C:\Playground\arcana-calendar` directory exists and is empty. It is the only
  new Playground entry; no acceptance files have been written.
- The live desktop Arcana app's Tauri `set_workspace_dir`/`get_workspace_dir`
  commands now set and confirmed the workspace as the canonical
  `\\?\C:\Playground\arcana-calendar` path. An earlier setup attempt had
  selected `C:\Users\razia\Documents`; it was corrected before submitting any
  task or granting provider egress consent. The PA chat is idle.
- Arcana is still running with Vite on port 1420 and WebView debugging on
  loopback 9222. No PA task has been sent and no Gemini consent has been granted.
  Do not use this live process to submit fleet work until the OS process-write
  boundary below is implemented and proven.
- The previously reviewed security gap remains a hard P4 blocker: Bash and
  `project_command` inherit the Arcana user's filesystem authority. Per-call
  Bash prompting does not confine `project_command` or arbitrary child code.
  A fresh read-only platform design review is underway; it must recommend a
  fail-closed Windows/Linux/macOS boundary and required tests before acceptance.- The current running desktop launch applied the saved-recipe migration and
  started all seven system workers: Supervisor, QC, backend, frontend, platform,
  MCP, and cleanup. This closes the earlier recipe-start gap only; no fleet task
  has been delegated and the process-write boundary remains open.- `get_workspace_dir` through the live Tauri bridge returns the canonical target;
  `get_workspace_egress_consent(google)` returns false and the PA composer is
  empty. No provider request has been sent. The curated PA task prompt and
  evidence checklist are prepared in
  `_verify/P4-49-CALENDAR-ACCEPTANCE.md`, but deliberately not submitted while
  child-process write containment is unproven.- Before any PA submission, recomputed SHA-256 and directory-kind state for
  the existing `C:\Playground` tree (16,040 entries). All entries outside the
  new empty `arcana-calendar` target exactly match the saved baseline; zero
  additions, changes, or removals were observed.- Stopped the preparation app after verifying the empty PA state; ports 1420 and
  9222 are free. No acceptance or provider process remains running.

### Windows sandbox continuation — 2026-09-28

- The approved workspace-scoped AppContainer lease is now implemented for
  production child launches. A `ToolExecutor` retains the canonical workspace
  lease for its run, same-workspace children reuse the sandbox and its ACL
  grants, distinct workspaces receive distinct leases, and the child retains
  the sandbox through job termination so access grants are revoked only after
  the final child exits.
- Passing evidence: `cargo test -p arcana-core --lib
  platform::process_sandbox::windows::tests --locked -- --nocapture` passed
  2/2 Windows tests in 122.03 seconds. The tests prove direct and nested child
  writes inside the selected workspace succeed, sibling writes are denied,
  and Rust-owned `.cmd` shim invocation preserves output and exit code 17.
  `cargo test -p arcana-core --lib
  platform::process_sandbox::workspace_lease_tests::a_run_reuses_only_its_canonical_workspace_lease
  --locked -- --nocapture` passed 1/1. Earlier,
  `cargo check -p arcana-core --tests --locked` passed.
- These results are Windows-focused, not TG4-09 closure: timeout/cancellation
  cleanup, full tool-to-IDE handoff, Linux/macOS confinement, and three-OS
  worker-path proof remain open. Existing compiler warnings remain.
- The target `C:\Playground\arcana-calendar` is still empty because the PA
  acceptance task has not been submitted. The current computer-control
  inventory exposes no native app controls; a browser-only or manual code run
  would not meet P4-49. The acceptance record and TG4-09 evidence were updated
  to distinguish this completed Windows proof from the still-pending desktop
  acceptance. No Playground contents were changed by these tests.
- No Arcana server ports are listening. The observed `cmd.exe` processes belong
  to the Codex app's CUA runtime launcher, not Arcana or the sandbox tests.

### Pause checkpoint — 2026-09-28: logs and Gemini defaults

- The owner will run P4-49 as the final P4 action. Do not submit the calendar
  task or write into `C:\Playground\arcana-calendar`; the owner will return with
  the PA transcript and logs for analysis. P4 remains open.
- The Logs tab was expanded to display Rust tracing module targets correctly,
  provide TRACE/PANIC filtering, show read errors, avoid overlapping refreshes,
  and report refresh latency and file size. Rust log commands now read a
  bounded tail, incrementally count appended lines, and retain a separate
  complete-file export. Focused frontend parser/component tests and four Rust
  log-command tests passed before this pause. Desktop live UI verification is
  still unavailable in this session.
- Gemini 3.8 Flash was made the fresh-install and built-in fleet default across
  provider catalogue, Settings hydration, workers, recipes, and fallback paths.
  A targeted seeded-worker migration test and pricing test passed. Existing
  explicit non-Gemini worker choices are preserved. Official model and pricing
  pages were checked; no provider key was read or changed.
- The production project-command corpus test exposed a Windows sandbox launch
  failure on the first Rust build intent:
  `CreateProcessW AppContainer launch: The system cannot find the file
  specified (os error 2)`. A tentative change in
  `arcana-core/src/platform/process_sandbox/windows.rs` grants non-inheriting
  `FILE_TRAVERSE` to user-profile ancestors while keeping read/execute grants
  on selected toolchain/cache directories. The fix is unverified.
- That corpus test consumed several minutes of high CPU during ACL setup without
  producing test output and was interrupted at the owner's request to stop.
  The test process is no longer running, but forced interruption may have
  bypassed the AppContainer profile/ACL cleanup. An `icacls` inspection showed
  an AppContainer SID ACE on `C:\Users\razia`; its owner has not been identified
  and it must not be removed speculatively. Before rerunning tests, identify
  whether it belongs to the interrupted test and clean up only a confirmed
  orphan. Avoid terminating a sandbox test after ACL mutation begins.
- Next session: inspect the dirty diff and ACL/SID state; redesign toolchain
  grants so they do not recursively touch every incidental PATH tree; rerun the
  focused production launcher test with a measured duration and let cleanup
  finish; then rerun Logs tests/typecheck, Gemini-targeted tests, and the
  relevant CLI smoke. Check that `C:\Playground\arcana-calendar` remains empty.
  Preserve the full Logs export path, refresh timing and file-size evidence for
  the owner's later calendar run.
- P4-49 and TG4-09 remain open. No commit or push was made. Do not commit or push
  until the owner supplies the final acceptance evidence and the remaining
  phase gates pass.
