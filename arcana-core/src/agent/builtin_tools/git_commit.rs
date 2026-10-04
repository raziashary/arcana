use async_trait::async_trait;
use git2::{Repository, Signature};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Component, Path};

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

pub struct GitCommitTool;

#[async_trait]
impl Tool for GitCommitTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Execute
    }

    fn name(&self) -> &str {
        "git_commit"
    }

    fn description(&self) -> &str {
        "Stage only named changed files and commit them after host checks. Refuses protected branches, pre-staged unrelated changes, ignored/generated paths, secret-bearing diffs, unsafe paths, and incomplete Conventional Commit messages."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "paths": {
                    "type": "array",
                    "minItems": 1,
                    "items": { "type": "string" },
                    "description": "Explicit repository-relative files changed by this task. Directories, globs, absolute paths, .env files, and ignored/generated paths are refused."
                },
                "message": {
                    "type": "string",
                    "description": "Conventional Commit subject followed by a blank line and a descriptive body."
                }
            },
            "required": ["paths", "message"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        if let Err(message) = ctx.require_current_task_claim() {
            return ToolResult::error(message);
        }
        let paths = match input["paths"].as_array() {
            Some(paths) => match paths.iter().map(Value::as_str).collect::<Option<Vec<_>>>() {
                Some(paths) => paths,
                None => return ToolResult::error("Every paths entry must be a string"),
            },
            None => {
                return ToolResult::error("Missing required field: paths (array of explicit files)")
            }
        };
        let message = match input["message"].as_str() {
            Some(message) => message,
            None => return ToolResult::error("Missing required field: message"),
        };
        let workspace = match resolve_commit_workspace(&ctx.working_dir) {
            Ok(workspace) => workspace,
            Err(error) => return ToolResult::error(error),
        };
        match commit_paths_with_checks(
            &workspace,
            &paths,
            message,
            || {},
            |_| {},
            || ctx.lock_current_task_claim(),
        )
        .await
        {
            Ok(oid) => ToolResult::success(format!("Committed task changes as {oid}")),
            Err(error) => ToolResult::error(error),
        }
    }
}

fn resolve_commit_workspace(working_dir: &str) -> Result<std::path::PathBuf, String> {
    let path = Path::new(working_dir);
    if !path.is_absolute() {
        return Err("Cannot commit outside a valid Arcana workspace: path must be absolute".into());
    }
    let canonical = path.canonicalize().map_err(|error| {
        format!(
            "Cannot resolve the Arcana workspace '{}': {error}",
            path.display()
        )
    })?;
    if !canonical.is_dir() {
        return Err(format!(
            "Cannot commit outside a valid Arcana workspace: '{}' is not a directory",
            canonical.display()
        ));
    }
    Ok(canonical)
}

#[cfg(test)]
async fn commit_paths(repo_path: &Path, paths: &[&str], message: &str) -> Result<String, String> {
    commit_paths_with_checks(repo_path, paths, message, || {}, |_| {}, || Ok(())).await
}

async fn commit_paths_with_checks<B, F, V, G>(
    repo_path: &Path,
    paths: &[&str],
    message: &str,
    mut before_file_lock: B,
    after_file_locks: F,
    mut validate_owner: V,
) -> Result<String, String>
where
    B: FnMut(),
    F: FnOnce(&Repository),
    V: FnMut() -> Result<G, String>,
{
    validate_commit_message(message)?;
    let discovered = Repository::discover(repo_path)
        .map_err(|error| format!("Cannot locate a Git repository: {error}"))?;
    let workdir = discovered
        .workdir()
        .ok_or_else(|| "Cannot commit from a bare Git repository".to_string())?
        .canonicalize()
        .map_err(|error| format!("Cannot resolve repository root: {error}"))?;
    // Every fleet worker shares one checkout and `.git/index`. Hold the same
    // process-wide path lock used by agent file writers across the complete
    // read/stage/check/commit/rollback transaction, then reopen Git state only
    // after acquiring it. Without this, one worker's refusal could reset a
    // second worker's staged files.
    let index_path = discovered.path().join("index");
    let workdir_string = workdir.to_string_lossy().into_owned();
    let _index_guard =
        crate::agent::builtin_tools::path_lock::acquire_for(&index_path, &workdir_string).await;
    let repo = Repository::discover(&workdir)
        .map_err(|error| format!("Cannot reopen Git repository after index lock: {error}"))?;
    let (branch, validated_head_ref, validated_head_oid) = {
        let initial_head = repo
            .head()
            .map_err(|error| format!("Cannot read current Git branch: {error}"))?;
        let branch = initial_head
            .shorthand()
            .map(str::to_owned)
            .unwrap_or_default();
        let validated_head_ref = initial_head
            .name()
            .map(str::to_owned)
            .ok_or_else(|| "Refusing to update a detached HEAD".to_string())?;
        let validated_head_oid = initial_head
            .target()
            .ok_or_else(|| "Refusing to update an unborn HEAD".to_string())?;
        (branch, validated_head_ref, validated_head_oid)
    };
    if matches!(branch.as_str(), "main" | "master" | "trunk") {
        return Err(format!(
            "Refusing to commit on protected branch '{branch}'. Switch to the shared arcana/fleet branch first."
        ));
    }
    if branch.is_empty() {
        return Err("Refusing to commit while HEAD is detached or unborn".into());
    }

    let mut normalized = BTreeSet::new();
    for supplied in paths {
        let path = validate_path(&workdir, supplied)?;
        if repo
            .status_should_ignore(Path::new(&path))
            .map_err(|error| format!("Cannot check whether '{path}' is ignored: {error}"))?
        {
            return Err(format!(
                "Refusing to stage ignored or generated path '{path}'"
            ));
        }
        normalized.insert(path);
    }
    if normalized.is_empty() {
        return Err("At least one explicit changed file is required".into());
    }

    // Hold each target's existing write lock through staging and commit so a
    // concurrent agent writer cannot change the bytes after validation or
    // secret scanning. The BTreeSet order gives every commit the same
    // acquisition order.
    let mut _file_guards = Vec::with_capacity(normalized.len());
    for path in &normalized {
        before_file_lock();
        _file_guards.push(
            crate::agent::builtin_tools::path_lock::acquire_for(Path::new(path), &workdir_string)
                .await,
        );
    }

    // A worker or external Git process can change HEAD while this call waits
    // for file locks. Revalidate the captured ref after all awaits, and only
    // ever update that captured ref below; never resolve a fresh (possibly
    // protected) HEAD ref at the final commit point.
    after_file_locks(&repo);
    ensure_validated_branch(&repo, &validated_head_ref, &validated_head_oid, &branch)?;
    drop(validate_owner()?);

    let mut index = repo
        .index()
        .map_err(|error| format!("Cannot read Git index: {error}"))?;
    let pre_staged = index_changed_paths(&repo, &index)?;
    if !pre_staged.is_empty() {
        return Err(format!(
            "Refusing to mix with pre-staged changes. Commit or unstage them first: {}",
            pre_staged.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    for path in &normalized {
        let absolute = workdir.join(path);
        if absolute.exists() {
            index
                .add_path(Path::new(path))
                .map_err(|error| format!("Could not stage '{path}': {error}"))?;
        } else {
            index
                .remove_path(Path::new(path))
                .map_err(|error| format!("Could not stage deletion of '{path}': {error}"))?;
        }
    }
    if let Err(error) = index.write() {
        let message = format!("Could not write Git index: {error}");
        reset_index_to_head(&repo)?;
        return Err(message);
    }

    let result = (|| {
        let staged = index_changed_paths(&repo, &index)?;
        if staged != normalized {
            return Err(format!(
                "Staged paths did not match the explicit request. Requested [{}], staged [{}].",
                normalized.iter().cloned().collect::<Vec<_>>().join(", "),
                staged.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }

        let repo_path = workdir.to_string_lossy().into_owned();
        let patch = crate::git::diff::diff(&repo_path, None, true)
            .map_err(|error| format!("Could not inspect staged diff: {error}"))?
            .patch;
        if crate::security::redaction::redact_text(&patch) != patch
            || staged_blobs_contain_sensitive_text(&repo, &index, &staged)?
        {
            return Err(
                "Refusing to commit: staged diff contains a credential or sensitive value".into(),
            );
        }

        let tree_oid = index
            .write_tree()
            .map_err(|error| format!("Could not write commit tree: {error}"))?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|error| format!("Could not read commit tree: {error}"))?;
        ensure_validated_branch(&repo, &validated_head_ref, &validated_head_oid, &branch)?;
        let _owner_claim_guard = validate_owner()?;
        let parent = repo
            .find_commit(validated_head_oid)
            .map_err(|error| format!("Cannot read validated branch parent: {error}"))?;
        let signature = repo
            .signature()
            .or_else(|_| Signature::now("Arcana", "arcana@localhost"))
            .map_err(|error| format!("Cannot create commit signature: {error}"))?;
        let oid = repo
            .commit(
                Some(&validated_head_ref),
                &signature,
                &signature,
                message,
                &tree,
                &[&parent],
            )
            .map_err(|error| format!("Git commit failed: {error}"))?;
        Ok(oid.to_string())
    })();

    if result.is_err() {
        reset_index_to_head(&repo)?;
    }
    result
}

fn ensure_validated_branch(
    repo: &Repository,
    validated_ref: &str,
    validated_oid: &git2::Oid,
    branch: &str,
) -> Result<(), String> {
    let head = repo
        .head()
        .map_err(|error| format!("Cannot recheck current Git branch: {error}"))?;
    if head.name() != Some(validated_ref) {
        return Err(format!(
            "Refusing to commit: HEAD changed from validated branch '{branch}' while waiting for locks"
        ));
    }
    if head.target() != Some(*validated_oid) {
        return Err(format!(
            "Refusing to commit: validated branch '{branch}' moved while waiting for locks"
        ));
    }
    Ok(())
}

fn reset_index_to_head(repo: &Repository) -> Result<(), String> {
    let head = repo
        .head()
        .and_then(|head| head.peel_to_commit())
        .map_err(|error| format!("Cannot restore the index after refusal: {error}"))?;
    repo.reset(head.as_object(), git2::ResetType::Mixed, None)
        .map_err(|error| format!("Cannot restore the index after refusal: {error}"))
}

fn staged_blobs_contain_sensitive_text(
    repo: &Repository,
    index: &git2::Index,
    changed_paths: &BTreeSet<String>,
) -> Result<bool, String> {
    for path in changed_paths {
        let Some(entry) = index.get_path(Path::new(path), 0) else {
            // A removed path has no staged blob to scan.
            continue;
        };
        let blob = repo
            .find_blob(entry.id)
            .map_err(|error| format!("Could not read staged blob for '{path}': {error}"))?;
        let text = String::from_utf8_lossy(blob.content());
        if crate::security::redaction::redact_text(&text) != text {
            return Ok(true);
        }
    }
    Ok(false)
}

fn validate_path(workdir: &Path, supplied: &str) -> Result<String, String> {
    let path = Path::new(supplied);
    if supplied.trim().is_empty() || path.is_absolute() {
        return Err(format!(
            "Refusing invalid repository-relative path '{supplied}'"
        ));
    }
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("Refusing unsafe path '{supplied}'"));
    }
    let normalized = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name == ".env" || (name.starts_with(".env.") && name != ".env.example") {
        return Err(format!(
            "Refusing to stage environment secret file '{supplied}'"
        ));
    }
    let absolute = workdir.join(path);
    if absolute.exists() {
        let canonical = absolute
            .canonicalize()
            .map_err(|error| format!("Cannot resolve '{supplied}': {error}"))?;
        if !canonical.starts_with(workdir) {
            return Err(format!(
                "Refusing path outside the repository: '{supplied}'"
            ));
        }
        if !canonical.is_file() {
            return Err(format!("Only explicit files may be staged: '{supplied}'"));
        }
    } else {
        let parent = absolute
            .parent()
            .ok_or_else(|| format!("Invalid parent directory for '{supplied}'"))?
            .canonicalize()
            .map_err(|error| format!("Cannot resolve parent of '{supplied}': {error}"))?;
        if !parent.starts_with(workdir) {
            return Err(format!(
                "Refusing path outside the repository: '{supplied}'"
            ));
        }
    }
    Ok(normalized)
}

fn index_changed_paths(repo: &Repository, index: &git2::Index) -> Result<BTreeSet<String>, String> {
    let head_tree = repo.head().ok().and_then(|head| head.peel_to_tree().ok());
    let diff = repo
        .diff_tree_to_index(head_tree.as_ref(), Some(index), None)
        .map_err(|error| format!("Cannot compare Git index with HEAD: {error}"))?;
    Ok(diff
        .deltas()
        .filter_map(|delta| delta.new_file().path().or_else(|| delta.old_file().path()))
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect())
}

fn validate_commit_message(message: &str) -> Result<(), String> {
    let mut lines = message.lines();
    let subject = lines.next().unwrap_or_default();
    let valid_subject = subject.split_once(": ").is_some_and(|(kind, summary)| {
        !summary.trim().is_empty()
            && kind
                .split_once('(')
                .map(|(prefix, scope)| {
                    let scope_value = scope.strip_suffix(')').unwrap_or_default();
                    !prefix.is_empty()
                        && scope.ends_with(')')
                        && !scope_value.trim().is_empty()
                        && prefix.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                })
                .unwrap_or_else(|| {
                    !kind.is_empty() && kind.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                })
    });
    if !valid_subject {
        return Err("Commit subject must use Conventional Commits, such as 'fix(core): prevent trunk commits'".into());
    }
    if lines.next() != Some("") {
        return Err("Commit message must separate its subject and body with a blank line".into());
    }
    let body = lines.collect::<Vec<_>>().join("\n");
    if body.trim().is_empty() {
        return Err("Commit message must include a descriptive body after the subject".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn repo() -> (tempfile::TempDir, Repository) {
        let dir = tempfile::tempdir().expect("temporary repository directory");
        let mut options = git2::RepositoryInitOptions::new();
        options.initial_head("main");
        let repo = Repository::init_opts(dir.path(), &options).expect("init repository");
        let mut config = repo.config().expect("git config");
        config.set_str("user.name", "Arcana Test").unwrap();
        config
            .set_str("user.email", "arcana-test@example.invalid")
            .unwrap();
        fs::write(dir.path().join("README.md"), "base\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let signature = Signature::now("Arcana Test", "arcana-test@example.invalid").unwrap();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "chore: initialize test repo\n\nTest fixture.",
            &tree,
            &[],
        )
        .unwrap();
        drop(tree);
        (dir, repo)
    }

    fn branch_to_fleet(repo: &Repository) {
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        let branch = repo.branch("arcana/fleet", &head, false).unwrap();
        repo.set_head(branch.get().name().unwrap()).unwrap();
    }

    #[tokio::test]
    async fn commit_refuses_protected_trunk() {
        let (dir, _) = repo();
        fs::write(dir.path().join("change.txt"), "change\n").unwrap();
        let error = commit_paths(
            dir.path(),
            &["change.txt"],
            "fix(core): safe change\n\nBody.",
        )
        .await
        .unwrap_err();
        assert!(error.contains("protected branch 'main'"), "{error}");
    }

    #[tokio::test]
    async fn commit_refuses_branch_named_trunk() {
        let (dir, repo) = repo();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        let branch = repo.branch("trunk", &head, false).unwrap();
        repo.set_head(branch.get().name().unwrap()).unwrap();
        fs::write(dir.path().join("change.txt"), "change\n").unwrap();
        assert!(commit_paths(
            dir.path(),
            &["change.txt"],
            "fix(core): safe change\n\nBody."
        )
        .await
        .unwrap_err()
        .contains("protected branch 'trunk'"));
    }

    #[tokio::test]
    async fn commit_refuses_head_switch_to_protected_branch_while_waiting_for_file_locks() {
        let (dir, repo) = repo();
        branch_to_fleet(&repo);
        let fleet_before = repo.head().unwrap().target().unwrap();
        let main_before = repo
            .find_branch("main", git2::BranchType::Local)
            .unwrap()
            .get()
            .target()
            .unwrap();
        fs::write(dir.path().join("change.txt"), "change\n").unwrap();

        let error = commit_paths_with_checks(
            dir.path(),
            &["change.txt"],
            "fix(core): refuse changed branch\n\nA branch switch during lock acquisition must abort.",
            || {},
            |repo| repo.set_head("refs/heads/main").expect("switch HEAD to protected branch"),
            || Ok(()),
        )
        .await
        .expect_err("changed HEAD must stop the commit");

        assert!(
            error.contains("HEAD changed from validated branch 'arcana/fleet'"),
            "{error}"
        );
        assert_eq!(repo.head().unwrap().shorthand(), Some("main"));
        assert_eq!(repo.head().unwrap().target(), Some(main_before));
        assert_eq!(
            repo.find_branch("arcana/fleet", git2::BranchType::Local)
                .unwrap()
                .get()
                .target(),
            Some(fleet_before)
        );
        assert!(repo
            .status_file(Path::new("change.txt"))
            .unwrap()
            .contains(git2::Status::WT_NEW));
    }

    #[tokio::test]
    async fn commit_rechecks_task_claim_after_file_lock_waits() {
        let (dir, repo) = repo();
        branch_to_fleet(&repo);
        let branch_before = repo.head().unwrap().target().unwrap();
        fs::write(dir.path().join("change.txt"), "change\n").unwrap();

        let error = commit_paths_with_checks(
            dir.path(),
            &["change.txt"],
            "fix(core): refuse stale task owner\n\nA reassigned worker cannot commit after waiting for locks.",
            || {},
            |_| {},
            || Err::<(), _>("This worker no longer owns the in-progress task".into()),
        )
        .await
        .expect_err("a claim revoked during lock wait must abort");

        assert!(error.contains("no longer owns"), "{error}");
        assert_eq!(repo.head().unwrap().target(), Some(branch_before));
        assert!(repo
            .status_file(Path::new("change.txt"))
            .unwrap()
            .contains(git2::Status::WT_NEW));
        assert!(index_changed_paths(&repo, &repo.index().unwrap())
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn production_task_claim_is_rechecked_after_file_lock_wait() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let tasks = crate::db::tasks::TasksDb::open().expect("open isolated tasks DB");
        let task = tasks
            .create(crate::db::tasks::CreateTaskInput {
                title: "commit claim revalidation".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "stale owner cannot commit after lock wait".into(),
                created_by: Some("git-commit-test".into()),
            })
            .expect("create task");
        tasks
            .update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign task");
        tasks
            .claim_task_with_pid(task.id, "worker-backend", std::process::id())
            .expect("claim task");

        let (dir, repo) = repo();
        branch_to_fleet(&repo);
        let head_before = repo.head().unwrap().target().unwrap();
        fs::write(dir.path().join("change.txt"), "change\n").unwrap();
        let key = crate::agent::builtin_tools::path_lock::lock_key(
            Path::new("change.txt"),
            &dir.path().to_string_lossy(),
        );
        let held_file_lock = crate::agent::builtin_tools::path_lock::acquire_owned(&key).await;
        let waiting = std::sync::Arc::new(tokio::sync::Notify::new());
        let waiting_for_lock = std::sync::Arc::clone(&waiting);
        let context = crate::agent::tool_trait::ToolContext {
            session_id: "git-commit-claim-test".into(),
            working_dir: dir.path().to_string_lossy().into_owned(),
            call_id: "git-commit-claim-call".into(),
            run_id: None,
            task_id: Some(task.id),
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        };
        let repo_path = dir.path().to_path_buf();
        context
            .require_current_task_claim()
            .expect("tool begins with a valid production claim");

        let commit = tokio::spawn(async move {
            commit_paths_with_checks(
                &repo_path,
                &["change.txt"],
                "fix(core): reject stale owner\n\nProduction TasksDb claim is revoked while the file lock is held.",
                move || waiting_for_lock.notify_one(),
                |_| {},
                || context.require_current_task_claim(),
            )
            .await
        });

        tokio::time::timeout(std::time::Duration::from_secs(2), waiting.notified())
            .await
            .expect("commit reached the file-lock acquisition after initial validation");
        tasks
            .update_status(task.id, "needs_info", Some("worker-backend"))
            .expect("revoke the active claim through the production task API");
        drop(held_file_lock);

        let error = tokio::time::timeout(std::time::Duration::from_secs(2), commit)
            .await
            .expect("commit finishes after lock release")
            .expect("commit task joins")
            .expect_err("commit with a stale production claim must be refused");
        assert!(error.contains("no longer owns"), "{error}");
        assert_eq!(repo.head().unwrap().target(), Some(head_before));
        assert!(index_changed_paths(&repo, &repo.index().unwrap())
            .unwrap()
            .is_empty());
        assert!(repo
            .status_file(Path::new("change.txt"))
            .unwrap()
            .contains(git2::Status::WT_NEW));
    }

    #[tokio::test]
    async fn production_git_commit_serializes_claim_revocation_after_final_check() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let tasks = crate::db::tasks::TasksDb::open().expect("open isolated tasks DB");
        let task = tasks
            .create(crate::db::tasks::CreateTaskInput {
                title: "commit claim serialization".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "claim revocation serializes behind the Git ref update".into(),
                created_by: Some("git-commit-test".into()),
            })
            .expect("create task");
        tasks
            .update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign task");
        tasks
            .claim_task_with_pid(task.id, "worker-backend", std::process::id())
            .expect("claim task");

        let (dir, repo) = repo();
        branch_to_fleet(&repo);
        let head_before = repo.head().unwrap().target().unwrap();
        fs::write(dir.path().join("change.txt"), "change\n").unwrap();
        let context = crate::agent::tool_trait::ToolContext {
            session_id: "git-commit-claim-lock-test".into(),
            working_dir: dir.path().to_string_lossy().into_owned(),
            call_id: "git-commit-claim-lock-call".into(),
            run_id: None,
            task_id: Some(task.id),
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        };
        let (writer_started_tx, writer_started_rx) = std::sync::mpsc::channel();
        let (writer_done_tx, writer_done_rx) = std::sync::mpsc::channel();
        let mut writer = None;
        let mut validations = 0;
        let repo_path = dir.path().to_path_buf();
        let oid = commit_paths_with_checks(
            &repo_path,
            &["change.txt"],
            "fix(core): serialize task claim\n\nThe final claim lock protects the host-owned Git ref update.",
            || {},
            |_| {},
            || {
                validations += 1;
                let claim = context.lock_current_task_claim()?;
                if validations == 2 {
                    let writer_started_tx = writer_started_tx.clone();
                    let writer_done_tx = writer_done_tx.clone();
                    writer = Some(std::thread::spawn(move || {
                        writer_started_tx.send(()).expect("signal status update attempt");
                        let tasks = crate::db::tasks::TasksDb::open()
                            .expect("open concurrent task writer");
                        tasks
                            .update_status(task.id, "needs_info", Some("worker-backend"))
                            .expect("status writer continues after commit guard drops");
                        writer_done_tx.send(()).expect("signal status update complete");
                    }));
                    writer_started_rx
                        .recv_timeout(std::time::Duration::from_secs(2))
                        .expect("concurrent writer reached the guarded commit window");
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    assert!(
                        writer_done_rx.try_recv().is_err(),
                        "task ownership changed between final validation and Git ref update"
                    );
                }
                Ok(claim)
            },
        )
        .await
        .expect("the current task owner can commit while status revocation waits");
        writer
            .expect("spawn status writer")
            .join()
            .expect("status writer joins");
        writer_done_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("status transition completes after guarded Git commit");

        assert_ne!(repo.head().unwrap().target(), Some(head_before));
        assert_eq!(
            tasks.get_by_id(task.id).unwrap().unwrap().status.as_str(),
            "needs_info"
        );
        assert!(repo
            .status_file(Path::new("change.txt"))
            .unwrap()
            .is_empty());
        assert_ne!(oid, head_before.to_string());
    }

    #[tokio::test]
    async fn commit_stages_only_named_files_and_commits_with_body_on_fleet_branch() {
        let (dir, repo) = repo();
        branch_to_fleet(&repo);
        fs::write(dir.path().join("change.txt"), "change\n").unwrap();
        fs::write(dir.path().join("leave.txt"), "leave unstaged\n").unwrap();
        let oid = commit_paths(
            dir.path(),
            &["change.txt"],
            "fix(core): safe change\n\nOnly the named file is included.",
        )
        .await
        .unwrap();
        assert_eq!(repo.head().unwrap().shorthand(), Some("arcana/fleet"));
        assert!(repo.find_commit(oid.parse().unwrap()).is_ok());
        assert_eq!(repo.head().unwrap().target().unwrap().to_string(), oid);
        let committed_tree = repo.head().unwrap().peel_to_tree().unwrap();
        assert!(committed_tree.get_path(Path::new("change.txt")).is_ok());
        assert!(committed_tree.get_path(Path::new("leave.txt")).is_err());
        assert!(repo
            .status_file(Path::new("leave.txt"))
            .unwrap()
            .contains(git2::Status::WT_NEW));
        fs::write(dir.path().join("second.txt"), "second\n").unwrap();
        commit_paths(
            dir.path(),
            &["second.txt"],
            "fix(core): allow the next worker commit\n\nThe first commit leaves no staged changes behind.",
        )
        .await
        .expect("a completed commit leaves the index ready for the next worker");
    }

    #[tokio::test]
    async fn commit_refuses_an_explicitly_named_ignored_build_output() {
        let (dir, repo) = repo();
        branch_to_fleet(&repo);
        fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        let output = dir.path().join("target").join("app.bin");
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::write(&output, "generated build output").unwrap();
        let before = repo.head().unwrap().target();

        let error = commit_paths(
            dir.path(),
            &["target/app.bin"],
            "fix(core): stage only source changes\n\nIgnored build artifacts must stay out of commits.",
        )
        .await
        .unwrap_err();

        assert!(error.contains("ignored or generated path"), "{error}");
        assert_eq!(repo.head().unwrap().target(), before);
        assert!(repo
            .status_file(Path::new("target/app.bin"))
            .unwrap()
            .contains(git2::Status::IGNORED));
    }

    #[tokio::test]
    async fn commit_refuses_pre_staged_paths_and_secret_files() {
        let (dir, repo) = repo();
        branch_to_fleet(&repo);
        fs::write(dir.path().join("other.txt"), "other\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("other.txt")).unwrap();
        index.write().unwrap();
        fs::write(dir.path().join("change.txt"), "change\n").unwrap();
        assert!(commit_paths(
            dir.path(),
            &["change.txt"],
            "fix(core): safe change\n\nBody."
        )
        .await
        .unwrap_err()
        .contains("pre-staged changes"));
        assert!(validate_path(dir.path(), ".env")
            .unwrap_err()
            .contains("environment secret"));
        assert!(validate_path(dir.path(), "../outside.txt")
            .unwrap_err()
            .contains("unsafe path"));
    }

    #[tokio::test]
    async fn commit_refuses_redactable_credentials_in_diff_and_restores_index() {
        let (dir, repo) = repo();
        branch_to_fleet(&repo);
        fs::write(
            dir.path().join("change.txt"),
            "GOOGLE_API_KEY=not-a-real-secret\n",
        )
        .unwrap();
        let error = commit_paths(
            dir.path(),
            &["change.txt"],
            "fix(core): keep credentials out of commits\n\nReject a credential-shaped diff.",
        )
        .await
        .unwrap_err();
        assert!(error.contains("credential or sensitive value"), "{error}");
        let index = repo.index().unwrap();
        assert!(index_changed_paths(&repo, &index).unwrap().is_empty());
    }

    #[tokio::test]
    async fn commit_refuses_recognizable_credentials_embedded_in_binary_file() {
        let (dir, repo) = repo();
        branch_to_fleet(&repo);
        fs::write(
            dir.path().join("asset.bin"),
            b"\0binary-prefix-GOOGLE_API_KEY=not-a-real-secret\n",
        )
        .unwrap();

        let error = commit_paths(
            dir.path(),
            &["asset.bin"],
            "fix(core): block credential in binary\n\nInspect raw bytes as well as text diffs.",
        )
        .await
        .unwrap_err();
        assert!(error.contains("credential or sensitive value"), "{error}");
        assert!(repo
            .index()
            .unwrap()
            .get_path(Path::new("asset.bin"), 0)
            .is_none());
        assert!(repo
            .status_file(Path::new("asset.bin"))
            .unwrap()
            .contains(git2::Status::WT_NEW));
    }

    #[tokio::test]
    async fn commit_input_refuses_non_string_path_elements() {
        let (dir, _repo) = repo();
        let context = ToolContext {
            session_id: "git-commit-test".into(),
            working_dir: dir.path().to_string_lossy().into_owned(),
            call_id: "invalid-path-array".into(),
            run_id: None,
            task_id: None,
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "supervisor".into(),

            run_token_budget: None,
        };
        let result = GitCommitTool
            .execute(
                json!({"paths": ["README.md", 42], "message": "fix: reject malformed paths\n\nNo path should be committed."}),
                &context,
            )
            .await;
        assert!(result.is_error);
        assert!(result.output.contains("Every paths entry must be a string"));
    }

    #[tokio::test]
    async fn concurrent_commits_serialize_index_staging_and_refusal_rollback() {
        let (dir, repo) = repo();
        branch_to_fleet(&repo);
        fs::write(dir.path().join("first.rs"), "fn first() {}\n").unwrap();
        fs::write(dir.path().join("second.rs"), "fn second() {}\n").unwrap();
        fs::write(
            dir.path().join("secret.bin"),
            b"\0GOOGLE_API_KEY=not-a-real-secret\n",
        )
        .unwrap();
        let root = dir.path().to_path_buf();
        let first_root = root.clone();
        let second_root = root.clone();
        let secret_root = root;

        let (first, second, secret) = tokio::join!(
            commit_paths(
                &first_root,
                &["first.rs"],
                "fix(core): commit first file\n\nOnly the first named file is included."
            ),
            commit_paths(
                &second_root,
                &["second.rs"],
                "fix(core): commit second file\n\nOnly the second named file is included."
            ),
            commit_paths(
                &secret_root,
                &["secret.bin"],
                "fix(core): reject secret file content\n\nRecognized credentials must not be committed."
            )
        );

        first.expect("first commit should succeed");
        second.expect("second commit should succeed");
        assert!(secret
            .unwrap_err()
            .contains("credential or sensitive value"));
        let repo = Repository::open(dir.path()).expect("reopen repository after concurrent calls");
        let tree = repo.head().unwrap().peel_to_tree().unwrap();
        assert!(tree.get_path(Path::new("first.rs")).is_ok());
        assert!(tree.get_path(Path::new("second.rs")).is_ok());
        assert!(tree.get_path(Path::new("secret.bin")).is_err());
        let index = repo.index().unwrap();
        assert!(index_changed_paths(&repo, &index).unwrap().is_empty());
        assert!(repo
            .status_file(Path::new("secret.bin"))
            .unwrap()
            .contains(git2::Status::WT_NEW));
    }

    #[test]
    fn commit_message_requires_conventional_subject_and_body() {
        assert!(validate_commit_message("did stuff\n\nBody").is_err());
        assert!(validate_commit_message("fix( ): safe change\n\nBody").is_err());
        assert!(validate_commit_message("fix(   ): safe change\n\nBody").is_err());
        assert!(validate_commit_message("fix(core): safe change").is_err());
        assert!(validate_commit_message("fix(core): safe change\nBody").is_err());
        assert!(validate_commit_message(
            "fix(core): safe change\n\nExplains the behavior and proof."
        )
        .is_ok());
    }

    #[test]
    fn commit_workspace_requires_an_existing_absolute_root() {
        assert!(resolve_commit_workspace(".")
            .unwrap_err()
            .contains("must be absolute"));
        let dir = tempfile::tempdir().expect("workspace directory");
        assert_eq!(
            resolve_commit_workspace(&dir.path().to_string_lossy()).unwrap(),
            dir.path().canonicalize().unwrap()
        );
    }
}
