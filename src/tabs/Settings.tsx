// Settings tab — provider key management, model selection, connection test.
// References: DEVELOPMENT_PLAN.md Section 9 Phase 1 Deliverables.

import { useEffect, useState } from 'react'
import { CheckCircle, XCircle, RefreshCw, Eye, EyeOff, RotateCcw, Save } from 'lucide-react'
import { useSettingsStore } from '../stores/settingsStore'

export default function SettingsTab() {
  const store = useSettingsStore()

  return (
    <div className="flex-1 overflow-y-auto p-5">
      <div className="max-w-2xl mx-auto space-y-5">

        <SectionHeader title="Providers" />

        {store.isLoadingProviders ? (
          <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
            Loading…
          </div>
        ) : (
          store.providers.map(provider => (
            <ProviderCard key={provider.id} providerId={provider.id} />
          ))
        )}

        <div className="pt-4">
          <SectionHeader title="System Prompt" />
          <SystemPromptEditor />
        </div>

      </div>
    </div>
  )
}

function SectionHeader({ title }: { title: string }) {
  return (
    <div
      className="font-mono text-xs mb-2"
      style={{
        color:         'var(--text-3)',
        letterSpacing: '0.1em',
        textTransform: 'uppercase',
        borderBottom:  '1px solid var(--border-1)',
        paddingBottom: '8px',
      }}
    >
      {title}
    </div>
  )
}

function SystemPromptEditor() {
  const store = useSettingsStore()
  const [draft, setDraft] = useState(store.systemPrompt ?? store.defaultSystemPrompt)
  const [saving, setSaving] = useState(false)
  const hasChanges = draft !== (store.systemPrompt ?? store.defaultSystemPrompt)
  const isUsingDefault = store.systemPrompt === null

  useEffect(() => {
    store.loadSystemPrompt()
  }, [])

  useEffect(() => {
    setDraft(store.systemPrompt ?? store.defaultSystemPrompt)
  }, [store.systemPrompt])

  const handleSave = async () => {
    setSaving(true)
    try {
      const trimmed = draft.trim()
      if (trimmed === store.defaultSystemPrompt.trim() || trimmed === '') {
        await store.resetSystemPrompt()
      } else {
        await store.setSystemPrompt(trimmed)
      }
    } finally {
      setSaving(false)
    }
  }

  const handleReset = () => {
    setDraft(store.defaultSystemPrompt)
  }

  const handleClear = async () => {
    await store.resetSystemPrompt()
    setDraft(store.defaultSystemPrompt)
  }

  return (
    <div
      className="rounded-sm p-4 space-y-3"
      style={{
        background: 'var(--bg-card)',
        border:     '1px solid var(--border-1)',
      }}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="font-mono text-xs" style={{ color: 'var(--text-2)' }}>
            {isUsingDefault ? 'Using default prompt' : 'Custom prompt active'}
          </span>
          {!isUsingDefault && (
            <span
              className="font-mono px-1.5 py-0.5"
              style={{
                fontSize:   '10px',
                background: 'var(--accent-muted)',
                border:     '1px solid var(--accent-border)',
                color:      'var(--accent)',
                borderRadius: '2px',
              }}
            >
              MODIFIED
            </span>
          )}
        </div>
        <div className="flex gap-2">
          {!isUsingDefault && (
            <button
              onClick={handleClear}
              className="font-mono text-xs px-2 py-1 rounded-sm cursor-pointer"
              style={{
                background: 'transparent',
                border:     '1px solid var(--border-2)',
                color:      'var(--text-3)',
              }}
            >
              Reset to Default
            </button>
          )}
          <button
            onClick={handleReset}
            disabled={!hasChanges}
            title="Revert changes"
            className="font-mono text-xs px-2 py-1 rounded-sm cursor-pointer disabled:opacity-40"
            style={{
              background: 'var(--bg-surface)',
              border:     '1px solid var(--border-2)',
              color:      'var(--text-2)',
            }}
          >
            <RotateCcw size={11} />
          </button>
          <button
            onClick={handleSave}
            disabled={!hasChanges || saving}
            className="font-mono text-xs px-3 py-1 rounded-sm cursor-pointer disabled:opacity-40 flex items-center gap-1.5"
            style={{
              background: 'var(--accent-muted)',
              border:     '1px solid var(--accent-border)',
              color:      'var(--accent)',
            }}
          >
            {saving ? 'Saving…' : <><Save size={11} /> Save</>}
          </button>
        </div>
      </div>

      <textarea
        value={draft}
        onChange={e => setDraft(e.target.value)}
        rows={12}
        className="w-full font-mono text-xs p-3 rounded-sm resize-y"
        style={{
          background: 'var(--bg-surface)',
          border:     '1px solid var(--border-2)',
          color:      'var(--text-1)',
          lineHeight: '1.6',
        }}
        placeholder="Enter custom system prompt..."
      />

      <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
        The system prompt defines how the AI behaves. Changes take effect on the next message.
        CLAUDE.md files in your home or project directory will still be appended.
      </div>
    </div>
  )
}

function ProviderCard({ providerId }: { providerId: string }) {
  const store        = useSettingsStore()
  const provider     = store.providers.find(p => p.id === providerId)
  const [key,        setKey]        = useState('')
  const [showKey,    setShowKey]    = useState(false)
  const [testing,    setTesting]    = useState(false)
  const [testResult, setTestResult] = useState<string | null>(null)
  const [saving,     setSaving]     = useState(false)

  if (!provider) return null

  const isActive      = store.activeProvider === providerId
  const displayName   = provider.name
  const hasKey        = provider.has_key
  const defaultModel  = provider.models[1]?.id ?? provider.models[0]?.id ?? ''

  const handleSaveKey = async () => {
    if (!key.trim()) return
    setSaving(true)
    try {
      await store.storeProviderKey(providerId, key.trim())
      if (!isActive && defaultModel) {
        await store.setActiveProvider(providerId, defaultModel)
      }
      setKey('')
      setTestResult(null)
    } catch (e) {
      console.error('Failed to save key:', e)
    } finally {
      setSaving(false)
    }
  }

  const handleDeleteKey = async () => {
    if (!confirm(`Remove ${displayName} API key?`)) return
    await store.deleteProviderKey(providerId)
    setTestResult(null)
  }

  const handleTest = async () => {
    setTesting(true)
    setTestResult(null)
    try {
      const result = await store.testProvider(providerId)
      setTestResult(result)
    } catch (e: unknown) {
      setTestResult(`Error: ${e instanceof Error ? e.message : String(e)}`)
    } finally {
      setTesting(false)
    }
  }

  const handleSetActive = async (model: string) => {
    await store.setActiveProvider(providerId, model)
  }

  return (
    <div
      className="rounded-sm p-4 space-y-3"
      style={{
        background: 'var(--bg-card)',
        border:     `1px solid ${isActive ? 'var(--accent-border)' : 'var(--border-1)'}`,
      }}
    >
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span
            className="w-1.5 h-1.5 rounded-full shrink-0"
            style={{ background: hasKey ? 'var(--accent)' : '#2a2a2a' }}
          />
          <span className="font-mono text-xs" style={{ color: 'var(--text-1)' }}>
            {displayName}
          </span>
          {isActive && (
            <span
              className="font-mono px-1.5 py-0.5"
              style={{
                fontSize:   '10px',
                background: 'var(--accent-muted)',
                border:     '1px solid var(--accent-border)',
                color:      'var(--accent)',
                borderRadius: '2px',
              }}
            >
              ACTIVE
            </span>
          )}
        </div>

        {hasKey && (
          <div className="flex gap-2">
            <button
              onClick={handleTest}
              disabled={testing}
              className="font-mono text-xs px-2 py-1 rounded-sm cursor-pointer disabled:opacity-40"
              style={{
                background: 'var(--bg-surface)',
                border:     '1px solid var(--border-2)',
                color:      'var(--text-2)',
              }}
            >
              {testing ? <RefreshCw size={11} className="animate-spin" /> : 'Test'}
            </button>
            <button
              onClick={handleDeleteKey}
              className="font-mono text-xs px-2 py-1 rounded-sm cursor-pointer"
              style={{
                background: 'transparent',
                border:     '1px solid #FF444430',
                color:      'var(--danger)',
              }}
            >
              Remove
            </button>
          </div>
        )}
      </div>

      {/* Test result */}
      {testResult && (
        <div
          className="flex items-center gap-1.5 font-mono text-xs"
          style={{ color: testResult.toLowerCase().includes('connected') ? 'var(--accent)' : 'var(--danger)' }}
        >
          {testResult.toLowerCase().includes('connected')
            ? <CheckCircle size={12} />
            : <XCircle    size={12} />
          }
          {testResult}
        </div>
      )}

      {/* API Key input */}
      {!hasKey && (
        <div className="flex gap-2 items-center">
          <div className="flex-1 flex items-center relative">
            <input
              type={showKey ? 'text' : 'password'}
              placeholder={`${displayName} API Key`}
              value={key}
              onChange={e => setKey(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && handleSaveKey()}
              className="flex-1 font-mono text-xs px-3 py-1.5 rounded-sm"
              style={{
                background: 'var(--bg-surface)',
                border:     '1px solid var(--border-2)',
                color:      'var(--text-1)',
                paddingRight: '32px',
              }}
            />
            <button
              onClick={() => setShowKey(!showKey)}
              className="absolute right-2 border-none cursor-pointer"
              style={{ background: 'transparent', color: 'var(--text-3)' }}
            >
              {showKey ? <EyeOff size={12} /> : <Eye size={12} />}
            </button>
          </div>
          <button
            onClick={handleSaveKey}
            disabled={saving || !key.trim()}
            className="font-mono text-xs px-3 py-1.5 rounded-sm cursor-pointer disabled:opacity-40"
            style={{
              background: 'var(--accent-muted)',
              border:     '1px solid var(--accent-border)',
              color:      'var(--accent)',
            }}
          >
            {saving ? 'Saving…' : 'Save'}
          </button>
        </div>
      )}

      {/* Model selector */}
      {hasKey && provider.models.length > 0 && (
        <div>
          <div
            className="font-mono text-xs mb-1.5"
            style={{ color: 'var(--text-3)', fontSize: '10px' }}
          >
            MODEL
          </div>
          <div className="flex flex-wrap gap-1.5">
            {provider.models.map(model => {
              const isSelectedModel = isActive && store.activeModel === model.id
              return (
                <button
                  key={model.id}
                  onClick={() => handleSetActive(model.id)}
                  className="font-mono text-xs px-2 py-1 rounded-sm cursor-pointer"
                  style={{
                    background: isSelectedModel ? 'var(--accent-muted)' : 'var(--bg-surface)',
                    border:     `1px solid ${isSelectedModel ? 'var(--accent-border)' : 'var(--border-2)'}`,
                    color:      isSelectedModel ? 'var(--accent)' : 'var(--text-2)',
                  }}
                >
                  {model.name}
                </button>
              )
            })}
          </div>
        </div>
      )}
    </div>
  )
}
