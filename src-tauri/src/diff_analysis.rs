// Diff classification and risk scoring.
//
// MOVED to `arcana-core` in P1-B and re-exported here so existing
// `crate::diff_analysis::…` paths keep resolving. Had zero `tauri::` references, which
// is why it could move first.
pub use arcana_core::diff_analysis::*;
