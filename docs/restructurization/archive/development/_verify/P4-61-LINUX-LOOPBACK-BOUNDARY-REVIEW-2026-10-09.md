# P4-61 Linux Loopback Boundary Review

Date: 2026-10-09
Scope: Source-only review of `arcana-core/src/platform/process_sandbox/linux_smoke.rs` against the owner contract for Linux private USER+NET networking, exact approved host `127.0.0.1` listener proxy, untrusted project code, filesystem denial outside user files, outbound network denial, no host socket/control authority, process-tree cancellation cleanup, and resource bounds.

Constraints: No source edits, execution, tests, formatting, git, network, environment/secret/userdata access, or further delegation. Candidate module is not enabled.

## Review progress

- [x] Created report before investigation.
- [ ] Trace sandbox setup, untrusted process launch, IPC and descriptor ownership.
- [ ] Trace namespace/capability, filesystem, network, cleanup, and resource-control paths.
- [ ] Record severity-sorted, source-cited actionable findings and distinguish confirmed defects from proof gaps.

## Findings

(To be appended as evidence is confirmed.)

### Confirmed findings

#### High — Sandbox code can kill its supervisor and leave the command tree running

- **Evidence:** `child_seccomp_filter` denies `kill`, `tkill`, and `tgkill` but its syscall deny list at lines 588–590 omits `rt_sigqueueinfo` (and `rt_tgsigqueueinfo`); after the list, the filter returns `ALLOW` for other calls at lines 591–595. The helper starts the untrusted executable without setting a parent-death signal at lines 442–463. Its only process-group cleanup is performed by the still-running helper at lines 472–499; the parent wrapper merely clears forwarding in `Drop` and explicitly sets `kill_on_drop(false)` at lines 104–109 and 174–181.
- **Reachability / impact:** The sandboxed process can obtain the helper PID with `getppid()` and invoke `rt_sigqueueinfo(helper_pid, SIGKILL, ...)`. The helper and sandbox command share the same mapped UID in the new user namespace. If permitted by the host kernel's same-UID signal check, helper death bypasses the loop's process-group kill; the command and descendants can continue running after the parent observes the helper exit or drops the wrapper. This breaks process-tree cancellation and can leave untrusted work consuming resources indefinitely. This is source-confirmed syscall-filter coverage; host-kernel behavior should be verified in the project's later isolated proof, which is outside this review's constraints.
- **Remediation direction:** Deny all signal-delivery syscalls that can target another process (`rt_sigqueueinfo`, `rt_tgsigqueueinfo`, and audit the full signal syscall family), or otherwise make helper termination reliably kill the confined process group. Add a parent-side cleanup path that kills/reaps the sandbox group whenever the helper exits unexpectedly; don't rely only on helper-owned cleanup.

#### High — Supervisor error exits can orphan the confined process

- **Evidence:** After `Command::spawn()` succeeds at line 462, `proxy_until_child_exit` uses `?` on `child.id()`/socket clone/timeout setup at lines 467–469, `child.try_wait()` at line 472, and forwarding-thread creation at lines 487–493. These error returns bypass the explicit group kill/reap paths at lines 474–475 and 497–499. `helper_main` propagates the error at line 259, while the outer parent wrapper has `kill_on_drop(false)` and does not kill the confined process group when the helper exits (lines 104–109, 174–181).
- **Reachability / impact:** A local resource/thread creation failure after the confined command has started (notably failure to create a forwarding thread after an accepted connection) makes the helper exit while the command group survives. This violates cancellation/cleanup and gives untrusted code a way to remain active when the proxy hits ordinary resource pressure.
- **Remediation direction:** Use a cleanup guard around the spawned command group and disarm it only after an observed/reaped exit. Ensure every post-spawn error path kills the group and waits for the direct child before returning.

#### High — No bound limits the sandboxed process tree's resource consumption

- **Evidence:** The only explicit bounds are request bytes, argument count, and forwarded client count at lines 23–25, 167–170, 282–285, and 481–484. The seccomp filter intentionally permits ordinary `clone`/fork and subprocess creation at lines 591–595, and there is no process, memory, CPU, file-size, or workspace-write quota setup in `run_confined_child` (lines 442–464) or the surrounding launch path.
- **Reachability / impact:** Any selected untrusted project executable can fork/clone without limit, allocate memory, run CPU loops, or fill the writable workspace. It can exhaust host-wide process/memory/CPU/disk resources and impair the desktop app or other workloads. The forwarded-client cap does not constrain these resources.
- **Remediation direction:** Place the command tree in enforced OS resource controls before untrusted execution (for example, cgroup limits where available, plus suitable rlimits and bounded writable storage). Fail closed when required limits cannot be applied, and make their inheritance cover all descendants.

## Boundary checks and hardening notes

- The parent launch path rejects non-IPv4-loopback or ephemeral listener binds at lines 149–153. The helper verifies the control descriptor is AF_UNIX/SOCK_STREAM at lines 262–279. The helper validates canonical workspace/cwd/executable/runtime roots and NULs at lines 282–329; it enters fresh USER+NET namespaces, raises only `lo`, clears capability sets, and fails closed on setup errors at lines 331–399. Landlock requires ABI 3 and grants write access only to the workspace and `/dev/null`, while executable/runtime roots receive read/execute rights at lines 404–440. The sandbox exec marks inherited descriptors CLOEXEC before applying Landlock and seccomp at lines 449–460. These source paths support the intended basic network and filesystem boundaries, subject to caller-selected roots.
- Environment credentials already present in the helper are scrubbed before request processing at lines 235–239; explicit overrides whose **names** are credential-shaped are rejected at lines 310–312. However, `env_set` otherwise accepts arbitrary values under innocuous names. This is a **hardening suggestion, not a confirmed vulnerability**: keep the eventual integration's environment allowlist narrow and ensure no secret values are supplied under names that evade the classifier. The current source does not show a consumer passing such values.
- The bridge request is bounded to 128 KiB, parsed as JSON, and protocol-version checked at lines 167–170 and 242–252. The SCM_RIGHTS channel is private to the parent/helper in the intended launch path; sandboxed code does not inherit its descriptor because of the CLOEXEC sweep at lines 449–460. `receive_fd` does not reject `MSG_CTRUNC` or validate the full ancillary header length at lines 543–560. Since the peer is the trusted parent accept thread, this is a defense-in-depth hardening opportunity rather than a demonstrated untrusted-input path in this candidate.

## Review completion

- [x] Traced untrusted process launch, namespace/capability setup, Landlock/seccomp, control-channel descriptor flow, serialization, environment scrub, and post-spawn error cleanup.
- [x] Findings above are severity-sorted and include reachability, impact, and remediation direction.
- [x] Candidate is not registered/enabled; this report covers source paths only and does not claim runtime proof.
