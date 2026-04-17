// HooksSettings — full hooks management panel in the Settings tab.
// Lists hooks, allows enable/disable/delete, add new hooks, and view logs.
// Phase 3.1

import { useEffect, useState } from 'react'
import { Plus, Trash2, ToggleLeft, ToggleRight, ChevronDown, ChevronUp, ScrollText, Play, HelpCircle } from 'lucide-react'
import { useHookStore, type AddHookInput, type HookConfig, type HookType } from '../../stores/hookStore'
import HookLogViewer from '../hooks/HookLogViewer'
import HooksInfoModal from '../hooks/HooksInfoModal'

const HOOK_TYPES: HookType[] = [
  'session_start', 'session_end', 'agent_loop_start', 'agent_loop_end', 'on_error',
  'pre_tool_use', 'post_tool_use', 'on_tool_error', 'on_permission_request',
  'pre_file_read', 'post_file_write', 'pre_file_delete',
  'on_checkpoint_create', 'on_checkpoint_restore',
  'pre_commit', 'post_commit', 'pre_push', 'on_branch_switch', 'on_merge_conflict',
  'pre_plan_step', 'post_plan_step', 'on_plan_activate', 'on_plan_complete', 'on_plan_abandon',
  'on_task_claim', 'on_task_complete', 'on_task_fail',
  'on_qc_review_start', 'on_qc_approve', 'on_qc_fail',
]

export default function HooksSettings() {
  const store = useHookStore()
  const [showAddForm, setShowAddForm] = useState(false)
  const [showLogs,    setShowLogs]    = useState(false)
  const [showInfo,    setShowInfo]    = useState(false)
  const [expandedId,  setExpandedId]  = useState<string | null>(null)

  useEffect(() => { store.loadHooks() }, [])

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <div className="font-mono text-sm font-semibold" style={{ color: 'var(--text-1)' }}>
            Hooks
          </div>
          <div className="text-xs mt-0.5" style={{ color: 'var(--text-3)' }}>
            Event-driven actions fired at agent lifecycle points
          </div>
        </div>
        <div className="flex items-center gap-2">
          <button
            title="Learn about hooks"
            onClick={() => setShowInfo(true)}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-mono"
            style={{ background: 'var(--bg-2)', color: 'var(--text-2)', border: '1px solid var(--border)' }}
          >
            <HelpCircle size={12} />
            What is Hooks?
          </button>
          <button
            title="View execution logs"
            onClick={() => setShowLogs(l => !l)}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-mono"
            style={{ background: 'var(--bg-2)', color: 'var(--text-2)', border: '1px solid var(--border)' }}
          >
            <ScrollText size={12} />
            Logs
          </button>
          <button
            title="Add hook"
            onClick={() => setShowAddForm(f => !f)}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-mono"
            style={{ background: 'var(--accent)', color: '#011627' }}
          >
            <Plus size={12} />
            Add Hook
          </button>
        </div>
      </div>

      {/* Add form */}
      {showAddForm && (
        <AddHookForm
          onAdd={async input => { await store.addHook(input); setShowAddForm(false) }}
          onCancel={() => setShowAddForm(false)}
        />
      )}

      {/* Log viewer */}
      {showLogs && (
        <div
          className="rounded-lg p-4"
          style={{ background: 'var(--bg-2)', border: '1px solid var(--border)' }}
        >
          <HookLogViewer limit={50} />
        </div>
      )}

      {/* Hook list */}
      {store.hooks.length === 0 ? (
        <div
          className="rounded-lg p-6 text-center text-xs font-mono"
          style={{ background: 'var(--bg-2)', border: '1px solid var(--border)', color: 'var(--text-3)' }}
        >
          No hooks configured. Add one to automate actions at lifecycle events.
        </div>
      ) : (
        <div className="space-y-2">
          {store.hooks.map(hook => (
            <HookRow
              key={hook.id}
              hook={hook}
              expanded={expandedId === hook.id}
              onToggleExpand={() => setExpandedId(id => id === hook.id ? null : hook.id)}
              onEnable={()  => store.enableHook(hook.id)}
              onDisable={() => store.disableHook(hook.id)}
              onRemove={() => store.removeHook(hook.id)}
            />
          ))}
        </div>
      )}

      {store.error && (
        <div className="text-xs p-3 rounded" style={{ background: 'rgba(255,99,99,0.1)', color: 'var(--danger)' }}>
          {store.error}
        </div>
      )}

      {showInfo && <HooksInfoModal onClose={() => setShowInfo(false)} />}
    </div>
  )
}

// ── HookRow ───────────────────────────────────────────────────────────────────

function HookRow({
  hook, expanded, onToggleExpand, onEnable, onDisable, onRemove,
}: {
  hook:            HookConfig
  expanded:        boolean
  onToggleExpand:  () => void
  onEnable:        () => void
  onDisable:       () => void
  onRemove:        () => void
}) {
  const store = useHookStore()
  const [testResult, setTestResult] = useState<{ outcome: string; output?: string; error?: string } | null>(null)
  const [testing,    setTesting]    = useState(false)

  async function handleTest() {
    setTesting(true)
    setTestResult(null)
    try {
      const logs = await store.testHook(hook.id, {})
      const log  = logs[0]
      setTestResult({ outcome: log.outcome, output: log.output, error: log.error })
    } catch (e) {
      setTestResult({ outcome: 'failed', error: String(e) })
    } finally {
      setTesting(false)
    }
  }
  return (
    <div
      className="rounded-lg overflow-hidden"
      style={{ background: 'var(--bg-2)', border: '1px solid var(--border)' }}
    >
      <div className="flex items-center gap-3 px-4 py-3">
        {/* Enable toggle */}
        <button
          title={hook.enabled ? 'Disable hook' : 'Enable hook'}
          onClick={hook.enabled ? onDisable : onEnable}
          className="shrink-0"
        >
          {hook.enabled
            ? <ToggleRight size={18} style={{ color: 'var(--aqua)' }} />
            : <ToggleLeft  size={18} style={{ color: 'var(--text-3)' }} />
          }
        </button>

        {/* Info */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-xs font-mono font-semibold" style={{ color: 'var(--text-1)' }}>
              {hook.name}
            </span>
            <span
              className="text-xs px-1.5 rounded"
              style={{ background: 'var(--bg-selected)', color: 'var(--accent)', fontFamily: 'monospace' }}
            >
              {hook.hook_type}
            </span>
            <span className="text-xs" style={{ color: 'var(--text-3)' }}>
              p{hook.priority}
            </span>
          </div>
          <div className="text-xs mt-0.5 truncate" style={{ color: 'var(--text-3)' }}>
            {actionSummary(hook)}
          </div>
        </div>

        {/* Actions */}
        <div className="flex items-center gap-1 shrink-0">
          <button
            title="Test hook with empty context"
            onClick={handleTest}
            disabled={testing}
            className="p-1.5 rounded hover:opacity-70 disabled:opacity-40"
          >
            <Play size={12} style={{ color: 'var(--accent)' }} />
          </button>
          <button
            title={expanded ? 'Collapse' : 'Expand details'}
            onClick={onToggleExpand}
            className="p-1.5 rounded hover:opacity-70"
          >
            {expanded
              ? <ChevronUp   size={13} style={{ color: 'var(--text-3)' }} />
              : <ChevronDown size={13} style={{ color: 'var(--text-3)' }} />
            }
          </button>
          <button
            title="Remove hook"
            onClick={onRemove}
            className="p-1.5 rounded hover:opacity-70"
          >
            <Trash2 size={13} style={{ color: 'var(--danger)' }} />
          </button>
        </div>
      </div>

      {/* Inline test result */}
      {testResult && (
        <div
          className="px-4 py-2 text-xs font-mono flex items-start gap-2"
          style={{
            borderTop:  '1px solid var(--border)',
            background: testResult.outcome === 'executed' ? 'rgba(173,219,103,0.06)' : 'rgba(255,99,99,0.06)',
          }}
        >
          <span
            className="shrink-0 uppercase font-semibold"
            style={{ color: testResult.outcome === 'executed' ? 'var(--success)' : 'var(--danger)', fontSize: '10px' }}
          >
            TEST: {testResult.outcome}
          </span>
          {testResult.output && (
            <span style={{ color: 'var(--text-2)', fontSize: '10px' }} className="truncate">{testResult.output}</span>
          )}
          {testResult.error && (
            <span style={{ color: 'var(--danger)', fontSize: '10px' }} className="truncate">{testResult.error}</span>
          )}
          <button
            title="Dismiss test result"
            onClick={() => setTestResult(null)}
            className="ml-auto shrink-0 hover:opacity-70"
            style={{ color: 'var(--text-3)', fontSize: '10px' }}
          >
            ✕
          </button>
        </div>
      )}

      {/* Expanded detail */}
      {expanded && (
        <div
          className="px-4 pb-4 pt-0 text-xs font-mono space-y-1"
          style={{ borderTop: '1px solid var(--border)', color: 'var(--text-3)' }}
        >
          <div className="pt-3">
            <span style={{ color: 'var(--text-2)' }}>Action: </span>
            <pre className="inline whitespace-pre-wrap" style={{ color: 'var(--text-1)' }}>
              {JSON.stringify(hook.action, null, 2)}
            </pre>
          </div>
          {Object.keys(hook.conditions).length > 0 && (
            <div>
              <span style={{ color: 'var(--text-2)' }}>Conditions: </span>
              <pre className="inline whitespace-pre-wrap" style={{ color: 'var(--text-1)' }}>
                {JSON.stringify(hook.conditions, null, 2)}
              </pre>
            </div>
          )}
          <div style={{ color: 'var(--text-3)' }}>
            id: {hook.id} · created: {hook.created_at.slice(0, 10)}
          </div>
          <div className="pt-1">
            <HookLogViewer hookId={hook.id} limit={10} />
          </div>
        </div>
      )}
    </div>
  )
}

function actionSummary(hook: HookConfig): string {
  const a = hook.action
  switch (a.type) {
    case 'shell':        return `shell: ${a.command.slice(0, 60)}`
    case 'http':         return `${a.method} ${a.url.slice(0, 60)}`
    case 'notification': return `notify: ${a.title}`
    case 'log':          return `log → ${a.file}`
    case 'block':        return `block: ${a.message ?? 'action blocked'}`
    case 'emit_event':   return `event: ${a.event_name}`
    default:             return JSON.stringify(a).slice(0, 60)
  }
}

// ── AddHookForm ───────────────────────────────────────────────────────────────

function AddHookForm({ onAdd, onCancel }: { onAdd: (i: AddHookInput) => Promise<void>; onCancel: () => void }) {
  const [name,      setName]      = useState('')
  const [hookType,  setHookType]  = useState<HookType>('post_file_write')
  const [actionType, setActionType] = useState<'shell' | 'http' | 'notification' | 'log' | 'block' | 'emit_event'>('shell')
  const [command,   setCommand]   = useState('')
  const [url,       setUrl]       = useState('')
  const [method,    setMethod]    = useState('POST')
  const [notifTitle, setNotifTitle] = useState('')
  const [notifMsg,  setNotifMsg]  = useState('')
  const [logFile,   setLogFile]   = useState('')
  const [logMsg,    setLogMsg]    = useState('{{hook_type}} fired')
  const [blockMsg,  setBlockMsg]  = useState('')
  const [eventName, setEventName] = useState('')
  const [saving,    setSaving]    = useState(false)
  const [error,     setError]     = useState<string | null>(null)

  async function handleSave() {
    if (!name.trim()) { setError('Name is required'); return }
    setSaving(true)
    setError(null)
    try {
      const action = buildAction()
      if (!action) { setError('Fill in all action fields'); setSaving(false); return }
      await onAdd({ name, hook_type: hookType, action })
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }

  function buildAction(): AddHookInput['action'] | null {
    switch (actionType) {
      case 'shell':        return command ? { type: 'shell', command } : null
      case 'http':         return url    ? { type: 'http', url, method } : null
      case 'notification': return notifTitle ? { type: 'notification', title: notifTitle, message: notifMsg } : null
      case 'log':          return logFile ? { type: 'log', file: logFile, message: logMsg } : null
      case 'block':        return { type: 'block', message: blockMsg || undefined }
      case 'emit_event':   return eventName ? { type: 'emit_event', event_name: eventName } : null
      default:             return null
    }
  }

  const inputStyle = {
    background: 'var(--bg-input)', border: '1px solid var(--border)',
    color: 'var(--text-1)', borderRadius: '4px', padding: '6px 10px',
    fontSize: '12px', width: '100%', fontFamily: 'monospace',
  }
  const labelStyle = { color: 'var(--text-3)', fontSize: '11px', fontFamily: 'monospace' }

  return (
    <div
      className="rounded-lg p-4 space-y-4"
      style={{ background: 'var(--bg-2)', border: '1px solid var(--accent)' }}
    >
      <div className="font-mono text-xs font-semibold" style={{ color: 'var(--accent)' }}>
        New Hook
      </div>

      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <label style={labelStyle}>Name *</label>
          <input value={name} onChange={e => setName(e.target.value)} style={inputStyle} placeholder="auto-format on save" />
        </div>
        <div className="space-y-1">
          <label style={labelStyle}>Event type *</label>
          <select title="Event type" value={hookType} onChange={e => setHookType(e.target.value as HookType)} style={inputStyle}>
            {HOOK_TYPES.map(t => <option key={t} value={t}>{t}</option>)}
          </select>
        </div>
      </div>

      <div className="space-y-1">
        <label style={labelStyle}>Action type *</label>
        <select title="Action type" value={actionType} onChange={e => setActionType(e.target.value as typeof actionType)} style={inputStyle}>
          <option value="shell">Shell command</option>
          <option value="http">HTTP request</option>
          <option value="notification">Desktop notification</option>
          <option value="log">Log to file</option>
          <option value="block">Block action</option>
          <option value="emit_event">Emit Tauri event</option>
        </select>
      </div>

      {/* Action-specific fields */}
      {actionType === 'shell' && (
        <div className="space-y-1">
          <label style={labelStyle}>Command</label>
          <input value={command} onChange={e => setCommand(e.target.value)} style={inputStyle} placeholder="cargo fmt" />
        </div>
      )}
      {actionType === 'http' && (
        <div className="grid grid-cols-3 gap-2">
          <div className="space-y-1">
            <label style={labelStyle}>Method</label>
            <select title="HTTP method" value={method} onChange={e => setMethod(e.target.value)} style={inputStyle}>
              {['GET','POST','PUT','PATCH','DELETE'].map(m => <option key={m} value={m}>{m}</option>)}
            </select>
          </div>
          <div className="col-span-2 space-y-1">
            <label style={labelStyle}>URL</label>
            <input value={url} onChange={e => setUrl(e.target.value)} style={inputStyle} placeholder="https://webhook.example.com/hook" />
          </div>
        </div>
      )}
      {actionType === 'notification' && (
        <div className="grid grid-cols-2 gap-2">
          <div className="space-y-1">
            <label style={labelStyle}>Title</label>
            <input value={notifTitle} onChange={e => setNotifTitle(e.target.value)} style={inputStyle} placeholder="AutoDev Alert" />
          </div>
          <div className="space-y-1">
            <label style={labelStyle}>Message</label>
            <input value={notifMsg} onChange={e => setNotifMsg(e.target.value)} style={inputStyle} placeholder="{{hook_type}} fired" />
          </div>
        </div>
      )}
      {actionType === 'log' && (
        <div className="grid grid-cols-2 gap-2">
          <div className="space-y-1">
            <label style={labelStyle}>File path</label>
            <input value={logFile} onChange={e => setLogFile(e.target.value)} style={inputStyle} placeholder="~/.autodev/logs/hooks.log" />
          </div>
          <div className="space-y-1">
            <label style={labelStyle}>Message template</label>
            <input title="Log message template" value={logMsg} onChange={e => setLogMsg(e.target.value)} style={inputStyle} placeholder="{{hook_type}} fired" />
          </div>
        </div>
      )}
      {actionType === 'block' && (
        <div className="space-y-1">
          <label style={labelStyle}>Block message (optional)</label>
          <input title="Block message" value={blockMsg} onChange={e => setBlockMsg(e.target.value)} style={inputStyle} placeholder="This action is not allowed" />
        </div>
      )}
      {actionType === 'emit_event' && (
        <div className="space-y-1">
          <label style={labelStyle}>Event name</label>
          <input value={eventName} onChange={e => setEventName(e.target.value)} style={inputStyle} placeholder="autodev:hook-fired" />
        </div>
      )}

      {error && <div className="text-xs" style={{ color: 'var(--danger)' }}>{error}</div>}

      <div className="flex justify-end gap-2">
        <button
          onClick={onCancel}
          className="px-3 py-1.5 rounded text-xs font-mono"
          style={{ background: 'var(--bg-panel)', color: 'var(--text-2)', border: '1px solid var(--border)' }}
        >
          Cancel
        </button>
        <button
          onClick={handleSave}
          disabled={saving}
          className="px-4 py-1.5 rounded text-xs font-mono font-semibold"
          style={{ background: 'var(--accent)', color: '#011627', opacity: saving ? 0.6 : 1 }}
        >
          {saving ? 'Saving…' : 'Save Hook'}
        </button>
      </div>
    </div>
  )
}
