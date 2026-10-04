//! OS-enforced write confinement for processes selected by an agent.
//!
//! The caller supplies argv and the canonical workspace. Platform setup errors
//! are returned before useful work runs; this module never falls back to an
//! unrestricted child.

#[cfg(windows)]
use std::collections::HashMap;
use std::ffi::OsString;
use std::io;
#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::sync::{Arc, Mutex, OnceLock, Weak};
use tokio::io::AsyncRead;
#[cfg(target_os = "macos")]
use tokio::process::Child;
#[cfg(target_os = "linux")]
use tokio::process::{Child, Command};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod psec;
#[cfg(windows)]
pub(crate) mod windows;
#[cfg(windows)]
mod windows_sbox;

/// A run-scoped reference to the sandbox for one selected workspace.
///
/// Tool executors retain this lease for their run. Child launches in the same
/// workspace reuse one canonical workspace object; each child receives a fresh
/// OS security environment bound to that workspace and its selected runtimes.
#[cfg(windows)]
pub(crate) struct WorkspaceLease {
    inner: Arc<WorkspaceLeaseInner>,
}

#[cfg(not(windows))]
pub(crate) struct WorkspaceLease;

#[cfg(windows)]
struct WorkspaceLeaseInner {
    workspace: PathBuf,
    sandbox: Mutex<Option<Arc<windows::WindowsWorkspaceSandbox>>>,
}

#[cfg(windows)]
static WORKSPACE_LEASES: OnceLock<Mutex<HashMap<String, Weak<WorkspaceLeaseInner>>>> =
    OnceLock::new();
#[cfg(windows)]
static WINDOWS_SANDBOX_STARTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

/// Retain a workspace-scoped sandbox lease through the lifetime of an agent
/// run. Canonicalization is repeated by `spawn` and remains fail-closed there.
pub(crate) fn retain_workspace(workspace: &Path) -> WorkspaceLease {
    #[cfg(windows)]
    {
        let workspace = workspace
            .canonicalize()
            .unwrap_or_else(|_| workspace.to_path_buf());
        let key = workspace.to_string_lossy().to_lowercase();
        let leases = WORKSPACE_LEASES.get_or_init(|| Mutex::new(HashMap::new()));
        let mut leases = leases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        leases.retain(|_, lease| lease.strong_count() > 0);
        if let Some(inner) = leases.get(&key).and_then(Weak::upgrade) {
            return WorkspaceLease { inner };
        }
        let inner = Arc::new(WorkspaceLeaseInner {
            workspace,
            sandbox: Mutex::new(None),
        });
        leases.insert(key, Arc::downgrade(&inner));
        WorkspaceLease { inner }
    }
    #[cfg(not(windows))]
    {
        let _ = workspace;
        WorkspaceLease
    }
}

#[cfg(windows)]
fn find_or_retain_workspace(workspace: &Path) -> WorkspaceLease {
    let workspace = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    retain_workspace(&workspace)
}

#[cfg(windows)]
impl WorkspaceLeaseInner {
    fn sandbox(&self) -> io::Result<Arc<windows::WindowsWorkspaceSandbox>> {
        let mut cached = self
            .sandbox
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(sandbox) = cached.as_ref() {
            return Ok(Arc::clone(sandbox));
        }
        let sandbox = Arc::new(windows::WindowsWorkspaceSandbox::new(&self.workspace)?);
        *cached = Some(Arc::clone(&sandbox));
        Ok(sandbox)
    }
}

pub(crate) struct ProcessSpec {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub raw_arg: Option<OsString>,
    pub cwd: PathBuf,
    pub env_remove: Vec<OsString>,
    pub env_set: Vec<(OsString, OsString)>,
    #[cfg(windows)]
    pub read_only_paths: Vec<PathBuf>,
}

pub(crate) enum SandboxedChild {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    Tokio { child: Child, process_group: i32 },
    #[cfg(windows)]
    Windows(windows::WindowsSandboxChild),
}

impl SandboxedChild {
    pub(crate) fn take_stdout(&mut self) -> Option<Box<dyn AsyncRead + Unpin + Send>> {
        match self {
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::Tokio { child, .. } => child
                .stdout
                .take()
                .map(|reader| Box::new(reader) as Box<dyn AsyncRead + Unpin + Send>),
            #[cfg(windows)]
            Self::Windows(child) => child
                .take_stdout()
                .ok()
                .map(|reader| Box::new(reader) as Box<dyn AsyncRead + Unpin + Send>),
        }
    }

    pub(crate) fn take_stderr(&mut self) -> Option<Box<dyn AsyncRead + Unpin + Send>> {
        match self {
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::Tokio { child, .. } => child
                .stderr
                .take()
                .map(|reader| Box::new(reader) as Box<dyn AsyncRead + Unpin + Send>),
            #[cfg(windows)]
            Self::Windows(child) => child
                .take_stderr()
                .ok()
                .map(|reader| Box::new(reader) as Box<dyn AsyncRead + Unpin + Send>),
        }
    }

    pub(crate) async fn wait(&mut self) -> io::Result<i32> {
        match self {
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::Tokio { child, .. } => {
                child.wait().await.map(|status| status.code().unwrap_or(-1))
            }
            #[cfg(windows)]
            Self::Windows(child) => child.wait().await.map(|status| status.code().unwrap_or(-1)),
        }
    }
}

impl Drop for SandboxedChild {
    fn drop(&mut self) {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        if let Self::Tokio { process_group, .. } = self {
            // Each Unix child starts a new process group. Killing the group on
            // drop covers descendants left behind after timeout/cancellation.
            unsafe {
                libc::kill(-*process_group, libc::SIGKILL);
            }
        }
    }
}

pub(crate) async fn spawn(spec: ProcessSpec, workspace: &Path) -> io::Result<SandboxedChild> {
    let workspace = workspace.canonicalize()?;
    #[cfg(target_os = "linux")]
    let env_set = {
        let mut overrides = workspace_cache_environment(&workspace)?;
        overrides.extend(spec.env_set.iter().cloned());
        overrides
    };
    #[cfg(windows)]
    {
        return run_windows_spawn_blocking(move || {
            let lease = find_or_retain_workspace(&workspace);
            let sandbox = lease.inner.sandbox()?;
            sandbox
                .spawn_experimental_with_read_only_paths(
                    &spec.program,
                    &spec.args,
                    spec.raw_arg.as_deref(),
                    &spec.cwd,
                    &spec.env_remove,
                    &spec.env_set,
                    &spec.read_only_paths,
                )
                .map(SandboxedChild::Windows)
        })
        .await;
    }

    #[cfg(target_os = "macos")]
    {
        let _ = (spec, workspace);
        return Err(macos::xpc_runner_unavailable());
    }

    #[cfg(target_os = "linux")]
    {
        let mut command = Command::new(&spec.program);
        command.args(&spec.args);
        command
            .current_dir(&spec.cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        for key in &spec.env_remove {
            command.env_remove(key);
        }
        command.envs(env_set.iter().cloned());
        #[cfg(unix)]
        command.as_std_mut().process_group(0);
        #[cfg(target_os = "linux")]
        let _landlock = linux::confine(&mut command, &workspace)?;
        let child = command.spawn()?;
        let process_group = child.id().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                "spawned child has no process identifier",
            )
        })? as i32;
        Ok(SandboxedChild::Tokio {
            child,
            process_group,
        })
    }
}

#[cfg(windows)]
async fn run_windows_spawn_blocking<T, F>(launch: F) -> io::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> io::Result<T> + Send + 'static,
{
    let permit = WINDOWS_SANDBOX_STARTS
        .acquire()
        .await
        .map_err(io::Error::other)?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        launch()
    })
    .await
    .map_err(io::Error::other)?
}

/// Dispatch Arcana's private OS-sandbox bridge before a host executable starts
/// its ordinary CLI or Tauri runtime. Non-Windows hosts have no bridge mode.
#[cfg(windows)]
pub fn run_bridge_entrypoint() -> Option<i32> {
    windows::run_bridge_entrypoint()
}

#[cfg(not(windows))]
pub fn run_bridge_entrypoint() -> Option<i32> {
    None
}

#[cfg(all(test, windows))]
mod workspace_lease_tests {
    use super::*;

    #[tokio::test]
    async fn cancelled_windows_launch_returns_and_drops_a_late_child() {
        struct DropSignal(std::sync::mpsc::Sender<()>);

        impl Drop for DropSignal {
            fn drop(&mut self) {
                let _ = self.0.send(());
            }
        }

        let (sender, receiver) = std::sync::mpsc::channel();
        let launch = run_windows_spawn_blocking(move || {
            std::thread::sleep(std::time::Duration::from_millis(75));
            Ok(DropSignal(sender))
        });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(5), launch)
                .await
                .is_err(),
            "the async caller must not wait for blocking OS launch setup after cancellation"
        );
        let dropped = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            tokio::task::spawn_blocking(move || {
                receiver.recv_timeout(std::time::Duration::from_secs(1))
            }),
        )
        .await
        .expect("late launch completion")
        .expect("receiver thread")
        .is_ok();
        assert!(
            dropped,
            "a child returned after cancellation must be dropped"
        );
    }

    #[test]
    fn a_run_reuses_only_its_canonical_workspace_lease() {
        let root = tempfile::tempdir().expect("temporary root");
        let workspace = root.path().join("workspace");
        let other_workspace = root.path().join("other");
        std::fs::create_dir_all(&workspace).expect("workspace");
        std::fs::create_dir_all(&other_workspace).expect("other workspace");

        let first = retain_workspace(&workspace);
        let same = retain_workspace(&workspace.join("."));
        let other = retain_workspace(&other_workspace);
        assert!(Arc::ptr_eq(&first.inner, &same.inner));
        assert!(!Arc::ptr_eq(&first.inner, &other.inner));
        let first_sandbox = first.inner.sandbox().expect("first workspace sandbox");
        let same_sandbox = same.inner.sandbox().expect("reused workspace sandbox");
        let other_sandbox = other.inner.sandbox().expect("separate workspace sandbox");
        assert!(Arc::ptr_eq(&first_sandbox, &same_sandbox));
        assert!(!Arc::ptr_eq(&first_sandbox, &other_sandbox));
    }
}

#[cfg(target_os = "linux")]
fn workspace_cache_environment(workspace: &Path) -> io::Result<Vec<(OsString, OsString)>> {
    let temp_dir = ensure_workspace_directory(workspace, ".arcana-tmp")?;
    let cache_dir = ensure_workspace_directory(workspace, ".arcana-cache")?;
    Ok([
        ("TMPDIR", temp_dir.clone()),
        ("TMP", temp_dir.clone()),
        ("TEMP", temp_dir),
        ("XDG_CACHE_HOME", cache_dir.join("xdg")),
        ("CARGO_HOME", cache_dir.join("cargo")),
        ("CARGO_TARGET_DIR", cache_dir.join("cargo-target")),
        ("NPM_CONFIG_CACHE", cache_dir.join("npm")),
        ("YARN_CACHE_FOLDER", cache_dir.join("yarn")),
        ("PNPM_HOME", cache_dir.join("pnpm")),
        ("BUN_INSTALL_CACHE_DIR", cache_dir.join("bun")),
        ("PIP_CACHE_DIR", cache_dir.join("python-pip")),
        ("PYTHONPYCACHEPREFIX", cache_dir.join("python-bytecode")),
        ("GOCACHE", cache_dir.join("go-build")),
        ("GOMODCACHE", cache_dir.join("go-mod")),
    ]
    .into_iter()
    .map(|(key, path)| (OsString::from(key), path.into_os_string()))
    .collect())
}

#[cfg(all(test, target_os = "linux"))]
mod linux_cache_environment_tests {
    use super::workspace_cache_environment;
    use tempfile::tempdir;

    #[test]
    fn cargo_cache_and_build_output_are_isolated_inside_the_selected_workspace() {
        let workspace = tempdir().expect("isolated workspace");
        let variables =
            workspace_cache_environment(workspace.path()).expect("workspace cache environment");
        for (key, suffix) in [
            ("CARGO_HOME", ".arcana-cache/cargo"),
            ("CARGO_TARGET_DIR", ".arcana-cache/cargo-target"),
        ] {
            let path = variables
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| std::path::PathBuf::from(value))
                .unwrap_or_else(|| panic!("{key} override"));
            assert!(path.starts_with(workspace.path()), "{key}={path:?}");
            assert!(path.ends_with(suffix), "{key}={path:?}");
        }
    }
}

#[cfg(target_os = "linux")]
fn ensure_workspace_directory(workspace: &Path, name: &str) -> io::Result<PathBuf> {
    let path = workspace.join(name);
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "sandbox directory {} must be a real workspace directory",
                    path.display()
                ),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => std::fs::create_dir(&path)?,
        Err(error) => return Err(error),
    }
    let canonical = path.canonicalize()?;
    if !canonical.starts_with(workspace) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "sandbox directory {} resolves outside the workspace",
                path.display()
            ),
        ));
    }
    Ok(canonical)
}

#[cfg(target_os = "macos")]
mod macos {
    use std::io;

    pub(super) fn xpc_runner_unavailable() -> io::Error {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "macOS project commands are disabled until the signed App Sandbox XPC runner is packaged and verified",
        )
    }
}

#[cfg(all(test, target_os = "macos"))]
mod macos_tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn project_command_fails_closed_until_signed_xpc_runner_is_available() {
        let parent = tempdir().expect("temporary parent");
        let workspace = parent.path().join("workspace");
        std::fs::create_dir(&workspace).expect("workspace");
        let result = spawn(
            ProcessSpec {
                program: "/bin/true".into(),
                args: Vec::new(),
                raw_arg: None,
                cwd: workspace.clone(),
                env_remove: Vec::new(),
                env_set: Vec::new(),
                #[cfg(windows)]
                read_only_paths: Vec::new(),
            },
            &workspace,
        );

        let error = match result {
            Ok(_) => panic!("macOS must not launch without the signed XPC runner"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert!(!workspace.join("allowed").exists());
    }
}
