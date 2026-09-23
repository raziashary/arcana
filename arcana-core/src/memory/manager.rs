// Backend selection and the degradation report (`P3-36`).
//
// One place decides which retrieval leg is live, so "are we degraded?" has a
// single answer rather than one per call site.

use crate::memory::backend::{LexicalBackend, RetrievalBackend};

/// The backend this build uses.
///
/// ⚠ **macOS is lexical-only, by ruling and by upstream constraint.** Microsoft
/// publishes no x86_64 macOS ONNX Runtime in any release from v1.25 to v1.29,
/// our DMG is a *universal* binary, and a universal binary needs both slices to
/// link — so an unconditional `fastembed` dependency does not degrade Intel
/// macOS, it fails the macOS build outright. The embedding backend is therefore
/// compiled in only where a runtime exists.
///
/// This is the architecture spec `03 §4.4` already specifies — *"degradation,
/// not failure"* — rather than a workaround bolted on.
pub fn active_backend() -> Box<dyn RetrievalBackend> {
    #[cfg(all(feature = "embeddings", not(target_os = "macos")))]
    {
        if let Some(v) = crate::memory::embed::vector_backend_if_ready() {
            return v;
        }
    }
    Box::new(LexicalBackend)
}

/// Whether semantic retrieval is compiled into this build at all.
///
/// Distinct from "is it ready" — a model still downloading is a different state
/// from a platform that can never have one, and a user deserves to be told
/// which.
pub const fn embeddings_supported_on_this_platform() -> bool {
    cfg!(all(feature = "embeddings", not(target_os = "macos")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backend_is_always_available() {
        // `P3-36`: retrieval must never be absent — "never empty, never an
        // error". Even with no model, no feature and no platform support,
        // something answers.
        let b = active_backend();
        assert!(!b.name().is_empty());
    }

    #[test]
    fn macos_reports_no_platform_support_and_still_retrieves() {
        if cfg!(target_os = "macos") {
            assert!(
                !embeddings_supported_on_this_platform(),
                "macOS cannot link ort — this must be reported, not attempted"
            );
        }
        // Either way there is a working backend.
        assert!(active_backend().is_degraded() || embeddings_supported_on_this_platform());
    }
}
