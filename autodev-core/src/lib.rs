// autodev-core — AutoDev's agent runtime, providers, memory and platform layer.
//
// This crate is shared by the desktop app (`src-tauri`) and the CLI
// (`src-cli`), and it must stay free of any UI framework. Nothing here may
// depend on `tauri`.
//
// That constraint is the point of the crate, not an incidental property: before
// P1 the agent runtime could not be reused by the CLI because every event it
// emitted required a `tauri::AppHandle`. `events::EventSink` is the boundary
// that removes that requirement — core depends on the trait, the host injects
// an implementation.
//
// See ADR-LOG-RESTRUCT-0011.

pub mod agent;
pub mod analytics;
pub mod budget;
pub mod cost;
pub mod db;
pub mod diff_analysis;
pub mod errors;
pub mod events;
pub mod git;
pub mod graph;
pub mod hooks;
pub mod ide;
pub mod memory;
pub mod observability;
pub mod orphan;
pub mod plan;
pub mod platform;
pub mod plugin;
pub mod provider;
pub mod recipes;
pub mod routing;
pub mod session;
pub mod state;
pub mod subagent;
pub mod vault;
