// CallsArc.tsx — Elevated glowing arcs for CALLS edges (QuadraticBezierCurve3 + TubeGeometry)
import { useMemo } from 'react'
import * as THREE from 'three'
import type { GraphNode, GraphEdge } from '../../../stores/graphStore'
import type { CityLayout } from '../layout/layoutTypes'

interface CallsArcProps {
  sourcePos: THREE.Vector3
  targetPos: THREE.Vector3
  confidence: string
}

function CallsArc({ sourcePos, targetPos, confidence }: CallsArcProps) {
  const geometry = useMemo(() => {
    const dist = sourcePos.distanceTo(targetPos)
    const arcHeight = 2.0 + dist * 0.08

    const midpoint = sourcePos.clone().add(targetPos).multiplyScalar(0.5)
    midpoint.y = arcHeight

    const curve = new THREE.QuadraticBezierCurve3(
      sourcePos.clone().setY(Math.max(sourcePos.y, 0.5) + 0.3),
      midpoint,
      targetPos.clone().setY(Math.max(targetPos.y, 0.5) + 0.3)
    )

    const tubeRadius = confidence === 'AMBIGUOUS' ? 0.04 : 0.09
    return new THREE.TubeGeometry(curve, 20, tubeRadius, 5, false)
  }, [sourcePos, targetPos, confidence])

  const opacity = confidence === 'EXTRACTED' ? 0.8 : confidence === 'INFERRED' ? 0.5 : 0.25

  return (
    <mesh geometry={geometry}>
      <meshStandardMaterial
        color="#00FF88"
        emissive="#00FF88"
        emissiveIntensity={0.6}
        transparent
        opacity={opacity}
        side={THREE.DoubleSide}
      />
    </mesh>
  )
}

// ─── CallsArcs: component consumed by CityView ───────────────────────────────

interface CallsArcsProps {
  edges: GraphEdge[]
  nodes?: GraphNode[]
  layout: CityLayout
  focusNodeId: string | null
}

export function CallsArcs({ edges, layout, focusNodeId }: CallsArcsProps) {
  // Filter to CALLS edges only; if focusNodeId, show only edges connected to it
  const callsEdges = useMemo(() => {
    return edges.filter(e => {
      if (e.kind !== 'CALLS') return false
      if (focusNodeId) {
        const srcId = typeof e.source === 'string' ? e.source : (e.source as GraphNode).id
        const tgtId = typeof e.target === 'string' ? e.target : (e.target as GraphNode).id
        return srcId === focusNodeId || tgtId === focusNodeId
      }
      return true
    })
  }, [edges, focusNodeId])

  // Cap at 200 arcs for performance
  const visibleEdges = callsEdges.slice(0, 200)

  return (
    <>
      {visibleEdges.map((edge, i) => {
        const srcId = typeof edge.source === 'string' ? edge.source : (edge.source as GraphNode).id
        const tgtId = typeof edge.target === 'string' ? edge.target : (edge.target as GraphNode).id

        const srcLayout = layout.buildingPositions.get(srcId)
        const tgtLayout = layout.buildingPositions.get(tgtId)
        if (!srcLayout || !tgtLayout) return null

        const srcPos = new THREE.Vector3(srcLayout.x, srcLayout.height, srcLayout.z)
        const tgtPos = new THREE.Vector3(tgtLayout.x, tgtLayout.height, tgtLayout.z)

        return (
          <CallsArc
            key={`calls-${srcId}-${tgtId}-${i}`}
            sourcePos={srcPos}
            targetPos={tgtPos}
            confidence={edge.confidence_tier ?? 'INFERRED'}
          />
        )
      })}
    </>
  )
}
