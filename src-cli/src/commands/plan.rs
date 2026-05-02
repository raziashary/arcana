use anyhow::Result;
use clap::{Args, Subcommand};
use autodev_shared::db::plans::PlansDb;
use crate::output::{OutputContext, status_colored};

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

    /// Scope: personal or team
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
    pub id: i64,
}

#[derive(Args)]
pub struct AddStepArgs {
    /// Plan ID
    pub plan_id: i64,
    /// Step title
    pub title: String,
    /// Optional description
    #[arg(short, long)]
    pub description: Option<String>,
}

#[derive(Args)]
pub struct UpdateStepArgs {
    /// Step ID
    pub step_id: i64,
    /// New status: pending, in_progress, completed, skipped
    pub status: String,
}

#[derive(Args)]
pub struct ActivateArgs {
    /// Plan ID
    pub id: i64,
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
    let plan = db.create(&args.title, args.description.as_deref(), args.scope.as_deref(), "airsctl")?;
    ctx.success(&format!("Created plan #{}", plan.id));
    print_plan_detail(&ctx, &plan, &[]);
    Ok(())
}

fn list(args: ListArgs, ctx: OutputContext) -> Result<()> {
    let db = PlansDb::open()?;
    let plans = db.list(args.status.as_deref())?;

    if plans.is_empty() {
        ctx.info("No plans found.");
        return Ok(());
    }

    let headers = ["ID", "Title", "Status", "Scope", "Created By", "Created"];
    let rows: Vec<Vec<String>> = plans.iter().map(|p| vec![
        p.id.to_string(),
        truncate(&p.title, 40),
        status_colored(&p.status),
        p.scope.clone(),
        p.created_by.clone(),
        p.created_at[..10].to_string(),
    ]).collect();

    ctx.print_table(&headers, rows);
    Ok(())
}

fn get(args: GetArgs, ctx: OutputContext) -> Result<()> {
    let db = PlansDb::open()?;
    match db.get(args.id)? {
        None => ctx.error(&format!("Plan #{} not found", args.id)),
        Some(plan) => {
            let steps = db.list_steps(plan.id)?;
            print_plan_detail(&ctx, &plan, &steps);
        }
    }
    Ok(())
}

fn add_step(args: AddStepArgs, ctx: OutputContext) -> Result<()> {
    let db = PlansDb::open()?;
    let step = db.add_step(args.plan_id, &args.title, args.description.as_deref())?;
    ctx.success(&format!("Added step #{} to plan #{}", step.id, args.plan_id));
    ctx.print_kv(&[
        ("step_id",     &step.id.to_string()),
        ("title",       &step.title),
        ("status",      &step.status),
        ("order_index", &step.order_index.to_string()),
    ]);
    Ok(())
}

fn update_step(args: UpdateStepArgs, ctx: OutputContext) -> Result<()> {
    let db = PlansDb::open()?;
    db.update_step_status(args.step_id, &args.status)?;
    ctx.success(&format!("Step #{} status updated to '{}'", args.step_id, args.status));
    Ok(())
}

fn activate(args: ActivateArgs, ctx: OutputContext) -> Result<()> {
    let db = PlansDb::open()?;
    db.activate(args.id)?;
    ctx.success(&format!("Plan #{} is now active", args.id));
    Ok(())
}

fn print_plan_detail(
    ctx: &OutputContext,
    p: &autodev_shared::db::plans::Plan,
    steps: &[autodev_shared::db::plans::PlanStep],
) {
    ctx.print_kv(&[
        ("id",          &p.id.to_string()),
        ("title",       &p.title),
        ("status",      &p.status),
        ("scope",       &p.scope),
        ("created_by",  &p.created_by),
        ("description", p.description.as_deref().unwrap_or("-")),
        ("created_at",  &p.created_at),
        ("updated_at",  &p.updated_at),
    ]);

    if !steps.is_empty() {
        ctx.info("\nSteps:");
        let headers = ["#", "ID", "Title", "Status"];
        let rows: Vec<Vec<String>> = steps.iter().map(|s| vec![
            s.order_index.to_string(),
            s.id.to_string(),
            s.title.clone(),
            status_colored(&s.status),
        ]).collect();
        ctx.print_table(&headers, rows);
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() }
    else { format!("{}…", &s[..max.saturating_sub(1)]) }
}
