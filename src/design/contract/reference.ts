// PD0-06 — the reference implementation.
//
// ── What this is, and what it deliberately is not ────────────────────────────
//
// It answers all four operations with correctly-shaped, contract-valid results,
// so the contract is **exercised** rather than merely written down. It does not
// convert a PSD: ingestion is `PD1`, and the converter that exists today
// (`src/design/psdToIr.ts`) is browser-bound — its asset store holds live
// `HTMLCanvasElement`s, so a spawned Node process cannot call it without a
// native canvas backend that is in no manifest and no task. That dependency is
// `PD1`'s to take deliberately, not this phase's to smuggle in.
//
// So: the seam is real, the shapes are real, the invariants are enforced, and
// the work behind them arrives in `PD1` **behind this same interface**.
//
// ── Why an async generator ───────────────────────────────────────────────────
//
// The in-process conformance suite and the spawned sidecar must exercise the
// *same* code, or the thing we test is not the thing we ship. A generator of
// events is the one shape both can consume: the suite collects it, the sidecar
// serializes each item to a line of stdout.

import type {
  ContractEvent,
  ContractRequest,
  Diagnostic,
  Operation,
  ProgressEvent,
  SettledEvent,
} from './types'
import {
  DESIGN_REF_CONTRACT_VERSION,
  EVENT_CONTRACT_VERSION,
  IR_SCHEMA_VERSION,
} from './version'

/** Injected so tests are deterministic and the suite can assert ordering. */
export interface Clock {
  now(): string
}

export const systemClock: Clock = { now: () => new Date().toISOString() }

/**
 * A fixed clock for tests.
 *
 * ⚠ Returns an offset-bearing ISO string, because `decodeSettled` refuses one
 * without an offset — and it refuses it because this programme has twice
 * shipped an age computed from an offset-less timestamp.
 */
export function fixedClock(iso = '2026-09-03T12:00:00.000Z'): Clock {
  return { now: () => iso }
}

const REF_PREFIX = 'autodev-design:'

/** A design reference the decoder will accept, derived from an id. */
export function referenceDesign(id: string, byteLength = 1024) {
  return {
    contractVersion: DESIGN_REF_CONTRACT_VERSION,
    schemaVersion: IR_SCHEMA_VERSION,
    ref: `${REF_PREFIX}${id}`,
    // A deterministic 64-hex stand-in. `PD1` replaces it with the real digest
    // of the bytes it writes; the shape is what the contract pins here.
    sha256: id.padEnd(64, '0').slice(0, 64).replace(/[^0-9a-f]/g, '0'),
    byteLength,
  }
}

/**
 * The diagnostic every operation carries today.
 *
 * ⚠ Named and typed rather than tidied away: a reference implementation that
 * silently reports `complete` would be claiming work it did not do, which is
 * the exact shape `PD0-04` exists to make impossible. So it reports `partial`
 * and says why — the contract's own machinery, used on itself.
 */
const NOT_IMPLEMENTED = (operation: Operation): Diagnostic => ({
  code: 'REFERENCE_IMPLEMENTATION_ONLY',
  category: 'dependency',
  severity: 'warning',
  retryable: false,
  safeMessage:
    `This is PD0's reference implementation: the ${operation} contract is exercised, ` +
    'but the conversion behind it lands in PD1.',
})

// ── Reading `input`, and why two more outcomes now have producers ────────────
//
// 🔴 Work-audit `W-7` and area-audit `A-7`, which are the same defect seen from
// two sides. `types.ts` said `input` was *"constrained per operation by the
// reference implementation"* — and the reference never read it. Meanwhile of
// the three outcomes the contract freezes, exactly **one** had a producer:
// both settle branches hard-coded `partial`, so `complete` and `held` were
// shapes a partner could write a branch for and never once receive. Worse, the
// published exit-code table promised `held` exits `0`, while every refusal the
// sidecar could actually reach happened before any event and exited `2` — so
// the documented state was not merely unproduced, it was **unreachable**.
//
// `PD0-04`'s own amendment is the rule being applied here: *"`ErrorClass` sat
// for months defined, unit-tested against itself, and never produced by
// anything. Every variant here needs a producer or it should not exist."*
//
// So `input` is now read, per operation, and the reading produces the two
// outcomes the reference can honestly reach:
//
//   · `held`     — the request decoded, we accepted it, and we will not serve
//                  it. A refusal after acceptance, which is exactly the state
//                  exit `0` + `held` exists to express.
//   · `complete` — the work asked for was genuinely finished with nothing
//                  lost. A `change` with an empty instruction list is a real
//                  no-op: the design comes back unchanged and no fidelity
//                  question arises.
//   · `partial`  — everything else, because the conversion lands in `PD1`.
//
// What still has no producer is written down rather than manufactured: see
// `§6.1` of the partner document.

/** Formats `export` will serialize today. `PD2` adds the rendered ones. */
export const SUPPORTED_EXPORT_FORMATS = ['json'] as const

const held = (code: string, category: Diagnostic['category'], message: string): Diagnostic => ({
  code,
  category,
  severity: 'failure',
  retryable: false,
  safeMessage: message,
})

/**
 * Decide the outcome from the request, before any work is attributed to it.
 *
 * Returns a diagnostic to hold on, or `null` to proceed. Kept separate from
 * `settle` so the rule is readable on its own and testable without a stream.
 */
export function holdReason(request: ContractRequest): Diagnostic | null {
  const input = request.input

  if (request.operation === 'ingest') {
    if (typeof input.source !== 'string' || input.source.length === 0) {
      return held(
        'INGEST_SOURCE_REQUIRED',
        'validation',
        'ingest requires input.source — a reference to the bytes to read. ' +
          'Nothing was read and nothing was produced.',
      )
    }
    return null
  }

  if (request.operation === 'adapt') {
    const target = input.target
    if (typeof target !== 'object' || target === null) {
      return held(
        'ADAPT_TARGET_REQUIRED',
        'validation',
        'adapt requires input.target — { width, height } in pixels. ' +
          'Adapting to an unstated size would be guessing.',
      )
    }
    const { width, height } = target as { width?: unknown; height?: unknown }
    if (!isPositiveInt(width) || !isPositiveInt(height)) {
      return held(
        'ADAPT_TARGET_INVALID',
        'validation',
        'input.target.width and input.target.height must both be positive integers.',
      )
    }
    return null
  }

  if (request.operation === 'export') {
    const format = input.format
    if (typeof format !== 'string' || format.length === 0) {
      return held(
        'EXPORT_FORMAT_REQUIRED',
        'validation',
        `export requires input.format — one of: ${SUPPORTED_EXPORT_FORMATS.join(', ')}.`,
      )
    }
    if (!(SUPPORTED_EXPORT_FORMATS as readonly string[]).includes(format)) {
      return held(
        'EXPORT_FORMAT_UNSUPPORTED',
        'admission',
        `export format "${format}" is not served. Supported today: ` +
          `${SUPPORTED_EXPORT_FORMATS.join(', ')}. Rendered formats arrive with PD2.`,
      )
    }
    return null
  }

  // `change`
  if (!Array.isArray(input.instructions)) {
    return held(
      'CHANGE_INSTRUCTIONS_REQUIRED',
      'validation',
      'change requires input.instructions — an array. An empty array is a ' +
        'valid no-op and completes; a missing one is a malformed request.',
    )
  }
  return null
}

function isPositiveInt(v: unknown): v is number {
  return typeof v === 'number' && Number.isSafeInteger(v) && v > 0
}

function progress(
  request: ContractRequest,
  type: ProgressEvent['type'],
  sequence: number,
  clock: Clock,
  stage: string | null = null,
): ProgressEvent {
  return {
    contractVersion: EVENT_CONTRACT_VERSION,
    type,
    operationId: request.operationId,
    operation: request.operation,
    sequence,
    observedAt: clock.now(),
    progress: stage === null ? null : { stage, completed: null, total: null },
  }
}

/**
 * Run one operation, yielding its events in order.
 *
 * The last event is always `settled`, exactly once. A caller that receives a
 * stream without one should treat the invocation as failed regardless of exit
 * code — which is what `PD0-08` says and what the conformance suite asserts.
 */
export async function* runOperation(
  request: ContractRequest,
  clock: Clock = systemClock,
): AsyncGenerator<ContractEvent> {
  yield progress(request, 'accepted', 1, clock)
  yield progress(request, 'started', 2, clock)

  const stage = request.operation === 'ingest' ? 'parse' : 'transform'
  yield progress(request, 'progressed', 3, clock, stage)

  yield settle(request, clock)
}

function settle(request: ContractRequest, clock: Clock): SettledEvent {
  const base = {
    contractVersion: EVENT_CONTRACT_VERSION,
    type: 'settled',
    operationId: request.operationId,
    operation: request.operation,
    sequence: 4,
    observedAt: clock.now(),
  } as const

  // ── held · we accepted the request and will not serve it ─────────────────
  //
  // This is the branch that makes `exit 0 + held` reachable. It settles, so it
  // is a successful invocation carrying an unsuccessful outcome — which is the
  // distinction the exit-code table draws and could not previously demonstrate.
  const hold = holdReason(request)
  if (hold !== null) {
    return {
      ...base,
      outcome: 'held',
      design: null,
      artifacts: [],
      diagnostics: [hold],
      // A held operation measured nothing, so there is nothing to report. The
      // decoder enforces that a `held` carries no output; `null` fidelity is
      // the honest reading of "the question did not arise".
      fidelity: null,
    }
  }

  // ── complete · the work asked for was finished, with nothing lost ────────
  //
  // A `change` with an empty instruction list is a genuine no-op: the design
  // returns unchanged and no conversion was involved, so reporting `partial`
  // here would be the mirror-image lie of reporting `complete` for work we did
  // not do. It is the one path the reference can honestly complete today.
  if (request.operation === 'change' && (request.input.instructions as unknown[]).length === 0) {
    return {
      ...base,
      outcome: 'complete',
      design: request.design ?? referenceDesign(request.operationId),
      artifacts: [],
      diagnostics: [],
      fidelity: null,
    }
  }

  // `export` serializes what it was given: it produces an artifact, not a
  // design, and fidelity is not a meaningful question about it — so the field
  // is `null` rather than a fabricated 1.0.
  if (request.operation === 'export') {
    const createdAt = clock.now()
    return {
      ...base,
      outcome: 'partial',
      design: null,
      artifacts: [
        {
          contractVersion: 'autodev-artifact-ref/0.1.0',
          artifactId: `${request.operationId}-export`,
          artifactKind: 'export_reference',
          mediaType: 'application/json',
          byteLength: 2,
          sha256: '0'.repeat(64),
          storageRef: `autodev-artifact:${request.operationId}`,
          createdAt,
          expiresAt: null,
        },
      ],
      diagnostics: [NOT_IMPLEMENTED('export')],
      fidelity: null,
    }
  }

  // `ingest`, `adapt` and `change` all yield a design.
  return {
    ...base,
    outcome: 'partial',
    design: referenceDesign(request.operationId),
    artifacts: [],
    diagnostics: [NOT_IMPLEMENTED(request.operation)],
    // 🔴 `unmeasured`, with a null score, and that is the honest answer.
    //
    // The AAE post-mortem's most valuable finding is that its engine reports a
    // fidelity number nothing measured — "a penalty tally over the same boxes
    // the engine just produced". A reference implementation that emitted `1.0`
    // here would be the same lie at the seam, on day one, in the field a
    // partner team would trust most. `PD2` builds the renderer; until it does,
    // this says so.
    fidelity: { score: null, method: 'unmeasured', losses: [] },
  }
}

/** Collect a run into an array — what the conformance suite drives. */
export async function collect(
  request: ContractRequest,
  clock: Clock = systemClock,
): Promise<ContractEvent[]> {
  const events: ContractEvent[] = []
  for await (const event of runOperation(request, clock)) events.push(event)
  return events
}
