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
