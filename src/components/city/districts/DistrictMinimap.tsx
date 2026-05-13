// DistrictMinimap.tsx — HTML Canvas 2D minimap overlay showing district footprints
import { useEffect, useRef } from 'react'
import type { CityLayout } from '../layout/layoutTypes'

interface DistrictMinimapProps {
  layout: CityLayout
  citySize: number
}

const MINIMAP_SIZE = 120

export function DistrictMinimap({ layout, citySize }: DistrictMinimapProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null)

  // Draw static district footprints once when layout changes
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const scale = MINIMAP_SIZE / citySize

    ctx.clearRect(0, 0, MINIMAP_SIZE, MINIMAP_SIZE)

    // Background
    ctx.fillStyle = 'rgba(5, 5, 8, 0.85)'
    ctx.fillRect(0, 0, MINIMAP_SIZE, MINIMAP_SIZE)

    // Draw districts
    layout.districts.forEach(d => {
      const mx = d.x * scale
      const mz = d.z * scale
      const mw = d.width * scale
      const md = d.depth * scale

      // Fill
      ctx.fillStyle = d.color + '30'
      ctx.fillRect(mx, mz, mw, md)

      // Border
      ctx.strokeStyle = d.color + '88'
      ctx.lineWidth = 0.5
      ctx.strokeRect(mx, mz, mw, md)
    })

    // Border on minimap itself
    ctx.strokeStyle = 'rgba(255,255,255,0.15)'
    ctx.lineWidth = 1
    ctx.strokeRect(0, 0, MINIMAP_SIZE, MINIMAP_SIZE)
  }, [layout, citySize])

  return (
    <div style={{
      position: 'absolute',
      top: 12,
      left: 12,
      zIndex: 10,
      pointerEvents: 'none',
    }}>
      <canvas
        ref={canvasRef}
        width={MINIMAP_SIZE}
        height={MINIMAP_SIZE}
        style={{
          display: 'block',
          borderRadius: 4,
          boxShadow: '0 2px 8px rgba(0,0,0,0.6)',
        }}
      />
      <div style={{
        marginTop: 3,
        fontSize: 9,
        color: 'rgba(255,255,255,0.3)',
        textAlign: 'center',
        fontFamily: 'monospace',
        letterSpacing: '0.05em',
      }}>
        CITY MAP
      </div>
    </div>
  )
}
