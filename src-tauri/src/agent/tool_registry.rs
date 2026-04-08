// ToolRegistry — registers and retrieves tool implementations.
// All built-in tools are registered at startup in AgentEngine::new().

use std::collections::HashMap;
use std::sync::Arc;
use crate::provider::types::ToolDefinition;
use super::tool_trait::Tool;

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
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
    pub fn to_definitions(&self) -> Vec<ToolDefinition> {
        let mut defs: Vec<ToolDefinition> = self.tools.values().map(|t| ToolDefinition {
            name:         t.name().to_string(),
            description:  t.description().to_string(),
            input_schema: t.input_schema(),
        }).collect();
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        defs
    }
}
