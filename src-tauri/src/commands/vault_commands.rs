// Vault commands — API key storage via OS keychain.

use crate::errors::Result;
use crate::vault::keychain::{VaultManager, provider_key, github_pat_key};

#[tauri::command]
pub fn store_provider_key(provider: String, api_key: String) -> Result<()> {
    VaultManager::store(&provider_key(&provider), &api_key)
}

#[tauri::command]
pub fn get_provider_key(provider: String) -> Result<Option<String>> {
    VaultManager::get(&provider_key(&provider))
}

#[tauri::command]
pub fn delete_provider_key(provider: String) -> Result<()> {
    VaultManager::delete(&provider_key(&provider))
}

#[tauri::command]
pub fn store_github_pat(pat: String) -> Result<()> {
    VaultManager::store(&github_pat_key(), &pat)
}

#[tauri::command]
pub fn get_github_pat() -> Result<Option<String>> {
    VaultManager::get(&github_pat_key())
}

#[tauri::command]
pub fn delete_github_pat() -> Result<()> {
    VaultManager::delete(&github_pat_key())
}
