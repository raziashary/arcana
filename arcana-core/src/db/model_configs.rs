// Model configuration database — per-model temperature, top_p, reasoning_effort, thinking.
// Storage: ~/.arcana/config.db (shared with ARCANA.md instructions).
// Phase 2.3: T4a/T4b — Reasoning Effort + Temperature/Top-P per-model config.

use crate::errors::{ArcanaError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub model_id: String,
    pub temperature: Option<f32>,         // 0.0 – 2.0
    pub top_p: Option<f32>,               // 0.0 – 1.0
    pub reasoning_effort: Option<String>, // "low" | "medium" | "high"
    pub thinking_enabled: bool,           // enable extended thinking / reasoning mode
    pub thinking_budget: Option<u32>,     // token budget for extended thinking (default 5000)
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            model_id: String::new(),
            temperature: None,
            top_p: None,
            reasoning_effort: None,
            thinking_enabled: false,
            thinking_budget: None,
        }
    }
}

// ─── Database Handle ──────────────────────────────────────────────────────────

pub struct ModelConfigsDb {
    conn: Connection,
}

impl ModelConfigsDb {
    /// Open ~/.arcana/config.db and ensure model_configs table exists.
    pub fn open() -> Result<Self> {
        // `config.db`'s schema and version belong to `db::config_db`, which
        // owns the file; this table is one of three it holds.
        Ok(Self {
            conn: crate::db::config_db::open()?,
        })
    }

    // ─── Queries ──────────────────────────────────────────────────────────────

    pub fn get(&self, model_id: &str) -> Result<Option<ModelConfig>> {
        self.conn.query_row(
            "SELECT model_id, temperature, top_p, reasoning_effort, thinking_enabled, thinking_budget \
             FROM model_configs WHERE model_id = ?1",
            params![model_id],
            |row| Ok(ModelConfig {
                model_id:         row.get(0)?,
                temperature:      row.get(1)?,
                top_p:            row.get(2)?,
                reasoning_effort: row.get(3)?,
                thinking_enabled: row.get::<_, i64>(4)? != 0,
                thinking_budget:  row.get::<_, Option<i64>>(5)?.map(|v| v as u32),
            }),
        ).optional()
         .map_err(|e| ArcanaError::Agent(format!("model_configs get: {e}")))
    }

    pub fn upsert(&self, cfg: &ModelConfig) -> Result<()> {
        self.conn.execute(
            "INSERT INTO model_configs (model_id, temperature, top_p, reasoning_effort, thinking_enabled, thinking_budget)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(model_id) DO UPDATE SET
               temperature      = excluded.temperature,
               top_p            = excluded.top_p,
               reasoning_effort = excluded.reasoning_effort,
               thinking_enabled = excluded.thinking_enabled,
               thinking_budget  = excluded.thinking_budget",
            params![
                cfg.model_id,
                cfg.temperature,
                cfg.top_p,
                cfg.reasoning_effort,
                cfg.thinking_enabled as i64,
                cfg.thinking_budget.map(|v| v as i64),
            ],
        ).map_err(|e| ArcanaError::Agent(format!("model_configs upsert: {e}")))?;
        Ok(())
    }

    pub fn delete(&self, model_id: &str) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM model_configs WHERE model_id = ?1",
                params![model_id],
            )
            .map_err(|e| ArcanaError::Agent(format!("model_configs delete: {e}")))?;
        Ok(())
    }
}
