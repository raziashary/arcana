// PD0 — the capability contract's types.
//
// This is the whole interface between AutoDesign and AutoDev in Rollout 1.
// A reader on the other side of the seam cannot ask us questions, so every
// decision that is not obvious carries the reason it was made.

import type {
  ARTIFACT_REF_CONTRACT_VERSION,
  DESIGN_REF_CONTRACT_VERSION,
  EVENT_CONTRACT_VERSION,
  IR_SCHEMA_VERSION,
  REQUEST_CONTRACT_VERSION,
} from './version'

// ── PD0-01 · the operations, frozen for v0 ──────────────────────────────────

/**
 * The four things AutoDev does to a design. Frozen: a fifth invented now is a
 * fifth to version forever.
 *
 * ⚠ **These are AutoDev's verbs and they do not appear in AutoDesign's
 * vocabulary.** Searching their tree for `ingest`, `adapt` and `change` as
 * literals returns nothing — theirs is a set of eighteen dotted `noun.verb`
 * capability ids. The mapping is a deliverable of this contract, not an
 * accident of it; see `docs/restructurization/public-facing/`.
 *
 * ⚠ One consequence a consumer must know: our single `change` covers what
 * AutoDesign splits across eight capabilities at two different approval
 * levels — `document.command.apply` is `risk: high, approval: explicit` while
 * `transform.apply` is `approval: none`. Their manifest marks that gradient
 * load-bearing (`controls.approvalIsCapabilitySpecific: z.literal(true)`), so a
 * caller that flattens it loses a safety property **on their side**. Approval
 * is theirs to enforce; we do not re-implement it, and we do not pretend the
 * difference is ours to erase.
 */
export const OPERATIONS = ['ingest', 'export', 'adapt', 'change'] as const

export type Operation = (typeof OPERATIONS)[number]

/**
 * Names deliberately held back, so adding one later is not a breaking change.
 *
 * 🔴 `measure` is reserved rather than implemented *(razi-ruled)*. Fidelity is
 * a confirmed capability — the vision records it as direction #15, and `PD2` is
 * a whole phase for it — but a caller does not need to *invoke* it: an ingest
 * already knows how faithful it was, so fidelity ships as a field on the result
 * (see `FidelityReport`). Reserving the name costs nothing now and means a
 * later decision to expose it separately does not have to widen a frozen enum.
 *
 * ⚠ A decoder must **reject** a reserved operation rather than ignore it. A
 * name that is silently accepted and does nothing is worse than one that is
 * refused, because the caller believes it ran.
 */
export const RESERVED_OPERATIONS = ['measure'] as const

export type ReservedOperation = (typeof RESERVED_OPERATIONS)[number]

// ── PD0-02 · the payload is a REFERENCE ─────────────────────────────────────

/**
 * A pointer to a design document. **Never the document itself.**
 *
 * 🔴 The original task read *"the IR schema is the wire payload"*. It cannot
 * be. AutoDesign's own envelope caps capability input at **16 KiB**:
 *
 *   packages/auto-design-capability-contracts/src/invocation.ts:35-39
 *     if (utf8ByteLength(serialized) > 16_384) { … 'Capability input exceeds
 *     the 16 KiB envelope limit.' }
 *
 * and the IR of a 30–45 MB PSD is orders of magnitude past that. Their answer
 * is a typed reference plus a byte handshake (`sourcePdf.ts:153-155`), and we
 * take the same one.
 *
 * ⚠ It is also right for a sidecar, independent of them. Twenty-five megabytes
 * of JSON down a pipe is the same mistake in a different carrier — and it is
 * precisely the mistake the IR was created to avoid, since `ir.ts`'s own header
 * exists because the converter's 17–25 MB HTML *"cannot enter a model's
 * context"*.
 */
export interface DesignRef {
  contractVersion: typeof DESIGN_REF_CONTRACT_VERSION
  /** Which `DesignDocument` shape lies behind `ref`. */
  schemaVersion: typeof IR_SCHEMA_VERSION
  /** Opaque locator, `autodev-design:<id>`. Resolution is the host's business. */
  ref: string
  /** Lowercase hex SHA-256 of the exact bytes at `ref`. */
  sha256: string
  byteLength: number
}

// ── PD0-05 · assets move by reference, with a stated lifetime ───────────────

/**
 * A binary produced or consumed by an operation.
 *
 * ⚠ The field set mirrors AutoDesign's `artifactReferenceSchema`
 * (`packages/auto-design-capability-contracts/src/artifacts.ts:4-21`)
 * deliberately, so a caller that already decodes theirs can decode ours. Their
 * `createdAt`/`expiresAt` pair is the answer to *"who owns their lifetime"*:
 * **the producer does, until `expiresAt`.** `null` means no expiry, not
 * "unknown".
 *
 * ⚠ Two gaps are AutoDesign's, not ours, and the public document names them:
 * their `artifactKind` is a closed five-member enum with no PSD and no generic
 * raster, and their byte route is pinned to `application/pdf`. Until those
 * widen, PSD bytes have no legal path *into* their side — which is fine for
 * Rollout 1, because the PSD arrives at ours.
 */
export interface ArtifactRef {
  contractVersion: typeof ARTIFACT_REF_CONTRACT_VERSION
  artifactId: string
  /** Free-form, unlike AutoDesign's closed enum — see the note above. */
  artifactKind: string
  /** IANA media type of the bytes at `storageRef`. */
  mediaType: string
  byteLength: number
  sha256: string
  storageRef: string
  createdAt: string
  /** ISO-8601, strictly after `createdAt`. `null` = never expires. */
  expiresAt: string | null
}

// ── PD0-04 · the outcome triple, and why it is not `refused | failed | degraded`

/**
 * How an operation ended.
 *
 * 🔴 The task originally named these `refused` / `failed` / `degraded`. The
 * instinct — *"degraded is the one that matters: it is the case the old
 * converter reported as success"* — is exactly right, and the names were not,
 * because a `degraded` result **cannot cross the seam**:
 *
 *   packages/auto-design-capability-contracts/src/lifecycle.ts:80-85
 *     if ((result.status === 'succeeded') === (result.error !== null)) { …reject }
 *     if (result.status !== 'succeeded' && (result.output !== null || …)) { …reject }
 *
 * A payload carrying output *and* a diagnostic is a decode failure on their
 * side. But they hit the same problem and solved it, in their PDF family:
 *
 *   packages/auto-design-capability-contracts/src/sourcePdf.ts:170
 *     outcome: z.enum(['inspection_ready', 'partial', 'held'])
 *
 * So we take theirs, renaming only the first for a reader outside the PDF
 * context. `partial` **is** degraded, and — as on their side — it must be
 * evidenced: a `partial` with no diagnostics is a contradiction, and
 * `decodeResult` refuses it.
 */
export const OUTCOMES = ['complete', 'partial', 'held'] as const

export type Outcome = (typeof OUTCOMES)[number]

/**
 * Why an operation degraded or stopped.
 *
 * `category` and `retryable` are AutoDesign's, verbatim
 * (`lifecycle.ts:55-61`), so a caller can route on either without translating.
 * `severity` is from their PDF family (`sourcePdf.ts:139-140`) and is what
 * separates *"a font was substituted"* from *"the file is not a PSD"*.
 */
export const DIAGNOSTIC_CATEGORIES = [
  'validation',
  'admission',
  'authorization',
  'conflict',
  'capacity',
  'timeout',
  'cancelled',
  'dependency',
  'internal',
] as const

export type DiagnosticCategory = (typeof DIAGNOSTIC_CATEGORIES)[number]

export interface Diagnostic {
  /** Stable, machine-routable, `SCREAMING_SNAKE`. Never a sentence. */
  code: string
  category: DiagnosticCategory
  severity: 'warning' | 'failure'
  retryable: boolean
  /**
   * One sentence a human can act on.
   *
   * ⚠ Must be safe to display: no file paths outside the request, no
   * credentials, no stack traces. AutoDesign calls the same field
   * `safeMessage` and bounds it; the name here says what it is for.
   */
  safeMessage: string
}

/**
 * How faithfully the result represents its source.
 *
 * 🔴 **`method` exists so this cannot lie.** The single most valuable thing the
 * AAE post-mortem taught is that its engine *"has no measured fidelity
 * anywhere — no rasterisation, no comparison; every score is a penalty tally
 * over the same boxes the engine just produced"* (`_verify/A12`). A contract
 * that carries a `score` and no account of where it came from invites exactly
 * that: a number that looks measured and is not.
 *
 * So until `PD2` builds the renderer, an honest implementation reports
 * `method: 'unmeasured'` and `score: null`. **It does not emit a plausible
 * number.** `decodeSettled` enforces the pairing: `unmeasured` must carry a
 * null score, and any other method must carry one.
 */
export interface FidelityReport {
  /** 0–1, or `null` when nothing measured it. Never a guess. */
  score: number | null
  /**
   * How `score` was obtained. `PD2` adds `rendered-comparison`; anything that
   * is not a real comparison stays `unmeasured`.
   */
  method: 'unmeasured' | 'rendered-comparison'
  /** What was lost. Same shape as any other diagnostic, so one reader serves both. */
  losses: Diagnostic[]
}

// ── PD0-03 · job semantics, carried on stdout ───────────────────────────────

/**
 * What AutoDev writes back, one JSON object per line.
 *
 * ⚠ Mirrors AutoDesign's **public** lifecycle vocabulary rather than its
 * internal one. Theirs has nine internal statuses projected lossily onto ten
 * public ones (`publicContractAdapter.ts:57-67` maps `running → started` and
 * `cancel_requested → running`); a contract that mirrored the internal set
 * would be mirroring an implementation detail they already hide.
 *
 * ⚠ Three of their declared event types have **no producer** —  `admitted`,
 * `progressed` and `approval_required` — and their `progress` field is
 * hard-`null`. We declare only what we emit. An event type nothing produces is
 * `L-C12`, and this register holds eight instances of it.
 */
export type EventType = 'accepted' | 'started' | 'progressed' | 'settled'

export interface ProgressDetail {
  /** Short, stable stage label, e.g. `parse`, `rasterize`, `write`. */
  stage: string
  /** `null` when the total is genuinely unknown — never a guess. */
  completed: number | null
  total: number | null
}

/** A non-terminal event. Zero or more precede exactly one `settled`. */
export interface ProgressEvent {
  contractVersion: typeof EVENT_CONTRACT_VERSION
  type: Exclude<EventType, 'settled'>
  operationId: string
  operation: Operation
  sequence: number
  observedAt: string
  progress: ProgressDetail | null
}

/**
 * The terminal event. Exactly one per invocation.
 *
 * The three invariants below are enforced by `decodeSettled`, not merely
 * documented — they are what makes `partial` mean something:
 *
 *   • `complete` → `design` or `artifacts` present, and **no** `failure` diagnostic
 *   • `partial`  → output present, and **at least one** diagnostic
 *   • `held`     → **no** output, and at least one `failure` diagnostic
 */
export interface SettledEvent {
  contractVersion: typeof EVENT_CONTRACT_VERSION
  type: 'settled'
  operationId: string
  operation: Operation
  sequence: number
  observedAt: string
  outcome: Outcome
  /** The resulting design, when the operation produces one. */
  design: DesignRef | null
  artifacts: ArtifactRef[]
  diagnostics: Diagnostic[]
  /**
   * Present when the operation produced or transformed a design; `null` when
   * fidelity is not a meaningful question for it (a `held` outcome, or an
   * export that only serializes what it was given).
   */
  fidelity: FidelityReport | null
}

export type ContractEvent = ProgressEvent | SettledEvent

// ── PD0-08 · the request ────────────────────────────────────────────────────

/**
 * One invocation, delivered as a single JSON line on stdin.
 *
 * ⚠ `input` is deliberately unconstrained here and constrained per operation by
 * the reference implementation. The envelope's job is identity, versioning and
 * routing; smuggling operation-specific shapes into it is how an envelope
 * acquires a field that only one operation reads.
 */
export interface ContractRequest {
  contractVersion: typeof REQUEST_CONTRACT_VERSION
  /** Caller-generated, unique per invocation, echoed on every event. */
  operationId: string
  operation: Operation
  /** The design to act on. `null` for `ingest`, which produces one. */
  design: DesignRef | null
  input: Record<string, unknown>
}
