// PD0 — fail-closed decoders for the capability contract.
//
// ── Why hand-rolled and not a schema library ─────────────────────────────────
//
// AutoDesign validates with zod. AutoDev has no zod, and adding one would put a
// dependency between a partner team and our contract for no gain: a consumer
// that already has zod can write their own schemas from these types, and one
// that does not can copy these functions. Zero dependencies is a feature of a
// contract, not a limitation of it. (`§0.15` also asks that a new dependency be
// flagged and proposed rather than assumed; declining one needs no permission.)
//
// ── The rule every function here obeys ───────────────────────────────────────
//
// **Refuse rather than repair.** A decoder that coerces, defaults, or ignores an
// unknown field is a decoder that lets a version skew through and surfaces it
// later as wrong behaviour instead of a clean rejection. AutoDesign's package
// marks every object `.strict()` for the same reason, and keeps a
// `fixtures/v0.2.0/invalid/` corpus to prove it.

import {
  DIAGNOSTIC_CATEGORIES,
  OPERATIONS,
  OUTCOMES,
  RESERVED_OPERATIONS,
  type ArtifactRef,
  type ContractRequest,
  type DesignRef,
  type Diagnostic,
  type FidelityReport,
  type Operation,
  type SettledEvent,
} from './types'
import {
  ARTIFACT_REF_CONTRACT_VERSION,
  DESIGN_REF_CONTRACT_VERSION,
  EVENT_CONTRACT_VERSION,
  IR_SCHEMA_VERSION,
  REQUEST_CONTRACT_VERSION,
} from './version'

/**
 * Why a payload was refused.
 *
 * ⚠ Carries a `path` because a consumer debugging across a process boundary has
 * nothing else to go on — "invalid request" costs them an hour that
 * `input.targetWidth: expected a positive integer` costs them a minute.
 */
export class ContractError extends Error {
  constructor(
    readonly path: string,
    message: string,
  ) {
    super(`${path}: ${message}`)
    this.name = 'ContractError'
  }
}

const fail = (path: string, message: string): never => {
  throw new ContractError(path, message)
}

// ── primitives ──────────────────────────────────────────────────────────────

const isObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v)

function obj(v: unknown, path: string): Record<string, unknown> {
  return isObject(v) ? v : fail(path, 'expected an object')
}

function str(v: unknown, path: string): string {
  if (typeof v !== 'string' || v.length === 0) {
    return fail(path, 'expected a non-empty string')
  }
  return v
}

function nonNegativeInt(v: unknown, path: string): number {
  if (typeof v !== 'number' || !Number.isSafeInteger(v) || v < 0) {
    return fail(path, 'expected a non-negative safe integer')
  }
  return v
}

function bool(v: unknown, path: string): boolean {
  return typeof v === 'boolean' ? v : fail(path, 'expected a boolean')
}

const SHA256 = /^[0-9a-f]{64}$/

function sha256(v: unknown, path: string): string {
  const s = str(v, path)
  return SHA256.test(s) ? s : fail(path, 'expected 64 lowercase hex characters')
}

/**
 * ISO-8601 with an explicit offset.
 *
 * ⚠ The offset is required, and that is not pedantry. This programme has
 * already shipped the same defect twice in one week: a timestamp written by
 * SQLite's `datetime('now')` without an offset, parsed by an RFC3339 reader
 * that failed and fell back to zero — so every queue age read as 0s — and the
 * same string read by `new Date(...)` in a browser, which treats it as *local*
 * and inflated every age by the machine's UTC offset. A contract that accepts
 * an offset-less timestamp is inviting both.
 */
function timestamp(v: unknown, path: string): string {
  const s = str(v, path)
  if (!/(?:Z|[+-]\d{2}:\d{2})$/.test(s) || Number.isNaN(Date.parse(s))) {
    return fail(path, 'expected an ISO-8601 timestamp with an explicit UTC offset')
  }
  return s
}

/**
 * Reject any field the contract does not declare.
 *
 * The equivalent of AutoDesign's `.strict()`. Without it, a caller can add a
 * field, believe it is being read, and discover otherwise in production.
 */
function noExtraKeys(v: Record<string, unknown>, allowed: readonly string[], path: string): void {
  const extra = Object.keys(v).filter((k) => !allowed.includes(k))
  if (extra.length > 0) {
    fail(path, `unknown field(s): ${extra.sort().join(', ')}`)
  }
}

function literal<T extends string>(v: unknown, expected: T, path: string): T {
  const s = str(v, path)
  return s === expected ? (s as T) : fail(path, `expected "${expected}", received "${s}"`)
}

function member<T extends string>(v: unknown, set: readonly T[], path: string): T {
  const s = str(v, path)
  return (set as readonly string[]).includes(s)
    ? (s as T)
    : fail(path, `expected one of ${set.join(' | ')}, received "${s}"`)
}

function array(v: unknown, path: string): unknown[] {
  return Array.isArray(v) ? v : fail(path, 'expected an array')
}

// ── the contract's own shapes ───────────────────────────────────────────────

const DESIGN_REF_KEYS = ['contractVersion', 'schemaVersion', 'ref', 'sha256', 'byteLength'] as const

export function decodeDesignRef(value: unknown, path = 'design'): DesignRef {
  const o = obj(value, path)
  noExtraKeys(o, DESIGN_REF_KEYS, path)
  return {
    contractVersion: literal(o.contractVersion, DESIGN_REF_CONTRACT_VERSION, `${path}.contractVersion`),
    // 🔴 This is the check `PD0-02` exists for. The IR had no version at all;
    // refusing an unknown one is what makes adding a version worth anything.
    schemaVersion: literal(o.schemaVersion, IR_SCHEMA_VERSION, `${path}.schemaVersion`),
    ref: str(o.ref, `${path}.ref`),
    sha256: sha256(o.sha256, `${path}.sha256`),
    byteLength: nonNegativeInt(o.byteLength, `${path}.byteLength`),
  }
}

const ARTIFACT_KEYS = [
  'contractVersion', 'artifactId', 'artifactKind', 'mediaType',
  'byteLength', 'sha256', 'storageRef', 'createdAt', 'expiresAt',
] as const

export function decodeArtifactRef(value: unknown, path: string): ArtifactRef {
  const o = obj(value, path)
  noExtraKeys(o, ARTIFACT_KEYS, path)
  const createdAt = timestamp(o.createdAt, `${path}.createdAt`)
  const expiresAt = o.expiresAt === null ? null : timestamp(o.expiresAt, `${path}.expiresAt`)
  // Mirrors AutoDesign's own refinement — an artifact that expires before it
  // exists is a bug in the producer, and saying so here is cheaper than a
  // consumer discovering it as a cache that never fills.
  if (expiresAt !== null && Date.parse(expiresAt) <= Date.parse(createdAt)) {
    fail(`${path}.expiresAt`, 'expiry must be strictly after creation')
  }
  return {
    contractVersion: literal(o.contractVersion, ARTIFACT_REF_CONTRACT_VERSION, `${path}.contractVersion`),
    artifactId: str(o.artifactId, `${path}.artifactId`),
    artifactKind: str(o.artifactKind, `${path}.artifactKind`),
    mediaType: str(o.mediaType, `${path}.mediaType`),
    byteLength: nonNegativeInt(o.byteLength, `${path}.byteLength`),
    sha256: sha256(o.sha256, `${path}.sha256`),
    storageRef: str(o.storageRef, `${path}.storageRef`),
    createdAt,
    expiresAt,
  }
}

const DIAGNOSTIC_KEYS = ['code', 'category', 'severity', 'retryable', 'safeMessage'] as const

export function decodeDiagnostic(value: unknown, path: string): Diagnostic {
  const o = obj(value, path)
  noExtraKeys(o, DIAGNOSTIC_KEYS, path)
  const code = str(o.code, `${path}.code`)
  if (!/^[A-Z][A-Z0-9_]{0,127}$/.test(code)) {
    fail(`${path}.code`, 'expected SCREAMING_SNAKE_CASE, at most 128 characters')
  }
  return {
    code,
    category: member(o.category, DIAGNOSTIC_CATEGORIES, `${path}.category`),
    severity: member(o.severity, ['warning', 'failure'] as const, `${path}.severity`),
    retryable: bool(o.retryable, `${path}.retryable`),
    safeMessage: str(o.safeMessage, `${path}.safeMessage`),
  }
}

const FIDELITY_KEYS = ['score', 'method', 'losses'] as const

export function decodeFidelity(value: unknown, path: string): FidelityReport {
  const o = obj(value, path)
  noExtraKeys(o, FIDELITY_KEYS, path)
  const method = member(o.method, ['unmeasured', 'rendered-comparison'] as const, `${path}.method`)
  const score = o.score
  // 🔴 The pairing is the whole point. An `unmeasured` report carrying a number
  // is the AAE failure exactly — a score that looks measured and is a tally
  // over the engine's own output. Refuse both halves of the contradiction.
  if (method === 'unmeasured') {
    if (score !== null) {
      fail(`${path}.score`, 'an unmeasured fidelity must report a null score, never a number')
    }
  } else if (typeof score !== 'number' || !(score >= 0 && score <= 1)) {
    fail(`${path}.score`, `method "${method}" must report a score between 0 and 1`)
  }
  return {
    score: score as number | null,
    method,
    losses: array(o.losses, `${path}.losses`).map((d, i) =>
      decodeDiagnostic(d, `${path}.losses[${i}]`),
    ),
  }
}

// ── the request ─────────────────────────────────────────────────────────────

const REQUEST_KEYS = ['contractVersion', 'operationId', 'operation', 'design', 'input'] as const

export function decodeRequest(value: unknown, path = 'request'): ContractRequest {
  const o = obj(value, path)
  noExtraKeys(o, REQUEST_KEYS, path)
  const operationRaw = str(o.operation, `${path}.operation`)
  // A reserved name is refused explicitly, so a caller reaching for `measure`
  // is told it is reserved rather than told it is unknown.
  if ((RESERVED_OPERATIONS as readonly string[]).includes(operationRaw)) {
    fail(
      `${path}.operation`,
      `"${operationRaw}" is reserved for a future version and is not implemented`,
    )
  }
  const operation = member(operationRaw, OPERATIONS, `${path}.operation`) as Operation
  const design = o.design === null ? null : decodeDesignRef(o.design, `${path}.design`)
  // `ingest` produces a design; every other operation acts on one. Stating it
  // here means a caller cannot get a confusing failure three steps later.
  if (operation === 'ingest' && design !== null) {
    fail(`${path}.design`, 'ingest produces a design and must not be given one')
  }
  if (operation !== 'ingest' && design === null) {
    fail(`${path}.design`, `${operation} acts on a design and requires one`)
  }
  return {
    contractVersion: literal(o.contractVersion, REQUEST_CONTRACT_VERSION, `${path}.contractVersion`),
    operationId: str(o.operationId, `${path}.operationId`),
    operation,
    design,
    input: obj(o.input, `${path}.input`),
  }
}

// ── the terminal event, and the three invariants that make `partial` mean something

const SETTLED_KEYS = [
  'contractVersion', 'type', 'operationId', 'operation', 'sequence',
  'observedAt', 'outcome', 'design', 'artifacts', 'diagnostics', 'fidelity',
] as const

export function decodeSettled(value: unknown, path = 'event'): SettledEvent {
  const o = obj(value, path)
  noExtraKeys(o, SETTLED_KEYS, path)
  const outcome = member(o.outcome, OUTCOMES, `${path}.outcome`)
  const design = o.design === null ? null : decodeDesignRef(o.design, `${path}.design`)
  const artifacts = array(o.artifacts, `${path}.artifacts`).map((a, i) =>
    decodeArtifactRef(a, `${path}.artifacts[${i}]`),
  )
  const diagnostics = array(o.diagnostics, `${path}.diagnostics`).map((d, i) =>
    decodeDiagnostic(d, `${path}.diagnostics[${i}]`),
  )
  const hasOutput = design !== null || artifacts.length > 0
  const failures = diagnostics.filter((d) => d.severity === 'failure')

  // 🔴 These three are the contract's substance, not its manners.
  //
  // AutoDesign enforces the equivalent pair in `lifecycle.ts:80-85` and it is
  // why a `degraded` result cannot cross their seam at all. `partial` is our
  // answer to that case, and it is only worth having if it is *evidenced* —
  // an unevidenced `partial` is precisely the old converter's behaviour of
  // reporting loss as success, which `PD0-04` exists to make impossible.
  if (outcome === 'complete') {
    if (!hasOutput) fail(`${path}.outcome`, 'a complete outcome must carry a design or artifacts')
    if (failures.length > 0) {
      fail(`${path}.diagnostics`, 'a complete outcome cannot carry a failure diagnostic')
    }
  } else if (outcome === 'partial') {
    if (!hasOutput) fail(`${path}.outcome`, 'a partial outcome must carry the output it partially produced')
    if (diagnostics.length === 0) {
      fail(`${path}.diagnostics`, 'a partial outcome must be evidenced by at least one diagnostic')
    }
  } else {
    if (hasOutput) fail(`${path}.outcome`, 'a held outcome cannot carry output')
    if (failures.length === 0) {
      fail(`${path}.diagnostics`, 'a held outcome must carry at least one failure diagnostic')
    }
  }

  return {
    contractVersion: literal(o.contractVersion, EVENT_CONTRACT_VERSION, `${path}.contractVersion`),
    type: literal(o.type, 'settled', `${path}.type`),
    operationId: str(o.operationId, `${path}.operationId`),
    operation: member(o.operation, OPERATIONS, `${path}.operation`),
    sequence: nonNegativeInt(o.sequence, `${path}.sequence`),
    observedAt: timestamp(o.observedAt, `${path}.observedAt`),
    outcome,
    design,
    artifacts,
    diagnostics,
    fidelity: o.fidelity === null ? null : decodeFidelity(o.fidelity, `${path}.fidelity`),
  }
}
