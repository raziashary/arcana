## Colours

**Primaries**
```
Milky White       #FFFEF7   page background
Sunny Herb        #CEF26D   primary CTA / success / valid
Ice Blue          #8DC2FF   info / selection / focus
Stellar White     #FFFFFF   panel surface
Nebula Purple     #3A2055
Orbital Blue      #2B4C75   secondary text
Deep Space Navy   #0E1131   primary text
```

**Secondary pastels**
```
Moonlit Mist      #DBDBFC   borders
Lunar Lavender    #BEBDF7
Comet Blue        #57A0F4   muted text
Aurora Rose       #C27BA7
Stellar Blue      #4C9FFF
Galaxy Veil       #EDECF9   surface-2
```

**Semantic** — CTA `#CEF26D`, danger/Nova Coral `#FF696B`, info `#8DC2FF`.

**Shipped tokens (D-019, 2026-11-13).** `src/styles/globals.css` `:root` is the one place colour is defined (`TG-P6-04`; `TG-P6-01` fails on an undefined
`var(--name)`). The values are the anchor's: `--bg-app #FFFEF7`, `--bg-surface #FFFFFF`, `--bg-code`/`--bg-2 #EDECF9`, `--border-primary #DBDBFC`,
`--border-secondary #BEBDF7`, `--text-primary #0E1131`, `--text-secondary #2B4C75`. Where the anchor's colour fails contrast as **text** on white, the text role
gets a darker variant of the same hue and the anchor colour stays the fill or tint:

| Role | Token | Value | Why it differs from the anchor |
|---|---|---|---|
| Accent, links, focus | `--accent` | `#2D6CDF` | Stellar Blue `#4C9FFF` is 2.8:1 on white; this is about 5:1 |
| Success, diff additions | `--success` | `#3C7A12` | Sunny Herb `#CEF26D` is unreadable as text; it is kept as `--cta` / `--success-fill` |
| Danger | `--danger` | `#D64550` | Nova Coral `#FF696B` is about 3:1 as text |
| Muted and dim text | `--text-muted`, `--text-dim` | `rgba(14,17,49,.62)`, `.42` | Comet Blue `#57A0F4` is 2.6:1; it is not used for text |
| Text on a solid fill | `--text-on-accent` | `#FFFFFF` | |

Literal colours exist only in `src/styles/palette.ts`, for the consumers that cannot read a CSS variable: the three.js city (a dark night scene with its own
palette), the graph's categorical community colours, and the Monaco theme (`arcana-light`). The Tailwind config's colours point at the variables.

Light mode only. **Not implemented in v1:** the pastel gradient page shell, glass surfaces, the 7px shape rule (the app uses 2px) and the motion table below. They
are anchor intent, not shipped behaviour; the shipped code is the anchor.

## Page shell

Pastel gradient — three radial washes over a diagonal base:

```css
background:
  radial-gradient(circle at 10% 8%,  rgba(87,160,244,0.22), transparent 34%),
  radial-gradient(circle at 88% 12%, rgba(190,189,247,0.28), transparent 36%),
  radial-gradient(circle at 74% 92%, rgba(194,123,167,0.12), transparent 32%),
  linear-gradient(135deg, #FFFEF7 0%, #EDECF9 42%, #DBDBFC 100%);
```

Rule attached to it: the gradient supplies atmosphere but must never cost readability — **all data-heavy areas sit on glass or milky-white**.

## Glass surfaces

| State | Background | Border | Extra |
|---|---|---|---|
| Default | `rgba(255,254,247,0.78)` | `rgba(141,194,255,0.28)` | blur 20px |
| Strong | `rgba(255,254,247,0.86)` | same | `.glass-strong` |
| Selected | `rgba(184,217,255,0.20)` | `rgba(141,194,255,0.62)` | glow `0 0 0 3px rgba(184,217,255,0.35)` |
| Error | `rgba(255,105,107,0.10)` | `rgba(255,105,107,0.55)` | |
| Info | `rgba(141,194,255,0.12)` | `rgba(141,194,255,0.42)` | |

Shadow is deliberately soft: `0 6px 22px rgba(46,76,117,0.07), 0 1px 2px rgba(46,76,117,0.05)`. The spec explicitly forbids heavy SaaS dashboard shadows.

## Typography

**Shipped (D-016, 2026-11-13): IBM Plex, bundled.** Two faces, SIL OFL, files in `src/assets/fonts/` (Latin subset, 400 and 500), declared with
`@font-face` in `src/styles/globals.css`; no font is fetched at runtime and both CSPs allow `font-src 'self'` only (`TG-P6-03`). The app's
`--font-sans` is IBM Plex Sans and `--font-mono` is IBM Plex Mono: Plex Sans for interface text, Plex Mono **only** for technical values
(JSON, IDs, timestamps, logs, code). The Urbanist/Manrope/Roboto Mono table this section used to hold was never implemented; the shipped code is the
anchor, so it was replaced. Rules that still hold: no thin weights in dense UI; never use website display sizes inside the tool.

## Shape — one locked rule

**Everything is 7px.** Panels, cards, inputs, dropdowns, buttons, badges, graph nodes.

> "All buttons should use 7px corner radius, **not fully rounded pill styling**."

Pills (`--radius-pill: 9999px`) are permitted for **status dots only**. `--radius-sm` is 2px.

## Motion

`--ease: cubic-bezier(0.22, 0.61, 0.36, 1)`

| Interaction | Timing |
|---|---|
| Hover / focus | 160–220ms (`--motion-hover: 180ms`) |
| Hover return | 260–320ms (`--motion-return: 290ms`) |
| Dropdown | 160–220ms |
| Sidebar expand | 220–300ms |
| Panel expand | 260–320ms (`--motion-panel: 290ms`) |
| Tab transition | 300–500ms (`--motion-tab: 360ms`) |
| Reduced-motion fallback | 200–300ms opacity fade |

Personality: subtle, professional, deliberate, calm, premium. **Explicitly banned:** bouncy spring, cursor tracking, large spins, auto-changing content, idle movement, heavy connector lines, shake-on-error.

## Class vocabulary

```
.airs-card  .airs-card-nested  .glass-strong  .airs-badge  .airs-mono
.btn  .btn-run-pipeline  .status-dot  .status-pill  .metric-chip  .filter-chip
.product-header  .product-logo  .product-title  .dev-tool-banner
.module-nav  .module-tab  .viz-sidebar  .viz-sidebar-item  .viz-main
.matrix-grid  .matrix-card  .stage-header  .stage-item-row
.inspector-tabs  .inspector-tab  .inspector-body  .detail-panel
.lineage-row  .lineage-stage  .lineage-rel  .lineage-db-tag
.audit-analysis  .code-preview  .cytoscape-container
.section-title  .section-sub  .sidebar-status
```

Status dots carry state via `status-dot complete | running | failed | waiting`; pills use `status-pill ok | fail | info`.

## Interaction posture

Dashboard, not website: compact overview first, click-to-inspect, sidebar navigation rather than a top anchor bar, no repeated "Expand" button on every card, graph sections stay full-width. There's also an accessibility block at the end of `styles.css` (V3 §19) covering focus rings and `prefers-reduced-motion`.

Sections beyond §3 spec the layout itself — §4 global frame, §5 header, §6 module nav, §7 sidebar, §9 run summary, §10 pipeline matrix, §11 inspector, §15 quality audit, §16 event log — if you need the per-component geometry rather than the tokens.