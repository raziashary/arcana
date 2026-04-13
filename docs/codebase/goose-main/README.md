# Goose Source - Architecture Documentation

> **Project**: `goose-main`  
> **Type**: Rust CLI Application (Workspace)  
> **Workspace Crates**: 9 crates  
> **Total Source Files**: ~1,500+  
> **Generated**: 2025

---

## Table of Contents

1. [Documentation Index](#documentation-index)
2. [Quick Start](#quick-start)
3. [Project Overview](#project-overview)
4. [Workspace Structure](#workspace-structure)
5. [Architecture Patterns](#architecture-patterns)
6. [Key Technologies](#key-technologies)
7. [Reading Order](#reading-order)

---

## Documentation Index

| Document | Purpose | Size |
|----------|---------|------|
| **[README.md](README.md)** | This file - index and navigation | - |
| **[goose-main-codebase-dissected.md](goose-main-codebase-dissected.md)** | Complete architectural overview | ~20KB |
| **[codebase_graph.json](codebase_graph.json)** | Machine-readable dependency graph | ~30KB |
| **[01-agent-system-deep-dive.md](01-agent-system-deep-dive.md)** | Agent system implementation | ~15KB |
| **[02-provider-system.md](02-provider-system.md)** | LLM provider architecture | ~15KB |
| **[03-mcp-integration.md](03-mcp-integration.md)** | MCP integration details | ~15KB |

---

## Quick Start

### For High-Level Overview

Start with **[goose-main-codebase-dissected.md](goose-main-codebase-dissected.md)** for the complete architectural overview.

### For Specific Topics

| If you want to learn about... | Read this document |
|------------------------------|-------------------|
| Agent loop and core logic | [01-agent-system-deep-dive.md](01-agent-system-deep-dive.md) |
| LLM providers (50+ supported) | [02-provider-system.md](02-provider-system.md) |
| MCP client and tool integration | [03-mcp-integration.md](03-mcp-integration.md) |
| Dependencies and file relationships | [codebase_graph.json](codebase_graph.json) |

---

## Project Overview

### What is Goose?

Goose is a **Rust-based CLI application** that provides an agentic AI assistant for software development. It's designed as a workspace with multiple crates:

- **Core Library** (`goose/`) - Agent implementation, providers, configuration
- **CLI** (`goose-cli/`) - Command-line interface
- **MCP** (`goose-mcp/`) - Model Context Protocol integration
- **Server** (`goose-server/`) - HTTP server component
- **ACP** (`goose-acp/`) - Agent Communication Protocol

### Architecture Highlights

```
┌─────────────────────────────────────────────────────────────────────────┐
│                            Goose Workspace                               │
│                                                                          │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐            │
│  │   goose-cli     │  │   goose-server  │  │   goose-sdk     │            │
│  │   (CLI App)     │  │   (HTTP API)    │  │   (SDK)         │            │
│  └────────┬────────┘  └────────┬────────┘  └────────┬────────┘            │
│           │                    │                    │                   │
│           └────────────────────┼────────────────────┘                   │
│                                │                                        │
│                                ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                         goose (Core Library)                      │   │
│  │                                                                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │   │
│  │  │   Agent     │  │  Providers  │  │    Config     │             │   │
│  │  │   (agent.rs)│  │  (50+ LLMs)  │  │   (base.rs)   │             │   │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘             │   │
│  │         │                │                │                    │   │
│  │         └────────────────┼────────────────┘                    │   │
│  │                          │                                      │   │
│  │                          ▼                                      │   │
│  │  ┌─────────────────────────────────────────────────────────┐   │   │
│  │  │              Extension Manager & MCP                     │   │   │
│  │  │         (Tool discovery & execution)                     │   │   │
│  │  └─────────────────────────────────────────────────────────┘   │   │
│  │                                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                │                                        │
│                                ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │              goose-mcp (MCP Protocol Implementation)             │   │
│  │                          (rmcp crate)                           │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Workspace Structure

```
goose-main/
├── Cargo.toml                  # Workspace manifest
├── Cargo.lock                  # Dependency lock
├── crates/
│   ├── goose/                  # Core library (287 items)
│   │   └── src/
│   │       ├── lib.rs           # Main library entry
│   │       ├── agents/
│   │       │   ├── agent.rs     # Main agent (103KB)
│   │       │   ├── extension_manager.rs  # Tool management (83KB)
│   │       │   ├── mcp_client.rs # MCP integration (34KB)
│   │       │   └── ...
│   │       ├── providers/
│   │       │   ├── base.rs      # Provider trait (38KB)
│   │       │   ├── anthropic.rs # Anthropic provider
│   │       │   ├── openai.rs    # OpenAI provider
│   │       │   ├── codex.rs     # OpenAI Codex (48KB)
│   │       │   └── ... (50+ providers)
│   │       ├── config/
│   │       │   ├── base.rs      # Configuration (70KB)
│   │       │   └── ...
│   │       └── ...
│   │
│   ├── goose-cli/              # CLI application (58 items)
│   │   └── src/
│   │       ├── main.rs          # CLI entry
│   │       ├── cli.rs           # CLI implementation (64KB)
│   │       └── ...
│   │
│   ├── goose-mcp/              # MCP protocol (40 items)
│   ├── goose-server/           # HTTP server (46 items)
│   ├── goose-acp/              # Agent Communication Protocol (36 items)
│   ├── goose-acp-macros/       # ACP macros (2 items)
│   ├── goose-sdk/              # SDK (4 items)
│   ├── goose-test/             # Tests (7 items)
│   └── goose-test-support/     # Test utilities (7 items)
│
├── ui/                         # Desktop UI (531 items)
├── documentation/              # Docs (585 items)
├── scripts/                    # Build scripts (31 items)
└── ...
```

### Crate Dependency Graph

```
                    ┌─────────────────┐
                    │   goose-cli     │
                    │   goose-server  │
                    │   goose-sdk     │
                    └────────┬────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │  goose (core)   │
                    │  287 source     │
                    │  files          │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
    ┌──────────────┐ ┌──────────┐ ┌──────────────┐
    │ goose-mcp    │ │ goose-acp│ │ Other deps   │
    │ (rmcp crate) │ │          │ │ (tokio, etc) │
    └──────────────┘ └──────────┘ └──────────────┘
```

---

## Architecture Patterns

### 1. Agent Pattern

**Core**: `crates/goose/src/agents/agent.rs` (2471 lines)

```rust
pub struct Agent {
    provider: SharedProvider,
    config: AgentConfig,
    extension_manager: Arc<ExtensionManager>,
    prompt_manager: Mutex<PromptManager>,
    // ...
}

impl Agent {
    pub async fn reply(
        &self,
        messages: Vec<Message>,
    ) -> Result<BoxStream<'_, Result<Message, anyhow::Error>>> {
        // Agent loop implementation
    }
}
```

**Key Features**:
- Provider-agnostic (works with any LLM provider)
- Extension-based tool system
- Context compaction for long conversations
- Permission management
- Retry logic

### 2. Provider Pattern

**Core**: `crates/goose/src/providers/base.rs` (1161 lines)

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    async fn complete(
        &self,
        model_config: &ModelConfig,
        system: Option<String>,
        messages: Vec<Message>,
        tools: Vec<Tool>,
    ) -> Result<ProviderResponse, ProviderError>;

    async fn stream_complete(
        &self,
        model_config: &ModelConfig,
        system: Option<String>,
        messages: Vec<Message>,
        tools: Vec<Tool>,
    ) -> Result<BoxStream<'_, Result<ProviderStreamChunk, ProviderError>>, ProviderError>;
}
```

**50+ Providers Supported**:
- Anthropic (Claude)
- OpenAI (GPT-4, Codex)
- Azure OpenAI
- Google (Gemini)
- AWS Bedrock
- Local inference (Ollama)
- And many more...

### 3. Extension/Tool Pattern

**Core**: `crates/goose/src/agents/extension_manager.rs` (83468 bytes)

```rust
pub struct ExtensionManager {
    clients: HashMap<String, ExtensionClient>,
    tool_cache: Vec<Tool>,
    // ...
}

pub struct ExtensionConfig {
    pub name: String,
    pub uri: String,
    pub transport: TransportType,
    // ...
}
```

**Features**:
- MCP-based tool discovery
- Dynamic tool loading
- Permission-based tool filtering
- Tool confirmation routing

### 4. MCP Integration Pattern

**Core**: `crates/goose/src/agents/mcp_client.rs` (33828 bytes)

```rust
pub struct MCPClient {
    client: rmcp::Client,
    config: ExtensionConfig,
}

impl MCPClient {
    pub async fn list_tools(&self) -> Result<Vec<Tool>>;
    pub async fn call_tool(&self, request: CallToolRequest) -> Result<CallToolResult>;
}
```

---

## Key Technologies

| Category | Technology | Purpose |
|----------|-----------|---------|
| **Language** | Rust | Primary language |
| **Async Runtime** | Tokio | Async execution |
| **HTTP Client** | reqwest | API calls |
| **Serialization** | serde | JSON handling |
| **CLI** | clap | Command-line parsing |
| **MCP** | rmcp | Model Context Protocol |
| **Tracing** | tracing | Logging & telemetry |
| **Error Handling** | anyhow | Error management |
| **Testing** | tokio-test | Async testing |

---

## File Statistics

### By Crate

| Crate | Files | Purpose |
|-------|-------|---------|
| **goose** | 287 | Core library |
| **goose-cli** | 58 | CLI application |
| **goose-server** | 46 | HTTP server |
| **goose-mcp** | 40 | MCP protocol |
| **goose-acp** | 36 | ACP protocol |
| **goose-acp-macros** | 2 | Macros |
| **goose-sdk** | 4 | SDK |
| **goose-test** | 7 | Tests |
| **goose-test-support** | 7 | Test support |
| **Total** | **487** | Source crates |

### Largest Files

| File | Size | Purpose |
|------|------|---------|
| `goose/src/agents/agent.rs` | 101 KB | Main agent implementation |
| `goose/src/config/base.rs` | 68 KB | Configuration system |
| `goose/src/providers/codex.rs` | 47 KB | OpenAI Codex provider |
| `goose/src/providers/claude_code.rs` | 56 KB | Claude Code provider |
| `goose/src/agents/extension_manager.rs` | 81 KB | Extension management |
| `goose/src/providers/chatgpt_codex.rs` | 42 KB | ChatGPT Codex |
| `goose/src/agents/reply_parts.rs` | 29 KB | Reply handling |
| `goose/src/providers/openai.rs` | 29 KB | OpenAI provider |

---

## Reading Order

### For New Contributors

1. Start with this README
2. Read [goose-main-codebase-dissected.md](goose-main-codebase-dissected.md) for overview
3. Dive into specific topics:
   - Agent system → [01-agent-system-deep-dive.md](01-agent-system-deep-dive.md)
   - Provider system → [02-provider-system.md](02-provider-system.md)
   - MCP integration → [03-mcp-integration.md](03-mcp-integration.md)
4. Reference [codebase_graph.json](codebase_graph.json) for dependencies

### For Quick Reference

| Topic | Document | Section |
|-------|----------|---------|
| Workspace structure | Main docs | Workspace Structure |
| Agent implementation | [01-agent-system-deep-dive.md](01-agent-system-deep-dive.md) | Agent System |
| Provider trait | [02-provider-system.md](02-provider-system.md) | Provider Pattern |
| MCP client | [03-mcp-integration.md](03-mcp-integration.md) | MCP Integration |
| CLI commands | [goose-main-codebase-dissected.md](goose-main-codebase-dissected.md) | CLI Crate |

---

## Key Files Cheat Sheet

| Purpose | File |
|---------|------|
| Library entry | `crates/goose/src/lib.rs` |
| Main agent | `crates/goose/src/agents/agent.rs` |
| Provider trait | `crates/goose/src/providers/base.rs` |
| Extension manager | `crates/goose/src/agents/extension_manager.rs` |
| MCP client | `crates/goose/src/agents/mcp_client.rs` |
| Configuration | `crates/goose/src/config/base.rs` |
| CLI entry | `crates/goose-cli/src/main.rs` |
| CLI implementation | `crates/goose-cli/src/cli.rs` |
| Server entry | `crates/goose-server/src/main.rs` |

---

## Contributing to Documentation

When adding to this documentation:

1. Update the relevant focused document (01-, 02-, 03-)
2. Update this README index if adding new documents
3. Update the main [goose-main-codebase-dissected.md](goose-main-codebase-dissected.md) for high-level changes
4. Update [codebase_graph.json](codebase_graph.json) for dependency changes

---

## Summary

This documentation provides:

- **Main Overview**: [goose-main-codebase-dissected.md](goose-main-codebase-dissected.md)
- **Dependency Graph**: [codebase_graph.json](codebase_graph.json) (machine-readable)
- **Agent System**: [01-agent-system-deep-dive.md](01-agent-system-deep-dive.md)
- **Provider System**: [02-provider-system.md](02-provider-system.md)
- **MCP Integration**: [03-mcp-integration.md](03-mcp-integration.md)

Total documentation size: ~95KB across 6 files covering 487+ source files in 9 crates.
