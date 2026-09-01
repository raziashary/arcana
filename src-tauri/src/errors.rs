// Error types — MOVED to `autodev-core` in P1-B.
//
// Re-exported here so the several hundred `crate::errors::…` paths across this
// crate keep resolving. The definition now lives in
// `autodev-core/src/errors.rs`, because `provider/`, `memory/` and
// `diff_analysis/` all depend on it and had to be able to move with it.
//
// This shim is deliberate, not laziness: rewriting every import in the same
// commit as the physical move would make the diff unreviewable and would mix a
// mechanical rename into a structural change.
pub use autodev_core::errors::*;
