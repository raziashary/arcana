// Diff analysis commands — Feature 3 (Ruflo).
// Exposed to the frontend and callable by the QC Agent via MCP tool.

use std::sync::{Arc, Mutex};
use tauri::State;

use crate::diff_analysis::{DiffAnalysisResult, DiffAnalyzer};
use crate::errors::{ArcanaError, Result};
use crate::provider::traits::Provider;
use crate::state::AppState;
use crate::vault::keychain::{provider_key, VaultManager};
use arcana_core::egress_consent::{provider_destination, WorkspaceEgressConsentDb};
use arcana_core::state::lock::LockTolerant;

fn build_analyzer(
    state: &Arc<Mutex<AppState>>,
    workspace: std::path::PathBuf,
) -> Result<DiffAnalyzer> {
    let (provider_id, model) = {
        let s = state.lock_tolerant();
        let p = s
            .active_provider
            .clone()
            .ok_or_else(|| ArcanaError::Agent("No active provider set".into()))?;
        let m = s
            .active_model
            .clone()
            .ok_or_else(|| ArcanaError::Agent("No active model set".into()))?;
        (p, m)
    };

    // Every supported provider takes a key; the keyless branch went with
    // Ollama (P2-11).
    let api_key = VaultManager::get(&provider_key(&provider_id))?
        .ok_or_else(|| ArcanaError::Agent(format!("No API key for provider '{provider_id}'")))?;

    let inner: Arc<dyn Provider> = arcana_core::provider::factory::build_inner(&provider_id, api_key)
        .map_err(|error| ArcanaError::Agent(error.to_string()))?;
    // Every call passes the daily ceiling (P2-21).
    let capped = arcana_core::provider::capped::CappedProvider::wrap(inner, state.clone());
    let provider = Arc::new(arcana_core::egress_consent::ConsentProvider::wrap(
        capped, workspace,
    ));

    Ok(DiffAnalyzer::new(provider, model))
}

fn configured_workspace() -> Result<std::path::PathBuf> {
    arcana_core::platform::workspace::resolve_workspace_dir(None).map_err(|error| {
        ArcanaError::Agent(format!("workspace unavailable for diff analysis: {error}"))
    })
}

async fn analyze_with_workspace_consent(
    diff_text: &str,
    workspace: std::path::PathBuf,
    state: &Arc<Mutex<AppState>>,
) -> Result<DiffAnalysisResult> {
    analyze_with_workspace_consent_using(
        diff_text,
        workspace,
        state,
        |diff_text, workspace, state| async move {
            build_analyzer(&state, workspace)?.analyze(&diff_text).await
        },
    )
    .await
}

async fn analyze_with_workspace_consent_using<F, Fut>(
    diff_text: &str,
    workspace: std::path::PathBuf,
    state: &Arc<Mutex<AppState>>,
    analyze: F,
) -> Result<DiffAnalysisResult>
where
    F: FnOnce(String, std::path::PathBuf, Arc<Mutex<AppState>>) -> Fut,
    Fut: std::future::Future<Output = Result<DiffAnalysisResult>>,
{
    let provider_id = state.lock_tolerant().active_provider.clone();
    let Some(provider_id) = provider_id else {
        return DiffAnalyzer::analyze_structural(diff_text);
    };
    let destination = provider_destination(&provider_id)?;
    let allowed = WorkspaceEgressConsentDb::open()?.authorize_and_audit(
        &workspace,
        &destination,
        "tauri_diff_analysis",
    )?;
    if !allowed {
        return DiffAnalyzer::analyze_structural(diff_text);
    }

    // The provider wrapper checks the persisted consent again immediately
    // before its outbound request, so revocation between this decision and the
    // call still fails closed.
    match analyze(diff_text.to_owned(), workspace, state.clone()).await {
        Err(ArcanaError::EgressDenied { .. }) => DiffAnalyzer::analyze_structural(diff_text),
        result => result,
    }
}

/// Analyze a raw unified diff string.
#[tauri::command]
pub async fn analyze_diff(
    diff_text: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<DiffAnalysisResult> {
    let workspace = configured_workspace()?;
    analyze_with_workspace_consent(&diff_text, workspace, &state).await
}

/// Generate and analyze the diff between two branches in a git repository.
#[tauri::command]
pub async fn analyze_pr_diff(
    repo_path: String,
    base_branch: String,
    head_branch: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<DiffAnalysisResult> {
    let workspace = configured_workspace()?;
    analyze_pr_diff_with_workspace_consent(
        &repo_path,
        &base_branch,
        &head_branch,
        workspace,
        &state,
        crate::git::diff::branch_diff,
        |diff_text, workspace, state| async move {
            build_analyzer(&state, workspace)?.analyze(&diff_text).await
        },
    )
    .await
}

async fn analyze_pr_diff_with_workspace_consent<F, Analyze, Fut>(
    repo_path: &str,
    base_branch: &str,
    head_branch: &str,
    workspace: std::path::PathBuf,
    state: &Arc<Mutex<AppState>>,
    branch_diff: F,
    analyze: Analyze,
) -> Result<DiffAnalysisResult>
where
    F: FnOnce(&str, &str, &str) -> Result<String>,
    Analyze: FnOnce(String, std::path::PathBuf, Arc<Mutex<AppState>>) -> Fut,
    Fut: std::future::Future<Output = Result<DiffAnalysisResult>>,
{
    let repo = std::path::PathBuf::from(repo_path)
        .canonicalize()
        .map_err(|e| ArcanaError::Agent(format!("invalid diff repository path: {e}")))?;
    if !repo.starts_with(&workspace) {
        return Err(ArcanaError::Agent(
            "diff repository must be inside the configured workspace".into(),
        ));
    }
    let patch = branch_diff(&repo.to_string_lossy(), base_branch, head_branch)?;
    analyze_with_workspace_consent_using(&patch, workspace, state, analyze).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn direct_diff_command_uses_structural_result_without_matching_consent_or_provider_key() {
        let home = tempfile::tempdir().expect("create isolated Arcana home");
        let workspace = home.path().join("project");
        std::fs::create_dir_all(&workspace).expect("create project workspace");
        let _home = crate::test_support::isolate_home(home.path());
        let consent = WorkspaceEgressConsentDb::open().expect("open isolated consent store");
        consent
            .grant(&workspace, "provider:openai")
            .expect("grant a different destination");

        let state = Arc::new(Mutex::new(AppState {
            active_provider: Some("google".into()),
            active_model: Some("gemini-3.7-flash".into()),
            ..AppState::default()
        }));
        let patch = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1,2 @@\n-old\n+new\n+added\n";
        let result = analyze_with_workspace_consent(patch, workspace, &state)
            .await
            .expect("mismatched consent must produce a local structural result");

        assert_eq!(result.analysis_mode, "deterministic_only");
        assert_eq!(result.file_count, 1);
        assert!(result.summary.contains("were not sent"));
    }

    #[tokio::test]
    async fn pr_diff_command_path_refuses_unconsented_repository_content_without_key_lookup() {
        let home = tempfile::tempdir().expect("create isolated Arcana home");
        let workspace_path = home.path().join("workspace");
        let repo_path = workspace_path.join("repo");
        std::fs::create_dir_all(&repo_path).expect("create workspace");
        let workspace = workspace_path
            .canonicalize()
            .expect("canonicalize workspace");
        let _home = crate::test_support::isolate_home(home.path());
        let repo = repo_path.canonicalize().expect("canonicalize test repo");

        let state = Arc::new(Mutex::new(AppState {
            active_provider: Some("google".into()),
            active_model: Some("gemini-3.8-flash".into()),
            ..AppState::default()
        }));
        let result = analyze_pr_diff_with_workspace_consent(
            repo_path.to_str().expect("repo path is UTF-8"),
            "refs/heads/base",
            "refs/heads/head",
            workspace.clone(),
            &state,
            |actual_repo, base, head| {
                assert_eq!(actual_repo, repo.to_string_lossy());
                assert_eq!(base, "refs/heads/base");
                assert_eq!(head, "refs/heads/head");
                Ok("diff --git a/src.txt b/src.txt\n--- a/src.txt\n+++ b/src.txt\n@@ -1 +1,2 @@\n before\n+private addition\n".into())
            },
            |_, _, _| async { panic!("unconsented PR diff reached the analyzer") },
        )
        .await
        .expect("absent consent should return deterministic review before vault lookup");

        assert_eq!(result.analysis_mode, "deterministic_only");
        assert!(result.summary.contains("were not sent"));
        let audit = arcana_core::db::config_db::open().expect("open isolated consent store");
        let row: (String, String, String, String) = audit
            .query_row(
                "SELECT workspace_path, destination, operation, outcome
                 FROM workspace_egress_audit ORDER BY id DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read denied PR diff audit");
        assert_eq!(row.0, workspace.to_string_lossy());
        assert_eq!(row.1, "provider:google");
        assert_eq!(row.2, "tauri_diff_analysis");
        assert_eq!(row.3, "denied");
    }

    #[tokio::test]
    async fn pr_diff_command_with_matching_consent_reaches_the_analyzer_without_live_provider() {
        let home = tempfile::tempdir().expect("create isolated Arcana home");
        let workspace_path = home.path().join("workspace");
        let repo_path = workspace_path.join("repo");
        std::fs::create_dir_all(&repo_path).expect("create workspace");
        let workspace = workspace_path
            .canonicalize()
            .expect("canonicalize workspace");
        let _home = crate::test_support::isolate_home(home.path());
        let repo = repo_path.canonicalize().expect("canonicalize test repo");
        WorkspaceEgressConsentDb::open()
            .expect("open isolated consent database")
            .grant(&workspace, "provider:google")
            .expect("grant the matching destination");

        let state = Arc::new(Mutex::new(AppState {
            active_provider: Some("google".into()),
            active_model: Some("gemini-3.8-flash".into()),
            ..AppState::default()
        }));
        let analyzer_called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_from_analyzer = Arc::clone(&analyzer_called);
        let expected_workspace = workspace.clone();
        let result = analyze_pr_diff_with_workspace_consent(
            repo_path.to_str().expect("repo path is UTF-8"),
            "refs/heads/base",
            "refs/heads/head",
            workspace.clone(),
            &state,
            |actual_repo, base, head| {
                assert_eq!(actual_repo, repo.to_string_lossy());
                assert_eq!(base, "refs/heads/base");
                assert_eq!(head, "refs/heads/head");
                Ok("diff --git a/src.txt b/src.txt\n+private addition\n".into())
            },
            move |diff_text, actual_workspace, _state| {
                let called = Arc::clone(&called_from_analyzer);
                async move {
                    called.store(true, std::sync::atomic::Ordering::SeqCst);
                    assert_eq!(actual_workspace, expected_workspace);
                    assert!(diff_text.contains("private addition"));
                    DiffAnalyzer::analyze_structural(&diff_text)
                }
            },
        )
        .await
        .expect("matching consent should pass the real gate to the analyzer seam");

        assert!(analyzer_called.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(result.analysis_mode, "deterministic_only");
        let audit = arcana_core::db::config_db::open().expect("open isolated consent store");
        let row: (String, String, String, String) = audit
            .query_row(
                "SELECT workspace_path, destination, operation, outcome
                 FROM workspace_egress_audit ORDER BY id DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read allowed PR diff audit");
        assert_eq!(row.0, workspace.to_string_lossy());
        assert_eq!(row.1, "provider:google");
        assert_eq!(row.2, "tauri_diff_analysis");
        assert_eq!(row.3, "allowed");
    }

    #[tokio::test]
    async fn revocation_between_diff_preflight_and_provider_request_returns_structural_review() {
        let home = tempfile::tempdir().expect("create isolated Arcana home");
        let workspace_path = home.path().join("workspace");
        std::fs::create_dir_all(&workspace_path).expect("create workspace");
        let workspace = workspace_path
            .canonicalize()
            .expect("canonicalize workspace");
        let _home = crate::test_support::isolate_home(home.path());
        WorkspaceEgressConsentDb::open()
            .expect("open consent database")
            .grant(&workspace, "provider:google")
            .expect("grant provider");
        let state = Arc::new(Mutex::new(AppState {
            active_provider: Some("google".into()),
            active_model: Some("gemini-3.8-flash".into()),
            ..AppState::default()
        }));
        let patch = "diff --git a/src.txt b/src.txt\n--- a/src.txt\n+++ b/src.txt\n@@ -1 +1,2 @@\n-old\n+new\n";
        let revoke_workspace = workspace.clone();
        let result = analyze_with_workspace_consent_using(
            patch,
            workspace.clone(),
            &state,
            move |_, _, _| async move {
                WorkspaceEgressConsentDb::open()
                    .expect("open consent database")
                    .revoke(&revoke_workspace, "provider:google")
                    .expect("revoke provider");
                Err(ArcanaError::EgressDenied {
                    destination: "provider:google".into(),
                })
            },
        )
        .await
        .expect("revoked provider should yield local review");
        assert_eq!(result.analysis_mode, "deterministic_only");
        assert!(result.summary.contains("were not sent"));
        assert!(!WorkspaceEgressConsentDb::open()
            .unwrap()
            .is_granted(&workspace, "provider:google")
            .unwrap());
    }
}
