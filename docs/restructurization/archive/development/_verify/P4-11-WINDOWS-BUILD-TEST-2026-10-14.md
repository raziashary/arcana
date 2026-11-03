# P4-11 Windows build/test command closeout

Date: 2026-10-14. Host: Windows. Scope: the named P4-11 Build/Test task,
not TG4-18's larger Lint/Run corpus.

The checked-in `fx-repo-rust`, `fx-repo-node` and `fx-repo-python` fixtures are
copied to separate isolated selected workspaces. Production
`detect_toolchains` identifies Cargo, npm and Python respectively.
`command::resolve` supplies each Build/Test argv, and
`run_resolved_command` executes it through the actual Windows workspace
sandbox. The test observes all six exit codes as zero; it does not infer
success from argv construction alone. Python Build uses `compileall`; Python
Test is the separate pytest confidence pass. The existing production
FastDevelopment `project_command` registrations and ToolExecutor receipts
establish that workers reach this resolver/launcher path (including the
claimed-task Cargo Test in
`P4-61-PROCESS-WRITE-ADMISSION-2026-10-13.md` and the real
tool-to-Logs readback in `P4-62-COMMAND-LOGS-READER-2026-10-09.md`).

`cargo test -p arcana-core --lib windows_build_and_test_intents_run_for_all_checked_in_ecosystems -- --nocapture`
passed 1/1 in 129.26 seconds after compilation. The test uses an isolated
Arcana profile and project folders; it makes no provider call and reads no
user workspace. The earlier combined corpus is still blocked at Cargo Clippy
config canonicalization before its Node sequence. That remains a real
TG4-18/TG4-31 limitation; this receipt does not bypass or solve it. Full
Windows lint/run containment, locked-dependency staging, the end-to-end
PA/Fleet artifact and Linux/macOS parity remain open under their named owners.
