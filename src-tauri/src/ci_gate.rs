// Phase-P0 spec-invariant gates that assert on repository files rather than on
// runtime behaviour.
//
// These are `TG0-02`, the static half of `TG0-03`, and the committed-tree half
// of `TG0-04` from TESTING_GATES.md. They live in the test binary so they run
// with `cargo test --all-targets`, which CI now uses — previously it ran
// `--lib`, so anything outside the library target was silently skipped while
// still reporting green.
//
// Each one previously existed only as a sentence in a document and a human
// promising they had checked. INV-4 applies to the gate documents themselves.
//
// Compiled only under `cfg(test)`.

use std::path::{Path, PathBuf};

/// Repository root — one level above `src-tauri/`.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri always has a parent")
        .to_path_buf()
}

fn workflow_files() -> Vec<PathBuf> {
    let dir = repo_root().join(".github").join("workflows");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("yml") | Some("yaml")
            )
        })
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TG0-02 `[governance]` — no correctness job may be permitted to fail.
    ///
    /// Four `continue-on-error: true` flags sat on `cargo fmt` and `cargo test`
    /// for both Rust crates. The entire defect register accumulated behind the
    /// resulting green tick.
    #[test]
    fn tg0_02_no_workflow_permits_a_correctness_job_to_fail() {
        let files = workflow_files();
        assert!(
            !files.is_empty(),
            "no workflow files found — is .github/workflows present?"
        );

        let mut findings = Vec::new();
        for file in &files {
            let source = std::fs::read_to_string(file).expect("read workflow");
            let name = file
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();

            for (idx, line) in source.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') {
                    continue;
                }
                if trimmed.starts_with("continue-on-error") && trimmed.contains("true") {
                    findings.push(format!("{name}:{}: {trimmed}", idx + 1));
                }
                if trimmed.ends_with("|| true") {
                    findings.push(format!("{name}:{}: {trimmed}", idx + 1));
                }
            }
        }

        assert!(
            findings.is_empty(),
            "these CI steps are allowed to fail without failing the build:\n  {}",
            findings.join("\n  ")
        );
    }

    /// TG0-02 (second half) — the build smokes must run on pull requests.
    ///
    /// They used to be gated to `push` on `main`, so a PR could merge without
    /// any platform having compiled the application.
    #[test]
    fn tg0_02_build_smokes_are_not_gated_to_pushes_on_main() {
        let ci = repo_root().join(".github").join("workflows").join("ci.yml");
        let source = std::fs::read_to_string(&ci).expect("read ci.yml");

        for (idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            let gates_to_main_push = trimmed.starts_with("if:")
                && trimmed.contains("github.event_name")
                && trimmed.contains("push")
                && trimmed.contains("refs/heads/main");
            assert!(
                !gates_to_main_push,
                "ci.yml:{}: a job is gated to pushes on main, so pull requests never run it:\n  {trimmed}",
                idx + 1
            );
        }
    }

    /// TG0-02 (third half) — every row of spec `10 §6`'s target column must
    /// have a job, and it must be a real command rather than an alias for
    /// another check. `npm run lint` was literally `tsc --noEmit`, so CI ran
    /// the typecheck twice and reported one of them as ESLint.
    #[test]
    fn tg0_02_every_required_check_is_present_and_distinct() {
        let ci = repo_root().join(".github").join("workflows").join("ci.yml");
        let source = std::fs::read_to_string(&ci).expect("read ci.yml");

        for required in [
            "cargo fmt",
            "cargo clippy",
            "cargo test",
            "npm run typecheck",
            "npm run lint",
            "npm test",
        ] {
            assert!(
                source.contains(required),
                "ci.yml has no `{required}` step; spec 10 §6 requires it"
            );
        }

        for os in ["windows-latest", "ubuntu-22.04", "macos-14"] {
            assert!(source.contains(os), "ci.yml never builds on {os}");
        }

        // `lint` must not be an alias for the typecheck.
        let package_json =
            std::fs::read_to_string(repo_root().join("package.json")).expect("read package.json");
        let lint_is_tsc = package_json.contains("\"lint\": \"tsc")
            || package_json.contains("\"lint\": \"tsc --noEmit\"");
        assert!(
            !lint_is_tsc,
            "package.json's `lint` script is the typecheck again. CI then runs the \
             same command twice and calls one of them ESLint."
        );
    }

    /// TG0-03 `[security]`, static half — the renderer must not be able to
    /// reach a provider or GitHub directly.
    ///
    /// If it could, a key would have to live in the renderer to use it, which
    /// is exactly what BYOK and INV-12 exist to prevent.
    #[test]
    fn tg0_03_csp_allows_no_external_origin_from_the_renderer() {
        let conf =
            std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json"))
                .expect("read tauri.conf.json");
        let parsed: serde_json::Value = serde_json::from_str(&conf).expect("parse tauri.conf.json");

        let csp = parsed["app"]["security"]["csp"]
            .as_str()
            .expect("tauri.conf.json must define a CSP");

        let connect_src = csp
            .split(';')
            .map(str::trim)
            .find(|d| d.starts_with("connect-src"))
            .expect("CSP must define connect-src");

        for forbidden in [
            "api.github.com",
            "api.anthropic.com",
            "api.openai.com",
            "generativelanguage.googleapis.com",
            "api.x.ai",
            "*",
        ] {
            assert!(
                !connect_src.contains(forbidden),
                "CSP connect-src reaches {forbidden}. A renderer that can call an \
                 external API directly needs the credential in the renderer, which \
                 defeats BYOK (INV-12).\n  {connect_src}"
            );
        }
    }

    /// TG0-04 `[security]`, committed-tree half — no secret or stray artifact
    /// in the repository.
    ///
    /// The artifact half (scanning a built installer) needs a bundle and runs
    /// in CI; this covers what can be asserted from a checkout. `dump.rdb` is
    /// named explicitly because 175 KB of one was committed once already.
    #[test]
    fn tg0_04_no_secret_or_stray_artifact_is_committed() {
        let root = repo_root();

        // The invariant is "never COMMITTED", not "never exists". A developer
        // `.env` holding provider keys for local testing is expected to be
        // present in a working tree — that is how AutoDev is developed. What
        // must never happen is it entering history or a shipped artifact.
        for forbidden in [".env", "src-tauri/dump.rdb", "dump.rdb"] {
            let tracked = std::process::Command::new("git")
                .args(["ls-files", "--error-unmatch", forbidden])
                .current_dir(&root)
                .output();

            if let Ok(out) = tracked {
                assert!(
                    !out.status.success(),
                    "{forbidden} is TRACKED by git. It must never be committed — \
                     remove it from the index with `git rm --cached {forbidden}`."
                );
            }
            // A git failure (not a repo, git absent) leaves this unasserted
            // rather than falsely green; the CI job always has both.
        }

        // .gitignore must keep them out rather than relying on nobody adding them.
        let gitignore = std::fs::read_to_string(root.join(".gitignore")).expect("read .gitignore");
        for pattern in [".env", "dump.rdb"] {
            assert!(
                gitignore.contains(pattern),
                ".gitignore does not exclude {pattern}"
            );
        }
    }
}
