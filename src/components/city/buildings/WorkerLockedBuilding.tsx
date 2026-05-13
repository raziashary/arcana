// WorkerLockedBuilding.tsx — Construction scaffolding overlay for worker-locked nodes
import * as THREE from 'three'
import type { BuildingLayout } from '../layout/layoutTypes'

interface WorkerLockedBuildingProps {
  layout: BuildingLayout
}

// Yellow-black diagonal scaffolding lines
const SCAFFOLD_COLOR = '#ffcc00'

export function WorkerLockedBuilding({ layout }: WorkerLockedBuildingProps) {
  const { x, z, width, height, depth } = layout

  return (
    <group position={[x, 0, z]}>
      {/* Semi-transparent overlay on building face */}
      <mesh position={[0, height / 2, 0]}>
        <boxGeometry args={[width + 0.1, height + 0.1, depth + 0.1]} />
        <meshStandardMaterial
          color={SCAFFOLD_COLOR}
          transparent
          opacity={0.08}
          wireframe={false}
          side={THREE.FrontSide}
          depthWrite={false}
        />
      </mesh>

      {/* Scaffolding wireframe */}
      <mesh position={[0, height / 2, 0]}>
        <boxGeometry args={[width + 0.15, height + 0.15, depth + 0.15]} />
        <meshStandardMaterial
          color={SCAFFOLD_COLOR}
          wireframe
          transparent
          opacity={0.35}
        />
      </mesh>

      {/* Crane arm */}
      <mesh position={[width * 0.3, height + 1.0, 0]} rotation={[0, 0, Math.PI / 8]}>
        <boxGeometry args={[0.08, 2.0, 0.08]} />
        <meshStandardMaterial color={SCAFFOLD_COLOR} metalness={0.7} roughness={0.3} />
      </mesh>

      {/* Crane vertical pole */}
      <mesh position={[0, height + 0.5, 0]}>
        <boxGeometry args={[0.08, 1.0, 0.08]} />
        <meshStandardMaterial color={SCAFFOLD_COLOR} metalness={0.7} roughness={0.3} />
      </mesh>

      {/* "UNDER CONSTRUCTION" sign billboard */}
      <mesh position={[0, height / 2, depth / 2 + 0.12]}>
        <planeGeometry args={[Math.min(width, 1.5), 0.4]} />
        <meshStandardMaterial color="#ffcc00" emissive="#aa8800" emissiveIntensity={0.3} />
      </mesh>
    </group>
  )
}
