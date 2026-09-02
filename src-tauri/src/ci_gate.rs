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

    /// A declared bundle resource must actually be IN the repository.
    ///
    /// `bundle.resources` listed `resources/sidecars` — 225 MB of Python that
    /// exists only on a developer machine and is untracked. Every clean
    /// checkout therefore failed `tauri_build::build()` with
    /// `resource path 'resources/sidecars' doesn't exist`, so CI was
    /// structurally incapable of passing and had been red on `main` for months.
    ///
    /// A gate that can never pass carries exactly as little signal as one that
    /// can never fail, and it is worse in one way: it trains everyone to ignore
    /// red. Found by actually running CI after the P0 gate work.
    #[test]
    fn every_declared_bundle_resource_exists_and_is_tracked() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let conf = std::fs::read_to_string(manifest.join("tauri.conf.json"))
            .expect("read tauri.conf.json");
        let parsed: serde_json::Value = serde_json::from_str(&conf).expect("parse tauri.conf.json");

        let mut declared: Vec<String> = Vec::new();
        for key in ["resources", "externalBin"] {
            match &parsed["bundle"][key] {
                serde_json::Value::Array(items) => {
                    declared.extend(items.iter().filter_map(|v| v.as_str().map(str::to_owned)));
                }
                serde_json::Value::Object(map) => {
                    declared.extend(map.keys().cloned());
                }
                _ => {}
            }
        }

        for rel in declared {
            let path = manifest.join(&rel);
            assert!(
                path.exists(),
                "tauri.conf.json declares bundle path `{rel}`, which does not exist. The build fails on any machine without it."
            );

            // Existing locally is not enough — CI checks out the repository.
            let tracked = std::process::Command::new("git")
                .args(["ls-files", "--error-unmatch", &rel])
                .current_dir(manifest)
                .output();
            if let Ok(out) = tracked {
                assert!(
                    out.status.success(),
                    "tauri.conf.json declares bundle path `{rel}`, which exists here but is NOT tracked by git. A clean checkout cannot build: this is exactly how `resources/sidecars` kept CI red."
                );
            }
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

    // ── TG-P1 · the crate boundary that makes `autodev-core` worth having ────

    /// Dependency lines in a Cargo manifest that pull in Tauri.
    ///
    /// A free function over text rather than a check welded to the real file,
    /// so it can be proven to fire. Falsifying the welded version would mean
    /// adding `tauri` to core's manifest, which stops the workspace compiling
    /// — the test would then fail for the wrong reason and prove nothing.
    fn tauri_dependency_lines(manifest: &str) -> Vec<String> {
        let mut out = Vec::new();
        for line in manifest.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let name = line.split(['=', ' ']).next().unwrap_or("").trim();
            if name == "tauri" || name.starts_with("tauri-") {
                out.push(line.to_string());
            }
        }
        out
    }

    /// Lines of Rust that reference Tauri in **code**, as `(line_number, text)`.
    ///
    /// Comments are stripped first, because core deliberately *discusses*
    /// Tauri: `events.rs` explains at length why the `AppHandle` it replaced
    /// does not belong here. A gate that failed on prose would pressure authors
    /// into deleting the explanation.
    fn tauri_code_references(source: &str) -> Vec<(usize, String)> {
        let mut out = Vec::new();
        for (i, raw) in source.lines().enumerate() {
            let line = raw.trim();
            if line.starts_with("//") || line.starts_with('*') || line.starts_with("/*") {
                continue;
            }
            let code = line.split("//").next().unwrap_or("");
            if code.contains("tauri::") || code.contains("use tauri") {
                out.push((i + 1, line.to_string()));
            }
        }
        out
    }

    fn core_rust_files() -> Vec<PathBuf> {
        fn walk(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
                    out.push(p);
                }
            }
        }
        let mut files = Vec::new();
        walk(&repo_root().join("autodev-core/src"), &mut files);
        files
    }

    /// `autodev-core` must not depend on Tauri.
    ///
    /// This is the whole point of the crate: `airsctl` links the same agent
    /// runtime the desktop app does, and it cannot link a UI framework. Before
    /// P1 the runtime was unusable from the CLI because every event it emitted
    /// required a `tauri::AppHandle` (ADR-LOG-RESTRUCT-0011).
    #[test]
    fn tg_p1_01_autodev_core_does_not_depend_on_tauri() {
        let manifest = std::fs::read_to_string(repo_root().join("autodev-core/Cargo.toml"))
            .expect("autodev-core/Cargo.toml");
        let offenders = tauri_dependency_lines(&manifest);
        assert!(
            offenders.is_empty(),
            "autodev-core has taken a dependency on Tauri, which defeats the crate's purpose — airsctl links this runtime and cannot link a UI framework: {}",
            offenders.join(" | ")
        );
    }

    /// The detector actually fires. Without this, a broken regex would leave
    /// TG-P1-01 permanently and silently green.
    #[test]
    fn tg_p1_01_detector_flags_a_tauri_dependency() {
        let planted = "[dependencies]
serde = \"1\"
tauri = { version = \"2\" }
tokio = \"1\"
";
        assert_eq!(tauri_dependency_lines(planted).len(), 1);
        assert!(tauri_dependency_lines(planted)[0].starts_with("tauri"));
        // Plugins count too — they drag the framework in just as surely.
        assert_eq!(
            tauri_dependency_lines(
                "tauri-plugin-shell = \"2\"
"
            )
            .len(),
            1
        );
        // A commented-out dependency is not a dependency.
        assert!(tauri_dependency_lines(
            "# tauri = \"2\"
"
        )
        .is_empty());
        // Nor is an unrelated crate whose name merely contains the substring.
        assert!(tauri_dependency_lines(
            "nottauri = \"1\"
"
        )
        .is_empty());
    }

    /// No source file in `autodev-core` may reference `tauri` in code.
    ///
    /// The manifest check is necessary but not sufficient: a path dependency,
    /// workspace inheritance, or a re-export through another crate could
    /// reintroduce the type without a literal `tauri = ` line.
    #[test]
    fn tg_p1_02_no_core_source_file_references_tauri_in_code() {
        let files = core_rust_files();
        assert!(
            files.len() > 150,
            "expected to walk autodev-core; found {} files",
            files.len()
        );

        let src = repo_root().join("autodev-core/src");
        let mut offenders = Vec::new();
        for file in &files {
            let Ok(text) = std::fs::read_to_string(file) else {
                continue;
            };
            for (line_no, line) in tauri_code_references(&text) {
                offenders.push(format!(
                    "{}:{}: {}",
                    file.strip_prefix(&src).unwrap_or(file).display(),
                    line_no,
                    line
                ));
            }
        }
        assert!(
            offenders.is_empty(),
            "autodev-core references Tauri in code. Core depends on traits the host implements — see events::EventSink and recipes::orchestrator — never on the host's framework: {}",
            offenders.join(" | ")
        );
    }

    /// The detector fires on code and stays quiet on prose.
    ///
    /// Both halves matter. If it missed `use tauri`, the gate would be
    /// decorative; if it flagged comments, `events.rs` — which must explain the
    /// `AppHandle` it replaced — could not keep its explanation.
    #[test]
    fn tg_p1_02_detector_flags_code_but_not_prose() {
        let planted = "use tauri::AppHandle;
let x = 1;
";
        assert_eq!(tauri_code_references(planted).len(), 1);
        assert_eq!(tauri_code_references(planted)[0].0, 1);

        let qualified = "    let h: tauri::AppHandle = get();
";
        assert_eq!(tauri_code_references(qualified).len(), 1);

        let prose = "// Before P1 every emit required a `tauri::AppHandle`.
/// See tauri::Emitter for the shape this replaced.
 * tauri::AppHandle in a block comment
";
        assert!(
            tauri_code_references(prose).is_empty(),
            "prose about Tauri must not trip the gate: {:?}",
            tauri_code_references(prose)
        );

        let trailing = "let x = 1; // tauri::AppHandle mentioned after code
";
        assert!(tauri_code_references(trailing).is_empty());
    }

    // ── TG-P1-04 · one owner per table ────────────────────────────────────

    /// Files declaring `CREATE TABLE … <table>`, as repo-relative paths.
    fn table_creators(table: &str) -> Vec<String> {
        let root = repo_root();
        let mut out = Vec::new();
        for krate in ["autodev-core", "src-tauri", "src-cli", "src-shared"] {
            let src = root.join(krate).join("src");
            let mut files = Vec::new();
            collect_rust_files(&src, &mut files);
            for f in files {
                let Ok(text) = std::fs::read_to_string(&f) else {
                    continue;
                };
                // Scan production code only. Test modules hold fixture SQL —
                // including this gate's own detector test, which names the
                // very statement it looks for. In P0 the vault gate matched
                // its own source this way and reported a violation that did
                // not exist (L-C7).
                let production = text.split("#[cfg(test)]").next().unwrap_or("");
                if creates_table(production, table) {
                    out.push(
                        f.strip_prefix(&root)
                            .unwrap_or(&f)
                            .display()
                            .to_string()
                            .replace('\\', "/"),
                    );
                }
            }
        }
        out.sort();
        out
    }

    /// Whether `source` contains a `CREATE TABLE` for `table`, ignoring
    /// comments so that prose naming the table does not count as a creator.
    fn creates_table(source: &str, table: &str) -> bool {
        source.lines().any(|raw| {
            let line = raw.trim();
            if line.starts_with("//") || line.starts_with('*') || line.starts_with("///") {
                return false;
            }
            let code = line.split("//").next().unwrap_or("");
            let lower = code.to_ascii_lowercase();
            lower.contains("create table") && lower.contains(&table.to_ascii_lowercase())
        })
    }

    fn collect_rust_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_rust_files(&p, out);
            } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(p);
            }
        }
    }

    /// Every table in `config.db` must have exactly one creator (`INV-10`,
    /// `P1-15`, `P1-10`).
    ///
    /// `app_settings` had three. They agreed on the columns and disagreed on
    /// the `updated_at` trigger — only `github_mcp_commands.rs` declared it —
    /// so whether the trigger existed depended on which caller opened
    /// `config.db` first on a given machine. That is a schema decided by
    /// startup order, and it is invisible until two installs behave
    /// differently.
    ///
    /// The owner is `db/config_db.rs` rather than any one table's module,
    /// because `user_version` belongs to the **file**: three modules each
    /// running their own ladder over `config.db` would each see a version the
    /// others wrote.
    #[test]
    fn tg_p1_04_config_db_tables_have_exactly_one_creator() {
        const OWNER: &str = "autodev-core/src/db/config_db.rs";
        // All six, not the three P1-15 knew about. `checkpoints` came from
        // `git/checkpoint.rs` and `hooks`/`hook_logs` from `hooks/db.rs`; both
        // predate P1 and both were missed because the search was for the table
        // name rather than for the file. Adopted into the ladder at v2.
        for table in autodev_core::db::config_db::TABLES {
            let creators = table_creators(table);
            assert_eq!(
                creators,
                vec![OWNER.to_string()],
                "{table} must be created in exactly one place ({OWNER}); found {}: {}",
                creators.len(),
                creators.join(" | ")
            );
        }
    }

    /// The detector fires on a real creator and ignores prose, so a broken
    /// matcher cannot leave TG-P1-04 permanently green.
    #[test]
    fn tg_p1_04_detector_flags_code_but_not_prose() {
        assert!(creates_table(
            "conn.execute_batch(\"CREATE TABLE IF NOT EXISTS app_settings (\")",
            "app_settings"
        ));
        assert!(creates_table(
            "  CREATE TABLE app_settings (",
            "app_settings"
        ));
        assert!(
            !creates_table(
                "// we used to CREATE TABLE app_settings here",
                "app_settings"
            ),
            "a comment must not count as a creator"
        );
        assert!(
            !creates_table(
                "SELECT value FROM app_settings WHERE key = ?1",
                "app_settings"
            ),
            "reading a table is not creating it"
        );
    }

    // ── TG-P1-05 · one tuned connection path ──────────────────────────────

    /// Lines opening a SQLite connection without going through `open_tuned`.
    ///
    /// `open_with_flags` and `open_in_memory` are deliberately allowed: the
    /// first is the read-only backup path, the second never touches a file.
    fn untuned_open_sites(source: &str) -> Vec<String> {
        let mut out = Vec::new();
        for raw in source.lines() {
            let line = raw.trim();
            if line.starts_with("//") || line.starts_with('*') || line.starts_with("///") {
                continue;
            }
            let code = line.split("//").next().unwrap_or("");
            if code.contains("Connection::open(") && !code.contains("open_in_memory") {
                out.push(line.to_string());
            }
        }
        out
    }

    /// Every database connection in core goes through `db::open_tuned`.
    ///
    /// `journal_mode` is per-connection, and setting it at some open sites and
    /// not others is not a style question: before P1, `analytics.db`,
    /// `annotations.db` and `workers.db` were all sitting in `journal=delete`
    /// on disk, and five of the six places `graph/store.rs` opens a connection
    /// set nothing at all. Under `delete` a reader and a writer cannot proceed
    /// at once — which is the shape of nearly every access here.
    #[test]
    fn tg_p1_05_every_core_connection_is_opened_tuned() {
        let src = repo_root().join("autodev-core/src");
        let mut files = Vec::new();
        collect_rust_files(&src, &mut files);
        assert!(
            files.len() > 150,
            "expected to walk core; found {}",
            files.len()
        );

        let mut offenders = Vec::new();
        for file in &files {
            let Ok(text) = std::fs::read_to_string(file) else {
                continue;
            };
            let rel = file
                .strip_prefix(&src)
                .unwrap_or(file)
                .display()
                .to_string()
                .replace('\\', "/");
            // `db/mod.rs` is where `open_tuned` itself lives.
            if rel == "db/mod.rs" {
                continue;
            }
            // Production code only: test modules open fixtures directly.
            let production = text.split("#[cfg(test)]").next().unwrap_or("");
            for line in untuned_open_sites(production) {
                offenders.push(format!("{rel}: {line}"));
            }
        }

        assert!(
            offenders.is_empty(),
            "these open a SQLite connection without db::open_tuned, so they get whatever journal_mode the file happens to carry: {}",
            offenders.join(" | ")
        );
    }

    /// The detector fires on a real open and ignores the allowed forms, so a
    /// broken matcher cannot leave TG-P1-05 permanently green.
    #[test]
    fn tg_p1_05_detector_flags_untuned_opens_only() {
        assert_eq!(
            untuned_open_sites("    let conn = Connection::open(&path)?;").len(),
            1
        );
        assert!(
            untuned_open_sites("    let conn = Connection::open_in_memory()?;").is_empty(),
            "in-memory databases touch no file"
        );
        assert!(
            untuned_open_sites("    let c = crate::db::open_tuned(&path)?;").is_empty(),
            "the tuned path is what we want"
        );
        assert!(
            untuned_open_sites("// historically this used Connection::open(&p)").is_empty(),
            "a comment is not a call"
        );
    }

    // ── TG-P1-06 · redaction reaches every boundary ──────────────────────

    /// Every call that runs a tool must redact the result it returns.
    ///
    /// **This gate replaces a substring check that was decorative.** Its first
    /// version asserted only that each boundary file *contained the string*
    /// `security::redaction`. `tool_executor.rs` did — inside
    /// `ToolExecutor::execute`, a method with **no production caller**. The
    /// agent loop calls `execute_batch`, which redacted nothing, so raw tool
    /// output reached the model, the `messages` table and the UI while this
    /// gate and the phase's headline smoke test were both green.
    ///
    /// So it now checks the *shape of the call*, not the presence of a word:
    /// every `tool.execute(` in the executor must be wrapped in
    /// `redact_result(`. A new branch added later — a third execution path, a
    /// retry, a streaming variant — fails this gate unless it redacts.
    #[test]
    fn tg_p1_06_every_tool_execution_redacts_its_result() {
        let path = repo_root().join("autodev-core/src/agent/tool_executor.rs");
        let text = std::fs::read_to_string(&path).expect("tool_executor.rs");
        let production = text.split("#[cfg(test)]").next().unwrap_or("");

        let mut unwrapped = Vec::new();
        let mut total = 0usize;
        for (i, raw) in production.lines().enumerate() {
            let line = raw.trim();
            if line.starts_with("//") || line.starts_with('*') {
                continue;
            }
            let code = line.split("//").next().unwrap_or("");
            if !code.contains("tool.execute(") {
                continue;
            }
            total += 1;
            // The call may be wrapped on the same line, or on the line above
            // when rustfmt has split it.
            let previous = if i > 0 {
                production.lines().nth(i - 1).unwrap_or("")
            } else {
                ""
            };
            if !code.contains("redact_result(") && !previous.contains("redact_result(") {
                unwrapped.push(format!("{}: {}", i + 1, line));
            }
        }

        assert!(
            total >= 3,
            "expected at least 3 tool.execute call sites, found {total} — has the executor been restructured? This gate must be re-pointed if so."
        );
        assert!(
            unwrapped.is_empty(),
            "these tool executions do not redact their result, so raw tool output reaches the model, the messages table and the UI: {}",
            unwrapped.join(" | ")
        );
    }

    /// The detector must fire on an unwrapped call and stay quiet on a wrapped
    /// one, or the gate above is decorative in a new way.
    #[test]
    fn tg_p1_06_detector_distinguishes_wrapped_from_unwrapped() {
        fn unwrapped_sites(src: &str) -> usize {
            let mut n = 0;
            for (i, raw) in src.lines().enumerate() {
                let line = raw.trim();
                if line.starts_with("//") {
                    continue;
                }
                let code = line.split("//").next().unwrap_or("");
                if !code.contains("tool.execute(") {
                    continue;
                }
                let previous = if i > 0 {
                    src.lines().nth(i - 1).unwrap_or("")
                } else {
                    ""
                };
                if !code.contains("redact_result(") && !previous.contains("redact_result(") {
                    n += 1;
                }
            }
            n
        }

        assert_eq!(
            unwrapped_sites("let r = tool.execute(input, &ctx).await;"),
            1
        );
        assert_eq!(
            unwrapped_sites("let r = Self::redact_result(tool.execute(input, &ctx).await);"),
            0
        );
        // rustfmt splits long calls across two lines; the wrapper is then on
        // the previous line.
        assert_eq!(
            unwrapped_sites(
                "let r =
    Self::redact_result(tool.execute(input, &ctx).await);"
            ),
            0
        );
        // A comment mentioning it is not a call.
        assert_eq!(unwrapped_sites("// we used to tool.execute(input) here"), 0);
    }

    // ── TG-P2-04 · no provider call escapes the daily ceiling ─────────────────

    /// Every site that builds a provider must wrap it in the ceiling.
    ///
    /// Spec `02 §5` puts the cap in *"the one place every call passes"*. That
    /// place is the provider itself, not the agent loop — the loop, the recipe
    /// runner, compaction and the diff classifier each build a provider and call
    /// it directly, so a check in the loop leaves three paths spending freely.
    ///
    /// **A hole in a spend cap is not a partial cap; it is the path spend will
    /// take.** This asserts there are none.
    #[test]
    fn tg_p2_04_every_provider_construction_is_capped() {
        let mut unwrapped = Vec::new();

        for dir in ["autodev-core/src", "src-tauri/src"] {
            let mut stack = vec![repo_root().join(dir)];
            while let Some(d) = stack.pop() {
                let Ok(entries) = std::fs::read_dir(&d) else {
                    continue;
                };
                for e in entries.flatten() {
                    let path = e.path();
                    if path.is_dir() {
                        stack.push(path);
                        continue;
                    }
                    if path.extension().and_then(|x| x.to_str()) != Some("rs") {
                        continue;
                    }
                    // The decorator itself constructs nothing to cap.
                    if path.ends_with("capped.rs") {
                        continue;
                    }
                    // Named exception, with its reason. `settings_commands`
                    // builds providers only to call `models()` — a static list
                    // that spends nothing — and `test_connection()`, which
                    // `CappedProvider` deliberately does not gate: refusing a
                    // key check at the ceiling would leave a user unable to
                    // verify credentials on the one day they most need to, the
                    // day their work stopped. Exempting it by name keeps the
                    // rule meaningful; loosening the rule would not.
                    if path.ends_with("settings_commands.rs") {
                        continue;
                    }
                    let text = std::fs::read_to_string(&path).unwrap_or_default();
                    let production = text.split("#[cfg(test)]").next().unwrap_or("");

                    // A file that dispatches on provider id to build one.
                    let builds = production.contains("AnthropicProvider::new(")
                        && production.contains("GoogleProvider::new(");
                    if !builds {
                        continue;
                    }
                    let capped = production.contains("CappedProvider::wrap");
                    if !capped {
                        unwrapped.push(
                            path.strip_prefix(repo_root())
                                .unwrap_or(&path)
                                .to_string_lossy()
                                .replace('\\', "/"),
                        );
                    }
                }
            }
        }

        assert!(
            unwrapped.is_empty(),
            "these files build a provider without wrapping it in the daily ceiling, \
             so calls made through them can spend past the cap: {}",
            unwrapped.join(", ")
        );
    }

    /// The shipping binary must publish its state, or `wrap_global` is inert.
    ///
    /// `wrap_global` returns the provider **unwrapped** when no state has been
    /// published — a deliberate concession so unit tests do not need a ceiling
    /// they never configured. That concession is safe only while the real binary
    /// publishes. Without this assertion the concession becomes the behaviour.
    #[test]
    fn tg_p2_04_the_desktop_binary_publishes_its_state() {
        let main = std::fs::read_to_string(repo_root().join("src-tauri/src/main.rs"))
            .expect("main.rs");
        let production = main.split("#[cfg(test)]").next().unwrap_or("");
        assert!(
            production.contains("app_state::set_global("),
            "main.rs must publish AppState, or CappedProvider::wrap_global silently \
             returns an UNCAPPED provider for every core-side caller"
        );
    }

    // ── TG-P2-03 · an autonomous agent runs under a spend ceiling ─────────────

    /// Every unattended `AgentEngine` must be given a budget.
    ///
    /// **This gate exists because the enforcement was unreachable for the whole
    /// life of the codebase.** `with_task_budget` had zero callers — its
    /// signature demanded a `task_id` that neither autonomous path has at
    /// construction — so `task_budget` was permanently `None`, the guard in
    /// `engine.run` never ran, and `escalate_budget_exceeded`,
    /// `TaskBudget::default_worker` and `BudgetExceededReason` were all dead.
    /// The app had **no enforced spend limit at any scope**.
    ///
    /// A test that only checked "the guard is correct" would have passed
    /// throughout. This checks that the guard is **reached**.
    #[test]
    fn tg_p2_03_autonomous_engines_are_given_a_budget() {
        // The two unattended paths. The interactive PA path is deliberately
        // absent: a user-driven session is capped by the daily ceiling, not by
        // a per-task one, and capping it at a worker's $0.50 would break it.
        let autonomous = [
            "autodev-core/src/recipes/runner.rs",
            "autodev-core/src/subagent/native_runner.rs",
        ];

        let mut unbudgeted = Vec::new();
        for rel in autonomous {
            let text = std::fs::read_to_string(repo_root().join(rel)).expect(rel);
            let production = text.split("#[cfg(test)]").next().unwrap_or("");
            if !production.contains("AgentEngine::new(") {
                continue;
            }
            // The builder must be applied to the engine this file constructs.
            let has_budget = production.contains(".with_budget(")
                || production.contains(".with_task_budget(");
            if !has_budget {
                unbudgeted.push(rel);
            }
        }

        assert!(
            unbudgeted.is_empty(),
            "these paths build an unattended AgentEngine with no budget, so the \
             guard in engine.run cannot fire and the run can spend without limit: \
             {}",
            unbudgeted.join(", ")
        );
    }

    /// The budget builders must have callers at all.
    ///
    /// Guards the exact defect above from returning in a different shape: a
    /// builder that exists, is tested, and is called by nothing.
    #[test]
    fn tg_p2_03_the_budget_builders_are_not_dead_code() {
        let mut callers = 0;
        for dir in ["autodev-core/src", "src-tauri/src"] {
            let root = repo_root().join(dir);
            let mut stack = vec![root];
            while let Some(d) = stack.pop() {
                let Ok(entries) = std::fs::read_dir(&d) else {
                    continue;
                };
                for e in entries.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                        continue;
                    }
                    if p.extension().and_then(|x| x.to_str()) != Some("rs") {
                        continue;
                    }
                    // The definition site is not a caller.
                    if p.ends_with("engine.rs") {
                        continue;
                    }
                    let text = std::fs::read_to_string(&p).unwrap_or_default();
                    let production = text.split("#[cfg(test)]").next().unwrap_or("");
                    callers += production.matches(".with_budget(").count();
                    callers += production.matches(".with_task_budget(").count();
                }
            }
        }
        assert!(
            callers >= 2,
            "the budget builders have {callers} production callers. They had ZERO \
             until 2026-09-02, which is why no spend limit was ever enforced."
        );
    }

    // ── TG-P2-02 · every model the app offers can be priced ───────────────────

    /// A model the UI offers must be priceable, or documented as knowingly not.
    ///
    /// **This gate is the one that was missing.** The price table and the model
    /// picker were maintained independently, and the drift was total: of the 12
    /// models `models.ts` offered, **9 had no entry** and silently billed at an
    /// invented $1.00/$3.00 — including `DEFAULT_MODEL`, so the common path was
    /// the wrong one.
    ///
    /// Two lists that must agree, with nothing asserting they do, will diverge.
    /// This reads both and fails when they do.
    #[test]
    fn tg_p2_02_every_offered_model_is_priced_or_documented() {
        let offered_src =
            std::fs::read_to_string(repo_root().join("src/data/models.ts")).expect("models.ts");
        let pricing_src =
            std::fs::read_to_string(repo_root().join("autodev-core/src/cost/pricing.rs"))
                .expect("pricing.rs");

        // Every quoted token in the MODEL_GROUPS block that looks like a model id.
        //
        // Scanned token-wise rather than line-wise, because the file uses BOTH
        // layouts: Google and Anthropic list their models inline on one line,
        // OpenAI one per line. The first version assumed one-per-line and
        // silently found 6 of 12 — written from the formatting rather than from
        // the invariant, which is L-C6 in miniature. The floor assertion below
        // is what caught it.
        let block = {
            let from = offered_src
                .find("MODEL_GROUPS")
                .expect("MODEL_GROUPS not found — re-point this gate");
            let rest = &offered_src[from..];
            let to = rest.find("] as const").unwrap_or(rest.len());
            &rest[..to]
        };

        let mut offered: Vec<String> = Vec::new();
        for token in block.split('\'').skip(1).step_by(2) {
            let has_digit = token.chars().any(|c| c.is_ascii_digit());
            let has_sep = token.contains('-') || token.contains('.');
            if has_digit && has_sep && !token.contains(' ') {
                offered.push(token.to_string());
            }
        }

        assert!(
            offered.len() >= 10,
            "could not read the offered model list — found {}: {:?}. Re-point this \
             gate if models.ts changed shape.",
            offered.len(),
            offered
        );

        // Priced, or named in the module's "deliberately absent" note. Both are
        // accounted for; silence is not.
        let unaccounted: Vec<&String> = offered
            .iter()
            .filter(|id| !pricing_src.contains(id.as_str()))
            .collect();

        assert!(
            unaccounted.is_empty(),
            "these models are offered in the UI but are neither priced nor recorded \
             as deliberately unpriced in cost/pricing.rs, so selecting one bills the \
             user at a fabricated rate: {:?}",
            unaccounted
        );
    }

    // ── TG-P2-01 · a credential never enters a URL, and both log sinks redact ──

    /// No provider may put an API key in a request URL.
    ///
    /// **Found live in P2's pre-build audit.** `google.rs` authenticated with
    /// `?key={}`, and `reqwest`'s `Error` Display includes the request URL. That
    /// string was logged *and* returned as `ProviderStatus::Error`, which
    /// crosses the Tauri IPC boundary and is rendered in Settings. Redaction did
    /// not save it: `SENSITIVE_KEYS` carries `api_key`/`apikey`/`api-key` but
    /// not bare `key`, so `?key=AIza…` matched nothing.
    ///
    /// A URL is the wrong place for a secret independently of redaction — it
    /// reaches proxy logs, browser history and any HTTP debug capture. Anthropic
    /// (`x-api-key`) and OpenAI (`Authorization`) already used headers; Google
    /// now uses `x-goog-api-key`.
    #[test]
    fn tg_p2_01_no_provider_puts_a_key_in_a_url() {
        let dir = repo_root().join("autodev-core/src/provider");
        let mut offenders = Vec::new();

        for entry in std::fs::read_dir(&dir).expect("provider dir") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read");
            let production = text.split("#[cfg(test)]").next().unwrap_or("");
            for (i, raw) in production.lines().enumerate() {
                let line = raw.trim();
                if line.starts_with("//") || line.starts_with('*') {
                    continue;
                }
                let code = line.split("//").next().unwrap_or("");
                // Query-string syntax is the unambiguous needle: `?key=` or
                // `&key=`.
                //
                // The first version of this check required a URL marker
                // (`http`/`_BASE`) on the SAME line as `key=`. `format!` puts
                // the template on one line and the arguments on the next, so it
                // could never match the real defect. It passed against a planted
                // regression -- L-C9, a check whose shape cannot express failure
                // -- and only falsification caught it.
                let has_key_param = code.contains("?key=")
                    || code.contains("&key=")
                    || code.contains("?api_key=")
                    || code.contains("&api_key=");
                if has_key_param {
                    offenders.push(format!(
                        "{}:{}: {}",
                        path.file_name().unwrap().to_string_lossy(),
                        i + 1,
                        line
                    ));
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "an API key is being placed in a request URL, where it reaches proxy \
             logs, transport error strings, and any HTTP capture. Use a header: {}",
            offenders.join(" | ")
        );
    }

    /// Both logging sinks redact, not just the file.
    ///
    /// P1 wrapped the file appender and left the console layer raw, while its
    /// own comment claimed every line was covered "regardless of which
    /// subsystem emitted it". Anything on stderr went out in the clear for a
    /// whole phase. This asserts the shape rather than the claim.
    #[test]
    fn tg_p2_01_both_log_sinks_are_redacted() {
        let text = std::fs::read_to_string(repo_root().join("src-tauri/src/logging.rs"))
            .expect("logging.rs");
        let production = text.split("#[cfg(test)]").next().unwrap_or("");

        let console_wrapped = production.contains("RedactingMakeWriter(std::io::stderr)");
        assert!(
            console_wrapped,
            "the console layer must write through the redactor; it was raw until \
             2026-09-02 and leaked a Google API key to stderr"
        );

        let file_wrapped = production.contains("RedactingWriter {");
        assert!(file_wrapped, "the file appender must write through the redactor");

        // Neither sink may be constructed without going through one of the two
        // wrappers. `fmt::layer()` with a bare writer is the regression shape.
        let bare_stderr = production
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .any(|l| l.contains(".with_writer(std::io::stderr"));
        assert!(
            !bare_stderr,
            "a log layer writes to stderr without redaction"
        );
    }

    /// Raw tool arguments may reach exactly ONE place: the permission dialog.
    ///
    /// The gate above guards the *result* side. This guards the *input* side,
    /// and it exists because the first A-01 fix did the former and forgot the
    /// latter: `execute_batch`'s write branch redacted what the tool returned
    /// and still handed the renderer, the `messages` row and the hook the raw
    /// arguments. A credential passed *to* a tool is as durable as one returned
    /// *from* it, and no gate could see the difference.
    ///
    /// Everything that reports or persists must take `reported`, the redacted
    /// copy. The dialog is the deliberate exception — the user is approving
    /// that exact call, and a redacted prompt asks them to approve a command
    /// they cannot see, which weakens the control rather than the leak.
    #[test]
    fn tg_p1_06_raw_tool_input_reaches_only_the_permission_dialog() {
        let path = repo_root().join("autodev-core/src/agent/tool_executor.rs");
        let text = std::fs::read_to_string(&path).expect("tool_executor.rs");
        let production = text.split("#[cfg(test)]").next().unwrap_or("");

        let sites = raw_input_sites(production);
        assert_eq!(
            sites.len(),
            1,
            "raw tool input must reach exactly one site (the permission dialog); found {}: {}",
            sites.len(),
            sites
                .iter()
                .map(|(n, f, l)| format!("{f}():{n} {l}"))
                .collect::<Vec<_>>()
                .join(" | ")
        );
        let (line_no, enclosing, line) = &sites[0];
        assert_eq!(
            enclosing, "request_permission",
            "the one site allowed to see raw tool arguments is the permission dialog, but the raw value is in {enclosing}() at line {line_no}:              {line} — this reaches the model, the database or a user-authored hook, all of which are durable"
        );
    }

    /// Every line that hands a *raw* `input` to a payload, with the function
    /// it sits in. Shared by the gate and its own falsification below.
    fn raw_input_sites(src: &str) -> Vec<(usize, String, String)> {
        let mut sites = Vec::new();
        let mut current = "<none>".to_string();
        for (i, raw) in src.lines().enumerate() {
            let line = raw.trim();
            for prefix in ["pub async fn ", "async fn ", "pub fn ", "fn "] {
                if let Some(rest) = line.strip_prefix(prefix) {
                    current = rest.split('(').next().unwrap_or("").to_string();
                    break;
                }
            }
            if line.starts_with("//") || line.starts_with('*') {
                continue;
            }
            let code = line.split("//").next().unwrap_or("");
            if code.contains("input: input.clone()") || code.contains("input: input,") {
                sites.push((i + 1, current.clone(), line.to_string()));
            }
        }
        sites
    }

    /// The detector must see a raw site, ignore a redacted one, and attribute
    /// each to the right function — otherwise the gate above passes by being
    /// blind rather than by the code being safe.
    #[test]
    fn tg_p1_06_raw_input_detector_can_fail() {
        let safe = "    fn report(&self) {
        Payload { input: reported.clone() }
    }";
        assert!(
            raw_input_sites(safe).is_empty(),
            "the redacted copy must not be reported as a leak"
        );

        let leaky = "    fn fire_hook(&self) {
        Ctx { input: input.clone() }
    }";
        let found = raw_input_sites(leaky);
        assert_eq!(found.len(), 1, "a raw site must be detected");
        assert_eq!(found[0].1, "fire_hook", "attributed to its function");

        // A comment is not a call site.
        assert!(raw_input_sites("        // input: input.clone() was here").is_empty());
    }

    /// Redaction must also reach the boundaries that are not the tool path.
    ///
    /// `P1-21` names five: prompts, logs, memory writes, analytics, egress.
    /// Paths are crate-qualified because they are not all in core — the log
    /// appender belongs to the desktop host.
    ///
    /// **On egress, stated rather than glossed.** There is deliberately no
    /// redaction of the outbound provider request body. Redacting what is
    /// *sent* would strip the very content the model has to reason about — a
    /// config file the user asked about, a stack trace they pasted — and would
    /// break the product to protect a channel the user chose to use. The egress
    /// risk that actually reaches durable storage is what we **record** about a
    /// provider call, and that is covered: `record_api_failure` redacts the
    /// error message (an auth failure routinely echoes the request), and the
    /// log appender redacts everything written to disk. See
    /// ADR-LOG-RESTRUCT-0019.
    #[test]
    fn tg_p1_06_every_named_boundary_reaches_the_redactor() {
        const BOUNDARIES: &[(&str, &str, &str)] = &[
            (
                "autodev-core/src",
                "agent/tool_executor.rs",
                "prompts: tool results on their way into the conversation",
            ),
            (
                "autodev-core/src",
                "memory/episodic.rs",
                "memory: conversation history, which is permanent",
            ),
            (
                "autodev-core/src",
                "memory/semantic.rs",
                "memory: extracted facts",
            ),
            (
                "autodev-core/src",
                "memory/procedural.rs",
                "memory: skills written to disk",
            ),
            (
                "autodev-core/src",
                "analytics/collector.rs",
                "analytics + egress diagnostics: provider errors and system events",
            ),
            (
                "src-tauri/src",
                "logging.rs",
                "logs: the file appender — a real sk- credential was found in a live log",
            ),
        ];

        let root = repo_root();
        let mut missing = Vec::new();
        for (krate, rel, why) in BOUNDARIES {
            let path = root.join(krate).join(rel);
            let Ok(text) = std::fs::read_to_string(&path) else {
                missing.push(format!(
                    "{krate}/{rel}: file not found — has the boundary moved?"
                ));
                continue;
            };
            let production = text.split("#[cfg(test)]").next().unwrap_or("");
            if !production.contains("security::redaction") {
                missing.push(format!("{krate}/{rel} ({why})"));
            }
        }

        assert!(
            missing.is_empty(),
            "these boundaries do not redact, so content passes through them verbatim: {}",
            missing.join(" | ")
        );
    }

    /// The ported key list must not quietly shrink.
    ///
    /// `G4b §A.14` called `SENSITIVE_KEYS` "the spec". Losing an entry silently
    /// widens what escapes, and nothing else would notice.
    #[test]
    fn tg_p1_06_the_sensitive_key_list_is_intact() {
        let path = repo_root().join("autodev-core/src/security/redaction.rs");
        let text = std::fs::read_to_string(&path).expect("redaction.rs");
        let production = text.split("#[cfg(test)]").next().unwrap_or("");

        // The four the Python source treats as load-bearing, plus the two the
        // plan's count omitted.
        for key in [
            "api_key",
            "password",
            "authorization",
            "private_key",
            "credential",
            "credentials",
        ] {
            assert!(
                production.contains(&format!("{key:?}")),
                "SENSITIVE_KEYS no longer contains {key}"
            );
        }
    }
}
