// Code viewer — Monaco-based read-only editor.
//
// Rationale: AutoDev never edits files from the UI; agents mutate files via
// the write_file / edit_file tools. The IDE viewer is therefore read-only,
// which also simplifies state management and avoids stale-buffer bugs.

import { useEffect, useRef } from 'react'
import Editor, { loader, type OnMount } from '@monaco-editor/react'
import * as monaco from 'monaco-editor'
import { useIdeStore } from '../../stores/ideStore'

// Configure @monaco-editor/react to use the locally bundled monaco-editor
// instead of loading it from a CDN. This is required because the Tauri CSP
// (script-src 'self') blocks external script loading, which would otherwise
// cause the editor to hang forever on "Loading...".
loader.config({ monaco })

interface Props {
  tabPath: string
}

export default function CodeViewer({ tabPath }: Props) {
  const tab = useIdeStore((s) => s.openTabs.find((t) => t.relativePath === tabPath))
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null)

  const handleMount: OnMount = (editor, monaco) => {
    editorRef.current = editor

    // Apply a theme tuned to the Winter-Is-Coming palette.
    monaco.editor.defineTheme('autodev-dark', {
      base:    'vs-dark',
      inherit: true,
      rules: [
        { token: '',          foreground: 'D6DEEB', background: '01111D' },
        { token: 'comment',   foreground: '637777', fontStyle: 'italic'  },
        { token: 'keyword',   foreground: '82AAFF'                       },
        { token: 'string',    foreground: 'ECC48D'                       },
        { token: 'number',    foreground: 'F78C6C'                       },
        { token: 'type',      foreground: '7FDBCA'                       },
        { token: 'function',  foreground: '82AAFF'                       },
        { token: 'variable',  foreground: 'D6DEEB'                       },
      ],
      colors: {
        'editor.background':                '#01111D',
        'editor.foreground':                '#D6DEEB',
        'editor.lineHighlightBackground':   '#022C4A',
        'editorLineNumber.foreground':      '#3D5A6E',
        'editorLineNumber.activeForeground':'#82AAFF',
        'editorCursor.foreground':          '#82AAFF',
        'editor.selectionBackground':       '#0B3A5C',
        'editor.inactiveSelectionBackground':'#0B3A5C80',
        'editorWidget.background':          '#00233B',
        'editorWidget.border':              '#0D3652',
      },
    })
    monaco.editor.setTheme('autodev-dark')
  }

  // React to gotoLine changes when "Open in IDE from chat" requests a jump.
  useEffect(() => {
    if (tab?.gotoLine && editorRef.current) {
      editorRef.current.revealLineInCenter(tab.gotoLine)
      editorRef.current.setPosition({ lineNumber: tab.gotoLine, column: 1 })
      editorRef.current.focus()
    }
  }, [tab?.gotoLine, tabPath])

  if (!tab) return null

  if (tab.loading) {
    return <CenteredMsg>Loading {tab.relativePath}…</CenteredMsg>
  }
  if (tab.error) {
    return (
      <CenteredMsg danger>
        <div className="flex flex-col items-center gap-3">
          <span>{tab.error}</span>
          <button
            type="button"
            onClick={() => useIdeStore.getState().openFile(tab.relativePath)}
            className="px-3 py-1.5 text-xs font-mono rounded border hover:bg-[var(--bg-hover)]"
            style={{ borderColor: 'var(--border-1)', color: 'var(--text-2)' }}
          >
            Retry
          </button>
        </div>
      </CenteredMsg>
    )
  }
  if (!tab.content) {
    return <CenteredMsg>No content</CenteredMsg>
  }
  if (tab.content.is_binary) {
    return (
      <CenteredMsg>
        Binary file ({formatBytes(tab.content.size_bytes)})
      </CenteredMsg>
    )
  }

  return (
    <div className="flex-1 overflow-hidden" style={{ background: '#01111D' }}>
      <Editor
        value={tab.content.content}
        language={tab.content.language}
        theme="autodev-dark"
        onMount={handleMount}
        options={{
          readOnly:                true,
          fontFamily:              'IBM Plex Mono, monospace',
          fontSize:                12,
          lineNumbers:             'on',
          minimap:                 { enabled: true },
          scrollBeyondLastLine:    false,
          renderLineHighlight:     'line',
          smoothScrolling:         true,
          folding:                 true,
          renderWhitespace:        'selection',
          wordWrap:                'off',
          automaticLayout:         true,
          scrollbar: {
            verticalScrollbarSize:   8,
            horizontalScrollbarSize: 8,
          },
        }}
      />
    </div>
  )
}

function CenteredMsg({ children, danger }: { children: React.ReactNode; danger?: boolean }) {
  return (
    <div
      className="flex-1 flex items-center justify-center p-6 text-center"
      style={{ color: danger ? 'var(--danger)' : 'var(--text-3)' }}
    >
      <span className="text-xs font-mono">{children}</span>
    </div>
  )
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / (1024 * 1024)).toFixed(1)} MB`
}
