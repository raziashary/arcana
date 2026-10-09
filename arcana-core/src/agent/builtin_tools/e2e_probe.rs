// Fleet smoke capability admission. Registration/schema remain available;
// execution requires proved confinement, aggregate quotas and tree cleanup.
// ADR0339/0344/0349/0350 forbid an unrestricted shell fallback.

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

/// Validate loopback syntax without DNS or an HTTP request. Capability
/// admission must not contact an unrelated host while refusing a launch.
fn validate_probe_url(raw: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(raw).map_err(|_| "'probe_url' is not a valid URL".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("'probe_url' must use http or https".into());
    }
    let host = url.host_str().unwrap_or_default();
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if !loopback {
        return Err(
            "'probe_url' is not loopback; use localhost or a literal loopback address".into(),
        );
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
        "End-to-end smoke probe for frontend/backend servers. Currently unavailable: \
         Arcana must first prove a platform-confined runner with explicit one-call \
         approval, exact loopback proxy, workspace disk quota and tree-wide resource \
         limits. Valid calls fail closed before process launch or HTTP access. \
         Do not bypass this denial with Bash; report the missing host capability."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "start_cmds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Requested server commands, retained for the future confined runner. \
                                    Currently unavailable: valid requests are denied before launch."
                },
                "probe_url": {
                    "type": "string",
                    "description": "Requested loopback probe URL (e.g. http://localhost:3000/). Currently validated only; no HTTP request is sent."
                },
                "expected_text": {
                    "type": "string",
                    "description": "Requested response fingerprint for the future confined runner. \
                                    No response is fetched while the capability is unavailable."
                },
                "boot_wait_s": {
                    "type": "integer",
                    "description": "Reserved boot interval for the future confined runner; currently no process is launched."
                },
                "timeout_s": {
                    "type": "integer",
                    "description": "Reserved total deadline for the future confined runner; currently requests are denied before launch."
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
        let _workspace = match resolve_probe_workspace(&ctx.working_dir) {
            Ok(workspace) => workspace,
            Err(error) => return ToolResult::error(error),
        };

        let _expected = match input["expected_text"].as_str() {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return ToolResult::error("'expected_text' is required — non-empty substring"),
        };

        ToolResult::sandbox_violation(
            "loopback_smoke",
            "e2e_probe",
            false,
            "The host has no proved confined smoke runner. Complete platform sandbox, \
             quota and process-tree cleanup verification before enabling this capability. \
             Do not retry through Bash or grant arbitrary host network access.",
        )
    }
}

fn resolve_probe_workspace(working_dir: &str) -> Result<String, String> {
    crate::platform::workspace::resolve_workspace_dir(Some(working_dir))
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| format!("End-to-end probe workspace is unavailable: {error}"))
}

#[cfg(test)]
mod workspace_tests {
    use super::*;

    #[tokio::test]
    async fn registered_smoke_refuses_unproved_capability_without_launch_or_http() {
        use crate::agent::{
            builtin_tools, permission, tool_executor::ToolExecutor, tool_registry::ToolRegistry,
            tool_trait::AgentIdentity, PAMode,
        };
        let home = tempfile::tempdir().expect("isolated Arcana profile");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated smoke workspace");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("owned probe listener");
        listener.set_nonblocking(true).unwrap();
        let run_id = crate::db::runs::RunsDb::open()
            .expect("isolated run store")
            .begin_worker_tick("supervisor", "smoke-denial-session")
            .expect("durable supervisor run");
        let mut registry = ToolRegistry::new();
        builtin_tools::register_for_mode(&mut registry, PAMode::FastDevelopment);
        let tool = registry
            .get("e2e_probe")
            .expect("shipped smoke registration");
        let mut executor = ToolExecutor::new(
            permission::PermissionMode::Autonomous,
            permission::new_pending_permissions(),
            "smoke-denial-session".into(),
            workspace.path().to_string_lossy().into_owned(),
            AgentIdentity::supervisor(),
        )
        .with_run_id(Some(run_id.clone()));
        let results = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            executor.execute_batch(vec![(
                tool,
                json!({
                    "start_cmds": ["echo PRIVATE_SMOKE_ARGUMENT > denied-marker.txt"],
                    "probe_url": format!("http://{}/", listener.local_addr().unwrap()),
                    "expected_text": "PRIVATE_SMOKE_SOURCE",
                    "api_key": "PRIVATE_SMOKE_KEY",
                    "prompt": "PRIVATE_SMOKE_PROMPT",
                    "boot_wait_s": 0,
                    "timeout_s": 1
                }),
                "smoke-denial-call".into(),
            )]),
        )
        .await
        .expect("unavailable capability is refused promptly")
        .expect("executor returns a failed result");
        assert_eq!(results.len(), 1);
        let result = &results[0].1;
        assert!(result.is_error);
        let envelope: Value = serde_json::from_str(&result.output).unwrap();
        assert_eq!(envelope["error_code"], "sandbox_violation");
        assert_eq!(envelope["category"], "sandbox");
        assert_eq!(envelope["error"]["capability"], "loopback_smoke");
        assert_eq!(envelope["error"]["policy"], "deny");
        assert_eq!(envelope["error"]["command"], "e2e_probe");
        assert_eq!(envelope["retryable"], false);
        assert_eq!(envelope["run_id"], run_id);
        assert_eq!(
            envelope["context"]["tool_call_id"],
            "smoke-denial-session-smoke-denial-call"
        );
        assert!(result.output.len() <= 8192);
        for canary in [
            "PRIVATE_SMOKE_ARGUMENT",
            "PRIVATE_SMOKE_SOURCE",
            "PRIVATE_SMOKE_KEY",
            "PRIVATE_SMOKE_PROMPT",
        ] {
            assert!(!result.output.contains(canary));
        }
        assert!(!workspace.path().join("denied-marker.txt").exists());
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        let owned_address = listener.local_addr().unwrap();
        drop(listener);
        drop(std::net::TcpListener::bind(owned_address).expect("owned port is free after cleanup"));
    }

    #[test]
    fn probe_requires_an_existing_absolute_workspace() {
        assert!(resolve_probe_workspace(".")
            .unwrap_err()
            .contains("must be absolute"));
        let workspace = tempfile::tempdir().expect("workspace");
        assert_eq!(
            resolve_probe_workspace(&workspace.path().to_string_lossy()).unwrap(),
            workspace.path().canonicalize().unwrap().to_string_lossy()
        );
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
    fn loopback_literals_and_localhost_are_valid_syntax() {
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
}

#[cfg(test)]
mod f19_tests {
    use super::*;

    // Future confined HTTP clients must validate redirects too. These tests
    // check URL admission only; the current unavailable capability sends no HTTP.
    #[test]
    fn metadata_shaped_redirect_urls_remain_invalid() {
        for target in [
            "http://169.254.169.254/latest/meta-data/",
            "http://8.8.8.8/",
            "https://example.com/",
        ] {
            assert!(
                validate_probe_url(target).is_err(),
                "{target} must fail loopback URL admission"
            );
        }
    }

    #[test]
    fn loopback_url_paths_remain_valid() {
        // Valid loopback path syntax remains accepted before capability denial.
        for target in [
            "http://localhost:3000/index.html",
            "http://127.0.0.1:8080/login",
        ] {
            validate_probe_url(target)
                .unwrap_or_else(|e| panic!("{target} must remain valid URL syntax: {e}"));
        }
    }
}

#[cfg(test)]
mod p4_51_probe_planner_matrix_tests {
    use super::*;
    use crate::agent::permission::{new_pending_permissions, PermissionMode};
    use crate::agent::tool_executor::ToolExecutor;
    use crate::agent::tool_registry::ToolRegistry;
    use crate::agent::tool_trait::{AgentIdentity, ToolResult};
    use serde_json::Value;

    const SESSION: &str = "p4-51-probe-planner-matrix";

    async fn invoke_registered(
        executor: &mut ToolExecutor,
        registry: &ToolRegistry,
        name: &str,
        input: Value,
        call_id: &str,
    ) -> ToolResult {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("FastDevelopment registry is missing {name}"));
        let results = executor
            .execute_batch(vec![(tool, input, call_id.to_string())])
            .await
            .expect("registered tool call returns a result");
        assert_eq!(results.len(), 1, "one result for {name}");
        let (returned_call_id, result) = results.into_iter().next().expect("one result");
        assert_eq!(
            returned_call_id, call_id,
            "executor preserves original call ID"
        );
        result
    }

    fn assert_normalized_failure(
        result: &ToolResult,
        tool_name: &str,
        call_id: &str,
        private_marker: &str,
    ) {
        assert!(result.is_error, "{tool_name} failure must remain an error");
        assert!(
            result.output.len() <= 8 * 1024,
            "bounded error for {tool_name}"
        );
        assert!(
            !result.output.contains(private_marker),
            "{tool_name} error reflected planted input {private_marker}"
        );
        let envelope: Value =
            serde_json::from_str(&result.output).expect("versioned JSON failure envelope");
        assert_eq!(envelope["schema_version"], 1, "{tool_name}");
        assert_eq!(envelope["ok"], false, "{tool_name} failure shape");
        assert_eq!(envelope["error"]["tool"], tool_name, "{tool_name}");
        assert_eq!(
            envelope["error"]["operation"],
            format!("tool.{tool_name}"),
            "{tool_name} operation"
        );
        assert_eq!(
            envelope["error"]["context"]["session_id"], SESSION,
            "{tool_name} session correlation"
        );
        assert_eq!(
            envelope["error"]["context"]["tool_call_id"],
            format!("{SESSION}-{call_id}"),
            "{tool_name} normalized call correlation"
        );
        for field in [
            "error_code",
            "category",
            "message",
            "cause",
            "suggested_action",
        ] {
            assert!(
                envelope["error"][field].as_str().is_some(),
                "{tool_name} has string {field}: {envelope}"
            );
        }
        for field in ["message", "cause", "suggested_action"] {
            assert!(
                envelope["error"][field].as_str().unwrap_or_default().len() <= 512,
                "{tool_name} {field} is bounded"
            );
        }
        assert!(envelope["error"]["retryable"].is_boolean(), "{tool_name}");
        assert!(
            envelope["error"]["retry_after_ms"].is_null()
                || envelope["error"]["retry_after_ms"].is_u64(),
            "{tool_name} retry delay is optional and numeric"
        );
        assert!(
            envelope["error"]["timestamp"].as_str().is_some(),
            "{tool_name} timestamp"
        );
    }

    #[tokio::test]
    async fn probe_and_planner_registered_matrix_uses_executor_and_real_reader() {
        use crate::agent::{builtin_tools, PAMode};

        let home = tempfile::tempdir().expect("isolated Arcana home and planner database");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated builtin workspace");
        let mut registry = ToolRegistry::new();
        builtin_tools::register_for_mode(&mut registry, PAMode::FastDevelopment);
        assert!(
            registry.get("planner").is_some(),
            "planner is a live registration"
        );
        assert!(
            registry.get("e2e_probe").is_some(),
            "probe is a live registration"
        );

        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            new_pending_permissions(),
            SESSION.into(),
            workspace.path().to_string_lossy().into_owned(),
            AgentIdentity::supervisor(),
        );

        let created = invoke_registered(
            &mut executor,
            &registry,
            "planner",
            json!({
                "yaml_dsl": "action: create\ntitle: P4-51 planner reader proof\ndescription: isolated executor matrix\nsession_id: p4-51-probe-planner-matrix\n"
            }),
            "planner-create",
        )
        .await;
        assert!(
            !created.is_error,
            "planner create output: {}",
            created.output
        );
        let created_json: Value =
            serde_json::from_str(&created.output).expect("planner create result is JSON");
        let plan_id = created_json["id"]
            .as_str()
            .expect("created plan exposes persisted ID")
            .to_string();

        let reread = invoke_registered(
            &mut executor,
            &registry,
            "planner",
            json!({"yaml_dsl": format!("action: getPlan\nplan_id: {plan_id}\n")}),
            "planner-reader",
        )
        .await;
        assert!(
            !reread.is_error,
            "planner getPlan output: {}",
            reread.output
        );
        let reread_json: Value =
            serde_json::from_str(&reread.output).expect("planner reader result is JSON");
        assert_eq!(reread_json["id"], plan_id);
        assert_eq!(reread_json["title"], "P4-51 planner reader proof");

        let planner_failure = invoke_registered(
            &mut executor,
            &registry,
            "planner",
            json!({"yaml_dsl": "private: PRIVATE_PLANNER_MATRIX_ARGUMENT\n"}),
            "planner-failure",
        )
        .await;
        assert_normalized_failure(
            &planner_failure,
            "planner",
            "planner-failure",
            "PRIVATE_PLANNER_MATRIX_ARGUMENT",
        );

        // Fail before URL validation, process creation, or HTTP so this probe
        // check stays deterministic and performs no external activity.
        let probe_failure = invoke_registered(
            &mut executor,
            &registry,
            "e2e_probe",
            json!({
                "start_cmds": [],
                "probe_url": "PRIVATE_PROBE_MATRIX_ARGUMENT",
                "expected_text": "unused"
            }),
            "probe-validation-failure",
        )
        .await;
        assert_normalized_failure(
            &probe_failure,
            "e2e_probe",
            "probe-validation-failure",
            "PRIVATE_PROBE_MATRIX_ARGUMENT",
        );
    }
}
