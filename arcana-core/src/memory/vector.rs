// Vector-search substrate for memory (`P3-30`, `P3-31`).
//
// ── Why this file is separate from `db.rs` ───────────────────────────────────
//
// `sqlite3_auto_extension` is **process-global**, not per-connection. It
// installs a hook that every subsequently-opened SQLite connection runs. That
// has two consequences the rest of the module depends on:
//
//   1. Registration must happen **once**, at startup, **before any subsystem
//      opens a database** — not lazily inside `MemoryDb::open()`. A lazy
//      registration would leave connections opened earlier in the process
//      without the `vec0` module, and they would fail with `no such module`
//      at a distance from the cause.
//   2. Registering twice is not idempotent in any documented way, so a
//      `OnceLock` guards it.
//
// Verified before adoption: `sqlite-vec`'s entire `[dependencies]` is `cc`. It
// compiles `sqlite-vec.c` with `-DSQLITE_CORE` and never touches
// `libsqlite3-sys`, so there is nothing to conflict with our pinned
// `rusqlite 0.31`, and `loadable_extension` never meets `bundled`.

use std::sync::OnceLock;

/// Embedding width. **384**, from BGE-small-en-v1.5.
///
/// ⚠ An earlier decision chose OpenAI `text-embedding-3-small` at 1536 and was
/// retracted the same day; the `vec0` column is **384** and the ledger records
/// the correction (`ADR-05-02`). The constant lives here so the schema and the
/// embedder cannot disagree about it.
pub const EMBEDDING_DIMS: usize = 384;

static REGISTERED: OnceLock<bool> = OnceLock::new();

/// Install `sqlite-vec` for every connection this process opens from now on.
///
/// Idempotent, and safe to call from anywhere — but it must be called **before
/// the first database is opened**, which is why `main.rs` calls it during
/// startup rather than any store calling it on demand.
///
/// Returns whether registration actually ran (`false` = already registered).
pub fn register_once() -> bool {
    let mut ran = false;
    REGISTERED.get_or_init(|| {
        // SAFETY: `sqlite3_vec_init` is the extension entry point the crate
        // exports for exactly this purpose, and the transmute to the
        // `xEntryPoint` signature SQLite expects is the registration idiom the
        // crate itself documents and tests.
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
        ran = true;
        true
    });
    ran
}

/// Whether `vec0` is usable on this connection.
///
/// ⚠ Asked by **executing** a `vec0` call rather than by inspecting state: a
/// registration that silently failed and a registration that never happened
/// look identical from the outside, and the only thing that distinguishes them
/// is whether the module answers.
pub fn is_available(conn: &rusqlite::Connection) -> bool {
    conn.query_row("SELECT vec_version()", [], |r| r.get::<_, String>(0))
        .is_ok()
}

/// The `vec0` version string, for diagnostics and the degraded-mode report.
pub fn version(conn: &rusqlite::Connection) -> Option<String> {
    conn.query_row("SELECT vec_version()", [], |r| r.get::<_, String>(0))
        .ok()
}

/// Serialise an embedding for `vec0`'s `float[N]` column.
///
/// `vec0` takes the raw little-endian `f32` bytes as a BLOB.
pub fn encode(embedding: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(embedding.len() * 4);
    for f in embedding {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn conn() -> Connection {
        register_once();
        Connection::open_in_memory().expect("open")
    }

    #[test]
    fn the_extension_registers_and_answers() {
        let c = conn();
        assert!(
            is_available(&c),
            "vec0 did not answer — registration is the load-bearing step and it \
             is process-global, so a failure here means every later connection \
             is also without it"
        );
        let v = version(&c).expect("a version");
        assert!(v.starts_with('v'), "unexpected version string: {v}");
    }

    #[test]
    fn registering_twice_is_a_no_op() {
        // `main.rs` calls this at startup; a store calling it defensively must
        // not double-register.
        register_once();
        let second = register_once();
        assert!(!second, "the second registration should report no-op");
        assert!(is_available(&conn()));
    }

    #[test]
    fn an_embedding_encodes_to_four_bytes_per_dimension() {
        let e = vec![1.0f32, -2.5, 0.0];
        let bytes = encode(&e);
        assert_eq!(bytes.len(), 12);
        assert_eq!(&bytes[0..4], &1.0f32.to_le_bytes());
        assert_eq!(&bytes[4..8], &(-2.5f32).to_le_bytes());
    }

    /// 🔴 `P3-31`'s whole claim: scope is a **pre-filter**, not a post-filter
    /// over the top-k.
    ///
    /// The fixture is built to **discriminate** between the two. A post-filter
    /// takes the global top-k and then drops the wrong scopes; a pre-filter
    /// searches only within the scope. They differ exactly when the global
    /// top-k is dominated by rows from another scope — so scope `b` is given
    /// the two nearest vectors, and the assertion is that a `k=2` search in
    /// scope `a` still returns two `a` rows, including a far one that a
    /// post-filter could never surface.
    #[test]
    fn scope_is_a_pre_filter_not_a_post_filter() {
        let c = conn();
        c.execute_batch(
            "CREATE VIRTUAL TABLE m USING vec0(
                 memory_id TEXT PRIMARY KEY,
                 scope_key TEXT partition key,
                 embedding float[2]
             );",
        )
        .expect("vec0 with a partition key must be creatable");

        let mut ins = c
            .prepare("INSERT INTO m(memory_id, scope_key, embedding) VALUES (?1, ?2, ?3)")
            .unwrap();
        // Scope `b` owns the two closest points to the query (0,0).
        ins.execute(rusqlite::params!["b1", "b", encode(&[0.0, 0.0])])
            .unwrap();
        ins.execute(rusqlite::params!["b2", "b", encode(&[0.1, 0.0])])
            .unwrap();
        // Scope `a` has one near-ish and one deliberately far point.
        ins.execute(rusqlite::params!["a1", "a", encode(&[0.5, 0.0])])
            .unwrap();
        ins.execute(rusqlite::params!["a2", "a", encode(&[1.0, 1.0])])
            .unwrap();
        drop(ins);

        let mut q = c
            .prepare(
                "SELECT memory_id, distance FROM m
                 WHERE embedding MATCH ?1 AND k = 2 AND scope_key = 'a'
                 ORDER BY distance",
            )
            .unwrap();
        let rows: Vec<(String, f64)> = q
            .query_map(rusqlite::params![encode(&[0.0, 0.0])], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
            })
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(
            rows.len(),
            2,
            "a scoped k=2 search returned {} rows — a POST-filter over the \
             global top-2 would have returned 0, because both global nearest \
             neighbours are in scope 'b'. This is the difference the task \
             names (G-12).",
            rows.len()
        );
        assert!(
            rows.iter().all(|(id, _)| id.starts_with('a')),
            "scope leaked: {rows:?}"
        );
        assert!(
            rows.iter().any(|(_, d)| *d > 1.0),
            "the far in-scope row was not surfaced, so this fixture did not \
             actually discriminate: {rows:?}"
        );
    }

    #[test]
    fn a_partition_key_must_be_text_or_integer() {
        // Measured limit, kept as a test so the schema cannot drift into an
        // unsupported type and fail at runtime with an error that names the
        // wrong column.
        let c = conn();
        let err = c
            .execute_batch(
                "CREATE VIRTUAL TABLE bad USING vec0(
                     id TEXT PRIMARY KEY,
                     weight FLOAT partition key,
                     embedding float[2]
                 );",
            )
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            !msg.is_empty(),
            "a FLOAT partition key must be rejected; sqlite-vec allows TEXT and \
             INTEGER only"
        );
    }
}
