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

    /// Lines with comments stripped, so a commented-out step never satisfies a
    /// "this check is present" assertion.
    fn uncommented(source: &str) -> String {
        source
            .lines()
            .map(|l| match l.find('#') {
                Some(i) => &l[..i],
                None => l,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// YAML 1.1 truthy spellings. `continue-on-error: yes` is as permissive as
    /// `: true`, and an expression like `${{ … }}` is unknowable statically, so
    /// it counts as permissive too.
    fn is_truthy(value: &str) -> bool {
        let v = value.trim().trim_matches(['"', '\'']).to_ascii_lowercase();
        matches!(v.as_str(), "true" | "yes" | "on" | "y" | "1") || v.starts_with("${{")
    }

    /// TG0-02 `[governance]` — no correctness job may be permitted to fail.
    ///
    /// Four `continue-on-error: true` flags sat on `cargo fmt` and `cargo test`
    /// for both Rust crates. The entire defect register accumulated behind the
    /// resulting green tick.
    ///
    /// Matches every truthy spelling, not just `true`, and every common way of
    /// neutralising a step — the first version of this test checked one
    /// spelling each, which the P0 audit (F-18) showed leaves the idiomatic
    /// re-break passing.
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

            for (idx, raw) in source.lines().enumerate() {
                let line = match raw.find('#') {
                    Some(i) => &raw[..i],
                    None => raw,
                };
                let trimmed = line.trim().trim_start_matches("- ").trim();
                let at = idx + 1;

                if let Some(value) = trimmed.strip_prefix("continue-on-error:") {
                    if is_truthy(value) {
                        findings.push(format!("{name}:{at}: {}", trimmed.trim()));
                    }
                }
                // Neutralising a command so a failure never reaches the runner.
                for escape in ["|| true", "|| :", "|| exit 0", "set +e"] {
                    if trimmed.contains(escape) {
                        findings.push(format!("{name}:{at}: {} ({escape})", trimmed.trim()));
                    }
                }
                // `if: always()` on a step makes a preceding failure survive.
                if trimmed.starts_with("if:") && trimmed.contains("always()") {
                    findings.push(format!("{name}:{at}: {}", trimmed.trim()));
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
    ///
    /// Asserts the ABSENCE of any branch/event condition rather than one
    /// spelling of it: the first version required a single line containing
    /// `github.event_name`, `push` AND `refs/heads/main`, so the far more
    /// common `if: github.ref == 'refs/heads/main'` passed it (F-18).
    #[test]
    fn tg0_02_build_smokes_are_not_gated_to_a_branch_or_event() {
        let ci = repo_root().join(".github").join("workflows").join("ci.yml");
        let source = std::fs::read_to_string(&ci).expect("read ci.yml");

        for (idx, raw) in source.lines().enumerate() {
            let line = match raw.find('#') {
                Some(i) => &raw[..i],
                None => raw,
            };
            let trimmed = line.trim();
            if !trimmed.starts_with("if:") {
                continue;
            }
            // `runner.os == 'Linux'` style platform conditions are fine — they
            // select a step within a job, they do not skip the job.
            let gates_on_branch_or_event = trimmed.contains("github.ref")
                || trimmed.contains("github.event_name")
                || trimmed.contains("refs/heads");
            assert!(
                !gates_on_branch_or_event,
                "ci.yml:{}: a job or step is gated on the branch or event, so it does \
                 not run on pull requests:\n  {trimmed}",
                idx + 1
            );
        }

        // `on:` must include pull_request, or none of the above matters.
        assert!(
            uncommented(&source).contains("pull_request:"),
            "ci.yml does not trigger on pull_request at all"
        );
    }

    /// TG0-02 (third half) — every check spec `10 §6` requires must be present,
    /// as a real command rather than an alias for another check.
    #[test]
    fn tg0_02_every_required_check_is_present_and_distinct() {
        let ci = repo_root().join(".github").join("workflows").join("ci.yml");
        // Comments stripped: a commented-out step used to satisfy this (F-18).
        let source = uncommented(&std::fs::read_to_string(&ci).expect("read ci.yml"));

        for required in [
            // Workspace-wide forms specifically. The workspace has three
            // members and these jobs used to check one, leaving `src-shared`
            // unformatted and unlinted by any job (F-15).
            "cargo fmt --all",
            "cargo clippy --workspace",
            "cargo test --workspace",
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

        // `lint` must not resolve to the typecheck by any route. The first
        // version checked only for a literal `tsc`, so `"lint": "npm run
        // typecheck"` restored the exact defect and passed (F-18).
        let package_json =
            std::fs::read_to_string(repo_root().join("package.json")).expect("read package.json");
        let parsed: serde_json::Value =
            serde_json::from_str(&package_json).expect("parse package.json");
        let lint = parsed["scripts"]["lint"]
            .as_str()
            .expect("package.json must define a lint script");
        assert!(
            lint.contains("eslint"),
            "`npm run lint` is `{lint}`, which does not run ESLint. Before P0 it was \
             `tsc --noEmit`, so CI ran the typecheck twice and called one of them \
             ESLint."
        );
        assert!(
            !lint.contains("tsc") && !lint.contains("typecheck"),
            "`npm run lint` is `{lint}` — it resolves to the typecheck"
        );
    }

    /// TG0-03 `[security]`, static half — the renderer must not be able to
    /// reach anything off-device.
    ///
    /// An ALLOW-LIST, not a blocklist. The first version listed forbidden
    /// hostnames, so `https://*.anthropic.com` — a wildcard granting the
    /// renderer direct provider access — contained none of them and passed
    /// (F-18). The invariant is "no external origin", so that is what is
    /// asserted: every source in `connect-src` must be one of the local
    /// schemes AutoDev's own IPC needs.
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

        // Everything the renderer legitimately needs to talk to is local: the
        // Tauri IPC bridge and its own origin. Nothing else.
        const ALLOWED: &[&str] = &["connect-src", "'self'", "ipc:", "http://ipc.localhost"];

        for source in connect_src.split_whitespace() {
            assert!(
                ALLOWED.contains(&source),
                "CSP connect-src permits `{source}`. The renderer must reach nothing \
                 off-device: anything it can call directly needs its credential in the \
                 renderer, which defeats BYOK (INV-12). If a new local scheme is truly \
                 needed, add it to ALLOWED deliberately.\n  {connect_src}"
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
        // `.env` holding provider keys is expected in a working tree — that is
        // how AutoDev is developed. What must never happen is it entering the
        // index or a shipped artifact.
        let mut checked_any = false;
        for forbidden in [
            ".env",
            "src-tauri/dump.rdb",
            "dump.rdb",
            "autodev-update-private.pem",
        ] {
            let tracked = std::process::Command::new("git")
                .args(["ls-files", "--error-unmatch", forbidden])
                .current_dir(&root)
                .output();

            if let Ok(out) = tracked {
                checked_any = true;
                assert!(
                    !out.status.success(),
                    "{forbidden} is TRACKED by git. It must never be committed — \
                     remove it from the index with `git rm --cached {forbidden}`."
                );
            }
        }
        assert!(
            checked_any,
            "git was not runnable, so nothing was actually checked — this test must \
             not pass by being unable to look"
        );

        // .gitignore must keep them out rather than relying on nobody adding them.
        let gitignore = std::fs::read_to_string(root.join(".gitignore")).expect("read .gitignore");
        for pattern in [".env", "dump.rdb", "*.pem"] {
            assert!(
                gitignore.contains(pattern),
                ".gitignore does not exclude {pattern}"
            );
        }
    }
}
