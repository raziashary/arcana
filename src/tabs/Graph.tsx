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

export default function GraphTab() {
  const workspaceDir = useWorkspaceStore(s => s.workspaceDir)
  const loadWorkspaceDir = useWorkspaceStore(s => s.loadWorkspaceDir)
  const svgRef = useRef<SVGSVGElement>(null)
  const [stats, setStats] = useState<GraphStats | null>(null)
  const [communities, setCommunities] = useState<Community[]>([])
  const [isBuilding, setIsBuilding] = useState(false)
  const [selectedCommunity, setSelectedCommunity] = useState<number | null>(null)
  const [renderMode, setRenderMode] = useState<'full' | 'community' | 'auto'>('auto')
  const [graphData, setGraphData] = useState<GraphVisualizationData | null>(null)
  const transformRef = useRef({ scale: 1, translateX: 0, translateY: 0, isDragging: false, dragStartX: 0, dragStartY: 0 })

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

  // Render graph whenever data or mode changes
  useEffect(() => {
    if (graphData && svgRef.current) {
      renderGraph()
      const cleanup = setupInteractions()
      return cleanup
    }
  }, [graphData, renderMode, selectedCommunity])

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
    try {
      console.log('[Graph] Loading graph stats with workspaceDir:', workspaceDir)
      const graphStats = await invoke<GraphStats>('get_graph_stats', { repoPath: workspaceDir })
      console.log('[Graph] Stats loaded:', graphStats)
      setStats(graphStats)
      
      // Fetch visualization data (no params needed - uses home dir)
      console.log('[Graph] Fetching visualization data...')
      const vizData = await invoke<GraphVisualizationData>('get_graph_visualization_data')
      console.log('[Graph] Visualization data loaded:', vizData)
      setGraphData(vizData)
      setCommunities(vizData.communities)
    } catch (error) {
      console.error('[Graph] Failed to load graph stats:', error)
    }
  }

  const renderGraph = () => {
    if (!graphData || !svgRef.current) return

    const svg = svgRef.current
    const width = svg.clientWidth || 800
    const height = svg.clientHeight || 600

    console.log(`[Graph] Rendering graph: ${width}x${height}, nodes: ${graphData.nodes.length}, edges: ${graphData.edges.length}`)

    // Clear previous content
    while (svg.firstChild) {
      svg.removeChild(svg.firstChild)
    }

    // Filter nodes/edges based on render mode
    let nodesToRender = graphData.nodes.map(n => ({ ...n }))
    let edgesToRender = graphData.edges

    if (renderMode === 'community' && selectedCommunity !== null) {
      const communityNodeIds = new Set(
        graphData.communities
          .find(c => c.id === selectedCommunity)
          ?.members || []
      )
      nodesToRender = graphData.nodes.filter(n => communityNodeIds.has(n.id)).map(n => ({ ...n }))
      edgesToRender = graphData.edges.filter(
        e => communityNodeIds.has(typeof e.source === 'string' ? e.source : e.source.id) &&
             communityNodeIds.has(typeof e.target === 'string' ? e.target : e.target.id)
      )
    }

    // Smart layout: grid when sparse/no edges, force-directed when connected
    const centerX = width / 2
    const centerY = height / 2
    const numNodes = nodesToRender.length
    const hasEdges = edgesToRender.length > 0

    if (!hasEdges) {
      // GRID LAYOUT for disconnected nodes — uses space efficiently
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
      // FORCE-DIRECTED LAYOUT for connected graphs
      // Initialize on a circle, then run simulation
      const radius = Math.min(width, height) * 0.35
      nodesToRender.forEach((node, i) => {
        const angle = (i / numNodes) * 2 * Math.PI
        node.x = centerX + radius * Math.cos(angle)
        node.y = centerY + radius * Math.sin(angle)
        node.vx = 0
        node.vy = 0
      })

      // Build adjacency for fast lookup
      const idToNode = new Map(nodesToRender.map(n => [n.id, n]))
      const k = Math.sqrt((width * height) / numNodes) * 0.3
      const iterations = 80

      for (let iter = 0; iter < iterations; iter++) {
        const cooling = 1 - iter / iterations
        // Reset forces
        nodesToRender.forEach(n => { n.fx = 0; n.fy = 0 })

        // Repulsion (all pairs)
        for (let i = 0; i < numNodes; i++) {
          for (let j = i + 1; j < numNodes; j++) {
            const a = nodesToRender[i], b = nodesToRender[j]
            const dx = (b.x! - a.x!) || 0.01
            const dy = (b.y! - a.y!) || 0.01
            const d2 = dx * dx + dy * dy
            const d = Math.sqrt(d2)
            const f = (k * k) / d
            const fx = (dx / d) * f
            const fy = (dy / d) * f
            a.fx! -= fx; a.fy! -= fy
            b.fx! += fx; b.fy! += fy
          }
        }

        // Attraction (edges)
        edgesToRender.forEach(edge => {
          const sId = typeof edge.source === 'string' ? edge.source : edge.source.id
          const tId = typeof edge.target === 'string' ? edge.target : edge.target.id
          const a = idToNode.get(sId), b = idToNode.get(tId)
          if (!a || !b) return
          const dx = b.x! - a.x!
          const dy = b.y! - a.y!
          const d = Math.sqrt(dx * dx + dy * dy) || 1
          const f = (d * d) / k
          const fx = (dx / d) * f
          const fy = (dy / d) * f
          a.fx! += fx; a.fy! += fy
          b.fx! -= fx; b.fy! -= fy
        })

        // Apply forces with cooling
        nodesToRender.forEach(n => {
          const fLen = Math.sqrt(n.fx! * n.fx! + n.fy! * n.fy!) || 1
          const limit = Math.min(fLen, 30 * cooling)
          n.x = Math.max(40, Math.min(width - 40, n.x! + (n.fx! / fLen) * limit))
          n.y = Math.max(40, Math.min(height - 40, n.y! + (n.fy! / fLen) * limit))
        })
      }
    }

    // Community colors (Winter is Coming theme)
    const communityColors: Record<number, string> = {}
    const colors = [THEME.aqua, THEME.accent, THEME.warning, THEME.success, THEME.danger, '#C792EA', '#7FDBCA', '#82AAFF']
    graphData.communities.forEach((c, i) => {
      communityColors[c.id] = colors[i % colors.length]
    })

    // Create main group for zoom/pan
    const mainGroup = document.createElementNS('http://www.w3.org/2000/svg', 'g')
    mainGroup.setAttribute('id', 'main-group')
    svg.appendChild(mainGroup)

    // Create SVG elements for edges
    const edgeGroup = document.createElementNS('http://www.w3.org/2000/svg', 'g')
    edgeGroup.setAttribute('class', 'edges')
    mainGroup.appendChild(edgeGroup)

    edgesToRender.forEach(edge => {
      const sourceId = typeof edge.source === 'string' ? edge.source : edge.source.id
      const targetId = typeof edge.target === 'string' ? edge.target : edge.target.id
      const sourceNode = nodesToRender.find(n => n.id === sourceId)
      const targetNode = nodesToRender.find(n => n.id === targetId)

      if (sourceNode && targetNode) {
        const line = document.createElementNS('http://www.w3.org/2000/svg', 'line')
        line.setAttribute('x1', String(sourceNode.x || centerX))
        line.setAttribute('y1', String(sourceNode.y || centerY))
        line.setAttribute('x2', String(targetNode.x || centerX))
        line.setAttribute('y2', String(targetNode.y || centerY))
        line.setAttribute('stroke', THEME.accent)
        line.setAttribute('stroke-width', String(Math.max(1, edge.weight || 1.5)))
        line.setAttribute('opacity', '0.6')
        edgeGroup.appendChild(line)
      }
    })

    // Create SVG elements for nodes
    const nodeGroup = document.createElementNS('http://www.w3.org/2000/svg', 'g')
    nodeGroup.setAttribute('class', 'nodes')
    mainGroup.appendChild(nodeGroup)

    nodesToRender.forEach((node) => {
      const x = node.x || centerX
      const y = node.y || centerY

      const circle = document.createElementNS('http://www.w3.org/2000/svg', 'circle')
      circle.setAttribute('cx', String(x))
      circle.setAttribute('cy', String(y))
      circle.setAttribute('r', String(node.size || 10))
      circle.setAttribute('fill', node.community ? communityColors[node.community] : THEME.aqua)
      circle.setAttribute('opacity', '0.9')
      circle.setAttribute('stroke', THEME.text)
      circle.setAttribute('stroke-width', '2')
      circle.setAttribute('class', 'graph-node')
      circle.setAttribute('title', `${node.label}\n${node.language}\n${node.file_path}`)
      circle.style.cursor = 'pointer'
      nodeGroup.appendChild(circle)

      // Add label for larger nodes
      if (node.size > 12) {
        const text = document.createElementNS('http://www.w3.org/2000/svg', 'text')
        text.setAttribute('x', String(x))
        text.setAttribute('y', String(y))
        text.setAttribute('text-anchor', 'middle')
        text.setAttribute('dy', '0.3em')
        text.setAttribute('font-size', '10')
        text.setAttribute('fill', THEME.bg)
        text.setAttribute('font-weight', 'bold')
        text.setAttribute('pointer-events', 'none')
        text.textContent = node.label.substring(0, 8)
        nodeGroup.appendChild(text)
      }
    })

    // Apply initial transform
    const transform = transformRef.current
    mainGroup.setAttribute('transform', `translate(${transform.translateX},${transform.translateY}) scale(${transform.scale})`)
  }

  const handleBuildGraph = async () => {
    console.log('[Graph] Build clicked, workspaceDir:', workspaceDir)
    if (!workspaceDir) {
      alert('❌ Please set a workspace directory in Settings first!\n\nGo to Settings tab → Workspace section → Pick Directory')
      return
    }
    setIsBuilding(true)
    try {
      console.log('[Graph] Building graph for:', workspaceDir)
      await invoke('build_graph', { repoPath: workspaceDir })
      await loadGraphStats()
    } catch (error) {
      console.error('Failed to build graph:', error)
      alert(`Failed to build graph: ${error}`)
    } finally {
      setIsBuilding(false)
    }
  }

  const handleUpdateGraph = async () => {
    if (!workspaceDir) {
      alert('Please set a workspace directory in Settings first')
      return
    }
    setIsBuilding(true)
    try {
      await invoke('update_graph', { repoPath: workspaceDir })
      await loadGraphStats()
    } catch (error) {
      console.error('Failed to update graph:', error)
      alert(`Failed to update graph: ${error}`)
    } finally {
      setIsBuilding(false)
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
            />
          ) : (
            <div className="canvas-placeholder">
              <p>📈 Graph Visualization</p>
              <p className="canvas-placeholder-text">
                Build a graph to see the visualization
              </p>
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
                  onClick={() => setSelectedCommunity(community.id)}
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
