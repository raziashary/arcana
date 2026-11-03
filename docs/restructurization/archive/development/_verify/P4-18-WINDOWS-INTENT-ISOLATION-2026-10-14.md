# TG4-18 Windows intent isolation

Date: 2026-10-14. Host: Windows. Scope: selected production command
resolver/launcher intents, not full TG4-18 closure.

The P4-11 Build/Test receipt observes six passing sandboxed exits for the
checked-in Rust, npm/Node and Python projects. A separate focused test copies
the checked-in Rust and npm/Node fixtures into isolated selected workspaces
and observes passing Rust Run, npm Lint and npm Run exits through
`command::resolve` and `run_resolved_command`. The npm cases traverse the
Windows `.cmd` shim. No provider key, user workspace, or live network service
is used.

`cargo test -p arcana-core --lib windows_rust_run_and_node_lint_run_use_production_sandbox -- --nocapture`
passed 1/1 in 22.42 seconds after compilation.

The current exact Clippy case was also rerun:
`cargo test -p arcana-core --lib production_clippy_environment_is_visible_and_readable_in_workspace_sandbox -- --nocapture`
failed 0/1 in 16.21 seconds. Cargo reports `error finding Clippy's
configuration file: Access is denied. (os error 5)` and exits 101. The
selected workspace config is readable, but the sandboxed Clippy process
cannot canonicalize its path. This is a real Windows command-path failure,
so TG4-18 and TG4-31 remain open. No permission or sandbox grant was widened.
