// HooksSettings — full hooks management panel in the Settings tab.
// Lists hooks, allows enable/disable/delete, add new hooks, view logs.
// Includes: template gallery (pre-built hooks) + mock-context test interface.
// Phase 3.1

import { useEffect, useState } from 'react'
import {
  Plus, Trash2, ToggleLeft, ToggleRight, ChevronDown, ChevronUp,
  ScrollText, Play, HelpCircle, LayoutGrid, X, CheckCircle, AlertCircle,
  ShieldCheck, Zap, Bell, FileText,
} from 'lucide-react'
import {
  useHookStore,
  type AddHookInput, type HookConfig, type HookType,
  type HookTemplate,
} from '../../stores/hookStore'
import HookLogViewer from '../hooks/HookLogViewer'
import HooksInfoModal from '../hooks/HooksInfoModal'

// ── Static template definitions ───────────────────────────────────────────────

const HOOK_TEMPLATES: HookTemplate[] = [
  // Safety
  {
    id: 'block-rm-rf',
    name: 'Block rm -rf /',
    description: 'Prevents accidental root filesystem deletion via shell commands.',
    category: 'safety',
    hook_type: 'pre_tool_use',
    conditions: { command_pattern: 'rm\\s+-rf\\s+/' },
    action: { type: 'block', message: 'Destructive root deletion blocked by Arcana' },
  },
  {
    id: 'block-force-push',
    name: 'Block force push',
    description: 'Stops --force pushes to any remote branch to prevent history loss.',
    category: 'safety',
    hook_type: 'pre_push',
    conditions: { command_pattern: '--force' },
    action: { type: 'block', message: 'Force push is disabled. Use --force-with-lease if needed.' },
  },
  {
    id: 'block-env-commit',
    name: 'Block .env file commits',
    description: 'Prevents accidental commit of .env or secrets files.',
    category: 'safety',
    hook_type: 'pre_commit',
    conditions: { file_pattern: '**/.env*' },
    action: { type: 'block', message: '.env files must not be committed. Add to .gitignore.' },
  },
  // Automation
  {
    id: 'auto-fmt-rust',
    name: 'Auto-format Rust on save',
    description: 'Runs `cargo fmt` whenever a .rs file is written.',
    category: 'automation',
    hook_type: 'post_file_write',
    conditions: { file_pattern: '**/*.rs' },
    action: { type: 'shell', command: 'cargo fmt' },
  },
  {
    id: 'auto-fmt-ts',
    name: 'Auto-format TypeScript on save',
    description: 'Runs prettier on .ts/.tsx files after every write.',
    category: 'automation',
    hook_type: 'post_file_write',
    conditions: { file_pattern: '**/*.{ts,tsx}' },
    action: { type: 'shell', command: 'npx prettier --write "$file_path"' },
  },
  {
    id: 'lint-on-commit',
    name: 'Clippy lint on commit',
    description: 'Runs `cargo clippy` before each git commit.',
    category: 'automation',
    hook_type: 'pre_commit',
    conditions: {},
    action: { type: 'shell', command: 'cargo clippy -- -D warnings', timeout_secs: 60 },
  },
  {
    id: 'webhook-on-plan-complete',
    name: 'Webhook on plan complete',
    description: 'Posts to your webhook when a plan finishes.',
    category: 'automation',
    hook_type: 'on_plan_complete',
    conditions: {},
    action: { type: 'http', url: 'https://hooks.example.com/arcana', method: 'POST' },
  },
  // Notifications
  {
    id: 'notify-task-done',
    name: 'Notify on task complete',
    description: 'Desktop notification when a fleet worker finishes a task.',
    category: 'notifications',
    hook_type: 'on_task_complete',
    conditions: {},
    action: { type: 'notification', title: 'Task Complete', message: 'A worker finished its task.' },
  },
  {
    id: 'notify-qc-fail',
    name: 'Notify on QC failure',
    description: 'Desktop alert when quality control rejects a result.',
    category: 'notifications',
    hook_type: 'on_qc_fail',
    conditions: {},
    action: { type: 'notification', title: 'QC Failed', message: 'Review required — QC rejected the output.' },
  },
  {
    id: 'notify-error',
    name: 'Notify on agent error',
    description: 'Desktop alert when the agent encounters an error.',
    category: 'notifications',
    hook_type: 'on_error',
    conditions: {},
    action: { type: 'notification', title: 'Arcana Error', message: 'Agent encountered an error. Check the logs.' },
  },
  // Logging
  {
    id: 'log-tool-calls',
    name: 'Log all tool calls',
    description: 'Appends every tool call to a persistent log file.',
    category: 'logging',
    hook_type: 'pre_tool_use',
    conditions: {},
    action: { type: 'log', file: '~/.arcana/logs/tools.log', message: '[{{hook_type}}] {{tool_name}}: {{command}}' },
  },
  {
    id: 'log-errors',
    name: 'Log errors to file',
    description: 'Captures all agent errors in a dedicated error log.',
    category: 'logging',
    hook_type: 'on_error',
    conditions: {},
    action: { type: 'log', file: '~/.arcana/logs/errors.log', message: '[{{hook_type}}] {{session_id}}: error' },
  },
  {
    id: 'log-git-events',
    name: 'Log git activity',
    description: 'Records all commits and pushes with timestamps.',
    category: 'logging',
    hook_type: 'post_commit',
    conditions: {},
    action: { type: 'log', file: '~/.arcana/logs/git.log', message: '[{{hook_type}}] commit by session {{session_id}}' },
  },
]

const CATEGORY_META = {
  safety:        { label: 'Safety',        color: 'var(--danger)',  icon: ShieldCheck },
  automation:    { label: 'Automation',    color: 'var(--aqua)',    icon: Zap         },
  notifications: { label: 'Notifications', color: 'var(--purple)',  icon: Bell        },
  logging:       { label: 'Logging',       color: 'var(--text-3)',  icon: FileText    },
} as const

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

// ── HooksSettings ─────────────────────────────────────────────────────────────

export default function HooksSettings() {
  const store = useHookStore()
  const [showAddForm,   setShowAddForm]   = useState(false)
  const [showLogs,      setShowLogs]      = useState(false)
  const [showInfo,      setShowInfo]      = useState(false)
  const [showTemplates, setShowTemplates] = useState(false)
  const [expandedId,    setExpandedId]    = useState<string | null>(null)
  const [testHookId,    setTestHookId]    = useState<string | null>(null)

  useEffect(() => { store.loadHooks() }, [])

  const testHook = testHookId ? store.hooks.find(h => h.id === testHookId) : null

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
            title="Browse pre-built hook templates"
            onClick={() => setShowTemplates(f => !f)}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-mono"
            style={{
              background: showTemplates ? 'var(--accent-muted)' : 'var(--bg-2)',
              color:      showTemplates ? 'var(--accent)'       : 'var(--text-2)',
              border: `1px solid ${showTemplates ? 'var(--accent-border)' : 'var(--border)'}`,
            }}
          >
            <LayoutGrid size={12} />
            Templates
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

      {/* Template gallery */}
      {showTemplates && (
        <HookTemplateGallery
          installedNames={store.hooks.map(h => h.name)}
          onInstall={async tpl => {
            await store.addHook({
              name:       tpl.name,
              hook_type:  tpl.hook_type,
              conditions: tpl.conditions,
              action:     tpl.action,
            })
            await store.loadHooks()
          }}
        />
      )}

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
          No hooks configured. Add one or browse Templates to get started.
        </div>
      ) : (
        <div className="space-y-2">
          {store.hooks.map(hook => (
            <HookRow
              key={hook.id}
              hook={hook}
              expanded={expandedId === hook.id}
              onToggleExpand={() => setExpandedId(id => id === hook.id ? null : hook.id)}
              onEnable={()    => store.enableHook(hook.id)}
              onDisable={()   => store.disableHook(hook.id)}
              onRemove={()    => store.removeHook(hook.id)}
              onOpenTest={()  => setTestHookId(hook.id)}
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

      {/* Test modal */}
      {testHook && (
        <HookTestModal
          hook={testHook}
          onClose={() => setTestHookId(null)}
        />
      )}
    </div>
  )
}

// ── HookTemplateGallery ───────────────────────────────────────────────────────

function HookTemplateGallery({
  installedNames, onInstall,
}: {
  installedNames: string[]
  onInstall:      (tpl: HookTemplate) => Promise<void>
}) {
  const [installing, setInstalling] = useState<string | null>(null)
  const [filter,     setFilter]     = useState<string>('all')

  const categories = ['all', 'safety', 'automation', 'notifications', 'logging'] as const
  const filtered = filter === 'all'
    ? HOOK_TEMPLATES
    : HOOK_TEMPLATES.filter(t => t.category === filter)

  return (
    <div
      className="rounded-lg p-4 space-y-3"
      style={{ background: 'var(--bg-2)', border: '1px solid var(--accent-border)' }}
    >
      <div className="flex items-center justify-between">
        <span className="font-mono text-xs font-semibold" style={{ color: 'var(--accent)' }}>
          Template Gallery — {HOOK_TEMPLATES.length} pre-built hooks
        </span>
        {/* Category filter pills */}
        <div className="flex gap-1">
          {categories.map(cat => (
            <button
              key={cat}
              onClick={() => setFilter(cat)}
              className="px-2.5 py-0.5 rounded-full text-xs font-mono capitalize"
              style={{
                background: filter === cat ? 'var(--accent)'       : 'var(--bg-panel)',
                color:      filter === cat ? '#011627'             : 'var(--text-3)',
                border:     filter === cat ? 'none'                : '1px solid var(--border)',
              }}
            >
              {cat}
            </button>
          ))}
        </div>
      </div>

      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
        {filtered.map(tpl => {
          const meta      = CATEGORY_META[tpl.category]
          const Icon      = meta.icon
          const installed = installedNames.includes(tpl.name)
          return (
            <div
              key={tpl.id}
              className="rounded p-3 flex items-start gap-3"
              style={{ background: 'var(--bg-panel)', border: '1px solid var(--border)' }}
            >
              <Icon size={14} style={{ color: meta.color, marginTop: 2, flexShrink: 0 }} />
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 flex-wrap">
                  <span className="text-xs font-mono font-semibold" style={{ color: 'var(--text-1)' }}>
                    {tpl.name}
                  </span>
                  <span
                    className="text-xs px-1.5 rounded"
                    style={{ background: 'var(--bg-selected)', color: 'var(--accent)', fontFamily: 'monospace', fontSize: '10px' }}
                  >
                    {tpl.hook_type}
                  </span>
                </div>
                <p className="text-xs mt-0.5 leading-snug" style={{ color: 'var(--text-3)' }}>
                  {tpl.description}
                </p>
              </div>
              <button
                disabled={installed || installing === tpl.id}
                onClick={async () => {
                  setInstalling(tpl.id)
                  try { await onInstall(tpl) } finally { setInstalling(null) }
                }}
                className="shrink-0 text-xs px-2.5 py-1 rounded font-mono disabled:opacity-40"
                style={{
                  background: installed ? 'var(--bg-selected)' : 'var(--accent)',
                  color:      installed ? 'var(--success)'     : '#011627',
                }}
              >
                {installed ? '✓ Installed' : installing === tpl.id ? '…' : 'Install'}
              </button>
            </div>
          )
        })}
      </div>
    </div>
  )
}

// ── HookTestModal ─────────────────────────────────────────────────────────────

function HookTestModal({ hook, onClose }: { hook: HookConfig; onClose: () => void }) {
  const store = useHookStore()

  // Context fields
  const [toolName,   setToolName]   = useState('')
  const [command,    setCommand]    = useState('')
  const [filePath,   setFilePath]   = useState('')
  const [agentMode,  setAgentMode]  = useState('')
  const [sessionId,  setSessionId]  = useState('test-session-1')
  const [workerId,   setWorkerId]   = useState('')
  const [taskId,     setTaskId]     = useState('')
  const [isError,    setIsError]    = useState(false)

  const [running,    setRunning]    = useState(false)
  const [result,     setResult]     = useState<{ outcome: string; output?: string; error?: string } | null>(null)

  async function runTest() {
    setRunning(true)
    setResult(null)
    try {
      const ctx = {
        hook_type:  hook.hook_type,
        timestamp:  new Date().toISOString(),
        session_id: sessionId || null,
        agent_mode: agentMode || null,
        file_path:  filePath  || null,
        task_id:    taskId    || null,
        worker_id:  workerId  || null,
        tool_call:  toolName ? {
          id:       `test-${Date.now()}`,
          name:     toolName,
          input:    { command },
          result:   null,
          is_error: isError,
        } : null,
        extra: {},
      }
      const logs = await store.testHook(hook.id, ctx as Record<string, unknown>)
      const log  = logs[0]
      setResult({ outcome: log.outcome, output: log.output, error: log.error })
    } catch (e) {
      setResult({ outcome: 'failed', error: String(e) })
    } finally {
      setRunning(false)
    }
  }

  const inp = {
    background: 'var(--bg-input)', border: '1px solid var(--border)',
    color: 'var(--text-1)', borderRadius: '4px', padding: '5px 8px',
    fontSize: '11px', width: '100%', fontFamily: 'monospace',
  }
  const lbl = { color: 'var(--text-3)', fontSize: '10px', fontFamily: 'monospace' }

  return (
    /* Backdrop */
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: 'rgba(0,0,0,0.55)' }}
      onClick={e => { if (e.target === e.currentTarget) onClose() }}
    >
      <div
        className="rounded-xl shadow-2xl w-full max-w-lg mx-4 overflow-hidden"
        style={{ background: 'var(--bg-panel)', border: '1px solid var(--border)' }}
      >
        {/* Header */}
        <div
          className="flex items-center justify-between px-5 py-4"
          style={{ borderBottom: '1px solid var(--border)' }}
        >
          <div>
            <div className="font-mono text-sm font-semibold" style={{ color: 'var(--text-1)' }}>
              Test: {hook.name}
            </div>
            <div className="text-xs mt-0.5" style={{ color: 'var(--text-3)' }}>
              Build a mock context and fire the hook against the real executor
            </div>
          </div>
          <button onClick={onClose} className="p-1.5 rounded hover:opacity-70">
            <X size={14} style={{ color: 'var(--text-3)' }} />
          </button>
        </div>

        {/* Form */}
        <div className="px-5 py-4 space-y-3">
          {/* Hook meta — read-only */}
          <div className="flex items-center gap-2 text-xs font-mono">
            <span style={{ color: 'var(--text-3)' }}>Event:</span>
            <span
              className="px-1.5 rounded"
              style={{ background: 'var(--bg-selected)', color: 'var(--accent)' }}
            >
              {hook.hook_type}
            </span>
            <span style={{ color: 'var(--text-3)' }}>Action:</span>
            <span style={{ color: 'var(--text-2)' }}>{hook.action.type}</span>
          </div>

          {/* Tool context */}
          <div className="grid grid-cols-2 gap-2">
            <div className="space-y-1">
              <label style={lbl}>tool_name</label>
              <input value={toolName} onChange={e => setToolName(e.target.value)} style={inp} placeholder="bash" />
            </div>
            <div className="space-y-1">
              <label style={lbl}>command (tool input)</label>
              <input value={command} onChange={e => setCommand(e.target.value)} style={inp} placeholder="cargo build" />
            </div>
          </div>

          {/* File + agent */}
          <div className="grid grid-cols-2 gap-2">
            <div className="space-y-1">
              <label style={lbl}>file_path</label>
              <input value={filePath} onChange={e => setFilePath(e.target.value)} style={inp} placeholder="/repo/src/main.rs" />
            </div>
            <div className="space-y-1">
              <label style={lbl}>agent_mode</label>
              <select
                title="agent_mode"
                value={agentMode}
                onChange={e => setAgentMode(e.target.value)}
                style={inp}
              >
                <option value="">(none)</option>
                <option value="fast_dev">fast_dev</option>
                <option value="review">review</option>
                <option value="delegate">delegate</option>
                <option value="pa">pa</option>
                <option value="fleet">fleet</option>
              </select>
            </div>
          </div>

          {/* IDs */}
          <div className="grid grid-cols-3 gap-2">
            <div className="space-y-1">
              <label style={lbl}>session_id</label>
              <input value={sessionId} onChange={e => setSessionId(e.target.value)} style={inp} placeholder="ses-1" />
            </div>
            <div className="space-y-1">
              <label style={lbl}>worker_id</label>
              <input value={workerId} onChange={e => setWorkerId(e.target.value)} style={inp} placeholder="w-dev" />
            </div>
            <div className="space-y-1">
              <label style={lbl}>task_id</label>
              <input value={taskId} onChange={e => setTaskId(e.target.value)} style={inp} placeholder="task-1" />
            </div>
          </div>

          {/* is_error toggle */}
          <label className="flex items-center gap-2 cursor-pointer text-xs font-mono" style={{ color: 'var(--text-2)' }}>
            <input
              type="checkbox"
              checked={isError}
              onChange={e => setIsError(e.target.checked)}
              className="w-3.5 h-3.5"
            />
            is_error (simulate a tool error context)
          </label>

          {/* Result */}
          {result && (
            <div
              className="rounded p-3 flex items-start gap-2 text-xs font-mono"
              style={{
                background: result.outcome === 'executed'
                  ? 'rgba(173,219,103,0.08)'
                  : result.outcome === 'skipped'
                  ? 'rgba(130,170,255,0.08)'
                  : 'rgba(255,99,99,0.08)',
                border: `1px solid ${
                  result.outcome === 'executed' ? 'rgba(173,219,103,0.25)'
                  : result.outcome === 'skipped' ? 'rgba(130,170,255,0.25)'
                  : 'rgba(255,99,99,0.25)'
                }`,
              }}
            >
              {result.outcome === 'executed'
                ? <CheckCircle size={13} style={{ color: 'var(--success)', flexShrink: 0, marginTop: 1 }} />
                : <AlertCircle size={13} style={{ color: 'var(--danger)',  flexShrink: 0, marginTop: 1 }} />
              }
              <div className="flex-1 min-w-0">
                <span
                  className="uppercase font-semibold"
                  style={{
                    color: result.outcome === 'executed' ? 'var(--success)'
                         : result.outcome === 'skipped'  ? 'var(--accent)'
                         : 'var(--danger)',
                    fontSize: '10px',
                  }}
                >
                  {result.outcome}
                </span>
                {result.output && (
                  <pre className="mt-1 whitespace-pre-wrap break-all" style={{ color: 'var(--text-2)', fontSize: '10px' }}>
                    {result.output}
                  </pre>
                )}
                {result.error && (
                  <pre className="mt-1 whitespace-pre-wrap break-all" style={{ color: 'var(--danger)', fontSize: '10px' }}>
                    {result.error}
                  </pre>
                )}
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div
          className="flex items-center justify-end gap-2 px-5 py-3"
          style={{ borderTop: '1px solid var(--border)' }}
        >
          <button
            onClick={onClose}
            className="px-3 py-1.5 rounded text-xs font-mono"
            style={{ background: 'var(--bg-panel)', color: 'var(--text-2)', border: '1px solid var(--border)' }}
          >
            Close
          </button>
          <button
            onClick={runTest}
            disabled={running}
            className="flex items-center gap-1.5 px-4 py-1.5 rounded text-xs font-mono font-semibold disabled:opacity-50"
            style={{ background: 'var(--accent)', color: '#011627' }}
          >
            <Play size={11} />
            {running ? 'Running…' : 'Run Test'}
          </button>
        </div>
      </div>
    </div>
  )
}

// ── HookRow ───────────────────────────────────────────────────────────────────

function HookRow({
  hook, expanded, onToggleExpand, onEnable, onDisable, onRemove, onOpenTest,
}: {
  hook:            HookConfig
  expanded:        boolean
  onToggleExpand:  () => void
  onEnable:        () => void
  onDisable:       () => void
  onRemove:        () => void
  onOpenTest:      () => void
}) {
  const store = useHookStore()
  const [customCondition,  setCustomCondition]  = useState(hook.conditions.custom_condition ?? '')
  const [condSaving,       setCondSaving]       = useState(false)

  async function handleSaveCondition() {
    setCondSaving(true)
    try {
      await store.addHook({
        ...hook,
        conditions: {
          ...hook.conditions,
          custom_condition: customCondition.trim() || undefined,
        },
      } as never)
    } catch (e) {
      console.error('[hook] save custom_condition failed:', e)
    } finally {
      setCondSaving(false)
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
            title="Test hook with mock context"
            onClick={onOpenTest}
            className="p-1.5 rounded hover:opacity-70"
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

      {/* Expanded detail */}
      {expanded && (
        <div
          className="px-4 pb-4 pt-0 text-xs font-mono space-y-2"
          style={{ borderTop: '1px solid var(--border)', color: 'var(--text-3)' }}
        >
          <div className="pt-3">
            <span style={{ color: 'var(--text-2)' }}>Action: </span>
            <pre className="inline whitespace-pre-wrap" style={{ color: 'var(--text-1)' }}>
              {JSON.stringify(hook.action, null, 2)}
            </pre>
          </div>

          {/* Structural conditions (read-only display) */}
          {(() => {
            const { custom_condition: _cc, ...structural } = hook.conditions
            return Object.keys(structural).length > 0 ? (
              <div>
                <span style={{ color: 'var(--text-2)' }}>Conditions: </span>
                <pre className="inline whitespace-pre-wrap" style={{ color: 'var(--text-1)' }}>
                  {JSON.stringify(structural, null, 2)}
                </pre>
              </div>
            ) : null
          })()}

          {/* Custom condition expression editor */}
          <div className="space-y-1 pt-1">
            <div className="flex items-center justify-between">
              <span style={{ color: 'var(--text-2)' }}>Custom condition <span style={{ color: 'var(--text-3)' }}>(Rhai expression)</span></span>
              {customCondition !== (hook.conditions.custom_condition ?? '') && (
                <button
                  onClick={handleSaveCondition}
                  disabled={condSaving}
                  className="text-xs px-2 py-0.5 rounded disabled:opacity-40"
                  style={{ background: 'var(--accent-muted)', color: 'var(--accent)', border: '1px solid var(--accent-border)' }}
                >
                  {condSaving ? 'Saving…' : 'Save'}
                </button>
              )}
            </div>
            <textarea
              value={customCondition}
              onChange={e => setCustomCondition(e.target.value)}
              rows={2}
              spellCheck={false}
              placeholder={`tool_name == "bash" && command.contains("rm -rf")`}
              className="w-full rounded px-2 py-1.5 resize-none"
              style={{
                background:  'var(--bg-surface)',
                border:      '1px solid var(--border-2)',
                color:       'var(--text-1)',
                fontFamily:  'monospace',
                fontSize:    '11px',
              }}
            />
            <div style={{ color: 'var(--text-3)', fontSize: '10px' }}>
              Variables: <code>tool_name</code>, <code>file_path</code>, <code>agent_mode</code>, <code>command</code>,{' '}
              <code>hook_type</code>, <code>session_id</code>, <code>worker_id</code>, <code>task_id</code>, <code>is_error</code>
            </div>
          </div>

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
  const [name,       setName]       = useState('')
  const [hookType,   setHookType]   = useState<HookType>('post_file_write')
  const [actionType, setActionType] = useState<'shell' | 'http' | 'notification' | 'log' | 'block' | 'emit_event'>('shell')
  const [command,    setCommand]    = useState('')
  const [url,        setUrl]        = useState('')
  const [method,     setMethod]     = useState('POST')
  const [notifTitle, setNotifTitle] = useState('')
  const [notifMsg,   setNotifMsg]   = useState('')
  const [logFile,    setLogFile]    = useState('')
  const [logMsg,     setLogMsg]     = useState('{{hook_type}} fired')
  const [blockMsg,   setBlockMsg]   = useState('')
  const [eventName,  setEventName]  = useState('')
  const [saving,     setSaving]     = useState(false)
  const [error,      setError]      = useState<string | null>(null)

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
      case 'shell':        return command    ? { type: 'shell', command } : null
      case 'http':         return url        ? { type: 'http', url, method } : null
      case 'notification': return notifTitle ? { type: 'notification', title: notifTitle, message: notifMsg } : null
      case 'log':          return logFile    ? { type: 'log', file: logFile, message: logMsg } : null
      case 'block':        return { type: 'block', message: blockMsg || undefined }
      case 'emit_event':   return eventName  ? { type: 'emit_event', event_name: eventName } : null
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
            <input value={notifTitle} onChange={e => setNotifTitle(e.target.value)} style={inputStyle} placeholder="Arcana Alert" />
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
            <input value={logFile} onChange={e => setLogFile(e.target.value)} style={inputStyle} placeholder="~/.arcana/logs/hooks.log" />
          </div>
          <div className="space-y-1">
            <label style={labelStyle}>Message template</label>
            <input title="Log message template" value={logMsg} onChange={e => setLogMsg(e.target.value)} style={inputStyle} />
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
          <input value={eventName} onChange={e => setEventName(e.target.value)} style={inputStyle} placeholder="arcana:hook-fired" />
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
