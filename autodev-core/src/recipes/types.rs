// Recipe types — YAML-defined workflow definitions for the Worker Fleet.
// Recipes are used by Workers, Supervisor, and QC — NOT by the PA.
// PA uses AUTODEV.md for its own instructions.
// See: DEVELOPMENT_PLAN.md Phase 2.1 — Recipes System

use serde::{Deserialize, Serialize};

/// Permission mode for recipe execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RecipePermissionMode {
    /// Agent executes all actions without prompting
    Autonomous,
    /// Agent prompts before destructive operations
    Supervised,
}

impl Default for RecipePermissionMode {
    fn default() -> Self {
        Self::Autonomous
    }
}

impl From<&RecipePermissionMode> for crate::agent::permission::PermissionMode {
    /// Translate a recipe's declared policy into the engine's permission mode.
    ///
    /// 🔴 `P3-65`. Until this existed, `permission_mode` was **written by nine
    /// recipes and read by nothing**: `runner.rs` hardcoded
    /// `PermissionMode::Autonomous`, so four recipes declaring `Supervised`
    /// ran with every prompt suppressed. A reader of `permission_mode:
    /// Supervised` on the QC agent would reasonably conclude it was
    /// supervised. It was not.
    ///
    /// 🔴 `P3-66`. `SmartApprove` had **zero construction sites** anywhere —
    /// the interactive path hardcodes `ManualApproval` — so its two live
    /// `match` arms were unreachable: a guard that could not fire. This is
    /// where it is built.
    ///
    /// The pairing is not arbitrary. `Supervised` is documented as *"prompts
    /// before destructive operations"*, which is exactly `SmartApprove`'s
    /// rule: prompt on `Execute` / `Network` / `Board`, pass `Read` and
    /// `Write`. `ManualApproval` would prompt on a plain file write too, which
    /// is a different and stricter policy than the one declared.
    fn from(mode: &RecipePermissionMode) -> Self {
        match mode {
            RecipePermissionMode::Autonomous => Self::Autonomous,
            RecipePermissionMode::Supervised => Self::SmartApprove,
        }
    }
}

/// Doc 05 Phase 3 — fallback policy declaration.
///
/// Controls what the recipe runner does when Mode 2 (core-orchestrator
/// LivingAgent) is unavailable:
///   • `Mode2Only` (default) — refuse to run the tick. Logs an explicit
///     "fallback skipped per policy" line so the log stream makes the
///     decision auditable. Appropriate for recipes that depend on MCP
///     tools (most worker recipes).
///   • `Mode1Capable` — fall through to the Rust `AgentEngine` provider.
///     Only suitable for recipes whose work fits the Mode 1 capabilities
///     (no MCP tools, file ops only). The on-disk recipe declares this
///     when it has been verified to behave under Mode 1.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FallbackPolicy {
    #[default]
    Mode2Only,
    Mode1Capable,
}

/// A single Recipe — stored as YAML in ~/.autodev/recipes/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub version: String, // "1.0.0"
    pub id: String,      // slug used as file name
    pub title: String,
    pub description: Option<String>,
    pub schedule: Option<String>, // cron expression (*/120 * * * * = every 2 min)
    pub permission_mode: RecipePermissionMode,
    pub instructions: String,       // system prompt injected into agent
    pub agent_role: Option<String>, // "supervisor" | "worker-backend" | "qc"
    pub model: Option<String>,      // model override (default from settings)
    pub is_system: bool,            // system recipes cannot be deleted by users
    // Doc 05 Phase 3 — per-recipe fallback policy. `serde(default)` so
    // existing on-disk YAML without the field deserializes as Mode2Only.
    #[serde(default)]
    pub fallback_policy: FallbackPolicy,
}

/// Execution state of a recipe run
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RecipeRunStatus {
    Idle,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// A record of a recipe execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeRun {
    pub id: String, // UUID
    pub recipe_id: String,
    pub status: RecipeRunStatus,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub error: Option<String>,
}

#[cfg(test)]
mod p3_65_66_tests {
    use super::*;
    use crate::agent::permission::PermissionMode;

    #[test]
    fn supervised_actually_supervises() {
        // 🔴 P3-65. `permission_mode` was written by nine recipes and read by
        // nothing — `runner.rs` hardcoded Autonomous — so a recipe declaring
        // `Supervised` ran with every prompt suppressed.
        let mode = PermissionMode::from(&RecipePermissionMode::Supervised);
        assert_eq!(mode, PermissionMode::SmartApprove);

        // And that mode must genuinely prompt, or the mapping is decorative.
        use crate::agent::tool_effect::ToolEffect;
        assert!(
            ToolEffect::Execute.requires_prompt(&mode),
            "Supervised must prompt before running a command"
        );
        assert!(
            ToolEffect::Network.requires_prompt(&mode),
            "Supervised must prompt before sending data off the machine"
        );
    }

    #[test]
    fn autonomous_still_prompts_for_nothing() {
        // The no-fire half: the fleet depends on this staying silent.
        let mode = PermissionMode::from(&RecipePermissionMode::Autonomous);
        assert_eq!(mode, PermissionMode::Autonomous);

        use crate::agent::tool_effect::ToolEffect;
        for effect in [
            ToolEffect::Read,
            ToolEffect::Write,
            ToolEffect::Execute,
            ToolEffect::Network,
            ToolEffect::Board,
        ] {
            assert!(
                !effect.requires_prompt(&mode),
                "{effect:?} must not prompt under Autonomous — unattended cron \
                 work depends on it"
            );
        }
    }

    #[test]
    fn smart_approve_now_has_a_construction_site() {
        // 🔴 P3-66. `SmartApprove` had ZERO construction sites anywhere: the
        // interactive path hardcodes ManualApproval and the recipe path
        // hardcoded Autonomous. Its two live `match` arms were unreachable —
        // a guard that could not fire. This mapping is where it is built.
        let built = PermissionMode::from(&RecipePermissionMode::Supervised);
        assert!(matches!(built, PermissionMode::SmartApprove));
    }
}
