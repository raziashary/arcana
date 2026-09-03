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
// anything"*. A contract is the easiest possible place to repeat it. So every
// check below drives a real implementation and reads what it produced; none of
// them assert that a type exists.

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

/** Build a valid request for `operation`, so callers need not hand-roll one. */
export function requestFor(operation: Operation, operationId = `conformance-${operation}`): ContractRequest {
  return {
    contractVersion: REQUEST_CONTRACT_VERSION,
    operationId,
    operation,
    design: operation === 'ingest' ? null : referenceDesign('source'),
    input: {},
  }
}

async function check(
  results: CheckResult[],
  name: string,
  body: () => Promise<string> | string,
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
  for (const operation of OPERATIONS) {
    await check(results, `${operation}: emits exactly one settled event, last`, async () => {
      const events = await driver.run(requestFor(operation))
      assert(events.length > 0, 'no events were emitted at all')
      const settledCount = events.filter((e) => e.type === 'settled').length
      assert(settledCount === 1, `expected exactly one settled event, saw ${settledCount}`)
      assert(
        events[events.length - 1]!.type === 'settled',
        'the settled event must be last — a consumer stops reading at it',
      )
      return `${events.length} events, terminal last`
    })

    await check(results, `${operation}: the terminal event decodes`, async () => {
      const events = await driver.run(requestFor(operation))
      const settled = events.find((e) => e.type === 'settled')
      assert(settled !== undefined, 'no settled event to decode')
      // 🔴 Decoded, not inspected. The decoder is where the outcome invariants
      // live, so running it is what proves the implementation obeys them
      // rather than merely emitting the right field names.
      const decoded = decodeSettled(settled)
      return `outcome=${decoded.outcome}, diagnostics=${decoded.diagnostics.length}`
    })

    await check(results, `${operation}: events carry the caller's operationId`, async () => {
      const id = `echo-${operation}-42`
      const events = await driver.run(requestFor(operation, id))
      const wrong = events.filter((e) => e.operationId !== id)
      assert(wrong.length === 0, `${wrong.length} event(s) carried a different operationId`)
      return `all ${events.length} events echoed ${id}`
    })

    await check(results, `${operation}: sequence numbers strictly increase`, async () => {
      const events = await driver.run(requestFor(operation))
      for (let i = 1; i < events.length; i += 1) {
        assert(
          events[i]!.sequence > events[i - 1]!.sequence,
          `sequence ${events[i]!.sequence} did not follow ${events[i - 1]!.sequence}`,
        )
      }
      return `${events.length} events in order`
    })
  }

  // ── the no-fire half ─────────────────────────────────────────────────────
  //
  // ⚠ Without these, every check above is satisfiable by an implementation that
  // returns a fixed valid response to anything — which is `L-C9`, a check whose
  // shape cannot express failure.

  await check(results, 'a fidelity report cannot claim a score it did not measure', () => {
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

  await check(results, 'a partial outcome must be evidenced', () => {
    let refused = false
    try {
      decodeSettled({ ...validSettledSkeleton(), outcome: 'partial', diagnostics: [] })
    } catch (error) {
      refused = error instanceof ContractError
    }
    assert(refused, 'an unevidenced partial was accepted — this is the defect PD0-04 exists for')
    return 'refused'
  })

  await check(results, 'a held outcome cannot carry output', () => {
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

  await check(results, 'an unknown schema version is rejected, not parsed', () => {
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

  await check(results, 'an unknown field is refused rather than ignored', () => {
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
