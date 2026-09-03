// PD0 — the AutoDev design capability contract.
//
// One import surface, so a consumer never has to know which file a symbol lives
// in and we can move things without breaking them.
//
// The contract in one place:
//
//   • Four operations — ingest · export · adapt · change. Frozen for v0.
//     `measure` is reserved; fidelity ships as a field, not a verb.
//   • The wire carries a **reference** to a design, never the design.
//   • An outcome is complete · partial · held, and `partial` must be evidenced.
//   • Fidelity states how it was measured, and `unmeasured` may not carry a score.
//   • Invocation is a spawned child process speaking NDJSON on stdout.
//
// The document written for the other side of the seam is
// `docs/restructurization/public-facing/AUTODESIGN-CAPABILITY-CONTRACT.md`.

export {
  OPERATIONS,
  RESERVED_OPERATIONS,
  OUTCOMES,
  DIAGNOSTIC_CATEGORIES,
  type Operation,
  type ReservedOperation,
  type Outcome,
  type DiagnosticCategory,
  type Diagnostic,
  type FidelityReport,
  type DesignRef,
  type ArtifactRef,
  type ProgressDetail,
  type ProgressEvent,
  type SettledEvent,
  type ContractEvent,
  type EventType,
  type ContractRequest,
} from './types'

export {
  REQUEST_CONTRACT_VERSION,
  EVENT_CONTRACT_VERSION,
  DESIGN_REF_CONTRACT_VERSION,
  ARTIFACT_REF_CONTRACT_VERSION,
  IR_SCHEMA_VERSION,
  SUPPORTED_VERSIONS,
} from './version'

export {
  ContractError,
  decodeRequest,
  decodeSettled,
  decodeDesignRef,
  decodeArtifactRef,
  decodeDiagnostic,
  decodeFidelity,
} from './decode'

export {
  runOperation,
  collect,
  referenceDesign,
  systemClock,
  fixedClock,
  type Clock,
} from './reference'

export {
  runConformance,
  requestFor,
  formatReport,
  type ContractDriver,
  type ConformanceReport,
  type CheckResult,
} from './conformance'

export { runInvocation, readAll, EXIT, type SidecarIo } from './sidecar'
