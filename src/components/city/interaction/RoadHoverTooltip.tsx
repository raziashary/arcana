// RoadHoverTooltip.tsx — Road hover tooltip HTML overlay (placeholder for future road hover)
import type { AggregateRoad } from '../layout/layoutTypes'

interface RoadHoverTooltipProps {
  road: AggregateRoad | null
  x: number
  y: number
}

export function RoadHoverTooltip({ road, x, y }: RoadHoverTooltipProps) {
  if (!road) return null

  return (
    <div style={{
      position: 'fixed',
      left: x + 12,
      top: y - 8,
      zIndex: 900,
      background: 'rgba(10, 10, 15, 0.92)',
      border: '1px solid rgba(255,255,255,0.12)',
      borderRadius: 5,
      padding: '6px 10px',
      fontSize: 11,
      color: '#ccc',
      fontFamily: 'monospace',
      pointerEvents: 'none',
      boxShadow: '0 4px 12px rgba(0,0,0,0.5)',
    }}>
      <div style={{ color: '#4477FF', fontWeight: 600, marginBottom: 3 }}>IMPORTS_FROM</div>
      <div>District {road.srcCommunity} → District {road.tgtCommunity}</div>
      <div style={{ color: '#666', marginTop: 2 }}>{road.edgeCount} edge{road.edgeCount !== 1 ? 's' : ''}</div>
    </div>
  )
}
