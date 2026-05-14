// buildingMaterials.ts — Material presets for each node kind + state
import * as THREE from 'three'

export interface MaterialDef { color: string; emissive: string; metalness: number; roughness: number }

// Base palette — colors at 60–80% lightness so buildings are readable against dark ground
const MATERIAL_DEFS: Record<string, MaterialDef> = {
  Function:   { color: '#5588dd', emissive: '#1a3a7a', metalness: 0.3, roughness: 0.55 },
  Class:      { color: '#55bb55', emissive: '#1a551a', metalness: 0.4, roughness: 0.50 },
  Module:     { color: '#8855cc', emissive: '#331166', metalness: 0.2, roughness: 0.65 },
  Test:       { color: '#ff9933', emissive: '#553300', metalness: 0.1, roughness: 0.75 },
  Type:       { color: '#44aaaa', emissive: '#114444', metalness: 0.5, roughness: 0.40 },
  Interface:  { color: '#cc9944', emissive: '#553311', metalness: 0.3, roughness: 0.55 },
  Constant:   { color: '#cc6677', emissive: '#441122', metalness: 0.2, roughness: 0.65 },
  Variable:   { color: '#77aacc', emissive: '#1a3a55', metalness: 0.2, roughness: 0.65 },
  default:    { color: '#7a7a8a', emissive: '#1a1a22', metalness: 0.2, roughness: 0.75 },
}

// Cache materials so we don't recreate them every frame
const materialCache = new Map<string, THREE.MeshStandardMaterial>()

export function getMaterialDef(kind: string): MaterialDef {
  return MATERIAL_DEFS[kind] ?? MATERIAL_DEFS.default
}

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
