// Plugin system module root.
// Exposes the PluginManager and all manifest types to the rest of the crate.

pub mod builtins;
pub mod manager;
pub mod manifest;
pub mod registry;
pub mod runner;
pub mod token;

pub use manager::PluginManager;
pub use manifest::{
    DependencyCheck, InstallResult, InstalledPlugin, PluginCommands, PluginConfig, PluginConfigRef,
    PluginDependency, PluginInfo, PluginManifest, PluginOsCommands, PluginType, RunResult,
    SetupToken, TokenClaims, UpdateResult, VerifyResult,
};
