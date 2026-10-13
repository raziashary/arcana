//! Host-owned recovery evidence. No model content is persisted in snapshots.

use crate::db::runs::OperationReconciliation;
use crate::errors::{ArcanaError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub(crate) const MAX_SNAPSHOT_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoverySnapshot {
    version: u32,
    task_id: i64,
    task_identity: String,
    assigned_worker: Option<String>,
    dependencies: Vec<(i64, String, String)>,
    session_id: String,
    workspace_identity: String,
    root_identity: (u64, u64),
    workspace_fingerprint: String,
    git: Option<GitSnapshot>,
    checkpoints: Vec<String>,
    expected_file: Option<ExpectedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GitSnapshot {
    head: Option<String>,
    branch_digest: Option<String>,
    index_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedFile {
    path: String,
    content_digest: String,
    other_inputs_digest: String,
    hooks_absent: bool,
    before: Option<FileState>,
    #[serde(default)]
    overwrite: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileState {
    digest: String,
    identity: (u64, u64),
    permissions: (u32, u32, u32),
}

fn file_state(root: &Path, relative: &Path) -> Result<Option<FileState>> {
    use std::io::Read;
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err(refused());
    }
    let path = super::builtin_tools::path_jail::validate_workspace_path(
        &root.join(relative),
        &root.to_string_lossy(),
    )
    .map_err(|_| refused())?;
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.len() > 16 * 1024 * 1024 {
        return Err(refused());
    }
    let file = std::fs::File::open(&path)?;
    let before = file.metadata()?;
    let canonical = path.canonicalize()?;
    let root = root.canonicalize()?;
    if !canonical.starts_with(&root)
        || !crate::platform::workspace_fingerprint::path_still_names_open_file(&file, &canonical)?
    {
        return Err(refused());
    }
    if !crate::platform::workspace_fingerprint::has_single_link(&file)? {
        return Err(refused());
    }
    #[cfg(unix)]
    let (identity, permissions) = {
        use std::os::unix::fs::MetadataExt;
        (
            (before.dev(), before.ino()),
            (before.mode(), before.uid(), before.gid()),
        )
    };
    #[cfg(windows)]
    let (identity, permissions) = {
        use std::os::windows::fs::MetadataExt;
        let (volume, index) = crate::platform::file_identity::windows_file_identity(&file)?;
        ((u64::from(volume), index), (before.file_attributes(), 0, 0))
    };
    #[cfg(not(any(unix, windows)))]
    let (identity, permissions) = return Err(refused());
    let mut bytes = Vec::new();
    (&file).take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    if bytes.len() > 16 * 1024 * 1024
        || before.len() != after.len()
        || before.modified()? != after.modified()?
        || !crate::platform::workspace_fingerprint::path_still_names_open_file(&file, &canonical)?
    {
        return Err(refused());
    }
    Ok(Some(FileState {
        digest: digest(&bytes),
        identity,
        permissions,
    }))
}

fn refused() -> ArcanaError {
    ArcanaError::Agent(
        "recovery snapshot evidence is absent, changed, or cannot be verified".into(),
    )
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn root_identity(root: &Path) -> Result<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = std::fs::metadata(root)?;
        Ok((metadata.dev(), metadata.ino()))
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let directory = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(0x0200_0000 | 0x0020_0000)
            .open(root)?;
        let (volume, index) = crate::platform::file_identity::windows_file_identity(&directory)?;
        Ok((u64::from(volume), index))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(refused())
    }
}

fn git_state(root: &Path) -> Result<Option<GitSnapshot>> {
    let repo = match git2::Repository::open(root) {
        Ok(repo) => repo,
        Err(error) if error.code() == git2::ErrorCode::NotFound => return Ok(None),
        Err(_) => return Err(refused()),
    };
    let head = match repo.head() {
        Ok(head) => head.target().map(|oid| oid.to_string()),
        Err(error)
            if matches!(
                error.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            None
        }
        Err(_) => return Err(refused()),
    };
    let branch_digest = repo
        .find_reference("HEAD")
        .map_err(|_| refused())?
        .symbolic_target()
        .map(|name| digest(name.as_bytes()));
    let index = repo.index().map_err(|_| refused())?;
    if index.len() > 50_000 {
        return Err(refused());
    }
    let mut hash = Sha256::new();
    for entry in index.iter() {
        hash.update((entry.path.len() as u64).to_le_bytes());
        hash.update(&entry.path);
        hash.update(entry.id.as_bytes());
        hash.update(entry.mode.to_le_bytes());
        hash.update(entry.flags.to_le_bytes());
    }
    Ok(Some(GitSnapshot {
        head,
        branch_digest,
        index_digest: format!("{:x}", hash.finalize()),
    }))
}

fn no_effect_hooks() -> Result<bool> {
    Ok(!crate::hooks::db::HooksDb::open()?.has_enabled_hooks()?)
}

impl RecoverySnapshot {
    pub(crate) fn owner(&self) -> Option<&str> {
        self.assigned_worker.as_deref()
    }

    pub(crate) fn task_id(&self) -> i64 {
        self.task_id
    }
    pub(crate) fn capture(
        task_id: i64,
        session_id: &str,
        root: &Path,
        expected: Option<(&str, &str, bool)>,
    ) -> Result<Self> {
        let tasks = crate::db::tasks::TasksDb::open()?;
        Self::capture_with_tasks(&tasks, task_id, session_id, root, expected)
    }

    fn capture_with_tasks(
        tasks: &crate::db::tasks::TasksDb,
        task_id: i64,
        session_id: &str,
        root: &Path,
        expected: Option<(&str, &str, bool)>,
    ) -> Result<Self> {
        let mut timer =
            crate::observability::timing::OperationTimer::start("recovery_snapshot_capture")
                .with_task_id(Some(task_id))
                .with_session_id(session_id);
        let task = tasks.get_by_id(task_id)?.ok_or_else(refused)?;
        let dependencies = task.blocked_by.as_deref().unwrap_or(&[]);
        if dependencies.len() > 128 || session_id.len() > 128 {
            return Err(refused());
        }
        let mut dependency_states = Vec::with_capacity(dependencies.len());
        for id in dependencies {
            let id = id.parse::<i64>().map_err(|_| refused())?;
            let dependency = tasks.get_by_id(id)?.ok_or_else(refused)?;
            dependency_states.push((
                id,
                dependency.task_identity,
                dependency.status.as_str().into(),
            ));
        }
        let root =
            crate::platform::workspace::resolve_workspace_dir(Some(&root.to_string_lossy()))?;
        let root = root.as_path();
        if crate::db::arcana_dir()?.canonicalize()?.starts_with(root) {
            return Err(refused());
        }
        let root_identity_before = root_identity(root)?;
        let git = git_state(root)?;
        let workspace_identity =
            digest(crate::platform::workspace::workspace_identity(root)?.as_bytes());
        let workspace_fingerprint =
            crate::platform::workspace_fingerprint::fingerprint_for_recovery(root)?;
        let checkpoints = crate::git::checkpoint::list(None, Some(session_id), Some(task_id))?;
        if checkpoints.len() > 512 {
            return Err(refused());
        }
        let expected_file = expected
            .map(|(path, content_digest, overwrite)| {
                Ok::<_, ArcanaError>(ExpectedFile {
                    path: path.into(),
                    content_digest: content_digest.into(),
                    other_inputs_digest:
                        crate::platform::workspace_fingerprint::fingerprint_without_file(
                            root,
                            Path::new(path),
                        )?,
                    hooks_absent: no_effect_hooks()?,
                    before: file_state(root, Path::new(path))?,
                    overwrite: Some(overwrite),
                })
            })
            .transpose()?;
        if git_state(root)? != git || root_identity(root)? != root_identity_before {
            return Err(refused());
        }
        let snapshot = Self {
            version: 2,
            task_id,
            task_identity: task.task_identity,
            assigned_worker: task.assigned_worker,
            dependencies: dependency_states,
            session_id: session_id.into(),
            workspace_identity,
            root_identity: root_identity_before,
            workspace_fingerprint,
            git,
            checkpoints: checkpoints
                .into_iter()
                .map(|checkpoint| checkpoint.id)
                .collect(),
            expected_file,
        };
        snapshot.encode()?;
        timer.succeed();
        Ok(snapshot)
    }

    pub(crate) fn file_expectation(content: &str) -> String {
        digest(content.as_bytes())
    }

    pub(crate) fn encode(&self) -> Result<String> {
        let encoded = serde_json::to_string(self).map_err(|_| refused())?;
        if !matches!(self.version, 1 | 2) || encoded.len() > MAX_SNAPSHOT_BYTES {
            return Err(refused());
        }
        Ok(encoded)
    }

    pub(crate) fn decode(raw: &str) -> Result<Self> {
        if raw.len() > MAX_SNAPSHOT_BYTES {
            return Err(refused());
        }
        let snapshot: Self = serde_json::from_str(raw).map_err(|_| refused())?;
        snapshot.encode()?;
        Ok(snapshot)
    }

    pub(crate) fn verify(
        &self,
        tasks: &crate::db::tasks::TasksDb,
        root: &Path,
        operation: &str,
        decision: OperationReconciliation,
    ) -> Result<()> {
        self.verify_state(tasks, root, operation, decision, None)
    }

    pub(crate) fn verify_retry_admission(
        &self,
        tasks: &crate::db::tasks::TasksDb,
        root: &Path,
        operation: &str,
        current: &RecoverySnapshot,
        worker: &str,
    ) -> Result<()> {
        let expected = self.expected_file.as_ref().ok_or_else(refused)?;
        let requested = current.expected_file.as_ref().ok_or_else(refused)?;
        if self.version != 2
            || expected.overwrite.is_none()
            || expected.path != requested.path
            || expected.content_digest != requested.content_digest
            || expected.overwrite != requested.overwrite
        {
            return Err(refused());
        }
        self.verify_state(
            tasks,
            root,
            operation,
            OperationReconciliation::RetrySafe,
            Some(worker),
        )
    }

    fn verify_state(
        &self,
        tasks: &crate::db::tasks::TasksDb,
        root: &Path,
        operation: &str,
        decision: OperationReconciliation,
        retry_worker: Option<&str>,
    ) -> Result<()> {
        let current = Self::capture_with_tasks(tasks, self.task_id, &self.session_id, root, None)?;
        let task = tasks.get_by_id(self.task_id)?.ok_or_else(refused)?;
        let lifecycle_valid = match retry_worker {
            None => {
                task.status == crate::db::tasks::TaskStatus::NeedsInfo
                    && task.claimed_by_pid.is_none()
            }
            Some(worker) => {
                task.status == crate::db::tasks::TaskStatus::InProgress
                    && task.assigned_worker.as_deref() == Some(worker)
                    && task.claimed_by_pid == Some(std::process::id() as i64)
                    && task.claim_host.as_deref() == Some(crate::db::runs::hostname().as_str())
            }
        };
        if !lifecycle_valid
            || current.task_identity != self.task_identity
            || current.assigned_worker != self.assigned_worker
            || current.dependencies != self.dependencies
            || current.workspace_identity != self.workspace_identity
            || current.root_identity != self.root_identity
            || current.git != self.git
            || self
                .checkpoints
                .iter()
                .any(|id| !current.checkpoints.contains(id))
        {
            return Err(refused());
        }
        let expected = self.expected_file.as_ref().ok_or_else(refused)?;
        if operation != "write_file" || !expected.hooks_absent || !no_effect_hooks()? {
            return Err(refused());
        }
        match decision {
            OperationReconciliation::RetrySafe
                if self.version == 2
                    && expected.overwrite.is_some()
                    && current.workspace_fingerprint == self.workspace_fingerprint
                    && file_state(root, Path::new(&expected.path))? == expected.before =>
            {
                Ok(())
            }
            OperationReconciliation::Reconciled => {
                let observed = file_state(root, Path::new(&expected.path))?.ok_or_else(refused)?;
                let others = crate::platform::workspace_fingerprint::fingerprint_without_file(
                    root,
                    Path::new(&expected.path),
                )?;
                if observed.digest == expected.content_digest
                    && others == expected.other_inputs_digest
                {
                    Ok(())
                } else {
                    Err(refused())
                }
            }
            _ => Err(refused()),
        }
    }
}
