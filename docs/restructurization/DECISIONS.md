# Decisions

New decision log, started 2026-11-03. Add an entry only when there was a real choice between alternatives.
Keep each one short: what was decided, by whom, why, and what it changes.

The 412 earlier entries (`ADR-LOG-RESTRUCT-0001`..`0412`, `ADR-0043`, `ADR-0044`, …) are in
[`archive/development/ADR_LOG.md`](archive/development/ADR_LOG.md). They remain valid history and code
comments may cite them; this file does not renumber or supersede them except where stated below.

## D-001 · Ship Arcana v1 as a Windows-only installer — 2026-11-03, Razi

Linux and macOS parity moves to the post-ship backlog. Unsupported execution stays fail-closed.
**Why:** the programme had spent weeks on sandbox parity and proof while P5, P6, P7 and P9 (the parts
a user sees) had not started. **Supersedes:** the cross-platform clause of the old `P9` goal and `P9-16`
as a v1 requirement; `ADR-LOG-RESTRUCT-0391` still holds for what it defers.

## D-002 · Close P4 on the middle path — 2026-11-03, Razi

Windows sandbox containment (`P4-18`, `P4-61`) is finished before P4 closes; observability depth,
continuity and worker-health depth move to the backlog. **Why:** containment is a safety boundary;
the rest is hardening. **Changes:** `P4_EXPANDED_GOAL.md` is no longer a ship requirement.

## D-003 · Slim the process — 2026-11-03, Razi

The master plan's `§0` rules are replaced by the nine rules in [`SHIP_PLAN.md`](SHIP_PLAN.md) §3.
Suspended: two audit subagents after every phase, adversarial verification before every phase,
per-phase learnings and public-brief refreshes, and an ADR plus receipt for every unit.
**Why:** about 62% of recent commits were tests or docs, and the plan itself had become the largest
artifact in the repo (about 2.5 MB of planning documents).

## D-004 · Archive the old planning tree — 2026-11-03, Razi

`docs/restructurization/development/`, `vision/`, `public-facing/` and the early `_verify` audits moved
to `archive/` with `git mv`; nothing deleted. `ui-anchor.md`, `memory-anchor.md`, the engineering learnings
and the project map moved to `reference/`. Two files that tests read were repointed: the gate register
(`archive/development/TESTING_GATES.md`, read by a `ci_gate.rs` test) and the P4-58 query inventory
(now `arcana-core/tests/fixtures/p4-58-query-inventory.md`).
