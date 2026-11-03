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

**App tokens:** `--app-background #FFFEF7`, `--app-surface #FFFFFF`, `--app-surface-2 #EDECF9`, `--app-border #DBDBFC`, text primary `#0E1131` / secondary `#2B4C75` / muted `#57A0F4`.

Light mode only — there is no dark palette.

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

Three faces, all SIL OFL, loaded from Google Fonts:

| Use | Font | Size | Weight |
|---|---|---|---|
| Product title | Urbanist | 18–20px | 600 |
| Section title | Urbanist | 20–24px | 600–700 |
| Card title | Urbanist | 15–17px | 600 |
| Body copy | Urbanist | 14–16px | 400–500 |
| Metadata / labels | Urbanist | 12–13px | 500 |
| **Buttons** | **Manrope** | 14–16px | 500–600 |
| **Code / IDs / logs** | **Roboto Mono** | 12–13px | 400–500 |

Rules: no thin weights in dense UI; Roboto Mono **only** for technical values (JSON, IDs, timestamps, logs); never use website display sizes inside the tool.

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