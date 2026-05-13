// ImpactPanel.tsx — Blast radius emergency side panel (HTML overlay)
import { useMemo } from 'react'
import type { GraphNode, ImpactData, ImpactRisk } from '../../../stores/graphStore'
import { useGraphStore } from '../../../stores/graphStore'

interface ImpactPanelProps {
  impactData: ImpactData
  nodes: GraphNode[]
}

function riskBucket(data: ImpactData): Record<ImpactRisk, string[]> {
  const buckets: Record<ImpactRisk, string[]> = {
    epicenter: [data.epicenterId],
    high: [],
    medium: [],
    low: [],
    test: [...data.testNodes],
    untested: [...data.untestedNodes],
    ghost: [],
  }

  data.affected.forEach(({ risk }, nodeId) => {
    if (risk !== 'ghost') buckets[risk].push(nodeId)
  })

  return buckets
}

export function ImpactPanel({ impactData, nodes }: ImpactPanelProps) {
  const { setImpactData } = useGraphStore()
  const nodeMap = useMemo(() => {
    const m = new Map<string, GraphNode>()
    nodes.forEach(n => m.set(n.id, n))
    return m
  }, [nodes])

  const buckets = useMemo(() => riskBucket(impactData), [impactData])
  const epicenterNode = nodeMap.get(impactData.epicenterId)
  const epicenterLabel = epicenterNode?.label || impactData.epicenterId.split('::').pop() || impactData.epicenterId

  const panelStyle: React.CSSProperties = {
    position: 'absolute',
    top: 48,
    right: 8,
    width: 260,
    background: 'rgba(10, 5, 5, 0.94)',
    border: '1px solid rgba(255, 50, 50, 0.4)',
    borderRadius: 6,
    overflow: 'hidden',
    zIndex: 20,
    fontFamily: 'monospace',
    fontSize: 11,
  }

  const headerStyle: React.CSSProperties = {
    background: 'rgba(255, 40, 40, 0.15)',
    borderBottom: '1px solid rgba(255, 50, 50, 0.3)',
    padding: '8px 12px',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
  }

  const sectionStyle: React.CSSProperties = {
    borderBottom: '1px solid rgba(255,255,255,0.06)',
    padding: '8px 12px',
  }

  function NodeList({ ids, max = 3 }: { ids: string[]; max?: number }) {
    const visible = ids.slice(0, max)
    const remaining = ids.length - max
    return (
      <div style={{ marginTop: 4 }}>
        {visible.map(id => {
          const n = nodeMap.get(id)
          return (
            <div key={id} style={{ color: '#888', padding: '1px 0', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              → {n?.label || id.split('::').pop() || id}
            </div>
          )
        })}
        {remaining > 0 && (
          <div style={{ color: '#555' }}>... {remaining} more</div>
        )}
      </div>
    )
  }

  return (
    <div style={panelStyle}>
      <div style={headerStyle}>
        <div style={{ color: '#ff4444', fontWeight: 700 }}>⚡ BLAST RADIUS</div>
        <div style={{ color: '#ff8888', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: 140 }}>
          {epicenterLabel}
        </div>
        <button
          onClick={() => setImpactData(null)}
          style={{ background: 'none', border: 'none', color: '#666', cursor: 'pointer', fontSize: 14, padding: '0 4px' }}
        >
          ✕
        </button>
      </div>

      {buckets.high.length > 0 && (
        <div style={sectionStyle}>
          <div style={{ color: '#ff2222', fontWeight: 600 }}>❗ HIGH RISK (1–2 hops) — {buckets.high.length} nodes</div>
          <NodeList ids={buckets.high} />
        </div>
      )}

      {buckets.medium.length > 0 && (
        <div style={sectionStyle}>
          <div style={{ color: '#ff8800', fontWeight: 600 }}>⚠ MEDIUM RISK (3–4 hops) — {buckets.medium.length} nodes</div>
          <NodeList ids={buckets.medium} />
        </div>
      )}

      {buckets.low.length > 0 && (
        <div style={sectionStyle}>
          <div style={{ color: '#aaff00', fontWeight: 600 }}>ℹ LOW RISK (5+ hops) — {buckets.low.length} nodes</div>
          <NodeList ids={buckets.low} max={2} />
        </div>
      )}

      {buckets.test.length > 0 && (
        <div style={sectionStyle}>
          <div style={{ color: '#00ff88', fontWeight: 600 }}>TESTS TO RUN: {buckets.test.length}</div>
          <NodeList ids={buckets.test} />
        </div>
      )}

      {buckets.untested.length > 0 && (
        <div style={sectionStyle}>
          <div style={{ color: '#ff4444', fontWeight: 600 }}>⛔ NO TEST COVERAGE: {buckets.untested.length} nodes</div>
          <NodeList ids={buckets.untested} />
        </div>
      )}

      <div style={{ padding: '8px 12px', display: 'flex', gap: 6 }}>
        <button style={{
          flex: 1, padding: '5px 0', borderRadius: 4, border: '1px solid rgba(0,255,136,0.3)',
          background: 'rgba(0,255,136,0.1)', color: '#00ff88', cursor: 'pointer', fontSize: 11, fontFamily: 'monospace',
        }}>
          ▶ Run Tests
        </button>
        <button style={{
          flex: 1, padding: '5px 0', borderRadius: 4, border: '1px solid rgba(255,255,255,0.1)',
          background: 'rgba(255,255,255,0.05)', color: '#888', cursor: 'pointer', fontSize: 11, fontFamily: 'monospace',
        }}>
          + Create Tasks
        </button>
      </div>
    </div>
  )
}
