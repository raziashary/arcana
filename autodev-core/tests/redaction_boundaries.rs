//! P1-21 — redaction is applied at the boundaries, proven by behaviour.
//!
//! These tests write a secret through the real store functions and then read
//! the database back with plain SQL. That is deliberately not a check that
//! `redact_text` was *called*: a grep for the call would pass just as happily
//! if the result were discarded, or if the redaction ran on a copy. Reading the
//! stored bytes back is the only thing that shows the secret is not there.
//!
//! Memory is persistent, so a secret written into it is a secret kept forever —
//! `G4:486` recorded exactly that happening with tool arguments.
//!
//! Runs in its own test binary so redirecting `HOME` cannot disturb the unit
//! tests.

use std::path::PathBuf;
use std::sync::Mutex;

use autodev_core::memory::db::MemoryDb;

/// `HOME`/`USERPROFILE` are process-global; serialise every test that swaps
/// them rather than relying on `--test-threads=1`, which nobody remembers.
static HOME_LOCK: Mutex<()> = Mutex::new(());

/// A realistic AWS secret access key: 40 characters.
const SECRET: &str = "wJalrXUtnFEMIK7MDENGbPxRfiCYEXAMPLEKEYZ1";

fn sandbox(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("autodev-redaction-boundary-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".autodev")).expect("mkdir");
    dir
}

fn with_home<T>(dir: &PathBuf, f: impl FnOnce() -> T) -> T {
    let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let old_user = std::env::var("USERPROFILE").ok();
    let old_home = std::env::var("HOME").ok();
    std::env::set_var("USERPROFILE", dir);
    std::env::set_var("HOME", dir);
    let out = f();
    match old_user {
        Some(v) => std::env::set_var("USERPROFILE", v),
        None => std::env::remove_var("USERPROFILE"),
    }
    match old_home {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    out
}

/// Read every text value out of `episodic_memories` so the assertion cannot
/// miss the secret by looking in the wrong column.
fn all_episodic_text(dir: &PathBuf) -> String {
    let db_path = dir.join(".autodev").join("memory.db");
    let conn = rusqlite::Connection::open(&db_path).expect("open memory.db");
    let mut stmt = conn
        .prepare("SELECT * FROM episodic_memories")
        .expect("select");
    let column_count = stmt.column_count();
    let mut out = String::new();
    let mut rows = stmt.query([]).expect("query");
    while let Some(row) = rows.next().expect("row") {
        for i in 0..column_count {
            if let Ok(v) = row.get::<_, String>(i) {
                out.push_str(&v);
                out.push('\n');
            }
        }
    }
    out
}

#[test]
fn a_secret_in_a_user_message_never_reaches_the_memory_store() {
    let dir = sandbox("user-message");
    let content = format!("here is my config:\nAWS_SECRET_ACCESS_KEY={SECRET}\nREGION=us-east-1");

    with_home(&dir, || {
        // Force the store to exist inside the sandbox before writing.
        MemoryDb::open().expect("memory db");
        autodev_core::memory::episodic::EpisodicStore::store_user_message(
            "session-1",
            &content,
            None,
            None,
        )
        .expect("store");
    });

    let stored = all_episodic_text(&dir);
    assert!(
        !stored.contains(SECRET),
        "the secret survived into the memory store:\n{stored}"
    );
    // The surrounding content must survive, or redaction has destroyed the
    // context the memory exists to preserve.
    assert!(
        stored.contains("us-east-1"),
        "redaction removed non-secret content too:\n{stored}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_secret_in_an_agent_response_never_reaches_the_memory_store() {
    let dir = sandbox("agent-response");
    let content = format!("I read the file. It contains AWS_SECRET_ACCESS_KEY={SECRET}");
    let tool_calls = serde_json::json!([{ "name": "read_file", "api_key": SECRET }]);

    with_home(&dir, || {
        MemoryDb::open().expect("memory db");
        autodev_core::memory::episodic::EpisodicStore::store_agent_response(
            "session-1",
            &content,
            Some("claude-haiku-4-5"),
            0.0,
            10,
            20,
            Some(&format!("thinking about {SECRET}")),
            Some(tool_calls),
            None,
            None,
        )
        .expect("store");
    });

    let stored = all_episodic_text(&dir);
    assert!(
        !stored.contains(SECRET),
        "the secret survived in content, thinking or tool_calls:\n{stored}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_secret_in_a_stored_fact_never_reaches_the_memory_store() {
    let dir = sandbox("fact");
    let fact = format!("the deploy key is AWS_SECRET_ACCESS_KEY={SECRET}");

    with_home(&dir, || {
        MemoryDb::open().expect("memory db");
        autodev_core::memory::semantic::SemanticStore::store_fact(
            &fact,
            autodev_core::memory::types::FactCategory::Reference,
            0.9,
            "test",
            None,
            None,
            &[],
        )
        .expect("store");
    });

    let db_path = dir.join(".autodev").join("memory.db");
    let conn = rusqlite::Connection::open(&db_path).expect("open");
    let text: String = conn
        .query_row("SELECT fact_text FROM semantic_memories LIMIT 1", [], |r| {
            r.get(0)
        })
        .expect("read fact back");

    assert!(
        !text.contains(SECRET),
        "the secret survived into semantic memory: {text}"
    );

    drop(conn);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The inverse: without redaction these tests would pass trivially if the
/// store simply dropped the content. This asserts the store really does keep
/// what it is given, so the assertions above mean something.
#[test]
fn the_memory_store_does_keep_the_content_it_is_given() {
    let dir = sandbox("control");
    let content = "a perfectly ordinary note about src/main.rs";

    with_home(&dir, || {
        MemoryDb::open().expect("memory db");
        autodev_core::memory::episodic::EpisodicStore::store_user_message(
            "session-1",
            content,
            None,
            None,
        )
        .expect("store");
    });

    let stored = all_episodic_text(&dir);
    assert!(
        stored.contains("a perfectly ordinary note about src/main.rs"),
        "the store dropped the content, so the redaction tests prove nothing:\n{stored}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
