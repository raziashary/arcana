# P4-51 / TG4-25 Memory Tool Matrix

- Status: test implementation ready for parent verification; no test execution in this delegated turn.
- Scope: success/failure matrix for registered `search_memory`, `get_memory`, `list_memories`, and `propose_memory` using the real registry and production executor.
- Constraints: edits limited to the `cfg(test)` section of `arcana-core/src/agent/builtin_tools/get_memory.rs` and this report. Isolated temp home/workspace only; no live database, credentials, network calls, or Git mutations.

## Investigation log

- Confirmed checkout root `C:/arcana`, current branch `main`; the checkout has broad pre-existing changes in engine/executor and several planning files. The assigned `get_memory.rs` did not appear in the initial modified-path list.
- No project-local `AGENTS.md` was found by the repository search.
- Current `get_memory.rs` includes direct-tool observability tests, while the requested matrix must exercise registry registration and `ToolExecutor::execute_batch`.
- Registered memory surfaces are referenced in `arcana-core/src/agent/builtin_tools/mod.rs`; P4 verification must avoid the active user database by installing the existing isolated-home test guard.

## Contract and fixture findings

- TG4-25 explicitly requires each failure to be a bounded, versioned JSON envelope with stable classification, operation/tool/call correlation and safe actionable fields; it also requires `is_error=true`, redaction of planted input, and no success-shaped result.
- The assigned slice is limited to the four real memory registrations. Its report and test cannot close the parent gate, which also requires every other builtin, shared error categories, timeout/cancellation and provider-consumer coverage.
- Existing isolated DB support is `crate::db::test_support::with_home`; the memory DB is SQLite under Arcana home. The live `FastDevelopment` registry registers exactly the four memory tools requested, but the registry constructor and executor APIs still need inspection before authoring.

## Harness design findings

- `ToolExecutor::execute_batch` receives `(Arc<dyn Tool>, input, tool_use_id)`, derives `call_id` as `<session>-<tool_use_id>`, and attaches executor-owned session/agent/task/run context to failures. Autonomous mode permits the `propose_memory` board write without interactive approval.
- `AgentIdentity::worker(...)`, `ToolRegistry::new()`, and `register_for_mode(..., PAMode::FastDevelopment)` provide the requested production identity and registry; each operation can be invoked by retrieving its registered `Arc<dyn Tool>` and passing it to the production batch API.
- The durable success path can be proven in one isolated home by proposing valid synthetic content, parsing the returned `mem-...` ID, then reading it back through the registered `get_memory`, `list_memories`, and `search_memory` handlers.
- Missing/invalid required values provide deterministic malformed failures for `search_memory`, `get_memory`, and `propose_memory`; `list_memories` has no required parameters, so its failure needs an isolated schema obstruction. Plan is to drop only `memories` in the temp-home MemoryDb after success/readback and invoke list against that failure. No user DB is touched.

- Error fixtures will be one case per registered tool: malformed required input for search/get/propose, plus a missing-`memories` table fixture for `list_memories` (which accepts an empty input). Assertions will check versioned failure shape, stable identity/correlation, bounded output/fields, and exclusion of planted credential, prompt, source, argument, and long diagnostic strings.
- Successful write/readback assertions will run before the isolated table obstruction. `ToolExecutor` receives a worker identity, synthetic run/task IDs and the temporary workspace; its proposal journal therefore also stays under the redirected temp home.

## Implementation record

- Added `p4_51_memory_executor_matrix_tests::memory_registered_executor_success_failure_matrix` in the existing `cfg(test)` area of `arcana-core/src/agent/builtin_tools/get_memory.rs`.
- The test builds the live FastDevelopment registry, verifies all four memory registrations, and invokes each through `ToolExecutor::execute_batch`.
- The success path uses a temporary home/workspace, creates an isolated RunsDb run for the executor journal, stores synthetic content with `propose_memory`, then verifies it via `get_memory`, `list_memories`, and `search_memory`.
- The failure path invokes malformed required arguments for search/get/propose and drops the `memories` table only in the temporary MemoryDb for list. All four cases check `is_error`, versioned bounded JSON, stable tool/operation identity, executor call/run/agent/task/session correlation, required error fields, absence of success-shaped fields, and omission of planted credential/prompt/source/argument/long-diagnostic markers.
- No tests, builds, smoke tests, or other validation commands were run, per the delegated instruction. No Git operations were performed. The expected parent-run filter is:

  `cargo test -p arcana-core --lib memory_registered_executor_success_failure_matrix --locked -- --test-threads=1`

## Evidence and limits

- Registry contract: `arcana-core/src/agent/builtin_tools/mod.rs` (`register_for_mode`, FastDevelopment arm).
- Production execution and failure wrapping: `arcana-core/src/agent/tool_executor.rs` (`ToolExecutor::execute_batch`, `begin_operation_journal`), plus `arcana-core/src/agent/tool_trait.rs` (`with_execution_context_and_input`).
- Tool success/failure paths: `arcana-core/src/agent/builtin_tools/{search_memory,get_memory,list_memories,propose_memory}.rs`; durable write and search behavior: `arcana-core/src/memory/gateway.rs`, `arcana-core/src/memory/recall.rs`, `arcana-core/src/memory/db.rs`.
- Isolated-home guard: `arcana-core/src/db/test_support.rs::with_home`.
- This unit covers only the four Memory registrations. Its deterministic failures are malformed inputs for three required-input tools and a temporary schema obstruction for `list_memories`; it does not exercise permission denial, timeout/cancellation, a long native OS error emitted by a handler, or an AgentEngine/provider consumer. P4-51 and TG4-25 remain open.
- Status: implementation ready for the assigned parent-run test; outcome unverified here as required.


## Main reproduction: shared JSON-output corruption

The real Memory executor matrix fails on Windows (0.33s) and Linux (0.44s) while parsing get_memory success JSON: the production shared text redactor matches digits inside a long decimal confidence and substitutes an unquoted marker. The stored memory is created/read; the executor's serialized-output redaction damages the consumer contract. This is a production defect, not a reason to weaken readback. A JSON-aware output policy decision is pending under Universal Rule4 because this shared boundary affects all tools. This candidate test source remains uncommitted and P4-51/TG4-25 stays open.


## Owner-approved correction and verified consumer, 2026-10-09

ADR0332 supersedes the pending policy choice above. JSON-aware output redaction preserves scalar types, redacts strings and masks sensitive-key values. The unchanged real registered Memory and routing reader assertions now pass on Windows/Linux (each matrix1/1); redaction22/22 and ToolExecutor18/18 also pass on both hosts. See P4-51-JSON-REDACTION-2026-10-09.md for commands and timings. This raises full registry matrix coverage to38/47 without closing P4-51/TG4-25.
