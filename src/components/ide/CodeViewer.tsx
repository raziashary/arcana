// Code viewer — Monaco-based read-only editor.
//
// Rationale: Arcana never edits files from the UI; agents mutate files via
// the write_file / edit_file tools. The IDE viewer is therefore read-only,
// which also simplifies state management and avoids stale-buffer bugs.

import { useEffect, useRef, useState } from 'react'
import Editor, { loader, type OnMount } from '@monaco-editor/react'
import * as monaco from 'monaco-editor'
import { useIdeStore } from '../../stores/ideStore'
import { useAnnotationStore, type Annotation } from '../../stores/annotationStore'
import { SelectionToolbar, type AnnotationIntent } from './SelectionToolbar'
import { AnnotationModal } from './AnnotationModal'
import { AnnotationPanel } from './AnnotationPanel'

// Configure @monaco-editor/react to use the locally bundled monaco-editor
// instead of loading it from a CDN. This is required because the Tauri CSP
// (script-src 'self') blocks external script loading, which would otherwise
// cause the editor to hang forever on "Loading...".
loader.config({ monaco })

interface SelectionState {
  startLine: number
  endLine:   number
  text:      string
  top:       number
  left:      number
}

interface Props {
  tabPath: string
}

// Stable empty array — must NOT be inline (`?? []`) inside a Zustand selector.
// `?? []` creates a new reference on every render; Zustand uses Object.is to
// compare selector results, so a new `[]` always looks "changed" and triggers
// another render → infinite loop → "Maximum update depth exceeded".
const EMPTY_ANNOTATIONS: Annotation[] = []

export default function CodeViewer({ tabPath }: Props) {
  const tab = useIdeStore((s) => s.openTabs.find((t) => t.relativePath === tabPath))
  const editorRef      = useRef<Parameters<OnMount>[0] | null>(null)
  const decorationsRef = useRef<monaco.editor.IEditorDecorationsCollection | null>(null)
  // Always-current tabPath for use inside stable onMount closures
  const tabPathRef     = useRef(tabPath)
  tabPathRef.current   = tabPath

  const [selection,    setSelection]    = useState<SelectionState | null>(null)
  const [modalIntent,  setModalIntent]  = useState<AnnotationIntent | null>(null)
  const [editorReady,  setEditorReady]  = useState(false)

  const annotations      = useAnnotationStore((s) => s.byFile[tabPath] ?? EMPTY_ANNOTATIONS)
  const activeId         = useAnnotationStore((s) => s.activeId)
  const activeAnnotation = useAnnotationStore((s) => {
    if (!s.activeId) return null
    for (const list of Object.values(s.byFile)) {
      const found = list.find((a) => a.id === s.activeId)
      if (found) return found
    }
    return null
  })
  const loadForFile = useAnnotationStore((s) => s.loadForFile)

  // Load annotations for the current file
  useEffect(() => {
    if (tabPath) void loadForFile(tabPath)
  }, [tabPath, loadForFile])

  // Apply/refresh gutter decorations whenever annotations or editor readiness changes
  useEffect(() => {
    const editor = editorRef.current
    if (!editor) return

    if (!decorationsRef.current) {
      decorationsRef.current = editor.createDecorationsCollection([])
    }

    const decors: monaco.editor.IModelDeltaDecoration[] = []
    for (const ann of annotations) {
      if (ann.status === 'resolved') continue
      decors.push({
        range: new monaco.Range(ann.startLine, 1, ann.endLine, 1),
        options: {
          isWholeLine:              true,
          linesDecorationsClassName: `ann-gutter-${ann.severity}`,
          className:                `ann-line-${ann.severity}`,
        },
      })
    }
    decorationsRef.current.set(decors)
  }, [annotations, editorReady])

  const handleMount: OnMount = (editor, monacoInst) => {
    editorRef.current = editor

    monacoInst.editor.defineTheme('arcana-dark', {
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
    monacoInst.editor.setTheme('arcana-dark')

    // Track selection to show the floating toolbar
    editor.onDidChangeCursorSelection(() => {
      const sel = editor.getSelection()
      if (!sel || sel.isEmpty()) {
        setSelection(null)
        return
      }
      const text       = editor.getModel()?.getValueInRange(sel) ?? ''
      const lineHeight = editor.getOption(monacoInst.editor.EditorOption.lineHeight)
      const scrollTop  = editor.getScrollTop()
      const layout     = editor.getLayoutInfo()
      const lineTop    = (sel.startLineNumber - 1) * lineHeight - scrollTop
      setSelection({
        startLine: sel.startLineNumber,
        endLine:   sel.endLineNumber,
        text,
        top:  Math.max(lineTop - 34, 4),
        left: layout.contentLeft + 8,
      })
    })

    // Gutter click → open annotation panel for that line
    editor.onMouseDown((e) => {
      const MT   = monacoInst.editor.MouseTargetType
      const type = e.target.type
      if (
        type !== MT.GUTTER_LINE_DECORATIONS &&
        type !== MT.GUTTER_GLYPH_MARGIN &&
        type !== MT.GUTTER_LINE_NUMBERS
      ) return
      const line = e.target.position?.lineNumber
      if (!line) return
      const ann = useAnnotationStore.getState().getForFile(tabPathRef.current)
        .find((a) => a.status !== 'resolved' && line >= a.startLine && line <= a.endLine)
      if (ann) useAnnotationStore.getState().setActive(ann.id)
    })

    setEditorReady(true)
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

  const showToolbar = selection !== null && modalIntent === null && activeId === null

  return (
    <div className="flex-1 overflow-hidden" style={{ position: 'relative', background: '#01111D' }}>
      <Editor
        value={tab.content.content}
        language={tab.content.language}
        theme="arcana-dark"
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

      {showToolbar && (
        <SelectionToolbar
          top={selection.top}
          left={selection.left}
          onAction={(intent) => setModalIntent(intent)}
        />
      )}

      {modalIntent !== null && selection !== null && (
        <AnnotationModal
          filePath={tabPath}
          startLine={selection.startLine}
          endLine={selection.endLine}
          selectedText={selection.text}
          intent={modalIntent}
          onClose={() => {
            setModalIntent(null)
            setSelection(null)
          }}
        />
      )}

      {activeAnnotation && (
        <AnnotationPanel annotation={activeAnnotation} />
      )}
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
