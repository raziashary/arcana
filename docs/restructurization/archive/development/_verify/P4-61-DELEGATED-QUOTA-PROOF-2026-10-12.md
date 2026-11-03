# P4-61 delegated Linux quota proof — 2026-10-12

## Scope and authorization

- Authorized temporary WSL/systemd test scope: `arcana-p4-quota-20261012-*`; no persistent machine configuration.
- Owned files: `arcana-core/src/platform/process_sandbox/linux_smoke.rs`, ignored `target/p4-ci-checks/` quota runner scripts, and this report. `linux_smoke/staging.rs`, `mod.rs`, `linux.rs`, trackers, ADRs, and status checkboxes remain parent-owned.
- The target is actual delegated cgroup-v2 tree membership and aggregate CPU, memory, and process enforcement, with observed kernel controls/counters and proven owned-scope teardown. Admission tests over ordinary files are not quota proof.
- Execution constraints: use only exact `linux_smoke` test filters, cargo target `/home/razi/arcana-p4-linux-target`, and a bounded explicitly named systemd unit. No persistent config, packages, system services, shared resource resets, live credentials/data, providers, network, Git writes, or production smoke.
- Stop immediately if the WSL launcher cannot create the approved delegated unit. Do not retry setup loops or silently fall back to per-process quotas.

## Initial evidence

- Existing source defines `CgroupLease` with an aggregate parent and `helper`/`work` child groups; it reads back aggregate `cpu.max`, `memory.max`, `pids.max`, and `memory.swap.max`, and provides `kill_work_and_wait` plus parent teardown. Existing fake-file tests do not exercise kernel cgroup semantics.
- Prior lifecycle and staged quota reports remain explicit that actual cgroup attach/enforcement is unproved. Their precise findings and current plan contract will be carried forward after reading the governing TG4-31 text.
- WSL inventory reports Ubuntu stopped and version 2; no quota unit or process has been created yet.

## Progress

- [x] Create this report before quota-test changes or runtime setup.
- [x] Inspect exact TG4-31 contract and existing exact test hooks.
- [x] Add narrowly scoped actual-cgroup test(s) and ignored LF Bash runner(s), without changing production capability gates.
- [x] Run only exact approved filters in named temporary units; capture controls, counters/events, and test output.
- [x] Prove complete owned unit/cgroup teardown and absence of retained processes/listeners.
- [x] Record source/test evidence, limitations, and handoff to parent for integration/status decisions.

## Evidence log

- No WSL launch, test, build, or owned privileged unit has been started at report creation.

- TG4-31 / P4-61 plan text was inspected in `MASTER_DEVELOPMENT_PLAN.md` under P4-61. It requires positive workspace/toolchain execution, external-file and local-network denial, approval/staging behavior, and production launcher corpus across Windows/Linux/macOS; Linux/macOS remain open. This bounded experiment covers only actual Linux cgroup-v2 enforcement/lifecycle and cannot close TG4-31 or P4-61.
- Existing authorized exact module-test filter will be extended with one kernel-only test, gated on the runner-provided `ARCANA_CGROUP_TEST_ROOT`; it will not run against or discover `/sys/fs/cgroup` by default.

## Initial invalid invocation (superseded)

- The initial WSL command used unit name `arcana-p4-quota-20261012-proof` with `systemd-run --scope --wait`; local help rejected it before unit creation with exact error: `--wait may not be combined with --scope.`
- At that point no quota test, kernel control, process placement, or counter observation had occurred. This was a launcher syntax mistake, not a delegation failure. The coordinator then authorized one corrected transient service attempt, recorded below.
- Ubuntu was briefly terminated after the syntax error. The coordinator later instructed not to terminate this shared WSL distro; it has not been terminated again.

## Status after first direct run (superseded by final source-matched evidence below)

- [x] Source/API and governing-plan inspection; report created before runtime setup.
- [x] Exact-filter test binary compiled with the already installed stable toolchain.
- [x] One direct exact-filter runtime attempt under the owned transient delegated service.
- [x] Owned service, candidate cgroup, and child process cleanup verified.
- [ ] Kernel CPU, memory, and pids quota enforcement: not reached because first child attachment failed with `EINVAL`.
- [ ] Parent integration, contract, and tracker decisions remain outstanding.

## Coordinator-corrected launch and kernel attach evidence

- **Retraction of initial launcher interpretation:** local `systemd-run --help` shows `--scope` and `--wait` are separate modes; the original `--scope --wait` command was rejected with `--wait may not be combined with --scope.` No unit had been attempted by that syntax. This was an invocation mistake, not evidence that WSL delegation was unavailable. The coordinator authorized one corrected transient service attempt. Ubuntu was briefly terminated after that first syntax error; I did not terminate it again, and it remains available to the other agents.
- The exact-filter precompile first inherited root's rustup home and failed with `no default toolchain configured`. Inspection found an already-installed stable toolchain at `/home/razi/.rustup/toolchains/stable-x86_64-unknown-linux-gnu`; setting `RUSTUP_HOME=/home/razi/.rustup` and `CARGO_HOME=/home/razi/.cargo` for the command only allowed the exact `--no-run` compile to pass. No toolchain was installed and no rustup default was changed. Command: `cargo test --offline -p arcana-core --lib platform::process_sandbox::linux_smoke::tests::delegated_cgroup_tree_enforces_aggregate_quotas_and_tears_down --locked --no-run`; exit 0 after 11m47s. Artifact: `/home/razi/arcana-p4-linux-target/debug/deps/arcana_core-51b3ee6f8201a22e`, SHA-256 `48c091955a7ccabc19749a31b6e7970a1a3ae615073c2db6a3fa66edac9758ca`.
- A corrected transient service `arcana-p4-quota-20261012-proof.service` did start with delegation, but its Cargo test command waited on the shared build-directory lock while another coordinator build ran outside that service. No quota test or cgroup was created. I stopped only that exact unit; afterward systemd reported `LoadState=not-found`, `ActiveState=inactive`, `ControlGroup=` and its runner/Cargo PIDs were absent. I then changed the runner to execute the precompiled exact-filter binary directly, avoiding Cargo inside the quota service.
- The direct attempt ran as transient service `arcana-p4-quota-20261012-direct-proof.service` with `Delegate=yes`, `CPUQuota=200%`, `MemoryMax=1073741824`, `TasksMax=256`, `KillMode=control-group`, `TimeoutStopSec=10s`, and `RuntimeMaxSec=90s`. Before the test, the service cgroup root had an empty `cgroup.procs`, `cgroup.subtree_control` read `cpu memory pids`, and the runner occupied the sibling `runner` child, so the delegated root had no internal process.
- The exact test failed on its first attempt to write the child PID to `work/cgroup.procs`: `arcana-core/src/platform/process_sandbox/linux_smoke.rs:2059:41`, OS error 22, `Invalid argument`. Test result was `0 passed; 1 failed`, elapsed `0.11s`; the captured log is ignored at `target/p4-ci-checks/p4-61-delegated-quota-2026-10-12.log`. CPU throttling, memory OOM events, denied-fork counters, and lease teardown were not reached, so no quota enforcement is proved.
- Owned-service cleanup succeeded: `systemctl show arcana-p4-quota-20261012-direct-proof.service` reported `LoadState=not-found`, `ActiveState=inactive`, `SubState=dead`, empty `ControlGroup`; `/sys/fs/cgroup/system.slice/arcana-p4-quota-20261012-direct-proof.service` did not exist. The runner also found zero `arcana-linux-smoke-*` children before returning.
- **Status at that point:** WSL delegated controller setup was proven for the temporary unit, but moving a spawned child into the candidate's work cgroup failed with `EINVAL`. A subsequent source-grounded test-fixture correction and separately authorized bounded launches are recorded below. P4-61/TG4-31 remain open.
## Aggregate-counter source correction and live CPU diagnostic

- **Retraction:** the compiled exact-filter artifact used during the first post-attach-fix run was stale relative to the source now on disk. I incorrectly described its zero `cpu.stat` values as an aggregate-parent failure. The current source reads `cpu_lease.path/cpu.stat`, `memory_lease.path/memory.events`, and `pids_lease.path/pids.events`; those are the nodes where `CgroupLease::configure_and_verify` writes `cpu.max`, `memory.max`, and `pids.max`. The old binary instead read the CPU work leaf, whose `cpu.max` is `max 100000` and whose throttle counters remain zero.
- A single bounded diagnostic rerun used the existing exact-filter binary SHA-256 `346820f8b3b5724f9dfdbeda3ba4e2ba1f081835169a1deb8afb1f5ca3760265` under transient service `arcana-p4-quota-20261012-cpu-diagnostic.service` (`Delegate=yes`, `CPUQuota=200%`, `MemoryMax=1073741824`, `TasksMax=256`, `KillMode=control-group`, `RuntimeMaxSec=20s`). The runner sampled only while that exact test was alive; it did not invoke memory/pids probes because the stale binary failed its CPU assertion first.
- Journal evidence showed aggregate cgroup `cgroup.type=domain`, `cpu.max=10000 100000`, empty `cgroup.procs`, and child `work` `cpu.max=max 100000`. Probe PID 458 was listed in `work/cgroup.procs`; `/proc/458/cgroup` identified `/system.slice/arcana-p4-quota-20261012-cpu-diagnostic.service/arcana-linux-smoke-446-1/work`. During that same live CPU probe, aggregate `cpu.stat` samples rose from `usage_usec=44017, nr_throttled=3, throttled_usec=216137` to `usage_usec=89161, nr_throttled=7, throttled_usec=578910`. This is direct kernel evidence that the configured aggregate CPU quota throttled the attached work process.
- The stale test binary itself still exited 101 after reporting leaf counters `nr_periods=0, nr_throttled=0, throttled_usec=0, usage_usec=104412`; this is not used as evidence against aggregate enforcement. Its test result was `0 passed; 1 failed`, elapsed 1.60s. Diagnostic journal: `journalctl -u arcana-p4-quota-20261012-cpu-diagnostic.service --no-pager -o cat`.
- Cleanup after that diagnostic: systemd reported `LoadState=not-found`, `ActiveState=inactive`, `SubState=dead`, empty `ControlGroup`; the exact unit cgroup path was absent and the runner found `OWNED_CGROUP_LEFTOVERS=0`. No WSL distro termination or persistent configuration occurred.
- Current-source exact-filter `--no-run` compilation with `/home/razi/arcana-p4-linux-target` completed successfully; source-matched runtime evidence follows.

## Final source-matched kernel proof — 2026-10-12

- The final exact-filter no-run command exited 0 after 7m26s: `cargo test --offline -p arcana-core --lib platform::process_sandbox::linux_smoke::tests::delegated_cgroup_tree_enforces_aggregate_quotas_and_tears_down --locked --no-run`. It emitted 87 existing warnings. The direct test binary was `/home/razi/arcana-p4-linux-target/debug/deps/arcana_core-51b3ee6f8201a22e`, SHA-256 `5f10756c70511a711d90186e17b893f9f2ae4bed3214607beabfbc65191116a8`.
- The exact test ran directly (no Cargo or compiler inside the quota unit) as `arcana-p4-quota-20261012-counters-proof.service`, with `Delegate=yes`, `CPUQuota=200%`, `MemoryMax=1073741824`, `TasksMax=256`, `KillMode=control-group`, `TimeoutStopSec=10s`, and `RuntimeMaxSec=90s`. The systemd service ran 12.595s. The delegated root exposed `cpuset cpu io memory pids`, enabled `cpu memory pids`, and held the runner in a sibling `runner` child while its root `cgroup.procs` was empty.
- Exact command inside the service: `/home/razi/arcana-p4-linux-target/debug/deps/arcana_core-51b3ee6f8201a22e platform::process_sandbox::linux_smoke::tests::delegated_cgroup_tree_enforces_aggregate_quotas_and_tears_down --exact --test-threads=1`. Result: `1 passed; 0 failed; 1094 filtered out`, 6.37s; runner exit 0.
- CPU tree `arcana-linux-smoke-474-1`: aggregate `cgroup.type=domain`, `cpu.max=10000 100000`, empty parent `cgroup.procs`; work child `cpu.max=max 100000` held PID 477 and `/proc/477/cgroup` confirmed its `/work` membership. Parent `cpu.stat` advanced while the process ran (e.g. `nr_throttled=0` to `2`, then `8`; `throttled_usec` reached `624751`). The exact test subsequently asserted both `nr_throttled` and `throttled_usec` increased versus the pre-probe parent counters.
- Memory tree `arcana-linux-smoke-474-2`: aggregate `memory.max=50331648` (48 MiB); live `memory.current` reached 50,331,648 before the kernel OOM event. Aggregate `memory.events` recorded `oom=1` and `oom_kill=1`; the exact test asserted `oom_kill` incremented and the 160 MiB probe exited unsuccessfully before its five-second deadline. The per-service cap remained 1 GiB.
- Pids tree `arcana-linux-smoke-474-3`: aggregate `pids.max=4`, `pids.current=4`, and `pids.events max=1`; the Python parent and three forked children were all observed in the work leaf. The exact test asserted the parent `max` event incremented, then used `cgroup.kill`, waited for `populated=0`, reaped the probe parent, and removed the tree.
- All three leases called `kill_wait_and_remove`, which sends `cgroup.kill`, waits for parent `cgroup.events populated=0`, removes work/helper children, disables descendant controllers, then removes the aggregate parent. Test success proves these operations completed; runner also reported `OWNED_CGROUP_LEFTOVERS=0`.
- Unit cleanup verification: `systemctl show arcana-p4-quota-20261012-counters-proof.service` reported `LoadState=not-found`, `ActiveState=inactive`, `SubState=dead`, empty `ControlGroup`; `/sys/fs/cgroup/system.slice/arcana-p4-quota-20261012-counters-proof.service` was absent. No candidate cgroups or probe processes remained. No distro termination, persistent configuration, package install, live credentials, or production smoke was used.
- Scoped source correction: the test now writes each child PID and newline in one write to `cgroup.procs`, matching production attach behavior and resolving the `EINVAL` fixture failure. Counter reads are at the aggregate parent where limits are configured. Production smoke registration, production capability conjunctions, `linux_smoke/staging.rs`, `mod.rs`, `linux.rs`, trackers, and ADRs were not edited by this task.
- Ignored runners and full logs are under `target/p4-ci-checks/`; final compile log is `p4-61-delegated-quota-compile-counters-2026-10-12.log`, final direct test log is `p4-61-delegated-quota-counters-2026-10-12.log`, and live sampler/service evidence is in the systemd journal for `arcana-p4-quota-20261012-counters-proof.service`.
- **Final delegated result:** actual WSL kernel cgroup-v2 aggregate CPU throttling, memory OOM enforcement, pids denial, and owned-tree teardown are proved for this bounded test. P4-61 and TG4-31 remain open: this single Linux cgroup proof does not complete workspace/toolchain, file/network denial, approval/staging, production launcher corpus, or macOS evidence. Parent owns integration and contract/status decisions.
