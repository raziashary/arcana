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
  removeAgent:         (agentId: string) => void
  setAgentModel:       (agentId: string, model: string | null) => void
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
    model:                     null,
    has_ever_been_active:      false,
  }
}

// ── Task-ID parsing heuristic (Doc 06 Phase 3) ────────────────────────────────
// When a QC-style tool call fires (`get_task`, `update_task_status`, etc.),
// pull the task_id out of the args string so the QC card can render
// "reviewing #N". Args is either a JSON object stringification or a
// k=v style preview — we just grep for the first integer following a
// `task_id` / `id` token.
function parseTaskId(toolName: string, args: string): number | null {
  if (!args) return null
  const taskTools = ['get_task', 'update_task_status', 'update_task',
                     'claim_task', 'request_review', 'set_task_status']
  if (!taskTools.includes(toolName)) return null
  // Match patterns like  "task_id": 42  /  task_id=42  /  id: 42
  const m = args.match(/"?(?:task_)?id"?\s*[:=]\s*"?(\d+)/i)
  return m ? parseInt(m[1], 10) : null
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
  isMonitorCollapsed: localStorage.getItem('arcana.live_monitor.collapsed') === 'true',
  lastEventAt:        Date.now(),

  // ── Initialise a set of agents (called from ArcanaTab on mount) ────────────
  initAgents: (agentIds) => {
    const existing = get().agents
    const agents: Record<string, AgentLiveState> = {}
    agentIds.forEach(id => {
      // Preserve existing live state if already present
      agents[id] = existing[id] ?? makeInitialAgent(id)
    })
    set({ agents })
  },

  removeAgent: (agentId) => {
    set(state => {
      if (!(agentId in state.agents)) return state
      const agents = { ...state.agents }
      delete agents[agentId]
      return { agents, lastEventAt: Date.now() }
    })
  },

  setMonitorCollapsed: (v) => {
    localStorage.setItem('arcana.live_monitor.collapsed', String(v))
    set({ isMonitorCollapsed: v })
  },

  // ── Doc 01 Phase 3 — wire `worker.model` from workerStore into the
  // live-state record so AgentLiveCard can render a "haiku" / "sonnet" badge.
  // Called from ArcanaTab's effect after `loadWorkers()` resolves.
  // Lazy-creates the agent record if it doesn't exist yet — same belt-and-
  // suspenders pattern as the event handlers.
  setAgentModel: (agentId, model) => {
    set(state => {
      const agent = state.agents[agentId] ?? makeInitialAgent(agentId)
      if (agent.model === model) return state
      return {
        agents: { ...state.agents, [agentId]: { ...agent, model } },
      }
    })
  },

  // ── Tool call start: adds a running log line ────────────────────────────────
  //
  // Lazy slug registration (docs/endtoendrun/14may2026/01 Phase 1): if the
  // agent record doesn't exist yet, create it on demand. The old behaviour
  // silently dropped any event that arrived before `initAgents()` had run,
  // which raced with every cold-start tick.
  handleToolCallStart: ({ agent_id, tool_call_id, name, args }) => {
    set(state => {
      const agent = state.agents[agent_id] ?? makeInitialAgent(agent_id)
      const newLine: LogLine = {
        id:                  tool_call_id,
        timestamp_offset_ms: Date.now() - agent.last_event_at,
        tool_name:           name,
        argument:            truncate(args ?? '', 40),
        full_argument:       args ?? '',
        status:              'running',
      }
      // Doc 06 Phase 3 — populate task_id from QC-style tool calls so the
      // card can render "reviewing #N". Falls back to existing value when
      // the args don't carry a task id.
      const parsedTaskId = parseTaskId(name, args ?? '')
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
            task_id:      parsedTaskId ?? agent.task_id,
            // Doc 01 Phase 3 — flip the "has ever been active" flag the
            // first time we see real activity. Idle dot turns from gray
            // (never seen) to blue (intentionally idle).
            has_ever_been_active: true,
          },
        },
      }
    })
  },

  // ── Tool call result: marks the log line done/error ─────────────────────────
  // Lazy-create: if a result arrives without a prior start (e.g. start was
  // dropped pre-init), still register the agent so the rest of the session
  // is tracked correctly. The dangling result frame is ignored when no
  // matching log_line exists, which is the same as the old behaviour but
  // without losing future events.
  handleToolCallResult: ({ agent_id, tool_call_id, is_error, elapsed_ms }) => {
    set(state => {
      const agent = state.agents[agent_id] ?? makeInitialAgent(agent_id)
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
  // Lazy slug registration (docs/endtoendrun/14may2026/01 Phase 1) — the
  // STARTUP `worker:tick-start` events fire ~1ms after Rust boot, long before
  // AgentRoster has mounted and called `initAgents()`. Before this patch the
  // store silently dropped them; the Live Monitor card never came online.
  handleStatusChange: ({ agent_id, status }) => {
    set(state => {
      const agent = state.agents[agent_id] ?? makeInitialAgent(agent_id)
      // Doc 01 Phase 3 — any 'active' / 'starting' transition counts as
      // "the agent has been seen alive at least once" for the dot-color
      // heuristic on the card.
      const seenAlive = agent.has_ever_been_active
        || status === 'active'
        || status === 'starting'
      return {
        lastEventAt: Date.now(),
        agents: {
          ...state.agents,
          [agent_id]: {
            ...agent,
            status,
            last_event_at:         Date.now(),
            has_ever_been_active:  seenAlive,
          },
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

    // Fleet workers do not emit the agent:* events above — those come from
    // the interactive chat path. Without bridging worker activity into the
    // same store, the Live Monitor sat at "IDLE" forever even though the
    // Agent Roster clearly showed workers running.
    //
    // Map Rust's worker:tick-start / worker:tick-end / worker:status
    // events onto the same handleStatusChange path so workers light up
    // green when they're mid-tick and go grey when they finish.
    unlisteners.push(
      listen<{ slug: string; mode: string; timestamp: string }>(
        'worker:tick-start',
        e => store.handleStatusChange({ agent_id: e.payload.slug, status: 'active' })
      )
    )
    unlisteners.push(
      listen<{ slug: string; outcome: string; timestamp: string }>(
        'worker:tick-end',
        e => store.handleStatusChange({
          agent_id: e.payload.slug,
          // outcome 'success' / 'mcp_unavailable' / 'error' — map error to error state
          status: e.payload.outcome.startsWith('error') ? 'error' : 'idle',
        })
      )
    )
    // Doc 06 Phase 3 — Rust pushes a one-line breadcrumb via worker:log-line
    // when a recipe gate short-circuits (e.g. qc-agent skip path: "idle —
    // no qc_review tasks"). Inject it as a synthetic LogLine on the matching
    // agent so the card body has visible content instead of looking dead.
    // Lazy-creates the agent record via the existing pattern.
    unlisteners.push(
      listen<{ slug: string; text: string; timestamp: string }>(
        'worker:log-line',
        e => {
          const { slug, text } = e.payload
          set(state => {
            const agent = state.agents[slug] ?? makeInitialAgent(slug)
            const newLine: LogLine = {
              id:                  `breadcrumb-${slug}-${Date.now()}`,
              timestamp_offset_ms: Date.now() - agent.last_event_at,
              tool_name:           '·',                 // dot — visually distinct from real tools
              argument:            truncate(text, 40),
              full_argument:       text,
              status:              'thought',          // styled as inert
            }
            return {
              lastEventAt: Date.now(),
              agents: {
                ...state.agents,
                [slug]: {
                  ...agent,
                  log_lines:    [...agent.log_lines.slice(-49), newLine],
                  last_event_at: Date.now(),
                },
              },
            }
          })
        }
      )
    )

    unlisteners.push(
      listen<{ slug: string; status: string }>(
        'worker:status',
        e => {
          if (e.payload.status === 'deleted') {
            store.removeAgent(e.payload.slug)
            return
          }
          // Rust emits status strings like "idle" / "working" / "error".
          // Translate to AgentLiveState's enum.
          //
          // The `mode1_idle_no_provider` arm is kept deliberately: `workers.db`
          // stores each worker's LAST status, so a row written before `P8-43`
          // survives an upgrade and would otherwise fall through to plain
          // `idle` — reporting "waiting for work" for a worker that is blocked
          // on a missing key. It is corrected on that worker's next tick.
          const mapped =
            e.payload.status === 'working'                  ? 'active'      :
            e.payload.status === 'error'                    ? 'error'       :
            e.payload.status === 'paused'                   ? 'paused'      :
            e.payload.status === 'idle_no_provider'         ? 'no_provider' :
            e.payload.status === 'mode1_idle_no_provider'   ? 'no_provider' :
            'idle'
          store.handleStatusChange({ agent_id: e.payload.slug, status: mapped as any })
        }
      )
    )

    // Subagent events — a tick that spawns subagents (via spawn_subagent /
    // spawn_parallel_subagents) gets full per-tool streaming through
    // `SubagentManager::pump_events` → `emit_subagent_event_global` → the
    // `subagent:event` channel. The payload shape is
    // `{ subagent_id, event: { kind: "tool_start"|"tool_end"|... } }`
    // (tagged enum from arcana-core/src/subagent/types.rs).
    //
    // Translate `tool_start` / `tool_end` here so the Live Monitor card for
    // each subagent fills in tool names and timings. The agent_id is the
    // subagent slug (e.g. `qc-rev-task42-abc123`), and `handleToolCallStart`
    // now auto-creates the agent record on first event — so we don't need
    // to pre-register the subagent slugs anywhere.
    //
    // ⚠ This covers SUBAGENT tool activity, not a worker's own. A worker's
    // per-tool stream is a separate, larger change. Subagent visibility is
    // the cheap
    // win available right now and matches doc 01 Phase 2's recommended scope.
    unlisteners.push(
      listen<{ subagent_id: string; event: any }>(
        'subagent:event',
        e => {
          const sid = e.payload.subagent_id
          const ev  = e.payload.event
          if (!ev || typeof ev !== 'object') return
          switch (ev.kind) {
            case 'tool_start': {
              const argsStr = typeof ev.input === 'string'
                ? ev.input
                : ev.input ? JSON.stringify(ev.input) : ''
              store.handleToolCallStart({
                agent_id:     sid,
                tool_call_id: `${sid}-${ev.tool ?? 'tool'}-${Date.now()}`,
                name:         ev.tool ?? 'unknown',
                args:         argsStr,
              })
              break
            }
            case 'tool_end': {
              // The Rust-side event doesn't carry the matching tool_call_id,
              // so look up the most recent running line for this agent + tool
              // and close it. Same shape `handleToolCallResult` expects.
              const agent = get().agents[sid]
              const matching = agent?.log_lines
                .slice()
                .reverse()
                .find(l => l.status === 'running' && l.tool_name === ev.tool)
              if (matching) {
                store.handleToolCallResult({
                  agent_id:     sid,
                  tool_call_id: matching.id,
                  result:       typeof ev.output === 'string' ? ev.output : '',
                  is_error:     !!ev.is_error,
                  elapsed_ms:   matching.timestamp_offset_ms,
                })
              }
              break
            }
            case 'status': {
              // SubagentStatus enum values: Spawning / Running / Completed /
              // Failed / Terminated. Map to Live Monitor enum.
              const s = String(ev.status ?? '').toLowerCase()
              const mapped =
                s === 'running'                     ? 'active'  :
                s === 'failed' || s === 'terminated' ? 'error'  :
                                                       'idle'
              store.handleStatusChange({ agent_id: sid, status: mapped as any })
              break
            }
            case 'error': {
              store.handleError({
                agent_id:      sid,
                error_message: String(ev.message ?? 'unknown error'),
                is_fatal:      false,
              })
              break
            }
            // token / thinking / final / cost are intentionally not
            // surfaced as live-monitor entries here — token deltas go
            // through `agent:token-update` for the sparkline (and could
            // be re-bridged later if desired).
          }
        }
      )
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
