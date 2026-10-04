// Auto-updater verification module.
// Verifies downloaded update packages via SHA-256 checksum + Ed25519 signature.
//
// Both checks are MANDATORY in every build profile. There is no "absent means
// skip" path: an update that arrives without a checksum, or without a signature,
// is refused. Previously an empty checksum and an empty signature URL each
// returned Ok(()), and the caller turned a failed checksum fetch into an empty
// string — three independent ways to install an unverified binary.
// See ADR-LOG-RESTRUCT-0002 and ADR-LOG-RESTRUCT-0003.
//
// The public key is embedded at COMPILE time from the ARCANA_UPDATE_PUBKEY
// environment variable (see build.rs). There is deliberately NO runtime override:
// an environment variable that can replace the update trust root is an injection
// surface, and main.rs loads .env files by walking up from the working directory,
// so a runtime lookup would be satisfiable by dropping a file next to the binary.

use anyhow::{anyhow, Context, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::path::Path;

/// Base64-encoded Ed25519 public key for verifying update packages.
/// Fixed at compile time; see the module header for why there is no override.
const EMBEDDED_PUBLIC_KEY: Option<&str> = option_env!("ARCANA_UPDATE_PUBKEY");

/// A release build with no signing key must not exist. Such a binary could only
/// ever refuse updates, which reads to a user as "updates are broken" rather than
/// "this build was produced without its key". Fail at compile time instead.
#[cfg(not(debug_assertions))]
const _RELEASE_REQUIRES_PUBKEY: () = {
    if EMBEDDED_PUBLIC_KEY.is_none() {
        panic!(
            "ARCANA_UPDATE_PUBKEY must be set at build time for release builds. \
             See docs/updater-signing.md for key generation and CI setup."
        );
    }
};

/// Verify the SHA-256 checksum of a downloaded file.
///
/// An absent or malformed checksum is a hard failure — it is not "no checksum to
/// compare against", it is "this artifact cannot be shown to be what it claims".
pub fn verify_checksum(file_path: &Path, expected_hex: &str) -> Result<()> {
    let expected = expected_hex.trim().to_ascii_lowercase();

    if expected.is_empty() {
        anyhow::bail!(
            "Update refused: no SHA-256 checksum was published for this artifact. \
             Arcana will not install an unverified update."
        );
    }
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        anyhow::bail!(
            "Update refused: malformed SHA-256 checksum ({} chars; expected 64 hex digits).",
            expected.len()
        );
    }

    let data = std::fs::read(file_path)
        .with_context(|| format!("Cannot read file for checksum: {}", file_path.display()))?;

    let computed = sha256_hex(&data);
    if !constant_time_eq(computed.as_bytes(), expected.as_bytes()) {
        anyhow::bail!(
            "Update refused: SHA-256 mismatch.\n  Expected: {expected}\n  Computed: {computed}"
        );
    }
    Ok(())
}

/// Verify the Ed25519 signature over a file using the compile-time embedded key.
/// `sig_bytes` is the raw 64-byte signature (not base64).
pub fn verify_signature(file_path: &Path, sig_bytes: &[u8]) -> Result<()> {
    let pubkey_b64 = EMBEDDED_PUBLIC_KEY.ok_or_else(|| {
        anyhow!(
            "Update refused: this build has no update-signing public key compiled in, \
             so no update can be verified. Rebuild with ARCANA_UPDATE_PUBKEY set."
        )
    })?;
    verify_signature_with_key(file_path, sig_bytes, pubkey_b64)
}

/// The verification itself, with the trust root passed in.
///
/// Split out from `verify_signature` so the signature path is testable: the
/// embedded key is a compile-time constant and is absent in a test build, which
/// would otherwise leave the load-bearing case — a signature made by the WRONG
/// key is rejected — with no way to exercise it.
fn verify_signature_with_key(file_path: &Path, sig_bytes: &[u8], pubkey_b64: &str) -> Result<()> {
    if sig_bytes.is_empty() {
        anyhow::bail!("Update refused: the downloaded signature is empty.");
    }
    let sig_arr: [u8; 64] = sig_bytes.try_into().map_err(|_| {
        anyhow!(
            "Update refused: invalid signature length {} (expected 64).",
            sig_bytes.len()
        )
    })?;
    let signature = Signature::from_bytes(&sig_arr);

    let pubkey_bytes =
        base64_decode(pubkey_b64).context("Update-signing public key is not valid base64")?;
    let pubkey_len = pubkey_bytes.len();
    let pubkey_arr: [u8; 32] = pubkey_bytes
        .try_into()
        .map_err(|_| anyhow!("Update-signing public key is {pubkey_len} bytes; expected 32."))?;
    let verifying_key = VerifyingKey::from_bytes(&pubkey_arr)
        .context("Update-signing public key is not a valid Ed25519 key")?;

    let data = std::fs::read(file_path)
        .with_context(|| format!("Cannot read file for signature: {}", file_path.display()))?;

    verifying_key
        .verify(&data, &signature)
        .context("Update refused: Ed25519 signature verification failed")?;

    Ok(())
}

/// Verify both checksum and signature. Returns Ok(()) only if BOTH pass.
pub async fn verify_update(file_path: &Path, expected_checksum: &str, sig_url: &str) -> Result<()> {
    // 1. SHA-256 checksum (fast, local)
    verify_checksum(file_path, expected_checksum).context("Checksum verification failed")?;

    // 2. Ed25519 signature. An absent signature URL is a refusal, not a skip.
    if sig_url.trim().is_empty() {
        anyhow::bail!(
            "Update refused: no signature was published for this artifact. Every Arcana \
             release must publish a detached .sig alongside its installer."
        );
    }
    super::download::require_https(sig_url).context("Signature URL rejected")?;

    let sig_bytes = super::download::download_signature(sig_url)
        .await
        .context("Failed to download signature")?;
    verify_signature(file_path, &sig_bytes).context("Signature verification failed")?;

    Ok(())
}

// ─── SHA-256 (uses sha2 crate already in Cargo.toml) ─────────────────────────

pub(super) fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    result.iter().map(|b| format!("{b:02x}")).collect()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn base64_decode(s: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .context("Base64 decode error")
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey};
    use std::io::Write;

    /// Write bytes to a uniquely-named file under the OS temp dir.
    /// Avoids adding a `tempfile` dev-dependency for four tests.
    fn temp_file(tag: &str, contents: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "arcana-verify-test-{tag}-{}.bin",
            uuid::Uuid::new_v4()
        ));
        let mut f = std::fs::File::create(&path).expect("create temp file");
        f.write_all(contents).expect("write temp file");
        path
    }

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    // ── checksum ─────────────────────────────────────────────────────────────

    #[test]
    fn checksum_accepts_a_correct_digest() {
        let path = temp_file("ck-ok", b"hello arcana");
        let expected = sha256_hex(b"hello arcana");
        assert!(verify_checksum(&path, &expected).is_ok());
        // Uppercase and surrounding whitespace must also be accepted.
        assert!(verify_checksum(&path, &format!("  {}  ", expected.to_uppercase())).is_ok());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn checksum_refuses_an_empty_string() {
        // The regression this exists for: an empty checksum used to return Ok(()),
        // and the caller's unwrap_or_default() produced exactly this input whenever
        // the checksum fetch failed.
        let path = temp_file("ck-empty", b"payload");
        let err = verify_checksum(&path, "").unwrap_err().to_string();
        assert!(
            err.contains("no SHA-256 checksum"),
            "unexpected error: {err}"
        );
        assert!(verify_checksum(&path, "   ").is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn checksum_refuses_malformed_and_mismatched_digests() {
        let path = temp_file("ck-bad", b"payload");
        // Too short.
        assert!(verify_checksum(&path, "abc123").is_err());
        // Right length, non-hex.
        assert!(verify_checksum(&path, &"z".repeat(64)).is_err());
        // Right length, valid hex, wrong value.
        assert!(verify_checksum(&path, &"a".repeat(64)).is_err());
        let _ = std::fs::remove_file(&path);
    }

    // ── signature ────────────────────────────────────────────────────────────

    #[test]
    fn signature_accepts_the_matching_key_and_refuses_another() {
        let payload = b"arcana installer bytes";
        let path = temp_file("sig-ok", payload);

        let signer = SigningKey::from_bytes(&[7u8; 32]);
        let impostor = SigningKey::from_bytes(&[9u8; 32]);

        let good_sig = signer.sign(payload).to_bytes();
        let good_key = b64(signer.verifying_key().as_bytes());
        let other_key = b64(impostor.verifying_key().as_bytes());

        // The correct key verifies.
        assert!(verify_signature_with_key(&path, &good_sig, &good_key).is_ok());

        // The load-bearing case: a signature made by a different key is refused.
        assert!(verify_signature_with_key(&path, &good_sig, &other_key).is_err());

        // And a signature over different bytes is refused by the correct key.
        let wrong_sig = signer.sign(b"other bytes").to_bytes();
        assert!(verify_signature_with_key(&path, &wrong_sig, &good_key).is_err());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn signature_refuses_empty_and_wrong_length() {
        let path = temp_file("sig-len", b"payload");
        let key = b64(SigningKey::from_bytes(&[3u8; 32])
            .verifying_key()
            .as_bytes());

        let err = verify_signature_with_key(&path, &[], &key)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("signature is empty"),
            "unexpected error: {err}"
        );

        assert!(verify_signature_with_key(&path, &[0u8; 63], &key).is_err());
        assert!(verify_signature_with_key(&path, &[0u8; 65], &key).is_err());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn signature_refuses_a_malformed_trust_root() {
        let path = temp_file("sig-key", b"payload");
        let sig = SigningKey::from_bytes(&[5u8; 32])
            .sign(b"payload")
            .to_bytes();

        // Not base64.
        assert!(verify_signature_with_key(&path, &sig, "!!!not base64!!!").is_err());
        // Valid base64, wrong length — this is the shape of the old placeholder key.
        let placeholder =
            "dW5kZWZpbmVkLXB1YmxpYy1rZXktZm9yLWRldmVsb3BtZW50LW9ubHktcGxhY2Vob2xkZXI=";
        let err = verify_signature_with_key(&path, &sig, placeholder)
            .unwrap_err()
            .to_string();
        assert!(err.contains("expected 32"), "unexpected error: {err}");

        let _ = std::fs::remove_file(&path);
    }

    /// Interop: a signature produced by **OpenSSL** must verify here.
    ///
    /// The release pipeline signs with `openssl pkeyutl -sign -rawin`
    /// (`.github/workflows/build-release.yml`), and this module verifies with
    /// `ed25519-dalek`. Both implement RFC 8032, but "should be compatible" is
    /// a claim, not a check — and if it were wrong, every release would be
    /// signed by CI and refused by every client, with nothing failing until a
    /// user tried to update.
    ///
    /// Fixed vector, generated once with a throwaway key that was then
    /// destroyed. The private half never existed outside a scratch directory;
    /// a public key and a signature over known bytes reveal nothing.
    #[test]
    fn a_signature_made_by_openssl_verifies_here() {
        const PUBKEY_B64: &str = "GuDUbKepVLIPKmkNoIcFAnQKSGh8Q95K8iaEYVk2jlI=";
        const SIG_B64: &str = "mkPDDpIGiY6uPcwbkJX0+zAOyCTqh22MGma/chFh0C9sOeQiR0liNwYtylweD4iZ9XcJy0JrWr5RGFONH/wFCQ==";
        const PAYLOAD: &[u8] = b"arcana test artifact";

        let sig = base64::engine::general_purpose::STANDARD
            .decode(SIG_B64)
            .expect("decode signature");
        assert_eq!(sig.len(), 64, "openssl must emit a raw 64-byte signature");

        let path = temp_file("openssl-interop", PAYLOAD);
        assert!(
            verify_signature_with_key(&path, &sig, PUBKEY_B64).is_ok(),
            "an OpenSSL-produced Ed25519 signature must verify with ed25519-dalek —              if this fails, CI signs every release and every client refuses it"
        );

        // And the same signature must NOT verify over different bytes.
        let tampered = temp_file("openssl-interop-tampered", b"arcana test artifactX");
        assert!(verify_signature_with_key(&tampered, &sig, PUBKEY_B64).is_err());

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&tampered);
    }

    // ── verify_update composition ────────────────────────────────────────────

    #[tokio::test]
    async fn verify_update_refuses_an_absent_signature_url() {
        let payload = b"installer";
        let path = temp_file("upd-nosig", payload);
        let checksum = sha256_hex(payload);

        // Correct checksum, no signature URL — must still refuse. This is the
        // second of the three bypasses: `if !sig_url.is_empty()` used to skip
        // the entire Ed25519 branch.
        let err = verify_update(&path, &checksum, "")
            .await
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("no signature was published"),
            "unexpected error: {err}"
        );
        assert!(verify_update(&path, &checksum, "   ").await.is_err());

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn verify_update_refuses_a_plaintext_signature_url() {
        let payload = b"installer";
        let path = temp_file("upd-http", payload);
        let checksum = sha256_hex(payload);

        assert!(verify_update(&path, &checksum, "http://example.com/a.sig")
            .await
            .is_err());

        let _ = std::fs::remove_file(&path);
    }
}
