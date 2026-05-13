// Auto-updater download module.
// Downloads update packages (full or delta) with progress tracking.
// Progress is reported via Tauri events: "updater:progress".

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
    use_delta: bool,
    progress_tx: tokio::sync::mpsc::Sender<DownloadProgress>,
) -> Result<PathBuf> {
    // Prefer delta if available and requested
    let (url, total_bytes) = if use_delta && asset.delta_url.is_some() {
        (
            asset.delta_url.as_deref().unwrap(),
            asset.delta_size.unwrap_or(asset.full_size),
        )
    } else {
        (asset.full_url.as_str(), asset.full_size)
    };

    let dest = download_dir()?.join(filename_from_url(url));

    // Resume if partially downloaded
    let existing_size = tokio::fs::metadata(&dest)
        .await
        .map(|m| m.len())
        .unwrap_or(0);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
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
pub async fn download_signature(sig_url: &str) -> Result<Vec<u8>> {
    if sig_url.is_empty() {
        return Ok(vec![]);
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("AutoDev auto-updater")
        .build()?;
    let bytes = client.get(sig_url).send().await?.bytes().await?;
    Ok(bytes.to_vec())
}

/// Download the SHA-256 checksum sidecar file.
pub async fn download_checksum(checksum_url: &str) -> Result<String> {
    if checksum_url.is_empty() {
        return Ok(String::new());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("AutoDev auto-updater")
        .build()?;
    let text = client.get(checksum_url).send().await?.text().await?;
    // Format: "sha256hex  filename"
    Ok(text.split_whitespace().next().unwrap_or("").to_string())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

pub fn download_dir() -> Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".autodev").join("updates");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn filename_from_url(url: &str) -> String {
    url.split('/').last().unwrap_or("autodev-update").to_string()
}
