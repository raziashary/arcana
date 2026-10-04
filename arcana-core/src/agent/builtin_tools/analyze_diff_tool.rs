// analyze_diff — MCP tool for the QC Agent.
// Generates and analyzes the diff between two branches, returning structured
// risk score, classification, and review notes to guide the QC decision.

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::diff_analysis::DiffAnalyzer;
use crate::vault::keychain::{provider_key, VaultManager};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

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
                    "description": "Optional. When provided, the analysis JSON is persisted to tasks.diff_analysis so the Arcana panel can render the risk card alongside the task. Pass the QC task you are reviewing."
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
        let repo_path_buf = match super::path_jail::validate_workspace_path(
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

        // Generate branch diff
        let patch = match crate::git::diff::branch_diff(repo_path, base_branch, head_branch) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Failed to generate diff: {e}")),
        };

        // 🔴 T-52 — bound the patch before it leaves on the user's key.
        //
        // `branch_diff` output went to the provider with no size check, no
        // truncation and no filter. A branch touching a lock file, a generated
        // bundle or a vendored tree produces megabytes, and every other tool in
        // this directory bounds its output. The cost lands on the user's own
        // key, so an unbounded upload is a spend defect as well as a latency
        // one -- and the model cannot use a patch it has no context window for.
        const MAX_PATCH_BYTES: usize = 256 * 1024;
        if patch.len() > MAX_PATCH_BYTES {
            return ToolResult::error(format!(
                "the diff is {} bytes, over the {MAX_PATCH_BYTES}-byte cap for \
                 analysis. Narrow it: analyse a subset of files, or compare \
                 nearer commits. Sending it would spend the user's key on a \
                 patch larger than the model's useful context.",
                patch.len()
            ));
        }

        let analysis =
            analyze_patch_with_workspace_consent(&patch, std::path::Path::new(&ctx.working_dir))
                .await;

        match analysis {
            Ok(result) => {
                let summary = format!(
                    "{} | Risk: {} ({:.2}) | Classification: {:?} | Files: {} | +{} -{}",
                    if result.analysis_mode == "deterministic_only" {
                        "Deterministic-only: workspace consent is missing, mismatched, or revoked"
                    } else {
                        "Provider-assisted"
                    },
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
                // task so the Arcana panel's DiffAnalysisCard can render it
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

fn active_provider_destination() -> crate::errors::Result<String> {
    let provider_id =
        crate::db::app_settings::get("active_provider")?.unwrap_or_else(|| "anthropic".to_string());
    crate::egress_consent::provider_destination(&provider_id)
}

async fn analyze_patch_with_workspace_consent(
    patch: &str,
    workspace: &std::path::Path,
) -> crate::errors::Result<crate::diff_analysis::DiffAnalysisResult> {
    let destination = active_provider_destination()?;
    let egress_allowed = crate::egress_consent::WorkspaceEgressConsentDb::open()?
        .authorize_and_audit(workspace, &destination, "qc_diff_analysis")?;
    if !egress_allowed {
        // The deterministic review must remain usable without a provider key.
        // Consent is checked before vault lookup or provider construction.
        return DiffAnalyzer::analyze_structural(patch);
    }

    let analyzer = build_analyzer_from_vault(workspace)?;
    match analyzer.analyze(patch).await {
        Err(crate::errors::ArcanaError::EgressDenied { .. }) => {
            DiffAnalyzer::analyze_structural(patch)
        }
        result => result,
    }
}

fn build_analyzer_from_vault(workspace: &std::path::Path) -> crate::errors::Result<DiffAnalyzer> {
    use crate::errors::ArcanaError;

    // ── P2-31 · this tool used to bypass model selection entirely ────────────
    //
    // It read `ARCANA_ACTIVE_PROVIDER` and `ARCANA_ACTIVE_MODEL` from the
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
            ArcanaError::Agent(format!(
                "No model available for provider '{provider_id}'. Choose a provider and \
                 model in Settings."
            ))
        })?;

    let api_key = VaultManager::get(&provider_key(&provider_id))?
        .ok_or_else(|| ArcanaError::Agent(format!("No API key for provider '{provider_id}'")))?;

    // Built through the shared factory, which applies the daily ceiling. This
    // tool holds no AppState — tools receive a ToolContext — so it reaches the
    // cap through the process-global handle. Without it this would be the one
    // path a call could take without meeting the ceiling (P2-21).
    let provider = crate::provider::factory::build(&provider_id, api_key)?;
    let provider = Arc::new(crate::egress_consent::ConsentProvider::wrap(
        provider, workspace,
    ));

    Ok(DiffAnalyzer::new(provider, model))
}

#[cfg(test)]
mod consent_tests {
    use super::*;
    use git2::{Repository, Signature};
    use std::fs;
    use std::path::Path;

    fn commit_file(repo: &Repository, path: &str, contents: &str, message: &str) {
        let repo_path = repo.workdir().expect("test repository worktree");
        fs::write(repo_path.join(path), contents).expect("write test fixture file");
        let mut index = repo.index().expect("open index");
        index.add_path(Path::new(path)).expect("stage fixture file");
        index.write().expect("write index");
        let tree_id = index.write_tree().expect("write fixture tree");
        let tree = repo.find_tree(tree_id).expect("read fixture tree");
        let signature = Signature::now("Arcana Test", "arcana-test@example.invalid")
            .expect("fixture signature");
        let parent = repo.head().ok().and_then(|head| head.peel_to_commit().ok());
        let parents = parent.iter().collect::<Vec<_>>();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        )
        .expect("commit fixture change");
    }

    #[tokio::test]
    async fn qc_diff_without_matching_consent_uses_structural_review_without_provider_key() {
        let home = tempfile::tempdir().expect("isolated Arcana data root");
        let _home = crate::db::test_support::with_home(home.path());
        let patch =
            "diff --git a/src/lib.rs b/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n";
        let workspace = home.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("create workspace");

        // A different provider grant must not authorize the active provider.
        let consent = crate::egress_consent::WorkspaceEgressConsentDb::open()
            .expect("open isolated consent store");
        consent
            .grant(&workspace, "provider:google")
            .expect("grant mismatched destination");

        let result = analyze_patch_with_workspace_consent(patch, &workspace)
            .await
            .expect("deterministic QC review should work without a provider key");
        assert_eq!(result.analysis_mode, "deterministic_only");
        assert!(result.file_count > 0);
    }

    #[tokio::test]
    async fn analyze_diff_tool_executes_repository_diff_without_consent_or_provider_key() {
        let home = tempfile::tempdir().expect("isolated Arcana data root");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = home.path().join("workspace");
        let repo_path = workspace.join("repo");
        fs::create_dir_all(&repo_path).expect("create test workspace");
        let mut options = git2::RepositoryInitOptions::new();
        options.initial_head("main");
        let repo = Repository::init_opts(&repo_path, &options).expect("initialize repo");
        let mut config = repo.config().expect("read test git config");
        config
            .set_str("user.name", "Arcana Test")
            .expect("set fixture author");
        config
            .set_str("user.email", "arcana-test@example.invalid")
            .expect("set fixture email");
        commit_file(&repo, "README.md", "Calendar baseline\n", "base");
        let base = repo
            .head()
            .expect("base head")
            .peel_to_commit()
            .expect("base commit");
        let feature = repo
            .branch("feature/calendar", &base, false)
            .expect("feature branch");
        repo.set_head(feature.get().name().expect("feature ref"))
            .expect("checkout feature ref");
        repo.checkout_head(None).expect("checkout feature worktree");
        commit_file(
            &repo,
            "README.md",
            "Calendar baseline\nAdded event planning\n",
            "feature",
        );

        let context = ToolContext {
            session_id: "analyze-diff-consent-test".into(),
            working_dir: workspace.to_string_lossy().into_owned(),
            call_id: "analyze-diff-call".into(),
            run_id: None,
            task_id: None,
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "qc-agent".into(),

            run_token_budget: None,
        };
        let result = AnalyzeDiffTool
            .execute(
                json!({
                    "repo_path": repo_path,
                    "base_branch": "main",
                    "head_branch": "feature/calendar"
                }),
                &context,
            )
            .await;
        assert!(
            !result.is_error,
            "local structural analysis should work: {}",
            result.output
        );
        assert!(
            result.output.contains("deterministic_only"),
            "tool result should identify its local-only analysis mode: {}",
            result.output
        );
        assert!(
            result
                .hint
                .as_deref()
                .unwrap_or_default()
                .contains("workspace consent is missing"),
            "tool result should explain why provider analysis was skipped"
        );

        let config = crate::db::config_db::open().expect("open isolated config DB");
        let denied: i64 = config
            .query_row(
                "SELECT COUNT(*) FROM workspace_egress_audit
                 WHERE outcome = 'denied' AND operation = 'qc_diff_analysis'",
                [],
                |row| row.get(0),
            )
            .expect("read actual tool egress audit");
        assert_eq!(denied, 1, "the production diff tool must audit the refusal");
    }
}
