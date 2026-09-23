// Tauri IPC commands for the plugin / marketplace system.
// All commands return Result<T, ArcanaError> so the frontend gets typed errors.

use crate::errors::ArcanaError;
use crate::plugin::{
    manager::PluginManager,
    manifest::{
        InstallResult, InstalledPlugin, PluginConfig, PluginInfo, RunResult, SetupToken,
        TokenClaims, UpdateResult, VerifyResult,
    },
};

// ─── Marketplace ─────────────────────────────────────────────────────────────

/// List all marketplace entries (built-ins + user-installed).
#[tauri::command]
pub async fn list_plugins() -> Result<Vec<PluginInfo>, ArcanaError> {
    let mgr = PluginManager::new();
    Ok(mgr.list_marketplace())
}

/// List currently installed plugins.
#[tauri::command]
pub async fn list_installed_plugins() -> Result<Vec<InstalledPlugin>, ArcanaError> {
    let mgr = PluginManager::new();
    Ok(mgr.list_installed())
}

/// Get the full manifest for a single plugin by name.
#[tauri::command]
pub async fn get_plugin(
    name: String,
) -> Result<Option<crate::plugin::manifest::PluginManifest>, ArcanaError> {
    let mgr = PluginManager::new();
    Ok(mgr.get_plugin(&name))
}

// ─── Install / Uninstall ─────────────────────────────────────────────────────

/// Install a plugin by name from the built-in catalogue.
#[tauri::command]
pub async fn install_plugin(name: String) -> Result<InstallResult, ArcanaError> {
    let mgr = PluginManager::new();
    mgr.install(&name).map_err(ArcanaError::from)
}

/// Install a plugin from a raw YAML string (user-supplied manifest).
#[tauri::command]
pub async fn install_plugin_from_yaml(yaml: String) -> Result<InstallResult, ArcanaError> {
    let mgr = PluginManager::new();
    mgr.install_from_yaml(&yaml).map_err(ArcanaError::from)
}

/// Uninstall a plugin by name.
#[tauri::command]
pub async fn uninstall_plugin(name: String) -> Result<bool, ArcanaError> {
    let mgr = PluginManager::new();
    mgr.uninstall(&name).map_err(ArcanaError::from)
}

// ─── Update ──────────────────────────────────────────────────────────────────

/// Update an installed plugin to the latest available version.
#[tauri::command]
pub async fn update_plugin(name: String) -> Result<UpdateResult, ArcanaError> {
    let mgr = PluginManager::new();
    mgr.update(&name).map_err(ArcanaError::from)
}

// ─── Verify ──────────────────────────────────────────────────────────────────

/// Run dependency checks for an installed plugin.
#[tauri::command]
pub async fn verify_plugin(name: String) -> Result<VerifyResult, ArcanaError> {
    let mgr = PluginManager::new();
    mgr.verify(&name).map_err(ArcanaError::from)
}

// ─── Run ─────────────────────────────────────────────────────────────────────

/// Execute a plugin's `run` command with optional arguments.
#[tauri::command]
pub async fn run_plugin(name: String, args: Vec<String>) -> Result<RunResult, ArcanaError> {
    let mgr = PluginManager::new();
    mgr.run(&name, &args).map_err(ArcanaError::from)
}

// ─── Config ──────────────────────────────────────────────────────────────────

/// Get the persisted config for an installed plugin.
#[tauri::command]
pub async fn get_plugin_config(name: String) -> Result<Option<serde_json::Value>, ArcanaError> {
    let mgr = PluginManager::new();
    mgr.get_config(&name).map_err(ArcanaError::from)
}

/// Set / update the config for an installed plugin.
#[tauri::command]
pub async fn set_plugin_config(
    name: String,
    values: serde_json::Value,
) -> Result<(), ArcanaError> {
    let mgr = PluginManager::new();
    let cfg = PluginConfig { name, values };
    mgr.set_config(&cfg).map_err(ArcanaError::from)
}

// ─── Setup tokens ─────────────────────────────────────────────────────────────

/// Generate a one-click worker setup token.
#[tauri::command]
pub async fn generate_setup_token(
    fleet_id: String,
    worker_type: String,
    repos: Vec<String>,
    ttl_hours: Option<u64>,
) -> Result<SetupToken, ArcanaError> {
    let mgr = PluginManager::new();
    let ttl = ttl_hours.unwrap_or(24);
    mgr.generate_setup_token(&fleet_id, &worker_type, &repos, ttl)
        .map_err(ArcanaError::from)
}

/// Validate a setup token and return its decoded claims.
#[tauri::command]
pub async fn validate_setup_token(token: String) -> Result<TokenClaims, ArcanaError> {
    let mgr = PluginManager::new();
    mgr.validate_setup_token(&token).map_err(ArcanaError::from)
}
