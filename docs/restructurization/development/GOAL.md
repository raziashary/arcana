# Arcana goal (2026-09-23)

Build a personal desktop multi-agent coding platform, using the in-process
Rust agent runtime for interactive chat, worker recipes, and subagents.
The current owner direction and active sequence are in
[ARCANA_CODING_SCOPE.md](ARCANA_CODING_SCOPE.md); the historical implementation
and proof rules remain in the MDP §0.

| Outcome | Current baseline | Completion proof |
|---|---|---|
| One coding runtime | Shared `arcana-core` is linked by desktop and CLI; Python sidecars removed | Chat and worker finish a real coding turn through native engine, with tool result read by UI/task board |
| Reliable fleet | P4 is largely open | Real task is assigned, claimed, built, QC-reviewed, and recovered after interruption; original P4 gates pass |
| Useful context | Memory and graph code exist; P3 memory gates remain incomplete | Persisted fact/code data is retrieved and visibly used by a later relevant turn |
| Honest UI | React tabs and event stores exist; P6/P7 open | Desktop launch, normal workflow, event producer/consumer and setting readback checks |
| Arcana identity | Rename applied in source; validation is in progress | Fresh Arcana data/key location, packaging checks |
| Installable product | P9 is later work | Windows, Linux, macOS build and launch smoke with applicable security checks |

Current checkpoint: P8 is closed. The live Gemini tool-backed desktop coding
turn, sidecar-absent Windows suite, ten retained coding-port proofs, final
debug installer inspection, and independent Luna reviews are recorded in the
P8 closeout. P0/P3 gate-proof gaps remain visible in their own registers.
Implementation now proceeds through P4, P5, P6, then P7; P9 and additional
features follow later.
Remaining design capability phases are withdrawn from the target. The owner
approved PD0 retirement and fresh Arcana state in ADR-0043.
