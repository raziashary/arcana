import { describe, it, expect } from 'vitest'
import {
  baseFamilyName,
  cssFontFamily,
  cssFontWeight,
  googleFontsHref,
  isItalic,
  resolveFont,
} from './fonts'

// The cases below are taken from the P1 spike against two real ad PSDs, not
// invented: `gamead_1080x1080bahasa-Recovered.psd` and its 1080×1920 sibling
// requested Montserrat-Bold, Montserrat-Black, GimbalExtended-Regular and
// BruceForever-Regular.

describe('resolveFont — the honest answer P3-48 requires', () => {
  it('reports a font it cannot supply as unresolved', () => {
    // These two are exactly what the spike lost. The old converter returned a
    // sans-serif stack for them and said nothing, so the render looked finished
    // with the design's display typography gone.
    expect(resolveFont('GimbalExtended-Regular')).toBe('unresolved')
    expect(resolveFont('BruceForever-Regular')).toBe('unresolved')
  })

  it('reports a fetchable family as google-fonts', () => {
    expect(resolveFont('Montserrat-Bold')).toBe('google-fonts')
    expect(resolveFont('Montserrat-Black')).toBe('google-fonts')
  })

  it('reports an OS-guaranteed family as system-stack, not as a fallback', () => {
    // Arial really does render as Arial. Calling this "unresolved" would cry
    // wolf, and a warning that fires on correct output gets switched off.
    expect(resolveFont('Arial-BoldMT')).toBe('system-stack')
    expect(resolveFont('TimesNewRomanPS-BoldMT')).toBe('system-stack')
  })

  it('never claims resolution for an empty name', () => {
    expect(resolveFont('')).toBe('unresolved')
  })
})

describe('cssFontFamily', () => {
  it('quotes with single quotes only', () => {
    // This value is embedded inside style="…". A double quote terminates the
    // attribute early and silently drops every property after it.
    const stack = cssFontFamily('GimbalExtended-Regular')
    expect(stack).not.toContain('"')
    expect(stack).toContain("'Gimbal Extended'")
  })

  it('splits camel case and strips PostScript suffixes', () => {
    expect(baseFamilyName('GimbalExtended-Regular')).toBe('Gimbal Extended')
    expect(baseFamilyName('BruceForever-Regular')).toBe('Bruce Forever')
    expect(baseFamilyName('TimesNewRomanPS-BoldMT')).toBe('Times New Roman')
  })

  it('classifies an unknown family by its name', () => {
    expect(cssFontFamily('SomethingMono-Regular')).toContain('monospace')
    expect(cssFontFamily('OldCaslon-Regular')).toContain('serif')
    expect(cssFontFamily('GimbalExtended-Regular')).toContain('sans-serif')
  })

  it('falls back to sans-serif for an absent name rather than throwing', () => {
    expect(cssFontFamily('')).toBe('sans-serif')
  })
})

describe('cssFontWeight', () => {
  it('maps the PostScript suffixes the spike actually saw', () => {
    expect(cssFontWeight('Montserrat-Bold')).toBe('700')
    expect(cssFontWeight('Montserrat-Black')).toBe('900')
    expect(cssFontWeight('BruceForever-Regular')).toBe('400')
  })

  it('tests the more specific suffix first', () => {
    // Every one of these contains "bold". Ordering is load-bearing: a naive
    // /bold/ test first would call all of them 700.
    expect(cssFontWeight('Inter-ExtraBold')).toBe('600')
    expect(cssFontWeight('Inter-SemiBold')).toBe('600')
    expect(cssFontWeight('Inter-UltraBold')).toBe('900')
    expect(cssFontWeight('Inter-Bold')).toBe('700')
  })

  it('separates light from extralight', () => {
    expect(cssFontWeight('Roboto-ExtraLight')).toBe('100')
    expect(cssFontWeight('Roboto-Light')).toBe('300')
  })

  it('defaults to 400', () => {
    expect(cssFontWeight('')).toBe('400')
    expect(cssFontWeight('Roboto')).toBe('400')
  })
})

describe('isItalic', () => {
  it('recognises both spellings', () => {
    expect(isItalic('Georgia-Italic')).toBe(true)
    expect(isItalic('Helvetica-Oblique')).toBe(true)
    expect(isItalic('Montserrat-Bold')).toBe(false)
  })
})

describe('googleFontsHref', () => {
  it('sorts weights numerically, not lexicographically', () => {
    // ['900','700','100'].sort() gives 100;700;900 by luck, but ['90','700']
    // would not. Sorting by Number is the only correct comparator here.
    const href = googleFontsHref([
      { postScriptName: 'Montserrat-Bold', weights: ['900', '700', '100'] },
    ])
    expect(href).toContain('Montserrat:wght@100;700;900')
  })

  it('omits families with no Google source instead of requesting them', () => {
    const href = googleFontsHref([
      { postScriptName: 'Montserrat-Bold', weights: ['700'] },
      { postScriptName: 'GimbalExtended-Regular', weights: ['400'] },
    ])
    expect(href).toContain('Montserrat')
    expect(href).not.toContain('Gimbal')
  })

  it('returns undefined when nothing is fetchable, so no empty link is emitted', () => {
    expect(
      googleFontsHref([{ postScriptName: 'GimbalExtended-Regular', weights: ['400'] }]),
    ).toBeUndefined()
    expect(googleFontsHref([])).toBeUndefined()
  })

  it('does not request the same family twice', () => {
    const href = googleFontsHref([
      { postScriptName: 'Montserrat-Bold', weights: ['700'] },
      { postScriptName: 'Montserrat-Black', weights: ['900'] },
    ])
    expect(href!.match(/family=/g)).toHaveLength(1)
  })
})
