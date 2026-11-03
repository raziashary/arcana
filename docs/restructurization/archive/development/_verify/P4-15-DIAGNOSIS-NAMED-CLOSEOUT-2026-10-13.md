# P4-15 / TG4-08 named closeout — 2026-10-13

Owner decision ADR0389 closes the observed-build-failure and one-use
Supervisor-approval contract on its own production evidence. Windows is the
P4 acceptance platform under ADR0391; Linux/macOS parity is P9-16/TG9-06,
and live-provider Fleet completion remains P4-50/TG4-24.

The existing task/approval receipts record actual Rust-owned nonzero exits,
three-failure diagnosis, pass reset, refusal of worker self-approval, a
Supervisor-authored task/worker/latest-batch-bound grant, atomic one-use
consumption after DB reopen and a two-consumer race. The shipped Supervisor
recipe runs through RecipeRunner/AgentEngine with a scripted provider. The
mixed-project batch tests exercise the real ProjectCommandTool and QC reader;
prior Windows/Linux results are in
`P4-14-MIXED-LINUX-CLOSEOUT-2026-10-10.md`.

Current Windows regression
`production_mixed_batch_stops_after_third_observed_failure` passed twice
against the compiled core test binary: once via `cargo test -p arcana-core
--lib production_mixed_batch_stops_after_third_observed_failure --locked --
--nocapture` (1/1, 99.21s), and once by running that binary with the
host-selected `RUSTUP_TOOLCHAIN=stable` set for the command (1/1, 106.23s).
The test seeds two observed failures, launches a broken Rust fixture, records
the third observed nonzero exit, and verifies that the later Python project
does not launch. The approved mixed success/QC readback remains in the prior
receipt. Source `project_command.rs` now prints the structured result on a
failed batch assertion; the temporary diagnostic `eprintln!` was removed.

One directly launched test-binary run without `RUSTUP_TOOLCHAIN` failed before
the Cargo child. The isolated profile redirects `USERPROFILE`, and Rustup
reported that no default toolchain was configured there. The tool correctly
classified this as `command_spawn_failed` and persisted an incomplete batch;
it did **not** count a fabricated nonzero exit. This is a test-launch
environment condition, not a passing diagnosis run. On this host,
`rustup show active-toolchain` reports `stable-x86_64-pc-windows-msvc`.
Reproduce the direct-binary fixture with that explicit toolchain, or invoke
it through Cargo's selected toolchain environment. The test should remain
open to future fixture hardening if a CI host does not provide an explicit
selection.

No paid provider, owner Calendar run, user database or server was used.
