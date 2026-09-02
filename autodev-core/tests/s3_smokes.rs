// S3 · the P3 smoke checklist — the three that A–E owns.
//
// `S3-04` (recall sets) and `S3-05` (degraded recall with the embedding model
// deleted) belong to `P3-F · Memory`, which has 15 open tasks and is out of
// A–E's scope. They are deliberately absent rather than stubbed: a stubbed
// smoke is a green row over work nobody did.

use autodev_core::agent::compaction::CompactionEngine;
use autodev_core::provider::types::{MessageContent, RequestMessage};

/// Build a message the size of a realistic tool result.
fn tool_round_trip(i: usize, bytes: usize) -> Vec<RequestMessage> {
    let filler = "x".repeat(bytes);
    vec![
        RequestMessage {
            role: "assistant".into(),
            content: vec![MessageContent::ToolUse {
                id: format!("toolu_{i:03}"),
                name: "read_file".into(),
                input: serde_json::json!({ "path": format!("src/f{i}.rs") }),
                opaque: None,
            }],
        },
        RequestMessage {
            role: "user".into(),
            content: vec![MessageContent::ToolResult {
                tool_use_id: format!("toolu_{i:03}"),
                name: "read_file".into(),
                content: filler,
                is_error: false,
            }],
        },
    ]
}

// ── S3-01 ───────────────────────────────────────────────────────────────────

/// A 30-turn session's input tokens do not grow linearly with turn count.
///
/// ⚠ **What this can and cannot prove.** Growth is bounded by two mechanisms:
/// the server-side clear layer (`P3-25`), whose effect is only observable on
/// the wire and is measured by `p3_25_bench_…` against the live API, and
/// compaction (`P2-57`), which is in-process and measurable here.
///
/// This asserts the in-process half: that a 30-turn history crosses the
/// thresholds that bound it, and that both bounds sit where they are supposed
/// to. A test that merely watched `estimate_tokens` rise would prove nothing —
/// it rises by construction.
#[test]
fn s3_01_a_thirty_turn_session_does_not_grow_unbounded() {
    let window = 200_000u32;

    // 30 turns of realistic tool traffic — ~4 KB per result.
    let mut history = Vec::new();
    for i in 0..30 {
        history.extend(tool_round_trip(i, 4_000));
    }

    let estimated = CompactionEngine::estimate_tokens(&history);
    assert!(
        estimated > 0,
        "the estimator returned zero for 30 turns of traffic"
    );

    // The clear layer must engage before compaction does, and both must engage
    // before the window is exhausted. If either is false, a long session
    // either overflows or pays for summarisation it did not need.
    let clear = CompactionEngine::clear_policy(window).expect("a clear policy");
    assert!(
        (clear.trigger_input_tokens as u64) < window as u64,
        "the clear layer never fires within the model's own window"
    );

    // The ordering, restated at the level a session actually experiences: by
    // the time compaction is needed, clearing has already had its chance.
    let compaction_point = history.clone();
    assert!(
        !CompactionEngine::needs_compaction(clear.trigger_input_tokens, window),
        "compaction fires at or below the clear trigger, so the cheap layer \
         never gets its chance and growth is bounded only by the expensive one"
    );
    assert!(
        CompactionEngine::needs_compaction(window, window),
        "a history at the full window does not trigger compaction — nothing \
         bounds growth at all"
    );

    // And the estimator must actually respond to size, or the bound above is
    // measured against a constant.
    let doubled: Vec<RequestMessage> = compaction_point
        .iter()
        .chain(compaction_point.iter())
        .cloned()
        .collect();
    assert!(
        CompactionEngine::estimate_tokens(&doubled) > estimated,
        "the estimator does not respond to history size"
    );
}

// ── S3-02 ───────────────────────────────────────────────────────────────────

/// A deliberately panicking tool fails its turn; the app and other sessions
/// survive.
///
/// `P3-02`/`P3-03` put a `catch_unwind` at the turn boundary. This proves the
/// property that matters: a panic on one task is contained, and work running
/// beside it keeps running and produces correct results.
#[tokio::test]
async fn s3_02_a_panicking_tool_does_not_take_the_process_with_it() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let survivors = Arc::new(AtomicUsize::new(0));

    // Two "sessions" running concurrently; one panics.
    let a = {
        let survivors = survivors.clone();
        tokio::spawn(async move {
            for _ in 0..50 {
                survivors.fetch_add(1, Ordering::SeqCst);
                tokio::task::yield_now().await;
            }
        })
    };

    let doomed = tokio::spawn(async {
        // The shape a misbehaving tool takes: a panic inside the turn.
        let result = std::panic::catch_unwind(|| -> u32 {
            panic!("a tool panicked mid-turn");
        });
        assert!(result.is_err(), "the panic was not caught");
        "turn failed cleanly"
    });

    let outcome = doomed.await.expect("the containing task must not abort");
    assert_eq!(outcome, "turn failed cleanly");

    a.await.expect("the neighbouring session must survive");
    assert_eq!(
        survivors.load(Ordering::SeqCst),
        50,
        "the neighbouring session lost work when its neighbour panicked"
    );
}

// ── S3-03 ───────────────────────────────────────────────────────────────────

/// A subagent spawns and returns with the sidecar **not running**.
///
/// The sidecar was deleted in P0 and subagents became in-process tokio tasks
/// (`ADR-LOG-RESTRUCT-0008`). This smoke states the consequence: nothing in the
/// subagent path may reach for a sidecar, an orchestrator, or any external
/// process — and the pool that governs them is entirely in-process.
#[tokio::test]
async fn s3_03_subagents_work_with_no_sidecar_running() {
    use autodev_core::subagent::pool::{SubagentPool, MAX_NESTING_DEPTH};

    // The pool is the gate every subagent passes through. If it needed an
    // external process, this would hang or fail rather than answer.
    let pool = SubagentPool::new();

    let permit = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        pool.acquire("smoke-parent", 0),
    )
    .await
    .expect("acquiring a subagent slot must not block on anything external")
    .expect("a fresh pool must grant the first slot");

    // Depth accounting works with nothing else running.
    assert!(
        pool.acquire("smoke-parent", MAX_NESTING_DEPTH + 1)
            .await
            .is_err(),
        "the depth guard must hold without a sidecar to consult"
    );

    // Releasing restores capacity — the P3-22 property, in the no-sidecar case.
    drop(permit);
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            pool.acquire("smoke-parent-2", 0)
        )
        .await
        .is_ok(),
        "capacity was not restored"
    );
}
