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
 * Letters and digits only, lower-cased — `PT Sans`, `PTSans` and `pt-sans` all
 * reduce to `ptsans`. This is the key both the table and the lookup use.
 */
function normaliseFamily(family: string): string {
  return family.toLowerCase().replace(/[^a-z0-9]+/g, '')
}

/**
 * The lookup index, built once at module load rather than per call.
 *
 * The two lookups below each used to rebuild `Object.keys(GOOGLE_FONTS)` and
 * recompute `baseFamilyName` separately (`A-21`), which also let them disagree.
 */
const GOOGLE_BY_NORMALISED: ReadonlyMap<string, string> = new Map(
  Object.entries(GOOGLE_FONTS).map(([family, token]) => [normaliseFamily(family), token]),
)

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

// ⚠ **Longest alternative first, always.** A regex alternation is first-match,
// not longest-match, so listing `Bold` before `ExtraBold` would strip `Bold`
// and leave `Montserrat-Extra`. Every two-word weight name below was MISSING
// from the first version of this list, which is area-audit finding `A-4`:
// `Montserrat-ExtraBold` survived the strip, the camel-case splitter then made
// it `Montserrat-Extra Bold`, and the lookup reported one of the most common
// headline faces in ad PSDs as **unresolved** — a false loss claim in the exact
// field `P3-48` created to stop false success claims.
const STYLE_SUFFIX =
  /-(ExtraBoldItalic|UltraBoldItalic|SemiBoldItalic|DemiBoldItalic|ExtraLightItalic|UltraLightItalic|BoldItalic|BoldOblique|ItalicMT|ObliqueMT|BoldMT|ExtraBlack|ExtraBold|UltraBold|ExtraLight|UltraLight|SemiBold|Semibold|DemiBold|Demibold|Hairline|Regular|Medium|Black|Heavy|Light|Thin|Book|Demi|Bold|Italic|Condensed|Extended|Narrow|Oblique|Roman)$/i
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

/**
 * CSS weight from the PostScript suffix. Order matters: the compound names are
 * tested before the simple ones they contain, or `ExtraBold` matches `bold`.
 *
 * 🔴 **The values were wrong, and wrong in both directions** (`A-12`).
 * `ultrabold` sat in the `900` branch while `extrabold` sat in the `600` branch
 * beside `semibold` — so the same physical face rendered two steps too light
 * under one name and one step too heavy under the other, and a design using
 * both names for one face showed a visible weight difference. `extralight` was
 * bucketed with `thin` at `100`. The CSS scale is fixed and unambiguous:
 * 100 Thin · 200 ExtraLight · 300 Light · 400 Regular · 500 Medium ·
 * 600 SemiBold · 700 Bold · 800 ExtraBold · 900 Black.
 *
 * This value is what `psdToIr` writes into `FontRequirement.weights` and what
 * `googleFontsHrefs` then requests, so a wrong weight fetches the wrong face
 * as well as rendering it wrongly.
 */
export function cssFontWeight(postScriptName: string): string {
  if (!postScriptName) return '400'
  const n = postScriptName.toLowerCase()
  if (/black|heavy|extrablack|ultrablack/.test(n)) return '900'
  if (/extrabold|ultrabold|extra[-_]bold|ultra[-_]bold/.test(n)) return '800'
  if (/demibold|semibold|semi[-_]bold|demi[-_]bold/.test(n)) return '600'
  if (/bold/.test(n)) return '700'
  if (/extralight|ultralight|extra[-_]light|ultra[-_]light/.test(n)) return '200'
  if (/thin|hairline/.test(n)) return '100'
  if (/light/.test(n)) return '300'
  if (/medium/.test(n)) return '500'
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
  return googleFontToken(postScriptName) ? 'google-fonts' : 'unresolved'
}

/**
 * The Google Fonts URL token for a family, or `undefined` if it has none.
 *
 * 🔴 Lookup is on the **normalised** key, not the literal string. A PostScript
 * name carries no spaces, so `PTSans-Bold` reduces to `PTSans` while the table
 * key is `PT Sans` — and exact equality reported both `PT Sans` and `Exo 2` as
 * unresolved (`A-16`) although both are fetchable and both are in the table
 * two screens up. Normalising strips everything that is not a letter or digit,
 * which is precisely the difference PostScript drops.
 */
export function googleFontToken(postScriptName: string): string | undefined {
  return GOOGLE_BY_NORMALISED.get(normaliseFamily(baseFamilyName(postScriptName)))
}

/**
 * Build one Google Fonts stylesheet href **per family**.
 *
 * Returns an empty array when nothing is fetchable, so callers emit no `<link>`
 * rather than an empty one. Weights are sorted numerically because
 * `['900','700'].sort()` is lexicographic and yields `700;900` only by luck.
 *
 * 🔴 **Why one request per family and not one request for all of them**
 * (`A-17`). Google Fonts answers `css2` with HTTP 400 for the *whole* request
 * when any single `wght@` value does not exist for its family — and
 * `psdToIr` forces `'700'` for any faux-bolded layer regardless of what the
 * family actually ships. So a designer faux-bolding a Bebas Neue headline
 * (Bebas Neue ships 400 only) took down the stylesheet for **every** font in
 * the document, and `unresolvedFonts()` reported nothing, because all of them
 * had resolved. We cannot check the axis list offline; what we can do is stop
 * one unavailable weight from costing the other families. One bad request now
 * loses one typeface.
 */
export function googleFontsHrefs(
  families: ReadonlyArray<{ postScriptName: string; weights: string[] }>,
): string[] {
  const hrefs: string[] = []
  const seen = new Set<string>()
  for (const { postScriptName, weights } of families) {
    const token = googleFontToken(postScriptName)
    if (!token || seen.has(token)) continue
    seen.add(token)
    const sorted = [...new Set(weights)].sort((a, b) => Number(a) - Number(b)).join(';')
    hrefs.push(`https://fonts.googleapis.com/css2?family=${token}:wght@${sorted}&display=swap`)
  }
  return hrefs
}
