// GroundRoad.tsx — Ground roads: aggregate district boulevards + per-node typed roads
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

// ─── PerNodeGroundRoad: narrow typed road between two building positions ─────

const PER_NODE_STYLES: Record<string, { color: string; emissive: string; width: number }> = {
  IMPORTS_FROM: { color: '#1a1f2e', emissive: '#2244aa', width: 1.2 },
  DEPENDS_ON:   { color: '#181f10', emissive: '#665500', width: 0.9 },
}

interface PerNodeGroundRoadProps {
  from: THREE.Vector3
  to: THREE.Vector3
  edgeKind: 'IMPORTS_FROM' | 'DEPENDS_ON'
}

export function PerNodeGroundRoad({ from, to, edgeKind }: PerNodeGroundRoadProps) {
  const { midX, midZ, length, angle } = useMemo(() => {
    const dir = to.clone().sub(from)
    const mid = from.clone().add(to).multiplyScalar(0.5)
    return {
      midX: mid.x,
      midZ: mid.z,
      length: dir.length(),
      angle: Math.atan2(dir.z, dir.x),
    }
  }, [from, to])

  const style = PER_NODE_STYLES[edgeKind] ?? PER_NODE_STYLES.DEPENDS_ON

  if (length < 0.1) return null

  return (
    <mesh
      position={[midX, 0.10, midZ]}
      rotation={[0, -angle, 0]}
      frustumCulled={false}
    >
      <boxGeometry args={[length, 0.06, style.width]} />
      <meshStandardMaterial
        color={style.color}
        emissive={style.emissive}
        emissiveIntensity={0.55}
        transparent
        opacity={0.88}
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
