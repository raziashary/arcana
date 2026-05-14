// sidecar/client.rs — HTTP client for AutoDev ↔ core-orchestrator communication.
//
// Workers dispatch through core-orchestrator's LivingAgent instead of the Rust
// AgentEngine to gain: 21 FSMs, mem0 semantic memory, A2A protocol, litellm
// multi-provider routing, and context scoring. Falls back gracefully to the
// Rust engine when core-orchestrator is unreachable.
//
// Auth: OrchestratorClient authenticates via /api/auth/service-login using API key,
//       receives JWT token, then sends Authorization: Bearer <token> on all requests.
//
// Endpoints used:
//   GET  /health                      — liveness probe (ops router, no auth required)
//   POST /api/auth/service-login      — authenticate with API key, get JWT
//   GET  /api/agents                  — list for slug lookup (requires JWT)
//   POST /api/agents                  — register new worker agent (requires JWT)
//   POST /api/agents/{id}/invoke      — run LivingAgent, returns AgentInvokeResponse (requires JWT)

use std::collections::HashMap;
use std::sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, Arc, Mutex};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::errors::{AutoDevError, Result};

/// Result of a Mode 2 LivingAgent invocation. Carries both the response content
/// and an approximation of token usage so the recipe runner can record analytics
/// and update the cost meter without modifying core-orchestrator.
#[derive(Debug, Clone)]
pub struct WorkerInvokeResult {
    pub content:    String,
    pub tokens_in:  u32,
    pub tokens_out: u32,
    /// Model reported by core-orchestrator's response metadata (if any).
    /// May be in core-orchestrator's normalized key format (e.g. "anthropic_claude_sonnet").
    pub model:      Option<String>,
}

/// Extract token usage from core-orchestrator's `response.metadata.tokens_used`,
/// with a char-length fallback when the field is missing or zero.
///
/// `tokens_used` is typed `Optional[int]` in the submodule but actually receives
/// a dict `{prompt_tokens, completion_tokens, total_tokens, cached_tokens}`
/// (living_agent.py:1948). Pydantic may coerce, drop, or pass the dict through —
/// we handle every shape we've seen.
///
/// Fallback estimate uses 1 token ≈ 4 chars and adds a 4× context-multiplier on
/// the input side to account for the system prompt, tools schema, mem0 recall,
/// and previous-turn context that LivingAgent layers on top of our recipe prompt.
/// This is approximate but proportional — far better than the previous zero.
fn extract_usage(
    tokens_used: Option<&serde_json::Value>,
    prompt_chars: usize,
    response_chars: usize,
) -> (u32, u32) {
    // Try dict shape first: {prompt_tokens, completion_tokens, total_tokens}
    if let Some(v) = tokens_used {
        if let Some(obj) = v.as_object() {
            let pin  = obj.get("prompt_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let pout = obj.get("completion_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            if pin > 0 || pout > 0 {
                debug!("[extract_usage] branch=dict_split prompt={pin} completion={pout}");
                return (pin, pout);
            }
            // Some providers only report total_tokens; split 70/30 in favor of input.
            let total = obj.get("total_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            if total > 0 {
                let pin = (total as f64 * 0.7) as u32;
                debug!("[extract_usage] branch=dict_total_70_30 total={total} pin={pin} pout={}", total - pin);
                return (pin, total - pin);
            }
        }
        // Int shape (matches the Pydantic schema literally).
        if let Some(total) = v.as_u64() {
            let total = total as u32;
            if total > 0 {
                let pin = (total as f64 * 0.7) as u32;
                debug!("[extract_usage] branch=int_total_70_30 total={total} pin={pin} pout={}", total - pin);
                return (pin, total - pin);
            }
        }
    }
    // Char-length fallback — see doc comment above for the multiplier rationale.
    // Doc 02 Phase 2 task 2.3: log which branch fired so future audits can tell
    // whether real `tokens_used` is arriving from core-orchestrator or whether
    // the char/4 fallback is doing all the work (the answer determines whether
    // billing numbers are accurate or merely proportional).
    let est_in  = ((prompt_chars / 4) as u32).saturating_mul(4);
    let est_out = (response_chars / 4) as u32;
    debug!("[extract_usage] branch=char_fallback prompt_chars={prompt_chars} response_chars={response_chars} est_in={est_in} est_out={est_out}");
    (est_in, est_out)
}

const BASE_URL: &str = "http://127.0.0.1:4000";
const MEMORY_URL: &str = "http://127.0.0.1:8000";  // memory-mcp-server base URL
const API_KEY: &str = "dev-key-123";               // matches core-orchestrator .env API_KEYS[0]
// Phase 8.3 — bumped from 5s to 10s so a busy event loop running 3 parallel
// LivingAgent invokes still has time to schedule the lightweight /health route.
const HEALTH_TIMEOUT_SECS: u64 = 10;
const INVOKE_TIMEOUT_SECS: u64 = 600;              // 10-minute ceiling per recipe tick

// Phase 8.3 — Use the lightweight /health route (api_gateway/modules/ops/router.py)
// instead of the heavy /api/system/health route which calls
// SystemMCPServer._get_system_health() and competes with active LivingAgent
// invokes for event-loop time. /health is a trivial 4-line handler that just
// returns {"status":"ok","time":now} with NO dependencies, so it answers
// instantly even while the orchestrator is mid-invoke. Auth middleware exempts
// both paths (see api_gateway/api/middleware/auth.py:44).
const HEALTH_PATH: &str = "/health";

// Phase 8.3 — consecutive-failure debounce threshold.
// is_healthy() must see this many consecutive failures before returning false.
// Bumped from 3 → 6 so a single tight-CPU burst (e.g. supervisor decomposing
// 11 tasks in one tick) cannot prematurely flip every worker to Mode 1.
// At 3s polling cadence, 6 misses = ~18s of true unresponsiveness required
// before the fleet falls back.
const HEALTH_FAIL_THRESHOLD: usize = 6;

// ─── OrchestratorClient ───────────────────────────────────────────────────────

/// HTTP client for worker dispatch through core-orchestrator's LivingAgent.
///
/// Maintains an in-memory slug → agent_id cache so each recipe tick avoids
/// a redundant GET /api/agents round-trip after the first registration.
///
/// Authentication flow:
/// 1. Call /api/auth/service-login with API key to obtain JWT
/// 2. Cache JWT and use Authorization: Bearer <token> on all API requests
/// 3. If token expires, re-authenticate automatically
pub struct OrchestratorClient {
    client:      reqwest::Client,
    base_url:    String,
    api_key:     String,
    jwt_token:   Mutex<Option<String>>,           // cached JWT token from service-login
    // worker_slug → (core-orchestrator agent_id, last-applied llm_model).
    // The model component lets us detect when the Tauri-side settings
    // (or per-worker override in workers.db) have changed so we can
    // push the new model into the agent's YAML config before the next
    // invoke, keeping Mode 2 in sync with Mode 1 without restart.
    agent_cache: Mutex<HashMap<String, (String, String)>>,
    // registry_key (short name) → full API model name. Fetched lazily from
    // core-orchestrator GET /api/llm/models and used by the Rust AgentEngine
    // fallback to map config keys (e.g. "claude-4-5-haiku") to canonical
    // LiteLLM names (e.g. "anthropic/claude-haiku-4-5-20251001").
    model_registry_cache: Mutex<HashMap<String, String>>,
    // Phase 7.8 — consecutive-failure debounce.
    // is_healthy() increments on failure, resets on success. Only after
    // HEALTH_FAIL_THRESHOLD consecutive failures does it return false so a
    // single timeout (orch busy with a long LivingAgent call) doesn't
    // unnecessarily trigger a Mode 1 fallback.
    health_fail_count: Arc<AtomicUsize>,
    // G.1 — startup grace window. core-orchestrator takes ~18s to boot.
    // Suppress WARN-level logging for health failures during this window
    // to eliminate the 59-failure noise seen in the May 5 run logs.
    startup_grace_until: std::time::Instant,
    // Fleet-ready latch — flipped the first time a health check returns
    // *truly* OK (real 2xx response, not a grace-window optimistic true).
    // Once set it never resets, even if orch later crashes — the watchdog
    // and existing failure logic handle subsequent state. The recipe
    // runner reads this to gate its very first dispatch tick, killing
    // the burst of `ensure_agent failed: service-login` warnings that
    // happens when workers fire while orch is still booting.
    first_health_success: Arc<AtomicBool>,
}

impl OrchestratorClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(INVOKE_TIMEOUT_SECS))
            .pool_max_idle_per_host(1)
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest::Client build failed (should not happen)");
        Self {
            client,
            base_url:    BASE_URL.into(),
            api_key:     API_KEY.into(),
            jwt_token:   Mutex::new(None),
            agent_cache: Mutex::new(HashMap::new()),
            model_registry_cache: Mutex::new(HashMap::new()),
            health_fail_count: Arc::new(AtomicUsize::new(0)),
            // 30s grace: core-orchestrator typically boots in ~18s.
            // Failures logged at DEBUG during this window to keep the log clean.
            startup_grace_until: std::time::Instant::now()
                + std::time::Duration::from_secs(30),
            first_health_success: Arc::new(AtomicBool::new(false)),
        }
    }

    /// True once `is_healthy()` or `is_healthy_strict()` has observed at
    /// least one real 2xx /health response. The recipe runner uses this to
    /// hold the first dispatch tick until core-orchestrator has *actually*
    /// finished booting — avoiding the cold-start burst of failed
    /// service-login calls when workers fire during the grace window.
    pub fn has_ever_been_healthy(&self) -> bool {
        self.first_health_success.load(Ordering::Acquire)
    }

    /// Record a confirmed-healthy probe. Called from both `is_healthy()`
    /// and `is_healthy_strict()` on real 2xx success. Idempotent; ignores
    /// repeat calls so we don't pay an atomic store on every poll.
    fn record_health_success(&self) {
        if !self.first_health_success.load(Ordering::Acquire) {
            self.first_health_success.store(true, Ordering::Release);
            info!("[OrchestratorClient] core-orchestrator confirmed healthy — fleet dispatch latch released");
        }
    }

    // ─── Authentication ───────────────────────────────────────────────────────

    /// Authenticate with core-orchestrator using API key to obtain JWT token.
    ///
    /// This exchanges the configured API key for a time-limited JWT that must
    /// be sent as Authorization: Bearer <token> on all subsequent API requests.
    pub async fn authenticate(&self) -> Result<()> {
        let login_url = format!("{}/api/auth/service-login", self.base_url);
        
        let body = serde_json::json!({
            "api_key": self.api_key,
        });

        let resp = self.client
            .post(&login_url)
            .json(&body)
            .timeout(Duration::from_secs(HEALTH_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| AutoDevError::Agent(format!("POST /api/auth/service-login: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AutoDevError::Agent(format!(
                "service login failed (HTTP {status}): {}",
                &body[..body.len().min(400)]
            )));
        }

        let parsed: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AutoDevError::Agent(format!("service login response parse: {e}")))?;

        let token = parsed["token"]
            .as_str()
            .ok_or_else(|| AutoDevError::Agent("service login response missing 'token'".into()))?;

        // Cache the JWT token
        *self.jwt_token.lock().unwrap() = Some(token.to_string());
        debug!("[OrchestratorClient] authenticated successfully, JWT cached");

        Ok(())
    }

    /// Ensure client is authenticated before making API calls.
    ///
    /// If no JWT token is cached, attempts to authenticate. Call this before
    /// any request that requires authentication.
    async fn ensure_authenticated(&self) -> Result<()> {
        let has_token = self.jwt_token.lock().unwrap().is_some();
        if !has_token {
            self.authenticate().await?;
        }
        Ok(())
    }

    /// Get the Authorization header value for API requests.
    ///
    /// Returns "Bearer <token>" if authenticated, or falls back to empty
    /// (which will fail for protected routes - caller should ensure auth first).
    fn get_auth_header(&self) -> String {
        self.jwt_token
            .lock()
            .unwrap()
            .as_ref()
            .map(|t| format!("Bearer {}", t))
            .unwrap_or_default()
    }

    // ─── Fleet Verification ──────────────────────────────────────────────────

    /// Expected fleet agents for a fully-booted AutoDev system.
    const FLEET_AGENTS: [&'static str; 7] = [
        "supervisor",
        "qc-agent",
        "worker-backend",
        "worker-frontend",
        "worker-mcp",
        "worker-platform",
        "worker-cleanup",
    ];

    /// Verify that all 7 fleet agents are registered in core-orchestrator.
    /// 
    /// Returns `(true, Vec<agent_name>)` if all agents exist, 
    /// or `(false, Vec<missing_agent_names>)` if some are missing.
    /// Used by the startup progress FSM to gate "FleetReady" phase.
    pub async fn verify_fleet_ready(&self) -> (bool, Vec<String>) {
        if !self.ensure_authenticated().await.is_ok() {
            return (false, Self::FLEET_AGENTS.iter().map(|s| s.to_string()).collect());
        }

        let list_url = format!("{}/api/agents", self.base_url);
        let list_resp = self.client
            .get(&list_url)
            .header("Authorization", self.get_auth_header())
            .send()
            .await;

        let agents: Vec<serde_json::Value> = match list_resp {
            Ok(resp) if resp.status().is_success() => {
                let raw: serde_json::Value = resp.json().await.unwrap_or_default();
                parse_agents_list(raw)
            }
            _ => return (false, Self::FLEET_AGENTS.iter().map(|s| s.to_string()).collect()),
        };

        let registered_names: std::collections::HashSet<String> = agents
            .iter()
            .filter_map(|a| a["name"].as_str().map(|s| s.to_string()))
            .collect();

        let missing: Vec<String> = Self::FLEET_AGENTS
            .iter()
            .filter(|&&name| !registered_names.contains(name))
            .map(|&name| name.to_string())
            .collect();

        let ready = missing.is_empty();
        if ready {
            debug!("[OrchestratorClient] Fleet verification passed — all 7 agents registered");
        } else {
            debug!("[OrchestratorClient] Fleet verification pending — missing: {:?}", missing);
        }

        (ready, missing)
    }

    /// Fetch a `slug → llm_model` map from the running core-orchestrator.
    ///
    /// This is the most reliable source of truth for "what model is this agent
    /// actually using right now" — it bypasses YAML reads (which depend on path
    /// resolution) and reflects any live PUT /config writes immediately.
    ///
    /// Returns an empty map on any failure (not authenticated, network error,
    /// orchestrator down). Callers should fall back to their stored DB model
    /// value in that case.
    pub async fn fetch_agent_models(&self) -> std::collections::HashMap<String, String> {
        let mut out = std::collections::HashMap::new();
        if self.ensure_authenticated().await.is_err() {
            return out;
        }
        let list_url = format!("{}/api/agents", self.base_url);
        let resp = match self.client
            .get(&list_url)
            .header("Authorization", self.get_auth_header())
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            _ => return out,
        };
        let raw: serde_json::Value = resp.json().await.unwrap_or_default();
        for agent in parse_agents_list(raw) {
            let name = agent["name"].as_str().unwrap_or("");
            let model = agent["llm_model"].as_str().unwrap_or("");
            if !name.is_empty() && !model.is_empty() {
                out.insert(name.to_string(), model.to_string());
            }
        }
        out
    }

    // ─── Health ──────────────────────────────────────────────────────────────

    /// Returns true if core-orchestrator responds with 2xx within HEALTH_TIMEOUT_SECS.
    ///
    /// Phase 7.8 — consecutive-failure debounce: a single timeout (e.g. orch busy
    /// processing a long LivingAgent call) is NOT enough to declare the orchestrator
    /// down. Only after HEALTH_FAIL_THRESHOLD (3) consecutive failures does this
    /// method return false and trigger a Mode 1 fallback.
    ///
    /// On success the counter is reset to 0, so a recovered orch is immediately
    /// treated as healthy on the next check.
    /// Strict version of `is_healthy` that returns false on any failure —
    /// no grace window, no "stay optimistic". Used for the initial "is the
    /// orchestrator already running externally?" probe at app startup,
    /// where being wrong leads us to wrongly mark the sidecar as Running
    /// and queue worker dispatches that all fail at auth-login.
    pub async fn is_healthy_strict(&self) -> bool {
        let url = format!("{}{}", self.base_url, HEALTH_PATH);
        match self.client
            .get(&url)
            .timeout(Duration::from_secs(HEALTH_TIMEOUT_SECS))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => {
                // Trip the fleet-ready latch — this is a real, non-optimistic success.
                self.record_health_success();
                true
            }
            _ => false,
        }
    }

    /// Last-resort liveness check used when `/health` timed out: does the
    /// orchestrator's port even accept a TCP connection? If yes, the
    /// process is alive — its event loop is just temporarily saturated by
    /// a long LivingAgent invoke. Returning true here keeps the fleet on
    /// Mode 2 instead of falling back to Mode 1, which would queue parallel
    /// LLM work and starve the event loop further.
    ///
    /// 2-second budget — plenty for a localhost TCP handshake; short enough
    /// that we don't compound the perceived hang.
    async fn tcp_liveness(&self) -> bool {
        use tokio::net::TcpStream;
        use tokio::time::timeout;
        // Parse "http://127.0.0.1:4000" → "127.0.0.1:4000"
        let addr = self.base_url
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        matches!(
            timeout(Duration::from_secs(2), TcpStream::connect(addr)).await,
            Ok(Ok(_))
        )
    }

    pub async fn is_healthy(&self) -> bool {
        let url = format!("{}{}", self.base_url, HEALTH_PATH);
        let result = self.client
            .get(&url)
            .timeout(Duration::from_secs(HEALTH_TIMEOUT_SECS))
            .send()
            .await;

        let in_grace = std::time::Instant::now() < self.startup_grace_until;

        match result {
            Ok(r) if r.status().is_success() => {
                // Reset the debounce counter on any success.
                self.health_fail_count.store(0, Ordering::Relaxed);
                // Trip the fleet-ready latch (idempotent after first success).
                self.record_health_success();
                true
            }
            Ok(r) => {
                // Non-2xx response — treat as a failure
                let count = self.health_fail_count.fetch_add(1, Ordering::Relaxed) + 1;
                // G.1 — during grace window, downgrade to DEBUG to avoid startup noise
                if in_grace {
                    debug!(
                        "[OrchestratorClient] health check returned HTTP {} during startup grace \
                         ({}/{} consecutive failures)",
                        r.status(), count, HEALTH_FAIL_THRESHOLD
                    );
                } else {
                    debug!(
                        "[OrchestratorClient] health check returned HTTP {} (consecutive failures: {}/{})",
                        r.status(), count, HEALTH_FAIL_THRESHOLD
                    );
                }
                // Still optimistic below threshold; unhealthy once threshold is reached.
                count < HEALTH_FAIL_THRESHOLD
            }
            Err(e) => {
                let count = self.health_fail_count.fetch_add(1, Ordering::Relaxed) + 1;

                if in_grace {
                    // G.1 — during the startup grace window, always return true regardless
                    // of the failure count. The orchestrator is still booting (~18s); we
                    // must not let 7 concurrent worker startup ticks exhaust the threshold
                    // and permanently flip all workers to Mode 1 before the server is ready.
                    debug!(
                        "[OrchestratorClient] health check failed during startup grace \
                         ({}/{HEALTH_FAIL_THRESHOLD}): {e} — staying optimistic",
                        count
                    );
                    true
                } else if count < HEALTH_FAIL_THRESHOLD {
                    warn!(
                        "[OrchestratorClient] health check failed ({count}/{HEALTH_FAIL_THRESHOLD}): {e} — \
                         treating as healthy until threshold reached"
                    );
                    true  // still optimistic — orch may just be busy
                } else {
                    // Phase 8.3 — Before declaring orch dead, ask the network
                    // whether port 4000 still accepts TCP connections. If yes,
                    // the process is alive and just saturated by a long
                    // LivingAgent invoke (common during big supervisor ticks
                    // or 3 concurrent worker invokes). Returning true here
                    // keeps the fleet on Mode 2 — falling back to Mode 1 in
                    // this scenario would only add more concurrent LLM work
                    // and starve the event loop further.
                    if self.tcp_liveness().await {
                        if count == HEALTH_FAIL_THRESHOLD {
                            warn!(
                                "[OrchestratorClient] /health failed {count}× but TCP :4000 is alive — \
                                 keeping Mode 2; orchestrator is busy, not dead. ({e})"
                            );
                        } else {
                            debug!(
                                "[OrchestratorClient] /health failing (count={count}) but TCP alive — staying on Mode 2"
                            );
                        }
                        return true;
                    }
                    // Log once at the exact threshold crossing; suppress repeated noise
                    // for every subsequent call above the threshold.
                    if count == HEALTH_FAIL_THRESHOLD {
                        // Doc 08 Phase 2 — single legitimate kind for the
                        // watchdog circuit breaker opening. Same kind is
                        // reserved for the auto-restart path so dashboards
                        // can plot both transitions together.
                        warn!(
                            kind = crate::observability::error_kinds::KIND_WATCHDOG_RESTART,
                            fail_count = count,
                            "[OrchestratorClient] health check failed {count} consecutive times AND TCP dead: {e} — \
                             declaring unhealthy, switching workers to Mode 1 fallback"
                        );
                    } else {
                        debug!(
                            "[OrchestratorClient] health check still failing (count={count}, TCP dead): {e}"
                        );
                    }
                    false
                }
            }
        }
    }

    /// Returns true if memory-mcp-server responds with 2xx on GET /health within 3 seconds.
    pub async fn is_memory_healthy(&self) -> bool {
        let url = format!("{}/health", MEMORY_URL);
        let result = self.client
            .get(&url)
            .timeout(Duration::from_secs(HEALTH_TIMEOUT_SECS))
            .send()
            .await;

        match result {
            Ok(r)  => r.status().is_success(),
            Err(e) => {
                debug!("[OrchestratorClient] memory-mcp-server health check failed: {e}");
                false
            }
        }
    }

    /// Returns true if Redis (or Memurai on Windows) responds to a PING on
    /// `127.0.0.1:6379` within 3 seconds. Uses raw RESP over TCP to avoid
    /// pulling in a redis crate dependency — we only need liveness, not
    /// full protocol support.
    ///
    /// Phase 3.4b: core-orchestrator uses Redis for the warm memory tier and
    /// agent card store. A missing Redis does NOT block fleet startup (the
    /// Python side falls back to cold tier only) but we surface the status
    /// in the UI so users know whether the warm tier is available.
    pub async fn is_redis_healthy(&self) -> bool {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpStream;
        use tokio::time::timeout;

        let connect = timeout(
            Duration::from_secs(HEALTH_TIMEOUT_SECS),
            TcpStream::connect("127.0.0.1:6379"),
        ).await;

        let mut stream = match connect {
            Ok(Ok(s))  => s,
            Ok(Err(e)) => {
                debug!("[OrchestratorClient] Redis connect failed: {e}");
                return false;
            }
            Err(_) => {
                debug!("[OrchestratorClient] Redis connect timeout");
                return false;
            }
        };

        // RESP inline command: PING\r\n — server responds with +PONG\r\n
        if timeout(
            Duration::from_secs(HEALTH_TIMEOUT_SECS),
            stream.write_all(b"PING\r\n"),
        ).await.ok().and_then(|r| r.ok()).is_none() {
            return false;
        }

        let mut buf = [0u8; 16];
        match timeout(
            Duration::from_secs(HEALTH_TIMEOUT_SECS),
            stream.read(&mut buf),
        ).await {
            Ok(Ok(n)) if n > 0 => {
                let reply = &buf[..n];
                reply.starts_with(b"+PONG")
            }
            _ => false,
        }
    }

    // ─── Agent Registration ───────────────────────────────────────────────────

    /// Idempotent: find an existing core-orchestrator agent by worker slug, or
    /// create one with a generic AutoDev worker system prompt.
    ///
    /// Recipe instructions are NOT stored as the system prompt because they can
    /// change across deployments and the AgentUpdate API does not expose system_prompt.
    /// Instead, instructions are injected per-tick in invoke_worker's prompt body.
    pub async fn ensure_agent(&self, slug: &str, model: &str) -> Result<String> {
        // Fast-path: cached agent_id from a previous tick, but only if the
        // caller-requested model still matches what we last pushed to the
        // orchestrator. If it diverges (user changed the fleet model in
        // Settings, or a per-worker override was written to workers.db),
        // fall through to the slow-path so we can PUT /config and rewrite
        // the agent's YAML before the next invoke.
        {
            let cache = self.agent_cache.lock().unwrap();
            if let Some((id, applied_model)) = cache.get(slug) {
                if applied_model == model {
                    debug!("[OrchestratorClient] cache hit '{slug}' → {id} (model='{model}')");
                    return Ok(id.clone());
                }
                debug!(
                    "[OrchestratorClient] cache stale for '{slug}': applied='{applied_model}' \
                     requested='{model}' — will reconcile YAML"
                );
            }
        }

        // Slow-path: find-or-create, then reconcile llm_model if needed.
        let id = self.find_or_create_agent(slug, model).await?;

        // Cache (id, model) as the last-applied tuple. On the next tick
        // the fast-path above short-circuits unless the model changes again.
        self.agent_cache
            .lock()
            .unwrap()
            .insert(slug.to_string(), (id.clone(), model.to_string()));
        Ok(id)
    }

    /// Push a `PUT /api/agents/{id}/config` with the new llm_model so the
    /// agent's YAML on disk (config/agents/{slug}.yaml) is rewritten and the
    /// in-memory LivingAgent is reloaded. This is the only supported way to
    /// keep Mode 2 fleet agents pinned to the user-selected model without
    /// restarting the core-orchestrator sidecar.
    async fn update_agent_model(&self, agent_id: &str, model: &str) -> Result<()> {
        self.ensure_authenticated().await?;
        let url = format!("{}/api/agents/{}/config", self.base_url, agent_id);
        let body = serde_json::json!({ "llm_model": model });

        let resp = self.client
            .put(&url)
            .header("Authorization", self.get_auth_header())
            .json(&body)
            .send()
            .await
            .map_err(|e| AutoDevError::Agent(format!("PUT /api/agents/{agent_id}/config: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text   = resp.text().await.unwrap_or_default();
            return Err(AutoDevError::Agent(format!(
                "agent config update failed (HTTP {status}): {}",
                &text[..text.len().min(400)]
            )));
        }
        debug!("[OrchestratorClient] pushed llm_model='{model}' to agent {agent_id}");
        Ok(())
    }

    async fn find_or_create_agent(&self, slug: &str, model: &str) -> Result<String> {
        // Translate Rust/Anthropic direct-API model IDs to orchestrator registry keys.
        // The Rust app stores models as "claude-haiku-4-5" (Anthropic API format).
        // The core-orchestrator registry uses "claude-4-5-haiku" (version-model format).
        let orch_model = to_orch_model_key(model);

        // Ensure we're authenticated before making API calls
        self.ensure_authenticated().await?;

        // ── 1. Try to find existing agent by name ─────────────────────────────
        let list_url = format!("{}/api/agents", self.base_url);
        let list_resp = self.client
            .get(&list_url)
            .header("Authorization", self.get_auth_header())
            .send()
            .await
            .map_err(|e| AutoDevError::Agent(format!("GET /api/agents: {e}")))?;

        if list_resp.status().is_success() {
            // Normalise response: handles both raw `[...]` and wrapped `{"agents":[...]}`
            let raw: serde_json::Value = list_resp.json().await.unwrap_or_default();
            let agents = parse_agents_list(raw);

            for agent in &agents {
                let name = agent["name"].as_str().unwrap_or("");
                if name == slug {
                    if let Some(id) = agent["id"].as_str() {
                        // Reconcile the agent's llm_model against what the
                        // caller wants (e.g. user changed the fleet model
                        // in Settings). Writing via /config rewrites the
                        // YAML on disk and reloads the LivingAgent, so
                        // subsequent invokes route through the new model
                        // without restarting core-orchestrator.
                        let current_model = agent["llm_model"].as_str().unwrap_or("");
                        if current_model != orch_model {
                            debug!(
                                "[OrchestratorClient] agent '{slug}' model drift \
                                 '{current_model}' → '{orch_model}' — updating YAML"
                            );
                            if let Err(e) = self.update_agent_model(id, orch_model).await {
                                // Non-fatal: log and continue with the old model
                                // rather than blocking the recipe tick.
                                tracing::warn!(
                                    "[OrchestratorClient] failed to push model update \
                                     for '{slug}' ({id}): {e}"
                                );
                            }
                        } else {
                            debug!("[OrchestratorClient] found existing agent '{slug}' → {id}");
                        }
                        return Ok(id.to_string());
                    }
                }
            }
        }

        // ── 2. Agent not found — create it ───────────────────────────────────
        debug!("[OrchestratorClient] creating agent for worker '{slug}'");

        let job_title = format!(
            "AutoDev {} Worker",
            slug.split('-')
                .map(|w| {
                    let mut c = w.chars();
                    c.next()
                        .map(|ch| ch.to_uppercase().to_string() + c.as_str())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join(" ")
        );

        let system_prompt = format!(
            "You are {slug}, an autonomous AutoDev fleet worker agent running on a scheduled cron. \
             You operate in Mode 2 (Worker Agent) powered by the core-orchestrator LivingAgent. \
             You have access to AutoDev tools to manage the task board and execute assigned work. \
             Follow your recipe instructions precisely each execution tick.",
        );

        let create_body = serde_json::json!({
            "name":          slug,
            "job_title":     job_title,
            "gender":        "male",
            "description":   format!("AutoDev fleet worker agent — {slug}"),
            "llm_model":     orch_model,
            "system_prompt": system_prompt,
            "mcp_servers": ["autodev-mcp-server"],
            "internal_tools": [],
            "autodev_tools": [],
            "capabilities":  ["task_management", "code_generation", "autodev_fleet"],
        });

        let create_url = format!("{}/api/agents", self.base_url);
        let create_resp = self.client
            .post(&create_url)
            .header("Authorization", self.get_auth_header())
            .json(&create_body)
            .send()
            .await
            .map_err(|e| AutoDevError::Agent(format!("POST /api/agents: {e}")))?;

        if !create_resp.status().is_success() {
            let status = create_resp.status();
            let body   = create_resp.text().await.unwrap_or_default();

            // HTTP 500 with "already exists" means the agent's YAML file is on
            // disk in `config/agents/{slug}.yaml` but the in-memory AgentRegistry
            // hasn't loaded it yet, so neither GET /api/agents nor
            // GET /api/agents/{slug} can see it. The orchestrator's own error
            // message confirms the situation:
            //   "Agent 'qc-agent' already exists. Use PUT /api/agents/qc-agent/config
            //    to update, or DELETE /api/agents/qc-agent then recreate."
            //
            // Because the orchestrator deterministically derives `agent_id` from
            // the agent's name (slug), we can trust `agent_id == slug` here and
            // skip the GET-recovery round-trip entirely. We then fire a
            // PUT /config which both (a) reloads the YAML into the live registry
            // and (b) reconciles the model, so subsequent /invoke calls hit a
            // populated agent.
            if status.as_u16() == 500 && body.contains("already exists") {
                tracing::warn!(
                    "[OrchestratorClient] POST create returned 500 'already exists' for '{slug}' \
                     — YAML on disk but agent not in live registry; forcing load via PUT /config"
                );

                // Force-load via PUT /api/agents/{slug}/config — this rewrites the
                // YAML with our requested model AND triggers the orchestrator to
                // reload the agent into the AgentRegistry.
                if let Err(e) = self.update_agent_model(slug, orch_model).await {
                    tracing::warn!(
                        "[OrchestratorClient] force-load PUT /config for '{slug}' failed: {e} \
                         — proceeding with slug-as-id; next /invoke may 404 until orch reloads"
                    );
                } else {
                    debug!(
                        "[OrchestratorClient] force-loaded '{slug}' via PUT /config \
                         (model='{orch_model}')"
                    );
                }

                return Ok(slug.to_string());
            }

            return Err(AutoDevError::Agent(format!(
                "agent creation failed (HTTP {status}): {}",
                &body[..body.len().min(400)]
            )));
        }

        let agent: serde_json::Value = create_resp
            .json()
            .await
            .map_err(|e| AutoDevError::Agent(format!("agent create parse: {e}")))?;

        let id = agent["id"]
            .as_str()
            .ok_or_else(|| AutoDevError::Agent("agent creation response missing 'id'".into()))?
            .to_string();

        debug!("[OrchestratorClient] created agent '{slug}' → {id}");
        Ok(id)
    }

    // ─── Worker Invocation ────────────────────────────────────────────────────

    /// Invoke a registered LivingAgent with the current recipe instructions and
    /// the standard execution trigger. The instructions are injected per-tick so
    /// recipe changes take effect immediately without re-registering the agent.
    ///
    /// Returns the text content from StructuredResponse.content on success.
    pub async fn invoke_worker(
        &self,
        agent_id:     &str,
        instructions: &str,
        trigger:      &str,
    ) -> Result<WorkerInvokeResult> {
        // Ensure we're authenticated before making API calls
        self.ensure_authenticated().await?;

        // Embed instructions as a context preamble so the LivingAgent uses current directives.
        // The LivingAgent's own system prompt + mem0 memories add long-term context on top.
        let prompt = format!(
            "[AUTODEV RECIPE INSTRUCTIONS]\n{instructions}\n\n[EXECUTION TRIGGER]\n{trigger}"
        );
        let prompt_len = prompt.len();

        // Note: session_id query param removed - core-orchestrator doesn't require it
        // for stateless recipe execution and returns 404 if session doesn't exist
        let invoke_url = format!("{}/api/agents/{}/invoke", self.base_url, agent_id);

        let body = serde_json::json!({
            "prompt":   prompt,
            "file_ids": [],
            "parameters": serde_json::Value::Null,
        });

        let resp = match self.try_invoke(&invoke_url, &body, agent_id).await {
            Ok(r) => r,
            Err(e) => return Err(e),
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let text   = resp.text().await.unwrap_or_default();
            return Err(AutoDevError::Agent(format!(
                "invoke failed (HTTP {status}): {}",
                &text[..text.len().min(400)]
            )));
        }

        // Parse AgentInvokeResponse → response (StructuredResponse) → content + usage
        let parsed: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AutoDevError::Agent(format!("invoke response parse: {e}")))?;

        let content = parsed
            .get("response")
            .and_then(|r| r.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("[LivingAgent returned no content]")
            .to_string();

        // Phase 8.2 — Mode 2 cost extraction.
        // The LivingAgent's StreamBroker populates `response.metadata.tokens_used`
        // (living_agent.py:1948). The field is declared Optional[int] in the
        // submodule's Pydantic model but the actual value is a dict
        // {prompt_tokens, completion_tokens, total_tokens, cached_tokens}.
        // We try BOTH shapes defensively and fall back to a char/4 estimate
        // when the field is None/zero — accurate enough to surface non-zero
        // Mode 2 spend on the cost meter, and self-correcting once the
        // submodule's tokens_used populates reliably.
        let metadata = parsed.get("response").and_then(|r| r.get("metadata"));
        let tokens_used = metadata.and_then(|m| m.get("tokens_used"));
        let model = metadata
            .and_then(|m| m.get("model"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let (tokens_in, tokens_out) = extract_usage(tokens_used, prompt_len, content.len());

        Ok(WorkerInvokeResult {
            content,
            tokens_in,
            tokens_out,
            model,
        })
    }

    /// Streaming variant of `invoke_worker` for Mode 2 main-worker ticks.
    ///
    /// Doc 01 Phase 3+ / Doc 06 Phase 3 follow-up: the original
    /// `invoke_worker` issues a unary POST to `/api/agents/{id}/invoke` and
    /// returns only the final response — meaning the Live Monitor card's
    /// per-tool body stays blank for the whole tick. We were forced to defer
    /// "main worker tool-call streaming" because all per-tool visibility
    /// flowed through the subagent SSE channel.
    ///
    /// This method opens an SSE stream against
    /// `/api/agents/{id}/invoke/stream` (already exposed by core-orchestrator
    /// — bridge.rs uses it for explicitly-spawned subagents), parses frames
    /// using the existing `subagent::bridge::parse_event_frame`, and re-emits
    /// each parsed `SubagentEvent` on the **same** `subagent:event` channel
    /// using the worker slug as `subagent_id`. The frontend's
    /// `agentLiveStore.subscribeToEvents` listener already routes that
    /// channel into `handleToolCallStart` / `handleToolCallResult`, so the
    /// main worker card lights up with the same fidelity as a subagent
    /// without any new frontend wiring.
    ///
    /// Returns the same `WorkerInvokeResult` shape as `invoke_worker` so the
    /// caller (recipes::runner) is a drop-in swap — final text comes from
    /// the SSE `final` frame (falling back to accumulated tokens if the
    /// stream closes without one), and token usage comes from the SSE
    /// `cost` frame (falling back to the char/4 heuristic via
    /// `extract_usage`).
    pub async fn invoke_worker_streaming(
        &self,
        agent_id:     &str,
        instructions: &str,
        trigger:      &str,
        worker_slug:  &str,
    ) -> Result<WorkerInvokeResult> {
        use crate::events::{emit_subagent_event_global, SubagentEventPayload};
        use crate::subagent::bridge::{find_frame_end, parse_event_frame};
        use crate::subagent::SubagentEvent;
        use futures::StreamExt;

        self.ensure_authenticated().await?;

        let prompt = format!(
            "[AUTODEV RECIPE INSTRUCTIONS]\n{instructions}\n\n[EXECUTION TRIGGER]\n{trigger}"
        );
        let prompt_len = prompt.len();

        debug!(
            "[OrchestratorClient] invoke_worker_streaming agent='{agent_id}' \
             slug='{worker_slug}' prompt_len={prompt_len}"
        );

        // The SSE endpoint is the same one the subagent bridge uses. Re-use
        // it directly so the protocol stays in one place.
        let resp = self.invoke_agent_stream_raw(agent_id, &prompt).await?;

        let mut byte_stream = resp.bytes_stream();
        let mut buf = Vec::<u8>::new();

        // Accumulators — mirror SubagentBridge::consume_stream behaviour.
        let mut accumulated_text = String::new();
        let mut last_final_text  = String::new();
        let mut cost_usd:   f64 = 0.0;
        let mut tokens_in:  u32 = 0;
        let mut tokens_out: u32 = 0;
        // model_seen — currently the SSE stream from core-orchestrator
        // doesn't carry a `model` field per-frame, so we leave this None
        // and let the runner re-resolve from the worker's configured model.
        // Keeping the slot makes it trivial to wire if the upstream adds it.
        let model_seen: Option<String> = None;

        while let Some(chunk_res) = byte_stream.next().await {
            let chunk = chunk_res
                .map_err(|e| AutoDevError::Agent(format!("SSE chunk read: {e}")))?;
            buf.extend_from_slice(&chunk);

            loop {
                let Some(end) = find_frame_end(&buf) else { break };
                let frame_bytes: Vec<u8> = buf.drain(..end).collect();
                let trailer_len = if buf.starts_with(b"\r\n\r\n") || buf.starts_with(b"\n\n") {
                    if buf.starts_with(b"\r\n\r\n") { 4 } else { 2 }
                } else { 0 };
                buf.drain(..trailer_len);

                let frame = String::from_utf8_lossy(&frame_bytes).to_string();
                if frame.trim().is_empty() { continue; }

                // Parse using the bridge's existing logic but key the resulting
                // event by the *worker slug*, not the agent_id. The frontend
                // listener uses `subagent_id` as the agent_id when routing
                // into the Live Monitor card store, so the worker's card is
                // the one that fills in.
                if let Some(evt) = parse_event_frame(&frame, worker_slug) {
                    match &evt {
                        SubagentEvent::Token { text, .. } => {
                            accumulated_text.push_str(text);
                        }
                        SubagentEvent::Final { text, .. } => {
                            last_final_text = text.clone();
                        }
                        SubagentEvent::Cost { cost_usd: c, tokens_in: ti, tokens_out: to_, .. } => {
                            cost_usd  = *c;
                            tokens_in = *ti;
                            tokens_out = *to_;
                        }
                        _ => {}
                    }
                    emit_subagent_event_global(SubagentEventPayload {
                        subagent_id: worker_slug.to_string(),
                        event:       evt,
                    });
                }
            }
        }

        // Final text: prefer the explicit `final` frame; fall back to the
        // accumulated token stream. The latter is what the user already
        // sees on the card, so the recipe runner getting it back keeps
        // record_token_usage / qc_review reconciliation in sync.
        let content = if !last_final_text.is_empty() {
            last_final_text
        } else if !accumulated_text.is_empty() {
            accumulated_text
        } else {
            "[LivingAgent stream returned no content]".to_string()
        };

        // If the orchestrator didn't emit a `cost` frame, fall back to the
        // same char/4 heuristic invoke_worker uses so analytics stay
        // non-zero (and debug logs continue to expose which branch fired).
        let (final_tokens_in, final_tokens_out) = if tokens_in == 0 && tokens_out == 0 {
            extract_usage(None, prompt_len, content.len())
        } else {
            (tokens_in, tokens_out)
        };

        // `cost_usd` is currently surfaced via record_token_usage downstream;
        // we don't return it through WorkerInvokeResult because invoke_worker
        // doesn't either — the runner recomputes cost from the (tokens_in,
        // tokens_out, model) tuple via crate::analytics::cost::compute_cost.
        let _ = cost_usd;

        Ok(WorkerInvokeResult {
            content,
            tokens_in:  final_tokens_in,
            tokens_out: final_tokens_out,
            model:      model_seen,
        })
    }

    // ─── Retry wrapper for invoke_worker (Fix 6b) ─────────────────────────────

    /// Send the invoke POST with a single retry on connection-level errors.
    /// Windows silently resets idle pooled connections (WinError 10054); a fresh
    /// retry usually succeeds because the OS has already re-established the
    /// loopback socket by the time we wake up.
    async fn try_invoke(
        &self,
        url: &str,
        body: &serde_json::Value,
        agent_id: &str,
    ) -> Result<reqwest::Response> {
        let req_builder = self.client
            .post(url)
            .header("Authorization", self.get_auth_header())
            .json(body);

        match req_builder.try_clone() {
            None => {
                // Cannot clone (e.g. streaming body) — single-shot, no retry
                return req_builder
                    .send()
                    .await
                    .map_err(|e| AutoDevError::Agent(format!("POST /api/agents/{agent_id}/invoke: {e}")));
            }
            Some(cloned) => {
                match cloned.send().await {
                    Ok(resp) => return Ok(resp),
                    Err(e) if e.is_connect() || e.is_timeout() => {
                        tracing::warn!(
                            "[OrchestratorClient] invoke connection error for '{agent_id}' ({e}), retrying in 100ms"
                        );
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        match req_builder.send().await {
                            Ok(resp) => Ok(resp),
                            Err(e2) => Err(AutoDevError::Agent(format!(
                                "POST /api/agents/{agent_id}/invoke (retry failed): {e2}"
                            ))),
                        }
                    }
                    Err(e) => Err(AutoDevError::Agent(format!(
                        "POST /api/agents/{agent_id}/invoke: {e}"
                    ))),
                }
            }
        }
    }

    // ─── Model Registry Cache (Fix 6d) ────────────────────────────────────────

    /// Resolve a short model config key to the canonical API model name.
    ///
    /// On first call, lazily fetches `GET /api/llm/models` from core-orchestrator,
    /// which returns `[{ "registry_key": "...", "name": "..." }, …]` and
    /// populates `model_registry_cache`. Subsequent calls are in-memory.
    ///
    /// If core-orchestrator is unreachable or the key is not found, returns
    /// `None` so the caller can fall back to the raw key.
    pub async fn resolve_model_name(&self, key: &str) -> Option<String> {
        {
            let cache = self.model_registry_cache.lock().unwrap();
            if let Some(name) = cache.get(key) {
                debug!("[OrchestratorClient] model cache hit '{key}' → {name}");
                return Some(name.clone());
            }
        }
        // Cache miss — fetch from core-orchestrator
        if self.ensure_authenticated().await.is_err() {
            debug!("[OrchestratorClient] cannot fetch model registry: not authenticated");
            return None;
        }
        let url = format!("{}/api/llm/models", self.base_url);
        match self.client
            .get(&url)
            .header("Authorization", self.get_auth_header())
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                let models: Vec<serde_json::Value> = resp.json().await.unwrap_or_default();
                let mut cache = self.model_registry_cache.lock().unwrap();
                for m in &models {
                    let rkey = m["registry_key"].as_str().unwrap_or("");
                    let name = m["name"].as_str().unwrap_or("");
                    if !rkey.is_empty() && !name.is_empty() {
                        cache.insert(rkey.to_string(), name.to_string());
                    }
                }
                cache.get(key).cloned()
            }
            Ok(resp) => {
                debug!("[OrchestratorClient] model registry fetch returned HTTP {}", resp.status());
                None
            }
            Err(e) => {
                debug!("[OrchestratorClient] model registry fetch failed: {e}");
                None
            }
        }
    }

    // ─── Cache Management ─────────────────────────────────────────────────────

    /// Clear the slug → agent_id cache. Call after core-orchestrator restarts so
    /// agents are re-registered/re-discovered on the next recipe tick.
    pub fn clear_cache(&self) {
        self.agent_cache.lock().unwrap().clear();
        debug!("[OrchestratorClient] agent cache cleared");
    }

    // ─── Immediate Model Update (UI-triggered) ────────────────────────────────

    /// Immediately update an agent's model in core-orchestrator.
    /// 
    /// Unlike the lazy update in `find_or_create_agent` (which waits for the
    /// next recipe tick), this actively looks up the agent by slug and pushes
    /// the model change immediately. Used when the user changes a worker's
    /// model in the Settings UI — they get immediate feedback via toast.
    ///
    /// Returns `Ok(true)` if the update was applied,
    /// `Ok(false)` if the agent doesn't exist yet (will be created on next tick),
    /// or `Err` if the API call failed.
    pub async fn update_agent_model_by_slug(&self, slug: &str, model: &str) -> Result<bool> {
        self.ensure_authenticated().await?;

        // Translate to orchestrator registry key before comparing or pushing.
        // `model` arrives in Anthropic API format ("claude-sonnet-4-5");
        // `llm_model` from GET /api/agents is in orch format ("claude-4-5-sonnet").
        // Without this translation the equality check always fails and we push a
        // no-op update every single time the function is called.
        let orch_model = to_orch_model_key(model);

        // Look up agent by name
        let list_url = format!("{}/api/agents", self.base_url);
        let list_resp = self.client
            .get(&list_url)
            .header("Authorization", self.get_auth_header())
            .send()
            .await
            .map_err(|e| AutoDevError::Agent(format!("GET /api/agents: {e}")))?;

        if !list_resp.status().is_success() {
            return Err(AutoDevError::Agent("Failed to list agents from orchestrator".into()));
        }

        // Normalise response: handles both raw `[...]` and wrapped `{"agents":[...]}`
        let raw: serde_json::Value = list_resp.json().await.unwrap_or_default();
        let agents = parse_agents_list(raw);

        // Find agent by slug
        let agent_info = agents.iter().find(|a| {
            a["name"].as_str().map(|n| n == slug).unwrap_or(false)
        });

        let (agent_id, current_model) = match agent_info {
            Some(agent) => {
                let id = agent["id"]
                    .as_str()
                    .ok_or_else(|| AutoDevError::Agent(format!("Agent '{slug}' missing 'id' field")))?;
                let current = agent["llm_model"].as_str().unwrap_or("");
                (id.to_string(), current.to_string())
            }
            None => {
                // Agent doesn't exist yet — will be created with correct model on next tick
                debug!("[OrchestratorClient] agent '{slug}' not found — will be created with model '{model}' on next tick");
                return Ok(false);
            }
        };

        // Only update if model actually changed (compare in orch key format)
        if current_model == orch_model {
            debug!("[OrchestratorClient] agent '{slug}' already on model '{orch_model}' — no change needed");
            return Ok(true);
        }

        // Push the model update using the translated key
        info!("[OrchestratorClient] Pushing model change for '{slug}': '{current_model}' → '{orch_model}'");
        self.update_agent_model(&agent_id, orch_model).await?;

        // Update cache to reflect new model
        self.agent_cache
            .lock()
            .unwrap()
            .insert(slug.to_string(), (agent_id, orch_model.to_string()));

        Ok(true)
    }

    // ─── MCP Server Registration ────────────────────────────────────────────────

    /// Register the in-process AutoDev MCP Server with core-orchestrator's
    /// MCP configuration so LivingAgents can discover tools from it.
    /// Called once after core-orchestrator is healthy (in main.rs startup).
    pub async fn register_mcp_server(&self) -> Result<()> {
        if self.ensure_authenticated().await.is_err() {
            return Err(AutoDevError::Agent(
                "Cannot register MCP server: not authenticated".into()
            ));
        }

        let url = format!("{}/api/mcp/servers", self.base_url);
        // Read the actual bound port from the in-process MCP server so the
        // registration follows the port fallback applied at startup (8767 →
        // 8768/8769/8770 when 8767 was held by a previous instance).
        let mcp_port = crate::mcp_server::current_mcp_port();
        let mcp_url  = format!("http://127.0.0.1:{}/mcp", mcp_port);
        // No auth header in body — MCP server is localhost-only, no auth required.
        let body = serde_json::json!({
            "id": "autodev-mcp-server",
            "name": "AutoDev MCP Server",
            "type": "autodev",
            "transport": "http",
            "http": {
                "url": mcp_url,
                "headers": {
                    "Content-Type": "application/json"
                }
            }
        });

        let resp = self.client
            .post(&url)
            .header("Authorization", self.get_auth_header())
            .json(&body)
            .send()
            .await
            .map_err(|e| AutoDevError::Agent(format!("POST /api/mcp/servers: {e}")))?;

        let status = resp.status();
        if status.is_success() {
            info!("[OrchestratorClient] autodev-mcp-server registered with core-orchestrator on port {mcp_port}");
            return Ok(());
        }

        // If the server entry already exists on disk from a previous run,
        // POST returns 409 (or 400/500 with "already exists" body). The
        // stored URL may now be stale (port fallback after Windows TIME_WAIT
        // bumped us from 8767 → 8768), so we follow up with a PUT to
        // refresh the URL on the orchestrator side. Without this, the
        // orchestrator would happily forward LivingAgent tool calls to a
        // dead port forever.
        let code  = status.as_u16();
        let text  = resp.text().await.unwrap_or_default();
        let lower = text.to_ascii_lowercase();
        let is_already_exists = code == 409
            || ((code == 400 || code == 500)
                && (lower.contains("already exists")
                    || lower.contains("already registered")
                    || lower.contains("duplicate")));

        if !is_already_exists {
            return Err(AutoDevError::Agent(format!(
                "MCP server registration failed (HTTP {status}): {}",
                &text[..text.len().min(400)]
            )));
        }

        info!(
            "[OrchestratorClient] autodev-mcp-server already registered (HTTP {} — \
             upserting URL to refresh port to {})",
            code, mcp_port
        );
        let put_url = format!("{}/api/mcp/servers/autodev-mcp-server", self.base_url);
        let put_body = serde_json::json!({
            "transport": "http",
            "http": {
                "url": mcp_url,
                "headers": { "Content-Type": "application/json" }
            }
        });
        let put_resp = self.client
            .put(&put_url)
            .header("Authorization", self.get_auth_header())
            .json(&put_body)
            .send()
            .await;
        match put_resp {
            Ok(r) if r.status().is_success() => {
                info!("[OrchestratorClient] autodev-mcp-server URL refreshed to {}", mcp_url);
                Ok(())
            }
            Ok(r) => {
                // PUT failed — log but don't fail the whole startup. If the
                // port hadn't actually changed, the existing entry is still
                // correct. The next /invoke will surface the real problem.
                let put_status = r.status();
                let put_text = r.text().await.unwrap_or_default();
                warn!(
                    "[OrchestratorClient] PUT refresh failed (HTTP {}): {} — \
                     proceeding with existing URL; this is fine unless the port changed",
                    put_status, &put_text[..put_text.len().min(200)]
                );
                Ok(())
            }
            Err(e) => {
                warn!(
                    "[OrchestratorClient] PUT refresh transport error: {} — \
                     proceeding with existing URL", e
                );
                Ok(())
            }
        }
    }

    // ─── Pre-flight MCP health check (BUG-002b safety net) ───────────────────

    /// Lightweight TCP probe to the in-process MCP server. Returns true if
    /// the port is accepting connections — a proxy for "LivingAgent will be
    /// able to discover tools". Uses `current_mcp_port()` so the probe
    /// targets the actual bound port (8767 by default, may fall back to
    /// 8768/8769/8770 after a Windows TIME_WAIT collision).
    ///
    /// Short-circuits to `true` when the in-process server has not finished
    /// binding yet — the runner uses this signal to gate Mode 2 dispatch,
    /// and reporting false here would force an unnecessary Mode 1 fallback
    /// for the first few seconds of every cold start.
    ///
    /// Used by `runner.rs` to abort a tick when the MCP server is down,
    /// avoiding an expensive tool-less LivingAgent tick.
    pub async fn mcp_server_healthy(&self) -> bool {
        use tokio::net::TcpStream;
        use tokio::time::timeout;

        // Bind not finished yet — optimistically allow the tick. The next
        // failure will be the orchestrator's invoke call, which is handled.
        if !crate::mcp_server::is_ready() {
            debug!("[OrchestratorClient] MCP server not yet bound — assuming healthy for this tick");
            return true;
        }

        let port = crate::mcp_server::current_mcp_port();
        let addr = format!("127.0.0.1:{}", port);
        match timeout(
            Duration::from_secs(2),
            TcpStream::connect(&addr),
        ).await {
            Ok(Ok(_)) => {
                debug!("[OrchestratorClient] MCP server TCP probe OK :{}", port);
                true
            }
            Ok(Err(e)) => {
                warn!("[OrchestratorClient] MCP server TCP connect failed :{} — {}", port, e);
                false
            }
            Err(_) => {
                warn!("[OrchestratorClient] MCP server TCP connect timed out :{} (>2s)", port);
                false
            }
        }
    }

    // ─── Phase 4.9 — Ephemeral Subagent Lifecycle ────────────────────────────
    //
    // Subagents reuse the persistent-agent CRUD endpoints with a
    // create-invoke(stream)-delete lifecycle. See:
    //   docs/development/PHASE_4_9_EPHEMERAL_VIA_LIFECYCLE.md
    //
    // The slug must already be unique (generated by `subagent::slug::generate`).
    // create_ephemeral_agent returns the registered agent_id on success.

    /// Create a transient subagent persona in core-orchestrator.
    ///
    /// Writes a YAML config to `config/agents/{slug}.yaml` and registers a
    /// LivingAgent in the registry. Caller is responsible for `delete_agent`
    /// when the subtask completes (or relies on `SubagentManager`'s RAII /
    /// orphan-cleanup safety nets).
    pub async fn create_ephemeral_agent(
        &self,
        slug:           &str,
        system_prompt:  &str,
        llm_model:      &str,
        mcp_servers:    &[String],
        capabilities:   &[String],
    ) -> Result<String> {
        self.ensure_authenticated().await?;

        // Job title derived from slug; cosmetic only (visible in agent listings).
        let job_title = format!(
            "AutoDev Ephemeral Subagent ({})",
            slug.split('-').last().unwrap_or("worker")
        );

        let create_body = serde_json::json!({
            "name":          slug,
            "job_title":     job_title,
            "gender":        "male",
            "description":   format!("Ephemeral subagent — auto-deleted after subtask completes"),
            "llm_model":     llm_model,
            "system_prompt": system_prompt,
            "mcp_servers":   mcp_servers,
            "internal_tools": [],
            "autodev_tools": [],
            "capabilities":  capabilities,
        });

        let create_url = format!("{}/api/agents", self.base_url);
        let resp = self.client
            .post(&create_url)
            .header("Authorization", self.get_auth_header())
            .json(&create_body)
            .send()
            .await
            .map_err(|e| AutoDevError::Agent(format!("POST /api/agents (subagent): {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body   = resp.text().await.unwrap_or_default();
            return Err(AutoDevError::Agent(format!(
                "subagent creation failed (HTTP {status}): {}",
                &body[..body.len().min(400)]
            )));
        }

        let agent: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AutoDevError::Agent(format!("subagent create parse: {e}")))?;

        let id = agent["id"]
            .as_str()
            .ok_or_else(|| AutoDevError::Agent("subagent create response missing 'id'".into()))?
            .to_string();

        debug!("[OrchestratorClient] created ephemeral subagent '{slug}' → {id}");
        Ok(id)
    }

    /// Delete an agent (used by subagent cleanup). Idempotent: 404 is OK.
    ///
    /// Removes both the YAML config on disk and the registry entry inside
    /// core-orchestrator. Safe to call multiple times.
    pub async fn delete_agent(&self, agent_id: &str) -> Result<()> {
        self.ensure_authenticated().await?;

        let url = format!("{}/api/agents/{}", self.base_url, agent_id);
        let resp = self.client
            .delete(&url)
            .header("Authorization", self.get_auth_header())
            .timeout(Duration::from_secs(HEALTH_TIMEOUT_SECS * 2))
            .send()
            .await
            .map_err(|e| AutoDevError::Agent(format!("DELETE /api/agents/{agent_id}: {e}")))?;

        let status = resp.status();
        if status.is_success() || status.as_u16() == 404 {
            debug!("[OrchestratorClient] deleted agent '{agent_id}' (HTTP {status})");
            return Ok(());
        }

        let body = resp.text().await.unwrap_or_default();
        Err(AutoDevError::Agent(format!(
            "delete agent failed (HTTP {status}): {}",
            &body[..body.len().min(400)]
        )))
    }

    /// Open an SSE stream against `/api/agents/{id}/invoke/stream`.
    ///
    /// Returns the raw response so the caller can stream bytes and run its
    /// own SSE frame parser (see `subagent::bridge::parse_sse_stream`). We
    /// avoid pulling a dedicated SSE crate here because the parsing is small
    /// and the protocol is stable.
    pub async fn invoke_agent_stream_raw(
        &self,
        agent_id: &str,
        prompt:   &str,
    ) -> Result<reqwest::Response> {
        self.ensure_authenticated().await?;

        let url = format!("{}/api/agents/{}/invoke/stream", self.base_url, agent_id);
        let body = serde_json::json!({
            "prompt":   prompt,
            "file_ids": [],
            "parameters": serde_json::Value::Null,
        });

        let resp = self.client
            .post(&url)
            .header("Authorization", self.get_auth_header())
            .header("Accept", "text/event-stream")
            .json(&body)
            .send()
            .await
            .map_err(|e| AutoDevError::Agent(format!("POST /api/agents/{agent_id}/invoke/stream: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text   = resp.text().await.unwrap_or_default();
            return Err(AutoDevError::Agent(format!(
                "stream invoke failed (HTTP {status}): {}",
                &text[..text.len().min(400)]
            )));
        }
        Ok(resp)
    }

    /// List all currently-registered agent slugs whose names start with the
    /// `subagent-` prefix. Used by `SubagentManager::cleanup_orphans` on
    /// startup to wipe ephemeral agents abandoned by a previous process.
    pub async fn list_ephemeral_subagent_slugs(&self) -> Result<Vec<String>> {
        self.ensure_authenticated().await?;
        let list_url = format!("{}/api/agents", self.base_url);
        let resp = self.client
            .get(&list_url)
            .header("Authorization", self.get_auth_header())
            .send()
            .await
            .map_err(|e| AutoDevError::Agent(format!("GET /api/agents: {e}")))?;

        if !resp.status().is_success() {
            return Err(AutoDevError::Agent(format!(
                "list agents failed (HTTP {})",
                resp.status()
            )));
        }

        let raw: serde_json::Value = resp.json().await.unwrap_or_default();
        let slugs = parse_agents_list(raw)
            .into_iter()
            .filter_map(|a| a["name"].as_str().map(|s| s.to_string()))
            .filter(|n| n.starts_with(crate::subagent::slug::SUBAGENT_PREFIX))
            .collect();
        Ok(slugs)
    }
}

impl Default for OrchestratorClient {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Model name translation ────────────────────────────────────────────────────
//
// Most model IDs stored in workers.db / the YAML files already match the
// core-orchestrator registry keys defined in:
//   submodules/core-orchestrator/src/llm/providers/model_registry.py
//
// The only exception is the haiku 4.5 model: AutoDev's default workers were
// seeded with "claude-haiku-4-5" (Anthropic's direct-API name) but the
// orchestrator registry key is "claude-4-5-haiku".  Everything else —
// "claude-sonnet-4-5", "claude-sonnet-4-5-thinking", "deepseek-v4-pro", etc. —
// uses the registry key directly and must pass through unchanged.
fn to_orch_model_key(model: &str) -> &str {
    match model {
        // Legacy seed value → correct registry key
        "claude-haiku-4-5"          => "claude-4-5-haiku",
        "claude-haiku-4-5-thinking" => "claude-4-5-haiku-thinking",
        // Everything else is already a valid registry key; pass through as-is.
        other => other,
    }
}

// ─── Response shape normaliser ─────────────────────────────────────────────────
//
// `GET /api/agents` may return:
//   • A raw JSON array:          `[{...}, ...]`
//   • A wrapped object:          `{"agents": [{...}, ...]}`
//
// Deserialising directly as `Vec<serde_json::Value>` silently yields an empty
// vec for the wrapped format (via `unwrap_or_default()`), causing every tick to
// fall through to POST create → HTTP 500 "already exists".
// This helper normalises both shapes into a flat Vec.
fn parse_agents_list(json: serde_json::Value) -> Vec<serde_json::Value> {
    match json {
        serde_json::Value::Array(arr) => arr,
        serde_json::Value::Object(ref map) => {
            for key in &["agents", "data", "items", "results"] {
                if let Some(serde_json::Value::Array(arr)) = map.get(*key) {
                    return arr.clone();
                }
            }
            vec![]
        }
        _ => vec![],
    }
}
