// planner.rs — Unified planning interface with YAML DSL compiler.
// Exposed via autodev-mcp-server so PA, Supervisor, Workers, and QC
// all share one tool for plan operations.
//
// Design: Single tool, single yaml_dsl parameter, action-based routing.
// See DEVELOPMENT_PLAN.md Phase 3.5.

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::plans::{
    AgentType, CreatePlanInput, CreatePlanItemInput, PlanItemStatus, PlanScope, PlansDb,
};
use crate::events::{emit_plan_changed_global, PlanChangedPayload};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct PlannerTool;

#[async_trait]
impl Tool for PlannerTool {
    fn name(&self) -> &str {
        "planner"
    }

    fn description(&self) -> &str {
        "YAML DSL planning interface. Pass 'action: listActions' to see all actions and schemas. \
         Core actions: create, addStep, updateStep, getPlan, listPlans, claimItem, submitForQC. \
         Use for tasks requiring 5+ steps or 10+ tool calls."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "yaml_dsl": {
                    "type": "string",
                    "description": "YAML DSL command. Use 'action: listActions' to discover available actions."
                }
            },
            "required": ["yaml_dsl"]
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let yaml_dsl = match input["yaml_dsl"].as_str() {
            Some(s) if !s.trim().is_empty() => s,
            _ => return ToolResult::error("Missing or empty 'yaml_dsl' parameter. Provide a YAML DSL block with an 'action' key."),
        };

        let compiler = match PlannerCompiler::new() {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to initialize planner: {e}")),
        };

        match compiler.compile(yaml_dsl) {
            Ok(result) => {
                ToolResult::success(serde_json::to_string_pretty(&result).unwrap_or_default())
            }
            Err(e) => ToolResult::error(format!("Planner error: {e}")),
        }
    }
}

// ─── PlannerCompiler — YAML DSL Action Router ────────────────────────────────

struct PlannerCompiler {
    db: PlansDb,
}

impl PlannerCompiler {
    fn new() -> anyhow::Result<Self> {
        let db = PlansDb::open()?;
        Ok(Self { db })
    }

    fn compile(&self, yaml_dsl: &str) -> anyhow::Result<Value> {
        let doc: serde_yaml::Value =
            serde_yaml::from_str(yaml_dsl).map_err(|e| anyhow::anyhow!("Invalid YAML DSL: {e}"))?;

        let action = doc
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'action' field in YAML DSL"))?;

        match action {
            "listActions" => self.list_actions(),
            "create" => self.create_plan(&doc),
            "addStep" => self.add_step(&doc),
            "updateStep" => self.update_step(&doc),
            "getPlan" => self.get_plan(&doc),
            "listPlans" => self.list_plans(&doc),
            "checkDependencies" => self.check_dependencies(&doc),
            "deletePlan" => self.delete_plan(&doc),
            "abandonPlan" => self.abandon_plan(&doc),
            "breakDownEpic" => self.breakdown_epic(&doc),
            "claimItem" => self.claim_item(&doc),
            "createSubplan" => self.create_subplan(&doc),
            "submitForQC" => self.submit_for_qc(&doc),
            _ => Err(anyhow::anyhow!(
                "Unknown action: '{action}'. Use 'action: listActions' to discover valid actions."
            )),
        }
    }

    // ─── Action Handlers ──────────────────────────────────────────────────

    fn list_actions(&self) -> anyhow::Result<Value> {
        Ok(json!({
            "actions": [
                { "name": "listActions",       "description": "Discover all actions", "read_only": true },
                { "name": "create",            "description": "Create a new plan", "required": ["title"] },
                { "name": "addStep",           "description": "Add a step to a plan", "required": ["plan_id", "title"] },
                { "name": "updateStep",        "description": "Update step status — pass step_index (0-based position) OR step_id (UUID from addStep)", "required": ["plan_id", "status"], "optional": ["step_index", "step_id"] },
                { "name": "getPlan",           "description": "Get plan details", "required": ["plan_id"] },
                { "name": "listPlans",         "description": "List plans", "required": [] },
                { "name": "checkDependencies", "description": "Check which steps are unblocked", "required": ["plan_id"] },
                { "name": "deletePlan",        "description": "Delete a plan", "required": ["plan_id"] },
                { "name": "abandonPlan",       "description": "Mark plan as abandoned", "required": ["plan_id"] },
                { "name": "breakDownEpic",     "description": "Convert epic to plan (Supervisor)", "required": ["epic_task_id"] },
                { "name": "claimItem",         "description": "Claim next available step (Worker)", "required": ["plan_id", "worker_id"] },
                { "name": "createSubplan",     "description": "Create nested plan (Worker)", "required": ["parent_plan_id", "title"] },
                { "name": "submitForQC",       "description": "Submit plan for QC review", "required": ["plan_id", "worker_id"] },
            ],
            "note": "All actions accept YAML DSL with 'action' as the first key."
        }))
    }

    fn create_plan(&self, doc: &serde_yaml::Value) -> anyhow::Result<Value> {
        let title = self.get_str(doc, "title")?;
        let description = self.get_optional_str(doc, "description");
        let session_id = self.get_optional_str(doc, "session_id");
        let assigned_agent = self.get_optional_str(doc, "assigned_agent");
        let created_by = self
            .get_optional_str(doc, "created_by")
            .or_else(|| Some("pa".to_string()));
        let scope = self
            .get_optional_str(doc, "scope")
            .unwrap_or_else(|| "personal".to_string());
        let agent_scope = PlanScope::from_str(&scope);
        let parent_plan_id = self.get_optional_str(doc, "parent_plan_id");
        let task_id = self.get_optional_i64(doc, "task_id");

        let input = CreatePlanInput {
            title,
            description,
            session_id,
            assigned_agent,
            created_by,
            agent_scope,
            parent_plan_id,
            task_id,
        };

        let plan = self.db.create_plan(input)?;
        emit_plan_changed_global(PlanChangedPayload {
            plan_id: Some(plan.id.clone()),
            kind: "created".to_string(),
            status: Some(plan.status.as_str().to_string()),
        });
        Ok(serde_json::to_value(&plan)?)
    }

    fn add_step(&self, doc: &serde_yaml::Value) -> anyhow::Result<Value> {
        let plan_id = self.get_str(doc, "plan_id")?;
        let title = self.get_str(doc, "title")?;
        let description = self.get_optional_str(doc, "description");
        let order_index = self.get_i64(doc, "order_index").unwrap_or(0);
        let agent_type = self
            .get_optional_str(doc, "agent_type")
            .map(|s| AgentType::from_str(&s));
        let estimated_tools = self.get_optional_str_array(doc, "estimated_tools");
        let assigned_worker = self.get_optional_str(doc, "assigned_worker");
        let worktree_branch = self.get_optional_str(doc, "worktree_branch");
        let parent_id = self.get_optional_str(doc, "parent_id");

        let input = CreatePlanItemInput {
            plan_id,
            parent_id,
            title,
            description,
            order_index,
            agent_type,
            estimated_tools,
            assigned_worker,
            worktree_branch,
        };

        let item = self.db.add_plan_item(input)?;
        emit_plan_changed_global(PlanChangedPayload {
            plan_id: Some(item.plan_id.clone()),
            kind: "step_added".to_string(),
            status: None,
        });
        Ok(serde_json::to_value(&item)?)
    }

    fn update_step(&self, doc: &serde_yaml::Value) -> anyhow::Result<Value> {
        let plan_id = self.get_str(doc, "plan_id")?;
        let items = self.db.list_plan_items(&plan_id)?;

        // Accept either step_id (preferred — stable UUID) or step_index (0-based position).
        // step_id is returned by addStep and is immune to reordering.
        // step_index is kept for backwards compatibility but is fragile.
        let item = if let Some(step_id) = self.get_optional_str(doc, "step_id") {
            items
                .iter()
                .find(|i| i.id == step_id)
                .ok_or_else(|| anyhow::anyhow!("Step id '{step_id}' not found in plan {plan_id}"))?
        } else {
            let step_index: usize = self.get_usize(doc, "step_index")
                .ok_or_else(|| anyhow::anyhow!(
                    "Either 'step_id' (UUID from addStep) or 'step_index' (0-based position) is required"
                ))?;
            items.get(step_index).ok_or_else(|| {
                anyhow::anyhow!(
                    "Step index {step_index} out of range (plan has {} steps)",
                    items.len()
                )
            })?
        };

        let status_str = self.get_str(doc, "status")?;
        let notes = self.get_optional_str(doc, "notes");

        self.db
            .update_plan_item_status(&item.id, &status_str, notes.as_deref())?;
        emit_plan_changed_global(PlanChangedPayload {
            plan_id: Some(plan_id.clone()),
            kind: "step_updated".to_string(),
            status: Some(status_str.clone()),
        });

        // If all items done, auto-mark plan completed
        let refreshed = self.db.list_plan_items(&plan_id)?;
        let all_done =
            !refreshed.is_empty() && refreshed.iter().all(|i| i.status == PlanItemStatus::Done);
        if all_done {
            self.db.update_plan_status(&plan_id, "completed")?;
            emit_plan_changed_global(PlanChangedPayload {
                plan_id: Some(plan_id.clone()),
                kind: "plan_completed".to_string(),
                status: Some("completed".to_string()),
            });
        }

        Ok(json!({ "updated_item_id": item.id, "new_status": status_str.clone() }))
    }

    fn get_plan(&self, doc: &serde_yaml::Value) -> anyhow::Result<Value> {
        let plan_id = self.get_str(doc, "plan_id")?;
        let plan = self
            .db
            .get_plan(&plan_id)?
            .ok_or_else(|| anyhow::anyhow!("Plan '{plan_id}' not found"))?;
        let include_items = self
            .get_optional_bool(doc, "include_items")
            .unwrap_or(false);

        let mut result = serde_json::to_value(&plan)?;
        if include_items {
            let items = self.db.list_plan_items(&plan_id)?;
            if let Some(obj) = result.as_object_mut() {
                obj.insert("items".to_string(), serde_json::to_value(&items)?);
            }
        }
        Ok(result)
    }

    fn list_plans(&self, doc: &serde_yaml::Value) -> anyhow::Result<Value> {
        let status = self.get_optional_str(doc, "status_filter");
        let scope = self.get_optional_str(doc, "scope");
        let session = self.get_optional_str(doc, "session_id");
        let plans = self
            .db
            .list_plans(status.as_deref(), scope.as_deref(), session.as_deref())?;
        Ok(serde_json::to_value(&plans)?)
    }

    fn check_dependencies(&self, doc: &serde_yaml::Value) -> anyhow::Result<Value> {
        let plan_id = self.get_str(doc, "plan_id")?;
        let items = self.db.list_plan_items(&plan_id)?;
        // Check each pending item — see if its dependencies are satisfied
        let mut results = Vec::new();
        for item in items {
            let deps = self.db.check_dependencies(&item.id)?;
            let satisfied = deps.iter().all(|&d| d);
            results.push(json!({
                "item_id": item.id,
                "title": item.title,
                "all_dependencies_satisfied": satisfied,
                "dependency_count": deps.len(),
                "satisfied_count": deps.iter().filter(|&&d| d).count(),
            }));
        }
        Ok(json!({ "plan_id": plan_id, "items": results }))
    }

    fn delete_plan(&self, doc: &serde_yaml::Value) -> anyhow::Result<Value> {
        let plan_id = self.get_str(doc, "plan_id")?;
        self.db.delete_plan(&plan_id)?;
        emit_plan_changed_global(PlanChangedPayload {
            plan_id: Some(plan_id.clone()),
            kind: "deleted".to_string(),
            status: None,
        });
        Ok(json!({ "deleted": true, "plan_id": plan_id }))
    }

    fn abandon_plan(&self, doc: &serde_yaml::Value) -> anyhow::Result<Value> {
        let plan_id = self.get_str(doc, "plan_id")?;
        self.db.update_plan_status(&plan_id, "abandoned")?;
        emit_plan_changed_global(PlanChangedPayload {
            plan_id: Some(plan_id.clone()),
            kind: "abandoned".to_string(),
            status: Some("abandoned".to_string()),
        });
        Ok(json!({ "abandoned": true, "plan_id": plan_id }))
    }

    // ─── Fleet-Specific Actions ─────────────────────────────────────────────

    fn breakdown_epic(&self, doc: &serde_yaml::Value) -> anyhow::Result<Value> {
        let epic_task_id = self
            .get_i64(doc, "epic_task_id")
            .ok_or_else(|| anyhow::anyhow!("Missing 'epic_task_id'"))?;
        let supervisor_id = self
            .get_optional_str(doc, "supervisor_id")
            .unwrap_or_else(|| "supervisor".to_string());

        let input = CreatePlanInput {
            title: format!("Epic breakdown: task {}", epic_task_id),
            description: Some(format!(
                "Auto-generated plan from epic task {}",
                epic_task_id
            )),
            session_id: Some("fleet".to_string()),
            assigned_agent: Some(supervisor_id.clone()),
            created_by: Some(supervisor_id),
            agent_scope: PlanScope::Epic,
            parent_plan_id: None,
            task_id: Some(epic_task_id),
        };

        let plan = self.db.create_plan(input)?;
        emit_plan_changed_global(PlanChangedPayload {
            plan_id: Some(plan.id.clone()),
            kind: "epic_breakdown".to_string(),
            status: Some(plan.status.as_str().to_string()),
        });
        Ok(serde_json::to_value(&plan)?)
    }

    fn claim_item(&self, doc: &serde_yaml::Value) -> anyhow::Result<Value> {
        let plan_id = self.get_str(doc, "plan_id")?;
        let worker_id = self.get_str(doc, "worker_id")?;
        let item = self.db.claim_next_item(&plan_id, &worker_id)?;
        match item {
            Some(it) => {
                emit_plan_changed_global(PlanChangedPayload {
                    plan_id: Some(plan_id.clone()),
                    kind: "item_claimed".to_string(),
                    status: Some("in_progress".to_string()),
                });
                Ok(serde_json::to_value(&it)?)
            }
            None => Ok(json!({ "claimed": false, "reason": "No pending items available" })),
        }
    }

    fn create_subplan(&self, doc: &serde_yaml::Value) -> anyhow::Result<Value> {
        let parent_plan_id = self.get_str(doc, "parent_plan_id")?;
        let worker_id = self.get_str(doc, "worker_id")?;
        let title = self.get_str(doc, "title")?;
        let description = self
            .get_optional_str(doc, "description")
            .unwrap_or_default();

        let input = CreatePlanInput {
            title,
            description: Some(description),
            session_id: Some("fleet".to_string()),
            assigned_agent: Some(worker_id.clone()),
            created_by: Some(worker_id),
            agent_scope: PlanScope::Fleet,
            parent_plan_id: Some(parent_plan_id),
            task_id: None,
        };

        let plan = self.db.create_plan(input)?;
        emit_plan_changed_global(PlanChangedPayload {
            plan_id: Some(plan.id.clone()),
            kind: "subplan_created".to_string(),
            status: Some(plan.status.as_str().to_string()),
        });
        Ok(serde_json::to_value(&plan)?)
    }

    fn submit_for_qc(&self, doc: &serde_yaml::Value) -> anyhow::Result<Value> {
        let plan_id = self.get_str(doc, "plan_id")?;
        let worker_id = self.get_str(doc, "worker_id")?;
        let summary = self
            .get_optional_str(doc, "result_summary")
            .unwrap_or_default();
        let pr_urls = self
            .get_optional_str_array(doc, "pr_urls")
            .unwrap_or_default();

        // Update plan status to active (ready for QC)
        self.db.update_plan_status(&plan_id, "active")?;

        emit_plan_changed_global(PlanChangedPayload {
            plan_id: Some(plan_id.clone()),
            kind: "submitted_for_qc".to_string(),
            status: Some("active".to_string()),
        });

        Ok(json!({
            "plan_id": plan_id,
            "worker_id": worker_id,
            "submitted": true,
            "result_summary": summary,
            "pr_urls": pr_urls,
        }))
    }

    // ─── YAML Helpers ──────────────────────────────────────────────────────

    fn get_str(&self, doc: &serde_yaml::Value, key: &str) -> anyhow::Result<String> {
        doc.get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing or non-string field: '{key}'"))
    }

    fn get_optional_str(&self, doc: &serde_yaml::Value, key: &str) -> Option<String> {
        doc.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
    }

    fn get_i64(&self, doc: &serde_yaml::Value, key: &str) -> Option<i64> {
        doc.get(key).and_then(|v| v.as_i64())
    }

    fn get_optional_i64(&self, doc: &serde_yaml::Value, key: &str) -> Option<i64> {
        doc.get(key).and_then(|v| v.as_i64())
    }

    fn get_usize(&self, doc: &serde_yaml::Value, key: &str) -> Option<usize> {
        doc.get(key)
            .and_then(|v| v.as_i64())
            .and_then(|n| n.try_into().ok())
    }

    fn get_optional_bool(&self, doc: &serde_yaml::Value, key: &str) -> Option<bool> {
        doc.get(key).and_then(|v| v.as_bool())
    }

    fn get_optional_str_array(&self, doc: &serde_yaml::Value, key: &str) -> Option<Vec<String>> {
        doc.get(key).and_then(|v| v.as_sequence()).map(|seq| {
            seq.iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect()
        })
    }
}
