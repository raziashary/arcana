// Shared graph state store — consumed by both 2D GraphCanvas and 3D CityView
import { create } from 'zustand'

export interface GraphNode {
  id: string
  label: string
  kind: string
  language: string
  community?: number
  community_name?: string
  file_path: string
  size: number
  line_start?: number
  line_end?: number
  x?: number
  y?: number
}

export interface GraphEdge {
  source: string | GraphNode
  target: string | GraphNode
  kind: string
  weight: number
  confidence_tier?: string
}

export interface Community {
  id: number
  name: string
  members: string[]
  cohesion_score: number
}

export interface GraphVisualizationData {
  nodes: GraphNode[]
  edges: GraphEdge[]
  communities: Community[]
  is_aggregated?: boolean
  total_node_count?: number
}

export type ImpactRisk = 'epicenter' | 'high' | 'medium' | 'low' | 'test' | 'untested' | 'ghost'

export interface ImpactData {
  epicenterId: string
  affected: Map<string, { risk: ImpactRisk; hops: number }>
  testNodes: string[]
  untestedNodes: string[]
}

interface GraphStore {
  // Shared data
  graphData: GraphVisualizationData | null
  setGraphData: (data: GraphVisualizationData | null) => void

  // View mode
  viewMode: 'city' | 'graph'
  setViewMode: (mode: 'city' | 'graph') => void

  // Shared selection
  selectedNodeId: string | null
  setSelectedNode: (nodeId: string | null) => void

  // Impact mode
  impactData: ImpactData | null
  setImpactData: (data: ImpactData | null) => void

  // City-specific
  hoveredNodeId: string | null
  setHoveredNode: (nodeId: string | null) => void
}

export const useGraphStore = create<GraphStore>((set) => ({
  graphData: null,
  setGraphData: (data) => set({ graphData: data }),

  viewMode: 'graph',
  setViewMode: (mode) => set({ viewMode: mode }),

  selectedNodeId: null,
  setSelectedNode: (nodeId) => set({ selectedNodeId: nodeId }),

  impactData: null,
  setImpactData: (data) => set({ impactData: data }),

  hoveredNodeId: null,
  setHoveredNode: (nodeId) => set({ hoveredNodeId: nodeId }),
}))
