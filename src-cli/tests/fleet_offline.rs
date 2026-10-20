#![cfg(feature = "offline-fleet-test")]

use arcana_core::db::runs::RunsDb;
use arcana_core::db::tasks::{TaskStatus, TasksDb};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};
use uuid::Uuid;

struct IsolatedProfile {
    root: PathBuf,
    token: String,
    workspace: PathBuf,
}

impl IsolatedProfile {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("arcana-cli-fleet-offline-{}", Uuid::new_v4()));
        let arcana = root.join(".arcana");
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&arcana).expect("create isolated Arcana profile");
        std::fs::create_dir_all(&workspace).expect("create isolated workspace");
        let fixture = workspace.join("fleet-fixture");
        std::fs::create_dir_all(fixture.join("src")).expect("create dependency-free Cargo fixture");
        std::fs::write(
            fixture.join("Cargo.toml"),
            "[package]\nname = \"fleet-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("write fixture manifest");
        std::fs::write(
            fixture.join("src").join("lib.rs"),
            "pub fn offline_fixture() -> bool { true }\n",
        )
        .expect("write fixture library");
        let lock = Command::new("cargo")
            .current_dir(&fixture)
            .env_remove("GOOGLE_API_KEY")
            .env_remove("GEMINI_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove("DATABASE_URL")
            .env_remove("DIRECT_URL")
            .args(["generate-lockfile", "--offline"])
            .output()
            .expect("generate dependency-free fixture lockfile offline");
        assert!(
            lock.status.success(),
            "offline fixture lock generation failed: {}",
            String::from_utf8_lossy(&lock.stderr)
        );
        let token = Uuid::new_v4().to_string();
        std::fs::write(arcana.join("offline-fleet-test.marker"), &token)
            .expect("write test marker");
        Self {
            root,
            token,
            workspace,
        }
    }

    fn run_fleet(&self, scenario: &str, title: &str, timeout_secs: u64) -> Output {
        let stdout_path = self.root.join("fleet-stdout.log");
        let stderr_path = self.root.join("fleet-stderr.log");
        let mut child = Command::new(env!("CARGO_BIN_EXE_arcanactl"))
            .env("USERPROFILE", &self.root)
            .env("HOME", &self.root)
            .env("ARCANA_OFFLINE_FLEET_SCENARIO", scenario)
            .env("ARCANA_OFFLINE_FLEET_MARKER_ID", &self.token)
            .env_remove("GOOGLE_API_KEY")
            .env_remove("GEMINI_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove("DATABASE_URL")
            .env_remove("DIRECT_URL")
            .args([
                "--quiet", "--output-format", "json", "--provider", "google",
                "--working-dir", self.workspace.to_str().expect("workspace path"),
                "fleet", "run", "--title", title, "--description", "Exercise real CLI Fleet board flow",
                "--acceptance-criteria", "The locked offline Cargo fixture builds successfully; worker build evidence is reviewed before completion.",
                "--timeout-secs", &timeout_secs.to_string(),
            ])
            .stdout(std::fs::File::create(&stdout_path).expect("create isolated stdout capture"))
            .stderr(std::fs::File::create(&stderr_path).expect("create isolated stderr capture"))
            .spawn()
            .expect("run isolated arcanactl Fleet subprocess");
        let started = Instant::now();
        let outer_deadline = Duration::from_secs(timeout_secs.saturating_add(30));
        let status = loop {
            if let Some(status) = child.try_wait().expect("poll owned CLI child") {
                break status;
            }
            if started.elapsed() >= outer_deadline {
                #[cfg(windows)]
                {
                    let stopped = Command::new("taskkill.exe")
                        .args(["/PID", &child.id().to_string(), "/T", "/F"])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status()
                        .expect("stop owned hung CLI process tree");
                    assert!(stopped.success(), "owned CLI tree termination failed");
                }
                #[cfg(not(windows))]
                child.kill().expect("stop owned hung CLI process");
                child.wait().expect("reap owned timed-out CLI process");
                panic!("CLI Fleet exceeded its outer test deadline of {outer_deadline:?}");
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        let read_capture = |path| {
            let mut bytes = Vec::new();
            std::fs::File::open(path)
                .expect("open isolated CLI capture")
                .take(4 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .expect("read bounded isolated CLI capture");
            assert!(
                bytes.len() <= 4 * 1024 * 1024,
                "CLI capture exceeded test bound"
            );
            bytes
        };
        Output {
            status,
            stdout: read_capture(stdout_path),
            stderr: read_capture(stderr_path),
        }
    }

    fn grant_workspace_egress(&self) {
        let output = Command::new(env!("CARGO_BIN_EXE_arcanactl"))
            .env("USERPROFILE", &self.root)
            .env("HOME", &self.root)
            .env_remove("GOOGLE_API_KEY")
            .env_remove("GEMINI_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove("DATABASE_URL")
            .env_remove("DIRECT_URL")
            .args(["--quiet", "consent", "grant", "--workspace"])
            .arg(&self.workspace)
            .args(["--provider", "google"])
            .output()
            .expect("grant provider egress for disposable test workspace");
        assert!(
            output.status.success(),
            "isolated workspace consent grant failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn tasks(&self) -> TasksDb {
        TasksDb::open_at(&self.root.join(".arcana").join("tasks.db")).expect("open CLI task board")
    }

    fn persist_failure_receipt(&self, title: &str, changed: &[String], stderr: &[u8]) -> PathBuf {
        let fleet_events = safe_fleet_event_summary(&self.root.join(".arcana").join("logs"));
        let board = self.tasks();
        let tasks = board
            .list(None)
            .expect("read failed Fleet board for receipt");
        let epic = tasks.iter().find(|task| task.title == title);
        let children = epic
            .and_then(|task| board.get_epic_tasks(&task.id.to_string()).ok())
            .unwrap_or_default();
        let child_rows = children
            .iter()
            .map(|task| {
                let batch = board
                    .latest_project_build_batch_status(task.id)
                    .ok()
                    .flatten();
                serde_json::json!({
                    "id": task.id,
                    "status": task.status.as_str(),
                    "assigned_worker": task.assigned_worker,
                    "under_review_by": task.under_review_by,
                    "epic_id": task.epic_id,
                    "latest_build_batch": batch.map(|row| serde_json::json!({
                        "outcome": row.0,
                        "expected_count": row.1,
                        "observed_count": row.2,
                        "passing_count": row.3,
                        "evidence_count": row.6,
                        "invalid_evidence_count": row.7,
                        "selected_workspace_matches": row.8 == 1,
                    })),
                })
            })
            .collect::<Vec<_>>();
        let mut lines = vec![serde_json::json!({
            "kind": "fleet_failure_snapshot",
            "workspace_input_changes": changed,
            "fleet_events": fleet_events,
            "epic": epic.map(|task| serde_json::json!({
                "id": task.id,
                "status": task.status.as_str(),
                "task_type": task.task_type.as_str(),
            })),
            "children": child_rows,
        })
        .to_string()];
        for line in String::from_utf8_lossy(stderr).lines() {
            if !line.contains("offline Fleet script did not match turn") {
                continue;
            }
            let Some(encoded) = line.split("tool_results=").nth(1) else {
                continue;
            };
            let Some(encoded) = encoded.strip_suffix(')') else {
                continue;
            };
            let Ok(tool_results) = serde_json::from_str::<serde_json::Value>(encoded) else {
                continue;
            };
            lines.push(
                serde_json::json!({
                    "kind": "scripted_provider_last_tool_results",
                    "supervisor": line.contains("supervisor=true"),
                    "qc_dispatcher": line.contains("qc_dispatcher=true"),
                    "qc_reviewer": line.contains("qc_reviewer=true"),
                    "worker": line.contains("worker=true"),
                    "tool_results": tool_results,
                })
                .to_string(),
            );
        }
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join("target")
            .join("p4-50-offline-fleet-receipts");
        std::fs::create_dir_all(&directory).expect("create ignored CLI test receipt directory");
        let path = directory.join(format!("{}.jsonl", Uuid::new_v4()));
        std::fs::write(&path, format!("{}\n", lines.join("\n")))
            .expect("write sanitized CLI failure receipt");
        path.file_name()
            .map(PathBuf::from)
            .expect("receipt filename")
    }

    fn export(&self) -> (String, serde_json::Value) {
        let logs = self.root.join("fleet-logs.jsonl");
        let log_output = Command::new(env!("CARGO_BIN_EXE_arcanactl"))
            .env("USERPROFILE", &self.root)
            .env("HOME", &self.root)
            .env_remove("GOOGLE_API_KEY")
            .env_remove("GEMINI_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove("DATABASE_URL")
            .env_remove("DIRECT_URL")
            .args(["--quiet", "logs", "export"])
            .arg(&logs)
            .output()
            .expect("invoke CLI logs export reader");
        assert!(
            log_output.status.success(),
            "logs export failed: {}",
            String::from_utf8_lossy(&log_output.stderr)
        );
        let logs = std::fs::read_to_string(logs).expect("read CLI-exported logs");

        let analytics = self.root.join("fleet-analytics.json");
        let analytics_output = Command::new(env!("CARGO_BIN_EXE_arcanactl"))
            .env("USERPROFILE", &self.root)
            .env("HOME", &self.root)
            .env_remove("GOOGLE_API_KEY")
            .env_remove("GEMINI_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove("DATABASE_URL")
            .env_remove("DIRECT_URL")
            .args([
                "--quiet",
                "--output-format",
                "json",
                "analytics",
                "export",
                "--output",
            ])
            .arg(&analytics)
            .output()
            .expect("invoke CLI analytics export reader");
        assert!(
            analytics_output.status.success(),
            "analytics export failed: {}",
            String::from_utf8_lossy(&analytics_output.stderr)
        );
        let analytics =
            serde_json::from_slice(&std::fs::read(analytics).expect("read CLI-exported analytics"))
                .expect("parse CLI-exported analytics JSON");
        (logs, analytics)
    }
}

#[test]
fn offline_fleet_failure_receipt_keeps_safe_fleet_events_only() {
    let directory =
        std::env::temp_dir().join(format!("arcana-offline-fleet-events-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory).expect("create isolated structured log directory");
    let log = directory.join("arcana-events.2026-10-12.jsonl");
    let mut fixture = concat!(
            "{\"timestamp\":\"2026-10-12T10:00:00Z\",\"target\":\"arcana::telemetry\",\"fields\":{\"event_type\":\"fleet_worker_tick\",\"worker\":\"qc-agent\",\"outcome\":\"error\",\"duration_ns\":1200,\"correlation_id\":\"ed11967f-8b05-41d8-8795-577b1f462fd5\",\"message\":\"private prompt payload\",\"command\":\"private command\"}}\n",
            "{\"timestamp\":\"2026-10-12T10:00:02Z\",\"target\":\"arcana::telemetry\",\"fields\":{\"event_type\":\"offline_fleet_test_step\",\"actor_code\":2,\"step_code\":10,\"scenario_code\":0,\"prior_tool_count\":3,\"tool_count\":12,\"decision_code\":0,\"message\":\"private prompt payload\"}}\n",
            "{\"timestamp\":\"2026-10-12T10:00:01Z\",\"target\":\"arcana::telemetry\",\"fields\":{\"event_type\":\"unregistered_private_event\",\"message\":\"must not be copied\"}}\n"
        )
        .to_owned();
    for _ in 0..300 {
        fixture.push_str(
            "{\"timestamp\":\"2026-10-12T10:00:03Z\",\"target\":\"arcana::telemetry\",\"fields\":{\"event_type\":\"database_statement\",\"duration_ms\":1}}\n",
        );
    }
    std::fs::write(&log, fixture).expect("write sanitized event fixture");

    let summary = safe_fleet_event_summary(&directory);
    assert_eq!(
        summary.len(),
        2,
        "only registered fleet and offline test events remain"
    );
    assert_eq!(summary[0]["worker"], "qc-agent");
    assert_eq!(summary[0]["outcome"], "error");
    assert_eq!(summary[0]["duration_ns"], 1200);
    assert_eq!(
        summary[0]["correlation_id"],
        "ed11967f-8b05-41d8-8795-577b1f462fd5"
    );
    assert_eq!(summary[1]["event_type"], "offline_fleet_test_step");
    assert_eq!(summary[1]["actor_code"], 2);
    assert_eq!(summary[1]["step_code"], 10);
    assert_eq!(summary[1]["decision_code"], 0);
    let serialized = serde_json::to_string(&summary).unwrap();
    assert!(!serialized.contains("private prompt payload"));
    assert!(!serialized.contains("private command"));
    assert!(!serialized.contains("must not be copied"));
    std::fs::remove_dir_all(directory).expect("remove isolated structured log directory");
}

fn safe_fleet_event_summary(log_dir: &Path) -> Vec<serde_json::Value> {
    const FLEET_EVENT_TYPES: &[&str] = &[
        "fleet_cli_run_blocked",
        "fleet_cli_run_completed",
        "fleet_cli_run_started",
        "fleet_cli_run_timeout",
        "fleet_cli_supervisor_failed",
        "fleet_worker_tick",
        "offline_fleet_test_step",
    ];
    const WORKERS: &[&str] = &[
        "supervisor",
        "worker-backend",
        "worker-frontend",
        "worker-platform",
        "worker-mcp",
        "worker-cleanup",
        "qc-agent",
    ];
    let Ok(entries) = std::fs::read_dir(log_dir) else {
        return Vec::new();
    };
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("arcana-events.") && name.ends_with(".jsonl"))
        })
        .collect::<Vec<_>>();
    paths.sort();

    let mut summaries = Vec::new();
    for path in paths {
        let Ok(contents) = std::fs::read_to_string(path) else {
            continue;
        };
        for line in contents.lines() {
            if !line.contains("\"event_type\"") {
                continue;
            }
            let Ok(record) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let Some(fields) = record.get("fields").and_then(serde_json::Value::as_object) else {
                continue;
            };
            let Some(event_type) = fields
                .get("event_type")
                .and_then(serde_json::Value::as_str)
                .filter(|event| FLEET_EVENT_TYPES.contains(event))
            else {
                continue;
            };

            let mut summary = serde_json::Map::new();
            summary.insert(
                "event_type".into(),
                serde_json::Value::String(event_type.to_owned()),
            );
            if let Some(timestamp) = record.get("timestamp").and_then(serde_json::Value::as_str) {
                summary.insert(
                    "timestamp".into(),
                    serde_json::Value::String(timestamp.to_owned()),
                );
            }
            for key in [
                "task_id",
                "worker_count",
                "timeout_secs",
                "child_count",
                "elapsed_ms",
                "duration_ns",
                "actor_code",
                "step_code",
                "scenario_code",
                "prior_tool_count",
                "tool_count",
                "decision_code",
            ] {
                if let Some(value) = fields.get(key).filter(|value| value.is_number()) {
                    summary.insert(key.into(), value.clone());
                }
            }
            if let Some(worker) = fields
                .get("worker")
                .and_then(serde_json::Value::as_str)
                .filter(|worker| WORKERS.contains(worker))
            {
                summary.insert(
                    "worker".into(),
                    serde_json::Value::String(worker.to_owned()),
                );
            }
            for key in ["status", "outcome"] {
                if let Some(value) =
                    fields
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .filter(|value| {
                            !value.is_empty()
                                && value.len() <= 64
                                && value.bytes().all(|byte| {
                                    byte.is_ascii_lowercase()
                                        || byte.is_ascii_digit()
                                        || byte == b'_'
                                })
                        })
                {
                    summary.insert(key.into(), serde_json::Value::String(value.to_owned()));
                }
            }
            if let Some(id) = fields
                .get("correlation_id")
                .and_then(serde_json::Value::as_str)
                .filter(|id| {
                    id.len() == 36
                        && id.bytes().enumerate().all(|(index, byte)| {
                            matches!(index, 8 | 13 | 18 | 23) && byte == b'-'
                                || !matches!(index, 8 | 13 | 18 | 23)
                                    && (byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                        })
                })
            {
                summary.insert(
                    "correlation_id".into(),
                    serde_json::Value::String(id.to_owned()),
                );
            }
            summaries.push(serde_json::Value::Object(summary));
        }
    }
    if summaries.len() > 256 {
        summaries.drain(..summaries.len() - 256);
    }
    summaries
}

fn workspace_input_snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    const GENERATED_DIRECTORIES: &[&str] = &[
        ".arcana-cache",
        ".arcana-tmp",
        ".git",
        ".mypy_cache",
        ".next",
        ".pytest_cache",
        ".ruff_cache",
        ".turbo",
        ".venv",
        "__pycache__",
        "coverage",
        "dist",
        "node_modules",
        "target",
        "venv",
    ];
    fn visit(root: &Path, dir: &Path, files: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in std::fs::read_dir(dir).expect("enumerate disposable workspace") {
            let entry = entry.expect("read disposable workspace entry");
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("workspace-relative path")
                .to_path_buf();
            let kind = entry.file_type().expect("read workspace entry type");
            if kind.is_dir()
                && GENERATED_DIRECTORIES.contains(&entry.file_name().to_string_lossy().as_ref())
            {
                continue;
            }
            if kind.is_dir() {
                files.insert(relative, None);
                visit(root, &path, files);
            } else if kind.is_symlink() {
                files.insert(
                    relative,
                    Some(
                        std::fs::read_link(path)
                            .expect("read workspace symlink")
                            .to_string_lossy()
                            .as_bytes()
                            .to_vec(),
                    ),
                );
            } else {
                files.insert(
                    relative,
                    Some(std::fs::read(path).expect("read workspace input")),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

fn changed_workspace_inputs(
    before: &BTreeMap<PathBuf, Option<Vec<u8>>>,
    after: &BTreeMap<PathBuf, Option<Vec<u8>>>,
) -> Vec<String> {
    before
        .keys()
        .chain(after.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|path| before.get(*path) != after.get(*path))
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect()
}

fn fleet_failure_diagnostics(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .filter(|line| {
            line.contains("offline Fleet script did not match turn")
                || line.contains("fleet_cli_run_timeout")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl Drop for IsolatedProfile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn offline_fleet_denied_preflight_persists_content_free_audit() {
    let profile = IsolatedProfile::new();
    let title = format!("Unconsented Fleet {}", Uuid::new_v4());
    let output = profile.run_fleet("success", &title, 10);
    assert!(
        !output.status.success(),
        "unconsented Fleet must stop before dispatch"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Provider egress is not authorized"),
        "Fleet should identify the missing grant"
    );
    let config = arcana_core::db::config_db::open_at(&profile.root.join(".arcana/config.db"))
        .expect("open isolated consent audit");
    let row: (String, String, String, String) = config
        .query_row(
            "SELECT workspace_path, destination, operation, outcome
             FROM workspace_egress_audit ORDER BY id DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read denied Fleet audit row");
    assert_eq!(
        row.0,
        profile.workspace.canonicalize().unwrap().to_string_lossy()
    );
    assert_eq!(row.1, "provider:google");
    assert_eq!(row.2, "fleet_cli_preflight");
    assert_eq!(row.3, "denied");
    assert!(
        !row.0.contains(&title),
        "task content must not enter the audit"
    );
    assert!(profile.tasks().list(None).unwrap().is_empty());
}

#[test]
fn offline_fleet_cli_completes_board_child_and_exports_observability() {
    let profile = IsolatedProfile::new();
    profile.grant_workspace_egress();
    let title = format!("Offline Fleet success {}", Uuid::new_v4());
    let before = workspace_input_snapshot(&profile.workspace);
    let output = profile.run_fleet("success", &title, 120);
    let changed = changed_workspace_inputs(&before, &workspace_input_snapshot(&profile.workspace));
    let receipt = (!output.status.success())
        .then(|| profile.persist_failure_receipt(&title, &changed, &output.stderr));
    assert!(
        output.status.success(),
        "Fleet failed (workspace input paths changed: {changed:?}; sanitized receipt: {:?}): {}",
        receipt,
        fleet_failure_diagnostics(&output.stderr)
    );
    let board = profile.tasks();
    let epic = board
        .list(None)
        .expect("read persisted board")
        .into_iter()
        .find(|task| task.title == title)
        .expect("CLI-created epic on board");
    assert_eq!(
        epic.status,
        TaskStatus::Done,
        "parent epic completion must come from child completion"
    );
    let children = board
        .get_epic_tasks(&epic.id.to_string())
        .expect("read epic children");
    assert_eq!(children.len(), 1);
    assert_eq!(
        children[0].status,
        TaskStatus::Done,
        "worker must consume and complete the assigned task"
    );
    assert_eq!(
        children[0].assigned_worker.as_deref(),
        Some("worker-backend")
    );

    let (logs, analytics) = profile.export();
    assert!(
        logs.contains("fleet_cli_run_completed"),
        "CLI log export must read the Fleet completion event"
    );
    assert!(
        logs.contains(&epic.id.to_string()),
        "CLI log export must retain the Fleet task identity"
    );
    assert!(
        analytics["worker_stats"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|row| row["worker_id"] == "worker-backend")),
        "analytics CLI export must read worker stats produced by the actual tick"
    );

    // Memory coverage: the run proposed a real fact and then retrieved it, so the
    // exported telemetry carries the memory gateway and search operations — not
    // only task and build events.
    assert!(
        logs.contains("memory_proposal") && logs.contains("memory_search_all"),
        "CLI log export must read the memory gateway and retrieval the worker performed"
    );
    // ...and the export carries the operations without the memory contents the
    // gate forbids (the fact below is exactly what the scripted worker proposed).
    const PROPOSED_FACT: &str = "The offline Fleet fixture is built from fleet-fixture with a locked Cargo.lock and no network access.";
    assert!(
        !logs.contains(PROPOSED_FACT),
        "telemetry export must never carry memory contents"
    );
    // The absence above is only meaningful if the proposal really wrote: the
    // gateway's insert statements are recorded as children of the
    // `memory_proposal` span, with the memory database named.
    let proposal_inserts = logs
        .lines()
        .filter(|line| line.contains("\"operation\":\"memory_proposal\""))
        .filter(|line| line.contains("\"operation\":\"insert\""))
        .filter(|line| line.contains("\"database\":\"memory\""))
        .count();
    assert!(
        proposal_inserts > 0,
        "the proposal must record its memory-database insert under the memory_proposal span, \
         otherwise the content-free export proves nothing"
    );

    // Supervisor/worker lifecycle readback from the durable run ledger, not just
    // the task board.
    let runs = RunsDb::open_at(&profile.root.join(".arcana").join("runs.db"))
        .expect("open CLI run ledger");
    let supervisor = runs
        .latest_worker_tick("supervisor")
        .expect("read supervisor lifecycle row")
        .expect("a supervisor tick must be recorded");
    assert!(
        supervisor.outcome.is_some(),
        "a finished supervisor tick must record an outcome, not only a heartbeat"
    );
    let worker = runs
        .latest_worker_tick("worker-backend")
        .expect("read worker lifecycle row")
        .expect("a worker tick must be recorded");
    assert_eq!(
        worker.task_id,
        Some(children[0].id),
        "the worker lifecycle row must carry the task it consumed"
    );
    assert!(
        worker.outcome.is_some(),
        "a finished worker tick must record an outcome"
    );
}

#[test]
fn offline_fleet_cli_completes_when_telemetry_cannot_persist() {
    // TG4-24 failure injection through a real Fleet run, with no test seam: the
    // production degradation path is reached by making the log directory
    // unpreparable on disk (a file where `~/.arcana/logs` must be a directory).
    // The CLI then initialises console-only, exactly as it would on a broken
    // profile.
    let profile = IsolatedProfile::new();
    let sink = profile.root.join(".arcana").join("logs");
    std::fs::write(&sink, "not a directory").expect("block the log directory");
    profile.grant_workspace_egress();

    let title = format!("Offline Fleet without a telemetry sink {}", Uuid::new_v4());
    let before = workspace_input_snapshot(&profile.workspace);
    let output = profile.run_fleet("success", &title, 120);
    let changed = changed_workspace_inputs(&before, &workspace_input_snapshot(&profile.workspace));
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "Fleet must complete when telemetry cannot persist: {}",
        fleet_failure_diagnostics(&output.stderr)
    );

    // (i) The actual work and its durable result are unaffected.
    let board = profile.tasks();
    let epic = board
        .list(None)
        .expect("read persisted board")
        .into_iter()
        .find(|task| task.title == title)
        .expect("CLI-created epic survives a missing telemetry sink");
    assert_eq!(
        epic.status,
        TaskStatus::Done,
        "epic completion comes from child completion and must not depend on telemetry"
    );
    let children = board
        .get_epic_tasks(&epic.id.to_string())
        .expect("read epic children");
    assert_eq!(children.len(), 1);
    assert_eq!(
        children[0].status,
        TaskStatus::Done,
        "the worker's task result must survive with no telemetry sink"
    );
    assert_eq!(
        children[0].assigned_worker.as_deref(),
        Some("worker-backend")
    );
    // The worker's *actual* result is the compiled fixture, not a status flag:
    // telemetry failure must leave the real artifact in place.
    assert!(
        changed.is_empty(),
        "a telemetry persistence failure must not disturb workspace inputs: {changed:?}"
    );

    // (ii) No partial stream was created, and the blocker was not replaced or
    // truncated by a retired writer.
    let entries = std::fs::read_dir(profile.root.join(".arcana"))
        .expect("read isolated profile")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(
        !entries
            .iter()
            .any(|name| name.ends_with(".log") || name.contains("arcana-events")),
        "an unpreparable sink must not leave a partial readable or structured stream: {entries:?}"
    );
    assert!(
        std::fs::metadata(&sink).is_ok_and(|metadata| metadata.is_file()),
        "the log-directory blocker must remain a file after the run"
    );
    assert_eq!(
        std::fs::read_to_string(&sink).expect("read the blocker back"),
        "not a directory",
        "the run must not overwrite the path it could not use"
    );

    // (iii) The degradation is reported, not silent. `--quiet` keeps the
    // console filter at its quiet level, so the CLI reports loss of the sink
    // through its own visible path rather than by pretending to have written.
    assert!(
        !stderr.contains("arcana-events"),
        "the CLI must not claim a structured stream it never had: {stderr}"
    );
}

#[test]
fn offline_fleet_cli_deadline_persists_ready_epic_and_returns_after_shutdown() {
    let profile = IsolatedProfile::new();
    profile.grant_workspace_egress();
    let title = format!("Offline Fleet deadline {}", Uuid::new_v4());
    let output = profile.run_fleet("deadline", &title, 1);
    assert!(
        !output.status.success(),
        "deadline scenario must return a bounded failure"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("did not finish within 1s"),
        "deadline error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let epic = profile
        .tasks()
        .list(None)
        .expect("read preserved task board")
        .into_iter()
        .find(|task| task.title == title)
        .expect("CLI-created epic survives deadline");
    assert_eq!(
        epic.status,
        TaskStatus::Ready,
        "deadline preserves the durable nonterminal epic"
    );
    let (logs, _) = profile.export();
    assert!(
        logs.contains("fleet_cli_run_timeout"),
        "CLI log export must read timeout telemetry"
    );
    assert!(
        logs.contains(&epic.id.to_string()),
        "timeout export must retain the epic identity"
    );
}

#[test]
fn offline_fleet_test_gate_rejects_unmarked_profiles() {
    let root = std::env::temp_dir().join(format!("arcana-cli-fleet-unmarked-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create unmarked profile");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let output = Command::new(env!("CARGO_BIN_EXE_arcanactl"))
        .env("USERPROFILE", &root)
        .env("HOME", &root)
        .env("ARCANA_OFFLINE_FLEET_SCENARIO", "success")
        .env("ARCANA_OFFLINE_FLEET_MARKER_ID", Uuid::new_v4().to_string())
        .env_remove("GOOGLE_API_KEY")
        .env_remove("GEMINI_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("DATABASE_URL")
        .env_remove("DIRECT_URL")
        .args(["--provider", "google", "--working-dir"])
        .arg(&workspace)
        .args([
            "fleet",
            "run",
            "--title",
            "must fail",
            "--description",
            "invalid gate",
            "--acceptance-criteria",
            "fail closed",
        ])
        .output()
        .expect("invoke gate validation subprocess");
    assert!(
        !output.status.success(),
        "an unmarked user profile must not activate the offline provider"
    );
    assert!(
        !root.join(".arcana").join("tasks.db").exists(),
        "invalid marker must fail before creating or modifying the task board"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn offline_fleet_test_gate_rejects_redirected_data_directory() {
    let profile = IsolatedProfile::new();
    let victim = IsolatedProfile::new();
    let data_dir = profile.root.join(".arcana");
    let victim_data = victim.root.join(".arcana");
    std::fs::rename(&data_dir, profile.root.join("original-arcana"))
        .expect("preserve original owned marker directory");
    std::fs::write(
        victim_data.join("offline-fleet-test.marker"),
        &profile.token,
    )
    .expect("write valid marker in separate disposable victim");
    #[cfg(windows)]
    {
        let created = Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&data_dir)
            .arg(&victim_data)
            .stdout(Stdio::null())
            .status()
            .expect("create owned test directory junction");
        assert!(created.success(), "create redirected data directory");
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&victim_data, &data_dir).expect("create owned test symlink");
    let output = profile.run_fleet("success", "redirected profile must fail", 1);
    assert!(
        !output.status.success(),
        "redirected profile must be refused"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("redirected marker path"));
    assert!(
        !victim_data.join("tasks.db").exists(),
        "external data must stay untouched"
    );
    #[cfg(windows)]
    std::fs::remove_dir(&data_dir).expect("remove owned test junction without its target");
    #[cfg(unix)]
    std::fs::remove_file(&data_dir).expect("remove owned test symlink without its target");
}
