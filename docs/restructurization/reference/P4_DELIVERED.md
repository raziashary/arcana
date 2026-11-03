# What P4 delivered (kept)

A map of the 51 checked P4 tasks plus related commits, grouped by what they give the product, so the clean-slate
reset in [`../SHIP_PLAN.md`](../SHIP_PLAN.md) does not hide them. Source: the archived plan
([`../archive/development/MASTER_DEVELOPMENT_PLAN.md`](../archive/development/MASTER_DEVELOPMENT_PLAN.md) §10), the ADR log and
`git log`. **Evidence class:** read from those records; the terminal-control code files were confirmed present on
2026-11-03, the rest were not individually re-checked and their tests were not re-run. Nothing here was deleted, archived
as code, or parked. Open P4 items are in the plan and backlog, not here.

## 1. Terminal and command control (an agent cannot run loose)
- `P4-11` Rust-owned command construction: ecosystem detection and argv through `project_command`; a closed intent set, not model-written shell.
- `P4-16` POSIX-only shell purged from shipped and persisted recipes, skills and hooks, with a validator on parse, save, import and hook execution.
- `P4-17` One workspace resolver that fails loudly; three divergent fallbacks deleted.
- `P4-18` (partial) agent writes confined to the workspace; `bash` and `project_command` both launch through the fail-closed OS process sandbox; Windows AppContainer proof for workspace write, sibling-read denial, nested children, `.cmd` shims and junction escapes.
- `P4-19`, `P4-20` a failed task checkpoint blocks every agent write; per-task reversibility without reverting another task's writes.
- `P4-60` persisted, revocable egress consent bound to workspace and destination; hook actions go through the same consent (ADR-0400, commit `3a4cbb1`).
- `P4-62` bounded long-running verification (one hour default, two hours cap).
- `P4-61` (partial) untrusted-code sandbox for Windows. Unfinished half (W9): see SHIP_PLAN R1.

## 2. Task ownership and concurrency (no task theft)
- `P4-07` atomic claim that includes the assigned worker, so one worker cannot take another's task; exactly one winner among concurrent claims.
- `P4-08`, `P4-38` the one-task-per-worker rule enforced inside the claim statement and in every batch entry.
- `P4-09` an epic can never be assigned or claimed as work, even through direct SQL.
- `P4-10` unblocking dependents and epic progress in one transaction, with crash injection at each boundary.
- `P4-34` the shipped worker path claims through the owner-checked claim and records the real PID and host.
- `P4-40` claim PID and host set only on a successful claim and cleared on leaving a claim-bearing state; compare-and-set for force release.

## 3. Dispatch and scheduling reliability
- `P4-04` dispatch from task-assigned events; the cron is only the five-minute stranded-work reconciler.
- `P4-05` a transient database error cannot terminate a schedule.
- `P4-06`, `P4-37` the in-flight slot is released on normal exit, panic, cancel and timeout (RAII guard); a worker slot leak after a successful run was fixed (`a2e7aa8`).
- `P4-24` an escalation cannot starve dispatch.
- `P4-28`, `P4-39` dependents unblock and dispatch automatically when prerequisites finish; budget-held tasks wait for an explicit resolution.
- `P4-35` an inbox-only kick reaches the reader even with no runnable task.
- `P4-43` idle ticks make no provider calls (measured).

## 4. Build evidence and the QC gate
- `P4-12` build evidence is a table that survives restart, not a 15-minute in-memory map.
- `P4-13` evidence only from the actual spawned build and its observed exit code, never misattributed.
- `P4-14` every transition into `qc_review` needs fresh, task-scoped, complete evidence, including through direct SQL.
- `P4-15` after three failed builds a worker diagnoses and stops; only an audited, single-use Supervisor grant allows more.

## 5. Routing and worker learning
- `P4-25` routing outcomes recorded from the tool, so the learning gate can open.
- `P4-26` `suggest_worker` never returns a system agent for implementation work.
- `P4-32`, `P4-33` the deterministic scorer is authoritative; the unreachable 0.85 static-pattern threshold fixed.
- `P4-30` startup no longer overwrites a user's edited recipe.
- `P4-44` procedural memory wired for workers, off by default until its equivalence gate is green.

## 6. Worker state and data integrity
- `P4-01`..`P4-03` typed `WorkerStatus` end to end; unknown values fail loudly; a database constraint and a migration, proven on legacy copies, never the user's database.
- `P4-57` one canonical UTC timestamp contract across task, run, analytics, event, memory and config writers and readers (Windows).
- `P4-58` the production query inventory is executable: every read has a registered fingerprint (`p4_query_registry` test, passing 9 of 9).

## 7. Worker messaging and the Supervisor
- `P4-22`, `P4-23`, `P4-42` worker messages through the Rust-native ACP path; durable at-least-once delivery with replay safe by message ID; the QC agent reads and acknowledges its inbox.
- `P4-27`, `P4-41` a real Supervisor turn creates an epic, decomposes it into tasks and leaves a visible board.

## 8. Recovery and resilience
- `P4-36` an interrupted in-process run is recovered: bounded heartbeat state, exact-owner claim release, Supervisor woken to reconcile uncertain effects instead of replaying them.
- `P4-52` bounded exponential startup retries with full jitter, observable attempt, delay and outcome; later commits add a persisted startup retry count (`3a2a26c`), host-verified recovery retries (`bdced1c`) and worker dispatch lineage for the Supervisor (`8356aea`).

## 9. Cost safety
- `P4-29` the saved per-task budget breaker is loaded and enforced alongside the worker-run cap.

## 10. Observability and retention
- `P4-55` log rotation: at most five files per stream, nothing older than seven days, no symlink following, visible cleanup failures.
- `P4-59` bounded retention for `token_usage`, `system_events`, `api_failures`, `worker_stats` and `hook_logs`.
- Commits: Logs tab with trace aggregation, producer cost and dropped-telemetry visibility, missing-log-sink made visible, telemetry labels registered. Depth beyond this is in the backlog.

## 11. IDE handoff
- `P4-21` a worker-created artifact opens in the IDE tab through the real event consumer and can be run (isolated desktop receipt, Windows).

## 12. Providers (commits outside the task list)
- Groq provider added (`7c497fc`); every provider built through one factory (`894bbe0`), which fixed a live defect where Groq could not run the Fleet; the retry class is carried as data, not parsed from message text (`ad56120`).

## 13. Hooks
- HTTP hooks consent-gated and hook log paths confined (`3a4cbb1`); a hook refusal says whether policy or the store decided it (`bc95c1b`).
