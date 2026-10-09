# P4-51 registered execution matrix — 2026-10-09

2026-10-09 P4-51 registered execution matrix: actual FastDevelopment Bash, GitCommit and AnalyzeDiff registrations pass through ToolExecutor on Windows/Linux. Bash one-call approval, observed nonzero exit and denial preserve private inputs; Git commits only the named file and refuses a pre-staged sibling without changing HEAD/index; AnalyzeDiff persists deterministic analysis and TasksDb reads it back. Corrected fixture prerequisites use real RunsDb worker runs and reload the separate libgit2 observer's cached index. Windows shell/git2/2 (2.12s), diff1/1 (0.91s); Linux2/2 (2.02s), diff1/1 (1.00s). Registered ProjectCommand passing build/selected timeout/private-envelope/descendant/CLI log consumer passes Windows1/1 (83.02s), Linux1/1 (39.25s). Coverage reaches46/47; confined e2e_probe success and remaining model/provider/compiler error conjunctions keep P4-51/TG4-25 open. Overall41/59 and19/33 unchanged. Evidence: _verify/P4-51-EXECUTION-MATRIX-CONSOLIDATED-2026-10-09.md. ADR0351 retracts the earlier invented-run fixture's handler-specific claim pending its correction; it did prove model result transport. No paid provider run or Calendar activity.

## Commands and observed outcomes

- `cargo test --offline -p arcana-core --lib registered_tool_ --locked -- --test-threads=1`: Windows2/2,2.12s; Linux2/2,2.02s. The existing native singleton test stays ignored and was proved separately in63c1488.
- `cargo test --offline -p arcana-core --lib analyze_diff_tool_executes_repository_diff_without_consent_or_provider_key --locked -- --test-threads=1`: Windows1/1,.91s; Linux1/1,1.00s.
- `cargo test --offline -p arcanactl --test project_command_stage_logs claimed_python_build_stages_survive_jsonl_writer_and_cli_export --locked -- --nocapture`: Windows1/1,83.02s; Linux1/1,39.25s. Actual configured timeout30s remains required; outer safety deadline cannot satisfy the test. Compiler build passes, real task batch and five JSONL stages read through CLI export, timeout stops descendant heartbeat and excludes four planted private input markers.

## Limits

These tests prove real registered executor results and intended producer/reader paths. They do not prove every tool reaches an AgentEngine model consumer or classify every provider timeout/cancellation. ADR0351 corrects the old17-tool fixture's invented-run prerequisite; its correction is a separate in-progress unit. e2e_probe confined success is missing; no full P4-51/TG4-25 closure, readiness rescore, Calendar or live paid run is claimed.

Linux uses its native temporary filesystem as documented byADR0348. Existing compiler warnings remain. Raw test logs are ignored under target/p4-ci-checks; no credentials or generated runtime is committed.

Validation: `cargo check --offline -p arcana-core -p arcanactl --locked` passes (1m14s); changed Rust files pass rustfmt check and staged diff whitespace/secret-pattern scan.
