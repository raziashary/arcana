import { describe, it, expect } from 'vitest'
import {
  assertNoPixels,
  boundsHeight,
  boundsWidth,
  countLayers,
  designIrToJson,
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
