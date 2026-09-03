// PD0 — the published import surface, resolved.
//
// 🔴 Area-audit `A-24`. `index.ts` is the barrel the partner-facing document
// tells AutoDesign to import from — and **nothing in this repository imported
// it**. `main.ts` imports `./sidecar` directly; the other test files import
// individual modules. So it typechecked and compiled, and was never *resolved*
// by anything that runs.
//
// That is the same shape as `L-C12` (a helper wired to nothing), with an extra
// edge: this file is the only part of the contract a consumer sees first. A
// rename in `decode.ts` that forgot to update the barrel would leave the
// published surface broken while every other test stayed green.
//
// This test therefore does the one thing nothing else did: it imports the
// barrel and touches every runtime export. It is deliberately shallow — the
// behaviour is tested where it lives; what is tested *here* is that the door
// opens.

import { describe, expect, it } from 'vitest'

import * as contract from './index'

describe('the published import surface', () => {
  it('exports every runtime symbol the partner document names', () => {
    // Types cannot be asserted at runtime; these are the values.
    const expected = [
      'OPERATIONS',
      'RESERVED_OPERATIONS',
      'OUTCOMES',
      'DIAGNOSTIC_CATEGORIES',
      'REQUEST_CONTRACT_VERSION',
      'EVENT_CONTRACT_VERSION',
      'DESIGN_REF_CONTRACT_VERSION',
      'ARTIFACT_REF_CONTRACT_VERSION',
      'IR_SCHEMA_VERSION',
      'SUPPORTED_VERSIONS',
      'ContractError',
      'decodeRequest',
      'decodeSettled',
      'decodeDesignRef',
      'decodeArtifactRef',
      'decodeDiagnostic',
      'decodeFidelity',
      'decodeProgress',
      'decodeEvent',
      'runOperation',
      'collect',
      'holdReason',
      'referenceDesign',
      'systemClock',
      'fixedClock',
      'SUPPORTED_EXPORT_FORMATS',
      'runConformance',
      'requestFor',
      'requestForHold',
      'formatReport',
      'runInvocation',
      'readAll',
      'EXIT',
    ]
    for (const name of expected) {
      expect(contract, name).toHaveProperty(name)
      expect((contract as Record<string, unknown>)[name], name).toBeDefined()
    }
  })

  it('exports nothing it did not mean to', () => {
    // The other direction: a stray `export *` added later would widen the
    // published surface silently. Comparing the SET, not the count — a count
    // check passes when one symbol is swapped for another.
    const actual = Object.keys(contract).sort()
    expect(actual.length).toBeGreaterThan(30)
    for (const name of actual) {
      expect(name, `${name} is exported but not declared in this test`).toMatch(
        /^(OPERATIONS|RESERVED_OPERATIONS|OUTCOMES|DIAGNOSTIC_CATEGORIES|REQUEST_CONTRACT_VERSION|EVENT_CONTRACT_VERSION|DESIGN_REF_CONTRACT_VERSION|ARTIFACT_REF_CONTRACT_VERSION|IR_SCHEMA_VERSION|SUPPORTED_VERSIONS|ContractError|decode(Request|Settled|DesignRef|ArtifactRef|Diagnostic|Fidelity|Progress|Event)|runOperation|collect|holdReason|referenceDesign|systemClock|fixedClock|SUPPORTED_EXPORT_FORMATS|runConformance|requestForHold|requestFor|formatReport|runInvocation|readAll|EXIT)$/,
      )
    }
  })

  it('the surface actually works when reached through the barrel', () => {
    // Resolving the module is not the same as the symbols behind it being
    // wired. One end-to-end touch through the published door.
    const request = contract.requestFor('adapt')
    expect(contract.decodeRequest(request).operation).toBe('adapt')
    expect(contract.holdReason(request)).toBeNull()
    expect(contract.holdReason(contract.requestForHold('adapt'))?.code).toBe(
      'ADAPT_TARGET_REQUIRED',
    )
    expect(contract.EXIT.SETTLED).toBe(0)
    expect([...contract.OPERATIONS]).toEqual(['ingest', 'export', 'adapt', 'change'])
  })
})
