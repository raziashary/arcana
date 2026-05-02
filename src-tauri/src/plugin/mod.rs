// Plugin system module root.
// Exposes the PluginManager and all manifest types to the rest of the crate.

pub mod manifest;
pub mod builtins;
pub mod registry;
pub mod runner;
pub mod token;
pub mod manager;

pub use manager::PluginManager;
pub use manifest::{
    DependencyCheck, InstalledPlugin, InstallResult, PluginCommands, PluginConfig,
    PluginConfigRef, PluginDependency, PluginInfo, PluginManifest, PluginOsCommands,
    PluginType, RunResult, SetupToken, TokenClaims, UpdateResult, VerifyResult,
};
