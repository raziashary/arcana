// Doc 09 T2.4 — task-associated build evidence.
//
// The Rust-owned `project_command` records build/test/lint results only after
// it spawns the selected toolchain command and observes its exit status.
// Results are persisted in the task database, keyed to the worker's unique
// in-progress task, so `update_task_status_tool` can refuse `qc_review` when
// evidence is missing, belongs to another task, is unattributable, or failed.

#[derive(Debug, Clone)]
pub struct BuildEvidence {
    pub task_id: i64,
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
    pub recorded_at: String,
    pub kind: BuildEvidenceKind,
    pub workspace_path: Option<String>,
    pub workspace_fingerprint: Option<String>,
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
    /// `make build`, `make test`, `make lint`, `make run`
    Make,
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
            Self::Make => "make",
        }
    }
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

/// Persist a completed platform-owned build/test/lint invocation against the
/// worker's claimed task. Callers must invoke this only after the spawned
/// process returns an observed exit code.
pub fn record_result_for_worker(
    worker_slug: &str,
    session_id: &str,
    workspace_path: &std::path::Path,
    command: &str,
    exit_code: i32,
    kind: BuildEvidenceKind,
) -> Option<i64> {
    let result = (|| {
        let db = crate::db::tasks::TasksDb::open()?;
        record_for_worker_in(
            &db,
            worker_slug,
            session_id,
            workspace_path,
            command,
            exit_code,
            kind,
        )
    })();
    match result {
        Ok(streak) => streak,
        Err(error) => {
            tracing::warn!(
                "[BuildEvidence] could not persist build result for worker '{worker_slug}': {error}"
            );
            None
        }
    }
}

/// Persist all spawned build exits from one deterministic command invocation.
/// Returns true when the observed failure sequence reached the diagnostic
/// threshold of three consecutive nonzero exits.
pub fn record_results_for_worker(
    worker_slug: &str,
    session_id: &str,
    workspace_path: &std::path::Path,
    attempts: &[(String, i32, BuildEvidenceKind, Option<String>)],
    aggregate_failure: Option<usize>,
) -> crate::errors::Result<Option<bool>> {
    let db = crate::db::tasks::TasksDb::open()?;
    let persisted = attempts
        .iter()
        .map(|(command, exit_code, kind, fingerprint)| {
            (
                command.chars().take(200).collect::<String>(),
                *exit_code,
                kind.as_str().to_owned(),
                Some(workspace_path.to_string_lossy().into_owned()),
                fingerprint.clone(),
            )
        })
        .collect::<Vec<_>>();
    db.record_build_attempts_with_workspace_for_worker(
        worker_slug,
        session_id,
        &persisted,
        aggregate_failure,
    )
    .map(|result| result.map(|(_, _, diagnostic_reached)| diagnostic_reached))
}

pub fn begin_project_build_batch_for_worker(
    worker_slug: &str,
    session_id: &str,
    workspace_path: &std::path::Path,
    expected_count: usize,
    workspace_fingerprint: Option<&str>,
) -> crate::errors::Result<Option<crate::db::tasks::ProjectBuildBatch>> {
    let db = crate::db::tasks::TasksDb::open()?;
    db.begin_project_build_batch_for_worker(
        worker_slug,
        session_id,
        expected_count,
        &workspace_path.to_string_lossy(),
        workspace_fingerprint,
    )
}

pub fn finish_project_build_batch_for_worker(
    batch_id: i64,
    worker_slug: &str,
    session_id: &str,
    workspace_path: &std::path::Path,
    attempts: &[(String, i32, BuildEvidenceKind, Option<String>)],
    aggregate_failure: Option<usize>,
    all_children_completed: bool,
) -> crate::errors::Result<Option<bool>> {
    let db = crate::db::tasks::TasksDb::open()?;
    let persisted = attempts
        .iter()
        .map(|(command, exit_code, kind, fingerprint)| {
            (
                command.chars().take(200).collect::<String>(),
                *exit_code,
                kind.as_str().to_owned(),
                Some(workspace_path.to_string_lossy().into_owned()),
                fingerprint.clone(),
            )
        })
        .collect::<Vec<_>>();
    db.finish_project_build_batch_for_worker(
        batch_id,
        worker_slug,
        session_id,
        &persisted,
        aggregate_failure,
        all_children_completed,
    )
    .map(|result| result.map(|(_, _, diagnostic_reached)| diagnostic_reached))
}

/// Ensures an in-flight build batch stops authorizing QC if its caller is
/// cancelled or unwinds before normal evidence finalization. Completed child
/// exits are retained in memory and persisted as observed evidence on drop;
/// missing children remain missing rather than receiving fabricated exits.
pub(super) struct ProjectBuildBatchCancellationGuard {
    batch_id: i64,
    worker_slug: String,
    session_id: String,
    workspace_path: std::path::PathBuf,
    attempts: Vec<(String, i32, BuildEvidenceKind, Option<String>)>,
    armed: bool,
}

impl ProjectBuildBatchCancellationGuard {
    pub(super) fn new(
        batch_id: i64,
        worker_slug: &str,
        session_id: &str,
        workspace_path: &std::path::Path,
    ) -> Self {
        Self {
            batch_id,
            worker_slug: worker_slug.to_owned(),
            session_id: session_id.to_owned(),
            workspace_path: workspace_path.to_path_buf(),
            attempts: Vec::new(),
            armed: true,
        }
    }

    pub(super) fn record_observed_exit(
        &mut self,
        command: &str,
        exit_code: i32,
        kind: BuildEvidenceKind,
        fingerprint: Option<String>,
    ) {
        self.attempts
            .push((command.to_owned(), exit_code, kind, fingerprint));
    }

    pub(super) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ProjectBuildBatchCancellationGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // Drop also runs when the async caller is cancelled, where no await is
        // available. The existing DB API uses a short SQLite transaction and
        // marks this batch failed/incomplete without inventing a child result.
        match finish_project_build_batch_for_worker(
            self.batch_id,
            &self.worker_slug,
            &self.session_id,
            &self.workspace_path,
            &self.attempts,
            None,
            false,
        ) {
            Ok(Some(_)) => {}
            Ok(None) => tracing::warn!(
                target: "arcana::build_evidence",
                batch_id = self.batch_id,
                "cancelled project build batch was already finalized or unavailable"
            ),
            Err(_) => tracing::error!(
                target: "arcana::build_evidence",
                batch_id = self.batch_id,
                "could not persist cancellation outcome for project build batch"
            ),
        }
    }
}

pub(super) fn record_for_worker_in(
    db: &crate::db::tasks::TasksDb,
    worker_slug: &str,
    session_id: &str,
    workspace_path: &std::path::Path,
    command: &str,
    exit_code: i32,
    kind: BuildEvidenceKind,
) -> crate::errors::Result<Option<i64>> {
    let workspace_fingerprint =
        crate::platform::workspace_fingerprint::fingerprint(workspace_path).ok();
    let attempts = [(
        command.chars().take(200).collect::<String>(),
        exit_code,
        kind.as_str().to_owned(),
        Some(workspace_path.to_string_lossy().into_owned()),
        workspace_fingerprint,
    )];
    let recorded = db.record_build_attempts_with_workspace_for_worker(
        worker_slug,
        session_id,
        &attempts,
        None,
    )?;
    Ok(recorded.map(|(_, consecutive_failures, _)| consecutive_failures))
}

pub fn most_recent_build(task_id: i64) -> crate::errors::Result<Option<BuildEvidence>> {
    let db = crate::db::tasks::TasksDb::open()?;
    most_recent_build_in(&db, task_id)
}

pub fn qc_gate_error(task_id: i64) -> crate::errors::Result<Option<String>> {
    let db = crate::db::tasks::TasksDb::open()?;
    qc_gate_error_in(&db, task_id)
}

pub fn qc_gate_error_for_workspace(
    task_id: i64,
    selected_workspace: &std::path::Path,
) -> crate::errors::Result<Option<String>> {
    let db = crate::db::tasks::TasksDb::open()?;
    qc_gate_error_in_for_workspace(&db, task_id, Some(selected_workspace))
}

pub(crate) fn qc_gate_error_in(
    db: &crate::db::tasks::TasksDb,
    task_id: i64,
) -> crate::errors::Result<Option<String>> {
    qc_gate_error_in_for_workspace(db, task_id, None)
}

pub(crate) fn qc_gate_error_in_for_workspace(
    db: &crate::db::tasks::TasksDb,
    task_id: i64,
    selected_workspace: Option<&std::path::Path>,
) -> crate::errors::Result<Option<String>> {
    let batch = db.latest_project_build_batch_status(task_id)?;
    Ok(match batch {
        None => Some(format!(
            "Cannot move task to qc_review without a complete project build batch for task #{task_id}."
        )),
        Some((outcome, expected, observed, passing, path, fingerprint, evidence_rows, bad_rows, _))
            if outcome != "passed"
                || expected <= 0
                || observed != expected
                || passing != expected
                || evidence_rows != expected
                || bad_rows != 0
                || !fingerprint.as_ref().is_some_and(|expected| {
                    crate::platform::workspace_fingerprint::fingerprint(
                        std::path::Path::new(&path),
                    )
                    .is_ok_and(|actual| actual == *expected)
                }) =>
        {
            Some("The latest project build batch is incomplete, failing, or no longer matches the current workspace. Rerun project_command(intent='build') after checking the selected workspace.".to_string())
        }
        Some((_, _, _, _, path, _, _, _, _))
            if !selected_workspace.is_some_and(|selected| {
                crate::platform::workspace::workspace_identity(selected)
                    .is_ok_and(|selected| {
                        crate::platform::workspace::workspace_identity(
                            std::path::Path::new(&path),
                        )
                        .is_ok_and(|batch| selected == batch)
                    })
            }) =>
        {
            Some("The latest passing batch belongs to a different or unverified selected workspace. Run project_command(intent='build') in the workspace currently selected for this task.".to_string())
        }
        Some(_) => None,
    })
}

pub(crate) fn most_recent_build_in(
    db: &crate::db::tasks::TasksDb,
    task_id: i64,
) -> crate::errors::Result<Option<BuildEvidence>> {
    db.latest_build_evidence(task_id).map(|row| {
        row.map(|row| BuildEvidence {
            task_id: row.task_id,
            session_id: row.session_id,
            command: row.command,
            exit_code: row.exit_code,
            exit_code_attributable: row.exit_code_attributable,
            recorded_at: row.created_at,
            kind: BuildEvidenceKind::from_str(&row.kind),
            workspace_path: row.workspace_path,
            workspace_fingerprint: row.workspace_fingerprint,
        })
    })
}

impl BuildEvidenceKind {
    fn from_str(kind: &str) -> Self {
        match kind {
            "node" => Self::Node,
            "cargo" => Self::Cargo,
            "python" => Self::Python,
            "go" => Self::Go,
            "smoke" => Self::Smoke,
            "make" => Self::Make,
            _ => unreachable!("database CHECK constraint restricts evidence kind"),
        }
    }
}

/*
 * Historical session-scoped, process-local evidence was removed. The database
 * query intentionally keys by task, so records from a different task or a
 * restarted application cannot satisfy the QC gate.
 */

#[cfg(test)]
mod tests {
    use super::*;

    fn create_task(db: &crate::db::tasks::TasksDb, title: &str) -> i64 {
        db.create(crate::db::tasks::CreateTaskInput {
            title: title.into(),
            description: None,
            priority: "p2".into(),
            task_type: "feature".into(),
            acceptance_criteria: "build".into(),
            created_by: None,
        })
        .unwrap()
        .id
    }

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
        let path =
            std::env::temp_dir().join(format!("arcana-build-evidence-{}.db", uuid::Uuid::new_v4()));
        let db = crate::db::tasks::TasksDb::open_at(&path).unwrap();
        let task = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "build evidence".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "build".into(),
                created_by: None,
            })
            .unwrap();
        db.insert_build_evidence(task.id, "test-session", "npm run build", 0, true, "node")
            .unwrap();
        drop(db);
        let db = crate::db::tasks::TasksDb::open_at(&path).unwrap();
        let recalled = most_recent_build_in(&db, task.id)
            .unwrap()
            .expect("should have evidence");
        assert_eq!(recalled.kind, BuildEvidenceKind::Node);
        assert_eq!(recalled.exit_code, 0);
        assert_eq!(recalled.task_id, task.id);
        assert_eq!(recalled.session_id, "test-session");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn completed_process_is_persisted_for_the_claimed_task_and_consumed_after_reopen() {
        let path = std::env::temp_dir().join(format!(
            "arcana-build-worker-evidence-{}.db",
            uuid::Uuid::new_v4()
        ));
        let db = crate::db::tasks::TasksDb::open_at(&path).unwrap();
        let task_id = create_task(&db, "claimed build");
        db.update_status(task_id, "assigned", Some("worker-backend"))
            .unwrap();
        db.claim_task_with_pid(task_id, "worker-backend", 1234)
            .unwrap();
        assert_eq!(
            record_for_worker_in(
                &db,
                "worker-backend",
                "worker-session",
                &std::env::current_dir().unwrap(),
                "cargo test --all-targets",
                0,
                BuildEvidenceKind::Cargo
            )
            .unwrap(),
            Some(0)
        );
        let (workspace, fingerprint) = crate::db::tasks::synthetic_build_evidence_workspace();
        let workspace_path = workspace.to_string_lossy().into_owned();
        let batch = db
            .begin_project_build_batch_for_worker(
                "worker-backend",
                "worker-session",
                1,
                &workspace_path,
                Some(fingerprint),
            )
            .unwrap()
            .expect("claimed task starts an attributable project batch");
        db.finish_project_build_batch_for_worker(
            batch.id,
            "worker-backend",
            "worker-session",
            &[(
                "cargo test --all-targets".into(),
                0,
                "cargo".into(),
                Some(workspace_path),
                Some(fingerprint.clone()),
            )],
            None,
            true,
        )
        .unwrap()
        .expect("persist the observed passing project build");
        drop(db);

        let reopened = crate::db::tasks::TasksDb::open_at(&path).unwrap();
        let evidence = most_recent_build_in(&reopened, task_id)
            .unwrap()
            .expect("durable evidence survives database reopen");
        assert_eq!(evidence.task_id, task_id);
        assert_eq!(evidence.session_id, "worker-session");
        assert_eq!(evidence.exit_code, 0);
        assert_eq!(
            qc_gate_error_in_for_workspace(
                &reopened,
                task_id,
                Some(&crate::db::tasks::synthetic_build_evidence_workspace().0),
            )
            .unwrap(),
            None
        );
        assert!(qc_gate_error_in(&reopened, task_id + 1).unwrap().is_some());
        drop(reopened);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn qc_gate_refuses_failed_or_unattributable_legacy_batches() {
        let path =
            std::env::temp_dir().join(format!("arcana-build-gate-{}.db", uuid::Uuid::new_v4()));
        let db = crate::db::tasks::TasksDb::open_at(&path).unwrap();
        let task_id = create_task(&db, "failing build");
        db.insert_build_evidence(task_id, "s1", "npm run build", 1, true, "node")
            .unwrap();
        let error = qc_gate_error_in_for_workspace(
            &db,
            task_id,
            Some(&crate::db::tasks::synthetic_build_evidence_workspace().0),
        )
        .unwrap()
        .expect("legacy result cannot satisfy current batch contract");
        assert!(
            error.contains("latest project build batch is incomplete"),
            "unexpected QC error: {error}"
        );
        db.insert_build_evidence(task_id, "s1", "npm run build | tee log", 0, false, "node")
            .unwrap();
        assert!(qc_gate_error_in_for_workspace(
            &db,
            task_id,
            Some(&crate::db::tasks::synthetic_build_evidence_workspace().0),
        )
        .unwrap()
        .unwrap()
        .contains("latest project build batch is incomplete"));
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn evidence_requires_one_in_progress_task_for_the_worker() {
        let path = std::env::temp_dir().join(format!(
            "arcana-build-task-resolution-{}.db",
            uuid::Uuid::new_v4()
        ));
        let db = crate::db::tasks::TasksDb::open_at(&path).unwrap();
        assert_eq!(
            record_for_worker_in(
                &db,
                "worker-backend",
                "s1",
                &std::env::current_dir().unwrap(),
                "cargo test",
                0,
                BuildEvidenceKind::Cargo
            )
            .unwrap(),
            None
        );
        let first = create_task(&db, "one");
        let second = create_task(&db, "two");
        db.update_status(first, "assigned", Some("worker-backend"))
            .unwrap();
        db.claim_task_with_pid(first, "worker-backend", (first + 100) as u32)
            .unwrap();
        db.update_status(second, "assigned", Some("worker-backend"))
            .unwrap();
        // The shared atomic claim API prevents a second active task. Direct
        // SQL is outside that API contract, so exercise the supported claim
        // boundary rather than expecting the metadata trigger to enforce load.
        assert!(db
            .claim_task_with_pid(second, "worker-backend", 9999)
            .is_err());
        assert!(record_for_worker_in(
            &db,
            "worker-backend",
            "s1",
            &std::env::current_dir().unwrap(),
            "cargo test",
            0,
            BuildEvidenceKind::Cargo
        )
        .unwrap()
        .is_some());
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn skips_unclassified_commands() {
        assert_eq!(classify("ls -la"), None);
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
        assert_eq!(
            classify("npm run build --silent"),
            Some(BuildEvidenceKind::Node)
        );
        assert_eq!(classify("cargo build"), Some(BuildEvidenceKind::Cargo));
        assert_eq!(classify("cargo test --all"), Some(BuildEvidenceKind::Cargo));
        assert_eq!(classify("uv run pytest"), Some(BuildEvidenceKind::Python));
    }

    #[test]
    fn a_build_in_a_later_segment_still_classifies() {
        // The build is not always the first thing on the line.
        assert_eq!(
            classify("cd frontend && npm run build"),
            Some(BuildEvidenceKind::Node)
        );
        assert_eq!(
            classify("CI=1 npm run build"),
            Some(BuildEvidenceKind::Node)
        );
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
