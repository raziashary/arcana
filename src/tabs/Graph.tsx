// Graph tab — code knowledge graph visualization and analysis

import { useEffect, useState, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useWorkspaceStore } from '../stores/workspaceStore'
import './Graph.css'

// Winter is Coming theme colors
const THEME = {
  bg: '#011627',
  panel: '#00233B',
  card: '#01111D',
  border: '#0D3652',
  text: '#D6DEEB',
  textSecondary: '#637777',
  accent: '#82AAFF',
  aqua: '#7FDBCA',
  success: '#ADDB67',
  warning: '#FFCB8B',
  danger: '#FF6363',
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
}

interface BuildProgress {
  phase: string
  message: string
  percent: number
}

export default function GraphTab() {
  const workspaceDir = useWorkspaceStore(s => s.workspaceDir)
  const loadWorkspaceDir = useWorkspaceStore(s => s.loadWorkspaceDir)
  const svgRef = useRef<SVGSVGElement>(null)
  const [stats, setStats] = useState<GraphStats | null>(null)
  const [communities, setCommunities] = useState<Community[]>([])
  const [isBuilding, setIsBuilding] = useState(false)
  const [buildProgress, setBuildProgress] = useState<BuildProgress | null>(null)
  const [selectedCommunity, setSelectedCommunity] = useState<number | null>(null)
  const [renderMode, setRenderMode] = useState<'full' | 'community' | 'auto'>('auto')
  const [graphData, setGraphData] = useState<GraphVisualizationData | null>(null)
  const [selectedNode, setSelectedNode] = useState<string | null>(null)
  const [isLoadingStats, setIsLoadingStats] = useState(false)
  const transformRef = useRef({ scale: 1, translateX: 0, translateY: 0, isDragging: false, dragStartX: 0, dragStartY: 0 })
  const selectedNodeRef = useRef<string | null>(null)

  // Keep ref in sync with state so event handlers always see latest value
  useEffect(() => { selectedNodeRef.current = selectedNode }, [selectedNode])

  useEffect(() => {
    // Load workspace directory from storage on mount
    loadWorkspaceDir()
  }, [loadWorkspaceDir])

  useEffect(() => {
    loadGraphStats()
    
    // Listen for graph updates
    const unsubscribe = listen('graph:updated', () => {
      loadGraphStats()
    })
    
    return () => {
      unsubscribe.then(fn => fn())
    }
  }, [])

  useEffect(() => {
    // Listen for build progress events
    const unsubscribe = listen<BuildProgress>('graph:build_progress', (event) => {
      setBuildProgress(event.payload)
      if (event.payload.phase === 'done') {
        setIsBuilding(false)
        setTimeout(() => setBuildProgress(null), 800)
      }
    })
    
    return () => {
      unsubscribe.then(fn => fn())
    }
  }, [])

  // Setup zoom/pan interactions once on mount — never re-register on selection changes
  useEffect(() => {
    if (!svgRef.current) return
    const cleanup = setupInteractions()
    return cleanup
  }, [])

  // Full graph rebuild when data, render mode, or community filter changes
  useEffect(() => {
    if (graphData && svgRef.current) {
      renderGraph()
    }
  }, [graphData, renderMode, selectedCommunity])

  // Fast highlight-only update when selected node changes (no DOM rebuild)
  useEffect(() => {
    updateNodeHighlights()
  }, [selectedNode])

  const resetView = () => {
    transformRef.current = { scale: 1, translateX: 0, translateY: 0, isDragging: false, dragStartX: 0, dragStartY: 0 }
    applyTransform()
  }

  const applyTransform = () => {
    if (!svgRef.current) return
    const mainGroup = svgRef.current.querySelector('#main-group') as SVGGElement | null
    if (!mainGroup) return
    const t = transformRef.current
    mainGroup.setAttribute('transform', `translate(${t.translateX} ${t.translateY}) scale(${t.scale})`)
  }

  const setupInteractions = () => {
    if (!svgRef.current) return
    const svg = svgRef.current
    const t = transformRef.current

    const handleWheel = (e: WheelEvent) => {
      e.preventDefault()
      e.stopPropagation()
      const rect = svg.getBoundingClientRect()
      const mx = e.clientX - rect.left
      const my = e.clientY - rect.top

      // Smooth zoom factor
      const factor = Math.exp(-e.deltaY * 0.0015)
      const newScale = Math.max(0.1, Math.min(10, t.scale * factor))
      const ratio = newScale / t.scale

      // Zoom toward cursor
      t.translateX = mx - (mx - t.translateX) * ratio
      t.translateY = my - (my - t.translateY) * ratio
      t.scale = newScale

      applyTransform()
    }

    const handleMouseDown = (e: MouseEvent) => {
      if (e.button !== 0) return
      t.isDragging = true
      t.dragStartX = e.clientX - t.translateX
      t.dragStartY = e.clientY - t.translateY
      svg.style.cursor = 'grabbing'
    }

    const handleMouseMove = (e: MouseEvent) => {
      if (!t.isDragging) return
      t.translateX = e.clientX - t.dragStartX
      t.translateY = e.clientY - t.dragStartY
      applyTransform()
    }

    const handleMouseUp = () => {
      t.isDragging = false
      svg.style.cursor = 'grab'
    }

    svg.style.cursor = 'grab'
    svg.addEventListener('wheel', handleWheel, { passive: false })
    svg.addEventListener('mousedown', handleMouseDown)
    window.addEventListener('mousemove', handleMouseMove)
    window.addEventListener('mouseup', handleMouseUp)

    return () => {
      svg.removeEventListener('wheel', handleWheel)
      svg.removeEventListener('mousedown', handleMouseDown)
      window.removeEventListener('mousemove', handleMouseMove)
      window.removeEventListener('mouseup', handleMouseUp)
    }
  }

  const loadGraphStats = async () => {
    if (isLoadingStats) return
    setIsLoadingStats(true)
    try {
      console.log('[Graph] Loading graph stats with workspaceDir:', workspaceDir)
      const graphStats = await invoke<GraphStats>('get_graph_stats', { repoPath: workspaceDir })
      console.log('[Graph] Stats loaded:', graphStats)
      setStats(graphStats)
      
      // Only fetch visualization data if there are nodes to render
      if (graphStats.total_nodes > 0) {
        console.log('[Graph] Fetching visualization data...')
        const vizData = await invoke<GraphVisualizationData>('get_graph_visualization_data')
        console.log('[Graph] Visualization data loaded:', vizData)
        setGraphData(vizData)
        setCommunities(vizData.communities)
      } else {
        console.log('[Graph] No nodes in graph, skipping visualization fetch')
        setGraphData(null)
        setCommunities([])
      }
    } catch (error) {
      console.error('[Graph] Failed to load graph stats:', error)
    } finally {
      setIsLoadingStats(false)
    }
  }

  const renderGraph = () => {
    if (!graphData || !svgRef.current) return

    const svg = svgRef.current
    const width = svg.clientWidth || 800
    const height = svg.clientHeight || 600

    // Set viewBox for proper zoom/pan
    svg.setAttribute('viewBox', `0 0 ${width} ${height}`)

    console.log(`[Graph] Rendering graph: ${width}x${height}, nodes: ${graphData.nodes.length}, edges: ${graphData.edges.length}`)

    // Clear previous content
    while (svg.firstChild) {
      svg.removeChild(svg.firstChild)
    }

    const centerX = width / 2
    const centerY = height / 2

    // Compute degrees (centrality) from FULL graph for sizing
    const degreeMap = new Map<string, number>()
    graphData.edges.forEach(edge => {
      const sId = typeof edge.source === 'string' ? edge.source : edge.source.id
      const tId = typeof edge.target === 'string' ? edge.target : edge.target.id
      degreeMap.set(sId, (degreeMap.get(sId) || 0) + 1)
      degreeMap.set(tId, (degreeMap.get(tId) || 0) + 1)
    })

    // Active community node set (for fading)
    const activeCommunityNodeIds = new Set<string>()
    if (selectedCommunity !== null) {
      const comm = graphData.communities.find(c => c.id === selectedCommunity)
      if (comm) comm.members.forEach(m => activeCommunityNodeIds.add(m))
    }
    const isCommunityMode = selectedCommunity !== null

    // All nodes rendered — community mode fades non-members
    const nodesToRender = graphData.nodes.map(n => ({ ...n }))
    const nodeIdSet = new Set(nodesToRender.map(n => n.id))
    const edgesToRender = graphData.edges.filter(e => {
      const sId = typeof e.source === 'string' ? e.source : e.source.id
      const tId = typeof e.target === 'string' ? e.target : e.target.id
      return nodeIdSet.has(sId) && nodeIdSet.has(tId)
    })

    const numNodes = nodesToRender.length
    const hasEdges = edgesToRender.length > 0

    // Build adjacency and degrees for LAYOUT (subset if community mode)
    const idToNode = new Map(nodesToRender.map(n => [n.id, n]))
    const adjacency = new Map<string, string[]>()
    edgesToRender.forEach(edge => {
      const sId = typeof edge.source === 'string' ? edge.source : edge.source.id
      const tId = typeof edge.target === 'string' ? edge.target : edge.target.id
      if (!adjacency.has(sId)) adjacency.set(sId, [])
      if (!adjacency.has(tId)) adjacency.set(tId, [])
      adjacency.get(sId)!.push(tId)
      adjacency.get(tId)!.push(sId)
    })
    const layoutDegrees = new Map(nodesToRender.map(n => [n.id, adjacency.get(n.id)?.length || 0]))
    const maxDegree = Math.max(1, ...layoutDegrees.values())

    // Pre-compute community centers for radial initialization
    const communityCenters = new Map<number, { x: number; y: number }>()
    if (hasEdges) {
      graphData.communities.forEach((c, i) => {
        const angle = (i / Math.max(1, graphData.communities.length)) * 2 * Math.PI
        const radius = Math.min(width, height) * 0.22
        communityCenters.set(c.id, {
          x: centerX + radius * Math.cos(angle),
          y: centerY + radius * Math.sin(angle)
        })
      })
    }

    if (!hasEdges) {
      const cols = Math.ceil(Math.sqrt(numNodes * (width / height)))
      const rows = Math.ceil(numNodes / cols)
      const cellW = width / (cols + 1)
      const cellH = height / (rows + 1)
      nodesToRender.forEach((node, i) => {
        const col = i % cols
        const row = Math.floor(i / cols)
        node.x = cellW * (col + 1)
        node.y = cellH * (row + 1)
        node.vx = 0
        node.vy = 0
      })
    } else {
      // Community-aware radial initialization
      nodesToRender.forEach((node, i) => {
        const deg = layoutDegrees.get(node.id) || 0
        const commCenter = communityCenters.get(node.community || -1)
        const baseRadius = maxDegree > 0 ? (1 - deg / maxDegree) * 100 : 60
        const radius = baseRadius * (0.4 + 0.6 * Math.random())
        const angle = (i / numNodes) * 2 * Math.PI + Math.random() * 0.3
        if (commCenter) {
          node.x = commCenter.x + radius * Math.cos(angle)
          node.y = commCenter.y + radius * Math.sin(angle)
        } else {
          node.x = centerX + radius * Math.cos(angle)
          node.y = centerY + radius * Math.sin(angle)
        }
        node.vx = 0
        node.vy = 0
      })

      const area = width * height
      const k = Math.sqrt(area / numNodes) * 0.7
      const iterations = 300
      const gravityStrength = 0.03

      for (let iter = 0; iter < iterations; iter++) {
        const temperature = 1 - iter / iterations
        nodesToRender.forEach(n => { n.fx = 0; n.fy = 0 })

        // Repulsion with 3x boost between different communities
        for (let i = 0; i < numNodes; i++) {
          for (let j = i + 1; j < numNodes; j++) {
            const a = nodesToRender[i], b = nodesToRender[j]
            const dx = (b.x! - a.x!) || 0.01
            const dy = (b.y! - a.y!) || 0.01
            const d2 = dx * dx + dy * dy
            const d = Math.sqrt(d2)
            let f = (k * k * 2.5) / d2
            if (a.community !== undefined && b.community !== undefined && a.community !== b.community) {
              f *= 3.0
            }
            const fx = (dx / d) * f
            const fy = (dy / d) * f
            a.fx! -= fx; a.fy! -= fy
            b.fx! += fx; b.fy! += fy
          }
        }

        edgesToRender.forEach(edge => {
          const sId = typeof edge.source === 'string' ? edge.source : edge.source.id
          const tId = typeof edge.target === 'string' ? edge.target : edge.target.id
          const a = idToNode.get(sId), b = idToNode.get(tId)
          if (!a || !b) return
          const dx = b.x! - a.x!
          const dy = b.y! - a.y!
          const dist = Math.sqrt(dx * dx + dy * dy) || 1
          const f = (dist - k * 0.5) * 0.04
          const fx = (dx / dist) * f
          const fy = (dy / dist) * f
          a.fx! += fx; a.fy! += fy
          b.fx! -= fx; b.fy! -= fy
        })

        nodesToRender.forEach(n => {
          n.fx! += (centerX - n.x!) * gravityStrength
          n.fy! += (centerY - n.y!) * gravityStrength
        })

        nodesToRender.forEach(n => {
          const fLen = Math.sqrt(n.fx! * n.fx! + n.fy! * n.fy!) || 1
          const limit = Math.min(fLen, 15 * temperature + 2)
          n.x = Math.max(30, Math.min(width - 30, n.x! + (n.fx! / fLen) * limit))
          n.y = Math.max(30, Math.min(height - 30, n.y! + (n.fy! / fLen) * limit))
        })
      }
    }

    // Community colors
    const communityColors: Record<number, string> = {}
    const colors = [THEME.aqua, THEME.accent, THEME.warning, THEME.success, THEME.danger, '#C792EA', '#7FDBCA', '#82AAFF']
    graphData.communities.forEach((c, i) => {
      communityColors[c.id] = colors[i % colors.length]
    })

    // SVG defs: glow filter for selected node
    const defs = document.createElementNS('http://www.w3.org/2000/svg', 'defs')
    const filter = document.createElementNS('http://www.w3.org/2000/svg', 'filter')
    filter.setAttribute('id', 'node-glow')
    filter.setAttribute('x', '-50%')
    filter.setAttribute('y', '-50%')
    filter.setAttribute('width', '200%')
    filter.setAttribute('height', '200%')
    const feGaussianBlur = document.createElementNS('http://www.w3.org/2000/svg', 'feGaussianBlur')
    feGaussianBlur.setAttribute('stdDeviation', '3.5')
    feGaussianBlur.setAttribute('result', 'coloredBlur')
    const feMerge = document.createElementNS('http://www.w3.org/2000/svg', 'feMerge')
    const feMergeNode1 = document.createElementNS('http://www.w3.org/2000/svg', 'feMergeNode')
    feMergeNode1.setAttribute('in', 'coloredBlur')
    const feMergeNode2 = document.createElementNS('http://www.w3.org/2000/svg', 'feMergeNode')
    feMergeNode2.setAttribute('in', 'SourceGraphic')
    feMerge.appendChild(feMergeNode1)
    feMerge.appendChild(feMergeNode2)
    filter.appendChild(feGaussianBlur)
    filter.appendChild(feMerge)
    defs.appendChild(filter)
    svg.appendChild(defs)

    const mainGroup = document.createElementNS('http://www.w3.org/2000/svg', 'g')
    mainGroup.setAttribute('id', 'main-group')
    svg.appendChild(mainGroup)

    // Edge group
    const edgeGroup = document.createElementNS('http://www.w3.org/2000/svg', 'g')
    edgeGroup.setAttribute('class', 'edges')
    mainGroup.appendChild(edgeGroup)

    edgesToRender.forEach(edge => {
      const sourceId = typeof edge.source === 'string' ? edge.source : edge.source.id
      const targetId = typeof edge.target === 'string' ? edge.target : edge.target.id
      const sourceNode = nodesToRender.find(n => n.id === sourceId)
      const targetNode = nodesToRender.find(n => n.id === targetId)
      if (!sourceNode || !targetNode) return

      const line = document.createElementNS('http://www.w3.org/2000/svg', 'line')
      line.setAttribute('x1', String(sourceNode.x || centerX))
      line.setAttribute('y1', String(sourceNode.y || centerY))
      line.setAttribute('x2', String(targetNode.x || centerX))
      line.setAttribute('y2', String(targetNode.y || centerY))
      line.setAttribute('stroke', THEME.accent)
      line.setAttribute('stroke-width', String(Math.max(0.5, (edge.weight || 1.5) * 0.7)))

      const sActive = activeCommunityNodeIds.has(sourceId)
      const tActive = activeCommunityNodeIds.has(targetId)
      const edgeOpacity = isCommunityMode
        ? (sActive && tActive ? 0.6 : 0.03)
        : 0.25
      line.setAttribute('opacity', String(edgeOpacity))
      edgeGroup.appendChild(line)
    })

    // Node group
    const nodeGroup = document.createElementNS('http://www.w3.org/2000/svg', 'g')
    nodeGroup.setAttribute('class', 'nodes')
    mainGroup.appendChild(nodeGroup)

    // Hover label group (rendered on top)
    const hoverLabelGroup = document.createElementNS('http://www.w3.org/2000/svg', 'g')
    hoverLabelGroup.setAttribute('class', 'hover-labels')
    hoverLabelGroup.setAttribute('pointer-events', 'none')

    // Helper: get node size by kind + centrality
    const getNodeSize = (node: VisualizationNode) => {
      const deg = degreeMap.get(node.id) || 0
      const kind = node.kind || ''
      const baseSize = 3 + Math.sqrt(deg) * 1.8
      if (kind.includes('File')) return baseSize + 6
      if (kind.includes('Class')) return baseSize + 3
      return Math.max(2.5, baseSize)
    }

    nodesToRender.forEach((node) => {
      const x = node.x || centerX
      const y = node.y || centerY
      const sel = selectedNodeRef.current === node.id
      const kind = node.kind || ''
      const isFile = kind.includes('File')
      const isClass = kind.includes('Class')
      const size = getNodeSize(node)
      const deg = degreeMap.get(node.id) || 0

      const isFaded = isCommunityMode && !activeCommunityNodeIds.has(node.id)
      const baseOpacity = isFaded ? 0.12 : (sel ? 1.0 : 0.85)
      const strokeColor = sel ? THEME.warning : THEME.text
      const strokeWidth = sel ? 3 : (isFile ? 2.5 : 1.5)

      let shape: Element
      if (isFile) {
        const rect = document.createElementNS('http://www.w3.org/2000/svg', 'rect')
        rect.setAttribute('x', String(x - size))
        rect.setAttribute('y', String(y - size * 0.7))
        rect.setAttribute('width', String(size * 2))
        rect.setAttribute('height', String(size * 1.4))
        rect.setAttribute('rx', String(size * 0.3))
        shape = rect
      } else if (isClass) {
        const poly = document.createElementNS('http://www.w3.org/2000/svg', 'polygon')
        const s = size * 1.3
        poly.setAttribute('points', `${x},${y - s} ${x + s},${y} ${x},${y + s} ${x - s},${y}`)
        shape = poly
      } else {
        const circle = document.createElementNS('http://www.w3.org/2000/svg', 'circle')
        circle.setAttribute('cx', String(x))
        circle.setAttribute('cy', String(y))
        circle.setAttribute('r', String(Math.max(2, size)))
        shape = circle
      }

      const color = node.community !== undefined ? communityColors[node.community] : THEME.aqua
      shape.setAttribute('fill', color)
      shape.setAttribute('opacity', String(baseOpacity))
      shape.setAttribute('stroke', strokeColor)
      shape.setAttribute('stroke-width', String(strokeWidth))
      shape.setAttribute('class', `graph-node${sel ? ' graph-node-selected' : ''}`)
      if (sel) shape.setAttribute('filter', 'url(#node-glow)')
      ;(shape as any).style.cursor = 'pointer'
      ;(shape as any).dataset.nodeId = node.id

      const title = document.createElementNS('http://www.w3.org/2000/svg', 'title')
      title.textContent = `${node.label} (${node.kind}) — degree: ${deg}\n${node.file_path}`
      shape.appendChild(title)

      shape.addEventListener('click', (e: Event) => {
        e.stopPropagation()
        setSelectedNode(node.id)
      })

      // Hover: highlight + show label
      shape.addEventListener('mouseenter', () => {
        shape.setAttribute('stroke', THEME.warning)
        shape.setAttribute('stroke-width', '3')
        shape.setAttribute('opacity', '1.0')
        if (!isFaded) {
          const label = document.createElementNS('http://www.w3.org/2000/svg', 'text')
          label.setAttribute('x', String(x))
          label.setAttribute('y', String(y - size - 6))
          label.setAttribute('text-anchor', 'middle')
          label.setAttribute('fill', THEME.text)
          label.setAttribute('font-size', '12')
          label.setAttribute('font-weight', 'bold')
          label.setAttribute('class', 'hover-label')
          label.textContent = node.label
          hoverLabelGroup.appendChild(label)
        }
      })

      shape.addEventListener('mouseleave', () => {
        const currentSel = selectedNodeRef.current === node.id
        shape.setAttribute('stroke', currentSel ? THEME.warning : strokeColor)
        shape.setAttribute('stroke-width', String(currentSel ? 3 : strokeWidth))
        shape.setAttribute('opacity', String(currentSel ? 1.0 : baseOpacity))
        while (hoverLabelGroup.firstChild) {
          hoverLabelGroup.removeChild(hoverLabelGroup.firstChild)
        }
      })

      nodeGroup.appendChild(shape)

      // Permanent labels for important nodes
      const showLabel = sel || isFile || deg >= 4
      if (showLabel && !isFaded) {
        const text = document.createElementNS('http://www.w3.org/2000/svg', 'text')
        text.setAttribute('x', String(x))
        text.setAttribute('y', String(y + (isFile ? size * 1.6 : size + 5)))
        text.setAttribute('text-anchor', 'middle')
        text.setAttribute('font-size', sel ? '13' : (isFile ? '11' : '9'))
        text.setAttribute('fill', THEME.text)
        text.setAttribute('font-weight', sel || isFile ? 'bold' : 'normal')
        text.setAttribute('pointer-events', 'none')
        const labelText = isFile ? node.label : node.label.substring(0, 14)
        text.textContent = labelText
        nodeGroup.appendChild(text)
      }
    })

    mainGroup.appendChild(hoverLabelGroup)

    // Apply transform
    const transform = transformRef.current
    mainGroup.setAttribute('transform', `translate(${transform.translateX},${transform.translateY}) scale(${transform.scale})`)
  }

  // Fast DOM-only highlight update — called when selectedNode changes without rebuilding graph
  const updateNodeHighlights = () => {
    if (!svgRef.current || !graphData) return
    const svg = svgRef.current

    // Remove old selected label
    const oldLabel = svg.querySelector('#selected-node-label')
    if (oldLabel) oldLabel.remove()

    svg.querySelectorAll('.graph-node').forEach(el => {
      const nodeId = (el as any).dataset?.nodeId
      if (!nodeId) return
      const isSelected = selectedNode === nodeId
      const node = graphData.nodes.find(n => n.id === nodeId)
      if (!node) return

      const kind = node.kind || ''
      const isFile = kind.includes('File')
      const isFaded = selectedCommunity !== null && !(() => {
        const comm = graphData.communities.find(c => c.id === selectedCommunity)
        return comm?.members.includes(nodeId) ?? false
      })()
      const baseOpacity = isFaded ? 0.12 : (isSelected ? 1.0 : 0.85)
      const strokeWidth = isSelected ? 3 : (isFile ? 2.5 : 1.5)
      const strokeColor = isSelected ? THEME.warning : THEME.text

      el.setAttribute('stroke', strokeColor)
      el.setAttribute('stroke-width', String(strokeWidth))
      el.setAttribute('opacity', String(baseOpacity))
      if (isSelected) {
        el.setAttribute('filter', 'url(#node-glow)')
      } else {
        el.removeAttribute('filter')
      }
    })

    // Add selected label
    if (selectedNode) {
      const node = graphData.nodes.find(n => n.id === selectedNode)
      const shape = svg.querySelector(`[data-node-id="${CSS.escape(selectedNode)}"]`)
      if (node && shape) {
        try {
          const bbox = (shape as SVGGraphicsElement).getBBox()
          const cx = bbox.x + bbox.width / 2
          const cy = bbox.y + bbox.height / 2
          const mainGroup = svg.querySelector('#main-group')
          if (mainGroup) {
            const label = document.createElementNS('http://www.w3.org/2000/svg', 'text')
            label.setAttribute('id', 'selected-node-label')
            label.setAttribute('x', String(cx))
            label.setAttribute('y', String(cy - 14))
            label.setAttribute('text-anchor', 'middle')
            label.setAttribute('font-size', '13')
            label.setAttribute('fill', THEME.warning)
            label.setAttribute('font-weight', 'bold')
            label.setAttribute('pointer-events', 'none')
            label.textContent = node.label
            mainGroup.appendChild(label)
          }
        } catch (e) {
          // getBBox may fail if element not yet rendered
        }
      }
    }
  }

  const handleBuildGraph = async () => {
    console.log('[Graph] Build clicked, workspaceDir:', workspaceDir)
    if (!workspaceDir) {
      alert('❌ Please set a workspace directory in Settings first!\n\nGo to Settings tab → Workspace section → Pick Directory')
      return
    }
    setIsBuilding(true)
    setBuildProgress({ phase: 'init', message: 'Starting build...', percent: 5 })
    try {
      console.log('[Graph] Building graph for:', workspaceDir)
      await invoke('build_graph', { repoPath: workspaceDir })
      await loadGraphStats()
    } catch (error) {
      console.error('Failed to build graph:', error)
      alert(`Failed to build graph: ${error}`)
    }
    // isBuilding is cleared by the graph:build_progress listener when phase === 'done'
  }

  const handleUpdateGraph = async () => {
    if (!workspaceDir) {
      alert('Please set a workspace directory in Settings first')
      return
    }
    setIsBuilding(true)
    setBuildProgress({ phase: 'init', message: 'Starting update...', percent: 5 })
    try {
      await invoke('update_graph', { repoPath: workspaceDir })
      await loadGraphStats()
    } catch (error) {
      console.error('Failed to update graph:', error)
      alert(`Failed to update graph: ${error}`)
    }
    // isBuilding is cleared by the graph:build_progress listener when phase === 'done'
  }

  const handleOpenInIde = async (filePath: string, line?: number) => {
    try {
      await invoke('open_in_ide', { file: filePath, line: line || 1 })
    } catch (error) {
      console.error('Failed to open in IDE:', error)
    }
  }

  const handleDeleteGraph = async () => {
    if (!workspaceDir) {
      alert('Please set a workspace directory in Settings first')
      return
    }
    if (!confirm('Are you sure you want to delete the graph? This cannot be undone.')) {
      return
    }
    try {
      await invoke('delete_graph', { repoPath: workspaceDir })
      setStats(null)
      setCommunities([])
      alert('Graph deleted successfully')
    } catch (error) {
      console.error('Failed to delete graph:', error)
      alert(`Failed to delete graph: ${error}`)
    }
  }

  const handleExport = async (format: 'json' | 'html' | 'svg') => {
    if (!workspaceDir) {
      alert('Please set a workspace directory in Settings first')
      return
    }
    try {
      await invoke('export_graph', { repoPath: workspaceDir, format, outputPath: undefined })
    } catch (error) {
      console.error(`Failed to export ${format}:`, error)
      alert(`Failed to export ${format}: ${error}`)
    }
  }

  return (
    <div className="graph-tab">
      <div className="graph-header">
        <div className="graph-header-top">
          <h2>Code Knowledge Graph</h2>
          {workspaceDir && <div className="workspace-path">📁 {workspaceDir}</div>}
        </div>
        <div className="graph-controls">
          <button 
            onClick={handleBuildGraph} 
            disabled={isBuilding}
            className="btn-primary"
          >
            {isBuilding ? '⏳ Building...' : '🔨 Build Graph'}
          </button>
          <button 
            onClick={handleUpdateGraph} 
            disabled={isBuilding}
            className="btn-secondary"
          >
            ↻ Update
          </button>
          <div className="export-dropdown">
            <button className="btn-secondary">📥 Export ▼</button>
            <div className="dropdown-menu">
              <button onClick={() => handleExport('json')}>JSON</button>
              <button onClick={() => handleExport('html')}>HTML</button>
              <button onClick={() => handleExport('svg')}>SVG</button>
            </div>
          </div>
          <button 
            onClick={resetView}
            className="btn-secondary"
            title="Reset zoom and pan"
          >
            🎯 Reset View
          </button>
          <button 
            onClick={handleDeleteGraph}
            className="btn-danger"
            title="Delete the graph database"
          >
            🗑️ Delete
          </button>
        </div>
      </div>

      <div className="graph-stats">
        {stats ? (
          <>
            <div className="stat-item">
              <span className="stat-label">🟢 Nodes:</span>
              <span className="stat-value">{stats.total_nodes}</span>
            </div>
            <div className="stat-item">
              <span className="stat-label">🔗 Edges:</span>
              <span className="stat-value">{stats.total_edges}</span>
            </div>
            <div className="stat-item">
              <span className="stat-label">📦 Communities:</span>
              <span className="stat-value">{stats.total_communities}</span>
            </div>
            <div className="stat-item">
              <span className="stat-label">📊 Avg Degree:</span>
              <span className="stat-value">{stats.average_degree.toFixed(2)}</span>
            </div>
            <div className="stat-item">
              <span className="stat-label">🔲 Density:</span>
              <span className="stat-value">{(stats.graph_density * 100).toFixed(2)}%</span>
            </div>
          </>
        ) : (
          <div className="stat-placeholder">No graph data. Click "Build Graph" to start.</div>
        )}
      </div>

      <div className="graph-content">
        <div className="graph-canvas">
          {graphData && graphData.nodes.length > 0 ? (
            <svg 
              ref={svgRef} 
              className="graph-svg"
              viewBox="0 0 800 600"
              preserveAspectRatio="xMidYMid meet"
            />
          ) : (
            <div className="canvas-placeholder">
              <p>📈 Graph Visualization</p>
              <p className="canvas-placeholder-text">
                Build a graph to see the visualization
              </p>
            </div>
          )}
          {/* Loading overlay */}
          {isBuilding && buildProgress && (
            <div className="graph-loading-overlay">
              <div className="graph-loading-spinner" />
              <div className="graph-loading-label">{buildProgress.message}</div>
              <div className="graph-loading-sub">{buildProgress.phase}</div>
              <div className="graph-loading-bar">
                <div 
                  className="graph-loading-bar-fill" 
                  style={{ width: `${buildProgress.percent}%` }}
                />
              </div>
            </div>
          )}
        </div>

        <div className="graph-sidebar">
          <div className="sidebar-section">
            <h3>Render Mode</h3>
            <select 
              value={renderMode} 
              onChange={(e) => setRenderMode(e.target.value as any)}
              className="select-control"
              aria-label="Graph render mode"
            >
              <option value="auto">Auto</option>
              <option value="full">Full</option>
              <option value="community">Community</option>
            </select>
          </div>

          <div className="sidebar-section">
            <h3>Communities ({communities.length})</h3>
            <div className="communities-list">
              {communities.slice(0, 10).map((community) => (
                <div
                  key={community.id}
                  className={`community-item ${selectedCommunity === community.id ? 'selected' : ''}`}
                  onClick={() => setSelectedCommunity(prev => prev === community.id ? null : community.id)}
                  title={selectedCommunity === community.id ? 'Click to deselect' : 'Click to isolate community'}
                  role="button"
                  tabIndex={0}
                >
                  <div className="community-name">{community.name}</div>
                  <div className="community-meta">
                    <span>{community.members.length} nodes</span>
                    <span className="cohesion-score">cohesion: {community.cohesion_score.toFixed(2)}</span>
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Node Inspector */}
          {selectedNode && graphData && (
            <div className="sidebar-section">
              <h3>Node Inspector</h3>
              {(() => {
                const node = graphData.nodes.find(n => n.id === selectedNode)
                if (!node) return null
                const incoming = graphData.edges.filter(e => {
                  const tId = typeof e.target === 'string' ? e.target : e.target.id
                  return tId === selectedNode
                })
                const outgoing = graphData.edges.filter(e => {
                  const sId = typeof e.source === 'string' ? e.source : e.source.id
                  return sId === selectedNode
                })
                return (
                  <div className="node-inspector">
                    <div className="node-inspector-header">{node.label}</div>
                    <div className="node-inspector-meta">
                      <span className="node-inspector-badge">{node.kind}</span>
                      <span className="node-inspector-badge">{node.language}</span>
                      {node.community !== undefined && (
                        <span className="node-inspector-badge">Comm {node.community}</span>
                      )}
                    </div>
                    <div 
                      className="node-inspector-path"
                      onClick={() => handleOpenInIde(node.file_path)}
                      title="Click to open in IDE"
                    >
                      {node.file_path}
                    </div>
                    {outgoing.length > 0 && (
                      <div className="node-inspector-section">
                        <h4>Outgoing ({outgoing.length})</h4>
                        <div className="node-inspector-edge-list">
                          {outgoing.slice(0, 8).map((e, i) => {
                            const targetId = typeof e.target === 'string' ? e.target : e.target.id
                            const targetNode = graphData.nodes.find(n => n.id === targetId)
                            return (
                              <div 
                                key={i} 
                                className="node-inspector-edge"
                                onClick={() => setSelectedNode(targetId)}
                              >
                                <span className="node-inspector-edge-dir">→</span>
                                {targetNode?.label || targetId}
                              </div>
                            )
                          })}
                        </div>
                      </div>
                    )}
                    {incoming.length > 0 && (
                      <div className="node-inspector-section">
                        <h4>Incoming ({incoming.length})</h4>
                        <div className="node-inspector-edge-list">
                          {incoming.slice(0, 8).map((e, i) => {
                            const sourceId = typeof e.source === 'string' ? e.source : e.source.id
                            const sourceNode = graphData.nodes.find(n => n.id === sourceId)
                            return (
                              <div 
                                key={i} 
                                className="node-inspector-edge"
                                onClick={() => setSelectedNode(sourceId)}
                              >
                                <span className="node-inspector-edge-dir">←</span>
                                {sourceNode?.label || sourceId}
                              </div>
                            )
                          })}
                        </div>
                      </div>
                    )}
                    <div className="node-inspector-actions">
                      <button className="btn-small" onClick={() => setSelectedNode(null)}>
                        Deselect
                      </button>
                      <button className="btn-small" onClick={() => handleOpenInIde(node.file_path)}>
                        Open File
                      </button>
                    </div>
                  </div>
                )
              })()}
            </div>
          )}

          <div className="sidebar-section">
            <h3>Analysis</h3>
            <button className="btn-small">🔍 God Nodes</button>
            <button className="btn-small">⚡ Surprising Connections</button>
            <button className="btn-small">❓ Suggested Questions</button>
            <button className="btn-small">📋 Generate Report</button>
          </div>
        </div>
      </div>
    </div>
  )
}
