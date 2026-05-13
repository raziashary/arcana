// ImpactParticles.tsx — Red particle streams along impact paths (blast radius visualization)
import { useRef, useMemo } from 'react'
import { useFrame } from '@react-three/fiber'
import * as THREE from 'three'
import type { GraphNode, GraphEdge, ImpactData } from '../../../stores/graphStore'
import type { CityLayout } from '../layout/layoutTypes'

interface ImpactParticlesProps {
  nodes: GraphNode[]
  edges: GraphEdge[]
  layout: CityLayout
  impactData: ImpactData
}

const IMPACT_PARTICLE_VERT = /* glsl */`
  uniform float uTime;
  attribute float aTimeOffset;
  attribute vec3 aStart;
  attribute vec3 aEnd;
  attribute float aRisk;  // 0=epicenter, 1=high, 2=medium, 3=low
  varying float vAlpha;
  varying vec3 vColor;

  void main() {
    float t = mod(uTime * 0.8 + aTimeOffset, 1.0);
    vec3 pos = mix(aStart, aEnd, t);
    pos.y += 0.5;  // slightly elevated

    vAlpha = sin(t * 3.14159) * 0.8 + 0.2;

    // Color: red for high → orange for medium → yellow for low
    if (aRisk < 1.5) {
      vColor = vec3(1.0, 0.13, 0.13);  // high: red
    } else if (aRisk < 2.5) {
      vColor = vec3(1.0, 0.53, 0.0);   // medium: orange
    } else {
      vColor = vec3(0.67, 1.0, 0.0);   // low: yellow-green
    }

    gl_Position = projectionMatrix * modelViewMatrix * vec4(pos, 1.0);
    gl_PointSize = 4.0 * vAlpha;
  }
`

const IMPACT_PARTICLE_FRAG = /* glsl */`
  varying float vAlpha;
  varying vec3 vColor;

  void main() {
    vec2 coord = gl_PointCoord - 0.5;
    if (length(coord) > 0.5) discard;
    gl_FragColor = vec4(vColor, vAlpha);
  }
`

const PARTICLES_PER_PATH = 12

export function ImpactParticles({ edges, layout, impactData }: ImpactParticlesProps) {
  const matRef = useRef<THREE.ShaderMaterial>(null!)

  const geometry = useMemo(() => {
    const affectedIds = new Set([...Array.from(impactData.affected.keys()), impactData.epicenterId])

    // Find edges between affected nodes
    const impactEdges = edges.filter(e => {
      const srcId = typeof e.source === 'string' ? e.source : (e.source as GraphNode).id
      const tgtId = typeof e.target === 'string' ? e.target : (e.target as GraphNode).id
      return affectedIds.has(srcId) && affectedIds.has(tgtId)
    }).slice(0, 100)

    const n = impactEdges.length * PARTICLES_PER_PATH
    if (n === 0) return null

    const aStart = new Float32Array(n * 3)
    const aEnd = new Float32Array(n * 3)
    const aTimeOffset = new Float32Array(n)
    const aRisk = new Float32Array(n)

    let idx = 0

    impactEdges.forEach(edge => {
      const srcId = typeof edge.source === 'string' ? edge.source : (edge.source as GraphNode).id
      const tgtId = typeof edge.target === 'string' ? edge.target : (edge.target as GraphNode).id

      const srcBl = layout.buildingPositions.get(srcId)
      const tgtBl = layout.buildingPositions.get(tgtId)
      if (!srcBl || !tgtBl) { idx += PARTICLES_PER_PATH; return }

      const tgtRisk = impactData.affected.get(tgtId)?.risk
      const riskNum = tgtRisk === 'high' ? 1 : tgtRisk === 'medium' ? 2 : 3

      for (let p = 0; p < PARTICLES_PER_PATH; p++) {
        const i3 = idx * 3
        aStart[i3] = srcBl.x; aStart[i3+1] = 0; aStart[i3+2] = srcBl.z
        aEnd[i3] = tgtBl.x;   aEnd[i3+1] = 0;   aEnd[i3+2] = tgtBl.z
        aTimeOffset[idx] = p / PARTICLES_PER_PATH
        aRisk[idx] = riskNum
        idx++
      }
    })

    const geo = new THREE.BufferGeometry()
    geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(n * 3), 3))
    geo.setAttribute('aStart', new THREE.BufferAttribute(aStart, 3))
    geo.setAttribute('aEnd', new THREE.BufferAttribute(aEnd, 3))
    geo.setAttribute('aTimeOffset', new THREE.BufferAttribute(aTimeOffset, 1))
    geo.setAttribute('aRisk', new THREE.BufferAttribute(aRisk, 1))
    return geo
  }, [edges, layout, impactData])

  useFrame(({ clock }) => {
    if (matRef.current) matRef.current.uniforms.uTime.value = clock.getElapsedTime()
  })

  if (!geometry) return null

  return (
    <points geometry={geometry} frustumCulled={false}>
      <shaderMaterial
        ref={matRef}
        vertexShader={IMPACT_PARTICLE_VERT}
        fragmentShader={IMPACT_PARTICLE_FRAG}
        uniforms={{ uTime: { value: 0 } }}
        transparent
        depthWrite={false}
        blending={THREE.AdditiveBlending}
      />
    </points>
  )
}
