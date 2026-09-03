// Doc 09 T2.4 — build evidence tracker.
//
// Records bash invocations that look like build / test / smoke commands so
// `update_task_status_tool` can reject `qc_review` transitions whose
// session has produced no evidence of an actual build.
//
// Why a module-level tracker instead of plumbing the tool history through
// ToolContext: the LivingAgent and AgentEngine both invoke tools through
// their own loops without exposing a shared transcript to the tool layer.
// We don't need accuracy — we need a directional "did this session run a
// build in the last few minutes?" signal. A small in-memory ring keyed by
// session_id with a TTL is enough.
//
// Each entry carries the matched command + exit code so we can also veto
// `qc_review` when the most recent build *failed* (exit != 0). That alone
// would have caught the calendar run's `"build": "node -c …"` bug —
// `-c` is a Bash flag, not a Node flag, so it errored 255 every time.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use crate::state::lock::LockTolerant;

/// Build evidence stays in the tracker for 15 minutes. Long enough for a
/// worker tick (build → write artefact → smoke → commit → status update),
/// short enough that a stale claim from a previous task doesn't leak.
pub const EVIDENCE_TTL: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone)]
pub struct BuildEvidence {
    pub session_id: String,
    pub command: String,
    pub exit_code: i32,
    /// Whether `exit_code` is actually the BUILD's status.
    ///
    /// False for a pipeline or command sequence: a shell reports the exit
    /// status of the LAST element, so `npm run build 2>&1 | tee build.log;
    /// echo "EXIT:$?"` exits 0 because `echo` did — whatever the build did.
    /// That form was recommended by this gate's own error message, so a failed
    /// build was recorded as exit 0 and certified as passing. P0 made this gate
    /// mandatory, which turned a dormant hole into an active one. Found by the
    /// P0 post-phase audit (F-23).
    pub exit_code_attributable: bool,
    pub recorded_at: Instant,
    pub kind: BuildEvidenceKind,
}

/// Whether a shell command's exit status belongs to the command we classified.
///
/// Conservative: any pipeline or sequencing operator makes the status belong to
/// something else. Quoting is not parsed — a false "unattributable" costs the
/// agent one plain re-run, while a false "attributable" certifies an unbuilt
/// artifact.
pub fn exit_code_is_attributable(command: &str) -> bool {
    !command.contains('|')
        && !command.contains(';')
        && !command.contains("&&")
        && !command.contains("||")
        && !command.contains('\n')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildEvidenceKind {
    /// `npm run build`, `npm install`, `npm run dev`, `pnpm build`, `yarn build`
    Node,
    /// `cargo test`, `cargo build`, `cargo check`
    Cargo,
    /// `uv run pytest`, `uv sync`, `pytest`, `python -m pytest`, `py.test`
    Python,
    /// `go test`, `go build`
    Go,
    /// `timeout … npm run dev` etc. — the T2.3 smoke probe
    Smoke,
}

impl BuildEvidenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Cargo => "cargo",
            Self::Python => "python",
            Self::Go => "go",
            Self::Smoke => "smoke",
        }
    }
}

fn ring() -> &'static Mutex<HashMap<String, Vec<BuildEvidence>>> {
    static RING: OnceLock<Mutex<HashMap<String, Vec<BuildEvidence>>>> = OnceLock::new();
    RING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Classify a bash command string. Returns None when the command isn't
/// recognised as build / test / smoke. The classifier is intentionally
/// lenient — it matches commonly-typed forms, not the entire universe of
/// possible invocations. False negatives are acceptable: the worker can
/// invoke a recognised build form to record evidence even when the actual
/// validation tool is bespoke.
/// Flags that make a build command exit 0 without building anything.
const NO_OP_FLAGS: &[&str] = &["--help", "-h", "--version", "-v", "--dry-run"];

/// Split a shell command into its separate invocations.
///
/// `cd sub && npm run build` is two segments; the build is the second one, so
/// the first token of the WHOLE command is not enough.
fn segments(command: &str) -> Vec<String> {
    command
        .replace("||", "\n")
        .replace("&&", "\n")
        .replace('|', "\n")
        .replace(';', "\n")
        .split('\n')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The tokens of one invocation, with leading `VAR=value` assignments dropped.
///
/// ⚠ Deliberately naive about quoting: it splits on whitespace, so
/// `grep "npm run build" f` yields `["grep", "\"npm", "run", "build\"", "f"]`.
/// That is *safe in the direction that matters* — the program is still `grep`,
/// so the command is still not a build. A quote-aware splitter would be more
/// correct and no more secure.
fn invocation_tokens(segment: &str) -> Vec<String> {
    segment
        .split_whitespace()
        .skip_while(|t| t.contains('=') && !t.starts_with('-'))
        .map(|t| t.to_lowercase())
        .collect()
}

/// Does this segment actually INVOKE `hit`?
///
/// 🔴 The whole point of `P8-28`. `classify` used to ask whether the command
/// *contained* `"npm run build"` anywhere, so `echo npm run build`,
/// `grep -rn "npm run build" package.json`, `git log --grep="npm run build"`
/// and `sed -i 's/npm run build/pnpm build/'` all classified as **a passing
/// Node build** and opened the `qc_review` door with nothing built.
///
/// ⚠ The accidental forms matter more than the deliberate ones: an agent does
/// not have to be adversarial to run `rg "npm run build" .`. And the gate was
/// asymmetric — aggressive about false negatives (any `|` or `;` rejects) and
/// completely permissive about false positives, which gives a rejected agent a
/// gradient straight toward the magic substring.
fn segment_invokes(segment: &str, hit: &str) -> bool {
    let tokens = invocation_tokens(segment);
    let wanted: Vec<String> = hit.split_whitespace().map(|t| t.to_lowercase()).collect();
    if tokens.len() < wanted.len() {
        return false;
    }
    if tokens[..wanted.len()] != wanted[..] {
        return false;
    }
    // `npm run build --help` exits 0 and builds nothing.
    !tokens.iter().any(|t| NO_OP_FLAGS.contains(&t.as_str()))
}

/// True when any invocation in `command` actually runs one of `hits`.
fn invokes_any(command: &str, hits: &[&str]) -> bool {
    segments(command)
        .iter()
        .any(|seg| hits.iter().any(|h| segment_invokes(seg, h)))
}

pub fn classify(command: &str) -> Option<BuildEvidenceKind> {
    let c = command.to_lowercase();

    // Smoke takes priority — it's literally a `timeout N <start-cmd>`.
    if c.starts_with("timeout ") || c.contains(" timeout ") {
        if c.contains("npm run dev")
            || c.contains("npm start")
            || c.contains("uvicorn")
            || c.contains("flask run")
            || c.contains("cargo run")
            || c.contains("go run")
        {
            return Some(BuildEvidenceKind::Smoke);
        }
    }

    // Node ecosystem
    let node_hits = [
        "npm run build",
        "npm install",
        "npm ci",
        "npm test",
        "npm run test",
        "pnpm build",
        "pnpm install",
        "pnpm test",
        "yarn build",
        "yarn install",
        "yarn test",
        "vite build",
        "tsc --noemit",
        "tsc -b",
    ];
    if invokes_any(command, &node_hits) {
        return Some(BuildEvidenceKind::Node);
    }

    // Cargo
    let cargo_hits = ["cargo test", "cargo build", "cargo check", "cargo clippy"];
    if invokes_any(command, &cargo_hits) {
        return Some(BuildEvidenceKind::Cargo);
    }

    // Python (covers uv + bare interpreter forms)
    let py_hits = [
        "uv run pytest",
        "uv sync",
        "pytest",
        "python -m pytest",
        "py.test",
        "uv pip install",
        "pip install -r",
    ];
    if invokes_any(command, &py_hits) {
        return Some(BuildEvidenceKind::Python);
    }

    // Go
    // 4 of 4. This branch was missed on the first pass of P8-28 and the
    // test caught it holding a SECOND defect: `c.contains("go build")` is
    // true of `cargo build`, because "car|go build". So any cargo command
    // that failed the cargo arm -- `cargo build --help`, say -- fell through
    // and classified as a **Go** build. Substring matching does not merely
    // admit impostors; it confuses one ecosystem for another.
    let go_hits = ["go test", "go build", "go mod tidy"];
    if invokes_any(command, &go_hits) {
        return Some(BuildEvidenceKind::Go);
    }

    None
}

/// Record a bash invocation. Idempotent — multiple records of the same
/// command back-to-back are not deduplicated; the GC just sweeps the
/// expired ones out on the next read.
pub fn record(session_id: &str, command: &str, exit_code: i32) {
    let Some(kind) = classify(command) else {
        return;
    };
    let evidence = BuildEvidence {
        session_id: session_id.to_string(),
        command: command.chars().take(200).collect(),
        exit_code,
        exit_code_attributable: exit_code_is_attributable(command),
        recorded_at: Instant::now(),
        kind,
    };
    let mut map = ring().lock_tolerant();
    map.entry(session_id.to_string())
        .or_default()
        .push(evidence);
}

/// Return the most-recent build evidence (excluding smoke) for a session,
/// or None when nothing fresh is on file. Sweeps expired entries as a
/// side effect.
pub fn most_recent_build(session_id: &str) -> Option<BuildEvidence> {
    let now = Instant::now();
    let mut map = ring().lock_tolerant();
    let Some(entries) = map.get_mut(session_id) else {
        return None;
    };
    entries.retain(|e| now.duration_since(e.recorded_at) < EVIDENCE_TTL);
    entries
        .iter()
        .filter(|e| e.kind != BuildEvidenceKind::Smoke)
        .max_by_key(|e| e.recorded_at)
        .cloned()
}

/// Return the most-recent smoke evidence for a session.
pub fn most_recent_smoke(session_id: &str) -> Option<BuildEvidence> {
    let now = Instant::now();
    let mut map = ring().lock_tolerant();
    let Some(entries) = map.get_mut(session_id) else {
        return None;
    };
    entries.retain(|e| now.duration_since(e.recorded_at) < EVIDENCE_TTL);
    entries
        .iter()
        .filter(|e| e.kind == BuildEvidenceKind::Smoke)
        .max_by_key(|e| e.recorded_at)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_npm_build() {
        assert_eq!(classify("npm run build"), Some(BuildEvidenceKind::Node));
        assert_eq!(
            classify("npm run build 2>&1 | tee build.log"),
            Some(BuildEvidenceKind::Node)
        );
        assert_eq!(classify("NPM RUN BUILD"), Some(BuildEvidenceKind::Node));
    }

    #[test]
    fn classifies_cargo() {
        assert_eq!(classify("cargo test"), Some(BuildEvidenceKind::Cargo));
        assert_eq!(
            classify("cargo check --all"),
            Some(BuildEvidenceKind::Cargo)
        );
    }

    #[test]
    fn classifies_smoke() {
        assert_eq!(
            classify("timeout 5 npm run dev > smoke.log 2>&1"),
            Some(BuildEvidenceKind::Smoke)
        );
    }

    #[test]
    fn rejects_non_build() {
        assert_eq!(classify("ls -la"), None);
        assert_eq!(classify("git status"), None);
        assert_eq!(classify("cat package.json"), None);
    }

    #[test]
    fn round_trip_record_and_recall() {
        let sid = "test-session-xyz";
        record(sid, "npm run build && echo done", 0);
        let recalled = most_recent_build(sid).expect("should have evidence");
        assert_eq!(recalled.kind, BuildEvidenceKind::Node);
        assert_eq!(recalled.exit_code, 0);
    }

    #[test]
    fn skips_unclassified_commands() {
        let sid = "test-skip-session";
        record(sid, "ls -la", 0);
        assert!(most_recent_build(sid).is_none());
    }
}

#[cfg(test)]
mod attribution_tests {
    use super::exit_code_is_attributable;

    /// Regression for the P0 post-phase audit finding F-23.
    ///
    /// This is the exact form the gate's own error message used to recommend.
    /// Its exit status is `echo`'s, which is always 0 — so a FAILED build was
    /// recorded as a pass, and P0 made that gate mandatory.
    #[test]
    fn the_recommended_pipeline_form_is_not_attributable() {
        assert!(!exit_code_is_attributable(
            "npm run build 2>&1 | tee build.log; echo \"EXIT:$?\""
        ));
    }

    #[test]
    fn pipelines_and_sequences_are_not_attributable() {
        for command in [
            "cargo build | tee out.log",
            "npm run build; echo done",
            "npm run build && npm test",
            "cargo test || true",
            "npm run build
cargo build",
        ] {
            assert!(
                !exit_code_is_attributable(command),
                "{command} reports someone else's exit status"
            );
        }
    }

    #[test]
    fn a_plain_command_is_attributable() {
        for command in [
            "npm run build",
            "cargo test --all-targets",
            "go build ./...",
            "npm run build 2>&1",
        ] {
            assert!(exit_code_is_attributable(command), "{command} should count");
        }
    }
}

#[cfg(test)]
mod p8_28_tests {
    //! `P8-28` — the build-evidence gate matched command TEXT.
    //!
    //! The gate that decides whether work may be called complete was satisfied
    //! by any command that merely *mentioned* a build.

    use super::*;

    // --- the exploits, every form named in the refutation ---------------------

    #[test]
    fn mentioning_a_build_is_not_building() {
        // 🔴 Each of these classified as a passing Node build and opened the
        // `qc_review` door with nothing built. The first is the plan's
        // example; the rest are the ACCIDENTAL forms, which matter more —
        // an agent does not have to be adversarial to run `rg`.
        for cmd in [
            r#"grep -rn "npm run build" package.json"#,
            "echo npm run build",
            r#"rg "npm run build" ."#,
            r#"git log --grep="npm run build""#,
            r#"sed -i 's/npm run build/pnpm build/' package.json"#,
            r#"cat README.md | grep "cargo build""#,
            "echo 'uv run pytest'",
        ] {
            assert_eq!(
                classify(cmd),
                None,
                "a command that only MENTIONS a build must not classify as one: {cmd}",
            );
        }
    }

    #[test]
    fn a_no_op_flag_is_not_a_build() {
        // Exits 0, builds nothing.
        for cmd in [
            "npm run build --help",
            "cargo build --version",
            "npm run build --dry-run",
        ] {
            assert_eq!(classify(cmd), None, "{cmd} builds nothing");
        }
    }

    // --- and the falsification: real builds must STILL classify ---------------

    #[test]
    fn a_real_build_still_classifies() {
        // ⚠ Without this, "reject everything" would satisfy the two tests
        // above while destroying the feature — `L-37`'s shape at the level of
        // a test suite rather than a single assertion.
        assert_eq!(classify("npm run build"), Some(BuildEvidenceKind::Node));
        assert_eq!(classify("npm run build --silent"), Some(BuildEvidenceKind::Node));
        assert_eq!(classify("cargo build"), Some(BuildEvidenceKind::Cargo));
        assert_eq!(classify("cargo test --all"), Some(BuildEvidenceKind::Cargo));
        assert_eq!(classify("uv run pytest"), Some(BuildEvidenceKind::Python));
    }

    #[test]
    fn a_build_in_a_later_segment_still_classifies() {
        // The build is not always the first thing on the line.
        assert_eq!(classify("cd frontend && npm run build"), Some(BuildEvidenceKind::Node));
        assert_eq!(classify("CI=1 npm run build"), Some(BuildEvidenceKind::Node));
        assert_eq!(
            classify("npm install; npm run build"),
            Some(BuildEvidenceKind::Node)
        );
    }

    #[test]
    fn the_asymmetry_that_made_this_severe_is_recorded() {
        // The gate rejects any pipe as non-attributable while accepting any
        // substring as proof of a build — aggressive about false negatives,
        // permissive about false positives. That gradient is what pushes a
        // rejected agent toward the magic substring.
        assert!(!exit_code_is_attributable("npm run build | tee log"));
        assert!(exit_code_is_attributable("npm run build"));
        // …and the permissive half is now closed:
        assert_eq!(classify(r#"echo "npm run build" | tee log"#), None);
    }
}
