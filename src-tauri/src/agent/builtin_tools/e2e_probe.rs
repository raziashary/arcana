// Doc 09 T4.2 — e2e_probe.
//
// Fleet smoke test: takes a list of start commands (backend, frontend),
// boots them in the background, waits a bit, curls a probe URL, looks for
// a fingerprint string in the rendered page, then tears everything down.
// Returns a structured pass/fail summary the QC dispatcher / smoke recipe
// can consume.
//
// Designed to be the CHEAPEST possible "does it actually run?" check.
// It is not a Playwright replacement — it does no headless browser, no
// JS evaluation, just the same HTML the dev-server returns to `curl`.
// For React apps where the fingerprint text is server-rendered (Vite
// dev does this for the bootstrap `<title>` and the bundle URL), this is
// enough to catch:
//   • dev server crashed on boot (EADDR / missing deps)
//   • index.html in the wrong location (Vite 404 on /)
//   • build artefact missing
//   • backend health endpoint not responding
//
// Composability with the runner.rs path-jail: the probe walks
// `working_dir` to launch its children, so a misconfigured workspace
// can't reach the wrong tree.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::time::timeout;
use tracing::{debug, warn};

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

const PROBE_DEFAULT_TIMEOUT_S: u64 = 30;
const PROBE_DEFAULT_BOOT_WAIT_S: u64 = 8;
const PROBE_DEFAULT_HTTP_TIMEOUT: u64 = 10;

pub struct E2eProbeTool;

#[async_trait]
impl Tool for E2eProbeTool {
    fn name(&self) -> &str {
        "e2e_probe"
    }

    fn description(&self) -> &str {
        "End-to-end smoke probe. Spawns one or more long-running commands \
         (backend, frontend dev server, etc.) in the background, waits the \
         configured boot interval, then issues an HTTP GET against \
         `probe_url` and asserts that `expected_text` appears in the body. \
         Always tears down its children before returning. Use after build \
         validation passes to confirm the artefact actually serves the \
         expected page — the cheapest way to catch boot-time failures the \
         build itself doesn't surface."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "start_cmds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Commands to launch in the background. Each is a bash one-liner. \
                                    Run order is preserved; each spawns before the next."
                },
                "probe_url": {
                    "type": "string",
                    "description": "URL to GET once the boot wait elapses (e.g. http://localhost:3000/)."
                },
                "expected_text": {
                    "type": "string",
                    "description": "Substring that must appear in the response body for the probe to pass. \
                                    For React apps this is usually the page <title> or a bootstrap element."
                },
                "boot_wait_s": {
                    "type": "integer",
                    "description": "Seconds to wait between launching the last process and the HTTP probe. \
                                    Default 8s — dev servers usually need a few seconds to bind a port."
                },
                "timeout_s": {
                    "type": "integer",
                    "description": "Total timeout for the entire probe (boot + HTTP). Default 30s."
                }
            },
            "required": ["start_cmds", "probe_url", "expected_text"]
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }
    fn requires_permission(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let start_cmds: Vec<String> = match input.get("start_cmds") {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            _ => {
                return ToolResult::error("'start_cmds' is required and must be a non-empty array")
            }
        };
        if start_cmds.is_empty() {
            return ToolResult::error("'start_cmds' must contain at least one command");
        }

        let probe_url = match input["probe_url"].as_str() {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return ToolResult::error("'probe_url' is required (e.g. http://localhost:3000/)"),
        };

        let expected = match input["expected_text"].as_str() {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return ToolResult::error("'expected_text' is required — non-empty substring"),
        };

        let boot_wait = Duration::from_secs(
            input["boot_wait_s"]
                .as_u64()
                .unwrap_or(PROBE_DEFAULT_BOOT_WAIT_S),
        );
        let total_timeout = Duration::from_secs(
            input["timeout_s"]
                .as_u64()
                .unwrap_or(PROBE_DEFAULT_TIMEOUT_S),
        );

        // Run the whole probe under a single timeout. The teardown runs even
        // on timeout because the spawned children are owned by this future.
        let probe = run_probe(
            ctx.working_dir.clone(),
            start_cmds,
            probe_url.clone(),
            expected.clone(),
            boot_wait,
        );

        let result = timeout(total_timeout, probe).await;

        match result {
            Ok(Ok(report)) => {
                let json_payload =
                    serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into());
                if report.success {
                    ToolResult::success(format!(
                        "e2e_probe: PASS — fingerprint '{expected}' found at {probe_url}\n\n{json_payload}"
                    ))
                } else {
                    ToolResult::error(format!(
                        "e2e_probe: FAIL — {}\n\n{json_payload}",
                        report.failure_reason.as_deref().unwrap_or("(no reason)"),
                    ))
                }
            }
            Ok(Err(e)) => ToolResult::error(format!("e2e_probe error: {e}")),
            Err(_) => ToolResult::error(format!(
                "e2e_probe timed out after {}s — probe URL '{probe_url}' did not respond \
                 with '{expected}' before the deadline. Children were killed.",
                total_timeout.as_secs(),
            )),
        }
    }
}

#[derive(Debug, serde::Serialize)]
struct ProbeReport {
    success: bool,
    probe_url: String,
    expected_text: String,
    status_code: Option<u16>,
    response_body_excerpt: Option<String>,
    failure_reason: Option<String>,
    started_processes: usize,
}

async fn run_probe(
    working_dir: String,
    start_cmds: Vec<String>,
    probe_url: String,
    expected: String,
    boot_wait: Duration,
) -> Result<ProbeReport, String> {
    // Spawn each start cmd. On panic / cancel, the Drop impl on the Vec
    // kills the children — but tokio Child needs an explicit `kill().await`
    // to clean up reliably; we do that in the teardown branches.
    let mut children: Vec<Child> = Vec::with_capacity(start_cmds.len());
    for cmd_str in &start_cmds {
        let child = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", cmd_str])
                .current_dir(&working_dir)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .kill_on_drop(true)
                .spawn()
        } else {
            Command::new("sh")
                .args(["-c", cmd_str])
                .current_dir(&working_dir)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .kill_on_drop(true)
                .spawn()
        };

        match child {
            Ok(c) => {
                debug!("[e2e_probe] spawned '{cmd_str}' pid={:?}", c.id());
                children.push(c);
            }
            Err(e) => {
                // Best-effort: kill anything we've already started so this
                // failed probe doesn't leak processes.
                for c in children.iter_mut() {
                    let _ = c.kill().await;
                }
                return Err(format!("Failed to spawn '{cmd_str}': {e}"));
            }
        }
    }

    let started_processes = children.len();

    // Wait for the children to bind their ports.
    tokio::time::sleep(boot_wait).await;

    // HTTP probe. reqwest is already in the dependency tree (see sidecar/client.rs).
    let probe_result = http_probe(&probe_url, &expected).await;

    // Always tear down before returning. kill_on_drop fires too, but
    // awaiting kill ensures the OS reclaims the socket before the report
    // claims "PASS" — important when a CI job chains probes.
    for c in children.iter_mut() {
        let _ = c.kill().await;
    }

    Ok(match probe_result {
        Ok((status, body)) => {
            let found = body.contains(&expected);
            ProbeReport {
                success: found && (200..400).contains(&status),
                probe_url: probe_url.clone(),
                expected_text: expected.clone(),
                status_code: Some(status),
                response_body_excerpt: Some(body.chars().take(400).collect()),
                failure_reason: if found && (200..400).contains(&status) {
                    None
                } else if !(200..400).contains(&status) {
                    Some(format!("HTTP status {status} (expected 2xx/3xx)"))
                } else {
                    Some(format!("Body did not contain fingerprint '{expected}'"))
                },
                started_processes,
            }
        }
        Err(e) => ProbeReport {
            success: false,
            probe_url: probe_url.clone(),
            expected_text: expected.clone(),
            status_code: None,
            response_body_excerpt: None,
            failure_reason: Some(format!("HTTP probe error: {e}")),
            started_processes,
        },
    })
}

async fn http_probe(url: &str, _expected: &str) -> Result<(u16, String), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(PROBE_DEFAULT_HTTP_TIMEOUT))
        .build()
        .map_err(|e| format!("client build: {e}"))?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("GET {url}: {e}"))?;
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    if body.is_empty() {
        warn!("[e2e_probe] {url} returned empty body (status={status})");
    }
    Ok((status, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_report_serializes() {
        let r = ProbeReport {
            success: true,
            probe_url: "http://localhost:3000".into(),
            expected_text: "Hello".into(),
            status_code: Some(200),
            response_body_excerpt: Some("<html>Hello</html>".into()),
            failure_reason: None,
            started_processes: 2,
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"success\":true"));
        assert!(s.contains("\"started_processes\":2"));
    }
}
