// What a tool does to the world (P3-11, spec `06 §1.2`, G-11).
//
// ── One flag, one meaning, one gate ──────────────────────────────────────────
//
// Prompting was keyed on `is_read_only()` — a flag whose own documentation
// describes it as a **concurrency hint** for the path lock. So the rule actually
// in force was *"any tool declaring itself read-only is never prompted, in any
// permission mode"*, and two tools declared it while acting on the world:
//
//   * **`analyze_diff`** writes to the database *and* **uploads the full diff of
//     any repository to a third-party LLM**. Under BYOK that is the user's own
//     code leaving the machine on the user's own key, with no prompt.
//   * **`check_file`** executes external checkers — including one it resolves
//     from **inside the workspace under inspection** (`P3-19`).
//
// And `Tool::requires_permission()`, which reads as the security boundary, has
// never been called by anything.
//
// ── Why an enum rather than more booleans ────────────────────────────────────
//
// A boolean answers one question and then gets reused for a second. That is
// exactly how `is_read_only` became a security control: it was written for
// *"may this run concurrently?"* and ended up answering *"may this run
// unprompted?"*. Those two questions have different answers for `check_file` —
// it is safe to run concurrently and unsafe to run unprompted — and a boolean
// cannot hold both.
//
// `ToolEffect` names the thing that actually matters: what escapes. Concurrency
// goes back to `is_read_only`, where it started.

use serde::{Deserialize, Serialize};

/// What a tool does that the user might want to be asked about.
///
/// Ordered by escalating consequence, which is also the order a reviewer should
/// read them in: observing is not mutating, mutating locally is not running
/// code, and none of those is sending the user's data off the machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolEffect {
    /// Observes. No side effect anywhere — not the filesystem, not the board,
    /// not the network.
    ///
    /// ⚠ Reading a file **outside the workspace** is an exfiltration primitive,
    /// not a harmless read. A tool is only `Read` if its reads are jailed
    /// (`P3-14`).
    Read,

    /// Mutates the workspace.
    Write,

    /// Runs a subprocess.
    ///
    /// Separate from `Write` because the blast radius is not the workspace: a
    /// subprocess inherits the environment and can do anything the user can.
    Execute,

    /// Sends data off the machine.
    ///
    /// ⚠ **Its own effect, never folded into `Read`.** A tool that uploads the
    /// user's source to a third party is not "read-only" under any reading, and
    /// BYOK makes it the user's own data and the user's own bill. This is the
    /// prompting half of `TG3-26` (`P3-56`); the consent *store* is P4's.
    Network,

    /// Mutates task or fleet state — the board.
    ///
    /// Distinct from `Write` because it is shared state that other agents act
    /// on, so the consequence of a wrong one is not confined to a file.
    Board,
}

impl ToolEffect {
    /// Whether this effect requires the user to be asked, in the given mode.
    ///
    /// ⚠ **This is the only prompting decision in the codebase.** It is a
    /// method on the effect rather than scattered `match` arms so that adding a
    /// mode or an effect forces exactly one place to be updated, and so a
    /// reviewer can read the whole policy at once.
    pub fn requires_prompt(&self, mode: &crate::agent::permission::PermissionMode) -> bool {
        use crate::agent::permission::PermissionMode as M;

        match mode {
            // Nothing is prompted. The user has said so explicitly, and
            // unattended fleet work depends on it.
            M::Autonomous => false,

            // Everything with a consequence is prompted.
            M::ManualApproval => !matches!(self, ToolEffect::Read),

            // Observation and local mutation pass; anything that runs code,
            // leaves the machine, or changes shared state is asked about.
            //
            // ⚠ `Network` prompts here even though `Write` does not — sending
            // the user's code to a third party is a larger step than editing a
            // file in their own workspace, and it is the one they cannot undo.
            M::SmartApprove => matches!(
                self,
                ToolEffect::Execute | ToolEffect::Network | ToolEffect::Board
            ),
            // ⚠ Spec `06 §1.1` names a fourth mode, `ChatOnly`. It does not
            // exist in `PermissionMode` — the code has three variants — and
            // §0.0 makes the shipped code the anchor, so it is not invented
            // here. The capability it describes is already delivered one layer
            // up: `PAMode::Delegate` strips every file tool from the registry
            // (`builtin_tools/mod.rs`), so those tools are never offered rather
            // than offered-and-refused. Recorded as a spec/code divergence
            // rather than silently reconciled either way.
        }
    }

    /// A short phrase for the approval dialog, in the user's terms.
    ///
    /// Says what will happen, not what the tool is called: "run a command on
    /// your machine" is actionable, "Execute effect" is not.
    pub fn describe(&self) -> &'static str {
        match self {
            ToolEffect::Read => "read from your workspace",
            ToolEffect::Write => "modify files in your workspace",
            ToolEffect::Execute => "run a command on your machine",
            ToolEffect::Network => "send data from your machine to an external service",
            ToolEffect::Board => "change shared task and fleet state",
        }
    }

    /// Whether the tool may run concurrently with others.
    ///
    /// ⚠ This is what `is_read_only` was written to answer, and the only
    /// question it should ever be asked again. Kept here beside the effect so
    /// the two stay visibly separate: `check_file` is concurrency-safe **and**
    /// requires a prompt, which is precisely the combination one boolean could
    /// not express.
    pub fn is_concurrency_safe(&self) -> bool {
        matches!(self, ToolEffect::Read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::permission::PermissionMode as M;

    #[test]
    fn network_is_never_silently_allowed_outside_autonomous() {
        // The defect this enum exists for: `analyze_diff` uploaded the full
        // diff of any repository to a third-party LLM while declaring itself
        // read-only, so it was never prompted in ANY mode.
        for mode in [M::ManualApproval, M::SmartApprove] {
            assert!(
                ToolEffect::Network.requires_prompt(&mode),
                "Network must prompt in {mode:?} — it is the user's data leaving their machine"
            );
        }
        // Autonomous is the user's explicit standing consent, and unattended
        // fleet work depends on it.
        assert!(!ToolEffect::Network.requires_prompt(&M::Autonomous));
    }

    #[test]
    fn executing_a_subprocess_always_prompts_where_anything_does() {
        for mode in [M::ManualApproval, M::SmartApprove] {
            assert!(ToolEffect::Execute.requires_prompt(&mode));
        }
    }

    #[test]
    fn reading_never_prompts() {
        for mode in [M::Autonomous, M::ManualApproval, M::SmartApprove] {
            assert!(
                !ToolEffect::Read.requires_prompt(&mode),
                "a jailed read is the one effect with no consequence to approve"
            );
        }
    }

    #[test]
    fn each_mode_behaves_differently_from_the_others() {
        // TG3-25: a mode that behaves identically to another is not a mode.
        // Compared as the full vector of decisions across every effect.
        let effects = [
            ToolEffect::Read,
            ToolEffect::Write,
            ToolEffect::Execute,
            ToolEffect::Network,
            ToolEffect::Board,
        ];
        let profile = |m: &M| {
            effects
                .iter()
                .map(|e| e.requires_prompt(m))
                .collect::<Vec<_>>()
        };

        let autonomous = profile(&M::Autonomous);
        let manual = profile(&M::ManualApproval);
        let smart = profile(&M::SmartApprove);

        assert_ne!(autonomous, manual);
        assert_ne!(manual, smart);
        assert_ne!(autonomous, smart);
    }

    #[test]
    fn concurrency_and_prompting_are_separable() {
        // The whole reason this is an enum and not a boolean. `check_file` is
        // Execute: safe to run concurrently with other reads in principle, and
        // absolutely not safe to run unprompted. One flag could not say both,
        // which is how a concurrency hint became a security control.
        assert!(ToolEffect::Read.is_concurrency_safe());
        assert!(!ToolEffect::Execute.is_concurrency_safe());
        assert!(ToolEffect::Execute.requires_prompt(&M::SmartApprove));
    }

    #[test]
    fn every_effect_describes_itself_in_the_users_terms() {
        for e in [
            ToolEffect::Read,
            ToolEffect::Write,
            ToolEffect::Execute,
            ToolEffect::Network,
            ToolEffect::Board,
        ] {
            let d = e.describe();
            assert!(!d.is_empty());
            // A dialog that names the variant tells the user nothing.
            assert!(
                !d.contains("Effect"),
                "{d:?} names the type, not the consequence"
            );
        }
    }
}
