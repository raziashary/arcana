// CityPostProcessing.tsx — Post-processing effects: Bloom, SSAO, Vignette
// Uses @react-three/postprocessing
import { EffectComposer, Bloom, Vignette } from '@react-three/postprocessing'
import { BlendFunction, KernelSize } from 'postprocessing'

export function CityPostProcessing() {
  return (
    <EffectComposer multisampling={0}>
      {/* Bloom: god node beacons and CALLS tubes glow */}
      <Bloom
        intensity={0.4}
        luminanceThreshold={0.65}
        luminanceSmoothing={0.85}
        kernelSize={KernelSize.MEDIUM}
        mipmapBlur
      />

      {/* Vignette: dark edges, cinematic depth */}
      <Vignette
        eskil={false}
        offset={0.12}
        darkness={0.45}
        blendFunction={BlendFunction.NORMAL}
      />
    </EffectComposer>
  )
}
