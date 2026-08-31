// spawn_qc_review — Doc 04 Phase 3 builtin tool.
//
// Called by the `qc-dispatcher` recipe once per qc_review task per tick.
// Atomically claims the task (`under_review_by = subagent_slug`), builds a
// per-task QC reviewer system prompt, spawns an ephemeral subagent on
// sonnet, and returns the subagent_id IMMEDIATELY. A background tokio task
// awaits the reviewer's terminal state and:
//   1. Adds the reviewer's `cost_usd` to `task_qc_cost_usd` on the task.
//   2. Clears the `under_review_by` claim.
//   3. Records the dispatch end via the analytics layer.
//
// This is the only sanctioned path for spawning QC reviewers — the
// dispatcher recipe is explicitly forbidden from calling spawn_subagent /
// spawn_parallel_subagents directly so the claim + accounting + cost guard
// stay centralised.

use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, info, warn};

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;
use crate::recipes::defaults::qc_reviewer_system_prompt;
use crate::subagent::{self, SubagentConfig, SubagentStatus};
use crate::subagent::pool::QC_DISPATCHER_SLUG;

pub struct SpawnQcReviewTool;

#[async_trait]
impl Tool for SpawnQcReviewTool {
    fn name(&self) -> &str { "spawn_qc_review" }

    fn description(&self) -> &str {
        "Spawn an ephemeral per-task QC reviewer subagent. Atomically claims \
         the task by setting `under_review_by` and returns the subagent_id \
         immediately — the reviewer runs in the background. The cost is \
         attributed to the task on completion. Only the qc-dispatcher recipe \
         should call this; workers must not."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "integer",
                    "description": "Integer id of the task in qc_review to review. Must already exist with status='qc_review'."
                },
                "repo_path": {
                    "type": "string",
                    "description": "Optional repository root for Phase 0 analyze_diff. When omitted, the reviewer falls back to a workspace-scoped review."
                },
                "model": {
                    "type": "string",
                    "description": "Override LLM (registry key). Default: claude-sonnet-4-5."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Max wall-clock seconds for the reviewer. Default: 600."
                }
            },
            "required": ["task_id"]
        })
    }

    fn is_read_only(&self) -> bool { false }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let task_id = match input.get("task_id").and_then(|v| v.as_i64()) {
            Some(n) if n > 0 => n,
            _ => return ToolResult::error("'task_id' is required and must be a positive integer"),
        };

        // K.11 — depth guard. qc-dispatcher runs at depth 0 → spawned reviewers
        // at depth 1. Anyone else calling this tool is misbehaving.
        if ctx.depth >= subagent::pool::MAX_NESTING_DEPTH {
            return ToolResult::error(format!(
                "spawn_qc_review rejected: caller is at depth {} (limit {})",
                ctx.depth, subagent::pool::MAX_NESTING_DEPTH
            ));
        }

        let mgr = match subagent::manager::global() {
            Some(m) => m,
            None => return ToolResult::error(
                "Subagent system unavailable: SubagentManager not initialised. \
                 Wait for sidecar startup and try again."
            ),
        };

        // Doc 04 Phase 4.4 — cost guard. Check the rolling hour QC spend
        // BEFORE claiming the task so a tripped guard doesn't leak claim
        // state. Guard reads/writes a module-level counter so it's
        // thread-safe and lock-free.
        if let Some(reject) = qc_cost_guard::check_and_record_dispatch_intent() {
            return ToolResult::error(&reject);
        }

        // ── Atomic claim ────────────────────────────────────────────────────
        let tasks_db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("open tasks.db failed: {e}")),
        };

        // Load the task to check status + read repo_path / assigned_worker
        let task = match tasks_db.get_by_id(task_id) {
            Ok(Some(t)) => t,
            Ok(None)    => return ToolResult::error(&format!("task #{task_id} not found")),
            Err(e)      => return ToolResult::error(&format!("read task #{task_id}: {e}")),
        };
        if task.status.as_str() != "qc_review" {
            return ToolResult::error(&format!(
                "task #{task_id} status is '{}' (not qc_review) — refusing to spawn reviewer",
                task.status.as_str()
            ));
        }
        if task.under_review_by.is_some() {
            return ToolResult::error(&format!(
                "task #{task_id} is already under review by '{}'",
                task.under_review_by.as_deref().unwrap_or("?")
            ));
        }

        // Generate the slug FIRST so it can be the claim value. We synthesise
        // it from a stable pattern so the reconciler can match registry IDs
        // back to claims; the real subagent_id will start with the same
        // prefix because slug::generate uses `subagent-<parent_slug>-...`.
        let claim_token = format!("qc-rev-task{task_id}-{}", chrono::Utc::now().timestamp_millis());

        match tasks_db.try_claim_for_qc_review(task_id, &claim_token) {
            Ok(true)  => debug!("[spawn_qc_review] claimed task #{task_id} -> {claim_token}"),
            Ok(false) => return ToolResult::error(&format!(
                "task #{task_id} claim raced with another reviewer — try the next dispatcher tick"
            )),
            Err(e) => return ToolResult::error(&format!("claim task #{task_id}: {e}")),
        }

        // ── Build the per-task reviewer prompt ─────────────────────────────
        let repo_path = input.get("repo_path").and_then(|v| v.as_str());
        let system_prompt = qc_reviewer_system_prompt(task_id, repo_path);

        // ── Spawn (does NOT await result) ──────────────────────────────────
        let model = input.get("model")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("claude-sonnet-4-5")
            .to_string();
        let timeout_secs = input.get("timeout_secs")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32)
            .unwrap_or(600);

        let config = SubagentConfig {
            parent_slug:       QC_DISPATCHER_SLUG.to_string(),
            parent_session_id: ctx.session_id.clone(),
            system_prompt,
            tools:             Vec::new(),                  // MCP-only
            model,
            working_dir:       ctx.working_dir.clone(),
            max_turns:         30,
            timeout_secs,
            depth:             ctx.depth + 1,
        };

        // Use a placeholder trigger; the reviewer reads task details via
        // get_task as its first action per its recipe.
        let trigger = format!("Begin QC review of task #{task_id}.");

        let handle = match mgr.spawn(config, trigger).await {
            Ok(h) => h,
            Err(e) => {
                // Roll back the claim so the next tick can retry.
                let _ = tasks_db.release_qc_claim(task_id);
                return ToolResult::error(&format!("spawn_qc_review: {e}"));
            }
        };

        let subagent_id = handle.id.clone();
        // Upgrade the claim from the synthetic token to the real subagent_id
        // so the stranded-reviewer reconciler can match it directly against
        // SubagentManager::list. Best-effort: a failure here just means the
        // reconciler falls back to the timestamp heuristic.
        if let Err(e) = tasks_db.upgrade_qc_claim(task_id, &subagent_id) {
            warn!(
                "[spawn_qc_review] upgrade_qc_claim task#{task_id} to '{subagent_id}' \
                 failed: {e} — reconciler will fall back to timestamp heuristic"
            );
        }
        info!(
            "[spawn_qc_review] dispatched task #{task_id} -> subagent '{subagent_id}' \
             (claim_token='{claim_token}')"
        );

        // ── Background completion handler ──────────────────────────────────
        // await_result blocks until the reviewer terminates. We can't block
        // the dispatcher tick (entire point of fan-out), so spin a detached
        // task. On terminal state: clear claim, attribute cost, record
        // analytics event.
        let mgr_for_bg = mgr.clone();
        let subagent_id_for_bg = subagent_id.clone();
        let task_id_for_bg = task_id;
        tokio::spawn(async move {
            match mgr_for_bg.await_result(&subagent_id_for_bg).await {
                Ok(result) => {
                    let cost = result.cost_usd;
                    // 1. Attribute cost to the task.
                    if cost > 0.0 {
                        if let Ok(db) = TasksDb::open() {
                            if let Err(e) = db.add_qc_cost(task_id_for_bg, cost) {
                                warn!(
                                    "[spawn_qc_review] add_qc_cost task#{task_id_for_bg} \
                                     cost={cost:.4} failed: {e}"
                                );
                            }
                        }
                    }
                    // 2. Roll the realised cost into the hour-bucketed
                    //    cost guard so the next dispatch tick sees the
                    //    accurate running spend.
                    qc_cost_guard::record_dispatch_complete(cost);

                    // 3. Release the claim.
                    if let Ok(db) = TasksDb::open() {
                        let _ = db.release_qc_claim(task_id_for_bg);
                    }

                    // 4. Analytics — record completed dispatch (idempotent).
                    record_analytics_event(
                        "qc_review_dispatched_complete",
                        task_id_for_bg,
                        &subagent_id_for_bg,
                        Some(cost),
                        result.duration_secs,
                        matches!(result.status,
                            SubagentStatus::Completed),
                    );
                    info!(
                        "[spawn_qc_review] task #{task_id_for_bg} reviewer '{}' \
                         terminated status={} cost=${cost:.4} duration={:.1}s",
                        subagent_id_for_bg,
                        result.status.as_str(),
                        result.duration_secs,
                    );
                }
                Err(e) => {
                    // Reviewer task crashed (join error, timeout, transport
                    // error). Roll the claim back so the next tick can retry.
                    warn!(
                        "[spawn_qc_review] reviewer '{}' for task #{task_id_for_bg} \
                         failed: {e}",
                        subagent_id_for_bg
                    );
                    if let Ok(db) = TasksDb::open() {
                        let _ = db.release_qc_claim(task_id_for_bg);
                    }
                    qc_cost_guard::record_dispatch_complete(0.0);
                    record_analytics_event(
                        "qc_review_dispatched_failed",
                        task_id_for_bg,
                        &subagent_id_for_bg,
                        None,
                        0.0,
                        false,
                    );
                }
            }
        });

        // 4.1 — record the dispatch START synchronously (the await_result
        // task records completion later).
        record_analytics_event(
            "qc_review_dispatched",
            task_id,
            &subagent_id,
            None,
            0.0,
            true,
        );

        ToolResult::success(json!({
            "task_id":     task_id,
            "subagent_id": subagent_id,
            "status":      "dispatched",
            "claim_token": claim_token,
        }).to_string())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Analytics breadcrumb — Doc 04 Phase 4.1
// ─────────────────────────────────────────────────────────────────────────────

fn record_analytics_event(
    kind: &str,
    task_id: i64,
    subagent_id: &str,
    cost_usd: Option<f64>,
    duration_secs: f64,
    success: bool,
) {
    // The analytics layer's system_event recorder is the right home; this
    // wrapper exists so callers don't have to reason about the underlying
    // schema. Failures are intentionally swallowed (events are
    // best-effort observability).
    let payload = serde_json::json!({
        "kind":          kind,
        "task_id":       task_id,
        "subagent_id":   subagent_id,
        "cost_usd":      cost_usd,
        "duration_secs": duration_secs,
        "success":       success,
    });
    debug!("[analytics:qc] {payload}");
    let mgr = crate::analytics::global();
    let severity = if success { "info" } else { "warn" };
    let message = format!(
        "task #{task_id} subagent={subagent_id} duration={duration_secs:.1}s cost={cost_usd:?}"
    );
    if let Err(e) = mgr.collector().record_system_event(
        "qc-dispatcher".to_string(),
        kind.to_string(),
        message,
        severity.to_string(),
        Some(payload.to_string()),
    ) {
        debug!("[analytics:qc] record_system_event {kind} failed: {e}");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cost guard — Doc 04 Phase 4.4
// ─────────────────────────────────────────────────────────────────────────────
//
// Belt-and-braces guard against a runaway reject/retry loop: if the rolling
// 1-hour QC spend exceeds a configurable threshold (default $5), the
// dispatcher's next `spawn_qc_review` call is rejected with a clear error.
//
// Implementation: a module-level mutex-protected ring of (timestamp,
// cost_usd) entries. Inserts on dispatch start (provisional 0.0) AND on
// dispatch complete (real cost). Reads sum all entries newer than 1 hour.
//
// The "intent" record at dispatch start lets the guard count in-flight
// reviewers conservatively — a burst of cheap-looking dispatches won't slip
// under the cap just because their realised costs aren't in yet.

mod qc_cost_guard {
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    // ~$5/hour ceiling. Generous for normal operation (sonnet QC costs a few
    // cents/task → ~100 reviews/hour cap), tight enough to catch a loop.
    const CEILING_USD_PER_HOUR: f64 = 5.0;
    // Pessimistic cost estimate per in-flight dispatch. Higher than the
    // expected per-task cost so the guard trips when many reviews are
    // already mid-flight, before their realised costs come in.
    const PROVISIONAL_COST_USD: f64 = 0.10;
    const WINDOW: Duration = Duration::from_secs(3600);

    struct Entry { at: Instant, cost: f64 }
    static RING: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();

    fn ring() -> &'static Mutex<Vec<Entry>> {
        RING.get_or_init(|| Mutex::new(Vec::new()))
    }

    fn current_sum(now: Instant) -> f64 {
        let mut g = ring().lock().unwrap();
        g.retain(|e| now.duration_since(e.at) < WINDOW);
        g.iter().map(|e| e.cost).sum()
    }

    /// Called at dispatch START. Returns Some(reason) when the guard is
    /// tripped (and the spawn should be rejected), None when the spawn may
    /// proceed. Side effect: records a provisional cost in the ring so the
    /// next check sees the in-flight reviewer.
    pub fn check_and_record_dispatch_intent() -> Option<String> {
        let now = Instant::now();
        let sum = current_sum(now);
        if sum + PROVISIONAL_COST_USD > CEILING_USD_PER_HOUR {
            return Some(format!(
                "QC cost guard tripped: rolling 1h spend ~${sum:.2} + provisional \
                 ${PROVISIONAL_COST_USD:.2} would exceed ${CEILING_USD_PER_HOUR:.2}/hour. \
                 Reviewer not spawned — wait for in-flight reviewers to complete."
            ));
        }
        let mut g = ring().lock().unwrap();
        g.push(Entry { at: now, cost: PROVISIONAL_COST_USD });
        None
    }

    /// Called at dispatch COMPLETE with the realised cost. The matching
    /// provisional entry is removed and replaced with the real one so the
    /// rolling sum is accurate. We can't trivially match entries to specific
    /// dispatches, so we simply pop one provisional entry (any one) and push
    /// the real cost. Over a busy window this self-corrects.
    pub fn record_dispatch_complete(realised_cost: f64) {
        let mut g = ring().lock().unwrap();
        // Pop the oldest provisional entry if present.
        if let Some(idx) = g.iter().position(|e| (e.cost - PROVISIONAL_COST_USD).abs() < 1e-9) {
            g.remove(idx);
        }
        g.push(Entry { at: Instant::now(), cost: realised_cost.max(0.0) });
    }
}
