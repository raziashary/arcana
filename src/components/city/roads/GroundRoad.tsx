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

  const { midX, midZ, length, angle } = useMemo(() => {
    const start = new THREE.Vector3(...from)
    const end = new THREE.Vector3(...to)
    const dir = end.clone().sub(start)
    const mid = start.clone().add(end).multiplyScalar(0.5)
    return {
      midX: mid.x,
      midZ: mid.z,
      length: dir.length(),
      angle: Math.atan2(dir.z, dir.x),
    }
  }, [from, to])

  return (
    <mesh
      position={[midX, 0.08, midZ]}
      rotation={[0, -angle, 0]}
      receiveShadow
      frustumCulled={false}
    >
      {/* Thin box so it has visible side faces at oblique camera angles */}
      <boxGeometry args={[length, 0.06, width]} />
      <meshStandardMaterial
        color={color}
        emissive="#2244aa"
        emissiveIntensity={0.4}
        transparent
        opacity={0.90}
        roughness={0.6}
        metalness={0.2}
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
