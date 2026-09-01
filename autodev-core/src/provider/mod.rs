// Provider module — LLM provider abstraction for PA mode.
// PA mode executes LLM calls natively in Rust; Worker mode delegates to
// core-orchestrator's litellm provider. See DEVELOPMENT_PLAN.md Section 4.1.

pub mod anthropic;
pub mod google;
pub mod openai;
pub mod streaming;
pub mod traits;
pub mod types;
pub mod usage;

pub use traits::Provider;
pub use types::{CompletionRequest, CompletionResponse, ModelInfo, ProviderStatus, StreamChunk};
