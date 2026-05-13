// ImpactOverlay.tsx — Emergency mode visual overlay for blast radius analysis
// Highlights affected buildings with color state machine (changed/high/medium/low/ghost)
import { useRef } from 'react'
import { useFrame } from '@react-three/fiber'
import * as THREE from 'three'
import type { GraphNode, ImpactData, ImpactRisk } from '../../../stores/graphStore'
import type { CityLayout } from '../layout/layoutTypes'

interface ImpactOverlayProps {
  nodes: GraphNode[]
  layout: CityLayout
  impactData: ImpactData
}

const RISK_COLORS: Record<ImpactRisk, THREE.Color> = {
  epicenter: new THREE.Color('#ffffff'),
  high:      new THREE.Color('#ff2222'),
  medium:    new THREE.Color('#ff8800'),
  low:       new THREE.Color('#aaff00'),
  test:      new THREE.Color('#00ff88'),
  untested:  new THREE.Color('#ff3333'),
  ghost:     new THREE.Color('#111111'),
}

const RISK_EMISSIVE_INTENSITY: Record<ImpactRisk, number> = {
  epicenter: 1.5,
  high:      0.9,
  medium:    0.5,
  low:       0.3,
  test:      0.7,
  untested:  0.4,
  ghost:     0.0,
}

interface ImpactBuildingProps {
  node: GraphNode
  layout: { x: number; z: number; width: number; height: number; depth: number }
  risk: ImpactRisk
}

function ImpactBuilding({ node, layout, risk }: ImpactBuildingProps) {
  const meshRef = useRef<THREE.Mesh>(null!)
  const matRef = useRef<THREE.MeshStandardMaterial>(null!)

  const color = RISK_COLORS[risk]
  const baseEmissive = RISK_EMISSIVE_INTENSITY[risk]

  useFrame(({ clock }) => {
    if (!matRef.current) return
    const t = clock.getElapsedTime()

    if (risk === 'epicenter') {
      // Strobe for epicenter
      matRef.current.emissiveIntensity = 0.8 + Math.sin(t * 12) * 0.7
    } else if (risk === 'high') {
      // Flicker for high risk
      matRef.current.emissiveIntensity = baseEmissive + Math.sin(t * 7 + node.id.length) * 0.3
    } else if (risk === 'test') {
      // Slow pulse for test nodes
      matRef.current.emissiveIntensity = baseEmissive + Math.sin(t * 2) * 0.3
    }
  })

  return (
    <mesh
      ref={meshRef}
      position={[layout.x, layout.height / 2, layout.z]}
      scale={[layout.width + 0.1, layout.height + 0.1, layout.depth + 0.1]}
    >
      <boxGeometry args={[1, 1, 1]} />
      <meshStandardMaterial
        ref={matRef}
        color={color}
        emissive={color}
        emissiveIntensity={baseEmissive}
        transparent
        opacity={risk === 'ghost' ? 0.05 : 0.75}
        depthWrite={risk !== 'ghost'}
      />
    </mesh>
  )
}

export function ImpactOverlay({ nodes, layout, impactData }: ImpactOverlayProps) {
  return (
    <>
      {nodes.map(node => {
        const bl = layout.buildingPositions.get(node.id)
        if (!bl) return null

        let risk: ImpactRisk = 'ghost'
        if (node.id === impactData.epicenterId) {
          risk = 'epicenter'
        } else {
          const impact = impactData.affected.get(node.id)
          if (impact) {
            risk = impact.risk
          }
        }

        return (
          <ImpactBuilding
            key={node.id}
            node={node}
            layout={bl}
            risk={risk}
          />
        )
      })}
    </>
  )
}
