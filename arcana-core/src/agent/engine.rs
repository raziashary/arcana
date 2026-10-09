// AgentEngine — the PA-mode agentic loop.
// State machine: Idle → Thinking → Streaming → ExecutingTool → Appending → Complete
// Pattern adopted from Goose's agent.rs reply_internal() and
// Claude Code's query.ts runQuery().
// See DEVELOPMENT_PLAN.md Section 9.4 for full state machine diagram.

use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::builtin_tools;
use super::compaction::CompactionEngine;
use super::context::ContextBuilder;
use super::permission::{PendingPermissions, PermissionMode};
use super::tool_executor::ToolExecutor;
use super::tool_registry::ToolRegistry;
use super::tool_trait::{Tool, ToolResult};
use crate::cost::tracker::CostTracker;
use crate::errors::{ArcanaError, Result};
use crate::events::{emit_thinking, emit_token, AgentDonePayload, ThinkingPayload, TokenPayload};
// ⚠ P3-05. `emit_agent_done` / `emit_agent_error` / `AgentErrorPayload` are
// deliberately NOT imported here. `TerminalEventGuard` owns terminal emission
// outright — there is no way to emit one without going through it, and
// therefore no way to emit two. Re-importing them here is how that guarantee
// gets quietly undone.
use crate::hooks::{HookContext, HookRegistry, HookType};
use crate::provider::traits::Provider;
use crate::provider::types::{CompletionRequest, MessageContent, RequestMessage, StreamChunk};
use crate::provider::usage::{StopReason, Usage};
use crate::session::manager::SessionManager;
use crate::state::lock::LockTolerant;
use crate::state::AppState;

const MAX_TOOL_TURNS: usize = 20;

struct ProviderCallTelemetry {
    provider: String,
    model: String,
    started: std::time::Instant,
    attempts: u32,
    usage: Usage,
    outcome: &'static str,
    result_blocks: usize,
}

impl ProviderCallTelemetry {
    fn new(model: &str) -> Self {
        Self {
            provider: crate::provider::routing::provider_for_model(model)
                .unwrap_or("unknown")
                .to_owned(),
            model: model.to_owned(),
            started: std::time::Instant::now(),
            attempts: 0,
            usage: Usage::default(),
            outcome: "error",
            result_blocks: 0,
        }
    }

    fn update_usage(&mut self, current: &Usage, before: Usage) {
        self.usage = Usage {
            input: current.input.saturating_sub(before.input),
            cached_read: current.cached_read.saturating_sub(before.cached_read),
            cache_write: current.cache_write.saturating_sub(before.cache_write),
            output: current.output.saturating_sub(before.output),
            reasoning: current.reasoning.saturating_sub(before.reasoning),
            tool: current.tool.saturating_sub(before.tool),
        };
    }
}

impl Drop for ProviderCallTelemetry {
    fn drop(&mut self) {
        tracing::info!(
            target: "arcana::telemetry",
            event_type = "provider_call",
            provider = %self.provider,
            model = %self.model,
            attempts = self.attempts,
            duration_ns = self.started.elapsed().as_nanos() as u64,
            input_tokens = self.usage.input,
            cache_read_tokens = self.usage.cached_read,
            cache_write_tokens = self.usage.cache_write,
            output_tokens = self.usage.output,
            reasoning_tokens = self.usage.reasoning,
            tool_tokens = self.usage.tool,
            outcome = self.outcome,
            result_blocks = self.result_blocks,
            "Provider call outcome recorded"
        );
    }
}

/// What one run of the engine actually cost.
///
/// P3-62. `SubagentResult`'s cost and token fields were hardcoded `0.0/0/0`
/// under a comment saying roll-up "is not wired yet". The QC **hourly dollar
/// ceiling** is computed from exactly those fields, so every completed
/// reviewer settled at $0.00 and the ceiling only ever bounded in-flight
/// concurrency -- never spend. It self-cancelled on completion.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunAccounting {
    pub cost_usd: f64,
    pub tokens_in: u32,
    pub tokens_out: u32,
}

pub struct AgentEngine {
    provider: Arc<dyn Provider>,
    registry: ToolRegistry,
    mode: PermissionMode,
    pa_mode: super::PAMode,
    pending: PendingPermissions,
    state: Arc<Mutex<AppState>>,
    /// Who this engine runs as, and its nesting depth. Threaded into every
    /// ToolContext, so tool-side identity and the recursion guard depend on the
    /// spawn tree rather than on anything the model says about itself.
    identity: super::tool_trait::AgentIdentity,
    session_mgr: Option<Arc<SessionManager>>,
    // Ruflo Feature 4 — Budget Circuit Breaker: set via with_task_budget()
    task_id: Option<i64>,
    task_budget: Option<crate::budget::TaskBudget>,
    /// Optional host policy ceiling for one generation request. Fleet
    /// Supervisor runs set this lower than the model's advertised maximum so
    /// run-token reservations do not reserve an unnecessarily large output.
    output_token_limit: Option<u32>,
    /// Inherited only by in-process child agents; roots create a fresh ledger
    /// for each run so independent runs do not consume one another's cap.
    run_token_budget: Option<Arc<crate::budget::run_tokens::RunTokenLedger>>,
    /// Production callers enable persisted workspace egress checks; unit
    /// fixtures keep the provider seam isolated.
    workspace_egress_consent: bool,
    /// Procedural recall is opt-in so only fleet workers receive it.
    procedural_recall: bool,
    /// Whether a live host can answer one-call approval requests.
    host_approval_available: bool,
    /// Durable fleet-run correlation; interactive engines leave this unset.
    worker_run_id: Option<String>,
    fleet_correlation_id: Option<String>,
    /// Accounting for the most recent run (`P3-62`). Interior mutability
    /// because `run` takes `&self` at every call site.
    last_run: Mutex<Option<RunAccounting>>,
}

impl AgentEngine {
    pub fn new(
        provider: Arc<dyn Provider>,
        mode: PermissionMode,
        pa_mode: super::PAMode,
        state: Arc<Mutex<AppState>>,
        pending: PendingPermissions,
        identity: super::tool_trait::AgentIdentity,
    ) -> Self {
        let mut registry = ToolRegistry::new();
        builtin_tools::register_for_mode(&mut registry, pa_mode);

        Self {
            provider,
            registry,
            mode,
            pa_mode,
            pending,
            state,
            identity,
            session_mgr: None,
            task_id: None,
            task_budget: None,
            output_token_limit: None,
            run_token_budget: None,
            workspace_egress_consent: false,
            procedural_recall: false,
            host_approval_available: true,
            worker_run_id: None,
            fleet_correlation_id: None,
            last_run: std::sync::Mutex::new(None),
        }
    }

    pub fn with_workspace_egress_consent(mut self) -> Self {
        self.workspace_egress_consent = true;
        self
    }

    /// Configure the host-side prompt channel for sensitive one-call actions.
    pub fn with_host_approval_available(mut self, available: bool) -> Self {
        self.host_approval_available = available;
        self
    }

    /// Enable procedural memory recall for this engine's tool turns.
    ///
    /// Interactive agents and subagents remain off unless their caller opts
    /// in. The fleet RecipeRunner is the production caller that does so.
    pub(crate) fn with_procedural_recall(mut self, on: bool) -> Self {
        self.procedural_recall = on;
        self
    }

    pub(crate) fn with_worker_run_id(mut self, run_id: impl Into<String>) -> Self {
        self.worker_run_id = Some(run_id.into());
        self
    }

    pub(crate) fn with_fleet_correlation_id(mut self, correlation_id: Option<String>) -> Self {
        self.fleet_correlation_id = correlation_id;
        self
    }

    /// Narrow this engine's tool registry to a declared subset (`P3-60`).
    ///
    /// Empty means unrestricted. Applied after `register_for_mode`, so a
    /// subset can only ever REMOVE tools the PA mode already granted — it can
    /// never widen access.
    pub fn with_tool_subset(mut self, names: &[String]) -> Self {
        self.registry.restrict_to(names);
        self
    }

    /// Usage and cost of the most recent run on this engine (`P3-62`).
    ///
    /// `None` until a run has settled its spend.
    pub fn last_run_accounting(&self) -> Option<RunAccounting> {
        match self.last_run.lock() {
            Ok(g) => *g,
            Err(p) => *p.into_inner(),
        }
    }

    /// Set the session manager for persisting messages
    pub fn with_session_manager(mut self, session_mgr: Arc<SessionManager>) -> Self {
        self.session_mgr = Some(session_mgr);
        self
    }

    /// Attach a task ID and spending budget so the engine enforces per-task token/cost/turn limits.
    /// Call this from the recipe runner before run() when executing a worker task.
    /// Attach a budget with no task id.
    ///
    /// ⚠ **This exists because `with_task_budget` had zero callers for the
    /// whole life of the codebase, and its signature is why.** It demands a
    /// `task_id`, and neither autonomous path has one at construction: the
    /// recipe runner builds the engine before the agent claims a task, and a
    /// subagent has no task at all. So ~100 lines of enforcement — the guard,
    /// `escalate_budget_exceeded`, `TaskBudget::default_worker`,
    /// `BudgetExceededReason` — were unreachable, and **the app had no enforced
    /// spend limit at any scope**.
    ///
    /// A budget is not a property of having a task. It is a property of running
    /// autonomously.
    pub fn with_budget(mut self, budget: crate::budget::TaskBudget) -> Self {
        self.task_budget = Some(budget);
        self
    }

    pub(crate) fn with_output_token_limit(mut self, limit: u32) -> Self {
        self.output_token_limit = Some(limit);
        self
    }

    pub(crate) fn with_optional_run_token_budget(
        mut self,
        ledger: Option<Arc<crate::budget::run_tokens::RunTokenLedger>>,
    ) -> Self {
        self.run_token_budget = ledger;
        self
    }

    /// Attach a budget **and** the task it belongs to, so an overrun can be
    /// escalated against that task.
    pub fn with_task_budget(mut self, task_id: i64, budget: crate::budget::TaskBudget) -> Self {
        self.task_id = Some(task_id);
        self.task_budget = Some(budget);
        self
    }

    /// Run the full agentic loop for a single user message.
    /// Streams tokens and tool events to the frontend via Tauri events.
    /// Returns once the model stops asking for tools — i.e. when the turn's
    /// stop reason classifies as anything other than `StopReason::ToolUse`.
    /// Stated by classification rather than by the literal `"end_turn"`, which
    /// is only Anthropic's spelling for it.
    #[tracing::instrument(
        target = "arcana::telemetry",
        skip_all,
        fields(
            event_type = "trace_span",
            operation = "agent_turn",
            session_id = %session_id,
            // Empty means this is an unscoped turn. Serializing `None` as
            // numeric zero makes unrelated PA turns share a false task key.
            task_id = %crate::observability::timing::correlation_task_id(self.task_id),
            agent_role = %self.identity.role,
            fleet_correlation_id = self.fleet_correlation_id.as_deref().unwrap_or(""),
        )
    )]
    pub async fn run(
        &self,
        session_id: &str,
        working_dir: &str,
        model: &str,
        mut history: Vec<RequestMessage>,
        user_message: &str,
        system_prompt_override: Option<&str>,
    ) -> Result<Vec<RequestMessage>> {
        let mut turn_timer = crate::observability::timing::OperationTimer::start("agent_turn")
            .with_session_id(session_id)
            .with_task_id(self.task_id);
        let mut cancelled_during_run = false;
        // ── P3-05 · exactly one terminal event, on every exit ────────────────
        //
        // Three paths returned without telling anyone (`:128`, `:328`, `:628`),
        // so the task was dead and the UI spun forever. Created **before** the
        // first possible return, and it owns the emission — there is no way to
        // emit a terminal event without going through it, and therefore no way
        // to emit two. `TG3-01` fails zero and two alike.
        let mut terminal = crate::agent::terminal_event::TerminalEventGuard::new(session_id);

        let consented_provider: Arc<dyn Provider> = if self.workspace_egress_consent {
            Arc::new(crate::egress_consent::ConsentProvider::wrap(
                self.provider.clone(),
                working_dir,
            ))
        } else {
            self.provider.clone()
        };
        let run_token_budget = self.run_token_budget.clone().unwrap_or_else(|| {
            crate::budget::run_tokens::RunTokenLedger::new(
                self.task_budget
                    .as_ref()
                    .and_then(|budget| budget.max_tokens),
            )
        });
        let provider = crate::provider::run_budget::RunTokenBudgetProvider::wrap(
            consented_provider,
            Arc::clone(&run_token_budget),
        );

        // Check for cancellation before starting
        if self.state.lock_tolerant().is_cancelled(session_id) {
            // P3-24, and this site was found by `TG-P3-24` itself rather than
            // by me: a session can be cancelled while children spawned by an
            // EARLIER run of the same session are still live. Returning here
            // without stopping them leaves them running and billing, exactly
            // as the in-loop path did.
            if let Some(mgr) = crate::subagent::manager::global() {
                mgr.abort_children_of(session_id).await;
            }
            turn_timer.set_outcome("cancelled");
            terminal.error("The run was cancelled before it started.");
            return Err(ArcanaError::Cancelled);
        }

        let context_builder = ContextBuilder::new(working_dir).with_session(session_id);

        // ── D-12 (P2 post-phase audit) · the two halves stay apart ───────────
        //
        // `build_system_prompt_parts` returns (stable, volatile) precisely so
        // the memory block can sit AFTER the cache breakpoint — and this call
        // site used the joining wrapper, then handed the joined string to
        // `prefix::build` as `system_base` with empty slices for everything
        // else. So the split existed, was unit-tested, and was **never taken in
        // production**: memory sat inside the cached region, every learned fact
        // invalidated the whole prefix, and each turn paid the cache-write
        // surcharge for a prefix that could never be read back.
        //
        // That is `L-18` — a fix that lands in the right place and is never
        // reached — committed in the same phase that recorded the lesson.
        let (stable_prompt, volatile_prompt) = match system_prompt_override {
            // An explicit override is opaque to us: we cannot know which part of
            // it is stable, so all of it is treated as stable and nothing is
            // hoisted. Callers that want caching should not override.
            //
            // 🔴 P3-45. The volatile half used to be `String::new()` here, and
            // the MEMORY BLOCK lives in the volatile half — so an override did
            // not merely skip caching, it **silently disabled all memory
            // recall**. It is a user-facing settings field, so any user who
            // customised their system prompt lost every memory with no warning;
            // and because recipes and subagents both pass an override, it is
            // also the real reason they had none (`P3-46`) — not the missing
            // session id that `context.rs`'s comment blamed for four revisions.
            //
            // An override replaces the INSTRUCTIONS. It was never a request to
            // forget what the agent knows.
            // 🔴 P8-35/P8-36. This used to be `override_str.to_string()` — the
            // override REPLACED the stable half outright, so a fleet worker or
            // a subagent ran on its recipe text alone: no prohibitions, no git
            // rules, and none of the user's own `ARCANA.md`. The agents that
            // write files and push commits were the ones with no floor under
            // them.
            //
            // ⚠ The cache split is unchanged: everything here is still the
            // STABLE half and the memory block is still volatile and last.
            // What changed is what the stable half contains.
            Some(override_str) => (
                context_builder.build_agent_stable_prompt(working_dir, override_str),
                context_builder.memory_block(working_dir, user_message),
            ),
            // 🔴 P3-43. `user_message` is the query. It used to be the empty
            // string literal, which made recall byte-identical every turn.
            None => {
                context_builder.build_system_prompt_parts(self.pa_mode, working_dir, user_message)
            }
        };

        // What actually goes on the wire, in the order the cache requires:
        // stable first, volatile last.
        let system_prompt = if volatile_prompt.is_empty() {
            stable_prompt.clone()
        } else {
            format!("{stable_prompt}\n\n---\n\n{volatile_prompt}")
        };
        let tool_defs = self.registry.to_definitions();

        // ── P2-16 · decide the cache policy once, for the whole run ──────────
        //
        // The prefix is the same for every turn of a session — tools and system
        // prompt do not change mid-run — so this is computed once and the
        // verdict reused. Recomputing per turn would be harmless but would
        // invite the prefix and the request to be built from different
        // snapshots, which is the one way this fails silently.
        let caps = provider.capabilities(model);
        let cache_prefix = crate::cache::prefix::build(
            &tool_defs
                .iter()
                .map(|t| (t.name.clone(), t.input_schema.to_string()))
                .collect::<Vec<_>>(),
            // ⚠ The STABLE half only. Passing the joined prompt here — which is
            // what this did until the P2 audit — puts the memory block inside
            // the hashed region, so the prefix changes whenever the agent
            // learns anything and no two requests ever share a cache entry.
            &stable_prompt,
            &[],
            // The volatile half is declared, not discarded: it is what makes
            // `CachePrefix::volatile` non-empty, and it is sent after the
            // breakpoint as part of `system_prompt` above.
            std::slice::from_ref(&volatile_prompt),
        );
        // `will_cache` warns when the prefix falls short of the model's
        // minimum, which is the ONLY notice anyone gets — no provider errors
        // below it (P2-15).
        let cache_policy = if crate::cache::prefix::will_cache(&cache_prefix, &caps, model) {
            match caps.cache {
                crate::provider::usage::CacheSupport::Explicit { .. } => {
                    crate::provider::usage::CachePolicy::Explicit {
                        // One breakpoint, at the end of the system block. The
                        // four-breakpoint budget is worth spending only where
                        // there are distinct regions with different lifetimes;
                        // here there is one.
                        breakpoints: vec![0],
                        // Five minutes, not an hour: the hour costs 2.0x on the
                        // write against 1.25x, and only pays back if the prefix
                        // is re-read after the five minutes would have expired.
                        // Nothing measures that yet, so the cheaper write wins.
                        ttl: crate::provider::usage::CacheTtl::FiveMinutes,
                    }
                }
                crate::provider::usage::CacheSupport::Automatic { .. } => {
                    // The affinity key steers identical prefixes onto the same
                    // backend node. Keyed by the prefix hash rather than the
                    // session: two sessions with the same prefix SHOULD share a
                    // cache entry, and keying by session would stop them.
                    crate::provider::usage::CachePolicy::Automatic {
                        affinity_key: format!("{:x}", cache_prefix.hash),
                    }
                }
                crate::provider::usage::CacheSupport::None => {
                    crate::provider::usage::CachePolicy::Off
                }
            }
        } else {
            crate::provider::usage::CachePolicy::Off
        };

        // P2-12. The alarm. Built before the caching it watches, and consulted
        // on every turn — a monitor that is constructed and never asked is the
        // same silent nothing as the cache it was built to catch.
        let cache_monitor = crate::cache::CacheMonitor::new();

        // ── P2-27 · the model settings the user chose are actually applied ───
        //
        // `ModelConfigsDb` has a table, a set of Tauri commands, and a Settings
        // panel that writes temperature, top_p, reasoning effort and thinking
        // budget. Nothing read any of it: every request in this file was built
        // with `temperature: None, top_p: None, reasoning_effort: None,
        // thinking_budget_tokens: None`, hardcoded.
        //
        // So the sliders moved, the values persisted, and the model ran on
        // provider defaults regardless — which is worse than not offering the
        // controls, because the user reasonably believes their setting took
        // effect and attributes the unchanged behaviour to the model.
        //
        // Read once per run rather than per turn: the config cannot change
        // mid-run, and re-reading would open a database handle on every turn of
        // every agent loop.
        let model_config = crate::db::model_configs::ModelConfigsDb::open()
            .and_then(|db| db.get(model))
            .unwrap_or_else(|e| {
                // A missing config is normal — most models have none. A failure
                // to READ is not, and silently falling back to provider
                // defaults is exactly the defect being fixed here.
                warn!("Could not read the model config for {model}: {e}");
                None
            });

        if let Some(ref cfg) = model_config {
            debug!(
                "Applying saved settings for {model}: temperature={:?} top_p={:?} \
                 reasoning={:?} thinking={}",
                cfg.temperature, cfg.top_p, cfg.reasoning_effort, cfg.thinking_enabled
            );
        }

        // Add user message to history
        history.push(RequestMessage {
            role: "user".to_string(),
            content: vec![MessageContent::Text {
                text: user_message.to_string(),
            }],
        });

        let tool_executor = ToolExecutor::new(
            self.mode.clone(),
            self.pending.clone(),
            session_id.to_string(),
            working_dir.to_string(),
            self.identity.clone(),
        )
        .with_host_approval_available(self.host_approval_available)
        .with_run_id(self.worker_run_id.clone())
        .with_run_token_budget(Arc::clone(&run_token_budget))
        .with_procedural_recall(self.procedural_recall)
        // 🔴 G-3 (P3 post-phase gap hunt). `with_state` had **zero callers
        // workspace-wide**, so `self.state` was always `None` — and
        // `request_permission`'s cancellation arm reads it through
        // `.map(...).unwrap_or(false)`, which meant that arm could never fire.
        // A user cancelling while a permission dialog was open waited the full
        // 300s `PERMISSION_TIMEOUT` regardless.
        //
        // That is `§0.16 L-C12` — a helper built and wired to nothing — in
        // `P3-08`'s own work, and the count stood at seven when I wrote it.
        .with_state(self.state.clone());
        let mut tool_executor = match &self.session_mgr {
            Some(sm) => tool_executor.with_session_manager(sm.clone()),
            None => tool_executor,
        };

        // ── P8-66 · the Live Monitor's own channel ──────────────────────────
        //
        // Agent-keyed, unlike the session-keyed `agent:token` stream that feeds
        // the chat transcript. The Live Monitor asks "what is this agent doing
        // right now", and until now nothing answered: it listened for these
        // names and no Rust source sent them.
        crate::events::emit_agent_status_change(crate::events::AgentStatusChangePayload {
            agent_id: self.identity.role.clone(),
            status: "active".to_string(),
        });

        let mut turn = 0;
        let mut total_usage = Usage::default();
        // Ruflo Feature 4 — cumulative budget usage tracked across all turns
        let mut budget_status = crate::budget::BudgetStatus::default();
        // Fleet workers know their task only after claim_task executes.
        let mut active_task_id = self.task_id;
        let mut active_budget = self.task_budget.clone();
        let mut task_budget_tokens_base = 0u64;
        let mut task_budget_cost_base = 0.0f64;

        loop {
            // Fire AgentLoopStart hook at beginning of each turn
            let loop_ctx = HookContext {
                hook_type: HookType::AgentLoopStart.as_str().to_string(),
                timestamp: chrono::Utc::now(),
                session_id: Some(session_id.to_string()),
                agent_mode: Some(self.pa_mode.as_str().to_string()),
                tool_call: None,
                file_path: Some(working_dir.to_string()),
                task_id: None,
                worker_id: None,
                extra: serde_json::json!({
                    "turn": turn,
                    "total_tokens_in": total_usage.total_input(),
                    "total_tokens_out": total_usage.output,
                }),
            };
            HookRegistry::fire(&HookType::AgentLoopStart, &loop_ctx).await;

            // Ruflo Feature 4 — Budget Circuit Breaker: pre-emptive halt before spending tokens
            budget_status.turns_used += 1;
            if let Some(ref budget) = active_budget {
                if let Some(reason) = budget.check(&budget_status) {
                    let reason_str = reason.as_event_str().to_string();
                    crate::events::emit_budget_exceeded(crate::events::BudgetExceededEvent {
                        session_id: session_id.to_string(),
                        task_id: active_task_id,
                        reason: reason_str.clone(),
                        tokens_used: budget_status.tokens_used,
                        cost_usd: budget_status.cost_usd,
                        turns_used: budget_status.turns_used,
                    });
                    if let Some(tid) = active_task_id {
                        self.escalate_budget_exceeded(tid, &budget_status, &reason_str);
                    }
                    // D-1. Record BEFORE returning. This is the budget path:
                    // the run that ends here is by construction the one that
                    // spent the most, and it used to record nothing at all.
                    self.record_spend(session_id, model, &total_usage);
                    terminal.error(format!("Budget exceeded: {reason_str}"));
                    return Err(ArcanaError::BudgetExceeded(reason_str));
                }
            }

            if turn >= MAX_TOOL_TURNS {
                error!("Max tool turns ({MAX_TOOL_TURNS}) reached for session {session_id}");
                break;
            }

            // Compact if context approaching limit — uses LLM summary when possible.
            //
            // P2-57. This was a hardcoded `200_000`, ignoring
            // `ModelInfo::context_window` which every adapter already populates.
            // Gemini's window is 1M, so a Gemini conversation was compacted at
            // **20% of its real capacity** — throwing away context the model
            // could still hold, and paying for a summarisation call to do it.
            let model_context = self
                .provider
                .models()
                .iter()
                .find(|m| m.id == model)
                .map(|m| m.context_window)
                // A model the adapter does not list falls back to the smallest
                // window we ship rather than the largest: compacting early is
                // wasteful, compacting late overflows the request and fails the
                // turn outright.
                .unwrap_or(200_000);
            if CompactionEngine::needs_compaction(
                CompactionEngine::estimate_tokens(&history),
                model_context,
            ) {
                info!("Compacting conversation for session {session_id}");
                // P2-66. Compaction performs a real, billed `complete()` call,
                // and its usage used to be discarded — the response was consumed
                // for its text and dropped. Compaction fires on LONG sessions,
                // which are the expensive ones, and it summarises the whole
                // middle of the conversation, so its input side is by
                // construction most of a context window.
                //
                // Folded into the turn total so it reaches cost, the daily
                // ceiling, and analytics like any other call.
                let (compacted, compaction_usage) =
                    CompactionEngine::compact_with_summary(history, provider.as_ref(), model).await;
                history = compacted;
                if compaction_usage.total() > 0 {
                    info!(
                        "Compaction cost {} input / {} output tokens",
                        compaction_usage.total_input(),
                        compaction_usage.output
                    );
                    total_usage.add(&compaction_usage);
                    // F-23: recorded now, not only at an exit the timeout may
                    // never let us reach.
                    self.stamp_accounting(model, &total_usage);
                }
            }

            // Cancel check
            if self.state.lock_tolerant().is_cancelled(session_id) {
                info!("Agent cancelled during turn {turn}");
                cancelled_during_run = true;
                // P3-24. Children are DETACHED tokio tasks: dropping the
                // JoinHandle the parent awaits detaches rather than aborts, so
                // breaking out here used to leave every subagent this session
                // spawned running to completion -- finishing its work, emitting
                // its events, and billing the user's key for a turn nobody
                // would ever read.
                //
                // `spawn_subagent` and `spawn_parallel_subagents` both set
                // `parent_slug: ctx.session_id`, so the session id stops
                // exactly this session's children and nothing else.
                if let Some(mgr) = crate::subagent::manager::global() {
                    mgr.abort_children_of(session_id).await;
                }
                break;
            }

            // A recalled procedure belongs to this provider request only. It
            // is rendered by recall.rs, kept out of conversation history, and
            // consumed before streaming so an empty/unknown tool batch cannot
            // repeat it on a later request.
            let request_system = match tool_executor.take_procedural_block() {
                Some(procedural) => format!("{system_prompt}\n\n{procedural}"),
                None => system_prompt.clone(),
            };
            let request = CompletionRequest {
                model: model.to_string(),
                messages: history.clone(),
                system: Some(request_system),
                tools: tool_defs.clone(),
                // ── P3-25 · the clear layer ──────────────────────────────
                //
                // Layered assembly is: cached prefix -> live history -> CLEAR
                // -> compaction. The first and last landed in P2; this is the
                // one that was missing, and its POSITION is the point.
                //
                // Clearing is server-side and costs one field on a request we
                // were sending anyway. Compaction costs a whole summarisation
                // call and discards more. So the cheap layer must get its
                // chance first, which is why `CLEAR_TRIGGER_FRACTION` sits
                // below `COMPACTION_THRESHOLD` -- the band between them is
                // where clearing does its work, and compaction only fires if
                // clearing was not enough.
                //
                // Declared on every turn rather than switched on at a
                // threshold we compute: the API's own `trigger` decides, and
                // it decides on REAL input tokens rather than our ~4-chars
                // estimate.
                clear_tool_uses: crate::agent::compaction::CompactionEngine::clear_policy(
                    model_context,
                ),
                // ⚠ F-16. Clamped to what the model will actually emit.
                //
                // `ModelCapabilities::max_output` was added in P2-03 explicitly
                // to bound this, and nothing read it — so a request could ask
                // for more than the model allows and be rejected outright,
                // which reads as a provider outage rather than a
                // misconfiguration. Clamped rather than errored: the caller
                // asked for "up to", and every model we ship allows well over
                // the 8,192 default.
                max_tokens: self.output_token_limit.unwrap_or(8192).min(caps.max_output),
                // P2-27. From the user's saved settings rather than four
                // hardcoded `None`s.
                temperature: model_config.as_ref().and_then(|c| c.temperature),
                top_p: model_config.as_ref().and_then(|c| c.top_p),
                reasoning_effort: model_config
                    .as_ref()
                    .and_then(|c| c.reasoning_effort.clone()),
                // The budget applies only when thinking is actually enabled.
                // Sending a budget with thinking off is a request the provider
                // may reject outright, and it would be a setting the user did
                // not ask for.
                thinking_budget_tokens: model_config
                    .as_ref()
                    .filter(|c| c.thinking_enabled)
                    .and_then(|c| c.thinking_budget),
                cache: cache_policy.clone(),
            };

            debug!("Starting LLM stream turn={turn} session={session_id}");

            // Stream response from provider
            let mut turn_stop_reason: Option<String> = None;
            let mut invalid_tool_inputs = std::collections::HashSet::new();
            // Snapshot for the per-turn delta. `total_usage` accumulates across
            // turns, and the cache question is per-request.
            let usage_before_turn = total_usage.clone();
            let stream_result = self
                .stream_response(
                    session_id,
                    model,
                    provider.clone(),
                    request,
                    &mut total_usage,
                    &mut turn_stop_reason,
                    &mut invalid_tool_inputs,
                )
                .await;

            if let Err(ArcanaError::BudgetExceeded(reason)) = &stream_result {
                if let Some(task_id) = active_task_id {
                    crate::events::emit_budget_exceeded(crate::events::BudgetExceededEvent {
                        session_id: session_id.to_string(),
                        task_id: Some(task_id),
                        reason: reason.clone(),
                        tokens_used: budget_status.tokens_used,
                        cost_usd: budget_status.cost_usd,
                        turns_used: budget_status.turns_used,
                    });
                    self.escalate_budget_exceeded(task_id, &budget_status, reason);
                    self.record_spend(session_id, model, &total_usage);
                    terminal.error(format!("Budget exceeded: {reason}"));
                    return Err(ArcanaError::BudgetExceeded(reason.clone()));
                }
            }

            // A cancellation may arrive while the provider stream is active.
            // Check it before accepting tool calls or a clean end-turn result;
            // otherwise a final response could be reported as successful and
            // a tool side effect could start after cancellation.
            if self.state.lock_tolerant().is_cancelled(session_id) {
                cancelled_during_run = true;
                if let Some(mgr) = crate::subagent::manager::global() {
                    mgr.abort_children_of(session_id).await;
                }
                break;
            }

            // The sparkline's data. `window_tokens` is this turn's delta and
            // `total_tokens` the running total, which is exactly the pair the
            // card renders (`P8-66`).
            crate::events::emit_agent_token_update(crate::events::AgentTokenUpdatePayload {
                agent_id: self.identity.role.clone(),
                total_tokens: (total_usage.total_input() + total_usage.output) as u32,
                window_tokens: ((total_usage.total_input() + total_usage.output)
                    .saturating_sub(usage_before_turn.total_input() + usage_before_turn.output))
                    as u32,
            });

            // ── P2-12 · did this request actually read from cache? ───────────
            //
            // The only fact that separates working caching from broken caching.
            // Every other signal — the request looking right, the call
            // succeeding, the content being correct — is identical in both
            // cases. `observe` stays quiet when zero cached reads is the
            // correct outcome (first sighting, below the minimum, no cache
            // support) and logs an error only when a repeated prefix read
            // nothing.
            //
            // ⚠ D-8. Only when the stream actually SUCCEEDED.
            //
            // The monitor was consulted before `stream_result` was checked, so a
            // network failure — which reports zero usage, because no call
            // completed — looked exactly like a repeated prefix that read
            // nothing from cache. The alarm then fired with no caching fault
            // behind it, on the one instrument whose entire value is that it
            // does not cry wolf.
            if stream_result.is_ok()
                && !matches!(cache_policy, crate::provider::usage::CachePolicy::Off)
            {
                let turn_usage = total_usage.delta_since(&usage_before_turn);
                cache_monitor.observe(
                    cache_prefix.hash,
                    cache_prefix.estimated_tokens,
                    &caps,
                    &turn_usage,
                );
                // F-13. Also into the process-wide monitor, which is the only
                // one that can see the cross-RUN reuse problem — six concurrent
                // QC reviewers are six runs, and a per-run monitor computes one
                // prefix and can never observe that the other five differ.
                crate::cache::monitor::global().observe(
                    cache_prefix.hash,
                    cache_prefix.estimated_tokens,
                    &caps,
                    &turn_usage,
                );
            }

            // P2-46. A turn cut short by the token ceiling is not a finished
            // turn, and continuing the loop over a truncated assistant message
            // silently corrupts the conversation.
            //
            // P2-10. This compared the raw wire string against `"max_tokens"`,
            // which is **Anthropic's** spelling. OpenAI sends `length` and
            // Google sends `MAX_TOKENS`, so on two of the three providers a
            // truncated turn matched nothing and the loop continued over a cut
            // assistant message without a word — including over a `tool_use`
            // block whose arguments were sliced mid-JSON.
            //
            // The check now runs on the normalised classification, so it holds
            // for every provider and for any spelling a vendor adds later.
            let reason = turn_stop_reason
                .as_deref()
                .map(|raw| StopReason::parse(provider.id(), raw));

            if reason.as_ref().is_some_and(StopReason::is_truncated) {
                warn!(
                    "Turn truncated by the output ceiling (model={model}, provider={}, \
                     wire reason={:?}); the response is incomplete",
                    provider.id(),
                    turn_stop_reason.as_deref().unwrap_or("<none>")
                );
            }

            // An unrecognised stop reason is reported rather than treated as a
            // clean end. Gemini's `MALFORMED_FUNCTION_CALL` lands here: the
            // model tried to call a tool and produced arguments that do not
            // parse, which reads as neither "finished" nor "wants a tool".
            if let Some(StopReason::Other(ref raw)) = reason {
                warn!("Unrecognised stop reason {raw} (model={model}) — treated as end of turn");
            }

            // Ruflo Feature 4 — accumulate token usage into budget_status after each LLM response
            if let Ok(ref _r) = stream_result {
                budget_status.tokens_used =
                    task_budget_tokens_base.saturating_add(total_usage.total() as u64);
                // An unpriced model is a configuration error, not a $0 turn.
                // `TG-P2-02` gates that every model the UI offers is priced, so
                // reaching this branch means the model came from somewhere the
                // picker does not control. Logged at error level and NOT folded
                // into the running cost: adding 0.0 would under-report spend
                // and, once the cap lands, let an unpriced model spend past it
                // silently.
                match CostTracker::calculate(model, &total_usage) {
                    Ok(c) => budget_status.cost_usd = task_budget_cost_base + c.cost_usd,
                    Err(e) => error!("Cost unavailable for this turn: {e}"),
                }
                if let Some(tid) = active_task_id {
                    if let Ok(db) = crate::db::tasks::TasksDb::open() {
                        // P2-47. This was `let _ =`, in a function that logs
                        // loudly for every other DB failure. It is the ONLY
                        // persistence of running budget consumption, so a
                        // silent failure means a task's spend is invisible to
                        // the next process — and the per-task ceiling then
                        // enforces against a figure that stopped updating.
                        if let Err(e) = db.update_task_budget_usage(
                            tid,
                            budget_status.tokens_used,
                            budget_status.cost_usd,
                            budget_status.turns_used,
                        ) {
                            error!(
                                "Failed to persist budget usage for task #{tid}: {e}. \
                                 The ceiling for this task is now enforcing against a \
                                 stale figure."
                            );
                        }
                    }
                }
            }

            let response_content = match stream_result {
                Ok(content) => content,
                Err(e) => {
                    // Fire OnError hook
                    let error_ctx = HookContext {
                        hook_type: HookType::OnError.as_str().to_string(),
                        timestamp: chrono::Utc::now(),
                        session_id: Some(session_id.to_string()),
                        agent_mode: Some(self.pa_mode.as_str().to_string()),
                        tool_call: None,
                        file_path: Some(working_dir.to_string()),
                        task_id: None,
                        worker_id: None,
                        extra: serde_json::json!({
                            "error": e.to_string(),
                            "turn": turn,
                            "phase": "stream_response",
                        }),
                    };
                    HookRegistry::fire(&HookType::OnError, &error_ctx).await;

                    // Through the guard rather than direct, so this path
                    // and the guard cannot both fire. This is the exit the
                    // plan mistook for a fourth silent one — it always
                    // reported, and adding an emission here would have made
                    // it report twice.
                    terminal.error(e.to_string());
                    // D-1. A stream that failed mid-run still spent whatever
                    // the provider had already billed for.
                    self.record_spend(session_id, model, &total_usage);
                    return Err(e);
                }
            };

            // Collect tool calls from the response
            let tool_calls: Vec<&MessageContent> = response_content
                .iter()
                .filter(|c| matches!(c, MessageContent::ToolUse { .. }))
                .collect();

            // ── P3-07 · never append an assistant message with no content ────
            //
            // An empty `content` vec is rejected by every provider, and once it
            // is in `history` it is re-sent on **every later turn of the
            // session** — so one empty turn poisons the rest of the
            // conversation, not just itself.
            //
            // ⚠ A partial fix already landed citing this defect by name: it
            // drops empty text *blocks*. That made the empty *vec* case MORE
            // likely, not less — a turn whose only block was empty text now
            // produces `[]` rather than `[""]`. This is the missing half.
            //
            // A turn with tool calls and no prose is NOT empty: the calls are
            // its content, and dropping it would orphan every `tool_use`.
            if response_content.is_empty() {
                warn!(
                    "The model returned no content and no tool calls for session                      {session_id}; not appending an empty assistant message, which would                      be re-sent on every later turn of this session."
                );
                break;
            }

            // Add assistant message to history
            history.push(RequestMessage {
                role: "assistant".to_string(),
                content: response_content.clone(),
            });

            if tool_calls.is_empty() {
                // No tool calls — agent is done
                debug!("Agent done (no tool calls) at turn={turn}");
                break;
            }

            // Execute all tool calls with partitioned concurrency
            // Read-only tools run concurrently (max 10), write tools run serially
            let mut tool_batch: Vec<(Arc<dyn Tool>, serde_json::Value, String)> = Vec::new();
            let mut unknown_tools: Vec<(String, String)> = Vec::new(); // (tool_use_id, name)
            let mut invalid_tools: Vec<(String, String)> = Vec::new();

            for tc in &tool_calls {
                if let MessageContent::ToolUse {
                    id, name, input, ..
                } = tc
                {
                    if invalid_tool_inputs.contains(id) {
                        invalid_tools.push((id.clone(), name.clone()));
                        continue;
                    }
                    match self.registry.get(name) {
                        Some(tool) => {
                            tool_batch.push((tool, input.clone(), id.clone()));
                        }
                        None => {
                            unknown_tools.push((id.clone(), name.clone()));
                        }
                    };
                }
            }

            // Execute batch with concurrency control
            let batch_results = if !tool_batch.is_empty() {
                // Convert executor infrastructure failures into failed tool
                // results so the provider sees the same error contract as a
                // handler failure. The run can then finish through the normal
                // spend-recording and terminal path.
                match tool_executor.execute_batch(tool_batch).await {
                    Ok(r) => r,
                    Err(error) => {
                        let (code, category, safe_message) = match &error {
                            ArcanaError::Cancelled => (
                                "operation_cancelled",
                                "cancellation",
                                "Arcana cancelled this tool batch before it returned.",
                            ),
                            ArcanaError::Database(_) => (
                                "tool_executor_database_failure",
                                "infrastructure",
                                "Arcana could not access the state required to execute this tool batch.",
                            ),
                            ArcanaError::Io(_) => (
                                "tool_executor_io_failure",
                                "infrastructure",
                                "Arcana encountered an internal I/O failure before this tool batch returned.",
                            ),
                            _ => (
                                "tool_executor_failed",
                                "infrastructure",
                                "Arcana could not complete this tool call because its executor failed.",
                            ),
                        };
                        tracing::error!(
                            target: "arcana::telemetry",
                            event_type = "tool_executor_failed",
                            session_id = %session_id,
                            task_id = %crate::observability::timing::correlation_task_id(active_task_id),
                            run_id = self.worker_run_id.as_deref().unwrap_or(""),
                            error_class = match &error {
                                ArcanaError::Cancelled => "cancelled",
                                ArcanaError::Database(_) => "database",
                                ArcanaError::Io(_) => "io",
                                _ => "executor",
                            },
                            "ToolExecutor failed before returning the tool batch"
                        );
                        tool_calls
                            .iter()
                            .filter_map(|call| match call {
                                MessageContent::ToolUse { id, name, .. } => {
                                    let call_id = format!("{session_id}-{id}");
                                    let failure = super::tool_trait::ToolResult::categorized_error(
                                        code,
                                        category,
                                        safe_message,
                                        "Inspect task and workspace state before retrying; the executor cannot certify whether a side effect began.",
                                    )
                                    .with_execution_context(
                                        name,
                                        session_id,
                                        &self.identity.role,
                                        active_task_id,
                                        &call_id,
                                        self.worker_run_id.as_deref(),
                                    );
                                    Some((id.clone(), failure))
                                }
                                _ => None,
                            })
                            .collect()
                    }
                }
            } else {
                Vec::new()
            };

            // claim_task is the runtime handoff to the persisted task budget.
            // Its response was assembled from the claimed DB row, so the
            // worker cannot choose or widen these limits.
            for (tool_use_id, result) in &batch_results {
                let is_claim = tool_calls.iter().any(|call| matches!(call,
                    MessageContent::ToolUse { id, name, .. } if id == tool_use_id && name == "claim_task"));
                if !is_claim || result.is_error {
                    continue;
                }
                let Ok(payload) = serde_json::from_str::<serde_json::Value>(&result.output) else {
                    continue;
                };
                let Some(task_id) = payload.get("task_id").and_then(|value| value.as_i64()) else {
                    continue;
                };
                let Some(saved) = payload.get("budget") else {
                    continue;
                };
                let task_budget = crate::budget::TaskBudget {
                    max_tokens: saved.get("max_tokens").and_then(|value| value.as_u64()),
                    max_usd: saved.get("max_usd").and_then(|value| value.as_f64()),
                    max_turns: saved
                        .get("max_turns")
                        .and_then(|value| value.as_u64())
                        .and_then(|value| u32::try_from(value).ok()),
                };
                active_budget = Some(match active_budget.take() {
                    Some(worker_cap) => worker_cap.restricted_by(&task_budget),
                    None => task_budget,
                });
                active_task_id = Some(task_id);
                task_budget_tokens_base = saved
                    .get("tokens_used")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0);
                run_token_budget
                    .tighten_limit(active_budget.as_ref().and_then(|budget| budget.max_tokens));
                run_token_budget.raise_used_floor(
                    task_budget_tokens_base.saturating_add(total_usage.total() as u64),
                );
                tool_executor.set_task_id(active_task_id);
                task_budget_cost_base = saved
                    .get("cost_usd")
                    .and_then(|value| value.as_f64())
                    .unwrap_or(0.0);
                budget_status.tokens_used =
                    task_budget_tokens_base.saturating_add(total_usage.total() as u64);
                budget_status.cost_usd = task_budget_cost_base
                    + CostTracker::calculate(model, &total_usage)
                        .map(|cost| cost.cost_usd)
                        .unwrap_or(0.0);
                budget_status.turns_used = saved
                    .get("turns_used")
                    .and_then(|value| value.as_u64())
                    .and_then(|value| u32::try_from(value).ok())
                    .unwrap_or(0)
                    .saturating_add(turn as u32 + 1);
                if let Ok(db) = crate::db::tasks::TasksDb::open() {
                    if let Err(e) = db.update_task_budget_usage(
                        task_id,
                        budget_status.tokens_used,
                        budget_status.cost_usd,
                        budget_status.turns_used,
                    ) {
                        error!(
                            "Failed to persist saved budget activation for task #{task_id}: {e}"
                        );
                    }
                }
            }

            // Record safe, bounded operation labels after each tool batch so
            // recovery can identify the last completed operation without
            // persisting prompts, arguments, paths, or tool output.
            if let Some(run_id) = self.worker_run_id.as_deref() {
                if let Ok(runs) = crate::db::runs::RunsDb::open() {
                    for call in &tool_calls {
                        let MessageContent::ToolUse { id, name, .. } = call else {
                            continue;
                        };
                        let safe_name = if self.registry.get(name).is_some() {
                            name.as_str()
                        } else {
                            "unknown_tool"
                        };
                        let failed = batch_results
                            .iter()
                            .find(|(call_id, _)| call_id == id)
                            .map(|(_, result)| result.is_error)
                            .unwrap_or(true);
                        let progress =
                            format!("tool_{safe_name}_{}", if failed { "error" } else { "ok" });
                        if let Err(error) =
                            runs.record_worker_progress(run_id, active_task_id, &progress)
                        {
                            tracing::warn!(
                                target: "arcana::telemetry",
                                event_type = "worker_progress_persist_failed",
                                run_id = %run_id,
                                error_class = match error {
                                    crate::errors::ArcanaError::Database(_) => "database",
                                    crate::errors::ArcanaError::Io(_) => "io",
                                    _ => "agent",
                                },
                                "Could not persist worker progress"
                            );
                            break;
                        }
                    }
                }
            }

            // ── P2-61 · results follow the order of the CALLS ───────────
            //
            // This used to emit every unknown-tool error first and then the
            // batch results, so the result sequence did not parallel the
            // assistant message's tool_use sequence. Anthropic tolerates it;
            // OpenAI emits `tool` messages in exactly this vec order, so a
            // mismatch there pairs results with the wrong calls. It is also a
            // third source of message-level churn once caching is on.
            //
            // Collected into a map and then walked in `tool_calls` order, which
            // is the order the model emitted and (since P2-59) the order the
            // assistant message carries.
            let mut by_id: std::collections::HashMap<String, (String, bool)> =
                std::collections::HashMap::new();
            for (tool_use_id, name) in unknown_tools {
                let call_id = format!("{session_id}-{tool_use_id}");
                let result = super::tool_trait::ToolResult::categorized_error(
                    "unknown_tool",
                    "tool_resolution",
                    format!("The requested tool '{name}' is not registered."),
                    "Choose a tool from the registered tool list and retry without repeating side effects.",
                )
                .with_execution_context(
                    &name,
                    session_id,
                    &self.identity.role,
                    active_task_id,
                    &call_id,
                    self.worker_run_id.as_deref(),
                );
                by_id.insert(tool_use_id, (result.output, result.is_error));
            }
            for (tool_use_id, name) in invalid_tools {
                let call_id = format!("{session_id}-{tool_use_id}");
                let result = ToolResult::categorized_error(
                    "invalid_tool_arguments",
                    "argument_validation",
                    "The model supplied invalid tool arguments; this call was not executed.",
                    "Provide a valid JSON object matching the tool's declared input schema.",
                )
                .with_execution_context(
                    &name,
                    session_id,
                    &self.identity.role,
                    active_task_id,
                    &call_id,
                    self.worker_run_id.as_deref(),
                );
                by_id.insert(tool_use_id, (result.output, result.is_error));
            }
            for (tool_use_id, result) in batch_results {
                // Hint appended inline so the model reads it as part of the result.
                let content = match result.hint {
                    Some(hint) => format!("{}\n\n[HINT] {}", result.output, hint),
                    None => result.output,
                };
                by_id.insert(tool_use_id, (content, result.is_error));
            }

            let mut tool_results: Vec<MessageContent> = Vec::new();
            for tc in &tool_calls {
                if let MessageContent::ToolUse { id, name, .. } = tc {
                    if let Some((content, is_error)) = by_id.remove(id) {
                        tool_results.push(MessageContent::ToolResult {
                            tool_use_id: id.clone(),
                            name: name.clone(),
                            content,
                            is_error,
                        });
                    }
                }
            }
            // Anything left produced a result for a call the model did not make.
            // It cannot be placed in order, and dropping it silently would hide
            // a real executor bug — so it goes last, loudly.
            for (tool_use_id, (content, is_error)) in by_id {
                warn!("Tool result for an unrequested call id={tool_use_id}");
                tool_results.push(MessageContent::ToolResult {
                    tool_use_id,
                    name: String::new(),
                    content,
                    is_error,
                });
            }

            // Add tool results as user message (Anthropic convention)
            history.push(RequestMessage {
                role: "user".to_string(),
                content: tool_results,
            });

            turn += 1;
        }

        // P2-64. If nearly every request carried its own prefix, caching is
        // not merely useless here — it COSTS more, because each cache write is
        // billed above the input rate and never read back. Reported once per
        // run rather than per turn, since it is a property of the run.
        // F-13. Asked of the PROCESS monitor: within one run the prefix is
        // stable by design, so the per-run monitor can only ever answer "one
        // prefix", which says nothing.
        crate::cache::monitor::global().report_reuse(turn as u32);

        // Fire AgentLoopEnd hook when loop completes
        let loop_end_ctx = HookContext {
            hook_type: HookType::AgentLoopEnd.as_str().to_string(),
            timestamp: chrono::Utc::now(),
            session_id: Some(session_id.to_string()),
            agent_mode: Some(self.pa_mode.as_str().to_string()),
            tool_call: None,
            file_path: Some(working_dir.to_string()),
            task_id: None,
            worker_id: None,
            extra: serde_json::json!({
                "final_turn": turn,
                "total_tokens_in": total_usage.total_input(),
                "total_tokens_out": total_usage.output,
            }),
        };
        HookRegistry::fire(&HookType::AgentLoopEnd, &loop_end_ctx).await;

        // ── P2-52 · a turn that did work must not report zero usage ──────
        //
        // Eight separate `.unwrap_or(0)` guards cover the usage fields across
        // three adapters, so a provider renaming a field degrades silently to
        // "this call was free" — no error, no log line, and with the spend cap
        // now live, a zero-cost turn passes every ceiling.
        //
        // Checked here as the INVARIANT rather than at each of the eight sites:
        // what matters is not which field was missing, it is that a turn which
        // produced output reported nothing. That is one condition, and it stays
        // true if a ninth extraction site is added.
        // ⚠ F-6 (P2 post-phase audit). This read `turn > 0`, and `turn` counts
        // TOOL-EXECUTING iterations — it is 0 for a single question answered
        // without tools, which is the commonest turn there is. So the alarm
        // could not fire for the case it most needed to cover.
        //
        // The condition is "the model produced something and reported nothing",
        // so it is asked of the content, not of the loop counter.
        if total_usage.total() == 0 && !history.is_empty() {
            error!(
                "Provider '{}' returned content but reported ZERO tokens for model \
                 '{model}'. Cost and every spend ceiling are blind to this turn. \
                 The usage field names have most likely changed.",
                provider.id()
            );
        }

        // Record cost in app state.
        //
        // ⚠ D-1 (P2 post-phase audit). This used to live HERE and only here —
        // after the turn loop — so it was reached only when the loop exited via
        // `break`. Three paths `return` out of the loop body instead, and each
        // discarded the entire run's accumulated usage:
        //
        //   * the per-task budget tripping, which by construction is the most
        //     expensive run there is;
        //   * a stream error;
        //   * a tool-batch failure.
        //
        // So an eighteen-turn run that died on turn nineteen recorded **$0.00**
        // against the daily ceiling. The fleet path was unaffected, because
        // `recipes/runner.rs` records per-turn inside its own loop — which is
        // probably why nobody noticed.
        //
        // `record_spend` is now called on every exit path.
        let cost_usd = self.record_spend(session_id, model, &total_usage);

        // P2-49. `emit_cost_update` had exactly ONE caller — a worker-tick
        // path that no longer runs — so the entire interactive agent path was
        // invisible to every cost surface that listens for it, including the
        // only budget warning the UI had. It emitted `agent_done` instead,
        // which carries no cost.
        // (The cost event is emitted by `record_spend`, on every exit path
        // rather than only this one.)

        if cancelled_during_run {
            turn_timer.set_outcome("cancelled");
            terminal.error("The run was cancelled.");
            return Err(ArcanaError::Cancelled);
        }

        terminal.done(AgentDonePayload {
            session_id: session_id.to_string(),
            message_id: Uuid::new_v4().to_string(),
            tokens_in: total_usage.total_input(),
            tokens_out: total_usage.output,
            cost_usd,
        });

        turn_timer.succeed();
        Ok(history)
    }

    /// Stream a single LLM response, emitting token events to the frontend.
    /// Returns the fully assembled response content blocks.
    /// `stop_reason` is an out-parameter because the caller must be able to
    /// tell a `max_tokens` truncation from a clean finish (P2-46). It used to be
    /// destructured away with `..`, so the two were literally the same code
    /// path and silent truncation was invisible.
    #[tracing::instrument(
        target = "arcana::telemetry",
        skip_all,
        fields(event_type = "trace_span", operation = "provider_stream", session_id = %session_id, model = %model)
    )]
    async fn stream_response(
        &self,
        session_id: &str,
        model: &str,
        provider: Arc<dyn Provider>,
        request: CompletionRequest,
        total_usage: &mut Usage,
        stop_reason: &mut Option<String>,
        invalid_tool_inputs: &mut std::collections::HashSet<String>,
    ) -> Result<Vec<MessageContent>> {
        let mut stream_timer =
            crate::observability::timing::OperationTimer::start("provider_stream")
                .with_session_id(session_id);
        let usage_before = *total_usage;
        let mut provider_call = ProviderCallTelemetry::new(model);
        // P2-58: `model` was `_model` — a parameter passed live and never read,
        // sitting exactly where per-model streaming behaviour would go.
        debug!("stream_response: model={model}");
        use futures::StreamExt;

        // ── P3-09 · the retry policy, applied where retrying is SAFE ─────────
        //
        // Only the **establishment** of the stream is retried. If `stream()`
        // returns `Err`, no chunk has been emitted, so a second attempt cannot
        // duplicate output. Retrying a stream that has already produced tokens
        // would re-emit them, which is worse than the failure.
        //
        // The classification comes from the error the adapter attached
        // (`ErrorClass::classify`), not from matching its text — the whole
        // defect `P3-09` fixes is that the classification was computed,
        // stringified and discarded, so any retry loop would have had to parse
        // `[Retryable]` back out of a message.
        //
        // ⚠ `QuotaExhausted` is never retried: a spend-cap 429 carries no
        // retry-after and never succeeds, so retrying it is a spend loop
        // against a ceiling that has already fired.
        let request_for_retry = request.clone();
        let mut stream_attempts = 0u32;
        let last_error_class = Arc::new(Mutex::new(None));
        let classify_failure = |e: &ArcanaError| {
            // Preserve the adapter's existing bounded class tags. Never save
            // its response text in analytics.
            let text = e.to_string();
            let class = if text.contains("[QuotaExhausted]") {
                crate::provider::usage::ErrorClass::QuotaExhausted
            } else if text.contains("[Overloaded]") {
                crate::provider::usage::ErrorClass::Overloaded
            } else if text.contains("[Retryable]") {
                crate::provider::usage::ErrorClass::Retryable
            } else {
                crate::provider::usage::ErrorClass::Fatal
            };
            *last_error_class
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(class);
            class
        };
        let stream_result = crate::provider::retry::with_retry(
            || {
                stream_attempts = stream_attempts.saturating_add(1);
                provider_call.attempts = stream_attempts;
                let attempt = stream_attempts;
                let p = provider.clone();
                let r = request_for_retry.clone();
                async move {
                    let started = std::time::Instant::now();
                    let result = p.stream(r).await;
                    tracing::info!(
                        target: "arcana::telemetry",
                        event_type = "provider_attempt",
                        provider = crate::provider::routing::provider_for_model(&model).unwrap_or("unknown"),
                        model = %model,
                        attempt,
                        duration_ns = started.elapsed().as_nanos() as u64,
                        outcome = if result.is_ok() { "stream_opened" } else { "error" },
                        "Provider stream establishment attempt completed"
                    );
                    result
                }
            },
            &classify_failure,
        )
        .await;

        // `api_failures` already has a dashboard reader but had no production
        // writer. Persist one bounded aggregate per provider call that saw a
        // failed attempt, retaining retry count and whether the call recovered.
        // Never persist the provider body: it can contain sensitive request
        // details, and the structured log already retains the stable class.
        let record_failure = |class: crate::provider::usage::ErrorClass, resolved: bool| {
            if let Some(analytics) = crate::analytics::try_global() {
                let outcome = if resolved {
                    "recovered after retry"
                } else {
                    "failed after retries"
                };
                let message = format!(
                    "Provider request {outcome} ({} attempt(s), class {}).",
                    stream_attempts,
                    class.as_str()
                );
                if analytics
                    .collector()
                    .record_api_failure_outcome(
                        provider.id().to_string(),
                        class.as_str().to_string(),
                        message,
                        None,
                        stream_attempts.saturating_sub(1) as i32,
                        resolved,
                    )
                    .is_err()
                {
                    tracing::error!(
                        target: "arcana::telemetry",
                        event_type = "provider_failure_persist_failed",
                        provider = provider.id(),
                        error_class = class.as_str(),
                        "Could not persist provider failure analytics"
                    );
                }
            }
        };
        let mut stream = match stream_result {
            Ok(stream) => stream,
            Err(error) => {
                record_failure(classify_failure(&error), false);
                return Err(error);
            }
        };

        // ── Ordering (P2-59, P2-60) ──────────────────────────────────────
        //
        // Both maps are keyed by the wire's own **content block index** and are
        // `BTreeMap`s, so assembly below walks them in the order the model
        // actually emitted.
        //
        // This was a `HashMap` keyed by tool-use UUID. `std::collections::HashMap`
        // iterates in an order derived from a per-process `RandomState` seed, so
        // when a model emitted two or more parallel tool calls the block order
        // written into `history` was **arbitrary and differed between runs of the
        // same binary** — and that reordered assistant message is then replayed
        // on every subsequent turn. It corrupts the transcript relative to the
        // wire, and once caching is on it is a messages-level invalidator that
        // looks exactly like "caching doesn't work" while the prefix is perfectly
        // stable: the hardest possible bug to attribute.
        //
        // Text was separately buffered into one string and pushed **first**,
        // inverting any commentary the model emitted after a tool call.
        let mut text_by_block: std::collections::BTreeMap<u64, String> =
            std::collections::BTreeMap::new();
        let mut thinking_by_block: std::collections::BTreeMap<u64, (String, Option<String>)> =
            std::collections::BTreeMap::new();
        let xml_thinking_fallback = !crate::provider::catalogue::capabilities_for(model).reasoning;
        let mut xml_detectors: std::collections::BTreeMap<
            u64,
            crate::provider::streaming::XmlThinkingDetector,
        > = std::collections::BTreeMap::new();
        // block index → (tool_use_id, tool_name, partial_input_json)
        // (id, name, accumulated-json, opaque-echo-token)
        //
        // The fourth element is Gemini'"'"'s `thoughtSignature`, which must be
        // echoed back on the next request or the turn is rejected with a 400.
        let mut tool_use_map: std::collections::BTreeMap<
            u64,
            (String, String, String, Option<String>),
        > = std::collections::BTreeMap::new();
        let mut content_blocks: Vec<MessageContent> = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => {
                    record_failure(classify_failure(&error), false);
                    return Err(error);
                }
            };
            match chunk {
                StreamChunk::TextDelta { text, block_index } => {
                    if xml_thinking_fallback {
                        let detector = xml_detectors.entry(block_index).or_default();
                        for segment in detector.push(&text) {
                            match segment {
                                crate::provider::streaming::ThinkingSegment::Content(text) => {
                                    emit_token(TokenPayload {
                                        session_id: session_id.to_string(),
                                        text: text.clone(),
                                    });
                                    text_by_block
                                        .entry(block_index)
                                        .or_default()
                                        .push_str(&text);
                                }
                                crate::provider::streaming::ThinkingSegment::Thinking(text) => {
                                    emit_thinking(ThinkingPayload {
                                        session_id: session_id.to_string(),
                                        text,
                                    });
                                }
                            }
                        }
                    } else {
                        emit_token(TokenPayload {
                            session_id: session_id.to_string(),
                            text: text.clone(),
                        });
                        text_by_block
                            .entry(block_index)
                            .or_default()
                            .push_str(&text);
                    }
                }
                StreamChunk::ThinkingDelta { text, block_index } => {
                    emit_thinking(ThinkingPayload {
                        session_id: session_id.to_string(),
                        text: text.clone(),
                    });
                    thinking_by_block
                        .entry(block_index)
                        .or_default()
                        .0
                        .push_str(&text);
                }
                StreamChunk::ThinkingSignature {
                    signature,
                    block_index,
                } => {
                    thinking_by_block.entry(block_index).or_default().1 = Some(signature);
                }
                StreamChunk::ToolUseStart {
                    id,
                    name,
                    block_index,
                    opaque,
                } => {
                    debug!("Tool use start: name={name} id={id} block_index={block_index}");
                    // Gemini's streamed candidate may repeat a function-call
                    // part as it completes the response. A later update can
                    // omit the already-issued thought signature; replacing the
                    // whole entry would erase it before the next request.
                    // Retain the first provider token observed for this part.
                    let opaque = opaque.or_else(|| {
                        tool_use_map
                            .get(&block_index)
                            .and_then(|(_, _, _, previous)| previous.clone())
                    });
                    tool_use_map.insert(block_index, (id, name, String::new(), opaque));
                }
                StreamChunk::ToolInputDelta {
                    block_index,
                    partial_json,
                } => {
                    // Keyed directly by block index now, so the id->index
                    // correlation map is gone: it existed only because the
                    // accumulator was keyed by id, which is what made ordering
                    // depend on hash iteration in the first place.
                    if let Some(entry) = tool_use_map.get_mut(&block_index) {
                        entry.2.push_str(&partial_json);
                    } else {
                        debug!("ToolInputDelta: no tool started at block_index={block_index}");
                    }
                }
                StreamChunk::MessageDelta {
                    stop_reason: reason,
                    usage,
                } => {
                    total_usage.add(&usage);
                    provider_call.update_usage(total_usage, usage_before);
                    // F-23: see `stamp_accounting`. A dropped future runs no
                    // exit path, so the accounting must already be written.
                    self.stamp_accounting(model, &total_usage);
                    // `message_start` opens the message and carries no stop
                    // reason; only a non-empty value is a real one.
                    if !reason.is_empty() {
                        *stop_reason = Some(reason);
                    }
                }
                StreamChunk::Done => break,
            }
        }

        for (block_index, detector) in &mut xml_detectors {
            if let Some(segment) = detector.finish() {
                match segment {
                    crate::provider::streaming::ThinkingSegment::Content(text) => {
                        emit_token(TokenPayload {
                            session_id: session_id.to_string(),
                            text: text.clone(),
                        });
                        text_by_block
                            .entry(*block_index)
                            .or_default()
                            .push_str(&text);
                    }
                    crate::provider::streaming::ThinkingSegment::Thinking(text) => {
                        emit_thinking(ThinkingPayload {
                            session_id: session_id.to_string(),
                            text,
                        });
                    }
                }
            }
        }

        for (block_index, segment) in finish_xml_thinking(&mut xml_detectors) {
            match segment {
                crate::provider::streaming::ThinkingSegment::Content(text) => {
                    emit_token(TokenPayload {
                        session_id: session_id.to_string(),
                        text: text.clone(),
                    });
                    text_by_block
                        .entry(block_index)
                        .or_default()
                        .push_str(&text);
                }
                crate::provider::streaming::ThinkingSegment::Thinking(text) => {
                    emit_thinking(ThinkingPayload {
                        session_id: session_id.to_string(),
                        text,
                    });
                }
            }
        }

        if thinking_by_block
            .values()
            .any(|(_, signature)| signature.as_deref().is_none_or(str::is_empty))
        {
            record_failure(crate::provider::usage::ErrorClass::Fatal, false);
            return Err(crate::errors::ArcanaError::Provider(
                "Anthropic thinking block has no signature; refusing an unreplayable turn".into(),
            ));
        }
        for (id, _, raw_input, _) in tool_use_map.values() {
            // An omitted argument delta represents the intentional no-argument
            // object. A nonempty malformed/non-object delta must never reach
            // a handler as its defaults. Keep only IDs, never parser/source text.
            if !raw_input.trim().is_empty()
                && !serde_json::from_str::<serde_json::Value>(raw_input)
                    .is_ok_and(|value| value.is_object())
            {
                invalid_tool_inputs.insert(id.clone());
            }
        }
        content_blocks.extend(assemble_with_thinking_in_wire_order(
            text_by_block,
            thinking_by_block,
            tool_use_map,
        ));

        if stream_attempts > 1 {
            let class = last_error_class
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .unwrap_or(crate::provider::usage::ErrorClass::Fatal);
            record_failure(class, true);
        }

        stream_timer.succeed();
        provider_call.result_blocks = content_blocks.len();
        provider_call.outcome = "success";

        Ok(content_blocks)
    }

    // ── Ruflo Feature 4 — Budget escalation helper ────────────────────────────
    // Creates a WorkerRequest of type report_issue and transitions the task to
    // needs_info so the Supervisor can decide to increase budget, split the task,
    // or reassign. Infallible — logs on DB failure so the budget halt still works.
    fn escalate_budget_exceeded(
        &self,
        task_id: i64,
        status: &crate::budget::BudgetStatus,
        reason_str: &str,
    ) {
        let budget_limit_str = self
            .task_budget
            .as_ref()
            .map(|b| {
                format!(
                    "{} tokens / ${:.2} USD / {} turns",
                    b.max_tokens
                        .map_or("unlimited".to_string(), |t| t.to_string()),
                    b.max_usd.map_or(0.0, |u| u),
                    b.max_turns.map_or(0, |t| t),
                )
            })
            .unwrap_or_else(|| "unknown".to_string());

        let issue_description = format!(
            "Budget exceeded on task #{task_id}: halted at {} tokens / ${:.4} USD after {} turns. \
             Budget limit: {budget_limit_str}. Task needs budget increase or scope reduction.",
            status.tokens_used, status.cost_usd, status.turns_used,
        );

        let payload = serde_json::json!({
            "issue_description": issue_description,
            "severity": "high",
            "budget_reason": reason_str,
            "tokens_used": status.tokens_used,
            "cost_usd": status.cost_usd,
            "turns_used": status.turns_used,
        });

        let req_input = crate::db::tasks::CreateWorkerRequestInput {
            worker_slug: "engine".to_string(),
            task_id: Some(task_id),
            request_type: "report_issue".to_string(),
            payload: payload.to_string(),
        };

        match crate::db::tasks::TasksDb::open() {
            Ok(db) => {
                if let Err(e) = db.create_worker_request(&req_input) {
                    error!("[budget] failed to create worker request for task {task_id}: {e}");
                }
                if let Err(e) = db.mark_task_budget_exceeded(task_id, reason_str) {
                    error!("[budget] failed to mark task {task_id} budget-exceeded: {e}");
                }
                crate::events::emit_task_changed(crate::events::TaskChangedPayload {
                    task_id: Some(task_id),
                    kind: "budget_exceeded".to_string(),
                    status: Some("needs_info".to_string()),
                    assigned_worker: None,
                });
            }
            Err(e) => {
                error!("[budget] failed to open tasks DB for escalation on task {task_id}: {e}");
            }
        }
    }
}

impl AgentEngine {
    /// Stamp the running accounting so it survives a **dropped** future.
    ///
    /// F-23 (post-phase claims audit). `record_spend` runs only at the turn
    /// loop's exit paths, and `tokio::time::timeout` DROPS the future rather
    /// than letting it return -- so a subagent that hit its timeout ran none of
    /// them, `last_run_accounting()` stayed `None`, and the result reported
    /// $0.00. That is precisely the case `P3-62` claimed to have fixed: the
    /// runs that die late are the expensive ones.
    ///
    /// Called after every accumulation, so whatever has been spent so far is
    /// already recorded when the future is dropped mid-turn.
    fn stamp_accounting(&self, model: &str, total: &Usage) {
        if total.total() == 0 {
            return;
        }
        let cost_usd = CostTracker::calculate(model, total)
            .map(|c| c.cost_usd)
            .unwrap_or(0.0);
        let accounting = RunAccounting {
            cost_usd,
            tokens_in: total.total_input(),
            tokens_out: total.output,
        };
        match self.last_run.lock() {
            Ok(mut g) => *g = Some(accounting),
            Err(p) => *p.into_inner() = Some(accounting),
        }
    }

    /// Fold one run's usage into the day's spend.
    ///
    /// ⚠ **Must be called on every path out of the turn loop**, including the
    /// error paths — especially those, since a run that ended because it ran out
    /// of budget is by definition the expensive one. `D-1` in the P2 post-phase
    /// audit was exactly this: `add_cost` sat after the loop and three `return`s
    /// jumped over it.
    ///
    /// Safe to call with an empty `Usage`: it is a no-op then, so an early exit
    /// before any turn costs nothing and records nothing.
    fn record_spend(&self, session_id: &str, model: &str, total_usage: &Usage) -> f64 {
        // ── P8-66 · the run is over, whatever else is true ─────────────────
        //
        // BEFORE the zero-usage early return, and inside this function rather
        // than at its four call sites. This function's own docstring is that it
        // runs on every exit path, so putting the transition here makes "the
        // Live Monitor is told the agent stopped" true by construction — the
        // same reasoning that places the procedural seam at the top of
        // `execute_batch`. Four call sites would be four chances to forget, and
        // the failure would be a card left showing ACTIVE forever.
        crate::events::emit_agent_status_change(crate::events::AgentStatusChangePayload {
            agent_id: self.identity.role.clone(),
            status: "idle".to_string(),
        });

        if total_usage.total() == 0 {
            return 0.0;
        }

        // ⚠ D-7. An unpriced model used to record `0.0` here, which is not a
        // neutral default — the daily ceiling is denominated in dollars, so a
        // $0 record means the call is INVISIBLE to it and the model runs
        // uncapped. The catalogue gate proves every model we *offer* is priced;
        // it cannot prove that every model reaching this line came from the
        // catalogue, and several do not (worker rows, recipe YAML, subagent
        // config all set a model without consulting it).
        //
        // Tokens are still recorded either way — they are provider-reported
        // fact. What changed is that the dollar figure is no longer quietly
        // zero: the ceiling is told it cannot account for this call.
        let cost_usd = match CostTracker::calculate(model, total_usage) {
            Ok(c) => c.cost_usd,
            Err(e) => {
                error!(
                    "UNMETERED SPEND: model '{model}' has no verified price ({e}), so this \
                     run's {} input / {} output tokens cannot be costed and the daily \
                     ceiling cannot see them. This model should not be reachable — it is \
                     not in the catalogue.",
                    total_usage.total_input(),
                    total_usage.output
                );
                0.0
            }
        };

        // Persist token usage alongside spend on every exit path. Keeping this
        // here (rather than only after a successful turn loop) means budget
        // halts, cancellations, and provider/tool errors remain visible to the
        // analytics reader with the provider-reported token categories.
        let analytics_provider = crate::provider::routing::provider_for_analytics(model);
        if let Err(error) = crate::analytics::global().collector().record_token_usage(
            session_id.to_string(),
            // This label is part of the existing dashboard aggregation
            // contract; role-specific provider metrics are emitted separately.
            "mode1_agent".to_string(),
            analytics_provider.to_string(),
            model.to_string(),
            total_usage.total_input() as i32,
            total_usage.output as i32,
            (total_usage.reasoning > 0).then_some(total_usage.reasoning as i32),
            cost_usd,
            "agent_session".to_string(),
        ) {
            error!("[analytics] token usage persistence failed for run: {error}");
        }

        let total_today = {
            let mut state = self.state.lock_tolerant();
            state.add_cost(cost_usd, total_usage.total_input(), total_usage.output);
            state.cost_today_usd
        };

        // P2-49. Emitted here rather than at the single happy-path exit, so the
        // cost surface updates when a run ends in an ERROR too — which is
        // exactly when the user most wants to see what it spent.
        crate::events::emit_cost_update(crate::events::CostUpdatePayload {
            session_id: session_id.to_string(),
            tokens_in: total_usage.total_input(),
            tokens_out: total_usage.output,
            cost_usd,
            total_today,
        });

        // P3-62. Recorded here rather than returned up through `run`, because
        // `record_spend` is called on EVERY exit path out of the turn loop
        // (including the error paths — D-1) and `run` returns history, not
        // accounting. A caller that needs the number reads it after awaiting;
        // `SubagentResult` is the one that needs it, and the QC hourly ceiling
        // is computed from that.
        {
            let accounting = RunAccounting {
                cost_usd,
                tokens_in: total_usage.total_input(),
                tokens_out: total_usage.output,
            };
            match self.last_run.lock() {
                Ok(mut g) => *g = Some(accounting),
                Err(p) => *p.into_inner() = Some(accounting),
            }
        }

        cost_usd
    }
}

/// Merge buffered text and tool calls back into the order the model emitted.
///
/// Extracted from `stream_response` so the ordering can be tested without a
/// live provider — the defect it fixes (P2-59) was **non-deterministic across
/// processes**, which is exactly the kind that survives manual checking.
///
/// Both inputs are keyed by the wire's content-block index and are `BTreeMap`s,
/// so merging them is simply walking the union in key order.
fn assemble_in_wire_order(
    text_by_block: std::collections::BTreeMap<u64, String>,
    tool_use_map: std::collections::BTreeMap<u64, (String, String, String, Option<String>)>,
) -> Vec<MessageContent> {
    assemble_with_thinking_in_wire_order(text_by_block, Default::default(), tool_use_map)
}

fn finish_xml_thinking(
    detectors: &mut std::collections::BTreeMap<
        u64,
        crate::provider::streaming::XmlThinkingDetector,
    >,
) -> Vec<(u64, crate::provider::streaming::ThinkingSegment)> {
    detectors
        .iter_mut()
        .filter_map(|(block_index, detector)| {
            detector.finish().map(|segment| (*block_index, segment))
        })
        .collect()
}

fn assemble_with_thinking_in_wire_order(
    text_by_block: std::collections::BTreeMap<u64, String>,
    thinking_by_block: std::collections::BTreeMap<u64, (String, Option<String>)>,
    tool_use_map: std::collections::BTreeMap<u64, (String, String, String, Option<String>)>,
) -> Vec<MessageContent> {
    let mut ordered: std::collections::BTreeMap<u64, MessageContent> =
        std::collections::BTreeMap::new();

    for (idx, (text, signature)) in thinking_by_block {
        ordered.insert(idx, MessageContent::Thinking { text, signature });
    }

    for (idx, text) in text_by_block {
        // An empty text block is dropped rather than emitted: an empty content
        // block poisons every later send in the session (P3-07's defect).
        if !text.is_empty() {
            ordered.insert(idx, MessageContent::Text { text });
        }
    }
    for (idx, (id, name, json_str, opaque)) in tool_use_map {
        // A tool call whose arguments never arrived becomes `{}` rather than
        // failing the turn — the provider said the model wanted this tool, and
        // dropping the call entirely would leave the model waiting for a result
        // it never gets.
        // Invalid IDs are rejected by the stream consumer before dispatch.
        // Keep their replay placeholder wire-safe, including non-object JSON.
        let input = serde_json::from_str::<serde_json::Value>(&json_str)
            .ok()
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
        ordered.insert(
            idx,
            MessageContent::ToolUse {
                id,
                name,
                input,
                // Carried from the stream. Gemini rejects a replayed call whose
                // signature is missing, so losing it here would break the
                // conversation one turn later — at the point where the model
                // has already done the work.
                opaque,
            },
        );
    }

    ordered.into_values().collect()
}

#[cfg(test)]
mod wire_order_tests {
    use super::*;
    use crate::provider::anthropic::AnthropicProvider;
    use crate::provider::streaming::parse_sse_line;
    use crate::provider::types::{CompletionRequest, RequestMessage};
    use std::collections::BTreeMap;

    fn tool(id: &str, name: &str, json: &str) -> (String, String, String, Option<String>) {
        (id.to_string(), name.to_string(), json.to_string(), None)
    }

    #[test]
    fn stream_end_flushes_an_incomplete_xml_delimiter_into_visible_history() {
        let mut detector = crate::provider::streaming::XmlThinkingDetector::default();
        assert_eq!(
            detector.push("visible <think"),
            vec![crate::provider::streaming::ThinkingSegment::Content(
                "visible ".into()
            )]
        );
        let mut detectors = BTreeMap::from([(3, detector)]);
        let flushed = finish_xml_thinking(&mut detectors);
        assert_eq!(
            flushed,
            vec![(
                3,
                crate::provider::streaming::ThinkingSegment::Content("<think".into())
            )]
        );

        let mut visible = BTreeMap::from([(3, "visible ".to_string())]);
        for (block, segment) in flushed {
            if let crate::provider::streaming::ThinkingSegment::Content(text) = segment {
                visible.entry(block).or_default().push_str(&text);
            }
        }
        assert!(matches!(
            assemble_in_wire_order(visible, BTreeMap::new()).as_slice(),
            [MessageContent::Text { text }] if text == "visible <think"
        ));
    }

    /// A tool call carrying a provider echo token (Gemini'"'"'s thoughtSignature).
    fn tool_with_signature(
        id: &str,
        name: &str,
        json: &str,
        sig: &str,
    ) -> (String, String, String, Option<String>) {
        (
            id.to_string(),
            name.to_string(),
            json.to_string(),
            Some(sig.to_string()),
        )
    }

    struct FixtureStreamProvider {
        chunks: Vec<StreamChunk>,
    }

    #[async_trait::async_trait]
    impl crate::provider::traits::Provider for FixtureStreamProvider {
        fn id(&self) -> &str {
            "anthropic"
        }
        fn name(&self) -> &str {
            "Fixture"
        }
        fn models(&self) -> Vec<crate::provider::types::ModelInfo> {
            Vec::new()
        }

        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> crate::errors::Result<crate::provider::types::CompletionResponse> {
            unreachable!("streaming tests must use the real stream consumer")
        }

        async fn stream(
            &self,
            _request: CompletionRequest,
        ) -> crate::errors::Result<crate::provider::traits::StreamResult> {
            Ok(Box::pin(futures::stream::iter(
                self.chunks.clone().into_iter().map(Ok),
            )))
        }

        async fn test_connection(
            &self,
        ) -> crate::errors::Result<crate::provider::types::ProviderStatus> {
            Ok(crate::provider::types::ProviderStatus::Connected)
        }
    }

    fn fixture_request() -> CompletionRequest {
        CompletionRequest {
            model: "claude-sonnet-5".into(),
            messages: vec![],
            system: None,
            tools: vec![],
            max_tokens: 128,
            temperature: None,
            top_p: None,
            reasoning_effort: None,
            thinking_budget_tokens: Some(64),
            cache: Default::default(),
            clear_tool_uses: None,
        }
    }

    fn fixture_engine(chunks: Vec<StreamChunk>) -> AgentEngine {
        let provider = Arc::new(FixtureStreamProvider { chunks });
        AgentEngine::new(
            provider,
            super::super::permission::PermissionMode::Autonomous,
            super::super::PAMode::FastDevelopment,
            Arc::new(Mutex::new(crate::state::AppState::default())),
            super::super::permission::new_pending_permissions(),
            super::super::tool_trait::AgentIdentity::supervisor(),
        )
    }

    #[tokio::test]
    async fn production_stream_consumer_replays_signed_thinking_before_a_tool_result() {
        let engine = fixture_engine(vec![
            StreamChunk::ThinkingDelta {
                text: "consider file".into(),
                block_index: 0,
            },
            StreamChunk::ThinkingSignature {
                signature: "signed-opaque".into(),
                block_index: 0,
            },
            StreamChunk::ToolUseStart {
                id: "toolu_1".into(),
                name: "read_file".into(),
                block_index: 1,
                opaque: None,
            },
            StreamChunk::ToolInputDelta {
                block_index: 1,
                partial_json: r#"{"path":"package.json"}"#.into(),
            },
            StreamChunk::MessageDelta {
                stop_reason: "tool_use".into(),
                usage: Usage::default(),
            },
            StreamChunk::Done,
        ]);
        let mut usage = Usage::default();
        let mut stop_reason = None;
        let assistant_content = engine
            .stream_response(
                "p8-anthropic-session",
                "claude-sonnet-5",
                engine.provider.clone(),
                fixture_request(),
                &mut usage,
                &mut stop_reason,
                &mut Default::default(),
            )
            .await
            .expect("production stream consumer should assemble the assistant turn");
        assert_eq!(stop_reason.as_deref(), Some("tool_use"));

        let follow_up = CompletionRequest {
            messages: vec![
                RequestMessage {
                    role: "assistant".into(),
                    content: assistant_content,
                },
                RequestMessage {
                    role: "user".into(),
                    content: vec![MessageContent::ToolResult {
                        tool_use_id: "toolu_1".into(),
                        name: "read_file".into(),
                        content: "{\"name\":\"arcana\"}".into(),
                        is_error: false,
                    }],
                },
            ],
            ..fixture_request()
        };
        let body = AnthropicProvider::new("unused".into())
            .build_checked_body_for_test(&follow_up, true)
            .expect("stream-produced signed history must serialize for the next request");
        assert_eq!(
            body["messages"][0]["content"][0],
            serde_json::json!({
                "type": "thinking", "thinking": "consider file", "signature": "signed-opaque",
            })
        );
        assert_eq!(body["messages"][0]["content"][1]["type"], "tool_use");
        assert_eq!(body["messages"][1]["content"][0]["tool_use_id"], "toolu_1");
    }

    #[tokio::test]
    async fn repeated_gemini_stream_part_does_not_erase_its_thought_signature() {
        let mut chunks = crate::provider::google::parse_google_sse_line(
            r#"data: {"candidates":[{"content":{"parts":[{"functionCall":{"name":"list_worker_health","args":{"include_paused":true}},"thoughtSignature":"provider-issued-signature"}]}}]}"#,
        )
        .expect("parse first Gemini stream update");
        // Streaming updates for one part can repeat the call without
        // repeating metadata that was present in an earlier update.
        chunks.extend(
            crate::provider::google::parse_google_sse_line(
                r#"data: {"candidates":[{"content":{"parts":[{"functionCall":{"name":"list_worker_health","args":{"include_paused":true}}}]}}]}"#,
            )
            .expect("parse repeated Gemini stream update"),
        );
        chunks.extend([
            StreamChunk::MessageDelta {
                stop_reason: "tool_use".into(),
                usage: Usage::default(),
            },
            StreamChunk::Done,
        ]);
        let engine = fixture_engine(chunks);
        let mut usage = Usage::default();
        let mut stop_reason = None;
        let assistant_content = engine
            .stream_response(
                "gemini-signature-session",
                "gemini-3.8-flash",
                engine.provider.clone(),
                fixture_request(),
                &mut usage,
                &mut stop_reason,
                &mut Default::default(),
            )
            .await
            .expect("production stream consumer should assemble the function call");

        let request = CompletionRequest {
            model: "gemini-3.8-flash".into(),
            messages: vec![
                RequestMessage {
                    role: "assistant".into(),
                    content: assistant_content,
                },
                RequestMessage {
                    role: "user".into(),
                    content: vec![MessageContent::ToolResult {
                        tool_use_id: "gemini-list_worker_health-6".into(),
                        name: "list_worker_health".into(),
                        content: "[]".into(),
                        is_error: false,
                    }],
                },
            ],
            ..fixture_request()
        };
        let body = crate::provider::google::GoogleProvider::new("unused".into())
            .build_body_for_test(&request, true);

        assert_eq!(
            body["contents"][0]["parts"][0]["thoughtSignature"], "provider-issued-signature",
            "a repeated stream update must not erase a provider replay token"
        );
        assert_eq!(
            body["contents"][1]["parts"][0]["functionResponse"]["name"],
            "list_worker_health"
        );
    }

    #[tokio::test]
    async fn production_stream_consumer_keeps_xml_reasoning_private_and_flushes_visible_tail() {
        let engine = fixture_engine(vec![
            StreamChunk::TextDelta {
                text: "safe <thi".into(),
                block_index: 0,
            },
            StreamChunk::TextDelta {
                text: "nking>secret</thinking> visible <think".into(),
                block_index: 0,
            },
            StreamChunk::MessageDelta {
                stop_reason: "stop".into(),
                usage: Usage::default(),
            },
            StreamChunk::Done,
        ]);
        let mut usage = Usage::default();
        let mut stop_reason = None;
        let content = engine
            .stream_response(
                "p8-streaming-session",
                "gpt-5.4-nano",
                engine.provider.clone(),
                fixture_request(),
                &mut usage,
                &mut stop_reason,
                &mut Default::default(),
            )
            .await
            .expect("non-reasoning model uses the production XML fallback");

        assert!(matches!(
            content.as_slice(),
            [MessageContent::Text { text }] if text == "safe  visible <think"
        ));
        assert!(content.iter().all(|part| match part {
            MessageContent::Text { text } => !text.contains("secret"),
            _ => false,
        }));
    }

    #[tokio::test]
    async fn production_stream_consumer_rejects_an_anthropic_thinking_block_without_signature() {
        let engine = fixture_engine(vec![
            StreamChunk::ThinkingDelta {
                text: "private".into(),
                block_index: 0,
            },
            StreamChunk::ToolUseStart {
                id: "toolu_1".into(),
                name: "read_file".into(),
                block_index: 1,
                opaque: None,
            },
            StreamChunk::MessageDelta {
                stop_reason: "tool_use".into(),
                usage: Usage::default(),
            },
            StreamChunk::Done,
        ]);
        let mut usage = Usage::default();
        let mut stop_reason = None;
        let error = engine
            .stream_response(
                "p8-anthropic-session",
                "claude-sonnet-5",
                engine.provider.clone(),
                fixture_request(),
                &mut usage,
                &mut stop_reason,
                &mut Default::default(),
            )
            .await
            .expect_err("an unsigned thinking block cannot be replayed");
        assert!(error.to_string().contains("no signature"));
    }

    #[test]
    fn signed_streamed_thinking_is_replayed_before_a_tool_follow_up() {
        let frames = [
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"consider "}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"file"}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"signed-opaque"}}"#,
            r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"read_file","input":{}}}"#,
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"package.json\"}"}}"#,
        ];
        let mut thinking = BTreeMap::new();
        let mut tools = BTreeMap::new();
        for frame in frames {
            let chunk = parse_sse_line(&format!("data: {frame}"))
                .expect("valid Anthropic frame")
                .expect("content event");
            match chunk {
                StreamChunk::ThinkingDelta { text, block_index } => {
                    thinking
                        .entry(block_index)
                        .or_insert_with(|| (String::new(), None))
                        .0
                        .push_str(&text);
                }
                StreamChunk::ThinkingSignature {
                    signature,
                    block_index,
                } => {
                    thinking
                        .entry(block_index)
                        .or_insert_with(|| (String::new(), None))
                        .1 = Some(signature);
                }
                StreamChunk::ToolUseStart {
                    id,
                    name,
                    block_index,
                    opaque,
                } => {
                    tools.insert(block_index, (id, name, String::new(), opaque));
                }
                StreamChunk::ToolInputDelta {
                    block_index,
                    partial_json,
                } => {
                    tools
                        .get_mut(&block_index)
                        .expect("tool started")
                        .2
                        .push_str(&partial_json);
                }
                _ => panic!("unexpected content event"),
            }
        }
        let assistant = assemble_with_thinking_in_wire_order(BTreeMap::new(), thinking, tools);
        let request = CompletionRequest {
            model: "claude-haiku-4-5".into(),
            messages: vec![
                RequestMessage {
                    role: "assistant".into(),
                    content: assistant,
                },
                RequestMessage {
                    role: "user".into(),
                    content: vec![MessageContent::ToolResult {
                        tool_use_id: "toolu_1".into(),
                        name: "read_file".into(),
                        content: "{\"name\":\"arcana\"}".into(),
                        is_error: false,
                    }],
                },
            ],
            system: None,
            tools: vec![],
            max_tokens: 128,
            temperature: None,
            top_p: None,
            reasoning_effort: None,
            thinking_budget_tokens: Some(64),
            cache: Default::default(),
            clear_tool_uses: None,
        };
        let body = AnthropicProvider::new("unused".into())
            .build_checked_body_for_test(&request, true)
            .expect("signed tool follow-up must be sendable");
        let blocks = body["messages"][0]["content"]
            .as_array()
            .expect("assistant blocks");
        assert_eq!(
            blocks[0],
            serde_json::json!({
                "type": "thinking", "thinking": "consider file", "signature": "signed-opaque",
            })
        );
        assert_eq!(blocks[1]["type"], "tool_use");
        assert_eq!(blocks[1]["input"]["path"], "package.json");
        assert_eq!(body["messages"][1]["content"][0]["tool_use_id"], "toolu_1");
    }

    #[test]
    fn blocks_come_back_in_the_order_the_model_emitted() {
        let mut text = BTreeMap::new();
        text.insert(1u64, "thinking about it".to_string());
        let mut tools = BTreeMap::new();
        tools.insert(0u64, tool("a", "read_file", r#"{"p":"a"}"#));
        tools.insert(2u64, tool("b", "grep", r#"{"q":"x"}"#));

        let out = assemble_in_wire_order(text, tools);
        assert_eq!(out.len(), 3);
        // Tool at 0, text at 1, tool at 2 — not "all text, then all tools".
        assert!(matches!(out[0], MessageContent::ToolUse { .. }));
        assert!(matches!(out[1], MessageContent::Text { .. }));
        assert!(matches!(out[2], MessageContent::ToolUse { .. }));
    }

    #[test]
    fn text_after_a_tool_call_stays_after_it() {
        // The old code pushed all buffered text FIRST, unconditionally, so a
        // model that called a tool and then commented had the two swapped —
        // and the swapped message is replayed on every later turn.
        let mut text = BTreeMap::new();
        text.insert(5u64, "and that is why".to_string());
        let mut tools = BTreeMap::new();
        tools.insert(1u64, tool("a", "read_file", "{}"));

        let out = assemble_in_wire_order(text, tools);
        assert!(matches!(out[0], MessageContent::ToolUse { .. }));
        assert!(matches!(out[1], MessageContent::Text { .. }));
    }

    #[test]
    fn parallel_tool_calls_keep_their_relative_order_every_time() {
        // The defect was non-deterministic ACROSS PROCESSES: HashMap iteration
        // is seeded per process, so a single run proved nothing. Ten identical
        // assemblies must agree, and with a BTreeMap they agree by construction
        // rather than by luck.
        let mut expected: Option<Vec<String>> = None;
        for _ in 0..10 {
            let mut tools = BTreeMap::new();
            for i in 0..6u64 {
                tools.insert(i, tool(&format!("id{i}"), &format!("tool{i}"), "{}"));
            }
            let names: Vec<String> = assemble_in_wire_order(BTreeMap::new(), tools)
                .into_iter()
                .map(|b| match b {
                    MessageContent::ToolUse { name, .. } => name,
                    _ => "?".to_string(),
                })
                .collect();
            match &expected {
                None => expected = Some(names),
                Some(e) => assert_eq!(e, &names, "assembly order must be stable"),
            }
        }
        assert_eq!(
            expected.unwrap(),
            vec!["tool0", "tool1", "tool2", "tool3", "tool4", "tool5"]
        );
    }

    #[test]
    fn an_empty_text_block_is_dropped() {
        // An empty content block poisons every later send in the session.
        let mut text = BTreeMap::new();
        text.insert(0u64, String::new());
        assert!(assemble_in_wire_order(text, BTreeMap::new()).is_empty());
    }

    #[test]
    fn a_tool_call_with_no_arguments_becomes_an_empty_object() {
        // Dropping the call would leave the model waiting for a result it never
        // gets; `{}` at least lets the tool refuse cleanly.
        let mut tools = BTreeMap::new();
        tools.insert(0u64, tool("a", "read_file", ""));
        let out = assemble_in_wire_order(BTreeMap::new(), tools);
        match &out[0] {
            MessageContent::ToolUse { input, .. } => {
                assert!(input.is_object() && input.as_object().unwrap().is_empty());
            }
            _ => panic!("expected a tool use"),
        }
    }
}

#[cfg(test)]
mod procedural_memory_tests {
    use super::*;
    use crate::agent::tool_trait::{ToolContext, ToolResult};
    use crate::provider::traits::{Provider, StreamResult};
    use crate::provider::types::{CompletionRequest, ModelInfo, ProviderStatus, StreamChunk};
    use crate::provider::usage::Usage;
    use async_trait::async_trait;
    use futures::stream;
    use std::collections::VecDeque;

    async fn ensure_isolated_test_analytics() {
        static INIT: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
        INIT.get_or_init(|| async {
            let analytics_home = Box::leak(Box::new(
                tempfile::tempdir().expect("isolated analytics home"),
            ));
            let _home = crate::db::test_support::with_home(analytics_home.path());
            let manager = Arc::new(
                crate::analytics::AnalyticsManager::new()
                    .await
                    .expect("initialize isolated analytics DB"),
            );
            crate::analytics::install_global(manager);
        })
        .await;
    }

    struct ScriptedProvider {
        provider_id: String,
        requests: Mutex<Vec<CompletionRequest>>,
        responses: Mutex<VecDeque<Vec<StreamChunk>>>,
        fail_next_attempt: Mutex<Option<String>>,
        cancel_on_stream: Option<(Arc<Mutex<crate::state::AppState>>, String)>,
        input_token_bound: Option<u64>,
    }

    #[async_trait]
    impl Provider for ScriptedProvider {
        fn id(&self) -> &str {
            &self.provider_id
        }

        fn name(&self) -> &str {
            "Procedural memory fixture"
        }

        fn models(&self) -> Vec<ModelInfo> {
            crate::provider::catalogue::models_for(&self.provider_id)
        }

        async fn input_token_upper_bound(&self, _request: &CompletionRequest) -> Result<u64> {
            self.input_token_bound.ok_or_else(|| {
                ArcanaError::Agent("scripted provider has no configured preflight bound".into())
            })
        }

        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<crate::provider::types::CompletionResponse> {
            unreachable!("the agent loop uses the scripted stream")
        }

        async fn stream(&self, request: CompletionRequest) -> Result<StreamResult> {
            if let Some((state, session_id)) = &self.cancel_on_stream {
                state
                    .lock()
                    .expect("cancel fixture state lock")
                    .cancel_agent(session_id);
            }
            self.requests
                .lock()
                .expect("request capture lock")
                .push(request);
            if let Some(error) = self
                .fail_next_attempt
                .lock()
                .expect("scripted failure lock")
                .take()
            {
                return Err(ArcanaError::Provider(error));
            }
            let response = self
                .responses
                .lock()
                .expect("script response lock")
                .pop_front()
                .expect("one scripted response per provider request");
            Ok(Box::pin(stream::iter(response.into_iter().map(Ok))))
        }

        async fn test_connection(&self) -> Result<ProviderStatus> {
            Ok(ProviderStatus::Connected)
        }
    }

    struct ScriptedTool {
        name: String,
    }

    #[async_trait]
    impl crate::agent::tool_trait::Tool for ScriptedTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "A deterministic tool used by the production engine path test."
        }

        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object", "properties": {}})
        }

        fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
            crate::agent::tool_effect::ToolEffect::Read
        }

        async fn execute(&self, _input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
            ToolResult::success("scripted tool completed")
        }
    }

    #[tokio::test]
    async fn unknown_tool_failure_uses_structured_correlated_tool_result() {
        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let provider = scripted_provider("not_registered");
        let workspace = tempfile::tempdir().expect("isolated workspace");
        let engine = AgentEngine::new(
            provider.clone(),
            super::super::permission::PermissionMode::Autonomous,
            super::super::PAMode::FastDevelopment,
            Arc::new(Mutex::new(crate::state::AppState::default())),
            super::super::permission::new_pending_permissions(),
            super::super::tool_trait::AgentIdentity::worker("worker-backend"),
        )
        .with_worker_run_id("run-unknown-tool");

        engine
            .run(
                "unknown-tool-session",
                workspace.path().to_str().expect("workspace path"),
                "fixture-model",
                Vec::new(),
                "Call the requested tool.",
                Some("Worker instructions."),
            )
            .await
            .expect("scripted engine completes after error result");

        let requests = provider.requests.lock().expect("captured requests");
        assert!(
            requests.len() >= 2,
            "the model must receive the failed result"
        );
        let result = requests[1]
            .messages
            .iter()
            .flat_map(|message| message.content.iter())
            .find_map(|content| match content {
                MessageContent::ToolResult {
                    tool_use_id,
                    name,
                    content,
                    is_error,
                } if tool_use_id == "call-1" && name == "not_registered" => {
                    Some((content, *is_error))
                }
                _ => None,
            })
            .expect("provider receives the matching unknown-tool result");
        let envelope: serde_json::Value =
            serde_json::from_str(result.0).expect("unknown-tool error is valid JSON");
        assert!(result.1, "unknown tool must not look like a success");
        assert_eq!(envelope["schema_version"], 1);
        assert_eq!(envelope["ok"], false);
        assert_eq!(envelope["error"]["code"], "unknown_tool");
        assert_eq!(envelope["error"]["category"], "tool_resolution");
        assert_eq!(envelope["error"]["tool"], "not_registered");
        assert_eq!(
            envelope["error"]["context"]["tool_call_id"],
            "unknown-tool-session-call-1"
        );
        assert_eq!(envelope["error"]["context"]["run_id"], "run-unknown-tool");
    }

    #[tokio::test]
    async fn builtin_handler_failures_reach_model_as_correlated_private_error_results() {
        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated workspace");
        let private_arguments = serde_json::json!({
            "private_prompt": "planted-private-prompt",
            "source": "planted-private-source",
            "api_key": "planted-private-credential",
            "os_error": "planted-private-os-error".repeat(512),
        });
        // These inputs deliberately omit each handler's required fields. They
        // exercise the real registry and executor without spawning commands,
        // invoking an MCP request, requiring a provider credential, or using
        // shared state. Empty-input list queries fail through an isolated
        // database-open blocker; no database is replaced or kept open.
        let expected_live_names = [
            "acknowledge_worker_message",
            "analyze_diff",
            "annotate_file",
            "append_file",
            "assign_task",
            "assign_tasks",
            "bash",
            "check_file",
            "claim_task",
            "create_epic",
            "create_phased_task",
            "create_task",
            "edit_file",
            "e2e_probe",
            "get_epic_progress",
            "get_memory",
            "get_ready_tasks_in_phase",
            "get_task",
            "git_commit",
            "glob",
            "grep",
            "list_directory",
            "list_memories",
            "list_stale_worker_messages",
            "list_tasks",
            "list_worker_health",
            "list_worker_messages",
            "list_worker_requests",
            "mcp_call_tool",
            "mcp_list_tools",
            "planner",
            "project_command",
            "propose_memory",
            "read_file",
            "reconcile_worker_operation",
            "request_supervisor_action",
            "resolve_worker_request",
            "search_memory",
            "send_worker_message",
            "set_task_budget",
            "spawn_parallel_subagents",
            "spawn_qc_review",
            "spawn_subagent",
            "suggest_worker",
            "update_task_status",
            "write_file",
            "write_interface_spec",
        ];
        let cases: Vec<(&str, serde_json::Value)> = vec![
            ("list_memories", private_arguments.clone()),
            ("list_tasks", private_arguments.clone()),
            ("list_worker_messages", private_arguments.clone()),
            ("list_worker_requests", private_arguments.clone()),
            ("mcp_list_tools", private_arguments.clone()),
            ("mcp_call_tool", private_arguments.clone()),
            ("read_file", private_arguments.clone()),
            ("write_file", private_arguments.clone()),
            ("edit_file", private_arguments.clone()),
            (
                "bash",
                serde_json::json!({
                    "command": "echo planted-bash-denial > denied-shell-marker.txt",
                    "private_prompt": "planted-private-prompt",
                    "source": "planted-private-source",
                    "api_key": "planted-private-credential"
                }),
            ),
            ("git_commit", private_arguments.clone()),
            ("project_command", private_arguments.clone()),
            ("check_file", private_arguments.clone()),
            ("append_file", private_arguments.clone()),
            ("get_task", private_arguments.clone()),
            ("create_task", private_arguments.clone()),
            ("assign_task", private_arguments.clone()),
            ("set_task_budget", private_arguments.clone()),
            ("create_epic", private_arguments.clone()),
            (
                "list_directory",
                serde_json::json!({
                    "path": "../planted-private-directory-argument",
                    "private_prompt": "planted-private-prompt",
                    "source": "planted-private-source",
                    "api_key": "planted-private-credential",
                    "os_error": "planted-private-os-error".repeat(512)
                }),
            ),
            ("grep", private_arguments.clone()),
            ("glob", private_arguments.clone()),
            ("create_phased_task", private_arguments.clone()),
            ("get_epic_progress", private_arguments.clone()),
            ("get_ready_tasks_in_phase", private_arguments.clone()),
            ("update_task_status", private_arguments.clone()),
            ("assign_tasks", private_arguments.clone()),
            ("request_supervisor_action", private_arguments.clone()),
            ("planner", private_arguments.clone()),
            ("spawn_subagent", private_arguments.clone()),
            ("spawn_parallel_subagents", private_arguments.clone()),
            ("spawn_qc_review", private_arguments.clone()),
            ("write_interface_spec", private_arguments.clone()),
            (
                "e2e_probe",
                serde_json::json!({
                    "start_cmds": ["echo planted-e2e-command"],
                    "probe_url": "http://127.0.0.1:1/",
                    "expected_text": "planted-e2e-source",
                    "api_key": "planted-private-credential",
                    "private_prompt": "planted-private-prompt"
                }),
            ),
            ("search_memory", private_arguments.clone()),
            ("get_memory", private_arguments.clone()),
            ("propose_memory", private_arguments.clone()),
            ("suggest_worker", private_arguments.clone()),
            ("claim_task", private_arguments.clone()),
            ("analyze_diff", private_arguments.clone()),
            ("annotate_file", private_arguments.clone()),
            ("list_stale_worker_messages", private_arguments.clone()),
            ("list_worker_health", private_arguments.clone()),
            ("resolve_worker_request", private_arguments.clone()),
            ("send_worker_message", private_arguments.clone()),
            ("reconcile_worker_operation", private_arguments.clone()),
            ("acknowledge_worker_message", private_arguments.clone()),
        ];
        let mut actual_live_names: Vec<_> = {
            let mut registry = super::super::tool_registry::ToolRegistry::new();
            super::super::builtin_tools::register_for_mode(
                &mut registry,
                super::super::PAMode::FastDevelopment,
            );
            registry
                .all()
                .iter()
                .map(|tool| tool.name().to_string())
                .collect()
        };
        actual_live_names.sort();
        let mut expected_names = expected_live_names
            .iter()
            .map(|name| (*name).to_string())
            .collect::<Vec<_>>();
        expected_names.sort();
        assert_eq!(actual_live_names, expected_names);
        let covered_names: std::collections::BTreeSet<_> =
            cases.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            covered_names.len(),
            cases.len(),
            "each registered handler has one unique consumer case"
        );
        assert_eq!(
            covered_names,
            expected_live_names
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            "every live registration has a safe controlled consumer failure"
        );

        // Missing required fields provide deterministic local failures. Bash
        // is denied through the public one-use PendingPermissions responder;
        // e2e_probe uses valid input and fails closed before process/HTTP work.
        for (name, input) in cases {
            let session_id = format!("builtin-failure-{name}");
            let blocked_database = match name {
                "list_memories" => Some("memory.db"),
                "list_tasks" | "list_worker_messages" | "list_worker_requests" => Some("tasks.db"),
                _ => None,
            };
            let blocked_path = blocked_database.map(|database| {
                let path = crate::db::arcana_dir()
                    .expect("isolated Arcana database directory")
                    .join(database);
                assert!(
                    !path.exists(),
                    "isolated database fixture must not already exist: {database}"
                );
                std::fs::create_dir(&path).expect("block only the isolated database path");
                path
            });
            let run_id = crate::db::runs::RunsDb::open()
                .expect("open isolated RunsDb")
                .begin_worker_tick("worker-backend", &session_id)
                .expect("persist worker run before tool execution");
            let pending = super::super::permission::new_pending_permissions();
            let bash_responder = if name == "bash" {
                let pending = Arc::clone(&pending);
                let permission_id = format!("{session_id}-builtin-failure-call");
                Some(tokio::spawn(async move {
                    tokio::time::timeout(std::time::Duration::from_secs(2), async {
                        loop {
                            let responder = pending
                                .lock()
                                .expect("pending permission map")
                                .remove(&permission_id);
                            if let Some(responder) = responder {
                                responder
                                    .send(super::super::permission::PermissionResponse::Deny)
                                    .expect("Bash permission waiter remains active");
                                break;
                            }
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .expect("Bash permission request arrives without interaction");
                }))
            } else {
                None
            };
            let mut provider = scripted_provider(name);
            let scripted = Arc::get_mut(&mut provider).expect("unique scripted provider");
            scripted.responses = Mutex::new(VecDeque::from([
                vec![
                    StreamChunk::ToolUseStart {
                        id: "builtin-failure-call".into(),
                        name: name.into(),
                        block_index: 0,
                        opaque: None,
                    },
                    StreamChunk::ToolInputDelta {
                        block_index: 0,
                        partial_json: input.to_string(),
                    },
                    StreamChunk::MessageDelta {
                        stop_reason: "tool_use".into(),
                        usage: Usage::default(),
                    },
                    StreamChunk::Done,
                ],
                vec![
                    StreamChunk::TextDelta {
                        text: "Failure acknowledged.".into(),
                        block_index: 0,
                    },
                    StreamChunk::MessageDelta {
                        stop_reason: "end_turn".into(),
                        usage: Usage::default(),
                    },
                    StreamChunk::Done,
                ],
            ]));
            let state = Arc::new(Mutex::new(crate::state::AppState::default()));
            let engine = AgentEngine::new(
                provider.clone(),
                PermissionMode::Autonomous,
                super::super::PAMode::FastDevelopment,
                state.clone(),
                Arc::clone(&pending),
                super::super::tool_trait::AgentIdentity::worker("worker-backend"),
            )
            .with_worker_run_id(&run_id);
            assert!(
                engine.registry.get(name).is_some(),
                "real builtin {name} is registered"
            );
            engine
                .run(
                    &session_id,
                    workspace.path().to_str().expect("workspace path"),
                    "fixture-model",
                    Vec::new(),
                    "Exercise the requested builtin.",
                    Some("Worker instructions."),
                )
                .await
                .expect("scripted model acknowledges the failed builtin");
            if let Some(blocked_path) = &blocked_path {
                std::fs::remove_dir(blocked_path)
                    .expect("restore the isolated database path before the next case");
            }

            let requests = provider.requests.lock().expect("captured requests");
            assert_eq!(
                requests.len(),
                2,
                "one tool turn and one consumer turn for {name}"
            );
            let (content, is_error) = requests[1]
                .messages
                .iter()
                .flat_map(|message| message.content.iter())
                .find_map(|content| match content {
                    MessageContent::ToolResult {
                        tool_use_id,
                        name: result_name,
                        content,
                        is_error,
                    } if tool_use_id == "builtin-failure-call" && result_name == name => {
                        Some((content, *is_error))
                    }
                    _ => None,
                })
                .expect("model receives matching builtin result");
            assert!(is_error, "{name} failure must not appear successful");
            assert!(content.len() <= 8192, "bounded error for {name}");
            let envelope: serde_json::Value =
                serde_json::from_str(content).expect("structured failure JSON");
            assert_eq!(envelope["schema_version"], 1);
            assert_eq!(envelope["ok"], false);
            assert_eq!(envelope["error"]["tool"], name);
            assert_eq!(envelope["error"]["operation"], format!("tool.{name}"));
            assert_ne!(
                envelope["error"]["code"], "operation_intent_unavailable",
                "{name} must reach its handler or deliberate permission/sandbox guard"
            );
            assert_eq!(
                envelope["error"]["context"]["tool_call_id"],
                format!("{session_id}-builtin-failure-call")
            );
            assert_eq!(envelope["error"]["context"]["run_id"], run_id.as_str());
            assert!(envelope["error"]["category"].as_str().is_some());
            assert!(envelope["error"]["code"].as_str().is_some());
            assert!(envelope["error"]["message"]
                .as_str()
                .is_some_and(|message| message.chars().count() <= 512));
            assert!(envelope["error"]["cause"]
                .as_str()
                .is_some_and(|cause| cause.chars().count() <= 512));
            assert!(envelope["error"]["suggested_action"]
                .as_str()
                .is_some_and(|text| !text.is_empty()));
            assert!(envelope["error"]["retryable"].is_boolean());
            assert!(envelope["error"]["timestamp"].as_str().is_some());
            for marker in [
                "planted-private-prompt",
                "planted-private-source",
                "planted-private-credential",
                "planted-private-os-error",
                "planted-private-directory-argument",
                "planted-bash-denial",
                "planted-e2e-command",
                "planted-e2e-source",
            ] {
                assert!(!content.contains(marker), "{name} must omit {marker}");
            }
            if let Some(responder) = bash_responder {
                responder.await.expect("one-use Bash responder");
                assert!(
                    !workspace.path().join("denied-shell-marker.txt").exists(),
                    "denied Bash command must not launch"
                );
                assert!(pending.lock().expect("pending permission map").is_empty());
                assert_eq!(envelope["error"]["code"], "permission_denied");
            }
            if name == "e2e_probe" {
                assert_eq!(envelope["error"]["code"], "sandbox_violation");
            }
            if name == "list_memories" {
                assert!(envelope["error"]["cause"]
                    .as_str()
                    .is_some_and(|cause| cause.contains("memory unavailable")));
            }
            if name == "list_tasks"
                || name == "list_worker_messages"
                || name == "list_worker_requests"
            {
                assert!(envelope["error"]["cause"].as_str().is_some_and(|cause| {
                    cause.contains("open tasks database") || cause.contains("open tasks DB")
                }));
            }
            if name == "mcp_call_tool" {
                assert_eq!(
                    envelope["error"]["cause"],
                    "A valid remote MCP tool name is required."
                );
            }
            if name == "mcp_list_tools" {
                assert!(envelope["error"]["cause"]
                    .as_str()
                    .is_some_and(|cause| cause.contains("no active consent")));
            }
            assert!(state
                .lock()
                .expect("state readback")
                .agents
                .values()
                .all(|agent| !agent.is_running));
            crate::db::runs::RunsDb::open()
                .expect("reopen isolated RunsDb")
                .finish_worker_tick(&run_id, "completed", "ok", None)
                .expect("close the consumed fixture run before the next worker case");
        }
    }

    #[tokio::test]
    async fn malformed_streamed_tool_arguments_fail_before_running_the_registered_builtin() {
        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated workspace");
        std::fs::write(
            workspace.path().join("must-not-enumerate-fixture.txt"),
            "fixture only",
        )
        .expect("seed enumeration sentinel");
        for (arguments, invalid) in [
            (r#"{"private":"planted-invalid-arguments", "#, true),
            (r#"["planted-invalid-arguments"]"#, true),
            ("null", true),
            ("", false),
            ("{}", false),
        ] {
            let mut provider = scripted_provider("list_directory");
            let responses = Arc::get_mut(&mut provider)
                .expect("unique provider")
                .responses
                .get_mut()
                .expect("scripted responses");
            let first = responses.front_mut().expect("first tool turn");
            for chunk in first {
                if let StreamChunk::ToolInputDelta { partial_json, .. } = chunk {
                    *partial_json = arguments.into();
                }
            }
            let engine = AgentEngine::new(
                provider.clone(),
                PermissionMode::Autonomous,
                super::super::PAMode::FastDevelopment,
                Arc::new(Mutex::new(crate::state::AppState::default())),
                super::super::permission::new_pending_permissions(),
                super::super::tool_trait::AgentIdentity::worker("worker-backend"),
            );
            engine
                .run(
                    "invalid-tool-arguments",
                    workspace.path().to_str().expect("workspace path"),
                    "fixture-model",
                    Vec::new(),
                    "Call the tool.",
                    Some("Worker instructions."),
                )
                .await
                .expect("model consumes the refusal");
            let requests = provider.requests.lock().expect("model requests");
            let replayed_input = requests[1]
                .messages
                .iter()
                .flat_map(|message| message.content.iter())
                .find_map(|block| match block {
                    MessageContent::ToolUse { id, input, .. } if id == "call-1" => Some(input),
                    _ => None,
                })
                .expect("replayed tool call");
            assert!(
                replayed_input.is_object(),
                "provider replay requires an object"
            );
            let (content, is_error) = requests[1]
                .messages
                .iter()
                .flat_map(|message| message.content.iter())
                .find_map(|block| match block {
                    MessageContent::ToolResult {
                        tool_use_id,
                        name,
                        content,
                        is_error,
                    } if tool_use_id == "call-1" && name == "list_directory" => {
                        Some((content, *is_error))
                    }
                    _ => None,
                })
                .expect("model-facing result");
            assert_eq!(
                is_error, invalid,
                "malformed arguments must not become a successful default-directory listing"
            );
            if !invalid {
                assert!(content.contains("must-not-enumerate-fixture.txt"));
                continue;
            }
            let error: serde_json::Value =
                serde_json::from_str(content).expect("bounded failure JSON");
            assert_eq!(error["ok"], false);
            assert_eq!(error["error"]["code"], "invalid_tool_arguments");
            assert_eq!(error["error"]["category"], "argument_validation");
            assert_eq!(
                error["error"]["context"]["tool_call_id"],
                "invalid-tool-arguments-call-1"
            );
            assert!(content.len() <= 8192);
            assert!(!content.contains("planted-invalid-arguments"));
            assert!(!content.contains("must-not-enumerate-fixture"));
        }
    }

    #[tokio::test]
    async fn provider_retry_outcome_is_readable_from_analytics_without_raw_error_text() {
        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let mut provider = scripted_provider("not_registered");
        Arc::get_mut(&mut provider)
            .expect("provider is uniquely owned")
            .provider_id = "fixture_retry_success".into();
        Arc::get_mut(&mut provider)
            .expect("provider is uniquely owned")
            .fail_next_attempt = Mutex::new(Some(
            "Google 503 [Overloaded]: private provider response body".to_string(),
        ));
        let workspace = tempfile::tempdir().expect("isolated workspace");
        let engine = AgentEngine::new(
            provider,
            super::super::permission::PermissionMode::Autonomous,
            super::super::PAMode::FastDevelopment,
            Arc::new(Mutex::new(crate::state::AppState::default())),
            super::super::permission::new_pending_permissions(),
            super::super::tool_trait::AgentIdentity::worker("worker-backend"),
        );

        engine
            .run(
                "provider-retry-analytics-session",
                workspace.path().to_str().expect("workspace path"),
                "fixture-model",
                Vec::new(),
                "Call the requested tool.",
                Some("Worker instructions."),
            )
            .await
            .expect("the retry should recover and the scripted run should finish");

        let report = crate::analytics::global()
            .queries()
            .get_api_failures(24)
            .expect("production analytics query reads the persisted result");
        let failure = report
            .recent_failures
            .iter()
            .find(|failure| {
                failure.provider == "fixture_retry_success"
                    && failure.error_type == "overloaded"
                    && failure.retry_count == 1
                    && failure.resolved
            })
            .expect("the recovered provider call is recorded as resolved");
        assert_eq!(
            report.failures_by_provider.get("fixture_retry_success"),
            Some(&1)
        );
        let retried = report
            .recent_failures
            .iter()
            .filter(|row| row.retry_count > 0)
            .count();
        let recovered = report
            .recent_failures
            .iter()
            .filter(|row| row.retry_count > 0 && row.resolved)
            .count();
        assert_eq!(
            report.auto_retry_success_rate,
            recovered as f64 / retried as f64
        );
        assert!(!failure
            .error_message
            .contains("private provider response body"));
    }

    #[tokio::test]
    async fn actual_google_adapter_terminal_failures_reach_analytics_without_private_bodies() {
        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated workspace");
        for (status, reason, body, expected_class) in [
            (
                429,
                "Too Many Requests",
                r#"{"error":{"status":"RESOURCE_EXHAUSTED","message":"quota exhausted planted-adapter-private-body"}}"#,
                "quota_exhausted",
            ),
            (
                200,
                "OK",
                "data: planted-adapter-private-body invalid JSON\n\n",
                "fatal",
            ),
        ] {
            let before = crate::analytics::global()
                .queries()
                .get_api_failures(24)
                .expect("read baseline failures")
                .recent_failures
                .into_iter()
                .filter_map(|row| row.id)
                .max()
                .unwrap_or(0);
            let (api_base, server) =
                crate::provider::usage::test_response_server(body.into(), status, reason);
            let provider = Arc::new(crate::provider::google::GoogleProvider::new_at(
                "fixture-only-secret".into(),
                api_base,
            ));
            let state = Arc::new(Mutex::new(crate::state::AppState::default()));
            let engine = AgentEngine::new(
                provider,
                PermissionMode::Autonomous,
                super::super::PAMode::FastDevelopment,
                state.clone(),
                super::super::permission::new_pending_permissions(),
                super::super::tool_trait::AgentIdentity::worker("worker-backend"),
            );
            let error = engine
                .run(
                    &format!("adapter-terminal-{expected_class}"),
                    workspace.path().to_str().expect("workspace path"),
                    "gemini-3.8-flash",
                    Vec::new(),
                    "Private fixture prompt.",
                    Some("Worker instructions."),
                )
                .await
                .expect_err("real adapter terminal failure stops the engine");
            server
                .await
                .expect("loopback fixture finishes after its single request");
            assert!(!error.to_string().contains("planted-adapter-private-body"));
            assert!(!error.to_string().contains("fixture-only-secret"));
            let report = crate::analytics::global()
                .queries()
                .get_api_failures(24)
                .expect("actual production analytics reader");
            let new_failures: Vec<_> = report
                .recent_failures
                .iter()
                .filter(|row| row.id.is_some_and(|id| id > before) && row.provider == "google")
                .collect();
            assert_eq!(
                new_failures.len(),
                1,
                "one aggregate per failed provider call"
            );
            let failure = new_failures[0];
            assert_eq!(
                failure.error_type, expected_class,
                "adapter failure flows through engine classification into durable analytics"
            );
            assert_eq!(failure.retry_count, 0);
            assert!(!failure.resolved);
            assert!(!failure
                .error_message
                .contains("planted-adapter-private-body"));
            assert!(!failure.error_message.contains("Private fixture prompt"));
            assert!(failure.error_message.len() < 512);
            assert!(state
                .lock()
                .expect("state readback")
                .agents
                .values()
                .all(|agent| !agent.is_running));
        }
    }

    #[tokio::test]
    async fn actual_google_retry_then_stream_failure_is_one_unresolved_analytics_call() {
        use std::io::{BufRead, BufReader, Read, Write};
        use std::net::TcpListener;

        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated workspace");
        let before = crate::analytics::global()
            .queries()
            .get_api_failures(24)
            .expect("baseline analytics")
            .recent_failures
            .into_iter()
            .filter_map(|row| row.id)
            .max()
            .unwrap_or(0);
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback fixture");
        let api_base = format!("http://{}", listener.local_addr().expect("fixture address"));
        listener
            .set_nonblocking(true)
            .expect("bounded fixture accepts");
        let server = tokio::task::spawn_blocking(move || {
            for (status, reason, content_type, body) in [
                (503, "Service Unavailable", "application/json", r#"{"error":{"status":"UNAVAILABLE","message":"planted-retry-private-body"}}"#),
                (200, "OK", "text/event-stream", "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Partial fixture output.\"}]}}]}\n\ndata: planted-retry-private-body malformed\n\n"),
            ] {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
                let (mut socket, _) = loop {
                    match listener.accept() {
                        Ok(connection) => break connection,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(std::time::Instant::now() < deadline, "fixture request deadline");
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                        Err(error) => panic!("fixture accept failed: {error}"),
                    }
                };
                socket.set_nonblocking(false).expect("blocking accepted fixture stream");
                socket.set_read_timeout(Some(std::time::Duration::from_secs(10))).expect("bounded request read");
                let mut reader = BufReader::new(socket.try_clone().expect("fixture stream"));
                let mut line = String::new();
                reader.read_line(&mut line).expect("request line");
                let mut length = 0usize;
                loop {
                    line.clear();
                    reader.read_line(&mut line).expect("request header");
                    if line == "\r\n" || line.is_empty() { break; }
                    if let Some((key, value)) = line.split_once(':') {
                        if key.eq_ignore_ascii_case("content-length") { length = value.trim().parse().expect("request length"); }
                    }
                }
                assert!(length < 2 * 1024 * 1024, "bounded fixture request");
                reader.read_exact(&mut vec![0; length]).expect("drain fixture request body");
                write!(socket, "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).expect("fixture response");
                socket.flush().expect("fixture response flush");
            }
            2usize
        });
        let engine = AgentEngine::new(
            Arc::new(crate::provider::google::GoogleProvider::new_at(
                "fixture-only-secret".into(),
                api_base,
            )),
            PermissionMode::Autonomous,
            super::super::PAMode::FastDevelopment,
            Arc::new(Mutex::new(crate::state::AppState::default())),
            super::super::permission::new_pending_permissions(),
            super::super::tool_trait::AgentIdentity::worker("worker-backend"),
        );
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            engine.run(
                "adapter-retry-stream-failure",
                workspace.path().to_str().expect("workspace path"),
                "gemini-3.8-flash",
                Vec::new(),
                "Private fixture prompt.",
                Some("Worker instructions."),
            ),
        )
        .await
        .expect("bounded engine run")
        .expect_err("stream failure is terminal after output");
        assert!(!error.to_string().contains("planted-retry-private-body"));
        assert_eq!(server.await.expect("owned fixture server finishes"), 2);
        let report = crate::analytics::global()
            .queries()
            .get_api_failures(24)
            .expect("production failure reader");
        let failures: Vec<_> = report
            .recent_failures
            .iter()
            .filter(|row| row.id.is_some_and(|id| id > before) && row.provider == "google")
            .collect();
        assert_eq!(
            failures.len(),
            1,
            "never count a recovered establishment and failed body as separate calls"
        );
        assert_eq!(failures[0].retry_count, 1);
        assert!(
            !failures[0].resolved,
            "opening the retry stream is not full-call recovery"
        );
        assert_eq!(failures[0].error_type, "fatal");
        assert!(!failures[0]
            .error_message
            .contains("planted-retry-private-body"));
        assert!(!failures[0].error_message.contains("Partial fixture output"));
    }

    #[tokio::test]
    async fn exhausted_provider_failure_is_recorded_without_retrying_or_persisting_body() {
        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let mut provider = scripted_provider("unused");
        Arc::get_mut(&mut provider)
            .expect("provider is uniquely owned")
            .fail_next_attempt = Mutex::new(Some(
            "Google 429 [QuotaExhausted]: private quota response body".to_string(),
        ));
        let workspace = tempfile::tempdir().expect("isolated workspace");
        let engine = AgentEngine::new(
            provider.clone(),
            super::super::permission::PermissionMode::Autonomous,
            super::super::PAMode::FastDevelopment,
            Arc::new(Mutex::new(crate::state::AppState::default())),
            super::super::permission::new_pending_permissions(),
            super::super::tool_trait::AgentIdentity::worker("worker-backend"),
        );

        let error = engine
            .run(
                "provider-quota-analytics-session",
                workspace.path().to_str().expect("workspace path"),
                "fixture-model",
                Vec::new(),
                "Complete the task.",
                Some("Worker instructions."),
            )
            .await
            .expect_err("quota exhaustion is terminal");
        assert!(error.to_string().contains("QuotaExhausted"));
        assert_eq!(
            provider.requests.lock().expect("captured requests").len(),
            1,
            "quota exhaustion must not trigger a retry"
        );

        let report = crate::analytics::global()
            .queries()
            .get_api_failures(24)
            .expect("production analytics query reads the persisted result");
        let failure = report
            .recent_failures
            .iter()
            .find(|failure| {
                failure.provider == "fixture"
                    && failure.error_type == "quota_exhausted"
                    && failure.retry_count == 0
                    && !failure.resolved
            })
            .expect("the terminal provider failure is recorded");
        assert!(!failure
            .error_message
            .contains("private quota response body"));
    }

    fn scripted_provider(tool_name: &str) -> Arc<ScriptedProvider> {
        Arc::new(ScriptedProvider {
            provider_id: "fixture".into(),
            requests: Mutex::new(Vec::new()),
            fail_next_attempt: Mutex::new(None),
            cancel_on_stream: None,
            input_token_bound: Some(10),
            responses: Mutex::new(VecDeque::from([
                vec![
                    StreamChunk::ToolUseStart {
                        id: "call-1".into(),
                        name: tool_name.into(),
                        block_index: 0,
                        opaque: None,
                    },
                    StreamChunk::ToolInputDelta {
                        block_index: 0,
                        partial_json: "{}".into(),
                    },
                    StreamChunk::MessageDelta {
                        stop_reason: "tool_use".into(),
                        usage: Usage::default(),
                    },
                    StreamChunk::Done,
                ],
                vec![
                    StreamChunk::ToolUseStart {
                        id: "call-2".into(),
                        name: "worker_publish".into(),
                        block_index: 0,
                        opaque: None,
                    },
                    StreamChunk::ToolInputDelta {
                        block_index: 0,
                        partial_json: "{}".into(),
                    },
                    StreamChunk::MessageDelta {
                        stop_reason: "tool_use".into(),
                        usage: Usage::default(),
                    },
                    StreamChunk::Done,
                ],
                vec![
                    StreamChunk::TextDelta {
                        text: "finished".into(),
                        block_index: 0,
                    },
                    StreamChunk::MessageDelta {
                        stop_reason: "end_turn".into(),
                        usage: Usage::default(),
                    },
                    StreamChunk::Done,
                ],
            ])),
        })
    }

    #[tokio::test]
    async fn engine_run_cap_denies_before_the_scripted_provider_stream_is_called() {
        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated workspace");
        let mut provider = scripted_provider("worker_scan");
        Arc::get_mut(&mut provider)
            .expect("unique fixture provider")
            .input_token_bound = Some(10);
        let engine = AgentEngine::new(
            provider.clone(),
            super::super::permission::PermissionMode::Autonomous,
            super::super::PAMode::FastDevelopment,
            Arc::new(Mutex::new(crate::state::AppState::default())),
            super::super::permission::new_pending_permissions(),
            super::super::tool_trait::AgentIdentity::worker("worker-backend"),
        )
        .with_budget(crate::budget::TaskBudget {
            max_tokens: Some(1),
            max_usd: None,
            max_turns: Some(20),
        });

        let result = engine
            .run(
                "run-token-denial-session",
                workspace.path().to_str().expect("workspace path"),
                "fixture-model",
                Vec::new(),
                "Start the scripted operation.",
                Some("Worker instructions."),
            )
            .await;
        assert!(matches!(result, Err(ArcanaError::BudgetExceeded(_))));
        assert!(
            provider.requests.lock().expect("request lock").is_empty(),
            "countTokens preflight may run, but the generation stream must not be dispatched"
        );
    }

    #[tokio::test]
    async fn output_token_limit_applies_to_budgeted_supervisor_without_changing_workers() {
        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());

        async fn run_one_turn(
            identity: super::super::tool_trait::AgentIdentity,
            session_id: &str,
            output_limit: Option<u32>,
            budget: crate::budget::TaskBudget,
        ) -> u32 {
            let workspace = tempfile::tempdir().expect("isolated workspace");
            let provider = scripted_provider("unused");
            *provider.responses.lock().expect("script response lock") = VecDeque::from([vec![
                StreamChunk::TextDelta {
                    text: "finished".into(),
                    block_index: 0,
                },
                StreamChunk::MessageDelta {
                    stop_reason: "end_turn".into(),
                    usage: Usage::default(),
                },
                StreamChunk::Done,
            ]]);

            let mut engine = AgentEngine::new(
                provider.clone(),
                super::super::permission::PermissionMode::Autonomous,
                super::super::PAMode::FastDevelopment,
                Arc::new(Mutex::new(crate::state::AppState::default())),
                super::super::permission::new_pending_permissions(),
                identity,
            )
            .with_budget(budget);
            if let Some(limit) = output_limit {
                engine = engine.with_output_token_limit(limit);
            }

            engine
                .run(
                    session_id,
                    workspace.path().to_str().expect("workspace path"),
                    "fixture-model",
                    Vec::new(),
                    "Complete this test turn.",
                    Some("Test instructions."),
                )
                .await
                .expect("scripted one-turn run completes");
            let max_tokens = provider.requests.lock().expect("request capture lock")[0].max_tokens;
            max_tokens
        }

        let supervisor_max_tokens = run_one_turn(
            super::super::tool_trait::AgentIdentity::supervisor(),
            "supervisor-output-limit-session",
            Some(6_144),
            crate::budget::TaskBudget::default_supervisor(),
        )
        .await;
        let worker_max_tokens = run_one_turn(
            super::super::tool_trait::AgentIdentity::worker("worker-backend"),
            "worker-output-limit-session",
            None,
            crate::budget::TaskBudget::default_worker(),
        )
        .await;

        assert_eq!(supervisor_max_tokens, 6_144);
        assert_eq!(worker_max_tokens, 8_192);
    }

    fn scripted_tool_response(
        call_id: &str,
        name: &str,
        input: serde_json::Value,
    ) -> Vec<StreamChunk> {
        vec![
            StreamChunk::ToolUseStart {
                id: call_id.to_string(),
                name: name.to_string(),
                block_index: 0,
                opaque: None,
            },
            StreamChunk::ToolInputDelta {
                block_index: 0,
                partial_json: input.to_string(),
            },
            StreamChunk::MessageDelta {
                stop_reason: "tool_use".into(),
                usage: Usage::default(),
            },
            StreamChunk::Done,
        ]
    }

    #[tokio::test]
    async fn agent_engine_calls_configured_mcp_through_production_executor() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated workspace");

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local MCP fixture");
        let address = listener.local_addr().expect("fixture address");
        let observed_lists = Arc::new(AtomicUsize::new(0));
        let observed_calls = Arc::new(AtomicUsize::new(0));
        let server_lists = Arc::clone(&observed_lists);
        let server_calls = Arc::clone(&observed_calls);
        let server = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let server_lists = Arc::clone(&server_lists);
                let server_calls = Arc::clone(&server_calls);
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut buffer = [0_u8; 4096];
                    let header_end = loop {
                        let count = socket.read(&mut buffer).await.unwrap_or(0);
                        if count == 0 {
                            return;
                        }
                        request.extend_from_slice(&buffer[..count]);
                        if let Some(index) =
                            request.windows(4).position(|bytes| bytes == b"\r\n\r\n")
                        {
                            break index + 4;
                        }
                    };
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    while request.len() < header_end + content_length {
                        let count = socket.read(&mut buffer).await.unwrap_or(0);
                        if count == 0 {
                            return;
                        }
                        request.extend_from_slice(&buffer[..count]);
                    }
                    let body: serde_json::Value =
                        serde_json::from_slice(&request[header_end..header_end + content_length])
                            .expect("MCP request JSON");
                    let method = body["method"].as_str().unwrap_or_default();
                    if method.starts_with("notifications/") {
                        let _ = socket
                            .write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                            .await;
                        return;
                    }
                    let result = match method {
                        "server/discover" => {
                            serde_json::to_value(rmcp::model::DiscoverResult::new(
                                vec![rmcp::model::ProtocolVersion::V_2026_07_28],
                                Default::default(),
                            ))
                            .expect("discovery result")
                        }
                        "tools/list" => {
                            server_lists.fetch_add(1, Ordering::SeqCst);
                            serde_json::json!({
                                "tools": [{
                                    "name":"fixture_search",
                                    "description":"Local synthetic search",
                                    "inputSchema":{"type":"object","properties":{}}
                                }]
                            })
                        }
                        "tools/call" => {
                            server_calls.fetch_add(1, Ordering::SeqCst);
                            serde_json::json!({
                                "content":[{"type":"text","text":"engine fixture result"}],
                                "isError":false
                            })
                        }
                        other => panic!("unexpected MCP request method {other}"),
                    };
                    let payload = serde_json::json!({
                        "jsonrpc":"2.0",
                        "id":body["id"],
                        "result":result
                    })
                    .to_string();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        payload.len(), payload
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });

        let endpoint = format!("http://{address}/mcp");
        crate::db::app_settings::set(
            "github_mcp_config",
            &serde_json::json!({"enabled":true,"server_url":endpoint}).to_string(),
        )
        .expect("persist configured fixture connector");
        crate::egress_consent::WorkspaceEgressConsentDb::open()
            .expect("open isolated consent DB")
            .grant(workspace.path(), "mcp:github-mcp")
            .expect("grant fixture workspace consent");
        let run_id = crate::db::runs::RunsDb::open()
            .expect("open isolated RunsDb")
            .begin_worker_tick("worker-backend", "session-mcp-engine")
            .expect("persist the worker run before tool execution");

        let provider = Arc::new(ScriptedProvider {
            provider_id: "fixture_mcp_engine".into(),
            requests: Mutex::new(Vec::new()),
            fail_next_attempt: Mutex::new(None),
            cancel_on_stream: None,
            input_token_bound: Some(10),
            responses: Mutex::new(VecDeque::from([
                scripted_tool_response("mcp-list", "mcp_list_tools", serde_json::json!({})),
                scripted_tool_response(
                    "mcp-call",
                    "mcp_call_tool",
                    serde_json::json!({"name":"fixture_search","arguments":{"query":"arcana"}}),
                ),
                vec![
                    StreamChunk::TextDelta {
                        text: "MCP result received".into(),
                        block_index: 0,
                    },
                    StreamChunk::MessageDelta {
                        stop_reason: "end_turn".into(),
                        usage: Usage::default(),
                    },
                    StreamChunk::Done,
                ],
            ])),
        });
        let engine = AgentEngine::new(
            provider.clone(),
            super::super::permission::PermissionMode::Autonomous,
            super::super::PAMode::FastDevelopment,
            Arc::new(Mutex::new(crate::state::AppState::default())),
            super::super::permission::new_pending_permissions(),
            super::super::tool_trait::AgentIdentity::worker("worker-backend"),
        )
        .with_worker_run_id(&run_id);

        engine
            .run(
                "session-mcp-engine",
                workspace.path().to_str().expect("workspace path"),
                "fixture-model",
                Vec::new(),
                "Discover and call the configured MCP tool.",
                Some("Use the available MCP tools."),
            )
            .await
            .expect("scripted AgentEngine MCP run completes");

        let requests = provider
            .requests
            .lock()
            .expect("captured provider requests");
        let list_result = requests[1]
            .messages
            .iter()
            .flat_map(|message| message.content.iter())
            .find_map(|content| match content {
                MessageContent::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                    ..
                } if tool_use_id.contains("mcp-list") => Some((content, *is_error)),
                _ => None,
            })
            .expect("provider receives the MCP list tool result");
        assert_eq!(
            observed_lists.load(Ordering::SeqCst),
            1,
            "local MCP list count; error={}; result={}",
            list_result.1,
            list_result.0
        );
        assert_eq!(observed_calls.load(Ordering::SeqCst), 1);
        assert_eq!(requests.len(), 3, "list, call, and final response turns");
        let call_result = requests[2]
            .messages
            .iter()
            .flat_map(|message| message.content.iter())
            .find_map(|content| match content {
                MessageContent::ToolResult {
                    name,
                    content,
                    is_error,
                    ..
                } if name == "mcp_call_tool" => Some((content, *is_error)),
                _ => None,
            })
            .expect("provider receives the real MCP tool result");
        assert!(!call_result.1);
        assert!(call_result.0.contains("engine fixture result"));
        let journal = crate::db::runs::RunsDb::open()
            .expect("reopen isolated RunsDb")
            .operation_journal_page(&run_id, 0, 10)
            .expect("read production operation journal")
            .entries;
        for operation in ["mcp_list_tools", "mcp_call_tool"] {
            assert!(
                journal.iter().any(|entry| {
                    entry.operation == operation
                        && entry.outcome == crate::db::runs::OperationOutcome::Ok
                }),
                "the production {operation} result must be durable"
            );
        }
        server.abort();
    }

    #[tokio::test]
    async fn cancellation_during_provider_stream_is_terminal_and_prevents_tool_side_effects() {
        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let session_id = "cancel-during-stream-session";
        let state = Arc::new(Mutex::new(crate::state::AppState::default()));
        state.lock().expect("state lock").agents.insert(
            session_id.to_owned(),
            crate::state::app_state::ActiveAgent {
                session_id: session_id.to_owned(),
                is_running: true,
                cancel_flag: false,
            },
        );
        let mut provider = scripted_provider("side_effect");
        let scripted = Arc::get_mut(&mut provider).expect("provider is uniquely owned");
        scripted.cancel_on_stream = Some((state.clone(), session_id.to_owned()));
        let response = scripted
            .responses
            .get_mut()
            .expect("scripted responses lock")
            .front_mut()
            .expect("scripted response");
        let usage = response
            .iter_mut()
            .find_map(|chunk| match chunk {
                StreamChunk::MessageDelta { usage, .. } => Some(usage),
                _ => None,
            })
            .expect("message usage chunk");
        *usage = Usage {
            input: 120,
            output: 18,
            ..Usage::default()
        };
        let side_effects = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut engine = AgentEngine::new(
            provider.clone(),
            super::super::permission::PermissionMode::Autonomous,
            super::super::PAMode::FastDevelopment,
            state,
            super::super::permission::new_pending_permissions(),
            super::super::tool_trait::AgentIdentity::worker("worker-backend"),
        );
        engine.registry.register(Arc::new(SideEffectTool {
            calls: side_effects.clone(),
        }));
        let workspace = tempfile::tempdir().expect("isolated workspace");

        let result = engine
            .run(
                session_id,
                workspace.path().to_str().expect("workspace path"),
                "fixture-model",
                Vec::new(),
                "Run the scripted operation.",
                Some("Worker instructions."),
            )
            .await;

        assert!(matches!(result, Err(ArcanaError::Cancelled)));
        assert_eq!(side_effects.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(
            provider.requests.lock().expect("request lock").len(),
            1,
            "the cancelled provider response must be terminal"
        );
        let rows = crate::analytics::global()
            .db()
            .get_token_usage_by_date(chrono::Local::now().date_naive())
            .expect("read analytics rows");
        let persisted = rows
            .iter()
            .find(|row| row.session_id == session_id)
            .expect("cancelled run token usage is persisted");
        assert_eq!(persisted.prompt_tokens, 120);
        assert_eq!(persisted.completion_tokens, 18);
    }

    struct SideEffectTool {
        calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait]
    impl crate::agent::tool_trait::Tool for SideEffectTool {
        fn name(&self) -> &str {
            "side_effect"
        }

        fn description(&self) -> &str {
            "Count a side effect in cancellation tests."
        }

        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object", "properties": {}})
        }

        fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
            crate::agent::tool_effect::ToolEffect::Execute
        }

        async fn execute(&self, _input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            ToolResult::success("side effect completed")
        }
    }

    async fn run_worker_turn(
        provider: Arc<ScriptedProvider>,
        tool_name: &str,
        procedural_recall: bool,
        working_dir: &str,
    ) -> Vec<CompletionRequest> {
        let mut engine = AgentEngine::new(
            provider.clone(),
            super::super::permission::PermissionMode::Autonomous,
            super::super::PAMode::FastDevelopment,
            Arc::new(Mutex::new(crate::state::AppState::default())),
            super::super::permission::new_pending_permissions(),
            super::super::tool_trait::AgentIdentity::worker("worker-backend"),
        );
        if procedural_recall {
            engine = engine.with_procedural_recall(true);
        }
        engine.registry.register(Arc::new(ScriptedTool {
            name: tool_name.into(),
        }));
        engine
            .run(
                &format!("procedural-test-{}", uuid::Uuid::new_v4()),
                working_dir,
                "fixture-model",
                Vec::new(),
                "Continue with the scripted operation.",
                Some("Worker instructions."),
            )
            .await
            .expect("scripted worker turn should complete");
        provider
            .requests
            .lock()
            .expect("request capture lock")
            .clone()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn worker_procedural_memory_reaches_only_the_next_matching_provider_request() {
        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let working_dir = tempfile::tempdir().expect("isolated worker workspace");
        crate::memory::procedural::ProceduralStore::save_topic(
            "worker-scan",
            "Specialized scan procedure",
            "When invoking worker_scan, inspect the assigned task before editing.",
            &[],
        )
        .expect("persist procedure in isolated Arcana home");

        let relevant_tool = "worker_scan";
        let default_requests = run_worker_turn(
            scripted_provider(relevant_tool),
            relevant_tool,
            false,
            working_dir.path().to_str().expect("workspace path"),
        )
        .await;
        assert_eq!(default_requests.len(), 3);
        assert!(
            !default_requests[0]
                .system
                .as_deref()
                .unwrap_or_default()
                .contains("saved procedure"),
            "procedural recall must be absent on the initial request"
        );
        assert!(
            !default_requests[1]
                .system
                .as_deref()
                .unwrap_or_default()
                .contains("(a saved procedure: Specialized scan procedure)"),
            "the engine default must leave procedural recall off"
        );
        assert!(
            !default_requests[2]
                .system
                .as_deref()
                .unwrap_or_default()
                .contains("(a saved procedure: Specialized scan procedure)"),
            "the engine default must leave procedural recall off on every request"
        );

        let recalled_requests = run_worker_turn(
            scripted_provider(relevant_tool),
            relevant_tool,
            true,
            working_dir.path().to_str().expect("workspace path"),
        )
        .await;
        assert_eq!(recalled_requests.len(), 3);
        let recalled_system = recalled_requests[1]
            .system
            .as_deref()
            .expect("system prompt on following request");
        assert!(recalled_system.contains("(a saved procedure: Specialized scan procedure)"));
        assert!(recalled_system.contains("inspect the assigned task before editing"));
        assert!(
            !recalled_requests[0]
                .system
                .as_deref()
                .unwrap_or_default()
                .contains("(a saved procedure: Specialized scan procedure)"),
            "recall must not be added before the matching tool batch runs"
        );
        assert!(
            !recalled_requests[2]
                .system
                .as_deref()
                .unwrap_or_default()
                .contains("(a saved procedure: Specialized scan procedure)"),
            "a procedure recalled for one request must not carry past an unrelated tool batch"
        );

        let unrelated_requests = run_worker_turn(
            scripted_provider("worker_publish"),
            "worker_publish",
            true,
            working_dir.path().to_str().expect("workspace path"),
        )
        .await;
        assert_eq!(unrelated_requests.len(), 3);
        assert!(
            !unrelated_requests[1]
                .system
                .as_deref()
                .unwrap_or_default()
                .contains("(a saved procedure: Specialized scan procedure)"),
            "unrelated invoked tool names must not recall the stored procedure"
        );
        assert!(
            !unrelated_requests[2]
                .system
                .as_deref()
                .unwrap_or_default()
                .contains("(a saved procedure: Specialized scan procedure)"),
            "unrelated tool names must not keep prior procedural text in later requests"
        );
        assert!(
            unrelated_requests[1]
                .messages
                .iter()
                .all(|message| !format!("{:?}", message.content).contains("Worker scan procedure")),
            "recalled text must stay out of persisted conversation history"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn saved_task_breaker_works_with_enabled_daily_reservations() {
        run_task_daily_budget_interaction(false).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn daily_reservation_denial_preserves_the_unclaimed_saved_task_budget() {
        run_task_daily_budget_interaction(true).await;
    }

    async fn run_task_daily_budget_interaction(deny_daily: bool) {
        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().unwrap();
        let _home = crate::db::test_support::with_home(home.path());
        let tasks = crate::db::tasks::TasksDb::open().unwrap();
        let task = tasks
            .create(crate::db::tasks::CreateTaskInput {
                title: "independent budget breakers".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "each cap enforces its own boundary".into(),
                created_by: Some("task-daily-budget-test".into()),
            })
            .unwrap();
        tasks
            .update_status(task.id, "assigned", Some("worker-backend"))
            .unwrap();
        tasks
            .set_task_budget(task.id, Some(100_000), Some(5.0), Some(1))
            .unwrap();
        let workers = crate::db::workers::WorkersDb::open().unwrap();
        let backend = workers.get_by_slug("worker-backend").unwrap().unwrap();
        workers
            .update_status(backend.id, crate::db::workers::WorkerStatus::Active)
            .unwrap();
        crate::budget::daily::DailyCap {
            enabled: true,
            limit_usd: if deny_daily { 0.000001 } else { 100.0 },
        }
        .save()
        .unwrap();
        let provider = Arc::new(ScriptedProvider {
            provider_id: "google".into(),
            requests: Mutex::new(Vec::new()),
            fail_next_attempt: Mutex::new(None),
            cancel_on_stream: None,
            input_token_bound: Some(10),
            responses: Mutex::new(VecDeque::from([vec![
                StreamChunk::ToolUseStart {
                    id: "claim-with-daily-cap".into(),
                    name: "claim_task".into(),
                    block_index: 0,
                    opaque: None,
                },
                StreamChunk::ToolInputDelta {
                    block_index: 0,
                    partial_json: serde_json::json!({"task_id":task.id}).to_string(),
                },
                StreamChunk::MessageDelta {
                    stop_reason: "tool_use".into(),
                    usage: Usage {
                        input: 10,
                        output: 1,
                        ..Usage::default()
                    },
                },
                StreamChunk::Done,
            ]])),
        });
        let state = Arc::new(Mutex::new(crate::state::AppState::default()));
        let capped = crate::provider::capped::CappedProvider::wrap(provider.clone(), state.clone());
        let engine = AgentEngine::new(
            capped,
            super::super::permission::PermissionMode::Autonomous,
            super::super::PAMode::FastDevelopment,
            state,
            super::super::permission::new_pending_permissions(),
            super::super::tool_trait::AgentIdentity::worker("worker-backend"),
        )
        .with_budget(crate::budget::TaskBudget::default_worker());
        let workspace = tempfile::tempdir().unwrap();
        let result = engine
            .run(
                "task-daily-budget-interaction",
                workspace.path().to_str().unwrap(),
                "gemini-3.8-flash",
                Vec::new(),
                "Claim the assigned task.",
                Some("Worker instructions."),
            )
            .await;
        let Err(ArcanaError::BudgetExceeded(reason)) = result else {
            panic!("both enabled policies must surface a budget refusal");
        };
        assert!(reason.contains(if deny_daily {
            "Daily spend ceiling"
        } else {
            "BUDGET_EXCEEDED_TURNS"
        }));
        assert_eq!(
            provider.requests.lock().unwrap().len(),
            if deny_daily { 0 } else { 1 }
        );
        let task = tasks.get_by_id(task.id).unwrap().unwrap();
        assert_eq!(
            (
                task.budget_max_tokens,
                task.budget_max_usd,
                task.budget_max_turns
            ),
            (Some(100_000), Some(5.0), Some(1))
        );
        if deny_daily {
            assert_eq!(task.status, crate::db::tasks::TaskStatus::Assigned);
            assert!(!task.budget_exceeded);
            assert_eq!(
                (
                    task.budget_tokens_used,
                    task.budget_turns_used,
                    task.budget_cost_usd
                ),
                (0, 0, 0.0)
            );
            assert!(task.claimed_by_pid.is_none() && task.claim_host.is_none());
            assert_eq!(
                workers
                    .get_by_slug("worker-backend")
                    .unwrap()
                    .unwrap()
                    .status,
                crate::db::workers::WorkerStatus::Paused
            );
        } else {
            assert_eq!(task.status, crate::db::tasks::TaskStatus::NeedsInfo);
            assert!(task.budget_exceeded);
            assert_eq!(task.budget_turns_used, 1);
            assert!(task.budget_cost_usd > 0.0);
            assert_ne!(
                workers
                    .get_by_slug("worker-backend")
                    .unwrap()
                    .unwrap()
                    .status,
                crate::db::workers::WorkerStatus::Paused
            );
        }
        let config = crate::db::config_db::open().unwrap();
        let holds: i64 = config
            .query_row(
                "SELECT count(*) FROM daily_budget_reservations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            holds, 0,
            "denied calls do not reserve; known usage settles exactly once"
        );
        let spent: i64 = config
            .query_row(
                "SELECT coalesce(sum(spent_micros),0) FROM daily_budget_totals",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let expected_cost = crate::cost::pricing::cost_usd(
            "gemini-3.8-flash",
            &Usage {
                input: 10,
                output: 1,
                ..Usage::default()
            },
        )
        .unwrap();
        assert_eq!(
            spent,
            if deny_daily {
                0
            } else {
                (expected_cost * 1_000_000.0).ceil() as i64
            },
            "daily ledger settles observed usage, never the conservative maximum"
        );
        assert!(
            (task.budget_cost_usd - if deny_daily { 0.0 } else { expected_cost }).abs() < 1e-12
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn claimed_task_budget_activates_before_the_next_provider_request() {
        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let task_db = crate::db::tasks::TasksDb::open().expect("isolated tasks DB");
        let task = task_db
            .create(crate::db::tasks::CreateTaskInput {
                title: "budget activation task".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "saved cap fires after claim".into(),
                created_by: Some("test".into()),
            })
            .expect("create task");
        task_db
            .update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign task");
        task_db
            .set_task_budget(task.id, Some(10_000), Some(1.0), Some(10))
            .expect("save task budget");
        task_db
            .update_task_budget_usage(task.id, 2_000, 0.0, 0)
            .expect("seed previously consumed task tokens");

        let provider = Arc::new(ScriptedProvider {
            provider_id: "fixture".into(),
            requests: Mutex::new(Vec::new()),
            fail_next_attempt: Mutex::new(None),
            cancel_on_stream: None,
            input_token_bound: Some(10),
            responses: Mutex::new(VecDeque::from([vec![
                StreamChunk::ToolUseStart {
                    id: "claim-call".into(),
                    name: "claim_task".into(),
                    block_index: 0,
                    opaque: None,
                },
                StreamChunk::ToolInputDelta {
                    block_index: 0,
                    partial_json: format!("{{\"task_id\":{}}}", task.id),
                },
                StreamChunk::MessageDelta {
                    stop_reason: "tool_use".into(),
                    usage: Usage::default(),
                },
                StreamChunk::Done,
            ]])),
        });
        let workspace = tempfile::tempdir().expect("worker workspace");
        let engine = AgentEngine::new(
            provider.clone(),
            super::super::permission::PermissionMode::Autonomous,
            super::super::PAMode::FastDevelopment,
            Arc::new(Mutex::new(crate::state::AppState::default())),
            super::super::permission::new_pending_permissions(),
            super::super::tool_trait::AgentIdentity::worker("worker-backend"),
        )
        .with_budget(crate::budget::TaskBudget::default_worker());

        let result = engine
            .run(
                "budget-activation-session",
                workspace.path().to_str().expect("workspace path"),
                "fixture-model",
                Vec::new(),
                "Claim the assigned task.",
                Some("Worker instructions."),
            )
            .await;
        assert!(matches!(result, Err(ArcanaError::BudgetExceeded(_))));
        assert_eq!(
            provider.requests.lock().expect("request lock").len(),
            1,
            "the saved token cap must block another provider request"
        );
        let claimed = task_db.get_by_id(task.id).unwrap().unwrap();
        assert_eq!(claimed.status, crate::db::tasks::TaskStatus::NeedsInfo);
        assert_eq!(claimed.budget_turns_used, 1);
    }
}
