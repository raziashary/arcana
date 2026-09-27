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
        "Stage only the named changed files and commit them after host checks. Refuses main/master, pre-staged unrelated changes, secret-bearing diffs, unsafe paths, and commit messages without a Conventional Commit subject and descriptive body."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "paths": {
                    "type": "array",
                    "minItems": 1,
                    "items": { "type": "string" },
                    "description": "Explicit repository-relative files changed by this task. Directories, globs, absolute paths, and .env files are refused."
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
        let paths = match input["paths"].as_array() {
            Some(paths) => paths.iter().filter_map(Value::as_str).collect::<Vec<_>>(),
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
        match commit_paths(&workspace, &paths, message) {
            Ok(oid) => ToolResult::success(format!("Committed task changes as {oid}")),
            Err(error) => ToolResult::error(error),
        }
    }
}

fn resolve_commit_workspace(working_dir: &str) -> Result<std::path::PathBuf, String> {
    crate::platform::workspace::resolve_workspace_dir(Some(working_dir))
        .map_err(|error| format!("Cannot commit outside a valid Arcana workspace: {error}"))
}

fn commit_paths(repo_path: &Path, paths: &[&str], message: &str) -> Result<String, String> {
    validate_commit_message(message)?;
    let repo = Repository::discover(repo_path)
        .map_err(|error| format!("Cannot locate a Git repository: {error}"))?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| "Cannot commit from a bare Git repository".to_string())?
        .canonicalize()
        .map_err(|error| format!("Cannot resolve repository root: {error}"))?;
    let branch = repo
        .head()
        .ok()
        .and_then(|head| head.shorthand().map(str::to_owned))
        .unwrap_or_default();
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
        normalized.insert(validate_path(&workdir, supplied)?);
    }
    if normalized.is_empty() {
        return Err("At least one explicit changed file is required".into());
    }

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
    index
        .write()
        .map_err(|error| format!("Could not write Git index: {error}"))?;

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
        if crate::security::redaction::redact_text(&patch) != patch {
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
        let parent = repo
            .head()
            .and_then(|head| head.peel_to_commit())
            .map_err(|error| format!("Cannot commit without a parent commit: {error}"))?;
        let signature = repo
            .signature()
            .or_else(|_| Signature::now("Arcana", "arcana@localhost"))
            .map_err(|error| format!("Cannot create commit signature: {error}"))?;
        let head_ref = repo
            .head()
            .map_err(|error| format!("Cannot update the current branch: {error}"))?
            .name()
            .map(str::to_owned)
            .ok_or_else(|| "Refusing to update a detached HEAD".to_string())?;
        let oid = repo
            .commit(
                Some(&head_ref),
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

fn reset_index_to_head(repo: &Repository) -> Result<(), String> {
    let head = repo
        .head()
        .and_then(|head| head.peel_to_commit())
        .map_err(|error| format!("Cannot restore the index after refusal: {error}"))?;
    repo.reset(head.as_object(), git2::ResetType::Mixed, None)
        .map_err(|error| format!("Cannot restore the index after refusal: {error}"))
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
        .unwrap_or_default();
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
                    !prefix.is_empty()
                        && scope.ends_with(')')
                        && scope.len() > 1
                        && prefix.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                })
                .unwrap_or_else(|| {
                    !kind.is_empty() && kind.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                })
    });
    if !valid_subject {
        return Err("Commit subject must use Conventional Commits, such as 'fix(core): prevent trunk commits'".into());
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
        let repo = Repository::init(dir.path()).expect("init repository");
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

    #[test]
    fn commit_refuses_protected_trunk() {
        let (dir, _) = repo();
        fs::write(dir.path().join("change.txt"), "change\n").unwrap();
        let error = commit_paths(
            dir.path(),
            &["change.txt"],
            "fix(core): safe change\n\nBody.",
        )
        .unwrap_err();
        assert!(
            error.contains("protected branch 'master'")
                || error.contains("protected branch 'main'"),
            "{error}"
        );
    }

    #[test]
    fn commit_refuses_branch_named_trunk() {
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
        .unwrap_err()
        .contains("protected branch 'trunk'"));
    }

    #[test]
    fn commit_stages_only_named_files_and_commits_with_body_on_fleet_branch() {
        let (dir, repo) = repo();
        branch_to_fleet(&repo);
        fs::write(dir.path().join("change.txt"), "change\n").unwrap();
        fs::write(dir.path().join("leave.txt"), "leave unstaged\n").unwrap();
        let oid = commit_paths(
            dir.path(),
            &["change.txt"],
            "fix(core): safe change\n\nOnly the named file is included.",
        )
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
        .expect("a completed commit leaves the index ready for the next worker");
    }

    #[test]
    fn commit_refuses_pre_staged_paths_and_secret_files() {
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
        .unwrap_err()
        .contains("pre-staged changes"));
        assert!(validate_path(dir.path(), ".env")
            .unwrap_err()
            .contains("environment secret"));
        assert!(validate_path(dir.path(), "../outside.txt")
            .unwrap_err()
            .contains("unsafe path"));
    }

    #[test]
    fn commit_refuses_redactable_credentials_in_diff_and_restores_index() {
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
        .unwrap_err();
        assert!(error.contains("credential or sensitive value"), "{error}");
        let index = repo.index().unwrap();
        assert!(index_changed_paths(&repo, &index).unwrap().is_empty());
    }

    #[test]
    fn commit_message_requires_conventional_subject_and_body() {
        assert!(validate_commit_message("did stuff\n\nBody").is_err());
        assert!(validate_commit_message("fix(core): safe change").is_err());
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
