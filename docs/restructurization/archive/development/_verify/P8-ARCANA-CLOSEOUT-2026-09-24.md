# P8 Arcana closeout evidence (2026-09-24)

This record supplements the historical P8 baseline. It reports current Arcana
checks, not the old AutoDev run totals. P8 closes on the evidence below; P4 is
the next active implementation phase.

## Live desktop coding turn

- Built `target/debug/arcana.exe` from the current tree with
  `cargo build -p arcana --bin arcana --locked` (exit 0), served the frontend with
  `npm run dev -- --host 127.0.0.1`, and launched the Tauri app on Windows.
- Active provider: Google `gemini-3.7-flash`, using the Arcana OS vault. The
  credential was loaded from the ignored `.env` into the app through its vault
  command; neither this record nor any build command contains the key.
- Desktop session `13fdac95-d2e1-4bea-83e3-ba168a9f54f4`, created
  `2026-09-24T05:09:29Z`, working directory `C:\arcana`.
- Through the visible Chat UI, requested a tool-based read of `package.json`
  and asked for the package name and TypeScript type-check script, with no edits
  or commands. The transcript showed two successful `read_file` calls and a
  final answer: `arcana` and `"typecheck": "tsc --noEmit"`. Those values match
  the file. The UI displayed 23,883 input tokens, 63 output tokens, and $0.0186.
- The first attempt exposed a real Google adapter error: putting the shared
  JSON Schema in `functionDeclarations[].parameters` caused a 400 rejection on
  `additionalProperties`. The corrected adapter uses `parametersJsonSchema`;
  the targeted conformance test passed before rebuilding and repeating this
  successful desktop turn (ADR-0052).
- A separate process-ancestry sampler was started before the successful turn,
  rooted at Arcana PID 12412. It inspected Python processes and walked each
  parent chain at 750 ms intervals for four minutes. It completed without an
  application-owned Python process being logged. The app process was stopped
  after the successful turn.

## Current verification register

| Check | Result |
|---|---|
| Google tool-schema conformance test | pass, 1/1 |
| Desktop debug build | pass |
| Gemini tool-backed desktop turn | pass, session above |
| Python process-ancestry sample | pass, no application-owned Python logged |
| Full isolated Rust all-targets suite | pass, 673 tests across 16 targets, 0 failed; TG8-01 proven under ADR-0055's built-gate scope; includes the final TG8-12 lease-scope check |
| Frontend production build | pass, `npm run build` exit 0; Vite chunk-size warnings |
| TypeScript | pass, `npm run typecheck` exit 0 |
| Frontend unit tests | pass, `npm test -- --run` 17/17 across 4 files |
| ESLint | pass, `npm run lint` exit 0 with 42 existing warnings, 0 errors |
| Rust compile | pass, `cargo check --workspace --all-targets --locked` exit 0, warnings |
| Clippy | pass, `cargo clippy --workspace --all-targets --locked` exit 0, warnings |
| Rust formatting | pass after owner-approved `cargo fmt --all`; `cargo fmt --all -- --check` exit 0; 124 total changed files include behavior/docs edits and the mechanical Rust pass |
| MDP count checker | pass: 389 explicit task rows, 266 checked, 123 open, 127 gates |
| MDP staleness checker | pass, 21/21 after correcting rename/archive and Windows shell assumptions (ADR-0054) |
| P8 implemented CI gates | pass, TG8-03…TG8-12; TG8-02 is closed by the ten-row evidence map below and the completed independent reviews |
| Windows debug NSIS installer | final-tree build passed; 13,439,723 bytes; SHA-256 recorded below |
| Installer contents | 8 internal entries (7 NSIS support assets plus `arcana.exe`); zero `.py` files |
| `TG8-02` capability port audit | all ten retained coding rows have destinations, readers and behavior proof below |
| `TG8-01` standing-gate clause | owner ruled built/runnable gates only (ADR-0055); pass |
| Independent reviews | completed by GPT-6 Luna reviewers; lifecycle recovery, production stream consumer, DB-backed public prompt composition, runtime lease scope, and reconnect race checked; no remaining P8 blocker |

## TG8-02 — G4b port verdict audit

The population is G4b's four `PORT` and six `PARTIAL` rows. This is the
completed retained-coding audit: every row names a Rust destination, a real
consumer, and behavior evidence. Adobe/design clauses are retired under
ADR-0056; no full row was dropped.

| G4b row | Native destination and production reader | Behavior proof in the final run |
|---|---|---|
| Providers | `provider/types.rs`, Anthropic/Google/OpenAI adapters, `AgentEngine::stream_response` and follow-up request history | Streamed Anthropic thinking/signature is assembled in wire order and serialized back before tool results; missing-signature rejection and cross-provider omission tests; successful Gemini desktop `read_file` turn |
| Execution | `ToolExecutor::execute_batch` → `ExecutionRegistry` → `get_active_tool_executions` → `agentStore` reconnect hydration | A blocked real tool is visible to the registry reader then disappears on completion; cancellation/panic cleanup tests; UI tests reject a late stale snapshot and deduplicate live starts |
| Streaming | Provider adapters emit native reasoning/thinking chunks; `AgentEngine` uses `XmlThinkingDetector` only for non-reasoning models | OpenAI/Gemini/Anthropic native chunk tests; split and incomplete XML delimiter tests; engine stream-end flush test preserves visible tail and keeps private segments out of ordinary history |
| Session | `RuntimeRegistry` admission leases consumed by interactive chat, recipe runner and native subagent; Tauri setup starts the reaper | Duplicate-pair, 500-entry cap, 600-second idle reap and panic-release tests; TG8-12 fails if any real caller or host reaper stops consulting it; live desktop turn exercises interactive admission |
| State | `TasksDb` legal-transition trigger and shared status updates; task tools, Tauri/UI and CLI write through the guarded DB; worker enum includes `idle_no_provider` | Legal/recovery edges, unknown status rejection, raw SQL negative cases, claim/unblock/epic completion, assignment-owner invariant, and direct DB lifecycle tests; TG-P2 spend boundary regression also passes |
| Prompts | One `ContextBuilder`; live non-deleted `WorkersDb` roster in the volatile Delegate fragment; the same DB roster validates assignment and message recipients; shared invariant core | An isolated real DB is injected into the public `build_system_prompt_parts` path; the test confirms custom workers appear only in the volatile prompt and deleted workers do not; composer/tool-honesty and dispatch tests also pass |
| Intelligence | Typed `RetryDecision` is consumed by `AgentEngine`; provider route selection is consumed by provider construction and subagent execution | Retry transient/fatal tests and cross-provider/default/model-routing tests; Gemini desktop smoke traverses selected provider and real tool loop. No duplicate generic action-plan engine is introduced |
| Bridges | `ToolRegistry` definitions feed model requests and `ToolExecutor`; structured `spawn_subagent` uses the permission gate and bounded `SubagentPool` | Gemini desktop turn proves model→built-in tool→tool result; native subagent permission/depth/limit tests cover structured delegation. Prose `DELEGATE` and external MCP discovery have no safe supported caller and are retired under ADR-0059/0060 |
| Events | Typed `InternalEvent` channel publishes tool starts/finishes; `ExecutionRegistry` subscribes to finish; Tauri snapshot and existing frontend event channel feed chat UI | A blocked real `execute_batch` tool is visible through the registry reader used by the Tauri snapshot command, then removed on completion; frontend reconnect ordering/deduplication tests cover hydration. The thin Tauri IPC wrapper is not claimed as a separate live reconnect smoke; existing Rust emitters remain checked by TG8-08 |
| Security | Shared redaction funnel covers tool results and all memory writes; OS vault owns provider-key persistence | Full Rust workspace run includes TG1-06 and S1-04 positive/negative redaction checks; no provider credential was present during suite execution |

The isolated Rust run redirects `USERPROFILE`, `HOME`, `APPDATA`, and
`LOCALAPPDATA` to `C:\Users\razia\AppData\Local\Arcana\verification\p8-suite-home`.
`CARGO_HOME` and `RUSTUP_HOME` remain the installed toolchain locations.
Provider and database credentials are removed from that test process. The
ignored test-profile location is `.arcana/verify/p8-suite-home`; no test log
is claimed unless the test command writes one.

The first TG8-01 green mark was retracted after review found that the P8
preamble literally required future P4–P7 gates, though those phases follow
P8. The owner then ruled that P8 requires all **already-built runnable**
standing gates; future gates remain owned by their phases (ADR-0055). Under
that scope, the current sidecar-absent suite is green. P0/P3 specification
proof gaps remain separate and are not silently marked passed by this run.

## Installer artifact

The final-tree Windows debug installer is
`target/debug/bundle/nsis/Arcana_1.0.0_x64-setup.exe`, 13,439,723 bytes,
SHA-256 `8CFD67F67581E84D8C801E30B5CC797039700933150F14AE6AAE7344CC339F4B`.
`7z l -slt` produced nine path records including the archive itself; the
installer contains eight entries: seven NSIS support assets and `arcana.exe`.
No entry ends in `.py`. This is a debug Windows bundle inspection. It does not
assert a release build or a Linux/macOS installer, which are P9 delivery
obligations.

## Independent review disposition

Three GPT-6 Luna read-only reviews found proof gaps during the closeout. They
were resolved and rechecked: an owned `needs_info` task now clears its owner
when unblocked; Anthropic signed-thinking replay and XML privacy tests exercise
the production stream consumer; the prompt test uses the public composer with
a real isolated worker database; TG8-12 asserts lease scope in each run body;
and the reconnect snapshot merge ignores tool calls that completed during
hydration. No remaining concrete P8 lifecycle, port, or event-store blocker was
reported. P8 does not claim a full Tauri IPC reconnect smoke.
