# P4 session checkpoint — 2026-10-05

## Current mandate and state

Complete core P4 in the MDP, testing gates and readiness first, committing/pushing logical units; only afterward read and finish the user-owned untracked `P4_EXPANDED_GOAL.md`. Owner alone runs final Calendar desktop acceptance; do not access that workspace. All subagents must use GPT-6 Luna High. Existing agent thread limit prevented a fresh review; no independent phase-review claim is made.

Current counts: **35/59 P4 tasks; 11/33 TG4 gates**. P4-55/TG4-29, P4-43 and now P4-40 closed since the earlier 32/59, 10/33 baseline. Remaining core tasks: 24 including owner Calendar P4-49. Full phase remains open.

## Completed claim-safety unit

P4-40 production writer/caller audit and late manual-release race are complete. Raw independent SQLite connection cannot forge inconsistent claim metadata; competing workers admit only one winner; claim tool records the actual process PID/host with isolated RunsDb; automatic and manual stale release preserve replacement claims. Three real Tauri command tests pass. Rebuilt CLI smoke proves claim/refusal/failure/readback with actual process PID. Evidence: `P4-40-CLAIM-SAFETY-CLOSEOUT-2026-10-05.md`; temporary smoke profile: `%TEMP%/arcana-p4-claim-cli-7wh1_62v`. No API or listeners used.

## CI prerequisite verified

Run 37218542575 passed frontend/CLI checks and desktop/core library tests, then failed S2-06's old structural check requiring `hydrate_today()` inside desktop main. The approved shared initializer still invokes hydration. ADR-0283 updates the guard to trace desktop and CLI startup into shared hydration and adds persisted same-day/old-day behavior readback. Windows S2 suite 9/9 and shared runtime tests pass; Linux S2 suite 9/9 and shared runtime group 5/5 pass. No production spend behavior changes.

## Next open milestone

Push the verified CI guard unit, then inspect the three-host saved-workflow portability matrix before closing P4-16/TG4-17. Continue remaining core tasks, especially sandbox corpus, complete recovery and live CLI Fleet-to-Logs proof. Prior bounded Gemini Fleet run exhausted quota before child task completion; do not silently rerun against the key. Registered telemetry labels were approved and fixed in 30518b7; canonical UUID correlation-field preservation is a separate pending owner decision. Calendar and expanded goal remain untouched.

P4-40 logical unit committed as `67fd7ab`; CI guard is a separate test-only unit. Local Windows shared runtime filter passed 11/11 (5 runtime and 6 Python runtime tests) in 117.99s; Linux shared runtime group passed 5/5 in 0.26s. All tests use isolated homes.
