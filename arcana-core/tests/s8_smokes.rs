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
        "arcana-s8-{tag}-{}-{:?}",
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
    let script = "git switch -c arcana/fleet 2>/dev/null || git switch arcana/fleet";
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
        "arcana/fleet",
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
        "arcana/fleet",
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
    let own = arcana_core::git::diff::branch_diff(&repo_str, "HEAD~1", "HEAD")
        .expect("HEAD~1..HEAD must resolve");
    assert!(
        own.contains("work()"),
        "the cleanup check cannot see the worker's own change: {own}"
    );

    // And the old base is vacuous, which is why the check passed everything.
    let vacuous = arcana_core::git::diff::branch_diff(&repo_str, &trunk, &trunk)
        .expect("trunk..trunk must resolve");
    assert!(
        vacuous.trim().is_empty(),
        "the pre-P8-38 comparison was expected to be empty and was not, so the \
         reason given for changing it is wrong: {vacuous}"
    );

    let _ = std::fs::remove_dir_all(&repo);
}

/// `S8-01` — a clean checkout needs no submodule initialisation.
///
/// Clones the repository into a sandbox with a plain `git clone` — **no
/// `--recurse-submodules`** — and asserts the result is complete: no submodule
/// configuration, no submodule directory, and every workspace member present.
///
/// ⚠ It does not compile the clone. A fresh `cargo build` there would re-fetch
/// and rebuild the whole dependency graph for a property this already
/// establishes; the compile half is what `cargo check --workspace` proves on
/// this tree, and the two together are the claim. Saying so is the point —
/// a smoke that quietly checks less than its name implies is worse than one
/// that states its boundary.
#[test]
fn s8_01_a_clean_checkout_needs_no_submodule_step() {
    assert!(
        Command::new("git").arg("--version").output().is_ok(),
        "git is not on PATH, so this smoke cannot report anything (`L-C9`)."
    );

    // The repository root: two levels up from `arcana-core/tests/`.
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("arcana-core always has a parent")
        .to_path_buf();
    assert!(
        repo_root.join(".git").exists(),
        "{repo_root:?} is not a git repository, so nothing below is measured"
    );

    let sandbox = std::env::temp_dir().join(format!(
        "arcana-s8-01-{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let dest = sandbox.join("clone");
    std::fs::create_dir_all(&sandbox).expect("sandbox");

    // A plain clone. No `--recurse-submodules`, deliberately.
    let out = Command::new("git")
        .args(["clone", "--depth", "1", "--quiet"])
        .arg(&repo_root)
        .arg(&dest)
        .output()
        .expect("git clone must start");
    assert!(
        out.status.success(),
        "the clone failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // ── no submodule step ────────────────────────────────────────────────────
    assert!(
        !dest.join(".gitmodules").exists(),
        "`.gitmodules` is in a clean checkout, so `git submodule update --init` \
         is back on the critical path for every new contributor"
    );
    assert!(
        !dest.join("submodules").exists(),
        "a `submodules/` directory is in a clean checkout"
    );
    let status = git_ok(&dest, &["submodule", "status"]);
    assert!(
        status.trim().is_empty(),
        "the clean checkout declares submodules: {status}"
    );

    // ── and the checkout is COMPLETE ─────────────────────────────────────────
    //
    // "No submodule step" is only good news if nothing is missing as a result.
    // Every workspace member's manifest must be present in the clone.
    let workspace = std::fs::read_to_string(dest.join("Cargo.toml")).expect("workspace manifest");
    let members: Vec<String> = workspace
        .split("members")
        .nth(1)
        .and_then(|r| r.split(']').next())
        .map(|list| {
            list.split('"')
                .filter(|p| !p.trim().is_empty() && !p.contains('[') && !p.contains(','))
                .map(|p| p.to_string())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        members.len() >= 3,
        "only {} workspace member(s) parsed from Cargo.toml, so this half \
         checks almost nothing: {members:?}",
        members.len()
    );
    for m in &members {
        assert!(
            dest.join(m).join("Cargo.toml").exists(),
            "workspace member `{m}` has no manifest in a clean checkout"
        );
    }
    assert!(
        dest.join("package.json").exists() && dest.join("src/App.tsx").exists(),
        "the front end is incomplete in a clean checkout"
    );

    let _ = std::fs::remove_dir_all(&sandbox);
}

/// `S8-02` (the process half) — nothing in the shipped tree spawns a Python
/// interpreter for Arcana's own operation.
///
/// ⚠ **The claim is narrower than "no Python", and that is the whole
/// difficulty.** `agent/builtin_tools/validate.rs` runs `python3`, `mypy` and
/// `pyright` — on the **user's** files, because Arcana can work on a Python
/// project. A check written against the token `python` would flag that
/// capability as a violation and invite someone to delete it. The property is
/// that no *start-up or agent-loop* path spawns an interpreter of its own.
///
/// The live half of `S8-02` — the app starts, completes an agent turn, and a
/// process listing shows no Python — needs a provider key and a desktop
/// session, and is not run from here.
#[test]
fn s8_02_no_startup_or_agent_path_spawns_an_interpreter() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent")
        .to_path_buf();

    // The paths that run without the user asking for anything.
    let startup: &[&str] = &[
        "src-tauri/src/main.rs",
        "src-tauri/src/lib.rs",
        "arcana-core/src/agent/engine.rs",
        "arcana-core/src/agent/tool_executor.rs",
        "arcana-core/src/recipes/runner.rs",
        "arcana-core/src/recipes/scheduler.rs",
        "arcana-core/src/subagent/native_runner.rs",
    ];

    let mut read = 0usize;
    for rel in startup {
        let p = root.join(rel);
        let body = std::fs::read_to_string(&p).unwrap_or_default();
        assert!(
            !body.trim().is_empty(),
            "{rel} did not read, so this smoke would pass over it silently"
        );
        read += 1;
        for needle in [
            "Command::new(\"python",
            "Command::new(\"uv\"",
            "Command::new(\"py\"",
            "spawn_sidecar",
        ] {
            assert!(
                !body.contains(needle),
                "{rel} spawns an interpreter of Arcana's own ({needle}). \
                 Running the user's Python tooling is a capability; starting \
                 one to run Arcana is the thing `P8` removed."
            );
        }
    }
    assert_eq!(read, startup.len(), "not every start-up path was read");

    // And the bundle carries no payload to start.
    let conf = std::fs::read_to_string(root.join("src-tauri/tauri.conf.json"))
        .expect("tauri.conf.json");
    assert!(
        conf.contains("\"resources\": []"),
        "the bundle declares resources again. A release ships the binary and \
         its icons; anything else here is a runtime being shipped alongside it."
    );
    assert!(
        !conf.contains("externalBin"),
        "the bundle declares an external binary"
    );
}
