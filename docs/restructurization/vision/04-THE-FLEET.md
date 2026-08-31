# 04 — The Fleet: dispatch, tasks, verification, delivery, coordination

| | |
|---|---|
| **Parent** | `00-…-MASTER-PLAN.md` §Part 1 (INV-5, INV-6, INV-8), §Part 1B (M-11…M-16, M-31…M-34), §Part 3.4, §Part 6 (P4) |
| **Locks** | Hands **ADR-02-01**, **ADR-02-02**, **ADR-02-04**, **ADR-02-06**, **ADR-02-07** to `00 §Part 8` |
| **Status** | PLANNED |
| **Grounding** | `_grounding/G9` (1,525 ln, 54 findings) · `G3a` (SK-01…SK-50) · `G3d` (F-02…F-55) · `G2b` (budget, subagents) · `G1 §C.4` |

---

## What this is

The Supervisor → Worker → QC pipeline: how work is created, decomposed, assigned, claimed, executed, verified and delivered — plus the coordination capabilities Razi has ruled are **preserved and completed, never cut** (ADR-02-06).

**The finding that shapes this spec: the fleet's machinery is largely built and largely inert.** Of the six coordination capabilities audited, exactly one works end to end. The rest are complete implementations with a missing caller, a missing column, or a registration that excludes the one agent that needs them.

**The subtle failures this subsystem is prone to:**

1. **A feedback loop that never closes.** Routing affinity records outcomes from a UI command that autonomous operation never calls, so the learning gate never opens.
2. **A capability registered away from its only consumer.** Epic decomposition is PA-only; the Supervisor reaches tools solely through MCP, which excludes it.
3. **Identity supplied by exactly one caller.** Worker messaging works today only because the MCP adapter sets `agent_role` — and that adapter is being deleted.
4. **A gate whose evidence is a 15-minute in-memory map.**

---

## 1. ⚠ A collision between two decided ADRs — read this first

**ADR-02-06 preserves worker↔worker messaging. ADR-03-02 deletes the MCP HTTP server. Executed in that order, messaging dies silently.**

The chain (`G9 F-XCUT-01`/`F-ACP-01`, CONFIRMED):

- All three ACP tools hard-refuse without a sender identity — `send_worker_message.rs:129-137`.
- `ToolContext.agent_role` is `None` at **all three in-house construction sites** — `agent/tool_executor.rs:89`, `:252`, `:316`.
- It is set in exactly **one** place in the entire codebase: `mcp_server/tool_adapter.rs:280`.

So worker↔worker messaging functions today **only** because fleet workers reach their tools through the MCP server. Delete that server without first fixing `ToolContext`, and every ACP call begins refusing — with no error surfaced to the user, because the refusal is a tool-level response the worker simply narrates around.

**Resolution, and it is a sequencing constraint on P4/P8:** `ToolContext` must carry a truthful `agent_role` from the in-house construction sites **before** the MCP server is deleted. This is already specified as M-09 / spec `01 §4` — the point here is that it is **not optional and not deferrable**, because it is what stops a preserved capability from being destroyed by an unrelated deletion.

The same three hardcoded fields also disable the subagent recursion guard and the `qc_review` build-evidence gate. **One fix, three capabilities.**

---

## 2. Task dispatch (M-11, ADR-02-01)

**Decided: (A)+(C) — event-driven dispatch, with worker state as an exhaustively-matched Rust enum, plus fixed worker/supervisor states.**

### 2.1 Why the enum is the fix, not vigilance

`scheduler.rs:227` matches worker status as a string and fires only for `idle` / `active` / `error`. `run_recipe` writes two statuses that match neither — `fallback_idle` (`runner.rs:594`) and `mode1_idle_no_provider` (`:625`) — so they fall to a `_` arm that logs and skips. **A worker that ever enters either state never ticks again, even after the condition clears** (`SK-01`).

A Rust enum with an exhaustive `match` makes this class impossible **inside Rust**: adding a variant without handling it fails compilation.

⚠ **But the review is right that "structurally impossible" was an overclaim, and the qualification matters.** `workers.status` is **unconstrained `TEXT`**, and `src-shared` writes arbitrary `&str` into it **from a second process** (`airsctl`). So the enum does not eliminate the failure — **it relocates it to `FromStr`**, where an unrecognised string arrives from the database rather than from a missing match arm.

The fix is therefore two parts, and the second is the one that was missing: the enum, **and** a `FromStr` that treats an unknown status as a **loud error, never a silent default**. Note `TaskPriority::from_str` already demonstrates the wrong version of this — every unrecognised string silently becomes `P2` (`G0 §5.8`). A `CHECK` constraint on the column closes it at the storage layer as well.

Adjacent, and equally fatal: `scheduler.rs:220` collapses a `WorkersDb` error to `None` via `.ok().flatten()`, and `None` matches the *paused/deleted* arm, which `break`s the loop. **One transient `SQLITE_BUSY` permanently terminates a worker's schedule** while the UI still shows it as scheduled (`SK-02`).

### 2.2 Event-driven, with a reconciler

A task-assigned event kicks the worker directly. Cron is demoted to a low-frequency **reconciliation sweep** for stranded work, not the dispatch mechanism. This also removes the 4–6 full task-board reads per worker per tick that polling costs today (`SK-33`).

Push dispatch has its own trap to fix: `kick_inflight` is released only by the spawned task's final line, so an abort or panic **locks that worker out of push dispatch for the process lifetime** (`SK-41`), and the kicked run is awaited with no timeout (`SK-42`). Both are RAII-and-timeout fixes, matching INV-9.

---

## 3. The task lifecycle and the claim (M-12)

### 3.1 The claim is one atomic statement, and it names the owner

`db/tasks.rs:895-902` — `claim_task_with_pid` is `UPDATE … WHERE id=? AND status='assigned'` and **never checks `assigned_worker`**. Any worker can claim a task assigned to another and overwrite the owner. `update_routing_outcome` then matches on `task_id` alone (`:990`), so `routing_history` keeps the *old* worker's name (`F-55`).

The one-task-per-worker guard is a separate, non-atomic `SELECT COUNT` that **fails open** via `.unwrap_or(0)` (`tasks.rs:877-885`).

**The rule: one statement, owner in the predicate, guard in the same statement.** Currently latent only because no outcomes are recorded at all (§5) — which is exactly why it must be fixed *before* affinity is switched on, or the model trains on corrupted labels.

### 3.2 Epics are not tasks

Epics are inserted `status='ready'` (`tasks.rs:1199-1206`), and **no dispatch or claim path filters `task_type='epic'`** (`assign_task.rs:96-99`, `assign_tasks_batch.rs:160`, `tasks.rs:895-902`). An epic can be handed to a worker as if it were a unit of work (`F-EPI-02`).

### 3.3 Reconciliation is dead

The timestamp-format defect appears at **two** sites here: `orphan/mod.rs:183` (the 4-hour orphan TTL can never fire) and `commands/task_commands.rs:133-137` (`get_qc_queue` reports every task's age as 0 s, so the stalled-review alert can never fire). Both parse SQLite `datetime('now')` with an RFC 3339 parser and swallow the failure. Frontend twins exist at `FleetChat.tsx:235` and `AutoDevPanel.tsx:41,51,383`. Discipline in spec `08 §3.3`.

---

## 4. Build verification and delivery (M-13, M-14, M-15)

### 4.1 Verification is deterministic, with the model as a last-resort debugger

**Decided (ADR-02-02): (A) as the main path, with (B) engaging only after three consecutive failures.**

```
attempt 1 ─┐
attempt 2 ─┼─ DET: detect ecosystem → Rust-owned argv → exit code → persist evidence
attempt 3 ─┘
    │  three consecutive failures
    ▼
ORGAN: the model diagnoses the failure  ← it never reports the verdict
    │
    ▼
DET: re-run the deterministic verification
```

The model is a **debugger, not a judge**. The exit code of a build is a fact; only the diagnosis of *why* is judgement. That distinction is what BUG-COMPILE was: today the model authors the command, and its exact byte-level syntax gates the pipeline.

### 4.2 Build evidence is persisted, not remembered

Today it is a process-global `HashMap` with a 15-minute `Instant` TTL (`build_evidence.rs:63-66`), written only by `bash.rs:114`, read only by `update_task_status_tool.rs:141`, and **never persisted**. Restart the app and every task's proof of a passing build evaporates.

**It becomes a table** — task id, command, exit code, timestamp, platform — and the `qc_review` guard reads *that*.

### 4.3 Delivery has a destination and a handoff

`load_workspace_dir()` returns `""` on unset **or error**, with a docstring claiming "Never fails" — so each consumer invented a fallback: `recipes/runner.rs:175-190` → `~/.autodev/agents/<slug>/`; `mcp_server/tool_adapter.rs:70-88` → `current_dir()`. **Live artifacts exist on the development machine in `~/.autodev/agents/worker-backend/`.**

**One resolver that fails loudly** (INV-5), and a real handoff: reveal, open in the IDE tab, and **run** the artifact. Today `open_in_ide` emits `ide:navigate`, which has zero listeners, and `e2e_probe` boots the app and then kills it — there is no step at which the user receives something runnable.

---

## 5. The coordination capabilities (M-31…M-34, ADR-02-06)

Razi: *"I want all of those capabilities in the AutoDev application."* One of six works.

| # | Capability | Verdict | The single thing that is wrong |
|---|---|---|---|
| 1 | **Supervisor escalation** | ✅ **WORKS** — the strongest capability in scope. Writer → store → reader → resolver → `worker:request` event → `WorkerRequestsWidget`, and Supervisor Step 1a genuinely drains it (`defaults.rs:229-239`) | **No expiry.** One unresolvable request plus the recipe's "STOP after this step" (`:239`) **starves dispatch forever** |
| 2 | **ACP worker↔worker messaging** | ⚠ **Works today, dies on port** | §1. Plus: `qc-agent`/`qc-dispatcher` accept mail **no recipe reads and no `workers.db` row backs**, so every broadcast leaves 2 dead rows; and listing flips `pending→read` while `list_stale_worker_messages` (`tasks.rs:799`) has **zero callers**, making delivery at-most-once while `defaults.rs:400` promises "lossless" |
| 3 | **Routing / affinity** | ❌ **INERT** — complete machinery that has never influenced a decision | Outcomes are written **only** by the Tauri command (`task_commands.rs:51-63`); `update_task_status_tool.rs` never calls `record_outcome_auto`. In autonomous operation `outcome` is **NULL forever**, so the `>= 3` learning gate (`scorer.rs:70`) never opens |
| 4 | **Epic / phase decomposition** | ❌ **INERT and UNREACHABLE** | `create_epic`/`create_phased_task` are registered **PA-only** (`mod.rs:222-223`) and deliberately excluded from MCP (`server.rs:140-141`) — but the Supervisor is `Mode2Only` and reaches tools **only** through MCP. **The Supervisor cannot create an epic.** Its recipe decomposes with flat `create_task` (`defaults.rs:288-294`). And `unblock_ready_tasks` has the same command-only defect as (3), so dependents never unblock and epics never auto-complete — the Supervisor reports "Board clean — idle" |
| 5 | **Interface contracts / QC** | ⚠ **MIXED** | INTERFACE.md is advisory — nothing verifies the consumer read it. `analyze_diff` **never runs**: QC Phase 0 is gated on `repo_path` "in task metadata", and **there is no such column on `tasks`**; the dispatcher passes only `task_id` (`defaults.rs:1826`). `spawn_qc_review` is well-built but sidecar-bound |
| 6 | **Per-task budget** | ❌ **FULLY INERT** | `with_task_budget` (`engine.rs:73-77`) has **zero callers**, so the circuit breaker (`:148-166`), usage persistence (`:216-230`), escalation (`:480-536`) and the `agent:budget-exceeded` event are all dead. Compounding it: **no recipe ever calls `set_task_budget`**, and the Settings "default worker budget" has no Rust reader at all |

### 5.1 `suggest_worker` currently recommends the Supervisor

Ties plus a stable sort plus `ORDER BY is_system_defined DESC, id ASC` (`db/workers.rs:209`) make `suggest_worker` return **`supervisor`** (`F-ROU-02`). And the static keyword layer beneath it is unreachable dead code: it requires `conf > 0.85` while the maximum confidence any pattern declares is exactly `0.85` (`scorer.rs:121` vs `patterns.rs:15`).

### 5.2 The ordering constraint

**M-12 (atomic claim) lands before affinity is enabled.** Task theft is latent only because no outcomes are recorded; turning on recording without fixing the claim means the model learns from wrong attributions — worse than not learning.

---

## 6. Worker identity and persona (ADR-02-07 — decided: (C))

`workers.db` owns identity, model and state — queryable, mutable, UI-editable. The recipe file owns the prompt — versionable, diffable, shareable. `workers.db` holds a **reference** (`recipe_path`), never a copy. One owner per fact (INV-10).

**Binding condition:** `recipes/defaults.rs:45` overwrites all system recipes on every launch, silently reverting user edits (`SK-49`). Personas are un-editable until that is fixed, so it is part of this work.

Seven workers ship as a **default team, user-editable** (OQ-2): Backend, Frontend, Platform, MCP Builder, Cleanup, Supervisor, QC.

---

## 7. Determinism balance

| Step | Balance |
|---|---|
| Dispatch, claim, state transitions | `DET` |
| Build verification | `DET` (+ `ORGAN` only on the 3rd consecutive failure) |
| Delivery and handoff | `DET` |
| Routing / affinity scoring | `MATH` |
| Escalation queue mechanics | `DET` |
| Task decomposition, code, QC review | `ORGAN` |

---

## 8. Test gates

| Gate | Asserts | Catches |
|---|---|---|
| **G-DISPATCH-EXHAUSTIVE** | Every worker state is handled; adding a state without handling it **fails compilation** | SK-01's dead latch |
| **G-DISPATCH-RESILIENT** | A transient DB error does not terminate a worker's schedule | SK-02 |
| **G-CLAIM-ATOMIC** | Concurrent claims: exactly one wins; a worker cannot claim another's task | F-55 |
| **G-EPIC-NOT-TASK** | An epic can never be assigned or claimed as work | F-EPI-02 |
| **G-EPIC-REACHABLE** | The Supervisor can create an epic **through the tool surface it actually has** | F-EPI-01 |
| **G-UNBLOCK** | Completing a dependency unblocks dependents and progresses the epic, in autonomous operation | F-EPI-03 |
| **G-BUILD-EVIDENCE** | Evidence survives a restart; `qc_review` is refused without a passing record; a **passing build is never recorded as failed** | The in-memory map; the PowerShell/cmd mismatch |
| **G-BUILD-ESCALATE** | Three consecutive failures engage the debug step; the verdict still comes from the exit code | ADR-02-02's shape |
| **G-DELIVER** | Agent output lands under the resolved workspace, never `~/.autodev/agents/` or the CWD; the handoff opens and runs it | F-DEL |
| **G-ACP-IDENTITY** | Worker→worker messaging works **with the MCP server deleted** | §1 — the capability-killing deletion |
| **G-ACP-DELIVERY** | A message is delivered once and only once; unacknowledged messages are recoverable | The at-most-once/"lossless" contradiction |
| **G-ESCALATION-LIVENESS** | An unresolved request cannot starve dispatch indefinitely | The Supervisor STOP starvation |
| **G-AFFINITY-CLOSES** | Autonomous completion records an outcome, and after N outcomes routing measurably changes | F-ROU-01 |
| **G-SUGGEST** | `suggest_worker` never returns a system-defined agent for implementation work | F-ROU-02 |
| **G-BUDGET-FIRES** | A task exceeding its budget trips the breaker and surfaces to the user | F-BUD-01 |
| **G-CHECKPOINT-BLOCKS** | ⚠ **ADDED at rev 0.9 (G-08, M-43).** A **failed checkpoint blocks the write** rather than proceeding silently; per-write undo restores prior content; each task has its own worker branch, and the two systems never disagree about current state | `db/checkpoints.rs` not being compiled, and `auto_checkpoint_before_write` returning `()` so a failure is invisible |
| **G-QC-ANALYZE** | The QC reviewer receives a repo path and `analyze_diff` actually runs — **under the workspace's recorded egress consent** (spec `06 §G-PERM-NETWORK`, corrected). With no consent recorded, QC degrades to the deterministic structural risk score and says so; it does not silently ship the diff, and it does not prompt an unattended machine | F-QC-01; and the gate collision the review caught |

---

> No ADR decision is recorded in this spec. It hands **ADR-02-01**, **ADR-02-02**, **ADR-02-04**, **ADR-02-06** and **ADR-02-07** to `00 §Part 8`.
