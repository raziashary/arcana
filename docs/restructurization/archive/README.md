# Archive — the 2026-04 to 2026-10 restructuring programme

Retired on 2026-11-03 (see [`../DECISIONS.md`](../DECISIONS.md) D-004). Files were moved with `git mv`, so
`git log --follow` still reaches their history. **Nothing here is an instruction.** The live plan is
[`../SHIP_PLAN.md`](../SHIP_PLAN.md).

| Path | Contents |
|---|---|
| `development/MASTER_DEVELOPMENT_PLAN.md` | The P0–P9 task plan and the `§0` rules it imposed |
| `development/TESTING_GATES.md` | The `TG<phase>-<nn>` gate register. **Still read by a `ci_gate.rs` test** that checks every gate ID cited in shipped code exists here, so keep it |
| `development/ADR_LOG.md` | The 412-entry decision journal; code comments cite its IDs |
| `development/GOAL.md`, `READINESS_MATRIX.md`, `PROJECT_MEMORY.md` | Old status trackers. They disagree with each other; do not trust their counts |
| `development/P4_EXPANDED_GOAL.md` | The reliability and observability expansion; now backlog section B |
| `development/AUDIT_2026-10-18_CODEBASE_REFACTOR_PLAN.md` | Audit and refactor plan; now backlog section C |
| `development/ARCANA_CODING_SCOPE.md` | The 2026-09-23 coding-only scope; still the best statement of AC-01..AC-07 |
| `development/_verify/` | About 230 evidence receipts |
| `_verify-early-audits/` | Earliest audit notes (A11–A15, pre-P8 review) |
| `vision/` | The AutoDev-era design specs `00`–`10` and grounding notes |
| `public-facing/` | Old programme briefs |

Relative links between archived files still work. Links from them to `ui-anchor.md`, `memory-anchor.md`,
`PROJECT_MAP.md` or the engineering learnings now point to [`../reference/`](../reference/).
