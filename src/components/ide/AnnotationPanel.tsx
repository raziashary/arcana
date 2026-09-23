// AnnotationPanel — side panel showing the details of a clicked annotation.
// Slides in from the right of the CodeViewer when a gutter icon is clicked.
// Supports resolving (with optional response text) and deleting annotations.

import { useState } from 'react'
import { useAnnotationStore, type Annotation } from '../../stores/annotationStore'
import { useUiStore } from '../../stores/uiStore'
import './Annotations.css'

interface AnnotationPanelProps {
  annotation: Annotation
}

export function AnnotationPanel({ annotation }: AnnotationPanelProps) {
  const resolve      = useAnnotationStore((s) => s.resolve)
  const remove       = useAnnotationStore((s) => s.remove)
  const setActive    = useAnnotationStore((s) => s.setActive)
  const setActiveTab = useUiStore((s) => s.setActiveTab)
  const [resolveText, setResolveText] = useState('')
  const [resolving, setResolving]     = useState(false)
  const [showResolve, setShowResolve] = useState(false)

  const locationLabel = annotation.startLine === annotation.endLine
    ? `Line ${annotation.startLine}`
    : `Lines ${annotation.startLine}–${annotation.endLine}`

  async function handleResolve() {
    setResolving(true)
    try {
      await resolve(annotation.id, resolveText.trim() || undefined)
      setShowResolve(false)
    } finally {
      setResolving(false)
    }
  }

  async function handleDelete() {
    await remove(annotation.id)
  }

  const isResolved = annotation.status === 'resolved'

  return (
    <div className="ann-panel">
      <div className="ann-panel-header">
        <span className="ann-panel-title">Annotation</span>
        <button className="ann-panel-close" onClick={() => setActive(null)}>×</button>
      </div>

      <div className="ann-panel-body">
        {/* Meta row */}
        <div className="ann-panel-meta">
          <div>
            <span className="ann-panel-location">{annotation.filePath}</span>
            {' · '}{locationLabel}
          </div>
          <div style={{ marginTop: 4 }}>
            <span className={`ann-severity-badge ann-severity-badge--${annotation.severity}`}>
              {annotation.severity}
            </span>
            <span className={`ann-status-badge${isResolved ? ' ann-status-badge--resolved' : ''}`}>
              {annotation.status}
            </span>
          </div>
          <div style={{ marginTop: 4 }}>
            <span className={`ann-panel-source ann-panel-source--${annotation.source}`}>
              {annotation.source === 'qc_agent' ? '◈ QC Agent'
               : annotation.source === 'worker' ? '◈ Worker'
               : '◈ You'}
            </span>
            {' · '}{new Date(annotation.createdAt).toLocaleString()}
          </div>
        </div>

        {/* Selected code preview */}
        {annotation.selectedText && (
          <div>
            <div className="ann-panel-section-label">Selected code</div>
            <pre className="ann-panel-code">{annotation.selectedText}</pre>
          </div>
        )}

        {/* Prompt */}
        <div>
          <div className="ann-panel-section-label">Note / Prompt</div>
          <div className="ann-panel-prompt">{annotation.prompt}</div>
        </div>

        {/* PA response (populated after "Ask PA" completes) */}
        {annotation.response ? (
          <div>
            <div className="ann-panel-section-label">PA Response</div>
            <div className="ann-panel-response">{annotation.response}</div>
          </div>
        ) : annotation.source === 'user' && annotation.prompt && !annotation.taskId && annotation.status !== 'resolved' ? (
          // Hint that the response appears in Chat tab
          <div
            style={{ fontSize: 10, color: 'var(--text-3)', fontStyle: 'italic', marginTop: 4 }}
          >
            Response will appear in the Chat tab and can be reviewed there.
          </div>
        ) : null}

        {/* Fleet task link (populated after "Delegate to Fleet" dispatches work) */}
        {annotation.taskId && (
          <div>
            <div className="ann-panel-section-label">Fleet Task</div>
            <button
              className="ann-panel-task-link"
              title="Open Arcana panel to view this task"
              onClick={() => {
                setActive(null)
                setActiveTab('arcana')
              }}
            >
              ◈ {annotation.taskId}
            </button>
          </div>
        )}

        {/* Inline resolve form */}
        {showResolve && !isResolved && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            <div className="ann-panel-section-label">Resolution note (optional)</div>
            <textarea
              style={{
                background:  '#161622',
                border:      '1px solid var(--border-1)',
                borderRadius: 2,
                color:       'var(--text-2)',
                fontFamily:  'var(--font-mono)',
                fontSize:    11,
                padding:     '6px 8px',
                resize:      'vertical',
                minHeight:   60,
              }}
              placeholder="How was this addressed?"
              value={resolveText}
              onChange={(e) => setResolveText(e.target.value)}
            />
            <div style={{ display: 'flex', gap: 6 }}>
              <button
                className="ann-panel-btn ann-panel-btn--resolve"
                onClick={handleResolve}
                disabled={resolving}
              >
                {resolving ? 'Saving…' : 'Mark resolved'}
              </button>
              <button
                className="ann-panel-btn"
                onClick={() => setShowResolve(false)}
              >
                Cancel
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Actions footer */}
      <div className="ann-panel-actions">
        {!isResolved && !showResolve && (
          <button
            className="ann-panel-btn ann-panel-btn--resolve"
            onClick={() => setShowResolve(true)}
          >
            Resolve
          </button>
        )}
        <button
          className="ann-panel-btn ann-panel-btn--delete"
          onClick={handleDelete}
        >
          Delete
        </button>
      </div>
    </div>
  )
}
