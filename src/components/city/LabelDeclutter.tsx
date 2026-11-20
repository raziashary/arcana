// LabelDeclutter — hides City labels that would overlap a more important one.
// Runs inside the Canvas so it can follow the camera; it only reads the labels'
// on-screen boxes (see labelOverlap.ts) and toggles `visibility`, so React never
// sees or resets it.

import { useRef } from 'react'
import { useFrame } from '@react-three/fiber'
import { hiddenLabels, type LabelBox } from './labelOverlap'

/** Re-measure every this many frames; labels move only when the camera does. */
const EVERY_N_FRAMES = 10

export function LabelDeclutter() {
  const frame = useRef(0)

  useFrame(({ gl }) => {
    frame.current += 1
    if (frame.current % EVERY_N_FRAMES !== 0) return
    const root = gl.domElement.parentElement
    if (!root) return
    const labels = Array.from(root.querySelectorAll<HTMLElement>('[data-city-label]'))
    const boxes: LabelBox[] = labels.map((el, id) => {
      const rect = el.getBoundingClientRect()
      return {
        id,
        priority: Number(el.dataset.priority ?? 0),
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
      }
    })
    const hidden = hiddenLabels(boxes)
    labels.forEach((el, id) => {
      const next = hidden.has(id) ? 'hidden' : 'visible'
      if (el.style.visibility !== next) el.style.visibility = next
    })
  })

  return null
}
