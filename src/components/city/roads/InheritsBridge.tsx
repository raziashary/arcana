// InheritsBridge.tsx — Elevated arch bridges for INHERITS edges
import { useMemo } from 'react'
import * as THREE from 'three'
import type { GraphNode, GraphEdge } from '../../../stores/graphStore'
import type { CityLayout } from '../layout/layoutTypes'

interface InheritsBridgeProps {
  sourcePos: THREE.Vector3
  targetPos: THREE.Vector3
}

export function InheritsBridge({ sourcePos, targetPos }: InheritsBridgeProps) {
  const geometry = useMemo(() => {
    const archHeight = 4.0

    const mid = sourcePos.clone().add(targetPos).multiplyScalar(0.5)
    mid.y = archHeight

    const curve = new THREE.QuadraticBezierCurve3(
      sourcePos.clone().setY(1.0),
      mid,
      targetPos.clone().setY(1.0)
    )

    return new THREE.TubeGeometry(curve, 24, 0.15, 6, false)
  }, [sourcePos, targetPos])

  return (
    <mesh geometry={geometry} castShadow frustumCulled={false}>
      <meshStandardMaterial
        color="#9966cc"
        emissive="#3d1166"
        emissiveIntensity={0.4}
        metalness={0.4}
        roughness={0.6}
        transparent
        opacity={0.85}
      />
    </mesh>
  )
}

interface TestedByRoadProps {
  sourcePos: THREE.Vector3
  targetPos: THREE.Vector3
}

export function TestedByRoad({ sourcePos, targetPos }: TestedByRoadProps) {
  const geometry = useMemo(() => {
    const curve = new THREE.LineCurve3(
      sourcePos.clone().setY(0.08),
      targetPos.clone().setY(0.08)
    )
    return new THREE.TubeGeometry(curve, 4, 0.06, 4, false)
  }, [sourcePos, targetPos])

  return (
    <mesh geometry={geometry} frustumCulled={false}>
      <meshStandardMaterial
        color="#cc6622"
        emissive="#882200"
        emissiveIntensity={0.4}
        transparent
        opacity={0.80}
      />
    </mesh>
  )
}

// ─── InheritsBridges + TestedByRoads: components for CityView ────────────────

interface EdgeRoadsProps {
  edges: GraphEdge[]
  nodes?: GraphNode[]
  layout: CityLayout
  kind: 'INHERITS' | 'TESTED_BY'
}

export function EdgeRoads({ edges, layout, kind }: EdgeRoadsProps) {
  const filtered = useMemo(() =>
    edges.filter(e => e.kind === kind).slice(0, 100),
    [edges, kind]
  )

  return (
    <>
      {filtered.map((edge, i) => {
        const srcId = typeof edge.source === 'string' ? edge.source : (edge.source as GraphNode).id
        const tgtId = typeof edge.target === 'string' ? edge.target : (edge.target as GraphNode).id

        const srcLayout = layout.buildingPositions.get(srcId)
        const tgtLayout = layout.buildingPositions.get(tgtId)
        if (!srcLayout || !tgtLayout) return null

        const srcPos = new THREE.Vector3(srcLayout.x, srcLayout.height, srcLayout.z)
        const tgtPos = new THREE.Vector3(tgtLayout.x, tgtLayout.height, tgtLayout.z)

        if (kind === 'INHERITS') {
          return <InheritsBridge key={`bridge-${i}`} sourcePos={srcPos} targetPos={tgtPos} />
        }
        return <TestedByRoad key={`tested-${i}`} sourcePos={srcPos} targetPos={tgtPos} />
      })}
    </>
  )
}
