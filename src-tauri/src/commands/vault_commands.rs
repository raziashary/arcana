// Vault commands — API key storage via the OS keychain.
//
// INV-12, asymmetric IPC: secrets travel INBOUND only.
//
// No command in this file returns key material. The frontend can store a
// secret, ask whether one exists, see the last four characters it just wrote,
// and delete it — but it can never read a stored secret back. This is what
// makes BYOK meaningful: a compromised or malicious frontend surface cannot
// exfiltrate the user's keys through the IPC bridge.
//
// `get_provider_key` and `get_github_pat` used to return the plaintext secret
// and were removed for exactly that reason (P0-09, ADR-LOG-RESTRUCT-0005).
//
// Rust code that needs the actual key calls `VaultManager::get` directly and
// in-process — provider construction in `agent_commands.rs`, `runner.rs` and
// friends. That is not an IPC surface and is unaffected by this rule.

use serde::{Deserialize, Serialize};

use crate::errors::Result;
use crate::vault::keychain::{github_pat_key, provider_key, VaultManager};

/// Providers whose keys are probed by `list_vault_entries`.
///
/// This deliberately includes ids that AutoDev no longer uses. A key stored
/// under a retired provider is precisely the orphan this command exists to
/// surface — dropping the id from the list would hide the secret in the user's
/// keychain rather than let them delete it.
const PROBED_PROVIDERS: &[&str] = &[
    // Current
    "anthropic",
    "openai",
    "google",
    "xai",
    // Retired or alternate spellings — probed so orphaned keys stay findable.
    "ollama",
    "gemini",
    "grok",
];

/// Outcome of an inbound secret write.
///
/// Carries just enough for the UI to confirm the intended key was pasted, and
/// nothing more.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultWriteResult {
    pub ok: bool,
    /// The last four characters of the stored secret. Empty when the secret is
    /// too short for a mask to reveal proportionally little.
    pub masked_last4: String,
}

/// Presence of one vault entry. Deliberately has no value field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEntryStatus {
    /// The canonical keychain key, e.g. `provider.anthropic.api_key`.
    pub key: String,
    /// A human-facing label for the UI.
    pub label: String,
    pub present: bool,
    /// True when a key exists for a provider AutoDev no longer supports, so the
    /// UI can offer to clean it up.
    pub orphaned: bool,
}

/// Last four characters of a secret, or empty if it is too short to mask.
///
/// Below eight characters, four would be half the secret.
fn masked_last4(secret: &str) -> String {
    let count = secret.chars().count();
    if count < 8 {
        return String::new();
    }
    secret.chars().skip(count - 4).collect()
}

/// Whether a provider id is one AutoDev currently supports.
fn is_supported_provider(id: &str) -> bool {
    matches!(id, "anthropic" | "openai" | "google" | "xai")
}

// ── Provider keys ────────────────────────────────────────────────────────────

/// Store a provider API key. Inbound only — the value is never returned.
#[tauri::command]
pub fn set_provider_key(provider: String, api_key: String) -> Result<VaultWriteResult> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        return Err(crate::errors::AutoDevError::Vault(
            "Refusing to store an empty API key.".into(),
        ));
    }
    VaultManager::store(&provider_key(&provider), trimmed)?;
    Ok(VaultWriteResult {
        ok: true,
        masked_last4: masked_last4(trimmed),
    })
}

/// Whether a key is stored for this provider. Never returns the key.
#[tauri::command]
pub fn has_provider_key(provider: String) -> Result<bool> {
    Ok(VaultManager::get(&provider_key(&provider))?.is_some())
}

#[tauri::command]
pub fn delete_provider_key(provider: String) -> Result<()> {
    VaultManager::delete(&provider_key(&provider))
}

// ── GitHub PAT ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn set_github_pat(pat: String) -> Result<VaultWriteResult> {
    let trimmed = pat.trim();
    if trimmed.is_empty() {
        return Err(crate::errors::AutoDevError::Vault(
            "Refusing to store an empty GitHub PAT.".into(),
        ));
    }
    VaultManager::store(&github_pat_key(), trimmed)?;
    Ok(VaultWriteResult {
        ok: true,
        masked_last4: masked_last4(trimmed),
    })
}

/// Whether a GitHub PAT is stored.
///
/// Replaces `get_github_pat`, which returned the token itself. `Settings.tsx`
/// only ever used it as a presence probe — and swallowed its errors, so
/// deleting it outright would have made the PAT badge read "none" forever
/// without any error surfacing.
#[tauri::command]
pub fn has_github_pat() -> Result<bool> {
    Ok(VaultManager::get(&github_pat_key())?.is_some())
}

#[tauri::command]
pub fn delete_github_pat() -> Result<()> {
    VaultManager::delete(&github_pat_key())
}

// ── Enumeration ──────────────────────────────────────────────────────────────

/// List every vault entry AutoDev knows how to look for, with presence only.
///
/// The `keyring` crate offers no cross-platform way to enumerate a service's
/// entries, so this probes a known key set rather than listing the keychain.
/// The consequence is stated plainly: a secret stored under a key not in
/// `PROBED_PROVIDERS` is invisible here.
#[tauri::command]
pub fn list_vault_entries() -> Result<Vec<VaultEntryStatus>> {
    let mut entries = Vec::with_capacity(PROBED_PROVIDERS.len() + 1);

    for id in PROBED_PROVIDERS {
        let key = provider_key(id);
        let present = VaultManager::get(&key)?.is_some();
        entries.push(VaultEntryStatus {
            key,
            label: format!("{id} API key"),
            present,
            orphaned: present && !is_supported_provider(id),
        });
    }

    let pat_key = github_pat_key();
    entries.push(VaultEntryStatus {
        present: VaultManager::get(&pat_key)?.is_some(),
        key: pat_key,
        label: "GitHub personal access token".to_string(),
        orphaned: false,
    });

    Ok(entries)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_shows_at_most_four_characters() {
        assert_eq!(masked_last4("sk-ant-api03-abcdefgh1234"), "1234");
        assert_eq!(masked_last4("12345678"), "5678");
    }

    #[test]
    fn mask_is_empty_for_secrets_too_short_to_mask() {
        // Four of seven characters would be most of the secret.
        assert_eq!(masked_last4("1234567"), "");
        assert_eq!(masked_last4("abc"), "");
        assert_eq!(masked_last4(""), "");
    }

    #[test]
    fn mask_handles_multibyte_secrets_without_panicking() {
        // Byte slicing here would panic on a char boundary.
        assert_eq!(masked_last4("kéyßµ-ünïcødé"), "cødé");
    }

    #[test]
    fn retired_providers_are_reported_as_orphaned() {
        assert!(is_supported_provider("anthropic"));
        assert!(is_supported_provider("xai"));
        assert!(!is_supported_provider("ollama"));
        assert!(!is_supported_provider("gemini"));
    }

    /// INV-12 as a check that can fail, across **every** command file.
    ///
    /// The rule: no `#[tauri::command]` that reads the vault may return a
    /// String-bearing type. `has_*` returns `bool`, `list_vault_entries`
    /// returns presence records, `test_provider_connection` returns a status —
    /// none of them can carry a secret out. The removed `get_provider_key` and
    /// `get_github_pat` both returned `Result<Option<String>>` straight from
    /// `VaultManager::get`, and both fail this test.
    ///
    /// Two rules, both narrow enough to mean something:
    ///   1. across **every** command file, the two removed function names must
    ///      not reappear;
    ///   2. within `vault_commands.rs`, the command set is closed and no
    ///      command returns a String-bearing type.
    ///
    /// Deliberately NOT a repo-wide "reads the vault ⇒ returns no String" rule.
    /// That was tried and is too coarse: `agent_commands::send_message` reads a
    /// key to construct a provider and returns `Ok(session_id)`, and
    /// `settings_commands::test_provider_connection` reads one and returns a
    /// status. Both are correct, and a gate that flags them gets switched off.
    /// A blanket ban on `Result<Option<String>>` is wrong for the same reason:
    /// `get_system_prompt` and `pick_workspace_dir` legitimately return one.
    ///
    /// Each file is truncated at its `#[cfg(test)]` marker, because this very
    /// assertion names the banned functions as string literals — a scan that
    /// matches its own source is the `"#[test]"`-in-a-literal trap that makes
    /// grep-based counters lie.
    #[test]
    fn no_command_returns_key_material() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands");
        let mut scanned = 0usize;

        for entry in std::fs::read_dir(&dir).expect("read src/commands") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read command file");
            let production = source.split("#[cfg(test)]").next().unwrap_or_default();
            scanned += 1;
            let file = path.file_name().unwrap_or_default().to_string_lossy().into_owned();

            for banned in ["fn get_provider_key", "fn get_github_pat"] {
                assert!(
                    !production.contains(banned),
                    "{file}: `{banned}` returns plaintext key material across the IPC \
                     boundary and must not exist (INV-12, P0-09). Use a has_* probe."
                );
            }

            if file != "vault_commands.rs" {
                continue;
            }

            // Within the vault's own IPC surface, the command set is closed and
            // no command may return a String-bearing type.
            let mut found: Vec<String> = Vec::new();
            for block in production.split("#[tauri::command]").skip(1) {
                let header = block.split_once('{').map(|(h, _)| h).unwrap_or(block);

                if let Some(rest) = header.split("pub fn ").nth(1) {
                    let name: String =
                        rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                    found.push(name);
                }

                // Only the RETURN type matters — parameters take `String`
                // routinely (a provider name, the key being stored), and that
                // direction is inbound and fine.
                if let Some((_, return_type)) = header.rsplit_once("->") {
                    assert!(
                        !return_type.contains("String"),
                        "{file}: a vault command returns a String-bearing type, which is \
                         how a secret leaves the process (INV-12):\n  -> {}",
                        return_type.trim()
                    );
                }
            }

            found.sort();
            let expected = [
                "delete_github_pat",
                "delete_provider_key",
                "has_github_pat",
                "has_provider_key",
                "list_vault_entries",
                "set_github_pat",
                "set_provider_key",
            ];
            assert_eq!(
                found, expected,
                "the vault IPC surface changed. This set is closed on purpose: adding a \
                 command here is the moment to check it cannot carry key material \
                 outbound (INV-12). Update this list deliberately."
            );
        }

        assert!(
            scanned > 20,
            "expected to scan the whole commands directory; scanned only {scanned} files"
        );
    }

    #[test]
    fn entry_status_carries_no_value_field() {
        let json = serde_json::to_string(&VaultEntryStatus {
            key: "provider.anthropic.api_key".into(),
            label: "anthropic API key".into(),
            present: true,
            orphaned: false,
        })
        .expect("serialize");

        assert!(!json.contains("value"), "VaultEntryStatus must never carry a value: {json}");
        assert!(json.contains("present"));
    }
}
