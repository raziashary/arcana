// buildingDimensions.ts — Height/width/depth from LOC + degree metrics
import type { GraphNode } from '../../../stores/graphStore'
import type { BuildingLayout } from '../layout/layoutTypes'

function clamp(v: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, v))
}

/** Building height encodes complexity: lines-of-code + outgoing call degree */
export function buildingHeight(node: GraphNode, outDegree: number): number {
  const loc = node.line_end && node.line_start ? Math.max(0, node.line_end - node.line_start) : 10
  return clamp(0.5 + loc / 50 + outDegree * 0.8, 0.5, 35)
}

/** Building width encodes popularity: in-degree (how many things import/call this) */
export function buildingWidth(inDegree: number): number {
  return clamp(0.8 + inDegree * 0.15, 0.8, 4.0)
}

/** Building depth encodes coupling: out-degree (how many things this calls) */
export function buildingDepth(outDegree: number): number {
  return clamp(0.8 + outDegree * 0.15, 0.8, 4.0)
}

/**
 * Returns [width, height, depth] for a node given its layout.
 * The layout already has pre-computed values — this is a fast accessor.
 */
export function getBuildingDimensions(layout: BuildingLayout): [number, number, number] {
  return [layout.width, layout.height, layout.depth]
}

/**
 * Test coverage color: green (full) → orange (partial) → red (none).
 * Right now test data is inferred from edge presence of TESTED_BY edges.
 */
export function testCoverageColor(
  nodeId: string,
  testedBySet: Set<string>
): { r: number; g: number; b: number } {
  if (testedBySet.has(nodeId)) {
    return { r: 0.15, g: 0.85, b: 0.45 }   // green — tested
  }
  return { r: 0.85, g: 0.35, b: 0.15 }     // orange-red — untested
}
