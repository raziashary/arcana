// Platform-correct command construction (BUG-COMPILE's root-cause fix).
//
// MOVED to `arcana-core` in P1-B and re-exported here so existing
// `crate::platform::…` paths keep resolving. Had zero `tauri::` references, which
// is why it could move first.
pub use arcana_core::platform::*;
