// Provider module — LLM provider abstraction for PA mode.
// PA mode executes LLM calls natively in Rust; Worker mode delegates to
// core-orchestrator's litellm provider. See DEVELOPMENT_PLAN.md Section 4.1.

pub mod traits;
pub mod types;
pub mod streaming;
pub mod anthropic;
pub mod openai;
pub mod google;

pub use traits::Provider;
pub use types::{CompletionRequest, CompletionResponse, StreamChunk, ModelInfo, ProviderStatus};
