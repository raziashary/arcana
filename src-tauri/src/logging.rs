//! Desktop-facing compatibility entry point for the shared Arcana logger.
//!
//! The writer and redaction boundaries live in `arcana-core` so the desktop
//! and CLI produce the same daily readable and structured telemetry streams.

pub use arcana_core::observability::logging::{init, init_with_filter, LoggingGuards};
