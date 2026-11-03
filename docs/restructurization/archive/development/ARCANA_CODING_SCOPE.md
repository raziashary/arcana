# Arcana coding-only scope (current direction, 2026-09-23)

Razi's current direction supersedes the design-first rollout in the historical
master development plan: this is a personal multi-agent **coding** platform,
named Arcana. The Rust-native agent and fleet restructuring continues. The
AutoDesign capability programme (`PD0`–`PD6`) is no longer a product goal.
The existing phase IDs and completed evidence remain as history; they are not
renumbered or counted as newly completed work.

Arcana's product distinction is its harness, not a model brand or chat shell.
Arcana owns deterministic project/tool discovery, command construction,
permissions, lifecycle, evidence, and recovery wherever behavior can be
specified safely. Models provide planning, code generation, review judgment,
and other work that needs intelligence. The built-in team workflow must be
usable without users first authoring their own agent files, subagents, skills,
or tool wiring; recipes let users tune those defaults. A registered tool or
written instruction is not delivered behavior until a real production consumer
uses it and behavior tests prove the contract.

## Verified baseline

- The desktop is Tauri v2, React/TypeScript, and a shared Rust `AgentEngine`
  (`src-tauri`, `src`, `arcana-core`). `arcanactl` shares the core library but its
  old HTTP `agent` subcommands were retired in P8.
- P8's Python orchestrator/memory sidecars are removed. The separate PD0 Node
  reference process was a protocol stub, not real design conversion. ADR-0043
  authorizes its retirement from the current source and npm test path.
- The legacy master plan records P0–P3, PD0, and P8 closed; it still records
  open proof gaps for P0, P3, and P8. P4–P7/P9 have substantial work open.
  See `_verify/ARCANA-RECON-PLAN.md` and `READINESS_MATRIX.md` for the evidence
  and the matrix's inconsistent historical headline.
- The desktop obtains provider keys through its vault, not `.env`.
  `GOOGLE_API_KEY` in the ignored `.env` is only for explicitly enabled live
  provider tests; `GEMINI_API_KEY` is not read by those tests.

## Active outcomes and sequence

| ID | Outcome | Primary existing phase | Proof needed |
|---|---|---|---|
| AC-01 | One native coding agent path for chat, worker recipes, and subagents | P3/P4 | A real coding turn plus tool result visible to its caller, permission and failure checks |
| AC-02 | Arcana acts as the harness around the model: deterministic project discovery and commands, permission checks, lifecycle, evidence, and recovery, with model intelligence retained for code and ambiguous judgments | P2/P3/P4 | Trace each contract to a production consumer; behavior-test correct selection, refusal, failure, recovery, and evidence; prove no prompt-only safety or unused tool path |
| AC-03 | A built-in development-team workflow works without user-authored agent scaffolding, while recipes allow customization | P3/P4/P6/P7 | Fresh-project coding task through default recipes, routing, claim, deterministic verification, review, artifact handoff and recovery; verify UI reads status/results and customization does not erase defaults |
| AC-04 | Code graph and memory improve coding context with observable reads | P3-F/P5 | Retrieval from data written by the product and consumed by a later turn |
| AC-05 | Desktop UI exposes truthful fleet, settings, status, and coding tools | P6/P7 | Tauri/React smoke, producer-to-consumer event checks, persistent setting readback |
| AC-06 | A personal Arcana identity with fresh local state | Current preparation | New data/vault roots, installation identity, and readback checks |
| AC-07 | Installable Windows, Linux, and macOS application | P9 (later) | Platform-specific build and launch smoke; signed update proof where enabled |

P8 is recorded closed; P4 is the active fleet phase. A cross-phase audit of
P1/P2/P3/P4/P8 is now required to verify that native extraction, provider
contracts, memory paths, retained coding behavior, and current fleet work serve
the harness and built-in workflow outcomes above. Fix evidence-backed gaps in
the active P4 path and record later-phase gaps for their owning phases. P5,
P6, P7, P9 delivery, and additional features remain later work. The remaining
design tasks (`PD1`–`PD6`) are withdrawn from
active scheduling. Razi chose to retire PD0 as well (ADR-0043). Design-only
parts of P4-G (`P4-45`–`P4-48`) and P6's design UI tasks are withdrawn from
the coding worklist; preserve their historical rows. The coding
`BuildEvidence` gate remains required.

## Rename boundaries

The source rename now uses `arcana-core`, `arcanactl`, `~/.arcana`, the Arcana
keyring service and `.arcana` association. The prior AutoDev data and keyring
entries remain untouched; no import is planned. Packaging and update identity
need final checks before P9. The former release feed is disabled unless an
explicit `ARCANA_UPDATE_URL` is provided (ADR-0044).

## Next implementation units

1. Verify the PD0 retirement and Arcana fresh-state rename under ADR-0043.
2. Reconcile P8 proof gaps, active P4–P7 gates, matrix, and public brief while
   keeping dated historical evidence readable.
3. Run the existing checks and smoke tests; record remaining gaps as the entry
   point for P4–P7 implementation under the MDP's §0 proof/audit rules.

The governing MDP §0 rules remain in force. This scope document changes the
target, not the safety, proof, audit, or commit rules.
