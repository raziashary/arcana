# Arcana goal (updated 2026-10-13)

**Current scope and proof (ADR0391, 2026-10-13):** Core P4 is Windows-first and
remains open at **45/59 tasks and 23/33 TG4 gates** after the named P4-14/TG4-07
QC, P4-15/TG4-08 diagnosis and P4-57/TG4-19 timestamp closeouts. Finish the
Windows PA → Fleet engineering path and complete the full built-in failure,
graph/MCP and observability requirements,
then leave P4-49's Calendar acceptance to the owner. Safe quarantine,
host-verified reconciliation and explicit Supervisor manual handoff stay in
P4. Automatic process-loss replay/reassignment belongs to P7-19/TG7-06;
Linux/macOS Fleet sandbox, toolchain, dependency, timestamp and worker parity
belongs to P9-16/TG9-06 and must stay fail-closed until proven. After core P4,
read and finish `P4_EXPANDED_GOAL.md`, followed by P5, P6, P7 and P9.
No second paid provider run is authorized. The dated status below is retained
as history where it differs from this current scope.

Current state: **active**. Owner resumed core P4 and approved all five runtime,
sandbox and privacy decisions in ADR0359. Earlier pause/pending notes below are
historical. Shared privacy hardening and provider cancellation units are verified. The 2026-10-12 feature-gated production CLI success/deadline/export harness passes 5/5, but live-provider Fleet completion remains open. Windows shell-hook lifecycle also gained seven focused passes, and the Linux staging planner regressions pass 8/8; strict sandbox, desktop attachment, provider-cost, and remaining observability proof continue.

Latest checkpoint/continuation: `_verify/P4-SESSION-CHECKPOINT-2026-10-13.md`. Core P4 is
**42/59 checked**, TG4 **20/33 passing**; 17 tasks remain, including the
owner-run Calendar acceptance. P4-59/TG4-21 closes on complete retention
consumer evidence across Windows, Linux and macOS. P4-52/TG4-26 closes on its
own verified retry contract with owner approval; successful paid Fleet
completion stays open under P4-50/TG4-24. Reports:
`_verify/P4-59-RETENTION-CLOSEOUT-2026-10-06.md` and
`_verify/P4-52-STARTUP-RETRY-CLOSEOUT-2026-10-06.md`.

Capture unit `1cdb7a1` passes the current Linux whole-workspace CI job and
Windows desktop 167/167. The prior capture-loss cause remains unconfirmed.
macOS passes all six retention selections; its separate command corpus fails
because the signed XPC runner is absent. This is not a green full pipeline.
Session/message annual write/read proof passes Windows/Linux. Owner-approved
hook execution-time preservation (ADR-0320) is implemented and verified: hook
module 11/11 on both hosts, core check, and real Windows CLI export 1/1.
P4-57 remains open for other pairs and complete platform proof. The isolated
test-only desktop build (ADR-0321) now builds; attachment proof is underway.
No second paid Fleet run is authorized. P5 and P6 remain downstream of P4 and the expanded-goal closeout. The expanded goal remains unread
until core P4 closes.

2026-10-06 continuation: three RunsDb CI failures were traced to stale test
fixtures after migration 8. The repaired production-reader suite passes
29/29 on Windows and Ubuntu WSL; the query registry passes 8/8 on Windows.
Remote CI and its supported-platform jobs are the next proof boundary.
P4 remains 38/59 and TG4 17/33; timestamp and retention closure are pending.

Analytics timestamp-pair tests now pass 2/2 on Windows/Linux through real
writers/readers and legacy fixtures. The complete six-group retention suite
passes on Windows and is selected in the three-host CI matrix. Evidence:
`_verify/P4-57-ANALYTICS-PAIR-PROOF-2026-10-06.md`. Remaining database pairs,
remote host results and successful Fleet completion are still required.

Historical continuation (2026-10-05): **P4 38/59 checked; TG4 17/33 passing**.
P4-55/TG4-29, P4-43, P4-40, P4-23/TG4-10/11 and P4-29/TG4-15 are proven. P4-16/TG4-17 and TG4-06 are also proven. Core P4 remains active; 21 tasks
remain, including owner-run P4-49. The expanded goal stays unread until core
P4 closes. Claim-safety proof is in
`_verify/P4-40-CLAIM-SAFETY-CLOSEOUT-2026-10-05.md`; budget interaction is in `_verify/P4-29-BUDGET-INTERACTION-CLOSEOUT-2026-10-05.md`. Remote host-matrix proof,
strict sandbox corpus and successful CLI Fleet completion remain open. Earlier
dated status entries below are historical snapshots.

Latest verified phase commits are pushed to `origin/main`: `129418e` records
timestamp-reader and retention reconciliation, followed by `44d3e75` for
bounded worker setup retry admission. Retry tests pass on Windows and Ubuntu;
the production JSONL logger-to-`arcanactl logs export` readback passes on both
platforms. Explicit Settings metadata-only provider checks now emit safe
outcome/timing events, and the shared consent wrapper's preflight/complete/
stream methods reject mismatched and revoked grants; focused Windows tests pass.
A bounded paid CLI Fleet run has now occurred: six Gemini generations succeeded, one attempt failed, and the next was denied by the run-token reservation. It consumed $0.05950545, created no child tasks, and its logs and analytics exports read back. This is partial production evidence, not successful Fleet completion; see _verify/P4-50-CLI-FLEET-LIMITED-2026-10-05.md. Counts remain 38/59 and 17/33, no P4/TG4
checkbox closed, the fresh-profile desktop harness still has an initial
DevTools attach failure, and successful paid CLI Fleet completion remains
open. The user-owned `P4_EXPANDED_GOAL.md` remains unread until core P4 closes.

The owner approved a 6,144-token output cap for budgeted Supervisor runs after
the trial's 8,192-token reservation limited admission. RecipeRunner now selects
the cap for Supervisor only; focused offline request-builder and selection
tests pass. This changes no checklist count and does not claim a second paid
run or successful CLI Fleet completion.

2026-10-05 continuation: P4-59 retention reconciliation now passes its focused
23-test suite on Windows and Ubuntu WSL; the current SQL registry passes 8/8.
Analytics migrations 6–8 preserve exact open-date session accounting, seal
pruned dates, and reject both direct inserts and timestamp updates into sealed
dates. CLI Fleet now owns the same maintenance job during its run and stops it
on return. P4-59/TG4-21 stay open pending remaining cross-platform/full-gate
proof. Selected P4-57 timestamp-reader regressions pass on Windows; full
TG4-19 inventory and cross-platform properties remain open. Overall counts do
not change.

Build Arcana as a personal desktop coding harness: Arcana owns deterministic,
observable, and safe execution where rules are knowable, while models provide
judgment and code generation where intelligence is needed. Use the in-process
Rust agent runtime for interactive chat, worker recipes, and subagents.
The current owner direction and active sequence are in
[ARCANA_CODING_SCOPE.md](ARCANA_CODING_SCOPE.md); the historical implementation
and proof rules remain in the MDP §0.

## Product vision

Arcana is not just another interface around a coding model. The harness and the
model work side by side: Rust-owned workflows handle project discovery,
toolchain selection, command construction, permissions, task lifecycle,
evidence, recovery, and other deterministic work; the model plans, writes and
reviews code, and handles ambiguity that cannot be resolved safely by fixed
rules. Every deterministic capability must be used by a real production path
and proven through behavior, not left as an unused tool or prompt suggestion.

Arcana should also arrive with a useful development-team workflow already in
place. A user should be able to start with the built-in recipes, agents, task
coordination, memory, review, and recovery without spending hours configuring
AGENTS.md files, subagents, skills, or tool wiring. Recipes remain the supported
way to tune that workflow to personal preferences. Built-in defaults must be
capable, coherent, safe, and testable before customization is considered a
substitute for missing product behavior.

## Failure-containment objective added 2026-09-28

P4 must make failure visible and contain its effects so coding work can
continue without silently losing progress. Every builtin tool failure must
return a bounded, versioned, categorized JSON contract with operation/tool
identity, retry guidance, safe cause, correlation context, and actionable next
step. Startup retries use capped exponential backoff with full jitter and stop
before replaying work after a side effect. Mid-run interruption must preserve
task-attributed progress and file checkpoints; the Supervisor must reconcile
claims and uncertain effects before resuming or safely reassigning work.
Security enforcement belongs at Rust, SQLite, filesystem, and OS boundaries.
Readable and structured logs rotate with at most five files per stream and a
seven-day maximum age, without touching foreign files or following symlinks.

| Outcome | Current baseline | Completion proof |
|---|---|---|
| One coding runtime | Shared `arcana-core` is linked by desktop and CLI; Python sidecars removed | Chat and worker finish a real coding turn through native engine, with tool result read by UI/task board |
| Arcana-owned harness | Rust-owned project/toolchain discovery, validated command selection, explicit install intent, task claims/transitions, persisted build evidence, and QC evidence consumption now have production consumers; generic shell remains available for coding work outside the deterministic command contract | For each deterministic workflow, identify the Rust-owned contract, real consumer, refusal/error behavior, and behavioral proof; cover project/toolchain discovery, validated command selection, explicit install intent, task claims/transitions, permissions, build evidence, recovery, and egress boundaries without treating model prose as enforcement |
| Built-in team workflow | Worker recipes, task coordination, memory, reviews, and recovery exist at different maturity levels; the 2026-10-03 P1/P2/P3/P8 source audit is recorded in `_verify/2026-10-03-CROSS-PHASE-AUDIT-REFRESH.md` with its proof gaps assigned to owning gates | A fresh project can complete a coding task using Arcana defaults without user-authored agent scaffolding; verify shipped recipes/tools are actually called, work is routed and recovered safely, review has evidence, and customization is optional rather than required |
| Reliable and failure-contained fleet | P4-52 retry admission/delays are bounded and tested; synchronous setup is not interrupted by the two-second check, so the total-deadline decision is pending in ADR-0287. P4-54 writes production worker run phases, 15-second heartbeats, task-attributed operation history and interruption outcomes; the Supervisor health tool also reads a live journaled operation label and elapsed milliseconds for open running tasks. Retry counters, model/token usage, parent/child links and recovery continuation remain open. P4-61 classifies `project_command(install)` as per-call host approval in every mode; saved tool allowances cannot skip it and the card hides “Allow Always.” Generic project dependency installs remain fail-closed until locked host-controlled staging exists; the exact pinned Python/Ruff host-tool request is a separate per-call approval path with verified wheel staging (production approval-to-lint proof remains open). Focused executor/tool and UI tests pass. Selected MSVC build smoke passes; the Windows Clippy corpus remains blocked because AppContainer reads config bytes but path canonicalization returns `Access is denied`, including when `CLIPPY_CONF_DIR=.`. Runtime grants are not yet an exact-file closure, and Linux/macOS do not meet the external-read/network boundary; dependency staging and trusted OS-denial consumption remain open. P4-62 uses a one-hour command default, two-hour per-command maximum, two-hour worker deadline, and separate sandbox/process/output/evidence timing. A Windows single-child timeout test passes; descendant launch was denied by AppContainer, so process-tree proof remains open. Current compile, formatting, TypeScript and 38 frontend tests pass; production timing, recovery and cross-platform proof remain open. | Complete every unchecked P4 task and TG4 gate, including P4-50 through P4-62 and TG4-16/19–21/24–32: structured agent-readable failures; bounded exponential full-jitter startup retry; task-scoped checkpoint/recovery; supervisor-visible health; bounded log/database retention; security falsification; four Logs sections with real Memory/SQLite instrumentation; untrusted child-process read/network isolation with staged dependencies and per-call install approval; long-command timing and worker-deadline proof; and successful CLI Fleet execution with observable recovery. Then leave only owner-run P4-49 desktop acceptance. |
| Useful context | Memory and graph code exist; P3 memory gates remain incomplete | Persisted project facts and code context are retrieved and visibly used by a later relevant turn, with source grounding and no irrelevant-context regression |
| Honest UI | React tabs and event stores exist; P6/P7 open | Desktop launch, normal workflow, event producer/consumer and setting readback checks |
| Arcana identity | Rename applied in source; validation is in progress | Fresh Arcana data/key location, packaging checks |
| Installable product | P9 is later work | Windows, Linux, macOS build and launch smoke with applicable security checks |

Historical checkpoint (2026-10-03): independent Luna High review reopened P8-74
and TG8-13 after finding a branch-switch race in host commit safety. The host
now captures/rechecks the validated branch ref and OID; the injected race
regression passes as part of `git_commit::tests`. A real isolated TasksDb claim
is now revoked while the commit waits on a file lock, and the production claim
validator refuses the commit with HEAD/index unchanged. The persisted shipped
backend recipe now drives a scripted model-generated `git_commit` through
scheduler kick, RecipeRunner, AgentEngine, task claim, host commit, and run-
journal readback; its focused test passes 1/1. The focused host, Bash, S8-06
and Tauri registration gates also pass; an independent GPT-6 Luna review found
no material gap in the production-path proof. P8-74/TG8-13 are proven. P4
remains active and incomplete. P4-35 is now proven: a persisted idle QC worker
was kicked through the scheduler, RecipeRunner and AgentEngine to process an
inbox-only message, and rejected work remains eligible for recovery. P4-23
still owns at-least-once replay/idempotency. The accumulated work is uncommitted on
`main`; no staged feature commits or push have been made in this continuation.
The final check-to-ref-update race is now closed by a short `BEGIN IMMEDIATE`
claim guard; the concurrent status-writer regression passes, and all 15
`git_commit::tests` pass. Orphan reconciliation now propagates database scan
and release failures and emits a timed Supervisor failure event; orphan tests
cover the failure paths. The Windows PSEC Clippy canonicalization failures and
other recorded P4 gaps remain open.
The resumed P4 work fixed the observed Windows equal-size log-rotation cache
bug by identifying the JSONL file from its open handle and reading from that
same handle. Metrics now avoid counting retry policy events as extra physical
provider attempts, include denied outcomes in failures, and keep call nodes
inside their Fleet-correlated trace. Focused Tauri log-command tests pass 24/24,
Logs tests pass 16/16, Tauri check, frontend typecheck, formatting, and diff
checks pass. These fixes improve P4-50 correctness but do not close TG4-24;
successful CLI Fleet completion and persisted reader-to-Logs UI evidence remain
open.
Windows Python Settings now persists an explicitly selected signed fallback
only beneath the current user's Local AppData Python tree, and runtime staging
consults it only after the preferred runtime fails. Runtime tests pass 5/5 and
the Settings selector tests pass 2/2. This machine has no `python.exe` in that
user-owned install tree, so fallback execution remains unproven.
At the owner's direction, Gemini 3.8 Flash is the PA and fleet default. The
Logs tab has partial groundwork for the final calendar run: correct Rust-target
parsing, full-log export, bounded tail reads, incremental line counts, visible
refresh timing/file size, and selected task/tool duration records. This is not
the P4-50 observability system. P4-50 adds structured JSON-expandable operation
events, correlated parent/child traces with duration, token/call/outcome
metrics, and four expandable UI sections for Logs, Structured Events, Traces,
and Metrics. The owner's latest P4-50 expansion also requires timed,
privacy-safe memory operations, per-statement timing across production SQLite
databases, and a real CLI Fleet run through the production runtime without Tauri, including representative success and injected recovery behavior. CLI help or capability preflight alone is not execution proof. The
production `arcanactl fleet run` command now uses the shared core runtime,
requires explicit workspace egress consent, and has persisted CLI
`consent grant|revoke|status` commands. Compilation and the isolated consent
lifecycle pass. A real Gemini Fleet attempt was denied before provider egress
until consent was granted; after consent, the synchronized saved credential was
used, and the Supervisor exhausted its existing run budget before decomposing
the epic.

The cross-phase audit reopened P4-14 after finding that workspace edits could
leave an earlier passing build eligible for QC after a later timeout. The owner
approved workspace-fingerprint binding. Additive migration 18 and the
production QC API/SQLite trigger now require the latest passing observed build
to match the current selected workspace fingerprint; timeout and spawn errors
remain distinct and do not fabricate exit evidence or alter the failure streak.
Focused real-Cargo, timeout, direct-SQL, migration-upgrade, and fingerprint
tests pass. P4-14 remains open for complete cross-platform process-corpus proof.
The corrected full core rerun completed 804/806 tests; the only two failures
are the same Windows AppContainer Clippy-config canonicalization denials in the
focused Clippy test and Rust production corpus. `cargo check`, the query
registry (2/2), Logs UI tests (12/12), TypeScript typecheck, formatting, and diff
checks pass; this is not an all-green P4 result.
The CLI shared analytics reader also now uses the profiled SQLite initializer;
its populated production-readback test passes. P4-50/TG4-24 remains open for
the real producer-to-Logs flow and full operation coverage.

The explicit bounded per-run Supervisor budget override is implemented and was used in an isolated CLI Fleet run. That run selected `gemini-3.8-flash`, initialized seven recipe-bound workers, and persisted epic #1, but Google returned `429 RESOURCE_EXHAUSTED` because the reported Gemini free-tier request quota (20) was exhausted. It created zero child tasks; isolated task/run state and JSONL logs were read back, temporary workspace consent was revoked, and the scheduler exited without a lingering process. The structured event file reached 26,633 records / 6,977,969 bytes, mostly DEBUG SQLite statement events. Successful CLI completion and Logs UI end-to-end readback remain unproven; the Calendar owner test remains untouched.
The earlier CLI help check remains preflight only; successful CLI task completion, producer-to-Logs-UI readback, and recovery
proof remain open; `--help` is not execution evidence.

The owner expanded P4 on 2026-09-28 to include failure containment: P4-51–56
cover structured tool errors, bounded startup retries, resumable task-owned
checkpoints, Supervisor-visible worker recovery/reassignment, five-file and
seven-day log retention, and cross-boundary security tests. These are newly
added scope, not verified capability; see TG4-25–30 and the P4 readiness row.
The owner clarified on 2026-09-29 that every unchecked P4 task and TG4 gate in
the active register is part of this closeout, not only P4-50 through P4-56.
Each must be completed and behavior-proven before the owner-run P4-49 Calendar
scenario; P4-49 remains the only intentionally owner-reserved acceptance.

The owner-provided revised `goal-objective.md` was reread on 2026-09-29 and audited against the active plan. The audit found requirements that had not been fully represented: production CLI Fleet execution, full categorized/correlated error fields, exponential full-jitter retry, repository snapshot and uncertain-side-effect reconciliation, complete Supervisor health fields, telemetry-failure isolation, and Fleet-wide cancellation propagation. MDP and TG4 wording now reflects these requirements; existing partial code does not satisfy them. The latest full Rust workspace attempt stopped in the desktop library target after 132/135 tests; three failures came from stale source gates. Those scanners were corrected and focused reruns passed, but a fresh full latest-source run is still required.

On 2026-09-29, P4-52 gained a bounded RecipeRunner setup retry before
`AgentEngine::run`: at most four attempts, exponential full jitter, and explicit
per-delay/total-time caps. Seven focused startup retry tests pass on the current
tree. The task remains partial until real worker claim/budget continuity and
production post-side-effect no-replay behavior are proven; see ADR-0114 and
TG4-26. Vault failures remain non-retryable because the current error type does
not distinguish transient keychain unavailability from permanent vault data
errors.

On 2026-09-29, P4-54 gained a durable `worker_run_state` and bounded
`worker_run_events` ledger in `runs.db`; production RecipeRunner records
startup retries and terminal outcomes, heartbeats worker/run state every 15
seconds, and stores safe task-attributed tool operation labels. Scheduler
interruption recovery closes the run only after releasing task claims. A
Supervisor-only `list_worker_health` tool reads roster, latest run state,
recent operation history, and task-scoped checkpoint counts; the default
Supervisor recipe now consults it before recovery. Eight RunsDb tests, two
health-tool tests, and all 13 RecipeRunner tests pass. This is health visibility
and recovery accounting, not yet resumable exactly-once execution: two-worker
handoff, side-effect reconciliation, and duplicate-effect prevention remain
open under TG4-27/28.

Desktop startup was exercised on 2026-09-28. The first run exposed a Tokio
reactor panic in the runtime reaper; Tauri setup now enters its managed async
runtime, and subsequent runs kept the Arcana process/window alive. A
backward-compatible migration repairs legacy shell-smoke sections in persisted
backend, frontend, MCP, fleet-smoke, and QC recipes while preserving user text;
its focused idempotence test passes. The latest live launch started all seven
workers, including backend and frontend. This is worker-start evidence only;
the owner-run PA-to-fleet calendar acceptance remains the final P4 work. The
owner will run it after the remaining engineering gates are ready and return
the transcript and logs for analysis; this session will not submit that task.

P4 now has focused behavior proof for owner-checked claims and DB claim
invariants, QC's SQLite evidence trigger (including direct-SQL rejection),
observed build-failure accounting, claim-activated task budgets, deterministic
batch routing based on automatic outcomes, task-associated file checkpoints,
workspace resolution, saved workflow portability validation, consent-aware
provider/diff paths (including a real `AnalyzeDiffTool` repository diff under
missing-consent refusal), and interrupted-run recovery pieces. Current focused tests
pass for the QC trigger, build-failure threshold, saved-budget activation,
automatic batch dispatch, runnable-worker filtering, and checkpoint restoration.
The checkpoint regression test and routing assertion were corrected in this
continuation; formatting and focused tests pass. New production-path tests also
prove denied Autonomous Bash does not execute, and that three observed failed
Cargo builds through `project_command` expose diagnostic escalation. Saved
recipe, skill, and hook portability refusal tests pass on Windows. These tests
do not close P4's full desktop paths or phase gates. The Tauri manual
force-release command also refuses an old UI snapshot after worker
reassignment; the injected preflight/CAS race remains open.

P4 remains open for full build retry and worker-diagnosis behavior; the full
TG4-17 corpus across shipped and persisted workflows; three-OS process write
containment (Bash and `project_command` now share a fail-closed launcher;
Windows AppContainer behavior is passing locally, while Linux Landlock and
macOS sandbox behavior have not yet been run on their target OS); task-wide
reversibility across all mutation paths; ACP recovery and
idempotent side-effect proof; full artifact reveal/open/run; remaining S4 smokes;
three-OS project-command results; and unresolved P1/P2/P3 audit follow-ups in
their owning registers. For P4-18, the owner has directed both OS write
containment and per-call Bash approval. Windows containment now has focused
passing tests for direct and nested child writes, a `.cmd` shim, a junction
escape, and the production Bash tool path. Cancellation, actual toolchain
through the sandbox, and target-OS CI proof remain before the PA fleet
acceptance run.

**Owner-run final P4 acceptance metric (P4-49).** After P4-50 observability,
the engineering work and gates are ready, the owner will launch the Windows desktop app and use the
PA chat window to delegate a coding task to the fleet. The fleet must build a
small Google-Calendar-inspired application in the dedicated folder
`C:\Playground\arcana-calendar`. The delivered app must have a working
frontend, backend, and their integration. Arcana must run its supported
verification and expose task/build evidence through the real fleet workflow.
The minimum functional scenario is to create a calendar event in the UI, read
it back after refresh from backend persistence, edit its details or time, and
delete it. Record the actual frontend request and backend response/storage path;
the UI must render the returned state rather than a frontend-only mock. Run the
supported frontend and backend verification through the fleet's task/build
evidence flow, launch and inspect the result, and retain the runnable artifact,
smoke logs, task IDs, worker handoffs, observed exits, and UI behavior under the
target folder. Export the full Arcana daily log and complete selected-run
observability, including structured events, correlated traces, and token/call/
latency metrics from the four Logs sections.
Fingerprint the existing Playground contents before and after;
preserve every entry outside `arcana-calendar`. A direct manual implementation,
CLI-only run, unit-test substitute, or frontend-only mock does not satisfy this
metric. This acceptance run has not yet happened; P4 remains open until the
owner returns its end-to-end evidence and it has been analyzed.
The controlled task brief and evidence checklist are in
[`_verify/P4-49-CALENDAR-ACCEPTANCE.md`](_verify/P4-49-CALENDAR-ACCEPTANCE.md).
This run is intentionally reserved for the owner as the final P4 action.

Owner decisions now settled for P4 include keeping per-task budgets and
activating them after a successful claim; counting each observed nonzero
Rust-owned `project_command` build exit and resetting on an observed pass;
keeping Bash for general work while reserving QC build evidence for
`project_command`; validating persisted custom recipes, skills, and hooks;
enforcing QC evidence and claim metadata at SQLite boundaries; and requiring
compare-and-set for manual and automatic claim release. ACP uses at-least-once
replay until acknowledgement with stable-ID idempotency and current-state
reconciliation. Consent covers Arcana-mediated provider and diff egress.
Decisions are recorded in the MDP and ADR log.

Arcana's required default workflow remains: Rust owns deterministic discovery,
selection, permissions, state, evidence, recovery, and command construction;
models provide code generation and judgment where fixed rules cannot decide.
Built-in team workflows must work without users first authoring AGENTS.md,
subagents, skills, and tool wiring, with recipes available for tuning. The user
has also reaffirmed that P4 is the fleet phase and that the calendar acceptance
run is one of its main success metrics. P5-P7 and future features remain later
work. PD-X design work is retired and preserved only as historical record.

Continuation note (2026-09-29): a Luna High read-only P4-50 audit identified observable producer/consumer gaps. This worktree now groups and filters trace views by correlation ID, fixes tool result-byte aggregation to consume `output_bytes`, adds safe timers for cross-tier Memory search and aggregate stats, reports semantic extraction write failures truthfully, profiles `open_tuned` setup pragmas, and records bounded MCP health/reconnect outcomes without endpoint URLs. Logs UI tests pass (5/5), the full frontend suite passes (8 files/28 tests), and TypeScript typecheck passes. Rust formatting and focused observability tests pass. The previous workspace run ended at 686 passed / 7 failed on an earlier snapshot; the current-source ecosystem launcher test failed at Clippy config access, while a separate bounded-output process test passed 1/1 in 430.42 seconds without stage-level timing. Current P4-53 work quarantines exact-owner interrupted claims in `needs_info`, exposes them to the Supervisor, and blocks automatic worker replay pending reconciliation. Focused DB, scheduler, Supervisor queue/next-tick, shipped-prompt, and status-tool tests pass. This is containment, not resumable execution. The production CLI Fleet path now exists and has been executed with the new .env key; it proved seeded-worker startup and provider-failure handling, but successful task completion and post-fix telemetry readback remain open, broad current-source Rust validation and observability cost/recovery proof remain open, and P4-49 remains owner-only.

### 2026-09-29 continuation — interrupt quarantine and measured launcher path

The fresh Luna High recovery audit confirmed the live scheduler returned
interrupted task claims to `ready` before reconciling uncertain side effects.
Recovery now compares the exact worker PID/host, moves only that claim to
`needs_info`, preserves the assigned worker, records a `[worker recovery]`
reason, emits a recovery event, and wakes the Supervisor. The Supervisor work
probe sees these holds; the worker probe does not. Its shipped instructions
require inspecting task/health/checkpoint evidence and leave uncertain work held
for PA review rather than automatically replaying it. Focused proof passes for
the DB claim/quarantine boundary, scheduler recovery and queue visibility,
panic/timeout handling, the next Supervisor RecipeRunner tick, the shipped
Supervisor prompt, and task-status handling. P4-53 remains open because durable
mutation intent/outcome, a completed-operation cursor, repository snapshots,
lost-acknowledgement reconciliation, and safe resume are still missing.

The isolated bounded-output process test passed 1/1 on 2026-09-29 in 430.42
seconds. This is an end-to-end cold test observation, not stage timing and not a
warm retained-lease comparison. ACL propagation is a plausible cost center but
is not yet measured; a staged cold/warm benchmark must precede grant changes.
The checked-in multi-ecosystem launcher still fails before Node/Python fixtures
at Clippy config access inside the Windows sandbox. The full workspace suite
has not been rerun against current edits. No current P4 gate is closed by these
focused results; CLI Fleet execution and P4-49 remain outstanding.

### 2026-09-29 continuation — focused failure and observability verification

The long workspace run completed at 686 passed / 7 failed against an earlier
compiled snapshot. Failures identified a missing MSVC/Windows SDK environment,
three scheduler fixtures that omitted the durable run-owner state required by
production recovery, recipe persistence accepting `/tmp/` in ordinary prose,
and a real P4-15 feedback defect: a 512-character error-cause bound truncated
the third-build-failure diagnostic. The repository now preserves actionable
caller guidance in structured `next_step`, rejects POSIX temp paths in
persisted recipe descriptions/instructions (on both save and load), and gives
interruption fixtures a durable owner record. Those regression fixes pass
focused tests: 14 recovery, 7 portability, one bounded-error, one
three-real-build diagnosis, 9 core observability, one Memory extraction, one
MCP-category, one metrics aggregation, and one production SQLite-to-JSONL test.
The production build-evidence/QC test also passes after three isolated builds
(208 seconds on this Windows host). The checked-in multi-ecosystem production
launcher test is currently running under a process-local VS/Windows SDK
environment. The previous 686/7 full-suite result is not a current-source
green result; finish the remaining launcher check and broader verification
before changing gate readiness. The user-provided `goal-objective.md` was read;
its five failure-containment items were already represented by P4-51–56 and
TG4-25–30. P4-49 remains the owner's final desktop acceptance.

### 2026-10-01 continuation — approval and telemetry audit fixes

The expanded P4 goal and the attached source-independent `project_command`
analysis were reread. The analysis's stated 544-second versus 0.81-second
ratio is not treated as a compiler benchmark because it has no matched
hardware, cache, command, or stage boundaries. The one-hour command default,
two-hour per-command/worker ceilings, and separate command-stage timings are
already implemented; production cancellation, virtual-time, reconciliation,
and matched cold/warm evidence remain open.

`CoreRuntimeOptions` now carries whether the host can answer one-call approval
prompts. Tauri keeps that capability; headless Fleet sets it unavailable and
the executor immediately refuses `HostApproval` effects without creating a
pending request or entering the protected handler. `project_command(install)`
still requests per-call approval in every desktop permission mode, ignores
standing allowances, and after approval fails closed before spawn until locked
host-controlled dependency staging exists. Focused CLI denial and desktop
approval tests pass; P4-61/TG4-31 remain open because external reads/network,
dependency staging, Windows runtime exactness, and cross-platform negative
proofs are incomplete.

Fresh P4 audits also found and this continuation fixed three observability
defects: taskless operation timers emitted a synthetic task ID `0`,
per-repository hashed graph databases appeared as `other`, and correlation
exports included unrelated readable log lines. Focused Rust tests cover those
cases; the headless approval test passes, `cargo check --workspace --tests
--locked` passes, the frontend suite passes 38/38, `npx tsc --noEmit` passes,
and formatting/whitespace checks pass. The audits still leave the structured
tool-failure context contract, SQLite result classification, observability
failure isolation/cost proof, durable resume/reconciliation, and target-OS
sandbox proof open. No commit or push has been made. The owner-run P4-49 test
remains the final reserved action; no `C:\Playground` path was touched.

The shared ToolExecutor now enriches failed tool results with stable category/code,
operation/tool identity, bounded cause and context, retryability, timestamp, and
available session/agent/task/call IDs before they reach event, persistence, or model
consumers. Focused envelope, headless permission-denial, approved-install sandbox-denial,
and a scripted production AgentEngine provider-consumer test for an unknown-tool
result pass. Available worker run IDs are propagated; executor infrastructure
failures become correlated failed tool results. P4-51 remains open: all-builtin and provider-consumer
coverage, timeout/cancellation classes, and planted-input privacy proof are not
complete; arbitrary build diagnostics may still contain source excerpts and need
a contract decision before this gate can close.

### 2026-10-02 continuation — failure contract and consumer proof

P4-51 now enriches handler, panic, refusal, sandbox, unknown-tool, and executor
infrastructure errors with bounded stable codes/categories, operation/tool
identity, retry classification, timestamp, and available session/agent/task/call
/run IDs. Tool call IDs are carried consistently from permission request through
execution and error context. A scripted production AgentEngine test verifies the
provider receives an unknown-tool failure as JSON with `is_error=true`; the
production Bash test verifies nonzero exit is categorized and contains neither
the raw command nor its output. Timeout and spawn failures are categorized
separately, and project-command timeouts omit command output. Focused Rust checks
pass. The owner decided on bounded source-free compiler diagnostics: stable
diagnostic codes and workspace-relative locations without raw compiler output
or source excerpts (ADR-LOG-RESTRUCT-0137). P4-51 remains open for the complete
builtin/error-class inventory and planted-input privacy coverage.

### 2026-10-02 continuation — Python verification and sandbox evidence

The owner approved Python `compileall` as deterministic Build evidence and a
separate Python test run as additional confidence (ADR-LOG-RESTRUCT-0139).
The resolver now defines standard-library `compileall` while keeping pytest
separate; current evidence is resolver-level only because the Windows
production corpus still fails in Clippy path canonicalization before reaching
Python. The temporary Windows config-ancestor grants use only non-inheriting
traverse/read-attributes permissions, and the focused AppContainer test denies
sibling reads and loopback network. The real Clippy production test still
returns `Access is denied` when canonicalizing the readable config path, so
TG4-18 and TG4-31 remain open. Linux/macOS sandbox denial and staged locked
dependency proofs are also outstanding. No successful CLI Fleet task has yet
completed, and P4-49 remains reserved for owner-run desktop acceptance.
# 2026-10-02 continuation — Gemini signature preservation and CLI evidence

The provider-signature regression test parses repeated Gemini SSE updates,
feeds them through `AgentEngine`'s production stream assembler, then verifies
the real Google request builder replays a token already observed for that
part. `cargo test -p arcana-core --lib
repeated_gemini_stream_part_does_not_erase_its_thought_signature --locked`
passed. The first CLI run after this change did not reach tool execution:
Google returned 503 `UNAVAILABLE`. A read-only inspection of the isolated
profile confirms Arcana persisted `provider_overloaded`, retained the epic as
`ready`, and created zero child tasks. This validates the real CLI's overload
failure/readback path but does not live-confirm signature replay or successful
Fleet completion. Keep P4-50 and its CLI-success clause open. The run was
isolated under `%TEMP%`; no Playground or owner calendar path was accessed.

The Clippy canonicalization investigation also tried the owner's approved
exact-ancestor ACL option beyond the workspace. That isolated test sustained
high CPU for more than 60 seconds and was interrupted. Its unique temporary
ACE entries on the exact Temp/AppData/profile chain were removed, the unique
AppContainer profile was deleted, and source was reverted to workspace-only
traversal grants. ADR-LOG-RESTRUCT-0142/0143 records the correction. Do not
repeat this outer-ancestor ACL experiment; keep Clippy and the production
Windows ecosystem corpus open until a cheaper path-resolution method is proven.

## 2026-10-02 continuation — verification and review

On the current dirty tree, `cargo check --workspace --locked`, Rust formatting,
`git diff --check`, frontend typecheck, 38 Vitest tests, ESLint (0 errors, 43
warnings), three log-retention tests, the Python resolver contract, and P8's
TG8-13, ten `git_commit` cases, Bash mutation guard, and isolated S8-06
real-Git smoke all pass. The host Python runtime is present, but pytest is not
installed; no dependency was added because installs remain explicit. An
independent Luna High P8 review confirms the written controls and production
recipe registration, while raising an owner decision on scanning
credential-shaped commit-message text (the current P8 wording covers staged
diffs). The P4 production sandbox, successful CLI Fleet completion, broad Rust
behavior suite, remaining P4 gates, and independent P4-50 review are still
open. See the dated continuation in
`_verify/P4-SESSION-CHECKPOINT-2026-09-29.md`; the checkout remains uncommitted.

### 2026-10-02 continuation — observability consumer fixes

Current-tree focused tests now pass for taskless correlation serialization and filtering, build-evidence failure metrics, CLI enumeration of readable and structured JSONL logs, and quiet-mode filtering for console/readable sinks. These close concrete consumer defects only. P4-50 remains open for producer-to-UI proof, real successful Fleet execution, full privacy/error coverage, cost measurements, and telemetry failure isolation. No commit or push has been made.

### 2026-10-02 continuation — worker deadline simulation

The P4-62 paused-clock regression now runs the production `run_worker_future`
wrapper with the configured two-hour deadline and completes a simulated
10-minute-plus operation. This closes only that simulation clause. Process-tree
cancellation, task reconciliation, and matched stage-timing measurements remain
open; P4-62 and the phase remain open.

### 2026-10-02 continuation — observability consumer alignment

The Logs consumers now normalize CLI `correlation_id` with worker
`fleet_correlation_id`, discover dates present only in structured JSONL, and
include backend `failure` outcomes in the failed-outcomes headline. The Logs UI
suite passes 9/9, TypeScript typecheck passes, and the JSONL-only date test
passes. These close focused reader inconsistencies only; production
producer-to-UI evidence remains open.

### 2026-10-02 continuation — timeout recovery checks

The two-hour worker wrapper completes a simulated 10m+1s operation under
paused Tokio time. The isolated timeout recovery test resets a claimed task
through the recovery handler, and `project_command` default/clamp tests verify
its one-hour default and two-hour ceiling. Process-tree cancellation and
stage-timing benchmarks remain open.

### Accepted P4 decisions — observability, retention, Python, and spend cap

On 2026-10-02 the owner chose operation-level result events for SQLite-backed
DB, Memory, and graph operations. Keep the profile callback as duration-only
statement instrumentation, and emit success/failure from the containing
production operation after its result is known. The owner also approved OQ-5's
90-day raw `worker_stats` window followed by monthly rollups and a combined
reader; old rollup results must expose their actual monthly coverage bounds.
Python keeps `compileall` as build evidence and pytest as a separate confidence
signal. For Windows Python execution, first stage the exact runtime; if that
fails, fall back to a user-owned Python installation with only the selected
runtime grant, while retaining external-user-file and network denial. P2's
daily cap must use atomic conservative per-call reservations, reject unknown
cost/config state, and reconcile reservations with observed usage.

The SQLite callback alone still proves neither operation result nor its
consumer. These accepted decisions define required implementation and tests;
they do not close P4-50, P4-59, P4-61, or TG2-17.

2026-10-02 evidence: `cargo test -p arcana-core --lib observability::sqlite::tests --locked` passes 4/4. A captured structured event from the production profile callback proves normalized database identity, operation category, duration, and SQL/bound-value omission. The shared logger filter test passes 4/4 and proves fast DEBUG telemetry reaches structured JSONL under default desktop filtering. Owner chose operation-level result events because rusqlite's callback cannot see statement success/failure. `OperationTimer` now captures a fixed database identity and has a JSON event regression; worker-stat rollup/range and graph build/node/edge/FTS/flush production operations emit outcome and duration signals. Memory recall distinguishes unavailable and lexical-fallback results, and Logs metrics count fallback outcomes as degraded failures. Comprehensive result-event instrumentation and actual Logs UI production readback remain open.

2026-10-02 Python fixture recheck: Python 3.14.7 / pytest 9.1.1; the fixture
passes `compileall` with bytecode redirected under `target/` and its separate
pytest intent passes 1/1. This remains fixture evidence, not production
AppContainer/worker execution.

### 2026-10-02 continuation — Python test confidence and child cleanup

The owner confirmed that Python should have both a standard-library
`compileall` build check and a separate test check. The resolver regression and
local `compileall` fixture run pass. This machine's Python 3.14.7 has no
`pytest`, so the Python test intent could not run here; no package was installed
because setup remains explicit.

A focused Windows AppContainer regression now proves dropping a command job
stops a nested command that is actively writing a heartbeat and prevents its
later file marker. A second regression runs the real `project_command` launcher
future inside the configured Tokio timeout and proves timeout drop stops the
descendant heartbeat and prevents its later marker. This establishes command
launcher cancellation and sandbox descendant cleanup. Wiring a timeout through
task reconciliation and matched cold/warm stage timings remain open. P4 stays
Active; these tests do not move TG4-31, TG4-32, P4-50, or P4-61 to complete.

### 2026-10-02 continuation — rebuilt CLI log-list smoke

After rebuilding with `cargo build -p arcanactl --locked`, the actual CLI
`logs list` command exited 0 and listed both `.log` and `.jsonl` files. The
structured file was 5.0 KB and the readable file was empty. I did not inspect
or attribute those existing JSONL contents; this is discovery evidence only,
not proof of a new Fleet event reaching the UI.

### 2026-10-02 continuation — Python confidence and structured tool failures

The owner confirmed Python Build should use standard-library `compileall` and
Python Test should remain a separate confidence check. The checked-in fixture
passes both on Python 3.14.7; pytest 9.1.1 was installed in the user Python
environment after the missing runner was discovered. This proves the host
fixture only, not production sandboxed Python execution. The shared tool
executor guarantees a bounded `suggested_action` by preserving a handler value
or promoting its safe `next_step`; Rust formatting and five focused
executor-context tests pass. P4-51 remains open for complete error-class and
privacy coverage; broader P4 gates and the owner-run calendar acceptance are
also outstanding.

### 2026-10-02 continuation — Python fixture test now runnable

Correction to the earlier same-day checkpoint: after the owner confirmed that
Python tests should add confidence, pytest 9.1.1 was installed in the user's
Python 3.14 environment. The checked-in fixture now passes both the exact
standard-library compileall intent with bytecode in ignored `target/`, and
`python -m pytest` (1 passed). This closes the local host Python test-runner
issue; it does not prove sandboxed production execution of Python intents.

### 2026-10-02 continuation — Python owner decision and production launcher result

The owner confirmed Python Build should use `compileall`, while Python Test
runs separately as additional passing-confidence evidence. The Rust resolver
contract test passes. A direct grant to the installed `C:\Python314` runtime
failed, so Arcana now stages its selected standard library and pytest closure
into an Arcana-owned runtime cache. The production AppContainer launcher
passes Python `compileall` Build and `pytest` Test; a separate sandbox
regression denies reads from a synthetic runtime-cache sibling and writes
into the staged tree. The measured cold production test took 42.61s; verified
cache reuse took 9.14s. P4-11/TG4-18 and P4-61/TG4-31 remain open for Python
lint, an explicitly selected user-owned fallback, broader unrelated-user-file
denial, and cross-platform proof. No numeric confidence formula exists in the
current task/evidence contract, so pytest remains independent verification
evidence rather than an invented score.

### 2026-10-02 continuation — isolated production CLI Fleet attempts

The isolated CLI consent grant/status round trip succeeds. Two production Fleet
smokes against `gemini-3.8-flash` created durable epics but no child tasks and
must not be counted as a successful Fleet run. The first was intentionally
limited to 16,000 Supervisor tokens: its successful provider calls used 28,576
tokens total before the next-request breaker halted the run, showing that the
cap is enforced between calls rather than as an estimate of the in-flight
request. The second used a 100,000-token, $0.50, ten-turn run budget; provider
telemetry recorded 57,489 tokens before Gemini returned an explicit free-tier
request-quota error (`provider_quota_exhausted`). No provider retries are
planned until the quota permits them. Both durable epics remain in the ignored
isolated smoke home under `target/p4-cli-smoke-home-20261002`; the selected
workspace is `target/p4-cli-smoke-workspace-20261002`. No artifact was written
and no `arcanactl` process remains. This is failure-path CLI evidence, not the
successful production Fleet completion or owner Calendar acceptance required
to close P4. The second run's empty `api_failures` table exposed an unwired
analytics writer. AgentEngine now persists one bounded record for calls with
failed attempts, including retry count, stable provider error class, and
whether retry recovered; it omits the provider response body. Two deterministic
production-engine tests verify recovered-overload and non-retryable-quota
outcomes through the existing AnalyticsQueries reader. These prove the
producer/consumer path locally; a live CLI confirmation remains blocked by
provider quota.
### 2026-10-02 continuation — log retention reader seam

All five core log-retention tests pass. They include active append-handle
survival while pruning older files and a captured tracing-subscriber proof that
cleanup failure detail and summary are logged without panic. A Tauri Logs
regression runs the core pruner against an isolated temporary directory, then
confirms the active date is still discoverable and the complete 3,005-line
file is written byte-for-byte through the production full-export helper. This
proves local retention/export behavior only. ~~Desktop startup integration was reported open.~~
Source verification confirms the Tauri entry point initializes logging before pruning the
canonical daily log directory; isolated startup-order execution and cross-platform symlink
execution remain open under P4-55/TG4-29.

### 2026-10-02 continuation — persisted skill portability

A P4-16 audit found that saved skill loading used the executable-example
validator and therefore accepted `/tmp/` in persisted prose, unlike recipes.
The loader now applies the strict persisted-guidance validator to both skill
descriptions and templates. The new negative test passes with the existing
skill portability cases; the focused Tauri portability set passes 3/3, core
portability tests pass 8/8, and shipped recipe/skill default corpus tests pass
individually. TG4-17 remains open for the complete corpus and three-OS proof.

### 2026-10-02 continuation — Tauri PR-diff consent path

The PR-diff helper now has isolated tests for denied fallback and matching
consent reaching a local analyzer callback. The callback avoids vault access
and live provider construction; the Tauri command test set passes 3/3. A failed
first draft incorrectly assumed redirecting `HOME` also isolated the Windows
OS keychain. That test was not accepted, and ADR-LOG-RESTRUCT-0148 records the
retraction and correction. The actual Tauri IPC/UI workflow and provider-backed
allow/revoke consumer proof remain open under P4-60/TG4-16.

2026-10-02 UI continuation: `npm test -- --run src/tabs/Settings.test.tsx`
passes 1/1 and `npm run typecheck` passes. The consent card reads state and
issues grant/revoke commands with UI refresh; the test switches to a second
workspace and verifies its grant is not displayed. This mocks Tauri `invoke`,
so live desktop IPC remains open.

### 2026-10-02 continuation — P4 dispatch and ACP audit fixes

The independent recovery audit found that the Tauri status command kicked a
worker before writing its assignment. It now writes first, and
`ui_assignment_persists_before_recipe_runner_dispatch` runs the command helper
through an isolated scheduler and real `RecipeRunner`, observing the assigned
row at kick time and the runner reaching `idle_no_provider` without a provider
request. ACP output now says message IDs are persisted and awaiting
acknowledgement; a production ToolExecutor regression verifies an unbound
recipient remains replayable until ack. P4-04 is now proof-backed complete;
full ACP interruption side-effect idempotency remains open.

The cross-phase audit also reconfirmed two unrelated high-severity open gaps:
task checkpoint restore does not account for later process writes without a
checkpoint, and the P2 daily spend limit has fail-open read/write paths plus
concurrent-call overspend risk. Both remain in the remediation queue. P4 is not
ready for owner Calendar acceptance while its security, observability,
cross-platform, and successful CLI Fleet evidence remains incomplete.

### 2026-10-02 continuation — query-plan evidence

P4-58/TG4-20 now has eight populated-fixture query-plan assertions across the
dashboard daily-cost range, task claim/orphan scans, task-board point/status/
worker reads, latest worker tick, and worker-request status/time filtering. The
tests run `ANALYZE` and confirm the date primary-key index, task status/assignment
indexes, `idx_worker_run_state_worker`, and the measured worker-request
expression index without primary-table scans. The working inventory lists
remaining routing, run recovery, ACP, analytics, observability and diagnostic
queries in `_verify/P4-58-QUERY-INVENTORY.md`; P4-58 remains open.

P4-58 continuation 2026-10-02: a populated production-shaped worker-request
query selected a table scan and temporary sort. A measured status plus
`julianday(created_at), id` expression index changed the same plan to an indexed
search without the temporary sort; migration 10 and its production SQL plan
regression are now recorded in ADR-LOG-RESTRUCT-0152. A second populated fixture
proves task board point/status/status+worker reads use the rowid primary key and
existing task indexes. The query inventory also now names routing-history and
affinity reads missing from its prior draft.
P4-58/TG4-20 remain open until the remaining production queries have registered
plan proofs and the inventory is enforceable against new queries.

P4-58 continuation 2026-10-02: expanded populated production-SQL plan coverage
now includes task claims/orphans, task-board point/status/worker reads, ACP
recipient/task/recent/stale inbox and count reads, worker-request status
filtering, dashboard daily/token/worker/failure/event analytics, latest worker
tick, fleet roster/progress, assigned and ready-task dispatch, routing
history/affinity, exact-owner interrupted-claim selection, and open-run recovery. Analytics query
migration v3 adds measured `julianday(timestamp)` expression indexes; indexed
UTC range queries preserve canonical `T` and legacy space-separated timestamp
behavior. Migrations 11/12 add measured ready-dispatch and recent-routing
indexes. Local-midnight, recent failure cutoff, malformed timestamp, filtered
event, oldest-ready claim, and one-active-claim regressions pass. See ADRs
0154-0158 and `_verify/P4-58-QUERY-INVENTORY.md`. The diagnostics table-count
reader is now tied to the five-table migration schema and its production count
queries are EXPLAINed; worker summary queries have populated plan/result proof,
with a measured `date(created_at)` index for workers added today and explicit
full-count exceptions for exact roster totals. Both worker-request variants are
covered. P4-58 remains partial because the remaining in-scope reader plan/result
proofs and scope classifications are still open; the exact SQL fingerprint
registry itself now passes.
The latest build-evidence and QC-transition readers now share SQL constants;
a populated 512-record test confirms both use the existing task/id index, reads
the latest passing result, and allows the real QC transition.

P4-58 continuation 2026-10-02: the follow-up reader audit added populated plan/result coverage for task budget status, task unblock/blocker reads, epic completion aggregation, daily-cost point lookup, and worker-by-slug. The focused task budget/unblock, worker roster/slug, and daily-cost range/point tests pass (1/1 each). The measured task-unblock plan continued to choose the existing status index, so the proposed partial index migration was removed. Dashboard summary reuses one daily-cost read for both prior-day values. TG4-20 remains open for a full source audit and an enforceable query registry.

P4-11/P4-61 continuation 2026-10-02: the owner-approved exact non-inheriting ancestor traversal/read-attributes ACE shape passes its focused test. The production Clippy test still fails after 78.29 seconds: AppContainer metadata and file-byte reads work, but Rust `canonicalize()` is denied for the selected config paths. Rust/DOS volume resolution inside AppContainer is a separate OS namespace restriction; do not widen permissions. Later Windows production Python evidence supersedes the earlier host-fixture-only note: Arcana stages signed CPython 3.14.7 and pytest 9.1.1, and the production launcher passes compileall and pytest while denying a sibling secret, writes to the staged runtime, and loopback TCP. Python runtime staging is implemented on Windows; Ruff 0.16.8 now has a Windows x86-64 host-approved pinned-wheel staging path with a passing exact-member extraction fixture, while production approval/download/sandboxed-lint proof remains open; still open are the explicit user-owned fallback, project locked-dependency staging and denial consumption, full Rust/Node corpus, the Clippy canonicalization issue, and Linux/macOS proof.

Correction to the earlier Clippy ACL investigation: the owner-approved exact ancestor grants are implemented and their grant-shape test passes, but further filesystem ACL widening is not a viable fix for canonicalization. Rust documents `std::fs::canonicalize` on Windows as using `CreateFile` and `GetFinalPathNameByHandle`; a current Microsoft STL AppContainer reproduction shows DOS volume-name resolution is denied by the AppContainer/kernel namespace boundary even with broad file ACLs (https://github.com/microsoft/STL/issues/6286; https://doc.rust-lang.org/stable/std/fs/fn.canonicalize.html). The production Clippy probe reproduces the same denial. A separate workspace target-directory creation denial still needs isolation. The Windows build/test sandbox decision is pending the owner; do not widen permissions while it is pending.

## P4 update — 2026-10-02

The owner expanded TG4-20 to include shared CLI analytics, embedded reads in mutations, and explicit exceptions only with reasoned production-result/plan evidence. The AST registry now discovers all Rust files beneath the scoped core DB, analytics, and shared DB directories instead of relying on a seven-file source list; five settings, annotation, and schema-helper exclusions are named and test-checked in the inventory. The expanded registration suite passes 4/4. The shared CLI analytics readers and two newly audited embedded paths (deterministic assignment's active-task guard and worker-progress retention) have populated result/plan tests. The worker-progress baseline exposed a global event-table scan, corrected by measured runs migration 3. TG4-20 remains open for the other lifecycle reads and the final source/plan audit.

The owner also approved a signed App Sandbox XPC runner and sandbox-owned workspace staging/copy-back on macOS if direct descendant access fails. This still has no implementation or platform test. P4-61 remains open across every OS: Windows Python runtime staging has focused production Build/Test proof, while real Ruff approval/download/lint proof, Python fallback selection, locked-dependency staging, trustworthy violation consumption, and full OS sandbox proof remain outstanding. The only owner-reserved final acceptance is P4-49; its Calendar directory remains untouched.

### 2026-10-02 resumed P4-58 proof update

Independent GPT-6 Luna High review found two TG4-20 defects: unresolved
positional `execute_batch` SQL fragments could evade registration, and the
worker-run migration's UPDATE trigger lacked its own plan/result proof. The
scanner now fails closed for unresolved SQL, with one exact owner/template
exception for the fixed `VACUUM INTO '{escaped}'` backup command. Regressions
exercise the positional-fragment rejection and narrow backup exception. The
RunsDb populated regression now explains both insert/update guard predicates
against 300 rows and proves both rejection paths, including that a completed
candidate remains completed. Exact fingerprint rows carry a concrete proof or
bounded rationale, which the registry test cross-checks against discovered
owner evidence.

The independent reviewer found no further unpaired query variant after these
corrections. `cargo test -p arcana-core --test p4_query_registry --locked --
--nocapture` passes 8/8; the populated run-trigger regression passes 1/1; the
ACP replay regression passes 1/1 with two-message delivery/ack coverage. The
latest `cargo fmt --all -- --check`, `cargo check -p arcana-core --locked`,
and `git diff --check` pass. P4-58 and TG4-20 are closed. P4 remains active:
the owner Calendar scenario, successful live CLI Fleet execution, full
P4-50 Logs producer/readback coverage, P4-61 cross-platform confinement,
P4-62 remaining timeout/stage timing proofs, other incomplete gates, and
cross-phase audits are still outstanding. The full core-suite baseline remains
811 passed / 2 failed due to Windows AppContainer Clippy config canonicalization
denials; this scoped closeout did not rerun the full suite. No provider or
Calendar workspace was accessed; no commits or pushes were made.

### 2026-10-02 observability continuation

Graph parent timing now carries the same `trace_id` as its phase events. The
filesystem-only procedural Memory index refresh no longer identifies itself as
a SQLite operation. Logs queues a latest-filter refresh if the date/correlation
selection changes during a read, covered by a delayed-read regression. CLI log
tailing streams lines while retaining only the requested tail. The Logs suite
passes 13/13, TypeScript typecheck passes, OperationTimer tests pass 3/3, and
the 50,000-line CLI tail test passes. These local fixes do not close P4-50:
real Fleet producer-to-Logs readback, complete operation coverage, cost
measurement, and telemetry failure isolation remain open. The full core suite
is 809 passed / 2 failed; both failures remain Windows AppContainer Clippy
configuration canonicalization denials. No Calendar workspace or provider was
accessed for these checks.

Follow-up: failed graph phases now produce safe events with the same `trace_id`
as the parent graph timer; the Logs trace rows display phase names. Graph failure
telemetry test passes 1/1 and Logs suite passes 14/14. A 20,000-query fixture
measured 10.77 ms without profiling, 14.28 ms with the callback, and 483.36 ms
with JSON event formatting to a sink. SQLite's profile callback currently
uses millisecond time ticks, so only 2/20,000 simple statements had nonzero
duration. The review also caught and removed false SQLite labels from the
procedural Memory index read/write, topic listing, and topic read timers. This
fixture omits file appender I/O and does not establish Fleet latency. The owner
decision on a higher-resolution per-connection timer remains pending.

Logs-consumer cost sample: a production-helper regression on a 3.46 MB
synthetic JSONL file measured 404.90 ms initial metrics aggregation for 25,000
records, 19.56 ms to consume 1,000 appended records, 3.77 ms to read a
2,000-line tail, and 1.63 ms for a full-file read. These are single-run local
measurements, not UI/IPC or export-write timing; representative Fleet log cost
remains open.

Memory follow-up: `save_memory_md` contained a stale comment claiming it also
updated topic timestamps in SQLite. Source/caller audit shows the exported
command only writes the index file, and normal topic writes/deletes regenerate
that index. The comment now matches the live contract. MEMORY.md read/write
timers record successful byte counts without recording content or identifying
SQLite; the production timing JSON regression now verifies `result_bytes`.
The independent follow-up review also found that procedural search's timing
event defaulted to zero results and that Logs metrics did not classify graph
`partial` outcomes as degraded failures. The search path now emits matching
result/timing counts, verified by a JSON capture test (1/1), and a populated
partial graph event increments the metric failure count (1/1).
`cargo check -p arcana-core --locked`, the Logs UI suite (14/14), typecheck,
format, and diff checks pass after this correction. P4-50 still requires full
Memory/DB coverage and producer-to-Logs verification.

Latest full-suite retest (2026-10-02): `cargo test -p arcana-core --lib
--locked` completes 811 passed / 2 failed / 0 ignored in 369.49 seconds. Both
failures remain the documented Windows AppContainer denial when Clippy
canonicalizes its readable config path. The new procedural search observability
test passes in this run. This does not close P4-11/TG4-18 or P4-61/TG4-31.

Cross-phase regression found while validating P4: the Tauri suite's TG-P1-06
redaction guard expected `.await` to remain on the same source line as the
shared handler call. Production has both read and write partitions calling
`execute_handler`; the guard was formatting-sensitive rather than exposing a
redaction bypass. It now counts the shared call expression independently of
await line wrapping. The focused guard passes, and the full Tauri library suite
passes 149/149. The earlier run was 148/149; no production redaction behavior
changed.

The complete CLI test suite also passes: `cargo test -p arcanactl --locked`
reports 13 passed across CLI units, cross-process consent persistence, and
headless runtime integration. The isolated live CLI Fleet attempt remains a
provider-quota failure before decomposition, so there is still no successful
provider-backed CLI task completion.

### 2026-10-02 Memory observability follow-up

The Memory review found that aggregate `get_stats()` could report success even
when component counters had returned fallback values, prompt recall timing did
not carry the actual result count, and procedural topic listing silently
skipped unreadable entries. Aggregate stats now report `partial` when a
component read falls back; recall result and timing records carry matching
counts; and topic listing propagates directory-entry and file-read failures so
its timing event records an error rather than a partial-success-shaped result.
Embedding inference no longer carries a SQLite database label. Memory topic
listing and index refresh both propagate partial directory/file failures rather
than silently accepting incomplete content. The serialized Memory suite passes
66/66, including captured JSON checks for partial stats, recall count agreement,
invalid topic-file handling, prevention of a partial index write, and
database-label omission. P4-50/TG4-24
remain open for complete operation coverage and real producer-to-Logs UI
readback. At that 2026-10-02 checkpoint, the full core suite was 811 passed / 2 failed before this
patch; the two documented failures are the Windows AppContainer Clippy
canonicalization boundary. No provider call or Calendar access was made.

2026-10-02 observability continuation: OperationTimer now emits result-aware
DB events at shared task-status transaction boundaries, task self-claim, worker
tick begin/phase/finish, and worker heartbeat. Events include database identity,
duration, safe counts, and known run/task/worker/session correlation; empty
claims differ from errors. Independent review caught and corrected premature
task-transition success events by moving instrumentation outside the inner SQL
helper and marking success only after commit. Production capture tests cover
success, empty, missing-row, stale-owner, stale-routing, and missing-run cases.
The owner chose native millisecond SQLite statement timing with nanosecond
logical operation spans. TG4-24 remains open for broad producer coverage,
persisted real Fleet-to-Logs UI readback, telemetry failure isolation, and
representative cost evidence. The goal remains open; no provider call or
Calendar access occurred.

2026-10-02 run-correlation follow-up: `ToolExecutor` now carries its existing
worker run ID through `ToolContext` to task-status persistence. The shared
status transaction event therefore correlates task, worker, and run when those
values are known; compatibility methods still support callers without a run.
Captured production-method testing asserts the serialized run ID, and the
P4-58 query registry was reconciled to the run-aware methods that own the same
SQL. Focused status tests pass, registry 8/8, and core check/format/diff checks
pass. This closes one correlation gap only. P4-50/TG4-24 and P4 remain open.

2026-10-02 latest verification: full `cargo test -p arcana-core --lib --locked`
completed 823 passed / 2 failed / 0 ignored in 411.80 seconds. Both failures
match the previously documented Windows AppContainer Clippy config
canonicalization denial in the checked-in Rust corpus and dedicated Clippy
probe; this run did not reveal a new failure class. The new executor-context
and production task-status run-correlation tests passed. `npm run typecheck`
passed and the Logs panel suite passed 14/14. Broad Fleet-to-Logs readback,
remaining instrumentation coverage, telemetry failure isolation and cost proof
remain open. This is not P4 closure.

2026-10-03 P4-50 continuation: PlansDb CRUD and task assignment, assigned
claim, orphan release, interrupted-run quarantine, and dependency reconciliation
now emit result-aware spans with fixed database identity, nanosecond duration,
safe row counts, and available task/worker correlation. Production DB tests
capture success, empty, refusal, and foreign-key error events and verify
committed claim/quarantine state. PlansDb tests pass 4/4 and the lifecycle
capture test passes 1/1. The task lifecycle now wraps SQL in correlated tracing
spans. Tauri's correlation filter and Logs trace tree now read the flattened
span-field shape produced by the shipped JSON formatter, and a production
SQLite event is verified under its task trace. Logs panel tests pass 15/15;
the Tauri correlation-reader test passes 1/1. Other DB operations still need
correlated parent spans, and transient vector embedding unavailability can
still appear as successful empty recall. P4-50/TG4-24 remains open for those
gaps, remaining database/Memory coverage, real Fleet-to-Logs readback,
telemetry failure isolation, and representative producer/refresh/export cost
evidence. This is not gate or phase closure.

2026-10-03 P4-50 database continuation: AnnotationsDb create/list/read/update/resolve/delete now emit result-aware timings under parent spans, allowing profiled SQLite statements to inherit safe annotation/operation context. Production capture verifies success, empty read, injected DB failure, statement correlation, and source-path/prompt exclusion; the annotation module passes 2/2, `cargo check -p arcana-core --locked`, formatting, and diff checks pass. P4-50/TG4-24 remain open for the wider DB/Memory surface and real Fleet Logs readback.

2026-10-03 P4-50 Memory fallback: if vector inference becomes unavailable after semantic backend selection, recall now runs the scoped lexical search and reports degraded retrieval instead of returning a false successful empty result. The backend suite passes 9/9. The owner selected native millisecond SQLite statement samples with nanosecond operation spans; ADR-LOG-RESTRUCT-0186 records the measurement evidence. The optional embeddings feature was not enabled because its build downloads the ONNX runtime; its gated implementation still needs compilation evidence. TG4-24 remains open for real Fleet-to-Logs readback, broader DB/Memory coverage, failure isolation, representative timing/cost proof and successful CLI completion.

2026-10-03 analytics DB follow-up: the production token-usage insert and local-date reader now emit privacy-safe result spans. An isolated AnalyticsDb production-path test captures successful write/read and an injected constraint failure, confirms `database=analytics`, duration and count fields, and checks session/provider/model markers are absent (1/1). `cargo check -p arcana-core --locked`, format, and diff checks pass. This adds two instrumented database operations; TG4-24 remains open.

2026-10-03 worker database follow-up: `WorkersDb::list` and `update_status` now emit safe result-aware spans. The seeded production-method regression passes 1/1 and verifies roster names are not emitted. This is incremental TG4-24 coverage; other database/Memory operations and end-to-end CLI Fleet-to-Logs evidence remain open.

2026-10-03 RunsDb follow-up: latest worker tick, latest worker owner, and latest fleet tick-list readers now emit result-aware spans with database identity, duration/count, and worker correlation when available. The populated production-reader test passes 1/1 and confirms session data is excluded. An offline build of the optional embeddings feature failed because `aligned v0.4.3` is not cached; no network request occurred. This adds read-side observability but does not close TG4-24.

2026-10-03 P4-62 update: `production_timeout_guard_quarantines_claim_for_supervisor_reconciliation` proves a hard worker timeout persists an interrupted run, moves the exact task claim to Supervisor-visible `needs_info`, and prevents automatic worker reclaim. The ignored matched Windows Python Build measurement passed for one cold plus five warm launches using identical inputs after an independent review found and fixed copied `__pycache__` in the initial sample. Corrected cold runtime/sandbox/process/drain was 36,386/1,030/435/<1ms; warm medians 3,147/13/394/<1ms; warm total range 3,552–3,603ms. These are fixture-specific runtime/cache and sandbox setup measurements only. P4-62 remains open for matched evidence-persistence timing, provider-backed CLI timeout/shutdown smoke, cross-platform measurements, and repeated cold-profile variance.

2026-10-03 QC freshness follow-up: ProjectCommand now compares workspace input fingerprints before and after each selected build command against one batch baseline. A changed or unavailable sample removes workspace provenance from all observed exits, preserving their actual exit values while preventing QC use; hashing also confirms stable open-file identity (Windows volume/file index, Unix device/inode). The comparison test and production ProjectCommand/QC regression pass. A build-script race fixture exited 101 and is not proof. Independent Luna High review found a separate high-severity case where one passing project can authorize QC despite a sibling timeout/spawn failure. An additive batch-completeness record is pending owner direction; TG4-07 and P4-14 remain open. Mutation restored before post-hash and directory enumeration races remain unclosed.

2026-10-03 resumed verification: read the expanded P4 completion goal from the actual attachment under `C:\Users\razia\.codex\attachments\...` (the user-provided path omitted the `.codex` directory separator). Current resumed checks: core fingerprint comparison 1/1; production ProjectCommand/QC path 1/1; core `cargo check`, `cargo fmt --all -- --check`, and `git diff --check` pass; P8 host-commit tests 11/11 plus real S8-06 smoke 1/1; frontend typecheck passes and all frontend tests pass 49/49; CLI `arcanactl` suite 14/14 including Ready-epic deadline regression. The provider-backed CLI Fleet run remains blocked by the recorded Gemini free-tier 429, and was not retried. Independent Luna review found multi-project partial-build QC bypass pending owner decision on batch-completeness persistence. Therefore P4 is not ready for the owner's desktop Calendar test. No provider request or Calendar access occurred; no commit or push occurred.

2026-10-03 P8 reclose verification: the host `git_commit` unit suite passes 11/11, the real Bash/host-tool S8-06 smoke passes 1/1, and Tauri TG8-13 registration/worker consumer test passes 1/1. This re-verifies the P8-74/TG8-13/S8-06 behavior in the current working tree; commit/push and final regression review remain part of staged closeout.

2026-10-03 recovery-contract audit: a GPT-6 Luna High source audit found the current scheduler safely quarantines interrupted claims and returns reconciliation to the Supervisor but does not prove automatic redispatch (P4-36); QC inbox-only and stale-message tests invoke AgentEngine directly and do not prove scheduler→RecipeRunner→QC acknowledgement (P4-42); startup retry tests lack a production-run identity/exactly-once engine execution assertion (P4-52); durable continuity does not yet include completed operation results/cursor, continuation instructions, changed paths or Git snapshot, and Supervisor health lacks active-operation elapsed time, retry state, model/token usage and parent/child relations (P4-53/54). These are open P4 proof/implementation gaps; the audit recommendations are recorded in the MDP and checkpoint. No readiness upgrade is claimed.

2026-10-03 P4-42 follow-up (superseded by the closeout entry below): the first focused production scheduler/runner regression proved direct QC inbox acknowledgement but did not yet consume the Supervisor stale-message nudge through that path. The subsequent expanded test closed that gap; see the P4-42/TG4-33 closeout entry below.

2026-10-03 P4-52 production retry proof: the same isolated scheduler→RecipeRunner QC regression now injects one transient setup timeout before any provider/tool side effect. It completes with one durable worker-run row and exactly one AgentEngine workflow (four scripted provider requests). This is taskless evidence only; claimed-task/budget continuity, retry-event readback, post-side-effect recovery, and live-provider behavior remain open. TG4-26/P4-52 are not closed.

2026-10-03 P4-42/TG4-33 closeout: the isolated production-path regression now ages an inbox row, reads it with the Supervisor stale-message tool, persists a QC nudge with the production sender, and then runs the persisted QC worker through scheduler kick, RecipeRunner, AgentEngine and inbox tools. Both stable message IDs are acknowledged, the queue is checked, and an empty subsequent kick performs no provider request. The unbound-recipient path exposes that delivery is still pending until ack. `cargo test -p arcana-core --lib scheduler_kick_runs_persistent_qc_inbox_through_recipe_runner_and_acknowledges --locked -- --nocapture --test-threads=1` passes 1/1. P4-42/TG4-33 are proven. The test uses an isolated scripted provider and does not contact Gemini.

2026-10-03 recovery-contract alignment: P4-36 and S4-03 now follow the owner's P4-53 no-blind-replay contract. In-process recovery releases only exact-owner claims, persists the interrupted run, wakes the Supervisor, and holds uncertain work until side effects are reconciled; safe continuation and provider-backed recovery remain open. ADR-LOG-RESTRUCT-0190 records why unconditional redispatch is not an acceptable success criterion.

2026-10-03 P4-50 continuation: owner confirmed millisecond native SQLite statement samples plus nanosecond logical-operation spans. The embeddings feature now passes its locked build check after dependencies were fetched. Memory instrumentation tests pass 68/68 and annotation DB telemetry passes 1/1. A Logs regression fixes repeated same-name span merging (17/17 targeted; full frontend 50/50; frontend typecheck passes). Independent review confirms TG4-24 still lacks a successful CLI Fleet run, persisted production Fleet-to-Logs UI proof, external MCP tool-call outcome events, telemetry-storage failure isolation, and representative cost evidence. No Gemini call was made in this continuation.

2026-10-03 P4-52 retry correlation: a focused captured-event regression proves startup retry and recovery events retain the same durable worker run ID and correct attempt/outcome data (1/1). The durable run ID is passed into retry handling; retry events still need persisted Logs-reader readback, and P4-52 remains partial for task budget and post-side-effect recovery.

2026-10-03 expanded cross-phase audit refresh: GPT-6 Luna High reviewed current P1/P2/P3/P8 source. The audit confirms current P8 host commit controls but finds no model-produced AgentEngine→scheduler commit proof; P1 graph registration/current Arcana runtime attribution remain partial; P2 provider replay, full timezone/rollup matrix, stream/restart/estimate/kill-switch spend-cap evidence, and Linux vault fallback remain open; P3 consent revocation/live desktop IPC remain unproven. The P3 `search_memory` degraded-output gap now has a backwards-compatible detailed recall result and exposes redacted degradation status for both empty and nonempty fallback results; degraded search timing is classified as `fallback`. `cargo test -p arcana-core --lib memory::recall:: --locked -- --nocapture --test-threads=1` passes 10/10 and `cargo test -p arcana-core --lib search_memory --locked -- --nocapture --test-threads=1` passes 2/2; TG3-17 remains partial because active-backend absent/downloading behavior is not covered. These audit findings are tracked in `_verify/2026-10-03-CROSS-PHASE-AUDIT-REFRESH.md`; audit alone closes no gate. P4 remains active and not ready for the owner's Calendar test. No Calendar access, provider call, commit, or push occurred.

2026-10-03 follow-up: the real `SearchMemoryTool::execute()` entry point now has an isolated persisted-database regression alongside the injected empty/nonempty fallback tests. `cargo test -p arcana-core --lib search_memory --locked -- --nocapture --test-threads=1` passes 3/3; it confirms the production consumer returns a stored result and the active lexical-only degraded status together. TG3-17 remains partial for production absent/downloading transition proof. This does not change P4 readiness.

2026-10-03 P4 observability audit: source review confirms Arcana currently has no external MCP tool-call client or invocation consumer; it persists connector settings and performs HTTP health/reconnect probes only. TG4-24 explicitly requests tool-call outcome telemetry, so P4-50 remains open pending owner direction on introducing MCP invocation now or assigning invocation telemetry to the phase that builds that capability. The audit finding and source references are recorded in the MDP and testing-gate amendment logs.

2026-10-03 P4-59 continuation: an injected abort during the second monthly worker-stat rollup now proves the first bucket and pruning roll back together; a retry commits both old months and leaves the exact cutoff day in daily storage. The focused regression passes 1/1. TG4-21 remains open for production retention cadence, schema preservation, active-writer safety, full export readback, and the other append-only tables.

The production analytics manager now also has a startup-cadence test proving its scheduled job immediately applies `Local::now() - 90 days`, rolls older rows, retains the exact cutoff day, and stops cleanly (1/1). This closes the scheduled-consumer and cutoff calculation proof only; TG4-21 remains open for migration preservation, active-writer safety, full export and other append-only tables.

2026-10-03 P4-50 producer-to-reader proof: the real `OperationTimer` tracing event is persisted to JSONL and read by the Tauri structured-event and incremental metrics consumers; `production_operation_event_persists_and_reads_through_tauri_log_consumers` passes 1/1. Run/trace correlation, duration, result, and count survive. This does not prove live desktop IPC or Fleet readback. TG4-24 remains open for successful CLI Fleet completion, MCP invocation scope, telemetry failure isolation, full producer coverage, and representative costs.

The Logs reader cost fixture now approximates the earlier Fleet event-file size: 7,492,780 bytes across 25,000 structured DB events plus 1,000 appended rows. Three warm runs measured median full metrics refresh 559.4 ms, incremental refresh 25.3 ms, 2,000-event tail 7.49 ms, and full read/export 3.52 ms. This excludes Tauri IPC, React and concurrent disk load; do not treat it as end-to-end UI latency or use it to change the refresh cadence.

Memory observability follow-up: direct PA `get_memory` and `list_memories` calls emit correlated, content-free result spans; not-found reads record an observed zero; list truncation uses raw lookahead and reports malformed rows separately; procedural search separates malformed tag fields from dropped rows. A production detailed search still returns retrieved results when its best-effort access-history update fails, with the secondary failure recorded. Focused regressions pass; ADR-LOG-RESTRUCT-0193 records the contract. Broader DB/Memory coverage and full Fleet-to-Logs remain open.

2026-10-03 config.db observability follow-up: app settings, model configurations, and stored Arcana instructions now emit result-aware fixed-label DB spans. Test coverage proves zero/one row outcomes and an injected settings write failure, while keeping setting keys/values, model IDs, project paths, and instruction text out of events. App-settings tests pass 6/6; model-config and instruction tests pass 1/1 each. `search_all_detailed` failure-resilience coverage now traverses the public production API and records its fallback result when optional access-history persistence fails (1/1). ADR-LOG-RESTRUCT-0195 records the producer contract. P4-50 remains open for other producer families, telemetry failure isolation, live CLI Fleet completion, and UI readback.

`OperationTimer` now emits `result_count_observed` and `result_bytes_observed`, distinguishing a measured zero from an unmeasured field. Its focused JSON tests pass 4/4; ADR-LOG-RESTRUCT-0194 records the additive telemetry contract.

2026-10-03 continuation: build-evidence batch persistence/readback and routing-history readers/writers now emit safe `tasks` operation spans; HooksDb configuration and execution-log paths emit safe `config` spans. Focused production-method/privacy tests pass 1/1 each. Core check and formatting pass. TG4-24 remains open for the complete producer inventory, telemetry failure isolation, successful CLI Fleet execution, live desktop/Fleet-to-Logs readback, MCP scope resolution, and representative end-to-end costs. Owner decisions for P4-14 atomic batch completion, P4-53 separate operation journal, and P4-50 MCP invocation are recorded in the 2026-10-03 owner-decision section; implementation proof remains open.

2026-10-03 follow-up: WorkersDb CRUD and RunsDb lifecycle/recovery methods now emit safe, result-aware DB spans. Focused production-method tests pass 1/1 each, including worker content privacy, run recovery, missing records, and injected write failures. TG4-24 remains open; see the gate ledger for exact test evidence and the remaining CLI Fleet, live Logs, telemetry-isolation, and coverage requirements.

2026-10-03 follow-up: AnalyticsDb worker-stat insert/read and live activity/token aggregate reads now emit result-aware fixed-label spans; fallback results remain distinguishable while the public API retains its existing zero-value fallback contract. worker_stats_and_live_token_aggregates_emit_safe_result_spans passes 1/1, including missing rows, injected SQL failure and telemetry privacy checks. Workspace formatting, core check, and diff checks pass. TG4-24 remains open; no Calendar test, provider request, commit, or push occurred.

2026-10-03 follow-up: SessionStorage now times database open/migration/schema-version plus production session/message CRUD and reads, with fixed labels, safe session correlation, observed counts and explicit not-found outcomes. The isolated storage suite passes 3/3; its observability test exercises every production repository method and injects a SQL failure while checking content/path/provider/model/error privacy. TG4-24 remains open for the remaining database families, telemetry failure isolation, CLI Fleet completion, live Fleet-to-Logs readback and end-to-end cost evidence. No Calendar access or provider request occurred.
2026-10-03 follow-up: Every public GraphStore operation now emits a fixed-label result span, covering writes, reads, snapshots, FTS rebuild, startup and WAL flush. The graph-store tests pass 3/3 with populated/empty cases, not-found outcomes, persistence readback, and a trigger-injected batch failure; graph content, paths, query values, snapshot JSON and DB paths stay out of telemetry. TG4-24 remains open for other producer families, telemetry-failure isolation, real Fleet-to-Logs readback, CLI Fleet completion and end-to-end cost evidence.
2026-10-03 follow-up: QC reviewer claim and cost persistence paths now emit task-correlated, fixed-label result spans. The isolated production TasksDb regression passes 1/1 across claim, conflict, upgrade/list/release, missing-row, no-op cost and injected failure cases; reviewer IDs, task content and values stay out of telemetry. TG4-24 remains open for remaining task DB families and producer-to-Logs/Fleet evidence.
2026-10-03 follow-up: Epic and phased-task create/read/summary/ready selectors now emit fixed-label task DB spans. `epic_phase_operations_emit_private_result_spans` passes 1/1 against production TasksDb methods with populated and empty reads plus an injected creation failure; planning content and epic/phase identifiers remain private. TG4-24 remains open for other DB families and end-to-end Fleet-to-Logs proof.
2026-10-03 follow-up: TasksDb retry-count, diff write, single-delete and clear-board operations now emit safe result timings. The clear-board backup event reports bytes and fixed operation/reason without exposing its absolute path. Focused production-method tests pass 1/1 for retry/diff/delete (including injected SQL failure) and clear-board backup-first behavior/privacy. TG4-24 remains open for the rest of the database inventory, telemetry failure isolation, CLI Fleet completion, desktop Fleet-to-Logs readback, and representative end-to-end costs.
2026-10-03 follow-up: Token analytics now gives the model/provider/agent grouping queries and top-session query separate fixed-label child timings beneath the existing range span. The production capture regression passes 1/1 and confirms safe observed result counts while keeping user/provider/model values private. TG4-24 remains open for the rest of the database inventory and live Fleet-to-Logs evidence.
2026-10-03 follow-up: Shared SQLite schema migration now emits a fixed-label total-ladder timing and applied-rung count. The isolated production helper test passes 1/1 for successful changes, already-current/empty ladders, and SQL failure; unrecognized DB labels are normalized and paths/migration SQL/names are excluded from operation events. TG4-24 remains open for complete producer inventory and real Fleet-to-Logs proof.
2026-10-03 migration validation update: the full `db::migrate::tests` suite passes 15/15 after the migration span began preserving committed-rung count when a later rung fails; formatting check passes. This confirms bounded behavior and does not close TG4-24.
2026-10-03 follow-up: The provider spend ledger now emits transaction-level decision and settlement events without reservation IDs or dollar values. The orphan selector reports empty, found, and failed read outcomes separately, addressing the scheduler's prior ambiguous empty fallback. Isolated production-method tests pass 1/1 for each path, including injected persistence/query failures. TG4-24 remains open for other producer paths and live Fleet-to-Logs evidence.
2026-10-03 follow-up: File checkpoint operations now expose timing and result outcomes across create/auto-checkpoint, compare/list, restore and delete. The isolated production lifecycle test passes 1/1 with restored-content verification and confirms source/snapshot paths, source text, reasons and checkpoint IDs stay out of operation events. TG4-24 remains open for remaining producers and Fleet-to-Logs proof.
2026-10-03 follow-up: CLI analytics commands now emit operation-level result spans for cost, token, worker-stat and export reads at the existing CLI/core boundary; the shared query crate remains decoupled. The telemetry regression passes 1/1 for success, empty and error results while checking row/error privacy. TG4-24 remains open for other consumers and complete Fleet-to-Logs proof.

## P4 owner decisions and current resumed state (2026-10-03)

The owner resolved the open contracts: P4-14 requires a persisted atomic completion record for each selected project-command batch; P4-53 requires a dedicated append-only operation journal table in the existing `runs.db`, linked to run/task IDs and separate from bounded `worker_run_events`; P4-50/TG4-24 must add real shared-core MCP invocation over configured Streamable HTTP endpoints using `rmcp` 3.5.0, with immediate workspace-consent audit, fail-closed authorization, bounded/cancellable calls, no replay of ambiguous outcomes, and privacy-safe telemetry. These supersede earlier notes that requested owner direction.

The MDP currently has 59 explicit P4 tasks: 31 checked complete and 28 unchecked. The owner-only P4-49 desktop Calendar scenario remains the final acceptance; all engineering tasks, applicable gates, and the successful CLI Fleet workflow must be completed first. The 2026-10-03 cross-phase audit keeps remaining P1/P2/P3/P8 proof debt assigned to its owning gates rather than silently closing it under P4.

In the current continuation, the injected structured-writer failure test passes 1/1 and proves a production TasksDb mutation remains durable/readable while process health records the writer failure. The Logs UI test passes 18/18 and renders readable/structured writer-failure counters. This is one TG4-24 slice only. P4 remains active; no provider request, Calendar run, commit, or push has occurred.

2026-10-03 resumed implementation: `cargo check --workspace --locked` passes. Operation-journal ambiguity is preserved end to end: structured tool errors marked `outcome_unknown` now persist `Unknown`; Bash/project-command timeouts and post-dispatch MCP failures carry the marker. The regression passes 1/1. Supervisor journal pagination now keeps independent cursors per worker run (reader suite 6/6), and operation-journal regressions pass 6/6. MCP telemetry now records bounded invalid-input, authorization/connect, timeout, protocol/remote-error, and result-rejection outcomes; its local protocol/privacy suite passes 6/6, and scoped consent deny/grant/revoke/audit passes 1/1. Formatting and workspace Rust check pass. These do not provide a production reconciliation/resume action, AgentEngine MCP invocation readback, pre-parse HTTP body bounds, or TG4-24 closure. P4 remains active and P4-49 remains owner-only.

2026-10-03 MCP consent-race continuation: the actual RMCP Streamable HTTP fixture now grants workspace consent in an isolated SQLite DB, completes a second local-server handshake, revokes the persisted grant, and proves the final authorization gate prevents `tools/call` from reaching the server. Consent lookup and audit are committed together in an immediate SQLite transaction; its commit is the authorization decision point, so a later revoke does not retroactively cancel an already-authorized in-flight request. The focused fixture passes 1/1, the MCP module 6/6, consent tests 4/4, and `cargo check --workspace --locked` passes. Configured AgentEngine calls through desktop/CLI, oversized wire-response bounds, connector scope policy, remote cancellation, telemetry persistence/Logs readback, and successful CLI Fleet evidence remain open. No provider call or Calendar access occurred; no commit or push occurred.

2026-10-03 CLI continuation correction: two isolated production CLI Fleet attempts using Gemini 3.8 Flash and the updated `.env` key did make provider calls. With 32k and 100k Supervisor run-token caps, they stopped after 3 calls/43,871 reported tokens (~$0.0337) and 7 calls/106,667 tokens (~$0.0625), respectively; both returned `BUDGET_EXCEEDED_TOKENS` before decomposition, left the epic ready with zero children, and produced readable persisted logs/analytics. The 100k run exported a 948,715-byte log bundle. This proves CLI startup, provider attempts, budget-failure telemetry, and selected log/analytics readers; it does not prove a successful CLI coding task or end-to-end Fleet-to-Logs UI. The user already approved atomic conservative reservations for the separate daily USD ceiling (P2-21); TG2-17 provider/restart/estimate/kill-switch integration remains open. ~~Run-token usage is still accounted after response, so pre-dispatch run-token enforcement remains an engineering gap before further live-provider testing.~~ **Correction 2026-10-04:** that sentence was superseded by the later production `RunTokenBudgetProvider` wrapper. It calls Gemini `countTokens`, reserves the count plus maximum output in the shared run ledger before each generation, and refuses dispatch when the cap would be crossed; denial/preflight/stream-settlement tests pass 4/4, and a later CLI trial confirms its sixth request is denied before generation. Process-restart/reconciliation of an in-flight unknown provider outcome and successful end-to-end CLI completion remain separate open evidence. The isolated CLI profile and scratch project were used; Calendar was untouched. No commit or push occurred.

2026-10-03 P4-50 MCP cancellation: production `mcp_list_tools` and `mcp_call_tool` now emit one structured cancellation outcome when Arcana drops their in-flight future. The focused MCP module passes 8/8; normal results are not double-counted. This is Arcana-side cancellation telemetry only, not remote cancellation acknowledgement or desktop/CLI AgentEngine readback. Redirect denial and pre-parse body bounds remain open; the redirect client requires a pinned reqwest 0.13 alias matching RMCP's Streamable HTTP trait, and owner direction is pending.
2026-10-03 MCP AgentEngine proof: scripted provider traffic now drives the production FastDevelopment AgentEngine through configured MCP discovery and invocation; the local server sees each request, the model receives the result, and RunsDb contains successful operation outcomes. Focused test passes 1/1. Desktop/CLI host wiring and Logs readback remain open; no external provider was used.
2026-10-03 P4-15 concurrent build-failure accounting: a production TasksDb batch regression now proves two concurrent nonzero command exits count toward the three-exit diagnosis threshold, the third reaches it, and a passing batch resets it. The focused test passes 1/1; model-driven diagnosis and repair remain open.

2026-10-03 P4-15 retry-cap decision and implementation: after three consecutive observed nonzero `project_command` build exits, the worker must diagnose from safe evidence, stop, and report to the Supervisor; only the Supervisor decides whether another build may run. The owner selected an explicit audited grant for exactly one later Rust-owned build batch, bound to the task and assigned worker and consumed atomically. An observed pass resets the streak; spawn failures and timeouts do not count. The tool currently blocks unapproved later builds and stops launching children mid-batch at the threshold; the approval grant and integrated Supervisor decision flow are not implemented, so P4-15/TG4-08 remain open.

2026-10-03 run-token cap decision: before each Gemini generation attempt, Arcana will call `models.countTokens` with the exact serialized `GenerateContentRequest`, then reserve the returned input count plus `maxOutputTokens` in one shared atomic run ledger. The reservation covers stream, unary completion, retries, compaction, and child agents and settles against reported usage; unknown dispatched outcomes retain the conservative reservation. This is not implemented. Do not run provider generation calls until production-path enforcement and focused tests pass. See ADR-LOG-RESTRUCT-0232.

2026-10-03 Windows staging correction: the approved guarded stage/copy-back design must first prove a stage location that PSEC can traverse. Moving the Clippy fixture to an ordinary OS temporary directory still produced Windows error 5 during canonicalization; no stage implementation or green Clippy proof exists. See ADR-LOG-RESTRUCT-0229.

2026-10-03 P4-51 failure-contract continuation: permission denial, approval timeout, session cancellation, and a dropped approval listener now remain distinct stable executor errors, and none reaches the tool handler without approval. Failure-envelope sanitization also removes planted echoed command, prompt, source, and path values while retaining Arcana-generated command identity and call correlation. Focused tests pass (8 envelope/privacy, 15 executor lifecycle). P4-51 remains open for every-builtin/provider failure coverage and a broader privacy/retry audit; P4 overall remains open.

2026-10-03 decision reconciliation: earlier dated notes that said P4-14 batch completeness, P4-53 journal storage, or P4-50 MCP invocation were awaiting owner direction are superseded. The owner approved an atomic persisted project-command batch record, a separate append-only operation journal in `runs.db`, and real MCP invocation in P4. Those choices are implemented in part but remain open for production consumer and failure/recovery proof. The RMCP-compatible reqwest 0.13 direct dependency approval remains a separate unresolved implementation prerequisite for redirect denial. The owner selected conservative pre-dispatch reservation for the run-token cap; its reservation bound and integration across retries, compaction, and child agents remain unimplemented. The daily USD ceiling decision is separately atomic conservative reservation and must not be conflated with the run-token cap.


2026-10-03 P4-14 mixed-project production proof: `production_mixed_project_batch_requires_every_child_to_pass_before_qc` runs the real ProjectCommandTool over detected Cargo + nested Python projects using the production Windows sandbox and staged signed Python runtime. With valid Rust and invalid Python, both commands return observed exits (expected=2, observed=2, passing=1), the persisted batch is `failed`, and the real task-status tool refuses `qc_review`. After repairing Python, both projects pass (expected=2, observed=2, passing=2) and the real status tool accepts QC. First run: 1/1 passed in 103.55 seconds; post-format repeat: 1/1 passed in 73.29 seconds. This does not close P4-14/TG4-07 because multi-project timeout/spawn/cancel and cross-platform corpus coverage remain.

2026-10-03 P4-53 recovery barrier: task-attributed side-effect intent check-and-insert now runs under an immediate SQLite transaction; an unresolved earlier operation blocks the production ToolExecutor handler with `operation_reconciliation_required`, and a unique index permits one reconciliation decision per intent. Focused ToolExecutor retry, concurrent intent, reconciliation, and migration tests pass. This closes the task-attributed dispatch bypass only; full recovery remains open for a production Supervisor/host reconciliation workflow, Git/workspace snapshot comparison, complete mutation capture, and replacement-worker continuation without duplicate effects.

2026-10-03 validation update: affected journal health tests pass 6/6, build-evidence tests 9/9, the identity source audit 1/1, and the two task migration fixtures 1/1 each. Workspace Rust check and formatting pass. The latest full core rerun completed 914 tests: 911 passed, 2 failed, 1 ignored in 815.22 seconds. The two failures are the Windows AppContainer/Clippy config canonicalization denial; the seven earlier fixture failures now pass their focused groups. The goal remains active and P4-11/P4-61 remain open.

2026-10-03 Windows sandbox continuation: PSEC is now the production Windows launcher, dynamically selected only when the system exports and reports filesystem-deny support; it keeps `CreateProcessW`, explicit standard-handle capture, suspended confinement, and kill-on-close descendant cleanup. Focused Windows-target check passes. Production evidence on the current tree: staged Python compileall+pytest passes 1/1; selected MSVC Cargo build passes 1/1; `.cmd` shim output/exit capture passes 1/1; workspace-write/sibling-read/loopback-denial passes 1/1; PSEC serialization tests passed 2/2. Old production ACL mutations were removed. The host reports PSEC v1.0 support flags `0x3`, without the v1.1 enumeration-only traversal bit; production Clippy still fails because Windows returns OS error 5 when the confined process canonicalizes even the selected workspace root. No broader path grant was added. The owner approved guarded Windows workspace staging/copy-back; that implementation and full ecosystem/descendant/cancellation corpus proof remain open. The latest full-suite result after PSEC is 916 tests: 913 passed, 2 failed, 1 ignored; no full rerun has followed the PSEC change.

Post-format revalidation update: `cargo fmt --all -- --check` passes. The same production mixed-project regression passes again (1/1, 73.29 seconds) after formatting. Therefore both recorded test executions pass; P4-14/TG4-07 remain open for timeout/spawn/cancel cases and cross-platform production proof.

2026-10-03 resumed revalidation: `cargo check --workspace --locked`, `npx tsc --noEmit`, `cargo fmt --all -- --check`, and `git diff --check` pass. The production PSEC Clippy corpus test was rerun and still fails: the confined child reads the exact `.clippy.toml` bytes but `std::fs::canonicalize` returns Windows OS error 5 for the workspace directory and file. This host reports PSEC v1.0 without the v1.1 enumeration-only traversal flag. The three P4-53 regressions pass (atomic unresolved-intent barrier, one durable reconciliation decision, and production ToolExecutor withholding a replacement handler until explicit RetrySafe). Full frontend suite passes 51/51 after destination-specific consent action labels fixed an ambiguous accessible-button selection; TypeScript passes. Full core suite has not been rerun after PSEC and remains latest-known 911 passed / 2 failed / 1 ignored. No provider request, Calendar access, commit, or push occurred.

2026-10-03 full core suite after PSEC: `cargo test -p arcana-core --lib --locked -- --test-threads=1` completed 916 tests: 913 passed, 2 failed, 1 ignored in 480.13 seconds. The same two production Windows Clippy cases fail because `canonicalize` returns OS error 5 under the PSEC v1.0 traversal policy; every other core test passed. This replaces the earlier 914-test pre-PSEC result. The suite is red, so P4 is not closed.

2026-10-03 final validation slice: the CLI test run passes 15/15 (11 unit, consent CLI 1, shared-runtime integration 3); `arcanactl fleet --help` and `arcanactl fleet run --help` start successfully. `npm run build` passes; Vite reports existing large-chunk and mixed dynamic/static import warnings plus stale Browserslist data. Full frontend tests pass 51/51. The full core run is 913/916 green with only the two PSEC Clippy canonicalization failures. No service or test port was left running. The owner selected conservative pre-dispatch reservation for the strict run-token cap; its safe bound and integration across retries/compaction/child agents remain unimplemented. The owner also approved guarded Windows workspace staging/copy-back; that implementation remains open. No commit or push due to the red full core suite.

2026-10-03 continued proof: `git_commit::tests` includes protected-branch switching and production TasksDb claim revalidation after file-lock acquisition. `production_task_claim_is_rechecked_after_file_lock_wait` revokes a real claim through the production task API, then proves commit refusal with HEAD/index unchanged. `model_produced_commit_runs_through_recipe_runner_and_host_guard` drives the persisted shipped backend recipe through scheduler kick, RecipeRunner, AgentEngine and the registered host commit tool; task claim, exact-file commit, returned result and successful run-journal readback pass 1/1. Independent GPT-6 Luna review found no material gap; the host suite passes 15/15, Bash guard 1/1, S8-06 real-tool smoke 1/1 and Tauri registration gate 1/1. P8-74/TG8-13 are proven. Shared ToolExecutor normalization tests pass 8/8. P4 remains open for its recorded blockers.

2026-10-03 P4-62 timeout observability: project-command substage durations now survive Tokio timeout cancellation as fixed-label structured events correlated to tool call, task and session. The production timeout regression captures the incomplete process-wait event while verifying descendant writes stop; focused tests pass. This is a partial reliability/observability improvement only. P4 remains active with the previously recorded Windows sandbox, provider token-cap, Fleet completion, telemetry-reader, recovery, cross-phase audit and owner-only Calendar acceptance work still open.

2026-10-03 continuation: partial `append_file` write errors are now classified as unknown side effects, preserving the durable retry barrier; the injected partial-write test passes. Project-command evidence persistence now emits a separate stage timing event keyed by the Logs reader's canonical `call_id`; the event contract test and production timeout regression pass. These are bounded P4-53/P4-62 fixes only. The Windows Clippy/sandbox failures, production reconciliation and continuation, CLI Fleet run/readback, cross-phase audits, and owner-run Calendar acceptance remain open.

2026-10-03 correlated Logs reader: the Tauri correlation-filtered structured JSONL reader now keeps a per-file byte-offset index, incrementally indexes appended complete records, holds incomplete trailing bytes until newline, seeks only matching recent rows, and prunes entries for deleted log files. Its seven focused tests pass, including correlation through spans/task IDs, recent-row ordering, partial append, file replacement/truncation, deleted-file pruning, and scoped export; formatting passes. Invalidation detects file identity change, truncation, and observed same-size mtime change. It assumes Arcana's append-only production writer and cannot detect arbitrary same-size rewrites preserving identity and mtime. This is not TG4-24 completion; live Fleet-to-Logs proof, complete producer/database/Memory coverage, failure isolation, token/result reconciliation, cost evidence, and successful CLI Fleet coding remain open.

2026-10-03 continuation on P4-15 and run-token reservations: the user approved one Supervisor-authorized build batch after three observed failures. Approval is now tied to the exact latest failed/incomplete batch, persisted with task/worker/Supervisor/reason/time, protected against direct SQL forgery by connection-scoped host authorization, and consumed atomically before execution. DB and real ProjectCommandTool tests pass; provider-driven Supervisor flow, concurrent-consumer/reopen proof and mixed-project approved-batch proof remain open. Gemini countTokens now reads a maximum 16 KiB incrementally; stream reservations wait for a nonzero usage frame, persisted task usage is applied before reservation, and denied task reservations use the existing needs-info escalation. Focused provider (4/4), task-activation (1/1), and production build-approval (1/1) tests pass; oversized Google response and full regression suites are pending. No live Gemini request or Calendar test occurred. P4 remains open.

2026-10-03 verification update: all three Gemini countTokens local fixtures pass, including bounded streaming rejection; full Rust workspace check, formatting and diff whitespace checks pass. Full `arcana-core` suite completes 944 tests: 941 passed, 2 failed, 1 ignored in 505.58s. Both failures are the same Windows PSEC/Clippy canonicalization denial (OS error 5); migration and provider-backed workflow fixtures pass after correcting their test setup for migration v20 and capped-provider preflight. No live Gemini request or Calendar access occurred. P4 remains open for PSEC staging, CLI Fleet run/readback, remaining gates, and the owner's final Calendar acceptance.

2026-10-03 P4-43 scheduler observability: the centralized production work-availability check records a fixed-label duration and safe `work`/`idle`/error outcome, and the shared dispatch helper records idle/already-running/dispatched outcomes plus full run duration. Both focused tests pass 1/1; empty Supervisor/QC queues still skip provider resolution. Physical provider-call counts and measured startup/event-kick/cron bounds remain open.

2026-10-03 P4-15 review status: the user-selected rule is to stop after the third-failure diagnosis and let the Supervisor decide. The current host cap safely refuses later build calls and stops remaining batch children at the threshold, but a fresh review identified that no Supervisor approval mechanism can reopen the build capability; existing Supervisor policy leaves exhausted work for manual review. An explicit authorization-path decision is pending, so P4-15/TG4-08 remain open and no approval behavior is invented.

2026-10-03 continuation: the user approved exactly one Supervisor-authorized build batch after diagnosis. Arcana now creates a typed task/worker-bound diagnosis request, lets only the Supervisor grant the audited one-use capability with a reason, consumes the grant atomically with batch creation, and revokes unused grants if task owner/status changes. The real ProjectCommandTool regression proves denial → approval → one additional build → reuse denial. Gemini `countTokens` preflight now shares the exact generation request and conservatively reserves input plus maximum output in an atomic run ledger; the ledger is passed through provider attempts and fleet child spawn paths. Focused provider, ledger, database, and tool-path tests pass. Full engine/CLI Fleet proof, provider-driven Supervisor reasoning, Windows guarded staging, full suite, and remaining P4 gates are still open.

2026-10-03 continuation: fixed the Gemini countTokens 400 by adding the required nested `generateContentRequest.model = models/{id}`. Local provider request/error-privacy fixtures pass 4/4; a live synthetic production-shaped count request returned 200. The CLI Fleet smoke now passes preflight and performs five successful Gemini Supervisor generations, then refuses the next request before dispatch when a 23,998-token reservation would cross its 100,000-token run cap (projected 100,266). No child task was created and the disposable arithmetic fixture was unchanged. This is useful production-path failure-boundary and log evidence, but not successful end-to-end Fleet coding; no Calendar workspace was touched. Do not raise the cap or repeat paid CLI work without a newly bounded decision. P4 remains active and incomplete.

2026-10-03 bounded CLI retry: after countTokens was corrected, a 150,000-token/$0.25 run reached the key's Gemini free-tier five-requests-per-minute generation quota. ~~Five Supervisor generations succeeded~~ Six Supervisor generations succeeded, but they only read board/worker state; the following request received `RESOURCE_EXHAUSTED` before any child task was created. The repeated prompt prefix also missed cache (9,885 tokens, zero cache reads). The code fixture remained unchanged. This identifies a real orchestration/latency/caching limitation; successful CLI Fleet coding is still unproven, and no further live call was made.

2026-10-03 observability readback: the live CLI run's logs and analytics were read back through Arcana CLI (`logs list`, `logs export`, `analytics tokens`). The JSONL held 1,874 structured events and exported to a 1,030,279-byte file; analytics reported 91,196 tokens and $0.0619. Counting the full persisted event file corrected the truncated-console summary: six Gemini generations succeeded and the next attempt failed on quota. This remains failure-path/CLI reader proof only; no child task was created, the fixture stayed unchanged, and the desktop Logs UI/calendar acceptance remain open.

2026-10-03 P4-15 follow-up: a scripted provider now exercises the persisted Supervisor recipe through the production RecipeRunner/AgentEngine, reads a typed build diagnosis, and invokes the actual one-attempt approval tool. The test reopens TasksDb and races two batch consumers; one and only one consumes the grant, with no fabricated exit. This closes the provider-decision and concurrent/reopen proof slices; P4 remains open for mixed-project approved execution, full CLI Fleet completion and other gates listed above.

2026-10-03 P4-15 approved mixed-project proof: the Windows production `production_mixed_batch_stops_after_third_observed_failure` test now continues through diagnosis and the real one-attempt Supervisor approval. It first proves the unapproved third observed Cargo failure prevents the later Python child from launching; after approval and a fixture repair, one complete Cargo+Python batch passes and the real QC transition succeeds. Focused test passes 1/1 (98.12s). This proves the approved mixed path on Windows; cross-platform execution and successful CLI Fleet completion remain open.

2026-10-03 P4-53 Supervisor recovery decision proof: the production shipped Supervisor recipe was driven through `RecipeRunner` and `AgentEngine` with an isolated scripted provider. It read the real task board, pending requests, ACP inbox, health journal, and exact quarantined task before invoking `reconcile_worker_operation` for that run/cursor. The test reopens RunsDb and TasksDb and confirms the unknown operation remains `do_not_retry`, with Supervisor actor/reason persisted and the task still held behind the retry barrier. `shipped_supervisor_recipe_keeps_uncertain_operation_quarantined` passes 1/1. RunsDb's temporary SQLite authorization now resets through RAII on errors and unwinding. This is fail-safe recovery-decision/persistence proof, not proof that a mutation was verified complete or safe to repeat. Repository snapshot/Git/checkpoint reconciliation, full mutation capture, safe replacement-worker continuation and no-duplicate resume remain open; TG4-27 and P4-53 are not closed. No live provider, CLI Fleet call, Calendar test, commit, or push occurred.

2026-10-03 P4-14 cancellation guard: production `ProjectCommandTool` now holds a drop guard for each persisted in-flight build batch. Cancellation/unwind finalizes only previously observed exits and marks an unobserved remainder incomplete, preventing an indefinitely `running` QC record. The focused isolated-DB drop test passes 1/1 and verifies one failed observed exit is counted while the missing selected child is not fabricated. This does not yet prove aborting the real command future stops descendants before finalization; P4-14/TG4-07 and the cross-platform process corpus remain open.

2026-10-03 P4-14 production cancellation proof: `cancelling_production_project_command_stops_descendant_and_closes_batch` aborts the actual Windows-sandboxed ProjectCommandTool while a build script's descendant writes a workspace heartbeat. After cancellation, the heartbeat stops, reopened TasksDb shows the expected one-child batch as incomplete with zero observed exits, the failure streak remains unchanged, and QC is refused (1/1, 17.64s). This upgrades the earlier guard-only evidence to real production cancellation ordering on Windows. Multi-project partial cancellation, timeout/spawn matrix, and cross-platform launchers remain open; P4-14/TG4-07 are not closed.

2026-10-04 P4-14 mixed timeout proof: the Windows production launcher now runs Cargo plus nested Python in one batch, lets Cargo exceed its bounded deadline, and observes Python compileall pass. The durable record remains expected=2/observed=1/passing=1 and `incomplete`; it invents no Cargo exit, does not change the nonzero-exit streak, and the real QC transition refuses the task. `production_mixed_batch_timeout_cannot_be_hidden_by_passing_sibling` passes 1/1 (63.48s). The earlier five-second attempt timed out both children during cold Python staging and is not counted as evidence. Multi-project cancellation, spawn failures, and Linux/macOS launchers remain open.

2026-10-04 P4-61 Windows bridge decision: the owner approved an Arcana-owned in-sandbox bridge for the experimental Windows sandbox API. The host exports the API and a prior disposable SBOX probe showed workspace-write plus sibling-read and loopback denial, but neither establishes production integration. The Rust bridge, packaged CLI/Tauri early dispatch, output bounds, process-tree cancellation, workspace staging/copy-back, supported-version matrix, complete Windows toolchain corpus, and Linux/macOS production runners remain open. No readiness status changed.
# 2026-10-04 Windows sandbox runner evidence

The Windows runner now calls Microsoft's experimental SBOX API with handle inheritance disabled and uses Arcana's CLI binary as a bounded-output bridge. On the current Windows 11 build 26200 machine, 13 focused process-sandbox tests pass, including workspace write, sibling read/write and loopback denial, stdout/stderr capture, a direct Clippy-config file-read probe, staged Python, `.cmd`, timeout and descendant cleanup. SBOX schema tests pass 2/2. CLI and Tauri targets compile and `arcanactl` builds. This improves P4-61 but does not close it: guarded stage/copy-back, locked dependency staging, packaged Tauri smoke, full core rerun, Linux production sandbox proof, signed macOS XPC/staging and the remaining expanded-goal audits/CLI Fleet acceptance remain open. The Calendar UI run remains reserved for the owner after these are clear.

2026-10-04 validation correction: ~~the focused SBOX tests prove production Clippy config access~~ They prove only a direct config-file read; the production Cargo Clippy path still fails to canonicalize the selected workspace/config path (Windows OS error 5). `cargo fmt --all -- --check` and `cargo check -p arcana-core --locked` pass, as does the isolated 13-test Windows sandbox suite. A full 956-test core run was interrupted after the staged-Python isolation test stayed CPU-bound for more than ten minutes. A per-launch ancestor-ACL experiment failed to resolve Clippy and was removed after a 185-second run. No phase or gate is closed; the owner Calendar run remains last.

2026-10-04 SBOX path-resolution diagnostic: a disposable child with an exact read-only stage grant could read `.clippy.toml` and obtain metadata, but Rust `Path::canonicalize()` returned OS error 5. `GetFinalPathNameByHandleW` in DOS-volume mode also returned error 5; NT-volume mode produced a `\Device\HarddiskVolume...` path that Rust could not reopen, and `\\?\GLOBALROOT` metadata worked while canonicalization still failed. This reproduces the production Cargo Clippy limitation outside the repository as well. Do not mark stage/copy-back proven or relax read/network policy; P4-11/P4-61 and TG4-18/TG4-31 remain open pending a supported path-resolution/backend solution.

2026-10-04 P4-62 observability follow-up: Python runtime preparation now has bounded phase timings for validation, inspection, fingerprints, copies, cache checks and runtime probes; the timing contract test and core check pass. The isolated Windows production sandbox test was stopped after ten minutes with staged files present but no Python child or completed result. This is not proof of Python isolation, phase measurements, or a gate pass; the post-staging stall remains open.

2026-10-04 P4-62 responsiveness follow-up: Windows sandbox setup now runs off the async executor with at most four setup jobs in flight. Focused cancellation, ProjectCommand timeout, 13 Windows launcher tests, and CLI/Tauri compilation pass. A blocked OS setup call itself cannot be interrupted, and the cold Python isolation test has not been rerun; P4 and TG4-32 remain open.

2026-10-04 full core verification update: after correcting the staged-Python stdout/stderr test deadlock, the complete `arcana-core` library run finishes: 959 passed, 2 failed, 1 ignored (962 total; 645.51s). Both failures are the same Windows sandbox limitation in real Cargo Clippy path canonicalization; a narrowly staged read-only config root did not change the result. Two additional test defects discovered by the first full run were repaired and their focused tests pass. No P4 task or TG4 gate closes; current totals remain 32/59 and 10/33. Keep the Windows Clippy/corpus, guarded staging, CLI Fleet, full observability, recovery, platform, and owner Calendar acceptance work open.

2026-10-04 cross-stack validation: `cargo check --workspace --locked`, `cargo fmt --all -- --check`, `npx tsc --noEmit`, `npm test -- --run` (52/52 across 12 files), and `git diff --check` pass. The full core run still has the two production Clippy canonicalization failures; all P4 totals remain unchanged at 32/59 tasks and 10/33 TG4 gates. No background test server/listener remains.

2026-10-04 P4 continuation: the exact non-inheriting ancestor traversal/read-attributes ACL probe failed to make Windows SBOX Rust workspace canonicalization succeed (`workspace_resolves=false`) while sibling reads stayed denied; temporary test seam removed. The Windows sandbox regression group then passed 16/16, CLI and desktop targets compile, and `arcanactl` tests pass 15/15. An isolated `arcanactl fleet run` preflight used Google/Gemini 3.8 Flash with a $0.01 cap and a test-owned home; it stopped for missing workspace egress consent before creating `tasks.db`, with no provider request. A focused live-smoke guard compares the vault's Google credential to `.env` `GOOGLE_API_KEY`; it passes 1/1 and prints neither secret. Successful paid CLI Fleet completion, full core (still two Clippy canonicalization failures), P4 observability/recovery/platform work, and the owner-only Calendar test remain open. Counts stay 32/59 P4 and 10/33 TG4; no task/gate changed.

2026-10-04 bounded bridge capture: with the owner's approval for Arcana's in-sandbox SBOX bridge, added a regression proving it retains only the newest 256 KiB while preserving the full observed byte count. The bridge-focused suite passes 5/5. No P4 task or gate closes; the full core suite remains 959 passed, 2 failed, 1 ignored due to the two production Windows Cargo Clippy canonicalization failures. Counts remain 32/59 P4 and 10/33 TG4.

2026-10-04 cross-phase cancellation follow-up: parallel read-tool handlers now race their production execution against the session cancellation signal. A blocked read is dropped and returns a correlated structured cancellation result; its active-tool entry is removed. The focused ToolExecutor suite passes 17/17. This is one P4-51/TG4-25 cancellation slice only; no P4 task or gate closes, and P4 remains 32/59 with TG4 10/33.

2026-10-04 observability follow-up: readable-log redaction now spans split writes within a bounded 256 KiB record, active-file retention outranks future-dated logs, and the shared daily-log prune runs from both Tauri and CLI startup. Focused readable writer tests pass 3/3, retention tests 6/6, and a real isolated CLI startup test passes 1/1. These tests do not close P4-50/P4-59 or TG4-24/TG4-21; MCP pre-parse total response bounds, four database-table retention consumers, full Supervisor health fields, live Fleet-to-Logs UI proof and the remaining source audit stay open. P4 remains 32/59 and TG4 10/33.

2026-10-04 continuation: the owner confirmed the Arcana-owned Windows sandbox bridge. The bounded bridge implementation and its tests were already present; the focused bridge group passes 5/5, including 256 KiB tail capture with total byte counts and real workspace/output/sibling/network sandbox probes. This does not solve the full production Clippy canonicalization failures or guarded stage/copy-back.

P4-54 health continuation: `list_worker_health` now reads exact per-session request/token/cost aggregates through the indexed production AnalyticsDb reader and returns them without exposing session IDs or per-request content. The populated 502-session query-plan/readback test passes 1/1, and the real isolated Supervisor health tool suite passes 9/9 with session-isolation and privacy assertions. Worker parent/child links and safe continuation/reassignment remain open.

P4-58/TG4-20 correction: the new session-token query fingerprint is registered with populated index-plan/readback evidence. Re-running the AST registry then found 22 additional missing production fingerprints across RunsDb and TasksDb and one stale TasksDb fingerprint. The P4-58/TG4-20 checkbox was reopened because the current checker fails. Live counts are now P4 31/59 and TG4 9/33; the earlier 32/59 and 10/33 tallies are superseded for current status. No commit or push has occurred.


2026-10-04 P4-58/TG4-20 restored: the prior current-tree recheck count was off by one; the executable AST registry listed 23 missing (owner, fingerprint) pairs and one stale QC pair. All 23 now have plan/result or trigger behavior references, and the obsolete pair is removed. RunsDb migration 7 adds the measured partial task-intent index; the v6-to-v7 populated upgrade/readback test passes. The full registry passes 8/8, and focused RunsDb/TasksDb plan and behavior tests pass. P4-58 and TG4-20 are checked again. At that historical scan, the task denominator had not yet been reconciled; the subsequent current-tree scan recorded 59 P4 tasks and 32 checked. TG4 has 10/33 checked. This restores the tracker state before the temporary reopen; it is not a net P4 checklist increase over the 32/59 checkpoint. The full core suite remains red on two Windows Clippy path-canonicalization failures, and CLI Fleet plus owner Calendar acceptance remain open.

## Current checkpoint (2026-10-04)

P4 remains active and incomplete at 33/59 tracked tasks with TG4 11/33. Recent verified implementation commits pushed to `main` are `a14cf6f` (Memory recency), `161283e` (retention query inventory), `6fd6926` (phase checkpoint), `d2f833b` (semantic Memory chronology), `e4fd313` (worker communication timestamp readback), `dc8fcc2` (build-evidence timestamp readback), and `2cc7bd6` (Linux fail-closed Ruff/sandbox test portability). The P4-58 query registry passes 8/8; focused Windows retention and timestamp tests pass. WSL Ubuntu has user-scoped Rust 1.99 and native build prerequisites; Linux retention passes 19/19, the selected production timestamp readers pass 9/9, and focused Landlock, structured Ruff-refusal, and timestamp-helper property tests pass. Linux/macOS broader sandbox and production-corpus evidence remains open. P4-57/TG4-19 remains partial; P4-59/TG4-21 remains open pending macOS matrix evidence and broader proof. The full core suite is still red at the 2026-10-04 baseline (959 passed, 2 Windows Clippy canonicalization failures, 1 ignored). Three isolated CLI runs with the current verified Gemini key recorded production consent and telemetry behavior but all stopped before child tasks because conservative reservation or provider quota rejected decomposition; no later provider attempt is warranted until quota availability changes. The owner Calendar acceptance remains unrun. Read the expanded P4 goal after core P4 closes.

2026-10-04 P4-55/TG4-29 closed after real desktop startup/retention/export/shutdown proof and the owner-approved concurrent migration fix (ADR-0276). Core P4 is 33/59; TG4 is 11/33. Next: remaining core P4 work, including the discovered fixed-label JSONL redaction defect, cross-platform workflow proof, sandbox/copy-back, successful CLI Fleet and recovery/observability contracts. Expanded goal remains unread and owner Calendar remains unrun.

## Owner pause — 2026-10-05

Goal is PAUSED. Resume core P4 from the final Paused checkpoint section of _verify/P4-SESSION-CHECKPOINT-2026-10-05.md. Current tally 38/59 tasks, 17/33 TG4 gates; the latest desktop consent proof is partial and dirty, with the first fresh-profile WebDriver session still failing. No new commit/push in that slice. Pending owner decisions remain pending; no live API spend or Calendar access. Expanded goal remains unread until core engineering closure.

2026-10-06 P4-51 verified slice (ADR-0327/0328/0329): malformed streamed arguments now fail before handler dispatch, while valid empty calls still run. Provider stream recovery is recorded only after full consumption; terminal/body failures produce one privacy-safe analytics aggregate and no retry after output. Windows/Linux pass engine26/26, production executor18/18, external MCP9/9 and provider129/129; the thirteen-tool file/task/epic/budget matrix plus separate two-tool MCP success/revocation fixture is verified against the actual47-name registry. Seventeen concrete builtin failures reach the model as correlated failed results. The additional ACP9, artifact3 and board6 registered-tool matrices pass on Windows/Linux, bringing deterministic success/failure coverage to33 of47 registered tools. The ACP negative fixture reproduced raw SQLite diagnostic leakage; ADR-0331 removes it from send_worker_message without changing delivery semantics. The surrounding ACP module passes5/5 on Windows/Linux (2.47s/4.44s). Core cargo check and formatting pass; every-builtin/error-category coverage remains open. Owner-approved ADR0332 now preserves tool JSON scalar types, redacts string values with the existing text policy and masks sensitive fields. Windows/Linux redaction22/22, Memory1/1, routing1/1 and executor18/18 pass through the real registered tools and readers. These five registrations raise deterministic success/failure coverage to38/47. Proof: `_verify/P4-51-JSON-REDACTION-2026-10-09.md`. Evidence: `_verify/P4-51-REMAINING-CONTRACT-2026-10-06.md`, `_verify/P4-51-BUILTIN-EXECUTOR-MATRIX-2026-10-06.md`, `_verify/P4-51-BOUNDARY-REVIEW-2026-10-06.md`. No task/gate/readiness closure or rescore: P4 40/59, TG4 19/33. No paid run or owner Calendar activity; expanded goal remains unread.


2026-10-09 P4-39 closes on its named autonomous-completion contract. Actual saved Supervisor/worker loops complete ordinary dependency work and explicitly resolved budget work, while unresolved budget siblings remain held. Supervisor-authored resolution is immutable, cumulative-usage preserving and distinct from lifecycle release; retained audit has stable UUID/numeric provenance and an explicit registered deleted-history reader. Windows/Linux pass TasksDb70/70, audit readers2/2, Fleet1/1, installed guidance1/1 and SQL registry8/8; core/CLI/Tauri checks pass. Proof: `_verify/P4-39-CLOSEOUT-2026-10-09.md`. Current core tally41/59, TG4 19/33;18 tasks remain including owner Calendar. P4-50/live CLI, full observability/security/platform gates and expanded goal remain open.

2026-10-09 P4-51 planner consumer proof: the actual FastDevelopment registration runs through ToolExecutor, creates a persisted plan and reads it with getPlan; malformed YAML and an empty smoke command produce bounded correlated failures without reflecting planted inputs. Windows1/1 (0.53s), Linux1/1 (0.84s). Coverage is now39/47 registered tools. Probe success and eight remaining registrations remain open; this does not close P4-51/TG4-25 or change readiness scores. Evidence: `_verify/P4-51-PROBE-PLANNER-MATRIX-2026-10-09.md`. Current overall P4 remains41/59 and TG419/33.


2026-10-09 P4-57 annual worker-history proof: the existing 4,400-instant leap-year/DST writer/readback corpus now verifies every heartbeat and count after production monthly rollup, including the legacy row (4,401 rows pruned). Windows1/1 (37.36s), Linux1/1 (90.30s); the existing CI selector includes the stronger fixture. P4-57/TG4-19 remain open for other timestamp pairs and complete target-host evidence. Evidence: `_verify/P4-57-ANNUAL-WORKER-ROLLUP-2026-10-09.md`.


2026-10-09 P4-62 registered command/log consumer proof (ADR-0343): ToolExecutor runs the actual FastDevelopment project_command for a claimed Python build; TasksDb reads a complete passing batch, and the real CLI Logs export reads exactly five successful stage durations under a stable derived telemetry UUID. Raw compound call IDs are absent, original provider/tool-result IDs are unchanged, and missing/wrong-owner claim refusals emit no stage rows. Windows1/1 (111.34s), Linux1/1 (13.65s); logging writer privacy/identity suite10/10 on both. These are whole-fixture times, not compiler measurements. P4-62/TG4-32 remain open for the actual selected-timeout path, finite CLI shutdown, matched platform measurements and macOS execution. Evidence: `_verify/P4-62-COMMAND-LOGS-READER-2026-10-09.md`.


2026-10-09 P4-51 native execution proof (ADR0346): the actual registered spawn_subagent, spawn_parallel_subagents and spawn_qc_review run through ToolExecutor, real manager and AgentEngine children with an explicit cfg(test)-only scripted provider. Windows1/1 (2.19s), Linux1/1 (2.69s) prove returned token/result readback and QC get_task consumer, persisted reviewer cost and released claim. Shipped Vault/provider selection is unchanged; synthetic QC build evidence is test input, not compiler proof. Coverage reaches42/47 registered tools; five execution registrations and the remaining error-category/privacy matrix keep P4-51/TG4-25 open. Independent Luna High source review reports no actionable defect. Evidence: _verify/P4-51-NATIVE-SUCCESS-2026-10-09.md. Core tally stays41/59 and TG419/33; no live paid run or Calendar activity.


2026-10-09 P4-62 selected timeout proof: actual registered project_command through ToolExecutor accepts timeout_secs=30, returns operation_timed_out, stops a real descendant heartbeat and yields correlated incomplete process_wait/timeout events through JSONL and the shipped CLI Logs export. Windows1/1 (115.83s fixture;30041ms command), Linux1/1 (34.09s fixture;30008ms command); complete passing build/batch/five-stage readback and invalid-claim controls remain in the same fixture. ADR0347 repairs isolated Linux venv/pytest; ADR0348 records WSL DrvFS capture-file incompatibility and uses native Linux temporary storage without relaxing policy. Evidence: _verify/P4-62-SELECTED-TIMEOUT-2026-10-09.md. P4-62/TG4-32 remain open for finite CLI shutdown, matched platform measurements and macOS. Tally remains41/59 and19/33.

2026-10-09 P4-51 registered execution matrix: actual FastDevelopment Bash, GitCommit and AnalyzeDiff registrations pass through ToolExecutor on Windows/Linux. Bash one-call approval, observed nonzero exit and denial preserve private inputs; Git commits only the named file and refuses a pre-staged sibling without changing HEAD/index; AnalyzeDiff persists deterministic analysis and TasksDb reads it back. Corrected fixture prerequisites use real RunsDb worker runs and reload the separate libgit2 observer's cached index. Windows shell/git2/2 (2.12s), diff1/1 (0.91s); Linux2/2 (2.02s), diff1/1 (1.00s). Registered ProjectCommand passing build/selected timeout/private-envelope/descendant/CLI log consumer passes Windows1/1 (83.02s), Linux1/1 (39.25s). Coverage reaches46/47; confined e2e_probe success and remaining model/provider/compiler error conjunctions keep P4-51/TG4-25 open. Overall41/59 and19/33 unchanged. Evidence: _verify/P4-51-EXECUTION-MATRIX-CONSOLIDATED-2026-10-09.md. ADR0351 retracts the earlier invented-run fixture's handler-specific claim; the later47-registration correction below supplies actual handler/guard and model-consumer proof. No paid provider run or Calendar activity.

2026-10-09 P4-61 smoke enforcement (ADR0350): removed e2e_probe's unrestricted model-command shell and arbitrary loopback HTTP launcher. The live registration/schema remain, but valid requests now return a bounded correlated sandbox_violation (loopback_smoke, deny, retryable=false) before process/HTTP activity until a proved confined backend exists. URL admission performs no DNS; fields/descriptions report the unavailable capability. Windows10/10 (.79s), Linux10/10 (.66s); the registered valid-call refusal proves no workspace marker, no connection to an owned loopback listener, private-input omission and cleaned port. Independent Luna High review's two stale schema/redirect claims are corrected. This removes a bypass but does not provide positive smoke execution or close P4-51/P4-61/TG4-25/TG4-31. Evidence: _verify/P4-61-PROBE-FAIL-CLOSED-2026-10-09.md.


2026-10-09 P4-51 model-consumer correction (ADR0351/0352): all47 FastDevelopment registrations now reach a controlled real handler/permission/sandbox failure and the next AgentEngine model request receives correlated bounded failed JSON. Real RunsDb ticks prevent the journal-prerequisite substitution in the earlier17-case fixture. Exact matrix passes Windows1/1 (6.13s), Linux1/1 (7.82s); surrounding engine module15/15 passes on both (12.36s/17.30s) after correcting accepted-socket blocking mode in the bounded Windows HTTP fixture. Coverage remains46/47 successful registrations; positive confined smoke, compiler/private OS diagnostic and provider IO conjunctions remain open. Evidence: `_verify/P4-51-MODEL-CONSUMER-MATRIX-2026-10-09.md`. P4 remains41/59 and TG419/33; no paid provider call or Calendar run.


2026-10-10 P4-51 registered compiler privacy proof: actual claimed Cargo build through FastDevelopment/ToolExecutor fails with E0425; bounded JSON contains only safe relative diagnostic location, and TasksDb reads the attributable nonzero exit/finalized failed batch while RunsDb reads the failed journal outcome. Private long source/key/raw-name/absolute path canaries are absent. Windows1/1 (16.41s), Linux1/1 (0.73s); fixture lockfile and Windows identity prerequisites match production helpers without weakening guards. Evidence: `_verify/P4-51-REAL-COMPILER-PRIVACY-2026-10-09.md`. Successful tool coverage46/47, P441/59 and TG419/33 unchanged.


2026-10-10 P4-51 provider IO receipt (ADR0353): real Google adapter request stalls under an explicit cfg(test)-only750ms client timeout, retries before output, fully consumes a successful stream and yields exactly one resolved retryable analytics row. Google module passes Windows21/21 (3.77s), Linux21/21 (3.84s), with owned bounded server cleanup and private marker omission. Shipped Client::new has no request/read timeout; this does not establish a production deadline or cancellation of in-flight requests. Shared cancellation correction awaits owner direction. Evidence: `_verify/P4-51-PROVIDER-TIMEOUT-CONSUMER-2026-10-09.md`. Overall P441/59, TG419/33 unchanged.


2026-10-10 active continuation: Linux production mixed Cargo/Python gates now pass3/3 (18.27s): failed sibling blocks QC, timed-out sibling leaves an incomplete batch, and three observed failures stop dispatch until one audited Supervisor retry passes both projects. Existing real multi-project spawn failure and cancellation pass Linux1/1 each (1.89s/1.16s), including reopened DB QC refusal and descendant-stop readback. Windows counterpart passes3/3 (266.56s). Linux sandbox candidate admission passes10/10 (.07s); actual delegated-cgroup enforcement and guarded host copy-back remain unproved. The actual oversized read_file OS failure now reproduces private filename-prefix leakage through the next AgentEngine request (ADR0355); correction direction is pending, and its failing fixture is uncommitted. P4 remains41/59; TG419/33. No paid provider or Calendar run. Evidence: `_verify/P4-14-MIXED-LINUX-CLOSEOUT-2026-10-10.md`, `_verify/P4-51-TRUNCATED-OS-ERROR-2026-10-10.md`.


2026-10-10 privacy decision boundary: oversized real read_file OS failure leaks a private argument prefix into the next model request on Windows1/1 (1.06s) and Linux1/1 (1.27s). These are deliberate failing regressions, not passing gates. Luna High confirms the mechanism and recommends retaining useful OS cause while removing partial reflection. Production remains unchanged pending Rule4 direction; the new failing engine fixture is uncommitted. Verified mixed-project unit `2513b72` is pushed to origin/main. No new task/gate closes; all owned test processes exited. Evidence: `_verify/P4-51-TRUNCATED-OS-ERROR-2026-10-10.md` and `_verify/P4-51-TRUNCATED-ARGUMENT-REVIEW-2026-10-10.md`.

2026-10-10 P4-57 worker-run chronology: the actual begin/heartbeat/progress/finish APIs now have exact creation, progress-event/state and terminal-run/state timestamp assertions through shared parsing and database reopen. Independent Luna High identified the initially missing creation-reopen assertion, now included. Windows1/1 (0.09s), Linux1/1 (0.13s). This is a concrete consumer proof, not an annual all-pair or monotonic-wall-clock guarantee; P4-57/TG4-19 remain open. Overall41/59 P4 and19/33 TG4 unchanged. Evidence: `_verify/P4-57-RUN-CHRONOLOGY-2026-10-10.md`. No paid provider or Calendar run.

2026-10-10 P4-57 procedural Memory correction: the real topic-write/reopened-search regression failed on Windows by selecting canonical09:00 over legacy noon at limit1. The production search now follows existing Memory instant ordering with julianday(updated_at) and stable ID tie-breaking; no schema/API/index change. Canonical/legacy/offset selection, ties, filtering and zero limits pass in the procedural suite: Windows6/6 (.46s), Linux6/6 (.55s); core/CLI check passes (16.91s). Luna High review reports no scoped defect. The Memory/config source inventory distinguishes actual timestamp consumers from stored-only fields. Annual all-pair/platform coverage remains open; overall41/59 P4 and19/33 TG4 unchanged. Evidence: `_verify/P4-57-PROCEDURAL-ORDER-2026-10-10.md`; ADR0357. No provider or Calendar run.

2026-10-10 P4-51 isolated OS-cause repair (ADR0358): read_file now retains its actual IO cause without echoing the requested path. The real all47-registration next-model matrix passes Windows1/1 (5.98s), Linux1/1 (6.79s), positively checking the observed OS error code and private-prefix omission. File-read bounds pass2/2 per host; core/CLI check passes (36.67s); Luna High reports no scoped defect. This repairs the demonstrated read_to_string case only; other producers and shared truncated-prefix sanitizer hardening remain separate pending scope. Successful tool coverage46/47, P441/59 and TG419/33 unchanged. Evidence: `_verify/P4-51-READ-FILE-CAUSE-2026-10-10.md`. All selected processes exited; no paid provider, Calendar, server or listener activity.

2026-10-10 owner stop: running goal paused on explicit request. Three verified units1554433/853a377/410b13b are pushed; P4 remains41/59, TG419/33. All owned tests/checks completed; older unpublished sandbox/macOS/desktop harness work is preserved. The latest owner-stop section in `_verify/P4-SESSION-CHECKPOINT-2026-10-09.md` lists exact remaining tasks and five pending Rule4 decisions. Resume core P4 first; expanded goal unread, Calendar owner-only, no second paid run authorized.

2026-10-12 resume: goal active; current HEAD/origin main df84370. Owner explicitly approves all five pending runtime/sandbox/sanitizer decisions (ADR0359). Snapshot of tracked diff/non-ignored untracked files taken before fan-out under ignored target/p4-ci-checks/p4-resume-2026-10-12-snapshot. Proceed with scoped implementation and proof; current P441/59, TG419/33. No second paid run, user-database access, Calendar execution or expanded-goal work authorized.

2026-10-12 P4-51 shared prefix hardening (ADR0359/0360): owner-approved executor enrichment now removes bounded full/truncated argument reflections at Unicode boundaries while preserving existing twelve-character matching, useful causes, JSON shape and correlation. Exact tool_result_error_tests pass Windows11/11 (.15s), Linux11/11 (.57s), with credential/database variables absent. Independent Luna High source review found no scoped sanitizer defect. Producer-truncated causes cannot be reconstructed. Evidence: `_verify/P4-51-SHARED-PREFIX-HARDENING-2026-10-12.md`. Provider cancellation and actual offline CLI Fleet proof proceed separately; sandbox/desktop prerequisites remain. No task/gate closure or readiness rescore: P441/59, TG419/33. No paid call, owner Calendar or expanded-goal work.

2026-10-12 provider cancellation unit (ADR0359/0361/0364): AgentEngine cancels preflight, request establishment, retry delay and body waits, prioritizes cancellation over ready chunks, and dispatches no later tool in the focused fixtures. Both budget decorators retain unknown-output holds through partial usage; explicit Done or recognized terminal reason followed by EOF reconciles observed totals, while bare/unknown EOF, errors and drops charge the conservative maximum. Windows exact run-token9/9, daily-cap9/9, engine cancellation6/6 pass; measured pending cancellation110–143ms, ready-body~5ms/onepoll. Independent Luna High final source review found no unresolved scoped defect. Linux and live-vendor cancellation are not established by this evidence; remote processing/billing may continue. Reports: `_verify/P4-51-PROVIDER-CANCELLATION-2026-10-12.md`, `_verify/P4-51-FAILURE-BOUNDARY-REVIEW-2026-10-12.md`. P441/59, TG419/33 unchanged; shared lease and actual CLI Fleet proof are separate ongoing units.

2026-10-12 P4-61 kernel quota milestone: source-matched delegated cgroup test passes Linux1/1 (6.37s). Actual aggregate CPU throttling, 48MiB memory OOM kill, pids.max=4 denial, confirmed empty-state teardown and zero owned leftovers are observed. Transient service is removed/inactive; no persistent machine changes. Evidence: `_verify/P4-61-DELEGATED-QUOTA-PROOF-2026-10-12.md`. This is kernel proof, not production sandbox enablement: private-stage lifetime, recoverable copy-back, full filesystem/network capability conjunctions and macOS remain open. Root managed-file lease tests pass Windows3/3 plus ignored subprocess helper, and actual file-tool consumer/context/panic checks pass1/1 each; admission-before-journal cancellation refinement is being verified. Owner explicitly approved sandboxed shell hooks and retained child/lease cleanup ownership (ADR0365/0367); launch remains disabled until lifecycle proof. Privacy6554942 and cancellation102ff7d are pushed to main. Core tally41/59, TG419/33 unchanged; actual CLI child has passing batch and is at QC, not yet full Fleet success. Calendar owner-only; expanded goal unread.

2026-10-12 continuation correction: the write-admission cancellation test now passes on Windows and confirms no false unknown journal entry after DB reopen. The owner-approved Windows shell-hook launch/cleanup path is enabled and passes its focused lifecycle tests; the earlier note that it remained disabled is superseded. The current cleanup owner reports `Retrying` and retains the workspace lease until the OS confirms an empty job. A remaining gap is that Execute/FreeformShell process handlers do not yet acquire the same lease; shutdown/drain exclusion is also unproven. No whole P4 task/gate is closed. P4 remains 41/59 checked and TG4 19/33 passing.

2026-10-12 offline CLI Fleet unit closeout: the actual feature-gated CLI Fleet subprocess completes worker Cargo verification, QC review, child/epic completion, and Logs/analytics export readback. Windows fleet_offline passes5/5 (22.54s), including unmarked/redirected-profile refusal; ordinary builds execute0 offline tests, and core/CLI cargo check passes (24.96s). Default-off marker paths are reparse/symlink guarded; the parent test has an outer deadline and Windows tree cleanup. Unix outer-timeout descendant cleanup remains unproved. Evidence: _verify/P4-50-CLI-OFFLINE-FLEET-2026-10-12.md and _verify/P4-50-OFFLINE-CLI-COMMIT-REVIEW-2026-10-12.md; ADR0368/0369. This completes the offline CLI execution slice, not live Gemini, full observability/UI, platform sandbox or P4-49. Core checklist remains41/59 P4 and19/33 TG4. No paid provider or owner Calendar activity.

2026-10-12 managed-file admission unit: ca9e21f is pushed to origin/main. ToolEffect::Write handlers hold a canonical-workspace OS lease; cancellation before admission records no intent, and actual cross-process/contention/drop/cancellation consumer evidence is recorded in _verify/P4-61-MANAGED-FILE-ADMISSION-2026-10-12.md. No new dependency/schema. Shell-hook/process writer integration, staged copy-back, host shutdown drain and complete recovery snapshots remain open. Core remains41/59 P4 and19/33 TG4. The initial remote commit_refs failure was retried only after verifying the remote exactly matched the local parent; the normal push succeeded without history changes.

2026-10-12 P4-53 recovery/path repair: actual shipped worker claims a task, performs two registered file writes, loses the second durable outcome, then is interrupted. Reopened file/checkpoint/journal readers preserve both effects, exact relative target references and first acknowledged changed path; the task is needs_info and a fresh scheduler kick makes no provider call or repeated write. Windows focused proof passes root1/1 (1.26s); normal core/CLI check11.27s, formatter/whitespace checks pass. Independent Luna High scoped review finds no defect; local-drive proof does not execute a UNC share. Existing Windows jail normalization now also normalizes the journal root before relative comparison (ADR0372). Evidence: _verify/P4-53-MULTI-MUTATION-INTERRUPTION-2026-10-12.md and _verify/P4-53-JOURNAL-PATH-REVIEW-2026-10-12.md. Full snapshots/host verification, other interruption modes, safe reassignment/resume and task restore remain open; ADR0371 owner direction is pending. No whole task/gate closes:41/59 P4,19/33 TG4.

2026-10-12 P4-62/TG4-32 closeout (ADR0373/0374): all named timing/deadline clauses now have evidence. Existing paused-clock production worker >10min and two-hour cap/reconciliation proofs, actual selected30s Windows/Linux descendant cancellation with exported incomplete stage events, five-stage production writer/CLI readback, and actual offline CLI Fleet deadline/shutdown are reused rather than repeated. New Windows measurement passes1/1 in166.19s:18passing claimed Python builds over3fresh homes, cold wall median35.255s (34.936–37.141), warm3.655s (3.602–4.083), evidence persistence6.313/6.410ms cold/warm medians. Windows core/CLI check17.91s, Linux test compilation2m55s, formatter/whitespace checks pass; existing warnings remain. Independent Luna High review finds no scoped defect. P4-62/TG4-32 close on their own BENCH-PROVEN reliability/performance contract; live-vendor acceptance remains P4-50 and macOS confinement/three-OS phase exit remain P4-11/P4-61/Part6. No phase closure or platform exception. Reports: _verify/P4-62-WINDOWS-TASK-EVIDENCE-TIMING-2026-10-12.md and _verify/P4-62-WINDOWS-TIMING-REVIEW-2026-10-12.md. Current tally42/59 P4 (16engineering plus ownerCalendar remain),20/33 TG4.

2026-10-13 P4-57 approved-clock unit (ADR0375/0376): isolated test-only Rust/SQLite clocks pass3/3 on Windows, including4400actual Task create/update and Run begin/finish instants through reopened production readers (24.99s). Canonical text and parsed instants match leap-year/offset/DST corpus. Existing numeric/date conversion functions and expression indexes remain built-in; no deterministic flag workaround. Normal core/CLI check34.19s, format/diff checks pass. Other generated-now pairs, checkpoint annual proof and platform closure remain open; counts42/59 P4,20/33 TG4 unchanged. Owner also approved durable snapshots/host verification; that implementation is next, not yet proved. Evidence: _verify/P4-57-ISOLATED-CLOCK-2026-10-12.md.

2026-10-13 P4-54 real interrupted-worker health proof (ADR0377): actual shipped worker two-mutation/lost-ack fixture now reads the registered Supervisor health tool through ToolExecutor after storage reopen. Windows1/1 (1.33s) proves failed lifecycle/failure/heartbeat, held task ownership/dependency value, two task/session checkpoints, exact acknowledged vs unknown cursors/paths and recovery-required flag; a later kick still makes no call/write. Other full-field/two-worker/reassignment clauses remain open; counts42/59 P4,20/33 TG4. Evidence: _verify/P4-54-INTERRUPTED-WORKER-HEALTH-2026-10-12.md.

2026-10-13 P4-57 worker/plan annual proof: new exact filter passesWindows1/1 (80.19s), driving4400instants through actual WorkersDb create/heartbeat and PlansDb plan/item create/status APIs, then reopening public readers. Seven more generated-now field pairs match exact canonical text and parsed instants, making11new annual pairs with task/run proof. Other inventory pairs and cross-platform closure remain open; no checkbox/count change (42/59 P4,20/33 TG4). Evidence: _verify/P4-57-ISOLATED-CLOCK-2026-10-12.md. These are isolated rows, not running Fleet agents or provider calls.

## Current recovery milestone — 2026-10-13

Source unit `2db3f39` implements approved bounded journal snapshots and host-verified completion. Actual registered acknowledged/lost-ack writes reopen before/after metadata, preserve both files and acknowledge the completed effect exactly once. Windows: recovery3/3, RunsDb29/29, executor20/20, shipped-worker interruption/health smoke1/1, hook presence1/1, input fingerprint1/1 and workspace lease3/3; SQL registry9/9, core/CLI check and workspace formatting pass. See `_verify/P4-53-HOST-SNAPSHOTS-2026-10-13.md`.

RetrySafe is refused in production until a task/operation-bound one-use permit is revalidated and consumed under executor admission's write lease (ADR0379, owner decision pending). Reconciled returns operation_complete=true and retry_permitted=false. P4-53/TG4-27 remain open for durable continuation, replay-safe replacement execution, other interruption/effect classes and task-scoped restore. Counts remain **42/59 P4, 20/33 TG4**:17 open tasks, including owner-only Calendar;16 engineering tasks. No paid provider, owner Calendar, server or listener was used. Core P4 precedes the unread expanded goal and P5/P6. Latest continuation: `_verify/P4-SESSION-CHECKPOINT-2026-10-13.md`.

2026-10-13 annual writer proof: Windows tests now add10fields across Memory (episodic.created_at, semantic.created_at/updated_at, procedural.created_at/updated_at, unified memories.created_at), checkpoint.created_ms, graph nodes.updated_at and hook-config.created_at/updated_at. Each uses4400leap-year/DST instants through actual writers and reopened production readers. Unified Memory covers direct gateway plus episodic/semantic/procedural sync paths; checkpoints also verify50-row per-file/ownership retention. Exact tests each pass1/1: Memory711.90s, checkpoints112.33s, graph264.95s, hook-config213.77s. Fixture wall times are not application speed measurements. This makes21distinct generated-now fields proven on Windows with the earlier11Task/Run/Worker/Plan fields. Remaining inventory pairs and platform/legacy aggregation keep P4-57/TG4-19 open; counts42/59 P4 and20/33 TG4 unchanged. Reports: `_verify/P4-57-MEMORY-ANNUAL-2026-10-13.md`, `_verify/P4-57-CHECKPOINT-ANNUAL-2026-10-13.md`, `_verify/P4-57-GRAPH-HOOK-ANNUAL-2026-10-13.md`. No live provider/database, Calendar, server or listener was used; all four test processes ended. Superseded by13October run-journal and Task-auxiliary receipts:34generated-now fields now have Windows annual proof; approved one-use admission is implemented and pushed as bdced1c. Platform/legacy aggregation remains open.

## Owner deadline — 2026-10-13

Owner sets a maximum deadline of16:00 Asia/Bangkok on2026-10-13, stating current time07:53. Prioritize closure of the17open core P4 items through their named gates, coherent verified commits/pushes and immediate blocker reporting. Do not fabricate platform/behavior proof or relax security to meet the deadline. Hook readers and one-use revalidated retry admission are now explicitly approved by ADR0380. Calendar ownership clarification is pending because P4-49 was previously owner-only. Existing earlier approvals stand; do not re-ask settled decisions.
