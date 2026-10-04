//! Windows child processes confined by the experimental SBOX process API.
//!
//! The OS API forbids inherited handles, so Arcana starts its own bridge in an
//! AppContainer. The bridge launches only the validated command, captures
//! bounded output, and terminates all descendants before returning results.

use std::ffi::{OsStr, OsString};
use std::io::{self, Read};
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, IntoRawHandle, OwnedHandle, RawHandle};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::thread;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, DuplexStream};
use windows_sys::Win32::Foundation::GENERIC_READ;
use windows_sys::Win32::Foundation::{
    DuplicateHandle, GetLastError, DUPLICATE_SAME_ACCESS, HANDLE, HANDLE_FLAG_INHERIT,
    INVALID_HANDLE_VALUE,
};
#[cfg(test)]
use windows_sys::Win32::Foundation::{LocalFree, HLOCAL};
#[cfg(test)]
use windows_sys::Win32::Security::Authorization::{
    GetNamedSecurityInfoW, SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W,
    GRANT_ACCESS, REVOKE_ACCESS, SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_USER,
};
#[cfg(test)]
use windows_sys::Win32::Security::Isolation::CreateAppContainerProfile;
use windows_sys::Win32::Security::Isolation::DeleteAppContainerProfile;
#[cfg(test)]
use windows_sys::Win32::Security::Isolation::DeriveAppContainerSidFromAppContainerName;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Security::{GetTokenInformation, TokenIsAppContainer, TOKEN_QUERY};
#[cfg(test)]
use windows_sys::Win32::Security::{
    CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, OBJECT_INHERIT_ACE, PSECURITY_DESCRIPTOR,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
#[cfg(test)]
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_DELETE_CHILD, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
    FILE_READ_ATTRIBUTES, FILE_TRAVERSE,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::SystemInformation::{GetSystemDirectoryW, OSVERSIONINFOW};
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetCurrentProcess, GetExitCodeProcess,
    InitializeProcThreadAttributeList, OpenProcessToken, ResumeThread, UpdateProcThreadAttribute,
    WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT,
    EXTENDED_STARTUPINFO_PRESENT, INFINITE, LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, STARTF_USESTDHANDLES, STARTUPINFOEXW, STARTUPINFOW,
};

#[cfg(test)]
const APP_CONTAINER_PREFIX: &str = "Arcana-Agent-";
pub(super) const TEMP_DIR_NAME: &str = ".arcana-tmp";
pub(super) const CACHE_DIR_NAME: &str = ".arcana-cache";
#[cfg(test)]
const INHERIT_TO_CHILDREN: u32 = CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE;
#[cfg(test)]
const WORKSPACE_ACCESS: u32 =
    FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE | FILE_DELETE_CHILD | DELETE;
const PROC_THREAD_ATTRIBUTE_HANDLE_LIST_VALUE: usize = PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize;
const PROC_THREAD_ATTRIBUTE_SECURITY_ENVIRONMENT_VALUE: usize =
    super::psec::process_security_environment_attribute();

/// A sandboxed bridge process with async stdout/stderr readers.
///
/// The job object is configured to terminate all descendants when this value is
/// dropped. Callers should drain both pipes concurrently with [`Self::wait`], as
/// they do for Tokio's child process type.
pub struct WindowsSandboxChild {
    process: OwnedHandle,
    stdout: Option<Box<dyn tokio::io::AsyncRead + Unpin + Send>>,
    stderr: Option<Box<dyn tokio::io::AsyncRead + Unpin + Send>>,
    bridge_stdout: Option<DuplexStream>,
    bridge_stderr: Option<DuplexStream>,
    bridge_files: Option<BridgeFiles>,
    _job: JobGuard,
    _sandbox: Arc<WindowsWorkspaceSandbox>,
    _security_environment: Option<super::psec::ProcessSecurityEnvironment>,
    _sbox_specification: Option<Vec<u8>>,
    _sbox_identity: Option<Vec<u16>>,
}

/// A canonical workspace identity reused for child process policy creation.
pub(super) struct WindowsWorkspaceSandbox {
    workspace: PathBuf,
}

impl WindowsWorkspaceSandbox {
    pub(super) fn new(workspace: &Path) -> io::Result<Self> {
        let workspace = canonical_workspace(workspace)?;
        Ok(Self { workspace })
    }

    #[cfg(test)]
    pub(super) fn spawn(
        self: &Arc<Self>,
        program: &OsStr,
        args: &[OsString],
        raw_arg: Option<&OsStr>,
        cwd: &Path,
        env_remove: &[OsString],
        env_set: &[(OsString, OsString)],
    ) -> io::Result<WindowsSandboxChild> {
        self.spawn_with_read_only_paths(program, args, raw_arg, cwd, env_remove, env_set, &[])
    }

    pub(super) fn spawn_with_read_only_paths(
        self: &Arc<Self>,
        program: &OsStr,
        args: &[OsString],
        raw_arg: Option<&OsStr>,
        cwd: &Path,
        env_remove: &[OsString],
        env_set: &[(OsString, OsString)],
        read_only_paths: &[PathBuf],
    ) -> io::Result<WindowsSandboxChild> {
        spawn_experimental_bridge(
            Arc::clone(self),
            program,
            args,
            raw_arg,
            cwd,
            env_remove,
            env_set,
            read_only_paths,
        )
    }

    pub(super) fn spawn_experimental_with_read_only_paths(
        self: &Arc<Self>,
        program: &OsStr,
        args: &[OsString],
        raw_arg: Option<&OsStr>,
        cwd: &Path,
        env_remove: &[OsString],
        env_set: &[(OsString, OsString)],
        read_only_paths: &[PathBuf],
    ) -> io::Result<WindowsSandboxChild> {
        spawn_experimental_bridge(
            Arc::clone(self),
            program,
            args,
            raw_arg,
            cwd,
            env_remove,
            env_set,
            read_only_paths,
        )
    }
}

impl WindowsSandboxChild {
    pub fn take_stdout(&mut self) -> io::Result<Box<dyn tokio::io::AsyncRead + Unpin + Send>> {
        self.stdout.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "child stdout was already taken")
        })
    }

    pub fn take_stderr(&mut self) -> io::Result<Box<dyn tokio::io::AsyncRead + Unpin + Send>> {
        self.stderr.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "child stderr was already taken")
        })
    }

    pub async fn wait(&mut self) -> io::Result<std::process::ExitStatus> {
        let process = duplicate_handle(self.process.as_raw_handle())?;
        let process_code = tokio::task::spawn_blocking(move || unsafe {
            let raw = process.as_raw_handle() as HANDLE;
            let wait = WaitForSingleObject(raw, INFINITE);
            if wait == u32::MAX {
                return Err(last_error());
            }
            let mut code = 0u32;
            if GetExitCodeProcess(raw, &mut code) == 0 {
                return Err(last_error());
            }
            Ok(code)
        })
        .await
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error))??;
        let process_code = self.finish_bridge_output(process_code).await?;
        Ok(exit_status(process_code as u32))
    }

    async fn finish_bridge_output(&mut self, process_code: u32) -> io::Result<i32> {
        let Some(files) = self.bridge_files.take() else {
            return Ok(process_code as i32);
        };
        let result = async {
            if process_code != 0 {
                let (failure, os_code) = decode_bridge_failure(process_code);
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "sandbox bridge failed before command output: stage={failure} os_code={os_code}"
                    ),
                ));
            }
            let bytes = tokio::fs::read(&files.output_path).await?;
            if bytes.len() > MAX_BRIDGE_RESULT_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "sandbox bridge result exceeded its size bound",
                ));
            }
            let result: BridgeResult = serde_json::from_slice(&bytes).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "sandbox bridge returned a malformed result",
                )
            })?;
            let stdout = BASE64.decode(result.stdout_tail).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "sandbox bridge stdout is malformed",
                )
            })?;
            let stderr = BASE64.decode(result.stderr_tail).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "sandbox bridge stderr is malformed",
                )
            })?;
            if stdout.len() > MAX_BRIDGE_CAPTURE_BYTES
                || stderr.len() > MAX_BRIDGE_CAPTURE_BYTES
                || result.stdout_total < stdout.len() as u64
                || result.stderr_total < stderr.len() as u64
                || result.exit_code < 0
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "sandbox bridge returned inconsistent capture metadata",
                ));
            }
            if let Some(writer) = self.bridge_stdout.as_mut() {
                writer.write_all(&stdout).await?;
                writer.shutdown().await?;
            }
            if let Some(writer) = self.bridge_stderr.as_mut() {
                writer.write_all(&stderr).await?;
                writer.shutdown().await?;
            }
            Ok(result.exit_code)
        }
        .await;
        if result.is_err() {
            if let Some(writer) = self.bridge_stdout.as_mut() {
                let _ = writer.shutdown().await;
            }
            if let Some(writer) = self.bridge_stderr.as_mut() {
                let _ = writer.shutdown().await;
            }
        }
        drop(files);
        result
    }

    /// Terminate the process and every descendant in its job.
    pub fn kill(&mut self) -> io::Result<()> {
        if unsafe { TerminateJobObject(self._job.0.as_raw_handle() as HANDLE, 1) } == 0 {
            return Err(last_error());
        }
        Ok(())
    }
}

impl Drop for WindowsSandboxChild {
    fn drop(&mut self) {
        if let Some(files) = self.bridge_files.take() {
            // Closing the job kills descendants before request/result cleanup.
            unsafe {
                TerminateJobObject(self._job.0.as_raw_handle() as HANDLE, 1);
                WaitForSingleObject(self._job.0.as_raw_handle() as HANDLE, INFINITE);
            }
            drop(files);
        }
        if let Some(identity) = self._sbox_identity.take() {
            if unsafe { DeleteAppContainerProfile(identity.as_ptr()) } < 0 {
                tracing::warn!(
                    target: "arcana::telemetry",
                    event_type = "sandbox_profile_cleanup_failed",
                    "could not remove the completed Windows sandbox profile"
                );
            }
        }
    }
}

const MAX_BRIDGE_CAPTURE_BYTES: usize = 256 * 1024;
const MAX_BRIDGE_RESULT_BYTES: usize = 800 * 1024;
const BRIDGE_ENTRYPOINT_ARG: &str = "--arcana-internal-sandbox-bridge";

#[derive(Serialize, Deserialize)]
struct BridgeRequest {
    version: u8,
    program: String,
    args: Vec<String>,
    raw_arg: Option<String>,
    cwd: String,
    workspace: String,
    output_path: String,
}

#[derive(Serialize, Deserialize)]
struct BridgeResult {
    exit_code: i32,
    stdout_total: u64,
    stderr_total: u64,
    stdout_tail: String,
    stderr_tail: String,
}

struct BridgeFiles {
    request_path: PathBuf,
    output_path: PathBuf,
}

impl Drop for BridgeFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.request_path);
        let _ = std::fs::remove_file(&self.output_path);
    }
}

/// Handle Arcana's private bridge invocation before normal CLI/Tauri startup.
/// The bridge refuses to run unless Windows confirms this process is already
/// inside an AppContainer token created by an OS sandbox policy.
pub fn run_bridge_entrypoint() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(OsStr::new(BRIDGE_ENTRYPOINT_ARG)) {
        return None;
    }
    let Some(request_path) = args.next() else {
        return Some(BRIDGE_FAILURE_EXIT);
    };
    if args.next().is_some() {
        return Some(BRIDGE_FAILURE_EXIT);
    }
    Some(match run_bridge_request(Path::new(&request_path)) {
        Ok(exit_code) => exit_code,
        Err(error) => error.exit_code(),
    })
}

const BRIDGE_FAILURE_EXIT: i32 = 125;
const BRIDGE_FAILURE_DETAIL_BASE: u32 = 1_000_000;
const BRIDGE_FAILURE_STAGE_STRIDE: u32 = 100_000;
const BRIDGE_FAILURE_APP_CONTAINER_CHECK: u32 = 1;
const BRIDGE_FAILURE_REQUEST_READ: u32 = 2;
const BRIDGE_FAILURE_REQUEST_VALIDATION: u32 = 3;
const BRIDGE_FAILURE_REQUEST_REMOVE: u32 = 4;
const BRIDGE_FAILURE_RESULT_CREATE: u32 = 5;
const BRIDGE_FAILURE_CHILD_LAUNCH: u32 = 6;
const BRIDGE_FAILURE_RESULT_WRITE: u32 = 7;
const MAX_BRIDGE_REQUEST_BYTES: u64 = 96 * 1024;

struct BridgeFailure {
    stage: u32,
    os_code: u32,
}

impl BridgeFailure {
    fn exit_code(&self) -> i32 {
        (BRIDGE_FAILURE_DETAIL_BASE
            + self.stage * BRIDGE_FAILURE_STAGE_STRIDE
            + self.os_code.min(BRIDGE_FAILURE_STAGE_STRIDE - 1)) as i32
    }
}

fn bridge_stage<T>(stage: u32, result: io::Result<T>) -> Result<T, BridgeFailure> {
    result.map_err(|error| BridgeFailure {
        stage,
        os_code: error.raw_os_error().unwrap_or_default() as u32,
    })
}

fn decode_bridge_failure(exit_code: u32) -> (&'static str, u32) {
    if exit_code < BRIDGE_FAILURE_DETAIL_BASE {
        return ("bridge_failed", 0);
    }
    let detail = exit_code - BRIDGE_FAILURE_DETAIL_BASE;
    let stage = detail / BRIDGE_FAILURE_STAGE_STRIDE;
    let os_code = detail % BRIDGE_FAILURE_STAGE_STRIDE;
    let label = match stage {
        BRIDGE_FAILURE_APP_CONTAINER_CHECK => "appcontainer_check",
        BRIDGE_FAILURE_REQUEST_READ => "request_read",
        BRIDGE_FAILURE_REQUEST_VALIDATION => "request_validation",
        BRIDGE_FAILURE_REQUEST_REMOVE => "request_remove",
        BRIDGE_FAILURE_RESULT_CREATE => "result_create",
        BRIDGE_FAILURE_CHILD_LAUNCH => "child_launch",
        BRIDGE_FAILURE_RESULT_WRITE => "result_write",
        _ => "unknown_stage",
    };
    (label, os_code)
}

fn run_bridge_request(request_path: &Path) -> Result<i32, BridgeFailure> {
    if !bridge_stage(
        BRIDGE_FAILURE_APP_CONTAINER_CHECK,
        process_is_app_container(),
    )? {
        return Err(BridgeFailure {
            stage: BRIDGE_FAILURE_APP_CONTAINER_CHECK,
            os_code: 0,
        });
    }

    let request_bytes = bridge_stage(BRIDGE_FAILURE_REQUEST_READ, std::fs::read(request_path))?;
    if request_bytes.len() as u64 > MAX_BRIDGE_REQUEST_BYTES {
        return Err(BridgeFailure {
            stage: BRIDGE_FAILURE_REQUEST_VALIDATION,
            os_code: 0,
        });
    }
    let request: BridgeRequest = bridge_stage(
        BRIDGE_FAILURE_REQUEST_VALIDATION,
        serde_json::from_slice(&request_bytes).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "sandbox bridge request is malformed",
            )
        }),
    )?;
    bridge_stage(
        BRIDGE_FAILURE_REQUEST_VALIDATION,
        validate_bridge_request(&request, request_path),
    )?;
    // The command does not start until its serialized host request has been
    // deleted, so untrusted project code cannot rewrite or replay it.
    bridge_stage(
        BRIDGE_FAILURE_REQUEST_REMOVE,
        std::fs::remove_file(request_path),
    )?;

    let output_path = PathBuf::from(&request.output_path);
    let mut output_file = bridge_stage(
        BRIDGE_FAILURE_RESULT_CREATE,
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(&output_path),
    )?;

    let (exit_code, stdout, stderr) =
        bridge_stage(BRIDGE_FAILURE_CHILD_LAUNCH, run_bridge_child(&request))?;
    let result = BridgeResult {
        exit_code,
        stdout_total: stdout.total,
        stderr_total: stderr.total,
        stdout_tail: BASE64.encode(stdout.tail),
        stderr_tail: BASE64.encode(stderr.tail),
    };
    let bytes = bridge_stage(
        BRIDGE_FAILURE_RESULT_WRITE,
        serde_json::to_vec(&result).map_err(io::Error::other),
    )?;
    if bytes.len() > MAX_BRIDGE_RESULT_BYTES {
        return Err(BridgeFailure {
            stage: BRIDGE_FAILURE_RESULT_WRITE,
            os_code: 0,
        });
    }
    use std::io::Write;
    bridge_stage(BRIDGE_FAILURE_RESULT_WRITE, output_file.write_all(&bytes))?;
    bridge_stage(BRIDGE_FAILURE_RESULT_WRITE, output_file.sync_all())?;
    Ok(0)
}

fn process_is_app_container() -> io::Result<bool> {
    let mut token = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(last_error());
    }
    let token = unsafe { OwnedHandle::from_raw_handle(token as RawHandle) };
    let mut value = 0u32;
    let mut returned = 0u32;
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle() as HANDLE,
            TokenIsAppContainer,
            (&mut value as *mut u32).cast(),
            size_of::<u32>() as u32,
            &mut returned,
        )
    } == 0
    {
        return Err(last_error());
    }
    Ok(value != 0)
}

fn validate_bridge_request(request: &BridgeRequest, request_path: &Path) -> io::Result<()> {
    if request.version != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "sandbox bridge request version is unsupported",
        ));
    }
    let workspace = PathBuf::from(&request.workspace);
    if !workspace.is_absolute() || contains_parent_component(&workspace) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "sandbox bridge workspace is not a normalized absolute path",
        ));
    }
    let temp = workspace.join(TEMP_DIR_NAME);
    validate_bridge_file(request_path, &temp, ".arcana-bridge-request-")?;
    let output = PathBuf::from(&request.output_path);
    validate_bridge_file(&output, &temp, ".arcana-bridge-result-")?;

    let cwd = PathBuf::from(&request.cwd);
    if !cwd.is_absolute() || contains_parent_component(&cwd) || !cwd.starts_with(&workspace) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "sandbox bridge working directory resolves outside the workspace",
        ));
    }
    if request.program.is_empty() || request.args.len() > 4096 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "sandbox bridge command is invalid or exceeds its argument bound",
        ));
    }
    Ok(())
}

fn contains_parent_component(path: &Path) -> bool {
    path.components()
        .any(|component| component == std::path::Component::ParentDir)
}

fn validate_bridge_file(path: &Path, temp: &Path, prefix: &str) -> io::Result<()> {
    if !path.is_absolute()
        || contains_parent_component(path)
        || path.parent() != Some(temp)
        || !path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with(prefix))
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "sandbox bridge file must be a uniquely named file in workspace temporary storage",
        ));
    }
    Ok(())
}

struct TailCapture {
    tail: Vec<u8>,
    total: u64,
}

fn capture_bridge_stream(mut reader: impl Read) -> io::Result<TailCapture> {
    let mut tail = Vec::with_capacity(MAX_BRIDGE_CAPTURE_BYTES);
    let mut buffer = [0u8; 8192];
    let mut total = 0u64;
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total.saturating_add(count as u64);
        tail.extend_from_slice(&buffer[..count]);
        if tail.len() > MAX_BRIDGE_CAPTURE_BYTES {
            let excess = tail.len() - MAX_BRIDGE_CAPTURE_BYTES;
            tail.drain(..excess);
        }
    }
    Ok(TailCapture { tail, total })
}

fn run_bridge_child(request: &BridgeRequest) -> io::Result<(i32, TailCapture, TailCapture)> {
    let program = OsString::from(&request.program);
    let args = request.args.iter().map(OsString::from).collect::<Vec<_>>();
    let raw_arg = request.raw_arg.as_deref().map(OsStr::new);
    let cwd = PathBuf::from(&request.cwd);
    let (application, mut command_line) = command_line(&program, &args, raw_arg, &cwd)?;
    // Rust canonicalization may produce Win32's extended `\\?\` prefix.
    // `cmd.exe` treats that form as a UNC working directory, so pass its
    // normalized DOS path to CreateProcessW while retaining canonical checks
    // in the parent.
    let current_dir = wide_null(&create_process_directory(&cwd));

    let (stdin, stdout_read, stdout_write, stderr_read, stderr_write) = create_standard_handles()?;
    let inherited = [
        stdin.as_raw_handle() as HANDLE,
        stdout_write.as_raw_handle() as HANDLE,
        stderr_write.as_raw_handle() as HANDLE,
    ];
    let mut attributes = AttributeList::new(1)?;
    attributes.update(PROC_THREAD_ATTRIBUTE_HANDLE_LIST_VALUE, &inherited)?;
    let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = stdin.as_raw_handle() as HANDLE;
    startup.StartupInfo.hStdOutput = stdout_write.as_raw_handle() as HANDLE;
    startup.StartupInfo.hStdError = stderr_write.as_raw_handle() as HANDLE;
    startup.lpAttributeList = attributes.as_mut_ptr();

    let mut process_info: PROCESS_INFORMATION = unsafe { zeroed() };
    let created = unsafe {
        CreateProcessW(
            application.as_ptr(),
            command_line.as_mut_ptr(),
            null(),
            null(),
            1,
            CREATE_SUSPENDED
                | EXTENDED_STARTUPINFO_PRESENT
                | CREATE_UNICODE_ENVIRONMENT
                | CREATE_NO_WINDOW,
            null_mut(),
            current_dir.as_ptr(),
            &startup.StartupInfo,
            &mut process_info,
        )
    };
    if created == 0 {
        return Err(stage_error("sandbox bridge child launch", last_error()));
    }
    let process = unsafe { OwnedHandle::from_raw_handle(process_info.hProcess as RawHandle) };
    let thread_handle = unsafe { OwnedHandle::from_raw_handle(process_info.hThread as RawHandle) };
    let job = JobGuard(create_kill_on_close_job()?);
    if unsafe {
        AssignProcessToJobObject(
            job.0.as_raw_handle() as HANDLE,
            process.as_raw_handle() as HANDLE,
        )
    } == 0
    {
        let error = last_error();
        unsafe {
            windows_sys::Win32::System::Threading::TerminateProcess(
                process.as_raw_handle() as HANDLE,
                BRIDGE_FAILURE_EXIT as u32,
            );
            WaitForSingleObject(process.as_raw_handle() as HANDLE, INFINITE);
        }
        return Err(stage_error(
            "sandbox bridge descendant job assignment",
            error,
        ));
    }

    let stdout_reader = unsafe { std::fs::File::from_raw_handle(stdout_read.into_raw_handle()) };
    let stderr_reader = unsafe { std::fs::File::from_raw_handle(stderr_read.into_raw_handle()) };
    let stdout_task = thread::spawn(move || capture_bridge_stream(stdout_reader));
    let stderr_task = thread::spawn(move || capture_bridge_stream(stderr_reader));

    drop(stdin);
    drop(stdout_write);
    drop(stderr_write);
    if unsafe { ResumeThread(thread_handle.as_raw_handle() as HANDLE) } == u32::MAX {
        let error = last_error();
        unsafe {
            TerminateJobObject(job.0.as_raw_handle() as HANDLE, BRIDGE_FAILURE_EXIT as u32);
            WaitForSingleObject(job.0.as_raw_handle() as HANDLE, INFINITE);
        }
        return Err(error);
    }

    if unsafe { WaitForSingleObject(process.as_raw_handle() as HANDLE, INFINITE) } == u32::MAX {
        return Err(last_error());
    }
    let mut exit_code = 0u32;
    if unsafe { GetExitCodeProcess(process.as_raw_handle() as HANDLE, &mut exit_code) } == 0 {
        return Err(last_error());
    }
    // A command that exits while leaving background descendants does not own
    // a detached capability. Stop that remainder before returning output.
    unsafe {
        TerminateJobObject(job.0.as_raw_handle() as HANDLE, 0);
        WaitForSingleObject(job.0.as_raw_handle() as HANDLE, INFINITE);
    }
    drop(job);

    let stdout = stdout_task
        .join()
        .map_err(|_| io::Error::other("sandbox bridge stdout reader panicked"))??;
    let stderr = stderr_task
        .join()
        .map_err(|_| io::Error::other("sandbox bridge stderr reader panicked"))??;
    Ok((exit_code as i32, stdout, stderr))
}

fn spawn_in_workspace_sandbox(
    sandbox: Arc<WindowsWorkspaceSandbox>,
    program: &OsStr,
    args: &[OsString],
    raw_arg: Option<&OsStr>,
    cwd: &Path,
    env_remove: &[OsString],
    env_set: &[(OsString, OsString)],
    read_only_paths: &[PathBuf],
) -> io::Result<WindowsSandboxChild> {
    validate_no_nul(program)?;
    validate_no_nul(cwd.as_os_str())?;
    for arg in args {
        validate_no_nul(arg)?;
    }
    if let Some(raw_arg) = raw_arg {
        validate_no_nul(raw_arg)?;
    }
    let cwd = cwd.canonicalize()?;
    if !cwd.starts_with(&sandbox.workspace) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "child working directory resolves outside the selected workspace",
        ));
    }

    let workspace = &sandbox.workspace;
    let temp_dir = ensure_workspace_directory(workspace, TEMP_DIR_NAME)?;
    let cache_dir = ensure_workspace_directory(workspace, CACHE_DIR_NAME)?;
    let cargo_target_dir = ensure_workspace_directory(&cache_dir, "cargo-target")?;

    for path in read_only_paths {
        let path = path.canonicalize()?;
        let runtime_root = crate::db::arcana_dir()
            .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?
            .canonicalize()?
            .join("python-runtimes");
        if !path.starts_with(&runtime_root) || !path.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only runtime grants must name a staged Arcana Python runtime directory",
            ));
        }
        // PSEC grants this exact staged runtime as read-only below.
    }

    let mut child_env = environment_block(
        env_remove,
        env_set,
        &temp_dir,
        &cache_dir,
        &cargo_target_dir,
    )?;
    let (stdin, stdout_read, stdout_write, stderr_read, stderr_write) =
        create_standard_handles()
            .map_err(|error| stage_error("create child standard handles", error))?;
    let inherited = [
        stdin.as_raw_handle() as HANDLE,
        stdout_write.as_raw_handle() as HANDLE,
        stderr_write.as_raw_handle() as HANDLE,
    ];
    let mut psec_read_only = read_only_paths.to_vec();
    for root in read_execute_roots(program, &cwd, workspace, env_set) {
        if root.path.is_dir() && !is_os_managed_directory(&root.path) {
            let path = root.path.canonicalize()?;
            if !path.starts_with(workspace) {
                psec_read_only.push(path);
            }
        }
    }
    let security_environment =
        super::psec::ProcessSecurityEnvironment::create(workspace, &psec_read_only)
            .map_err(|error| stage_error("create strict PSEC process policy", error))?;
    let mut attributes = AttributeList::new(2)?;
    attributes.update(
        PROC_THREAD_ATTRIBUTE_SECURITY_ENVIRONMENT_VALUE,
        &security_environment.raw_handle(),
    )?;
    attributes.update(PROC_THREAD_ATTRIBUTE_HANDLE_LIST_VALUE, &inherited)?;

    let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = stdin.as_raw_handle() as HANDLE;
    startup.StartupInfo.hStdOutput = stdout_write.as_raw_handle() as HANDLE;
    startup.StartupInfo.hStdError = stderr_write.as_raw_handle() as HANDLE;
    startup.lpAttributeList = attributes.as_mut_ptr();

    let (application, mut command_line) = command_line(program, args, raw_arg, &cwd)?;
    let current_dir = wide_null(&create_process_directory(&cwd));
    let mut process_info: PROCESS_INFORMATION = unsafe { zeroed() };
    let created = unsafe {
        CreateProcessW(
            application.as_ptr(),
            command_line.as_mut_ptr(),
            null(),
            null(),
            1,
            CREATE_SUSPENDED
                | EXTENDED_STARTUPINFO_PRESENT
                | CREATE_UNICODE_ENVIRONMENT
                | CREATE_NO_WINDOW,
            child_env.as_mut_ptr().cast(),
            current_dir.as_ptr(),
            &startup.StartupInfo,
            &mut process_info,
        )
    };
    if created == 0 {
        return Err(stage_error("CreateProcessW PSEC launch", last_error()));
    }

    let process = unsafe { OwnedHandle::from_raw_handle(process_info.hProcess as RawHandle) };
    let thread = unsafe { OwnedHandle::from_raw_handle(process_info.hThread as RawHandle) };
    let job = match create_kill_on_close_job() {
        Ok(job) => JobGuard(job),
        Err(error) => {
            unsafe {
                windows_sys::Win32::System::Threading::TerminateProcess(
                    process.as_raw_handle() as HANDLE,
                    1,
                );
                WaitForSingleObject(process.as_raw_handle() as HANDLE, INFINITE);
            }
            return Err(error);
        }
    };
    if unsafe {
        AssignProcessToJobObject(
            job.0.as_raw_handle() as HANDLE,
            process.as_raw_handle() as HANDLE,
        )
    } == 0
    {
        let error = last_error();
        unsafe {
            windows_sys::Win32::System::Threading::TerminateProcess(
                process.as_raw_handle() as HANDLE,
                1,
            );
            WaitForSingleObject(process.as_raw_handle() as HANDLE, INFINITE);
        }
        return Err(stage_error("assign child to kill-on-drop job", error));
    }
    if unsafe { ResumeThread(thread.as_raw_handle() as HANDLE) } == u32::MAX {
        let error = last_error();
        unsafe {
            TerminateJobObject(job.0.as_raw_handle() as HANDLE, 1);
            WaitForSingleObject(job.0.as_raw_handle() as HANDLE, INFINITE);
        }
        return Err(error);
    }

    drop(stdin);
    drop(stdout_write);
    drop(stderr_write);
    Ok(WindowsSandboxChild {
        process,
        stdout: Some(Box::new(unsafe { async_file(stdout_read) })),
        stderr: Some(Box::new(unsafe { async_file(stderr_read) })),
        bridge_stdout: None,
        bridge_stderr: None,
        bridge_files: None,
        _job: job,
        _sandbox: sandbox,
        _security_environment: Some(security_environment),
        _sbox_specification: None,
        _sbox_identity: None,
    })
}

type ExperimentalCreateProcessInSandbox = unsafe extern "system" fn(
    *const u16,
    *mut u16,
    *const SECURITY_ATTRIBUTES,
    *const SECURITY_ATTRIBUTES,
    i32,
    u32,
    *mut std::ffi::c_void,
    *const u16,
    *const STARTUPINFOW,
    *const u16,
    *const std::ffi::c_void,
    u32,
    *mut PROCESS_INFORMATION,
) -> i32;

fn experimental_create_process_api() -> io::Result<ExperimentalCreateProcessInSandbox> {
    verify_supported_windows_build()?;
    let module_name = wide_null(OsStr::new("processmodel.dll"));
    let module = unsafe {
        LoadLibraryExW(
            module_name.as_ptr(),
            null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    };
    if module.is_null() {
        return Err(stage_error(
            "load Windows process sandbox runtime",
            last_error(),
        ));
    }
    let proc = unsafe { GetProcAddress(module, b"Experimental_CreateProcessInSandbox\0".as_ptr()) }
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "Windows processmodel.dll does not export Experimental_CreateProcessInSandbox",
            )
        })?;
    // SAFETY: the ABI is copied from Microsoft's published declaration. The
    // system32 module remains loaded for the process lifetime.
    Ok(unsafe { std::mem::transmute_copy(&proc) })
}

fn verify_supported_windows_build() -> io::Result<()> {
    type RtlGetVersionFn = unsafe extern "system" fn(*mut OSVERSIONINFOW) -> i32;
    let module_name = wide_null(OsStr::new("ntdll.dll"));
    let module = unsafe {
        LoadLibraryExW(
            module_name.as_ptr(),
            null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    };
    if module.is_null() {
        return Err(stage_error("load Windows version runtime", last_error()));
    }
    let proc = unsafe { GetProcAddress(module, b"RtlGetVersion\0".as_ptr()) }.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows does not expose RtlGetVersion for sandbox compatibility checks",
        )
    })?;
    // SAFETY: RtlGetVersion accepts the documented OSVERSIONINFOW structure.
    let get_version: RtlGetVersionFn = unsafe { std::mem::transmute_copy(&proc) };
    let mut version = OSVERSIONINFOW::default();
    version.dwOSVersionInfoSize = size_of::<OSVERSIONINFOW>() as u32;
    let status = unsafe { get_version(&mut version) };
    if status != 0 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("RtlGetVersion failed with NTSTATUS 0x{:08X}", status as u32),
        ));
    }
    if !supports_windows_sbox(version.dwMajorVersion, version.dwBuildNumber) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "Windows sandbox runner requires Windows 11 build 22000 or newer; found {}.{} build {}",
                version.dwMajorVersion, version.dwMinorVersion, version.dwBuildNumber
            ),
        ));
    }
    Ok(())
}

fn supports_windows_sbox(major: u32, build: u32) -> bool {
    major > 10 || (major == 10 && build >= 22_000)
}

fn spawn_experimental_bridge(
    sandbox: Arc<WindowsWorkspaceSandbox>,
    program: &OsStr,
    args: &[OsString],
    raw_arg: Option<&OsStr>,
    cwd: &Path,
    env_remove: &[OsString],
    env_set: &[(OsString, OsString)],
    read_only_paths: &[PathBuf],
) -> io::Result<WindowsSandboxChild> {
    validate_no_nul(program)?;
    validate_no_nul(cwd.as_os_str())?;
    for arg in args {
        validate_no_nul(arg)?;
    }
    if let Some(raw_arg) = raw_arg {
        validate_no_nul(raw_arg)?;
    }
    let cwd = cwd.canonicalize()?;
    if !cwd.starts_with(&sandbox.workspace) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "child working directory resolves outside the selected workspace",
        ));
    }

    let workspace = &sandbox.workspace;
    let temp_dir = ensure_workspace_directory(workspace, TEMP_DIR_NAME)?;
    let cache_dir = ensure_workspace_directory(workspace, CACHE_DIR_NAME)?;
    let cargo_target_dir = ensure_workspace_directory(&cache_dir, "cargo-target")?;
    let mut child_env = environment_block(
        env_remove,
        env_set,
        &temp_dir,
        &cache_dir,
        &cargo_target_dir,
    )?;

    let process_executable = bridge_executable()?.canonicalize()?;
    let executable_root = process_executable.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Arcana bridge executable has no parent directory",
        )
    })?;
    let mut read_roots = read_only_paths.to_vec();
    read_roots.push(executable_root.to_path_buf());
    // Windows loader and inbox runtime dependencies are part of the trusted
    // execution environment. Keep this scoped to System32 instead of granting
    // the entire OS volume; selected external toolchains are added below.
    read_roots.push(system_directory()?);
    for root in read_execute_roots(program, &cwd, workspace, env_set) {
        if root.required && !root.path.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "required selected runtime directory is unavailable: {}",
                    root.path.display()
                ),
            ));
        }
        if root.path.is_dir() {
            read_roots.push(root.path.canonicalize()?);
        }
    }
    let specification = super::windows_sbox::build_specification(workspace, &read_roots)?;
    let identity = wide_null(OsStr::new(&format!(
        "ArcanaSandbox-{}",
        uuid::Uuid::new_v4()
    )));

    let request_id = uuid::Uuid::new_v4();
    let request_path = temp_dir.join(format!(".arcana-bridge-request-{request_id}.json"));
    let output_path = temp_dir.join(format!(".arcana-bridge-result-{request_id}.json"));
    let bridge_files = BridgeFiles {
        request_path: request_path.clone(),
        output_path: output_path.clone(),
    };
    let request = BridgeRequest {
        version: 1,
        program: os_to_string(program)?,
        args: args
            .iter()
            .map(|value| os_to_string(value))
            .collect::<io::Result<_>>()?,
        raw_arg: raw_arg.map(os_to_string).transpose()?,
        cwd: os_to_string(cwd.as_os_str())?,
        workspace: os_to_string(workspace.as_os_str())?,
        output_path: os_to_string(output_path.as_os_str())?,
    };
    let request_bytes = serde_json::to_vec(&request).map_err(io::Error::other)?;
    if request_bytes.len() as u64 > MAX_BRIDGE_REQUEST_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "project command exceeds the sandbox bridge request bound",
        ));
    }
    let mut request_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .share_mode(0)
        .open(&request_path)?;
    use std::io::Write;
    request_file.write_all(&request_bytes)?;
    request_file.sync_all()?;
    drop(request_file);

    let request_arg = os_to_string(request_path.as_os_str())?;
    let bridge_args = [
        OsString::from(BRIDGE_ENTRYPOINT_ARG),
        OsString::from(request_arg),
    ];
    let (application, mut command_line) = command_line(
        process_executable.as_os_str(),
        &bridge_args,
        None,
        workspace,
    )?;
    let current_dir = wide_null(workspace.as_os_str());
    let identity = identity;
    let specification_size = u32::try_from(specification.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "SBOX policy exceeds the API size limit",
        )
    })?;
    let api = experimental_create_process_api()?;
    let mut startup: STARTUPINFOW = unsafe { zeroed() };
    startup.cb = size_of::<STARTUPINFOW>() as u32;
    let mut process_info: PROCESS_INFORMATION = unsafe { zeroed() };
    let created = unsafe {
        api(
            application.as_ptr(),
            command_line.as_mut_ptr(),
            null(),
            null(),
            0,
            CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW,
            child_env.as_mut_ptr().cast(),
            current_dir.as_ptr(),
            &startup,
            identity.as_ptr(),
            specification.as_ptr().cast(),
            specification_size,
            &mut process_info,
        )
    };
    if created == 0 {
        return Err(stage_error(
            "Experimental_CreateProcessInSandbox (inheritHandles=false)",
            last_error(),
        ));
    }
    let process = unsafe { OwnedHandle::from_raw_handle(process_info.hProcess as RawHandle) };
    let thread_handle = unsafe { OwnedHandle::from_raw_handle(process_info.hThread as RawHandle) };
    let job = match create_kill_on_close_job() {
        Ok(job) => JobGuard(job),
        Err(error) => {
            unsafe {
                windows_sys::Win32::System::Threading::TerminateProcess(
                    process.as_raw_handle() as HANDLE,
                    BRIDGE_FAILURE_EXIT as u32,
                );
                WaitForSingleObject(process.as_raw_handle() as HANDLE, INFINITE);
            }
            return Err(error);
        }
    };
    if unsafe {
        AssignProcessToJobObject(
            job.0.as_raw_handle() as HANDLE,
            process.as_raw_handle() as HANDLE,
        )
    } == 0
    {
        let error = last_error();
        unsafe {
            windows_sys::Win32::System::Threading::TerminateProcess(
                process.as_raw_handle() as HANDLE,
                BRIDGE_FAILURE_EXIT as u32,
            );
            WaitForSingleObject(process.as_raw_handle() as HANDLE, INFINITE);
        }
        return Err(stage_error(
            "assign sandbox bridge kill-on-close job",
            error,
        ));
    }
    if unsafe { ResumeThread(thread_handle.as_raw_handle() as HANDLE) } == u32::MAX {
        let error = last_error();
        unsafe {
            TerminateJobObject(job.0.as_raw_handle() as HANDLE, BRIDGE_FAILURE_EXIT as u32);
            WaitForSingleObject(job.0.as_raw_handle() as HANDLE, INFINITE);
        }
        return Err(error);
    }

    let (stdout_reader, stdout_writer) = tokio::io::duplex(MAX_BRIDGE_CAPTURE_BYTES);
    let (stderr_reader, stderr_writer) = tokio::io::duplex(MAX_BRIDGE_CAPTURE_BYTES);
    Ok(WindowsSandboxChild {
        process,
        stdout: Some(Box::new(stdout_reader)),
        stderr: Some(Box::new(stderr_reader)),
        bridge_stdout: Some(stdout_writer),
        bridge_stderr: Some(stderr_writer),
        bridge_files: Some(bridge_files),
        _job: job,
        _sandbox: sandbox,
        _security_environment: None,
        _sbox_specification: Some(specification),
        _sbox_identity: Some(identity),
    })
}

fn bridge_executable() -> io::Result<PathBuf> {
    #[cfg(test)]
    {
        // Unit-test executables use Rust's test-harness main and cannot enter
        // the private bridge mode. Exercise the same CLI executable shipped to
        // users instead; the test command builds it before running this test.
        let test_executable = std::env::current_exe()?;
        if let Some(debug_directory) = test_executable.parent().and_then(Path::parent) {
            let cli_executable = debug_directory.join("arcanactl.exe");
            if cli_executable.is_file() {
                return Ok(cli_executable);
            }
        }
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Windows sandbox bridge test requires target/debug/arcanactl.exe; build the CLI before running this test",
        ));
    }
    #[cfg(not(test))]
    {
        std::env::current_exe()
    }
}

fn os_to_string(value: &OsStr) -> io::Result<String> {
    value.to_str().map(str::to_owned).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "sandbox bridge cannot serialize a non-Unicode Windows command value",
        )
    })
}

#[cfg(test)]
fn grant_ancestor_traversal(
    access: &mut AccessPolicy,
    selected_path: &Path,
    workspace_root: &Path,
) -> io::Result<()> {
    let selected_path = selected_path.canonicalize()?;
    let workspace_root = workspace_root.canonicalize()?;
    if !selected_path.starts_with(&workspace_root) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "selected path is outside the workspace",
        ));
    }
    let traversal_only = FILE_TRAVERSE | FILE_READ_ATTRIBUTES;
    let mut reached_workspace = false;
    for ancestor in selected_path.ancestors() {
        // Do not modify the volume root. It is protected by the host and denied
        // even to this user; failure stays fail-closed for selected toolchains
        // that require an inaccessible ancestor.
        if ancestor.parent().is_some() && ancestor.is_dir() {
            access
                .grant_with_inheritance(ancestor, traversal_only, 0)
                .map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!(
                            "could not grant path metadata on {}: {error}",
                            ancestor.display()
                        ),
                    )
                })?;
        }
        if ancestor == workspace_root {
            reached_workspace = true;
        }
    }
    if reached_workspace {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "workspace is not an ancestor of the selected path",
        ))
    }
}

fn canonical_workspace(workspace: &Path) -> io::Result<PathBuf> {
    let canonical = workspace.canonicalize()?;
    if !canonical.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "selected workspace is not a directory",
        ));
    }
    Ok(canonical)
}

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

fn stage_error(stage: &str, error: io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{stage}: {error}"))
}

#[cfg(test)]
struct AccessPolicy {
    name: Vec<u16>,
    sid: Sid,
    granted_paths: Vec<PathGrant>,
}

#[cfg(test)]
struct PathGrant {
    path: PathBuf,
    mask: u32,
    inheritance: u32,
}

#[cfg(test)]
fn grant_toolchain_file_access(
    access: &mut AccessPolicy,
    path: &Path,
    _workspace: &Path,
) -> io::Result<()> {
    let path = path.canonicalize()?;
    // The AppContainer needs traverse/list permission on this exact runtime
    // directory before individually allowlisted files are usable. Do not
    // inherit this grant into sibling or parent toolchain trees.
    access.grant_with_inheritance(&path, FILE_GENERIC_READ | FILE_GENERIC_EXECUTE, 0)?;
    for entry in std::fs::read_dir(&path)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file = entry.path().canonicalize()?;
        if file.parent() != Some(path.as_path()) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "selected toolchain file resolves outside its runtime directory",
            ));
        }
        access.grant_with_inheritance(&file, FILE_GENERIC_READ | FILE_GENERIC_EXECUTE, 0)?;
    }
    Ok(())
}

#[cfg(test)]
fn profile_root_for_workspace(
    workspace: &Path,
    user_profile: Option<&OsStr>,
    home_drive: Option<&OsStr>,
    home_path: Option<&OsStr>,
) -> Option<PathBuf> {
    let workspace = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let profile = user_profile
        .map(PathBuf::from)
        .and_then(|path| path.canonicalize().ok());
    let profile_is_test_home = profile
        .as_ref()
        .is_some_and(|profile| profile.starts_with(&workspace));
    if profile_is_test_home {
        if let (Some(drive), Some(path)) = (home_drive, home_path) {
            let stable_profile = PathBuf::from(format!(
                "{}{}",
                drive.to_string_lossy(),
                path.to_string_lossy()
            ));
            if let Ok(stable_profile) = stable_profile.canonicalize() {
                return Some(stable_profile);
            }
        }
    }
    profile
}

#[cfg(test)]
impl AccessPolicy {
    fn new() -> io::Result<Self> {
        let name = format!("{}{}", APP_CONTAINER_PREFIX, uuid::Uuid::new_v4());
        let name = wide_null(OsStr::new(&name));
        let display = wide_null(OsStr::new("Arcana agent process"));
        let description = wide_null(OsStr::new("Restricted child processes launched by Arcana"));
        let mut sid = null_mut();
        let status = unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                display.as_ptr(),
                description.as_ptr(),
                null(),
                0,
                &mut sid,
            )
        };
        if status < 0 {
            return Err(io::Error::from_raw_os_error(
                (status as u32 & 0xffff) as i32,
            ));
        }
        if sid.is_null() {
            let result =
                unsafe { DeriveAppContainerSidFromAppContainerName(name.as_ptr(), &mut sid) };
            if result < 0 || sid.is_null() {
                unsafe { DeleteAppContainerProfile(name.as_ptr()) };
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "could not derive unique Arcana AppContainer SID (HRESULT 0x{:08x})",
                        result as u32
                    ),
                ));
            }
        }
        Ok(Self {
            name,
            sid: Sid(sid),
            granted_paths: Vec::new(),
        })
    }

    fn grant_with_inheritance(
        &mut self,
        path: &Path,
        mask: u32,
        inheritance: u32,
    ) -> io::Result<()> {
        let path = path.canonicalize()?;
        if let Some(existing) = self
            .granted_paths
            .iter_mut()
            .find(|existing| existing.path == path && existing.inheritance == inheritance)
        {
            let combined_mask = existing.mask | mask;
            if combined_mask == existing.mask {
                return Ok(());
            }
            grant_directory_access(&path, self.sid.0, combined_mask, inheritance)?;
            existing.mask = combined_mask;
            return Ok(());
        }
        grant_directory_access(&path, self.sid.0, mask, inheritance)?;
        self.granted_paths.push(PathGrant {
            path,
            mask,
            inheritance,
        });
        Ok(())
    }
}

#[cfg(test)]
impl Drop for AccessPolicy {
    fn drop(&mut self) {
        let mut failed = false;
        for grant in self.granted_paths.iter().rev() {
            if let Err(error) = revoke_directory_access(&grant.path, self.sid.0, grant.inheritance)
            {
                failed = true;
                tracing::warn!(path = %grant.path.display(), %error, "failed to remove Arcana AppContainer ACL grant");
            }
        }
        let deleted = unsafe { DeleteAppContainerProfile(self.name.as_ptr()) };
        if deleted < 0 {
            tracing::warn!(
                hresult = format_args!("0x{:08x}", deleted as u32),
                "failed to delete unique Arcana AppContainer profile"
            );
        }
        if failed {
            tracing::warn!(
                "one or more unique AppContainer ACL entries may remain; their SID is not reused"
            );
        }
    }
}

#[cfg(test)]
struct Sid(windows_sys::Win32::Security::PSID);

// SAFETY: the SID allocation is owned by this value and its pointer is only
// passed to synchronous Windows APIs. Those APIs do not retain the pointer;
// moving the owner between Tokio worker threads is therefore safe. `Sid` is
// deliberately not `Sync`, so concurrent access cannot occur.
#[cfg(test)]
unsafe impl Send for Sid {}

#[cfg(test)]
impl Drop for Sid {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { windows_sys::Win32::Security::FreeSid(self.0) };
        }
    }
}

struct JobGuard(OwnedHandle);

impl Drop for JobGuard {
    fn drop(&mut self) {
        unsafe {
            if TerminateJobObject(self.0.as_raw_handle() as HANDLE, 1) == 0 {
                tracing::warn!(error = %last_error(), "failed to terminate sandbox job during cleanup");
                return;
            }
            if WaitForSingleObject(self.0.as_raw_handle() as HANDLE, INFINITE) == u32::MAX {
                tracing::warn!(error = %last_error(), "failed waiting for sandbox job cleanup");
            }
        }
    }
}

#[cfg(test)]
fn grant_directory_access(
    path: &Path,
    sid: windows_sys::Win32::Security::PSID,
    mask: u32,
    inheritance: u32,
) -> io::Result<()> {
    let path = wide_null(path.as_os_str());
    let mut old_acl = null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    let get = unsafe {
        GetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut old_acl,
            null_mut(),
            &mut descriptor,
        )
    };
    if get != 0 {
        return Err(io::Error::from_raw_os_error(get as i32));
    }
    if old_acl.is_null() {
        unsafe { LocalFree(descriptor as HLOCAL) };
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "refusing to replace a null or absent DACL on {}",
                PathBuf::from(OsString::from_wide(&path[..path.len() - 1])).display()
            ),
        ));
    }

    let mut access = EXPLICIT_ACCESS_W::default();
    access.grfAccessPermissions = mask;
    access.grfAccessMode = GRANT_ACCESS;
    access.grfInheritance = inheritance;
    access.Trustee.TrusteeForm = TRUSTEE_IS_SID;
    access.Trustee.TrusteeType = TRUSTEE_IS_USER;
    access.Trustee.ptstrName = sid.cast();
    let mut new_acl = null_mut();
    let merge = unsafe { SetEntriesInAclW(1, &access, old_acl, &mut new_acl) };
    unsafe { LocalFree(descriptor as HLOCAL) };
    if merge != 0 {
        return Err(io::Error::from_raw_os_error(merge as i32));
    }
    let set = unsafe {
        SetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            new_acl,
            null_mut(),
        )
    };
    unsafe { LocalFree(new_acl as HLOCAL) };
    if set != 0 {
        return Err(io::Error::from_raw_os_error(set as i32));
    }
    Ok(())
}

#[cfg(test)]
fn revoke_directory_access(
    path: &Path,
    sid: windows_sys::Win32::Security::PSID,
    inheritance: u32,
) -> io::Result<()> {
    let path = wide_null(path.as_os_str());
    let mut old_acl = null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    let get = unsafe {
        GetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut old_acl,
            null_mut(),
            &mut descriptor,
        )
    };
    if get != 0 {
        return Err(io::Error::from_raw_os_error(get as i32));
    }
    if old_acl.is_null() {
        unsafe { LocalFree(descriptor as HLOCAL) };
        return Ok(());
    }
    let mut access = EXPLICIT_ACCESS_W::default();
    access.grfAccessMode = REVOKE_ACCESS;
    access.grfInheritance = inheritance;
    access.Trustee.TrusteeForm = TRUSTEE_IS_SID;
    access.Trustee.TrusteeType = TRUSTEE_IS_USER;
    access.Trustee.ptstrName = sid.cast();
    let mut new_acl = null_mut();
    let merge = unsafe { SetEntriesInAclW(1, &access, old_acl, &mut new_acl) };
    unsafe { LocalFree(descriptor as HLOCAL) };
    if merge != 0 {
        return Err(io::Error::from_raw_os_error(merge as i32));
    }
    let set = unsafe {
        SetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            new_acl,
            null_mut(),
        )
    };
    unsafe { LocalFree(new_acl as HLOCAL) };
    if set != 0 {
        return Err(io::Error::from_raw_os_error(set as i32));
    }
    Ok(())
}

struct ReadExecuteRoot {
    path: PathBuf,
    required: bool,
    file_children_only: bool,
}

fn read_execute_roots(
    program: &OsStr,
    cwd: &Path,
    _workspace: &Path,
    env_set: &[(OsString, OsString)],
) -> Vec<ReadExecuteRoot> {
    let mut roots = Vec::new();
    let program_lower = program.to_string_lossy().to_ascii_lowercase();
    let program_stem = Path::new(program)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let is_cargo = program_stem == "cargo";
    let is_go = program_stem == "go";
    let system_command = matches!(program_lower.as_str(), "cmd" | "cmd.exe");
    let selected_program = if system_command {
        system_directory().ok().map(|path| path.join("cmd.exe"))
    } else {
        resolve_selected_program(program, cwd)
    };
    if let Some(program) = selected_program {
        if let Some(parent) = program.parent().filter(|path| !path.as_os_str().is_empty()) {
            roots.push(ReadExecuteRoot {
                path: parent.to_path_buf(),
                required: true,
                file_children_only: true,
            });
        }
    }
    if let Some(parent) = Path::new(program)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        roots.push(ReadExecuteRoot {
            path: parent.to_path_buf(),
            required: true,
            file_children_only: true,
        });
    }
    // Do not grant every directory on PATH. It can contain unrelated user data
    // and large trees; PSEC receives only the selected executable and runtime
    // support roots required by this toolchain.
    if is_cargo {
        if let Some(bin) = Path::new(program).parent() {
            roots.push(ReadExecuteRoot {
                path: bin
                    .join("..")
                    .join("lib")
                    .join("rustlib")
                    .join("x86_64-pc-windows-msvc")
                    .join("lib"),
                required: true,
                file_children_only: true,
            });
        }
        let selected_path = env_set
            .iter()
            .find(|(key, _)| key.to_string_lossy().eq_ignore_ascii_case("PATH"))
            .map(|(_, value)| value.clone())
            .or_else(|| std::env::var_os("PATH"));
        if let Some(linker) = selected_path.and_then(|path| {
            std::env::split_paths(&path)
                .map(|directory| directory.join("link.exe"))
                .find(|candidate| candidate.is_file())
        }) {
            if let Some(parent) = linker.parent() {
                roots.push(ReadExecuteRoot {
                    path: parent.to_path_buf(),
                    required: true,
                    file_children_only: true,
                });
            }
        }
    }
    if is_go {
        // Go module caches are dependency sources, not toolchain runtime.
        // P4-61 stages locked dependencies under the workspace instead of
        // granting this shared cache to untrusted build/test processes.
    }
    let mut unique: Vec<ReadExecuteRoot> = Vec::new();
    for root in roots {
        if let Some(existing) = unique.iter_mut().find(|existing| {
            existing
                .path
                .to_string_lossy()
                .eq_ignore_ascii_case(&root.path.to_string_lossy())
        }) {
            existing.required |= root.required;
            existing.file_children_only &= root.file_children_only;
        } else {
            unique.push(root);
        }
    }
    unique.sort_by(|left, right| left.path.cmp(&right.path));
    unique
}

fn resolve_selected_program(program: &OsStr, cwd: &Path) -> Option<PathBuf> {
    let program_path = Path::new(program);
    if program_path.components().count() > 1 || program_path.is_absolute() {
        return Some(if program_path.is_absolute() {
            program_path.to_path_buf()
        } else {
            cwd.join(program_path)
        });
    }
    let extensions = std::env::var_os("PATHEXT")
        .map(|value| {
            value
                .to_string_lossy()
                .split(';')
                .map(OsString::from)
                .collect::<Vec<_>>()
        })
        .filter(|extensions| !extensions.is_empty())
        .unwrap_or_else(|| {
            vec![
                OsString::from(".COM"),
                OsString::from(".EXE"),
                OsString::from(".BAT"),
                OsString::from(".CMD"),
            ]
        });
    let has_extension = program_path.extension().is_some();
    let candidates = if has_extension {
        vec![program.to_os_string()]
    } else {
        extensions
            .into_iter()
            .map(|extension| {
                let mut candidate = program.to_os_string();
                candidate.push(extension);
                candidate
            })
            .collect()
    };
    let search_roots = std::iter::once(cwd.to_path_buf()).chain(
        std::env::var_os("PATH")
            .into_iter()
            .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>()),
    );
    for root in search_roots {
        for candidate in &candidates {
            let candidate = root.join(candidate);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn is_os_managed_directory(path: &Path) -> bool {
    ["SystemRoot", "ProgramFiles", "ProgramFiles(x86)"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .filter_map(|root| root.canonicalize().ok())
        .any(|root| {
            path.canonicalize()
                .map(|path| path.starts_with(root))
                .unwrap_or(false)
        })
}

fn environment_block(
    env_remove: &[OsString],
    env_set: &[(OsString, OsString)],
    temp_dir: &Path,
    cache_dir: &Path,
    cargo_target_dir: &Path,
) -> io::Result<Vec<u16>> {
    let removed = env_remove
        .iter()
        .map(|key| key.to_string_lossy().to_ascii_lowercase())
        .collect::<Vec<_>>();
    let mut values = std::env::vars_os()
        .filter(|(key, _)| {
            !removed
                .iter()
                .any(|removed| key.to_string_lossy().eq_ignore_ascii_case(removed))
        })
        .collect::<Vec<_>>();
    for (key, value) in env_set {
        let key_string = key.to_string_lossy();
        if removed
            .iter()
            .any(|removed| key_string.eq_ignore_ascii_case(removed))
        {
            continue;
        }
        values
            .retain(|(existing, _)| !existing.to_string_lossy().eq_ignore_ascii_case(&key_string));
        values.push((key.clone(), value.clone()));
    }
    let workspace_overrides = vec![
        ("TEMP", temp_dir.as_os_str().to_os_string()),
        ("TMP", temp_dir.as_os_str().to_os_string()),
        (
            "CARGO_TARGET_DIR",
            cargo_target_dir.as_os_str().to_os_string(),
        ),
        // Keep Cargo's incremental state predictable inside the selected
        // workspace cache on Windows.
        ("CARGO_INCREMENTAL", OsString::from("0")),
        ("NPM_CONFIG_CACHE", cache_dir.join("npm").into_os_string()),
        ("YARN_CACHE_FOLDER", cache_dir.join("yarn").into_os_string()),
        ("PNPM_STORE_DIR", cache_dir.join("pnpm").into_os_string()),
        (
            "BUN_INSTALL_CACHE_DIR",
            cache_dir.join("bun").into_os_string(),
        ),
        (
            "PIP_CACHE_DIR",
            cache_dir.join("python-pip").into_os_string(),
        ),
        (
            "PYTHONPYCACHEPREFIX",
            cache_dir.join("python-bytecode").into_os_string(),
        ),
        (
            "PYTHONUSERBASE",
            cache_dir.join("python-user").into_os_string(),
        ),
        ("UV_CACHE_DIR", cache_dir.join("python-uv").into_os_string()),
        (
            "POETRY_CACHE_DIR",
            cache_dir.join("python-poetry").into_os_string(),
        ),
        ("GOCACHE", cache_dir.join("go-build").into_os_string()),
        ("GOMODCACHE", cache_dir.join("go-mod").into_os_string()),
        ("XDG_CACHE_HOME", cache_dir.join("xdg").into_os_string()),
    ];
    for (key, value) in workspace_overrides {
        values.retain(|(existing, _)| !existing.to_string_lossy().eq_ignore_ascii_case(key));
        values.push((OsString::from(key), value));
    }
    values.sort_by(|a, b| {
        a.0.to_string_lossy()
            .to_ascii_lowercase()
            .cmp(&b.0.to_string_lossy().to_ascii_lowercase())
    });

    let mut block = Vec::new();
    for (key, value) in values {
        block.extend(key.encode_wide());
        block.push(b'=' as u16);
        block.extend(value.encode_wide());
        block.push(0);
    }
    block.push(0);
    if block.len() == 1 {
        block.push(0);
    }
    Ok(block)
}

fn create_standard_handles() -> io::Result<(
    OwnedHandle,
    OwnedHandle,
    OwnedHandle,
    OwnedHandle,
    OwnedHandle,
)> {
    let inheritable = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: null_mut(),
        bInheritHandle: 1,
    };
    let mut stdout_read = null_mut();
    let mut stdout_write = null_mut();
    if unsafe { CreatePipe(&mut stdout_read, &mut stdout_write, &inheritable, 0) } == 0 {
        return Err(last_error());
    }
    let stdout_read = unsafe { OwnedHandle::from_raw_handle(stdout_read as RawHandle) };
    let stdout_write = unsafe { OwnedHandle::from_raw_handle(stdout_write as RawHandle) };
    if unsafe {
        windows_sys::Win32::Foundation::SetHandleInformation(
            stdout_read.as_raw_handle() as HANDLE,
            HANDLE_FLAG_INHERIT,
            0,
        )
    } == 0
    {
        return Err(last_error());
    }

    let mut stderr_read = null_mut();
    let mut stderr_write = null_mut();
    if unsafe { CreatePipe(&mut stderr_read, &mut stderr_write, &inheritable, 0) } == 0 {
        return Err(last_error());
    }
    let stderr_read = unsafe { OwnedHandle::from_raw_handle(stderr_read as RawHandle) };
    let stderr_write = unsafe { OwnedHandle::from_raw_handle(stderr_write as RawHandle) };
    if unsafe {
        windows_sys::Win32::Foundation::SetHandleInformation(
            stderr_read.as_raw_handle() as HANDLE,
            HANDLE_FLAG_INHERIT,
            0,
        )
    } == 0
    {
        return Err(last_error());
    }

    let null_name = wide_null(OsStr::new("NUL"));
    let stdin = unsafe {
        CreateFileW(
            null_name.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            &inheritable,
            OPEN_EXISTING,
            0,
            null_mut(),
        )
    };
    if stdin == INVALID_HANDLE_VALUE {
        return Err(last_error());
    }
    let stdin = unsafe { OwnedHandle::from_raw_handle(stdin as RawHandle) };
    Ok((stdin, stdout_read, stdout_write, stderr_read, stderr_write))
}

unsafe fn async_file(handle: OwnedHandle) -> File {
    File::from_std(std::fs::File::from_raw_handle(handle.into_raw_handle()))
}

struct AttributeList {
    storage: Vec<usize>,
    initialized: bool,
}
impl AttributeList {
    fn new(count: u32) -> io::Result<Self> {
        let mut size = 0usize;
        unsafe { InitializeProcThreadAttributeList(null_mut(), count, 0, &mut size) };
        if size == 0 {
            return Err(last_error());
        }
        let mut storage = vec![0usize; size.div_ceil(size_of::<usize>())];
        if unsafe {
            InitializeProcThreadAttributeList(storage.as_mut_ptr().cast(), count, 0, &mut size)
        } == 0
        {
            return Err(last_error());
        }
        Ok(Self {
            storage,
            initialized: true,
        })
    }

    fn update<T>(&mut self, attribute: usize, value: &T) -> io::Result<()> {
        if unsafe {
            UpdateProcThreadAttribute(
                self.as_mut_ptr(),
                0,
                attribute,
                (value as *const T).cast(),
                size_of::<T>(),
                null_mut(),
                null(),
            )
        } == 0
        {
            return Err(last_error());
        }
        Ok(())
    }

    fn as_mut_ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_mut_ptr().cast()
    }
}
impl Drop for AttributeList {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { DeleteProcThreadAttributeList(self.as_mut_ptr()) };
        }
    }
}

fn create_kill_on_close_job() -> io::Result<OwnedHandle> {
    let job = unsafe { CreateJobObjectW(null(), null()) };
    if job.is_null() {
        return Err(last_error());
    }
    let job = unsafe { OwnedHandle::from_raw_handle(job as RawHandle) };
    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if unsafe {
        SetInformationJobObject(
            job.as_raw_handle() as HANDLE,
            JobObjectExtendedLimitInformation,
            (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    } == 0
    {
        return Err(last_error());
    }
    Ok(job)
}

fn command_line(
    program: &OsStr,
    args: &[OsString],
    raw_arg: Option<&OsStr>,
    cwd: &Path,
) -> io::Result<(Vec<u16>, Vec<u16>)> {
    let program_lower = program.to_string_lossy().to_ascii_lowercase();
    let is_batch_shim = program_lower.ends_with(".cmd") || program_lower.ends_with(".bat");
    let application: OsString = if program_lower == "cmd.exe" || program_lower == "cmd" {
        system_directory()?.join("cmd.exe").into_os_string()
    } else if is_batch_shim {
        system_directory()?.join("cmd.exe").into_os_string()
    } else {
        resolve_selected_program(program, cwd)
            .map(PathBuf::into_os_string)
            .unwrap_or_else(|| program.to_os_string())
    };
    let mut line = OsString::new();
    if is_batch_shim {
        line.push(quote_windows_arg(application.as_os_str()));
        line.push(" /d /s /c ");
        line.push(format!("\"\"{}\"", program.to_string_lossy()));
        for arg in args {
            line.push(" ");
            line.push(quote_windows_arg(arg));
        }
        line.push("\"");
    } else {
        line.push(quote_windows_arg(application.as_os_str()));
        for arg in args {
            line.push(" ");
            line.push(quote_windows_arg(arg));
        }
        if let Some(raw) = raw_arg {
            line.push(" ");
            line.push(raw);
        }
    }
    let mut command_line = wide_null(&line);
    command_line.pop();
    Ok((wide_null(&application), command_line))
}

fn quote_windows_arg(value: &OsStr) -> OsString {
    let text = value.to_string_lossy();
    if !text.is_empty() && !text.chars().any(|c| c.is_whitespace() || c == '"') {
        return value.to_os_string();
    }
    let mut quoted = String::from("\"");
    let mut slashes = 0usize;
    for ch in text.chars() {
        match ch {
            '\\' => slashes += 1,
            '"' => {
                quoted.push_str(&"\\".repeat(slashes * 2 + 1));
                quoted.push('"');
                slashes = 0;
            }
            _ => {
                quoted.push_str(&"\\".repeat(slashes));
                quoted.push(ch);
                slashes = 0;
            }
        }
    }
    quoted.push_str(&"\\".repeat(slashes * 2));
    quoted.push('"');
    OsString::from(quoted)
}

fn system_directory() -> io::Result<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    let len = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    if len == 0 || len >= buffer.len() {
        return Err(last_error());
    }
    buffer.truncate(len);
    Ok(PathBuf::from(OsString::from_wide(&buffer)))
}

fn duplicate_handle(handle: RawHandle) -> io::Result<OwnedHandle> {
    let mut duplicate = null_mut();
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            handle as HANDLE,
            GetCurrentProcess(),
            &mut duplicate,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    } == 0
    {
        return Err(last_error());
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(duplicate as RawHandle) })
}

fn wide_null(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

fn create_process_directory(path: &Path) -> OsString {
    const EXTENDED_PREFIX: [u16; 4] = [b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16];
    const EXTENDED_UNC_PREFIX: [u16; 8] = [
        b'\\' as u16,
        b'\\' as u16,
        b'?' as u16,
        b'\\' as u16,
        b'U' as u16,
        b'N' as u16,
        b'C' as u16,
        b'\\' as u16,
    ];
    let wide = path.as_os_str().encode_wide().collect::<Vec<_>>();
    if wide.starts_with(&EXTENDED_UNC_PREFIX) {
        let mut normalized = vec![b'\\' as u16, b'\\' as u16];
        normalized.extend_from_slice(&wide[EXTENDED_UNC_PREFIX.len()..]);
        OsString::from_wide(&normalized)
    } else if wide.starts_with(&EXTENDED_PREFIX) {
        OsString::from_wide(&wide[EXTENDED_PREFIX.len()..])
    } else {
        path.as_os_str().to_os_string()
    }
}

fn validate_no_nul(value: &OsStr) -> io::Result<()> {
    if value.encode_wide().any(|unit| unit == 0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "process arguments and paths cannot contain NUL",
        ));
    }
    Ok(())
}

fn last_error() -> io::Error {
    io::Error::from_raw_os_error(unsafe { GetLastError() } as i32)
}

#[cfg(windows)]
fn exit_status(code: u32) -> std::process::ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    static ACL_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[test]
    fn experimental_runner_requires_a_windows_11_build() {
        assert!(!supports_windows_sbox(6, 96_000));
        assert!(!supports_windows_sbox(10, 21_999));
        assert!(supports_windows_sbox(10, 22_000));
        assert!(supports_windows_sbox(10, 26_200));
        assert!(supports_windows_sbox(11, 1));
    }

    #[test]
    fn bridge_failure_exit_exposes_only_fixed_stage_and_os_code() {
        let code = (BRIDGE_FAILURE_DETAIL_BASE
            + BRIDGE_FAILURE_REQUEST_READ * BRIDGE_FAILURE_STAGE_STRIDE
            + 5) as i32;
        assert_eq!(decode_bridge_failure(code as u32), ("request_read", 5));
        assert_eq!(
            decode_bridge_failure(BRIDGE_FAILURE_EXIT as u32),
            ("bridge_failed", 0)
        );
    }

    #[test]
    fn bridge_stream_capture_keeps_a_bounded_tail_and_total_byte_count() {
        let mut input = vec![b'a'; MAX_BRIDGE_CAPTURE_BYTES + 8192];
        input[MAX_BRIDGE_CAPTURE_BYTES..].fill(b'z');

        let captured = capture_bridge_stream(std::io::Cursor::new(&input))
            .expect("capture the complete bridge stream");

        assert_eq!(captured.total, input.len() as u64);
        assert_eq!(captured.tail.len(), MAX_BRIDGE_CAPTURE_BYTES);
        assert!(captured.tail[..MAX_BRIDGE_CAPTURE_BYTES - 8192]
            .iter()
            .all(|byte| *byte == b'a'));
        assert!(captured.tail[MAX_BRIDGE_CAPTURE_BYTES - 8192..]
            .iter()
            .all(|byte| *byte == b'z'));
    }

    fn acl_test_tempdir(prefix: &str) -> tempfile::TempDir {
        let parent = std::env::current_dir()
            .expect("repository root")
            .join("target")
            .join("p4-acl-tests");
        std::fs::create_dir_all(&parent).expect("create ignored ACL fixture parent");
        tempfile::Builder::new()
            .prefix(prefix)
            .tempdir_in(parent)
            .expect("temporary ACL fixture")
    }

    #[test]
    fn redirected_test_profile_does_not_expand_traversal_above_the_real_profile() {
        let workspace = tempfile::tempdir().expect("temporary workspace");
        let test_profile = workspace.path().join("home");
        std::fs::create_dir(&test_profile).expect("create redirected profile");
        let profile = profile_root_for_workspace(
            workspace.path(),
            Some(test_profile.as_os_str()),
            Some(OsStr::new("C:")),
            Some(OsStr::new(r"\Users\razia")),
        )
        .expect("resolve the host profile from stable Windows variables");
        assert_eq!(
            profile,
            PathBuf::from(r"C:\Users\razia")
                .canonicalize()
                .expect("host profile exists")
        );
    }

    #[test]
    fn cargo_process_uses_only_selected_toolchain_and_linker_roots() {
        let workspace = tempfile::tempdir().expect("isolated workspace");
        let selected_bin = workspace.path().join("rust-toolchain\\bin");
        std::fs::create_dir_all(&selected_bin).expect("selected toolchain bin");
        let cargo = selected_bin.join("cargo.exe");
        let roots = read_execute_roots(
            cargo.as_os_str(),
            &workspace.path().join("project"),
            workspace.path(),
            &[],
        );

        assert!(roots.iter().any(|root| root.path == selected_bin));
        assert!(roots.iter().any(|root| {
            root.path
                .ends_with("rust-toolchain\\bin\\..\\lib\\rustlib\\x86_64-pc-windows-msvc\\lib")
        }));
        assert!(
            roots.len() <= 3,
            "Cargo must not recursively grant every unrelated PATH/cache root"
        );
        assert!(!roots
            .iter()
            .any(|root| root.path.ends_with(".rustup\\toolchains")));
    }

    #[test]
    fn selected_rustup_toolchain_bin_accepts_a_temporary_read_execute_lease() {
        let rustup_home = std::env::var_os("RUSTUP_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(".rustup"))
            });
        let Some(rustup_home) = rustup_home else {
            return;
        };
        let toolchain_root = rustup_home
            .join("toolchains")
            .join("stable-x86_64-pc-windows-msvc")
            .join("bin")
            .canonicalize()
            .expect("selected Rustup toolchain bin resolves");
        if !toolchain_root.is_dir() {
            return;
        }
        let mut access = AccessPolicy::new().expect("unique AppContainer profile");
        let workspace = std::env::current_dir().expect("workspace");
        grant_toolchain_file_access(&mut access, &toolchain_root, &workspace)
            .expect("selected Rustup runtime files should accept exact temporary grants");
        assert!(!access.granted_paths.is_empty());
        assert!(access.granted_paths.iter().all(|grant| {
            (grant.path == toolchain_root || grant.path.parent() == Some(toolchain_root.as_path()))
                && grant.inheritance == 0
        }));
    }

    #[test]
    fn selected_path_ancestors_receive_only_non_inheriting_traversal_grants() {
        let root = acl_test_tempdir("selected-path-");
        let workspace = root.path().join("workspace");
        let nested = workspace.join(".arcana-cache");
        std::fs::create_dir_all(&nested).expect("selected config directory");
        let mut access = AccessPolicy::new().expect("AppContainer profile");

        grant_ancestor_traversal(&mut access, &nested, &workspace)
            .expect("temporary traversal grants");

        let traversal_only = FILE_TRAVERSE | FILE_READ_ATTRIBUTES;
        let granted = access
            .granted_paths
            .iter()
            .filter(|grant| grant.mask == traversal_only)
            .collect::<Vec<_>>();
        let canonical_workspace = workspace.canonicalize().unwrap();
        let canonical_nested = nested.canonicalize().unwrap();
        assert!(granted
            .iter()
            .any(|grant| grant.path == canonical_workspace));
        assert!(granted.iter().any(|grant| grant.path == canonical_nested));
        let canonical_root = root.path().canonicalize().unwrap();
        assert!(granted
            .iter()
            .any(|grant| grant.path == canonical_root.parent().unwrap()));
        let volume_root = canonical_nested.ancestors().last().unwrap();
        assert!(!granted.iter().any(|grant| grant.path == volume_root));
        assert!(granted.iter().all(|grant| {
            canonical_nested
                .ancestors()
                .any(|ancestor| ancestor == grant.path)
        }));
        assert!(granted.iter().all(|grant| grant.inheritance == 0));
        assert!(granted.iter().all(|grant| grant.mask == traversal_only));
    }

    #[test]
    fn appcontainer_acl_cache_upgrades_permissions_for_an_existing_path() {
        let root = tempfile::tempdir().expect("temporary ACL target");
        let mut access = AccessPolicy::new().expect("AppContainer profile");

        access
            .grant_with_inheritance(root.path(), FILE_GENERIC_READ, INHERIT_TO_CHILDREN)
            .expect("initial read grant");
        access
            .grant_with_inheritance(root.path(), WORKSPACE_ACCESS, INHERIT_TO_CHILDREN)
            .expect("upgrade to workspace grant");
        access
            .grant_with_inheritance(root.path(), FILE_GENERIC_READ, INHERIT_TO_CHILDREN)
            .expect("subsumed read grant");

        assert_eq!(access.granted_paths.len(), 1);
        assert_eq!(
            access.granted_paths[0].mask,
            FILE_GENERIC_READ | WORKSPACE_ACCESS
        );
    }

    fn credentials_to_remove() -> Vec<OsString> {
        std::env::vars_os()
            .filter_map(|(key, _)| {
                crate::security::env_scrub::looks_like_a_credential(&key.to_string_lossy())
                    .then_some(key)
            })
            .collect()
    }

    async fn finish(
        mut child: WindowsSandboxChild,
    ) -> (std::process::ExitStatus, Vec<u8>, Vec<u8>) {
        let mut stdout = child.take_stdout().expect("stdout");
        let mut stderr = child.take_stderr().expect("stderr");
        let mut stdout_bytes = Vec::new();
        let mut stderr_bytes = Vec::new();
        let (stdout_result, stderr_result, status) = tokio::join!(
            stdout.read_to_end(&mut stdout_bytes),
            stderr.read_to_end(&mut stderr_bytes),
            child.wait()
        );
        stdout_result.expect("read stdout");
        stderr_result.expect("read stderr");
        (status.expect("wait for child"), stdout_bytes, stderr_bytes)
    }

    #[tokio::test]
    async fn experimental_bridge_captures_output_and_enforces_sbox_read_boundary() {
        let _serial = ACL_TEST_LOCK.lock().await;
        let root = acl_test_tempdir("experimental-bridge-");
        let workspace = root.path().join("workspace");
        let sibling = root.path().join("sibling");
        std::fs::create_dir_all(&workspace).expect("workspace");
        std::fs::create_dir_all(&sibling).expect("sibling");
        let secret = sibling.join("secret.txt");
        std::fs::write(&secret, "not-readable-from-sandbox").expect("secret fixture");
        let sandbox = Arc::new(WindowsWorkspaceSandbox::new(&workspace).expect("sandbox"));

        let mut child = sandbox
            .spawn_experimental_with_read_only_paths(
                OsStr::new("cmd.exe"),
                &[
                    OsString::from("/d"),
                    OsString::from("/s"),
                    OsString::from("/c"),
                    OsString::from(
                        "echo bridge-stdout & echo bridge-stderr 1>&2 & echo workspace-write>bridge-output.txt",
                    ),
                ],
                None,
                &workspace,
                &[],
                &[],
                &[],
            )
            .expect("start through the experimental SBOX runner");
        let mut stdout = child.take_stdout().expect("stdout");
        let mut stderr = child.take_stderr().expect("stderr");
        let mut stdout_bytes = Vec::new();
        let mut stderr_bytes = Vec::new();
        let (stdout_result, stderr_result, status) = tokio::join!(
            stdout.read_to_end(&mut stdout_bytes),
            stderr.read_to_end(&mut stderr_bytes),
            child.wait()
        );
        stdout_result.expect("read stdout");
        stderr_result.expect("read stderr");
        let status = status.expect("wait");
        assert_eq!(
            status.code(),
            Some(0),
            "sandbox command failed; stdout={} stderr={}",
            String::from_utf8_lossy(&stdout_bytes),
            String::from_utf8_lossy(&stderr_bytes)
        );
        assert!(String::from_utf8_lossy(&stdout_bytes).contains("bridge-stdout"));
        assert!(String::from_utf8_lossy(&stderr_bytes).contains("bridge-stderr"));
        assert_eq!(
            std::fs::read_to_string(workspace.join("bridge-output.txt"))
                .expect("workspace write")
                .trim(),
            "workspace-write"
        );

        let mut child = sandbox
            .spawn_experimental_with_read_only_paths(
                OsStr::new("cmd.exe"),
                &[
                    OsString::from("/d"),
                    OsString::from("/s"),
                    OsString::from("/c"),
                    OsString::from(format!("type \"{}\"", secret.display())),
                ],
                None,
                &workspace,
                &[],
                &[],
                &[],
            )
            .expect("start sibling-read denial probe");
        let mut stdout = child.take_stdout().expect("stdout");
        let mut stderr = child.take_stderr().expect("stderr");
        let mut stdout_bytes = Vec::new();
        let mut stderr_bytes = Vec::new();
        let (stdout_result, stderr_result, status) = tokio::join!(
            stdout.read_to_end(&mut stdout_bytes),
            stderr.read_to_end(&mut stderr_bytes),
            child.wait()
        );
        stdout_result.expect("read stdout");
        stderr_result.expect("read stderr");
        assert_ne!(status.expect("wait").code(), Some(0));
        let observed = format!(
            "{}{}",
            String::from_utf8_lossy(&stdout_bytes),
            String::from_utf8_lossy(&stderr_bytes)
        );
        assert!(
            !observed.contains("not-readable-from-sandbox"),
            "the sibling file contents escaped the SBOX read boundary: {observed}"
        );

        let outside_write = root.path().join("outside-write.txt");
        let child = sandbox
            .spawn_experimental_with_read_only_paths(
                OsStr::new("cmd.exe"),
                &[
                    OsString::from("/d"),
                    OsString::from("/s"),
                    OsString::from("/c"),
                    OsString::from("echo denied>..\\outside-write.txt"),
                ],
                None,
                &workspace,
                &[],
                &[],
                &[],
            )
            .expect("start sibling-write denial probe");
        let _ = finish(child).await;
        assert!(
            !outside_write.exists(),
            "SBOX allowed writes outside the selected workspace"
        );

        let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
            .expect("bind local egress probe listener");
        listener
            .set_nonblocking(true)
            .expect("make listener nonblocking");
        let address = listener.local_addr().expect("listener address");
        let accepted = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let accepted_by_server = Arc::clone(&accepted);
        let listener_thread = thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
            while std::time::Instant::now() < deadline {
                match listener.accept() {
                    Ok((stream, _)) => {
                        accepted_by_server.store(true, std::sync::atomic::Ordering::SeqCst);
                        drop(stream);
                        return;
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(20));
                    }
                    Err(_) => return,
                }
            }
        });
        let child = sandbox
            .spawn_experimental_with_read_only_paths(
                OsStr::new("curl.exe"),
                &[
                    OsString::from("--noproxy"),
                    OsString::from("*"),
                    OsString::from("--max-time"),
                    OsString::from("2"),
                    OsString::from(format!("http://{address}/")),
                ],
                None,
                &workspace,
                &[],
                &[],
                &[],
            )
            .expect("start loopback network denial probe");
        let (status, _, _) = finish(child).await;
        listener_thread.join().expect("join local listener");
        assert!(!accepted.load(std::sync::atomic::Ordering::SeqCst));
        assert_ne!(status.code(), Some(0), "SBOX allowed loopback egress");
    }

    #[tokio::test]
    async fn experimental_bridge_runs_from_os_temp_workspace() {
        let _serial = ACL_TEST_LOCK.lock().await;
        let workspace = tempfile::tempdir().expect("OS temporary workspace");
        let sandbox = Arc::new(WindowsWorkspaceSandbox::new(workspace.path()).expect("sandbox"));
        let (status, stdout, stderr) = finish(
            sandbox
                .spawn_experimental_with_read_only_paths(
                    OsStr::new("cmd.exe"),
                    &[
                        OsString::from("/d"),
                        OsString::from("/s"),
                        OsString::from("/c"),
                        OsString::from("echo temp-workspace"),
                    ],
                    None,
                    workspace.path(),
                    &[],
                    &[],
                    &[],
                )
                .expect("launch from OS temporary workspace"),
        )
        .await;

        assert_eq!(
            status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&stderr)
        );
        assert!(String::from_utf8_lossy(&stdout).contains("temp-workspace"));
    }

    #[tokio::test]
    async fn experimental_bridge_reads_an_explicit_external_root() {
        let _serial = ACL_TEST_LOCK.lock().await;
        let root = acl_test_tempdir("external-read-root-");
        let workspace = root.path().join("workspace");
        std::fs::create_dir(&workspace).expect("workspace");
        let runtime = tempfile::tempdir().expect("external runtime root");
        let marker = runtime.path().join("runtime-marker.txt");
        std::fs::write(&marker, "explicitly-granted-runtime").expect("runtime marker");
        let sandbox = Arc::new(WindowsWorkspaceSandbox::new(&workspace).expect("sandbox"));
        let command = format!("type {}", marker.display());
        let (status, stdout, stderr) = finish(
            sandbox
                .spawn_experimental_with_read_only_paths(
                    OsStr::new("cmd.exe"),
                    &[
                        OsString::from("/d"),
                        OsString::from("/s"),
                        OsString::from("/c"),
                        OsString::from(command),
                    ],
                    None,
                    &workspace,
                    &[],
                    &[],
                    &[runtime.path().to_path_buf()],
                )
                .expect("launch with explicit read-only runtime root"),
        )
        .await;

        assert_eq!(
            status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&stderr)
        );
        assert!(String::from_utf8_lossy(&stdout).contains("explicitly-granted-runtime"));
    }

    #[tokio::test]
    async fn sandbox_allows_workspace_writes_and_blocks_sibling_writes_from_child_tree() {
        let _serial = ACL_TEST_LOCK.lock().await;
        let root = acl_test_tempdir("appcontainer-workspace-write-");
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let inside = workspace.join("inside.txt");
        let outside = root.path().join("outside.txt");
        let nested_outside = root.path().join("nested-outside.txt");
        let sandbox =
            Arc::new(WindowsWorkspaceSandbox::new(&workspace).expect("workspace sandbox"));

        let script =
            "mkdir target && echo allowed>inside.txt && echo nested-allowed>target\\inside.txt";
        let mut child = sandbox
            .spawn(
                OsStr::new("cmd.exe"),
                &[
                    OsString::from("/d"),
                    OsString::from("/s"),
                    OsString::from("/c"),
                    OsString::from(script),
                ],
                None,
                &workspace,
                &credentials_to_remove(),
                &[],
            )
            .expect("spawn contained writer");
        let (status, _, stderr) = finish(child).await;
        assert!(
            status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&stderr)
        );
        assert_eq!(
            std::fs::read_to_string(&inside)
                .expect("read workspace write")
                .trim(),
            "allowed"
        );
        assert_eq!(
            std::fs::read_to_string(workspace.join("target").join("inside.txt"))
                .expect("read nested workspace write")
                .trim(),
            "nested-allowed"
        );

        let outside_script = "echo denied>..\\outside.txt";
        let child = sandbox
            .spawn(
                OsStr::new("cmd.exe"),
                &[
                    OsString::from("/d"),
                    OsString::from("/s"),
                    OsString::from("/c"),
                    OsString::from(outside_script),
                ],
                None,
                &workspace,
                &credentials_to_remove(),
                &[],
            )
            .expect("spawn sibling-write probe");
        let _ = finish(child).await;
        assert!(
            !outside.exists(),
            "the process sandbox allowed writes to a sibling directory"
        );

        let outside_dir = root.path().join("outside-dir");
        std::fs::create_dir(&outside_dir).expect("outside target directory");
        let junction = workspace.join("outside-junction");
        let junction_status = std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside_dir)
            .status()
            .expect("create unprivileged directory junction");
        assert!(junction_status.success(), "could not create junction probe");
        let junction_escape = outside_dir.join("junction-escape.txt");
        let junction_script = "echo denied>outside-junction\\junction-escape.txt";
        let child = sandbox
            .spawn(
                OsStr::new("cmd.exe"),
                &[
                    OsString::from("/d"),
                    OsString::from("/s"),
                    OsString::from("/c"),
                    OsString::from(junction_script),
                ],
                None,
                &workspace,
                &credentials_to_remove(),
                &[],
            )
            .expect("spawn reparse-point escape probe");
        let _ = finish(child).await;
        assert!(
            !junction_escape.exists(),
            "the process sandbox followed a workspace junction outside its write boundary"
        );
        std::fs::remove_dir(&junction).expect("remove test junction");

        std::fs::write(
            workspace.join("nested-child.cmd"),
            "@echo off\r\necho nested-ran\r\necho nested-allowed>nested-inside.txt\r\necho denied>..\\nested-outside.txt\r\n",
        )
        .expect("write nested child probe");
        let nested_script = "cmd.exe /d /c nested-child.cmd";
        let child = sandbox
            .spawn(
                OsStr::new("cmd.exe"),
                &[
                    OsString::from("/d"),
                    OsString::from("/s"),
                    OsString::from("/c"),
                    OsString::from(nested_script),
                ],
                None,
                &workspace,
                &credentials_to_remove(),
                &[],
            )
            .expect("spawn nested-process probe");
        let (status, stdout, stderr) = finish(child).await;
        assert!(
            String::from_utf8_lossy(&stdout).contains("nested-ran"),
            "nested cmd did not execute its script (exit {:?}); stderr: {}",
            status.code(),
            String::from_utf8_lossy(&stderr),
        );
        assert_eq!(
            std::fs::read_to_string(workspace.join("nested-inside.txt"))
                .expect("nested child workspace write")
                .trim(),
            "nested-allowed"
        );
        assert!(
            !nested_outside.exists(),
            "nested process escaped the AppContainer write ACL (exit {:?}, stderr: {})",
            status.code(),
            String::from_utf8_lossy(&stderr)
        );
    }

    #[tokio::test]
    async fn sandbox_path_policy_blocks_sibling_reads_and_loopback_network() {
        use tokio::net::TcpListener;

        let _serial = ACL_TEST_LOCK.lock().await;
        let root = acl_test_tempdir("appcontainer-read-boundary-");
        let workspace = root.path().join("workspace");
        let sibling = root.path().join("sibling");
        let cache = workspace.join(".arcana-cache");
        std::fs::create_dir_all(&cache).expect("workspace cache");
        std::fs::create_dir(&sibling).expect("sibling directory");
        let secret = sibling.join("secret.txt");
        std::fs::write(&secret, "synthetic-sibling-secret").expect("secret fixture");

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("local denial-test listener");
        let address = listener.local_addr().expect("listener address");
        let script = format!(
            "$readDenied=$false; try {{ $value=[IO.File]::ReadAllText('{}'); Write-Output 'READ_ALLOWED' }} catch {{ $readDenied=$true; Write-Output 'READ_DENIED' }}; $networkDenied=$false; try {{ $client=New-Object Net.Sockets.TcpClient; $client.Connect('127.0.0.1',{}); Write-Output 'NETWORK_ALLOWED'; $client.Close() }} catch {{ $networkDenied=$true; Write-Output 'NETWORK_DENIED' }}; if ($readDenied -and $networkDenied) {{ exit 0 }} else {{ exit 42 }}",
            secret.display(), address.port()
        );
        let sandbox = Arc::new(WindowsWorkspaceSandbox::new(&workspace).expect("sandbox"));
        let child = sandbox
            .spawn(
                OsStr::new("powershell.exe"),
                &[
                    OsString::from("-NoProfile"),
                    OsString::from("-NonInteractive"),
                    OsString::from("-Command"),
                    OsString::from(script),
                ],
                None,
                &workspace,
                &credentials_to_remove(),
                &[(
                    OsString::from("CLIPPY_CONF_DIR"),
                    cache.clone().into_os_string(),
                )],
            )
            .expect("spawn isolated boundary probe");
        let (status, stdout, _) = finish(child).await;
        let stdout = String::from_utf8_lossy(&stdout);
        assert!(status.success(), "boundary probe failed: {stdout}");
        assert!(stdout.contains("READ_DENIED"), "sibling read was allowed");
        assert!(stdout.contains("NETWORK_DENIED"), "loopback was reachable");
        assert!(!stdout.contains("synthetic-sibling-secret"));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(150), listener.accept())
                .await
                .is_err(),
            "the child connected to the local listener"
        );
    }

    #[tokio::test]
    async fn sandbox_preserves_rust_owned_cmd_shim_invocation_and_exit_code() {
        let _serial = ACL_TEST_LOCK.lock().await;
        let root = tempfile::Builder::new()
            .prefix("arcana appcontainer cmd ")
            .tempdir()
            .expect("temporary workspace");
        let shim = root.path().join("npm.cmd");
        std::fs::write(&shim, "@echo off\r\necho %1 %2\r\nexit /b 17\r\n")
            .expect("write test shim");
        let invocation = OsString::from(format!("\"\"{}\" run build\"", shim.display()));
        let sandbox =
            Arc::new(WindowsWorkspaceSandbox::new(root.path()).expect("workspace sandbox"));
        let child = sandbox
            .spawn(
                OsStr::new("cmd.exe"),
                &[
                    OsString::from("/d"),
                    OsString::from("/s"),
                    OsString::from("/c"),
                ],
                Some(&invocation),
                root.path(),
                &credentials_to_remove(),
                &[],
            )
            .expect("spawn .cmd shim through cmd.exe");
        let (status, stdout, stderr) = finish(child).await;
        assert_eq!(
            status.code(),
            Some(17),
            "stderr: {}",
            String::from_utf8_lossy(&stderr)
        );
        assert!(String::from_utf8_lossy(&stdout).contains("run build"));
    }

    #[tokio::test]
    async fn dropping_a_timed_out_sandbox_child_terminates_it() {
        let _serial = ACL_TEST_LOCK.lock().await;
        let root = tempfile::Builder::new()
            .prefix("arcana-appcontainer-cancellation-")
            .tempdir()
            .expect("temporary workspace");
        let workspace = root.path().to_path_buf();
        let started = workspace.join("started.txt");
        let late = workspace.join("late.txt");
        let sandbox =
            Arc::new(WindowsWorkspaceSandbox::new(&workspace).expect("workspace sandbox"));
        let script =
            "echo started>started.txt & for /l %i in (1,1,100000000) do @rem & echo late>late.txt";
        let mut child = sandbox
            .spawn(
                OsStr::new("cmd.exe"),
                &[
                    OsString::from("/d"),
                    OsString::from("/s"),
                    OsString::from("/c"),
                    OsString::from(script),
                ],
                None,
                &workspace,
                &credentials_to_remove(),
                &[],
            )
            .expect("spawn cancellable command");

        let started_result = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while !started.exists() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await;
        if started_result.is_err() {
            let (status, stdout, stderr) = finish(child).await;
            panic!(
                "child did not begin before cancellation; exit={status}, stdout={}, stderr={}",
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
        }

        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), child.wait())
                .await
                .is_err()
        );
        drop(child);
        assert!(started.is_file());
        assert!(
            !late.exists(),
            "timed-out command continued running after its job was dropped"
        );
    }

    #[tokio::test]
    async fn dropping_a_sandbox_command_terminates_its_descendant_processes() {
        let _serial = ACL_TEST_LOCK.lock().await;
        let root = tempfile::Builder::new()
            .prefix("arcana-appcontainer-descendant-cancellation-")
            .tempdir()
            .expect("temporary workspace");
        let workspace = root.path().to_path_buf();
        let parent_script = workspace.join("parent.cmd");
        let descendant_script = workspace.join("descendant.cmd");
        let descendant_started = workspace.join("descendant-started.txt");
        let heartbeat = workspace.join("descendant-heartbeat.txt");
        let late = workspace.join("descendant-late.txt");
        std::fs::write(
            &descendant_script,
            "@echo off\r\necho started>descendant-started.txt\r\nfor /l %%i in (1,1,100000000) do @echo alive>>descendant-heartbeat.txt\r\necho late>descendant-late.txt\r\n",
        )
        .expect("write descendant command");
        std::fs::write(
            &parent_script,
            "@echo off\r\ncmd.exe /d /c descendant.cmd\r\n",
        )
        .expect("write parent command");
        let sandbox =
            Arc::new(WindowsWorkspaceSandbox::new(&workspace).expect("workspace sandbox"));
        let child = sandbox
            .spawn(
                OsStr::new("cmd.exe"),
                &[
                    OsString::from("/d"),
                    OsString::from("/c"),
                    OsString::from("parent.cmd"),
                ],
                None,
                &workspace,
                &credentials_to_remove(),
                &[],
            )
            .expect("spawn parent and descendant commands");

        let started = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while !descendant_started.exists() || !heartbeat.exists() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await;
        if started.is_err() {
            let (status, stdout, stderr) = finish(child).await;
            panic!(
                "descendant did not begin before cancellation; exit={status}, stdout={}, stderr={}",
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
        }

        drop(child);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let heartbeat_size = std::fs::metadata(&heartbeat)
            .expect("descendant heartbeat exists")
            .len();
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        assert_eq!(
            std::fs::metadata(&heartbeat)
                .expect("descendant heartbeat remains readable")
                .len(),
            heartbeat_size,
            "descendant continued running after its parent command was cancelled"
        );
        assert!(
            !late.exists(),
            "descendant reached its post-cancellation side effect"
        );
    }

    #[tokio::test]
    async fn sandbox_can_read_workspace_local_clippy_config_directory() {
        let _serial = ACL_TEST_LOCK.lock().await;
        let root = acl_test_tempdir("appcontainer-clippy-");
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("create workspace");
        let sandbox =
            Arc::new(WindowsWorkspaceSandbox::new(&workspace).expect("workspace sandbox"));
        let first_child = sandbox
            .spawn(
                OsStr::new("cmd.exe"),
                &[
                    OsString::from("/d"),
                    OsString::from("/c"),
                    OsString::from("exit /b 0"),
                ],
                None,
                &workspace,
                &credentials_to_remove(),
                &[],
            )
            .expect("spawn a process before the Clippy cache exists");
        let (status, _, _) = finish(first_child).await;
        assert!(status.success(), "initial workspace process should pass");

        let config_dir = workspace.join(".arcana-cache");
        std::fs::create_dir_all(&config_dir).expect("create workspace config directory");
        std::fs::write(config_dir.join(".clippy.toml"), "").expect("create Clippy config");
        let config_dir_env = config_dir.as_os_str().to_os_string();
        let child = sandbox
            .spawn(
                OsStr::new("cmd.exe"),
                &[
                    OsString::from("/d"),
                    OsString::from("/s"),
                    OsString::from("/c"),
                    OsString::from("type %CLIPPY_CONF_DIR%\\.clippy.toml"),
                ],
                None,
                &workspace,
                &credentials_to_remove(),
                &[(OsString::from("CLIPPY_CONF_DIR"), config_dir_env)],
            )
            .expect("spawn config reader");
        let (status, stdout, stderr) = finish(child).await;
        assert!(
            status.success(),
            "Clippy config read failed; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr)
        );
    }
}
