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

/// How much of each child's output to keep for evidence, per stream.
///
/// The **tail**, not the head: a dev server prints a banner on success and
/// the reason for death last.
const CHILD_LOG_TAIL_BYTES: usize = 4_000;

/// Confine `probe_url` to a server on this machine.
///
/// 🔴 `P3-68`. `probe_url` was fetched with no restriction whatsoever and up
/// to 400 characters of the response body were returned to the model. That is
/// a server-side request forgery **with readback**: the model names
/// `http://169.254.169.254/latest/meta-data/iam/security-credentials/`, or any
/// internal service reachable from the user's machine, and the contents come
/// back into its context. Under BYOK the user's own credentials are what it
/// would be reading.
///
/// ⚠ The check resolves the host and requires **every** resolved address to be
/// loopback. A string check on "localhost" would not do: `127.0.0.1.nip.io`
/// and any attacker-controlled DNS name resolving to the metadata address both
/// read as ordinary hostnames.
///
/// This costs the tool nothing it was for. Its entire purpose is to probe a
/// server it spawned itself, seconds earlier, on this machine.
fn validate_probe_url(raw: &str) -> Result<(), String> {
    let url =
        reqwest::Url::parse(raw).map_err(|e| format!("'probe_url' is not a valid URL: {e}"))?;

    match url.scheme() {
        "http" | "https" => {}
        other => {
            return Err(format!(
                "'probe_url' scheme '{other}' is not allowed — e2e_probe speaks \
                 http/https to a server it started on this machine."
            ))
        }
    }

    let host = url
        .host_str()
        .ok_or_else(|| "'probe_url' has no host".to_string())?;

    // Resolution, not spelling. `to_socket_addrs` performs the same lookup the
    // HTTP client will.
    use std::net::ToSocketAddrs;
    let port = url.port_or_known_default().unwrap_or(80);
    let addrs: Vec<std::net::SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("'probe_url' host '{host}' did not resolve: {e}"))?
        .collect();

    if addrs.is_empty() {
        return Err(format!(
            "'probe_url' host '{host}' resolved to no addresses"
        ));
    }

    // EVERY address must be loopback. One public address among several is
    // still a reachable public address.
    if let Some(bad) = addrs.iter().find(|a| !a.ip().is_loopback()) {
        return Err(format!(
            "'probe_url' host '{host}' resolves to {}, which is not loopback. \
             e2e_probe only probes servers on this machine — it returns the \
             response body to the model, so an off-machine URL would exfiltrate \
             whatever that host serves (cloud metadata endpoints included).",
            bad.ip()
        ));
    }

    Ok(())
}

pub struct E2eProbeTool;

#[async_trait]
impl Tool for E2eProbeTool {
    // ⚠ P3-21. This declared `Network`, under a comment saying "the
    // higher-consequence effect wins" — and then picked the lower one.
    // Running arbitrary shell strictly SUBSUMES making a request: you can
    // curl from a shell, not shell from a curl.
    //
    // `Execute` and `Network` prompt identically in every mode, so this is
    // not about whether the user is asked. It is about **what they are
    // asked**. The consent sentences differ:
    //
    //     Network  → "send data from your machine to an external service"
    //     Execute  → "run a command on your machine"
    //
    // A user approving this tool was consenting to a network call while it
    // spawned `sh -c <model-supplied string>`. That is consent for the wrong
    // act. And after `P3-68` confined `probe_url` to loopback, the tool no
    // longer sends anything to an external service at all, so `Network` had
    // become false as well as misleading.
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Execute
    }

    fn name(&self) -> &str {
        "e2e_probe"
    }

    fn description(&self) -> &str {
        "End-to-end smoke probe. Spawns one or more long-running commands \
         (backend, frontend dev server, etc.) in the background, waits the \
         configured boot interval, then issues an HTTP GET against \
         `probe_url` and asserts that `expected_text` appears in the body. \
         \
         `probe_url` must point at THIS machine: its host has to resolve to a \
         loopback address (localhost / 127.0.0.1 / ::1). The response body is \
         returned to you, so an off-machine URL would read that host's content \
         into this conversation. \
         \
         Each start command's stdout/stderr tail comes back in \
         `start_cmd_output` — read it when a probe fails; it is where the \
         boot error is. \
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

        // P3-68: confine the probe to this machine before anything is spawned.
        if let Err(msg) = validate_probe_url(&probe_url) {
            return ToolResult::error(msg);
        }

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

/// A bounded tail of one child's combined output.
///
/// Bounded because a dev server can emit megabytes in the boot window, and
/// unbounded capture would put all of it into the model's context.
#[derive(Clone)]
struct ChildLog(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl ChildLog {
    fn new() -> Self {
        Self(std::sync::Arc::new(std::sync::Mutex::new(Vec::new())))
    }

    /// Continuously drain both streams into the tail buffer.
    ///
    /// ⚠ The tasks are spawned and never joined **on purpose**: they end when
    /// the child's pipes close, which `kill_on_drop` guarantees at teardown.
    /// Awaiting them would reintroduce the block this exists to avoid.
    fn drain(
        &self,
        stdout: Option<tokio::process::ChildStdout>,
        stderr: Option<tokio::process::ChildStderr>,
    ) {
        use tokio::io::AsyncReadExt;
        for stream in [
            stdout.map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Send + Unpin>),
            stderr.map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Send + Unpin>),
        ]
        .into_iter()
        .flatten()
        {
            let sink = self.0.clone();
            tokio::spawn(async move {
                let mut stream = stream;
                let mut buf = [0u8; 4096];
                loop {
                    match stream.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            // Poison-tolerant: a panicked reader must not
                            // silence the other stream (`state::lock`).
                            let mut guard = match sink.lock() {
                                Ok(g) => g,
                                Err(p) => p.into_inner(),
                            };
                            guard.extend_from_slice(&buf[..n]);
                            // Trim eagerly so a chatty server cannot grow this
                            // without bound between reads.
                            if guard.len() > CHILD_LOG_TAIL_BYTES * 2 {
                                let cut = guard.len() - CHILD_LOG_TAIL_BYTES;
                                guard.drain(..cut);
                            }
                        }
                    }
                }
            });
        }
    }

    /// The captured tail, truncated on a character boundary.
    fn tail(&self) -> Option<String> {
        let guard = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if guard.is_empty() {
            return None;
        }
        let text = String::from_utf8_lossy(&guard).to_string();
        Some(
            crate::security::truncate::truncate_bytes_from_end(&text, CHILD_LOG_TAIL_BYTES)
                .to_string(),
        )
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
    /// The tail of each start command's stdout+stderr — the evidence for why
    /// a probe failed. `P3-21`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    start_cmd_output: Vec<StartCmdOutput>,
}

#[derive(Debug, serde::Serialize)]
struct StartCmdOutput {
    command: String,
    output_tail: String,
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
    // 🔴 P3-21. Both streams were `Stdio::null()`, so when a probe failed the
    // report said FAIL and could not say why — the one thing a smoke test
    // exists to tell you. "dev server crashed on boot (EADDR / missing deps)"
    // is listed at the top of this file as a case it catches; it caught the
    // symptom and threw away the cause.
    //
    // ⚠ Piping without draining deadlocks: a dev server fills the 64 KB pipe
    // buffer and blocks forever, which would hang the probe rather than fail
    // it. Each stream therefore gets a reader task that drains continuously
    // into a bounded tail.
    let logs: Vec<ChildLog> = (0..start_cmds.len()).map(|_| ChildLog::new()).collect();

    for (i, cmd_str) in start_cmds.iter().enumerate() {
        let mut builder = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", cmd_str]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", cmd_str]);
            c
        };
        // 🔴 G-8 (post-phase gap hunt). `bash` strips credentials from the
        // child's environment — "a single missed variable here is a readable
        // secret, not a degraded feature", as its own comment puts it — and
        // this tool, which spawns the SAME `sh -c` / `cmd /C` with commands the
        // model chose, did not. Its children inherited every provider key in
        // the process.
        //
        // ⚠ `bash`'s comment records that its first version landed in the
        // Windows branch only, "which is its own small lesson about two-branch
        // fixes". This is the same lesson one tool over: the fix was applied
        // where the defect was found rather than everywhere it existed.
        for (key, _) in std::env::vars() {
            if crate::security::env_scrub::looks_like_a_credential(&key) {
                builder.env_remove(&key);
            }
        }

        let child = builder
            .current_dir(&working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .spawn();

        match child {
            Ok(mut c) => {
                debug!("[e2e_probe] spawned '{cmd_str}' pid={:?}", c.id());
                logs[i].drain(c.stdout.take(), c.stderr.take());
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

    // P3-21: gather the evidence AFTER teardown, so a server that printed its
    // failure on the way down is still represented.
    let start_cmd_output: Vec<StartCmdOutput> = start_cmds
        .iter()
        .zip(logs.iter())
        .filter_map(|(cmd, log)| {
            log.tail().map(|output_tail| StartCmdOutput {
                command: cmd.clone(),
                output_tail,
            })
        })
        .collect();

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
                start_cmd_output,
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
            start_cmd_output,
        },
    })
}

async fn http_probe(url: &str, _expected: &str) -> Result<(u16, String), String> {
    // 🔴 F-19 (post-phase claims audit). `validate_probe_url` confined the
    // FIRST request and nothing else: no redirect policy was set, so reqwest
    // follows up to ten hops by default. Any server the model can make respond
    // — including the one it just started itself — returning
    // `302 Location: http://169.254.169.254/…` defeated the confinement
    // entirely, and the body still came back to the model. The check guarded
    // the front door and left the hallway open.
    //
    // Every hop is now re-validated by the same function as the first.
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(PROBE_DEFAULT_HTTP_TIMEOUT))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 {
                return attempt.stop();
            }
            match validate_probe_url(attempt.url().as_str()) {
                Ok(()) => attempt.follow(),
                Err(_) => attempt.stop(),
            }
        }))
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
            start_cmd_output: vec![StartCmdOutput {
                command: "npm run dev".into(),
                output_tail: "vite ready in 312 ms".into(),
            }],
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"success\":true"));
        assert!(s.contains("\"started_processes\":2"));
        // P3-21: the evidence must survive serialisation, or it never reaches
        // the model that has to act on it.
        assert!(s.contains("vite ready in 312 ms"), "{s}");
    }
}

#[cfg(test)]
mod p3_21_68_tests {
    use super::*;

    // ── P3-68 · the SSRF ────────────────────────────────────────────────

    #[test]
    fn the_cloud_metadata_endpoint_is_refused() {
        // 🔴 THE DEFECT, concretely. `probe_url` was fetched unrestricted and
        // 400 characters of the body were handed back to the model. This URL
        // returns IAM credentials on any EC2 host.
        let err = validate_probe_url("http://169.254.169.254/latest/meta-data/iam/")
            .expect_err("the metadata endpoint must be refused");
        assert!(err.contains("not loopback"), "got: {err}");
    }

    #[test]
    fn a_public_address_is_refused() {
        // A literal, so this test does no DNS and cannot fail offline.
        let err = validate_probe_url("http://8.8.8.8/").expect_err("public IP must be refused");
        assert!(err.contains("not loopback"), "got: {err}");
    }

    #[test]
    fn a_non_http_scheme_is_refused() {
        for url in [
            "file:///etc/passwd",
            "gopher://127.0.0.1/",
            "ftp://127.0.0.1/",
        ] {
            assert!(
                validate_probe_url(url).is_err(),
                "{url} must be refused — only http/https reach a dev server"
            );
        }
    }

    #[test]
    fn the_probe_still_works_for_what_it_is_for() {
        // The no-fire half. If this over-tightens, the tool is dead: every
        // legitimate use is a dev server on loopback.
        for url in [
            "http://localhost:3000/",
            "http://127.0.0.1:8080/health",
            "https://localhost:5173/",
        ] {
            validate_probe_url(url).unwrap_or_else(|e| panic!("{url} must be allowed: {e}"));
        }
    }

    #[test]
    fn a_malformed_url_is_refused_rather_than_guessed_at() {
        assert!(validate_probe_url("not a url").is_err());
        assert!(validate_probe_url("").is_err());
    }

    // ── P3-21 · the evidence ────────────────────────────────────────────

    #[tokio::test]
    async fn a_child_s_output_is_captured_as_evidence() {
        // 🔴 Both streams were `Stdio::null()`, so a failed probe reported
        // FAIL with no way to say why — while this file's own header claims
        // it catches "dev server crashed on boot (EADDR / missing deps)".
        let log = ChildLog::new();
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.args(["/C", "echo BOOT_FAILURE_REASON"]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", "echo BOOT_FAILURE_REASON"]);
            c
        };
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn");

        log.drain(child.stdout.take(), child.stderr.take());
        let _ = child.wait().await;
        // The drain tasks are independent of `wait`; give them a moment to
        // observe EOF. Bounded, so a hang here fails rather than blocks.
        for _ in 0..50 {
            if log.tail().is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        let tail = log.tail().expect("the child's output must be captured");
        assert!(
            tail.contains("BOOT_FAILURE_REASON"),
            "the evidence must contain what the child printed, got: {tail:?}"
        );
    }

    #[test]
    fn an_unstarted_child_contributes_no_noise() {
        // The no-fire half: a silent child must not produce an empty evidence
        // block that reads as "we looked and there was nothing".
        assert!(ChildLog::new().tail().is_none());
    }

    #[tokio::test]
    async fn the_captured_tail_is_bounded() {
        // A dev server can emit megabytes in the boot window; unbounded
        // capture would put all of it into the model's context.
        let log = ChildLog::new();
        {
            let mut g = log.0.lock().unwrap();
            g.extend(std::iter::repeat(b'x').take(CHILD_LOG_TAIL_BYTES * 5));
        }
        let tail = log.tail().expect("non-empty");
        assert!(
            tail.len() <= CHILD_LOG_TAIL_BYTES,
            "tail is {} bytes, cap is {}",
            tail.len(),
            CHILD_LOG_TAIL_BYTES
        );
    }
}

#[cfg(test)]
mod f19_tests {
    use super::*;

    /// A redirect hop is validated exactly as the first request is.
    ///
    /// 🔴 F-19 (post-phase claims audit). `validate_probe_url` guarded only the
    /// FIRST request; no redirect policy was set, so reqwest followed up to ten
    /// hops. Any server the model could make respond — including the one it
    /// just started — could answer `302 Location: http://169.254.169.254/…`
    /// and the body still came back to it.
    #[test]
    fn the_metadata_endpoint_is_refused_as_a_redirect_target_too() {
        // The policy re-uses this exact function, so proving the function
        // refuses the target proves the hop is refused.
        for target in [
            "http://169.254.169.254/latest/meta-data/",
            "http://8.8.8.8/",
            "https://example.com/",
        ] {
            assert!(
                validate_probe_url(target).is_err(),
                "{target} would be followed as a redirect and its body returned"
            );
        }
    }

    #[test]
    fn a_loopback_redirect_is_still_allowed() {
        // The no-fire half: a dev server redirecting `/` to `/index.html` or to
        // a login page on its own port must keep working.
        for target in [
            "http://localhost:3000/index.html",
            "http://127.0.0.1:8080/login",
        ] {
            validate_probe_url(target)
                .unwrap_or_else(|e| panic!("{target} must still be followed: {e}"));
        }
    }
}
