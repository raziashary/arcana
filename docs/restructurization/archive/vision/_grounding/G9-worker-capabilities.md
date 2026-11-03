# G9 — Worker Capabilities Audit (read-only grounding scout)

**Scope:** Supervisor escalation queue · worker↔worker messaging (ACP) · dynamic routing / affinity ·
epic/phase decomposition · interface contracts & QC support · per-worker budget controls.

**Framing:** These capabilities are considered GOOD by the owner and are to be **preserved** through the
restructure (Python sidecar deleted, all agent capability moved in-house into Rust). Findings are framed as
*what must be preserved / fixed / completed*, never as deletion candidates.

**Method:** read-only. Every load-bearing claim carries a `path:line`. Claims are made from function
bodies, not docstrings; where a docstring and the code disagree, that disagreement is itself logged
as a finding. Nothing was built, run, or modified.

**Status:** COMPLETE.

---

## 1. Executive summary

**Scope note.** Six capabilities, ~2,900 lines of tool code plus their DB, event, command and React
legs. All six are worth preserving — the *designs* are consistently good. What the audit found is
that four of the six are **built but inert**: the code exists, the schema exists, the UI exists, and
some single connecting call is missing. The dominant failure mode in this codebase is not bad
design; it is a **writer or trigger that lives on the wrong path**.

**Two structural root causes explain most of the 54 findings:**

* **(A) `ToolContext.agent_role` exists only at the MCP boundary.** It is set once
  (`mcp_server/tool_adapter.rs:280`) and hardcoded `None` at all three in-house executor sites
  (`agent/tool_executor.rs:89, 252, 316`). Five of six capabilities read it. Today the fleet runs
  Mode 2 through the sidecar so it works; the day the sidecar is deleted, the ACP stops entirely.
* **(B) Post-write side effects live in the Tauri UI command, not in the agent tool.** Routing
  outcome recording, dependency unblocking and phase completion all run in
  `commands/task_commands.rs:45-98` and are absent from `update_task_status_tool.rs` — the tool every
  worker and QC agent actually calls. So affinity learning and epic gating only happen when a human
  drags a card.

---

**1. Supervisor escalation queue — VERDICT: WORKS, the strongest capability in scope. Preserve as-is;
add a reaper.** Writer, store, reader, resolver, live `worker:request` event and the
`WorkerRequestsWidget` are all present and connected (`request_supervisor_action.rs:111-132` ->
`db/tasks.rs:327-341, 618-686` -> `list_worker_requests_tool.rs:49` ->
`commands/task_commands.rs:165-179` -> `stores/taskStore.ts:193-209` ->
`tabs/AutoDevPanel.tsx:569-860`), and the Supervisor recipe genuinely drains it every tick
(`recipes/defaults.rs:229-239`). The queue *is* drained — but only by prompt discipline. Nothing
expires an unresolved request, and Step 1a's "STOP after this step" (`:239`) means one
permanently-unresolvable request starves task dispatch indefinitely (F-WRQ-01). Fully survives the
sidecar deletion.

**2. Worker↔worker messaging (ACP) — VERDICT: WORKS TODAY, BREAKS ON PORT. The single highest-risk
item in the restructure.** The mediated-inbox design is sound and completely wired: schema with
correct hot-path indexes (`db/tasks.rs:345-362`), send/list/ack tools, a `worker:message` event, a
Fleet Chat panel (`components/autodev/FleetChat.tsx`, mounted `AutoDevPanel.tsx:886`), and a genuine
receive path — every worker recipe has a Step 1.5 inbox drain (`defaults.rs:395-400, 648, 860, 972,
1148`). Three defects matter: **(a)** all three tools hard-refuse without `ctx.agent_role`, which
in-house execution never sets (F-ACP-01, CRITICAL for the port); **(b)** `qc-agent` and
`qc-dispatcher` accept mail that no recipe ever reads and no `workers.db` row backs (F-ACP-02) —
every broadcast leaves 2 undeliverable rows; **(c)** listing marks messages `read` and nothing
re-surfaces them, so delivery is at-most-once while the recipe promises "lossless"
(`defaults.rs:400`, F-ACP-03). Messages are not budget-accounted and each send fires a full LLM tick
on the recipient (F-ACP-04).

**3. Dynamic routing / worker affinity — VERDICT: INERT. The machinery is complete and has never
influenced a decision.** The algorithm is real and reasonable
(`success_rate*0.6 + duration_score*0.4`, `>=3`-sample gate, `total/(total+5)` confidence —
`routing/scorer.rs:70-83`), assignments are recorded from both dispatch paths
(`assign_task.rs:114`, `assign_tasks_batch.rs:190`), and a dashboard reads the aggregate
(`WorkerAffinityPanel.tsx:35`). But the **outcome** half is written only by the UI command
(F-ROU-01), so `routing_history.outcome` is NULL forever in autonomous operation, the sample gate
never opens, and every worker returns `score 0.5 / method "default"`. With all scores tied, the
stable sort plus `ORDER BY is_system_defined DESC, id ASC` (`db/workers.rs:209`) makes
`suggest_worker` return **`supervisor`** (F-ROU-02). And the static keyword layer is unreachable
dead code: it requires `confidence > 0.85` while the maximum declared is exactly `0.85`
(`scorer.rs:121` vs `patterns.rs:15`, F-ROU-03). The task-stealing hole the sibling audit found
(`db/tasks.rs:895-902`) is real and will corrupt affinity labels the moment F-ROU-01 is fixed —
they must be fixed together. Fully survives the sidecar deletion.

**4. Epic / phase decomposition — VERDICT: INERT AND UNREACHABLE FROM THE FLEET.** The model is
good — epic-as-a-task (`task_type='epic'`), five phase columns, `blocked_by` as the real dependency
edge, an all-blockers-done re-check (`db/tasks.rs:1358-1363`), epic auto-completion, live
`task:unblocked` / `task:phase-complete` events and an `EpicPhaseView` panel. Three critical breaks:
**(a)** `create_epic` / `create_phased_task` are registered for PA modes only and deliberately
excluded from the MCP registry (`mod.rs:222-223` vs `:96-164`; `mcp_server/server.rs:140-141`) —
but the Supervisor is `Mode2Only` and reaches tools *only* through MCP, so its own recipe decomposes
with flat `create_task` (`defaults.rs:288-294`) and no fleet agent can ever create an epic
(F-EPI-01); **(b)** epics are inserted `status='ready'` and no dispatch path filters
`task_type='epic'`, so an epic can be assigned and claimed as ordinary work (F-EPI-02 — the sibling
audit's suspicion, CONFIRMED); **(c)** `unblock_ready_tasks` runs only on the UI path, so under
autonomous operation dependents are never unblocked and epics never complete (F-EPI-03). Phase
gating via `is_unlocked` is advisory only. Survives the sidecar; the registry split that blocks it
is itself a sidecar artefact.

**5. Interface contracts and QC support — VERDICT: MIXED. `spawn_qc_review` is the best-engineered
code in scope and the only piece that does *not* survive the port.** The INTERFACE.md handshake is
**advisory**: the producer requirement is recipe prose (`defaults.rs:415-431`) with no code gate on
the `qc_review` transition, and the consumer check `tier2_6_api_contract` fires at write time but
only appends a lint to a **success** result (`write_file.rs:184-197`) and self-disables when no
spec exists (`validate.rs:660-661`). Nothing verifies the frontend read it — there is no read
receipt anywhere; the closest proxy is `analyze_diff`'s `interface_drift` classification. But
`analyze_diff` itself never runs: both QC recipes gate it on a `repo_path` "in task metadata"
(`defaults.rs:1298, 1311`) and **`tasks` has no such column**, while the dispatcher calls
`spawn_qc_review(task_id=…)` with no `repo_path` (F-QC-01). `e2e_probe` proves exactly one thing —
that after an 8-second sleep, a GET to `probe_url` returns 2xx/3xx and the body contains a
fingerprint — and it always tears down, but only kills the *shell* it spawned, so on Windows
`npm run dev` survives as an orphaned `node.exe` holding the port (F-QC-03); nothing schedules the
`fleet-smoke` recipe that was meant to drive it (F-QC-04). `spawn_qc_review` does everything right
(atomic claim, cost attribution, cost guard, release on every path) but spawns through
`SubagentManager`, which is built on `OrchestratorClient` — **it does not survive the sidecar
deletion**.

**6. Budget and cost controls — VERDICT: FULLY INERT. The sibling audit is confirmed, and the blast
radius is larger than reported.** `with_task_budget` (`agent/engine.rs:73-77`) has zero callers, so
`task_budget` **and** `task_id` are permanently `None` (`:62`). That kills four things, not one: the
circuit breaker never fires (`:148-166`); per-task usage is never persisted, so
`budget_tokens_used`/`budget_cost_usd`/`budget_turns_used` are 0 for every task ever
(`:216-230`); `escalate_budget_exceeded` — the best idea in the subsystem, halt-and-hand-back to the
Supervisor — is dead code (`:480-536`); and `agent:budget-exceeded` is never emitted, leaving two
frontend subscribers listening to silence. Two more budget paths are also inert: the Settings
default worker budget has **no Rust reader at all** (F-BUD-02), and **no recipe ever calls
`set_task_budget`** (grep of `recipes/defaults.rs` → zero hits). `set_task_budget` also reports
defaults it never persists (F-BUD-03). Even a fix would not reach the fleet today, because
enforcement lives in `AgentEngine` — the Mode 1 last-resort path the Mode2Only fleet never runs.
**Deleting the sidecar is what finally makes this capability enforceable.**

---

### The five changes that unlock the most

1. Thread `agent_role` from the recipe into `ToolContext` at all three executor sites — unblocks
   ACP, annotation provenance and escalation identity (root cause A).
2. Collapse `commands/task_commands.rs:45-98` and `update_task_status_tool.rs` onto one shared
   `transition_task_status()` — turns routing affinity and epic gating from inert to live
   (root cause B).
3. Call `with_task_budget` from the tick dispatcher, resolving task -> runtime default -> preset —
   turns the entire budget subsystem on.
4. Give the Supervisor the epic tools and add `task_type != 'epic'` to dispatch.
5. Replace `SubagentManager` in-house while preserving `spawn_qc_review`'s four invariants.

---

## 2. Capability 1 — Supervisor escalation queue

### 2.1 How it works today (the real mechanism)

**Write side.** `RequestSupervisorActionTool` (`src-tauri/src/agent/builtin_tools/request_supervisor_action.rs:65-143`)
validates `request_type` against a 5-value allow-list (`:71-77`) — `decompose_audit`, `reassign`,
`needs_info`, `report_issue`, `request_review` — requires `payload` to be a JSON **object**
(`:81-84`, serialised with `.to_string()`, so the DB stores raw JSON text), resolves the sender
identity, inserts a row via `TasksDb::create_worker_request` (`db/tasks.rs:618-631`, always
`status='pending'`), then does two side effects:

* `crate::recipes::scheduler::kick("supervisor")` (`request_supervisor_action.rs:122`) — wakes the
  Supervisor immediately instead of waiting for the `*/1 * * * *` cron (`recipes/defaults.rs:215`).
  `kick` degrades gracefully to "wait for next cron tick" when the global scheduler is not yet
  installed (`recipes/scheduler.rs:514-522`).
* `emit_worker_request_global(...)` (`request_supervisor_action.rs:125-132` -> `events.rs:328-332`)
  — a Tauri `worker:request` event for the UI.

**Sender identity** is resolved in a 3-step ladder (`request_supervisor_action.rs:98-109`):

1. `tasks.assigned_worker` of the task named by `task_id` (authoritative — `assign_task` wrote it);
2. `ctx.agent_role` (the MCP `__agent_role` meta-arg);
3. the literal string `"unknown"`, with a `tracing::warn!`.

**A second, non-agent writer exists:** the budget circuit breaker escalates by inserting a
`report_issue` request with `worker_slug = "engine"` (`agent/engine.rs:510-521`), plus
`mark_task_budget_exceeded` and a `task:changed` event (`:522-530`). See §7 — that path is
currently dead because the per-task budget is never populated.

**Read side (Supervisor).** `ListWorkerRequestsTool` (`list_worker_requests_tool.rs:41-81`) is a
plain `SELECT ... ORDER BY created_at DESC` with an optional `status` filter
(`db/tasks.rs:643-666`). It returns a `{count, filter, requests[]}` JSON blob and, when the pending
list is empty, attaches a hint steering the Supervisor to Step 2 (`:73-77`).

**Resolve side.** `ResolveWorkerRequestTool` (`resolve_worker_request_tool.rs:52-81`) validates the
action against `completed|acknowledged|rejected` and calls
`TasksDb::update_worker_request_status` (`db/tasks.rs:668-686`), which stamps `resolved_at =
datetime('now')` only for `completed`/`rejected` and **explicitly NULLs `resolved_at` for
`acknowledged`** (`:674-678`).

**Consumer recipe.** The Supervisor recipe's *Step 1a* (`recipes/defaults.rs:229-239`) is the drain
loop: list pending -> branch per `request_type` -> `resolve_worker_request(...)`, with the explicit
warning that failing to resolve produces an infinite re-process loop (`:230-232`). Note `:239` —
"If any request was handled, STOP after this step" — the Supervisor drains **and then ends its
tick**, so dispatch (Step 2) is deferred to the next tick.

### 2.2 Wired end to end?

**Yes — this is the most completely wired capability in the audit scope.**

| Leg | Evidence |
|---|---|
| Writer (agent) | `request_supervisor_action.rs:111-119` |
| Writer (engine/budget) | `agent/engine.rs:510-521` (currently unreachable — see F-BUD-01) |
| Store | `worker_requests` table, `db/tasks.rs:327-341` (3 indexes: status, worker_slug, task_id) |
| Reader (agent) | `list_worker_requests_tool.rs:49` -> `db/tasks.rs:643` |
| Reader (Tauri cmd) | `commands/task_commands.rs:165-169`, registered `lib.rs:215-216` |
| Resolver | `resolve_worker_request_tool.rs:71` and `commands/task_commands.rs:172-179` |
| Push channel | `events.rs:324-332` (`worker:request`) -> `stores/taskStore.ts:250-253` |
| UI | `tabs/AutoDevPanel.tsx:569+` `WorkerRequestsWidget`, mounted `:855-858`, badge `:851-853`, wired `:1210-1211`, initial load `:1041` |
| Recipe consumer | `recipes/defaults.rs:229-239` (Step 1a), tool list `:351-355` |

The queue **is** drained — but only by prompt-level discipline. There is no code path that forces
resolution; the enforcement is the recipe sentence at `defaults.rs:230-232`.

### 2.3 Survives sidecar deletion?

**YES — fully.** Every leg is Rust + SQLite + Tauri. The only sidecar coupling is *indirect*: the
worker identity ladder step 2 reads `ctx.agent_role`, which today is populated only at the MCP
boundary (`mcp_server/tool_adapter.rs:191-192, 280`). In-house Rust execution sets
`agent_role: None` at all three `ToolContext` construction sites (`agent/tool_executor.rs:89`,
`:252`, `:316`). For *this* capability the damage is bounded because step 1 (task.assigned_worker)
usually covers it — the fallback only matters for requests sent without a `task_id`.

**Port work:** none for the queue itself; the shared fix is to thread `agent_role` into
`ToolContext` in `tool_executor.rs` (see F-ACP-01, which is fatal for capability 2).

### 2.4 Defects

* **F-WRQ-01 (HIGH, leak/liveness).** *Nothing expires, times out, or garbage-collects a worker
  request.* The only deletion is `TasksDb::clear_all` (`db/tasks.rs:603-612`, the UI "Clear Board"
  button). A `pending` request whose Supervisor branch throws — or a `request_type` the Supervisor
  mis-handles — stays pending forever and is re-read on every 1-minute tick
  (`defaults.rs:215`). Combined with `defaults.rs:239` ("If any request was handled, STOP after
  this step") a single permanently-unresolvable request **starves task dispatch**: ticks spend
  themselves on Step 1a and never reach Step 2. **CONFIRMED** (by absence: no
  `DELETE FROM worker_requests` outside `clear_all`, no `resolved_at`-based reaper, no age filter
  in `list_worker_requests`). *Failure scenario:* a worker files `decompose_audit` with a malformed
  `suggested_tasks`; the Supervisor's `create_task` calls all fail; the LLM gives up without
  calling `resolve_worker_request`; every subsequent tick re-reads the same request. The board
  freezes with tasks in `ready` and nothing surfaces except the widget badge.

* **F-WRQ-02 (MEDIUM, contract).** `list_worker_requests` has **no `LIMIT`** and no age filter
  (`db/tasks.rs:643-658`). With `status=None` the whole historical queue is returned, `payload`
  blobs included, straight into an LLM context window. **CONFIRMED.** *Failure scenario:* after a
  few hundred requests, a Supervisor tick that omits the status filter spends a large fraction of
  its context on resolved history.

* **F-WRQ-03 (LOW-MEDIUM, semantics).** `acknowledged` is a *terminal-looking but non-terminal*
  status: `update_worker_request_status` sets `resolved_at = NULL` for it (`db/tasks.rs:674-678`),
  yet the recipe uses `acknowledged` as the terminal action for **two of five** request types —
  `needs_info` (`defaults.rs:235`) and `report_issue` (`:236`). Those requests leave the pending
  queue but carry no resolution timestamp, so no age-based tooling can be written against them.
  **CONFIRMED.**

* **F-WRQ-04 (LOW, UI gap).** The `WorkerRequestsWidget` `onResolve` signature only offers
  `'completed' | 'rejected'` (`tabs/AutoDevPanel.tsx:574`) — a human cannot record
  `acknowledged` from the UI, though backend and recipe both use it. The widget also returns
  `null` when the list is empty (`:587`), so there is no "queue healthy" affordance and no view of
  historical/resolved requests. **CONFIRMED.**

* **F-WRQ-05 (LOW, identity).** When a request carries no `task_id` **and** `ctx.agent_role` is
  `None` (the in-house executor case, `tool_executor.rs:89`), `worker_slug` is silently recorded as
  `"unknown"` (`request_supervisor_action.rs:101-109`). The Supervisor's `reassign` and
  `needs_info` branches then have no addressee. **CONFIRMED** for the in-house path.

* **F-WRQ-06 (LOW, integrity).** `payload` is opaque TEXT with `DEFAULT '{}'` (`db/tasks.rs:332`)
  and is never schema-validated per `request_type`, despite the tool description promising per-type
  shapes (`request_supervisor_action.rs:51-56`). The Supervisor is told to "validate
  `suggested_tasks`" in prose only (`defaults.rs:233`). **CONFIRMED.**

### 2.5 What the restructure must preserve

1. **The five escalation verbs** and their exact semantics — the worker's only sanctioned way to
   reach the Supervisor without task-creation privilege (`mod.rs:111-112, 140-148`).
2. **Kick-on-write** (`request_supervisor_action.rs:122`) — escalation is latency-sensitive;
   falling back to the 1-minute cron is a visible regression.
3. **The identity ladder** (task-first, role-second) — it is what makes escalation work when the
   caller's role metadata is missing.
4. **The live UI channel** (`worker:request` -> widget badge) — the only place a human sees the
   fleet asking for help.
5. **The resolve-or-loop contract**, promoted from prose to code: add a max-age/attempt reaper and
   stop Step 1a from being able to starve Step 2 (fixes F-WRQ-01).

---

## 3. Capability 2 — Worker<->worker messaging ("ACP")

### 3.1 Message schema and lifecycle

**Schema** (`db/tasks.rs:345-362`, struct at `:69-80`):

```
worker_messages(
  id INTEGER PK AUTOINCREMENT,
  from_slug TEXT NOT NULL,          -- sender agent slug (from ctx.agent_role)
  to_slug   TEXT NOT NULL,          -- recipient slug (broadcast fans out to N rows)
  body      TEXT NOT NULL,          -- <= 4096 bytes, enforced in the tool
  kind      TEXT NOT NULL DEFAULT 'nudge',  -- nudge|question|answer|fyi|broadcast
  task_id   INTEGER,                -- FK -> tasks(id), optional thread key
  status    TEXT NOT NULL DEFAULT 'pending', -- pending|read|acknowledged
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  read_at   TEXT )
```

Indexes: `(to_slug, status)`, `(task_id)`, `(created_at)` (`db/tasks.rs:357-362`) — these cover the
inbox query (`WHERE to_slug=? AND status='pending'`), the per-task thread query, and the recent
feed. **Hot-path indexing is correct.** No index on `from_slug` (no query needs one).

**Lifecycle:** `pending` --(`list_worker_messages` returns it)--> `read` --(`acknowledge_worker_message`)-->
`acknowledged`. The `pending -> read` transition is a **side effect of listing**
(`db/tasks.rs:754-775`): the rows are snapshotted first, then a single `UPDATE ... WHERE id IN (...)
AND status='pending'` flips them. `acknowledge_worker_message` accepts only `pending`/`read`
(`db/tasks.rs:781-793`) and errors on a double-ack.

**Send path** (`send_worker_message.rs:87-227`): validate `to` against
`is_known_recipient` (`:31-37` — the 5 `FLEET_WORKERS` plus `supervisor`, `qc-agent`,
`qc-dispatcher`, `*`); validate `body` non-empty and `<= 4096` bytes (`:100-111`); validate `kind`
against `VALID_KINDS` (`:113-122`); resolve `from` from `ctx.agent_role` and **hard-refuse when it
is absent** (`:129-137`); refuse self-addressed (`:140-145`); enforce broadcast-is-supervisor-only
(`:150-155`); then, per target, `insert_worker_message` + `emit_worker_message_global` +
`scheduler::kick(target)` (`:181-207`).

**Receive path** — the docstring claim that "the target reads its inbox during its own scheduled
tick" is **VERIFIED**: every worker recipe has a Step 1.5 "Drain your ACP inbox" that calls
`list_worker_messages` (`recipes/defaults.rs:395-400` backend, `:648` frontend, `:860` platform,
`:972` mcp, `:1148` cleanup) and the Supervisor has Step 1b (`:241-248`). Delivery latency is
sub-second under the `kick`, else the recipe's cron (`*/1` for backend/frontend/mcp/supervisor,
`*/5` platform, `*/2` cleanup — `defaults.rs:215, 377, 629, 844, 953, 1125`).

### 3.2 Wired end to end?

| Leg | Evidence |
|---|---|
| Writer | `send_worker_message.rs:182` -> `db/tasks.rs:693-711` |
| Inbox reader (agent) | `list_worker_messages.rs:73` -> `db/tasks.rs:728-777` |
| Ack | `acknowledge_worker_message.rs:77` -> `db/tasks.rs:781-793` |
| Push channel | `events.rs:385-400` (`worker:message`) |
| UI store | `stores/workerChatStore.ts:84` (hydrate, limit 200), `:105-126` (live listen) |
| UI panel | `components/autodev/FleetChat.tsx:41-250`, mounted at `tabs/AutoDevPanel.tsx:886` (obs tab `fleetchat`) |
| Tauri commands | `commands/worker_message_commands.rs:14, 23, 32`, registered `lib.rs:467-469` |
| Recipe consumers | `defaults.rs:241-248, 395-400, 648-649, 860, 972, 1148` |

**Yes — with two dead branches.** (a) `TasksDb::list_stale_worker_messages` (`db/tasks.rs:799-812`)
— the documented "Supervisor drain for stuck conversations" — has **zero callers** anywhere in the
tree; the Supervisor's Step 1b prose (`defaults.rs:248`) instructs the LLM to inspect stale threads
but exposes no tool that can see another agent's inbox. (b) Two accepted recipients have **no
reader** (see F-ACP-02).

### 3.3 The `agent_role` question (sibling audit cross-check)

**CONFIRMED, and it is the single most important fact about this capability.** `ToolContext.agent_role`
is hardcoded `None` at all three in-house executor construction sites: `agent/tool_executor.rs:89`
(serial `execute`), `:252` (concurrent read-tool partition), `:316` (serial write partition). It is
populated **only** at the MCP boundary — `mcp_server/tool_adapter.rs:191-192` reads `__agent_role`
out of the call arguments and `:280` puts it into the `ToolContext`.

Consequence today: the fleet runs **Mode 2** (core-orchestrator LivingAgent — the Python sidecar),
and every fleet recipe except `worker-cleanup` is `FallbackPolicy::Mode2Only`
(`defaults.rs:220, 382, 634, 849, 958, 1286, 1804, 1881`; cleanup is `Mode1Capable` at `:1135`).
Mode 2 agents reach tools through the MCP server, so `agent_role` is set and the ACP works.

Consequence after the sidecar deletion: if in-house execution routes through
`agent/tool_executor.rs`, **all three ACP tools hard-refuse**:
`send_worker_message.rs:129-137`, `list_worker_messages.rs:60-66`,
`acknowledge_worker_message.rs:48-55` each return an error when `agent_role` is `None`. The
capability does not degrade — it stops. The self-messaging guard (`:140-145`) and the
broadcast-is-supervisor-only guard (`:150-155`) are both derived from `from`, so they become
unreachable rather than unsafe.

Note the same variable also gates `update_task_status(status='qc_review')`
(`update_task_status_tool.rs:131-137`) and `annotate_file`'s `source` attribution
(`annotate_file_tool.rs:77`), so the fix is shared across capabilities.

### 3.4 Delivery guarantees — what the code actually promises

* **At-least-once persistence, at-most-once *effective* delivery.** The row is durable, but
  `list_worker_messages` flips `pending -> read` as a *side effect of listing*
  (`db/tasks.rs:754-775`) and defaults to `unread_only=true` (`list_worker_messages.rs:58`). If the
  agent's turn dies after the list call and before acting, the message is `read`, never
  `acknowledged`, and never appears in a default inbox read again. Nothing re-surfaces it: the one
  query that could (`list_stale_worker_messages`) has no caller. The worker recipe's claim "Inbox
  is async and **lossless**" (`defaults.rs:400`) is therefore false.
* **No retry, no dead-letter, no TTL, no expiry.** `worker_messages` rows are never deleted — not
  even by `clear_all`, which drains only `worker_requests` and `tasks` (`db/tasks.rs:603-612`).
  Because `worker_messages(task_id)` is a declared FK to `tasks(id)` and `PRAGMA foreign_keys=ON`,
  see F-ACP-06.
* **No ordering guarantee across senders** beyond `created_at ASC` at 1-second resolution
  (`datetime('now')` has no sub-second precision), so same-second messages order by insertion
  arbitrarily.

### 3.5 Budget accounting

**None.** No ACP tool touches `crate::budget`, `update_task_budget_usage`, or any cost path — a
`grep` of the three tool files shows zero budget references. Each `send_worker_message` does,
however, trigger `scheduler::kick(target)` (`send_worker_message.rs:204`), and `kick_worker` spawns
a **full recipe run** — i.e. a complete LLM tick — per kick (`recipes/scheduler.rs:442-449`). So a
message is a free write that causes an unbudgeted, unmetered inference run on another agent.

### 3.6 Loop / storm risk

Three code-level brakes exist, and they are partial:

1. `kick_inflight` — a process-wide `HashSet<String>`; a second kick for a slug already in flight is
   dropped (`scheduler.rs:362-373, 500-503`).
2. The DB gate — `kick_worker` returns early if the worker row's `status == "working"`
   (`scheduler.rs:397-400`).
3. Self-send refusal (`send_worker_message.rs:140-145`).

What is **not** bounded: (a) the number of messages one agent may send per tick — no per-tick or
per-minute cap anywhere; (b) A->B->A ping-pong, which is prevented only by recipe prose telling the
receiver to `acknowledge` rather than reply (`defaults.rs:397-399`); (c) broadcast amplification —
one supervisor broadcast writes 7 rows and issues 7 kicks (`send_worker_message.rs:168-178`),
each a full LLM tick. A confused Supervisor that broadcasts every tick pins the whole fleet at
100% inference duty cycle. **SUSPECTED-to-CONFIRMED**: the mechanism is confirmed by code; the
observed occurrence is not.

### 3.7 Survives sidecar deletion?

**PARTIALLY — the storage and UI survive; the identity does not.** Table, tools, events, commands
and panel are all Rust/TS. But `agent_role` today comes only from the MCP boundary
(`tool_adapter.rs:191, 280`); the in-house executor sets `None` (`tool_executor.rs:89, 252, 316`).

**Port work (mandatory, blocking):**
1. Thread an agent identity into `ToolExecutor` (constructor field, propagated to all three
   `ToolContext` literals) and populate it from the recipe's `agent_role`
   (`recipes/types.rs:52`, values at `defaults.rs:217, 379, 631, 846, 955, 1127, 1283, 1798, 1878`).
2. Keep the hard-refuse-on-missing-identity behaviour — it is the guard that keeps the inbox
   fleet-only.
3. Re-point `scheduler::kick` at whatever replaces the LivingAgent tick.

### 3.8 Defects

* **F-ACP-01 (CRITICAL for the restructure, identity).** `ToolContext.agent_role` is `None` at
  `agent/tool_executor.rs:89`, `:252`, `:316`; only `mcp_server/tool_adapter.rs:280` sets it. The
  entire ACP (send/list/ack) hard-refuses without it. **CONFIRMED.** *Failure scenario:* the day
  the sidecar is removed and workers run on the Rust engine, every ACP call returns
  "refused: no `__agent_role` on the call", cross-worker coordination silently stops, and the
  recipes keep instructing agents to use it — burning a tool call per tick per worker on a
  guaranteed error.

* **F-ACP-02 (HIGH, dead-letter).** `qc-dispatcher` and `qc-agent` are accepted recipients
  (`send_worker_message.rs:33-34`) and both receive every broadcast (`:172-173`), but **no agent
  ever reads those inboxes**: `qc_agent_recipe` (`defaults.rs:1275+`) and `qc_dispatcher_recipe`
  (`:1782+`) contain no `list_worker_messages` step (the only occurrences are at `:243, 396, 648,
  860, 972, 1148` — supervisor + the five workers). Worse, no agent's `agent_role` is ever
  `"qc-dispatcher"` — the dispatcher recipe declares `agent_role: Some("qc-agent")`
  (`defaults.rs:1798`) — and `workers.db` has no `qc-dispatcher` row (`db/workers.rs:175-190`), so
  `scheduler::kick("qc-dispatcher")` logs "worker not found" and returns
  (`scheduler.rs:404-407`). **CONFIRMED.** *Failure scenario:* every Supervisor broadcast leaves 2
  permanently-`pending` rows; the FleetChat unread badge for `qc-agent`/`qc-dispatcher`
  (`workerChatStore.ts:94-98`) climbs forever and no human action can clear it.

* **F-ACP-03 (HIGH, delivery).** Listing marks messages `read`; nothing ever re-surfaces a `read`
  but un-acked message (`db/tasks.rs:754-775`, `list_worker_messages.rs:58`;
  `list_stale_worker_messages` at `db/tasks.rs:799` has zero callers). Messages are lost on any
  mid-tick failure while the recipe promises losslessness (`defaults.rs:400`). **CONFIRMED.**
  *Failure scenario:* worker-frontend lists 3 messages, then its LivingAgent tick errors out
  (`runner.rs:500-536`); the messages are `read`, the next tick's default inbox read returns empty,
  and the questions are never answered — the asking worker waits forever with no timeout.

* **F-ACP-04 (MEDIUM, cost).** A message send triggers `scheduler::kick(target)`
  (`send_worker_message.rs:204`), which spawns a full recipe/LLM tick (`scheduler.rs:442-449`),
  with no per-sender rate limit and no budget accounting. Broadcast multiplies this by 7.
  **CONFIRMED** (mechanism).

* **F-ACP-05 (MEDIUM, UI correctness — timestamp class).** `worker_messages.created_at` is written
  by SQLite `datetime('now')` (`db/tasks.rs:353`), i.e. `"YYYY-MM-DD HH:MM:SS"` **UTC with no zone
  marker**. `FleetChat.tsx:26-34` parses it with `new Date(iso)` and renders `timeAgo(m.created_at)`
  (`:235`); V8 parses that non-ISO form as **local time**, so every message is displayed offset by
  the viewer's UTC offset. The same string flows through the live event as
  `WorkerMessagePayload.timestamp` (`send_worker_message.rs:197` -> `workerChatStore.ts:115`).
  This is the frontend twin of the RFC3339-vs-`datetime('now')` defect the sibling audits found in
  Rust. **CONFIRMED.** *Failure scenario:* at UTC+05:30 every freshly-sent message renders as
  "5h ago", making the Fleet Chat feed useless for judging staleness — which is exactly what the
  Supervisor's Step 1b stale-thread heuristic (`defaults.rs:248`) depends on.

* **F-ACP-06 (MEDIUM, data integrity).** `worker_messages.task_id` is a declared FK to `tasks(id)`
  with `PRAGMA foreign_keys=ON`, but `clear_all` deletes only `worker_requests` then `tasks`
  (`db/tasks.rs:603-612`) — it never drains `worker_messages`. The comment at `:596-602` explains
  the FK problem for `worker_requests` and does not account for the newer `worker_messages` FK.
  **CONFIRMED by inspection / SUSPECTED at runtime** (depends on whether SQLite raises the FK
  violation; with `foreign_keys=ON` and a non-null `task_id`, `DELETE FROM tasks` should fail).
  *Failure scenario:* the user clicks "Clear Board" after any run with task-threaded chat and the
  whole clear transaction aborts, leaving the board un-clearable.

* **F-ACP-07 (LOW, contract).** `MAX_BODY_BYTES` is compared against `body.len()`
  (`send_worker_message.rs:105`) — a byte length on a `String`, which is correct for the stated
  "4096 bytes"; but the schema description says "Plain text, max 4096 bytes" while the error text
  says the same — no defect in the check itself. The real gap: **no total-inbox cap**.
  `list_worker_messages` with `unread_only=false` returns the agent's entire history unbounded
  (`db/tasks.rs:743-748`) straight into a context window. **CONFIRMED.**

* **F-ACP-08 (LOW, ergonomics).** `list_worker_messages` declares `is_read_only() -> false`
  deliberately (`list_worker_messages.rs:46-55`), which is correct for the write partition, but it
  means the tool is subject to permission prompts under `ManualApproval`/`SmartApprove`
  (`tool_executor.rs:101-116`) — an inbox read can block on a human. **CONFIRMED**; low impact
  because fleet recipes run `Autonomous`/`Supervised`.

### 3.9 What the restructure must preserve

1. **The mediated-inbox model itself** — persistent, auditable, Supervisor-visible, async. This is
   the design's core value versus direct agent-to-agent calls (`send_worker_message.rs:8-14`).
2. **The five kinds and the recipient allow-list** — including broadcast-is-supervisor-only
   (`:150-155`), which is the only anti-flood control that exists.
3. **Identity-derived sender + refuse-on-missing-identity** (`:129-137`) — but the identity must
   now come from the recipe, not the MCP envelope (F-ACP-01).
4. **`task_id` threading** — it is what makes FleetChat's per-task view and the Supervisor's
   conversation audit possible.
5. **Kick-on-send latency** (`:204`), with a rate limit added (F-ACP-04).
6. **The three-state lifecycle**, but with `read` made recoverable — either re-surface un-acked
   messages after N minutes (wire up `list_stale_worker_messages`) or move the `pending -> read`
   transition to an explicit call (fixes F-ACP-03).

---

## 4. Capability 3 — Dynamic routing / worker affinity

### 4.1 The actual algorithm and its formula

`TaskRouter::suggest_worker` (`routing/mod.rs:35-49`) does four things: static keyword match, load
the last 200 routing rows, load all workers, score, select.

**Stage 1 — static patterns** (`routing/patterns.rs:45-66`). Lowercases `title + " " + description`
and counts substring hits per pattern; a pattern fires at `hits >= 2`; the pattern with the most
hits wins. Five patterns with fixed confidences: backend 0.85, frontend 0.85, platform 0.80,
mcp 0.80, cleanup 0.75 (`patterns.rs:10-41`).

**Stage 2 — historical scoring** (`routing/scorer.rs:19-103`). Rows are filtered to
`entry.task_type == task.task_type` (`:37`). Per worker it accumulates `successes`, `failures`, and
mean `duration_ms` over rows where `duration_ms > 0`. A worker qualifies for a computed score only
at `successes + failures >= 3` (`:70`); otherwise it gets `score 0.5, confidence 0.0, method
"default"` (`:91-96`). For qualifying workers:

```
success_rate   = successes / (successes + failures)
duration_score = 1.0 - (avg_dur - min_dur) / max(max_dur - min_dur, 1.0)      # fastest -> 1.0
score          = success_rate * 0.6 + duration_score * 0.4                     # scorer.rs:82
confidence     = total / (total + 5.0)                                          # scorer.rs:83
```

Scores are sorted descending with a **stable** sort (`:101`).

**Stage 3 — selection** (`scorer.rs:107-141`). If a static match exists **and**
`static_conf > 0.85` **and** that worker's historical score `>= 0.3`, return the static worker with
`score = (static_conf + hist_score)/2`. Otherwise `scores.remove(0)` — the top-ranked worker.

### 4.2 What history is recorded, and who reads it

**Writers.** `TaskRouter::record_assignment` (`routing/mod.rs:52-67`) inserts a row with a fresh
UUID, `assigned_at = chrono::Utc::now().to_rfc3339()`, and `outcome = NULL`. It is called from
exactly two places: `assign_task.rs:114` and `assign_tasks_batch.rs:190`.

`TaskRouter::record_outcome_auto` (`routing/mod.rs:86-99`) back-fills `outcome`, `outcome_at`, and a
computed `duration_ms`. It is called from exactly one place: `commands/task_commands.rs:53, 56, 60`.

**Readers.** `scorer::score_workers` via `suggest_worker` (`routing/mod.rs:42`);
`TaskRouter::get_affinity_stats` (`:102-125`) -> `db::get_affinity_stats` (`db/tasks.rs:1171-1193`)
-> Tauri `get_worker_affinity_stats` (`commands/routing_commands.rs:23`) ->
`components/dashboard/WorkerAffinityPanel.tsx:35`, mounted as the `affinity` observability tab
(`tabs/AutoDevPanel.tsx:863-865`).

`TasksDb::get_worker_success_rates` (`db/tasks.rs:1022-1045`) has **zero callers** — dead code.

### 4.3 Does affinity learning actually influence a later decision? (reader-exists proof)

**No — not in autonomous operation. CONFIRMED.**

The reader exists and is correct, but the *outcome* half of the writer is only on the human-UI path.
`TaskRouter::record_outcome_auto` is invoked solely by the Tauri command
`update_task_status` (`commands/task_commands.rs:27, 51-63`), which is what the kanban board calls.
The agent-facing tool `UpdateTaskStatusTool` — the one every worker and QC recipe actually uses to
move a task to `done`/`failed` — writes the row (`update_task_status_tool.rs:176-180`), kicks
(`:187-206`), emits (`:209-214`), fires hooks (`:233-241`) and **never touches routing**
(`grep record_outcome src-tauri/src` returns only `routing/mod.rs` and `commands/task_commands.rs`).

Therefore, in a fleet run with no human dragging cards: every `routing_history` row keeps
`outcome = NULL`; `score_workers` counts only `Some("success")`/`Some("failure")` (`scorer.rs:41-45`),
so every worker stays at `successes + failures = 0`, never reaches the `>= 3` gate (`:70`), and every
worker is returned as `score 0.5, confidence 0.0, method "default"`. Affinity learning is inert.

**And the fallback is actively wrong.** With all scores equal at 0.5, Rust's stable
`sort_by` (`scorer.rs:101`) preserves the input order, which is `WorkersDb::list()`'s
`ORDER BY is_system_defined DESC, id ASC` (`db/workers.rs:209`). The only `is_system_defined = true`
seeds are `supervisor` (id 1) and `qc-agent` (id 2) (`db/workers.rs:177, 184`). So
`scores.remove(0)` returns **`"supervisor"`**, and `suggest_worker` recommends assigning
implementation work to the Supervisor. `assign_task` would then reject it (`VALID_WORKERS` =
`FLEET_WORKERS`, `assign_task.rs:28, 71-76`), so the Supervisor burns a tool call, gets a
suggestion it cannot use, and falls back to the Routing Matrix prose (`defaults.rs:277-286`).

**The static-pattern layer is dead code.** `select_worker` requires `static_conf > 0.85`
(`scorer.rs:121`) but the maximum confidence any pattern declares is exactly `0.85`
(`patterns.rs:15, 21`). `0.85 > 0.85` is false, so the static branch can never be taken and
`method: "static"` — advertised in the tool description (`suggest_worker_tool.rs:21`) and rendered
by the UI — is unreachable. **CONFIRMED.**

### 4.4 Interaction with `assign_task`

`assign_task` is independent of the router: it validates the slug against `FLEET_WORKERS`
(`assign_task.rs:28, 71-76`), checks the task is in `ready|assigned|failed|rejected`
(`:96-105`), writes `update_status(id, "assigned", worker)` (`:107`), then fire-and-forgets
`record_assignment` (`:114`), kicks the worker (`:120`), emits (`:123-128`) and fires `OnTaskClaim`
(`:133-161`). `assign_tasks_batch` mirrors this per entry (`:160-210`) with one kick per unique
worker (`:213+`). Nothing forces the Supervisor to consult `suggest_worker` — the recipe explicitly
tells it to prefer the hard-coded Routing Matrix and to call `suggest_worker` only for "ambiguous"
tasks (`defaults.rs:254-259`). So in practice routing is a **static prose table in the prompt**,
and the whole `routing/` module is a rarely-consulted tiebreaker.

### 4.5 Task-stealing damage to affinity (sibling-audit cross-check)

**CONFIRMED.** `claim_task_with_pid` (`db/tasks.rs:873-913`) gates on
`WHERE id = ?4 AND status = 'assigned'` (`:900`) and unconditionally writes
`assigned_worker = ?1` (`:897`). There is no `AND assigned_worker = ?` predicate, so any worker
that knows the id can claim a task assigned to someone else and overwrite the owner. (The only
guard is the per-worker single-in-progress pre-flight at `:877-890`, which constrains the thief's
own concurrency, not ownership.)

Damage to affinity learning, specifically:

1. The `routing_history` row was written at assign time with the **originally assigned** worker
   (`assign_task.rs:114`, before any claim), so `worker_slug` records worker A.
2. `record_outcome_auto` matches on `task_id` and `outcome IS NULL` only
   (`routing/mod.rs:92-93`, `db/tasks.rs:990`) — it never checks who actually did the work.
3. Result: worker A is credited with worker B's success, or blamed for B's failure. The
   `duration_ms` is also wrong — it measures A's assignment timestamp to B's completion.
4. `get_affinity_stats` (`db/tasks.rs:1171-1180`) groups by that same mis-attributed `worker_slug`,
   so the dashboard panel shows the wrong worker's competence.

Because outcomes are not recorded at all in autonomous operation (§4.3), this defect is currently
*latent* — it becomes live the moment F-ROU-01 is fixed. Fix both together or affinity learning
will train on corrupted labels.

### 4.6 Wired end to end?

| Leg | Evidence | Verdict |
|---|---|---|
| Assignment writer | `assign_task.rs:114`, `assign_tasks_batch.rs:190` | OK |
| Outcome writer | `commands/task_commands.rs:53-60` only | **GAP — UI path only** |
| Scorer reader | `routing/scorer.rs:19-141` via `routing/mod.rs:42` | OK but starved |
| Agent-facing reader | `suggest_worker_tool.rs:57` | OK |
| Tauri readers | `commands/routing_commands.rs:11, 23` | OK |
| UI | `WorkerAffinityPanel.tsx:35`, tab at `AutoDevPanel.tsx:863-865` | OK but renders zeros |
| Dead reader | `db/tasks.rs:1022` `get_worker_success_rates` | **zero callers** |

### 4.7 Survives sidecar deletion?

**YES — fully.** `routing/` is pure Rust arithmetic over `tasks.db`; no sidecar, no HTTP, no
orchestrator call anywhere in `routing/mod.rs`, `scorer.rs`, `patterns.rs`. `suggest_worker_tool`
reaches it through the normal tool trait and does not read `ctx.agent_role`
(`suggest_worker_tool.rs:40`). **Port work: none** — but the restructure is the natural moment to
close F-ROU-01/02/03, because it is the same "agent path vs UI path" refactor that Capability 4
needs.

### 4.8 Defects

* **F-ROU-01 (CRITICAL, learning inert).** The outcome half of routing history is written only by
  the Tauri command (`commands/task_commands.rs:51-63`); the agent tool
  `update_task_status_tool.rs` (286 lines, no `record_outcome` call) does not. In autonomous
  operation `routing_history.outcome` stays NULL forever, the `>= 3` sample gate never opens
  (`scorer.rs:70`), and no worker ever gets a historical score. **CONFIRMED.** *Failure scenario:*
  the fleet runs for a month, the Worker Affinity panel shows every worker at 0 successes / 0
  failures / 0% success rate, and `suggest_worker` returns `method="default"` for every task
  forever — the feature looks alive (rows accumulate, the panel renders) but has never influenced a
  single decision.

* **F-ROU-02 (HIGH, wrong recommendation).** With all scores tied at 0.5, the stable sort
  (`scorer.rs:101`) plus `WorkersDb::list()`'s `ORDER BY is_system_defined DESC, id ASC`
  (`db/workers.rs:209`) makes `scores.remove(0)` (`scorer.rs:140`) return `supervisor`, and
  `score_workers` never excludes non-implementer roles (it maps over every row in `workers`,
  `scorer.rs:65-99`). **CONFIRMED.** *Failure scenario:* on a fresh install the Supervisor calls
  `suggest_worker(task_id=1)`, receives `{"worker_slug":"supervisor","method":"default"}`, and
  either wastes the call or — worse — tries `assign_task(1,"supervisor")` and gets a hard error,
  producing a retry loop on the very first task of the run.

* **F-ROU-03 (HIGH, dead branch).** The static-pattern layer can never fire:
  `if static_conf > 0.85` (`scorer.rs:121`) versus max declared confidence `0.85`
  (`patterns.rs:15, 21`). Every keyword pattern is computed (`routing/mod.rs:36-39`) and thrown
  away. **CONFIRMED.** *Failure scenario:* a task titled "Add FastAPI endpoint with Pydantic
  schema" scores 4 backend keyword hits, and the router still returns `method="default"` /
  `supervisor`.

* **F-ROU-04 (HIGH, attribution corruption — latent).** `claim_task_with_pid`
  (`db/tasks.rs:895-902`) does not check `assigned_worker`, so a task can be stolen and its owner
  overwritten while `routing_history.worker_slug` keeps the original name; `update_routing_outcome`
  matches on `task_id` alone (`db/tasks.rs:990`). **CONFIRMED.** *Failure scenario:* after
  F-ROU-01 is fixed, worker-cleanup steals a worker-backend task, fails it, and the failure is
  recorded against worker-backend — the scorer then routes backend work away from the worker that
  is actually competent at it.

* **F-ROU-05 (MEDIUM, sampling).** `get_routing_history(200)` (`routing/mod.rs:42`,
  `db/tasks.rs:996-1001`) caps the learning window at the 200 most recent rows **globally**, then
  filters by task_type (`scorer.rs:37`). With 5 workers and 6 task types, a busy board can push a
  worker's entire history for a given type out of the window, silently resetting its affinity.
  `record_outcome_auto` has the same 200-row scan to find its own pending row
  (`routing/mod.rs:91-93`), so on a busy board an outcome can fail to attach at all. **CONFIRMED.**

* **F-ROU-06 (MEDIUM, index).** `routing_history` has indexes on `worker_slug`, `task_type`, and
  `outcome` (`db/tasks.rs:429-434`) but **none on `task_id`** — the column `update_routing_outcome`
  filters on (`db/tasks.rs:990`) — and none on `assigned_at`, the column every read orders by
  (`db/tasks.rs:1001`). Both hot paths are full scans. **CONFIRMED.**

* **F-ROU-07 (LOW-MEDIUM, schema).** `routing_history.task_id` is `TEXT NOT NULL`
  (`db/tasks.rs:419`) holding a stringified integer (`routing/mod.rs:55`), with no FK to `tasks(id)`
  — so `clear_all` (`db/tasks.rs:603-612`) leaves orphaned routing rows that keep feeding the
  scorer after the board is wiped. **CONFIRMED.**

* **F-ROU-08 (LOW, timestamp inconsistency).** Within one table, `assigned_at` is RFC3339 written
  in Rust (`routing/mod.rs:59`) while `outcome_at` is SQLite `datetime('now')`
  (`db/tasks.rs:987`). The RFC3339 parse at `routing/mod.rs:94` is therefore **correct** for
  `assigned_at` (this is *not* the sibling audits' defect), but the two columns are not comparable
  and `ORDER BY assigned_at` cannot be mixed with any `datetime()` predicate. **CONFIRMED.**

* **F-ROU-09 (LOW, dead code).** `TasksDb::get_worker_success_rates` (`db/tasks.rs:1022-1045`) has
  zero callers. **CONFIRMED.**

* **F-ROU-10 (LOW, math).** With a single qualifying worker, `min_dur == max_dur`, `dur_range`
  falls back to `1.0` (`scorer.rs:63`), and `duration_score` evaluates to `1.0` — the lone worker
  gets full speed credit regardless of how slow it is. **CONFIRMED.**

### 4.9 What the restructure must preserve

1. **The two-layer design** — cheap static keyword routing first, historical affinity as the
   refinement. Both layers are sound; the blend threshold is off by an epsilon (F-ROU-03).
2. **The 0.6 success / 0.4 speed weighting and the `>= 3`-sample confidence gate**
   (`scorer.rs:70, 82-83`) — a defensible cold-start policy worth keeping verbatim.
3. **`routing_history` as the append-only affinity ledger**, including `duration_ms` and
   `tokens_used`, and its per-assignment write from both `assign_task` and `assign_tasks_batch`.
4. **The affinity dashboard contract** (`WorkerAffinityStats`) consumed by
   `WorkerAffinityPanel.tsx` — the only visible proof the fleet is learning.
5. **The invariant the restructure must add:** every terminal status transition records an outcome
   *on the agent path*, keyed to the worker that actually did the work — i.e. fix F-ROU-01 and
   F-ROU-04 together, and exclude non-implementer roles from the candidate set (F-ROU-02).

---

## 5. Capability 4 — Epic / phase decomposition

### 5.1 The epic -> phase -> task model

There is **no separate epic table**. An epic is a row in `tasks` with `task_type='epic'`
(`db/tasks.rs:118` enum variant, `:1199-1207` insert). `create_epic` hardcodes
`priority='p1', status='ready', acceptance_criteria='All subtasks complete'` (`:1202`) and takes no
worker. Subtasks are ordinary `tasks` rows carrying five phase columns added by migration
(`db/tasks.rs:392-397`): `phase TEXT`, `phase_order INTEGER`, `milestone TEXT`, `epic_id TEXT`,
`blocked_by TEXT` (a JSON array of stringified task ids, `:1212-1217`). Indexes:
`idx_tasks_epic_id` and `idx_tasks_phase(phase, phase_order)` (`:408-413`).

`create_phased_task` (`db/tasks.rs:1210-1242`) chooses the initial status by a single rule:

```rust
let initial_status = if input.blocked_by.is_empty() { "ready" } else { "needs_info" };  // :1219
```

The tool wrapper (`create_phased_task_tool.rs:78-207`) validates that `epic_id` parses as an int
and points at a row whose `task_type == Epic` (`:119-137`), and that every `blocked_by` id refers to
an existing task (`:141-162`) — but **not** that blockers belong to the same epic, are in an earlier
phase, or are acyclic.

`plan/` is a 4-line re-export shim over `db/plans.rs` (`plan/mod.rs:1-4`) and carries no epic logic.

### 5.2 How phase gating actually works

**Gating is entirely `blocked_by`-driven, and `phase_order` is decorative.**

* `phase_order` is used only for **ordering and display**: `get_epic_tasks` orders by
  `COALESCE(phase_order, 999), id` (`db/tasks.rs:1258`); `get_phase_summary` groups by
  `(order, phase)` in a `BTreeMap` (`:1270-1276`) and computes `is_unlocked` as "first phase, or the
  previous phase `is_complete`" (`:1291-1297`).
* **`is_unlocked` is advisory only.** Nothing consults it before dispatch: `assign_task`
  (`assign_task.rs:96-105`) and `assign_tasks_batch` (`:160`) check `status` alone. A phase-3 task
  created with an empty `blocked_by` is `ready` immediately and will be dispatched in the same
  Supervisor tick as phase-1 work.
* The real gate is `unblock_ready_tasks` (`db/tasks.rs:1343-1405`): on a task reaching `done`, it
  scans `get_blocked_tasks()` (`:1320-1337`), and for each task listing the completed id among its
  blockers, re-checks that **all** blockers are `Done` (`:1358-1362`) before flipping
  `needs_info -> ready` (`:1365-1368`) and emitting `task:changed{kind:"unblocked"}`.

### 5.3 Progress roll-up

`get_phase_summary` (`db/tasks.rs:1266-1317`) counts per phase: `done`, `in_progress`,
`ready` (= `Ready` **or** `Assigned`, `:1286`), `blocked` (= `NeedsInfo` **and** non-empty
`blocked_by`, `:1287`), `is_complete = total > 0 && done == total` (`:1288`).
`get_epic_progress_tool.rs:63-86` sums those into `total_tasks`, `total_done`, `all_complete`.

Roll-up is **arithmetically correct but blind to failure**: a task in `failed`, `build_failed`,
`rejected` or `qc_review` is counted in `total_tasks` and in none of the four buckets, so a phase
with one permanently-failed task shows `done 4 / total 5`, `is_complete=false`, `blocked=0`,
`ready=0` — indistinguishable from "someone is still working" (see F-EPI-04).

Epic auto-completion lives at `db/tasks.rs:1381-1401`: when a subtask completes and
`get_epic_tasks(epic_id)` shows every subtask `Done`, the epic row is set `done` with
`completed_at` and a `task:changed{kind:"epic_complete"}` event fires.

### 5.4 Wired end to end?

**No — the writer is unreachable from the fleet, and the gate is unreachable from the agent path.**

| Leg | Evidence | Verdict |
|---|---|---|
| `create_epic` / `create_phased_task` tools | `create_epic_tool.rs`, `create_phased_task_tool.rs` | exist |
| Registered for MCP fleet | **absent** from `register_for_mcp_worker` (`mod.rs:96-164`) | **GAP** |
| Registered for PA | `mod.rs:222-223` (`register_for_mode`, FastDevelopment/Review only) | OK |
| MCP exclusion is deliberate | `mcp_server/server.rs:140-141`: "NOT exposed (intentional): create_epic_tool / create_phased_task_tool — Supervisor/PA-only" | **self-contradictory — see F-EPI-01** |
| Read tools for fleet | `get_epic_progress`, `get_ready_tasks_in_phase` (`mod.rs:124-125`) | OK |
| Blocker gate | `unblock_ready_tasks` called **only** at `commands/task_commands.rs:71` | **GAP — UI path only** |
| Tauri commands | `get_epic_tasks`, `get_epic_phase_summary`, `create_phased_task` (`task_commands.rs:187, 195, 205`; `lib.rs:59, 218`) | OK |
| Frontend reader | `stores/taskStore.ts:211-227`, `components/dashboard/EpicPhaseView.tsx:162-196`, tab at `AutoDevPanel.tsx:867-871` | OK |
| Frontend creator | none — `grep create_phased_task src/**` finds only a help string (`EpicPhaseView.tsx:285`) | **GAP** |
| Recipe usage | Supervisor Step 2a says use **`create_task`** (flat), not `create_epic` (`defaults.rs:288-294`); supervisor tool list omits epic tools (`:349-363`) | **GAP** |

### 5.5 Can an epic be assigned to a worker as if it were a task?

**YES. CONFIRMED.** `create_epic` inserts with `status='ready'` (`db/tasks.rs:1202`). `assign_task`'s
only precondition is `matches!(prev_status, "ready"|"assigned"|"failed"|"rejected")`
(`assign_task.rs:96-99`) — there is no `task_type` filter anywhere in the file, and
`assign_tasks_batch.rs:160` has the identical check. The Supervisor recipe's Step 2 dispatch loop
says "For every task with `status='ready'`" (`defaults.rs:251`) and the Routing Matrix's only epic
row says "decompose first (Step 2a)" (`:286`) — prose, not code.

*Failure scenario:* PA creates epic #10 ("Implement OAuth2 Login Flow", `status=ready`,
`acceptance_criteria='All subtasks complete'`). The next Supervisor tick sees a ready task, matches
"login/flow" against no pattern, and batch-assigns #10 to worker-backend. The worker claims it
(`claim_task_with_pid` also has no `task_type` filter, `db/tasks.rs:895-902`), reads an acceptance
criterion it cannot satisfy, and either implements the entire epic in one unbudgeted session or
escalates. Meanwhile the epic's own auto-completion path (`db/tasks.rs:1381-1401`) can now never
fire correctly because the epic row itself is in `in_progress`.

### 5.6 Partial failure behaviour

* **A subtask that never reaches `done` permanently blocks its dependents.** `unblock_ready_tasks`
  only ever flips `needs_info -> ready` when **all** blockers are `Done` (`:1358-1363`); a blocker
  ending in `failed`/`rejected` leaves dependents in `needs_info` with no timeout, no alert, and no
  "unblockable" state. The Supervisor's Step 4 stop condition (`defaults.rs:331-340`) does not
  consider `needs_info`, so the board reports **"Board clean — idle."** while an entire epic is
  stuck.
* **`needs_info` is triple-overloaded**: blocked-by-dependency (`db/tasks.rs:1219`),
  budget-exceeded (`agent/engine.rs:528`), and worker-asked-a-question (`request_supervisor_action`
  `needs_info`, `defaults.rs:235`). Nothing distinguishes them except the presence of `blocked_by`.
* **Epic auto-completion is all-or-nothing** and fires only from the UI command path (§5.4), so an
  epic whose subtasks all complete under fleet operation stays `ready` forever — and is therefore
  eligible for dispatch (§5.5).

### 5.7 Survives sidecar deletion?

**YES — the model survives; it was never coupled to the sidecar.** Every leg is
SQLite + Rust + Tauri + React. The coupling that matters is the opposite one: the capability is
**gated by the MCP registry split** (`register_for_mcp_worker` vs `register_for_mode`,
`mod.rs:96-164` vs `:174-235`). Deleting the sidecar removes the reason that split exists, which is
the opportunity to fix F-EPI-01 by giving the Supervisor the epic tools directly.

**Port work:**
1. Expose `create_epic` / `create_phased_task` to the Supervisor role in the unified registry.
2. Move `unblock_ready_tasks` + phase-complete emission out of `commands/task_commands.rs:65-98`
   into the shared status-transition path so the agent tool triggers it too (same refactor as
   F-ROU-01).
3. Add `task_type != 'epic'` to the dispatch precondition.

### 5.8 Defects

* **F-EPI-01 (CRITICAL, unreachable writer).** The only agents that can create epics or phased
  tasks are PA-mode agents (`mod.rs:222-223`); the MCP registry deliberately omits both tools
  (`mcp_server/server.rs:140-141`), yet the Supervisor — the intended user per those very comments
  — runs Mode2Only (`defaults.rs:220`) and therefore reaches tools **only** through
  `register_for_mcp_worker`. The Supervisor recipe never mentions the tools and decomposes with
  flat `create_task` instead (`defaults.rs:288-294`, tool list `:349-363`). **CONFIRMED.**
  *Failure scenario:* the entire phase/milestone/blocker model — schema, tools, commands, and the
  Epics UI tab — is exercised by nothing in a normal fleet run; `EpicsSection` renders an empty
  list forever.

* **F-EPI-02 (CRITICAL, epic dispatched as work).** No dispatch or claim path filters
  `task_type='epic'` (`assign_task.rs:96-99`, `assign_tasks_batch.rs:160`,
  `db/tasks.rs:895-902`) while `create_epic` sets `status='ready'` (`db/tasks.rs:1202`).
  **CONFIRMED.** Scenario in §5.5.

* **F-EPI-03 (CRITICAL, gate on the wrong path).** `unblock_ready_tasks` and the
  `task:phase-complete` emission run only inside the Tauri command `update_task_status`
  (`commands/task_commands.rs:69-98`). The agent tool `update_task_status_tool.rs` (286 lines) never
  calls them. **CONFIRMED.** *Failure scenario:* a fleet run completes every phase-1 task; because
  no human touched the kanban, no dependent is ever unblocked, every phase-2 task sits in
  `needs_info` forever, the epic never auto-completes, and the Supervisor's Step 4 declares
  "Board clean — idle." with the epic 20% done.

* **F-EPI-04 (HIGH, roll-up blindness).** `get_phase_summary` buckets only
  `Done`/`InProgress`/`Ready|Assigned`/`NeedsInfo-with-blockers` (`db/tasks.rs:1284-1287`).
  `failed`, `build_failed`, `rejected`, and `qc_review` tasks vanish from every bucket while still
  counting toward `total_tasks`, so `is_complete` can never become true and the UI shows a phase
  frozen at N-1/N with zero indication of why. **CONFIRMED.**

* **F-EPI-05 (HIGH, no cycle/scope validation).** `create_phased_task_tool.rs:141-162` validates
  only that blocker ids exist. Nothing prevents `blocked_by` cycles (A blocks B blocks A), blockers
  from a different epic, or a blocker in a *later* phase. `unblock_ready_tasks` has no cycle
  detection either. **CONFIRMED.** *Failure scenario:* an LLM decomposition emits a mutual blocker
  pair; both tasks sit `needs_info` permanently and no code path can ever release them.

* **F-EPI-06 (MEDIUM, status overload).** `needs_info` means blocked, budget-halted, or
  question-pending (`db/tasks.rs:1219`; `agent/engine.rs:528`; `defaults.rs:235`), and
  `get_phase_summary` guesses between them by testing `blocked_by` non-empty (`:1287`). The
  Supervisor stop condition ignores `needs_info` entirely (`defaults.rs:331-340`). **CONFIRMED.**

* **F-EPI-07 (MEDIUM, identity).** Both epic tools write `created_by = ctx.session_id`
  (`create_epic_tool.rs:53`, `create_phased_task_tool.rs:175`) — a UUID — into a column whose other
  writers use role slugs (`db/tasks.rs:441` defaults to `'pa'`; `create_phased_task` defaults to
  `'supervisor'` at `:1211`). Provenance for epics is therefore unqueryable. **CONFIRMED.**

* **F-EPI-08 (MEDIUM, referential).** `epic_id` is `TEXT` holding a stringified `tasks.id` with no
  FK and no `ON DELETE` behaviour (`db/tasks.rs:396`); `blocked_by` is a JSON array of stringified
  ids (`:1212-1217`) resolved by `parse::<i64>()` at read time (`:1358-1361`). Deleting a task
  leaves dangling blocker references that silently evaluate to "not done" forever (a failed
  `get_by_id` maps to `false` at `:1361`). **CONFIRMED.**

* **F-EPI-09 (LOW, unused surface).** The Tauri command `create_phased_task`
  (`commands/task_commands.rs:205-231`, registered `lib.rs:59`) has no frontend caller
  (`grep create_phased_task src/**` → only the placeholder text at `EpicPhaseView.tsx:285`).
  **CONFIRMED.**

### 5.9 What the restructure must preserve

1. **Epic-as-a-task** — a single `tasks` table with `task_type='epic'` plus five phase columns is a
   genuinely good simplification; keep it rather than introducing an epics table.
2. **`blocked_by` as the real dependency edge and `phase`/`phase_order` as the human-legible
   grouping** — the split works; what is missing is a code-level phase gate (F-EPI-03).
3. **`unblock_ready_tasks`'s all-blockers-done re-check** (`db/tasks.rs:1358-1363`) — correct and
   race-tolerant; it must simply be called from the agent path too.
4. **Epic auto-completion + `task:phase-complete` / `task:unblocked` events** — the events already
   drive `EpicPhaseView`'s live refresh (`EpicPhaseView.tsx:184-196`); keep the contract.
5. **`get_ready_tasks_in_phase`'s `status IN ('ready','assigned')` semantics**
   (`db/tasks.rs:1488`) — the correct definition of "dispatchable in this phase".
6. **The invariants to add:** epics are never dispatchable; a phase is not dispatchable until its
   predecessor is complete (promote `is_unlocked` from advisory to enforced); a failed/rejected task
   surfaces its phase as *stalled* rather than silently incomplete.

---

## 6. Capability 5 — Interface contracts and QC support

### 6.1 `write_interface_spec` — how the handshake really works

**Write side** (`write_interface_spec.rs:100-186`). Deserialises `routes` into `RouteSpec`
(`:34-44`; only `method` and `path` are required), renders markdown via `format_route`
(`:189-223`), and writes `<ctx.working_dir>/INTERFACE.md` (`:137, :174`). `mode='merge'` is a
**naïve append with no deduplication**, explicitly by design (`:146-152`). The tool declares
`requires_permission() -> false` (`:98`), so it writes to disk without a permission gate.

**Read side.** Two consumers:
1. **A recipe instruction** — the frontend worker's Step 3.0 "Read INTERFACE.md if it exists"
   (`defaults.rs:661-662`), and a nudge-handling line (`:650`).
2. **A code validator** — `validate::tier2_6_api_contract` (`validate.rs:650-678`), invoked from
   `write_file.rs:184-192` and `check_file.rs:170-178`. It extracts URLs from JS/TS content by
   regex (`extract_api_urls`, `validate.rs:680-700+`), locates `INTERFACE.md` by walking upward
   (`:660-661`), and reports any URL not found by **substring containment** in the spec text
   (`:670-675`).

### 6.2 Is the handshake enforced, or advisory?

**Advisory on both sides, with one soft code-level check. CONFIRMED.**

* **Producer side is pure prose.** The backend recipe's Step 3.5 says "you MUST call
  `write_interface_spec` before transitioning to qc_review" (`defaults.rs:415-431`), but
  `update_task_status_tool.rs` has no INTERFACE.md precondition anywhere in its 286 lines — the
  only guard on the `qc_review` transition is the build-evidence check (`:131-162`). A backend
  worker can ship an API-producing task straight to QC with no spec.
* **Consumer side is a warning, not a gate.** `write_file.rs:184-192` appends the api_contract
  result to `lint_parts`, and the tool still returns `ToolResult::success(...)` with the lint text
  attached (`:193-203`). The write happens regardless.
* **The check self-disables.** `tier2_6_api_contract` returns `None` — silently clean — when the
  file isn't JS/TS (`validate.rs:651-654`), when no URL is extracted (`:658`), or when no
  `INTERFACE.md` is found upward (`:660-661`). The last case is deliberate ("don't punish frontends
  that ship before the backend writes a spec", `:642-644`) but it means the common failure — backend
  forgot to write the spec — produces **zero** signal on the frontend side.
* **Containment matching is weak.** `spec.contains(&normalised)` (`:672`) passes as long as the path
  appears anywhere in the markdown, including inside a stale entry, a code fence, or a prose
  sentence. Combined with merge-mode's non-deduplication (`write_interface_spec.rs:146-152`), a
  deleted route stays "declared" forever.

### 6.3 Does anything verify the frontend read it?

**No. CONFIRMED.** There is no read receipt, no hash, no `interface_spec_read` flag on the task, and
no per-agent state. The nearest proxies are both indirect:
* the api_contract lint fires at *write* time on URL strings — it detects a mismatch, not a read;
* `analyze_diff`'s `interface_drift` field (`analyze_diff_tool.rs:154-212`) classifies a **patch**
  as `contract_only` / `consumer_only` / `both` / `neither` by regex over `+++ b/` markers
  (`:178-200`). `consumer_only` is the closest thing to "the frontend changed without reading the
  contract", and it is offered to the QC reviewer as advisory JSON only.

Note the drift heuristic's consumer test has an operator-precedence trap at
`analyze_diff_tool.rs:192-196`: `pl.contains("apiclient") || pl.contains("api_client") ||
pl.contains("use") && (...) || pl.contains("/hooks/") || pl.contains("/api/")` — `&&` binds tighter
than `||`, so the `use`-prefixed clause is a separate term. The result is what the comment intends
here, but any file path containing `/api/` or `/hooks/` counts as a consumer, so a **backend** route
file under `src/api/` is classified as a frontend consumer.

### 6.4 `analyze_diff` — real, but gated on a field that does not exist

`analyze_diff_tool.rs:39-151` generates a branch diff (`:78-85`), runs an **LLM** analyser built
from vault credentials and the `AUTODEV_ACTIVE_PROVIDER` / `AUTODEV_ACTIVE_MODEL` env vars with a
hardcoded fallback model (`:214-240`), adds the `interface_drift` field, and — when `task_id` is
passed — persists the JSON to `tasks.diff_analysis` and emits `task:changed{kind:"diff_analysis"}`
(`:127-146`).

**But both QC recipes gate Phase 0 on a `repo_path` "in the task's metadata"**
(`defaults.rs:1298`, `:1311`, `:1419`, and the reviewer prompt at `:1564-1566`) — and **the `tasks`
table has no `repo_path` column** (`grep repo_path src-tauri/src/db/tasks.rs` → no matches; the
column list in `row_to_task`'s queries, e.g. `db/tasks.rs:1246-1258`, has no such field).
`spawn_qc_review` accepts `repo_path` as a *tool input* (`spawn_qc_review.rs:49, 139`) and the
dispatcher recipe calls `spawn_qc_review(task_id=<task.id>)` with nothing else
(`defaults.rs:1826`), so `repo_path` is always `None`. **Diff risk analysis is therefore never run
in the automated QC flow.**

### 6.5 `annotate_file` — writes land, but detach from the task

`annotate_file_tool.rs:57-105` validates the range, infers `source` from `ctx.agent_role`
(`:77-81` — `qc-agent` -> `"qc_agent"`, `worker*` -> `"worker"`, anything else -> `"user"`), and
inserts into `annotations` (`db/annotations.rs:88-102`). The annotation is readable by file
(`list_for_file`, `db/annotations.rs:104`) and surfaces in the IDE panels
(`src/components/ide/AnnotationPanel.tsx`, `CodeViewer.tsx`, `stores/annotationStore.ts`).

Two gaps: `task_id` is hardcoded `None` (`annotate_file_tool.rs:91`) even though the column, the
input field and an `update_task_id` setter all exist (`db/annotations.rs:72, 169-174`); and there is
**no `list_by_task` query at all** (`db/annotations.rs` exposes only `list_for_file` and
`get_by_id`). So QC findings cannot be joined back to the task they rejected. Separately, the
`source` inference depends on `ctx.agent_role` and will collapse to `"user"` for every agent on the
in-house executor path (F-ACP-01).

### 6.6 `spawn_qc_review` — the best-engineered piece in scope

`spawn_qc_review.rs:74-270` is genuinely tight: depth guard (`:74-80`), status precondition
(`:105-116`), pre-check on `under_review_by` (`:117-121`), **atomic** claim via
`try_claim_for_qc_review`'s `WHERE under_review_by IS NULL` (`db/tasks.rs:1414-1420`), claim upgrade
to the real subagent id (`:185-190`), a detached completion handler that attributes
`cost_usd` to `tasks.task_qc_cost_usd` via `add_qc_cost` (`:209-218`), rolls the spend into an
hour-bucketed `qc_cost_guard` (`:222`), releases the claim on **both** the success and the error
path (`:225-227`, `:255-257`), and records analytics either way (`:230-238`, `:259-266`).

**Its one hard dependency is the sidecar**: it spawns through `SubagentManager`
(`spawn_qc_review.rs:171`), which is constructed from an `OrchestratorClient`
(`subagent/manager.rs:31, 55, 61`) — the core-orchestrator process — and the reviewer is configured
`tools: Vec::new()  // MCP-only` and `mcp_servers: Vec::new()  // default = autodev-mcp-server`
(`:157, 162`).

### 6.7 `e2e_probe` — what it proves, and what it leaves behind

**What it proves** (`e2e_probe.rs:167-257`): that after spawning N shell commands and sleeping a
fixed `boot_wait` (default 8 s, `:35, 216`), an HTTP GET to `probe_url` returns 2xx/3xx **and** the
raw response body contains `expected_text` (`:230-243`). No headless browser, no JS evaluation —
explicitly scoped to catching dev-server-didn't-boot / wrong-index.html / backend-health failures
(`:9-18`).

**What it does with the app afterwards**: it kills its children — `for c in children.iter_mut() {
let _ = c.kill().await; }` (`:224-226`), plus `kill_on_drop(true)` on each (`:186, 195`) so the
timeout path also tears down. Teardown runs on the spawn-failure path too (`:207`).

**The teardown is shallow.** Each child is a *shell* — `cmd /C <cmd>` on Windows, `sh -c <cmd>`
elsewhere (`:179-197`). Killing `cmd.exe` does not kill the process tree beneath it, so
`npm run dev` leaves `node.exe` alive holding the port. (On POSIX, `sh -c` with a simple command
typically `exec`s and is replaced by the child, so kill lands — the failure is Windows-specific,
which is this project's primary platform.) There is no `taskkill /T`, no process-group kill, and no
post-teardown port check.

The module header also claims path-jail composability — "the probe walks `working_dir` to launch its
children, so a misconfigured workspace can't reach the wrong tree" (`:20-22`) — but only
`current_dir` is set (`:182, 191`); the command string is arbitrary shell and can `cd` anywhere.
Docstring-vs-code mismatch.

**And nothing schedules it.** `e2e_probe` is registered in both registries (`mod.rs:139, 207`) and
referenced by the QC reviewer prompt as "REQUIRED before approval" (`defaults.rs:1633-1678, 1771`),
but the `fleet-smoke` recipe that was designed to drive it has `schedule: None`
(`defaults.rs:1876`), no `workers.db` row binding it (`db/workers.rs:175-190` has no `fleet-smoke`
seed), and no code path kicks it — `grep fleet.smoke` across `src-tauri/src` returns only the
recipe's own definition (`defaults.rs:41, 57, 1867, 1878, 1942`). The comment at `defaults.rs:36-40`
says it is "wired by the supervisor recipe", but the supervisor instructions (`:221-363`) never
mention it.

### 6.8 Wired end to end?

| Piece | Writer | Reader | UI | Verdict |
|---|---|---|---|---|
| INTERFACE.md | `write_interface_spec.rs:174` | recipe prose `defaults.rs:661`; `validate.rs:650` via `write_file.rs:184`, `check_file.rs:170` | none | advisory; no read receipt |
| `analyze_diff` | `analyze_diff_tool.rs:129` -> `tasks.diff_analysis` | AutoDev panel DiffAnalysisCard (via `task:changed`) | yes | **never invoked — gated on nonexistent `repo_path`** |
| `annotate_file` | `annotate_file_tool.rs:95` | `db/annotations.rs:104` `list_for_file`; IDE panels | yes | wired by file, **not by task** |
| `spawn_qc_review` | claim + spawn `:130, :171` | `qc-dispatcher` recipe `defaults.rs:1826` | QC ROI tab (`AutoDevPanel.tsx:872-880`) | fully wired; **sidecar-bound** |
| `e2e_probe` | n/a (returns a report) | QC reviewer prompt `defaults.rs:1646-1678` | none | **no scheduler; leaks child processes on Windows** |

### 6.9 Survives sidecar deletion?

| Piece | Survives | Coupling |
|---|---|---|
| `write_interface_spec` | **YES** | pure `std::fs` + `ctx.working_dir` |
| `tier2_6_api_contract` | **YES** | pure regex + fs (`validate.rs:650`) |
| `analyze_diff` | **YES** | own provider stack from vault (`analyze_diff_tool.rs:214-240`) — no orchestrator |
| `annotate_file` | **PARTIALLY** | logic survives, but `source` attribution dies with `ctx.agent_role` (F-ACP-01) |
| `spawn_qc_review` | **NO** | `SubagentManager` is built on `OrchestratorClient` (`subagent/manager.rs:31, 55, 61`); reviewers are `MCP-only` subagents (`spawn_qc_review.rs:157, 162`) |
| `e2e_probe` | **YES** | `tokio::process` + `reqwest` only |

**Port work:** the entire per-task QC fan-out needs an in-house replacement for
`SubagentManager::spawn` / `await_result` that preserves the four behaviours the current code gets
right — atomic claim, cost attribution to `task_qc_cost_usd`, hour-bucketed cost guard, and claim
release on every terminal path.

### 6.10 Defects

* **F-QC-01 (HIGH, dead capability).** Diff risk analysis is gated on `repo_path` "in the task's
  metadata" (`defaults.rs:1298, 1311, 1419, 1564-1566`), a field that does not exist on `tasks`, and
  the dispatcher never passes `repo_path` to `spawn_qc_review` (`defaults.rs:1826` vs
  `spawn_qc_review.rs:139`). **CONFIRMED.** *Failure scenario:* every reviewer skips Phase 0, so
  `risk_level='critical'` never blocks a review, `tasks.diff_analysis` is never populated, and the
  AutoDev panel's risk card never renders — while the recipe text asserts the check ran.

* **F-QC-02 (HIGH, unenforced contract).** Nothing prevents `update_task_status(id,'qc_review')` on
  an API-producing task with no INTERFACE.md; the only guard on that transition is build evidence
  (`update_task_status_tool.rs:131-162`). The consumer-side check is a lint appended to a
  **success** result (`write_file.rs:184-197`) and self-disables when the spec is absent
  (`validate.rs:660-661`). **CONFIRMED.** *Failure scenario:* exactly the `/api/api/events`
  double-prefix bug the recipe cites as motivation (`defaults.rs:424-428`) reproduces, because the
  one case the checker cannot see is the one where the backend skipped the spec.

* **F-QC-03 (HIGH, orphan processes — Windows).** `e2e_probe` kills only the shell it spawned
  (`e2e_probe.rs:224-226` with `cmd /C` at `:180-181`), not the process tree, and never verifies the
  port was released. **CONFIRMED** (mechanism). *Failure scenario:* one probe of a Vite dev server
  leaves `node.exe` holding port 3000; the next probe's dev server fails to bind, the probe reports
  FAIL, QC rejects a correct task, and the leaked process survives until the machine is rebooted.

* **F-QC-04 (MEDIUM, unreachable recipe).** `fleet-smoke` has `schedule: None`
  (`defaults.rs:1876`), no `workers.db` binding (`db/workers.rs:175-190`), and no kick site
  anywhere. The Doc 09 T4.1 fleet-level smoke gate does not run. **CONFIRMED.**

* **F-QC-05 (MEDIUM, lost provenance).** `annotate_file` hardcodes `task_id: None`
  (`annotate_file_tool.rs:91`) and `db/annotations.rs` has no by-task query, so QC findings cannot
  be listed for the task they rejected. **CONFIRMED.**

* **F-QC-06 (MEDIUM, spec rot).** `mode='merge'` appends without dedup
  (`write_interface_spec.rs:146-152`) and the contract check is substring containment
  (`validate.rs:672`), so deleted or superseded routes remain "declared" indefinitely and satisfy
  the check. **CONFIRMED.**

* **F-QC-07 (MEDIUM, unmetered LLM).** `analyze_diff` builds its own provider from the vault and an
  env-var model with a hardcoded default (`analyze_diff_tool.rs:214-240`) and calls
  `analyzer.analyze(&patch)` (`:87`) with the full patch — no size cap, no budget check, no cost
  attribution to the task. **CONFIRMED** (currently masked by F-QC-01).

* **F-QC-08 (LOW-MEDIUM, misclassification).** `detect_interface_drift`'s consumer heuristic
  (`analyze_diff_tool.rs:190-199`) treats any `.ts`/`.tsx` path containing `/api/` or `/hooks/` as a
  frontend consumer, so backend route modules under `src/api/` are classified as consumers and a
  backend-only diff can report `status:"both"`. **CONFIRMED.**

* **F-QC-09 (LOW, permission).** `write_interface_spec` declares `requires_permission() -> false`
  (`:98`) and writes to the workspace root unconditionally; `mode='overwrite'` is the default
  (`:118`), so a single mis-issued call silently destroys the whole contract file. **CONFIRMED.**

* **F-QC-10 (LOW, docstring vs code).** `e2e_probe`'s header claims path-jail composability
  (`:20-22`) while only `current_dir` is set (`:182, 191`) and the command string is unconstrained
  shell. **CONFIRMED.**

### 6.11 What the restructure must preserve

1. **INTERFACE.md as a file-based, human-readable contract** at the workspace root — it is the only
   artefact both workers and a human can read, and the drift class it targets is real.
2. **The `tier2_6_api_contract` pass firing inside `write_file`/`check_file`** — feedback at write
   time, not at QC, is the right place; make it a hard failure for the frontend once the spec
   exists, and add a producer-side gate on the `qc_review` transition.
3. **`spawn_qc_review`'s four invariants** — atomic `under_review_by` claim, cost attribution to
   `task_qc_cost_usd`, hour-bucketed cost guard, claim release on every terminal path
   (`spawn_qc_review.rs:130, 209-218, 222, 225-227, 255-257`). These must survive the
   `SubagentManager` port intact.
4. **`analyze_diff`'s persisted `diff_analysis` JSON + `task:changed{kind:"diff_analysis"}` event**
   — the panel already consumes it; only the trigger is broken.
5. **`e2e_probe`'s always-tear-down contract** (`:224-226`) — with a real process-tree kill and a
   post-teardown port check.
6. **The QC ROI accounting surface** (`task_qc_cost_usd`, the QC ROI tab at
   `AutoDevPanel.tsx:872-880`) — the only per-task cost signal that is actually populated today.

---

## 7. Capability 6 — Budget and cost controls at the worker level

### 7.1 How it is designed to work

`budget/mod.rs` (130 lines, the whole module) defines `TaskBudget { max_tokens, max_usd, max_turns }`
(`:15-20`), `BudgetStatus { tokens_used, cost_usd, turns_used, exceeded, exceeded_reason }`
(`:22-29`), three presets (`default_worker` 100k/$0.50/20, `default_supervisor` 50k/$0.25/10,
`unlimited` — `:51-70`), the check in token -> cost -> turn priority (`:74-85`), and
`validate_budget` with hard ceilings 1,000,000 tokens / $100 / 100 turns (`:90-118`).

The intended enforcement point is the agent loop: at the top of every turn,
`budget_status.turns_used += 1` then `if let Some(ref budget) = self.task_budget { if let
Some(reason) = budget.check(&budget_status) { ... return Err(BudgetExceeded) } }`
(`agent/engine.rs:148-166`). On trip it emits `agent:budget-exceeded` (`:153-160`) and calls
`escalate_budget_exceeded` (`:162`), which writes a `report_issue` worker request with
`worker_slug="engine"`, marks the task budget-exceeded, and flips it to `needs_info`
(`:480-536`, `db/tasks.rs:1104-1111`). After each provider response it persists cumulative usage
via `update_task_budget_usage` (`engine.rs:216-230`, `db/tasks.rs:1088-1101`).

### 7.2 What a budget gates, and when it is checked — in reality

**It gates nothing. The circuit breaker cannot fire. CONFIRMED.**

The sibling audit's finding is correct and the blast radius is wider than "the breaker never fires":

1. **`with_task_budget` has zero callers.** It is defined at `agent/engine.rs:73-77` and referenced
   nowhere else (`grep with_task_budget src-tauri/src` returns only that definition and the comment
   at `:45`). `AgentEngine::new` initialises `task_id: None, task_budget: None` (`:62`).
2. **Therefore `self.task_budget` is always `None`**, and the `if let Some(ref budget)` at
   `engine.rs:150` never enters. `TaskBudget::check` is never called from production code.
3. **Therefore `self.task_id` is always `None`**, so the usage write at `engine.rs:220-229` never
   runs. `budget_tokens_used`, `budget_cost_usd`, `budget_turns_used` stay at their column defaults
   of 0 for every task, forever (`db/tasks.rs:387-389`).
4. **Therefore `escalate_budget_exceeded` (`engine.rs:480-536`) is dead code**, and with it the only
   non-agent writer into `worker_requests`.
5. **Therefore `emit_budget_exceeded` never fires** — its sole call site is `engine.rs:153`
   (`grep emit_budget_exceeded` → `events.rs:490` definition + `engine.rs:153`). The frontend
   listeners in `stores/taskStore.ts:263-277` and `components/budget/BudgetBar.tsx:81` are
   subscribed to an event that is never emitted.
6. **And even a fix to (1) would not reach the fleet today**, because `AgentEngine` is the **Mode 1**
   last-resort path. Every fleet recipe except `worker-cleanup` is `FallbackPolicy::Mode2Only`
   (`defaults.rs:220, 382, 634, 849, 958, 1286, 1804, 1881`; cleanup is `Mode1Capable` at `:1135`),
   and Mode 2 runs the LivingAgent inside the core-orchestrator (`recipes/runner.rs:5, 129-130,
   192-196`). The Mode 2 branch does its own token accounting through `CostTracker` and
   `record_token_usage` (`runner.rs:382, 411`) and never consults `TaskBudget`.

**Two further budget paths are also inert:**

* **The Settings default budget has no reader.** `set_default_worker_budget`
  (`commands/budget_commands.rs:59-79`) persists `default_worker_budget_tokens/usd/turns` and
  `budget_warn_pct` into `runtime_config.json` with the comment "so the Rust recipe runner can read
  them when assigning tasks without an explicit budget override" (`:55-57`). Grep of the whole tree
  finds those fields only in the struct definition (`runtime_config.rs:50-69`), the setter
  (`budget_commands.rs:73-76`), and the Settings UI (`stores/runtimeConfigStore.ts:18-21, 65`,
  `tabs/Settings.tsx:335-338`). **No Rust consumer.** The user can set a fleet-wide default budget
  and nothing anywhere reads it.
* **No recipe ever calls `set_task_budget`.** The tool is registered for MCP workers
  (`mod.rs:122`) and PA modes (`:220`), and `mcp_server/server.rs:131` advertises it, but a grep for
  `set_task_budget` in `recipes/defaults.rs` returns **nothing** — the Supervisor is never told to
  budget a task before assigning it. Every occurrence of "budget" in the recipes is prose about
  "task budget" or the read-only "discovery budget" (`defaults.rs:82, 335, 408, 661, 668, 672`).

### 7.3 Wired end to end?

| Leg | Evidence | Verdict |
|---|---|---|
| Cap writer (tool) | `set_task_budget_tool.rs:79` -> `db/tasks.rs:1067-1085` | works, but **never invoked by any recipe** |
| Cap writer (Tauri) | `commands/budget_commands.rs:19-31`, `lib.rs:452` | works; no UI caller found |
| Default-budget writer | `commands/budget_commands.rs:59-79`, Settings UI | **no reader** |
| Enforcement | `agent/engine.rs:148-166` gated on `task_budget` | **unreachable — `with_task_budget` has 0 callers** |
| Usage writer | `agent/engine.rs:216-230` gated on `task_id` | **unreachable** |
| Escalation | `agent/engine.rs:480-536` | **unreachable** |
| Status reader | `db/tasks.rs:1114-1142` -> `get_task_budget_status` (`budget_commands.rs:38`) -> `BudgetBar.tsx:69` | reads, always zeros |
| Daily summary | `db/tasks.rs:1145-1166` -> `get_daily_budget_summary` | reads, always zeros |
| Event channel | `events.rs:490` `agent:budget-exceeded` | **never emitted**; two subscribers |
| Task columns | `db/tasks.rs:384-391` | present, unpopulated |

The only per-task cost figure that **is** populated is `task_qc_cost_usd`, written by
`spawn_qc_review`'s completion handler (`spawn_qc_review.rs:209-218`) — an entirely separate path
that does not go through `budget/`.

### 7.4 Survives sidecar deletion?

**YES — and the deletion is what finally makes it *possible*.** `budget/mod.rs` is pure arithmetic;
the columns are SQLite; the tool and commands are Rust; the UI is wired and waiting. The reason the
breaker cannot work today is that enforcement lives in `AgentEngine`, which the Mode2Only fleet
never executes. Once the sidecar is deleted and all agent execution runs through the in-house Rust
loop, `agent/engine.rs:148-166` becomes the real chokepoint for every worker tick.

**Port work (small, high value):**
1. Call `with_task_budget(task_id, budget)` from the recipe runner when it dispatches a worker tick
   for a task, resolving the budget as: task columns -> `runtime_config.default_worker_budget_*` ->
   `TaskBudget::default_worker()`.
2. Give `set_task_budget` a place in the Supervisor recipe (or apply the default automatically at
   `assign_task` time — the tool description already says "called by Supervisor before assigning",
   `db/tasks.rs:1066`).
3. Make `set_task_budget` persist the defaults it claims to apply (F-BUD-03).
4. Wire `budget_warn_pct` into the usage-write path so `BudgetBar` can warn before the halt.

### 7.5 Defects

* **F-BUD-01 (CRITICAL, inert control).** `with_task_budget` (`agent/engine.rs:73-77`) has zero
  callers, so `task_budget` and `task_id` are permanently `None` (`:62`); the circuit breaker
  (`:148-166`), the usage persistence (`:216-230`), the escalation (`:480-536`) and the
  `agent:budget-exceeded` event (`:153`) are all unreachable. **CONFIRMED.** *Failure scenario:* a
  worker enters a tool-call loop on a malformed task; nothing halts it except `MAX_TOOL_TURNS`
  (`engine.rs:168`) — and in Mode 2 not even that, since the Rust loop isn't running. The user sees
  spend accumulate in the cost meter with no per-task attribution, no cap, and no alert. The
  `BudgetBar` for that task renders 0 / 0 / 0.

* **F-BUD-02 (HIGH, no reader).** `default_worker_budget_tokens/usd/turns` and `budget_warn_pct` are
  written to `runtime_config.json` by the Settings panel and read by **nothing** in Rust
  (`runtime_config.rs:50-69` definition + `budget_commands.rs:73-76` setter are the only
  occurrences). The command's own doc-comment asserts the recipe runner reads them
  (`budget_commands.rs:55-57`) — it does not. **CONFIRMED.** *Failure scenario:* the user sets a
  $2/task ceiling in Settings, sees it persist and reload correctly, and it has no effect whatsoever.

* **F-BUD-03 (HIGH, lying success).** `set_task_budget_tool` computes "effective" defaults
  (100,000 / $0.50 / 20) **for the message only** (`set_task_budget_tool.rs:83-85`) and reports them
  as `budget_set` (`:92-102`), while `db.set_task_budget` writes the raw `Option`s — i.e. NULLs — to
  the columns (`:79`, `db/tasks.rs:1074-1083`). **CONFIRMED.** *Failure scenario:* the Supervisor
  calls `set_task_budget(task_id=7)`, is told "Budget set for task #7: 100000 tokens / $0.50 USD /
  20 turns max", records that in its reasoning, and the task in fact carries three NULL caps. Worse,
  the call **overwrites** the `budget_max_turns` column default of 20 (`db/tasks.rs:386`) with NULL.

* **F-BUD-04 (HIGH, unbudgeted spend paths).** Three fleet actions spend money with no budget
  interaction at all: `send_worker_message` triggers a full recipe/LLM tick per recipient
  (`send_worker_message.rs:204` -> `scheduler.rs:442-449`, up to 7 per broadcast);
  `analyze_diff` runs an LLM over an unbounded patch (`analyze_diff_tool.rs:78-87`); and
  `spawn_qc_review` spawns reviewer subagents whose only ceiling is the hour-bucketed
  `qc_cost_guard` (`spawn_qc_review.rs:222`), not the task budget. **CONFIRMED.**

* **F-BUD-05 (MEDIUM, status collision).** `mark_task_budget_exceeded` forces the task to
  `needs_info` (`db/tasks.rs:1104-1111`), the same status used for dependency-blocked tasks
  (`:1219`) and worker questions (`defaults.rs:235`). `get_phase_summary` will count a
  budget-halted task as *not* blocked (it has no `blocked_by`, `:1287`), so it disappears from every
  phase bucket. **CONFIRMED** (latent behind F-BUD-01).

* **F-BUD-06 (MEDIUM, accounting scope).** `budget_status.tokens_used` is set to
  `total_tokens_in + total_tokens_out` for the **whole engine session**
  (`engine.rs:218`), not per task, and `CostTracker::calculate` is re-run on cumulative totals each
  turn (`:219`). If one engine session ever served two tasks, the second would inherit the first's
  spend. **CONFIRMED by inspection** (latent behind F-BUD-01).

* **F-BUD-07 (LOW, presets unused).** `TaskBudget::default_worker`, `default_supervisor`, and
  `unlimited` (`budget/mod.rs:51-70`) have no callers. **CONFIRMED.**

* **F-BUD-08 (LOW, turn accounting).** `turns_used` is incremented *before* the check
  (`engine.rs:149-150`), so `max_turns: Some(1)` halts before the first LLM call ever happens
  (`check` uses `>=`, `budget/mod.rs:81`). Off-by-one against the natural reading of "max turns".
  **CONFIRMED.**

### 7.6 What the restructure must preserve

1. **The three-axis budget (tokens, USD, turns) with a validated ceiling** — `validate_budget`'s
   NaN/Inf/negative/ceiling rejection (`budget/mod.rs:97-118`) is correct and worth keeping verbatim.
2. **Pre-emptive halt at the top of the turn, before spending** (`engine.rs:148-166`) — checking
   before the provider call rather than after is the right design.
3. **Halt -> escalate, not halt -> die**: the `report_issue` worker request plus `needs_info`
   transition (`engine.rs:480-536`) hands the decision back to the Supervisor. This is the single
   best idea in the budget subsystem and it has never executed once.
4. **The per-task usage columns and `BudgetBar`/daily-summary readers** — the whole read side is
   built and waiting for data.
5. **`task_qc_cost_usd` + `qc_cost_guard`** (`spawn_qc_review.rs:209-222`) — the one cost control
   that demonstrably works; fold it into the unified budget model rather than replacing it.
6. **The invariant to add:** a worker tick that spends tokens on a task always runs under a resolved
   budget (task -> runtime default -> preset), and every spend path (messages, diff analysis, QC
   reviewers, subagents) debits the same account.

---

## 8. Findings table

Severity legend: **C** critical · **H** high · **M** medium · **LM** low-medium · **L** low.
Class: `inert` (built but never executes) · `gap` (missing leg) · `bug` · `leak` · `contract`
· `schema` · `cost` · `ts` (timestamp).

| ID | file:line | Sev | Class | What it does | Why it is wrong | Concrete failure scenario | Status |
|---|---|---|---|---|---|---|---|
| **F-XCUT-01** | `agent/tool_executor.rs:89, 252, 316` | C | gap | Builds `ToolContext` with `agent_role: None` at all 3 in-house sites; only `mcp_server/tool_adapter.rs:280` sets it | Agent identity exists only at the MCP boundary — the boundary the restructure deletes | Post-sidecar, all 3 ACP tools hard-refuse, `annotate_file` sources collapse to `"user"`, `qc_review` gate at `update_task_status_tool.rs:133` changes behaviour | CONFIRMED |
| F-WRQ-01 | `db/tasks.rs:603-612`; `defaults.rs:239` | H | leak | No expiry/reaper for `worker_requests`; only `clear_all` deletes | An unresolvable pending request is re-read every 60 s and Step 1a's "STOP after this step" starves dispatch | Malformed `decompose_audit`; Supervisor can't resolve it; board freezes with tasks in `ready` | CONFIRMED |
| F-WRQ-02 | `db/tasks.rs:643-658` | M | contract | `list_worker_requests` has no LIMIT / age filter | Unbounded history + payload blobs into an LLM context | Unfiltered call after 300 requests eats the Supervisor's context | CONFIRMED |
| F-WRQ-03 | `db/tasks.rs:674-678`; `defaults.rs:235-236` | LM | contract | `acknowledged` sets `resolved_at = NULL` yet is the terminal action for 2/5 types | Terminal-in-practice rows carry no resolution time | No age-based reporting possible on `needs_info`/`report_issue` escalations | CONFIRMED |
| F-WRQ-04 | `tabs/AutoDevPanel.tsx:574, 587` | L | gap | Widget offers only `completed`/`rejected`; hides when empty | Human cannot record `acknowledged`; no history view | Operator must use the chat agent to acknowledge a request | CONFIRMED |
| F-WRQ-05 | `request_supervisor_action.rs:101-109` | L | bug | Falls back to `worker_slug="unknown"` | With no `task_id` and no `agent_role`, the request has no addressee | Supervisor's `reassign` branch has nobody to reassign from | CONFIRMED |
| F-WRQ-06 | `db/tasks.rs:332`; `request_supervisor_action.rs:51-56` | L | contract | `payload` is opaque TEXT, never validated per type | Tool description promises per-type shapes; nothing enforces | `decompose_audit` with a malformed `suggested_tasks` reaches the Supervisor | CONFIRMED |
| **F-ACP-01** | `agent/tool_executor.rs:89, 252, 316`; `send_worker_message.rs:129-137` | C | gap | ACP identity comes only from MCP `__agent_role` | Entire ACP hard-refuses on the in-house path | Day one post-sidecar: every worker's Step 1.5 inbox call errors, cross-worker coordination stops silently | CONFIRMED |
| F-ACP-02 | `send_worker_message.rs:33-34, 172-173`; `defaults.rs:1798`; `db/workers.rs:175-190` | H | gap | `qc-agent`/`qc-dispatcher` accept mail; no recipe reads it; no `qc-dispatcher` worker row or role | Two of eight recipients are dead-letter boxes; every broadcast writes 2 undeliverable rows | Unread badges for qc-agent/qc-dispatcher climb forever and cannot be cleared | CONFIRMED |
| F-ACP-03 | `db/tasks.rs:754-775, 799`; `list_worker_messages.rs:58`; `defaults.rs:400` | H | bug | Listing flips `pending -> read`; nothing re-surfaces un-acked `read` rows (`list_stale_worker_messages` has 0 callers) | Recipe claims "lossless"; delivery is at-most-once | Worker lists 3 messages, tick errors out, questions are never answered and the asker waits forever | CONFIRMED |
| F-ACP-04 | `send_worker_message.rs:168-207`; `scheduler.rs:442-449` | M | cost | Each send kicks a full recipe/LLM tick; broadcast = 7 kicks | No rate limit, no budget accounting on a free write | A Supervisor that broadcasts each tick pins the fleet at 100% inference duty cycle | CONFIRMED |
| F-ACP-05 | `db/tasks.rs:353`; `FleetChat.tsx:26-34, 235`; `workerChatStore.ts:115` | M | ts | `created_at` is SQLite UTC `"Y-m-d H:M:S"`; JS `new Date()` parses it as local time | Every message timestamp skewed by the viewer's UTC offset | At UTC+5:30 every fresh message shows "5h ago"; the Supervisor's stale-thread heuristic is unusable | CONFIRMED |
| F-ACP-06 | `db/tasks.rs:603-612` vs `:355` | M | schema | `clear_all` drains `worker_requests`+`tasks` but not `worker_messages`, which also FKs `tasks(id)` | With `foreign_keys=ON` the transaction should abort | "Clear Board" fails after any task-threaded chat; board becomes un-clearable | CONFIRMED (code) / SUSPECTED (runtime) |
| F-ACP-07 | `db/tasks.rs:743-748` | L | contract | `unread_only=false` returns the agent's entire inbox history, unbounded | No LIMIT on a context-bound read | A long-lived worker re-reading its inbox blows its context | CONFIRMED |
| F-ACP-08 | `list_worker_messages.rs:46-55`; `tool_executor.rs:101-116` | L | contract | Inbox read is `is_read_only()==false` | Subject to permission prompts in non-autonomous modes | An inbox read blocks on a human approval dialog | CONFIRMED |
| **F-ROU-01** | `commands/task_commands.rs:51-63` vs `update_task_status_tool.rs` (no call) | C | inert | Routing outcomes recorded only on the UI path | Agent path never writes `outcome`, so the `>=3` gate never opens | A month of fleet runs; affinity panel shows 0/0/0%; `suggest_worker` always `method="default"` | CONFIRMED |
| F-ROU-02 | `scorer.rs:65-101, 140`; `db/workers.rs:209, 177-184` | H | bug | All-tied scores + stable sort + `is_system_defined DESC` ordering | `scores.remove(0)` returns `supervisor`; non-implementers are never excluded | Fresh install: `suggest_worker` recommends assigning code work to the Supervisor; `assign_task` then rejects the slug | CONFIRMED |
| F-ROU-03 | `scorer.rs:121` vs `patterns.rs:15, 21` | H | inert | Static branch needs `conf > 0.85`; max declared confidence is `0.85` | Off-by-epsilon makes the whole keyword layer dead code | "Add FastAPI endpoint with Pydantic schema" scores 4 backend hits and still routes `default` | CONFIRMED |
| F-ROU-04 | `db/tasks.rs:895-902, 990` | H | bug | Claim overwrites `assigned_worker` without checking it; outcome matches on `task_id` alone | Stolen tasks are credited/blamed to the original assignee | Post-F-ROU-01 fix, the scorer trains on mis-attributed labels and routes work away from the competent worker | CONFIRMED |
| F-ROU-05 | `routing/mod.rs:42, 91-93`; `db/tasks.rs:996-1001` | M | bug | Learning window is the 200 most recent rows globally | A busy board evicts a worker's history for a type; `record_outcome_auto` can fail to find its own row | Affinity silently resets mid-run; some outcomes never attach | CONFIRMED |
| F-ROU-06 | `db/tasks.rs:429-434, 990, 1001` | M | schema | No index on `routing_history(task_id)` or `(assigned_at)` | Both hot paths (outcome update, ordered read) are full scans | Routing calls slow linearly with history size | CONFIRMED |
| F-ROU-07 | `db/tasks.rs:419`; `routing/mod.rs:55` | LM | schema | `task_id TEXT`, no FK to `tasks(id)` | `clear_all` leaves orphaned routing rows feeding the scorer | Board wiped; affinity still shaped by deleted tasks | CONFIRMED |
| F-ROU-08 | `routing/mod.rs:59` vs `db/tasks.rs:987` | L | ts | `assigned_at` RFC3339, `outcome_at` SQLite `datetime('now')` in one table | Columns not comparable; no mixed `datetime()` predicate possible | Any future "time to outcome" query is wrong | CONFIRMED |
| F-ROU-09 | `db/tasks.rs:1022-1045` | L | inert | `get_worker_success_rates` has zero callers | Dead code | — | CONFIRMED |
| F-ROU-10 | `scorer.rs:61-63, 74-80` | L | bug | Single qualifying worker → `dur_range` = 1.0 → `duration_score` = 1.0 | Lone worker gets full speed credit regardless of speed | Early in a run, one worker's score is inflated by 0.4 | CONFIRMED |
| **F-EPI-01** | `mod.rs:96-164` vs `:222-223`; `mcp_server/server.rs:140-141`; `defaults.rs:220, 288-294` | C | gap | Epic tools registered for PA only; Supervisor is Mode2Only and reaches tools via MCP | The tools' intended caller cannot call them; the recipe decomposes with flat `create_task` | The whole phase/milestone/blocker model is exercised by nothing; the Epics tab is permanently empty | CONFIRMED |
| **F-EPI-02** | `db/tasks.rs:1202`; `assign_task.rs:96-99`; `assign_tasks_batch.rs:160` | C | bug | Epics are inserted `status='ready'`; no dispatch path filters `task_type='epic'` | An epic is indistinguishable from work | Epic #10 is batch-assigned to worker-backend, claimed, and "implemented" as one task | CONFIRMED |
| **F-EPI-03** | `commands/task_commands.rs:69-98` vs `update_task_status_tool.rs` (no call) | C | inert | `unblock_ready_tasks` + phase-complete emission run only on the UI path | Autonomous completion never unblocks dependents or completes epics | Phase 1 finishes; every phase-2 task stays `needs_info`; Supervisor reports "Board clean — idle." | CONFIRMED |
| F-EPI-04 | `db/tasks.rs:1284-1288` | H | bug | Phase buckets ignore `failed`/`build_failed`/`rejected`/`qc_review` | A failed task counts in `total` but no bucket; `is_complete` can never be true | Phase frozen at 4/5 with 0 ready and 0 blocked — no signal why | CONFIRMED |
| F-EPI-05 | `create_phased_task_tool.rs:141-162`; `db/tasks.rs:1343-1363` | H | bug | Blockers validated for existence only — no cycle, epic-scope or phase-order check | Mutual blockers are permanently unresolvable | LLM decomposition emits A↔B; both sit `needs_info` forever with no recovery path | CONFIRMED |
| F-EPI-06 | `db/tasks.rs:1219`; `agent/engine.rs:528`; `defaults.rs:235, 331-340` | M | contract | `needs_info` means blocked, budget-halted, or question-pending | Ambiguous status; Supervisor's stop condition ignores it entirely | Board declared idle while three different kinds of stuck work sit in `needs_info` | CONFIRMED |
| F-EPI-07 | `create_epic_tool.rs:53`; `create_phased_task_tool.rs:175` | M | bug | `created_by = ctx.session_id` (a UUID) | Column's other writers use role slugs (`'pa'`, `'supervisor'`) | Epic provenance unqueryable; UI renders a UUID at `AutoDevPanel.tsx:383` | CONFIRMED |
| F-EPI-08 | `db/tasks.rs:396, 1212-1217, 1358-1361` | M | schema | `epic_id`/`blocked_by` are stringified ids with no FK | Deleted tasks leave dangling blockers that evaluate to "not done" forever | Deleting one blocker task strands every dependent permanently | CONFIRMED |
| F-EPI-09 | `commands/task_commands.rs:205-231`; `lib.rs:59` | L | inert | Tauri `create_phased_task` has no frontend caller | Registered surface nobody uses | — | CONFIRMED |
| F-QC-01 | `defaults.rs:1298, 1311, 1826`; `spawn_qc_review.rs:139`; `db/tasks.rs` (no `repo_path`) | H | inert | QC Phase 0 is gated on `repo_path` "in task metadata"; no such column and the dispatcher never passes it | Diff risk analysis never runs | `risk_level='critical'` never blocks a review; `tasks.diff_analysis` never populated; risk card never renders | CONFIRMED |
| F-QC-02 | `update_task_status_tool.rs:131-162`; `write_file.rs:184-197`; `validate.rs:660-661` | H | contract | INTERFACE.md is required by prose only; consumer check is a lint on a success result and self-disables when the spec is absent | The one case the checker can't see is the one that matters | The `/api/api/events` double-prefix class the recipe cites reproduces | CONFIRMED |
| F-QC-03 | `e2e_probe.rs:179-197, 224-226` | H | leak | Kills the spawned shell (`cmd /C`), not the process tree; no port re-check | Windows dev servers survive teardown | Leaked `node.exe` holds :3000; next probe FAILs; QC rejects a correct task | CONFIRMED |
| F-QC-04 | `defaults.rs:1876`; `db/workers.rs:175-190` | M | inert | `fleet-smoke` has no schedule, no worker binding, no kick site | The fleet-level smoke gate never runs | Doc 09 T4.1 assembly check is documented but never executed | CONFIRMED |
| F-QC-05 | `annotate_file_tool.rs:91`; `db/annotations.rs` (no by-task query) | M | gap | `task_id` hardcoded `None`; no `list_by_task` | QC findings cannot be joined to the task they rejected | Reviewer pins 6 blocking notes; the task card shows none of them | CONFIRMED |
| F-QC-06 | `write_interface_spec.rs:146-152`; `validate.rs:672` | M | contract | Merge appends without dedup; check is substring containment | Deleted/superseded routes stay "declared" and satisfy the check | Backend removes `/api/events`; frontend keeps calling it; contract check passes | CONFIRMED |
| F-QC-07 | `analyze_diff_tool.rs:78-87, 214-240` | M | cost | Own provider from vault + env-var model; full patch, no size cap, no budget/cost attribution | Unmetered LLM spend inside QC | A 12k-line diff is sent whole to Sonnet on every review with no cap | CONFIRMED (masked by F-QC-01) |
| F-QC-08 | `analyze_diff_tool.rs:190-199` | LM | bug | Any `.ts`/`.tsx` path containing `/api/` or `/hooks/` counts as a frontend consumer | Backend route modules misclassified | A backend-only diff reports `interface_drift.status="both"` | CONFIRMED |
| F-QC-09 | `write_interface_spec.rs:98, 118` | L | contract | `requires_permission()==false`, default mode `overwrite` | One mis-issued call destroys the whole contract file | Worker calls without `mode='merge'`; every peer route is erased | CONFIRMED |
| F-QC-10 | `e2e_probe.rs:20-22 vs 182, 191` | L | contract | Header claims path-jail composability; only `current_dir` is set | Docstring promises a guarantee the code does not provide | A probe command can `cd` outside the workspace | CONFIRMED |
| **F-BUD-01** | `agent/engine.rs:62, 73-77, 148-166, 216-230, 480-536` | C | inert | `with_task_budget` has zero callers → `task_budget`/`task_id` always `None` | Circuit breaker, usage persistence, escalation and the `agent:budget-exceeded` event are all unreachable | A looping worker is capped by nothing; `BudgetBar` renders 0/0/0; no per-task cost attribution ever exists | CONFIRMED |
| F-BUD-02 | `commands/budget_commands.rs:55-79`; `runtime_config.rs:50-69` | H | gap | Settings-set default worker budget has no Rust reader | Doc-comment claims the recipe runner reads it; it does not | User sets a $2/task ceiling; it persists, reloads, and does nothing | CONFIRMED |
| F-BUD-03 | `set_task_budget_tool.rs:79, 83-85, 92-102` | H | bug | Reports "effective" defaults it never persists; writes raw `Option`s (NULLs) | Success message lies; also nulls the `budget_max_turns` column default of 20 | Supervisor is told task #7 has a 100k/$0.50/20 budget; the row has three NULLs | CONFIRMED |
| F-BUD-04 | `send_worker_message.rs:204`; `analyze_diff_tool.rs:87`; `spawn_qc_review.rs:222` | H | cost | Messages, diff analysis and QC reviewers spend without touching the task budget | Three unmetered spend paths beside the (inert) budget | A broadcast triggers 7 LLM ticks charged to nothing | CONFIRMED |
| F-BUD-05 | `db/tasks.rs:1104-1111, 1219, 1287` | M | contract | Budget halt forces `needs_info`, colliding with "blocked" | A budget-halted task has no `blocked_by`, so it disappears from every phase bucket | Phase progress silently under-reports a halted task | CONFIRMED (latent) |
| F-BUD-06 | `agent/engine.rs:218-219` | M | bug | `tokens_used` is the whole engine session, not per task | A session serving two tasks charges the first's spend to the second | Second task trips a budget it never spent | CONFIRMED (latent) |
| F-BUD-07 | `budget/mod.rs:51-70` | L | inert | Three `TaskBudget` presets have no callers | Dead code | — | CONFIRMED |
| F-BUD-08 | `agent/engine.rs:149-150`; `budget/mod.rs:81` | L | bug | `turns_used` incremented before the `>=` check | `max_turns: 1` halts before the first LLM call | Off-by-one against the natural reading of "max turns" | CONFIRMED |
| F-TS-01 | `orphan/mod.rs:183` vs `db/tasks.rs:313, 324, 899, 945` | H | ts | Parses `tasks.updated_at` (SQLite `datetime('now')`) with `parse_from_rfc3339(...).ok()?` | Parse always fails; `?` returns `None` = "not an orphan", so the TTL fallback never fires | A task claimed on another host, or assigned via the UI, is stranded `in_progress` forever — and its worker is permanently blocked by the single-in-progress guard (`db/tasks.rs:877-890`) | CONFIRMED |
| F-TS-02 | `tabs/AutoDevPanel.tsx:41, 51, 383` | M | ts | `new Date(dateStr)` on `tasks.created_at`/`updated_at` (SQLite UTC, no zone marker) | Parsed as local time in V8 | Task ages and "created" timestamps skewed by the viewer's UTC offset — same class as F-ACP-05 | CONFIRMED |

**Counts — 54 findings:** 7 critical · 15 high · 16 medium · 3 low-medium · 13 low.
All 54 are CONFIRMED by code inspection; F-ACP-06 is confirmed as code and suspected at runtime;
F-ACP-04's storm *occurrence* (as opposed to its mechanism) is unobserved.

---

## 9. Schema appendix

All tables live in `~/.autodev/tasks.db` (`db/tasks.rs:1-4`), created in `TasksDb::migrate`
(`db/tasks.rs:294-437`) under `PRAGMA journal_mode = WAL` and `PRAGMA foreign_keys = ON`
(`:297-298`).

### 9.1 `worker_requests` — `db/tasks.rs:327-341`

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK AUTOINCREMENT | |
| `worker_slug` | TEXT NOT NULL | from task.assigned_worker -> `agent_role` -> `"unknown"` |
| `task_id` | INTEGER | **FK -> tasks(id)**, enforced |
| `request_type` | TEXT NOT NULL | 5 values, validated in the tool only |
| `payload` | TEXT NOT NULL DEFAULT `'{}'` | opaque JSON, never validated (F-WRQ-06) |
| `status` | TEXT NOT NULL DEFAULT `'pending'` | pending/acknowledged/completed/rejected |
| `resolution_note` | TEXT | |
| `created_at` | TEXT NOT NULL DEFAULT `(datetime('now'))` | **SQLite UTC format** |
| `resolved_at` | TEXT | set only for completed/rejected (`:674-678`) |

Indexes: `status`, `worker_slug`, `task_id` (`:339-341`) — the hot query
(`WHERE status = ? ORDER BY created_at DESC`) is index-covered on the filter, sorts unindexed.
Migration: created in the initial batch; no ALTERs. **Missing:** no LIMIT/reaper (F-WRQ-01/02).

### 9.2 `worker_messages` — `db/tasks.rs:345-362`

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK AUTOINCREMENT | |
| `from_slug` / `to_slug` | TEXT NOT NULL | both from `ctx.agent_role` / the `to` argument |
| `body` | TEXT NOT NULL | `<= 4096` bytes, tool-enforced (`send_worker_message.rs:105`) |
| `kind` | TEXT NOT NULL DEFAULT `'nudge'` | nudge/question/answer/fyi/broadcast |
| `task_id` | INTEGER | **FK -> tasks(id)** — not drained by `clear_all` (F-ACP-06) |
| `status` | TEXT NOT NULL DEFAULT `'pending'` | pending/read/acknowledged |
| `created_at` | TEXT NOT NULL DEFAULT `(datetime('now'))` | **SQLite UTC** — mis-parsed by the UI (F-ACP-05) |
| `read_at` | TEXT | stamped by the list side effect (`:766`) |

Indexes: `(to_slug, status)`, `(task_id)`, `(created_at)` (`:357-362`) — correctly covers the inbox
query, the per-task thread and the recent feed. Migration: initial batch only.

### 9.3 `routing_history` — `db/tasks.rs:417-434`

| Column | Type | Notes |
|---|---|---|
| `id` | TEXT PK | UUID v4 (`routing/mod.rs:54`) |
| `task_id` | TEXT NOT NULL | stringified `tasks.id`, **no FK** (F-ROU-07) |
| `task_title`, `task_type`, `worker_slug` | TEXT NOT NULL | snapshot at assign time |
| `assigned_at` | TEXT NOT NULL | **RFC3339** (`routing/mod.rs:59`) — differs from every other table |
| `outcome` | TEXT | success/failure/reassigned; **NULL forever on the agent path** (F-ROU-01) |
| `outcome_at` | TEXT | **SQLite `datetime('now')`** (`:987`) — format clash with `assigned_at` (F-ROU-08) |
| `duration_ms`, `tokens_used` | INTEGER | `tokens_used` is never written non-zero (`routing/mod.rs:98`) |

Indexes: `worker_slug`, `task_type`, `outcome` (`:429-434`). **Missing:** `task_id` (the update
predicate) and `assigned_at` (the sort key) — F-ROU-06. Migration: created in a second
`execute_batch` (`:415-436`), so it post-dates the original schema but is not version-gated.

### 9.4 Epic / phase columns on `tasks` — `db/tasks.rs:392-413`

Added by idempotent `ALTER TABLE ... ADD COLUMN` calls whose errors are discarded with `let _ =`
(the project's migration idiom): `phase TEXT`, `phase_order INTEGER`, `milestone TEXT`,
`epic_id TEXT`, `blocked_by TEXT` (JSON array of stringified ids). Indexes added the same way:
`idx_tasks_epic_id (epic_id)` and `idx_tasks_phase (phase, phase_order)` (`:408-413`).
`task_type='epic'` is the epic marker (`:118`, insert at `:1202`). No FKs, no cascade (F-EPI-08).

### 9.5 Budget columns on `tasks` — `db/tasks.rs:384-391`

`budget_max_tokens INTEGER`, `budget_max_usd REAL`, `budget_max_turns INTEGER DEFAULT 20`,
`budget_tokens_used INTEGER DEFAULT 0`, `budget_cost_usd REAL DEFAULT 0.0`,
`budget_turns_used INTEGER DEFAULT 0`, `budget_exceeded INTEGER DEFAULT 0`,
`budget_exceeded_reason TEXT`. Every `*_used` column is read with `COALESCE(...,0)`
(e.g. `:924-928`) and **never written** (F-BUD-01). Related QC columns from the same idiom:
`under_review_by TEXT` (+ `idx_tasks_under_review_by`, `:402-406`) and
`task_qc_cost_usd REAL DEFAULT 0.0` — the latter is the only populated cost column
(`spawn_qc_review.rs:211`).

### 9.6 Timestamp-format audit for this scope

| Value | Written as | Read as | Verdict |
|---|---|---|---|
| `worker_requests.created_at` / `resolved_at` | SQLite `datetime('now')` | passed through as String to JSON/UI; no Rust parse | OK in Rust; UI does not render it |
| `worker_messages.created_at` | SQLite `datetime('now')` | `datetime(created_at) <= datetime('now', ?)` in SQL (`db/tasks.rs:804`) | **OK** — same-format SQL comparison |
| `worker_messages.created_at` | SQLite `datetime('now')` | JS `new Date()` (`FleetChat.tsx:26, 235`) | **DEFECT — F-ACP-05** |
| `routing_history.assigned_at` | Rust RFC3339 (`routing/mod.rs:59`) | `parse_from_rfc3339` (`routing/mod.rs:94`) | **OK** — matched pair |
| `routing_history.outcome_at` | SQLite `datetime('now')` (`:987`) | never parsed | latent inconsistency — F-ROU-08 |
| `tasks.updated_at` | SQLite `datetime('now')` | `parse_from_rfc3339(...).ok()?` (`orphan/mod.rs:183`) | **DEFECT — F-TS-01** (the sibling audits' class, confirmed again) |
| `tasks.created_at` / `updated_at` | SQLite `datetime('now')` | JS `new Date()` (`AutoDevPanel.tsx:41, 51, 383`) | **DEFECT — F-TS-02** |
| `tasks.created_at` | SQLite `datetime('now')` | `date(created_at) = date('now')` (`db/tasks.rs:1155`) | **OK** — same-format SQL |

**Rule the restructure should adopt:** one timestamp format for the whole DB. Given that every
`DEFAULT` and trigger already uses `datetime('now')` (`db/tasks.rs:312-313, 324`), the cheapest
correct choice is SQLite-UTC everywhere, with (a) `routing/mod.rs:59` switched to match, (b) every
Rust reader using `NaiveDateTime::parse_from_str(..., "%Y-%m-%d %H:%M:%S")` and treating it as UTC,
and (c) every JSON boundary serialising as RFC3339 so the frontend's `new Date()` is correct.

---

## 10. Sidecar-survival matrix

| # | Capability | Survives sidecar deletion? | The coupling, cited | Port work named |
|---|---|---|---|---|
| 1 | **Supervisor escalation queue** | **YES** | Storage/tools/commands/UI are all Rust+SQLite+Tauri. Only soft link: identity fallback via `ctx.agent_role` (`request_supervisor_action.rs:102`), populated at `mcp_server/tool_adapter.rs:280`, `None` in-house (`tool_executor.rs:89`) — bounded because `task.assigned_worker` covers most calls | Thread `agent_role` into `ToolContext` (shared with #2). Add a pending-request reaper and stop Step 1a starving Step 2 (F-WRQ-01) |
| 2 | **Worker↔worker messaging (ACP)** | **PARTIALLY — identity does not survive** | All three tools hard-refuse without `ctx.agent_role` (`send_worker_message.rs:129-137`, `list_worker_messages.rs:60-66`, `acknowledge_worker_message.rs:48-55`); it is set only at the MCP boundary (`tool_adapter.rs:191, 280`). Delivery latency depends on `scheduler::kick` -> `runner.run_recipe`, whose Mode 2 branch is the orchestrator (`runner.rs:192-196`) | (a) Carry the recipe's `agent_role` (`recipes/types.rs:52`) into the executor and all 3 `ToolContext` sites; (b) keep refuse-on-missing-identity; (c) re-point `kick` at the in-house tick; (d) fix read-loss (F-ACP-03) and the two dead-letter slugs (F-ACP-02) |
| 3 | **Dynamic routing / affinity** | **YES** | `routing/` is pure arithmetic over `tasks.db`; `suggest_worker_tool.rs:40` does not read `agent_role`; no orchestrator call in `routing/mod.rs`, `scorer.rs`, `patterns.rs` | None for the port itself. Use the restructure to move outcome recording onto the shared status path (F-ROU-01), exclude non-implementers (F-ROU-02), fix the `> 0.85` threshold (F-ROU-03), and add `assigned_worker` to the claim predicate (F-ROU-04) |
| 4 | **Epic / phase decomposition** | **YES** | No sidecar coupling anywhere. The blocker is the MCP-vs-PA registry split (`mod.rs:96-164` vs `:174-235`, rationalised at `mcp_server/server.rs:140-141`) — an artefact of the sidecar architecture that disappears with it | Expose `create_epic`/`create_phased_task` to the Supervisor role; move `unblock_ready_tasks` + phase-complete emission onto the shared status path (F-EPI-03); add `task_type != 'epic'` to dispatch (F-EPI-02) |
| 5 | **Interface contracts + QC support** | **MIXED** — `write_interface_spec` **YES** (`std::fs` only); `tier2_6_api_contract` **YES** (regex+fs); `analyze_diff` **YES** (own provider from vault, `analyze_diff_tool.rs:214-240`); `e2e_probe` **YES** (`tokio::process`+`reqwest`); `annotate_file` **PARTIAL** (`source` dies with `agent_role`); **`spawn_qc_review` NO** | `spawn_qc_review.rs:171` spawns via `SubagentManager`, constructed from `OrchestratorClient` (`subagent/manager.rs:31, 55, 61`); reviewers are configured `tools: []  // MCP-only`, `mcp_servers: []  // default = autodev-mcp-server` (`spawn_qc_review.rs:157, 162`) | Replace `SubagentManager::spawn`/`await_result` in-house while preserving the four invariants: atomic `under_review_by` claim (`:130`), cost attribution to `task_qc_cost_usd` (`:209-218`), the hour-bucketed `qc_cost_guard` (`:222`), and claim release on every terminal path (`:225-227, 255-257`). Also supply `repo_path` so Phase 0 can run (F-QC-01) and fix the Windows process-tree kill (F-QC-03) |
| 6 | **Worker budget / cost controls** | **YES — and deletion is what makes it possible** | `budget/mod.rs` is pure arithmetic; columns are SQLite; UI is wired. Enforcement lives in `AgentEngine` (`engine.rs:148-166`), which the Mode2Only fleet never runs (`defaults.rs:220, 382, 634, 849, 958, 1286, 1804, 1881`; `runner.rs:129-130, 192-196`) | Call `with_task_budget` from the tick dispatcher (F-BUD-01), resolving task columns -> `runtime_config.default_worker_budget_*` (F-BUD-02) -> `TaskBudget::default_worker()`; persist the defaults `set_task_budget` claims (F-BUD-03); debit messages / diff analysis / QC reviewers from the same account (F-BUD-04) |

### Cross-cutting port item (affects #1, #2, #5)

`ToolContext.agent_role` is the single shared dependency on the deleted boundary. It is set once, at
`mcp_server/tool_adapter.rs:280`, and hardcoded `None` at `agent/tool_executor.rs:89`, `:252`,
`:316`. Beyond the ACP, it gates `annotate_file`'s `source` (`annotate_file_tool.rs:77-81`), the
`qc_review` transition branch (`update_task_status_tool.rs:131-137`), and the escalation identity
fallback (`request_supervisor_action.rs:102`). **Fix this first** — five of the six capabilities
touch it, and every one of them fails silently rather than loudly.

### Second cross-cutting item (affects #3, #4)

Two capabilities are inert for the same structural reason: post-write side effects live in the
**Tauri command** `commands/task_commands.rs:45-98` (routing outcome, blocker unblocking, phase
completion) and not in the **agent tool** `update_task_status_tool.rs`. The restructure should
collapse both onto one shared `transition_task_status()` function so the agent path and the UI path
can never diverge again.
