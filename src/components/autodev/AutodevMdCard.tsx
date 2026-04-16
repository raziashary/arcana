// AUTODEV.md Card — sidebar widget showing status and quick-edit buttons.
// Displays global + repo instruction status with character counts.
// Opens AutodevMdEditor overlay when edit buttons clicked.
// See: DEVELOPMENT_PLAN.md Phase 2.1.2

import { useEffect, useState } from 'react'
import { FileText } from 'lucide-react'
import { useAutodevMdStore, MAX_AUTODEV_CHARS } from '../../stores/autodevMdStore'
import { useAgentStore } from '../../stores/agentStore'
import AutodevMdEditor from './AutodevMdEditor'

function StatusDot({ content }: { content: string }) {
  const isEmpty    = content.trim().length === 0
  const isOver     = content.length > MAX_AUTODEV_CHARS * 0.85
  const background = isEmpty ? '#0D3652' : isOver ? 'var(--warning)' : 'var(--success)'
  return (
    <div
      className="shrink-0 rounded-full"
      style={{ width: '5px', height: '5px', background }}
    />
  )
}

function CharCount({ content }: { content: string }) {
  const count   = content.length
  const isOver  = count > MAX_AUTODEV_CHARS
  return (
    <span style={{ color: isOver ? 'var(--error)' : 'var(--text-4)', fontSize: '9px' }}>
      {count > 0 ? `${count.toLocaleString()} ch` : 'empty'}
    </span>
  )
}

export default function AutodevMdCard() {
  const store      = useAutodevMdStore()
  const workingDir = useAgentStore(s => s.workingDir)

  const [editorOpen,  setEditorOpen]  = useState(false)
  const [editorScope, setEditorScope] = useState<'global' | 'repo'>('global')

  useEffect(() => {
    store.loadGlobal()
  }, [])

  useEffect(() => {
    if (workingDir) store.loadRepo(workingDir)
  }, [workingDir])

  const openEditor = (scope: 'global' | 'repo') => {
    setEditorScope(scope)
    setEditorOpen(true)
  }

  const repoContent = workingDir ? (store.repoContents[workingDir] ?? '') : ''

  return (
    <>
      <div className="font-mono" style={{ fontSize: '10px' }}>
        {/* Section header */}
        <div
          className="flex items-center gap-1.5 mb-1.5"
          style={{ color: 'var(--text-4)', letterSpacing: '0.08em', textTransform: 'uppercase' }}
        >
          <FileText size={9} />
          <span>AUTODEV.md</span>
        </div>

        {/* Global row */}
        <div className="flex items-center gap-1.5 py-0.5">
          <StatusDot content={store.globalContent} />
          <span className="flex-1 truncate" style={{ color: 'var(--text-3)' }}>
            global
          </span>
          <CharCount content={store.globalContent} />
          <button
            onClick={() => openEditor('global')}
            className="rounded transition-opacity hover:opacity-80"
            style={{
              fontSize:   '9px',
              padding:    '1px 5px',
              background: 'var(--bg-card)',
              border:     '1px solid var(--border-2)',
              color:      'var(--text-3)',
              cursor:     'pointer',
            }}
          >
            Edit
          </button>
        </div>

        {/* Repo row */}
        <div className="flex items-center gap-1.5 py-0.5">
          <StatusDot content={repoContent} />
          <span
            className="flex-1 truncate"
            style={{ color: workingDir ? 'var(--text-3)' : 'var(--text-4)' }}
            title={workingDir ?? 'No working directory set'}
          >
            {workingDir ? 'this repo' : 'no repo'}
          </span>
          {workingDir && <CharCount content={repoContent} />}
          <button
            onClick={() => openEditor('repo')}
            disabled={!workingDir}
            className="rounded transition-opacity hover:opacity-80"
            style={{
              fontSize:   '9px',
              padding:    '1px 5px',
              background: 'var(--bg-card)',
              border:     '1px solid var(--border-2)',
              color:      workingDir ? 'var(--text-3)' : 'var(--text-4)',
              cursor:     workingDir ? 'pointer' : 'not-allowed',
              opacity:    workingDir ? 1 : 0.4,
            }}
          >
            Edit
          </button>
        </div>
      </div>

      {editorOpen && (
        <AutodevMdEditor
          initialScope={editorScope}
          onClose={() => setEditorOpen(false)}
        />
      )}
    </>
  )
}
