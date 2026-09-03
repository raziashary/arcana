import { describe, it, expect } from 'vitest'
import {
  baseFamilyName,
  cssFontFamily,
  cssFontWeight,
  googleFontsHrefs,
  googleFontToken,
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
    //
    // 🔴 **These three assertions used to read 600 / 600 / 900, and they were
    // wrong** — corrected with the A-12 fix. The test was written from the
    // implementation rather than from the CSS weight scale, so it pinned the
    // defect in place and reported green over it: ExtraBold and UltraBold are
    // one physical face at 800, and the old pair put them three steps apart.
    // `L-C6` — a check written from the CLAIM rather than from the INVARIANT
    // it protects — is this programme's most-repeated defect class, and this
    // is an instance of it in a file whose own docstring named the ordering
    // rule correctly while getting the values wrong.
    expect(cssFontWeight('Inter-ExtraBold')).toBe('800')
    expect(cssFontWeight('Inter-SemiBold')).toBe('600')
    expect(cssFontWeight('Inter-UltraBold')).toBe('800')
    expect(cssFontWeight('Inter-Bold')).toBe('700')
  })

  it('separates light from extralight', () => {
    // Also corrected: CSS ExtraLight is 200, and only Thin/Hairline is 100.
    // The old assertion collapsed two distinct weights onto one value.
    expect(cssFontWeight('Roboto-ExtraLight')).toBe('200')
    expect(cssFontWeight('Roboto-Thin')).toBe('100')
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

describe('googleFontsHrefs', () => {
  it('sorts weights numerically, not lexicographically', () => {
    // ['900','700','100'].sort() gives 100;700;900 by luck, but ['90','700']
    // would not. Sorting by Number is the only correct comparator here.
    const hrefs = googleFontsHrefs([
      { postScriptName: 'Montserrat-Bold', weights: ['900', '700', '100'] },
    ])
    expect(hrefs[0]).toContain('Montserrat:wght@100;700;900')
  })

  it('omits families with no Google source instead of requesting them', () => {
    const hrefs = googleFontsHrefs([
      { postScriptName: 'Montserrat-Bold', weights: ['700'] },
      { postScriptName: 'GimbalExtended-Regular', weights: ['400'] },
    ])
    expect(hrefs).toHaveLength(1)
    expect(hrefs[0]).toContain('Montserrat')
    expect(hrefs.join(' ')).not.toContain('Gimbal')
  })

  it('returns nothing when nothing is fetchable, so no empty link is emitted', () => {
    expect(googleFontsHrefs([{ postScriptName: 'GimbalExtended-Regular', weights: ['400'] }]))
      .toEqual([])
    expect(googleFontsHrefs([])).toEqual([])
  })

  it('does not request the same family twice', () => {
    const hrefs = googleFontsHrefs([
      { postScriptName: 'Montserrat-Bold', weights: ['700'] },
      { postScriptName: 'Montserrat-Black', weights: ['900'] },
    ])
    expect(hrefs).toHaveLength(1)
  })

  it('isolates each family in its own request — A-17', () => {
    // 🔴 The audit's scenario, made a test. Google Fonts 400s the WHOLE css2
    // request when one wght value does not exist for its family, and psdToIr
    // forces '700' for any faux-bolded layer regardless of what the family
    // ships. Bebas Neue ships 400 only. One combined URL therefore lost every
    // font in the document to one faux-bolded headline, and unresolvedFonts()
    // reported nothing because all of them had resolved.
    const hrefs = googleFontsHrefs([
      { postScriptName: 'BebasNeue-Regular', weights: ['700'] },
      { postScriptName: 'Montserrat-Bold', weights: ['700'] },
      { postScriptName: 'Roboto-Regular', weights: ['400'] },
    ])
    expect(hrefs).toHaveLength(3)
    for (const href of hrefs) {
      expect(href.match(/family=/g)).toHaveLength(1)
    }
    // The failure this prevents: a doomed Bebas request must not be able to
    // carry Montserrat down with it.
    const doomed = hrefs.filter((h) => h.includes('Bebas'))
    expect(doomed).toHaveLength(1)
    expect(doomed[0]).not.toContain('Montserrat')
  })
})

// ── The area audit's font findings, each reproduced before it was fixed ──────

describe('two-word weight suffixes — A-4', () => {
  it('strips ExtraBold rather than splitting it into the family name', () => {
    // Reproduced by the audit: baseFamilyName('Montserrat-ExtraBold') returned
    // 'Montserrat-Extra Bold', so cssFontFamily emitted a family that exists
    // nowhere and resolveFont reported one of the most common headline faces
    // in ad PSDs as unresolved — a FALSE LOSS in the field P3-48 created to
    // stop false success. Montserrat is the first entry in GOOGLE_FONTS.
    expect(baseFamilyName('Montserrat-ExtraBold')).toBe('Montserrat')
    expect(resolveFont('Montserrat-ExtraBold')).toBe('google-fonts')
    expect(cssFontFamily('Montserrat-ExtraBold')).toContain("'Montserrat'")
  })

  it('strips every other two-word weight name the first list omitted', () => {
    for (const name of [
      'Montserrat-UltraBold',
      'Montserrat-ExtraLight',
      'Montserrat-UltraLight',
      'Montserrat-Thin',
      'Montserrat-Heavy',
      'Montserrat-Book',
      'Montserrat-Demi',
      'Montserrat-ExtraBlack',
      'Montserrat-SemiBoldItalic',
    ]) {
      expect(baseFamilyName(name), name).toBe('Montserrat')
      expect(resolveFont(name), name).toBe('google-fonts')
    }
  })

  it('still splits a genuine camel-case family, which the strip must not eat', () => {
    // The guard against over-correcting: Gimbal Extended is a real two-word
    // family, not a weight suffix, and must keep its space.
    expect(baseFamilyName('GimbalExtended-Regular')).toBe('Gimbal Extended')
    expect(resolveFont('GimbalExtended-Regular')).toBe('unresolved')
  })
})

describe('CSS weight values — A-12', () => {
  it('gives ExtraBold and UltraBold the same weight, because they are one face', () => {
    // The defect: ultrabold sat in the 900 branch and extrabold in the 600
    // branch beside semibold. A design using both names for the same physical
    // face rendered them three steps apart.
    expect(cssFontWeight('Montserrat-ExtraBold')).toBe('800')
    expect(cssFontWeight('Montserrat-UltraBold')).toBe('800')
  })

  it('follows the CSS scale for every named weight', () => {
    const expected: ReadonlyArray<readonly [string, string]> = [
      ['Roboto-Thin', '100'],
      ['Roboto-Hairline', '100'],
      ['Roboto-ExtraLight', '200'],
      ['Roboto-UltraLight', '200'],
      ['Roboto-Light', '300'],
      ['Roboto-Regular', '400'],
      ['Roboto-Medium', '500'],
      ['Roboto-SemiBold', '600'],
      ['Roboto-DemiBold', '600'],
      ['Roboto-Bold', '700'],
      ['Roboto-ExtraBold', '800'],
      ['Roboto-Black', '900'],
      ['Roboto-Heavy', '900'],
    ]
    for (const [name, weight] of expected) {
      expect(cssFontWeight(name), name).toBe(weight)
    }
  })
})

describe('normalised family lookup — A-16', () => {
  it('matches a spaced table key against an unspaced PostScript name', () => {
    // PostScript names carry no spaces. Exact equality therefore reported
    // 'PT Sans' and 'Exo 2' as unresolved although both are fetchable and both
    // are listed in GOOGLE_FONTS.
    expect(resolveFont('PTSans-Bold')).toBe('google-fonts')
    expect(resolveFont('Exo2-Bold')).toBe('google-fonts')
    expect(googleFontToken('PTSans-Bold')).toBe('PT+Sans')
    expect(googleFontToken('Exo2-Bold')).toBe('Exo+2')
  })

  it('does not start matching families that are genuinely absent', () => {
    // Normalising must widen the match, not weaken it: these are still losses
    // and must still be reported as losses.
    expect(googleFontToken('GimbalExtended-Regular')).toBeUndefined()
    expect(googleFontToken('BruceForever-Regular')).toBeUndefined()
    expect(resolveFont('')).toBe('unresolved')
  })
})
