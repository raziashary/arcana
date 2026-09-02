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
use super::tool_trait::Tool;
use crate::cost::tracker::CostTracker;
use crate::errors::{AutoDevError, Result};
use crate::events::{
    emit_agent_done, emit_agent_error, emit_thinking, emit_token, AgentDonePayload,
    AgentErrorPayload, ThinkingPayload, TokenPayload,
};
use crate::hooks::{HookContext, HookRegistry, HookType};
use crate::provider::traits::Provider;
use crate::provider::types::{CompletionRequest, MessageContent, RequestMessage, StreamChunk};
use crate::session::manager::SessionManager;
use crate::state::AppState;
use crate::provider::usage::{StopReason, Usage};
use crate::state::lock::LockTolerant;

const MAX_TOOL_TURNS: usize = 20;

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
            last_run: std::sync::Mutex::new(None),
        }
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
    pub async fn run(
        &self,
        session_id: &str,
        working_dir: &str,
        model: &str,
        mut history: Vec<RequestMessage>,
        user_message: &str,
        system_prompt_override: Option<&str>,
    ) -> Result<Vec<RequestMessage>> {
        // ── P3-05 · exactly one terminal event, on every exit ────────────────
        //
        // Three paths returned without telling anyone (`:128`, `:328`, `:628`),
        // so the task was dead and the UI spun forever. Created **before** the
        // first possible return, and it owns the emission — there is no way to
        // emit a terminal event without going through it, and therefore no way
        // to emit two. `TG3-01` fails zero and two alike.
        let mut terminal = crate::agent::terminal_event::TerminalEventGuard::new(session_id);

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
            terminal.error("The run was cancelled before it started.");
            return Err(AutoDevError::Agent(
                "Agent was cancelled before start".into(),
            ));
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
            Some(override_str) => (override_str.to_string(), String::new()),
            None => context_builder.build_system_prompt_parts(self.pa_mode, working_dir),
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
        let caps = self.provider.capabilities(model);
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
        );
        let mut tool_executor = match &self.session_mgr {
            Some(sm) => tool_executor.with_session_manager(sm.clone()),
            None => tool_executor,
        };

        let mut turn = 0;
        let mut total_usage = Usage::default();
        // Ruflo Feature 4 — cumulative budget usage tracked across all turns
        let mut budget_status = crate::budget::BudgetStatus::default();

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
            if let Some(ref budget) = self.task_budget {
                if let Some(reason) = budget.check(&budget_status) {
                    let reason_str = reason.as_event_str().to_string();
                    crate::events::emit_budget_exceeded(crate::events::BudgetExceededEvent {
                        session_id: session_id.to_string(),
                        task_id: self.task_id,
                        reason: reason_str.clone(),
                        tokens_used: budget_status.tokens_used,
                        cost_usd: budget_status.cost_usd,
                        turns_used: budget_status.turns_used,
                    });
                    if let Some(tid) = self.task_id {
                        self.escalate_budget_exceeded(tid, &budget_status, &reason_str);
                    }
                    // D-1. Record BEFORE returning. This is the budget path:
                    // the run that ends here is by construction the one that
                    // spent the most, and it used to record nothing at all.
                    self.record_spend(session_id, model, &total_usage);
                    terminal.error(format!("Budget exceeded: {reason_str}"));
                    return Err(AutoDevError::BudgetExceeded(reason_str));
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
                    CompactionEngine::compact_with_summary(history, self.provider.as_ref(), model)
                        .await;
                history = compacted;
                if compaction_usage.total() > 0 {
                    info!(
                        "Compaction cost {} input / {} output tokens",
                        compaction_usage.total_input(),
                        compaction_usage.output
                    );
                    total_usage.add(&compaction_usage);
                }
            }

            // Cancel check
            if self.state.lock_tolerant().is_cancelled(session_id) {
                info!("Agent cancelled during turn {turn}");
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

            let request = CompletionRequest {
                model: model.to_string(),
                messages: history.clone(),
                system: Some(system_prompt.clone()),
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
                max_tokens: 8192.min(caps.max_output),
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
            // Snapshot for the per-turn delta. `total_usage` accumulates across
            // turns, and the cache question is per-request.
            let usage_before_turn = total_usage.clone();
            let stream_result = self
                .stream_response(
                    session_id,
                    model,
                    request,
                    &mut total_usage,
                    &mut turn_stop_reason,
                )
                .await;

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
                .map(|raw| StopReason::parse(self.provider.id(), raw));

            if reason.as_ref().is_some_and(StopReason::is_truncated) {
                warn!(
                    "Turn truncated by the output ceiling (model={model}, provider={}, \
                     wire reason={:?}); the response is incomplete",
                    self.provider.id(),
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
                budget_status.tokens_used = total_usage.total() as u64;
                // An unpriced model is a configuration error, not a $0 turn.
                // `TG-P2-02` gates that every model the UI offers is priced, so
                // reaching this branch means the model came from somewhere the
                // picker does not control. Logged at error level and NOT folded
                // into the running cost: adding 0.0 would under-report spend
                // and, once the cap lands, let an unpriced model spend past it
                // silently.
                match CostTracker::calculate(model, &total_usage) {
                    Ok(c) => budget_status.cost_usd = c.cost_usd,
                    Err(e) => error!("Cost unavailable for this turn: {e}"),
                }
                if let Some(tid) = self.task_id {
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

            for tc in &tool_calls {
                if let MessageContent::ToolUse { id, name, input, .. } = tc {
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
                // D-1. The `?` here was the third path that jumped over cost
                // recording: a tool-batch failure discarded the whole run's
                // spend. Handled explicitly so the record happens first.
                match tool_executor.execute_batch(tool_batch).await {
                    Ok(r) => r,
                    Err(e) => {
                        self.record_spend(session_id, model, &total_usage);
                        terminal.error(format!("Tool execution failed: {e}"));
                        return Err(e);
                    }
                }
            } else {
                Vec::new()
            };

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
                by_id.insert(tool_use_id, (format!("Unknown tool: {name}"), true));
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
                self.provider.id()
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
        // against the daily ceiling. Mode 2 was unaffected because
        // `recipes/runner.rs` records per-turn, inside its loop — which is
        // probably why nobody noticed.
        //
        // `record_spend` is now called on every exit path.
        let cost_usd = self.record_spend(session_id, model, &total_usage);

        // P2-49. `emit_cost_update` had exactly ONE caller — the Mode 2 worker
        // tick — so the entire Mode 1 agent path was invisible to every cost
        // surface that listens for it, including the only budget warning the UI
        // had. Mode 1 emitted `agent_done` instead, which carries no cost.
        // (The cost event is emitted by `record_spend`, on every exit path
        // rather than only this one.)

        // Phase 7.5 — Record token usage in analytics DB.
        // Infer provider from model name (engine only knows the model string).
        {
            // P2-53. This substring ladder was written out here AND in the other
                            // runtime path, and they did not agree: `o4-mini`
                            // matched neither and was filed as "unknown",
                            // attributing real spend to a vendor that does not
                            // exist. One shared function now answers.
                            let analytics_provider =
                                crate::provider::routing::provider_for_analytics(model);
            // Phase 1 of docs/endtoendrun/14may2026/02 — fail loudly so a
            // future analytics regression shows up in the log instead of
            // silently zeroing the Dashboard.
            if let Err(e) = crate::analytics::global().collector().record_token_usage(
                session_id.to_string(),
                "mode1_agent".to_string(),
                analytics_provider.to_string(),
                model.to_string(),
                total_usage.total_input() as i32,
                total_usage.output as i32,
                None,
                cost_usd,
                "agent_session".to_string(),
            ) {
                error!("[Mode1] record_token_usage failed for session {session_id}: {e}");
            }
        }

        terminal.done(AgentDonePayload {
            session_id: session_id.to_string(),
            message_id: Uuid::new_v4().to_string(),
            tokens_in: total_usage.total_input(),
            tokens_out: total_usage.output,
            cost_usd,
        });

        Ok(history)
    }

    /// Stream a single LLM response, emitting token events to the frontend.
    /// Returns the fully assembled response content blocks.
    /// `stop_reason` is an out-parameter because the caller must be able to
    /// tell a `max_tokens` truncation from a clean finish (P2-46). It used to be
    /// destructured away with `..`, so the two were literally the same code
    /// path and silent truncation was invisible.
    async fn stream_response(
        &self,
        session_id: &str,
        model: &str,
        request: CompletionRequest,
        total_usage: &mut Usage,
        stop_reason: &mut Option<String>,
    ) -> Result<Vec<MessageContent>> {
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
        let provider = self.provider.clone();
        let mut stream = crate::provider::retry::with_retry(
            || {
                let p = provider.clone();
                let r = request_for_retry.clone();
                async move { p.stream(r).await }
            },
            |e| {
                // The adapter embedded the class in the message as `[Class]`.
                // Read back until the error type carries it structurally —
                // recorded as `F-5`, and this is its first real consumer.
                let text = e.to_string();
                if text.contains("[QuotaExhausted]") {
                    crate::provider::usage::ErrorClass::QuotaExhausted
                } else if text.contains("[Overloaded]") {
                    crate::provider::usage::ErrorClass::Overloaded
                } else if text.contains("[Retryable]") {
                    crate::provider::usage::ErrorClass::Retryable
                } else {
                    crate::provider::usage::ErrorClass::Fatal
                }
            },
        )
        .await?;

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
        // block index → (tool_use_id, tool_name, partial_input_json)
        // (id, name, accumulated-json, opaque-echo-token)
        //
        // The fourth element is Gemini'"'"'s `thoughtSignature`, which must be
        // echoed back on the next request or the turn is rejected with a 400.
        let mut tool_use_map: std::collections::BTreeMap<u64, (String, String, String, Option<String>)> =
            std::collections::BTreeMap::new();
        let mut content_blocks: Vec<MessageContent> = Vec::new();

        while let Some(chunk) = stream.next().await {
            match chunk? {
                StreamChunk::TextDelta { text, block_index } => {
                    emit_token(TokenPayload {
                        session_id: session_id.to_string(),
                        text: text.clone(),
                    });
                    text_by_block.entry(block_index).or_default().push_str(&text);
                }
                StreamChunk::ThinkingDelta { text } => {
                    emit_thinking(ThinkingPayload {
                        session_id: session_id.to_string(),
                        text: text.clone(),
                    });
                }
                StreamChunk::ToolUseStart {
                    id,
                    name,
                    block_index,
                    opaque,
                } => {
                    debug!("Tool use start: name={name} id={id} block_index={block_index}");
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
                    // `message_start` opens the message and carries no stop
                    // reason; only a non-empty value is a real one.
                    if !reason.is_empty() {
                        *stop_reason = Some(reason);
                    }
                }
                StreamChunk::Done => break,
            }
        }

        content_blocks.extend(assemble_in_wire_order(text_by_block, tool_use_map));

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
    let mut ordered: std::collections::BTreeMap<u64, MessageContent> =
        std::collections::BTreeMap::new();

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
        let input = serde_json::from_str(&json_str)
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
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
    use std::collections::BTreeMap;

    fn tool(id: &str, name: &str, json: &str) -> (String, String, String, Option<String>) {
        (id.to_string(), name.to_string(), json.to_string(), None)
    }

    /// A tool call carrying a provider echo token (Gemini'"'"'s thoughtSignature).
    fn tool_with_signature(
        id: &str,
        name: &str,
        json: &str,
        sig: &str,
    ) -> (String, String, String, Option<String>) {
        (id.to_string(), name.to_string(), json.to_string(), Some(sig.to_string()))
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
