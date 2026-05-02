// Tauri IPC commands for the plugin / marketplace system.
// All commands return Result<T, AutoDevError> so the frontend gets typed errors.

use crate::errors::AutoDevError;
use crate::plugin::{
    manager::PluginManager,
    manifest::{
        InstalledPlugin, InstallResult, PluginConfig, PluginInfo,
        RunResult, SetupToken, TokenClaims, UpdateResult, VerifyResult,
    },
};

// ─── Marketplace ─────────────────────────────────────────────────────────────

/// List all marketplace entries (built-ins + user-installed).
#[tauri::command]
pub async fn list_plugins() -> Result<Vec<PluginInfo>, AutoDevError> {
    let mgr = PluginManager::new();
    Ok(mgr.list_marketplace())
}

/// List currently installed plugins.
#[tauri::command]
pub async fn list_installed_plugins() -> Result<Vec<InstalledPlugin>, AutoDevError> {
    let mgr = PluginManager::new();
    Ok(mgr.list_installed())
}

/// Get the full manifest for a single plugin by name.
#[tauri::command]
pub async fn get_plugin(name: String) -> Result<Option<crate::plugin::manifest::PluginManifest>, AutoDevError> {
    let mgr = PluginManager::new();
    Ok(mgr.get_plugin(&name))
}

// ─── Install / Uninstall ─────────────────────────────────────────────────────

/// Install a plugin by name from the built-in catalogue.
#[tauri::command]
pub async fn install_plugin(name: String) -> Result<InstallResult, AutoDevError> {
    let mgr = PluginManager::new();
    mgr.install(&name).map_err(AutoDevError::from)
}

/// Install a plugin from a raw YAML string (user-supplied manifest).
#[tauri::command]
pub async fn install_plugin_from_yaml(yaml: String) -> Result<InstallResult, AutoDevError> {
    let mgr = PluginManager::new();
    mgr.install_from_yaml(&yaml).map_err(AutoDevError::from)
}

/// Uninstall a plugin by name.
#[tauri::command]
pub async fn uninstall_plugin(name: String) -> Result<bool, AutoDevError> {
    let mgr = PluginManager::new();
    mgr.uninstall(&name).map_err(AutoDevError::from)
}

// ─── Update ──────────────────────────────────────────────────────────────────

/// Update an installed plugin to the latest available version.
#[tauri::command]
pub async fn update_plugin(name: String) -> Result<UpdateResult, AutoDevError> {
    let mgr = PluginManager::new();
    mgr.update(&name).map_err(AutoDevError::from)
}

// ─── Verify ──────────────────────────────────────────────────────────────────

/// Run dependency checks for an installed plugin.
#[tauri::command]
pub async fn verify_plugin(name: String) -> Result<VerifyResult, AutoDevError> {
    let mgr = PluginManager::new();
    mgr.verify(&name).map_err(AutoDevError::from)
}

// ─── Run ─────────────────────────────────────────────────────────────────────

/// Execute a plugin's `run` command with optional arguments.
#[tauri::command]
pub async fn run_plugin(name: String, args: Vec<String>) -> Result<RunResult, AutoDevError> {
    let mgr = PluginManager::new();
    mgr.run(&name, &args).map_err(AutoDevError::from)
}

// ─── Config ──────────────────────────────────────────────────────────────────

/// Get the persisted config for an installed plugin.
#[tauri::command]
pub async fn get_plugin_config(name: String) -> Result<Option<serde_json::Value>, AutoDevError> {
    let mgr = PluginManager::new();
    mgr.get_config(&name).map_err(AutoDevError::from)
}

/// Set / update the config for an installed plugin.
#[tauri::command]
pub async fn set_plugin_config(name: String, values: serde_json::Value) -> Result<(), AutoDevError> {
    let mgr = PluginManager::new();
    let cfg = PluginConfig { name, values };
    mgr.set_config(&cfg).map_err(AutoDevError::from)
}

// ─── Setup tokens ─────────────────────────────────────────────────────────────

/// Generate a one-click worker setup token.
#[tauri::command]
pub async fn generate_setup_token(
    fleet_id:    String,
    worker_type: String,
    repos:       Vec<String>,
    ttl_hours:   Option<u64>,
) -> Result<SetupToken, AutoDevError> {
    let mgr = PluginManager::new();
    let ttl = ttl_hours.unwrap_or(24);
    mgr.generate_setup_token(&fleet_id, &worker_type, &repos, ttl)
        .map_err(AutoDevError::from)
}

/// Validate a setup token and return its decoded claims.
#[tauri::command]
pub async fn validate_setup_token(token: String) -> Result<TokenClaims, AutoDevError> {
    let mgr = PluginManager::new();
    mgr.validate_setup_token(&token).map_err(AutoDevError::from)
}
