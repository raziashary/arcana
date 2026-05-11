# AutoDev Graph Engine — Visualization Refinement Specification
### Phase 6.14: Graphify Tab — Graph Canvas Design

**Document Status:** Draft v1.0  
**Author:** Razi (Programming Lead, AIRS Foundry)  
**Date:** May 2026  
**Scope:** Graphify tab — D3.js canvas rendering, interaction model, layout algorithms, visual encoding  
**Companion Docs:** `AutoDev_Graph_Engine_Spec`, `AutoDev_UI_Spec_v2`  
**Reference Projects:** `graphify-5`, `code-review-graph`, `emerge` (glato/emerge)

---

## Table of Contents

1. [Problem Diagnosis: What Is Wrong With the Current Graph](#1-problem-diagnosis-what-is-wrong-with-the-current-graph)
2. [Reference Benchmarks](#2-reference-benchmarks)
3. [Core Design Principles](#3-core-design-principles)
4. [Rendering Architecture & Level-of-Detail System](#4-rendering-architecture--level-of-detail-system)
5. [Force Simulation Parameters](#5-force-simulation-parameters)
6. [Community Hull System](#6-community-hull-system)
7. [Node Visual Encoding](#7-node-visual-encoding)
8. [Edge Visual Encoding](#8-edge-visual-encoding)
9. [Label System](#9-label-system)
10. [Canvas Interaction Model](#10-canvas-interaction-model)
11. [Node Inspector Panel](#11-node-inspector-panel)
12. [Communities Panel](#12-communities-panel)
13. [Toolbar & Controls](#13-toolbar--controls)
14. [Stats Bar](#14-stats-bar)
15. [Render Mode Specifications](#15-render-mode-specifications)
    - 15.1 Full Mode (< 300 nodes)
    - 15.2 File Mode (300–1500 nodes)
    - 15.3 Community Mode (> 1500 nodes)
    - 15.4 Focus Mode (on-demand, any count)
    - 15.5 Impact Mode (blast radius overlay)
16. [Color Palette](#16-color-palette)
17. [Loading & Build States](#17-loading--build-states)
18. [Tab Layout & Panel Sizing](#18-tab-layout--panel-sizing)
19. [Performance Constraints](#19-performance-constraints)
20. [D3 Implementation Notes](#20-d3-implementation-notes)
21. [React Component Map](#21-react-component-map)

---

## 1. Problem Diagnosis: What Is Wrong With the Current Graph

The screenshot reveals **seven distinct visual problems** that together produce an unreadable "hairball" graph despite correct underlying data (926 nodes, 703 edges, 9 communities).

### 1.1 The Central Mass Problem ("Big Ball of Mud")

The center of the graph is a dense solid cluster of hundreds of hollow circle nodes pressed together. This is the classic D3 force-directed failure mode: all low-degree nodes attract toward the center because the charge repulsion is too weak to push them apart from each other, while the link force pulls them inward.

**Root cause**: Default `forceCharge` strength (typically `-30`) is far too weak for 900+ nodes. Low-degree nodes (degree ≤ 2) have no strong repulsion neighbors and collapse into a pile. There is also no collision detection preventing node overlap.

**Fix required**: Stronger charge (`-180` to `-300` depending on count), `forceCollide` with per-node radius, and cluster-grouped initial positioning (not random) so nodes start near their community center rather than at the canvas center.

### 1.2 No Community Visual Separation

The 9 communities are invisible in the current layout. There is no spatial grouping (nodes from the same community are scattered across the canvas), no convex hull or blob rendered around each community, and no color difference between communities — all nodes appear in the same teal/mint color.

The user cannot answer the question "which cluster is this node in?" by looking at the graph.

**Fix required**: 
1. Pre-position nodes at their community centroid before simulation starts (cluster-grouped seeding)
2. Add a gentle `forceCluster` component that pulls nodes toward their community center
3. Render `d3.polygonHull` convex hulls with per-community fill behind the nodes
4. Color nodes by community (distinct palette, 10 colors)

### 1.3 No Node Size Hierarchy

All nodes appear roughly the same size regardless of degree. God nodes — the 10 most connected architectural abstractions — are invisible in the crowd. A `LivingAgent` class with 40 edges looks identical to a `__init__.py` with 1 edge.

**Fix required**: Node radius on a log scale: `r = 4 + Math.log(1 + degree) * 4`. Cap at 24px. God nodes (degree ≥ 15) get an additional outer glow ring to highlight them.

### 1.4 Undifferentiated Edge Color

All edges appear as the same faint light color. It is impossible to distinguish:
- `CALLS` (function invocations — most structurally important)
- `IMPORTS_FROM` (module dependencies)
- `INHERITS` (class hierarchy)
- `TESTED_BY` (test coverage — critical for QC)
- `CONTAINS` (structural nesting — visually noisy)

**Fix required**: Per-kind edge color with explicit legend. `CONTAINS` edges hidden by default (they are visually noisy structural containment). Toggle-able by edge type filter.

### 1.5 All Labels Always Visible

All 926 node labels are always rendered, producing illegible overlapping text throughout. The label font is too small to read but too present to ignore.

**Fix required**: Zoom-dependent label visibility. Labels only render when the effective pixel size of the node exceeds a threshold (e.g. node screen-radius > 8px at current zoom). At low zoom, only god nodes show labels. At high zoom, all visible nodes show labels.

### 1.6 Node Shape Is Not Meaningful Enough

The current shape encoding (diamond = class, circle = file/function) is too subtle — diamonds and circles at 6–8px size look nearly identical. The difference doesn't register at a glance.

**Fix required**: 
- Encode **kind** via shape AND color combined
- Use shapes with enough visual area difference to be distinct at small sizes
- Supplement with a clear legend in the right panel

### 1.7 No Focus / Selection State

Clicking a node highlights it but the visual change isn't dramatic enough. The 100+ connected neighbors don't dim or highlight differently. It's hard to trace what connects to what.

**Fix required**: Selection puts the canvas into **Focus Mode** — selected node and its 1-hop neighbors are full opacity, everything else dims to `opacity: 0.08`. Connected edges glow. This isolates the neighborhood visually.

---

## 2. Reference Benchmarks

These three tools define the target quality bar for the AutoDev graph canvas:

### 2.1 emerge (glato/emerge) — Community Hull Standard

emerge uses D3 force-directed layout + Louvain clustering + **convex hull polygons** drawn behind each community. The key insight: spatial grouping + hull polygons + per-community color makes cluster structure immediately readable even at 500+ nodes. Hulls that overlap indicate architectural coupling between communities — a design smell visible at a glance.

**What AutoDev borrows**: Convex hull rendering, community-based initial positioning, hull overlap as a coupling signal.

### 2.2 NDepend Dependency Graph — Scale & Hierarchy Standard

NDepend's graph renders 4,000+ nodes readably by using **box nesting** (children nested inside parent boxes) and **proportional node area** (box area = lines of code). The architectural hierarchy is always visible regardless of zoom level. Color-on-hover highlights: red = mutual dependency, blue = direct use, light blue = indirect use.

**What AutoDev borrows**: Color-on-hover impact visualization for blast radius overlay, the principle that hierarchy must be encoded spatially not just via color.

### 2.3 code-review-graph — Tooltip & Panel Standard

code-review-graph's node inspector panel surfaces qualified_name, file_path, line range, callers, callees, and tests in a compact dark-theme sidebar. The `INCOMING (N)` / `OUTGOING (N)` layout with expandable lists is clean and functional.

**What AutoDev borrows**: Node inspector layout, the compact caller/callee lists with count badges.

---

## 3. Core Design Principles

### P1 — Community First
The primary organizational unit of a code graph is the **community**. The visualization must make community structure visible at every zoom level. Before seeing individual functions, the user sees modules. This matches how software architects reason.

### P2 — Hierarchy of Attention
Visual salience must match structural importance:
1. **God nodes** — largest, brightest, labeled always
2. **Regular class/function nodes** — medium, community-colored, label on zoom
3. **File hub nodes** — smaller, dimmer (they are mechanical, not semantic)
4. **Isolated nodes** — smallest, dim gray, no label at low zoom

### P3 — Edges Are Information, Not Decoration
Every edge type encodes a specific relationship. The user must be able to see at a glance: "these two components call each other" vs "this module imports from that module" vs "this function has test coverage." Default: show `CALLS` + `IMPORTS_FROM`. Hide `CONTAINS` (noise).

### P4 — Focus Beats Overview
A 926-node graph is too dense to understand globally. The most valuable interaction is **focus**: click a node, see only its neighborhood. The global view is for orientation; the focused view is for work.

### P5 — Dark Industrial Aesthetic
Consistent with AutoDev's overall design language: `#0A0A0A` background, muted primary text, AIRS green (`#00FF88`) for active/selected states. The graph should feel like a live systems diagram, not a bubble chart.

### P6 — No Hairball Tolerance
If the simulation produces a hairball (all nodes in one central mass), the layout has failed. Use initial cluster-seeded positioning, strong repulsion, and collision detection to prevent it.

---

## 4. Rendering Architecture & Level-of-Detail System

### 4.1 Auto Mode Thresholds

The render mode is determined by `nodeCount` after filtering:

| Node Count | Default Mode | Rationale |
|---|---|---|
| 0–300 | **Full Mode** | All nodes visible, force sim at full resolution |
| 301–1500 | **File Mode** | Aggregate individual functions into their parent file nodes |
| 1501–5000 | **Community Mode** | Aggregate all nodes into their community supernode |
| 5000+ | **Community Mode** with message | Community mode required; build a full view on request with warning |

User can override at any time via the Render Mode dropdown. The override persists until repo changes.

### 4.2 Progressive Rendering

For large graphs, the canvas renders progressively to avoid blocking the main thread:

1. **Frame 1**: Community hulls only (colored blobs) — user immediately sees structure
2. **Frame 2–5**: Nodes added in batches of 200, starting with god nodes and high-degree nodes first
3. **Frame 6+**: Low-degree nodes, edges added in batches

This ensures the user sees a useful partial picture within 100ms even for 5000-node graphs.

### 4.3 Canvas Technology

Render in **SVG for ≤ 1000 nodes**, switch to **Canvas 2D API for > 1000 nodes**. SVG is needed for DOM events (hover, click, tooltips). Canvas is needed for performance at scale.

For the Canvas path:
- Use a lightweight hitbox layer (invisible SVG overlay with transparent rects per node) to preserve click/hover events
- Or: quadtree-based hit detection from D3

---

## 5. Force Simulation Parameters

These are the corrected force parameters. The current implementation uses defaults that are wildly insufficient for 900+ nodes.

### 5.1 Force Components

```javascript
const simulation = d3.forceSimulation(nodes)
  // Repulsion: much stronger than default
  .force("charge", d3.forceManyBody()
    .strength(d => -150 - (d.degree * 8))  // god nodes repel harder
    .distanceMax(400)                        // don't repel across entire canvas
    .theta(0.9)                              // Barnes-Hut approximation
  )
  
  // Link distance scaled by edge weight
  .force("link", d3.forceLink(edges)
    .id(d => d.id)
    .distance(d => {
      if (d.kind === "CONTAINS") return 20;  // keep file→child close
      if (d.kind === "CALLS") return 60;
      if (d.kind === "IMPORTS_FROM") return 80;
      return 70;
    })
    .strength(d => {
      if (d.kind === "CONTAINS") return 0.9;
      return 0.3;
    })
  )
  
  // Cluster force: pull nodes toward their community centroid
  .force("cluster", forceCluster()
    .strength(0.15)       // gentle pull, overrides repulsion only weakly
    .centers(communitycentroids)
  )
  
  // Collision: prevents node overlap
  .force("collision", d3.forceCollide()
    .radius(d => nodeRadius(d) + 3)   // +3px padding
    .strength(0.9)
    .iterations(3)
  )
  
  // Centering: keeps graph on screen
  .force("center", d3.forceCenter(width / 2, height / 2).strength(0.05))
  
  .alphaDecay(0.02)       // slower cooling → better layout
  .velocityDecay(0.4);
```

### 5.2 Cluster Force Implementation

The custom `forceCluster()` is not in D3's standard library. It must be implemented:

```javascript
function forceCluster() {
  let strength = 0.1;
  let centers = {};  // { communityId: { x, y } }
  
  function force(alpha) {
    nodes.forEach(d => {
      const center = centers[d.community];
      if (!center) return;
      d.vx -= (d.x - center.x) * strength * alpha;
      d.vy -= (d.y - center.y) * strength * alpha;
    });
  }
  
  force.strength = s => { strength = s; return force; };
  force.centers = c => { centers = c; return force; };
  return force;
}
```

### 5.3 Initial Node Positioning

Do NOT place all nodes at the canvas center (D3's default). Instead, seed nodes at their community centroid with small random jitter:

```javascript
// Assign community centroids evenly on a circle
const R = Math.min(width, height) * 0.35;  // 35% of canvas size
const communityIds = [...new Set(nodes.map(n => n.community))];

const centroids = {};
communityIds.forEach((id, i) => {
  const angle = (2 * Math.PI * i) / communityIds.length;
  centroids[id] = {
    x: width / 2 + R * Math.cos(angle),
    y: height / 2 + R * Math.sin(angle)
  };
});

// Pre-position nodes
nodes.forEach(node => {
  const c = centroids[node.community] || { x: width / 2, y: height / 2 };
  node.x = c.x + (Math.random() - 0.5) * 80;
  node.y = c.y + (Math.random() - 0.5) * 80;
});
```

This produces a circular arrangement of communities immediately on load, before the simulation runs.

### 5.4 Alpha and Cooling

- `alpha` starts at `1.0`, decays to `alphaMin = 0.001`
- `alphaDecay = 0.02` (slower than default `0.0228`) — gives simulation more time to settle
- Simulation runs for ~500 ticks for an initial layout, then pauses
- Re-activated on node addition or `[Update]` trigger

---

## 6. Community Hull System

This is the single most impactful visual improvement. Community hulls make cluster structure visible at a glance.

### 6.1 Hull Rendering Algorithm

```javascript
// For each community, compute the convex hull of member node positions
// Expand hull outward by hullPadding pixels
// Render as smooth closed curve (Catmull-Rom spline through hull points)

function drawCommunityHulls(svg, nodes, communities) {
  const hullPadding = 28;  // px expansion beyond outermost node
  
  communities.forEach(community => {
    const memberNodes = nodes.filter(n => n.community === community.id);
    if (memberNodes.length < 3) return;  // need 3 points for hull
    
    const points = memberNodes.map(n => [n.x, n.y]);
    const hull = d3.polygonHull(points);
    if (!hull) return;
    
    // Expand hull outward from centroid
    const centroid = d3.polygonCentroid(hull);
    const expandedHull = hull.map(([x, y]) => {
      const dx = x - centroid[0];
      const dy = y - centroid[1];
      const len = Math.sqrt(dx*dx + dy*dy);
      return [x + (dx / len) * hullPadding, y + (dy / len) * hullPadding];
    });
    
    // Render as smooth curve (line generator with Catmull-Rom)
    const lineGen = d3.line()
      .x(d => d[0])
      .y(d => d[1])
      .curve(d3.curveCatmullRomClosed.alpha(0.5));
    
    svg.select(".hulls").append("path")
      .attr("d", lineGen(expandedHull))
      .attr("class", `hull hull-${community.id}`)
      .attr("fill", communityColor(community.id))
      .attr("fill-opacity", 0.06)       // very subtle fill
      .attr("stroke", communityColor(community.id))
      .attr("stroke-opacity", 0.35)
      .attr("stroke-width", 1.5)
      .attr("stroke-dasharray", "4 3"); // dashed border
  });
}
```

### 6.2 Hull Layers in SVG Stack

Hulls must render **below** nodes and edges:

```
SVG layers (bottom to top):
  1. <g class="hulls">      — community hull polygons (rendered first)
  2. <g class="edges">      — edge lines
  3. <g class="nodes">      — node circles/shapes
  4. <g class="labels">     — text labels (rendered last, on top)
```

### 6.3 Hull Update Frequency

During simulation, hulls must update every tick alongside node positions:

```javascript
simulation.on("tick", () => {
  updateNodePositions();
  updateEdgePositions();
  updateHulls();        // recalculate hull from current node positions
});
```

This is computationally expensive for 900 nodes. Optimization: only update hulls every 3rd tick during the hot phase, every 1st tick once alpha < 0.1.

### 6.4 Hull Overlap Visualization

When two community hulls overlap significantly, it signals architectural coupling between modules. AutoDev highlights this:

- Compute hull intersection area
- If `intersection_area / smaller_hull_area > 0.15` (15% overlap), add an `⚠` warning icon at the overlap centroid
- Tooltip: "Communities `api-health` and `db-models` are tightly coupled (34% overlap). Consider reviewing dependencies."

This directly surfaces the "Big Ball of Mud" anti-pattern that the Emerge tool identifies.

---

## 7. Node Visual Encoding

### 7.1 Node Radius (Degree-Based Sizing)

```javascript
function nodeRadius(node) {
  const base = 4;
  const scale = Math.log(1 + node.degree) * 3.5;
  const r = base + scale;
  return Math.min(r, 22);  // cap at 22px
}
```

Resulting sizes:
| Degree | Radius | Visual label |
|---|---|---|
| 0 (isolated) | 4px | Tiny dot |
| 1–2 | 5–7px | Small |
| 3–5 | 7–10px | Medium |
| 6–10 | 10–14px | Prominent |
| 11–20 | 14–18px | Large |
| 21+ | 18–22px | God node |

### 7.2 Node Shape by Kind

Each node kind gets a distinct D3 symbol:

| Kind | Symbol | D3 Path | Visual Description |
|---|---|---|---|
| `Class` | `d3.symbolSquare` | Square, rotated 45° (diamond) | Diamond — represents a defined structure |
| `Function` | `d3.symbolCircle` | Circle | Round — represents a callable unit |
| `Test` | `d3.symbolTriangle` | Upward triangle | Triangle — tests "point at" what they cover |
| `Type` | `d3.symbolDiamond` | Narrow diamond | Narrow diamond — type aliases |
| `File` | `d3.symbolSquare` | Square (not rotated) | Square — represents a container |
| `Module` | `d3.symbolHexagon` | Hexagon | Hexagon — represents a package boundary |
| `Import` | `d3.symbolCross` | Plus/cross | Cross — external dependency |

**Implementation**: Use D3's `d3.symbol()` generator:
```javascript
const symbolGen = d3.symbol()
  .type(d => symbolForKind(d.kind))
  .size(d => Math.PI * nodeRadius(d) ** 2);  // size in sq px, not radius
```

### 7.3 Node Color by Community

The base fill color is determined by the node's community ID:

```javascript
const COMMUNITY_COLORS = [
  "#4E79A7",  // Tableau blue
  "#F28E2B",  // Tableau orange
  "#59A14F",  // Tableau green
  "#E15759",  // Tableau red
  "#76B7B2",  // Tableau teal
  "#EDC948",  // Tableau yellow
  "#B07AA1",  // Tableau purple
  "#FF9DA7",  // Tableau pink
  "#9C755F",  // Tableau brown
  "#BAB0AC",  // Tableau gray
];

function communityColor(communityId) {
  return COMMUNITY_COLORS[communityId % COMMUNITY_COLORS.length];
}
```

These are the Tableau 10 palette — designed specifically for categorical color discrimination in data visualization. They have sufficient contrast against the dark `#0A0A0A` background.

### 7.4 Node Color Modifications by State

| State | Fill Modification | Stroke |
|---|---|---|
| Default | Community color at 85% opacity | Same color at 50% opacity, 0.5px |
| Hovered | Community color at 100% opacity | `#FFFFFF` 1px |
| Selected | Community color at 100% opacity | `#FFFFFF` 2px + outer ring |
| Neighbor (of selected) | Community color at 60% opacity | Same color 0.5px |
| Dimmed (non-neighbor when selection active) | Community color at 8% opacity | Same at 5% |
| God node (degree ≥ 15) | Community color at 100% opacity | Community color outer glow ring, 4px, 20% opacity |
| Worker-locked | Add striped pattern overlay (`repeating-linear-gradient`) | `#FFAA00` 1.5px |
| Isolated (degree = 0) | Gray `#444444` at 50% | none |

### 7.5 God Node Glow Ring

```javascript
// For nodes with degree >= 15, append an outer circle as a glow ring
nodeEnter.filter(d => d.degree >= 15)
  .append("circle")
  .attr("class", "god-ring")
  .attr("r", d => nodeRadius(d) + 6)
  .attr("fill", "none")
  .attr("stroke", d => communityColor(d.community))
  .attr("stroke-width", 3)
  .attr("stroke-opacity", 0.25);
```

### 7.6 Confidence Opacity Modifier

Nodes and edges inferred by the analysis (not directly extracted from AST) should be visually de-emphasized:

```javascript
function nodeOpacity(node) {
  if (node.confidence_tier === "AMBIGUOUS") return 0.5;
  if (node.confidence_tier === "INFERRED") return 0.75;
  return 1.0;  // EXTRACTED
}
```

---

## 8. Edge Visual Encoding

### 8.1 Edge Color by Kind

```javascript
const EDGE_COLORS = {
  "CALLS":               "#00FF88",  // AIRS green — most important relationship
  "IMPORTS_FROM":        "#4477FF",  // blue — module dependency
  "INHERITS":            "#B07AA1",  // purple — class hierarchy
  "IMPLEMENTS":          "#76B7B2",  // teal — interface implementation
  "TESTED_BY":           "#F28E2B",  // orange — test coverage (important for QC)
  "DEPENDS_ON":          "#EDC948",  // yellow — module dependency
  "REFERENCES":          "#888888",  // gray — soft reference
  "CONTAINS":            "#2A2A2A",  // near-invisible — structural (hidden by default)
  "SEMANTICALLY_SIMILAR":"#9C755F",  // brown — inferred similarity
};
```

### 8.2 Edge Opacity by Confidence

```javascript
function edgeOpacity(edge) {
  const tierBase = {
    "EXTRACTED": 0.55,
    "INFERRED":  0.30,
    "AMBIGUOUS": 0.15,
  }[edge.confidence_tier] || 0.40;
  
  // Further dim if selection is active and this edge is not connected to selected node
  if (selectedNode && edge.source !== selectedNode.id && edge.target !== selectedNode.id) {
    return tierBase * 0.08;  // nearly invisible
  }
  return tierBase;
}
```

### 8.3 Edge Width by Weight

```javascript
function edgeWidth(edge) {
  return 0.5 + edge.weight * 1.5;  // 0.5px to 2.0px
}
```

### 8.4 Edge Direction (Arrows)

Directed edges get arrowhead markers. Use D3 `<defs>` marker:

```javascript
// Arrowhead per edge color (must define one marker per color)
svg.append("defs").selectAll("marker")
  .data(Object.entries(EDGE_COLORS))
  .join("marker")
    .attr("id", ([kind]) => `arrow-${kind.toLowerCase()}`)
    .attr("viewBox", "0 -4 8 8")
    .attr("refX", 12)   // offset from node edge
    .attr("markerWidth", 6)
    .attr("markerHeight", 6)
    .attr("orient", "auto")
  .append("path")
    .attr("d", "M0,-4L8,0L0,4")
    .attr("fill", ([, color]) => color)
    .attr("opacity", 0.7);
```

`CONTAINS` edges: **no arrow** (structural, not directional in a meaningful sense).  
`SEMANTICALLY_SIMILAR` edges: **double-headed arrow** (undirected).

### 8.5 Default Edge Visibility

| Edge Kind | Default Visible | Rationale |
|---|---|---|
| `CALLS` | ✅ Yes | Most important — shows execution dependencies |
| `IMPORTS_FROM` | ✅ Yes | Module dependency structure |
| `INHERITS` | ✅ Yes | Class hierarchy |
| `TESTED_BY` | ✅ Yes | Test coverage — critical for QC agent |
| `IMPLEMENTS` | ✅ Yes | Interface relationships |
| `DEPENDS_ON` | ✅ Yes | Module-level coupling |
| `REFERENCES` | ⬜ Off | Often noisy (type annotations) — opt-in |
| `CONTAINS` | ⬜ Off | Pure structural noise — opt-in only |
| `SEMANTICALLY_SIMILAR` | ⬜ Off | LLM-inferred, noisy — opt-in |

---

## 9. Label System

### 9.1 Zoom-Dependent Label Visibility

Labels only render when the **screen-space radius** of the node exceeds a threshold:

```javascript
const LABEL_THRESHOLD_PX = 8;  // node must be at least 8px on screen

function shouldShowLabel(node, transform) {
  const screenRadius = nodeRadius(node) * transform.k;  // k = zoom scale
  return screenRadius >= LABEL_THRESHOLD_PX;
}

// Update label visibility on zoom
svg.call(d3.zoom().on("zoom", (event) => {
  const { transform } = event;
  gCanvas.attr("transform", transform);
  
  labels.attr("display", d => 
    shouldShowLabel(d, transform) ? null : "none"
  );
}));
```

**God nodes exception**: God nodes (degree ≥ 15) always show their label regardless of zoom.

### 9.2 Label Content

Default: show `node.label` (short name, e.g. `process_message`, `LivingAgent`). NOT the full qualified name — that's for the inspector panel.

On high zoom (zoom scale > 2.5): show `label + "\n" + file_basename` as two-line label.

### 9.3 Label Styling

```javascript
labels.append("text")
  .attr("class", "node-label")
  .attr("dy", d => nodeRadius(d) + 10)  // below the node
  .attr("text-anchor", "middle")
  .style("font-family", "'IBM Plex Mono', monospace")
  .style("font-size", "10px")
  .style("fill", "#888888")
  .style("pointer-events", "none")   // labels don't intercept mouse events
  .text(d => d.label);
```

Selected node label: `font-size: 12px`, `fill: #F0F0F0` (brighter).

### 9.4 Label Collision Avoidance

With many visible labels, collisions produce unreadable overlapping text. Use a simple quadtree-based visibility pruning for labels at medium zoom:

```javascript
// At zoom 1.0–2.0: keep only labels for nodes with degree >= 3 AND space
// Use d3.quadtree to check if any two label bounding boxes overlap
// If overlap: hide the lower-degree node's label
```

This is a simplified version of the label pruning used in D3 network diagrams.

---

## 10. Canvas Interaction Model

### 10.1 Zoom & Pan

Standard D3 zoom. Constraints:

```javascript
const zoom = d3.zoom()
  .scaleExtent([0.05, 12])    // 5% to 1200% zoom
  .on("zoom", handleZoom);
```

Zoom to fit all nodes on first render: `fitToCanvas()` calculates bounding box of all node positions and sets initial transform.

**Zoom shortcuts** (keyboard, when canvas has focus):
- `+` / `=` → zoom in 1.25×
- `-` → zoom out 0.8×
- `0` → reset zoom to fit
- `F` → fit to selection (selected node + neighborhood)

### 10.2 Node Hover

On mouse enter a node:
1. Cursor changes to `pointer`
2. Node border brightens: stroke → `#FFFFFF`
3. Node tooltip appears (see tooltip spec below)
4. First-hop neighbor edges and nodes subtly brighten

Tooltip (appears 200ms after hover, 12px below cursor):

```
┌────────────────────────────────────┐
│  LivingAgent                       │
│  Class · Python · Community 2      │
│  src/airs/agent/living_agent.py    │
│  L42–L2425  ·  degree: 43         │
└────────────────────────────────────┘
```

Tooltip style: `background: #111111`, `border: 1px solid #222222`, `border-radius: 2px`, `IBM Plex Mono 11px`, `color: #888888`, `padding: 6px 10px`.

### 10.3 Node Click → Focus Mode

Single click on a node:
1. Canvas enters **Focus Mode**
2. Selected node: full opacity, white border, white label
3. Direct neighbors (1-hop): full opacity, community color
4. Everything else: `opacity: 0.08` (dimmed — nearly invisible)
5. Connected edges: full opacity, colored by kind
6. Non-connected edges: `opacity: 0`
7. Right panel: Node Inspector opens/updates with this node's data

Click on empty canvas space → exit Focus Mode, restore full opacity.

### 10.4 Node Double-Click → Zoom to Node

Double-click zooms the canvas to center on the clicked node at zoom scale 3.0, showing all its 1-hop neighbors clearly.

### 10.5 Right-Click Context Menu

```
┌────────────────────────────────────┐
│  Open in IDE Tab                   │
│  ─────────────────                 │
│  Focus on this node                │
│  Expand to 2-hop neighborhood      │
│  Show blast radius                 │
│  ─────────────────                 │
│  Copy qualified name               │
│  Copy file path                    │
│  ─────────────────                 │
│  Find path to...                   │
└────────────────────────────────────┘
```

"Show blast radius" → switches to Impact Mode overlay (Section 15.5).  
"Find path to..." → opens a small search input; user types a second node name; canvas draws the shortest path between them with highlighted edges.

### 10.6 Canvas Background Click

Clicking the background canvas (not a node):
- If Focus Mode active → exit Focus Mode, restore all opacities
- If path visualization active → clear path
- If Impact Mode active → exit Impact Mode

### 10.7 Drag to Reposition Nodes

Nodes are draggable. Dragging re-pins the node (`node.fx = event.x, node.fy = event.y`), warming up the simulation. Release to unpin: `node.fx = null`.

Community hulls recompute during drag.

---

## 11. Node Inspector Panel

The right-side panel. Width: `300px`. Replaces the current version with a more structured layout.

```
┌──────────────────────────────────────────────────────┐
│  NODE INSPECTOR                                       │
├──────────────────────────────────────────────────────┤
│                                                       │
│  LivingAgent                                         │
│  ┌──────────┐  ┌───────────┐  ┌──────────────────┐  │
│  │  CLASS   │  │  PYTHON   │  │  COMM-2  api-mem  │  │
│  └──────────┘  └───────────┘  └──────────────────┘  │
│                                                       │
│  src/airs/agent/living_agent.py                      │
│  Lines 42–2425  ·  degree: 43                        │
│  Confidence: EXTRACTED                               │
│  Agent: worker-backend  ·  Task #142                 │
│                                                       │
├──────────────────────────────────────────────────────┤
│  INCOMING  (18)                                       │
│  ● process_message()  CALLS  L94                     │
│  ● factory.py  IMPORTS_FROM                          │
│  ● LivingAgentTest  TESTED_BY                        │
│  ···  15 more  [Show all]                            │
├──────────────────────────────────────────────────────┤
│  OUTGOING  (25)                                       │
│  → MemoryCore  CALLS  L312                           │
│  → UnifiedToolHandler  CALLS  L201                   │
│  → litellm  IMPORTS_FROM                             │
│  ···  22 more  [Show all]                            │
├──────────────────────────────────────────────────────┤
│  [📂 Open in IDE]  [💥 Blast Radius]  [🔗 Find Path] │
└──────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Node name | `IBM Plex Mono 14px`, `color: #F0F0F0` |
| Kind badge | `10px uppercase`, community color background at 15% opacity, community color text |
| Language badge | `10px uppercase`, `background: #1A1A1A`, `color: #666666` |
| Community badge | `10px`, `background: community_color at 20%`, `color: community_color` — shows ID + name |
| File path | `IBM Plex Mono 11px`, `color: #4477FF`, clickable → opens in IDE |
| Line range | `IBM Plex Mono 11px`, `color: #666666` |
| Degree | `IBM Plex Mono 11px`, `color: #888888` |
| Confidence tier | `10px`, `EXTRACTED` green, `INFERRED` amber, `AMBIGUOUS` red |
| Agent/Task | Only shown if `agent_author` is set; `10px`, `color: #666666` |
| INCOMING section | Edge list, 3 shown, `[Show all]` expands |
| OUTGOING section | Edge list, 3 shown, `[Show all]` expands |
| Edge item | `→` or `●` prefix, target name in `color: community_color`, edge kind in `color: EDGE_COLORS[kind]`, line number |
| `[Open in IDE]` | Fires Tauri event: switches to IDE tab, opens file at line_start |
| `[Blast Radius]` | Switches canvas to Impact Mode for this node |
| `[Find Path]` | Opens inline search to pick a second node |

---

## 12. Communities Panel

Below the Node Inspector (or in a separate panel section when no node is selected).

```
COMMUNITIES  (9)
──────────────────────────────────────────
  ████  api-health       18 nodes  0.67
  ████  module-mcp        3 nodes  0.67
  ████  agent-core       47 nodes  0.51
  ████  memory-system    31 nodes  0.48
  ████  llm-providers    12 nodes  0.44
  ████  state-machines   38 nodes  0.39
  ████  base-utils       24 nodes  0.32
  ████  test-suite       19 nodes  0.28
  ████  module-c         11 nodes  0.18  ⚠
```

| Element | Spec |
|---|---|
| Color swatch | 12×8px rect, `border-radius: 1px`, community color |
| Community name | `IBM Plex Mono 11px`, `color: #888888` |
| Node count | `IBM Plex Mono 11px`, `color: #666666` |
| Cohesion score | Right-aligned, `IBM Plex Mono 11px`, `color: #00FF88` if > 0.5, `color: #FFAA00` if 0.25–0.5, `color: #FF4444` if < 0.25 |
| `⚠` warning | Appears when cohesion < 0.20. Tooltip: "Low cohesion — consider splitting" |
| Click community row | Focuses canvas on that community (zooms to its hull, dims others) |
| Community row hover | Highlights the hull polygon on the canvas |
| Show/Hide toggle | Checkbox per row — hides all nodes in that community from canvas |

---

## 13. Toolbar & Controls

```
┌───────────────────────────────────────────────────────────────────────────────────┐
│  [■ C:\AIRS Foundry\BACKUPcoreorchestrator ▾]  [⚡ Build Graph]  [↻ Update]       │
│  [📤 Export ▾]  [↩ Reset View]  [✕ Delete]                                        │
│                                                                                    │
│  Render: [Auto ▾]   Edges: [CALLS ✓][IMPORTS ✓][INHERITS ✓][TESTED ✓] [+ more]   │
│  [🔍 Search nodes...]    Confidence: [ALL ▾]   Min degree: [1___]                 │
└───────────────────────────────────────────────────────────────────────────────────┘
```

### 13.1 Repo Selector

Dropdown showing registered repos (from multi-repo registry). Shows active repo path in `IBM Plex Mono 11px`. Selecting a different repo loads that repo's graph DB.

### 13.2 Build Graph Button

`[⚡ Build Graph]` — triggers full graph build (Stage 1–7 pipeline). While running:
- Button becomes `[◌ Building... 34%]` with spinning indicator
- Progress reflects pipeline stage (Detect 10%, Extract 40%, Build 60%, Cluster 75%, Analyze 90%, Report 95%, Export 100%)
- Canvas shows a "Building..." overlay with stage progress

### 13.3 Update Button

`[↻ Update]` — triggers incremental update (changed files only). Faster than full build. While running: `[◌ Updating... 5 files]`.

### 13.4 Export Dropdown

```
Export ▾
  → graph.json
  → graph.html (standalone visualization)
  → graph.svg
  → GRAPH_REPORT.md
```

### 13.5 Render Mode Selector

Dropdown: `Auto` / `Full` / `File` / `Community` / `Focus`.

### 13.6 Edge Type Toggles

Inline checkboxes for each visible edge type. Toggling shows/hides that edge class from the canvas. `CONTAINS` is unchecked by default.

### 13.7 Search Input

`[🔍 Search nodes...]` — live FTS5 search against the graph DB. Results appear as a dropdown list; selecting a result focuses the canvas on that node and opens the inspector.

### 13.8 Confidence Filter

Dropdown: `ALL` / `EXTRACTED only` / `EXTRACTED + INFERRED` / `AMBIGUOUS only`.

Filtering to `AMBIGUOUS only` surfaces the uncertain edges that need human review — directly ties to the "Suggested Questions" analysis.

### 13.9 Min Degree Filter

Integer input: hide all nodes with degree < N. Default 1. Setting to 3 removes isolated nodes and very low-degree periphery, dramatically reducing visual noise. Setting to 5 shows only the structurally important nodes.

---

## 14. Stats Bar

The strip just below the toolbar, showing graph statistics:

```
● Nodes: 926  ·  🔗 Edges: 703  ·  🏘 Communities: 9  ·  Avg Degree: 1.52  ·  Density: 0.16%  ·  Last built: 14:32
```

| Element | Spec |
|---|---|
| Container | `height: 28px`, `background: #0D0D0D`, `border-bottom: 1px solid #1A1A1A` |
| Font | `IBM Plex Mono 11px`, `color: #555555` |
| Nodes count | `color: #00FF88` if graph was recently updated (< 5 min), else `#555555` |
| Last built | `color: #444444`, relative time ("14 minutes ago") |
| When filtered | Appends: `· showing N/926 nodes · M/703 edges` in `color: #FFAA00` to indicate filtering is active |

---

## 15. Render Mode Specifications

### 15.1 Full Mode (< 300 nodes)

All individual nodes and edges rendered. Full force simulation. Community hulls drawn behind nodes.

**Visual checklist**:
- Every node visible as its symbol shape
- Community colors applied
- God nodes with glow ring
- Hulls behind nodes with dashed outline
- CALLS edges in green, IMPORTS_FROM in blue
- Zoom-dependent labels
- Node inspector on click

### 15.2 File Mode (300–1500 nodes)

Group all function/class/type nodes into their parent **File** node. Each file node:
- Radius = `8 + Math.log(1 + contained_node_count) * 4`
- Label = filename (basename only)
- Color = community color of the majority of contained nodes
- Interior shows a stacked mini-indicator: count of functions, classes, tests

```
┌──────────────────────────────────────┐
│   living_agent.py                    │
│   ████ 12 functions  3 classes       │
└──────────────────────────────────────┘
```

Clicking a file node in File Mode → drills down to Full Mode filtered to that file's contents.

### 15.3 Community Mode (> 1500 nodes)

Aggregate all nodes into community supernodes. Each supernode:
- Large circle (radius 40–80px proportional to node count)
- Filled solid with community color at 60% opacity
- Label = community name + "(N nodes)"
- Inner text shows top 3 node names in tiny font
- Inter-community edges: thickness proportional to edge count between communities

```
 ┌─────────────────┐
 │   agent-core    │
 │   47 nodes      │
 │ LivingAgent     │
 │ MemoryCore      │
 │ TaskExecutionFSM│
 └─────────────────┘
```

Double-clicking a community supernode → drills down to Full Mode filtered to that community.

### 15.4 Focus Mode (on-demand, any count)

Activated by:
- Clicking a node in any other mode
- Right-click → "Focus on this node"
- Selecting a node from search results

State:
- Selected node at center (auto-zoomed and panned to)
- 1-hop neighbors visible at full opacity, arranged around it
- 2-hop neighbors visible at 30% opacity (optional, toggle with `[2-hop]` button)
- Everything else at 4% opacity (almost invisible)
- Hulls hidden (unnecessary in focus view)
- Labels visible for all shown nodes regardless of size

Focus Mode is the primary **working view** — when the agent needs to understand a specific function, this is the context it gets.

### 15.5 Impact Mode (blast radius overlay)

Activated from:
- Node inspector `[💥 Blast Radius]` button
- Right-click → "Show blast radius"
- MCP tool `get_impact_radius()` call (wires result back to canvas)

Visual encoding:
```
Selected (changed) node:  ██ solid white fill, red border 2px
High risk (1-2 hops):     ██ solid red fill (#FF4444) at 80%
Medium risk (3-4 hops):   ██ solid orange fill (#FFAA00) at 60%
Low risk (5+ hops):        ██ solid yellow fill (#EDC948) at 40%
Test nodes reached:         ██ solid green fill (#00FF88) at 70%
Untested affected nodes:    ██ solid red with ✗ overlay
Not affected:               ██ gray at 8% opacity
```

A side panel replaces the Node Inspector during Impact Mode:

```
BLAST RADIUS — LivingAgent
────────────────────────────────
● Changed node: LivingAgent

High risk (1–2 hops)    3 nodes
  factory.py
  registry.py
  agent_bridge.py

Medium risk (3–4 hops)  8 nodes
  ···

Test coverage
  ● 3 tests will run
  ✗ 5 affected nodes have NO test coverage

[Run affected tests]  [Create test tasks]
```

`[Run affected tests]` → fires bash tool call with `pytest` filtered to relevant test files.  
`[Create test tasks]` → fires Delegate Mode task creation for each uncovered node.

---

## 16. Color Palette

Complete color reference for the graph canvas.

### 16.1 Background & Chrome

| Token | Value | Usage |
|---|---|---|
| Canvas background | `#0A0A0A` | Main SVG/Canvas bg |
| Panel background | `#0D0D0D` | Right panel, toolbar |
| Card background | `#111111` | Tooltip, inspector cards |
| Border subtle | `#1A1A1A` | Panel separators |
| Border medium | `#222222` | Node default stroke |

### 16.2 Community Colors (Categorical)

Tableau 10 palette — optimized for distinguishability on dark backgrounds:

| ID | Hex | Name |
|---|---|---|
| 0 | `#4E79A7` | Tableau Blue |
| 1 | `#F28E2B` | Tableau Orange |
| 2 | `#59A14F` | Tableau Green |
| 3 | `#E15759` | Tableau Red |
| 4 | `#76B7B2` | Tableau Teal |
| 5 | `#EDC948` | Tableau Yellow |
| 6 | `#B07AA1` | Tableau Purple |
| 7 | `#FF9DA7` | Tableau Pink |
| 8 | `#9C755F` | Tableau Brown |
| 9 | `#BAB0AC` | Tableau Gray |

### 16.3 Semantic Colors

| Usage | Hex |
|---|---|
| AIRS accent / selected | `#00FF88` |
| CALLS edges | `#00FF88` |
| IMPORTS_FROM edges | `#4477FF` |
| INHERITS edges | `#B07AA1` |
| TESTED_BY edges | `#F28E2B` |
| Error / danger | `#FF4444` |
| Warning | `#FFAA00` |
| High risk impact | `#FF4444` |
| Medium risk impact | `#FFAA00` |
| Low risk impact | `#EDC948` |
| Covered test node | `#00FF88` |

---

## 17. Loading & Build States

### 17.1 Graph Not Built Yet (Empty State)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                               │
│                     No graph built for this repo                             │
│                                                                               │
│           ⬡                                                                  │
│                                                                               │
│              [⚡ Build Graph]                                                 │
│                                                                               │
│           Parses your codebase into a queryable node graph.                  │
│           First build may take 30–90s for large repos.                        │
│                                                                               │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 17.2 Build In Progress Overlay

Overlays the canvas (or replaces it if first build):

```
┌──────────────────────────────────────────────────────────────────────────┐
│                                                                            │
│  Building graph for core-orchestrator                                     │
│                                                                            │
│  [██████████████████████████████████░░░░░░░░░░░░]  68%                  │
│                                                                            │
│  ✓  Stage 1: Detect — 588 files found                                    │
│  ✓  Stage 2: Extract — 588 files parsed (4.2s)                           │
│  ✓  Stage 3: Build — 926 nodes, 703 edges                                │
│  ◌  Stage 4: Cluster — Running Leiden community detection...              │
│  ○  Stage 5: Analyze — god nodes, surprises, questions                   │
│  ○  Stage 6: Report — GRAPH_REPORT.md                                    │
│  ○  Stage 7: Export — graph.json, graph.html                             │
│                                                                            │
│                               [✗ Cancel]                                  │
│                                                                            │
└──────────────────────────────────────────────────────────────────────────┘
```

Matches the AutoDev startup screen aesthetic exactly: pure text, IBM Plex Mono, progress bar.

### 17.3 Incremental Update Indicator

Non-blocking. Shows as a status strip at the bottom of the canvas canvas instead of an overlay:

```
◌  Updating graph — 3 files changed · 00:01 elapsed
```

When complete:
```
✓  Graph updated · 3 files · +12 nodes · 00:02
```

Fades out after 4 seconds.

---

## 18. Tab Layout & Panel Sizing

```
┌─── Graphify Tab ──────────────────────────────────────────────────────────────────────────────────────┐
│  [Repo selector]   [⚡ Build]  [↻ Update]  [📤 Export ▾]  [↩ Reset]  [✕ Delete]                      │
│  Render: [Auto ▾]   Edges: [CALLS][IMPORTS][INHERITS][TESTED] [+ more]   [🔍 search]   [Degree ≥ 1]   │
│  ● 926 nodes · 703 edges · 9 communities · Avg Degree: 1.52 · Last built: 5 min ago                   │
├──────────────────────────────────────────────────────────────────┬────────────────────────────────────┤
│                                                                  │  NODE INSPECTOR                     │
│                                                                  │  (300px fixed, appears on select)   │
│                                                                  │                                     │
│   D3.js CANVAS  (flex: 1, takes all remaining space)            │  ─────────────────────────────      │
│                                                                  │                                     │
│   [zoom controls bottom-left]                                    │  COMMUNITIES  (9)                   │
│   [legend bottom-right]                                          │  (always visible)                   │
│                                                                  │                                     │
│                                                                  │  ─────────────────────────────      │
│                                                                  │                                     │
│                                                                  │  GRAPH REPORT                       │
│                                                                  │  (collapsible, below communities)   │
│                                                                  │                                     │
└──────────────────────────────────────────────────────────────────┴────────────────────────────────────┘
```

| Panel | Width | Behavior |
|---|---|---|
| D3 Canvas | `calc(100% - 300px)` | All remaining width, fills height |
| Right Panel | `300px`, fixed | Node Inspector (top, shown when node selected) + Communities (always) + Graph Report (collapsible) |
| Toolbar | Full width | Two rows: actions row + filter row |
| Stats bar | Full width, `28px` | One row |

**Collapsible sections in right panel**: Each section has a `▼` toggle header. Collapsing "Graph Report" when reading code gives more vertical space to Communities.

### 18.1 Floating Zoom Controls

Bottom-left of canvas area, always visible:

```
[+]
[-]
[⊡ Fit]
```

Three buttons stacked vertically. `IBM Plex Mono 14px`, `background: #111111`, `border: 1px solid #222222`, `border-radius: 2px`.

### 18.2 Legend (Edge Types)

Bottom-right of canvas area. Compact, toggleable:

```
[ⓘ Legend ▾]

━━ CALLS
━━ IMPORTS_FROM
━━ INHERITS
━━ TESTED_BY
━━ DEPENDS_ON
```

Each item shows a short colored dash in the edge color. Click any item → toggles that edge type visibility.

---

## 19. Performance Constraints

| Metric | Target | Degradation path |
|---|---|---|
| First meaningful render | < 150ms after data load | Show community hulls only if > 500 nodes |
| Full render (< 300 nodes) | < 300ms | — |
| Full render (300–1500 nodes, File Mode) | < 500ms | — |
| Community mode render (any count) | < 200ms | Always fast (few supernodes) |
| Simulation frame rate | ≥ 30 fps during interaction | Reduce tick rate if below 25 fps |
| Focus mode activation | < 50ms | Instant opacity change via CSS class swap |
| Incremental hull update (tick) | < 5ms per tick | Skip hull update every 2nd tick if > 1000 nodes |
| Node hover tooltip | < 16ms (1 frame) | Pre-compute tooltip content, show immediately |
| Search FTS5 query | < 100ms | Debounce input at 200ms |

### 19.1 SVG vs Canvas Decision

| Node Count | Renderer | Rationale |
|---|---|---|
| ≤ 800 | SVG | Full D3 DOM events, CSS transitions, simple implementation |
| 801–3000 | SVG with simplified rendering | Reduce DOM element count: merge low-degree nodes, skip text |
| 3001+ | Canvas 2D + SVG overlay | Canvas for performance, transparent SVG overlay for hit detection |

The Canvas path uses `requestAnimationFrame` for all updates and only re-renders dirty regions.

---

## 20. D3 Implementation Notes

### 20.1 Critical: Separate Hull Update from Node Update

```javascript
// WRONG — recalculating hull geometry inside the main simulation tick causes jank
simulation.on("tick", () => {
  updateAllPositions();  // includes expensive hull recalculation
});

// CORRECT — throttle hull updates
let tickCount = 0;
simulation.on("tick", () => {
  tickCount++;
  updateNodePositions();
  updateEdgePositions();
  if (tickCount % 3 === 0 || simulation.alpha() < 0.05) {
    updateHulls();  // only every 3rd tick during hot phase, every tick when cooling
  }
});
```

### 20.2 Node Enter/Update/Exit Pattern

```javascript
const nodeSelection = g.selectAll(".node")
  .data(nodes, d => d.id);  // key by stable id, NOT array index

const nodeEnter = nodeSelection.enter()
  .append("g")
  .attr("class", d => `node node-${d.kind.toLowerCase()} node-comm-${d.community}`)
  .call(drag);

nodeEnter.append("path")  // the symbol shape
  .attr("d", d => symbolGen.type(symbolForKind(d.kind)).size(nodeSizePx(d))())
  .attr("fill", d => communityColor(d.community))
  .attr("fill-opacity", d => nodeOpacity(d))
  .attr("stroke", d => communityColor(d.community))
  .attr("stroke-opacity", 0.5)
  .attr("stroke-width", 0.5);
```

### 20.3 Edge Performance: Use Line Elements Not Path

For performance at 700+ edges, use `<line>` elements not `<path>` elements. Lines are rendered by the browser's SVG engine more efficiently.

```javascript
const linkSelection = g.selectAll(".edge")
  .data(edges, d => `${d.source.id}-${d.target.id}-${d.kind}`)
  .join("line")
  .attr("class", d => `edge edge-${d.kind.toLowerCase()}`)
  .attr("stroke", d => EDGE_COLORS[d.kind] || "#555")
  .attr("stroke-opacity", d => edgeOpacity(d))
  .attr("stroke-width", d => edgeWidth(d))
  .attr("marker-end", d => d.kind !== "CONTAINS" ? `url(#arrow-${d.kind.toLowerCase()})` : null);
```

### 20.4 Zoom Transform Preservation

Always use a single `<g class="canvas-root">` wrapper inside the SVG and apply the zoom transform to it. Never transform individual elements. This preserves crisp rendering at all zoom levels.

### 20.5 React Integration

Use `useRef` for the SVG element, `useEffect` for D3 initialization, and Zustand for graph data. Never pass D3 selection objects through React state — D3 manages its own DOM, React manages component lifecycle.

```typescript
// GraphCanvas.tsx
const svgRef = useRef<SVGSVGElement>(null);
const { nodes, edges, communities } = useGraphStore();

useEffect(() => {
  if (!svgRef.current || !nodes.length) return;
  const svg = d3.select(svgRef.current);
  initializeSimulation(svg, nodes, edges, communities);
  return () => destroySimulation();  // cleanup on unmount
}, [nodes, edges, communities]);

// When zoom changes (external trigger like "zoom to node"):
useEffect(() => {
  if (!svgRef.current || !focusNodeId) return;
  zoomToNode(d3.select(svgRef.current), focusNodeId);
}, [focusNodeId]);
```

---

## 21. React Component Map

```typescript
src/
├── tabs/
│   └── Graphify.tsx              # Tab container, toolbar, stats bar, panel layout
│
├── components/graph/
│   ├── GraphCanvas.tsx           # D3 SVG/Canvas initialization, simulation, zoom
│   ├── GraphHulls.tsx            # Community hull path rendering (separate SVG layer)
│   ├── GraphNodes.tsx            # Node symbol rendering + interaction handlers
│   ├── GraphEdges.tsx            # Edge line rendering + arrow markers
│   ├── GraphLabels.tsx           # Zoom-dependent label rendering + collision pruning
│   ├── GraphLegend.tsx           # Edge type legend (bottom-right of canvas)
│   ├── GraphZoomControls.tsx     # +/- /fit buttons (bottom-left of canvas)
│   ├── NodeTooltip.tsx           # Hover tooltip (portaled to document.body)
│   ├── NodeContextMenu.tsx       # Right-click menu
│   │
│   ├── panels/
│   │   ├── NodeInspectorPanel.tsx    # Right panel — selected node details
│   │   ├── CommunitiesPanel.tsx      # Right panel — community list
│   │   ├── GraphReportPanel.tsx      # Right panel — GRAPH_REPORT.md rendered
│   │   └── ImpactRadiusPanel.tsx     # Right panel — blast radius details (Impact Mode)
│   │
│   └── toolbar/
│       ├── RepoSelector.tsx          # Dropdown for multi-repo registry
│       ├── BuildControls.tsx         # Build/Update/Export buttons
│       ├── RenderModeSelector.tsx    # Full/File/Community/Focus/Impact
│       ├── EdgeTypeToggles.tsx       # Per-edge-kind show/hide checkboxes
│       ├── NodeSearchInput.tsx       # FTS5 search input with results dropdown
│       └── FilterControls.tsx        # Confidence filter, min degree input
│
└── stores/
    └── graphStore.ts             # nodes, edges, communities, selectedNode,
                                  # renderMode, edgeFilters, buildStatus,
                                  # focusNodeId, impactRadiusData
```

---

*This document defines the target visual quality for AutoDev Phase 6.14. All D3 implementation must satisfy the performance constraints in Section 19 and the visual encoding spec in Sections 7–9 before the Graphify tab is considered complete.*

*Reference the companion document `AutoDev_Graph_Engine_Spec` for the backend pipeline that produces the data this visualization consumes.*