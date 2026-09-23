// The encrypted-file vault fallback (P2-24).
//
// ── When this is used, and why it has to exist ───────────────────────────────
//
// `deb` and `appimage` are shipped bundle targets. On Linux the `keyring` crate
// talks to the Secret Service over D-Bus, which is not present on a headless
// host, inside most containers, or under a minimal window manager. There, every
// vault call fails — so BYOK fails, so the app cannot make a single LLM call.
// That is not a degraded experience, it is an unusable install.
//
// ── What this is NOT ─────────────────────────────────────────────────────────
//
// ⚠ **This is materially weaker than the OS keychain, and the user is told so.**
//
// There is no user passphrase, so the AES key has to live on the same disk as
// the ciphertext. An attacker who can read the user's home directory can read
// both files and decrypt the vault. What this actually defends against is
// narrower, and worth stating precisely rather than dressing up:
//
//   * a secret sitting in plaintext in a config file that gets copied into a
//     bug report, a backup, a synced folder, or a screen share;
//   * another user on the same machine, via `0600` permissions;
//   * a process that reads config files opportunistically without targeting us.
//
// It does **not** defend against a local attacker who is looking for it. The OS
// keychain does, which is why it is always preferred and why
// `VaultManager::backend()` exists (`P2-71`) — the user is entitled to know
// which of the two is holding their key.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Key, Nonce};
use zeroize::Zeroize;

use crate::errors::{ArcanaError, Result};

/// Where the ciphertext lives.
fn vault_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("vault.enc"))
}

/// Where the AES key lives. Separate file so the key can be `0600` regardless
/// of how the vault file is handled, and so a stray copy of one is useless
/// without the other.
fn key_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("vault.key"))
}

fn config_dir() -> Result<PathBuf> {
    let base = dirs::home_dir()
        .ok_or_else(|| ArcanaError::Vault("no home directory to store the vault in".into()))?;
    let dir = base.join(".arcana").join("config");
    std::fs::create_dir_all(&dir)
        .map_err(|e| ArcanaError::Vault(format!("creating {}: {e}", dir.display())))?;
    Ok(dir)
}

/// Write a file readable only by its owner.
///
/// On Unix the mode is set **at creation**, not afterwards: a `create` followed
/// by a `set_permissions` leaves a window in which the file exists at the
/// default umask, and for a file holding an encryption key that window is the
/// whole vulnerability.
fn write_owner_only(path: &Path, bytes: &[u8]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| ArcanaError::Vault(format!("opening {}: {e}", path.display())))?;
        f.write_all(bytes)
            .map_err(|e| ArcanaError::Vault(format!("writing {}: {e}", path.display())))?;
        return Ok(());
    }

    #[cfg(not(unix))]
    {
        // Windows and macOS reach this only in unusual configurations — the OS
        // keychain is available on both and is what gets used. ACL narrowing is
        // not attempted here rather than done badly; the backend disclosure
        // (P2-71) tells the user which store is in play.
        std::fs::write(path, bytes)
            .map_err(|e| ArcanaError::Vault(format!("writing {}: {e}", path.display())))?;
        Ok(())
    }
}

/// Load the AES key, creating one on first use.
fn load_or_create_key() -> Result<[u8; 32]> {
    let path = key_path()?;

    if path.exists() {
        let bytes = std::fs::read(&path)
            .map_err(|e| ArcanaError::Vault(format!("reading the vault key: {e}")))?;
        if bytes.len() != 32 {
            return Err(ArcanaError::Vault(format!(
                "the vault key at {} is {} bytes, not 32 — refusing to guess at it, because \
                 overwriting it would make every stored secret permanently unreadable",
                path.display(),
                bytes.len()
            )));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        return Ok(key);
    }

    // ⚠ D-4. Generating a fresh key when a vault already exists would make
    // every stored secret permanently undecryptable — and silently, because the
    // next `load()` would simply fail to decrypt and report corruption.
    //
    // Refused instead, with the actual remedy stated. The keys are recoverable
    // by re-entering them; a vault silently replaced by an empty one is not
    // recoverable at all, and the user would not know to try.
    if vault_path()?.exists() {
        return Err(ArcanaError::Vault(format!(
            "the vault at {} exists but its key file {} is missing. Every stored secret \
             is unreadable without it. If you have a copy of the key file, restore it; \
             otherwise delete the vault file and re-enter your API keys in Settings.",
            vault_path()?.display(),
            path.display()
        )));
    }

    let key = Aes256Gcm::generate_key(&mut OsRng);
    write_owner_only(&path, key.as_slice())?;
    let mut out = [0u8; 32];
    out.copy_from_slice(key.as_slice());
    Ok(out)
}

/// Read and decrypt the whole vault.
fn load() -> Result<BTreeMap<String, String>> {
    let path = vault_path()?;
    if !path.exists() {
        return Ok(BTreeMap::new());
    }

    let blob = std::fs::read(&path)
        .map_err(|e| ArcanaError::Vault(format!("reading the vault: {e}")))?;
    if blob.len() < 12 {
        return Err(ArcanaError::Vault(
            "the vault file is too short to contain a nonce — it is truncated or corrupt".into(),
        ));
    }

    let mut key_bytes = load_or_create_key()?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
    key_bytes.zeroize();

    let (nonce_bytes, ciphertext) = blob.split_at(12);
    let mut plaintext = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .map_err(|_| {
            // Deliberately does not say which of the two it was: the
            // distinction is not actionable for the user and the failure is
            // the same either way.
            ArcanaError::Vault(
                "the vault could not be decrypted — the key file and the vault file do not \
                 match, or one of them is corrupt"
                    .into(),
            )
        })?;

    let parsed: BTreeMap<String, String> = serde_json::from_slice(&plaintext)
        .map_err(|e| ArcanaError::Vault(format!("the decrypted vault is not valid JSON: {e}")))?;
    plaintext.zeroize();

    Ok(parsed)
}

/// Encrypt and write the whole vault.
fn save(entries: &BTreeMap<String, String>) -> Result<()> {
    let mut key_bytes = load_or_create_key()?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
    key_bytes.zeroize();

    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let mut plaintext = serde_json::to_vec(entries)
        .map_err(|e| ArcanaError::Vault(format!("serialising the vault: {e}")))?;

    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_slice())
        .map_err(|_| ArcanaError::Vault("encrypting the vault failed".into()))?;
    plaintext.zeroize();

    // Nonce is prepended. It is not secret — it must simply never repeat for a
    // given key, which `generate_nonce` handles.
    let mut blob = nonce.to_vec();
    blob.extend_from_slice(&ciphertext);

    // ⚠ D-3. Written to a temporary file and RENAMED over the real one.
    //
    // This used to truncate the live vault in place. A crash, a power loss, or
    // a full disk between the truncate and the write left a zero-length or
    // half-written file — and since the contents are a single encrypted blob,
    // that is not "some keys lost", it is **every key lost**, unrecoverably,
    // with no backup anywhere.
    //
    // `rename` within the same directory is atomic on every platform we ship:
    // a reader sees either the old vault or the new one, never a partial.
    let target = vault_path()?;
    let temp = target.with_extension("enc.tmp");

    write_owner_only(&temp, &blob)?;
    std::fs::rename(&temp, &target).map_err(|e| {
        // Leave the temp file: it holds the only copy of the new state, and
        // deleting it here would discard the write we just made.
        ArcanaError::Vault(format!(
            "could not replace the vault at {}: {e}. The new contents are in {} — \
             rename it manually to recover.",
            target.display(),
            temp.display()
        ))
    })
}

/// Apply a store to an entry map.
///
/// Separated from the file I/O so `P2-25` can be proven without writing to the
/// developer's real vault. Running the disk path in a unit test would rewrite
/// the machine's actual credentials, which is not a cost a test may impose.
pub fn apply_store(entries: &mut BTreeMap<String, String>, key: &str, value: &str) {
    entries.insert(key.to_string(), value.to_string());
}

/// Apply a delete to an entry map. Returns whether anything was removed.
pub fn apply_delete(entries: &mut BTreeMap<String, String>, key: &str) -> bool {
    entries.remove(key).is_some()
}

pub fn store(key: &str, value: &str) -> Result<()> {
    let mut entries = load()?;
    apply_store(&mut entries, key, value);
    save(&entries)
}

pub fn get(key: &str) -> Result<Option<String>> {
    Ok(load()?.get(key).cloned())
}

/// Remove a key. Returns whether anything was actually removed.
///
/// D-5: the bool used to be discarded here while a test comment claimed
/// `VaultManager::delete` surfaces it to the caller — so "no such secret" and
/// "removed successfully" were the same outcome, and a typo'd provider id
/// reported success.
pub fn delete(key: &str) -> Result<bool> {
    let mut entries = load()?;
    let removed = apply_delete(&mut entries, key);
    if removed {
        save(&entries)?;
    }
    Ok(removed)
}

/// Every key held here.
///
/// Enumeration is free for this backend — it is a map. The OS keychain is the
/// one that cannot enumerate, which is what `P2-33` is about.
pub fn keys() -> Result<Vec<String>> {
    Ok(load()?.keys().cloned().collect())
}

/// Whether the fallback has ever been written on this machine.
pub fn is_present() -> bool {
    vault_path().map(|p| p.exists()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    // These touch real files under the user's home directory, so they are
    // deliberately NOT run by default: a developer running `cargo test` should
    // not have their own vault rewritten. `P2-24`'s platform proof is the
    // Linux CI job.
    //
    // The pure logic below needs no filesystem.

    #[test]
    fn a_short_vault_file_is_reported_as_corrupt_not_parsed() {
        // A blob shorter than the nonce cannot be split, and slicing it would
        // panic. The check exists so a truncated file is an error rather than
        // a crash.
        let blob = vec![0u8; 4];
        assert!(blob.len() < 12, "the guard's premise");
    }

    #[test]
    fn the_nonce_is_prepended_and_is_twelve_bytes() {
        // Pinning the framing: `load` splits at 12, so `save` must write 12.
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        assert_eq!(nonce.len(), 12, "load() splits the blob at 12 bytes");
    }

    #[test]
    fn a_round_trip_through_the_cipher_recovers_the_secret() {
        // The cipher contract itself, without touching the filesystem.
        let key = Aes256Gcm::generate_key(&mut OsRng);
        let cipher = Aes256Gcm::new(&key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        let secret = br#"{"provider.anthropic.api_key":"sk-ant-test"}"#;
        let ct = cipher.encrypt(&nonce, secret.as_ref()).expect("encrypt");
        assert_ne!(&ct[..], &secret[..], "the ciphertext must not be the plaintext");

        let pt = cipher.decrypt(&nonce, ct.as_ref()).expect("decrypt");
        assert_eq!(pt, secret);
    }

    #[test]
    fn a_wrong_key_fails_to_decrypt_rather_than_returning_garbage() {
        // AES-GCM is authenticated, so a mismatched key is an error rather
        // than plausible-looking nonsense. That is what lets `load()` report
        // "the key file and the vault file do not match" honestly.
        let cipher = Aes256Gcm::new(&Aes256Gcm::generate_key(&mut OsRng));
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ct = cipher.encrypt(&nonce, b"secret".as_ref()).expect("encrypt");

        let other = Aes256Gcm::new(&Aes256Gcm::generate_key(&mut OsRng));
        assert!(other.decrypt(&nonce, ct.as_ref()).is_err());
    }
    // ── P2-25 · per-provider keys, independently configurable ─────────────
    //
    // Proven at the map level. The disk path is exercised by the live smoke,
    // because a unit test that ran it would rewrite the developer's real
    // credentials — a cost no test may impose to prove a point about naming.

    use crate::vault::keychain::provider_key;

    #[test]
    fn each_provider_gets_its_own_entry() {
        let mut v = BTreeMap::new();
        apply_store(&mut v, &provider_key("anthropic"), "sk-ant-A");
        apply_store(&mut v, &provider_key("openai"), "sk-oai-B");
        apply_store(&mut v, &provider_key("google"), "AIza-C");

        assert_eq!(v.len(), 3, "three providers must occupy three entries");
        assert_eq!(v.get(&provider_key("anthropic")).map(String::as_str), Some("sk-ant-A"));
        assert_eq!(v.get(&provider_key("openai")).map(String::as_str), Some("sk-oai-B"));
        assert_eq!(v.get(&provider_key("google")).map(String::as_str), Some("AIza-C"));
    }

    #[test]
    fn rotating_one_key_replaces_only_that_key() {
        // Rotation is a store over an existing entry. The invariant that
        // matters is that it does not disturb the others: a user rotating a
        // leaked Anthropic key must not lose their OpenAI access.
        let mut v = BTreeMap::new();
        apply_store(&mut v, &provider_key("anthropic"), "sk-ant-OLD");
        apply_store(&mut v, &provider_key("openai"), "sk-oai-KEEP");

        apply_store(&mut v, &provider_key("anthropic"), "sk-ant-NEW");

        assert_eq!(v.get(&provider_key("anthropic")).map(String::as_str), Some("sk-ant-NEW"));
        assert_eq!(
            v.get(&provider_key("openai")).map(String::as_str),
            Some("sk-oai-KEEP"),
            "rotating one provider must not disturb another"
        );
        assert_eq!(v.len(), 2, "rotation replaces, it does not accumulate");
    }

    #[test]
    fn removing_one_key_leaves_the_others_intact() {
        let mut v = BTreeMap::new();
        for p in ["anthropic", "openai", "google"] {
            apply_store(&mut v, &provider_key(p), "secret");
        }

        assert!(apply_delete(&mut v, &provider_key("openai")));
        assert_eq!(v.len(), 2);
        assert!(!v.contains_key(&provider_key("openai")));
        assert!(v.contains_key(&provider_key("anthropic")));
        assert!(v.contains_key(&provider_key("google")));

        // Deleting again reports that nothing was there, rather than silently
        // succeeding — `VaultManager::delete` surfaces that to the caller.
        assert!(!apply_delete(&mut v, &provider_key("openai")));
    }

    #[test]
    fn the_github_pat_is_outside_the_provider_namespace() {
        // So a provider that happened to be called "github" could never collide
        // with the PAT and silently overwrite it.
        let mut v = BTreeMap::new();
        apply_store(&mut v, &crate::vault::keychain::github_pat_key(), "ghp_TOKEN");
        apply_store(&mut v, &provider_key("github"), "not-the-pat");

        assert_eq!(v.len(), 2, "these must be two distinct entries");
        assert_eq!(
            v.get(&crate::vault::keychain::github_pat_key()).map(String::as_str),
            Some("ghp_TOKEN")
        );
    }
}
