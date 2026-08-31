use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use futures::StreamExt;
use crate::output::OutputContext;
use crate::noninteractive;

#[derive(Subcommand)]
pub enum AgentCmd {
    /// Run an agent with a task (streams output)
    Run(RunArgs),
    /// List recent agent sessions
    List(ListArgs),
    /// Get agent session details
    Status(StatusArgs),
    /// Stop a running agent session
    Stop(StopArgs),
    /// Tail logs for an agent session
    Logs(LogsArgs),
}

#[derive(Args)]
pub struct RunArgs {
    /// Task or prompt for the agent (omit to use --file)
    pub task: Option<String>,

    /// Read the prompt from a file instead of the command line
    #[arg(long, short)]
    pub file: Option<String>,

    /// Resume an existing session by ID
    #[arg(long)]
    pub session_id: Option<String>,

    /// Stream output in real time (default: true)
    #[arg(long, default_value = "true")]
    pub stream: bool,

    /// Timeout in seconds
    #[arg(long, default_value = "300")]
    pub timeout: u64,
}

#[derive(Args)]
pub struct ListArgs {
    /// Maximum sessions to show
    #[arg(short, long, default_value = "20")]
    pub limit: usize,
}

#[derive(Args)]
pub struct StatusArgs {
    /// Session ID
    pub session_id: String,
}

#[derive(Args)]
pub struct StopArgs {
    /// Session ID
    pub session_id: String,
}

#[derive(Args)]
pub struct LogsArgs {
    /// Session ID
    pub session_id: String,
    /// Number of lines to tail
    #[arg(short, long, default_value = "100")]
    pub lines: usize,
}

// ── Orchestrator API types ────────────────────────────────────────────────────

#[derive(Serialize)]
struct InvokeRequest {
    task: String,
    session_id: Option<String>,
    stream: bool,
    provider: Option<String>,
    model: Option<String>,
    working_dir: Option<String>,
}

#[derive(Deserialize, Debug)]
struct AgentSession {
    id: String,
    status: String,
    task: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    provider: Option<String>,
    model: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ListResponse {
    sessions: Vec<AgentSession>,
}

// SSE event from the orchestrator
#[derive(Deserialize, Debug)]
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    content: Option<String>,
    error: Option<String>,
    session_id: Option<String>,
}

pub async fn run(cmd: AgentCmd, ctx: OutputContext) -> Result<()> {
    match cmd {
        AgentCmd::Run(args) => agent_run(args, ctx).await,
        AgentCmd::List(args) => agent_list(args, ctx).await,
        AgentCmd::Status(args) => agent_status(args, ctx).await,
        AgentCmd::Stop(args) => agent_stop(args, ctx).await,
        AgentCmd::Logs(args) => agent_logs(args, ctx).await,
    }
}

async fn agent_run(args: RunArgs, ctx: OutputContext) -> Result<()> {
    // Resolve the task prompt from inline arg or --file
    let task = match (&args.task, &args.file) {
        (Some(t), _) => t.clone(),
        (None, Some(f)) => noninteractive::read_prompt_file(f)?,
        (None, None) => {
            return Err(anyhow::anyhow!(
                "Provide a task prompt as an argument or use --file <PATH>"
            ));
        }
    };

    let client = build_client(args.timeout)?;

    let url = format!("{}/api/agents/invoke", ctx.orchestrator_url);
    let body = InvokeRequest {
        task: task.clone(),
        session_id: args.session_id.clone(),
        stream: args.stream,
        provider: ctx.provider.clone(),
        model: ctx.model.clone(),
        working_dir: ctx.working_dir.clone(),
    };

    if !ctx.quiet {
        println!("{} {}", "→ Running task:".cyan(), task.bold());
        if let Some(sid) = &args.session_id {
            println!("{} {}", "  Session:".dimmed(), sid);
        }
    }

    let req = client.post(&url).json(&body);

    let resp = req.send().await.map_err(|e| {
        anyhow::anyhow!(
            "Cannot reach core-orchestrator at {}.\n  \
             Start it with: cargo run --bin core-orchestrator\n  Error: {}",
            ctx.orchestrator_url, e
        )
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("Orchestrator returned {status}: {body}"));
    }

    let content_type = resp.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if content_type.contains("text/event-stream") {
        // Streaming SSE response
        stream_sse_response(resp, &ctx).await
    } else {
        // Non-streaming JSON response
        let session: AgentSession = resp.json().await?;
        print_session(&ctx, &session);
        Ok(())
    }
}

async fn stream_sse_response(resp: reqwest::Response, ctx: &OutputContext) -> Result<()> {
    let pb = if !ctx.quiet {
        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap());
        pb.set_message("Agent working…");
        pb.enable_steady_tick(Duration::from_millis(100));
        Some(pb)
    } else {
        None
    };

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut session_id: Option<String> = None;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buf.push_str(&String::from_utf8_lossy(&chunk));

        // SSE lines are terminated by \n\n
        while let Some(pos) = buf.find("\n\n") {
            let block = buf[..pos].to_string();
            buf.drain(..pos + 2);

            for line in block.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        if let Some(pb) = &pb { pb.finish_and_clear(); }
                        if !ctx.quiet {
                            if let Some(sid) = &session_id {
                                println!("{} session {}", "✓ Completed".green().bold(), sid.dimmed());
                            } else {
                                println!("{}", "✓ Completed".green().bold());
                            }
                        }
                        return Ok(());
                    }

                    if let Ok(event) = serde_json::from_str::<StreamEvent>(data) {
                        if let Some(sid) = &event.session_id {
                            session_id = Some(sid.clone());
                        }

                        match event.event_type.as_str() {
                            "content" | "text" => {
                                if let Some(content) = &event.content {
                                    if let Some(pb) = &pb { pb.suspend(|| print!("{content}")); }
                                    else { print!("{content}"); }
                                }
                            }
                            "error" => {
                                if let Some(pb) = &pb { pb.finish_and_clear(); }
                                let err = event.error.as_deref().unwrap_or("Unknown error");
                                return Err(anyhow::anyhow!("Agent error: {err}"));
                            }
                            "status" => {
                                if let Some(content) = &event.content {
                                    if let Some(pb) = &pb { pb.set_message(content.clone()); }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    if let Some(pb) = pb { pb.finish_and_clear(); }
    Ok(())
}

async fn agent_list(args: ListArgs, ctx: OutputContext) -> Result<()> {
    let client = build_client(30)?;
    let url = format!("{}/api/agents?limit={}", ctx.orchestrator_url, args.limit);

    let req = client.get(&url);

    let resp = req.send().await.map_err(|e| {
        anyhow::anyhow!("Cannot reach orchestrator: {e}")
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(anyhow::anyhow!("Orchestrator returned {status}"));
    }

    let list: ListResponse = resp.json().await?;
    if list.sessions.is_empty() {
        ctx.info("No agent sessions found.");
        return Ok(());
    }

    let headers = ["Session ID", "Status", "Provider", "Model", "Created"];
    let rows: Vec<Vec<String>> = list.sessions.iter().map(|s| vec![
        s.id[..8].to_string() + "…",
        crate::output::status_colored(&s.status),
        s.provider.clone().unwrap_or_else(|| "-".into()),
        s.model.clone().unwrap_or_else(|| "-".into()),
        s.created_at.as_deref()
            .map(|t| t.get(..19).unwrap_or(t))
            .unwrap_or("-")
            .to_string(),
    ]).collect();

    ctx.print_table(&headers, rows);
    Ok(())
}

async fn agent_status(args: StatusArgs, ctx: OutputContext) -> Result<()> {
    let client = build_client(30)?;
    let url = format!("{}/api/agents/{}", ctx.orchestrator_url, args.session_id);

    let req = client.get(&url);

    let resp = req.send().await.map_err(|e| anyhow::anyhow!("Cannot reach orchestrator: {e}"))?;

    if resp.status().as_u16() == 404 {
        ctx.error(&format!("Session '{}' not found", args.session_id));
        return Ok(());
    }

    if !resp.status().is_success() {
        return Err(anyhow::anyhow!("Orchestrator returned {}", resp.status()));
    }

    let session: AgentSession = resp.json().await?;
    print_session(&ctx, &session);
    Ok(())
}

async fn agent_stop(args: StopArgs, ctx: OutputContext) -> Result<()> {
    let client = build_client(30)?;
    let url = format!("{}/api/agents/{}/stop", ctx.orchestrator_url, args.session_id);

    let req = client.post(&url);

    let resp = req.send().await.map_err(|e| anyhow::anyhow!("Cannot reach orchestrator: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("Stop failed ({status}): {body}"));
    }

    ctx.success(&format!("Session '{}' stopped", args.session_id));
    Ok(())
}

async fn agent_logs(args: LogsArgs, ctx: OutputContext) -> Result<()> {
    let client = build_client(30)?;
    let url = format!(
        "{}/api/agents/{}/logs?lines={}",
        ctx.orchestrator_url, args.session_id, args.lines
    );

    let req = client.get(&url);

    let resp = req.send().await.map_err(|e| anyhow::anyhow!("Cannot reach orchestrator: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(anyhow::anyhow!("Logs request failed ({status})"));
    }

    let text = resp.text().await?;
    println!("{text}");
    Ok(())
}

fn print_session(ctx: &OutputContext, s: &AgentSession) {
    ctx.print_kv(&[
        ("session_id", &s.id),
        ("status",     &s.status),
        ("task",       s.task.as_deref().unwrap_or("-")),
        ("provider",   s.provider.as_deref().unwrap_or("-")),
        ("model",      s.model.as_deref().unwrap_or("-")),
        ("created_at", s.created_at.as_deref().unwrap_or("-")),
        ("updated_at", s.updated_at.as_deref().unwrap_or("-")),
    ]);
}

fn build_client(timeout_secs: u64) -> Result<Client> {
    Ok(Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()?)
}
