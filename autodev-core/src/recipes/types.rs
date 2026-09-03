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
    // P8-25/P8-47 — `fallback_policy` DELETED.
    //
    // 🔴 It defaulted to `Mode2Only`, and `runner.rs` refused to run any
    // recipe carrying that policy. `orch_healthy` has been pinned `false`
    // since P0, so the guard was UNCONDITIONAL: `Mode2Only` did not mean
    // "needs the second engine", it meant **never runs**. And it was the serde
    // default, so every recipe the UI saved -- `RecipeEditor.tsx`'s
    // `buildRecipe()` omits the key -- deserialised as stranded and then
    // sat reporting `fallback_idle` forever. The fleet reported itself
    // idle rather than broken, which is the hardest kind of dead to see.
    //
    // ⚠ Deleting the field is what makes this a real fix. Flipping the
    // default would have left every `fallback_policy: mode2_only` already
    // written to `~/.autodev/recipes/*.yaml` still stranded, because
    // `#[serde(default)]` only fills a key that is MISSING. Nothing here
    // sets `deny_unknown_fields`, so that stale key is now ignored and old
    // YAML keeps parsing -- see `p8_25_tests`.
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

#[cfg(test)]
mod p8_25_tests {
    //! `P8-25` — every recipe saved in the UI was silently stranded.
    //!
    //! `FallbackPolicy` defaulted to `Mode2Only`; `RecipeEditor.tsx`'s
    //! `buildRecipe()` omits the key, so `#[serde(default)]` filled in the
    //! stranding value; and `runner.rs` refused to run any recipe carrying it.
    //! `orch_healthy` has been pinned `false` since `P0`, so that guard was
    //! **unconditional** — the recipe never ran and never errored. It reported
    //! `fallback_idle`, so the fleet looked idle rather than broken.

    use super::*;

    /// 🔴 **The test that decides whether this is a fix at all.**
    ///
    /// Flipping the default to `Mode1Capable` would have fixed new recipes and
    /// left every `fallback_policy: mode2_only` ALREADY WRITTEN to
    /// `~/.autodev/recipes/*.yaml` still stranded, because `#[serde(default)]`
    /// only fills a key that is **missing** — an explicitly-written value
    /// survives any change to the Rust default. Deleting the field is what
    /// reaches those files, and this asserts the deletion is backwards
    /// compatible rather than a parse error on next launch.
    #[test]
    fn a_recipe_yaml_still_carrying_the_deleted_key_still_parses() {
        let yaml = r#"
version: "1.0.0"
id: worker-legacy
title: A recipe written before P8
description: null
schedule: null
permission_mode: supervised
instructions: "do the thing"
agent_role: null
model: null
is_system: false
fallback_policy: mode2_only
"#;
        let recipe: Recipe = serde_yaml::from_str(yaml)
            .expect("a recipe carrying the deleted key must still deserialise");
        assert_eq!(recipe.id, "worker-legacy");
        assert_eq!(recipe.title, "A recipe written before P8");
    }

    #[test]
    fn a_recipe_yaml_without_the_key_parses_too() {
        // The shape every recipe written after this phase has.
        let yaml = r#"
version: "1.0.0"
id: worker-new
title: A recipe written after P8
description: null
schedule: null
permission_mode: supervised
instructions: "do the thing"
agent_role: null
model: null
is_system: false
"#;
        let recipe: Recipe = serde_yaml::from_str(yaml).expect("must deserialise");
        assert_eq!(recipe.id, "worker-new");
    }

    #[test]
    fn a_serialised_recipe_no_longer_emits_the_key() {
        // Round-tripping an old recipe must not write the stranding key back.
        let yaml = r#"
version: "1.0.0"
id: r
title: t
description: null
schedule: null
permission_mode: supervised
instructions: "do the thing"
agent_role: null
model: null
is_system: false
fallback_policy: mode2_only
"#;
        let recipe: Recipe = serde_yaml::from_str(yaml).expect("parse");
        let out = serde_yaml::to_string(&recipe).expect("serialise");
        assert!(
            !out.contains("fallback_policy"),
            "the stranding key was written back out: {out}"
        );
    }
}
