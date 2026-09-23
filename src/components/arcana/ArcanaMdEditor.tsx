// ARCANA.md Editor — full-screen overlay with scope tabs (Global / This Repo).
// Uses a styled textarea (Monaco not yet installed).
// Integrated into the sidebar via ArcanaMdCard trigger.
// See: DEVELOPMENT_PLAN.md Phase 2.1.2

import { useEffect, useState, useCallback } from 'react'
import { X, Save, RotateCcw, AlertTriangle } from 'lucide-react'
import {
  useArcanaMdStore,
  type ArcanaScope,
  MAX_ARCANA_CHARS,
} from '../../stores/arcanaMdStore'
import { useAgentStore } from '../../stores/agentStore'

interface Props {
  onClose:     () => void
  initialScope?: ArcanaScope
}

const ARCANA_TEMPLATE = `# Project Name

## Stack
- Language/framework versions
- Package managers
- Key dependencies

## Commands
- \`test\`: How to run tests
- \`lint\`: How to lint
- \`dev\`: How to start dev server

## Architecture
- Directory structure explanation
- Key abstractions

## Patterns
- Coding conventions
- Testing patterns
- Error handling approach
`

export default function ArcanaMdEditor({ onClose, initialScope = 'global' }: Props) {
  const store      = useArcanaMdStore()
  const workingDir = useAgentStore(s => s.workingDir)

  const [scope,   setScope]   = useState<ArcanaScope>(initialScope)
  const [draft,   setDraft]   = useState('')
  const [saved,   setSaved]   = useState(false)

  const repoPath   = workingDir || null
  const scopeKey   = scope === 'global' ? 'global' : (repoPath ?? '')
  const isModified = store.modifiedScopes.has(scopeKey)
  const charCount  = draft.length
  const isOver     = charCount > MAX_ARCANA_CHARS

  // Load content when scope or repoPath changes
  useEffect(() => {
    if (scope === 'global') {
      store.loadGlobal().then(() => {
        setDraft(store.globalContent)
      })
    } else if (repoPath) {
      store.loadRepo(repoPath).then(() => {
        setDraft(store.repoContents[repoPath] ?? '')
      })
    }
  }, [scope, repoPath])

  // Sync draft from store after load
  useEffect(() => {
    if (scope === 'global') {
      setDraft(store.globalContent)
    } else if (repoPath) {
      setDraft(store.repoContents[repoPath] ?? '')
    }
  }, [store.globalContent, store.repoContents, scope, repoPath])

  const handleChange = useCallback((value: string) => {
    setDraft(value)
    store.setModified(scopeKey, value !== (scope === 'global' ? store.globalContent : (store.repoContents[repoPath ?? ''] ?? '')))
    setSaved(false)
  }, [scopeKey, scope, repoPath, store])

  const handleSave = async () => {
    if (isOver) return
    if (scope === 'global') {
      await store.saveGlobal(draft)
    } else if (repoPath) {
      await store.saveRepo(repoPath, draft)
    }
    setSaved(true)
    setTimeout(() => setSaved(false), 2000)
  }

  const handleReset = () => {
    const original = scope === 'global'
      ? store.globalContent
      : (repoPath ? store.repoContents[repoPath] ?? '' : '')
    setDraft(original)
    store.setModified(scopeKey, false)
  }

  const handleInsertTemplate = () => {
    if (draft.trim() === '') setDraft(ARCANA_TEMPLATE)
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 's') {
      e.preventDefault()
      handleSave()
    }
    if (e.key === 'Escape') onClose()
  }

  const progressPct = Math.min((charCount / MAX_ARCANA_CHARS) * 100, 100)
  const progressColor = isOver
    ? 'var(--error)'
    : charCount > MAX_ARCANA_CHARS * 0.85
      ? 'var(--warning)'
      : 'var(--accent)'

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center"
      style={{ background: 'rgba(0,0,0,0.7)', backdropFilter: 'blur(2px)' }}
      onClick={e => { if (e.target === e.currentTarget) onClose() }}
    >
      <div
        className="flex flex-col w-full max-w-3xl mt-12 mb-12 rounded-lg overflow-hidden font-mono"
        style={{
          background:  'var(--bg-surface)',
          border:      '1px solid var(--border-2)',
          maxHeight:   'calc(100vh - 6rem)',
          boxShadow:   '0 24px 64px rgba(0,0,0,0.5)',
        }}
        onKeyDown={handleKeyDown}
      >
        {/* Header */}
        <div
          className="flex items-center gap-3 px-4 py-3 shrink-0"
          style={{ borderBottom: '1px solid var(--border-1)', background: 'var(--bg-card)' }}
        >
          <span className="text-xs font-bold" style={{ color: 'var(--text-1)' }}>
            ARCANA.md
          </span>
          <span className="text-xs" style={{ color: 'var(--text-4)' }}>
            Instructions for PA (you)
          </span>
          <div className="flex-1" />

          {/* Scope tabs */}
          <div
            className="flex items-center gap-0.5 rounded"
            style={{ background: 'var(--bg-surface)', border: '1px solid var(--border-2)', padding: '2px' }}
          >
            {(['global', 'repo'] as ArcanaScope[]).map(s => (
              <button
                key={s}
                onClick={() => setScope(s)}
                className="rounded transition-colors"
                style={{
                  fontSize:   '10px',
                  padding:    '2px 8px',
                  background: scope === s ? 'var(--bg-card)' : 'transparent',
                  color:      scope === s ? 'var(--text-1)' : 'var(--text-4)',
                  border:     scope === s ? '1px solid var(--border-2)' : '1px solid transparent',
                  cursor:     'pointer',
                }}
              >
                {s === 'global' ? 'Global' : 'This Repo'}
              </button>
            ))}
          </div>

          <button
            onClick={onClose}
            aria-label="Close ARCANA.md editor"
            style={{ color: 'var(--text-4)', background: 'none', border: 'none', cursor: 'pointer', padding: 4 }}
          >
            <X size={14} />
          </button>
        </div>

        {/* Scope description */}
        <div
          className="px-4 py-2 text-xs shrink-0"
          style={{ borderBottom: '1px solid var(--border-1)', color: 'var(--text-4)', background: 'var(--bg-card)' }}
        >
          {scope === 'global'
            ? 'Applied to all sessions — your general coding preferences and conventions'
            : repoPath
              ? `Per-repo instructions for: ${repoPath}`
              : '⚠️ No working directory set — switch to a session first'}
        </div>

        {/* Error */}
        {store.error && (
          <div
            className="flex items-center gap-2 px-4 py-2 text-xs shrink-0"
            style={{ background: 'rgba(239,68,68,0.1)', color: 'var(--error)' }}
          >
            <AlertTriangle size={12} />
            {store.error}
          </div>
        )}

        {/* Textarea */}
        <div className="flex-1 overflow-hidden relative">
          <textarea
            className="w-full h-full resize-none outline-none text-xs leading-relaxed"
            style={{
              background:  'var(--bg-base)',
              color:       'var(--text-1)',
              fontFamily:  'var(--font-mono)',
              padding:     '16px',
              border:      'none',
            }}
            value={draft}
            onChange={e => handleChange(e.target.value)}
            placeholder={scope === 'repo' && !repoPath
              ? 'Set a working directory first to edit repo-specific instructions'
              : 'Write your ARCANA.md instructions here...\n\nOr click "Insert Template" below to start from the standard structure.'}
            disabled={scope === 'repo' && !repoPath}
            spellCheck={false}
          />
        </div>

        {/* Footer */}
        <div
          className="flex items-center gap-2 px-4 py-2 shrink-0"
          style={{ borderTop: '1px solid var(--border-1)', background: 'var(--bg-card)' }}
        >
          {/* Character counter */}
          <div className="flex items-center gap-2 flex-1">
            <div
              className="h-1 rounded-full flex-1 max-w-24"
              style={{ background: 'var(--bg-surface)', overflow: 'hidden' }}
            >
              <div
                className="h-full rounded-full transition-all"
                style={{ width: `${progressPct}%`, background: progressColor }}
              />
            </div>
            <span
              className="text-xs"
              style={{ color: isOver ? 'var(--error)' : 'var(--text-4)', minWidth: '90px' }}
            >
              {charCount.toLocaleString()} / {MAX_ARCANA_CHARS.toLocaleString()}
            </span>
            {isOver && (
              <span className="text-xs" style={{ color: 'var(--error)' }}>
                OVER LIMIT
              </span>
            )}
          </div>

          {/* Actions */}
          <button
            onClick={handleInsertTemplate}
            className="text-xs px-2 py-1 rounded transition-opacity hover:opacity-80"
            style={{
              background: 'var(--bg-surface)',
              border:     '1px solid var(--border-2)',
              color:      'var(--text-3)',
              cursor:     'pointer',
            }}
          >
            Insert Template
          </button>

          {isModified && (
            <button
              onClick={handleReset}
              className="flex items-center gap-1 text-xs px-2 py-1 rounded transition-opacity hover:opacity-80"
              style={{
                background: 'var(--bg-surface)',
                border:     '1px solid var(--border-2)',
                color:      'var(--text-3)',
                cursor:     'pointer',
              }}
            >
              <RotateCcw size={10} />
              Reset
            </button>
          )}

          <button
            onClick={handleSave}
            disabled={isOver || store.isSaving || (scope === 'repo' && !repoPath)}
            className="flex items-center gap-1 text-xs px-3 py-1 rounded transition-opacity"
            style={{
              background: saved ? 'var(--success)' : 'var(--accent)',
              color:      '#fff',
              border:     'none',
              cursor:     isOver ? 'not-allowed' : 'pointer',
              opacity:    isOver || store.isSaving ? 0.5 : 1,
            }}
          >
            <Save size={10} />
            {store.isSaving ? 'Saving…' : saved ? 'Saved!' : 'Save'}
          </button>
        </div>
      </div>
    </div>
  )
}
