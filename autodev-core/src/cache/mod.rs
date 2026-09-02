// Prompt caching (P2-12 … P2-16).
//
// The order of these two modules is deliberate and is the phase's rule:
// `monitor` — the instrument that can report failure — is built and proven
// before `prefix` builds anything for it to measure, and before any adapter
// sends a `cache_control` block.
//
// Caching is the one optimisation that fails completely silently: below a
// model's minimum, or after a single changed byte, the provider caches nothing
// and returns no error. `usage.cached_read` on a repeated prefix is the only
// fact that separates a working cache from a broken one.

pub mod monitor;
pub mod prefix;

pub use monitor::{CacheMonitor, CacheVerdict};
pub use prefix::{build, will_cache, CachePrefix};
