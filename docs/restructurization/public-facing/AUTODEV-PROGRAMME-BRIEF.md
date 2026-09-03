# AutoDev — Programme Brief

**A condensed view of the restructure, for readers who are not in the code daily.**

| | |
|---|---|
| **Prepared** | 2026-09-02 |
| **Owner** | Razi Ashary — Programming Lead, AIRS Foundry |
| **Status** | P0–P3 complete · **PD0 (the contract) next** · overall readiness **34%** · 319 open tasks |
| **Source documents** | `docs/restructurization/vision/00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md` (why) · `docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md` (what and when) |

This brief condenses both. Where it is terse, those documents are authoritative.

---

## 1 · What AutoDev is

A desktop application in which **a team of AI agents handles a programming task end to end**,
from a query as simple as the user can make it. It is a Tauri v2 app: a Rust core, a React
renderer, ten local SQLite databases, and a fleet of agents that plan, write, verify and
revise code.

It is being restructured because the shipped version could not honestly claim to do that yet.

---

## 2 · Why restructure — the grounded case

The programme opened with an audit of the shipped code rather than an aspiration. What it
found, in the codebase and not in opinion:

| Finding | Consequence |
|---|---|
| **918 lines of agent code were never compiled** — present on disk, absent from the module graph | A capability the product advertised did not exist in the binary |
| **The `/mcp` endpoint accepted unauthenticated remote execution** | Remote code execution on the user's machine |
| **The updater installed unverified payloads** | Any tampered update would install |
| **Commands returned raw API keys to the renderer** | The user's credentials were reachable from the web layer |
| **CI could not fail** — every check carried `continue-on-error` | Green builds meant nothing |
| **Ten databases had no migration ladder** | A schema change could silently destroy user data |
| **Secret redaction did not exist in the Rust tree** | Credentials could reach prompts, logs and permanent memory |

None of these were speculative. Each was located, and each is now closed or scheduled with a
test that fails when it regresses.

---

## 3 · The two rollouts

AutoDev ships **twice**. The first shipment is not the standalone application.

### Rollout 1 — *"The backend is ready, wire it to AutoDesign"*

**AutoDesign** is a sister product: a design tool. A design converts to HTML and CSS — which
is code — and code is AutoDev's domain. So in R1, **AutoDesign is the front end and AutoDev is
the engine behind it**. AutoDesign supplies the interface, the file, the user, and the
approval step; AutoDev supplies the capability.

R1 delivers four things:

1. **Ingestion** — a PSD or PDF becomes a structured, editable representation, faithful to the original *pixel by pixel*.
2. **Fidelity** — tooling that **measures** that faithfulness and refines what falls short. The existing converter looks right and is not right, and "looks right" is not a standard anyone can ship against.
3. **Export** — back out to a webpage, a PDF, or a PSD.
4. **Adaptation and changes** — reformat a design to another size or ratio, rebrand it, or change its content.

R1 also requires the parts of AutoDev that make those work at all: three conforming LLM
providers with honest cost accounting and a real spend ceiling, a runtime where one failing
tool cannot take down the application, working memory, and a fleet.

### Rollout 2 — *"The entire application is ready for usage"*

AutoDev standing on its own: its own interface, a compiled code graph for context, a
first-run experience, and signed installers for Windows, Linux and macOS.

**R1 is not R2 with pieces missing.** It is a complete product with a different consumer.
That distinction is what allows interface work to sit in R2 without crippling R1.

---

## 4 · The plan

Fifteen phases. Every one has an explicit slot — nothing is parked.

### Rollout 1

| # | Phase | Delivers | Tasks | Est. |
|---|---|---|---|---|
| ✅ | **P0** Stop the bleeding | The security holes closed; CI can fail | 27 | done |
| ✅ | **P1** Structural | Everything shipped is compiled; one owner per database; redaction live | 22 | done |
| ✅ | **P2** Providers | Three providers conform; **Gemini primary**; cost honest; spend capped — proven against the live APIs, not only against fixtures | 74 | done |
| ✅ | **P3 A–E** Runtime | One tool path with a real permission gate; a panic cannot kill the app — **and four ways the model could reach past both, closed** | 39 | done |
| ✅ | **P3-F** Memory | Memory that works, is scoped to its project, and cannot be written to by an agent claiming to be you | 18 | done |
| 1 | **PD0** Contract | The four verbs, the wire payload, and a live stub that answers them | 7 | **~1 h** |
| 🤝 | **M1 · Contract** | **AutoDesign can wire and test against the stub** | — | **~1 h away** |
| 2 | **P8** Conversion | The Python subsystem deleted entirely — **and every live defect closed with it** | 65 | **~11 h** ⚠ |
| 3 | **PD1** Ingestion | PSD **and** PDF → one representation | 21 | **~3.5 h** ⚠ |
| 🤝 | **M2 · First capability** | **A real file becomes a real design; the stub stops being a stub** | — | **~15 h in** |
| 4 | **PD2** Fidelity | Faithfulness **rendered and measured**, then refined | 27 | **~4.5 h** ⚠ |
| 5 | **PD3** Export | Webpage · PDF · PSD | 15 | **~2.5 h** ⚠ |
| 6 | **PD4** Adaptation | Reformat to any target size | 33 | **~5.5 h** ⚠ |
| 7 | **PD5** Changes | Text, image, position, colour, shape | 13 | **~2 h** ⚠ |
| 8 | **PD6** Brand guideline | Ingest a brand guide and apply it | 10 | **~1.5 h** ⚠ |
| 🤝 | **M3 · Full Rollout 1** | **Every design capability live** | — | **~32 h in** |
| 9 | **P4** Fleet | A task runs end to end and lands a verified artifact | 44 | **~7.5 h** |

### Rollout 2 — *"The entire application is ready for usage"*

AutoDev standing on its own: its own interface, a compiled code graph for context, a
first-run experience, and signed installers for Windows, Linux and macOS.

**R1 is not R2 with pieces missing.** It is a complete product with a different consumer.
That distinction is what allows interface work to sit in R2 without crippling R1.

---

## 4 · The plan

Fifteen phases. Every one has an explicit slot — nothing is parked.

### Rollout 1

| # | Phase | Delivers | Tasks | Est. |
|---|---|---|---|---|
| ✅ | **P0** Stop the bleeding | The security holes closed; CI can fail | 27 | done |
| ✅ | **P1** Structural | Everything shipped is compiled; one owner per database; redaction live | 22 | done |
| ✅ | **P2** Providers | Three providers conform; **Gemini primary**; cost honest; spend capped — proven against the live APIs, not only against fixtures | 74 | done |
| ✅ | **P3 A–E** Runtime | One tool path with a real permission gate; a panic cannot kill the app — **and four ways the model could reach past both, closed** | 39 | done |
| ✅ | **P3-F** Memory | Memory that works, is scoped to its project, and cannot be written to by an agent claiming to be you | 18 | done |
| 1 | **P8** Sidecar removal | The retired Python subsystem deleted entirely | 24 | **~4 h** |
| 🤝 | **Handover** | **The AutoDesign team can begin wiring** | — | **~5 h away** |
| 3 | **PD1** Ingestion | PSD **and** PDF → one representation | 14 | **~2.5 h** |
| 4 | **PD2** Fidelity | Faithfulness measured and refined | 14 | **~2.5 h** ⚠ |
| 5 | **PD3** Export | Webpage · PDF · PSD | 15 | **~2.5 h** |
| 6 | **PD4** Adaptation | Reformat to any target size | 13 | **~2 h** |
| 7 | **PD5** Changes | Text, image, position, colour, shape | 13 | **~2 h** |
| 8 | **PD6** Brand guideline | Ingest a brand guide and apply it | 9 | **~1.5 h** |
| 9 | **P4** Fleet | A task runs end to end and lands a verified artifact | 31 | **~5 h** |
| 2 | **PD0** Contract | The integration surface, published early | 6 | **~1 h** |

### Rollout 2

| # | Phase | Delivers | Tasks | Est. |
|---|---|---|---|---|
| 10 | **P5** Graph | A compiled code graph, read for context | 21 | **~3.5 h** |
| 11 | **P6** UI | AutoDev's own interface, including a **live design viewer** beside the conversation | 28 | **~4.5 h** ⚠ |
| 12 | **P7** Experience | A new user reaches a working turn without documentation | 18 | **~3 h** ⚠ |
| 13 | **P9** Delivery | Signed installers, three platforms | 16 | **~2.5 h** ⚠ |

**Totals.** 236 tasks and roughly **39.5 hours** to complete Rollout 1; 83 tasks and
roughly **14 hours** for Rollout 2. **319 open tasks** across the programme.

The phase rows account for 318 of those 319. The remaining one is P0's end-to-end test
harness, blocked by tooling rather than by effort — the Windows driver is not installed
locally, and the Linux/macOS driver has no engine for the framework AutoDev uses. It is
counted in the totals and left out of the estimates, because an hour figure would imply
the wait is ours to shorten.

⚠ **Rollout 1 grew from ~23.5 hours to ~39.5 on 3 September, and P8 nearly tripled.**
That is not a re-estimate — the rate is unchanged. A pre-P8 code review found ten live
defects in shipped code, and rather than carry them past a partner handover they were
folded into P8 along with the prompt-architecture work and the complete legacy
conversion. §5 says what they were. The rows marked ⚠ are the ones the throughput
measurement does not cover.

### The handover point — three of them

The single "handover" this document used to describe was overstated, and the phase
table above used to list the contract **last**, which read as it arriving at the very
end. Both are corrected.

**M1 · The contract — about an hour away, and blocked by nothing.** `PD0` freezes the
four operations, publishes the representation that crosses the wire, defines the job
and error semantics, and stands up a **live stub that answers all four**. That is
enough for the AutoDesign team to build and test their entire side of the integration
while the capability behind it is still being built. It does not depend on the Python
removal, and it does not depend on any design phase.

**M2 · The first real capability — about fifteen hours in.** After `P8` and `PD1`, a
real PSD or PDF becomes a real representation. The stub stops being a stub and the
integration starts returning true results.

**M3 · Full Rollout 1 — about thirty-two hours in.** Fidelity, export, adaptation,
changes and brand guidelines all live.

What the AutoDesign team receives at M1:

1. A **published integration contract**, written to AutoDesign's own versioned format.
2. A running AutoDev that **answers that contract with working stubs**, so the
   integration can be built and tested on day one rather than waited for.
3. A stated **execution model** — in Rollout 1 the personal assistant performs the
   design work; the worker fleet becomes an additional consumer in Rollout 2. Written
   into the contract rather than left to be inferred.

The codebase with the retired Python subsystem **fully removed** arrives at M2, not M1.

---

## 5 · Where we are

**P0, P1, P2 and all of P3 are complete**, each closed with a mandatory independent audit.

Delivered so far: the security holes closed and gated; a `autodev-core` crate extracted with
zero dependency on the UI framework, letting the desktop app and the command-line tool share
one implementation; migration ladders on all ten databases, applied to live data with zero
rows lost; secret redaction on the production path; three providers proven against their live
APIs rather than against fixtures; and the first of the design work — the intermediate
representation that everything in Rollout 1 is built on.

**P3 A–E — the runtime — closed on 2026-09-02.** 43 tasks, 19 automated gates, 520 tests. The
headline outcome is that the agent can no longer reach past its own guards. Four of the
defects it closed were reachable by the model itself:

- The permission check **did not run at all** on tools classified read-only. A tool declaring
  itself read-only was approved unconditionally in every mode, including the one whose entire
  purpose is to ask first.
- The syntax checker resolved its Python interpreter **from the repository being inspected**.
  Cloning a hostile repository and opening a `.py` file ran that repository's own binary.
- The end-to-end probe fetched **any URL the model named** and returned the response body. A
  cloud metadata endpoint returns credentials.
- Seven tools that take a file path never checked it against the workspace, two of which acted
  on the path rather than merely reading it.

None of these were in the phase's task list as written. They were found because the plan
requires an adversarial pass *before* building, and two independent audits *after*.

**P3-F — memory — closed on 2026-09-02.** 18 tasks, 608 tests. Memory now depends on what
you actually asked: before this phase every recall returned the same text whatever the
question, because the query was never passed to the search. It is scoped to the project it
belongs to, it degrades to keyword search on machines that cannot run the embedding model and
says so in the prompt rather than silently returning less, and an agent can no longer write
into it freely — a gateway decides what is worth keeping and tells the agent why when it
declines.

**The audits found four live defects in the phase's own finished code, and that is the point
of running them.** Three were reachable by the model itself:

- The task whose *entire purpose* is stopping an agent from claiming your authority shipped a
  flag that let any agent do exactly that — writing a memory at your own confidence, labelled
  "you told me", into every repository you open. A comment, a test and an automated gate all
  asserted this was impossible. All three were written in one sitting from one misreading, so
  they agreed with each other rather than with the code.
- Two of the three kinds of memory were **never written to the index recall reads**. Every
  conversation after the upgrade would have been silently unrecallable — and *"forget this"*
  did not forget: a memory you removed kept being fed to the model on every turn while every
  screen reported it deleted.
- Memory text was placed into the model's instructions **unquoted and last**, so a single
  poisoned file in a repository could have written a standing instruction that survived into
  every later session, and into every other project.
- One defect was in shared safety code: the backup check that guards *every* database upgrade
  had been rejecting perfectly good backups since the previous phase, which would have blocked
  the app from upgrading at all.

All are fixed, each with a test that was **deliberately broken first** to prove it can fail.

**Readiness rose 22% → 34%, not to 40%.** The phase is built and its smoke tests pass, but the
specification's own gate register is not fully satisfied — five of the eight memory gates are
not met and three are partly met, verified by an independent reviewer that had not seen any of
the reasoning behind the code. The largest remaining item: the vector-search table is queried
by the code and **never actually created**, so semantic search would fail if switched on. It
is off by default and nothing is broken today, but the honest score reflects it. An earlier
draft of this figure said 40% and was corrected the same hour.

**The estimates were wrong by about 3×, and every phase came in early — which is exactly
why nobody noticed.** This document said P2 would take ~11 hours; it took 5 h 32 m including its
audit. P3 A–E: ~12 hours estimated, 4 h 50 m actual. P3-F: ~7.5 hours estimated, 1 h 58 m actual.
An estimate that is always beaten never gets challenged, so the figure stayed wrong for three
phases while looking like good news.

They have now been rebuilt from commit timestamps rather than judgement. **P0 through P3-F is
183 tasks in 21 h 23 m of working time, across roughly 49 hours of calendar** — 8.6 tasks an
hour, counting build, tests, gates, the independent audits, the remediation those audits caused,
and the trackers. The figures in §4 plan at a deliberately slower **6 tasks an hour**, because a
phase's task list *grows* when it is checked before building: P2 was written as 27 tasks and
became 74, P3 A–E 28 became 43, P3-F 15 became 18. The slower rate carries that growth, and it
reproduces the three phases above to within about half an hour each.

Two honest caveats. The measured work is **all Rust and documentation**, so the two interface
phases have no measured throughput behind them at all. And the audit overhead — 46% on top of
build time — has fallen every phase as the plan improved; the estimates assume it stays low,
which is an assumption rather than a measurement.

---

## 6 · How the work is governed

The discipline is the reason the estimates are worth anything.

**Evidence classes.** No task is complete without a named proof recorded against it —
`UNIT-PROVEN`, `LIVE-DB-PROVEN`, `SMOKE-PROVEN`, `GATE-PROVEN`, `BENCH-PROVEN`. "It should
work" is not a status.

**Falsification.** A test that has never been seen to fail is not trusted. A violation is
deliberately introduced, the check is confirmed to catch it, and the violation is removed.

**Mandatory post-phase audit.** Every phase ends with independent auditors reviewing the work
against the evidence. **This is not a formality.** P1's audit returned 116 findings and
established that the phase's headline claim was false: secret redaction had been wired to a
function that nothing in production called, so it protected nothing while two separate proofs
reported green. It was found, fixed, re-proven, and recorded — including *why* both proofs had
been able to pass over unprotected code.

**A learnings register with running counts.** Recurring mistakes are tracked by class and
counted, on the principle that a repeat is worse than a novel error.

**Every decision logged.** Architectural decisions are recorded as they are made, with what,
why, when, where and how — including retractions when a previous finding proves wrong.

---

## 7 · Risks and open items

| Risk | Standing |
|---|---|
| **Signing certificates have external lead time** | Apple and Windows certificates gate the final phase and no engineering shortens the wait. Procurement should begin at Rollout 1 close, not at Rollout 2 |
| **The existing conversion algorithm is not pixel-accurate** | Known and scheduled. `PD2` exists specifically to measure and close it, rather than to assume it |
| **The integration depends on a second team** | Mitigated by publishing the contract early and backing it with working stubs, so both sides build in parallel against one agreed surface |
| **Fonts a design uses may not be obtainable** | Reported rather than silently substituted. A conversion that quietly loses a designer's typeface and still reports success is treated as a defect |

**Open decisions:** the brand-guide input format (a reference schema has been derived from
published guides and awaits approval), and confirmation of the exact model set beyond
`gemini-3.5-pro` and `gemini-3.5-flash`.

---

## 8 · The one-paragraph summary

AutoDev is being rebuilt to a standard where every claim is backed by a test that can fail.
Four phases are complete and independently audited, including the runtime phase that closed
four ways the agent could reach past its own guards. The next milestone is a
working backend that turns designs into faithful, editable, adaptable code — handed to the
AutoDesign team roughly 37 working hours from now, with a published contract and live stubs so
they can build in parallel. Full capability follows at roughly 92 hours, and the standalone
application at roughly 127. The estimates are calibrated on this team's own measured
throughput, and they include the cost of auditing the work rather than assuming it is right.
