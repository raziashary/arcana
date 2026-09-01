// The Provider trait — common interface for all LLM providers in PA mode.
// Pattern adopted from Goose's Provider trait (goose-main/crates/goose/src/providers/).

use super::types::{CompletionRequest, CompletionResponse, ModelInfo, ProviderStatus, StreamChunk};
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

    /// Non-streaming completion — used for short tasks, testing
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;

    /// Streaming completion — primary path for agent loop
    async fn stream(&self, request: CompletionRequest) -> Result<StreamResult>;

    /// Test API key validity (cheap API call)
    async fn test_connection(&self) -> Result<ProviderStatus>;
}
