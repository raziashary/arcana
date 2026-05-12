# AutoDev — Ruflo Implementation Plan

**Document Status:** Draft v1.0  
**Author:** Razi (Programming Lead, AIRS Foundry)  
**Date:** May 2026  
**Classification:** Internal — AutoDev Project

> This document defines the atomic implementation tasks for features inspired by Ruflo's architecture and adapted into AutoDev's Tauri/Rust/React codebase.  
> **Constraint:** No changes to `submodules/core-orchestrator/` or `submodules/memory-mcp-server/`. All code lives inside `src-tauri/src/` or `src/`.

---

## Table of Contents

1. [Semantic Task Routing (Worker Affinity Learning)](#1-semantic-task-routing--worker-affinity-learning)
2. [PID-Based Orphan Detection](#2-pid-based-orphan-detection)
3. [Diff Risk Scoring for QC Agent](#3-diff-risk-scoring-for-qc-agent)
4. [Budget Circuit Breaker](#4-budget-circuit-breaker)
5. [Phase-Based Task Decomposition](#5-phase-based-task-decomposition)
6. [MCP Connection Health Monitor](#6-mcp-connection-health-monitor)
7. [Browser Automation QC Skill](#7-browser-automation-qc-skill)

---

## 1. Semantic Task Routing — Worker Affinity Learning

**Ruflo Reference:** `hooks_route` concept in `v3/@claude-flow/cli/src/mcp-tools/hooks-tools.ts`  
**Goal:** The Supervisor learns which worker performs best for specific task types. Instead of pure keyword/role matching, routing decisions improve over time based on historical success/failure data.  
**Does not touch submodules.** Embeddings are retrieved by calling the already-running `memory-mcp-server` sidecar via its existing HTTP API through the existing `sidecar/client.rs` client — we're calling it, not modifying it.

**Depends On:** Existing `db/tasks.rs`, `sidecar/client.rs`, `commands/task_commands.rs`, `mcp_server/server.rs`

---

### 1.1 Database Schema Extension

**File:** `src-tauri/src/db/tasks.rs`

Add a new `routing_history` table to the existing `tasks.db`. This stores every Supervisor assignment decision and its eventual outcome so the router can learn from it.

- [x] Add migration `V4__routing_history.sql` (or inline as a schema string in `tasks.rs`):

```sql
CREATE TABLE IF NOT EXISTS routing_history (
    id           TEXT PRIMARY KEY,
    task_id      TEXT NOT NULL,
    task_title   TEXT NOT NULL,
    task_type    TEXT NOT NULL,
    worker_slug  TEXT NOT NULL,
    assigned_at  TEXT NOT NULL,
    outcome      TEXT,          -- 'success' | 'failure' | 'reassigned' | NULL (pending)
    outcome_at   TEXT,
    duration_ms  INTEGER,
    tokens_used  INTEGER
);

CREATE INDEX IF NOT EXISTS idx_routing_history_worker ON routing_history(worker_slug);
CREATE INDEX IF NOT EXISTS idx_routing_history_task_type ON routing_history(task_type);
CREATE INDEX IF NOT EXISTS idx_routing_history_outcome ON routing_history(outcome);
```

- [x] Add Rust struct to `db/tasks.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingHistoryEntry {
    pub id: String,
    pub task_id: String,
    pub task_title: String,
    pub task_type: String,
    pub worker_slug: String,
    pub assigned_at: String,
    pub outcome: Option<String>,
    pub outcome_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub tokens_used: Option<i64>,
}
```

- [x] Add DB functions to `db/tasks.rs`:
  - `insert_routing_history(entry: &RoutingHistoryEntry) -> Result<()>`
  - `update_routing_outcome(task_id: &str, outcome: &str, duration_ms: i64, tokens: i64) -> Result<()>` — called when a task transitions to `done` or `failed`
  - `get_routing_history(limit: i64) -> Result<Vec<RoutingHistoryEntry>>`
  - `get_worker_success_rates() -> Result<HashMap<String, f64>>` — computes `success / (success + failure)` per worker_slug, grouped by task_type

---

### 1.2 Static Pattern Registry

**File:** `src-tauri/src/routing/mod.rs` *(new file)*  
**File:** `src-tauri/src/routing/patterns.rs` *(new file)*

Static patterns are the fast path — checked before any historical lookup. These are keyword rules defined in code (no LLM call, <1ms).

- [x] Create `src-tauri/src/routing/` directory
- [x] `patterns.rs` — define `StaticPattern` struct and `STATIC_PATTERNS` constant:

```rust
pub struct StaticPattern {
    pub keywords: &'static [&'static str],
    pub worker_slug: &'static str,
    pub confidence: f32,
}

pub const STATIC_PATTERNS: &[StaticPattern] = &[
    StaticPattern {
        keywords: &["api", "endpoint", "route", "fastapi", "pydantic", "schema", "backend", "database", "migration"],
        worker_slug: "worker-backend",
        confidence: 0.85,
    },
    StaticPattern {
        keywords: &["react", "component", "ui", "css", "tailwind", "frontend", "button", "modal", "store", "zustand"],
        worker_slug: "worker-frontend",
        confidence: 0.85,
    },
    StaticPattern {
        keywords: &["docker", "ci", "github actions", "deploy", "infrastructure", "workflow", "platform", "devops"],
        worker_slug: "worker-platform",
        confidence: 0.80,
    },
    StaticPattern {
        keywords: &["mcp", "protocol", "tool", "server", "stdio", "transport", "plugin"],
        worker_slug: "worker-mcp",
        confidence: 0.80,
    },
    StaticPattern {
        keywords: &["dead code", "refactor", "cleanup", "dependency", "unused", "deprecate", "lint"],
        worker_slug: "worker-cleanup",
        confidence: 0.75,
    },
];
```

- [x] `fn match_static(task_title: &str, task_description: &str) -> Option<(&'static str, f32)>` — lowercases input, counts keyword hits per pattern, returns pattern with highest hit count if above threshold (≥2 keyword matches)

---

### 1.3 Affinity Scorer

**File:** `src-tauri/src/routing/scorer.rs` *(new file)*

Historical scoring using the `routing_history` table. No vector embeddings — pure SQL aggregation with task_type + keyword overlap scoring.

- [x] Define `AffinityScore` struct:

```rust
pub struct AffinityScore {
    pub worker_slug: String,
    pub score: f32,       // 0.0 - 1.0
    pub confidence: f32,  // how much historical data backs this
    pub method: String,   // "static" | "historical" | "default"
}
```

- [x] `async fn score_workers(task: &Task, history: &[RoutingHistoryEntry], workers: &[Worker]) -> Vec<AffinityScore>`:
  1. Filter `history` to entries with matching `task_type`
  2. For each `worker_slug` with ≥3 historical entries of that type, compute: `score = success_rate * 0.6 + (1.0 / avg_duration_rank) * 0.4`
  3. For workers with <3 entries, assign `score = 0.5` (neutral — not enough data)
  4. Normalize scores across all active workers
  5. Return sorted list

- [x] `fn select_worker(scores: Vec<AffinityScore>, static_match: Option<(&str, f32)>) -> AffinityScore`:
  - If static match confidence > 0.85 AND historical score for same worker > 0.5: use static match
  - If static match confidence > 0.85 AND historical score for same worker < 0.3: defer to historical (worker has been bad at that keyword pattern)
  - Otherwise: return highest scoring worker from historical

---

### 1.4 TaskRouter Struct

**File:** `src-tauri/src/routing/mod.rs`

- [x] Define `TaskRouter`:

```rust
pub struct TaskRouter {
    db: Arc<DbPool>,
}

impl TaskRouter {
    pub fn new(db: Arc<DbPool>) -> Self { ... }

    pub async fn suggest_worker(&self, task: &Task, active_workers: &[Worker]) -> AffinityScore {
        // 1. Try static pattern matching
        let static_match = patterns::match_static(&task.title, task.description.as_deref().unwrap_or(""));

        // 2. Load recent routing history (last 200 entries)
        let history = self.db.get_routing_history(200).await?;

        // 3. Score workers historically
        let scores = scorer::score_workers(task, &history, active_workers).await;

        // 4. Select best worker
        scorer::select_worker(scores, static_match)
    }

    pub async fn record_assignment(&self, task: &Task, worker_slug: &str) -> Result<()> {
        // Inserts into routing_history with outcome = NULL
        let entry = RoutingHistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            task_id: task.id.clone(),
            task_title: task.title.clone(),
            task_type: task.task_type.to_string(),
            worker_slug: worker_slug.to_string(),
            assigned_at: chrono::Utc::now().to_rfc3339(),
            outcome: None,
            ..Default::default()
        };
        self.db.insert_routing_history(&entry).await
    }

    pub async fn record_outcome(&self, task_id: &str, outcome: &str, duration_ms: i64, tokens: i64) -> Result<()> {
        self.db.update_routing_outcome(task_id, outcome, duration_ms, tokens).await
    }
}
```

- [x] Register `TaskRouter` in `state/app_state.rs` — add field `pub task_router: Arc<TaskRouter>` *(implemented as stateless free-function pattern instead; no Arc needed)*
- [x] Initialize in `main.rs` alongside other state components *(n/a — stateless pattern)*

---

### 1.5 Tauri Commands

**File:** `src-tauri/src/commands/routing_commands.rs` *(new file)*

- [x] `suggest_worker_for_task(task_id: String) -> Result<AffinityScore>` — loads task from DB, calls `TaskRouter::suggest_worker()`, returns result
- [x] `get_worker_affinity_stats() -> Result<Vec<WorkerAffinityStats>>` — returns per-worker success rates grouped by task_type for the UI
- [x] Register both commands in `src-tauri/src/commands/mod.rs`

---

### 1.6 MCP Tool Exposure for Supervisor

**File:** `src-tauri/src/mcp_server/server.rs`

The Supervisor recipe uses this tool to ask "what worker should handle this task?" before assigning.

- [x] Add `suggest_worker` to the MCP tool registry (alongside existing task board tools):

```rust
Tool {
    name: "suggest_worker".into(),
    description: Some("Get the recommended worker for a task based on historical performance and task type. Returns worker_slug and confidence score.".into()),
    input_schema: json!({
        "type": "object",
        "properties": {
            "task_id": { "type": "string", "description": "Task ID from the internal board" }
        },
        "required": ["task_id"]
    }),
}
```

- [x] Add handler in `mcp_server/tool_adapter.rs`: calls `routing_commands::suggest_worker_for_task()` *(implemented as builtin tool in `suggest_worker_tool.rs`; dispatched via standard ToolRegistry path)*
- [x] This tool is available to Supervisor recipe only — add it to the allowed list alongside the existing supervisor tools *(exposed to all workers via MCP; Supervisor recipe calls it, others don't)*

---

### 1.7 Outcome Wiring

**File:** `src-tauri/src/commands/task_commands.rs`

When a task transitions to `done` or `failed`, automatically record the routing outcome.

- [x] In the `update_task_status` command handler, after the DB update succeeds:
  - If new status is `Done`: call `router.record_outcome(task_id, "success", duration_ms, tokens_from_recipe_run)`
  - If new status is `Failed`: call `router.record_outcome(task_id, "failure", ...)`
  - If new status is `Assigned` (reassignment): call `router.record_outcome(old_task_id, "reassigned", ...)`

---

### 1.8 Frontend: Affinity Stats Panel

**File:** `src/tabs/AutoDevPanel.tsx` (or a new sub-component)  
**File:** `src/components/dashboard/WorkerAffinityPanel.tsx` *(new file)*

- [x] New component `WorkerAffinityPanel` — displays a small table:

```
Worker          | Backend Tasks | Frontend Tasks | Cleanup Tasks | Success Rate
worker-backend  |   23 (88%)    |    2 (50%)     |    0          |   87%
worker-frontend |    1 (100%)   |   18 (94%)     |    0          |   94%
```

- [x] Calls `invoke('get_worker_affinity_stats')` on mount + refresh
- [x] Renders in the AutoDev Panel tab beneath the Agent Roster, collapsible

---

## 2. PID-Based Orphan Detection

**Ruflo Reference:** `swarm_init` orphan reconciliation in `v3/@claude-flow/swarm/src/unified-coordinator/` and `v3/@claude-flow/cli/src/mcp-tools/swarm-tools.ts` lines 2301-2340  
**Goal:** Replace the current 4-hour stale lock timeout with an immediate PID-aliveness check. When a worker process dies unexpectedly, its claimed tasks are released within the next Supervisor poll cycle (2 min) rather than after 4 hours.  
**Does not touch submodules.** All PID logic is in Tauri Rust using the `sysinfo` crate (already available in the workspace or to be added).

**Depends On:** `db/tasks.rs`, `recipes/scheduler.rs`, `commands/task_commands.rs`, `state/app_state.rs`

---

### 2.1 Cargo Dependency

**File:** `src-tauri/Cargo.toml`

- [x] Verify `sysinfo` crate is present; if not, add: *(already at `sysinfo = "0.33"` in Cargo.toml)*

```toml
[dependencies]
sysinfo = { version = "0.32", default-features = false, features = ["system"] }
```

`sysinfo` provides cross-platform PID checking (Windows, macOS, Linux) without unsafe OS API calls. It's already used in several Tauri ecosystem projects.

---

### 2.2 Database Schema Extension

**File:** `src-tauri/src/db/tasks.rs`

Add PID tracking columns to the `tasks` table. Workers record their own PID when claiming a task.

- [x] Migration — add to existing schema or as a new migration:

```sql
ALTER TABLE tasks ADD COLUMN claimed_by_pid INTEGER;
ALTER TABLE tasks ADD COLUMN claim_host     TEXT;  -- hostname for future multi-machine use
```

- [x] Update `Task` struct in `db/tasks.rs`:

```rust
pub struct Task {
    // ... existing fields ...
    pub claimed_by_pid: Option<i64>,
    pub claim_host: Option<String>,
}
```

- [x] Add DB functions:
  - `claim_task_with_pid(task_id: &str, worker_slug: &str, pid: u32) -> Result<()>` — sets `status = 'in_progress'`, `assigned_worker`, `claimed_by_pid`, `claim_host = hostname::get()`
  - `get_orphan_candidates() -> Result<Vec<Task>>` — returns tasks with `status IN ('assigned', 'in_progress') AND claimed_by_pid IS NOT NULL`
  - `release_orphaned_task(task_id: &str, reason: &str) -> Result<()>` — resets `status = 'ready'`, clears `assigned_worker`, `claimed_by_pid`, appends reason to task notes

---

### 2.3 PID Checker Module

**File:** `src-tauri/src/orphan/mod.rs` *(new file)*  
**File:** `src-tauri/src/orphan/pid_checker.rs` *(new file)*

- [x] `pid_checker.rs`:

```rust
use sysinfo::{System, Pid};

/// Returns true if the process with the given PID is currently alive.
/// Cross-platform: works on Windows, macOS, Linux.
pub fn is_pid_alive(pid: u32) -> bool {
    let mut sys = System::new();
    // Refresh only the specific process — not entire system scan
    sys.refresh_process(Pid::from(pid as usize));
    sys.process(Pid::from(pid as usize)).is_some()
}

/// Returns true if the PID belongs to an AutoDev recipe runner process.
/// Prevents false negatives when a new unrelated process reuses the same PID.
pub fn is_autodev_process(pid: u32) -> bool {
    let mut sys = System::new();
    sys.refresh_process(Pid::from(pid as usize));
    if let Some(process) = sys.process(Pid::from(pid as usize)) {
        let name = process.name().to_string_lossy().to_lowercase();
        // AutoDev recipe runners are spawned as the main autodev process or a tokio task inside it
        name.contains("autodev") || name.contains("airsctl")
    } else {
        false
    }
}
```

- [x] `mod.rs`:

```rust
pub mod pid_checker;

use crate::db::tasks::{Task, TaskDb};

pub struct OrphanDetector {
    db: Arc<TaskDb>,
    /// Stale TTL fallback: if no PID recorded, how long before we consider a task orphaned
    stale_ttl_hours: i64,
}

impl OrphanDetector {
    pub fn new(db: Arc<TaskDb>) -> Self {
        Self { db, stale_ttl_hours: 4 }
    }

    /// Main reconciliation loop — call this on every Supervisor poll tick
    pub async fn reconcile(&self) -> Result<Vec<String>> {
        let candidates = self.db.get_orphan_candidates().await?;
        let mut released = Vec::new();

        for task in candidates {
            let orphan_reason = self.check_task(&task).await;
            if let Some(reason) = orphan_reason {
                self.db.release_orphaned_task(&task.id, &reason).await?;
                released.push(task.id.clone());
                tracing::warn!("Released orphaned task {} ({}): {}", task.id, task.title, reason);
            }
        }

        Ok(released)
    }

    async fn check_task(&self, task: &Task) -> Option<String> {
        // PID-based check (fast, immediate)
        if let Some(pid) = task.claimed_by_pid {
            let pid = pid as u32;
            if !pid_checker::is_pid_alive(pid) {
                return Some(format!("host process PID {} exited", pid));
            }
            // PID alive but is it really an AutoDev process?
            // Only enforce if we know the hostname matches current machine
            if let Some(ref host) = task.claim_host {
                let current_host = hostname::get().map(|h| h.to_string_lossy().to_string()).unwrap_or_default();
                if host == &current_host && !pid_checker::is_autodev_process(pid) {
                    return Some(format!("PID {} is alive but is not an AutoDev process (PID reuse suspected)", pid));
                }
            }
            return None; // PID alive and valid, not orphaned
        }

        // TTL fallback: no PID recorded (pre-PID tasks or external assignments)
        if let Some(updated_at) = &task.updated_at {
            let updated = chrono::DateTime::parse_from_rfc3339(updated_at).ok()?;
            let age_hours = chrono::Utc::now().signed_duration_since(updated).num_hours();
            if age_hours > self.stale_ttl_hours {
                return Some(format!("no PID recorded and task is {}h stale (TTL fallback)", age_hours));
            }
        }

        None
    }
}
```

---

### 2.4 Register OrphanDetector in App State

**File:** `src-tauri/src/state/app_state.rs`

- [x] Add `pub orphan_detector: Arc<OrphanDetector>` to `AppState` *(implemented as a stateless module — `crate::orphan::reconcile()` free function, no AppState field needed)*
- [x] Initialize in `main.rs` after DB is ready: *(n/a — stateless; called directly from scheduler)*

```rust
let orphan_detector = Arc::new(OrphanDetector::new(db.tasks.clone()));
```

---

### 2.5 Wire Into Recipe Scheduler

**File:** `src-tauri/src/recipes/scheduler.rs`

The Supervisor recipe already polls every 2 minutes. We hook orphan detection into the Supervisor's startup tick and each subsequent poll.

- [x] In the scheduler's Supervisor tick handler (the section that fires before/after `RecipeRunner::run_recipe()`):

```rust
// Run orphan detection BEFORE the Supervisor recipe executes
// so the Supervisor sees freshly-released tasks on its next poll
let released = state.orphan_detector.reconcile().await?;
if !released.is_empty() {
    tracing::info!("OrphanDetector released {} tasks: {:?}", released.len(), released);
    // Emit event so frontend task board updates
    app.emit("task:orphans-released", &released)?;
}
```

- [x] This means orphan detection runs every ~2 minutes (same as Supervisor poll) — no extra timer needed

---

### 2.6 Worker MCP Tool: Claim with PID

**File:** `src-tauri/src/mcp_server/tool_adapter.rs`  
**File:** `src-tauri/src/mcp_server/server.rs`

Workers need to record their PID when claiming a task. Expose a new MCP tool `claim_task` that replaces calling `update_task_status` with `in_progress`.

- [x] Add MCP tool definition in `server.rs`:

```rust
Tool {
    name: "claim_task".into(),
    description: Some("Claim a task and begin working on it. Records this process's PID for orphan detection.".into()),
    input_schema: json!({
        "type": "object",
        "properties": {
            "task_id": { "type": "string" }
        },
        "required": ["task_id"]
    }),
}
```

- [x] Handler in `tool_adapter.rs`:

```rust
"claim_task" => {
    let task_id = input["task_id"].as_str().ok_or("missing task_id")?;
    let pid = std::process::id(); // current process PID
    state.db.tasks.claim_task_with_pid(task_id, &worker_slug, pid).await?;
    Ok(json!({ "claimed": true, "pid": pid }))
}
```

- [x] Add `claim_task` to the allowed tools list for worker recipes (not for QC or Supervisor — they use `assign_task`) *(registered in `register_for_mcp_worker` and `FastDevelopment/Review` modes)*

---

### 2.7 Tauri Commands

**File:** `src-tauri/src/commands/task_commands.rs`

- [x] `get_orphaned_tasks() -> Result<Vec<Task>>` — returns current orphan candidates with their PID and age
- [x] `force_release_task(task_id: String) -> Result<()>` — manual override for Razi to release a stuck task from the UI
- [x] Register in `commands/mod.rs`

---

### 2.8 Frontend: Orphan Alert

**File:** `src/stores/agentStore.ts` or `src/stores/dashboardStore.ts`  
**File:** `src/components/dashboard/OrphanAlert.tsx` *(new file)*

- [x] Listen for `task:orphans-released` event in the dashboard store:

```typescript
listen('task:orphans-released', (event) => {
  const released = event.payload as string[];
  set(state => ({
    orphanAlerts: [...state.orphanAlerts, {
      id: Date.now(),
      taskIds: released,
      at: new Date().toISOString(),
    }]
  }));
});
```

- [x] `OrphanAlert.tsx` — small toast-style notification in the AutoDev Panel: "⚠️ 2 tasks released — worker process exited unexpectedly. Tasks returned to Ready queue."
- [x] Auto-dismiss after 10 seconds

---

## 3. Diff Risk Scoring for QC Agent

**Ruflo Reference:** `analyze_diff` in `v3/@claude-flow/cli/src/mcp-tools/analyze-tools.ts` lines 1-355  
**Goal:** Give the QC Agent a structured diff analysis tool that outputs risk score, change classification, and a human-readable summary. The QC Agent includes this in its PR review decision rather than doing free-form analysis.  
**Does not touch submodules.** Diff parsing is done in Rust using the existing `src-tauri/src/git/diff.rs`. Classification uses the LLM provider already wired in `src-tauri/src/provider/`.

**Depends On:** `git/diff.rs`, `provider/traits.rs`, `mcp_server/server.rs`, `commands/mod.rs`

---

### 3.1 Diff Analysis Module

**File:** `src-tauri/src/diff_analysis/mod.rs` *(new file)*  
**File:** `src-tauri/src/diff_analysis/parser.rs` *(new file)*  
**File:** `src-tauri/src/diff_analysis/risk_scorer.rs` *(new file)*

#### 3.1.1 Parser

- [ ] `parser.rs` — Parse raw unified diff text into structured change data:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct ParsedDiff {
    pub files: Vec<ChangedFile>,
    pub total_lines_added: usize,
    pub total_lines_deleted: usize,
    pub file_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangedFile {
    pub path: String,
    pub extension: String,
    pub lines_added: usize,
    pub lines_deleted: usize,
    pub is_new_file: bool,
    pub is_deleted_file: bool,
    pub is_renamed: bool,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiffHunk {
    pub header: String,
    pub added_lines: Vec<String>,
    pub deleted_lines: Vec<String>,
}
```

- [ ] `fn parse_unified_diff(diff_text: &str) -> Result<ParsedDiff>` — standard unified diff parsing:
  - Split on `diff --git` lines to get per-file sections
  - Parse `+++ / ---` headers to extract file paths and rename info
  - Count `+` / `-` lines per hunk
  - Detect `new file mode` and `deleted file mode` markers

#### 3.1.2 Risk Scorer (structural, no LLM)

- [ ] `risk_scorer.rs` — Compute a structural risk score from the parsed diff alone (fast, no API call):

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct StructuralRisk {
    pub score: f32,         // 0.0 (safe) to 1.0 (high risk)
    pub factors: Vec<RiskFactor>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RiskFactor {
    pub name: String,
    pub contribution: f32,
    pub detail: String,
}
```

- [ ] `fn compute_structural_risk(diff: &ParsedDiff) -> StructuralRisk`:

| Factor | Weight | Trigger |
|--------|--------|---------|
| High churn | 0.25 | total lines changed > 500 |
| Critical file paths | 0.30 | changes to `auth`, `security`, `migration`, `db/`, `schema` paths |
| File count breadth | 0.15 | changes span >10 files |
| Binary/config changes | 0.15 | `.env`, `.toml`, `.yaml`, `.json`, `Dockerfile` changed |
| Deletions-heavy | 0.15 | deletion-to-addition ratio > 2:1 |

Each factor contributes its weight if triggered, score is a sum clamped to 1.0.

---

### 3.2 LLM Classification

**File:** `src-tauri/src/diff_analysis/classifier.rs` *(new file)*

This makes a single LLM call to classify the diff and generate a human-readable summary. Uses the existing `Provider` trait — no new API dependencies.

- [ ] Define output types:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub enum ChangeClassification {
    Feature,
    Bugfix,
    Refactor,
    Documentation,
    Test,
    Config,
    Security,
    Performance,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiffClassification {
    pub classification: ChangeClassification,
    pub summary: String,        // 1-2 sentence human-readable summary
    pub review_notes: Vec<String>,   // Specific things QC should check
    pub suggested_checks: Vec<String>,  // e.g., "Run integration tests", "Check auth middleware"
}
```

- [ ] `async fn classify_diff(provider: &dyn Provider, diff: &ParsedDiff, structural_risk: &StructuralRisk) -> Result<DiffClassification>`:
  - Build a compact prompt using the parsed diff summary (NOT the full diff text — too many tokens):
    - File list with line counts
    - Key changed paths
    - Structural risk factors
    - Sample of the first 50 added lines and 50 deleted lines
  - Call `provider.complete()` (non-streaming, structured output request)
  - Parse JSON response into `DiffClassification`
  - System prompt includes: "You are a code review assistant. Analyze this diff summary and return JSON: `{classification, summary, review_notes[], suggested_checks[]}`"

---

### 3.3 DiffAnalyzer Facade

**File:** `src-tauri/src/diff_analysis/mod.rs`

- [ ] `DiffAnalyzer` struct that wires parser + risk scorer + classifier:

```rust
pub struct DiffAnalyzer {
    provider: Arc<dyn Provider>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiffAnalysisResult {
    pub risk_score: f32,
    pub risk_level: String,          // "low" | "medium" | "high" | "critical"
    pub classification: ChangeClassification,
    pub summary: String,
    pub review_notes: Vec<String>,
    pub suggested_checks: Vec<String>,
    pub file_count: usize,
    pub lines_added: usize,
    pub lines_deleted: usize,
    pub critical_files: Vec<String>, // files that triggered high-risk factors
    pub analyzed_at: String,
}

impl DiffAnalyzer {
    pub async fn analyze(&self, diff_text: &str) -> Result<DiffAnalysisResult> {
        // 1. Parse diff
        let parsed = parser::parse_unified_diff(diff_text)?;

        // 2. Structural risk (no LLM)
        let structural = risk_scorer::compute_structural_risk(&parsed);

        // 3. LLM classification (single call)
        let classification = classifier::classify_diff(&*self.provider, &parsed, &structural).await?;

        // 4. Combine into result
        let risk_level = match structural.score {
            s if s >= 0.75 => "critical",
            s if s >= 0.50 => "high",
            s if s >= 0.25 => "medium",
            _ => "low",
        };

        Ok(DiffAnalysisResult {
            risk_score: structural.score,
            risk_level: risk_level.to_string(),
            classification: classification.classification,
            summary: classification.summary,
            review_notes: classification.review_notes,
            suggested_checks: classification.suggested_checks,
            file_count: parsed.file_count,
            lines_added: parsed.total_lines_added,
            lines_deleted: parsed.total_lines_deleted,
            critical_files: structural.factors.iter()
                .filter(|f| f.name == "critical_file_paths")
                .flat_map(|_| parsed.files.iter().filter(|f| is_critical_path(&f.path)).map(|f| f.path.clone()))
                .collect(),
            analyzed_at: chrono::Utc::now().to_rfc3339(),
        })
    }
}
```

---

### 3.4 Tauri Command

**File:** `src-tauri/src/commands/diff_commands.rs` *(new file)*

- [ ] `analyze_diff(diff_text: String) -> Result<DiffAnalysisResult>` — takes a raw diff string, returns analysis
- [ ] `analyze_pr_diff(repo_path: String, base_branch: String, head_branch: String) -> Result<DiffAnalysisResult>` — uses `git/diff.rs` to generate the diff, then calls `DiffAnalyzer::analyze()`:
  - Call `git2::Repository::open(repo_path)`
  - Generate diff between `base_branch..head_branch` using existing `git::diff` module
  - Convert to unified diff string
  - Pass to analyzer

- [ ] Register both in `commands/mod.rs`

---

### 3.5 MCP Tool for QC Agent

**File:** `src-tauri/src/mcp_server/server.rs`  
**File:** `src-tauri/src/mcp_server/tool_adapter.rs`

- [ ] Add `analyze_diff` MCP tool available to QC Agent only:

```rust
Tool {
    name: "analyze_diff".into(),
    description: Some("Analyze a git diff and get risk score, change classification, and review notes. Returns structured analysis to guide QC review decision.".into()),
    input_schema: json!({
        "type": "object",
        "properties": {
            "repo_path": { "type": "string", "description": "Absolute path to the git repo" },
            "base_branch": { "type": "string", "description": "Base branch (e.g., main)" },
            "head_branch": { "type": "string", "description": "PR branch to analyze" }
        },
        "required": ["repo_path", "base_branch", "head_branch"]
    }),
}
```

- [ ] Handler routes to `diff_commands::analyze_pr_diff()`
- [ ] Add `analyze_diff` to the QC Agent's allowed tool list in `server.rs`

---

### 3.6 QC Recipe Update

**File:** `~/.autodev/recipes/qc-agent.yaml` (the system recipe definition in `src-tauri/src/recipes/`)

- [ ] Update the QC recipe instructions to include the analysis step:

```yaml
instructions: |
  You are the AIRS QC Agent. For each PR assigned to you:
  1. Use analyze_diff to get a structured risk assessment of the PR diff.
  2. Read the risk_level and review_notes from the analysis.
  3. If risk_level is "critical": flag for human review, do NOT approve.
  4. If risk_level is "high": run all suggested_checks before deciding.
  5. Run tests via bash tool: use the suggested_checks list as your test plan.
  6. Approve if tests pass and risk is acceptable. Reject with review_notes if not.
  Include the risk_score and classification in your PR review comment.
```

---

### 3.7 Frontend: Diff Analysis Display

**File:** `src/components/dashboard/DiffAnalysisCard.tsx` *(new file)*

- [ ] Small card component used in the task board when a task is in `qc_review`:
  - Risk level badge (color-coded: green/yellow/orange/red)
  - Classification tag
  - Summary text
  - Collapsible "Review Notes" list

- [ ] Called from the task card component when task status is `qc_review` and `diff_analysis` data is available in the task record
- [ ] Store the `DiffAnalysisResult` JSON in a new nullable `diff_analysis` TEXT column in `tasks.db` (added as part of this feature's migration)

---

## 4. Budget Circuit Breaker

**Ruflo Reference:** ADR-097 Federation Budget in `v3/@claude-flow/plugin-agent-federation/src/domain/value-objects/federation-budget.ts`  
**Goal:** Pre-emptive per-task token and cost caps enforced by the agent engine loop. When a worker's running token total for a task exceeds its budget, the loop halts and the task is marked as `needs_info` (requesting Supervisor guidance) rather than silently burning tokens.  
**Does not touch submodules.** Enforced entirely inside `src-tauri/src/agent/engine.rs` and the task database.

**Depends On:** `agent/engine.rs`, `db/tasks.rs`, `cost/tracker.rs`, `commands/task_commands.rs`, `recipes/runner.rs`

---

### 4.1 Budget Types

**File:** `src-tauri/src/budget/mod.rs` *(new file)*

- [ ] Define budget structures:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskBudget {
    pub max_tokens: Option<u64>,   // None = unbounded
    pub max_usd: Option<f64>,      // None = unbounded
    pub max_turns: Option<u32>,    // Agent loop turn limit (default: 20)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub tokens_used: u64,
    pub cost_usd: f64,
    pub turns_used: u32,
    pub exceeded: bool,
    pub exceeded_reason: Option<BudgetExceededReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BudgetExceededReason {
    TokenLimitReached,
    CostLimitReached,
    TurnLimitReached,
}

impl TaskBudget {
    pub fn default_worker() -> Self {
        Self {
            max_tokens: Some(100_000),   // 100k tokens per task by default
            max_usd: Some(0.50),         // $0.50 per task by default
            max_turns: Some(20),         // 20 turns max
        }
    }

    pub fn default_supervisor() -> Self {
        Self {
            max_tokens: Some(50_000),
            max_usd: Some(0.25),
            max_turns: Some(10),
        }
    }

    pub fn unlimited() -> Self {
        Self { max_tokens: None, max_usd: None, max_turns: Some(20) }
    }

    /// Returns exceeded reason if any budget is exceeded
    pub fn check(&self, status: &BudgetStatus) -> Option<BudgetExceededReason> {
        if self.max_tokens.map_or(false, |m| status.tokens_used >= m) {
            return Some(BudgetExceededReason::TokenLimitReached);
        }
        if self.max_usd.map_or(false, |m| status.cost_usd >= m) {
            return Some(BudgetExceededReason::CostLimitReached);
        }
        if self.max_turns.map_or(false, |m| status.turns_used >= m) {
            return Some(BudgetExceededReason::TurnLimitReached);
        }
        None
    }
}
```

- [ ] Input validation mirroring ADR-097 — in the command that sets budgets:
  - Reject `NaN`, `Infinity`, negative values
  - Hard ceiling: `max_tokens` ≤ 1,000,000; `max_usd` ≤ $100.00; `max_turns` ≤ 100
  - Return constant-string errors: `"BUDGET_INVALID_TOKENS"`, `"BUDGET_INVALID_USD"`, `"BUDGET_LIMIT_EXCEEDED"` — no echo of remaining values

---

### 4.2 Database Schema Extension

**File:** `src-tauri/src/db/tasks.rs`

- [ ] Add budget columns to `tasks` table:

```sql
ALTER TABLE tasks ADD COLUMN budget_max_tokens INTEGER;
ALTER TABLE tasks ADD COLUMN budget_max_usd    REAL;
ALTER TABLE tasks ADD COLUMN budget_max_turns  INTEGER DEFAULT 20;
ALTER TABLE tasks ADD COLUMN budget_tokens_used INTEGER DEFAULT 0;
ALTER TABLE tasks ADD COLUMN budget_cost_usd   REAL DEFAULT 0.0;
ALTER TABLE tasks ADD COLUMN budget_turns_used INTEGER DEFAULT 0;
ALTER TABLE tasks ADD COLUMN budget_exceeded   INTEGER DEFAULT 0;  -- boolean
ALTER TABLE tasks ADD COLUMN budget_exceeded_reason TEXT;
```

- [ ] Add DB functions:
  - `set_task_budget(task_id: &str, budget: &TaskBudget) -> Result<()>`
  - `update_task_budget_usage(task_id: &str, tokens: u64, cost: f64, turns: u32) -> Result<()>` — called after each agent turn
  - `mark_task_budget_exceeded(task_id: &str, reason: &str) -> Result<()>` — sets `budget_exceeded = 1`, updates status

---

### 4.3 Budget Enforcement in Agent Engine

**File:** `src-tauri/src/agent/engine.rs`

This is the core change. The agent loop currently runs until `stop_reason == "end_turn"` or 20 turns. We add budget checks at the top of each turn.

- [ ] Add `task_budget: Option<TaskBudget>` field to `AgentEngine` struct
- [ ] Add `budget_status: BudgetStatus` tracking field (cumulative across turns)
- [ ] At the **start of each loop turn**, before calling the LLM:

```rust
// Budget check — pre-emptive halt before spending more tokens
if let Some(ref budget) = self.task_budget {
    self.budget_status.turns_used += 1;
    if let Some(reason) = budget.check(&self.budget_status) {
        // Halt the loop
        let reason_str = match reason {
            BudgetExceededReason::TokenLimitReached => "BUDGET_EXCEEDED_TOKENS",
            BudgetExceededReason::CostLimitReached => "BUDGET_EXCEEDED_USD",
            BudgetExceededReason::TurnLimitReached => "BUDGET_EXCEEDED_TURNS",
        };
        app.emit("agent:budget-exceeded", BudgetExceededEvent {
            session_id: session_id.clone(),
            task_id: self.task_id.clone(),
            reason: reason_str.to_string(),
            tokens_used: self.budget_status.tokens_used,
            cost_usd: self.budget_status.cost_usd,
        })?;
        return Err(AgentError::BudgetExceeded(reason_str.to_string()));
    }
}
```

- [ ] After each LLM response, accumulate token usage:

```rust
self.budget_status.tokens_used += response.usage.input_tokens + response.usage.output_tokens;
self.budget_status.cost_usd += calculated_cost;

// Persist to DB if this engine is running a task
if let Some(ref task_id) = self.task_id {
    db.update_task_budget_usage(
        task_id,
        self.budget_status.tokens_used,
        self.budget_status.cost_usd,
        self.budget_status.turns_used,
    ).await?;
}
```

---

### 4.4 Budget Exceeded → Task Escalation

**File:** `src-tauri/src/agent/engine.rs`  
**File:** `src-tauri/src/commands/task_commands.rs`

When budget is exceeded, the task should NOT be marked as `failed` — it should escalate to the Supervisor for guidance. This uses the existing `WorkerRequest` system.

- [ ] In the budget exceeded handler (after the `agent:budget-exceeded` emit):
  - If the engine has a `task_id`: automatically create a `WorkerRequest` of type `report_issue`:
    ```
    title: "Budget exceeded on task #{task_id}"
    message: "Worker halted at {tokens_used} tokens / ${cost_usd:.4} USD. Budget limit: {max_tokens} tokens / ${max_usd}. Task needs budget increase or scope reduction."
    ```
  - Set task status to `needs_info` (existing status)
  - Supervisor's next poll will see `needs_info` tasks and decide: increase budget, split the task, or reassign

---

### 4.5 Supervisor Budget Assignment

**File:** `src-tauri/src/mcp_server/server.rs`

The Supervisor needs a tool to set task budgets when assigning work.

- [ ] Add `set_task_budget` MCP tool (Supervisor-only):

```rust
Tool {
    name: "set_task_budget".into(),
    description: Some("Set token and cost budget for a task before assigning it to a worker.".into()),
    input_schema: json!({
        "type": "object",
        "properties": {
            "task_id": { "type": "string" },
            "max_tokens": { "type": "integer", "description": "Max tokens for this task (omit for unlimited)" },
            "max_usd": { "type": "number", "description": "Max USD spend for this task (omit for unlimited)" }
        },
        "required": ["task_id"]
    }),
}
```

- [ ] Add to Supervisor-allowed tools list

---

### 4.6 Tauri Commands

**File:** `src-tauri/src/commands/budget_commands.rs` *(new file)*

- [ ] `set_task_budget(task_id: String, budget: TaskBudget) -> Result<()>` — validates input, calls DB
- [ ] `get_task_budget_status(task_id: String) -> Result<BudgetStatus>` — returns current usage vs limits
- [ ] `get_daily_budget_summary() -> Result<DailyBudgetSummary>` — aggregates all task budgets for the day: total allocated, total spent, tasks that exceeded
- [ ] Register in `commands/mod.rs`

---

### 4.7 Frontend: Budget Display

**File:** `src/components/dashboard/TaskCard.tsx` (existing component — extend it)  
**File:** `src/components/budget/BudgetBar.tsx` *(new file)*

- [ ] `BudgetBar.tsx` — a progress bar component:
  - Shows token usage vs budget (e.g., "45,232 / 100,000 tokens")
  - Color: green (<50%), yellow (50-80%), orange (80-95%), red (>95%)
  - Also shows cost: "$0.12 / $0.50"
  - Shows "EXCEEDED" badge if `budget_exceeded = true`

- [ ] Render `BudgetBar` inside task cards when task is `in_progress` or `needs_info` with a budget set
- [ ] Listen for `agent:budget-exceeded` Tauri event in `agentStore.ts` → add a notification to the dashboard

---

### 4.8 Settings UI: Default Budgets

**File:** `src/tabs/Settings.tsx` or the AIRS AutoDev section of Settings

- [ ] Add "Default Worker Budget" settings group:
  - `Max Tokens per Task` — number input, default 100,000
  - `Max USD per Task` — number input, default $0.50
  - `Warn at X% of budget` — slider, default 80%
- [ ] Store in `~/.autodev/autodev.toml` config (already reads from there)
- [ ] These become the default `TaskBudget` applied when Supervisor assigns a task without an explicit budget

---

## 5. Phase-Based Task Decomposition

**Ruflo Reference:** Swarm phase management in `v3/@claude-flow/swarm/src/unified-coordinator/` and `ISwarmHub.getCurrentPhase()` pattern  
**Goal:** When the Supervisor decomposes an epic, it can organize subtasks into phases (Foundation → Core → Integration → QA). Tasks in a later phase don't start until the previous phase reaches a milestone. This gives Razi a visible progress arc on the AutoDev Panel.  
**Does not touch submodules.** Implemented entirely as new columns in `tasks.db` and schema changes, plus UI additions.

**Depends On:** `db/tasks.rs`, `commands/task_commands.rs`, `mcp_server/server.rs`, `src/tabs/AutoDevPanel.tsx`

---

### 5.1 Database Schema Extension

**File:** `src-tauri/src/db/tasks.rs`

- [ ] Add phase/milestone columns to `tasks` table:

```sql
ALTER TABLE tasks ADD COLUMN phase         TEXT;         -- e.g., 'foundation', 'core', 'integration', 'qa'
ALTER TABLE tasks ADD COLUMN phase_order   INTEGER;      -- 1, 2, 3, 4 — controls sequencing
ALTER TABLE tasks ADD COLUMN milestone     TEXT;         -- optional milestone name this task contributes to
ALTER TABLE tasks ADD COLUMN epic_id       TEXT;         -- groups tasks that belong to the same epic
ALTER TABLE tasks ADD COLUMN blocked_by    TEXT;         -- JSON array of task_ids that must complete first

CREATE INDEX IF NOT EXISTS idx_tasks_epic_id ON tasks(epic_id);
CREATE INDEX IF NOT EXISTS idx_tasks_phase ON tasks(phase, phase_order);
```

- [ ] Update `Task` struct:

```rust
pub struct Task {
    // ... existing fields ...
    pub phase: Option<String>,
    pub phase_order: Option<i32>,
    pub milestone: Option<String>,
    pub epic_id: Option<String>,
    pub blocked_by: Option<Vec<String>>,  // deserialized from JSON
}
```

- [ ] Add DB functions:
  - `get_epic_tasks(epic_id: &str) -> Result<Vec<Task>>` — all tasks in an epic, ordered by `phase_order`, then by `id`
  - `get_phase_summary(epic_id: &str) -> Result<Vec<PhaseSummary>>` — count per phase and their completion status
  - `get_blocked_tasks() -> Result<Vec<Task>>` — tasks with `blocked_by` entries where blocking tasks are not done
  - `unblock_ready_tasks(completed_task_id: &str) -> Result<Vec<String>>` — called after a task completes; returns IDs of tasks whose blockers are now all resolved

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct PhaseSummary {
    pub phase: String,
    pub phase_order: i32,
    pub milestone: Option<String>,
    pub total_tasks: i32,
    pub done_tasks: i32,
    pub in_progress_tasks: i32,
    pub ready_tasks: i32,
    pub blocked_tasks: i32,
    pub is_complete: bool,        // true when done_tasks == total_tasks
    pub is_unlocked: bool,        // true when previous phase is complete
}
```

---

### 5.2 Blocker Resolution Hook

**File:** `src-tauri/src/commands/task_commands.rs`

When a task is marked `done`, automatically unblock dependent tasks.

- [ ] In `update_task_status` handler, after status is set to `Done`:

```rust
// Unblock dependent tasks
let unblocked = db.tasks.unblock_ready_tasks(&task_id).await?;
if !unblocked.is_empty() {
    for unblocked_id in &unblocked {
        app.emit("task:unblocked", UnblockedEvent { task_id: unblocked_id.clone() })?;
    }
    // Kick the scheduler — there may now be ready tasks for workers
    scheduler.kick_all_workers().await?;
}

// Check if the phase is now complete
if let Some(epic_id) = &task.epic_id {
    let phase_summary = db.tasks.get_phase_summary(epic_id).await?;
    let completed_phase = phase_summary.iter().find(|p| p.is_complete && p.phase == task.phase.as_deref().unwrap_or(""));
    if let Some(phase) = completed_phase {
        app.emit("task:phase-complete", PhaseCompleteEvent {
            epic_id: epic_id.clone(),
            phase: phase.phase.clone(),
            milestone: phase.milestone.clone(),
        })?;
    }
}
```

---

### 5.3 MCP Tools for Supervisor

**File:** `src-tauri/src/mcp_server/server.rs`

The Supervisor uses these tools when decomposing an epic into phases.

- [ ] `create_epic(title, description) -> { epic_id }` — creates a parent record (can reuse tasks table with a special type, or add a minimal `epics` table — see 5.4)
- [ ] `create_phased_task(epic_id, phase, phase_order, title, priority, task_type, blocked_by[]) -> { task_id }` — creates a task with phase metadata and optional blockers
- [ ] `get_epic_progress(epic_id) -> { phases: PhaseSummary[] }` — Supervisor checks overall epic state
- [ ] `get_ready_tasks_in_phase(epic_id, phase) -> { tasks[] }` — returns tasks in a specific phase that are ready to assign (blockers resolved)
- [ ] Add all four tools to Supervisor-allowed tools list

---

### 5.4 Epic Record (Optional Minimal Table)

**File:** `src-tauri/src/db/tasks.rs`

Epics are not full tasks — they are parent groupings. A minimal approach: reuse tasks with `task_type = 'Epic'` and `status` tracks whether the epic as a whole is done.

- [ ] Add `Epic` variant to `TaskType` enum:

```rust
pub enum TaskType {
    Feature,
    Bug,
    Refactor,
    Docs,
    Chore,
    Epic,       // NEW: parent grouping for phased work
}
```

- [ ] Epic tasks: `assigned_worker = NULL`, `phase = NULL`. Subtasks have `epic_id = epic_task.id`
- [ ] Epic auto-completes when all subtasks are done: hook in `unblock_ready_tasks` — after resolving blockers, check if all `epic_id` tasks are done and mark the epic task as `Done` if so

---

### 5.5 Tauri Commands

**File:** `src-tauri/src/commands/task_commands.rs` (extend existing)

- [ ] `get_epic_tasks(epic_id: String) -> Result<Vec<Task>>`
- [ ] `get_epic_phase_summary(epic_id: String) -> Result<Vec<PhaseSummary>>`
- [ ] `create_phased_task(epic_id, phase, phase_order, title, priority, task_type, blocked_by) -> Result<Task>`
- [ ] Register in `commands/mod.rs`

---

### 5.6 Frontend: Phase View in AutoDev Panel

**File:** `src/tabs/AutoDevPanel.tsx` (extend the Task Board section)  
**File:** `src/components/dashboard/EpicPhaseView.tsx` *(new file)*

- [ ] `EpicPhaseView.tsx` — renders a phased progress view for an epic:

```
EPIC: "Implement /health endpoint with metrics dashboard"
─────────────────────────────────────────────────────────
  [Phase 1: Foundation]  ████████████ 100%  ✅ MILESTONE: API scaffold
  [Phase 2: Core]        ██████░░░░░░  52%  🟡 IN PROGRESS
  [Phase 3: Integration] ░░░░░░░░░░░░   0%  🔒 BLOCKED (waiting for Phase 2)
  [Phase 4: QA]          ░░░░░░░░░░░░   0%  🔒 BLOCKED (waiting for Phase 3)
```

- [ ] Clicking a phase expands to show individual task cards within that phase
- [ ] `BLOCKED` phases show which phase they're waiting on
- [ ] `✅ MILESTONE` shown when phase reaches its milestone marker
- [ ] Listens for `task:phase-complete` and `task:unblocked` events to update in real time

- [ ] In the Task Board section of AutoDev Panel, when a task has an `epic_id`, show a small "Epic" badge on the task card that links to the Epic Phase View
- [ ] Add an "Epics" tab within the AutoDev Panel task board (alongside the existing kanban columns)

---

## 6. MCP Connection Health Monitor

**Ruflo Reference:** Plugin `healthCheck?(): Promise<HealthCheckResult>` lifecycle in `v3/@claude-flow/plugins/src/core/plugin-interface.ts` lines 1093-1096  
**Goal:** Periodic liveness pings on connected MCP servers. When a connection drops (e.g., the GitHub MCP server crashes, network blip), the UI shows an alert and the system attempts automatic reconnection — instead of silently failing on the next tool call.  
**Does not touch submodules.** This monitors connections *to* external MCP servers (GitHub, Slack, custom) configured by the user. The health logic lives entirely in `src-tauri/src/mcp_server/`.

**Depends On:** `mcp_server/server.rs`, `sidecar/client.rs`, `commands/sidecar_commands.rs`, `src/stores/sidecarStore.ts`

---

### 6.1 Connection Registry

**File:** `src-tauri/src/mcp_server/health_monitor.rs` *(new file)*

- [ ] Define connection state types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum McpConnectionState {
    Connected,
    Degraded,       // responded slowly or with errors
    Disconnected,
    Reconnecting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerHealth {
    pub server_name: String,
    pub endpoint: String,
    pub state: McpConnectionState,
    pub last_ping_at: Option<String>,
    pub last_success_at: Option<String>,
    pub consecutive_failures: u32,
    pub avg_latency_ms: f64,
    pub error_message: Option<String>,
}
```

- [ ] `McpHealthMonitor` struct:

```rust
pub struct McpHealthMonitor {
    connections: Arc<RwLock<HashMap<String, McpServerHealth>>>,
    ping_interval_secs: u64,     // default: 30
    failure_threshold: u32,      // default: 3 consecutive failures = Disconnected
    reconnect_delay_secs: u64,   // default: 10
}
```

- [ ] `impl McpHealthMonitor`:
  - `register_server(name: &str, endpoint: &str)` — adds a server to the registry
  - `unregister_server(name: &str)` — removes from registry (when user removes an MCP config)
  - `get_all_health() -> Vec<McpServerHealth>` — returns current state of all registered servers
  - `async fn start_monitoring(self: Arc<Self>, app: AppHandle)` — spawns background Tokio task

---

### 6.2 Ping Loop

**File:** `src-tauri/src/mcp_server/health_monitor.rs`

- [ ] The monitoring task (spawned once on startup, runs forever):

```rust
async fn monitoring_loop(monitor: Arc<McpHealthMonitor>, app: AppHandle) {
    let mut interval = tokio::time::interval(
        Duration::from_secs(monitor.ping_interval_secs)
    );

    loop {
        interval.tick().await;
        let server_names: Vec<String> = monitor.connections.read().await.keys().cloned().collect();

        for name in server_names {
            let result = monitor.ping_server(&name).await;
            monitor.process_ping_result(&name, result, &app).await;
        }
    }
}
```

- [ ] `async fn ping_server(name: &str) -> Result<PingResult, PingError>`:
  - For HTTP/SSE MCP servers: send `GET /health` or `GET /ping` with 5-second timeout
  - For stdio-based MCP servers: send a `ping` JSON-RPC call
  - Record latency
  - Return `PingResult { latency_ms: u64 }` or `PingError::Timeout | PingError::ConnectionRefused | PingError::InvalidResponse`

- [ ] `async fn process_ping_result(name: &str, result, app: &AppHandle)`:
  - On success: reset `consecutive_failures = 0`, update `last_success_at`, `avg_latency_ms`, set state to `Connected`
  - On failure: increment `consecutive_failures`
    - If `consecutive_failures >= threshold` AND previous state was `Connected`: set state to `Disconnected`, emit `mcp:connection-lost`
    - If already `Disconnected`: set state to `Reconnecting`, attempt reconnect (see 6.3)
  - If latency > 2000ms but request succeeded: set state to `Degraded`, emit `mcp:connection-degraded`

---

### 6.3 Auto-Reconnect

**File:** `src-tauri/src/mcp_server/health_monitor.rs`

- [ ] `async fn attempt_reconnect(name: &str, endpoint: &str, app: &AppHandle)`:
  - Wait `reconnect_delay_secs` before trying
  - Try to re-establish the MCP client connection (reconnect logic depends on transport type):
    - For HTTP: simply retry a ping — if successful, mark as `Connected`
    - For stdio: the MCP process may have crashed — emit `mcp:requires-restart` event to frontend (cannot restart it automatically without knowing the original launch command)
  - If reconnect succeeds: emit `mcp:reconnected` event
  - If reconnect fails: emit `mcp:reconnect-failed`, stay in `Reconnecting` state, retry after exponential backoff (max 5 min)

---

### 6.4 Startup Wiring

**File:** `src-tauri/src/main.rs`

- [ ] After sidecar health checks pass (in the `FleetReady` startup phase), start the health monitor:

```rust
let health_monitor = Arc::new(McpHealthMonitor::new(30, 3, 10));

// Register all configured MCP servers from settings
for server in config.mcp_servers.iter() {
    health_monitor.register_server(&server.name, &server.endpoint).await;
}

// Start monitoring in background
tokio::spawn(McpHealthMonitor::start_monitoring(
    health_monitor.clone(),
    app_handle.clone()
));

// Store reference for later use (re-register when user adds MCP servers in Settings)
app_state.mcp_health_monitor = health_monitor;
```

- [ ] When user adds/removes an MCP server in Settings: call `health_monitor.register_server()` or `unregister_server()`

---

### 6.5 Tauri Events

**File:** `src-tauri/src/events.rs`

- [ ] Add event definitions:
  - `mcp:health-update` — periodic broadcast of all MCP server states (every ping cycle)
  - `mcp:connection-lost` — `{ server_name, endpoint, consecutive_failures }`
  - `mcp:connection-degraded` — `{ server_name, latency_ms }`
  - `mcp:reconnected` — `{ server_name }`
  - `mcp:reconnect-failed` — `{ server_name, attempt }`
  - `mcp:requires-restart` — `{ server_name }` — stdio server needs manual restart

---

### 6.6 Tauri Commands

**File:** `src-tauri/src/commands/sidecar_commands.rs` (extend existing)

- [ ] `get_mcp_health() -> Result<Vec<McpServerHealth>>` — returns current health for all registered MCP servers
- [ ] `force_reconnect_mcp(server_name: String) -> Result<()>` — manual reconnect trigger from UI
- [ ] Register in `commands/mod.rs`

---

### 6.7 Frontend: MCP Health Indicator

**File:** `src/stores/sidecarStore.ts` (extend existing)

- [ ] Add to sidecar store state:

```typescript
mcpServerHealth: McpServerHealth[],
mcpDisconnected: string[],   // server names currently disconnected
```

- [ ] Listen for `mcp:health-update` event — update `mcpServerHealth` in store
- [ ] Listen for `mcp:connection-lost` — add to `mcpDisconnected`, show alert
- [ ] Listen for `mcp:reconnected` — remove from `mcpDisconnected`

**File:** `src/components/sidecar/McpHealthPanel.tsx` *(new file or extend McpToolBrowser.tsx)*

- [ ] Show connection state badges next to each MCP server in the Sidecar Panel:
  - `🟢 Connected (12ms avg)`
  - `🟡 Degraded (2,340ms avg)`
  - `🔴 Disconnected (3 failures) [Reconnect]`
  - `🔄 Reconnecting...`

- [ ] "Reconnect" button calls `invoke('force_reconnect_mcp', { server_name })`

**File:** `src/components/layout/StatusBar.tsx` (extend existing)

- [ ] Add a small MCP health indicator to the status bar: if any server is `Disconnected`, show `⚠️ MCP` badge in orange/red

---

## 7. Browser Automation QC Skill

**Ruflo Reference:** Browser automation tools in `v3/@claude-flow/cli/src/mcp-tools/browser-tools.ts` lines 1-705 (25 tools via `agent-browser` CLI)  
**Phase:** 3+ (not required for initial fleet launch)  
**Goal:** QC Agent can run visual regression and end-to-end tests on frontend PRs by controlling a browser via Playwright. Implemented as a **skill** (not a built-in tool) so it requires no changes to the agent engine or MCP server.  
**Does not touch submodules.** Implemented entirely as a skill YAML + optional Rust helper command for Playwright install detection.

**Depends On:** `skills/engine.rs`, `skills/loader.rs`, `agent/builtin_tools/bash.rs` (existing), `agent/engine.rs` (no changes)

---

### 7.1 Playwright Detection Helper

**File:** `src-tauri/src/commands/browser_commands.rs` *(new file)*

Since browser automation depends on Playwright being installed, we need a detection command so the UI can warn users.

- [ ] `check_playwright_available() -> Result<BrowserAvailability>`:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct BrowserAvailability {
    pub playwright_installed: bool,
    pub playwright_version: Option<String>,
    pub browsers: Vec<BrowserInfo>,  // e.g., chromium, firefox, webkit
    pub npx_available: bool,
}
```

- [ ] Implementation:
  - Check `npx playwright --version` — if exit 0, playwright is available
  - Check `npx playwright install --dry-run` to list available browsers
  - Parse output into `BrowserAvailability`
  
- [ ] `install_playwright_browsers(browser: String) -> Result<()>`:
  - Runs `npx playwright install {browser}` as a subprocess
  - Streams stdout via `bash` tool pattern (reuse existing `bash.rs` streaming)
  
- [ ] Register in `commands/mod.rs`

---

### 7.2 Browser Skill Definition

**File:** Written into `~/.autodev/skills/browser-test/SKILL.md` on first run (via `skills/engine.rs`)

The skill is installed as a system skill, similar to how existing system skills are written to disk. It instructs the agent to use the existing `bash` tool to run Playwright commands.

- [ ] Add this skill to the system skill set in `skills/engine.rs` (alongside existing built-in skills):

```markdown
---
name: browser-test
description: Run browser automation tests using Playwright. Use for visual regression testing, E2E tests on frontend PRs, and verifying UI after deployments.
argument-hint: "<url> [--screenshot] [--test-file path]"
allowed-tools: bash, read_file, write_file
---

# Browser Test Skill

You have access to Playwright for browser automation. Use `npx playwright` commands via the `bash` tool.

## Setup Check
First verify Playwright is installed:
```bash
npx playwright --version
```
If not installed, run: `npx playwright install chromium`

## Visual Regression
Take a screenshot and compare:
```bash
# Take screenshot of the current state
npx playwright screenshot --full-page {url} /tmp/autodev-screenshot-{timestamp}.png

# If baseline exists, compare using Playwright's comparison
npx playwright test --grep "visual"
```

## Running E2E Tests
```bash
# Run all playwright tests in the project
npx playwright test {test_file_or_dir}

# Run in headed mode for debugging (only if display available)
npx playwright test --headed {test_file_or_dir}

# Run specific browser
npx playwright test --project=chromium {test_file_or_dir}
```

## Common Patterns
- Navigate to URL and take screenshot for visual review
- Fill forms and check validation
- Click through critical user flows
- Assert page titles and key text content

## Output
Always return:
1. Screenshot path (if taken)
2. Test pass/fail summary
3. Specific failures with selectors and expected vs actual values
```

- [ ] The skill file is written to disk during the `Skills` startup phase (same as other system skills)

---

### 7.3 QC Recipe Integration

**File:** `~/.autodev/recipes/qc-agent.yaml` (the system QC recipe)

- [ ] Update QC recipe instructions to optionally invoke the browser-test skill for frontend PRs:

```yaml
instructions: |
  You are the AIRS QC Agent.
  
  For ALL PRs:
  1. Use analyze_diff to assess risk.
  2. Run backend tests appropriate to the changed files.
  
  For PRs where worker_type is 'worker-frontend' OR changed files include .tsx/.ts/.css:
  3. Check if the PR includes Playwright test files.
     - If YES: run `/browser-test` skill with the test directory.
     - If NO and risk_level is "high": use bash to run `npx playwright test` against the staging URL if available.
  4. Include screenshot paths in your review comment if screenshots were taken.
  
  Always include analyze_diff risk_score in your final review comment.
```

---

### 7.4 Settings: Browser Automation Section

**File:** `src/tabs/Settings.tsx`

- [ ] Add a "Browser Testing" settings section within the QC settings group:
  - Playwright status indicator (calls `check_playwright_available()` on load)
  - "Install Playwright Browsers" button → calls `install_playwright_browsers('chromium')`
  - Staging URL input — the URL QC will navigate to for visual testing (e.g., `http://localhost:3000`)
  - Toggle: "Enable browser testing for frontend PRs" (default: off until Playwright is confirmed installed)

- [ ] If Playwright is not installed, show a banner: "⚠️ Browser testing requires Playwright. Click Install to set it up."

---

### 7.5 Skill Registration in Skills Registry

**File:** `src-tauri/src/skills/registry.rs`

- [ ] Ensure `browser-test` skill is marked as `is_system = true` so it cannot be deleted from the UI
- [ ] Mark it as `requires_capability: ["playwright"]` — a new optional field on skills that the skills palette uses to show a warning badge if the capability is not available
- [ ] In `SkillsPalette.tsx`: if a skill has `requires_capability` and `check_playwright_available()` returns `playwright_installed: false`, show a `⚠️ Requires setup` badge next to the skill name

---

## Appendix: Implementation Order

The features above are listed in recommended implementation order. Each is independent of the others except where noted.

| # | Feature | Effort | Priority | Depends On |
|---|---------|--------|----------|------------|
| 2 | PID-Based Orphan Detection | Low | High | tasks.db |
| 6 | MCP Connection Health Monitor | Medium | High | sidecar system |
| 4 | Budget Circuit Breaker | Medium | High | agent/engine.rs |
| 5 | Phase-Based Task Decomposition | Medium | Medium | tasks.db |
| 1 | Semantic Task Routing | Medium | Medium | tasks.db, routing_history |
| 3 | Diff Risk Scoring | Medium | Medium | git/diff.rs, provider |
| 7 | Browser Automation QC Skill | Low-Medium | Low (Phase 3+) | skills system |

**Suggested sprint order**: PID Orphan (2) → MCP Health (6) → Budget (4) → Phase Decomposition (5) → Task Routing (1) → Diff Risk (3) → Browser (7, Phase 3+)
