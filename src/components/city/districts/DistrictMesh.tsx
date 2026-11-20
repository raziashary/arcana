// DistrictMesh.tsx — Community ground plane + boundary wall + name billboard
import * as THREE from 'three'
import { CityLabel } from '../CityLabel'
import { districtLabel } from './districtLabels'
import type { DistrictLayout } from '../layout/layoutTypes'

interface DistrictMeshProps {
  district: DistrictLayout
  /** The label to show, when the caller has made it unique among districts. */
  label?: string
}

export function DistrictMesh({ district, label: given }: DistrictMeshProps) {
  const { x, z, width, depth, color, communityId, name } = district

  const centerX = x + width / 2
  const centerZ = z + depth / 2

  const groundColor = new THREE.Color(color)
  const wallColor = new THREE.Color(color)

  const label = given ?? districtLabel(name, communityId)

  return (
    <group>
      {/* Ground fill — slightly above main ground, rotated flat (PlaneGeometry is XY by default) */}
      <mesh position={[centerX, 0.02, centerZ]} rotation={[-Math.PI / 2, 0, 0]} receiveShadow>
        <planeGeometry args={[width, depth]} />
        <meshStandardMaterial
          color={groundColor}
          transparent
          opacity={0.40}
          roughness={0.9}
          metalness={0.0}
          side={THREE.DoubleSide}
          depthWrite={false}
        />
      </mesh>

      {/* Boundary walls — thin raised box on each edge */}
      {/* North wall */}
      <mesh position={[centerX, 0.75, z]} castShadow frustumCulled={false}>
        <boxGeometry args={[width, 1.5, 0.15]} />
        <meshStandardMaterial color={wallColor} emissive={wallColor} emissiveIntensity={0.15} transparent opacity={0.50} roughness={0.6} />
      </mesh>
      {/* South wall */}
      <mesh position={[centerX, 0.75, z + depth]} castShadow frustumCulled={false}>
        <boxGeometry args={[width, 1.5, 0.15]} />
        <meshStandardMaterial color={wallColor} emissive={wallColor} emissiveIntensity={0.15} transparent opacity={0.50} roughness={0.6} />
      </mesh>
      {/* West wall */}
      <mesh position={[x, 0.75, centerZ]} castShadow frustumCulled={false}>
        <boxGeometry args={[0.15, 1.5, depth]} />
        <meshStandardMaterial color={wallColor} emissive={wallColor} emissiveIntensity={0.15} transparent opacity={0.50} roughness={0.6} />
      </mesh>
      {/* East wall */}
      <mesh position={[x + width, 0.75, centerZ]} castShadow frustumCulled={false}>
        <boxGeometry args={[0.15, 1.5, depth]} />
        <meshStandardMaterial color={wallColor} emissive={wallColor} emissiveIntensity={0.15} transparent opacity={0.50} roughness={0.6} />
      </mesh>

      {/* District name billboard */}
      <CityLabel
        position={[centerX, 4.0, centerZ]}
        label={label}
        color={color}
        fontPx={Math.max(14, Math.min(24, Math.min(width, depth) * 0.9))}
        distanceFactor={90}
        maxWidthPx={Math.max(160, Math.min(width, depth) * 6)}
        priority={width * depth}
      />
    </group>
  )
}
