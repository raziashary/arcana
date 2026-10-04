//! Runtime-probed Windows Process Security Environment (PSEC) support.
//!
//! The PSEC schema is Microsoft-owned and pinned beside this module. All OS
//! exports are resolved from System32 at runtime so unsupported Windows builds
//! fail closed rather than weakening the process policy.

use std::ffi::c_void;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::OnceLock;

use super::windows::{CACHE_DIR_NAME, TEMP_DIR_NAME};
use windows_sys::Win32::Foundation::{GetLastError, HANDLE, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
};

#[path = "psec_generated/ProcessSecurityEnvironment_generated.rs"]
#[allow(non_snake_case)]
mod ProcessSecurityEnvironment_generated;
use self::ProcessSecurityEnvironment_generated as generated;

type CreatePsec = unsafe extern "system" fn(*const c_void, u32, u32, *mut HANDLE) -> i32;
type ClosePsec = unsafe extern "system" fn(HANDLE);
type QuerySupport = unsafe extern "system" fn(*mut u64) -> i32;
type QueryVersion = unsafe extern "system" fn(u32, *mut u8, *mut u32) -> i32;

const SUPPORT_FILESYSTEM_DENY: u64 = 0x1;
const PSEC_V1_MAJOR: u32 = 1;
const PSEC_V1_REQUIRED_MINOR: u32 = 0;
const PSEC_ATTRIBUTE: usize = 35 | 0x0002_0000;

#[derive(Clone, Copy)]
struct Api {
    create: CreatePsec,
    close: ClosePsec,
}

static API: OnceLock<Result<Api, String>> = OnceLock::new();

/// Owns one OS-created security environment. Keep it alive until the process
/// created with this environment has exited.
pub(super) struct ProcessSecurityEnvironment {
    handle: HANDLE,
    close: ClosePsec,
}

// SAFETY: the opaque OS environment handle is uniquely owned and only passed
// by value to process creation or closed once; moving that ownership between
// worker threads does not change the native handle's lifetime or identity.
unsafe impl Send for ProcessSecurityEnvironment {}

impl ProcessSecurityEnvironment {
    pub(super) fn create(workspace: &Path, read_only_paths: &[PathBuf]) -> io::Result<Self> {
        let api = *API
            .get_or_init(load_api)
            .as_ref()
            .map_err(|message| io::Error::new(io::ErrorKind::Unsupported, message.clone()))?;
        let specification = build_specification(workspace, read_only_paths)?;
        let size = u32::try_from(specification.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "PSEC specification is too large",
            )
        })?;
        let mut handle = null_mut();
        let result = unsafe { (api.create)(specification.as_ptr().cast(), size, 0, &mut handle) };
        if result < 0 || handle.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "CreateProcessSecurityEnvironment failed (HRESULT 0x{:08x})",
                    result as u32
                ),
            ));
        }
        Ok(Self {
            handle,
            close: api.close,
        })
    }

    pub(super) fn raw_handle(&self) -> HANDLE {
        self.handle
    }
}

impl Drop for ProcessSecurityEnvironment {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { (self.close)(self.handle) };
            self.handle = null_mut();
        }
    }
}

pub(super) const fn process_security_environment_attribute() -> usize {
    PSEC_ATTRIBUTE
}

fn load_api() -> Result<Api, String> {
    let module = processmodel_module().map_err(|error| error.to_string())?;
    let create = unsafe { resolve::<CreatePsec>(module, b"CreateProcessSecurityEnvironment\0") }
        .ok_or_else(|| {
            "Windows processmodel.dll lacks CreateProcessSecurityEnvironment".to_owned()
        })?;
    let close = unsafe { resolve::<ClosePsec>(module, b"CloseProcessSecurityEnvironment\0") }
        .ok_or_else(|| {
            "Windows processmodel.dll lacks CloseProcessSecurityEnvironment".to_owned()
        })?;
    let query =
        unsafe { resolve::<QuerySupport>(module, b"QueryProcessSecurityEnvironmentSupport\0") }
            .ok_or_else(|| {
                "Windows processmodel.dll lacks QueryProcessSecurityEnvironmentSupport".to_owned()
            })?;
    let version = unsafe {
        resolve::<QueryVersion>(module, b"IsProcessSecurityEnvironmentVersionSupported\0")
    }
    .ok_or_else(|| {
        "Windows processmodel.dll lacks IsProcessSecurityEnvironmentVersionSupported".to_owned()
    })?;

    let mut support = 0u64;
    let result = unsafe { query(&mut support) };
    if result < 0 {
        return Err(format!(
            "QueryProcessSecurityEnvironmentSupport failed (HRESULT 0x{:08x})",
            result as u32
        ));
    }
    if support & SUPPORT_FILESYSTEM_DENY == 0 {
        return Err("Windows PSEC runtime does not advertise filesystem-deny support".to_owned());
    }

    let mut available = 0u8;
    let mut minor = 0u32;
    let result = unsafe { version(PSEC_V1_MAJOR, &mut available, &mut minor) };
    if result < 0 || available == 0 || minor < PSEC_V1_REQUIRED_MINOR {
        return Err(format!(
            "Windows PSEC schema v1.0 is unavailable (HRESULT 0x{:08x}, available={}, minor={})",
            result as u32, available, minor
        ));
    }
    Ok(Api { create, close })
}

fn processmodel_module() -> io::Result<HMODULE> {
    let name = wide_null("processmodel.dll");
    let module = unsafe { LoadLibraryExW(name.as_ptr(), null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32) };
    if module.is_null() {
        return Err(io::Error::from_raw_os_error(
            unsafe { GetLastError() } as i32
        ));
    }
    Ok(module)
}

unsafe fn resolve<T: Copy>(module: HMODULE, name: &'static [u8]) -> Option<T> {
    let address = unsafe { GetProcAddress(module, name.as_ptr()) }?;
    // SAFETY: each caller supplies the ABI-verified signature for this export;
    // the module handle remains loaded for the process lifetime.
    Some(unsafe { std::mem::transmute_copy(&address) })
}

fn build_specification(workspace: &Path, read_only_paths: &[PathBuf]) -> io::Result<Vec<u8>> {
    use generated::process_security_environment_layout as psec;

    let workspace = canonical_policy_path(workspace)?;
    let mut read_only = Vec::with_capacity(read_only_paths.len());
    for path in read_only_paths {
        let path = canonical_policy_path(path)?;
        if path.starts_with(&workspace) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "PSEC read-only grants must not overlap the writable workspace",
            ));
        }
        if !read_only.contains(&path) {
            read_only.push(path);
        }
    }

    let private_roots = user_private_roots();
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let mut writable_paths = vec![workspace.clone()];
    // PSEC 1.0 applies access by path identity. Repeat Arcana's cache/temp
    // roots explicitly so child tools which canonicalize a nested selected
    // directory receive the same workspace-scoped capability as ordinary IO.
    for relative in [TEMP_DIR_NAME, CACHE_DIR_NAME, ".arcana-cache/cargo-target"] {
        let path = workspace.join(relative);
        if path.is_dir() {
            let path = path.canonicalize()?;
            if !path.starts_with(&workspace) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "PSEC workspace subdirectory resolves outside the selected workspace",
                ));
            }
            writable_paths.push(path);
        }
    }
    let rw_offsets = writable_paths
        .iter()
        .map(|path| psec_path_string(path).map(|path| builder.create_string(path)))
        .collect::<io::Result<Vec<_>>>()?;
    let rw = builder.create_vector(&rw_offsets);
    let ro_offsets = read_only
        .iter()
        .map(|path| psec_path_string(path).map(|path| builder.create_string(path)))
        .collect::<io::Result<Vec<_>>>()?;
    let ro = builder.create_vector(&ro_offsets);
    let deny_offsets = private_roots
        .iter()
        .map(|path| psec_path_string(path).map(|path| builder.create_string(path)))
        .collect::<io::Result<Vec<_>>>()?;
    let deny = builder.create_vector(&deny_offsets);

    let egress = {
        let mut args = psec::EndpointPolicyArgs::default();
        args.default_action = psec::FilterAction::deny;
        let mut table = psec::EndpointPolicyBuilder::new(&mut builder);
        table.add_default_action(args.default_action);
        table.finish()
    };
    let network = {
        let mut table = psec::NetworkPolicyBuilder::new(&mut builder);
        table.add_egress(egress);
        table.finish()
    };
    let version = psec::SchemaVersion([1, 0, 0, 0]);
    let root = {
        let mut table = psec::ProcessSecurityEnvironmentBuilder::new(&mut builder);
        table.add_version(&version);
        table.add_fs_read_write(rw);
        table.add_fs_read_only(ro);
        table.add_fs_deny(deny);
        table.add_network_policy(network);
        table.finish()
    };
    psec::finish_process_security_environment_buffer(&mut builder, root);
    let bytes = builder.finished_data().to_vec();
    psec::root_as_process_security_environment(&bytes).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("generated invalid PSEC: {error}"),
        )
    })?;
    Ok(bytes)
}

fn canonical_policy_path(path: &Path) -> io::Result<PathBuf> {
    path.canonicalize()
}

fn psec_path_string(path: &Path) -> io::Result<&str> {
    path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "PSEC cannot encode a Windows path that is not valid UTF-8",
        )
    })
}

fn user_private_roots() -> Vec<PathBuf> {
    let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) else {
        return Vec::new();
    };
    [".ssh", ".aws", ".azure", ".gnupg", ".kube"]
        .into_iter()
        .map(|name| profile.join(name))
        .map(|path| path.canonicalize().unwrap_or(path))
        .collect()
}

fn wide_null(value: &str) -> Vec<u16> {
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(Some(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use generated::process_security_environment_layout as psec;

    #[test]
    fn serialized_policy_limits_files_and_denies_all_network_egress() {
        let root = tempfile::tempdir().expect("workspace parent");
        let workspace = root.path().join("workspace");
        let runtime = root.path().join("runtime");
        std::fs::create_dir(&workspace).expect("workspace");
        std::fs::create_dir(&runtime).expect("runtime");
        let bytes = build_specification(&workspace, &[runtime.clone()]).expect("PSEC policy");
        assert!(psec::process_security_environment_buffer_has_identifier(
            &bytes
        ));
        let spec = psec::root_as_process_security_environment(&bytes).expect("valid PSEC");
        assert_eq!(spec.version().major(), 1);
        assert_eq!(spec.version().minor(), 0);
        assert_eq!(
            spec.fs_read_write().unwrap().get(0),
            workspace.canonicalize().unwrap().to_string_lossy()
        );
        assert_eq!(
            spec.fs_read_only().unwrap().get(0),
            runtime.canonicalize().unwrap().to_string_lossy()
        );
        assert_eq!(
            spec.network_policy()
                .unwrap()
                .egress()
                .unwrap()
                .default_action(),
            psec::FilterAction::deny
        );
        assert!(spec.fs_deny().is_some());
    }

    #[test]
    fn refuses_a_read_only_path_inside_the_writable_workspace() {
        let workspace = tempfile::tempdir().expect("workspace");
        let nested = workspace.path().join("runtime");
        std::fs::create_dir(&nested).expect("runtime");
        let error = build_specification(workspace.path(), &[nested]).expect_err("overlap denied");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
