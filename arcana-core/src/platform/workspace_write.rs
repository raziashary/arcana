//! Cooperative host-write serialization, shared by Arcana processes.
//!
//! An OS file lock coordinates managed writers; it cannot prevent external
//! editors. Staged copy-back must independently detect/preserve their changes.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

/// The file handle owns the lock. Dropping a waiting future or this lease
/// releases its handle; lock files are retained to avoid inode replacement.
pub(crate) struct WorkspaceWriteLease {
    _file: File,
}

impl WorkspaceWriteLease {
    pub(crate) async fn acquire(workspace: &Path) -> io::Result<Self> {
        Self::acquire_with_deadline(workspace, Duration::from_secs(30)).await
    }

    async fn acquire_with_deadline(workspace: &Path, timeout: Duration) -> io::Result<Self> {
        let identity = super::workspace::workspace_identity(workspace)
            .map_err(|_| io::Error::other("Cannot identify selected workspace"))?;
        let directory = crate::db::arcana_dir()
            .map_err(|_| io::Error::other("Cannot resolve Arcana write-lease storage"))?
            .join("workspace-write-locks");
        std::fs::create_dir_all(&directory)?;
        let metadata = std::fs::symlink_metadata(&directory)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::other(
                "Write-lease directory is not an ordinary directory",
            ));
        }
        let path = directory.join(format!("{:x}.lock", Sha256::digest(identity.as_bytes())));
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // Open the reparse point itself, then reject it below.
            options.custom_flags(0x0020_0000);
        }
        let file = options.open(&path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || std::fs::symlink_metadata(&path)?.file_type().is_symlink() {
            return Err(io::Error::other(
                "Write-lease handle is not an ordinary file",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.nlink() != 1 {
                return Err(io::Error::other("Write-lease file has multiple links"));
            }
        }
        let started = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(std::fs::TryLockError::WouldBlock) if started.elapsed() < timeout => {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "Workspace writer is busy",
                    ));
                }
                Err(std::fs::TryLockError::Error(error)) => return Err(error),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn write_lease_coordinates_distinct_processes() {
        let home = tempfile::tempdir().expect("isolated home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = home.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        let lease = WorkspaceWriteLease::acquire(&workspace).await.unwrap();
        let run_child = |expected: &str| {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "platform::workspace_write::tests::write_lease_child",
                    "--ignored",
                    "--test-threads=1",
                ])
                .env("USERPROFILE", home.path())
                .env("HOME", home.path())
                .env("ARCANA_TEST_WRITE_LEASE_WORKSPACE", &workspace)
                .env("ARCANA_TEST_WRITE_LEASE_EXPECTED", expected);
            for key in [
                "GEMINI_API_KEY",
                "GOOGLE_API_KEY",
                "OPENAI_API_KEY",
                "ANTHROPIC_API_KEY",
                "DATABASE_URL",
            ] {
                command.env_remove(key);
            }
            let output = command.output().expect("isolated lock contender");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
        };
        run_child("busy");
        drop(lease);
        run_child("acquired");
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "invoked only by the isolated cross-process lease test"]
    async fn write_lease_child() {
        let workspace =
            std::env::var_os("ARCANA_TEST_WRITE_LEASE_WORKSPACE").expect("test workspace");
        let expected =
            std::env::var("ARCANA_TEST_WRITE_LEASE_EXPECTED").expect("expected admission");
        let result = WorkspaceWriteLease::acquire_with_deadline(
            Path::new(&workspace),
            Duration::from_millis(60),
        )
        .await;
        match expected.as_str() {
            "busy" => assert_eq!(
                result.err().expect("parent retains lease").kind(),
                io::ErrorKind::TimedOut
            ),
            "acquired" => {
                result.expect("parent released lease");
            }
            _ => panic!("unsupported test admission"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn write_lease_serializes_handles_and_releases_after_drop_or_cancel() {
        let home = tempfile::tempdir().expect("isolated home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = home.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        let lease = WorkspaceWriteLease::acquire(&workspace).await.unwrap();
        let error =
            WorkspaceWriteLease::acquire_with_deadline(&workspace, Duration::from_millis(60))
                .await
                .err()
                .expect("another handle must wait");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        let waiting = WorkspaceWriteLease::acquire(&workspace);
        assert!(tokio::time::timeout(Duration::from_millis(30), waiting)
            .await
            .is_err());
        drop(lease);
        let _next = WorkspaceWriteLease::acquire(&workspace).await.unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn distinct_workspaces_do_not_share_a_write_lease() {
        let home = tempfile::tempdir().expect("isolated home");
        let _home = crate::db::test_support::with_home(home.path());
        let first = home.path().join("first");
        let second = home.path().join("second");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        let _a = WorkspaceWriteLease::acquire(&first).await.unwrap();
        let _b = WorkspaceWriteLease::acquire(&second).await.unwrap();
    }
}
