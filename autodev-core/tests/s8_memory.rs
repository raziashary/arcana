// `S8-04` — memory recall works end to end with no memory server on disk.
//
// ⚠ **One test, in its own integration binary, on purpose.** It repoints
// `USERPROFILE` / `HOME` at a sandbox so `db::autodev_dir()` resolves there
// instead of at the developer's real `~/.autodev` (`§0.8`). Environment
// variables are process-wide, and Rust runs the tests inside one file on
// parallel threads — so a second test in this file could observe a half-set
// environment, or set it back while this one is mid-write. `L-30` is that
// mistake: three P4 tests shared one temp directory and passed alone.
//
// Rust gives each `tests/*.rs` its own binary, which is what makes a
// single-test file the safe shape for a test that must mutate the environment.

use autodev_core::memory::db::MemoryDb;
use autodev_core::memory::gateway::{self, Proposal, Proposer};
use autodev_core::memory::recall;
use autodev_core::memory::types::MemoryTier;

#[test]
fn memory_survives_a_write_and_a_recall_with_nothing_running() {
    let sandbox = std::env::temp_dir().join(format!(
        "autodev-s8-04-{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&sandbox).expect("sandbox");

    // Repoint the home directory BEFORE anything opens a database.
    std::env::set_var("USERPROFILE", &sandbox);
    std::env::set_var("HOME", &sandbox);

    // Prove the redirection took, rather than assuming it. Without this the
    // test could write into the developer's real store and still pass, which is
    // the failure that matters most here.
    let dir = autodev_core::db::autodev_dir().expect("autodev_dir");
    assert!(
        dir.starts_with(&sandbox),
        "the data directory did not follow the sandbox: {dir:?} is not under \
         {sandbox:?}. This test would have written to the real store."
    );

    let repo = sandbox.to_string_lossy().to_string();
    let session = format!("s8-04-{}", std::process::id());

    // ── write ────────────────────────────────────────────────────────────────
    //
    // Through the gateway, which is the only write seam an agent has (`P3-39`).
    let db = MemoryDb::open().expect("the memory store must open with nothing running");
    let verdict = gateway::propose(
        &db.conn,
        &Proposal {
            content: "the smoke build for this project is `cargo test --workspace`".into(),
            tier: MemoryTier::Semantic,
            scope: "project".into(),
            scope_ref: repo.clone(),
            proposer: Proposer::Assistant,
            claimed_confidence: 0.9,
            source_file: None,
            source_line: None,
        },
    )
    .expect("the gateway must answer");
    assert!(
        verdict.is_stored(),
        "the gateway refused a well-formed proposal: {verdict:?}"
    );

    // ── recall ───────────────────────────────────────────────────────────────
    let block = recall::build_memory_context(&session, "how do I run the smoke build?", &repo)
        .expect(
            "nothing was recalled. Both stores are SQLite in this process — if \
             this is empty, memory is not working at all, not merely degraded.",
        );
    assert!(
        block.contains("cargo test --workspace"),
        "the memory was stored but did not come back for a query it plainly \
         answers:\n{block}"
    );

    let _ = std::fs::remove_dir_all(&sandbox);
}
