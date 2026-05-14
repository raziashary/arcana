// BuildingInstances.tsx — InstancedMesh rendering for all regular buildings
// One draw call per node kind (Function, Class, Test, Type, etc.)
import { useRef, useEffect, useMemo } from 'react'
import * as THREE from 'three'
import type { GraphNode, ImpactData } from '../../../stores/graphStore'
import type { CityLayout } from '../layout/layoutTypes'
import { testCoverageColor } from './buildingDimensions'
import { getMaterialDef, IMPACT_COLORS } from './buildingMaterials'

interface BuildingInstancesProps {
  nodes: GraphNode[]
  layout: CityLayout
  degreeMap?: { inDeg: Map<string, number>; outDeg: Map<string, number> }
  selectedNodeId: string | null
  impactData: ImpactData | null
}

// Group nodes by kind for separate instanced meshes
function groupByKind(nodes: GraphNode[]): Map<string, GraphNode[]> {
  const groups = new Map<string, GraphNode[]>()
  nodes.forEach(n => {
    const kind = n.kind || 'default'
    if (!groups.has(kind)) groups.set(kind, [])
    groups.get(kind)!.push(n)
  })
  return groups
}

interface KindInstancesProps {
  nodes: GraphNode[]
  kind: string
  layout: CityLayout
  testedBySet: Set<string>
  selectedNodeId: string | null
  impactData: ImpactData | null
}

function KindInstances({ nodes, kind, layout, testedBySet, selectedNodeId, impactData }: KindInstancesProps) {
  const meshRef = useRef<THREE.InstancedMesh>(null!)
  const count = nodes.length
  const def = useMemo(() => getMaterialDef(kind), [kind])

  useEffect(() => {
    if (!meshRef.current || count === 0) return
    const mesh = meshRef.current
    const temp = new THREE.Object3D()
    const color = new THREE.Color()

    nodes.forEach((node, i) => {
      const bl = layout.buildingPositions.get(node.id)
      if (!bl) {
        // Zero-scale hidden building
        temp.position.set(0, -100, 0)
        temp.scale.set(0, 0, 0)
        temp.updateMatrix()
        mesh.setMatrixAt(i, temp.matrix)
        return
      }

      temp.position.set(bl.x, bl.height / 2, bl.z)
      temp.scale.set(bl.width, bl.height, bl.depth)
      temp.rotation.set(0, 0, 0)
      temp.updateMatrix()
      mesh.setMatrixAt(i, temp.matrix)

      // Determine color: impact mode overrides test coverage
      if (impactData) {
        const risk = impactData.affected.get(node.id)?.risk
        if (node.id === impactData.epicenterId) {
          color.copy(IMPACT_COLORS.epicenter)
        } else if (risk) {
          color.copy(IMPACT_COLORS[risk])
        } else {
          color.set('#111111') // ghost
        }
      } else if (selectedNodeId === node.id) {
        color.set('#ffffff')
      } else {
        const tc = testCoverageColor(node.id, testedBySet)
        // Blend base material color with coverage tint (slight)
        color.set(def.color).lerp(new THREE.Color(tc.r, tc.g, tc.b), 0.15)
      }

      mesh.setColorAt(i, color)
    })

    mesh.instanceMatrix.needsUpdate = true
    if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true
  }, [nodes, layout, testedBySet, selectedNodeId, impactData, kind])

  if (count === 0) return null

  return (
    <instancedMesh
      ref={meshRef}
      args={[undefined, undefined, count]}
      castShadow
      receiveShadow
      frustumCulled={false}
    >
      <boxGeometry args={[1, 1, 1]} />
      <meshStandardMaterial
        color={def.color}
        emissive={def.emissive}
        metalness={def.metalness}
        roughness={def.roughness}
      />
    </instancedMesh>
  )
}

export function BuildingInstances({
  nodes,
  layout,
  selectedNodeId,
  impactData,
}: BuildingInstancesProps) {
  // Build set of nodes that have TESTED_BY edges (derived from layout data)
  // We approximate: any node whose kind contains 'test' is a test, others: check if tested
  const testedBySet = useMemo(() => {
    // We don't have raw edges here, so we infer from building positions for now
    // The actual TESTED_BY connections are handled in ImpactOverlay
    // This is a placeholder that marks nothing, coloring will rely on base materials
    return new Set<string>()
  }, [])

  const groups = useMemo(() => groupByKind(nodes), [nodes])

  return (
    <>
      {Array.from(groups.entries()).map(([kind, kindNodes]) => (
        <KindInstances
          key={kind}
          nodes={kindNodes}
          kind={kind}
          layout={layout}
          testedBySet={testedBySet}
          selectedNodeId={selectedNodeId}
          impactData={impactData}
        />
      ))}
    </>
  )
}
