// PluginManager — orchestrates install / uninstall / update / verify / run.
// Stateless: all methods take &self and operate on the filesystem.

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

use super::{
    builtins::get_builtin,
    manifest::{
        InstallResult, InstalledPlugin, PluginConfig, PluginInfo, PluginManifest, RunResult,
        SetupToken, TokenClaims, UpdateResult, VerifyResult,
    },
    registry, runner, token,
};

pub struct PluginManager;

impl PluginManager {
    pub fn new() -> Self {
        Self
    }

    // ─── Marketplace / listing ────────────────────────────────────────────

    /// List all plugins available in the marketplace (built-ins + installed user plugins).
    pub fn list_marketplace(&self) -> Vec<PluginInfo> {
        registry::list_marketplace()
    }

    /// List currently installed plugins (built-ins that were installed + user plugins).
    pub fn list_installed(&self) -> Vec<InstalledPlugin> {
        registry::list_installed()
    }

    /// Get a single plugin's full manifest by name (installed or built-in).
    pub fn get_plugin(&self, name: &str) -> Option<PluginManifest> {
        // Check installed first
        registry::list_installed()
            .into_iter()
            .find(|p| p.manifest.name == name)
            .map(|p| p.manifest)
            // Fall back to built-in
            .or_else(|| get_builtin(name))
    }

    // ─── Install ─────────────────────────────────────────────────────────

    /// Install a plugin by name (from built-ins or a provided manifest).
    pub fn install(&self, name: &str) -> Result<InstallResult> {
        let manifest = self.resolve_manifest(name)?;

        // Create install directory
        let install_dir = registry::plugin_install_dir(name)?;
        std::fs::create_dir_all(&install_dir)?;

        // Write scripts
        runner::write_plugin_scripts(&manifest, &install_dir)
            .unwrap_or_else(|e| eprintln!("Could not write plugin scripts: {e}"));

        // Run install command
        let run_result = runner::install_plugin(&manifest, &install_dir);

        let mut warnings = Vec::new();
        if !run_result.success {
            // Clean up on failure
            let _ = std::fs::remove_dir_all(&install_dir);
            return Ok(InstallResult {
                success: false,
                plugin: name.to_string(),
                version: manifest.version,
                message: format!("Install failed: {}", run_result.stderr.trim()),
                warnings,
            });
        }

        // Persist manifest + metadata
        if let Err(e) = registry::persist_install(&manifest, None) {
            warnings.push(format!("Could not persist manifest: {e}"));
        }

        Ok(InstallResult {
            success: true,
            plugin: name.to_string(),
            version: manifest.version,
            message: format!("Plugin '{}' installed successfully.", name),
            warnings,
        })
    }

    /// Install a plugin from a raw YAML manifest string (for user-provided plugins).
    pub fn install_from_yaml(&self, yaml: &str) -> Result<InstallResult> {
        let manifest: PluginManifest =
            serde_yaml::from_str(yaml).context("Cannot parse plugin manifest YAML")?;
        let name = manifest.name.clone();

        // Write manifest to registry first so install can find it
        let install_dir = registry::plugin_install_dir(&name)?;
        std::fs::create_dir_all(&install_dir)?;
        std::fs::write(install_dir.join("manifest.yaml"), yaml)?;

        runner::write_plugin_scripts(&manifest, &install_dir)
            .unwrap_or_else(|e| eprintln!("Could not write scripts: {e}"));

        let run_result = runner::install_plugin(&manifest, &install_dir);

        if !run_result.success {
            let _ = std::fs::remove_dir_all(&install_dir);
            return Ok(InstallResult {
                success: false,
                plugin: name,
                version: manifest.version,
                message: format!("Install failed: {}", run_result.stderr.trim()),
                warnings: vec![],
            });
        }

        if let Err(e) = registry::persist_install(&manifest, None) {
            eprintln!("persist_install: {e}");
        }

        Ok(InstallResult {
            success: true,
            plugin: name.clone(),
            version: manifest.version,
            message: format!("Plugin '{}' installed from custom manifest.", name),
            warnings: vec![],
        })
    }

    // ─── Uninstall ────────────────────────────────────────────────────────

    pub fn uninstall(&self, name: &str) -> Result<bool> {
        let manifest = self
            .get_plugin(name)
            .ok_or_else(|| anyhow!("Plugin '{}' not found", name))?;

        let install_dir = registry::plugin_install_dir(name)?;
        let result = runner::uninstall_plugin(&manifest, &install_dir);

        if !result.success {
            return Err(anyhow!(
                "Uninstall command failed: {}",
                result.stderr.trim()
            ));
        }

        registry::remove_install(name)?;
        Ok(true)
    }

    // ─── Update ───────────────────────────────────────────────────────────

    pub fn update(&self, name: &str) -> Result<UpdateResult> {
        let old_manifest = self
            .get_plugin(name)
            .ok_or_else(|| anyhow!("Plugin '{}' not found", name))?;
        let old_version = old_manifest.version.clone();

        // Uninstall (ignoring errors) then re-install
        let _ = self.uninstall(name);
        let install_result = self.install(name)?;

        Ok(UpdateResult {
            success: install_result.success,
            plugin: name.to_string(),
            from_version: old_version,
            to_version: install_result.version,
            message: install_result.message,
        })
    }

    // ─── Verify ───────────────────────────────────────────────────────────

    pub fn verify(&self, name: &str) -> Result<VerifyResult> {
        let manifest = self
            .get_plugin(name)
            .ok_or_else(|| anyhow!("Plugin '{}' not found", name))?;
        Ok(runner::verify_plugin(&manifest))
    }

    // ─── Run ──────────────────────────────────────────────────────────────

    pub fn run(&self, name: &str, args: &[String]) -> Result<RunResult> {
        let manifest = self
            .get_plugin(name)
            .ok_or_else(|| anyhow!("Plugin '{}' not found", name))?;
        let install_dir = registry::plugin_install_dir(name)?;
        runner::run_plugin(&manifest, &install_dir, args)
    }

    // ─── Config ───────────────────────────────────────────────────────────

    pub fn get_config(&self, name: &str) -> Result<Option<Value>> {
        registry::list_installed()
            .into_iter()
            .find(|p| p.manifest.name == name)
            .map(|p| Ok(p.config))
            .unwrap_or(Ok(None))
    }

    pub fn set_config(&self, cfg: &PluginConfig) -> Result<()> {
        let manifest = self
            .get_plugin(&cfg.name)
            .ok_or_else(|| anyhow!("Plugin '{}' not found", cfg.name))?;
        registry::persist_install(&manifest, Some(&cfg.values))?;
        Ok(())
    }

    // ─── Setup tokens ─────────────────────────────────────────────────────

    pub fn generate_setup_token(
        &self,
        fleet_id: &str,
        worker_type: &str,
        repos: &[String],
        ttl_hours: u64,
    ) -> Result<SetupToken> {
        token::generate_token(fleet_id, worker_type, repos, ttl_hours)
    }

    pub fn validate_setup_token(&self, raw_token: &str) -> Result<TokenClaims> {
        token::validate_token(raw_token)
    }

    // ─── Helpers ──────────────────────────────────────────────────────────

    fn resolve_manifest(&self, name: &str) -> Result<PluginManifest> {
        // Prefer installed; fall back to built-in
        get_builtin(name)
            .or_else(|| {
                registry::list_installed()
                    .into_iter()
                    .find(|p| p.manifest.name == name)
                    .map(|p| p.manifest)
            })
            .ok_or_else(|| anyhow!("Plugin '{}' not found in built-ins or registry", name))
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
