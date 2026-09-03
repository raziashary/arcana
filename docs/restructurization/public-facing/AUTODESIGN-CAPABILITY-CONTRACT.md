# AutoDev · design capability contract `v0.1.0`

**For the AutoDesign team.** This document is the whole interface. You should be
able to build your side against it without asking us anything — if you find a
question it does not answer, that is a defect in this document and we want to
hear about it.

| | |
|---|---|
| **Status** | Contract frozen for `v0`. Reference implementation and conformance suite ship with it. |
| **Integration model** | AutoDev runs as a **sidecar you spawn** — a child process, like your own. No socket, no port, no daemon. |
| **Source of truth** | `src/design/contract/` in the AutoDev repository. This document is written from it, not the other way round. |
| **Conformance** | `runConformance(driver)` in `src/design/contract/conformance.ts`. It imports no test framework, so you can run it in your harness against your own stand-in. |

---

## 1 · Why a sidecar, and not a service

You have no MCP client, and building one costs you time you should not spend on
us. AutoDev has **no inbound surface an external process can reach**, and adding
one would put a listening socket in a local desktop application — the last one
it had was an unauthenticated endpoint removed for being local
remote-code-execution.

⚠ To be exact, because you are sizing the security posture of a process you are
about to spawn: AutoDev today *starts and dials* two loopback services of its
own — a Python orchestrator on `127.0.0.1:4000` and a memory service on `:8000`
— both of which are deleted in our next phase. Neither accepts traffic from
anything but AutoDev. What does not exist, and what we are declining to build,
is a port **you** would connect to.

So: **you spawn us, we answer on stdout, we exit.** No port to configure, no
credential to provision, no daemon to supervise, nothing to leave running. It is
the same shape you already operate for your own sidecar.

⚠ **We know about `/v1/forge/tool-invoke`.** You have a live, authenticated
inbound route and you already publish twelve tools into Forge through
`auto_execute.register_tools`. That path would work, and it is the natural home
for this later — but it makes AutoDev depend on Forge, a third system under a
hard no-edit rule, for a capability that does not need it yet. If you would
rather we register as a Forge origin instead, say so; the contract below does not
change, only its carrier.

---

## 2 · Invocation

```
node dist-sidecar/design/contract/main.js  < request.json
```

| Stream | Carries |
|---|---|
| **stdin** | Exactly one JSON object — the request — terminated by EOF. |
| **stdout** | Newline-delimited JSON. Zero or more progress events, then **exactly one** `settled` event, which is always last. |
| **stderr** | Protocol errors and crashes. **Never** contract traffic, so you can parse stdout line-by-line without a mode switch. |

### Exit codes

| Code | Meaning |
|---|---|
| **0** | A terminal `settled` event was emitted. **Read stdout for the outcome** — a refusal is still a successful invocation. |
| **1** | Accepted, then failed before settling. stdout may hold partial events; do not treat them as a result. |
| **2** | The request never decoded. Nothing was attempted. stderr names the offending field. |
| **130** | A signal arrived mid-flight. No terminal event was emitted. |

⚠ **`0` does not mean "the design converted".** It means "we answered". A `held`
outcome — we refused your input — exits `0`, because the invocation worked
perfectly and the answer was no. Conflating those would make every rejected file
look like a crashed sidecar.

### Cancellation

**Kill the child.** There is no cancel message: one that arrived on the same pipe
as the request would need an interleaving protocol nobody needs while one
operation is one spawn. Every operation is safely interruptible, and on a signal
we exit **without** emitting a terminal event — a `settled` for work that did not
finish is worse than silence, because you would believe it.

### Concurrency

Spawn more children. One invocation is one process; that is what process
isolation is for.

---

## 3 · The four operations

Frozen for `v0`. A fifth invented now is a fifth to version forever.

| Verb | Does | Takes a design? |
|---|---|---|
| `ingest` | A source file becomes a design representation | **No** — it produces one |
| `export` | A design becomes a webpage, PDF or PSD | Yes |
| `adapt` | A design is reformatted to another size | Yes |
| `change` | Text, image, position, colour or shape is altered | Yes |

`measure` is **reserved**. It is not implemented and a request naming it is
refused explicitly — you will be told it is reserved, not that it is unknown, so
you know it is coming rather than concluding you misspelled something.

### How these map to your vocabulary

There is **no lexical overlap** between our four verbs and your eighteen
capability ids, so here is the mapping rather than leaving you to infer it.

| Our verb | Your nearest capability | Note |
|---|---|---|
| `ingest` | `source.pdf.extract` · `source.psd.reference` · `source.raster.inspect` | Ours is one verb over three of yours; the source kind is in the request |
| `export` | `export.png` · `render.preview` | ⚠ Yours is PNG-only; ours covers webpage, PDF and PSD |
| `adapt` | *(none)* | ⚠ Your only namesake, `adapt_artboard`, is a **Forge tool**, not a capability — a different registry |
| `change` | `document.command.apply` · `transform.apply` · `text.edit` · `vector.*` · `selection.update` · `history.*` | ⚠ **Ours is one to your eight.** See below |
| *(none)* | `instruction.propose` | One of your two currently-available capabilities has no counterpart here |

⚠ **The `change` fan-out matters and it is your safety property, not ours.** You
split those eight across two approval levels — `document.command.apply` is
`risk: high, approval: explicit` while `transform.apply` is `approval: none` —
and your manifest marks the gradient load-bearing with
`controls.approvalIsCapabilitySpecific: z.literal(true)`. Our single `change`
erases that distinction on our side of the pipe. **Approval stays yours to
enforce, before you spawn us.** We do not re-implement it and we do not pretend
the difference is ours to erase.

---

## 3.5 · Who executes, on our side

Stated because the contract is otherwise silent on it, and silence would leave
you to infer it.

**In Rollout 1 the work is done by AutoDev's personal assistant** — the
interactive agent — not by its worker fleet. That is a deliberate choice by our
owner rather than a limitation: routing design work through a multi-agent fleet
now would, in his words, be *"pretty confusing and hard to debug"* while the
conversion itself is still being refined. **The fleet becomes an additional
consumer of this same contract in Rollout 2.** Nothing you build against this
document changes when that happens.

Two consequences worth having in writing:

- **`adapt` is deterministic.** No model call happens inside it — that is
  enforced on our side by a gate, not by convention. Given the same design and
  the same target it returns the same result.
- **Judgement lives at the edges.** Choosing a target size, adjudicating a
  `partial` outcome, deciding whether a substituted font is acceptable — those
  are agent decisions, and in Rollout 1 they are the assistant's.

**Approval is yours.** We do not re-implement your approval gradient and we do
not evaluate it. You decide whether an operation may run, before you spawn us.

---

## 4 · The payload is a reference

**The wire never carries a design.** It carries a pointer to one:

```json
{
  "contractVersion": "autodev-design-ref/0.1.0",
  "schemaVersion":   "autodev-design-ir/0.1.0",
  "ref":             "autodev-design:abc123",
  "sha256":          "…64 lowercase hex…",
  "byteLength":      1048576
}
```

Two reasons, and the second is the one that generalises:

1. **Your own envelope caps capability input at 16 KiB** (`invocation.ts:35-39`).
   The representation of a 30–45 MB PSD is orders of magnitude past it.
2. **It would be wrong anyway.** The representation exists precisely because the
   old converter's output — 17.41 MB for a 1080×1080, 25.27 MB for a 1080×1920,
   every layer an embedded base64 PNG — cannot enter a model's context. Putting
   it down a pipe instead of into a context window is the same mistake with a
   different carrier.

`schemaVersion` is checked, not decorative: a payload naming a version we do not
know is **rejected**, never parsed optimistically.

---

## 5 · Outcomes

Every terminal event carries exactly one of:

| Outcome | Means | Invariant we enforce |
|---|---|---|
| `complete` | It worked | Output present, **no** failure diagnostic |
| `partial` | It worked, **with loss** | Output present, **at least one diagnostic** |
| `held` | We refused | **No** output, at least one failure diagnostic |

⚠ **`partial` is the one that matters, and it is why these are not called
`refused`/`failed`/`degraded`.** A "succeeded with loss" result cannot cross your
seam at all — `lifecycle.ts:80-85` rejects any result carrying both output and an
error. But you had the same problem and solved it, in your PDF family:
`outcome: z.enum(['inspection_ready', 'partial', 'held'])`. We took yours.

And as on your side, **`partial` must be evidenced**: a `partial` with no
diagnostics is refused by our own decoder. An unevidenced partial is exactly the
old converter's behaviour of reporting loss as success, which is the failure this
taxonomy exists to make impossible.

### Diagnostics

```json
{
  "code":        "FONT_UNRESOLVED",
  "category":    "dependency",
  "severity":    "warning",
  "retryable":   false,
  "safeMessage": "Montserrat-Bold was substituted with a system stack."
}
```

`category` and `retryable` are **yours, verbatim** (`lifecycle.ts:55-61`), so you
can route on either without translating. `severity` is from your PDF family and
is what separates *"a font was substituted"* from *"this is not a PSD"*.

---

## 6 · Fidelity

A field on the result, not a verb.

```json
{ "score": null, "method": "unmeasured", "losses": [] }
```

⚠ **`method` exists so this cannot lie, and today it says `unmeasured`.** We
have not built the renderer yet, so nothing measures fidelity, so we report a
`null` score rather than a plausible number.

That is deliberate and it is the most important thing on this page. The
organisation's previous adaptation engine reported a fidelity score that nothing
measured — a penalty tally computed over the same boxes the engine had just
produced, with no rasterisation and no comparison anywhere in the pipeline. It
looked measured for a year. When our renderer lands, `method` becomes
`rendered-comparison` and `score` becomes a real number; until then the contract
refuses to let us pretend, and our decoder rejects any `unmeasured` report that
carries a score.

---

## 7 · Assets

Bytes move by reference, and the field set **follows your
`artifactReferenceSchema`** so the shape is familiar rather than novel:

```json
{
  "contractVersion": "autodev-artifact-ref/0.1.0",
  "artifactId":   "op-1-export",
  "artifactKind": "export_reference",
  "mediaType":    "image/png",
  "byteLength":   40960,
  "sha256":       "…",
  "storageRef":   "autodev-artifact:op-1",
  "createdAt":    "2026-09-03T12:00:00.000Z",
  "expiresAt":    null
}
```

**Who owns the lifetime: the producer, until `expiresAt`.** `null` means never
expires — not "unknown". We enforce your refinement too: expiry must be strictly
after creation.

⚠ **It is not drop-in, and an earlier draft of this page wrongly said it was.**
Ours adds `contractVersion` and `artifactKind`, and your schemas are `.strict()`
— so your decoder will *reject* our payload on the two undeclared keys, and
`artifactKind` is free-form here against a closed five-member enum there. The
field names and semantics match so a mapping is mechanical; the decoder is not
shared. We would rather you learn that here than from a failing `.parse()`.

⚠ **Timestamps must carry an explicit UTC offset.** We refuse ones that do not,
and that is not pedantry — this codebase shipped the offset-less-timestamp defect
twice in one week, in both directions: a queue age that always read `0s` because
an RFC3339 parser failed and fell back to zero, and a UI that inflated every age
by five and a half hours because `new Date("…")` treated the same string as local
time.

⚠ **Two gaps are on your side, and we would rather name them than have you find
them.** Your `artifactKind` is a closed five-member enum with no `psd` and no
generic raster, and your byte-transfer route is pinned to `application/pdf`.
Until those widen, PSD bytes have no legal path *into* your side. That is fine
for Rollout 1 — the PSD arrives at ours — but it will matter at export.

---

## 8 · A worked example

**Request** (one line on stdin):

```json
{"contractVersion":"autodev-design-request/0.1.0","operationId":"op-42","operation":"adapt","design":{"contractVersion":"autodev-design-ref/0.1.0","schemaVersion":"autodev-design-ir/0.1.0","ref":"autodev-design:src","sha256":"0000000000000000000000000000000000000000000000000000000000000000","byteLength":1024},"input":{}}
```

**Response** (stdout, one JSON object per line, abridged):

```
{"contractVersion":"autodev-design-event/0.1.0","type":"accepted","operationId":"op-42","operation":"adapt","sequence":1,…}
{"contractVersion":"autodev-design-event/0.1.0","type":"started","operationId":"op-42",…,"sequence":2,…}
{"contractVersion":"autodev-design-event/0.1.0","type":"progressed",…,"sequence":3,"progress":{"stage":"transform","completed":null,"total":null}}
{"contractVersion":"autodev-design-event/0.1.0","type":"settled",…,"sequence":4,"outcome":"partial","design":{…},"artifacts":[],"diagnostics":[{"code":"REFERENCE_IMPLEMENTATION_ONLY",…}],"fidelity":{"score":null,"method":"unmeasured","losses":[]}}
```

Exit code `0`.

⚠ **What you are talking to today is the reference implementation.** It answers
the contract correctly and honestly reports `partial` with
`REFERENCE_IMPLEMENTATION_ONLY`, because the conversion behind it lands in our
next phase. It will not claim `complete` for work it did not do. Build against
it; the shapes will not move under you.

---

## 9 · What is not settled yet

Stated rather than discovered later:

- **Real conversion.** Our PSD reader is currently browser-bound — its asset
  store holds live canvas objects — so a spawned process needs a native canvas
  backend we have not yet taken on. That is our next phase's first task.
- **Latency.** You may have seen "25–35 s for a 30–45 MB input" quoted. That is a
  real measurement **of the old browser converter**, which produced the 17–25 MB
  HTML this contract exists to avoid. The representation path's cost has never
  been measured, and we will not quote you a number we have not taken. Treat the
  contract as async — which it is — and we will publish a real figure when we
  have one.
- **Whether `measure` becomes a verb.** Reserved, not promised.
- **`instruction.propose`.** Your live agentic entry point has no counterpart
  here. If you want one, it is a conversation worth having early.

---

## 10 · Running the conformance suite

```ts
import { runConformance, formatReport } from './src/design/contract'

const report = await runConformance({ run: (request) => yourImplementation(request) })
console.log(formatReport(report))
```

It imports no test framework.

**What it drives against your implementation:** that every operation settles
exactly once and last, that the settled event's `operation` is the one you
asked for, that events echo your `operationId`, that sequence numbers increase,
and — the one that carries the invariants — that your terminal event **decodes**
through the same `decodeSettled` we use, which is where the outcome rules live.

**What it checks about the contract itself, not about you:** that a fidelity
score cannot be claimed without a method, that an unevidenced `partial` is
refused, that a `held` cannot carry output, that an unknown schema version is
rejected and an undeclared field is not ignored. These run our decoder against
fixtures. ⚠ An earlier draft of this page described them as testing *your*
implementation; they do not, and the distinction matters when you read a green
report.

---

*Contract `v0.1.0` · AutoDev · this document is refreshed at every design-phase
closeout, so its date is the repository's, not this line's.*
