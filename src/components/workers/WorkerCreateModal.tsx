// WorkerCreateModal — form to create a new worker in the registry.
// Phase 2.2: Worker Management System.
// Called from WorkerTable "+ New Worker" button.

import { useState } from 'react'
import { useWorkerStore, CreateWorkerInput, WORKER_TYPE_LABELS } from '../../stores/workerStore'

// ─── Available Models ─────────────────────────────────────────────────────────

const MODEL_OPTIONS = [
  'claude-opus-4-5',
  'claude-sonnet-4-5',
  'claude-haiku-4-5',
  'claude-opus-4-0',
  'claude-sonnet-4-0',
  'gpt-4o',
  'gpt-4o-mini',
  'gemini-2.0-flash',
  'gemini-2.0-pro',
]

const WORKER_TYPES = Object.keys(WORKER_TYPE_LABELS).filter(t => t !== 'supervisor' && t !== 'qc')

// ─── WorkerCreateModal ────────────────────────────────────────────────────────

interface WorkerCreateModalProps {
  onClose:   () => void
  onCreated: (slug: string) => void
}

export default function WorkerCreateModal({ onClose, onCreated }: WorkerCreateModalProps) {
  const store = useWorkerStore()

  const [name,            setName]            = useState('')
  const [workerType,      setWorkerType]      = useState('backend')
  const [model,           setModel]           = useState('claude-sonnet-4-5')
  const [repoInput,       setRepoInput]       = useState('')
  const [repos,           setRepos]           = useState<string[]>([])
  const [pollInterval,    setPollInterval]    = useState(120)
  const [description,     setDescription]     = useState('')
  const [saving,          setSaving]          = useState(false)
  const [error,           setError]           = useState<string | null>(null)

  const addRepo = () => {
    const r = repoInput.trim()
    if (r && !repos.includes(r)) {
      setRepos(prev => [...prev, r])
      setRepoInput('')
    }
  }

  const removeRepo = (repo: string) =>
    setRepos(prev => prev.filter(r => r !== repo))

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!name.trim()) { setError('Name is required'); return }

    setSaving(true)
    setError(null)
    try {
      const input: CreateWorkerInput = {
        name:             name.trim(),
        worker_type:      workerType,
        model,
        primary_repos:    repos,
        polling_interval: pollInterval,
        description:      description.trim() || undefined,
      }
      const result = await store.createWorker(input)
      onCreated(result.slug)
      onClose()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setSaving(false)
    }
  }

  return (
    <div style={{
      position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.7)',
      display: 'flex', alignItems: 'center', justifyContent: 'center',
      zIndex: 1000,
    }}>
      <div style={{
        background: 'var(--bg-1)', border: '1px solid var(--border)',
        borderRadius: 8, padding: 24, width: 440, maxHeight: '80vh',
        overflowY: 'auto',
      }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 20 }}>
          <h3 style={{ fontFamily: 'monospace', fontSize: 13, color: 'var(--text-1)', margin: 0 }}>
            New Worker
          </h3>
          <button onClick={onClose} style={{ background: 'none', border: 'none', color: 'var(--text-3)', cursor: 'pointer', fontSize: 16 }}>
            ✕
          </button>
        </div>

        <form onSubmit={handleSubmit}>
          <Field label="Name">
            <input
              value={name}
              onChange={e => setName(e.target.value)}
              placeholder="e.g. Worker Backend 2"
              style={inputStyle}
            />
          </Field>

          <Field label="Type">
            <select value={workerType} onChange={e => setWorkerType(e.target.value)} title="Worker type" style={inputStyle}>
              {WORKER_TYPES.map(t => (
                <option key={t} value={t}>{WORKER_TYPE_LABELS[t]}</option>
              ))}
            </select>
          </Field>

          <Field label="Model">
            <select value={model} onChange={e => setModel(e.target.value)} title="Model" style={inputStyle}>
              {MODEL_OPTIONS.map(m => (
                <option key={m} value={m}>{m}</option>
              ))}
            </select>
          </Field>

          <Field label="Polling Interval (s)">
            <input
              type="number"
              min={30}
              max={3600}
              value={pollInterval}
              onChange={e => setPollInterval(Number(e.target.value))}
              title="Polling interval in seconds"
              style={inputStyle}
            />
          </Field>

          <Field label="Primary Repos">
            <div style={{ display: 'flex', gap: 6, marginBottom: 6 }}>
              <input
                value={repoInput}
                onChange={e => setRepoInput(e.target.value)}
                onKeyDown={e => e.key === 'Enter' && (e.preventDefault(), addRepo())}
                placeholder="owner/repo"
                title="Repository in owner/repo format"
                style={{ ...inputStyle, flex: 1 }}
              />
              <button type="button" onClick={addRepo} style={addBtnStyle}>
                Add
              </button>
            </div>
            {repos.length > 0 && (
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
                {repos.map(r => (
                  <span key={r} style={{
                    fontFamily: 'monospace', fontSize: 10,
                    padding: '2px 8px', borderRadius: 3,
                    background: 'rgba(255,255,255,0.06)',
                    color: 'var(--text-2)',
                    display: 'flex', alignItems: 'center', gap: 4,
                  }}>
                    {r}
                    <button
                      type="button"
                      onClick={() => removeRepo(r)}
                      style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-3)', padding: 0, fontSize: 10 }}
                    >
                      ✕
                    </button>
                  </span>
                ))}
              </div>
            )}
          </Field>

          <Field label="Description (optional)">
            <textarea
              value={description}
              onChange={e => setDescription(e.target.value)}
              rows={2}
              placeholder="What does this worker do?"
              style={{ ...inputStyle, resize: 'vertical', minHeight: 56 }}
            />
          </Field>

          {error && (
            <div style={{ fontFamily: 'monospace', fontSize: 11, color: '#FF6363', marginBottom: 12 }}>
              {error}
            </div>
          )}

          <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
            <button type="button" onClick={onClose} style={cancelBtnStyle}>
              Cancel
            </button>
            <button type="submit" disabled={saving} style={submitBtnStyle}>
              {saving ? 'Creating…' : 'Create Worker'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

// ─── Field wrapper ────────────────────────────────────────────────────────────

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: 14 }}>
      <label style={{
        display: 'block', fontFamily: 'monospace', fontSize: 10,
        color: 'var(--text-3)', textTransform: 'uppercase',
        letterSpacing: 1, marginBottom: 4,
      }}>
        {label}
      </label>
      {children}
    </div>
  )
}

// ─── Style helpers ─────────────────────────────────────────────────────────────

const inputStyle: React.CSSProperties = {
  width: '100%', padding: '6px 8px', fontFamily: 'monospace', fontSize: 12,
  background: 'var(--bg-2)', border: '1px solid var(--border)',
  borderRadius: 4, color: 'var(--text-1)', outline: 'none', boxSizing: 'border-box',
}

const addBtnStyle: React.CSSProperties = {
  padding: '6px 12px', fontFamily: 'monospace', fontSize: 11,
  border: '1px solid var(--border)', borderRadius: 4,
  background: 'transparent', color: 'var(--text-2)', cursor: 'pointer',
}

const cancelBtnStyle: React.CSSProperties = {
  padding: '6px 14px', fontFamily: 'monospace', fontSize: 11,
  border: '1px solid var(--border)', borderRadius: 4,
  background: 'transparent', color: 'var(--text-2)', cursor: 'pointer',
}

const submitBtnStyle: React.CSSProperties = {
  padding: '6px 14px', fontFamily: 'monospace', fontSize: 11,
  border: '1px solid var(--accent)', borderRadius: 4,
  background: 'transparent', color: 'var(--accent)', cursor: 'pointer',
}
