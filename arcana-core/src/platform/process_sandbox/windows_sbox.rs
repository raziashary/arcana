//! Builder for Microsoft's experimental Windows SBOX sandbox contract.
//!
//! This is deliberately separate from `psec.rs`: the experimental launcher
//! consumes the versioned `SBOX` schema and does not accept a PSEC buffer.

use std::io;
use std::path::{Path, PathBuf};

use flatbuffers::{FlatBufferBuilder, Follow, ForwardsUOffset, Table, Vector, WIPOffset};

const VERSION: &str = "0.1.0";

/// Construct a fail-closed AppContainer policy for one workspace and exact
/// read-only runtime roots. Network egress has an explicit default deny.
pub(super) fn build_specification(
    workspace: &Path,
    read_only_paths: &[PathBuf],
) -> io::Result<Vec<u8>> {
    let workspace = canonical_directory(workspace, "writable workspace")?;
    let mut read_only = Vec::with_capacity(read_only_paths.len());
    for path in read_only_paths {
        let canonical = canonical_directory(path, "read-only runtime")?;
        if canonical.starts_with(&workspace) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "SBOX read-only paths cannot overlap the writable workspace",
            ));
        }
        if !read_only.contains(&canonical) {
            read_only.push(canonical);
        }
    }

    let workspace_text = path_for_sbox(&workspace)?;
    let read_only_text = read_only
        .iter()
        .map(|path| path_for_sbox(path))
        .collect::<io::Result<Vec<_>>>()?;

    let mut builder = FlatBufferBuilder::new();
    let version = builder.create_string(VERSION);
    let workspace_offset = builder.create_string(&workspace_text);
    let writable = builder.create_vector(&[workspace_offset]);
    let read_only_offsets = read_only_text
        .iter()
        .map(|path| builder.create_string(path))
        .collect::<Vec<_>>();
    let read_only = builder.create_vector(&read_only_offsets);

    // endpoint_policy.default_action = FilterAction::deny (enum value 0).
    let endpoint = builder.start_table();
    builder.push_slot::<u8>(4, 0, 0);
    let endpoint = builder.end_table(endpoint);

    // NetworkPolicy.egress is the second field (vtable byte offset 6).
    let network = builder.start_table();
    builder.push_slot_always::<WIPOffset<_>>(6, endpoint);
    let network = builder.end_table(network);

    let sandbox = builder.start_table();
    builder.push_slot_always::<WIPOffset<_>>(4, version);
    builder.push_slot::<bool>(6, true, false); // app_container
    builder.push_slot::<u32>(8, 0, 0); // deprecated integrity_level
                                       // The Arcana bridge fails during Windows loader initialization with the
                                       // Win32k-disallow mitigation on build 26200 (STATUS_DLL_INIT_FAILED). The
                                       // AppContainer remains headless by capability: no desktop/window handles
                                       // are inherited, and filesystem/network policy remains default-deny.
    builder.push_slot::<bool>(10, false, false);
    builder.push_slot::<u64>(12, 0, 0); // ui_restrictions
    builder.push_slot::<bool>(14, true, false); // least_privilege
    builder.push_slot_always::<WIPOffset<_>>(18, writable);
    builder.push_slot_always::<WIPOffset<_>>(20, read_only);
    builder.push_slot_always::<WIPOffset<_>>(22, network);
    let sandbox = builder.end_table(sandbox);
    builder.finish(sandbox, Some("SBOX"));

    Ok(builder.finished_data().to_vec())
}

fn canonical_directory(path: &Path, label: &str) -> io::Result<PathBuf> {
    let canonical = path.canonicalize()?;
    if !canonical.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("SBOX {label} must be a directory"),
        ));
    }
    Ok(canonical)
}

fn path_for_sbox(path: &Path) -> io::Result<String> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "SBOX cannot encode a path that is not valid UTF-8",
        )
    })
}

#[derive(Clone, Copy)]
struct SandboxSpec<'a> {
    table: Table<'a>,
}

impl<'a> Follow<'a> for SandboxSpec<'a> {
    type Inner = Self;

    unsafe fn follow(buf: &'a [u8], loc: usize) -> Self::Inner {
        Self {
            // SAFETY: `flatbuffers::root` has checked that the root offset is in
            // the provided buffer. Accessors below are used only on builder-
            // generated test data.
            table: unsafe { Table::new(buf, loc) },
        }
    }
}

impl<'a> SandboxSpec<'a> {
    fn version(&self) -> &'a str {
        unsafe {
            self.table
                .get::<ForwardsUOffset<&'a str>>(4, None)
                .expect("required schema version")
        }
    }

    fn app_container(&self) -> bool {
        unsafe { self.table.get::<bool>(6, Some(false)).unwrap_or(false) }
    }

    fn workspace_paths(&self) -> Vector<'a, ForwardsUOffset<&'a str>> {
        unsafe {
            self.table
                .get::<ForwardsUOffset<Vector<'a, ForwardsUOffset<&'a str>>>>(18, None)
                .expect("workspace write grant")
        }
    }

    fn read_only_paths(&self) -> Vector<'a, ForwardsUOffset<&'a str>> {
        unsafe {
            self.table
                .get::<ForwardsUOffset<Vector<'a, ForwardsUOffset<&'a str>>>>(20, None)
                .expect("read-only grants")
        }
    }

    fn network_policy(&self) -> Table<'a> {
        unsafe {
            self.table
                .get::<ForwardsUOffset<Table<'a>>>(22, None)
                .expect("explicit network policy")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specification_is_sbox_appcontainer_with_workspace_only_write_and_deny_egress() {
        let root = tempfile::tempdir().expect("temporary root");
        let workspace = root.path().join("workspace");
        let runtime = root.path().join("runtime");
        std::fs::create_dir_all(&workspace).expect("workspace");
        std::fs::create_dir_all(&runtime).expect("runtime");

        let bytes = build_specification(&workspace, &[runtime.clone()]).expect("SBOX policy");
        assert!(flatbuffers::buffer_has_identifier(&bytes, "SBOX", false));
        // SAFETY: bytes were produced by the FlatBufferBuilder immediately
        // above, and the file identifier plus accessed field slots are checked.
        let spec = unsafe { flatbuffers::root_unchecked::<SandboxSpec>(&bytes) };
        assert_eq!(spec.version(), VERSION);
        assert!(spec.app_container());
        assert_eq!(
            spec.workspace_paths().get(0),
            workspace.canonicalize().unwrap().to_str().unwrap()
        );
        let read_only_paths = spec.read_only_paths();
        assert!((0..read_only_paths.len()).any(|index| {
            read_only_paths.get(index) == runtime.canonicalize().unwrap().to_str().unwrap()
        }));

        let network = spec.network_policy();
        let egress = unsafe {
            network
                .get::<ForwardsUOffset<Table<'_>>>(6, None)
                .expect("egress policy")
        };
        let action = unsafe { egress.get::<u8>(4, Some(0)).expect("deny default") };
        assert_eq!(action, 0, "SBOX FilterAction::deny is enum value zero");
    }

    #[test]
    fn specification_refuses_runtime_write_grants_and_missing_directories() {
        let root = tempfile::tempdir().expect("temporary root");
        let workspace = root.path().join("workspace");
        let runtime = workspace.join("runtime");
        std::fs::create_dir_all(&runtime).expect("runtime");

        assert!(build_specification(&workspace, &[runtime]).is_err());
        assert!(build_specification(&workspace, &[root.path().join("missing")]).is_err());
    }
}
