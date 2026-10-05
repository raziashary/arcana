# P4 session checkpoint — 2026-10-05

## Current mandate and state

Complete core P4 in the MDP, testing gates and readiness first, committing/pushing logical units; only afterward read and finish the user-owned untracked `P4_EXPANDED_GOAL.md`. Owner alone runs final Calendar desktop acceptance; do not access that workspace. All subagents must use GPT-6 Luna High. Existing agent thread limit prevented a fresh review; no independent phase-review claim is made.

Current counts: **37/59 P4 tasks; 14/33 TG4 gates**. P4-55/TG4-29, P4-43 and now P4-40 closed since the earlier 32/59, 10/33 baseline. Remaining core tasks: 22 including owner Calendar P4-49. Full phase remains open.

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
