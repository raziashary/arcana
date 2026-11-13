import { CITY } from '../../../styles/palette'

// roadGenerator.ts — Dispatch edge kind → road geometry type
// This module is used for future extension and documentation of the road system.
// The actual rendering components are in their respective files.

export type EdgeKind = 'CALLS' | 'IMPORTS_FROM' | 'INHERITS' | 'TESTED_BY' | 'DEPENDS_ON' | 'SEMANTICALLY_SIMILAR'

export interface RoadConfig {
  type: 'arc' | 'ground' | 'bridge' | 'service'
  color: string
  opacity: number
  width: number
  elevated: boolean
}

export const EDGE_ROAD_CONFIG: Record<string, RoadConfig> = {
  CALLS: {
    type: 'arc',
    color: CITY.ok,
    opacity: 0.8,
    width: 0.1,
    elevated: true,
  },
  IMPORTS_FROM: {
    type: 'ground',
    color: CITY.link,
    opacity: 0.3,
    width: 0.8,
    elevated: false,
  },
  INHERITS: {
    type: 'bridge',
    color: CITY.cat6,
    opacity: 0.75,
    width: 0.15,
    elevated: true,
  },
  TESTED_BY: {
    type: 'service',
    color: CITY.cat1,
    opacity: 0.5,
    width: 0.06,
    elevated: false,
  },
  DEPENDS_ON: {
    type: 'ground',
    color: CITY.cat5,
    opacity: 0.3,
    width: 0.5,
    elevated: false,
  },
  SEMANTICALLY_SIMILAR: {
    type: 'ground',
    color: CITY.cat8,
    opacity: 0.2,
    width: 0.2,
    elevated: false,
  },
}

export function getRoadConfig(edgeKind: string): RoadConfig {
  return EDGE_ROAD_CONFIG[edgeKind] ?? EDGE_ROAD_CONFIG.DEPENDS_ON
}
