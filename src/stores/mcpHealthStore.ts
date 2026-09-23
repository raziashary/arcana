// mcpHealthStore.ts — health of the EXTERNAL MCP servers Arcana connects to.
//
// 🔴 **Why this file exists (`P8-55`).** This state lived in `sidecarStore.ts`,
// which carried two unrelated concerns: the dead Python sidecar's lifecycle,
// and this — the live health of external MCP servers, rendered by
// `McpHealthPanel` and backed by the `get_mcp_health` / `force_reconnect_mcp`
// commands that `P8-51` kept for exactly this reason.
//
// Deleting the store wholesale, as "delete the sidecar frontend" implies,
// would have taken a live feature with it. That is the same mistake `P8-51`
// caught on the Rust side and `P8-54` caught in `src/components/sidecar/` —
// three containers named for the sidecar, each holding something alive.

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

/** Connection state of one external MCP server. */
export type McpConnectionState =
  | 'connected'
  | 'degraded'
  | 'disconnected'
  | 'reconnecting'
  | 'not_monitored'

/**
 * Health of one external MCP server, as `get_mcp_health` returns it.
 *
 * ⚠ Copied verbatim from the store this replaces rather than retyped. The
 * first version here was written from memory and was wrong in four ways —
 * two missing states and a different field set — which `McpHealthPanel`
 * caught at typecheck only because it is typed against this. `L-9`.
 */
export interface McpServerHealth {
  server_name:          string
  endpoint:             string
  transport:            string       // "http" | "stdio"
  state:                McpConnectionState
  last_ping_at:         string | null
  last_success_at:      string | null
  consecutive_failures: number
  avg_latency_ms:       number
  error_message:        string | null
}

interface McpHealthState {
  servers: McpServerHealth[]
  loading: boolean
  error: string | null

  load: () => Promise<void>
  reconnect: (serverName: string) => Promise<void>
}

/**
 * Names of the servers that are not currently connected.
 *
 * Derived rather than stored: `sidecarStore` kept a separate `mcpDisconnected`
 * array alongside the health list, which is two lists that must agree — and
 * the second one is always the one nobody updates (`L-19`).
 */
export function disconnectedServers(servers: McpServerHealth[]): string[] {
  return servers.filter((s) => s.state !== 'connected').map((s) => s.server_name)
}

export const useMcpHealthStore = create<McpHealthState>((set) => ({
  servers: [],
  loading: false,
  error: null,

  load: async () => {
    set({ loading: true, error: null })
    try {
      const servers = await invoke<McpServerHealth[]>('get_mcp_health')
      set({ servers, loading: false })
    } catch (e) {
      // ⚠ Surfaced, not swallowed. The previous version logged to the console
      // and left the panel showing stale rows, so a failing health check was
      // indistinguishable from every server being fine.
      set({ error: String(e), loading: false })
    }
  },

  reconnect: async (serverName: string) => {
    try {
      await invoke('force_reconnect_mcp', { serverName })
    } catch (e) {
      set({ error: String(e) })
      return
    }
    // Re-read rather than guessing the new state: the reconnect is
    // asynchronous on the Rust side and the panel must show what is, not what
    // was requested.
    const servers = await invoke<McpServerHealth[]>('get_mcp_health')
    set({ servers })
  },
}))
