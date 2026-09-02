# AutoDev — Programme Brief

**A condensed view of the restructure, for readers who are not in the code daily.**

| | |
|---|---|
| **Prepared** | 2026-09-02 |
| **Owner** | Razi Ashary — Programming Lead, AIRS Foundry |
| **Status** | P0, P1, P2 and **P3 A–E** complete · P3-F (memory) next · overall readiness **22%** |
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
| ✅ | **P0** Stop the bleeding | The security holes closed; CI can fail | 12 | done |
| ✅ | **P1** Structural | Everything shipped is compiled; one owner per database; redaction live | 22 | done |
| ✅ | **P2** Providers | Three providers conform; **Gemini primary**; cost honest; spend capped — proven against the live APIs, not only against fixtures | 74 | done |
| ✅ | **P3 A–E** Runtime | One tool path with a real permission gate; a panic cannot kill the app — **and four ways the model could reach past both, closed** | 43 | done |
| 1 | **P3-F** Memory | Memory that works and is provably read | 15 | ~7.5 h |
| 3 | **P8** Sidecar removal | The retired Python subsystem deleted entirely | 24 | ~6 h |
| 🤝 | **Handover** | **The AutoDesign team can begin wiring** | — | **~37 h in** |
| 5 | **PD1** Ingestion | PSD **and** PDF → one representation | 14 | ~6 h |
| 6 | **PD2** Fidelity | Faithfulness measured and refined | 14 | ~7.5 h |
| 7 | **PD3** Export | Webpage · PDF · PSD | 15 | ~7 h |
| 8 | **PD4** Adaptation | Reformat to any target size | 13 | ~7 h |
| 9 | **PD5** Changes | Text, image, position, colour, shape | 13 | ~5.5 h |
| 10 | **PD6** Brand guideline | Ingest a brand guide and apply it | 9 | ~3.5 h |
| 11 | **P4** Fleet | A task runs end to end and lands a verified artifact | 30 | ~12.5 h |
| — | **PD0** Contract | The integration surface, published early | 6 | ~2 h |

### Rollout 2

| # | Phase | Delivers | Tasks | Est. |
|---|---|---|---|---|
| 12 | **P5** Graph | A compiled code graph, read for context | 21 | ~9 h |
| 13 | **P6** UI | AutoDev's own interface | 28 | ~10 h |
| 14 | **P7** Experience | A new user reaches a working turn without documentation | 18 | ~5.5 h |
| 15 | **P9** Delivery | Signed installers, three platforms | 16 | ~7 h |

**Totals.** 208 tasks and roughly **92 hours** to complete Rollout 1; 83 tasks and roughly
**35 hours** for Rollout 2. **292 open tasks** across the programme, each written out and each
carrying the kind of evidence required to call it done.

### The handover point

The AutoDesign team can start after phase 4, roughly **37 working hours in**. They receive:

1. A codebase with the retired subsystem **fully removed** — no dead dependency to inherit.
2. A **published integration contract**, written to AutoDesign's own versioned format.
3. A running AutoDev that **answers that contract with working stubs**, so the integration
   can be built and tested on day one rather than waited for.

The contract's schema can be shared immediately — it depends only on the four operations,
which are already decided.

---

## 5 · Where we are

**P0, P1, P2 and P3 A–E are complete**, each closed with a mandatory independent audit.

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

**Why the readiness figure did not move.** P3 is scored as one area covering A–G, and P3-F
(memory) is untouched — so under the programme's own no-partial-credit rule the row stays at
zero. Recording A–E against a row that spans A–G would be exactly the arithmetic the rule
exists to prevent. What did move is the safety overlay: fault isolation from RED to AMBER,
and four model-reachable holes closed under security posture.

**Estimates are calibrated on measured throughput, not on judgement.** P1 built 22 tasks in
2 h 50 m; its audit and the resulting fixes took a further 3 h 30 m. The figures in §4 carry
that audit cost rather than omitting it.

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
