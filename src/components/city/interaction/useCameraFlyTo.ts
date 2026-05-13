// useCameraFlyTo.ts — Smooth camera fly-to animation (no gsap dependency)
// Uses useFrame lerp for zero-dependency smooth camera movement
import { useRef, useCallback } from 'react'
import { useThree, useFrame } from '@react-three/fiber'
import * as THREE from 'three'

interface FlyTarget {
  position: THREE.Vector3
  lookAt: THREE.Vector3
}

export function useCameraFlyTo() {
  const { camera } = useThree()
  const targetRef = useRef<FlyTarget | null>(null)
  const progressRef = useRef(0)

  useFrame(() => {
    if (!targetRef.current) return
    progressRef.current = Math.min(progressRef.current + 0.025, 1)
    const t = easeInOut(progressRef.current)

    camera.position.lerp(targetRef.current.position, t * 0.1)
    if (progressRef.current >= 1) {
      targetRef.current = null
    }
  })

  const flyTo = useCallback((position: THREE.Vector3, lookAt: THREE.Vector3) => {
    targetRef.current = { position: position.clone(), lookAt: lookAt.clone() }
    progressRef.current = 0
  }, [])

  return flyTo
}

function easeInOut(t: number): number {
  return t < 0.5 ? 2 * t * t : -1 + (4 - 2 * t) * t
}

// Camera preset helpers
export function cityOverviewPosition(citySize: number): { pos: THREE.Vector3; look: THREE.Vector3 } {
  return {
    pos: new THREE.Vector3(citySize / 2, citySize * 0.6, citySize * 0.9),
    look: new THREE.Vector3(citySize / 2, 0, citySize / 2),
  }
}

export function streetLevelPosition(citySize: number): { pos: THREE.Vector3; look: THREE.Vector3 } {
  return {
    pos: new THREE.Vector3(citySize / 2, 5, citySize * 0.7),
    look: new THREE.Vector3(citySize / 2, 0, citySize / 2),
  }
}

export function topDownPosition(citySize: number): { pos: THREE.Vector3; look: THREE.Vector3 } {
  return {
    pos: new THREE.Vector3(citySize / 2, citySize * 1.2, citySize / 2),
    look: new THREE.Vector3(citySize / 2, 0, citySize / 2),
  }
}
