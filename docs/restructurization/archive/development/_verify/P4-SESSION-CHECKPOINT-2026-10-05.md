# P4 session checkpoint — 2026-10-05

## Current mandate and state

Complete core P4 in the MDP, testing gates and readiness first, committing/pushing logical units; only afterward read and finish the user-owned untracked `P4_EXPANDED_GOAL.md`. Owner alone runs final Calendar desktop acceptance; do not access that workspace. All subagents must use GPT-6 Luna High. Existing agent thread limit prevented a fresh review; no independent phase-review claim is made.

Current counts at owner pause: **38/59 P4 tasks; 17/33 TG4 gates**. Remaining tracked tasks: 21 including owner Calendar P4-49. Full phase remains open. Earlier counts below are historical; resume from the final Paused checkpoint section.

## Completed claim-safety unit

P4-40 production writer/caller audit and late manual-release race are complete. Raw independent SQLite connection cannot forge inconsistent claim metadata; competing workers admit only one winner; claim tool records the actual process PID/host with isolated RunsDb; automatic and manual stale release preserve replacement claims. Three real Tauri command tests pass. Rebuilt CLI smoke proves claim/refusal/failure/readback with actual process PID. Evidence: `P4-40-CLAIM-SAFETY-CLOSEOUT-2026-10-05.md`; temporary smoke profile: `%TEMP%/arcana-p4-claim-cli-7wh1_62v`. No API or listeners used.

## CI prerequisite verified

Run 37218542575 passed frontend/CLI checks and desktop/core library tests, then failed S2-06's old structural check requiring `hydrate_today()` inside desktop main. The approved shared initializer still invokes hydration. ADR-0283 updates the guard to trace desktop and CLI startup into shared hydration and adds persisted same-day/old-day behavior readback. Windows S2 suite 9/9 and shared runtime tests pass; Linux S2 suite 9/9 and shared runtime group 5/5 pass. No production spend behavior changes.

## Next open milestone

Push the verified CI guard unit, then inspect the three-host saved-workflow portability matrix before closing P4-16/TG4-17. Continue remaining core tasks, especially sandbox corpus, complete recovery and live CLI Fleet-to-Logs proof. Prior bounded Gemini Fleet run exhausted quota before child task completion; do not silently rerun against the key. Registered telemetry labels were approved and fixed in 30518b7; canonical UUID correlation-field preservation is a separate pending owner decision. Calendar and expanded goal remain untouched.

P4-40 logical unit committed as `67fd7ab`; CI guard is a separate test-only unit. Local Windows shared runtime filter passed 11/11 (5 runtime and 6 Python runtime tests) in 117.99s; Linux shared runtime group passed 5/5 in 0.26s. All tests use isolated homes.

## ACP replay unit closed

P4-23, TG4-10, TG4-11, S4-04 and S4-05 are checked after native producer/recipient, stable-ID replay, completed-effect reconciliation and ownership refusal proof. Windows/Linux tests pass; the saved Supervisor policy reaches the actual consumer and one observed task mutation survives interruption/redispatch. See `P4-23-ACP-REPLAY-CLOSEOUT-2026-10-05.md`. Existing bounds follow ADR-0095; no new per-sender rate limit. Current tally is 36/59 tasks and 13/33 gates. CI run 37220225970 at ccef776 later failed the checkpoint fixture setup as detailed below; frontend/CLI passed.

## Remote CI fixture correction in progress

Run 37220225970 finished with frontend/CLI success, but the core library had 983 passes and one fixture setup failure: config.db already existed before the checkpoint-refusal test could construct its blocker. ADR-0285 prepares the blocker before publishing HOME; no production checkpoint rule changes. A local WSL core-library run uses CI's four threads in an isolated outer Arcana profile, followed by all core integration targets with live-smoke disabled and live keys unset. P4-23 commit is `8ff0b5f`; push follows this verified prerequisite checkpoint.

## Verified prerequisite checkpoint and current retry work

ADR-0285 Windows checkpoint regression passes 12/12; Linux core library passes 985/985 (four threads, 86.68s), and all core integration targets pass 44/44 including the CLI/host smoke readers. Logs persist in ignored `target/ci-core-parallel-current.txt` and `target/ci-core-integration-current.txt`. The first local attempt lacked Linux npm after its temporary runtime disappeared; official Node 24.21.0 was restored to `/home/razi/arcana-p4-linux-tools`, checksum-verified, with Python3 alias and Linux-only PATH. No test package was installed. This checkpoint changes no P4 tally.

P4-52 next: ADR-0286 adds test-only exhausted startup budget/reservation preservation and failure after real host commit. Initial new test used an incorrect Task field name (`claim_pid`); corrected to live `claimed_by_pid`, not a product bug. New tests remain unverified/uncommitted. Independent review spawn again hit the agent thread limit; no review is claimed. Expanded goal and owner Calendar remain untouched.

## P4-52 verified unit and next decision

ADR-0286 worker preservation/post-commit group passes Windows 3/3 (4.85s), Linux 3/3 (4.67s). Policy group passes 9/9 on both; real redacting writer → actual CLI export reader passes 1/1 on both (1.44s/1.34s). Workspace check and format pass. Source changes are test-only. P4-52/TG4-26 remains open because the synchronous setup closure can exceed the two-second limit (ADR-0287); owner question is pending. Counts remain 36/59, 13/33.

ACP `8ff0b5f` and fixture `0f18f53` are pushed to established origin/main. Remote CI 37224469153 is running at that checkpoint: frontend and CLI green, Rust workspace tests pending. Do not cancel that portability attempt with incidental pushes. Reused timestamp agent could not verify its own model restriction and stopped without writing; no fresh model-compliant review is claimed. Next independent bounded work: saved-task/daily-cap interaction under P4-29/TG4-15, then remaining core task/gate milestones.

## P4-29 / TG4-15 closeout

Saved-task and daily-budget interaction is proven (ADR-0288): production Engine/ClaimTaskTool/CappedProvider exact-cost/refusal/persisted-reader cases pass within the 10/10 engine group on Windows/Linux; workspace check passes. Task29/gate15 are checked; tally 37/59, 14/33. TG2-17 remains separately open. Proof: `P4-29-BUDGET-INTERACTION-CLOSEOUT-2026-10-05.md`.

Remote run37224469153 passed Rust workspace, frontend and CLI. macOS then fails compiling the old synchronous refusal test because spawn is async. ADR-0289 changes only that fixture to Tokio async + await; no macOS execution capability is enabled. Windows/Linux saved-workflow matrix steps were still running at the last check. Retry unit `5e4a5f7` is committed locally, not pushed while the matrix is active. Owner decisions pending: hard startup deadline versus retry admission semantics, canonical correlation UUID preservation, and one new bounded live CLI Fleet smoke ($1/120k/20 turns/10-minute epic). Do not spend while that new live approval is pending.

## Three-host CI results and prerequisite repairs

Run 37224469153 finished: Rust workspace/frontend/CLI pass. Windows and Linux persisted-workflow portability and active-writer retention steps pass; Linux build/launch smoke passes. macOS core test compilation fails on a missing await in the old fail-closed launcher fixture (ADR-0289). Windows command corpus fails before first command because that job has not built its required arcanactl.exe bridge (ADR-0290). Both prerequisites are repaired without relaxing production policy; CI correctness gates pass 3/3 and CLI builds locally. New remote matrix proof is required. P4-16/TG4-17 and P4-59/TG4-21 remain open until missing platform/contract evidence is available. Counts remain 37/59 and 14/33.

## Resumed checkpoint — audited consumer defects with failing proof

Push complete: 5e4a5f7 (retry boundary), b7170c7 (task/daily budgets), 0926c6f (macOS async-test and Windows CI bridge prerequisites) are on origin/main. CI run 37226796142 at 0926c6f is active. Counts remain 37/59 P4, 14/33 TG4. Fresh audit work used only GPT-6 Luna High; no phase-close audit claim.

Uncommitted test-only source: Memory get_stats legacy/mixed-time regression; RunsDb recent/open/latest-worker/owner/roster chronology regressions; Fleet Chat hydration regression; two advancing-cutoff retention-cost regressions. All reproduce existing consumer defects on Windows, intentionally failing before approved production fixes. Do not commit/push these red regression cases as a verified closeout. Populated analytics/config migration cases are separate, passing on Windows; Linux verification passes 2/2 (0.55s). Reports are the P4-57/P4-59 LUNA-CLOSEOUT-AUDIT files. ADR-0291/0292/0293 records decisions, reproduction and corrected CLI lifecycle claim.

Required pending owner decisions: fix timestamp audit findings and additive RunsDb indexes; durable cost-summary coverage/reconciliation; owned Fleet maintenance disposition; earlier startup hard-deadline versus retry-admission contract; canonical correlation UUID preservation; new bounded live CLI Fleet trial ($1/120k tokens/20 turns/10-minute epic). Do not spend, change these contracts or weaken sandbox while answers are pending. Calendar workspace and untracked expanded goal remain untouched. No servers/listeners started in this resumed checkpoint. Next: finish upgrade proof/verified unit, integrate owner-approved fixes, reprove remote matrix, then remaining core P4 contracts.

## Workflow portability closeout and platform execution gap

2026-10-05 closeout: all six TG4-17 filters pass on Windows, Linux and macOS in CI run 37226796142 at 0926c6f (16 checks per host). Production saved recipe/skill/hook readers and writers accept portable guidance and refuse prohibited executable examples before process launch. P4-16/TG4-17 are checked; command execution portability remains independently open. Proof: _verify/P4-16-WORKFLOW-PORTABILITY-CLOSEOUT-2026-10-05.md. Current tally: **38/59 P4; 15/33 TG4**, 21 tasks remain including owner Calendar. Migration preservation unit is committed as 0df119a; advancing-cutoff and chronology tests remain intentionally red and uncommitted awaiting owner decisions. CI passes workspace/frontend/CLI and Linux build/launch. Windows execution corpus fails because the runner lacks processmodel.dll (OS error 126); macOS refuses execution until the approved signed XPC runner is built. No sandbox policy is weakened to hide these failures.

## Dependency dispatch and Linux network checkpoint

Current tally: **38/59 P4; 16/33 TG4**. TG4-06 closes from the controlled-provider scheduler integration on Windows/Linux, including actual dependent completion and budget-sibling refusal. P4-39 remains open for explicit budget resolution (owner question pending). Linux seccomp endpoint denial, deliberately inherited socket closure, descendant denial and x32 refusal pass 3/3 (0.64s); actual Rust/Node/Python command corpus passes 1/1 (15.37s). Workspace check/format pass. Read isolation, dependency staging and Windows/macOS execution remain open. Full evidence and scoped Luna High review are in the two new verification records; no new live API spend or Calendar access. Four known-red timestamp/retention regression files remain uncommitted awaiting owner disposition.

## Timing correctness checkpoint

`17f073b` and `ed69cb0` are pushed to origin/main. Tally remains 38/59 P4, 16/33 TG4. Corrected overlapping project-command stage timers (ADR-0299): the real 454.6ms call had incorrectly included 418.8ms sandbox plus 407.1ms process time. Final 18-build Linux measurement passes (3.78s), with passing task batch readback for every sample and stage totals bounded by full call time; cold workspace-cache calls median 208.245ms, warm 69.420ms with explicitly reported variance. Final descendant timeout/order test passes Windows 1/1 (2.44s), Linux 1/1 (1.20s). Workspace check and TypeScript pass. No full P4-62 closure is claimed.

The Luna High agent hit its usage limit after writing the TG4-02 actual-schedule test; main took over. Its first execution failed because the paused Tokio clock does not advance cron wall UTC; corrected the second simulated boundary to 610 seconds and added disposable schema RAII restoration. Exact Windows/Linux reruns are in progress; no TG4-02 closure before their results. Remaining owner questions and four known-red regression files remain as recorded above.

## TG4-02 verified closeout

The actual scheduled-loop regression passes Windows 1/1 (1.32s), Linux 1/1 (1.85s), observing database failure, unchanged live handle and later saved QC recipe acknowledgement. TG4-02 is checked; current tally **38/59 P4, 17/33 TG4**. Timer-correction unit is committed locally as 43a2527; its final push accompanies this verified scheduler test-only checkpoint. Main completed the Luna-authored fixture after the agent usage failure; no other model or independent review is claimed. No new live API spend, Calendar access or expanded-goal read.

## Paused checkpoint — 2026-10-05, 01:58 UTC

Owner requested immediate stop. The app goal is PAUSED (confirmed by get_goal). No phase closure and no new commit/push during the desktop-consent slice. Main and origin/main remain at 186737a; index is unstaged. Current authoritative tally remains **38/59 P4 tasks and 17/33 TG4 gates**. The task/gate list above remains the resume scope. Expanded goal stays unread and untracked; owner Calendar folder was not accessed.

## Continued work after the paused checkpoint

Owner resumed the active P4 goal. `b3a133a` is now pushed to `origin/main`, preserving only valid canonical telemetry UUIDs in designated correlation fields. The retry-admission follow-up implements ADR-0302/0304 and is locally verified: Windows/Linux policy tests cover slow success, failed overrun, and a late backoff wake; the production logger-to-CLI-export integration reads back `setup_overrun` status and timing on both hosts. The earlier “hard deadline decision pending” passages above describe the state at the time of the pause and are superseded for current behavior by ADR-0302/0304. This does not close P4-52/TG4-26; successful CLI Fleet completion remains open. No P4 task/gate checkbox is changed by this evidence alone.

The timestamp-reader and analytics-retention production regressions are under verification. The first query-registry run correctly failed on stale/missing fingerprints after those SQL changes; P4-58 inventory reconciliation and populated-plan proof are still required. An independent review found a session-count overlap defect in retention reconciliation and an unbounded session-ID ledger; a scoped fix is in progress, and owner direction is pending on whether completed dates may be sealed so the ledger can be expired. Do not mark P4-57/P4-59/TG4-19/TG4-21 complete until these issues, registry evidence, CLI maintenance readback, and required platform proof are resolved. The expanded-goal attachment and owner Calendar directory remain untouched.

### Verified progress during the preceding continuation

Already pushed: 17f073b Linux child network denial; ed69cb0 actual scheduler/dependent completion closing TG4-06; 43a2527 non-overlapping command-stage timers; 186737a scheduled database-read recovery closing TG4-02. These are meaningful production boundary/gate changes, not full P4 closure. Workflow portability had earlier closed P4-16/TG4-17. All actual agents used Luna High; the agent usage failure remains explicitly recorded.

### Latest consent slice — passing evidence, still dirty

- Owner approved local Windows drivers. Installed tauri-driver 2.1.0 via cargo install --locked --root C:\arcana\target\e2e-tools. Matching Edge driver 154.0.4258.53 came from Microsoft's official endpoint; Authenticode signature is Valid/Microsoft. No global PATH/config or package dependency changes.
- npm run tauri -- build --debug --no-bundle passed (embedded frontend; native dev build 2m45s; pre-existing warnings). Explicit E2E TypeScript compilation and application typecheck passed before the last debug-port change. The native-override option and latest debug-port change need final explicit typecheck on resume.
- New real WebDriver Settings consent scenario passed three times (13.1s, 7.9s, 17.7s). It grants MCP consent via the real UI, reads native storage, restarts the application and reads the persisted grant, switches to a second disposable workspace, returns and revokes through UI, and reads revocation after remount. Native analyze_diff runs three times with Google consent absent (MCP grant does not authorize Google), and returns deterministic_only with one parsed file and explicit no-send summary. No provider generation or MCP invocation in desktop runs. Seven workers auto-started in the disposable profile; this is not successful paid Fleet execution.
- New test-only matrix in arcana-core/src/egress_consent.rs covers google/anthropic/openai across input-token preflight, completion and streaming. Mismatched workspace/destination and revocation deny every method before reaching the inner provider; exact grants reach all methods; independent connections read persisted grants/revocation; audit rows read denied/allowed/denied per method. Fake completions/streams deliberately return provider-reached errors; no live provider success is claimed. Consent suite passes Windows 6/6 (0.82s), Linux 6/6 (1.38s). Production local MCP invocation/discovery/revocation test passes Windows 1/1 (0.52s); native diff command helper suite passes 3/3 (0.31s).

### Exact next issue — do not repeat broad audits

Full native harness is NOT green. On every fresh disposable profile, its first session fails after about 60s with DevToolsActivePort file doesn't exist. A following consent session passes. Increasing the WDIO timeout did not fix it. Aligning documented webviewOptions.userDataFolder with the fixture environment also did not fix it. Native verbose logs confirm the intended binary/profile reach EdgeDriver; the browser starts with remote-debugging-port=0 but no discovery file was observed during that first session. This is not a proven root cause or speed claim.

The final diagnostic was interrupted by the owner before a result: a dynamically selected free loopback port (57473) is supplied through documented webviewOptions.additionalBrowserArguments. The existing EdgeDriver adapter still drives the built app; no separate CDP script/control path was added. Do NOT call this a fix. On resume, inspect the retained diagnostic and decide whether to keep or remove this narrow harness experiment, then run one fresh-profile launch/consent proof. Avoid extending this into an unrelated browser architecture project.

- Current e2e/wdio.conf.ts has disposable HOME/USERPROFILE/XDG, explicit info log verbosity, key/database env removal, owned Windows tree cleanup, finally restoration, no session retry and 120s transport timeout, Windows native userDataFolder plus experimental dynamic debug port, and optional ARCANA_E2E_NATIVE_DRIVER for tauri-driver's existing --native-driver flag.
- The native diagnostic wrapper is ignored target/e2e-tools/bin/native-debug.cmd. Raw logs: target/e2e-tools/p4-consent-run.log; p4-full-desktop-run.log; p4-full-desktop-aligned.log; p4-desktop-native-debug.log; p4-desktop-explicit-port.log; edge-native-debug.log. Test results: consent-core-tests.log, consent-linux-tests.log, consent-mcp-tests.log, consent-tauri-diff-tests.log.
- Retained disposable profiles: arcana-desktop-e2e-cj1BE5, 1CmttM, RzuFnn, 9rpwJ3, tAnqZT under the Windows temp directory. Raw profiles/logs may include OS browser identity/cache data; do not commit them. Native consent databases are under each profile/.arcana. The Windows credential store remains shared; tests never write keys or invoke credential validation.
- Ambient RUST_LOG=warn explained the first empty readable log. The later fixture explicitly uses info; no readable logger defect was inferred.

### Dirty inventory / remaining decisions

New consent work: egress_consent.rs (test-only), e2e/wdio.conf.ts, new workspace-consent.spec.ts, ADR-0301 and P4-60-DESKTOP-CONSENT-WORK-2026-10-05.md. Not committed: final harness is unverified/failing. Preserve the earlier red regressions separately: analytics/db.rs, db/runs.rs, memory/recall.rs and untracked workerChatStore.test.ts. Their source fixes still await owner disposition; do not fold them into a green consent commit.

Standing pending decisions remain: timestamp reader/index fixes; durable cost-summary coverage/reconciliation; Fleet retention lifecycle; startup hard deadline versus retry-admission semantics; exact designated correlation UUID preservation; new bounded paid Fleet trial; explicit budget-hold resolution. Do not spend or reinterpret these while pending. Driver installation was answered YES and must not be re-asked.

After native harness resolution, finish the P4-60 consumer inventory/readback and update MDP/TG4-16/readiness only if the whole contract is proven; then commit/push that logical unit. Remaining broad work is sandbox read/runtime/dependency isolation and Windows/macOS staging, recovery continuity, full observability/error contracts, pending chronology/retention fixes, CLI Fleet success, and the remaining core task/gate rows. Read expanded goal only after core engineering closure; owner Calendar remains final acceptance.

### Stop / environment

Killed only this test's verified local tauri-driver process tree, including native EdgeDriver and Arcana/WebView children. Ports **4444, 4445 and 57473 are free** and no Arcana/tauri-driver/msedgedriver process remains. The interrupted test reports ECONNRESET from this deliberate stop, not a newly diagnosed application defect. No new servers or compilation jobs remain. User stop takes precedence; do not continue implementation until resumed.

## Continued work after owner resumed — retention and timestamp evidence

Owner resumed the active P4 goal and selected the bounded dedup policy: keep
session IDs while a local date has raw token rows; after coverage and pruning,
seal the date, expire those IDs, and reject subsequent writes for that date.
Current phase tally remains **38/59 P4 tasks and 17/33 TG4 gates**; no task or
gate checkbox changed.

- Analytics retention now reconciles existing daily cost summaries instead of
  overwriting earlier same-day history. The exact distinct-session union is
  persisted across overlapping batches. Ambiguous legacy summaries fail
  closed. Additive migrations 6–8 add coverage/session/sealed tables, reject
  inserts into sealed local dates, and reject timestamp updates that would
  move a retained row into a sealed date. This v8 UPDATE trigger was added
  after an independent read-only review found the v7 INSERT-only boundary
  could be bypassed by UPDATE.
- `cargo test --offline -p arcana-core --lib retention_ --locked --
  --test-threads=1` passes **23/23 on Windows and Ubuntu WSL**, including
  overlapping session union, advancing-cutoff summary preservation, legacy
  adoption/refusal, populated pre-retention upgrade preservation, insert and
  update sealing refusal, active-writer serialization and the production
  manager consumer.
- `cargo test --offline -p arcana-core --test p4_query_registry --locked --
  --test-threads=1` passes **8/8** on the current tree after replacing stale
  RunsDb/retention hashes and registering new maintenance/trigger owners.
  The evidence explicitly keeps retention SQL under TG4-21 maintenance scope;
  it does not claim dashboard/dispatch plans for lifecycle writes.
- CLI Fleet now sets `start_analytics_rollup=true`; a drop guard stops and
  joins the owned analytics job on normal return and early errors. Windows
  `cargo check --offline -p arcanactl --locked` passes. The scheduled manager
  test proves the real immediate cleanup consumer in a disposable profile.
- Five Windows P4-57 focused tests pass for mixed RunsDb chronology/latest
  readers, migration v7 upgrade/index preservation, Memory stats recency, and
  latest-worker tick. `npm run typecheck` and the exact
  `src/stores/workerChatStore.test.ts` pass. These are selected pairs only;
  TG4-19 remains open.
- The retry logger/export integration passes on Windows after the RAII lock
  blocker change; frontend typecheck and worker-chat-store test pass.
- The previously approved workspace formatting pass and final
  `cargo fmt --all -- --check` pass. Exact diff review is still required
  before phase commits.

P4-59/TG4-21 stay open until macOS/current-tree matrix and remaining gate
clauses are proven. P4-57/TG4-19 stays open for full writer/reader and platform
property coverage. P4-60's Settings real-desktop harness still has a fresh
profile DevToolsActivePort failure followed by passing consent scenarios; do
not commit that failing harness work as a verified unit. The expanded goal
remains unread; owner Calendar remains untouched. Next steps: rerun CLI build
check after migration 8, review exact dirty diffs, commit verified units
separately, and push them to established `main` under existing authorization.

## Continued work after 2026-10-05 retention checkpoint

The retention/timestamp phase is committed and pushed as `129418e`.
`44d3e75 fix(p4): report worker setup retry overruns` is also pushed. Windows
and Ubuntu both pass the overrun-admission policy test and the production
redacting-logger-to-CLI-export integration (1/1 each); the event is persisted
and read back with the bounded outcome and elapsed time. Formatting and diff
checks pass. P4-52/TG4-26 remain open because this does not prove successful
CLI Fleet completion or every startup/recovery scenario.

The owner-approved Settings exception is implemented through one shared
metadata-only connection helper used for both explicit key validation and
Settings Test connection. It emits fixed destination, outcome, key-validation
purpose, no-workspace-payload, and duration fields, and suppresses raw provider
errors. `connection_audit_tests` passes 1/1; the provider consent wrapper's
token-preflight/complete/stream matrix passes 1/1 on Windows for mismatched,
matching and revoked workspace grants. ADR-0305, MDP P4-60, TG4-16, and the
readiness row record this as partial evidence. No task/gate checkbox or
overall count changed. The existing desktop scenario passed with persisted
grant, restart, workspace switch/revoke, and structural-only diff refusal;
however, the full fresh-profile WebDriver run still has an initial DevTools
attachment failure, so desktop consent is not closed.

The retry unit is `44d3e75`; this Settings evidence slice follows it as a
separate commit. The still-unverified WDIO config/spec and desktop run report
remain uncommitted and must not be treated as green evidence.
The user-created `P4_EXPANDED_GOAL.md` remains untouched/unread, owner Calendar
workspace remains untouched, and ~~no paid CLI Fleet run occurred in this
continuation~~ a bounded paid CLI Fleet run did occur (corrected below). Counts remain P4 38/59 and TG4 17/33. Continue against the open
MDP and gate items; do not revisit completed phase proof without new evidence.

## Paid CLI Fleet trial correction and evidence — 2026-10-05

Correction to the preceding checkpoint: ~~no paid CLI Fleet run occurred in this continuation.~~ The single owner-authorized run did occur. The earlier statement was based on checking the final run status without reconciling persisted provider-call and token-reservation events.

The isolated production run used Gemini 3.8 Flash with explicit workspace consent and the authorized $1 / 120,000-token / 20-turn / 10-minute Supervisor bounds. Six provider generations succeeded and one failed attempt was recorded. The next generation was refused before dispatch: countTokens estimated 16,376 input tokens, and the fixed 8,192 output allowance made the request reserve 24,568 tokens, projecting 121,447/120,000. Analytics readback recorded 95,824 prompt tokens, 116 completion tokens, 95,940 total tokens, and $0.05950545; it also exposed one API-failure row and one failed Supervisor worker-stat row. The Supervisor made six diagnostic reads, created no child task, and left the scratch workspace unchanged. The production log exporter read 2,424 events across two files (1,718,837 bytes), and analytics export returned the token, failure and worker-stat rows. Workspace consent was revoked and its successful audit event read back. No further provider call is authorized by the consumed one-run approval.

The run is partial P4-50/TG4-24 evidence only: it proves real CLI startup, bounded generation/refusal, persisted usage/failure telemetry, and log/analytics export consumers. It does not prove task assignment, coding, successful Fleet completion, recovery injection, or Logs UI readback. Counts remain 38/59 and 17/33; no checklist item is closed. Detailed sanitized evidence is in _verify/P4-50-CLI-FLEET-LIMITED-2026-10-05.md.
