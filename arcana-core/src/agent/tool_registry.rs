// ToolRegistry — registers and retrieves tool implementations.
// All built-in tools are registered at startup in AgentEngine::new().

use super::tool_trait::Tool;
use crate::provider::types::ToolDefinition;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn all(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.values().cloned().collect()
    }

    /// Build ToolDefinition list for the LLM provider request.
    /// Keep only the named tools.
    ///
    /// 🔴 `P3-60`. `SubagentConfig.tools` is documented as *"tool subset the
    /// subagent is allowed to use"*, is written by all three spawn tools **and**
    /// the Tauri command — and had **zero readers**. The child received the
    /// full registry. A containment promise the code did not keep.
    ///
    /// An empty list means *no restriction*, matching how every caller writes
    /// it: the field is left empty when the parent has no opinion.
    pub fn restrict_to(&mut self, names: &[String]) {
        if names.is_empty() {
            return;
        }
        self.tools.retain(|name, _| names.iter().any(|n| n == name));
    }

    pub fn to_definitions(&self) -> Vec<ToolDefinition> {
        let mut defs: Vec<ToolDefinition> = self
            .tools
            .values()
            .map(|t| ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
            })
            .collect();
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        defs
    }
}

#[cfg(test)]
mod p3_60_tests {
    use super::*;
    use crate::agent::PAMode;

    fn full() -> ToolRegistry {
        let mut r = ToolRegistry::new();
        crate::agent::builtin_tools::register_for_mode(&mut r, PAMode::FastDevelopment);
        r
    }

    #[test]
    fn a_declared_subset_actually_narrows_the_registry() {
        // 🔴 P3-60. `SubagentConfig.tools` is documented as "tool subset the
        // subagent is allowed to use", written by all three spawn tools AND
        // the Tauri command — with zero readers. The child got everything.
        let mut r = full();
        let before = r.all().len();
        assert!(before > 2, "the full registry should be substantial");

        r.restrict_to(&["read_file".to_string(), "grep".to_string()]);

        let names: Vec<String> = r.all().iter().map(|t| t.name().to_string()).collect();
        assert_eq!(names.len(), 2, "got {names:?}");
        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"grep".to_string()));
        assert!(
            !names.contains(&"bash".to_string()),
            "a subagent restricted to reads was still handed `bash`"
        );
    }

    #[test]
    fn an_empty_subset_means_unrestricted_not_disarmed() {
        // Every caller leaves the field empty when it has no opinion. If empty
        // meant "no tools", wiring the field up would silently disarm every
        // existing subagent — a fix worse than the defect.
        let mut r = full();
        let before = r.all().len();
        r.restrict_to(&[]);
        assert_eq!(r.all().len(), before);
    }

    #[test]
    fn a_subset_can_only_remove_never_add() {
        // The containment property. Naming a tool the mode never granted must
        // not conjure it — otherwise the "subset" is an escalation vector.
        let mut r = ToolRegistry::new();
        crate::agent::builtin_tools::register_for_mode(&mut r, PAMode::Delegate);
        let granted: Vec<String> = r.all().iter().map(|t| t.name().to_string()).collect();

        r.restrict_to(&["write_file".to_string(), "bash".to_string()]);

        for t in r.all() {
            assert!(
                granted.contains(&t.name().to_string()),
                "restrict_to introduced '{}', which the mode never granted",
                t.name()
            );
        }
    }
}

#[cfg(test)]
mod p8_30_tests {
    //! `P8-30` — `spawn_subagent` misdescribed its own tool grant.
    //!
    //! Its schema told the model *"Empty = MCP tools only"* — the most
    //! restrictive reading available — while `restrict_to` returns early on an
    //! empty list and the child keeps **everything**. A model reading its own
    //! documentation granted more than it intended, and MCP had already been
    //! deleted in `P0`, so the promised restriction named a subsystem that no
    //! longer existed.
    //!
    //! Ruled (`P8-30` leaves the choice open): **empty means INHERIT**. It is
    //! the shipped behaviour, three spawn tools and a Tauri command write this
    //! field, and flipping it to "none" would silently disarm every caller
    //! that omits it. The description now says so.

    use super::*;

    #[test]
    fn an_empty_subset_is_not_a_restriction() {
        // 🔴 The behaviour the description denied. Asserted directly so that
        // anyone who later "fixes" `restrict_to` to mean "none" has to come
        // through this test and update the description in the same step.
        let mut reg = ToolRegistry::new();
        crate::agent::builtin_tools::register_for_mode(&mut reg, crate::agent::PAMode::FastDevelopment);
        let before = reg.to_definitions().len();
        assert!(before > 1, "fixture needs a populated registry, got {before}");
        reg.restrict_to(&[]);
        assert_eq!(
            reg.to_definitions().len(),
            before,
            "an empty subset must inherit, not restrict"
        );
    }

    #[test]
    fn a_named_subset_removes_everything_else() {
        let mut reg = ToolRegistry::new();
        crate::agent::builtin_tools::register_for_mode(&mut reg, crate::agent::PAMode::FastDevelopment);
        let all = reg.to_definitions();
        if all.len() < 2 {
            // A registry this small cannot demonstrate narrowing; say so
            // rather than passing vacuously (`L-C9`).
            panic!("the default registry has {} tools; the fixture needs 2+", all.len());
        }
        let keep = all[0].name.clone();
        reg.restrict_to(&[keep.clone()]);
        let names: Vec<String> = reg.to_definitions().into_iter().map(|d| d.name).collect();
        assert_eq!(names, vec![keep]);
    }
}
