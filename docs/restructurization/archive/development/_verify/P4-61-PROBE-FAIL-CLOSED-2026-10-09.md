# P4-61 smoke launch fail-closed integration — 2026-10-09

Status: in progress. Production e2e_probe has an unconfined model-command launcher. Owner-approved ADR0339/0344/0349 prohibit enabling an unproved capability. ADR0350 removes this unsafe route; this does not complete the loopback runner.

## Verified unit

2026-10-09 P4-61 smoke enforcement (ADR0350): removed e2e_probe's unrestricted model-command shell and arbitrary loopback HTTP launcher. The live registration/schema remain, but valid requests now return a bounded correlated sandbox_violation (loopback_smoke, deny, retryable=false) before process/HTTP activity until a proved confined backend exists. URL admission performs no DNS; fields/descriptions report the unavailable capability. Windows10/10 (.79s), Linux10/10 (.66s); the registered valid-call refusal proves no workspace marker, no connection to an owned loopback listener, private-input omission and cleaned port. Independent Luna High review's two stale schema/redirect claims are corrected. This removes a bypass but does not provide positive smoke execution or close P4-51/P4-61/TG4-25/TG4-31. Evidence: _verify/P4-61-PROBE-FAIL-CLOSED-2026-10-09.md.

Exact command: `cargo test --offline -p arcana-core --lib e2e_probe --locked -- --test-threads=1`. The10 tests include actual FastDevelopment registration through ToolExecutor, real RunsDb run, valid model-authored command and localhost URL. The observable result is denial, not a fabricated smoke success. Existing planner readback/invalid probe and URL/workspace guards remain tested. Obsolete unconfined launcher/report/capture tests are removed with their production implementation. `cargo check --offline -p arcana-core -p arcanactl --locked` passes; changed file rustfmt and diff checks pass. No provider key or user DB is used. Owned listener is dropped and rebound to prove cleanup.

## Remaining boundary

Linux candidate admission9/9 passes; its full runner/staging/copy-back is unintegrated. A separate trusted USER+MOUNT+NET namespace experiment mounts tmpfs with size65536/nr_inodes16, observes real ENOSPC from both allocation and inode creation, unmounts and verifies the original directory's baseline unchanged. These are primitive tests, not a production smoke launch or aggregate cgroup proof. Parent requested direction for a temporary systemd delegated test scope and a test-build CLI seam; no dependent actions may proceed before answers.

Sources for the approved primitives: https://cdn.kernel.org/doc/html/latest/filesystems/tmpfs.html and https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html .
