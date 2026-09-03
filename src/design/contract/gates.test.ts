// TG-PD0 — the permanent spec-invariant gates for the capability contract.
//
// Separate from `contract.test.ts` on purpose. Those are unit tests: they may be
// rewritten freely as the implementation moves. These encode promises made to a
// partner team, and a change here is a change to the contract.
//
// ⚠ Every gate below drives real behaviour. None asserts that a type exists —
// `L-C12` has eight instances in this programme, and a contract is the easiest
// possible place to add a ninth.

import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

import { decodeRequest, decodeSettled } from './decode'
import { collect, fixedClock, referenceDesign } from './reference'
import { requestFor, runConformance, formatReport, type ContractDriver } from './conformance'
import { OPERATIONS, OUTCOMES, RESERVED_OPERATIONS } from './types'
import { REQUEST_CONTRACT_VERSION } from './version'

const DOC = 'docs/restructurization/public-facing/AUTODESIGN-CAPABILITY-CONTRACT.md'
const SIDECAR = 'dist-sidecar/design/contract/main.js'

describe('TG-PD0 · the capability contract', () => {
  // ── TGD0-01 [policy] ──────────────────────────────────────────────────────
  it('TGD0-01 · the operation set is frozen and a reserved verb is refused', () => {
    expect([...OPERATIONS]).toEqual(['ingest', 'export', 'adapt', 'change'])
    expect([...OUTCOMES]).toEqual(['complete', 'partial', 'held'])

    // A reserved name must be refused *as reserved*. Silently accepting one is
    // worse than refusing it, because the caller believes it ran.
    for (const reserved of RESERVED_OPERATIONS) {
      expect(() =>
        decodeRequest({
          contractVersion: REQUEST_CONTRACT_VERSION,
          operationId: 'g',
          operation: reserved,
          design: null,
          input: {},
        }),
      ).toThrow(/reserved/)
    }
  })

  // ── TGD0-02 [conservation] ────────────────────────────────────────────────
  it('TGD0-02 · no event carries a design document, only a reference', async () => {
    // 🔴 Behavioural, not structural. Asserting "DesignRef has no layers field"
    // would pass forever while someone widened `input` or added a convenience
    // field holding the document. A byte budget catches the actual mistake:
    // the whole point of the reference is that the wire stays small, and an
    // inlined IR cannot hide from a size check.
    //
    // 4 KiB is generous for an envelope of ids, digests and timestamps, and is
    // four times under AutoDesign's own 16 KiB input cap.
    const BUDGET = 4096
    for (const operation of OPERATIONS) {
      const events = await collect(requestFor(operation), fixedClock())
      for (const event of events) {
        const size = Buffer.byteLength(JSON.stringify(event), 'utf8')
        expect(
          size,
          `${operation}/${event.type} serialized to ${size} bytes — the wire carries a reference, never the document`,
        ).toBeLessThanOrEqual(BUDGET)
      }
    }
  })

  // ── TGD0-03 [correctness] ─────────────────────────────────────────────────
  it('TGD0-03 · an unknown schema version is rejected, never parsed', () => {
    expect(() =>
      decodeRequest({
        ...requestFor('adapt'),
        design: { ...referenceDesign('x'), schemaVersion: 'autodev-design-ir/9.9.9' },
      }),
    ).toThrow()
    // …and the known one still works, or the gate is passing because the
    // decoder rejects everything.
    expect(() => decodeRequest(requestFor('adapt'))).not.toThrow()
  })

  // ── TGD0-04 [data] ────────────────────────────────────────────────────────
  it('TGD0-04 · fidelity cannot claim a score nothing measured', () => {
    const base = {
      contractVersion: 'autodev-design-event/0.1.0',
      type: 'settled',
      operationId: 'g',
      operation: 'ingest',
      sequence: 1,
      observedAt: '2026-09-03T12:00:00.000Z',
      outcome: 'complete',
      design: referenceDesign('g'),
      artifacts: [],
      diagnostics: [],
    }
    expect(() =>
      decodeSettled({ ...base, fidelity: { score: 0.99, method: 'unmeasured', losses: [] } }),
    ).toThrow(/null score/)
    // The honest report is accepted, so the gate is not simply banning the field.
    expect(() =>
      decodeSettled({ ...base, fidelity: { score: null, method: 'unmeasured', losses: [] } }),
    ).not.toThrow()
  })

  // ── TGD0-05 [correctness] ─────────────────────────────────────────────────
  it('TGD0-05 · a partial outcome must be evidenced', () => {
    const base = {
      contractVersion: 'autodev-design-event/0.1.0',
      type: 'settled',
      operationId: 'g',
      operation: 'ingest',
      sequence: 1,
      observedAt: '2026-09-03T12:00:00.000Z',
      design: referenceDesign('g'),
      artifacts: [],
      fidelity: null,
    }
    expect(() => decodeSettled({ ...base, outcome: 'partial', diagnostics: [] })).toThrow(
      /evidenced/,
    )
  })

  // ── TGD0-06 [seam] ────────────────────────────────────────────────────────
  it('TGD0-06 · the sidecar is spawnable and settles as a child process', () => {
    // 🔴 The one gate that proves the SEAM rather than the shapes. Everything
    // else here runs in-process; a contract nobody has spoken across a real
    // process boundary is a document, which is what PD0-06 exists to prevent.
    if (!existsSync(SIDECAR)) {
      throw new Error(
        `${SIDECAR} is missing — run \`npm run build:sidecar\`. ` +
          'This gate deliberately fails rather than skipping: a seam that is not built is not proven.',
      )
    }
    const request = JSON.stringify(requestFor('ingest'))
    const stdout = execFileSync('node', [SIDECAR], { input: request, encoding: 'utf8' })
    const events = stdout
      .split('\n')
      .filter((l) => l.trim() !== '')
      .map((l) => JSON.parse(l) as unknown)

    expect(events.length).toBeGreaterThan(1)
    const last = events[events.length - 1] as { type?: string }
    expect(last.type).toBe('settled')
    // Decoded, so the child's output is held to the same invariants as ours.
    expect(() => decodeSettled(last)).not.toThrow()
  })

  // ── TGD0-07 [governance] ──────────────────────────────────────────────────
  it('TGD0-07 · the published document still describes the shipped contract', () => {
    // 🔴 The doc/code drift gate. `§0.3` makes a stale document a hard defect,
    // and this one is read by a team that cannot ask us questions — so it going
    // stale is not a tidiness problem, it is a wrong answer given to someone
    // building against us.
    expect(existsSync(DOC), `${DOC} is missing`).toBe(true)
    const doc = readFileSync(DOC, 'utf8')

    // 🔴 The set, not a mention. `toContain` over the whole document was the
    // first version of this check and falsifying it proved it blind: deleting
    // the operation's row from the table left a stray mention in the mapping
    // section, and the gate stayed green over a document that no longer
    // described a shipped operation. `L-C9` — a check whose shape cannot
    // express the failure it exists to catch.
    //
    // So: parse the operations table and compare the SET. This fails when an
    // operation is added to the code and not the document, and when one is
    // removed from the document and not the code.
    const section = doc.split('## 3 · The four operations')[1] ?? ''
    const table = section.split('\n\n').find((b) => b.includes('|---'))
    expect(table, 'the operations table is missing from the document').toBeDefined()
    const documented = new Set(
      (table ?? '')
        .split('\n')
        .map((row) => /^\|\s*`([a-z]+)`/.exec(row)?.[1])
        .filter((v): v is string => v !== undefined),
    )
    expect([...documented].sort(), 'the documented operations are not the shipped ones').toEqual(
      [...OPERATIONS].sort(),
    )
    for (const outcome of OUTCOMES) {
      expect(doc, `the document does not mention the "${outcome}" outcome`).toContain(
        `\`${outcome}\``,
      )
    }
    for (const reserved of RESERVED_OPERATIONS) {
      expect(doc, `the document does not say "${reserved}" is reserved`).toContain(reserved)
    }
    // The exit codes are the part a consumer branches on before parsing a byte.
    for (const code of ['**0**', '**1**', '**2**', '**130**']) {
      expect(doc, `the document does not document exit code ${code}`).toContain(code)
    }
  })

  // ── TGD0-08 [seam] ────────────────────────────────────────────────────────
  it('TGD0-08 · the conformance suite can fail', async () => {
    // A suite that cannot fail is `L-C9`. Prove it by driving something broken.
    const broken: ContractDriver = {
      run: async (request) =>
        (await collect(request, fixedClock())).filter((e) => e.type !== 'settled'),
    }
    const report = await runConformance(broken)
    expect(report.passed, formatReport(report)).toBe(false)

    // …and it passes on the real one, so the failure above is discrimination
    // rather than a suite that rejects everything.
    const good: ContractDriver = { run: (r) => collect(r, fixedClock()) }
    expect((await runConformance(good)).passed).toBe(true)
  })
})
