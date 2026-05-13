import { useEffect } from 'react'
import { MapControls } from '@react-three/drei'
import { useThree } from '@react-three/fiber'
import * as THREE from 'three'

interface CityCameraProps {
  citySize?: number
}

export function CityCamera({ citySize = 180 }: CityCameraProps) {
  const { camera } = useThree()

  useEffect(() => {
    // Initial camera position: 45° angle overhead
    camera.position.set(citySize / 2, citySize * 0.6, citySize * 0.9)
    camera.lookAt(citySize / 2, 0, citySize / 2)
  }, [camera, citySize])

  // Keyboard shortcuts
  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      const target = new THREE.Vector3(citySize / 2, 0, citySize / 2)
      switch (e.key) {
        case '0':
          camera.position.set(citySize / 2, citySize * 0.6, citySize * 0.9)
          camera.lookAt(target)
          break
        case 'g':
        case 'G':
          camera.position.set(citySize / 2, 5, citySize * 0.7)
          camera.lookAt(target)
          break
        case 't':
        case 'T':
          camera.position.set(citySize / 2, citySize * 1.2, citySize / 2)
          camera.lookAt(target)
          break
      }
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [camera, citySize])

  return (
    <MapControls
      minDistance={3}
      maxDistance={citySize * 1.8}
      minPolarAngle={Math.PI / 12}
      maxPolarAngle={Math.PI / 2.1}
      enableDamping
      dampingFactor={0.08}
      autoRotate={false}
      enablePan
      panSpeed={1.2}
      zoomSpeed={1.0}
    />
  )
}
