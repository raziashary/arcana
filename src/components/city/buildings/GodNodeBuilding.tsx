// GodNodeBuilding.tsx — Individual landmark tower for god nodes (high-degree nodes)
// Rendered as a tall octagonal tower with animated beacon + point light
import { useRef } from 'react'
import { useFrame } from '@react-three/fiber'
import { Text, Billboard } from '@react-three/drei'
import * as THREE from 'three'
import type { GraphNode, ImpactData } from '../../../stores/graphStore'
import type { BuildingLayout } from '../layout/layoutTypes'
import { IMPACT_COLORS } from './buildingMaterials'
import { communityColor } from '../layout/cityLayout'

interface GodNodeBuildingProps {
  node: GraphNode
  layout: BuildingLayout
  isSelected: boolean
  impactData: ImpactData | null
}

export function GodNodeBuilding({ node, layout, isSelected, impactData }: GodNodeBuildingProps) {
  const beaconRef = useRef<THREE.Mesh>(null!)
  const lightRef = useRef<THREE.PointLight>(null!)
  const towerRef = useRef<THREE.Mesh>(null!)

  const { x, z, height, width } = layout
  const towerH = height * 1.5  // god nodes are taller
  const baseColor = communityColor(node.community ?? 0)

  // Determine color in impact mode
  const impactRisk = impactData?.affected.get(node.id)?.risk
  const color = impactData
    ? (node.id === impactData.epicenterId ? '#ffffff' : impactRisk ? IMPACT_COLORS[impactRisk].getStyle() : '#111111')
    : (isSelected ? '#ffffff' : baseColor)

  // Animate beacon pulse
  useFrame(({ clock }) => {
    const t = clock.getElapsedTime()
    if (beaconRef.current) {
      const pulse = 0.8 + Math.sin(t * 2.5) * 0.3
      beaconRef.current.scale.setScalar(pulse)
    }
    if (lightRef.current) {
      lightRef.current.intensity = 1.5 + Math.sin(t * 2.5) * 0.5
    }
    if (towerRef.current && impactData && node.id === impactData.epicenterId) {
      // Strobe effect for epicenter
      const strobe = Math.sin(t * 10) > 0 ? 1 : 0.6
      ;(towerRef.current.material as THREE.MeshStandardMaterial).emissiveIntensity = strobe
    }
  })

  const label = node.label || node.id.split('::').pop() || node.id

  return (
    <group position={[x, 0, z]}>
      {/* Main tower body — octagonal (CylinderGeometry with 8 segments) */}
      <mesh ref={towerRef} position={[0, towerH / 2, 0]} castShadow>
        <cylinderGeometry args={[width * 0.45, width * 0.55, towerH, 8]} />
        <meshStandardMaterial
          color={color}
          emissive={color}
          emissiveIntensity={0.3}
          metalness={0.7}
          roughness={0.3}
        />
      </mesh>

      {/* Tapered top cap */}
      <mesh position={[0, towerH + width * 0.3, 0]} castShadow>
        <coneGeometry args={[width * 0.35, width * 0.6, 8]} />
        <meshStandardMaterial
          color={color}
          emissive={color}
          emissiveIntensity={0.5}
          metalness={0.8}
          roughness={0.2}
        />
      </mesh>

      {/* Pulsing beacon sphere */}
      <mesh ref={beaconRef} position={[0, towerH + width * 0.7, 0]}>
        <sphereGeometry args={[width * 0.25, 8, 8]} />
        <meshStandardMaterial
          color={color}
          emissive={color}
          emissiveIntensity={1.5}
          metalness={0.1}
          roughness={0.1}
          transparent
          opacity={0.85}
        />
      </mesh>

      {/* Point light for bloom */}
      <pointLight
        ref={lightRef}
        position={[0, towerH + width * 0.7, 0]}
        color={color}
        intensity={2.0}
        distance={20}
        decay={2}
      />

      {/* Name label — billboard so it always faces camera */}
      <Billboard position={[0, towerH + width + 1.5, 0]} follow>
        <Text
          fontSize={0.6}
          color={color}
          fillOpacity={0.9}
          anchorX="center"
          anchorY="middle"
          outlineWidth={0.04}
          outlineColor="#000000"
          outlineOpacity={0.8}
          maxWidth={10}
          textAlign="center"
        >
          {label}
        </Text>
      </Billboard>
    </group>
  )
}
