// Observability module — Doc 08 Phase 2.
//
// Centralises log-error taxonomy and (P3) log retention policy.

pub mod error_kinds;
pub mod log_retention;
// Doc 07 Phase 3 — startup-time scan for worker-generated stray files
// in the AutoDev repo. Emits warns + a Tauri event the banner consumes.
pub mod stray_files;
