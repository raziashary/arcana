// AnnotationModal — prompt input for creating a new annotation.
// Invoked by SelectionToolbar for 'ask', 'delegate', and 'note' intents.
// 'ask' → creates annotation + sends prompt to PA via send_message
// 'delegate' → creates annotation with pending status (user creates task separately)
// 'note'     → creates annotation as a plain note, no PA call

import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useAnnotationStore } from '../../stores/annotationStore'
import type { AnnotationIntent } from './SelectionToolbar'
import './Annotations.css'

interface AnnotationModalProps {
  filePath:    string
  startLine:   number
  endLine:     number
  selectedText: string
  intent:      AnnotationIntent
  onClose:     () => void
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
  const [prompt, setPrompt]       = useState('')
  const [severity, setSeverity]   = useState<'info' | 'warning' | 'blocking'>('info')
  const [submitting, setSubmitting] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const { create } = useAnnotationStore()

  useEffect(() => {
    textareaRef.current?.focus()
  }, [])

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [onClose])

  async function handleSubmit() {
    if (!prompt.trim()) return
    setSubmitting(true)
    try {
      const annotation = await create({
        filePath,
        startLine,
        endLine,
        selectedText: selectedText || undefined,
        prompt:       prompt.trim(),
        source:       'user',
        severity,
      })

      if (intent === 'ask') {
        // Inject the selected code + question into the PA chat
        const codeContext = selectedText
          ? `\`\`\`\n${selectedText}\n\`\`\`\n\n`
          : ''
        const fullMessage = `[${filePath}:${startLine}-${endLine}]\n${codeContext}${prompt.trim()}`
        // Fire-and-forget — the Chat tab handles the streaming response
        void invoke('send_message', {
          sessionId:   null,   // use active session
          userMessage: fullMessage,
          paMode:      null,   // use current PA mode
        }).catch(() => {/* non-fatal */})
      }

      void annotation  // suppress unused warning — side-effect is store update
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
            if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) handleSubmit()
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
            onClick={handleSubmit}
            disabled={!prompt.trim() || submitting}
          >
            {submitting ? 'Saving…' : 'Submit ↵'}
          </button>
        </div>
      </div>
    </div>
  )
}
