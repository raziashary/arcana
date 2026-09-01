//! S1-04 — a file containing `AWS_SECRET_ACCESS_KEY=…`, read by a tool,
//! appears **redacted** in the prompt and in memory.
//!
//! This is the phase's headline smoke, run end to end rather than in pieces: a
//! real secret is written to a real file, the real `read_file` tool reads it
//! through the real `ToolExecutor`, and the assertion is made against the
//! string that would be handed to the model.
//!
//! The distinction from the unit tests matters. `redaction.rs`'s tests prove
//! the redactor works on a string someone passed it. This proves the redactor
//! is actually *on the path* — that a secret read from disk cannot reach the
//! conversation, whatever any individual tool does, because every tool result
//! funnels through one place.

use std::sync::{Arc, Mutex};

use autodev_core::agent::permission::PermissionMode;
use autodev_core::agent::tool_executor::ToolExecutor;
use autodev_core::agent::tool_trait::AgentIdentity;

/// A realistic AWS secret access key: 40 characters.
const SECRET: &str = "wJalrXUtnFEMIK7MDENGbPxRfiCYEXAMPLEKEYZ1";

#[tokio::test]
async fn a_secret_read_from_disk_never_reaches_the_prompt() {
    let dir = std::env::temp_dir().join("autodev-s1-04");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");

    let env_file = dir.join(".env.example");
    std::fs::write(
        &env_file,
        format!(
            "# deployment configuration\n\
             AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE\n\
             AWS_SECRET_ACCESS_KEY={SECRET}\n\
             AWS_REGION=us-east-1\n\
             LOG_LEVEL=debug\n"
        ),
    )
    .expect("write the file the agent will read");

    let mut executor = ToolExecutor::new(
        // Autonomous: the point is the redaction path, not the prompt path.
        PermissionMode::Autonomous,
        Arc::new(Mutex::new(std::collections::HashMap::new())),
        "s1-04-session".to_string(),
        dir.to_string_lossy().to_string(),
        AgentIdentity::supervisor(),
    );

    let mut registry = autodev_core::agent::tool_registry::ToolRegistry::new();
    autodev_core::agent::builtin_tools::register_for_mode(
        &mut registry,
        autodev_core::agent::PAMode::FastDevelopment,
    );
    let read_file = registry
        .get("read_file")
        .expect("read_file must be registered");

    let result = executor
        .execute(
            read_file,
            serde_json::json!({ "path": env_file.to_string_lossy() }),
        )
        .await
        .expect("read_file should succeed");

    // `result.output` is what goes into the conversation the model sees.
    assert!(
        !result.output.contains(SECRET),
        "THE SECRET REACHED THE PROMPT:\n{}",
        result.output
    );
    assert!(
        !result.output.contains("AKIAIOSFODNN7EXAMPLE"),
        "the access key id reached the prompt:\n{}",
        result.output
    );

    // The file must still be useful to the agent, or redaction has broken the
    // capability it is protecting.
    assert!(
        result.output.contains("us-east-1") && result.output.contains("LOG_LEVEL"),
        "redaction destroyed the non-secret configuration the agent needs:\n{}",
        result.output
    );

    // And the control: the tool really did read the file. Without this, a tool
    // that returned an empty string would pass every assertion above.
    assert!(
        result.output.contains("deployment configuration"),
        "read_file returned nothing useful, so this test proves nothing:\n{}",
        result.output
    );

    let _ = std::fs::remove_dir_all(&dir);
}
