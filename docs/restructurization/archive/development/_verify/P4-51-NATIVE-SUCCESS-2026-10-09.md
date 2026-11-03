# Registered native subagent success proof — 2026-10-09

Owner-approved ADR0346 permits a test-only provider override analogous to RecipeRunner. The real manager, native AgentEngine, consent store, ToolExecutor registrations, accounting and QC background reader remain in use. Shipped Vault/provider construction is unchanged. The process-global manager requires one explicitly ignored test run in a fresh test process; ordinary no-manager negative tests remain separate.

Grounding: native_runner::run_inner constructs AgentEngine and requires workspace consent; spawn_subagent and spawn_parallel_subagents await real children; spawn_qc_review claims and dispatches, then asynchronously attributes actual reviewer cost and releases the claim. The fixture must read these effects rather than return a fake manager result. Validation pending. No paid provider call or task/gate closure claimed.

## Verified result

The fresh-process ignored native success fixture passes Windows1/1 (2.19s) and Linux1/1 (2.69s). Four real AgentEngine children execute: one single, two parallel, one QC; five scripted generations include the QC get_task round-trip. The real task reader observes qc_review, then background settlement clears its reviewer claim and persists positive actual child cost. Returned direct/parallel summaries and exact token counts are read through ToolExecutor. No Vault credential, network provider request or fake manager is used. Synthetic passing build evidence is only QC input setup and does not count as compiler proof.

The existing uninstalled-manager privacy fixture passes in a separate Windows process1/1 (1.53s); Linux regression1/1 passes (0.54s), also in a separate process. Core/CLI cargo check passes (34.73s). A focused independent Luna High static review found no actionable defect and documents the fixture limits. CI invokes the success test separately with --ignored so ordinary negative fixtures retain their empty manager. Complete P4-51/TG4-25 remains open.
