# Architecture Clarifications — Dual-Mode Design

> **Created**: April 8, 2026  
> **Purpose**: Address questions about apparent code duplication between Rust implementation and core-orchestrator submodule

---

## Executive Summary

The DEVELOPMENT_PLAN.md proposes building provider abstraction and MCP client in Rust **even though core-orchestrator already has these via `litellm` and `airs_mcp`**. This is NOT duplication — it's a **dual-mode architecture**.

**Key insight**: AutoDev operates in TWO distinct modes with different execution paths:
- **PA mode** (Personal Assistant) — Rust-native, lightweight, for interactive use
- **Worker mode** — Delegates to core-orchestrator for full intelligence stack

---

## Question 1: Why Duplicate the Provider Layer?

### The Apparent Conflict

**DEVELOPMENT_PLAN.md Section 8** proposes:
```
src-tauri/src/
├── provider/
│   ├── anthropic.rs
│   ├── openai.rs
│   ├── gemini.rs
│   ├── ollama.rs
│   └── model_router.rs
```

**But core-orchestrator already has**:
```
submodules/core-orchestrator/src/llm/
├── providers/
│   ├── litellm_provider.py
│   └── model_registry.py
```

### The Resolution: Dual-Mode Architecture

#### PA Mode (Rust-Native Provider)

**Who uses it:**
- Razi using the desktop app for quick tasks
- Team members asking questions, creating issues
- Anyone using `airsctl -p "prompt"` from CLI

**Why Rust provider:**
```
User sends message
  ↓
Tauri (Rust) provider::anthropic::stream()
  ↓ (direct HTTPS to api.anthropic.com)
Response in ~200ms
```

**Benefits:**
- **100-500ms latency savings** — no HTTP round-trip to localhost:4000
- **Works offline with Ollama** — local models, no internet
- **Survives sidecar failures** — PA mode works even if Python crashes
- **Settings UI testing** — test API keys without starting core-orchestrator
- **Simpler debugging** — fewer moving parts

#### Worker Mode (Python litellm Provider)

**Who uses it:**
- Worker agents running recipes (worker-backend-poll.yaml)
- Supervisor breaking down epics
- QC agent reviewing PRs
- Any long-running autonomous task

**Why delegate to core-orchestrator:**
```
Recipe triggers
  ↓
Tauri → HTTP POST localhost:4000/api/v1/agent/invoke
  ↓
core-orchestrator LivingAgent (21 FSMs)
  ↓
litellm_provider.py (unified provider layer)
  ↓
LLM API + mem0 memory + A2A protocol + context scoring
```

**Needs from core-orchestrator:**
- **A2A protocol** — supervisor ↔ workers coordination (impossible in simple Rust)
- **mem0 semantic memory** — retrieval-augmented generation, long-term context
- **21 finite state machines** — agent lifecycle, tool execution tracking, memory tiers
- **Context scoring** — relevance ranking, smart compaction at 80% window
- **Feedback-driven synopsis** — conversation evolution over time

### Conclusion on Point 1

**No duplication**. Two provider implementations serve two use cases:
- Rust provider = **lightweight, low-latency, offline-capable** for humans
- litellm provider = **full intelligence stack, multi-agent, semantic memory** for workers

---

## Question 2: Why Duplicate the MCP Client?

### The Apparent Conflict

**DEVELOPMENT_PLAN.md Section 8** proposes:
```
src-tauri/src/
├── mcp/
│   ├── client.rs    # MCP client via rmcp crate
│   ├── server.rs    # AIRS MCP server
│   └── config.rs
```

**But core-orchestrator already has**:
```
submodules/core-orchestrator/airs_mcp/
├── client.py              # Universal MCP client
├── client_manager.py      # Connection pooling
├── adapters/
│   ├── stdio.py
│   └── http.py
```

### The Resolution: Dual-Mode Architecture (Again)

#### PA Mode (Rust rmcp Client)

**Why Rust MCP client:**
```
PA user invokes GitHub MCP tool
  ↓
Tauri mcp::client.rs → rmcp crate
  ↓ (stdio or HTTP to github-mcp server)
Tool result in ~50ms
```

**Use cases:**
- Quick GitHub issue creation from Chat tab
- Slack notifications during interactive work
- Settings → test MCP server connections
- Headless CLI with MCP dependencies

#### Worker Mode (Python airs_mcp Client)

**Why delegate to core-orchestrator:**
```
Worker recipe uses GitHub MCP
  ↓
core-orchestrator airs_mcp/client.py
  ↓
Connection pooling (reuse connections across turns)
  ↓
Registry manager (cache capabilities, smart routing)
  ↓
Tool result + memory context + A2A coordination
```

**Needs from airs_mcp:**
- **Connection pooling** — workers reuse MCP connections across long sessions
- **Registry manager** — cache server capabilities, smart routing
- **Integration with mem0** — MCP tool results go into semantic memory
- **A2A-aware** — workers can invoke MCP tools on behalf of supervisor

### Conclusion on Point 2

**No duplication**. Two MCP clients serve two use cases:
- Rust `rmcp` = **simple, direct, low-latency** for PA mode
- Python `airs_mcp` = **pooled, cached, memory-integrated** for Worker mode

Additionally, Rust `mcp::server.rs` implements the **AIRS MCP server** (airs_get_tasks, airs_claim_task, etc.) which is unique to AutoDev — not duplicated.

---

## Question 3: Contradictions Between Plans?

### Documents Checked
- `docs/development/DEVELOPMENT_PLAN.md` (18 sections)
- `docs/autodev-vision/autodev-master-plan.md` (living spec)
- `submodules/core-orchestrator/` (Python codebase)

### Contradictions Found & Resolved

#### ✅ RESOLVED: Tool Execution Split

**Was contradictory:**
- Risk Log Q1 said "Memory tools, MCP tools route through Python"
- But Module Breakdown showed `provider/` and `mcp/` in Rust

**Now consistent:**
- New Section 4.1 (Dual-Mode Architecture) explains the split
- Risk Log Q1 updated to reference Section 4.1
- Module comments clarify: "PA mode only; Worker mode uses core-orchestrator"

#### ✅ RESOLVED: When to Use Sidecars

**Was unclear:**
- Architecture diagram showed sidecars
- But data flow showed Rust → LLM directly
- No explanation of when each path is used

**Now consistent:**
- Section 4.1 defines `SessionMode::PersonalAssistant` vs `SessionMode::WorkerAgent`
- Section 4.2 documents PA mode data flow
- Section 5 documents Worker mode data flow (via core-orchestrator)

#### ✅ CONSISTENT: Git Strategy

- Both plans agree: git2-rs in Rust for all modes (PA and Worker)
- No delegation to core-orchestrator for git operations
- Worktree isolation applies to both modes

#### ✅ CONSISTENT: File I/O Tools

- Both plans agree: file tools (`read_file`, `write_file`, `bash`) run natively in Rust
- No delegation to core-orchestrator for file operations
- Applies to both PA and Worker modes

### Remaining Ambiguities (Minor)

#### 1. Recipe Execution Mode Selection

**Question:** How does a recipe specify it wants Worker mode vs PA mode?

**Proposed resolution:**
```yaml
# In recipe YAML
version: "1.0.0"
execution_mode: worker  # or "pa" for lightweight
```

Default to `worker` for all scheduled recipes.

#### 2. Headless CLI Mode Selection

**Question:** Does `airsctl -p "..."` use PA mode or Worker mode?

**Proposed resolution:**
```bash
airsctl -p "..."              # PA mode (default, fast)
airsctl --worker -p "..."     # Worker mode (full orchestrator)
```

PA mode by default for low latency. Add `--worker` flag when you need A2A, mem0, or FSMs.

---

## Implementation Checklist

### Updates Made

- [x] Add Section 4.1 "Dual-Mode Architecture" to DEVELOPMENT_PLAN.md
- [x] Update Module Breakdown comments to reference dual-mode
- [x] Update Risk Log Q1 resolution
- [x] Add dual-mode explanation to autodev-master-plan.md Section 4.1

### Updates Needed

- [ ] Add `SessionMode` enum to Rust types (`src-tauri/src/session/types.rs`)
- [ ] Add `execution_mode` field to Recipe YAML schema
- [ ] Document CLI flag `--worker` in Phase 5 CLI specification
- [ ] Add mode selection UI in Settings tab (Phase 1)

---

## Summary

**All three points addressed:**

1. **Provider layer** — NOT duplicated. Rust provider for PA mode (low latency), litellm for Worker mode (full intelligence).

2. **MCP client** — NOT duplicated. Rust rmcp for PA mode (simple), airs_mcp for Worker mode (pooled + memory-integrated).

3. **Contradictions** — All resolved. Both plans now consistently describe the dual-mode architecture.

**The key insight:** AutoDev is not "one app" — it's **two execution engines sharing a UI**. The complexity split is intentional and necessary.
