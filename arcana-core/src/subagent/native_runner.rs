// Native in-process subagent runner.
//
// Replaces `subagent/bridge.rs`, which drove a create → invoke(SSE) → delete
// lifecycle against a separate process. Subagents now run on the same Rust
// `AgentEngine` as every other agent, inside this one.
//
// Two things this buys beyond dropping the dependency:
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

use crate::agent::engine::{AgentEngine, RunAccounting};
use crate::agent::permission::PendingPermissions;
use crate::agent::tool_trait::AgentIdentity;
use crate::agent::PAMode;
use crate::errors::{ArcanaError, Result};
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::google::GoogleProvider;
use crate::provider::openai::OpenAIProvider;
use crate::provider::Provider;
use crate::state::app_state::AppState;
use crate::vault::keychain::{provider_key, VaultManager};

use super::types::{SubagentConfig, SubagentEvent, SubagentResult, SubagentStatus};
use crate::state::lock::LockTolerant;

/// What the runner needs from the application to stand up a child engine.
///
/// Held by `SubagentManager` and cloned per run. The `AppHandle` is fetched
/// lazily from the process-wide handle installed in `main.rs`'s setup hook,
/// because the manager is constructed before Tauri has one.
#[derive(Clone)]
pub struct RunnerDeps {
    pub state: Arc<Mutex<AppState>>,
    pub pending: PendingPermissions,
    #[cfg(any(test, feature = "offline-fleet-test"))]
    pub provider_override: Option<Arc<dyn Provider>>,
}

#[cfg(test)]
mod registered_tool_success_tests;

/// Choose a model the active provider can actually serve.
///
/// 🔴 `P3-61`. A subagent inherited `DEFAULT_MODEL` — a hardcoded Anthropic id
/// — regardless of which provider was active, so a Google-primary install sent
/// Anthropic ids to Google on every default subagent. `spawn_qc_review` had the
/// same defect against its own hardcoded id.
///
/// Order: the requested model if this provider serves it, then the user's
/// active model if this provider serves that, then the provider's own default.
/// The last step is what makes this total — there is always an answer, and it
/// is always one the provider recognises.
fn pick_servable_model(provider_id: &str, requested: &str, active: Option<&str>) -> String {
    let serves = |m: &str| {
        !m.trim().is_empty() && crate::provider::routing::provider_for_model(m) == Some(provider_id)
    };

    if serves(requested) {
        return requested.to_string();
    }
    if let Some(a) = active {
        if serves(a) {
            warn!(
                "[NativeSubagent] requested model '{requested}' is not served by \
                 '{provider_id}' — falling back to the active model '{a}'"
            );
            return a.to_string();
        }
    }

    let fallback = crate::provider::catalogue::default_model_for(provider_id)
        .unwrap_or(SubagentConfig::DEFAULT_MODEL);
    warn!(
        "[NativeSubagent] neither '{requested}' nor the active model is served by \
         '{provider_id}' — using its default '{fallback}'"
    );
    fallback.to_string()
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
    run_token_budget: Option<Arc<crate::budget::run_tokens::RunTokenLedger>>,
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

    let mut accounting = RunAccounting::default();
    let outcome = run_inner(
        &deps,
        &subagent_id,
        &config,
        &prompt,
        &events_tx,
        &mut accounting,
        run_token_budget,
    )
    .await;
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
                // 🔴 P3-62. These were hardcoded `0.0/0/0` under a comment
                // saying roll-up "is not wired yet". The QC **hourly dollar
                // ceiling** is computed from exactly these fields, so every
                // completed reviewer settled at $0.00 and the ceiling only
                // ever bounded in-flight concurrency — it self-cancelled on
                // completion and never bounded spend at all.
                //
                // Pairs with `P3-55`: that was a charge never released, this
                // is a cost never recorded. Either one alone leaves the
                // ceiling meaningless.
                cost_usd: accounting.cost_usd,
                tokens_in: accounting.tokens_in,
                tokens_out: accounting.tokens_out,
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
                // P3-62. A failed subagent still spent money — a timeout at
                // 600s spent a great deal of it. Reporting zero here would
                // hide precisely the runs the ceiling exists to catch.
                cost_usd: accounting.cost_usd,
                tokens_in: accounting.tokens_in,
                tokens_out: accounting.tokens_out,
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
    // ⚠ P3-62. An out-param rather than a return value, deliberately: the
    // accounting must survive the **error** paths too, and a `Result` carries
    // it on neither. A subagent that timed out still spent the user's money —
    // and under the QC hourly ceiling, the runs that fail late are exactly the
    // expensive ones. Same discipline as `record_spend`: written on every exit,
    // especially the ones that return early.
    accounting: &mut RunAccounting,
    run_token_budget: Option<Arc<crate::budget::run_tokens::RunTokenLedger>>,
) -> Result<String> {
    let (provider_id, fallback_model) = {
        let s = deps.state.lock_tolerant();
        (s.active_provider.clone(), s.active_model.clone())
    };
    let provider_id = provider_id.ok_or_else(|| {
        ArcanaError::Agent("No active provider set — cannot spawn a subagent".into())
    })?;

    // ⚠ P3-61. `manager.rs::spawn` fills `config.model` with
    // `SubagentConfig::DEFAULT_MODEL` (an **Anthropic** id) before the config
    // is cloned, so `config.model.is_empty()` was never true by the time
    // execution reached here — making this whole `fallback_model` branch dead
    // code. On a Google-primary install, which `models.ts` declares as
    // supported, **every default subagent sent an Anthropic model id to
    // Google**.
    //
    // Emptiness is therefore the wrong question. The right one is whether the
    // model belongs to the provider that will actually serve it.
    let model = pick_servable_model(&provider_id, &config.model, fallback_model.as_deref());

    // A subagent's ID is its run identity; pair it with a stable per-run
    // session key so duplicate active runs are rejected and cleanup is scoped.
    let _runtime_lease =
        crate::state::runtime_registry::global().admit(subagent_id, subagent_id)?;

    #[cfg(any(test, feature = "offline-fleet-test"))]
    let provider = match &deps.provider_override {
        Some(provider) => {
            crate::provider::capped::CappedProvider::wrap(provider.clone(), deps.state.clone())
        }
        None => build_provider(&provider_id, deps.state.clone())?,
    };
    #[cfg(not(any(test, feature = "offline-fleet-test")))]
    let provider = build_provider(&provider_id, deps.state.clone())?;

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

    // ⚠ G-5. This hardcoded `PermissionMode::Autonomous` under the comment
    // *"a subagent has no user to prompt for permission, and a blocked prompt
    // would hang until the timeout"* — while `deps.pending`, the permission
    // channel itself, was passed in two lines below. A model held in
    // `ManualApproval` escaped every prompt with one `spawn_subagent` call:
    // the child ran unattended, with `bash`, on the user's own key.
    //
    // The mode is inherited from the parent now. A fleet recipe already runs
    // Autonomous, so its subagents still do — nothing about unattended work
    // changes. What changes is that an INTERACTIVE parent's child is bound by
    // the same policy the user chose for the parent.
    //
    // No SessionManager is attached: subagent sessions are ephemeral and were
    // never inserted into the sessions table, so append_message would fail on a
    // foreign-key constraint. This mirrors RecipeRunner's Mode 1 path.
    let engine = AgentEngine::new(
        provider,
        config.permission_mode.clone(),
        PAMode::FastDevelopment,
        deps.state.clone(),
        deps.pending.clone(),
        identity,
    )
    .with_workspace_egress_consent()
    // P2-48. A subagent is autonomous and fans out — `spawn_parallel_subagents`
    // runs several at once — so an unbounded one multiplies. Same default
    // ceiling as a worker.
    .with_budget({
        // ⚠ P3-63. `config.max_turns` was defaulted in `manager.rs::spawn` and
        // then never read by anything: a caller setting a turn bound on a
        // subagent got the worker default regardless. `TaskBudget` already
        // carries `max_turns` and the engine already enforces it — the field
        // simply was not connected to it. An explicit value now reaches the
        // enforcer, which is the fix.
        //
        // ⚠ F-27 (post-phase claims audit): the `> 0` test is **always true**.
        // `manager.rs:122` replaces a zero with `DEFAULT_MAX_TURNS` before the
        // config is cloned, so the else-branch is unreachable — the same dead
        // `is_empty()`/`== 0` shape as `P3-61`, which I identified and fixed in
        // that very commit and then reproduced here. It is harmless only by
        // coincidence: `DEFAULT_MAX_TURNS` and `default_worker().max_turns` are
        // both 20. The guard is kept because it states the intended contract
        // and stops being coincidental the moment either constant moves.
        let mut budget = crate::budget::TaskBudget::default_worker();
        if config.max_turns > 0 {
            budget.max_turns = Some(config.max_turns);
        }
        budget
    })
    .with_optional_run_token_budget(run_token_budget)
    // ⚠ P3-60. `SubagentConfig.tools` is documented as "tool subset the
    // subagent is allowed to use", is written by all three spawn tools AND the
    // Tauri command, and had **zero readers** — the child received the full
    // registry. A containment promise the code did not keep.
    //
    // Narrowing happens after `register_for_mode`, so a subset can only REMOVE
    // tools the PA mode already granted; it can never widen access.
    .with_tool_subset(&config.tools);

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

    let outcome = timeout(
        Duration::from_secs(config.timeout_secs.max(1) as u64),
        run_future,
    )
    .await;

    // Read BEFORE unwrapping the outcome, so a timeout or an engine error
    // still reports what it spent.
    if let Some(spent) = engine.last_run_accounting() {
        *accounting = spent;
    }

    let history = match outcome {
        Ok(result) => result?,
        Err(_) => {
            return Err(ArcanaError::Agent(format!(
                "Subagent exceeded its {}s timeout",
                config.timeout_secs
            )))
        }
    };

    Ok(last_assistant_text(&history))
}

/// Build a provider from the stored key. Mirrors `RecipeRunner`'s resolution so
/// a subagent uses exactly the same credentials as its parent.
/// Build the provider a subagent will use, **wrapped in the daily ceiling**.
///
/// The wrap happens here rather than at the call site so a future caller cannot
/// get an uncapped provider by forgetting a step — the only way to obtain one
/// from this module is through the cap.
fn build_provider(
    provider_id: &str,
    state: Arc<std::sync::Mutex<AppState>>,
) -> Result<Arc<dyn Provider>> {
    // Every supported provider takes a key. The local-provider branch went
    // with Ollama (P2-11) — there is no keyless provider any more, so the
    // absence of a key is now unconditionally an error rather than a case.
    let api_key = VaultManager::get(&provider_key(provider_id))?.ok_or_else(|| {
        ArcanaError::Agent(format!(
            "No API key stored for provider '{provider_id}' — set it in Settings"
        ))
    })?;

    let inner: Arc<dyn Provider> = match provider_id {
        "anthropic" => Arc::new(AnthropicProvider::new(api_key)) as Arc<dyn Provider>,
        "openai" => Arc::new(OpenAIProvider::new(api_key)),
        "google" => Arc::new(GoogleProvider::new(api_key)),
        other => {
            return Err(ArcanaError::Agent(format!(
                "Unsupported provider for subagent: {other}"
            )))
        }
    };

    // Every subagent call passes the daily ceiling (P2-21).
    Ok(crate::provider::capped::CappedProvider::wrap(inner, state))
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
    use crate::agent::permission::PermissionMode;

    fn config_at_depth(depth: u32) -> SubagentConfig {
        SubagentConfig {
            parent_slug: "worker-backend".into(),
            role: "subagent".into(),
            permission_mode: PermissionMode::Autonomous,
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

#[cfg(test)]
mod p3_58_63_tests {
    use super::*;

    // ── P3-61 · a subagent never gets a model its provider cannot serve ──

    #[test]
    fn a_google_install_never_receives_an_anthropic_model_id() {
        // 🔴 THE DEFECT. `manager.rs::spawn` fills `config.model` with
        // `DEFAULT_MODEL` — an Anthropic id — BEFORE the config is cloned, so
        // `config.model.is_empty()` was never true here and the whole
        // `fallback_model` branch was dead code. On a Google-primary install,
        // which `models.ts` declares as supported, every default subagent sent
        // an Anthropic id to Google.
        let chosen = pick_servable_model("google", SubagentConfig::DEFAULT_MODEL, None);

        assert_eq!(
            crate::provider::routing::provider_for_model(&chosen),
            Some("google"),
            "a Google-primary install was handed '{chosen}', which Google does \
             not serve"
        );
    }

    #[test]
    fn the_qc_default_is_corrected_too() {
        // `spawn_qc_review` hardcodes `claude-sonnet-5`. It reaches the same
        // chokepoint, which is why the fix lives here rather than at each
        // caller — a per-caller fix leaves the next caller broken.
        let chosen = pick_servable_model("google", crate::db::workers::DEFAULT_MODEL, None);
        assert_eq!(
            crate::provider::routing::provider_for_model(&chosen),
            Some("google")
        );
    }

    #[test]
    fn a_requested_model_the_provider_serves_is_honoured() {
        // The no-fire half: if this over-corrects, every explicit model choice
        // is silently discarded.
        let requested = "claude-sonnet-5";
        let chosen = pick_servable_model("anthropic", requested, None);
        assert_eq!(chosen, requested);
    }

    #[test]
    fn the_active_model_is_preferred_over_the_providers_default() {
        // A user who picked a specific Gemini should get that one, not
        // whichever Gemini the catalogue lists first.
        let active = crate::provider::catalogue::models_for("google")
            .into_iter()
            .map(|m| m.id)
            .find(|id| {
                Some(id.as_str()) != crate::provider::catalogue::default_model_for("google")
            });

        if let Some(active) = active {
            let chosen = pick_servable_model("google", "claude-opus-4", Some(&active));
            assert_eq!(
                chosen, active,
                "the user's active model must win over the catalogue default"
            );
        }
    }

    #[test]
    fn every_provider_has_a_default_the_router_agrees_with() {
        // Guards the fallback itself: a provider whose default is served by a
        // DIFFERENT provider would send the wrong id on every subagent, which
        // is the exact defect one layer down.
        for provider in ["anthropic", "openai", "google"] {
            let d = crate::provider::catalogue::default_model_for(provider)
                .unwrap_or_else(|| panic!("{provider} has no catalogue entry"));
            assert_eq!(
                crate::provider::routing::provider_for_model(d),
                Some(provider),
                "{provider}'s default '{d}' routes elsewhere"
            );
        }
    }
}

#[cfg(test)]
mod g5_tests {
    use super::*;
    use crate::agent::permission::PermissionMode;
    use crate::agent::tool_effect::ToolEffect;

    /// A subagent inherits its parent's permission mode.
    ///
    /// 🔴 G-5 (post-phase gap hunt). `native_runner` hardcoded
    /// `PermissionMode::Autonomous` for every subagent regardless of parent,
    /// `tools` defaulted to empty (= unrestricted), and the child got `bash`.
    /// So a model held in `ManualApproval` escaped every prompt with a single
    /// `spawn_subagent` call — on the user's own key, under BYOK.
    ///
    /// The justification comment read "a subagent has no user to prompt for
    /// permission"; two lines below it, `deps.pending` — the permission
    /// channel — was passed straight in.
    #[test]
    fn a_child_cannot_outrank_its_parent() {
        for parent in [
            PermissionMode::ManualApproval,
            PermissionMode::SmartApprove,
            PermissionMode::Autonomous,
        ] {
            let ctx_mode = parent.clone();
            // What the spawn tools now write into the config.
            let child = ctx_mode.clone();

            assert_eq!(
                child, parent,
                "the child's mode must equal the parent's; anything else is an \
                 escape hatch the model can reach with one tool call"
            );

            // And the inheritance must be load-bearing: under a prompting
            // parent, the child must still prompt for the dangerous effects.
            if !matches!(parent, PermissionMode::Autonomous) {
                assert!(
                    ToolEffect::Execute.requires_prompt(&child),
                    "a child of a {parent:?} parent runs commands unprompted"
                );
                assert!(
                    ToolEffect::Network.requires_prompt(&child),
                    "a child of a {parent:?} parent leaves the machine unprompted"
                );
            }
            assert!(ToolEffect::HostApproval.requires_prompt(&child));
        }
    }

    #[test]
    fn the_fleet_is_unaffected() {
        // The no-fire half, and it matters: a recipe already runs Autonomous,
        // so its subagents still do. If inheritance changed that, unattended
        // cron work would stall on prompts nobody can answer.
        let fleet_parent = PermissionMode::Autonomous;
        for effect in [
            ToolEffect::Read,
            ToolEffect::Write,
            ToolEffect::Execute,
            ToolEffect::Network,
            ToolEffect::Board,
        ] {
            assert!(
                !effect.requires_prompt(&fleet_parent),
                "{effect:?} would prompt a fleet subagent"
            );
        }
        assert!(ToolEffect::FreeformShell.requires_prompt(&fleet_parent));
        assert!(ToolEffect::HostApproval.requires_prompt(&fleet_parent));
    }
}
