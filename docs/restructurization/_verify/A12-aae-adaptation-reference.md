# AAE Reference Brief — what the Adobe MCP "Adaptive Adjustment Engine" already solved

**Source read:** `C:\AIRS Foundry\AIRS Microservices\mcp-server-adobe\` (TypeScript).
**Read date:** 2026-09-03. Files as of last modification 2026-06-01/02.
**Method:** code is authoritative; the three audit markdowns are treated as *claims* and checked against the code. Disagreements are flagged inline as **DOC != CODE**.

---

## 1. The adaptation model

### 1.1 One sentence

It is **not** a constraint solver. It is a **generate-and-test search over a small set of hand-authored layout hypotheses**, where each hypothesis compiles to a **plan of typed semantic transforms**, and the winner is chosen by **counting violations** (fewest violations, then fewest errors, then lowest seed index).

### 1.2 Input representation

`src/aae/types.ts:127` — the whole engine consumes a flat, absolute-pixel scene:

```ts
interface Scene { width: number; height: number; dpi?: number; nodes: Node[] }  // nodes is FLAT
interface Node {
  id: number; name: string; type: 'text'|'smartObject'|'shape'|'group';
  role?: Role; z: number; box: Box; parentId?: number;
  rotate?: number; content?: string; children?: Node[];
  style?: { fontSize?, leading?, tracking?, weight?, fontName?, color?, alignment?, orientation? };
}
interface Box { left: number; top: number; width: number; height: number }   // types.ts:15
```

Key properties of the IR:

- **Absolute px only.** There is no relative/constraint/percentage geometry *in the scene*. `types.ts:9` says AAE needs "strict, non-optional boxes for mathematical operations" — stricter than the Adobe API's partial `BoundsSchema`.
- **Hierarchy is advisory.** `nodes` is flat with `parentId`/`z`; `children` exists but the pipeline iterates `scene.nodes` flat everywhere (`v2-adapt.ts:298`, `transform-plan.ts:232`). Group semantics are re-derived by walking `parentId` (`transform-plan.ts:206` `isContainedByNamedGroup`).
- **No paths, masks, effects, fills, constraints, or text runs.** A text node carries one `content` string and one `style`. Rich text, stroke, shadow, clipping, blend mode are absent from the IR. This is the largest gap for a PSD/PDF-backed IR.
- **Rotation is carried but never used.** `rotate?: number` is commented "preserved during adaptation"; no transform function reads it. All geometry is axis-aligned AABB.
- **DPI is declared and never read** in `src/aae/` layout code.

### 1.3 Output representation

Two outputs, and the duplication matters:

**(a) `AdaptedLayout`** (`types.ts:198`) — the legacy shape actually consumed by the runner:
```ts
interface AdaptedLayout {
  boxes:  Record<number, Box>;                                   // nodeId -> adapted box
  styles: Record<number, { fontSize?; tracking?; leading? }>;
  report: { nodes[]; cost?; iterations?; convergence?; warnings?: string[];
            elementsMoved?; textReflowed?: boolean; scaled?: boolean; croppedPercent? };
}
```

**(b) `TransformPlan`** (`transform-plan.ts:111`) — the declared canonical, format-agnostic IR:
```ts
interface TransformPlan {
  runId: string;
  source: CanvasSpec; output: CanvasSpec;        // CanvasSpec = { width, height }
  policyName: string;                            // winning SEED name
  guideId?: string;                              // brand policy id
  elements: TransformElement[];
  refinementIterations: number;
  violations: Array<{ type; severity; message }>;
}
interface TransformElement {
  elementId: string;              // `${role}-${nodeId}`
  sourceElementId: number;        // PSD layer id
  role: Role;
  action: 'transform' | 'hide' | 'skip';
  sourceBox: Box; targetBox: Box;               // <- the definitive geometry
  scale?: number; translate?: Vec2; cropRatio?: number;   // derived summaries only
  zOrderChange?: number; visible?: boolean;
  nodeType: 'group' | 'leaf';
  containedByGroup: boolean;
  decisionOrigin: string[];       // ["solver:<policy>","transform:<kind>","guide:<id>","refinement:iter_N"]
}
```

The derived fields are computed once in `buildTransformPlan` (`transform-plan.ts:242-268`) and *omitted when negligible*: `scale` only if `|s-1| > 0.001`, `translate` only if centre moves > 0.5px, `cropRatio` only if > 0.001. `cropRatio` is an **axis-clipped area fraction** against the target canvas, not a true intersection with a clip path.

> **DOC != CODE.** `AAE_PIPELINE_AUDIT.md:194-227` documents `TransformPlan` as `{ canvas, elements, metadata:{sourceWidth,...,adaptMetrics} }` with `TransformElement.transform:{kind}` and `CanvasSpec.{anchorX,anchorY}`. **None of that exists.** The real plan is box-to-box; the transform *kind* survives only as a provenance string in `decisionOrigin`. The same doc at `:241-247` puts `elementsMoved/scaled/textReflowed/croppedPercent` at the top level of `AdaptedLayout`; in code they live under `report`, and `scaled`/`textReflowed` are **booleans**, not counts (`types.ts:217-219`). Trust the code.

### 1.4 The unit of change

Two units, at two levels — this is the engine's central and best idea:

- **Stage 2 (reasoning) operates on `Component`s**, not nodes. A component is a cluster of nodes carrying one role.
- **Stage 2 emits `SemanticTransform` — intent, no pixels** (`plan.ts:49`):
```ts
type SemanticTransform =
  | { kind:'preserve'; noScale?; heroScale?; offset? }
  | { kind:'anchor'; anchor: Anchor; noScale?; heroScale?; offset? }
  | { kind:'relative'; toRole: Role; where:'above'|'below'|'left'|'right'; gapToken:'tight'|'normal'|'loose'; offset? }
  | { kind:'stack'; direction:'vertical'|'horizontal'; gapToken; offset? }
  | { kind:'reflow'; strategy:'rows'|'columns'; offset? }
  | { kind:'fit'; mode:'contain'|'cover'; offset? }
  | { kind:'cover'; offset? }
  | { kind:'fill-region'; widthFraction; heightFraction; anchor; fit:'contain'|'cover'; offset? }
  | { kind:'static'; offset? };
```
with the stated rule at `plan.ts:44`: *"No pixel values allowed in transforms (no offset px, no scale factors)."* — **except** the `offset?: Vec2` escape hatch that overlap resolution uses, which the same comment admits. That exception is the first crack in the abstraction; see §4.

- **Stage 3 (`compile.ts`) is declared taste-free**: `compile.ts:6` "Stage 3 has NO TASTE - just deterministic execution. All subjectivity lives in Stage 2's policy weights."

So: **the unit of change is a component-level typed transform, compiled to a per-node target box.** The artefact handed downstream is a box pair, not a transform.

### 1.5 The actual pipeline (`v2-adapt.ts:254 adaptLayoutV2`)

```
guard degenerate canvas (v2-adapt.ts:264)     throws on non-finite / <=0 dims
Phase 1  analyzeBreakpoint()        breakpoints.ts   -> AR buckets + layout mode + BucketPolicy
Phase 2  extractComponents()        components.ts    -> Component[]
Phase 3  inferRelationships()       components.ts    -> relationships between components
Phase 4  buildLayoutGraph()         components.ts    -> LayoutGraph (+ StructuralUnits, normalized geometry)
Phase 5  generateCandidateSeeds()   candidates.ts    -> CandidateSeed[]  (policy seed unshifted at :393)
Phase 5b generateLayoutPlans()      plan-generator   -> LayoutPlan[]  (one per seed)
Phase 5c resolveOverlapsInPlan()    overlap-resolver -> LayoutPlan[] with offsets baked in
Phase 6  compileLayoutPlan() x N    compile.ts       -> CompilationResult[]
         selectBestCompilation()    v2-adapt.ts:204  -> argmin(violations, errors, index)
         refinement while-loop, max 3 iterations     refinement.ts
Phase 7  styles taken straight from Stage 3 (v2-adapt.ts:574 - no second typography pass)
         enforcePolicySafeZoneWithReport()           policy-resolver -> post-hoc box clamping
         buildTransformPlan()                        transform-plan.ts
```

Selection is **lexicographic violation counting**, not scoring (`v2-adapt.ts:216`):
```ts
scored.sort((a,b) => a.vCount - b.vCount || a.eCount - b.eCount || a.idx - b.idx);
```
There is no cost function, no per-type weighting, and no aesthetic term in selection. `report.cost` is hardcoded `0` (`v2-adapt.ts:654`). `CostWeights` (`types.ts:181`, with `thirdsAttract`, `goldenSplitAlign`, `hierarchyPreserve`, `spacingConsistency`) is a **fully dead type** — nothing constructs or consumes it.

Two late-added correctness guards are worth stealing verbatim:
- **`isBetterCompilation` / best-so-far** (`v2-adapt.ts:235`, commented "Razi 8.3"): refinement previously returned the *last* iteration's result, which could be worse than an earlier one. Now the loop keeps the best across all iterations and explicitly reverts (`:561-565`).
- **Degenerate-canvas guard** (`v2-adapt.ts:264`, "Razi 10.1"): rejects non-finite/zero dims up front to stop `Infinity`/`NaN` propagating through the many `w/h` divisions.

### 1.6 `adapt.ts` vs `v2-adapt.ts` — what V2 changed, and why

**As shipped, `adapt.ts` is a 71-line pass-through with no V1 code at all:**
```ts
export function adaptLayout(...): AdaptedLayout {
  logger.info('aae-adapt', 'Using Layout Engine V2 (Holy Grail)');
  return adaptLayoutFull(...).adaptedLayout;                    // adapt.ts:42-52
}
```
`adaptLayoutFull()` exists purely so callers can reach the `TransformPlan` and `effectiveRoles`, not only `boxes`/`styles`.

> **DOC != CODE.** `AAE_AUDIT_REPORT.md:432/477` and `AAE_PIPELINE_AUDIT.md:68-77` both describe `adapt.ts` as routing on `process.env.AAE_LAYOUT_ENGINE !== 'v1'`, and both document a live "V1 Layout Engine" with a **gradient-descent soft pass**. **That env check does not exist, and there is no gradient descent anywhere in `src/aae/`.** V1 was deleted, not deprecated. `adapt.ts` still carries `_enableSoftPass` and `_roleWeightsOverride` parameters that are underscore-prefixed and **ignored** — dead API surface kept for call-site compatibility. `AAE_ERRORS_AND_FAULTS.md:103-111` cites `adapt.ts:128` and `adapt.ts:228`; the file is 71 lines long. The audits are stale by roughly one refactor.

**What survived of V1: `solver.ts` only.** Its header now reads *"V1-specific optimization functions have been removed"* (`solver.ts:5`). Three pure functions remain and are reused by V2 candidate generation:

- `initialScale()` (`solver.ts:25`) — dispatch bg -> cover, else responsive. Note the smell: backgrounds are detected **by layer-name substring** (`name.includes('background')`, `'backdrop'`, `'hero-bg'`) as well as by role (`solver.ts:32-36`), and it `console.log`s on every hit.
- `initialScaleCover()` (`solver.ts:50`) — `max(sx,sy)`, centred, with cover-offset correction so the source centre maps correctly under overflow.
- `initialScaleResponsive()` (`solver.ts:92`) — **the load-bearing idea**: *centre-fraction preservation with uniform `min(sx,sy)` scale.* An element whose centre sits at (50%,50%) of source lands at (50%,50%) of target; size scales by the limiting axis. Everything else in the engine is a correction on top of this baseline.
- `clampToSafe()` (`solver.ts:130`) — shrink-to-fit-safe-rect (uniform), then clamp position. `safePct` bounded to `[0, 0.49]` to stop an inverted safe rect.

**The V2 thesis:** replace "scale everything proportionally, then optimise a cost function" with "**pick a composition strategy, express it as typed intent per semantic component, compile deterministically, count what broke, retry with adjusted constraints.**" The stated motivation is separability (`plan.ts:9`, "NO TASTE in Stage 3") so subjectivity is auditable and located in exactly one place. That motivation is sound and is the main idea worth carrying forward. The execution is where it went wrong (§4).

---

## 2. How layout intent is captured

### 2.1 The short answer: role first, geometry second

Intent is **not** authored per-element by a designer and **not** inferred from constraints in the source file. It is derived almost entirely from **a semantic role string** on each node, plus that node's **normalised position in the source canvas**.

`Role` (`types.ts:27`) is a closed 22-value enum organised into behaviour families, and the comment block *is* the intent model:

```
bg | overlay | pattern                          -> "Cover elements (scale to fill canvas)"
heroImage | productImage | supportingImage      -> "Smart-place (scale + reposition per composition)"
logo | cta | badge | icon | sticker             -> "Preserve elements (maintain size, smart reposition)"
title | subtitle | bodyText | price | promoText -> "Text elements (text-specific adaptation)"
disclaimer | contactUrl                         -> "Anchor elements (pin to edge, shrink-only)"
decoration                                      -> "Static (never scale, only reposition)"
+ 7 legacy aliases (headline, subhead, body, decor, tileTitle, tileNewPrice, tileOldPrice)
```

`canonicalRole()` (`types.ts:67`) normalises the aliases at input boundaries; the comment "avoid branching on them in layout logic" records a real bug class — code that branched on `'decor'` never matched real `'decoration'` nodes (fixed at `checker.ts:219`, "Razi 7.3").

### 2.2 `plan-generator.ts::computeSemanticTransform` is the intent engine

Despite the "candidate search" framing, **the transform decision is a hard-coded role cascade** (`plan-generator.ts:400-617`), evaluated in this order:

1. explicit policy `ElementRule.transform` (`stack`|`relative`|`contain`|`cover`|`anchor`)
2. **bg** — by role *or by layer-name substring* (`background`, `bg`, `backdrop`, `hero-bg`) -> `cover`
3. **logo** -> `{ kind:'anchor', anchor: detectNearestCorner(...), noScale:true }`
4. **cta** -> seed-dependent: `under-hero` -> `relative below heroImage`; `side` -> `anchor right-center`; `bottom` -> `anchor bottom-center`; else nearest source corner. All `noScale:true`.
5. **overlay / pattern** -> `cover`, always
6. **heroImage / productImage / supportingImage** -> `fill-region` for `fill-*` seeds; else **target-bucket-aware** anchor with `heroScale` (default **0.67**). Non-hero image roles ignore the seed and always `preserve`.
7. **title/subtitle/bodyText/price/promoText** -> `anchor` at each element's *own* nearest source corner
8. **disclaimer / contactUrl / decoration** -> `preserve`
9. **badge** -> nearest corner, `noScale:true`
10. default -> `preserve`

The `CandidateSeed` influences only three things: `heroPlacement`, `ctaPlacement`, `textStrategy`. `anchorStrategy` and `stackingOrder` are carried in the type and **never read** by `computeSemanticTransform`. For most scenes the "candidates" are near-duplicates.

**`detectNearestCorner`** applied to the *source* bbox is the single most transferable idea in this file: "an element that lived in the top-right stays in the top-right". It reconstructs a corner constraint the source format never stored.

Two comments record hard-won lessons worth keeping:

- `plan-generator.ts:498-503` — overlay/pattern must *always* `cover`. An area-based heuristic (">50% of source area -> cover") let a gradient overlay fall through to `preserve`, producing a visible "darker shade line" across five named test assets (EPL I-02/I-09, BGMI B-01, Sega S-01). **Coupled elements must inherit their parent's transform, not be re-decided independently.**
- `plan-generator.ts:602-605` — `decoration` deliberately uses `preserve`, not `static`, because `static` copies source pixel coordinates unchanged and puts decorations off-canvas on any resize. **A "don't touch" transform is wrong when the canvas itself moved.**

### 2.3 `components.ts` — the grouping layer

`extractComponents` (`components.ts:140`) lifts flat layers into `Component`s in four steps: (1) group-based components from PSD folders, (2) proximity clustering of the remainder (`PROXIMITY_THRESHOLD = 50px`, `MAX_CLUSTER_DIAMETER = 500px` — **both absolute px, not canvas-relative**), (3) grid detection, (4) stack detection.

The most valuable single line is the isolation set (`components.ts:162`):
```ts
const ISOLATE_ROLES = new Set<Role>(['bg', 'overlay', 'pattern', 'logo']);
```
with the note ("Razi NF-R4") that a full-canvas background overlaps everything, so edge-distance clustering absorbed logos and overlays into one giant bg cluster and destroyed their role transforms — the observed *"logo missing / overlay shade line"* failures. **Full-bleed elements must be excluded from proximity clustering before it runs.**

`ComponentConstraints` (`components.ts:82`) is the declared permission model — what may crop, wrap, reorder, stretch:
```ts
interface ComponentConstraints {
  minSize?: number; maxCrop?: number;
  cropAllowed: boolean; readabilityProtected: boolean; reorderAllowed: boolean;
  wrapAllowed?: boolean; aspectRatioLocked?: boolean;
  anchorPreference?: 'center'|'top-left'|'bottom-right';
}
```
`inferComponentConstraints` (`components.ts:629`) fills it per role — logo `{minSize:96, maxCrop:0, cropAllowed:false, aspectRatioLocked:true}`, text `{minSize:18, readabilityProtected:true}`, cta `{minSize:120}`, decoration `{cropAllowed:true, reorderAllowed:true}`, bg `{cropAllowed:true, maxCrop:50}`.

> **This is the biggest single waste in the codebase.** A grep across `src/` finds **exactly one** read of any of these eight fields: `overlap-resolver.ts:75` reads `constraints.reorderAllowed`. `cropAllowed`, `readabilityProtected`, `aspectRatioLocked`, `maxCrop`, `minSize`, `wrapAllowed` and `anchorPreference` are computed for every component on every run and **never consulted by any transform, check, or score.** The permission model was designed and then bypassed by the role cascade in §2.2, which re-derives the same decisions ad hoc from the role string.

`LayoutGraph` (`components.ts:104`) likewise declares `structuralUnits`, `normalizedGeometry`, `compositionType`, `hierarchy`, `zones`. In the live pipeline:
- `structuralUnits` — **used, and important**: `plan-generator.ts:88-103` transforms *units first, then expands to member components*. This is what stops a badge/CTA pair drifting apart under an anchor change.
- `relationships` — used for exactly one decision: `plan-generator.ts:134`, `unit.relation === 'text_stack'`.
- `normalizedGeometry`, `compositionType`, `hierarchy`, `zones` — computed and **only ever printed to a log line** (`v2-adapt.ts:343-344`, `:808-810`). Dead analysis.

### 2.4 `breakpoints.ts` — the coarse regime switch

Two orthogonal quantisations of the resize (`breakpoints.ts:62-94`):
```
detectARBucket(ar):  <0.75 tall_portrait | <0.95 portrait | <1.15 square
                     | <1.6 landscape | <2.0 wide | else ultra_wide
selectLayoutMode(srcAR,tgtAR): |dAR|/srcAR  <0.2 preserve | <0.8 reflow | else recompose
```
`BucketPolicy` then declares per-bucket permissions: `gridCols`, `stackingAllowed`, `heroScaling`, `ctaPlacement`, `textWrap`, `decorCrop`, `reorderElements`, with per-`DesignType` overrides (`banner|poster|story|social_post|kv|free_design`).

> **Caveat from the code:** `v2-adapt.ts:313` calls `analyzeBreakpoint(..., 'free_design')` with the design type **hardcoded**, and `DESIGN_POLICY_OVERRIDES.free_design = {}` (`breakpoints.ts:264`). **Every design-type override in this file is unreachable in the live pipeline.** Of the seven `BucketPolicy` fields only `heroScaling`/`stackingAllowed`/`gridCols` are ever logged; `decorCrop`, `textWrap`, `reorderElements` are never consulted by the compiler; `getGridConfig()` is never called. What *does* matter is the bucket itself (hero placement routes on `breakpoint.targetBucket`, `plan-generator.ts:555`) and the mode (selects the seed set, `candidates.ts:194`). The policy table is largely inert.

### 2.5 `candidates.ts` — seeds, not candidates

Only three exports are live: `HardConstraints`, `DEFAULT_HARD_CONSTRAINTS`, `generateCandidateSeeds`.

```ts
interface HardConstraints {
  minFontSize: number;         // default 18
  minCtaSize: number;          // default 120  (min dimension of the CTA box, px)
  maxLogoCrop: number;         // default 0    (LINEAR % allowance, not area)
  safeMargins: number;         // default 0.04
  maxPrimaryOverlap: number;   // default 10   <- DECLARED, NEVER CHECKED ANYWHERE
  maxLogoScaleVector?: number; // default 1.5
  maxLogoScaleRaster?: number; // default 1.0  (rasters never upscale)
}
```

`generateCandidateSeeds(bucket, mode, graph)` (`candidates.ts:187`) returns 1-3 hand-written seeds per (mode,bucket) pair — `preserve` -> `center_preserve`, `golden_preserve`; `reflow`+portrait -> `stack_vertical_center`, `stack_vertical_left`, `hero_dominant`; `recompose`+`ultra_wide` -> `three_column`, `hero_center_sides`. The `graph` argument is accepted and ignored. **The search space is a fixed lookup table of at most three hypotheses.**

> Roughly 500 of `candidates.ts`'s 737 lines are a **complete, dead, V1 weighted scoring engine**: `generateCandidates`, `scoreCandidates`, `selectBest`, `scoreHierarchy`, `scoreAlignment`, `scoreWhitespace`, `scoreReadability`, `scoreComposition`, plus the `Candidate`, `BrandPolicy`, `DEFAULT_BRAND_POLICY` types. All are non-exported or unreferenced outside the file and marked `DEPRECATED`. **Nothing in the shipped pipeline computes a weighted layout score.** The engine deleted its own scoring function and replaced it with violation counting.

### 2.6 Relative vs absolute units

`units.ts` is small, clean, complete — the best-factored file in the engine. It resolves three unit systems against a reference dimension:
```ts
type UnitValue = number | string | { value: number; unit: 'px'|'%'|'fr' };  // "16px" | "5%" | "0.3fr"
resolveUnit(v, referencePx)            // px -> round(v); % -> v/100*ref; fr -> v*ref
resolveSpacing(spec, canvasW, canvasH) // left/right resolve vs W; top/bottom vs H
resolveAsFraction(v, referencePx)      // -> 0..1; bare number <=1 treated as already a fraction
```
Architecture note at `units.ts:16-19`: *"Policy JSON (authored) -> UnitValue; policy-resolver -> resolveUnit() -> number (px); compile.ts receives only px numbers."*

**The crucial design decision: relative units exist only in the authored policy layer — never in the scene, never in the engine.** `Scene`, `Box` and `TransformPlan` are pure absolute pixels; everything relative is resolved at the boundary. The split is clean and it worked. Its consequences, visible in the code:

- Because the *scene* has no relative geometry, all responsiveness must be re-derived per run from centre fractions (`solver.ts:104`), nearest-corner detection, and role tables. **Nothing about the designer's actual intent is ever stored** — it is guessed, discarded, and guessed again on the next run.
- Axis binding is hardcoded, not declared: vertical gaps resolve against `canvasH`, horizontal against `canvasW` (`units.ts:180-184`, `compile.ts:159-170`). Gap tokens are canvas-relative: `tight = 2%`, `normal = 4%`, `loose = 8%` of the axis reference. That scale is worth stealing.
- Several magic numbers escaped the unit system and are **absolute px**: `PROXIMITY_THRESHOLD=50` and `MAX_CLUSTER_DIAMETER=500` (`components.ts:18,24`), the overlap nudge cap of `50px` (`overlap-resolver.ts:335`), `minCtaSize=120`, `minFontSize=18`, logo `minSize=96`. None scale with canvas; all will misbehave across a 4x size range (a 50px proximity threshold means something completely different at 640x480 and at 4096x4096).

---

## 3. How correctness is checked

### 3.1 Where checking actually happens (and where it does not)

There are **four** places that produce violations in the live pipeline, and `checker.ts` is not one of them.

| Producer | Emits | Severity |
|---|---|---|
| `compile.ts::verifyHardConstraints` (`:935`) | `font_too_small`, `cta_too_small`, `logo_scaled_down`, `fully_off_canvas` | error |
| same | `partially_off_canvas` | warning |
| `compile.ts::clampComponentBoxesToCanvas` (`:784`) | `clamped_to_canvas` | warning |
| `compile.ts::applyStackTransform` (`:209/:224`) | `stack_overflow` | error |
| `overlap-resolver.ts:123` | `unresolvable_overlap` string pushed to `plan.report.hardConstraintViolations` | **never read** |
| `policy-resolver::enforcePolicySafeZoneWithReport` | `residualViolations` -> appended to `report.warnings` only | post-hoc |

> **`checker.ts` is dead code.** It exists, it is complete, and **nothing imports it.** A grep for `runChecks`, `resolveMaxCrop`, `detectSafeZoneViolations`, `DEFAULT_PENALTIES` finds hits only inside `checker.ts` itself. `overlap-resolver.ts` has its own private `detectOverlaps` (`:231`) that is *not* the checker's. This directly contradicts `AAE_ERRORS_AND_FAULTS.md:11-36`, which marks the missing-checker issue "RESOLVED — Module now exists with all documented functions". It exists; it is not wired. **The audit checked for existence, not for reachability.** That distinction is the single most useful lesson in this whole repo.

### 3.2 Invariants actually enforced

Enforced (checked, and a violation is counted toward plan selection):
- **Minimum font size** — `styles[node].fontSize >= minFontSize` (default 18px). Error.
- **Minimum CTA size** — `min(box.width, box.height) >= minCtaSize` (default 120px). Error.
- **Logo shrink limit** — `min(scaleW, scaleH)*100 >= 100 - maxLogoCrop`. Error. Note the "Razi NF-C2" comment (`compile.ts:983`): this must be compared as a **linear** ratio, not an area ratio — "a logo at 0.9 linear is a 10% crop, not 19% (= 1 - 0.9^2)". The identical bug was independently fixed in the dead `checker.ts:414` ("Razi NF-E1"). Two copies, two fixes.
- **Canvas containment** — `fully_off_canvas` (error) / `partially_off_canvas` (warning), with `bg`, `overlay`, `decoration`, `pattern` exempt because they bleed by design.
- **Stack overflow** — a stacked component past `targetH - safeMargin`. Error.
- **Logo upscale caps** — vector logos <=1.5x, raster logos <=1.0x (never upscaled). Enforced at `compile.ts:146-149` ("Razi NF-C0", previously computed and ignored).

**Not enforced, despite being declared:**
- **Overlap.** `HardConstraints.maxPrimaryOverlap` (default 10%) is declared in `candidates.ts:24` and **checked nowhere.** `compile.ts::verifyHardConstraints` has no overlap test. The only overlap machinery is the Stage-2 preview nudger (§3.3), whose findings never become violations.
- **Safe area** as an engine invariant. `safeMargins` is used by `applyStackTransform`'s overflow test and by `clampToSafe` in the dead V1 helper, but there is no general safe-zone check in the compile path. Safe zones exist only on the **policy** path and only as post-hoc, shift-only clamping.
- **Aspect-ratio preservation.** `aspectRatioLocked` is never read. Worse, `mapComponentBoxesToNodes` (`compile.ts:834-847`) applies **independent `scaleX`/`scaleY`** from the component box back to member nodes, so any non-uniform component box (a `cover` bg forced to `0,0,W,H`, a `fill-region`) **stretches its children non-uniformly** with nothing to catch it.
- **Legibility** beyond a raw px floor. No contrast check, no measured text fit, no line count.
- **Parent/child containment.** A child may leave its group's bbox with no complaint.

### 3.3 `overlap-resolver.ts` — priority-guided nudging, and why it barely works

The design intent is good and the header states the history plainly (`overlap-resolver.ts:4-8`):
> *"V1 disabled overlap resolution because it 'destroyed compositions.' V2 enables it safely by respecting component priorities. Key principle: Only move lower-priority components."*

Priorities are role-derived (`plan-generator.ts:650`): `logo 100 > heroImage 90 > productImage 88 > title 80 > cta 75 > subtitle 65 > bodyText/price 55 > promoText 50 > badge 40 > icon/sticker 35 > disclaimer/contactUrl 30 > decoration 20 > pattern 12 > overlay 11 > bg 10`, multiplied by an optional policy `rule.priority`. **This ordering is genuinely reusable** — it is a compact statement of "what a brand cares about most".

Everything around it is weak:

1. **`previewBoxes` is a second, approximate implementation of Stage 3** (`overlap-resolver.ts:141-225`). It handles only `preserve`, `anchor` and `fit`. Every other kind — `stack`, `reflow`, `cover`, `fill-region`, `relative`, `static` — falls to `box = comp.bbox`, i.e. **the source box in source-canvas coordinates**. Overlap is therefore detected in a mixed coordinate space whenever any component uses one of those kinds. Two implementations of the same geometry, one of them wrong, is the classic failure this architecture invites.
2. **Overlap detection here is role-blind.** `detectOverlaps` (`:231`) compares every pair with no notion of intentional overlap. Since `bg`/`overlay` get `fit`/`cover` boxes spanning the whole canvas, **every other component overlaps them by 100%**, every run. The role-aware `isOverlapAllowed` (text-over-image is fine) exists only in the **dead** `checker.ts:157`.
3. **The escape valve fails closed.** Only components with `constraints.reorderAllowed === true` may move — which, per `inferComponentConstraints`, is only `decoration`. Everything else hits `continue`, the loop reports `unresolvable_overlap` into a field nobody reads, and stops.
4. **The nudge is arbitrary and not canvas-relative**: `nudgeDistance = min(50, sqrt(overlapArea))` px (`:335`), direction = normalised centre-to-centre. `sqrt(area)` is dimensionally a length but has no relation to how much separation is needed, and the 50px cap means nothing at 4K.
5. **At most 10 nudges per plan** — the loop `break`s after the first successful move each iteration, and `maxIterations = 10`.
6. **It mutates the plan in place.** `compToMove.transform.offset = ...` and `plan.overlapResolutions = resolutions` (`:92, :131`); `plans.map(p => resolveOverlapsInPlan(p, graph))` returns the same objects it was given. Any caller holding the pre-resolution plan sees it change underneath.
7. `offset` on a transform is the **explicit violation of the "no pixels in Stage 2" rule** the type comment admits at `plan.ts:47`.

### 3.4 `refinement.ts` — a feedback loop whose wires are mostly cut

The loop (`v2-adapt.ts:464-554`): analyse violations -> build adjustments -> re-compile (or, from iteration 2, regenerate alternative seeds too) -> reselect. Max 3 iterations.

`analyzeViolations` (`refinement.ts:85`) has six branches, keyed on violation **type strings**:

| branch key | emitted by compile.ts? | effect |
|---|---|---|
| `overlap_detected` | **no** | `increase_gap` (high) |
| `font_too_small` | yes | `decrease_scale` (high) |
| `cta_too_small` | yes | `increase_scale` (high) |
| `logo_scaled_down` | yes | `adjust_constraints` (medium) |
| `constraint_violated` | **no** | `adjust_constraints` (high) |
| `transform_unimplemented` | **no** (removed when stack/reflow were implemented) | `change_layout_mode` (medium) |

**Three of six branches can never fire.** The consequence that matters: `increase_gap` is the *only* thing that raises `gapMultiplier`, and it requires `overlap_detected`, which nothing emits. **`gapMultiplier` is therefore permanently 1.0 in the shipped pipeline** — the entire gap-widening half of the refinement loop is inert. Meanwhile `stack_overflow`, `clamped_to_canvas`, `fully_off_canvas` and `partially_off_canvas` *are* emitted, count toward selection, and have **no branch at all**, so they can never be refined away.

Note also that `font_too_small` -> `decrease_scale` and `cta_too_small` -> `increase_scale` are directly opposed, applied to the same global `scaleMultiplier`; a scene with both violations oscillates at +/-0.15 per iteration.

`buildRefinementAdjustments` was later bounded ("Razi 8.1 / NF-F1", `refinement.ts:235-248`) with a good comment: unbounded repeated refinement "drove the gap to ~2.35 and could push minFontSize negative / minCtaSize below touch-target / maxLogoCrop past 100 — i.e. vacuous constraints." Bounds now: `gapMultiplier` in `[0.5,2.0]`, `scaleMultiplier` in `[0.5,1.5]`, `minFontSize >= 12`, `minCtaSize >= 44`, `maxLogoCrop <= 50`.

> **The deeper problem is the direction of the feedback.** `adjust_constraints` **relaxes the checker** — lowering `minFontSize`, lowering `minCtaSize`, raising `maxLogoCrop` — so that the same layout produces fewer violations next iteration. The loop's declared success condition (`violations === 0`) can be reached by weakening the test rather than improving the layout. Bounding the relaxation limits the damage but does not change the shape of the incentive.

### 3.5 Is there a measured fidelity score? No.

This section matters most for AutoDev's "fidelity measured, not asserted" phase, so state it precisely.

**There is no rendering, no rasterisation, and no comparison of source appearance to output appearance anywhere in this repo.** `AAE_PIPELINE_AUDIT.md:539-542` lists this as an open gap in its own words: *"Missing PNG Generation — No PNG output capability... would enable preview without Photoshop execution."* Every quality signal is computed from **boxes and numbers**, never from pixels.

What exists:

1. **`checker.ts::computeScore` (`:443`) — a score, but dead and derived, not measured.**
```ts
score = clamp01( 1 - sum over violations of ( basePenalty(severity) * typeWeight(type) ) )
DEFAULT_PENALTIES = { errorPenalty: 0.15, warningPenalty: 0.05,
  typeWeights: { font_too_small:1.0, cta_too_small:1.2, logo_scaled_down:0.8,
                 overlap_detected:0.5, outside_safe_zone:0.7, constraint_violated:1.0,
                 transform_unimplemented:0.3 } }
```
This is a *penalty tally re-expressed as a 0-1 number* — a restatement of the violation list, not an independent measurement of how much the design changed. And it never runs: nothing calls `runChecks`.

2. **`LayoutPlan.report.score` / `softScores`** (`plan.ts:118-122`) — initialised to `{ score: ?, hardConstraintViolations: [], softScores: {} }` at `plan-generator.ts:174-178` and **never written or read again**.

3. **`AdaptedLayout.report.cost`** — hardcoded `0` (`v2-adapt.ts:654`).

4. **The only genuinely measured quantities are crop and displacement**, and they are decent:
   - `croppedPercent` (`v2-adapt.ts:621-639`) — for the hero node only: `round((1 - visibleArea/totalArea) * 100)` against the target canvas rect.
   - `TransformElement.cropRatio` (`transform-plan.ts:258-268`) — the same clipped-area fraction, per element, omitted below 0.001.
   - `elementsMoved` (`v2-adapt.ts:580-610`) — and this carries a genuinely good lesson, commented "Razi 8.F2": movement is compared in **normalised** coordinates (fraction of each canvas), with `REL_EPS = 0.01`, because *"for a resizing engine, almost everything reads as 'moved/scaled' purely from the canvas change, making the metric low-signal."* **Any AutoDev fidelity metric must normalise for the intended resize before it can say anything.**
   - `scaled` / `textReflowed` are booleans over the whole scene — near-useless as reported.

5. **Text fit is never verified.** `applyTextOperations` (`compile.ts:860`) performs **no text measurement of any kind**: `scale`/`scale-wrap` multiply `fontSize` by `min(boxW/srcW, boxH/srcH)` and floor it at `minFontSize` (correctly preserving the leading/fontSize ratio, `:895-902`); `wrap` and `preserve` are identical no-ops that return the original size. There is no glyph metric, no line-breaking, no overflow test, no truncation. `TextFitResult { overflow, truncated, needsBreak }` (`types.ts:226`) is a **dead type**. So the `wrap` and `scale-wrap` directives — and the whole `textWrap` bucket policy — are unimplemented in substance. "Text fits" is asserted by a font-size floor and nothing else.

**Conclusion for §3:** the engine's notion of correctness is *"the compiler did not flag anything"*, where the compiler checks six numeric predicates on axis-aligned boxes, does not check overlap at all, and is allowed to lower its own thresholds until it stops complaining. The final shipped boxes are additionally mutated by post-hoc safe-zone shifting (`v2-adapt.ts:712-730`) **after** verification and **after** plan selection, and those residual violations never reach `TransformPlan.violations` — only `report.warnings`. **Nothing re-verifies the geometry that actually ships.**

---

## 4. What went wrong

### 4.0 First, a correction about which documents matter

The task named the three root-level markdowns. **They are the weakest sources in the repo.** `AAE_AUDIT_REPORT.md` (dated 2025-02-10, authored "Cascade AI") and `AAE_PIPELINE_AUDIT.md` (2026-05-25) are stale by at least one refactor and materially wrong about the code (see the DOC != CODE boxes in §1.3 and §1.6). `AAE_ERRORS_AND_FAULTS.md` is a five-item checklist where every item is struck through as RESOLVED — including one, `checker.ts`, that is resolved only in the sense that the file now exists while nothing imports it (§3.1).

The real failure record is **`docs/AAEAuditbyRazi/`** (June 2026, the owner's own audit):

| file | what it is |
|---|---|
| `endtoendrun/EndToEndGroundingJune2026.md` | **the anchor** — a real 5-PSD x 3-size production run, eight observed failure patterns mapped to code paths and re-ranked by observed damage |
| `endtoendrun/AAE Test Run - Document.md` | 2.5 MB raw run notes, per-PSD issue registers, embedded visual evidence |
| `ActionJSONPipelineAuditJune2026.md` | round-trip harness audit, findings F1-F7 |
| `MathematicalFormulas.md` (+ `...VerificationJune2026.md`) | ~80 formula findings with a FIXED/OPEN/DEAD-CODE ledger, and an **empirical re-test that refutes several of them** |
| `PolicyAuditJune2026.md` | 80 KB on why the brand-guideline path is dark in production |
| `DevPass1_Implemented_June2026.md` | what was actually changed, with before/after numbers and an honest handover TODO |

Two process points from that set are worth more than any single bug:
- **They built a round-trip simulator** (`scripts/audit-actionjson-pipeline.mjs`) that re-executes the emitted output under target-app semantics (including group-transform propagation to children) and asserts the result reproduces the intended layout. 12/12 tests, no Adobe API needed. *"No live Adobe API needed for any of this — the ActionJSON build path is pure local math."* That harness is the reason the June fixes are trustworthy and the earlier audits are not.
- **They re-tested their own findings.** `MathematicalFormulasVerificationJune2026.md` confirms most and **refutes some** (finding 3.3; 6.4 "non-uniform child scaling" declared unreachable). Auditing without re-testing produced the stale root docs; auditing with re-testing produced a usable ledger.

### 4.1 The eight observed failure patterns (production, 5 PSDs x 3 sizes)

Verbatim in structure from `EndToEndGroundingJune2026.md` §2, with the code path each was traced to.

**P-A · Grouped/folder assets separate or scale independently — the #1 defect.**
Mechanism, fully traced: `scene-parser` flattens a PSD folder into `[groupNode, child1, child2, ...]`; the old `detectGroups` kept **only the group node**; the children were re-pooled into proximity clustering and each received *its own* role-based transform — "a nested overlay covers, a label anchors, a silhouette preserves" — so they desynced. Downstream, `decision-mapper` then **drops** children marked `containedByGroup`, trusting a group op that does not reproduce the children's transforms. Observed as *"text leaves the pink blocks"*, *"silhouettes split"* (Octobro, called the strongest pattern), plus EPL/BGMI/Sega/A1 instances. Convergence of four separately-filed findings: pipeline-F1, math-10.2, math-4.4, math-5.3. **Fixed** in Dev Pass 1 by keeping children inside the group component so they inherit one rigid transform (T07 210px error -> 0px).

**P-B · Background/overlay full-bleed failure — a new gap nobody had filed (NF-R1).**
`types.ts:30-31` documents overlay and pattern as "coupled with bg". **No coupling was implemented.** `bg` got `cover`; overlay/pattern got `cover` only if >50% of *source* canvas area, else `preserve`. Two failures: a small gradient overlay fell to `preserve` and left a visible *"darker shade line / rectangular edge"*; and where both did cover, each computed cover from **its own** source bbox, so a nested or over-bleeding overlay desynced from bg at the target size. Compounded by **DEV-1**: `clampBoxScaleRange` was capping a correct 2.84x cover down to the role's `scaleRange.max` of 1.5x — *"the real 'background not full-bleed'"*. Both fixed.

**P-C · Aspect-ratio recomposition failure — landscape/portrait just reuse the source layout.**
*"Square layout reused, CTA/hero overlap"* at 1024x600; *"composition shifted/reused rather than recomposed"*. Two causes. (i) `selectLayoutMode` is **direction-asymmetric**: verified that `0.5 -> 1.0` selects `recompose` while `1.0 -> 0.5` selects `reflow`, because the denominator is `sourceAR` (`breakpoints.ts:89`). A landscape->portrait swap and its inverse get different treatment. (ii) **NF-R2, still open and marked ARCH:** *"the 'recompose' path is a handful of anchor-strategy seeds, not true per-ratio layout templates. There is no notion of 'for 1024x600: heroes left, headline/CTA right, logo top-left'... The seeds reposition individual roles but never rebuild composition zones, so a landscape target ends up as a rescaled square. This is a design-rules gap, not a math bug."*

**P-D · Off-canvas assets, especially logos.**
*"Astro One logo missing"* = anchored off-canvas. `applyAnchorTransform` had **no post-clamp**; the ActionJSON builder **emitted off-canvas boxes verbatim**; the checker only flagged, never corrected. A synthetic probe reproduced a logo at `left = -180` on a 1080 canvas. **Fixed** by a component-level clamp pass before node mapping (off-canvas elements went from up to 9 per test to <=1, the exempt bg).

**P-E · Hero scale and crop failures.**
Three distinct causes, all instructive:
- **NF-C1:** `heroScale` was a **replacement** for contain-scale on the `preserve` path but a **multiplier** on the `anchor` path — a verified **2x size difference for the same nominal value** (0.67 vs 1.34). Since hero routing between `preserve` and `anchor` depends on the target AR bucket, *the same hero came out a different size at each of the three target sizes.*
- **NF-C0:** the hero/logo **upscale caps were dead** — computed and never applied. A raster logo was verified upscaling **4x**.
- **NF-R3 (still open):** *"no concept of protected sub-regions (faces, the floating product)... Cropping is measured by area only, so 'a character is cut' passes as long as the overall area loss is under the role threshold."* This is the most important unsolved *modelling* problem in the engine.

**P-F · Logo/brand-mark placement and scale** — a composite of P-D (no clamp), NF-C0 (dead caps), NF-C2/NF-E1 (logo crop computed as **area** when the allowance is **linear** — "a logo at 0.9 linear is a 10% crop, not 19%"), and NF-R4 (proximity clustering absorbing a `logo` into the bg cluster, which erased its role transform entirely).

**P-G · Brand guideline never converted into measurable rules — the policy findings.**
This is the largest strategic failure and it is worth quoting the ranking: **POL-R1, severity critical: "the brand guideline did not influence output."** The reason, from the policy audit's thesis: the `brandGuide` tool input (Path A) *"only yields `constraints` + `roleWeights` and never reaches layout composition"*, and *"Path A and the policy system (Path B) are wired to different inputs and never connect."* The run confirmed it end-to-end — **a guideline changed nothing visible.** Two more:
- **POL-R2:** a reference *image* cannot become rules. Users attached images as "guideline/reference"; there is no vision-extraction step turning an image into anchors/zones/scale ratios, so *"reference-only prompts are inert (P3 == P2)"*. Filed explicitly as a feature gap, not a bug — *"but it must be stated so reference-only refinement isn't expected to work."*
- **POL-R3:** a safe-zone guide was **misread as "shrink everything"** — the engine shrank the whole composition and produced white margins, instead of *full-bleed bg + safe-zone foreground*. The rule "**bg ignores the safe zone; only foreground respects it**" was never expressed anywhere. Still open.
- **POL-R4:** guidelines carry no per-aspect-ratio layout, so all three sizes inherit one intent.

**P-H · Process/source failure** (expired presigned URL) — correctly separated as infrastructure, not adaptation.

### 4.2 What the run said did *not* matter

Equally valuable, because it shows where audit effort was wasted (`EndToEndGrounding` §3):
- **math 6.4, non-uniform child scaling** — filed high, then **refuted as unreachable** by the verification pass; the run showed no glyph or shape stretching.
- **The dead V1 candidate scorer** — *"invisible, as predicted (dead code)."*
- **The divide-by-zero / NaN guard family** — no degenerate canvases occurred; defence-in-depth, not urgent.
- **The unbounded-refinement findings (8.1/8.2/NF-F1)** — and a sharp methodological catch: *"the run's 'P2/P3 didn't improve' is **not** evidence for these. P2/P3 are separate user re-prompts, not iterations of the internal refinement loop."* Someone nearly attributed a product failure to the wrong subsystem.

### 4.3 The architectural regrets, stated plainly

Reading the code and the June docs together, six regrets are legible. These are the transferable ones.

1. **Two layers of geometry were allowed to exist, and they drifted.** `overlap-resolver::previewBoxes` re-implements a subset of `compile.ts`'s transforms so Stage 2 can "see" the result. It handles three of nine transform kinds and returns raw source-space boxes for the rest. The same pattern recurs as `verifyHardConstraints` existing in **both** `compile.ts:935` and `checker.ts:368` — and the identical area-vs-linear logo bug had to be fixed **twice**, in both copies (`NF-C2` and `NF-E1`). *One geometry evaluator, or the copies will disagree.*
2. **A rich permission model was designed and then bypassed.** `ComponentConstraints` (crop/reorder/wrap/aspect/minSize/maxCrop) is populated per component and read once, for one field. The transform decision instead lives in a 200-line role cascade that re-derives the same facts from the role string. Two sources of truth for "what may happen to this element", one of them inert.
3. **The output was allowed to escape the check.** Safe-zone clamping runs after selection and after verification (`v2-adapt.ts:712-730`); the boxes that ship are never re-verified, and safe-zone residuals never enter `TransformPlan.violations`.
4. **The feedback loop was allowed to weaken the test.** `adjust_constraints` lowers `minFontSize`/`minCtaSize` and raises `maxLogoCrop` so the *same* layout scores clean. Bounds were added late; the incentive shape was not changed.
5. **Roles were named twice and the aliases bit.** Seven legacy role names coexist with the canonical set; code that branched on `'decor'` silently never matched `'decoration'` (`checker.ts:219`, "Razi 7.3"). Backwards compatibility at the vocabulary level cost real bugs.
6. **The "no taste in Stage 3" contract was breached the moment overlap resolution needed pixels.** `SemanticTransform.offset?: Vec2` exists solely so the Stage-2 nudger can write pixel deltas into a type whose doc comment forbids them (`plan.ts:44-48`). Once the exception existed, nothing else defended the boundary.

### 4.4 The unsolved problems, as they stand today

- **Per-ratio layout templates (NF-R2, ARCH, open).** The single most-cited *design* gap: there is no representation of "what a landscape composition is", only per-role anchors. Extreme AR changes therefore produce rescaled source layouts.
- **Protected sub-regions for crop (NF-R3, open).** No faces, no product bounding boxes, no salient regions. Crop is one percentage per role over total area.
- **Brand guideline -> layout bridge (POL-R1, partially proven, ARCH).** A sandbox test proved a `policyIntent` *can* move a logo from `{820,1360}` to `{40,60}`; the production input path still does not reach layout.
- **Image -> rules extraction (POL-R2, open).**
- **Safe-zone semantics (POL-R3, open):** background must be exempt from the safe zone; only foreground respects it. Never encoded.
- **AR-mode direction symmetry (3.2, deferred).**
- **IoU-based overlap and error-count-ranked scoring (checker 7.1/7.2, deferred, and in a module nothing calls anyway).**
- **Text measurement.** Not filed as a finding at all — which is itself notable. `wrap` and `scale-wrap` remain unimplemented in substance (§3.5).
- **Lint gate broken:** *"ESLint v9 has no flat config in this repo; `npm run lint` fails"* — `tsc` is the only gate.

---

## 5. Reusable vs. do-not-repeat

### 5.1 Adopt (concepts and invariants that earned their place)

1. **Two-stage separation: intent, then geometry.** A reasoning stage that emits typed, pixel-free intent; an execution stage that is pure deterministic math. `plan.ts:9` states it; it is the right shape. The failure was not enforcing it (§4.3.6), which a Rust type system can do properly: make the plan type structurally incapable of holding pixels, and put the overlap adjustment in the plan *as a typed constraint*, not as a smuggled `offset`.
2. **Centre-fraction preservation as the geometric baseline** (`solver.ts:92-121`). Map the element's centre as a fraction of each canvas; scale by `min(sx, sy)`. Simple, predictable, and every other rule is a correction on it.
3. **Cover with centre-offset correction** (`solver.ts:50-77`) for full-bleed elements: `max(sx, sy)`, then re-derive the centre through the overflow offset so the focal point does not drift.
4. **Nearest-source-corner detection** as reconstructed intent. "It was top-right, keep it top-right" is the single highest-value heuristic in the engine, and it generalises to any format that lacks constraints (PSD, PDF, flattened SVG).
5. **Role-derived priority ordering** (`plan-generator.ts:650`): logo 100 > hero 90 > title 80 > cta 75 > ... > bg 10. A compact, editable statement of what matters. Keep it as data, not code.
6. **Full-bleed roles must be isolated before clustering** (`components.ts:162`). A canvas-spanning element overlaps everything, so any proximity or overlap algorithm must exclude it first or it swallows the composition. This cost them the "logo missing" bug.
7. **Coupled elements inherit, they do not re-decide.** overlay/pattern must take bg's *resolved target box*, not compute their own cover. Generalise: a coupling relation in the IR ("this element's frame is derived from that one's").
8. **Groups are rigid units, end to end.** The #1 production defect and its fix. Children stay in the group component, the unit gets one transform, and the unit is transformed *before* expansion to members.
9. **Structural-unit-first ordering** (`plan-generator.ts:88-103`) — transform units, then expand — is what stops paired elements drifting.
10. **Canvas-relative spacing tokens**: `tight/normal/loose = 2%/4%/8%` of the axis reference, with vertical resolving against height and horizontal against width.
11. **Linear, not area, for shrink allowances.** "A logo at 0.9 linear is a 10% crop, not 19%." Fixed twice here; get it right once.
12. **Normalised movement metrics.** `REL_EPS = 0.01` of a canvas dimension, comparing centre *fractions* — because in a resizing engine raw-pixel comparison reports everything as moved (`v2-adapt.ts:588`, "Razi 8.F2"). Any fidelity metric must first quotient out the intended resize.
13. **Decision provenance on every element.** `decisionOrigin: ["solver:<policy>", "transform:<kind>", "guide:<id>", "refinement:iter_N"]` is cheap, and it is the only reason the June audit could trace failures to code paths at all.
14. **Best-so-far across refinement iterations** (`v2-adapt.ts:235`) — an iterative improver must never be able to return worse than an earlier iteration.
15. **Degenerate-input guard at the entry point** (`v2-adapt.ts:264`) — reject non-finite/zero canvases once, rather than adding divide-by-zero guards at 30 call sites (which is what they ended up doing anyway).
16. **The round-trip harness as the primary gate.** Re-execute the emitted output under the target renderer's semantics and assert it reproduces the intended layout, offline, with no vendor API. This is the highest-value practice in the repo.
17. **Units resolved only at the policy boundary** (`units.ts`). `%`/`fr`/`px` in authored config; pure px inside the engine.

### 5.2 Reject (and why)

1. **Reject violation-counting as the selection criterion.** `sort(vCount, eCount, idx)` treats "logo 3% too small" and "hero fully off-canvas" as one point each, and cannot express "worse geometry but no hard failure". Use an ordered pair: *(hard-constraint satisfaction as a boolean lattice, then a real-valued quality score)* — and make the score measured (§5.3), not a penalty tally.
2. **Reject a refinement loop that may relax its own constraints.** Constraints are the specification. If a layout cannot satisfy them, the correct outputs are *a different layout* or *an explicit, surfaced failure* — never a quietly lowered threshold. If relaxation is genuinely needed, it must be an input the caller authorised, recorded in the output, and never discovered by the engine.
3. **Reject a second, approximate geometry evaluator.** `previewBoxes` must not exist. If Stage 2 needs boxes, it calls the one compiler. In Rust: one `fn compile(plan, scene) -> Layout`, used by planner, checker and emitter alike.
4. **Reject a permission model you do not consult.** Either `ComponentConstraints` drives the transform selection, or delete it. Do not ship both a declarative constraint record and an imperative role cascade.
5. **Reject legacy role aliases.** Seven aliases, one silent miss (`decor` vs `decoration`) that survived to production. Version the vocabulary at the boundary, translate once, and let the internal enum be exhaustive so the compiler catches every unhandled variant.
6. **Reject transform kinds that fall back silently.** `stack`/`reflow` fell back to `preserve` while emitting a warning nobody actioned; `previewBoxes` falls back to a source-space box with no signal at all. In Rust, an unimplemented transform should be unrepresentable, not a runtime warning.
7. **Reject absolute-pixel tuning constants.** `PROXIMITY_THRESHOLD = 50`, `MAX_CLUSTER_DIAMETER = 500`, nudge cap `50`, `minCtaSize = 120`, `minFontSize = 18`, logo `minSize = 96`. Express thresholds as canvas fractions, physical units (mm/pt at a stated DPI), or explicit accessibility minima with a stated reference size — never bare pixels that silently mean different things at 640x480 and 4096x4096.
8. **Reject non-uniform child scaling by default.** `mapComponentBoxesToNodes` applies independent `scaleX`/`scaleY` (`compile.ts:834`). Uniform scale plus explicit re-anchoring should be the default; anisotropic scaling should require an opt-in per element and be checked.
9. **Reject "the audit says it is fixed" without a reachability check.** `checker.ts` is the cautionary tale: complete, correct, documented as RESOLVED, and imported by nothing. A dead-code / unreachable-symbol gate belongs in CI.
10. **Reject dual output shapes.** `AdaptedLayout` (boxes+styles+report) and `TransformPlan` (source/target boxes + provenance) both exist; the runner consumes the legacy one, the "canonical" one is largely observational. Pick one; if a legacy view is needed, derive it and mark it derived.
11. **Reject `report.cost = 0`, `softScores: {}`, `hardConstraintViolations: []`-that-nobody-reads.** Fields that are always written and never read are worse than absent — they make the system look like it measures things it does not.
12. **Reject inference of structural role from layer-name substrings** as a *primary* path. `name.includes('background')` appears in both `solver.ts:33` and `plan-generator.ts:451`. Fine as a low-confidence hint feeding an explicit confidence field (`Component.role.confidence/method/reasons` already exists and is the right shape); wrong as a branch condition in the geometry code.
13. **Reject the hardcoded design type.** `analyzeBreakpoint(..., 'free_design')` makes an entire configuration table unreachable. If a parameter is always one value, delete the parameter or wire it.

### 5.3 On fidelity specifically

The engine has **no measured fidelity** (§3.5) and its own audit lists PNG generation as missing. Everything it reports — score, cost, violations — is derived from the same box arithmetic that produced the layout. **A system cannot grade itself with the numbers it just made up.** For AutoDev's "fidelity measured, not asserted" phase, the transferable content from this codebase is mostly negative, plus two positives:
- The **normalised-comparison lesson**: any before/after metric must quotient out the intended resize, or it saturates (`v2-adapt.ts:588`).
- The **round-trip harness pattern**: render or re-simulate the *emitted* artefact under the consumer's semantics and compare against intent — the only check in this repo that ever caught anything (T07: 210px error -> 0px).
Everything else (per-role crop percentages, penalty weights, `computeScore`) is an assertion dressed as a measurement.

### 5.4 Adobe-specific coupling — what does not transfer

AutoDev has no Adobe runtime, so these must be treated as *removed*, not ported:

- **`ActionJSON` emission** (`src/aae-orchestrator/actionjson-*.ts`): Photoshop descriptor ops `canvasSize`, `select`, `move`, `transform`, `set`/`textLayer`/`textStyleRange`/`paragraphStyleRange`, `RGBColor`. Entirely vendor-specific.
- **`documentManifest` -> `Scene` parsing** (`scene-parser.ts`): Adobe's manifest shape, `doc.resolution`, 16-bit -> 8-bit RGB normalisation, Photoshop layer-type mapping (`smartObject`/`textLayer`/`layerSection`).
- **Presigned-URL I/O and job orchestration** (`runner.ts`, `manifest-validator.ts`): storage/transport, not adaptation.
- **The `containedByGroup` strategy is an Adobe *semantics* dependency, and it is load-bearing.** `transform-plan.ts:84-92` says group children are dropped from emission because *"children move with it via Photoshop's natural group cascade."* **In a renderer without that cascade, dropping the children loses them.** This is a vendor semantic embedded inside the type the repo calls its "format-agnostic intermediate representation". A Rust IR must instead resolve group transforms **explicitly onto children** (which is also math-finding 10.2, still only "mitigated") and emit absolute per-element geometry. Do not inherit the drop-children strategy.
- **The vector/raster upscale distinction** is implemented as "does this component contain <=1 `smartObject`" (`compile.ts:101`) — a Photoshop proxy for "is this vector". Keep the *rule* (never upscale raster; vectors may upscale to a cap), replace the *detection* with real asset-type knowledge from the ingest layer.
- **Font handling.** There is none beyond `fontName`/`fontPostScriptName`/`fontStyleName` string passthrough; sizing is delegated to Photoshop's own text engine (`setText`). AutoDev must own text shaping itself — this is a whole subsystem the AAE never had to build, and it is exactly why `wrap` was never implemented.

---

## 6. Vocabulary and data shapes

Compact extraction of the types a new IR would need to be compatible with, or deliberately differ from. `[LIVE]` = consumed by the shipped pipeline. `[DEAD]` = declared, never read.

### 6.1 Geometry and scene (`src/aae/types.ts`)

```ts
interface Box   { left: number; top: number; width: number; height: number }          // [LIVE] absolute px, AABB
interface Vec2  { x: number; y: number }                                              // plan.ts:18
interface CanvasSpec { width: number; height: number }                                // transform-plan.ts:30

type NodeType = 'text' | 'smartObject' | 'shape' | 'group';                           // [LIVE]
interface Node {
  id: number; name: string; type: NodeType; role?: Role;
  z: number; box: Box; parentId?: number;
  rotate?: number;            // [DEAD in layout] carried, never read by any transform
  content?: string;           // single flat string; no runs
  children?: Node[];          // [DEAD] pipeline uses the flat list + parentId
  style?: { fontSize?: number; leading?: number; tracking?: number;
            weight?: 'regular'|'bold'; fontName?: string; fontPostScriptName?: string;
            fontStyleName?: string; color?: {red;green;blue};
            alignment?: 'left'|'center'|'right'; orientation?: 'horizontal'|'vertical' };
}
interface Scene { width: number; height: number; dpi?: number /*[DEAD]*/; nodes: Node[] }

interface Constraints { safeMarginPct; minFontPx; maxFontPx; minLogoPx; grid?: GridStructure /*[DEAD]*/ }
interface RoleWeights  { [role: string]: number }   // [DEAD past config] never reaches layout math
interface CostWeights  { overlap; safeZone; minSize; aesthetic; thirdsAttract?; goldenSplitAlign?;
                         hierarchyPreserve?; spacingConsistency?; anchorDeviation?;
                         outsideSafe?; textOverflow? }                                 // [DEAD] nothing constructs it
interface Anchors      { nodeId; hAnchor; vAnchor; margin?; lockAspect?; preserves? }   // [DEAD]
interface TextFitResult{ size; fontSize?; leading?; tracking?;
                         overflow?; truncated?; needsBreak? }                           // [DEAD] no text measurement exists
```

**Differ deliberately:** an AutoDev IR should keep `Box`/`Vec2` verbatim (they are right), and diverge on: real hierarchy with composed transforms rather than flat+`parentId`; rotation/shear as a proper affine so it cannot be silently dropped; text as runs with measurable metrics rather than one string + one style; explicit asset kind (vector/raster/text/shape) rather than a Photoshop `smartObject` proxy; and a unit/DPI story that is actually used.

### 6.2 Roles and semantics

```ts
type Role =
  | 'bg' | 'overlay' | 'pattern'                                   // cover / full-bleed family
  | 'heroImage' | 'productImage' | 'supportingImage'               // smart-place family
  | 'logo' | 'cta' | 'badge' | 'icon' | 'sticker'                  // preserve-size family
  | 'title' | 'subtitle' | 'bodyText' | 'price' | 'promoText'      // text family
  | 'disclaimer' | 'contactUrl'                                    // edge-pinned, shrink-only
  | 'decoration'                                                   // reposition only
  | 'headline'|'subhead'|'body'|'decor'|'tileTitle'|'tileNewPrice'|'tileOldPrice'; // LEGACY - do not port
function canonicalRole(role?: Role): Role | undefined              // alias normalisation
```
Role priority for conflict resolution (`plan-generator.ts:650`):
`logo 100 · heroImage 90 · productImage 88 · title 80 · cta 75 · subtitle 65 · bodyText/price 55 · promoText 50 · badge 40 · icon/sticker 35 · disclaimer/contactUrl 30 · decoration 20 · pattern 12 · overlay 11 · bg 10`

Role confidence is modelled (and is the right shape, though only `value` is used):
```ts
role?: { value: Role; confidence: number /*0-1*/;
         method: 'name'|'position'|'type'|'heuristic'; reasons: string[] }   // components.ts:46
```

### 6.3 Component / graph layer (`src/aae/components.ts`)

```ts
type ComponentType = 'cluster' | 'grid' | 'stack' | 'single';
type RelationType  = 'promo_cluster'|'hero_cluster'|'text_stack'|'grid_cell'
                   | 'decoration_of'|'proximity'|'alignment'|'stacking'|'grouping';

interface Component { id; type; nodes: Node[]; role?; bbox: Box;
                      relationships: Relationship[]; constraints: ComponentConstraints;
                      metadata: { isGrid?; gridCols?; gridRows?; stackDirection?; clusterDensity? } }

interface ComponentConstraints {   // [almost entirely DEAD - only reorderAllowed is read]
  minSize?; maxCrop?; cropAllowed; readabilityProtected; reorderAllowed;
  wrapAllowed?; aspectRatioLocked?; anchorPreference?;
}

type StructuralUnit = { type:'single'; componentId }                                  // [LIVE]
                    | { type:'cluster'; componentIds: string[]; relation: RelationType }
                    | { type:'grid'; componentIds: string[]; rows; cols };

interface LayoutGraph { sourceCanvas: {width;height}; components: Component[];
                        structuralUnits: StructuralUnit[];                            // [LIVE]
                        normalizedGeometry: {...};                                    // [DEAD - logged only]
                        compositionType: 'centered'|'grid'|'asymmetric'|'flowing';    // [DEAD - logged only]
                        hierarchy: Array<{componentId; level; importance}>;           // [DEAD - logged only]
                        zones?: { hero?; header?; footer?; sidebar? } }               // [DEAD]
```

### 6.4 Plan layer (`src/aae/plan.ts`) — the contract worth stealing

```ts
type Anchor = 'center'|'top-left'|'top-center'|'top-right'|'left-center'|'right-center'
            | 'bottom-left'|'bottom-center'|'bottom-right'|'golden-left'|'golden-right';
// golden-left = 0.382w, golden-right = 0.618w  (utils.ts:31-36, "Razi 3.4" — it used to be
// rule-of-thirds 0.333w; they changed the math to match the name, not the name to match the math)

type SemanticTransform =        // 9 kinds; full field lists in section 1.4
  preserve | anchor | relative | stack | reflow | fit | cover | fill-region | static

type TextDirective = 'preserve' | 'scale' | 'wrap' | 'scale-wrap';
//   scale / scale-wrap -> multiply fontSize by min(box scale), floor at minFontSize,
//                         preserve the leading/fontSize ratio
//   wrap / preserve    -> IDENTICAL no-ops. Wrapping is NOT implemented.

interface Violation { type: 'font_too_small'|'cta_too_small'|'logo_scaled_down'
                          |'transform_unimplemented'|string;   // open union - see the dead-branch bug
                      componentId: string; severity: 'error'|'warning';
                      message: string; value?: number; threshold?: number }

interface LayoutPlan {
  targetCanvas: {width;height}; bucket: ARBucket; mode: LayoutMode; policy: string;
  components: Array<{ componentId; transform: SemanticTransform; textDirective?; priority: number;
                      scaleRange?: [number,number]; maxCrop?; margin?; clearSpace?/*[DEAD]*/;
                      zIndex?; fit?; regionFraction?; alignment? }>;
  overlapResolutions: Array<{ movedComponentId; reason; delta: Vec2; iteration }>;  // written, never read
  report: { score: number; hardConstraintViolations: string[]; softScores: Record<string,number> };
                                                                                   // [DEAD - all three]
}

interface CandidateSeed { name;
  anchorStrategy: Anchor;                                                      // [DEAD]
  stackingOrder: 'original'|'hero-first'|'cta-first'|'z-order';                // [DEAD]
  heroPlacement: 'preserve'|'left'|'center'|'right'|'top'|'bottom'
               |'fill-left'|'fill-right'|'fill-top'|'fill-bottom';             // [LIVE]
  ctaPlacement: 'preserve'|'bottom'|'under-hero'|'side'|'top';                 // [LIVE]
  textStrategy: 'scale'|'wrap'|'scale-wrap' }                                  // [LIVE]
```

### 6.5 Breakpoints (`src/aae/breakpoints.ts`)

```ts
type ARBucket   = 'tall_portrait'|'portrait'|'square'|'landscape'|'wide'|'ultra_wide';
type LayoutMode = 'preserve'|'reflow'|'recompose';
interface BucketPolicy { gridCols; stackingAllowed; heroScaling:'preserve'|'shrink'|'reposition';
                         ctaPlacement:'bottom'|'under_hero'|'side'|'flexible';
                         textWrap; decorCrop; reorderElements }   // mostly inert - see section 2.4
interface BreakpointContext { sourceBucket; targetBucket; sourceAR; targetAR; arDistance;
                              layoutMode; policy; isPortraitToLandscape; isLandscapeToPortrait }
```

### 6.6 Units (`src/aae/units.ts`) — port as-is

```ts
type UnitValue = number | string | { value: number; unit: 'px'|'%'|'fr' };
interface SpacingSpec     { top?: UnitValue; right?: UnitValue; bottom?: UnitValue; left?: UnitValue }
interface ResolvedSpacing { top: number; right: number; bottom: number; left: number }
resolveUnit(v, refPx) -> px          resolveOptionalUnit(v, refPx, defaultPx)
resolveSpacing(spec, W, H)           // L/R resolve vs W, T/B vs H
resolveAsFraction(v, refPx) -> 0..1
```

### 6.7 Policy / brand-guide layer (`src/policy/types.ts`, `src/aae/config/brand-guide.schema.ts`)

This is the layer AutoDev's "brand-guideline application" task maps onto. It is well-typed and **it never reached layout in production (POL-R1)** — the shapes are worth studying; the wiring is not.

```ts
interface Policy { id: "{brandId}:{variant}:v{version}"; name; version;
  meta: PolicyMeta; scope: PolicyScope;
  defaults: AdaptationRuleSet;
  sizeCategories?: Partial<Record<'landscape'|'portrait'|'square', Partial<AdaptationRuleSet>>>;
  sizeOverrides?: Record<"WIDTHxHEIGHT", Partial<AdaptationRuleSet>>;
  roleOverrides?: Record<layerName, Role> }   // layer names are source-file constants, so not per-size
// resolution order: engine defaults -> guide defaults -> size category -> exact size; deepest wins

interface ResolvedPolicy {   // flat, no optionals, carries provenance - the right output shape
  policyId; policyVersion; policyType; approvalState;
  brandId; targetWidth; targetHeight;
  composition?: CompositionStrategy; layout?: LayoutPreferences;
  elements: Partial<Record<Role, ElementRule>>;
  constraints: AdaptationConstraints; safeZone: SafeZone;
  tolerances: AdaptationTolerances; scoring: ScoringConfig;
  roleOverrides: Record<string, Role> }

interface ElementRule {
  transform?: 'preserve'|'anchor'|'cover'|'contain'|'relative'|'stack';
  fit?: 'contain'|'cover'|'fill'|'preserve';  anchor?: AnchorPosition;  region?: UnitValue;
  relativeTo?: { role: Role; where:'above'|'below'|'left'|'right'; gap?: UnitValue; align? };
  stack?: { direction:'vertical'|'horizontal'; gap?: UnitValue; align? };
  scale?: { min?; max?; uniform? };   crop?: number;   margin?: SpacingSpec;
  alignment?: { horizontal?; vertical? };
  clearSpace?: UnitValue;    // [DEAD] schema comment literally says "gap enforcement (future)"
  gap?: UnitValue;  zIndex?;
  maxSizePx?: number;        // [DEAD]
  protectedArea?: boolean;   // [DEAD] written by policy-maker.ts:105, read by nobody
  safeZoneExempt?: boolean;  // [LIVE] policy-resolver.ts:260
  priority?: number }        // multiplier on the role priority table

interface CompositionStrategy { focalPoint?: { horizontal: FocalPosition; verticalBias? };
                                contentZone?: ContentZone }
type FocalPosition = 'far-left'|'left-center'|'center'|'right-center'|'far-right';
type ContentZone   = 'lower-third-center'|'bottom-center'|'middle-right'|'middle-left'
                   | 'left-center'|'right-center'|'far-left'|'far-right';

interface AdaptationConstraints { minFontSize; maxFontSize; minCtaSize; maxLogoCrop;
                                  maxOverlap /* never checked anywhere */; minLogoSize;
                                  keepLogoInSafeZone; maxLogoScaleVector; maxLogoScaleRaster }
interface SafeZone { top: number; right: number; bottom: number; left: number }   // px, resolved
type PolicyApprovalState = 'draft'|'pending'|'approved'|'rejected'|'deprecated';
type PolicyStrength      = 'required'|'recommended'|'optional';
type PolicyConfidence    = 'high'|'medium'|'low';
```

**`FocalPosition` + `ContentZone` are the seed of the missing per-ratio template idea (NF-R2).** They name composition *zones* rather than per-element anchors — exactly what "recompose" needed and never got. An AutoDev IR should promote this from a policy hint to a first-class layout construct: a target size resolves to a set of named regions, and roles are assigned to regions.

### 6.8 Output layer (`src/aae/transform-plan.ts`)

Full shapes are in §1.3. `TransformPlan` / `TransformElement` are the closest thing here to a portable adaptation record. Port `sourceBox` / `targetBox` / `role` / `decisionOrigin` / `cropRatio`. Do **not** port `containedByGroup` + child-dropping (§5.4). Add what is missing: per-element **style** deltas (they live in a separate `styles` map today, so the plan alone cannot reproduce the output), an explicit **affine** rather than a `scale`+`translate` summary, and the **check result for the boxes that actually ship** (today the plan's `violations` describe the pre-safe-zone geometry).

---

## Candidate tasks for PD4

Atomic, orderable. Each line: **task** — *rationale, traceable to what was read*.

**A. IR and vocabulary**

1. **Define `Frame` (affine + AABB) and `Canvas` primitives in the IR.** — AAE's `Box{left,top,width,height}` is right for layout math but silently drops `rotate` (`types.ts:103`, carried and never read by any transform).
2. **Define a closed `Role` enum with no legacy aliases, plus a versioned translation at ingest.** — Seven aliases in `types.ts:53-60` caused a live bug where code branching on `'decor'` never matched `'decoration'` (`checker.ts:219`).
3. **Model role assignment as `{ value, confidence, method, reasons }`.** — `components.ts:46` already has this shape; only `value` is ever used, so provenance was designed and thrown away.
4. **Make hierarchy real: parent transforms compose onto children at parse time.** — Math finding 10.2 is still only "◐ mitigated"; the flat `[groupNode, child, child]` model is the direct cause of the #1 production defect (group desync).
5. **Model text as runs with measurable metrics (family, size, leading, tracking, per-run).** — `Node.style` is a single style over a single `content` string; `TextFitResult{overflow,truncated,needsBreak}` is a dead type because nothing measures text.
6. **Carry explicit asset kind (vector / raster / text / shape) from ingest.** — `isVectorLikeComponent` guesses "vector" as "<=1 smartObject" (`compile.ts:101`), a Photoshop proxy that will not exist in AutoDev.
7. **Port `UnitValue`/`SpacingSpec`/`resolveUnit`/`resolveAsFraction` essentially verbatim, resolving only at the policy boundary.** — `units.ts` is the cleanest module in the engine and the px-only-inside rule held up.

**B. Layout intent**

8. **Implement `centre-fraction preservation with uniform min(sx,sy) scale` as the baseline transform.** — `solver.ts:92-121`; every other rule in AAE is a correction on this.
9. **Implement `cover` with centre-offset correction for full-bleed elements.** — `solver.ts:50-77`; naive cover drifts the focal point.
10. **Implement `detect_nearest_source_corner` and corner-anchored placement.** — `plan-generator.ts:470/593/611`; the highest-value reconstructed-intent heuristic, and the only thing keeping corner elements in their corners.
11. **Make coupling a first-class IR relation ("this element's frame derives from that one's") and use it for bg/overlay/pattern.** — NF-R1: overlay was documented as "coupled with bg" and the coupling was never implemented, producing the "darker shade line" across five test assets.
12. **Treat named groups as rigid units: transform the unit, then expand to members.** — Rank 1 fix; `plan-generator.ts:88-103` unit-first ordering took T07 from 210px error to 0px.
13. **Exclude full-bleed roles from proximity clustering before it runs.** — `components.ts:162` NF-R4; a canvas-spanning bg absorbed logos and overlays into one cluster and erased their transforms.
14. **Express all layout thresholds as canvas fractions or physical units; ban bare-pixel constants.** — `PROXIMITY_THRESHOLD=50`, `MAX_CLUSTER_DIAMETER=500`, nudge cap `50`, `minCtaSize=120` all mean different things at 640x480 vs 4096x4096.
15. **Adopt the canvas-relative gap scale (tight 2% / normal 4% / loose 8%, axis-bound).** — `compile.ts:159-170`; the one spacing decision that scaled correctly.
16. **Build per-ratio composition templates: a target size resolves to named regions, roles bind to regions.** — NF-R2, Rank 6, still ARCH/open: *"the recompose path is a handful of anchor-strategy seeds, not true per-ratio layout templates... a landscape target ends up as a rescaled square."* `FocalPosition`/`ContentZone` (§6.7) are the seed of this.
17. **Make the aspect-ratio mode selector direction-symmetric.** — math 3.2, verified: `0.5 -> 1.0` selects `recompose` but `1.0 -> 0.5` selects `reflow`, because `selectLayoutMode` divides by `sourceAR` (`breakpoints.ts:89`).
18. **Encode "background ignores the safe zone; only foreground respects it" as an explicit rule.** — POL-R3, still open: a safe-zone guide was misread as "shrink everything", producing white margins instead of full-bleed bg + inset foreground.

**C. One compiler, one truth**

19. **Write exactly one `compile(plan, scene) -> Layout` and have the planner, the checker and the emitter all call it.** — `overlap-resolver::previewBoxes` is a second, partial geometry evaluator that returns *source-space* boxes for six of nine transform kinds.
20. **Make every transform kind total: no silent fallback to `preserve`.** — `stack`/`reflow` fell back for months while emitting a warning nobody actioned (`AAE_AUDIT_REPORT.md` M1); an unimplemented arm should not compile.
21. **Default to uniform scale when mapping a group frame onto members; make anisotropic scaling opt-in and checked.** — `mapComponentBoxesToNodes` (`compile.ts:834`) applies independent scaleX/scaleY, so any non-uniform group frame stretches its children with nothing to catch it.
22. **Apply shrink allowances as linear ratios, never area ratios.** — NF-C2 / NF-E1: *"a logo at 0.9 linear is a 10% crop, not 19%"*, a bug that had to be fixed in two copies of the same function.
23. **Apply upscale caps: raster never above 1.0x, vector to an explicit cap.** — NF-C0: the caps were computed and never applied; a raster logo was verified upscaling 4x.
24. **Make a scale parameter mean one thing on every code path.** — NF-C1: `heroScale` was a *replacement* on the preserve path and a *multiplier* on the anchor path — a verified 2x difference for the same nominal value, giving the same hero a different size at each target size.
25. **Clamp non-full-bleed elements inside the canvas at the unit level, before member expansion, and surface the clamp.** — Rank 2 fix; took off-canvas elements from 9/test to <=1 and cured "logo missing".

**D. Checking and fidelity**

26. **Give every invariant a typed identity shared by the emitter and the analyser (no string matching).** — Three of six refinement branches key on violation type strings that `compile.ts` never emits (`overlap_detected`, `constraint_violated`, `transform_unimplemented`), so `gapMultiplier` is permanently 1.0 in the shipped pipeline.
27. **Implement overlap as a real, role-aware invariant with an IoU or occlusion measure.** — `HardConstraints.maxPrimaryOverlap` (default 10) is declared in `candidates.ts:24` and checked nowhere; the role-aware `isOverlapAllowed` lives only in dead `checker.ts:157`.
28. **Re-verify the geometry that actually ships, after every post-hoc adjustment.** — safe-zone clamping runs after selection and after verification (`v2-adapt.ts:712-730`) and its residuals never enter `TransformPlan.violations`.
29. **Forbid the refinement loop from mutating its own thresholds; make relaxation a caller-authorised, recorded input.** — `adjust_constraints` lowers `minFontSize`/`minCtaSize` and raises `maxLogoCrop` so the same layout scores clean; bounds were added late and do not change the incentive.
30. **Select layouts by (hard-constraint satisfaction, then a real-valued quality score), not by counting violations.** — `selectBestCompilation` sorts on `(count, errorCount, index)`, so "logo 3% small" and "hero fully off-canvas" weigh the same.
31. **Keep best-so-far across refinement iterations and never return worse than an earlier one.** — `isBetterCompilation` ("Razi 8.3", `v2-adapt.ts:235`) was added precisely because refinement returned the last result rather than the best.
32. **Build a renderer and make fidelity a measured comparison of rendered source vs rendered output.** — there is no rasterisation anywhere in AAE; `AAE_PIPELINE_AUDIT.md:539` lists PNG generation as missing, and every "score" is a penalty tally over the same boxes the engine just produced.
33. **Normalise every before/after metric against the intended resize before reporting it.** — "Razi 8.F2" (`v2-adapt.ts:588`): raw-pixel comparison made *everything* read as moved/scaled, "making the metric low-signal"; comparing centre fractions with `REL_EPS=0.01` fixed it.
34. **Model protected sub-regions (faces, product bounds, focal points) and check crop against them, not against total area.** — NF-R3, still open: *"'a character is cut' passes as long as the overall area loss is under the role threshold."*
35. **Implement real text shaping: measure, line-break, detect overflow, and make `wrap` mean something.** — `applyTextOperations` does no measurement at all; `wrap` and `preserve` are identical no-ops, so the whole `textWrap`/`scale-wrap` vocabulary is currently a lie.
36. **Add a legibility invariant beyond a pixel floor (contrast against the composited backdrop, minimum rendered x-height).** — the only legibility check in AAE is `fontSize >= 18`, applied to a font size the engine itself just chose.

**E. Process gates (cheap, high return)**

37. **Build the round-trip harness first: re-execute the emitted artefact under the consumer's semantics and assert it reproduces the intended layout, offline.** — `scripts/audit-actionjson-pipeline.mjs`, 12/12, is the only check in the repo that ever caught anything, and it needed no vendor API.
38. **Add a dead-code / unreachable-symbol gate to CI.** — `checker.ts` is complete, correct, marked RESOLVED in the audit, and imported by nothing; `candidates.ts` carries ~500 lines of dead scorer; `ComponentConstraints` has 8 fields and 1 reader.
39. **Ban write-only fields: if a struct field is never read, delete it.** — `report.cost = 0`, `softScores: {}`, `hardConstraintViolations` (written by the overlap resolver, read by nobody), `overlapResolutions`, `zones`, `normalizedGeometry`, `compositionType`, `hierarchy`.
40. **Require every audit finding to be re-tested against compiled code before it is accepted or closed.** — the June verification pass **refuted** several accepted findings (3.3; 6.4 declared unreachable), while the un-retested root-level audits describe an engine-routing flag and a gradient-descent solver that do not exist.
41. **Require an end-to-end run on real assets across the full target matrix before declaring a phase done.** — the 5-PSD x 3-size run reordered the entire defect priority list and surfaced five findings (NF-R1..R5) that three prior desk audits had all missed.
42. **Separate "the user re-prompted and it did not improve" from "the internal loop diverged" in every failure report.** — `EndToEndGrounding` §3 catches exactly this misattribution: *"P2/P3 are separate user re-prompts, not iterations of the internal refinement.ts loop."*
43. **Wire the brand-guideline input to layout composition on day one, with a test that proves a guideline changes the output.** — POL-R1, severity critical: *"the brand guideline did not influence output"*; the sandbox proof (a `policyIntent` moving a logo from `{820,1360}` to `{40,60}`) came a year after the policy schema was written.
