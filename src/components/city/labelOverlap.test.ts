import { describe, expect, it } from 'vitest'
import { declutterLabels, hiddenLabels, type LabelBox } from './labelOverlap'

const box = (id: number, priority: number, left: number, top: number, w = 100, h = 20): LabelBox => ({
  id, priority, left, top, right: left + w, bottom: top + h,
})

describe('hiddenLabels', () => {
  it('keeps the higher-priority label of an overlapping pair', () => {
    const hidden = hiddenLabels([box(0, 1, 0, 0), box(1, 9, 50, 5)])
    expect([...hidden]).toEqual([0])
  })

  it('keeps labels that do not touch, padding included', () => {
    expect(hiddenLabels([box(0, 1, 0, 0), box(1, 1, 200, 0), box(2, 1, 0, 100)]).size).toBe(0)
    // 100 wide ending at 100; the next starts at 103: inside the 4px padding.
    expect([...hiddenLabels([box(0, 2, 0, 0), box(1, 1, 103, 0)])]).toEqual([1])
  })

  it('does not let a hidden label hide another: only kept labels block', () => {
    // 1 overlaps 0 and is hidden; 2 overlaps only 1, so it stays.
    const hidden = hiddenLabels([box(0, 9, 0, 0), box(1, 5, 80, 0), box(2, 1, 170, 0)])
    expect([...hidden]).toEqual([1])
  })

  it('ignores a label that has not been laid out yet', () => {
    expect(hiddenLabels([box(0, 1, 0, 0, 0, 0), box(1, 1, 0, 0)]).size).toBe(0)
  })

  it('is stable when priorities tie: the lower id stays', () => {
    expect([...hiddenLabels([box(3, 1, 0, 0), box(1, 1, 10, 0)])]).toEqual([3])
  })

  it('breaks a tie by the label text, whatever the DOM order', () => {
    const a = { ...box(0, 1, 0, 0), tie: 'zeta' }
    const b = { ...box(1, 1, 10, 0), tie: 'alpha' }
    expect([...hiddenLabels([a, b])]).toEqual([0])
    expect([...hiddenLabels([b, a])]).toEqual([0])
  })
})

describe('declutterLabels', () => {
  function labelEl(text: string, priority: number, rect: [number, number, number, number]) {
    const el = document.createElement('div')
    el.dataset.cityLabel = text
    el.dataset.priority = String(priority)
    const [left, top, right, bottom] = rect
    el.getBoundingClientRect = () => ({ left, top, right, bottom, width: right - left, height: bottom - top, x: left, y: top, toJSON: () => ({}) })
    return el
  }

  it('hides the lower-priority label of an overlapping pair and keeps the rest', () => {
    const root = document.createElement('div')
    const big = labelEl('big', 9, [0, 0, 100, 20])
    const small = labelEl('small', 1, [50, 5, 150, 25])
    const apart = labelEl('apart', 1, [300, 0, 400, 20])
    root.append(big, small, apart)
    expect(declutterLabels(root)).toBe(1)
    expect([big, small, apart].map(e => e.style.visibility)).toEqual(['visible', 'hidden', 'visible'])
  })

  it('shows a hidden label again once nothing overlaps it', () => {
    const root = document.createElement('div')
    const big = labelEl('big', 9, [0, 0, 100, 20])
    const small = labelEl('small', 1, [50, 5, 150, 25])
    root.append(big, small)
    declutterLabels(root)
    expect(small.style.visibility).toBe('hidden')
    big.getBoundingClientRect = () => ({ left: 500, top: 0, right: 600, bottom: 20, width: 100, height: 20, x: 500, y: 0, toJSON: () => ({}) })
    expect(declutterLabels(root)).toBe(0)
    expect(small.style.visibility).toBe('visible')
  })

  it('leaves a label that has no box yet visible, and ignores unmarked elements', () => {
    const root = document.createElement('div')
    const pending = labelEl('pending', 1, [0, 0, 0, 0])
    const other = document.createElement('div')
    root.append(pending, other)
    expect(declutterLabels(root)).toBe(0)
    expect(pending.style.visibility).toBe('visible')
    expect(other.style.visibility).toBe('')
  })
})
