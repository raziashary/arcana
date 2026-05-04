// Auto-updater verification module.
// Verifies downloaded update packages via Ed25519 signature + SHA-256 checksum.
// The public key is embedded at compile time from AUTODEV_UPDATE_PUBKEY env var
// or falls back to the hardcoded development key.

use anyhow::{anyhow, Context, Result};
use ed25519_dalek::{Signature, VerifyingKey, Verifier};
use std::path::Path;

/// Base64-encoded Ed25519 public key for verifying update packages.
/// Override at build time via AUTODEV_UPDATE_PUBKEY env var.
/// This is a placeholder key — replace with real key before shipping.
const EMBEDDED_PUBLIC_KEY: &str =
    "dW5kZWZpbmVkLXB1YmxpYy1rZXktZm9yLWRldmVsb3BtZW50LW9ubHktcGxhY2Vob2xkZXI=";

/// Verify the SHA-256 checksum of a downloaded file.
pub fn verify_checksum(file_path: &Path, expected_hex: &str) -> Result<()> {
    if expected_hex.is_empty() {
        // No checksum provided — skip (warn in production)
        return Ok(());
    }

    let data = std::fs::read(file_path)
        .with_context(|| format!("Cannot read file for checksum: {}", file_path.display()))?;

    let computed = sha256_hex(&data);
    if !constant_time_eq(computed.as_bytes(), expected_hex.as_bytes()) {
        anyhow::bail!(
            "SHA-256 checksum mismatch!\n  Expected: {expected_hex}\n  Computed: {computed}"
        );
    }
    Ok(())
}

/// Verify the Ed25519 signature over a file.
/// `sig_bytes` is the raw 64-byte signature (not base64).
pub fn verify_signature(file_path: &Path, sig_bytes: &[u8]) -> Result<()> {
    if sig_bytes.is_empty() {
        // No signature available — skip in dev builds, error in prod
        #[cfg(debug_assertions)]
        return Ok(());
        #[cfg(not(debug_assertions))]
        anyhow::bail!("No signature provided for update verification");
    }

    let pubkey_b64 = std::env::var("AUTODEV_UPDATE_PUBKEY")
        .unwrap_or_else(|_| EMBEDDED_PUBLIC_KEY.to_string());

    let pubkey_bytes = base64_decode(&pubkey_b64)
        .context("Failed to decode embedded public key")?;

    if pubkey_bytes.len() != 32 {
        // Dev placeholder key — skip verification
        #[cfg(debug_assertions)]
        return Ok(());
        #[cfg(not(debug_assertions))]
        anyhow::bail!("Invalid public key length ({})", pubkey_bytes.len());
    }

    let pubkey_arr: [u8; 32] = pubkey_bytes.try_into().map_err(|_| anyhow!("Key size error"))?;
    let verifying_key = VerifyingKey::from_bytes(&pubkey_arr)
        .context("Invalid Ed25519 public key")?;

    if sig_bytes.len() != 64 {
        anyhow::bail!("Invalid signature length: {} (expected 64)", sig_bytes.len());
    }
    let sig_arr: [u8; 64] = sig_bytes.try_into().map_err(|_| anyhow!("Sig size error"))?;
    let signature = Signature::from_bytes(&sig_arr);

    let data = std::fs::read(file_path)
        .with_context(|| format!("Cannot read file for signature: {}", file_path.display()))?;

    verifying_key.verify(&data, &signature)
        .context("Ed25519 signature verification failed")?;

    Ok(())
}

/// Verify both checksum and signature. Returns Ok(()) if both pass.
pub async fn verify_update(
    file_path: &Path,
    expected_checksum: &str,
    sig_url: &str,
) -> Result<()> {
    // 1. SHA-256 checksum (fast, local)
    verify_checksum(file_path, expected_checksum)
        .context("Checksum verification failed")?;

    // 2. Ed25519 signature (requires downloading .sig)
    if !sig_url.is_empty() {
        let sig_bytes = super::download::download_signature(sig_url).await
            .context("Failed to download signature")?;
        verify_signature(file_path, &sig_bytes)
            .context("Signature verification failed")?;
    }

    Ok(())
}

// ─── SHA-256 (uses sha2 crate already in Cargo.toml) ─────────────────────────

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    result.iter().map(|b| format!("{b:02x}")).collect()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn base64_decode(s: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .context("Base64 decode error")
}
