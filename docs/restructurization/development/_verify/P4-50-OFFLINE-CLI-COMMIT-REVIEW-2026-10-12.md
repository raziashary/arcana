# P4-50 Offline CLI Commit Review — 2026-10-12

Review scope: offline Fleet feature wiring, runtime marker/provider seam, actual CLI command, scripted provider, integration harness, and P4-50 evidence note.

Sequence note: before receiving the instruction to create this report first, I had already inspected the scoped git status/diff and read the P4-50 evidence note. No files were changed until this report was created.

## Findings

Review in progress.
### P2 — The disposable-profile gate follows `.arcana` symlinks outside the temp profile

In `arcana-core/src/runtime.rs`, `offline_fleet_test_scenario` canonicalizes the profile home and checks it is under the temp directory, but then joins `.arcana/offline-fleet-test.marker` and reads it without canonicalizing or validating that `.arcana` and the marker remain inside that profile (around lines 262–272). A profile-shaped temp directory whose `.arcana` is a symlink to an existing Arcana data directory can therefore pass the marker check if the marker is present there; the feature can then select the scripted provider while normal runtime database paths follow that same symlink. This defeats the gate’s disposable-data guarantee and can redirect the CLI’s board/log/profile writes into non-disposable data. Canonicalize and containment-check the `.arcana` directory/marker (and reject symlinks) before activating the feature.

### Proof gap — CLI subprocess invocations have no outer timeout

`src-cli/tests/fleet_offline.rs` uses blocking `Command::output()` for the Fleet subprocess in `IsolatedProfile::run_fleet` (around lines 61–81), with no parent-side process deadline. The deadline scenario verifies the CLI’s own poll deadline and its subsequent scheduler shutdown, but if either path hangs, this test target hangs indefinitely and cannot prove a bounded subprocess return. Add a bounded child-process wait/kill path if the harness is meant to prove bounded return under shutdown failure.

Review completed read-only. The allowlist remains default-off through Cargo feature forwarding, CLI workspace egress consent is still required, and recipe/subagent provider paths retain the existing capped-provider, workspace-consent, and run-budget wrappers. No tests or builds were run for this review; the reported subprocess bound is a harness proof gap, and the prior 4/4 evidence remains as recorded in the P4-50 verification note.

## Targeted recheck of reported findings

The two original Windows findings are resolved in current source:

- `arcana-core/src/runtime.rs:275-292` now uses `symlink_metadata` on both `.arcana` and the marker, rejects Windows reparse points (and symlinks on Unix), and requires the expected directory/file types before reading the marker. `src-cli/tests/fleet_offline.rs:705-755` adds a redirected-data-directory case that provides a valid marker through the redirect and asserts the victim board is untouched.
- `src-cli/tests/fleet_offline.rs:63-125` now owns the Fleet child process, polls an outer `timeout_secs + 30s` deadline, kills and reaps on expiry, and reads captured output with a 4 MiB bound. Windows uses `taskkill /T /F` to stop the process tree. The integration target is feature-gated at the crate root (`src-cli/tests/fleet_offline.rs:1`).

Remaining limit: on Unix the timeout branch calls `child.kill()` for the parent only; descendant termination is not established, so no Unix process-tree cleanup claim is made. The parent reports the credential-free Windows `fleet_offline` suite passed 5/5 in 22.54s; I did not rerun it under this read-only recheck.
