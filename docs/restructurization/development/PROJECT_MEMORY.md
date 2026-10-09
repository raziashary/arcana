# Arcana working memory (updated 2026-10-06)

Latest active checkpoint: `_verify/P4-SESSION-CHECKPOINT-2026-10-06.md`.
Current tally: **40/59 P4, 19/33 TG4**. Retention P4-59/TG4-21 is proven
across the complete Windows/Linux/macOS consumer checks. Startup retry
P4-52/TG4-26 is proven on its own contract, as the owner approved in ADR-0319;
successful paid Fleet stays open under P4-50. Capture unit `1cdb7a1` passes
the current Linux whole-workspace CI job; earlier capture-loss cause remains
unconfirmed. macOS's six retention groups pass, while its separate command
corpus fails on the absent signed XPC runner. No full-pipeline claim.
Session/message annual pairs pass Windows/Linux. HooksDb now preserves
validated execution time (ADR-0320): Windows/Linux hook module 11/11, core
check, and real Windows CLI export 1/1. Other timestamp pairs/platform proof
remain open. The approved isolated desktop diagnostic build (ADR-0321)
builds successfully; attachment proof is underway. Expanded goal unread; Calendar owner-only; no second
paid Fleet run authorized.

2026-10-06 recovery: latest pushed commit `c2e3622` bounds Supervisor output.
CI run 37305252643 failed three stale RunsDb tests, blocking platform jobs;
ADR-LOG-RESTRUCT-0310 records fixture-only repairs. RunsDb now passes 29/29
on Windows/Ubuntu and the query registry passes 8/8 on Windows. Rerun remote
CI before claiming platform proof. Retention closure still needs the complete
platform corpus.
Analytics timestamp tests now pass 2/2 on both hosts; all six retention
consumer selections pass on Windows and are wired into the three-host matrix.
Evidence: `_verify/P4-57-ANALYTICS-PAIR-PROOF-2026-10-06.md`.
Keep unfinished desktop consent files separate and the expanded goal unread.

Historical continuation (2026-10-05): **P4 38/59 checked; TG4 17/33 passing**.
P4-55/TG4-29, P4-43, P4-40, P4-23/TG4-10/11 and P4-29/TG4-15 are proven. P4-16/TG4-17 is also proven on all three CI hosts; TG4-02 and TG4-06 are proven through saved recipes on Windows/Linux. Core P4 remains active; 21 tasks
remain, including owner-run P4-49. The expanded goal stays unread until core
P4 closes. Claim-safety proof is in
`_verify/P4-40-CLAIM-SAFETY-CLOSEOUT-2026-10-05.md`; budget interaction is in `_verify/P4-29-BUDGET-INTERACTION-CLOSEOUT-2026-10-05.md`. Remote host-matrix proof,
strict sandbox corpus and successful CLI Fleet completion remain open. Earlier
dated status entries below are historical snapshots.

Authoritative current scope: [ARCANA_CODING_SCOPE.md](ARCANA_CODING_SCOPE.md).
Historical implementation tasks and invariant rules: `MASTER_DEVELOPMENT_PLAN.md`
§0 and phase sections. Current code and observed behavior override historical
status prose; `_verify/ARCANA-RECON-PLAN.md` and
`_verify/ARCANA-RECON-APP.md` record this reconnaissance.

The product's current distinction is a native coding harness: Arcana owns
deterministic execution and lifecycle behavior; models supply code generation,
planning, and judgment. Built-in recipes and team workflow must work without
users first creating their own agent scaffolding, with recipe customization as
an option. Prove every capability at its real production consumer. The
cross-phase audit findings and owner dispositions are in
`_verify/2026-09-27-CROSS-PHASE-HARNESS-AUDIT.md`.

- Desktop: Tauri v2 (`src-tauri`) with React/TypeScript (`src`), invoking Rust
  commands and listening to events. `arcana-core` owns the native engine,
  providers, task board, memory, recipes, graph, security, and persistence.
- `src-cli`/`arcanactl` shares core but its old HTTP agent verb was retired in P8.
  `src-shared` carries remaining shared types/data access.
- P8 removed Python orchestration and memory sidecars. PD0's separate Node
  protocol reference was retired from source and npm by ADR-0043; its history
  remains in the restructuring documents.
- Provider keys are stored through `VaultManager` in OS keyring or encrypted
  fallback. The desktop does not load `.env`. Only the explicit live-provider
  tests read `GOOGLE_API_KEY` from the repo `.env` for Gemini.
- Source points to fresh Arcana state (`~/.arcana`), keyring service,
  application identifier, `.arcana` association, crates/packages and CLI.
  Old AutoDev state remains untouched; the final Windows debug NSIS package
  has been rebuilt and inspected for P8.
- The original weighted readiness snapshots (34.0% and 36.7%) are historical;
  no current Arcana percentage is claimed. P0/P3 gate-proof gaps remain in
  their own registers and were not waived by P8.
- The 2026-09-24 P8 closeout was reopened under ADR-0075 for host-enforced
  commit safety. The 2026-09-28 follow-up audit's shared-index, staged-binary
  scan, and stale gate findings are now fixed and retested in the current
  worktree; this final P8 hardening was verified and pushed on 2026-10-03. P4 remains active,
  followed by P5–P7; P9 is later.
  Retired design rows and the former two-rollout sequence live in
  `HISTORICAL_DESIGN_PROGRAMME.md`. The 2026-09-28 active register has 391
  explicitly checkboxed P-numbered rows (295 checked, 96 open), not a
  readiness score; the source-derived MDP count excludes retired design rows.
  P8-01 and P8-68–74 are checked after TG8-02's consumer-backed port proofs
  and TG8-13/S8-06 host commit-safety proof. The 2026-09-28 strengthening
  serializes the in-process shared index, validates malformed path arrays,
  scans staged blobs (including binary contents), and enforces the documented
  message separator. The Bash textual heuristic still is not a process sandbox.
- Owner changed target on 2026-09-23 to a personal coding-only Arcana. ADR-0042
  records this; ADR-0043 authorizes PD0 retirement and fresh Arcana state.
- The 2026-09-24 desktop smoke completed a Gemini `read_file` coding turn in
  session `13fdac95-d2e1-4bea-83e3-ba168a9f54f4` with a correct visible
  answer; a four-minute process sample found no app-owned Python process.
  `_verify/P8-ARCANA-CLOSEOUT-2026-09-24.md` is the evidence record. The first
  attempt exposed a Google tool-schema incompatibility, corrected in ADR-0052.
- At startup the environment scrub removed
  `LD_LIBRARY_PATH` and several non-secret path/metadata variable names while
  announcing them as credential-shaped. Audit this filter before P4's
  cross-platform worker-command proof; no behavior claim follows from the
  current source-only security gate.

The 2026-09-24 P8 evidence and independent Luna audit disposition are in
`_verify/P8-ARCANA-CLOSEOUT-2026-09-24.md`; ADR-0075 reopened P8 for host
commit safety. A 2026-10-03 Luna High review found a branch-switch race, so
P8-74/TG8-13 are open again; the host captures/rechecks the branch ref/OID and
the injected protected-branch race regression passes. Task-claim revalidation
and production AgentEngine-to-scheduler proof remain open. The final claim
check now holds `BEGIN IMMEDIATE` across the synchronous Git ref update; a real
concurrent status-writer regression proves revocation waits, and all 15 focused
`git_commit::tests` pass. Orphan scan/release DB failures now propagate to the
Supervisor caller, with a distinct timed failure event and focused tests. P4 is
active. P9
delivery and additional features are later scope.

Current P4 continuation (2026-10-03): partial `append_file` write failures now
carry `outcome_unknown=true`, engaging the RunsDb/ToolExecutor unresolved-effect
barrier; the production reconciliation action and safe continuation are still
missing. Project-command build-evidence persistence has a separate Logs-readable
stage timing event using `call_id`; P4-62 still needs provider-backed CLI
shutdown, measured persistence cost, cross-platform runs, and repeated cold
profile variance. The latest full `arcana-core` suite remains 913 passed, 2
failed, and 1 ignored; both failures are Windows sandbox/Clippy path
canonicalization denials. The tree remains broadly dirty on `main` with no
feature commits or pushes from this continuation.

## P4 checkpoint (2026-09-26)

P4 is active on `main` from `9cc0433`; both GPT-6 Luna source/doc refutation
audits are in `_verify/P4-PREBUILD-DISPATCH-TASKS.md` and
`_verify/P4-PREBUILD-DELIVERY-COORDINATION.md`. Implementation is underway and
the phase is not complete. Completed slices include strict typed worker status
with isolated v2 migration tests; owner-checked atomic claims, one-task limits
and epic guards; transactional dependency/epic updates and atomic per-entry
assignment/routing history; and config DB consent persistence, provider
enforcement, Settings grant/revoke controls, and deterministic QC fallback.
Focused tests, `cargo check -p arcana --locked`, and TS typecheck passed. The
full handoff and remaining work are in
`_verify/P4-SESSION-CHECKPOINT-2026-09-26.md`.

The initial source orientation found `Worker.status` as an unchecked `String`
and `build_evidence` as a process-local map; those findings are now resolved
by the typed status/schema work and task-associated build-evidence table.
P4-13 is closed with an integration test of the real `project_command`
producer and QC consumer: missing/failed builds are refused, passing evidence
permits QC, and the result survives DB reopen. P4-14 is now closed: SQLite's
QC trigger rejects direct SQL, and production tests cover the shared TasksDb
path, Tauri task-status command, and stranded-task RecipeRunner reconciliation.
TG4-07 remains open for spawned-process exit coverage and cross-platform worker
proof. The owner settled P4-15 semantics: count every observed nonzero
`project_command` build exit, reset on observed success, keep Bash for general
work, and require its per-call approval while only `project_command` may mint
QC evidence. P4-20 keeps the shared fleet branch and task checkpoints; P4 keeps
saved per-task budgets; P4 owns persisted/revocable consent behind
TG3-26/TG4-16; S4-03 proves in-process cancellation/recovery. ADR-0067…0070
record these decisions. Checkpoints now carry verified task IDs in ToolContext,
distinguish prior file absence, fail closed before direct file-tool mutation,
and refuse stale cross-task restoration. Whether process writes also need
task-level snapshots remains pending; OS write containment and Bash approval
are both required by P4-18.

P4-17 closed on 2026-09-28: the shared strict workspace resolver canonicalizes
configured or explicit project roots and rejects unset, relative, missing, and
non-directory paths. Sessions, fleet recipes, IDE/file access, diff analysis,
and persisted egress consent use it. The former session CWD fallback and
recipe Arcana-agent/home fallbacks are removed. P4-18 file-write enforcement
remains open. P4-21 now has a behavior-tested app-root `ide:navigate` listener
that maps in-workspace absolute paths, expands the IDE tree and opens the
requested line. The full artifact-to-run smoke and TG4-09 remain open.

P4-22 closed on 2026-09-28: the real ToolExecutor path persists sender
identity, refuses self-send and worker broadcast, caps message size, and runs
the registered recipient inbox/ack tools through recipient identity. P4-23
has partial production-consumer proof for stable-ID replay and ack; full
scheduler redispatch and idempotent side effects remain open under TG4-11.

The user also chose durable ACP delivery at-least-once until acknowledgement;
stale recovery may replay the same stable message ID and consumers must apply
effects idempotently. Exactly-once delivery is not a valid claim (ADR-0070).
P4 source-refutation reports are complete; findings continue to be fixed and
verified. The dispatcher now uses event kicks plus a five-minute recovery
cadence, keeps its schedule after transient DB reads, skips empty task/message
runs, applies a ten-minute push-run timeout, releases interrupted claims, and
wakes the Supervisor when autonomous completion unblocks work. Focused tests
pass, while end-to-end dispatch smokes and remaining phase tasks stay open.

2026-09-28 follow-up: scheduler future panics/timeouts are contained, cancelled
runs recover through the RAII guard, failed task-DB recovery leaves the worker
marked `working` for a later retry, and a fresh real RecipeRunner tick is
proven after timeout. The persisted `qc-agent` is bound to `qc-dispatcher` and
the shipped recipe reads/acks inbox-only work through AgentEngine. The shipped
Supervisor test consumes stale-mail IDs and persists a QC nudge; automatic
worker redispatch after that nudge remains open. QC
deterministic diff fallback checks consent before provider-key resolution.
Current behavior evidence and still-open S4/TG proof are appended to the
session checkpoint and cross-phase audit. P4 remains uncommitted and open;
current plan count is 391 P-numbered rows (295 checked, 96 open). P4-30 now
proves customized recipe and worker-profile readback through the file-backed
worker binding used by `RecipeScheduler`; P4-44 now sends scoped procedural
recall only to the next matching fleet provider request, with interactive
default-off behavior preserved. Workspace formatting has been applied and
`cargo fmt --all -- --check` passes.
The 2026-09-26 continuation added `tasks.db` migration v5, worker-result
persistence after process completion, and task-keyed QC lookup. On 2026-09-28,
P4-13 was closed with a spawned Cargo build flowing through persisted evidence
into the shipped status tool. P4-14 is now closed: SQLite enforces the
task-scoped QC evidence boundary, and focused production tests cover direct SQL,
the shared TasksDb API, Tauri status command, and RecipeRunner reconciliation.
P4-11 and TG4-07 remain open for
three-platform command and full desktop worker/QC proof. P4-12 is closed on
the file-backed evidence write/reopen/reader test recorded in the master plan.
See the session checkpoint for exact validation and resume order.

The 2026-09-27 portability scan passes for shipped defaults: core tests scan
built-in recipes, interactive system prompts, and registered tool descriptions
in all three modes; the Tauri test scans all 17 built-in skill templates. The
The owner ruled that P4-16/TG4-17 cover persisted user-authored recipes,
skills and hooks. Shared validation now runs at save/load/use boundaries; the
complete shipped/custom corpus gate remains open. No shipped hook templates
exist. TG4-18 and cross-platform process execution remain open.
ADR-0073 records the owner-approved scope: Arcana-mediated provider/diff egress
is gated; free-form shell network activity needs separate OS-level control and
is outside this consent contract. An early file-backed worker
migration test wrote synthetic backup snapshots under
`C:\Users\razia\.arcana\backups`; the files were not inspected or removed,
and no actual user DB content was changed. See ADR-0071 before further
file-backed migration tests.

P4-26/TG4-14 closed on 2026-09-28: routing excludes system-defined workers
before scoring and excludes their history from affinity and duration
normalization. Static matches only select a scored eligible worker; an empty
implementation roster returns an error. Focused tests load the seeded roster in
an isolated database, exercise Supervisor/QC with stronger history and a
static match, and prove system-agent timing history cannot change the selected
implementation worker. See the MDP/TG4-14 evidence and scorer tests.
P4-33 closed on 2026-09-28: the static selector now accepts the shipped pattern boundary at 0.85. The regression test obtains the real API/endpoint pattern and runs it against the actual seeded worker roster in memory, proving the route returns the backend worker with `method: static`. Earlier file-backed test execution left backup artifacts under `C:\Users\razia\.arcana\backups`; they were preserved. Routing tests now use an in-memory seeded DB.
P4-38 closed on 2026-09-28: `assign_with_routing_history` enforces the existing one-in-progress-task-per-worker rule in the same atomic task update. The shipped batch loop continues after a refused entry; isolated in-memory proof confirms the row remains ready and a subsequent valid entry succeeds.

P4-24/TG4-12 closed on 2026-09-28: pending Supervisor requests now count as
recoverable work, but the shipped tick protocol continues to dispatch unrelated
READY tasks and can stop idle while the request remains pending. The focused
test leaves the request unresolved and reads back a successful real batch-tool
assignment. P4-25 closed at the tool writer: task completion and the newest
pending routing-row outcome for the task's pre-transition owner commit together;
older pending attempts are preserved, and manually assigned work without a
routing row still completes. The legacy task-only outcome helper updates at
most the newest pending row. Tool and Tauri status outcomes compare the
observed status and owner in the transaction, refusing a delayed completion
after reassignment. P4-40 automatic orphan release now compare-and-sets the
scanned state, owner, PID, host and updated timestamp; a stale snapshot cannot
release a newer claim. SQLite-wide metadata enforcement and manual force-release
semantics remain open pending owner direction. See ADR-0078…0080 and the P4
session checkpoint for tests. Full TG4-13 N-outcome ranking proof remains open.

Latest 2026-09-28 continuation: the real `UpdateTaskStatusTool` dependency
completion test now proves an ordinary dependent reaches `ready` while a
budget-exceeded dependent with the same blocker stays `needs_info` and retains
its reason. This is partial P4-39 evidence; automatic Supervisor routing and
worker dispatch after completion remain open. P8 host-safety tests, S8-06, and
TG8-13 were rerun successfully; frontend typecheck and all 21 Vitest tests
pass. A Luna High checkpoint review found P4-19/20 need task-owned snapshots,
fail-closed snapshot creation, complete direct-writer coverage, and stale
restore refusal; the broad Bash/process boundary awaits P4-15 direction.

Cross-phase P2 analytics audit correction: analytics date readers now group
stored UTC timestamps by SQLite's local calendar day. An isolated boundary test
verifies records around local midnight through the date, today-activity, and
range readers. ADR-0085 and TG2-15 record the partial fix; timezone matrix,
daylight-saving, and sub-hour cadence proof remain open. See
`_verify/2026-09-28-P1-P2-CROSS-PHASE-AUDIT.md`.

P8 reclose evidence (2026-09-28): TG8-13 now also fails if any shipped worker
recipe omits the `main`/`master`/`trunk` protected-alias instruction. Re-ran
the gate, all ten `git_commit` tests, the Bash Git-mutation guard, and S8-06
against an isolated real Git repository; all pass. On 2026-10-03, independent
Luna High review reopened P8-74/TG8-13 after a lock-wait branch-switch race.
The host now captures and rechecks the validated branch ref/OID; `git_commit`
tests pass 12/12. Task ownership can still go stale across lock waits, and the
real AgentEngine-to-scheduler commit path is not yet exercised. Bash remains a
textual guard and external Git writers are outside Arcana's in-process lock.

P1/P2 cross-phase audit corrections: TG2-11 is now unchecked/PARTIAL because
Linux no-D-Bus fallback creation, permissions, backend reporting, and key readback
have no runtime proof. TG2-14 now verifies the actual supported-provider set
instead of requiring removal of legacy strings needed to reject retired IDs
and let users remove old keys; the four focused factory tests pass. These
corrections are recorded in `TESTING_GATES.md`, `READINESS_MATRIX.md`, and the
P1/P2 audit. TG2-15 remains open beyond the host-local midnight boundary test.

Fresh P4-11 verification (2026-09-28): all eight `project_command` unit tests
pass, including mixed Rust/TypeScript root discovery, nested package selector
fallback, workspace boundary rejection, real process exit reporting, bounded
output, persisted evidence/QC readback, and Windows `.cmd` package-manager
launch through a path with spaces. Checked-in Rust/Node/Python command corpora
now run build/test/lint/run intents through the production launcher; Windows
passed locally, and the three-OS CI build matrix now runs the corpus test.
Linux/macOS results and full production worker execution remain open under
P4-11/TG4-18.

Independent P8/P2 review follow-up (2026-09-28): S8-06 was narrowed in the MDP
to say the smoke invokes the real Bash and host-commit tools directly; it does
not claim a full model-produced AgentEngine/RecipeRunner turn. TG8-13 separately
proves registry and default-recipe wiring. TG2-14 now includes a Settings
consumer test and derives selectable provider IDs from the provider factory.

Independent review corrections (2026-09-28): S8-06 wording now says the
isolated smoke calls the real Bash and host-commit tools directly; FastDevelopment
registration and shipped recipe text are proved separately by TG8-13. It no
longer implies a model-produced full worker turn. TG2-14's Settings consumer
now derives selectable IDs from the provider factory and the focused consumer
test passes. The cross-phase audit retracts its earlier inaccurate claim that
the Settings list already used that factory set.

The active MDP task count was re-derived with `MDP-COUNTS-CHECK.py` and is 392
tasks (295 checked, 97 open) with 129 gates. The plan header, §5.1 and current
project-memory count now agree; earlier checkpoint entries retain historical
counts for their recorded dates.

IDE file mutations found during P4-18 review now use the existing canonical
workspace path jail for both source and destination paths; traversal tests pass
on Windows, and a Unix symlink-escape test is included. This is a separate
user-facing IDE path-safety repair, not evidence that the pending agent/process
write boundary or checkpoint tasks are closed. ADR-0088 records the scope.

### 2026-09-28 continuation — P4 routing and acceptance goal

P4-32/TG4-13 now have production-path proof: the real batch dispatch tool
receives a task without model-selected worker identity, uses persisted automatic
outcomes to choose the frontend worker after three backend failures and three
frontend successes, and retains explicit audited overrides outside learning.
Together with P4-25's real status-tool attribution test, this closes normal
outcome-driven routing. The runnable-worker test initially failed because its
assertion bypassed TaskRouter's production filter; the test now mirrors that
boundary and passes. The append checkpoint test now filters by task ID because
Windows canonicalizes separator spelling in the stored file path.

The user's required P4 acceptance smoke is restated in `GOAL.md`: launch the
actual desktop app, delegate through PA chat to the fleet, and build and inspect
a small Google-Calendar-inspired frontend/backend application under
`C:\Playground`, using a new child folder to preserve existing contents. This
real user-facing run is not complete and cannot be substituted by scripted
unit tests. Current plan count is 391 tasks (295 checked, 96 open); no P4
closeout, commit, or push has occurred.

### 2026-09-28 continuation — P4 command and workflow evidence

`autonomous_bash_requires_permission_before_shell_execution` passes through
the real `ToolExecutor::execute_batch` path: Autonomous Bash opens a pending
permission, denial returns a tool error, no marker file is created, and the
pending entry drains. The three-failed-build test now runs actual Cargo
failures through `ProjectCommandTool::execute` and observes diagnostic
escalation at retry_count 3. MDP P4-15/P4-18 and TG4-08 record the partial
proof; model-driven diagnosis/retry and OS write containment remain open.

On Windows, built-in TG4-17 prompt scans and persisted custom recipe, skill,
hook, and expanded-hook refusal tests pass. The full portability corpus and
cross-platform result remain open. Formatting check passes after formatting
the new executor assertion. No application server or listening port was started.
The real PA-to-fleet calendar smoke under a dedicated child of `C:\Playground`
remains mandatory and pending.

The MDP staleness sweep also found its live Tauri command inventory was stale:
250 was recorded while 242 anchored command attributes exist in the current
source tree. The current inventory and module map now state 242, and all 21
staleness checks pass.

### 2026-09-28 continuation — P4-50 observability expansion

The owner expanded P4-50/TG4-24 to require privacy-safe timing and operation
records for memory retrieval/write/index/embedding/fallback paths and every
production SQLite database, plus the broadest safe CLI fleet preflight. The
four UI sections remain Logs, Structured Events, Traces, and Metrics. Official
Rust docs research supports retaining `tracing`/`tracing-subscriber` and
enabling JSON capture; rusqlite 0.31 provides a `trace` feature with SQLite
profile durations. ADR-0108 records the choice to keep the first release
local-first, omit an OpenTelemetry collector/exporter, and omit SQL text and
bound values from telemetry. P4-50 code and proof are pending.

The CLI source currently has task/worker CRUD and an interactive REPL, but no
command that starts the production fleet runner. This must be verified against
the runtime before claiming a CLI full-fleet test. The latest Windows
project-command corpus attempt failed at the first Cargo launch with AppContainer
`CreateProcessW` error 2; its ACL-heavy retry was interrupted and a tentative
traversal-only fix remains unverified. Do not call the Windows corpus green.

TG4-16 now has a production `AnalyzeDiffTool::execute` test over a temporary
Git branch diff. With no workspace consent and no provider key, the tool
returns deterministic-only analysis and writes a denied `qc_diff_analysis`
audit row. This closes the direct QC-tool path proof; the Tauri PR-diff
invocation and full desktop consent path remain open.

P4-40 now has a Tauri command regression test that invokes manual release with
an old UI snapshot after the task was released and claimed by a replacement
worker. The command refuses the stale snapshot and preserves the new claim.
The CAS store tests cover stale automatic release; a fresh writer audit and an
injected race between Tauri preflight and CAS remain open.

## 2026-09-28 continuation pause

The owner will perform P4-49 as the final P4 action and return with the PA transcript and full Logs export. Do not submit the calendar task; `C:\Playground\arcana-calendar` must remain empty until the owner runs it. Gemini 3.8 Flash defaults and the Logs panel tail/export/latency/file-size work are implemented; focused log UI/Rust tests and Gemini model/pricing tests passed. P4 remains open. The production project-command corpus currently fails at its first Windows Cargo launch with AppContainer `CreateProcessW` error 2. A tentative traversal-only ACL change in `process_sandbox/windows.rs` is unverified. Its corpus test was interrupted after several minutes of high CPU during ACL setup; inspect the unidentified AppContainer SID ACE on `C:\Users\razia` before further sandbox testing and confirm cleanup only for a proven orphan. Full continuation details and exact evidence are in `_verify/P4-SESSION-CHECKPOINT-2026-09-26.md` under “Pause checkpoint — 2026-09-28: logs and Gemini defaults.” No commit or push was made.

### 2026-09-28 continuation — owner-expanded failure containment

The attached `goal-objective.md` expanded P4 with structured agent-readable tool failures, bounded exponential full-jitter startup retry, task-owned recovery and resume, Supervisor-visible run health, bounded local telemetry retention, and explicit security containment. The owner selected a maximum of five files per log stream and a seven-day age limit. ADR-0110 records that startup retry remains distinct from the shipped P3 provider-stream policy.

The current source audit found a high-severity authorization gap: autonomous workers could update or assign tasks belonging to other workers. The initial fix now restricts assignment tools to the Supervisor, limits worker status updates to their claimed in-progress task, and adds an atomic SQLite owner/status check for those worker transitions. Focused adversarial tests and the full suite are pending. The audit also found that writes before task claim and subprocess file mutations lack task-owned checkpoint attribution; P4-53/54 remain blocked until those paths are safely reconciled.

P4-50 memory and SQLite observability instrumentation has been extended, result counts/bytes now reach Metrics, and the SQLite profiler no longer labels an unobservable statement outcome as success. The completed full Rust workspace run reported 664 passed and four failures: two Windows process-sandbox/project-command cases (including missing `link.exe` in the inherited PATH), the custom recipe portability test, and one Windows Bash/AppContainer test timeout. That binary predates later source edits and is not final verification. A command-scoped Visual Studio/Windows SDK environment now compiles `arcana-core`; the desktop build and full latest-source suite remain unverified. The owner-only P4-49 calendar run remains untouched.

### 2026-09-29 continuation — contain panicking tool handlers

`ToolExecutor` now catches panics from both concurrency-safe and serial tool-handler futures and converts them to the standard bounded JSON error before normal telemetry, event, persistence, and model-result handling continues. The production executor test `handler_panics_become_structured_results_for_read_and_write_partitions` passed in `cargo test -p arcana-core --lib handler_panics_become_structured_results_for_read_and_write_partitions --locked` (1 passed). That test required a per-command Visual Studio environment plus the installed Windows SDK include/library paths; machine-wide environment was not changed. The test also exposed and fixed an existing `ctx`/`_ctx` compile error in `AssignTaskTool`. P4-51/TG4-25 remain open for complete builtin coverage, consumer correlation, timeout/cancellation, and privacy proofs. The latest full workspace run predates these changes and had four failures as recorded above.

The operational cadence register was corrected to match the live P3-09 provider retry implementation: three total stream-open attempts with 500 ms then 1 s delays and no current `Retry-After` handling. The plan now marks this source-grounded policy rather than the stale five-attempt/250 ms/8 s description.

### 2026-09-29 continuation — security review follow-up

The read-only P4-56 review found a Git mutation bypass via a shell-variable executable. The Bash guard was tightened for that pattern; `cargo test -p arcana-core --lib git_mutation_guard_tests --locked` passed, and `cargo test -p arcana-core --test s8_git_safety --locked` passed after the real-tool smoke added `G=git; $G commit`. It also found a likely checkpoint-list IPC contract mismatch; `gitStore.refreshCheckpoints` now supplies `taskId: null`, with `npm run typecheck` and `npm test -- --reporter=dot` green (8 files, 25 tests).

P4-56 remains open. The review's confirmed residuals are a time-of-check/time-of-use gap between worker claim verification and mutation, read/network access outside the workspace on Linux/macOS process sandboxes, and a checkpoint restore symlink retarget race. These need production-boundary fixes plus negative behavior tests or a precise documented scope with owner approval; they are not treated as passing because a static test or UI path looks safe. P4-49 is still owner-only, and no desktop session or calendar task was started.

`cargo run -p arcanactl --locked -- --help` built and ran the CLI help path. Its command list has no fleet-run entry, confirming that the current CLI cannot execute the production fleet. This records capability preflight only; no task, worker, or fleet CLI command that mutates persisted state was invoked. It does not satisfy the revised goal's required production CLI Fleet run.

### 2026-09-29 continuation — ACL rights and active-log retention

An independent Windows sandbox review found `AccessPolicy` deduplicated on canonical path and inheritance but ignored permission masks. A narrow grant could therefore suppress later Rustup read/execute access on that same path. The cache now stores and unions masks before deciding a grant is covered. `cargo test -p arcana-core --lib appcontainer_acl_cache_upgrades_permissions_for_an_existing_path --locked` passes on Windows, covering read-to-workspace rights and the Rustup-shaped traverse-to-read/execute upgrade; temporary profile/ACL cleanup ran when the test ended. The first launch's ACL fan-out remains measured only by the earlier interrupted corpus, so no latency improvement is claimed.

Retention now prioritizes canonical current-day readable/structured filenames over same-date legacy aliases and applies a deterministic filename tie-break. `cargo test -p arcana-core --lib policy_keeps_current_files_when_legacy_names_share_the_active_date --locked` passes on Windows using a test-only cap of one. TG4-29 still requires the production five-file/seven-day corpus, open-writer export, symlink/malformed-name behavior, and startup-failure reporting.

The 2026-09-29 P4 failure-path audit snapshot found: P4-52 startup retry absent; P4-53 requeued interrupted claims instead of resuming from a progress cursor; P4-54 had no production heartbeat and its DB update could change `working` to `active`; P4-51 used one conservative `retry_safe:false` value even for side-effect-free validation errors; and P4-55's production gate was open. The P4-52 and P4-54 heartbeat/run-state portions have since been implemented partially; current evidence and remaining recovery gaps are recorded in the later continuation below and ADR-0115. The established P4-56 findings (claim-check race, checkpoint retarget race, Linux/macOS outside-workspace read/network) remain open. No P4 task or gate has been closed; the owner-only P4-49 calendar task remains untouched.

The heartbeat DB boundary preserves `working`, `paused`, and `error` lifecycle states while recording `last_seen`; existing created-to-active behavior remains. The initial heartbeat-group run caught a regression in created-worker behavior, which was corrected, and all three heartbeat tests passed. **Superseding status (2026-09-29):** RecipeRunner now refreshes worker and durable run heartbeat every 15 seconds, records run phases and bounded task-attributed tool progress, and scheduler interruption recovery closes run records after claim release. The Supervisor-only `list_worker_health` reader is in the real FastDevelopment registry and returns current roster/run state, recent safe operations, and task-scoped checkpoint counts. Eight RunsDb tests, two health-reader tests, and all 13 RecipeRunner tests pass. Two-worker resume/reassignment and duplicate-side-effect prevention remain open; see ADR-0115 and TG4-28.

### 2026-09-29 continuation — startup retry boundary

P4-52 is now partially implemented in `RecipeRunner`: canonical workspace/key/provider/engine setup retries before `AgentEngine::run`, while the ephemeral session ID and runtime lease are allocated once outside the loop. The bounded policy uses at most four attempts, exponential full jitter, a 2.2-second per-delay cap, a 2-second total cap, and structured attempt/outcome telemetry without raw error text. Missing keys, invalid workspace configuration, unsupported providers, and current string-only Vault errors fail without retry; transient I/O, SQLite busy/locked, and startup panics may retry.

`cargo test -p arcana-core --lib startup_retry_tests --locked` now passes seven tests on Windows, including exponential full-jitter bounds, cancellation during delay, transient recovery, permanent failure, retry exhaustion, panic containment, and Vault non-retry. Earlier 13-test RecipeRunner evidence predates this turn. Startup preparation validates the workspace within the single retry boundary before deciding there is no provider, preserving fail-closed behavior. TG4-26 remains open for real worker claim/task-budget continuity and production-path no-replay injection.

### 2026-09-29 continuation — updated goal and full-suite gate triage

The owner’s revised `goal-objective.md` was read. Its tool-failure, startup retry,
mid-run continuity/Supervisor, retention, and security requirements were already
represented by P4-51–56 and TG4-25–30, so no duplicate tasks were added. The
latest full workspace attempt reached 132/135 `arcana` library tests; the three
failures were stale source scans in TG8-07 (runner scan ended at an early
`#[cfg(test)]` helper), P1-06 (redaction moved into `execute_handler`), and
P2-03 (worker constructor appeared after an early test helper). Production paths
were read and a Luna High read-only triage confirmed these are gate-shape defects.
The scans now assert the live delegation shapes; formatting and focused TG8-07,
P1-06, and P2-03 test groups pass. The workspace suite still needs a fresh rerun;
the PowerShell wrapper used on the first full run also needs to preserve Cargo’s
exit code across environment restoration. No product behavior changed in
this gate-fix slice, and P4-49 remains owner-only and unsubmitted.

### 2026-09-29 continuation — P4-50 consumer audit corrections

A GPT-6 Luna High read-only audit traced P4-50 producers through persistence, metric aggregation, and Logs UI. It found the tool producer emitted `output_bytes` while the aggregate read `result_bytes`, trace spans were grouped only by span names across runs, cross-tier Memory search and aggregate stats lacked timers, semantic extraction counted attempted writes as successful, and `open_tuned` installed SQLite profiling after its busy/WAL setup. It also found MCP registration logged raw endpoint URLs and reconnect attempts had no duration/outcome event.

The worktree now addresses those concrete mismatches: aggregation reads `output_bytes` with backward compatibility for older `result_bytes` records; trace roots use the strongest available trace/run/task/session/call ID and have an ID filter; Memory search/stats emit fixed-label duration records; semantic extraction reports attempted/stored/failed counts and uses `degraded` when writes fail; query text is removed from the `search_memory` debug log; SQLite profiling is installed before setup pragmas; MCP registration no longer logs endpoints, and health/reconnect records have safe categories and duration. `npm run typecheck`, all frontend tests (8 files/28 tests), focused Logs UI tests (5/5), `cargo fmt --all -- --check`, and `git diff --check` pass. Focused Rust tests and a fresh workspace test against these Rust edits remain pending.

The active `cargo test --workspace --locked -- --test-threads=1` session 94241 is confirmed live and CPU-active on Windows AppContainer tests. It was compiled before the P4-50 edits and therefore does not validate them. It has reported failures in project-command ecosystem/build tests, with exact diagnostics deferred until the core test target completes. Do not interrupt while ACL grants are being applied or cleaned up; no test-owned listening ports have been observed.

### 2026-09-29 continuation — current focused test state

The long workspace test is no longer active. It completed 686 passed / 7 failed from an earlier compile snapshot (not validation of this turn's Rust changes). This turn fixed the four failure classes and has passing targeted evidence for Windows scheduler recovery (14), persisted recipe portability (7), structured bounded-error guidance (1), production third-build diagnosis across three isolated Cargo runs (1, 300.93 seconds), core observability (9), semantic extraction privacy/counting (1), MCP safe error categories (1), metrics aggregation (1), SQLite producer-to-JSONL persistence (1), and production build-evidence/QC (1, three isolated builds, 208.29 seconds). The current-source checked-in ecosystem corpus test completed but failed after 399.87 seconds at the Rust Clippy lint step: Clippy returned `Access is denied` while finding its config inside the Windows AppContainer. Rust build/test passed; Node and Python fixture runs were not reached. A direct AppContainer test can read the workspace-local Clippy config file, but does not yet reproduce Clippy's config-discovery behavior. VS 2026 Build Tools and Windows SDK are installed; Cargo tests require a process-local VsDevCmd plus explicit SDK INCLUDE/LIB because the standard developer prompt did not populate those paths. No global environment changes were made.

### 2026-09-29 continuation — quarantine interrupted work

A fresh GPT-6 Luna High review confirmed that the scheduler released interrupted
claims to `ready`, so the Supervisor could automatically replay an effect whose
acknowledgement was lost. Production recovery now transitions only claims owned
by the reconciled PID/host to `needs_info`, clears those claim fields, preserves
the worker assignment, writes a `[worker recovery]` reason, emits a recovery
event, and wakes the Supervisor. The supervisor work probe includes this marker
while worker probes exclude it. The shipped Supervisor instructions require
health/checkpoint inspection and prohibit retry when effect completion remains
uncertain; unresolved holds remain visible for PA review. Focused tests pass for
DB owner/quarantine behavior, Supervisor/worker queue visibility, interrupted
run recovery, panic and timeout, the next real RecipeRunner tick, shipped prompt
delivery, and task-status handling. This is a containment step; P4-53 remains
open for a durable mutation cursor/intent ledger, repository snapshot,
lost-acknowledgement reconciliation, and non-duplicating resume.

The isolated Windows bounded-output test passed 1/1 in 430.42 seconds. It covers
one cold PowerShell launch and bounded concurrent stdout/stderr capture; the
duration is end-to-end and has no stage breakdown or warm same-lease comparison.
Do not attribute it to ACL propagation without measured stage timings or narrow
the grants. A GPT-6 Luna High performance review recommends a same-host cold vs
three-launch retained-lease benchmark with root/grant counters and per-ACL API
timings. The previous 686/7 workspace result is not current-source validation;
the ecosystem launcher still fails at Clippy config discovery with AppContainer
access denied, before Node/Python fixtures. No full workspace run is currently
active and no test-owned listener remains.

A separate CLI architecture trace confirmed `arcanactl` has no production Fleet
run command. `RecipeRunner` and `RecipeScheduler` are core-owned, but a CLI host
must compose the shared app state, provider/vault settings, analytics,
subagent/scheduler globals, structured JSONL telemetry, and a completion-aware
lifetime. The current CLI formatter and help path do not satisfy TG4-24. P4-50
must implement and execute that production path; desktop Calendar acceptance
remains reserved for the owner.


### 2026-09-29 continuation — CLI consent and bounded Fleet attempt

The production CLI now exposes `fleet run` through the shared host initializer
and `consent grant|revoke|status` for canonical workspace/provider scope. An
isolated subprocess test passes grant, readback, revoke, and post-revoke
readback across separate CLI processes. The real first attempt correctly failed
closed without consent and persisted a denied audit record. After explicit
consent, Gemini 3.8 Flash reached the Supervisor; the Supervisor exhausted its
50k-token/$0.25 run cap before decomposing the epic. The CLI now detects this
durable failure rather than waiting for timeout. The user approved an explicit,
bounded per-run Supervisor budget override; implementation and focused tests
are in progress. No successful coding task has yet completed through CLI Fleet,
and P4-49 remains reserved for the owner.

A token-persistence regression was found during that run: failed/budget-exceeded
provider calls were accounted for but token_usage was only written on success.
`record_spend` now persists nonzero usage for every terminal path; the focused
cancellation test passes and verifies the row by unique session ID.

The MDP count check currently reports 402 tasks (295 checked, 107 open) and
135 gates; the MDP and Testing Gates headers were stale and have been corrected.
`MDP-COUNTS-CHECK.py` must be rerun after all current edits.

### 2026-09-29 continuation — timestamp contract and credential recheck

The current live Gemini Models-endpoint smoke was rerun and passed using the
repository `.env` reader, confirming the new key remains the credential source
for this test. The Gemini 3.8 streamed tool-signature replay still receives
Google 503 Overloaded after bounded retries; this is a provider-availability
failure, not a key rejection. The saved vault value was synchronized from
`.env` in the previous checkpoint for production CLI/desktop runs.

P4-57 implementation is underway under ADR-0128: new database and API
timestamps use fixed-millisecond UTC RFC3339 with `Z`, while readers preserve
compatibility with offset-bearing RFC3339 and historical SQLite UTC strings.
The shared Rust helper now has a full leap-year/UTC-offset/DST-edge round-trip
corpus and a Rust-vs-SQLite canonical-shape test. Memory/session writers use
the shared formatter; session reads now propagate malformed timestamps and
have canonical-write/legacy-read tests. Frontend task/Fleet/hook/date-bucket
tests pass under America/New_York and Australia/Sydney. Database
defaults/triggers, analytics time-window comparisons, production DB readback,
and TG4-19 full-pair evidence remain in progress.

### 2026-09-29 continuation — credential rotation and CLI failure evidence

The owner clarified that live Gemini tests must use the new `GOOGLE_API_KEY`
from repository `.env` and authorized updating the saved vault value. A
short-lived helper loaded `.env`, wrote it through `VaultManager`, verified an
in-memory readback, zeroized both values, and was removed. The feature-gated
`p4_google_live_key_from_dotenv_is_accepted_by_models_endpoint` passes without
ambient-environment fallback. The Gemini 3.8 streamed tool-signature replay
smoke compiles but received repeated Google 503 Overloaded responses after
bounded retries; that behavior is not live-proven yet.

An isolated production CLI run with explicit workspace consent used the
synchronized vault value, created/read back an epic, and started all seven
seeded workers. Gemini 3.8 Flash repeatedly returned 503 Overloaded; the CLI
failed fast after 86 seconds with zero children. Task/worker state was readable
and the process exited. This revealed provider overload persisted as
`permanent`; RecipeRunner now reports distinct provider-overload, quota,
retryable, and fatal outcomes. The focused classifier test passes; a new
production run/readback after the change remains open. Fresh CLI bootstrapping
starts recipe-bound Created workers before they become routable as Idle; three
focused CLI tests pass. No code or logs were written under `C:\Playground`.
P4-49 remains reserved for the owner's desktop Calendar acceptance.

### 2026-09-29 pause checkpoint — P4 timestamp and Gemini key

The owner paused the goal at 16:37 +07:00. The active goal record is paused.
The new Gemini key was loaded directly from repo `.env`, written to the Google
provider vault slot, and verified by in-memory readback; the value was never
logged. The feature-gated models-endpoint smoke passed 1/1 from `.env`, with no
shell-environment fallback. Do not rerun the streamed Gemini tool-replay smoke
until provider overload clears.

P4-57 remains open. The timestamp filter now passes 25/25 serially, including
legacy DB reads for plans, tasks, and workers. Plans and worker migration tests
checkpoint/copy their legacy fixtures into distinct upgrade folders to avoid
colliding with the first migration's destructive-backup name. The focused plans,
workers, tasks, and app-settings test modules exited successfully. A production
`project_command` QC-evidence test was paused after 60+ seconds; it was neither
passing nor failing. It left a test fixture at
`C:\Users\razia\AppData\Local\Temp\arcana-project-command-build-5a76b6ff-ed89-4834-ae61-9e3f3e574efd`;
preserve that directory until cleanup is authorized. The test process and child
build processes exited; it opened no server port. Full-core test evidence is
still incomplete from the earlier interrupted contention-heavy run.

Resume details and remaining P4 gates are in
[_verify/P4-SESSION-CHECKPOINT-2026-09-29.md](_verify/P4-SESSION-CHECKPOINT-2026-09-29.md).
No commit or push was made; the workspace remains dirty on `main`.

### 2026-10-01 expanded P4 continuation — project execution trust boundary

The user's current security contract is in ADR-LOG-RESTRUCT-0133/0135:
project code, tests, dependencies, build scripts and descendants are untrusted;
build/test children have no network and cannot read arbitrary external files;
locked dependencies must be staged in-workspace; missing dependencies require
host-controlled installation approval. `project_command(intent="install")`
now resolves to a per-call `HostApproval` effect even under Autonomous mode,
cannot be covered by a saved tool allowance, and does not expose “Allow Always”
in the desktop permission card. Targeted executor/tool tests pass. This is not a
secure staged-installer implementation yet. Linux Landlock remains write-only,
macOS `sandbox-exec` still allows default reads/network, Windows still has
runtime ACL and Clippy launch failures, and no cross-platform denial proof is
complete. Keep P4-61/TG4-31 open.

P4-62 sets project command timeout to 3600 seconds by default and clamps to
7200; the worker scheduler deadline is two hours. Timing now distinguishes
sandbox preparation, process wait, output drain, and build-evidence persistence,
and includes the tool call ID. The 544s/0.81s ratio from an external analysis
is not comparable evidence. ADR-LOG-RESTRUCT-0134 corrects the prior claim of a
committed 600-second scheduler timeout: `HEAD` directly awaited push/cron runs.
The focused timeout test, `cargo check -p arcana-core --tests --locked`, eight
install-related core tests, `npx tsc --noEmit`, frontend permission-dialog
tests, format and whitespace checks passed. The first Windows timeout harness
was invalid because noninteractive `timeout.exe` returned immediately; the
replacement direct-child cancellation test passes using a bounded CPU loop.
An attempted descendant-spawn test was denied by AppContainer, so process-tree
cancellation remains unproven. Approved installs now fail closed before spawn
with structured network-capability denial until the accepted locked staging
path is implemented. Remaining P4-61/P4-62 proof includes cross-platform read
and network isolation, staged dependency consumption, trusted denial
readback, virtual time beyond ten minutes, production timing consumer
readback, cold/warm matched fixtures, and scheduler recovery. No commit or push
was made. Do not inspect, create, write or run anything under `C:\Playground`;
P4-49 remains owner-only.

Continuation verification 2026-10-01: `cargo check --workspace --tests --locked`,
`cargo fmt --all -- --check`, `git diff --check`, `npx tsc --noEmit`, all 38
frontend tests, and the focused approved-install fail-closed test pass. The
selected MSVC build smoke passes. The Windows Clippy sandbox test still fails:
the AppContainer reads the config bytes but cannot canonicalize the path
(`Access is denied`), so the Rust/Node/Python launcher corpus is not green. A
relative path and project-local config staging experiment did not fix it and
was reverted; no such staged config workaround remains. The descendant process
launch remains denied and does not prove recursive cancellation. P4 is still
open, no files under the owner's Playground directory were touched, and no
commit or push was made.

Additional Clippy falsification: upstream's config loader canonicalizes the
configured directory before searching for either supported filename. Setting
`CLIPPY_CONF_DIR` to `.` also fails inside the AppContainer with the same
permission error. This eliminates the simplest relative-current-directory
workaround; do not weaken the no-external-read contract or report Clippy as
passing to conceal it. The Windows sandbox boundary needs a supported way to
resolve the selected workspace path without granting broad ancestor access.

Continuation update 2026-10-01: per-call project install approval now has a
headless capability boundary. Tauri defaults to an available approval responder;
CLI Fleet explicitly sets `host_approval_available=false`, and ToolExecutor
rejects host-approval effects before pending-request creation or handler entry.
Tests prove immediate CLI denial and approved Autonomous install denial with a
structured network sandbox violation before spawn while dependency staging is
absent. Telemetry fixes avoid synthetic task ID 0, recognize hashed repository
graph DB files, and prevent correlation-scoped readable exports from including
unrelated day-log lines. SQLite callback telemetry currently reports an observed
duration but cannot identify success/failure; P4-50 remains open.

Continuation update 2026-10-02: the focused SQLite observability suite passes
4/4. Its isolated JSON subscriber captures an event from the production
profiler callback and verifies normalized database identity, operation
category, duration, and omission of SQL and bound values. The profiler cannot
observe statement success/failure, and the test does not prove production DB
reader/UI reconciliation or instrumentation cost; P4-50 remains open.
The shared logger test suite also passes 4/4; its default-filter case proves
fast `DEBUG` telemetry reaches JSONL while staying out of the readable log.

ADR-LOG-RESTRUCT-0149 corrects a stale P4-55 checkpoint statement: the Tauri
entry point already initializes logging before pruning `.arcana/logs`. The
remaining gap is isolated startup-order execution plus non-Windows symlink
proof, not missing production wiring.

Continuation update 2026-10-02: ToolExecutor now owns error-envelope enrichment
and uses one call ID from permission through execution, event, persistence, and
model result. It attaches available run IDs from AgentEngine. Unknown model tool
names and executor infrastructure failures are returned as failed structured
tool results. Bash timeout, spawn, and nonzero exits plus project-command
timeouts have distinct categories. Tests cover the production unknown-tool
provider consumer, Bash nonzero-output omission, envelope fields, and approved
install denial. The owner chose source-free compiler diagnostics; the filter
now returns bounded stable codes and workspace-relative locations, with focused
privacy and path-rejection tests. Python Build now uses standard-library
`compileall`, with Python tests separate per owner decision; production Python
execution remains unproven. Temporary non-inheriting Windows ancestor grants
are in place, and a focused AppContainer test denies sibling reads and loopback
network. Production Clippy still cannot canonicalize the readable config path
(`Access is denied`), so P4-11/TG4-18 and P4-61/TG4-31 remain open. Linux/macOS
read/network confinement, locked dependency staging, full error privacy matrix,
and successful CLI Fleet completion remain outstanding. No commit or push was
made, and the owner-only calendar directory remains untouched.

Continuation update 2026-10-02: a regression now retains a Google thought
signature when a later streamed update for the same part omits it. The focused
test exercises parsed SSE updates, `AgentEngine` stream assembly, and the real
Google request body, and passes. ADR-LOG-RESTRUCT-0140 records this
provider-agnostic opaque-token rule. A fresh isolated CLI trial after the fix
returned Google 503 `UNAVAILABLE` before tool execution. Read-only SQLite
inspection confirmed the run records `provider_overloaded`, the epic remains
`ready`, and no child tasks were created. This proves failure classification
and readback only; successful CLI completion and a live-confirmed signature
replay are still open. The verbose run produced a 3.4 MB SQLite-heavy log,
which is a P4-50 volume/filtering observation, not a default log-size claim.
The separate broader Windows ACL experiment was stopped after over 60 seconds
of high CPU while granting exact path ancestors outside the workspace. Its
unique ACEs on the test workspace, Temp, Local, AppData, and profile paths were
removed with exact-SID cleanup; the temporary AppContainer mapping was deleted
and verified absent. The source change was reverted to workspace-only grants.
ADR-LOG-RESTRUCT-0142/0143 records this retraction. Do not repeat the
outer-ancestor experiment; production Clippy canonicalization remains open.

Continuation update 2026-10-02: the owner confirmed Python Build should use
standard-library `compileall` and a separate Python Test intent for additional
confidence. Python 3.14.7 runs both on the checked-in fixture: the compileall
intent writes bytecode under ignored `target/`, and pytest reports 1 passing
test. Pytest 9.1.1 was installed in the user's Python environment to repair the
missing test runner. This is host-fixture evidence only; production sandboxed
Python execution and cross-platform worker proof remain open under P4-11/TG4-18.
The ToolExecutor now guarantees `suggested_action` on enriched failures by
preserving handler guidance or copying its bounded `next_step`; five focused
executor-context tests pass. P4-51 remains open for the complete error-class
matrix and planted-input privacy coverage.

Continuation update 2026-10-02: an independent P4 recovery audit found that
Tauri task assignment kicked the worker before persisting the new status. The
command now persists the transition first; its focused test reads the task at
kick time and sees the assigned owner/status. The ACP producer also now says
that a message is persisted with acknowledgement pending, including for an
unbound worker with no current recipe consumer. Its ToolExecutor proof reads
the durable row and confirms acknowledgement removes it from replay. Full
RecipeRunner dispatch/recovery and replay side-effect idempotency remain open.

Continuation update 2026-10-02: `retention_keeps_the_active_date_discoverable_and_fully_exportable`
passes through the production core log pruner and Tauri Logs date/full-file
export helper using only a disposable temporary directory. It proves the
active date and complete 3,005-line log survive count pruning and are written
to a full export. Concurrent
active-writer behavior and startup cleanup-failure proof remain open.

Follow-up 2026-10-02: `active_writer_remains_writable_while_retention_prunes_old_files`
holds the active log's append handle open while the production retention
function prunes an older recognized file, then appends and reads both markers.
`cargo test -p arcana-core --lib observability::log_retention::tests --locked`
passes 5/5 on Windows. A separate captured tracing-subscriber test proves the
public pruner logs cleanup-failure detail and its summary, then returns
normally. Startup integration and Unix/macOS symlink execution remain open.

P4-16 follow-up 2026-10-02: the saved-skill loader used the weaker executable
example validator and accepted `/tmp/` in persisted prose. It now uses the
strict persisted-guidance validator for descriptions and templates. The new
negative test passes; focused skill portability tests pass 3/3, core
portability tests pass 8/8, and shipped recipe/skill default corpus tests each
pass. TG4-17 still needs its full corpus and Linux/macOS proof.

P4-60 follow-up 2026-10-02: the Tauri PR-diff helper now proves denied fallback
without calling its analyzer and matching consent reaching a local analyzer
callback; `cargo test -p arcana --lib commands::diff_commands::tests --locked`
passes 3/3. The local callback avoids vault access and live provider setup. A
first test draft's claim that isolated `HOME` also isolated the Windows
keychain was retracted in ADR-LOG-RESTRUCT-0148; no further keychain-backed
attempt was made. Tauri IPC/UI and provider-backed allow/revoke proof remain
open. `src/tabs/Settings.test.tsx` now covers the consent card's
read/grant/refresh/revoke command contract and workspace-switch isolation
(1/1), and TypeScript passes. It mocks Tauri `invoke`; live desktop IPC and
the provider revocation race remain open.

P4 observability follow-up 2026-10-02: real isolated CLI Fleet failures showed
token usage and run outcomes persist, but `api_failures` had no production
writer. AgentEngine now records a bounded aggregate for a provider call with
failed attempts: stable error class, retry count, eventual recovery, and no raw
provider body. Two scripted production-engine tests read the result through
`AnalyticsQueries` and verify overload recovery and terminal quota behavior.
A live CLI confirmation is deferred because Gemini's free-tier request quota
was exhausted during the smoke; successful CLI Fleet completion remains open.

P4 recovery follow-up 2026-10-02: the Tauri task-status path persisted
assignment before its synchronous worker kick. An isolated integration test
drives the helper through a real RecipeScheduler and RecipeRunner, reads the
assigned task at kick time, and observes `idle_no_provider` without model
access. ACP response IDs are now explicitly persisted rather than described
as delivered; the unbound-recipient test exercises the production ToolExecutor,
reads its inbox row, and proves ack ends replay. P4-04 is checked with this
evidence; end-to-end ACP idempotency remains open.

The cross-phase audit also reconfirmed task-checkpoint restore risk after
uncheckpointed process writes and P2 daily-cap fail-open/concurrency gaps.
These remain open remediation items; they do not satisfy any P4 gate.

P4-58 query-plan work has eight populated-fixture assertions spanning daily-cost
range, task claims/orphans, task-board point/status/worker reads, latest worker
ticks, and worker-request status/time filtering. The plans select the date
primary-key, task status/assignment, worker-run-state, and measured
worker-request expression indexes without primary-table scans. The working
query inventory is `_verify/P4-58-QUERY-INVENTORY.md`; this is partial TG4-20
evidence.

P4-58 continuation: `WORKER_REQUESTS_BY_STATUS_SQL` is the production-shared
reader for status-filtered worker requests. A 160-row fixture with `ANALYZE`
proved the baseline query scanned and temp-sorted; migration 10 adds measured
`idx_worker_requests_status_created_at` over status and the existing
`julianday(created_at), id` order. Its focused plan regression passes. The
query inventory was expanded to include routing-history and affinity reads;
TG4-20 remains partial and the new-query registry is not yet enforced.

P4-58 analytics/run/ACP continuation (2026-10-02): populated production-SQL
plan tests now cover task claim/orphan and assigned/ready dispatch, task board,
ACP recipient/task/recent/stale inbox and count reads, worker requests,
analytics token/activity/worker/failure/event readers, fleet roster/progress,
routing history/affinity, exact-owner interrupted-claim selection, and open-run
recovery. Analytics migration v3 adds measured `julianday(timestamp)` indexes;
task migrations 11/12/13 add measured ready-dispatch, recent-routing, and ACP
recent/stale indexes.
Local-day UTC bounds distinguish canonical `T` rows from legacy space-separated
rows after a test caught accidental overlap; recent failure/event queries retain
malformed timestamps for strict parse errors. Analytics DB tests pass 7/7, the
ACP inbox/count and fleet recovery/health plan tests pass, and the ready-task
dispatch plus routing/affinity measured-index tests pass. Both worker-request
list variants now have production plan/result coverage. Diagnostics now
EXPLAIN each fixed count query and exercise the real five-table reader. Worker
summary plan proof reproduces the workers-added-today scan and confirms measured
`idx_workers_created_date` selection; exact roster counts remain explicit
full-count exceptions. P4-58/TG4-20 remain partial pending an enforceable
reader registry and the remaining in-scope reader audit.

2026-10-02 task-health query continuation: latest task build-evidence queries
and the shared `qc_review` transition guard now use production-shared SQL. A
populated 512-row test verifies both use the existing task/id index, reads the
latest passing result, and permits the actual QC transition. Focused test passes;
no migration was needed.

2026-10-02 query-registry correction: the owner-only `syn` inventory guard was
insufficient because a new SQL read inside an already registered method could
escape detection. The gate now compares exact normalized SQL fingerprints for
statically resolved SELECT/WITH reads across its six scoped production modules.
Its focused test passes 2/2 with 79 fingerprints. Independent tests now assert
actual latest worker-run owner/tick results, token totals/groups/top sessions,
and the exact blocked-task reader. Planner dispatch/dependency queries and the
worker-by-ID detail reader now have populated plan/result tests. Caller audit
boundedly excludes unused run helpers, planner presentation helpers, and worker
slug uniqueness validation; see ADR-0169. These additions close those specific
inventory gaps only; TG4-20 remains open for remaining query plan/result proofs.
See ADR-0168 and `_verify/P4-58-QUERY-INVENTORY.md`.

2026-10-02 Windows Python sandbox continuation: `platform::python_runtime`
stages the selected PATH interpreter's standard runtime plus a bounded pytest
closure into `%USERPROFILE%\\.arcana\\python-runtimes`; `project_command`
launches it with isolated startup flags and requests an exact read-only
AppContainer capability. The production Build/Test fixture passes, while
runtime-sibling reads, staged-runtime writes, and controlled loopback TCP are
denied. Cold/warm focused-test measurements were 42.61s/9.14s. Python fallback
selection, lint package availability, broader unrelated-file denial, and
Linux/macOS execution remain open; see ADR-0178 and the 2026-10-02 checkpoint.

Continuation after checkpoint 2026-10-02: the expanded P4 goal is active again. A fresh Luna High static review confirmed the recorded CLI provider quota failure, Windows-only Python production Build/Test evidence, and outstanding P4-50, P4-58, P4-61, and P8 boundaries. It found `GOAL.md` understated Windows Python staging; that is corrected to distinguish production-staged Build/Test proof from the still-open Ruff, fallback, dependency, denial-consumption, and cross-platform work.

P4-58 source registration no longer relies on seven literal Rust files. The AST registry discovers every Rust module under core DB, core analytics, and shared DB source roots; five settings, annotation, and schema-helper files are explicit, test-checked scope exclusions. `cargo test -p arcana-core --test p4_query_registry --locked -- --nocapture` passes 4/4, including nested-source discovery and inventory-exclusion checks. This is registration evidence only; embedded lifecycle plan/result evidence and the remaining audit still keep TG4-20 open. `cargo fmt --all -- --check` and `git diff --check` pass. ADR-0180 records the discovery decision.

The goal remains open. The latest known full core result is 804/806, with the two Windows AppContainer Clippy canonicalization failures; latest focused Python staged-runtime production Build/Test evidence passes, but the full core suite has not been rerun after those Python changes. Gemini quota is exhausted and the isolated CLI Fleet run created no child task, so no provider rerun is warranted. Do not access `C:\Playground\arcana-calendar`. No commit or push has been made.

2026-10-02 continuation: P4-58/TG4-20 is now closed after independent GPT-6 Luna High source/evidence review. The SQL registry fails closed on unresolved builders (with only the exact owner/template backup `VACUUM INTO` exception), fingerprints finite variants and migration/embedded reads, and validates direct proof/rationale text for each registered query. Registry 8/8, populated worker-run INSERT/UPDATE trigger test 1/1, ACP multi-message replay/ack test 1/1; formatting, core check, and diff check pass. Review found no remaining unpaired query variant. P4 overall remains active; P4-50, P4-61, P4-62, CLI Fleet live success, broader P1/P2/P3/P8 audits, and owner-only P4-49 remain open. Latest full core suite remains 811 passed / 2 failed on the known Windows AppContainer Clippy path-canonicalization denial pair. No provider call, Calendar access, commit, or push occurred in this continuation.

2026-10-02 P4-50 continuation: shared task-status transaction boundaries,
task claims, worker tick begin/phase/finish, and heartbeats now emit bounded
OperationTimer JSON with DB identity and available correlation. An independent
review caught and corrected premature success reporting before commit and
missing stale-owner failure events. Captured API tests prove success, empty,
and error outcomes; query registry 8/8, core check, format and diff checks pass.
Owner selected millisecond SQLite statement timings alongside nanosecond
logical DB operation spans. P4-50/TG4-24 stays open for broad producer coverage
and persisted real Fleet Logs readback. No provider or Calendar execution.

2026-10-02 P4-50 run-correlation follow-up: the existing worker `run_id` now
travels from `ToolExecutor` through `ToolContext` and the task-status tool to
run-aware `TasksDb` methods. Focused captured-event testing verifies the ID in
serialized status telemetry. Compatibility APIs remain for callers without a
worker run context. The SQL is unchanged and the P4-58 inventory now tracks the
new method owners. This closes a correlation gap only; P4-50 remains open for
broad producer coverage, real Fleet-to-Logs readback, telemetry failure
isolation, and cost evidence.

2026-10-03 P4-50 continuation: PlansDb public CRUD and task lifecycle transactions now emit result-aware timing JSON with fixed database names, nanosecond duration, safe row counts, and available task/worker IDs. Production-method tests cover create/list/missing-read/error and assignment/claim/refusal/quarantine/empty dependency reconcile with persisted-state reads (PlansDb 4/4; lifecycle telemetry 1/1). Task SQL statements now inherit lifecycle span fields. The Tauri correlation filter and Logs trace tree accept flattened span objects from the production JSON formatter; focused proof places a real task SQL event under its task trace. Logs UI tests pass 15/15 and the Tauri reader test passes 1/1. Other DB paths still need correlated spans. Transient vector embedding unavailability can still appear as successful empty recall. Full Fleet-to-Logs readback, failure isolation, and cost proof remain open; P4 remains active. No provider request, Calendar access, commit, or push occurred.

2026-10-03: AnnotationsDb CRUD lifecycle operations now emit safe result spans with nested SQLite correlation; annotation module tests pass 2/2 and verify privacy plus success/empty/error outcomes. `cargo check -p arcana-core --locked`, format check and diff check pass. This is incremental TG4-24 work only.

2026-10-03 follow-up: the prior note that transient vector embedding unavailability could look like an empty success is resolved in source. VectorBackend now falls back to scoped lexical FTS and marks recall degraded. `cargo test -p arcana-core --lib memory::backend::tests --locked -- --nocapture --test-threads=1` passes 9/9 including the fallback helper. The owner selected SQLite's bundled millisecond callback timing plus nanosecond logical operation spans (ADR-LOG-RESTRUCT-0186). The optional embeddings feature remains unbuilt; remaining observability coverage and end-to-end proof stay open.

2026-10-03 analytics result telemetry: `AnalyticsDb::insert_token_usage` and `get_token_usage_by_date` now emit fixed-label result events with the analytics DB identity, safe result counts, and duration. A temp-database production-path capture passes for successful insert/read and an injected trigger failure while excluding session/provider/model markers (1/1). Core check, formatting, and diff check pass; TG4-24 remains open.

2026-10-03: Worker roster reads and status writes now emit privacy-safe result spans (`database=workers`, elapsed time and row count). `cargo test -p arcana-core --lib worker_roster_and_status_operations_emit_safe_result_events --locked -- --nocapture --test-threads=1` passes 1/1 with a seeded in-memory WorkersDb and asserts worker names are absent. This is partial TG4-24 coverage only.

2026-10-03: `RunsDb` latest worker tick/owner/fleet-list readers now emit safe DB result spans. `cargo test -p arcana-core --lib worker_health_readers_emit_safe_result_spans --locked -- --nocapture --test-threads=1` passes 1/1 against populated isolated state. The offline optional embeddings build remains unverified because `aligned v0.4.3` is not cached; no download was attempted.

2026-10-03 QC freshness continuation: `ProjectCommandTool` rechecks the selected workspace fingerprint after every build command against the initial batch baseline and refuses provenance on changed/unavailable samples. File hashing now verifies path identity against the open handle (Windows volume/file index; Unix device/inode). Focused fingerprint and production ProjectCommand/QC tests pass. Independent Luna High review found a separate multi-project partial-batch bypass; this requires owner direction on additive batch-completeness persistence and keeps P4-14/TG4-07 open. See ADR-LOG-RESTRUCT-0189 and the 2026-10-02 checkpoint addendum. Do not access the Calendar workspace. No commit or push.

2026-10-03 P8 retest: after adding the newly required `ToolContext.run_id` to the S8-06 smoke fixture, `cargo test -p arcana-core --test s8_git_safety --locked -- --nocapture` passes 1/1 and the host `git_commit::tests` pass 11/11. This is current-tree test evidence only; commit and push remain unperformed.

2026-10-03 P4-42 initial production path (superseded by the closeout entry below): the first scheduler/RecipeRunner proof covered direct inbox acknowledgement; it did not yet include Supervisor stale-message nudge consumption. The expanded production-path regression below closes that gap.

2026-10-03 P4-52 bounded production retry: the scheduler/RecipeRunner QC regression now injects a one-shot transient preparation timeout before the provider call. It verifies one completed durable run row and exactly four scripted requests for one AgentEngine workflow. This does not prove task claim/budget identity or post-side-effect recovery.

2026-10-03 P4-42/TG4-33 proven: an isolated production-path test reads an aged message using the Supervisor stale-message tool, persists a QC nudge with the production sender, and runs the persisted QC worker through scheduler kick, RecipeRunner, AgentEngine and inbox acknowledgement. Both IDs are read back acknowledged and no repeat provider turn occurs after the queue drains. Unbound-recipient delivery remains visibly pending. No live credential/provider call was used.

2026-10-03 P4-50 continuation: the optional embeddings feature passes `cargo check -p arcana-core --features embeddings --locked` after fetching locked dependencies (30 compiler warnings remain). The Memory instrumentation suite passes 68/68 and annotation telemetry passes 1/1. The Logs trace tree now preserves repeated same-named operations as separate nodes when producer span IDs are absent; targeted Logs tests pass 17/17, full frontend tests pass 50/50, and `npm run typecheck` passes. Independent review keeps TG4-24 open for production Fleet-to-JSONL-to-Tauri-to-UI readback, successful CLI Fleet completion, external MCP call outcome telemetry, telemetry-storage failure isolation, and representative cost evidence. ADR-LOG-RESTRUCT-0191 records the UI identity rule and its limitation.

2026-10-03 P4-52 retry correlation: startup retry scheduled/recovered and retry-phase persistence events carry the durable run ID created before startup. The captured production event test passes 1/1 with the same run ID across attempt 1/retry_scheduled and attempt 2/recovered. Persisted Logs-reader readback and claimed-task budget continuity remain open.

2026-10-03 cross-phase audit refresh: current-source Luna High review leaves P1 graph/runtime provenance, P2 provider replay/spend lifecycle/timezone/vault proof, P3 consent IPC proof, and P8 AgentEngine-to-scheduler commit proof open. `search_memory` now returns redacted degraded status for empty and nonempty fallback results through a detailed recall API; degraded timing is marked `fallback`. Recall 10/10 and tool tests 3/3 pass, including real tool execute reading persisted MemoryDb data. TG3-17 remains partial because production active-backend absent/downloading behavior is not proven. See `_verify/2026-10-03-CROSS-PHASE-AUDIT-REFRESH.md`; the active readiness matrix remains authoritative.

2026-10-03 P4-50 source-audit correction: the owner chose MCP invocation in P4. The shared-core Streamable HTTP client and `mcp_list_tools`/`mcp_call_tool` tools now exist; local RMCP discovery/call and consent-revocation tests pass. This supersedes the earlier pre-implementation audit sentence. Desktop/CLI AgentEngine consumer proof, transport response bounds, cancellation/redirect behavior and persisted telemetry-to-Logs readback remain open.

2026-10-03 P4-59: worker-stat retention now has a focused second-month failure-injection proof; monthly insertion and raw pruning roll back atomically, and retry preserves the exact cutoff day. TG4-21 remains open for production cadence, migration preservation, active-writer, export, and non-worker append-only retention proof.

The production AnalyticsManager startup job is verified in an isolated home to apply the 90-day cutoff immediately, return historical coverage from monthly rows, retain the cutoff day as daily data, and stop/join cleanly (1/1). Remaining TG4-21 scope: deployed migration preservation, concurrent writer safety, full export, and the other append-only table policies.

2026-10-03 P4-50 producer-to-reader evidence: `OperationTimer` emitted a real tracing event into JSONL and Tauri's structured-event and incremental metrics readers consumed it with correlation, duration, outcome and count intact. The focused Tauri test passes 1/1. This is not live desktop IPC/Fleet evidence. Owner chose native SQLite statement timing at millisecond resolution with finer nanosecond operation spans (ADR-LOG-RESTRUCT-0186).

2026-10-03 Logs reader fixture: on a 7,492,780-byte JSONL with 25,000 structured DB events and 1,000 appended events, three warm runs measured median initial metrics refresh 559.4 ms, incremental refresh 25.3 ms, tail-2,000 read 7.49 ms and full read/export 3.52 ms. Reader-only synthetic evidence; excludes desktop IPC, React rendering and concurrent load. See TG4-24 amendment history.

2026-10-03 Memory producer audit/fix: direct PA `get_memory`/`list_memories` results have safe correlated timings; not-found records an observed zero; list truncation uses raw lookahead and malformed counts are separate. Procedural search distinguishes malformed tag sets from dropped rows. `record_access` has a distinct result-aware event and remains best-effort; `search_all_detailed` production-path regression proves recall results survive a failed history update. Focused tests and ADR-LOG-RESTRUCT-0193/TG4-24 contain evidence. Other DB/Memory producers remain under audit.

2026-10-03 config.db observability: app-settings, model-config, and Arcana-instructions repository operations emit fixed-label result counts/outcomes on the existing logical timer; sensitive keys/values, model IDs, paths, and content are omitted. Tests cover found/missing rows, writes/deletes, and an injected settings SQL failure. ADR-LOG-RESTRUCT-0195 and the TG4-24 amendment list exact evidence. Full DB producer coverage and Fleet-to-Logs UI readback are still open.

Operation timing JSON now has `result_count_observed` and `result_bytes_observed` flags so a measured empty result is distinguishable from a producer that never measured a count. Focused timing tests pass 4/4; existing numeric fields remain compatible. ADR-LOG-RESTRUCT-0194.

2026-10-03 CLI Fleet failure-path evidence: isolated CLI consent grant/status worked, and two real `gemini-3.8-flash` Fleet attempts made 3 and 7 provider calls before exceeding their post-response 32k/100k run-token caps. Reported use/cost: 43,871/~$0.0337 and 106,667/~$0.0625. Neither decomposed the epic (zero children); the second persisted telemetry and was read through `logs list`, `logs export` (948,715 bytes), `analytics tokens`, and `task get`. This demonstrates failure-path startup/provider/log-reader behavior only, not a successful CLI coding task or Logs UI roundtrip. Atomic conservative reservations are the approved policy for the separate daily USD ceiling; its TG2-17 provider/restart/estimate/kill-switch proof remains open. The run-token cap's pre-dispatch enforcement remains unresolved; do not repeat provider calls until it is bounded. The new credential was imported into the isolated profile without printing it; Calendar was not touched.

2026-10-03 operation-journal claim linkage: because the worker engine is created before it knows its task, the executor now copies only a positive requested `claim_task.task_id` into the intent row. The claim tool labels failures after the ownership write as `unknown` when budget readback fails. The isolated production writer/readback test passes 1/1. This is safe attribution, not recovery proof; journal reconciliation and requeue gating remain open.


2026-10-03 P4-54 health slice: list_worker_health now reads the currently active journaled operation only for an open running worker run and reports safe operation label, start time and elapsed milliseconds. The production Supervisor reader regression passes 1/1. Retry state, model/token usage, hierarchy and recovery continuation remain open; no gate is closed by this slice.

2026-10-03 P4-50 MCP cancellation instrumentation: shared-core production MCP handlers now wrap their futures with a drop guard that records one correlated cancelled outcome; the full focused module passes 8/8 and normal completed calls do not add another event. Remaining: remote cancellation acknowledgement, configured AgentEngine desktop/CLI invocation proof, pre-parse response bounds, redirect rejection, and telemetry persistence/Logs readback. RMCP 3.5.0's custom-client trait is implemented for reqwest 0.13 while Arcana directly depends on 0.12; a pinned direct alias is pending owner direction before any redirect-policy implementation.
2026-10-03 MCP AgentEngine consumer proof: `AgentEngine` in FastDevelopment mode now calls the configured MCP list/call tools through the production executor against a local RMCP server, with isolated config/consent/run databases. The test verifies remote request counts, model-visible tool result, and durable success outcomes labeled `mcp_list_tools`/`mcp_call_tool`. Desktop/CLI integration, redirect/body limits, telemetry-to-Logs proof, and full MCP acceptance remain open.
2026-10-03 P4-15 streak proof: the production TasksDb batch API now has a regression proving concurrent failed project-command exits count individually, the third failure engages diagnosis, and success resets the streak. This is database/exit-accounting proof; provider-driven repair and fleet retry remain open.
2026-10-03 P4-51 executor contract/privacy follow-up: permission denial, approval timeout, session cancellation, and closed approval listener now produce distinct correlated failure codes and remain fail-closed. The shared envelope filter removes exact echoed command/prompt/source/path argument strings from handler cause, corrective action, and diagnostic path fields while retaining Arcana-generated command identity. Focused envelope/privacy tests pass 8/8 and executor lifecycle tests pass 15/15. The all-builtin/error-provider inventory and broad planted-input audit remain open.

2026-10-03 P4-62 timeout-stage observability: project-command execution emits correlated fixed-label runtime-preparation, sandbox-preparation, process-wait and output-drain stage events. RAII timing events are emitted on future drop, so the active stage duration remains visible when Tokio cancels a command at its timeout. The Windows production timeout regression captures `process_wait` as `incomplete` with tool-call/task correlation while also proving descendant writes stop. Focused timing-event and timeout tests pass 1/1 each. P4-62 remains partial for CLI provider-backed deadline/shutdown, evidence-persistence comparison, cross-platform timings and repeated cold-profile variance.

2026-10-03 P4-53 recovery barrier: RunsDb now uses an immediate transaction for task-attributed side-effect intent check-and-insert and refuses new task side effects while an earlier non-board outcome remains unknown/unresolved. The real ToolExecutor returns `operation_reconciliation_required` without calling the handler; after explicit `RetrySafe`, it proceeds. A unique partial index allows one reconciliation record per intent. Focused executor, sequential retry, simultaneous-writer, single-winner and migration tests pass. P4-53 remains open for production reconciliation UI/Supervisor action, task/workspace/Git snapshot comparison, complete mutation capture, and replacement-worker continuation/no-duplicate proof.

2026-10-03 P4-53 follow-up validation: worker-health journal fixtures now respect the single-unresolved-effect contract (6/6); build-evidence tests use the atomic project-batch contract (9/9); tool identity source audit passes (1/1); both legacy task migration fixtures pass after removing the v19 trigger before migration replay (1/1 each). `cargo check --workspace --locked` and `cargo fmt --all -- --check` pass. Full `cargo test -p arcana-core --lib --locked -- --test-threads=1` rerun completed 914 tests: 911 passed, 2 failed, 1 ignored in 815.22s. The two remaining production launcher failures both reproduce at Windows AppContainer Clippy config canonicalization denial (OS error 5); all seven earlier fixture failures are green in focused groups. P4-11/P4-61 remain open.


2026-10-03 P4-14 mixed-project production proof: `production_mixed_project_batch_requires_every_child_to_pass_before_qc` runs the real ProjectCommandTool over detected Cargo + nested Python projects using the production Windows sandbox and staged signed Python runtime. With valid Rust and invalid Python, both commands return observed exits (expected=2, observed=2, passing=1), the persisted batch is `failed`, and the real task-status tool refuses `qc_review`. After repairing Python, both projects pass (expected=2, observed=2, passing=2) and the real status tool accepts QC. First run: 1/1 passed in 103.55 seconds; post-format repeat: 1/1 passed in 73.29 seconds. This does not close P4-14/TG4-07 because multi-project timeout/spawn/cancel and cross-platform corpus coverage remain.

Post-format revalidation update: `cargo fmt --all -- --check` passes. The same production mixed-project regression passes again (1/1, 73.29 seconds) after formatting. Therefore both recorded test executions pass; P4-14/TG4-07 remain open for timeout/spawn/cancel cases and cross-platform production proof.

P8 current follow-up (2026-10-03): the earlier missing model-produced commit
consumer proof is now covered by
`model_produced_commit_runs_through_recipe_runner_and_host_guard`. A scripted
provider drives the persisted shipped backend recipe through scheduler,
RecipeRunner, AgentEngine and the host `git_commit` tool; task claim, exact-file
commit, returned tool result, completed run and success-journal readback pass
1/1. The host suite passes 15/15, Bash guard, S8-06 smoke and Tauri registration
each pass, and independent GPT-6 Luna review found no material gap. P8-74/TG8-13
are proven. P4 remains open; see the current session checkpoint for validation
and remaining blockers.

2026-10-04 P4 Windows staged-Python isolation correction (ADR-LOG-RESTRUCT-0242): the apparent SBOX launch hang was a test deadlock. The bridge completed with exit 0, but the test awaited stdout/stderr EOF before `child.wait()` finalized and closed those streams. Concurrent draining plus wait fixes the test. The exact staged-runtime isolation test passes 1/1 in 38.46s; focused Windows process-sandbox tests pass 16/16 in 96.37s. This does not close P4-11/P4-61/TG4-18/TG4-31 because production Clippy canonicalization, copy-back/dependency staging and other OS backends remain open; it also does not close the full-core or TG4-32 CLI/performance clauses.

2026-10-04 full core rerun: 962 `arcana-core --lib` tests ran, 959 passed, 2 failed, 1 ignored (645.51s). The staged Python sandbox test deadlock is fixed: both output drains and bridge `wait()` must be polled concurrently. The only full-run failures are real Cargo Clippy in the production Windows sandbox and the checked-in project-command corpus, both due `Path::canonicalize()` returning OS error 5. An explicitly read-only config stage in Arcana profile storage also fails canonicalization and was removed. Two ancillary failing fixtures were corrected (unknown-only journal reconciliation; UTC dashboard date) and pass focused tests. P4/TG4 totals remain unchanged at 32/59 and 10/33; don't claim the full core gate passed.

Cross-stack closeout for this continuation: workspace `cargo check --workspace --locked`, `cargo fmt --all -- --check`, `npx tsc --noEmit`, frontend `npm test -- --run` (12 files, 52 tests), and `git diff --check` pass. No test listener remains. The full core test gate remains red for the same two Windows Clippy path-canonicalization failures; P4/TG4 counts stay 32/59 and 10/33.

2026-10-04 bounded SBOX bridge output: `bridge_stream_capture_keeps_a_bounded_tail_and_total_byte_count` proves 256 KiB tail retention while preserving total observed bytes; the bridge-focused suite passes 5/5. This does not resolve Windows Cargo Clippy canonicalization or close P4/TG4; counts remain 32/59 and 10/33.

## Current P4 continuation (2026-10-04)

- Active source of truth: `MASTER_DEVELOPMENT_PLAN.md`, `TESTING_GATES.md`, `READINESS_MATRIX.md`, owner-authored `P4_EXPANDED_GOAL.md` (pending read until core P4 closes), and `_verify/P4-SESSION-CHECKPOINT-2026-10-04.md`.
- Recent implementation commits pushed to `main`: `a14cf6f` fixes Memory recency ordering; `161283e` reconciles AnalyticsDb retention query evidence; `6fd6926` refreshes the checkpoint; `d2f833b` fixes semantic Memory chronology; `e4fd313` verifies worker request/message timestamp readback; `dc8fcc2` verifies build-evidence timestamp readback; and `2cc7bd6` fixes Linux sandbox-test/Ruff host portability. Two older commit bodies contain literal `\n\n` separators; published history was left intact.
- Current documented phase count remains P4 32/59 and TG4 10/33. P4-58/TG4-20 query registry passes 8/8 on current source. P4-57/TG4-19 remains partial; Linux retention tests now pass 19/19, while macOS active-writer evidence for P4-59/TG4-21 remains pending. Full `arcana-core` test baseline from 2026-10-04 is 959 passed, 2 failed (Windows Clippy path canonicalization, OS error 5), 1 ignored.
- Latest focused changes also cover semantic Memory ordering, task communication timestamp readback, and production build-evidence timestamp readback. Their focused Rust tests pass on Windows and selected production timestamp readers pass on Linux (9/9). WSL Ubuntu has user-scoped Rust 1.99 and native build prerequisites; Linux retention tests pass 19/19, with focused Landlock, structured Ruff-refusal, and timestamp helper property tests passing. Use `$HOME/arcana-p4-linux-target` for persistent Cargo output: WSL removes `/tmp` between distro sessions. Broader Linux sandbox/corpus proof and macOS evidence remain pending. Calendar acceptance stays with the owner and the target project is untouched.
- Do not read `P4_EXPANDED_GOAL.md` until all core P4 engineering tasks/gates close; then execute it before the final owner Calendar acceptance.

Current 2026-10-04 checkpoint: P4-55/TG4-29 closed with real isolated desktop startup, production full readable export and clean shutdown; core P4 is 33/59 and TG4 is 11/33. ADR-0276 fixes concurrent migration/version checks, legacy column repairs, WAL initialization contention and automatic backup collisions. See `_verify/P4-55-RETENTION-CLOSEOUT-2026-10-04.md`. Long fixed operation/event names are over-redacted in structured JSONL; this newly observed P4-50 defect remains open. Expanded goal remains unread until core P4 closes.

2026-10-05 resumed audit: HEAD 0926c6f is pushed; core tally 37/59, TG4 14/33. Fresh Luna High reports identify actual timestamp reader defects (Memory stats, Runs chronology/owner, Fleet Chat) and partial daily-cost overwrite during pruning; main-session regressions reproduce them. Tests remain dirty/red pending owner disposition, not phase proof. CLI Fleet cleanup currently disabled: earlier desktop+CLI lifecycle narrative corrected. Populated-file migration proof is a separate unit. Details/answers pending in the 2026-10-05 checkpoint; expanded goal stays unread.

2026-10-05 verified checkpoint: populated analytics/config upgrades pass Windows/Linux 2/2 and are committed as 0df119a. Workflow portability closes on Windows/Linux/macOS. Sandbox corpus fails on Windows CI without processmodel.dll (OS error 126), and macOS correctly refuses execution pending the signed XPC runner. These are distinct from the passing TG4-17 gate. Timestamp/cost-history fixes, startup deadline, correlation UUID and live smoke owner decisions remain pending.

## Paused 2026-10-05 desktop-consent checkpoint

Goal paused by owner; main/origin main at 186737a. Current tally 38/59 P4, 17/33 TG4. Real native Settings grant/restart/workspace/revoke and denied structural diff scenario passed three times; provider method matrix passed Windows/Linux 6/6. Full desktop harness still fails its first fresh-profile native session (DevToolsActivePort missing); explicit-port diagnostic was deliberately stopped, not proven. Driver install is approved and completed locally in ignored target/e2e-tools. Consent changes remain uncommitted alongside separately known-red chronology/retention regressions. Resume exact steps, raw evidence, remaining decisions and process cleanup are in _verify/P4-SESSION-CHECKPOINT-2026-10-05.md, final Paused checkpoint section. Expanded goal unread; owner Calendar untouched. No test listener remains.

## P4 resumed after desktop-consent checkpoint — 2026-10-05

Owner resumed the active core-P4 goal. Current tally is unchanged at 38/59 P4
and 17/33 TG4. P4-59 exact-session retention now seals dates after raw rows are
pruned and expires per-session IDs; additive SQLite migration 8 also blocks
updates that would move a retained row into a sealed date. Focused retention
tests pass 23/23 on Windows and Ubuntu WSL; P4-58 registry passes 8/8. CLI
Fleet now owns this maintenance while it runs and stops/joins the thread on
return. P4-57 focused RunsDb and Memory timestamp regressions pass on Windows.
Details and remaining limits are in `_verify/P4-SESSION-CHECKPOINT-2026-10-05.md`
and the current plan/gate/matrix. macOS retention matrix and full TG4-21/TG4-19
coverage remain open; expanded goal is unread and Calendar was untouched.

2026-10-06 P4-51 verified slice (ADR-0327/0328/0329): malformed streamed arguments now fail before handler dispatch, while valid empty calls still run. Provider stream recovery is recorded only after full consumption; terminal/body failures produce one privacy-safe analytics aggregate and no retry after output. Windows/Linux pass engine26/26, production executor18/18, external MCP9/9 and provider129/129; the thirteen-tool file/task/epic/budget matrix plus separate two-tool MCP success/revocation fixture is verified against the actual47-name registry. Seventeen concrete builtin failures reach the model as correlated failed results. The additional ACP9, artifact3 and board6 registered-tool matrices pass on Windows/Linux, bringing deterministic success/failure coverage to33 of47 registered tools. The ACP negative fixture reproduced raw SQLite diagnostic leakage; ADR-0331 removes it from send_worker_message without changing delivery semantics. The surrounding ACP module passes5/5 on Windows/Linux (2.47s/4.44s). Core cargo check and formatting pass; every-builtin/error-category coverage remains open. Owner-approved ADR0332 now preserves tool JSON scalar types, redacts string values with the existing text policy and masks sensitive fields. Windows/Linux redaction22/22, Memory1/1, routing1/1 and executor18/18 pass through the real registered tools and readers. These five registrations raise deterministic success/failure coverage to38/47. Proof: `_verify/P4-51-JSON-REDACTION-2026-10-09.md`. Evidence: `_verify/P4-51-REMAINING-CONTRACT-2026-10-06.md`, `_verify/P4-51-BUILTIN-EXECUTOR-MATRIX-2026-10-06.md`, `_verify/P4-51-BOUNDARY-REVIEW-2026-10-06.md`. No task/gate/readiness closure or rescore: P4 40/59, TG4 19/33. No paid run or owner Calendar activity; expanded goal remains unread.
