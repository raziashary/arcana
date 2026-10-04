// AnnotationModal — prompt input for creating a new annotation.
// Invoked by SelectionToolbar for 'ask', 'delegate', and 'note' intents.
//
// Ask PA ('ask'):
//   1. Save annotation to DB (bookmark of the question + code selection)
//   2. Get or create a Chat session (same pattern as Chat.tsx ensureSession)
//   3. Call send_message with the formatted prompt → PA responds in the Chat tab
//
// Delegate to Fleet ('delegate'):
//   1. Save annotation to DB with source='user'
//   2. Call send_message with paMode='delegate' so the PA uses create_task
//      to dispatch the work to the worker fleet
//
// Note ('note'):
//   Save annotation only — no PA call

import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useAnnotationStore } from '../../stores/annotationStore'
import { useSessionStore }    from '../../stores/sessionStore'
import { useSettingsStore }   from '../../stores/settingsStore'
import { useWorkspaceStore }  from '../../stores/workspaceStore'
import type { AnnotationIntent } from './SelectionToolbar'
import './Annotations.css'

interface AnnotationModalProps {
  filePath:     string
  startLine:    number
  endLine:      number
  selectedText: string
  intent:       AnnotationIntent
  onClose:      () => void
}

const INTENT_LABELS: Record<AnnotationIntent, string> = {
  ask:      'Ask PA',
  delegate: 'Delegate to Fleet',
  note:     'Add Note',
}

const INTENT_PLACEHOLDERS: Record<AnnotationIntent, string> = {
  ask:      'What would you like to know about this code?',
  delegate: 'Describe what change you want on these lines…',
  note:     'Add a note or comment about this code…',
}

export function AnnotationModal({
  filePath,
  startLine,
  endLine,
  selectedText,
  intent,
  onClose,
}: AnnotationModalProps) {
  const [prompt,     setPrompt]     = useState('')
  const [severity,   setSeverity]   = useState<'info' | 'warning' | 'blocking'>('info')
  const [submitting, setSubmitting] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  // Selectors — individual to avoid full-store re-subscriptions
  const create          = useAnnotationStore((s) => s.create)
  const activeSessionId = useSessionStore((s) => s.activeSessionId)
  const createSession   = useSessionStore((s) => s.createSession)
  const activeProvider  = useSettingsStore((s) => s.activeProvider)
  const activeModel     = useSettingsStore((s) => s.activeModel)
  const paMode          = useSettingsStore((s) => s.paMode)
  const workspaceDir    = useWorkspaceStore((s) => s.workspaceDir)

  useEffect(() => {
    textareaRef.current?.focus()
  }, [])

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [onClose])

  async function ensureSession(): Promise<string> {
    if (activeSessionId) return activeSessionId
    const provider = activeProvider ?? 'google'
    const model    = activeModel    ?? 'gemini-3.8-flash'
    const dir      = workspaceDir   ?? ''
    return createSession(dir, provider, model)
  }

  async function handleSubmit() {
    if (!prompt.trim()) return
    setSubmitting(true)
    try {
      // 1. Persist the annotation (bookmark) first so it's visible even if
      //    the PA call below fails or the user closes the Chat tab.
      await create({
        filePath,
        startLine,
        endLine,
        selectedText: selectedText || undefined,
        prompt:       prompt.trim(),
        source:       'user',
        severity,
      })

      // 2. Send to PA / fleet for 'ask' and 'delegate' intents
      if (intent === 'ask' || intent === 'delegate') {
        const sessionId = await ensureSession()

        // Format: file location header + code fence + user question/instruction
        const codeContext  = selectedText ? `\`\`\`\n${selectedText}\n\`\`\`\n\n` : ''
        const fullMessage  = `[${filePath}:${startLine}–${endLine}]\n${codeContext}${prompt.trim()}`

        // 'ask' → use the current PA mode (Fast Dev / Review) so normal PA rules apply
        // 'delegate' → force paMode='delegate' so PA only uses create_task
        // P8-32 — 'Delegate to Fleet' is an EXPLICIT one-shot choice, so it
        // travels as `paModeOverride`, which outranks the session row.
        //
        // 🔴 It used to travel as `paMode`, and the backend prefers
        // `session.pa_mode` whenever that is non-empty — which it always is,
        // because the column is `NOT NULL DEFAULT 'fast_development'`. So the
        // argument was unreachable and this button ran in whatever mode the
        // session already had: the label said Delegate and the PA wrote code.
        //
        // ⚠ `paMode` still carries the ambient store value and still loses to
        // the session, deliberately — that ordering is what stops a reload
        // from resetting the PA's mode mid-conversation. The override is a
        // separate field precisely so a stale store cannot borrow its
        // authority.
        void invoke('send_message', {
          sessionId,
          userMessage: fullMessage,
          paMode,
          paModeOverride: intent === 'delegate' ? 'delegate' : null,
        }).catch((e: unknown) => {
          console.error('[AnnotationModal] send_message failed:', e)
        })
      }

      onClose()
    } catch (e) {
      console.error('[AnnotationModal] submit failed:', e)
    } finally {
      setSubmitting(false)
    }
  }

  const locationLabel = startLine === endLine
    ? `Line ${startLine}`
    : `Lines ${startLine}–${endLine}`

  return (
    <div className="ann-modal-overlay" onClick={(e) => { if (e.target === e.currentTarget) onClose() }}>
      <div className="ann-modal">
        <div className="ann-modal-header">
          <span className="ann-modal-title">{INTENT_LABELS[intent]}</span>
          <button className="ann-modal-close" onClick={onClose}>×</button>
        </div>

        {selectedText && (
          <pre className="ann-modal-code-preview">{selectedText}</pre>
        )}

        <div className="ann-modal-location">
          {filePath} · {locationLabel}
        </div>

        <textarea
          ref={textareaRef}
          className="ann-modal-textarea"
          placeholder={INTENT_PLACEHOLDERS[intent]}
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) void handleSubmit()
          }}
          rows={4}
        />

        <div className="ann-modal-footer">
          <div className="ann-modal-severity">
            <span>Severity</span>
            <select value={severity} onChange={(e) => setSeverity(e.target.value as typeof severity)}>
              <option value="info">info</option>
              <option value="warning">warning</option>
              <option value="blocking">blocking</option>
            </select>
          </div>
          <button
            className="ann-modal-submit"
            onClick={() => void handleSubmit()}
            disabled={!prompt.trim() || submitting}
          >
            {submitting ? 'Saving…' : 'Submit ↵'}
          </button>
        </div>
      </div>
    </div>
  )
}
