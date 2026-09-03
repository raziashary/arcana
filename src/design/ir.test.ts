import { describe, it, expect } from 'vitest'
import {
  assertNoPixels,
  boundsHeight,
  boundsWidth,
  countLayers,
  designIrToJson,
  MAX_LAYER_DEPTH,
  walkLayers,
  type DesignDocument,
  type DesignLayer,
} from './ir'

function doc(layers: DesignLayer[]): DesignDocument {
  return {
    width: 1080,
    height: 1080,
    sourceName: 'test.psd',
    layers,
    fonts: [],
    assetIds: [],
  }
}

const image = (id: string, assetId: string): DesignLayer => ({
  kind: 'image',
  id,
  name: id,
  bounds: { left: 0, top: 0, right: 100, bottom: 50 },
  opacity: 1,
  blendMode: 'normal',
  hidden: false,
  assetId,
  naturalWidth: 100,
  naturalHeight: 50,
})

describe('bounds', () => {
  it('derives size from PSD edge coordinates', () => {
    const b = { left: -1531, top: -632, right: 2977, bottom: 2538 }
    // Real bounds from the spike: layers legitimately extend outside the canvas.
    expect(boundsWidth(b)).toBe(4508)
    expect(boundsHeight(b)).toBe(3170)
  })
})

describe('walkLayers', () => {
  const tree: DesignLayer[] = [
    {
      kind: 'group',
      id: 'g1',
      name: 'Background',
      bounds: { left: 0, top: 0, right: 0, bottom: 0 },
      opacity: 1,
      blendMode: 'normal',
      hidden: false,
      passThrough: true,
      children: [image('i1', 'a1'), image('i2', 'a2')],
    },
    image('i3', 'a3'),
  ]

  it('visits parents before children, groups included', () => {
    const seen: Array<[string, number]> = []
    walkLayers(tree, (l, depth) => seen.push([l.id, depth]))
    expect(seen).toEqual([
      ['g1', 0],
      ['i1', 1],
      ['i2', 1],
      ['i3', 0],
    ])
  })

  it('counts groups as layers', () => {
    // The spike reported "37 layers" for a file with 7 top-level entries; that
    // count includes groups, so this must too or the two disagree.
    expect(countLayers(tree)).toBe(4)
  })
})

describe('assertNoPixels — the invariant that gives the IR its purpose', () => {
  it('accepts a document whose images are references', () => {
    expect(() => assertNoPixels(doc([image('i1', 'asset-1')]))).not.toThrow()
  })

  it('throws when a data URI is smuggled onto a layer', () => {
    // This is the natural mistake: attaching the layer canvas "just for
    // preview" restores the 17 MB problem the IR exists to avoid. The spike
    // measured 17.41 MB and 25.27 MB of exactly this.
    const leaky = {
      ...image('i1', 'asset-1'),
      assetId: 'data:image/png;base64,iVBORw0KGgo=',
    } as DesignLayer
    expect(() => assertNoPixels(doc([leaky]))).toThrow(/must not carry pixels/)
  })

  it('finds a data URI nested inside a group', () => {
    const leaky = {
      ...image('i1', 'asset-1'),
      assetId: 'data:image/png;base64,AAAA',
    } as DesignLayer
    const nested: DesignLayer = {
      kind: 'group',
      id: 'g',
      name: 'g',
      bounds: { left: 0, top: 0, right: 0, bottom: 0 },
      opacity: 1,
      blendMode: 'normal',
      hidden: false,
      passThrough: false,
      children: [leaky],
    }
    expect(() => assertNoPixels(doc([nested]))).toThrow(/must not carry pixels/)
  })

  it('names the offending layer so the error is actionable', () => {
    const leaky = {
      ...image('hero', 'asset-1'),
      assetId: 'data:image/png;base64,AAAA',
    } as DesignLayer
    expect(() => assertNoPixels(doc([leaky]))).toThrow(/hero/)
  })
})

// ── A-5 · the cases the first version of this guard could not see ───────────
//
// Every case below PASSED the original guard, which inspected only top-level
// own properties whose value was a string. They are the mistakes the guard's
// own docstring says it exists to catch.

describe('assertNoPixels — the shapes the guard used to miss (A-5)', () => {
  it('throws when the layer canvas is attached, which is the mistake the comment names', () => {
    // `return { ...common, kind: 'image', assetId, canvas }` in psdToIr — the
    // literal line the docstring warns about. A canvas is an object, so
    // `typeof value === 'string'` was false and the guard passed it. Worse:
    // JSON.stringify renders a canvas as `{}`, so the payload-size assertion
    // in psdToIr.test.ts stayed green as well. Two checks, one blind spot.
    class HTMLCanvasElement {
      width = 1080
      height = 1080
    }
    const leaky = { ...image('i1', 'asset-1'), canvas: new HTMLCanvasElement() } as unknown as DesignLayer
    expect(() => assertNoPixels(doc([leaky]))).toThrow(/must not carry pixels/)
    expect(() => assertNoPixels(doc([leaky]))).toThrow(/HTMLCanvasElement/)
  })

  it('throws on a binary payload of any class, not just a canvas', () => {
    for (const payload of [new Uint8Array([1, 2, 3]), new ArrayBuffer(8), new Map(), new Date()]) {
      const leaky = { ...image('i1', 'asset-1'), preview: payload } as unknown as DesignLayer
      expect(() => assertNoPixels(doc([leaky])), String(payload)).toThrow(/must not carry pixels/)
    }
  })

  it('finds a data URI nested below the top level', () => {
    // `Object.entries` is shallow, so a URI one level down — the natural place
    // for it — was invisible: style.backgroundImage, styleRuns[0].preview.
    const nestedObject = {
      ...image('i1', 'asset-1'),
      style: { backgroundImage: 'data:image/png;base64,AAAA' },
    } as unknown as DesignLayer
    expect(() => assertNoPixels(doc([nestedObject]))).toThrow(/style.backgroundImage/)

    const nestedArray = {
      ...image('i2', 'asset-2'),
      styleRuns: [{ preview: 'data:image/png;base64,AAAA' }],
    } as unknown as DesignLayer
    expect(() => assertNoPixels(doc([nestedArray]))).toThrow(/styleRuns.0.preview/)
  })

  it('catches a blob: URL as well as a data: URI', () => {
    const leaky = { ...image('i1', 'asset-1'), src: 'blob:http://x/abc' } as unknown as DesignLayer
    expect(() => assertNoPixels(doc([leaky]))).toThrow(/must not carry pixels/)
  })

  it('still accepts the legitimate document, so the guard has not become noise', () => {
    // The other half of a guard being real: it must not fire on correct input.
    // A guard that fires on everything is switched off, which is the same
    // outcome as one that fires on nothing.
    const ordinary: DesignLayer = {
      kind: 'text',
      id: 't',
      name: 'Headline',
      bounds: { left: 0, top: 0, right: 10, bottom: 10 },
      opacity: 1,
      blendMode: 'normal',
      hidden: false,
      text: 'Hello',
      style: {
        postScriptName: 'Montserrat-Bold',
        cssFontFamily: "'Montserrat',sans-serif",
        cssFontWeight: '700',
        italic: false,
        fontSizePx: 24,
        color: { r: 0, g: 0, b: 0 },
        letterSpacingEm: 0,
        lineHeightPx: null,
        align: 'left',
        uppercase: false,
      },
      styleRuns: [{ length: 5, color: { r: 0, g: 0, b: 0 } }],
      transform: [1, 0, 0, 1, 0, 0],
    } as unknown as DesignLayer
    expect(() => assertNoPixels(doc([ordinary, image('i', 'asset-1')]))).not.toThrow()
  })
})

// ── A-20 · the walk had no depth bound and no cycle detection ───────────────

describe('walkLayers depth bound (A-20)', () => {
  function nest(depth: number): DesignLayer {
    let node: DesignLayer = image('leaf', 'asset-leaf')
    for (let i = 0; i < depth; i += 1) {
      node = {
        kind: 'group',
        id: `g${i}`,
        name: `g${i}`,
        bounds: { left: 0, top: 0, right: 0, bottom: 0 },
        opacity: 1,
        blendMode: 'normal',
        hidden: false,
        passThrough: false,
        children: [node],
      }
    }
    return node
  }

  it('walks a deep but legitimate tree', () => {
    expect(countLayers([nest(MAX_LAYER_DEPTH - 2)])).toBe(MAX_LAYER_DEPTH - 1)
  })

  it('refuses past the bound instead of overflowing the stack', () => {
    expect(() => countLayers([nest(MAX_LAYER_DEPTH + 5)])).toThrow(RangeError)
    expect(() => countLayers([nest(MAX_LAYER_DEPTH + 5)])).toThrow(/malformed or cyclic/)
  })

  it('terminates on a cyclic tree rather than hanging', () => {
    // A corrupt file can produce this; before the bound it was a stack
    // overflow, which behind the sidecar becomes `UNSETTLED: RangeError` with
    // no diagnostic — bypassing the whole failure taxonomy the contract has.
    const group = {
      kind: 'group',
      id: 'cycle',
      name: 'cycle',
      bounds: { left: 0, top: 0, right: 0, bottom: 0 },
      opacity: 1,
      blendMode: 'normal',
      hidden: false,
      passThrough: false,
      children: [] as DesignLayer[],
    } as DesignLayer & { children: DesignLayer[] }
    group.children.push(group)
    // 🔴 Asserting only `toThrow(RangeError)` here would be a check that cannot
    // express the defect: an unbounded walk of a cycle overflows the stack,
    // and "Maximum call stack size exceeded" IS a RangeError — so that
    // assertion passed against the BROKEN code when it was first written.
    // Caught by running it against the pre-fix file, which is why §0.2m makes
    // falsification a rule rather than a habit. The message is the only thing
    // that distinguishes a deliberate refusal from a crash.
    expect(() => countLayers([group])).toThrow(/malformed or cyclic/)
    expect(() => countLayers([group])).not.toThrow(/call stack/)
  })
})

describe('designIrToJson', () => {
  it('round-trips a clean document', () => {
    const d = doc([image('i1', 'asset-1')])
    expect(JSON.parse(designIrToJson(d))).toEqual(d)
  })

  it('refuses to serialize a document carrying pixels', () => {
    // Serialization is the last place to catch this before it reaches a model's
    // context, so the guard runs here rather than only at construction.
    const leaky = {
      ...image('i1', 'a'),
      assetId: 'data:image/png;base64,AAAA',
    } as DesignLayer
    expect(() => designIrToJson(doc([leaky]))).toThrow(/must not carry pixels/)
  })

  it('stays small enough to be worth having', () => {
    // The whole premise. 40 layers of IR must be orders of magnitude under the
    // 17.41 MB the HTML serializer produced for 37 layers.
    const many = Array.from({ length: 40 }, (_, i) => image(`i${i}`, `asset-${i}`))
    expect(designIrToJson(doc(many)).length).toBeLessThan(32 * 1024)
  })
})
