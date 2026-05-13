// BuildingRaycaster.tsx — Click/hover detection on instanced meshes
// Uses Three.js raycaster with instanceId → node lookup
import { useCallback, useMemo, useEffect } from 'react'
import { useThree } from '@react-three/fiber'
import * as THREE from 'three'
import type { GraphNode } from '../../../stores/graphStore'
import type { CityLayout } from '../layout/layoutTypes'
import { useGraphStore } from '../../../stores/graphStore'

interface BuildingRaycasterProps {
  nodes: GraphNode[]
  layout: CityLayout
  onBuildingClick: (nodeId: string) => void
  onBuildingRightClick: (nodeId: string, x: number, y: number) => void
}

// Build an array of [position, nodeId] for fast raycasting fallback
function buildHitmap(nodes: GraphNode[], layout: CityLayout): Array<{ nodeId: string; box: THREE.Box3 }> {
  return nodes
    .map(n => {
      const bl = layout.buildingPositions.get(n.id)
      if (!bl) return null
      const box = new THREE.Box3(
        new THREE.Vector3(bl.x - bl.width / 2, 0, bl.z - bl.depth / 2),
        new THREE.Vector3(bl.x + bl.width / 2, bl.height, bl.z + bl.depth / 2)
      )
      return { nodeId: n.id, box }
    })
    .filter(Boolean) as Array<{ nodeId: string; box: THREE.Box3 }>
}

export function BuildingRaycaster({
  nodes,
  layout,
  onBuildingClick,
  onBuildingRightClick,
}: BuildingRaycasterProps) {
  const { camera, gl } = useThree()
  const { setHoveredNode } = useGraphStore()
  const raycaster = useMemo(() => new THREE.Raycaster(), [])
  const hitmap = useMemo(() => buildHitmap(nodes, layout), [nodes, layout])

  const getNearestHit = useCallback(
    (clientX: number, clientY: number): string | null => {
      const rect = gl.domElement.getBoundingClientRect()
      const ndc = new THREE.Vector2(
        ((clientX - rect.left) / rect.width) * 2 - 1,
        -((clientY - rect.top) / rect.height) * 2 + 1
      )
      raycaster.setFromCamera(ndc, camera)
      const ray = raycaster.ray

      let nearest: string | null = null
      let nearestDist = Infinity

      hitmap.forEach(({ nodeId, box }) => {
        const target = new THREE.Vector3()
        if (ray.intersectBox(box, target)) {
          const d = camera.position.distanceTo(target)
          if (d < nearestDist) {
            nearestDist = d
            nearest = nodeId
          }
        }
      })

      return nearest
    },
    [camera, gl, hitmap, raycaster]
  )

  const handleClick = useCallback(
    (e: MouseEvent) => {
      const hit = getNearestHit(e.clientX, e.clientY)
      if (hit) {
        e.stopPropagation()
        onBuildingClick(hit)
      }
    },
    [getNearestHit, onBuildingClick]
  )

  const handleRightClick = useCallback(
    (e: MouseEvent) => {
      const hit = getNearestHit(e.clientX, e.clientY)
      if (hit) {
        e.preventDefault()
        e.stopPropagation()
        onBuildingRightClick(hit, e.clientX, e.clientY)
      }
    },
    [getNearestHit, onBuildingRightClick]
  )

  const handleMouseMove = useCallback(
    (e: MouseEvent) => {
      const hit = getNearestHit(e.clientX, e.clientY)
      setHoveredNode(hit)
      gl.domElement.style.cursor = hit ? 'pointer' : 'default'
    },
    [getNearestHit, setHoveredNode, gl]
  )

  // Attach event listeners to the canvas — must be useEffect (not useMemo) so cleanup runs
  useEffect(() => {
    const el = gl.domElement
    el.addEventListener('click', handleClick)
    el.addEventListener('contextmenu', handleRightClick)
    el.addEventListener('mousemove', handleMouseMove)
    return () => {
      el.removeEventListener('click', handleClick)
      el.removeEventListener('contextmenu', handleRightClick)
      el.removeEventListener('mousemove', handleMouseMove)
    }
  }, [gl, handleClick, handleRightClick, handleMouseMove])

  // This component has no visual output — it's purely event-driven
  return null
}
