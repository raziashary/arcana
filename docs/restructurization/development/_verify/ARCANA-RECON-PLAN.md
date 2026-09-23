# Arcana reconnaissance: restructuring plan audit

> Pre-scope-change audit snapshot. The active MDP, gate index and retired-design
> archive were rewritten after this report; its original findings remain
> evidence for that change, not current task status.

Read-only source audit, not phase closure. No tests, network, credential reads, or source changes. Investigation began before the parent discovered MDP §0.2i; report created immediately on amended dispatch.

Initial finding: MDP header states next P8, whereas Readiness Matrix update log records P8 closed 2026-09-04. Current source verification remains pending.

## Confirmed findings so far

- P8 is recorded closed in the newest commit (`bc6db17`, followed by public refresh `4ea9f07`) and READINESS_MATRIX update log (line 202), but MDP header (line 12) and ordering (line 879) still call it next. Readiness header is 34.0% while log and A8 row imply 36.7%; use qualified, dated records instead of repeating header values.
- PD means design-capability phases, not generic software design stages: PD0 published contract, PD1 ingestion, PD2 fidelity, PD3 export, PD4 deterministic adaptation, PD5 changes, PD6 brand guideline (MDP §5.5).
- Existing design code is real but largely isolated: `src/design/contract/main.ts` invokes `runInvocation`, which streams `runOperation`. `reference.ts` implements reference-shaped partial outcomes; only empty change completes. The PSD converter has a browser canvas AssetStore and currently has test callers, not app integration callers found by source search. package.json pretest builds the sidecar, npm tests include it, and CI references its spawning gate.
- Deleting all names containing sidecar is expressly unsafe: P8 protects PD0's published partner seam plus updater signature/hash assets and SQLite WAL backup. See TESTING_GATES TG8-11 and ADR-0036/0038.

## Source-verified baseline (2026-09-23)

Checkbox recount (including struck task IDs): P0 34/35; P1 22/22; P2 75/75; P3 61/61 (43 A-E + 18 F); P8 67/67; PD0 10/10; PD1 4/25; P4 1/49; P5 0/21; P6 0/28; P7 0/18; P9 0/16; PD2 0/27; PD3 0/15; PD4 0/33; PD5 0/13; PD6 0/10. Total 525, checked 274, open 251. This is document state, not fresh runtime validation.

P0's outstanding e2e harness task remains unchecked. P3's matrix Gates row remains unchecked (memory gates partial/unmet); P8's Smoke row remains unchecked because a desktop launch plus real provider-backed turn was not run. Thus recorded phase closure is not all-green product readiness. Sources: MDP §5.1, §5.5, S8-02; READINESS_MATRIX §2, §2.1, update 2026-09-04.

Executable design surfaces:
- package.json lines 9-18: build:sidecar, pretest, test, smoke:design. Design contract is part of regular JS test preparation.
- src/design/contract/main.ts:117 -> sidecar.ts:69-109 -> reference.ts:233-341. This is a spawnable NDJSON protocol reference, no socket.
- reference.ts:55-67 generates reference locators and synthetic digests; :79 names REFERENCE_IMPLEMENTATION_ONLY; :287-335 supplies partial placeholder export/design outcomes. Only empty change instructions complete, with the same design reference.
- src/design/psdToIr.ts:377 defines conversion from an already parsed Psd; :42-46 AssetStore holds browser HTMLCanvasElement objects. Current source search finds its call sites in psdToIr.test.ts, not app production modules.
- src-tauri/src/ci_gate.rs:5507 and :5620-5635 explicitly protect the PD0 seam, build config, and smoke runner. Removing them is a deliberate contract/gate revision, not a cleanup.

## Findings requiring honesty in any follow-on plan

1. MDP header line 12 and §5.1 still call P8 NEXT, contradicting P8 checked tasks, ADR-0041 (line 1599), matrix 2026-09-04 update, and HEAD commits bc6db17/4ea9f07.
2. READINESS_MATRIX header line 9 and executive line 46 say 34.0%; scored-area total line 71 and latest update say 36.7%. SO-1 line 131 still says main.rs spawns the deleted Python subsystem. Executive lines 49-50 also describe the old Windows rewrite and unbuilt P3-F. Do not cite these as current facts.
3. MDP S8-02 line 2552 calls smoke:design a proven design conversion. Source establishes only the PD0 reference protocol exercise (synthetic references and partial outcomes). This is an overstatement, independent of the live-launch gap it correctly records.
4. ADR_LOG header line 9 claims 19 entries through 0019; ADR-0041 exists. Vision master header lines 8-9 still says initial draft/no GO even though MDP records GO and later work. Historical amendment rows must remain historical; current headers need correction.
5. main.rs:216-218 and :352-357 still describe removed SidecarManager/startup FSM behavior. The narrow forbidden token set in TG8-09 does not catch every stale architecture comment.
6. PD phases have no matrix scored area (matrix update 2026-09-03). Removing them should not manufacture a readiness increase by silently changing weights or declaring their requirements passed.

## Safest coding-only scope amendment proposal (not enacted)

Record an owner-approved scope ADR that supersedes the two-rollout/design-first mandate (vision directions 14-20, 23; ADR-0023/0024/0033 design decisions; design-specific parts of ADR-0037). Mark PD1-PD6 remaining work withdrawn from active product scope, retaining their IDs, text, evidence, and already-completed work. Retain PD0 as a shipped historical contract pending an explicit retirement/compatibility decision; do not rename its version strings, opaque URI schemes, or delete its protected implementation under a cosmetic rename.

Update active planning sections together: MDP §5 sequencing/milestones/dependencies/counts, PD sections' scope banners, P4-G verification/design recipes (P4-45..48), P6-25..28 viewer/conversion/fidelity/adaptation UI, S8-02 design clause, TESTING_GATES applicability, READINESS_MATRIX truthful baseline, vision current mandate, and public-facing current scope. Keep existing historical logs and evidence intact. Reassess P4 dependency (currently PD6) and coding sequence P4 -> P5 -> P6 -> P7 -> P9 after scope is accepted; do not assume dropping design means dropping memory, native agents, security, build proof, or cross-platform packaging. memory-anchor §1 explicitly targets coding agents and remains directly relevant.

Branding should be a separate bounded work unit: user-facing AutoDev -> Arcana is different from crate/package identifiers, `.autodev` data roots, vault identities, update identity, and published autodev-* contract values. Shipped contracts require the §0.0 ADR/migration/live-proof discipline if changed.

Governing constraints: MDP §0.0 shipped source anchor; §0.2 additive/destruction ADR; §0.2m adversarial pre-build refutation; §0.2a-c post-phase audits and overseer disposition; §0.3 no stale docs; §0.5 smoke plus gates; §0.8 backup before live state/schema changes; §0.11 append-only decision/retraction record; §0.13 vision/source conflict surfaced; §0.15 scope/contract escalation; §0.17 public-facing refresh. No new phase was executed or closed in this audit.
