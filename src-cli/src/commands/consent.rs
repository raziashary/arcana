//! Explicit management for persisted workspace-bound egress consent.

use std::path::PathBuf;

use anyhow::{Context, Result};
use arcana_core::egress_consent::{
    mcp_destination, provider_destination, WorkspaceEgressConsentDb,
};
use clap::{Args, Subcommand};

use crate::output::OutputContext;

#[derive(Subcommand)]
pub enum ConsentCmd {
    /// Persist consent for Arcana-mediated requests from one workspace.
    Grant(ConsentArgs),
    /// Revoke previously persisted consent for one workspace.
    Revoke(ConsentArgs),
    /// Show whether a provider is authorized for one workspace.
    Status(ConsentArgs),
}

#[derive(Args)]
pub struct ConsentArgs {
    /// Workspace whose content may be sent through Arcana-mediated requests.
    #[arg(long)]
    workspace: PathBuf,

    /// Provider destination or configured MCP connector covered by the grant.
    #[arg(long, value_parser = ["google", "openai", "anthropic", "groq", "mcp:github-mcp"])]
    provider: String,
}

pub fn run(command: ConsentCmd, ctx: OutputContext) -> Result<()> {
    let args = match command {
        ConsentCmd::Grant(args) => {
            let (workspace, destination) = scope(&args)?;
            WorkspaceEgressConsentDb::open()
                .context("Could not open Arcana's workspace-consent store")?
                .grant(&workspace, &destination)
                .context("Could not persist workspace egress consent")?;
            ctx.success(&format!(
                "Granted {} egress consent for {}",
                args.provider,
                workspace.display()
            ));
            return Ok(());
        }
        ConsentCmd::Revoke(args) => {
            let (workspace, destination) = scope(&args)?;
            let revoked = WorkspaceEgressConsentDb::open()
                .context("Could not open Arcana's workspace-consent store")?
                .revoke(&workspace, &destination)
                .context("Could not revoke workspace egress consent")?;
            let message = if revoked {
                format!(
                    "Revoked {} egress consent for {}",
                    args.provider,
                    workspace.display()
                )
            } else {
                format!(
                    "No active {} egress consent existed for {}",
                    args.provider,
                    workspace.display()
                )
            };
            ctx.success(&message);
            return Ok(());
        }
        ConsentCmd::Status(args) => args,
    };

    let (workspace, destination) = scope(&args)?;
    let granted = WorkspaceEgressConsentDb::open()
        .context("Could not open Arcana's workspace-consent store")?
        .is_granted(&workspace, &destination)
        .context("Could not read workspace egress consent")?;
    if ctx.format == crate::output::OutputFormat::Json {
        ctx.print_json(&serde_json::json!({
            "workspace": workspace,
            "provider": args.provider,
            "granted": granted,
        }));
    } else {
        ctx.print_table(
            &["Workspace", "Provider", "Granted"],
            vec![vec![
                workspace.display().to_string(),
                args.provider,
                granted.to_string(),
            ]],
        );
    }
    Ok(())
}

fn scope(args: &ConsentArgs) -> Result<(PathBuf, String)> {
    let workspace = args
        .workspace
        .canonicalize()
        .with_context(|| format!("Workspace does not exist: {}", args.workspace.display()))?;
    let destination = if let Some(connector_id) = args.provider.strip_prefix("mcp:") {
        mcp_destination(connector_id)?
    } else {
        provider_destination(&args.provider)?
    };
    Ok((workspace, destination))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consent_scope_requires_an_existing_workspace_and_supported_provider() {
        let root = std::env::temp_dir();
        let args = ConsentArgs {
            workspace: root.clone(),
            provider: "google".into(),
        };
        let (workspace, destination) = scope(&args).expect("resolve consent scope");
        assert_eq!(workspace, root.canonicalize().unwrap());
        assert_eq!(destination, "provider:google");

        let unsupported = ConsentArgs {
            workspace: root,
            provider: "unknown".into(),
        };
        assert!(scope(&unsupported).is_err());
    }
}
