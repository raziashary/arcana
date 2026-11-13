// CityLabel — a text label in the 3D city, drawn as a DOM overlay.
//
// This replaces drei's `<Text>` (troika). Troika loads its default font and its
// unicode fallback fonts from a CDN at runtime, and the app's CSP allows no
// network origin besides IPC (`connect-src ipc: http://ipc.localhost`), so the
// request is refused, the text promise never resolves, and the `<Suspense>`
// around every label stays on its fallback: Graph City never finished drawing.
// A DOM label needs no font file and no fetch; it uses the page's own font
// stack, so it also follows the app's typography.

import { Html } from '@react-three/drei'
import { CITY } from '../../styles/palette'

interface CityLabelProps {
  position: [number, number, number]
  label: string
  color: string
  /** CSS pixels at the reference distance. */
  fontPx: number
  /** drei's `distanceFactor`: the label scales by this over the camera distance. */
  distanceFactor: number
  maxWidthPx?: number
}

export function CityLabel({
  position,
  label,
  color,
  fontPx,
  distanceFactor,
  maxWidthPx = 220,
}: CityLabelProps) {
  return (
    <Html
      position={position}
      center
      distanceFactor={distanceFactor}
      zIndexRange={[20, 0]}
      style={{ pointerEvents: 'none', userSelect: 'none' }}
    >
      <div
        style={{
          color,
          fontFamily: "'IBM Plex Mono', ui-monospace, Consolas, monospace",
          fontSize: fontPx,
          fontWeight: 500,
          lineHeight: 1.2,
          maxWidth: maxWidthPx,
          textAlign: 'center',
          // One line: a label wider than maxWidthPx is clipped with an
          // ellipsis rather than wrapped (the overlay shrink-wraps, so wrapping
          // breaks it into one character per line).
          whiteSpace: 'nowrap',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          textShadow: `0 0 3px ${CITY.black}, 0 0 3px ${CITY.black}, 0 1px 2px ${CITY.black}`,
          opacity: 0.92,
        }}
      >
        {label}
      </div>
    </Html>
  )
}
