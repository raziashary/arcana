// City layout — squarified treemap using d3-hierarchy
import * as d3Hierarchy from 'd3-hierarchy'
import type { GraphNode, GraphEdge, Community } from '../../../stores/graphStore'
import type { CityLayout, BuildingLayout, DistrictLayout } from './layoutTypes'

const COMMUNITY_COLORS = [
  '#4E79A7', '#F28E2B', '#59A14F', '#E15759', '#76B7B2',
  '#EDC948', '#B07AA1', '#FF9DA7', '#9C755F', '#BAB0AC',
]

export function communityColor(id: number): string {
  return COMMUNITY_COLORS[id % COMMUNITY_COLORS.length]
}

function clamp(v: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, v))
}

export function buildingHeight(node: GraphNode, outDegree: number): number {
  const loc = node.line_end && node.line_start ? node.line_end - node.line_start : 10
  return clamp(0.5 + loc / 50 + outDegree * 0.8, 0.5, 35)
}

export function buildingWidth(inDegree: number): number {
  return clamp(0.8 + inDegree * 0.15, 0.8, 4.0)
}

export function buildingDepth(outDegree: number): number {
  return clamp(0.8 + outDegree * 0.15, 0.8, 4.0)
}

export function computeCityLayout(
  communities: Community[],
  nodes: GraphNode[],
  edges: GraphEdge[],
  citySize = 180
): CityLayout {
  // Build degree maps
  const inDegreeMap = new Map<string, number>()
  const outDegreeMap = new Map<string, number>()
  edges.forEach(e => {
    const s = typeof e.source === 'string' ? e.source : (e.source as GraphNode).id
    const t = typeof e.target === 'string' ? e.target : (e.target as GraphNode).id
    outDegreeMap.set(s, (outDegreeMap.get(s) || 0) + 1)
    inDegreeMap.set(t, (inDegreeMap.get(t) || 0) + 1)
  })

  // Build node lookup
  const nodeMap = new Map<string, GraphNode>()
  nodes.forEach(n => nodeMap.set(n.id, n))

  // Build community → nodes map
  const commNodes = new Map<number, GraphNode[]>()
  communities.forEach(c => commNodes.set(c.id, []))
  nodes.forEach(n => {
    if (n.community !== undefined) {
      if (!commNodes.has(n.community)) commNodes.set(n.community, [])
      commNodes.get(n.community)!.push(n)
    }
  })

  // Build d3 hierarchy data
  const hierarchyData: any = {
    id: 'root',
    value: 0,
    children: communities.map(c => {
      const cNodes = commNodes.get(c.id) || []
      return {
        id: `community-${c.id}`,
        communityId: c.id,
        communityName: c.name,
        value: Math.max(1, cNodes.length),
        children: cNodes.map(n => ({
          id: n.id,
          nodeId: n.id,
          value: Math.max(1, Math.log(1 + (n.line_end && n.line_start ? n.line_end - n.line_start : 10))),
        }))
      }
    })
  }

  // Apply squarified treemap
  const root = d3Hierarchy.hierarchy(hierarchyData)
    .sum((d: any) => d.value ?? 0)
    .sort((a, b) => (b.value ?? 0) - (a.value ?? 0))

  d3Hierarchy.treemap<any>()
    .size([citySize, citySize])
    .paddingOuter(4)
    .paddingInner(2)
    .paddingTop(5)
    .tile(d3Hierarchy.treemapSquarify)(root)

  const buildingPositions = new Map<string, BuildingLayout>()
  const districts: DistrictLayout[] = []

  root.children?.forEach((distNode: any) => {
    const communityId: number = distNode.data.communityId
    const dx = distNode.x0
    const dz = distNode.y0
    const dw = distNode.x1 - distNode.x0
    const dd = distNode.y1 - distNode.y0

    const buildingLayouts: BuildingLayout[] = []

    distNode.children?.forEach((buildingNode: any) => {
      const nodeId: string = buildingNode.data.nodeId
      const node = nodeMap.get(nodeId)
      if (!node) return

      const bx = buildingNode.x0
      const bz = buildingNode.y0
      const bw = buildingNode.x1 - buildingNode.x0
      const bd = buildingNode.y1 - buildingNode.y0

      const outDeg = outDegreeMap.get(nodeId) || 0
      const bh = buildingHeight(node, outDeg)

      const layout: BuildingLayout = {
        nodeId,
        x: bx + bw / 2,   // center
        z: bz + bd / 2,
        // Fill 75% of the treemap cell — cell size already encodes node weight
        width: Math.max(0.5, bw * 0.75),
        depth: Math.max(0.5, bd * 0.75),
        // Triple height so buildings read from overview zoom
        height: clamp(bh * 3, 1.5, 80),
        districtId: communityId,
      }

      buildingLayouts.push(layout)
      buildingPositions.set(nodeId, layout)
    })

    districts.push({
      communityId,
      name: distNode.data.communityName,
      x: dx,
      z: dz,
      width: dw,
      depth: dd,
      color: communityColor(communityId),
      buildings: buildingLayouts,
    })
  })

  return { districts, citySize, buildingPositions }
}
