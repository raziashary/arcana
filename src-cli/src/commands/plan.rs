use crate::output::{status_colored, OutputContext};
use anyhow::Result;
use arcana_core::db::plans::{
    CreatePlanInput, CreatePlanItemInput, Plan, PlanItem, PlanScope, PlansDb,
};
use clap::{Args, Subcommand};

#[derive(Subcommand)]
pub enum PlanCmd {
    /// Create a new plan
    Create(CreateArgs),
    /// List plans
    List(ListArgs),
    /// Get plan details with steps
    Get(GetArgs),
    /// Add a step to a plan
    AddStep(AddStepArgs),
    /// Update step status
    UpdateStep(UpdateStepArgs),
    /// Activate a draft plan
    Activate(ActivateArgs),
}

#[derive(Args)]
pub struct CreateArgs {
    /// Plan title
    pub title: String,

    /// Optional description
    #[arg(short, long)]
    pub description: Option<String>,

    /// Scope: personal, fleet or epic
    ///
    /// The old help text said "personal or team"; `team` was never a value the
    /// store understood and silently fell back to `personal`.
    #[arg(short, long, default_value = "personal")]
    pub scope: Option<String>,
}

#[derive(Args)]
pub struct ListArgs {
    /// Filter by status (draft, active, completed, archived)
    #[arg(short, long)]
    pub status: Option<String>,
}

#[derive(Args)]
pub struct GetArgs {
    /// Plan ID
    pub id: String,
}

#[derive(Args)]
pub struct AddStepArgs {
    /// Plan ID
    pub plan_id: String,
    /// Step title
    pub title: String,
    /// Optional description
    #[arg(short, long)]
    pub description: Option<String>,
}

#[derive(Args)]
pub struct UpdateStepArgs {
    /// Step ID
    pub step_id: String,
    /// New status: pending, in_progress, completed, skipped
    pub status: String,
}

#[derive(Args)]
pub struct ActivateArgs {
    /// Plan ID
    pub id: String,
}

pub async fn run(cmd: PlanCmd, ctx: OutputContext) -> Result<()> {
    match cmd {
        PlanCmd::Create(args) => create(args, ctx),
        PlanCmd::List(args) => list(args, ctx),
        PlanCmd::Get(args) => get(args, ctx),
        PlanCmd::AddStep(args) => add_step(args, ctx),
        PlanCmd::UpdateStep(args) => update_step(args, ctx),
        PlanCmd::Activate(args) => activate(args, ctx),
    }
}

fn create(args: CreateArgs, ctx: OutputContext) -> Result<()> {
    let db = PlansDb::open()?;
    let plan = db.create_plan(CreatePlanInput {
        title: args.title,
        description: args.description,
        session_id: None,
        assigned_agent: None,
        created_by: Some("arcanactl".into()),
        agent_scope: PlanScope::from_str(args.scope.as_deref().unwrap_or("personal")),
        parent_plan_id: None,
        task_id: None,
    })?;
    ctx.success(&format!("Created plan {}", plan.id));
    print_plan_detail(&ctx, &plan, &[]);
    Ok(())
}

fn list(args: ListArgs, ctx: OutputContext) -> Result<()> {
    let db = PlansDb::open()?;
    let plans = db.list_plans(args.status.as_deref(), None, None)?;

    if plans.is_empty() {
        ctx.info("No plans found.");
        return Ok(());
    }

    let headers = ["ID", "Title", "Status", "Scope", "Created By", "Created"];
    let rows: Vec<Vec<String>> = plans
        .iter()
        .map(|p| {
            vec![
                p.id.clone(),
                truncate(&p.title, 40),
                status_colored(p.status.as_str()),
                p.agent_scope.as_str().to_string(),
                p.created_by.clone().unwrap_or_else(|| "-".into()),
                p.created_at[..10].to_string(),
            ]
        })
        .collect();

    ctx.print_table(&headers, rows);
    Ok(())
}

fn get(args: GetArgs, ctx: OutputContext) -> Result<()> {
    let db = PlansDb::open()?;
    match db.get_plan(&args.id)? {
        None => ctx.error(&format!("Plan {} not found", args.id)),
        Some(plan) => {
            let steps = db.list_plan_items(&plan.id)?;
            print_plan_detail(&ctx, &plan, &steps);
        }
    }
    Ok(())
}

fn add_step(args: AddStepArgs, ctx: OutputContext) -> Result<()> {
    let db = PlansDb::open()?;
    // Core takes an explicit order_index; append after whatever is already
    // there rather than assuming the caller tracks it.
    let order_index = db.list_plan_items(&args.plan_id)?.len() as i64;
    let step = db.add_plan_item(CreatePlanItemInput {
        plan_id: args.plan_id.clone(),
        parent_id: None,
        title: args.title,
        description: args.description,
        order_index,
        agent_type: None,
        estimated_tools: None,
        assigned_worker: None,
        worktree_branch: None,
    })?;
    ctx.success(&format!("Added step {} to plan {}", step.id, args.plan_id));
    ctx.print_kv(&[
        ("step_id", &step.id),
        ("title", &step.title),
        ("status", step.status.as_str()),
        ("order_index", &step.order_index.to_string()),
    ]);
    Ok(())
}

fn update_step(args: UpdateStepArgs, ctx: OutputContext) -> Result<()> {
    let db = PlansDb::open()?;
    db.update_plan_item_status(&args.step_id, &args.status, None)?;
    ctx.success(&format!(
        "Step {} status updated to '{}'",
        args.step_id, args.status
    ));
    Ok(())
}

fn activate(args: ActivateArgs, ctx: OutputContext) -> Result<()> {
    let db = PlansDb::open()?;
    db.update_plan_status(&args.id, "active")?;
    ctx.success(&format!("Plan {} is now active", args.id));
    Ok(())
}

fn print_plan_detail(ctx: &OutputContext, p: &Plan, steps: &[PlanItem]) {
    ctx.print_kv(&[
        ("id", &p.id),
        ("title", &p.title),
        ("status", p.status.as_str()),
        ("scope", p.agent_scope.as_str()),
        ("created_by", p.created_by.as_deref().unwrap_or("-")),
        ("description", p.description.as_deref().unwrap_or("-")),
        ("created_at", &p.created_at),
        ("updated_at", &p.updated_at),
    ]);

    if !steps.is_empty() {
        ctx.info("\nSteps:");
        let headers = ["#", "ID", "Title", "Status"];
        let rows: Vec<Vec<String>> = steps
            .iter()
            .map(|s| {
                vec![
                    s.order_index.to_string(),
                    s.id.clone(),
                    s.title.clone(),
                    status_colored(s.status.as_str()),
                ]
            })
            .collect();
        ctx.print_table(&headers, rows);
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}
