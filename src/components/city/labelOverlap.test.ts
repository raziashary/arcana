import { describe, expect, it } from 'vitest'
import { hiddenLabels, type LabelBox } from './labelOverlap'

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
})
