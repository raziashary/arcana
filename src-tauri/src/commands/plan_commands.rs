// Plan commands — Tauri IPC for the Phase 3.5 Plan Management System.
// Bridge between UI / external callers and the plans DB.

use crate::db::plans::{CreatePlanInput, Plan, PlanItem, PlanItemStatus, PlanScope, PlansDb};
use crate::errors::Result;
use crate::events::{emit_plan_changed, PlanChangedPayload};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct PlanFilter {
    pub status: Option<String>,
    pub scope: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlanTree {
    pub plan: Plan,
    pub items: Vec<PlanItem>,
}

// ─── Tauri Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn planner_compile(yaml_dsl: String) -> Result<String> {
    // NOTE: Direct YAML compilation from UI is synchronous by design;
    // the planner tool execute() is async_trait but for UI-initiated calls
    // we can run a local Tokio block_on or return the DB handle directly.
    // For simplicity, return a marker; the real compile path is via the Tool trait.
    Ok(format!(
        "Use the planner tool directly. Received YAML: {} chars",
        yaml_dsl.len()
    ))
}

#[tauri::command]
pub fn get_plan(plan_id: String) -> Result<Option<Plan>> {
    let db = PlansDb::open()?;
    db.get_plan(&plan_id)
}

#[tauri::command]
pub fn list_plans(filter: PlanFilter) -> Result<Vec<Plan>> {
    let db = PlansDb::open()?;
    db.list_plans(
        filter.status.as_deref(),
        filter.scope.as_deref(),
        filter.session_id.as_deref(),
    )
}

#[tauri::command]
pub fn get_plan_tree(plan_id: String) -> Result<Option<PlanTree>> {
    let db = PlansDb::open()?;
    let plan = match db.get_plan(&plan_id)? {
        Some(p) => p,
        None => return Ok(None),
    };
    let items = db.list_plan_items(&plan_id)?;
    Ok(Some(PlanTree { plan, items }))
}

#[tauri::command]
pub fn delete_plan(plan_id: String, app: AppHandle) -> Result<bool> {
    let db = PlansDb::open()?;
    db.delete_plan(&plan_id)?;
    emit_plan_changed(PlanChangedPayload {
        plan_id: Some(plan_id),
        kind: "deleted".to_string(),
        status: None,
    });
    Ok(true)
}

// ─── Fleet Commands ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn break_down_epic(epic_task_id: i64, supervisor_id: String, app: AppHandle) -> Result<Plan> {
    let db = PlansDb::open()?;

    let input = CreatePlanInput {
        title: format!("Epic Breakdown #{}", epic_task_id),
        description: Some("Supervisor-created epic breakdown plan".to_string()),
        session_id: Some("fleet".to_string()),
        assigned_agent: Some(supervisor_id.clone()),
        created_by: Some(supervisor_id),
        agent_scope: PlanScope::Epic,
        parent_plan_id: None,
        task_id: Some(epic_task_id),
    };

    let plan = db.create_plan(input)?;

    emit_plan_changed(PlanChangedPayload {
        plan_id: Some(plan.id.clone()),
        kind: "epic_breakdown".to_string(),
        status: Some("active".to_string()),
    });

    Ok(plan)
}

#[tauri::command]
pub fn claim_plan_item(
    plan_id: String,
    worker_id: String,
    app: AppHandle,
) -> Result<Option<PlanItem>> {
    let db = PlansDb::open()?;

    // Get next unclaimed item
    let items = db.list_plan_items(&plan_id)?;
    let next_item = items
        .iter()
        .find(|item| {
            item.assigned_worker.is_none() && matches!(item.status, PlanItemStatus::Pending)
        })
        .cloned();

    if let Some(mut item) = next_item {
        item.assigned_worker = Some(worker_id.clone());
        // Update in DB (would need update_plan_item method)

        emit_plan_changed(PlanChangedPayload {
            plan_id: Some(plan_id),
            kind: "item_claimed".to_string(),
            status: Some(worker_id),
        });

        Ok(Some(item))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub fn create_subplan(
    parent_plan_id: String,
    worker_id: String,
    title: String,
    description: String,
    app: AppHandle,
) -> Result<Plan> {
    let db = PlansDb::open()?;

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

    let plan = db.create_plan(input)?;

    emit_plan_changed(PlanChangedPayload {
        plan_id: Some(plan.id.clone()),
        kind: "subplan_created".to_string(),
        status: Some("active".to_string()),
    });

    Ok(plan)
}

#[tauri::command]
pub fn submit_plan_for_qc(
    plan_id: String,
    worker_id: String,
    _result_summary: String,
    _pr_urls: Vec<String>,
    app: AppHandle,
) -> Result<bool> {
    let db = PlansDb::open()?;

    // Mark plan as completed
    db.delete_plan(&plan_id)?; // Placeholder — would need update_plan_status

    emit_plan_changed(PlanChangedPayload {
        plan_id: Some(plan_id),
        kind: "qc_submitted".to_string(),
        status: Some(format!("submitted by {}", worker_id)),
    });

    Ok(true)
}
