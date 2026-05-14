// Live Monitor types — fleet agent real-time state
// Used by agentLiveStore, LiveMonitorSection, AgentLiveCard, and sub-components.

export type StageStatus = 'done' | 'current' | 'pending' | 'error' | 'blocked'

export interface Stage {
  id:         string       // e.g. "read", "write", "test"
  label:      string       // display text
  status:     StageStatus
  elapsed_ms?: number      // set when status → "done"
}

export const WORKER_STAGES: Stage[] = [
  { id: 'claim', label: 'claim', status: 'pending' },
  { id: 'read',  label: 'read',  status: 'pending' },
  { id: 'write', label: 'write', status: 'pending' },
  { id: 'test',  label: 'test',  status: 'pending' },
  { id: 'push',  label: 'push',  status: 'pending' },
]

export const SUPERVISOR_STAGES: Stage[] = [
  { id: 'scan',     label: 'scan',     status: 'pending' },
  { id: 'think',    label: 'think',    status: 'pending' },
  { id: 'dispatch', label: 'dispatch', status: 'pending' },
  { id: 'monitor',  label: 'monitor',  status: 'pending' },
]

export const QC_STAGES: Stage[] = [
  { id: 'claim_pr',  label: 'claim PR',  status: 'pending' },
  { id: 'read_diff', label: 'read diff', status: 'pending' },
  { id: 'review',    label: 'review',    status: 'pending' },
  { id: 'verdict',   label: 'verdict',   status: 'pending' },
]

export interface LogLine {
  id:                   string   // uuid / tool_call_id — React key
  timestamp_offset_ms:  number   // ms since agent started current task
  tool_name:            string
  argument:             string   // truncated to 40 chars
  full_argument:        string
  status:               'running' | 'success' | 'error' | 'thought'
  elapsed_ms?:          number
}

export interface SparklineWindow {
  window_index: number   // 0–9
  tokens:       number
}

// Doc 05 Phase 2 — `fallback_idle` is distinct from ordinary `idle` and
// signals "Mode 1 fallback engaged but skipped per policy / no provider".
// `mode1_active` flags workers actually running on the Rust AgentEngine.
export type AgentLiveStatus =
  | 'active' | 'idle' | 'error' | 'stopped' | 'starting'
  | 'fallback_idle' | 'mode1_active'
export type AgentType       = 'supervisor' | 'worker' | 'qc' | 'pa'

export interface AgentLiveState {
  agent_id:                  string
  agent_type:                AgentType
  display_name:              string
  status:                    AgentLiveStatus
  stages:                    Stage[]
  current_tool:              string | null
  current_file:              string | null
  current_file_diff?:        string        // e.g. "+10 -0"
  log_lines:                 LogLine[]     // capped at 50
  total_tokens:              number
  sparkline:                 SparklineWindow[]  // always 10 entries
  error_message:             string | null
  last_event_at:             number        // Date.now()
  task_id:                   number | null
  elapsed_since_last_tool_ms: number

  // ── Doc 01 Phase 3 — UI polish fields ─────────────────────────────────────
  // Resolved from workerStore at agent-init time. `null` when the agent is a
  // synthetic subagent slug (no workerStore row) or the model hasn't been
  // wired yet. Displayed as a small badge next to the agent_type chip.
  model:                     string | null
  // Flips to `true` the first time the agent receives ANY live event
  // (tool_start, stage current, status='active'). Used to distinguish
  // "blank-slate idle — never received an event" (gray dot) from
  // "intentionally idle — tick ended cleanly" (blue dot).
  has_ever_been_active:      boolean
}

// ── Tauri event payload types ─────────────────────────────────────────────────

export interface AgentToolCallStartPayload {
  agent_id:     string
  tool_call_id: string
  name:         string
  args:         string
}

export interface AgentToolCallResultPayload {
  agent_id:     string
  tool_call_id: string
  result:       string
  is_error:     boolean
  elapsed_ms:   number
}

export interface AgentStageChangePayload {
  agent_id:   string
  stage:      string
  status:     StageStatus
  elapsed_ms?: number
  reason?:    string
}

export interface AgentTokenUpdatePayload {
  agent_id:      string
  total_tokens:  number
  window_tokens: number
}

export interface AgentStatusChangePayload {
  agent_id: string
  status:   AgentLiveStatus
}

export interface AgentErrorPayload {
  agent_id:     string
  error_message: string
  tool_call_id?: string
  is_fatal:     boolean
}
