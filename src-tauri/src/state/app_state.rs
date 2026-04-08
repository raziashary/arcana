// Shared mutable application state held behind Arc<Mutex<>> in Tauri.
// Avoid storing large data here — this is for lightweight runtime flags.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveAgent {
    pub session_id:  String,
    pub is_running:  bool,
    pub cancel_flag: bool,
}

#[derive(Debug, Default)]
pub struct AppState {
    // Active agent sessions (session_id → state)
    pub agents: HashMap<String, ActiveAgent>,

    // Currently active provider name (e.g. "anthropic")
    pub active_provider: Option<String>,

    // Active model identifier (e.g. "claude-sonnet-4-5")
    pub active_model: Option<String>,

    // Today's running cost in USD
    pub cost_today_usd: f64,

    // Today's cumulative token counts
    pub tokens_in_today:  u64,
    pub tokens_out_today: u64,

    // Custom system prompt override (if set, replaces BASE_SYSTEM_PROMPT)
    pub system_prompt_override: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            system_prompt_override: None,
            ..Self::default()
        }
    }

    pub fn cancel_agent(&mut self, session_id: &str) {
        if let Some(agent) = self.agents.get_mut(session_id) {
            agent.cancel_flag = true;
        }
    }

    pub fn is_cancelled(&self, session_id: &str) -> bool {
        self.agents
            .get(session_id)
            .map(|a| a.cancel_flag)
            .unwrap_or(false)
    }

    pub fn add_cost(&mut self, cost_usd: f64, tokens_in: u32, tokens_out: u32) {
        self.cost_today_usd   += cost_usd;
        self.tokens_in_today  += tokens_in as u64;
        self.tokens_out_today += tokens_out as u64;
    }
}
