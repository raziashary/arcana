use super::*;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::os::unix::{ffi::OsStrExt, io::FromRawFd};
use tokio::process::Command;

const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1;
const LANDLOCK_RULE_PATH_BENEATH: u32 = 1;
const LANDLOCK_ACCESS_FS_WRITE_FILE: u64 = 1 << 1;
const LANDLOCK_ACCESS_FS_REMOVE_DIR: u64 = 1 << 4;
const LANDLOCK_ACCESS_FS_REMOVE_FILE: u64 = 1 << 5;
const LANDLOCK_ACCESS_FS_MAKE_CHAR: u64 = 1 << 6;
const LANDLOCK_ACCESS_FS_MAKE_DIR: u64 = 1 << 7;
const LANDLOCK_ACCESS_FS_MAKE_REG: u64 = 1 << 8;
const LANDLOCK_ACCESS_FS_MAKE_SOCK: u64 = 1 << 9;
const LANDLOCK_ACCESS_FS_MAKE_FIFO: u64 = 1 << 10;
const LANDLOCK_ACCESS_FS_MAKE_BLOCK: u64 = 1 << 11;
const LANDLOCK_ACCESS_FS_MAKE_SYM: u64 = 1 << 12;
const LANDLOCK_ACCESS_FS_REFER: u64 = 1 << 13;
const LANDLOCK_ACCESS_FS_TRUNCATE: u64 = 1 << 14;
const WRITE_RIGHTS: u64 = LANDLOCK_ACCESS_FS_WRITE_FILE
    | LANDLOCK_ACCESS_FS_REMOVE_DIR
    | LANDLOCK_ACCESS_FS_REMOVE_FILE
    | LANDLOCK_ACCESS_FS_MAKE_CHAR
    | LANDLOCK_ACCESS_FS_MAKE_DIR
    | LANDLOCK_ACCESS_FS_MAKE_REG
    | LANDLOCK_ACCESS_FS_MAKE_SOCK
    | LANDLOCK_ACCESS_FS_MAKE_FIFO
    | LANDLOCK_ACCESS_FS_MAKE_BLOCK
    | LANDLOCK_ACCESS_FS_MAKE_SYM
    | LANDLOCK_ACCESS_FS_REFER
    | LANDLOCK_ACCESS_FS_TRUNCATE;

#[repr(C)]
struct RulesetAttr {
    handled_access_fs: u64,
}

#[repr(C)]
struct PathBeneathAttr {
    allowed_access: u64,
    parent_fd: i32,
}

pub(super) struct LandlockGuard {
    _ruleset: OwnedFd,
    _workspace: OwnedFd,
    _null_device: OwnedFd,
}

pub(super) fn confine(command: &mut Command, workspace: &Path) -> io::Result<LandlockGuard> {
    let abi = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<RulesetAttr>(),
            0usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    };
    if abi < 3 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "Landlock ABI 3 or newer is required for complete write confinement (found {abi})"
            ),
        ));
    }

    let ruleset_attr = RulesetAttr {
        handled_access_fs: WRITE_RIGHTS,
    };
    let ruleset_fd = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            &ruleset_attr,
            std::mem::size_of::<RulesetAttr>(),
            0u32,
        )
    };
    if ruleset_fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let ruleset = unsafe { OwnedFd::from_raw_fd(ruleset_fd as i32) };
    let workspace_fd = open_workspace(workspace)?;
    let path_rule = PathBeneathAttr {
        allowed_access: WRITE_RIGHTS,
        parent_fd: workspace_fd.as_raw_fd(),
    };
    let add_rule = unsafe {
        libc::syscall(
            libc::SYS_landlock_add_rule,
            ruleset.as_raw_fd(),
            LANDLOCK_RULE_PATH_BENEATH,
            &path_rule,
            0u32,
        )
    };
    if add_rule != 0 {
        return Err(io::Error::last_os_error());
    }
    // Standard Unix runtimes and package-manager shims commonly redirect
    // diagnostics to /dev/null. Permit writes to this single character device
    // while all other writes remain confined to the selected workspace.
    let null_device = open_path("/dev/null")?;
    let null_rule = PathBeneathAttr {
        allowed_access: LANDLOCK_ACCESS_FS_WRITE_FILE,
        parent_fd: null_device.as_raw_fd(),
    };
    let add_null_rule = unsafe {
        libc::syscall(
            libc::SYS_landlock_add_rule,
            ruleset.as_raw_fd(),
            LANDLOCK_RULE_PATH_BENEATH,
            &null_rule,
            0u32,
        )
    };
    if add_null_rule != 0 {
        return Err(io::Error::last_os_error());
    }

    let ruleset_fd = ruleset.as_raw_fd();
    unsafe {
        command.as_std_mut().pre_exec(move || {
            if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            let result = libc::syscall(libc::SYS_landlock_restrict_self, ruleset_fd, 0u32);
            if result != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    // Keep descriptors alive until `Command::spawn` runs the pre-exec hook;
    // CLOEXEC closes them in the confined child only after restriction applies.
    Ok(LandlockGuard {
        _ruleset: ruleset,
        _workspace: workspace_fd,
        _null_device: null_device,
    })
}

fn open_workspace(workspace: &Path) -> io::Result<OwnedFd> {
    open_path(workspace.as_os_str())
}

fn open_path(path: impl AsRef<std::ffi::OsStr>) -> io::Result<OwnedFd> {
    let path = std::ffi::CString::new(path.as_ref().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "workspace path contains NUL"))?;
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[tokio::test]
    async fn confined_child_can_write_workspace_and_cannot_escape_via_symlink_or_child() {
        let parent = tempdir().unwrap();
        let workspace = parent.path().join("workspace");
        let outside = parent.path().join("outside");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::create_dir(&outside).unwrap();
        symlink(&outside, workspace.join("outside-link")).unwrap();

        let script = "printf ok > allowed; printf discard >/dev/null; printf denied > outside-link/marker; sh -c 'printf child-denied > ../outside/child-marker'";
        let child = spawn(
            ProcessSpec {
                program: "sh".into(),
                args: vec!["-c".into(), script.into()],
                raw_arg: None,
                cwd: workspace.clone(),
                env_remove: Vec::new(),
                env_set: Vec::new(),
                #[cfg(windows)]
                read_only_paths: Vec::new(),
            },
            &workspace,
        )
        .await
        .unwrap();
        let mut child = child;
        let code = child.wait().await.unwrap();
        assert_ne!(code, 0);
        assert_eq!(
            std::fs::read_to_string(workspace.join("allowed")).unwrap(),
            "ok"
        );
        assert!(!outside.join("marker").exists());
        assert!(!outside.join("child-marker").exists());
    }
}
