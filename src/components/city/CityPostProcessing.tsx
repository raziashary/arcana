// CityPostProcessing.tsx — Post-processing effects: Bloom + Vignette
// Uses @react-three/postprocessing v3 — only use props that exist in this version.
// Do NOT import from 'postprocessing' directly (not guaranteed to be installed as
// a direct dep and its enum values differ between major versions).
import { EffectComposer, Bloom, Vignette } from '@react-three/postprocessing'

export function CityPostProcessing() {
  return (
    <EffectComposer>
      {/* Bloom: god node beacons and CALLS tubes emit glow */}
      <Bloom
        intensity={0.35}
        luminanceThreshold={0.7}
        luminanceSmoothing={0.9}
      />

      {/* Vignette: subtle dark edges for cinematic depth */}
      <Vignette offset={0.15} darkness={0.4} eskil={false} />
    </EffectComposer>
  )
}
