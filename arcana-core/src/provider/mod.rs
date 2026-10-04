// Provider module — the LLM provider abstraction.
// Every LLM call in this application is made from here, natively in Rust,
// for the assistant and the fleet alike.

pub mod anthropic;
pub mod capped;
pub mod catalogue;
pub mod factory;
pub mod google;
pub mod openai;
pub mod retry;
pub mod routing;
pub mod run_budget;
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
