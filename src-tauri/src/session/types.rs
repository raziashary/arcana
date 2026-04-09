// Core session and message data types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub call_id:   String,
    pub tool_name: String,
    pub input:     serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id:  String,
    pub output:   String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id:         String,
    pub session_id: String,
    pub role:       MessageRole,
    pub content:    String,
    pub tool_calls: Vec<ToolCall>,
    pub tool_results: Vec<ToolResult>,
    // Token/cost for this message (assistant messages only)
    pub tokens_in:  u32,
    pub tokens_out: u32,
    pub cost_usd:   f64,
    pub created_at: DateTime<Utc>,
}

impl Message {
    pub fn new_user(session_id: &str, content: &str) -> Self {
        Self {
            id:           Uuid::new_v4().to_string(),
            session_id:   session_id.to_string(),
            role:         MessageRole::User,
            content:      content.to_string(),
            tool_calls:   vec![],
            tool_results: vec![],
            tokens_in:    0,
            tokens_out:   0,
            cost_usd:     0.0,
            created_at:   Utc::now(),
        }
    }

    pub fn new_assistant(session_id: &str) -> Self {
        Self {
            id:           Uuid::new_v4().to_string(),
            session_id:   session_id.to_string(),
            role:         MessageRole::Assistant,
            content:      String::new(),
            tool_calls:   vec![],
            tool_results: vec![],
            tokens_in:    0,
            tokens_out:   0,
            cost_usd:     0.0,
            created_at:   Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id:          String,
    pub title:       String,
    pub working_dir: String,
    pub provider:    String,
    pub model:       String,
    pub created_at:  DateTime<Utc>,
    pub updated_at:  DateTime<Utc>,
    pub messages:    Vec<Message>,
}

impl Session {
    pub fn new(working_dir: &str, provider: &str, model: &str) -> Self {
        let now = Utc::now();
        
        // Resolve working_dir to absolute path
        let resolved_dir = resolve_working_dir(working_dir);
        
        Self {
            id:          Uuid::new_v4().to_string(),
            title:       "New Session".to_string(),
            working_dir: resolved_dir,
            provider:    provider.to_string(),
            model:       model.to_string(),
            created_at:  now,
            updated_at:  now,
            messages:    vec![],
        }
    }
}

/// Resolve working directory to absolute path, handling empty, ".", and relative paths
fn resolve_working_dir(working_dir: &str) -> String {
    let path = if working_dir.is_empty() || working_dir == "." {
        // Use current directory
        env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        let p = Path::new(working_dir);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            // Resolve relative path from current directory
            env::current_dir()
                .map(|cwd| cwd.join(p))
                .unwrap_or_else(|_| p.to_path_buf())
        }
    };
    
    // Try to canonicalize (resolves .. and symlinks), fallback to as-is
    path.canonicalize()
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}
