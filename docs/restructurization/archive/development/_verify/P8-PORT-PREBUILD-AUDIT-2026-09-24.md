# P8 retained-port pre-build refutation (2026-09-24)

Two independent read-only reviewers challenged
`P8-PORT-WORKPLAN-2026-09-24.md` under MDP §0.2m before new port code. These
findings are source-level, not test passes. The main-session corrections are
ADR-0058/0059 and the workplan changes below. TG8-02 remains open.

| Draft claim | Verdict | Source-grounded correction or measurement |
|---|---|---|
| P4-02 owns strict task parsing | **REFUTED** | P4-02 names `workers.status`; its parser remains P4. P8 owner ruling concerns task transitions. `WorkerStatus` omits the real `idle_no_provider` value. |
| Guarding `update_status_with_reason` covers all task writes | **REFUTED** | `db/tasks.rs` also writes status directly in claim, release, budget, unblock, and parent-completion paths. Inventory every writer or enforce at a database boundary; exercise recovery and unblock. |
| Old MCPBridge implies a new external MCP client | **REFUTED** | G4b A.12/C.7 describes Python calling Arcana's built-in Rust tools over HTTP. `ToolRegistry` now supplies those directly. External MCP has config/health, no agent tool caller. Prove the built-in model-definition→dispatch→result path; do not create a hosted MCP server or external tool client by implication (ADR-0059). |
| A persistent `(agent,session)` runtime map is necessary | **OVERSTATED** | Interactive, recipe and subagent paths construct fresh `AgentEngine`s; recipe/subagent sessions are intentionally ephemeral. Measure isolation/admission/idle cleanup on actual callers before retaining engines or imposing a 500-entry lifetime. |
| In-flight registry has a reader | **PARTIALLY CONFIRMED** | `ToolExecutor::execute_batch` is live, but no production query consumer exists. Name one and test blocked tool→reader sees active→completion/error/cancel cleanup, or prove native event/turn state equivalent under owner ruling. |
| New unified `ActionPlan` is needed | **REFUTED as motivated** | Retry already drives `engine.rs`; routing and supervisor recipe have separate consumers. Move a real decision consumer into a typed result and prove its action changes, or show these native decisions cover the retained behavior without a dead abstraction. |
| Prompt roster in text is authoritative | **OVERSTATED** | `WorkersDb::list` is editable, but `assign_task` checks static `FLEET_WORKERS`. Prompt and dispatch validator must share roster source; preserve stable/volatile prompt separation. |
| Prose `DELEGATE` parser is a live coding seam | **OVERSTATED** | Current provider loop dispatches structured `ToolUse` to `spawn_subagent`, with depth checked by the pool. A prose parser could become a second permission channel. Require an actual caller and equivalent permission/cycle proof, or accept native structured delegation under owner ruling. |
| A new internal broadcast bus has a subscriber | **REFUTED as written** | Core's `EventSink` forwards to Tauri, whose frontend has real readers. No Rust subscriber is named. A bus cannot replace the `OnceLock` sink without preserving the existing frontend channel; prove a named subscriber or native event delivery equivalence. |
| Anthropic signed thinking survives tool follow-up | **REFUTED** | The streaming parser drops signature deltas, engine forwards thinking to UI only, and request history cannot replay signed blocks. This is a real provider-path gap; test the exact next outbound body and guard privacy for Google/OpenAI. |

The owner subsequently ruled to prefer recreating and integrating old coding
mechanisms, with tested native equivalence where literal recreation conflicts
with Arcana's architecture (ADR-0060). This does **not** overturn the
refutations: every added mechanism needs a production reader and a
permission-safe path. Anthropic signed-thinking replay and task transition
enforcement were grounded as direct fixes before this ruling.
