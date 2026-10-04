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
    let network_filter = network_deny_filter()?;
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
            // Preserve the private exec-status descriptors until exec, but
            // never inherit an ambient host socket/file into project code.
            if libc::syscall(
                libc::SYS_close_range,
                3u32,
                u32::MAX,
                libc::CLOSE_RANGE_CLOEXEC | libc::CLOSE_RANGE_UNSHARE,
            ) != 0
            {
                return Err(io::Error::last_os_error());
            }
            if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            let result = libc::syscall(libc::SYS_landlock_restrict_self, ruleset_fd, 0u32);
            if result != 0 {
                return Err(io::Error::last_os_error());
            }
            let filter = libc::sock_fprog {
                len: network_filter.len() as u16,
                filter: network_filter.as_ptr().cast_mut(),
            };
            // The filter is allocated before fork and borrowed only during
            // this syscall. The kernel copies it before exec; descendants
            // inherit both seccomp and the no-new-privileges restriction.
            if libc::prctl(libc::PR_SET_SECCOMP, libc::SECCOMP_MODE_FILTER, &filter) != 0 {
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

/// Do not leave an alternate syscall ABI or asynchronous submission path
/// available to untrusted children. This complements filesystem confinement;
/// it does not make outside-workspace file reads safe.
fn network_deny_filter() -> io::Result<Vec<libc::sock_filter>> {
    #[cfg(target_arch = "x86_64")]
    let audit_arch = 0xc000_003e;
    #[cfg(target_arch = "aarch64")]
    let audit_arch = 0xc000_00b7;
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    return Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Linux child network confinement requires a supported native syscall ABI",
    ));

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        // seccomp_data begins with an i32 syscall number and a u32 audit arch.
        let instruction = |code, jt, jf, k| libc::sock_filter { code, jt, jf, k };
        const LOAD_WORD_ABSOLUTE: u16 = 0x20;
        const JUMP_EQUAL: u16 = 0x15;
        const RETURN: u16 = 0x06;
        const KILL_PROCESS: u32 = 0x8000_0000;
        const DENY: u32 = 0x0005_0000 | libc::EACCES as u32;
        const ALLOW: u32 = 0x7fff_0000;
        let mut filter = vec![
            instruction(LOAD_WORD_ABSOLUTE, 0, 0, 4),
            instruction(JUMP_EQUAL, 1, 0, audit_arch),
            instruction(RETURN, 0, 0, KILL_PROCESS),
            instruction(LOAD_WORD_ABSOLUTE, 0, 0, 0),
        ];
        #[cfg(target_arch = "x86_64")]
        {
            // x32 shares AUDIT_ARCH_X86_64 but uses a different syscall table.
            const JUMP_BITS_SET: u16 = 0x45;
            filter.push(instruction(JUMP_BITS_SET, 0, 1, 0x4000_0000));
            filter.push(instruction(RETURN, 0, 0, KILL_PROCESS));
        }
        // Rust's process launcher uses an anonymous Unix socket pair to
        // report exec failures. It cannot name or connect to an external
        // endpoint. Host descriptors are closed at exec, so subsequent
        // descriptor passing is confined to objects acquired by these children.
        filter.extend([
            instruction(JUMP_EQUAL, 0, 3, libc::SYS_socketpair as u32),
            instruction(LOAD_WORD_ABSOLUTE, 0, 0, 16), // args[0]: socket domain
            instruction(JUMP_EQUAL, 1, 0, libc::AF_UNIX as u32),
            instruction(RETURN, 0, 0, DENY),
            instruction(LOAD_WORD_ABSOLUTE, 0, 0, 0),
        ]);
        for syscall in [
            libc::SYS_socket,
            libc::SYS_connect,
            libc::SYS_bind,
            libc::SYS_listen,
            libc::SYS_accept,
            libc::SYS_accept4,
            libc::SYS_io_uring_setup,
            libc::SYS_io_uring_enter,
            libc::SYS_io_uring_register,
            libc::SYS_ptrace,
            libc::SYS_process_vm_readv,
            libc::SYS_process_vm_writev,
            libc::SYS_pidfd_getfd,
        ] {
            filter.push(instruction(JUMP_EQUAL, 0, 1, syscall as u32));
            filter.push(instruction(RETURN, 0, 0, DENY));
        }
        filter.push(instruction(RETURN, 0, 0, ALLOW));
        Ok(filter)
    }
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
    use std::net::{TcpListener, TcpStream};
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[cfg(target_arch = "x86_64")]
    #[tokio::test]
    async fn production_linux_sandbox_rejects_x32_syscall_abi() {
        let workspace = tempdir().expect("owned alternate-ABI workspace");
        let script = format!(
            "import ctypes, pathlib\nctypes.CDLL(None).syscall({})\npathlib.Path('continued').write_text('unexpected ABI accepted')\n",
            libc::SYS_getpid | 0x4000_0000
        );
        let mut child = spawn(
            ProcessSpec {
                program: "python3".into(),
                args: vec!["-c".into(), script.into()],
                raw_arg: None,
                cwd: workspace.path().to_path_buf(),
                env_remove: Vec::new(),
                env_set: Vec::new(),
            },
            workspace.path(),
        )
        .await
        .expect("sandboxed ABI fixture starts");
        assert_ne!(
            tokio::time::timeout(std::time::Duration::from_secs(10), child.wait())
                .await
                .expect("bounded alternate-ABI fixture")
                .unwrap(),
            0
        );
        assert!(!workspace.path().join("continued").exists());
    }

    #[tokio::test]
    async fn production_linux_sandbox_denies_network_and_async_bypass_in_descendants() {
        let workspace = tempdir().expect("owned network-denial workspace");
        let listener = TcpListener::bind("127.0.0.1:0").expect("owned local endpoint");
        listener.set_nonblocking(true).unwrap();
        // Prove the endpoint actually works in the unrestricted host.
        let host_connection = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (accepted, _) = listener.accept().unwrap();
        let leaked_fd = unsafe { libc::fcntl(host_connection.as_raw_fd(), libc::F_DUPFD, 200) };
        assert!(
            leaked_fd >= 200,
            "create deliberately non-CLOEXEC host socket"
        );
        let _ambient_socket = unsafe { OwnedFd::from_raw_fd(leaked_fd) };
        assert_eq!(unsafe { libc::fcntl(leaked_fd, libc::F_GETFD) }, 0);
        let script = format!(
            r#"import ctypes, errno, os, pathlib, socket, subprocess, sys
try:
    os.fstat({})
except OSError as error:
    assert error.errno == errno.EBADF, error.errno
else:
    raise AssertionError('ambient host socket survived exec')
def denied(operation):
    try:
        operation()
    except OSError as error:
        assert error.errno == errno.EACCES, error.errno
    else:
        raise AssertionError('sandbox allowed prohibited operation')
for family in (socket.AF_INET, socket.AF_INET6, socket.AF_UNIX):
    denied(lambda: socket.socket(family, socket.SOCK_STREAM))
denied(lambda: socket.socket(socket.AF_INET, socket.SOCK_DGRAM))
denied(lambda: socket.socketpair(socket.AF_INET))
left, right = socket.socketpair()
os.write(left.fileno(), b'internal IPC')
assert os.read(right.fileno(), 32) == b'internal IPC'
left.close()
right.close()
libc = ctypes.CDLL(None, use_errno=True)
libc.syscall.restype = ctypes.c_long
for number in ({}, {}, {}, {}, {}, {}):
    ctypes.set_errno(0)
    result = libc.syscall(number, 0, 0, 0, 0, 0, 0)
    assert result == -1 and ctypes.get_errno() == errno.EACCES, (number, result, ctypes.get_errno())
if len(sys.argv) == 1:
    subprocess.run([sys.executable, __file__, 'descendant'], check=True)
pathlib.Path('denied-' + ('parent' if len(sys.argv) == 1 else 'descendant')).write_text('workspace write succeeds')
"#,
            leaked_fd,
            libc::SYS_connect,
            libc::SYS_io_uring_setup,
            libc::SYS_io_uring_enter,
            libc::SYS_io_uring_register,
            libc::SYS_ptrace,
            libc::SYS_pidfd_getfd,
        );
        std::fs::write(workspace.path().join("network-denial.py"), script).unwrap();
        let mut child = spawn(
            ProcessSpec {
                program: "python3".into(),
                args: vec!["network-denial.py".into()],
                raw_arg: None,
                cwd: workspace.path().to_path_buf(),
                env_remove: Vec::new(),
                env_set: Vec::new(),
            },
            workspace.path(),
        )
        .await
        .expect("production launcher establishes both restrictions");
        let mut stderr = child.take_stderr().expect("diagnostic pipe");
        let drain = tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).await.unwrap();
            bytes
        });
        let code = tokio::time::timeout(std::time::Duration::from_secs(10), child.wait())
            .await
            .expect("bounded network fixture")
            .unwrap();
        let diagnostic = drain.await.unwrap();
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&diagnostic));
        for marker in ["denied-parent", "denied-descendant"] {
            assert_eq!(
                std::fs::read_to_string(workspace.path().join(marker)).unwrap(),
                "workspace write succeeds"
            );
        }
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        drop(accepted);
        drop(host_connection);
        // Dropping the child also closes its confined process group; the
        // listener belongs to this test and closes when the fixture returns.
    }

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
