# A15 · Fleet audit — assignment, affinity, and the board capabilities

Read-only audit, 2026-09-03, against Razi's report that *"work posted by the
supervisor on the board did not get picked up by the worker for a long time,
and was not assigned using the affinity."* The auditor could not write files;
this records the actionable core.

⚠ **Verified in the main session:** that `assign_task` / `assign_tasks_batch`
never consult the scorer, and that `static_conf > 0.85` is unreachable against
a maximum pattern confidence of exactly `0.85`. The rest is agent-reported.

---

## 1 · Affinity is never consulted

`assign_task.rs` and `assign_tasks_batch.rs` take `worker_slug` **from the
model's tool arguments**, validate it against `FLEET_WORKERS`, and write. The
only thing they call in `routing` is `record_assignment` — they *write* history
and never *read* it. The only production callers of `suggest_worker` are the
agent-facing tool and a Tauri command.

So the decision input is the supervisor LLM's reading of a hard-coded keyword
table in its own prompt (`defaults.rs:289-298`), which explicitly ranks itself
above the scorer: *"Do NOT call `suggest_worker` on tasks the Matrix already
disambiguates"*.

**And when it is called, its answer is unusable.** Three defects compound:

- **The static layer is unreachable.** `select_worker` requires
  `static_conf > 0.85` (`scorer.rs:135`); the maximum any pattern declares is
  exactly `0.85` (`patterns.rs:27,46`). The advertised `method: "static"` can
  never be produced.
- **The historical layer never opens.** The `>= 3` gate counts only rows whose
  `outcome` is set, and `outcome` is written only by `record_outcome_auto`,
  whose sole caller is a Tauri command — i.e. a human clicking in the UI. In
  autonomous operation it is `NULL` for ever.
- **So every worker ties at 0.5 and the tie-break returns `supervisor`**, which
  `assign_task` then rejects as not a valid worker.

Note also `routing_commands.rs:9` claims it is *"used by the UI 'Suggest
Worker' button"*. There is no such button.

## 2 · Four ways a worker goes permanently quiet

In each case the roster still shows the worker as scheduled.

1. **Two statuses have no transition out of the cron loop.** The gate fires
   only for `idle | active | error`; `run_recipe` writes `fallback_idle`
   (`runner.rs:735`) and `mode1_idle_no_provider` (`:766`), which match neither
   that list nor the `break` list. Only `run_recipe` can return a worker to
   `idle`, and the loop will never call it again.
2. **A transient DB error kills the schedule permanently.** `...ok().flatten()`
   collapses any error to `None`, which shares an arm with `paused | deleted`
   → `break`. One `SQLITE_BUSY` ends that worker's loop.
3. **A worker stuck in `working` is never rescued.** Both the cron gate and
   `kick_worker` skip it; nothing sets a tick deadline, and `WorkersDb::heartbeat`
   has **zero callers in the app**, so `last_seen` is never refreshed.
4. **`kick_inflight` leaks on the async path.** The synchronous returns are
   RAII-guarded; the spawned task removes its slug on the last line. A panic
   inside `run_recipe` leaves the slot held and kills push dispatch for that
   worker for the process lifetime.

## 3 · The tick gate swallows what the kick was sent for

`runner.rs:206-215` short-circuits the whole tick when `has_runnable_task`
finds nothing in `Assigned | InProgress` — **before the engine is built**. So:

- **A `rejected` task never wakes its worker.** `update_task_status_tool`
  explicitly kicks the original worker on `rejected`, and the gate does not
  count `rejected`. (The dead Mode 2 branch *did* count it.)
- **An idle worker never reads its inbox.** A message kicks the recipient, and
  the tick is short-circuited before `list_worker_messages` is ever called.

## 4 · Board capabilities

| Capability | Verdict |
|---|---|
| Task board / dispatch | **Working**, with the stalls above |
| Affinity / routing | **Built-but-unreached** — never consulted; learning gate never opens; tie-break answer invalid |
| **Epics** | **Built-but-unreached.** Model, tools, Tauri commands and an Epics tab all exist. **No recipe mentions `create_epic` or `create_phased_task`**, the supervisor's tool list omits them, and no frontend caller exists. The panel is permanently empty because nothing creates the row |
| Epic unblocking / phase gating | **Built-but-unreached, and a dead end.** `unblock_ready_tasks` has exactly one caller — the UI. A phased task sits in `needs_info` for ever, and the supervisor's self-heal list has no branch for it |
| **Worker chat (ACP)** | **Partially working.** Agent→agent works only when the recipient is already busy (see §3). QC recipes never read their inbox, so broadcasts leave permanently unread rows. `list_stale_worker_messages` has zero callers, so delivery is at-most-once against a recipe promising "lossless". **Human→worker chat does not exist** — Fleet Chat has no input and every command in that module is read-only |
| Supervisor escalation | **Working** — the strongest of the set |
| **ROI scoring** | **Absent.** No model, writer, reader, UI or task. The only "ROI" in the tree is QC *cost* per task. Dispatch ordering is the `p0…p3` priority string in the prompt only |
| Per-task budget | **Built-but-unreached** — `with_task_budget` has zero production callers, so the breaker can never fire |
| Orphan recovery | **Degraded** — the timestamp bug is genuinely fixed, but recipes never call `claim_task`, so no PID is recorded and it falls back to the 4-hour TTL |

## 5 · Two comments that assert the opposite of the code

- `assign_tasks` "will refuse and report per-entry failures" for a busy worker
  (`defaults.rs:281-283`) — the code never looks at the target's load.
- `suggest_worker_for_task` is "used by the UI button" — no such button exists.

## 6 · The likely amplifier of the original complaint

`get_qc_queue` parses `updated_at` with an RFC3339 parser; the column is
written by `datetime('now')` with **no offset**, so the parse fails,
`.unwrap_or(0)` reports every review's age as 0s, and the stalled-review alert
can never fire. The frontend has the same class of bug in the other direction:
`new Date("2026-09-01 06:23:58")` is read as **local** time by V8, so at
UTC+5:30 every age is inflated by 5.5 hours. **A task assigned a minute ago can
read as "6 hours ago".**

⚠ This site was missed by `P1-13`'s sweep because it does not contain the
string `parse_from_rfc3339` — which is exactly what the sweep searched for.

---

## Proposed tasks

- **P4-32 · Dispatch consults affinity deterministically.** Rust picks the
  worker when the caller supplies no slug, or the supervisor must pass the
  scorer's answer plus an explicit override reason. Gate: with N recorded
  outcomes favouring worker X for a task type, a new task of that type is
  assigned to X **without** the model choosing. *(Blocked on P4-07 + P4-25.)*
- **P4-33 · Fix the static threshold and prove the blend is reachable.**
  `>=` versus `>`, or raise the pattern confidences; unit-prove that
  `method:"static"` can be produced at least once.
- **P4-34 · One claim path, and the recipes use it.** Either rewrite every
  worker recipe's Step 2 to call `claim_task`, or make
  `update_task_status(status='in_progress')` delegate to the guarded claim.
  Gate: driving the **shipped recipe text**, worker B cannot move worker A's
  task to `in_progress`, and every claim records a PID. *This is the task that
  makes P4-07 and P4-08 mean anything.*
- **P4-35 · The tick gate stops swallowing non-execution work.** Handle
  `rejected` and drain the inbox before the short-circuit.
- **P4-36 · Worker liveness watchdog.** Write `last_seen` on tick start/end; a
  worker `working` beyond a ceiling is reset with a system event.
- **P4-37 · `kick_inflight` released on panic** (RAII inside the spawned task).
- **P4-38 · Batch assignment honours worker load**, or the recipe stops
  claiming that it does.
- **P4-39 · `needs_info` is not a terminal state** — give the supervisor a
  branch, and run the epic-unblock path autonomously.
- **P4-40 · Clear `claimed_by_pid` / `claim_host` on every non-claim status
  write**, so orphan detection cannot act on a previous lifecycle's PID.
- **P4-41 · The remaining timestamp readers** — `get_qc_queue` through the
  helper, and the three frontend readers given an explicit UTC parse. The gate
  must be behavioural: grepping for `parse_from_rfc3339` is what hid this site.
- **P4-42 · Epics reachable from the fleet's real surface** — name
  `create_epic` / `create_phased_task` in the supervisor recipe's protocol
  *and* its tool list, with a decomposition rule and a UI entry point.
- **P4-43 · QC inbox, or drop QC as a recipient** — add
  `list_worker_messages` to the QC recipes and seed a `qc-dispatcher` worker
  row, or remove them from the broadcast fan-out.
- **P4-44 · Empty-board short-circuit for `supervisor` and `qc-agent`** — they
  currently invoke the LLM every 60s and every 30s against an empty board. The
  existing QC skip lives inside the dead Mode 2 branch.
- **P4-45 · Task ROI scoring** — ⚠ blocked on a definition from Razi: what the
  score is over, who writes it, and whether it orders dispatch ahead of `p0…p3`.

## Already covered by an existing P4 task

`P4-01`…`P4-10` cover the dead statuses, cron-as-dispatch, the DB-error break,
`kick_inflight`, the claim predicate, the fail-open load guard, epics as
assignable work and transactional reconciliation. `P4-25`/`P4-24a` cover
outcome recording, `P4-26` the system-agent suggestion, `P4-28`
`unblock_ready_tasks`, `P4-29` the per-task budget, `P4-30` recipe overwrites.

⚠ **`P4-27`'s stated reason is now stale** — it says `create_epic` is "PA-only
and reaches tools only via MCP". MCP is gone and the tool *is* registered on
the path the supervisor uses; the live gap is that no recipe mentions it.

⚠ **`P4-22` appears already satisfied in code** (`runner.rs:832` threads a real
worker identity) yet is still open and unverified.
