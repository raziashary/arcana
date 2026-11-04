# Post-ship hardening backlog

Work that was open in the old programme and is **not required for Arcana v1** (Windows-only; see
[`SHIP_PLAN.md`](SHIP_PLAN.md) §2). Nothing here is cancelled; it is ordered after the ship.
IDs and receipts point into [`archive/development/`](archive/development/).

## A. Platform parity (old `P9-16` / `TG9-06`, ADR-LOG-RESTRUCT-0391)

Linux and macOS must stay **fail-closed**: unsupported execution is refused until its gate is green.

- Linux and macOS installers and launch smoke (`P9-06`, `P9-07`), macOS notarisation (`P9-08`), Apple Developer ID (`P9-01`).
- Prove the P4 Fleet boundary on Linux/macOS: real build/test/lint/run intents, dependency staging,
  descendant read/write and network denial, timestamp corpus (`P9-16`).
- **Parked on the local branch `park/linux-macos-sandbox`** (commit `bc54874`, 2026-11-03, **not pushed**):
  `arcana-core/src/platform/process_sandbox/linux_smoke.rs` and `linux_smoke/` (about 4,400 lines, a
  `cfg(all(test, linux))` harness registered by 3 lines in `process_sandbox/mod.rs`), `native/macos-sandbox/`
  (macOS XPC helper, 126 lines of C) and `scripts/build-macos-sandbox.sh`. The branch is additive (9 files, no
  deletions) and unvalidated: it only compiles on Linux, rustfmt reports drift in `staging.rs`, and the macOS helper
  was never built. The Linux/macOS code already on `main` (`linux.rs`, the macOS paths in `mod.rs`) is unchanged.
- Not parked: `e2e/specs/workspace-consent.spec.ts` and the `e2e/wdio.conf.ts` edit are workspace-consent test
  tooling, still uncommitted in the working tree and not tied to Linux/macOS. Decide separately whether to commit them.

## B. Reliability and observability depth (old `P4_EXPANDED_GOAL.md`, `P4-50/51/53/54/56`)

Already built and kept: the Logs tab, telemetry labels and retention, startup retry count, dispatch lineage.
Still open:

- `P4-50` / `TG4-24`: full PA/fleet observability breadth (all four views, graph/MCP coverage, live-provider Fleet).
- `P4-51` / `TG4-25`: a versioned JSON error envelope for every built-in tool, through the production executor.
- `P4-53` / `TG4-27`: whole-app restart handoff and restore.
- `P4-54` / `TG4-28`: worker health with a production writer and reader, bounded temporary-worker creation,
  token usage end to end, two-worker interruption without duplicate effects.
- `P4-56` / `TG4-30`: independent attack testing of the non-command failure paths (retry loops, checkpoints and resume, supervisor reassignment, telemetry retention). The command and sandbox path was moved into `SHIP_PLAN.md` R1 on 2026-11-03.
- `P7-19` / `TG7-06`: automatic replay or reassignment after process loss.
- `P4-49` live-provider paid Fleet completion beyond the Calendar acceptance run.
- Telemetry-capture tests race each other: about twenty `src-tauri` tests install a thread-local subscriber with `tracing::subscriber::with_default` while the suite runs on four threads, and tracing caches each callsite's interest process-wide, so a test whose subscriber is enabled can lose an event another test's subscriber disabled first. Seen once on Linux CI (run 41786764330, `github_mcp_configuration_operations_emit_private_result_spans`: "missing github_mcp_config_read outcome empty"), passing on the runs before and after; none of that commit's changes touch the `arcana` test binary's logging. Fix by serialising these tests behind one lock, or by capturing through a shared global subscriber.
- Desktop log flush on exit: `src-tauri/src/main.rs` holds `_log_guard`, but the Tauri event loop exits the process without dropping it, so the `LoggingGuards` drain (D-010) never runs there and the last events of a session can be lost. Flush from the `RunEvent::Exit` handler.
- npm staging, small edges from the R1 review: `copy_tree` has no size or depth bound, `already_staged` trusts the marker alone, `npm ci` stderr is read unbounded, and `sweep_stale_scratch` removes scratch older than one hour even if a very slow install still uses it.

## C. Codebase audit workstreams (2026-10-18)

[`archive/development/AUDIT_2026-10-18_CODEBASE_REFACTOR_PLAN.md`](archive/development/AUDIT_2026-10-18_CODEBASE_REFACTOR_PLAN.md):
wave 0 and about 1.5 of 9 workstreams are done (W8 hook consent, part of W4/W6). Not started: W1 guards for
hand-kept tables, W2 dead-code deletion, W3 duplicate collapse, W5 test-harness integrity, W7 frontend
cleanup; W9 one vetted command surface (mechanism chosen in ADR-0401; moved into `SHIP_PLAN.md` R1 on 2026-11-03). Take items from here
when they block a ship task, not as a programme.

Approved but not done: generate the telemetry label set instead of hand-maintaining it; port the graph store
onto the shared migration ladder with a backup guard (this overlaps `P5-03`); decide the fate of `guard_and_validate`.

## D. Earlier-phase proof debt

The archived register shows open gates in P0 (4 of 5), P2 (10 of 17), P3 (34 of 41) even though those phases
are recorded closed ([`archive/development/TESTING_GATES.md`](archive/development/TESTING_GATES.md)).
Re-derive which still matter from the code before opening any. Notable: `P0-21` e2e harness scaffold and `S0-01`.

## E. Items deferred from the v1 roadmap

Cuts Razi confirmed on 2026-11-03 (rows without `v1` in `SHIP_PLAN.md`): incremental graph updates (`P5-08..13`), dark mode,
UI drift gate, reconnect replay, command type checking, Graph City performance bench, frontend reachability gate,
interaction-grammar gate, production updater key and signed updates (`P9-03/04/11..14`).

## F. Withdrawn, not deferred

The AutoDesign programme (`PD0`–`PD6`, design-only parts of `P4-G` and `P6`) was withdrawn on 2026-09-23.
History: [`archive/development/HISTORICAL_DESIGN_PROGRAMME.md`](archive/development/HISTORICAL_DESIGN_PROGRAMME.md).
