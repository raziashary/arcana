# P4-39 Dependency Completion Test Work — 2026-10-05

## Scope

Add one focused scheduler integration regression test in `arcana-core/src/recipes/scheduler.rs` for dependency completion waking a persisted Supervisor and routing/claiming/completing its ordinary dependency child, while a budget-exceeded sibling remains unresolved and unclaimable.

## Progress

- [x] Created this report before investigation.
- [x] Read MDP §0 and P4-39.
- [x] Drafted assertions from the user requirement and P4-39 before reading implementation bodies.
- [x] Read public interfaces and relevant scheduler/status-tool/test-support source.
- [x] Add the focused test using the child-test-process isolation seam.
- [x] Record the test name, evidence, limitations, and unresolved findings.
- [x] Coordinator-run formatting and exact Windows/Linux verification pass.

## Evidence and Findings

### Assertion draft

The focused test must drive the production `UpdateTaskStatusTool` completion path for a prerequisite whose persisted dependency child starts blocked. The test must then observe the persisted Supervisor wake, read the newly eligible ordinary child through the Supervisor's real task-list consumer, route that child through ordinary deterministic dispatch, and atomically claim and complete it through its saved worker recipe/AgentEngine path. A sibling held for budget exhaustion must remain `needs_info`; automatic wake/dispatch must not resolve it, and a claim attempt without explicit budget/user resolution must fail while preserving the held state. The fixture must isolate the global scheduler singleton from other tests and avoid live providers or credentials.

These assertions follow the task text and MDP §10 P4-39. Whether the current public interfaces and safe test seams can observe every step remains to be established from source.

### Source findings

- `UpdateTaskStatusTool` invokes dependency reconciliation for `done`, then calls `scheduler::kick("supervisor")` when rows become ready (`arcana-core/src/agent/builtin_tools/update_task_status_tool.rs`, `unblock_completed_task`).
- `AssignTaskTool` persists automatic routing history and invokes `scheduler::kick(worker_slug)` (`arcana-core/src/agent/builtin_tools/assign_task.rs`). The scheduler resolves the persisted worker row and recipe before spawning `RecipeRunner::run_recipe` (`arcana-core/src/recipes/scheduler.rs`, `kick_worker`).
- `RecipeRunner::with_provider_for_test` preserves production scheduler dispatch, saved-recipe resolution, AgentEngine, and tool execution while replacing only provider construction (`arcana-core/src/recipes/runner.rs`). Existing test support uses `with_home` to isolate Arcana storage.
- `GLOBAL_SCHEDULER` and its kick-inflight set are process-wide `OnceLock`s (`scheduler.rs`). Running the exact current test executable/name in a child process gives the fixture a separate process-global state; the parent test process will not install a scheduler.
- Existing `shipped_status_tool_completion_unblocks_a_dependent_for_supervisor_dispatch` in `recipes/defaults.rs` covers tool-level unblock/list/automatic-route/claim/held-state behavior, but not the scheduler wake through the persisted Supervisor and saved worker recipe completion. This test will cover that missing integrated consumer path.

### Added test

`recipes::scheduler::tests::dependency_completion_wakes_persisted_supervisor_and_completes_saved_worker_recipe` now runs its fixture in an exact-name child invocation of the current Rust test executable. In that child, it installs a test scheduler, completes an in-progress prerequisite through `UpdateTaskStatusTool`, and uses a scripted provider override with real scheduler kicks, persisted Supervisor/backend worker recipe resolution, AgentEngine, and registered tools. The Supervisor consumes `list_tasks`, routes the ordinary child through `assign_task` without an override, and the saved backend worker recipe calls `claim_task` followed by `update_task_status(done)`. Assertions inspect the routed owner, successful worker claim result, Supervisor ready-list output, and held sibling's `needs_info`/budget reason plus refused claim.

The fixture has no live provider, credential, network, or database dependency. Child-process isolation contains both `GLOBAL_SCHEDULER` and the process-wide kick-inflight set. The test and report are the only intended edits. Tests/builds/formatting were not run here, per coordinator instruction; the coordinator owns that verification. No CLI or live-Fleet proof is claimed, and P4-39 remains open pending that broader owner acceptance evidence and the separate owner-approved budget resolution.

## Focused Linux sandbox review

Read-only review of `arcana-core/src/platform/process_sandbox/linux.rs`'s current diff found the seccomp filter applied after `PR_SET_NO_NEW_PRIVS` and Landlock setup. On x86_64 and aarch64 it validates the audit architecture, rejects x32 on x86_64, denies endpoint socket and network I/O syscalls plus io_uring/process-inspection calls, and permits only `AF_UNIX` `socketpair` for the launcher's exec-error channel. The added production-launcher regression exercises denied socket families, connect and selected bypass syscalls in both the launched Python process and its descendant, while checking a local host endpoint works and receives no connection.

No concrete defect was identified in this narrow syscall-level network-denial path by source review. This is not execution evidence and does not establish complete network isolation across every possible inherited descriptor or kernel interface; the filesystem confinement rules are outside this review's claim.

## Final Linux network-boundary review — 2026-10-05

### Scope and progress

Review the final Linux network filter and Linux process-spawn setup in `arcana-core/src/platform/process_sandbox/linux.rs` and `mod.rs`, including inherited descriptors and `pre_exec` ordering. Scope excludes external-file read allowlisting, which remains unfinished and is not claimed. No tests/builds or repository mutations are authorized.

- [x] Appended this review scope before source investigation.
- [x] Read final Linux network filter and spawn setup.
- [x] Check relevant libc syscall/flag definitions locally.
- [x] Record the scoped review result.

### Findings

No concrete network-capability bypass or `pre_exec` ordering defect was identified in the requested boundary by source review.

This final-state review supersedes the earlier focused note above: the current source now explicitly closes ambient descriptors at exec and the regression includes a deliberately non-CLOEXEC host socket. The filter itself permits `sendmsg`/`recvmsg` to support Rust's exec-status channel; that remains safe for network isolation only while the spawn contract keeps host network descriptors out of the post-exec child.

- `process_sandbox::spawn` replaces fd 0 with null and uses private pipes for fd 1/2 before calling `linux::confine`; it does not pass user-selected host descriptors (`arcana-core/src/platform/process_sandbox/mod.rs`, `spawn`).
- `confine` invokes `close_range(3, UINT_MAX, CLOSE_RANGE_CLOEXEC | CLOSE_RANGE_UNSHARE)` before `PR_SET_NO_NEW_PRIVS`, Landlock, and seccomp. The unshare flag keeps this fd-table change child-local; CLOEXEC marks descriptors to close at successful exec, leaving Rust's private exec-status channel usable while pre-exec errors are reported. In the libc 0.2.186 source available locally, these flags are `CLOSE_RANGE_UNSHARE = 1 << 1` and `CLOSE_RANGE_CLOEXEC = 1 << 2` (`.../libc-0.2.186/src/unix/linux_like/linux/mod.rs`).
- The seccomp policy rejects non-native audit architectures and x32 on x86_64, allows only `socketpair(AF_UNIX)`, and rejects the endpoint socket operations plus io_uring and cross-process descriptor/memory access. `sendmsg`/`recvmsg` remain available for the private exec-status/anonymous Unix-pair path. With all ambient fds >=3 CLOEXEC and stdio replaced, those allowed message operations do not expose a host network socket to the post-exec child; descendants inherit the filter and start with only their parent's confined descriptors. The added regression duplicates a live host TCP socket to a deliberately non-CLOEXEC descriptor 200 and has the child assert `fstat(200)` fails with `EBADF`, directly covering the ambient-descriptor premise; it also confirms the external listener receives no child connection.
- Local libc 0.2.186 declarations confirm the used x86_64/aarch64 `SYS_socketpair`, `SYS_sendmsg`, `SYS_recvmsg`, and `SYS_close_range` constants are available in their GNU b64 definitions. This is declaration-level corroboration only, not kernel execution evidence.

Scope limits: no tests/builds were run. This review does not claim complete filesystem read isolation (the external-file allowlist is unfinished), and it does not prove every kernel interface or hostile host-process interaction. Main-session verification remains necessary; owner disposition stays with Razi under MDP §0.2c.

## Coordinator verification and gate disposition

The final fixture uses saved recipe prompts for role identity, explicit disposable-workspace consent, compact=false ready-list readback and joined scheduler shutdown. The shipped DONE status tool unassigns a task when assigned_worker is omitted; completion therefore asserts no remaining assignment and reads its production routing-history success attributed to worker-backend. No lifecycle behavior was changed to satisfy the fixture.

- Windows exact scheduler filter: 1/1 pass, 1.43s (`target/p4-dependency-completion-windows.txt`).
- Linux exact scheduler filter: 1/1 pass, 3.30s (`target/p4-dependency-completion-linux.txt`).
- Workspace cargo check passes (29.60s); workspace format/diff checks pass.
- TG4-06 is proven: the completion event automatically wakes and executes persisted Supervisor/backend recipes, makes the dependent claimable, and progresses it to DONE without a human dispatch. Provider responses are deterministic fixtures; all scheduler/recipe/engine/tool/database consumers are production paths.
- P4-39 stays open: set_task_budget currently updates saved caps without clearing budget_exceeded. A Supervisor/user-only audited explicit resolution is proposed and awaiting owner direction; this fixture deliberately proves automatic dispatch cannot release that hold. No successful live CLI Fleet or Calendar acceptance is claimed.
