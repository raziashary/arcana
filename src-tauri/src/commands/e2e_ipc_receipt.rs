//! Explicitly isolated renderer-to-native IPC receipt used by the P4-60 desktop
//! harness. This module is absent from normal builds.

use crate::errors::{ArcanaError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::path::PathBuf;

const ENABLED_ENV: &str = "ARCANA_E2E_IPC_RECEIPT";
const FIXTURE_ROOT_ENV: &str = "ARCANA_E2E_FIXTURE_ROOT";
const WORKSPACE_ENV: &str = "ARCANA_E2E_WORKSPACE";
const FIXTURE_MARKER: &str = ".arcana-e2e-ipc-receipt";
const RUN_ID: &str = "p4-60-ipc-receipt";
const OPERATION: &str = "p4_60_ipc_receipt_probe";
const PRIVATE_CANARY: &str = "P4_60_PRIVATE_DIFF_CANARY";
const MAX_RECEIPT_BYTES: usize = 32 * 1024;
const MAX_EXPORT_BYTES: u64 = 2 * 1024 * 1024;
const REQUIRED_PROFILE_DIRS: &[&str] = &[
    "APPDATA",
    "LOCALAPPDATA",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "WEBVIEW2_USER_DATA_FOLDER",
];

#[derive(Debug, Serialize)]
pub struct ReceiptFixture {
    pub workspace: String,
    pub date: String,
    pub run_id: String,
    pub export_path: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RendererClaims {
    renderer_ready: bool,
    workspace_bound: bool,
    consent_ui_granted: bool,
    consent_ui_revoked: bool,
    exact_provider_grant: bool,
    cross_destination_denied: bool,
    exact_provider_revoked: bool,
    missing_and_revoked_diff_denied: bool,
    readable_logs_visible: bool,
    structured_event_visible: bool,
    trace_visible: bool,
    metrics_visible: bool,
    export_ipc_returned: bool,
}

fn enabled() -> Result<()> {
    if std::env::var(ENABLED_ENV).as_deref() != Ok("1") {
        return Err(ArcanaError::Agent(
            "isolated IPC receipt harness is not enabled".into(),
        ));
    }
    Ok(())
}

/// Reject a feature build that is not launched inside the disposable receipt
/// fixture. main calls this before the bridge, logging, runtime, or database
/// startup paths can run.
pub fn validate_startup_fixture() -> Result<()> {
    enabled()?;
    let root = std::env::var_os(FIXTURE_ROOT_ENV)
        .map(PathBuf::from)
        .ok_or_else(|| ArcanaError::Agent("isolated receipt fixture root is missing".into()))?
        .canonicalize()?;
    if std::fs::read_to_string(root.join(FIXTURE_MARKER))? != "arcana-p4-60-ipc-receipt-v1\n" {
        return Err(ArcanaError::Agent(
            "isolated receipt fixture marker is invalid".into(),
        ));
    }

    let workspace = std::env::var_os(WORKSPACE_ENV)
        .map(PathBuf::from)
        .ok_or_else(|| ArcanaError::Agent("isolated receipt workspace is missing".into()))?
        .canonicalize()?;
    if workspace.parent() != Some(root.as_path()) {
        return Err(ArcanaError::Agent(
            "receipt workspace is outside its fixture root".into(),
        ));
    }

    let profile = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .ok_or_else(|| ArcanaError::Agent("isolated receipt profile is missing".into()))?
        .canonicalize()?;
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| ArcanaError::Agent("isolated receipt home is missing".into()))?
        .canonicalize()?;
    if profile != home || profile.parent() != Some(root.as_path()) {
        return Err(ArcanaError::Agent(
            "receipt profile is outside its fixture root".into(),
        ));
    }

    for variable in REQUIRED_PROFILE_DIRS {
        let path = std::env::var_os(variable)
            .map(PathBuf::from)
            .ok_or_else(|| ArcanaError::Agent(format!("isolated {variable} is missing")))?
            .canonicalize()?;
        if !path.starts_with(&profile) || path == profile {
            return Err(ArcanaError::Agent(format!(
                "isolated {variable} is outside the receipt profile"
            )));
        }
    }

    let credential_name = |name: &std::ffi::OsStr| {
        let name = name.to_string_lossy().to_ascii_uppercase();
        [
            "API_KEY",
            "ACCESS_TOKEN",
            "TOKEN",
            "SECRET",
            "DATABASE_URL",
            "GEMINI",
            "GOOGLE",
            "OPENAI",
            "ANTHROPIC",
            "VAULT",
        ]
        .iter()
        .any(|pattern| name.contains(pattern))
    };
    if std::env::vars_os().any(|(name, _)| credential_name(&name)) {
        return Err(ArcanaError::Agent(
            "credential-shaped environment variable is present".into(),
        ));
    }
    Ok(())
}

fn fixture_paths() -> Result<(PathBuf, PathBuf, PathBuf)> {
    enabled()?;
    let root = std::env::var_os(FIXTURE_ROOT_ENV)
        .map(PathBuf::from)
        .ok_or_else(|| ArcanaError::Agent("isolated receipt fixture root is missing".into()))?
        .canonicalize()?;
    let marker = std::fs::read_to_string(root.join(FIXTURE_MARKER))?;
    if marker != "arcana-p4-60-ipc-receipt-v1\n" {
        return Err(ArcanaError::Agent(
            "isolated receipt fixture marker is invalid".into(),
        ));
    }

    let workspace = std::env::var_os(WORKSPACE_ENV)
        .map(PathBuf::from)
        .ok_or_else(|| ArcanaError::Agent("isolated receipt workspace is missing".into()))?
        .canonicalize()?;
    if workspace.parent() != Some(root.as_path()) || workspace.file_name().is_none() {
        return Err(ArcanaError::Agent(
            "receipt workspace is outside its fixture root".into(),
        ));
    }

    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or_else(|| ArcanaError::Agent("isolated receipt profile is missing".into()))?
        .canonicalize()?;
    if !home.starts_with(&root) || home == root {
        return Err(ArcanaError::Agent(
            "receipt profile is outside its fixture root".into(),
        ));
    }
    let arcana_dir = arcana_core::db::arcana_dir()?;
    let export_path = arcana_dir.join("p4-60-ipc-export.json");
    let receipt_path = arcana_dir.join("p4-60-ipc-receipt.json");
    Ok((workspace, export_path, receipt_path))
}

/// Produce fixed, privacy-safe evidence through Arcana's real tracing producer
/// and return only paths rooted in the launcher's disposable profile.
#[tauri::command]
pub fn begin_e2e_ipc_receipt() -> Result<ReceiptFixture> {
    let (workspace, export_path, _) = fixture_paths()?;
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut timer = arcana_core::observability::timing::OperationTimer::start(OPERATION)
        .with_run_id(RUN_ID)
        .with_trace_id("p4-60-ipc-receipt-trace")
        .with_call_context("p4-60-ipc-receipt-call", "desktop-e2e")
        .with_result_count(1);
    tracing::info!("P4-60 IPC receipt readable producer");
    timer.succeed();

    Ok(ReceiptFixture {
        workspace: workspace.to_string_lossy().into_owned(),
        date,
        run_id: RUN_ID.into(),
        export_path: export_path.to_string_lossy().into_owned(),
    })
}

/// Accept a small claim set from the fixed renderer probe, independently
/// verify the real log export, and persist one bounded receipt in the isolated
/// profile. No caller-supplied path is accepted.
#[tauri::command]
pub fn finish_e2e_ipc_receipt(app: tauri::AppHandle, renderer_receipt: String) -> Result<()> {
    let (_, export_path, receipt_path) = fixture_paths()?;
    if renderer_receipt.len() > MAX_RECEIPT_BYTES {
        return Err(ArcanaError::Agent(
            "renderer receipt exceeds the size limit".into(),
        ));
    }
    let claims: RendererClaims = serde_json::from_str(&renderer_receipt)
        .map_err(|_| ArcanaError::Agent("renderer receipt shape is invalid".into()))?;
    let all_claims = [
        claims.renderer_ready,
        claims.workspace_bound,
        claims.consent_ui_granted,
        claims.consent_ui_revoked,
        claims.exact_provider_grant,
        claims.cross_destination_denied,
        claims.exact_provider_revoked,
        claims.missing_and_revoked_diff_denied,
        claims.readable_logs_visible,
        claims.structured_event_visible,
        claims.trace_visible,
        claims.metrics_visible,
        claims.export_ipc_returned,
    ];
    if all_claims.iter().any(|claim| !claim) || renderer_receipt.contains(PRIVATE_CANARY) {
        return Err(ArcanaError::Agent(
            "renderer receipt is incomplete or contains private fixture input".into(),
        ));
    }

    let export_size = std::fs::metadata(&export_path)?.len();
    if export_size == 0 || export_size > MAX_EXPORT_BYTES {
        return Err(ArcanaError::Agent(
            "observability export size is outside the receipt bound".into(),
        ));
    }
    let exported: Value = serde_json::from_slice(&std::fs::read(&export_path)?)
        .map_err(|_| ArcanaError::Agent("observability export is invalid".into()))?;
    let readable = exported
        .get("logs")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let structured = exported
        .get("structured_events_jsonl")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !readable.contains("P4-60 IPC receipt readable producer")
        || !structured.contains(OPERATION)
        || !structured.contains(RUN_ID)
        || readable.contains(PRIVATE_CANARY)
        || structured.contains(PRIVATE_CANARY)
    {
        return Err(ArcanaError::Agent(
            "exported producer evidence is incomplete or reflects private input".into(),
        ));
    }

    let export_event_count = structured.lines().count();
    let receipt = serde_json::json!({
        "schema": "arcana-p4-60-ipc-receipt-v1",
        "renderer": claims,
        "native_export_verified": true,
        "export_event_count": export_event_count,
        "export_bytes": export_size,
        "private_canary_absent": true,
    });
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|_| ArcanaError::Agent("receipt serialization failed".into()))?;
    if bytes.len() > MAX_RECEIPT_BYTES {
        return Err(ArcanaError::Agent(
            "verified receipt exceeds the size limit".into(),
        ));
    }
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(receipt_path)?;
    output.write_all(&bytes)?;
    output.sync_all()?;
    app.exit(0);
    Ok(())
}

/// Report only a fixed probe stage when the renderer cannot finish. This
/// feature-only command cannot accept page content or caller-selected paths.
#[tauri::command]
pub fn fail_e2e_ipc_receipt(app: tauri::AppHandle, stage: String, reason: String) -> Result<()> {
    const STAGES: &[&str] = &[
        "renderer_start",
        "settings_controls",
        "workspace_binding",
        "consent_flow",
        "logs_tab",
        "logs_date",
        "logs_readable",
        "logs_structured",
        "logs_trace_metrics",
        "export",
    ];
    if !STAGES.contains(&stage.as_str()) {
        return Err(ArcanaError::Agent("unknown receipt probe stage".into()));
    }
    const REASONS: &[&str] = &[
        "button_absent",
        "timeout",
        "logs_crashed",
        "settings_still_visible",
        "logs_panel_missing",
        "invoke_error",
        "unexpected",
    ];
    if !REASONS.contains(&reason.as_str()) {
        return Err(ArcanaError::Agent(
            "unknown receipt probe failure reason".into(),
        ));
    }
    let (_, _, receipt_path) = fixture_paths()?;
    let failure_path = receipt_path.with_file_name("p4-60-ipc-failure.json");
    let bytes = serde_json::to_vec(&serde_json::json!({ "stage": stage, "reason": reason }))
        .map_err(|_| ArcanaError::Agent("receipt failure serialization failed".into()))?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(failure_path)?
        .write_all(&bytes)?;
    app.exit(1);
    Ok(())
}
