use crate::output::{status_colored, OutputContext};
use anyhow::Result;
use arcana_core::db::workers::{Worker, WorkersDb, DEFAULT_MODEL};
use clap::{Args, Subcommand};

/// Matches the `polling_interval` DEFAULT in core's workers schema.
const DEFAULT_POLLING_INTERVAL: i64 = 120;

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
    let name = args.name.unwrap_or_else(|| args.slug.clone());
    let model = args.model.as_deref().unwrap_or(DEFAULT_MODEL);

    // `primary_repos` and `polling_interval` have no CLI flags; the empty repo
    // list and the schema's own default interval keep a CLI-registered worker
    // identical to a UI-registered one.
    let id = db.create(
        &name,
        &args.slug,
        &args.worker_type,
        model,
        &[],
        DEFAULT_POLLING_INTERVAL,
        args.description.as_deref(),
    )?;
    ctx.success(&format!("Registered worker '{}' (id: {id})", args.slug));

    // Read back rather than echo the inputs: the row is what the app will see,
    // including any defaults SQLite filled in.
    match db.get_by_slug(&args.slug)? {
        Some(w) => print_worker_detail(&ctx, &w),
        None => ctx.error("Worker was created but could not be read back"),
    }
    Ok(())
}

fn list(args: ListArgs, ctx: OutputContext) -> Result<()> {
    let db = WorkersDb::open()?;
    // Core's `list()` returns every non-deleted worker; the CLI's two filters
    // are applied here rather than widening core's API for a caller-specific
    // query.
    let workers: Vec<Worker> = db
        .list()?
        .into_iter()
        .filter(|w| {
            args.worker_type
                .as_deref()
                .is_none_or(|t| w.worker_type == t)
        })
        .filter(|w| args.status.as_deref().is_none_or(|st| w.status == st))
        .collect();

    if workers.is_empty() {
        ctx.info("No workers found.");
        return Ok(());
    }

    let headers = ["ID", "Slug", "Name", "Type", "Status", "Model", "Last Seen"];
    let rows: Vec<Vec<String>> = workers
        .iter()
        .map(|w| {
            vec![
                w.id.to_string(),
                w.slug.clone(),
                w.name.clone(),
                w.worker_type.clone(),
                status_colored(&w.status),
                w.model.clone(),
                w.last_seen.as_deref().unwrap_or("-").to_string(),
            ]
        })
        .collect();

    ctx.print_table(&headers, rows);
    Ok(())
}

fn status(args: StatusArgs, ctx: OutputContext) -> Result<()> {
    let db = WorkersDb::open()?;
    match args.slug {
        None => {
            let workers = db.list()?;
            if workers.is_empty() {
                ctx.info("No workers registered.");
                return Ok(());
            }
            let headers = ["ID", "Slug", "Status", "Type", "Last Seen"];
            let rows: Vec<Vec<String>> = workers
                .iter()
                .map(|w| {
                    vec![
                        w.id.to_string(),
                        w.slug.clone(),
                        status_colored(&w.status),
                        w.worker_type.clone(),
                        w.last_seen.as_deref().unwrap_or("-").to_string(),
                    ]
                })
                .collect();
            ctx.print_table(&headers, rows);
        }
        Some(slug) => match db.get_by_slug(&slug)? {
            None => ctx.error(&format!("Worker '{slug}' not found")),
            Some(w) => print_worker_detail(&ctx, &w),
        },
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
    db.deactivate(&args.slug)?;
    ctx.success(&format!("Unregistered worker '{}'", args.slug));
    Ok(())
}

fn print_worker_detail(ctx: &OutputContext, w: &Worker) {
    ctx.print_kv(&[
        ("id", &w.id.to_string()),
        ("slug", &w.slug),
        ("name", &w.name),
        ("type", &w.worker_type),
        ("status", &w.status),
        ("model", &w.model),
        ("description", w.description.as_deref().unwrap_or("-")),
        ("last_seen", w.last_seen.as_deref().unwrap_or("-")),
        ("created_at", &w.created_at),
        ("updated_at", &w.updated_at),
    ]);
}
