// Platform-specific behaviour, isolated so the rest of the codebase does not
// branch on `cfg!(windows)` inline.
//
// Moves wholesale into `arcana-core/platform/` in P1-05, which is why it is a
// directory module from the start rather than a single file.

pub mod command;
#[cfg(windows)]
pub mod file_identity;
pub mod process_sandbox;
#[cfg(windows)]
pub mod python_runtime;
pub mod shell_portability;
pub mod workspace;
pub mod workspace_fingerprint;
