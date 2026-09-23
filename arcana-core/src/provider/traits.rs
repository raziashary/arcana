// The Provider trait — common interface for all LLM providers in PA mode.
// Pattern adopted from Goose's Provider trait (goose-main/crates/goose/src/providers/).

use super::types::{CompletionRequest, CompletionResponse, ModelInfo, ProviderStatus, StreamChunk};
use super::usage::ModelCapabilities;
use crate::errors::Result;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

pub type StreamResult = Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>;

#[async_trait]
pub trait Provider: Send + Sync {
    /// Provider identifier (e.g. "anthropic", "openai")
    fn id(&self) -> &str;

    /// Human-readable display name
    fn name(&self) -> &str;

    /// All models available from this provider
    fn models(&self) -> Vec<ModelInfo>;

    /// What one **model** can do (`P2-03`).
    ///
    /// Keyed by model rather than by provider, because capability varies
    /// *within* a vendor and a provider-level answer is wrong for most of
    /// them: `claude-sonnet-5` caches above 1,024 tokens while
    /// `claude-opus-5` needs 4,096, and `gpt-5.4` reasons where
    /// `gpt-5.4-nano` does not.
    ///
    /// Defaulted to the shared catalogue, which is keyed by model id and is
    /// the same table `models()` is built from — so an adapter cannot answer
    /// this question differently from how it answers that one.
    fn capabilities(&self, model: &str) -> ModelCapabilities {
        super::catalogue::capabilities_for(model)
    }

    /// Non-streaming completion — used for short tasks, testing
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;

    /// Streaming completion — primary path for agent loop
    async fn stream(&self, request: CompletionRequest) -> Result<StreamResult>;

    /// Test API key validity (cheap API call)
    async fn test_connection(&self) -> Result<ProviderStatus>;
}
