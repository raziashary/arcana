// CityView.tsx — 3D Code City using React Three Fiber
import { Suspense, useMemo, useCallback, useState } from 'react'
import { Canvas } from '@react-three/fiber'
import { Stats } from '@react-three/drei'
import { useGraphStore } from '../../stores/graphStore'
import { computeCityLayout } from './layout/cityLayout'
import type { CityLayout } from './layout/layoutTypes'
import { CityCamera } from './CityCamera'
import { CityLighting, CityGround } from './CityScene'
import { DistrictMesh } from './districts/DistrictMesh'
import { DistrictMinimap } from './districts/DistrictMinimap'
import { BuildingInstances } from './buildings/BuildingInstances'
import { GodNodeBuilding } from './buildings/GodNodeBuilding'
import { WorkerLockedBuilding } from './buildings/WorkerLockedBuilding'
import { AggregateRoads } from './roads/GroundRoad'
import { CallsArcs } from './roads/CallsArc'
import { TrafficParticles } from './particles/TrafficParticles'
import { BuildingRaycaster } from './interaction/BuildingRaycaster'
import { BuildingContextMenu } from './interaction/BuildingContextMenu'
import { ImpactOverlay } from './impact/ImpactOverlay'
import { ImpactPanel } from './impact/ImpactPanel'
import { CityPostProcessing } from './CityPostProcessing'

const CITY_SIZE = 180
const GOD_NODE_DEGREE_THRESHOLD = 15

export function CityView() {
  const { graphData, selectedNodeId, setSelectedNode, impactData } = useGraphStore()
  const [showStats, setShowStats] = useState(false)
  const [showCalls, setShowCalls] = useState(false)
  const [contextMenu, setContextMenu] = useState<{ nodeId: string; x: number; y: number } | null>(null)

  // Compute layout from graph data (memoized so it only recomputes when data changes)
  const layout: CityLayout | null = useMemo(() => {
    if (!graphData || !graphData.nodes.length) return null
    return computeCityLayout(
      graphData.communities,
      graphData.nodes,
      graphData.edges,
      CITY_SIZE
    )
  }, [graphData])

  // Compute degree maps for building dimensions
  const degreeMap = useMemo(() => {
    const inDeg = new Map<string, number>()
    const outDeg = new Map<string, number>()
    if (!graphData) return { inDeg, outDeg }
    graphData.edges.forEach(e => {
      const s = typeof e.source === 'string' ? e.source : (e.source as any).id
      const t = typeof e.target === 'string' ? e.target : (e.target as any).id
      outDeg.set(s, (outDeg.get(s) || 0) + 1)
      inDeg.set(t, (inDeg.get(t) || 0) + 1)
    })
    return { inDeg, outDeg }
  }, [graphData])

  // Separate god nodes from regular nodes
  const { godNodes, regularNodes } = useMemo(() => {
    if (!graphData) return { godNodes: [], regularNodes: [] }
    const god: typeof graphData.nodes = []
    const regular: typeof graphData.nodes = []
    graphData.nodes.forEach(n => {
      const deg = (degreeMap.inDeg.get(n.id) || 0) + (degreeMap.outDeg.get(n.id) || 0)
      if (deg >= GOD_NODE_DEGREE_THRESHOLD) god.push(n)
      else regular.push(n)
    })
    return { godNodes: god, regularNodes: regular }
  }, [graphData, degreeMap])

  const handleBuildingClick = useCallback((nodeId: string) => {
    setSelectedNode(selectedNodeId === nodeId ? null : nodeId)
    setContextMenu(null)
  }, [selectedNodeId, setSelectedNode])

  const handleBuildingRightClick = useCallback((nodeId: string, x: number, y: number) => {
    setContextMenu({ nodeId, x, y })
  }, [])

  if (!graphData || !layout) {
    return (
      <div style={{
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        height: '100%', color: '#555', flexDirection: 'column', gap: 12
      }}>
        <div style={{ fontSize: 48 }}>🏙</div>
        <div style={{ fontSize: 14 }}>Build a graph first to see the 3D city</div>
      </div>
    )
  }

  return (
    <div style={{ position: 'relative', width: '100%', height: '100%' }}>
      <Canvas
        shadows
        camera={{ fov: 45, near: 0.1, far: 1000 }}
        gl={{ antialias: true, alpha: false }}
        style={{ background: '#050508' }}
        onPointerMissed={() => { setSelectedNode(null); setContextMenu(null) }}
      >
        <Suspense fallback={null}>
          <CityCamera citySize={CITY_SIZE} />
          <CityLighting />
          <CityGround size={CITY_SIZE} />

          {/* Districts (community ground planes) */}
          {layout.districts.map(district => (
            <DistrictMesh key={district.communityId} district={district} />
          ))}

          {/* Regular buildings (instanced for performance) */}
          <BuildingInstances
            nodes={regularNodes}
            layout={layout}
            degreeMap={degreeMap}
            selectedNodeId={selectedNodeId}
            impactData={impactData}
          />

          {/* God node landmarks (individual meshes) */}
          {godNodes.map(node => {
            const bl = layout.buildingPositions.get(node.id)
            if (!bl) return null
            return (
              <GodNodeBuilding
                key={node.id}
                node={node}
                layout={bl}
                isSelected={selectedNodeId === node.id}
                impactData={impactData}
              />
            )
          })}

          {/* Worker-locked buildings overlay */}
          {graphData.nodes
            .filter(n => n.kind === 'worker_locked')
            .map(node => {
              const bl = layout.buildingPositions.get(node.id)
              if (!bl) return null
              return <WorkerLockedBuilding key={node.id} layout={bl} />
            })
          }

          {/* Aggregate import roads (between districts) */}
          <AggregateRoads
            edges={graphData.edges}
            nodes={graphData.nodes}
            layout={layout}
          />

          {/* CALLS arcs — visible when a node is selected or showCalls is on */}
          {(showCalls || selectedNodeId) && (
            <CallsArcs
              edges={graphData.edges}
              nodes={graphData.nodes}
              layout={layout}
              focusNodeId={selectedNodeId}
            />
          )}

          {/* Traffic particles */}
          <TrafficParticles
            edges={graphData.edges}
            nodes={graphData.nodes}
            layout={layout}
          />

          {/* Building raycaster — handles click/hover detection on instanced meshes */}
          <BuildingRaycaster
            nodes={regularNodes}
            layout={layout}
            onBuildingClick={handleBuildingClick}
            onBuildingRightClick={handleBuildingRightClick}
          />

          {/* Impact mode overlay */}
          {impactData && (
            <ImpactOverlay
              nodes={graphData.nodes}
              layout={layout}
              impactData={impactData}
            />
          )}

          {/* Post-processing (bloom, SSAO, vignette) */}
          <CityPostProcessing />

          {showStats && <Stats />}
        </Suspense>
      </Canvas>

      {/* HTML overlays */}
      <DistrictMinimap layout={layout} citySize={CITY_SIZE} />

      {contextMenu && (
        <BuildingContextMenu
          nodeId={contextMenu.nodeId}
          nodes={graphData.nodes}
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={() => setContextMenu(null)}
        />
      )}

      {impactData && <ImpactPanel impactData={impactData} nodes={graphData.nodes} />}

      {/* City toolbar overlays */}
      <div style={{
        position: 'absolute', top: 8, right: 8, display: 'flex', gap: 6,
        zIndex: 10,
      }}>
        <button
          onClick={() => setShowCalls(v => !v)}
          style={{
            background: showCalls ? 'rgba(0,255,136,0.2)' : 'rgba(0,0,0,0.6)',
            border: `1px solid ${showCalls ? '#00ff88' : 'rgba(255,255,255,0.15)'}`,
            color: showCalls ? '#00ff88' : '#888',
            padding: '4px 10px', borderRadius: 4, cursor: 'pointer', fontSize: 11,
          }}
          title="Toggle CALLS arcs"
        >
          ⚡ CALLS
        </button>
        <button
          onClick={() => setShowStats(v => !v)}
          style={{
            background: 'rgba(0,0,0,0.6)',
            border: '1px solid rgba(255,255,255,0.15)',
            color: '#888', padding: '4px 10px', borderRadius: 4, cursor: 'pointer', fontSize: 11,
          }}
          title="Toggle FPS stats"
        >
          FPS
        </button>
      </div>

      {/* Legend */}
      <div style={{
        position: 'absolute', bottom: 12, left: 12,
        background: 'rgba(0,0,0,0.7)', border: '1px solid rgba(255,255,255,0.08)',
        borderRadius: 6, padding: '8px 12px', fontSize: 10, color: '#666', zIndex: 10,
      }}>
        <div style={{ color: '#888', marginBottom: 4, fontWeight: 600 }}>KEYBOARD</div>
        <div>0 — Overview &nbsp; G — Street level &nbsp; T — Top down</div>
        <div style={{ marginTop: 3 }}>🖱 Drag pan · Scroll zoom · Right-drag orbit</div>
      </div>
    </div>
  )
}
