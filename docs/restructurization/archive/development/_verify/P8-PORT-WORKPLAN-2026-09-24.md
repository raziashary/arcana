# P8 retained-port implementation plan — pre-build draft

The owner ruled that Arcana retires Adobe/design subclauses but implements and
proves the coding behavior in all ten G4b PORT/PARTIAL rows (ADR-0056),
preferring integrated native ports and accepting demonstrated equivalents
where literal recreation conflicts with Arcana's architecture (ADR-0060).
This draft was challenged under MDP §0.2m before new port code; see
`P8-PORT-PREBUILD-AUDIT-2026-09-24.md`. Existing behavior is mapped in
`P8-ARCANA-CLOSEOUT-2026-09-24.md`; G4b §A is the historical behavior source.

## Scope and sequence

1. **Provider and streaming (G4b A.1/A.6).** Preserve signed Anthropic
   thinking blocks and their original wire order across a tool follow-up;
   check the next outbound body, not merely parsed chunks. Carry native
   reasoning/native thinking boundaries and a bounded XML fallback through
   the stream-to-UI path. Keep per-request keys, current model catalogue and
   loop behavior. The historic 11-provider catalogue is not itself a mandate
   to add providers: G4b flags its IDs as stale and current Arcana ships three.
2. **Execution and session (A.2/A.4).** Add a queryable in-flight tool registry
   to the actual `execute_batch` path, with cleanup on completion, error,
   cancellation and panic; give a real coding UI or runtime consumer a query
   path that reads active state. Integrate ephemeral `(agent_id, session_id)`
   isolation/admission leases with the interactive, recipe and subagent entry
   paths, 600-second idle cleanup and 500-entry cap without retaining old
   engines or deleting persisted sessions (ADR-0062). Prove with blocked tools
   and a controllable clock; verify production callers use the structures.
3. **Lifecycle (A.5).** Derive legal tool/session/agent/task transitions and
   recovery exceptions from all current writers, including direct SQL status
   writes. Enforce task status at the shared DB boundary; represent the real
   `idle_no_provider` worker status without taking P4-02's separate strict
   worker-status parser task. The Python `SystemStartupFSM` is not
   reintroduced: P8-52/OQ-3 already removed the sidecar startup overlay and
   its phases. Verify the native app's existing startup/shutdown behavior
   separately. Owner moved task enforcement into P8 (ADR-0057).
4. **Prompts and decisions (A.7/A.9).** Recover the non-Adobe YAML prompt
   assets from history or derive equivalent coding fragments with explicit
   tag selection in the existing stable composer; ensure the current worker
   roster is read at prompt construction for delegation **and from the same
   source by dispatch validation**, and tool-honesty text reaches a real
   agent. Add a typed decision result for coding route, retry and escalation
   only with an actual supervisor/runner reader; avoid a duplicate decision
   path. Keep Adobe
   failure regexes retired. Do not add a second conflicting prompt composer.
5. **Bridges and events (A.11/A.12).** Prove the current in-process
   `ToolRegistry` covers the old MCP round trip for built-in tools, including
   an actual model request and executed tool result. The historical MCPBridge
   connected those same Rust built-ins through Python; current configured
   external MCP servers have health/config only and no agent caller, so a new
   external `tools/list`/`tools/call` client is not this port (ADR-0059).
   Integrate `DELEGATE[@id]:` parsing with current roster, depth ≤3,
   call-graph cycle checks and the **same permission decision as structured
   `spawn_subagent`**; never execute unauthorized model prose. Add typed,
   correlated internal event subscription while preserving Tauri event
   names/payloads; wire a real Rust reader and verify the frontend consumer
   still receives them. Keep Adobe compiler, Redis/WebSocket stubs and server
   schema breadth retired.
6. **Security (A.14).** Keep the existing tool-result and memory-write
   redaction proof as this row's destination; test it again with the complete
   port suite. OS-vault replacement remains the owner-approved Arcana design.

## Acceptance and phase closeout

- Each row receives a named Rust destination, a test that fails when its
  behavior is removed, and a consumer-read check. `TG8-02` is checked only
  after all ten row verdicts are backed by those proofs. A source-symbol match
  alone is not evidence.
- Run the full isolated Rust all-targets suite without live credentials and
  with no sidecar trees, plus TS typecheck/lint/unit and production build;
  `cargo fmt --all -- --check` and Clippy must pass. Repeat the live Gemini
  desktop coding turn if provider/agent-path changes can affect it.
- Run the P8 smokes, two required post-phase auditors, no-stale and count
  checkers, and update MDP, gate register, readiness matrix and public-facing
  docs. Commit validated logical units and push the authorized `main` branch.

## Claims the pre-build audit must try to refute

- That every item above is truly a retained **coding** behavior rather than a
  server/design implementation detail; check the cited G4b verdict and the
  current Arcana consumer.
- That adding a registry/bus/decision type improves the real path rather than
  writing state no reader consumes.
- That all task/worker writers can obey one transition policy without
  breaking recovery, migration or existing sessions.
- That a direct in-process tool dispatch satisfies MCPBridge semantics, or
  that an external MCP bridge is required by an actual coding caller.
- That 12 non-Adobe YAML templates can be recovered or mapped without
  duplicating the existing P8 prompt floor or leaking stale roster entries.
- That a three-mode streaming detector and Anthropic signed thinking can be
  carried across the current shared provider types without losing provider
  compatibility or showing private thinking as ordinary chat text.
