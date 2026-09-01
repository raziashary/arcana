// SidecarPanel.tsx — Phase 3.4b
//
// Renders live status for the four AutoDev managed services with per-service
// lifecycle controls (Start/Stop/Restart) and a detail modal that shows
// Resources (CPU%, memory) and Logs (tail) when a service row is clicked.
//
// Managed services:
//   1. memory-mcp-server   — spawned process, can Start/Stop/Restart
//   2. core-orchestrator   — spawned process, can Start/Stop/Restart
//   4. redis / memurai     — externally managed, no lifecycle controls
//
// Data is fed by Tauri commands:
//   - get_full_sidecar_status()     — cached snapshot (fast)
//   - check_sidecar_connectivity()  — live probes (slow)
//   - start_memory_mcp / stop_memory_mcp / start_orchestrator / stop_orchestrator
//   - get_resource_usage(service_id) — sysinfo CPU% + memory (only for spawned processes)
//   - read_sidecar_log(service_id) — tail of sidecar-specific log file

import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import {
  RefreshCw, CheckCircle2, AlertTriangle, Loader2, Circle,
  Play, Square, RotateCcw, Activity, FileText, X, Cpu, HardDrive,
} from 'lucide-react'
import { McpHealthPanel } from './McpHealthPanel'

interface ServiceStatus {
  id:       string
  name:     string
  status:   string   // "stopped" | "starting" | "running" | "error: ..."
  required: boolean
  port:     number
  note:     string
}

interface ResourceSnapshot {
  cpu_percent:  number
  memory_bytes: number
  pid:          number
}

type Tone = 'ok' | 'warn' | 'err' | 'idle'
type ModalTab = 'resources' | 'logs'

function toneOf(status: string, required: boolean): Tone {
  if (status === 'running') return 'ok'
  if (status === 'starting') return 'warn'
  if (status.startsWith('error')) return required ? 'err' : 'warn'
  return 'idle'
}

function IconFor({ tone }: { tone: Tone }) {
  const size = 14
  switch (tone) {
    case 'ok':   return <CheckCircle2 size={size} style={{ color: 'var(--aqua, #7fdbff)' }} />
    case 'warn': return <Loader2       size={size} style={{ color: 'var(--warning, #FFCB8B)' }} className="spin" />
    case 'err':  return <AlertTriangle size={size} style={{ color: 'var(--danger, #ff6b6b)' }} />
    case 'idle': return <Circle        size={size} style={{ color: 'var(--text-3)' }} />
  }
}

function prettyStatus(raw: string): string {
  if (raw === 'running')  return 'Running'
  if (raw === 'starting') return 'Starting…'
  if (raw === 'stopped')  return 'Stopped'
  if (raw.startsWith('error:')) return raw.slice(6).trim() || 'Error'
  return raw
}

function formatBytes(b: number): string {
  if (b < 1024) return `${b} B`
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`
  if (b < 1024 * 1024 * 1024) return `${(b / (1024 * 1024)).toFixed(1)} MB`
  return `${(b / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

/** Returns true if the service is a spawned Python sidecar we can control. */
function isControllable(id: string): boolean {
  return id === 'memory-mcp-server' || id === 'core-orchestrator'
}

export default function SidecarPanel() {
  const [services, setServices]   = useState<ServiceStatus[]>([])
  const [probing,  setProbing]    = useState(false)
  const [error,    setError]      = useState<string | null>(null)

  // Modal state
  const [modalService, setModalService]   = useState<string | null>(null)
  const [modalTab,     setModalTab]       = useState<ModalTab>('resources')
  const [resources,     setResources]       = useState<ResourceSnapshot | null>(null)
  const [resLoading,    setResLoading]    = useState(false)
  const [resError,      setResError]      = useState<string | null>(null)
  const [logContent,    setLogContent]    = useState<string>('')
  const [logLoading,    setLogLoading]    = useState(false)
  const [logError,      setLogError]      = useState<string | null>(null)

  // Action-in-progress state per service id
  const [acting, setActing] = useState<Set<string>>(new Set())

  async function refresh(probe: boolean) {
    try {
      if (probe) {
        setProbing(true)
        await invoke<boolean>('check_sidecar_connectivity')
      }
      const snapshot = await invoke<ServiceStatus[]>('get_full_sidecar_status')
      setServices(snapshot)
      setError(null)
    } catch (e) {
      setError(String(e))
    } finally {
      setProbing(false)
    }
  }

  // Subscribe to MCP port-conflict state for the banner below. The store

  // ─── Modal data loaders ──────────────────────────────────────────────────

  async function loadResources(serviceId: string) {
    setResLoading(true)
    setResError(null)
    try {
      const snap = await invoke<ResourceSnapshot>('get_resource_usage', { serviceId })
      setResources(snap)
    } catch (e) {
      setResources(null)
      setResError(String(e))
    } finally {
      setResLoading(false)
    }
  }

  async function loadLogs(serviceId: string) {
    setLogLoading(true)
    setLogError(null)
    try {
      const text = await invoke<string>('read_sidecar_log', { serviceId, tailLines: 500 })
      setLogContent(text)
    } catch (e) {
      setLogContent('')
      setLogError(String(e))
    } finally {
      setLogLoading(false)
    }
  }

  function openModal(serviceId: string) {
    setModalService(serviceId)
    setModalTab('resources')
    setResources(null)
    setLogContent('')
    void loadResources(serviceId)
  }

  function closeModal() {
    setModalService(null)
  }

  useEffect(() => {
    if (!modalService) return
    if (modalTab === 'resources') {
      void loadResources(modalService)
    } else {
      void loadLogs(modalService)
    }
  }, [modalService, modalTab])

  // ─── Lifecycle actions ───────────────────────────────────────────────────

  async function doAction(serviceId: string, action: 'start' | 'stop' | 'restart') {
    setActing((prev) => {
      const next = new Set(prev)
      next.add(`${serviceId}:${action}`)
      return next
    })
    try {
      if (action === 'restart') {
        // Stop then start
        if (serviceId === 'memory-mcp-server') {
          await invoke('stop_memory_mcp')
          await invoke('start_memory_mcp')
        } else if (serviceId === 'core-orchestrator') {
          await invoke('stop_orchestrator')
          await invoke('start_orchestrator', { path: null as unknown as undefined })
        }
      } else if (action === 'start') {
        if (serviceId === 'memory-mcp-server') {
          await invoke('start_memory_mcp')
        } else if (serviceId === 'core-orchestrator') {
          await invoke('start_orchestrator', { path: null as unknown as undefined })
        }
      } else if (action === 'stop') {
        if (serviceId === 'memory-mcp-server') {
          await invoke('stop_memory_mcp')
        } else if (serviceId === 'core-orchestrator') {
          await invoke('stop_orchestrator')
        }
      }
      // Refresh status after action
      await refresh(true)
    } catch (e) {
      setError(`Action ${action} on ${serviceId} failed: ${String(e)}`)
    } finally {
      setActing((prev) => {
        const next = new Set(prev)
        next.delete(`${serviceId}:${action}`)
        return next
      })
    }
  }

  // ─── Render ──────────────────────────────────────────────────────────────

  return (
    <div
      className="rounded-sm p-4 space-y-3"
      style={{ background: 'var(--bg-card)', border: '1px solid var(--border-1)' }}
    >
      <div className="flex items-center justify-between">
        <div className="font-mono text-xs" style={{ color: 'var(--text-2)' }}>
          Managed services — live status
        </div>
        <button
          type="button"
          onClick={() => { void refresh(true) }}
          disabled={probing}
          className="flex items-center gap-1.5 font-mono px-2 py-1 rounded-sm"
          style={{
            fontSize:   '11px',
            background: 'var(--bg-elevated, #1a1a1a)',
            border:     '1px solid var(--border-1)',
            color:      'var(--text-2)',
            opacity:    probing ? 0.6 : 1,
            cursor:     probing ? 'wait' : 'pointer',
          }}
        >
          <RefreshCw size={12} className={probing ? 'spin' : undefined} />
          Refresh
        </button>
      </div>

      {error && (
        <div className="font-mono text-xs" style={{ color: 'var(--danger, #ff6b6b)' }}>
          {error}
        </div>
      )}


      {/* Sidecar crash banners — render once per service whose status begins
          with `error:`. Pulls the captured stderr tail from the Rust backend
          so the actual Python traceback / config error is shown right there
          in the panel instead of leaving the user to dig through log files. */}
      {services
        .filter(s => s.status.startsWith('error:'))
        .map(s => (
          <SidecarCrashBanner key={`crash-${s.id}`} service={s} />
        ))}

      <ul className="space-y-2" style={{ listStyle: 'none', padding: 0, margin: 0 }}>
        {services.map((s) => {
          const tone      = toneOf(s.status, s.required)
          const canControl = isControllable(s.id)
          const isRunning  = s.status === 'running'
          const isStarting = s.status === 'starting'

          return (
            <li
              key={s.id}
              className="flex items-start gap-3 p-2 rounded-sm"
              style={{
                background: 'var(--bg-elevated, #1a1a1a)',
                border:     '1px solid var(--border-1)',
              }}
            >
              <div style={{ paddingTop: 2, cursor: 'pointer' }} onClick={() => openModal(s.id)}>
                <IconFor tone={tone} />
              </div>
              <div className="flex-1 min-w-0">
                {/* Name + port + optional badge */}
                <div className="flex items-center gap-2 flex-wrap">
                  <span
                    className="font-mono text-xs"
                    style={{ color: 'var(--text-1)', cursor: 'pointer' }}
                    onClick={() => openModal(s.id)}
                    title="Click for Resources & Logs"
                  >
                    {s.name}
                  </span>
                  <span
                    className="font-mono px-1.5 py-0.5 rounded-sm"
                    style={{ fontSize: '10px', background: 'var(--accent-muted)', color: 'var(--text-3)' }}
                  >
                    :{s.port}
                  </span>
                  {!s.required && (
                    <span
                      className="font-mono px-1.5 py-0.5 rounded-sm"
                      style={{ fontSize: '10px', background: 'var(--accent-muted)', color: 'var(--text-3)' }}
                    >
                      optional
                    </span>
                  )}

                  {/* Status label */}
                  <span
                    className="font-mono"
                    style={{
                      fontSize: '11px',
                      color:
                        tone === 'ok'   ? 'var(--aqua, #7fdbff)' :
                        tone === 'warn' ? 'var(--warning, #FFCB8B)' :
                        tone === 'err'  ? 'var(--danger, #ff6b6b)' :
                                          'var(--text-3)',
                    }}
                  >
                    {prettyStatus(s.status)}
                  </span>
                </div>

                {/* Note + action buttons */}
                <div className="flex items-center justify-between gap-2" style={{ marginTop: 2 }}>
                  <div
                    className="font-mono flex-1 min-w-0"
                    style={{ fontSize: '11px', color: 'var(--text-3)' }}
                  >
                    {s.note}
                  </div>

                  {canControl && (
                    <div className="flex items-center gap-1">
                      {/* Start */}
                      {!isRunning && !isStarting && (
                        <button
                          type="button"
                          title="Start"
                          disabled={acting.has(`${s.id}:start`) || acting.has(`${s.id}:restart`)}
                          onClick={() => { void doAction(s.id, 'start') }}
                          className="flex items-center gap-1 font-mono px-1.5 py-0.5 rounded-sm"
                          style={{
                            fontSize:   '10px',
                            background: 'var(--aqua-muted, #7fdbff15)',
                            border:     '1px solid var(--aqua, #7fdbff30)',
                            color:      'var(--aqua, #7fdbff)',
                            cursor:     'pointer',
                          }}
                        >
                          <Play size={10} />
                          Start
                        </button>
                      )}

                      {/* Stop */}
                      {(isRunning || isStarting) && (
                        <button
                          type="button"
                          title="Stop"
                          disabled={acting.has(`${s.id}:stop`) || acting.has(`${s.id}:restart`)}
                          onClick={() => { void doAction(s.id, 'stop') }}
                          className="flex items-center gap-1 font-mono px-1.5 py-0.5 rounded-sm"
                          style={{
                            fontSize:   '10px',
                            background: 'var(--danger-muted, #ff6b6b15)',
                            border:     '1px solid var(--danger, #ff6b6b30)',
                            color:      'var(--danger, #ff6b6b)',
                            cursor:     'pointer',
                          }}
                        >
                          <Square size={10} />
                          Stop
                        </button>
                      )}

                      {/* Restart */}
                      <button
                        type="button"
                        title="Restart"
                        disabled={acting.has(`${s.id}:restart`)}
                        onClick={() => { void doAction(s.id, 'restart') }}
                        className="flex items-center gap-1 font-mono px-1.5 py-0.5 rounded-sm"
                        style={{
                          fontSize:   '10px',
                          background: 'var(--accent-muted)',
                          border:     '1px solid var(--border-1)',
                          color:      'var(--text-2)',
                          cursor:     'pointer',
                        }}
                      >
                        <RotateCcw size={10} />
                        Restart
                      </button>
                    </div>
                  )}
                </div>
              </div>
            </li>
          )
        })}
      </ul>

      <div className="font-mono" style={{ fontSize: '11px', color: 'var(--text-3)' }}>
        Redis is optional — if missing, the warm memory tier is disabled and core-orchestrator
        falls back to cold tier only. On Windows, install{' '}
        <span style={{ color: 'var(--accent)' }}>Memurai</span> and run it as a service.
      </div>

      {/* ─── Ruflo Feature 6 — External MCP Server Health ─────────────── */}
      <McpHealthPanel />

      {/* ─── Detail Modal ─────────────────────────────────────────────── */}
      {modalService && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center p-4"
          style={{ background: 'rgba(0,0,0,0.6)' }}
          onClick={closeModal}
        >
          <div
            className="w-full max-w-lg max-h-[80vh] flex flex-col rounded-sm"
            style={{
              background: 'var(--bg-card)',
              border:     '1px solid var(--border-1)',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex items-center justify-between p-3" style={{ borderBottom: '1px solid var(--border-1)' }}>
              <div className="font-mono text-sm" style={{ color: 'var(--text-1)' }}>
                {services.find((s) => s.id === modalService)?.name ?? modalService}
              </div>
              <button
                type="button"
                aria-label="Close"
                onClick={closeModal}
                className="p-1 rounded-sm"
                style={{ cursor: 'pointer', color: 'var(--text-3)' }}
              >
                <X size={14} />
              </button>
            </div>

            {/* Tabs */}
            <div className="flex gap-1 p-2" style={{ borderBottom: '1px solid var(--border-1)' }}>
              <button
                type="button"
                onClick={() => setModalTab('resources')}
                className="flex items-center gap-1.5 font-mono px-2 py-1 rounded-sm"
                style={{
                  fontSize:   '11px',
                  background: modalTab === 'resources' ? 'var(--accent-muted)' : 'transparent',
                  color:      modalTab === 'resources' ? 'var(--text-1)' : 'var(--text-3)',
                  border:     '1px solid var(--border-1)',
                  cursor:     'pointer',
                }}
              >
                <Activity size={12} />
                Resources
              </button>
              <button
                type="button"
                onClick={() => setModalTab('logs')}
                className="flex items-center gap-1.5 font-mono px-2 py-1 rounded-sm"
                style={{
                  fontSize:   '11px',
                  background: modalTab === 'logs' ? 'var(--accent-muted)' : 'transparent',
                  color:      modalTab === 'logs' ? 'var(--text-1)' : 'var(--text-3)',
                  border:     '1px solid var(--border-1)',
                  cursor:     'pointer',
                }}
              >
                <FileText size={12} />
                Logs
              </button>
            </div>

            {/* Content */}
            <div className="flex-1 overflow-auto p-3">
              {modalTab === 'resources' && (
                <div className="space-y-3">
                  {resLoading && (
                    <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
                      <Loader2 size={12} className="spin inline mr-1" />
                      Sampling process metrics…
                    </div>
                  )}
                  {resError && (
                    <div className="font-mono text-xs" style={{ color: 'var(--danger, #ff6b6b)' }}>
                      {resError}
                    </div>
                  )}
                  {resources && (
                    <div className="space-y-2">
                      <div
                        className="flex items-center gap-3 p-2 rounded-sm"
                        style={{ background: 'var(--bg-elevated)', border: '1px solid var(--border-1)' }}
                      >
                        <Cpu size={14} style={{ color: 'var(--aqua, #7fdbff)' }} />
                        <div>
                          <div className="font-mono text-xs" style={{ color: 'var(--text-2)' }}>CPU Usage</div>
                          <div className="font-mono text-sm" style={{ color: 'var(--text-1)' }}>
                            {resources.cpu_percent.toFixed(1)}%
                          </div>
                        </div>
                      </div>
                      <div
                        className="flex items-center gap-3 p-2 rounded-sm"
                        style={{ background: 'var(--bg-elevated)', border: '1px solid var(--border-1)' }}
                      >
                        <HardDrive size={14} style={{ color: 'var(--warning, #FFCB8B)' }} />
                        <div>
                          <div className="font-mono text-xs" style={{ color: 'var(--text-2)' }}>Memory</div>
                          <div className="font-mono text-sm" style={{ color: 'var(--text-1)' }}>
                            {formatBytes(resources.memory_bytes)}
                          </div>
                        </div>
                      </div>
                      <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
                        PID {resources.pid} — sampled once per modal open.
                      </div>
                    </div>
                  )}
                  {!resources && !resLoading && !resError && (
                    <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
                      No resource data. The process may not be running.
                    </div>
                  )}
                </div>
              )}

              {modalTab === 'logs' && (
                <div className="space-y-2">
                  {logLoading && (
                    <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
                      <Loader2 size={12} className="spin inline mr-1" />
                      Reading log tail…
                    </div>
                  )}
                  {logError && (
                    <div className="font-mono text-xs" style={{ color: 'var(--danger, #ff6b6b)' }}>
                      {logError}
                    </div>
                  )}
                  {logContent ? (
                    <pre
                      className="font-mono"
                      style={{
                        fontSize:   '10px',
                        color:      'var(--text-2)',
                        background: 'var(--bg-deep, #0f0f0f)',
                        padding:    '8px 10px',
                        borderRadius: '4px',
                        overflow:   'auto',
                        maxHeight:  '50vh',
                        whiteSpace: 'pre-wrap',
                        wordBreak:  'break-word',
                      }}
                    >
                      {logContent}
                    </pre>
                  ) : (
                    !logLoading && !logError && (
                      <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
                        No log content. The log file may not exist yet.
                      </div>
                    )
                  )}
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

// ── Sidecar crash banner ──────────────────────────────────────────────────────
//
// Renders inline when a sidecar process is in the `error:` state. Fetches the
// rolling stderr tail captured by the Rust log router and displays the last
// ~15 lines so the user sees the actual Python traceback (e.g. the
// pydantic-settings JSON parse error on CORS_ORIGINS) instead of just
// "Mode 1 fallback" with no explanation. Refresh button lets the user
// re-check after fixing the underlying problem.

function SidecarCrashBanner({ service }: { service: ServiceStatus }) {
  const [tail, setTail] = useState<string[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    setLoading(true)
    try {
      const lines = await invoke<string[]>('get_sidecar_stderr_tail', { id: service.id })
      // Keep only the trailing portion that's likely the actual error —
      // earlier lines are usually log scaffolding (logger config, env loading).
      setTail(lines.slice(-25))
    } catch {
      // command may not exist on older builds; banner still shows the status message
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => { void load() }, [service.id])

  // status format from Rust: "error: <message>"
  const errMsg = service.status.replace(/^error:\s*/, '')

  return (
    <div
      className="font-mono rounded-sm p-3 mb-2"
      style={{
        background: 'rgba(255, 68, 68, 0.06)',
        border:     '1px solid var(--danger, #ff4444)',
        color:      'var(--text-1)',
        fontSize:   '11px',
        lineHeight: 1.5,
      }}
    >
      <div className="flex items-center gap-2 mb-1" style={{ color: 'var(--danger, #ff4444)' }}>
        <AlertTriangle size={12} />
        <span style={{ fontWeight: 600, letterSpacing: '0.05em', textTransform: 'uppercase' }}>
          {service.name} crashed
        </span>
      </div>
      <div style={{ color: 'var(--text-2)' }}>{errMsg}</div>
      {tail.length > 0 && (
        <pre
          className="mt-2"
          style={{
            background:  'var(--bg-deep, #0f0f0f)',
            color:       'var(--text-3)',
            fontSize:    '10px',
            padding:     '8px 10px',
            borderRadius: 4,
            maxHeight:   '180px',
            overflow:    'auto',
            whiteSpace:  'pre-wrap',
            wordBreak:   'break-word',
          }}
        >
          {tail.join('\n')}
        </pre>
      )}
      <div className="flex gap-2 mt-2">
        <button
          type="button"
          onClick={() => { void load() }}
          disabled={loading}
          className="font-mono px-2 py-1 rounded-sm"
          style={{
            fontSize:   '10px',
            background: 'var(--bg-elevated, #1a1a1a)',
            border:     '1px solid var(--border-1)',
            color:      'var(--text-2)',
            cursor:     loading ? 'wait' : 'pointer',
          }}
        >
          Refresh trace
        </button>
      </div>
    </div>
  )
}
