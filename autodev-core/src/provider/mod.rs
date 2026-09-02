// Provider module — LLM provider abstraction for PA mode.
// PA mode executes LLM calls natively in Rust; Worker mode delegates to
// core-orchestrator's litellm provider. See DEVELOPMENT_PLAN.md Section 4.1.

pub mod anthropic;
pub mod capped;
pub mod catalogue;
pub mod google;
pub mod openai;
pub mod routing;
pub mod streaming;
pub mod traits;
pub mod types;
pub mod usage;

pub use traits::Provider;
pub use types::{CompletionRequest, CompletionResponse, ModelInfo, ProviderStatus, StreamChunk};

#[cfg(test)]
#[path = "conformance_tests.rs"]
mod conformance_tests;

#[cfg(test)]
#[path = "anthropic_tests.rs"]
mod anthropic_tests;

#[cfg(test)]
#[path = "openai_tests.rs"]
mod openai_tests;
