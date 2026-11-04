//! Host-controlled dependency staging from a lockfile (`D-008`).
//!
//! Sandboxed build children have no network, so a project's dependencies must
//! already be on disk. The host fetches them here, outside the sandbox, with
//! the network open for that one step, and the build then reads them through a
//! read-only grant.
//!
//! The cache lives under Arcana's own directory, never in the workspace: a
//! sandboxed build can write anywhere in the workspace, and anything it wrote
//! that the next host fetch read (a Cargo `config.toml`, for example) would run
//! with host privileges.
//!
//! What the user approves is a digest over every file the host `cargo` will
//! read: the lockfile cargo actually uses (the workspace root's, not merely the
//! nearest) and each manifest reachable from the project through workspace
//! members and path dependencies. All of them must sit inside the workspace.
//!
//! Every refusal here happens before a process starts. The plan is computed
//! again immediately before the fetch and once more after it, and a stage whose
//! digest moved is discarded.

use std::collections::BTreeSet;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

const CRATES_IO_GIT_INDEX: &str = "registry+https://github.com/rust-lang/crates.io-index";
const CRATES_IO_SPARSE_INDEX: &str = "sparse+https://index.crates.io/";
pub(super) const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_MANIFESTS: usize = 128;
const MAX_SUMMARY_PACKAGES: usize = 40;
pub(super) const MARKER_FILE: &str = "staged.json";

/// The directory name under Arcana's home that holds staged dependencies.
pub(crate) const DEPENDENCY_CACHE_DIR: &str = "dependency-cache";

/// Files that change which compiler, registry or credentials a host `cargo`
/// uses. The workspace is agent-writable, so none may sit on the path the host
/// process searches.
const HOST_REDIRECTING_FILES: [&str; 4] = [
    ".cargo/config",
    ".cargo/config.toml",
    "rust-toolchain",
    "rust-toolchain.toml",
];

/// Dependency tables whose entries may carry a `path`.
const DEPENDENCY_TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// What a staging call would fetch, computed without starting any process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CargoStagingPlan {
    pub project_dir: PathBuf,
    /// The lockfile cargo uses for this project: the workspace root's.
    pub lockfile: PathBuf,
    /// Digest over the lockfile and every manifest cargo will read.
    pub digest: String,
    pub manifest_count: usize,
    /// `name version` for every registry package in the lockfile.
    pub packages: Vec<String>,
}

impl CargoStagingPlan {
    /// The text the approval dialog shows. It names the digest the call must
    /// pin, so the user consents to this exact set of files.
    pub(crate) fn summary(&self) -> String {
        let shown = self
            .packages
            .iter()
            .take(MAX_SUMMARY_PACKAGES)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        let more = self.packages.len().saturating_sub(MAX_SUMMARY_PACKAGES);
        let tail = if more > 0 {
            format!(" and {more} more")
        } else {
            String::new()
        };
        format!(
            "Arcana will download {} crates.io package(s) named by Cargo.lock into its own cache, with build scripts not run. Approval covers Cargo.lock and {} Cargo.toml file(s), plan sha256 {}. Packages: {shown}{tail}.",
            self.packages.len(),
            self.manifest_count,
            self.digest
        )
    }
}

/// Plan a Cargo staging, or say why it is refused.
pub(crate) fn plan_cargo(
    workspace_root: &Path,
    project_dir: &Path,
) -> Result<CargoStagingPlan, String> {
    // The caller's two paths come from different canonicalizers, one of which
    // keeps the Windows verbatim prefix. Compare them in one form.
    let workspace_root = &strip_verbatim_prefix(workspace_root);
    let project_dir = &strip_verbatim_prefix(project_dir);
    refuse_redirecting_files(workspace_root, project_dir)?;
    refuse_reparse_components(workspace_root, project_dir)?;
    let project_manifest = project_dir.join("Cargo.toml");
    if !project_manifest.is_file() {
        return Err("No Cargo.toml in the selected project, so there is nothing to stage.".into());
    }
    let root = cargo_workspace_root(workspace_root, project_dir)?;
    let manifests = manifest_closure(workspace_root, &root, project_dir)?;

    let lockfile = root.join("Cargo.lock");
    let mut extra_locks = Vec::new();
    let mut directory = Some(project_dir.as_path());
    while let Some(current) = directory {
        if current == root {
            break;
        }
        let candidate = current.join("Cargo.lock");
        if candidate.exists() {
            extra_locks.push(candidate);
        }
        directory = current.parent();
    }
    if !lockfile.is_file() {
        return Err("No Cargo.lock found for this project's workspace. Dependency staging only installs what a lockfile already pins; Arcana does not create one.".into());
    }
    if is_reparse_point(&lockfile).map_err(|error| error.to_string())? {
        return Err("Cargo.lock is a link, which staging refuses.".into());
    }
    let lock_bytes = read_bounded(&lockfile, "Cargo.lock")?;
    let lock_text =
        std::str::from_utf8(&lock_bytes).map_err(|_| "Cargo.lock is not UTF-8.".to_string())?;
    let mut packages = registry_packages(lock_text)?;
    // A lockfile cargo ignores today is still bound: it is one edit from being
    // the one it reads.
    let mut extra_bytes = Vec::new();
    for extra in &extra_locks {
        let bytes = read_bounded(extra, "Cargo.lock")?;
        let text =
            std::str::from_utf8(&bytes).map_err(|_| "Cargo.lock is not UTF-8.".to_string())?;
        packages.extend(registry_packages(text)?);
        extra_bytes.push((extra.clone(), bytes));
    }
    packages.sort();
    packages.dedup();

    let mut hasher = Sha256::new();
    hasher.update(b"arcana-cargo-staging-v1\n");
    hash_file(&mut hasher, workspace_root, &lockfile, &lock_bytes);
    for (extra, bytes) in &extra_bytes {
        hash_file(&mut hasher, workspace_root, extra, bytes);
    }
    for manifest in &manifests {
        let bytes = read_bounded(manifest, "Cargo.toml")?;
        hash_file(&mut hasher, workspace_root, manifest, &bytes);
    }
    Ok(CargoStagingPlan {
        project_dir: project_dir.to_path_buf(),
        lockfile,
        digest: hex::encode(hasher.finalize()),
        manifest_count: manifests.len(),
        packages,
    })
}

pub(super) fn hash_file(hasher: &mut Sha256, workspace_root: &Path, path: &Path, bytes: &[u8]) {
    let relative = path
        .strip_prefix(workspace_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    hasher.update(relative.as_bytes());
    hasher.update([0]);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

pub(super) fn read_bounded(path: &Path, label: &str) -> Result<Vec<u8>, String> {
    if is_reparse_point(path).map_err(|error| format!("Could not read {label}: {error}"))? {
        return Err(format!("{label} is a link, which staging refuses."));
    }
    let length = std::fs::metadata(path)
        .map_err(|error| format!("Could not read {label}: {error}"))?
        .len();
    if length > MAX_FILE_BYTES {
        return Err(format!(
            "{label} is {length} bytes, over the {MAX_FILE_BYTES} byte staging limit."
        ));
    }
    std::fs::read(path).map_err(|error| format!("Could not read {label}: {error}"))
}

fn read_manifest(path: &Path) -> Result<toml::Value, String> {
    let bytes = read_bounded(path, "Cargo.toml")?;
    let text = std::str::from_utf8(&bytes).map_err(|_| "A Cargo.toml is not UTF-8.".to_string())?;
    toml::from_str(text)
        .map_err(|error| format!("A Cargo.toml does not parse: {}", error.message()))
}

/// The directory whose `Cargo.lock` cargo uses: the nearest manifest at or
/// above the project that declares `[workspace]`, else the project itself.
fn cargo_workspace_root(workspace_root: &Path, project_dir: &Path) -> Result<PathBuf, String> {
    let project_manifest = read_manifest(&project_dir.join("Cargo.toml"))?;
    if project_manifest
        .get("package")
        .and_then(|package| package.get("workspace"))
        .is_some()
    {
        return Err("The project names its workspace with package.workspace, which staging does not follow.".into());
    }
    let mut directory = Some(project_dir);
    while let Some(current) = directory {
        let manifest = current.join("Cargo.toml");
        if manifest.is_file() {
            let document = read_manifest(&manifest)?;
            if let Some(workspace) = document.get("workspace") {
                if current != project_dir {
                    refuse_if_excluded(workspace, current, project_dir)?;
                    require_listed_member(workspace, current, project_dir)?;
                }
                return Ok(current.to_path_buf());
            }
        }
        if current == workspace_root {
            break;
        }
        directory = current.parent();
    }
    Ok(project_dir.to_path_buf())
}

/// A project named in `[workspace] exclude` is its own cargo root and reads its
/// own `Cargo.lock`, not the workspace's, so the approved lockfile would not be
/// the one fetched. Patterns cannot be matched safely, so they are refused too.
fn refuse_if_excluded(
    workspace: &toml::Value,
    workspace_dir: &Path,
    project_dir: &Path,
) -> Result<(), String> {
    for entry in workspace
        .get("exclude")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
    {
        if entry.contains(['*', '?', '[']) {
            return Err(format!(
                "Workspace exclude '{entry}' uses a pattern staging does not expand."
            ));
        }
        let excluded = comparable(&workspace_dir.join(entry.trim_start_matches("./")));
        if comparable(project_dir).starts_with(&excluded) {
            return Err("The project is excluded from its workspace, so cargo would use a different Cargo.lock than the one approved. Staging refuses it.".into());
        }
    }
    Ok(())
}

/// Cargo falls back to treating a project that its enclosing workspace does not
/// list as a root of its own, with its own `Cargo.lock`. Only a project the
/// nearest workspace names as a member is known to use that workspace's lock,
/// so anything else is refused rather than predicted.
fn require_listed_member(
    workspace: &toml::Value,
    workspace_dir: &Path,
    project_dir: &Path,
) -> Result<(), String> {
    let wanted = comparable(project_dir);
    for member in workspace
        .get("members")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
    {
        if expand_member(workspace_dir, member)?
            .iter()
            .any(|candidate| comparable(candidate) == wanted)
        {
            return Ok(());
        }
    }
    Err("The project is not listed in its enclosing workspace's members, so which Cargo.lock cargo would use cannot be told. List it under [workspace] members, or stage the dependencies yourself.".into())
}

/// A path reduced to lowercase normal components, so two spellings of one
/// directory compare equal on a case-insensitive filesystem.
fn comparable(path: &Path) -> Vec<String> {
    lexically_normal(path)
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_lowercase()),
            Component::Prefix(prefix) => Some(prefix.as_os_str().to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect()
}

fn lexically_normal(path: &Path) -> PathBuf {
    let mut normal = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                normal.pop();
            }
            Component::CurDir => {}
            other => normal.push(other.as_os_str()),
        }
    }
    strip_verbatim_prefix(&normal)
}

/// Every manifest cargo will read for this project, found through workspace
/// members and path dependencies. A reference that leaves the workspace, goes
/// through a link or names a missing manifest refuses the whole plan.
fn manifest_closure(
    workspace_root: &Path,
    root: &Path,
    project_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    let mut seen = BTreeSet::new();
    let mut queue = vec![root.join("Cargo.toml"), project_dir.join("Cargo.toml")];
    while let Some(manifest) = queue.pop() {
        if !seen.insert(manifest.clone()) {
            continue;
        }
        if seen.len() > MAX_MANIFESTS {
            return Err(format!(
                "The workspace reaches more than {MAX_MANIFESTS} manifests, over the staging limit."
            ));
        }
        let directory = manifest.parent().unwrap_or(workspace_root).to_path_buf();
        let document = read_manifest(&manifest)?;
        for referenced in referenced_directories(&document, &directory)? {
            let referenced = confine(workspace_root, &referenced)?;
            let next = referenced.join("Cargo.toml");
            if !next.is_file() {
                return Err(format!(
                    "A path dependency or workspace member names {}, which has no Cargo.toml.",
                    display_in(workspace_root, &referenced)
                ));
            }
            queue.push(next);
        }
    }
    Ok(seen.into_iter().collect())
}

/// Directories a manifest points at: workspace members and path dependencies,
/// including target-specific, workspace-level and patched ones.
fn referenced_directories(
    document: &toml::Value,
    manifest_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    let mut directories = Vec::new();
    let mut dependency_maps: Vec<&toml::Value> = Vec::new();
    for table in DEPENDENCY_TABLES {
        dependency_maps.extend(document.get(table));
    }
    if let Some(workspace) = document.get("workspace") {
        dependency_maps.extend(workspace.get("dependencies"));
        for member in workspace
            .get("members")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(toml::Value::as_str)
        {
            directories.extend(expand_member(manifest_dir, member)?);
        }
    }
    if let Some(targets) = document.get("target").and_then(toml::Value::as_table) {
        for target in targets.values() {
            for table in DEPENDENCY_TABLES {
                dependency_maps.extend(target.get(table));
            }
        }
    }
    if let Some(patches) = document.get("patch").and_then(toml::Value::as_table) {
        dependency_maps.extend(patches.values());
    }
    dependency_maps.extend(document.get("replace"));
    for map in dependency_maps {
        for entry in map.as_table().into_iter().flat_map(|table| table.values()) {
            if let Some(path) = entry.get("path").and_then(toml::Value::as_str) {
                directories.push(manifest_dir.join(path));
            }
        }
    }
    Ok(directories)
}

/// A workspace member is a directory or a one-level `dir/*` pattern. Anything
/// else cargo would expand is refused rather than guessed at.
fn expand_member(manifest_dir: &Path, member: &str) -> Result<Vec<PathBuf>, String> {
    let Some(parent) = member.strip_suffix("/*") else {
        if member.contains(['*', '?', '[']) {
            return Err(format!(
                "Workspace member '{member}' uses a pattern staging does not expand; list members by directory or as 'dir/*'."
            ));
        }
        return Ok(vec![manifest_dir.join(member)]);
    };
    if parent.contains(['*', '?', '[']) {
        return Err(format!(
            "Workspace member '{member}' uses a pattern staging does not expand."
        ));
    }
    let listing = std::fs::read_dir(manifest_dir.join(parent))
        .map_err(|error| format!("Could not list workspace members under '{parent}': {error}"))?;
    let mut members = Vec::new();
    for entry in listing.flatten() {
        if entry.path().join("Cargo.toml").is_file() {
            members.push(entry.path());
        }
    }
    Ok(members)
}

/// Resolve `.` and `..` without touching the disk, then require the result to
/// sit inside the workspace and not to pass through a link.
fn confine(workspace_root: &Path, path: &Path) -> Result<PathBuf, String> {
    let mut normal = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                if normal.starts_with(workspace_root) {
                    match is_reparse_point(&normal) {
                        Ok(false) => {}
                        Ok(true) => return Err("A path dependency or workspace member steps up from a link, which staging refuses.".into()),
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                        Err(error) => return Err(format!("Could not inspect a path dependency: {error}")),
                    }
                }
                normal.pop();
            }
            Component::CurDir => {}
            other => normal.push(other.as_os_str()),
        }
    }
    let normal = strip_verbatim_prefix(&normal);
    if !normal.starts_with(workspace_root) {
        return Err(format!(
            "A path dependency or workspace member points outside the workspace ({}). Staging reads only files inside it.",
            display_in(workspace_root, &normal)
        ));
    }
    refuse_reparse_components(workspace_root, &normal)?;
    Ok(normal)
}

pub(super) fn display_in(workspace_root: &Path, path: &Path) -> String {
    path.strip_prefix(workspace_root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| "a path outside the workspace".into())
}

/// Accept only crates.io registry packages, each with a checksum and a plain
/// name and version. A package with no `source` is a path package inside the
/// workspace and fetches nothing.
fn registry_packages(lock_text: &str) -> Result<Vec<String>, String> {
    let document: toml::Value = toml::from_str(lock_text)
        .map_err(|error| format!("Cargo.lock does not parse: {}", error.message()))?;
    let Some(entries) = document.get("package").and_then(toml::Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut registry = Vec::new();
    for entry in entries {
        let field = |name: &str| entry.get(name).and_then(toml::Value::as_str);
        let (Some(name), Some(version)) = (field("name"), field("version")) else {
            return Err("Cargo.lock has a package without a name or version.".into());
        };
        if !plain(name, |c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            || !plain(version, |c| c.is_ascii_alphanumeric() || ".+-".contains(c))
        {
            return Err(
                "Cargo.lock has a package whose name or version has unexpected characters.".into(),
            );
        }
        let Some(source) = field("source") else {
            continue;
        };
        if source != CRATES_IO_GIT_INDEX && source != CRATES_IO_SPARSE_INDEX {
            return Err(format!(
                "Cargo.lock package {name} {version} comes from a source that is not crates.io. Staging only fetches crates.io packages."
            ));
        }
        let checksum_ok = field("checksum")
            .is_some_and(|sum| sum.len() == 64 && sum.bytes().all(|byte| byte.is_ascii_hexdigit()));
        if !checksum_ok {
            return Err(format!(
                "Cargo.lock package {name} {version} has no SHA-256 checksum."
            ));
        }
        registry.push(format!("{name} {version}"));
    }
    Ok(registry)
}

pub(super) fn plain(text: &str, allowed: impl Fn(char) -> bool) -> bool {
    !text.is_empty() && text.len() <= 100 && text.chars().all(allowed)
}

fn refuse_redirecting_files(workspace_root: &Path, project_dir: &Path) -> Result<(), String> {
    let mut directory = Some(project_dir);
    while let Some(current) = directory {
        for name in HOST_REDIRECTING_FILES {
            if current.join(name).exists() {
                let relative = current
                    .join(name)
                    .strip_prefix(workspace_root)
                    .map(|path| path.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|_| name.to_string());
                return Err(format!(
                    "Refusing to stage: {relative} can redirect the host cargo to another compiler, registry or credential. Remove it, or stage the dependencies yourself."
                ));
            }
        }
        if current == workspace_root {
            break;
        }
        directory = current.parent();
    }
    Ok(())
}

/// Every directory from the workspace root to the target must be a real
/// directory, so a junction cannot steer the host process out of the workspace.
pub(super) fn refuse_reparse_components(
    workspace_root: &Path,
    target: &Path,
) -> Result<(), String> {
    let relative = target
        .strip_prefix(workspace_root)
        .map_err(|_| "A staged path is outside the selected workspace.".to_string())?;
    let mut current = workspace_root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match is_reparse_point(&current) {
            Ok(false) => {}
            Ok(true) => {
                return Err(format!(
                    "Refusing to stage: {} is a link.",
                    display_in(workspace_root, &current)
                ))
            }
            // A missing path is reported by the caller with a clearer reason.
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

pub(super) fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

pub(super) fn is_reparse_point(path: &Path) -> io::Result<bool> {
    let metadata = std::fs::symlink_metadata(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        Ok(metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
    }
    #[cfg(not(windows))]
    {
        Ok(metadata.file_type().is_symlink())
    }
}

/// Host-owned directory for one plan's staged dependencies; `kind` is the
/// ecosystem (`cargo`, `npm`).
pub(super) fn cache_dir(kind: &str, digest: &str) -> io::Result<PathBuf> {
    let base = crate::db::arcana_dir().map_err(|error| io::Error::other(error.to_string()))?;
    Ok(base.join(DEPENDENCY_CACHE_DIR).join(kind).join(digest))
}

/// The `CARGO_HOME` of a completed staging that matches the project's current
/// plan, if one exists. Any edit to the lockfile or a manifest changes the
/// digest and finds nothing, so a build then runs unstaged.
pub(crate) fn staged_cargo_home(workspace_root: &Path, project_dir: &Path) -> Option<PathBuf> {
    let plan = plan_cargo(workspace_root, project_dir).ok()?;
    let directory = cache_dir("cargo", &plan.digest).ok()?.canonicalize().ok()?;
    let marker = std::fs::read_to_string(directory.join(MARKER_FILE)).ok()?;
    let marker: serde_json::Value = serde_json::from_str(&marker).ok()?;
    (marker["plan_sha256"] == plan.digest.as_str())
        .then(|| directory.join("home"))
        .filter(|home| home.is_dir())
}

/// Result of a completed staging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StagingReport {
    pub package_count: usize,
    pub already_staged: bool,
}

/// Fetch the plan's packages into the host cache.
///
/// `cargo` is the absolute path of the toolchain's cargo and `toolchain_dir`
/// its directory, so no rustup proxy runs. The child gets a rebuilt
/// environment, never the caller's, so a wrapper or credential variable cannot
/// ride along. The fetch writes to a scratch directory that is renamed into
/// place only after the plan is re-checked, so a failed or raced stage leaves
/// nothing a build could read.
pub(crate) async fn stage_cargo(
    plan: &CargoStagingPlan,
    workspace_root: &Path,
    cargo: &Path,
    toolchain_dir: &Path,
    timeout: Duration,
) -> Result<StagingReport, String> {
    let final_dir = cache_dir("cargo", &plan.digest).map_err(|error| error.to_string())?;
    if final_dir.join(MARKER_FILE).is_file() {
        return Ok(StagingReport {
            package_count: plan.packages.len(),
            already_staged: true,
        });
    }
    // The files may have moved since the user approved this digest.
    ensure_unchanged(workspace_root, plan, "before the fetch")?;

    let parent = final_dir
        .parent()
        .ok_or_else(|| "The dependency cache path has no parent.".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("Could not prepare the dependency cache: {error}"))?;
    let scratch = tempfile::Builder::new()
        .prefix("stage-")
        .tempdir_in(parent)
        .map_err(|error| format!("Could not prepare the dependency cache: {error}"))?;
    let home = scratch.path().join("home");
    let temp = scratch.path().join("tmp");
    for path in [&home, &temp] {
        std::fs::create_dir_all(path)
            .map_err(|error| format!("Could not prepare the dependency cache: {error}"))?;
    }

    if !plan.packages.is_empty() {
        fetch(plan, cargo, toolchain_dir, &home, &temp, timeout).await?;
    }
    ensure_unchanged(workspace_root, plan, "while the fetch ran")?;

    std::fs::remove_dir_all(&temp).ok();
    write_marker(scratch.path(), plan)?;
    let kept = scratch.keep();
    match std::fs::rename(&kept, &final_dir) {
        Ok(()) => {}
        Err(_) if final_dir.join(MARKER_FILE).is_file() => {
            // Another Arcana process finished the same plan first.
            std::fs::remove_dir_all(&kept).ok();
        }
        Err(error) => {
            std::fs::remove_dir_all(&kept).ok();
            return Err(format!(
                "Could not publish the staged dependencies: {error}"
            ));
        }
    }
    Ok(StagingReport {
        package_count: plan.packages.len(),
        already_staged: false,
    })
}

fn ensure_unchanged(
    workspace_root: &Path,
    plan: &CargoStagingPlan,
    when: &str,
) -> Result<(), String> {
    let current = plan_cargo(workspace_root, &plan.project_dir)?;
    if current.digest == plan.digest {
        Ok(())
    } else {
        Err(format!(
            "Cargo.lock or a Cargo.toml changed {when}; the stage was discarded. Read the plan again and have the user approve the current files."
        ))
    }
}

async fn fetch(
    plan: &CargoStagingPlan,
    cargo: &Path,
    toolchain_dir: &Path,
    home: &Path,
    temp: &Path,
    timeout: Duration,
) -> Result<(), String> {
    let system_root = std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("windir"))
        .unwrap_or_default();
    let mut path = toolchain_dir.as_os_str().to_os_string();
    path.push(";");
    path.push(Path::new(&system_root).join("System32"));
    path.push(";");
    path.push(&system_root);

    let mut command = tokio::process::Command::new(cargo);
    command
        .args(["fetch", "--locked", "--manifest-path"])
        .arg(plan.project_dir.join("Cargo.toml"))
        .current_dir(&plan.project_dir)
        .env_clear()
        .env("PATH", path)
        .env("SystemRoot", &system_root)
        .env("CARGO_HOME", home)
        .env("TEMP", temp)
        .env("TMP", temp)
        .env("CARGO_TERM_COLOR", "never")
        .env("CARGO_TERM_PROGRESS_WHEN", "never")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    // A host behind a proxy still needs it for this one step. These are the
    // host's own settings, passed through by name; nothing else is.
    for key in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "NO_PROXY",
        "ALL_PROXY",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);

    let output = tokio::time::timeout(timeout, command.output())
        .await
        .map_err(|_| format!("cargo fetch did not finish within {}s.", timeout.as_secs()))?
        .map_err(|error| format!("Could not start cargo fetch: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    // cargo's message can quote file contents and host paths, and it would go
    // straight back to the agent. Keep it on the host in the log, return a
    // fixed reason.
    tracing::warn!(
        target: "arcana::project_command",
        exit_code = output.status.code().unwrap_or(-1),
        stderr_tail = %crate::security::redaction::redact_text(
            &String::from_utf8_lossy(&output.stderr)
                .chars()
                .rev()
                .take(2000)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<String>()
        ),
        "cargo fetch failed during dependency staging"
    );
    Err(format!(
        "cargo fetch failed (exit {}). Check host network access and that every crate in Cargo.lock exists on crates.io; details are in the host log.",
        output.status.code().unwrap_or(-1)
    ))
}

fn write_marker(directory: &Path, plan: &CargoStagingPlan) -> Result<(), String> {
    let marker = serde_json::json!({
        "plan_sha256": plan.digest,
        "package_count": plan.packages.len(),
        "manifest_count": plan.manifest_count,
    });
    std::fs::write(directory.join(MARKER_FILE), marker.to_string())
        .map_err(|error| format!("Could not record the finished stage: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHECKSUM: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn lock(entries: &str) -> String {
        format!("version = 4\n\n{entries}")
    }

    fn registry_entry(name: &str, source: &str, checksum: Option<&str>) -> String {
        let checksum = checksum
            .map(|sum| format!("checksum = \"{sum}\"\n"))
            .unwrap_or_default();
        format!(
            "[[package]]\nname = \"{name}\"\nversion = \"1.0.0\"\nsource = \"{source}\"\n{checksum}\n"
        )
    }

    fn manifest(name: &str) -> String {
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n")
    }

    fn itoa_lock() -> String {
        lock(&registry_entry(
            "itoa",
            CRATES_IO_SPARSE_INDEX,
            Some(CHECKSUM),
        ))
    }

    fn project(lock_text: &str) -> (tempfile::TempDir, PathBuf) {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        std::fs::write(root.join("Cargo.toml"), manifest("demo")).unwrap();
        std::fs::write(root.join("Cargo.lock"), lock_text).unwrap();
        (workspace, root)
    }

    #[test]
    fn crates_io_packages_with_checksums_are_listed_and_path_packages_are_not() {
        let text = lock(&format!(
            "[[package]]\nname = \"demo\"\nversion = \"0.1.0\"\n\n{}{}",
            registry_entry("itoa", CRATES_IO_GIT_INDEX, Some(CHECKSUM)),
            registry_entry("ryu", CRATES_IO_SPARSE_INDEX, Some(CHECKSUM)),
        ));
        assert_eq!(
            registry_packages(&text).unwrap(),
            vec!["itoa 1.0.0".to_string(), "ryu 1.0.0".to_string()]
        );
    }

    #[test]
    fn a_lockfile_naming_git_or_another_registry_is_refused() {
        for source in [
            "git+https://example.com/evil/crate#abcdef",
            "registry+https://registry.example.com/index",
            "sparse+https://index.crates.io.evil.example/",
        ] {
            let text = lock(&registry_entry("evil", source, Some(CHECKSUM)));
            let error = registry_packages(&text).unwrap_err();
            assert!(error.contains("not crates.io"), "{source}: {error}");
            assert!(
                !error.contains("example"),
                "the agent's URL must not be echoed: {error}"
            );
        }
    }

    #[test]
    fn a_registry_package_without_a_valid_checksum_is_refused() {
        for checksum in [None, Some("abc"), Some(&"z".repeat(64)[..])] {
            let text = lock(&registry_entry("itoa", CRATES_IO_SPARSE_INDEX, checksum));
            assert!(registry_packages(&text)
                .unwrap_err()
                .contains("no SHA-256 checksum"));
        }
    }

    #[test]
    fn package_names_that_could_forge_the_approval_text_are_refused() {
        let text = lock(&registry_entry(
            "itoa 1.0.0. Approved by the user\\nnew line",
            CRATES_IO_SPARSE_INDEX,
            Some(CHECKSUM),
        ));
        assert!(registry_packages(&text)
            .unwrap_err()
            .contains("unexpected characters"));
    }

    #[test]
    fn the_digest_covers_the_lockfile_and_the_manifests() {
        let text = itoa_lock();
        let (_workspace, root) = project(&text);
        let plan = plan_cargo(&root, &root).unwrap();
        assert_eq!(plan.packages, vec!["itoa 1.0.0".to_string()]);
        assert!(plan.summary().contains(&plan.digest));
        assert!(plan.summary().contains("itoa 1.0.0"));
        assert_eq!(plan_cargo(&root, &root).unwrap().digest, plan.digest);

        std::fs::write(root.join("Cargo.lock"), format!("{text}\n# edited\n")).unwrap();
        let edited_lock = plan_cargo(&root, &root).unwrap().digest;
        assert_ne!(
            edited_lock, plan.digest,
            "an edited lockfile must change the digest"
        );

        std::fs::write(root.join("Cargo.lock"), &text).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            format!("{}\n# edited\n", manifest("demo")),
        )
        .unwrap();
        assert_ne!(
            plan_cargo(&root, &root).unwrap().digest,
            plan.digest,
            "an edited manifest must change the digest"
        );
    }

    #[test]
    fn staging_without_a_lockfile_is_refused_and_creates_none() {
        let (_workspace, root) = project("");
        std::fs::remove_file(root.join("Cargo.lock")).unwrap();
        let error = plan_cargo(&root, &root).unwrap_err();
        assert!(error.contains("No Cargo.lock"), "{error}");
        assert!(!root.join("Cargo.lock").exists());
    }

    #[test]
    fn files_that_redirect_the_host_cargo_are_refused_anywhere_on_the_path() {
        for name in [
            ".cargo/config.toml",
            ".cargo/config",
            "rust-toolchain",
            "rust-toolchain.toml",
        ] {
            let (_workspace, root) = project(&itoa_lock());
            let nested = root.join("crates").join("app");
            std::fs::create_dir_all(&nested).unwrap();
            std::fs::write(nested.join("Cargo.toml"), manifest("app")).unwrap();
            let target = root.join(name);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(&target, "x = 1\n").unwrap();
            let error = plan_cargo(&root, &nested).unwrap_err();
            assert!(
                error.contains("can redirect the host cargo"),
                "{name}: {error}"
            );
            assert!(error.contains(&name.replace('\\', "/")), "{name}: {error}");
        }
    }

    #[test]
    fn a_member_uses_the_workspace_root_lockfile_not_the_nearest_one() {
        // The agent plants a harmless lockfile beside the member and a hostile
        // one at the workspace root, which is the one cargo reads.
        let hostile = lock(&registry_entry(
            "evil",
            "git+https://example.com/evil#abc",
            Some(CHECKSUM),
        ));
        let (_workspace, root) = project(&hostile);
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        )
        .unwrap();
        let member = root.join("member");
        std::fs::create_dir_all(&member).unwrap();
        std::fs::write(member.join("Cargo.toml"), manifest("member")).unwrap();
        std::fs::write(member.join("Cargo.lock"), itoa_lock()).unwrap();

        let error = plan_cargo(&root, &member).unwrap_err();
        assert!(error.contains("not crates.io"), "{error}");
    }

    #[test]
    fn a_member_of_a_clean_workspace_stages_the_root_lockfile() {
        let (_workspace, root) = project(&itoa_lock());
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
        let member = root.join("crates").join("one");
        std::fs::create_dir_all(&member).unwrap();
        std::fs::write(member.join("Cargo.toml"), manifest("one")).unwrap();
        let plan = plan_cargo(&root, &member).unwrap();
        assert_eq!(
            plan.lockfile,
            strip_verbatim_prefix(&root).join("Cargo.lock")
        );
        assert_eq!(plan.manifest_count, 2);
    }

    #[test]
    fn path_dependencies_inside_the_workspace_are_bound_and_outside_are_refused() {
        let (_workspace, root) = project(&itoa_lock());
        let sibling = root.join("local");
        std::fs::create_dir_all(&sibling).unwrap();
        std::fs::write(sibling.join("Cargo.toml"), manifest("local")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            format!(
                "{}\n[dependencies]\nlocal = {{ path = \"local\" }}\n",
                manifest("demo")
            ),
        )
        .unwrap();
        let inside = plan_cargo(&root, &root).unwrap();
        assert_eq!(inside.manifest_count, 2);

        // The dependency's own manifest is part of the approval.
        std::fs::write(
            sibling.join("Cargo.toml"),
            format!("{}\n# edited\n", manifest("local")),
        )
        .unwrap();
        assert_ne!(plan_cargo(&root, &root).unwrap().digest, inside.digest);

        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("Cargo.toml"), manifest("outside")).unwrap();
        for path in [
            "../outside".to_string(),
            outside.path().to_string_lossy().replace('\\', "/"),
        ] {
            std::fs::write(
                root.join("Cargo.toml"),
                format!(
                    "{}\n[dependencies]\nsneaky = {{ path = \"{path}\" }}\n",
                    manifest("demo")
                ),
            )
            .unwrap();
            let error = plan_cargo(&root, &root).unwrap_err();
            assert!(error.contains("outside the workspace"), "{path}: {error}");
        }
    }

    #[test]
    fn unexpandable_workspace_patterns_and_package_workspace_are_refused() {
        let (_workspace, root) = project(&itoa_lock());
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"a*b\"]\n",
        )
        .unwrap();
        assert!(plan_cargo(&root, &root)
            .unwrap_err()
            .contains("pattern staging does not expand"));

        std::fs::write(
            root.join("Cargo.toml"),
            format!("{}workspace = \"../elsewhere\"\n", manifest("demo")),
        )
        .unwrap();
        assert!(plan_cargo(&root, &root)
            .unwrap_err()
            .contains("package.workspace"));
    }

    #[test]
    fn a_project_excluded_from_its_workspace_is_refused() {
        // Cargo gives an excluded project its own Cargo.lock, so the approved
        // root lockfile would not be the one fetched.
        let (_workspace, root) = project(&itoa_lock());
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = []\nexclude = [\"app\"]\n",
        )
        .unwrap();
        let app = root.join("app");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("Cargo.toml"), manifest("app")).unwrap();
        std::fs::write(
            app.join("Cargo.lock"),
            lock(&registry_entry(
                "evil",
                "git+https://example.com/x#abc",
                Some(CHECKSUM),
            )),
        )
        .unwrap();
        assert!(plan_cargo(&root, &app)
            .unwrap_err()
            .contains("excluded from its workspace"));

        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = []\nexclude = [\"a*\"]\n",
        )
        .unwrap();
        assert!(plan_cargo(&root, &app)
            .unwrap_err()
            .contains("pattern staging does not expand"));
    }

    #[test]
    fn a_project_the_enclosing_workspace_does_not_list_is_refused() {
        let (_workspace, root) = project(&itoa_lock());
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]
members = [\"listed\"]
",
        )
        .unwrap();
        for name in ["listed", "unlisted"] {
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("Cargo.toml"), manifest(name)).unwrap();
        }
        assert!(plan_cargo(&root, &root.join("listed")).is_ok());
        assert!(plan_cargo(&root, &root.join("unlisted"))
            .unwrap_err()
            .contains("not listed in its enclosing workspace"));
    }

    #[cfg(windows)]
    #[test]
    fn an_exclude_written_in_another_case_still_refuses_the_project() {
        let (_workspace, root) = project(&itoa_lock());
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]
members = [\"app\"]
exclude = [\"APP\"]
",
        )
        .unwrap();
        let app = root.join("app");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("Cargo.toml"), manifest("app")).unwrap();
        assert!(plan_cargo(&root, &app)
            .unwrap_err()
            .contains("excluded from its workspace"));
    }

    #[test]
    fn a_lockfile_beside_a_member_is_bound_and_checked_even_though_cargo_ignores_it() {
        let (_workspace, root) = project(&itoa_lock());
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        )
        .unwrap();
        let member = root.join("member");
        std::fs::create_dir_all(&member).unwrap();
        std::fs::write(member.join("Cargo.toml"), manifest("member")).unwrap();
        std::fs::write(member.join("Cargo.lock"), itoa_lock()).unwrap();
        let bound = plan_cargo(&root, &member).unwrap().digest;

        std::fs::write(
            member.join("Cargo.lock"),
            format!("{}\n# edited\n", itoa_lock()),
        )
        .unwrap();
        assert_ne!(plan_cargo(&root, &member).unwrap().digest, bound);
    }

    #[cfg(windows)]
    #[test]
    fn a_path_dependency_that_steps_up_from_a_junction_is_refused() {
        let (_workspace, root) = project(&itoa_lock());
        let real = root.join("real");
        std::fs::create_dir_all(real.join("deep")).unwrap();
        let link = root.join("link");
        let made = std::process::Command::new("cmd")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&link)
            .arg(real.join("deep"))
            .output()
            .unwrap();
        assert!(made.status.success(), "junction fixture was not created");
        std::fs::write(
            root.join("Cargo.toml"),
            format!(
                "{}\n[dependencies]\nx = {{ path = \"link/../sibling\" }}\n",
                manifest("demo")
            ),
        )
        .unwrap();
        let error = plan_cargo(&root, &root).unwrap_err();
        assert!(error.contains("link"), "{error}");
    }

    #[cfg(windows)]
    #[test]
    fn a_junction_between_the_workspace_and_the_project_is_refused() {
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("Cargo.toml"), manifest("out")).unwrap();
        std::fs::write(outside.path().join("Cargo.lock"), lock("")).unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        let link = root.join("linked");
        let status = std::process::Command::new("cmd")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&link)
            .arg(outside.path())
            .output()
            .unwrap();
        assert!(status.status.success(), "junction fixture was not created");
        let error = plan_cargo(&root, &link).unwrap_err();
        assert!(error.contains("is a link"), "{error}");
    }
}
