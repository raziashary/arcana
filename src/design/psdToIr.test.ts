import { describe, it, expect } from 'vitest'
import type { Psd } from 'ag-psd'
import { countLayers, designIrToJson, type TextLayer } from './ir'
import {
  createAssetStore,
  normalizeOpacity,
  normalizePsdText,
  psdToIr,
  unresolvedFonts,
} from './psdToIr'

// The fixtures below reproduce values observed in the P1 spike's debug output
// against two real ad PSDs, not invented shapes.

describe('normalizePsdText — P3-47', () => {
  it('turns PSD point-text separators into line breaks', () => {
    // The exact string the spike produced. Untranslated, U+0003 reaches the
    // browser as an unmapped control character and draws a visible glyph box:
    // "Minggu␃30 Des 2026-2027".
    const raw = 'Minggu\u000330 Des 2026-2027'
    expect(normalizePsdText(raw)).toBe('Minggu\n30 Des 2026-2027')
    expect(normalizePsdText(raw)).not.toContain('\u0003')
  })

  it('handles paragraph-text carriage returns too', () => {
    expect(normalizePsdText('a\rb')).toBe('a\nb')
    expect(normalizePsdText('a\r\nb')).toBe('a\nb')
  })

  it('does not collapse a CRLF into two breaks', () => {
    // Order matters: replacing \r before \r\n would double every Windows break.
    expect(normalizePsdText('a\r\nb').split('\n')).toHaveLength(2)
  })

  it('leaves ordinary newlines and text alone', () => {
    expect(normalizePsdText('Esport\nEsport')).toBe('Esport\nEsport')
    expect(normalizePsdText('')).toBe('')
  })

  it('handles several separators in one string', () => {
    expect(normalizePsdText('a\u0003b\u0003c')).toBe('a\nb\nc')
  })
})

describe('normalizeOpacity', () => {
  it('passes through the 0-1 form ag-psd v30 emits', () => {
    // Real values from the spike's debug trace.
    expect(normalizeOpacity(1)).toBe(1)
    expect(normalizeOpacity(0.5215686274509804)).toBeCloseTo(0.5216, 4)
  })

  it('rescales the 0-255 form older writers produce', () => {
    // Treating 128 as a ratio would clamp to fully opaque and silently discard
    // the design's transparency.
    expect(normalizeOpacity(255)).toBe(1)
    expect(normalizeOpacity(128)).toBeCloseTo(0.502, 3)
  })

  it('defaults to opaque rather than invisible when absent', () => {
    expect(normalizeOpacity(undefined)).toBe(1)
    expect(normalizeOpacity(Number.NaN)).toBe(1)
  })
})

function fakeCanvas(w: number, h: number): HTMLCanvasElement {
  return { width: w, height: h } as HTMLCanvasElement
}

function psdFixture(): Psd {
  return {
    width: 1080,
    height: 1080,
    children: [
      {
        id: 2468,
        name: 'Background',
        blendMode: 'pass through',
        opacity: 1,
        children: [
          {
            id: 2661,
            name: 'Layer 5',
            left: -818,
            top: -275,
            right: 2039,
            bottom: 1332,
            opacity: 1,
            blendMode: 'normal',
            canvas: fakeCanvas(2857, 1607),
          },
        ],
      },
      {
        id: 2553,
        name: 'Minggu 30 Des 2026-2027',
        left: 610,
        top: 943,
        right: 1029,
        bottom: 1029,
        opacity: 1,
        blendMode: 'normal',
        text: {
          text: 'Minggu\u000330 Des 2026-2027',
          transform: [11.3936, 0, 0, 11.3936, 1029.06, 975.29],
          style: {
            font: { name: 'Montserrat-Black' },
            fontSize: 3.85067,
            fillColor: { r: 255, g: 255, b: 255 },
            tracking: 0,
            autoLeading: true,
            fontCaps: 2,
          },
          paragraphStyle: { justification: 'right' },
        },
      },
      {
        id: 2429,
        name: 'Esport',
        left: 43,
        top: 99,
        right: 1030,
        bottom: 578,
        opacity: 1,
        blendMode: 'normal',
        text: {
          text: 'Esport\nEsport\nEsport',
          transform: [6.8179, 0, 0, 6.8179, 31.05, 198.99],
          style: { font: { name: 'GimbalExtended-Regular' }, fontSize: 20, tracking: -50 },
          styleRuns: [
            { length: 7, style: { fillColor: { r: 255, g: 255, b: 255 } } },
            { length: 7, style: { fillColor: { r: 255, g: 239.9958, b: 0.0051 } } },
            { length: 6, style: { fillColor: { r: 255, g: 255, b: 255 } } },
          ],
        },
      },
    ],
  } as unknown as Psd
}

describe('psdToIr', () => {
  it('counts groups and their children the way the spike reported them', () => {
    const { document } = psdToIr(psdFixture(), 'gamead.psd')
    expect(countLayers(document.layers)).toBe(4)
    expect(document.width).toBe(1080)
    expect(document.sourceName).toBe('gamead.psd')
  })

  it('applies P3-47 to text as it enters the IR', () => {
    const { document } = psdToIr(psdFixture(), 'x.psd')
    const dated = document.layers.find((l) => l.kind === 'text') as TextLayer
    expect(dated.text).toBe('Minggu\n30 Des 2026-2027')
  })

  it('multiplies font size by the layer transform, not the raw point size', () => {
    // 3.85067pt on a layer scaled 11.3936x renders at ~43.9px. Reading fontSize
    // alone would draw it at 4px, and transformed text is the common case here.
    const { document } = psdToIr(psdFixture(), 'x.psd')
    const dated = document.layers.find((l) => l.kind === 'text') as TextLayer
    expect(dated.style.fontSizePx).toBeCloseTo(43.87, 1)
  })

  it('preserves style runs, which a content swap must not flatten', () => {
    const { document } = psdToIr(psdFixture(), 'x.psd')
    const esport = document.layers.filter((l) => l.kind === 'text')[1] as TextLayer
    expect(esport.styleRuns).toHaveLength(3)
    expect(esport.styleRuns[1].color).toEqual({ r: 255, g: 239.9958, b: 0.0051 })
  })

  it('defaults an absent fill colour to black rather than dropping it', () => {
    const { document } = psdToIr(psdFixture(), 'x.psd')
    const esport = document.layers.filter((l) => l.kind === 'text')[1] as TextLayer
    expect(esport.style.color).toEqual({ r: 0, g: 0, b: 0 })
  })

  it('carries pixels in the asset store, never in the IR', () => {
    const { document, assets } = psdToIr(psdFixture(), 'x.psd')
    expect(assets.ids()).toHaveLength(1)
    expect(document.assetIds).toEqual(assets.ids())
    // The guard in designIrToJson would throw if a bitmap had leaked in.
    expect(() => designIrToJson(document)).not.toThrow()
  })

  it('keeps the serialized IR small enough for a model to read', () => {
    // The same design serialized as HTML was 17.41 MB.
    const { document } = psdToIr(psdFixture(), 'x.psd')
    expect(designIrToJson(document).length).toBeLessThan(4096)
  })

  it('normalises a pass-through group without losing the flag', () => {
    const { document } = psdToIr(psdFixture(), 'x.psd')
    const group = document.layers[0]
    expect(group.kind).toBe('group')
    expect(group.kind === 'group' && group.passThrough).toBe(true)
    expect(group.blendMode).toBe('normal')
  })
})

// ── A-13 · the falsy guard re-created the bug the function exists to fix ────

describe('renderedFontSizePx under a transform (A-13)', () => {
  function textPsd(transform: number[], fontSize = 20) {
    return {
      width: 1080,
      height: 1080,
      children: [
        {
          id: 1,
          name: 'Headline',
          left: 0,
          top: 0,
          right: 500,
          bottom: 200,
          opacity: 1,
          blendMode: 'normal',
          text: {
            text: 'Esport',
            transform,
            style: { font: { name: 'Montserrat-Bold' }, fontSize },
          },
        },
      ],
    } as unknown as Psd
  }

  function sizeOf(psd: Psd): number {
    const { document } = psdToIr(psd, 't.psd')
    return (document.layers[0] as TextLayer).style.fontSizePx
  }

  it('reads the scale of a plain scaled layer', () => {
    expect(sizeOf(textPsd([6.8179, 0, 0, 6.8179, 0, 0]))).toBeCloseTo(136.358, 2)
  })

  it('reads the scale of a layer rotated 90 degrees', () => {
    // 🔴 The defect, verbatim: for a 90° rotation yy = cos 90 = 0, and the old
    // `base * (scaleY || 1)` converted that legitimate scale into "no scale" —
    // reporting 20 px for text that renders at 136 px. That is the failure the
    // function's own docstring says it was written to prevent, re-created by
    // the guard meant to protect it.
    expect(sizeOf(textPsd([0, 6.8179, -6.8179, 0, 31.05, 198.99]))).toBeCloseTo(136.358, 2)
  })

  it('never reports a negative size for a vertically flipped layer', () => {
    // yy = -1 passed straight through, producing fontSizePx: -20.
    // `font-size: -20px` is invalid CSS, so it is dropped and the glyphs
    // silently inherit the parent size — a loss with no diagnostic.
    const size = sizeOf(textPsd([1, 0, 0, -1, 0, 0]))
    expect(size).toBeGreaterThan(0)
    expect(size).toBeCloseTo(20, 5)
  })

  it('falls back to the declared size when there is no usable transform', () => {
    expect(sizeOf(textPsd([]))).toBe(20)
  })
})

// ── A-18 · a duplicate PSD layer id overwrote another layer's bitmap ────────

describe('asset store collisions (A-18)', () => {
  it('refuses a second put on the same id rather than replacing the bitmap', () => {
    // Asset ids come from the PSD's own `lyid`. A merge/paste round-trip can
    // repeat one, and a bare `map.set` then made one layer render the other
    // layer's bitmap: `assetIds` listed the id once while two layers pointed
    // at it, and nothing reported it — not assertNoPixels, not
    // unresolvedFonts, not the asset-count assertion in this file.
    const store = createAssetStore()
    const canvasA = { width: 10, height: 10 } as HTMLCanvasElement
    const canvasB = { width: 99, height: 99 } as HTMLCanvasElement
    store.put('asset-42', canvasA)
    expect(() => store.put('asset-42', canvasB)).toThrow(/Duplicate asset id "asset-42"/)
    // The first layer's bitmap must survive the refusal.
    expect(store.get('asset-42')).toBe(canvasA)
    expect(store.ids()).toEqual(['asset-42'])
  })
})

// ── A-19 · a prototype-chained lookup answered for `toString` ───────────────

describe('blend mode lookup (A-19)', () => {
  function blendPsd(blendMode: string) {
    return {
      width: 100,
      height: 100,
      children: [
        {
          id: 1,
          name: 'L',
          left: 0,
          top: 0,
          right: 10,
          bottom: 10,
          opacity: 1,
          blendMode,
          text: { text: 'x', style: { font: { name: 'Arial' }, fontSize: 12 } },
        },
      ],
    } as unknown as Psd
  }

  it('maps a real blend mode', () => {
    const { document } = psdToIr(blendPsd('multiply'), 't.psd')
    expect(document.layers[0]!.blendMode).toBe('multiply')
  })

  it('returns normal for an inherited Object.prototype key, not a function', () => {
    // `BLEND_TO_CSS[mode] ?? 'normal'` — `??` only catches null/undefined, and
    // an object literal inherits Object.prototype, so `toString` returned an
    // inherited FUNCTION. `blendMode` is typed `string`; a function reached the
    // IR, JSON.stringify dropped it, and the field became undefined at runtime.
    for (const hostile of ['toString', 'constructor', 'valueOf', 'hasOwnProperty']) {
      const { document } = psdToIr(blendPsd(hostile), 't.psd')
      const mode = document.layers[0]!.blendMode
      expect(typeof mode, hostile).toBe('string')
      expect(mode, hostile).toBe('normal')
    }
  })

  it('returns normal for an unmapped mode', () => {
    const { document } = psdToIr(blendPsd('not-a-mode'), 't.psd')
    expect(document.layers[0]!.blendMode).toBe('normal')
  })
})

describe('unresolvedFonts — P3-48', () => {
  it('names the font the design will silently lose', () => {
    const { document } = psdToIr(psdFixture(), 'x.psd')
    const unresolved = unresolvedFonts(document)
    expect(unresolved.map((f) => f.postScriptName)).toEqual(['GimbalExtended-Regular'])
  })

  it('does not flag a font we can actually fetch', () => {
    const { document } = psdToIr(psdFixture(), 'x.psd')
    const montserrat = document.fonts.find((f) => f.postScriptName === 'Montserrat-Black')
    expect(montserrat?.resolution).toBe('google-fonts')
  })

  it('collects the weights each family is used at', () => {
    const { document } = psdToIr(psdFixture(), 'x.psd')
    const montserrat = document.fonts.find((f) => f.postScriptName === 'Montserrat-Black')
    expect(montserrat?.weights).toEqual(['900'])
  })
})
