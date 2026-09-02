// analyze_diff — MCP tool for the QC Agent.
// Generates and analyzes the diff between two branches, returning structured
// risk score, classification, and review notes to guide the QC decision.

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::diff_analysis::DiffAnalyzer;
use crate::provider::traits::Provider;
use crate::vault::keychain::{provider_key, VaultManager};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct AnalyzeDiffTool;

#[async_trait]
impl Tool for AnalyzeDiffTool {
    // ⚠ `Network`, not `Read`. It uploads the **full diff of the
    // repository** to a third-party LLM — under BYOK, the user's own code on
    // the user's own key. It declared `is_read_only = true` and was therefore
    // never prompted in any mode.
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Network
    }

    fn name(&self) -> &str {
        "analyze_diff"
    }

    fn description(&self) -> &str {
        "Analyze a git diff between two branches and get a structured risk score, change \
        classification, and review notes to guide the QC review decision. Returns risk_level \
        ('low'|'medium'|'high'|'critical'), classification, summary, review_notes, and \
        suggested_checks."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "repo_path": {
                    "type": "string",
                    "description": "Absolute path to the git repository"
                },
                "base_branch": {
                    "type": "string",
                    "description": "Base branch to diff from (e.g. 'main')"
                },
                "head_branch": {
                    "type": "string",
                    "description": "Head branch to diff to (e.g. 'feature/my-pr')"
                },
                "task_id": {
                    "type": "integer",
                    "description": "Optional. When provided, the analysis JSON is persisted to tasks.diff_analysis so the AutoDev panel can render the risk card alongside the task. Pass the QC task you are reviewing."
                }
            },
            "required": ["repo_path", "base_branch", "head_branch"]
        })
    }


    // `_ctx` until P3-14: the workspace was never consulted, so any repository
    // path the model named was diffed and uploaded.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let repo_path_raw = match input["repo_path"].as_str() {
            Some(p) if !p.is_empty() => p,
            _ => {
                return ToolResult::error("'repo_path' is required and must be a non-empty string")
            }
        };

        // ⚠ P3-14, and the second of the two tools that ACT on the path. This
        // takes a repository path from the model, diffs it, and **uploads the
        // result to a third-party LLM** — so an unjailed path here exfiltrates
        // any repository on the machine, not merely reads it. It also declared
        // `is_read_only = true` and was therefore never prompted (`P3-56`).
        let repo_path_buf =
            match super::path_jail::validate_workspace_path(
                std::path::Path::new(repo_path_raw),
                &ctx.working_dir,
            ) {
                Ok(canonical) => canonical,
                Err(e) => return ToolResult::error(e),
            };
        let repo_path = repo_path_buf.to_string_lossy().to_string();
        let repo_path = repo_path.as_str();
        let base_branch = match input["base_branch"].as_str() {
            Some(b) if !b.is_empty() => b,
            _ => {
                return ToolResult::error(
                    "'base_branch' is required and must be a non-empty string",
                )
            }
        };
        let head_branch = match input["head_branch"].as_str() {
            Some(h) if !h.is_empty() => h,
            _ => {
                return ToolResult::error(
                    "'head_branch' is required and must be a non-empty string",
                )
            }
        };

        // Build analyzer using active provider from vault
        let analyzer = match build_analyzer_from_vault() {
            Ok(a) => a,
            Err(e) => return ToolResult::error(format!("Failed to initialize analyzer: {e}")),
        };

        // Generate branch diff
        let patch = match crate::git::diff::branch_diff(repo_path, base_branch, head_branch) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Failed to generate diff: {e}")),
        };

        match analyzer.analyze(&patch).await {
            Ok(result) => {
                let summary = format!(
                    "Risk: {} ({:.2}) | Classification: {:?} | Files: {} | +{} -{}",
                    result.risk_level,
                    result.risk_score,
                    result.classification,
                    result.file_count,
                    result.lines_added,
                    result.lines_deleted,
                );
                let mut json_value = match serde_json::to_value(&result) {
                    Ok(v) => v,
                    Err(e) => return ToolResult::error(format!("Failed to serialize result: {e}")),
                };
                // Doc 09 T3.4 — interface_drift field.
                //
                // When the diff touches INTERFACE.md AND/OR any frontend
                // consumer of an API, we surface a structured drift signal
                // the QC reviewer can hard-fail on. Heuristic — scan the
                // patch for the file change set:
                //   • `INTERFACE.md` touched → backend changed the contract.
                //   • A file matching apiClient.*/use*Events*-style consumer
                //     was touched → frontend changed too.
                // When BOTH or NEITHER side appears we emit an "unknown"
                // drift; the QC reviewer can re-run the api_contract check
                // for a definitive verdict.
                let drift = detect_interface_drift(&patch);
                if let Some(obj) = json_value.as_object_mut() {
                    obj.insert(
                        "interface_drift".into(),
                        serde_json::to_value(&drift).unwrap_or(Value::Null),
                    );
                }
                let json = match serde_json::to_string(&json_value) {
                    Ok(j) => j,
                    Err(e) => return ToolResult::error(format!("Failed to serialize result: {e}")),
                };
                // If the caller passed a task_id, persist the analysis on that
                // task so the AutoDev panel's DiffAnalysisCard can render it
                // without re-running the LLM. Failure to persist is non-fatal —
                // the tool result still includes the JSON for the agent to use.
                if let Some(task_id) = input.get("task_id").and_then(|v| v.as_i64()) {
                    if let Ok(db) = crate::db::tasks::TasksDb::open() {
                        if let Err(e) = db.set_task_diff_analysis(task_id, &json) {
                            tracing::warn!("[analyze_diff] persist to task #{task_id} failed: {e}");
                        } else {
                            // Push the kanban a fresh task:changed so any open
                            // DiffAnalysisCard refreshes immediately.
                            crate::events::emit_task_changed(crate::events::TaskChangedPayload {
                                task_id: Some(task_id),
                                kind: "diff_analysis".into(),
                                status: None,
                                assigned_worker: None,
                            });
                        }
                    }
                }
                ToolResult::success_with_hint(json, summary)
            }
            Err(e) => ToolResult::error(format!("Diff analysis failed: {e}")),
        }
    }
}

/// Doc 09 T3.4 — interface drift detector.
///
/// Pure regex scan over the unified-diff `+++ b/path` markers. Returns a
/// JSON-friendly shape the QC reviewer can match on:
///   • status         : "contract_only" | "consumer_only" | "both" |
///                      "neither" | "unknown"
///   • touched_spec   : true when INTERFACE.md is in the diff
///   • touched_consumer : true when any FE consumer file is in the diff
///   • files          : the actual paths matched on each side
#[derive(Debug, serde::Serialize)]
struct InterfaceDrift {
    status: &'static str,
    touched_spec: bool,
    touched_consumer: bool,
    spec_files: Vec<String>,
    consumer_files: Vec<String>,
}

fn detect_interface_drift(patch: &str) -> InterfaceDrift {
    let mut spec_files: Vec<String> = Vec::new();
    let mut consumer_files: Vec<String> = Vec::new();

    // Match `+++ b/<path>` (new-side) and `--- a/<path>` (old-side) so a
    // pure-deletion of an interface still surfaces.
    let re = regex::Regex::new(r"(?m)^(?:\+\+\+ b/|--- a/)(.+)$").unwrap();
    for cap in re.captures_iter(patch) {
        let path = cap[1].trim().to_string();
        if path.is_empty() || path == "/dev/null" {
            continue;
        }

        let pl = path.to_lowercase();
        if pl.ends_with("interface.md") || pl.contains("/interface.md") {
            if !spec_files.contains(&path) {
                spec_files.push(path.clone());
            }
        }

        // Frontend API consumer heuristic — file is JS/TS AND its name
        // resembles a known consumer pattern.
        let is_js_ts = pl.ends_with(".ts")
            || pl.ends_with(".tsx")
            || pl.ends_with(".js")
            || pl.ends_with(".jsx");
        let consumer_hint = pl.contains("apiclient")
            || pl.contains("api_client")
            || pl.contains("use")
                && (pl.contains("query") || pl.contains("fetch") || pl.contains("event"))
            || pl.contains("/hooks/")
            || pl.contains("/api/");
        if is_js_ts && consumer_hint {
            if !consumer_files.contains(&path) {
                consumer_files.push(path);
            }
        }
    }

    let touched_spec = !spec_files.is_empty();
    let touched_consumer = !consumer_files.is_empty();
    let status = match (touched_spec, touched_consumer) {
        (true, true) => "both",
        (true, false) => "contract_only",
        (false, true) => "consumer_only",
        (false, false) => "neither",
    };

    InterfaceDrift {
        status,
        touched_spec,
        touched_consumer,
        spec_files,
        consumer_files,
    }
}

fn build_analyzer_from_vault() -> crate::errors::Result<DiffAnalyzer> {
    use crate::errors::AutoDevError;

    // ── P2-31 · this tool used to bypass model selection entirely ────────────
    //
    // It read `AUTODEV_ACTIVE_PROVIDER` and `AUTODEV_ACTIVE_MODEL` from the
    // process environment — two variables **nothing in the tree ever sets** —
    // and fell back to `"claude-sonnet-4-6"`, an id that appears in no provider
    // catalogue. So the fallback was not a degraded path, it was the ONLY path,
    // and it named a model that does not exist. Every call through this tool
    // failed at the provider with a 404 that looked like an outage.
    //
    // It now reads the same persisted selection the rest of the app uses
    // (`P2-26`), so the user's choice in Settings governs this tool too.
    let provider_id = crate::db::app_settings::get("active_provider")
        .ok()
        .flatten()
        .unwrap_or_else(|| "anthropic".to_string());

    let model = crate::db::app_settings::get("active_model")
        .ok()
        .flatten()
        .filter(|m| crate::provider::catalogue::entry(m).is_some())
        .or_else(|| {
            // No saved choice, or one that has since been retired. Fall back to
            // the first catalogued model this provider actually serves, rather
            // than to a literal — a hardcoded id is exactly what rotted here.
            crate::provider::catalogue::CATALOGUE
                .iter()
                .find(|e| e.provider == provider_id)
                .map(|e| e.id.to_string())
        })
        .ok_or_else(|| {
            AutoDevError::Agent(format!(
                "No model available for provider '{provider_id}'. Choose a provider and \
                 model in Settings."
            ))
        })?;

    let api_key = VaultManager::get(&provider_key(&provider_id))?.ok_or_else(|| {
        AutoDevError::Agent(format!("No API key for provider '{provider_id}'"))
    })?;

    // Built through the shared factory, which applies the daily ceiling. This
    // tool holds no AppState — tools receive a ToolContext — so it reaches the
    // cap through the process-global handle. Without it this would be the one
    // path a call could take without meeting the ceiling (P2-21).
    let provider = crate::provider::factory::build(&provider_id, api_key)?;

    Ok(DiffAnalyzer::new(provider, model))
}
