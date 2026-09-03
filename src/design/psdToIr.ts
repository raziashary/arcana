// PSD → the design IR (P3-46), with the U+0003 fix (P3-47) and honest font
// reporting (P3-48).
//
// The extraction rules below are taken from the working converter in
// `apps/psdconversions/index.html`, which was tuned against real ad PSDs and
// verified visually during the P1 spike. Where this file differs from that one
// the difference is deliberate and commented.
//
// The largest deliberate difference: that converter walks the layer tree and
// emits an HTML string in one pass, embedding every layer bitmap as a base64
// PNG — 17.41 MB for a 1080×1080 ad. This walk emits structured data and hands
// bitmaps to an asset store keyed by id, so the tree an agent reads stays a few
// KB. See `ir.ts` for why that is the whole point.

import type { Layer, Psd } from 'ag-psd'
import {
  cssFontFamily,
  cssFontWeight,
  isItalic,
  resolveFont,
  type FontResolution,
} from './fonts'
import type {
  Bounds,
  DesignDocument,
  DesignLayer,
  FontRequirement,
  Rgb,
  StyleRun,
  TextStyle,
} from './ir'

/**
 * Where layer bitmaps go, so they stay out of the IR.
 *
 * The IR holds `assetId`; this holds the pixels. Serializers resolve ids
 * through it at the moment they need them, which is what lets the same IR
 * produce a 25 MB standalone HTML file or a 4 KB JSON payload for a model.
 */
export interface AssetStore {
  put(id: string, canvas: HTMLCanvasElement): void
  get(id: string): HTMLCanvasElement | undefined
  ids(): string[]
}

export function createAssetStore(): AssetStore {
  const map = new Map<string, HTMLCanvasElement>()
  return {
    /**
     * 🔴 `A-18`: this was a bare `map.set`, and asset ids are derived from the
     * PSD's own `lyid` (`asset-${layer.id}`). A file whose layer ids repeat —
     * which a merge/paste round-trip produces — put two image layers on one
     * key, so the second canvas replaced the first and one layer rendered the
     * other layer's bitmap. `assetIds` listed the id once while two layers
     * referenced it, and nothing reported it: not `assertNoPixels`, not
     * `unresolvedFonts`, not the asset-count assertion in the tests.
     *
     * Refusing is right rather than renaming: a duplicate id means the file's
     * identifiers are not identifiers, and silently disambiguating them would
     * hide that from the diagnostic the conversion owes the user.
     */
    put: (id, canvas) => {
      if (map.has(id)) {
        throw new Error(
          `Duplicate asset id "${id}" — two layers claim the same PSD layer id. ` +
            `Storing it would make one layer render the other's bitmap.`,
        )
      }
      map.set(id, canvas)
    },
    get: (id) => map.get(id),
    ids: () => [...map.keys()],
  }
}

/** PSD blend mode → CSS `mix-blend-mode`. Unmapped modes render `normal`. */
const BLEND_TO_CSS: Readonly<Record<string, string>> = {
  normal: 'normal',
  multiply: 'multiply',
  screen: 'screen',
  overlay: 'overlay',
  darken: 'darken',
  lighten: 'lighten',
  'color-dodge': 'color-dodge',
  'color-burn': 'color-burn',
  'hard-light': 'hard-light',
  'soft-light': 'soft-light',
  difference: 'difference',
  exclusion: 'exclusion',
  hue: 'hue',
  saturation: 'saturation',
  color: 'color',
  luminosity: 'luminosity',
}

/**
 * Normalise PSD text to `\n` line breaks. **This is P3-47.**
 *
 * Photoshop separates lines in *point* text with `U+0003` (END OF TEXT), not a
 * newline. Passed through untouched it reaches the browser as an unmapped
 * control character and renders as a visible glyph box — the P1 spike caught it
 * in production output as `"Minggu␃30 Des 2026-2027"`, in both the 1080×1080
 * and 1080×1920 conversions.
 *
 * `\r` is handled for the same reason: Photoshop's *paragraph* text uses it.
 */
export function normalizePsdText(raw: string): string {
  return (
    raw
      .replace(/\r\n/g, '\n')
      // Written as an escape, never as the literal character. A raw U+0003
      // in source is invisible in every editor, diff and review -- and this
      // line IS the fix, so it must be the most legible line in the file.
      //
      // `no-control-regex` exists to catch a control character that arrived
      // by accident. This one is the entire point of the function, so the
      // rule is disabled on this line specifically and nowhere else.
      // eslint-disable-next-line no-control-regex
      .replace(/\u0003/g, '\n')
      .replace(/\r/g, '\n')
  )
}

/**
 * PSD opacity → 0–1.
 *
 * ag-psd v30 emits 0–1 already, but older files and some writers produce 0–255.
 * Treating a 0–255 value as a ratio would make every layer 255× opaque, which
 * clamps to fully visible and silently discards the design's transparency — so
 * the ambiguity is resolved rather than assumed. A value above 1 can only have
 * come from the 0–255 encoding.
 */
export function normalizeOpacity(raw: number | undefined): number {
  if (typeof raw !== 'number' || Number.isNaN(raw)) return 1
  if (raw > 1) return Math.min(1, raw / 255)
  return Math.max(0, raw)
}

function toBounds(layer: Layer): Bounds {
  return {
    left: layer.left ?? 0,
    top: layer.top ?? 0,
    right: layer.right ?? 0,
    bottom: layer.bottom ?? 0,
  }
}

function toRgb(color: unknown): Rgb | undefined {
  if (!color || typeof color !== 'object') return undefined
  const c = color as { r?: number; g?: number; b?: number }
  if (typeof c.r !== 'number' || typeof c.g !== 'number' || typeof c.b !== 'number') {
    return undefined
  }
  return { r: c.r, g: c.g, b: c.b }
}

/** PSD's justification values, narrowed to the CSS ones we emit. */
function toJustification(raw: unknown): TextStyle['justification'] {
  return raw === 'center' || raw === 'right' || raw === 'justify' ? raw : 'left'
}

type PsdText = NonNullable<Layer['text']>

/**
 * Rendered font size in pixels.
 *
 * PSD stores the size in points *before* the layer transform, so a layer scaled
 * 6.8× carries `fontSize: 20` and renders at 136 px. Reading `fontSize` alone —
 * the obvious thing to do — produces text at a fraction of its real size on
 * every transformed layer, and transformed text layers are the common case in
 * these files, not the exception.
 */
function renderedFontSizePx(text: PsdText): number {
  const base = text.style?.fontSize ?? 12
  const transform = text.transform
  if (!Array.isArray(transform)) return base
  const yx = transform[2]
  const yy = transform[3]
  if (typeof yx !== 'number' || typeof yy !== 'number') return base
  // 🔴 `A-13`: this was `base * (scaleY || 1)`, reading only `transform[3]`.
  //
  // The transform is `[xx, xy, yx, yy, tx, ty]`, so `yy` alone is the vertical
  // scale ONLY for an unrotated layer. Rotate a layer 90° and `yy = cos 90 = 0`
  // — whereupon `0 || 1` converted a legitimate scale into "no scale", and a
  // headline scaled 6.8× and rotated reported `fontSizePx: 20` while rendering
  // at 136 px. That is verbatim the failure this function's own docstring says
  // it was written to prevent, re-created by the falsy-guard. A vertically
  // flipped layer (`yy = -1`) passed straight through and produced
  // `fontSizePx: -20`; `font-size: -20px` is invalid CSS, so it is dropped and
  // the glyphs silently inherit their size from the parent.
  //
  // The magnitude of the transformed unit-y vector is the scale under any
  // rotation, and it is never negative.
  const scale = Math.hypot(yx, yy)
  return scale > 0 ? base * scale : base
}

function toStyleRuns(text: PsdText): StyleRun[] {
  const runs = text.styleRuns
  if (!Array.isArray(runs)) return []
  return runs.map((run) => {
    const style = (run.style ?? {}) as {
      font?: { name?: string }
      fontSize?: number
      fauxBold?: boolean
      fauxItalic?: boolean
      fillColor?: unknown
    }
    const out: StyleRun = { length: run.length ?? 0 }
    const color = toRgb(style.fillColor)
    if (color) out.color = color
    if (style.font?.name) out.fontName = style.font.name
    if (typeof style.fontSize === 'number') out.fontSize = style.fontSize
    if (typeof style.fauxBold === 'boolean') out.bold = style.fauxBold
    if (typeof style.fauxItalic === 'boolean') out.italic = style.fauxItalic
    return out
  })
}

function toTextStyle(text: PsdText): TextStyle {
  const style = text.style ?? {}
  const postScriptName = style.font?.name ?? ''
  const paragraph = text.paragraphStyle ?? {}
  return {
    postScriptName,
    cssFontFamily: cssFontFamily(postScriptName),
    cssFontWeight: style.fauxBold ? '700' : cssFontWeight(postScriptName),
    italic: Boolean(style.fauxItalic) || isItalic(postScriptName),
    fontSizePx: renderedFontSizePx(text),
    // PSD leaves fillColor undefined for layers whose colour comes only from
    // style runs. Black is the Photoshop default and matches the converter.
    color: toRgb(style.fillColor) ?? { r: 0, g: 0, b: 0 },
    tracking: style.tracking ?? 0,
    leading: typeof style.leading === 'number' ? style.leading : undefined,
    autoLeading: style.autoLeading !== false,
    justification: toJustification(paragraph.justification),
    fontCaps: style.fontCaps ?? 0,
    underline: Boolean(style.underline),
    strikethrough: Boolean(style.strikethrough),
  }
}

function layerKindOf(layer: Layer): DesignLayer['kind'] {
  if (layer.children && layer.children.length > 0) return 'group'
  if (layer.text) return 'text'
  if (layer.canvas) return 'image'
  if ((layer as { vectorFill?: unknown }).vectorFill) return 'shape'
  return 'empty'
}

interface ConvertState {
  assets: AssetStore
  /** PostScript name → the weights the design asks for. */
  fonts: Map<string, Set<string>>
  nextSyntheticId: number
}

/**
 * PSD blend mode → CSS, safely.
 *
 * 🔴 `A-19`: the call site was `BLEND_TO_CSS[mode ?? 'normal'] ?? 'normal'`.
 * `??` only catches `null` and `undefined`, and an object literal inherits
 * `Object.prototype` — so a blend mode string of `toString`, `constructor`,
 * `valueOf` or `hasOwnProperty` returned an inherited **function**, which is
 * not nullish and so survived the fallback. `blendMode` is typed `string`;
 * what reached the IR was a function. `JSON.stringify` drops it, so the field
 * silently becomes `undefined` at runtime, and a non-JSON serializer would
 * emit function source into a CSS attribute.
 *
 * `Object.hasOwn` removes the class rather than patching the instance.
 */
function blendModeToCss(mode: string | undefined): string {
  const key = mode ?? 'normal'
  // `Object.hasOwn` would read better; the project targets below ES2022, so
  // this is the same check spelled in the form the target supports.
  return Object.prototype.hasOwnProperty.call(BLEND_TO_CSS, key) ? BLEND_TO_CSS[key]! : 'normal'
}

function convertLayer(layer: Layer, state: ConvertState): DesignLayer {
  const id =
    layer.id !== undefined && layer.id !== null
      ? String(layer.id)
      : `syn-${state.nextSyntheticId++}`
  const common = {
    id,
    name: layer.name ?? 'Layer',
    bounds: toBounds(layer),
    opacity: normalizeOpacity(layer.opacity),
    blendMode: blendModeToCss(layer.blendMode),
    hidden: Boolean(layer.hidden),
  }

  switch (layerKindOf(layer)) {
    case 'group':
      return {
        ...common,
        kind: 'group',
        // "pass through" groups do not isolate blending; CSS has no direct
        // equivalent, so the flag is preserved for the serializer to decide.
        passThrough: layer.blendMode === 'pass through',
        children: (layer.children ?? []).map((child) => convertLayer(child, state)),
      }

    case 'text': {
      const text = layer.text as PsdText
      const style = toTextStyle(text)
      if (style.postScriptName) {
        const weights = state.fonts.get(style.postScriptName) ?? new Set<string>()
        weights.add(style.cssFontWeight)
        state.fonts.set(style.postScriptName, weights)
      }
      return {
        ...common,
        kind: 'text',
        text: normalizePsdText(text.text ?? ''),
        style,
        styleRuns: toStyleRuns(text),
        transform: toTransform(text.transform),
      }
    }

    case 'image': {
      const canvas = layer.canvas as HTMLCanvasElement
      const assetId = `asset-${id}`
      state.assets.put(assetId, canvas)
      return {
        ...common,
        kind: 'image',
        assetId,
        naturalWidth: canvas.width,
        naturalHeight: canvas.height,
      }
    }

    case 'shape':
      return {
        ...common,
        kind: 'shape',
        fill: toRgb((layer as { vectorFill?: { color?: unknown } }).vectorFill?.color),
      }

    default:
      return { ...common, kind: 'empty' }
  }
}

type TextLayerTransform = readonly [number, number, number, number, number, number]
/**
 * PSD's 6-element affine transform, as a tuple.
 *
 * Built element by element rather than asserted from `number[]`: a cast
 * would compile against an array of any length, and a short or absent
 * transform would then reach the serializer as a malformed tuple and
 * mis-scale every glyph on the layer. Identity is the correct default.
 */
function toTransform(raw: unknown): TextLayerTransform {
  if (!Array.isArray(raw) || raw.length !== 6 || raw.some((n) => typeof n !== "number")) {
    return [1, 0, 0, 1, 0, 0]
  }
  const t = raw as number[]
  return [t[0], t[1], t[2], t[3], t[4], t[5]]
}


export interface ConversionResult {
  document: DesignDocument
  assets: AssetStore
}

/**
 * Convert a parsed PSD into the IR plus a side-table of layer bitmaps.
 *
 * The two come back together because neither is useful alone: the IR describes
 * a design it cannot render, and the assets are pixels with no arrangement.
 * Keeping them separate is what makes the IR small enough to hand to a model.
 */
export function psdToIr(psd: Psd, sourceName: string): ConversionResult {
  const state: ConvertState = {
    assets: createAssetStore(),
    fonts: new Map(),
    nextSyntheticId: 1,
  }

  const layers = (psd.children ?? []).map((child) => convertLayer(child, state))

  const fonts: FontRequirement[] = [...state.fonts.entries()].map(
    ([postScriptName, weights]) => ({
      postScriptName,
      cssFamily: cssFontFamily(postScriptName),
      weights: [...weights].sort((a, b) => Number(a) - Number(b)),
      resolution: resolveFont(postScriptName) as FontResolution,
    }),
  )

  return {
    document: {
      width: psd.width,
      height: psd.height,
      sourceName,
      layers,
      fonts,
      assetIds: state.assets.ids(),
    },
    assets: state.assets,
  }
}

/**
 * The fonts this design asks for that we cannot actually supply.
 *
 * Callers must surface this. A conversion that drops the designer's typeface
 * and still reports success is the defect `P3-48` exists to close, and it is
 * only closed if something downstream reads this.
 */
export function unresolvedFonts(doc: DesignDocument): FontRequirement[] {
  return doc.fonts.filter((font) => font.resolution === 'unresolved')
}
