// S8 · the P8 smoke checklist — the ones P8-E owns.
//
// `S8-01` (clean checkout, no submodule step), `S8-02` (no Python process),
// `S8-03` (installer contents) and `S8-04` (memory with the MCP server absent)
// are the deletion phase's smokes and are run against a real checkout and a
// real installer, not from here.
//
// `S8-05` is `P8-38`'s. It is a smoke rather than a gate because `TG8-06`
// proves the recipes *say* the right thing, and saying it is not the same as
// the procedure working: the commit protocol is a sequence of real `git`
// invocations that has to survive a real repository, a `||` fallback, and two
// workers arriving at the same moment.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Run a git command in `dir` and return `(exit_ok, stdout_trimmed)`.
fn git(dir: &Path, args: &[&str]) -> (bool, String) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("git {args:?} failed to start: {e}"));
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
    )
}

fn git_ok(dir: &Path, args: &[&str]) -> String {
    let (ok, out) = git(dir, args);
    assert!(ok, "git {args:?} failed in {}", dir.display());
    out
}

/// A throwaway repository with one commit on its default trunk.
///
/// ⚠ One sandbox per test, under `std::env::temp_dir()`. `L-30`: three P4 tests
/// shared a temp directory, each removed it, and they passed alone and failed
/// in the full suite.
fn sandbox_repo(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "autodev-s8-{tag}-{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("sandbox");

    git_ok(&dir, &["init"]);
    git_ok(&dir, &["config", "user.email", "smoke@example.invalid"]);
    git_ok(&dir, &["config", "user.name", "S8 Smoke"]);
    git_ok(&dir, &["config", "commit.gpgsign", "false"]);
    std::fs::write(dir.join("app.rs"), "fn main() {}\n").unwrap();
    git_ok(&dir, &["add", "app.rs"]);
    git_ok(&dir, &["commit", "-m", "initial"]);
    dir
}

/// The exact branch step the recipes give the worker, run through a shell the
/// way `bash()` runs it.
fn run_c1(dir: &Path) -> bool {
    let script = "git switch -c autodev/fleet 2>/dev/null || git switch autodev/fleet";
    // `sh` on every platform. Git for Windows ships one, and the recipe text is
    // a POSIX one-liner — the `||` fallback and the `2>/dev/null` are the two
    // parts of C1 that a shell has to actually honour for the step to be
    // convergent, so running it through anything else would prove nothing.
    Command::new("sh")
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// `S8-05` — a worker's commit lands on a branch, stages only what it named,
/// and leaves the trunk where it found it.
///
/// 🔴 **The defect, as found.** All five worker recipes ran
/// `bash('git add -A && git commit …')` with **no branch created**. Every fleet
/// commit therefore landed on whatever the user had checked out, staged
/// everything in the tree, and was never read before it was made.
#[test]
fn s8_05_a_worker_commit_branches_stages_narrowly_and_spares_the_trunk() {
    assert!(
        Command::new("git").arg("--version").output().is_ok(),
        "git is not on PATH, so this smoke cannot report anything. A smoke that cannot look must say so rather than pass (`L-C9`)."
    );

    let repo = sandbox_repo("05");
    let trunk = git_ok(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert!(
        trunk == "main" || trunk == "master",
        "a fresh repo did not start on a trunk, it started on {trunk} — the \
         protocol's C1 check keys on exactly these two names"
    );
    let trunk_before = git_ok(&repo, &["rev-parse", "HEAD"]);

    // ── C1, run TWICE: the second worker must join, not fail ─────────────────
    assert!(run_c1(&repo), "C1 failed for the first worker");
    assert_eq!(
        git_ok(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "autodev/fleet",
        "the first worker did not end up on the fleet branch"
    );

    // Simulate a peer that ran C1 a moment later. All workers share ONE working
    // directory (`recipes/runner.rs`), so the second arrival is the normal case,
    // not the edge case — and `switch -c` alone would have failed for it.
    assert!(
        run_c1(&repo),
        "C1 is not convergent: the second worker to run it failed, so a peer \
         arriving after the branch exists would abort its commit step"
    );
    assert_eq!(
        git_ok(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "autodev/fleet",
        "the second worker did not end up on the fleet branch"
    );

    // ── C2: name the paths ───────────────────────────────────────────────────
    std::fs::write(repo.join("app.rs"), "fn main() { work(); }\n").unwrap();
    // What a blind stage sweeps up: build output, and a file holding a secret.
    std::fs::create_dir_all(repo.join("target")).unwrap();
    std::fs::write(repo.join("target").join("app.bin"), "binary").unwrap();
    std::fs::write(repo.join(".env"), "ANTHROPIC_API_KEY=sk-ant-not-a-real-key\n").unwrap();

    git_ok(&repo, &["add", "app.rs"]);
    let staged = git_ok(&repo, &["diff", "--cached", "--name-only"]);
    let staged: Vec<&str> = staged.lines().collect();
    assert_eq!(
        staged,
        vec!["app.rs"],
        "naming the path staged more than the path: {staged:?}"
    );

    // ── the counterfactual: what the deleted instruction would have done ─────
    //
    // This is the half that shows the defect was real rather than theoretical.
    //
    // ⚠ The claim is **"everything untracked that no ignore rule covers"**, not
    // "specifically your `.env`" — a repository that ignores `.env` was never
    // exposed through that particular file. The danger is the general one: a
    // worker running `git add -A` stages whatever the user's ignore rules did
    // not anticipate, and it cannot know what those rules cover. Both files are
    // asserted so the smoke stands on the general property.
    let probe = sandbox_repo("05-counterfactual");
    std::fs::write(probe.join("app.rs"), "fn main() { work(); }\n").unwrap();
    std::fs::create_dir_all(probe.join("target")).unwrap();
    std::fs::write(probe.join("target").join("app.bin"), "binary").unwrap();
    std::fs::write(probe.join(".env"), "ANTHROPIC_API_KEY=sk-ant-not-a-real-key\n").unwrap();
    git_ok(&probe, &["add", "-A"]);
    let swept = git_ok(&probe, &["diff", "--cached", "--name-only"]);
    let swept_paths: Vec<&str> = swept.lines().collect();
    for expected in [".env", "target/app.bin"] {
        assert!(
            swept_paths.contains(&expected),
            "the counterfactual did not reproduce the defect, so this smoke is \
             not measuring what it claims: `git add -A` staged {swept_paths:?} \
             and {expected} is not among them"
        );
    }
    let _ = std::fs::remove_dir_all(&probe);

    // ── C3: the staged diff is readable before the commit ────────────────────
    let cached = git_ok(&repo, &["diff", "--cached"]);
    assert!(
        cached.contains("work()"),
        "C3 shows the agent nothing to review: {cached}"
    );
    assert!(
        !cached.contains("sk-ant-"),
        "the staged diff carries a credential — C2 did not hold"
    );

    // ── C4 ───────────────────────────────────────────────────────────────────
    git_ok(&repo, &["commit", "-m", "feat(app): call work()"]);

    // ── the trunk is where the worker found it ───────────────────────────────
    let trunk_after = git_ok(&repo, &["rev-parse", &trunk]);
    assert_eq!(
        trunk_before, trunk_after,
        "the worker's commit moved {trunk}. Before P8-38 this is exactly where \
         every fleet commit landed."
    );

    // ── the cleanup worker's safety check now has a patch to judge ───────────
    //
    // Run through the shipped resolver, which is what `analyze_diff` calls.
    let repo_str = repo.to_string_lossy().to_string();
    let own = autodev_core::git::diff::branch_diff(&repo_str, "HEAD~1", "HEAD")
        .expect("HEAD~1..HEAD must resolve");
    assert!(
        own.contains("work()"),
        "the cleanup check cannot see the worker's own change: {own}"
    );

    // And the old base is vacuous, which is why the check passed everything.
    let vacuous = autodev_core::git::diff::branch_diff(&repo_str, &trunk, &trunk)
        .expect("trunk..trunk must resolve");
    assert!(
        vacuous.trim().is_empty(),
        "the pre-P8-38 comparison was expected to be empty and was not, so the \
         reason given for changing it is wrong: {vacuous}"
    );

    let _ = std::fs::remove_dir_all(&repo);
}
