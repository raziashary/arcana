// TrafficParticles.tsx — GPU-accelerated particle flow along roads
// Uses THREE.Points with a custom shader to animate particles along Bezier paths
import { useRef, useMemo } from 'react'
import { useFrame } from '@react-three/fiber'
import * as THREE from 'three'
import type { GraphNode, GraphEdge } from '../../../stores/graphStore'
import type { CityLayout } from '../layout/layoutTypes'

interface TrafficParticlesProps {
  edges: GraphEdge[]
  nodes: GraphNode[]
  layout: CityLayout
}

// Edge kinds that have particles active by default
const PARTICLE_EDGE_KINDS = new Set(['IMPORTS_FROM', 'DEPENDS_ON', 'TESTED_BY'])

const EDGE_COLORS: Record<string, [number, number, number]> = {
  IMPORTS_FROM:       [0.27, 0.47, 1.0],
  TESTED_BY:          [0.95, 0.56, 0.17],
  DEPENDS_ON:         [0.93, 0.79, 0.28],
  CALLS:              [0.0,  1.0,  0.53],
  INHERITS:           [0.69, 0.48, 0.63],
}

// GLSL shaders (inlined for Vite compatibility)
const vertexShader = /* glsl */`
  uniform float uTime;
  attribute float aTimeOffset;
  attribute vec3 aStart;
  attribute vec3 aControl;
  attribute vec3 aEnd;
  attribute vec3 aColor;
  varying float vAlpha;
  varying vec3 vColor;

  vec3 quadBezier(vec3 p0, vec3 p1, vec3 p2, float t) {
    float u = 1.0 - t;
    return u * u * p0 + 2.0 * u * t * p1 + t * t * p2;
  }

  void main() {
    float t = mod(uTime * 0.4 + aTimeOffset, 1.0);
    vec3 pos = quadBezier(aStart, aControl, aEnd, t);

    vAlpha = sin(t * 3.14159) * 0.9 + 0.1;
    vColor = aColor;

    gl_Position = projectionMatrix * modelViewMatrix * vec4(pos, 1.0);
    gl_PointSize = 3.5 * vAlpha;
  }
`

const fragmentShader = /* glsl */`
  varying float vAlpha;
  varying vec3 vColor;

  void main() {
    // Circular point shape
    vec2 coord = gl_PointCoord - 0.5;
    float dist = length(coord);
    if (dist > 0.5) discard;

    float alpha = vAlpha * (1.0 - dist * 2.0);
    gl_FragColor = vec4(vColor, alpha);
  }
`

const PARTICLES_PER_EDGE = 8
const MAX_EDGES = 300

export function TrafficParticles({ edges, layout }: TrafficParticlesProps) {
  const pointsRef = useRef<THREE.Points>(null!)
  const materialRef = useRef<THREE.ShaderMaterial>(null!)

  // Filter to particle-visible edges
  const activeEdges = useMemo(() => {
    return edges
      .filter(e => PARTICLE_EDGE_KINDS.has(e.kind))
      .slice(0, MAX_EDGES)
  }, [edges])

  // Build geometry attributes from active edges
  const { geometry, count } = useMemo(() => {
    const n = activeEdges.length * PARTICLES_PER_EDGE

    const aStart = new Float32Array(n * 3)
    const aControl = new Float32Array(n * 3)
    const aEnd = new Float32Array(n * 3)
    const aColor = new Float32Array(n * 3)
    const aTimeOffset = new Float32Array(n)

    let idx = 0

    activeEdges.forEach((edge) => {
      const srcId = typeof edge.source === 'string' ? edge.source : (edge.source as GraphNode).id
      const tgtId = typeof edge.target === 'string' ? edge.target : (edge.target as GraphNode).id

      const srcBl = layout.buildingPositions.get(srcId)
      const tgtBl = layout.buildingPositions.get(tgtId)

      if (!srcBl || !tgtBl) {
        // Skip — advance idx without writing
        idx += PARTICLES_PER_EDGE
        return
      }

      const start = new THREE.Vector3(srcBl.x, 0.2, srcBl.z)
      const end = new THREE.Vector3(tgtBl.x, 0.2, tgtBl.z)
      const ctrl = start.clone().add(end).multiplyScalar(0.5)
      ctrl.y = 0.5 + start.distanceTo(end) * 0.05

      const col = EDGE_COLORS[edge.kind] ?? [0.5, 0.5, 0.5]

      for (let p = 0; p < PARTICLES_PER_EDGE; p++) {
        const i3 = idx * 3
        aStart[i3] = start.x;  aStart[i3+1] = start.y;  aStart[i3+2] = start.z
        aControl[i3] = ctrl.x; aControl[i3+1] = ctrl.y; aControl[i3+2] = ctrl.z
        aEnd[i3] = end.x;      aEnd[i3+1] = end.y;      aEnd[i3+2] = end.z
        aColor[i3] = col[0];   aColor[i3+1] = col[1];   aColor[i3+2] = col[2]
        aTimeOffset[idx] = p / PARTICLES_PER_EDGE
        idx++
      }
    })

    const geo = new THREE.BufferGeometry()
    geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(n * 3), 3))
    geo.setAttribute('aStart', new THREE.BufferAttribute(aStart, 3))
    geo.setAttribute('aControl', new THREE.BufferAttribute(aControl, 3))
    geo.setAttribute('aEnd', new THREE.BufferAttribute(aEnd, 3))
    geo.setAttribute('aColor', new THREE.BufferAttribute(aColor, 3))
    geo.setAttribute('aTimeOffset', new THREE.BufferAttribute(aTimeOffset, 1))

    return { geometry: geo, count: n }
  }, [activeEdges, layout])

  // Animate time uniform
  useFrame(({ clock }) => {
    if (materialRef.current) {
      materialRef.current.uniforms.uTime.value = clock.getElapsedTime()
    }
  })

  if (count === 0) return null

  return (
    <points ref={pointsRef} geometry={geometry} frustumCulled={false}>
      <shaderMaterial
        ref={materialRef}
        vertexShader={vertexShader}
        fragmentShader={fragmentShader}
        uniforms={{ uTime: { value: 0 } }}
        transparent
        depthWrite={false}
        blending={THREE.AdditiveBlending}
      />
    </points>
  )
}
