# AutoDev Startup Sequence & Agent Persistence Analysis

> **Date:** April 20, 2026  
> **Purpose:** Analyze startup toast notifications flow and agent UUID persistence issue  
> **Based on:** Live logs from `startingloading.txt` and codebase analysis  
> **Status:** ✅ **IMPLEMENTED** — both Part 1 and Part 2 shipped on April 20, 2026. See the [Implementation Log](#implementation-log) at the bottom for a file-by-file diff and verification results.

---

## Part 1: Startup Toast Notification System

### Overview
The AutoDev application startup is a multi-phase process involving:
1. Rust application initialization (logging, recipes, skills)
2. Sidecar process spawning (memory-mcp-server, core-orchestrator)
3. Health check verification
4. Worker agent registration/creation
5. Recipe scheduler activation

### Startup Sequence Timeline (from logs)

```
T+0.000s    AutoDev logging initialised
            ↓
T+0.004s    Recipes initialized
            ↓
T+0.008s    16 skills loaded
            ↓
T+1.146s    [RecipeScheduler] Starting worker 'supervisor'
            [RecipeScheduler] Starting worker 'qc-agent'
            [RecipeScheduler] Starting worker 'worker-backend'
            [RecipeScheduler] Starting worker 'worker-frontend'
            [RecipeScheduler] Starting worker 'worker-platform'
            [RecipeScheduler] Starting worker 'worker-mcp'
            [RecipeScheduler] Starting worker 'worker-cleanup'
            ↓
T+2.985s    Health checks begin (fail initially - expected)
            [OrchestratorClient] memory-mcp-server health check failed
            [OrchestratorClient] health check failed (core-orchestrator)
            ↓
T+5.012s    [SidecarManager] Spawning memory-mcp-server
            ↓
T+7.012s    [memory-mcp] Signal handlers registered
            [memory-mcp] Mem0 client initializing
            [memory-mcp] Qdrant vector store initializing
            ↓
T+8.261s    [memory-mcp] Server running on http://0.0.0.0:8000
            [SidecarManager] memory-mcp-server is ready on :8000
            ↓
T+8.262s    [SidecarManager] Spawning core-orchestrator
            ↓
T+8.844s    [core-orch] API Gateway logging configured
            [core-orch] GatewayContext initializing
            [core-orch] 21 routers discovered
            ↓
T+10.985s   [core-orch] A2A Server Manager initialized
            [core-orch] Loading existing agent configs...
            ↓
T+14.244s   [core-orch] Registering agents: supervisor, qc-agent, worker-*
            [core-orch] LivingAgent instances created
            [core-orch] MemoryCore initialized for each agent
            ↓
T+15.432s   [core-orch] All agents registered (25 A2A servers running)
            [core-orch] Recipe workers scheduling...
            ↓
T+~20s      ✅ Application fully ready
```

### Proposed Toast Sequence

| Phase | Toast Message | Trigger Event (from logs) | Estimated Duration |
|-------|---------------|---------------------------|-------------------|
| 1 | "Initializing AutoDev..." | `AutoDev logging initialised` | 0.5s |
| 2 | "Loading 16 skills..." | `[SkillLoader] loaded 16 skills total` | 0.5s |
| 3 | "Starting Memory MCP Server..." | `[SidecarManager] Spawning memory-mcp-server` | 3-4s |
| 4 | "Connecting to vector database..." | `[memory-mcp] Using Memory() with explicit configuration` | 2s |
| 5 | "Starting Core Orchestrator..." | `[SidecarManager] Spawning core-orchestrator` | 3-4s |
| 6 | "Loading 21 API routers..." | `[core-orch] Registered 21 routers` | 1s |
| 7 | "Initializing agent registry..." | `[core-orch] Loading agent config from:` | 2s |
| 8 | "Creating LivingAgents..." | `[core-orch] LivingAgent '{id}' born with memory consciousness` | 3-4s |
| 9 | "Starting worker fleet..." | `[RecipeScheduler] Starting worker '{name}'` | 2s |
| 10 | "✅ AutoDev is ready" | `[SidecarManager] fleet_is_ready() = true` | - |

### Implementation Approach

#### Backend: Add Tauri Events

Modify `src-tauri/src/sidecar/mod.rs` to emit events:

```rust
// Add to spawn_log_router when key patterns are detected
fn spawn_log_router_with_events<R: std::io::Read + Send + 'static>(
    reader: R,
    log_file: PathBuf,
    prefix: &'static str,
    app_handle: tauri::AppHandle,  // Add this
) {
    std::thread::spawn(move || {
        let mut file = /* ... */;
        let reader = BufReader::new(reader);
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    // Emit Tauri event for toast consumption
                    app_handle.emit("sidecar:log", json!({
                        "source": prefix,
                        "message": l,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    })).ok();
                    
                    // Log parsing for structured events
                    if l.contains("Memory MCP Server is running") {
                        app_handle.emit("startup:progress", {
                            "phase": "memory_ready",
                            "message": "Memory MCP Server is ready"
                        }).ok();
                    }
                    // ... other patterns
                    
                    info!("[{prefix}] {l}");
                    let _ = writeln!(file, "{l}");
                }
                Err(_) => break,
            }
        }
    });
}
```

#### Frontend: Toast Store

Create `src/stores/startupStore.ts`:

```typescript
import { create } from 'zustand';
import { listen } from '@tauri-apps/api/event';

interface StartupPhase {
  id: string;
  message: string;
  status: 'pending' | 'active' | 'complete' | 'error';
  progress?: number;
}

interface StartupState {
  phases: StartupPhase[];
  isComplete: boolean;
  currentPhase: string | null;
  subscribeToEvents: () => Promise<() => void>;
}

const startupPhases: StartupPhase[] = [
  { id: 'init', message: 'Initializing AutoDev...', status: 'pending' },
  { id: 'skills', message: 'Loading 16 skills...', status: 'pending' },
  { id: 'memory_mcp', message: 'Starting Memory MCP Server...', status: 'pending' },
  { id: 'vector_db', message: 'Connecting to vector database...', status: 'pending' },
  { id: 'core_orch', message: 'Starting Core Orchestrator...', status: 'pending' },
  { id: 'routers', message: 'Loading 21 API routers...', status: 'pending' },
  { id: 'agents', message: 'Initializing agent registry...', status: 'pending' },
  { id: 'living_agents', message: 'Creating LivingAgents...', status: 'pending' },
  { id: 'workers', message: 'Starting worker fleet...', status: 'pending' },
  { id: 'ready', message: '✅ AutoDev is ready', status: 'pending' },
];

export const useStartupStore = create<StartupState>((set, get) => ({
  phases: startupPhases,
  isComplete: false,
  currentPhase: null,
  
  subscribeToEvents: async () => {
    const unlisten = await listen('startup:progress', (event) => {
      const { phase, message } = event.payload as any;
      
      set((state) => {
        const newPhases = state.phases.map((p) => {
          if (p.id === phase) return { ...p, status: 'active' };
          if (p.status === 'active') return { ...p, status: 'complete' };
          return p;
        });
        return { 
          phases: newPhases, 
          currentPhase: phase,
          isComplete: phase === 'ready'
        };
      });
    });
    
    return unlisten;
  }
}));
```

#### UI: Loading Overlay

Create `src/components/startup/StartupOverlay.tsx`:

```tsx
export function StartupOverlay() {
  const { phases, isComplete, currentPhase } = useStartupStore();
  
  if (isComplete) return null;
  
  return (
    <div className="fixed inset-0 bg-black/80 z-50 flex items-center justify-center">
      <div className="bg-gray-900 rounded-lg p-8 w-96">
        <h2 className="text-xl font-bold text-white mb-4">Starting AutoDev</h2>
        <div className="space-y-2">
          {phases.map((phase) => (
            <div 
              key={phase.id}
              className={`flex items-center gap-2 text-sm ${
                phase.status === 'active' ? 'text-blue-400' :
                phase.status === 'complete' ? 'text-green-400' :
                'text-gray-500'
              }`}
            >
              {phase.status === 'active' && <Loader className="w-4 h-4 animate-spin" />}
              {phase.status === 'complete' && <Check className="w-4 h-4" />}
              {phase.status === 'pending' && <Circle className="w-4 h-4" />}
              <span>{phase.message}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
```

---

## Part 2: Agent UUID Persistence Issue

### Problem Statement

**Current Behavior:** Every time the application restarts, new agent YAML files are created with different ULID suffixes, resulting in:
- Multiple stale agent files accumulating (`worker-backend-8mhrv0b1.yaml`, `worker-backend-9ehggj75.yaml`, etc.)
- Loss of agent identity continuity (each "new" agent gets a fresh MemoryCore with no history)
- Pollution of `config/agents/` directory
- Memory session loss for users (agents don't remember previous interactions)

**Evidence from Logs:**

```
# Multiple backend workers from previous runs - all loaded on startup
T+14.244s  Loading: worker-backend-8mhrv0b1.yaml
T+14.244s  Loading: worker-backend-9ehggj75.yaml  ← OLD (previous session)
T+14.244s  Loading: worker-backend-srfb83a6.yaml  ← OLD (previous session)

# Multiple frontend workers - same pattern
T+14.244s  Loading: worker-frontend-b04tzws8.yaml
T+14.244s  Loading: worker-frontend-d93p7g7s.yaml  ← OLD
T+14.244s  Loading: worker-frontend-p9t98jab.yaml  ← OLD

# Auto-register attempts to recreate (with NEW IDs because UUID always changes)
T+15.432s  Agent worker-backend-8mhrv0b1 not found in registry...
T+15.433s  Creating new agent: worker-backend-x9k2m7p4  ← NEW UUID AGAIN
```

**Root Cause:** The `generate_agent_id()` function in `api_gateway/modules/agents/service.py` uses `ulid()` which generates a **new timestamp-based unique ID on every single call**:

```python
def generate_agent_id(name: str) -> str:
    slug = name.lower().strip()
    slug = re.sub(r'[^a-z0-9]+', '-', slug)
    full_ulid = ulid()  # ← THIS IS THE PROBLEM: NEW ID EVERY TIME
    ulid_suffix = full_ulid[-8:].lower()
    return f"{slug}-{ulid_suffix}"
```

When the app restarts:
1. Auto-register scans `config/agents/` and finds ALL historical files
2. For each file, it checks if already registered in `AgentRegistry`
3. Files with old UUIDs aren't found (different ID = different registry key)
4. System tries to create "new" agents → NEW UUID generated → NEW files created
5. Result: Agent proliferation, orphaned memory sessions, wasted disk space

---

### Solution Analysis

We researched two primary approaches to fix this:

#### Solution 1: Deterministic Hash-Based IDs (Fixed Suffix)

Replace `ulid()` with a hash of the agent name:

```python
import hashlib

def generate_agent_id(name: str) -> str:
    """
    Generate deterministic agent ID from name hash.
    Same name always produces the same ID.
    """
    slug = name.lower().strip()
    slug = re.sub(r'[^a-z0-9]+', '-', slug)
    slug = slug.strip('-')
    
    # Deterministic 8-char suffix from SHA256 hash
    hash_bytes = hashlib.sha256(slug.encode()).digest()
    suffix = hash_bytes[:4].hex()[:8]
    
    return f"{slug}-{suffix}"

# Example:
# "worker-backend" → always "worker-backend-a7f3c8d2"
```

**Pros:**
- Deterministic: Same input → Same ID every time
- Survives restarts: File `worker-backend-a7f3c8d2.yaml` is found, reused, updated
- No stale file accumulation

**Cons:**
- Still has suffix (just deterministic instead of random)
- File name is `worker-backend-{hash}.yaml` not `worker-backend.yaml`
- Need to handle transition from old ULID-based files

#### Solution 2: No UUID Suffix At All (Clean Name)

Simply return the slugified name directly:

```python
def generate_agent_id(name: str) -> str:
    """
    Generate agent ID from persona name only.
    No UUID/hash suffix - clean, readable, deterministic.
    """
    slug = name.lower().strip()
    slug = re.sub(r'[^a-z0-9]+', '-', slug)
    slug = slug.strip('-')
    
    return slug  # ← Just the name, no suffix

# Examples:
# "worker-backend" → "worker-backend"
# "Supervisor Agent" → "supervisor-agent"
# "QC Agent" → "qc-agent"
```

**Pros:**
- Clean, human-readable agent IDs
- File name matches exactly: `worker-backend.yaml`
- Registry key is exactly the worker slug
- Intuitive and debuggable

**Cons:**
- No "built-in" duplicate prevention at ID generation level

---

### Final Solution: No UUID (Clean Name-Based IDs)

**After researching the core-orchestrator codebase, we determined NO UUID is the superior solution.**

#### Why No UUID is Better Than Hash-Based

**1. Registry Already Enforces Uniqueness**

The `AgentRegistry` in `src/airs/agent/registry.py:95-97` already prevents duplicate names:

```python
@classmethod
def register(cls, name: str, agent_card: AgentCard, func: Callable, metadata: AgentMetadata):
    registry = cls._get_registry()
    with registry.lock:
        if name in registry.agents:
            logger.warning(f"Agent {name} is already registered")
            raise ValueError(f"Agent {name} is already registered")
        # ... register
```

The registry is a **unique keyed store** (`Dict[str, AgentData]`). You literally cannot register two agents with the same name. The ULID suffix was a workaround that allowed duplicates — which is the bug we're fixing.

**2. Natural Duplicate Prevention at API Level**

The `create_agent` method in `api_gateway/modules/agents/service.py:516` already checks for duplicates:

```python
async def create_agent(self, agent_data: AgentCreate, user: Dict[str, Any]) -> AgentResponse:
    agent_id = generate_agent_id(agent_data.name)  # "worker-backend"
    
    # Check if agent already exists
    writer = AgentConfigWriter()
    if writer.config_exists(agent_id):  # Checks if worker-backend.yaml exists
        raise ValueError(f"Agent '{agent_id}' already exists. Use update endpoint to modify it.")
```

If someone tries to create a second "worker-backend", they get a clear error. This is **exactly the protection you want**.

**3. True Persistence**

With clean naming:
```
worker-backend → ID: "worker-backend" → File: worker-backend.yaml → Registry key: "worker-backend"
```

On restart:
1. `auto_register_config_metadata_only()` scans `config/agents/worker-backend.yaml`
2. Calls `AgentRegistry.get_agent("worker-backend")` 
3. If found → skips (already registered)
4. If not found → registers from config

**No new files. No lost memory. No proliferation.**

**4. Architecture Alignment**

The fleet architecture (supervisor, qc-agent, worker-backend, worker-frontend, etc.) is designed as **singleton roles**. You don't WANT multiple worker-backends — that would break the Supervisor's delegation model. The no-UUID approach enforces this architecturally.

---

### Implementation: Files to Change

#### File 1: `api_gateway/modules/agents/service.py`

**Change:** Modify `generate_agent_id()` to return clean slugified name

```python
def generate_agent_id(name: str) -> str:
    """
    Generate agent ID from persona name.
    
    Returns clean slugified name WITHOUT UUID suffix.
    This ensures agent persistence across restarts:
    - Same name always produces same ID
    - File is overwritten (not duplicated)
    - Registry key matches exactly
    
    Examples:
        "Layla Soft" → "layla-soft"
        "Worker Backend" → "worker-backend"
        "QC Agent" → "qc-agent"
    
    Duplicate prevention is handled by:
    - AgentRegistry (raises ValueError if name exists)
    - AgentConfigWriter.config_exists() check in create_agent()
    
    Args:
        name: Human persona name
        
    Returns:
        Clean agent ID (slugified name only)
    """
    slug = name.lower().strip()
    slug = re.sub(r'[^a-z0-9]+', '-', slug)
    slug = slug.strip('-')
    
    return slug  # No UUID/hash suffix - clean name
```

**Also update** `create_agent()` method to use `overwrite=True` for updates:

```python
async def create_agent(self, agent_data: AgentCreate, user: Dict[str, Any]) -> AgentResponse:
    from src.airs.config import AgentConfigWriter, create_agent_config_from_request
    
    agent_id = generate_agent_id(agent_data.name)  # "worker-backend"
    
    writer = AgentConfigWriter()
    
    # Check if agent already exists
    if writer.config_exists(agent_id):
        raise ValueError(
            f"Agent '{agent_id}' already exists. "
            f"Use PUT /api/agents/{agent_id}/config to update, "
            f"or DELETE then recreate."
        )
    
    logger.info(f"Creating new persistent agent: {agent_id}")
    
    # Create config (AgentConfig.id will be "worker-backend")
    agent_config = create_agent_config_from_request(
        agent_id=agent_id,  # "worker-backend"
        name=agent_data.name,
        job_title=agent_data.job_title,
        # ...
    )
    
    # Write YAML - results in config/agents/worker-backend.yaml
    config_path = writer.write_config(agent_config, overwrite=False)
    
    # ... rest of method
```

#### File 2: `src/airs/config/agent_schema.py` (if ID validation exists)

**Verify:** Ensure `AgentConfig.id` field accepts plain names without suffix requirements. No changes needed if using standard string validation.

#### File 3: Migration Script (One-Time Cleanup)

**Create:** `scripts/migrate_to_clean_ids.py`

```python
"""
One-time migration: Consolidate UUID-based agent files to clean names.
Run once after deploying the no-UUID change.
"""
import re
from pathlib import Path
from shutil import move

AGENT_DIR = Path("config/agents")
ARCHIVE_DIR = AGENT_DIR / ".archive"
ARCHIVE_DIR.mkdir(exist_ok=True)

# Known fleet workers (preserve these, archive duplicates)
FLEET_WORKERS = [
    "supervisor",
    "qc-agent",
    "worker-backend",
    "worker-frontend",
    "worker-platform",
    "worker-mcp",
    "worker-cleanup"
]

def migrate_worker_files():
    """Consolidate duplicate worker files, keep most recent."""
    for worker_base in FLEET_WORKERS:
        # Find all files matching worker-base-*.yaml (with UUID suffix)
        pattern = re.compile(rf"^{re.escape(worker_base)}-[a-z0-9]{{8}}\.yaml$")
        matching_files = [
            f for f in AGENT_DIR.glob(f"{worker_base}-*.yaml")
            if pattern.match(f.name)
        ]
        
        if not matching_files:
            continue
        
        # Sort by modification time (newest first)
        matching_files.sort(key=lambda p: p.stat().st_mtime, reverse=True)
        
        # Keep the most recent one
        keep = matching_files[0]
        clean_name = f"{worker_base}.yaml"
        clean_path = AGENT_DIR / clean_name
        
        if clean_path.exists():
            # Clean name already exists (maybe from previous migration)
            # Archive the UUID file
            print(f"Archiving duplicate: {keep.name}")
            move(keep, ARCHIVE_DIR / keep.name)
        else:
            # Rename most recent to clean name
            print(f"Migrating: {keep.name} → {clean_name}")
            move(keep, clean_path)
        
        # Archive remaining duplicates
        for old_file in matching_files[1:]:
            print(f"Archiving old: {old_file.name}")
            move(old_file, ARCHIVE_DIR / old_file.name)

if __name__ == "__main__":
    print("Starting agent file migration to clean IDs...")
    migrate_worker_files()
    print(f"Migration complete. Archived files moved to: {ARCHIVE_DIR}")
```

#### File 4: `src/airs/config/auto_register.py` (Verify Behavior)

**Verify:** The existing auto-register logic should work correctly with clean IDs:

```python
async def register_agent_metadata_only(config: AgentConfig) -> bool:
    # Idempotency: skip if already registered
    if AgentRegistry.get_agent(config.id):  # config.id is now "worker-backend"
        logger.info(f"📋 Metadata already registered for {config.id}; skipping")
        return True
    # ... register
```

This already works correctly — `config.id` from the YAML is now a clean name like "worker-backend", and the registry lookup matches exactly.

#### File 5: `src-tauri/src/sidecar/client.rs` (Optional Logging)

**Add:** Enhanced logging to verify agent reuse:

```rust
async fn find_or_create_agent(&self, slug: &str, model: &str) -> Result<String> {
    // 1. List all agents
    let list_url = format!("{}/api/agents", self.base_url);
    let list_resp = self.client.get(&list_url).send().await?;
    
    if list_resp.status().is_success() {
        let agents: Vec<serde_json::Value> = list_resp.json().await.unwrap_or_default();
        
        // 2. Look for exact match (slug is now "worker-backend", agent["name"] is "worker-backend")
        for agent in &agents {
            let name = agent["name"].as_str().unwrap_or("");
            if name == slug {
                if let Some(id) = agent["id"].as_str() {
                    info!("[OrchestratorClient] Reusing existing agent '{}' → {}", slug, id);
                    return Ok(id.to_string());
                }
            }
        }
    }
    
    // 3. Not found — create new (will get clean ID like "worker-backend")
    info!("[OrchestratorClient] Creating new agent for worker '{}'", slug);
    // ... POST /api/agents
}
```

---

### Expected Behavior After Fix

**Before Fix (Current):**
```
Startup 1:
  Create worker-backend-8mhrv0b1.yaml
  Memory session: NEW (no history)

Startup 2:
  Load worker-backend-8mhrv0b1.yaml (from disk)
  Create worker-backend-x9k2m7p4.yaml (NEW - UUID changed!)
  Memory session: NEW (orphaned previous session)
  
Startup 3:
  Load 2 old files + create NEW file
  3 different agent histories, all disconnected
```

**After Fix (No UUID):**
```
Startup 1:
  Create worker-backend.yaml
  Memory session: NEW (expected for first run)

Startup 2:
  Load worker-backend.yaml
  AgentRegistry.get_agent("worker-backend") found!
  Skip creation - reuse existing
  Memory session: PRESERVED (continuous from previous session)
  
Startup N:
  Always same file: worker-backend.yaml
  Always same ID: "worker-backend"
  Always same memory session
  File count: 1 (stable)
```

---

### Testing the Fix

**1. Fresh Start Test:**
```bash
# Clean slate
rm -rf config/agents/*.yaml

# Start app
# Expected: Creates worker-backend.yaml, worker-frontend.yaml, etc.
# Each with clean name (no suffix)
```

**2. Restart Persistence Test:**
```bash
# First run - interact with worker-backend, create some memory
# Restart app
# Expected: 
#   - Logs show "Reusing existing agent 'worker-backend'"
#   - No new YAML files created
#   - Agent remembers previous interactions
```

**3. Duplicate Prevention Test:**
```bash
# Try to create second "worker-backend"
curl -X POST http://localhost:4000/api/agents \
  -H "Authorization: Bearer dev-key-123" \
  -d '{"name": "worker-backend", "job_title": "Test", "gender": "male", "description": "Test"}'

# Expected: 400 Bad Request
# {"error": "Agent 'worker-backend' already exists. Use PUT /api/agents/worker-backend/config to update..."}
```

**4. File Hygiene Test:**
```bash
# After multiple restarts, count agent files
ls config/agents/*.yaml | wc -l

# Expected: Fixed number (7 for fleet workers), not growing
```

---

### Summary

| Approach | UUID Suffix | Duplicate Prevention | Memory Persistence | File Hygiene | Recommendation |
|----------|-------------|---------------------|-------------------|--------------|----------------|
| **Current (ULID)** | Random 8-chars | ❌ Broken | ❌ Lost each restart | ❌ Accumulates | **REMOVE** |
| **Hash-Based** | Deterministic 8-chars | ✅ By hash | ✅ Preserved | ✅ Stable | Acceptable |
| **No UUID (Final)** | None | ✅ By registry/API | ✅ Preserved | ✅ Clean names | **ADOPT** |

**The No-UUID approach wins because:**
1. Uses existing registry uniqueness enforcement (no new code needed)
2. Leverages existing API duplicate checks (clear error messages)
3. Produces clean, debuggable agent IDs and filenames
4. Aligns with singleton fleet architecture
5. Simplest implementation (remove code, don't add it)

**Files to modify:**
1. `api_gateway/modules/agents/service.py` - Change `generate_agent_id()` to return clean slug
2. Create migration script (one-time cleanup)
3. Optional: Add logging in `sidecar/client.rs` for visibility

---

## Summary

### Toast Notifications
- **Status:** ✅ Implemented (April 20, 2026)
- **Approach:** Rust-side `StartupProgress` FSM + Tauri `startup:progress` event → Zustand `useStartupStore` → `StartupOverlay` modal.

### Agent UUID Issue
- **Status:** ✅ Implemented (April 20, 2026)
- **Approach:** Remove UUID suffix from `generate_agent_id()` entirely. Duplicate prevention delegated to `AgentRegistry` and `AgentConfigWriter.config_exists()`. Migration script provided for legacy UUID-suffixed files.

### Immediate Actions
1. [x] Remove ULID suffix from `generate_agent_id()` — now returns clean slug
2. [x] Ship migration script to consolidate legacy UUID files
3. [x] Ship startup toast overlay (Rust FSM + Tauri events + React store + CSS modal)
4. [ ] **User step:** Run `uv run python scripts/migrate_to_clean_ids.py --dry-run` from `submodules/core-orchestrator`, inspect output, then re-run without `--dry-run` to archive legacy files.
5. [ ] **User step:** Verify agent memory persistence by restarting the app and checking that `config/agents/` contains exactly one YAML per fleet worker.

---

## Implementation Log

### Part 1 — Startup Toast System (IMPLEMENTED)

**Architecture**

```
Tauri setup hook (main.rs)
   └─► StartupProgress::new(AppHandle)  ─── Init, Skills ───► frontend
         │
         └─► SidecarManager::set_progress(Arc<StartupProgress>)
               ├─► start_memory() ─────► MemoryStarting
               │   └─► log router scans stdout  ─► VectorDb (pattern match)
               ├─► mark_memory_running() ──────► MemoryReady
               ├─► start() ────────────────────► OrchStarting
               │   └─► log router scans stdout  ─► Routers / Agents / LivingAgents
               └─► mark_running() ─────────────► OrchReady → Workers → Ready
```

The FSM is monotonic: `StartupProgress::advance()` rejects any transition that
would move the machine backwards. This is intentional — the scheduler spawn
fires at a fixed +500ms while the sidecar fleet takes ~15s, so the fast path
can easily outrun the slow path.

**Files changed / added**

| File | Change |
|---|---|
| `src-tauri/src/sidecar/startup_progress.rs` | **NEW** — `StartupPhase` enum (12 phases), `StartupProgress` FSM with monotonic `advance()`, log-line `scan_line()`, Tauri `startup:progress` emission. |
| `src-tauri/src/sidecar/mod.rs` | `spawn_log_router` now accepts `Option<Arc<StartupProgress>>`; `SidecarManager` holds an attached progress tracker and advances the FSM from `start_memory`/`start`/`mark_memory_running`/`mark_running`. |
| `src-tauri/src/main.rs` | Creates `Arc<StartupProgress>` in the setup hook, emits `Init`+`Skills`, attaches it to `SidecarManager` before the sidecar auto-start task runs. |
| `src/stores/startupStore.ts` | **NEW** — Zustand store mirroring the Rust FSM; subscribes to `startup:progress`, auto-dismisses 600 ms after `ready`. |
| `src/components/startup/StartupOverlay.tsx` | **NEW** — Full-screen modal with per-phase icons (lucide `Check` / `Loader2` / `Circle`), progress bar, percent + hint footer. |
| `src/components/startup/StartupOverlay.css` | **NEW** — Winter-is-Coming themed styles using existing CSS variables (`--bg-card`, `--accent`, `--aqua`, `--text-*`). |
| `src/App.tsx` | Mounts `<StartupOverlay />` at the root of `.app-shell` so it overlays every tab. |

**Phase contract (Rust ↔ TS sync point)**

| Index | Rust `StartupPhase::id()` | TS `StartupPhaseId` | UI label |
|---|---|---|---|
| 0 | `init` | `init` | Initializing AutoDev… |
| 1 | `skills` | `skills` | Loading skills… |
| 2 | `memory_starting` | `memory_starting` | Starting Memory MCP Server… |
| 3 | `vector_db` | `vector_db` | Connecting to vector database… |
| 4 | `memory_ready` | `memory_ready` | Memory MCP Server ready |
| 5 | `orch_starting` | `orch_starting` | Starting Core Orchestrator… |
| 6 | `routers` | `routers` | Loading API routers… |
| 7 | `agents` | `agents` | Initializing agent registry… |
| 8 | `living_agents` | `living_agents` | Creating LivingAgents… |
| 9 | `orch_ready` | `orch_ready` | Core Orchestrator ready |
| 10 | `workers` | `workers` | Starting worker fleet… |
| 11 | `ready` | `ready` | AutoDev is ready |

**Log patterns that advance the FSM** (see `StartupProgress::scan_line`)

- `memory-mcp` stdout containing `Using Memory(` or `Qdrant` → `VectorDb`
- `core-orch` stdout containing `Registered` + `routers` → `Routers`
- `core-orch` stdout containing `Loading agent config from` → `Agents`
- `core-orch` stdout containing `Created instance for non-system agent` or `born with memory consciousness` or `LivingAgent` → `LivingAgents`

### Part 2 — Agent UUID Removal (IMPLEMENTED)

**Files changed / added**

| File | Change |
|---|---|
| `submodules/core-orchestrator/api_gateway/modules/agents/service.py` | `generate_agent_id()` now returns the clean slug only (no ULID suffix). Docstring explains the persistence guarantees and delegation to `AgentRegistry` / `AgentConfigWriter`. `create_agent()` error message updated with explicit update/delete instructions. |
| `submodules/core-orchestrator/scripts/migrate_to_clean_ids.py` | **NEW** — One-time cleanup: for each fleet worker, rename the newest UUID-suffixed YAML to `{worker}.yaml` (rewriting its `id` field), archive the rest to `config/agents/.archive/`. Supports `--dry-run`. |

**Unchanged but verified correct**

- `src/airs/config/auto_register.py::register_agent_metadata_only` — already idempotent via `AgentRegistry.get_agent(config.id)` check; the clean ID now matches the registry key exactly.
- `src/airs/agent/registry.py::register` — already raises `ValueError` on duplicate name; this is the primary duplicate-prevention mechanism for the no-UUID scheme.
- `src-tauri/src/sidecar/client.rs::find_or_create_agent` — the existing name-exact match `if name == slug` now succeeds on the first startup after migration because both sides use clean slugs.

### Verification

- **Rust:** `cargo check --lib` → 0 errors, 2 pre-existing warnings (`grep.rs::context_lines`, `git/repo.rs::to_repo_relative`). Both are unrelated to this change.
- **TypeScript:** `npx tsc --noEmit` → 3 pre-existing errors in unrelated files (`SkillCreatorModal`, `SkillsPalette`, `hookStore`). No new errors introduced.

### Migration procedure (user action required)

```powershell
# 1. Dry-run — verify which files would be renamed/archived
cd autodev/submodules/core-orchestrator
uv run python scripts/migrate_to_clean_ids.py --dry-run

# 2. Real run — performs the moves + rewrites the id field in the promoted YAMLs
uv run python scripts/migrate_to_clean_ids.py

# 3. Restart AutoDev. The loading overlay should now walk through all 12 phases.
#    After boot, config/agents/ should contain exactly one YAML per fleet worker
#    (worker-backend.yaml, worker-frontend.yaml, supervisor.yaml, …), all with
#    clean ids. Legacy UUID files are preserved in config/agents/.archive/ for
#    recovery if something goes wrong.
```

---

*Document generated from analysis of live logs and codebase review.*  
*Implementation log appended April 20, 2026.*
