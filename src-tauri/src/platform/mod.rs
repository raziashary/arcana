// Platform-specific behaviour, isolated so the rest of the codebase does not
// branch on `cfg!(windows)` inline.
//
// Moves wholesale into `autodev-core/platform/` in P1-05, which is why it is a
// directory module from the start rather than a single file.

pub mod command;
