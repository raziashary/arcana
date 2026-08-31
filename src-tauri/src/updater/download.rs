// Auto-updater download module.
// Downloads update packages with progress tracking.
// Progress is reported via Tauri events: "updater:progress".
//
// Two safety rules are enforced here rather than at the call sites, because a
// call site that forgets them is silent:
//   • every update endpoint must be HTTPS (`require_https`)
//   • the on-disk filename is DERIVED by allow-list, never taken from the URL
//     (`safe_filename`) — see ADR-LOG-RESTRUCT-0004

use anyhow::{Context, Result};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

use super::check::PlatformAsset;

/// Progress snapshot sent to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub version:         String,
    pub bytes_downloaded: u64,
    pub total_bytes:     u64,
    pub percent:         f32,
    pub speed_bps:       u64,
    pub stage:           DownloadStage,
    pub path:            Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DownloadStage {
    Pending,
    Downloading,
    Verifying,
    Ready,
    Failed,
}

/// Shared download state — stored in Tauri managed state.
#[derive(Debug, Default)]
pub struct DownloadState {
    pub progress: tokio::sync::Mutex<Option<DownloadProgress>>,
}

/// Download the installer for a given version.
/// Returns the path to the downloaded file.
pub async fn download_update(
    version: &str,
    asset: &PlatformAsset,
    progress_tx: tokio::sync::mpsc::Sender<DownloadProgress>,
) -> Result<PathBuf> {
    let url = asset.full_url.as_str();
    let total_bytes = asset.full_size;

    require_https(url).context("Installer URL rejected")?;

    let dest = download_dir()?.join(safe_filename(url));

    // Resume if partially downloaded. This is only sound because the checksum
    // is now mandatory (verify.rs): a stale or corrupt partial file that is
    // appended to will fail verification rather than being installed.
    let existing_size = tokio::fs::metadata(&dest)
        .await
        .map(|m| m.len())
        .unwrap_or(0);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .https_only(true)
        .user_agent("AutoDev auto-updater")
        .build()?;

    let mut request = client.get(url);
    if existing_size > 0 {
        request = request.header("Range", format!("bytes={}-", existing_size));
    }

    let resp = request.send().await.context("Download request failed")?;
    let status = resp.status();

    // 206 = partial content (resume), 200 = full
    if !status.is_success() {
        anyhow::bail!("Download server returned {status}");
    }

    let append = status == reqwest::StatusCode::PARTIAL_CONTENT;
    let mut file = if append {
        tokio::fs::OpenOptions::new().append(true).open(&dest).await?
    } else {
        tokio::fs::File::create(&dest).await?
    };

    let mut bytes_downloaded = if append { existing_size } else { 0 };
    let mut stream = resp.bytes_stream();
    let start_time = std::time::Instant::now();

    let _ = progress_tx.send(DownloadProgress {
        version: version.to_string(),
        bytes_downloaded,
        total_bytes,
        percent: 0.0,
        speed_bps: 0,
        stage: DownloadStage::Downloading,
        path: None,
    }).await;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Download stream error")?;
        file.write_all(&chunk).await.context("Write error")?;
        bytes_downloaded += chunk.len() as u64;

        let elapsed = start_time.elapsed().as_secs_f64().max(0.001);
        let speed   = (bytes_downloaded as f64 / elapsed) as u64;
        let percent = if total_bytes > 0 {
            (bytes_downloaded as f32 / total_bytes as f32 * 100.0).min(100.0)
        } else {
            0.0
        };

        let _ = progress_tx.send(DownloadProgress {
            version: version.to_string(),
            bytes_downloaded,
            total_bytes,
            percent,
            speed_bps: speed,
            stage: DownloadStage::Downloading,
            path: None,
        }).await;
    }

    file.flush().await?;
    drop(file);

    // Signal ready for verification
    let _ = progress_tx.send(DownloadProgress {
        version: version.to_string(),
        bytes_downloaded,
        total_bytes,
        percent: 100.0,
        speed_bps: 0,
        stage: DownloadStage::Verifying,
        path: Some(dest.to_string_lossy().into_owned()),
    }).await;

    Ok(dest)
}

/// Download the signature sidecar file (.sig URL → bytes).
///
/// An empty URL is refused rather than returning an empty vec: the caller
/// treats empty bytes as "no signature", and `verify_update` must never be
/// handed a silently-degraded result.
pub async fn download_signature(sig_url: &str) -> Result<Vec<u8>> {
    require_https(sig_url).context("Signature URL rejected")?;
    let client = sidecar_client()?;
    let resp = client.get(sig_url).send().await
        .with_context(|| format!("Signature request failed: {sig_url}"))?;
    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("Signature server returned {status} for {sig_url}");
    }
    let bytes = resp.bytes().await.context("Reading signature body failed")?;
    Ok(bytes.to_vec())
}

/// Download the SHA-256 checksum sidecar file.
///
/// The HTTP status is checked: without it a 404 body becomes the "checksum",
/// which then fails verification with a confusing mismatch instead of a clear
/// "the checksum could not be fetched".
pub async fn download_checksum(checksum_url: &str) -> Result<String> {
    require_https(checksum_url).context("Checksum URL rejected")?;
    let client = sidecar_client()?;
    let resp = client.get(checksum_url).send().await
        .with_context(|| format!("Checksum request failed: {checksum_url}"))?;
    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("Checksum server returned {status} for {checksum_url}");
    }
    let text = resp.text().await.context("Reading checksum body failed")?;
    // Format: "sha256hex  filename"
    let first = text.split_whitespace().next().unwrap_or("").to_string();
    if first.is_empty() {
        anyhow::bail!("Checksum response from {checksum_url} was empty");
    }
    Ok(first)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Short-timeout HTTPS-only client for the small sidecar files.
fn sidecar_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .https_only(true)
        .user_agent("AutoDev auto-updater")
        .build()
        .context("Building HTTP client failed")
}

/// Refuse any update endpoint that is not HTTPS with a real host.
///
/// Enforced in code rather than assumed of the release manifest: the manifest
/// URL itself is overridable (`AUTODEV_UPDATE_URL`), so a plaintext endpoint is
/// reachable without touching the repo.
pub fn require_https(url: &str) -> Result<()> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Refused: empty update URL.");
    }
    let parsed = reqwest::Url::parse(trimmed)
        .with_context(|| format!("Refused: not a valid URL: {trimmed}"))?;

    if parsed.scheme() != "https" {
        anyhow::bail!(
            "Refused: update endpoints must use HTTPS, got '{}': {trimmed}",
            parsed.scheme()
        );
    }
    match parsed.host_str() {
        Some(h) if !h.is_empty() => Ok(()),
        _ => anyhow::bail!("Refused: update URL has no host: {trimmed}"),
    }
}

pub fn download_dir() -> Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".autodev").join("updates");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Filenames we are willing to write, by extension.
const KNOWN_EXTENSIONS: &[&str] = &[
    ".msi", ".exe", ".deb", ".appimage", ".dmg", ".zip", ".gz", ".sig",
];

/// Derive a safe on-disk filename for a download URL.
///
/// This is an ALLOW-LIST, deliberately. The previous implementation was
/// `url.split('/').last()`, which had four separate problems: `\` is not a
/// separator to `split('/')` so `..\..\x.exe` escaped the updates directory on
/// Windows; an ABSOLUTE final segment made `PathBuf::join` discard the base
/// directory entirely (write-anywhere, not merely traversal); query strings
/// survived into the filename producing an illegal `?` on Windows; and the
/// `unwrap_or` fallback was unreachable, so a trailing `/` yielded `""` and made
/// the destination the directory itself.
///
/// Anything not matching the allow-list falls back to a deterministic name
/// derived from the URL, so resume still works and two different URLs cannot
/// collide on one file.
pub fn safe_filename(url: &str) -> String {
    let candidate = reqwest::Url::parse(url.trim())
        .ok()
        .and_then(|u| {
            u.path_segments()
                .and_then(|mut segs| segs.rfind(|s| !s.is_empty()).map(str::to_owned))
        })
        .unwrap_or_default();

    if is_safe_filename(&candidate) {
        return candidate;
    }
    let digest = super::verify::sha256_hex(url.trim().as_bytes());
    format!("autodev-update-{}.bin", &digest[..16])
}

fn is_safe_filename(name: &str) -> bool {
    if name.is_empty() || name.len() > 128 || name.starts_with('.') {
        return false;
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
    {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    KNOWN_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_https_accepts_https_only() {
        assert!(require_https("https://github.com/x/y/releases/a.msi").is_ok());

        assert!(require_https("http://github.com/x/y/a.msi").is_err());
        assert!(require_https("ftp://example.com/a.msi").is_err());
        assert!(require_https("file:///C:/evil.exe").is_err());
        assert!(require_https("").is_err());
        assert!(require_https("   ").is_err());
        assert!(require_https("not a url").is_err());
        // A scheme with no host at all.
        assert!(require_https("https://").is_err());
    }

    #[test]
    fn safe_filename_keeps_ordinary_asset_names() {
        assert_eq!(
            safe_filename("https://example.com/rel/autodev_1.2.3_x64_en-US.msi"),
            "autodev_1.2.3_x64_en-US.msi"
        );
        assert_eq!(
            safe_filename("https://example.com/a/AutoDev-1.2.3.AppImage"),
            "AutoDev-1.2.3.AppImage"
        );
        assert_eq!(
            safe_filename("https://example.com/a/autodev.msi.sig"),
            "autodev.msi.sig"
        );
    }

    /// The invariant, over every shape that used to break out: whatever name
    /// comes back must stay inside the updates directory and carry no separator.
    ///
    /// Each of these produced a path outside the directory, an illegal Windows
    /// filename, or the directory itself under the old `url.split('/').last()`.
    #[test]
    fn safe_filename_never_escapes_the_updates_directory() {
        let dir = PathBuf::from("/base/updates");

        for url in [
            r"https://example.com/..\..\evil.exe",        // Windows separator
            "https://example.com/../../evil.exe",         // POSIX traversal
            r"https://example.com/C:\Users\Public\x.exe", // absolute → join discarded base
            "https://example.com/a.msi?token=abc",        // query string
            "https://example.com/releases/",              // trailing slash → empty
            "https://example.com/..",                     // parent directory
            "https://example.com/a%2F..%2Fevil.exe",      // percent-encoded separator
            "https://example.com/payload.bat",            // extension off the allow-list
        ] {
            let name = safe_filename(url);
            let joined = dir.join(&name);

            assert!(
                joined.starts_with(&dir),
                "{url} escaped the base directory: {joined:?}"
            );
            assert!(
                !name.contains(['/', '\\', ':', '?', '%']),
                "unsafe characters survived in {name:?} (from {url})"
            );
            assert!(!name.is_empty() && name != "." && name != "..", "bad name {name:?}");
        }
    }

    /// Parsing the URL first means WHATWG normalisation has already collapsed
    /// traversal and stripped the query by the time the allow-list runs — so
    /// these resolve to an ordinary, confined filename rather than a fallback.
    /// Asserted explicitly so a future change back to raw string splitting,
    /// which would reintroduce the escape, fails here.
    #[test]
    fn safe_filename_normalises_rather_than_rejecting_where_it_can() {
        assert_eq!(safe_filename(r"https://example.com/..\..\evil.exe"), "evil.exe");
        assert_eq!(safe_filename("https://example.com/../../evil.exe"), "evil.exe");
        assert_eq!(safe_filename(r"https://example.com/C:\Users\Public\x.exe"), "x.exe");
        assert_eq!(safe_filename("https://example.com/a.msi?token=abc"), "a.msi");
    }

    /// The shapes that cannot yield a safe name must fall back, not improvise.
    #[test]
    fn safe_filename_falls_back_when_no_safe_name_exists() {
        for url in [
            "https://example.com/releases/",         // no final segment
            "https://example.com/..",                // dot-segment
            "https://example.com/a%2F..%2Fevil.exe", // percent-encoded separator
            "https://example.com/payload.bat",       // extension off the allow-list
            "https://example.com/no-extension",
        ] {
            let name = safe_filename(url);
            assert!(
                name.starts_with("autodev-update-") && name.ends_with(".bin"),
                "expected a fallback name for {url}, got {name}"
            );
        }
    }

    #[test]
    fn safe_filename_fallback_is_deterministic_and_url_specific() {
        let a = safe_filename("https://example.com/releases/");
        let b = safe_filename("https://example.com/releases/");
        let c = safe_filename("https://example.com/other/");

        assert_eq!(a, b, "same URL must yield the same file so resume works");
        assert_ne!(a, c, "different URLs must not collide on one file");
    }
}
