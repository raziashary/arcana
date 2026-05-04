// Auto-updater delta patch module.
// Applies binary delta patches to produce an updated installer.
// Strategy: try delta patch first, fall back to full download on any error.
//
// The patch format is a simple concatenation of:
//   [4 bytes: magic "ADEV"] [4 bytes: version u32 LE] [bsdiff payload]
//
// For v1.0 we implement the full-download path; delta support is scaffolded
// and will be completed once the release server provides .patch assets.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"ADEV";
const PATCH_VERSION: u32 = 1;

/// Apply a delta patch file to `old_path`, writing the result to `output_path`.
/// Returns `Err` if the patch is invalid or the magic doesn't match — the
/// caller should fall back to a full download in that case.
pub fn apply_patch(
    old_path: &Path,
    patch_path: &Path,
    output_path: &Path,
) -> Result<()> {
    let patch_data = std::fs::read(patch_path)
        .with_context(|| format!("Cannot read patch file: {}", patch_path.display()))?;

    // Validate magic header
    if patch_data.len() < 8 {
        anyhow::bail!("Patch file too small ({}B)", patch_data.len());
    }
    if &patch_data[..4] != MAGIC {
        anyhow::bail!("Invalid patch magic — expected {:?}", MAGIC);
    }
    let patch_ver = u32::from_le_bytes(patch_data[4..8].try_into().unwrap());
    if patch_ver != PATCH_VERSION {
        anyhow::bail!("Unsupported patch version {patch_ver} (expected {PATCH_VERSION})");
    }

    let old_data = std::fs::read(old_path)
        .with_context(|| format!("Cannot read old binary: {}", old_path.display()))?;

    // Apply bsdiff payload (bytes 8..)
    let payload = &patch_data[8..];
    let new_data = apply_bsdiff(old_data, payload)
        .context("bsdiff patch application failed")?;

    std::fs::write(output_path, &new_data)
        .with_context(|| format!("Cannot write patched output: {}", output_path.display()))?;

    Ok(())
}

/// Minimal bsdiff reader/applier (pure Rust, no C dependency).
/// Implements the bsdiff4 wire format:
///   - 3 bzip2 streams: control data, diff data, extra data
///   - Control tuples: (x, y, z) — add x bytes, copy y bytes from extra, skip z bytes
fn apply_bsdiff(old: Vec<u8>, patch: &[u8]) -> Result<Vec<u8>> {
    use std::io::Read;

    if patch.len() < 32 {
        anyhow::bail!("bsdiff patch too short");
    }

    // Header: "BSDIFF40" + new_size (8 bytes LE) + ctrl_len (8 bytes LE) + diff_len (8 bytes LE)
    if &patch[..8] != b"BSDIFF40" {
        anyhow::bail!("Not a bsdiff4 patch (invalid magic)");
    }
    let new_size  = u64::from_le_bytes(patch[ 8..16].try_into()?) as usize;
    let ctrl_len  = u64::from_le_bytes(patch[16..24].try_into()?) as usize;
    let diff_len  = u64::from_le_bytes(patch[24..32].try_into()?) as usize;

    let patch_body = &patch[32..];
    if patch_body.len() < ctrl_len + diff_len {
        anyhow::bail!("bsdiff patch body too short");
    }

    // Decompress three bzip2 streams
    let ctrl_raw  = bzip2_decompress(&patch_body[..ctrl_len])?;
    let diff_raw  = bzip2_decompress(&patch_body[ctrl_len..ctrl_len + diff_len])?;
    let extra_raw = bzip2_decompress(&patch_body[ctrl_len + diff_len..])?;

    let mut new_buf = vec![0u8; new_size];
    let mut old_pos = 0i64;
    let mut new_pos = 0i64;
    let mut ctrl_reader = ctrl_raw.as_slice();
    let mut diff_pos = 0usize;
    let mut extra_pos = 0usize;

    let read_i64 = |r: &mut &[u8]| -> Result<i64> {
        let mut buf = [0u8; 8];
        r.read_exact(&mut buf).context("ctrl read error")?;
        let v = u64::from_le_bytes(buf) as i64;
        Ok(v)
    };

    while new_pos < new_size as i64 {
        let x = read_i64(&mut ctrl_reader)?; // add x bytes from diff + old
        let y = read_i64(&mut ctrl_reader)?; // copy y bytes from extra
        let z = read_i64(&mut ctrl_reader)?; // skip z bytes in old

        // Add diff to old data
        for i in 0..x as usize {
            let diff_byte  = if diff_pos < diff_raw.len() { diff_raw[diff_pos] } else { 0 };
            let old_byte   = if old_pos >= 0 && (old_pos as usize) < old.len() {
                old[old_pos as usize]
            } else { 0 };
            new_buf[new_pos as usize] = diff_byte.wrapping_add(old_byte);
            diff_pos  += 1;
            old_pos   += 1;
            new_pos   += 1;
        }

        // Copy extra bytes
        for i in 0..y as usize {
            if new_pos as usize >= new_size { break; }
            new_buf[new_pos as usize] = if extra_pos < extra_raw.len() {
                extra_raw[extra_pos]
            } else { 0 };
            extra_pos += 1;
            new_pos   += 1;
        }

        old_pos += z;
    }

    Ok(new_buf)
}

/// Decompress bzip2 data (uses flate2's bzip2 support or fallback).
/// AutoDev's Cargo.toml includes `flate2 = "1"` which covers only gzip/deflate.
/// For bzip2, we'd need `bzip2 = "0.4"`. For now, stub with a passthrough that
/// returns the raw bytes — full bsdiff support requires adding the bzip2 crate.
fn bzip2_decompress(data: &[u8]) -> Result<Vec<u8>> {
    // TODO: Add `bzip2 = "0.4"` to Cargo.toml and implement proper decompression.
    // For now return raw data (works if patch was built without compression).
    Ok(data.to_vec())
}

/// Create an ADEV-format patch file from two binary files.
/// Used by the release automation tooling to generate delta patches.
pub fn create_patch(old_path: &Path, new_path: &Path, output_path: &Path) -> Result<()> {
    // Header
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&PATCH_VERSION.to_le_bytes());

    // Embed a simple "replace all" patch using bsdiff4 format header
    let new_data = std::fs::read(new_path)?;
    let old_data = std::fs::read(old_path)?;

    // For now: emit a no-op bsdiff4 header with just extra (new) data.
    // A proper implementation would compute the actual binary diff.
    let new_size = new_data.len() as u64;
    let ctrl_data: Vec<u8> = {
        // One control tuple: (0, new_size, 0) — copy all from extra
        let mut c = b"BSDIFF40".to_vec();
        c.extend_from_slice(&new_size.to_le_bytes()); // new_size
        c.extend_from_slice(&0u64.to_le_bytes());      // ctrl_len (raw, no ctrl)
        c.extend_from_slice(&0u64.to_le_bytes());      // diff_len
        c
    };

    out.extend_from_slice(&ctrl_data);
    out.extend_from_slice(&new_data);

    std::fs::write(output_path, &out)?;
    Ok(())
}

/// Decide whether to use delta or full download.
/// Returns true if delta is available and the old installer exists.
pub fn should_use_delta(old_installer: Option<&Path>, delta_available: bool) -> bool {
    if !delta_available { return false; }
    if let Some(old) = old_installer {
        return old.exists();
    }
    false
}

/// Path where the previous installer version is cached.
pub fn cached_installer_path(version: &str, platform_key: &str) -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let mut p = PathBuf::from(home).join(".autodev").join("updates").join("cache");
    p.push(format!("{version}-{platform_key}"));
    p
}
