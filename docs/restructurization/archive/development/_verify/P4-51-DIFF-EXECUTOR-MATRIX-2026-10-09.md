# P4-51 / TG4-25 registered diff executor proof

## Progress

- Created this report before repository investigation, as directed.
- Scope is limited to cfg(test) additions in `arcana-core/src/agent/builtin_tools/analyze_diff_tool.rs` and this report.
- No tests, builds, processes, network, installs, environment/credential reads, user database access, or Git mutations will be performed.

- Quick memory pass found no Arcana P4-51/TG4-25 or diff-executor note in the searchable memory registry; repository evidence will govern.

- Repository search surfaced existing P4-51 verification records and the named diff tool/TasksDb modules; next I am resolving the governing MDP/TG sections and public helper signatures before drafting assertions.

## Assertion draft from requirements (before fixture implementation inspection)

- Positive contract: register the real `analyze_diff` builtin in `FastDevelopment`, call it through `ToolExecutor`, and assert the returned tool result is successful and correlated to a real task identifier.
- Persistence contract: query the task via the actual `TasksDb` reader using that returned identifier and assert the analysis value is present in the persisted task.
- Negative contract: malformed or missing-branch input returns a failed result with a bounded, versioned structured envelope, stable category/code and operation/tool identity, actionable safe guidance, and call/task correlation where available.
- Privacy contract: plant distinct prompt, credential/key, source, and argument markers in rejected input; assert none occur anywhere in the structured failure, while the serialized envelope remains under the established size bound.
- Consent/security contract: preserve the existing structural fallback fixture guarantees; assert deterministic-only mode and a denied workspace-egress audit, with no Vault lookup/provider/network path invoked.
- Limits: this is one builtin registration and two deterministic outcomes; it cannot close the every-builtin TG4-25 inventory or cover unrelated error categories.

- Public contract traced: `builtin_tools::register_for_mode` accepts `PAMode::FastDevelopment`; `ToolExecutor::execute_batch` accepts registered `Tool` objects with JSON input and call ID; `TasksDb::open_at` opens an explicit isolated path and `get_by_id` is the actual reader. `Task` is publicly readable after lookup.

- Existing fixture facts: it creates an isolated home/workspace/repository, grants consent for a mismatched provider, calls the handler directly, checks `deterministic_only`, and reads one denied `qc_diff_analysis` audit row. It does not cover the registered executor, task persistence, or malformed arguments.
- The current consent helper returns structural analysis immediately on denied consent before `build_analyzer_from_vault`; that builder contains `VaultManager::get`, provider construction, then analysis. I will make this no-call property observable with test-only counters at those three boundaries, compiled only under `cfg(test)`.
- Executor identity contract: `execute_batch` returns the original tool-use ID and builds call correlation as `<session>-<tool_use_id>`; plain handler errors normalize to schema version 1, `tool_operation_failed`, `tool_execution`, `tool.analyze_diff`, and executor-owned context.

- Scope clarification from the coordinator: keep production helper bodies unchanged; no test counters or Vault instrumentation. The test will cover only the real registered success/readback and malformed-argument envelope. The no-consent no-call claim is source-path evidence (denial returns structural before builder) paired with observed deterministic output and denied audit; there is no runtime Vault/provider/network counter in this slice.

## Test changes drafted

- Extended `analyze_diff_tool_executes_repository_diff_without_consent_or_provider_key` to register through `FastDevelopment` and call via `ToolExecutor::execute_batch`.
- Added an isolated task fixture, passed its `task_id`, and read `Task.diff_analysis` back through reopened `TasksDb::get_by_id`; the stored JSON is compared with the successful tool result and must report `deterministic_only`.
- Preserved mismatched workspace-consent setup and denied `qc_diff_analysis` audit assertion. The actual feature diff contains the planted source marker.
- Added an omitted-`head_branch` call through the same registered tool. It asserts failed result status, bounded/versioned JSON, stable code/category, operation/tool identity, actionable fields, retry metadata shape, run/task/call correlation, and absence of prompt/key/source/argument canaries.
- No runtime Vault/provider/network counter is present per coordinator direction. The no-call conclusion is limited to the current source path: consent denial returns structural analysis before builder/Vault/provider code, with the fixture observing deterministic output and one denied audit.

## Validation and claim limits

- Exact test filter: `agent::builtin_tools::analyze_diff_tool::consent_tests::analyze_diff_tool_executes_repository_diff_without_consent_or_provider_key`.
- Not run: coordinator prohibited tests/builds and did not provide an exact command to run.
- No live credentials, user database, process, network, install, environment/credential read, or Git mutation was used.
- This fixture adds one registered success and one malformed-argument failure. It does not close P4-51/TG4-25, cover other registered tools/error categories, or provide runtime instrumentation proving zero Vault/provider/network calls.


## Coordinator verification

Exact filter passes Windows1/1 (0.91s), Linux1/1 (1.00s), with real isolated RunsDb worker-run prerequisite. Actual TasksDb analysis readback and bounded private failure assertions pass. No paid provider request or live user database was used.
