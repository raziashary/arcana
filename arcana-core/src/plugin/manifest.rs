// Plugin manifest types — parsed from plugin.yaml.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum PluginType {
    WorkerSetup,
    Environment,
    Security,
    Custom,
}

impl PluginType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::WorkerSetup => "worker-setup",
            Self::Environment => "environment",
            Self::Security => "security",
            Self::Custom => "custom",
        }
    }
}

impl std::fmt::Display for PluginType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
    pub name: String,
    pub version: String,
    /// Shell command to check if installed (e.g. "rustc --version")
    pub check_cmd: Option<String>,
    /// Shell command to install
    pub install_cmd: Option<String>,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginOsCommands {
    pub linux: Option<String>,
    pub macos: Option<String>,
    pub windows: Option<String>,
}

impl PluginOsCommands {
    pub fn for_current_os(&self) -> Option<&str> {
        #[cfg(target_os = "windows")]
        return self.windows.as_deref();
        #[cfg(target_os = "macos")]
        return self.macos.as_deref();
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        return self.linux.as_deref();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginCommands {
    pub install: Option<PluginOsCommands>,
    pub verify: Option<String>,
    pub uninstall: Option<String>,
    pub run: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfigRef {
    pub schema: Option<String>,
    pub defaults: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    #[serde(rename = "type")]
    pub plugin_type: PluginType,
    #[serde(default)]
    pub os_support: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<PluginDependency>,
    pub commands: PluginCommands,
    pub config: Option<PluginConfigRef>,
    /// True for plugins shipped with Arcana (not user-installed).
    #[serde(default)]
    pub builtin: bool,
    /// Short tag list for marketplace filtering.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Icon emoji or URL.
    #[serde(default = "default_icon")]
    pub icon: String,
}

fn default_icon() -> String {
    "🔌".into()
}

/// Summary for marketplace browsing (lighter than full manifest).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub plugin_type: String,
    pub os_support: Vec<String>,
    pub tags: Vec<String>,
    pub icon: String,
    pub installed: bool,
    pub builtin: bool,
}

impl From<&PluginManifest> for PluginInfo {
    fn from(m: &PluginManifest) -> Self {
        Self {
            name: m.name.clone(),
            version: m.version.clone(),
            description: m.description.clone(),
            author: m.author.clone(),
            plugin_type: m.plugin_type.to_string(),
            os_support: m.os_support.clone(),
            tags: m.tags.clone(),
            icon: m.icon.clone(),
            installed: false,
            builtin: m.builtin,
        }
    }
}

/// An installed plugin — manifest + installation metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub manifest: PluginManifest,
    pub install_path: String,
    pub installed_at: String,
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    pub success: bool,
    pub plugin: String,
    pub version: String,
    pub message: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResult {
    pub success: bool,
    pub plugin: String,
    pub from_version: String,
    pub to_version: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    pub success: bool,
    pub plugin: String,
    pub checks: Vec<DependencyCheck>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyCheck {
    pub name: String,
    pub required: bool,
    pub present: bool,
    pub version: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub name: String,
    pub values: serde_json::Value,
}

/// Setup token for one-click worker enrollment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupToken {
    pub token: String,
    pub worker_type: String,
    pub expires_at: String,
    pub one_liner: String,
}

/// Decoded claims from a setup token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    pub fleet_id: String,
    pub worker_type: String,
    pub repos: Vec<String>,
    pub expires_at: String,
    pub issued_at: String,
}
