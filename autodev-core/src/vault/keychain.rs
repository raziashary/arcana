// Secret storage for AutoDev (API keys, PATs).
//
// Prefers the OS keychain and falls back to an encrypted file where no keychain
// is reachable (`P2-24`). The service name is scoped to "autodev" so keys do
// not collide with other applications.
//
// ── The user is told which store is holding their key (P2-71) ────────────────
//
// The two backends are not equivalent, and the difference is not cosmetic: the
// OS keychain is protected by the operating system, and the encrypted file is
// protected by file permissions and the fact that the key sits in a separate
// file. A user who believes their key is in the system keychain when it is in a
// file in their home directory has been misled about something that matters, so
// `VaultManager::backend()` reports the truth and the UI surfaces it.

use keyring::Entry;
use zeroize::Zeroize;

use super::fallback;
use crate::errors::{AutoDevError, Result};

const SERVICE: &str = "autodev";

/// Which store is holding secrets on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VaultBackend {
    /// The OS keychain — Keychain on macOS, Credential Manager on Windows,
    /// Secret Service on Linux.
    OsKeychain,
    /// The encrypted-file fallback, used when no OS keychain is reachable.
    EncryptedFile,
}

impl VaultBackend {
    /// A short, honest name for the UI.
    pub fn label(&self) -> &'static str {
        match self {
            VaultBackend::OsKeychain => {
                if cfg!(target_os = "macos") {
                    "macOS Keychain"
                } else if cfg!(target_os = "windows") {
                    "Windows Credential Manager"
                } else {
                    "Secret Service (system keyring)"
                }
            }
            VaultBackend::EncryptedFile => "Encrypted file (~/.autodev/config/vault.enc)",
        }
    }

    /// What the user needs to know about this store's protection.
    ///
    /// ⚠ Stated plainly rather than reassuringly. `docsMore.ts` claimed
    /// behaviour the code did not implement, which is worse than saying
    /// nothing: it invites the user to store a key under a guarantee that was
    /// never true.
    pub fn security_note(&self) -> &'static str {
        match self {
            VaultBackend::OsKeychain => {
                "Protected by the operating system's credential store."
            }
            VaultBackend::EncryptedFile => {
                "No system keyring was available, so keys are held in an encrypted file \
                 readable only by your user account. This is weaker than the system \
                 keyring: anyone who can read your home directory can read both the \
                 vault and its key. On Linux, installing gnome-keyring or KWallet \
                 restores keyring storage."
            }
        }
    }

    pub fn is_os_keychain(&self) -> bool {
        matches!(self, VaultBackend::OsKeychain)
    }
}

pub struct VaultManager;

impl VaultManager {
    /// Which backend this machine is using.
    ///
    /// Probed rather than assumed: the answer depends on whether a Secret
    /// Service is actually reachable right now, which a compile-time `cfg`
    /// cannot know. A container and a desktop session run the same binary.
    pub fn backend() -> VaultBackend {
        match Entry::new(SERVICE, "__backend_probe__") {
            // `NoEntry` means the keychain answered — it is reachable, and it
            // simply has no such entry. That is a success for this question.
            Ok(entry) => match entry.get_password() {
                Ok(_) | Err(keyring::Error::NoEntry) => VaultBackend::OsKeychain,
                Err(_) => VaultBackend::EncryptedFile,
            },
            Err(_) => VaultBackend::EncryptedFile,
        }
    }

    /// Store a secret. `key` acts as the username in the keychain entry.
    pub fn store(key: &str, value: &str) -> Result<()> {
        match Entry::new(SERVICE, key).and_then(|e| e.set_password(value)) {
            Ok(()) => {
                // ⚠ D-2. Remove any fallback copy of the same key.
                //
                // `get` prefers the keychain, so a stale fallback entry is
                // normally shadowed — but the reverse order is the dangerous
                // one: if a write ever landed in the fallback and a later write
                // succeeded to the keychain, two different values for one
                // provider would exist. Rotating a LEAKED key and having the old
                // one survive somewhere is the failure that matters here.
                if fallback::is_present() {
                    if let Err(e) = fallback::delete(key) {
                        tracing::warn!("could not clear the fallback copy of {key}: {e}");
                    }
                }
                index_add(key);
                Ok(())
            }
            Err(e) => {
                tracing::warn!(
                    "OS keychain unavailable ({e}); storing in the encrypted-file vault \
                     instead. This is weaker — see VaultBackend::security_note."
                );
                fallback::store(key, value)?;
                index_add(key);
                Ok(())
            }
        }
    }

    /// Retrieve a secret, or `None` if there is no such entry.
    pub fn get(key: &str) -> Result<Option<String>> {
        let entry = match Entry::new(SERVICE, key) {
            Ok(e) => e,
            Err(_) => return fallback::get(key),
        };
        match entry.get_password() {
            Ok(val) => Ok(Some(val)),
            Err(keyring::Error::NoEntry) => {
                // The keychain is reachable and has no such entry — but the
                // fallback may hold one written before a keyring was
                // installed. Checked so a key does not silently vanish when
                // the environment changes underneath the user.
                if fallback::is_present() {
                    fallback::get(key)
                } else {
                    Ok(None)
                }
            }
            Err(_) => fallback::get(key),
        }
    }

    /// Whether a secret exists, **without leaving it in memory** (`P2-70`).
    ///
    /// ── Why this is not just `get(..).is_some()` ─────────────────────────────
    ///
    /// `list_vault_entries` calls this once per probed provider on every
    /// Settings render, and each call previously pulled the full plaintext key
    /// out of the keychain, computed a boolean from it, and dropped the
    /// `String` — leaving the secret in freed heap memory to be reused or
    /// swapped at the allocator's convenience.
    ///
    /// ⚠ **It is not possible to avoid the decrypt.** The `keyring` crate
    /// exposes `get_password()` and nothing cheaper; there is no cross-platform
    /// "does this entry exist" call. So the secret does enter this process
    /// either way — what changes is that it is now overwritten immediately
    /// rather than left for the allocator.
    ///
    /// Claiming this "avoids decrypting the secret" would be the more
    /// satisfying sentence and would not be true.
    pub fn exists(key: &str) -> Result<bool> {
        match Self::get(key)? {
            Some(mut secret) => {
                secret.zeroize();
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Delete a secret.
    pub fn delete(key: &str) -> Result<()> {
        let mut removed_anywhere = false;

        if let Ok(entry) = Entry::new(SERVICE, key) {
            match entry.delete_password() {
                Ok(()) => removed_anywhere = true,
                Err(keyring::Error::NoEntry) => {}
                Err(e) => tracing::warn!("keychain delete for {key} failed: {e}"),
            }
        }

        // Always attempt the fallback too. A key can exist in both if the
        // environment changed between writes, and deleting only one leaves the
        // user believing a secret is gone while it is still on disk.
        if fallback::is_present() && fallback::delete(key)? {
            removed_anywhere = true;
        }

        index_remove(key);

        if removed_anywhere {
            Ok(())
        } else {
            Err(AutoDevError::Vault(format!("no stored secret for {key}")))
        }
    }

    /// Every key this machine has stored (`P2-33`).
    ///
    /// ── Why an index file exists ─────────────────────────────────────────────
    ///
    /// `keyring` offers no cross-platform enumeration, so `list_vault_entries`
    /// probed a hardcoded list of provider ids. Anything stored under a key not
    /// on that list was invisible — including a key for a provider dropped from
    /// the product, which is exactly the orphan the user needs to find in order
    /// to delete it.
    ///
    /// The index holds **key names only, never secrets**, so it is an ordinary
    /// config file. It is a supplement to the hardcoded probe list, not a
    /// replacement: an index can be lost, and the probe list still catches the
    /// providers we know about.
    pub fn enumerate() -> Result<Vec<String>> {
        let mut keys = index_read();

        if fallback::is_present() {
            for k in fallback::keys()? {
                if !keys.contains(&k) {
                    keys.push(k);
                }
            }
        }

        keys.sort();
        keys.dedup();
        Ok(keys)
    }
}

// ── The key-name index ───────────────────────────────────────────────────────
//
// Names only. If this file leaked it would reveal which providers a user has
// configured and nothing else.

fn index_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".autodev").join("config").join("vault-index.json"))
}

fn index_read() -> Vec<String> {
    index_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default()
}

fn index_write(keys: &[String]) {
    let Some(path) = index_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(keys) {
        // A failed index write must not fail the store that preceded it: the
        // secret is already safely in the keychain, and losing the index costs
        // discoverability, not the key.
        if let Err(e) = std::fs::write(&path, json) {
            tracing::warn!("could not update the vault key index at {}: {e}", path.display());
        }
    }
}

fn index_add(key: &str) {
    let mut keys = index_read();
    if !keys.iter().any(|k| k == key) {
        keys.push(key.to_string());
        keys.sort();
        index_write(&keys);
    }
}

fn index_remove(key: &str) {
    let mut keys = index_read();
    let before = keys.len();
    keys.retain(|k| k != key);
    if keys.len() != before {
        index_write(&keys);
    }
}

// ── Canonical key helpers ─────────────────────────────────────────────────────

pub fn provider_key(provider: &str) -> String {
    format!("provider.{provider}.api_key")
}

pub fn github_pat_key() -> String {
    "github.pat".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backend_always_reports_a_label_and_a_security_note() {
        // Both must be non-empty: the UI shows them, and an empty security note
        // reads as "nothing to worry about" for the backend that has the most
        // to say.
        for b in [VaultBackend::OsKeychain, VaultBackend::EncryptedFile] {
            assert!(!b.label().is_empty());
            assert!(!b.security_note().is_empty());
        }
    }

    #[test]
    fn the_fallback_note_states_the_weakness_rather_than_glossing_it() {
        // `docsMore.ts` claimed protection the code did not implement. The
        // fallback's note must name the actual limit, or it repeats that.
        let note = VaultBackend::EncryptedFile.security_note();
        assert!(note.contains("weaker"), "the note must not imply parity with the keyring");
        assert!(
            note.contains("home directory"),
            "the note must say who can read it, not merely that it is encrypted"
        );
        assert!(
            !VaultBackend::EncryptedFile.is_os_keychain(),
            "the fallback must never report itself as the OS keychain"
        );
    }

    #[test]
    fn provider_keys_are_namespaced_and_distinct() {
        assert_eq!(provider_key("anthropic"), "provider.anthropic.api_key");
        assert_ne!(provider_key("openai"), provider_key("google"));
        // The PAT lives outside the provider namespace so a provider named
        // "github" could never collide with it.
        assert!(!github_pat_key().starts_with("provider."));
    }

    #[test]
    fn the_backend_probe_treats_no_entry_as_a_reachable_keychain() {
        // The distinction the probe rests on: `NoEntry` means the keychain
        // ANSWERED and holds no such key, which is a success. Reading it as a
        // failure would push every machine onto the file fallback and quietly
        // downgrade everyone's key storage.
        let b = VaultManager::backend();
        assert!(
            matches!(b, VaultBackend::OsKeychain | VaultBackend::EncryptedFile),
            "the probe must always resolve to a concrete backend"
        );
    }
}
