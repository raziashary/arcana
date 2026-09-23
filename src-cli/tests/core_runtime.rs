// INV-1: one agent runtime, linked by two binaries.
//
// This is the test the invariant was missing. Before P1 it could not have been
// written: `AgentEngine::new` required a `tauri::AppHandle`, so the runtime was
// constructible only from inside a running desktop window, and `arcanactl` had no
// way to reach it. That is why the CLI grew its own parallel implementations in
// `src-shared` — and why `src-shared/src/db/workers.rs` drifted from the schema
// `src-tauri` actually writes, breaking every `arcanactl worker` call.
//
// What this proves: the agent runtime constructs, registers its tools and runs
// its permission machinery in a process with no UI framework linked at all.
// What it does not prove: that a turn completes — that needs a live provider
// and belongs to the phase that ships CLI agent runs.
//
// See ADR-LOG-RESTRUCT-0011 and ADR-LOG-RESTRUCT-0015.

use std::sync::{Arc, Mutex};

use arcana_core::agent::builtin_tools::register_for_mode;
use arcana_core::agent::permission::PermissionMode;
use arcana_core::agent::tool_registry::ToolRegistry;
use arcana_core::agent::tool_trait::AgentIdentity;
use arcana_core::agent::{AgentEngine, PAMode};
use arcana_core::events::{emit, install_sink, NullEventSink};
use arcana_core::provider::{
    CompletionRequest, CompletionResponse, ModelInfo, Provider, ProviderStatus,
};
use arcana_core::state::app_state::AppState;

/// A provider that is never called.
///
/// The point is constructing the engine, not driving it: a real provider would
/// need a key and a network, and neither is what INV-1 is about.
struct StubProvider;

#[async_trait::async_trait]
impl Provider for StubProvider {
    fn id(&self) -> &str {
        "stub"
    }

    fn name(&self) -> &str {
        "Stub Provider"
    }

    fn models(&self) -> Vec<ModelInfo> {
        Vec::new()
    }

    async fn complete(
        &self,
        _request: CompletionRequest,
    ) -> arcana_core::errors::Result<CompletionResponse> {
        unreachable!("the runtime-construction test never issues a completion")
    }

    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> arcana_core::errors::Result<arcana_core::provider::traits::StreamResult> {
        unreachable!("the runtime-construction test never opens a stream")
    }

    async fn test_connection(&self) -> arcana_core::errors::Result<ProviderStatus> {
        unreachable!("the runtime-construction test never tests a connection")
    }
}

#[test]
fn the_agent_runtime_constructs_in_a_process_with_no_ui() {
    let _engine = AgentEngine::new(
        Arc::new(StubProvider),
        PermissionMode::Autonomous,
        PAMode::FastDevelopment,
        Arc::new(Mutex::new(AppState::default())),
        Arc::new(Mutex::new(std::collections::HashMap::new())),
        AgentIdentity::supervisor(),
    );

    // Constructing it is the proof: `AgentEngine::new` returns `Self`, so
    // reaching this line means the CLI linked and instantiated the runtime with
    // no UI framework in the process. Before P1 this file could not compile.
    //
    // Tool registration is asserted separately below, because an engine that
    // constructs but registers nothing would be a runtime in name only, and
    // `new` exposes no accessor to check it through.
}

#[test]
fn the_cli_can_build_the_same_builtin_tool_set_the_desktop_app_runs() {
    let mut registry = ToolRegistry::new();
    register_for_mode(&mut registry, PAMode::FastDevelopment);

    let tools = registry.all();
    assert!(
        !tools.is_empty(),
        "no builtin tools registered — linking core bought the CLI nothing"
    );

    // The file tools are the ones that make a coding agent a coding agent; if
    // the set silently narrowed to nothing useful, an empty-check would still
    // pass.
    for expected in ["read_file", "write_file", "bash"] {
        assert!(
            registry.get(expected).is_some(),
            "builtin tool '{expected}' is missing from the CLI's registry"
        );
    }
}

#[test]
fn emitting_without_a_host_sink_is_a_no_op_not_a_panic() {
    // The headless default. `arcanactl` installs this explicitly in `main`, but
    // the property that matters is that emitting is safe even before any host
    // has installed anything — a core call site must never have to ask whether
    // a UI exists.
    emit("agent:token", serde_json::json!({ "text": "hello" }));

    assert!(
        install_sink(Arc::new(NullEventSink)),
        "no sink should have been installed before this test ran"
    );

    emit("agent:token", serde_json::json!({ "text": "still fine" }));
}
