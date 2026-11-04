//! Host-controlled npm dependency staging from a lockfile (`D-008`).
//!
//! The Cargo half of this lives in `dependency_staging`; this is the npm half
//! and follows the same rules: the host fetches with the network open for that
//! one step, outside the sandbox, and the sandboxed build then runs offline.
//!
//! npm differs in one way that shapes the design. A build finds its packages by
//! walking up from the project to a `node_modules` directory, so a read-only
//! grant to a cache elsewhere cannot serve them. Instead the host installs into
//! a scratch project that holds only the approved `package.json` and lockfile
//! bytes, publishes the result under the digest in Arcana's own cache, and copies
//! that tree into the project's `node_modules`. The cache is never readable by
//! the sandbox, and nothing the host later reads or runs comes from the
//! workspace copy.
//!
//! What the user approves is a digest over `package.json` and every lockfile npm
//! might read (`npm-shrinkwrap.json` takes precedence over `package-lock.json`,
//! and both are bound). Every lockfile entry that npm could fetch must name the
//! canonical `registry.npmjs.org` tarball for its own name and version, with a
//! sha512 integrity value; git, file, link and other-host entries, dependency
//! specs in `package.json` that are not plain registry ranges, `.npmrc` files and
//! npm workspaces are refused before any process starts, and `npm ci` is also
//! told to allow no git, file, directory or remote source. Install scripts are
//! never run.
//!
//! A project `.npmrc` is not copied into the scratch project, so it cannot
//! redirect the fetch. It is refused because it says the project expects another
//! registry or credentials, and a tree installed from the public registry would
//! not be what its own `npm ci` produces.
//!
//! Residuals, recorded in `D-008`: the sandboxed build can edit `node_modules`
//! after it is copied in (the next stage replaces it); a file hard-linked into the
//! cache by npm is not detected; and a registry package that is a decompression
//! bomb is bounded only after it has been written, by [`measure_tree`].

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::dependency_staging::{
    cache_dir, hash_file, plain, read_bounded, refuse_reparse_components, strip_verbatim_prefix,
    MARKER_FILE,
};

const REGISTRY: &str = "https://registry.npmjs.org/";
const MAX_PACKAGES: usize = 5_000;
const MAX_SUMMARY_PACKAGES: usize = 40;
const MAX_TREE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_TREE_ENTRIES: usize = 400_000;
const NODE_MODULES: &str = "node_modules";
/// Written inside a `node_modules` that Arcana installed, holding the plan
/// digest, so a later stage can tell its own tree from one the agent made.
const INSTALLED_MARKER: &str = ".arcana-staged";
/// Where the copy is assembled before it is renamed to `node_modules`.
const ASSEMBLY_DIR: &str = ".arcana-node-modules-stage";
const LOCKFILES: [&str; 2] = ["npm-shrinkwrap.json", "package-lock.json"];

/// What an npm staging call would fetch, computed without starting a process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NpmStagingPlan {
    pub project_dir: PathBuf,
    /// The lockfile `npm ci` reads: the shrinkwrap if there is one.
    pub lockfile_name: &'static str,
    /// Digest over `package.json` and every lockfile present.
    pub digest: String,
    /// `name version` for every package the lockfile fetches.
    pub packages: Vec<String>,
    /// How many of those declare an install script (which is not run).
    pub install_script_packages: usize,
    /// The exact bytes that were hashed. The scratch project is written from
    /// these, so what is installed is what the digest covers.
    package_json: Vec<u8>,
    lockfile: Vec<u8>,
}

impl NpmStagingPlan {
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
            "Arcana will download {} npm package(s) named by {} from registry.npmjs.org into its own cache and copy them into this project's node_modules. Install scripts are not run ({} package(s) declare one). Approval covers package.json and the lockfile(s), plan sha256 {}. Packages: {shown}{tail}.",
            self.packages.len(),
            self.lockfile_name,
            self.install_script_packages,
            self.digest
        )
    }
}

/// Plan an npm staging, or say why it is refused.
pub(crate) fn plan_npm(
    workspace_root: &Path,
    project_dir: &Path,
) -> Result<NpmStagingPlan, String> {
    let workspace_root = &strip_verbatim_prefix(workspace_root);
    let project_dir = &strip_verbatim_prefix(project_dir);
    refuse_reparse_components(workspace_root, project_dir)?;
    refuse_redirecting_config(workspace_root, project_dir)?;

    let manifest_path = project_dir.join("package.json");
    if !manifest_path.is_file() {
        return Err(
            "No package.json in the selected project, so there is nothing to stage.".into(),
        );
    }
    let package_json = read_bounded(&manifest_path, "package.json")?;
    let manifest: serde_json::Value = serde_json::from_slice(&package_json)
        .map_err(|_| "package.json does not parse as JSON.".to_string())?;
    if manifest.get("workspaces").is_some() {
        return Err("package.json declares npm workspaces, which staging does not support. Stage the dependencies yourself.".into());
    }
    refuse_non_registry_specs(&manifest)?;

    // npm reads the shrinkwrap first. Every lockfile present is bound and
    // checked: the other is one deletion away from being the one npm reads.
    let mut locks = Vec::new();
    for name in LOCKFILES {
        let path = project_dir.join(name);
        if std::fs::symlink_metadata(&path).is_ok() {
            let bytes = read_bounded(&path, name)?;
            locks.push((name, path, bytes));
        }
    }
    let Some((lockfile_name, _, lockfile)) = locks.first().cloned() else {
        return Err("No package-lock.json or npm-shrinkwrap.json found. Dependency staging only installs what a lockfile already pins; Arcana does not create one.".into());
    };
    let mut packages = Vec::new();
    let mut install_script_packages = 0;
    for (name, _, bytes) in &locks {
        let (listed, scripts) = locked_packages(name, bytes)?;
        if *name == lockfile_name {
            packages = listed;
            install_script_packages = scripts;
        }
    }
    packages.sort();
    packages.dedup();

    let mut hasher = Sha256::new();
    hasher.update(b"arcana-npm-staging-v1\n");
    hash_file(&mut hasher, workspace_root, &manifest_path, &package_json);
    for (_, path, bytes) in &locks {
        hash_file(&mut hasher, workspace_root, path, bytes);
    }
    Ok(NpmStagingPlan {
        project_dir: project_dir.to_path_buf(),
        lockfile_name,
        digest: hex::encode(hasher.finalize()),
        packages,
        install_script_packages,
        package_json,
        lockfile,
    })
}

/// Every dependency spec in `package.json` (and every `overrides` value) must be
/// a plain registry range. A URL, git, file, alias or path spec is a second route
/// to a fetch from somewhere other than the registry.
fn refuse_non_registry_specs(manifest: &serde_json::Value) -> Result<(), String> {
    fn plain_spec(value: &str) -> bool {
        // A leading dot is npm's spelling of a directory spec (`.`, `..`).
        value.len() <= 200 && !value.starts_with('.') && !value.contains([':', '/', '\\', '#', '@'])
    }
    fn check(value: &serde_json::Value, field: &str) -> Result<(), String> {
        match value {
            serde_json::Value::String(spec) if plain_spec(spec) => Ok(()),
            serde_json::Value::String(_) => Err(format!(
                "package.json {field} names a source that is not a plain registry version range, which staging refuses."
            )),
            serde_json::Value::Object(map) => map
                .values()
                .try_for_each(|inner| check(inner, field)),
            _ => Err(format!(
                "package.json {field} has a value staging does not understand."
            )),
        }
    }
    for field in [
        "dependencies",
        "devDependencies",
        "optionalDependencies",
        "peerDependencies",
        "overrides",
    ] {
        if let Some(value) = manifest.get(field) {
            if field == "overrides" {
                check(value, field)?;
            } else if let Some(map) = value.as_object() {
                map.values().try_for_each(|spec| check(spec, field))?;
            } else {
                return Err(format!("package.json {field} is not an object."));
            }
        }
    }
    Ok(())
}

/// An enclosing `workspaces` manifest changes which lockfile and tree npm uses,
/// and a `.npmrc` says the project expects another registry or credentials. The
/// workspace is agent-writable, so neither may sit on the path.
fn refuse_redirecting_config(workspace_root: &Path, project_dir: &Path) -> Result<(), String> {
    let mut directory = Some(project_dir);
    while let Some(current) = directory {
        if std::fs::symlink_metadata(current.join(".npmrc")).is_ok() {
            return Err(format!(
                "Refusing to stage: {} can redirect npm to another registry or credential. Remove it, or stage the dependencies yourself.",
                super::dependency_staging::display_in(workspace_root, &current.join(".npmrc"))
            ));
        }
        if current != project_dir {
            let manifest = current.join("package.json");
            if manifest.is_file() {
                let bytes = read_bounded(&manifest, "package.json")?;
                let enclosing = serde_json::from_slice::<serde_json::Value>(&bytes)
                    .ok()
                    .is_some_and(|value| value.get("workspaces").is_some());
                if enclosing {
                    return Err("The project sits inside an npm workspace, which staging does not support. Stage the dependencies yourself.".into());
                }
            }
        }
        if current == workspace_root {
            break;
        }
        directory = current.parent();
    }
    Ok(())
}

/// Accept only registry.npmjs.org tarballs with a sha512 integrity value and
/// plain names and versions. Returns `name version` for each package that is
/// fetched and how many of them declare an install script.
fn locked_packages(label: &str, bytes: &[u8]) -> Result<(Vec<String>, usize), String> {
    let document: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| format!("{label} does not parse as JSON."))?;
    match document["lockfileVersion"].as_u64() {
        Some(2 | 3) => {}
        other => {
            return Err(format!(
                "{label} has lockfileVersion {}; staging needs 2 or 3 (npm 7 or later).",
                other.map_or_else(|| "none".to_string(), |version| version.to_string())
            ))
        }
    }
    let Some(entries) = document["packages"].as_object() else {
        return Err(format!("{label} has no packages table."));
    };
    if entries.len() > MAX_PACKAGES {
        return Err(format!(
            "{label} lists {} packages, over the {MAX_PACKAGES} staging limit.",
            entries.len()
        ));
    }
    let mut fetched = Vec::new();
    let mut install_scripts = 0;
    for (key, entry) in entries {
        if key.is_empty() {
            continue;
        }
        let Some(name) = package_name(key) else {
            return Err(format!(
                "{label} has a package path staging does not accept."
            ));
        };
        if !matches!(
            entry["link"],
            serde_json::Value::Null | serde_json::Value::Bool(false)
        ) {
            return Err(format!(
                "{label} links package {name} to another directory, which staging refuses."
            ));
        }
        // An alias installs another package's tarball under this name.
        if entry
            .get("name")
            .is_some_and(|alias| alias.as_str() != Some(name.as_str()))
        {
            return Err(format!(
                "{label} installs package {name} under a different name, which staging refuses."
            ));
        }
        let version = entry["version"].as_str().unwrap_or_default();
        if !plain(version, |c| c.is_ascii_alphanumeric() || ".+-".contains(c)) {
            return Err(format!(
                "{label} has package {name} with a missing or unexpected version."
            ));
        }
        // A bundled package ships inside its parent's tarball, so it has no
        // tarball of its own and only exists under a parent. One that does name
        // a `resolved` URL gets the full check below: npm may fetch it.
        let bundled = entry["inBundle"].as_bool() == Some(true);
        if bundled && key.matches("node_modules/").count() < 2 {
            return Err(format!(
                "{label} marks top-level package {name} as bundled, which staging refuses."
            ));
        }
        if bundled && entry.get("resolved").is_none() {
            // npm takes it from the parent's tarball, which is pinned. If the
            // parent does not actually bundle it, npm fetches `name@version`
            // from the registry with no integrity check, so the parent's lock
            // entry must say it does.
            let parent = key.rfind("/node_modules/").map(|at| &key[..at]);
            let declared = parent
                .and_then(|parent_key| entries.get(parent_key))
                .is_some_and(|parent_entry| {
                    ["bundleDependencies", "bundledDependencies"]
                        .iter()
                        .filter_map(|field| parent_entry[*field].as_array())
                        .flatten()
                        .any(|bundled_name| bundled_name.as_str() == Some(name.as_str()))
                });
            if !declared {
                return Err(format!(
                    "{label} package {name} is marked bundled but its parent does not bundle it, so npm would fetch it without an integrity check."
                ));
            }
            fetched.push(format!("{name} {version} (bundled)"));
            continue;
        }
        let resolved = entry["resolved"].as_str().unwrap_or_default();
        if resolved != registry_tarball(&name, version) {
            return Err(format!(
                "{label} package {name} {version} does not name its {REGISTRY} tarball. Staging only fetches registry.npmjs.org packages."
            ));
        }
        let integrity = entry["integrity"].as_str().unwrap_or_default();
        let sha512 = integrity.strip_prefix("sha512-").is_some_and(|digest| {
            digest.len() >= 86
                && digest.len() <= 100
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"+/=".contains(&byte))
        });
        if !sha512 {
            return Err(format!(
                "{label} package {name} {version} has no sha512 integrity value."
            ));
        }
        if entry["hasInstallScript"].as_bool() == Some(true) {
            install_scripts += 1;
        }
        fetched.push(format!("{name} {version}"));
    }
    Ok((fetched, install_scripts))
}

/// The one URL the registry serves a package version's tarball from.
fn registry_tarball(name: &str, version: &str) -> String {
    let unscoped = name.rsplit('/').next().unwrap_or(name);
    format!("{REGISTRY}{name}/-/{unscoped}-{version}.tgz")
}

/// The package name a `node_modules/...` lockfile key installs, or `None` if
/// the key could place files anywhere but under a `node_modules` directory.
fn package_name(key: &str) -> Option<String> {
    let segments = key.split('/').collect::<Vec<_>>();
    // Windows drops trailing dots and spaces and treats device names as devices,
    // so a segment that is one of those would not land where its name says.
    let allowed = |segment: &str| {
        plain(segment, |c| {
            c.is_ascii_alphanumeric() || "._~@-".contains(c)
        }) && !segment.starts_with('.')
            && !segment.ends_with(['.', ' '])
            && !is_windows_device_name(segment)
    };
    if !segments.iter().all(|segment| allowed(segment)) || segments.first() != Some(&NODE_MODULES) {
        return None;
    }
    let last_modules = segments
        .iter()
        .rposition(|segment| *segment == NODE_MODULES)?;
    let name = &segments[last_modules + 1..];
    let well_formed = match name {
        [single] => !single.starts_with('@'),
        [scope, package] => scope.starts_with('@') && !package.starts_with('@'),
        _ => false,
    };
    well_formed.then(|| name.join("/"))
}

/// `CON`, `NUL`, `COM1` and the rest, with or without an extension.
fn is_windows_device_name(segment: &str) -> bool {
    let stem = segment
        .split('.')
        .next()
        .unwrap_or(segment)
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || (stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && stem.as_bytes()[3].is_ascii_digit())
}

/// The host node and the npm it ships with. Running `node npm-cli.js` directly
/// avoids the `.cmd` shim, so a timeout kills the process that does the work.
#[derive(Debug, Clone)]
pub(crate) struct NpmToolchain {
    pub node: PathBuf,
    pub npm_cli: PathBuf,
}

/// Locate node and npm's entry script in the directory that holds `npm.cmd`.
pub(crate) fn toolchain_in(directory: &Path) -> Result<NpmToolchain, String> {
    let node = directory.join("node.exe");
    let npm_cli = directory
        .join(NODE_MODULES)
        .join("npm")
        .join("bin")
        .join("npm-cli.js");
    if node.is_file() && npm_cli.is_file() {
        Ok(NpmToolchain { node, npm_cli })
    } else {
        Err("npm staging needs node.exe and npm's npm-cli.js beside npm.cmd in the Node installation.".into())
    }
}

/// Result of a completed staging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NpmStagingReport {
    pub package_count: usize,
    /// The packages were already in Arcana's cache, so nothing was downloaded.
    pub already_staged: bool,
}

/// Fetch the plan's packages into the host cache and install them into the
/// project's `node_modules`.
pub(crate) async fn stage_npm(
    plan: &NpmStagingPlan,
    workspace_root: &Path,
    toolchain: &NpmToolchain,
    timeout: Duration,
) -> Result<NpmStagingReport, String> {
    let final_dir = cache_dir("npm", &plan.digest).map_err(|error| error.to_string())?;
    let already_staged = final_dir.join(MARKER_FILE).is_file();
    if !already_staged {
        // The files may have moved since the user approved this digest.
        ensure_unchanged(workspace_root, plan, "before the fetch")?;
        fetch_into_cache(plan, toolchain, &final_dir, timeout).await?;
        ensure_unchanged(workspace_root, plan, "while the fetch ran")?;
    }
    let source = final_dir.join(NODE_MODULES);
    let project_dir = plan.project_dir.clone();
    let workspace = workspace_root.to_path_buf();
    let digest = plan.digest.clone();
    // Always copied afresh: the sandboxed build can edit its node_modules, so a
    // matching marker says nothing about what is in the tree now.
    tokio::task::spawn_blocking(move || {
        install_into_project(&source, &project_dir, &workspace, &digest)
    })
    .await
    .map_err(|error| format!("Installing node_modules did not finish: {error}"))??;
    Ok(NpmStagingReport {
        package_count: plan.packages.len(),
        already_staged,
    })
}

fn ensure_unchanged(
    workspace_root: &Path,
    plan: &NpmStagingPlan,
    when: &str,
) -> Result<(), String> {
    let current = plan_npm(workspace_root, &plan.project_dir)?;
    if current.digest == plan.digest {
        Ok(())
    } else {
        Err(format!(
            "package.json or a lockfile changed {when}; the stage was discarded. Read the plan again and have the user approve the current files."
        ))
    }
}

/// Run `npm ci --ignore-scripts` in a scratch project and publish the finished
/// tree under the plan's digest. A failed or raced run leaves nothing a build
/// could read.
async fn fetch_into_cache(
    plan: &NpmStagingPlan,
    toolchain: &NpmToolchain,
    final_dir: &Path,
    timeout: Duration,
) -> Result<(), String> {
    let parent = final_dir
        .parent()
        .ok_or_else(|| "The dependency cache path has no parent.".to_string())?;
    let prepare = |error: io::Error| format!("Could not prepare the dependency cache: {error}");
    std::fs::create_dir_all(parent).map_err(prepare)?;
    sweep_stale_scratch(parent);
    let scratch = tempfile::Builder::new()
        .prefix("stage-")
        .tempdir_in(parent)
        .map_err(prepare)?;
    let project = scratch.path().join("project");
    let home = scratch.path().join("home");
    let temp = scratch.path().join("tmp");
    let npm_cache = scratch.path().join("npm-cache");
    let logs = scratch.path().join("logs");
    for path in [&project, &home, &temp, &npm_cache, &logs] {
        std::fs::create_dir_all(path).map_err(prepare)?;
    }
    // npm refuses to load one file as both the user and the global config.
    let user_config = scratch.path().join("user.npmrc");
    let global_config = scratch.path().join("global.npmrc");
    for config in [&user_config, &global_config] {
        std::fs::write(config, b"").map_err(prepare)?;
    }
    std::fs::write(project.join("package.json"), &plan.package_json).map_err(prepare)?;
    std::fs::write(project.join(plan.lockfile_name), &plan.lockfile).map_err(prepare)?;

    if plan.packages.is_empty() {
        // npm ci creates no node_modules for a lockfile that fetches nothing.
        std::fs::create_dir(project.join(NODE_MODULES)).map_err(prepare)?;
    } else {
        run_npm_ci(
            toolchain,
            &project,
            &home,
            &temp,
            &npm_cache,
            &logs,
            (&user_config, &global_config),
            timeout,
        )
        .await?;
    }

    let installed = project.join(NODE_MODULES);
    if !installed.is_dir() {
        return Err("npm ci finished without creating node_modules.".into());
    }
    let (entries, bytes) = measure_tree(&installed)?;
    tracing::info!(
        target: "arcana::project_command",
        entries,
        bytes,
        "npm dependency staging produced a node_modules tree"
    );
    std::fs::rename(&installed, scratch.path().join(NODE_MODULES)).map_err(prepare)?;
    for leftover in [&project, &home, &temp, &npm_cache, &logs] {
        std::fs::remove_dir_all(leftover).ok();
    }
    for config in [&user_config, &global_config] {
        std::fs::remove_file(config).ok();
    }
    let marker = serde_json::json!({
        "plan_sha256": plan.digest,
        "package_count": plan.packages.len(),
        "lockfile": plan.lockfile_name,
    });
    std::fs::write(scratch.path().join(MARKER_FILE), marker.to_string())
        .map_err(|error| format!("Could not record the finished stage: {error}"))?;

    let kept = scratch.keep();
    match std::fs::rename(&kept, final_dir) {
        Ok(()) => Ok(()),
        Err(_) if final_dir.join(MARKER_FILE).is_file() => {
            // Another Arcana process finished the same plan first.
            std::fs::remove_dir_all(&kept).ok();
            Ok(())
        }
        Err(error) => {
            std::fs::remove_dir_all(&kept).ok();
            Err(format!(
                "Could not publish the staged dependencies: {error}"
            ))
        }
    }
}

/// Remove scratch directories an earlier run left behind (a crash or a kill).
/// Only `stage-*` directories untouched for an hour are removed, and links are
/// never followed.
fn sweep_stale_scratch(parent: &Path) {
    let cutoff = Duration::from_secs(60 * 60);
    let Ok(listing) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in listing.flatten() {
        let is_scratch = entry.file_name().to_string_lossy().starts_with("stage-");
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let stale = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > cutoff);
        if is_scratch && metadata.is_dir() && !is_link(&metadata) && stale {
            std::fs::remove_dir_all(entry.path()).ok();
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_npm_ci(
    toolchain: &NpmToolchain,
    project: &Path,
    home: &Path,
    temp: &Path,
    npm_cache: &Path,
    logs: &Path,
    (user_config, global_config): (&Path, &Path),
    timeout: Duration,
) -> Result<(), String> {
    let system_root = std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("windir"))
        .unwrap_or_default();
    let node_dir = toolchain
        .node
        .parent()
        .ok_or_else(|| "The node.exe path has no directory.".to_string())?;
    let mut path = node_dir.as_os_str().to_os_string();
    path.push(";");
    path.push(Path::new(&system_root).join("System32"));
    path.push(";");
    path.push(&system_root);

    let mut command = tokio::process::Command::new(&toolchain.node);
    command
        .arg(&toolchain.npm_cli)
        .args([
            "ci",
            "--ignore-scripts",
            "--no-audit",
            "--no-fund",
            "--no-update-notifier",
            "--registry",
            REGISTRY,
            "--allow-git=none",
            "--allow-remote=none",
            "--allow-file=none",
            "--allow-directory=none",
        ])
        .arg("--userconfig")
        .arg(user_config)
        .arg("--globalconfig")
        .arg(global_config)
        .arg("--cache")
        .arg(npm_cache)
        .arg("--logs-dir")
        .arg(logs)
        .current_dir(project)
        .env_clear()
        .env("PATH", path)
        .env("SystemRoot", &system_root)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home.join("appdata"))
        .env("LOCALAPPDATA", home.join("local"))
        .env("TEMP", temp)
        .env("TMP", temp)
        .env("npm_config_ignore_scripts", "true")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
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
        "NODE_EXTRA_CA_CERTS",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);

    // Not `output()`, which would pipe stdout too and hold all of it in memory.
    let child = command
        .spawn()
        .map_err(|error| format!("Could not start npm ci: {error}"))?;
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| format!("npm ci did not finish within {}s.", timeout.as_secs()))?
        .map_err(|error| format!("npm ci did not finish: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    // npm's message can quote file contents and host paths, and it would go
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
        "npm ci failed during dependency staging"
    );
    Err(format!(
        "npm ci failed (exit {}). Check host network access, that package.json and the lockfile agree, and that every package exists on registry.npmjs.org; details are in the host log.",
        output.status.code().unwrap_or(-1)
    ))
}

fn is_link(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

/// Count a tree's entries and bytes, refusing links and anything over the
/// bounds. Run once, on what npm produced; the copy relies on that bound
/// and refuses links itself.
fn measure_tree(root: &Path) -> Result<(usize, u64), String> {
    let mut pending = vec![root.to_path_buf()];
    let (mut entries, mut bytes) = (0usize, 0u64);
    while let Some(directory) = pending.pop() {
        let listing = std::fs::read_dir(&directory)
            .map_err(|error| format!("Could not read the staged tree: {error}"))?;
        for entry in listing {
            let entry =
                entry.map_err(|error| format!("Could not read the staged tree: {error}"))?;
            let metadata = entry
                .metadata()
                .map_err(|error| format!("Could not read the staged tree: {error}"))?;
            if is_link(&metadata) {
                return Err("The staged packages contain a link, which staging refuses.".into());
            }
            entries += 1;
            bytes += metadata.len();
            if entries > MAX_TREE_ENTRIES || bytes > MAX_TREE_BYTES {
                return Err("The staged packages are larger than the staging limit.".into());
            }
            if metadata.is_dir() {
                pending.push(entry.path());
            }
        }
    }
    Ok((entries, bytes))
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    let copy_error = |error: io::Error| format!("Could not copy the staged packages: {error}");
    std::fs::create_dir(destination).map_err(copy_error)?;
    let mut pending = vec![(source.to_path_buf(), destination.to_path_buf())];
    while let Some((from, to)) = pending.pop() {
        for entry in std::fs::read_dir(&from).map_err(copy_error)? {
            let entry = entry.map_err(copy_error)?;
            let metadata = entry.metadata().map_err(copy_error)?;
            if is_link(&metadata) {
                return Err("The staged packages contain a link, which staging refuses.".into());
            }
            let target = to.join(entry.file_name());
            if metadata.is_dir() {
                std::fs::create_dir(&target).map_err(copy_error)?;
                pending.push((entry.path(), target));
            } else {
                std::fs::copy(entry.path(), &target).map_err(copy_error)?;
            }
        }
    }
    Ok(())
}

/// Copy the cached tree into `<project>/node_modules`, replacing only a tree
/// Arcana installed earlier. A tree already marked with this digest is replaced
/// too: the sandboxed build may have changed it since.
fn install_into_project(
    source: &Path,
    project_dir: &Path,
    workspace_root: &Path,
    digest: &str,
) -> Result<(), String> {
    refuse_reparse_components(
        &strip_verbatim_prefix(workspace_root),
        &strip_verbatim_prefix(project_dir),
    )?;
    // Verbatim paths, so deeply nested packages do not hit the 260 character limit.
    let source = source
        .canonicalize()
        .map_err(|error| format!("The staged packages are missing: {error}"))?;
    let project = project_dir
        .canonicalize()
        .map_err(|error| format!("Cannot resolve the project: {error}"))?;
    let destination = project.join(NODE_MODULES);
    let assembly = project.join(ASSEMBLY_DIR);

    let mut replace_existing = false;
    match std::fs::symlink_metadata(&destination) {
        Ok(metadata) if is_link(&metadata) || !metadata.is_dir() => {
            return Err(
                "node_modules in this project is a link or a file, which staging will not replace."
                    .into(),
            );
        }
        Ok(_) => {
            // Any digest: an earlier plan's tree is Arcana's to replace.
            if std::fs::read_to_string(destination.join(INSTALLED_MARKER)).is_err() {
                return Err("node_modules already exists in this project and was not installed by Arcana's staging. Remove it, or stage the dependencies yourself.".into());
            }
            replace_existing = true;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("Could not inspect node_modules: {error}")),
    }
    match std::fs::symlink_metadata(&assembly) {
        Ok(metadata) if is_link(&metadata) => {
            return Err(format!(
                "{ASSEMBLY_DIR} in this project is a link, which staging refuses."
            ));
        }
        Ok(_) => std::fs::remove_dir_all(&assembly)
            .map_err(|error| format!("Could not clear an earlier unfinished install: {error}"))?,
        Err(_) => {}
    }

    let result = (|| {
        copy_tree(&source, &assembly)?;
        std::fs::write(assembly.join(INSTALLED_MARKER), digest)
            .map_err(|error| format!("Could not record the installed tree: {error}"))?;
        // The copy took time, and the directories are agent-writable: look
        // again, immediately before each step that deletes or moves.
        refuse_link(&assembly)?;
        if replace_existing {
            refuse_link(&destination)?;
            std::fs::remove_dir_all(&destination)
                .map_err(|error| format!("Could not replace the earlier node_modules: {error}"))?;
        }
        refuse_link(&assembly)?;
        std::fs::rename(&assembly, &destination)
            .map_err(|error| format!("Could not move node_modules into place: {error}"))
    })();
    if result.is_err() && !is_link_at(&assembly) {
        std::fs::remove_dir_all(&assembly).ok();
    }
    result
}

fn is_link_at(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| is_link(&metadata))
}

fn refuse_link(path: &Path) -> Result<(), String> {
    if is_link_at(path) {
        Err("node_modules or its assembly directory was turned into a link while staging ran; the install was stopped.".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTEGRITY: &str = "sha512-41Cifkg6e8TylSpdtTpeLVMqvSBEVzTttHvERD741+pnZ8ANv0004MRL43QKPDlK9cGvNp6NZWZUBlbGXYxxng==";

    fn package_entry(name: &str, resolved: &str, integrity: &str) -> String {
        format!(
            r#""node_modules/{name}": {{"version": "1.0.0", "resolved": "{resolved}", "integrity": "{integrity}"}}"#
        )
    }

    fn registry_entry(name: &str) -> String {
        let installed = name.rsplit("node_modules/").next().unwrap();
        package_entry(name, &registry_tarball(installed, "1.0.0"), INTEGRITY)
    }

    #[test]
    fn the_registry_tarball_url_is_the_registrys_own_layout() {
        assert_eq!(
            registry_tarball("is-number", "7.0.0"),
            "https://registry.npmjs.org/is-number/-/is-number-7.0.0.tgz"
        );
        assert_eq!(
            registry_tarball("@types/node", "20.1.0"),
            "https://registry.npmjs.org/@types/node/-/node-20.1.0.tgz"
        );
    }

    fn lock(version: u64, entries: &[String]) -> String {
        format!(
            r#"{{"name": "demo", "version": "1.0.0", "lockfileVersion": {version}, "requires": true, "packages": {{"": {{"name": "demo", "version": "1.0.0"}}, {}}}}}"#,
            entries.join(", ")
        )
    }

    fn good_lock() -> String {
        lock(3, &[registry_entry("is-number")])
    }

    const PACKAGE_JSON: &str =
        r#"{"name": "demo", "version": "1.0.0", "dependencies": {"is-number": "1.0.0"}}"#;

    fn project(lock_text: &str) -> (tempfile::TempDir, PathBuf) {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        std::fs::write(root.join("package.json"), PACKAGE_JSON).unwrap();
        std::fs::write(root.join("package-lock.json"), lock_text).unwrap();
        (workspace, root)
    }

    #[test]
    fn registry_packages_are_listed_with_their_install_script_count() {
        let scripted = package_entry(
            "esbuild",
            "https://registry.npmjs.org/esbuild/-/esbuild-1.0.0.tgz",
            INTEGRITY,
        )
        .replace(
            r#""version": "1.0.0""#,
            r#""version": "1.0.0", "hasInstallScript": true"#,
        );
        let (_workspace, root) = project(&lock(3, &[registry_entry("is-number"), scripted]));
        let plan = plan_npm(&root, &root).unwrap();
        assert_eq!(plan.packages, ["esbuild 1.0.0", "is-number 1.0.0"]);
        assert_eq!(plan.install_script_packages, 1);
        assert_eq!(plan.lockfile_name, "package-lock.json");
        assert!(plan.summary().contains(&plan.digest));
        assert!(plan.summary().contains("is-number 1.0.0"));
    }

    #[test]
    fn a_lockfile_naming_anything_but_registry_tarballs_is_refused() {
        for (resolved, integrity) in [
            ("git+https://example.com/x.git#abc", INTEGRITY),
            ("https://example.com/x/-/x-1.0.0.tgz", INTEGRITY),
            (
                "https://registry.npmjs.org.evil.test/x-1.0.0.tgz",
                INTEGRITY,
            ),
            (
                "https://registry.npmjs.org@evil.test/x-1.0.0.tgz",
                INTEGRITY,
            ),
            ("file:../x", INTEGRITY),
            // A real registry URL, but not this package's tarball.
            ("https://registry.npmjs.org/y/-/y-1.0.0.tgz", INTEGRITY),
            ("https://registry.npmjs.org/x/-/x-9.9.9.tgz", INTEGRITY),
            ("https://registry.npmjs.org/x/-/x-1.0.0.tgz?a=b", INTEGRITY),
            ("", INTEGRITY),
            ("https://registry.npmjs.org/x/-/x-1.0.0.tgz", ""),
            (
                "https://registry.npmjs.org/x/-/x-1.0.0.tgz",
                "sha1-AAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            ),
        ] {
            let (_workspace, root) = project(&lock(3, &[package_entry("x", resolved, integrity)]));
            assert!(
                plan_npm(&root, &root).is_err(),
                "{resolved:?} / {integrity:?} must be refused"
            );
        }
    }

    #[test]
    fn links_old_lockfile_versions_and_odd_package_paths_are_refused() {
        let cases = [
            (
                3,
                r#""node_modules/x": {"resolved": "../x", "link": true}"#.to_string(),
            ),
            (1, registry_entry("x")),
            (
                3,
                registry_entry("x").replace("node_modules/x", "node_modules/../x"),
            ),
            (3, registry_entry("x").replace("node_modules/x", "x")),
            (
                3,
                registry_entry("x").replace("node_modules/x", "node_modules/@s"),
            ),
            (
                3,
                registry_entry("x").replace("node_modules/x", "node_modules/x\\\\y"),
            ),
            (
                3,
                registry_entry("x")
                    .replace("\"1.0.0\", \"resolved", "\"1.0.0 && calc\", \"resolved"),
            ),
            (
                3,
                registry_entry("x").replace("\"version\"", "\"link\": \"yes\", \"version\""),
            ),
            // An alias installs another package's tarball under this name.
            (
                3,
                registry_entry("x").replace("\"version\"", "\"name\": \"y\", \"version\""),
            ),
            (
                3,
                registry_entry("x").replace("node_modules/x", "node_modules/x."),
            ),
            (
                3,
                registry_entry("x").replace("node_modules/x", "node_modules/.x"),
            ),
            (
                3,
                registry_entry("x").replace("node_modules/x", "node_modules/con"),
            ),
            (
                3,
                registry_entry("x").replace("node_modules/x", "node_modules/NUL.txt"),
            ),
            (
                3,
                registry_entry("x").replace("node_modules/x", "node_modules/com1"),
            ),
        ];
        for (version, entry) in cases {
            let (_workspace, root) = project(&lock(version, &[entry.clone()]));
            assert!(plan_npm(&root, &root).is_err(), "{entry} must be refused");
        }
    }

    #[test]
    fn scoped_and_nested_packages_are_accepted() {
        let (_workspace, root) = project(&lock(
            3,
            &[
                registry_entry("@scope/pkg"),
                registry_entry("a/node_modules/b"),
            ],
        ));
        let plan = plan_npm(&root, &root).unwrap();
        assert_eq!(plan.packages.len(), 2, "{:?}", plan.packages);
    }

    #[test]
    fn a_nested_bundled_package_without_a_tarball_is_allowed_and_listed() {
        let bundled = r#""node_modules/a/node_modules/b": {"version": "1.0.0", "inBundle": true}"#
            .to_string();
        let parent = registry_entry("a").replace(
            r#""version": "1.0.0""#,
            r#""version": "1.0.0", "bundleDependencies": ["b"]"#,
        );
        let (_workspace, root) = project(&lock(3, &[parent, bundled.clone()]));
        assert_eq!(
            plan_npm(&root, &root).unwrap().packages,
            ["a 1.0.0", "b 1.0.0 (bundled)"]
        );

        // Without the parent saying it bundles `b`, npm would fetch b@1.0.0
        // from the registry with no integrity value.
        let (_workspace, root) = project(&lock(3, &[registry_entry("a"), bundled]));
        assert!(plan_npm(&root, &root)
            .unwrap_err()
            .contains("its parent does not bundle it"));
    }

    #[test]
    fn a_bundled_flag_does_not_skip_the_registry_policy() {
        let top_level = r#""node_modules/b": {"version": "1.0.0", "inBundle": true}"#.to_string();
        let evil_url = r#""node_modules/a/node_modules/b": {"version": "1.0.0", "inBundle": true, "resolved": "https://evil.test/b.tgz", "integrity": "x"}"#.to_string();
        let evil_git = r#""node_modules/a/node_modules/b": {"version": "1.0.0", "inBundle": true, "resolved": "git+https://example.com/b.git"}"#.to_string();
        for bundled in [top_level, evil_url, evil_git] {
            let (_workspace, root) = project(&lock(3, &[registry_entry("a"), bundled.clone()]));
            assert!(plan_npm(&root, &root).is_err(), "{bundled} must be refused");
        }
    }

    #[test]
    fn dependency_specs_must_be_plain_registry_ranges() {
        let (_workspace, root) = project(&good_lock());
        for spec in [
            "git+https://example.com/x.git",
            "github:user/repo",
            "file:../x",
            "https://example.com/x.tgz",
            "npm:other@1.0.0",
            "../x",
            "user/repo",
            "1.0.0#abc",
            ".",
            "..",
            "./x",
        ] {
            for field in [
                "dependencies",
                "devDependencies",
                "optionalDependencies",
                "peerDependencies",
            ] {
                std::fs::write(
                    root.join("package.json"),
                    format!(r#"{{"name": "demo", "{field}": {{"x": "{spec}"}}}}"#),
                )
                .unwrap();
                assert!(plan_npm(&root, &root).is_err(), "{field}: {spec}");
            }
            std::fs::write(
                root.join("package.json"),
                format!(r#"{{"name": "demo", "overrides": {{"x": {{"y": "{spec}"}}}}}}"#),
            )
            .unwrap();
            assert!(plan_npm(&root, &root).is_err(), "overrides: {spec}");
        }
        std::fs::write(
            root.join("package.json"),
            r#"{"name": "demo", "dependencies": {"x": "^1.2.3 || ~2.0 || >=3 <4 || 1.x || *"}, "overrides": {"x": {".": "1.0.0", "y": "$x"}}}"#,
        )
        .unwrap();
        assert!(plan_npm(&root, &root).is_ok());
    }

    #[test]
    fn a_lockfile_that_fetches_nothing_is_a_valid_plan() {
        let (_workspace, root) = project(
            r#"{"name": "demo", "version": "1.0.0", "lockfileVersion": 3, "requires": true, "packages": {"": {"name": "demo", "version": "1.0.0"}}}"#,
        );
        assert!(plan_npm(&root, &root).unwrap().packages.is_empty());
    }

    #[test]
    fn a_missing_lockfile_or_manifest_is_refused() {
        let (_workspace, root) = project(&good_lock());
        std::fs::remove_file(root.join("package-lock.json")).unwrap();
        assert!(plan_npm(&root, &root)
            .unwrap_err()
            .contains("No package-lock.json"));
        std::fs::remove_file(root.join("package.json")).unwrap();
        assert!(plan_npm(&root, &root)
            .unwrap_err()
            .contains("No package.json"));
    }

    #[test]
    fn the_digest_covers_the_manifest_and_the_lockfile() {
        let (_workspace, root) = project(&good_lock());
        let bound = plan_npm(&root, &root).unwrap().digest;
        assert_eq!(plan_npm(&root, &root).unwrap().digest, bound);

        std::fs::write(
            root.join("package.json"),
            PACKAGE_JSON.replace("1.0.0\"}}", "1.0.1\"}}"),
        )
        .unwrap();
        let after_manifest = plan_npm(&root, &root).unwrap().digest;
        assert_ne!(after_manifest, bound);

        std::fs::write(
            root.join("package-lock.json"),
            lock(3, &[registry_entry("is-number"), registry_entry("extra")]),
        )
        .unwrap();
        assert_ne!(plan_npm(&root, &root).unwrap().digest, after_manifest);
    }

    #[test]
    fn the_shrinkwrap_wins_and_both_lockfiles_are_bound_and_checked() {
        let (_workspace, root) = project(&good_lock());
        let alone = plan_npm(&root, &root).unwrap().digest;

        std::fs::write(
            root.join("npm-shrinkwrap.json"),
            lock(3, &[registry_entry("shrunk")]),
        )
        .unwrap();
        let both = plan_npm(&root, &root).unwrap();
        assert_eq!(both.lockfile_name, "npm-shrinkwrap.json");
        assert_eq!(both.packages, ["shrunk 1.0.0"]);
        assert_ne!(both.digest, alone);

        // The lockfile npm ignores is still policy-checked: deleting the
        // shrinkwrap would make it the one npm reads.
        std::fs::write(
            root.join("package-lock.json"),
            lock(
                3,
                &[package_entry(
                    "evil",
                    "git+https://example.com/x.git",
                    INTEGRITY,
                )],
            ),
        )
        .unwrap();
        assert!(plan_npm(&root, &root).is_err());
    }

    #[test]
    fn an_npmrc_anywhere_on_the_path_is_refused() {
        let (_workspace, root) = project(&good_lock());
        let nested = root.join("app");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("package.json"), PACKAGE_JSON).unwrap();
        std::fs::write(nested.join("package-lock.json"), good_lock()).unwrap();
        assert!(plan_npm(&root, &nested).is_ok());

        std::fs::write(root.join(".npmrc"), "registry=https://evil.test/\n").unwrap();
        assert!(plan_npm(&root, &nested).unwrap_err().contains(".npmrc"));
        std::fs::remove_file(root.join(".npmrc")).unwrap();

        std::fs::write(nested.join(".npmrc"), "").unwrap();
        assert!(plan_npm(&root, &nested).unwrap_err().contains(".npmrc"));
    }

    #[test]
    fn npm_workspaces_here_or_above_are_refused() {
        let (_workspace, root) = project(&good_lock());
        let nested = root.join("app");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("package.json"), PACKAGE_JSON).unwrap();
        std::fs::write(nested.join("package-lock.json"), good_lock()).unwrap();

        std::fs::write(
            root.join("package.json"),
            r#"{"name": "root", "workspaces": ["app"]}"#,
        )
        .unwrap();
        assert!(plan_npm(&root, &nested)
            .unwrap_err()
            .contains("inside an npm workspace"));

        std::fs::write(root.join("package.json"), PACKAGE_JSON).unwrap();
        std::fs::write(
            nested.join("package.json"),
            r#"{"name": "app", "workspaces": ["packages/*"]}"#,
        )
        .unwrap();
        assert!(plan_npm(&root, &nested)
            .unwrap_err()
            .contains("npm workspaces"));
    }

    #[test]
    fn package_names_resolve_only_under_node_modules() {
        assert_eq!(package_name("node_modules/a").as_deref(), Some("a"));
        assert_eq!(package_name("node_modules/@s/a").as_deref(), Some("@s/a"));
        assert_eq!(
            package_name("node_modules/a/node_modules/@s/b").as_deref(),
            Some("@s/b")
        );
        for bad in [
            "a",
            "node_modules",
            "node_modules/",
            "node_modules/..",
            "node_modules/a/../b",
            "node_modules/a b",
            "node_modules/@s",
            "node_modules/a/b",
            "../node_modules/a",
            "node_modules/a:b",
            "node_modules/a.",
            "node_modules/.a",
            "node_modules/con",
            "node_modules/Aux.js",
            "node_modules/@s/lpt1",
        ] {
            assert_eq!(package_name(bad), None, "{bad}");
        }
    }

    fn cached_tree() -> (tempfile::TempDir, PathBuf) {
        let cache = tempfile::tempdir().unwrap();
        let source = cache.path().canonicalize().unwrap().join("node_modules");
        std::fs::create_dir_all(source.join("is-number")).unwrap();
        std::fs::write(
            source.join("is-number").join("index.js"),
            "module.exports = 1;",
        )
        .unwrap();
        std::fs::create_dir_all(source.join(".bin")).unwrap();
        std::fs::write(source.join(".bin").join("tool.cmd"), "@echo off").unwrap();
        (cache, source)
    }

    #[test]
    fn the_cached_tree_is_copied_in_and_always_replaces_an_earlier_one() {
        let (_workspace, root) = project(&good_lock());
        let (_cache, source) = cached_tree();
        install_into_project(&source, &root, &root, "d1").unwrap();
        assert!(root.join("node_modules/is-number/index.js").is_file());
        assert!(root.join("node_modules/.bin/tool.cmd").is_file());
        assert_eq!(
            std::fs::read_to_string(root.join("node_modules").join(INSTALLED_MARKER)).unwrap(),
            "d1"
        );
        assert!(!root.join(ASSEMBLY_DIR).exists());

        // The same plan restores what the build changed: the marker proves the
        // tree was Arcana's, not that it is still intact.
        std::fs::write(root.join("node_modules/is-number/extra.js"), "").unwrap();
        install_into_project(&source, &root, &root, "d1").unwrap();
        assert!(!root.join("node_modules/is-number/extra.js").exists());

        // So does a new plan.
        std::fs::write(root.join("node_modules/is-number/extra.js"), "").unwrap();
        install_into_project(&source, &root, &root, "d2").unwrap();
        assert!(!root.join("node_modules/is-number/extra.js").exists());
        assert_eq!(
            std::fs::read_to_string(root.join("node_modules").join(INSTALLED_MARKER)).unwrap(),
            "d2"
        );
    }

    #[test]
    fn a_node_modules_arcana_did_not_install_is_never_replaced() {
        let (_workspace, root) = project(&good_lock());
        let (_cache, source) = cached_tree();
        std::fs::create_dir_all(root.join("node_modules")).unwrap();
        std::fs::write(root.join("node_modules/mine.js"), "").unwrap();
        let error = install_into_project(&source, &root, &root, "d1").unwrap_err();
        assert!(error.contains("not installed by Arcana"), "{error}");
        assert!(root.join("node_modules/mine.js").is_file());
        assert!(!root.join(ASSEMBLY_DIR).exists());
    }

    #[test]
    fn a_node_modules_file_is_refused() {
        let (_workspace, root) = project(&good_lock());
        let (_cache, source) = cached_tree();
        std::fs::write(root.join("node_modules"), "not a directory").unwrap();
        assert!(install_into_project(&source, &root, &root, "d1").is_err());
    }

    #[cfg(windows)]
    fn junction(link: &Path, target: &Path) {
        let made = std::process::Command::new("cmd")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(made.status.success(), "junction fixture was not created");
    }

    #[cfg(windows)]
    #[test]
    fn a_junction_as_node_modules_or_in_the_cached_tree_is_refused() {
        let (_workspace, root) = project(&good_lock());
        let (_cache, source) = cached_tree();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("keep.txt"), "keep").unwrap();

        junction(&root.join("node_modules"), outside.path());
        assert!(install_into_project(&source, &root, &root, "d1").is_err());
        assert!(outside.path().join("keep.txt").is_file());
        std::fs::remove_dir(root.join("node_modules")).unwrap();

        junction(&source.join("evil"), outside.path());
        assert!(install_into_project(&source, &root, &root, "d1").is_err());
        assert!(!root.join("node_modules").exists());
        assert!(!root.join(ASSEMBLY_DIR).exists());
        assert!(outside.path().join("keep.txt").is_file());
    }

    #[cfg(windows)]
    #[test]
    fn replacing_a_node_modules_never_follows_a_junction_inside_it() {
        let (_workspace, root) = project(&good_lock());
        let (_cache, source) = cached_tree();
        install_into_project(&source, &root, &root, "d1").unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("keep.txt"), "keep").unwrap();
        junction(&root.join("node_modules/is-number/escape"), outside.path());

        install_into_project(&source, &root, &root, "d1").unwrap();
        assert!(outside.path().join("keep.txt").is_file());
        assert!(!root.join("node_modules/is-number/escape").exists());
    }

    #[cfg(windows)]
    #[test]
    fn a_junction_assembly_directory_is_refused_without_following_it() {
        let (_workspace, root) = project(&good_lock());
        let (_cache, source) = cached_tree();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("keep.txt"), "keep").unwrap();
        junction(&root.join(ASSEMBLY_DIR), outside.path());
        assert!(install_into_project(&source, &root, &root, "d1").is_err());
        assert!(outside.path().join("keep.txt").is_file());
    }
}
