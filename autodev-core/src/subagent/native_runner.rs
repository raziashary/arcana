// Native in-process subagent runner.
//
// Replaces `subagent/bridge.rs`, which drove a create → invoke(SSE) → delete
// lifecycle against the Python `core-orchestrator` sidecar. Subagents now run
// on the same Rust `AgentEngine` as every other agent, inside this process.
//
// Two things this buys beyond dropping a Python dependency:
//
//   1. The nesting guard becomes real. The old path carried the child's depth
//      in its PROMPT — `bridge.rs` injected "Include `__depth: {n}` in every
//      tool call" and `mcp_server/tool_adapter.rs` read it back off the MCP
//      arguments — so `MAX_NESTING_DEPTH` was enforced by asking the model to
//      report its own recursion depth honestly. Depth is now threaded in Rust
//      from the spawn site (`AgentIdentity::child`) and cannot be forged.
//
//   2. Subagents work on a machine with no Python, which the headline
//      acceptance test requires.
//
// See ADR-LOG-RESTRUCT-0001.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use tracing::{info, warn};

use crate::agent::engine::AgentEngine;
use crate::agent::permission::{PendingPermissions, PermissionMode};
use crate::agent::tool_trait::AgentIdentity;
use crate::agent::PAMode;
use crate::errors::{AutoDevError, Result};
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::google::GoogleProvider;
use crate::provider::openai::OpenAIProvider;
use crate::provider::Provider;
use crate::state::app_state::AppState;
use crate::vault::keychain::{provider_key, VaultManager};

use super::types::{SubagentConfig, SubagentEvent, SubagentResult, SubagentStatus};

/// What the runner needs from the application to stand up a child engine.
///
/// Held by `SubagentManager` and cloned per run. The `AppHandle` is fetched
/// lazily from the process-wide handle installed in `main.rs`'s setup hook,
/// because the manager is constructed before Tauri has one.
#[derive(Clone)]
pub struct RunnerDeps {
    pub state: Arc<Mutex<AppState>>,
    pub pending: PendingPermissions,
}

/// Execute a subagent to completion on a nested `AgentEngine`.
///
/// `events_tx` receives lifecycle events so `SubagentManager` can rebroadcast
/// them. Token-level output is not duplicated onto this channel: the child
/// engine already streams tokens and tool events on the normal agent event bus
/// under `subagent_id` as its session id, and emitting them twice would double
/// every token in the UI.
pub async fn run(
    deps: RunnerDeps,
    subagent_id: String,
    config: SubagentConfig,
    prompt: String,
    events_tx: mpsc::UnboundedSender<SubagentEvent>,
) -> Result<SubagentResult> {
    let started_at = Instant::now();

    emit(
        &events_tx,
        SubagentEvent::Status {
            subagent_id: subagent_id.clone(),
            status: SubagentStatus::Spawning,
            message: Some("Starting in-process subagent".into()),
        },
    );

    let outcome = run_inner(&deps, &subagent_id, &config, &prompt, &events_tx).await;
    let duration_secs = started_at.elapsed().as_secs_f64();

    match outcome {
        Ok(summary) => {
            emit(
                &events_tx,
                SubagentEvent::Final {
                    subagent_id: subagent_id.clone(),
                    text: summary.clone(),
                },
            );
            emit(
                &events_tx,
                SubagentEvent::Status {
                    subagent_id: subagent_id.clone(),
                    status: SubagentStatus::Completed,
                    message: None,
                },
            );
            info!("[NativeSubagent] {subagent_id} completed in {duration_secs:.1}s");

            Ok(SubagentResult {
                subagent_id,
                status: SubagentStatus::Completed,
                result_summary: summary,
                // Populated when worktree integration lands (P4). Left empty
                // rather than guessed — a fabricated file list is worse than
                // an honest absence.
                files_modified: Vec::new(),
                // The child engine records its own token usage and cost into
                // analytics per session. Rolling that up to the subagent level
                // is not wired yet, so these are reported as zero rather than
                // invented.
                cost_usd: 0.0,
                tokens_in: 0,
                tokens_out: 0,
                duration_secs,
                error: None,
            })
        }
        Err(e) => {
            let message = e.to_string();
            warn!("[NativeSubagent] {subagent_id} failed: {message}");
            emit(
                &events_tx,
                SubagentEvent::Status {
                    subagent_id: subagent_id.clone(),
                    status: SubagentStatus::Failed,
                    message: Some(message.clone()),
                },
            );

            Ok(SubagentResult {
                subagent_id,
                status: SubagentStatus::Failed,
                result_summary: String::new(),
                files_modified: Vec::new(),
                cost_usd: 0.0,
                tokens_in: 0,
                tokens_out: 0,
                duration_secs,
                error: Some(message),
            })
        }
    }
}

/// The run itself. Split out so both success and failure paths get a single
/// place to stamp duration and emit terminal status.
async fn run_inner(
    deps: &RunnerDeps,
    subagent_id: &str,
    config: &SubagentConfig,
    prompt: &str,
    events_tx: &mpsc::UnboundedSender<SubagentEvent>,
) -> Result<String> {
    let (provider_id, fallback_model) = {
        let s = deps.state.lock().unwrap();
        (s.active_provider.clone(), s.active_model.clone())
    };
    let provider_id = provider_id.ok_or_else(|| {
        AutoDevError::Agent("No active provider set — cannot spawn a subagent".into())
    })?;

    let model = if config.model.is_empty() {
        fallback_model.unwrap_or_else(|| SubagentConfig::DEFAULT_MODEL.to_string())
    } else {
        config.model.clone()
    };

    let provider = build_provider(&provider_id)?;

    emit(
        events_tx,
        SubagentEvent::Status {
            subagent_id: subagent_id.to_string(),
            status: SubagentStatus::Running,
            message: Some(format!("Running on {provider_id}/{model}")),
        },
    );

    // `config.depth` is ALREADY this subagent's depth: every spawn tool sets
    // `depth: ctx.depth + 1` when building the config. Calling `.child()` here
    // as well incremented it a second time, so a subagent of a depth-0 agent
    // ran at depth 2 — the pool guard and SubagentSummary saw the right number
    // while the agent itself saw another, and MAX_NESTING_DEPTH fired after
    // roughly half the intended generations. Found by the P0 post-phase audit
    // (F-21), in the commit that claimed depth was now unforgeable.
    let identity = identity_for(subagent_id, config);

    // Autonomous: a subagent has no user to prompt for permission, and a
    // blocked prompt would hang until the timeout.
    //
    // No SessionManager is attached: subagent sessions are ephemeral and were
    // never inserted into the sessions table, so append_message would fail on a
    // foreign-key constraint. This mirrors RecipeRunner's Mode 1 path.
    let engine = AgentEngine::new(
        provider,
        PermissionMode::Autonomous,
        PAMode::FastDevelopment,
        deps.state.clone(),
        deps.pending.clone(),
        identity,
    );

    let working_dir = config.working_dir.clone();
    let system_prompt =
        (!config.system_prompt.trim().is_empty()).then(|| config.system_prompt.clone());

    let run_future = engine.run(
        subagent_id,
        &working_dir,
        &model,
        Vec::new(),
        prompt,
        system_prompt.as_deref(),
    );

    let history = match timeout(
        Duration::from_secs(config.timeout_secs.max(1) as u64),
        run_future,
    )
    .await
    {
        Ok(result) => result?,
        Err(_) => {
            return Err(AutoDevError::Agent(format!(
                "Subagent exceeded its {}s timeout",
                config.timeout_secs
            )))
        }
    };

    Ok(last_assistant_text(&history))
}

/// Build a provider from the stored key. Mirrors `RecipeRunner`'s resolution so
/// a subagent uses exactly the same credentials as its parent.
fn build_provider(provider_id: &str) -> Result<Arc<dyn Provider>> {
    // Every supported provider takes a key. The local-provider branch went
      // with Ollama (P2-11) — there is no keyless provider any more, so the
      // absence of a key is now unconditionally an error rather than a case.
    let api_key = VaultManager::get(&provider_key(provider_id))?.ok_or_else(|| {
        AutoDevError::Agent(format!(
            "No API key stored for provider '{provider_id}' — set it in Settings"
        ))
    })?;

    Ok(match provider_id {
        "anthropic" => Arc::new(AnthropicProvider::new(api_key)) as Arc<dyn Provider>,
        "openai" => Arc::new(OpenAIProvider::new(api_key)),
        "google" => Arc::new(GoogleProvider::new(api_key)),
        other => {
            return Err(AutoDevError::Agent(format!(
                "Unsupported provider for subagent: {other}"
            )))
        }
    })
}

/// Concatenate the text of the final assistant message.
fn last_assistant_text(history: &[crate::provider::types::RequestMessage]) -> String {
    use crate::provider::types::MessageContent;

    history
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .map(|m| {
            m.content
                .iter()
                .filter_map(|c| match c {
                    MessageContent::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn emit(tx: &mpsc::UnboundedSender<SubagentEvent>, event: SubagentEvent) {
    // A closed receiver means the manager stopped listening; the run itself is
    // still valid, so this is not an error.
    let _ = tx.send(event);
}

/// The identity a subagent runs under.
///
/// `config.depth` is ALREADY the child's depth — every spawn tool sets
/// `depth: ctx.depth + 1` when it builds the config — so this must NOT
/// increment again. Split out purely so that invariant is testable.
fn identity_for(_subagent_id: &str, config: &SubagentConfig) -> AgentIdentity {
    // The ROLE, not the id. Using the generated per-spawn id here made every
    // role comparison downstream fail: `annotate_file_tool` filed QC
    // annotations as "user", and the messaging tools produced an inbox address
    // no sender could know (F-04, F-05).
    let role = if config.role.trim().is_empty() {
        "subagent".to_string()
    } else {
        config.role.trim().to_string()
    };
    AgentIdentity {
        role,
        depth: config.depth,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_at_depth(depth: u32) -> SubagentConfig {
        SubagentConfig {
            parent_slug: "worker-backend".into(),
            role: "subagent".into(),
            parent_session_id: "sess-1".into(),
            system_prompt: String::new(),
            tools: Vec::new(),
            model: String::new(),
            working_dir: String::new(),
            max_turns: 0,
            timeout_secs: 0,
            depth,
        }
    }

    /// Regression for the P0 post-phase audit finding F-21.
    ///
    /// The spawn tools already set `depth: ctx.depth + 1`. This used to build
    /// an identity at that depth and then call `.child()`, incrementing a
    /// second time — so a subagent of a depth-0 agent ran at depth 2. The pool
    /// guard and `SubagentSummary` saw the correct number while the agent
    /// itself saw another, and `MAX_NESTING_DEPTH` fired after roughly half the
    /// intended generations.
    #[test]
    fn a_subagent_runs_at_exactly_the_depth_its_config_names() {
        for depth in [0, 1, 2, 3] {
            let identity = identity_for("subagent-worker-backend-1", &config_at_depth(depth));
            assert_eq!(
                identity.depth, depth,
                "config.depth is already the child's depth; it must not be incremented again"
            );
        }
    }

    /// Regression for F-04/F-05: the identity carries the child's ROLE, not the
    /// per-spawn id. A role is matched by consumers (`annotate_file_tool`) and
    /// used as an inbox address; an id is unique per spawn and matches nothing.
    #[test]
    fn the_subagents_identity_is_its_role_not_its_generated_id() {
        let mut config = config_at_depth(1);
        config.role = "qc-agent".into();
        let identity = identity_for("subagent-qc-dispatcher-1745740800123-1", &config);

        assert_eq!(identity.role, "qc-agent");
        assert!(
            !identity.role.starts_with("subagent-qc-dispatcher-"),
            "the generated id must not be used as a role"
        );
        assert_ne!(identity.role, "worker-backend", "nor the parent's role");
    }

    #[test]
    fn a_blank_role_falls_back_to_a_real_one() {
        let mut config = config_at_depth(1);
        config.role = "  ".into();
        assert_eq!(identity_for("subagent-x-1", &config).role, "subagent");
    }

    /// The end-to-end shape the guard depends on: a parent at depth N spawns a
    /// config at N+1, and the child runs at exactly N+1.
    #[test]
    fn one_generation_is_exactly_one_level() {
        let parent = AgentIdentity::worker("worker-backend");
        let config = config_at_depth(parent.depth + 1);
        assert_eq!(identity_for("subagent-x", &config).depth, parent.depth + 1);
    }
}
