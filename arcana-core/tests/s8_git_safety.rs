use arcana_core::agent::builtin_tools::bash::BashTool;
use arcana_core::agent::builtin_tools::git_commit::GitCommitTool;
use arcana_core::agent::permission::PermissionMode;
use arcana_core::agent::tool_trait::{Tool, ToolContext};
use git2::{Repository, Signature};
use serde_json::json;
use std::fs;
use std::path::Path;

fn context(path: &Path) -> ToolContext {
    ToolContext {
        session_id: "s8-git-safety".into(),
        working_dir: path.to_string_lossy().into_owned(),
        call_id: "s8-git-safety-call".into(),
        permission_mode: PermissionMode::Autonomous,
        depth: 0,
        agent_role: "worker-backend".into(),
    }
}

fn init_repo(dir: &Path) -> (Repository, String) {
    let repo = Repository::init(dir).expect("initialize isolated repository");
    let mut config = repo.config().expect("repository config");
    config
        .set_str("user.name", "Arcana Smoke")
        .expect("set test commit name");
    config
        .set_str("user.email", "arcana-smoke@example.invalid")
        .expect("set test commit email");
    fs::write(dir.join("app.rs"), "fn main() {}\n").expect("write base file");
    let mut index = repo.index().expect("repository index");
    index
        .add_path(Path::new("app.rs"))
        .expect("stage base file");
    index.write().expect("write base index");
    let tree_id = index.write_tree().expect("write base tree");
    let tree = repo.find_tree(tree_id).expect("read base tree");
    let signature =
        Signature::now("Arcana Smoke", "arcana-smoke@example.invalid").expect("test signature");
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "chore: initialize smoke repository\n\nCreate an isolated commit base.",
        &tree,
        &[],
    )
    .expect("create base commit");
    drop(tree);
    let trunk = repo.head().unwrap().shorthand().unwrap().to_owned();
    (repo, trunk)
}

#[test]
fn s8_06_host_git_tool_refuses_trunk_and_commits_only_explicit_safe_files() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    let sandbox = tempfile::tempdir().expect("isolated Git sandbox");
    let (repo, trunk) = init_repo(sandbox.path());
    assert!(matches!(trunk.as_str(), "main" | "master"));
    let trunk_before = repo.head().unwrap().target().unwrap();

    fs::write(sandbox.path().join("app.rs"), "fn main() { safe(); }\n").expect("write task change");
    fs::create_dir_all(sandbox.path().join("target")).expect("create build output");
    fs::write(sandbox.path().join("target/app.bin"), "generated")
        .expect("write untracked build output");
    fs::write(
        sandbox.path().join(".env"),
        "GOOGLE_API_KEY=not-a-real-secret\n",
    )
    .expect("write ignored-secret probe");

    let ctx = context(sandbox.path());
    let bash = BashTool;
    let blocked = runtime.block_on(bash.execute(json!({"command":"git add -A"}), &ctx));
    assert!(blocked.is_error, "Bash must block direct staging");
    assert!(
        blocked.output.contains("git_commit"),
        "give the safe path: {}",
        blocked.output
    );

    let tool = GitCommitTool;
    let trunk_refused = runtime.block_on(tool.execute(
        json!({
            "paths": ["app.rs"],
            "message": "fix(core): protect trunk\n\nA worker must not update the trunk branch."
        }),
        &ctx,
    ));
    assert!(trunk_refused.is_error, "host must refuse commits on trunk");
    assert!(trunk_refused.output.contains("protected branch"));
    assert_eq!(repo.head().unwrap().target(), Some(trunk_before));
    assert_eq!(
        repo.status_file(Path::new("app.rs")).unwrap(),
        git2::Status::WT_MODIFIED,
        "refused trunk commit leaves the user's change unstaged"
    );

    let git_branch = repo
        .branch(
            "arcana/fleet",
            &repo.head().unwrap().peel_to_commit().unwrap(),
            false,
        )
        .expect("create fleet branch");
    repo.set_head(git_branch.get().name().unwrap())
        .expect("select shared fleet branch");

    let result = runtime.block_on(tool.execute(
        json!({
            "paths": ["app.rs"],
            "message": "fix(core): protect task commit\n\nThe host stages only this file and preserves the trunk."
        }),
        &ctx,
    ));
    assert!(!result.is_error, "host commit failed: {}", result.output);
    assert_eq!(repo.head().unwrap().shorthand(), Some("arcana/fleet"));
    assert_eq!(
        repo.find_commit(repo.head().unwrap().target().unwrap())
            .unwrap()
            .parent(0)
            .unwrap()
            .id(),
        trunk_before
    );
    assert_eq!(
        repo.status_file(Path::new(".env")).unwrap(),
        git2::Status::WT_NEW
    );
    assert_eq!(
        repo.status_file(Path::new("target/app.bin")).unwrap(),
        git2::Status::WT_NEW
    );
    let committed_tree = repo.head().unwrap().peel_to_tree().unwrap();
    assert!(committed_tree.get_path(Path::new("app.rs")).is_ok());
    assert!(committed_tree.get_path(Path::new(".env")).is_err());
    assert!(committed_tree
        .get_path(Path::new("target/app.bin"))
        .is_err());
    assert_eq!(
        repo.find_branch(&trunk, git2::BranchType::Local)
            .unwrap()
            .get()
            .target(),
        Some(trunk_before),
        "the selected trunk reference must remain unchanged"
    );
    assert!(!repo
        .find_commit(trunk_before)
        .unwrap()
        .tree()
        .unwrap()
        .get_path(Path::new(".env"))
        .is_ok());
}
