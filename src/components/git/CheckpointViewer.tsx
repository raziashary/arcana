// Checkpoint viewer — timeline of auto-snapshots with restore/delete.

import { useEffect, useMemo, useState } from 'react'
import { Clock, History, RotateCcw, Trash2 } from 'lucide-react'
import { useGitStore, type CheckpointInfo } from '../../stores/gitStore'
import { formatDistanceToNow } from 'date-fns'

interface Props {
  /// When provided, restricts the timeline to a single file path.
  filePath?: string
}

export default function CheckpointViewer({ filePath }: Props) {
  const checkpoints        = useGitStore((s) => s.checkpoints)
  const refreshCheckpoints = useGitStore((s) => s.refreshCheckpoints)
  const restoreCheckpoint  = useGitStore((s) => s.restoreCheckpoint)
  const deleteCheckpoint   = useGitStore((s) => s.deleteCheckpoint)

  useEffect(() => {
    void refreshCheckpoints(filePath)
  }, [filePath])

  // Group by file for a tidy display.
  const groups = useMemo(() => {
    const g = new Map<string, CheckpointInfo[]>()
    for (const c of checkpoints) {
      const arr = g.get(c.file_path) ?? []
      arr.push(c)
      g.set(c.file_path, arr)
    }
    return Array.from(g.entries())
  }, [checkpoints])

  const [confirmId, setConfirmId] = useState<string | null>(null)

  if (checkpoints.length === 0) {
    return (
      <div
        className="flex-1 flex items-center justify-center p-4 text-[11px] font-mono text-center"
        style={{ color: 'var(--text-3)' }}
      >
        no checkpoints yet — they are created automatically before every agent edit
      </div>
    )
  }

  return (
    <div className="flex-1 overflow-y-auto">
      {groups.map(([file, list]) => (
        <div key={file}>
          <div
            className="sticky top-0 px-2 py-1 text-[10px] font-mono"
            style={{ color: 'var(--text-2)', background: 'var(--bg-2)' }}
            title={file}
          >
            <History size={10} className="inline mr-1" style={{ color: 'var(--text-3)' }} />
            {file}
          </div>
          {list.map((c) => (
            <div
              key={c.id}
              className="flex items-center gap-1.5 px-3 py-1 text-[11px] font-mono hover:bg-[var(--bg-hover)] group"
              style={{ color: 'var(--text-2)' }}
            >
              <Clock size={10} style={{ color: 'var(--text-3)' }} />
              <span className="flex-1 truncate" title={new Date(c.created_ms).toISOString()}>
                {formatDistanceToNow(c.created_ms, { addSuffix: true })} — {c.reason}
              </span>
              <span className="text-[10px]" style={{ color: 'var(--text-3)' }}>
                {formatBytes(c.size_bytes)}
              </span>
              {confirmId === c.id ? (
                <>
                  <button
                    onClick={async () => { await restoreCheckpoint(c.id); setConfirmId(null) }}
                    className="text-[10px] px-1.5 py-0.5 rounded border"
                    style={{ borderColor: 'var(--accent)', color: 'var(--accent)' }}
                  >
                    Confirm restore
                  </button>
                  <button
                    onClick={() => setConfirmId(null)}
                    className="text-[10px] px-1 py-0.5"
                    style={{ color: 'var(--text-3)' }}
                  >
                    cancel
                  </button>
                </>
              ) : (
                <>
                  <button
                    onClick={() => setConfirmId(c.id)}
                    className="opacity-0 group-hover:opacity-100"
                    title="Restore this snapshot"
                    aria-label="Restore checkpoint"
                  >
                    <RotateCcw size={11} style={{ color: 'var(--accent)' }} />
                  </button>
                  <button
                    onClick={() => void deleteCheckpoint(c.id)}
                    className="opacity-0 group-hover:opacity-100"
                    title="Delete checkpoint"
                    aria-label="Delete checkpoint"
                  >
                    <Trash2 size={10} style={{ color: 'var(--danger)' }} />
                  </button>
                </>
              )}
            </div>
          ))}
        </div>
      ))}
    </div>
  )
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n}B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)}K`
  return `${(n / (1024 * 1024)).toFixed(1)}M`
}
