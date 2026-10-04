//! Stable content fingerprints used to bind build evidence to workspace state.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

const GENERATED_DIRECTORIES: &[&str] = &[
    ".arcana-cache",
    ".arcana-tmp",
    ".git",
    ".mypy_cache",
    ".next",
    ".pytest_cache",
    ".ruff_cache",
    ".turbo",
    ".venv",
    "__pycache__",
    "coverage",
    "dist",
    "node_modules",
    "target",
    "venv",
];

/// Hash workspace inputs in deterministic path order. Generated output and
/// dependency installation trees are excluded so ordinary builds do not
/// invalidate their own evidence; manifests and lockfiles remain included.
/// Symlinks are fingerprinted by their target text and are never traversed.
pub fn fingerprint(root: &Path) -> std::io::Result<String> {
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "workspace root is not a directory",
        ));
    }

    let mut hasher = Sha256::new();
    hasher.update(b"arcana-workspace-fingerprint-v1\0");
    hasher.update(root.to_string_lossy().as_bytes());
    hasher.update([0]);

    for entry in WalkDir::new(&root)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| !is_excluded_generated_directory(entry))
    {
        let entry = entry?;
        if entry.path() == root {
            continue;
        }
        let relative = entry.path().strip_prefix(&root).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "workspace traversal escaped its canonical root",
            )
        })?;
        hash_path(&mut hasher, relative);

        let file_type = entry.file_type();
        if file_type.is_symlink() {
            hasher.update(b"symlink\0");
            let target = std::fs::read_link(entry.path())?;
            hasher.update(target.to_string_lossy().as_bytes());
            hasher.update([0]);
        } else if file_type.is_dir() {
            hasher.update(b"directory\0");
        } else if file_type.is_file() {
            hasher.update(b"file\0");
            let canonical = entry.path().canonicalize()?;
            if !canonical.starts_with(&root) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "workspace file resolves outside its canonical root",
                ));
            }
            let mut file = std::fs::File::open(&canonical)?;
            if !path_still_names_open_file(&file, &canonical)? {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "workspace file changed while its fingerprint was being computed",
                ));
            }
            let before = file.metadata()?;
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let read = file.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
            }
            let after = file.metadata()?;
            if before.len() != after.len()
                || before.modified()? != after.modified()?
                || !path_still_names_open_file(&file, &canonical)?
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "workspace changed while its fingerprint was being computed",
                ));
            }
            hasher.update(after.len().to_le_bytes());
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "workspace contains a non-file system entry",
            ));
        }
        hasher.update([0xff]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(windows)]
fn path_still_names_open_file(file: &std::fs::File, path: &Path) -> std::io::Result<bool> {
    let current_path_file = std::fs::File::open(path)?;
    Ok(crate::platform::file_identity::windows_file_identity(file)?
        == crate::platform::file_identity::windows_file_identity(&current_path_file)?)
}

#[cfg(unix)]
fn path_still_names_open_file(file: &std::fs::File, path: &Path) -> std::io::Result<bool> {
    use std::os::unix::fs::MetadataExt;

    let handle = file.metadata()?;
    let current_path = std::fs::metadata(path)?;
    Ok(handle.dev() == current_path.dev() && handle.ino() == current_path.ino())
}

#[cfg(not(any(unix, windows)))]
fn path_still_names_open_file(file: &std::fs::File, path: &Path) -> std::io::Result<bool> {
    let handle = file.metadata()?;
    let current_path = std::fs::metadata(path)?;
    Ok(handle.len() == current_path.len() && handle.modified()? == current_path.modified()?)
}

fn is_excluded_generated_directory(entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| GENERATED_DIRECTORIES.contains(&name))
}

fn hash_path(hasher: &mut Sha256, path: &Path) {
    let normalized = PathBuf::from(path).to_string_lossy().replace('\\', "/");
    hasher.update(normalized.as_bytes());
    hasher.update([0]);
}

#[cfg(test)]
mod tests {
    use super::fingerprint;

    #[test]
    fn fingerprint_tracks_workspace_inputs_but_ignores_generated_output() {
        let root = std::env::temp_dir().join(format!(
            "arcana-workspace-fingerprint-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::create_dir_all(root.join("__pycache__")).unwrap();
        std::fs::create_dir_all(root.join(".venv")).unwrap();
        std::fs::write(root.join("Cargo.lock"), "lock-a").unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub fn value() -> u8 { 1 }\n").unwrap();

        let initial = fingerprint(&root).unwrap();
        std::fs::write(root.join("target/cache"), "first build output").unwrap();
        std::fs::write(root.join("__pycache__/module.pyc"), "bytecode").unwrap();
        std::fs::write(root.join(".venv/installed.txt"), "environment").unwrap();
        assert_eq!(initial, fingerprint(&root).unwrap());

        std::fs::write(root.join("src/lib.rs"), "pub fn value() -> u8 { 2 }\n").unwrap();
        assert_ne!(initial, fingerprint(&root).unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }
}
