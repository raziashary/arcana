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
    color: '#00FF88',
    opacity: 0.8,
    width: 0.1,
    elevated: true,
  },
  IMPORTS_FROM: {
    type: 'ground',
    color: '#4477FF',
    opacity: 0.3,
    width: 0.8,
    elevated: false,
  },
  INHERITS: {
    type: 'bridge',
    color: '#B07AA1',
    opacity: 0.75,
    width: 0.15,
    elevated: true,
  },
  TESTED_BY: {
    type: 'service',
    color: '#F28E2B',
    opacity: 0.5,
    width: 0.06,
    elevated: false,
  },
  DEPENDS_ON: {
    type: 'ground',
    color: '#EDC948',
    opacity: 0.3,
    width: 0.5,
    elevated: false,
  },
  SEMANTICALLY_SIMILAR: {
    type: 'ground',
    color: '#9C755F',
    opacity: 0.2,
    width: 0.2,
    elevated: false,
  },
}

export function getRoadConfig(edgeKind: string): RoadConfig {
  return EDGE_ROAD_CONFIG[edgeKind] ?? EDGE_ROAD_CONFIG.DEPENDS_ON
}
