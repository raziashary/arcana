import { useEffect, useRef, useMemo } from 'react'
import { MapControls } from '@react-three/drei'
import { useThree } from '@react-three/fiber'
import * as THREE from 'three'

interface CityCameraProps {
  citySize?: number
}

export function CityCamera({ citySize = 180 }: CityCameraProps) {
  const { camera } = useThree()
  // Ref to the MapControls instance so we can update its target programmatically
  const controlsRef = useRef<any>(null)

  const cityCenter = useMemo(() => new THREE.Vector3(citySize / 2, 0, citySize / 2), [citySize])

  useEffect(() => {
    // Initial camera position: 45° overhead angle looking at city centre
    camera.position.set(citySize / 2, citySize * 0.55, citySize * 0.85)
    camera.updateProjectionMatrix()
    // Also update MapControls target so the orbit pivot is city centre
    if (controlsRef.current) {
      controlsRef.current.target.copy(cityCenter)
      controlsRef.current.update()
    }
  }, [camera, citySize]) // eslint-disable-line react-hooks/exhaustive-deps

  // Keyboard shortcuts
  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      // Ignore if user is typing in an input
      if ((e.target as HTMLElement)?.tagName === 'INPUT') return
      switch (e.key) {
        case '0': {
          camera.position.set(citySize / 2, citySize * 0.55, citySize * 0.85)
          if (controlsRef.current) { controlsRef.current.target.copy(cityCenter); controlsRef.current.update() }
          break
        }
        case 'g':
        case 'G': {
          camera.position.set(citySize / 2, 8, citySize * 0.65)
          if (controlsRef.current) { controlsRef.current.target.copy(cityCenter); controlsRef.current.update() }
          break
        }
        case 't':
        case 'T': {
          camera.position.set(citySize / 2, citySize * 1.1, citySize / 2)
          if (controlsRef.current) { controlsRef.current.target.copy(cityCenter); controlsRef.current.update() }
          break
        }
      }
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [camera, citySize]) // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <MapControls
      ref={controlsRef}
      target={cityCenter}
      minDistance={0.5}
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
