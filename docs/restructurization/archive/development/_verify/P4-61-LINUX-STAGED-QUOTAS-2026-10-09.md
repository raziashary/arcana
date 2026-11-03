# P4-61 Linux staged workspace and cgroup quotas — 2026-10-09

## Progress

- Created this report before repository investigation, as directed.
- Scope is limited to `arcana-core/src/platform/process_sandbox/linux_smoke.rs` and this report.
- No builds, tests, process launches, network access, environment mutation, credentials, user database access, or Git mutations will be performed.

- Quick memory pass found no Arcana P4-61/Linux sandbox/quota entry in the searchable memory registry; current repository evidence will govern.
- The coordinator supplied kernel documentation URLs but prohibited network access, so I will not fetch them; implementation will follow the explicit approved requirements and existing local code contracts.

- Initial source review: `linux_smoke.rs` is a test-only USER+NET namespace helper; current cleanup tracks a process group and listener thread, with per-process rlimits documented as insufficient for aggregate tree quotas.

- The candidate currently launches a trusted helper in its own process group, then the helper unshares USER+NET, installs Landlock/seccomp/rlimits, and launches the selected child. Parent cleanup only sends `SIGKILL` to the helper process group; it does not own a cgroup or join the accept-forwarder thread.
- `process_sandbox/mod.rs` compiles this module under `cfg(all(test, target_os = "linux"))`; there is no caller outside the module today. The change will therefore remain in this unintegrated test candidate.
- MDP P4-61 and TG4-31 remain open for cross-platform proof; the local host docs also state Linux delegated cgroup launch proof is unavailable in WSL. This implementation can add admission/lifecycle code but cannot establish kernel enforcement here.

## Implemented candidate slice

- Added explicit `CgroupLimits` and required `cgroup_root`/limits arguments to the unintegrated Linux smoke `spawn` function. Admission rejects non-absolute/noncanonical roots, unavailable or disabled `cpu`/`memory`/`pids` controllers, malformed control contents, missing `cgroup.kill`, and controls that cannot be opened for writing. It never discovers a root or falls back.
- Each launch creates an exclusive PID/sequence-named child cgroup, writes `cpu.max`, `memory.max`, and `pids.max`, and reads each back to verify the requested values. The trusted helper PID is written to that cgroup from `pre_exec` using a pre-opened `cgroup.procs` descriptor before helper exec; the untrusted child inherits membership.
- `LinuxLoopbackChild` retains the cgroup lease and accept-forwarder join handle. Wait/drop stops and joins the accept thread, sends `cgroup.kill`, polls `cgroup.events` until `populated 0`, and removes only the owned cgroup directory with `remove_dir`; failure to confirm emptiness leaves the cgroup in place.
- Added cfg(test) admission coverage for positive/invalid quotas, an invalid relative path, fake controller availability/enabled state and malformed controls, and refusal of `/sys/fs/cgroup` on a nondelegated host. Fake ordinary files validate parsing/admission only; they do not simulate kernel enforcement.
- Parent flagged a future staging lifecycle need: the helper may need to retain a private mount for copy-back after descendants stop. This slice places helper and descendants in one cancellation cgroup; a nested helper/child cgroup split must be designed with the later staging implementation. No tmpfs or copy-back work is included.

## Symbols and validation handoff

- Main symbols: `CgroupLimits`, `CgroupLease::{create,set_and_verify_limits,kill_wait_and_remove}`, `validate_cgroup_limits`, `validate_delegated_cgroup_root`, `attach_process_to_cgroup`, and `LinuxLoopbackChild::{wait,stop_forwarder}`. `spawn` now requires caller-supplied `cgroup_root` and `cgroup_limits`.
- New focused test symbols: `cgroup_limits_reject_zero_values`, `delegated_cgroup_root_requires_explicit_enabled_controls`, and `nondelegated_system_cgroup_root_fails_closed` in `platform::process_sandbox::linux_smoke::tests`.
- Not run: the coordinator prohibited cargo builds/tests/process launches. No successful cgroup launch, quota enforcement, namespace behavior, or process-tree shutdown is claimed from this host.

- Admission also checks the selected directory filesystem magic for cgroup v2 before controller validation. Fake ordinary directories pass only the file-content validator and are explicitly rejected by the full delegated-root validator.
- Parent-reported compile diagnostic: joining the accept-forwarder thread from a match guard attempted to move a value in a guard (`E0507`). Rewrote that arm as a normal match body with the join check inside; source-reviewed only, with no build or test run.

## Staging follow-up slice

- New bounded assignment: implement only `arcana-core/src/platform/process_sandbox/linux_smoke/staging.rs` and append this report; coordinator owns later integration into `linux_smoke.rs`/`mod.rs`.
- Approved target: private bounded tmpfs-backed staging primitive, with explicit byte/entry caps, nofollow descriptor-relative traversal, snapshot digests/identities, and conflict-safe copy-back only after child-tree emptiness is confirmed. Parent's primitive test establishes tmpfs size/inode exhaustion behavior only, not a production launch.
- Prohibited for this slice: mount or process launch, builds/tests, network, environment mutation, credentials, databases, and Git operations. Coordinator will run exact tests.
- I will implement copy-back only if the local source provides a defensible serialization mechanism against concurrent host edits; otherwise the API will remain a validated plan with an explicit application limitation.
- ADR-LOG-RESTRUCT-0349 approves private size/inode-limited tmpfs staging and conflict-safe copy-back only after the complete child tree stops; it does not itself supply a host-side writer serialization primitive. The existing Linux candidate has no workspace write lock/serialization facility. I will therefore keep host mutation out of this unintegrated primitive and expose conflict-plan generation only unless source review finds a reliable existing serialization boundary.
- Existing local dependencies include `sha2` and Unix `libc`; the assigned module can use those without introducing dependencies. The caller will have to establish a private stage root and confirm the child tree is empty before requesting a staged-tree plan.
- Added `staging.rs` with explicit positive byte/entry caps, nofollow pinned-root opens, fd-relative sorted directory traversal, symlink/special-file/multilink refusal, stable-file checks, SHA-256 baselines, and exclusive stage file creation. The snapshot copies into an already-empty stage root and records root/path identities plus file digests.
- Added stopped-tree plan generation that rescans host and stage, detects host root/path/parent substitution for affected operations, rejects file/directory replacement, and emits checked relative create/write/delete operations with staged digests/sizes. Tests cover normal snapshot/planning, symlink/hardlink refusal, byte/entry limits, concurrent file edits, and parent replacement.
- Copy-back remains intentionally unapplied: source review found no host workspace writer lock or serialization primitive. Per-file rechecks plus atomic rename alone cannot guarantee no concurrent writer between the last check and replacement; the interface is therefore plan-only and must not be described as validated copy-back. No tmpfs mount, child launch, build, or test was performed.
- Coordinator review found directory enumeration allocated/sorted all names before the traversal cap was checked. Updated `directory_names` to require a remaining-entry bound and reject before allocating/pushing the first over-budget name; stage-empty admission probes with a zero-name budget and stops at the first entry. This bounds each enumeration by the explicit remaining entry limit. No build/test was run.
- Coordinator requested mode preservation for executable workspace files. Staged regular files now receive the source's `0o777` ordinary permission bits through `fchmod` after copying, with setuid/setgid and other special bits excluded; added a source fixture with an executable `0o755` script carrying setid bits and assertions that executable mode survives while staged `0o6000` bits are clear. Caller mount must still use `nosuid,nodev`. Not run.



## Coordinator primitive evidence (not production enablement)

Linux cgroup/admission tests pass9/9 (.03s); staged snapshot/conflict-plan tests pass5/5 (.06s), including bounded enumeration and executable-mode/setid filtering. A disposable private USER+MOUNT+NET namespace mounts tmpfs with64KiB/16inodes, observes real ENOSPC for both limits, unmounts and proves the original host directory unchanged. These are admission, staging-plan and kernel primitive checks only: no delegated-cgroup enforcement, production launch, host copy-back or end-to-end smoke is proved. Candidate remains test-only and uncommitted. Await direction for temporary delegated test scope; missing support still fails closed.
