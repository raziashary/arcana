//! Typed Git bookkeeping for workers (`W9`).
//!
//! The worker recipe used to run `git rev-parse`, `git switch -c` and
//! `git status` through `bash`, so the user was asked to approve a free-form
//! shell for what are two fixed questions. `git_status` answers "where am I and
//! what changed" and `git_use_fleet_branch` is the one branch move the commit
//! protocol needs; neither takes a command, a branch name or a path from the
//! model.

use async_trait::async_trait;
use git2::{build::CheckoutBuilder, BranchType, ObjectType, Repository, Status, StatusOptions};
use serde_json::{json, Value};
use std::path::Path;

use super::git_commit::resolve_commit_workspace;
use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

/// Branches no worker commits to. `git_commit` refuses the same three.
const PROTECTED_BRANCHES: [&str; 3] = ["main", "master", "trunk"];
/// The one shared branch every fleet worker commits to.
const FLEET_BRANCH: &str = "arcana/fleet";
const MAX_STATUS_ENTRIES: usize = 200;

pub struct GitStatusTool;
pub struct GitUseFleetBranchTool;

fn open_repository(working_dir: &str) -> Result<(Repository, std::path::PathBuf), String> {
    let workspace = resolve_commit_workspace(working_dir)?;
    let repo = Repository::discover(&workspace)
        .map_err(|error| format!("Cannot locate a Git repository: {error}"))?;
    Ok((repo, workspace))
}

fn current_branch(repo: &Repository) -> Result<String, String> {
    let head = repo
        .head()
        .map_err(|error| format!("Cannot read the current Git branch: {error}"))?;
    if !head.is_branch() {
        return Err("HEAD is detached, so there is no current branch.".into());
    }
    head.shorthand()
        .map(str::to_owned)
        .ok_or_else(|| "The current Git branch name is not valid UTF-8.".to_string())
}

fn status_code(status: Status) -> String {
    let index = if status.contains(Status::INDEX_NEW) {
        'A'
    } else if status.contains(Status::INDEX_MODIFIED) {
        'M'
    } else if status.contains(Status::INDEX_DELETED) {
        'D'
    } else if status.contains(Status::INDEX_RENAMED) {
        'R'
    } else if status.contains(Status::INDEX_TYPECHANGE) {
        'T'
    } else {
        ' '
    };
    let tree = if status.contains(Status::WT_NEW) {
        '?'
    } else if status.contains(Status::WT_MODIFIED) {
        'M'
    } else if status.contains(Status::WT_DELETED) {
        'D'
    } else if status.contains(Status::WT_RENAMED) {
        'R'
    } else if status.contains(Status::WT_TYPECHANGE) {
        'T'
    } else {
        ' '
    };
    if status.contains(Status::CONFLICTED) {
        return "UU".into();
    }
    if index == ' ' && tree == '?' {
        return "??".into();
    }
    format!("{index}{tree}")
}

#[async_trait]
impl Tool for GitStatusTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Read
    }

    fn name(&self) -> &str {
        "git_status"
    }

    fn description(&self) -> &str {
        "Report the current Git branch, whether it is a protected trunk (main, master, trunk), and the changed files under the selected workspace in short porcelain form. Read-only; takes no arguments."
    }

    fn input_schema(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    async fn execute(&self, _input: Value, ctx: &ToolContext) -> ToolResult {
        let (repo, workspace) = match open_repository(&ctx.working_dir) {
            Ok(opened) => opened,
            Err(error) => return ToolResult::error(error),
        };
        let branch = match current_branch(&repo) {
            Ok(branch) => branch,
            Err(error) => return ToolResult::error(error),
        };
        let mut options = StatusOptions::new();
        options
            .include_untracked(true)
            .recurse_untracked_dirs(false);
        // Only the selected workspace is reported when it is a subfolder of the
        // repository.
        let relative = repo.workdir().and_then(|workdir| {
            let workdir = workdir.canonicalize().ok()?;
            workspace
                .strip_prefix(workdir)
                .ok()
                .map(|path| path.to_string_lossy().replace('\\', "/"))
        });
        if let Some(prefix) = relative.filter(|prefix| !prefix.is_empty()) {
            options.pathspec(prefix);
        }
        let statuses = match repo.statuses(Some(&mut options)) {
            Ok(statuses) => statuses,
            Err(error) => return ToolResult::error(format!("Cannot read Git status: {error}")),
        };
        let mut lines = Vec::new();
        for entry in statuses.iter() {
            let Some(path) = entry.path() else { continue };
            if lines.len() == MAX_STATUS_ENTRIES {
                break;
            }
            lines.push(format!("{} {path}", status_code(entry.status())));
        }
        let omitted = statuses.len().saturating_sub(lines.len());
        let mut report = format!(
            "branch: {branch}\nprotected: {}\nchanged files: {}\n",
            if PROTECTED_BRANCHES.contains(&branch.as_str()) {
                "yes"
            } else {
                "no"
            },
            statuses.len()
        );
        for line in &lines {
            report.push_str(line);
            report.push('\n');
        }
        if omitted > 0 {
            report.push_str(&format!("... and {omitted} more\n"));
        }
        ToolResult::success(report)
    }
}

#[async_trait]
impl Tool for GitUseFleetBranchTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Write
    }

    fn name(&self) -> &str {
        "git_use_fleet_branch"
    }

    fn description(&self) -> &str {
        "Move the shared checkout off a protected trunk (main, master, trunk) onto the shared arcana/fleet branch, creating it from the current commit if it does not exist and joining it if a peer already did. On any other branch it changes nothing. Never discards local changes; refuses if they would be overwritten. Takes no arguments."
    }

    fn input_schema(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    async fn execute(&self, _input: Value, ctx: &ToolContext) -> ToolResult {
        if let Err(message) = ctx.require_current_task_claim() {
            return ToolResult::error(message);
        }
        match use_fleet_branch(&ctx.working_dir).await {
            Ok(message) => ToolResult::success(message),
            Err(error) => ToolResult::error(error),
        }
    }
}

async fn use_fleet_branch(working_dir: &str) -> Result<String, String> {
    let (discovered, _) = open_repository(working_dir)?;
    let workdir = discovered
        .workdir()
        .ok_or_else(|| "Cannot switch branches in a bare Git repository".to_string())?
        .canonicalize()
        .map_err(|error| format!("Cannot resolve repository root: {error}"))?;
    // Every fleet worker shares one checkout. Take the lock `git_commit` takes,
    // so two workers cannot move HEAD at once, and read HEAD only after it is
    // held: the peer that got there first leaves this call nothing to do.
    let index_path = discovered.path().join("index");
    let workdir_string = workdir.to_string_lossy().into_owned();
    let _index_guard = super::path_lock::acquire_for(Path::new(&index_path), &workdir_string).await;
    let repo = Repository::discover(&workdir)
        .map_err(|error| format!("Cannot reopen the Git repository after the lock: {error}"))?;

    let branch = current_branch(&repo)?;
    if !PROTECTED_BRANCHES.contains(&branch.as_str()) {
        return Ok(format!(
            "Already on '{branch}', which is not a protected trunk. Nothing was changed; stay on it."
        ));
    }
    let head_commit = repo
        .head()
        .and_then(|head| head.peel_to_commit())
        .map_err(|error| format!("Cannot read the current commit: {error}"))?;
    let (fleet, created) = match repo.find_branch(FLEET_BRANCH, BranchType::Local) {
        Ok(existing) => (existing, false),
        Err(_) => (
            repo.branch(FLEET_BRANCH, &head_commit, false)
                .map_err(|error| format!("Cannot create {FLEET_BRANCH}: {error}"))?,
            true,
        ),
    };
    let fleet_ref = fleet
        .get()
        .name()
        .ok_or_else(|| format!("The {FLEET_BRANCH} reference name is not valid UTF-8."))?
        .to_owned();
    let target = fleet
        .get()
        .peel(ObjectType::Commit)
        .map_err(|error| format!("Cannot read {FLEET_BRANCH}: {error}"))?;
    // Safe checkout stops, without touching anything, if a local change would be
    // overwritten; HEAD moves only once the files are in place.
    repo.checkout_tree(&target, Some(CheckoutBuilder::new().safe()))
        .map_err(|error| {
            format!("Cannot switch to {FLEET_BRANCH} without overwriting local changes: {error}")
        })?;
    repo.set_head(&fleet_ref)
        .map_err(|error| format!("Cannot point HEAD at {FLEET_BRANCH}: {error}"))?;
    Ok(format!(
        "Moved from '{branch}' to the shared {FLEET_BRANCH} branch ({}). Commit with git_commit; do not switch branches again during this task.",
        if created { "created" } else { "joined" }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;
    use std::fs;

    fn repo() -> (tempfile::TempDir, Repository) {
        let dir = tempfile::tempdir().expect("temporary repository directory");
        let mut options = git2::RepositoryInitOptions::new();
        options.initial_head("main");
        let repo = Repository::init_opts(dir.path(), &options).expect("init repository");
        fs::write(dir.path().join("README.md"), "base\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
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

    fn context(dir: &Path) -> ToolContext {
        ToolContext {
            session_id: "git-branch-session".into(),
            working_dir: dir.to_string_lossy().into_owned(),
            call_id: "git-branch-call".into(),
            run_id: None,
            task_id: None,
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),
            run_token_budget: None,
        }
    }

    #[tokio::test]
    async fn status_reports_branch_protection_and_changed_files() {
        let (dir, _repo) = repo();
        let clean = GitStatusTool.execute(json!({}), &context(dir.path())).await;
        assert!(!clean.is_error, "{}", clean.output);
        assert!(clean.output.contains("branch: main"), "{}", clean.output);
        assert!(clean.output.contains("protected: yes"), "{}", clean.output);
        assert!(
            clean.output.contains("changed files: 0"),
            "{}",
            clean.output
        );

        fs::write(dir.path().join("README.md"), "edited\n").unwrap();
        fs::write(dir.path().join("new.txt"), "new\n").unwrap();
        let dirty = GitStatusTool.execute(json!({}), &context(dir.path())).await;
        assert!(dirty.output.contains(" M README.md"), "{}", dirty.output);
        assert!(dirty.output.contains("?? new.txt"), "{}", dirty.output);
    }

    #[tokio::test]
    async fn status_is_bounded_and_scoped_to_the_workspace_subfolder() {
        let (dir, repo) = repo();
        // A directory that already holds a tracked file lists its new files one
        // by one; a wholly untracked directory is reported as a single entry.
        fs::create_dir(dir.path().join("inside")).unwrap();
        fs::write(dir.path().join("inside").join("tracked.txt"), "x").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("inside/tracked.txt")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let signature = Signature::now("Arcana Test", "arcana-test@example.invalid").unwrap();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "chore: track a file

Fixture.",
            &tree,
            &[&parent],
        )
        .unwrap();
        drop(tree);
        for index in 0..(MAX_STATUS_ENTRIES + 25) {
            fs::write(dir.path().join("inside").join(format!("f{index}.txt")), "x").unwrap();
        }
        fs::write(dir.path().join("outside.txt"), "x").unwrap();
        let report = GitStatusTool
            .execute(json!({}), &context(&dir.path().join("inside")))
            .await;
        assert!(!report.is_error, "{}", report.output);
        assert!(!report.output.contains("outside.txt"), "{}", report.output);
        assert!(report.output.contains("and 25 more"), "{}", report.output);
        assert_eq!(
            report
                .output
                .lines()
                .filter(|l| l.starts_with("??"))
                .count(),
            MAX_STATUS_ENTRIES
        );
    }

    #[tokio::test]
    async fn status_refuses_a_detached_head() {
        let (dir, repo) = repo();
        let commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.set_head_detached(commit.id()).unwrap();
        let result = GitStatusTool.execute(json!({}), &context(dir.path())).await;
        assert!(result.is_error);
        assert!(result.output.contains("detached"), "{}", result.output);
    }

    #[tokio::test]
    async fn a_trunk_is_left_for_a_new_fleet_branch_keeping_local_changes() {
        let (dir, repo) = repo();
        fs::write(dir.path().join("README.md"), "edited\n").unwrap();
        fs::write(dir.path().join("new.txt"), "new\n").unwrap();

        let moved = use_fleet_branch(&dir.path().to_string_lossy())
            .await
            .unwrap();
        assert!(moved.contains("created"), "{moved}");
        assert_eq!(repo.head().unwrap().shorthand(), Some(FLEET_BRANCH));
        assert_eq!(
            fs::read_to_string(dir.path().join("README.md")).unwrap(),
            "edited\n"
        );
        assert!(dir.path().join("new.txt").is_file());
        assert!(repo.find_branch("main", BranchType::Local).is_ok());
    }

    #[tokio::test]
    async fn a_second_call_and_a_non_trunk_branch_change_nothing() {
        let (dir, repo) = repo();
        use_fleet_branch(&dir.path().to_string_lossy())
            .await
            .unwrap();
        let again = use_fleet_branch(&dir.path().to_string_lossy())
            .await
            .unwrap();
        assert!(again.contains("Already on 'arcana/fleet'"), "{again}");

        let head = repo.head().unwrap().peel_to_commit().unwrap();
        let feature = repo.branch("feature/x", &head, false).unwrap();
        repo.set_head(feature.get().name().unwrap()).unwrap();
        let left = use_fleet_branch(&dir.path().to_string_lossy())
            .await
            .unwrap();
        assert!(left.contains("Already on 'feature/x'"), "{left}");
        assert_eq!(repo.head().unwrap().shorthand(), Some("feature/x"));
    }

    #[tokio::test]
    async fn an_existing_fleet_branch_is_joined_not_recreated() {
        let (dir, repo) = repo();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        let fleet = repo.branch(FLEET_BRANCH, &head, false).unwrap();
        let before = fleet.get().target().unwrap();

        let moved = use_fleet_branch(&dir.path().to_string_lossy())
            .await
            .unwrap();
        assert!(moved.contains("joined"), "{moved}");
        assert_eq!(repo.head().unwrap().shorthand(), Some(FLEET_BRANCH));
        assert_eq!(repo.head().unwrap().target(), Some(before));
    }

    #[tokio::test]
    async fn joining_a_diverged_fleet_branch_never_overwrites_a_local_change() {
        let (dir, repo) = repo();
        let signature = Signature::now("Arcana Test", "arcana-test@example.invalid").unwrap();
        // Commit a different README on the fleet branch.
        let base = repo.head().unwrap().peel_to_commit().unwrap();
        let mut index = repo.index().unwrap();
        fs::write(dir.path().join("README.md"), "fleet version\n").unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let fleet_commit = repo
            .commit(
                None,
                &signature,
                &signature,
                "feat: fleet change\n\nDiverges README.",
                &tree,
                &[&base],
            )
            .unwrap();
        repo.reference(
            &format!("refs/heads/{FLEET_BRANCH}"),
            fleet_commit,
            false,
            "test",
        )
        .unwrap();
        // Back to the trunk's content, plus an uncommitted edit of the same file.
        repo.reset(base.as_object(), git2::ResetType::Hard, None)
            .unwrap();
        fs::write(dir.path().join("README.md"), "my local edit\n").unwrap();

        let error = use_fleet_branch(&dir.path().to_string_lossy())
            .await
            .unwrap_err();
        assert!(
            error.contains("without overwriting local changes"),
            "{error}"
        );
        assert_eq!(repo.head().unwrap().shorthand(), Some("main"));
        assert_eq!(
            fs::read_to_string(dir.path().join("README.md")).unwrap(),
            "my local edit\n"
        );
    }

    #[tokio::test]
    async fn a_detached_head_is_refused_and_a_task_claim_is_required() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let (dir, repo) = repo();
        let commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.set_head_detached(commit.id()).unwrap();
        let error = use_fleet_branch(&dir.path().to_string_lossy())
            .await
            .unwrap_err();
        assert!(error.contains("detached"), "{error}");

        let unclaimed = GitUseFleetBranchTool
            .execute(json!({}), &context(dir.path()))
            .await;
        assert!(unclaimed.is_error);
        assert!(
            unclaimed.output.contains("task claim"),
            "{}",
            unclaimed.output
        );
    }
}
