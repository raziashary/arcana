// Git panel — togglable sub-panel inside the IDE tab.
//
// Five sections, exposed as top-level tabs:
//   Changes  — staged / unstaged / untracked (with diff preview below)
//   History  — commit log
//   Branches — local + remote, with create/delete/checkout
//   Stash    — list + push / apply / drop
//   Undo     — checkpoint timeline (auto-snapshots)
//
// The panel talks to `useGitStore`. It opens the repository lazily the
// first time it is mounted for a given workspace.

import { useEffect, useState } from 'react'
import {
  ArrowDown,
  ArrowUp,
  FileDiff,
  GitBranch,
  GitCommit,
  History,
  Minus,
  Plus,
  RefreshCw,
  RotateCcw,
  Trash2,
  X,
} from 'lucide-react'
import { formatDistanceToNow } from 'date-fns'

import { useGitStore, type StatusEntry } from '../../stores/gitStore'
import { useWorkspaceStore } from '../../stores/workspaceStore'

import DiffViewer        from './DiffViewer'
import BranchManager     from './BranchManager'
import StashManager      from './StashManager'
import CheckpointViewer  from './CheckpointViewer'

type Tab = 'changes' | 'history' | 'branches' | 'stash' | 'undo'

interface Props {
  onClose: () => void
}

export default function GitPanel({ onClose }: Props) {
  const { workspaceDir } = useWorkspaceStore()

  const repo           = useGitStore((s) => s.repo)
  const status         = useGitStore((s) => s.status)
  const error          = useGitStore((s) => s.error)
  const openRepo       = useGitStore((s) => s.openRepo)
  const refreshAll     = useGitStore((s) => s.refreshAll)
  const fetch          = useGitStore((s) => s.fetch)
  const pull           = useGitStore((s) => s.pull)
  const push           = useGitStore((s) => s.push)

  const [tab, setTab] = useState<Tab>('changes')

  // Boot: open the repository for the workspace folder. Errors (e.g. the
  // workspace isn't a git repo) are surfaced via `error` in the store.
  useEffect(() => {
    if (workspaceDir && (!repo || repo.workdir.replace(/\\/g, '/') !== workspaceDir.replace(/\\/g, '/'))) {
      void openRepo(workspaceDir)
    }
  }, [workspaceDir])

  return (
    <div
      className="flex flex-col border-l"
      style={{ borderColor: 'var(--border-1)', width: 360, minWidth: 280, background: 'var(--bg-panel)' }}
    >
      {/* Header */}
      <div
        className="flex items-center gap-1 px-2 py-1.5 border-b"
        style={{ borderColor: 'var(--border-1)' }}
      >
        <GitBranch size={12} style={{ color: 'var(--accent)' }} />
        <span
          className="text-[10px] font-mono uppercase tracking-wider"
          style={{ color: 'var(--text-2)' }}
        >
          Git
        </span>
        {status?.head_branch && (
          <span className="text-[11px] font-mono ml-2" style={{ color: 'var(--text-1)' }}>
            {status.head_branch}
          </span>
        )}
        {status && (status.ahead > 0 || status.behind > 0) && (
          <span className="text-[10px] font-mono ml-1" style={{ color: 'var(--text-3)' }}>
            {status.ahead > 0 && `↑${status.ahead}`}{status.behind > 0 && `↓${status.behind}`}
          </span>
        )}
        <div className="flex-1" />
        <IconBtn title="Fetch"   onClick={() => void fetch()}><RefreshCw size={11} /></IconBtn>
        <IconBtn title="Pull"    onClick={() => void pull()}><ArrowDown size={11} /></IconBtn>
        <IconBtn title="Push"    onClick={() => void push()}><ArrowUp size={11} /></IconBtn>
        <IconBtn title="Refresh" onClick={() => void refreshAll()}><RotateCcw size={11} /></IconBtn>
        <IconBtn title="Close"   onClick={onClose}><X size={11} /></IconBtn>
      </div>

      {!repo && (
        <div
          className="flex-1 flex items-center justify-center text-[11px] font-mono p-4 text-center"
          style={{ color: error ? 'var(--danger)' : 'var(--text-3)' }}
        >
          {error ?? 'Opening repository…'}
        </div>
      )}

      {repo && (
        <>
          {/* Tab bar */}
          <div
            className="flex border-b text-[10px] font-mono uppercase"
            style={{ borderColor: 'var(--border-1)' }}
          >
            <TabBtn active={tab === 'changes'}  onClick={() => setTab('changes')}>Changes</TabBtn>
            <TabBtn active={tab === 'history'}  onClick={() => setTab('history')}>History</TabBtn>
            <TabBtn active={tab === 'branches'} onClick={() => setTab('branches')}>Branches</TabBtn>
            <TabBtn active={tab === 'stash'}    onClick={() => setTab('stash')}>Stash</TabBtn>
            <TabBtn active={tab === 'undo'}     onClick={() => setTab('undo')}>Undo</TabBtn>
          </div>

          {tab === 'changes'  && <ChangesTab />}
          {tab === 'history'  && <HistoryTab />}
          {tab === 'branches' && <BranchManager />}
          {tab === 'stash'    && <StashManager />}
          {tab === 'undo'     && <CheckpointViewer />}
        </>
      )}
    </div>
  )
}

// ─── Changes tab ──────────────────────────────────────────────────────────────

function ChangesTab() {
  const status       = useGitStore((s) => s.status)
  const stage        = useGitStore((s) => s.stage)
  const unstage      = useGitStore((s) => s.unstage)
  const discard      = useGitStore((s) => s.discard)
  const commit       = useGitStore((s) => s.commit)
  const selectDiff   = useGitStore((s) => s.selectDiff)
  const selectedDiff = useGitStore((s) => s.selectedDiff)
  const clearDiff    = useGitStore((s) => s.clearDiff)

  const [message, setMessage] = useState('')
  const [trailer, setTrailer] = useState('Arcana/PA')
  const [committing, setCommitting] = useState(false)

  if (!status) return null

  const hasStaged = status.staged.length > 0

  const onCommit = async () => {
    if (!message.trim() || !hasStaged) return
    setCommitting(true)
    try {
      await commit(message.trim(), trailer.trim() || undefined)
      setMessage('')
    } finally {
      setCommitting(false)
    }
  }

  const stageAll = () => {
    const paths = [
      ...status.unstaged.map((e) => e.path),
      ...status.untracked.map((e) => e.path),
    ]
    if (paths.length > 0) void stage(paths)
  }

  const unstageAll = () => {
    const paths = status.staged.map((e) => e.path)
    if (paths.length > 0) void unstage(paths)
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* File lists */}
      <div className="flex-1 overflow-y-auto">
        <ChangesSection
          label={`Staged (${status.staged.length})`}
          entries={status.staged}
          actionLabel="Unstage all"
          onBulkAction={unstageAll}
          renderRow={(e) => (
            <ChangeRow
              key={`s-${e.path}`}
              entry={e}
              onDiff={() => void selectDiff(e.path, true)}
              primary={{ icon: <Minus size={10} />, title: 'Unstage', onClick: () => void unstage([e.path]) }}
            />
          )}
        />
        <ChangesSection
          label={`Changes (${status.unstaged.length})`}
          entries={status.unstaged}
          actionLabel="Stage all"
          onBulkAction={stageAll}
          renderRow={(e) => (
            <ChangeRow
              key={`u-${e.path}`}
              entry={e}
              onDiff={() => void selectDiff(e.path, false)}
              primary={{ icon: <Plus size={10} />, title: 'Stage', onClick: () => void stage([e.path]) }}
              secondary={{ icon: <Trash2 size={10} />, title: 'Discard', onClick: () => void discard([e.path]) }}
            />
          )}
        />
        <ChangesSection
          label={`Untracked (${status.untracked.length})`}
          entries={status.untracked}
          actionLabel={null}
          onBulkAction={() => {}}
          renderRow={(e) => (
            <ChangeRow
              key={`t-${e.path}`}
              entry={e}
              onDiff={null}
              primary={{ icon: <Plus size={10} />, title: 'Stage', onClick: () => void stage([e.path]) }}
            />
          )}
        />
      </div>

      {/* Diff preview */}
      {selectedDiff && (
        <div
          className="border-t flex flex-col"
          style={{ borderColor: 'var(--border-1)', height: 240 }}
        >
          <div
            className="flex items-center gap-1 px-2 py-1 text-[10px] font-mono"
            style={{ color: 'var(--text-2)' }}
          >
            <FileDiff size={10} />
            <span className="truncate">{selectedDiff.path}</span>
            <span className="opacity-60">({selectedDiff.staged ? 'staged' : 'unstaged'})</span>
            <button className="ml-auto" onClick={clearDiff} aria-label="Close diff">
              <X size={10} style={{ color: 'var(--text-3)' }} />
            </button>
          </div>
          {selectedDiff.loading && (
            <div className="flex-1 flex items-center justify-center text-[11px] font-mono" style={{ color: 'var(--text-3)' }}>
              loading…
            </div>
          )}
          {selectedDiff.error && (
            <div className="flex-1 flex items-center justify-center text-[11px] font-mono p-4" style={{ color: 'var(--danger)' }}>
              {selectedDiff.error}
            </div>
          )}
          {selectedDiff.result && (
            <DiffViewer
              patch={selectedDiff.result.patch}
              filesChanged={selectedDiff.result.files_changed}
              insertions={selectedDiff.result.insertions}
              deletions={selectedDiff.result.deletions}
            />
          )}
        </div>
      )}

      {/* Commit bar */}
      <div
        className="border-t p-2 flex flex-col gap-1"
        style={{ borderColor: 'var(--border-1)' }}
      >
        <textarea
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          placeholder="commit message"
          rows={2}
          className="w-full text-[11px] font-mono rounded px-1.5 py-1 outline-none resize-none"
          style={{
            background:  'var(--bg-input)',
            color:       'var(--text-1)',
            borderColor: 'var(--border-1)',
          }}
        />
        <div className="flex items-center gap-1">
          <span className="text-[10px] font-mono" style={{ color: 'var(--text-3)' }}>
            trailer:
          </span>
          <input
            value={trailer}
            onChange={(e) => setTrailer(e.target.value)}
            placeholder="Commit trailer (Generated-by)"
            title="Commit attribution trailer appended as `Generated-by:`"
            aria-label="Commit trailer"
            className="flex-1 text-[10px] font-mono bg-transparent outline-none"
            style={{ color: 'var(--text-2)' }}
          />
          <button
            onClick={() => void onCommit()}
            disabled={!message.trim() || !hasStaged || committing}
            className="text-[11px] font-mono px-2 py-1 rounded border flex items-center gap-1"
            style={{
              borderColor: hasStaged ? 'var(--accent)' : 'var(--border-1)',
              color:       hasStaged ? 'var(--accent)' : 'var(--text-3)',
              opacity:     hasStaged && message.trim() ? 1 : 0.5,
            }}
          >
            <GitCommit size={11} />
            Commit {status.staged.length}
          </button>
        </div>
      </div>
    </div>
  )
}

interface ChangesSectionProps {
  label: string
  entries: StatusEntry[]
  actionLabel: string | null
  onBulkAction: () => void
  renderRow: (e: StatusEntry) => React.ReactNode
}

function ChangesSection({ label, entries, actionLabel, onBulkAction, renderRow }: ChangesSectionProps) {
  if (entries.length === 0) return null
  return (
    <div>
      <div
        className="flex items-center px-2 py-1 text-[10px] font-mono uppercase tracking-wider"
        style={{ color: 'var(--text-3)', background: 'var(--bg-2)' }}
      >
        <span>{label}</span>
        {actionLabel && (
          <button
            onClick={onBulkAction}
            className="ml-auto text-[9px] normal-case"
            style={{ color: 'var(--text-2)' }}
          >
            {actionLabel}
          </button>
        )}
      </div>
      {entries.map(renderRow)}
    </div>
  )
}

interface ChangeRowProps {
  entry:     StatusEntry
  onDiff:    (() => void) | null
  primary:   { icon: React.ReactNode; title: string; onClick: () => void }
  secondary?: { icon: React.ReactNode; title: string; onClick: () => void }
}

function ChangeRow({ entry, onDiff, primary, secondary }: ChangeRowProps) {
  return (
    <div
      className="flex items-center gap-1.5 px-2 py-0.5 text-[11px] font-mono hover:bg-[var(--bg-hover)] group"
      style={{ color: 'var(--text-2)' }}
    >
      <span
        className="text-[9px] font-bold w-3 text-center"
        style={{ color: statusColor(entry.status) }}
      >
        {statusLetter(entry.status)}
      </span>
      {onDiff ? (
        <button
          className="flex-1 text-left truncate"
          onClick={onDiff}
          title={entry.path}
        >
          {entry.path}
        </button>
      ) : (
        <span className="flex-1 truncate" title={entry.path}>{entry.path}</span>
      )}
      <button
        onClick={primary.onClick}
        className="opacity-0 group-hover:opacity-100"
        title={primary.title}
        aria-label={primary.title}
      >
        {primary.icon}
      </button>
      {secondary && (
        <button
          onClick={secondary.onClick}
          className="opacity-0 group-hover:opacity-100"
          title={secondary.title}
          aria-label={secondary.title}
        >
          {secondary.icon}
        </button>
      )}
    </div>
  )
}

function statusLetter(s: StatusEntry['status']): string {
  switch (s) {
    case 'index_new':         return 'A'
    case 'index_modified':    return 'M'
    case 'index_deleted':     return 'D'
    case 'index_renamed':     return 'R'
    case 'worktree_new':      return '?'
    case 'worktree_modified': return 'M'
    case 'worktree_deleted':  return 'D'
    case 'conflicted':        return '!'
    case 'ignored':           return 'I'
    case 'clean':             return ' '
  }
}

function statusColor(s: StatusEntry['status']): string {
  switch (s) {
    case 'index_new':
    case 'index_modified':
    case 'index_renamed':     return 'var(--success)'
    case 'index_deleted':
    case 'worktree_deleted':  return 'var(--danger)'
    case 'worktree_new':      return 'var(--aqua)'
    case 'worktree_modified': return 'var(--warning)'
    case 'conflicted':        return 'var(--danger)'
    default:                  return 'var(--text-3)'
  }
}

// ─── History tab ──────────────────────────────────────────────────────────────

function HistoryTab() {
  const log = useGitStore((s) => s.log)

  if (log.length === 0) {
    return (
      <div
        className="flex-1 flex items-center justify-center text-[11px] font-mono"
        style={{ color: 'var(--text-3)' }}
      >
        no commits
      </div>
    )
  }

  return (
    <div className="flex-1 overflow-y-auto">
      {log.map((c) => (
        <div
          key={c.oid}
          className="px-2 py-1.5 border-b hover:bg-[var(--bg-hover)]"
          style={{ borderColor: 'var(--border-1)' }}
        >
          <div className="flex items-center gap-1.5 text-[11px] font-mono" style={{ color: 'var(--text-1)' }}>
            <History size={10} style={{ color: 'var(--text-3)' }} />
            <span className="truncate flex-1" title={c.summary}>{c.summary}</span>
            <span className="text-[10px]" style={{ color: 'var(--text-3)' }}>
              {c.short_oid}
            </span>
          </div>
          <div className="text-[10px] mt-0.5" style={{ color: 'var(--text-3)' }}>
            {c.author_name} · {tryRelativeTime(c.time)}
          </div>
        </div>
      ))}
    </div>
  )
}

function tryRelativeTime(iso: string): string {
  try {
    const d = new Date(iso)
    if (Number.isNaN(d.getTime())) return iso
    return formatDistanceToNow(d, { addSuffix: true })
  } catch {
    return iso
  }
}

// ─── Small helpers ────────────────────────────────────────────────────────────

function IconBtn({ onClick, title, children }: { onClick: () => void; title: string; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      className="p-1 rounded hover:bg-[var(--bg-hover)]"
      title={title}
      aria-label={title}
    >
      <span style={{ color: 'var(--text-3)' }}>{children}</span>
    </button>
  )
}

function TabBtn({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      className="flex-1 py-1.5 tracking-wider border-b-2"
      style={{
        color: active ? 'var(--text-1)' : 'var(--text-3)',
        borderColor: active ? 'var(--accent)' : 'transparent',
      }}
    >
      {children}
    </button>
  )
}
