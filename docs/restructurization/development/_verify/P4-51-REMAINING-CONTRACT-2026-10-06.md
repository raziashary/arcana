# P4-51 remaining contract investigation — 2026-10-06

## Progress log

- 2026-10-06: Created the report before investigation, as required. Scope is limited to the MDP P4-51 contract, TG4 structured-failure gate, agent failure envelope, ToolExecutor tests, and provider failure classification. No source changes or tests.


## Source findings recorded so far

- `MASTER_DEVELOPMENT_PLAN.md` P4-51 (line 1528) and `TESTING_GATES.md` TG4-25 (lines 504-511) define the live contract: every registered builtin exercised through production `ToolExecutor::execute_batch`, including deterministic success plus injected failure, failed provider-facing result, envelope privacy/bounds/correlation, and categories spanning parse/unknown, permission, handler/database, timeout, cancellation, and infrastructure. The dated gate prose is lagging: it says provider transport/malformed/frame failures remain open, while MDP/TG4 entries dated 2026-10-04 say these provider privacy slices have since been covered. Gate remains open for the broader contract.
- `arcana-core/src/agent/tool_executor.rs` is the shared consumer path. Its `execute_batch` normalizes handler results and maps explicit headless/permission/cancel outcomes; panic recovery is covered by `handler_panics_become_structured_results_for_read_and_write_partitions`. Existing executor tests also cover operation journal read/write failures and concurrent-read cancellation, but the journal tests assert executor persistence behavior, not provider-facing builtin-family failure results.
- `arcana-core/src/agent/engine.rs` resolves unknown names itself and returns `ToolResult` with `is_error` through `MessageContent::ToolResult`; `unknown_tool_failure_uses_structured_correlated_tool_result` is the known end-to-end consumer case. Batch infrastructure `ArcanaError` mapping exists in the production consumer body, but no named consumer regression surfaced yet.
- `arcana-core/src/agent/tool_trait.rs` has 8 envelope tests under filter `agent::tool_trait::tool_result_error_tests`; they cover serialization, generic enrichment, redaction of planted command/prompt/source/path input, action guidance, and bounded diagnostic shape. They are helper-level, not registered-builtin `ToolExecutor` tests.

## Remaining matrix (source-grounded; no tests executed)

| Required proof | Current production evidence | Exact remaining slice / next test case | Existing useful filter |
|---|---|---|---|
| Shared JSON contract, bounded fields, privacy, stable `is_error` | `tool_trait.rs` envelope tests (8 tests) assert top-level/nested contract, context, suggestions and planted command/prompt/source/path removal. `tool_executor.rs` panic tests assert read/write panic suppression and `is_error`. | Keep these as shared-contract checks. They do not prove any registered builtin's failure becomes the same contract via `execute_batch` and then a failed provider-facing `ToolResult`. | `agent::tool_trait::tool_result_error_tests`; `handler_panics_become_structured_results_for_read_and_write_partitions` |
| Unknown name / handler argument shape | Engine has a production unknown-name path and one end-to-end scripted consumer regression. No common input-schema validator exists at the executor boundary: `input_schema()` is used for provider tool declarations; `execute_batch` receives already-decoded `Value` and calls the handler. | Keep unknown-tool test; add invalid/missing/wrong-type arguments as handler-produced failures for each registered concrete builtin (do not label this as an executor parser failure). Assert structured `is_error` reaches `MessageContent::ToolResult`. | `unknown_tool_failure_uses_structured_correlated_tool_result` |
| Permission and headless behavior | Production executor tests cover autonomous Bash consent, project install consent, headless host-approval refusal, cancellation/timeout/dead approval listener, and preserve denial vs timeout vs cancellation codes. | These are boundary examples, not an all-builtin matrix. Include permission-denied input for each permission/effect class actually registered and assert handler did not run; retain separate stable outcomes. | `p8_execution_registry_tests`; specific names `headless_host_approval_fails_immediately_without_running_handler`, `approval_cancellation_is_not_reported_as_user_denial`, `approval_timeout_and_closed_listener_have_distinct_decisions` |
| Handler / DB / infrastructure failures | Executor synthetic tools cover panic in read/write, journal intent/outcome DB errors and unknown side-effect quarantine. Concrete Bash structured nonzero, project-command build diagnostics and timeout have tests, but these concrete tool tests call `Tool::execute` directly, not `ToolExecutor` and the model consumer. | Cover at least one deterministic injected handler and DB failure through the registered tool families; return the `execute_batch` result through scripted `AgentEngine` and assert failed tool result. Specific registered families are: file/workspace (`read_file`, `write_file`, `edit_file`, `append_file`, `list_directory`, `grep`, `glob`, `check_file`, `git_commit`, `annotate_file`, `write_interface_spec`, `e2e_probe`); task/epic/budget/routing (`list_tasks`, `get_task`, `update_task_status`, `assign_task`, `assign_tasks_batch`, `create_task`, `claim_task`, `set_task_budget`, `create_epic`, `create_phased_task`, `get_epic_progress`, `get_ready_tasks_in_phase`, `suggest_worker`); Memory (`search_memory`, `get_memory`, `list_memories`, `propose_memory`); worker/ACP/reconciliation (`request_supervisor_action`, `list_worker_requests`, `resolve_worker_request`, `send_worker_message`, `list_worker_messages`, `list_stale_worker_messages`, `list_worker_health`, `reconcile_worker_operation`, `acknowledge_worker_message`); agent/planning (`planner`, `spawn_subagent`, `spawn_parallel_subagents`, `spawn_qc_review`); command/MCP (`bash`, `project_command`, `mcp_list_tools`, `mcp_call_tool`). Inventory comes from `register_for_mode(FastDevelopment | Review)` itself; use the live registry in the harness so later additions cannot silently escape coverage. | `handler_panics_become_structured_results_for_read_and_write_partitions`; `operation_journal_intent_failure_blocks_the_mutating_handler`; `operation_journal_outcome_failure_surfaces_unknown_without_retrying_the_effect`; concrete direct tests `nonzero_shell_exit_is_a_structured_failure_without_command_output`, `failed_compiler_output_becomes_source_free_structured_diagnostics` |
| Timeout / cancellation | Synthetic executor read cancellation and uncertain write cancellation prove correlated failure and side-effect handling. Bash/project-command direct tests prove tool-specific timeout outcomes. | Missing concrete registered-tool timeout plus model-consumer proof (Bash/project_command are existing low-risk representatives); missing provider request timeout/cancellation classification and production consumer readback. Do not infer from direct tool/helper tests. | `p8_execution_registry_tests`; `production_command_timeout_stops_nested_process_writes_and_records_stage_time` |
| Provider classification / privacy | Adapter methods have local fixtures for quota 429, one malformed unary success body, and unreachable unary transport on Google/OpenAI/Anthropic. Google also tests countTokens failure, oversized body; parser tests reject malformed SSE frames without echoing planted markers. Shared classifier tests cover 429 quota-vs-rate-limit, 503/529 overload, 500/502/504 retryable, fatal auth/request/unknown statuses. Engine has one persisted Overloaded regression for a 503 fixture. | Missing actual adapter HTTP tests for rate-limit 429 versus quota 429, 503 and Anthropic 529/500-504 classification; missing provider streaming/count-token transport and malformed-success consumer cases beyond the Google countTokens slices; missing timeout/cancel. Classifier-helper coverage is not adapter/AgentEngine consumer evidence. In the overload regression, the 503 is synthetic text fed to the AgentEngine failure classifier; it does not prove response-body classification at the adapter plus production readback. | `provider::`; `provider_error_body_is_not_forwarded`; `exhausted_provider_failure_is_recorded_without_retrying_or_persisting_body`; classifier tests in `provider::usage` |

### Recommended next implementation/test unit

Add one P4-51 production failure-matrix test unit around the live `FastDevelopment` registry and `ToolExecutor::execute_batch`, then drive its resulting error through the scripted AgentEngine provider consumer. For every registered tool, provide one deterministic success and one controlled failure (wrong-shape/missing input or its isolated handler/DB fixture); assert valid bounded envelope, `ok=false`, `is_error=true`, registered tool/operation and call correlation, no planted credentials/prompt/source/raw arguments/long OS text, and no success-shaped result. Add separate shared rows for each contract category: permission denial, executor handler/panic/DB infrastructure, concrete timeout, read/write cancellation, and unknown name. This is the missing unit TG4-25 literally prescribes; current representative tests can stay as focused regressions.

Before P4-51 closeout, add provider adapter fixtures for the missing status cases and timeout/cancellation outcomes, then prove adapter → `AgentEngine` failure classification → durable analytics/log reader for at least the privacy and overload/retry distinctions. Keep existing provider/ToolExecutor filters below as focused regressions; neither a direct classifier test nor a module-wide provider suite substitutes for consumer readback.

## Dated status reconciliation and commands

`TESTING_GATES.md` TG4-25's parenthetical in line 504 and its 2026-10-03 notes at lines 505-510 predate several 2026-10-04 updates. Specifically, line 511 says transport/malformed-response/SSE privacy remains open, but `MASTER_DEVELOPMENT_PLAN.md` lines 2459-2461 and `TESTING_GATES.md` lines 1108-1109 record those privacy slices as covered. Treat those as completed narrow slices. Current unresolved state is the full every-builtin/error-category/provider-consumer matrix, not absence of all provider privacy tests. `MASTER_DEVELOPMENT_PLAN.md` line 1528 itself already says the full inventory, provider timeout/cancellation, broader planted-input coverage and retry safety remain pending; the checkbox remains unchecked.

Useful current filters (not run during this read-only investigation):

- `cargo test -p arcana-core --lib agent::tool_trait::tool_result_error_tests --locked -- --test-threads=1`
- `cargo test -p arcana-core --lib p8_execution_registry_tests --locked -- --test-threads=1`
- `cargo test -p arcana-core --lib unknown_tool_failure_uses_structured_correlated_tool_result --locked -- --test-threads=1`
- `cargo test -p arcana-core --lib exhausted_provider_failure_is_recorded_without_retrying_or_persisting_body --locked -- --test-threads=1`
- `cargo test -p arcana-core --lib provider_error_body_is_not_forwarded --locked -- --test-threads=1`
- `cargo test -p arcana-core --lib provider:: --locked -- --test-threads=1`

## Scope and completion

Read only the named MDP/gate and implementation/test sources. No source changes, Git operations, credentials, environment files, databases, or tests were accessed. The report is an investigation and proposed next unit; it does not mark P4-51 or TG4-25 complete.

## Source reference index and audit correction

- Contract and status: `docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md:1528`; `docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md:2459-2461`; `docs/restructurization/development/TESTING_GATES.md:504-511,1108-1109`.
- Live registry and executor path: `arcana-core/src/agent/builtin_tools/mod.rs:114-202`; `arcana-core/src/agent/tool_executor.rs:493-512,586-` (handler normalization and the sole production executor API); `arcana-core/src/agent/engine.rs:995-1034,1214-1249` (unknown names, infrastructure conversion, failed result flag).
- Envelope and existing consumer regressions: `arcana-core/src/agent/tool_trait.rs:281-385,396-447,449-` (constructor, normalization, privacy, tests); `arcana-core/src/agent/tool_executor.rs:1646,1750,1954,2016,2408,2452,2509`; `arcana-core/src/agent/engine.rs:2661,2786`.
- Concrete tool examples are direct handler tests: `arcana-core/src/agent/builtin_tools/bash.rs:902`; `arcana-core/src/agent/builtin_tools/project_command.rs:2202,2522`.
- Provider classification/adapter evidence: `arcana-core/src/provider/usage.rs:275-430,888-967`; `arcana-core/src/provider/google_tests.rs:74-166`; `arcana-core/src/provider/openai_tests.rs:247,344-399`; `arcana-core/src/provider/anthropic_tests.rs:260,449-508`; engine persistence regression `arcana-core/src/agent/engine.rs:2786`.

Correction to the inventory table above: the first family list omitted `analyze_diff` from the task/board tools group. It is registered in `builtin_tools/mod.rs` immediately after `claim_task` and should be included. The table names the 47 FastDevelopment/Review registrations from the live registry, grouped by ownership; it does not imply each needs every failure category repeated independently.

Reference precision note: executor function body spans `tool_executor.rs:586-1237`; envelope tests are specifically `tool_trait.rs:449-680` plus `tool_trait.rs:992-1014`. The Task/board group in the matrix includes `analyze_diff` per the correction above.

## 2026-10-06 implementation-unit log: provider HTTP classification fixtures

- Starting bounded test-only implementation for adapter-level HTTP classification requested by the parent task. Owned source files: `arcana-core/src/provider/google_tests.rs`, `openai_tests.rs`, `anthropic_tests.rs`; this report is the only owned document. No tests will be run here.

### Source check before edits

- Re-read MDP P4-51 and TG4-25: their relevant clauses remain stable structured failure, classifications, bounded/redacted details and consumer-proof requirements; this bounded unit adds adapter fixtures only and cannot close either full gate.
- Existing adapter error fixtures use a reusable loopback response server in `provider::usage::test_response_server(body,status,reason)`. Provider methods build final diagnostics as fixed provider/status/class/summary strings; OpenAI/Anthropic use `provider_error_response`, Google exposes only its allowlisted status/field summary.
- Existing rate-vs-quota classifier tests use Anthropic `rate_limit_error` text without “quota” for Retryable, and invalid/credit wording for QuotaExhausted. The existing Anthropic privacy test mixed `rate_limit_error` with a planted “quota exceeded” phrase and asserted QuotaExhausted. I notified the parent; the test-only correction will preserve the private marker but use a consistent rate-limit message and add a separate actual quota response case. Production code remains unchanged.

### Test-only edits applied (not executed)

- Added `google_tests::adapter_http_failures_keep_class_and_safe_google_summary`: real `GoogleProvider::complete` loopback responses cover retryable rate-limit 429, quota-exhausted 429, overloaded 503, and retryable 500/502/504; each asserts exact stable safe summary and omission of the planted body marker.
- Added `openai_tests::adapter_http_failures_keep_class_and_fixed_summary`: real `OpenAIProvider::complete` loopback responses cover the same 429 distinction, overloaded 503, and retryable 500/502/504; each asserts fixed `provider_error_response` and body-marker omission.
- Added `anthropic_tests::adapter_http_failures_keep_class_and_fixed_summary`: real `AnthropicProvider::complete` loopback responses cover rate-limit vs credit/quota 429, overloaded 503 and vendor-specific 529, and retryable 500/502/504; each asserts fixed summary and body-marker omission.
- Corrected existing Anthropic privacy fixture to a genuine rate-limit message and expected `Retryable`; quota wording is covered separately by the new credit-balance row. No provider implementation or public API was changed. No test command was run as requested; the parent owns execution.

### Coverage refinement

The classification tables now drive both unary `complete` and streaming `stream` methods for every listed HTTP status, using a fresh loopback response fixture for each call. Google asserts the provider-specific allowlisted status summary; OpenAI and Anthropic assert the exact fixed `provider_error_response` summary. This avoids treating coverage of one adapter request method as evidence for its separate stream-status branch.

### Handoff status

Exact new test names and locations:

- `adapter_http_failures_keep_class_and_safe_google_summary` — `arcana-core/src/provider/google_tests.rs:171`.
- `adapter_http_failures_keep_class_and_fixed_summary` — `arcana-core/src/provider/openai_tests.rs:401` and `arcana-core/src/provider/anthropic_tests.rs:510`.
- The existing `provider_error_body_is_not_forwarded_to_anthropic_consumers` was corrected at `anthropic_tests.rs:457-480`: its rate-limit response now has a private rate-limit marker without quota wording and expects Retryable for unary and stream calls. The new table carries a distinct credit-balance QuotaExhausted case.

Each new table invokes both provider `complete` and `stream` methods with local HTTP fixtures. Cases cover 429 Retryable vs QuotaExhausted, 503 Overloaded, Anthropic 529 Overloaded, and 500/502/504 Retryable, with per-provider fixed/allowlisted summaries and planted body marker exclusion. Production APIs were left untouched. No test, build, or external network command was run. This unit supplies provider adapter evidence only; TG4-25/P4-51 remain open for AgentEngine/provider persistence readback, timeout/cancellation and the full builtin ToolExecutor matrix.

## 2026-10-06 fixture assertion correction

- Parent ran the new classification tests and reported three assertion failures: `ArcanaError` Display prepends `Provider error: `; the Google exact equality and OpenAI/Anthropic `starts_with` assertions omitted that established prefix. The classification/body-summary assertions were not the reported cause.
- Applying only that prefix correction in the three owned provider test files. No tests or Git commands will be run here; parent retains test execution and production changes.
- The first narrow patch did not apply because the Anthropic assertion had been rustfmt-shaped across multiple lines; it changed no file. I inspected only the three owned assertion contexts and will apply exact local hunks.
- Updated exactly the three expected prefixes to include `Provider error: ` in Google equality and OpenAI/Anthropic prefixes. Source inspection confirms those owned assertions now reflect the parent-reported ArcanaError Display shape. No tests, Git commands, or production source changes.

## Main integration: actual consumer defect

The actual Google adapter→AgentEngine→Analytics regression failed before correction: an error after stream establishment bypasses the API-failure writer, and successful establishment after a retry was recorded as recovered before consuming its body. ADR-0327 records a bounded fix within the existing method/collector/classifier. The real HTTP fixture now sends a malformed SSE frame rather than generic non-SSE text; it proves the intended body-error path. Separate actual adapter cases cover terminal quota and a malformed frame, and a two-request fixture covers 503 recovery followed by partial output and a malformed frame, requiring exactly one unresolved aggregate with one retry. Tests are in progress; no gate closure.

## 2026-10-06 focused review: stream-response analytics fix

- Starting read-only review of the main-session `AgentEngine` stream-consumption/recovery change and `actual_google_*` consumer tests. Review only the changed flow, terminal/midstream recording, no-retry-after-output and existing failed/overloaded classification. No source writes, tests, Git commands, credentials, or live state.

### Focused flow review

- Recovery is deferred to the right boundary for a stream call: `with_retry` wraps only `provider.stream(request)` establishment; the returned chunk stream is consumed afterward. A retry can therefore happen only before any text/tool chunks have been observed. Once a chunk error occurs, the consumer calls `record_failure` and returns immediately; there is no enclosing retry loop to replay emitted output.
- The terminal opening-error path and stream-consumption error path are mutually exclusive: an `Err` from `with_retry` records then returns before entering the consumption loop; a stream-item `Err` records then returns from inside that loop. Recovered establishment errors are recorded only after the opened stream is fully consumed successfully, yielding one aggregate with `resolved=true`. The terminal/midstream test's fixture count and `retry_count == 1` guard the one pre-output retry, while it expects one unresolved row after the body fails.
- Classification behavior remains compatible with existing tags: `[QuotaExhausted]`, `[Overloaded]`, and `[Retryable]` map to the existing analytics labels; the final malformed Google SSE frame has no tag and is intentionally classified `fatal`. Quota remains non-retryable in `provider::retry`; overload/retryable statuses may retry only at establishment. Analytics messages are generated from fixed class/attempt/outcome fields and do not copy the provider error or streamed output.
- The actual adapter tests use an isolated analytics home, capture a pre-call row ID, invoke the real Google adapter through `AgentEngine`, then read `api_failures` back through the production analytics reader. The midstream test returns one 503 followed by partial text and a malformed frame; it asserts exactly two accepted fixture responses, one new analytics row, retry_count 1, unresolved status, fatal terminal class, and absence of planted provider/prompt/output text.

### Review result

No material correctness, privacy, duplicate-recording, or no-retry-after-output defect found in the reviewed diff. This is bounded to the Google adapter-backed terminal and midstream consumer paths and the `stream_response` retry/record branches. It does not establish equivalent midstream evidence for OpenAI/Anthropic or cancellation while the stream future is dropped; those are outside this focused review and remain distinct coverage gaps if later required. No source files were changed and no tests were run.

Verified main results so far: complete Windows provider module129/129 (6.75s) after correcting the draft assertions for the existing ArcanaError prefix and stream operation labels. Actual Google terminal/body and retry-then-body-error consumers pass Windows2/2 (0.92s) and Linux2/2 (1.66s), with durable production-reader readback and one aggregate per call. Nine model-facing concrete builtin failures pass Windows1/1 (0.77s); this consumer matrix is being extended to match the executor slice before final verification. The first executor inventory draft incorrectly called the registered tool assign_tasks_batch instead of its actual name assign_tasks, used a Vec where set operations require BTreeSet, and treated generic word invalid as a unique private marker. These were fixture errors; corrected initial ten-tool executor slice passes Windows1/1 (0.50s). No production scope is narrowed and P4-51/TG4-25 remain open for uncovered tools/categories.

## 2026-10-06 implementation slice: registered external MCP tool path

- Starting a test-only extension of the existing local Streamable HTTP MCP fixture. Owned code scope is only the `cfg(test)` module in `arcana-core/src/agent/builtin_tools/external_mcp.rs`; append-only updates go in this report. Planned proof: invoke live FastDevelopment registry `mcp_list_tools` and `mcp_call_tool` via `ToolExecutor::execute_batch`, revoke consent, repeat with planted private arguments, verify normalized correlated errors and zero remote effects. No tests/Git/live resources; main will run the named filter.
- Scope clarification from parent: `engine.rs::agent_engine_calls_configured_mcp_through_production_executor` already proves positive model-consumer list/call and RunsDb readback. This slice will add direct `ToolExecutor::execute_batch` failure-envelope and revoked-consent/no-remote-effect proof only; it will not duplicate model-consumer proof or describe it as missing.

### Fixture design before edits

- Positive model-consumer list/call plus RunsDb readback already exists in `engine.rs::agent_engine_calls_configured_mcp_through_production_executor`; this new case specifically covers ToolExecutor normalization/correlation and post-revocation no-effect behavior.
- The existing fixture's remote counters represent `tools/list` and `tools/call`; direct-handler and direct-client positive paths currently leave both at 2. The added executor batch contributes one of each, so successful and all revoked-path assertions must expect 3. MCP calls are serialized by the executor because they have `ToolEffect::Network`.
- A worker run row is required for the run-scoped operation journal; create it in the fixture's isolated home, then pass that returned run ID to the executor. Workspace consent is revoked before the second executor batch; both handlers authorize before connecting, so remote list/call counters should remain unchanged.
- The existing server spawned detached connection tasks. I will track those tasks in a `JoinSet` and use a oneshot shutdown, abort and join remaining connections before the test returns, preserving loopback-only behavior and preventing listener/task leaks.

### MCP fixture implementation progress
- Added owned connection-task tracking with a shutdown oneshot to the existing loopback fixture; shutdown aborts and drains any active request tasks before the server task exits. This replaces detached request-task cleanup and will be joined by the test.
- No tests were run. Next: inspect the current fixture, add production ToolExecutor success/revocation assertions, and replace the old server abort with a bounded graceful join.

- Extended streamable_http_client_discovers_and_invokes_a_local_mcp_tool with registered FastDevelopment MCP tools sent through the production ToolExecutor and a persisted worker run ID. The allowed batch asserts actual remote list/call results; expected fixture effects are now three tools/list and three tools/call requests after existing direct-handler and low-level-client cases.
- Added a post-revocation executor batch with planted private cursor/call arguments. It asserts normalized mcp_operation_failed / external_service envelopes, operation/tool identity, session/worker/run/call correlation, timestamp, marker redaction, and unchanged remote list/call counters. Existing engine-level positive consumer proof remains owned by the parent and is not duplicated here.
- Replaced detached fixture request tasks with JoinSet ownership and replaced abort-at-end with a bounded shutdown signal + server task join. No tests were run; source-level compilation/format still awaits the parent’s exact filter.

### Final MCP slice handoff

- Exact modified test: `streamable_http_client_discovers_and_invokes_a_local_mcp_tool` in `arcana-core/src/agent/builtin_tools/external_mcp.rs`.
- The fixture now covers production FastDevelopment registry tools through `ToolExecutor::execute_batch`: one successful list/call batch while consent is granted, then one denied list/call batch after revocation. The denied cases verify the normalized envelope and exact executor-owned correlation fields, redact both planted arguments, and leave the remote `tools/list` and `tools/call` counters at three each.
- Fixture shutdown sends the oneshot, then awaits the server task with a two-second bound. The server aborts and drains any in-flight connection tasks before returning, so no listener or child request task is intentionally left running.
- Existing engine-level test `agent_engine_calls_configured_mcp_through_production_executor` already covers provider/model-consumer list/call with RunsDb readback; this slice adds the direct executor/revocation case and does not duplicate that coverage.
- Verification intentionally not run by this agent. Parent-requested exact filter: `cargo test -p arcana-core --lib streamable_http_client_discovers_and_invokes_a_local_mcp_tool --locked -- --test-threads=1`.
- Tightened the denied envelope assertion against the source-backed fixed consent summary and `retryable=false`, in addition to code/category/correlation/redaction. No tests were run.

## Final suite triage (main session)

The first expanded engine run passed 24/26 on Windows and 25/26 on Linux. The recovered-call fixture incorrectly assumed the shared analytics history contained no earlier retry; it now computes the expected reader aggregate from persisted rows while retaining the provider-specific resolved record assertion. The Windows terminal adapter fixture also exposed unstable response classification before request-body consumption. The test-only HTTP server now consumes bounded Content-Length input before closing, checks response writes, and bounds accepts/reads/writes. Its first nonblocking-accept version reproduced Windows accepted-socket inheritance (WouldBlock on request-line read); the accepted socket is explicitly returned to blocking mode with timeouts. These are fixture corrections, not changes to provider retry or classification policy. Final suites are rerunning; no gate closure is claimed.

The real malformed-argument test failed before correction because list_directory successfully enumerated its default workspace. Invalid nonempty JSON, arrays and null now return correlated invalid_tool_arguments/argument_validation without dispatch; empty deltas and valid empty objects remain successful controls. Replay inputs are asserted to remain objects. The expanded seventeen-builtin model consumer matrix passes in the engine run; complete final results and host checks are pending.

Independent Luna High source review found no material engine-boundary defect. Its report records that provider-failure analytics rows do not carry a request/session identity; separate provider tracing remains the session-correlated surface. This slice does not claim a new analytics correlation contract.

## Verified engine/executor/MCP/provider unit

| Production selection | Windows | Linux WSL |
| --- | --- | --- |
| agent::engine:: | 26/26, 7.46s | 26/26, 18.54s |
| p8_execution_registry_tests | 18/18, 2.61s | 18/18, 3.11s |
| builtin_tools::external_mcp:: | 9/9, 1.19s | 9/9, 2.21s |
| provider:: | 129/129, 7.04s | 129/129, 10.14s |

All selections use cargo test --offline -p arcana-core --lib FILTER --locked -- --test-threads=1, no live credentials, isolated stores and loopback fixtures. Logs are under ignored target/p4-ci-checks/final-*2026-10-06.log; the old final-executor-linux log is the preserved failed draft, superseded by final-p8_execution_registry_tests-linux. Windows cargo check passes (23.67s). The shared HTTP fixture explicitly restores blocking mode on accepted Windows sockets and consumes the bounded request body; final adapter classification assertions pass unchanged. Independent source review reports no material engine-boundary defect. Current registered-tool executor coverage in this unit is15/47 (13 core +2 MCP), with17 concrete model-consumer failures and separate parsing controls. Full TG4-25/P4-51 remains open. Memory/ACP/artifact/board source candidates are outside this unit and are not claimed passing.
