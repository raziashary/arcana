# Using Ollama with Rust — A Comprehensive Guide

> **Last updated:** April 2026 | **ollama-rs version:** 0.3.4 | **Ollama:** latest

---

## Table of Contents

1. [Overview](#overview)
2. [Prerequisites](#prerequisites)
3. [Setting Up Ollama](#setting-up-ollama)
4. [Project Setup](#project-setup)
5. [Initialization](#initialization)
6. [Completion Generation](#completion-generation)
7. [Streaming Responses](#streaming-responses)
8. [Generation Options & Tuning](#generation-options--tuning)
9. [Chat Mode & Conversation History](#chat-mode--conversation-history)
10. [Model Management](#model-management)
11. [Embeddings](#embeddings)
12. [Function Calling & the Coordinator](#function-calling--the-coordinator)
13. [Custom Tools with the `#[function]` Macro](#custom-tools-with-the-function-macro)
14. [Using the Raw HTTP API Directly](#using-the-raw-http-api-directly)
15. [OpenAI-Compatible Endpoint](#openai-compatible-endpoint)
16. [Error Handling Best Practices](#error-handling-best-practices)
17. [Feature Flags Reference](#feature-flags-reference)
18. [Tips & Gotchas](#tips--gotchas)

---

## Overview

Ollama is a runtime that lets you download and serve large language models (LLMs) locally via a simple REST API at `http://localhost:11434`. For Rust developers, the primary crate for integrating with Ollama is **`ollama-rs`** — a type-safe, async-first library built on top of `reqwest` and `tokio`.

There are two main approaches to using Ollama from Rust:

- **`ollama-rs`** (recommended) — high-level, ergonomic Rust SDK maintained by the community.
- **Raw HTTP** — call the REST endpoints directly with `reqwest`, giving you full control at the cost of more boilerplate.

This guide covers both approaches comprehensively.

---

## Prerequisites

Before writing any Rust code, ensure you have the following installed:

- **Rust toolchain** — install via [rustup.rs](https://rustup.rs). Stable channel is sufficient.
- **Ollama** — install from [ollama.com](https://ollama.com).
- At least one **model pulled locally**, for example:
  ```bash
  ollama pull llama3.2
  ollama pull nomic-embed-text   # for embeddings
  ollama pull qwen2.5:32b        # recommended for tool/function calling
  ```
- Ollama service running (it starts automatically on most platforms after install; verify with `ollama list`).

---

## Setting Up Ollama

### Starting the Server

Ollama exposes its API at `http://localhost:11434` by default. To run it as a background service:

```bash
ollama serve
```

You can override the host/port with the `OLLAMA_HOST` environment variable:

```bash
OLLAMA_HOST=0.0.0.0:11434 ollama serve
```

> **Security note:** Exposing Ollama on `0.0.0.0` without a reverse proxy or firewall means any machine on your network can run inference on your hardware with no authentication. Keep it on `localhost` for personal use.

### Verifying the API

```bash
curl http://localhost:11434/api/tags
```

This returns a JSON list of all models available locally.

---

## Project Setup

### Creating a New Project

```bash
cargo new my-ollama-app
cd my-ollama-app
```

### Cargo.toml

Add the following to your `Cargo.toml`. The `stream` feature enables streaming responses and is highly recommended:

```toml
[dependencies]
ollama-rs = { version = "0.3.4", features = ["stream"] }
tokio = { version = "1", features = ["full"] }
tokio-stream = "0.1"

# Optional but useful extras
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json"] }
```

### Main Entry Point

Your `main.rs` must be async, powered by Tokio:

```rust
#[tokio::main]
async fn main() {
    // your code here
}
```

---

## Initialization

The `Ollama` struct is the main client. It can be constructed with defaults or with custom host/port values.

```rust
use ollama_rs::Ollama;

// Connects to http://localhost:11434 by default
let ollama = Ollama::default();

// Custom host and port
let ollama = Ollama::new("http://localhost".to_string(), 11434);

// Remote Ollama instance (e.g., a server on your LAN)
let ollama = Ollama::new("http://192.168.1.100".to_string(), 11434);
```

The `Ollama` struct is cheaply cloneable and can be shared across tasks with `Arc<Ollama>` if needed in concurrent workloads.

---

## Completion Generation

### Basic (Non-Streaming)

The simplest form of inference — send a prompt, receive a complete response:

```rust
use ollama_rs::Ollama;
use ollama_rs::generation::completion::GenerationRequest;

#[tokio::main]
async fn main() {
    let ollama = Ollama::default();

    let model = "llama3.2:latest".to_string();
    let prompt = "Why is the sky blue?".to_string();

    let res = ollama
        .generate(GenerationRequest::new(model, prompt))
        .await;

    match res {
        Ok(res) => println!("{}", res.response),
        Err(e) => eprintln!("Error: {e}"),
    }
}
```

The response struct (`GenerationResponse`) contains fields like `response` (the text), `model`, `done`, and timing statistics like `total_duration` and `eval_count`.

---

## Streaming Responses

Streaming returns tokens as they are generated rather than waiting for the full response. This is essential for any interactive or CLI application.

> **Requires:** the `stream` feature in `Cargo.toml`.

```rust
use ollama_rs::Ollama;
use ollama_rs::generation::completion::GenerationRequest;
use tokio::io::{self, AsyncWriteExt};
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() {
    let ollama = Ollama::default();

    let model = "llama3.2:latest".to_string();
    let prompt = "Explain how TCP/IP works in detail.".to_string();

    let mut stream = ollama
        .generate_stream(GenerationRequest::new(model, prompt))
        .await
        .expect("Failed to start stream");

    let mut stdout = io::stdout();

    while let Some(res) = stream.next().await {
        let responses = res.unwrap();
        for resp in responses {
            stdout
                .write_all(resp.response.as_bytes())
                .await
                .unwrap();
            stdout.flush().await.unwrap();
        }
    }

    println!(); // final newline
}
```

Each chunk in the stream is a `Vec<GenerationResponse>`, and `resp.done` will be `true` on the final chunk.

---

## Generation Options & Tuning

You can fine-tune model behaviour with `ModelOptions` / `GenerationOptions` (depending on your version of `ollama-rs`):

```rust
use ollama_rs::Ollama;
use ollama_rs::generation::completion::GenerationRequest;
use ollama_rs::models::ModelOptions;

#[tokio::main]
async fn main() {
    let ollama = Ollama::default();

    let options = ModelOptions::default()
        .temperature(0.2)       // Lower = more deterministic
        .repeat_penalty(1.5)    // Penalises repeated tokens
        .top_k(25)              // Limits sampling to top-K tokens
        .top_p(0.25)            // Nucleus sampling probability mass
        .num_predict(512);      // Max tokens to generate

    let req = GenerationRequest::new(
        "llama3.2:latest".to_string(),
        "Write a haiku about Rust programming.".to_string(),
    )
    .options(options);

    let res = ollama.generate(req).await.unwrap();
    println!("{}", res.response);
}
```

### Common Options Reference

| Option | Type | Description |
|---|---|---|
| `temperature` | f32 | Randomness (0.0–2.0). Lower = deterministic. |
| `top_k` | u32 | Limit sampling pool to top-K candidates. |
| `top_p` | f32 | Nucleus sampling — cumulative probability cutoff. |
| `repeat_penalty` | f32 | Penalise token repetition. Default ~1.1. |
| `num_predict` | i32 | Max new tokens to generate (-1 for unlimited). |
| `num_ctx` | u32 | Context window size. Critical for long conversations. |
| `seed` | i32 | Fixed seed for reproducible outputs. |

---

## Chat Mode & Conversation History

Chat mode keeps track of the conversation context, allowing multi-turn interactions where the model remembers earlier messages.

### Basic Chat

```rust
use ollama_rs::Ollama;
use ollama_rs::generation::chat::{request::ChatMessageRequest, ChatMessage};

#[tokio::main]
async fn main() {
    let ollama = Ollama::default();

    let model = "llama3.2:latest".to_string();

    let messages = vec![
        ChatMessage::system("You are a helpful Rust programming assistant.".to_string()),
        ChatMessage::user("What is ownership in Rust?".to_string()),
    ];

    let req = ChatMessageRequest::new(model, messages);
    let res = ollama.send_chat_messages(req).await.unwrap();

    println!("{}", res.message.content);
}
```

### Chat with History (Multi-Turn)

The library can automatically maintain conversation history for you. `Vec<ChatMessage>` implements the `ChatHistory` trait:

```rust
use ollama_rs::Ollama;
use ollama_rs::generation::chat::{request::ChatMessageRequest, ChatMessage};
use ollama_rs::history::ChatHistory;

#[tokio::main]
async fn main() {
    let ollama = Ollama::default();
    let model = "llama3.2:latest".to_string();

    let mut history: Vec<ChatMessage> = vec![];

    // First turn
    let res = ollama
        .send_chat_messages_with_history(
            &mut history,
            ChatMessageRequest::new(
                model.clone(),
                vec![ChatMessage::user("What is Rust?".to_string())],
            ),
        )
        .await
        .unwrap();

    println!("Assistant: {}", res.message.content);

    // Second turn — history is automatically included
    let res = ollama
        .send_chat_messages_with_history(
            &mut history,
            ChatMessageRequest::new(
                model.clone(),
                vec![ChatMessage::user("What are its main use cases?".to_string())],
            ),
        )
        .await
        .unwrap();

    println!("Assistant: {}", res.message.content);
}
```

Every message sent and received is stored in `history`, so subsequent requests automatically include the full context without any extra work on your part.

### Streaming Chat

```rust
use ollama_rs::Ollama;
use ollama_rs::generation::chat::{request::ChatMessageRequest, ChatMessage};
use tokio::io::{self, AsyncWriteExt};
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() {
    let ollama = Ollama::default();

    let mut stream = ollama
        .send_chat_messages_stream(ChatMessageRequest::new(
            "llama3.2:latest".to_string(),
            vec![ChatMessage::user("Tell me about async Rust.".to_string())],
        ))
        .await
        .unwrap();

    let mut stdout = io::stdout();

    while let Some(Ok(res)) = stream.next().await {
        if let Some(msg) = res.message {
            stdout.write_all(msg.content.as_bytes()).await.unwrap();
            stdout.flush().await.unwrap();
        }
    }

    println!();
}
```

---

## Model Management

`ollama-rs` exposes the full model management surface of the Ollama API.

### List Local Models

```rust
let models = ollama.list_local_models().await.unwrap();
for model in models {
    println!("{} — {} bytes", model.name, model.size);
}
```

### Show Model Information

```rust
let info = ollama
    .show_model_info("llama3.2:latest".to_string())
    .await
    .unwrap();

println!("Template: {}", info.template);
println!("Parameters: {:?}", info.details);
```

### Pull a Model

```rust
use ollama_rs::models::pull::PullModelRequest;

// Non-streaming pull
let status = ollama
    .pull_model(PullModelRequest::new("llama3.2".to_string(), false))
    .await
    .unwrap();

println!("Pull status: {}", status.message);
```

### Pull with Progress Stream

```rust
use tokio_stream::StreamExt;

let mut stream = ollama
    .pull_model_stream(PullModelRequest::new("llama3.2".to_string(), false))
    .await
    .unwrap();

while let Some(Ok(status)) = stream.next().await {
    println!("{:?}", status.message);
}
```

### Copy and Delete Models

```rust
// Copy a model under a new name (useful for fine-tuning bases)
ollama.copy_model("llama3.2".into(), "llama3.2-copy".into()).await.unwrap();

// Delete a model from disk
ollama.delete_model("llama3.2-copy".into()).await.unwrap();
```

### Create a Model from a Modelfile

```rust
use ollama_rs::models::create::CreateModelRequest;

let status = ollama
    .create_model(CreateModelRequest::path(
        "my-custom-model".into(),
        "/path/to/Modelfile".into(),
    ))
    .await
    .unwrap();

println!("{}", status.message);
```

---

## Embeddings

Embeddings convert text into dense vector representations, enabling semantic search, RAG pipelines, and similarity comparisons.

### Single Embedding

```rust
use ollama_rs::Ollama;
use ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest;

#[tokio::main]
async fn main() {
    let ollama = Ollama::default();

    let request = GenerateEmbeddingsRequest::new(
        "nomic-embed-text:latest".to_string(),
        "The quick brown fox jumps over the lazy dog".into(),
    );

    let res = ollama.generate_embeddings(request).await.unwrap();

    // res.embeddings is a Vec<Vec<f32>>
    println!("Embedding dimension: {}", res.embeddings[0].len());
}
```

### Batch Embeddings

You can embed multiple strings in a single request for efficiency:

```rust
use ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest;

let texts = vec![
    "Why is the sky blue?",
    "What causes rainbows?",
    "How does photosynthesis work?",
];

let request = GenerateEmbeddingsRequest::new(
    "nomic-embed-text:latest".to_string(),
    texts.into(),
);

let res = ollama.generate_embeddings(request).await.unwrap();

for (i, embedding) in res.embeddings.iter().enumerate() {
    println!("Text {}: {} dimensions", i, embedding.len());
}
```

### Computing Cosine Similarity

A common use-case after generating embeddings is finding the most semantically similar item:

```rust
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm_a * norm_b)
}
```

> **Tip:** For production RAG pipelines, use a dedicated embedding model like `nomic-embed-text` rather than a general chat model. It produces 768-dimensional vectors optimised for semantic similarity tasks.

---

## Function Calling & the Coordinator

The `Coordinator` is `ollama-rs`'s high-level system for function/tool calling. It orchestrates the loop between the model, your tools, and the user — automatically routing tool invocations and feeding results back to the LLM.

### Using Built-In Tools

The library ships with ready-made tools: `DDGSearcher` (DuckDuckGo search), `Scraper` (web page fetch), and `Calculator`.

```rust
use ollama_rs::Ollama;
use ollama_rs::coordinator::Coordinator;
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::generation::tools::implementations::{DDGSearcher, Scraper, Calculator};
use ollama_rs::models::ModelOptions;

#[tokio::main]
async fn main() {
    let ollama = Ollama::default();
    let mut history = vec![];

    let mut coordinator = Coordinator::new(
        ollama,
        "qwen2.5:32b".to_string(),
        history,
    )
    .options(ModelOptions::default().num_ctx(16384))
    .add_tool(DDGSearcher::new())
    .add_tool(Scraper {})
    .add_tool(Calculator {});

    let resp = coordinator
        .chat(vec![ChatMessage::user(
            "What is the current price of silver per ounce?".to_string(),
        )])
        .await
        .unwrap();

    println!("{}", resp.message.content);
}
```

The Coordinator will:
1. Send the user message to the model with tool definitions attached.
2. If the model decides to call a tool, execute it locally.
3. Feed the tool result back to the model.
4. Return the final natural-language answer.

> **Model recommendation:** Tool/function calling works best with instruction-tuned models that explicitly support it — `qwen2.5`, `llama3.1+`, and `mistral-nemo` are good choices. Use a `num_ctx` of at least 8192 for reliable tool use; 16384+ is better.

---

## Custom Tools with the `#[function]` Macro

You can turn any async Rust function into a callable tool using the `#[ollama_rs::function]` attribute macro. The doc comment is critical — it becomes the tool description the model reads to decide when and how to use it.

```rust
use ollama_rs::Ollama;
use ollama_rs::coordinator::Coordinator;
use ollama_rs::generation::chat::ChatMessage;

/// Retrieve the current weather for a specified city.
///
/// * city - The name of the city (e.g., "London", "Tokyo", "New York").
#[ollama_rs::function]
async fn get_weather(
    city: String,
) -> Result<String, Box<dyn std::error::Error + Sync + Send>> {
    let url = format!("https://wttr.in/{city}?format=%C+%t");
    let response = reqwest::get(&url).await?.text().await?;
    Ok(response)
}

/// Convert an amount from one currency to another using live exchange rates.
///
/// * amount  - The numeric amount to convert.
/// * from    - The source currency code (e.g., "USD").
/// * to      - The target currency code (e.g., "EUR").
#[ollama_rs::function]
async fn convert_currency(
    amount: f64,
    from: String,
    to: String,
) -> Result<String, Box<dyn std::error::Error + Sync + Send>> {
    // In production, you'd call a real exchange rate API here
    Ok(format!("Converted {amount} {from} to {to}: result would appear here"))
}

#[tokio::main]
async fn main() {
    let ollama = Ollama::default();
    let mut history = vec![];

    let mut coordinator = Coordinator::new(ollama, "qwen2.5:14b".to_string(), history)
        .add_tool(get_weather)
        .add_tool(convert_currency);

    let resp = coordinator
        .chat(vec![ChatMessage::user(
            "What's the weather like in Jakarta right now?".to_string(),
        )])
        .await
        .unwrap();

    println!("{}", resp.message.content);
}
```

### Rules for Custom Tool Functions

- Must be `async`.
- Must return `Result<String, Box<dyn std::error::Error + Sync + Send>>`.
- Parameters must be basic types that map to JSON (`String`, `f64`, `i64`, `bool`).
- The doc comment (`///`) is **required** and used as the tool's description for the model. Be specific about parameter names.

### Structured Output with Function Calling

You can combine tool calls with structured JSON output using `serde`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct WeatherReport {
    city: String,
    condition: String,
    temperature_celsius: f32,
    summary: String,
}

// Configure the coordinator to return structured JSON
let coordinator = Coordinator::new(ollama, "qwen2.5:14b".to_string(), history)
    .add_tool(get_weather)
    .format(serde_json::json!({
        "type": "object",
        "properties": {
            "city": { "type": "string" },
            "condition": { "type": "string" },
            "temperature_celsius": { "type": "number" },
            "summary": { "type": "string" }
        }
    }));
```

---

## Using the Raw HTTP API Directly

If you prefer not to use `ollama-rs` or need more control, you can call the Ollama REST API directly with `reqwest`. This is also useful for understanding what the library does under the hood.

### Setup

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
```

### Basic Chat Request

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    message: Message,
    done: bool,
}

#[tokio::main]
async fn main() {
    let client = Client::new();

    let body = json!({
        "model": "llama3.2",
        "messages": [
            { "role": "user", "content": "Hello! What can you do?" }
        ],
        "stream": false
    });

    let res: ChatResponse = client
        .post("http://localhost:11434/api/chat")
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    println!("{}", res.message.content);
}
```

### Streaming with Raw HTTP

Ollama's streaming API returns newline-delimited JSON (NDJSON). Each line is a partial response that you deserialize and print:

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::{self, AsyncWriteExt};

#[derive(Deserialize)]
struct MessageChunk {
    content: String,
}

#[derive(Deserialize)]
struct StreamChunk {
    message: MessageChunk,
    done: bool,
}

#[tokio::main]
async fn main() {
    let client = Client::new();
    let mut stdout = io::stdout();

    let mut res = client
        .post("http://127.0.0.1:11434/api/chat")
        .json(&json!({
            "model": "llama3.2",
            "messages": [
                { "role": "user", "content": "Explain Rust's borrow checker." }
            ]
        }))
        .send()
        .await
        .unwrap();

    while let Some(chunk) = res.chunk().await.unwrap() {
        if let Ok(parsed) = serde_json::from_slice::<StreamChunk>(&chunk) {
            if !parsed.done {
                stdout
                    .write_all(parsed.message.content.as_bytes())
                    .await
                    .unwrap();
                stdout.flush().await.unwrap();
            }
        }
    }

    println!();
}
```

### Key REST Endpoints

| Endpoint | Method | Description |
|---|---|---|
| `/api/generate` | POST | Single-turn text completion |
| `/api/chat` | POST | Multi-turn chat with message history |
| `/api/embed` | POST | Generate embeddings (batch-capable) |
| `/api/tags` | GET | List all locally installed models |
| `/api/show` | POST | Detailed info about a specific model |
| `/api/pull` | POST | Download a model from the registry |
| `/api/create` | POST | Create a model from a Modelfile |
| `/api/copy` | POST | Copy/rename a model |
| `/api/delete` | DELETE | Remove a model from disk |

---

## OpenAI-Compatible Endpoint

Ollama exposes an OpenAI-compatible API at `/v1/chat/completions`, `/v1/embeddings`, and `/v1/models`. This lets you use any OpenAI-compatible Rust SDK (such as `async-openai`) pointed at your local Ollama instance.

```toml
[dependencies]
async-openai = "0.27"
tokio = { version = "1", features = ["full"] }
```

```rust
use async_openai::{
    config::OpenAIConfig,
    types::{CreateChatCompletionRequestArgs, ChatCompletionRequestUserMessageArgs},
    Client,
};

#[tokio::main]
async fn main() {
    // Point the OpenAI client at your local Ollama
    let config = OpenAIConfig::new()
        .with_api_base("http://localhost:11434/v1")
        .with_api_key("ollama"); // any non-empty string works

    let client = Client::with_config(config);

    let request = CreateChatCompletionRequestArgs::default()
        .model("llama3.2")
        .messages([
            ChatCompletionRequestUserMessageArgs::default()
                .content("Write a fizzbuzz in Rust.")
                .build()
                .unwrap()
                .into(),
        ])
        .build()
        .unwrap();

    let response = client
        .chat()
        .create(request)
        .await
        .unwrap();

    println!(
        "{}",
        response.choices[0].message.content.as_deref().unwrap_or("")
    );
}
```

> **Important:** The model name must exactly match a model you have pulled locally (e.g., `"llama3.2"` not `"gpt-4"`). The OpenAI-compatible endpoint does not support all OpenAI parameters — `logprobs` and `n` are silently ignored.

---

## Error Handling Best Practices

The `ollama-rs` examples use `.unwrap()` for brevity, but production code should handle errors properly.

### Using `anyhow` for Simple Apps

```toml
[dependencies]
anyhow = "1"
```

```rust
use anyhow::{Context, Result};
use ollama_rs::Ollama;
use ollama_rs::generation::completion::GenerationRequest;

async fn ask_ollama(prompt: &str) -> Result<String> {
    let ollama = Ollama::default();

    let res = ollama
        .generate(GenerationRequest::new(
            "llama3.2:latest".to_string(),
            prompt.to_string(),
        ))
        .await
        .context("Failed to generate response from Ollama")?;

    Ok(res.response)
}

#[tokio::main]
async fn main() -> Result<()> {
    let answer = ask_ollama("What is Rust?").await?;
    println!("{answer}");
    Ok(())
}
```

### Using `thiserror` for Libraries

```toml
[dependencies]
thiserror = "1"
```

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OllamaError {
    #[error("Model not available: {model}")]
    ModelNotFound { model: String },

    #[error("Generation failed: {0}")]
    GenerationFailed(#[from] ollama_rs::error::OllamaError),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
}
```

### Checking Model Availability

A useful pattern before making a generation request is to verify the model is available locally:

```rust
async fn model_exists(ollama: &Ollama, model_name: &str) -> bool {
    ollama
        .list_local_models()
        .await
        .map(|models| models.iter().any(|m| m.name.starts_with(model_name)))
        .unwrap_or(false)
}
```

---

## Feature Flags Reference

`ollama-rs` uses Cargo feature flags to keep the binary lean. Enable only what you need:

```toml
[dependencies]
ollama-rs = { version = "0.3.4", features = ["stream", "chat-history"] }
```

| Feature | Description |
|---|---|
| `stream` | Enables streaming generation and streaming model creation/pull |
| `chat-history` | Enables the `send_chat_messages_with_history` helper |
| (default) | Basic generation, chat, model management, and embeddings |

The `Coordinator` and function calling tools are included by default.

---

## Tips & Gotchas

**Context window matters for tool use.** Set `num_ctx` to at least 8192 when using the Coordinator. Tool schemas, function results, and conversation history can quickly exhaust a small context window, causing silent truncation or hallucination.

**Streaming + tool calling caveats.** As of early 2026, Ollama's streaming implementation for tool calls is incomplete — tool call responses do not stream progressively like text content. When doing function calling, prefer `stream: false` at the request level for reliability.

**Match model names exactly.** Whether using the REST API or the OpenAI-compatible endpoint, the `model` field must match the exact name returned by `ollama list` (e.g., `"llama3.2:latest"` vs `"llama3.2"`).

**Use embedding-specific models.** For RAG and semantic search, pull `nomic-embed-text` rather than using a general chat model for embeddings. Dedicated embedding models produce significantly better vector quality for similarity tasks.

**The `#[function]` macro requires descriptive doc comments.** The LLM uses your doc comment to understand when and how to call your tool. Vague or missing comments result in the model failing to invoke tools correctly.

**For reproducible outputs**, set `seed` in your `ModelOptions`. Combined with a low `temperature` (0.0–0.1), this makes generation deterministic — useful for testing and evaluation pipelines.

**Connection pooling.** The `Ollama` struct wraps a `reqwest::Client` which internally maintains a connection pool. Create it once and clone or share it (via `Arc`) rather than constructing a new instance per request.

---

## Further Resources

- [ollama-rs on crates.io](https://crates.io/crates/ollama-rs)
- [ollama-rs GitHub Repository](https://github.com/pepperoni21/ollama-rs)
- [ollama-rs API Docs (docs.rs)](https://docs.rs/ollama-rs/latest/ollama_rs/)
- [Ollama Official REST API Reference](https://github.com/ollama/ollama/blob/main/docs/api.md)
- [Ollama Model Library](https://ollama.com/library)
- [DeepWiki: ollama-rs architecture deep dive](https://deepwiki.com/pepperoni21/ollama-rs)