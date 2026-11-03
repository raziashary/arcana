# P4-39 fleet dependency completion verification

Status: investigation started.

## Evidence log
- 2026-10-06: Created this report before source investigation, as required.
- 2026-10-06: P4-39 says dependency-blocked tasks resume and dispatch automatically after prerequisite completion, while budget-exceeded tasks stay held for explicit resolution and do not resume just because both use `needs_info`; proof is required through autonomous completion (MASTER_DEVELOPMENT_PLAN.md §10, P4-39).
- 2026-10-06: TG4-06's 2026-10-05 evidence names the existing scheduler integration test `dependency_completion_wakes_persisted_supervisor_and_completes_saved_worker_recipe`; it covers status-tool completion, scheduler wake, saved Supervisor list/routing, saved backend claim/DONE, and a held sibling, while explicitly leaving P4-39 open (MASTER_DEVELOPMENT_PLAN.md progress entry dated 2026-10-05).
- 2026-10-06: The scheduler test module already has an isolated child-process test using `with_home`, shipped `recipes::init`, isolated consent/workspace, `RecipeRunner::with_provider_for_test`, persisted workers, and a scripted provider. It verifies final dependent `done`, automatic routing history, budget-held `needs_info`, and refused claim. Missing explicit proof is single provider-run/claim cardinality; I will strengthen this existing end-to-end regression and name it for the requested focused filter.
- 2026-10-06: Strengthened the existing scheduler production-path regression (renamed to match the requested `scheduler_fleet_dependency_completion` filter). It now requires exactly six scripted provider requests (three turns each for Supervisor and backend) and exactly one distinct successful `claim_task` result, after shutting down the scheduler once both workers return to Idle. No production APIs, schema, dependencies, or files outside the authorized test module/report changed.
- 2026-10-06: Before running the requested focused offline test, reviewed the fixture's child-process isolation, temporary Arcana home, shipped recipe registration, workspace-specific scripted-provider consent, and scripted provider; it makes no live provider/network request.
- 2026-10-06: Ran the authorized focused command after clearing inherited environment variable names matching `KEY` or `TOKEN`, plus `DATABASE_URL`, for that command's process; values were restored afterward: `cargo test --offline -p arcana-core --lib scheduler_fleet_dependency_completion --locked -- --test-threads=1` → **passed, 1 passed, 0 failed, 1049 filtered out**, test runtime 4.99s. Cargo emitted 43 warnings from the crate build. The integration fixture's child process and temporary home completed; it used the injected provider and did not contact a live provider/network or start a listener.
- 2026-10-06: `git diff --check` passes for the owned source/report files. No production contract, schema, dependency, or plan checkbox was changed; no git refs or index entries were modified. Cross-platform/CLI Fleet acceptance and P4-39 plan closure remain outside this focused regression and remain unclaimed.

## Result
The scheduler-to-RecipeRunner-to-AgentEngine regression now directly asserts single-run/single-claim cardinality alongside autonomous dependent completion, automatic routing, and continued budget hold/refusal. The production path is exercised with shipped recipes and isolated scripted providers. This is regression evidence for P4-39; it does not authorize changing the plan's completion status.



## Main-session complete contract proof

The later budget-resolution implementation and both real Fleet smoke cases are recorded in `P4-39-BUDGET-RESOLUTION-2026-10-06.md`. Main extends this same test to run separate ordinary-dependency and explicitly-resolved-budget cases in isolated child processes. Both cases pass on Windows3.26s and Ubuntu4.80s, with saved recipes, actual scheduler/runner/engine/tools, exact generation/claim cardinality and persisted routing/audit/usage readback. These results supersede the earlier tests-only scope; successful paid CLI Fleet and owner Calendar remain separately open.

## Supervisor agent-loop proof replacing the direct-call fixture

The focused review correctly found that the earlier six-call resolved-budget case invoked SetTaskBudgetTool from fixture code. That evidence alone did not prove Supervisor-authored resolution. The test now supplies the resolution as an actual model tool call through the saved Supervisor recipe, RecipeRunner, AgentEngine and ToolExecutor. The Supervisor first calls get_task, then set_task_budget(resolve_exceeded=true), reads get_task again to observe preserved cumulative usage and the durable audit, and only then requests the normal ready transition and deterministic dispatch. Its captured prompt/tool declarations must contain the installed resolution guidance and registered budget tool.

The resolved case makes exactly seven Supervisor and three worker calls; the ordinary case remains three plus three. Both require one distinct successful claim and final persisted completion. The current exact test passes Windows 1/1 (two isolated cases, 3.12s) and Ubuntu WSL 1/1 (two isolated cases, 2.57s). Raw safe logs: target/p4-ci-checks/budget-supervisor-fleet-{windows,linux}-2026-10-06.log. These replace the earlier direct-call consumer claim; no paid provider or user database was used. The task-deletion/audit retention decision remains pending, so P4-39 is still unchecked and no tally changes.


## Owner-approved provenance correction, 2026-10-09

ADR0333/0336 supersede the pending retention/ID-reuse choice above. The new additive migration22 assigns/backfills immutable unique task UUIDs, includes them in retained resolution rows and task readers, and preserves numeric task AUTOINCREMENT across Clear Board. Deleted audit has no restrictive task FK; insertion still requires the active held task and authorized Supervisor. Both immutable-audit and tasks_updated_at triggers are restored inside the atomic migration, so backfill preserves historical timestamps. Older deleted legacy audit has no recoverable parent identity; it receives a distinct synthetic UUID rather than an invented link to a new task.

Final production TasksDb module passes Windows70/70 (61.36s), Linux70/70 (137.95s), including populated upgrades, immutable history, delete/clear, non-reuse, usage/other holds, rollback, direct-SQL guard and legacy timestamp checks. Windows installed Supervisor guidance1/1 (0.18s), real saved-Fleet ordinary/resolved cases1/1 (4.93s), exact SQL registry8/8 (4.82s), core and CLI/Tauri checks pass. Linux Fleet/default/query and both-host retained audit reader verification are recorded in the final closeout once completed. The new reader is an explicit opt-in under ADR0340; default get_task missing behavior stays unchanged.
