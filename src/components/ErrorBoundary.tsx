// ErrorBoundary — catches React render errors and shows a readable error card
// instead of a blank screen. Without this, any unhandled throw during rendering
// kills the entire React tree; Tauri's CSP also blocks Vite's error overlay so
// the user only sees a blank white screen with no clue what went wrong.
//
// Usage: wrap any subtree that should be isolated (e.g., each tab panel).

import { Component, type ErrorInfo, type ReactNode } from 'react'

interface Props {
  children:  ReactNode
  /** Optional label shown in the error card heading, e.g. "IDE" */
  label?:    string
  /** If true, render a compact inline error instead of a full-panel card */
  inline?:   boolean
}

interface State {
  error:     Error | null
  errorInfo: ErrorInfo | null
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null, errorInfo: null }

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    this.setState({ error, errorInfo: info })
    console.error('[ErrorBoundary] Caught render error:', error, info)
  }

  handleReset = () => {
    this.setState({ error: null, errorInfo: null })
  }

  render() {
    const { error, errorInfo } = this.state
    const { children, label, inline } = this.props

    if (!error) return children

    const title   = label ? `${label} crashed` : 'Something went wrong'
    const message = error.message ?? String(error)
    const stack   = errorInfo?.componentStack ?? ''

    if (inline) {
      return (
        <div
          style={{
            padding:    '8px 12px',
            background: 'var(--danger-bg, #2a0a0a)',
            border:     '1px solid var(--danger, #e45)',
            borderRadius: 4,
            color:      'var(--danger, #e45)',
            fontFamily: 'var(--font-mono, monospace)',
            fontSize:   11,
          }}
        >
          <strong>{title}:</strong> {message}
          <button
            onClick={this.handleReset}
            style={{ marginLeft: 8, cursor: 'pointer', textDecoration: 'underline', background: 'none', border: 'none', color: 'inherit', fontSize: 'inherit' }}
          >
            retry
          </button>
        </div>
      )
    }

    return (
      <div
        style={{
          flex:           1,
          display:        'flex',
          flexDirection:  'column',
          alignItems:     'center',
          justifyContent: 'center',
          padding:        32,
          background:     'var(--bg-panel, #0a0a14)',
          color:          'var(--text-1, #d6deeb)',
        }}
      >
        <div
          style={{
            maxWidth:     600,
            width:        '100%',
            border:       '1px solid var(--danger, #e45)',
            borderRadius: 6,
            overflow:     'hidden',
          }}
        >
          {/* Header */}
          <div
            style={{
              display:        'flex',
              alignItems:     'center',
              justifyContent: 'space-between',
              padding:        '10px 16px',
              background:     'var(--danger-bg, #2a0a0a)',
              borderBottom:   '1px solid var(--danger, #e45)',
            }}
          >
            <span style={{ fontFamily: 'var(--font-mono, monospace)', fontSize: 13, color: 'var(--danger, #e45)', fontWeight: 600 }}>
              ⚠ {title}
            </span>
            <button
              onClick={this.handleReset}
              style={{
                padding:      '3px 10px',
                background:   'transparent',
                border:       '1px solid var(--danger, #e45)',
                borderRadius: 3,
                color:        'var(--danger, #e45)',
                cursor:       'pointer',
                fontFamily:   'var(--font-mono, monospace)',
                fontSize:     11,
              }}
            >
              Retry
            </button>
          </div>

          {/* Error message */}
          <div style={{ padding: '12px 16px', background: 'var(--bg-card, #0d1b2a)' }}>
            <div
              style={{
                fontFamily:  'var(--font-mono, monospace)',
                fontSize:    12,
                color:       'var(--text-1, #d6deeb)',
                marginBottom: 12,
                whiteSpace:  'pre-wrap',
                wordBreak:   'break-word',
              }}
            >
              {message}
            </div>

            {stack && (
              <details style={{ marginTop: 8 }}>
                <summary
                  style={{
                    cursor:      'pointer',
                    fontFamily:  'var(--font-mono, monospace)',
                    fontSize:    10,
                    color:       'var(--text-3, #637777)',
                    userSelect:  'none',
                  }}
                >
                  Component stack
                </summary>
                <pre
                  style={{
                    marginTop:   6,
                    padding:     '8px 10px',
                    background:  'var(--bg-surface, #01111d)',
                    borderRadius: 3,
                    fontFamily:  'var(--font-mono, monospace)',
                    fontSize:    10,
                    color:       'var(--text-3, #637777)',
                    whiteSpace:  'pre-wrap',
                    wordBreak:   'break-all',
                    overflowX:   'auto',
                    maxHeight:   200,
                    overflowY:   'auto',
                  }}
                >
                  {stack}
                </pre>
              </details>
            )}
          </div>
        </div>
      </div>
    )
  }
}
