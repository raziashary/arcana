// cityWorker.ts — runs in Web Worker context
// Receives graph data, returns computed layout

import { computeCityLayout } from './cityLayout'

self.onmessage = (event: MessageEvent) => {
  const { nodes, edges, communities, citySize } = event.data
  try {
    const layout = computeCityLayout(communities, nodes, edges, citySize ?? 180)
    self.postMessage({ ok: true, layout })
  } catch (err: any) {
    self.postMessage({ ok: false, error: err.message })
  }
}

export {} // Make this a module
