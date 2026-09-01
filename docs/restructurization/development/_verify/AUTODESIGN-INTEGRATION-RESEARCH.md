# AutoDesign integration — research findings and sizing

**Written 2026-09-01.** Commissioned by Razi: *"research properly first on how much time
to take, what can we defer to make it faster, etc then come back to me."*

**Question.** AutoDev's endgame is to be wired to AutoDesign so a design becomes working
code. How long does that take, what is on the critical path, and what can be deferred
without lying about it?

---

## 1 · What the spike proved (`SPIKE-PROVEN`, 2026-09-01)

The existing converter was run against real designs, end to end, in a browser.

| | Square | Portrait |
|---|---|---|
| Input | `gamead_1080x1080bahasa-Recovered.psd` — 31.8 MB, 37 layers | `gamead_1080x1920_instagram_story.psd` — 41.9 MB |
| Engine | `ag-psd` 30.1.0, pure client-side | same |
| Time | ~25 s | ~35 s |
| Output | 17.41 MB single-file HTML | 25.27 MB |
| Fidelity | verified **visually**: hero art, both logos, text, badges, dates all correctly placed | same — renders correctly |

**It works.** The premise — a design becomes HTML/CSS, which is code, which is AutoDev's
domain — is demonstrated, not assumed.

**A correction I owe the record:** the refinements docs describe a *viewport scaling failure*
on 1080×1920, and my first spike used 1080×1080 — the size those docs say already works. So
I tested the easy case. I then ran the portrait case specifically to check, and **it renders
correctly**: that bug was fixed in the current `index.html`, which postdates the doc. The
estimate below does not carry it.

### 1.1 Three findings that change the design

**F-1 — the agent cannot work on the HTML.** 17.41 MB square, 25.27 MB portrait, because
every layer is an embedded base64 PNG. That cannot enter a model's context. But
`docs/psdtohtml/psdtohtml_architecture.md` already states the answer:

> *"the in-memory layer array is the universal intermediate representation … Everything
> downstream — PSD, HTML, SVG, React, Flutter — is just a different serialization of that
> same structured data."*

So **the layer IR is the agent-facing format and HTML is one serializer.** The IR is a few KB
of JSON — bounds, fonts, sizes, colors, tracking, `styleRuns`, blend modes, opacity, group
nesting. *That* is what adaptation operates on. Getting this backwards — handing the model
the HTML — makes the feature impossible, so it is the first architectural commitment, not an
optimisation.

**F-2 — two conversion defects, both observed in both orientations.**

1. A raw `U+0003` passes through untranslated. It is PSD's point-text line separator and it
   renders as a visible glyph box: *"Minggu**␃**30 Des"*.
2. Fonts resolve partially. `Montserrat` loads from Google Fonts; `Gimbal Extended` and
   `Bruce Forever` have no source and fall back to `sans-serif` **silently**. The design's
   display typography is lost and nothing reports it. The silent half is the real defect —
   it looks like it succeeded.

**F-3 — the CSP is nearly a non-issue.** AutoDev's renderer policy is `script-src 'self'
blob:`, so the four CDN `<script>` tags are blocked — but it *already* permits
`fonts.googleapis.com` (style-src) and `fonts.gstatic.com` (font-src). Only the libraries
need vendoring; `ag-psd@30.1.0` was pulled locally during the spike and confirmed a clean
drop-in.

---

## 2 · AutoDesign — what already exists

`C:\AIRS Foundry\auto-stack\autodesign`. Node ≥ 22.22, TypeScript, Vite, Vitest.

### 2.1 The integration seam exists already

`apps/auto-design-sidecar` is a Node sidecar AutoDesign already spawns locally. It carries,
today:

| Piece | What it is | Why it matters |
|---|---|---|
| `server/createLoopbackServer.ts` | loopback HTTP server, `EphemeralHostCredential` auth (hashed, `timingSafeEqual`) | **this is the local IPC** — it exists, it does not need inventing |
| `packages/auto-design-capability-contracts` | versioned wire + capability contracts | the surface AutoDev implements against |
| `capabilities/jobs/` | job store, job service, terminal statuses, operation events | conversion takes 25–35 s, so it must be a job — and the job model is built |
| `capabilities/source/` | `PdfTransferSpoolStore` → `PdfJsExtractionAdapter` → `PdfDerivedResultStore` → `createPassivePdfExtractionJobHandler` | **the template** a PSD capability copies file-for-file |
| `providers/ProposalProvider.ts` | `fake` \| `openai` seam | where an AI proposal enters |

### 2.2 The rule AutoDev must respect

> **AI never writes.** AI produces a *proposal*; the user must explicitly **Apply** or
> **Reject**, and Apply runs the same durable path.

AutoDev's output into AutoDesign is therefore a **proposal**, never a write.

### 2.3 The Forge collision, and why it is not fatal

AutoDesign is pinned to `submodules/core-orchestrator` (**Forge**) and forbidden from editing
it. AutoDev's **P8** deletes that same submodule "by content **or** by address."

They collide only if AutoDev must *contain* Forge. It does not: Forge is *"callable over
HTTP"* and the two apps talk over the loopback server. **P8 stands unchanged.** Embedding
Forge into AutoDev is the only version that breaks it, and nothing here requires that.

---

## 3 · AutoDesign's capability manifest — the binding constraint

`apps/auto-design-sidecar/src/capabilities/capabilityManifest.ts`. Every capability carries a
phase and an availability. **Two are `available`. Everything else is `planned` or `deferred`.**

| Capability | Phase | Availability | Relevance |
|---|---|---|---|
| `instruction.propose` | G2 | ✅ **available** | how a proposal enters |
| `source.pdf.extract` | G3B | ✅ **available** | the template |
| `source.psd.reference` | G3B | ⏳ planned | **the PSD slot exists in the contract, unbuilt** |
| `document.read` | G4A | ⏳ planned | reading the design to adapt |
| `document.command.apply` | G4A | ⏳ planned | applying an adaptation back |
| `transform.apply` · `text.edit` | G4A | ⏳ planned | the edit primitives |
| `render.preview` · `export.png` | G5 | ⏳ planned | seeing the result |
| `integration.invoke` | **G6** | ❌ **deferred**, `future_forge_extension` | **the capability that would invoke AutoDev at all** |

### 3.1 Stated plainly

**The binding constraint on "wire AutoDev to AutoDesign" is not AutoDev — it is AutoDesign's
own roadmap.** AutoDesign is at G2/G3B. The capability whose job is invoking an external
system is at **G6**, three phases out, marked **deferred**, and annotated
`future_forge_extension` — meaning the anticipated shape is *AutoDev registers as a Forge
tool and AutoDesign reaches it through Forge*. The sidecar already has
`forge/toolRegistration.ts` and `createAutoDesignToolTransport`, so that path is real; it is
simply not scheduled.

This does not contradict "local, not HTTP" — Forge and the loopback server both run on this
machine, and loopback HTTP *is* local IPC. What it means is that the wired outcome has a
dependency in a repository with a hard no-edit rule on Forge.

### 3.2 Consequence for the two outcomes

> *"AutoDev itself can be a single application so it can be two outcomes. One AutoDev on its
> own to code and the other one is wired to AutoDesign."*

- **Outcome 1 — AutoDev standalone, design → code.** No external dependency. Everything it
  needs is proven or already exists.
- **Outcome 2 — AutoDesign drives AutoDev.** Needs `source.psd.reference` (G3B, planned) and
  an invocation path (`integration.invoke`, G6, deferred) built on AutoDesign's side.

**Outcome 1 first is not a compromise — it is the prerequisite.** Outcome 2 is Outcome 1 plus
a transport, and the transport is the part that is not ours to schedule.

---

## 4 · What AutoDev already has

Counted in the tree, not assumed from the plan:

| | Today | What P2/P3 change |
|---|---|---|
| Providers | 4 files (`anthropic`, `openai`, `google`, `ollama`) | make them **conform**; `ollama` deleted (P2-11) |
| Agent loop | `agent/engine.rs`, 565 lines, `run()` exists | bounded context, one terminal event, cancellation |
| Tools | **45** builtin tool files + a registry | one execution path, `ToolEffect`, path jail |
| Databases | 10, all laddered | — (P1 done) |
| Redaction | on the production path | — (P1 done) |

**The agent loop is not built by P2/P3 — it already runs.** P2 and P3 make it *correct*. That
distinction is what makes a short path credible: design → code needs the loop to work well
enough to adapt an IR, not to be finished.

Matrix today: *Agent correctness* 🔴 RED — *"No caching, unbounded context, no permission gate
in force."* *Fault tolerance* 🔴 RED — *"`panic = "abort"` + in-process agents: one panic kills
everything."*

### 4.1 One P4 item the wiring partly sidesteps

Matrix: *Product capability* 🔴 RED — *"Agent output lands where the user never looks; no run
step exists. P4."* In the **wired** outcome AutoDesign **is** where the user looks, and it
already enforces Apply/Reject. The wired path inherits a review surface instead of building
one. AutoDev **standalone** still needs P4.

---

## 5 · Critical path for Outcome 1

Working days, one implementer.

| # | Work | Days | Basis |
|---|---|---|---|
| C-1 | Vendor `ag-psd@30.1.0` + the 411-line PSD→HTML core into the AutoDev webview | 0.5 | libs pulled, confirmed clean drop-in; only `script-src` to satisfy |
| C-2 | Fix `U+0003` → line break | 0.1 | one substitution, one test |
| C-3 | Font resolution: **report** unresolved fonts instead of silent fallback | 0.5 | the list is already collected in `_usedPsdFonts` |
| C-4 | **The layer IR** — a second serializer emitting JSON from the same walk that emits HTML | 1.0 | data already computed in `psdLayerToHtml`; a parallel emitter, not new parsing |
| C-5 | Register conversion as an agent tool + bound its output (P3-29) | 0.5 | 45 tools and a registry already exist |
| C-6 | Agent adaptation **on the IR** — read, modify, re-serialize | 1.5 | the open-ended one; see §5.1 |
| C-7 | Gates + smoke: real PSD converts, IR round-trips, no secret in the payload | 0.5 | P1's harness is in place |
| | **Subtotal** | **4.6** | |

Minimum from P2/P3 for C-6 to work at all:

| # | Work | Days |
|---|---|---|
| M-1 | **P3-A fault isolation** (P3-01…04) | 0.5 |
| M-2 | **P2 Anthropic conforming** (P2-01, 02, 03, 04, 17) | 1.5 |
| M-3 | **P2 cost honest + ceiling** (P2-18, 21, 22) | 1.0 |
| M-4 | **P3-C tool path minimum** (P3-10, 11, 14) | 1.0 |
| | **Subtotal** | **4.0** |

**Critical path total: ~8.5 working days.**

### 5.1 Adaptation — scope confirmed by Razi, and C-6 re-sized

Asked directly, Razi answered: **"adaptation means reformat across aspect ratio and rebrand
and content swap."** Two of the three candidates, and — importantly — **not** the fourth
option offered (design → semantic responsive codebase).

That exclusion is the load-bearing half of the answer. The output stays a **faithful
layer-based rendering, adapted** — it does not have to become a component library with
inferred semantics (*is this a nav? a card? a button?*), which was the expensive
interpretation. The IR carries everything the three confirmed kinds need.

C-6 therefore splits and grows:

| | Work | Days | Note |
|---|---|---|---|
| C-6a | **Reformat across aspect ratios** — reposition and rescale layers onto a new canvas | 1.5 | the hardest: it is layout reasoning, not substitution. Anchor/safe-area rules per layer. |
| C-6b | **Rebrand + content swap** — palette, logo, type, and text replacement preserving `styleRuns` | 2.0 | these share one machinery: mutate the IR, re-serialize. Rebrand adds palette/type mapping; content swap adds text and per-character style handling. |
| C-6c | **Brand-guide input** — a format for "what to rebrand *to*" | 1.0 | does not exist on our side today. AutoDesign's whole framing is *"findings can cite the brand guide they came from"*, so this must be a real input, not a prompt string. |
| | **C-6 total** | **4.5** | was 1.5 when adaptation was assumed to be reformat only |

**Revised critical path: 4.6 − 1.5 + 4.5 = 7.6 (C-series) + 4.0 (M-series) ≈ 11.5 days.**

I am recording the increase rather than absorbing it. The ~8–9 day figure Razi accepted was
priced on reformat alone; confirming three kinds of adaptation adds ~3 days, and quietly
keeping the old number would be the same failure class as a green gate over an unfinished
path.

### 5.2 Why P3-A is not droppable

Conversion runs 25–35 s of heavy canvas work on 30–45 MB inputs and holds a full layer bitmap
set in memory. That is exactly where a panic or OOM happens, and today `panic = "abort"`
means one takes the whole app down. Cutting fault isolation to save half a day, on the one
feature that allocates hardest, is a bad trade.

---

## 6 · What can be deferred — each with an owner

Nothing is dropped; each row names where it goes.

| Deferred | Tasks | Owner | Why it is safe |
|---|---|---|---|
| **P3-F Memory** | P3-30…P3-43 (**14**) | P3 later / P6 | sqlite-vec, ONNX vendoring, fastembed, reranker, gateway. Design→code needs none of it. Keep **P3-44** as an FTS5-only stub so recall is never empty. **The single biggest saving available.** |
| **P5 Graph** | whole phase | after the design path | graph-scoped context optimises code navigation; not a design→code dependency |
| **3 of 4 providers** | P2-05, 06, 07 | P2 continued | one conforming provider proves the loop; a *sequencing* deferral, not a cut |
| **P3-D Subagents** | P3-22, 23, 24 | P3 continued | one conversion is one job; fan-out is not on the path |
| **P2 polish** | P2-20, 24, 26, 27 | P2 continued | none blocks a working turn on Windows |
| **P4 Fleet** | whole phase | unchanged | wired mode inherits AutoDesign's review surface (§4.1) |

⚠ **These deferrals are already excluded from §5.** The ~8.5 days is what remains *after*
cutting memory, graph, three providers, subagents and the fleet. There is no second round of
deferral that reaches 3–4 days without cutting scope rather than schedule.

---

## 7 · The honest estimate

| Outcome | Estimate | Confidence |
|---|---|---|
| **Spike — PSD → HTML/CSS proven** | **done** | `SPIKE-PROVEN`, both orientations, visually verified |
| **Outcome 1 — standalone: PSD → IR → adapted output** | **~11–12 working days** | medium-high. Was ~8–9 when adaptation was assumed to be reformat only; three confirmed kinds add ~3 (§5.1) |
| **Outcome 2 — AutoDesign drives AutoDev** | **not ours to schedule** | AutoDesign is owned elsewhere (§7.2) |

### 7.1 On the number changing

Razi accepted "~8–9 days" against an estimate priced on **reformat only**. He then confirmed
adaptation is **three** kinds. The number is now **~11–12**, and it is stated here rather
than absorbed silently. `L-C9` and `L-C6` in the learnings register are both about
verification that could not express failure; an estimate that cannot express having grown is
the same defect wearing a different hat.

### 7.2 Ownership — decided

**Razi's ruling: someone else owns AutoDesign.** So this repository builds to a defined seam
and stops there:

- AutoDev implements the **capability contract** it would expose, written to AutoDesign's
  existing versioned contract shape (`@airs/auto-design-capability-contracts`), so both
  halves can be built in parallel against one agreed seam.
- AutoDev does **not** implement `source.psd.reference` or `integration.invoke`. Those stay
  AutoDesign's, at its G3B and G6.
- The contract is a **deliverable of this work**, not a byproduct: without it the parallel
  build has nothing to agree on.

This also means Outcome 1 must be genuinely useful standalone — it cannot be a half-feature
waiting for a transport that another team schedules.

---

## 8 · Decisions taken (2026-09-01)

| Question | Razi's answer |
|---|---|
| What is adaptation? | **reformat across aspect ratios + rebrand + content swap** (not design → semantic codebase) |
| Which target? | **Outcome 1 properly** — not the demo spine |
| Who owns AutoDesign's side? | **someone else**; AutoDev defines and publishes the contract |

**Deferred, with owners, to fund this** (unchanged from §6): P3-F memory (14 tasks), P5 Graph,
P2-05/06/07 (three adapters), P3-D subagents, P2 polish, P4 Fleet.
