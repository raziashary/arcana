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

  // Center-line marking width — narrow bright strip on top of the road
  const centerWidth = Math.max(0.15, width * 0.12)

  return (
    <group position={[midX, 0, midZ]} rotation={[0, -angle, 0]}>
      {/* Road base — solid, sits clearly above ground */}
      <mesh position={[0, 0.12, 0]} receiveShadow frustumCulled={false}>
        <boxGeometry args={[length, 0.08, width]} />
        <meshStandardMaterial
          color={color}
          emissive="#5588ff"
          emissiveIntensity={1.2}
          roughness={0.55}
          metalness={0.25}
        />
      </mesh>
      {/* Center lane stripe — bright glow line along the road */}
      <mesh position={[0, 0.17, 0]} frustumCulled={false}>
        <boxGeometry args={[length * 0.96, 0.02, centerWidth]} />
        <meshStandardMaterial
          color="#ffffff"
          emissive="#aaccff"
          emissiveIntensity={2.0}
          roughness={0.3}
          metalness={0.1}
        />
      </mesh>
    </group>
  )
}

// ─── PerNodeGroundRoad: narrow typed road between two building positions ─────

const PER_NODE_STYLES: Record<string, { color: string; emissive: string; width: number }> = {
  IMPORTS_FROM: { color: '#4477ff', emissive: '#6699ff', width: 1.4 },
  DEPENDS_ON:   { color: '#ddaa33', emissive: '#ffcc55', width: 1.0 },
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
      position={[midX, 0.14, midZ]}
      rotation={[0, -angle, 0]}
      frustumCulled={false}
    >
      <boxGeometry args={[length, 0.07, style.width]} />
      <meshStandardMaterial
        color={style.color}
        emissive={style.emissive}
        emissiveIntensity={1.4}
        roughness={0.5}
        metalness={0.2}
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
