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
use std::sync::Mutex;
use std::time::Duration;
use tracing::{debug, info};

use crate::errors::{AutoDevError, Result};

const BASE_URL: &str = "http://127.0.0.1:4000";
const MEMORY_URL: &str = "http://127.0.0.1:8000";  // memory-mcp-server base URL
const API_KEY: &str = "dev-key-123";               // matches core-orchestrator .env API_KEYS[0]
const HEALTH_TIMEOUT_SECS: u64 = 3;
const INVOKE_TIMEOUT_SECS: u64 = 600;              // 10-minute ceiling per recipe tick

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
}

impl OrchestratorClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(INVOKE_TIMEOUT_SECS))
            .build()
            .expect("reqwest::Client build failed (should not happen)");
        Self {
            client,
            base_url:    BASE_URL.into(),
            api_key:     API_KEY.into(),
            jwt_token:   Mutex::new(None),
            agent_cache: Mutex::new(HashMap::new()),
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
                resp.json().await.unwrap_or_default()
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

    // ─── Health ──────────────────────────────────────────────────────────────

    /// Returns true if core-orchestrator responds with 2xx within 3 seconds.
    /// Any error (connection refused, timeout) returns false — caller falls back.
    pub async fn is_healthy(&self) -> bool {
        let url = format!("{}/health", self.base_url);
        let result = self.client
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .timeout(Duration::from_secs(HEALTH_TIMEOUT_SECS))
            .send()
            .await;

        match result {
            Ok(r)  => r.status().is_success(),
            Err(e) => {
                debug!("[OrchestratorClient] health check failed: {e}");
                false
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
            let agents: Vec<serde_json::Value> = list_resp
                .json()
                .await
                .unwrap_or_default();

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
                        if current_model != model {
                            debug!(
                                "[OrchestratorClient] agent '{slug}' model drift \
                                 '{current_model}' → '{model}' — updating YAML"
                            );
                            if let Err(e) = self.update_agent_model(id, model).await {
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
            "llm_model":     model,
            "system_prompt": system_prompt,
            "mcp_servers":   [],
            "internal_tools": [],
            // Tools bridged from the Rust host into the core-orchestrator
            // LivingAgent (Mode 2). These must mirror what's registered in
            // `agent/builtin_tools/mod.rs` for Mode 1, otherwise fleet agents
            // can only do file ops in Mode 2 and cannot touch the task board —
            // which silently breaks every supervisor/worker/qc recipe.
            "autodev_tools": [
                // File / execution
                "read_file__autodev",
                "write_file__autodev",
                "edit_file__autodev",
                "bash__autodev",
                "list_directory__autodev",
                "grep__autodev",
                "glob__autodev",
                // Worker registry
                "list_workers__autodev",
                // Task board — required for supervisor, workers, qc-agent
                "list_tasks__autodev",
                "update_task_status__autodev",
                "assign_task__autodev",
                "create_task__autodev",
            ],
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
    ) -> Result<String> {
        // Ensure we're authenticated before making API calls
        self.ensure_authenticated().await?;

        // Embed instructions as a context preamble so the LivingAgent uses current directives.
        // The LivingAgent's own system prompt + mem0 memories add long-term context on top.
        let prompt = format!(
            "[AUTODEV RECIPE INSTRUCTIONS]\n{instructions}\n\n[EXECUTION TRIGGER]\n{trigger}"
        );

        // Note: session_id query param removed - core-orchestrator doesn't require it
        // for stateless recipe execution and returns 404 if session doesn't exist
        let invoke_url = format!("{}/api/agents/{}/invoke", self.base_url, agent_id);

        let body = serde_json::json!({
            "prompt":   prompt,
            "file_ids": [],
            "parameters": serde_json::Value::Null,
        });

        let resp = self.client
            .post(&invoke_url)
            .header("Authorization", self.get_auth_header())
            .json(&body)
            .send()
            .await
            .map_err(|e| AutoDevError::Agent(format!("POST /api/agents/{agent_id}/invoke: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text   = resp.text().await.unwrap_or_default();
            return Err(AutoDevError::Agent(format!(
                "invoke failed (HTTP {status}): {}",
                &text[..text.len().min(400)]
            )));
        }

        // Parse AgentInvokeResponse → response (StructuredResponse) → content
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

        Ok(content)
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

        let agents: Vec<serde_json::Value> = list_resp
            .json()
            .await
            .unwrap_or_default();

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

        // Only update if model actually changed
        if current_model == model {
            debug!("[OrchestratorClient] agent '{slug}' already on model '{model}' — no change needed");
            return Ok(true);
        }

        // Push the model update
        info!("[OrchestratorClient] Pushing model change for '{slug}': '{current_model}' → '{model}'");
        self.update_agent_model(&agent_id, model).await?;

        // Update cache to reflect new model
        self.agent_cache
            .lock()
            .unwrap()
            .insert(slug.to_string(), (agent_id, model.to_string()));

        Ok(true)
    }
}

impl Default for OrchestratorClient {
    fn default() -> Self {
        Self::new()
    }
}
