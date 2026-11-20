// LabelDeclutter — hides City labels that would overlap a more important one.
// Runs inside the Canvas so it can follow the camera; it only reads the labels'
// on-screen boxes (see labelOverlap.ts) and toggles `visibility`, so React never
// sees or resets it.

import { useRef } from 'react'
import { useFrame } from '@react-three/fiber'
import { declutterLabels } from './labelOverlap'

/** Re-measure every this many frames; labels move only when the camera does. A
 *  newly overlapping pair shows for at most this many frames (about 80 ms). */
const EVERY_N_FRAMES = 5

export function LabelDeclutter() {
  const frame = useRef(0)

  useFrame(() => {
    frame.current += 1
    if (frame.current % EVERY_N_FRAMES !== 0) return
    // drei mounts `Html` content outside the canvas's own wrapper (checked in the
    // real app: two levels up), so the canvas's parent is the wrong subtree. Only
    // City labels carry `data-city-label`, so the document is the right scope.
    declutterLabels(document)
  })

  return null
}
