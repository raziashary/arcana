# P4-23 / TG4-10 / TG4-11 — native delivery and reconciled replay

Status: **P4-23, TG4-10, TG4-11 and S4-04 proven**, 2026-10-05.

## Production path and behavior

`SendWorkerMessageTool` runs through ToolExecutor with the Rust-authored worker identity, persists TasksDb rows and requests a scheduler kick. The recipient's saved recipe runs through RecipeScheduler → RecipeRunner → AgentEngine → native inbox/ack tools. Pending/read rows remain deliverable under the original message ID until acknowledgement. No Python or MCP sidecar is in this route.

The strengthened `scheduler_kick_runs_persistent_qc_inbox_through_recipe_runner_and_acknowledges` sends from the real backend ToolExecutor to the persisted `qc-agent`. It verifies stored sender/recipient identity, ages the message in the isolated fixture, invokes the Supervisor stale-reader and native nudge sender, then drives the actual saved QC recipe. The recipient acknowledges both IDs, durable work clears, one injected transient setup failure does not create a second run, and the next idle kick invokes no provider. Windows 1/1 (1.60s), Linux 1/1 (1.53s).

The new `scheduler_replays_inbox_after_completed_effect_without_duplicate_mutation` drives the saved Supervisor recipe, reads its inbox and cancels a real task. It pauses the provider after the mutation but before acknowledgement, cancels the actual scheduler run, verifies an interrupted durable run and runnable worker, then constructs a new scheduler over the same stores. The second run receives the original read/unacknowledged message ID, reads the persisted cancelled task, and acknowledges without repeating cancellation. Captured production requests prove both tool results and the saved replay policy reach the engine consumer. An isolated audit trigger observes exactly one task-status mutation. The next idle kick issues no provider request. Windows 1/1 (1.42s initial proof; the final policy assertion also passes in the three-test scheduler regression), Linux 1/1 (2.24s).

The native sender regression group passes 4/4 on Windows (1.44s) and Linux (2.78s). It proves truthful sender identity, self-send refusal, worker-broadcast refusal, oversized-body rejection with no extra stored message, deleted/unbound recipient semantics, and truthful pending-delivery output. The Windows scheduler-filter group passes 3/3 (3.16s).

## Existing amplification policy

ADR-0095 records the owner's choice to use existing bounds. Those are the 4096-byte body ceiling, self-send refusal, Supervisor-only broadcast over the active roster, exclusive per-worker scheduler slot and finite worker/Supervisor run budgets. P4-43 separately proves startup/idle gating and configured provider-turn limits. No new per-sender per-minute limiter was selected or implemented; the former TG4-10 note saying owner direction was pending was stale. These are message/run bounds, not a claim of a lifetime global queue quota.

## Validation and limits

These are real native producer/consumer tests with scripted providers, isolated homes, actual SQLite stores and no live API calls. They prove in-process interruption and redispatch; they do not claim whole-app process-loss recovery, arbitrary filesystem/MCP effect idempotency, successful live CLI Fleet coding or owner Calendar acceptance. Those remain owned by P4-36/53/56/50/49. No test listeners remain. Workspace check and formatting passed; the three-platform phase matrix remains open.
