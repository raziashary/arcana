// PostScript font names → CSS, and an honest answer about what we can render.
//
// The mapping tables are ported from `apps/psdconversions/index.html`
// (`psNameToCssFontFamily` / `psNameToCssFontWeight`), which was tuned against
// real ad PSDs. They are reproduced rather than reinvented.
//
// ── What is NOT ported: the silence (P3-48) ──────────────────────────────────
//
// The original returns a CSS stack and nothing else, so a font it cannot supply
// degrades to `sans-serif` with no signal. The spike hit this immediately: a
// game ad asked for `Gimbal Extended` and `Bruce Forever`, neither is on Google
// Fonts, and the render came back looking finished with its display typography
// replaced by a generic sans. Nothing reported it.
//
// A conversion that quietly loses the design's typeface and still reports
// success is the same defect class as a gate that cannot fail. So resolution
// here returns *how* a font will be rendered, and callers must carry that to
// the user.

/** Families we can actually fetch, and their Google Fonts URL token. */
const GOOGLE_FONTS: Readonly<Record<string, string>> = {
  Montserrat: 'Montserrat',
  Roboto: 'Roboto',
  'Open Sans': 'Open+Sans',
  Lato: 'Lato',
  Oswald: 'Oswald',
  Raleway: 'Raleway',
  Poppins: 'Poppins',
  Nunito: 'Nunito',
  Inter: 'Inter',
  'Bebas Neue': 'Bebas+Neue',
  'Playfair Display': 'Playfair+Display',
  'Source Sans Pro': 'Source+Sans+Pro',
  'PT Sans': 'PT+Sans',
  Ubuntu: 'Ubuntu',
  Merriweather: 'Merriweather',
  'Noto Sans': 'Noto+Sans',
  Rubik: 'Rubik',
  'Work Sans': 'Work+Sans',
  Mulish: 'Mulish',
  'Josefin Sans': 'Josefin+Sans',
  Barlow: 'Barlow',
  'Barlow Condensed': 'Barlow+Condensed',
  'Exo 2': 'Exo+2',
  Exo: 'Exo',
  Rajdhani: 'Rajdhani',
  Orbitron: 'Orbitron',
  Teko: 'Teko',
}

/**
 * Families guaranteed by the operating system rather than fetched. Matching one
 * is a real resolution, not a fallback — `Arial` renders as `Arial`.
 */
const SYSTEM_STACKS: ReadonlyArray<readonly [RegExp, string]> = [
  [/^times|^timesnewroman/, "'Times New Roman',Times,serif"],
  [/^arial|^helveticaneue/, 'Arial,Helvetica,sans-serif'],
  [/^helvetica(?!neue)/, 'Helvetica,Arial,sans-serif'],
  [/^courier/, "'Courier New',Courier,monospace"],
  [/^georgia/, 'Georgia,serif'],
  [/^verdana/, 'Verdana,Geneva,sans-serif'],
  [/^tahoma/, 'Tahoma,Geneva,sans-serif'],
  [/^trebuchet/, "'Trebuchet MS',sans-serif"],
  [/^garamond/, 'Garamond,serif'],
  [/^palatino/, "'Palatino Linotype',Palatino,serif"],
  [/^futura/, "'Century Gothic',Futura,sans-serif"],
  [/^impact/, 'Impact,Charcoal,sans-serif'],
  [/^lucida/, "'Lucida Grande','Lucida Sans',sans-serif"],
  [/^myriad/, "'Myriad Pro',Myriad,sans-serif"],
  [/^minion/, "'Minion Pro',Minion,serif"],
  [/^gill/, "'Gill Sans',sans-serif"],
  [/^optima/, 'Optima,serif'],
  [/^baskerville/, 'Baskerville,serif'],
]

const STYLE_SUFFIX =
  /-(BoldItalic|BoldOblique|ItalicMT|ObliqueMT|BoldMT|Regular|Bold|Italic|Light|Medium|SemiBold|Semibold|Black|Condensed|Extended|Narrow|Oblique|Roman)$/i
const FOUNDRY_SUFFIX = /(PSMT|PST|PS|MT|Type1)$/i

/**
 * The family name with PostScript style and foundry suffixes stripped, spaced
 * at camel-case boundaries: `GimbalExtended-Regular` → `Gimbal Extended`.
 */
export function baseFamilyName(postScriptName: string): string {
  return postScriptName
    .replace(STYLE_SUFFIX, '')
    .replace(FOUNDRY_SUFFIX, '')
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    .trim()
}

/**
 * A CSS `font-family` stack.
 *
 * Single quotes, never double: this value is embedded inside a `style="…"`
 * attribute, and a double quote would terminate the attribute early and drop
 * every property after it. That is a real bug the source file documents.
 */
export function cssFontFamily(postScriptName: string): string {
  if (!postScriptName) return 'sans-serif'
  const lowered = postScriptName.toLowerCase()
  for (const [pattern, stack] of SYSTEM_STACKS) {
    if (pattern.test(lowered)) return stack
  }
  const base = baseFamilyName(postScriptName)
  const mono = /mono|code|courier|typewriter|console/i.test(base)
  const serif =
    /roman|garamond|palatino|georgia|times|bodoni|caslon|didot|baskerville|charter|century|minion|bookman/i.test(
      base,
    )
  return `'${base}',${mono ? 'monospace' : serif ? 'serif' : 'sans-serif'}`
}

/** CSS weight from the PostScript suffix. Order matters: `extrabold` before `bold`. */
export function cssFontWeight(postScriptName: string): string {
  if (!postScriptName) return '400'
  const n = postScriptName.toLowerCase()
  if (/black|heavy|ultrabold|extrablack/.test(n)) return '900'
  if (/extrabold|demibold|semibold|semi[-_]bold|demi[-_]bold/.test(n)) return '600'
  if (/bold/.test(n)) return '700'
  if (/thin|hairline|ultralight|extralight|extra[-_]light/.test(n)) return '100'
  if (/light/.test(n)) return '300'
  return '400'
}

export function isItalic(postScriptName: string): boolean {
  return /italic|oblique/i.test(postScriptName)
}

export type FontResolution = 'google-fonts' | 'system-stack' | 'unresolved'

/**
 * How this family will actually render — the answer `P3-48` requires.
 *
 * `unresolved` means we will draw the text in something that is **not** the
 * designer's typeface. It is not an error (the layout is still useful) but it
 * must never be presented as a faithful reproduction.
 */
export function resolveFont(postScriptName: string): FontResolution {
  const lowered = postScriptName.toLowerCase()
  if (SYSTEM_STACKS.some(([pattern]) => pattern.test(lowered))) return 'system-stack'
  const base = baseFamilyName(postScriptName)
  const inGoogle = Object.keys(GOOGLE_FONTS).some(
    (family) => family.toLowerCase() === base.toLowerCase(),
  )
  return inGoogle ? 'google-fonts' : 'unresolved'
}

/** The Google Fonts URL token for a family, or `undefined` if it has none. */
export function googleFontToken(postScriptName: string): string | undefined {
  const base = baseFamilyName(postScriptName).toLowerCase()
  const key = Object.keys(GOOGLE_FONTS).find((family) => family.toLowerCase() === base)
  return key ? GOOGLE_FONTS[key] : undefined
}

/**
 * Build the Google Fonts stylesheet href for the families that have one.
 *
 * Returns `undefined` when nothing is fetchable, so callers emit no `<link>`
 * rather than an empty one. Weights are sorted numerically because
 * `['900','700'].sort()` is lexicographic and yields `700;900` only by luck.
 */
export function googleFontsHref(
  families: ReadonlyArray<{ postScriptName: string; weights: string[] }>,
): string | undefined {
  const specs: string[] = []
  const seen = new Set<string>()
  for (const { postScriptName, weights } of families) {
    const token = googleFontToken(postScriptName)
    if (!token || seen.has(token)) continue
    seen.add(token)
    const sorted = [...new Set(weights)].sort((a, b) => Number(a) - Number(b)).join(';')
    specs.push(`${token}:wght@${sorted}`)
  }
  if (specs.length === 0) return undefined
  return `https://fonts.googleapis.com/css2?family=${specs.join('&family=')}&display=swap`
}
