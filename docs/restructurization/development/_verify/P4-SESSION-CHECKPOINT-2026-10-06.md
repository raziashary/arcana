# P4 active checkpoint — 2026-10-06

The goal remains active. Finish core MDP P4 and its named gates, commit/push verified logical units, then read and execute the owner-owned `P4_EXPANDED_GOAL.md`. The Calendar acceptance is owner-only. Current tally: **40/59 P4, 19/33 TG4**; P4-59/TG4-21 and P4-52/TG4-26 now close on verified contract evidence. Only GPT-6 Luna High subagents. Earlier sections below preserve the investigation sequence; the current boundary immediately below supersedes their pending/status claims.

## Current boundary after reliability closeouts

- P4-59/TG4-21 closes: complete Windows consumer checks, current Linux whole-workspace CI job 112205610578, and macOS six-group step 112207740059. The Ubuntu capture assertions now pass; the old loss mechanism is unknown. The separate macOS XPC corpus remains red/open under P4-11/61. Report: `P4-59-RETENTION-CLOSEOUT-2026-10-06.md`.
- P4-52/TG4-26 closes with owner-approved separation from successful paid Fleet. Windows/Linux each pass retry policy 11/11, shipped worker claim/budget/non-replay 3/3, and real JSONL-to-CLI export 1/1. Report: `P4-52-STARTUP-RETRY-CLOSEOUT-2026-10-06.md`; ADR-0319.
- Session/hook tests are committed locally as `140002d`, Windows/Linux timestamp group 4/4. Owner now approves preserving validated executor timestamps in HooksDb (ADR-0320). A bounded implementation is in progress; the prior hook writer fixture is historical evidence only.
- Owner approves isolated test-only desktop attach diagnostics (ADR-0321). Matched attach failures are not consent proof; new investigation must preserve shipped settings and owned cleanup. Existing e2e WIP stays separate.
- Remaining tasks: P4-11,14,15,18,19,20,21,36,39,49(owner),50,51,53,54,56,57,60,61,62. No new paid run authorized. Expanded goal stays unread.

## Verified and pushed

- `fc08a80`: repaired three stale RunsDb CI fixtures without changing production behavior. Production reader/migration suite: Windows 29/29, Linux 29/29; query registry Windows 8/8.
- `60e4666`: AnalyticsDb annual/DST production timestamp-pair tests, Windows 2/2 (13.74s), Linux 2/2 (102.35s). Extended existing three-host CI retention step to all six producer/consumer groups. Windows selected groups: 23, 5, 4, 1, 1, 1 passing.
- `294086c`: aligned two retention captures with the existing filtered Registry pattern. Windows focused captures 2/2, Logs group 35/35, complete desktop library 167/167 with four threads. **Not a confirmed CI fix:** Ubuntu run 37441953293 still fails the hook-retention persisted-operation-label assertion (168 pass, one fail). Narrow triage is active; no skipped or weakened assertion.
- Current startup retry checks: Windows/Linux policy 11/11; shipped worker identity/budget/non-replay 3/3 each; real JSONL-to-CLI export 1/1 each (Windows 6.75s, Linux 8.21s).
- `1cdb7a1`: use the existing guarded nonblocking capture and a separately asserted fixed readiness probe. Windows retention selection passes 3/3 and the complete desktop library 167/167 (24.26s) with four threads. Remote CI is required; no confirmed cause or gate closure is claimed.

## Remote CI boundaries

Run 37440361509 at `60e4666` passes whole-workspace Rust, frontend, and CLI jobs. macOS completes all six retention groups (23, 5, 4, 1, 1, 1), then the separate project-command corpus fails: the signed App Sandbox XPC runner is not packaged. Windows/Linux matrix jobs were cancelled by the subsequent push. Run 37441953293 at `294086c` passes frontend/CLI but fails the desktop hook capture, so its platform jobs are skipped. Never report either full pipeline green. Raw safe logs are under ignored `target/p4-ci-checks`.

## Desktop consent harness

Owner approved matched attach mode. `e2e/windows-attach-diagnostic.mjs` launches the built desktop and native driver against a fresh disposable profile/workspace, clears credential variables, waits for the debugging endpoint, and checks owned process/port cleanup. Both current invocations fail before session creation because the endpoint is unavailable within 30s. The second invocation's owned-process inspection confirms browser/renderer creation and the debug-port flag, but no listener is found at the inspected port. Root cause remains unknown. Both invocations complete cleanup without errors. Production app startup/migration activity is present; it is not consent UI proof.

The owner has been asked to authorize a separate test-only Tauri build with explicit browser arguments and diagnostics. Keep that decision pending. Existing dirty `e2e/wdio.conf.ts` and `e2e/specs/workspace-consent.spec.ts` remain unverified as a complete harness; do not stage them as a passing unit. The earlier passing native grant/restart/switch/revoke scenarios remain partial evidence.

## Immediate work

1. Diagnose the repeated Ubuntu hook-retention persisted-label failure; preserve real production cleanup, Logs readback, and privacy assertions. Close P4-59/TG4-21 only after all clauses and current platform proof pass.
2. Owner decision pending: reconcile P4-52/TG4-26's literal retry clauses with extra status prose requiring successful live Fleet completion. Independent review finds the former covered by current checks; P4-50 owns successful Fleet completion. Do not silently remove the added status dependency.
3. Extend P4-57 session/message and hook-log annual writer/reader proof in a bounded tests-only unit. The complete per-pair inventory still remains open.
4. Continue the other unchecked core P4 tasks and smoke/gate clauses; platform sandbox/XPC, continuity/reassignment, full tool-error matrix, complete observability/UI and successful CLI Fleet remain material work. Do not repeat completed proof absent new failure/change.

## Boundaries and working tree

No second paid generation run is authorized. The one $1/120k/20-turn/10-minute CLI trial spent $0.05950545, six generations succeeded, one failed, and it created no child task. Offline Supervisor output cap 6,144 is implemented. Do not use owner Calendar or open/stage the expanded goal before core closure. No live credentials are supplied to tests.

Uncommitted: existing desktop config/spec/work report, matched-attach diagnostic/report, retry closeout review, current documentation, and active session/hook timestamp tests/report. Deliberately preserve each unit and owner-owned untracked expanded goal. Commit verified units with descriptive subject/body; inspect staged diff/secrets and outgoing messages; push only established main upstream under existing permission. No reset, amend, force push, Git config change or file cleanup.

## Session/hook proof continuation

The four-test timestamp group passes Windows 4/4 (59.67s), Ubuntu WSL 4/4 (130.50s). New session/message cases use the actual production writers/readers over 1,106 annual/midnight/DST instants plus legacy rows. Hook historical rows test production readers only; the real writer ignores the executor-provided timestamp and uses SQLite insertion time. Owner decision pending: validate/canonically persist the execution instant, or retain/document insertion-time semantics. No API behavior change or P4-57 closure yet. Evidence: `_verify/P4-57-SESSION-HOOK-PAIRS-2026-10-06.md`. Capture triage and its ADRs are committed in `1cdb7a1`; latest remote result remains pending. No active test process or listener remains from the local checks.
