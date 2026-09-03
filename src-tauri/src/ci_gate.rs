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
use autodev_core::state::lock::RwLockTolerant;

/// Repository root — one level above `src-tauri/`.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri always has a parent")
        .to_path_buf()
}

/// Every `.rs` file under `dir`, recursively.
fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_rs(&p, out);
        } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

/// Every file under `dir` with the given extension, recursively.
fn collect_ext(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_ext(&p, ext, out);
        } else if p.extension().and_then(|x| x.to_str()) == Some(ext) {
            out.push(p);
        }
    }
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
            read_source(&repo_root().join("package.json"));
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
        let manifest = read_source(&repo_root().join("autodev-core/Cargo.toml"));
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

        // ⚠ P3-67. This asserted `total >= 3`, sized to the executor's structure
        // at the time rather than to the invariant — so `P3-10` (collapse to one
        // execution path) would have **failed this gate by succeeding**. The
        // message even anticipated it: *"has the executor been restructured?"*
        //
        // The invariant is *every execute site redacts, and at least one
        // exists*. A floor of one keeps the gate from passing vacuously if the
        // scan is ever re-pointed at the wrong file; it does not encode how many
        // paths there happen to be. (`L-C6`.)
        assert!(
            total >= 1,
            "found no tool.execute call sites at all — this gate is scanning the wrong              file, so its green result means nothing"
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
                    // Test modules included via `#[path]` carry no
                    // `#[cfg(test)]` marker inside them, so the split above
                    // cannot strip them. A conformance test that builds a
                    // provider to inspect its request body makes no API call
                    // and needs no ceiling.
                    if path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.ends_with("_tests.rs"))
                        .unwrap_or(false)
                    {
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
                    let text = read_source(&path);
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
        let main = read_source(&repo_root().join("src-tauri/src/main.rs"));
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
            let text = read_source(&repo_root().join(rel));
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
            read_source(&repo_root().join("src/data/models.ts"));
        let pricing_src =
            read_source(&repo_root().join("autodev-core/src/cost/pricing.rs"));

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
        // Strip `//` comments BEFORE pairing quotes.
        //
        // The model ids are code; the prose around them is not. One apostrophe
        // in a comment shifts every subsequent quote pair by one, so the scan
        // reads the GAPS between ids rather than the ids themselves. That
        // happened on 2026-09-02 while editing this very list — a comment
        // containing "404" plus an apostrophe plus "d" — and the floor
        // assertion below is what caught it: the gate found 0 models and said
        // so, rather than finding a plausible subset and passing.
        let code_only: String = block
            .lines()
            .map(|l| match l.find("//") {
                Some(j) => &l[..j],
                None => l,
            })
            .collect::<Vec<_>>()
            .join("\n");

        for token in code_only.split('\'').skip(1).step_by(2) {
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

    /// Whether a path is inside the sidecar, which is scheduled for deletion
    /// and speaks LiteLLM proxy naming rather than direct-API ids.
    fn rel_dir_is_sidecar(path: &std::path::Path) -> bool {
        path.components()
            .any(|c| c.as_os_str().to_str() == Some("sidecar"))
    }

    // ── TG-P2-13 · a transport error never reaches the renderer ───────────────

    /// `ProviderStatus::Error` must not carry a raw `reqwest::Error`.
    ///
    /// **D-6, from the P2 post-phase audit — and a recurrence.** `reqwest`'s
    /// `Error` Display includes the request URL. `ProviderStatus` crosses the
    /// Tauri IPC boundary and is rendered in Settings, so returning
    /// `e.to_string()` publishes whatever was in that URL to the renderer.
    ///
    /// P2's pre-build audit found exactly this leaking a Google API key from a
    /// `?key=` query string. The fix moved the key to a header — and left the
    /// conduit itself open on two of the three adapters. Since `P2-23`,
    /// `set_provider_key` surfaces that string to the user directly, so the
    /// conduit became load-bearing rather than merely latent.
    ///
    /// The error is still logged in full; the log sink redacts. Only the
    /// IPC-facing value is fixed text.
    #[test]
    fn tg_p2_13_no_adapter_returns_a_raw_transport_error_to_the_renderer() {
        let dir = repo_root().join("autodev-core/src/provider");
        let mut offenders = Vec::new();

        for adapter in ["anthropic.rs", "openai.rs", "google.rs"] {
            let text = read_source(&dir.join(adapter));

            for (n, line) in text.lines().enumerate() {
                let code = line.split("//").next().unwrap_or("");
                if !code.contains("ProviderStatus::Error(") {
                    continue;
                }
                // `e.to_string()` or a format! interpolating the error binding.
                let leaks = code.contains("e.to_string()")
                    || code.contains("{e}")
                    || code.contains("{err}")
                    || code.contains("err.to_string()");
                if leaks {
                    offenders.push(format!(
                        "{adapter}:{} returns a transport error across the IPC boundary, and                          reqwest's Display includes the request URL: {}",
                        n + 1,
                        code.trim()
                    ));
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "a provider status shown in the UI must not carry a raw transport error:
{}",
            offenders.join("
")
        );
    }

        /// Read a source file with line endings normalised to `\n`.
///
    /// ── Why this exists (found 2026-09-02, P3) ───────────────────────────────────
///
    /// Several gates locate a function body by splitting on a literal `"\n}\n"` or
    /// `"\n    }"`. Git's `core.autocrlf` rewrote the working tree to CRLF, so those
    /// patterns stopped matching — and the `unwrap_or(whole_file)` fallback meant
    /// the gate silently widened from *"this function's body"* to **"the rest of the
    /// file"**, then failed on a symbol that appears somewhere further down.
///
    /// `S2-11` failed exactly this way: it asserts the tool-ordering path holds no
    /// `HashMap`, the path is `BTreeMap`-only and correct, and the gate reported it
    /// broken because it was reading 40 KB it was never meant to see.
///
    /// A gate whose meaning depends on how git happened to check the file out is
    /// not a gate. Every gate that parses source reads through here.
    fn read_source(path: &std::path::Path) -> String {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .replace("\r\n", "\n")
    }

    // ── TG-P3-03 · one panic cannot poison the application ────────────────────

    /// No shipped code may take a `std::sync` lock with `.lock().unwrap()`.
    ///
    /// ── What this actually prevents ──────────────────────────────────────────
    ///
    /// `P3-03` said a panicking worker takes down the app. It does not — agent
    /// paths run inside `tokio::spawn` and unwinding is on. What happens is that
    /// the panic **poisons every mutex the thread held**, and this tree had
    /// **98 `.lock().unwrap()` sites against 5 poison-tolerant**. So one panic
    /// became a cascade of unrelated panics at every later lock: a permanent,
    /// restart-only failure presenting several layers from its cause.
    ///
    /// `catch_unwind` contains the *first* panic and does nothing about the
    /// poison it leaves behind. Both halves are required; this gate holds the
    /// second.
    ///
    /// ⚠ Asserted as *"no bare `.lock().unwrap()` exists"* rather than *"those
    /// 98 sites were changed"*, so site 99 added next week is caught too.
    #[test]
    fn tg_p3_03_no_lock_propagates_poison() {
        let roots = [
            repo_root().join("autodev-core/src"),
            repo_root().join("src-tauri/src"),
            repo_root().join("src-cli/src"),
        ];

        let mut offenders = Vec::new();
        let mut scanned = 0usize;

        for root in &roots {
            let mut stack = vec![root.clone()];
            while let Some(dir) = stack.pop() {
                let Ok(entries) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                        continue;
                    }
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !name.ends_with(".rs") {
                        continue;
                    }
                    // `lock.rs` demonstrates the raw pattern on purpose, and
                    // its tests must be able to observe a genuine poisoning.
                    if name == "lock.rs" {
                        continue;
                    }

                    let text = read_source(&path);
                    scanned += 1;
                    let rel = path
                        .strip_prefix(repo_root())
                        .unwrap_or(&path)
                        .display()
                        .to_string()
                        .replace('\\', "/");

                    let production = match text.find("#[cfg(test)]") {
                        Some(i) => &text[..i],
                        None => &text[..],
                    };

                    for (n, line) in production.lines().enumerate() {
                        let code = line.split("//").next().unwrap_or("");
                        if code.contains(".lock().unwrap()")
                            || code.contains(".read().unwrap()")
                            || code.contains(".write().unwrap()")
                        {
                            offenders.push(format!("{rel}:{}: {}", n + 1, code.trim()));
                        }
                    }
                }
            }
        }

        assert!(
            scanned > 100,
            "only {scanned} files scanned — this gate is not reaching the tree"
        );

        assert!(
            offenders.is_empty(),
            "these propagate lock poisoning, so one panic anywhere becomes a cascade of \
             unrelated panics and a restart-only application. Use `lock_tolerant()` / \
             `read_tolerant()` / `write_tolerant()`:\n{}",
            offenders.join("\n")
        );
    }

    // ── TG-P3-02 · a panicking turn is contained and always clears its flag ───

    /// The agent turn runs under `catch_unwind`, and `is_running` clears on
    /// every unwind path.
    ///
    /// `is_running` was cleared on the line *after* the await, so a panic
    /// skipped it and left the session permanently marked running — the user
    /// could never start another turn there, and nothing said why. A `Drop`
    /// guard clears it during unwinding; a line after `catch_unwind` is one
    /// early `return` away from being skipped again.
    #[test]
    fn tg_p3_02_a_panicking_turn_is_contained_and_releases_its_session() {
        let src = read_source(&repo_root().join("src-tauri/src/commands/agent_commands.rs"));

        assert!(
            src.contains("catch_unwind"),
            "the agent turn does not run under catch_unwind, so a panicking tool takes \
             the whole spawn down with no reported failure"
        );

        assert!(
            src.contains("impl Drop for RunningGuard"),
            "is_running is not cleared by a Drop guard, so a panicking turn leaves the \
             session permanently marked running"
        );

        // And the clear must NOT also sit as a bare post-await statement: two
        // clears is a sign the guard was added without removing the old path,
        // which is how the guard silently stops being the thing that matters.
        let bare_clears = src.matches("agent.is_running = false;").count();
        assert!(
            bare_clears <= 2,
            "found {bare_clears} sites clearing is_running; expected the Drop guard plus \
             at most the early-return path"
        );
    }

    // ── TG-P3-11 · exactly one permission gate, keyed on ToolEffect ───────────

    /// Every tool passes **exactly one** permission check, and `is_read_only`
    /// gates nothing.
    ///
    /// ── The defect ───────────────────────────────────────────────────────────
    ///
    /// There were two divergent gates and one missing one. `execute()` matched
    /// on the mode and used `is_read_only()` — a **concurrency hint** — as the
    /// auto-approval key. The write partition of `execute_batch()` matched
    /// differently. And the **read partition had no permission code at all**,
    /// running under `// Execute (read-only always approved)` after cloning
    /// `self.mode` and `self.always_allow` and binding them as `_mode` /
    /// `_always_allow` to silence the unused warning.
    ///
    /// `execute_batch` is the only path the agent loop calls. So
    /// `is_read_only() -> true` was an unconditional bypass of every permission
    /// mode — and `check_file` declared it while executing binaries resolved
    /// from inside the workspace under inspection.
    #[test]
    fn tg_p3_11_one_permission_gate_keyed_on_effect() {

        // ── the invariant this gate should have asserted from the start ──
        //
        // 🔴 The P3 post-phase gap hunt found a SECOND `check_permission` call
        // inside `execute_batch`'s write partition. Every write/execute tool
        // was prompted twice — and with a fresh `Uuid::new_v4()` call id the
        // second time, so the user saw an unrelated request for a call they
        // had just approved.
        //
        // This gate was green throughout. It asserted the ABSENCE of
        // `is_read_only` — the shape of the P3-64 fix — rather than "exactly
        // one gate", which is what the fix existed to establish. `§0.16 L-C6`,
        // written into a gate an hour after recording L-C6's seventh instance.
        {
            let executor = read_source(
                &repo_root().join("autodev-core/src/agent/tool_executor.rs"),
            );
            let batch = executor
                .split("pub async fn execute_batch")
                .nth(1)
                .expect("execute_batch must exist");

            let gates = batch
                .lines()
                .filter(|l| {
                    let code = l.split("//").next().unwrap_or("");
                    code.contains("self.check_permission(")
                })
                .count();

            assert_eq!(
                gates, 1,
                "`execute_batch` contains {gates} permission gates. Its own                  docstring says every tool passes exactly one; two means the                  user is asked twice for one call, and zero means the gate that                  P3-64 exists to enforce is gone."
            );
        }

        let src = read_source(&repo_root().join("autodev-core/src/agent/tool_executor.rs"));

        // Exactly one function may decide permission.
        let gate_definitions = src.matches("async fn check_permission(").count();
        assert_eq!(
            gate_definitions, 1,
            "expected exactly one permission gate, found {gate_definitions}"
        );

        // `is_read_only` must not appear in the executor **at all**.
        //
        // ⚠ The first version asked whether `is_read_only` appeared on a line
        // that *also* mentioned `approved`, `request_permission` or
        // `always_allow` — the shape the old code happened to have.
        // Falsification exposed it at once: planting
        // `if tool.is_read_only() || !tool.effect().requires_prompt(..)` back
        // into the gate **passed**, because that line mentions none of those
        // words. A check written from the defect's old shape rather than from
        // the invariant is `L-C6`, and this was its seventh instance — written
        // an hour after recording the sixth.
        //
        // The invariant is simply that the executor never consults it: the
        // partition asks `effect().is_concurrency_safe()` and prompting asks
        // `effect().requires_prompt()`. Zero occurrences, so a planted one
        // fails however the line is written.
        let offenders: Vec<String> = src
            .lines()
            .enumerate()
            .filter(|(_, l)| {
                let code = l.split("//").next().unwrap_or("");
                code.contains("is_read_only")
            })
            .map(|(n, l)| format!("{}: {}", n + 1, l.trim()))
            .collect();

        assert!(
            offenders.is_empty(),
            "tool_executor.rs still consults `is_read_only`. It is a concurrency hint, and \
             using it here is how a flag became a security control — ask `effect()` \
             instead:\n{}",
            offenders.join("\n")
        );

        // The read partition must not run anything the gate has not seen.
        //
        // ⚠ Scanned with comments stripped. The first version matched the
        // phrase anywhere in the file — and the docstring that *explains* the
        // defect quotes it, so the gate flagged the documentation of the fix.
        // Same shape as the apostrophe that broke `TG-P2-02`: prose describing
        // a defect is not the defect.
        let code_only: String = src
            .lines()
            .map(|l| match l.find("//") {
                Some(i) => &l[..i],
                None => l,
            })
            .collect::<Vec<_>>()
            .join("
");

        assert!(
            !code_only.contains("read-only always approved"),
            "the read partition still executes without a permission check"
        );

        // And the decision must be reached before the partition, so a
        // concurrency-safe tool cannot skip it.
        let gate_at = src.find("self.check_permission(").expect("the gate must be called");
        let partition_at = src
            .find("Partition the approved calls")
            .expect("the partition comment must exist");
        assert!(
            gate_at < partition_at,
            "permission is decided after the partition, so the concurrent path can run \
             tools the gate never saw"
        );
    }

    // ── TG-P3-56 · a tool that leaves the machine declares it ─────────────────

    /// Every tool declares a `ToolEffect`, and the ones that send data off the
    /// machine declare `Network`.
    ///
    /// `analyze_diff` uploaded the full diff of any repository to a third-party
    /// LLM while declaring `is_read_only = true`, so it was never prompted in
    /// any mode. Under BYOK that is the user's own code on the user's own key.
    #[test]
    fn tg_p3_56_tools_that_leave_the_machine_declare_network() {
        let dir = repo_root().join("autodev-core/src/agent/builtin_tools");

        let mut declared = 0usize;
        let mut missing = Vec::new();

        for entry in std::fs::read_dir(&dir).expect("builtin_tools").flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = read_source(&path);
            if !text.contains("impl Tool for") {
                continue;
            }
            let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("").to_string();

            if text.contains("fn effect(") {
                declared += 1;
            } else {
                missing.push(name.clone());
            }

            // The two that were mislabelled, pinned by name so a future edit
            // cannot quietly return them to Read.
            if name == "analyze_diff_tool" {
                assert!(
                    text.contains("ToolEffect::Network"),
                    "analyze_diff uploads the repository diff to a third-party LLM; it must \
                     declare Network"
                );
            }
            if name == "check_file" {
                assert!(
                    text.contains("ToolEffect::Execute"),
                    "check_file runs external checkers, including one resolved from inside \
                     the inspected workspace; it must declare Execute"
                );
            }
        }

        assert!(
            missing.is_empty(),
            "these tools declare no effect, so nothing decides whether they prompt: {:?}",
            missing
        );
        assert!(
            declared >= 30,
            "only {declared} tools declared an effect — this gate is not reaching the tree"
        );
    }

    // ── TG-P3-14 · every path-taking tool calls the jail ──────────────────────

    /// A tool that accepts a path from the model must validate it against the
    /// workspace.
    ///
    /// Six bypassed it entirely and were auto-approved: `read_file`, `grep`,
    /// `glob`, `list_directory` — **plus `check_file` and `analyze_diff`, which
    /// act on the path rather than merely reading it**. `check_file` spawns
    /// external checkers against it; `analyze_diff` diffs the repository and
    /// uploads the result to a third-party LLM, and its `ToolContext` parameter
    /// was named `_ctx` because the workspace was never consulted at all.
    ///
    /// Reading a file outside the workspace is an **exfiltration primitive**,
    /// not a harmless read: the model names a path, the contents enter its
    /// context, and nothing asks the user.
    ///
    /// ⚠ Keyed on *"declares a path input"*, so a **new** tool with a path
    /// parameter and no jail call fails the build — which is `TG3-27`'s
    /// requirement and the difference between fixing six tools and closing the
    /// hole.
    #[test]
    fn tg_p3_14_every_path_taking_tool_calls_the_jail() {
        let dir = repo_root().join("autodev-core/src/agent/builtin_tools");

        let mut offenders = Vec::new();
        let mut checked = 0usize;

        for entry in std::fs::read_dir(&dir).expect("builtin_tools").flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("").to_string();
            // The jail itself, and the lock that keys on jailed paths.
            if name == "path_jail" || name == "path_lock" || name == "mod" {
                continue;
            }

            let text = read_source(&path);
            if !text.contains("impl Tool for") {
                continue;
            }

            // Does its schema declare a path-shaped input?
            let declares_path = text.contains("\"path\"")
                || text.contains("\"repo_path\"")
                || text.contains("\"file_path\"")
                || text.contains("\"directory\"");

            if !declares_path {
                continue;
            }
            checked += 1;

            // Board tools take a `path` that is a label, not a filesystem
            // location — they never touch the disk. Recognised by having no
            // filesystem access at all.
            let touches_disk = text.contains("std::fs")
                || text.contains("PathBuf::from")
                || text.contains("Path::new")
                || text.contains("tokio::fs");
            if !touches_disk {
                continue;
            }

            if !text.contains("validate_workspace_path") {
                offenders.push(name);
            }
        }

        assert!(
            checked >= 6,
            "only {checked} path-taking tools examined — this gate is not reaching the tree"
        );

        assert!(
            offenders.is_empty(),
            "these tools take a path from the model and touch the filesystem without \
             validating it against the workspace, so the model can name any path on the \
             machine: {:?}",
            offenders
        );
    }

    // ── TG-P3-19 · no executable is resolved from the inspected workspace ─────

    /// The workspace under inspection must never supply a binary we then run.
    ///
    /// 🔴 `resolve_python` searched `<working_dir>/.venv/Scripts/python.exe`
    /// **first** and returned it when present, so cloning a hostile repository
    /// and asking the agent to check any `.py` file executed the repository's
    /// own interpreter — unprompted in every mode, once `P3-64` is accounted
    /// for.
    ///
    /// ⚠ Scoped by **measurement, not assumption**: a probe confirmed that
    /// `Command::new("name").current_dir(untrusted)` does *not* resolve the
    /// program from the child's directory on this platform, so
    /// `.current_dir(working_dir)` on the other checkers is not a second
    /// vector. The explicit path built by `resolve_python` was the only one.
    ///
    /// Keyed on the invariant — *"no executable path is built from the
    /// workspace"* — rather than on `resolve_python`'s current body, so a new
    /// resolver with the same shape fails too.
    #[test]
    fn tg_p3_19_no_executable_is_resolved_from_the_workspace() {
        let path = repo_root().join("autodev-core/src/agent/builtin_tools/validate.rs");
        let text = read_source(&path);

        // Markers of an interpreter/executable location, in any spelling.
        const EXE_MARKERS: [&str; 6] = ["Scripts", "/bin/", ".exe", "venv", "python.exe", "bin\\\\"];

        let mut offenders = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if !code.contains("working_dir") {
                continue;
            }
            if EXE_MARKERS.iter().any(|m| code.contains(m)) {
                offenders.push(format!("{}: {}", i + 1, line.trim()));
            }
        }

        assert!(
            offenders.is_empty(),
            "an executable path is being built from the inspected workspace — the \
             repository under review would supply the binary we then run:\n  {}",
            offenders.join("\n  ")
        );

        // And the capability itself is gone, not merely unused: a resolver that
        // cannot see the workspace cannot be edited back into reading from it.
        assert!(
            text.contains("pub fn resolve_python() -> String"),
            "resolve_python must take no workspace argument; presence of the \
             capability is what let the defect exist (§0 Rule 7)"
        );
    }

    // ── TG-P3-20 · a tool description may not name a mechanism it lacks ───────

    /// Split a builtin-tool source into (description text, the rest of the code).
    ///
    /// The split matters: a keyword occurring only *inside* the description is
    /// a claim with nothing behind it, which is the whole defect class.
    fn split_description(text: &str) -> Option<(String, String)> {
        let i = text.find("fn description(&self)")?;
        let j = text[i..].find("\n    }")? + i;

        // ⚠ Only the string literal reaches the model — a `//` comment beside
        // it does not. Stripping comment lines is not cosmetic: several of
        // these descriptions now carry a comment QUOTING the false claim they
        // replaced, and scanning those would flag the fix as the defect.
        // (It did, on this gate's second run.)
        let desc: String = text[i..j]
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        Some((desc, format!("{}{}", &text[..i], &text[j..])))
    }

    /// A description that names a mechanism must have that mechanism in its
    /// own code.
    ///
    /// 🔴 `glob` claimed *"Automatically respects .gitignore"* and there was no
    /// gitignore handling anywhere in the file — only a fixed `skip_dirs`
    /// array. A model told its results are ignore-filtered assumes `.env` and
    /// build output are absent from them. They were not.
    ///
    /// ⚠ Descriptions are **model-facing contract**, not comments: the model
    /// cannot read the code to check them. A false one is acted on.
    ///
    /// Keyed on the class, not on the four tools that were wrong: a *new*
    /// description promising an absent mechanism fails this too.
    #[test]
    fn tg_p3_20_a_description_may_not_name_a_mechanism_the_code_lacks() {
        // (claim keyword in the description, tokens that would implement it)
        const BACKING: [(&str, &[&str]); 5] = [
            ("gitignore", &["gitignore", "ignore::", "GitignoreBuilder"]),
            ("PID", &["process::id", "pid"]),
            ("tears down", &["kill"]),
            ("timeout", &["timeout", "Duration"]),
            ("transaction", &["transaction", "BEGIN", "tx"]),
        ];

        let dir = repo_root().join("autodev-core/src/agent/builtin_tools");
        let mut offenders = Vec::new();
        let mut examined = 0usize;

        for entry in std::fs::read_dir(&dir).expect("builtin_tools").flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = read_source(&path);
            if !text.contains("impl Tool for") {
                continue;
            }
            let Some((desc, code)) = split_description(&text) else {
                continue;
            };
            examined += 1;

            let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("?");
            for (claim, tokens) in BACKING {
                if !desc.to_lowercase().contains(&claim.to_lowercase()) {
                    continue;
                }
                // A description may legitimately name a mechanism in order to
                // say it is ABSENT — `glob` now does exactly that, and the
                // disclaimer is the most useful sentence in it. Only an
                // un-negated mention is a promise.
                //
                // (This gate flagged that disclaimer on its first run. The
                // check had been written from the claim's *shape* rather than
                // its meaning — §0.16 L-C6, caught here by running it.)
                let lowered = desc.to_lowercase();
                let needle = claim.to_lowercase();

                // ⚠ Sentence-splitting on '.' does not work here: the claim
                // itself may begin with one (".gitignore"), which severs the
                // negation from the word it negates and turns a disclaimer
                // back into a promise. Look at the text immediately BEFORE
                // each occurrence instead.
                let promised = lowered.match_indices(&needle).any(|(at, _)| {
                    let from = at.saturating_sub(60);
                    let before = &lowered[from..at];
                    !before.contains(" not ")
                        && !before.contains("n't")
                        && !before.contains("never")
                });
                if !promised {
                    continue;
                }

                let backed = tokens
                    .iter()
                    .any(|t| code.to_lowercase().contains(&t.to_lowercase()));
                if !backed {
                    offenders.push(format!("{name}: description promises '{claim}'"));
                }
            }
        }

        assert!(
            examined >= 30,
            "only {examined} tool descriptions examined — this gate is not \
             reaching the tree, and an empty sweep passes silently"
        );

        assert!(
            offenders.is_empty(),
            "these tool descriptions name a mechanism absent from their own \
             code — the model cannot read the code to check, so it acts on the \
             claim:\n  {}",
            offenders.join("\n  ")
        );
    }

    /// A description that enumerates valid values must match the schema's enum.
    ///
    /// The other half of the same class: two lists of the same thing, one
    /// model-facing prose and one machine-checked, drifting apart silently.
    #[test]
    fn tg_p3_20_enumerated_values_match_the_schema() {
        let dir = repo_root().join("autodev-core/src/agent/builtin_tools");
        let mut checked = 0usize;
        let mut offenders = Vec::new();

        // Every .rs in the core crate, concatenated once, so a value declared
        // in a shared constant resolves.
        //
        // 🔴 **With descriptions stripped.** The first version concatenated the
        // files whole — including the very descriptions being checked — so
        // every listed value trivially "existed in the crate" and the gate
        // could not fail. Falsification caught it: a planted `worker-ghost`
        // passed. A corpus that contains the claim cannot corroborate it
        // (`§0.16 L-C9`).
        let crate_sources = {
            let mut all = String::new();
            let mut stack = vec![repo_root().join("autodev-core/src")];
            while let Some(d) = stack.pop() {
                for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                        let t = read_source(&p);
                        match split_description(&t) {
                            Some((_, code)) => all.push_str(&code),
                            None => all.push_str(&t),
                        }
                    }
                }
            }
            all
        };

        for entry in std::fs::read_dir(&dir).expect("builtin_tools").flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = read_source(&path);
            let Some((desc, code)) = split_description(&text) else {
                continue;
            };
            let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("?");

            // "Valid statuses: a, b, c." / "Valid workers: a, b, c."
            for marker in ["Valid statuses:", "Valid workers:"] {
                let Some(rest) = desc.split(marker).nth(1) else {
                    continue;
                };
                let listed: Vec<String> = rest
                    .split('.')
                    .next()
                    .unwrap_or("")
                    .replace('\\', " ")
                    .split(',')
                    .map(|t| t.trim().trim_matches('"').to_string())
                    .filter(|t| !t.is_empty() && !t.contains(' '))
                    .collect();
                if listed.is_empty() {
                    continue;
                }
                checked += 1;
                // ⚠ The value may be defined in a shared constant rather than
                // inline — `assign_task`'s `VALID_WORKERS` is `FLEET_WORKERS`
                // from `recipes/defaults.rs`, which is correct indirection, not
                // drift. The invariant is that the listed value exists SOMEWHERE
                // in the crate, not that it is spelled in this file.
                //
                // (The first run of this gate flagged all five workers for
                // exactly this reason. Scoping a check to the wrong file is how
                // a green gate and a red one both fail to mean anything.)
                for value in &listed {
                    if !code.contains(value) && !crate_sources.contains(value) {
                        offenders.push(format!(
                            "{name}: description lists '{value}' under '{marker}', \
                             but it appears nowhere in the code"
                        ));
                    }
                }
            }
        }

        assert!(
            checked >= 1,
            "no enumerated-value description was found — the gate would pass \
             on an empty set, which proves nothing"
        );
        assert!(offenders.is_empty(), "{}", offenders.join("\n  "));
    }

    // ── TG-P3-21/68 · e2e_probe captures evidence and cannot leave the box ──

    /// `e2e_probe` must keep its children's output and must not fetch
    /// off-machine URLs.
    ///
    /// 🔴 `P3-68`: `probe_url` was fetched with **no restriction** and up to
    /// 400 characters of the response body were returned to the model — an
    /// SSRF with readback. `http://169.254.169.254/latest/meta-data/...`
    /// returns IAM credentials on any EC2 host.
    ///
    /// 🔴 `P3-21`: both child streams were `Stdio::null()`, so a failed probe
    /// reported FAIL and could not say why — while the file's own header
    /// claims it catches "dev server crashed on boot".
    ///
    /// ⚠ The URL check must run **before** anything is spawned. Validating
    /// after the children start would still boot the model's shell commands on
    /// the way to rejecting the URL.
    #[test]
    fn tg_p3_21_68_e2e_probe_keeps_evidence_and_stays_on_this_machine() {
        let path = repo_root().join("autodev-core/src/agent/builtin_tools/e2e_probe.rs");
        let text = read_source(&path);

        // ── P3-68 · the URL is confined, by resolution rather than spelling ─
        let validator = text
            .split("fn validate_probe_url")
            .nth(1)
            .expect("e2e_probe must validate probe_url before fetching it");
        let validator_body = validator.split("\n}\n").next().unwrap_or(validator);

        assert!(
            validator_body.contains("is_loopback"),
            "probe_url must be confined to loopback — the response body is \
             returned to the model, so any other host is an exfiltration read"
        );
        assert!(
            validator_body.contains("to_socket_addrs"),
            "the host must be RESOLVED, not string-matched: `127.0.0.1.nip.io` \
             and any attacker-controlled DNS name read as ordinary hostnames"
        );

        // ── ordering: validated before any child is spawned ──────────────
        let call_site = text
            .find("validate_probe_url(&probe_url)")
            .expect("validate_probe_url must actually be CALLED, not merely defined");
        let spawn_site = text
            .find("run_probe(")
            .expect("run_probe must exist");
        assert!(
            call_site < spawn_site,
            "probe_url is validated at byte {call_site} but children are spawned \
             at {spawn_site} — rejecting the URL after booting the model's shell \
             commands is not a check, it is a postscript"
        );

        // ── P3-21 · the children's output is kept ────────────────────
        let runner = text
            .split("async fn run_probe")
            .nth(1)
            .expect("run_probe must exist");
        let spawn_block = runner.split("let started_processes").next().unwrap_or(runner);

        assert!(
            !spawn_block.contains("stdout(Stdio::null())"),
            "the children's stdout is discarded — a failing probe then reports \
             FAIL with no evidence, which is the one thing a smoke test exists \
             to provide"
        );
        assert!(
            !spawn_block.contains("stderr(Stdio::null())"),
            "the children's stderr is discarded — that is where the boot error is"
        );
        assert!(
            spawn_block.contains("Stdio::piped()"),
            "the children's streams must be piped so they can be captured"
        );
        assert!(
            text.contains("start_cmd_output"),
            "the captured output must reach the report — capturing it and \
             dropping it on the floor is the same as not capturing it"
        );

        // ── the consent sentence matches the act ────────────────────
        assert!(
            text.contains("ToolEffect::Execute"),
            "e2e_probe runs arbitrary shell, so the user must be asked about \
             running a command — it declared `Network` (\"send data to an \
             external service\"), which is consent for a different act"
        );
    }

    // ── TG-P3-65 · a scheduled recipe may not declare a prompting mode ────────

    /// `permission_mode` is honoured, and no cron recipe asks for a prompt.
    ///
    /// 🔴 `P3-65`. The field was written by nine recipes and read by nothing:
    /// `runner.rs` hardcoded `PermissionMode::Autonomous`, so four recipes
    /// declaring `Supervised` ran with every prompt suppressed. Anyone reading
    /// `permission_mode: Supervised` on the QC agent would conclude it was
    /// supervised.
    ///
    /// ⚠ Wiring it up creates the opposite hazard, and **that** is what this
    /// gate protects: a recipe on a cron schedule has nobody present to answer
    /// a dialog, so a prompting mode there means every Execute/Network/Board
    /// tool blocks for the 300s `PERMISSION_TIMEOUT`. `qc-dispatcher` fires
    /// every 30 seconds — it would pile up blocked runs and stall the fleet.
    ///
    /// razi-ruled 2026-09-02: honour the field, and let the unattended recipes
    /// declare the Autonomous they actually need. The rule that keeps both
    /// halves true is the one asserted here.
    #[test]
    fn tg_p3_65_no_scheduled_recipe_declares_a_prompting_mode() {
        let path = repo_root().join("autodev-core/src/recipes/defaults.rs");
        let text = read_source(&path);

        let mut current = String::new();
        let mut schedule: Option<bool> = None; // Some(true) = has a cron schedule
        let mut offenders = Vec::new();
        let mut recipes = 0usize;
        let mut scheduled = 0usize;

        for line in text.lines() {
            // ⚠ Comments only — several of the comments added by this very
            // change discuss both "Supervised" and "schedule: None", and
            // scanning them would make the gate report on its own prose.
            let code = line.split("//").next().unwrap_or("");

            if let Some(rest) = code.trim().strip_prefix("fn ") {
                if rest.contains("_recipe() -> Recipe {") {
                    current = rest.split("_recipe").next().unwrap_or("?").to_string();
                    schedule = None;
                    recipes += 1;
                }
            }

            if code.contains("schedule:") {
                schedule = Some(code.contains("Some("));
                if schedule == Some(true) {
                    scheduled += 1;
                }
            }

            if code.contains("permission_mode:") && !code.contains("pub ") {
                let prompting = code.contains("Supervised");
                if prompting && schedule == Some(true) {
                    offenders.push(format!(
                        "{current}: scheduled on cron AND declares Supervised"
                    ));
                }
            }
        }

        assert!(
            recipes >= 9,
            "only {recipes} recipes found — this gate is not reaching the \
             declarations, and an empty sweep passes silently"
        );
        assert!(
            scheduled >= 3,
            "only {scheduled} scheduled recipes found; the hazard this gate \
             guards would be unrepresented"
        );

        assert!(
            offenders.is_empty(),
            "a cron-scheduled recipe declares a prompting permission mode. \
             There is nobody present to answer, so every Execute/Network/Board \
             tool will block for the full 300s permission timeout and the \
             fleet will stall:\n  {}",
            offenders.join("\n  ")
        );

        // The other half: the field must actually be READ, or none of the
        // above matters and we are back to a decorative control.
        let runner = read_source(&repo_root().join("autodev-core/src/recipes/runner.rs"));
        assert!(
            runner.contains("PermissionMode::from(&recipe.permission_mode)"),
            "runner.rs must build the engine from the recipe's declared mode; \
             a hardcoded constant there is what made this field dead"
        );

        // And `SmartApprove` must have a construction site (`P3-66`): a mode
        // that cannot be selected is a guard that cannot fire.
        let types = read_source(&repo_root().join("autodev-core/src/recipes/types.rs"));
        assert!(
            types.contains("Self::SmartApprove"),
            "SmartApprove must be constructible — it had zero construction \
             sites and two live but unreachable match arms"
        );
    }

    // ── TG-P3-55 · the QC provisional charge is released on every exit ────────

    /// A dispatch that never flies must not hold its provisional charge.
    ///
    /// 🔴 `P3-55`. `spawn_qc_review` recorded $0.10 at dispatch start and
    /// released it only from the background completion task — with **eight
    /// error returns in between**, none of which released. The dominant case
    /// is routine: the qc-dispatcher recipe ticks every 30 seconds, and a task
    /// already claimed for review takes the already-claimed branch every time.
    /// **50 ticks x $0.10 = $5.00**, the hourly ceiling — so QC disables
    /// itself for an hour at zero real spend while the ledger says it was busy.
    ///
    /// ⚠ The sharp assertion is the **ordering**: the handoff must come after
    /// the LAST early return. A guard whose handoff sits above some error
    /// return leaves that path leaking, and every other check here would still
    /// pass. This is the same shape as `P3-05` — every exit path must do the
    /// thing, and only the type system writes that once.
    #[test]
    fn tg_p3_55_the_provisional_charge_is_released_on_every_failed_dispatch() {
        let path =
            repo_root().join("autodev-core/src/agent/builtin_tools/spawn_qc_review.rs");
        let text = read_source(&path);

        // The release must be automatic, not eight hand-written call sites.
        assert!(
            text.contains("impl Drop for ProvisionalCharge"),
            "the provisional charge must release via Drop — adding a release \
             call to each error branch works until the ninth branch is added"
        );

        // The guard must be BOUND. `let _ = ...` drops it immediately, which
        // releases the charge before the reviewer is dispatched and makes the
        // ceiling blind to in-flight work — a different bug wearing the fix.
        assert!(
            text.contains("let _charge = match qc_cost_guard::check_and_record_dispatch_intent()"),
            "the charge must be bound to a named variable; `let _ =` would run \
             Drop immediately and defeat the ceiling"
        );

        // ── the ordering that makes the guard load-bearing ────────────────
        let execute = text
            .split("async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult")
            .nth(1)
            .expect("execute must exist");

        let charge_at = execute
            .find("let _charge =")
            .expect("the charge must be taken inside execute");
        let handoff_at = execute
            .find("_charge.handed_off()")
            .expect(
                "the charge must be handed off once the reviewer is in flight, or \
                 the success path releases it and the ceiling never sees in-flight work",
            );

        assert!(
            charge_at < handoff_at,
            "the charge is handed off before it is taken"
        );

        // Every early return between them is covered by Drop. What must be
        // proven is that none sits AFTER the handoff.
        let after_handoff = &execute[handoff_at..];
        let leaked: Vec<&str> = after_handoff
            .lines()
            .filter(|l| {
                let code = l.split("//").next().unwrap_or("");
                code.contains("return ToolResult::error")
            })
            .collect();

        assert!(
            leaked.is_empty(),
            "these failure paths sit AFTER the charge is handed off, so they \
             leak it — move the handoff below them:\n  {}",
            leaked.join("\n  ")
        );

        // And the guard must actually be guarding something: at least one
        // failure path between taking the charge and handing it off. If this
        // ever reaches zero the guard is dead weight and someone has moved the
        // charge, not fixed it.
        let covered = execute[charge_at..handoff_at]
            .lines()
            .filter(|l| {
                let code = l.split("//").next().unwrap_or("");
                code.contains("return ToolResult::error")
            })
            .count();

        assert!(
            covered >= 5,
            "only {covered} failure paths sit under the charge guard; there \
             were eight, so the charge has probably been moved rather than the \
             leak fixed"
        );
    }

    // ── TG-P3-22 · a finished subagent does not hold its pool slot ────────────

    /// Capacity is released at termination, and the prune runs before the wait.
    ///
    /// 🔴 `P3-22`. The permit lived in the registry entry and was released
    /// only when `prune_terminal`'s `retain` dropped it. `spawn` called
    /// `acquire` first and `prune_terminal` **after** — so the prune that
    /// would free permits sat behind the acquire waiting for them.
    /// `prune_terminal`'s only other caller is `list()`, reached solely from
    /// Tauri commands the UI drives. **With the subagent monitor panel closed,
    /// 16 completed subagents blocked the 17th spawn permanently.**
    ///
    /// ⚠ `finalise`'s own docstring said *"The pool permit is released here"*.
    /// It stamped status and nothing else. A docstring that promises behaviour
    /// the code does not perform is a defect, and this one concealed a
    /// deadlock for as long as it was believed.
    ///
    /// The gate asserts the **invariant**, not the two lines that were wrong:
    /// every site that stamps `terminated_at` must also release the permit.
    /// A third such site — and there are already two — fails this.
    #[test]
    fn tg_p3_22_capacity_is_released_at_termination() {
        let path = repo_root().join("autodev-core/src/subagent/manager.rs");
        let text = read_source(&path);

        // The permit must be takeable at all: `_permit: PoolPermit` is held
        // for the entry's whole life by construction.
        assert!(
            text.contains("permit: Option<PoolPermit>"),
            "the permit must be releasable independently of the registry \
             entry — retention is for the UI, not for capacity"
        );

        // ── the invariant · every terminal stamp releases ────────────────
        let stamps: Vec<usize> = text
            .match_indices("terminated_at = Some(Instant::now())")
            .map(|(i, _)| i)
            .collect();

        assert!(
            stamps.len() >= 2,
            "expected at least two terminal-stamping sites (`finalise` and the \
             event pump); found {}. If they were consolidated, re-point this \
             gate deliberately rather than letting it pass on an empty set",
            stamps.len()
        );

        for at in &stamps {
            // The release must sit in the same block, right after the stamp.
            let window_end = (at + 220).min(text.len());
            let window = &text[*at..window_end];
            assert!(
                window.contains("permit.take()"),
                "a site stamps `terminated_at` without releasing the pool \
                 permit. That subagent is finished but still consuming one of \
                 the {} global slots, and the only thing that would free it is \
                 a prune the UI has to trigger:\n...{}",
                "MAX_GLOBAL_CONCURRENT",
                &window[..window.len().min(160)]
            );
        }

        // ── the ordering · prune before the wait ─────────────────────────
        let spawn_fn = text
            .split("pub async fn spawn(")
            .nth(1)
            .expect("spawn must exist");

        let prune_at = spawn_fn
            .find("self.prune_terminal().await")
            .expect("spawn must prune stale terminal entries");
        let acquire_at = spawn_fn
            .find("self.pool.acquire(")
            .expect("spawn must acquire a permit");

        assert!(
            prune_at < acquire_at,
            "`prune_terminal` runs AFTER `acquire` in spawn. `acquire` waits on \
             the semaphore, and the prune queued behind it is the only thing \
             that would release those permits — that is a deadlock, not an \
             ordering preference"
        );
    }

    // ── TG-P3-24 · cancelling a parent stops its children ─────────────────────

    /// A cancelled session must not leave subagents running.
    ///
    /// 🔴 `P3-24`. Subagents are detached tokio tasks, and dropping the
    /// `JoinHandle` the parent awaits **detaches** rather than aborts. So a
    /// cancelled parent left every child running to completion — finishing its
    /// work, emitting its events, and billing the user's own key under BYOK for
    /// a turn nobody would read.
    ///
    /// ⚠ The load-bearing assertion is that the helper is **called**, not that
    /// it exists. `§0.16 L-C12` — a helper built and wired to nothing — stands
    /// at seven instances, every one of which passed a gate that checked only
    /// for the helper's definition.
    #[test]
    fn tg_p3_24_cancellation_reaches_the_children() {
        let manager = read_source(
            &repo_root().join("autodev-core/src/subagent/manager.rs"),
        );
        let engine = read_source(&repo_root().join("autodev-core/src/agent/engine.rs"));

        assert!(
            manager.contains("pub async fn abort_children_of"),
            "the manager must be able to stop a parent's children"
        );

        // ── it is CALLED, from the cancel path, not merely defined ────────
        let cancel_site = engine
            .find("is_cancelled(session_id)")
            .expect("the engine must check for cancellation");
        let after_cancel = &engine[cancel_site..];
        let block_end = after_cancel.find("            }").unwrap_or(after_cancel.len());
        let cancel_block = &after_cancel[..block_end.min(1400)];

        assert!(
            cancel_block.contains("abort_children_of"),
            "the engine breaks out of its turn loop on cancellation without \
             stopping the subagents it spawned — they keep running and billing"
        );

        // ── the collect-then-terminate ordering ──────────────────────────
        let helper = manager
            .split("pub async fn abort_children_of")
            .nth(1)
            .expect("abort_children_of body");
        let body = helper.split("\n    }").next().unwrap_or(helper);

        let collect_at = body
            .find(".collect()")
            .expect("the ids must be collected before terminating");
        let terminate_at = body
            .find("self.abort(")
            .expect("it must actually terminate the children it found");

        assert!(
            collect_at < terminate_at,
            "ids must be collected BEFORE terminating: `terminate` takes the \
             registry lock itself, so holding it across the calls deadlocks on \
             the first child"
        );

        // The lock must be released between the phases — a `let reg = ...`
        // still in scope at the terminate call is the deadlock.
        let between = &body[collect_at..terminate_at];
        assert!(
            between.contains("};"),
            "the registry lock must go out of scope between collecting and \
             terminating; found no scope close between the two phases"
        );
    }

    // ── TG-P3-58/60/61/62/63 · every declared subagent control is read ────────

    /// Five fields a caller can set on a subagent, all of which were written
    /// and none of which were read.
    ///
    /// 🔴 `P3-58` one depth boundary, not two on opposite sides ·
    /// 🔴 `P3-60` `SubagentConfig.tools` had zero readers, so a declared tool
    /// subset was a containment promise the code did not keep ·
    /// 🔴 `P3-61` `DEFAULT_MODEL` is an Anthropic id handed to whichever
    /// provider is active ·
    /// 🔴 `P3-62` cost and tokens hardcoded to zero, which is what the QC
    /// hourly ceiling is computed from ·
    /// 🔴 `P3-63` `max_turns` defaulted then discarded.
    ///
    /// ⚠ These are one gate because they are one defect class — `§0.16 L-C12`,
    /// a field or helper wired to nothing, standing at seven instances. The
    /// assertions are all of the same shape: the value must reach a **reader**.
    #[test]
    fn tg_p3_58_63_declared_subagent_controls_are_honoured() {
        let runner = read_source(
            &repo_root().join("autodev-core/src/subagent/native_runner.rs"),
        );
        let registry = read_source(
            &repo_root().join("autodev-core/src/agent/tool_registry.rs"),
        );

        // ── P3-58 · the pool is the ONLY thing that decides depth ────────
        //
        // 🔴 F-21 (post-phase claims audit) rewrote this. The previous version
        // asserted that each tool compared `ctx.depth + 1 > MAX` instead of
        // `ctx.depth >= MAX` — and for `u32` **those are the same predicate**,
        // so it was gating a no-op and calling it a fix.
        //
        // It also **enumerated two files** and therefore never saw the third
        // guard, in `spawn_qc_review.rs`, written in the very spelling its own
        // failure message called a defect. A gate that lists its subjects
        // misses the one nobody listed; this one sweeps the directory.
        {
            let dir = repo_root().join("autodev-core/src/agent/builtin_tools");
            let mut offenders = Vec::new();
            let mut swept = 0usize;

            for entry in std::fs::read_dir(&dir).expect("builtin_tools").flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                    continue;
                }
                let raw = read_source(&path);
                // Comments discuss the removed guards on purpose.
                let text: String = raw
                    .lines()
                    .filter(|l| !l.trim_start().starts_with("//"))
                    .collect::<Vec<_>>()
                    .join("
");
                swept += 1;

                let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("?");
                if text.contains("MAX_SUBAGENT_DEPTH")
                    || text.contains("ctx.depth >= subagent::pool::MAX_NESTING_DEPTH")
                    || text.contains("ctx.depth + 1 > subagent::pool::MAX_NESTING_DEPTH")
                {
                    offenders.push(name.to_string());
                }
            }

            assert!(
                swept >= 30,
                "only {swept} builtin tools swept — the gate is not reaching the tree"
            );
            assert!(
                offenders.is_empty(),
                "these tools carry their own depth guard: {:?}. `pool.acquire` is                  the first thing `manager::spawn` does and already refuses a child                  past the limit with a message every caller surfaces. A second                  guard in front of it makes the pool's branch unreachable, which                  is the defect `P3-58` was written to remove — and which its own                  first fix preserved.",
                offenders
            );
        }

        // ── P3-60 · the declared tool subset is applied ──────────────────
        assert!(
            registry.contains("pub fn restrict_to"),
            "the registry must be narrowable"
        );
        assert!(
            runner.contains("with_tool_subset(&config.tools)"),
            "`config.tools` must reach the engine — it was written by all three \
             spawn tools and the Tauri command, and read by nothing"
        );

        // ── P3-61 · the model is routed, not assumed ─────────────────────
        assert!(
            runner.contains("pick_servable_model(&provider_id"),
            "the subagent's model must be checked against the provider that \
             will serve it"
        );
        assert!(
            !runner.contains("fallback_model.unwrap_or_else(|| SubagentConfig::DEFAULT_MODEL"),
            "the dead `is_empty()` fallback is back: `manager.rs` fills \
             `config.model` before this runs, so that branch never executes"
        );

        // ── P3-63 · max_turns reaches the budget the engine enforces ─────
        assert!(
            runner.contains("budget.max_turns = Some(config.max_turns)"),
            "`config.max_turns` must reach `TaskBudget`, which is the thing the \
             engine actually enforces"
        );

        // ── P3-62 · the reported cost is the real one, on BOTH arms ──────
        let zero_costs = runner.matches("cost_usd: 0.0").count();
        assert_eq!(
            zero_costs, 0,
            "a SubagentResult still reports zero cost. The QC hourly dollar \
             ceiling is computed from this field, so a zero here makes the \
             ceiling self-cancel on completion and bound nothing but in-flight \
             concurrency"
        );
        assert_eq!(
            runner.matches("cost_usd: accounting.cost_usd").count(),
            2,
            "both the completed AND the failed arm must report real cost — a \
             subagent that timed out at 600s is exactly the expensive one"
        );
    }

    // ── TG-P3-59 · compaction reads the window through the production path ────

    /// The compaction window must be correct **through `Provider::models()`**,
    /// which is what the engine actually calls.
    ///
    /// 🔴 `P3-59` — my own defect from P2. The guarding test asserted on
    /// `capabilities_for(..).context_window`
    /// (`ModelCapabilities::context_window`) while production reads
    /// `ModelInfo::context_window` via `provider.models()`. **Two different
    /// fields**, agreeing only because both derive from one `CatalogueEntry` —
    /// so a regression in `models_for` alone would leave the test green while
    /// Gemini compacted at 200k again, which is the exact bug `P2-57` fixed.
    ///
    /// ⚠ This gate constructs a **real provider** and calls the real trait
    /// method, rather than reaching into the catalogue. `models()` is static
    /// data, so the dummy key never leaves the process and no request is made.
    /// The point is to traverse every hop the engine traverses: a break at any
    /// one of them fails here.
    #[test]
    fn tg_p3_59_the_compaction_window_survives_the_production_path() {
        use autodev_core::provider::traits::Provider;

        // The engine's lookup, reproduced exactly: find the model in
        // `provider.models()` and read `context_window`.
        fn window_via_provider(p: &dyn Provider, model: &str) -> Option<u32> {
            p.models().iter().find(|m| m.id == model).map(|m| m.context_window)
        }

        let google = autodev_core::provider::google::GoogleProvider::new("not-a-real-key".into());

        // Every Google model must survive the trip with a usable window.
        let listed = google.models();
        assert!(
            !listed.is_empty(),
            "the Google adapter lists no models — `provider.models()` is the \
             engine's only source for the window, so this alone would compact \
             every Gemini conversation at the 200k fallback"
        );

        for m in &listed {
            let via = window_via_provider(&google, &m.id).unwrap_or(0);
            assert!(
                via > 0,
                "{} reaches production with no context window",
                m.id
            );
            assert_eq!(
                via,
                autodev_core::provider::catalogue::capabilities_for(&m.id).context_window,
                "{}'s window differs between ModelInfo (what production reads) \
                 and ModelCapabilities (what the old test read). Two fields for \
                 one fact is how the P2 test passed over a live regression",
                m.id
            );
        }

        // The headline case, stated concretely: Gemini's window must not be
        // the 200k fallback.
        let gemini = listed
            .iter()
            .find(|m| m.id.starts_with("gemini"))
            .expect("the catalogue must ship at least one Gemini");
        assert!(
            gemini.context_window > 200_000,
            "{} arrives at the engine with a {}-token window — that IS the \
             unknown-model fallback, so compaction fires at a fifth of the real \
             capacity while looking fixed",
            gemini.id,
            gemini.context_window
        );

        // And the engine must still be reading that field, not a constant.
        let engine = read_source(&repo_root().join("autodev-core/src/agent/engine.rs"));
        assert!(
            engine.contains(".map(|m| m.context_window)"),
            "the engine must read the window from `provider.models()`; a \
             hardcoded limit here is P2-57 reintroduced"
        );
    }

    // ── TG-P3-G5 · a subagent never outranks its parent ───────────────────────

    /// The permission mode is inherited, never hardcoded.
    ///
    /// 🔴 `G-5` (post-phase gap hunt). `native_runner` built every child
    /// with `PermissionMode::Autonomous` regardless of its parent, so a model
    /// held in `ManualApproval` escaped every prompt with one `spawn_subagent`
    /// call — unattended, with `bash`, on the user's own key under BYOK.
    ///
    /// ⚠ The comment justifying it read *"a subagent has no user to prompt for
    /// permission"*, and two lines below, `deps.pending` — the permission
    /// channel itself — was passed in. A justification contradicted by the
    /// line beneath it is the tell.
    #[test]
    fn tg_p3_g5_a_subagent_inherits_its_parents_permission_mode() {
        let runner = read_source(
            &repo_root().join("autodev-core/src/subagent/native_runner.rs"),
        );

        let engine_call = runner
            .split("let engine = AgentEngine::new(")
            .nth(1)
            .expect("the runner must build an engine");
        let args = engine_call.split(");").next().unwrap_or(engine_call);

        assert!(
            args.contains("config.permission_mode"),
            "the subagent engine's mode must come from its config, which the              spawn tools fill from the PARENT's context. A literal here is a              one-call escape from whatever the user chose."
        );
        assert!(
            !args.contains("PermissionMode::Autonomous"),
            "the runner hardcodes Autonomous again"
        );

        // ── and the tools must actually pass it down ────────────────────
        let dir = repo_root().join("autodev-core/src/agent/builtin_tools");
        let mut spawners = 0usize;
        let mut offenders = Vec::new();

        for entry in std::fs::read_dir(&dir).expect("builtin_tools").flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = read_source(&path);
            if !text.contains("SubagentConfig {") {
                continue;
            }
            spawners += 1;
            let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("?");
            if !text.contains("permission_mode: ctx.permission_mode") {
                offenders.push(name.to_string());
            }
        }

        assert!(
            spawners >= 3,
            "only {spawners} spawn tools found — the sweep is not reaching them"
        );
        assert!(
            offenders.is_empty(),
            "these spawn tools do not pass the caller's permission mode to the              child, so the child's policy is whatever the config defaults to:              {:?}",
            offenders
        );
    }

    // ── TG-P3-G8 · every tool that spawns a shell scrubs credentials ──────────

    /// A tool that runs a model-chosen shell command must not hand it the
    /// process's credentials.
    ///
    /// 🔴 `G-8` (post-phase gap hunt). `bash` strips credentials from the
    /// child environment; `e2e_probe`, which spawns the same `sh -c` /
    /// `cmd /C` with commands the model wrote, did not — so its children
    /// inherited every provider key. `echo $ANTHROPIC_API_KEY` needs no
    /// vulnerability beyond an inherited environment.
    ///
    /// ⚠ `bash`'s own comment records that its first version of this landed in
    /// the Windows branch only, "which is its own small lesson about two-branch
    /// fixes". G-8 is the same lesson one tool over. This gate is keyed on
    /// *"spawns a shell"* rather than on the two tools that do it today, so the
    /// third one fails the build instead of repeating the lesson a third time.
    #[test]
    fn tg_p3_g8_every_shell_spawning_tool_scrubs_credentials() {
        let dir = repo_root().join("autodev-core/src/agent/builtin_tools");
        let mut offenders = Vec::new();
        let mut spawners = 0usize;

        for entry in std::fs::read_dir(&dir).expect("builtin_tools").flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = read_source(&path);
            let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("?");

            // Does it hand a command string to a shell?
            let spawns_shell = (text.contains("Command::new(\"sh\")")
                || text.contains("Command::new(\"cmd\")"))
                && (text.contains("\"-c\"") || text.contains("\"/C\""));
            if !spawns_shell {
                continue;
            }
            spawners += 1;

            if !text.contains("env_scrub::looks_like_a_credential") {
                offenders.push(name.to_string());
            }
        }

        assert!(
            spawners >= 2,
            "only {spawners} shell-spawning tools found — the sweep is not              reaching them, and an empty sweep passes silently"
        );
        assert!(
            offenders.is_empty(),
            "these tools spawn a shell with a model-chosen command and do NOT              strip credentials from the child's environment: {:?}",
            offenders
        );
    }

    // ── TG-P3-F8 · a gate id cited in code must be a gate that exists ─────────

    /// Shipped code may not cite a gate that does not exist.
    ///
    /// 🔴 `F-8` (post-phase claims audit). Eight `TG3-xx` ids were cited in
    /// production comments as though they were live; six of them are still
    /// unchecked, and `TG3-01` — cited **nine times**, including as `P3-05`'s
    /// entire `GATE-PROVEN` evidence — had no implementation at all.
    ///
    /// ⚠ A citation is load-bearing here in a way it usually is not. These
    /// comments are how the next reader decides a behaviour is already
    /// protected, so a dangling id does not merely mislead — it stops the
    /// protection ever being written. The repo has logged this once before, for
    /// `TG-P2-05`.
    ///
    /// This asserts EXISTENCE, not checkedness: citing a planned gate is
    /// legitimate ("this is what TG3-27 will assert"), citing a fiction is not.
    #[test]
    fn tg_p3_f8_every_cited_gate_id_exists_in_the_register() {
        let register = read_source(
            &repo_root().join("docs/restructurization/development/TESTING_GATES.md"),
        );

        let mut cited: std::collections::BTreeSet<String> = Default::default();
        let mut scanned = 0usize;

        let mut stack = vec![
            repo_root().join("autodev-core/src"),
            repo_root().join("src-tauri/src"),
        ];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                let path = e.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|x| x.to_str()) != Some("rs") {
                    continue;
                }
                scanned += 1;
                let text = read_source(&path);
                // `TG3-07`, `TG-P3-11`, `TG4-18` … capture the whole token.
                let bytes: Vec<char> = text.chars().collect();
                let mut i = 0;
                while i + 3 < bytes.len() {
                    if bytes[i] == 'T' && bytes[i + 1] == 'G' {
                        let mut j = i + 2;
                        let mut tok = String::from("TG");
                        while j < bytes.len()
                            && (bytes[j].is_ascii_alphanumeric() || bytes[j] == '-')
                        {
                            tok.push(bytes[j]);
                            j += 1;
                        }
                        // `TG3-xx` and friends are placeholders in prose, not
                        // citations. A real id ends in digits.
                        let is_placeholder = !tok
                            .chars()
                            .last()
                            .map(|c| c.is_ascii_digit())
                            .unwrap_or(false);
                        if tok.len() > 4
                            && tok.contains('-')
                            && tok.chars().any(|c| c.is_ascii_digit())
                            && !is_placeholder
                        {
                            cited.insert(tok.trim_end_matches('-').to_string());
                        }
                        i = j;
                        continue;
                    }
                    i += 1;
                }
            }
        }

        assert!(
            scanned > 50,
            "only {scanned} source files scanned — the sweep is not reaching the tree"
        );

        // `TG-P3-*` are this repo's implementation gates and live in ci_gate.rs
        // itself; the register tracks the SPEC gates (`TG<phase>-<nn>`).
        let dangling: Vec<&String> = cited
            .iter()
            .filter(|id| !id.starts_with("TG-P"))
            .filter(|id| !register.contains(id.as_str()))
            .collect();

        assert!(
            dangling.is_empty(),
            "these gate ids are cited in shipped code and appear nowhere in              TESTING_GATES.md. A reader takes such a citation as proof the              behaviour is already protected, so a dangling one does not merely              mislead — it stops the protection ever being written: {:?}",
            dangling
        );
    }

    // ── TG-P4-31 · no shipped recipe is stranded on a mode that cannot run ────

    /// While Mode 2 is inert, no shipped recipe may declare `Mode2Only`.
    ///
    /// 🔴 `P4-31`. Eight of the nine shipped recipes declared `Mode2Only`
    /// against a Mode 2 that `runner.rs` hardcodes off (`orch_healthy = false`,
    /// since P0 deleted the in-process MCP server it reached tools through).
    /// Every tick fell to the fallback-policy check, logged
    /// `fallback_skipped_policy`, and returned `Ok(())` — so **the fleet
    /// reported itself idle rather than broken**, indefinitely.
    ///
    /// ⚠ The declarations were never updated when Mode 2 went inert, and
    /// nothing connected the two: `runner.rs` states *"every worker therefore
    /// runs on the in-process Rust engine (Mode 1)"* while the data said
    /// otherwise. This gate is that connection — it fails if the two ever
    /// disagree again.
    ///
    /// It is conditional on purpose: if Mode 2 is ever revived,
    /// `orch_healthy` stops being hardcoded and `Mode2Only` becomes meaningful
    /// again. The gate keys on the runner's actual state, not on a preference.
    /// `TG-P4-31`, re-pointed by `P8-45`.
    ///
    /// 🔴 **What it used to assert, and why that could not survive `P8`.** It
    /// read the literal `"let orch_healthy = false"` out of `runner.rs` and
    /// `panic!`d when it was absent — deliberately, so that reviving Mode 2
    /// would have to be an explicit decision rather than a silent pass. Then
    /// it counted `FallbackPolicy::Mode1Capable` declarations in `defaults.rs`
    /// and required at least nine.
    ///
    /// `P8-47` deletes the enum, so **the property became unstateable**: there
    /// is no `Mode1Capable` left to count, and the `declared >= 9` floor — the
    /// assertion that the sweep was reaching anything at all — would have been
    /// the first thing in the phase to go red.
    ///
    /// ⚠ The task predicted that failure in the wrong commit. It said the gate
    /// *"fails CI on the first commit of `P8-B`"*; `P8-B` is `P8-04`…`P8-12`
    /// and touches **none** of the two files this gate reads. A builder
    /// trusting that justification would have looked for the break in the
    /// wrong place and concluded the gate was fine.
    ///
    /// **So it now asserts the deletion instead**, in the shape of
    /// `tg_p3_39_the_memory_gateway_is_the_only_write_path` above: the thing
    /// is gone, and staying gone is the invariant. `P8-53` retires it.
    #[test]
    fn tg_p4_31_mode2_and_its_fallback_policy_are_gone() {
        let types = read_source(&repo_root().join("autodev-core/src/recipes/types.rs"));
        let runner = read_source(&repo_root().join("autodev-core/src/recipes/runner.rs"));
        let defaults = read_source(&repo_root().join("autodev-core/src/recipes/defaults.rs"));

        // ⚠ `read_source` is `unwrap_or_default()`, so a MISSING file reads as
        // an empty string and every "is it gone?" assertion below would pass
        // vacuously. Prove the files were actually read before asserting on
        // their contents — `L-C9`, a check whose shape cannot express failure.
        for (name, src) in [
            ("types.rs", &types),
            ("runner.rs", &runner),
            ("defaults.rs", &defaults),
        ] {
            assert!(
                src.len() > 500,
                "{name} read as {} bytes — the gate is not reading the file, \
                 so every assertion below would pass vacuously",
                src.len()
            );
        }

        // Strip comments AND everything from `#[cfg(test)]` onward.
        //
        // 🔴 Without the second half this gate FAILED ON ITSELF the
        // moment `P8-25`'s tests landed: those fixtures contain
        // `fallback_policy: mode2_only` as YAML, to prove that recipes
        // already on disk still parse. The gate read its own evidence as
        // the defect. `L-C7`, and `L-8` prescribes exactly this remedy --
        // truncate at the test module, or build the needle at runtime.
        let code = |src: &str| -> String {
            let body = match src.find("#[cfg(test)]") {
                Some(i) => &src[..i],
                None => src,
            };
            body.lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        assert!(
            !code(&types).contains("enum FallbackPolicy"),
            "`FallbackPolicy` is back in types.rs. It defaulted to `Mode2Only`, \
             and the runner refused to run any recipe carrying that policy — so \
             every recipe saved from the UI, which omits the key, deserialised \
             as permanently idle. P8-47 deleted it; reviving it must be a \
             deliberate decision, not a merge."
        );
        assert!(
            !code(&types).contains("fallback_policy"),
            "`Recipe.fallback_policy` is back. Old YAML on disk still carries \
             the key and is ignored by serde; re-adding the field would strand \
             those recipes again."
        );
        assert!(
            !code(&runner).contains("fallback_skipped_policy"),
            "the Mode2Only guard is back in runner.rs — it returned \
             `fallback_idle` on every tick, forever, so the fleet reported \
             itself idle rather than broken."
        );
        assert!(
            !code(&defaults).contains("fallback_policy"),
            "a shipped recipe declares a fallback policy again"
        );
    }

    // ── TG-P3-39 · the gateway replaced the unrestricted write ────────────────

    /// `remember_this` is gone, and the four gateway surfaces exist.
    ///
    /// 🔴 `P3-39` / `P3-47`. `remember_this` wrote into the user's long-term
    /// memory at `confidence = 1.0` labelled `user_explicit`, from **every** PA
    /// mode — and `recall_facts` had no scope predicate and ordered by
    /// confidence. So a fleet worker's assertion landed at the top of the
    /// interactive user's *"What I know about you and this project"*,
    /// indistinguishable from something the user had said. `P4-31` made that
    /// path live earlier the same day.
    ///
    /// ⚠ Asserts the tool is **deleted**, not deprecated. An unreferenced
    /// second write path is a defect waiting for a caller — which is `TG3-07`'s
    /// own phrasing, proven right once already this phase.
    #[test]
    fn tg_p3_39_the_memory_gateway_is_the_only_write_path() {
        let dir = repo_root().join("autodev-core/src/agent/builtin_tools");

        assert!(
            !dir.join("remember_this.rs").exists(),
            "remember_this.rs still exists. It gave any agent an unrestricted \
             write at the user's own confidence and provenance; P3-39 removes \
             it rather than deprecating it."
        );

        let registry = read_source(&dir.join("mod.rs"));
        let code: String = registry
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            !code.contains("RememberThisTool"),
            "the registry still registers RememberThisTool"
        );

        // All four surfaces spec `03 §4.5` names.
        for tool in [
            "SearchMemoryTool",
            "GetMemoryTool",
            "ListMemoriesTool",
            "ProposeMemoryTool",
        ] {
            assert!(
                code.contains(tool),
                "the gateway is incomplete: {tool} is not registered"
            );
        }
    }

    /// An agent cannot elect its own provenance or confidence.
    ///
    /// 🔴 `P3-47`. The ceiling is only meaningful if the proposer is
    /// **derived** from the runtime's identity. A schema field for `source`,
    /// `proposer` or `importance` would let a model name itself the user, and
    /// the whole control would be decorative.
    ///
    /// ⚠ It WAS decorative, and this gate was green over it for the whole of
    /// `P3-F`. See the two comments in the body: the blocklist missed the one
    /// key that mattered, and the call-site check matched a prefix. Both are
    /// repaired here, and the load-bearing assertions are now structural —
    /// they constrain the shape of the call and of the signature, not the
    /// vocabulary of the schema.
    #[test]
    fn tg_p3_47_provenance_is_derived_not_declared() {
        let tool = read_source(
            &repo_root().join("autodev-core/src/agent/builtin_tools/propose_memory.rs"),
        );

        // The schema must not offer the model a way to claim provenance.
        let schema = tool
            .split("fn input_schema")
            .nth(1)
            .expect("propose_memory must declare a schema");
        let schema_body = schema.split("async fn execute").next().unwrap_or(schema);
        // 🔴 This list used to hold exactly three names, and the gate was
        // GREEN while `"user_stated"` sat in that schema doing precisely what
        // the three forbid: `propose_memory` read it out of the model's tool
        // input and `from_agent_role` short-circuited to `Proposer::User` on
        // it — ceiling 1.0, `source = "user"`, rendered to the user as
        // "(you told me)". A fleet worker could write a global memory at the
        // user's own authority. `L-27`: an enumerating check is blind to the
        // subject nobody put on the list.
        //
        // The list stays because it is cheap, but it is no longer what makes
        // this gate work. The two assertions below are.
        for forbidden in [
            "\"source\":",
            "\"proposer\":",
            "\"importance\":",
            "\"user_stated\":",
        ] {
            assert!(
                !schema_body.contains(forbidden),
                "propose_memory's schema exposes {forbidden} — provenance and \
                 importance are assigned from the caller's identity, never taken \
                 from the caller"
            );
        }

        // ⚠ The CLOSING PAREN is the assertion.
        //
        // The previous version asserted `contains("...from_agent_role(&ctx.agent_role")`
        // — a prefix, which matched happily when the call was
        // `from_agent_role(&ctx.agent_role, user_stated)`. It inspected the
        // receiver and ignored the argument that overrode it (`L-C6`: a check
        // written from the claim rather than from the invariant).
        //
        // Requiring the call to END there means no second argument exists, so
        // no input CAN reach the decision — whatever the schema grows next.
        assert!(
            tool.contains("Proposer::from_agent_role(&ctx.agent_role)"),
            "the proposer must be derived from the runtime's agent role ALONE — \
             any further argument is a channel through which a model could name \
             its own provenance"
        );

        // …and the same invariant at the definition, so a second parameter
        // cannot be reintroduced and then merely passed a constant.
        let gw_src = read_source(&repo_root().join("autodev-core/src/memory/gateway.rs"));
        assert!(
            gw_src.contains("pub fn from_agent_role(role: &str) -> Self"),
            "from_agent_role must take the role and nothing else; its signature \
             is what enforces that provenance is derived rather than declared"
        );

        // …and the ceiling itself must still discriminate.
        let gw = read_source(&repo_root().join("autodev-core/src/memory/gateway.rs"));
        assert!(
            gw.contains("fn confidence_ceiling"),
            "the confidence ceiling is what stops an agent outranking the user"
        );
        assert!(
            gw.contains("clamp(0.0, ceiling)"),
            "the claimed confidence must be CLAMPED by the ceiling, not merely \
             compared against it"
        );
    }

    /// Every writer into memory redacts on the way in — **discovered, not
    /// listed**.
    ///
    /// 🔴 `P3-F` post-phase audit. Three tier stores called
    /// `security::redaction` before insert, each with a comment naming the
    /// reason: *"a store that never held the secret cannot leak it through a
    /// path nobody thought of."* The gateway — the ONLY live writer into the
    /// table recall actually reads — called nothing. So the single write path
    /// that could reach the system prompt was the one that skipped the
    /// boundary, and a `.env` or a token read out of a tool result could be
    /// persisted verbatim, for ever, and re-sent to the provider on every
    /// later turn in that scope.
    ///
    /// ⚠ This gate **finds** the writers rather than naming them. A gate
    /// written over the fix would list `gateway.rs` and stay green while a
    /// fifth writer arrived unredacted next phase — which is exactly the shape
    /// that let this defect through in the first place. Any file that inserts
    /// into a memory table must redact; adding one without redaction fails
    /// here, with no list to remember to update.
    #[test]
    fn tg_p3_48_every_memory_writer_redacts_on_the_way_in() {
        let dir = repo_root().join("autodev-core/src/memory");
        let mut writers: Vec<(String, String)> = Vec::new();

        for entry in std::fs::read_dir(&dir).expect("memory module").flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let src = read_source(&p);
            let name = p.file_name().unwrap().to_string_lossy().to_string();

            // `db.rs` is the migration ladder: its inserts are SQL backfills of
            // rows the tier stores already redacted on the way in, so there is
            // no unredacted content entering there.
            if name == "db.rs" {
                continue;
            }
            // Production code only. `backend.rs` seeds `memories` directly from
            // its own `#[cfg(test)]` fixtures, which is not a leak path — and
            // finding it is this gate working rather than misfiring: it named a
            // writer that no list of mine contained.
            //
            // Comments go too, or the gate trips on the prose explaining it,
            // which has now happened four separate times in this file.
            let production = src.split("#[cfg(test)]").next().unwrap_or(&src).to_string();
            let code: String = production
                .lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");

            let inserts = ["INSERT INTO memories", "INSERT INTO episodic_memories",
                           "INSERT INTO semantic_memories", "INSERT INTO procedural_memories"];
            if inserts.iter().any(|i| code.contains(i)) {
                writers.push((name, code));
            }
        }

        assert!(
            writers.len() >= 4,
            "expected at least the three tier stores and the gateway; found {} — \
             this gate has stopped finding its own subject",
            writers.len()
        );

        for (name, code) in &writers {
            assert!(
                code.contains("redaction::redact") || code.contains("redact_text")
                    || code.contains("redact_json") || code.contains("redact_value_text"),
                "{name} inserts into a memory table without redacting first — memory \
                 is permanent and is replayed into the system prompt, so a secret \
                 written here is a secret kept and re-sent for ever"
            );
        }
    }

    /// A recalled memory is **quoted** into the prompt, never allowed to author
    /// it.
    ///
    /// 🔴 `P3-F` post-phase audit. Memory content is model-written and
    /// attacker-influenceable — the model proposes memories from repository
    /// files, tool output and subagent results, all of which a hostile repo
    /// controls. `render` spliced that text in raw, and `engine.rs` appends the
    /// block LAST: the highest-recency position in the system message, after
    /// the base prompt's own prohibitions, with nothing following to re-assert
    /// authority. A memory containing a newline and a `##` heading turned a
    /// one-shot injection into a persistent system-prompt implant, re-injected
    /// every turn — and with `scope: "global"`, into every other repository the
    /// user opens.
    #[test]
    fn tg_p3_49_recalled_memory_cannot_author_the_prompt() {
        let recall = read_source(&repo_root().join("autodev-core/src/memory/recall.rs"));
        // ⚠ Production code only — `L-25`, caught by falsifying this very gate.
        // The first version scanned the whole file, and the third assertion
        // below passed over a planted defect because `injection_tests` asserts
        // the same string it was looking for. A corpus that contains the claim
        // cannot corroborate the claim.
        let production = recall.split("#[cfg(test)]").next().unwrap_or(&recall).to_string();
        let code: String = production
            .lines()
            .filter(|l| !l.trim_start().starts_with("///") && !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            code.contains("fn flatten_for_prompt"),
            "recall must flatten stored content before it reaches the prompt"
        );
        // ⚠ The invariant is that the RENDERED row is flattened, not merely
        // that a flattener exists somewhere in the file — `L-C12`, a helper
        // wired to nothing, which this phase has already produced once.
        assert!(
            code.contains("truncate(&flatten_for_prompt(&c.content)"),
            "the rendered row does not pass through the flattener — the function \
             exists but the prompt is still built from raw stored content"
        );
        // …and the block says what it is, because a model that is not told the
        // text is data has no reason to treat it as data.
        assert!(
            code.contains("not instructions"),
            "the memory block must frame its contents as data rather than as \
             instructions from the user"
        );
    }

    /// Recall is scoped, and scoped means "nothing" when no scope is given.
    ///
    /// 🔴 `P3-31`. `recall_facts` filtered on `is_active` and `confidence`
    /// only — `working_dir` was SELECTed and never used as a predicate. Every
    /// project recalled every other project's memories.
    #[test]
    fn tg_p3_31_every_retrieval_path_applies_scope() {
        let backend = read_source(&repo_root().join("autodev-core/src/memory/backend.rs"));
        assert!(
            backend.contains("if q.scope_keys.is_empty()"),
            "the backend must handle an empty scope explicitly"
        );
        assert!(
            backend.contains("return Ok(Vec::new())"),
            "an empty scope must return NOTHING. Treating 'no scope' as 'all \
             scopes' is exactly how cross-project contamination happens."
        );

        // The tools that read memory must scope too, or they are a way around
        // the filter the prompt path enforces.
        for tool in ["get_memory.rs", "list_memories.rs"] {
            let src = read_source(
                &repo_root()
                    .join("autodev-core/src/agent/builtin_tools")
                    .join(tool),
            );
            assert!(
                src.contains("scope_keys_for(&ctx.working_dir)"),
                "{tool} does not scope its reads — an id from another project \
                 would be readable by guessing it"
            );
        }
    }

    /// Memory reaches the prompt on the override path too.
    ///
    /// 🔴 `P3-45`. An override returned an empty VOLATILE half, and the
    /// memory block lives there — so a user-facing settings field silently
    /// disabled all recall, and recipes and subagents (which both pass an
    /// override) had none for the same reason.
    ///
    /// ⚠ `P3-44`'s original wording — *"a recalled memory appears in the
    /// outbound prompt bytes"* — was already true and proved nothing, because
    /// recall was query-blind: the same bytes appeared whatever was asked. This
    /// asserts the two properties that were actually missing.
    #[test]
    fn tg_p3_44_45_memory_reaches_the_prompt_and_depends_on_the_query() {
        let engine = read_source(&repo_root().join("autodev-core/src/agent/engine.rs"));
        let ctx = read_source(&repo_root().join("autodev-core/src/agent/context.rs"));

        // The override branch must assemble memory rather than empty it.
        // ⚠ Comments stripped FIRST. This is the third time a gate of mine has
        // tripped on prose describing the very fix it checks for (`glob`'s
        // gitignore disclaimer, the removed depth constant, and this branch's
        // "used to be String::new()" note). Recorded as a named sub-pattern of
        // `L-27`: the comment explaining a fix names the defect, so a gate that
        // reads comments finds the defect in its own explanation.
        let engine_code: String = engine
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("
");
        let branch = engine_code
            .split("match system_prompt_override")
            .nth(1)
            .expect("the override branch must exist");
        let branch = branch.split("};").next().unwrap_or(branch);
        assert!(
            branch.contains("memory_block(working_dir, user_message)"),
            "the override path does not assemble memory. It returns the \
             volatile half, and the memory block lives there — so a custom \
             system prompt silently disables all recall."
        );
        assert!(
            !branch.contains("String::new()"),
            "the override branch still empties the volatile half"
        );

        // The query must be threaded, not hardcoded.
        assert!(
            ctx.contains("pub fn memory_block(&self, repo_path: &str, query: &str)"),
            "memory_block must take a query"
        );
        let ctx_code: String = ctx
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("
");
        assert!(
            !ctx_code.contains("build_memory_context(sid, \"\", repo_path)"),
            "the empty-string query is back. That single literal made recall \
             byte-identical every turn, whatever the user asked."
        );
    }

    // ── TG-P3-33 · the supply-chain surface is opt-in and pinned ──────────────

    /// The embedding dependency cannot enter a build by accident.
    ///
    /// 🔴 `P3-33`. `fastembed` fetches the ONNX Runtime from a third-party CDN
    /// **at build time**. The task prescribed disabling `ort`'s default
    /// features — but `fastembed` already depends on `ort` with
    /// `default-features = false`, and `ort` is not a direct dependency of ours,
    /// so **the prescribed fix would have changed nothing while looking like a
    /// fix**. The flag is on `fastembed`.
    ///
    /// razi-ruled: off by default, so a default build and CI fetch nothing, and
    /// the vendoring decision stays open rather than being made silently by
    /// adding a dependency.
    #[test]
    fn tg_p3_33_the_embedding_dependency_is_optional_and_exactly_pinned() {
        let manifest = read_source(&repo_root().join("autodev-core/Cargo.toml"));
        let code: String = manifest
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("
");

        let dep_line = code
            .lines()
            .find(|l| l.trim_start().starts_with("fastembed"))
            .expect("fastembed must be declared");

        // ⚠ Defence in depth, and NOT independently falsifiable — stated so
        // because I tried. Removing `optional = true` makes Cargo reject the
        // manifest outright, since `embeddings = ["dep:fastembed"]` requires an
        // optional dependency. The plant produced a manifest error rather than
        // a gate failure, so Cargo enforces this more strongly than the gate
        // does. Kept because it states the requirement where a reader will find
        // it; credited honestly, because "falsified" would have been false
        // (`§0.16 L-26`).
        assert!(
            dep_line.contains("optional = true"),
            "fastembed must be OPTIONAL. A non-optional dependency fetches ~30 MB              from a third-party CDN on every build, including CI: {dep_line}"
        );
        assert!(
            dep_line.contains("\"=") || dep_line.contains("= \"="),
            "fastembed must be pinned EXACTLY (`=x.y.z`). A supply-chain surface              is not something to let float on a caret range: {dep_line}"
        );

        // The feature must exist and must NOT be in `default`.
        assert!(
            code.contains("embeddings = [\"dep:fastembed\"]"),
            "the `embeddings` feature must gate the dependency"
        );
        for line in code.lines() {
            let t = line.trim();
            if t.starts_with("default") && t.contains('=') {
                assert!(
                    !t.contains("embeddings"),
                    "`embeddings` is in the default feature set — a default build                      would then fetch from the CDN: {t}"
                );
            }
        }

        // …and the vector backend must be compiled out on macOS regardless,
        // because no x86_64 ONNX Runtime exists upstream and the DMG is a
        // universal binary: enabling it there fails the build outright.
        let manager = read_source(&repo_root().join("autodev-core/src/memory/manager.rs"));
        assert!(
            manager.contains("not(target_os = \"macos\")"),
            "the vector backend must be excluded on macOS at compile time"
        );
    }

    // ── TG-P3-17 · no validator subprocess runs without a ceiling ─────────────

    /// Every subprocess the validator spawns must go through `run_bounded`.
    ///
    /// 🔴 There were **five** spawn sites in `validate.rs` and **not one had a
    /// timeout** — including the `pyright --version` availability probe. These
    /// run inside `write_file`, so a checker that hangs hangs the write.
    ///
    /// ⚠ Keyed on the **invariant** (`no bare .output().await outside the one
    /// bounded helper`), not on the presence of the fix. A gate that merely
    /// looked for the string `run_bounded` would pass while a **new** sixth
    /// spawn site sat beside it, unbounded — which is exactly the shape that
    /// let `TG-P3-11` pass over a planted defect (`§0.16 L-C6`).
    #[test]
    fn tg_p3_17_every_validator_subprocess_is_bounded() {
        let path = repo_root().join("autodev-core/src/agent/builtin_tools/validate.rs");
        let text = read_source(&path);

        // The single legitimate `.output()` lives inside `run_bounded`, which
        // wraps it in `tokio::time::timeout` and sets `kill_on_drop`.
        let bounded_helper = text
            .split("async fn run_bounded")
            .nth(1)
            .expect("run_bounded must exist — it is the only sanctioned spawn path");
        let helper_body = bounded_helper
            .split("\n}\n")
            .next()
            .expect("run_bounded must have a body");

        assert!(
            helper_body.contains("tokio::time::timeout"),
            "run_bounded must actually impose a ceiling"
        );
        assert!(
            helper_body.contains("kill_on_drop"),
            "without kill_on_drop the timed-out child is orphaned and keeps running"
        );

        // Now: every OTHER `.output()` in the file is an unbounded spawn.
        let total_output_calls = text.matches(".output()").count();
        let in_helper = helper_body.matches(".output()").count();

        assert_eq!(
            in_helper, 1,
            "run_bounded should contain exactly one `.output()`; found {in_helper}"
        );
        assert_eq!(
            total_output_calls, 1,
            "found {} unbounded validator subprocess spawn(s) — every \
             `tokio::process::Command` in validate.rs must go through \
             `run_bounded`, or a hanging checker hangs the write that called it",
            total_output_calls - in_helper
        );

        // And the ceiling must be a real bound, not `Duration::MAX`.
        assert!(
            text.contains("const CHECKER_TIMEOUT"),
            "the ceiling must be a named constant, not scattered literals"
        );
    }

    /// A checker that did not run must never be reportable as clean.
    ///
    /// 🔴 `is_clean()` was `!any(severity == "error")` while a missing binary
    /// produced a `"warning"` — so an uninstalled `tsc` rendered as
    /// `"✓ No syntax errors."`. Paired with `TG-P3-17` because the timeout
    /// alone would have routed a **hang** into that same branch.
    #[test]
    fn tg_p3_18_a_check_that_did_not_run_cannot_report_clean() {
        let path = repo_root().join("autodev-core/src/agent/builtin_tools/validate.rs");
        let text = read_source(&path);

        let is_clean = text
            .split("pub fn is_clean(&self) -> bool {")
            .nth(1)
            .expect("LintResult::is_clean must exist");
        let body = is_clean.split("    }").next().expect("is_clean body");

        assert!(
            body.contains("self.ran()"),
            "is_clean must consult whether the checker ran; without it, \
             'not checked' and 'checked and fine' are the same verdict"
        );

        // The severity must be its own value. Folding it back into "warning"
        // would silently restore the defect while leaving `ran()` in place.
        assert!(
            text.contains("SEVERITY_UNAVAILABLE"),
            "the did-not-run state needs a severity of its own"
        );
        assert!(
            !text.contains("severity: \"warning\",\n                message: format!(\n                    \"Could not run"),
            "a missing binary must not be a warning"
        );
    }

    // ── TG-P3-01 · the release profile is where Cargo will read it ────────────

    /// `[profile.release]` must live at the **workspace root**.
    ///
    /// **`P3-01` was a no-op as written.** The block lived in
    /// `src-tauri/Cargo.toml`, a non-root member, and Cargo discards those —
    /// it says so on every invocation:
    ///
    /// > `warning: profiles for the non root package will be ignored, specify`
    /// > `profiles at the workspace root`
    ///
    /// So `panic = "abort"` was **never in effect**, and the phase's headline
    /// fault-isolation task was to edit a line that does nothing. `lto`,
    /// `codegen-units`, `opt-level` and `strip` were inert for the same reason,
    /// so every release build shipped un-LTO'd and unstripped.
    ///
    /// The gate asserts the location, not the values — a setting in the wrong
    /// file is indistinguishable from no setting at all, and that is the defect.
    #[test]
    fn tg_p3_01_the_release_profile_is_at_the_workspace_root() {
        let root = read_source(&repo_root().join("Cargo.toml"));

        assert!(
            root.contains("[profile.release]"),
            "the workspace root declares no [profile.release]; any profile in a member \
             crate is silently discarded by Cargo"
        );
        assert!(
            root.contains("panic = \"unwind\""),
            "the root profile does not set panic = \"unwind\". Fault isolation (M-41) \
             depends on unwinding, and this is the file Cargo actually reads"
        );

        // And no member may re-declare one, because a member's block looks
        // authoritative in review and is ignored at build time.
        for member in ["src-tauri", "autodev-core", "src-cli", "src-shared"] {
            let path = repo_root().join(member).join("Cargo.toml");
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let declares = text
                .lines()
                .any(|l| l.trim_start().starts_with("[profile."));
            assert!(
                !declares,
                "{member}/Cargo.toml declares a [profile.*] block. Cargo ignores it, so it \
                 is a setting that reads as active and is not"
            );
        }
    }

    // ── TG-P3-04 · no site slices a string by a raw byte index ────────────────

    /// Truncation must go through the shared UTF-8-safe helper.
    ///
    /// Slicing a `&str` at a byte index that is not a character boundary
    /// **panics**. Four sites did exactly that on strings that routinely carry
    /// non-ASCII — and `bash.rs` is the one most likely to fire, because rustc
    /// and cargo emit box-drawing characters in ordinary diagnostics.
    ///
    /// ⚠ The gate asserts the **invariant** (no raw byte-index slicing of a
    /// string anywhere), not that those four sites were fixed. Written the
    /// other way it would pass for a fifth site added tomorrow — which is
    /// `L-C6`, and this codebase has six recorded instances of it.
    #[test]
    fn tg_p3_04_no_string_is_sliced_by_a_raw_byte_index() {
        let roots = [
            repo_root().join("autodev-core/src"),
            repo_root().join("src-tauri/src"),
            repo_root().join("src-cli/src"),
        ];

        // `&name[..N]`, `&name[N..]`, `name[expr..]` where the index is not a
        // literal 0 and the expression is not obviously a byte-slice.
        let pattern = regex_lite_slice();
        let mut offenders = Vec::new();
        let mut scanned = 0usize;

        for root in &roots {
            let mut stack = vec![root.clone()];
            while let Some(dir) = stack.pop() {
                let Ok(entries) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                        continue;
                    }
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !name.ends_with(".rs") || name.ends_with("_tests.rs") {
                        continue;
                    }
                    // The helper itself is the one place allowed to do this.
                    if name == "truncate.rs" {
                        continue;
                    }

                    let text = read_source(&path);
                    scanned += 1;
                    let rel = path
                        .strip_prefix(repo_root())
                        .unwrap_or(&path)
                        .display()
                        .to_string()
                        .replace('\\', "/");

                    let production = match text.find("#[cfg(test)]") {
                        Some(i) => &text[..i],
                        None => &text[..],
                    };

                    for (n, line) in production.lines().enumerate() {
                        let code = line.split("//").next().unwrap_or("");
                        if pattern(code) {
                            offenders.push(format!("{rel}:{}: {}", n + 1, code.trim()));
                        }
                    }
                }
            }
        }

        assert!(
            scanned > 100,
            "only {scanned} files scanned — this gate is not reaching the tree"
        );

        assert!(
            offenders.is_empty(),
            "these slice a string by a raw byte index, which panics when the index falls \
             inside a UTF-8 character — route them through security::truncate:\n{}",
            offenders.join("\n")
        );
    }

    /// Detects a string sliced at an index that could fall **inside** a
    /// character.
    ///
    /// ── The distinction that makes this gate usable ──────────────────────────
    ///
    /// Not every `&s[i..]` is a hazard, and a gate that says so gets muted.
    ///
    /// * An index from `str::find(..)`, `position(..)` or `char_indices()` is
    ///   **always a valid boundary** — `find` returns the start of a match, and
    ///   a match on an ASCII pattern lands on a boundary by construction. The
    ///   first draft of this gate flagged `graph/changes.rs:55`
    ///   (`&line[plus_idx + 1..]`, where `plus_idx` comes from `find('+')`) and
    ///   `bash.rs`'s own full-range `[..]`. Both are correct code.
    /// * An index derived from **`len()` arithmetic** or a numeric literal is
    ///   arbitrary, and that is precisely how all five real sites were written.
    ///
    /// So the rule is: flag a literal or a `len()`-derived index; ignore one
    /// that came from a boundary-returning search. This is `TG3-28`'s paired
    /// no-fire discipline applied to a static check — a gate that refuses
    /// everything passes a refusal-only test and breaks the build for correct
    /// code.
    fn regex_lite_slice() -> impl Fn(&str) -> bool {
        |code: &str| {
            if !code.contains('[') {
                return false;
            }
            // A full range is always safe.
            if code.contains("[..]") {
                return false;
            }
            // Bytes and vectors are not strings.
            if code.contains("as_bytes()") || code.contains("Vec<") || code.contains("&self.0[") {
                return false;
            }
            // An index from a boundary-returning search is safe by construction.
            if code.contains("find(")
                || code.contains("position(")
                || code.contains("char_indices")
                || code.contains("is_char_boundary")
                || code.contains("truncate_bytes")
            {
                return false;
            }

            // The string bindings this codebase slices, named rather than
            // guessed: a broad heuristic flags every Vec slice in the tree.
            let touches_a_string = [
                "&fact[", "&note[", "&output[", "&command[", "&text[", "&s[", "&content[",
                "&combined[", "&stdout[", "&stderr[", "&line[", "&body[", "&prompt[",
            ]
            .iter()
            .any(|b| code.contains(b));

            if !touches_a_string {
                return false;
            }

            // Arbitrary only if the index is a literal or comes from `len()`.
            let has_literal_index = code
                .split(&['[', ']', '.'][..])
                .any(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()));

            has_literal_index || code.contains("len()")
        }
    }

    // ── TG-P2-05 · the Rust catalogue and the picker are the same list ────────

    /// `provider/catalogue.rs` and `src/data/models.ts` must offer exactly the
    /// same models.
    ///
    /// ⚠ **This gate was cited twice in `catalogue.rs` and did not exist.** Its
    /// doc comment says "`TG-P2-05` fails the build if it does not", and no such
    /// test had been written — so the file carried an assurance nothing
    /// provided. The two lists then disagreed exactly as predicted: `models.ts`
    /// declared `DEFAULT_MODEL = 'gemini-3.5-flash'`, a model the Rust
    /// catalogue asserts must not be offered and the worker database migrates
    /// away from, and it was the initial model for every new worker.
    ///
    /// `TG-P2-09` did not catch it because that gate scans Rust only. A model
    /// id living in TypeScript is exactly as dead as one living in Rust.
    #[test]
    fn tg_p2_05_the_backend_catalogue_and_the_frontend_picker_agree() {
        let catalogue =
            read_source(&repo_root().join("autodev-core/src/provider/catalogue.rs"));
        let models_ts =
            read_source(&repo_root().join("src/data/models.ts"));

        // The Rust side: every `id: "..."` inside a CatalogueEntry.
        let mut backend: Vec<String> = catalogue
            .split("        id: \"")
            .skip(1)
            .filter_map(|r| r.split_once('"').map(|(id, _)| id.to_string()))
            .collect();
        backend.sort();

        // The TypeScript side: quoted tokens inside MODEL_GROUPS, comments
        // stripped first so an apostrophe in prose cannot shift the pairing.
        let block = {
            let from = models_ts.find("MODEL_GROUPS").expect("MODEL_GROUPS");
            let rest = &models_ts[from..];
            let to = rest.find("] as const").unwrap_or(rest.len());
            &rest[..to]
        };
        let code_only: String = block
            .lines()
            .map(|l| match l.find("//") {
                Some(i) => &l[..i],
                None => l,
            })
            .collect::<Vec<_>>()
            .join("\n");

        let mut frontend: Vec<String> = code_only
            .split('\'')
            .skip(1)
            .step_by(2)
            .filter(|t| {
                t.chars().any(|c| c.is_ascii_digit())
                    && (t.contains('-') || t.contains('.'))
                    && !t.contains(' ')
            })
            .map(|t| t.to_string())
            .collect();
        frontend.sort();

        assert!(
            backend.len() >= 8 && frontend.len() >= 8,
            "read {} backend and {} frontend models — re-point this gate",
            backend.len(),
            frontend.len()
        );

        assert_eq!(
            backend, frontend,
            "the Rust catalogue and the model picker disagree. A model offered in the UI \
             but absent from the catalogue has no capabilities, no context window and no \
             route; one in the catalogue but not the UI cannot be selected."
        );

        // The frontend's own default must be a model that exists. This is the
        // specific defect the missing gate let through.
        let default_model = models_ts
            .split("export const DEFAULT_MODEL = '")
            .nth(1)
            .and_then(|r| r.split_once('\'').map(|(m, _)| m))
            .expect("models.ts must declare DEFAULT_MODEL");

        assert!(
            backend.contains(&default_model.to_string()),
            "DEFAULT_MODEL is '{default_model}', which the catalogue does not offer — it is \
             the initial model for every new worker"
        );
    }

    // ── TG-P2-10 · every exit from the turn loop records its spend ────────────

    /// A run that ends in an error still spent money.
    ///
    /// **D-1, from the P2 post-phase audit.** `add_cost` sat after the turn
    /// loop, so it was reached only when the loop exited via `break`. Three
    /// paths `return` out of the loop body instead — the per-task budget
    /// tripping, a stream error, and a tool-batch failure — and each discarded
    /// the entire run's accumulated usage. An eighteen-turn run that died on
    /// turn nineteen recorded **$0.00** against the daily ceiling.
    ///
    /// The budget path is the worst of the three: the run that ends because it
    /// ran out of budget is by construction the expensive one.
    ///
    /// Mode 2 was unaffected — `recipes/runner.rs` records per-turn, inside its
    /// loop — which is most likely why this survived.
    #[test]
    fn tg_p2_10_every_error_exit_from_the_agent_loop_records_its_spend() {
        let engine = read_source(&repo_root().join("autodev-core/src/agent/engine.rs"));

        // The run loop, from `loop {` to the cost recording that follows it.
        let run = engine
            .split("    pub async fn run(")
            .nth(1)
            .expect("run() must exist");

        let mut unrecorded = Vec::new();
        let mut recorded = 0usize;

        // Every `return Err(` inside run() must be preceded by a spend record
        // within the few lines before it.
        let lines: Vec<&str> = run.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if !line.trim_start().starts_with("return Err(") {
                continue;
            }
            // The guard clause before any turn has run cannot have spent
            // anything — it precedes the loop entirely.
            let window_start = i.saturating_sub(6);
            let preceded_by_record = lines[window_start..i]
                .iter()
                .any(|l| l.contains("record_spend("));

            if preceded_by_record {
                recorded += 1;
            } else {
                // A `return` before the loop starts is fine. Detect that by
                // whether any turn machinery appears above it.
                let before: String = lines[..i].join("\n");
                if before.contains("loop {") {
                    unrecorded.push(format!("line ~{}: {}", i + 1, line.trim()));
                }
            }
        }

        assert!(
            recorded >= 3,
            "expected at least 3 recorded error exits, found {recorded} — re-point this \
             gate if run() changed shape"
        );

        assert!(
            unrecorded.is_empty(),
            "these exits from the agent loop discard the run's spend, so it never reaches \
             the daily ceiling:\n{}",
            unrecorded.join("\n")
        );
    }

    // ── TG-P2-11 · the cache prefix excludes the volatile half ────────────────

    /// The stable/volatile split must be USED, not merely built.
    ///
    /// **D-12, from the P2 post-phase audit.** `build_system_prompt_parts`
    /// correctly returned `(stable, volatile)` with the memory block in the
    /// volatile half, and `cache::prefix` correctly kept the two apart — and
    /// the engine called the *joining* wrapper and passed the whole prompt as
    /// `system_base` with empty slices for everything else.
    ///
    /// So the capability existed, was unit-tested, and was never invoked in
    /// production: memory sat inside the hashed region, every learned fact
    /// invalidated the prefix, and each turn paid the cache-write surcharge for
    /// a prefix that could never be read back. `cache::` passed 17/17 the whole
    /// time — a green test for a path production does not take, which is the
    /// exact shape of `L-18`.
    #[test]
    fn tg_p2_11_the_cached_prefix_is_built_from_the_stable_half_only() {
        let engine = read_source(&repo_root().join("autodev-core/src/agent/engine.rs"));

        assert!(
            engine.contains("build_system_prompt_parts"),
            "the engine uses the JOINING prompt builder, so the volatile half is inside \
             the cached region and every new memory invalidates the whole prefix"
        );

        let call = engine
            .split("crate::cache::prefix::build(")
            .nth(1)
            .and_then(|r| r.split_once(");").map(|(a, _)| a))
            .expect("the engine must build a cache prefix");

        assert!(
            call.contains("stable_prompt"),
            "the prefix is not built from the stable half:\n{call}"
        );
        assert!(
            !call.contains("&system_prompt"),
            "the prefix is built from the JOINED prompt, which puts volatile content \
             inside the hashed region:\n{call}"
        );
    }

    // ── TG-P2-12 · key validation cannot name a model ─────────────────────────

    /// `test_connection` must not depend on a specific model id.
    ///
    /// **D-10, from the P2 post-phase audit.** Google's validation POSTed a
    /// `generateContent` call to a hardcoded `gemini-2.5-flash` — a model this
    /// tree's own `the_retired_generation_is_gone` test lists as withdrawn. So
    /// validation 404'd for every user, and because `P2-23` refuses to store a
    /// key the provider rejects, a **valid Google key could not be saved at
    /// all** — on the provider chosen as primary for rollout 1.
    ///
    /// It was also a billed completion, while the vault command told the user
    /// validation is "a models list, not a completion. It costs nothing" and
    /// `capped.rs` exempts `test_connection` from the ceiling *because* of that
    /// claim.
    ///
    /// A validation call that names a model can only rot again, so the gate
    /// forbids naming one rather than forbidding the specific dead id.
    #[test]
    fn tg_p2_12_key_validation_does_not_name_a_model() {
        let dir = repo_root().join("autodev-core/src/provider");
        let mut offenders = Vec::new();

        for adapter in ["anthropic.rs", "openai.rs", "google.rs"] {
            let text = read_source(&dir.join(adapter));

            let f = text
                .split("async fn test_connection(")
                .nth(1)
                .unwrap_or_else(|| panic!("{adapter} has no test_connection"));
            // The function body, up to the next top-level `\n    }`.
            let body = f.split("\n    }").next().unwrap_or(f);

            for (n, line) in body.lines().enumerate() {
                let code = line.split("//").next().unwrap_or("");
                for token in code.split('"').skip(1).step_by(2) {
                    let names_a_model = (token.contains("claude-")
                        || token.contains("gemini-")
                        || token.contains("gpt-"))
                        && token.chars().any(|c| c.is_ascii_digit());
                    if names_a_model {
                        offenders.push(format!(
                            "{adapter} test_connection line ~{}: names '{token}'",
                            n + 1
                        ));
                    }
                }

                if code.contains("generateContent") || code.contains("/messages") {
                    offenders.push(format!(
                        "{adapter} test_connection line ~{}: validates with a BILLED \
                         completion, not a models list",
                        n + 1
                    ));
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "key validation must not name a model or spend money — a named model rots, \
             and a billed call contradicts what the UI tells the user:\n{}",
            offenders.join("\n")
        );
    }

    // ── TG-P2-09 · no shipped code names a model that does not exist ──────────

    /// Every model id written into shipped code must be one the app offers.
    ///
    /// ── What this caught ────────────────────────────────────────────────────
    ///
    /// Model ids rot silently. Anthropic withdrew `claude-sonnet-4-5` and
    /// `claude-opus-4-5` in February 2026, and by 2026-09-02 the tree still
    /// named them in four places that mattered:
    ///
    ///   * `analyze_diff_tool.rs` fell back to `claude-sonnet-4-6` — an id that
    ///     never existed in ANY catalogue — behind two environment variables
    ///     nothing sets, so the fallback was the only path;
    ///   * `spawn_qc_review.rs` defaulted every unnamed QC review to a retired
    ///     model;
    ///   * `recipes/runner.rs` fell back to one;
    ///   * and `anthropic.rs`'s slug normaliser — whose entire purpose is to
    ///     prevent 404s — mapped old names ONTO two retired ids, so it was
    ///     manufacturing the failure it existed to prevent.
    ///
    /// None of it failed at compile time, and none of it failed in any test:
    /// the ids are strings, and a string is wrong only when a provider says so.
    ///
    /// Comments and test files are excluded — a comment recording that an id was
    /// retired is documentation, not a defect, and the fixture suites
    /// deliberately name dead ids to assert they are refused.
    #[test]
    fn tg_p2_09_no_shipped_code_names_a_model_outside_the_catalogue() {
        let catalogue =
            read_source(&repo_root().join("autodev-core/src/provider/catalogue.rs"));

        // The ids the app offers, read from the catalogue's own entries.
        let offered: Vec<String> = catalogue
            .split("        id: \"")
            .skip(1)
            .filter_map(|rest| rest.split_once('"').map(|(id, _)| id.to_string()))
            .collect();

        assert!(
            offered.len() >= 8,
            "read only {} models from the catalogue — re-point this gate",
            offered.len()
        );

        let roots = [
            repo_root().join("autodev-core/src"),
            repo_root().join("src-tauri/src"),
            repo_root().join("src-cli/src"),
        ];

        let mut offenders = Vec::new();
        let mut scanned = 0usize;

        for root in &roots {
            let mut stack = vec![root.clone()];
            while let Some(dir) = stack.pop() {
                let Ok(entries) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                        continue;
                    }
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !name.ends_with(".rs") {
                        continue;
                    }
                    // The catalogue and price table are the registries
                    // themselves; the fixture suites name dead ids on purpose.
                    if name == "catalogue.rs"
                        || name == "pricing.rs"
                        || name.ends_with("_tests.rs")
                    {
                        continue;
                    }

                    // The sidecar translates INTO LiteLLM proxy naming
                    // (`claude-4-5-haiku`), which is a different namespace from
                    // the direct-API ids this gate governs — a dead id there is
                    // a correct output, not a defect. It is also scheduled for
                    // deletion in full before the AutoDesign handover (PD0-PD6),
                    // so hardening it would be work with no destination.
                    if rel_dir_is_sidecar(&path) {
                        continue;
                    }

                    let text = read_source(&path);
                    scanned += 1;
                    let rel = path
                        .strip_prefix(repo_root())
                        .unwrap_or(&path)
                        .display()
                        .to_string()
                        .replace('\\', "/");

                    // Everything from the first inline `#[cfg(test)]` onward.
                    //
                    // Tests legitimately name ids the app does not offer — that
                    // is how `the_retired_generation_is_gone` and
                    // `a_dropped_provider_does_not_route` assert a model is
                    // REFUSED. The gate governs shipped code.
                    let production = match text.find("#[cfg(test)]") {
                        Some(i) => &text[..i],
                        None => &text[..],
                    };

                    for (n, line) in production.lines().enumerate() {
                        let code = line.split("//").next().unwrap_or("");
                        if code.trim().is_empty() {
                            continue;
                        }

                        // Quoted tokens that look like a model id.
                        for token in code.split('"').skip(1).step_by(2) {
                            let looks_like_a_model = (token.starts_with("claude-")
                                || token.starts_with("gemini-")
                                || token.starts_with("gpt-"))
                                && token.chars().any(|c| c.is_ascii_digit())
                                && !token.contains(' ');

                            if !looks_like_a_model {
                                continue;
                            }

                            // A legacy slug being MAPPED to a live id is the fix,
                            // not the defect — the offence is naming a dead id as
                            // a destination or a default.
                            let is_a_match_arm_input =
                                code.contains("=>") && code.find(token) < code.find("=>");

                            if !offered.iter().any(|o| o == token) && !is_a_match_arm_input {
                                offenders.push(format!(
                                    "{rel}:{} names '{token}', which the app does not offer: {}",
                                    n + 1,
                                    code.trim()
                                ));
                            }
                        }
                    }
                }
            }
        }

        assert!(
            scanned > 100,
            "only {scanned} files scanned — this gate is not reaching the tree"
        );

        assert!(
            offenders.is_empty(),
            "these hardcoded model ids are not in the catalogue, so a call using one \
             fails at the provider with a 404 that reads like an outage:\n{}",
            offenders.join("\n")
        );
    }

    // ── TG-P2-08 · a setting the UI writes is a setting the request carries ───

    /// Model settings must reach the request, not merely the database.
    ///
    /// `ModelConfigsDb` has a table, four Tauri commands and a Settings panel
    /// that writes temperature, top_p, reasoning effort and a thinking budget.
    /// **Nothing read any of it.** Every `CompletionRequest` in the agent loop
    /// was built with those four fields hardcoded to `None`.
    ///
    /// That is worse than not offering the controls: the slider moves, the value
    /// persists, the panel redisplays it on reload — and the model runs on
    /// provider defaults, so the user attributes the unchanged behaviour to the
    /// model rather than to a setting that was never sent.
    ///
    /// The gate is deliberately narrow. It does not try to prove the values are
    /// correct — it asserts that the agent engine reads the config at all and
    /// does not build its request from literal `None`s, which is the exact
    /// shape the defect had.
    #[test]
    fn tg_p2_08_model_settings_reach_the_request() {
        let engine = read_source(&repo_root().join("autodev-core/src/agent/engine.rs"));

        assert!(
            engine.contains("ModelConfigsDb"),
            "the agent engine does not read ModelConfigsDb, so every model setting the \
             user saves is written and never sent"
        );

        // Find the request the agent loop actually sends and check those four
        // fields are not literal `None`s again.
        let request = engine
            .split("let request = CompletionRequest {")
            .nth(1)
            .and_then(|rest| rest.split_once("};").map(|(body, _)| body))
            .expect("the agent loop builds a CompletionRequest");

        for field in [
            "temperature: None",
            "top_p: None",
            "reasoning_effort: None",
            "thinking_budget_tokens: None",
        ] {
            assert!(
                !request.contains(field),
                "the agent request hardcodes `{field}`, so the user's saved setting for it \
                 is written to the database and never sent to the provider"
            );
        }

        assert!(
            request.contains("model_config"),
            "the agent request is not built from the saved model config"
        );
    }

    // ── TG-P2-07 · a provider key never comes from argv or the environment ────

    /// BYOK means the key comes from the user, through the UI, into the OS
    /// credential store — and from nowhere else.
    ///
    /// Two routes were closed and this keeps them closed:
    ///
    ///   * **A CLI flag.** `airsctl` took an API key as an argument. A secret in
    ///     argv is visible in shell history and in the process table to every
    ///     other process on the machine, for the lifetime of the command.
    ///   * **A process environment variable.** Falling back to
    ///     `ANTHROPIC_API_KEY` means the app silently runs on a credential the
    ///     user never gave it and cannot see, rotate, or delete from Settings.
    ///     It also inverts the trust story: "no sensitive keys inside the
    ///     application" stops being true the moment the app reads one from the
    ///     ambient environment.
    ///
    /// ⚠ The dev `.env` is NOT an exception to this — it is outside it. The live
    /// smoke reads that file directly and deliberately, never
    /// `std::env::var`, and it is behind the `live-smoke` cargo feature so it is
    /// not linked into either shipped binary. `tests/` is excluded here for that
    /// reason and no other.
    #[test]
    fn tg_p2_07_no_shipped_code_reads_a_provider_key_from_argv_or_the_environment() {
        let roots = [
            repo_root().join("autodev-core/src"),
            repo_root().join("src-tauri/src"),
            repo_root().join("src-cli/src"),
        ];

        let mut offenders = Vec::new();
        let mut scanned = 0usize;

        for root in &roots {
            let mut stack = vec![root.clone()];
            while let Some(dir) = stack.pop() {
                let Ok(entries) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                        continue;
                    }
                    if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                        continue;
                    }

                    let text = read_source(&path);
                    scanned += 1;
                    let rel = path
                        .strip_prefix(repo_root())
                        .unwrap_or(&path)
                        .display()
                        .to_string()
                        .replace('\\', "/");

                    for (n, line) in text.lines().enumerate() {
                        // Comments describing the closed hole are not the hole.
                        let code = line.split("//").next().unwrap_or("");
                        if code.trim().is_empty() {
                            continue;
                        }

                        let reads_env = code.contains("env::var") || code.contains("env!(");
                        let names_a_provider_key = code.contains("ANTHROPIC_API_KEY")
                            || code.contains("OPENAI_API_KEY")
                            || code.contains("GOOGLE_API_KEY")
                            || code.contains("GEMINI_API_KEY");

                        if reads_env && names_a_provider_key {
                            offenders.push(format!(
                                "{rel}:{} reads a provider key from the environment: {}",
                                n + 1,
                                code.trim()
                            ));
                        }

                        // A clap argument literally named for a key.
                        let is_cli_flag = code.contains("api_key")
                            && (code.contains("#[arg") || code.contains("long ="));
                        if is_cli_flag {
                            offenders.push(format!(
                                "{rel}:{} exposes an API key as a CLI argument, which puts it \
                                 in shell history and the process table: {}",
                                n + 1,
                                code.trim()
                            ));
                        }
                    }
                }
            }
        }

        // Floor assertion. A scan that walks nothing passes silently, and this
        // gate is only meaningful if it actually read the tree.
        assert!(
            scanned > 100,
            "only {scanned} source files scanned — this gate is not reaching the tree"
        );

        assert!(
            offenders.is_empty(),
            "BYOK requires that a provider key reach the app only from the user, through \
             the UI, into the credential store:\n{}",
            offenders.join("\n")
        );
    }

    // ── TG-P2-06 · every provider error carries a classification ──────────────

    /// A failed provider response must be classified, not just stringified.
    ///
    /// **`ErrorClass` had no constructor at all until 2026-09-02.** It was
    /// defined, unit-tested against itself, and produced by nothing: all three
    /// adapters flattened failures into
    /// `AutoDevError::Provider(format!("{status}: {text}"))`. So a spend-cap
    /// 429 and a rate-limit 429 were the same string, and Anthropic's 529
    /// overload was the same string as a 500.
    ///
    /// Nothing was looping on the wrong answer, because no retry logic exists
    /// yet — which is exactly why this gate matters now. The first retry loop
    /// anyone writes would otherwise have to re-derive the mapping at the one
    /// place where getting it wrong costs money on every attempt.
    ///
    /// The gate keys on the response body being available: any site that reads
    /// `resp.text()` after a non-success status is a site that can classify,
    /// so it must.
    #[test]
    fn tg_p2_06_every_provider_http_error_is_classified() {
        let dir = repo_root().join("autodev-core/src/provider");
        let mut offenders = Vec::new();

        for adapter in ["anthropic.rs", "openai.rs", "google.rs"] {
            let text = read_source(&dir.join(adapter));

            // Each `if !status.is_success()` block is one error path.
            let blocks = text.matches("if !status.is_success()").count();
            let classified = text.matches("ErrorClass::classify").count();

            if blocks == 0 {
                offenders.push(format!(
                    "{adapter}: no `if !status.is_success()` guard found at all — either \
                     the adapter stopped checking status, or this gate is pointed at the \
                     wrong shape"
                ));
                continue;
            }

            if classified < blocks {
                offenders.push(format!(
                    "{adapter}: {blocks} HTTP error path(s) but only {classified} \
                     classified — an unclassified failure is a string, and a spend-cap \
                     429 is indistinguishable from a rate-limit 429 in a string"
                ));
            }
        }

        assert!(
            offenders.is_empty(),
            "provider errors must carry an ErrorClass:\n{}",
            offenders.join("\n")
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
            let text = read_source(&path);
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
        let text = read_source(&repo_root().join("src-tauri/src/logging.rs"));
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

    // ── TG8-03/04 · no agent runs without the invariant core ──────────────────

    /// Every agent kind receives `INVARIANT_CORE`, asserted by RUNNING the
    /// composer each kind's own call site uses.
    ///
    /// 🔴 `P8-34`/`P8-35`/`P8-36`. `engine.rs` treated a system-prompt override
    /// as a **total replacement** of the stable half. A recipe's `instructions`
    /// are passed as that override (`recipes/runner.rs`), and so is a
    /// subagent's `system_prompt` (`subagent/native_runner.rs`) — so the
    /// prohibitions, the git house rules and the user's own `AUTODEV.md`
    /// reached the interactive assistant and **none of the agents that write
    /// files, run commands and commit**.
    ///
    /// ⚠ This calls the real composer with the real recipe text rather than
    /// searching a source file for a phrase. `L-C7`: a source-scanning
    /// assertion can match its own source, and this gate's own explanation
    /// names every string it would look for.
    ///
    /// ⚠ The worker list comes from `FLEET_WORKERS`, not from a list retyped
    /// here. Two lists that must agree is `L-19`, and the second one is always
    /// the one nobody updates when a worker is added.
    #[test]
    fn tg8_03_every_agent_kind_receives_the_invariant_core() {
        use autodev_core::agent::context::{ContextBuilder, INVARIANT_CORE};
        use autodev_core::agent::PAMode;
        use autodev_core::recipes::defaults::{ensure_default_recipes, FLEET_WORKERS};
        use autodev_core::recipes::parser::load_all_recipes;

        // One sandbox for this test alone. `L-30`: three P4 tests shared a temp
        // dir, each called `remove_dir_all`, and they passed alone and failed
        // in the full suite.
        let sandbox = std::env::temp_dir().join(format!(
            "autodev-tg8-03-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&sandbox).expect("sandbox");

        let ctx = ContextBuilder::new(sandbox.to_string_lossy().to_string());
        let repo = sandbox.to_string_lossy().to_string();

        // ── the PA, on every mode it can be in ───────────────────────────────
        // `agent_commands.rs` passes `None` when the user has set no custom
        // prompt, which lands on the `build_system_prompt_parts` arm.
        for mode in [PAMode::FastDevelopment, PAMode::Review, PAMode::Delegate] {
            let (stable, _volatile) = ctx.build_system_prompt_parts(mode, &repo, "q");
            assert!(
                stable.starts_with(INVARIANT_CORE),
                "the PA in {mode:?} does not open with the invariant core"
            );
        }

        // ── the PA with a user-set custom prompt ─────────────────────────────
        // `agent_commands.rs` passes `Some(settings.system_prompt_override)`.
        // A user customising their prompt is not a user opting out of the
        // prohibitions.
        let custom = ctx.build_agent_stable_prompt(&repo, "Answer only in haiku.");
        assert!(
            custom.starts_with(INVARIANT_CORE),
            "a user-set system prompt drops the invariant core"
        );
        assert!(
            custom.contains("Answer only in haiku."),
            "the core swallowed the user's own instructions — it must EXTEND \
             them, not replace them"
        );

        // ── every fleet worker, from its real recipe ─────────────────────────
        ensure_default_recipes(&sandbox).expect("write default recipes");
        let (recipes, errors) = load_all_recipes(&sandbox);
        assert!(
            errors.is_empty(),
            "the default recipes do not parse: {errors:?}"
        );
        assert!(
            !recipes.is_empty(),
            "no recipes were written, so this gate checked nothing"
        );

        for slug in FLEET_WORKERS {
            let recipe = recipes
                .iter()
                .find(|r| r.agent_role.as_deref() == Some(slug))
                .unwrap_or_else(|| {
                    panic!("no default recipe for fleet worker {slug} — either the \
                            worker has no recipe, or `agent_role` no longer carries \
                            the slug this gate matches on")
                });

            // Exactly what `recipes/runner.rs` passes as the override.
            let stable = ctx.build_agent_stable_prompt(&repo, &recipe.instructions);
            assert!(
                stable.starts_with(INVARIANT_CORE),
                "fleet worker {slug} runs without the invariant core"
            );
            assert!(
                stable.contains(&recipe.instructions),
                "fleet worker {slug} lost its own recipe instructions"
            );
        }

        // ── a subagent ───────────────────────────────────────────────────────
        // `subagent/native_runner.rs` passes `config.system_prompt` when it is
        // non-empty, and `None` when it is not — both arms are covered above,
        // but a spawned subagent is the agent kind with the least oversight and
        // is asserted explicitly.
        let sub = ctx.build_agent_stable_prompt(&repo, "You review one diff and report.");
        assert!(
            sub.starts_with(INVARIANT_CORE),
            "a spawned subagent runs without the invariant core"
        );

        let _ = std::fs::remove_dir_all(&sandbox);
    }

    /// The engine has exactly ONE place a stable prompt is produced, and every
    /// arm of it composes.
    ///
    /// ⚠ `TG8-03` proves the composer is correct. It cannot prove that a new
    /// agent kind will *use* it — a fourth branch returning `override_str`
    /// directly would leave `TG8-03` green. This is the structural half, over
    /// the chokepoint every agent kind actually passes through.
    #[test]
    fn tg8_04_no_engine_path_bypasses_the_composer() {
        let engine = read_source(&repo_root().join("autodev-core/src/agent/engine.rs"));

        // Comments stripped FIRST. This gate's own explanation contains the
        // string it searches for — the recorded sub-pattern of `L-27` that has
        // now caught three gates in this file.
        let code: String = engine
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        let branch = code
            .split("match system_prompt_override")
            .nth(1)
            .expect("the override match must exist")
            .split("};")
            .next()
            .expect("the match must terminate")
            .to_string();

        assert!(
            branch.contains("build_agent_stable_prompt"),
            "the override arm does not compose the invariant core:\n{branch}"
        );
        assert!(
            branch.contains("build_system_prompt_parts"),
            "the no-override arm no longer uses the PA builder:\n{branch}"
        );
        assert!(
            !branch.contains("override_str.to_string()"),
            "the override arm returns the override verbatim again, which drops \
             the invariant core and the user's AUTODEV.md:\n{branch}"
        );

        // And the PA builder must delegate rather than re-assemble, or there
        // are two composers and one of them will drift (`L-19`).
        let ctx = read_source(&repo_root().join("autodev-core/src/agent/context.rs"));
        let ctx_code: String = ctx
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let pa = ctx_code
            .split("pub fn build_system_prompt_parts")
            .nth(1)
            .expect("build_system_prompt_parts must exist")
            .split("\n    /// ")
            .next()
            .unwrap_or_default()
            .to_string();
        assert!(
            pa.contains("build_agent_stable_prompt"),
            "build_system_prompt_parts assembles its own prompt instead of \
             delegating, so the PA and the workers have two different \
             composers:\n{pa}"
        );

        // The core must be the first thing the composer emits, structurally —
        // not merely present somewhere in it.
        // The BODY, not the signature. The first cut of this assertion sliced
        // from the function NAME, and went red because `own_instructions`
        // appears in the parameter list before `INVARIANT_CORE` appears in the
        // body. The code was correct and the check was wrong — an assertion
        // written from the claim ("the core comes first") rather than from the
        // invariant it stands for (`L-C6`), failing for a mechanism it did not
        // name (`L-37`).
        let composer = ctx_code
            .split("pub fn build_agent_stable_prompt")
            .nth(1)
            .expect("build_agent_stable_prompt must exist")
            .split_once("-> String {")
            .expect("the composer signature must terminate")
            .1
            .split("\n    }")
            .next()
            .expect("the composer body must terminate")
            .to_string();
        let core_at = composer
            .find("INVARIANT_CORE")
            .expect("the composer must reference the core");
        let own_at = composer
            .find("own_instructions")
            .expect("the composer must reference the caller's instructions");
        assert!(
            core_at < own_at,
            "the composer no longer puts the invariant core first:\n{composer}"
        );
    }

    // ── TG8-05 · the core's git rules are the house rules ─────────────────────

    /// `P8-37`. The five house rules are present in the core, each asserted by
    /// the behaviour it forbids rather than by a slogan.
    ///
    /// ⚠ Asserted against the CONST at runtime, not by scanning the source
    /// file. The source file also contains this gate's own doc comment naming
    /// every rule, and a gate that reads source finds the rule in its own
    /// explanation (`L-C7`, and the `L-27` sub-pattern this file has recorded
    /// three times).
    #[test]
    fn tg8_05_the_core_carries_the_house_git_rules() {
        use autodev_core::agent::context::INVARIANT_CORE;

        // (rule, the substring that can only appear if the rule is stated)
        let rules: &[(&str, &[&str])] = &[
            ("branch before the first commit", &["branch before your first commit"]),
            ("stage deliberately, never a blind add", &["git add -A", "Stage deliberately"]),
            ("never commit secrets or .env", &["secrets", ".env"]),
            ("never force-push or rewrite published history", &["force-push", "published history"]),
            (
                "never push to main / open / merge a PR unapproved",
                &["push to `main`", "pull request"],
            ),
            ("never delete branches, stashes, refs", &["Never delete"]),
        ];

        let core_lc = INVARIANT_CORE.to_lowercase();
        let mut missing: Vec<&str> = Vec::new();
        for (rule, needles) in rules {
            if !needles.iter().all(|n| core_lc.contains(&n.to_lowercase())) {
                missing.push(rule);
            }
        }
        assert!(
            missing.is_empty(),
            "the invariant core no longer states these house rules: {missing:?}\n\
             They are taken from the project's own CLAUDE.md and are the rules \
             every agent that touches version control must be given."
        );

        // The core is a floor, not a manual. If it grows past a page it becomes
        // a second place to look for a worker's behaviour, which `P8-34`
        // explicitly refuses.
        assert!(
            INVARIANT_CORE.len() < 4_000,
            "the invariant core has grown to {} bytes. It is prepended to EVERY \
             agent's prompt; keep role-specific text in the recipe.",
            INVARIANT_CORE.len()
        );
    }

    // ── TG8-06 · no recipe commits blind, and none commits onto main ──────────

    /// `P8-38`. Every default recipe that commits does so through the shared
    /// protocol: branch first, stage named paths, scan the diff, stop at the
    /// commit.
    ///
    /// 🔴 All five worker recipes ran `bash('git add -A && git commit …')` with
    /// no branch created, so every fleet commit landed on the user's checked-out
    /// trunk, staged blindly and unscanned. The knock-on was worse: the cleanup
    /// worker's **mandatory** functionality-preservation check diffed `main`
    /// against the current branch, and since nothing created a branch it diffed
    /// `main` against itself — the one hard safety gate in that worker was
    /// judging an empty patch and passing everything.
    ///
    /// ⚠ Asserted over the recipes as WRITTEN TO DISK, which is what the fleet
    /// loads and what the model is actually given.
    #[test]
    fn tg8_06_no_default_recipe_commits_blind_or_onto_a_trunk() {
        use autodev_core::recipes::defaults::{ensure_default_recipes, FLEET_WORKERS};
        use autodev_core::recipes::parser::load_all_recipes;

        let sandbox = std::env::temp_dir().join(format!(
            "autodev-tg8-06-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&sandbox).expect("sandbox");
        ensure_default_recipes(&sandbox).expect("write default recipes");
        let (recipes, errors) = load_all_recipes(&sandbox);
        assert!(errors.is_empty(), "default recipes do not parse: {errors:?}");
        assert!(!recipes.is_empty(), "no recipes written — this gate checked nothing");

        for r in &recipes {
            // A blind stage, as an INSTRUCTION rather than as the prohibition.
            // The protocol names `git add -A` in order to forbid it, so match
            // the executable form: inside a `bash(` call.
            for bad in ["bash('git add -A", "bash(\"git add -A", "bash('git add .", "bash(\"git add ."] {
                assert!(
                    !r.instructions.contains(bad),
                    "recipe `{}` still tells the agent to stage blindly: {bad}",
                    r.id
                );
            }
        }

        // Every worker that commits must carry the protocol, and the protocol
        // must be the one that branches.
        for slug in FLEET_WORKERS {
            let r = recipes
                .iter()
                .find(|r| r.agent_role.as_deref() == Some(slug))
                .unwrap_or_else(|| panic!("no default recipe for fleet worker {slug}"));

            assert!(
                r.instructions.contains("## Commit Protocol"),
                "recipe for {slug} has no Commit Protocol section"
            );
            assert!(
                r.instructions.contains("git switch -c autodev/fleet"),
                "recipe for {slug} never creates a branch, so its commits land on \
                 whatever the user had checked out"
            );
            assert!(
                r.instructions.contains("git diff --cached"),
                "recipe for {slug} commits without reading the staged diff, so a \
                 credential in the working tree is committed unseen"
            );
        }

        // The cleanup worker's safety check must judge its own commit.
        let cleanup = recipes
            .iter()
            .find(|r| r.agent_role.as_deref() == Some("worker-cleanup"))
            .expect("worker-cleanup recipe");
        assert!(
            !cleanup.instructions.contains("base_branch: \"main\""),
            "worker-cleanup still diffs against `main`. On a shared fleet branch \
             that puts other workers' feature commits in front of a check that \
             rejects anything classified as a logic change."
        );
        assert!(
            cleanup.instructions.contains("base_branch: \"HEAD~1\""),
            "worker-cleanup's functionality-preservation check no longer diffs \
             its own commit, so it is not judging the change it made"
        );

        let _ = std::fs::remove_dir_all(&sandbox);
    }

    // ── TG8-07 · the worker-status vocabulary agrees across the seam ──────────

    /// Every status Rust writes is understood by the store that renders it,
    /// and the store understands nothing Rust cannot write.
    ///
    /// 🔴 `P8-43`. `AgentLiveStatus` carried `mode1_active`, documented as
    /// flagging *"workers actually running on the Rust AgentEngine"* — which is
    /// every worker, always — and **no Rust call site ever wrote it**. A union
    /// member, a dot colour and a mapping arm were all maintained for a value
    /// that could not arrive. `fallback_idle` was the mirror defect: written
    /// under a different name (`mode1_idle_no_provider`) for a condition that
    /// is not a fallback.
    ///
    /// ⚠ The mapping is a **fall-through** — anything unmatched renders as
    /// plain `idle`. That is why a missing arm is not a visible error but a
    /// worker reported as "waiting for work" when it is actually blocked. This
    /// gate exists because the failure is silent in exactly the direction that
    /// looks healthy.
    ///
    /// ⚠ Comments stripped from both sides first: this gate's own explanation
    /// names every string it searches for, and so do the fixes it protects
    /// (`L-C7`).
    #[test]
    fn tg8_07_the_worker_status_vocabulary_agrees_across_the_seam() {
        // Statuses the store deliberately still accepts although Rust no longer
        // writes them. `workers.db` stores each worker's LAST status, so a row
        // written by an older build survives an upgrade; dropping the arm would
        // render that worker as ordinary `idle` until its next tick.
        const LEGACY: &[&str] = &["mode1_idle_no_provider"];

        // 🔴 **The first cut of this gate read `set_worker_status(` in
        // `runner.rs` and nothing else — and the defect was not there.**
        // `scheduler::stop_worker` and `budget::daily::pause_fleet` write
        // `"paused"` straight through `WorkersDb::update_status`, and
        // `pause_fleet` guarded on `"running"`, a status nothing in this
        // application writes. An extractor that reads one helper in one file
        // cannot see a writer that bypasses the helper, so it reported
        // agreement over a vocabulary it had only half enumerated.
        //
        // The writer set is DERIVED: every file that mentions `WorkersDb` and
        // calls `update_status(`. Naming the files here would be the same list
        // that must agree, one indirection later (`L-19`).
        let core = repo_root().join("autodev-core/src");
        let mut writer_files: Vec<std::path::PathBuf> = Vec::new();
        collect_rs(&core, &mut writer_files);
        let writer_files: Vec<std::path::PathBuf> = writer_files
            .into_iter()
            .filter(|p| {
                let t = read_source(p);
                t.contains("WorkersDb") && t.contains("update_status(")
            })
            .collect();
        assert!(
            writer_files.len() >= 2,
            "only {} worker-status writer file(s) found. The runner and at least \
             one direct writer must be present, so the discovery is broken and \
             this gate would compare against a partial vocabulary: {writer_files:?}",
            writer_files.len()
        );

        let mut written: Vec<String> = Vec::new();
        for path in &writer_files {
            let src = read_source(path);
            assert!(
                !src.trim().is_empty(),
                "{path:?} did not read — `read_source` is unwrap_or_default, so \
                 every assertion below would pass vacuously (`L-C9`)"
            );
            let code: String = src
                .lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");

            // `set_worker_status(slug, "x")` and `update_status(id, "x")`.
            for call in ["set_worker_status(", "update_status("] {
                for seg in code.split(call).skip(1) {
                    let Some(args) = seg.split(')').next() else {
                        continue;
                    };
                    // ⚠ `TasksDb` has an `update_status` too, and `runner.rs`
                    // calls both — so a file-level filter pulled `qc_review`
                    // and `build_failed` into the worker vocabulary and the
                    // gate went red naming task statuses. They are told apart
                    // by **arity**, which is a property of the call rather than
                    // of the file it sits in: `WorkersDb::update_status(id,
                    // status)` takes two arguments, `TasksDb::update_status(id,
                    // status, assignee)` takes three.
                    if args.matches(',').count() != 1 {
                        continue;
                    }
                    if let Some(q) = args.find('"') {
                        if let Some(end) = args[q + 1..].find('"') {
                            written.push(args[q + 1..q + 1 + end].to_string());
                        }
                    }
                }
            }
        }
        written.sort();
        written.dedup();
        assert!(
            written.len() >= 4,
            "only {} status literals found across {} writer file(s) — the \
             extractor is not matching the call shape, so this gate is checking \
             almost nothing: {written:?}",
            written.len(),
            writer_files.len()
        );

        let store = read_source(&repo_root().join("src/stores/agentLiveStore.ts"));
        assert!(!store.trim().is_empty(), "agentLiveStore.ts did not read");
        let store_code: String = store
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        // Every `e.payload.status === '<literal>'`.
        let mut handled: Vec<String> = Vec::new();
        for seg in store_code.split("e.payload.status === '").skip(1) {
            if let Some(end) = seg.find('\'') {
                handled.push(seg[..end].to_string());
            }
        }
        handled.sort();
        handled.dedup();
        assert!(
            !handled.is_empty(),
            "no status arms found in agentLiveStore.ts — the extractor is not \
             matching, so this gate is checking nothing"
        );

        // (a) Anything Rust writes other than plain `idle` must have an arm.
        let unhandled: Vec<&String> = written
            .iter()
            .filter(|w| w.as_str() != "idle" && !handled.contains(w))
            .collect();
        assert!(
            unhandled.is_empty(),
            "Rust writes these worker statuses and the store has no arm for them, \
             so they fall through and render as ordinary `idle` — a blocked or \
             failing worker shown as waiting for work: {unhandled:?}\n\
             written={written:?}\nhandled={handled:?}"
        );

        // (b) Every arm must correspond to something Rust can write, or be
        //     declared legacy above. An arm with no writer is `W-7`.
        let orphaned: Vec<&String> = handled
            .iter()
            .filter(|h| !written.contains(h) && !LEGACY.contains(&h.as_str()))
            .collect();
        assert!(
            orphaned.is_empty(),
            "the store maps these statuses and no Rust call site writes them — a \
             reader with no writer. Either produce them or remove them; if one is \
             deliberately kept for rows written by an older build, add it to \
             LEGACY with the reason: {orphaned:?}\n\
             written={written:?}\nhandled={handled:?}"
        );

        // (b2) The SCHEDULER must recognise every status the runner writes.
        //
        // 🔴 This is the assertion that was missing, and its absence is why the
        // `pause_fleet` falsification stayed green. The store is not the only
        // consumer of a worker status: `recipes/scheduler.rs` matches on it to
        // decide whether to fire the next tick, and its catch-all arm logs
        // "unexpected status" and fires nothing. So a status the runner writes
        // and the scheduler does not list **parks that worker permanently** —
        // no error, no failing test, and a log line that reads as a curiosity.
        // `idle_no_provider` was in exactly that state.
        let sched = read_source(&repo_root().join("autodev-core/src/recipes/scheduler.rs"));
        assert!(!sched.trim().is_empty(), "scheduler.rs did not read");
        let sched_code: String = sched
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let arms = sched_code
            .split("match current_status.as_deref()")
            .nth(1)
            .expect("the scheduler must match on the worker status")
            .split("\n            }")
            .next()
            .unwrap_or_default()
            .to_string();
        let mut recognised: Vec<String> = Vec::new();
        for seg in arms.split("Some(\"").skip(1) {
            if let Some(end) = seg.find('"') {
                recognised.push(seg[..end].to_string());
            }
        }
        assert!(
            recognised.len() >= 4,
            "only {} status arms found in the scheduler match — the extractor is \
             not matching, so this half checks nothing: {recognised:?}"
            , recognised.len()
        );
        let unrecognised: Vec<&String> = written
            .iter()
            .filter(|w| !recognised.contains(w))
            .collect();
        assert!(
            unrecognised.is_empty(),
            "the runner writes these statuses and `recipes/scheduler.rs` does not \
             list them, so they fall to its catch-all arm and NO further tick is \
             ever fired for that worker — it is parked until someone stops and \
             starts it: {unrecognised:?}\n\
             written={written:?}\nrecognised={recognised:?}"
        );

        // (c) The rendered union may not carry a member the mapper cannot produce.
        let types = read_source(&repo_root().join("src/types/live.ts"));
        assert!(!types.trim().is_empty(), "types/live.ts did not read");
        let union = types
            .split("export type AgentLiveStatus =")
            .nth(1)
            .and_then(|r| r.split("export type").next())
            .expect("the AgentLiveStatus union must exist")
            .to_string();
        for dead in ["mode1_active", "fallback_idle", "'stopped'"] {
            assert!(
                !union.contains(dead),
                "`{dead}` is back in AgentLiveStatus. It names an engine \
                 distinction that no longer exists, and nothing writes it:\n{union}"
            );
        }
    }

    // ── TG8-08 · no frontend listener without a Rust emitter ──────────────────

    /// Every Tauri event the frontend subscribes to is emitted somewhere in
    /// Rust.
    ///
    /// 🔴 `P8-52` deleted the startup FSM that emitted `startup:progress` and
    /// the `get_startup_phase` command the overlay fell back to — and left
    /// `StartupOverlay` mounted in `App.tsx`. The overlay hides itself only
    /// when a `startup:progress` event reaches the "ready" phase, so
    /// `dismissed` could never flip and the app rendered a **full-screen
    /// `z-index: 9999` modal that nothing could dismiss**. The task was marked
    /// done and `SMOKE-PROVEN`. Nothing in the suite could fail, because the
    /// two halves live in different languages and neither one is wrong on its
    /// own.
    ///
    /// ⚠ **`ci_gate.rs` is excluded from the emitter scan.** This file names
    /// every event in `KNOWN_UNEMITTED` below, so scanning itself would let
    /// the gate satisfy its own assertion — `L-C7`, which this repository has
    /// now recorded four times.
    #[test]
    fn tg8_08_every_frontend_listener_has_a_rust_emitter() {
        // 🔴 **Ten events the frontend listens for that Rust never emits.**
        // Found by writing this gate. They are NOT `P8`'s subject — none names
        // a deleted subsystem, they are ordinary readers with no writer — so
        // they are recorded here rather than rewired, which is a product
        // decision about what ought to emit them:
        //
        //   * the Live Monitor's stage changes and token sparkline
        //     (`agent:stage-change`, `agent:status-change`,
        //     `agent:token-update`, `agent:tool-call-start`,
        //     `agent:tool-call-result`) — the card subscribes to five events
        //     that never arrive;
        //   * the whole plan store (`plan:created`, `plan:updated`,
        //     `plan:item_claimed`, `plan:step_completed`,
        //     `plan:subplan_created`) — Rust emits only `plan:changed`, which
        //     nothing listens for. The two sides were named independently and
        //     have never once agreed.
        //
        // The list is a **ratchet**: a new listener with no emitter fails this
        // gate, and each of these can be struck as it is wired up.
        const KNOWN_UNEMITTED: &[&str] = &[
            "agent:stage-change",
            "agent:status-change",
            "agent:token-update",
            "agent:tool-call-start",
            "agent:tool-call-result",
            "plan:created",
            "plan:updated",
            "plan:item_claimed",
            "plan:step_completed",
            "plan:subplan_created",
        ];

        // ── what the frontend listens for ────────────────────────────────────
        let mut ts_files: Vec<std::path::PathBuf> = Vec::new();
        collect_ext(&repo_root().join("src"), "ts", &mut ts_files);
        collect_ext(&repo_root().join("src"), "tsx", &mut ts_files);
        assert!(
            ts_files.len() > 20,
            "only {} frontend source file(s) found — the walk is broken and this \
             gate would compare an empty set: {ts_files:?}",
            ts_files.len()
        );

        let mut listened: Vec<String> = Vec::new();
        for p in &ts_files {
            let src = read_source(p);
            for seg in src.split("listen").skip(1) {
                // `listen<Payload>('name'` or `listen('name'`
                let head: String = seg.chars().take(120).collect();
                let Some(q) = head.find(['\'', '"']) else { continue };
                let quote = head.as_bytes()[q] as char;
                let rest = &head[q + 1..];
                let Some(end) = rest.find(quote) else { continue };
                let name = &rest[..end];
                // An event name is `namespace:event`. Anything else is some
                // other call that merely starts with "listen".
                if name.contains(':') && !name.contains(' ') && name.len() < 60 {
                    listened.push(name.to_string());
                }
            }
        }
        listened.sort();
        listened.dedup();
        assert!(
            listened.len() >= 15,
            "only {} listener(s) extracted — the extractor is not matching the \
             call shape, so this gate is checking almost nothing: {listened:?}",
            listened.len()
        );

        // ── what Rust can emit ───────────────────────────────────────────────
        let mut rs_files: Vec<std::path::PathBuf> = Vec::new();
        collect_rs(&repo_root().join("src-tauri/src"), &mut rs_files);
        collect_rs(&repo_root().join("autodev-core/src"), &mut rs_files);
        // See the doc comment: this file names every KNOWN_UNEMITTED event.
        rs_files.retain(|p| !p.ends_with("ci_gate.rs"));
        assert!(
            rs_files.len() > 20,
            "only {} Rust source file(s) found — the walk is broken",
            rs_files.len()
        );
        let rust: String = rs_files.iter().map(|p| read_source(p)).collect();
        assert!(
            rust.len() > 100_000,
            "the Rust corpus read back as {} bytes, which cannot be right — \
             every 'no emitter' assertion below would fire spuriously",
            rust.len()
        );

        let mut orphaned: Vec<&String> = Vec::new();
        for name in &listened {
            if KNOWN_UNEMITTED.contains(&name.as_str()) {
                continue;
            }
            if !rust.contains(&format!("\"{name}\"")) {
                orphaned.push(name);
            }
        }
        assert!(
            orphaned.is_empty(),
            "the frontend subscribes to these events and NO Rust source emits \
             them, so the UI they drive is permanently inert: {orphaned:?}\n\
             If a listener is deliberately unwired for now, add it to \
             KNOWN_UNEMITTED with the reason — but a component whose only data \
             source is one of these should be deleted, not left mounted."
        );

        // The ratchet only tightens: an entry that IS now emitted must be
        // struck, or the list quietly becomes a list of nothing.
        let resurrected: Vec<&&str> = KNOWN_UNEMITTED
            .iter()
            .filter(|n| rust.contains(&format!("\"{n}\"")))
            .collect();
        assert!(
            resurrected.is_empty(),
            "these events now HAVE a Rust emitter and are still listed as \
             unemitted. Strike them from KNOWN_UNEMITTED: {resurrected:?}"
        );
    }
}
