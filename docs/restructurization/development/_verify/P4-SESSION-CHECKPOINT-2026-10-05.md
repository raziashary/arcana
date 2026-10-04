# P4 session checkpoint — 2026-10-05

## Current mandate and state

Complete core P4 in the MDP, testing gates and readiness first, committing/pushing logical units; only afterward read and finish the user-owned untracked `P4_EXPANDED_GOAL.md`. Owner alone runs final Calendar desktop acceptance; do not access that workspace. All subagents must use GPT-6 Luna High. Existing agent thread limit prevented a fresh review; no independent phase-review claim is made.

Current counts: **36/59 P4 tasks; 13/33 TG4 gates**. P4-55/TG4-29, P4-43 and now P4-40 closed since the earlier 32/59, 10/33 baseline. Remaining core tasks: 23 including owner Calendar P4-49. Full phase remains open.

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
