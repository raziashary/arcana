use anyhow::Result;
use clap::{Args, Subcommand};
use autodev_shared::db::workers::{WorkersDb, CreateWorkerInput};
use crate::output::{OutputContext, status_colored};

#[derive(Subcommand)]
pub enum WorkerCmd {
    /// Register a new worker
    Register(RegisterArgs),
    /// List workers
    List(ListArgs),
    /// Show worker status
    Status(StatusArgs),
    /// Send a heartbeat for a worker
    Heartbeat(HeartbeatArgs),
    /// Unregister (soft-delete) a worker
    Unregister(UnregisterArgs),
}

#[derive(Args)]
pub struct RegisterArgs {
    /// Worker slug (unique short name, e.g. "backend-worker-1")
    pub slug: String,

    /// Human-readable display name
    #[arg(short, long)]
    pub name: Option<String>,

    /// Worker type (code, test, review, deploy, general, custom)
    #[arg(short = 't', long, default_value = "custom")]
    pub worker_type: String,

    /// Default model (e.g. claude-sonnet-4-6)
    #[arg(short, long)]
    pub model: Option<String>,

    /// Optional description
    #[arg(short, long)]
    pub description: Option<String>,
}

#[derive(Args)]
pub struct ListArgs {
    /// Filter by type
    #[arg(short = 't', long)]
    pub worker_type: Option<String>,

    /// Filter by status (created, active, deleted)
    #[arg(short, long)]
    pub status: Option<String>,
}

#[derive(Args)]
pub struct StatusArgs {
    /// Worker slug. If omitted, shows all workers.
    pub slug: Option<String>,
}

#[derive(Args)]
pub struct HeartbeatArgs {
    /// Worker slug
    pub slug: String,
}

#[derive(Args)]
pub struct UnregisterArgs {
    /// Worker slug
    pub slug: String,
}

pub async fn run(cmd: WorkerCmd, ctx: OutputContext) -> Result<()> {
    match cmd {
        WorkerCmd::Register(args) => register(args, ctx),
        WorkerCmd::List(args) => list(args, ctx),
        WorkerCmd::Status(args) => status(args, ctx),
        WorkerCmd::Heartbeat(args) => heartbeat(args, ctx),
        WorkerCmd::Unregister(args) => unregister(args, ctx),
    }
}

fn register(args: RegisterArgs, ctx: OutputContext) -> Result<()> {
    let db = WorkersDb::open()?;
    let input = CreateWorkerInput {
        slug: args.slug.clone(),
        name: args.name.unwrap_or_else(|| args.slug.clone()),
        worker_type: args.worker_type,
        model: args.model,
        description: args.description,
    };
    let worker = db.create(input)?;
    ctx.success(&format!("Registered worker '{}' (id: {})", worker.slug, worker.id));
    print_worker_detail(&ctx, &worker);
    Ok(())
}

fn list(args: ListArgs, ctx: OutputContext) -> Result<()> {
    let db = WorkersDb::open()?;
    let workers = db.list(args.worker_type.as_deref(), args.status.as_deref())?;

    if workers.is_empty() {
        ctx.info("No workers found.");
        return Ok(());
    }

    let headers = ["ID", "Slug", "Name", "Type", "Status", "Model", "Last Seen"];
    let rows: Vec<Vec<String>> = workers.iter().map(|w| vec![
        w.id.to_string(),
        w.slug.clone(),
        w.name.clone(),
        w.worker_type.clone(),
        status_colored(&w.status),
        w.model.clone(),
        w.last_seen.as_deref().unwrap_or("-").to_string(),
    ]).collect();

    ctx.print_table(&headers, rows);
    Ok(())
}

fn status(args: StatusArgs, ctx: OutputContext) -> Result<()> {
    let db = WorkersDb::open()?;
    match args.slug {
        None => {
            let workers = db.list(None, None)?;
            if workers.is_empty() {
                ctx.info("No workers registered.");
                return Ok(());
            }
            let headers = ["ID", "Slug", "Status", "Type", "Last Seen"];
            let rows: Vec<Vec<String>> = workers.iter().map(|w| vec![
                w.id.to_string(),
                w.slug.clone(),
                status_colored(&w.status),
                w.worker_type.clone(),
                w.last_seen.as_deref().unwrap_or("-").to_string(),
            ]).collect();
            ctx.print_table(&headers, rows);
        }
        Some(slug) => {
            match db.get_by_slug(&slug)? {
                None => ctx.error(&format!("Worker '{slug}' not found")),
                Some(w) => print_worker_detail(&ctx, &w),
            }
        }
    }
    Ok(())
}

fn heartbeat(args: HeartbeatArgs, ctx: OutputContext) -> Result<()> {
    let db = WorkersDb::open()?;
    db.heartbeat(&args.slug)?;
    ctx.success(&format!("Heartbeat sent for worker '{}'", args.slug));
    Ok(())
}

fn unregister(args: UnregisterArgs, ctx: OutputContext) -> Result<()> {
    let db = WorkersDb::open()?;
    db.unregister(&args.slug)?;
    ctx.success(&format!("Unregistered worker '{}'", args.slug));
    Ok(())
}

fn print_worker_detail(ctx: &OutputContext, w: &autodev_shared::db::workers::Worker) {
    ctx.print_kv(&[
        ("id",          &w.id.to_string()),
        ("slug",        &w.slug),
        ("name",        &w.name),
        ("type",        &w.worker_type),
        ("status",      &w.status),
        ("model",       &w.model),
        ("description", w.description.as_deref().unwrap_or("-")),
        ("last_seen",   w.last_seen.as_deref().unwrap_or("-")),
        ("created_at",  &w.created_at),
        ("updated_at",  &w.updated_at),
    ]);
}
