// S3 · Permission modes are honoured — the smoke for `P3-12`, `P3-64`, `P3-65`.
//
// `P3-12` as originally written said the UI's mode dropdown was ignored. That
// attribution was REFUTED during the P3 refutation pass: the dropdown sets
// `PAMode`, not `PermissionMode`, and `PAMode` **is** honoured (it drives the
// system prompt and the tool registry). Implementing the task literally would
// have fused tool availability, prompt and permission policy onto one
// three-valued control — a product change nobody asked for.
//
// What was genuinely broken was found elsewhere, and this file is the smoke
// over all three:
//
//   * `P3-64` — the concurrent read partition performed NO permission check at
//     all. `is_read_only() -> true` was an unconditional bypass of every mode.
//   * `P3-65` — `Recipe::permission_mode` was written by nine recipes and read
//     by nothing; the runner hardcoded Autonomous.
//   * `P3-66` — `SmartApprove` had zero construction sites.
//
// The invariant these share: **a permission mode must produce a different
// decision than the other modes, for the effects it is defined to differ on.**
// A mode that decides nothing is not a mode.

use autodev_core::agent::permission::PermissionMode;
use autodev_core::agent::tool_effect::ToolEffect;
use autodev_core::recipes::types::RecipePermissionMode;

const EVERY_EFFECT: [ToolEffect; 5] = [
    ToolEffect::Read,
    ToolEffect::Write,
    ToolEffect::Execute,
    ToolEffect::Network,
    ToolEffect::Board,
];

#[test]
fn s3_01_every_mode_is_reachable_from_something_a_user_can_set() {
    // 🔴 P3-66. A mode nobody can select is a guard that cannot fire. Before
    // this, `SmartApprove` was exactly that — the interactive path hardcodes
    // ManualApproval and the recipe path hardcoded Autonomous.
    let from_recipe_autonomous = PermissionMode::from(&RecipePermissionMode::Autonomous);
    let from_recipe_supervised = PermissionMode::from(&RecipePermissionMode::Supervised);

    assert_eq!(from_recipe_autonomous, PermissionMode::Autonomous);
    assert_eq!(
        from_recipe_supervised,
        PermissionMode::SmartApprove,
        "the recipe editor's 'supervised' must build a mode that actually prompts"
    );

    // ManualApproval is reachable from the interactive assistant path
    // (agent_commands.rs), which is deliberate and documented there.
    let _ = PermissionMode::ManualApproval;
}

#[test]
fn s3_02_the_three_modes_decide_differently() {
    // If two modes agree on every effect, one of them is decoration. This is
    // the check that would have failed while `SmartApprove` was unbuildable
    // *and* would fail again if someone collapsed the arms.
    let profile = |m: &PermissionMode| -> Vec<bool> {
        EVERY_EFFECT.iter().map(|e| e.requires_prompt(m)).collect()
    };

    let autonomous = profile(&PermissionMode::Autonomous);
    let manual = profile(&PermissionMode::ManualApproval);
    let smart = profile(&PermissionMode::SmartApprove);

    assert_ne!(autonomous, manual, "Autonomous and ManualApproval must differ");
    assert_ne!(autonomous, smart, "Autonomous and SmartApprove must differ");
    assert_ne!(
        manual, smart,
        "ManualApproval and SmartApprove must differ — otherwise one is \
         decoration and the recipe control means nothing"
    );
}

#[test]
fn s3_03_autonomous_prompts_for_nothing_and_the_others_prompt_for_something() {
    // The fleet depends on the first half; the user depends on the second.
    for effect in EVERY_EFFECT {
        assert!(
            !effect.requires_prompt(&PermissionMode::Autonomous),
            "{effect:?} prompted under Autonomous — unattended cron work would stall"
        );
    }

    for mode in [PermissionMode::ManualApproval, PermissionMode::SmartApprove] {
        assert!(
            EVERY_EFFECT.iter().any(|e| e.requires_prompt(&mode)),
            "{mode:?} prompts for nothing at all, which makes it Autonomous \
             under another name"
        );
    }
}

#[test]
fn s3_04_reading_is_never_gated_and_running_always_is() {
    // 🔴 P3-64's inverse. The bypass was keyed on `is_read_only()`, which
    // `check_file` declared true while spawning external checkers. Effects,
    // not a boolean, are what decides now — so the two ends of the scale must
    // hold in every mode that prompts at all.
    for mode in [PermissionMode::ManualApproval, PermissionMode::SmartApprove] {
        assert!(
            !ToolEffect::Read.requires_prompt(&mode),
            "a pure read must not prompt in {mode:?} — every turn would stall"
        );
        assert!(
            ToolEffect::Execute.requires_prompt(&mode),
            "running a command must prompt in {mode:?}"
        );
        assert!(
            ToolEffect::Network.requires_prompt(&mode),
            "sending the user's code off the machine must prompt in {mode:?} — \
             this is the one they cannot undo"
        );
    }
}
