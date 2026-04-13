# PA Memory System Specification

> **Version**: 1.0  
> **Phase**: 4 — Agent Memory + Subagents + Model Router  
> **Status**: Design Document  
> **Replaces**: Message compaction (sliding window truncation)

---

## 1. Executive Summary

The PA Memory System is AutoDev's three-tier persistent memory architecture that replaces message compaction. Instead of truncating old messages, the PA remembers everything—session history, learned facts, and accumulated wisdom—across all conversations.

**Inspiration**: Claude Code's Auto Memory + Mem0's tiered architecture + Core Orchestrator's living agent memory.

**Key Principle**: The PA never forgets. It learns from every interaction, builds knowledge over time, and applies that knowledge to future tasks.

---

## 2. The Three Memory Tiers

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    PA MEMORY ARCHITECTURE                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐         │
│  │  EPISODIC       │  │   SEMANTIC      │  │  PROCEDURAL     │         │
│  │  (What happened)│  │  (What I know)  │  │ (What I learned)│         │
│  │                 │  │                 │  │                 │         │
│  │ "User asked me  │  │ "User prefers   │  │ "When Firebase  │         │
│  │  to refactor    │  │  early returns  │  │  rate limits,   │         │
│  │  auth on Jan 15"│  │  over nesting"  │  │  reduce batch   │         │
│  │                 │  │                 │  │  size"          │         │
│  │ • Raw sessions  │  │ • User prefs    │  │ • Skills        │         │
│  │ • Chronological │  │ • Facts         │  │ • Patterns      │         │
│  │ • Searchable    │  │ • Relationships │  │ • Corrections   │         │
│  │                 │  │                 │  │                 │         │
│  │ Stored: SQLite  │  │ Stored: SQLite  │  │ Stored: SQLite  │         │
│  │ + mem0 vector │  │ + mem0 vector │  │  (~/.autodev/   │         │
│  │  (memory.db)    │  │  (memory.db)    │  │   memory.db)    │         │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘         │
│           │                    │                    │                     │
│           └────────────────────┼────────────────────┘                     │
│                                ▼                                        │
│                    ┌─────────────────────┐                              │
│                    │   SYSTEM PROMPT     │                              │
│                    │   INJECTION         │                              │
│                    │                     │                              │
│                    │ Relevant memories   │                              │
│                    │ loaded per query    │                              │
│                    └─────────────────────┘                              │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Tier 1: Episodic Memory

### 3.1 Purpose
Raw record of all PA experiences. The "autobiography" of the agent.

### 3.2 What Gets Stored
- **User messages** — Every question, instruction, correction
- **Agent responses** — Every reply, thought process, tool execution
- **Tool calls** — What tools were used, with what parameters, what results
- **Session metadata** — Start time, working directory, model used, cost

### 3.3 Storage Schema
```sql
-- episodic_memories table
CREATE TABLE episodic_memories (
    id TEXT PRIMARY KEY,           -- UUID
    session_id TEXT NOT NULL,      -- Links to chat session
    user_id TEXT NOT NULL,         -- Always "razi" for PA
    agent_id TEXT NOT NULL,        -- "pa" for personal assistant
    
    -- Content
    content TEXT NOT NULL,         -- The actual message/response
    content_type TEXT,             -- "user_message", "agent_response", "tool_call"
    
    -- Chronological indexing
    turn_index INTEGER,            -- 1, 2, 3... (for "third interaction" queries)
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    
    -- Metadata JSON
    metadata JSON,                 -- {
                                   --   "model": "claude-sonnet-4",
                                   --   "thinking": "...",
                                   --   "tool_calls": [...],
                                   --   "cost_usd": 0.023,
                                   --   "working_dir": "/projects/myapp"
                                   -- }
    
    -- Vector search
    embedding BLOB,                -- For semantic search
    
    -- Tags for filtering
    tags TEXT                      -- "session:abc123,user:razi,tier:1"
);

-- Index for fast chronological queries
CREATE INDEX idx_episodic_session_turn ON episodic_memories(session_id, turn_index);
CREATE INDEX idx_episodic_timestamp ON episodic_memories(timestamp DESC);
```

### 3.4 Recall Strategies
1. **Recent Context** — Last N messages (e.g., last 10 turns)
2. **Semantic Search** — Find messages similar to current query
3. **Ordinal Queries** — "What did I say in the third interaction?"
4. **Timeline Queries** — "What happened on January 15th?"

### 3.5 Auto-Summarization
After every session ends (or every 20 turns), generate a session summary:

```json
{
  "session_id": "sess_abc123",
  "summary": "Refactored authentication middleware from Express to Fastify. Removed 3 deprecated files. User prefers early returns.",
  "key_actions": ["refactor", "remove_deprecated", "learned_preference"],
  "files_touched": ["src/auth.ts", "src/middleware/jwt.ts"],
  "decisions_made": ["Migrated from Express to Fastify"],
  "timestamp": "2026-01-15T14:30:00Z"
}
```

---

## 4. Tier 2: Semantic Memory

### 4.1 Purpose
Extracted facts and knowledge about the user and projects. The PA's knowledge base.

### 4.2 What Gets Stored
- **User preferences** — "Razi prefers early returns over nested if/else"
- **Project facts** — "Build command is `pnpm build`"
- **Relationships** — "Backend API depends on PostgreSQL"
- **Conventions** — "Tests live in `__tests__/` directory"

### 4.3 Storage Schema
```sql
-- semantic_memories (facts) table
CREATE TABLE semantic_memories (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,         -- Always "razi"
    
    -- Fact content
    fact_text TEXT NOT NULL,       -- "Razi prefers early returns"
    fact_type TEXT,                -- "preference", "convention", "relationship"
    
    -- Source tracking
    source_session_id TEXT,        -- Which session extracted this
    source_message_id TEXT,        -- Which message
    extraction_confidence FLOAT,   -- 0.0-1.0 (LLM confidence)
    
    -- Categorization
    category TEXT,                 -- "coding_style", "project_setup", "user_pref"
    project_context TEXT,          -- "autodev", "myapp", etc.
    
    -- Temporal
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    last_accessed DATETIME,        -- For LRU eviction if needed
    access_count INTEGER DEFAULT 0, -- Popularity tracking
    
    -- Vector for semantic search
    embedding BLOB,
    
    -- Contradiction handling
    superseded_by TEXT,            -- If this fact was corrected
    is_active BOOLEAN DEFAULT TRUE
);

-- Track fact evolution
CREATE TABLE fact_history (
    fact_id TEXT PRIMARY KEY,
    previous_value TEXT,
    new_value TEXT,
    change_reason TEXT,            -- "user_correction", "auto_extracted"
    changed_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### 4.4 Extraction Process
```python
# Triggered automatically on user messages (not agent responses)
async def extract_facts(user_message: str, session_id: str):
    """
    Use LLM to extract facts from user message.
    Only user messages → prevents circular reasoning.
    """
    prompt = f"""
    Extract facts from this user message. Return JSON array:
    [
      {{
        "fact": "concise statement of fact",
        "category": "preference|convention|relationship|setup",
        "confidence": 0.0-1.0
      }}
    ]
    
    User message: {user_message}
    """
    
    facts = await llm.extract(prompt)
    for fact in facts:
        if fact.confidence > 0.7:
            await store_semantic_memory(fact)
```

### 4.5 Fact Categories (Claude Code-inspired)

| Category | Examples |
|----------|----------|
| **Project Patterns** | Build commands, test conventions, code style, dependency choices |
| **User Feedback** | Corrections, preferences, "don't use X" instructions |
| **Architecture** | Structural decisions, migration dates, tech stack choices |
| **Reference** | Where things live, file locations, API endpoints |

---

## 5. Tier 3: Procedural Memory

### 5.1 Purpose
Skills, workflows, and accumulated wisdom. How the PA approaches tasks.

### 5.2 What Gets Stored
- **Successful patterns** — "For refactoring, first grep for usages"
- **Failure avoidances** — "Don't use Gemini for file paths outside workspace"
- **Tool preferences** — "Prefer `read_file` with line ranges over full file"
- **Workflow learnings** — "When user says 'clean up', they mean remove + format"

### 5.3 Storage: MEMORY.md + SQLite Hybrid

Unlike episodic/semantic (pure SQLite), procedural memory is **UI-editable**:

```
~/.autodev/memory/
├── MEMORY.md           ← Editable index (loaded into UI)
├── debugging.md        ← Topic files (auto-generated + editable)
├── workflow-patterns.md
├── tool-preferences.md
└── ...                 ← Any topic files
```

**SQLite backing** for search:
```sql
-- procedural_memories table (mirrors MEMORY.md content)
CREATE TABLE procedural_memories (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    
    -- Source file reference
    source_file TEXT,              -- "debugging.md"
    section_heading TEXT,          -- "Docker Port Conflicts"
    
    -- Content
    learning_text TEXT NOT NULL,   -- The actual learning/skill
    learning_type TEXT,            -- "skill", "pattern", "avoidance", "preference"
    
    -- Context
    applies_to TEXT,               -- "refactoring", "firebase", "all"
    priority INTEGER DEFAULT 5,    -- 1-10 (for ordering in prompt)
    
    -- Auto-extraction metadata
    source_experience TEXT,          -- JSON ref to episodic memory that taught this
    extraction_date DATETIME,
    
    -- User editing
    is_user_edited BOOLEAN DEFAULT FALSE,
    is_user_created BOOLEAN DEFAULT FALSE,
    
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### 5.4 MEMORY.md Format

```markdown
# PA Memory Index

> Auto-generated and user-editable. Keep under 200 lines for fast loading.

## Project Patterns
- Build: `pnpm build`, dev server: `pnpm dev` (port 3000)
- Tests: `pnpm test` (Vitest + happy-dom)
- Lint: `pnpm lint` (ESLint + Prettier pre-commit hook)

## Architecture
- Monorepo: packages/frontend, packages/api, packages/shared
- API uses Fastify (migrated from Express on 2026-02-14)
- Auth: JWT in httpOnly cookie, not localStorage

## Key Decisions
- [Redis caching strategy](./debugging.md) — decided 2026-03-15
- [Debugging patterns](./debugging.md) — common Docker port conflicts

## User Preferences
- Prefers early returns over nested if/else
- Commit format: `type(scope): desc`
- Yarn/pnpm only — never npm

## Tool Learnings
- **Gemini Path Issue**: Don't use Gemini for files outside working dir
- **Refactoring Pattern**: Always grep first, then read, then edit
- **Firebase**: Has rate limits — check quota before batch operations

## Recent Learnings
- E2E tests require local Redis instance on port 6379
- Stripe webhook endpoint must validate signatures before processing
```

### 5.5 Loading Rules
- **First 200 lines** of MEMORY.md load at session startup
- Topic files (e.g., `debugging.md`) referenced and loaded on demand
- Each entry should be ~1 line, ~150 chars max
- Keep index lean; put details in topic files

---

## 6. Memory Injection into System Prompt

### 6.1 Context Assembly Flow

```rust
// context.rs — System prompt construction

pub async fn build_system_prompt(
    &self,
    mode: PAMode,
    session_id: &str,
    user_query: &str,  // For relevance scoring
) -> String {
    // 1. Base system prompt (static)
    let base = match mode {
        PAMode::FastDevelopment => BASE_SYSTEM_PROMPT,
        PAMode::Review => REVIEW_SYSTEM_PROMPT,
        PAMode::Delegate => DELEGATE_SYSTEM_PROMPT,
    };
    
    // 2. AUTODEV.md (user instructions)
    let autodev_md = self.load_autodev_md().await;
    
    // 3. MEMORY.md index (procedural memory — first 200 lines)
    let memory_md = self.load_memory_md_index().await;
    
    // 4. Dynamic memory retrieval (based on query)
    let relevant_memories = self.memory_service.recall(
        session_id,
        user_query,
        RecallOptions {
            episodic_limit: 5,      // Recent + relevant messages
            semantic_limit: 10,     // Facts about user/project
            procedural_limit: 3,    // Skills relevant to query
        }
    ).await;
    
    // 5. Assemble
    format!(
        r#"{base}

## User Instructions (AUTODEV.md)
{autodev_md}

## PA Learnings (MEMORY.md)
{memory_md}

## Relevant Memories for Current Query
{relevant_memories}

## Recent Session Context (Last 10 messages)
{recent_messages}
"#
    )
}
```

### 6.2 Recall API

```rust
// Rust Tauri command → calls core-orchestrator memory endpoints

#[tauri::command]
pub async fn recall_memories(
    session_id: String,
    query: String,
    options: RecallOptions,
) -> Result<MemoryRecallResult, String> {
    // Call core-orchestrator /memory/recall endpoint
    let result = sidecar_client
        .post("/memory/recall")
        .json(&json!({
            "session_id": session_id,
            "query": query,
            "tiers": ["episodic", "semantic", "procedural"],
            "limits": options,
        }))
        .send()
        .await?
        .json::<MemoryRecallResult>()
        .await?;
    
    Ok(result)
}
```

---

## 7. UI Components

### 7.1 MEMORY.md Sidebar UI

Similar to AUTODEV.md tab:

```
┌─────────────────────────────────────┐
│  📋 MEMORY.md                       │
├─────────────────────────────────────┤
│  ┌─────────────────────────────┐   │
│  │ # PA Memory Index            │   │
│  │                              │   │
│  │ ## Project Patterns          │   │
│  │ - Build: `pnpm build`...     │   │ ← Monaco editor
│  │ ...                          │   │
│  └─────────────────────────────┘   │
│                                    │
│  [💾 Save] [↺ Reset] [📤 Export]  │
│                                    │
│  Topic Files:                      │
│  📄 debugging.md                   │
│  📄 workflow-patterns.md           │
│  📄 tool-preferences.md            │
│  [+ New Topic]                     │
│                                    │
│  Search Memories:                  │
│  [🔍 Search...         ]          │
│                                    │
│  ─────────────────────────────────  │
│  Stats: 47 facts | 12 skills       │
│  Last updated: 2 min ago           │
└─────────────────────────────────────┘
```

### 7.2 Memory Search Tool for PA

The PA can search its own memory via tool:

```rust
// builtin_tools/search_memory.rs

pub struct SearchMemoryTool;

#[derive(Serialize, Deserialize)]
pub struct SearchMemoryInput {
    pub query: String,
    pub memory_tier: Option<String>, // "episodic", "semantic", "procedural", "all"
    pub limit: Option<i32>,
    pub time_range: Option<String>,  // "today", "week", "month", "all"
}

impl Tool for SearchMemoryTool {
    fn name(&self) -> &str {
        "search_memory"
    }
    
    fn description(&self) -> &str {
        r#"Search PA's persistent memory for relevant information.
        
        Use this when:
        - User refers to something from a previous session
        - You need to recall user preferences or project conventions
        - You want to avoid repeating past mistakes
        
        Returns: Relevant memories with timestamps and confidence scores.
        "#
    }
    
    async fn call(&self, input: SearchMemoryInput) -> ToolResult {
        let results = memory_service
            .search(&input.query, input.memory_tier, input.limit)
            .await?;
        
        Ok(ToolOutput::Json(json!({
            "query": input.query,
            "results": results,
            "total_found": results.len(),
        })))
    }
}
```

---

## 8. Integration with Core Orchestrator

### 8.1 Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        AUTODEV (Tauri)                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐    │
│  │  Chat UI    │  │ MEMORY.md   │  │  Memory Service     │    │
│  │             │  │  Sidebar    │  │  (Rust wrapper)     │    │
│  └──────┬──────┘  └─────────────┘  └──────────┬──────────┘    │
│         │                                      │               │
│         └──────────────────────────────────────┘               │
│                            │                                    │
│                     HTTP/gRPC                                  │
│                            │                                    │
└────────────────────────────┼────────────────────────────────────┘
                             │
┌────────────────────────────┼────────────────────────────────────┐
│               CORE ORCHESTRATOR (Python Sidecar)                │
│                            │                                    │
│  ┌─────────────────────────┴─────────────────────────┐          │
│  │              Context Service                       │          │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐  │          │
│  │  │  Episodic   │ │  Semantic   │ │  Procedural │  │          │
│  │  │   Memory    │ │   Memory    │ │   Memory    │  │          │
│  │  └──────┬──────┘ └──────┬──────┘ └──────┬──────┘  │          │
│  └─────────┼──────────────┼──────────────┼───────────┘          │
│            │              │              │                       │
│            └──────────────┼──────────────┘                       │
│                           │                                     │
│              ┌────────────┴────────────┐                        │
│              │     Memory MCP Client    │                        │
│              │  (connects to mem0/       │                        │
│              │   custom provider)       │                        │
│              └────────────┬────────────┘                        │
└───────────────────────────┼───────────────────────────────────────┘
                            │
┌───────────────────────────┼───────────────────────────────────────┐
│          MEMORY MCP SERVER (or direct SQLite)                     │
│                           │                                        │
│              ┌────────────┴────────────┐                         │
│              │      SQLite + Vector      │                         │
│              │   (~/.autodev/memory.db) │                         │
│              └───────────────────────────┘                         │
└────────────────────────────────────────────────────────────────────┘
```

### 8.2 API Endpoints

Core orchestrator exposes:

```python
# api_gateway/routes/memory_routes.py

@router.post("/memory/recall")
async def recall_memories(request: RecallRequest):
    """
    Recall memories from all tiers based on query relevance.
    """
    context_service = get_context_service()
    
    # Parallel recall from all tiers
    episodic = await context_service.episodic.recall(
        user_id=request.user_id,
        session_id=request.session_id,
        query=request.query
    )
    
    semantic = await context_service.semantic.recall(
        user_id=request.user_id,
        query=request.query
    )
    
    procedural = await context_service.procedural.recall(
        query=request.query
    )
    
    return {
        "episodic": episodic[:request.limits.episodic],
        "semantic": semantic[:request.limits.semantic],
        "procedural": procedural[:request.limits.procedural],
        "combined_relevance_score": calculate_combined_scores(...)
    }

@router.post("/memory/store/episodic")
async def store_episodic(request: StoreEpisodicRequest):
    """Store a message/response to episodic memory."""
    ...

@router.post("/memory/extract")
async def extract_semantic(request: ExtractRequest):
    """Trigger fact extraction from user message."""
    ...

@router.get("/memory/procedural")
async def get_procedural_memory():
    """Get full MEMORY.md content for UI."""
    ...

@router.post("/memory/procedural")
async def update_procedural_memory(request: UpdateProceduralRequest):
    """Update MEMORY.md from UI edit."""
    ...
```

---

## 9. Data Flow: A Complete Session

```
1. SESSION START
   ├── Load AUTODEV.md (user instructions)
   ├── Load MEMORY.md index (first 200 lines)
   └── Query: "What do I know about this project?"
       └── Recall semantic memories → "Build is pnpm"

2. USER MESSAGE: "Refactor the auth middleware"
   ├── Store to EPISODIC: "Refactor auth middleware"
   ├── Extract SEMANTIC: None (instruction, not fact)
   └── Inject into context:
       ├── AUTODEV.md
       ├── MEMORY.md index
       └── Relevant semantic: "Uses Fastify now"

3. AGENT RESPONSE: (uses tools, generates code)
   ├── Store to EPISODIC: Response + tool calls
   └── Background: No semantic extraction (agent response)

4. USER CORRECTION: "Use early returns, not nesting"
   ├── Store to EPISODIC: "Use early returns..."
   ├── Extract SEMANTIC: {
   │   fact: "User prefers early returns over nesting",
   │   category: "preference",
   │   confidence: 0.95
   │ }
   └── Store SEMANTIC: "User prefers early returns..."

5. LATER SESSION: "Fix the login bug"
   ├── Inject semantic: "User prefers early returns..."
   └── Agent generates code with early returns ✓

6. SESSION END (or every 20 turns)
   ├── Generate session summary
   ├── Store summary to episodic (as "session_summary" type)
   └── If significant learnings → add to MEMORY.md
```

---

## 10. Migration from Compaction

### 10.1 Current State (Phase 1-3)
- Messages stored in SQLite `sessions` table as JSON blob
- Old messages truncated via sliding window (compaction)
- No persistent memory across sessions

### 10.2 Migration Path
```rust
// One-time migration script

pub async fn migrate_sessions_to_memory() -> Result<()> {
    let sessions = db.get_all_sessions().await?;
    
    for session in sessions {
        // 1. Convert old messages to episodic memories
        for (i, message) in session.messages.iter().enumerate() {
            memory_service.store_episodic(
                session_id: &session.id,
                turn_index: i + 1,
                content: &message.content,
                content_type: if message.is_user { "user" } else { "agent" },
                metadata: json!({
                    "migrated_from": "session_v1",
                    "original_timestamp": message.timestamp
                })
            ).await?;
        }
        
        // 2. Generate session summary (if > 5 messages)
        if session.messages.len() > 5 {
            let summary = llm.summarize_session(&session.messages).await?;
            memory_service.store_episodic_summary(
                session_id: &session.id,
                summary: &summary
            ).await?;
        }
    }
    
    Ok(())
}
```

---

## 11. Success Metrics

| Metric | Target |
|--------|--------|
| **Episodic recall accuracy** | >90% for "what happened" queries |
| **Semantic extraction precision** | >80% confidence threshold |
| **Memory injection → better responses** | User confirms PA "remembered" preference |
| **Query → relevant memory latency** | <500ms for combined recall |
| **MEMORY.md user edits** | At least 1 user edit per week (engagement) |

---

## 12. References

1. **Claude Code Auto Memory** — `docs/reference/claudecodememoryexplained.md`
2. **Core Orchestrator Memory** — `submodules/core-orchestrator/src/airs/memory/`
   - `episodic.py` — Raw experience storage
   - `semantic.py` — Fact extraction
   - `procedural.py` — Skills and learnings
3. **Context Service** — `submodules/core-orchestrator/api_gateway/services/context_service.py`
4. **Memory MCP Server** — `submodules/memory-mcp-server/src/`

---

## Appendix A: Database Schema (Complete)

```sql
-- ~/.autodev/memory.db

-- Episodic: Raw experiences
CREATE TABLE episodic_memories (...);  -- See §3.3

-- Semantic: Facts and knowledge
CREATE TABLE semantic_memories (...);   -- See §4.3
CREATE TABLE fact_history (...);          -- See §4.3

-- Procedural: Skills and learnings
CREATE TABLE procedural_memories (...); -- See §5.3

-- Session summaries (for fast context loading)
CREATE TABLE session_summaries (
    session_id TEXT PRIMARY KEY,
    summary_text TEXT NOT NULL,
    key_actions JSON,
    files_touched JSON,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Memory access analytics (for debugging)
CREATE TABLE memory_access_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    query TEXT,
    tier TEXT,
    results_count INTEGER,
    latency_ms INTEGER,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

---

## Appendix B: Memory Service Interface (Rust)

```rust
// src-tauri/src/memory/mod.rs

pub struct MemoryService {
    db: SqlitePool,
    sidecar: SidecarClient,
}

impl MemoryService {
    /// Store user message to episodic memory
    pub async fn store_user_message(
        &self,
        session_id: &str,
        content: &str,
        metadata: Option<Value>,
    ) -> Result<String>; // Returns memory_id
    
    /// Store agent response to episodic memory
    pub async fn store_agent_response(
        &self,
        session_id: &str,
        content: &str,
        thinking: Option<&str>,
        tool_calls: Option<Vec<ToolCall>>,
    ) -> Result<String>;
    
    /// Recall memories from all tiers
    pub async fn recall(
        &self,
        session_id: &str,
        query: &str,
        options: RecallOptions,
    ) -> Result<MemoryRecallResult>;
    
    /// Get MEMORY.md content for UI
    pub async fn get_memory_md(&self) -> Result<String>;
    
    /// Update MEMORY.md from UI edit
    pub async fn update_memory_md(&self, content: &str) -> Result<()>;
    
    /// Generate session summary (background task)
    pub async fn summarize_session(&self, session_id: &str) -> Result<String>;
}
```

---

*End of PA Memory System Specification*
