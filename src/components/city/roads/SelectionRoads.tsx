// SelectionRoads.tsx — Typed per-node roads that appear when a building is selected.
// Renders IMPORTS_FROM, DEPENDS_ON, INHERITS, TESTED_BY, SEMANTICALLY_SIMILAR
// as their respective road visuals. CALLS is handled separately by CallsArcs.
import { useMemo } from 'react'
import * as THREE from 'three'
import type { GraphEdge, GraphNode } from '../../../stores/graphStore'
import type { CityLayout } from '../layout/layoutTypes'
import { PerNodeGroundRoad } from './GroundRoad'
import { InheritsBridge, TestedByRoad } from './InheritsBridge'

// Max roads per edge kind to keep draw calls bounded
const KIND_CAP: Record<string, number> = {
  IMPORTS_FROM:        15,
  DEPENDS_ON:          15,
  INHERITS:            10,
  TESTED_BY:           10,
  SEMANTICALLY_SIMILAR: 8,
}

// ─── DashedPath — SEMANTICALLY_SIMILAR pedestrian walkway ──────────────────

interface DashedPathProps {
  from: THREE.Vector3
  to: THREE.Vector3
}

function DashedPath({ from, to }: DashedPathProps) {
  // Pre-compute segment positions so we don't do it in JSX inline
  const segments = useMemo(() => {
    const dir = to.clone().sub(from)
    const totalLen = dir.length()
    if (totalLen < 0.5) return []

    const angle = Math.atan2(dir.z, dir.x)
    const segLen = 0.55        // filled portion of each dash
    const gapLen = 0.45        // gap between dashes
    const period = segLen + gapLen
    const count = Math.floor(totalLen / period)

    const result: Array<{ x: number; z: number; angle: number }> = []
    for (let i = 0; i < count; i++) {
      const t = (i * period + segLen / 2) / totalLen
      const pos = from.clone().lerp(to, t)
      result.push({ x: pos.x, z: pos.z, angle })
    }
    return result
  }, [from, to])

  if (segments.length === 0) return null

  return (
    <>
      {segments.map((seg, i) => (
        <mesh
          key={i}
          position={[seg.x, 0.08, seg.z]}
          rotation={[0, -seg.angle, 0]}
          frustumCulled={false}
        >
          <boxGeometry args={[0.55, 0.05, 0.38]} />
          <meshStandardMaterial
            color="#6e5a48"
            transparent
            opacity={0.40}
            roughness={0.95}
            metalness={0.0}
            depthWrite={false}
          />
        </mesh>
      ))}
    </>
  )
}

// ─── SelectionRoads ─────────────────────────────────────────────────────────

interface RoadEntry {
  edge: GraphEdge
  srcPos: THREE.Vector3
  tgtPos: THREE.Vector3
}

interface SelectionRoadsProps {
  selectedNodeId: string
  edges: GraphEdge[]
  layout: CityLayout
}

export function SelectionRoads({ selectedNodeId, edges, layout }: SelectionRoadsProps) {
  const roads = useMemo((): RoadEntry[] => {
    // Collect edges touching the selected node, excluding CALLS (handled by CallsArcs)
    const connected = edges.filter(e => {
      const srcId = typeof e.source === 'string' ? e.source : (e.source as GraphNode).id
      const tgtId = typeof e.target === 'string' ? e.target : (e.target as GraphNode).id
      return (srcId === selectedNodeId || tgtId === selectedNodeId) && e.kind !== 'CALLS'
    })

    // Bucket by kind, apply per-kind caps
    const byKind = new Map<string, GraphEdge[]>()
    connected.forEach(e => {
      if (!byKind.has(e.kind)) byKind.set(e.kind, [])
      byKind.get(e.kind)!.push(e)
    })

    const result: RoadEntry[] = []
    byKind.forEach((kindEdges, kind) => {
      const cap = KIND_CAP[kind] ?? 8
      kindEdges.slice(0, cap).forEach(edge => {
        const srcId = typeof edge.source === 'string' ? edge.source : (edge.source as GraphNode).id
        const tgtId = typeof edge.target === 'string' ? edge.target : (edge.target as GraphNode).id
        const srcBl = layout.buildingPositions.get(srcId)
        const tgtBl = layout.buildingPositions.get(tgtId)
        if (!srcBl || !tgtBl) return
        result.push({
          edge,
          // Pass ground-level XZ; each road component handles its own Y
          srcPos: new THREE.Vector3(srcBl.x, 0, srcBl.z),
          tgtPos: new THREE.Vector3(tgtBl.x, 0, tgtBl.z),
        })
      })
    })

    return result
  }, [selectedNodeId, edges, layout])

  if (roads.length === 0) return null

  return (
    <>
      {roads.map(({ edge, srcPos, tgtPos }, i) => {
        const key = `sel-${i}-${edge.kind}`
        switch (edge.kind) {
          case 'IMPORTS_FROM':
            return (
              <PerNodeGroundRoad key={key} from={srcPos} to={tgtPos} edgeKind="IMPORTS_FROM" />
            )
          case 'DEPENDS_ON':
            return (
              <PerNodeGroundRoad key={key} from={srcPos} to={tgtPos} edgeKind="DEPENDS_ON" />
            )
          case 'INHERITS':
            return (
              <InheritsBridge key={key} sourcePos={srcPos} targetPos={tgtPos} />
            )
          case 'TESTED_BY':
            return (
              <TestedByRoad key={key} sourcePos={srcPos} targetPos={tgtPos} />
            )
          case 'SEMANTICALLY_SIMILAR':
            return (
              <DashedPath key={key} from={srcPos} to={tgtPos} />
            )
          default:
            return null
        }
      })}
    </>
  )
}
