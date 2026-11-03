# P4-51 Shell and Git Executor Matrix — 2026-10-09

Status: investigation started. Scope is limited to cfg(test) additions in arcana-core/src/agent/builtin_tools/git_commit.rs and this report. No tests/builds or live-resource operations are authorized in this task. Parent will run the exact test filter after review.

## Investigation

The memory registry had no Arcana P4-51 or git_commit match. Repository search located the current P4-51 verification records and ADR log; next I am reading the named P4-51/TG4-25 rule text and public executor/tool interfaces before drafting assertions.

## Requirement-derived assertion draft (before handler inspection)

From MDP P4-51 and TG4-25, plus the coordinator's bounded shell/git scope, the new matrix will assert:

- Resolve `bash` and `git_commit` from the live `FastDevelopment` registry and submit real tool calls through `ToolExecutor::execute_batch`, preserving the returned values at the consumer boundary.
- Git success: create a deterministic isolated repository fixture, establish a real atomic worker claim in `TasksDb`, commit an explicitly named working file on `arcana/fleet`, then read the resulting commit and tree back through libgit2. Verify only the requested file changed; sibling content and index state remain as arranged.
- Git failure: submit a deterministic guard-invalid or otherwise isolated negative call with planted private input values; require a failed bounded JSON result with stable tool/operation and available correlation, and verify the repository/index do not move.
- Shell success: issue host-specific exact argv (`cmd` echo/exit on Windows; `sh` printf/exit on Unix) to write an owned marker within the canonical workspace. Grant it only via the public pending-permission request and one-use responder; verify marker contents through the workspace consumer.
- Shell negative: exercise a nonzero exit and an approval denial as bounded failed JSON, correlated to each call, while excluding planted key/prompt/source/argument values. A denial must not run the process or create the marker.
- Restrict process-launch success to Windows/Linux because the sandbox implementation is unavailable on macOS; macOS must not count as a pass for that fixture.

The reported scope will name the exact focused test filter and distinguish assertions that remain conditional on the coordinator's execution host. TG4-25/P4-51 remain open after this bounded two-tool slice.

## Public-surface review

### Source-grounded fixture details

- `builtin_tools::register_for_mode` registers the concrete `BashTool` and `GitCommitTool` in `FastDevelopment`; `ToolRegistry::get` returns those production handlers. `ToolExecutor::new` is the public constructor and `execute_batch` is the dispatch path. The latter adds executor-owned metadata to failure results; the shared envelope includes schema, `ok`, tool/operation, error code/category, retryability/delay, message/cause/action, context, and timestamp.
- `FreeformShell` requires a per-call prompt even in `Autonomous`; `ToolExecutor::new` has a pending-permission channel available. The public `PendingPermissions` map is keyed by `session_id-tool_use_id`; removing the oneshot sender and sending exactly one `PermissionResponse::{Allow,Deny}` uses the same one-use response path as the existing executor regression.
- `git_commit` requires a current worker claim before mutation and checks it again around the commit. The fixture can isolate `TasksDb::open()` with the existing `db::test_support::with_home`, create a task, assign it to the worker, then call public `claim_task_with_pid`; the test must retain the home guard until all handlers finish.
- Existing `repo()` creates a real libgit2 repository with `main`, base commit, and identity; `branch_to_fleet()` moves HEAD to `arcana/fleet`. `index_changed_paths()` compares the actual index to HEAD, so it can prove an intentionally pre-staged sibling remains exactly the same after a negative result.
- Bash constructs exact argv as `cmd /C <command>` on Windows or `sh -c <command>` elsewhere. The process sandbox module has no macOS launch implementation; the matrix's Bash launch cases therefore need `#[cfg(any(windows, target_os = "linux"))]`.

Proposed test shape: the Git and Bash rows are separate focused tests inside `git_commit::tests`. Each owns an isolated home, creates and atomically claims one real task, and uses the live FastDevelopment registration with a worker-identity executor and run ID. Git success and pre-staged-sibling refusal run in the all-host test; Bash approved-success, approved-nonzero and denied calls run in the Windows/Linux-only test. For each failure the tests check `ToolResult.is_error`, parsed `ok=false`/schema, stable tool/operation and call correlation, bounded message/cause/action, and omission of synthetic key/prompt/source/argument canaries. The positive Git row reads the commit and named file from libgit2 and verifies the sibling is absent from the commit and the index is clean; the negative row compares the full changed-path set before/after. The positive Bash row reads the owned marker from its canonical workspace; the denied row proves the marker was not created.

The local macOS sandbox gap is an explicit execution limitation, not a reason to skip registration-level Git assertions. No source helper launches a command, and no tests have been run here.

## Test-only additions

- Added `registered_tool_git_commit_success_and_guard_failure_reach_executor`. It uses the existing `repo()`/`branch_to_fleet()` helpers, isolated `with_home`, and a `TasksDb` task transitioned through assigned to the public atomic `claim_task_with_pid` operation. The live FastDevelopment `git_commit` runs through `ToolExecutor::execute_batch`; libgit2 reads the committed blob and confirms the named file alone entered the tree. A pre-staged sibling then triggers a real guard failure. The test compares HEAD, complete index changed-path set, index entry OID, sibling bytes, and the uncommitted working file before/after.
- Added `registered_tool_bash_success_nonzero_and_denial_reach_executor`, compiled only for Windows/Linux. It drives the FastDevelopment registration through `ToolExecutor::execute_batch` and responds through the actual public one-use pending-permission sender for each call. The success input is `echo ARCANA_SHELL_MARKER> bash-marker.txt` on Windows (`cmd /C`) and `printf '%s' 'ARCANA_SHELL_MARKER' > bash-marker.txt` on Linux (`sh -c`). It reads the exact platform marker bytes from the canonical fixture workspace. The nonzero case emits all four synthetic canaries and exits 17; the denial case carries the same canaries and proves its marker was never created.
- Shared assertions require failed `ToolResult`, JSON `schema_version=1`/`ok=false`, bounded output and message/cause/action, stable tool/operation/error identity, retry fields, timestamp, run/agent/task/call correlation, and absence of planted private values. No test, build, shell process, network, credential, `.env`, persistent database, or Git-mutating command was run during authoring. Only the source fixture itself contains git2 writes to its temporary repository when the coordinator later runs it.

Exact focused filters for coordinator execution:

- `agent::builtin_tools::git_commit::tests::registered_tool_git_commit_success_and_guard_failure_reach_executor`
- `agent::builtin_tools::git_commit::tests::registered_tool_bash_success_nonzero_and_denial_reach_executor` (Windows/Linux only; absent on macOS by cfg)

P4-51 and TG4-25 remain open. This slice covers two registrations and representative success, guard, permission, and nonzero outcomes; it does not close every builtin or failure class.

## Coordinator execution and fixture corrections

Initial fixture run failed before handlers because invented run IDs did not exist in RunsDb. Both tests now use actual begin_worker_tick records. The Git observer also reloads its cached libgit2 index after the separate handler commits. No production guard was weakened. Corrected exact `registered_tool_` filter passes Windows2/2 (2.12s), Linux2/2 (2.02s), with the separately isolated native child test ignored as intended. No server listeners were created. All repo/database mutations are disposable fixtures.
