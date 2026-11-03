# Arcana ship plan — the one live plan

| | |
|---|---|
| **Owner** | Razi (sole developer) |
| **Written** | 2026-11-03, replacing the P0–P9 master plan (now in [`archive/`](archive/README.md)) |
| **Status** | Cuts confirmed by Razi 2026-11-03. One ruling still open: `P9-02`, Windows code signing (see R4). Restructuring work resumes only when Razi says go. |

## 1. What "shipped" means

**Arcana v1 is a Windows-only installer** (Razi's whole company runs Windows). On a clean Windows machine a user installs it,
enters a provider key, hands it a coding task, and receives a build-verified result,
without reading documentation. That is the old headline acceptance test (`P9-15`), cut to
one platform.

Product definition (unchanged from [`archive/development/ARCANA_CODING_SCOPE.md`](archive/development/ARCANA_CODING_SCOPE.md)):
a native coding harness. The model plans and writes code; Arcana owns deterministic
project discovery, command selection, task claims, permissions, execution boundaries,
evidence and recovery. Outcomes AC-01..AC-07 stand; AC-07 (installable app) is Windows-only for v1.

## 2. Rulings (Razi, 2026-11-03)

1. v1 is **Windows-only**. Linux/macOS parity is post-ship ([`HARDENING_BACKLOG.md`](HARDENING_BACKLOG.md)).
2. P4 closes on the **middle path**: finish Windows process-sandbox containment (`P4-18`, `P4-61`,
   gates `TG4-09`/`TG4-18`/`TG4-31`) because it is a safety boundary. Observability depth,
   mid-run continuity and worker-health depth (`P4-50/51/53/54/56`, `P4_EXPANDED_GOAL.md`) move to the backlog.
3. The old MDP `§0` process is **slimmed** to the rules in §3 below.
4. The Linux/macOS sandbox work is **parked** on the local branch `park/linux-macos-sandbox` (commit `bc54874`, not
   pushed); see the backlog. All Windows harness code (process sandbox, `project_command`, egress consent, IDE Run) stays on `main`.
5. Razi may have docs changed and pushed to `main` directly. This does not extend to code pushes (`AGENTS.md`).
6. The proposed cuts in §4 were reviewed and confirmed.

## 3. Working rules (replace MDP §0)

1. **Ground in the code.** Read the code and this plan before building. The shipped code is the
   anchor; if faithful work contradicts a doc, fix the doc.
2. **Every task names a proof that can fail:** a test, a smoke run or a measured result. "It compiles"
   and "the doc says so" are not proof. A checkbox is a claim; re-run the owning check before relying on it.
3. **Live data is irreplaceable until Razi says otherwise.** Back up before destructive or schema-changing
   operations, including ones reachable through tests. Run suites without live provider credentials;
   `.env` keys are for explicitly enabled live smokes only.
4. **Security-sensitive code gets an independent read-only review before it lands:** the vault and keys,
   the process sandbox, egress consent, commit safety. Everything else is reviewed once per phase (rule 5).
5. **One independent review at the end of each phase,** read-only, with a self-contained brief. Findings
   are fixed, or recorded in [`DECISIONS.md`](DECISIONS.md) with a reason for deferral.
6. **Decisions go in `DECISIONS.md` only when there was a real choice** between alternatives. No entry for
   routine implementation.
7. **No stale docs.** The change that makes a line here untrue fixes it in the same commit.
8. **Commits:** one logical unit, Conventional Commit subject plus a body, staged by explicit path
   (never `git add -A`; never `.env` or `target/`). Pushes follow `AGENTS.md`.
9. **A phase boundary is a checkpoint, not an exit.** Commit, update §5, continue. Stop for a real blocker
   or a decision that is Razi's.

Provider-layer and keys conventions stay in [`../../AGENTS.md`](../../AGENTS.md).

## 4. Roadmap

```
R0 baseline green → R1 close P4 (Windows) → R2 graph (P5) → R3 UI + experience (P6, P7) → R4 Windows delivery (P9)
```

A proof column names the check that closes each row. IDs are the old task IDs so existing code comments
and archived receipts stay traceable. `v1` means required. Rows without it were cut or deferred on 2026-11-03.

### Already built in P4 (kept; the roadmap below does not redo it)

P4 delivered far more than the open tasks suggest. This table is the short form; the task-by-task map is
[`reference/P4_DELIVERED.md`](reference/P4_DELIVERED.md). **Evidence class:** read from the archived plan, ADR log and
commits. The terminal-control files were confirmed present on 2026-11-03; the other rows were not individually
re-checked and their tests were not re-run (R0 does that). Nothing here was deleted, archived as code or parked.

| Area | What is kept | Tasks |
|---|---|---|
| Terminal and command control | Commands built by Arcana from a closed intent set (`project_command`), not model-written shell; `bash` routed through the same fail-closed Windows sandbox, prompting in Autonomous mode where a stored `AllowAlways` cannot bypass it; Windows AppContainer containment (write containment, sibling-read denial, nested children, `.cmd` shims, junction escapes); one workspace resolver that fails loudly; checkpoint-before-write blocking agent writes; persisted, revocable egress consent for diff, provider and hook traffic; POSIX-only shell purged from recipes, skills and hooks; env scrubbing and redaction; host-enforced commit safety; bounded long verification | `P4-11`, `P4-16`..`P4-20`, `P4-60`, `P4-62`, `P8-74`; `P4-18` and `P4-61` partial |
| Task ownership and concurrency | Atomic claim that includes the assigned worker (no task theft, one winner); one task per worker enforced in the claim and each batch entry; epics never claimable as work; unblocking dependents is one transaction; claim PID and host set and cleared with compare-and-set force release | `P4-07`..`P4-10`, `P4-34`, `P4-38`, `P4-40` |
| Dispatch reliability | Event-driven dispatch with a five-minute reconciler; a database error cannot kill the schedule; the in-flight slot is released on panic, cancel and timeout; escalation cannot starve dispatch; dependents auto-resume, budget holds wait; idle ticks make no provider calls | `P4-04`..`P4-06`, `P4-24`, `P4-28`, `P4-35`, `P4-37`, `P4-39`, `P4-43` |
| Build evidence and QC gate | Evidence is a table that survives restart, recorded from the actual spawn and exit code; no transition into `qc_review` without fresh task-scoped evidence, even by direct SQL; three failed builds stop the worker until a single-use Supervisor grant | `P4-12`..`P4-15` |
| Routing and learning | Deterministic scorer is authoritative (unreachable threshold fixed); system agents never suggested for implementation; outcomes recorded from the tool; startup no longer overwrites user recipe edits; procedural memory wired, off by default | `P4-25`, `P4-26`, `P4-30`, `P4-32`, `P4-33`, `P4-44` |
| Data integrity | Typed `WorkerStatus` end to end with a constrained column and a legacy-safe migration; one canonical UTC timestamp contract; every production query in a registered inventory (`p4_query_registry`, 9 of 9 pass) | `P4-01`..`P4-03`, `P4-57`, `P4-58` |
| Messaging and Supervisor | Durable at-least-once worker messages, replay-safe by message ID; the QC agent reads its inbox; a real Supervisor turn creates and decomposes an epic | `P4-22`, `P4-23`, `P4-27`, `P4-41`, `P4-42` |
| Recovery and resilience | An interrupted run is recovered without replaying uncertain effects; startup retries with capped backoff and full jitter; persisted retry count, host-verified recovery retries and worker dispatch lineage for the Supervisor | `P4-36`, `P4-52`; commits `bdced1c`, `3a2a26c`, `8356aea` |
| Cost safety | The saved per-task budget breaker is enforced beside the worker-run cap | `P4-29` |
| Observability and retention | Log rotation (five files per stream, seven days); bounded retention for the append-only tables; Logs tab with trace aggregation, producer cost and dropped-telemetry visibility | `P4-55`, `P4-59`; `P4-50` partial |
| IDE handoff | A worker-made file opens in the IDE tab through the real event consumer and runs | `P4-21` |
| Providers and hooks | Groq provider; every provider built through one factory (fixed Groq failing in the Fleet); retry class carried as data; HTTP hooks consent-gated; hook refusals say whether policy or the store decided | commits `7c497fc`, `894bbe0`, `ad56120`, `3a4cbb1`, `bc95c1b` |

### R0 · Baseline green

| Item | Proof |
|---|---|
| Fix `cargo fmt` drift in `arcana-core/src/recipes/{runner,scheduler}.rs` — **done 2026-11-03** | `cargo fmt --all -- --check` clean |
| Fix four red `ci_gate` tests — **done 2026-11-03.** All four were stale static scans, not holes. `tg_p1_05`, `tg_p3_03` and `tg_p3_14` cut each file at the first `#[cfg(test)]`, which misses test-only modules declared `#[cfg(test)] mod x;` in their parent and `#[cfg(all(test, …))]` modules; they now use one structural `production_source` helper (no file excluded by name; each test-gated item, field or variant is blanked rather than the file cut, which also brings shipped code after a mid-file test item, in 25 of 351 files, under the scan for the first time) with both-direction self-tests. The `e2e_probe` gate demanded DNS resolution and a runner that no longer exist; it now pins the denial contract (loopback checked without DNS before the denial, no success path, none of a list of spawn and fetch markers in shipped code, `ToolEffect::Execute`). The marker list is a text check, not a proof that nothing can launch a process; it is weaker than the old runner-specific assertions, which no longer apply and switches the original rules back on automatically if a runner appears, with a self-test that catches ten regressions | `cargo test -p arcana --lib ci_gate` all green |
| Fix the intermittent `src-cli/tests/fleet_offline.rs` failure — **partly done.** It was two flakes. The dispatcher assertion read the newest supervisor tick, but a later tick can exist; it now reads the dispatcher through the worker's recorded parent. The second, a missing `fleet_cli_run_completed` event in the export, is **unreproduced in 40 runs**. A plausible cause is that `tracing-appender`'s guard waits at most one second for the writer on drop, but that is unproven, so no production change was made. Also: the test is not in CI | Parallel run passes repeatedly (51/52 so far); enable the feature in CI or record why not |
| Re-run `cargo clippy --workspace --all-targets`, `cargo test --workspace --all-targets --locked`, `npm test` and record the true failure set — **done**, results in §5. Fixes made: the missing telemetry labels `database_run_children_read` and `database_worker_startup_retry_update` (unregistered labels are treated as untrusted); the clippy compile error in `psec.rs`; a missing home guard in the hook-retention test, which could write a migration backup into the real `~/.arcana/backups` and raced other tests | Results written into §5 |
| Confirm the old closure prerequisites `TG3-26` and `TG4-16` — **done**: the named tests pass; the register's own open clauses are listed in §5 and stay open | Test or smoke |

### R1 · Close P4 (Windows)

| Task | Scope | Proof |
|---|---|---|
| `P4-18`, `P4-61` v1 | Agent writes stay in the selected workspace; untrusted build/test children run under the OS sandbox with no network and no reads outside the workspace; the Windows command corpus (`.cmd` shims, Node/Python/Cargo) works under it | `TG4-09`, `TG4-18`, `TG4-31` |
| `S4-01` v1 | One task runs end to end on Windows and produces a build-verified artifact in the chosen workspace | Smoke |
| `P4-49` v1 | Razi's acceptance: build the Calendar app from the PA chat in `C:\Playground\arcana-calendar` | Razi runs it through `npm run tauri dev` |
| W9 v1 (added 2026-11-03, Razi to confirm) | **Judged in scope, but smaller than the first draft.** `bash` is already always asked per call (even in Autonomous), cannot be standing-allowed, runs through the OS sandbox and is denied headlessly, so what is left is that the consent names the tool, not what the command does. Do first: (1) stop the shipped worker recipe from using `bash` for git bookkeeping (`rev-parse`, `switch -c`, `status`, `recipes/defaults.rs`) by giving it typed git tools, and hide `bash` from default Fleet workers; (2) re-run the intent suite through the production `execute` path rather than the internal launcher. Lower priority and probably post-v1: a capability object in `ProcessSpec` with consent keyed by capability, and `run_ide_project` passing the claim and build-evidence check (it is user-initiated only) | New test per clause; intent suite via `execute` |
| Locked dependency staging (found 2026-11-03, **Razi to rule**) | `project_command` refuses every dependency install except the pinned Ruff one (`locked_dependency_staging_unavailable`, test `dependency_install_fails_closed_until_locked_staging_exists`). So an agent cannot run `npm install` or `cargo fetch`, and a fresh Node or Python project cannot build under the sandbox. This is the main reason a Calendar-style task (`P4-49`) and the v1 headline (a build-verified artifact from a new project) may not complete. Options: stage dependencies from a lockfile on the host, outside the sandbox and with the network allowed only for that step, then build inside the sandbox; or state that v1 builds only projects whose dependencies are already present | A fresh npm or Cargo project builds end to end under the sandbox |
| `P4-56` / `TG4-30`, command path only (added 2026-11-03, Razi to confirm) | Independent attack tests of the command and sandbox path: task theft, path escape, secret leakage through command output and environment. The rest of the failure-containment review stays in the backlog | `TG4-30` narrowed to commands |
| `P4-50/51/53/54` | Deferred to the backlog (what is already built stays) | — |
| `TG4-24/25/27/28` | Deferred with their tasks | — |

The terminal-control pieces already built are the first row of "Already built in P4" above; W9 and the narrowed
`TG4-30` finish that area.

### R2 · Graph (P5)

Goal: the graph is compiled, correct, and read when the agent selects context.

| Task | Scope | Proof |
|---|---|---|
| `P5-01..03` v1 | One normalised graph DB path (four spellings produce four databases today); migrate existing DBs | Live-DB check, `S5-01` |
| `P5-04..07` v1 | Stop `INSERT OR REPLACE` cascade-deleting edges; real upsert; edge-conservation test; backup on the destructive ladder step | `S5-02` |
| `P5-14..17` v1 | Graph is read during context selection, scoped by graph distance, bounded; a recalled node appears in the outbound prompt bytes | `S5-05` |
| `P5-18..19` v1 | The 27 `graph_tools.rs` structs implement `Tool` and are registered; descriptions written to the `P3-20` standard (this absorbs `P5-21`) | `S5-04` |
| `P5-08..13` | Incremental re-cluster, rename, convergence: **defer; re-index in full on change** unless full re-index is too slow on a real repo | If kept: `S5-03` |
| `P5-20` | Bookkeeping only; dropped | — |

### R3 · UI and experience (P6, P7)

Goal: a new user reaches a working agent turn, errors are visible, nothing is lost, nothing is inert.

| Task | Scope | Proof |
|---|---|---|
| `P6-02` v1 | Self-host the font (the CSP blocks the external fetch, which is why Graph City hangs) | `S6-01` |
| `P6-01`, `P6-04` v1 | Token set from [`reference/ui-anchor.md`](reference/ui-anchor.md), defined once, no literal hex in components | Static gate |
| `P6-03`, `P6-05` | Dark mode and a drift gate for every surface: defer | — |
| `P6-06/07/09` v1 | Event subscription above the tab boundary; no event lost on tab switch; dead listeners removed | `S6-02` |
| `P6-08` | Reconnect replay: defer | — |
| `P6-10..13` v1 | Add the 2 missing commands, remove the 47 dead commands and 10 dead listeners, gate that both sides agree | Contract test |
| `P6-14` | Argument/return type checking: defer | — |
| `P6-15..18` v1 | Every setting persists and hydrates; provider/model survive restart; `ModelConfigsDb` read as well as written; no inert controls | `S6-03` |
| `P6-19..20` v1 | Graph City renders and reads the real graph; `P6-21` performance bench deferred | `S6-01` |
| `P6-22..23` v1 | Route or delete ~3,700 orphaned frontend lines; every route reachable from a cold start; `P6-24` gate deferred | `S6-04` |
| `P7-01..04` v1 | BYOK first run: enter a key, see it accepted, reach a turn, key never echoed, every failure names its next action | `S7-01` |
| `P7-05..08` v1 | Fix the two toast producers; errors surface once in one place; provider errors say what the user can do; completion, cancel, provider error and tool error are distinguishable | Smoke |
| `P7-09..11` v1 | Honest progress; cancellation actually cancels; long operations say what they are doing | `S7-03` |
| `P7-12..14` v1 | The 10 unconfirmed destructive actions ask first and say what is lost; undo where `git/checkpoint.rs` supports it | `S7-02` |
| `P7-15`, `P7-17` v1 | One vocabulary (worker, task, run, session); empty states that teach | Doc/UI check |
| `P7-16` | Interaction-grammar doc gate: drop | — |
| `P7-18` | Capstone walkthrough without docs: folded into `S7-01` | `S7-01` |
| `P7-19` | Automatic recovery after process loss: backlog | — |
| `P2-21` | Daily spend ceiling (M-42), the one open P2 task. Unpriced models are invisible to the cap, so a paid-key app should not ship without it | Test |

### R4 · Windows delivery (P9)

| Task | Scope | Proof |
|---|---|---|
| `P9-05` v1 | Windows installer (`installers/wix/`) builds, installs, launches | Smoke |
| `P9-09`, `P9-10` v1 | No `.py`, sidecar or dev `.env` inside the installer; installed size recorded | Gate |
| `P9-15` v1 | **Headline acceptance** on a clean Windows machine, from the shipped installer: install, enter key, hand over a task, receive a build-verified artifact (`S9-01`, `S9-03`) | Smoke |
| `P9-02` | Windows code-signing certificate (Razi's action). Without it the installer works but shows a SmartScreen warning, which matters now that the whole company runs Windows. Ruling needed | — |
| `P9-03`, `P9-04`, `P9-11..14` | Production updater key, signed updates, rollback: **ship v1 with the updater inert** (the README already says no feed exists). `S9-02` goes with it | — |
| `P9-01`, `P9-06..08`, `P9-16` | Apple/Linux installers, notarisation, cross-platform parity: backlog | — |

## 5. Status snapshot

Verified on 2026-11-03 (R0 run; provider keys unset, `USERPROFILE`/`HOME` redirected to a scratch profile, `NoDefaultCurrentDirectoryInExePath` unset where noted):

| Check | Result |
|---|---|
| `npm run typecheck` | passes |
| `npm run lint` | 0 errors, 40 warnings |
| `npm test` | 63 pass in 14 files |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets --locked` (CI mode, no `-D warnings`) | passes with about 400 warnings. It failed to compile `arcana-core` before R0: the deny-by-default `clippy::absurd_extreme_comparisons` at `process_sandbox/psec.rs`, so CI's clippy step was red. The `needless_borrow` set in `src-shared/src/db/analytics.rs` is still present (a `-D warnings` run would fail) |
| `cargo test -p arcana --lib` | 179 pass (178 on three consecutive runs before the last self-test was added) |
| `cargo test -p arcana-core --lib` | 1187 pass, 6 fail, 3 ignored in the full run (35 minutes; the slow ones are the leap-year/DST round-trip tests). Every failure is explained: telemetry labels (fixed), four environment effects (below) |
| `cargo test --workspace` other targets | `concurrent_writers`, `p4_query_registry`, `redaction_boundaries`, `s1`..`s8` targets, `arcanactl` unit tests, `consent_cli`, `core_runtime`, `project_command_stage_logs`, `startup_retry_logs` all pass |
| `fleet_offline` (`--features offline-fleet-test`) | **Not in CI and runs 0 tests in a default `cargo test`.** 7 pass; 51 of 52 parallel runs since the dispatcher fix pass; before it, 1 of 6 failed on the dispatcher assertion. The one failure since was a missing `fleet_cli_run_completed` event in the log export (**not reproduced in 40 further runs, root cause unproven**; the failure message now prints the log files on disk) |
| `TG3-26` / `TG4-16` named tests | pass (`qc_diff_without_matching_consent…`, `analyze_diff_tool_executes_repository_diff…`, `consent_commands_persist_and_revoke…`, `concurrent_revocation_blocks_the_next_request…`, `consent_cli_persists_grant_status_and_revoke_across_processes`). The register's own open clauses are **still open**: the full allowed request/log/revoke path through every consumer and the Tauri PR-diff invocation, and the actual desktop IPC smoke |

Environment effects, not product defects (each passes once the condition is removed):

- Four tests (`sandbox_allows_workspace_writes_and_blocks_sibling_writes_from_child_tree`, `dropping_a_sandbox_command_terminates_its_descendant_processes`, `confirmed_tree_stop_counts_and_terminates_real_bridge_descendants`, `production_command_timeout_stops_nested_process_writes…`) fail with "`parent.cmd` is not recognized" when the invoking shell sets `NoDefaultCurrentDirectoryInExePath=1`. This session's shell does. They are fragile to the caller's environment and should clear that variable for their child; not done in R0.
- `checked_in_ecosystem_corpora_run_real_intents_through_the_production_launcher` needs the signed Python runtime staged under the real `~/.arcana/python-runtimes`; it fails under a redirected home.
- No `OS error 5` AppContainer failure appeared in this run.

Claimed progress from the archived plan, **not re-verified**: P0–P3 and P8 closed; P4 at 51/59 tasks and
25/33 gates (the old trackers disagreed, giving 46 to 51); P5, P6, P7, P9 untouched. The archived gate
checkboxes are unreliable: 34 of 41 TG3 gates are unchecked although P3 is recorded closed. Treat the
code and a fresh test run as the truth, not those boxes.

Last feature commit: `8356aea` (2026-10-28), worker dispatch lineage. Since then: the docs reset, the e2e consent spec (`121b28e`, unrun) and the R0 baseline fixes (`3d039ab`: gates, telemetry labels, clippy error, test fixes; no feature change).

## 6. Where things are

| Need | Go to |
|---|---|
| Deferred work and the parked Linux/macOS branch | [`HARDENING_BACKLOG.md`](HARDENING_BACKLOG.md) |
| Decisions made from now on | [`DECISIONS.md`](DECISIONS.md) |
| Code map (verify against the code; last refreshed 2026-10-14) | [`reference/PROJECT_MAP.md`](reference/PROJECT_MAP.md) |
| UI and memory design anchors | [`reference/ui-anchor.md`](reference/ui-anchor.md), [`reference/memory-anchor.md`](reference/memory-anchor.md) |
| What P4 already delivered, grouped by area | [`reference/P4_DELIVERED.md`](reference/P4_DELIVERED.md) |
| Lessons learned in P4 | [`reference/ARCANA_ENGINEERING_LEARNINGS.md`](reference/ARCANA_ENGINEERING_LEARNINGS.md) |
| Old plan, gate register, 412 ADRs, 230 receipts | [`archive/`](archive/README.md) |
