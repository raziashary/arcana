# P4-50 limited production CLI Fleet trial

Date: 2026-10-05
Status: partial evidence; P4-50 and TG4-24 remain open.

## Scope and authorization

This was the one owner-authorized paid CLI run. It used the isolated profile and scratch workspace under target/p4-cli-approved-smoke-494b324182764cf88a117a47d26ed3b3, Gemini 3.8 Flash, explicit Google workspace consent, a $1 Supervisor cost cap, a 120,000-token run cap, 20 Supervisor turns, and a 10-minute timeout. It did not access C:\Playground\arcana-calendar. No provider key was passed in the CLI environment. The isolated workspace contained only its initial README.

## Result

The production arcanactl fleet run exited unsuccessfully after about 20.16 seconds. Structured log readback shows six successful provider generations and one failed provider attempt. The Supervisor made six diagnostic reads (list_tasks, list_worker_health, list_worker_requests, list_worker_messages, get_task, get_epic_progress); it created no child task and made no workspace change.

The next generation was denied before provider dispatch by the run-token reservation. Gemini countTokens estimated 16,376 input tokens; AgentEngine requested its fixed 8,192-token output allowance; the resulting 24,568-token reservation would project 121,447 tokens against the 120,000 cap. This is a pre-dispatch budget refusal, not a provider failure for that denied request.

Analytics export read back one token-usage row with 95,824 prompt tokens, 116 completion tokens, 95,940 total tokens, and $0.05950545. It also read back one API-failure row and one Supervisor worker-stat row. The JSONL export contained 2,424 events across two files and was 1,718,837 bytes. logs export and analytics export both exited successfully. Workspace consent was revoked afterward; the CLI revoke exited successfully and the persisted audit outcome was success.

## What this proves

- The real headless CLI can initialize the production Fleet runtime and dispatch Gemini generations under a bounded run.
- Gemini input preflight and the shared run-token ledger can deny a request before dispatch when the reservation exceeds the cap.
- Provider usage and a provider failure persist to analytics, and the production CLI readers export the logs and analytics.
- The consent grant/revoke commands operate on the isolated profile, and revocation is auditable.

## What remains unproven

No child task was assigned, no code was written, no worker completed a coding task, and no successful Fleet completion or recovery injection was exercised. This did not prove desktop Logs UI readback or the complete TG4-24 workflow. P4-50/TG4-24 remain open; the run does not close a checklist item.

## Output reservation finding

arcana-core/src/agent/engine.rs originally requested up to 8,192 output tokens for each generation. RunTokenBudgetProvider reserves the Gemini input estimate plus this requested maximum. The six successful responses reported only 116 completion tokens in total. On 2026-10-05, the owner approved a 6,144-token maximum for budgeted Supervisor runs. RecipeRunner now applies that limit only to the Supervisor; workers retain the 8,192 default. Focused offline tests prove the request builder and Supervisor-only selection. Reconstructing the denied reservation with the same 16,376 input estimate yields 22,520 reserved tokens and a projected 119,399/120,000, so the request would fit arithmetically if all other run state were unchanged. This is not a live rerun or proof of successful task creation. No second paid run is authorized by the consumed one-run approval.
