// The design intermediate representation (P3-46).
//
// ── Why this exists ──────────────────────────────────────────────────────────
//
// The converter's HTML output is 17–25 MB for a single social-media ad, because
// every layer is an embedded base64 PNG. That cannot enter a model's context, so
// an agent cannot adapt it. The spike measured exactly this: 17.41 MB for a
// 1080×1080 and 25.27 MB for a 1080×1920.
//
// The converter's own architecture document had already said what to do:
//
//   "the in-memory layer array is the universal intermediate representation …
//    Everything downstream — PSD, HTML, SVG, React, Flutter — is just a
//    different serialization of that same structured data."
//
// So the IR below is the agent-facing format and HTML is one serializer of it.
// Reformatting, rebranding and content-swapping are all mutations of this tree.
//
// ── The rule that keeps it small ─────────────────────────────────────────────
//
// **No pixels live in the IR.** An image layer carries an `assetId`, never the
// bitmap and never a data URI. Assets are held beside the IR and resolved only
// when a serializer needs them. Break this rule and the IR becomes the thing it
// was created to avoid — so `designIrToJson` asserts it (see `assertNoPixels`).

/** Pixel-space rectangle, PSD's own coordinate system: origin top-left. */
export interface Bounds {
  left: number
  top: number
  right: number
  bottom: number
}

export function boundsWidth(b: Bounds): number {
  return b.right - b.left
}

export function boundsHeight(b: Bounds): number {
  return b.bottom - b.top
}

/** sRGB, 0–255 per channel. PSD emits fractional channels; we keep them. */
export interface Rgb {
  r: number
  g: number
  b: number
}

/**
 * A run of characters sharing one style inside a text layer.
 *
 * These are real in production files, not a theoretical case: the spike found a
 * layer reading "Esport Esport Esport Esport" whose three runs alternate white
 * and yellow. A content swap that discards runs silently flattens that to one
 * colour, which is why `P3-51` names run preservation explicitly.
 */
export interface StyleRun {
  /** Character count this run covers, in the order runs appear. */
  length: number
  color?: Rgb
  fontName?: string
  fontSize?: number
  bold?: boolean
  italic?: boolean
}

export interface TextStyle {
  /** The PostScript name exactly as PSD stored it, e.g. `Montserrat-Bold`. */
  postScriptName: string
  /** Resolved CSS stack. Derived; `postScriptName` stays authoritative. */
  cssFontFamily: string
  /** Resolved CSS weight, 100–900, derived from the PostScript suffix. */
  cssFontWeight: string
  italic: boolean
  /** Already multiplied by the layer transform's Y scale — real rendered px. */
  fontSizePx: number
  color: Rgb
  /** PSD thousandths of an em. CSS letter-spacing = tracking / 1000 × size. */
  tracking: number
  leading?: number
  autoLeading: boolean
  justification: 'left' | 'center' | 'right' | 'justify'
  /** PSD `fontCaps`: 0 none, 1 small caps, 2 all caps. */
  fontCaps: number
  underline: boolean
  strikethrough: boolean
}

export type LayerKind = 'group' | 'text' | 'image' | 'shape' | 'empty'

interface LayerCommon {
  /** PSD layer id where present; otherwise a stable synthesised one. */
  id: string
  name: string
  bounds: Bounds
  /** 0–1. PSD stores 0–255 in some versions; normalised on the way in. */
  opacity: number
  /** A CSS `mix-blend-mode` value; unmapped PSD modes become `normal`. */
  blendMode: string
  hidden: boolean
}

export interface GroupLayer extends LayerCommon {
  kind: 'group'
  children: DesignLayer[]
  /** PSD "pass through" groups do not isolate blending. */
  passThrough: boolean
}

export interface TextLayer extends LayerCommon {
  kind: 'text'
  /** Line separators normalised to `\n` — PSD's raw `U+0003` never survives (P3-47). */
  text: string
  style: TextStyle
  styleRuns: StyleRun[]
  /** PSD's 6-element affine transform `[xx, xy, yx, yy, tx, ty]`. */
  transform: readonly [number, number, number, number, number, number]
}

export interface ImageLayer extends LayerCommon {
  kind: 'image'
  /** Key into the asset store. **Never** a data URI — see the header note. */
  assetId: string
  naturalWidth: number
  naturalHeight: number
}

export interface ShapeLayer extends LayerCommon {
  kind: 'shape'
  fill?: Rgb
}

export interface EmptyLayer extends LayerCommon {
  kind: 'empty'
}

export type DesignLayer = GroupLayer | TextLayer | ImageLayer | ShapeLayer | EmptyLayer

/** A font the design asks for, and whether we can actually honour it. */
export interface FontRequirement {
  postScriptName: string
  cssFamily: string
  weights: string[]
  /**
   * How the family will actually be rendered.
   *
   * `unresolved` is the case `P3-48` exists for: today the converter silently
   * substitutes `sans-serif` and the output looks successful while the design's
   * display typography is gone. An unresolved font must reach the caller as
   * data, not as a log line nobody reads.
   */
  resolution: 'google-fonts' | 'system-stack' | 'unresolved'
}

export interface DesignDocument {
  width: number
  height: number
  /** Source filename, for provenance. Not used for layout. */
  sourceName: string
  layers: DesignLayer[]
  fonts: FontRequirement[]
  /** Ids present in `layers`; the asset store must satisfy exactly these. */
  assetIds: string[]
}

/**
 * Depth-first walk over every layer, groups included, parents before children.
 */
export function walkLayers(
  layers: DesignLayer[],
  visit: (layer: DesignLayer, depth: number) => void,
  depth = 0,
): void {
  for (const layer of layers) {
    visit(layer, depth)
    if (layer.kind === 'group') walkLayers(layer.children, visit, depth + 1)
  }
}

export function countLayers(layers: DesignLayer[]): number {
  let n = 0
  walkLayers(layers, () => {
    n += 1
  })
  return n
}

/**
 * Guard the one invariant that gives the IR its reason to exist.
 *
 * Throws if any pixel payload has been smuggled into the tree. This is not
 * defensive decoration: the whole point of the IR is that it is small enough to
 * reason over, and the natural mistake — attaching the layer canvas "just for
 * preview" — silently restores the 17 MB problem. Cheaper to fail loudly.
 */
export function assertNoPixels(doc: DesignDocument): void {
  const offenders: string[] = []
  walkLayers(doc.layers, (layer) => {
    for (const [key, value] of Object.entries(layer)) {
      if (typeof value === 'string' && value.startsWith('data:')) {
        offenders.push(`${layer.name} (${layer.kind}) carries a data URI in "${key}"`)
      }
    }
  })
  if (offenders.length > 0) {
    throw new Error(
      `The design IR must not carry pixels — an image layer references an assetId ` +
        `and nothing more. Found ${offenders.length}: ${offenders.join('; ')}`,
    )
  }
}

/** Serialize for an agent. Refuses to emit a document carrying pixels. */
export function designIrToJson(doc: DesignDocument, pretty = false): string {
  assertNoPixels(doc)
  return JSON.stringify(doc, null, pretty ? 2 : undefined)
}
