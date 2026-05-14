// Doc 08 Phase 3 — log retention policy.
//
// `tracing-appender::rolling::daily` keeps creating files forever; nothing
// in the appender prunes old logs. After a few months of continuous use
// `~/.autodev/logs/` accumulates hundreds of MB of old daily files. This
// module sweeps logs older than the configured retention cap exactly once
// at host startup.
//
// Configuration:
//   • Default cap: 14 days (matches the value documented in
//     docs/development/LOGGING.md).
//   • Override: `AUTODEV_LOG_RETENTION_DAYS=<n>` environment variable.
//     `n == 0` disables pruning entirely (useful for debugging / when an
//     ops team takes over rotation via external tooling).
//   • A one-line `tracing::info!` summarises the work — files dropped and
//     bytes reclaimed — so an ops audit can confirm retention is alive.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use tracing::{debug, info, warn};

const DEFAULT_RETENTION_DAYS: u64 = 14;
const ENV_VAR: &str = "AUTODEV_LOG_RETENTION_DAYS";

/// Prune log files older than the configured retention cap. Idempotent and
/// safe to call once per process at startup.
pub fn prune(log_dir: &Path) {
    let retention_days = retention_days_from_env();
    if retention_days == 0 {
        info!("[log_retention] disabled via {ENV_VAR}=0 — no pruning performed");
        return;
    }

    let cutoff = match SystemTime::now()
        .checked_sub(Duration::from_secs(retention_days * 24 * 60 * 60))
    {
        Some(t) => t,
        None    => {
            warn!("[log_retention] cutoff math overflowed; skipping prune");
            return;
        }
    };

    let entries = match std::fs::read_dir(log_dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("[log_retention] cannot read {:?}: {e} — skipping prune", log_dir);
            return;
        }
    };

    let mut dropped: u64 = 0;
    let mut bytes:   u64 = 0;
    let mut errors:  u64 = 0;

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_autodev_log(&path) { continue; }

        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => { errors += 1; continue; }
        };
        let modified = match meta.modified() {
            Ok(t) => t,
            Err(_) => { errors += 1; continue; }
        };
        if modified >= cutoff { continue; }

        let size = meta.len();
        match std::fs::remove_file(&path) {
            Ok(_) => {
                dropped += 1;
                bytes   += size;
                debug!("[log_retention] dropped {:?} ({} bytes)", path, size);
            }
            Err(e) => {
                warn!("[log_retention] failed to drop {:?}: {e}", path);
                errors += 1;
            }
        }
    }

    info!(
        "[log_retention] policy={retention_days}d → dropped {dropped} file(s) \
         reclaiming {bytes} bytes ({} error(s))",
        errors,
    );
}

/// Returns the configured retention in days. Invalid env values fall back
/// to the default rather than crashing the host.
fn retention_days_from_env() -> u64 {
    match std::env::var(ENV_VAR) {
        Ok(v) => match v.parse::<u64>() {
            Ok(n)  => n,
            Err(_) => {
                warn!(
                    "[log_retention] {ENV_VAR}='{v}' is not an integer — using \
                     default {DEFAULT_RETENTION_DAYS}d"
                );
                DEFAULT_RETENTION_DAYS
            }
        },
        Err(_) => DEFAULT_RETENTION_DAYS,
    }
}

/// Matches the filename pattern `tracing-appender` produces for the
/// `RollingFileAppender::builder().filename_prefix("autodev-").filename_suffix(".log")`
/// configuration in logging.rs. Conservative: ignores any file that doesn't
/// start with `autodev-` so a stray user file in the dir is never deleted.
fn is_autodev_log(path: &PathBuf) -> bool {
    let name = match path.file_name().and_then(|s| s.to_str()) {
        Some(n) => n,
        None    => return false,
    };
    name.starts_with("autodev-") && (name.ends_with(".log") || name.contains(".log."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_canonical_log_filenames() {
        assert!(is_autodev_log(&PathBuf::from("autodev-2026-05-14.log")));
        assert!(is_autodev_log(&PathBuf::from("autodev-.2026-05-14..log")));
        assert!(is_autodev_log(&PathBuf::from("/tmp/autodev-2026-05-14.log")));
        // Negative cases — must never delete these
        assert!(!is_autodev_log(&PathBuf::from("user-notes.md")));
        assert!(!is_autodev_log(&PathBuf::from("autodev.log")));
        assert!(!is_autodev_log(&PathBuf::from("autodev_2026.log")));
    }

    #[test]
    fn defaults_to_14_days_with_unset_env() {
        std::env::remove_var(ENV_VAR);
        assert_eq!(retention_days_from_env(), 14);
    }
}
