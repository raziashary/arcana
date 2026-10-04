# P3 Cross-Phase Audit: Memory, Context, and Graph Delivery

Date: 2026-09-28
Scope: Read-only audit of Arcana P3 memory/context/graph delivery and current P4 boundaries. Governing sources include the P3 section of the Master Development Plan (MDP), TG3 gates, and current readiness/cross-phase audit records. Source and governing plan files remain read-only; this report is the only file written.

## Progress

- [x] Read governing P3 plan, TG3 gates, and latest readiness/cross-phase audit.
- [x] Trace production consumers for memory write, recall, procedural memory, graph, and code-context paths.
- [x] Classify findings as CONFIRMED, REFUTED, or PARTIAL; distinguish P3-owned proof from P4/P5-owned consumer proof.
- [x] Record stale or structural-only gate evidence, behavior actually proved, remaining end-to-end gaps, and harness refinements.

## Findings

Findings are recorded below with status, production consumer, current proof, and remaining ownership/gaps.

## Governing record snapshot

The MDP currently says P4 is active and P8 closed. Its P3-F entries are historically checked, but their own notes qualify the vector, provenance, retrieval, and procedural paths. The current readiness matrix explicitly says P3 is historical closeout only: eight memory gate clauses plus production-reader evidence remain open, and TG3-24/25/27/28 path/effect coverage is incomplete. It assigns complete TG3-26/TG4-16 persisted-consent consumer proof to P4 under ADR-0068. P5 owns graph consumer proof; the readiness entry says graph delivery remains open.

The latest cross-phase audit (2026-09-27, with a 2026-09-28 consent follow-up) confirms that P3 memory source and focused tests exist but does not infer consumer proof from writers or unit tests. It states TG3-26/TG4-16 remains partial and P4-owned, with the provider-assisted diff request/log/revoke and actual AnalyzeDiffTool repository-diff path open. It also records P1 graph tools as unavailable to coding turns and assigns real graph access to P5. The latest P4 checkpoint still lists TG3-26/TG4-16 completion as pending. The 2026-09-28 P8 closeout audit is the latest dated global closeout artifact; the 2026-09-27 harness audit is the latest cross-phase disposition.

This is the claim baseline only. Each item below is checked against current source consumers.

## Progress update

- [x] Read MDP P3/P4 task records, TG3 register, readiness §2.1, latest cross-phase audit, and current P4 checkpoint.
- [x] Trace current production consumers and match each to the gate/task evidence.
- [x] Classify outcomes and record harness refinements.

## Source-to-consumer findings

### 1. Semantic memory write → recall tools and automatic prompt

**PARTIAL — mechanism is live; the P3 gates are not satisfied.** `builtin_tools::register_for_mode` registers `ProposeMemoryTool`, `SearchMemoryTool`, `GetMemoryTool`, and `ListMemoriesTool` in FastDevelopment/Review and Delegate. `ProposeMemoryTool::execute` derives proposer from runtime `ToolContext.agent_role`, opens `MemoryDb`, and calls `gateway::propose`. The gateway redacts before persistence, clamps confidence to the proposer ceiling, checks minimum substance and same-tier/same-scope duplicate overlap, then writes `memories`. `Proposer::from_agent_role` takes only a role argument; model input cannot select the user provenance. This confirms a production write path, source attribution, input redaction, and retrieval surfaces.

The normal interactive `AgentEngine::run` calls `build_system_prompt_parts` with the current user message; the override arm separately calls `ContextBuilder::memory_block(working_dir, user_message)`. `memory_block` calls `build_memory_context`, which calls `recall`; that opens the memory DB, obtains `active_backend`, searches, ranks and budgets candidates, records access, and renders provenance plus flattened/truncated content. `engine.rs` joins stable then volatile prompt and passes the resulting system text to `CompletionRequest`. Thus a real automatic prompt consumer exists in source. `search_memory` also invokes `search_all` with the caller's workspace and query, and `get_memory`/`list_memories` independently apply the same global-plus-project scope filter, so the read tools do not simply trust an earlier search filter.

Remaining P3 gaps are material: `recall` ignores `session_id`; `scope_keys_for` emits only `global` and optional `project:{working_dir}` (no task/agent scope); default backend is lexical unless the optional embeddings feature is enabled and ready; readiness says `memories_vec` is selected but not product-created/written, so the vector gate is not established by the throwaway vec0 fixture. The model tool can write only the semantic tier via `propose_memory`; the current gate audit says 5/11 declared subtypes lack an end-to-end writer, with episodic/procedural/session-summary paths reachable only through Tauri. The schema accepts unrecognised/defaulted source values without a CHECK or write validation; `repository` provenance is unreachable and cannot meet its file/line contract. Gateway policy has no importance threshold, confidence is clamped rather than rejected below a floor, and novelty is only the duplicate Jaccard threshold.

**Evidence actually present:** the current readiness matrix records TG3-11 NOT SATISFIED, TG3-12 PARTIAL, TG3-13 NOT SATISFIED, TG3-14 NOT SATISFIED, TG3-17 PARTIAL, TG3-19 NOT SATISFIED, TG3-20 NOT SATISFIED, and TG3-21 PARTIAL. TG3-11 specifically lacks an assertion on serialized provider request bytes and lacks an isolated DB seam; the current context unit test accepts either empty output or any output containing `Memory`, so it cannot prove an inserted memory reaches the request. The query signature test only checks that the builder accepts a query; it does not prove different prompts select different records. The legacy `P3-44/45` checkmarks and “GATE-PROVEN” task notes therefore overstate current reader proof. `P3-31` similarly proves sqlite-vec partition-key behavior using a test-created table, not the product's vector store. `P3-38` demonstrates display mapping, not the promised complete source constraints.

**Gate/task ownership:** P3 owns TG3-11…14, 17, 19…21 and the memory writer/reader obligations in P3-F (including P3-31, P3-38…43, P3-45…47). P4 owns only end-to-end value proof TG3-23 and separately the persisted-consent cross-phase contract TG3-26/TG4-16; those assignments do not transfer the open P3 memory correctness gates to P4.

### 2. Procedural memory → fleet provider request

**CONFIRMED for the P4 worker consumer, PARTIAL for overall procedural-memory delivery.** `ToolExecutor::retrieve_procedural` is opt-in, runs once above the batch loop in `execute_batch`, builds a query from the invoked tool names, calls `recall::procedural_context`, and stores the rendered result for one provider request. `AgentEngine` consumes it with `take_procedural_block` when building the next `CompletionRequest.system`; because it is taken, it is not retained in history or later requests. Interactive `AgentEngine` defaults this switch off; `recipes::runner` explicitly opts fleet workers in. Existing test source `worker_procedural_memory_reaches_only_the_next_matching_provider_request` builds an isolated Arcana home, writes a procedure, uses a scripted provider, and checks matching recall appears only on the next captured provider request, unrelated tools do not retrieve it, and default-off behavior persists. This is behavior-shaped production-consumer evidence, although this audit did not run it.

The P3-owned persistence coverage remains partial: TG3-14's existing verdict says procedural/session-summary writes are reachable only from Tauri, and the cross-tier model-facing `propose_memory` writer only creates semantic memories. P8-40/TG8-10 establish the generic seam, off-by-default state, one-query position and flattener; P4-44 is the worker opt-in and consumer proof. Do not attribute P4-44's consumer proof to P3, or use the staged P8 seam as proof of the earlier P3 all-tier writer contract. TG3-14 stays P3-owned/open; P4-44 is checked with a worker-path test, but the available test proves request delivery, not reductions in rediscovered work (TG3-23, P4-owned).

### 3. Graph and code-context path

**REFUTED for coding-turn graph context; CONFIRMED for graph UI persistence/readback.** `arcana-core/src/agent/builtin_tools/graph_tools.rs` defines 27 tool-shaped structs with static `name`/`execute` methods, but they do not implement the agent `Tool` trait. `builtin_tools::mod` declares the module and does not register graph tools; a repository-wide call-site search found no production caller for `GetMinimalContextTool`, `GetReviewContextTool`, or `query_by_name`/dependency query functions outside graph code. `graph_prompts.rs` contains a review recipe-like prompt naming those tools, but there is no registered callable consumer. Consequently graph analysis is not read into `AgentEngine` context or outbound coding prompt today.

A separate graph UI path does exist: Tauri `graph_commands::build_graph` scans/extracts/builds and stores a graph; commands such as `get_graph_nodes_for_viz`, `get_graph_edges_for_viz`, and `search_graph_nodes` read it for `src/tabs/Graph.tsx`. This confirms graph persistence and visualization consumers, not task-context selection. `GraphBuilder::build_graph` itself remains a no-op placeholder; the live Tauri command uses the free `graph::build::build` path after extraction.

The owner boundary is explicit: `TG3-15` was merged from P3 to P5 and `TESTING_GATES.md` says P5 owns it; MDP P5-14 is the graph-read-during-context-selection criterion, P5-17 requires a recalled node in outbound prompt bytes, P5-18 requires the 27 structs to implement `Tool`, and P5-19 requires registration/callability. P5 is open in readiness. The 2026-09-27 harness audit explicitly leaves graph unavailable until P5 proves a real consumer and says TG1-01 remains open. Therefore do not count graph UI commands or struct presence as P3 graph-context proof, and do not silently pull P5 work into the P3 memory audit.

### 4. P3/P4 persisted workspace egress consent boundary

**PARTIAL — the consent architecture and several readers exist; complete P4 consumer proof is open.** P3-56 delivered the Network `ToolEffect` prompting behavior; source comments and the MDP's required cross-phase text assign the persisted consent store and production readers to P4. `AgentEngine` only wraps providers when constructed with `with_workspace_egress_consent`; Tauri interactive setup and `RecipeRunner` opt in. `ConsentProvider::wrap` is the common request boundary. `AnalyzeDiffTool` has a workspace-consent path that returns deterministic structural review without looking up a provider key when no matching grant exists. The Tauri `analyze_diff` and `analyze_pr_diff` commands both call `analyze_with_workspace_consent`, and Settings commands read/grant/revoke the persisted selected-workspace grant.

Current evidence is focused and partial: the 2026-09-28 harness audit records provider wrapper allow/block/log/recheck tests, Settings grant/read/revoke, QC fallback without provider key, and direct Tauri diff mismatch fallback. It leaves full allowed request/log/revoke through every production consumer, the actual `AnalyzeDiffTool` Git-diff path, and Tauri PR-diff invocation open. Source shows `analyze_pr_diff` reaches the shared wrapper, but no cited behavior test exercises that command. Therefore TG3-26 and TG4-16 remain P4-owned/open under ADR-0068/0073; P3-56's Network prompt and the P3 general permission checks do not close consent, and the missing P4 proof must not be charged to P3.

## Stale, structural-only, and incomplete proof inventory

- **Historical P3 closure vs live register:** MDP P3-F tasks remain checked and use `GATE-PROVEN`/`UNIT-PROVEN` wording, while the current readiness matrix and gate register explicitly leave eight memory gate clauses open or partial. Readiness and TG3 clause-level verdicts govern present readiness; the historical checkbox preserves that P3 work was once declared complete and does not establish current phase acceptance.
- **Structural tests with weak or missing failure shapes:** `ContextBuilder::the_memory_block_is_reachable_without_going_through_the_full_prompt` accepts empty or any text containing “Memory”; `the_prompt_builder_demands_a_query` proves a signature only. Neither uses a seeded isolated store plus captured provider request bytes. The vector partition test creates its own schema, detached from migrations/writers. These tests can remain useful unit evidence only if labelled at that layer.
- **Behavior gates still pending/partial:** TG3-11 serialized request bytes; TG3-12 cache/negative-cache clauses where no cache/marker exists; TG3-13 product KNN plus meaningful scope corpus and task/agent scope; TG3-14 subtype writer→reader coverage; TG3-17 degradation state through the search tool (search_all currently returns entries only, so its caller drops `degraded_reason`; the tool maps empty results to “No memory entries found”); TG3-19 download resume/retry; TG3-20 source validation and repository file/line constraints; TG3-21 importance/confidence/novelty gateway checks. TG3-23 is P4-owned repeated-task value measurement. TG3-24/25/27/28 path/effect proof remains incomplete per readiness.
- **Graph tests/gates must be against the harness consumer:** TG3-15 should execute the real task-context selection path with a labelled repo, capture the exact outbound request, compare the selected relevant files against the no-graph baseline, and falsify both precision and recall. P5-18/19 must prove the same registered `Tool` and tool-call path used by AgentEngine; pure struct execution and Tauri Graph UI snapshots do not satisfy them.
- **Consent gates must enumerate all egress readers, not only wrappers:** preserve tests for matching grant, missing/mismatched grant, revoke between requests, and audit log at provider, QC AnalyzeDiffTool, Tauri direct diff, and Tauri PR-diff consumers. A wrapper's unit test or shared helper is not evidence each production entry point uses it.
- **Arcana harness alignment:** treat the provider request as the observable harness boundary. Seed only isolated app data; submit a task/query through the real AgentEngine/RecipeRunner entry point; capture serialized system/request bytes, selected file identities, tool effects, consent decision/log, and final reader outcome. Add positive and negative controls so a permanently empty recall, all-refusing graph selector, or an unused consent wrapper cannot pass. Keep deterministic local memory/context and graph selection tests separate from optional-provider or model-download smokes; report degradation explicitly to both prompt assembly and tool output. Tie each test to its production construction site and enumerate every consumer at the gate, while keeping ownership rows explicit (P3 memory correctness, P4 task-value/consent, P5 graph context).

## Progress

- [x] Read governing P3 plan, TG3 gates, current readiness matrix, cross-phase audit, and P4 checkpoint.
- [x] Trace production consumers for memory write/recall, procedural memory, graph UI/context, and consent egress.
- [x] Classify findings and separate P3, P4, and P5 ownership.
- [x] Identify stale/structural-only evidence and propose harness refinements.

Constraints observed: source, plans, gates, and other audit records were read-only. No tests, app, server, live database/provider, secrets, or Git operations were run. Only this report was written.

## P4 follow-up evidence addendum — 2026-09-28

After this audit, `analyze_diff_tool_executes_repository_diff_without_consent_or_provider_key`
was added and passed. It exercises the real `AnalyzeDiffTool::execute` path
against an isolated Git branch diff with no consent or provider key, verifies
deterministic-only output, and reads the persisted denied `qc_diff_analysis`
audit row. This closes the actual QC Git-diff path gap identified above; it
does not close the Tauri `analyze_pr_diff` command or prove a matching/revoked
consent path through every desktop egress consumer. TG3-26/TG4-16 remain P4
owned and open for those remaining cases.

### Consent-path audit correction — 2026-09-28

The earlier finding and addendum above are superseded for the
`AnalyzeDiffTool` repository-diff path: the later
`analyze_diff_tool_executes_repository_diff_without_consent_or_provider_key`
test exercises that production tool against an isolated Git diff, verifies the
deterministic fallback without key lookup, and reads the persisted denied
egress-audit row. Remaining consent proof is still open for the Tauri
`analyze_pr_diff` invocation and the complete matching/revoked-consent path
across desktop consumers. TG3-26/TG4-16 remain P4-owned and open.


\n