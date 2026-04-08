// Vault module — OS keychain integration for secure API key storage.
// Uses the `keyring` crate which wraps the platform keychain:
//   Windows: Windows Credential Manager
//   macOS:   Keychain Services
//   Linux:   libsecret / KDE Wallet

pub mod keychain;
pub use keychain::{VaultManager, VaultEntry};
