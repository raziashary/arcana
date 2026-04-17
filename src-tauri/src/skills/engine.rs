// SkillEngine — parameter validation + {{param}} template substitution.
// Phase 3.1

use std::collections::HashMap;

use super::types::{
    Skill, SkillExecutionResult, SkillParamType, ValidationError, ValidationResult,
};

pub struct SkillEngine;

impl SkillEngine {
    /// Validate that `params` satisfies the skill's parameter schema.
    pub fn validate(
        skill: &Skill,
        params: &serde_json::Map<String, serde_json::Value>,
    ) -> ValidationResult {
        let mut errors = Vec::new();

        for p in &skill.parameters {
            match params.get(&p.name) {
                None => {
                    if p.required && p.default.is_none() {
                        errors.push(ValidationError {
                            field:   p.name.clone(),
                            message: format!("'{}' is required", p.name),
                        });
                    }
                }
                Some(v) => {
                    // Type check
                    let type_ok = match p.param_type {
                        SkillParamType::String  => v.is_string(),
                        SkillParamType::Boolean => v.is_boolean(),
                        SkillParamType::Integer => v.is_i64() || v.is_u64(),
                        SkillParamType::Enum    => {
                            if let (Some(s), Some(vals)) = (v.as_str(), &p.values) {
                                vals.iter().any(|allowed| allowed == s)
                            } else {
                                false
                            }
                        }
                    };
                    if !type_ok {
                        errors.push(ValidationError {
                            field:   p.name.clone(),
                            message: format!("'{}' has wrong type", p.name),
                        });
                    }
                }
            }
        }

        if errors.is_empty() { ValidationResult::ok() } else { ValidationResult::fail(errors) }
    }

    /// Render the skill template by substituting {{param_name}} with actual values.
    /// Defaults are applied for missing optional parameters.
    pub fn render(
        skill: &Skill,
        params: &serde_json::Map<String, serde_json::Value>,
    ) -> SkillExecutionResult {
        // Merge defaults into a working map
        let mut resolved: HashMap<String, String> = HashMap::new();

        for p in &skill.parameters {
            let value = params
                .get(&p.name)
                .or(p.default.as_ref())
                .map(|v| value_to_string(v))
                .unwrap_or_default();
            resolved.insert(p.name.clone(), value);
        }

        // Also expose any extra params passed in (not in schema) verbatim
        for (k, v) in params {
            resolved.entry(k.clone()).or_insert_with(|| value_to_string(v));
        }

        // Perform substitution
        let mut rendered = skill.template.clone();
        for (key, val) in &resolved {
            rendered = rendered.replace(&format!("{{{{{key}}}}}"), val);
        }

        SkillExecutionResult {
            skill_name:  skill.name.clone(),
            rendered,
            executed_at: chrono::Utc::now(),
            parameters:  params.clone(),
        }
    }
}

fn value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s)  => s.clone(),
        serde_json::Value::Bool(b)    => b.to_string(),
        serde_json::Value::Number(n)  => n.to_string(),
        serde_json::Value::Null       => String::new(),
        other                         => other.to_string(),
    }
}
