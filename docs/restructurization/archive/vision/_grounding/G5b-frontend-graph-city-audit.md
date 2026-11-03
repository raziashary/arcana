# G5b — Frontend Audit: Graph / "Graph City" (BUG-CITY, BUG-GRAPH)

**Scope:** `src/tabs/Graph.tsx`, `Graph.css`, `src/components/city/**`, `src/stores/graphStore.ts`, against `src-tauri/src/commands/graph_commands.rs` and `src-tauri/src/graph/`
**Method:** read-only, traced end to end; every claim carries `path:line`
**Date:** 2026-08-31
**Status:** COMPLETE — traced mechanism of the owner's bugs #4 and #6

Legend: **CONFIRMED** = traced end-to-end in source. **SUSPECTED** = strong code-level inference requiring a runtime to be 100% certain.

---

## 1. What `Graph.tsx` renders

**Mount:** only from `App.tsx:110` — `{activeTab === 'graph' && <ErrorBoundary label="Graph"><GraphTab /></ErrorBoundary>}`. Conditional render ⇒ **the tab fully unmounts on every tab switch** and re-runs its whole lifecycle on return.

**Structure:** toolbar (`Graph.tsx:787-899`) with Build / Update / Export▾ / Fit / Delete, a **view-mode pill** (`:812-824`, Graph ⬡ vs City 🏙), render-mode select (`:830-840`), 9 edge-kind toggles, node search, min-degree input. Stats bar `:902-929`; aggregation banner `:932-942`; canvas `:948-1059`; right sidebar `:1062-1291`.

Two overlaid layers inside `.graph-canvas`: the **3D city** (`:950-954`, `zIndex: 1`) and the **2D D3 SVG wrapper** (`:957-1027`, `opacity: 0` + `pointerEvents:'none'` when city is active — hidden, not unmounted, "to preserve D3 sim").

**Data-loading lifecycle:**
- `:174` — `useEffect(() => { loadWorkspaceDir() }, [loadWorkspaceDir])` (async, resolves later)
- `:176-180` — `useEffect(..., [])`: calls `loadGraphStats()` **once on mount** and registers `listen('graph:updated')`
- `:636-655` — `loadGraphStats()`: `get_graph_stats` → if `total_nodes > 0` then `get_graph_visualization_data`
- `:167-171` — mirrors local `graphData` into `useGraphStore` (the only feed for `CityView`)

---

## 2. DATA SHAPE CONTRACT — frontend TS vs Rust serde

**Serde casing: there is NO `#[serde(rename_all = ...)]` anywhere in `src-tauri/src/graph/` or `graph_commands.rs`** (verified by grep). All structs serialize as **snake_case**, and every frontend interface uses snake_case. **So the camelCase/snake_case hypothesis is NOT the bug.**

| Command | Rust | Frontend | Verdict |
|---|---|---|---|
| `get_graph_stats` | `GraphStats` `graph_commands.rs:24-31` | `Graph.tsx:60-67` | ✅ exact match |
| `get_graph_visualization_data` | `GraphVisualizationData` `:53-63` | `Graph.tsx:100-106` | ✅ match |
| ↳ nodes | `VisualizationNode` `:34-43` | `Graph.tsx:76-91` | ⚠️ M1 |
| ↳ edges | `VisualizationEdge` `:46-51` | `Graph.tsx:93-98` | ⚠️ M2 |
| ↳ communities | `Community` `cluster.rs:27-32` | `Graph.tsx:69-74` | ⚠️ M3 |
| `search_graph_nodes` | `:581-591` | typed `VisualizationNode[]` `:588` | ⚠️ M4 |
| `open_in_ide` | `(app, file: String, line: u32)` `:860` | Graph.tsx `{file, line}` ✅ / **BuildingContextMenu `{filePath}`** ❌ | **M5 — hard break** |
| `get_graph_nodes_for_viz` / `get_graph_edges_for_viz` / `get_graph_communities` / `register_repo` / `list_repos` / `merge_all_repos_cmd` | registered at `lib.rs:131-135, 382-400` | **zero frontend callers** | dead API |

### M1 — CONFIRMED: `line_start` / `line_end` are never sent
`graphStore.ts:13-14` declares them optional; `cityLayout.ts:20,76` and `buildingDimensions.ts:11` all compute building **height** and treemap **weight** from `node.line_end - node.line_start`. `VisualizationNode` has no such fields even though `store.rs:529-530` reads them from SQLite. **Result: `loc` falls back to the constant `10` for every node ⇒ every building is the same height. The city's primary visual encoding (height = complexity) is dead flat.**

### M2 — CONFIRMED: `confidence_tier` is never sent
`graphStore.ts:24` declares it; `CallsArc.tsx:91` reads `edge.confidence_tier ?? 'INFERRED'`. `VisualizationEdge` drops it (it *is* present in the unused `get_graph_edges_for_viz`, `:470`). Every CALLS arc renders at a fixed opacity 0.5 / radius 0.09; **the confidence encoding is dead.**

### M3 — CONFIRMED: community id `-1` breaks all colour lookups
`cluster.rs:409-423` merges every community with `< 3` members into a bucket with **`id = -1`** named `"miscellaneous"`. Frontend colour maps are all `ARR[id % ARR.length]`:
- `Graph.tsx:367` → `COMMUNITY_COLORS[-1] = undefined` → `.attr('fill', undefined)` (`:482`) removes the attribute → those nodes render **black on a `#0A0A0A` background** (`Graph.css:362-367`) ⇒ **invisible**.
- `:1083-1084` → `undefined + '33'` = `"undefined33"`, an invalid CSS colour.
- `:1181` community swatch → `background: undefined`.
- `cityLayout.ts:11-13` `communityColor(-1)` → `undefined` → `new THREE.Color(undefined)` in `DistrictMesh.tsx:34-35` is a silent no-op leaving the colour **white** ⇒ the miscellaneous district is a glaring white slab; `GodNodeBuilding.tsx:26,31` likewise renders white towers.
- `DistrictMinimap.tsx:38,42` → `"undefined30"` → `ctx.fillStyle` assignment silently ignored, so that district is drawn in the **previous district's colour**.

### M5 — CONFIRMED, hard break: `open_in_ide` argument mismatch in the city
`BuildingContextMenu.tsx:21` calls `invoke('open_in_ide', { filePath: node.file_path })`. The signature is `open_in_ide(app, file: String, line: u32)`. Tauri v2 maps `filePath`→`file_path`, which is not a parameter, and `line` is missing entirely ⇒ **the invoke always rejects**, swallowed into `console.error` at `:23`. **"Open in IDE Tab" from a 3D building silently does nothing, forever.** (The 2D path at `Graph.tsx:702-705` is correct.)

### M6 — CONFIRMED (minor): edge kind `IMPORTS` has no frontend entry
`extract.rs:58` emits `"IMPORTS"` (produced at `:397`), but `EDGE_COLORS` (`Graph.tsx:17-27`) and `DEFAULT_EDGE_FILTERS` (`:29-33`) only know `IMPORTS_FROM`. Since the filter test is `edgeFilters[e.kind] !== false` (`:355`), `IMPORTS` edges are always drawn, always grey, with no arrowhead marker and no legend row or toggle.

---

## 3. Is the 3D city mounted / reachable?

**Reachable — CONFIRMED.** `CityView` is imported at `Graph.tsx:8` and rendered at `:950-954` behind **`viewMode === 'city' && hasGraph`**, where `hasGraph = Boolean(graphData && graphData.nodes.length > 0)` (`:779`). The City button is `disabled={!hasGraph}` (`:822`). **So: no graph loaded ⇒ City is unreachable.** Combined with B1 below, that is the #1 reason "Graph City" appears broken.

**CONFIRMED inconsistency:** `viewMode` lives in a module-level zustand store and survives unmount, but `graphData` (local `useState`) does not. Re-entering the Graph tab after having been in city mode leaves the pill showing **City active** while `hasGraph` is false ⇒ the 2D "No graph for this repo" placeholder with the City button highlighted and disabled.

### Dead code under `src/components/city/` — CONFIRMED (grep: only self-referencing)
- `CityPostProcessing.tsx` — **never imported anywhere.** All the emissive materials (`GroundRoad.tsx:39,50`, `CallsArc.tsx:38`, `GodNodeBuilding.tsx:61,74,85`) are authored assuming bloom that is never composited. `@react-three/postprocessing` + `postprocessing` are installed purely for this dead file.
- `layout/cityWorker.ts` — never imported (see §5).
- `impact/ImpactParticles.tsx` — never imported.
- `interaction/RoadHoverTooltip.tsx` — never imported (its own header calls it a "placeholder").
- `interaction/useCameraFlyTo.ts` (+ position presets) — never imported. `CityCamera.tsx:33-51` re-implements them by teleporting the camera.
- `roads/InheritsBridge.tsx` → `EdgeRoads` — exported, never used.
- `buildings/buildingMaterials.ts` → `getMaterialForKind`, `nodeGeometryHint`, `disposeMaterials`, `SELECTED_COLOR`, `GOD_NODE_COLOR` — all unused (the material cache is never populated *or* disposed).
- `buildings/buildingDimensions.ts` → `buildingHeight`, `buildingWidth`, `buildingDepth`, `getBuildingDimensions` — unused **duplicates** of `cityLayout.ts:19-30`. Only `testCoverageColor` is used.
- `impact/ImpactOverlay.tsx` + `impact/ImpactPanel.tsx` are imported (`CityView.tsx:22-23`) but **`setImpactData` is never called anywhere in the app**. `impactData` is permanently `null` ⇒ blast-radius mode, `IMPACT_COLORS`, the epicenter strobe and the whole `ImpactPanel` are unreachable. `ImpactPanel`'s "▶ Run Tests" / "+ Create Tasks" buttons (`:140-151`) have **no `onClick` at all**.
- `WorkerLockedBuilding` is imported and rendered, but gated on `n.kind === 'worker_locked'` (`CityView.tsx:143`). Node kinds are `File|Class|Function|Type|Test|Module|Import` (`extract.rs:26-33`); `worker_locked` is a **boolean DB column**, never a kind, and it isn't carried in `VisualizationNode`. **CONFIRMED: that branch can never fire.**

---

## 4. Three.js / R3F dependencies

`package.json:14-16,34` declares `@react-three/drei ^10.7.7`, `@react-three/fiber ^9.6.1`, `@react-three/postprocessing ^3.0.4`, `three ^0.184.0`.

**Installed and version-matched — CONFIRMED** (verified each `node_modules/<pkg>/package.json`): three 0.184.0, fiber 9.6.1, drei 10.7.7, postprocessing 6.39.1 (transitive, satisfies peer range), troika-three-text/utils present. No missing dep, no version skew. React 19.2 + fiber 9 is the correct pairing. **This is not the failure.**

### B-CSP — CONFIRMED (traced), high-impact: drei `<Text>` fetches fonts from a CDN that both CSPs block

1. `node_modules/@react-three/drei/core/Text.js:33-36` — every `<Text>` calls `suspend(() => new Promise(res => preloadFont({font, characters}, res)), […])`. Neither `DistrictMesh.tsx:79-93` nor `GodNodeBuilding.tsx:105-117` passes a `font` prop.
2. `troika-three-text.esm.js:1592` — `CONFIG.defaultFontURL: null`, so troika defers to the bundled `@unicode-font-resolver/client`, whose data URL constant is `https://cdn.jsdelivr.net/gh/lojjic/unicode-font-resolver@v1.0.1/packages/data` (`:453`), fetched with `fetch()`.
3. `src-tauri/tauri.conf.json:28` — `connect-src ipc: http://ipc.localhost https://api.github.com https://releases.airs.dev`. `index.html:8` is stricter still: `connect-src ipc: http://ipc.localhost`. **`font-src https://fonts.gstatic.com` does not cover `fetch()`; that is governed by `connect-src`.**

⇒ The font fetch is refused by CSP. `preloadFont`'s callback fires only on successful render-info resolution, so **the `suspend()` promise never settles** ⇒ each `<Suspense fallback={null}>` at `CityView.tsx:205-208` and `:215-222` **suspends forever**. Because `DistrictMesh` and `GodNodeBuilding` are *inside* those boundaries, the user-visible failure is not just missing labels — **no district ground planes, no district boundary walls, and no god-node landmark towers ever render.** You get bare ground + instanced grey boxes + roads, and a console full of CSP violations.

Secondary: troika spawns its worker from a `blob:` URL. `tauri.conf.json:28` allows `worker-src 'self' blob:`, but `index.html:8` has no `worker-src`/`child-src` and `default-src 'self'` ⇒ in dev the worker is blocked and troika falls back to main-thread SDF generation — a jank source, not a break.

---

## 5. `layout/cityWorker.ts` — is there a real web worker?

**CONFIRMED: no.** `cityWorker.ts` (16 lines) defines `self.onmessage` and calls `computeCityLayout`, but:
- **Nothing constructs it.** No `new Worker(...)`, no `?worker` import, no `import.meta.url` reference anywhere in `src/`.
- `CityView.tsx:37-45` computes the layout **synchronously on the main thread** inside a `useMemo`.
- Under Vite it would need `new Worker(new URL('./cityWorker.ts', import.meta.url), {type:'module'})` to be bundled at all; as written it is a normal ES module whose top-level `self.onmessage = ...` would, if ever imported, **overwrite `window.onmessage`** on the main thread.

The layout is a dead-stop main-thread `d3-hierarchy` treemap over every node on each `graphData` identity change, blocking the UI thread.

---

## 6. Empty-graph / never-built error states

- `loadGraphStats` (`:636-655`) wraps everything in try/catch and only `console.error`s (`:651`). **No error state is surfaced** — a backend failure is visually identical to "no graph".
- `stats === null` ⇒ `:927` "No graph — click ⚡ Build to start". `hasDisplay === false` ⇒ `:961-971` placeholder card. `CityView.tsx:90-100` guards `!graphData || !layout`. All safe.
- Rust: every command validates `repo_path`; `GraphStore::open` + `create_dir_all` create the DB on demand, so querying a never-built graph returns zeros rather than panicking. `delete_graph` is `exists()`-guarded.

**No crash on empty. But there is a silent-failure hole (B1) that makes a *populated* graph look empty.**

---

## 7. Concrete broken mechanisms

### B1 — CONFIRMED, headline: the graph never auto-loads; `get_graph_stats` is always called with an empty `repoPath`
`workspaceStore.ts:25` initialises `workspaceDir: ''` and `:30-38` loads it **asynchronously**. `Graph.tsx:174` kicks that off; `:176-180` then runs on the *same* mount with **`[]` deps** and immediately calls `loadGraphStats()`, whose closure captured `workspaceDir === ''`. `:640` sends `{ repoPath: '' }`; `graph_commands.rs:324-326` maps empty→`None`→`Err("repo_path required for graph stats")`. The throw is swallowed at `:651`.

**There is no effect keyed on `workspaceDir`** — the only re-fetch paths are the `graph:updated` listener (which holds the *same stale* closure, `:178`) and the explicit Build/Update buttons (`:664`, `:681`, which use a fresh closure and therefore do work).

**User-visible:** every time you open the Graph tab it says "No graph — click ⚡ Build to start" **even when a fully built graph exists on disk**, the canvas shows the empty placeholder, and **the 🏙 City button is disabled** — Graph City is simply unreachable until you sit through a 30–90 s rebuild. Because the tab unmounts on every tab switch, **this repeats every single visit.**

### B2 — CONFIRMED: for any real repo, "Graph City" degenerates to one building per district
`graph_commands.rs:650` `NODE_RENDER_LIMIT = 2000`; `:737-795` — above it the backend returns **only community super-nodes** (`id: "community::N"`, `file_path: ""`, `kind:"module"`). `CityView.tsx:37-45` feeds those into `computeCityLayout`, which at `cityLayout.ts:66-79` builds one district per community, each containing exactly **one** child node. So the "city" for a >2000-node codebase (this repo qualifies many times over) is N coloured squares each holding a single box. Every per-node feature is inert: `godNodes`, `BuildingRaycaster` hover/click, `SelectionRoads`, `CallsArcs`, `TrafficParticles`.

### B3 — CONFIRMED: the "Switch to Full" instruction in the aggregation banner is a no-op
`:932-941` tells the user *"Large codebase (N nodes) — showing community overview. Switch to Full … to inspect individual files."* But `:199` — `if (renderMode === 'auto' || renderMode === 'full') return graphData` — Full returns the backend payload **unchanged**, and the backend already decided to aggregate. There is no re-fetch and no `max_nodes`/`force_full` parameter on `get_graph_visualization_data`. **The user can never see individual nodes on a large repo, in 2D or in the city.**

### B4 — CONFIRMED: "Zoom To", search-result zoom, and "⊡ Fit" never work
`initD3Graph` **clones** the nodes at `:345-347` and hands the clones to the simulation (`:537`). D3 mutates `x/y` on the clones only; the objects inside `displayData.nodes` keep `x === undefined` forever.
- `zoomToNode` (`:626-634`) does `if (!n || n.x === undefined) return` ⇒ **always returns early.** Kills the "Zoom To" inspector button, the search-result click, the context-menu "Focus node", and the God-Nodes panel click.
- `zoomFit` (`:613-625`) reads the same undefined `x/y`, falls back to `W/2, H/2` for all nodes ⇒ `bw = bh = 1` ⇒ a fixed transform unrelated to the actual layout. **The ⊡ Fit button does not fit the graph.**

### B5 — CONFIRMED: toggling any edge filter / clicking a community rebuilds and re-lays-out the entire graph
`initD3Graph` is a `useCallback` with deps `[displayData, edgeFilters, minDegree, selectedCommunity, ...]` (`:567`), re-invoked at `:569-572`. `initD3Graph:387` does `svg.selectAll('*').remove()` and `:537` builds a brand-new simulation with re-randomised seed positions. **Clicking a community in the sidebar — which is only supposed to dim things — blows the layout away and re-animates from scratch; all zoom/pan state is reset.**

### B6 — CONFIRMED: selecting a node in "Community" render mode blanks the canvas and opens no inspector
`displayData` in community mode synthesises `community::N` super-nodes (`:211-218`), but every consumer of the selection reads the **raw** `graphData`: `activeNode` (`:766`), `inEdges`/`outEdges` (`:767-774`), and `applyFocusMode` via `graphDataRef` (`:250-267`). Clicking a super-node sets `selectedNode = 'community::5'`, which matches nothing ⇒ no inspector, and `applyFocusMode` dims every node to `0.08` and sets **every** edge's `stroke-opacity` to `0` (`:264`).

### B7 — CONFIRMED: nodes with no community are invisible in the city (and black in 2D)
`cluster.rs:392-393` builds adjacency **only from edges**, so isolated nodes never get a community; and `incremental.rs` **never calls `run_clustering`** (grep: zero hits) so everything added by ↻ Update has `community = NULL`. Serde emits `null`, and `cityLayout.ts:56` tests `if (n.community !== undefined)` — `null !== undefined` is **true**, so those nodes are bucketed under the map key `null` and, because the hierarchy is built by iterating `communities` (`:66-72`), they are **silently dropped: no building at all**. In 2D they render black-on-black (same mechanism as M3).
**Corollary: after ↻ Update, newly-added code never appears in Graph City** until a full ⚡ Build.

### B8 — CONFIRMED: per-mousemove re-render → 200 `TubeGeometry` allocations, never disposed
`CityView.tsx:30` subscribes with a bare `useGraphStore()` (no selector) ⇒ CityView re-renders on **any** store change. `BuildingRaycaster.tsx:95-102` calls `setHoveredNode(hit)` on **every `mousemove`**. On each re-render `CallsArc.tsx:83-84` constructs fresh `THREE.Vector3` props, invalidating `CallsArc`'s `useMemo` deps ⇒ up to **200 new `THREE.TubeGeometry` objects per mouse move** (`:28`, cap at `:71`). Geometries passed via the `geometry={...}` prop are not created by R3F and are **never disposed**.
**User-visible:** as soon as ⚡ CALLS is toggled on or any building is selected, moving the mouse tanks the frame rate and steadily leaks GPU memory until the WebGL context is lost.

### B9 — CONFIRMED: O(n) box raycast per mousemove, on the main thread
`BuildingRaycaster.tsx:56-65` iterates the **entire** hitmap and allocates a `THREE.Vector3` per building, per event, for `click`, `contextmenu` **and** `mousemove` (`:107-109`). No BVH, no instanced-mesh `instanceId` raycast (despite the file header claiming one), no throttling. It also fights `MapControls` (`CityCamera.tsx:58`) during those same mousemoves.

### B10 — CONFIRMED: traffic-particle animation resets on every re-render
`TrafficParticles.tsx:152` passes a freshly-constructed `uniforms={{ uTime: { value: 0 } }}` object literal. R3F re-assigns it on each render, discarding the value `useFrame` has been advancing (`:138-142`). Given B8's per-mousemove re-render, **particles visibly snap back to t=0 whenever the pointer moves.** The `BufferGeometry` at `:126-134` is likewise never disposed.

### B11 — CONFIRMED: in city mode the 2D overlays are hidden but still mounted, and the build overlay disappears
`Graph.tsx:957` wraps the SVG **and** the zoom controls (`:988-994`), the legend (`:997-1013`), the tooltip (`:1016-1025`) **and the build-progress overlay** (`:975-985`) in a div with `opacity: 0; pointerEvents: none` when `viewMode === 'city'`. So while the city is on screen: no zoom buttons, no legend, and — worse — **clicking ⚡ Build shows no progress overlay at all**, just a `◌ 0%` button label. Meanwhile the hidden D3 force simulation keeps ticking (`:551-560`) alongside the 3D render loop (`CityView.tsx:115` `frameloop="always"`), so **both engines burn CPU simultaneously**.

### B12 — CONFIRMED: Export writes to an unknown location with no feedback
`Graph.tsx:698` passes `outputPath: undefined`; `graph_commands.rs:404` falls back to `format!("graph.{}", format)` — a **relative** path resolved against the process CWD. For a packaged Tauri app that is wherever the `.exe` was launched from (often `C:\Program Files\...` ⇒ permission denied, surfaced only as `alert(...)`). On success there is **no toast, no path shown, no file dialog**.

### B13 — CONFIRMED (minor): city selection is decoupled from the 2D inspector
`CityView`/`BuildingRaycaster` write to `graphStore.selectedNodeId` (`:81`, `CityView.tsx:82`), while `Graph.tsx:140` keeps an entirely separate local `selectedNode`. **No component outside `components/city/` ever reads `selectedNodeId`.** Clicking a building never populates the NODE INSPECTOR sidebar, and selecting a node in 2D never highlights it in the city.

### B14 — CONFIRMED (minor): `graphStore.graphData` is never cleared
`:167-171` only writes when `graphData` is truthy. `handleDeleteGraph` (`:688-694`) sets local state to `null` but leaves the store holding the deleted graph, so a re-entry into city mode could paint a city for a graph that no longer exists.

### B15 — CONFIRMED (perf): shadows on thousands of instanced boxes, always-on frameloop
`CityView.tsx:114-115` — `shadows` + `frameloop="always"`; `CityScene.tsx:12-18` — a `castShadow` directional light with a **2048×2048** shadow map and a 400×400 ortho frustum; `BuildingInstances.tsx:97-99` — every instanced mesh is `castShadow`, `receiveShadow`, and `frustumCulled={false}`. The full shadow pass re-renders every frame even when the scene is idle.

### B16 — CONFIRMED (cosmetic): `testCoverageColor` is fed a permanently empty set
`BuildingInstances.tsx:120-125` returns `new Set<string>()` with a comment admitting it is a placeholder, then `:79-81` lerps every building 15 % toward the "untested" orange-red. **Every building in the city is tinted as untested**, regardless of `TESTED_BY` edges the extractor does produce.

---

## Ranked root causes for "Graph City UI is broken"

1. **B1** — the tab never loads an existing graph (stale `workspaceDir === ''` closure) ⇒ the City button is permanently disabled on open. `Graph.tsx:176-180`, `:640`, `workspaceStore.ts:25,30-38`.
2. **B-CSP** — troika/drei fonts blocked by `tauri.conf.json:28` + `index.html:8` ⇒ every `<Suspense>` around `DistrictMesh` and `GodNodeBuilding` never resolves ⇒ **no districts, no walls, no landmark towers, no labels**. `CityView.tsx:205-222`.
3. **B2/B3** — >2000 nodes ⇒ backend-forced community aggregation with no escape hatch ⇒ the "city" is N boxes. `graph_commands.rs:650,737` + `Graph.tsx:199`.
4. **M1/M3/B7** — missing `line_start`/`line_end` (flat skyline), community `-1` (white/black/undefined colours), null communities (missing buildings).
5. **B8/B9/B10/B15** — per-mousemove full re-render, unbounded `TubeGeometry` churn, O(n) raycast, always-on shadowed frameloop ⇒ the city that *does* render is unusably slow.
6. **M5/B4/B12** — "Open in IDE" from a building, every zoom-to-node path, ⊡ Fit, and Export all silently fail.

---

## Bearing on BUG-GRAPH (bug #4: "graph not working, workers not using it for token management")

This audit covers the **frontend** half. It confirms the graph *is* built and *is* queryable (the Build button path works), so "graph not working" is substantially a **rendering and auto-load** problem, not a build problem.

The **"workers are not using it for token management"** half is a backend question and is **not answered here** — it requires proving whether any consumer in `src-tauri/src/agent/context.rs` reads the graph to scope context. That trace is outstanding (see the G2 workstream).
