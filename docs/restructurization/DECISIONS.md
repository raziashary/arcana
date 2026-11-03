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

## D-005 · Cuts confirmed, Linux/macOS work parked, Windows-first reaffirmed — 2026-11-03, Razi

Razi confirmed the proposed cuts in `SHIP_PLAN.md` (deferred: incremental graph updates `P5-08..13`, dark mode,
UI drift gate, reconnect replay, command type checking, performance bench, frontend reachability gate,
interaction-grammar gate, the production updater; kept: the daily spend ceiling `P2-21`). `P9-02`
(Windows code signing) is still an open ruling. The Linux smoke harness and macOS helper are committed on the
local branch `park/linux-macos-sandbox` (`bc54874`, unpushed). **Why Windows-only:** Razi's whole company runs
Windows, so the Windows harness is the product; nothing on `main` was removed to park the other platforms.

## D-006 · Terminal-control work stays in v1 scope — 2026-11-03, Razi (clarification) / Claude (edit, to confirm)

Razi clarified that most of the P4 effort was about not letting an agent run loose in the terminal. Code check:
none of it was lost (no code file deleted since `8356aea`; only a test path and e2e files changed). The plan was
however thin on the unfinished half: W9 (capability-scoped consent for the freeform shell, ADR-LOG-RESTRUCT-0401)
and attack testing of the command path sat in the backlog. They are added to `SHIP_PLAN.md` R1 as v1 items,
pending Razi's confirmation. This refines D-002 (the middle path), which named only `P4-18` and `P4-61`.

## D-007 · R0 done except one unreproduced flake: stale gates fixed, not weakened — 2026-11-03, Claude (to confirm)

The four red `ci_gate` tests were stale static scans. `tg_p1_05`, `tg_p3_03` and `tg_p3_14` read test code as
shipped code because they cut files at the first `#[cfg(test)]`; they now share `production_source`, which skips
modules declared under a `test` cfg and blanks each `#[cfg(test)]`/`#[cfg(all(test, …))]` item (brace-matched) instead
of cutting the file, so shipped code after a test-only helper, field or variant is now scanned too (25 of 351 files have such an item). No file is
excluded by name; self-tests run in both directions. The `e2e_probe` gate required DNS resolution and a runner that were removed on
purpose (commit `ef7c0eb`); it now pins the denial contract and re-activates the original rules if a runner
returns. **Retraction:** the earlier working note that these might be containment holes was wrong; none of the
four hid a hole. **Open:** one `fleet_offline` flake (a missing completion event in the log export) is
unreproduced in 40 runs and no production change was made for it; the test is not run by CI. Dependency installs
under the sandbox remain refused (`SHIP_PLAN.md` R1, ruling needed).

