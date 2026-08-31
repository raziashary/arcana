# AutoDev Restructure — Runtime ADR Log

> **Append-only build-time decision journal.** A reversal is a new `SUPERSEDED-BY` entry, never an edit. Retractions of one's own findings are entries too — record the **mechanism of the error**, not just the correction.

| | |
|---|---|
| **Parents** | `MASTER_DEVELOPMENT_PLAN.md` §0.11 · `../vision/00-…-MASTER-PLAN.md` Part 8 |
| **Rev** | 1.0 — 2026-08-31 |
| **Entries** | **0** — no build-time decisions yet; awaiting GO |

---

# §0 · Register relationships

Three registers, three jobs. Confusing them is how a decision gets made twice with different answers.

| Register | Holds | Written when |
|---|---|---|
| `../vision/00-…-MASTER-PLAN.md` **Part 8** | **Pre-build truth** — the 51 ratified architectural decisions | During planning, before GO. **Closed.** |
| **`ADR_LOG.md`** (this) | **Build-time decisions** — everything decided while implementing | Continuously, during the build |
| House `DECISIONS.md` | The **ratified register** — promoted, durable decisions | At phase closeout, by promotion |

**The promotion flow:**

```
a decision is made during a task
   ↓  immediately (§0.11 — an acted-on-but-unlogged decision is a no-stale violation)
ADR_LOG entry, status DECIDED
   ↓  at phase closeout, if ratifiable
house DECISIONS.md, with a back-reference to the log ID
   ↓
ADR_LOG entry status → PROMOTED
```

**Phase-0 duty:** mirror the ratified pre-build decisions from vision Part 8 into the house `DECISIONS.md` as the starting register, each with a back-reference to its ADR id. This log stays empty until the first build-time decision.

---

# §1 · ID namespace

**`ADR-LOG-RESTRUCT-NNNN`**, numbered from 0001, **namespaced from day one**.

Two logs sharing one bare numbering space has already caused cross-pillar collisions in this house. More pointedly, **this very programme produced an ID collision during planning** — two different decisions were both numbered `ADR-02-06`, and both were answered before it was caught. The pillar prefix and the never-renumber rule exist because of exactly that.

**Never renumber history.** If an entry is wrong, supersede it.

---

# §2 · Authorship and tiering

Razi may rule at any time. Claude logs every decision under one of three tiers:

| Tier | Means | Requires |
|---|---|---|
| **`decided-independently`** | A clear precedent exists in the shipped codebase or the vision suite | Log it; proceed |
| **`flagged-and-proposed`** | A new dependency, pattern, or scope change — **or anything that narrows a §0 rule** | Log it, propose, and **wait**. Narrowing a §0 rule is always escalated, without exception |
| **`razi-ruled`** | Razi decided | Log it verbatim, then reword into the polished decision voice preserving intent |

**The escalation rule that matters most:** if implementing a spec faithfully would require narrowing an invariant or a §0 rule, that is never a quiet judgement call. It is `flagged-and-proposed`, and it stops.

---

# §3 · Entry format

Copy this block for every entry.

```markdown
## ADR-LOG-RESTRUCT-NNNN — <short title>

**Status:** PROPOSED | DECIDED | SUPERSEDED-BY ADR-LOG-RESTRUCT-NNNN | PROMOTED
**Author:** <who>
**Tier:** decided-independently | flagged-and-proposed | razi-ruled

**What.** <The choice, precisely. One or two sentences. No hedging.>

**Why.** <What breaks or is worse without it. Cite the grounding fact —
a file:line, a gate, a measured number. "It seemed cleaner" is not a why.>

**When.** <date · phase P<n> · triggering task P<n>-<nn>>

**Where.** <exact repo paths touched>

**How.** <The implementation, and the gate/test/evidence that proves it.
Name the evidence class.>

**Links.** <vision ADR ids · spec sections · TG gate ids · grounding report ids>
```

---

# §4 · Entries

*None yet. The build has not started.*

The first entries are expected in **P0**, where three decisions are already foreseeable and will be logged as they are made:

- the exact Ed25519 key-management procedure for the updater trust root (`P0-01`), which touches CI secrets and is `flagged-and-proposed`;
- whether `rmcp` and `axum` can be dropped outright or are still transitively required (`P0-16`);
- the shape of the `platform::Command` intent enum (`P0-23`), which every later phase depends on.

---

# §5 · Standing entries carried from planning

Not build-time decisions — but recorded here because a future session will otherwise rediscover them the hard way. Each is already ratified in vision Part 8; these are the **operational consequences** that bite during implementation.

| # | Carried fact | Consequence during the build |
|---|---|---|
| C-1 | `subagent/bridge.rs`'s SSE frame parsers are `pub(crate)` and **shared with** `sidecar/client.rs` | Those two deletions **cannot be sequenced independently**. Attempting one alone breaks the build. |
| C-2 | ACP sender identity is set in **exactly one place** — the MCP adapter deleted in P0 | `ToolContext.agent_role` must be truthful **before** P4 relies on messaging, or a preserved capability dies silently. |
| C-3 | `graph/` migration head **v5 is a broken statement** (`ADD COLUMN IF NOT EXISTS`) | It must be repaired before v6 is added, and the repair touches live per-repo graph DBs — back up first (§0.8). |
| C-4 | Four of seven surviving MCP items live inside **sidecar-named files** | They must be relocated **by address**, or sidecar code survives P8 by filename alone. |
| C-5 | The correct UTF-8-safe truncation **already exists** at `validate.rs:846-853` | Three sites panic on byte-slice truncation; the fix is to use the existing helper, not to write a fourth implementation. |
| C-6 | `is_read_only()` is documented as a **concurrency hint** and is currently the auto-approval key | Repurposing it back to concurrency and introducing `ToolEffect` must happen together, or a window opens where nothing gates prompting. |
| C-7 | Three subsystems share one missing path-normalisation helper — path lock, checkpoint retention, graph DB key | Fix once, in `autodev-core`, and route all three through it. Fixing them separately guarantees they drift again. |

---

# §6 · Log discipline

- **Immediate.** Logged in the same step as the decision, not at wrap-up.
- **Append-only.** Never edit a past entry; supersede it.
- **Retractions are entries.** When a finding or claim of mine proves wrong, the entry records **how the error happened**, not merely that it was corrected. Planning already produced six such corrections — including one where I silently narrowed Razi's ADR-02-02 answer instead of flagging that it conflicted with the doctrine. That kind of thing is logged, in the open, with the mechanism named.
- **A decision with no `Why` citing a grounding fact is not a decision.** It is a preference, and it gets challenged at closeout.

| Date | Rev | Change |
|---|---|---|
| 2026-08-31 | 1.0 | Created. Register relationships, namespaced ID scheme (`ADR-LOG-RESTRUCT-NNNN`), tiering, entry template. Zero entries — build not started. Seven standing facts carried from planning that would otherwise be rediscovered destructively. |
