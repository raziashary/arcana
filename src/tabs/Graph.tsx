// Graph tab — code knowledge graph visualization (D3 force-directed)

import { useEffect, useState, useRef, useCallback } from 'react'
import * as d3 from 'd3'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useWorkspaceStore } from '../stores/workspaceStore'
import './Graph.css'

// ── Visual constants (AutoDev Industrial Dark palette) ──────────────
const COMMUNITY_COLORS = [
  '#4E79A7', '#F28E2B', '#59A14F', '#E15759', '#76B7B2',
  '#EDC948', '#B07AA1', '#FF9DA7', '#9C755F', '#BAB0AC',
]

const EDGE_COLORS: Record<string, string> = {
  CALLS:               '#00FF88',
  IMPORTS_FROM:        '#4477FF',
  INHERITS:            '#B07AA1',
  IMPLEMENTS:          '#76B7B2',
  TESTED_BY:           '#F28E2B',
  DEPENDS_ON:          '#EDC948',
  REFERENCES:          '#888888',
  CONTAINS:            '#2A2A2A',
  SEMANTICALLY_SIMILAR:'#9C755F',
}

const DEFAULT_EDGE_FILTERS: Record<string, boolean> = {
  CALLS: true, IMPORTS_FROM: true, INHERITS: true, IMPLEMENTS: true,
  TESTED_BY: true, DEPENDS_ON: true, REFERENCES: false,
  CONTAINS: false, SEMANTICALLY_SIMILAR: false,
}

function symbolForKind(kind: string): d3.SymbolType {
  switch (kind.toLowerCase()) {
    case 'file':    return d3.symbolDiamond   // distinct from Class
    case 'class':   return d3.symbolSquare
    case 'function':
    case 'method':  return d3.symbolCircle
    case 'test':    return d3.symbolTriangle
    case 'type':    return d3.symbolStar
    case 'module':
    case 'package': return d3.symbolWye
    case 'import':  return d3.symbolCross
    default:        return d3.symbolCircle
  }
}

function nodeRadius(degree: number): number {
  return Math.min(22, 4 + Math.log(1 + degree) * 3.5)
}

function cohesionColor(score: number): string {
  if (score > 0.5)  return '#00FF88'
  if (score >= 0.25) return '#FFAA00'
  return '#FF4444'
}

interface GraphStats {
  total_nodes: number
  total_edges: number
  total_communities: number
  average_degree: number
  graph_density: number
  last_updated?: string
}

interface Community {
  id: number
  name: string
  members: string[]
  cohesion_score: number
}

interface VisualizationNode {
  id: string
  label: string
  kind: string
  language: string
  community?: number
  community_name?: string
  file_path: string
  size: number
  x?: number
  y?: number
  vx?: number
  vy?: number
  fx?: number
  fy?: number
}

interface VisualizationEdge {
  source: string | VisualizationNode
  target: string | VisualizationNode
  kind: string
  weight: number
}

interface GraphVisualizationData {
  nodes: VisualizationNode[]
  edges: VisualizationEdge[]
  communities: Community[]
  is_aggregated?: boolean
  total_node_count?: number
}

interface BuildProgress {
  phase: string
  message: string
  percent: number
}

export default function GraphTab() {
  const workspaceDir = useWorkspaceStore(s => s.workspaceDir)
  const loadWorkspaceDir = useWorkspaceStore(s => s.loadWorkspaceDir)

  // ── SVG + D3 refs ───────────────────────────────────────────
  const svgRef   = useRef<SVGSVGElement>(null)
  const simRef   = useRef<d3.Simulation<any, any> | null>(null)
  const zoomRef  = useRef<d3.ZoomBehavior<SVGSVGElement, unknown> | null>(null)
  const nodeGRef = useRef<d3.Selection<SVGGElement, unknown, null, undefined> | null>(null)
  const edgeGRef = useRef<d3.Selection<SVGGElement, unknown, null, undefined> | null>(null)
  const hullGRef = useRef<d3.Selection<SVGGElement, unknown, null, undefined> | null>(null)
  const labelGRef= useRef<d3.Selection<SVGGElement, unknown, null, undefined> | null>(null)
  const selectedNodeRef = useRef<string | null>(null)
  const graphDataRef    = useRef<GraphVisualizationData | null>(null)

  // ── React state ─────────────────────────────────────────────
  const [stats, setStats] = useState<GraphStats | null>(null)
  const [communities, setCommunities] = useState<Community[]>([])
  const [isBuilding, setIsBuilding] = useState(false)
  const [buildProgress, setBuildProgress] = useState<BuildProgress | null>(null)
  const [selectedCommunity, setSelectedCommunity] = useState<number | null>(null)
  const [renderMode, setRenderMode] = useState<'full' | 'community' | 'auto'>('auto')
  const [graphData, setGraphData] = useState<GraphVisualizationData | null>(null)
  const [selectedNode, setSelectedNode] = useState<string | null>(null)
  const [isLoadingStats, setIsLoadingStats] = useState(false)
  const [edgeFilters, setEdgeFilters] = useState<Record<string, boolean>>(DEFAULT_EDGE_FILTERS)
  const [minDegree, setMinDegree] = useState(1)
  const [searchQuery, setSearchQuery] = useState('')
  const [searchResults, setSearchResults] = useState<VisualizationNode[]>([])
  const [tooltip, setTooltip] = useState<{ node: VisualizationNode; x: number; y: number } | null>(null)
  const [ctxMenu, setCtxMenu] = useState<{ node: VisualizationNode; x: number; y: number } | null>(null)
  const [legendExpanded, setLegendExpanded] = useState(false)
  const [commOpen, setCommOpen] = useState(true)
  const [inspOpen, setInspOpen] = useState(true)
  const [showAllIn, setShowAllIn] = useState(false)
  const [showAllOut, setShowAllOut] = useState(false)

  // Keep refs in sync
  useEffect(() => { selectedNodeRef.current = selectedNode }, [selectedNode])
  useEffect(() => { graphDataRef.current = graphData }, [graphData])

  // ── Lifecycle effects ───────────────────────────────────────
  useEffect(() => { loadWorkspaceDir() }, [loadWorkspaceDir])

  useEffect(() => {
    loadGraphStats()
    const unsub = listen('graph:updated', () => loadGraphStats())
    return () => { unsub.then(fn => fn()) }
  }, [])

  useEffect(() => {
    const unsub = listen<BuildProgress>('graph:build_progress', (event) => {
      setBuildProgress(event.payload)
      if (event.payload.phase === 'done') {
        setIsBuilding(false)
        setTimeout(() => setBuildProgress(null), 800)
      }
    })
    return () => { unsub.then(fn => fn()) }
  }, [])

  // ── D3 force graph init ──────────────────────────────────────
  const applyFocusMode = useCallback((nodeId: string | null) => {
    if (!nodeGRef.current || !edgeGRef.current || !labelGRef.current) return
    if (!nodeId) {
      nodeGRef.current.selectAll<SVGGElement, any>('g.node').attr('opacity', 1)
      edgeGRef.current.selectAll('line.edge').attr('stroke-opacity', 0.4)
      labelGRef.current.selectAll('text.node-label').attr('opacity', 1)
      return
    }
    const gd = graphDataRef.current
    if (!gd) return
    const neighbors = new Set<string>([nodeId])
    gd.edges.forEach(e => {
      const sId = typeof e.source === 'string' ? e.source : (e.source as VisualizationNode).id
      const tId = typeof e.target === 'string' ? e.target : (e.target as VisualizationNode).id
      if (sId === nodeId || tId === nodeId) { neighbors.add(sId); neighbors.add(tId) }
    })
    nodeGRef.current.selectAll<SVGGElement, any>('g.node')
      .attr('opacity', (d: any) => d.id === nodeId || neighbors.has(d.id) ? 1 : 0.08)
    edgeGRef.current.selectAll<SVGLineElement, any>('line.edge')
      .attr('stroke-opacity', (d: any) => {
        const s = typeof d.source === 'string' ? d.source : d.source?.id
        const t = typeof d.target === 'string' ? d.target : d.target?.id
        return (s === nodeId || t === nodeId) ? 0.8 : 0
      })
    labelGRef.current.selectAll<SVGTextElement, any>('text.node-label')
      .attr('opacity', (d: any) => d && (d.id === nodeId || neighbors.has(d.id)) ? 1 : 0.08)
  }, [])

  const applyCommunityFilter = useCallback((communityId: number | null) => {
    if (!nodeGRef.current || !edgeGRef.current) return
    if (communityId === null) {
      nodeGRef.current.selectAll('g.node').attr('opacity', 1)
      edgeGRef.current.selectAll('line.edge').attr('stroke-opacity', 0.4)
      return
    }
    const gd = graphDataRef.current
    if (!gd) return
    const comm = gd.communities.find(c => c.id === communityId)
    const members = new Set(comm?.members || [])
    nodeGRef.current.selectAll<SVGGElement, any>('g.node')
      .attr('opacity', (d: any) => members.has(d.id) ? 1 : 0.08)
    edgeGRef.current.selectAll<SVGLineElement, any>('line.edge')
      .attr('stroke-opacity', (d: any) => {
        const s = typeof d.source === 'string' ? d.source : d.source?.id
        const t = typeof d.target === 'string' ? d.target : d.target?.id
        return members.has(s) && members.has(t) ? 0.6 : 0.02
      })
  }, [])

  const updateHulls = useCallback((nodes: any[], commColorMap: Record<number, string>) => {
    if (!hullGRef.current) return
    const byComm = new Map<number, [number, number][]>()
    nodes.forEach((n: any) => {
      if (n.community === undefined || n.x === undefined || n.y === undefined) return
      if (!byComm.has(n.community)) byComm.set(n.community, [])
      byComm.get(n.community)!.push([n.x, n.y])
    })
    const hullData: Array<{ id: number; path: string; color: string }> = []
    byComm.forEach((pts, commId) => {
      if (pts.length < 3) return
      const hull = d3.polygonHull(pts)
      if (!hull) return
      const centroid = d3.polygonCentroid(hull)
      const expanded = hull.map(p => {
        const dx = p[0] - centroid[0], dy = p[1] - centroid[1]
        const len = Math.sqrt(dx * dx + dy * dy) || 1
        return [p[0] + (dx / len) * 28, p[1] + (dy / len) * 28] as [number, number]
      })
      const path = d3.line<[number, number]>().curve(d3.curveCatmullRomClosed)(expanded)
      if (path) hullData.push({ id: commId, path, color: commColorMap[commId] || '#888888' })
    })
    hullGRef.current.selectAll<SVGPathElement, any>('path.hull')
      .data(hullData, (d: any) => d.id)
      .join('path')
      .attr('class', 'hull')
      .attr('d', (d: any) => d.path)
      .attr('fill', (d: any) => d.color)
      .attr('fill-opacity', 0.06)
      .attr('stroke', (d: any) => d.color)
      .attr('stroke-opacity', 0.22)
      .attr('stroke-width', 1)
      .attr('stroke-dasharray', '4,3')
      .attr('pointer-events', 'none')
  }, [])

  const initD3Graph = useCallback(() => {
    if (!svgRef.current || !graphData) return
    if (simRef.current) simRef.current.stop()

    const el = svgRef.current
    const W = el.clientWidth || 800
    const H = el.clientHeight || 600

    // Degree map from full graph
    const degMap = new Map<string, number>()
    graphData.edges.forEach(e => {
      const s = typeof e.source === 'string' ? e.source : (e.source as VisualizationNode).id
      const t = typeof e.target === 'string' ? e.target : (e.target as VisualizationNode).id
      degMap.set(s, (degMap.get(s) || 0) + 1)
      degMap.set(t, (degMap.get(t) || 0) + 1)
    })

    // Filter nodes by minDegree
    const nodes: any[] = graphData.nodes
      .filter(n => (degMap.get(n.id) || 0) >= minDegree || selectedNodeRef.current === n.id)
      .map(n => ({ ...n, _degree: degMap.get(n.id) || 0 }))
    const nodeIds = new Set(nodes.map((n: any) => n.id))

    // Filter edges by kind + node availability
    const edges: any[] = graphData.edges
      .filter(e => {
        const s = typeof e.source === 'string' ? e.source : (e.source as VisualizationNode).id
        const t = typeof e.target === 'string' ? e.target : (e.target as VisualizationNode).id
        return nodeIds.has(s) && nodeIds.has(t) && edgeFilters[e.kind] !== false
      })
      .map(e => ({
        source: typeof e.source === 'string' ? e.source : (e.source as VisualizationNode).id,
        target: typeof e.target === 'string' ? e.target : (e.target as VisualizationNode).id,
        kind: e.kind,
        weight: e.weight,
      }))

    // Community color map
    const commColorMap: Record<number, string> = {}
    graphData.communities.forEach((c, i) => { commColorMap[c.id] = COMMUNITY_COLORS[i % COMMUNITY_COLORS.length] })

    // Community-seeded initial positions
    const commIds = [...new Set(nodes.map((n: any) => n.community ?? -1))]
    const centroids: Record<number, { x: number; y: number }> = {}
    const R = Math.min(W, H) * 0.35
    commIds.forEach((id, i) => {
      const angle = (2 * Math.PI * i) / Math.max(1, commIds.length)
      centroids[id] = { x: W / 2 + R * Math.cos(angle), y: H / 2 + R * Math.sin(angle) }
    })
    nodes.forEach((n: any) => {
      if (n.x === undefined) {
        const c = centroids[n.community ?? -1] || { x: W / 2, y: H / 2 }
        n.x = c.x + (Math.random() - 0.5) * 80
        n.y = c.y + (Math.random() - 0.5) * 80
      }
    })

    // Build SVG
    const svg = d3.select<SVGSVGElement, unknown>(el)
    svg.selectAll('*').remove()
    svg.on('click', () => { setSelectedNode(null); setCtxMenu(null) })

    // Defs: glow filter + arrowheads
    const defs = svg.append('defs')
    const flt = defs.append('filter').attr('id', 'node-glow')
      .attr('x', '-50%').attr('y', '-50%').attr('width', '200%').attr('height', '200%')
    flt.append('feGaussianBlur').attr('stdDeviation', '3.5').attr('result', 'coloredBlur')
    const mrg = flt.append('feMerge')
    mrg.append('feMergeNode').attr('in', 'coloredBlur')
    mrg.append('feMergeNode').attr('in', 'SourceGraphic')
    Object.entries(EDGE_COLORS).forEach(([kind, color]) => {
      if (kind === 'CONTAINS' || kind === 'SEMANTICALLY_SIMILAR') return
      defs.append('marker').attr('id', `arr-${kind.toLowerCase()}`)
        .attr('viewBox', '0 -4 8 8').attr('refX', 14)
        .attr('markerWidth', 5).attr('markerHeight', 5).attr('orient', 'auto')
        .append('path').attr('d', 'M0,-4L8,0L0,4').attr('fill', color).attr('opacity', 0.6)
    })

    // Canvas group (zoom-managed)
    const g = svg.append('g').attr('class', 'canvas-root')
    const hullG  = g.append('g').attr('class', 'hulls')
    const edgeG  = g.append('g').attr('class', 'edges')
    const nodeG  = g.append('g').attr('class', 'nodes')
    const labelG = g.append('g').attr('class', 'labels').attr('pointer-events', 'none')
    hullGRef.current  = hullG as any
    edgeGRef.current  = edgeG as any
    nodeGRef.current  = nodeG as any
    labelGRef.current = labelG as any

    // Edges
    const edgeSel = edgeG.selectAll<SVGLineElement, any>('line.edge')
      .data(edges, (d: any) => `${d.source}-${d.target}-${d.kind}`)
      .join('line')
      .attr('class', (d: any) => `edge edge-${d.kind.toLowerCase()}`)
      .attr('stroke', (d: any) => EDGE_COLORS[d.kind] || '#555555')
      .attr('stroke-opacity', 0.4)
      .attr('stroke-width', (d: any) => 0.5 + (d.weight || 1) * 1.5)
      .attr('marker-end', (d: any) =>
        (d.kind !== 'CONTAINS' && d.kind !== 'SEMANTICALLY_SIMILAR')
          ? `url(#arr-${d.kind.toLowerCase()})` : null)

    // Nodes
    const symGen = d3.symbol()
    const nodeSel = nodeG.selectAll<SVGGElement, any>('g.node')
      .data(nodes, (d: any) => d.id)
      .join('g')
      .attr('class', (d: any) => `node node-kind-${(d.kind || '').toLowerCase().replace(/\s+/g, '-')}`)
      .attr('cursor', 'pointer')
      .on('click', (ev, d: any) => {
        ev.stopPropagation()
        setSelectedNode((prev: string | null) => prev === d.id ? null : d.id)
        setShowAllIn(false); setShowAllOut(false); setCtxMenu(null)
      })
      .on('contextmenu', (ev, d: any) => {
        ev.preventDefault(); ev.stopPropagation()
        setCtxMenu({ node: d, x: ev.clientX, y: ev.clientY })
      })
      .on('mouseenter', (ev, d: any) => {
        setTooltip({ node: d, x: ev.clientX, y: ev.clientY })
        d3.select(ev.currentTarget).select('path').attr('stroke', '#FFFFFF').attr('stroke-width', 2)
      })
      .on('mouseleave', (ev, d: any) => {
        setTooltip(null)
        d3.select(ev.currentTarget).select('path')
          .attr('stroke', commColorMap[d.community ?? -1] || '#444444')
          .attr('stroke-width', selectedNodeRef.current === d.id ? 2 : 0.5)
      })
      .call(d3.drag<SVGGElement, any>()
        .on('start', (ev, d: any) => {
          if (!ev.active && simRef.current) simRef.current.alphaTarget(0.3).restart()
          d.fx = d.x; d.fy = d.y
        })
        .on('drag', (ev, d: any) => { d.fx = ev.x; d.fy = ev.y })
        .on('end', (ev, d: any) => {
          if (!ev.active && simRef.current) simRef.current.alphaTarget(0)
          d.fx = null; d.fy = null
        })
      )

    // God-node outer glow ring
    nodeSel.filter((d: any) => d._degree >= 15)
      .append('circle')
      .attr('r', (d: any) => nodeRadius(d._degree) + 6)
      .attr('fill', 'none')
      .attr('stroke', (d: any) => commColorMap[d.community ?? -1] || '#888888')
      .attr('stroke-width', 2.5)
      .attr('stroke-opacity', 0.25)

    // Main shape
    nodeSel.append('path')
      .attr('d', (d: any) => {
        const r = nodeRadius(d._degree)
        return symGen.type(symbolForKind(d.kind || '')).size(Math.PI * r * r)()
      })
      .attr('fill', (d: any) => commColorMap[d.community ?? -1] || '#444444')
      .attr('fill-opacity', (d: any) => d._degree === 0 ? 0.4 : 0.85)
      .attr('stroke', (d: any) => commColorMap[d.community ?? -1] || '#444444')
      .attr('stroke-opacity', 0.5)
      .attr('stroke-width', 0.5)

    // Labels — hidden until zoom-check
    const labelSel = labelG.selectAll<SVGTextElement, any>('text.node-label')
      .data(nodes, (d: any) => d.id)
      .join('text')
      .attr('class', 'node-label hover-label')
      .attr('text-anchor', 'middle')
      .style('font-size', '10px')
      .style('fill', '#888888')
      .text((d: any) => (d.label || '').substring(0, 20))
      .attr('display', 'none')

    // D3 Zoom
    const zoom = d3.zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.05, 12])
      .on('zoom', (ev) => {
        g.attr('transform', ev.transform)
        const k = ev.transform.k
        labelSel.attr('display', (d: any) =>
          nodeRadius(d._degree) * k >= 8 || d._degree >= 15 ? null : 'none'
        )
        labelSel
          .attr('x', (d: any) => d.x ?? 0)
          .attr('y', (d: any) => (d.y ?? 0) + nodeRadius(d._degree) + 12)
      })
    svg.call(zoom)
    zoomRef.current = zoom

    // Fit initially
    const xs = nodes.map((n: any) => n.x ?? W / 2)
    const ys = nodes.map((n: any) => n.y ?? H / 2)
    if (xs.length) {
      const x0 = Math.min(...xs), x1 = Math.max(...xs)
      const y0 = Math.min(...ys), y1 = Math.max(...ys)
      const bw = x1 - x0 || 1, bh = y1 - y0 || 1
      const s = Math.min(0.85, 0.85 * Math.min(W / bw, H / bh))
      svg.call(zoom.transform, d3.zoomIdentity.translate(W / 2 - s * (x0 + x1) / 2, H / 2 - s * (y0 + y1) / 2).scale(s))
    }

    // Force simulation
    let tick = 0
    const clusterForce = (alpha: number) => {
      nodes.forEach((d: any) => {
        const c = centroids[d.community ?? -1]
        if (!c) return
        d.vx -= (d.x - c.x) * 0.15 * alpha
        d.vy -= (d.y - c.y) * 0.15 * alpha
      })
    }

    const sim = d3.forceSimulation<any>(nodes)
      .force('charge', d3.forceManyBody<any>()
        .strength((d: any) => -150 - d._degree * 8).distanceMax(400).theta(0.9))
      .force('link', d3.forceLink<any, any>(edges).id((d: any) => d.id)
        .distance((d: any) => d.kind === 'CONTAINS' ? 20 : d.kind === 'CALLS' ? 60 : 70)
        .strength((d: any) => d.kind === 'CONTAINS' ? 0.9 : 0.3))
      .force('cluster', clusterForce as any)
      .force('collision', d3.forceCollide<any>()
        .radius((d: any) => nodeRadius(d._degree) + 3).strength(0.9).iterations(3))
      .force('center', d3.forceCenter(W / 2, H / 2).strength(0.05))
      .alphaDecay(0.02).velocityDecay(0.4)

    simRef.current = sim

    sim.on('tick', () => {
      tick++
      edgeSel
        .attr('x1', (d: any) => d.source?.x ?? 0).attr('y1', (d: any) => d.source?.y ?? 0)
        .attr('x2', (d: any) => d.target?.x ?? 0).attr('y2', (d: any) => d.target?.y ?? 0)
      nodeSel.attr('transform', (d: any) => `translate(${d.x ?? 0},${d.y ?? 0})`)
      labelSel.attr('x', (d: any) => d.x ?? 0)
        .attr('y', (d: any) => (d.y ?? 0) + nodeRadius(d._degree) + 12)
      if (tick % 3 === 0 || sim.alpha() < 0.05) updateHulls(nodes, commColorMap)
    })
    sim.on('end', () => updateHulls(nodes, commColorMap))

    // Apply initial visual state
    const selNode = selectedNodeRef.current
    if (selNode) applyFocusMode(selNode)
    else if (selectedCommunity !== null) applyCommunityFilter(selectedCommunity)
  }, [graphData, edgeFilters, minDegree, selectedCommunity, applyFocusMode, applyCommunityFilter, updateHulls])

  useEffect(() => {
    if (graphData && svgRef.current) initD3Graph()
    return () => { if (simRef.current) simRef.current.stop() }
  }, [initD3Graph])

  useEffect(() => { applyFocusMode(selectedNode) }, [selectedNode, applyFocusMode])

  useEffect(() => {
    if (!selectedNode) applyCommunityFilter(selectedCommunity)
  }, [selectedCommunity, selectedNode, applyCommunityFilter])

  useEffect(() => {
    if (!searchQuery.trim() || !graphData) { setSearchResults([]); return }
    const q = searchQuery.toLowerCase()
    setSearchResults(graphData.nodes.filter(n =>
      n.label.toLowerCase().includes(q) || n.file_path.toLowerCase().includes(q)
    ).slice(0, 10))
  }, [searchQuery, graphData])

  // ── Zoom controls ────────────────────────────────────────────
  const zoomIn  = () => { if (svgRef.current && zoomRef.current) d3.select(svgRef.current).call(zoomRef.current.scaleBy, 1.4) }
  const zoomOut = () => { if (svgRef.current && zoomRef.current) d3.select(svgRef.current).call(zoomRef.current.scaleBy, 0.7) }
  const zoomFit = () => {
    if (!svgRef.current || !zoomRef.current || !graphData?.nodes.length) return
    const el = svgRef.current
    const W = el.clientWidth || 800, H = el.clientHeight || 600
    const xs = graphData.nodes.map(n => n.x ?? W / 2)
    const ys = graphData.nodes.map(n => n.y ?? H / 2)
    const x0 = Math.min(...xs), x1 = Math.max(...xs)
    const y0 = Math.min(...ys), y1 = Math.max(...ys)
    const bw = x1 - x0 || 1, bh = y1 - y0 || 1
    const s = Math.min(0.85, 0.85 * Math.min(W / bw, H / bh))
    d3.select(el).transition().duration(500)
      .call(zoomRef.current.transform, d3.zoomIdentity.translate(W / 2 - s * (x0 + x1) / 2, H / 2 - s * (y0 + y1) / 2).scale(s))
  }
  const zoomToNode = (nodeId: string) => {
    if (!svgRef.current || !zoomRef.current || !graphData) return
    const n = graphData.nodes.find(x => x.id === nodeId)
    if (!n || n.x === undefined) return
    const el = svgRef.current
    const W = el.clientWidth || 800, H = el.clientHeight || 600
    d3.select(el).transition().duration(600)
      .call(zoomRef.current.transform, d3.zoomIdentity.translate(W / 2, H / 2).scale(3).translate(-n.x!, -n.y!))
  }

  const loadGraphStats = async () => {
    if (isLoadingStats) return
    setIsLoadingStats(true)
    try {
      const graphStats = await invoke<GraphStats>('get_graph_stats', { repoPath: workspaceDir })
      setStats(graphStats)
      if (graphStats.total_nodes > 0) {
        const vizData = await invoke<GraphVisualizationData>('get_graph_visualization_data', { repoPath: workspaceDir })
        setGraphData(vizData)
        setCommunities(vizData.communities)
      } else {
        setGraphData(null)
        setCommunities([])
      }
    } catch (err) {
      console.error('[Graph] loadGraphStats:', err)
    } finally {
      setIsLoadingStats(false)
    }
  }

  // ── Action handlers ─────────────────────────────────────────
  const handleBuildGraph = async () => {
    if (!workspaceDir) { alert('Set a workspace directory in Settings first.'); return }
    setIsBuilding(true)
    setBuildProgress({ phase: 'init', message: 'Starting build…', percent: 5 })
    try {
      await invoke('build_graph', { repoPath: workspaceDir })
      await loadGraphStats()
    } catch (err) {
      console.error('[Graph] build_graph:', err)
      alert(`Build failed: ${err}`)
    } finally {
      setIsBuilding(false)
    }
  }

  const handleUpdateGraph = async () => {
    if (!workspaceDir) { alert('Set a workspace directory in Settings first.'); return }
    setIsBuilding(true)
    setBuildProgress({ phase: 'init', message: 'Starting update…', percent: 5 })
    try {
      await invoke('update_graph', { repoPath: workspaceDir })
      await loadGraphStats()
    } catch (err) {
      console.error('[Graph] update_graph:', err)
      alert(`Update failed: ${err}`)
    } finally {
      setIsBuilding(false)
    }
  }

  const handleDeleteGraph = async () => {
    if (!workspaceDir || !confirm('Delete the code graph? Cannot be undone.')) return
    try {
      await invoke('delete_graph', { repoPath: workspaceDir })
      setStats(null); setGraphData(null); setCommunities([])
    } catch (err) { alert(`Delete failed: ${err}`) }
  }

  const handleExport = async (format: 'json' | 'html' | 'svg') => {
    if (!workspaceDir) return
    try { await invoke('export_graph', { repoPath: workspaceDir, format, outputPath: undefined }) }
    catch (err) { alert(`Export ${format} failed: ${err}`) }
  }

  const handleOpenInIde = async (filePath: string, line = 1) => {
    try { await invoke('open_in_ide', { file: filePath, line }) }
    catch (err) { console.error('[Graph] open_in_ide:', err) }
  }

  const getNodeDegree = (nodeId: string) => {
    if (!graphData) return 0
    return graphData.edges.filter(e => {
      const s = typeof e.source === 'string' ? e.source : (e.source as VisualizationNode).id
      const t = typeof e.target === 'string' ? e.target : (e.target as VisualizationNode).id
      return s === nodeId || t === nodeId
    }).length
  }

  // ── Derived render values ────────────────────────────────────
  const activeNode = selectedNode ? graphData?.nodes.find(n => n.id === selectedNode) ?? null : null
  const inEdges = activeNode && graphData ? graphData.edges.filter(e => {
    const t = typeof e.target === 'string' ? e.target : (e.target as VisualizationNode).id
    return t === selectedNode
  }) : []
  const outEdges = activeNode && graphData ? graphData.edges.filter(e => {
    const s = typeof e.source === 'string' ? e.source : (e.source as VisualizationNode).id
    return s === selectedNode
  }) : []
  const shownIn  = showAllIn  ? inEdges  : inEdges.slice(0, 5)
  const shownOut = showAllOut ? outEdges : outEdges.slice(0, 5)

  const ALL_EDGE_KINDS = Object.keys(EDGE_COLORS)
  const hasGraph = Boolean(graphData && graphData.nodes.length > 0)

  // ── Render ───────────────────────────────────────────────────
  return (
    <div className="graph-tab" onClick={() => setCtxMenu(null)}>

      {/* ── Toolbar ── */}
      <div className="graph-toolbar">
        <div className="graph-toolbar-row">
          {workspaceDir && (
            <span className="workspace-path" title={workspaceDir}>■ {workspaceDir}</span>
          )}
          <button
            className={`btn-primary${isBuilding ? ' building' : ''}`}
            onClick={handleBuildGraph}
            disabled={isBuilding}
          >
            {isBuilding ? `◌ ${buildProgress?.percent ?? 0}%` : '⚡ Build'}
          </button>
          <button className="btn-secondary" onClick={handleUpdateGraph} disabled={isBuilding}>↻ Update</button>
          <div className="export-dropdown">
            <button className="btn-secondary">📤 Export ▾</button>
            <div className="dropdown-menu">
              <button onClick={() => handleExport('json')}>JSON</button>
              <button onClick={() => handleExport('html')}>HTML</button>
              <button onClick={() => handleExport('svg')}>SVG</button>
            </div>
          </div>
          <button className="btn-secondary" onClick={zoomFit} disabled={!hasGraph}>⊡ Fit</button>
          <button className="btn-danger" onClick={handleDeleteGraph} disabled={!hasGraph}>✕</button>
        </div>

        <div className="graph-toolbar-row">
          <div className="render-mode-row">
            <span className="filter-label">Mode</span>
            <select
              className="select-control"
              title="Render mode"
              aria-label="Render mode"
              value={renderMode}
              onChange={e => setRenderMode(e.target.value as 'full' | 'community' | 'auto')}
            >
              <option value="auto">Auto</option>
              <option value="full">Full</option>
              <option value="community">Community</option>
            </select>
          </div>

          <div className="edge-filter-row">
            <span className="edge-filter-label">Edges</span>
            {ALL_EDGE_KINDS.map(kind => (
              <button
                key={kind}
                className={`edge-toggle${edgeFilters[kind] ? ' active' : ''}`}
                onClick={() => setEdgeFilters(prev => ({ ...prev, [kind]: !prev[kind] }))}
                title={kind}
              >
                {/* inline background is runtime data from EDGE_COLORS — cannot be static CSS */}
                <span className="edge-toggle-dot" style={{ background: EDGE_COLORS[kind] }} />
                {kind.replace(/_/g, ' ')}
              </button>
            ))}
          </div>

          <div className="filter-controls">
            <div className="search-wrapper">
              <input
                className="search-input"
                placeholder="🔍 Search nodes…"
                value={searchQuery}
                onChange={e => setSearchQuery(e.target.value)}
              />
              {searchResults.length > 0 && (
                <div className="search-results">
                  {searchResults.map(n => (
                    <div
                      key={n.id}
                      className="search-result-item"
                      onClick={() => {
                        setSelectedNode(n.id)
                        zoomToNode(n.id)
                        setSearchQuery('')
                        setSearchResults([])
                      }}
                    >
                      {n.label}
                      <span className="search-result-kind">{n.kind}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
            <span className="filter-label">Deg ≥</span>
            <input
              className="degree-input"
              type="number"
              min={0}
              title="Minimum node degree"
              aria-label="Minimum node degree"
              value={minDegree}
              onChange={e => setMinDegree(Math.max(0, parseInt(e.target.value) || 0))}
            />
          </div>
        </div>
      </div>

      {/* ── Stats bar ── */}
      <div className="graph-stats">
        {stats ? (
          <>
            <div className="stat-item">
              <span className="stat-label">Nodes</span>
              <span className="stat-value">{stats.total_nodes}</span>
            </div>
            <div className="stat-item">
              <span className="stat-label">Edges</span>
              <span className="stat-value">{stats.total_edges}</span>
            </div>
            <div className="stat-item">
              <span className="stat-label">Communities</span>
              <span className="stat-value">{stats.total_communities}</span>
            </div>
            <div className="stat-item">
              <span className="stat-label">Avg Deg</span>
              <span className="stat-value dim">{stats.average_degree.toFixed(2)}</span>
            </div>
            <div className="stat-item">
              <span className="stat-label">Density</span>
              <span className="stat-value dim">{(stats.graph_density * 100).toFixed(2)}%</span>
            </div>
          </>
        ) : (
          <span className="stat-placeholder">No graph — click ⚡ Build to start</span>
        )}
      </div>

      {/* ── Aggregation notice (shown when >2000 nodes triggers community view) ── */}
      {graphData?.is_aggregated && (
        <div className="graph-aggregation-banner">
          <span className="graph-aggregation-icon">⬡</span>
          <span className="graph-aggregation-text">
            Large codebase ({graphData.total_node_count?.toLocaleString()} nodes) — showing community overview.
            Each node represents a community. Narrow scope or filter to inspect individual files.
          </span>
        </div>
      )}

      {/* ── Main content ── */}
      <div className="graph-content">

        {/* Canvas area */}
        <div className="graph-canvas">
          {hasGraph ? (
            <svg ref={svgRef} className="graph-svg" />
          ) : (
            <div className="canvas-placeholder">
              <div className="canvas-placeholder-icon">⬡</div>
              <div className="canvas-placeholder-title">No graph for this repo</div>
              <div className="canvas-placeholder-text">
                Build a graph to visualize your codebase.<br />
                First build may take 30–90 s for large repos.
              </div>
              <button className="btn-primary" onClick={handleBuildGraph} disabled={isBuilding}>
                ⚡ Build Graph
              </button>
            </div>
          )}

          {/* Build overlay */}
          {isBuilding && buildProgress && (
            <div className="graph-loading-overlay">
              <div className="graph-loading-spinner" />
              <div className="graph-loading-label">{buildProgress.message}</div>
              <div className="graph-loading-sub">{buildProgress.phase}</div>
              <div className="graph-loading-bar">
                {/* width is a runtime percentage — cannot be static CSS */}
                <div className="graph-loading-bar-fill" style={{ width: `${buildProgress.percent}%` }} />
              </div>
            </div>
          )}

          {/* Zoom controls */}
          {hasGraph && (
            <div className="graph-zoom-controls">
              <button className="zoom-btn" onClick={zoomIn} title="Zoom in">+</button>
              <button className="zoom-btn" onClick={zoomOut} title="Zoom out">−</button>
              <button className="zoom-btn" onClick={zoomFit} title="Fit">⊡</button>
            </div>
          )}

          {/* Edge legend */}
          {hasGraph && (
            <div className="graph-legend">
              <div className="legend-toggle" onClick={() => setLegendExpanded(!legendExpanded)}>
                {legendExpanded ? '▾' : '▸'} Legend
              </div>
              {legendExpanded && ALL_EDGE_KINDS
                .filter(k => edgeFilters[k] && k !== 'CONTAINS')
                .map(k => (
                  <div key={k} className="legend-item" onClick={() => setEdgeFilters(p => ({ ...p, [k]: !p[k] }))}>
                    {/* background is runtime data from EDGE_COLORS map — cannot be static CSS */}
                    <div className="legend-dash" style={{ background: EDGE_COLORS[k] }} />
                    {k}
                  </div>
                ))
              }
            </div>
          )}

          {/* Tooltip — position is from mouse event, cannot be static CSS */}
          {tooltip && (
            <div className="graph-tooltip" style={{ left: tooltip.x + 14, top: tooltip.y + 14 }}>
              <div className="tooltip-name">{tooltip.node.label}</div>
              <div className="tooltip-meta">
                {tooltip.node.kind} · {tooltip.node.language} · Community {tooltip.node.community}
              </div>
              <div className="tooltip-path">{tooltip.node.file_path}</div>
              <div className="tooltip-degree">degree: {getNodeDegree(tooltip.node.id)}</div>
            </div>
          )}

          {/* Context menu — position is from mouse event, cannot be static CSS */}
          {ctxMenu && (
            <div className="graph-context-menu" style={{ left: ctxMenu.x, top: ctxMenu.y }}>
              <div
                className="context-menu-item"
                onClick={() => { handleOpenInIde(ctxMenu.node.file_path); setCtxMenu(null) }}
              >
                Open in IDE
              </div>
              <div className="context-menu-sep" />
              <div
                className="context-menu-item"
                onClick={() => { setSelectedNode(ctxMenu.node.id); zoomToNode(ctxMenu.node.id); setCtxMenu(null) }}
              >
                Focus node
              </div>
              <div
                className="context-menu-item"
                onClick={() => { navigator.clipboard.writeText(ctxMenu.node.label); setCtxMenu(null) }}
              >
                Copy name
              </div>
              <div
                className="context-menu-item"
                onClick={() => { navigator.clipboard.writeText(ctxMenu.node.file_path); setCtxMenu(null) }}
              >
                Copy file path
              </div>
            </div>
          )}
        </div>

        {/* ── Right sidebar ── */}
        <div className="graph-sidebar">

          {/* Node Inspector */}
          {activeNode && (
            <div className="sidebar-section">
              <div className="sidebar-section-header" onClick={() => setInspOpen(!inspOpen)}>
                <span className="sidebar-section-title">NODE INSPECTOR</span>
                <span className="sidebar-section-caret">{inspOpen ? '▾' : '▸'}</span>
              </div>
              {inspOpen && (
                <div className="sidebar-section-body">
                  <div className="node-inspector">
                    <div className="node-inspector-header">{activeNode.label}</div>
                    <div className="node-inspector-meta">
                      <span className="node-inspector-badge badge-kind">{activeNode.kind}</span>
                      <span className="node-inspector-badge badge-lang">{activeNode.language}</span>
                      {activeNode.community !== undefined && (
                        <span
                          className="node-inspector-badge badge-community"
                          style={{
                            /* community color is runtime-indexed — cannot be static CSS */
                            background: COMMUNITY_COLORS[activeNode.community % COMMUNITY_COLORS.length] + '33',
                            color: COMMUNITY_COLORS[activeNode.community % COMMUNITY_COLORS.length],
                          }}
                        >
                          C-{activeNode.community}
                        </span>
                      )}
                    </div>
                    <div
                      className="node-inspector-path"
                      onClick={() => handleOpenInIde(activeNode.file_path)}
                      title="Click to open in IDE"
                    >
                      {activeNode.file_path}
                    </div>
                    <div className="node-inspector-stat">degree: <span>{getNodeDegree(activeNode.id)}</span></div>

                    {/* Incoming edges */}
                    {inEdges.length > 0 && (
                      <div className="node-inspector-section">
                        <div className="node-inspector-section-title">INCOMING ({inEdges.length})</div>
                        <div className="node-inspector-edge-list">
                          {shownIn.map((e, i) => {
                            const srcId = typeof e.source === 'string' ? e.source : (e.source as VisualizationNode).id
                            const srcNode = graphData!.nodes.find(n => n.id === srcId)
                            const col = EDGE_COLORS[e.kind] || '#888888'
                            return (
                              <div key={i} className="node-inspector-edge" onClick={() => setSelectedNode(srcId)}>
                                {/* edge kind badge color is runtime data from EDGE_COLORS */}
                                <span className="node-inspector-edge-kind" style={{ background: col + '22', color: col }}>{e.kind}</span>
                                <span className="node-inspector-edge-target">{srcNode?.label ?? srcId}</span>
                              </div>
                            )
                          })}
                        </div>
                        {inEdges.length > 5 && (
                          <div className="show-all-btn" onClick={() => setShowAllIn(!showAllIn)}>
                            {showAllIn ? 'Show less' : `+${inEdges.length - 5} more`}
                          </div>
                        )}
                      </div>
                    )}

                    {/* Outgoing edges */}
                    {outEdges.length > 0 && (
                      <div className="node-inspector-section">
                        <div className="node-inspector-section-title">OUTGOING ({outEdges.length})</div>
                        <div className="node-inspector-edge-list">
                          {shownOut.map((e, i) => {
                            const tgtId = typeof e.target === 'string' ? e.target : (e.target as VisualizationNode).id
                            const tgtNode = graphData!.nodes.find(n => n.id === tgtId)
                            const col = EDGE_COLORS[e.kind] || '#888888'
                            return (
                              <div key={i} className="node-inspector-edge" onClick={() => setSelectedNode(tgtId)}>
                                {/* edge kind badge color is runtime data from EDGE_COLORS */}
                                <span className="node-inspector-edge-kind" style={{ background: col + '22', color: col }}>{e.kind}</span>
                                <span className="node-inspector-edge-target">{tgtNode?.label ?? tgtId}</span>
                              </div>
                            )
                          })}
                        </div>
                        {outEdges.length > 5 && (
                          <div className="show-all-btn" onClick={() => setShowAllOut(!showAllOut)}>
                            {showAllOut ? 'Show less' : `+${outEdges.length - 5} more`}
                          </div>
                        )}
                      </div>
                    )}

                    <div className="node-inspector-actions">
                      <button className="btn-small" onClick={() => setSelectedNode(null)}>Deselect</button>
                      <button className="btn-small" onClick={() => handleOpenInIde(activeNode.file_path)}>Open File</button>
                      <button className="btn-small" onClick={() => zoomToNode(activeNode.id)}>Zoom To</button>
                    </div>
                  </div>
                </div>
              )}
            </div>
          )}

          {/* Communities */}
          <div className="sidebar-section">
            <div className="sidebar-section-header" onClick={() => setCommOpen(!commOpen)}>
              <span className="sidebar-section-title">COMMUNITIES ({communities.length})</span>
              <span className="sidebar-section-caret">{commOpen ? '▾' : '▸'}</span>
            </div>
            {commOpen && (
              <div className="sidebar-section-body">
                <div className="communities-list">
                  {communities.map((c, i) => (
                    <div
                      key={c.id}
                      className={`community-item${selectedCommunity === c.id ? ' selected' : ''}`}
                      onClick={() => setSelectedCommunity(prev => prev === c.id ? null : c.id)}
                      role="button"
                      tabIndex={0}
                    >
                      {/* community swatch color is runtime-indexed — cannot be static CSS */}
                      <div className="community-swatch" style={{ background: COMMUNITY_COLORS[i % COMMUNITY_COLORS.length] }} />
                      <span className="community-name">{c.name}</span>
                      <span className="community-count">{c.members.length}</span>
                      {/* cohesion color is runtime-computed — cannot be static CSS */}
                      <span className="community-cohesion" style={{ color: cohesionColor(c.cohesion_score) }}>
                        {c.cohesion_score.toFixed(2)}{c.cohesion_score < 0.2 ? ' ⚠' : ''}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>

          {/* Analysis */}
          <div className="sidebar-section">
            <div className="sidebar-section-header">
              <span className="sidebar-section-title">ANALYSIS</span>
            </div>
            <div className="sidebar-section-body">
              <div className="analysis-btns">
                <button className="btn-small">🔍 God Nodes</button>
                <button className="btn-small">⚡ Connections</button>
                <button className="btn-small">❓ Questions</button>
                <button className="btn-small">📋 Report</button>
              </div>
            </div>
          </div>

        </div>
      </div>
    </div>
  )
}
