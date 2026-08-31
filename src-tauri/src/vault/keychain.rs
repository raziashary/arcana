// OS keychain storage for AutoDev secrets (API keys, PATs).
// Service name scoped to "autodev" so keys don't collide with other apps.

use crate::errors::{AutoDevError, Result};
use keyring::Entry;

const SERVICE: &str = "autodev";

pub struct VaultManager;

impl VaultManager {
    /// Store a secret in the OS keychain.
    /// `key` acts as the username in the keychain entry.
    pub fn store(key: &str, value: &str) -> Result<()> {
        let entry = Entry::new(SERVICE, key).map_err(|e| AutoDevError::Vault(e.to_string()))?;
        entry
            .set_password(value)
            .map_err(|e| AutoDevError::Vault(e.to_string()))
    }

    /// Retrieve a secret from the OS keychain.
    /// Returns None if the key does not exist.
    pub fn get(key: &str) -> Result<Option<String>> {
        let entry = Entry::new(SERVICE, key).map_err(|e| AutoDevError::Vault(e.to_string()))?;
        match entry.get_password() {
            Ok(val) => Ok(Some(val)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AutoDevError::Vault(e.to_string())),
        }
    }

    /// Delete a secret from the OS keychain.
    pub fn delete(key: &str) -> Result<()> {
        let entry = Entry::new(SERVICE, key).map_err(|e| AutoDevError::Vault(e.to_string()))?;
        entry
            .delete_password()
            .map_err(|e: keyring::Error| AutoDevError::Vault(e.to_string()))
    }
}

// ── Canonical key helpers ─────────────────────────────────────────────────────

pub fn provider_key(provider: &str) -> String {
    format!("provider.{provider}.api_key")
}

pub fn github_pat_key() -> String {
    "github.pat".to_string()
}
