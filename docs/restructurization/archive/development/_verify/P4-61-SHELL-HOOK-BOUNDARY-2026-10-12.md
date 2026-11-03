# P4-61 Shell Hook Boundary — 2026-10-12

Historical initial status; superseded by the verified Windows follow-up at the end of this report.

The source trace and initial implementation notes below describe the pre-approval state; the verified follow-up at the end supersedes those status claims.

## Approved contract

ADR-LOG-RESTRUCT-0365 approves routing shell hooks through the selected-workspace write lease and existing OS sandbox, with confirmed process-tree cleanup; platforms without the required capability must fail closed. No remote side effect or billing guarantee is implied.

## Source trace

- `HookExecutor::run_action` previously used raw `tokio::process::Command::{cmd /C, sh -c}` and `Command::output()`, bypassing both the workspace lease and `platform::process_sandbox`.
- PreToolUse hooks run before the tool handler acquires its write lease. PostToolUse hooks run after the handler returns and releases the lease. PostFileWrite hooks from `write_file` and `edit_file` are detached, so they execute after the writer releases its lease. No synchronous same-lease re-entry was found in the inspected write, append, or annotation paths.
- Shell hook contexts may carry `extra.working_dir`; contexts without it resolve the configured selected workspace or fail closed. The root owner is adding `extra.working_dir` to the detached PostFileWrite producers so a session-specific workspace is carried through. An action-level working directory must resolve to an existing directory inside that selected workspace.
- Linux's production sandbox is not approved for this shell boundary under the strict read/network contract; macOS process execution reports the XPC runner unavailable. Both must refuse shell hooks before spawning.
- The existing sandbox API initially had no uniform async tree-stop-and-confirm operation. The root owner added `SandboxedChild::terminate_and_wait_tree`; Windows now waits until the host job reports zero active processes, with a five-second deadline, while Unix returns `Unsupported` so shell hooks remain disabled there. Root is adding a Windows bridge membership/stop regression fixture. The owner has not approved how to recover/quarantine if this stop confirmation fails. Therefore a hard-coded Windows launch admission gate currently refuses every shell hook before acquiring the lease or spawning. The Windows sandbox execution and timeout path compile behind that gate but are not enabled or behavior-proven.

## Implementation and proof

The executor resolves the selected workspace, rejects out-of-workspace action directories, and keeps the Windows sandbox launch code behind the fail-closed admission gate. If approved later, that path acquires `WorkspaceWriteLease`, scrubs credential-shaped environment variables, launches only through `ProcessSpec` and `process_sandbox::spawn`, and concurrently drains stdout/stderr with a 50,000-byte retained tail. Linux/macOS refuse before spawn. The scoped tests are `hooks::executor::shell_boundary_tests::shell_hook_` (missing workspace, working-directory escape, Windows owner-gate refusal; non-Windows sandbox-policy refusal). The exact command was run with all provider/database credential variables removed: `$env:GEMINI_API_KEY=$null; $env:GOOGLE_API_KEY=$null; $env:OPENAI_API_KEY=$null; $env:ANTHROPIC_API_KEY=$null; $env:DATABASE_URL=$null; cargo test -p arcana-core hooks::executor::shell_boundary_tests::shell_hook_ -- --nocapture --test-threads=1`. Compilation stopped before test execution because the concurrently owned `agent/tool_executor.rs:2189` test helper calls `.lock()` on an `Option<Shared<Mutex<AppState>>>`. No hook test passed or failed, and no production cancellation behavior is proved. `rustfmt --edition 2024 arcana-core/src/hooks/executor.rs` completed successfully.

## Reentrancy note

The inspected `write_file` and `edit_file` PostFileWrite hooks are spawned asynchronously after their file operation; `append_file` has no PostFileWrite dispatch, and annotation creation persists to its database without a PostFileWrite hook. Acquiring the lease inside the hook therefore does not nest under those tool-held leases. This finding is source inspection, not a claim that arbitrary future hooks cannot re-enter a writer.
## Verified follow-up — 2026-10-12

Owner decisions ADR-LOG-RESTRUCT-0365 and ADR-LOG-RESTRUCT-0367 authorize sandboxed shell hooks and require child, cleanup admission, and workspace write lease ownership to remain retained until the OS confirms the process tree is empty. The implementation now routes Windows shell hooks through the selected-workspace write lease and experimental SBOX launcher. A normal completion confirms the job tree is empty before releasing the lease; timeout, wait/output failure, cancellation, or failed confirmation transfers ownership to the bounded cleanup worker, which quarantines the workspace until confirmation. Unsupported non-Windows shell hooks return before spawn.

Credential/database variables were removed for these Windows tests:

- cargo test --offline -p arcana-core --lib --locked shell_hook -- --nocapture --test-threads=1: 5 passed, including selected-workspace marker write and missing/escaped workspace refusals.
- cargo test --offline -p arcana-core --lib --locked failed_stop_keeps_real_write_lease_and_retry_releases_it_after_confirmation -- --nocapture --test-threads=1: 1 passed.
- cargo test --offline -p arcana-core --lib --locked dropped_hook_owner_retains_process_and_write_lease_until_cleanup_confirms_empty -- --nocapture --test-threads=1: 1 passed.

cargo fmt --all -- --check and git diff --check pass. The lease is observed unavailable while a child is alive and available only after the Windows job reports empty. This proves the Windows shell-hook execution/cleanup sub-slice; it does not prove the complete project-command matrix, guarded staging/copy-back, dependency staging, Linux/macOS execution, or loopback smoke capability. P4-61/TG4-31 remain open.

## Current closeout — 2026-10-13

Normal core/CLI compilation passes19.14s with this owning source. The current isolated Windows library test binary was used for finite closeout checks without another Cargo build: shell_hook5/5(.39s); failed-stop lease retention1/1(.42s); dropped hook-owner actual cleanup1/1(.19s); real bridge/command/descendant termination1/1(1.30s). The latter reads the Windows job active-process count before/after and proves heartbeat writes cease. Credentials were cleared per command; fixture trees are owned temporary paths, not user DB or Calendar. All selected test processes completed, with confirmed tree cleanup.

A scoped rustfmt invocation initially traversed the concurrently owned unfinished Linux staging source and reported formatting differences there; no hook-source formatting defect was reported. Closeout uses skip_children=true on the reviewed Windows/hook files and leaves the Linux author to format their own candidate. Shared Execute/FreeformShell/host-install lease integration is being validated separately before this source unit ships. P4-61/TG4-31 remain open for staged process integration, locked dependencies and required supported-platform execution.
