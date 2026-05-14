// GroundRoad.tsx — IMPORTS_FROM community-level aggregate boulevard roads
// Also exports AggregateRoads component (used by CityView)
import { useMemo } from 'react'
import * as THREE from 'three'
import type { GraphNode, GraphEdge } from '../../../stores/graphStore'
import type { CityLayout, AggregateRoad } from '../layout/layoutTypes'
import { aggregateImportRoads } from './roadAggregator'

interface GroundRoadProps {
  road: AggregateRoad
}

function GroundRoad({ road }: GroundRoadProps) {
  const { from, to, width, color } = road

  const geometry = useMemo(() => {
    const start = new THREE.Vector3(...from)
    const end = new THREE.Vector3(...to)
    const dir = end.clone().sub(start)
    const length = dir.length()

    const geo = new THREE.PlaneGeometry(length, width)
    geo.rotateX(-Math.PI / 2)  // lay flat

    // Rotate to point from start to end
    const angle = Math.atan2(dir.z, dir.x)
    geo.rotateY(-angle)

    // Center on midpoint
    const mid = start.clone().add(end).multiplyScalar(0.5)
    geo.translate(mid.x, 0.04, mid.z)

    return geo
  }, [from, to, width])

  return (
    <mesh geometry={geometry} receiveShadow>
      <meshStandardMaterial
        color={color}
        emissive="#1a2a5a"
        emissiveIntensity={0.5}
        transparent
        opacity={0.65}
        roughness={0.7}
        metalness={0.1}
        side={THREE.DoubleSide}
        depthWrite={false}
      />
    </mesh>
  )
}

// ─── AggregateRoads: component consumed by CityView ─────────────────────────

interface AggregateRoadsProps {
  edges: GraphEdge[]
  nodes: GraphNode[]
  layout: CityLayout
}

export function AggregateRoads({ edges, nodes, layout }: AggregateRoadsProps) {
  const roads = useMemo(
    () => aggregateImportRoads(edges, nodes, layout),
    [edges, nodes, layout]
  )

  return (
    <>
      {roads.map((road, i) => (
        <GroundRoad key={`${road.srcCommunity}->${road.tgtCommunity}-${i}`} road={road} />
      ))}
    </>
  )
}
