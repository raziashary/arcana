# AutoDev Graph Engine — 3D City Visualization Specification
### Phase 6.14-B: CityView — Human-Facing 3D Code City for the Graphify Tab

**Document Status:** Specification v1.0  
**Author:** Razi (Programming Lead, AIRS Foundry)  
**Date:** May 2026  
**Scope:** 3D city visualization layer for the Graphify tab — human-only, zero agent exposure  
**Companion Docs:** `AutoDev_Graph_Engine_Spec`, `AutoDev_Visualization_Refinement_Spec`  
**Research Basis:** CodeCity (Wettel & Lanza 2008), JSCity, emerge, Three.js/R3F ecosystem

---

## Table of Contents

1. [Philosophy & Motivation](#1-philosophy--motivation)
2. [The Dual-Layer Architecture (Critical Design Constraint)](#2-the-dual-layer-architecture-critical-design-constraint)
3. [The City Metaphor — Mapping Code Concepts to Urban Elements](#3-the-city-metaphor--mapping-code-concepts-to-urban-elements)
4. [Academic Foundation: CodeCity Metaphor](#4-academic-foundation-codecity-metaphor)
5. [Technology Stack](#5-technology-stack)
6. [Layout Algorithm: Squarified Treemap for Districts](#6-layout-algorithm-squarified-treemap-for-districts)
7. [Building Generation (Nodes → 3D Structures)](#7-building-generation-nodes--3d-structures)
8. [Road & Connection System (Edges → City Infrastructure)](#8-road--connection-system-edges--city-infrastructure)
9. [Lighting, Materials & Visual Identity](#9-lighting-materials--visual-identity)
10. [Camera System: Google Maps-Style Navigation](#10-camera-system-google-maps-style-navigation)
11. [Level of Detail (LOD) System](#11-level-of-detail-lod-system)
12. [Interaction Model](#12-interaction-model)
13. [Impact Mode: Blast Radius as City Emergency Overlay](#13-impact-mode-blast-radius-as-city-emergency-overlay)
14. [Animated Particle Flows (Edge Traffic)](#14-animated-particle-flows-edge-traffic)
15. [Performance Architecture](#15-performance-architecture)
16. [The Dual-View Toggle: City ↔ Graph](#16-the-dual-view-toggle-city--graph)
17. [React Component Map (CityView)](#17-react-component-map-cityview)
18. [Tauri Integration & Data Pipeline](#18-tauri-integration--data-pipeline)
19. [Agent Isolation: What Agents Never See](#19-agent-isolation-what-agents-never-see)
20. [Atomic Build Tasks (Phase 6.14-B)](#20-atomic-build-tasks-phase-614-b)

---

## 1. Philosophy & Motivation

The current 2D force-directed graph (Phase 6.14) is technically sound and agent-readable, but cognitively overloaded for humans. A user looking at 930 nodes and 1,337 edges sees a colorful hairball, not a mental model of their codebase.

The insight from 17 years of software visualization research (CodeCity, 2008 → present) is this: **humans navigate cities every day**. We know instinctively that a taller building is more important, that a district on the edge of town is peripheral, that a busy road with lots of traffic is a critical artery. By mapping code structure to a city metaphor, we offload cognitive work onto spatial intuition.

**The 3D city view is exclusively a human comprehension tool.** It is visually spectacular and cognitively powerful. It is also completely opaque to the agent fleet — agents receive and query the same plain graph data they always have. The city is a rendering skin, not a data transformation.

### What the city answers at a glance

| Question | City Signal |
|---|---|
| "What is the biggest/most complex module?" | The tallest district cluster |
| "Which function is most central to the system?" | The god-node skyscraper in the center of its district |
| "Which modules are tightly coupled?" | Districts whose roads carry the most traffic (thick animated roads) |
| "Does this function have test coverage?" | Green halo around the building; orange if partial; red if none |
| "What breaks if I change this file?" | Blast radius: neighboring buildings light up red/orange |
| "Where are the untested areas?" | Dark, unlit buildings on the city's edge at night |

---

## 2. The Dual-Layer Architecture (Critical Design Constraint)

This is the single most important architectural decision in this spec. The 3D city and the agent graph are **completely separated layers**. They share a data source (the SQLite graph DB) but nothing else.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                         AUTODEV GRAPHIFY TAB                                 │
│                                                                               │
│  ┌──────────────────────────────┐   ┌────────────────────────────────────┐  │
│  │    2D GRAPH VIEW (existing)  │   │    3D CITY VIEW (new, this spec)   │  │
│  │    D3.js Force-Directed      │   │    React Three Fiber + Three.js    │  │
│  │    Nodes, edges, hulls       │   │    Buildings, roads, districts     │  │
│  └──────────────────────────────┘   └────────────────────────────────────┘  │
│                           [City / Graph toggle]                               │
│                                                                               │
│                    Both read from the same data:                              │
│              ┌───────────────────────────────────────┐                       │
│              │  graphStore.ts (Zustand)               │                       │
│              │  { nodes, edges, communities, ... }    │                       │
│              └───────────────────────────────────────┘                       │
│                                │                                              │
│              ┌─────────────────▼─────────────────────┐                       │
│              │  Tauri Command API                     │                       │
│              │  invoke('get_graph_data', { repo })    │                       │
│              └─────────────────┬─────────────────────┘                       │
│                                │                                              │
│              ┌─────────────────▼─────────────────────┐                       │
│              │  graph.db (SQLite)                     │                       │
│              │  nodes, edges, communities             │                       │
│              └───────────────────────────────────────┘                       │
└──────────────────────────────────────────────────────────────────────────────┘
                                │
                                │  MCP stdio transport (agents only)
                                │
          ┌─────────────────────▼──────────────────────────────────┐
          │  MCP GRAPH SERVER                                        │
          │  get_minimal_context(), query_graph(), get_impact_radius()│
          │  Returns: plain text nodes + edges                       │
          │  NEVER: 3D coordinates, building heights, road data      │
          └────────────────────────────────────────────────────────┘
```

**The key invariant:** `CityView.tsx` and all of its child components exist under `src/components/city/`. They import from `graphStore.ts` for data. They export **nothing** to the MCP layer, the agent context builder, or any other agent-facing component.

The MCP server is a completely separate Rust module (`src-tauri/src/graph/mcp.rs`) that queries SQLite directly. It has no knowledge of the city rendering layer.

---

## 3. The City Metaphor — Mapping Code Concepts to Urban Elements

This is the core semantic mapping. Every code entity has a precise urban analogue.

### 3.1 The Full Mapping Table

| Code Concept | City Element | Visual Encoding | Rationale |
|---|---|---|---|
| **Community** | **City District** | Colored ground plane + low boundary wall | Communities are the highest-level organizational unit, like neighborhoods |
| **Module / Package** | **City Block** | Raised concrete platform within district | A module is a cluster of related files |
| **File** | **City Block Subdivision** | Darker ground tile within block | Files are the physical containers |
| **Class** | **Office Tower / Skyscraper** | Tall box building | Classes are complex structures with many floors (methods) |
| **Function / Method** | **Low-Rise Building** | Shorter box building | Functions are simpler, single-purpose structures |
| **Test Function** | **Fire Station / Test Facility** | Orange-accented building with antenna | Tests are support infrastructure |
| **Type / Interface** | **Archway / Gateway** | Flat arch structure | Types are definitional, not operational |
| **Module Import (external)** | **Airport / Port** | Structure at district boundary | External imports come "from outside the city" |
| **God Node (high degree)** | **Central Landmark Tower** | Tallest building in district, glowing beacon at top | The most connected nodes are architectural landmarks |
| **CALLS edge** | **Underground Metro Line** | Glowing tube below street level (visible when hovering) | Function calls are the rapid, direct connections — like subway lines |
| **IMPORTS_FROM edge** | **Main Road (Boulevard)** | Flat, wide road at street level | Module imports are the primary supply routes |
| **INHERITS edge** | **Elevated Bridge** | Arched bridge between parent/child buildings | Inheritance is an elevated structural relationship |
| **TESTED_BY edge** | **Service Road to Fire Station** | Narrow road connecting function to its test | Tests protect specific buildings |
| **CONTAINS edge** | **Building Footprint** | No road — the building's position within block implies containment | Containment is spatial, not a road |
| **DEPENDS_ON edge** | **Supply Route** | Medium-width paved road | Dependencies flow like supply trucks |
| **SEMANTICALLY_SIMILAR** | **Pedestrian Walkway** | Dotted path between buildings | Informal connections, not hard infrastructure |
| **Worker-locked node** | **Building Under Construction** | Scaffolding overlay, yellow warning lights | Active work = construction zone |
| **Isolated node (degree 0)** | **Abandoned Building** | Dark, no lights, faded texture | Disconnected code is "abandoned" |
| **AMBIGUOUS edge** | **Unpaved Dirt Road** | Brown, rough texture, dashed | Uncertain relationships are informal dirt roads |
| **INFERRED edge** | **Cobblestone Road** | Gray cobblestone texture | Inferred relationships are older, less certain infrastructure |
| **EXTRACTED edge** | **Paved Asphalt Road** | Clean black asphalt, crisp lanes | Explicitly parsed relationships are reliable paved roads |

### 3.2 Building Height Encoding (the most important dimension)

Building height encodes **complexity** — a direct, intuitive spatial signal:

```
Height = base_height + (lines_of_code / 50) + (degree × 2)

Where:
  base_height = 0.5 units  (minimum — every building has some presence)
  lines_of_code contribution = capped at 20 units
  degree contribution = capped at 15 units

Maximum height: ~35 units (god node skyscrapers)
Minimum height: 0.5 units (empty / stub functions)
```

### 3.3 Building Footprint Encoding (width × depth)

Building footprint encodes **coupling surface** — how many connections this node has relative to its size:

```
Width  = 0.8 + (incoming_edge_count × 0.15)  (capped at 4.0)
Depth  = 0.8 + (outgoing_edge_count × 0.15)  (capped at 4.0)
```

A wide building = many people import it (broad API surface).  
A deep building = it calls many other things (many dependencies).  
A building that is both wide and deep = a potential god node.

---

## 4. Academic Foundation: CodeCity Metaphor

The CodeCity metaphor was formalized by **Richard Wettel and Michele Lanza** (University of Lugano, 2008) and is the basis for this implementation. Their key finding after empirical evaluation with industry practitioners: *consistent spatial locality and solid orientation points allow engineers to navigate and understand large software systems far more effectively than 2D graphs.*

### Core principles inherited from CodeCity:

1. **Classes → Buildings, Packages → Districts.** The spatial nesting mirrors the code nesting. Districts within districts reflect package nesting depth.

2. **Metrics → Building dimensions.** Height = LOC or complexity; Width/Depth = coupling. This makes outliers (god classes, bloated files) immediately visible as skyscrapers.

3. **Containment layout.** A rectangle-packing algorithm (squarified treemap) places buildings within their district block. The city grows organically from the structural hierarchy.

4. **Consistent locality.** Once the user learns where `module-agent` is in the city, it stays there across sessions. Layout is deterministic from the data. Users build a spatial memory of their codebase.

**AutoDev extensions beyond CodeCity:**

- **Roads as edges** (CodeCity left relationships implicit in layout proximity; AutoDev makes them explicit as roads/bridges/tunnels)
- **Animated particle flows** on roads showing traffic volume and direction
- **Impact Mode** emergency overlay (blast radius as city emergency)
- **Worker-locked construction zones**
- **Test coverage as building lighting** (dark buildings = untested)
- **Community names** as district signage (not just numeric IDs)
- **JSCity-style inner function nesting** for deeply nested code

---

## 5. Technology Stack

### 5.1 Core Libraries

| Library | Version | Role |
|---|---|---|
| `three` | `^0.165.0` | 3D rendering engine — all geometry, materials, lighting |
| `@react-three/fiber` (R3F) | `^8.16.0` | React renderer for Three.js — declarative 3D components |
| `@react-three/drei` | `^9.108.0` | Three.js helper library — controls, text, instances, LOD, postprocessing |
| `@react-three/postprocessing` | `^2.16.0` | Post-processing effects — bloom, ambient occlusion |
| `zustand` | existing | Shared graph state store |
| `d3-hierarchy` | `^3.1.2` | Squarified treemap layout algorithm for districts |

### 5.2 Why React Three Fiber (R3F) over Raw Three.js

R3F wraps Three.js in a React component tree, making the entire Three.js API available as JSX. This means:
- `graphStore` state changes flow naturally into 3D scene updates via React reconciler
- The city components (`<District>`, `<Building>`, `<Road>`) are composable React components
- Hooks like `useFrame()` (per-animation-frame), `useThree()` (camera access), `useInteract()` work with React's state model
- **Critical**: R3F does **not** re-render the scene on every React state change — it uses its own reconciler that only updates dirty Three.js objects

### 5.3 Key Drei Utilities Used

| Utility | Usage |
|---|---|
| `<MapControls>` | Google Maps-style camera (orbit around center, drag pan) |
| `<Text>` | Building labels, district signs, street signs |
| `<Instance>` / `<Instances>` | Batch-render hundreds of buildings in single draw call |
| `<Detailed>` | LOD component — swap high/low poly building based on camera distance |
| `<Outlines>` | Selection highlight (white outline on selected building) |
| `<Billboard>` | Labels that always face camera |
| `useGLTF` | Load optional decorative assets (trees, streetlights) |

---

## 6. Layout Algorithm: Squarified Treemap for Districts

The city must be laid out deterministically (same data = same city every time) and must minimize wasted space. The **squarified treemap** algorithm achieves this — it is used in tools like TreeSize Pro and Voronoi maps to pack rectangles with near-square aspect ratios.

### 6.1 Hierarchy to Layout Mapping

```
Graph hierarchy → Layout hierarchy:

Level 0: Entire codebase → City footprint (e.g. 200 × 200 units)
Level 1: Communities → Districts (squarified treemap tiles)
Level 2: Files within community → Building plots within district
Level 3: Functions within file → Sub-buildings (at high zoom only)
```

### 6.2 Algorithm (d3-hierarchy + Squarify)

```typescript
// cityLayout.ts

import * as d3 from 'd3-hierarchy';

export function computeCityLayout(
  communities: Community[],
  nodes: GraphNode[],
  citySize = 200
): CityLayout {

  // 1. Build d3 hierarchy: root → communities → files → functions
  const hierarchyData = {
    id: 'root',
    children: communities.map(c => ({
      id: `community-${c.id}`,
      community: c,
      value: c.member_count,   // area proportional to community size
      children: getFilesInCommunity(nodes, c.id).map(f => ({
        id: `file-${f.file_path}`,
        file: f,
        value: getNodesInFile(nodes, f.file_path).length,
        children: getNodesInFile(nodes, f.file_path).map(n => ({
          id: n.qualified_name,
          node: n,
          value: Math.max(1, Math.log(1 + n.line_count ?? 10))
        }))
      }))
    }))
  };

  // 2. Apply squarify treemap layout
  const root = d3.hierarchy(hierarchyData)
    .sum(d => d.value)
    .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));

  d3.treemap()
    .size([citySize, citySize])
    .paddingOuter(3)      // gap between districts (roads)
    .paddingInner(1.5)    // gap within district (streets)
    .paddingTop(4)        // space for district nameplate
    .tile(d3.treemapSquarify)
    (root);

  // 3. Convert d3 layout coordinates to 3D positions
  return {
    districts: root.children.map(d => ({
      communityId: d.data.community.id,
      x: d.x0, z: d.y0,              // d3 uses x/y; we map y → z
      width: d.x1 - d.x0,
      depth: d.y1 - d.y0,
      color: COMMUNITY_COLORS[d.data.community.id % 10],
      buildings: d.children?.map(f => buildingFromFile(f)) ?? []
    }))
  };
}
```

### 6.3 Gap as Road Space

The `paddingOuter(3)` between districts becomes **boulevard space** — the algorithm does not draw roads, but reserves the gaps for the road-generation pass. Road generation (Section 8) traces paths through these gaps.

### 6.4 Determinism

The layout is seeded by `communityId` sort order. Given identical SQLite data, the layout is identical every session. This enables **spatial memory** — users can learn "the agent module is upper-left."

---

## 7. Building Generation (Nodes → 3D Structures)

### 7.1 Building Geometry

All buildings use `BoxGeometry`. The three dimensions encode three different metrics:

```typescript
// buildingGeometry.ts

function buildingDimensions(node: GraphNode): [width: number, height: number, depth: number] {
  const loc = node.line_end ? (node.line_end - (node.line_start ?? 0)) : 10;
  const inDegree = countIncomingEdges(node.qualified_name);
  const outDegree = countOutgoingEdges(node.qualified_name);

  const height = clamp(0.5 + loc / 50 + outDegree * 0.8, 0.5, 35);
  const width  = clamp(0.8 + inDegree * 0.15, 0.8, 4.0);
  const depth  = clamp(0.8 + outDegree * 0.15, 0.8, 4.0);

  return [width, height, depth];
}
```

### 7.2 Building Material System

Each building gets a `MeshStandardMaterial` customized by node kind and state:

```typescript
// buildingMaterials.ts

const BASE_MATERIALS: Record<NodeKind, THREE.MeshStandardMaterial> = {
  Class:    new THREE.MeshStandardMaterial({ color: '#1A2744', metalness: 0.6, roughness: 0.4 }),
  Function: new THREE.MeshStandardMaterial({ color: '#0F1F1F', metalness: 0.2, roughness: 0.7 }),
  Test:     new THREE.MeshStandardMaterial({ color: '#1A1208', metalness: 0.1, roughness: 0.8 }),
  Type:     new THREE.MeshStandardMaterial({ color: '#1A0F2E', metalness: 0.4, roughness: 0.5 }),
  File:     new THREE.MeshStandardMaterial({ color: '#111111', metalness: 0.0, roughness: 0.9 }),
  Module:   new THREE.MeshStandardMaterial({ color: '#0A1A0A', metalness: 0.1, roughness: 0.8 }),
};
```

### 7.3 Test Coverage Lighting (Window Illumination)

Buildings emit light through "windows" using emissive color. The emissive intensity encodes test coverage:

```
Test coverage state     → Emissive color     → Visual
────────────────────────────────────────────────────────
Fully covered           → #00FF88 (AIRS green)  → Green-lit windows
Partially covered       → #F28E2B (orange)       → Amber-lit windows
No tests at all         → #000000 (black)         → Dark, unlit building
Worker-locked           → #FFAA00 (yellow)        → Yellow construction lights
God node                → #FFFFFF + bloom         → Brilliant white beacon at top
Isolated (degree 0)     → #1A0000 (dark red)      → Faint eerie red glow
```

Window lights are implemented as a custom shader that procedurally places small emissive rectangles on building faces based on a hash of the node ID — so the same building always has the same window pattern.

### 7.4 God Node Landmark Treatment

God nodes (degree ≥ 15) get special architectural treatment:

```typescript
// GodNodeBuilding.tsx

function GodNodeBuilding({ node, position, dimensions }) {
  const [w, h, d] = dimensions;

  return (
    <group position={position}>
      {/* Main tower — tapered at top (truncated pyramid) */}
      <mesh>
        <cylinderGeometry args={[w * 0.3, w * 0.5, h, 8]} />   {/* octagon cross-section */}
        <meshStandardMaterial color={communityColor(node.community)} metalness={0.8} roughness={0.2} />
      </mesh>

      {/* Beacon at top — animated bloom sphere */}
      <mesh position={[0, h / 2 + 0.5, 0]}>
        <sphereGeometry args={[0.3, 16, 16]} />
        <meshStandardMaterial
          emissive={communityColor(node.community)}
          emissiveIntensity={3.0}
        />
      </mesh>

      {/* Point light emanating from beacon */}
      <pointLight
        position={[0, h / 2 + 0.5, 0]}
        color={communityColor(node.community)}
        intensity={2.0}
        distance={15}
      />
    </group>
  );
}
```

### 7.5 Instanced Rendering for Performance

Regular (non-god-node) buildings of the same kind are rendered as a single `InstancedMesh` draw call:

```typescript
// BuildingInstances.tsx  — renders ALL Function nodes as one draw call

function FunctionBuildingInstances({ nodes, layout }) {
  const meshRef = useRef<THREE.InstancedMesh>(null!);
  const count = nodes.filter(n => n.kind === 'Function').length;

  useEffect(() => {
    const temp = new THREE.Object3D();
    let i = 0;
    nodes.filter(n => n.kind === 'Function').forEach(node => {
      const pos = layout.getBuildingPosition(node.qualified_name);
      const [w, h, d] = buildingDimensions(node);
      temp.position.set(pos.x, h / 2, pos.z);  // y = half-height (base on ground)
      temp.scale.set(w, h, d);
      temp.updateMatrix();
      meshRef.current.setMatrixAt(i, temp.matrix);
      // Encode test coverage as instance color
      meshRef.current.setColorAt(i, testCoverageColor(node));
      i++;
    });
    meshRef.current.instanceMatrix.needsUpdate = true;
    meshRef.current.instanceColor!.needsUpdate = true;
  }, [nodes, layout]);

  return (
    <instancedMesh ref={meshRef} args={[undefined, undefined, count]}>
      <boxGeometry args={[1, 1, 1]} />
      <meshStandardMaterial metalness={0.2} roughness={0.7} />
    </instancedMesh>
  );
}
```

**Draw call budget**: With instancing, all Function buildings = 1 draw call, all Class buildings = 1 draw call, all Test buildings = 1 draw call. Total draw calls for 930 nodes: ~10 (one per node kind × material variant), vs 930 without instancing.

---

## 8. Road & Connection System (Edges → City Infrastructure)

Roads are the most visually distinctive part of the city metaphor. Different edge types produce different road classes with distinct visual treatments.

### 8.1 Road Generation Algorithm

For each edge in the graph, a road is generated in the gap space between buildings:

```typescript
// roadGenerator.ts

function generateRoad(
  edge: GraphEdge,
  sourcePos: THREE.Vector3,
  targetPos: THREE.Vector3,
  edgeKind: EdgeKind
): RoadGeometry {

  switch (edgeKind) {
    case 'IMPORTS_FROM':
      return generateGroundRoad(sourcePos, targetPos, {
        width: 0.8,
        texture: 'asphalt',     // EXTRACTED: clean asphalt
        laneCount: 2
      });

    case 'CALLS':
      return generateElevatedArc(sourcePos, targetPos, {
        arcHeight: 2.0 + distance(sourcePos, targetPos) * 0.1,
        tubeRadius: 0.12,
        color: '#00FF88',       // AIRS green
        glow: true
      });

    case 'INHERITS':
      return generateElevatedBridge(sourcePos, targetPos, {
        archHeight: 4.0,
        bridgeWidth: 0.6,
        material: 'concrete',
        color: '#B07AA1'
      });

    case 'TESTED_BY':
      return generateServiceRoad(sourcePos, targetPos, {
        width: 0.3,
        texture: 'gravel',
        color: '#F28E2B',
        dashed: false
      });

    case 'DEPENDS_ON':
      return generateGroundRoad(sourcePos, targetPos, {
        width: 0.5,
        texture: 'asphalt',
        color: '#EDC948'
      });

    case 'SEMANTICALLY_SIMILAR':
      return generatePedestrianPath(sourcePos, targetPos, {
        width: 0.2,
        texture: 'cobblestone',
        dashed: true,
        color: '#9C755F'
      });
  }
}
```

### 8.2 CALLS Edges — Elevated Glowing Tubes

`CALLS` edges are the most important architectural signal. They are rendered as elevated arcing tube geometry (not ground roads) so they are visible over the city skyline:

```typescript
// CallsEdge.tsx

function CallsEdge({ source, target, confidence }) {
  const midpoint = source.clone().add(target).multiplyScalar(0.5);
  const arcHeight = 2.0 + source.distanceTo(target) * 0.08;
  midpoint.y = arcHeight;

  const curve = new THREE.QuadraticBezierCurve3(
    source.clone().setY(source.y + 0.5),  // lift off building roof
    midpoint,
    target.clone().setY(target.y + 0.5)
  );

  const tubeRadius = confidence === 'AMBIGUOUS' ? 0.05 : 0.10;
  const opacity = { EXTRACTED: 0.85, INFERRED: 0.5, AMBIGUOUS: 0.25 }[confidence];

  return (
    <mesh>
      <tubeGeometry args={[curve, 32, tubeRadius, 6, false]} />
      <meshStandardMaterial
        color="#00FF88"
        emissive="#00FF88"
        emissiveIntensity={0.6}
        transparent
        opacity={opacity}
      />
    </mesh>
  );
}
```

**CALLS edges are hidden by default at city scale** (too many, too dense). They appear when:
1. User selects a building (shows only its CALLS connections)
2. User enables "Show CALLS" in the edge filter toggle
3. Impact Mode is active

### 8.3 IMPORTS_FROM Edges — Ground Boulevards

`IMPORTS_FROM` edges are wide ground-level roads connecting district entry points. They are the city's primary arterial system.

Instead of drawing individual roads between every pair of nodes, **IMPORTS_FROM edges are aggregated at the community level**: if community A imports heavily from community B, a wide multi-lane boulevard is drawn from district A's boundary to district B's boundary.

```typescript
// importRoadAggregator.ts

function aggregateImportRoads(edges: GraphEdge[], layout: CityLayout): AggregatRoad[] {
  // Group IMPORTS_FROM edges by community pair
  const communityPairs = new Map<string, number>();
  edges
    .filter(e => e.kind === 'IMPORTS_FROM')
    .forEach(e => {
      const srcComm = getCommunity(e.source_qualified);
      const tgtComm = getCommunity(e.target_qualified);
      if (srcComm === tgtComm) return;    // same district, internal street
      const key = `${srcComm}->${tgtComm}`;
      communityPairs.set(key, (communityPairs.get(key) ?? 0) + 1);
    });

  // Render aggregated roads: width = log(count) × 0.5
  return Array.from(communityPairs.entries()).map(([key, count]) => {
    const [srcComm, tgtComm] = key.split('->').map(Number);
    const srcBoundary = layout.districts[srcComm].getBoundaryPoint(tgtComm);
    const tgtBoundary = layout.districts[tgtComm].getBoundaryPoint(srcComm);
    return {
      from: srcBoundary,
      to: tgtBoundary,
      width: Math.log(1 + count) * 0.5,
      laneCount: Math.min(count, 6),
      color: '#4477FF',
    };
  });
}
```

### 8.4 INHERITS Edges — Elevated Bridges

Class inheritance is rendered as a literal architectural bridge: an arch spanning from parent class building to child class building, elevated above street level. The bridge is rendered as an extruded arch shape.

### 8.5 Road Confidence Encoding

| Confidence Tier | Road Surface | Visual Texture |
|---|---|---|
| `EXTRACTED` | Clean asphalt | Solid black, painted lane markings |
| `INFERRED` | Cobblestone | Gray stone pattern, slightly rough |
| `AMBIGUOUS` | Dirt path | Brown, no markings, dashed edges |

---

## 9. Lighting, Materials & Visual Identity

### 9.1 Scene Lighting Setup

```typescript
// CityScene.tsx — lighting setup

function CityLighting() {
  return (
    <>
      {/* Ambient: low-level fill so dark buildings aren't pitch black */}
      <ambientLight intensity={0.15} color="#101828" />

      {/* Primary directional: angled "sun" casting shadows */}
      <directionalLight
        position={[50, 80, 30]}
        intensity={1.2}
        color="#E8F0FF"
        castShadow
        shadow-mapSize-width={2048}
        shadow-mapSize-height={2048}
        shadow-camera-far={300}
        shadow-camera-left={-120}
        shadow-camera-right={120}
        shadow-camera-top={120}
        shadow-camera-bottom={-120}
      />

      {/* Hemisphere: sky/ground color differentiation */}
      <hemisphereLight skyColor="#1a2240" groundColor="#0a0a0a" intensity={0.5} />

      {/* Fog: atmospheric depth (city fades into darkness at distance) */}
      <fog attach="fog" args={['#050508', 80, 250]} />
    </>
  );
}
```

### 9.2 Ground Plane Material

```typescript
// The city sits on a dark ground plane
// Ground plane has a subtle grid pattern matching AutoDev's aesthetic

const groundMaterial = new THREE.MeshStandardMaterial({
  color: '#060606',
  roughness: 0.95,
  metalness: 0.05,
});

// Grid overlay (ShaderMaterial for precision)
// Thin #1A1A1A grid lines every 5 units — like a city planning map
```

### 9.3 District Ground Planes

Each community's ground plate is rendered as a slightly elevated plane (0.05 units above the base ground) with the community color at low opacity, creating a subtle colored "borough" demarcation:

```
District ground color = communityColor(id) at 8% opacity
District border = communityColor(id) at 35% opacity, 0.1 unit raised wall
District name sign = Billboard text at district center, 0.5 units high, communityColor at 60% opacity
```

### 9.4 Post-Processing Effects

Using `@react-three/postprocessing`:

```typescript
// PostProcessingEffects.tsx

function CityPostProcessing() {
  return (
    <EffectComposer>
      {/* Bloom: god node beacons and glowing CALLS tubes glow */}
      <Bloom
        intensity={0.4}
        luminanceThreshold={0.7}
        luminanceSmoothing={0.9}
        radius={0.6}
      />

      {/* SSAO: ambient occlusion — buildings cast shadow on streets */}
      <SSAO
        radius={0.4}
        intensity={30}
        luminanceInfluence={0.4}
        color="black"
      />

      {/* Vignette: subtle dark edges — classic cinematic depth */}
      <Vignette eskil={false} offset={0.1} darkness={0.4} />
    </EffectComposer>
  );
}
```

The bloom effect is critical: god node beacons and `CALLS` tube edges emit light that blooms in the scene, making architectural landmarks visible from far away.

---

## 10. Camera System: Google Maps-Style Navigation

The camera is the primary navigation tool. It must feel exactly like Google Maps 3D: drag to pan across the city, scroll to zoom in/out, right-click-drag to orbit.

### 10.1 MapControls Configuration

```typescript
// CityCamera.tsx
import { MapControls } from '@react-three/drei';

function CityCamera() {
  return (
    <MapControls
      // Zoom constraints
      minDistance={3}          // minimum: ~street level (inside a district)
      maxDistance={220}        // maximum: see entire city
      minPolarAngle={Math.PI / 12}   // 15° — can't go flat to ground
      maxPolarAngle={Math.PI / 2.2}  // ~82° — can't go past vertical
      
      // Smooth damping (Google Maps feel)
      enableDamping
      dampingFactor={0.08}
      
      // Auto-rotation disabled (user controls orientation)
      autoRotate={false}
      
      // Enable pan (drag to move across city)
      enablePan
      panSpeed={1.2}
      
      // Zoom speed
      zoomSpeed={1.0}
    />
  );
}
```

### 10.2 Camera Presets (Keyboard Shortcuts)

| Shortcut | Camera Action |
|---|---|
| `0` / `Numpad 0` | Reset: fit entire city in view, 45° angle |
| `G` | Ground level: drop to street-level perspective |
| `T` | Top-down: orthographic-like overhead view |
| `F` | Fit: zoom to selected building's district |
| `B` | Focus building: zoom to specific selected building at street level |
| `←` `→` `↑` `↓` | Pan across city at current altitude |

### 10.3 Zoom-to-Building Animation

When a building is clicked, the camera smoothly animates to a close-up:

```typescript
// useCameraFlyTo.ts
import { useThree, useFrame } from '@react-three/fiber';

function useFlyToBuilding() {
  const { camera, controls } = useThree();

  return (buildingPosition: THREE.Vector3, buildingHeight: number) => {
    const targetPos = buildingPosition.clone().add(
      new THREE.Vector3(buildingHeight * 0.8, buildingHeight * 1.2, buildingHeight * 0.8)
    );
    const lookAt = buildingPosition.clone().setY(buildingHeight / 2);

    // Animate via gsap or manual lerp over ~60 frames
    gsap.to(camera.position, {
      x: targetPos.x, y: targetPos.y, z: targetPos.z,
      duration: 1.2,
      ease: 'power2.inOut',
      onUpdate: () => controls.target.lerp(lookAt, 0.05)
    });
  };
}
```

---

## 11. Level of Detail (LOD) System

The LOD system is essential for rendering 930+ buildings performantly. The city has four detail levels based on camera distance to each building.

### 11.1 LOD Distance Thresholds

```
LOD 0 (FULL): camera distance ≤ 8 units
  - Full building geometry with all detail
  - Window shader active
  - Building label visible
  - Animated particle flows on adjacent roads
  - Service roads visible

LOD 1 (HIGH): 8 < distance ≤ 25 units  
  - Full geometry, no window shader (just solid color)
  - Building label visible
  - Roads visible, particles hidden

LOD 2 (MEDIUM): 25 < distance ≤ 60 units
  - Simplified geometry (fewer polygons, no bevels)
  - No label
  - District road network visible, individual streets hidden

LOD 3 (LOW): distance > 60 units
  - Single box with community color (instanced)
  - No label, no roads
  - Only district-level aggregate roads
```

### 11.2 Drei `<Detailed>` Implementation

```typescript
import { Detailed } from '@react-three/drei';

function Building({ node, position, dimensions }) {
  const distanceLevels = [8, 25, 60, Infinity];

  return (
    <Detailed distances={distanceLevels}>
      <FullDetailBuilding node={node} dimensions={dimensions} />
      <HighDetailBuilding node={node} dimensions={dimensions} />
      <MediumDetailBuilding node={node} dimensions={dimensions} />
      <LowDetailBuilding node={node} dimensions={dimensions} />
    </Detailed>
  );
}
```

---

## 12. Interaction Model

### 12.1 Building Click → Street-Level Inspection

Clicking a building:
1. Camera flies to building (Section 10.3)
2. Building gets white outline highlight (`<Outlines>` from Drei)
3. Adjacent buildings (connected via edges) pulse subtly
4. Right panel switches to **Node Inspector** (same data as 2D graph view)
5. Adjacent roads glow brighter
6. Unrelated buildings dim to 30% opacity

### 12.2 District Click → District Overview

Clicking a district's ground plane:
1. Camera pulls back to show full district
2. District boundary wall brightens
3. Right panel switches to **Community Panel**
4. All buildings in district highlight, others dim

### 12.3 Road Hover → Edge Info Tooltip

Hovering over a road shows:
- Edge kind (IMPORTS_FROM, CALLS, etc.)
- Source node → Target node
- Confidence tier
- Edge weight / count (for aggregate roads: total edge count)

### 12.4 Right-Click Context Menu

```
Right-click a building:
┌──────────────────────────────────┐
│  Open in IDE Tab                 │
│  Focus in 2D Graph View          │
│  Show Blast Radius               │
│  Show Incoming Roads Only        │
│  Show Outgoing Roads Only        │
│  Find Path to...                 │
│  ────────────────                │
│  Copy qualified name             │
│  Copy file path                  │
└──────────────────────────────────┘
```

### 12.5 District Hover → Community Info

Hovering over a district ground plate shows a tooltip with:
- Community name (e.g. "agent-core")
- Node count and dominant language
- Cohesion score with color indicator
- Top 3 god nodes in district

### 12.6 Minimap

A small 2D minimap (top-left corner, 120×120px) shows the full city footprint in community colors. A white rectangle represents the current camera frustum's footprint. Clicking the minimap teleports the camera.

```typescript
// Minimap is a <Canvas> rendered to a texture, displayed as an HTML overlay
// The white frustum rect is a CSS-positioned div that updates each frame
// via a Tauri event from the main canvas
```

---

## 13. Impact Mode: Blast Radius as City Emergency Overlay

When a user triggers impact analysis, the city switches to Emergency Mode — a visual language inspired by disaster movies where a city goes into lockdown.

### 13.1 Visual States in Impact Mode

```
Changed building (epicenter):
  → White pulse animation (strobe)
  → Red-orange emergency light cone from roof
  → "CHANGING" label billboard

High-risk buildings (1–2 hops):
  → Emissive red fill (#FF4444)
  → Flickering intensity (simulates power instability)
  → Alert icon (⚠) as billboard at roof level

Medium-risk buildings (3–4 hops):
  → Emissive orange fill (#FFAA00)
  → Steady illumination
  
Low-risk buildings (5+ hops):
  → Emissive yellow-green fill (#CCFF00)
  → Dim, not urgent

Test buildings that will run:
  → Green pulse (#00FF88)
  → "TESTING" label billboard

Untested affected buildings:
  → Red fill + X overlay texture
  → "NO TESTS" label billboard

All unaffected buildings:
  → Dimmed to 5% opacity ("ghost" buildings)
  → Roads invisible except impact paths
```

### 13.2 Impact Roads

The paths from changed node to affected nodes are traced through the city as glowing red particle streams:

```typescript
// Each impact path is rendered as a particle stream traveling from
// the changed building through intermediate roads to affected buildings
// Particle speed: fast (1.5× normal traffic)
// Particle color: red→orange gradient based on hop count
```

### 13.3 Impact Side Panel

The right panel in Impact Mode shows the blast radius data in "emergency dispatch" style:

```
╔═══════════════════════════════════════╗
║  ⚡ BLAST RADIUS — LivingAgent        ║
╠═══════════════════════════════════════╣
║  ❗ HIGH RISK (1–2 hops)    3 nodes  ║
║  ⚠  MEDIUM RISK (3–4 hops) 8 nodes  ║
║  ℹ  LOW RISK (5+ hops)     12 nodes ║
╠═══════════════════════════════════════╣
║  TESTS TO RUN: 3                     ║
║  ✓ test_living_agent.py              ║
║  ✓ test_factory.py                   ║
║  ✓ test_integration.py               ║
╠═══════════════════════════════════════╣
║  ⛔ NO TEST COVERAGE: 5 nodes        ║
║  → registry.py::AgentRegistry        ║
║  → factory.py::create_agent          ║
║  ... 3 more                          ║
╠═══════════════════════════════════════╣
║  [▶ Run Tests]  [+ Create Tasks]     ║
╚═══════════════════════════════════════╝
```

---

## 14. Animated Particle Flows (Edge Traffic)

Traffic particles move along roads to show the direction and volume of data/control flow. This is the most "alive" element of the city — it transforms a static map into a living system.

### 14.1 Particle System Architecture

Particles are rendered using a single `THREE.Points` object with a custom shader — **not** individual mesh instances. This allows thousands of particles at near-zero GPU cost.

```typescript
// TrafficParticles.tsx — GPU-accelerated particle flow

// Each road registers a particle emitter:
// - emitRate: proportional to edge weight / call frequency
// - path: CatmullRomCurve3 following the road geometry
// - color: EDGE_COLORS[edgeKind]

// The vertex shader:
// - Advances each particle along its path using a time uniform
// - Particles loop: when t=1.0, reset to t=0.0
// - Fade in at 0.0, fade out at 1.0

const particleVertexShader = `
  uniform float uTime;
  attribute float aPathOffset;    // which road (0-1 index into road array)
  attribute float aTimeOffset;    // stagger particle timing
  attribute vec3 aColor;

  // sample position along CatmullRom curve at time t
  vec3 getPositionOnCurve(float t, int roadIndex) {
    // ... cubic bezier evaluation using control points passed as uniforms
  }

  void main() {
    float t = mod(uTime + aTimeOffset, 1.0);
    vec3 pos = getPositionOnCurve(t, int(aPathOffset));
    
    float alpha = sin(t * 3.14159);  // fade in/out at ends
    vAlpha = alpha;
    
    gl_Position = projectionMatrix * modelViewMatrix * vec4(pos, 1.0);
    gl_PointSize = 3.0 * alpha;
  }
`;
```

### 14.2 Default Particle Visibility

| Edge Kind | Particles Active by Default | Particle Color | Speed |
|---|---|---|---|
| `CALLS` | Only on selected/focused node | `#00FF88` | Fast |
| `IMPORTS_FROM` | Yes (district roads) | `#4477FF` | Medium |
| `INHERITS` | No (too static) | `#B07AA1` | Slow |
| `TESTED_BY` | Yes | `#F28E2B` | Medium |
| `DEPENDS_ON` | Yes (district roads) | `#EDC948` | Medium |

---

## 15. Performance Architecture

The 3D city must run at ≥ 30fps on a mid-range developer machine (RTX 3060 or equivalent) with 930 nodes and 1,337 edges.

### 15.1 Draw Call Budget

| Object Category | Strategy | Draw Calls |
|---|---|---|
| Function buildings | InstancedMesh | 1 |
| Class buildings | InstancedMesh | 1 |
| Test buildings | InstancedMesh | 1 |
| Type/Interface shapes | InstancedMesh | 1 |
| God node buildings | Individual meshes | ~10 (one per god node) |
| District ground planes | Merged geometry | 1 |
| Ground plane | Single mesh | 1 |
| District walls | Merged geometry | 1 |
| Boulevard roads (IMPORTS_FROM) | Merged geometry | 1 |
| Bridge roads (INHERITS) | Merged geometry | 1 |
| Traffic particles | Single THREE.Points | 1 |
| CALLS tubes (active only) | Up to 50 visible | ~50 |
| Post-processing passes | — | 3 |
| **Total (typical)** | | **~72 draw calls** |

Target: well under 200 draw calls at all times.

### 15.2 Geometry Pre-computation

All city geometry is computed **once** when graph data loads, in a Web Worker (Tauri offscreen thread) to avoid blocking the UI thread:

```typescript
// cityWorker.ts (Web Worker)
self.onmessage = ({ data: { nodes, edges, communities } }) => {
  const layout = computeCityLayout(communities, nodes);
  const buildings = computeBuildingGeometries(nodes, layout);
  const roads = computeRoadGeometries(edges, layout);
  const particles = computeParticleStartPositions(roads);

  self.postMessage({ layout, buildings, roads, particles });
};
```

Typical computation time: < 500ms for 930 nodes.

### 15.3 Frustum Culling

R3F + Three.js perform frustum culling automatically. All instanced meshes have frustum culling enabled per-instance via bounding spheres. Buildings outside the camera's view frustum are not submitted to the GPU.

### 15.4 Shadow Map Scope

Shadows are only cast by buildings within 40 units of the camera target (progressive shadow scope). Distant buildings receive ambient occlusion from the SSAO post-processing pass, not real-time shadows.

### 15.5 State Update Isolation

The 3D city only re-renders when graph data changes (not on every React state update). The `useMemo` pattern prevents unnecessary rebuilds:

```typescript
const cityGeometry = useMemo(() => {
  if (!nodes.length) return null;
  return computeCityLayout(communities, nodes, edges);
}, [nodes, edges, communities]);   // only recompute when graph data changes
```

---

## 16. The Dual-View Toggle: City ↔ Graph

The Graphify tab has a toggle button that switches between 3D City View and 2D Graph View. The transition is animated.

### 16.1 Toggle UI

```
[🏙 City]  [⬡ Graph]      ← pill toggle, top-right of canvas area
```

Active view has AIRS green (`#00FF88`) background. Inactive is `#1A1A1A`.

### 16.2 Transition Animation

On toggle from Graph → City:
1. D3 graph canvas fades out (opacity 1.0 → 0.0 over 300ms)
2. R3F canvas fades in (opacity 0.0 → 1.0)
3. Camera resets to default city overview position
4. Buildings animate from ground level up (scale Y 0.0 → 1.0 over 600ms, staggered by district)

On toggle from City → Graph:
1. Buildings compress down (scale Y 1.0 → 0.0 over 300ms)
2. R3F canvas fades out
3. D3 canvas fades in

### 16.3 State Synchronization

The two views share a `selectedNodeId` in `graphStore`. When a building is selected in City View and the user toggles to Graph View, the graph view focuses on the same node (and vice versa).

```typescript
// graphStore.ts additions for dual-view
interface GraphStore {
  // ... existing fields ...
  viewMode: 'city' | 'graph';
  setViewMode: (mode: 'city' | 'graph') => void;
  selectedNodeId: string | null;
  setSelectedNode: (nodeId: string | null) => void;
}
```

---

## 17. React Component Map (CityView)

```
src/
├── components/city/                          # ALL city components (isolated from agent layer)
│   │
│   ├── CityView.tsx                          # Top-level R3F Canvas + scene setup
│   │                                         # Imports from graphStore only
│   │
│   ├── CityScene.tsx                         # Three.js scene: lighting, fog, ground, post-processing
│   ├── CityCamera.tsx                        # MapControls, camera presets, fly-to animation
│   ├── CityPostProcessing.tsx                # Bloom, SSAO, Vignette
│   │
│   ├── districts/
│   │   ├── DistrictMesh.tsx                  # Community ground plane + colored fill + boundary wall
│   │   ├── DistrictLabel.tsx                 # Billboard community name sign
│   │   └── DistrictMinimap.tsx               # Overhead minimap overlay (HTML Canvas)
│   │
│   ├── buildings/
│   │   ├── BuildingInstances.tsx             # InstancedMesh for Function, Class, Test, Type nodes
│   │   ├── GodNodeBuilding.tsx               # Special landmark towers for god nodes (individual mesh)
│   │   ├── WorkerLockedBuilding.tsx          # Construction scaffolding overlay
│   │   ├── buildingDimensions.ts             # Height/width/depth calc from node metrics
│   │   ├── buildingMaterials.ts              # Material library by kind + state
│   │   └── windowShader.glsl                 # Procedural window illumination (test coverage)
│   │
│   ├── roads/
│   │   ├── GroundRoad.tsx                    # IMPORTS_FROM: flat extruded plane road
│   │   ├── CallsArc.tsx                      # CALLS: elevated glowing CatmullRom tube
│   │   ├── InheritsBridge.tsx                # INHERITS: arched concrete bridge
│   │   ├── TestedByRoad.tsx                  # TESTED_BY: narrow service road
│   │   ├── roadGenerator.ts                  # Edge → road geometry dispatch
│   │   └── roadAggregator.ts                 # Aggregate IMPORTS_FROM by community pair
│   │
│   ├── particles/
│   │   ├── TrafficParticles.tsx              # GPU-accelerated particle flow along roads
│   │   ├── trafficParticles.vert.glsl        # Vertex shader: particle path animation
│   │   └── trafficParticles.frag.glsl        # Fragment shader: glow + fade
│   │
│   ├── interaction/
│   │   ├── BuildingRaycaster.tsx             # Click/hover detection (raycaster → instanced mesh)
│   │   ├── BuildingContextMenu.tsx           # Right-click context menu
│   │   ├── RoadHoverTooltip.tsx              # Road hover tooltip (HTML overlay)
│   │   └── useCameraFlyTo.ts                 # Smooth camera fly-to animation
│   │
│   ├── impact/
│   │   ├── ImpactOverlay.tsx                 # Emergency mode visual overlay
│   │   ├── ImpactParticles.tsx               # Red particle streams on impact paths
│   │   └── ImpactPanel.tsx                   # Blast radius side panel (HTML)
│   │
│   └── layout/
│       ├── cityLayout.ts                     # Squarified treemap layout computation
│       ├── cityWorker.ts                     # Web Worker for geometry pre-computation
│       └── layoutTypes.ts                    # CityLayout, DistrictLayout, BuildingLayout types
│
└── tabs/
    └── Graphify.tsx                          # Existing tab — adds [City / Graph] toggle
                                              # Mounts either <CityView> or <GraphCanvas>
```

---

## 18. Tauri Integration & Data Pipeline

```
SQLite graph.db
      │
      │  invoke('get_graph_data', { repo })
      ▼
Tauri Command (Rust) → serialize nodes/edges/communities as JSON
      │
      ▼
graphStore.ts (Zustand) — single source of truth
      │
      ├──→ GraphCanvas.tsx (D3 2D graph view) — existing
      │
      └──→ CityView.tsx (Three.js 3D city) — new
             │
             └──→ cityWorker.ts (Web Worker)
                    │  compute layout + geometry
                    ▼
                 CityScene.tsx renders pre-computed geometry
```

**The Tauri `get_graph_data` command is unchanged.** CityView consumes the same data contract as GraphCanvas. No new Tauri commands are needed for the city view (except the existing `open_in_ide` command for the "Open in IDE" interaction).

---

## 19. Agent Isolation: What Agents Never See

This section explicitly documents what is **never** transmitted to the agent fleet.

| Data | Agent Access | Rationale |
|---|---|---|
| Node qualified names | ✅ Yes | Core graph data |
| Edge kinds + confidence | ✅ Yes | Core graph data |
| Community assignments | ✅ Yes | Structural metadata |
| Node line numbers | ✅ Yes | Navigation metadata |
| **3D positions (x, y, z)** | ❌ Never | City-specific layout coordinate |
| **Building dimensions** | ❌ Never | Derived from LOC for visual encoding only |
| **Road geometry** | ❌ Never | Visual representation of edges, not data |
| **District tile layout** | ❌ Never | Visual layout, no semantic content |
| **Camera position** | ❌ Never | User navigation state |
| **Particle positions** | ❌ Never | Animation state |
| **LOD level** | ❌ Never | Render optimization state |
| **Bloom/SSAO state** | ❌ Never | Post-processing state |

The MCP server (`mcp.rs`) reads only from the `nodes`, `edges`, `communities`, `flows`, and `snapshots` tables. It has no access to and no interest in any data produced by `cityLayout.ts`, `buildingDimensions.ts`, or any other city-rendering module.

**No new MCP tools are needed or added for the city view.** The city is a pure visualization skin.

---

## 20. Atomic Build Tasks (Phase 6.14-B)

Building the 3D city view requires 22 atomic tasks. These are **independent from Phase 6.14's 2D graph tasks** and can be built in parallel after the 2D view is functional (since they share the same data store).

### 20.1 Foundation (4 tasks)

| # | Status | Task | Output |
|---|--------|------|--------|
| 6.14B.1 | ✅ DONE | Set up R3F Canvas in `CityView.tsx`: basic scene, camera, lighting, MapControls | `src/components/city/CityView.tsx` |
| 6.14B.2 | ✅ DONE | Implement `cityLayout.ts`: squarified treemap layout using d3-hierarchy, community → district → building position mapping | `src/components/city/layout/cityLayout.ts` |
| 6.14B.3 | ✅ DONE | Implement `cityWorker.ts`: Web Worker that computes full layout + building dimensions off main thread | `src/components/city/layout/cityWorker.ts` |
| 6.14B.4 | ✅ DONE | Add `viewMode: 'city' \| 'graph'` + `selectedNodeId` to `graphStore.ts`, add `[🏙 City]  [⬡ Graph]` toggle with fade animation to `Graph.tsx` | `src/stores/graphStore.ts`, `src/tabs/Graph.tsx` |

### 20.2 District Layer (2 tasks)

| # | Status | Task | Output |
|---|--------|------|--------|
| 6.14B.5 | ✅ DONE | Implement `DistrictMesh.tsx`: colored ground plane per community using treemap rect, boundary wall, name billboard | `src/components/city/districts/` |
| 6.14B.6 | ✅ DONE | Implement `DistrictMinimap.tsx`: HTML Canvas 2D minimap showing city top-down with camera frustum rect | `src/components/city/districts/` |

### 20.3 Building Layer (4 tasks)

| # | Status | Task | Output |
|---|--------|------|--------|
| 6.14B.7 | ✅ DONE | Implement `buildingDimensions.ts` + `buildingMaterials.ts`: height/width/depth from LOC+degree, materials by kind | `src/components/city/buildings/` |
| 6.14B.8 | ✅ DONE | Implement `BuildingInstances.tsx`: InstancedMesh for Function, Class, Test, Type nodes; instance color from test coverage | `src/components/city/buildings/` |
| 6.14B.9 | ✅ DONE | Implement `GodNodeBuilding.tsx`: octagonal tower + animated beacon sphere + point light for god nodes | `src/components/city/buildings/` |
| 6.14B.10 | ✅ DONE | Implement `WorkerLockedBuilding.tsx` + scaffolding overlay; window shader inlined as GLSL string in `buildingMaterials.ts` | `src/components/city/buildings/` |

### 20.4 Road Layer (3 tasks)

| # | Status | Task | Output |
|---|--------|------|--------|
| 6.14B.11 | ✅ DONE | Implement `roadAggregator.ts` + `GroundRoad.tsx`: IMPORTS_FROM edges aggregated by community pair, rendered as merged boulevard geometry | `src/components/city/roads/` |
| 6.14B.12 | ✅ DONE | Implement `CallsArc.tsx`: QuadraticBezierCurve3 + TubeGeometry for CALLS edges, glow material, visible only on selection | `src/components/city/roads/` |
| 6.14B.13 | ✅ DONE | Implement `InheritsBridge.tsx` + `TestedByRoad.tsx` (via `EdgeRoads`) + full `roadGenerator.ts` dispatch config | `src/components/city/roads/` |

### 20.5 Traffic Particles (2 tasks)

| # | Status | Task | Output |
|---|--------|------|--------|
| 6.14B.14 | ✅ DONE | GPU particle vertex + fragment shaders inlined as GLSL template literals in `TrafficParticles.tsx` (Vite-compatible, no plugin needed) | `src/components/city/particles/` |
| 6.14B.15 | ✅ DONE | Implement `TrafficParticles.tsx`: THREE.Points with custom shader, one particle system for all active roads, default visibility rules | `src/components/city/particles/` |

### 20.6 Interaction & Camera (3 tasks)

| # | Status | Task | Output |
|---|--------|------|--------|
| 6.14B.16 | ✅ DONE | Implement `BuildingRaycaster.tsx`: AABB hit detection (raycaster → bounding box per building), hover + click handlers, cursor change | `src/components/city/interaction/` |
| 6.14B.17 | ✅ DONE | Implement `useCameraFlyTo.ts` + camera keyboard shortcuts (`0`, `G`, `T`) in `CityCamera.tsx` + MapControls constraints | `src/components/city/interaction/` |
| 6.14B.18 | ✅ DONE | Implement `BuildingContextMenu.tsx` + `RoadHoverTooltip.tsx`: right-click menu (Open in IDE, Copy) + road hover HTML overlay | `src/components/city/interaction/` |

### 20.7 Impact Mode (2 tasks)

| # | Status | Task | Output |
|---|--------|------|--------|
| 6.14B.19 | ✅ DONE | Implement `ImpactOverlay.tsx`: building color state machine (epicenter/high/medium/low/test/untested/ghost) with animated strobing for epicenter | `src/components/city/impact/` |
| 6.14B.20 | ✅ DONE | Implement `ImpactParticles.tsx` + `ImpactPanel.tsx`: red GPU particle streams on impact paths + emergency side panel with Run Tests / Create Tasks | `src/components/city/impact/` |

### 20.8 Post-Processing & Polish (2 tasks)

| # | Status | Task | Output |
|---|--------|------|--------|
| 6.14B.21 | ✅ DONE | Implement `CityPostProcessing.tsx`: Bloom (god nodes, CALLS tubes), Vignette; SSAO omitted (not in postprocessing v3 API) | `src/components/city/CityPostProcessing.tsx` |
| 6.14B.22 | ✅ DONE | Performance pass: `useMemo` guards on all heavy computations; instanced meshes = 1 draw call/kind; particles = 1 draw call; TypeScript clean (0 errors) | All city components |

**Total atomic tasks: 22 — ALL COMPLETE ✅**

**Implementation date:** May 2026  
**TypeScript status:** 0 errors, 0 warnings in city components  
**Draw call budget:** ~12 draw calls at rest (1/kind instanced + districts + ground + roads + particles + post-processing)

### 20.9 Recommended Build Order

```
Phase 1 (Foundation): 6.14B.1 → 6.14B.2 → 6.14B.3 → 6.14B.4  ✅ DONE
Phase 2 (Structure):  6.14B.5, 6.14B.7, 6.14B.8 (parallel)     ✅ DONE
Phase 3 (Detail):     6.14B.6, 6.14B.9, 6.14B.10 (parallel)    ✅ DONE
Phase 4 (Roads):      6.14B.11 → 6.14B.12 → 6.14B.13           ✅ DONE
Phase 5 (Life):       6.14B.14 → 6.14B.15                       ✅ DONE
Phase 6 (Interaction):6.14B.16 → 6.14B.17 → 6.14B.18           ✅ DONE
Phase 7 (Impact):     6.14B.19 → 6.14B.20                       ✅ DONE
Phase 8 (Polish):     6.14B.21 → 6.14B.22                       ✅ DONE
```

---

## Appendix A: Key Library Install Commands

```bash
# Install core 3D libs (run from AIRS Code project root)
pnpm add three @react-three/fiber @react-three/drei @react-three/postprocessing

# Type definitions
pnpm add -D @types/three

# Layout algorithm
pnpm add d3-hierarchy
pnpm add -D @types/d3-hierarchy
```

**Note**: `three` is already available in the Tauri R3F ecosystem at `r128`. The constraint from the main AIRS Code spec applies: `THREE.OrbitControls` is unavailable at r128. Use `MapControls` from `@react-three/drei` instead (which wraps OrbitControls internally but ships its own copy).

---

## Appendix B: Performance Reference Targets

| Scenario | Target FPS | Achieved via |
|---|---|---|
| Full city, static | ≥ 60fps | Instancing, merged geometry |
| Full city, particles active | ≥ 45fps | GPU shader particles (single draw call) |
| Full city, Impact Mode | ≥ 30fps | Impact state is CSS/emissive change, not new geometry |
| Flying camera (rapid movement) | ≥ 30fps | LOD system swaps low-poly at distance |
| City + post-processing | ≥ 30fps | SSAO at half-res, Bloom at quarter-res |

---

## Appendix C: Community-to-City Name Convention

The city district labels should be meaningful, not cryptic. Apply these naming conventions for district signage:

| Community Name from DB | City District Sign |
|---|---|
| `agent-core` | **AGENT DISTRICT** |
| `api-health` | **API QUARTER** |
| `memory-system` | **MEMORY SECTOR** |
| `test-suite` | **TEST PRECINCT** |
| `module-mcp` | **MCP TERMINAL** |
| `module-c:\airs foundry\ba...` | **EXTERNAL MODULES** |
| Community ID only (no name) | **DISTRICT {ID}** |

The district sign is a `<Text>` component from Drei, rendered as a `<Billboard>` (always faces camera), positioned at the center of the district at ground level.

---

*This document defines CityView — the 3D human-facing visualization layer for AutoDev Phase 6.14-B. It is a pure rendering concern: zero agent exposure, zero MCP impact, zero changes to the existing graph data model. The 3D city consumes the same SQLite graph data as the 2D view; it simply renders it differently. Agents continue to query the graph through MCP tools exactly as before.*

*Build this after Phase 6.14 (2D graph) is functional. The shared `graphStore.ts` data contract makes them interoperable with a single toggle.*