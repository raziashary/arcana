export function CityLighting() {
  return (
    <>
      {/* Ambient: bright enough to see unlit geometry */}
      <ambientLight intensity={0.7} color="#8899cc" />
      {/* Primary directional — angled sun */}
      <directionalLight
        position={[90, 120, 60]}
        intensity={2.0}
        color="#ffffff"
        castShadow
        shadow-mapSize-width={2048}
        shadow-mapSize-height={2048}
        shadow-camera-far={500}
        shadow-camera-left={-200}
        shadow-camera-right={200}
        shadow-camera-top={200}
        shadow-camera-bottom={-200}
      />
      {/* Fill light from opposite side */}
      <directionalLight position={[-60, 40, -60]} intensity={0.5} color="#4466aa" />
      {/* Hemisphere sky/ground */}
      <hemisphereLight args={['#2a3a6a', '#1a1a2a', 0.8]} />
      <fog attach="fog" args={['#050508', 120, 380]} />
    </>
  )
}

export function CityGround({ size = 180 }: { size?: number }) {
  return (
    <group>
      {/* Base ground */}
      <mesh rotation={[-Math.PI / 2, 0, 0]} position={[size / 2, -0.02, size / 2]} receiveShadow>
        <planeGeometry args={[size + 20, size + 20]} />
        <meshStandardMaterial color="#060606" roughness={0.95} metalness={0.05} />
      </mesh>
      {/* Grid lines overlay using wireframe */}
      <gridHelper
        args={[size + 20, Math.floor((size + 20) / 5), '#1a1a1a', '#111111']}
        position={[size / 2, 0, size / 2]}
      />
    </group>
  )
}
