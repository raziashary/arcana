# Native subagent success and failure proof — 2026-10-13

Scope: P4-51 / TG4-25 production-tool proof for the three registered native subagent tools. This evidence does not close a task or gate.

Grounding: `arcana-core/src/agent/builtin_tools/mod.rs::register_for_mode` registers `spawn_subagent`, `spawn_parallel_subagents`, and `spawn_qc_review` for FastDevelopment. Each is invoked through the public `ToolExecutor`; the real `SubagentManager` launches `native_runner::run`, which constructs the actual child `AgentEngine`. `RunnerDeps::provider_override` is already limited to test/offline-fleet builds, analogous to RecipeRunner; this fixture uses that override, while production builds continue through `build_provider` and Vault.

## Verified result

The exact authorized fresh-process test passes: one test, zero failures, 1.44 seconds. Four successful child generations run through the actual registered tools (single, two parallel, one QC); the scripted provider supplies five successful model generations because QC first calls the real `get_task` tool and then completes. The parent reads single/parallel summaries and token accounting through `ToolExecutor`. QC's real task reader sees `qc_review`; background settlement releases the review claim and records positive child cost.

The same provider override then returns a deterministic fixed provider error for four actual child executions (single, two parallel, one QC). The single and all-failed parallel calls return correlated tool errors. The QC dispatch returns its asynchronous dispatch receipt, after which the manager reports terminal `Failed` and the real task row has no reviewer claim. A private canary is verified in the child request and absent from each corresponding parent tool output. The test does not call a live provider or read Vault credentials; the provider/model identity is asserted as Google / `gemini-3.8-flash`, and the production provider construction path is unchanged.

Command (credential environment variables cleared):

```powershell
cargo test -p arcana-core --lib native_subagent_tools_use_scripted_provider_and_return_real_results -- --test-threads=1
```

Result: `1 passed; 0 failed; 0 ignored; 1128 filtered out`; test body 1.44s. Compilation emitted existing workspace warnings. `rustfmt --edition 2021` was applied to the scoped test file; formatting and whitespace checks are recorded with this report's closeout.

## Limits and remaining work

This is one focused tool/provider failure shape, not the full P4-51 error-class, retry, or privacy matrix. QC build evidence remains synthetic fixture setup, not compiler execution. The test's provider error is deliberately fixed and does not prove vendor transport failures. P4-51 and TG4-25 remain open; no readiness score or plan status was changed.
