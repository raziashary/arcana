# P4-61 Linux loopback runner candidate

Status: design and source review in progress. This candidate is not production-enabled; security proof remains pending.

## Work log

- Created this report before repository investigation, as required.
## Source findings

- `arcana-core/src/platform/process_sandbox/linux.rs` currently adds Landlock rules for writes under the workspace plus `/dev/null`; its handled rights are write rights only, so it does not deny outside-workspace file reads. Its seccomp filter denies socket creation and network syscalls, except AF_UNIX `socketpair` for Rust's exec-status reporting. There is no namespace bridge there.
- `arcana-core/src/platform/process_sandbox/mod.rs` compiles `linux` only on Linux, but `run_bridge_entrypoint()` currently dispatches only on Windows and returns `None` elsewhere. The existing Linux production `spawn` launches the requested program directly and applies `linux::confine` in `pre_exec`.
- `arcana-core/src/platform/process_sandbox/windows.rs` establishes a precedent for an internal executable argument, bounded serialized request/result, validation before child launch, and fail-closed bridge errors. Its code is Windows-only and does not solve Linux namespace or proxy ownership.
- Durable Arcana memory says native port completion needs a real consumer plus behavior proof; this candidate must stay marked pending until the host entrypoint, production consumer, negative tests, and end-to-end evidence are integrated and verified.
## Candidate module and current limit

Added `arcana-core/src/platform/process_sandbox/linux_smoke.rs` as a fail-closed private-mode sentinel for `--arcana-loopback-bridge`. It deliberately does not create a namespace or launch a child. The file is not registered in `mod.rs`; no production behavior changes, and it is not yet an operational bridge.

This initial assessment was superseded by the coordinator's approval to implement a bounded, unregistered module candidate within the two-file boundary. Later sections record that implementation and the remaining integration obligations. The production consumer still needs to adapt output capture, wait/status, timeout/cancellation, and descendant cleanup to its `SandboxedChild` contract.

## Required integration before this can run

1. Extend `ProcessSpec` with canonical Linux runtime read roots and the explicit opt-in/selected loopback endpoint. Derive roots from the selected executable and its required runtime only; do not grant PATH, home, or arbitrary host directories. Validate each root and executable at launch. Existing Windows `read_execute_roots` is platform-specific and cannot be copied as a Linux policy.
2. Make Linux Landlock handle read, execute, and write rights, deny outside-root reads by default, grant the canonical workspace read/write, exact runtime roots read/execute, and only narrowly required system loader/runtime roots. Keep `/dev/null` narrowly handled. Verify supported Landlock ABI and fail closed when any requested right is unsupported.
3. Add a parent-owned proxy lifecycle to `spawn`: bind only `127.0.0.1` and the caller-selected port, pass accepted client FDs to the trusted helper over a private Unix socketpair using `SCM_RIGHTS`, and relay bytes to the exact server port on namespace loopback. The internal channel must be authenticated/validated and closed in the child before exec. No path may pass a host-connected socket into untrusted project code.
4. In the pre-runtime helper, require the exact private argv shape and a valid inherited control FD, then create USER+NET namespaces, write `setgroups=deny` and UID/GID maps, bring up only `lo`, confirm no non-loopback interfaces/routes, drop supplementary groups/capabilities, and launch only the validated argv/cwd/environment. Apply Landlock and seccomp before exec; the child may use network sockets only inside the isolated namespace, while host AF_UNIX/host descriptors remain unavailable. Close all helper control and namespace FDs in the untrusted child.
5. Preserve `SandboxedChild` stdout/stderr/wait/drop semantics, including killing the helper, child, and descendants when the consumer drops or cancels. Tauri calls the shared `run_bridge_entrypoint()` from synchronous `main`; CLI currently calls it inside `#[tokio::main]` after runtime creation and must move dispatch earlier. Register Linux only after its protocol is complete.

## Required proof before enablement

- Per-OS Linux proof on supported x86_64 and aarch64 kernels: positive namespace setup, loopback-only interface/route, child can serve on only the expected private loopback target, and an approved host `127.0.0.1:<port>` client can reach it through the Arcana proxy.
- Negative behavior: disabled/unavailable user namespaces, unmappable UID/GID, unsupported Landlock ABI/right, malformed helper argv/protocol, another host port/address, outbound IPv4/IPv6, host Unix sockets, outside-workspace file reads/writes, leaked control/accepted FDs, and process cancellation all fail or remain unreachable as applicable.
- Real consumer proof through `ProcessSpec` and `SandboxedChild` from the CLI and Tauri early entrypoint, with outputs and exit status observed. Source presence or namespace creation alone is not completion evidence.

No compilation, tests, formatters, integration, or runtime proof were run by this task. The bridge remains unavailable and must not be reported enabled or secure from this source alone.
## Coordinator direction

The coordinator approved a module-local candidate API accepting explicit canonical runtime read roots and asked that namespace, parent listener/SCM_RIGHTS, and runner APIs remain in this new module for later integration. Implementation is now proceeding against that bounded boundary; the helper remains unregistered and security proof is still pending.
## Updated implementation state

The initial fail-closed sentinel note above is superseded by the current unregistered source candidate. `linux_smoke.rs` now exposes module-local `spawn(...)`, `LinuxLoopbackChild`, and `run_bridge_entrypoint()` APIs. The launcher requires an already-bound exact `127.0.0.1:<nonzero-port>` listener, an absolute selected executable, canonical workspace/cwd, and at least one explicit canonical runtime read root; argv/environment serialization fails on non-UTF-8 values. It starts the current Arcana executable in its private helper mode, transfers only accepted client FDs by `SCM_RIGHTS`, and the helper connects those to the same port on namespace loopback.

The helper checks request bounds/canonical paths, scrubs inherited credential-shaped environment variables before launching project code, enters USER+NET namespaces, writes the UID/GID maps, brings up `lo`, checks interface/route tables, clears capability sets, and prepares a Landlock read/execute allowlist plus workspace write access. The untrusted child receives only stdio; close-range CLOEXEC removes helper control/ambient descriptors at exec. Its seccomp filter permits IP sockets for namespace-local serving and anonymous AF_UNIX socketpairs, while blocking named AF_UNIX sockets, namespace changes, namespace-creating `clone` flags, and selected process-introspection/escape syscalls.

## Source-review limits requiring proof or follow-up

- Nothing in this file is registered or reachable yet. Do not call it enabled or secure. No compilation, tests, formatting, per-OS runtime proof, or smoke test was performed, per task scope.
- The SCM_RIGHTS relay has a source-level admission cap of 32 forwarded clients. Verify that limit under load and check descriptor/thread cleanup on both successful and failed forwarding.
- The USER namespace mapping sequence, supplementary-group clearing, loopback ioctl structure/flags, close-range behavior, Landlock rights handling, seccomp BPF jump behavior, IPv6 route check, and process-group cleanup all need Linux runtime verification on supported x86_64 and aarch64 systems. Every unsupported path returns failure, but source review alone cannot establish correct kernel behavior.
- Runtime roots must be selected and validated by the eventual caller; dynamic loader and shared-library roots must be supplied explicitly. The module intentionally does not infer PATH/home/system read roots. Coordinator integration must pass only app-approved roots and account for the existing Linux `ProcessSpec` lacking a runtime-read-path field.
- The process-group migration/cleanup and drop/cancel behavior need negative behavior proof. Kernel or machine failure remains outside this source-level guarantee.
- `run_bridge_entrypoint()` is module-local and not wired from the shared dispatcher. Once proven, the coordinator must wire Linux dispatch through `process_sandbox::run_bridge_entrypoint()`; both CLI and Tauri already call that shared function before normal startup.

## Coordinator integration checklist update

Before registration, the coordinator must also change the CLI startup ordering: `src-cli/src/main.rs` currently calls the shared bridge dispatcher inside `#[tokio::main] async fn main`, after Tokio may have started multiple threads. The Linux candidate checks `/proc/self/task` and refuses helper mode unless exactly one thread is present, because USER namespace creation is not safe/available from a multithreaded process. Move the private helper dispatch to a synchronous CLI entrypoint before building Tokio, then enter the async CLI body only for ordinary startup. Tauri already calls the shared dispatcher from its synchronous `main` before runtime initialization.

After proof, integration shape is: declare `#[cfg(target_os = "linux")] mod linux_smoke;`, dispatch Linux `run_bridge_entrypoint()` from the shared function, add an explicit opt-in/approved listener and canonical runtime roots at the existing process-sandbox consumer, adapt `LinuxLoopbackChild` to `SandboxedChild`, and preserve the current workspace cache environment plus process/output cleanup contract. Do not use this candidate for ordinary commands until that consumer supplies the explicit listener and runtime authority.


- The candidate denies `kill`, `tkill`, and `tgkill` in the sandbox child to prevent signaling same-user host processes because this design does not create a PID namespace. This may restrict runtimes that use signals to manage their own subprocesses; verify intended server/runtime compatibility before integration. The supported contract remains pending proof.

## Follow-up boundary review work log (2026-10-09)

- Coordinator requested fixes for signal-syscall bypasses, RAII cleanup after child start, and standard bounded resource controls where available. The focused review is being read now; helper remains unregistered and no proof claim changes.
- Patched the child seccomp deny list to include `rt_sigqueueinfo` and `rt_tgsigqueueinfo` alongside `kill`, `tkill`, `tgkill`, and pidfd signaling.
- Added a child-process-group RAII guard immediately after successful spawn, and arranged for the helper to leave its private process group after launching the untrusted child. The parent can therefore kill the helper's original process group by its known PID on wait/drop, including when the helper exits unexpectedly; all helper-side `?` paths after child launch unwind through the guard. Source review only; no runtime claim.
- Added mandatory caller-supplied CPU-time, address-space, per-file-size, and open-file rlimits plus zero core-dump limit. These are per-process ceilings, not aggregate tree quotas.
- The focused review also identified possible self-signal bypasses. The child seccomp deny list now covers `kill`, `tkill`, `tgkill`, `rt_sigqueueinfo`, `rt_tgsigqueueinfo`, `pidfd_open`, and `pidfd_send_signal`; the bridge remains disabled pending syscall and behavior proof.

## Follow-up implementation notes (2026-10-09)

The parent starts the helper in a dedicated process group, so the group ID is known immediately from the helper PID without a status pipe or a race-prone post-start update. The untrusted child inherits that group; after the child is guarded, the trusted helper moves itself back into the parent's process group. The child cannot change its group/session under the seccomp policy. Parent wait/drop and helper-side RAII cleanup both target the sandbox group, and parent drop also kills the helper directly. This is a candidate lifecycle design only; process-group reuse, races, and kernel behavior still require Linux runtime proof.

The parent control request write and accept-thread startup failures explicitly terminate and reap the helper. Parent wait errors trigger group termination and a helper kill/reap attempt. Inside the helper, any error after child spawn—including proxy FD receive/forwarding thread startup errors—drops `ChildGroupGuard`, which kills the group and waits for the direct child. No run/build/test was performed to validate these paths.

The workspace pins `libc` 0.2.186; source inspection confirms existing Linux code already uses `SYS_close_range` and `CLOSE_RANGE_CLOEXEC | CLOSE_RANGE_UNSHARE`. The candidate's syscall and RLIMIT identifiers are Linux libc API names expected on its x86_64/aarch64 target guards, with no new dependency. Compilation was explicitly out of scope, so target-specific constant availability and the module as a whole remain unverified until the coordinator's authorized Linux build/proof phase.

## Coordinator source correction (2026-10-09)

The coordinator identified two compile/lifetime issues by source review: the parent launcher awaited child reaping while declared synchronous, and the duplicated control `OwnedFd` did not live through `Command::spawn`. The launcher is now async, matching the shared sandbox lifecycle, and the duplicated descriptor is bound outside the unsafe setup block so it stays open through spawn. No compiler or runtime check was run.

## Validation-test source work (2026-10-09)

Coordinator requested module-local `cfg(test)` checks for explicit nonzero resource limits, canonical workspace/cwd containment, credential-shaped environment overrides, malformed request bounds, and exact IPv4 loopback listener admission. Tests will exercise the same pure validation helpers used by `spawn`/request validation, create only temporary directories as needed, and bind only the explicitly requested loopback listener case. No helper process, namespace request, outbound network request, compiler, or test runner will be used.

Coordinator refined the runtime-root contract: explicit canonical runtime roots must support individual regular files as well as directories, to avoid granting broad binary directories for selected executables. Implementation will retain exact caller-selected roots, apply file-appropriate Landlock READ_FILE|EXECUTE rights without READ_DIR, reject other file types, and add focused validation coverage. No default/system root inference is authorized.

Coordinator identified a standard-library relay API issue: `std::io::copy_bidirectional` does not exist. The forwarding path will be replaced with two bounded directional `std::io::copy` workers per connection, TCP half-close behavior, explicit connection shutdown/join on cleanup, and the existing maximum of 32 active connections. No detached forwarding resources will remain by design; runtime behavior remains unverified.

The source candidate now has five request/listener tests plus one exact runtime file/directory root test under `linux_smoke::tests`: `request_requires_explicit_nonzero_resource_limits`, `workspace_and_cwd_must_be_canonical_and_contained`, `runtime_allowlist_accepts_exact_canonical_files_and_directories`, `credential_shaped_environment_overrides_are_rejected`, `request_lengths_and_structural_bounds_are_enforced`, and `only_an_already_bound_exact_ipv4_loopback_listener_is_admitted`. Expected focused Cargo filter after the coordinator registers this module: `cargo test -p arcana-core linux_smoke::tests::`. These tests are source-only and were not compiled or run here.

## Resource-bound contract requiring owner choice

The candidate now requires caller-supplied `ResourceLimits` and applies these inherited per-process rlimits before exec: `RLIMIT_CPU`, `RLIMIT_AS`, `RLIMIT_FSIZE`, `RLIMIT_NOFILE`, plus `RLIMIT_CORE=0`. The owner must select the numeric ceilings for CPU seconds, virtual address bytes, per-file bytes, and descriptors before integration. Zero is rejected and any kernel refusal aborts launch.

These do not impose aggregate process-tree CPU/RSS, process-count, or workspace disk limits: each descendant can receive its own per-process allowance, `RLIMIT_FSIZE` is per file, and no `RLIMIT_NPROC` is installed because it is charged against the host real UID and can disrupt unrelated user processes. The remaining contract is an owner decision: either require a delegated cgroup v2 subtree with chosen `pids.max`, `memory.max`, and `cpu.max` values plus an enforceable workspace disk quota, failing closed when unavailable; or keep the candidate disabled until an equivalent per-run OS quota boundary exists. Do not describe the rlimits alone as aggregate resource confinement.

## Coordinator first Linux compile

Test-only module registration deliberately exposes the candidate to the real Linux compiler without enabling production smoke. The first compile found nine type errors: Tokio child.id is optional, IPv4 address access returns a reference, and Linux setrlimit uses an unsigned resource selector. These are corrected without changing capabilities or limits. This is not a runtime security proof; the focused validation rerun remains required.
