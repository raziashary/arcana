// buildingMaterials.ts — Material presets for each node kind + state
import * as THREE from 'three'
import { CITY } from '../../../styles/palette'

export interface MaterialDef { color: string; emissive: string; metalness: number; roughness: number }

// Base palette — colors at 60–80% lightness so buildings are readable against dark ground
const MATERIAL_DEFS: Record<string, MaterialDef> = {
  Function:   { color: CITY.kindFunction, emissive: CITY.kindFunctionGlow, metalness: 0.3, roughness: 0.55 },
  Class:      { color: CITY.kindClass, emissive: CITY.kindClassGlow, metalness: 0.4, roughness: 0.50 },
  Module:     { color: CITY.kindModule, emissive: CITY.kindModuleGlow, metalness: 0.2, roughness: 0.65 },
  Test:       { color: CITY.kindTest, emissive: CITY.kindTestGlow, metalness: 0.1, roughness: 0.75 },
  Type:       { color: CITY.kindType, emissive: CITY.kindTypeGlow, metalness: 0.5, roughness: 0.40 },
  Interface:  { color: CITY.kindInterface, emissive: CITY.kindInterfaceGlow, metalness: 0.3, roughness: 0.55 },
  Constant:   { color: CITY.kindConstant, emissive: CITY.kindConstantGlow, metalness: 0.2, roughness: 0.65 },
  Variable:   { color: CITY.kindVariable, emissive: CITY.kindVariableGlow, metalness: 0.2, roughness: 0.65 },
  default:    { color: CITY.kindDefault, emissive: CITY.kindDefaultGlow, metalness: 0.2, roughness: 0.75 },
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
export const SELECTED_COLOR = new THREE.Color(CITY.white)

/** God node color — bright warm gold */
export const GOD_NODE_COLOR = new THREE.Color(CITY.god)

/** Impact mode colors */
export const IMPACT_COLORS = {
  epicenter: new THREE.Color(CITY.white),
  high:      new THREE.Color(CITY.riskHigh),
  medium:    new THREE.Color(CITY.riskMedium),
  low:       new THREE.Color(CITY.riskLow),
  test:      new THREE.Color(CITY.ok),
  untested:  new THREE.Color(CITY.riskUntested),
  ghost:     new THREE.Color(CITY.panel),
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
