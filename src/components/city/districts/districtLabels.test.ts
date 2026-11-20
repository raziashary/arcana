import { describe, expect, it } from 'vitest'
import { districtLabel, districtLabels } from './districtLabels'

describe('districtLabel', () => {
  it('upper-cases a kebab-case name', () => {
    expect(districtLabel('tabs-chatheader', 1)).toBe('TABS CHATHEADER')
  })
  it('shows the folder for a legacy path name, either separator', () => {
    expect(districtLabel('module-c:\\work\\src\\city\\a.ts', 2)).toBe('CITY')
    expect(districtLabel('module-/work/src/city/a.ts', 2)).toBe('CITY')
  })
  it('falls back to the id when there is no name', () => {
    expect(districtLabel('', 7)).toBe('DISTRICT 7')
  })
})

describe('districtLabels', () => {
  it('leaves unique labels alone', () => {
    const labels = districtLabels([
      { communityId: 1, name: 'tabs-chat' },
      { communityId: 2, name: 'tabs-docs' },
    ])
    expect([...labels.values()]).toEqual(['TABS CHAT', 'TABS DOCS'])
  })

  it('numbers every district that shares a label, and only those', () => {
    const labels = districtLabels([
      { communityId: 1, name: 'module-c:\\w\\src\\city\\a.ts' },
      { communityId: 2, name: 'module-c:\\w\\src\\city\\b.ts' },
      { communityId: 3, name: 'tabs-docs' },
    ])
    expect(labels.get(1)).toBe('CITY #1')
    expect(labels.get(2)).toBe('CITY #2')
    expect(labels.get(3)).toBe('TABS DOCS')
    expect(new Set(labels.values()).size).toBe(3)
  })
})
