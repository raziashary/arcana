# Cross-phase audit refresh — 2026-10-03

## Scope and method

A GPT-6 Luna High read-only review checked current source and the active
readiness record for P1, P2, P3, and P8 while expanded P4 remains in progress.
This is a source audit, not new runtime evidence. No production database,
provider, or owner Calendar workspace was used.

## Findings and disposition

| Area | Current-source finding | Disposition |
|---|---|---|
| P1 graph tools | Graph module exists, but `register_for_mode` does not register graph tools. | Open and owned by P5's real AgentEngine registration/consumer gates. Do not count symbol presence as P1 proof. |
| P1 runtime and ledger | Historical AutoDev smokes do not prove the current Arcana-root runtime. Worker runs now have durable IDs, but provider cost and all mutations are not comprehensively attributed. | Preserve historical closeout; current Arcana runtime evidence and complete action provenance remain partial. |
| P2 provider contracts | Provider catalogue is the source for Settings coverage. Persisted Gemini opaque history is not represented in session `Message`; live request replay therefore lacks reopen/readback proof. | Keep provider restart/wire and adapter replay gates open. |
| P2 local-day analytics | Readers use the local date-boundary helper. | Implemented path is fixed; UTC−12/0/+14, DST, and rollup cadence matrix remains open. |
| P2 spend ceiling | `CappedProvider` reserves before dispatch and AgentEngine records result-aware failures. | Prior check-then-record/fail-open audit claims are superseded for those paths. Keep TG2-17 open for stream completion/error/drop, restart, estimate-bound, and kill-switch/broadcast evidence. |
| P2 vault and historical smoke | Linux no-D-Bus fallback has not been exercised; old AutoDev smoke is not current Arcana proof. | Keep those environment gates open. |
| P3 Memory tool degradation | Initial audit found `search_memory` used `search_all` and rendered empty fallback as ordinary empty output. | Focused fix adds a detailed, backwards-compatible recall result and exposes redacted degraded state for empty and nonempty tool output. Degraded operation timing is classified as `fallback`. Recall tests pass 10/10; helper/renderer and real `SearchMemoryTool::execute()` tests pass 3/3, with the latter reading persisted data through an isolated migrated MemoryDb. TG3-17 remains partial because the production active-backend absent/downloading transition and broader clauses are not proven by the injected-backend fixture. |
| P3 consent | Shared consent reaches both Tauri diff paths. | Keep live desktop IPC, revocation-between-requests, and complete consumer readback open. |
| P8 host commit safety | Current `git_commit` checks registration, task claim, staged content, and shared-index transaction behavior; TG8-13 and S8-06 have current passing tests. | Source/host behavior is reclosed under ADR-0075. Full model-produced AgentEngine → scheduler commit proof is still absent; Bash text heuristics and cross-process Git writers remain documented limits. |

## Readiness rule

This audit does not change any task to complete. The focused Memory fix has
production-tool response coverage for persisted results and explicit degraded
state, plus empty and nonempty injected-fallback cases; it is narrower than
TG3-17's active-backend absent/downloading proof requirement. Keep P1/P2/P3
proof debt with its owning gates; P4 closeout must not silently claim those
future-phase gaps as complete. P8's current host contract remains reclosed while
the missing consumer proof stays visible.
