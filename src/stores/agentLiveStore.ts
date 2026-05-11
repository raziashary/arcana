// agentLiveStore — real-time agent fleet state
// Subscribes to Tauri events and maintains per-agent live state for the Live Monitor section.

import { create } from 'zustand'
import { listen } from '@tauri-apps/api/event'
import {
  type AgentLiveState,
  type AgentType,
  type Stage,
  type LogLine,
  type SparklineWindow,
  type AgentToolCallStartPayload,
  type AgentToolCallResultPayload,
  type AgentStageChangePayload,
  type AgentTokenUpdatePayload,
  type AgentStatusChangePayload,
  type AgentErrorPayload,
  WORKER_STAGES,
  SUPERVISOR_STAGES,
  QC_STAGES,
} from '../types/live'

// ── Token update throttle buffer (module-level, outside store) ────────────────
const tokenBuffer: Record<string, { total: number; window: number }> = {}
let   tokenFlushTimer: ReturnType<typeof setInterval> | null = null

// ── Store interface ───────────────────────────────────────────────────────────

interface AgentLiveStore {
  agents:             Record<string, AgentLiveState>
  isMonitorCollapsed: boolean
  lastEventAt:        number

  initAgents:          (agentIds: string[]) => void
  setMonitorCollapsed: (v: boolean) => void
  handleToolCallStart: (payload: AgentToolCallStartPayload) => void
  handleToolCallResult:(payload: AgentToolCallResultPayload) => void
  handleStageChange:   (payload: AgentStageChangePayload) => void
  handleTokenUpdate:   (payload: AgentTokenUpdatePayload) => void
  handleStatusChange:  (payload: AgentStatusChangePayload) => void
  handleError:         (payload: AgentErrorPayload) => void
  subscribeToEvents:   () => () => void   // returns cleanup fn
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function inferAgentType(id: string): AgentType {
  if (id === 'supervisor') return 'supervisor'
  if (id === 'qc-agent')   return 'qc'
  if (id === 'pa')         return 'pa'
  return 'worker'
}

function getInitialStages(id: string): Stage[] {
  const type = inferAgentType(id)
  if (type === 'supervisor') return SUPERVISOR_STAGES.map(s => ({ ...s }))
  if (type === 'qc')         return QC_STAGES.map(s => ({ ...s }))
  return WORKER_STAGES.map(s => ({ ...s }))
}

function makeEmptySparkline(): SparklineWindow[] {
  return Array.from({ length: 10 }, (_, i) => ({ window_index: i, tokens: 0 }))
}

function makeInitialAgent(id: string): AgentLiveState {
  return {
    agent_id:                  id,
    agent_type:                inferAgentType(id),
    display_name:              id.toUpperCase(),
    status:                    'idle',
    stages:                    getInitialStages(id),
    current_tool:              null,
    current_file:              null,
    current_file_diff:         undefined,
    log_lines:                 [],
    total_tokens:              0,
    sparkline:                 makeEmptySparkline(),
    error_message:             null,
    last_event_at:             Date.now(),
    task_id:                   null,
    elapsed_since_last_tool_ms: 0,
  }
}

// Extract a short filename from a tool args string
function extractFile(args: string): string | null {
  if (!args) return null
  // Prefer last path segment that looks like a filename with extension
  const match = args.match(/([^/\\]+\.[a-z]{1,6})/i)
  return match ? match[1] : null
}

function truncate(s: string, n: number): string {
  return s.length > n ? s.slice(0, n) + '…' : s
}

// ── Store ─────────────────────────────────────────────────────────────────────

export const useAgentLiveStore = create<AgentLiveStore>((set, get) => ({
  agents:             {},
  isMonitorCollapsed: localStorage.getItem('autodev.live_monitor.collapsed') === 'true',
  lastEventAt:        Date.now(),

  // ── Initialise a set of agents (called from AutoDevTab on mount) ────────────
  initAgents: (agentIds) => {
    const existing = get().agents
    const agents: Record<string, AgentLiveState> = {}
    agentIds.forEach(id => {
      // Preserve existing live state if already present
      agents[id] = existing[id] ?? makeInitialAgent(id)
    })
    set({ agents })
  },

  setMonitorCollapsed: (v) => {
    localStorage.setItem('autodev.live_monitor.collapsed', String(v))
    set({ isMonitorCollapsed: v })
  },

  // ── Tool call start: adds a running log line ────────────────────────────────
  handleToolCallStart: ({ agent_id, tool_call_id, name, args }) => {
    set(state => {
      const agent = state.agents[agent_id]
      if (!agent) return state
      const newLine: LogLine = {
        id:                  tool_call_id,
        timestamp_offset_ms: Date.now() - agent.last_event_at,
        tool_name:           name,
        argument:            truncate(args ?? '', 40),
        full_argument:       args ?? '',
        status:              'running',
      }
      return {
        lastEventAt: Date.now(),
        agents: {
          ...state.agents,
          [agent_id]: {
            ...agent,
            status:       'active',
            current_tool: name,
            current_file: extractFile(args ?? ''),
            current_file_diff: undefined,
            log_lines:    [...agent.log_lines.slice(-49), newLine],
            last_event_at: Date.now(),
          },
        },
      }
    })
  },

  // ── Tool call result: marks the log line done/error ─────────────────────────
  handleToolCallResult: ({ agent_id, tool_call_id, is_error, elapsed_ms }) => {
    set(state => {
      const agent = state.agents[agent_id]
      if (!agent) return state
      return {
        lastEventAt: Date.now(),
        agents: {
          ...state.agents,
          [agent_id]: {
            ...agent,
            current_tool:  null,
            last_event_at: Date.now(),
            log_lines: agent.log_lines.map(line =>
              line.id === tool_call_id
                ? { ...line, status: is_error ? 'error' : 'success', elapsed_ms }
                : line
            ),
          },
        },
      }
    })
  },

  // ── Stage change: advances the pipeline strip ────────────────────────────────
  handleStageChange: ({ agent_id, stage, status, elapsed_ms }) => {
    set(state => {
      const agent = state.agents[agent_id]
      if (!agent) return state
      return {
        agents: {
          ...state.agents,
          [agent_id]: {
            ...agent,
            stages: agent.stages.map(s =>
              s.id === stage ? { ...s, status, elapsed_ms } : s
            ),
          },
        },
      }
    })
  },

  // ── Token update: buffered — flushed every 500ms by the interval ─────────────
  handleTokenUpdate: ({ agent_id, total_tokens, window_tokens }) => {
    tokenBuffer[agent_id] = { total: total_tokens, window: window_tokens }
  },

  // ── Status change: idle / active / error / stopped ───────────────────────────
  handleStatusChange: ({ agent_id, status }) => {
    set(state => {
      const agent = state.agents[agent_id]
      if (!agent) return state
      return {
        lastEventAt: Date.now(),
        agents: {
          ...state.agents,
          [agent_id]: { ...agent, status, last_event_at: Date.now() },
        },
      }
    })
  },

  // ── Error event ──────────────────────────────────────────────────────────────
  handleError: ({ agent_id, error_message, is_fatal }) => {
    set(state => {
      const agent = state.agents[agent_id]
      if (!agent) return state
      return {
        lastEventAt: Date.now(),
        agents: {
          ...state.agents,
          [agent_id]: {
            ...agent,
            status:        is_fatal ? 'error' : agent.status,
            error_message,
            last_event_at: Date.now(),
          },
        },
      }
    })
  },

  // ── Subscribe to all relevant Tauri events ───────────────────────────────────
  subscribeToEvents: () => {
    const store = get()
    const unlisteners: Promise<() => void>[] = []

    unlisteners.push(
      listen<AgentToolCallStartPayload>('agent:tool-call-start',
        e => store.handleToolCallStart(e.payload))
    )
    unlisteners.push(
      listen<AgentToolCallResultPayload>('agent:tool-call-result',
        e => store.handleToolCallResult(e.payload))
    )
    unlisteners.push(
      listen<AgentStageChangePayload>('agent:stage-change',
        e => store.handleStageChange(e.payload))
    )
    unlisteners.push(
      listen<AgentTokenUpdatePayload>('agent:token-update',
        e => store.handleTokenUpdate(e.payload))
    )
    unlisteners.push(
      listen<AgentStatusChangePayload>('agent:status-change',
        e => store.handleStatusChange(e.payload))
    )
    unlisteners.push(
      listen<AgentErrorPayload>('agent:error',
        e => store.handleError(e.payload))
    )

    // Flush token buffer to store every 500ms to avoid overwhelming React
    tokenFlushTimer = setInterval(() => {
      const entries = Object.entries(tokenBuffer)
      if (entries.length === 0) return

      set(state => {
        const newAgents = { ...state.agents }
        entries.forEach(([agentId, { total, window: win }]) => {
          const agent = newAgents[agentId]
          if (!agent) return
          const newSparkline: SparklineWindow[] = [
            ...agent.sparkline.slice(1),
            { window_index: 9, tokens: win },
          ]
          newAgents[agentId] = { ...agent, total_tokens: total, sparkline: newSparkline }
          delete tokenBuffer[agentId]
        })
        return { agents: newAgents }
      })
    }, 500)

    return () => {
      unlisteners.forEach(p => p.then(fn => fn()))
      if (tokenFlushTimer) clearInterval(tokenFlushTimer)
    }
  },
}))
