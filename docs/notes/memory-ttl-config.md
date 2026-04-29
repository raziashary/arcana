# mem0 Memory TTL Configuration for AutoDev

**Created**: 2026-04-29  
**Phase**: 7.10  
**Status**: Documentation / deferred Python-side implementation

---

## Background

AutoDev uses mem0 (backed by Qdrant) as the semantic memory store for LivingAgents in
core-orchestrator. Every worker agent (`worker-backend`, `supervisor`, etc.) accumulates
memories across sessions. Without TTL or project scoping, `working_dir` paths and
session-specific facts persist indefinitely and pollute future sessions.

---

## mem0 TTL Mechanisms

### 1. Metadata-Based Expiry (Qdrant)

Qdrant stores memories as vector points with arbitrary payload. mem0 writes a `metadata`
dict alongside each memory. To implement soft-TTL:

```python
from datetime import datetime, timedelta

# When adding a working-dir memory:
expires_at = (datetime.utcnow() + timedelta(hours=24)).isoformat()

mem0_client.add(
    messages=[{"role": "system", "content": f"working_dir: {working_dir}"}],
    agent_id=scoped_agent_id,
    metadata={
        "type": "working_dir",
        "expires_at": expires_at,
        "project_path": working_dir,
    }
)
```

**Note**: Qdrant does NOT auto-delete expired points. A cleanup job is required.

### 2. Cleanup Job (Recommended)

Add to core-orchestrator's startup or a scheduled cron:

```python
from qdrant_client import QdrantClient
from datetime import datetime

def purge_expired_memories():
    client = QdrantClient(host="localhost", port=6333)
    now_iso = datetime.utcnow().isoformat()

    # Scroll through all points and delete expired ones
    # (requires Qdrant 1.7+ for payload filtering on scroll)
    result = client.scroll(
        collection_name="memories",
        scroll_filter={
            "must": [{
                "key": "metadata.expires_at",
                "range": {"lt": now_iso}
            }]
        },
        limit=1000,
    )

    expired_ids = [p.id for p in result[0]]
    if expired_ids:
        client.delete(collection_name="memories", points_selector=expired_ids)
        print(f"[MemoryTTL] Purged {len(expired_ids)} expired memories")
```

### 3. Session-Scoped Namespace (Prevents Contamination)

The most robust solution — see `core-orch-memory-namespace-fix.md`:

```python
# Agent ID format: {slug}_{project_hash}
# Example: "worker-backend_a3f9c1d2"
scoped_agent_id = f"{self.slug}_{project_hash}"
```

With scoped namespaces, old project memories are simply ignored (wrong namespace)
without needing deletion.

---

## Recommended Configuration for AutoDev

Apply all three layers for maximum safety:

| Layer | What it does | Where to implement |
|-------|-------------|-------------------|
| Workspace header injection | Overrides stale recall in context window | ✅ Done — `runner.rs` |
| Project-scoped agent_id | Physically separates memories by project | `living_agent.py` (deferred) |
| 24h TTL on `working_dir` memories | Auto-expires stale paths | `living_agent.py` (deferred) |
| Startup memory purge | Clean slate on project switch | `living_agent.py` or startup hook |

---

## Manual Memory Inspection

To see what a worker currently remembers:

```python
from mem0 import Memory
m = Memory()

results = m.get_all(agent_id="worker-backend")
for r in results:
    print(r["memory"], "—", r.get("metadata", {}).get("expires_at", "no TTL"))
```

To delete all memories for a worker (clean slate):

```python
m.delete_all(agent_id="worker-backend")
```

---

## Qdrant Collection Settings

Configure Qdrant collection with an on-disk index and reasonable quantization for
performance (AutoDev typically stores < 10k vectors):

```python
client.recreate_collection(
    collection_name="memories",
    vectors_config=VectorParams(size=1536, distance=Distance.COSINE),
    optimizers_config=OptimizersConfigDiff(indexing_threshold=5000),
    hnsw_config=HnswConfigDiff(m=16, ef_construct=100),
)
```

---

## Related Files

- `docs/notes/core-orch-memory-namespace-fix.md` — project namespace scoping
- `src-tauri/src/recipes/runner.rs` — Rust-side workspace injection (already done)
- `docs/development/DEVELOPMENT_PLAN.md` — Phase 7.10
