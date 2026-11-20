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
- Telemetry-capture tests race (`src-tauri`, fixed 2026-11-06 with `test_support::capture_default`; `arcana-core` tests that use `with_default` have the same exposure and have not failed). With one dispatcher registered, `tracing-core` computes a callsite's interest from the registering thread's own default, so a thread with no subscriber can cache "never" for a shared callsite (`operation_timing`) and the capturing test reads nothing. It failed Linux CI twice (runs 41786764330 and 41839567401, the second on a docs-only commit) and not locally on Windows in 3 runs with 8 threads. The fix installs a process-wide `Registry` default so interest is the union. Not reproduced; the CI runs after this are the check.
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

## G. Graph leftovers from R2 (2026-11-12, see D-013 and D-014)

- **Deleted (D-017, 2026-11-13):** `graph_tools.rs` (27 unregistered structs) and `graph_prompts.rs` (five prompts that told the model to call them by name), superseded by `graph_query_tools.rs`. The four ideas that did not ship are the next item.
- **Four tool ideas parked until their producers are fixed:** `detect_graph_changes` (runs `git` in the process directory, not the
  workspace), `get_graph_diff` (nothing writes snapshots), `get_graph_report` (reads a placeholder file), `get_flow_criticality`
  (call chains labelled as flows). `query.rs`, `flows.rs`, `changes.rs` and `analyze.rs` now compare the right edge-kind strings
  but still load the whole graph per call; rewrite on the per-node edge lookup before any tool depends on them again.
- **Cross-file calls are not extracted.** The extractor emits `CALLS` only between functions of one file (a regex scan; the
  other languages rely on the same approach and have not been checked for the blank-line defect fixed for Python and JS imports
  in D-014). Cross-file dependence is file-level `IMPORTS_FROM` only, so `graph_callers` and `graph_impact` on a function stop at
  the file edge. Fixing this means name resolution across imports, a real extractor change.
- **Extractor fidelity:** the Python extractor sets `line_end` equal to `line_start` (the regex finds only the first line), so
  `graph_file` and the node details show one-line spans for Python; the other languages have not been checked.
- **Name resolution cost:** `graph_*` tools resolve a simple name through the 200 best search hits; a very common name in a huge
  graph can miss an exact match that ranks lower. Add an exact-name lookup on `nodes(name)` if it shows up.
- **Test flake:** `graph::build::observability_tests::production_graph_build_emits_correlated_operation_outcomes` fails in about
  one parallel module run in three and passes alone. It is the thread-local `with_default` callsite-interest race already noted
  in section B for `src-tauri`; `arcana-core` needs the same process-wide default (`capture_default`).
- **No `TESTED_BY` producer.** `analyze.rs`, `changes.rs` and `cluster.rs` read `TESTED_BY` edges that no extractor writes, so impact
  analysis never lists tests. Add a producer (test-file naming or call from a `Test` node) before surfacing test links again.
- **Per-call connection churn in the tools:** every `GraphStore` method opens a connection, so `graph_impact` (up to ~200 nodes) and
  `graph_path` (up to 5000 visited) open one per visited node, and `get_edges_for_node` has no `LIMIT`. Not measured. Give the
  store a held read connection for traversals if it shows up.
- **Pre-ladder graphs get no backup:** `GraphStore::open` backs up only a database with `user_version` 1..head, so a database at
  version 0 that already holds data runs the whole ladder, including the v7 duplicate-edge delete, without one. Whether such
  files exist is unverified. Builder path, not a reader; found in review.
- **Graph block bidi/zero-width characters** are not stripped from repository names (structure is safe; display could mislead).
- **Dead duplicate hydration:** `src-tauri/src/commands/settings_commands.rs::hydrate_active_provider` has no caller (the live copy is `arcana-core/src/runtime.rs::hydrate_state`, which also restores the system prompt). Delete it so the two cannot drift; found while checking `P6-15..18`.
- **Typed turn outcome, the rest (D-025):** `agent:error` now carries `kind` (`failed` or `cancelled`), so a Stop no longer depends on matching prose. The budget, auth, rate-limit and provider-down classes that `explainTurnError` still matches by text could become kinds the same way. The dead `agentLiveStore` listener on `agent:error` (it read `agent_id` and `error_message`, which the event does not carry) was removed on 2026-11-18; the Live Monitor still shows no turn errors from the chat path, because the event has no agent id to attach them to. Feed it from `session_id` if the Monitor should show chat turns.
- **Undo for destructive actions (D-017):** confirmation is the only protection in v1. Wire the checkpoint commands as undo where `git/checkpoint.rs` can reverse the action, with a proof per action. The three two-click buttons (clear tasks, clear logs, delete worker) confirm but do not say what is lost; move them to `confirmDestructive`.
- **`P5-08..13`** (true incremental update) stays deferred; `update_graph` is a full re-index (D-013).
- **Backend left behind by D-021:** the plan system (`src-shared`/`arcana-core` `db/plans`, plan events) and the plugin system (`arcana-core/src/plugin`) have no UI and no Tauri
  command now. Either delete them or give them a UI after v1; until then they are compiled, unreachable from the app, and carry their tests.
- **Update channel is not persisted:** the Settings channel selector (stable, beta, nightly) is component state passed to each manual `check_for_update`; the automatic check does not use it and it resets on restart. Persist it, or drop the selector, when the updater is worked on (R4 ships stable only).
- **Where a key actually landed (D-024):** `vault_backend_info` and `list_vault_entries` report the backend by a read probe, while `VaultManager::store` can fall back to the encrypted file when only the write fails. Make `store` return the backend it used and show that.
- **App-wide dashboard refresh (D-024):** `dashboardStore` refreshes (five invokes) on every `agent:done`, `task:changed` worker tick and `worker:tick-end` for the whole session, where it used to do so only while Dashboard was open. Measure it with a busy fleet; debounce if it shows.
- **Contrast over `bg-code` tints (D-024):** accent, danger and success text fall to 4.1 to 4.4:1 if a hover or selection tint is ever drawn over the lavender `bg-code`; no such row exists today.
- **Unused WiX fragment (D-026):** `installers/wix/fragments/ArcanaExtensions.wxs` is referenced by nothing; the installer is NSIS. Delete it, and rename `installers/wix/` to `installers/windows/` when the scripts are next touched.
- **Real-app run findings (2026-11-18, left open):** the Windows keychain is per user, not per `USERPROFILE`, so a run with a scratch profile still sees the real Google key (first run showed "A Google key is stored"); isolate by test credentials, not by profile. The workspace path is shown with the `\\?\` verbatim prefix in the Graph toolbar and the community list shows full paths in older graphs; show a normalised path. A build reports 446 nodes and 532 edges extracted but the store holds 417 and 248 (duplicate identities collapsing, D-013); the build result should say what was stored.
- **Toolchain staging progress (P7), half done 2026-11-20:** staging now logs `toolchain_staging_started` before the long step. Still missing: periodic progress while it runs, and a surface for it (the Logs tab was not re-read for the event; the task card shows nothing).
- **Review leftovers (2026-11-20, D-028):** the Graph City declutter ranks districts only by area (same-named districts now carry their community id, D-029; the Update settings notice for an inert updater is also D-029, read in code and tests but not yet re-read in the rebuilt app); the in-app Docs reference text keeps about 64 technical uses of "agent"; the worker id `qc-agent` is a stored name that shows on screen; `consent_database_operations_emit_private_result_aware_timings` failed once in a parallel run (passes alone), another telemetry-capture flake like the one in G; the CI Linux failure of `production_multi_project_spawn_failure_cannot_be_hidden_by_passing_sibling` is open until the diagnostic from `07e23d6` runs on CI.
- **City district labels collide at overview zoom (2026-11-18):** with 70 districts over a 417-node graph the DOM labels overlap and read as noise until zoomed in. Hide or thin labels by district size and distance. Names themselves are now distinct (`graph::cluster` fix); a graph built before it keeps its old names until rebuilt, and the City shows the folder of any name that carries a path.
