// PD0-06 — the conformance suite.
//
// ── Why this exists, and why it takes no test framework ──────────────────────
//
// `PD0-06`'s original wording was *"a live stub answers every operation … a
// contract nobody has exercised is a document; a stub makes it an integration"*.
// The instinct is right and the mechanism was wrong — there was no transport to
// answer over. This is the replacement: a suite that **drives** an
// implementation and checks the contract's invariants against what came back.
//
// It imports nothing from `vitest`, deliberately. AutoDesign's team must be able
// to run it against *their* consumer-side stand-in, in their own harness, with
// their own runner. A conformance suite that only runs inside our test command
// is our test, not a contract.
//
// ── The failure mode it exists to prevent ────────────────────────────────────
//
// `L-C12` — a helper built, unit-proven, and wired to nothing — has eight
// recorded instances in this programme, one of them an error taxonomy that sat
// for months *"defined, unit-tested against itself, and never produced by
// anything"*. A contract is the easiest possible place to repeat it.
//
// ⚠ **This header used to claim "every check below drives a real implementation
// and reads what it produced". That was false when written**, and the area
// audit caught it: the five invariant checks at the bottom never touch the
// driver — they run our decoder against fixtures. The file is now split under
// two headings that say which is which, because a comment asserting the
// opposite of the code is the defect this programme has recorded most often.

import { decodeSettled, ContractError } from './decode'
import { OPERATIONS, type ContractEvent, type ContractRequest, type Operation } from './types'
import { REQUEST_CONTRACT_VERSION } from './version'
import { referenceDesign } from './reference'

/** Anything that can answer the contract. The suite does not care how. */
export interface ContractDriver {
  /** Run one request to completion and return every event it emitted, in order. */
  run(request: ContractRequest): Promise<ContractEvent[]>
}

export interface CheckResult {
  name: string
  passed: boolean
  detail: string
}

export interface ConformanceReport {
  results: CheckResult[]
  passed: boolean
}

/**
 * Build a valid request for `operation`, so callers need not hand-roll one.
 *
 * ⚠ `input` carries the operation's own shape (see `ContractRequest`). It used
 * to be `{}` for everything, which was harmless only because nothing read it.
 */
export function requestFor(
  operation: Operation,
  operationId = `conformance-${operation}`,
): ContractRequest {
  return {
    contractVersion: REQUEST_CONTRACT_VERSION,
    operationId,
    operation,
    design: operation === 'ingest' ? null : referenceDesign('source'),
    input: VALID_INPUT[operation],
  }
}

const VALID_INPUT: Readonly<Record<Operation, Record<string, unknown>>> = {
  ingest: { source: 'autodev-source:conformance.psd' },
  export: { format: 'json' },
  adapt: { target: { width: 1080, height: 1920 } },
  change: { instructions: [{ kind: 'conformance-noop' }] },
}

/**
 * A request the implementation is expected to **hold** — well-formed, decodable,
 * and asking for something it should decline rather than attempt.
 *
 * 🔴 Added for `W-7`. Of the three outcomes the contract freezes, only
 * `partial` had a producer, so a consumer's `held` branch was untestable and
 * the documented `exit 0` + `held` state was unreachable. A conformance suite
 * that never provokes a refusal cannot tell a correct refusal from an absent
 * one.
 */
export function requestForHold(
  operation: Operation,
  operationId = `conformance-hold-${operation}`,
): ContractRequest {
  return {
    contractVersion: REQUEST_CONTRACT_VERSION,
    operationId,
    operation,
    design: operation === 'ingest' ? null : referenceDesign('source'),
    // Empty input names none of the operation's required shape. Every
    // operation must decline it; none may decode-error on it, because the
    // envelope is valid — that distinction is the whole point of `held`.
    input: {},
  }
}

/**
 * Run one check against an already-collected result.
 *
 * Synchronous on purpose: the driver has already been run, so a check that
 * awaited would invite the per-check re-run that `A-21` was raised about.
 */
function check(results: CheckResult[], name: string, body: () => string): void {
  try {
    results.push({ name, passed: true, detail: body() })
  } catch (error) {
    results.push({ name, passed: false, detail: String(error) })
  }
}

/** The same, for the two places that must actually drive the implementation. */
async function checkAsync(
  results: CheckResult[],
  name: string,
  body: () => Promise<string>,
): Promise<void> {
  try {
    results.push({ name, passed: true, detail: await body() })
  } catch (error) {
    results.push({ name, passed: false, detail: String(error) })
  }
}

function assert(condition: boolean, message: string): void {
  if (!condition) throw new Error(message)
}

/**
 * Drive an implementation through every invariant the contract states.
 *
 * Returns a report rather than throwing, so a caller can present all failures
 * at once. A suite that stops at the first failure tells you one thing per run.
 */
export async function runConformance(driver: ContractDriver): Promise<ConformanceReport> {
  const results: CheckResult[] = []

  // ── every operation answers, and answers in the shape it declared ─────────
  //
  // ⚠ **One driver run per operation, not five** (`A-21`). The first version
  // called `driver.run` separately for each check — sixteen full runs where
  // four do — which is not merely wasteful against a partner driver that
  // actually converts a PSD: five checks observing five *different* runs can
  // disagree, so a non-deterministic implementation could fail a check on one
  // run and pass the equivalent one on another, and the report would not say
  // so. One run, then every property asserted against that one result.
  for (const operation of OPERATIONS) {
    const id = `echo-${operation}-42`
    let events: ContractEvent[]
    try {
      events = await driver.run(requestFor(operation, id))
    } catch (error) {
      results.push({
        name: `${operation}: the driver answers at all`,
        passed: false,
        detail: String(error),
      })
      continue
    }

    check(results, `${operation}: emits exactly one settled event, last`, () => {
      assert(events.length > 0, 'no events were emitted at all')
      const settledCount = events.filter((e) => e.type === 'settled').length
      assert(settledCount === 1, `expected exactly one settled event, saw ${settledCount}`)
      assert(
        events[events.length - 1]!.type === 'settled',
        'the settled event must be last — a consumer stops reading at it',
      )
      return `${events.length} events, terminal last`
    })

    check(results, `${operation}: the settled event reports the operation asked for`, () => {
      // 🔴 Added after the area audit. Without it, a stand-in that ignores
      // `request.operation` and always returns the `ingest` sequence with the
      // caller's id substituted passes every other check and is reported
      // CONFORMANT — so a team could ship against a seam they had implemented
      // for exactly one of the four verbs.
      const settled = events.find((e) => e.type === 'settled')
      assert(settled !== undefined, 'no settled event')
      assert(
        settled!.operation === operation,
        `asked for "${operation}", the settled event reported "${settled!.operation}"`,
      )
      return `echoed ${operation}`
    })

    check(results, `${operation}: the terminal event decodes`, () => {
      const settled = events.find((e) => e.type === 'settled')
      assert(settled !== undefined, 'no settled event to decode')
      // 🔴 Decoded, not inspected. The decoder is where the outcome invariants
      // live, so running it is what proves the implementation obeys them
      // rather than merely emitting the right field names.
      const decoded = decodeSettled(settled)
      return `outcome=${decoded.outcome}, diagnostics=${decoded.diagnostics.length}`
    })

    check(results, `${operation}: events carry the caller's operationId`, () => {
      const wrong = events.filter((e) => e.operationId !== id)
      assert(wrong.length === 0, `${wrong.length} event(s) carried a different operationId`)
      return `all ${events.length} events echoed ${id}`
    })

    check(results, `${operation}: sequence numbers strictly increase`, () => {
      for (let i = 1; i < events.length; i += 1) {
        assert(
          events[i]!.sequence > events[i - 1]!.sequence,
          `sequence ${events[i]!.sequence} did not follow ${events[i - 1]!.sequence}`,
        )
      }
      return `${events.length} events in order`
    })
  }

  // ── the refusal path, which is a settled outcome and not an error ─────────
  //
  // 🔴 `W-7`. A request that decodes but names none of its operation's input
  // must come back as a settled `held` — not as a protocol error, and not as
  // a `partial` pretending work happened. This is the check that makes the
  // documented "exit 0, outcome held" state something an implementation can
  // be held to rather than a paragraph.
  for (const operation of OPERATIONS) {
    await checkAsync(results, `${operation}: declines an unserviceable request as a settled hold`, async () => {
      const events = await driver.run(requestForHold(operation))
      const settled = events.find((e) => e.type === 'settled')
      assert(settled !== undefined, 'a decodable request produced no settled event')
      const decoded = decodeSettled(settled)
      assert(
        decoded.outcome === 'held',
        `expected outcome "held" for a request naming no input, got "${decoded.outcome}"`,
      )
      // The decoder already refuses a `held` carrying output; what it cannot
      // check is that the refusal was EXPLAINED. An unexplained refusal is
      // indistinguishable from a crash the implementation swallowed.
      assert(
        decoded.diagnostics.some((d) => d.severity === 'failure'),
        'a held outcome must carry a failure diagnostic saying why',
      )
      return `held: ${decoded.diagnostics.map((d) => d.code).join(', ')}`
    })
  }

  // ── the contract's own invariants ────────────────────────────────────────
  //
  // ⚠ **These do NOT drive your implementation.** They run our decoder against
  // fixtures, to demonstrate that the rules the contract states are actually
  // enforced by the decoder we ship — which is what your terminal event is put
  // through by the `decodes` check above. An earlier version of this file
  // claimed they tested the driver; they never did, and a report that mixes the
  // two invites over-trust in a green result. The checks that exercise *you*
  // are the four per-operation ones above.

  check(results, 'a fidelity report cannot claim a score it did not measure', () => {
    let refused = false
    try {
      decodeSettled({
        ...validSettledSkeleton(),
        fidelity: { score: 0.97, method: 'unmeasured', losses: [] },
      })
    } catch (error) {
      refused = error instanceof ContractError
    }
    assert(refused, 'an unmeasured fidelity carrying a score was accepted')
    return 'refused'
  })

  check(results, 'a partial outcome must be evidenced', () => {
    let refused = false
    try {
      decodeSettled({ ...validSettledSkeleton(), outcome: 'partial', diagnostics: [] })
    } catch (error) {
      refused = error instanceof ContractError
    }
    assert(refused, 'an unevidenced partial was accepted — this is the defect PD0-04 exists for')
    return 'refused'
  })

  check(results, 'a held outcome cannot carry output', () => {
    let refused = false
    try {
      decodeSettled({
        ...validSettledSkeleton(),
        outcome: 'held',
        design: referenceDesign('x'),
        diagnostics: [failureDiagnostic()],
      })
    } catch (error) {
      refused = error instanceof ContractError
    }
    assert(refused, 'a held outcome carrying a design was accepted')
    return 'refused'
  })

  check(results, 'an unknown schema version is rejected, not parsed', () => {
    let refused = false
    try {
      decodeSettled({
        ...validSettledSkeleton(),
        design: { ...referenceDesign('x'), schemaVersion: 'autodev-design-ir/9.9.9' },
      })
    } catch (error) {
      refused = error instanceof ContractError
    }
    assert(refused, 'a payload from an unknown schema version was parsed optimistically')
    return 'refused'
  })

  check(results, 'an unknown field is refused rather than ignored', () => {
    let refused = false
    try {
      decodeSettled({ ...validSettledSkeleton(), somethingNew: true })
    } catch (error) {
      refused = error instanceof ContractError
    }
    assert(refused, 'an undeclared field was silently ignored')
    return 'refused'
  })

  return { results, passed: results.every((r) => r.passed) }
}

// ── fixtures ────────────────────────────────────────────────────────────────

function failureDiagnostic() {
  return {
    code: 'CONFORMANCE_FIXTURE',
    category: 'validation' as const,
    severity: 'failure' as const,
    retryable: false,
    safeMessage: 'A fixture used by the conformance suite.',
  }
}

/** A minimal settled event that decodes cleanly, for the negative cases to break. */
function validSettledSkeleton(): Record<string, unknown> {
  return {
    contractVersion: 'autodev-design-event/0.1.0',
    type: 'settled',
    operationId: 'fixture',
    operation: 'ingest',
    sequence: 1,
    observedAt: '2026-09-03T12:00:00.000Z',
    outcome: 'complete',
    design: referenceDesign('fixture'),
    artifacts: [],
    diagnostics: [],
    fidelity: null,
  }
}

/** Render a report as text, for a CLI or a CI log. */
export function formatReport(report: ConformanceReport): string {
  const lines = report.results.map(
    (r) => `${r.passed ? 'PASS' : 'FAIL'}  ${r.name}\n        ${r.detail}`,
  )
  const failed = report.results.filter((r) => !r.passed).length
  lines.push(
    '',
    `${report.results.length} checks, ${failed} failed — ${report.passed ? 'CONFORMANT' : 'NOT CONFORMANT'}`,
  )
  return lines.join('\n')
}
