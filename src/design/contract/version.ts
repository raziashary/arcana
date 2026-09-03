// PD0 — the capability contract's version identifiers.
//
// ── Why a string and not a number ────────────────────────────────────────────
//
// AutoDesign's own contract package versions every family with a namespaced
// literal, `<name>/<semver>`, and pins it with `z.literal` so a payload from an
// unknown version fails to decode rather than being parsed optimistically:
//
//   packages/auto-design-capability-contracts/src/capabilities.ts:4
//     export const CAPABILITY_CATALOG_CONTRACT_VERSION = 'ad-capability-catalog/0.2.0' as const
//
// We inherit the convention, not their types. Their `capabilityIdSchema` is a
// closed `z.enum` of eighteen AutoDesign capabilities and their host manifest
// declares `productOwner: z.literal('airs_auto_design')` — so AutoDev cannot
// publish a conforming manifest, and "write our contract to their shape" was
// never executable. What transfers is how they version, how they fail, and what
// they learned. See `_verify/PD0-CORRECTIONS.md`.
//
// ── Why each family carries its own ──────────────────────────────────────────
//
// A single global version forces every consumer to re-integrate when any one
// shape moves. Per-family versions let the artifact reference stay at 0.1.0
// while the request envelope goes to 0.2.0, which is the common case.

/** The request an AutoDesign-side caller sends. */
export const REQUEST_CONTRACT_VERSION = 'autodev-design-request/0.1.0' as const

/** Any event or terminal result AutoDev writes back. */
export const EVENT_CONTRACT_VERSION = 'autodev-design-event/0.1.0' as const

/** A reference to a design document held beside the wire, never on it. */
export const DESIGN_REF_CONTRACT_VERSION = 'autodev-design-ref/0.1.0' as const

/** A reference to a binary asset — image bytes, an exported file. */
export const ARTIFACT_REF_CONTRACT_VERSION = 'autodev-artifact-ref/0.1.0' as const

/**
 * The IR schema version carried inside a `DesignRef`.
 *
 * 🔴 `src/design/ir.ts`'s `DesignDocument` has **no version field of any kind**,
 * which the PD0 refutation found while checking the claim that the IR is
 * "published as a versioned type". The type existed; the versioning did not.
 * This is that version, and `decodeDesignRef` refuses a value it does not know
 * rather than parsing the document behind it.
 *
 * Bump this when `DesignDocument` changes shape in a way a reader would notice.
 */
export const IR_SCHEMA_VERSION = 'autodev-design-ir/0.1.0' as const

/**
 * Every contract version this build understands, by family.
 *
 * ⚠ Exported as data so a decoder cannot drift from the list, and so a consumer
 * can negotiate rather than guess. A hand-maintained `if` chain in each decoder
 * is how one family silently accepts a version the others reject.
 */
export const SUPPORTED_VERSIONS = {
  request: [REQUEST_CONTRACT_VERSION],
  event: [EVENT_CONTRACT_VERSION],
  designRef: [DESIGN_REF_CONTRACT_VERSION],
  artifactRef: [ARTIFACT_REF_CONTRACT_VERSION],
  ir: [IR_SCHEMA_VERSION],
} as const satisfies Record<string, readonly string[]>
