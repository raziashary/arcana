import { describe, it, expect } from 'vitest'
import type { Psd } from 'ag-psd'
import { countLayers, designIrToJson, type TextLayer } from './ir'
import { normalizeOpacity, normalizePsdText, psdToIr, unresolvedFonts } from './psdToIr'

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
