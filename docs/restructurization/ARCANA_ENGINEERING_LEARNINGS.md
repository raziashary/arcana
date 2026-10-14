# Arcana engineering learnings

This is a compact record of lessons from the coding-only restructuring. It is
not a task checklist or a replacement for the authoritative
[`development/MASTER_DEVELOPMENT_PLAN.md`](development/MASTER_DEVELOPMENT_PLAN.md),
[`development/TESTING_GATES.md`](development/TESTING_GATES.md),
[`development/READINESS_MATRIX.md`](development/READINESS_MATRIX.md), or
[`development/ADR_LOG.md`](development/ADR_LOG.md). Status and measurements in
those documents and the linked `_verify` receipts take precedence.

## What the product is

Arcana is a native coding harness: the model plans and writes code; Arcana
owns deterministic project discovery, command selection, task claims,
permissions, execution boundaries, evidence, and recovery. The desktop is
React/TypeScript in `src/` over Tauri commands in `src-tauri/`; `arcana-core/`
owns AgentEngine, worker recipes, provider adapters, SQLite stores and tools.
`src-cli/` shares that core. The current module/data-flow map is
[`development/PROJECT_MAP.md`](development/PROJECT_MAP.md).

## Lessons borne out by P4 work

1. **A written rule needs a production consumer.** Persisting a build row did
   not prove QC safety until the shared task transition and direct-SQL paths
   refused missing, stale, unattributable and incomplete batches. P4-14/TG4-07
   closeout records the reader and negative tests in
   [`development/_verify/P4-14-QC-NAMED-CLOSEOUT-2026-10-13.md`](development/_verify/P4-14-QC-NAMED-CLOSEOUT-2026-10-13.md).
2. **An observed exit is different from a launch failure.** Rust-owned
   `project_command` can persist an actual nonzero build exit, while timeout,
   preparation and spawn errors leave a batch incomplete. One isolated
   P4-15 test launch lacked a Rustup toolchain after redirecting its profile;
   that produced `command_spawn_failed`, not a third compiler failure. With
   the host toolchain selected, the mixed-project threshold test passed twice;
   see [`development/_verify/P4-15-DIAGNOSIS-NAMED-CLOSEOUT-2026-10-13.md`](development/_verify/P4-15-DIAGNOSIS-NAMED-CLOSEOUT-2026-10-13.md).
3. **Untrusted project execution is an OS boundary.** Builds, tests, scripts
   and descendants may read or mutate outside a selected workspace unless
   the host confines them. Host approval for an install is separate from
   giving build/test children unrestricted network access. P4 acceptance is
   Windows-first; Linux/macOS parity and packaging are a fail-closed P9-16/
   TG9-06 release blocker under ADR-LOG-RESTRUCT-0391.
4. **Uncertain side effects require durable restraint.** A lost acknowledgement
   does not prove a file write did not happen. The run journal, workspace
   lease, task claim and Git/checkpoint snapshot must agree before any
   continuation. P4 requires quarantine and an actionable manual Supervisor
   handoff; automatic process-loss replay/reassignment is P7-19/TG7-06.
5. **Observability is only useful when read back.** A producer writing JSONL,
   SQLite timing, usage or MCP events is partial evidence until the Logs UI
   and CLI export can correlate and display it with safe labels, time spans,
   bounded payloads and privacy negatives. Full built-in failure,
   timestamp-pair, graph/MCP and four-view breadth remains in P4.
6. **Scope and proof must be explicit.** A passing unit test cannot close a
   desktop Fleet or Calendar smoke. Dated snapshots can conflict with current
   source; the phase checklist and gate receipt identify what is proven and
   what is merely implemented. The owner performs the final P4-49 Calendar
   acceptance after engineering readiness.
7. **Desktop handoff needs the real producer and reader.** The P4-21 receipt
   creates an artifact with the production file tool, follows Tauri's
   `ide:navigate` event into the IDE, clicks Run and reads back the sandboxed
   Cargo result. A `.cmd` package manager is a separate Windows runtime case:
   fixed `cmd.exe` argv alone did not grant the selected Node installation.
   The focused npm run now passes with sibling-read denial, while the full
   Windows command corpus remains open. A reported linker-temp `LNK1104` did
   not recur in a complete desktop receipt or three repeated MSVC sandbox
   builds, so its cause remains unconfirmed. See
   [`development/_verify/P4-21-IDE-RUN-2026-10-14.md`](development/_verify/P4-21-IDE-RUN-2026-10-14.md)
   and [`development/_verify/P4-11-NODE-SHIM-2026-10-14.md`](development/_verify/P4-11-NODE-SHIM-2026-10-14.md).

## Current route

Complete the remaining Windows P4 engineering and TG4 evidence, stage and
push coherent verified units, then let the owner run the Calendar scenario.
After core P4, follow `development/P4_EXPANDED_GOAL.md`, then P5 graph, P6 UI,
P7 experience and automatic recovery, and P9 delivery/platform parity. Keep
unsupported execution fail-closed until its gate is green.
