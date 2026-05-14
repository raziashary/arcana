// roadAggregator.ts — Aggregate IMPORTS_FROM edges by community pair
// Returns community-level aggregate roads instead of per-node roads
import type { GraphNode, GraphEdge } from '../../../stores/graphStore'
import type { CityLayout, AggregateRoad } from '../layout/layoutTypes'

export function aggregateImportRoads(
  edges: GraphEdge[],
  nodes: GraphNode[],
  layout: CityLayout
): AggregateRoad[] {
  // Build node → community lookup
  const nodeComm = new Map<string, number>()
  nodes.forEach(n => {
    if (n.community !== undefined) nodeComm.set(n.id, n.community)
  })

  // Build district center lookup
  const districtCenter = new Map<number, [number, number, number]>()
  layout.districts.forEach(d => {
    districtCenter.set(d.communityId, [d.x + d.width / 2, 0, d.z + d.depth / 2])
  })

  // Count edges per community pair
  const pairCounts = new Map<string, number>()

  edges.forEach(e => {
    if (e.kind !== 'IMPORTS_FROM' && e.kind !== 'DEPENDS_ON') return

    const srcId = typeof e.source === 'string' ? e.source : (e.source as GraphNode).id
    const tgtId = typeof e.target === 'string' ? e.target : (e.target as GraphNode).id

    const srcComm = nodeComm.get(srcId)
    const tgtComm = nodeComm.get(tgtId)
    if (srcComm === undefined || tgtComm === undefined) return
    if (srcComm === tgtComm) return  // Same district — internal road, skip

    const key = `${srcComm}->${tgtComm}`
    pairCounts.set(key, (pairCounts.get(key) ?? 0) + 1)
  })

  const roads: AggregateRoad[] = []

  pairCounts.forEach((count, key) => {
    const [srcStr, tgtStr] = key.split('->')
    const srcComm = parseInt(srcStr)
    const tgtComm = parseInt(tgtStr)

    const srcCenter = districtCenter.get(srcComm)
    const tgtCenter = districtCenter.get(tgtComm)
    if (!srcCenter || !tgtCenter) return

    roads.push({
      from: srcCenter,
      to: tgtCenter,
      width: Math.max(2.5, Math.log(1 + count) * 2.5),
      laneCount: Math.min(count, 6),
      color: '#3a5fff',
      srcCommunity: srcComm,
      tgtCommunity: tgtComm,
      edgeCount: count,
    })
  })

  return roads
}
