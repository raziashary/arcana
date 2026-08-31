# Memory System Architecture for an Autonomous AI Development Agent

## 1. Overview

The goal of this system is to build a **persistent memory layer for an autonomous AI development agent**.

The agent should be capable of operating similarly to systems such as Claude Code or Codex: it can inspect a codebase, reason about tasks, modify files, execute commands, test its work, recover from failures, and continue working across multiple sessions.

A conventional RAG system is not sufficient for this use case.

The system needs to distinguish between:

* **Project knowledge** — what exists in the codebase
* **User knowledge** — preferences, requirements, and long-term instructions
* **Agent knowledge** — what the agent has learned through previous work
* **Task knowledge** — decisions, discoveries, failures, and progress related to a specific task
* **Working context** — information relevant to the current execution

The memory system should therefore function as an **external long-term memory for the agent**, while the agent's context window acts as its short-term working memory.

---

# 2. Core Principle

The system should not attempt to put the entire history of the agent into the context window.

Instead:

```text
                    AUTONOMOUS AGENT
                           │
                           ▼
                  ┌─────────────────┐
                  │ Memory Gateway  │
                  └────────┬────────┘
                           │
              ┌────────────┼────────────┐
              ▼            ▼            ▼
          Retrieval      Storage      Updates
              │            │            │
              └────────────┼────────────┘
                           ▼
                    Relevant Memory
                           │
                           ▼
                     Agent Context
```

The agent should retrieve memory **only when it is relevant**.

This keeps the context window small while allowing the agent to accumulate knowledge over long periods of development.

---

# 3. Memory Is Not Just RAG

A traditional RAG system generally looks like:

```text
Documents
   ↓
Chunks
   ↓
Embeddings
   ↓
Vector Database
   ↓
Retriever
   ↓
LLM
```

An autonomous development agent requires something more sophisticated:

```text
                     Agent
                       │
                       ▼
                 Memory Gateway
                       │
          ┌────────────┼────────────┐
          ▼            ▼            ▼
      Episodic      Semantic      Procedural
       Memory        Memory        Memory
          │            │            │
          └────────────┼────────────┘
                       ▼
                Hybrid Retrieval
                       │
                       ▼
                   Reranking
                       │
                       ▼
                Context Builder
                       │
                       ▼
                     Agent
```

RAG should therefore be considered a **retrieval mechanism inside the memory system**, rather than the entire memory system itself.

---

# 4. Types of Memory

## 4.1 Episodic Memory

Episodic memory stores things that happened during previous agent executions.

Examples:

```text
The agent attempted to migrate the database
but the migration failed because the production
schema differs from the local schema.
```

```text
The agent discovered that the authentication
tests must be executed using the integration
environment rather than the local environment.
```

```text
The agent previously fixed a similar WebSocket
reconnection bug in connection_manager.py.
```

Episodic memory answers:

> **"What happened before?"**

---

## 4.2 Semantic Memory

Semantic memory stores facts that the agent has learned.

Examples:

```text
The project uses PostgreSQL.

The API uses JWT authentication.

The frontend is built with React.

The backend uses FastAPI.

The repository uses pytest for testing.
```

Semantic memory answers:

> **"What do I know?"**

---

## 4.3 Procedural Memory

Procedural memory stores knowledge about how something should be done.

Examples:

```text
When modifying database models, create a migration
before modifying the production schema.
```

```text
Run the unit tests before running the integration suite.
```

```text
Frontend components should use the existing
design-system components instead of introducing
new UI primitives.
```

Procedural memory answers:

> **"How should I do this?"**

This type of memory is particularly valuable for autonomous agents because it allows the agent to learn **development patterns and workflows**.

---

## 4.4 Working Memory

Working memory represents information currently relevant to the active task.

For example:

```text
Task:
Implement OAuth authentication.

Current findings:
- OAuth provider is already configured.
- Callback endpoint exists.
- Token refresh is missing.

Current plan:
1. Implement refresh-token handling.
2. Add tests.
3. Run integration suite.
```

Working memory should generally remain temporary and highly task-focused.

It does not necessarily belong in long-term memory.

---

# 5. Memory Lifecycle

A memory should go through a lifecycle rather than being permanently stored immediately.

```text
              Agent Experience
                     │
                     ▼
              Candidate Memory
                     │
                     ▼
              Memory Evaluation
                     │
          ┌──────────┴──────────┐
          ▼                     ▼
      Valuable                Not useful
          │                     │
          ▼                     ▼
       Store                  Discard
          │
          ▼
       Retrieve
          │
          ▼
      Re-evaluate
          │
          ▼
      Update / Merge
```

The agent should ask:

> "Is this information likely to be useful again?"

before creating long-term memory.

---

# 6. Memory Should Be Structured

Avoid storing memories as unstructured blobs whenever possible.

A memory should contain information such as:

```json
{
  "id": "memory_123",
  "type": "procedural",
  "content": "Run integration tests using the staging environment.",
  "scope": "project",
  "project_id": "project_abc",
  "source": "agent_observation",
  "confidence": 0.91,
  "importance": 0.82,
  "created_at": "...",
  "updated_at": "...",
  "last_accessed_at": "...",
  "access_count": 7,
  "expires_at": null
}
```

The exact schema can evolve, but the important idea is that memory should contain **metadata describing its meaning and relevance**.

---

# 7. Memory Scope

Memories should have different scopes.

```text
Global
  │
  ├── User
  │
  ├── Organization
  │
  └── Agent

Project
  │
  ├── Architecture
  ├── Conventions
  ├── Dependencies
  └── Decisions

Task
  │
  ├── Current findings
  ├── Decisions
  ├── Failures
  └── Progress
```

For example:

```text
"User prefers TypeScript"
```

might be global/user memory.

While:

```text
"This project uses Zustand for client state"
```

is project memory.

And:

```text
"OAuth callback currently fails because state
validation is performed twice"
```

is task-specific memory.

Scope is critical because it prevents irrelevant memories from contaminating retrieval.

---

# 8. Retrieval Architecture

Memory retrieval should not rely exclusively on vector similarity.

The recommended architecture is:

```text
                    User / Agent Query
                           │
                           ▼
                    Query Understanding
                           │
              ┌────────────┼────────────┐
              ▼            ▼            ▼
          Semantic       Keyword      Metadata
          Retrieval      Retrieval      Filter
              │            │            │
              └────────────┼────────────┘
                           ▼
                    Candidate Memories
                           │
                           ▼
                       Reranker
                           │
                           ▼
                  Relevance + Importance
                           │
                           ▼
                    Top Memories
                           │
                           ▼
                   Context Builder
                           │
                           ▼
                         Agent
```

---

# 9. Hybrid Retrieval

Semantic search is useful for conceptual similarity.

For example:

```text
Query:
"How did we solve the authentication problem?"

Memory:
"Fixed OAuth callback state validation by removing
duplicate verification from the middleware."
```

The wording doesn't need to match exactly.

However, exact identifiers are extremely important in software development.

For example:

```text
ERR_CONNECTION_RESET
AuthMiddleware
UserService
PR #482
commit 7f83a21
```

Keyword retrieval can be much better for these cases.

Therefore the memory system should combine:

```text
Dense Vector Search
        +
Keyword / BM25 Search
        +
Metadata Filtering
```

---

# 10. Reranking

Initial retrieval should produce a relatively large candidate set.

For example:

```text
Query
  ↓
Hybrid Retrieval
  ↓
50 candidate memories
  ↓
Reranker
  ↓
5–10 relevant memories
```

The reranker should consider more than semantic similarity.

A useful relevance score could conceptually be:

```text
Score =
    semantic_relevance
  + lexical_relevance
  + importance
  + recency
  + access_frequency
  + scope_match
  + task_similarity
  + confidence
```

The exact weighting should be determined experimentally.

---

# 11. Recency and Memory Decay

Not every memory remains equally useful forever.

For example:

```text
"The project uses React 18"
```

may become obsolete.

Meanwhile:

```text
"The authentication architecture uses OAuth"
```

may remain useful for years.

The system should therefore track:

* creation time
* last access
* access frequency
* confidence
* importance
* expiration
* version/context

A memory's retrieval score can incorporate recency without automatically deleting old memories.

Old memories should generally be **deprioritized before they are destroyed**.

---

# 12. Memory Consolidation

Autonomous agents will naturally produce duplicate memories.

For example:

```text
Memory A:
"The project uses PostgreSQL."

Memory B:
"Postgres is the project's database."

Memory C:
"Database is PostgreSQL."
```

These should eventually be consolidated.

```text
              Memory A
                  │
              Memory B ──→ Consolidation ──→ Canonical Memory
                  │
              Memory C
```

Consolidation can:

* merge duplicates
* increase confidence
* resolve contradictions
* update outdated facts
* remove low-value memories

This is analogous to **memory consolidation in cognitive systems**.

---

# 13. Contradiction Handling

Contradictions are inevitable in long-running autonomous systems.

For example:

```text
Memory A:
"The backend uses Flask."

Memory B:
"The backend uses FastAPI."
```

The system should not blindly retrieve both and allow the LLM to decide.

Instead, memories should have provenance:

```text
Memory A
source: repository_scan
timestamp: 2025
confidence: 0.72

Memory B
source: repository_scan
timestamp: 2026
confidence: 0.96
```

The newer, higher-confidence observation can supersede the older one.

The system should maintain relationships such as:

```text
SUPERSEDES
CONTRADICTS
DERIVED_FROM
DUPLICATE_OF
```

---

# 14. Memory Provenance

Every important memory should have a source.

Possible sources include:

```text
user
agent_observation
repository
tool_result
documentation
conversation
test_result
external_source
```

For example:

```json
{
  "content": "The API uses PostgreSQL.",
  "source": {
    "type": "repository",
    "file": "docker-compose.yml",
    "line": 24
  }
}
```

This makes memories auditable.

The agent can therefore distinguish:

```text
"I know this because the user told me."

from:

"I believe this because I observed it in the repository."
```

---

# 15. Memory and the Codebase Are Different

This distinction is extremely important for an autonomous coding agent.

The codebase itself is already a knowledge source.

Therefore:

```text
                 Agent
                   │
          ┌────────┴────────┐
          ▼                 ▼
     Code Retrieval      Memory Retrieval
          │                 │
          ▼                 ▼
    "What exists?"      "What did we learn?"
```

Code retrieval answers:

> What does the current repository contain?

Memory retrieval answers:

> What did we learn about this repository, user, task, or previous attempts?

These systems should interact, but they should not necessarily be the same database.

---

# 16. Memory Gateway

The agent should interact with memory through a controlled interface.

For example:

```text
store_memory()
search_memory()
get_memory()
list_memories()
update_memory()
delete_memory()
```

The gateway becomes responsible for:

* validation
* permissions
* deduplication
* embedding
* retrieval
* ranking
* metadata filtering
* consolidation
* lifecycle management

The agent should not need to understand the underlying database.

---

# 17. MCP Integration

For an MCP-based autonomous agent, memory can be exposed as an MCP server.

Conceptually:

```text
                 Autonomous Agent
                        │
                        │ MCP
                        ▼
                ┌───────────────┐
                │ Memory Server │
                └───────┬───────┘
                        │
          ┌─────────────┼─────────────┐
          ▼             ▼             ▼
       Storage       Retrieval    Consolidation
          │             │             │
          └─────────────┼─────────────┘
                        ▼
                   Memory DB
```

However, the agent should not necessarily receive unrestricted memory-management capabilities.

For example, a read-only agent could receive:

```text
search_memory
get_memory
list_memories
```

while a trusted memory-management component could additionally access:

```text
store_memory
update_memory
delete_memory
```

This creates a useful security boundary.

---

# 18. Autonomous Memory Creation

The agent should not store every observation.

Instead, memory creation should be selective.

After completing a task, the agent can evaluate:

```text
What did I learn?

Is this likely to matter again?

Is this information already known?

Is it project-specific?

Is it temporary?

How confident am I?

Should it replace an existing memory?
```

A memory candidate can then be created:

```text
Observation
    ↓
Importance evaluation
    ↓
Novelty check
    ↓
Duplicate check
    ↓
Confidence estimation
    ↓
Memory creation
```

---

# 19. Example

Suppose the agent receives:

```text
Fix the authentication tests.
```

During execution it discovers:

```text
pytest tests/auth
```

fails because the tests require:

```text
AUTH_TEST_DATABASE_URL
```

After solving the problem, the agent can create:

```json
{
  "type": "procedural",
  "content": "Authentication integration tests require AUTH_TEST_DATABASE_URL.",
  "scope": "project",
  "importance": 0.83,
  "confidence": 0.97,
  "source": "test_execution"
}
```

Later, another task begins:

```text
Fix the authentication tests again.
```

The agent retrieves the memory:

```text
Authentication integration tests require
AUTH_TEST_DATABASE_URL.
```

The agent avoids rediscovering the same information.

This is the primary value of persistent agent memory:

> **The agent should become progressively more effective at working on the same environment.**

---

# 20. Memory Should Reduce Repeated Reasoning

The ultimate metric isn't:

> "How many memories do we have?"

It is:

> **"How much unnecessary work does memory prevent?"**

A successful memory system should reduce:

```text
Repeated exploration
Repeated debugging
Repeated mistakes
Repeated user questions
Repeated architectural discovery
Repeated failed approaches
```

while improving:

```text
Task completion
Consistency
Decision quality
Development speed
Personalization
Long-term autonomy
```

---

# 21. Recommended Initial Architecture

A practical first implementation can be:

```text
                     AUTONOMOUS AGENT
                            │
                            ▼
                    ┌───────────────┐
                    │ Memory Gateway│
                    └───────┬───────┘
                            │
                ┌───────────┴───────────┐
                ▼                       ▼
          Memory Storage          Memory Retrieval
                │                       │
                ▼                       ▼
           PostgreSQL             Hybrid Search
                │                 /           \
                │             Vector          BM25
                │                 \           /
                │                  Reranker
                │                      │
                └──────────────┬───────┘
                               ▼
                         Context Builder
                               │
                               ▼
                              Agent
```

For a first version, PostgreSQL + pgvector is a reasonable architecture because it allows structured metadata, relational queries, and vector search in one system.

The architecture should remain modular enough that the retrieval backend can later be replaced.

---

# 22. Recommended Development Phases

## Phase 1 — Basic Memory

Implement:

```text
store_memory
search_memory
get_memory
list_memories
```

with:

* PostgreSQL
* pgvector
* embeddings
* basic metadata

Focus on correctness.

---

## Phase 2 — Better Retrieval

Add:

```text
hybrid search
metadata filtering
reranking
```

Measure:

```text
Recall@K
MRR
NDCG
```

---

## Phase 3 — Memory Quality

Add:

```text
importance
confidence
recency
deduplication
```

---

## Phase 4 — Memory Consolidation

Add:

```text
duplicate detection
memory merging
contradiction detection
superseding
```

---

## Phase 5 — Autonomous Memory

Allow the agent to automatically identify useful memories.

```text
Agent Experience
       ↓
Memory Candidate
       ↓
Evaluate
       ↓
Store
```

---

## Phase 6 — Advanced Memory

Only after the fundamentals work should the system explore:

* hierarchical memory
* knowledge graphs
* multi-hop retrieval
* temporal reasoning
* agentic retrieval
* memory compression
* learned retrieval policies

---

# 23. Evaluation

The memory system should have its own evaluation suite.

### Retrieval

Measure:

```text
Recall@5
Recall@10
MRR
NDCG
```

### Memory quality

Measure:

```text
Useful memories retrieved
Irrelevant memories retrieved
Duplicate memories
Outdated memories
Contradictory memories
```

### Agent performance

Ultimately measure:

```text
Task success rate
Time to completion
Number of tool calls
Repeated mistakes
Tokens consumed
Number of rediscovered facts
```

The last category is particularly important.

If the memory system is working, the agent should increasingly avoid repeating work it has already done.

---

# 24. The Long-Term Vision

The final architecture should look less like a chatbot with RAG and more like an autonomous cognitive system:

```text
                         ┌───────────────┐
                         │     AGENT     │
                         └───────┬───────┘
                                 │
             ┌───────────────────┼───────────────────┐
             │                   │                   │
             ▼                   ▼                   ▼
        Working Memory       Tool/Environment    Long-Term Memory
             │                   │                   │
             │                   │          ┌────────┼────────┐
             │                   │          ▼        ▼        ▼
             │                   │       Episodic Semantic Procedural
             │                   │          │        │        │
             └───────────────────┴──────────┴────────┴────────┘
                                         │
                                         ▼
                                  Memory Retrieval
                                         │
                                         ▼
                                    Consolidation
                                         │
                                         ▼
                                  Persistent Knowledge
```

The important conceptual shift is:

> **RAG retrieves information. Memory preserves experience.**

For an autonomous development agent, you want both.

The codebase provides the agent with **current external state**.

RAG provides **efficient access to large bodies of information**.

Memory provides **continuity across time**.

Together:

```text
Codebase
   +
RAG
   +
Persistent Memory
   +
Agent Reasoning
   =
Long-running Autonomous Development Agent
```

The objective isn't to give the agent the biggest possible memory.

It is to give the agent the **right memory at the right time**.
