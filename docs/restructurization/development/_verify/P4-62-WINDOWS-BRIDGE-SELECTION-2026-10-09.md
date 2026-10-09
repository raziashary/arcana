# P4-62 Windows test-host bridge selection — 2026-10-09

The production launcher recognized core/desktop unit-test basenames but selected a Cargo integration harness as the sandbox bridge. A harness cannot dispatch Arcana's private bridge flag. `is_cargo_harness_executable` now recognizes the existing `deps/<name>-<16 hex characters>` layout, preserving ordinary debug/installed host paths. No argv, ACL, filesystem, network or sandbox policy changed.

The positive/negative layout regression passes on Windows (1/1). `cargo check -p arcana-core --locked` passes. The actual claimed Python integration now completes its build and persists a complete passing batch, then fails the independent JSONL correlation assertion (0/5 stages rather than5). That reader assertion remains unchanged; this commit does not claim P4-62/TG4-32 closure.

Safe logs: target/p4-ci-checks/bridge-selection-windows-2026-10-09.log, bridge-core-check-windows-2026-10-09.log, project-command-stage-logs-fixed-windows-2026-10-09.log. The isolated test cleaned its profile/workspace; no provider, live database or server was used. The newer executor-based log-reader test remains uncommitted pending the owner correlation decision.
