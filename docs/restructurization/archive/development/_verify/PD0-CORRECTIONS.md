# PD0 pre-build refutation — main-session findings

Read in the main session, not delegated. §0.2m requires load-bearing results to
be re-verified here before they change a row; these are the ones I established
directly.

---

## M-1 · The IR exists, is good, and is **TypeScript in the frontend** · CONFIRMED-WITH-CONSEQUENCE

`src/design/` — `ir.ts` (7.6 KB), `psdToIr.ts` (12.3 KB), `fonts.ts` (7.3 KB)
and three test files. Landed by `88041f8` and `8ccd5f2`.

It is genuinely well-built. Its own header states the rule that keeps it small:

> *"**No pixels live in the IR.** An image layer carries an `assetId`, never the
> bitmap and never a data URI … Break this rule and the IR becomes the thing it
> was created to avoid — so `designIrToJson` asserts it (see `assertNoPixels`)."*

`StyleRun` preserves per-run styling, and the comment records why from the
spike: a layer reading *"Esport Esport Esport Esport"* whose three runs alternate
white and yellow, which a naive content swap would flatten.

**Consequence for PD0.** The wire payload PD0-02 publishes is a **TypeScript
interface in the Tauri frontend**, while the capability that would serve it is
Rust. Nothing in `autodev-core` or `src-tauri` mirrors it. So PD0 has to answer
a question its task list never asks: **which side owns the contract type**, and
if both, what keeps the two definitions from drifting — a problem this repo has
hit before (`src-shared`'s mirrored tables, deleted in P1 for exactly this).

## M-2 · The IR is **not versioned** · REFUTED

PD0-02: *"The IR schema is the wire payload, published as a **versioned** type."*

`ir.ts:155` —

```ts
export interface DesignDocument {
  width: number
  height: number
  sourceName: string
  layers: DesignLayer[]
  fonts: FontRequirement[]
  assetIds: string[]
}
```

No `version`, no `schemaVersion`, and a search for `version` across `ir.ts` and
`psdToIr.ts` returns only an unrelated comment about PSD opacity encoding and a
reference to conversion sizes. **The type exists; the versioning does not.**

Not fatal — it is a task rather than a blocker — but the claim as written is
false today, and "published as a versioned type" is the whole of PD0-02.

## M-3 · Razi already ruled the ownership question · CONFIRMED

`_verify/AUTODESIGN-INTEGRATION-RESEARCH.md §7.2`, 2026-09-01:

> **"Razi's ruling: someone else owns AutoDesign."** … *"AutoDev implements the
> **capability contract** it would expose, written to AutoDesign's existing
> versioned contract shape (`@airs/auto-design-capability-contracts`) … AutoDev
> does **not** implement `source.psd.reference` or `integration.invoke`."*

So AutoDesign's `integration.invoke` sitting at **G6, deferred** does **not**
block PD0 — publishing the contract is precisely how both halves get built in
parallel. The doc says so directly: *"The contract is a deliverable of this
work, not a byproduct: without it the parallel build has nothing to agree on."*

⚠ **But it does correct the milestone wording I wrote yesterday.** M1 said
*"AutoDesign can wire and test against the stub"*. They cannot — their own
invocation capability is three phases out. What M1 actually delivers is
**an agreed seam both halves can build against in parallel**. Fixing.

## M-4 · The precedent PD0-04 should follow, and the trap it must avoid

`provider/usage.rs:219` already defines an `ErrorClass` taxonomy, so PD0 should
follow that shape rather than invent one. Its own doc-comment carries the
warning:

> *"Until 2026-09-02 this enum had **no constructor at all**. It was defined,
> unit-tested against itself, and never produced by anything."*

That is `L-C12`, and it is the single likeliest way PD0 fails: a contract, a
taxonomy and a stub that are all beautifully typed and that **nothing produces
or consumes**. Every PD0 task needs a reachability proof, not a presence proof.

## M-5 · The design programme has **no novelty-ledger rows** · FINDING

`NOVELTY_LEDGER.md` holds **20** rows; none covers design. ⚠ This line originally said 6 — a `grep -c "^| "` counting table rows inside entries rather than entries — and the wrong figure propagated into the plan and the ADR before the work audit caught it. Yet `PD0` adopts an
external contract shape, `PD1` is grounded in an external converter
(`apps/psdconversions`), and `PD4`/`PD2` now carry ~33 tasks derived from an
external adaptation engine (the AAE post-mortem, folded in 2026-09-03).

The anchor-set rule is that **no externally-inspired mechanic becomes product
truth without a ledger row** carrying what we adopt, what we reject, and the
gate that proves it. Three externally-derived bodies of work currently have
none. Closing the PD0 row in this phase; flagging PD1/PD2/PD4 as owed.

## M-6 · 🔴 There is **no transport**. `PD0-06` cannot be satisfied as written · REFUTED

PD0-06: *"A **live stub** answers every operation with a valid, correctly-shaped
response … A contract nobody has exercised is a document; a stub makes it an
integration."*

**Answers over what?** Searched (not sampled) across `autodev-core/src`,
`src-tauri/src` and `src-cli/src` for `axum`, `actix`, `warp`,
`hyper::server`, `TcpListener::bind`, `HttpServer`. **Zero hits**, and neither
`Cargo.toml` carries a server dependency.

So AutoDev today has **no surface an external process can reach**:

- **Tauri commands** are in-process — they serve the app's own webview, not
  another application.
- **The `/mcp` server was deleted in P0**, deliberately: it accepted
  unauthenticated remote execution.
- **The Python sidecar's loopback goes in P8.**
- **`airsctl`** is a CLI: process invocation, not a service.

This is a scope finding, not a blocker — but PD0-06 as written assumes a
transport that does not exist, and building one is an architectural decision
with a security dimension, since the last listening socket in this codebase was
removed for being an RCE. **`§0.15` → stop and ask.**

### The options, as I understand them

| | Shape | For | Against |
|---|---|---|---|
| **A** | **MCP server surface** — the four operations as MCP tools | Forge and AutoDesign already speak MCP; AutoDev already has MCP *client* infrastructure and a live tool browser | Re-opens the surface P0 closed; needs auth designed, not assumed |
| **B** | **Loopback HTTP + ephemeral credential** | A proven pattern exists to copy in the sister product — `createLoopbackServer.ts` with `EphemeralHostCredential`, hashed and `timingSafeEqual` | A listening socket, again; and the pattern is theirs, in TypeScript |
| **C** | **CLI surface on `airsctl`** — `airsctl design ingest --in x.psd` | No socket, no auth surface, nothing to attack; `airsctl` keeps its task/DB surface after `P8-44` | Async job semantics and operation events fit a process invocation awkwardly |
| **D** | **Contract + conformance suite, no live process** | Cheapest; the seam is still agreed and both halves still build in parallel | PD0-06 explicitly rejects this — *"a contract nobody has exercised is a document"* |

⚠ Whichever is chosen, `PD0-06`'s evidence class is `SMOKE-PROVEN`, so the stub
has to be **exercised end to end by something that is not its own test**.
`L-C12` is the failure mode: a stub, a taxonomy and a contract that are
beautifully typed and that nothing produces or consumes.

---

## Razi's rulings, 2026-09-03

### R-1 · Transport — AutoDev becomes a sidecar of AutoDesign

> *"AutoDesign does not have any opening point and AutoDev itself is a local
> application, so if we create an HTTP process it will be unnecessary; and if we
> make it an MCP server, AutoDesign does not have an MCP client and building one
> will take time. So what we do is we will use AutoDev as a sidecar in
> AutoDesign essentially — so an executable contract, and make a single markdown
> document in docs public-facing for this one so the AutoDesign team knows what
> to do."*

Three things follow, and each closes something the refutation opened:

1. **No socket.** `M-6`'s finding stands and is answered without re-opening the
   surface `P0` deleted. The seam is **process invocation over stdio**.
2. **`PD0-06` is re-pointed** from a listening stub to an **executable
   contract** — a reference implementation plus a conformance suite both teams
   can run. The task's own intent survives intact; only the carrier changes.
3. **A new deliverable**: one public-facing document for the AutoDesign team
   (`PD0-09`). They own their half, they are three phases from calling us, and
   they cannot ask us questions — so that document *is* the interface until G6.

⚠ This also settles why the model fits: AutoDesign **already spawns a sidecar**
of its own, so "AutoDev is another sidecar" reuses a pattern that team already
operates rather than asking them to build a client.

### R-2 · The IR type is TypeScript-canonical

Rust mirrors it in `PD1`, behind a conformance test that pins the two together.
The IR and its consumer are both TypeScript today; porting it to Rust inside
`PD0` would be pulling `PD1`'s work forward for no gain at the seam.

⚠ The drift risk is real and named: this repository already deleted
`src-shared`'s mirrored task/worker/plan tables in `P1` because two definitions
of one schema had diverged. The pinning test is what stops the same shape
recurring, and it is `PD1`'s to write.

---

## The AutoDesign-side refutation (agent A), re-verified in the main session

Six claims attacked. Verdicts: **1 partially confirmed (actionable half refuted)**,
**1 partially confirmed/overstated**, **2 refuted**, **1 first-half confirmed /
second-half overstated**, **1 confirmed**. The three load-bearing findings were
re-read here before they changed a task line, per `§0.2m`.

### A-1 · The contracts package is real, excellent — and closed to us · ACTIONABLE HALF REFUTED

`@airs/auto-design-capability-contracts@0.2.0` exists, is `private: true`, and is
versioned in **four simultaneous ways**: a package version, a `CONTRACT_VERSION`
constant, per-family `z.literal` strings shaped `ad-<name>/0.2.0`, and a
`fixtures/v0.2.0/{valid,invalid,races}` corpus. Every schema is `.strict()`.

**But it cannot express an AutoDev capability.** Verified here:

```ts
capabilities.ts:37   export const capabilityIdSchema = z.enum(AUTO_DESIGN_CAPABILITY_IDS)
capabilities.ts:48     productOwner: z.literal('airs_auto_design'),
capabilities.ts:100    catalog: z.array(capabilityDescriptorSchema).length(AUTO_DESIGN_CAPABILITY_IDS.length),
```

The id set is a closed 18-member enum; the manifest names AutoDesign as the only
legal author; the catalog must carry every canonical capability exactly once.
**AutoDev is not one of the eighteen and cannot publish a conforming manifest.**

⚠ Their README says the schemas are *"provider-neutral"*. The code says
otherwise, and the code wins (`§0.2L-1`). They are transport-neutral and
vocabulary-specific.

**This validates the sidecar ruling.** The capability-host route was never open
to us, so "written to AutoDesign's shape" was not executable as stated. What
transfers is their **conventions** — `<name>/<semver>` literal versions, strict
decode, fail-closed on unknown fields, a versioned fixture corpus — not their
types.

### A-2 · Job semantics are under-specified by an order of magnitude · OVERSTATED

`CapabilityJobService` exists. But *"async, terminal statuses, operation events"*
flattens **9 internal statuses (5 terminal)** projected onto **10 public ones
(6 terminal)**, through a lossy adapter: `running → started`,
`cancel_requested → running`. Three contract-declared event types
(`admitted`, `progressed`, `approval_required`) have **no producer**, `progress`
is hard-`null`, and `awaiting_approval` is unreachable.

Mirror the **public** set, not the internal one. And note what the three-word
summary omits: single-file admission serialisation, a triple-keyed replay guard,
compare-and-swap on a state version, `attemptGeneration` capped at 8, a
three-valued `dispatchState` driving a kill switch, startup reconciliation into
`interrupted`, and result expiry returning 410.

### A-3 · `degraded` is a decode failure on their side · REFUTED

```ts
lifecycle.ts:80-85
  if ((result.status === 'succeeded') === (result.error !== null)) { …reject }
  if (result.status !== 'succeeded' && (result.output !== null || result.artifacts.length > 0)) { …reject }
```

A result carrying output **and** a diagnostic — which is exactly *"succeeded with
loss"* — fails their decode. Their error taxonomy is **nine categories**
(`validation, admission, authorization, conflict, capacity, timeout, cancelled,
dependency, internal`) plus a `retryable` boolean, and none of them is `refused`
or `degraded`.

**They solved the same problem elsewhere:** `sourcePdf.ts:170` —
`outcome: z.enum(['inspection_ready', 'partial', 'held'])`, with `partial`
required to be evidenced and `held` required to carry a typed failure. That is
`PD0-04`'s three-valued outcome, already designed and already shipped.

### A-4 · Asset transfer is already designed, twice · REFUTED

**By reference** — `artifacts.ts:4-21`, `artifactReferenceSchema` with
`artifactId`, `artifactKind`, `mediaType`, `byteLength`, `sha256`, `storageRef`,
`sourceDocumentRevision`, `createdAt`, `expiresAt`, and a refinement asserting
expiry follows creation. **That pair is the "who owns their lifetime" answer.**

**By bytes** — a request→handle→receipt handshake (`sourcePdf.ts:72-98`) with a
raw `PUT`, a pinned `application/pdf` content type, a refusal of any
`content-encoding`, and a 32 MB cap. **Not base64 in JSON.**

⚠ Two gaps are theirs: `artifactKind` is a closed five-member enum with no PSD
and no generic raster, and the byte route is PDF-only.

### A-5 · "They cannot reach us until G6" · OVERSTATED

The manifest citation is exact — `integration.invoke` is G6, `deferred`,
`future_forge_extension`, and it is the only entry with
`effect: 'external_effect'`. But **two live inbound paths already exist**:

```
createLoopbackServer.ts:726   if (url.pathname === '/v1/forge/tool-invoke') {
createLoopbackServer.ts:319   if (pathname === '/v1/forge/tool-invoke') { assertToolSeamAuthorized(…) }
```

reached **by Forge**, with server-to-server auth that bypasses the browser origin
check — and AutoDesign already publishes twelve tools into Forge via
`auto_execute.register_tools`. So `integration.invoke` is *AutoDesign calling
out*; Forge tool registration is *AutoDev being called*, which is what we
actually want.

⚠ **This corrects a premise of the ruling, not the ruling.** Razi said
*"AutoDesign does not have any opening point"* — it has one, but it is reached
through Forge, a third system under a hard no-edit rule. The sidecar still needs
nothing from anybody, which is why it stands. Recorded as the documented future
option in `PD0-09`.

### A-6 · Zero lexical overlap between the two vocabularies · CONFIRMED

Searched their tree for `ingest`, `adapt`, `change` as literals: **zero hits**.
`adapt` has no capability counterpart at all — its namesake `adapt_artboard`
lives in their *tool* register, a different contract with a different consumer.
And our single `change` collapses a **1→8** fan-out they split by risk and
approval, with `controls.approvalIsCapabilitySpecific: z.literal(true)` making
that gradient load-bearing.

### The single thing most likely to have made PD0 wrong

> *"PD0 is written as though `@airs/auto-design-capability-contracts` is a shape
> you can conform to. It is not a shape — it is a closed enumeration of
> AutoDesign's own eighteen capabilities, and AutoDev is not one of them."*

Second: `PD0-02`'s *"the IR schema is the wire payload"* against a **16 KiB**
envelope cap. Both are corrected above, before any code.
