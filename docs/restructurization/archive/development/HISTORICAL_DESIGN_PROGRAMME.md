# Historical AutoDev design programme (retired 2026-09-23)

These sections are preserved verbatim from the pre-Arcana development suite.
They record the former AutoDesign dependency and PD0–PD6 tasks, including
completed evidence. They are **not** Arcana's active task or gate register.
Razi retired the design contract and approved fresh Arcana state in
ADR-LOG-RESTRUCT-0042/0043. Current work follows P8 cleanup and P4–P7 coding
phases; P9 delivery is later.

## Former sequence and estimates

# §5 · Phase table and sequencing rationale

## §5.0 · Two rollouts (Razi, 2026-09-01)

> **Historical sequence superseded 2026-09-23.** Arcana has one coding-only
> product sequence: reconcile proof gaps → P4 fleet → P5 graph → P6 UI → P7
> experience → P9 delivery. ADR-0042/0043 retire the design R1/PD path. The
> original rollout tables below remain to explain completed work and old IDs.

AutoDev ships **twice**, and the first shipment is not the standalone application.

| | Rollout | The sentence that defines it | Audience |
|---|---|---|---|
| **R1** | **Backend for AutoDesign** | *"The backend is ready, wire it to AutoDesign."* | the AutoDesign team |
| **R2** | **The application** | *"The entire application is ready for usage."* | end users |

R1 is not a subset of R2 with pieces missing — it is a **complete product with a
different consumer**. AutoDesign supplies the UI, the file, the user and the
Apply/Reject review surface; AutoDev supplies ingestion, fidelity, export, adaptation
and changes, driven by a fleet with real memory. R2 then adds the parts that only
matter when a human drives AutoDev directly: the UI, the graph, first-run experience
and signed installers.

**Nothing is deferred.** Every phase below has a slot. Where a phase was previously
marked "deferred", it has been re-seated, not dropped.

## §5.1 · The R1 ordering, and why each phase sits where it does

⚠ **Rewritten 2026-09-03.** The previous version of this table had **no `PD0`
row at all** — the phase was built, closed and handed to a partner team while
the ordering table that is supposed to govern the programme did not list it —
and its task counts were stale for **nine** phases, most by more than double
(`P0` 12 against 27, `P2` 27 against 74, `P8` 24 against 65). Both figures are
now **derived from the document's own checkboxes**, not carried forward:

```
grep -cE '^- \[[ x]\] \*\*<PHASE>-[0-9]' MASTER_DEVELOPMENT_PLAN.md
```

`§0.3` is the rule this table broke, and it broke it in the one place a reader
goes to ask *what is left*.

| # | Phase | Why here | Tasks |
|---|---|---|---|
| ✅ | **P0** Stop the bleeding | done — 33 of 34; `P0-21`'s e2e harness is blocked on the environment, not on throughput. ⚠ Was **27** here until 2026-09-03: seven completed tasks (`P0-13b`, `P0-28`…`P0-33`) had no line in `§6` and so were invisible to every count that walks it (`A18` `S-39`) | 35 |
| ✅ | **P1** Structural | done | 22 |
| ✅ | **P2** Providers | Everything downstream makes model calls. **Gemini primary** (`gemini-3.5-flash` + 3.5 flagship); OpenAI and Anthropic follow in the same phase | 75 |
| ✅ | **P3 A–E** Runtime | The loop that will execute every design tool: fault isolation, one tool path, bounded context. Conversion holds a full layer bitmap set for 25–35 s, so `panic = "abort"` must die **before** the design phases, not after | 43 |
| ✅ | **P3-F** Memory | *"the agent needs proper guidance and memory as well, so memory is a no brainer."* Placed before the design phases deliberately: memory changes context assembly, and every design tool built earlier would need re-testing against the change | 18 |
| ✅ | **PD0** The capability contract | **Out of numeric order deliberately, and that is the point.** Its only dependency is `P2`; it does **not** wait for `P8`. Building it early is what let the AutoDesign team start their side while everything below is still being built. Closed 2026-09-03 — on the second attempt, after the `§0.2a` audits the first closure skipped | 10 |
| — | 🤝 **M1 · Contract reached** | ✅ 2026-09-03. An agreed, exercisable seam: the contract, a reference implementation they spawn as a child process, and a conformance suite either side can run. **`PD0` alone** — nothing else was needed for it | — |
| **4** | **P8** Sidecar deletion + legacy conversion | ⬅ **NEXT.** **Moved into R1, before handover** — *"we dont want them to have an unused sidecar on the backend."* Feasible because `OrchestratorProbe` already has a null implementation (`NoOrchestrator`) from P1, `skip_sidecars` already exists, and `P3-F` replaced `memory-mcp-server`, the last live dependency. ⚠ **67 tasks, not 24** — after Razi's pre-P8 review it also carries the live defects, the system-prompt floor, the procedural-memory seam, the Live Monitor restoration and the `airsctl agent` retirement, and — added 2026-09-04 on razi's ruling from `TG8-08`'s finding — the two event-wiring tasks `P8-66`/`P8-67` | 67 |
| — | 🤝 **HANDOVER** | The AutoDesign team receives a codebase with **no sidecar in it**, and the contract they have already been building against since M1. See `§5.3` | — |
| 5 | **PD1** Ingestion | PSD **and** PDF → the IR. The v4.7 operator-list pipeline is the source. 4 of its 25 tasks are already done (the AutoDesign sizing work and the layer IR) | 25 |
| — | 🤝 **M2 · First real capability** | A real PSD or PDF returns a real IR — the reference implementation starts returning true results. **`P8` + `PD1`**, ≈15 h from now | — |
| 6 | **PD2** Fidelity | *"visually it looks okay but in reality pixel by pixel wise is not."* Ingestion is not done until something can **prove** it, so this follows PD1 immediately. ⚠ Now also carries the renderer | 27 |
| 7 | **PD3** Export | webpage · PDF · back to PSD. Needs a fidelity measure to know an export is faithful, so it follows PD2 | 15 |
| 8 | **PD4** Adaptation | Deterministic geometry tool. Needs export to see its result and fidelity to grade it. ⚠ Now carries the adaptation-engine findings folded in by the pre-P8 review | 33 |
| 9 | **PD5** Changes | Reverses the PSD **construction** the source pipeline already performs. Needs PD3's writer | 13 |
| 10 | **PD6** Brand guideline | Consumes PD5's change primitives — a rebrand *is* a batch of changes driven by a guide | 10 |
| — | 🤝 **M3 · Every design capability live** | Fidelity, export, adapt, change and brand guideline all real. **`PD2`–`PD6`**, ≈30.5 h from now | — |
| 11 | **P4** Fleet | *"including the fleet."* Last in R1 because a fleet multiplies whatever the tools already do; multiplying unproven tools only multiplies the debugging. ⚠ **+4 on 2026-09-03** — `P4-G`, the one-fleet ruling (`ADR-LOG-RESTRUCT-0037`) | 49 |
| 12 | **R1 close** | Integration, post-phase audit (`§0.2a`), contract v1. ≈38 h from now | — |

**Open R1 tasks: 168.** Completed R1 tasks: **274** (`P0`–`P3-F` and `PD0`, plus
4 of `PD1`'s and 1 of `P4`'s taken early). Programme total **525**, of which
**251 open**. Hours are `§5.6`'s measured model, not estimates.

⚠ **Recounted 2026-09-03 by walking the checkboxes, and three of the four
figures here were wrong before `P4-G` was added.** The header said *"312 of
**506**"* while this paragraph said *"programme total **513**"* — two derived
totals, in one document, disagreeing by seven. Measured: **517** rows, **201**
done, **316** open, no duplicate ids. `P4-G`'s four account for the last of the
gap; the rest was `L-C1` residue that no count-walking check had ever been
pointed at the *totals*, only at the per-phase table. `_verify/MDP-COUNTS-CHECK.py`
now has a companion question to answer.

## §5.2 · The R2 ordering

*Counts here were re-derived on 2026-09-03 alongside `§5.1` and were already
correct — recorded so a reader knows they were checked rather than skipped.*
**Open R2 tasks: 83**, ≈14 h.

| # | Phase | Why here | Tasks |
|---|---|---|---|
| 13 | **P5** Graph | The first thing the *standalone* product needs and the design path never did: context scoping over a real codebase | 21 |
| 14 | **P6** UI | In R1 AutoDesign **was** the UI. Here AutoDev grows its own | 28 |
| 15 | **P7** Experience | Only meaningful once there is a UI to have an experience in | 18 |
| 16 | **P9** Delivery | Signing, installers, the capstone. **Certificate procurement has lead time — start it at R1 close, not here** | 16 |

## §5.3 · The handover point, stated precisely

**When:** after **P8**, before **PD1**.

**Why not earlier:** Razi's ruling — the AutoDesign team must not inherit a backend
carrying **3,300** lines of dead sidecar, 17 dead commands and a 225 MB bundle
resource. *(3,300 measured 2026-09-03 — `wc -l src-tauri/src/sidecar/*.rs`. This
sentence said 3,867 and `§4` said 3,161; `A18` `S-8`.)*

**Why not later:** everything after P8 is capability the contract already describes.
Making them wait for PD1–PD6 would idle them for the largest stretch of the programme.

**What they receive:**

1. A codebase with **no sidecar** — verified by content *and* by address (`P8-21`).
2. The **capability contract** (`PD0`), following AutoDesign's versioning and decode *conventions*. ⚠ Not written to their types: their capability id set is a closed 18-member enum and their manifest names AutoDesign as its only legal author, so a conforming AutoDev manifest is impossible by construction (`ADR-LOG-RESTRUCT-0034`).
3. A running AutoDev that answers the contract with an **executable reference
   implementation** they spawn as a child process — every operation returns a
   valid, correctly-shaped, decodable response before the conversion behind it
   exists.

Point 3 is what makes the parallelism honest. A contract nobody has exercised is a
document; a contract with an executable reference implementation behind it is an
integration they can build and test against on day one.

⚠ **Point 3 already happened.** `PD0` closed on 2026-09-03 and `M1` is reached,
which changes what this handover *is*: the AutoDesign team have had the contract
and the spawnable reference since then, so **`P8` is the only thing between here
and the handover**, and what they receive at it is a codebase with no sidecar
rather than a contract they are seeing for the first time. `PD0` was scheduled
early and separately for exactly this reason — its dependency was `P2`, never
`P8`.

⚠ **And one thing they receive is honestly incomplete**, stated here because
`§5.3` is the promise: three of the contract's declared shapes still have no
producer — six of nine diagnostic categories, `retryable: true`, and
`rendered-comparison` fidelity. That is published in the contract document's
`§9.0` with the owning phase against each, rather than left for them to
discover by writing a branch that never fires.

## §5.4 · Standing sequencing rules (unchanged)

Land **all migrations in the earliest persistence phase as one linear unit** — no
later phase renumbers. Build the **run/audit ledger early** so every later act can
stamp its run id. Sequence a phase after its data producer. Put the capstone last.

## §5.5 · The full phase table

| Phase | Rollout | Headline outcome | Depends on |
|---|---|---|---|
| **P0** | — | No unverified update installs · no command returns a key · the `/mcp` RCE is gone · gates can fail | — |
| **P1** | — | Everything shipped is in the binary · one schema owner · `autodev-core` exists · redaction restored | P0 |
| **P2** | R1 | Three providers conform · caching measurably works · cost is honest · **spend has a ceiling** | P1 |
| **P3 A–E** | R1 | Context is bounded · one tool path with a real gate · subagents in-process · a panic cannot kill the app | P2 |
| **P3-F** | R1 | Memory works and is **proven read** | P3 A–E |
| **P8** | R1 | Not one line of sidecar code survives, by content **or** by address | P3-F |
| **PD0** | R1 | The capability contract is published and answered by an executable reference implementation | P2 — ⚠ **not P8**; runnable now, and the AutoDesign team is blocked on it |
| **PD1** | R1 | A PSD **or** PDF becomes an IR that describes the design exactly | P8 |
| **PD2** | R1 | Fidelity is **measured**, not asserted — and refined until it passes | PD1 |
| **PD3** | R1 | The IR exports to webpage, PDF and PSD | PD2 |
| **PD4** | R1 | A design is reformatted to any target size by a **deterministic tool** | PD3 |
| **PD5** | R1 | Text, image, position, colour and shape can be changed and written back | PD3 |
| **PD6** | R1 | A brand guide is ingested and applied as a batch of changes | PD5 |
| **P4** | R1 | A task runs end to end and lands a build-verified, reversible artifact | PD6 |
| **P5** | R2 | The graph is compiled, correct across updates, and **read** for context scoping | P4 |
| **P6** | R2 | The anchor is implemented · no event is lost · every setting persists | P5 |
| **P7** | R2 | A new user reaches a working agent turn without documentation | P6 |
| **P9** | R2 | Signed, verified installers for Windows, Linux and macOS | P7 |

## §5.6 · Measured throughput — what the phases actually cost

⚠ **This section exists because the estimates lived nowhere near the plan.**
Until now the only per-phase hour figures in the programme were in the
*public brief*, which is a condensation and has no business being the sole home
of a planning number. It went stale unchallenged for three phases. The numbers
belong here, next to the tasks they are derived from, and the brief renders
them like everything else.

### Method, stated so the figures can be argued with

Both columns come from commit timestamps — nothing is recalled or estimated.

* **Span** — first commit of a phase window to its last. What a calendar shows.
* **Active** — the same, minus every gap longer than **45 minutes**. The
  threshold is stated rather than tuned: in dense stretches the commit cadence
  here is 10–20 minutes, so a multi-hour gap is a break, not silent work.

Active is a **lower bound on effort**: any real work that produced no commit
for 45 minutes is discarded by this method. It is the right bias — an estimate
built from a number that flatters the estimator is worthless.

Task counts are the **MDP's own checkbox counts**, not the brief's table, which
was stale for three completed phases (it recorded P0 at 12 tasks).

⚠ **And when that sentence was written, `§5.1` of this document said 12 as well**
— the example of the brief's staleness was live two sections above it (`A18`
`S-42`). `§5.1` is now derived from the checkboxes and **checked** by
`_verify/MDP-COUNTS-CHECK.py`, which is what makes the claim above true rather
than merely asserted. ⚠ P0's real figure is **34**, not the 27 that correction
first produced: seven completed tasks had no line in `§6` at all (`S-39`).

### Measured, `P0` → `P3-F`

| Window | Tasks | Span | Active |
|---|---|---|---|
| Anchor set (planning) | — | 0h 03m | 0h 03m |
| **P0** build + closeout + audit | 26 | 11h 35m | **4h 43m** |
| **P1** build + audit + rulings | 22 | 6h 19m | **3h 57m** |
| AutoDesign sizing + the layer IR | 4 *(PD1)* | 0h 20m | 0h 20m |
| **P2** re-cut + build + audit | 75 | 14h 15m | **5h 32m** |
| **P3 A–E** build + audit | 43 | 5h 51m | **4h 50m** |
| **P3-F** build + audit + closeout | 18 | 2h 54m | **1h 58m** |
| **Total** | **183** | **≈49h calendar** | **21h 23m** |

**8.6 tasks/hour, all-in** — build, smoke, gates, audits, remediation, trackers
and commits included. Two days of calendar; twenty-one and a half hours of work.

### The planning rate is deliberately lower than the measured one

A phase's task count **grows during its own pre-build refutation**, and the
estimate has to be against the count as *written*, because that is the only
number available when the estimate is made. Observed growth: `P2` **27 → 74**
(×2.7), `P3 A–E` **28 → 43** (×1.5), `P3-F` **15 → 18** (×1.2). The trend is
downward — the plan gets better as more of the codebase is known — but it is
not 1.0 and will not be.

So the figure to plan with is **6 tasks/hour against the written count**
(≈10 minutes per written task), which carries the growth, the audit and the
closeout inside it. Checked against the three phases that had both a written
count and an outcome:

| Phase | Written | Old estimate | This model | Actual |
|---|---|---|---|---|
| `P2` | 27 | ~11 h | 4.5 h | **5h 32m** |
| `P3 A–E` | 28 | ~12 h | 4.7 h | **4h 50m** |
| `P3-F` | 15 | ~7.5 h | 2.5 h | **1h 58m** |

Two within 35 minutes, one conservative by an hour. The **old** estimates were
built on an unstated ~2.3 tasks/hour and were wrong by 2.5–3.8×, in the
direction that matters least for trust and most for planning: everything landed
early, so nothing was ever caught, and the figure quietly stayed wrong.

### Recalibrated remainder — 312 open tasks

⚠ **Re-cut 2026-09-03**, after Razi's pre-P8 review folded the investigation's
findings into the plan. The remainder grew from 223 tasks to **312**, and
`P8` grew from 24 tasks to **65** — it now carries the live defects, the prompt
floor, the procedural-memory seam, the Live Monitor restoration, the `airsctl
agent` retirement and the complete legacy conversion, not just the sidecar's
deletion. The rate is unchanged; only the work is.

| Phase | Open | Prev. est | **New** | Confidence |
|---|---|---|---|---|
| `P0-21` e2e harness | 1 | *blocked* | *blocked* | Environment, not throughput |
| **P8** Conversion + defects | 67 | ~4 h | **~11 h** | ⚠ Medium — 21 of its tasks are prose or frontend, where no throughput has been measured |
| **PD0** Contract | ~~7~~ **10** | ~1 h | **done** | — |
| **PD1** Ingestion | 21 | ~2.5 h | **~3.5 h** | ⚠ Medium |
| **PD2** Fidelity | 27 | ~2.5 h | **~4.5 h** | ⚠ Low — a research loop, and now carrying the renderer |
| **PD3** Export | 15 | ~2.5 h | **~2.5 h** | ⚠ Medium |
| **PD4** Adaptation | 33 | ~2 h | **~5.5 h** | ⚠ Medium |
| **PD5** Changes | 13 | ~2 h | **~2 h** | ⚠ Medium |
| **PD6** Brand guideline | 10 | ~1.5 h | **~1.5 h** | ⚠ Medium |
| **P4** Fleet | 44 | ~5 h | **~7.5 h** | High — Rust, and the shape is known |
| **Rollout 1** | **229** | **≈23.5 h** | **≈38 h** | |
| **P5** Graph | 21 | ~3.5 h | **~3.5 h** | High |
| **P6** UI | 28 | ~4.5 h | **~4.5 h** | ⚠ **Low — but no longer zero.** `PD0`'s ten tasks were **entirely TypeScript** and took ≈1 h all-in, which is the programme's only frontend datapoint. It is contract code rather than UI, so it is weak evidence for `P6` — but *"no measured frontend throughput"* stopped being true on 2026-09-03 (`A18` `S-36`) |
| **P7** Experience | 18 | ~3 h | **~3 h** | ⚠ Low — same reason |
| **P9** Delivery | 16 | ~2.5 h | **~2.5 h** | ⚠ **Not throughput-bound** — gated on Razi's signing keypair and on CI across three platforms |
| **Rollout 2** | **83** | **≈14 h** | **≈14 h** | |
| **Programme** | **312** | **≈37.5 h** | **≈52 h** | |

### Handover is three milestones, not one

🔴 **Corrected 2026-09-03, on Razi's challenge.** This plan and the public brief
both carried a single row reading *"HANDOVER — the AutoDesign team can begin
wiring"*, placed after `P8`. He asked the obvious question: how can they wire
against a contract that does not exist yet, for capabilities built after `P8`?

Both are true, and the single-milestone framing overstated it. What `PD0`
delivers is a **contract plus an executable reference implementation** (`PD0-06`),
which lets AutoDesign
build and test *their* side against a real, versioned, exercised surface while
the capability behind it is still being built. That is genuine parallel work —
and it is not "AutoDev is ready for them".

| Milestone | What AutoDesign can actually do | Needs | Cost |
|---|---|---|---|
| **M1 · Contract** | ✅ **Reached.** An agreed, exercisable seam: the contract, a reference implementation spawned as a child process, and a conformance suite | **`PD0` only** | **done** |
| **M2 · First real capability** | Send a real PSD or PDF and get a real IR back — the reference implementation starts returning true results | `P8` + `PD1` | ≈15 h |
| **M3 · Every design capability live** | Fidelity, export, adapt, change, brand guideline | `PD2`–`PD6` | ≈30.5 h |
| **R1 close** | The above **plus the fleet** — the point at which R1 is complete | `P4` | ≈38 h |

⚠ **`M3` used to read "Full Rollout 1 … ≈39.5 h" and both halves were wrong.**
The row's own *Needs* column says `PD2`–`PD6`, which excludes `P4` — so it was
never "full Rollout 1", and the fleet is 7.5 h of that gap. The 39.5 h figure
was the pre-recut R1 total carried forward after `§5.6` was recomputed to 38 h,
which is `L-C1` again: a number restated in a second place and not regenerated
when its source changed. Derived here from `§5.6`'s own rows:
`P8` 11 + `PD1` 3.5 = **15 h** to `M2`; + `PD2` 4.5 + `PD3` 2.5 + `PD4` 5.5 +
`PD5` 2 + `PD6` 1.5 = **30.5 h** to `M3`; + `P4` 7.5 = **38 h** to R1 close.

⚠ **`PD0` is not blocked by `P8`.** Its dependency is `P2`, which closed on
2026-09-02, and `§10.5.0` already says it *"runs early and in parallel"*. The
public brief's phase table nonetheless listed it **last, after `P4`**, which
reads as the contract arriving at the very end — the opposite of its intent.
That was a presentation defect and is corrected.

~~**Recommended order: `PD0` first, then `P8`.** One hour unblocks a partner team
who are otherwise waiting on a document, and nothing in `PD0` depends on the
sidecar being gone.~~ ✅ **Taken, and done.** `PD0` closed 2026-09-03 and `M1`
is reached; `P8` is next. Kept struck rather than deleted because the *reasoning*
— that a partner team's unblock outranks a deletion in sequencing — is the
precedent, and `PD1`–`PD6` will face the same question.



### Where this model should NOT be trusted

Stated rather than buried, because a recalibration that is confident everywhere
is just the old error with a new number:

1. **All 183 measured tasks are Rust and documentation.** `P6` and `P7` are 46
   tasks of frontend work with **zero** measured throughput behind them.
2. **`PD2` is a research loop.** "Fidelity is measured and refined until it
   passes" does not decompose into 14 units of equal cost, and no phase so far
   has had that shape.
3. **`P9` waits on things that are not me** — a keypair only Razi can generate,
   and installer builds on three platforms.
4. **The audit overhead is 46% on top of build time and is included above.**
   It has fallen every phase (P0 3h 36m → P3-F 19m) as the plan improved, and
   it is assumed to stay low. If a phase's audits start finding what P3-F's
   did, that assumption goes.


## Former P3-G design allocation

### P3-G · Design ingestion — ~~a sub-section~~ → **PROMOTED to phases PD0–PD6**

⚠ **Superseded 2026-09-01, razi-ruled.** This was written as a ten-task sub-section of P3.
Razi's third pass expanded the scope well past what a sub-section can hold — pixel-level
fidelity detection with its own refinement tooling, PDF ingestion alongside PSD, export to
three formats including back to PSD, adaptation as a **deterministic tool**, a change
system reverse-engineered from the source pipeline's PSD construction, and brand-guide
ingestion. That is **88 tasks across seven phases**, not a lettered group.

The work moved to **`§10.5`, phases PD0–PD6**. Task IDs are **not** renumbered (`§1`):

| Old | New | State |
|---|---|---|
| `P3-45` vendor `ag-psd`, no CDN | `PD1-01` | ✅ **done** — pinned `30.1.0`, bundled by Vite |
| `P3-46` the layer IR | `PD1-02` | ✅ **done** — `src/design/ir.ts`, `assertNoPixels` |
| `P3-47` `U+0003` → line break | `PD1-03` | ✅ **done** — falsified |
| `P3-48` report unresolved fonts | `PD1-04` | ✅ **done** — `resolveFont` returns three states |
| `P3-49` conversion as an agent tool | `PD1-18` · `PD4-11` · `PD5-13` | struck here |
| `P3-50` reformat across aspect ratios | **`PD4`** (13 tasks) | struck here |
| `P3-51` rebrand + content swap | **`PD5`** + **`PD6`** (22 tasks) | struck here |
| `P3-52` brand-guide input | **`PD6-01`**…`PD6-02` | struck here |
| `P3-53` publish the capability contract | **`PD0`** (6 tasks) | struck here |
| `P3-54` conversion under fault isolation | `PD1` exit condition; `P3-A` is now an **ordering** dependency | struck here |

The four completed tasks stay ticked — the work exists, is tested and is pushed. They are
carried into `PD1-A` with their evidence rather than re-proven.

**Smokes `S3-06`…`S3-10` likewise move** to `SD1`…`SD6` beside the phases that own them.


## Former P4-G design verification tasks

### P4-G · One fleet, two definitions of done *(razi-ruled, `ADR-LOG-RESTRUCT-0037`)*

> **Withdrawn from Arcana's active worklist (ADR-0042/0043).** P4-45–P4-48
> were added for design verification and design recipes. Coding build evidence
> remains covered by P4-C and must still be proven. These historical rows are
> left unchecked and do not count as required active coding tasks.

> 🔴 **The question and the ruling.** Razi asked whether the `PD-X` design work
> should get its **own fleet** — a main designer, a QC reviewer, a brief
> analyst — since the existing one is shaped for programming. **Ruled: one
> fleet.** A design worker is a **recipe**, because `P4-30` already says *"DB
> owns identity/state, recipe owns the prompt"*. A second fleet would duplicate
> dispatch, atomic claiming, budgets, ACP messaging and affinity — **each of
> which has an open defect in the four groups above** — and the second copy is
> the one nobody runs.
>
> ⚠ **What actually differs is not the roster; it is the definition of done.**
> `P4-11`…`P4-15` hardcode verification as *build evidence*: an ecosystem-
> detected command, a real exit code, a persisted row `qc_review` reads. Design
> work has no compiler, and a QC worker that cannot fail objectively is
> theatre (`L-C9`). The oracle already exists in this plan — **`PD2` is a
> pixel-level fidelity measure** — so the seam is built here and its second
> implementation lands with `PD2`.
>
> ⚠ **And the framing correction that made this cheap:** `PD1`–`PD5` are not
> creative-design tasks. Read `§5.1`: ingestion, fidelity, export, *"a
> deterministic geometry tool"*, change primitives. That is a **pipeline**, and
> building it is programming work the existing worker shape already fits.

- [ ] **P4-45** A **`Verification` trait** with `BuildEvidence` as its first implementation; `qc_review` consults the trait rather than the concrete type. ⚠ Sequenced **after** `P4-14`, which is what makes `qc_review` read the persisted table at all — a seam over a guard that still reads the in-memory map would abstract the wrong thing. → `UNIT-PROVEN`
- [ ] **P4-46** The task type carries **which verification it requires**, and an unknown value is a **loud refusal**, never a default pass. ⚠ Same rule as `P4-02`'s `FromStr`, for the same reason: a silent default here means a design task is graded by a compiler that never ran, and reports green. → `LIVE-DB-PROVEN`
- [ ] **P4-47** A **`FidelityEvidence`** implementation backed by `PD2`'s measure. ⚠ **Lands with `PD2`, not here** — this row exists so the second implementation has an owner and cannot be forgotten, which is `L-C12`'s remedy applied in advance. Until `PD2`, a design task's verification is `Unsupported` and refuses, per `P4-46`. → `GATE-PROVEN`
- [ ] **P4-48** Two design **recipes** — `designer` and `design-qc` — extending `P8-34`'s invariant core like every other agent. **No new dispatch code, no second scheduler, no second claim path.** Gate: a design task is dispatched, claimed and QC'd through the *same* code path as a build task, differing only in its recipe and its verification. → `SMOKE-PROVEN`

> ⚠ **Explicitly not created: a *brief analyst* worker.** It is a phase of a
> task rather than an agent, and the capability it names — read the brief,
> gather references, return a typed summary — is what the **intelligence
> layer** (`AIRS-Foundry/intelligence-layer`, `M1`) already does. A worker for
> it would be a second implementation of a shipped capability. If `PD6`'s brand
> guideline work shows a standing need, a *guideline interpreter* recipe is
> added then, on its own evidence — not now, on anticipation.





## Former PD0–PD6 phase tasks

# §10.5 · Rollout 1 · The design phases (PD0–PD6)

> **Withdrawn 2026-09-23 by Razi (ADR-0042/0043).** PD0 is historical shipped
> work whose executable contract has been retired from Arcana source and CI;
> PD1–PD6 are not future Arcana tasks. Preserve the original IDs and evidence
> below for traceability. Unchecked PD rows are not active coding work.

**Added 2026-09-01** on Razi's third-pass direction. These phases are what makes AutoDev
the executor behind AutoDesign. They are grounded in `C:\AIRS Foundry\apps\psdconversions`
— its `index.html` (3,202 lines, v4.7/v5.0) and its four architecture documents — and in
the P1 spike, which converted two real ad PSDs end to end and verified them visually.

**The one architectural commitment, stated once and inherited by every phase below.**
The converter's HTML output measured **17.41 MB** (1080×1080) and **25.27 MB** (1080×1920),
because every layer is an embedded base64 PNG. That cannot enter a model's context. The
source's own architecture document already ruled on it:

> *"the in-memory layer array is the universal intermediate representation … Everything
> downstream — PSD, HTML, SVG, React, Flutter — is just a different serialization of that
> same structured data."*

So: **the IR is the working format; every output is a serializer; every tool is an IR
mutation.** Ingestion produces it, fidelity grades it, export serializes it, adaptation and
changes mutate it.

---

## §10.5.0 · PD0 · The capability contract

**Goal.** The AutoDesign team can build against a real, versioned, exercised surface.
**Depends on.** P2. **Runs early and in parallel** — its schema half depends only on the
operation set, which is decided.

> ⚠ **AMENDED by the `§0.2m` pre-build refutation, 2026-09-03**, before the first
> line of code. Full findings in `_verify/PD0-CORRECTIONS.md`; two razi rulings
> and four corrections are folded into the task lines below.
>
> **The two rulings.**
> **(1) Transport — AutoDev becomes a sidecar of AutoDesign.** Razi: *"AutoDesign
> does not have any opening point and AutoDev itself is a local application, so
> if we create an HTTP process it will be unnecessary; and if we make it an MCP
> server, AutoDesign does not have an MCP client and building one will take
> time. So we will use AutoDev as a sidecar in AutoDesign essentially — an
> executable contract, and a single markdown document in docs public-facing so
> the AutoDesign team knows what to do."* So the seam is **process invocation
> over stdio**, not a socket, and PD0 ships an **executable** contract rather
> than a listening stub.
> **(2) The IR type is TypeScript-canonical**, with Rust mirroring in `PD1`.
>
> **What the refutation established, which the tasks below now reflect:**
> - 🔴 **There is no transport, and PD0-06 assumed one.** Searched — not sampled
>   — for `axum`, `actix`, `warp`, `hyper::server`, `TcpListener::bind` and
>   `HttpServer` across all three crates: **zero hits**, and no server dependency
>   in either manifest. `/mcp` was deleted in P0 for accepting unauthenticated
>   remote execution, and the sidecar's loopback dies in P8. Ruling (1) settles
>   it without re-opening that surface.
> - 🔴 **The IR is not versioned.** `ir.ts:155`'s `DesignDocument` carries
>   `width`, `height`, `sourceName`, `layers`, `fonts`, `assetIds` — and no
>   version field of any kind. `PD0-02` says *"published as a versioned type"*;
>   the type exists, the versioning does not. Now `PD0-02`'s actual work.
> - ⚠ **The IR lives in the frontend** (`src/design/`), while the capability that
>   serves it will be Rust. Ruling (2) settles ownership; the drift risk is real
>   and `PD0-02` carries the pinning test that answers it.
> - ⚠ **`integration.invoke` — AutoDesign's ability to call anything — is at
>   G6, deferred.** Which does *not* block PD0: Razi ruled on 2026-09-01 that
>   AutoDev *defines and publishes* the contract precisely so both halves build
>   in parallel. It does mean the handover milestone is *"an agreed seam"*, not
>   *"they can wire today"*.
> **The AutoDesign-side refutation found four more, all re-verified here:**
> - 🔴 **The contracts package cannot be conformed to.** `@airs/auto-design-capability-contracts@0.2.0` is real and well-built, but its capability id set is a **closed 18-member enum** — `capabilities.ts:37` `z.enum(AUTO_DESIGN_CAPABILITY_IDS)` — its manifest declares `productOwner: z.literal('airs_auto_design')` (`:48`), and the catalog must contain **every** canonical capability exactly once (`:100`, `:108-111`). **AutoDev is not one of them and cannot publish a conforming manifest.** ⚠ This *validates* the sidecar ruling rather than threatening it: the capability-host route was closed to us by construction, so a child process with its own protocol was always the only open door. What we inherit is their **conventions**, not their types.
> - 🔴 **`PD0-02` as written is impossible even for them.** `invocation.ts:35-39` caps capability input at **16 KiB** — *"Capability input exceeds the 16 KiB envelope limit."* The IR of a 30–45 MB PSD does not fit, and *"the IR schema is the wire payload"* assumes it does. **Their own answer is a reference**: `sourcePdf.ts:153-155`'s `resultRef` plus a three-step byte handshake. `PD0-02` is corrected to match, and the constraint is real for a sidecar too — a 25 MB JSON blob down a pipe is the same mistake with a different carrier.
> - 🔴 **`degraded` is a decode failure on their side.** `lifecycle.ts:80-85` rejects any result carrying both output and an error, which is exactly what *"succeeded with loss"* is. But they solved the same problem in the PDF family: `sourcePdf.ts:170` `outcome: z.enum(['inspection_ready', 'partial', 'held'])`. `PD0-04` adopts those semantics.
> - ⚠ **Asset transfer is already designed, twice** — `artifactReferenceSchema` (`artifacts.ts:4-21`, and its `expiresAt`/`createdAt` pair is the *"who owns their lifetime"* answer `PD0-05` asks for) and a request→handle→receipt byte handshake (`sourcePdf.ts:72-98`). `PD0-05` conforms rather than invents. ⚠ Two gaps are theirs to close, not ours: `artifactKind` is a closed five-member enum with no PSD or raster, and the byte route is `z.literal('application/pdf')`.
>
> ⚠ **One correction to the ruling's premise, which does not change the ruling.** *"AutoDesign does not have any opening point"* — it has one: `/v1/forge/tool-invoke` (`createLoopbackServer.ts:726`), a live authenticated inbound route, and AutoDesign already registers twelve tools into Forge via `auto_execute.register_tools`. But it is reached **by Forge**, not by AutoDesign directly, so using it means depending on a third system that carries a hard no-edit rule. The sidecar remains the least-coupled choice and the only one that needs nothing from anybody. Recorded in `PD0-09` as the documented future option.
> > - ⚠ **The design programme has no `NOVELTY_LEDGER` rows** despite being
>   grounded in an external converter, an external contract shape and an
>   external adaptation engine. `PD0-10` opens the register.

- [x] **PD0-01** The operation set is **named and frozen for v0**: `ingest` · `export` · `adapt` · `change`. Four verbs, because those are the four things Razi specified and a fifth invented now is a fifth to version forever. → `DOC-GATE` *(`TGD0-01`. ⚠ **Not falsified** — corrected 2026-09-03 by the work audit, which found the notes claiming eight falsifications against four actually run (`TGD0-02`, `04`, `06`, `07`). Falsification is this programme's strongest evidence currency and inflating it 2× is the worst place to be loose. Four verbs frozen, and `measure` **reserved** rather than invented — a reserved name is refused *as reserved*, so a caller learns it is coming instead of concluding they misspelled something. ⚠ The mapping to AutoDesign's eighteen capability ids is published, because there is **zero** lexical overlap and `adapt` has no counterpart at all — its namesake lives in their Forge *tool* register, a different contract entirely)*
      ⚠ **Amended: these are AutoDev's verbs, and the mapping is a deliverable.** Searched AutoDesign for `ingest`, `adapt` and `change` as literals: **zero hits**. Their vocabulary is 18 dotted `noun.verb` ids, and `adapt` has **no counterpart at all** — its only namesake, `adapt_artboard`, lives in a different registry (their Forge *tool* register, a different contract with a different consumer). As a sidecar we keep our four verbs; what we owe their team is an explicit **mapping table** in `PD0-09`. ⚠ And one thing the table must say out loud: our single `change` collapses a **1→8 fan-out** they deliberately split by risk and approval — `document.command.apply` is `risk: high, approval: explicit` while `transform.apply` is `approval: none`, and `controls.approvalIsCapabilitySpecific` is `z.literal(true)`, so that gradient is load-bearing for them. A consumer that flattens it loses a safety property.
- [x] **PD0-02** The **IR schema is the wire payload**, published as a versioned type. Not the HTML, not the PSD bytes — see the commitment above. → `UNIT-PROVEN` *(`TGD0-02` **falsified**; `TGD0-03` gate-proven, **not** falsified. ⚠ **Corrected by the work audit: the IR did not gain a version — the *envelope* did.** `src/design/ir.ts` is not in this phase's diffstat, and `DesignDocument` still carries no version field. What is versioned and checked is the `DesignRef` pointing at it, so a document whose *shape* changes still decodes under `autodev-design-ir/0.1.0`. Binding the constant to the type is `PD1`'s pinning test. The wire carries `{schemaVersion, ref, sha256, byteLength}` — **never the document**. An unknown version is rejected rather than parsed. ⚠ The no-document rule is gated as a **byte budget** on every serialized event, not as a structural check on the type: a structural check passes for ever while somebody adds a convenience field holding the document)*
      ⚠ **Amended: the versioning is the work.** `src/design/ir.ts:155` already defines `DesignDocument`, and it carries **no version field** — so "published as a versioned type" describes something that does not exist yet. Add an explicit schema version, and a check that **refuses** a payload whose version it does not understand rather than parsing it optimistically. ⚠ **TypeScript is canonical** *(razi-ruled)*: the IR and the consumer are both TS today, and `PD1` mirrors it into Rust behind a conformance test that pins the two together — the `src-shared` mirrored-table problem P1 deleted is exactly what that test exists to prevent.
      🔴 **And the payload is a REFERENCE, not the document.** *"The IR schema is the wire payload"* does not survive contact: AutoDesign caps capability input at **16 KiB** (`invocation.ts:35-39`) and a 30–45 MB PSD's IR is orders of magnitude past it. Their own solution is a typed `resultRef` (`sourcePdf.ts:153-155`) plus a byte handshake. **The wire carries `{schemaVersion, resultRef, sha256, byteLength}`; the IR is written beside it and fetched by reference.** ⚠ This is not merely conformance — it is right for a sidecar too. Twenty-five megabytes of JSON down a pipe is the same mistake with a different carrier, and it is the mistake the IR was created to avoid in the first place (`ir.ts` header: the converter's 17–25 MB HTML *"cannot enter a model's context"*).
- [x] **PD0-03** Job semantics mirror AutoDesign's existing `CapabilityJobService`: async, terminal statuses, operation events. Conversion runs 25–35 s on a 30–45 MB input, so a synchronous contract would be wrong on the first real file. → `UNIT-PROVEN` *(`TGD0-03`; behaviour in `contract.test.ts` "the conformance suite", and driven end-to-end by `conformance.ts`'s five per-operation checks. ⚠ **The proof was unnamed until the work audit (`W-20`)**: `§0.2e` requires every task to name the smoke or gate that proves it, this task and `PD0-05` named none, and a note with no named artifact is a note nothing can check — which is exactly the gap that let `PD0-07` be ticked with no deliverable at all. Job semantics as a spawned child: `accepted` → `started` → `progressed` → exactly one `settled`, always last, sequence numbers strictly increasing, the caller's `operationId` echoed on every event. ⚠ Mirrors AutoDesign's **public** lifecycle vocabulary rather than its internal one — theirs projects nine internal statuses lossily onto ten public ones, and mirroring the internal set would be mirroring an implementation detail they already hide. ⚠ We declare only what we emit: three of their declared event types have no producer at all)*
      ⚠ **Amended for the sidecar model.** With AutoDev spawned as a child process, "async job" means **a long-running invocation that streams progress and terminal status over stdout**, not an HTTP job resource that is polled. Statuses and events stay as the contract names them; the carrier changes. ⚠ The 25–35 s figure is inherited from the spike — if it turns out to be an estimate rather than a measurement, the async decision still holds, because a 30–45 MB PSD parse is not a request-response shape at any plausible number.
- [x] **PD0-04** An **error taxonomy**: ~~distinguishing *refused* (bad input), *failed* (we broke), and *degraded* (succeeded with loss — an unresolvable font, a rasterized blend mode). Degraded is the one that matters: it is the case the old converter reported as success.~~ → **`complete` · `partial` · `held`.** ⚠ The struck text is the task **as written**, kept because the *intent* — name the succeeded-with-loss case so it stops being reported as success — is exactly right and is what `partial` now carries. The three names were refuted during the phase (see the note below) and the premise was left unstruck until 2026-09-03 (`A18` `S-23`), so a completed task went on advertising a taxonomy the same task had replaced. → `UNIT-PROVEN` *(`TGD0-04` **falsified**; `TGD0-05` gate-proven, **not** falsified — and it has no positive control, so a decoder refusing *every* `partial` would pass it. `complete` · `partial` · `held`, with all three invariants enforced by the decoder rather than documented. ⚠ **Not `refused`/`failed`/`degraded`** — a "succeeded with loss" result cannot cross AutoDesign's seam at all, since `lifecycle.ts:80-85` rejects any result carrying both output and an error. They solved the same problem in their PDF family and we took their answer. **`partial` must be evidenced**, which is what makes it mean something)*
      ⚠ **Amended to a taxonomy that composes.** The instinct is right and the *names* are wrong: AutoDesign's result schema **rejects** a payload carrying both output and an error (`lifecycle.ts:80-85`) — which is precisely what "succeeded with loss" is, so a `degraded` result would fail their decode. They hit the same problem and solved it in their PDF family: `sourcePdf.ts:170` — `outcome: z.enum(['inspection_ready', 'partial', 'held'])`, with `partial` required to be evidenced and `held` required to carry a typed failure. **Adopt those three**, and carry their nine-value `category` and `retryable` alongside (`lifecycle.ts:55-61`) so a consumer can route on either. ⚠ `L-C12` applies with force: `ErrorClass` in `provider/usage.rs` sat for months *"defined, unit-tested against itself, and never produced by anything"*. Every variant here needs a producer or it should not exist.
- [x] **PD0-05** **Asset transfer**: how bitmaps move between the two processes, and who owns their lifetime. → `UNIT-PROVEN` *(Proof: `contract.test.ts` "artifact references" — three cases, including the expiry-before-creation refusal and the offset-less timestamp refusal. ⚠ Unnamed until `W-20`; see `PD0-03`. Artifact references **follow** AutoDesign's `artifactReferenceSchema`, so the shape is familiar and a mapping is mechanical. ⚠ **Corrected: it is NOT drop-in, and the first version of this note said it was.** Ours adds `contractVersion` and `artifactKind`, their schemas are `.strict()`, and their `artifactKind` is a closed enum against our free-form string — so their decoder rejects our payload on the undeclared keys. Both halves of that contradiction were in files this phase wrote, and it was published to the partner team before the audit caught it. **`createdAt`/`expiresAt` is the lifetime answer**: the producer owns it until expiry, and `null` means never — not "unknown". Expiry must be strictly after creation, their refinement kept. ⚠ Timestamps require an explicit UTC offset, because this codebase shipped the offset-less defect **twice in one week in both directions** — an age that always read 0s, and an age inflated by 5.5 hours)*
      ⚠ **Amended: this is already designed on their side — conform to it.** `artifactReferenceSchema` (`artifacts.ts:4-21`) carries `artifactId`, `mediaType`, `byteLength`, `sha256`, `storageRef`, `createdAt` and `expiresAt` — and that last pair **is** the lifetime answer this task asks for, with a refinement asserting expiry follows creation. Mirror that field set. Bytes move as a raw body with a pinned content type and a hard cap, never base64 inside JSON (`sourcePdf.ts:21`, 32 MB). ⚠ **Two gaps are theirs to close and the document must name them**: `artifactKind` is a closed five-member enum with no `psd` and no generic raster, and the byte route is `z.literal('application/pdf')`. Until those widen, PSD bytes have no legal path into their side — which is fine, because in Rollout 1 the PSD arrives at *our* end.
- [x] **PD0-06** 🔴 **RE-POINTED.** ~~A **live stub** answers every operation over a transport~~ → **an executable contract**: a reference implementation answering all four operations, plus a **conformance suite both teams can run** against any implementation of the contract. The intent is unchanged and is the right one — *"a contract nobody has exercised is a document"* — but the original wording assumed a listening surface that does not exist and that Razi has ruled against building. ⚠ **`UNIT-PROVEN` at closeout — the work audit was right — and `SMOKE-PROVEN` from `2026-09-03`, when the thing the class describes was actually built.**

  The order matters and is recorded rather than smoothed over. At closeout the class on this line said `SMOKE-PROVEN` and the register defines that as *"the end-to-end smoke driving the real app"*, while `runConformance` had four call sites and every one was inside vitest. `W-3` graded that `UNIT-PROVEN`, correctly. What changed is not the grading but the evidence: `W-2`'s remediation added `scripts/smoke-design-sidecar.mjs`, which **spawns** the reference implementation as a child process across 14 cases — four operations, four holds, five malformed requests and a killed child — with nothing running in-process. That is a smoke driving the real binary, so the class is now earned rather than claimed. **A class is upgraded when the proof lands, never before it.** ~~The register defines `SMOKE-PROVEN` as *"the end-to-end smoke driving the real app"*, and `runConformance` has four call sites, all inside vitest. The suite is not `reference.ts`'s own unit test, which is what the original note leaned on — but it is still started by one, and that is a weaker thing than the class claims.~~ **← the pre-remediation argument, struck 2026-09-03.** It sat immediately after the sentence that supersedes it, so the note asserted the class was earned and then, two sentences later, that it was not (`A18` `S-24`). Kept rather than deleted because it is the reasoning that produced the `UNIT-PROVEN` downgrade, and the downgrade was correct at the time. The seam itself **is** genuinely proven across a process boundary, by `TGD0-06` and `SD0-01`. `L-C12` is the failure mode here and it has eight instances in this register: a contract, a taxonomy and a stub, all beautifully typed, that nothing produces or consumes. → `SMOKE-PROVEN` *(`TGD0-08`, gate-proven — **not** falsified. A reference implementation answering all four operations, and a conformance suite that **imports no test framework** so the AutoDesign team can run it in their own harness against their own stand-in. ⚠ Its no-fire half is the half that matters: it proves an implementation *cannot* claim an unmeasured fidelity score, emit an unevidenced `partial`, return output on a `held`, parse an unknown schema version, or silently ignore an undeclared field)*
- [x] **PD0-07** 🔴 **The contract states who executes.** *(razi-ruled 2026-09-03, `D3`.)* Across the whole of `PD1`–`PD6` the words *personal assistant*, *PA*, *fleet*, *worker* and *supervisor* appear **zero times** — the phases define capabilities and never say who drives them, which left AutoDesign to infer it. **The ruling: the PA executes design work in Rollout 1**, refined until it is ready, and **the fleet becomes an additional consumer in Rollout 2** — running it through the fleet now would be, in Razi's words, *"pretty confusing and hard to debug"*. So: `adapt` stays deterministic (`PD4-02`), the **PA** chooses parameters and adjudicates a `partial` result, and nothing in Rollout 1 depends on a working fleet. ⚠ This note said `degraded` when written on 2026-09-03 — a paraphrase of Razi's ruling that reached for the retired vocabulary; the outcome triple `PD0-04` froze is `complete` · `partial` · `held` (`A18` `S-20`). Write it into the contract so the consumer does not have to guess. → `DOC-GATE` *(⚠ **This note claimed §1 and §3 and the document said none of it — `W-1`, the finding that should have blocked the phase's closure.** A `grep -ci` for *personal assistant*, *PA*, *fleet* and *supervisor* across the contract document AND the code returned **0**: a checkbox flipped with no deliverable of any kind, and a completion note describing work that was never done. It is now `§3.5 · Who executes, on our side`, written 2026-09-03 as part of the audit remediation. What it says, and now actually says: the PA executes design work in Rollout 1, the fleet becomes an additional consumer in Rollout 2, and approval stays AutoDesign's to enforce before they spawn us. ⚠ The document says out loud that our single `change` erases an approval gradient their manifest marks load-bearing — we do not re-implement it and we do not pretend the difference is ours to erase)*

- [x] **PD0-08** 🔴 **The sidecar invocation model is defined** *(razi-ruled 2026-09-03)*. AutoDesign spawns AutoDev as a child process, the way it already spawns its own sidecar. So the contract states: **how it is invoked** (executable, argv shape), **how a request and its response cross** (stdio, one JSON object per line), **how progress and terminal status are signalled**, **how a job is cancelled** (the parent kills the child, so every operation must be safely interruptible), and **what the exit code means** independently of the payload. ⚠ No socket, no port, no listener — that is the point of the ruling. → `UNIT-PROVEN` *(`TGD0-06`, falsified. `npm run build:sidecar` emits a Node entry point; AutoDesign spawns it, writes one JSON request to stdin, reads NDJSON events off stdout. Exit codes are enumerated and mean something independent of the payload — ⚠ **`0` does not mean "it converted"**, it means "we answered", and a `held` refusal exits `0` because the invocation worked and the answer was no. Cancellation is the parent killing the child, and on a signal we exit **without** a terminal event, because a `settled` for unfinished work is worse than silence. ⚠ Its tsconfig omits the DOM lib deliberately: the existing PSD reader is browser-bound, so this build **fails** rather than emitting a file that dies at runtime)*
- [x] **PD0-09** 🔴 **One public-facing document for the AutoDesign team** *(razi-ruled)*, in `docs/restructurization/public-facing/`: the four operations, the payload, the error taxonomy, the asset rules, the invocation model, and a worked example. ⚠ **One example, for `adapt`** — the task text said one per operation and the phase shipped one. `export` is the gap that matters, since it is the only operation whose settled event carries `artifacts` and a null fidelity. Owed by `PD1`, which is the first phase to produce a real one. Written for a reader on the other side of the seam who cannot ask us questions — they own their half and are three phases from calling us, so the document is the whole of the interface until then. Governed by `§0.17`, so it is refreshed at every later design-phase closeout rather than written once. → `DOC-GATE` *(`TGD0-07`, falsified — and the falsification found the gate blind first. `AUTODESIGN-CAPABILITY-CONTRACT.md`: the four operations, the mapping table to their vocabulary, the reference payload, the outcome triple, fidelity, assets, a worked example, and **§9 · what is not settled yet**. ⚠ That last section names the things we will not pretend about: real conversion needs a canvas backend we have not taken on, and the "25–35 s" figure is a real measurement **of the old browser converter** — the IR path's cost has never been measured, so we do not quote it as our latency)*
- [x] **PD0-10** **Open the `NOVELTY_LEDGER` row for the contract.** Writing to another product's versioned contract shape is an externally-inspired mechanic, and the register held **twenty** rows and **none for the design programme** *(present tense until 2026-09-03: it holds **21**, and `N-21` is the row this very task opened — `A18` `S-27`)* — though `PD1` is grounded in an external converter and `PD2`/`PD4` now carry ~33 tasks derived from an external adaptation engine. Record what we adopt from AutoDesign's shape, what we deliberately do not, and the gate that proves it. ⚠ Flag the `PD1`/`PD2`/`PD4` rows as owed by their own phases. → `DOC-GATE` *(The `NOVELTY_LEDGER` row for the contract shape, opening the register for the design programme — which had **twenty rows and none for design** — ⚠ the completion note first said *six*, a figure taken from a `grep -c "^| "` that counted table rows inside entries rather than entries, then copied into three governance documents unchecked. `§0.2L-3`: search for the string, do not sample, while `PD1` is grounded in an external converter and `PD2`/`PD4` now carry ~33 tasks derived from an external adaptation engine. ⚠ `PD1`/`PD2`/`PD4` rows recorded as owed by their own phases)*

**Smoke — SD0**
- [x] **SD0-01** A caller invokes all four operations against the reference implementation **as a spawned child process** and receives well-formed, schema-valid responses — plus the no-fire half: a **malformed request is refused** with the taxonomy's `refused`, a payload carrying an **unknown schema version is rejected rather than parsed**, and a **killed child leaves no partial output claiming success**. ⚠ Sharpened deliberately: "receives well-formed responses" is satisfiable by a function that returns a constant, which is `L-C9` — a check whose shape cannot express failure. *(**`SMOKE-PROVEN` 2026-09-03** — the sidecar spawned as a real child process, ~~nine~~ **fourteen** cases. ⚠ Said *nine* until 2026-09-03 (`A18` `S-25`): the nine-case version is the one `W-2` found could not be reproduced from the repository at all. The committed smoke runs four operations, **four holds**, five malformed requests and **a killed child** — the two additions being exactly what the audit said was missing. All four operations settle with exit `0`, four events each, terminal event last and decodable. All five malformed inputs — bad JSON, an unknown verb, the reserved verb, an unknown schema version, an undeclared field — exit `2` with **nothing on stdout**, which is the half that matters: a consumer parsing stdout line-by-line must never see half an answer to a request we refused.)*

---

**Exit.** `SD0` → `TGD0` → matrix → progress → no-stale → commit. *(Named `TG-PD0` until 2026-09-03; there is no such family — the gates are `TGD0-01`…`TGD0-10`. `A18` `S-28`.)*

⚠ **Closed on the second attempt, 2026-09-03.** The first closure ran five of
those six steps and skipped `§0.2a`'s two auditors entirely, which is a `§0.2b`
violation on its own; Razi caught it. The audits returned **45 findings**, three
of which should have blocked the closure. Razi's ruling put all 45 in-phase:
*"we fix all of the findings found by the audit … findings found by the audit is
also part of the phase, so it is a part of `PD0`."*

🔴 **The claim "all 45 are closed" is checked, not asserted** —
`_verify/PD0-FINDINGS-LEDGER.py` runs one check per finding and **parses the
finding ids out of the two audit reports** rather than listing them, so a
finding I forgot fails the ledger the same way a finding I broke does. Both
failure modes were falsified. `python docs/restructurization/development/_verify/PD0-FINDINGS-LEDGER.py`
→ `45 findings, 0 open — ALL CLOSED`, exit `0`.

**Final state:** 10 gates (`TGD0-01…10`, 6 falsified) · 118 JS tests · 611 Rust
tests · `SD0-01` re-runnable as `npm run smoke:design`, 14 spawned cases ·
`tsc` and `eslint` clean · `ADR-LOG-RESTRUCT-0035` · `§0.16 L-35`…`L-38`.

⚠ **PD0 had no exit line, no gate section and no matrix row.** The exit line and the gate section (`TESTING_GATES.md §6.5`) were supplied on 2026-09-03; **the matrix row still does not exist**, and this sentence implied all three were fixed (`A18` `S-26`). `READINESS_MATRIX.md`'s scored areas are still `A0`–`A9` against `P0`–`P9`, so `PD0` closing moved the headline by **zero** — which that document now records twice, as a finding rather than a rounding. Adding an area means re-weighting the ten that sum to 100, which changes the headline figure, so it is **Razi's call: flagged, not taken.** —
none of the design phases did, which is why `§0.5`'s *"smoke-then-gates is
mandatory"* was pointing at nothing for a whole rollout. `PD1`–`PD6` owe theirs.

---

## §10.5.1 · PD1 · Ingestion — PSD **and** PDF

**Goal.** Either source becomes one IR that describes the design exactly.
**Depends on.** P8 (handover-clean tree). **Source.** `psdconversions` v4.7 pipeline.

### PD1-A · The PSD path *(four tasks already complete, carried from P3-G)*
- [x] **PD1-01** `ag-psd` pinned at exactly `30.1.0` and bundled locally; **no CDN script survives** the renderer CSP (`script-src 'self' blob:`). *(was `P3-45`)* → `GATE-PROVEN`
- [x] **PD1-02** The layer IR: bounds, fonts, colours, tracking, `styleRuns`, blend modes, opacity, group nesting — with **no pixels**, enforced by `assertNoPixels`. *(was `P3-46`)* → `UNIT-PROVEN`
- [x] **PD1-03** PSD's `U+0003` point-text separator becomes a line break. *(was `P3-47`)* → `UNIT-PROVEN`
- [x] **PD1-04** An unresolvable font is **reported**, never silently substituted. *(was `P3-48`)* → `UNIT-PROVEN`
- [ ] **PD1-05** **Shape layers keep their geometry.** `vectorMask` Bezier knots enter the IR as path data, not as a rasterized preview — rasterizing here forfeits SVG export (`PD3-09`) and shape edits (`PD5-06`) permanently. → `UNIT-PROVEN`
- [ ] **PD1-06** **Layer effects are recorded even when CSS cannot express them** — drop shadow, inner shadow, stroke, outer glow, gradient overlay, bevel. The IR records what the design *has*; the serializer decides what it can *render*. Collapsing that distinction at ingestion is unrecoverable. → `UNIT-PROVEN`
- [ ] **PD1-07** Clipping masks and clip paths enter the IR, including circular image clips. → `UNIT-PROVEN`
- [ ] **PD1-08** Smart objects and adjustment layers are recorded with a **`rasterize-required`** marker rather than dropped. → `UNIT-PROVEN`

### PD1-B · The PDF path *(ported from the v4.7 four-phase pipeline)*
- [ ] **PD1-09** **Phase 1** — operator-list segmentation (`segmentOperatorList`, `flushSubGroup`): walk the raw drawing commands and emit sub-groups. The pipeline is built on the operator list rather than text/image extraction, and that choice is what makes native shape layers possible at all. → `UNIT-PROVEN`
- [ ] **PD1-10** **Per-draw-op CTM tracking.** `pixelCoord = viewportTransform × pageTransform × elementTransform × localCoord`. Every draw op carries its own `ctmAtDraw`. → `UNIT-PROVEN`
- [ ] **PD1-11** **Phase 2** — group classification and bounds (`classifyAllGroups`, `computeGroupBounds`), with clip-companion and spatial-overlap merging. → `UNIT-PROVEN`
- [ ] **PD1-12** **Phase 3A** — shape-layer creation (`shouldUseShapeLayer`, `pdfPathToPsdVectorMask`, `createShapeLayer`). Knots in `[x,y]` order per the v4.5 correction; rectangular clips are **never** used as a `vectorMask` (they are content-area bounds, not shapes). → `UNIT-PROVEN`
- [ ] **PD1-13** **Phase 3B** — rasterization fallback (`rasterizeForPreview`, `rasterizeVectorGroup`) for mixed content and non-solid fills. → `UNIT-PROVEN`
- [ ] **PD1-14** **Phase 4** — text assembly with the **ten merge guards** (`mergeBlocks`): word coalescing and paragraph merge without crossing columns. → `UNIT-PROVEN`
- [ ] **PD1-15** Image extraction via the CTM-tracked operator walk (`extractPageImages`). → `UNIT-PROVEN`
- [ ] **PD1-16** **Position-correlated text colour** — colours matched to text by spatial proximity in the operator stream, not by index. The v4.5 change exists because index-based matching silently mis-coloured text. → `UNIT-PROVEN`

### PD1-C · Shared
- [ ] **PD1-17** **The seven guards are ported with their reasons**: canvas cap at 15,000 px, PSD limit at 30,000 px, CMYK detection (output is RGB — say so), image dedup under 5 px, full-page image skip above 90%, tiny image skip under 30 px, shape-layer fallback on mixed content. → `GATE-PROVEN`
- [ ] **PD1-18** **One schema from either source.** A PSD-derived IR and a PDF-derived IR are the same type, and every downstream phase is written once. → `GATE-PROVEN`

**Smoke — SD1**
- [ ] **SD1-01** A real PSD converts inside AutoDev with **no network fetch** and the render matches the source.
- [ ] **SD1-02** A real PDF converts to the same IR type, and to a PSD Photoshop opens without repair.
- [ ] **SD1-03** A design with Bezier shapes keeps them as paths — the IR contains knots, not a bitmap.
- [ ] **SD1-04** A CMYK PDF is detected and the RGB conversion is **reported**, not silent.

---


> ### 📐 Grounded in the AAE post-mortem
>
> The organisation already built a design **Adaptive Adjustment Engine** —
> `AIRS Microservices/mcp-server-adobe/src/aae` — and it reached production
> against a real 5-PSD × 3-size matrix. Its full reading is in
> `_verify/A12-aae-adaptation-reference.md`; the tasks below are traced to it
> line by line, and razi-ruled into this plan on 2026-09-03.
>
> **The single most important thing it teaches:** the engine has **no measured
> fidelity anywhere**. There is no rasterisation and no comparison — every
> "score" is a penalty tally over the same boxes the engine just produced. Its
> correctness checker is complete, correct, was marked RESOLVED in its own
> audit, and **is imported by nothing**. Both are shapes this repo knows.
>
> ⚠ Its three root-level audit reports describe an engine that no longer
> exists. The real record is `docs/AAEAuditbyRazi/` — the owner's own June run.

### PD1-D · IR vocabulary the AAE post-mortem paid for

- [ ] **PD1-19** `Frame` carries an **affine transform, not just an AABB**. AAE's `Box{left,top,width,height}` is right for layout maths and silently drops `rotate` — carried in the type and read by no transform. → `UNIT-PROVEN`
- [ ] **PD1-20** A **closed `Role` enum with no aliases**, plus a versioned translation at ingest. Seven aliases caused a live bug where code branching on `'decor'` never matched `'decoration'`. → `UNIT-PROVEN`
- [ ] **PD1-21** Role assignment is `{value, confidence, method, reasons}` — AAE already had this shape and only ever read `value`, so provenance was designed and thrown away. → `UNIT-PROVEN`
- [ ] **PD1-22** 🔴 **Hierarchy is real: parent transforms compose onto children at parse time.** AAE's flat `[groupNode, child, child]` model is the direct cause of its **#1 production defect** — group/folder desync. → `UNIT-PROVEN`
- [ ] **PD1-23** Text is **runs with measurable metrics** (family, size, leading, tracking, per run). AAE had a single style over a single string, which made its `TextFitResult{overflow,truncated,needsBreak}` a dead type because nothing ever measured text. → `UNIT-PROVEN`
- [ ] **PD1-24** Asset **kind** (vector / raster / text / shape) is carried explicitly from ingest. AAE guessed "vector" as *"≤1 smartObject"* — a Photoshop proxy that does not exist here. → `UNIT-PROVEN`
- [ ] **PD1-25** Port `UnitValue` / `SpacingSpec` / `resolveUnit` / `resolveAsFraction` **essentially verbatim**, resolving only at the policy boundary. The cleanest module in that engine, and its px-only-inside rule held up. → `UNIT-PROVEN`


## §10.5.2 · PD2 · Fidelity — measured, not asserted

**Goal.** *"Visually it looks okay but in reality pixel by pixel wise is not, so we need a
tool here that can detect that and do refinements."* **Depends on.** PD1.

**Why this is its own phase.** Ingestion is not finished when it looks right; it is finished
when something can **prove** it is right and **say where** it is not. The P1 spike is the
cautionary case: the render looked correct, and only a per-value trace revealed a control
character rendering as a glyph box and two fonts silently replaced.

- [ ] **PD2-01** **Composite renderer** — the IR rasterized at native resolution, deterministically. → `UNIT-PROVEN`
- [ ] **PD2-02** **Reference raster** — the source's own composite (PSD flattened image, or PDF page render at matched DPI). → `UNIT-PROVEN`
- [ ] **PD2-03** **Per-pixel difference**, reported as a percentage of pixels outside tolerance and as a maximum channel deviation. → `UNIT-PROVEN`
- [ ] **PD2-04** **SSIM** over the whole canvas, because a small number of badly wrong pixels and a large number of slightly wrong ones are different defects and per-pixel alone cannot tell them apart. → `UNIT-PROVEN`
- [ ] **PD2-05** **Per-layer maximum deviation** — the metric that localises. A whole-canvas score says the conversion is 97% right; this says *which layer* owns the other 3%. → `UNIT-PROVEN`
- [ ] **PD2-06** **Thresholds are declared numbers**, per metric and per layer kind, committed to the repository. A threshold argued at review time is not a gate. → `DOC-GATE`
- [ ] **PD2-07** **Text carries its own tolerance.** Browser and Photoshop rasterisers differ on anti-aliasing and sub-pixel kerning by design; holding text to the shape threshold would fail every correct conversion, and a gate that fires on correct output gets switched off. → `UNIT-PROVEN`
- [ ] **PD2-08** The report **names the worst layers in order**, with their metric and their kind. → `UNIT-PROVEN`
- [ ] **PD2-09** **Cause localisation** — a failing layer is attributed to a candidate cause: font substitution, blend-mode fallback, effect rasterisation, transform error, colour-space shift. → `UNIT-PROVEN`
- [ ] **PD2-10** **Refinement proposes and applies a correction**, then re-measures. → `SMOKE-PROVEN`
- [ ] **PD2-11** **The refinement loop terminates** — bounded iterations, and it stops when a round fails to improve. An unbounded "keep fixing" loop on a 25 s conversion is an unbounded cost. → `GATE-PROVEN`
- [ ] **PD2-12** A **regression corpus** of real designs — the `gamead` family (1080×1080, 1080×1920, 1920×1080), plus a text-heavy, an effect-heavy and a vector-heavy case. → `DOC-GATE`
- [ ] **PD2-13** **The gate fails on a planted regression.** Falsification is the acceptance criterion for this phase, not an extra. → `GATE-PROVEN`
- [ ] **PD2-14** **Fidelity travels with the result.** Every ingest and every export returns its score; a caller can refuse a conversion whose fidelity is too low. ⚠ Said *"a degraded conversion"* until 2026-09-03: `degraded` is **not** an outcome this contract can emit — `PD0-04` froze `complete` · `partial` · `held` and retired the `refused`/`failed`/`degraded` triple, because AutoDesign's `lifecycle.ts:80-85` rejects any result carrying both output and an error (`A18` `S-20`). Measuring fidelity and then not returning it repeats `L-C12` — built, proven, wired to nothing. → `GATE-PROVEN`

**Smoke — SD2**
- [ ] **SD2-01** Every design in the corpus converts and scores above its declared thresholds.
- [ ] **SD2-02** A deliberately broken conversion (font forced to fallback) is **caught**, and the report names the affected layers.
- [ ] **SD2-03** The refinement loop improves a failing case and terminates.

---


### PD2-D · What "measured, not asserted" has to mean *(AAE post-mortem)*

> 🔴 **AAE never measured fidelity and did not notice.** This group exists so
> this phase's title stays true.

- [ ] **PD2-15** 🔴 **Build a renderer; fidelity is a measured comparison of rendered source against rendered output.** There is no rasterisation anywhere in AAE — its pipeline audit lists PNG generation as missing — so every score it reports is a penalty tally over its own output. → `BENCH-PROVEN`
- [ ] **PD2-16** **Normalise every before/after metric against the intended resize.** Raw-pixel comparison made *everything* read as moved and scaled; comparing centre fractions with a relative epsilon is what gave the metric signal. → `UNIT-PROVEN`
- [ ] **PD2-17** Every invariant has a **typed identity** shared by emitter and analyser — no string matching. Three of six AAE refinement branches key on violation strings its compiler never emits, so its gap multiplier is permanently 1.0. → `UNIT-PROVEN`
- [ ] **PD2-18** **Overlap is a real, role-aware invariant** with an IoU or occlusion measure. AAE declares `maxPrimaryOverlap` and checks it nowhere; the role-aware version lives only in the file nothing imports. → `UNIT-PROVEN`
- [ ] **PD2-19** **Re-verify the geometry that actually ships**, after every post-hoc adjustment. AAE's safe-zone clamping runs *after* selection and *after* verification, and its residuals never enter the reported violations. → `GATE-PROVEN`
- [ ] **PD2-20** 🔴 **The refinement loop may not mutate its own thresholds.** AAE's `adjust_constraints` lowers the minimum font and CTA sizes and raises the maximum logo crop until the same layout scores clean. Relaxation must be a caller-authorised, recorded input. → `GATE-PROVEN`
- [ ] **PD2-21** **Select by (hard-constraint satisfaction, then a real-valued quality score)** — not by counting violations. AAE sorts on `(violationCount, errorCount, seedIndex)`, so *"logo 3% small"* and *"hero fully off-canvas"* weigh exactly the same. → `UNIT-PROVEN`
- [ ] **PD2-22** **Keep best-so-far across refinement and never return worse than an earlier iteration.** AAE added this late, precisely because refinement returned the *last* result rather than the best. → `UNIT-PROVEN`
- [ ] **PD2-23** **Protected sub-regions** (faces, product bounds, focal points) are modelled and crop is checked against them. Still open in AAE: *"'a character is cut' passes as long as the overall area loss is under the role threshold."* → `UNIT-PROVEN`
- [ ] **PD2-24** **Real text shaping** — measure, line-break, detect overflow, and make `wrap` mean something. In AAE `wrap` and `preserve` are identical no-ops, so its whole text-wrap vocabulary is a lie. → `UNIT-PROVEN`
- [ ] **PD2-25** A **legibility invariant beyond a pixel floor** — contrast against the composited backdrop, minimum rendered x-height. AAE's only legibility check is `fontSize >= 18`, applied to a size the engine itself just chose. → `UNIT-PROVEN`
- [ ] **PD2-26** **The offline round-trip harness, built first.** Re-execute the emitted artefact under the consumer's semantics and assert it reproduces the intended layout. ⚠ AAE's equivalent is *the only check in that repo that ever caught anything*, and it needed no vendor API. → `GATE-PROVEN`
- [ ] **PD2-27** **An end-to-end run on real assets across the full target matrix before this phase closes.** AAE's 5-PSD × 3-size run **reordered its entire defect priority list** and surfaced five findings that three prior desk audits had all missed. → `SMOKE-PROVEN`


## §10.5.3 · PD3 · Export — webpage, PDF, PSD

**Goal.** The IR becomes any of the three outputs. **Depends on.** PD2.

- [ ] **PD3-01** **IR → standalone HTML**, at parity with the source converter. → `SMOKE-PROVEN`
- [ ] **PD3-02** Assets resolve from the store at serialization time; the IR still carries none. → `GATE-PROVEN`
- [ ] **PD3-03** Google Fonts are linked **only** for families that resolve; nothing else is requested. → `UNIT-PROVEN`
- [ ] **PD3-04** Unresolved fonts appear in the **export result**, not only in a log. → `GATE-PROVEN`
- [ ] **PD3-05** **IR → TypeScript/React components.** → `SMOKE-PROVEN`
- [ ] **PD3-06** **The semantic escape hatch is bounded by evidence, not taste.** Razi: *"semantic components that fills in where HTML and CSS cannot handle it with the code limitations."* The eligible set is exactly the source's own **⚠ Approximate** and **❌ Not Supported** rows — Dissolve, Subtract, Divide, Hard Mix, bevel/emboss/satin, pattern overlays, adjustment layers. A component is written **only** for a row on that list. → `DOC-GATE` · `GATE-PROVEN`
- [ ] **PD3-07** The **16 CSS-native blend modes** map 1:1; the four with no equivalent rasterize, and say so. → `UNIT-PROVEN`
- [ ] **PD3-08** Layer effects → CSS where expressible (`filter: drop-shadow`, `box-shadow`, `box-shadow: inset`, gradients); rasterize otherwise, and record which. → `UNIT-PROVEN`
- [ ] **PD3-09** Bezier shapes export as SVG `<path>`, not as bitmaps. → `UNIT-PROVEN`
- [ ] **PD3-10** Clip paths export as SVG `<clipPath>`. → `UNIT-PROVEN`
- [ ] **PD3-11** **IR → PDF.** → `SMOKE-PROVEN`
- [ ] **PD3-12** **IR → PSD** via `writePsd` — shape layers with `vectorMask`/`vectorFill`, editable type layers, pixel layers. This is the construction path the source pipeline already performs; `PD5` reverses it. → `SMOKE-PROVEN`
- [ ] **PD3-13** **Round-trip.** PSD → IR → PSD preserves layer count, bounds, text content and style runs. The strongest single check that the IR lost nothing. → `GATE-PROVEN`
- [ ] **PD3-14** Every export is **graded by PD2** before it is returned. → `GATE-PROVEN`
- [ ] **PD3-15** Export size is bounded and reported; a 25 MB result is a fact the caller learns before receiving it. → `UNIT-PROVEN`

**Smoke — SD3**
- [ ] **SD3-01** One IR exports to all three formats and each scores above threshold.
- [ ] **SD3-02** A PSD survives the round trip with its text still editable in Photoshop.
- [ ] **SD3-03** A design using an unsupported blend mode exports correctly **and** reports the rasterisation.

---

## §10.5.4 · PD4 · Adaptation — a deterministic tool

**Goal.** Reformat a design to a target size and ratio. **Depends on.** PD3.

**Doctrine ruling (Razi).** *"The entire tool does the calculation and stuff and the
LLM/AutoDev fleet here is only the executor of that tool."* Adaptation geometry is
**code**. The fleet chooses *when* to adapt and to *what*; it never computes the layout.
This sits on the deterministic side of the master plan's PART 2 split.

- [ ] **PD4-01** Tool signature: target width, target height, and a ratio policy (`contain` · `cover` · `stretch-safe`). → `UNIT-PROVEN`
- [ ] **PD4-02** **No model call inside the tool.** Enforced as a gate, not a convention. → `GATE-PROVEN`
- [ ] **PD4-03** **Per-layer anchor model** — each layer resolves to an anchor (corner, edge, centre) derived from its position and its group. → `UNIT-PROVEN`
- [ ] **PD4-04** **Safe areas** — margins that content must not cross in the target format. → `UNIT-PROVEN`
- [ ] **PD4-05** Scale computation per layer kind: images cover, shapes scale, text rescales by its own rule. → `UNIT-PROVEN`
- [ ] **PD4-06** Reposition computation honouring anchors and safe areas. → `UNIT-PROVEN`
- [ ] **PD4-07** **Text reflow.** A headline that fits 1920 wide does not fit 1080 wide; the tool adjusts size or wraps, and reports which. → `UNIT-PROVEN`
- [ ] **PD4-08** Hero/background fill strategy when the aspect ratio changes — extend, re-crop by focal point, or letterbox, stated per layer. → `UNIT-PROVEN`
- [ ] **PD4-09** **No layer silently leaves the canvas.** Anything that cannot be placed is reported. Silent loss is the failure mode this phase is most exposed to: it looks like a clean adaptation. → `GATE-PROVEN`
- [ ] **PD4-10** Group-relative transforms survive — children move with their parent. → `UNIT-PROVEN`
- [ ] **PD4-11** The fleet invokes it as a **tool** and cannot reimplement its geometry. → `GATE-PROVEN`
- [ ] **PD4-12** Every adaptation is graded by PD2 against the source. → `GATE-PROVEN`
- [ ] **PD4-13** Presets for the formats actually in use: 1080×1080, 1080×1920, 1920×1080. → `UNIT-PROVEN`

**Smoke — SD4**
- [ ] **SD4-01** 1080×1080 → 1080×1920: every visible layer lands inside the canvas.
- [ ] **SD4-02** 1920×1080 → 1080×1080: text reflows and the reflow is reported.
- [ ] **SD4-03** The same input and target produce byte-identical output twice (determinism).

---


### PD4-B · Layout intent *(AAE post-mortem)*

- [ ] **PD4-14** **Centre-fraction preservation with uniform `min(sx,sy)` scale** is the baseline transform. Every other rule in AAE is a correction on top of this one. → `UNIT-PROVEN`
- [ ] **PD4-15** `cover` with **centre-offset correction** for full-bleed elements — naive cover drifts the focal point. → `UNIT-PROVEN`
- [ ] **PD4-16** **Nearest-source-corner detection and corner-anchored placement.** The highest-value reconstructed-intent heuristic in that engine, and the only thing keeping corner elements in their corners. → `UNIT-PROVEN`
- [ ] **PD4-17** 🔴 **Coupling is a first-class IR relation** — *"this element's frame derives from that one's"* — used for background / overlay / pattern. AAE documented the overlay as "coupled with bg" and **never implemented the coupling**, producing a visible dark seam across all five test assets. → `UNIT-PROVEN`
- [ ] **PD4-18** **Named groups are rigid units**: transform the unit, then expand to members. AAE's rank-1 fix; unit-first ordering took one test from 210 px of error to 0. → `UNIT-PROVEN`
- [ ] **PD4-19** **Exclude full-bleed roles from proximity clustering** before it runs. A canvas-spanning background absorbed logos and overlays into one cluster and erased their transforms. → `UNIT-PROVEN`
- [ ] **PD4-20** **Every layout threshold is a canvas fraction or a physical unit — bare-pixel constants are banned.** AAE's `PROXIMITY_THRESHOLD=50`, `MAX_CLUSTER_DIAMETER=500`, nudge cap `50` and `minCtaSize=120` mean entirely different things at 640×480 and at 4096×4096. → `GATE-PROVEN`
- [ ] **PD4-21** Adopt the **canvas-relative gap scale** (tight 2% / normal 4% / loose 8%, axis-bound) — the one spacing decision in AAE that scaled correctly. → `UNIT-PROVEN`
- [ ] **PD4-22** 🔴 **Per-ratio composition templates**: a target size resolves to named regions and roles bind to regions. Still open in AAE and the reason for its most visible failure — *"the recompose path is a handful of anchor-strategy seeds, not true per-ratio layout templates … a landscape target ends up as a rescaled square."* → `UNIT-PROVEN`
- [ ] **PD4-23** The aspect-ratio mode selector is **direction-symmetric**. Verified in AAE: `0.5 → 1.0` selects `recompose` while `1.0 → 0.5` selects `reflow`, because the selector divides by the source ratio. → `UNIT-PROVEN`
- [ ] **PD4-24** **Background ignores the safe zone; only foreground respects it** — stated as an explicit rule. Still open in AAE: a safe-zone guide was read as *"shrink everything"*, producing white margins instead of a full-bleed background with inset foreground. → `UNIT-PROVEN`

### PD4-C · One compiler, one truth *(AAE post-mortem)*

- [ ] **PD4-25** 🔴 **Exactly one `compile(plan, scene) -> Layout`**, called by the planner, the checker and the emitter alike. AAE's overlap resolver is a **second, partial geometry evaluator** that returns *source-space* boxes for six of nine transform kinds. → `GATE-PROVEN`
- [ ] **PD4-26** **Every transform kind is total — no silent fallback to `preserve`.** Two kinds fell back for months while emitting a warning nobody actioned; an unimplemented arm should fail to compile. → `UNIT-PROVEN`
- [ ] **PD4-27** **Uniform scale by default** when mapping a group frame onto members; anisotropic scaling is opt-in and checked. AAE applies independent `scaleX`/`scaleY`, so any non-uniform group frame stretches its children with nothing to catch it. → `UNIT-PROVEN`
- [ ] **PD4-28** **Shrink allowances are linear ratios, never area ratios** — *"a logo at 0.9 linear is a 10% crop, not 19%"*. A bug AAE had to fix in two copies of the same function. → `UNIT-PROVEN`
- [ ] **PD4-29** **Upscale caps are applied, not merely computed**: raster never above 1.0×, vector to an explicit cap. In AAE the caps were computed and never applied, and a raster logo was verified upscaling 4×. → `UNIT-PROVEN`
- [ ] **PD4-30** 🔴 **A scale parameter means one thing on every code path.** AAE's `heroScale` was a *replacement* on one path and a *multiplier* on another — a verified 2× difference for the same nominal value, giving the same hero a different size at each target size. → `GATE-PROVEN`
- [ ] **PD4-31** **Clamp non-full-bleed elements inside the canvas at the unit level**, before member expansion, and surface the clamp. AAE's rank-2 fix; took off-canvas elements from nine per test to at most one, and cured *"logo missing"*. → `UNIT-PROVEN`
- [ ] **PD4-32** **A dead-code / unreachable-symbol gate for this subsystem.** AAE's checker is complete, correct, marked RESOLVED in its own audit, and imported by nothing; its candidate scorer is ~500 dead lines; its constraint struct has eight fields and one reader. ⚠ Same family as `L-C12`, which this repo has hit eight times. → `GATE-PROVEN`
- [ ] **PD4-33** **No write-only fields.** If a struct field is never read, it is deleted. AAE carries at least eight, including a hardcoded `cost = 0` that its report presents as a computed figure. → `GATE-PROVEN`


## §10.5.5 · PD5 · Changes

**Goal.** Change what a design contains. **Depends on.** PD3.

**Grounding (Razi).** *"There is none … the psdconversion repo has defined a way to create
PSD from scratch and HTML CSS design from a PSD, if we can build it from scratch then we
can tune and reverse engineer it as well to build for changes."* Confirmed in the source:
the pipeline constructs shape layers (`vectorMask` + `vectorFill`), editable type layers
and pixel layers from nothing. **Construction reversed is mutation.**

- [ ] **PD5-01** The change primitives: text · image · position · size · colour · shape geometry · visibility. → `UNIT-PROVEN`
- [ ] **PD5-02** **A text change preserves `styleRuns`.** The spike found a layer whose three runs alternate white and yellow; a naive replacement flattens it to one colour and nothing reports the loss. → `GATE-PROVEN`
- [ ] **PD5-03** An image change preserves fit and crop intent. → `UNIT-PROVEN`
- [ ] **PD5-04** Position and size changes, respecting group transforms. → `UNIT-PROVEN`
- [ ] **PD5-05** Colour changes across fill, stroke and gradient stops. → `UNIT-PROVEN`
- [ ] **PD5-06** Shape geometry changes on Bezier knots — available only because `PD1-05` refused to rasterize. → `UNIT-PROVEN`
- [ ] **PD5-07** Every change is an **IR mutation and is reversible**; the inverse is computed, not remembered. → `UNIT-PROVEN`
- [ ] **PD5-08** A **change set** is a value, applied atomically — all or none. Half-applied is worse than refused. → `UNIT-PROVEN`
- [ ] **PD5-09** Changes write back to **PSD** through the construction path. → `SMOKE-PROVEN`
- [ ] **PD5-10** Changes write back to HTML and PDF. → `SMOKE-PROVEN`
- [ ] **PD5-11** **A change never silently drops a layer property.** Anything the mutation cannot carry is reported. → `GATE-PROVEN`
- [ ] **PD5-12** Changed output is graded by PD2 — a change must not degrade untouched layers. → `GATE-PROVEN`
- [ ] **PD5-13** The fleet invokes changes as tools. → `GATE-PROVEN`

**Smoke — SD5**
- [ ] **SD5-01** Text is replaced on a multi-run layer and every run's colour survives.
- [ ] **SD5-02** A change is applied, written to PSD, reopened, and the change is present.
- [ ] **SD5-03** A change set that cannot fully apply is refused whole, leaving the IR untouched.

---

## §10.5.6 · PD6 · Brand guideline

**Goal.** Ingest a brand guide and apply it. **Depends on.** PD5.

**Why it follows PD5.** A rebrand is a batch of changes driven by a document. Building it
before the change primitives would mean building those primitives twice.

**Grounding (researched 2026-09-01, razi-ruled: *"search the web for an example brand guide and we use it, i dont have one right now"*).**

Real published guides were surveyed to derive the schema rather than inventing one. The
[openSUSE brand guidelines](https://opensuse.github.io/branding-guidelines/) were readable end
to end and are the **reference fixture**; NASA, Uber, Slack, Netflix and Oatly were surveyed for
the field set. What a guide actually specifies:

| Section | Fields a machine-readable guide must carry |
|---|---|
| **Colour** | named entries, each in several spaces — `hex` and `rgb` (digital), `cmyk`, `pantone_coated`/`pantone_uncoated` (print), `grayscale_percent` (mono). Roles: primary, secondary, accent — **plus tints**, which openSUSE carries four of per colour |
| **Typography** | `family` · `weight` (Light/Regular/SemiBold/Bold) · **`role`** — titles, taglines, body. The role is the load-bearing field: a rebrand maps *roles*, not font names, or a display face lands in body copy |
| **Logo** | variants (horizontal, stacked, mark-only), clear space, minimum size, permitted backgrounds |
| **Misuse** | explicit do-not rules — the half that makes a guide checkable rather than decorative |

⚠ **Two findings that shape the format.** First, real guides are **incomplete**: openSUSE's
own document specifies no logo variants, no minimum sizes and no clear-space rules, and gives
colours only as an image. A format that requires every field will reject most real guides, so
**every field is optional and absence is reported, never inferred** (`PD6-08`). Second, guides
ship as **PDFs** — so `PD1`'s PDF ingestion is the natural intake path, and the two capabilities
compose rather than duplicating each other.

- [ ] **PD6-01** **The brand-guide format** — a real input, not a prompt string. AutoDesign's premise is that findings *"cite the brand guide they came from"*, which free text cannot support. → `DOC-GATE`
- [ ] **PD6-02** Ingest a guide into a typed value. → `UNIT-PROVEN`
- [ ] **PD6-03** Palette mapping: source colours → brand colours, including near-matches. → `UNIT-PROVEN`
- [ ] **PD6-04** Type mapping: source families → brand families, reusing `PD1-04`'s resolution reporting. → `UNIT-PROVEN`
- [ ] **PD6-05** Logo and asset substitution. → `UNIT-PROVEN`
- [ ] **PD6-06** A rebrand **compiles to a `PD5` change set** and is applied by the same path. → `GATE-PROVEN`
- [ ] **PD6-07** **Every applied rule cites the guide rule it came from.** This is the whole reason AutoDesign wants a guide rather than an instruction. → `GATE-PROVEN`
- [ ] **PD6-08** An unmappable rule is **reported, never guessed**. A plausible invented colour is worse than a refusal, because it ships. → `GATE-PROVEN`
- [ ] **PD6-09** Rebranded output is graded by PD2. → `GATE-PROVEN`

**Smoke — SD6**
- [ ] **SD6-01** A guide is ingested and a design is rebranded; every change cites its rule.
- [ ] **SD6-02** A guide with a rule the design cannot satisfy reports it rather than approximating.

---

### PD6-B · The failure this phase exists to avoid *(AAE post-mortem)*

- [ ] **PD6-10** 🔴 **Wire the brand guideline to layout composition on day one, with a test that proves a guideline changes the output.** AAE's most severe open finding is exactly this: *"the brand guideline did not influence output."* The policy schema was written; the proof that a policy could move a logo came **a year later**. A guideline that ingests, validates, reports success and changes nothing visible is this phase failing while reporting green. → `GATE-PROVEN`



## Former PD0 test gates

## 6.5 PD0 — The capability contract

> ⚠ **The design phases had no gate section at all** until 2026-09-03 — a
> `grep` for `PD[0-9]` across this file returned zero hits, while `§0.5` says
> *"smoke-then-gates is mandatory; both are the definition of done"*. So every
> `DOC-GATE` / `UNIT-PROVEN` / `SMOKE-PROVEN` marker in `§10.5` pointed at
> nothing. This section opens the register for `PD0`; `PD1`–`PD6` owe theirs at
> their own closeouts.
>
> These run in `npm test` (`src/design/contract/gates.test.ts`), not in
> `cargo test` — the contract is TypeScript by Razi's ruling. ⚠ That places them
> outside `reachability_gate.rs`, which walks the four Rust crates only; it is
> why `src/design/` could sit caller-less through two phases without anything
> noticing. Wiring the suite into `npm test` is what replaces that backstop.

- [x] **TGD0-01** `[policy]` — The operation set is exactly `ingest` · `export` · `adapt` · `change`, the outcome set is exactly `complete` · `partial` · `held`, and a **reserved** verb is refused *as reserved* rather than as unknown — so a caller reaching for `measure` learns it is coming, instead of concluding they misspelled something.
      Source: `PD0-01` · Catches: a fifth verb arriving without a version decision · **Evidence: `GATE-PROVEN` 2026-09-03**
- [x] **TGD0-02** `[conservation]` — **No event carries a design document, only a reference.** Asserted as a **byte budget** on every serialized event across all four operations, not as a structural check on the type: a structural check passes forever while someone adds a convenience field holding the document, and the byte budget catches the actual mistake. 4 KiB, four times under AutoDesign's own 16 KiB input cap.
      Source: `PD0-02` · Catches: the 17–25 MB payload the IR exists to avoid, arriving down a pipe instead of into a context window · **Evidence: `GATE-PROVEN`, falsified** by inlining 9 KB into a settled event — fails naming *"the wire carries a reference, never the document"*
- [x] **TGD0-03** `[correctness]` — An **unknown schema version is rejected, never parsed optimistically** — and the known one still decodes, so the gate cannot pass by refusing everything.
      Source: `PD0-02` · Catches: version skew surfacing as wrong behaviour instead of a clean refusal · **Evidence: `GATE-PROVEN` 2026-09-03**
- [x] **TGD0-04** `[data]` — **A fidelity report cannot claim a score nothing measured.** `method: 'unmeasured'` must carry a `null` score; any other method must carry a real one. The honest report is accepted, so the gate is not simply banning the field.
      Source: `PD0-04` · Catches: **the AAE failure exactly** — an engine that reported a fidelity number computed as a penalty tally over its own output, with no rasterisation and no comparison anywhere, and looked measured for a year · **Evidence: `GATE-PROVEN`, falsified** by deleting the pairing check — fails naming *"null score"*
- [x] **TGD0-05** `[correctness]` — **A `partial` outcome must be evidenced.** An unevidenced partial is refused.
      Source: `PD0-04` · Catches: the old converter's behaviour of reporting loss as success, which is the case `PD0-04` exists for · **Evidence: `GATE-PROVEN` 2026-09-03**
- [x] **TGD0-06** `[seam]` — **The sidecar is spawnable and settles as a real child process.** `execFileSync('node', [main.js])`, stdin request, NDJSON parsed off stdout, terminal event decoded through the same invariants as ours. ⚠ It **fails rather than skips** when the build is absent: a seam that is not built is not proven, and a skipping gate is not green (`§0.5`).
      Source: `PD0-06`, `PD0-08` · Catches: `L-C12` at contract scale — a beautifully typed surface nothing has ever spoken across a process boundary · **Evidence: `GATE-PROVEN` 2026-09-03, falsified** by suppressing the terminal event — fails naming *"settled"*
      ⚠ **The class was written ahead of its proof, and the work audit (`W-15`) is right about it.** `GATE-PROVEN` is defined here as *"a blocking CI job that can fail is green"* — and at the moment this was recorded, CI had no `build:sidecar` step, so the job would have been **red on every run** from the commit that introduced the gate. It was corrected in the next commit (`a03d7ed`), which is the right outcome and the wrong order. Two consequences now closed: `pretest` runs `build:sidecar`, so the gate can no longer pass against pre-edit bytes locally (`A-22`), and `npm run smoke:design` is committed, so the seam is reproducible from the repository rather than from a scratchpad (`W-2`).
- [x] **TGD0-07** `[governance]` — **The published document still describes the shipped contract.** Parses the operations table out of `AUTODESIGN-CAPABILITY-CONTRACT.md` and compares the **set** against `OPERATIONS`; also requires every outcome, the reserved verb and all four exit codes to appear.
      ⚠ **The first version of this gate was blind and falsification proved it.** It used `toContain` over the whole document, so deleting an operation's table row left a stray mention in the mapping section and the gate stayed green over a document that no longer described a shipped operation — `L-C9`, in a gate written to prevent staleness. Re-pointed to the set.
      Source: `PD0-09`, `§0.3` · Catches: a partner team reading a document that no longer matches what they are calling · **Evidence: `GATE-PROVEN`, falsified** by swapping a documented verb — fails naming *"not the shipped ones"*
- [x] **TGD0-08** `[seam]` — **The conformance suite can fail.** Driven against a deliberately broken implementation it reports NOT CONFORMANT, and against the reference it passes — so the failure is discrimination rather than a suite that rejects everything.
      Source: `PD0-06` · Catches: `L-C9` in the deliverable a partner team will trust most · **Evidence: `GATE-PROVEN` 2026-09-03** — gate-proven, **not** falsified.

- [x] **TGD0-09** `[correctness]` — **Every frozen outcome has a producer.** Drives the reference across a hold, a no-op `change` and an ordinary operation, and compares the **set** of outcomes produced against `OUTCOMES`.
      ⚠ Set, not count: `L-27` is on record for a count check passing when one member is swapped for another.
      Source: `PD0-04`, work-audit `W-7` · Catches: the defect this phase shipped — at closeout only `partial` had a producer, so a consumer's `held` branch was untestable and the documented `exit 0` + `held` state was **unreachable** · **Evidence: `GATE-PROVEN` 2026-09-03, falsified** by making the hold branch unreachable with a fall-through that still decodes — fails `expected [ 'complete', 'partial' ] to deeply equal [ 'complete', 'held', 'partial' ]`

- [x] **TGD0-10** `[seam]` — **A refusal settles and exits `0`, across a real process boundary.** Spawns the sidecar with a request it must decline; the terminal event decodes as `held` carrying a `failure` diagnostic, and the process exits `0`.
      ⚠ Only the boundary can prove this: the exit code does not exist in-process.
      Source: `PD0-08`, work-audit `W-7` · Catches: `held` collapsing into "error", which silently merges the partner's two branches into one · **Evidence: `GATE-PROVEN` 2026-09-03, falsified** by the same plant — fails `expected 'partial' to be 'held'`

⚠ **The first falsification attempt on both of these did not count, and is recorded rather than quietly replaced.** Planting `outcome: 'partial'` on the hold branch turned both gates red — with `ContractError: a partial outcome must carry the output it partially produced`. That is the **decoder** rejecting the plant before either gate's own assertion ran. Red for the wrong reason is not a falsification, and accepting it would have recorded two gates as proven when neither had shown it could detect the thing it claims.

## Former P9 design delivery check

- [ ] **P9-16** The R1 capability still works from the shipped installer — ingest, grade, adapt, export. **Shipping R2 must not silently break R1**, and the only way to know is to test it here. → `SMOKE-PROVEN`
- [ ] **S9-04** A PSD ingests and exports from a shipped installer.
