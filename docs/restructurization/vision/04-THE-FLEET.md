# 04 — The Fleet: dispatch, tasks, verification, delivery, coordination

| | |
|---|---|
| **Parent** | `00-…-MASTER-PLAN.md` §Part 1 (INV-5, INV-6, INV-8), §Part 1B (M-11…M-16, M-31…M-34), §Part 3.4, §Part 6 (P4) |
| **Locks** | Hands **ADR-02-01**, **ADR-02-02**, **ADR-02-04**, **ADR-02-06**, **ADR-02-07** to `00 §Part 8` |
| **Status** | P4 IMPLEMENTATION ACTIVE — source and behavior re-audited 2026-09-26 |
| **Grounding** | `_grounding/G9` (1,525 ln, 54 findings) · `G3a` (SK-01…SK-50) · `G3d` (F-02…F-55) · `G2b` (budget, subagents) · `G1 §C.4` |

---

## What this is

The Supervisor → Worker → QC pipeline: how work is created, decomposed, assigned, claimed, executed, verified and delivered — plus the coordination capabilities Razi has ruled are **preserved and completed, never cut** (ADR-02-06).

**Current Arcana baseline (2026-09-26):** this document began as a pre-P8 source audit. Its dated findings below are historical evidence, not current behavior claims. P8 removed Python orchestration and the MCP HTTP server. P4 re-audited its active tasks, gates and smokes against the shipped Rust/TypeScript application; the current results and source references are in `docs/restructurization/development/_verify/P4-PREBUILD-DISPATCH-TASKS.md` and `P4-PREBUILD-DELIVERY-COORDINATION.md`.

The active target remains a reliable coding fleet. Current open behaviors include owner-checked atomic claims, strict worker-status hydration, durable task-scoped build evidence, shared-workspace delivery, task-associated checkpoints, autonomous dependency unblocking, routing outcomes, and end-to-end epic use. Do not infer a current defect solely from the older numbered findings in this document.

**The subtle failures this subsystem is prone to:**

1. **A feedback loop that never closes.** Routing affinity records outcomes from a UI command that autonomous operation never calls, so the learning gate never opens.
2. **A capability registered away from its only consumer.** Epic decomposition is PA-only; the Supervisor reaches tools solely through MCP, which excludes it.
3. **Identity supplied by exactly one caller.** Worker messaging works today only because the MCP adapter sets `agent_role` — and that adapter is being deleted.
4. **A gate whose evidence is a 15-minute in-memory map.**

---

## 1. Rust-native ACP identity and delivery

P8 removed the MCP HTTP server. The current `ToolContext.agent_role` is a
required `String`, and the in-house `ToolExecutor` construction sites supply
identity. P4-22/TG4-10 therefore prove the live Rust-native message path,
including truthful sender/recipient checks, rather than redoing deleted MCP
identity plumbing. P4-23/TG4-11 prove the durable inbox contract: delivery is
at-least-once until acknowledgement, stale recovery can replay a message with
the same stable ID, and the consumer applies its effect idempotently. Ack ends
delivery. Arcana makes no exactly-once claim. The old three-site/adapter chain below this heading is retained as
historical audit evidence only.

---

## 2. Task dispatch (M-11, ADR-02-01)

**Decided: (A)+(C) — event-driven dispatch, with worker state as an exhaustively-matched Rust enum, plus fixed worker/supervisor states.**

### 2.1 Strict state at the real database boundary

The current `WorkerStatus` enum is not used by production row hydration:
`Worker.status` is still a `String`, unknown values default to `Created`, and
the persisted column lacks a `CHECK`. The current unconstrained writer is the
Tauri worker-status command; the old `src-shared` second-process writer was
removed. P4-01…03/TG4-01 require typed production boundaries, loud rejection,
and a migrated schema, proven on isolated SQLite copies.

The scheduler also turns a worker DB read error into `None`, then treats `None`
as paused/deleted and breaks the worker schedule. P4-05/TG4-02 require the
worker to survive a transient error and resume on a later tick.

### 2.2 Event-driven, with a reconciler

A task-assigned event kicks the worker directly. Cron is demoted to the
stranded-work reconciliation sweep: **every five minutes and on app start**,
per MDP §14.5. The sweep is a safety net, not the dispatch mechanism.

Push dispatch must release `kick_inflight` on every exit. The run has a
10-minute hard timeout; timeout/cancellation releases the task claim for the
reconciler. Workers are Tokio tasks inside Arcana, so S4-03 injects an
in-process cancellation or panic and proves automatic recovery; it does not
pretend there is a separate worker process (ADR-0068).

---

## 3. The task lifecycle and the claim (M-12)

### 3.1 The claim is one atomic statement, and it names the owner

The current `TasksDb::claim_task_with_pid` updates by task ID and status but
does not include the assigned owner in its predicate. The fleet tool also
prefers the row's existing owner over the caller's identity. P4-07/P4-34 and
TG4-03/S4-05 require the actual worker tool path to reject a wrong-owner claim
and make the load limit part of the atomic claim. Routing outcome recording
must remain ordered after that fix.

The current fleet claim path reads worker load separately and fails open on
read errors. Keep the owner and one-task guard in the same claim statement.

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

Today build evidence remains a process-local map with a short TTL. The
production `update_task_status` reader consumes that transient evidence before
QC review; an app restart loses it. P4-12…14 persist evidence keyed by task and
make the QC guard read the same durable record.

**It becomes a table** — task id, command, exit code, timestamp, platform — and the `qc_review` guard reads *that*.

### 4.3 Delivery has a destination and a handoff

The current setting reader still collapses unset and database error to an empty
string; the recipe runner falls back to `~/.arcana/agents/<slug>`. MCP adapter
fallback behavior was removed with the sidecar. P4-17/18 require one resolver
that fails loudly and every write path to stay under its result.

**One resolver that fails loudly** (INV-5), and a real handoff: reveal, open in
the IDE tab, and **run** the artifact. The source audit found an `ide:navigate`
emitter but no literal listener; P4-21/TG4-09 and S4-02 must prove the full
event consumer and run-from-IDE path instead of treating that search as final
proof.

---

## 5. The coordination capabilities (M-31…M-34, ADR-02-06)

Owner direction: preserve coding capabilities in the Arcana application. The
following table reflects the 2026-09-26 P4 audit; all verdicts require the
consumer-backed proofs named in the active MDP and gate register.

| # | Capability | Verdict | The single thing that is wrong |
|---|---|---|---|
| 1 | **Supervisor escalation** | Request writer/store/reader and Supervisor consumer exist | P4-24/TG4-12 prove an unresolved request cannot starve dispatch and that the fleet continues making progress. |
| 2 | **ACP worker↔worker messaging** | Native identity path is shipped; behavior remains open | P4-22/23 prove the production worker inbox, recipient roster, acknowledgement and stale-message recovery. The QC inbox consumer/recipient contract is grounded on the current `qc-agent` recipe, not the historical `qc-dispatcher` slug. |
| 3 | **Routing / affinity** | Outcome learning is incomplete | Autonomous task completion must record the actual worker's outcome after owner-checked claims; P4-07/P4-25/TG4-03/TG4-13 prove the attribution and learning path. |
| 4 | **Epic / phase decomposition** | Schemas are registered; successful Supervisor use is unproven | Epic tools are available in the FastDevelopment recipe registry, but the Supervisor recipe currently decomposes with flat `create_task`. P4-27/TG4-05 prove registration plus real epic use; P4-28/TG4-06 prove autonomous unblocking and epic progress. |
| 5 | **Interface contracts / QC** | QC review exists; production evidence and consent are incomplete | P4-12…16/TG4-07…08 and TG3-26/TG4-16 prove task-scoped build evidence, recorded egress consent, and the deterministic no-consent fallback. |
| 6 | **Per-task budget** | Run cap exists; task-specific behavior remains unproven | Recipe runs receive a default worker budget, but the saved advisory budget for the assigned task is not yet loaded into the worker engine. P4-29/TG4-15 retain that task-level behavior under the hard P2 daily ceiling (ADR-0068). |

### 5.1 `suggest_worker` currently recommends the Supervisor

Ties plus a stable sort plus `ORDER BY is_system_defined DESC, id ASC` (`db/workers.rs:209`) make `suggest_worker` return **`supervisor`** (`F-ROU-02`). And the static keyword layer beneath it is unreachable dead code: it requires `conf > 0.85` while the maximum confidence any pattern declares is exactly `0.85` (`scorer.rs:121` vs `patterns.rs:15`).

### 5.2 The ordering constraint

**M-12 (atomic claim) lands before affinity is enabled.** Task theft is latent only because no outcomes are recorded; turning on recording without fixing the claim means the model learns from wrong attributions — worse than not learning.

---

## 6. Worker identity and persona (ADR-02-07 — decided: (C))

`workers.db` owns identity, model and state — queryable, mutable, UI-editable. The recipe file owns the prompt — versionable, diffable, shareable. `workers.db` holds a **reference** (`recipe_path`), never a copy. One owner per fact (INV-10).

**Binding condition:** `ensure_default_recipes` currently overwrites all system recipes on every launch, silently reverting user edits. P4-30 must prove an edited recipe survives initialization while required built-in defaults remain available.

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
| **G-DELIVER** | Agent output lands under the resolved workspace, never `~/.arcana/agents/` or the CWD; the handoff opens and runs it | F-DEL |
| **G-ACP-IDENTITY** | Worker→worker messaging uses truthful identity on the Rust-native path | P4-22/TG4-10 |
| **G-ACP-DELIVERY** | A message is delivered durably at least once until acknowledged; a stale consumer receives the same stable message ID and handles its effects idempotently; acknowledged messages are not redelivered | ADR-0070 replaces the old at-most-once/"lossless" contradiction |
| **G-ESCALATION-LIVENESS** | An unresolved request cannot starve dispatch indefinitely | The Supervisor STOP starvation |
| **G-AFFINITY-CLOSES** | Autonomous completion records an outcome, and after N outcomes routing measurably changes | F-ROU-01 |
| **G-SUGGEST** | `suggest_worker` never returns a system-defined agent for implementation work | F-ROU-02 |
| **G-BUDGET-FIRES** | A task exceeding its budget trips the breaker and surfaces to the user | F-BUD-01 |
| **G-CHECKPOINT-BLOCKS** | A failed checkpoint blocks the write; each write carries its task identity; restoring one task's checkpoint leaves another task's writes untouched; workers keep the shared `arcana/fleet` branch | M-43, ADR-0067 |
| **G-QC-ANALYZE** | The QC reviewer receives a repo path and `analyze_diff` actually runs — **under the workspace's recorded egress consent** (spec `06 §G-PERM-NETWORK`, corrected). With no consent recorded, QC degrades to the deterministic structural risk score and says so; it does not silently ship the diff, and it does not prompt an unattended machine | F-QC-01; and the gate collision the review caught |

**Consent boundary (owner ruled 2026-09-26, ADR-0073).** Persisted workspace
consent governs Arcana-mediated provider and diff-analysis requests, scoped to
the canonical workspace and provider destination. Free-form shell commands can
select arbitrary network destinations and are outside this app-mediated
contract; they require separate OS-level network controls.

---

> No ADR decision is recorded in this spec. It hands **ADR-02-01**, **ADR-02-02**, **ADR-02-04**, **ADR-02-06** and **ADR-02-07** to `00 §Part 8`.
