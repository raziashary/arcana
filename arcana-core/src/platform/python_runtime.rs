//! Stages the selected Windows Python runtime into Arcana-owned storage so
//! AppContainer grants can expose only its exact read/execute tree.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use walkdir::WalkDir;

const PYTEST_PACKAGES: &[&str] = &[
    "_pytest",
    "colorama",
    "iniconfig",
    "packaging",
    "pluggy",
    "pygments",
    "pytest",
    "py",
];

const RUFF_VERSION: &str = "0.16.8";
const RUFF_WINDOWS_X64_WHEEL_URL: &str = "https://files.pythonhosted.org/packages/1e/5d/a509c07d714b6da88f2c518b4637cf6f1d46f1d46b074be8f0f1e5fb9ff5126fe/ruff-0.16.8-py3-none-win_amd64.whl";
const RUFF_WINDOWS_X64_WHEEL_SHA256: &str =
    "a6b85621fd3c81e31fc5f5add09c9c078b430db3595ca632efafdec9e64ebfaa";
const RUFF_WINDOWS_X64_EXECUTABLE_SHA256: &str =
    "22b93aa554f2c3ec969fa83c5d77da3d61a4a54c61cbca70f5eeec63eef28e48";
const RUFF_WHEEL_MAX_BYTES: usize = 12 * 1024 * 1024;
const RUFF_EXECUTABLE_MAX_BYTES: usize = 32 * 1024 * 1024;
const USER_PYTHON_FALLBACK_KEY: &str = "python_user_fallback_executable";

static STAGED_RUNTIME: OnceLock<Mutex<Option<StagedPython>>> = OnceLock::new();

#[derive(Clone, Debug)]
pub(crate) struct StagedPython {
    pub executable: PathBuf,
    pub root: PathBuf,
    source_key: String,
}

#[derive(Deserialize)]
struct HostPython {
    prefix: PathBuf,
    purelib: PathBuf,
    usersite: PathBuf,
    major: u8,
    minor: u8,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    digest: String,
    source_key: String,
}

#[derive(Serialize, Deserialize)]
struct RuffManifest {
    version: String,
    wheel_sha256: String,
    executable_sha256: String,
}

/// Fetch and stage the pinned Ruff executable in Arcana-owned storage.
///
/// This is called only from the exact `project_command(install, python,
/// ruff)` request, after `ToolExecutor` has completed its one-call host
/// approval. Generic project dependency installation remains unavailable.
#[cfg(all(windows, target_arch = "x86_64"))]
pub(crate) async fn install_ruff() -> io::Result<()> {
    use sha2::{Digest, Sha256};

    let started = std::time::Instant::now();
    let result = async {
        let arcana_dir = crate::db::arcana_dir().map_err(|error| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("resolve Arcana tool cache: {error}"),
            )
        })?;
        // Keep the staged tool under the runtime root accepted by the Windows
        // AppContainer's existing exact-tree read grant.
        let tool_root = arcana_dir.join("python-runtimes");
        ensure_real_directory(&tool_root)?;
        let final_root = tool_root.join(format!("ruff-{RUFF_VERSION}-win-x64"));
        if final_root.exists() {
            verify_staged_ruff(&final_root)?;
            return Ok(());
        }

        // Host-side network access is explicit, version-pinned, size-bounded,
        // and hash-verified before the archive is ever opened. The agent's
        // sandboxed lint child receives only the resulting executable.
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(io_other)?;
        let response = client
            .get(RUFF_WINDOWS_X64_WHEEL_URL)
            .send()
            .await
            .map_err(io_other)?;
        if !response.status().is_success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("pinned Ruff download returned HTTP {}", response.status()),
            ));
        }
        let declared_length = response.content_length();
        if declared_length.is_some_and(|length| length > RUFF_WHEEL_MAX_BYTES as u64) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "pinned Ruff wheel exceeded its size limit",
            ));
        }
        use futures::StreamExt;
        let mut stream = response.bytes_stream();
        let mut wheel = Vec::with_capacity(
            declared_length
                .unwrap_or(0)
                .min(RUFF_WHEEL_MAX_BYTES as u64) as usize,
        );
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(io_other)?;
            if wheel.len().saturating_add(chunk.len()) > RUFF_WHEEL_MAX_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "pinned Ruff wheel exceeded its size limit",
                ));
            }
            wheel.extend_from_slice(&chunk);
        }
        let wheel_sha256 = hex::encode(Sha256::digest(&wheel));
        if wheel_sha256 != RUFF_WINDOWS_X64_WHEEL_SHA256 {
            return Err(permission_error(
                "downloaded Ruff wheel did not match the pinned SHA-256",
            ));
        }

        let python = find_python_executable().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Ruff staging requires a signed standard Python installation",
            )
        })?;
        let staging = tempfile::Builder::new()
            .prefix("ruff-stage-")
            .tempdir_in(&tool_root)?;
        let wheel_path = staging.path().join("ruff.whl");
        let executable_path = staging.path().join("ruff.exe");
        std::fs::write(&wheel_path, &wheel)?;
        extract_pinned_windows_ruff(&python, &wheel_path, &executable_path).await?;
        let executable_sha256 = hash_file(&executable_path)?;
        if executable_sha256 != RUFF_WINDOWS_X64_EXECUTABLE_SHA256 {
            return Err(permission_error(
                "extracted Ruff executable did not match its pinned SHA-256",
            ));
        }
        std::fs::write(
            staging.path().join("_arcana-ruff.json"),
            serde_json::to_vec(&RuffManifest {
                version: RUFF_VERSION.to_string(),
                wheel_sha256,
                executable_sha256,
            })
            .map_err(io_other)?,
        )?;
        std::fs::remove_file(&wheel_path)?;
        match std::fs::rename(staging.path(), &final_root) {
            Ok(()) => {
                let _ = staging.keep();
            }
            Err(_error) if final_root.exists() => {
                verify_staged_ruff(&final_root)?;
                drop(staging);
            }
            Err(error) => return Err(error),
        }
        verify_staged_ruff(&final_root)?;
        Ok(())
    }
    .await;

    tracing::info!(
        target: "arcana::telemetry",
        event_type = "python_ruff_install",
        version = RUFF_VERSION,
        duration_ms = started.elapsed().as_millis() as u64,
        outcome = if result.is_ok() { "success" } else { "failure" },
        "Host-approved Ruff runtime staging completed"
    );
    result
}

#[cfg(not(all(windows, target_arch = "x86_64")))]
pub(crate) async fn install_ruff() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Arcana's pinned Ruff wheel is not available for this host architecture",
    ))
}

#[cfg(all(windows, target_arch = "x86_64"))]
pub(crate) fn staged_ruff() -> io::Result<(PathBuf, PathBuf)> {
    let arcana_dir = crate::db::arcana_dir().map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("resolve Arcana tool cache: {error}"),
        )
    })?;
    let root = arcana_dir
        .join("python-runtimes")
        .join(format!("ruff-{RUFF_VERSION}-win-x64"));
    verify_staged_ruff(&root)?;
    Ok((root.join("ruff.exe"), root))
}

#[cfg(not(all(windows, target_arch = "x86_64")))]
pub(crate) fn staged_ruff() -> io::Result<(PathBuf, PathBuf)> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Arcana's pinned Ruff wheel is not available for this host architecture",
    ))
}

#[cfg(all(windows, target_arch = "x86_64"))]
async fn extract_pinned_windows_ruff(
    python: &Path,
    wheel: &Path,
    destination: &Path,
) -> io::Result<()> {
    let script = format!(
        r#"import pathlib,sys,zipfile
wheel=pathlib.Path(sys.argv[1]); destination=pathlib.Path(sys.argv[2])
member='ruff-0.16.8.data/scripts/ruff.exe'
with zipfile.ZipFile(wheel) as archive:
    matches=[entry for entry in archive.infolist() if entry.filename == member]
    if len(matches) != 1 or matches[0].is_dir() or matches[0].file_size > {RUFF_EXECUTABLE_MAX_BYTES}:
        raise SystemExit('wheel does not contain exactly one bounded Ruff executable')
    with archive.open(matches[0]) as source, destination.open('xb') as target:
        while chunk := source.read(1024*1024):
            target.write(chunk)
"#
    );
    let mut command = tokio::process::Command::new(python);
    command
        .args([
            OsStr::new("-B"),
            OsStr::new("-I"),
            OsStr::new("-S"),
            OsStr::new("-c"),
            OsStr::new(&script),
        ])
        .arg(wheel)
        .arg(destination)
        .env_remove("PYTHONHOME")
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONUSERBASE")
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(20), command.output())
        .await
        .map_err(|_| {
            io::Error::new(io::ErrorKind::TimedOut, "Ruff wheel extraction timed out")
        })??;
    if !output.status.success() {
        let _ = std::fs::remove_file(destination);
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "signed Python could not extract the pinned Ruff wheel",
        ));
    }
    Ok(())
}

fn ensure_real_directory(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)?;
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(permission_error(
            "Arcana tool cache must be a real directory",
        ));
    }
    Ok(())
}

fn verify_staged_ruff(root: &Path) -> io::Result<()> {
    use sha2::{Digest, Sha256};

    let metadata = std::fs::symlink_metadata(root)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(permission_error("staged Ruff directory is not trusted"));
    }
    let manifest_path = root.join("_arcana-ruff.json");
    let manifest_metadata = std::fs::symlink_metadata(&manifest_path)?;
    if !manifest_metadata.is_file() || manifest_metadata.file_type().is_symlink() {
        return Err(permission_error(
            "staged Ruff manifest is not a regular file",
        ));
    }
    let manifest: RuffManifest =
        serde_json::from_slice(&std::fs::read(manifest_path)?).map_err(io_other)?;
    if manifest.version != RUFF_VERSION
        || manifest.wheel_sha256 != RUFF_WINDOWS_X64_WHEEL_SHA256
        || manifest.executable_sha256 != RUFF_WINDOWS_X64_EXECUTABLE_SHA256
    {
        return Err(permission_error("staged Ruff manifest is not trusted"));
    }
    let executable = root.join("ruff.exe");
    let executable_metadata = std::fs::symlink_metadata(&executable)?;
    if !executable_metadata.is_file() || executable_metadata.file_type().is_symlink() {
        return Err(permission_error(
            "staged Ruff executable is not a regular file",
        ));
    }
    let digest = hex::encode(Sha256::digest(std::fs::read(executable)?));
    if digest != RUFF_WINDOWS_X64_EXECUTABLE_SHA256 {
        return Err(permission_error(
            "staged Ruff executable failed integrity check",
        ));
    }
    Ok(())
}

fn hash_file(path: &Path) -> io::Result<String> {
    use sha2::{Digest, Sha256};
    Ok(hex::encode(Sha256::digest(std::fs::read(path)?)))
}

fn io_other(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::Other, error.to_string())
}

pub(crate) fn stage() -> io::Result<StagedPython> {
    let cache = STAGED_RUNTIME.get_or_init(|| Mutex::new(None));
    let mut cache = cache
        .lock()
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "Python runtime cache lock poisoned"))?;
    let arcana_dir = crate::db::arcana_dir()
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?;
    let runtime_cache = arcana_dir.join("python-runtimes");
    ensure_real_directory(&runtime_cache)?;
    let runtime_cache = runtime_cache.canonicalize()?;
    if cache
        .as_ref()
        .is_some_and(|runtime| !runtime.root.starts_with(&runtime_cache))
    {
        // A process can serve another profile in tests or an embedded host.
        // Never reuse a verified runtime from a different Arcana data root.
        *cache = None;
    }
    let process_cache_hit = cache.is_some();
    let started = std::time::Instant::now();
    let result: io::Result<StagedPython> = (|| {
        if let Some(runtime) = cache.as_ref() {
            verify_staged_runtime(&runtime.root, &runtime.source_key)?;
            return Ok(runtime.clone());
        }
        let runtime = stage_selected_python()?;
        *cache = Some(runtime.clone());
        Ok(runtime)
    })();
    tracing::info!(
        target: "arcana::telemetry",
        event_type = "python_runtime_prepare",
        process_cache_hit,
        elapsed_ms = started.elapsed().as_millis() as u64,
        outcome = if result.is_ok() { "success" } else { "failed" },
        "Python runtime preparation completed"
    );
    result
}

fn stage_selected_python() -> io::Result<StagedPython> {
    let primary = find_python_executable();
    if let Some(executable) = primary.as_deref() {
        match stage_python_executable(executable) {
            Ok(runtime) => return Ok(runtime),
            Err(primary_error) => {
                let fallback = selected_user_python_fallback()?;
                let Some(fallback) = fallback else {
                    return Err(io::Error::new(
                        primary_error.kind(),
                        format!(
                            "Python staging failed and no user-owned fallback is selected: {}",
                            safe_io_summary(&primary_error)
                        ),
                    ));
                };
                if Some(fallback.as_path()) == primary.as_deref() {
                    return Err(primary_error);
                }
                return stage_python_executable(&fallback).map_err(|fallback_error| {
                    io::Error::new(
                        fallback_error.kind(),
                        format!(
                            "Python staging failed for both the primary and selected fallback ({})",
                            safe_io_summary(&fallback_error)
                        ),
                    )
                });
            }
        }
    }

    let fallback = selected_user_python_fallback()?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Python staging requires a signed interpreter on PATH or an explicitly selected user-owned fallback",
        )
    })?;
    stage_python_executable(&fallback)
}

fn stage_python_executable(source_executable: &Path) -> io::Result<StagedPython> {
    let source_executable = timed_io("python_runtime.validate_interpreter", || {
        validate_signed_python_executable(source_executable)
    })?;
    let host = timed_io("python_runtime.inspect_interpreter", || {
        inspect_python(&source_executable)
    })?;
    let source_root = host.prefix.canonicalize()?;
    if source_executable.parent() != Some(source_root.as_path()) {
        return Err(permission_error(
            "selected python.exe is outside the Python prefix it reported",
        ));
    }
    let source_stdlib = resolve_stdlib_root(&source_root, host.major, host.minor)?;
    let source_dlls = source_root.join("DLLs").canonicalize()?;
    if !source_dlls.starts_with(&source_root) {
        return Err(permission_error(
            "selected Python DLL directory resolves outside its prefix",
        ));
    }
    let purelib = host.purelib.canonicalize().ok();
    let usersite = host.usersite.canonicalize().ok();
    let packages = purelib
        .iter()
        .chain(usersite.iter())
        .find(|path| path.join("pytest").is_dir() && path.join("_pytest").is_dir())
        .cloned()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "pytest is not installed in the selected Python runtime; host-controlled setup is required before sandboxed Python tests can run",
            )
        })?;

    let arcana_dir = crate::db::arcana_dir().map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("resolve Arcana runtime cache: {error}"),
        )
    })?;
    let runtime_cache = arcana_dir.join("python-runtimes");
    std::fs::create_dir_all(&runtime_cache)?;
    if !std::fs::symlink_metadata(&runtime_cache)?.is_dir()
        || std::fs::symlink_metadata(&runtime_cache)?
            .file_type()
            .is_symlink()
    {
        return Err(permission_error(
            "Arcana Python runtime cache must be a real directory",
        ));
    }
    let source_key = timed_io("python_runtime.fingerprint_source", || {
        source_key(&source_root, &source_dlls, &source_stdlib, &packages)
    })?;
    let final_root = runtime_cache.join(format!("python{}{}-{source_key}", host.major, host.minor));
    if final_root.exists() {
        let metadata = std::fs::symlink_metadata(&final_root)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(permission_error(
                "cached Python runtime must be a real directory",
            ));
        }
        timed_io("python_runtime.verify_cached_runtime", || {
            verify_staged_runtime(&final_root, &source_key)
        })?;
        let final_root = final_root.canonicalize()?;
        let executable = final_root.join("python.exe");
        timed_io("python_runtime.probe_cached_runtime", || {
            verify_runtime_probe(&executable, &final_root)
        })?;
        tracing::info!(
            target: "arcana::telemetry",
            event_type = "python_runtime_cache",
            cache = "verified_disk_cache",
            "Verified staged Python runtime cache"
        );
        return Ok(StagedPython {
            executable,
            root: final_root,
            source_key,
        });
    }
    let staging = tempfile::Builder::new()
        .prefix("python-stage-")
        .tempdir_in(&runtime_cache)?;
    let staging_root = staging.path();

    timed_io("python_runtime.copy_interpreter_files", || {
        for entry in std::fs::read_dir(&source_root)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let entry_path = entry.path();
            let extension = entry_path.extension().and_then(OsStr::to_str);
            let name = entry.file_name();
            let selected = name == OsStr::new("python.exe")
                || extension.is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("dll") || extension.eq_ignore_ascii_case("pyd")
                });
            if selected {
                if !kind.is_file() {
                    return Err(permission_error(
                        "Python runtime contains a non-file root component",
                    ));
                }
                copy_file(&entry_path, &staging_root.join(name))?;
            }
        }
        Ok(())
    })?;
    if !staging_root.join("python.exe").is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the selected Python prefix does not contain python.exe",
        ));
    }
    timed_io("python_runtime.copy_dlls", || {
        copy_tree(&source_dlls, &staging_root.join("DLLs"))
    })?;
    timed_io("python_runtime.copy_stdlib", || {
        copy_stdlib_tree(&source_stdlib, &staging_root.join("Lib"))
    })?;

    let staged_site = staging_root.join("Lib").join("site-packages");
    std::fs::create_dir_all(&staged_site)?;
    timed_io("python_runtime.copy_pytest_closure", || {
        copy_pytest_closure(&packages, &staged_site)
    })?;
    let pth = format!(
        "python{}{}.zip\n.\nDLLs\nLib\nLib\\site-packages\n",
        host.major, host.minor
    );
    std::fs::write(
        staging_root.join(format!("python{}{}._pth", host.major, host.minor)),
        pth,
    )?;

    let digest = timed_io("python_runtime.fingerprint_staged_runtime", || {
        hash_tree(staging_root)
    })?;
    std::fs::write(
        staging_root.join("_arcana-runtime.json"),
        serde_json::to_vec(&Manifest {
            digest,
            source_key: source_key.clone(),
        })
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?,
    )?;
    match std::fs::rename(staging_root, &final_root) {
        Ok(()) => {
            let _ = staging.keep();
        }
        Err(_error) if final_root.exists() => {
            timed_io("python_runtime.verify_concurrent_runtime", || {
                verify_staged_runtime(&final_root, &source_key)
            })?;
            drop(staging);
            tracing::debug!(
                target: "arcana::telemetry",
                event_type = "python_runtime_cache_race",
                "Another Arcana process staged the same Python runtime"
            );
        }
        Err(error) => return Err(error),
    }
    let final_root = final_root.canonicalize()?;
    let executable = final_root.join("python.exe");
    timed_io("python_runtime.probe_staged_runtime", || {
        verify_runtime_probe(&executable, &final_root)
    })?;
    tracing::info!(
        target: "arcana::telemetry",
        event_type = "python_runtime_cache",
        cache = "created",
        "Staged selected Python runtime"
    );
    Ok(StagedPython {
        executable,
        root: final_root,
        source_key,
    })
}

fn timed_io<T>(operation: &'static str, action: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    let mut timer = crate::observability::timing::OperationTimer::start(operation);
    let result = action();
    if result.is_ok() {
        timer.succeed();
    }
    result
}

fn safe_io_summary(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::NotFound => "runtime files were not found",
        io::ErrorKind::PermissionDenied => "runtime access was denied",
        io::ErrorKind::InvalidData => "runtime contents failed validation",
        io::ErrorKind::Unsupported => "runtime layout is unsupported",
        _ => "runtime staging could not complete",
    }
}

fn validate_signed_python_executable(executable: &Path) -> io::Result<PathBuf> {
    let canonical = executable.canonicalize()?;
    if canonical
        .file_name()
        .is_none_or(|name| !name.eq_ignore_ascii_case("python.exe"))
    {
        return Err(permission_error(
            "selected Python executable must be named python.exe",
        ));
    }
    let metadata = std::fs::metadata(&canonical)?;
    if !metadata.is_file() {
        return Err(permission_error(
            "selected Python executable is not a regular file",
        ));
    }
    verify_authenticode(&canonical)?;
    Ok(canonical)
}

fn user_python_root() -> io::Result<PathBuf> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| permission_error("LOCALAPPDATA is unavailable"))?;
    PathBuf::from(local_app_data)
        .join("Programs")
        .join("Python")
        .canonicalize()
}

fn validate_user_owned_python_path(
    executable: &Path,
    user_python_root: &Path,
) -> io::Result<PathBuf> {
    let canonical_root = user_python_root.canonicalize()?;
    let canonical = executable.canonicalize()?;
    if !canonical.starts_with(&canonical_root)
        || canonical
            .file_name()
            .is_none_or(|name| !name.eq_ignore_ascii_case("python.exe"))
    {
        return Err(permission_error(
            "fallback Python must be python.exe beneath the current user's Python installation directory",
        ));
    }
    let metadata = std::fs::metadata(&canonical)?;
    if !metadata.is_file() {
        return Err(permission_error(
            "fallback Python executable is not a regular file",
        ));
    }
    Ok(canonical)
}

pub fn get_user_python_fallback() -> io::Result<Option<PathBuf>> {
    let selected = crate::db::app_settings::get(USER_PYTHON_FALLBACK_KEY).map_err(|_| {
        io::Error::new(
            io::ErrorKind::Other,
            "could not read Python fallback setting",
        )
    })?;
    selected
        .map(|path| {
            let path = PathBuf::from(path);
            let validated = validate_user_owned_python_path(&path, &user_python_root()?)?;
            validate_signed_python_executable(&validated)
        })
        .transpose()
}

pub fn set_user_python_fallback(executable: Option<&Path>) -> io::Result<Option<PathBuf>> {
    let selected = executable
        .map(|path| {
            let validated = validate_user_owned_python_path(path, &user_python_root()?)?;
            validate_signed_python_executable(&validated)
        })
        .transpose()?;
    match selected.as_deref() {
        Some(path) => {
            crate::db::app_settings::set(USER_PYTHON_FALLBACK_KEY, &path.to_string_lossy())
                .map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        "could not save Python fallback setting",
                    )
                })?
        }
        None => {
            crate::db::app_settings::delete(USER_PYTHON_FALLBACK_KEY).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::Other,
                    "could not clear Python fallback setting",
                )
            })?;
        }
    }
    invalidate_staged_runtime();
    Ok(selected)
}

fn invalidate_staged_runtime() {
    if let Some(cache) = STAGED_RUNTIME.get() {
        // Settings changes take effect on the next project command even if
        // this process previously staged a different fallback interpreter.
        *cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
}

#[cfg(test)]
pub(crate) fn reset_process_stage_cache_for_test() {
    invalidate_staged_runtime();
}

fn selected_user_python_fallback() -> io::Result<Option<PathBuf>> {
    get_user_python_fallback()
}

fn find_python_executable() -> Option<PathBuf> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join("python.exe"))
        .filter(|path| path.is_file())
        .filter_map(|path| path.canonicalize().ok())
        .find(|path| verify_authenticode(path).is_ok())
}

fn verify_authenticode(path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Security::WinTrust::{
        WinVerifyTrustEx, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0,
        WINTRUST_FILE_INFO, WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE,
        WTD_REVOCATION_CHECK_NONE, WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY, WTD_UI_NONE,
    };

    let mut wide_path = path.as_os_str().encode_wide().collect::<Vec<_>>();
    wide_path.push(0);
    let mut file_info = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: wide_path.as_ptr(),
        ..Default::default()
    };
    let mut data = WINTRUST_DATA {
        cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOCATION_CHECK_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 {
            pFile: &mut file_info,
        },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL,
        ..Default::default()
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    // WinVerifyTrustEx validates the Authenticode chain without launching the
    // candidate executable. Cache-only retrieval keeps interpreter selection
    // from causing network access during command preparation.
    let status = unsafe { WinVerifyTrustEx(std::ptr::null_mut(), &mut action, &mut data) };
    data.dwStateAction = WTD_STATEACTION_CLOSE;
    unsafe {
        WinVerifyTrustEx(std::ptr::null_mut(), &mut action, &mut data);
    }
    if status == 0 {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("Python interpreter has no trusted Authenticode signature ({status:#x})"),
        ))
    }
}

fn resolve_stdlib_root(source_root: &Path, major: u8, minor: u8) -> io::Result<PathBuf> {
    let expected = source_root.join("Lib").canonicalize()?;
    // Some Windows Python distributions expose Lib through a junction into
    // the per-user install tree. Accept that exact versioned target only.
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| permission_error("selected Python Lib resolves outside its prefix"))?;
    let user_owned_lib = PathBuf::from(local_app_data)
        .join("Programs")
        .join("Python")
        .join(format!("Python{major}{minor}"))
        .join("Lib")
        .canonicalize()?;
    validate_stdlib_target(source_root, &expected, &user_owned_lib)?;
    Ok(expected)
}

fn validate_stdlib_target(
    source_root: &Path,
    resolved: &Path,
    user_owned_lib: &Path,
) -> io::Result<()> {
    if resolved.starts_with(source_root) || resolved == user_owned_lib {
        Ok(())
    } else {
        Err(permission_error(
            "selected Python Lib resolves outside its prefix and approved user runtime",
        ))
    }
}

fn inspect_python(executable: &Path) -> io::Result<HostPython> {
    const SCRIPT: &str = "import json,site,sys,sysconfig; print(json.dumps({'prefix':sys.prefix,'purelib':sysconfig.get_path('purelib'),'usersite':site.getusersitepackages(),'major':sys.version_info.major,'minor':sys.version_info.minor}))";
    let output = Command::new(executable)
        .args([
            OsStr::new("-B"),
            OsStr::new("-I"),
            OsStr::new("-S"),
            OsStr::new("-c"),
            OsStr::new(SCRIPT),
        ])
        .env_remove("PYTHONHOME")
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONUSERBASE")
        .output()?;
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "selected Python runtime inspection failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn copy_pytest_closure(source: &Path, destination: &Path) -> io::Result<()> {
    for package in PYTEST_PACKAGES {
        let path = source.join(package);
        if path.exists() {
            copy_tree(&path, &destination.join(package))?;
        }
        let module = source.join(format!("{package}.py"));
        if module.is_file() {
            copy_file(&module, &destination.join(format!("{package}.py")))?;
        }
        let normalized = package.trim_start_matches('_').replace('_', "-");
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name.starts_with(&format!("{normalized}-")) && name.ends_with(".dist-info") {
                copy_tree(&entry.path(), &destination.join(entry.file_name()))?;
            }
        }
    }
    if !destination.join("pytest").is_dir() || !destination.join("_pytest").is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "selected Python runtime has an incomplete pytest installation",
        ));
    }
    Ok(())
}

fn source_key(root: &Path, dlls: &Path, stdlib: &Path, packages: &Path) -> io::Result<String> {
    let mut inputs = vec![dlls.to_path_buf(), stdlib.to_path_buf()];
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let extension = path.extension().and_then(OsStr::to_str);
        if entry.file_name() == OsStr::new("python.exe")
            || extension.is_some_and(|extension| {
                extension.eq_ignore_ascii_case("dll") || extension.eq_ignore_ascii_case("pyd")
            })
        {
            inputs.push(path);
        }
    }
    for package in PYTEST_PACKAGES {
        let path = packages.join(package);
        if path.exists() {
            inputs.push(path);
        }
        let module = packages.join(format!("{package}.py"));
        if module.is_file() {
            inputs.push(module);
        }
        let normalized = package.trim_start_matches('_').replace('_', "-");
        for entry in std::fs::read_dir(packages)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name.starts_with(&format!("{normalized}-")) && name.ends_with(".dist-info") {
                inputs.push(entry.path());
            }
        }
    }
    let mut entries = Vec::new();
    for input in inputs {
        if input.is_file() {
            entries.push(input);
        } else {
            for entry in WalkDir::new(&input)
                .follow_links(false)
                .into_iter()
                .filter_entry(|entry| {
                    if entry.path() == input {
                        return true;
                    }
                    if entry.file_type().is_dir()
                        && (entry.file_name() == OsStr::new("__pycache__")
                            || (input == stdlib
                                && entry.path().strip_prefix(stdlib).is_ok_and(|relative| {
                                    relative.components().next().is_some_and(|component| {
                                        component.as_os_str() == OsStr::new("site-packages")
                                    })
                                })))
                    {
                        return false;
                    }
                    true
                })
            {
                let entry = entry.map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
                if entry.file_type().is_symlink() {
                    return Err(permission_error(
                        "selected Python runtime contains a symlink",
                    ));
                }
                entries.push(entry.path().to_path_buf());
            }
        }
    }
    entries.sort();
    let mut digest = Sha256::new();
    digest.update(b"arcana-python-runtime-layout-v5\0");
    digest.update(root.to_string_lossy().as_bytes());
    for path in entries {
        if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pyc"))
        {
            continue;
        }
        digest.update(path.to_string_lossy().as_bytes());
        digest.update([0]);
        let metadata = std::fs::metadata(&path)?;
        if metadata.is_file() {
            digest.update(b"file\0");
            digest.update(std::fs::read(&path)?);
        } else if metadata.is_dir() {
            digest.update(b"directory\0");
        } else {
            return Err(permission_error(
                "selected Python runtime contains a special file",
            ));
        }
    }
    Ok(hex::encode(digest.finalize()))
}

fn copy_tree(source: &Path, destination: &Path) -> io::Result<()> {
    copy_tree_filtered(source, destination, false)
}

fn copy_stdlib_tree(source: &Path, destination: &Path) -> io::Result<()> {
    copy_tree_filtered(source, destination, true)
}

fn copy_tree_filtered(
    source: &Path,
    destination: &Path,
    exclude_site_packages: bool,
) -> io::Result<()> {
    for entry in WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if entry.path() == source {
                return true;
            }
            let is_directory = entry.file_type().is_dir();
            if is_directory && entry.file_name() == OsStr::new("__pycache__") {
                return false;
            }
            if exclude_site_packages
                && is_directory
                && entry.file_name() == OsStr::new("site-packages")
            {
                return false;
            }
            true
        })
    {
        let entry = entry.map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        validate_relative_path(relative)?;
        if relative.as_os_str().is_empty() {
            if !entry.file_type().is_dir() {
                return Err(permission_error("Python runtime root is not a directory"));
            }
            std::fs::create_dir_all(destination)?;
            continue;
        }
        if entry.file_type().is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pyc"))
        {
            continue;
        }
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(target)?;
        } else if entry.file_type().is_file() {
            copy_file(entry.path(), &target)?;
        } else {
            return Err(permission_error(
                "Python runtime contains a symlink or special file",
            ));
        }
    }
    Ok(())
}

fn copy_file(source: &Path, destination: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(source)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(permission_error(
            "Python runtime contains a symlink or non-file component",
        ));
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(source, destination)?;
    Ok(())
}

fn validate_relative_path(path: &Path) -> io::Result<()> {
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(permission_error(
            "Python runtime path escapes its selected source tree",
        ));
    }
    Ok(())
}

fn hash_tree(root: &Path) -> io::Result<String> {
    let mut entries = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .map(|entry| entry.map_err(|error| io::Error::new(io::ErrorKind::Other, error)))
        .collect::<io::Result<Vec<_>>>()?;
    entries.sort_by(|left, right| left.path().cmp(right.path()));
    let mut digest = Sha256::new();
    for entry in entries {
        if entry.path() == root {
            continue;
        }
        if entry.file_name() == OsStr::new("_arcana-runtime.json") {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        validate_relative_path(relative)?;
        if entry.file_type().is_symlink() {
            return Err(permission_error(
                "staged Python runtime contains a symlink or special file",
            ));
        }
        if entry.file_type().is_dir() {
            digest.update(b"directory\0");
            digest.update(relative.to_string_lossy().as_bytes());
            digest.update(b"\0");
        } else if entry.file_type().is_file() {
            digest.update(b"file\0");
            digest.update(relative.to_string_lossy().as_bytes());
            digest.update(b"\0");
            let bytes = std::fs::read(entry.path())?;
            digest.update((bytes.len() as u64).to_le_bytes());
            digest.update(&bytes);
        } else {
            return Err(permission_error(
                "staged Python runtime contains a symlink or special file",
            ));
        }
    }
    Ok(hex::encode(digest.finalize()))
}

fn verify_staged_runtime(root: &Path, expected_source_key: &str) -> io::Result<()> {
    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(root.join("_arcana-runtime.json"))?)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if manifest.source_key != expected_source_key || hash_tree(root)? != manifest.digest {
        return Err(permission_error(
            "cached Python runtime failed its content-integrity check",
        ));
    }
    Ok(())
}

fn verify_runtime_probe(executable: &Path, expected_root: &Path) -> io::Result<()> {
    const SCRIPT: &str =
        "import json,sys; sys.dont_write_bytecode=True; import pytest; print(json.dumps({'prefix':sys.prefix,'pytest':pytest.__file__}))";
    let output = Command::new(executable)
        .args([
            OsStr::new("-B"),
            OsStr::new("-I"),
            OsStr::new("-S"),
            OsStr::new("-c"),
            OsStr::new(SCRIPT),
        ])
        .env_remove("PYTHONHOME")
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONUSERBASE")
        .env("PYTEST_DISABLE_PLUGIN_AUTOLOAD", "1")
        .output()?;
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "staged Python runtime failed its isolated pytest probe: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    #[derive(Deserialize)]
    struct Probe {
        prefix: PathBuf,
        pytest: PathBuf,
    }
    let probe: Probe = serde_json::from_slice(&output.stdout)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let expected_root = expected_root.canonicalize()?;
    if probe.prefix.canonicalize()? != expected_root
        || !probe.pytest.canonicalize()?.starts_with(&expected_root)
    {
        return Err(permission_error(
            "staged Python resolved imports outside its runtime tree",
        ));
    }
    Ok(())
}

fn permission_error(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::process_sandbox::{self, ProcessSpec};
    use std::ffi::OsString;
    use std::io::Write;
    use std::net::TcpListener;
    use tokio::io::AsyncReadExt;

    #[cfg(all(windows, target_arch = "x86_64"))]
    #[tokio::test]
    async fn ruff_wheel_extractor_writes_only_the_exact_expected_executable() {
        let python = find_python_executable().expect("signed Python runtime");
        let fixture = tempfile::tempdir().expect("Ruff wheel fixture");
        let wheel = fixture.path().join("ruff-test.whl");
        let destination = fixture.path().join("ruff-test.exe");
        let escaped = fixture.path().parent().unwrap().join("ruff-wheel-escape");
        let script = format!(
            r#"import pathlib,sys,zipfile
with zipfile.ZipFile(sys.argv[1], 'w') as archive:
    archive.writestr('ruff-0.16.8.data/scripts/ruff.exe', b'x' * (17 * 1024 * 1024))
    archive.writestr('../../ruff-wheel-escape', b'not extractable')
"#
        );
        let output = Command::new(&python)
            .args(["-B", "-I", "-S", "-c", &script])
            .arg(&wheel)
            .env_remove("PYTHONHOME")
            .env_remove("PYTHONPATH")
            .env_remove("PYTHONUSERBASE")
            .output()
            .expect("create synthetic wheel with signed Python");
        assert!(output.status.success());

        extract_pinned_windows_ruff(&python, &wheel, &destination)
            .await
            .expect("extract exact pinned wheel member");
        let extracted = std::fs::read(&destination).expect("read extracted executable");
        assert_eq!(extracted.len(), 17 * 1024 * 1024);
        assert!(extracted.iter().all(|byte| *byte == b'x'));
        assert!(
            !escaped.exists(),
            "archive traversal member was not extracted"
        );
    }

    #[test]
    fn stdlib_junction_must_resolve_inside_prefix_or_exact_user_runtime() {
        let fixture = tempfile::tempdir().expect("runtime paths fixture");
        let source_root = fixture.path().join("machine-python");
        let approved_lib = fixture.path().join("user-python").join("Lib");
        let unrelated = fixture.path().join("unrelated").join("Lib");
        std::fs::create_dir_all(&source_root).expect("source prefix");
        std::fs::create_dir_all(&approved_lib).expect("approved user runtime");
        std::fs::create_dir_all(&unrelated).expect("unrelated external path");

        assert!(
            validate_stdlib_target(&source_root, &source_root.join("Lib"), &approved_lib).is_ok()
        );
        assert!(validate_stdlib_target(&source_root, &approved_lib, &approved_lib).is_ok());
        assert!(validate_stdlib_target(&source_root, &unrelated, &approved_lib).is_err());
    }

    #[test]
    fn selected_python_fallback_must_be_inside_user_install_root_and_named_python() {
        let fixture = tempfile::tempdir().expect("fallback root fixture");
        let user_root = fixture.path().join("Programs").join("Python");
        let install = user_root.join("Python314");
        let sibling = fixture.path().join("machine-python");
        std::fs::create_dir_all(&install).expect("user Python installation");
        std::fs::create_dir_all(&sibling).expect("external Python installation");
        let user_python = install.join("python.exe");
        let external_python = sibling.join("python.exe");
        let wrong_name = install.join("pythonw.exe");
        std::fs::write(&user_python, b"fixture").expect("user interpreter fixture");
        std::fs::write(&external_python, b"fixture").expect("external interpreter fixture");
        std::fs::write(&wrong_name, b"fixture").expect("wrong interpreter fixture");

        assert_eq!(
            validate_user_owned_python_path(&user_python, &user_root).unwrap(),
            user_python.canonicalize().unwrap()
        );
        assert!(validate_user_owned_python_path(&external_python, &user_root).is_err());
        assert!(validate_user_owned_python_path(&wrong_name, &user_root).is_err());
    }

    #[test]
    fn changing_python_fallback_invalidates_the_process_runtime_cache() {
        let cache = STAGED_RUNTIME.get_or_init(|| Mutex::new(None));
        *cache.lock().unwrap() = Some(StagedPython {
            executable: PathBuf::from("C:/old/python.exe"),
            root: PathBuf::from("C:/old/runtime"),
            source_key: "old-runtime".into(),
        });

        invalidate_staged_runtime();

        assert!(cache.lock().unwrap().is_none());
    }

    #[test]
    fn python_runtime_phase_timing_records_success_and_failure() {
        let (success, events) = crate::observability::timing::capture_json_events(|| {
            timed_io("python_runtime.test_phase", || Ok::<_, io::Error>(7))
        });
        assert_eq!(success.unwrap(), 7);
        let success = events
            .iter()
            .find(|event| event["fields"]["operation"] == "python_runtime.test_phase")
            .expect("success phase event");
        assert_eq!(success["fields"]["outcome"], "success");
        assert!(success["fields"]["duration_ns"].as_u64().is_some());

        let (failure, events) = crate::observability::timing::capture_json_events(|| {
            timed_io("python_runtime.test_phase", || {
                Err::<(), _>(io::Error::other("private path must not be logged"))
            })
        });
        assert!(failure.is_err());
        let failure = events
            .iter()
            .find(|event| event["fields"]["operation"] == "python_runtime.test_phase")
            .expect("failure phase event");
        assert_eq!(failure["fields"]["outcome"], "error");
        assert!(!failure.to_string().contains("private path"));
    }

    #[tokio::test]
    async fn staged_runtime_is_readable_but_not_writable_and_cannot_read_its_sibling() {
        let profile = tempfile::tempdir().expect("isolated Arcana profile");
        let _profile_guard = crate::db::test_support::with_home(profile.path());
        let (runtime, phase_events) = crate::observability::timing::capture_json_events(stage);
        let runtime = runtime.expect("stage selected Python");
        for phase in [
            "python_runtime.validate_interpreter",
            "python_runtime.inspect_interpreter",
            "python_runtime.fingerprint_source",
            "python_runtime.copy_stdlib",
            "python_runtime.copy_pytest_closure",
            "python_runtime.fingerprint_staged_runtime",
            "python_runtime.probe_staged_runtime",
        ] {
            assert!(
                phase_events
                    .iter()
                    .any(|event| event["fields"]["operation"] == phase),
                "missing Python runtime phase timing for {phase}: {phase_events:?}"
            );
        }
        assert!(runtime.executable.starts_with(&runtime.root));
        let parent = runtime.root.parent().expect("runtime cache parent");
        let mut secret_file =
            tempfile::NamedTempFile::new_in(parent).expect("create sibling secret");
        secret_file
            .write_all(b"must remain unreadable")
            .expect("write sibling secret");
        let secret = secret_file.path().to_path_buf();
        let staged_tool = tempfile::tempdir_in(parent).expect("create staged tool directory");
        let staged_tool_marker = staged_tool.path().join("ruff.exe");
        std::fs::write(&staged_tool_marker, b"pinned-tool-marker")
            .expect("write staged tool marker");
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback probe");
        let network_port = listener.local_addr().expect("read loopback port").port();
        let fixture = tempfile::tempdir().expect("workspace fixture");
        let secret_json =
            serde_json::to_string(&secret.to_string_lossy()).expect("encode sibling path");
        let staged_tool_json = serde_json::to_string(&staged_tool_marker.to_string_lossy())
            .expect("encode staged-tool path");
        let script = format!(
            "import pathlib,socket,sys; p={secret_json}; tool={staged_tool_json}; marker=pathlib.Path(sys.prefix,'_arcana_write_probe'); port={network_port};\ntry: pathlib.Path(p).read_text(); raise SystemExit('sibling-read-allowed')\nexcept PermissionError: print('sibling-denied')\ntry: marker.write_text('unexpected'); raise SystemExit('runtime-write-allowed')\nexcept PermissionError: print('runtime-read-only')\ntry: print('staged-tool='+pathlib.Path(tool).read_text())\nexcept Exception as error: raise SystemExit('staged-tool-unreadable:'+type(error).__name__)\ntry: socket.create_connection(('127.0.0.1',port),timeout=1); raise SystemExit('network-allowed')\nexcept OSError: print('network-denied')\nimport pytest; print('pytest='+pytest.__version__)"
        );
        let mut child = process_sandbox::spawn(
            ProcessSpec {
                program: runtime.executable.clone().into_os_string(),
                args: vec![
                    OsString::from("-B"),
                    OsString::from("-I"),
                    OsString::from("-S"),
                    OsString::from("-c"),
                    OsString::from(script),
                ],
                raw_arg: None,
                cwd: fixture.path().to_path_buf(),
                env_remove: ["PYTHONHOME", "PYTHONPATH", "PYTHONUSERBASE"]
                    .into_iter()
                    .map(OsString::from)
                    .collect(),
                env_set: vec![(
                    OsString::from("PYTEST_DISABLE_PLUGIN_AUTOLOAD"),
                    OsString::from("1"),
                )],
                read_only_paths: vec![runtime.root.clone(), staged_tool.path().to_path_buf()],
            },
            fixture.path(),
        )
        .await
        .expect("launch staged Python in AppContainer");
        let mut stdout_reader = child.take_stdout().expect("stdout");
        let mut stderr_reader = child.take_stderr().expect("stderr");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let (stdout_result, stderr_result, status) = tokio::join!(
            stdout_reader.read_to_end(&mut stdout),
            stderr_reader.read_to_end(&mut stderr),
            child.wait()
        );
        stdout_result.expect("read stdout");
        stderr_result.expect("read stderr");
        let status = status.expect("wait for sandboxed Python");
        drop(listener);
        let _ = std::fs::remove_file(runtime.root.join("_arcana_write_probe"));
        assert_eq!(status, 0, "{}", String::from_utf8_lossy(&stderr));
        let output = String::from_utf8_lossy(&stdout);
        assert!(output.contains("sibling-denied"), "{output}");
        assert!(output.contains("runtime-read-only"), "{output}");
        assert!(
            output.contains("staged-tool=pinned-tool-marker"),
            "{output}"
        );
        assert!(output.contains("network-denied"), "{output}");
        assert!(output.contains("pytest="), "{output}");
    }
}
