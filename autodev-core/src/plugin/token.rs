// Setup token: one-click worker enrollment.
// Tokens are base64-encoded JSON with a HMAC-SHA256 signature (using the
// fleet secret stored in the app vault / config). They encode the fleet ID,
// worker type, allowed repos, and an expiry timestamp.
//
// For the first-party use case (AIRS Foundry internal fleet) the secret is
// simply a random 32-byte key stored in ~/.autodev/fleet_secret. For multi-tenant
// deployments the secret would come from the AIRS backend API.

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};

use super::manifest::{SetupToken, TokenClaims};

// ─── Token format ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct TokenPayload {
    fleet_id: String,
    worker_type: String,
    repos: Vec<String>,
    expires_at: u64, // Unix timestamp
    issued_at: u64,  // Unix timestamp
    nonce: String,
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Generate a new setup token.
pub fn generate_token(
    fleet_id: &str,
    worker_type: &str,
    repos: &[String],
    ttl_hours: u64,
) -> Result<SetupToken> {
    let secret = load_or_create_fleet_secret()?;
    let now = unix_now();
    let expires_at = now + ttl_hours * 3600;

    let payload = TokenPayload {
        fleet_id: fleet_id.to_string(),
        worker_type: worker_type.to_string(),
        repos: repos.to_vec(),
        expires_at,
        issued_at: now,
        nonce: random_hex(8),
    };

    let payload_json = serde_json::to_string(&payload)?;
    let payload_b64 = URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
    let sig = hmac_sha256(&secret, payload_b64.as_bytes());
    let token = format!("{payload_b64}.{sig}");

    let one_liner = build_one_liner(&token, worker_type);
    let expires_iso = iso8601(expires_at);

    Ok(SetupToken {
        token,
        worker_type: worker_type.to_string(),
        expires_at: expires_iso,
        one_liner,
    })
}

/// Validate a setup token. Returns the decoded claims or an error.
pub fn validate_token(token: &str) -> Result<TokenClaims> {
    let secret = load_or_create_fleet_secret()?;

    let (payload_b64, sig) = token
        .rsplit_once('.')
        .ok_or_else(|| anyhow!("Invalid token format"))?;

    // Verify signature
    let expected_sig = hmac_sha256(&secret, payload_b64.as_bytes());
    if !constant_time_eq(sig.as_bytes(), expected_sig.as_bytes()) {
        return Err(anyhow!("Token signature invalid"));
    }

    // Decode payload
    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .context("Token payload is not valid base64")?;
    let payload: TokenPayload =
        serde_json::from_slice(&payload_bytes).context("Token payload JSON is invalid")?;

    // Check expiry
    if unix_now() > payload.expires_at {
        return Err(anyhow!("Token has expired"));
    }

    Ok(TokenClaims {
        fleet_id: payload.fleet_id,
        worker_type: payload.worker_type,
        repos: payload.repos,
        expires_at: iso8601(payload.expires_at),
        issued_at: iso8601(payload.issued_at),
    })
}

// ─── Fleet secret ─────────────────────────────────────────────────────────────

fn fleet_secret_path() -> Result<std::path::PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| anyhow!("Cannot determine home directory"))?;
    Ok(std::path::PathBuf::from(home)
        .join(".autodev")
        .join("fleet_secret"))
}

fn load_or_create_fleet_secret() -> Result<Vec<u8>> {
    let path = fleet_secret_path()?;
    if path.exists() {
        let hex = std::fs::read_to_string(&path)?;
        hex_decode(hex.trim())
    } else {
        // Generate a new 32-byte secret
        let secret = random_bytes(32);
        let hex = hex_encode(&secret);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &hex)?;
        // Set restrictive permissions on Unix
        #[cfg(not(target_os = "windows"))]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(secret)
    }
}

// ─── One-liner builder ────────────────────────────────────────────────────────

fn build_one_liner(token: &str, worker_type: &str) -> String {
    // The one-liner downloads setup.sh (or setup.ps1) from the GitHub release
    // and passes the token as an argument.
    let base = "https://raw.githubusercontent.com/AIRS-Foundry/autodev/main/src-cli";
    format!("curl -fsSL {base}/setup.sh | bash -s -- --token \"{token}\" --type \"{worker_type}\"")
}

// ─── HMAC-SHA256 (pure-Rust, no external dep) ────────────────────────────────
// A minimal HMAC-SHA256 implementation so we don't need the `hmac` / `sha2`
// crates at the Tauri layer. This is sufficient for self-signed fleet tokens.

fn hmac_sha256(key: &[u8], data: &[u8]) -> String {
    // Pad / hash key
    let k = if key.len() > 64 {
        sha256(key)
    } else {
        key.to_vec()
    };
    let mut k_padded = [0u8; 64];
    k_padded[..k.len()].copy_from_slice(&k);

    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5Cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k_padded[i];
        opad[i] ^= k_padded[i];
    }

    let mut inner = ipad.to_vec();
    inner.extend_from_slice(data);
    let inner_hash = sha256(&inner);

    let mut outer = opad.to_vec();
    outer.extend_from_slice(&inner_hash);
    let result = sha256(&outer);
    hex_encode(&result)
}

/// Minimal SHA-256 — enough for token signing.
/// Uses the standard SHA-256 constants / schedule.
fn sha256(data: &[u8]) -> Vec<u8> {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    // Pre-processing: add padding
    let msg_len = data.len();
    let bit_len = (msg_len as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    // Process each 512-bit chunk
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] =
            [h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = Vec::with_capacity(32);
    for word in &h {
        out.extend_from_slice(&word.to_be_bytes());
    }
    out
}

// ─── Tiny utilities ───────────────────────────────────────────────────────────

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn iso8601(secs: u64) -> String {
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    let y400 = days / 146097;
    let rem = days % 146097;
    let y100 = rem / 36524;
    let rem = rem % 36524;
    let y4 = rem / 1461;
    let rem = rem % 1461;
    let y1 = rem / 365;
    let year = y400 * 400 + y100 * 100 + y4 * 4 + y1 + 1970;
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let mdays: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut doy = rem % 365 + if y1 == 4 { 365 } else { 0 };
    let mut mo = 1u64;
    for md in &mdays {
        if doy < *md {
            break;
        }
        doy -= md;
        mo += 1;
    }
    let day = doy + 1;
    format!("{year}-{mo:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

fn random_bytes(n: usize) -> Vec<u8> {
    // Use the system RNG via std::collections::HashMap's random state seed trick
    // or just read /dev/urandom on Unix, or CSPRNG on Windows via platform API.
    // For simplicity, combine process ID + thread ID + system time.
    let mut seed = unix_now() ^ (std::process::id() as u64).wrapping_mul(6364136223846793005);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        out.push((seed >> 33) as u8);
    }
    out
}

fn random_hex(bytes: usize) -> String {
    hex_encode(&random_bytes(bytes))
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(s: &str) -> Result<Vec<u8>> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| anyhow!("Invalid hex: {e}")))
        .collect()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
