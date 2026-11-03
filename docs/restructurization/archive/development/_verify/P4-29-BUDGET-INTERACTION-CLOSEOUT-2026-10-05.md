# P4-29 / TG4-15 — saved-task and daily-budget interaction

## Production boundary

RecipeRunner constructs AgentEngine with the worker cap and CappedProvider; ClaimTaskTool publishes the claimed task's saved budget/usage through the run context. The engine consumes that identity and combines its cap with the worker cap before the next request. The existing saved-worker commit integration proves this construction/claim path; the new interaction tests drive AgentEngine, ClaimTaskTool, TasksDb and the actual CappedProvider with both limits enabled. A scripted Google provider supplies known normalized usage (input=10, output=1); no live request or credential is used.

## Falsifiable cases

1. `saved_task_breaker_works_with_enabled_daily_reservations`: daily ceiling $100; saved task cap one turn plus finite token/cost caps. The first provider response claims the assigned task; the next turn returns `BUDGET_EXCEEDED_TURNS` without a second request. Production TasksDb readback shows needs_info, budget_exceeded and persisted usage. WorkersDb is not paused by the task breaker. The daily reservation settles to exactly the priced observed usage (ceil to microdollars), with zero remaining holds; saved task cost matches that same usage.
2. `daily_reservation_denial_preserves_the_unclaimed_saved_task_budget`: enabled ceiling $0.000001 refuses the conservative reservation before dispatch. Provider count is zero; the task remains assigned, unclaimed, with original saved caps and zero consumption/no task breaker. The returned error identifies the daily ceiling; WorkersDb reads paused. There is no hold and no charged spend.
3. Existing `claimed_task_budget_activates_before_the_next_provider_request` still passes. Its older wording called this a one-turn cap, but it actually exhausts the saved token cap with seeded prior usage plus conservative unknown-usage charging. ADR-0288 corrects that wording; its behavior is unchanged.

## Validation

- `cargo test -p arcana-core --lib procedural_memory_tests --locked -- --test-threads=1`: Windows 10/10, 4.27s; Linux 10/10, 5.70s. Includes both exact-cost/refusal cases, original claim-cap test, real MCP consumer, cancellation and error consumers affected by the shared scripted-provider catalogue fixture.
- Earlier `daily_` regression group: Windows 15/15, 14.17s; Linux 15/15, 23.64s. The final exact-cost/message assertions are covered by the later 10/10 runs.
- Workspace check passes (14.16s); formatting/diff checks pass. Production worker smoke/claim-budget/commit is recorded in ADR-0286 and the P4-52 proof update (3/3 each host); no new production code or executable behavior is introduced by this test-only closeout.

## Limits

P4-29/TG4-15 closes. This does not close TG2-17's whole-provider concurrency, pause visibility, kill-switch or global in-flight contract. It does not prove successful live CLI Fleet coding or owner Calendar acceptance. Test profiles are isolated; the local MCP fixture in the adjacent regression suite is aborted and the test process exits. No dev server or persistent listener is started.
