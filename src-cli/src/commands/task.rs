use anyhow::Result;
use clap::{Args, Subcommand};
use autodev_shared::db::tasks::{TasksDb, CreateTaskInput};
use crate::output::{OutputContext, status_colored};

#[derive(Subcommand)]
pub enum TaskCmd {
    /// Create a new task
    Create(CreateArgs),
    /// List tasks
    List(ListArgs),
    /// Get task details
    Get(GetArgs),
    /// Assign a task to a worker
    Assign(AssignArgs),
    /// Claim the next ready task (worker self-assign)
    Claim(ClaimArgs),
    /// Mark a task as done
    Complete(CompleteArgs),
    /// Mark a task as failed
    Fail(FailArgs),
    /// Delete a task
    Delete(DeleteArgs),
}

#[derive(Args)]
pub struct CreateArgs {
    /// Task title
    pub title: String,

    /// Optional description
    #[arg(short, long)]
    pub description: Option<String>,

    /// Priority: p0, p1, p2, p3
    #[arg(short, long, default_value = "p2")]
    pub priority: String,

    /// Type: feature, bug, refactor, docs, chore
    #[arg(short = 't', long, default_value = "feature")]
    pub task_type: String,

    /// Acceptance criteria
    #[arg(short, long)]
    pub acceptance_criteria: Option<String>,
}

#[derive(Args)]
pub struct ListArgs {
    /// Filter by status (ready, assigned, in_progress, qc_review, done, failed, cancelled)
    #[arg(short, long)]
    pub status: Option<String>,

    /// Filter by worker slug
    #[arg(short, long)]
    pub worker: Option<String>,

    /// Max rows to display
    #[arg(short, long, default_value = "20")]
    pub limit: usize,
}

#[derive(Args)]
pub struct GetArgs {
    /// Task ID
    pub id: i64,
}

#[derive(Args)]
pub struct AssignArgs {
    /// Task ID
    pub id: i64,
    /// Worker slug to assign to
    pub worker: String,
}

#[derive(Args)]
pub struct ClaimArgs {
    /// Worker slug claiming the task
    pub worker: String,
}

#[derive(Args)]
pub struct CompleteArgs {
    /// Task ID
    pub id: i64,
}

#[derive(Args)]
pub struct FailArgs {
    /// Task ID
    pub id: i64,
}

#[derive(Args)]
pub struct DeleteArgs {
    /// Task ID
    pub id: i64,
}

pub async fn run(cmd: TaskCmd, ctx: OutputContext) -> Result<()> {
    match cmd {
        TaskCmd::Create(args) => create(args, ctx),
        TaskCmd::List(args) => list(args, ctx),
        TaskCmd::Get(args) => get(args, ctx),
        TaskCmd::Assign(args) => assign(args, ctx),
        TaskCmd::Claim(args) => claim(args, ctx),
        TaskCmd::Complete(args) => complete(args, ctx),
        TaskCmd::Fail(args) => fail(args, ctx),
        TaskCmd::Delete(args) => delete(args, ctx),
    }
}

fn create(args: CreateArgs, ctx: OutputContext) -> Result<()> {
    let db = TasksDb::open()?;
    let input = CreateTaskInput {
        title: args.title,
        description: args.description,
        priority: Some(args.priority),
        task_type: Some(args.task_type),
        acceptance_criteria: args.acceptance_criteria,
        created_by: Some("airsctl".into()),
    };
    let task = db.create(input)?;
    ctx.success(&format!("Created task #{}", task.id));
    print_task_detail(&ctx, &task);
    Ok(())
}

fn list(args: ListArgs, ctx: OutputContext) -> Result<()> {
    let db = TasksDb::open()?;
    let tasks = db.list(args.status.as_deref(), args.worker.as_deref())?;
    let tasks: Vec<_> = tasks.into_iter().take(args.limit).collect();

    if tasks.is_empty() {
        ctx.info("No tasks found.");
        return Ok(());
    }

    let headers = ["ID", "Title", "Status", "Priority", "Type", "Worker", "Created"];
    let rows: Vec<Vec<String>> = tasks.iter().map(|t| vec![
        t.id.to_string(),
        truncate(&t.title, 40),
        status_colored(&t.status.to_string()),
        t.priority.to_string(),
        t.task_type.to_string(),
        t.assigned_worker.clone().unwrap_or_else(|| "-".into()),
        t.created_at[..10].to_string(),
    ]).collect();

    ctx.print_table(&headers, rows);
    Ok(())
}

fn get(args: GetArgs, ctx: OutputContext) -> Result<()> {
    let db = TasksDb::open()?;
    match db.get(args.id)? {
        None => ctx.error(&format!("Task #{} not found", args.id)),
        Some(t) => print_task_detail(&ctx, &t),
    }
    Ok(())
}

fn assign(args: AssignArgs, ctx: OutputContext) -> Result<()> {
    let db = TasksDb::open()?;
    db.assign(args.id, &args.worker)?;
    ctx.success(&format!("Assigned task #{} to worker '{}'", args.id, args.worker));
    Ok(())
}

fn claim(args: ClaimArgs, ctx: OutputContext) -> Result<()> {
    let db = TasksDb::open()?;
    match db.claim_next(&args.worker)? {
        None => ctx.info("No ready tasks available."),
        Some(t) => {
            ctx.success(&format!("Claimed task #{}: {}", t.id, t.title));
            print_task_detail(&ctx, &t);
        }
    }
    Ok(())
}

fn complete(args: CompleteArgs, ctx: OutputContext) -> Result<()> {
    let db = TasksDb::open()?;
    db.update_status(args.id, "done", None)?;
    ctx.success(&format!("Task #{} marked as done", args.id));
    Ok(())
}

fn fail(args: FailArgs, ctx: OutputContext) -> Result<()> {
    let db = TasksDb::open()?;
    db.update_status(args.id, "failed", None)?;
    ctx.warn(&format!("Task #{} marked as failed", args.id));
    Ok(())
}

fn delete(args: DeleteArgs, ctx: OutputContext) -> Result<()> {
    let db = TasksDb::open()?;
    db.delete(args.id)?;
    ctx.success(&format!("Deleted task #{}", args.id));
    Ok(())
}

fn print_task_detail(ctx: &OutputContext, t: &autodev_shared::db::tasks::Task) {
    ctx.print_kv(&[
        ("id",                  &t.id.to_string()),
        ("title",               &t.title),
        ("status",              &t.status.to_string()),
        ("priority",            &t.priority.to_string()),
        ("type",                &t.task_type.to_string()),
        ("worker",              t.assigned_worker.as_deref().unwrap_or("-")),
        ("description",         t.description.as_deref().unwrap_or("-")),
        ("acceptance_criteria", &t.acceptance_criteria),
        ("created_by",          &t.created_by),
        ("created_at",          &t.created_at),
        ("updated_at",          &t.updated_at),
    ]);
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() }
    else { format!("{}…", &s[..max.saturating_sub(1)]) }
}
