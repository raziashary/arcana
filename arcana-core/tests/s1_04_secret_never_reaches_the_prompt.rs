//! S1-04 — a file containing `AWS_SECRET_ACCESS_KEY=…`, read by a tool,
//! appears **redacted** in the prompt and in memory.
//!
//! ## Why this file was rewritten
//!
//! The first version of this test called `ToolExecutor::execute`. That method
//! has **no production caller**. The agent loop calls `execute_batch`
//! (`agent/engine.rs:314`), which at the time carried raw tool input and output
//! to the model, the `messages` table and the UI event — so the redaction was
//! live in dead code, this test drove the dead method, and it passed. The gate
//! was a substring check over the file, so it was green too.
//!
//! Three independent post-phase auditors found it. The lesson is recorded as
//! `L-C12` (a thing built, proven, and wired to nothing) and `L-C9` (a
//! verification whose shape cannot express the failure).
//!
//! So this version drives **`execute_batch`** — the method the agent loop
//! actually calls — and covers **both** of its branches, because they are
//! separate code paths that reach the conversation independently:
//!
//!   * read-only tools run concurrently (`read_file` → this is the S1-04 case)
//!   * write tools run serially through the permission path (`write_file`)
//!
//! A test that covered only one branch would have passed against a fix applied
//! to only one branch.

use std::sync::{Arc, Mutex};

use arcana_core::agent::permission::PermissionMode;
use arcana_core::agent::tool_executor::ToolExecutor;
use arcana_core::agent::tool_trait::AgentIdentity;

/// A realistic AWS secret access key: 40 characters.
const SECRET: &str = "wJalrXUtnFEMIK7MDENGbPxRfiCYEXAMPLEKEYZ1";

fn sandbox(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("arcana-s1-04-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir
}

fn executor(dir: &std::path::Path) -> ToolExecutor {
    ToolExecutor::new(
        // Autonomous: the point is the redaction path, not the prompt path.
        PermissionMode::Autonomous,
        Arc::new(Mutex::new(std::collections::HashMap::new())),
        "s1-04-session".to_string(),
        dir.to_string_lossy().to_string(),
        AgentIdentity::supervisor(),
    )
}

fn registry() -> arcana_core::agent::tool_registry::ToolRegistry {
    let mut r = arcana_core::agent::tool_registry::ToolRegistry::new();
    arcana_core::agent::builtin_tools::register_for_mode(
        &mut r,
        arcana_core::agent::PAMode::FastDevelopment,
    );
    r
}

const ENV_FILE: &str = "# deployment configuration\n\
                        AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE\n\
                        AWS_SECRET_ACCESS_KEY=SECRET_PLACEHOLDER\n\
                        AWS_REGION=us-east-1\n\
                        LOG_LEVEL=debug\n";

/// The headline case, on the path the agent loop actually takes.
///
/// `read_file` is read-only, so `execute_batch` puts it on the **concurrent**
/// branch — a different code path from the serial write branch below.
#[tokio::test]
async fn a_secret_read_from_disk_never_reaches_the_prompt() {
    let dir = sandbox("read");
    let env_file = dir.join(".env.example");
    std::fs::write(&env_file, ENV_FILE.replace("SECRET_PLACEHOLDER", SECRET))
        .expect("write the file the agent will read");

    let mut executor = executor(&dir);
    let read_file = registry()
        .get("read_file")
        .expect("read_file must be registered");

    // execute_batch — the method `agent/engine.rs:314` calls.
    let results = executor
        .execute_batch(vec![(
            read_file,
            serde_json::json!({ "path": env_file.to_string_lossy() }),
            "call-1".to_string(),
        )])
        .await
        .expect("read_file should succeed");

    assert_eq!(results.len(), 1, "one tool in, one result out");
    let output = &results[0].1.output;

    assert!(
        !output.contains(SECRET),
        "THE SECRET REACHED THE PROMPT via execute_batch:\n{output}"
    );
    assert!(
        !output.contains("AKIAIOSFODNN7EXAMPLE"),
        "the access key id reached the prompt:\n{output}"
    );

    // The file must still be useful to the agent, or redaction has broken the
    // capability it protects.
    assert!(
        output.contains("us-east-1") && output.contains("LOG_LEVEL"),
        "redaction destroyed the non-secret configuration the agent needs:\n{output}"
    );

    // The control. Without it, a tool that returned an empty string — or a
    // batch that silently dropped the call — would satisfy every assertion
    // above. This is exactly how the previous version of this test passed
    // while proving nothing.
    assert!(
        output.contains("deployment configuration"),
        "read_file returned nothing useful, so this test proves nothing:\n{output}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The other branch. Write tools take the serial permission path, which reaches
/// the conversation by its own route, so it needs its own proof.
#[tokio::test]
async fn a_secret_echoed_by_a_write_tool_never_reaches_the_prompt() {
    let dir = sandbox("write");
    let target = dir.join("out.env");

    let mut executor = executor(&dir);
    let write_file = registry()
        .get("write_file")
        .expect("write_file must be registered");

    let results = executor
        .execute_batch(vec![(
            write_file,
            serde_json::json!({
                "path": target.to_string_lossy(),
                "content": ENV_FILE.replace("SECRET_PLACEHOLDER", SECRET),
            }),
            "call-1".to_string(),
        )])
        .await
        .expect("write_file should succeed");

    let output = &results[0].1.output;
    assert!(
        !output.contains(SECRET),
        "THE SECRET REACHED THE PROMPT via the write branch:\n{output}"
    );

    // The write itself must have happened — otherwise the branch was never
    // exercised and the assertion above is vacuous.
    assert!(
        target.exists(),
        "write_file did not write, so this proves nothing"
    );
    let on_disk = std::fs::read_to_string(&target).expect("read back");
    assert!(
        on_disk.contains(SECRET),
        "the tool must receive the RAW input — redacting what is written to \
         disk would corrupt the user's file, not protect it"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A batch mixing both kinds, which is the ordinary case in a real turn.
#[tokio::test]
async fn a_mixed_batch_redacts_every_result() {
    let dir = sandbox("mixed");
    let env_file = dir.join(".env.example");
    std::fs::write(&env_file, ENV_FILE.replace("SECRET_PLACEHOLDER", SECRET)).expect("write");

    let mut executor = executor(&dir);
    let reg = registry();

    let results = executor
        .execute_batch(vec![
            (
                reg.get("read_file").expect("read_file"),
                serde_json::json!({ "path": env_file.to_string_lossy() }),
                "call-read".to_string(),
            ),
            (
                reg.get("write_file").expect("write_file"),
                serde_json::json!({
                    "path": dir.join("copy.env").to_string_lossy(),
                    "content": ENV_FILE.replace("SECRET_PLACEHOLDER", SECRET),
                }),
                "call-write".to_string(),
            ),
        ])
        .await
        .expect("batch should succeed");

    assert_eq!(results.len(), 2, "both results must come back");
    for (id, result) in &results {
        assert!(
            !result.output.contains(SECRET),
            "the secret survived in result {id}:\n{}",
            result.output
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}
