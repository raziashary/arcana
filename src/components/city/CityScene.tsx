export function CityLighting() {
  return (
    <>
      <ambientLight intensity={0.15} color="#101828" />
      <directionalLight
        position={[50, 80, 30]}
        intensity={1.2}
        color="#E8F0FF"
        castShadow
        shadow-mapSize-width={2048}
        shadow-mapSize-height={2048}
        shadow-camera-far={400}
        shadow-camera-left={-150}
        shadow-camera-right={150}
        shadow-camera-top={150}
        shadow-camera-bottom={-150}
      />
      <hemisphereLight args={['#1a2240', '#0a0a0a', 0.5]} />
      <fog attach="fog" args={['#050508', 100, 320]} />
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
