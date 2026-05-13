// buildingMaterials.ts — Material presets for each node kind + state
import * as THREE from 'three'

// Base palette
const MATERIAL_DEFS: Record<string, { color: string; emissive: string; metalness: number; roughness: number }> = {
  Function:   { color: '#2a4a7f', emissive: '#0a1a3a', metalness: 0.3, roughness: 0.6 },
  Class:      { color: '#3d7a3d', emissive: '#0f1f0f', metalness: 0.4, roughness: 0.5 },
  Module:     { color: '#5a3d7a', emissive: '#1a0f2a', metalness: 0.2, roughness: 0.7 },
  Test:       { color: '#f28e2b', emissive: '#3a2000', metalness: 0.1, roughness: 0.8 },
  Type:       { color: '#4d8080', emissive: '#0f2020', metalness: 0.5, roughness: 0.4 },
  Interface:  { color: '#806644', emissive: '#201500', metalness: 0.3, roughness: 0.6 },
  Constant:   { color: '#806666', emissive: '#200f0f', metalness: 0.2, roughness: 0.7 },
  Variable:   { color: '#668080', emissive: '#0f2020', metalness: 0.2, roughness: 0.7 },
  default:    { color: '#404040', emissive: '#0a0a0a', metalness: 0.2, roughness: 0.8 },
}

// Cache materials so we don't recreate them every frame
const materialCache = new Map<string, THREE.MeshStandardMaterial>()

export function getMaterialForKind(kind: string): THREE.MeshStandardMaterial {
  const key = kind
  if (materialCache.has(key)) return materialCache.get(key)!

  const def = MATERIAL_DEFS[kind] ?? MATERIAL_DEFS.default
  const mat = new THREE.MeshStandardMaterial({
    color: new THREE.Color(def.color),
    emissive: new THREE.Color(def.emissive),
    metalness: def.metalness,
    roughness: def.roughness,
  })
  materialCache.set(key, mat)
  return mat
}

/** Selected building outline color */
export const SELECTED_COLOR = new THREE.Color('#ffffff')

/** God node color — bright warm gold */
export const GOD_NODE_COLOR = new THREE.Color('#ffcc44')

/** Impact mode colors */
export const IMPACT_COLORS = {
  epicenter: new THREE.Color('#ffffff'),
  high:      new THREE.Color('#ff2222'),
  medium:    new THREE.Color('#ff8800'),
  low:       new THREE.Color('#aaff00'),
  test:      new THREE.Color('#00ff88'),
  untested:  new THREE.Color('#ff0000'),
  ghost:     new THREE.Color('#111111'),
}

/** Node kind → geometry type hint */
export function nodeGeometryHint(kind: string): 'box' | 'cylinder' | 'octagon' {
  if (kind === 'Class' || kind === 'Interface') return 'cylinder'
  return 'box'
}

export function disposeMaterials(): void {
  materialCache.forEach(mat => mat.dispose())
  materialCache.clear()
}
