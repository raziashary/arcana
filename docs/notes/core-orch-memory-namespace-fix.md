# Core-Orchestrator: Memory Namespace Fix for Working Directory Contamination

**Created**: 2026-04-29  
**Phase**: 7.10  
**Status**: Rust-side workaround applied; Python-side fix deferred

---

## Problem

mem0's Qdrant vector store persists worker context across sessions. When workers ran
against a previous project directory (e.g. `C:\AIRS Foundry\Backup\BACKUPcoreorchestrator`),
that `working_dir` fact was embedded in the vector store and recalled via semantic
similarity during new sessions. New sessions inherited the stale context because there
is no per-project scoping or TTL on working-directory memories.

**Observed symptom**: After a project switch, workers attempted to write files into the
*old* project directory path instead of the current workspace.

---

## Rust-Side Fix (Applied — Phase 7.10)

`src-tauri/src/recipes/runner.rs` already injects an authoritative `[AUTODEV WORKSPACE]`
header at the start of every `invoke_worker()` call:

```
[AUTODEV WORKSPACE]
working_dir: C:\path\to\current\project
All file operations (read_file, write_file, bash, glob, grep) MUST be resolved
relative to this absolute path unless the user provides another absolute path.
```

This overrides any stale `working_dir` that mem0 may have recalled from previous
sessions. It is sent on **every tick**, not just the first, so the correct path is
always present in the LivingAgent's context window.

---

## Python-Side Fix (Deferred — core-orchestrator change required)

### Option A: Project-Scoped Memory Namespace (Recommended)

Scope each worker's memory to the current project by appending a hash of `working_dir`
to the mem0 `agent_id`.

**File to change**: `core-orchestrator/agents/living_agent.py` (or wherever mem0
`add()` / `search()` is called)

**Current code (approximate)**:
```python
memory_results = mem0_client.search(query=user_message, agent_id=self.slug)
```

**Fixed code**:
```python
import hashlib

def _project_namespace(self, working_dir: str) -> str:
    """Return a stable 8-char hash of the working directory path."""
    return hashlib.md5(working_dir.encode()).hexdigest()[:8]

# Usage — pass project-scoped agent_id to all mem0 calls:
project_id = self._project_namespace(working_dir)
scoped_agent_id = f"{self.slug}_{project_id}"

memory_results = mem0_client.search(query=user_message, agent_id=scoped_agent_id)
mem0_client.add(messages, agent_id=scoped_agent_id)
```

This ensures workers in project A cannot see memories from project B.

### Option B: Purge on Project Switch

When AutoDev detects a workspace change (i.e. `load_workspace_dir()` returns a
different path than on the previous boot), send a cleanup request to core-orchestrator
to delete stale memories:

```python
# core-orchestrator endpoint: POST /api/agents/{slug}/memory/purge
# Rust side: call this from runner.rs when working_dir changes
mem0_client.delete_all(agent_id=worker_slug)
```

---

## mem0 Memory TTL Configuration

mem0 supports per-memory expiry (TTL) in some storage backends. To prevent working
directory memories from accumulating indefinitely:

1. When adding a `working_dir` memory, tag it with a short TTL (e.g. 1 session):
   ```python
   mem0_client.add(
       [{"role": "system", "content": f"working_dir: {working_dir}"}],
       agent_id=scoped_agent_id,
       metadata={"ttl_seconds": 86400}  # 24h
   )
   ```

2. For Qdrant backend: set `expires_at` in the point payload — Qdrant does NOT
   auto-expire points; a periodic cleanup cron is needed.

---

## How to Flush Stale Context Manually

Run in a Python shell from the core-orchestrator directory:

```python
from mem0 import Memory
m = Memory()
for slug in ["supervisor", "qc-agent", "worker-backend", "worker-frontend",
             "worker-mcp", "worker-platform", "worker-cleanup"]:
    m.delete_all(agent_id=slug)
    print(f"Purged memories for {slug}")
```

Or via the AutoDev MCP tool (once implemented):
```
clear_worker_memory(worker_slug="worker-backend")
```

---

## Related Files

- `src-tauri/src/recipes/runner.rs` — `[AUTODEV WORKSPACE]` injection (Rust fix)
- `core-orchestrator/agents/living_agent.py` — mem0 call sites (Python fix location)
- `docs/notes/memory-ttl-config.md` — TTL configuration details
- `docs/development/DEVELOPMENT_PLAN.md` — Phase 7.10
