use crate::errors::{ArcanaError, Result};
use arcana_core::egress_consent::{
    mcp_destination, provider_destination, WorkspaceEgressConsentDb,
};
use std::path::PathBuf;

fn scope(provider_id: &str) -> Result<(PathBuf, String)> {
    let workspace =
        arcana_core::platform::workspace::resolve_workspace_dir(None).map_err(|error| {
            ArcanaError::Agent(format!("workspace unavailable for consent: {error}"))
        })?;
    let destination = if let Some(connector_id) = provider_id.strip_prefix("mcp:") {
        mcp_destination(connector_id)
    } else {
        provider_destination(&provider_id)
    }
    .map_err(|error| ArcanaError::Agent(format!("cannot manage egress consent: {error}")))?;
    Ok((workspace, destination))
}

#[tauri::command]
pub fn get_workspace_egress_consent(provider_id: String) -> Result<bool> {
    let (workspace, destination) = scope(&provider_id)?;
    WorkspaceEgressConsentDb::open()?.is_granted(&workspace, &destination)
}

/// Grant is an explicit user action from workspace Settings.
#[tauri::command]
pub fn grant_workspace_egress_consent(provider_id: String) -> Result<()> {
    let (workspace, destination) = scope(&provider_id)?;
    WorkspaceEgressConsentDb::open()?.grant(&workspace, &destination)
}

#[tauri::command]
pub fn revoke_workspace_egress_consent(provider_id: String) -> Result<bool> {
    let (workspace, destination) = scope(&provider_id)?;
    WorkspaceEgressConsentDb::open()?.revoke(&workspace, &destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consent_commands_persist_and_revoke_the_selected_workspace_grant() {
        let home = tempfile::tempdir().expect("create isolated Arcana home");
        let workspace = home.path().join("project");
        std::fs::create_dir_all(&workspace).expect("create selected project workspace");
        let _home = crate::test_support::isolate_home(home.path());
        arcana_core::db::app_settings::set(
            arcana_core::db::app_settings::WORKSPACE_KEY,
            &workspace.to_string_lossy(),
        )
        .expect("persist selected workspace");

        assert!(!get_workspace_egress_consent("google".into()).unwrap());
        grant_workspace_egress_consent("google".into()).unwrap();
        assert!(get_workspace_egress_consent("google".into()).unwrap());
        assert!(revoke_workspace_egress_consent("google".into()).unwrap());
        assert!(!get_workspace_egress_consent("google".into()).unwrap());
        assert!(!revoke_workspace_egress_consent("google".into()).unwrap());
        assert!(grant_workspace_egress_consent("unknown".into()).is_err());
    }
}
