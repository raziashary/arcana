use crate::output::{status_colored, OutputContext};
use anyhow::Result;
use autodev_core::db::tasks::{CreateTaskInput, Task, TasksDb};
use clap::{Args, Subcommand};

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
        priority: args.priority,
        task_type: args.task_type,
        acceptance_criteria: args.acceptance_criteria.unwrap_or_default(),
        created_by: Some("airsctl".into()),
    };
    let task = db.create(input)?;
    ctx.success(&format!("Created task #{}", task.id));
    print_task_detail(&ctx, &task);
    Ok(())
}

fn list(args: ListArgs, ctx: OutputContext) -> Result<()> {
    let db = TasksDb::open()?;
    // Core filters by status in SQL; the worker filter is applied here rather
    // than widening core's API for a caller-specific query.
    let tasks: Vec<Task> = db
        .list(args.status.as_deref())?
        .into_iter()
        .filter(|t| {
            args.worker
                .as_deref()
                .is_none_or(|w| t.assigned_worker.as_deref() == Some(w))
        })
        .take(args.limit)
        .collect();

    if tasks.is_empty() {
        ctx.info("No tasks found.");
        return Ok(());
    }

    let headers = [
        "ID", "Title", "Status", "Priority", "Type", "Worker", "Created",
    ];
    let rows: Vec<Vec<String>> = tasks
        .iter()
        .map(|t| {
            vec![
                t.id.to_string(),
                truncate(&t.title, 40),
                status_colored(t.status.as_str()),
                t.priority.as_str().to_string(),
                t.task_type.as_str().to_string(),
                t.assigned_worker.clone().unwrap_or_else(|| "-".into()),
                t.created_at[..10].to_string(),
            ]
        })
        .collect();

    ctx.print_table(&headers, rows);
    Ok(())
}

fn get(args: GetArgs, ctx: OutputContext) -> Result<()> {
    let db = TasksDb::open()?;
    match db.get_by_id(args.id)? {
        None => ctx.error(&format!("Task #{} not found", args.id)),
        Some(t) => print_task_detail(&ctx, &t),
    }
    Ok(())
}

fn assign(args: AssignArgs, ctx: OutputContext) -> Result<()> {
    let db = TasksDb::open()?;
    db.update_status(args.id, "assigned", Some(&args.worker))?;
    ctx.success(&format!(
        "Assigned task #{} to worker '{}'",
        args.id, args.worker
    ));
    Ok(())
}

fn claim(args: ClaimArgs, ctx: OutputContext) -> Result<()> {
    let db = TasksDb::open()?;

    // The queue policy — oldest ready task first — stays here; the claim itself
    // goes through core's `claim_task_with_pid`.
    //
    // ⚠ Behaviour change, deliberate (P1-09). `src-shared`'s `claim_next` wrote
    // `status` and `assigned_worker` and nothing else. Core's claim also records
    // the claiming PID and host, and refuses a worker that already holds an
    // in-progress task. Both matter: orphan detection releases stale tasks by
    // looking at `claimed_by_pid`, so a CLI-claimed task under the old path
    // could never be detected as orphaned, and a worker could hold several
    // tasks at once while the app assumed one.
    let next = db
        .list(Some("ready"))?
        .into_iter()
        .min_by(|a, b| a.created_at.cmp(&b.created_at));

    match next {
        None => ctx.info("No ready tasks available."),
        Some(t) => {
            // Core models the handoff in two steps — a task is assigned to a
            // worker, then that worker claims it — and `claim_task_with_pid`
            // refuses anything not already `assigned`. `src-shared` went
            // straight from `ready` to `in_progress`, so this walks both steps
            // to keep the CLI's one-command behaviour while going through the
            // guarded path.
            db.update_status(t.id, "assigned", Some(&args.worker))?;
            db.claim_task_with_pid(t.id, &args.worker, std::process::id())?;
            ctx.success(&format!("Claimed task #{}: {}", t.id, t.title));
            match db.get_by_id(t.id)? {
                Some(claimed) => print_task_detail(&ctx, &claimed),
                None => ctx.error("Task was claimed but could not be read back"),
            }
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

fn print_task_detail(ctx: &OutputContext, t: &Task) {
    ctx.print_kv(&[
        ("id", &t.id.to_string()),
        ("title", &t.title),
        ("status", t.status.as_str()),
        ("priority", t.priority.as_str()),
        ("type", t.task_type.as_str()),
        ("worker", t.assigned_worker.as_deref().unwrap_or("-")),
        ("description", t.description.as_deref().unwrap_or("-")),
        ("acceptance_criteria", &t.acceptance_criteria),
        ("created_by", &t.created_by),
        ("created_at", &t.created_at),
        ("updated_at", &t.updated_at),
    ]);
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}
