# P4-62 selected production timeout — 2026-10-09

The prior descendant test used a test-owned timeout around the launcher. This fixture selects timeout_secs=30 through the actual FastDevelopment project_command registration and ToolExecutor. pytest spawns a real heartbeat-writing descendant and waits for it. Assertions require operation_timed_out/category timeout from the production tool, a started child and nonzero heartbeat, no further writes after return, and no late marker. A 120-second outer bound is only fixture safety and cannot satisfy the production error assertions.

The same fixture first performs a passing claimed Python build and reads the complete passing TasksDb batch. It then reads the configured JSONL through the shipped CLI logs export: all five successful build stages survive, and the timed-out call has an incomplete process_wait plus timeout result with the same derived private telemetry identity. No raw compound ID is exported. Existing missing-claim/wrong-worker negative controls remain unchanged.

## Verification

- Windows1/1 passes in115.83s; selected command timeout observed at30041ms.
- Linux1/1 passes in34.09s; selected command timeout observed at30008ms.
- Core/CLI cargo check, workspace format and diff checks pass. No provider call or user database is involved. No server listener starts; fixture child cleanup is asserted before owned workspace removal.

Logs: target/p4-ci-checks/project-command-selected-timeout-windows-2026-10-09.log and project-command-stage-logs-linux-2026-10-09.log. Times are whole-fixture measurements, not comparable command performance benchmarks.

## Linux test setup and filesystem limit

Owner ADR0347 approves matching Ubuntu python3.10-venv plus pytest9.1.1 only in /home/razi/arcana-p4-python-tools. Apt installed the venv package and its necessary Python3.10 security patch dependencies; no general distribution upgrade or global pip installation was performed. Exact setup logs are install-linux-venv-2026-10-09.log and the isolated prepare script under target/p4-ci-checks.

After setup, the underlying sandbox launcher comparison passes on native /tmp but fails on WSL DrvFS /mnt/c when pytest truncates its already-unlinked temporary capture file (ENOENT). ADR0348 uses native temporary storage for Linux while retaining the repository-local parent needed by Windows ACL fixtures. Sandbox policy and timeout assertions are unchanged. This does not prove DrvFS compatibility; the limitation remains documented. The temporary diagnostic test was removed after comparison.

P4-62/TG4-32 remain open for finite CLI shutdown, matched platform measurements and macOS execution. No full phase or Calendar acceptance is claimed.
