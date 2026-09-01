// WorkerTable — displays all workers from the registry with status indicators,
// model info, and start/stop/delete controls.
// Phase 2.2: Part of the Workers section in Settings tab.
// Phase 2.3: Recipe binding column — assign recipes to workers via bind_worker_recipe.

import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import {
  useWorkerStore,
  Worker,
  WorkerStatus,
  workerStatusColor,
  workerStatusLabel,
  WORKER_TYPE_LABELS,
} from '../../stores/workerStore'

// Toast notification state
type ToastType = 'success' | 'warning' | 'error'
interface Toast {
  id: number
  message: string
  type: ToastType
}

// ─── Available models ────────────────────────────────────────────────────────
// Registry keys from core-orchestrator/src/llm/providers/model_registry.py
// Grouped by provider for the <optgroup> selector.

const MODEL_GROUPS: Array<{ label: string; models: string[] }> = [
  {
    label: 'Anthropic Claude',
    models: [
      'claude-4-5-haiku',
      'claude-4-5-haiku-thinking',
      'claude-sonnet-4-5',
      'claude-sonnet-4-5-thinking',
      'claude-4-1-opus',
      'claude-4-1-opus-thinking',
      'claude-4-opus',
      'claude-4-opus-thinking',
      'claude-4-sonnet',
      'claude-4-sonnet-thinking',
    ],
  },
  {
    label: 'OpenAI GPT',
    models: [
      'gpt-5',
      'gpt-5-mini',
      'gpt-4.1',
      'gpt-4.1-mini',
      'gpt-4.1-nano',
      'gpt-4o',
      'gpt-4o-mini',
      'o3',
      'o4-mini',
    ],
  },
  {
    label: 'Google Gemini',
    models: [
      'gemini-3-pro',
      'gemini-3-pro-thinking',
      'gemini-3-flash',
      'gemini-3-flash-thinking',
      'gemini-2.5-pro',
      'gemini-2.5-flash',
    ],
  },
  {
    label: 'DeepSeek',
    models: [
      'deepseek-v4-flash',
      'deepseek-v4-pro',
    ],
  },
  {
    label: 'Moonshot / Kimi',
    models: [
      'kimi-k2',
      'kimi-k2.5',
      'kimi-k2-thinking',
    ],
  },
  {
    label: 'Groq (Llama)',
    models: [
      'llama-3.3-70b',
      'llama-3.1-8b',
    ],
  },
  {
    label: 'SEA-LION (AI Singapore)',
    models: [
      'sea-lion-llama-3.5-70b-Reasoning',
      'sea-lion-llama-3-70b',
      'sea-lion-gemma-4-27b',
    ],
  },
  {
    label: 'Dashscope / Qwen',
    models: [
      'qwen3-max',
    ],
  },
  {
    label: 'NVIDIA NIM',
    models: [
      'kimi-k2.5-nvidia',
      'qwen-3.5-nvidia',
      'deepseek-v3.2-nvidia',
    ],
  },
]

// Flat set for membership checks
const ALL_KNOWN_MODELS = new Set(MODEL_GROUPS.flatMap(g => g.models))

// ─── Status Dot ───────────────────────────────────────────────────────────────

function StatusDot({ status }: { status: WorkerStatus }) {
  const color  = workerStatusColor(status)
  const active = status === 'active' || status === 'working'
  return (
    <span style={{
      display:       'inline-block',
      width:         8,
      height:        8,
      borderRadius:  '50%',
      backgroundColor: color,
      boxShadow:     active ? `0 0 6px ${color}` : 'none',
      flexShrink:    0,
    }} />
  )
}

// ─── Worker Row ───────────────────────────────────────────────────────────────

function WorkerRow({
  worker,
  recipes,
  onStart,
  onStop,
  onDelete,
  onModelChange,
  onModelSyncResult,
}: {
  worker:        Worker
  recipes:       Array<{ id: string; title: string }>
  onStart:       (id: number) => void
  onStop:        (id: number) => void
  onDelete:      (id: number) => void
  onModelChange: (id: number, model: string) => void
  onModelSyncResult: (message: string, type: ToastType) => void
}) {
  const [binding,  setBinding]  = useState(false)
  const [savingModel, setSavingModel] = useState(false)

  const handleModelChange = async (model: string) => {
    setSavingModel(true)
    try {
      // Use sync version that immediately pushes to core-orchestrator
      const result = await invoke<{
        db_updated: boolean
        sync_status: string
        message: string
        model: string
      }>('update_worker_model_sync', { workerId: worker.id, model })
      
      onModelChange(worker.id, model)
      
      // Show toast based on sync status
      const toastType: ToastType = 
        result.sync_status === 'synced' ? 'success' :
        result.sync_status === 'pending' ? 'warning' : 'error'
      onModelSyncResult(result.message, toastType)
    } catch (e) {
      console.error('update_worker_model_sync failed:', e)
      onModelSyncResult(`Failed to update model: ${e}`, 'error')
    } finally {
      setSavingModel(false)
    }
  }

  const handleBindRecipe = async (recipeId: string) => {
    setBinding(true)
    try {
      if (recipeId === '') {
        await invoke('bind_worker_recipe', { workerId: worker.id, recipeId: null })
      } else {
        await invoke('bind_worker_recipe', { workerId: worker.id, recipeId })
      }
    } catch (e) {
      console.error('bind_worker_recipe failed:', e)
    } finally {
      setBinding(false)
    }
  }
  const isRunning  = worker.status === 'active' || worker.status === 'working'
  const canDelete  = !worker.is_system_defined
  const typeLabel  = WORKER_TYPE_LABELS[worker.worker_type] ?? worker.worker_type

  return (
    <tr style={{ borderBottom: '1px solid var(--border)' }}>
      <td style={{ padding: '8px 10px', whiteSpace: 'nowrap' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <StatusDot status={worker.status} />
          <span style={{ fontFamily: 'monospace', fontSize: 12, color: 'var(--text-1)' }}>
            {worker.name}
          </span>
          {worker.is_system_defined && (
            <span style={{
              fontSize: 9, padding: '1px 5px', borderRadius: 3,
              background: 'rgba(255,255,255,0.06)', color: 'var(--text-3)',
              textTransform: 'uppercase', letterSpacing: 1,
            }}>sys</span>
          )}
        </div>
      </td>
      <td style={{ padding: '8px 10px', fontSize: 11, color: 'var(--text-3)', fontFamily: 'monospace' }}>
        {worker.slug}
      </td>
      <td style={{ padding: '8px 10px', fontSize: 11, color: 'var(--text-3)' }}>
        {typeLabel}
      </td>
      <td style={{ padding: '4px 10px', minWidth: 160 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
          <select
            title="Worker model"
            value={worker.model}
            onChange={e => void handleModelChange(e.target.value)}
            disabled={savingModel}
            style={{
              fontFamily: 'monospace', fontSize: 10,
              background: 'var(--bg-2)', border: '1px solid var(--border)',
              color: savingModel ? 'var(--text-3)' : 'var(--accent)',
              borderRadius: 3, padding: '2px 4px',
              cursor: savingModel ? 'wait' : 'pointer', width: '100%',
            }}
          >
            {/* If the worker's current model isn't in our known list, show it
                as the first option so the dropdown displays the real value
                rather than jumping to the first known entry. */}
            {!ALL_KNOWN_MODELS.has(worker.model) && (
              <option value={worker.model}>{worker.model} (current)</option>
            )}
            {MODEL_GROUPS.map(group => (
              <optgroup key={group.label} label={group.label}>
                {group.models.map(m => (
                  <option key={m} value={m}>{m}</option>
                ))}
              </optgroup>
            ))}
          </select>
          {savingModel && (
            <span style={{
              fontSize: 10, color: 'var(--text-3)',
              animation: 'spin 1s linear infinite',
              display: 'inline-block', flexShrink: 0,
            }}>⟳</span>
          )}
        </div>
      </td>
      <td style={{ padding: '8px 10px', fontSize: 11 }}>
        <span style={{ color: workerStatusColor(worker.status) }}>
          {workerStatusLabel(worker.status)}
        </span>
      </td>
      {/* Recipe binding column */}
      <td style={{ padding: '4px 10px', minWidth: 140 }}>
        <select
          title="Bound recipe"
          value={worker.recipe_id ?? ''}
          onChange={e => handleBindRecipe(e.target.value)}
          disabled={binding}
          style={{
            fontFamily: 'monospace', fontSize: 10,
            background: 'var(--bg-2)', border: '1px solid var(--border)',
            color: worker.recipe_id ? 'var(--accent)' : 'var(--text-3)',
            borderRadius: 3, padding: '2px 4px', cursor: 'pointer', width: '100%',
          }}
        >
          <option value="">— no recipe —</option>
          {recipes.map(r => (
            <option key={r.id} value={r.id}>{r.title}</option>
          ))}
        </select>
      </td>
      <td style={{ padding: '8px 10px' }}>
        <div style={{ display: 'flex', gap: 6 }}>
          {isRunning ? (
            <button
              onClick={() => onStop(worker.id)}
              style={btnStyle('var(--text-3)')}
            >
              Stop
            </button>
          ) : (
            <button
              onClick={() => onStart(worker.id)}
              style={btnStyle('#00AA55')}
              disabled={worker.status === 'error'}
            >
              Start
            </button>
          )}
          {canDelete && (
            <button
              onClick={() => onDelete(worker.id)}
              style={btnStyle('#AA3333')}
            >
              Del
            </button>
          )}
        </div>
      </td>
    </tr>
  )
}

function btnStyle(color: string): React.CSSProperties {
  return {
    padding:      '2px 8px',
    fontSize:     11,
    fontFamily:   'monospace',
    border:       `1px solid ${color}`,
    borderRadius: 3,
    background:   'transparent',
    color,
    cursor:       'pointer',
  }
}

// ─── WorkerTable ─────────────────────────────────────────────────────────────

interface WorkerTableProps {
  onAddWorker: () => void
}

export default function WorkerTable({ onAddWorker }: WorkerTableProps) {
  const store      = useWorkerStore()
  const [confirm, setConfirm] = useState<number | null>(null)
  const [recipes, setRecipes] = useState<Array<{ id: string; title: string }>>([])
  const [toasts, setToasts] = useState<Toast[]>([])
  const [toastId, setToastId] = useState(0)

  useEffect(() => {
    invoke<Array<{ id: string; title: string; is_system: boolean }>>('list_recipes')
      .then(list => setRecipes(list.map(r => ({ id: r.id, title: r.title }))))
      .catch(() => setRecipes([]))
  }, [])

  const handleDelete = (id: number) => {
    if (confirm === id) {
      store.deleteWorker(id).catch(console.error)
      setConfirm(null)
    } else {
      setConfirm(id)
      setTimeout(() => setConfirm(null), 3000)
    }
  }

  const handleModelChange = (_id: number, _model: string) => {
    // Reload the worker list so the store reflects the persisted model change
    store.loadWorkers().catch(console.error)
  }

  const showToast = (message: string, type: ToastType) => {
    const id = toastId + 1
    setToastId(id)
    const newToast: Toast = { id, message, type }
    setToasts(prev => [...prev, newToast])
    // Auto-dismiss after 4 seconds
    setTimeout(() => {
      setToasts(prev => prev.filter(t => t.id !== id))
    }, 4000)
  }

  const handleStart = (id: number) =>
    store.startWorker(id).catch(console.error)

  const handleStop = (id: number) =>
    store.stopWorker(id).catch(console.error)

  if (store.isLoading) {
    return (
      <div style={{ fontFamily: 'monospace', fontSize: 12, color: 'var(--text-3)', padding: 16 }}>
        Loading workers…
      </div>
    )
  }

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 10 }}>
        <span style={{ fontFamily: 'monospace', fontSize: 11, color: 'var(--text-3)' }}>
          {store.workers.length} agents registered
        </span>
        <button
          onClick={onAddWorker}
          style={{
            fontFamily: 'monospace', fontSize: 11,
            padding: '3px 10px', borderRadius: 4,
            border: '1px solid var(--accent)', background: 'transparent',
            color: 'var(--accent)', cursor: 'pointer',
          }}
        >
          + New Worker
        </button>
      </div>

      <div style={{ overflowX: 'auto' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
          <thead>
            <tr style={{ borderBottom: '1px solid var(--border)' }}>
              {['Name', 'Slug', 'Type', 'Model', 'Status', 'Recipe', 'Actions'].map(h => (
                <th key={h} style={{
                  textAlign: 'left', padding: '6px 10px',
                  fontFamily: 'monospace', fontSize: 10,
                  color: 'var(--text-3)', textTransform: 'uppercase',
                  letterSpacing: 1, fontWeight: 'normal',
                }}>
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {store.workers.map(w => (
              <WorkerRow
                key={w.id}
                worker={{ ...w, status: confirm === w.id ? 'error' : w.status }}
                recipes={recipes}
                onStart={handleStart}
                onStop={handleStop}
                onDelete={handleDelete}
                onModelChange={handleModelChange}
                onModelSyncResult={showToast}
              />
            ))}
          </tbody>
        </table>
      </div>

      {/* Toast notifications */}
      <div style={{
        position: 'fixed',
        bottom: 20,
        right: 20,
        display: 'flex',
        flexDirection: 'column',
        gap: 8,
        zIndex: 1000,
      }}>
        {toasts.map(toast => (
          <div
            key={toast.id}
            style={{
              padding: '10px 14px',
              borderRadius: 4,
              fontFamily: 'monospace',
              fontSize: 11,
              maxWidth: 320,
              boxShadow: '0 4px 12px rgba(0,0,0,0.3)',
              animation: 'fadeInUp 0.3s ease-out',
              border: '1px solid',
              borderColor: toast.type === 'success' ? 'var(--success)' : 
                          toast.type === 'warning' ? '#FFCB8B' : '#FF6363',
              backgroundColor: toast.type === 'success' ? 'rgba(173,219,103,0.1)' :
                              toast.type === 'warning' ? 'rgba(255,203,139,0.1)' :
                              'rgba(255,99,99,0.1)',
              color: toast.type === 'success' ? 'var(--success)' :
                     toast.type === 'warning' ? '#FFCB8B' : '#FF6363',
            }}
          >
            {toast.type === 'success' && '✓ '}
            {toast.type === 'warning' && '⚠ '}
            {toast.type === 'error' && '✗ '}
            {toast.message}
          </div>
        ))}
      </div>

      {confirm !== null && (
        <div style={{
          marginTop: 8, fontFamily: 'monospace', fontSize: 11,
          color: '#FF6363', padding: '6px 10px', border: '1px solid #FF636350',
          borderRadius: 4, background: 'rgba(255,99,99,0.06)',
        }}>
          Click Delete again within 3s to confirm deletion
        </div>
      )}

      {store.error && (
        <div style={{ marginTop: 8, fontFamily: 'monospace', fontSize: 11, color: '#FF6363' }}>
          Error: {store.error}
        </div>
      )}
    </div>
  )
}
