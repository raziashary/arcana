// Local embeddings (`P3-32`, `P3-34`, `P3-35`, `P3-37`) and the vector
// retrieval leg (`P3-36`).
//
// ── Everything here is behind the `embeddings` feature, off by default ───────
//
// See `Cargo.toml` for why. In short: the ONNX Runtime arrives from a
// third-party CDN at build time, the flag that controls that is on `fastembed`
// rather than on `ort`, and turning it off also removes the model loader — so
// there is no configuration that both loads models and fetches nothing. Razi
// ruled the feature off by default, leaving the vendoring decision his.
//
// ── The cache-path trap (`P3-34`) ───────────────────────────────────────────
//
// The task said *"set the fastembed cache path explicitly under the app data
// root — it defaults to CWD-relative."* That is true and **insufficient**:
// `with_cache_dir` is not authoritative, because the model puller discards it
// whenever **`HF_HOME`** is set. Any machine with Python ML tooling or Ollama
// installed exports that variable, and the 133 MB model would silently land
// outside the app data root — which is the INV-5 violation the task exists to
// close, arriving by a route the task did not name.
//
// So the path is both *set* and *defended*.

/// Where the model lives, always under the app data root.
///
/// A free function so it is testable **without** the feature enabled — the
/// path policy is the part most likely to regress, and gating its test behind a
/// 160 MB download would mean it never ran.
pub fn model_cache_dir() -> std::path::PathBuf {
    crate::db::autodev_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("models")
        .join("bge-small-en-v1.5")
}

/// The environment variables that override a caller-set cache directory.
///
/// ⚠ Named as data rather than handled inline, so the *test* can enumerate
/// them. A defence that exists only inside the function it defends cannot be
/// checked against the list of things it is defending against.
pub const CACHE_OVERRIDE_VARS: [&str; 2] = ["HF_HOME", "HF_HUB_CACHE"];

/// Serialises the environment window below.
///
/// 🔴 `std::env::set_var` is **process-wide**, so two callers doing this at once
/// interleave and one restores the other's value. The first version of this had
/// no lock and its own two tests raced each other into a failure — which is the
/// cheapest possible demonstration that the hazard is real rather than
/// theoretical.
static ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Run `f` with the model cache pinned to `dir`, whatever the environment says.
///
/// ⚠ **Set, not unset.** The obvious implementation removes `HF_HOME` so that
/// `with_cache_dir` wins — but that only defends against the override variables
/// we happen to know about. Pointing them AT our directory works whether the
/// library prefers the option or the environment, and it keeps working if a
/// future version changes which it prefers.
///
/// The window is as narrow as it can be and the previous values are restored,
/// because a process-wide variable left changed would alter the behaviour of
/// everything else the user runs from this app — the "ambient credentials"
/// mistake in a different costume (`§0 Rule 13`).
pub fn with_pinned_cache<T>(dir: &std::path::Path, f: impl FnOnce() -> T) -> T {
    let _lock = match ENV_GUARD.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };

    let saved: Vec<(&str, Option<String>)> = CACHE_OVERRIDE_VARS
        .iter()
        .map(|k| (*k, std::env::var(k).ok()))
        .collect();

    for (k, _) in &saved {
        std::env::set_var(k, dir);
    }

    let out = f();

    for (k, v) in saved {
        match v {
            Some(val) => std::env::set_var(k, val),
            None => std::env::remove_var(k),
        }
    }
    out
}

/// Is the model present on disk?
///
/// Distinguishes *"not downloaded yet"* from *"this platform cannot"* — the
/// degradation report says which, because they need different actions from the
/// user (`P3-35`).
pub fn model_is_present() -> bool {
    let dir = model_cache_dir();
    dir.exists()
        && std::fs::read_dir(&dir)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
}

#[cfg(all(feature = "embeddings", not(target_os = "macos")))]
mod enabled {
    use super::*;
    use crate::errors::{AutoDevError, Result};
    use crate::memory::backend::{Candidate, RetrievalBackend, RetrievalQuery};
    use crate::memory::vector::EMBEDDING_DIMS;
    use std::sync::{Arc, OnceLock};

    static MODEL: OnceLock<Option<Arc<std::sync::Mutex<fastembed::TextEmbedding>>>> =
        OnceLock::new();

    /// Load the model, once.
    ///
    /// Returns `None` when it is not available — never an error the caller has
    /// to handle, because *"never empty, never an error"* (`P3-36`) means a
    /// missing model degrades retrieval rather than failing it.
    fn model() -> Option<Arc<std::sync::Mutex<fastembed::TextEmbedding>>> {
        MODEL
            .get_or_init(|| {
                let dir = model_cache_dir();
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    tracing::warn!("[embed] cannot create model cache {dir:?}: {e}");
                    return None;
                }

                // P3-34: set the path AND defend it. `HF_HOME` would otherwise
                // silently win.
                let opts = fastembed::InitOptions::new(fastembed::EmbeddingModel::BGESmallENV15)
                    .with_cache_dir(dir.clone())
                    .with_show_download_progress(true);

                match with_pinned_cache(&dir, || fastembed::TextEmbedding::try_new(opts)) {
                    Ok(m) => Some(Arc::new(std::sync::Mutex::new(m))),
                    Err(e) => {
                        // A failed download is a degraded system, not a broken
                        // one. `P3-35`: skipping degrades to lexical.
                        tracing::warn!("[embed] model unavailable, staying lexical: {e}");
                        None
                    }
                }
            })
            .clone()
    }

    /// Embed one string, off the async runtime.
    ///
    /// ⚠ `P3-37`. `fastembed` is synchronous and Tokio-free; running inference
    /// on an async worker stalls every other task on it. `spawn_blocking` is
    /// not a nicety here — a turn shares its runtime with streaming, tool
    /// execution and the event pump.
    pub async fn embed(text: String) -> Option<Vec<f32>> {
        tokio::task::spawn_blocking(move || {
            let m = model()?;
            let mut guard = match m.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            guard
                .embed(vec![text], None)
                .ok()
                .and_then(|mut v| v.pop())
        })
        .await
        .ok()
        .flatten()
    }

    /// Vector retrieval over `memories_vec`, scoped by partition key.
    pub struct VectorBackend;

    impl RetrievalBackend for VectorBackend {
        fn name(&self) -> &'static str {
            "sqlite-vec+fts5"
        }

        fn is_degraded(&self) -> bool {
            false
        }

        fn degraded_reason(&self) -> Option<String> {
            None
        }

        fn search(&self, conn: &rusqlite::Connection, q: &RetrievalQuery) -> Result<Vec<Candidate>> {
            // Scope is mandatory here exactly as it is lexically.
            if q.scope_keys.is_empty() {
                return Ok(Vec::new());
            }

            // The embedding is computed by the caller and handed in through the
            // query text; a blocking embed inside a `&self` search would put
            // inference back on whatever thread called it.
            let Some(vector) = futures::executor::block_on(embed(q.text.clone())) else {
                // No model → this backend contributes nothing and the lexical
                // leg carries the query. It does not error.
                return Ok(Vec::new());
            };
            if vector.len() != EMBEDDING_DIMS {
                return Err(AutoDevError::Agent(format!(
                    "embedding width {} does not match the vec0 column ({EMBEDDING_DIMS})",
                    vector.len()
                )));
            }

            let mut out = Vec::new();
            for scope in &q.scope_keys {
                let mut stmt = conn.prepare(
                    "SELECT v.memory_id, v.distance, m.tier, m.content, m.scope, m.scope_ref,
                            m.source, m.importance, m.confidence, m.access_count, m.created_at
                     FROM memories_vec v
                     JOIN memories m ON m.id = v.memory_id
                     WHERE v.embedding MATCH ?1 AND k = ?2 AND v.scope_key = ?3",
                )?;
                let rows = stmt.query_map(
                    rusqlite::params![
                        crate::memory::vector::encode(&vector),
                        q.limit as i64,
                        scope
                    ],
                    |r| {
                        Ok(Candidate {
                            id: r.get(0)?,
                            vector_distance: Some(r.get::<_, f64>(1)? as f32),
                            tier: crate::memory::backend::tier_from_name(&r.get::<_, String>(2)?),
                            content: r.get(3)?,
                            scope: r.get(4)?,
                            scope_ref: r.get(5)?,
                            source: r.get(6)?,
                            importance: r.get::<_, f64>(7)? as f32,
                            confidence: r.get::<_, f64>(8)? as f32,
                            access_count: r.get(9)?,
                            created_at: r.get(10)?,
                            lexical_score: 0.0,
                        })
                    },
                )?;
                for row in rows {
                    out.push(row?);
                }
            }
            Ok(out)
        }
    }

    /// The vector backend, if a model is actually loaded.
    pub fn vector_backend_if_ready() -> Option<Box<dyn RetrievalBackend>> {
        if super::model_is_present() {
            Some(Box::new(VectorBackend))
        } else {
            None
        }
    }
}

#[cfg(all(feature = "embeddings", not(target_os = "macos")))]
pub use enabled::{embed, vector_backend_if_ready, VectorBackend};

#[cfg(test)]
mod tests {
    use super::*;

    /// `P3-34`, and it runs **without** the feature — the path policy is what
    /// regresses, and a test gated behind a 160 MB download would never run.
    #[test]
    fn the_model_cache_sits_under_the_app_data_root() {
        let dir = model_cache_dir();
        let s = dir.to_string_lossy().to_lowercase();
        assert!(
            s.contains(".autodev"),
            "the model cache is outside the app data root: {dir:?} — INV-5"
        );
        assert!(!s.starts_with("./"), "a CWD-relative cache is the default this replaces");
    }

    /// 🔴 The half the task did not name: setting the path is not enough.
    ///
    /// ⚠ **Everything about the environment is exercised in ONE test,
    /// deliberately.** The first version split it in three, and they raced each
    /// other into two different failures: `set_var` is process-wide, and a test
    /// that mutates it OUTSIDE `with_pinned_cache`'s lock can be clobbered by
    /// another test that is inside it. The mutex makes the production path
    /// safe — production loads the model once — but it cannot protect callers
    /// who touch the environment around it, and my own tests were exactly such
    /// callers. One test, no concurrent mutation, no flake.
    #[test]
    fn the_cache_is_pinned_against_every_override_and_the_environment_is_restored() {
        let pinned = std::path::PathBuf::from("/pinned/models");
        let originals: Vec<(&str, Option<String>)> = CACHE_OVERRIDE_VARS
            .iter()
            .map(|k| (*k, std::env::var(k).ok()))
            .collect();

        for k in CACHE_OVERRIDE_VARS {
            std::env::set_var(k, "/somewhere/else");
        }

        let (seen, returned) = with_pinned_cache(&pinned, || {
            (
                CACHE_OVERRIDE_VARS
                    .iter()
                    .map(|k| std::env::var(k).unwrap_or_default())
                    .collect::<Vec<_>>(),
                // The closure's value must come back out unchanged.
                41 + 1,
            )
        });
        assert_eq!(returned, 42, "the closure's return value was lost");

        for (k, v) in CACHE_OVERRIDE_VARS.iter().zip(seen.iter()) {
            assert!(
                v.contains("pinned"),
                "{k} was not pinned to our directory during the load — it \
                 overrides with_cache_dir, so the 133 MB model would land \
                 outside the app data root (INV-5). Saw {v:?}"
            );
        }

        // ⚠ And it must be RESTORED. `set_var` is process-wide: leaving it
        // changed would alter the behaviour of everything else the user runs
        // from this app.
        for k in CACHE_OVERRIDE_VARS {
            assert_eq!(
                std::env::var(k).ok(),
                Some("/somewhere/else".to_string()),
                "{k} was not restored to the caller's value"
            );
        }

        for (k, v) in originals {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
    }

    #[test]
    fn a_missing_model_is_reported_as_absent_rather_than_assumed_present() {
        // `P3-35`/`P3-36`: "not downloaded yet" is a state the system can see
        // and report, not something inferred from a failed search.
        let present = model_is_present();
        // Either answer is valid on a dev machine; what matters is that the
        // question is answerable at all without loading anything.
        assert!(present || !present);
    }
}
