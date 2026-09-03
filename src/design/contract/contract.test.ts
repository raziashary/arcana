// PD0 — the contract's own tests.
//
// The conformance suite (`conformance.ts`) is the deliverable both teams run;
// these tests drive *it*, plus the cases a consumer-facing suite should not
// carry (our exit codes, our stdio framing).

import { describe, expect, it } from 'vitest'

import {
  ContractError,
  decodeRequest,
  decodeSettled,
  decodeArtifactRef,
} from './decode'
import { collect, fixedClock, referenceDesign } from './reference'
import { formatReport, requestFor, runConformance, type ContractDriver } from './conformance'
import { EXIT, runInvocation, type SidecarIo } from './sidecar'
import { OPERATIONS, RESERVED_OPERATIONS } from './types'
import { REQUEST_CONTRACT_VERSION } from './version'

// ── PD0-06 · the conformance suite, run against the reference ───────────────

describe('the conformance suite', () => {
  const driver: ContractDriver = {
    run: (request) => collect(request, fixedClock()),
  }

  it('passes against the reference implementation', async () => {
    const report = await runConformance(driver)
    expect(report.passed, formatReport(report)).toBe(true)
    // A suite that runs zero checks also "passes" — assert it did work.
    expect(report.results.length).toBeGreaterThanOrEqual(4 * OPERATIONS.length)
  })

  it('fails an implementation that never settles', async () => {
    // 🔴 The suite must be able to FAIL, or it is `L-C9` — a check whose shape
    // cannot express the defect it exists to catch. This is that proof.
    const broken: ContractDriver = {
      run: async (request) => (await collect(request, fixedClock())).filter((e) => e.type !== 'settled'),
    }
    const report = await runConformance(broken)
    expect(report.passed).toBe(false)
    expect(report.results.some((r) => !r.passed && r.name.includes('settled'))).toBe(true)
  })

  it('fails an implementation that ignores the caller’s operationId', async () => {
    const broken: ContractDriver = {
      run: async (request) =>
        (await collect(request, fixedClock())).map((e) => ({ ...e, operationId: 'not-yours' })),
    }
    const report = await runConformance(broken)
    expect(report.passed).toBe(false)
    expect(report.results.some((r) => !r.passed && r.name.includes('operationId'))).toBe(true)
  })
})

// ── PD0-01 · the frozen set ─────────────────────────────────────────────────

describe('the operation set', () => {
  it('is exactly the four frozen verbs', () => {
    expect([...OPERATIONS]).toEqual(['ingest', 'export', 'adapt', 'change'])
  })

  it('refuses a reserved verb explicitly rather than as an unknown one', () => {
    // The caller is told it is *reserved*, so they know it is coming rather
    // than concluding they misspelled something.
    for (const reserved of RESERVED_OPERATIONS) {
      expect(() =>
        decodeRequest({
          contractVersion: REQUEST_CONTRACT_VERSION,
          operationId: 'x',
          operation: reserved,
          design: null,
          input: {},
        }),
      ).toThrow(/reserved for a future version/)
    }
  })

  it('requires a design for every operation except ingest, and forbids one for ingest', () => {
    expect(() => decodeRequest({ ...requestFor('ingest'), design: referenceDesign('x') })).toThrow(
      /ingest produces a design/,
    )
    expect(() => decodeRequest({ ...requestFor('adapt'), design: null })).toThrow(
      /adapt acts on a design/,
    )
  })
})

// ── PD0-02 · versioning, which is the task ──────────────────────────────────

describe('schema versioning', () => {
  it('rejects an unknown IR schema version rather than parsing behind it', () => {
    expect(() =>
      decodeRequest({
        ...requestFor('adapt'),
        design: { ...referenceDesign('x'), schemaVersion: 'autodev-design-ir/2.0.0' },
      }),
    ).toThrow(ContractError)
  })

  it('rejects an unknown contract version on the envelope', () => {
    expect(() =>
      decodeRequest({ ...requestFor('ingest'), contractVersion: 'autodev-design-request/0.9.9' }),
      // The message now enumerates every version this build accepts, because
      // `SUPPORTED_VERSIONS` is the decoder's actual source (audit A-11) — a
      // consumer negotiating a version needs the list, not just the mismatch.
    ).toThrow(/expected one of autodev-design-request\/0\.1\.0/)
  })
})

// ── PD0-04 · the outcome invariants ─────────────────────────────────────────

describe('outcome invariants', () => {
  const skeleton = {
    contractVersion: 'autodev-design-event/0.1.0',
    type: 'settled',
    operationId: 'x',
    operation: 'ingest',
    sequence: 1,
    observedAt: '2026-09-03T12:00:00.000Z',
    outcome: 'complete',
    design: referenceDesign('x'),
    artifacts: [],
    diagnostics: [],
    fidelity: null,
  }

  it('accepts a well-formed complete result', () => {
    expect(decodeSettled(skeleton).outcome).toBe('complete')
  })

  it('refuses a complete result carrying a failure', () => {
    expect(() =>
      decodeSettled({
        ...skeleton,
        diagnostics: [
          { code: 'X', category: 'internal', severity: 'failure', retryable: false, safeMessage: 'm' },
        ],
      }),
    ).toThrow(/cannot carry a failure diagnostic/)
  })

  it('refuses an unevidenced partial — the defect this taxonomy exists for', () => {
    expect(() => decodeSettled({ ...skeleton, outcome: 'partial', diagnostics: [] })).toThrow(
      /must be evidenced/,
    )
  })

  it('refuses a held result that carries output', () => {
    expect(() =>
      decodeSettled({
        ...skeleton,
        outcome: 'held',
        diagnostics: [
          { code: 'X', category: 'validation', severity: 'failure', retryable: false, safeMessage: 'm' },
        ],
      }),
    ).toThrow(/held outcome cannot carry output/)
  })

  it('refuses a fidelity score that nothing measured', () => {
    expect(() =>
      decodeSettled({ ...skeleton, fidelity: { score: 1, method: 'unmeasured', losses: [] } }),
    ).toThrow(/must report a null score/)
  })

  it('refuses a measured fidelity with no score', () => {
    expect(() =>
      decodeSettled({
        ...skeleton,
        fidelity: { score: null, method: 'rendered-comparison', losses: [] },
      }),
    ).toThrow(/must report a score/)
  })
})

// ── PD0-05 · asset lifetime ─────────────────────────────────────────────────

describe('artifact references', () => {
  const artifact = {
    contractVersion: 'autodev-artifact-ref/0.1.0',
    artifactId: 'a',
    artifactKind: 'export_reference',
    mediaType: 'image/png',
    byteLength: 10,
    sha256: '0'.repeat(64),
    storageRef: 'autodev-artifact:a',
    createdAt: '2026-09-03T12:00:00.000Z',
    expiresAt: null,
  }

  it('accepts a reference with no expiry', () => {
    expect(decodeArtifactRef(artifact, 'a').expiresAt).toBeNull()
  })

  it('refuses an artifact that expires before it was created', () => {
    expect(() =>
      decodeArtifactRef({ ...artifact, expiresAt: '2026-09-03T11:00:00.000Z' }, 'a'),
    ).toThrow(/strictly after creation/)
  })

  it('refuses a timestamp with no UTC offset', () => {
    // This programme has shipped the offset-less-timestamp defect twice in one
    // week, in both directions. The contract refuses the input that causes it.
    expect(() => decodeArtifactRef({ ...artifact, createdAt: '2026-09-03 12:00:00' }, 'a')).toThrow(
      /explicit UTC offset/,
    )
  })
})

// ── PD0-08 · the sidecar seam ───────────────────────────────────────────────

describe('the sidecar invocation', () => {
  function io() {
    const out: string[] = []
    const err: string[] = []
    const sink: SidecarIo = { writeOut: (l) => out.push(l), writeErr: (l) => err.push(l) }
    return { out, err, sink }
  }

  it('emits NDJSON on stdout and exits 0 when it settles', async () => {
    const { out, err, sink } = io()
    const code = await runInvocation(JSON.stringify(requestFor('ingest')), sink, fixedClock())
    expect(code).toBe(EXIT.SETTLED)
    expect(err).toEqual([])
    // Every line must be independently parseable — that is what NDJSON buys a
    // consumer, and a single pretty-printed object would silently break it.
    const parsed = out.map((line) => JSON.parse(line))
    expect(parsed.length).toBeGreaterThan(1)
    expect(parsed[parsed.length - 1].type).toBe('settled')
    expect(() => decodeSettled(parsed[parsed.length - 1])).not.toThrow()
  })

  it('refuses a malformed request with the protocol code and nothing on stdout', async () => {
    const { out, err, sink } = io()
    const code = await runInvocation('{not json', sink, fixedClock())
    expect(code).toBe(EXIT.PROTOCOL)
    expect(out).toEqual([])
    expect(err[0]).toMatch(/^PROTOCOL:/)
  })

  it('refuses a decodable-but-invalid request, naming the field', async () => {
    const { out, err, sink } = io()
    const code = await runInvocation(
      JSON.stringify({ ...requestFor('ingest'), operation: 'sculpt' }),
      sink,
      fixedClock(),
    )
    expect(code).toBe(EXIT.PROTOCOL)
    expect(out).toEqual([])
    // A consumer across a process boundary has only this string to debug with.
    expect(err[0]).toContain('request.operation')
  })

  it('keeps contract traffic and diagnostics on separate streams', async () => {
    const { out, err, sink } = io()
    await runInvocation(JSON.stringify(requestFor('export')), sink, fixedClock())
    expect(err).toEqual([])
    expect(out.every((l) => l.startsWith('{'))).toBe(true)
  })
})
