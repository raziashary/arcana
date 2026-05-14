// DistrictMesh.tsx — Community ground plane + boundary wall + name billboard
import { Text, Billboard } from '@react-three/drei'
import * as THREE from 'three'
import type { DistrictLayout } from '../layout/layoutTypes'

interface DistrictMeshProps {
  district: DistrictLayout
}

// Translate a community DB name to a human-readable city district label
function districtLabel(name: string, id: number): string {
  if (!name) return `DISTRICT ${id}`
  const map: Record<string, string> = {
    'agent-core': 'AGENT DISTRICT',
    'api-health': 'API QUARTER',
    'memory-system': 'MEMORY SECTOR',
    'test-suite': 'TEST PRECINCT',
    'module-mcp': 'MCP TERMINAL',
  }
  const lower = name.toLowerCase()
  if (map[lower]) return map[lower]
  // module paths → "EXTERNAL MODULES"
  if (lower.startsWith('module-c:') || lower.startsWith('module-/')) return 'EXTERNAL MODULES'
  // Convert kebab-case community names to ALL CAPS label
  return name.toUpperCase().replace(/-/g, ' ')
}

export function DistrictMesh({ district }: DistrictMeshProps) {
  const { x, z, width, depth, color, communityId, name } = district

  const centerX = x + width / 2
  const centerZ = z + depth / 2

  const groundColor = new THREE.Color(color)
  const wallColor = new THREE.Color(color)

  const label = districtLabel(name, communityId)

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
      <mesh position={[centerX, 0.15, z]} castShadow>
        <boxGeometry args={[width, 0.3, 0.12]} />
        <meshStandardMaterial color={wallColor} transparent opacity={0.35} roughness={0.6} />
      </mesh>
      {/* South wall */}
      <mesh position={[centerX, 0.15, z + depth]} castShadow>
        <boxGeometry args={[width, 0.3, 0.12]} />
        <meshStandardMaterial color={wallColor} transparent opacity={0.35} roughness={0.6} />
      </mesh>
      {/* West wall */}
      <mesh position={[x, 0.15, centerZ]} castShadow>
        <boxGeometry args={[0.12, 0.3, depth]} />
        <meshStandardMaterial color={wallColor} transparent opacity={0.35} roughness={0.6} />
      </mesh>
      {/* East wall */}
      <mesh position={[x + width, 0.15, centerZ]} castShadow>
        <boxGeometry args={[0.12, 0.3, depth]} />
        <meshStandardMaterial color={wallColor} transparent opacity={0.35} roughness={0.6} />
      </mesh>

      {/* District name billboard */}
      <Billboard position={[centerX, 4.0, centerZ]} follow lockX={false} lockY={false} lockZ={false}>
        <Text
          fontSize={Math.max(3.0, Math.min(width, depth) * 0.18)}
          color={color}
          fillOpacity={0.9}
          anchorX="center"
          anchorY="middle"
          outlineWidth={0.15}
          outlineColor="#000000"
          outlineOpacity={1.0}
          maxWidth={width * 0.95}
          textAlign="center"
          depthOffset={-2}
          renderOrder={10}
        >
          {label}
        </Text>
      </Billboard>
    </group>
  )
}
