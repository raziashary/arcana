// Stash manager — list, create, apply, drop stashes.

import { useState } from 'react'
import { Archive, Play, Trash2 } from 'lucide-react'
import { useGitStore } from '../../stores/gitStore'

export default function StashManager() {
  const stashes    = useGitStore((s) => s.stashes)
  const stashPush  = useGitStore((s) => s.stashPush)
  const stashApply = useGitStore((s) => s.stashApply)
  const stashDrop  = useGitStore((s) => s.stashDrop)

  const [msg, setMsg] = useState('')
  const [withUntracked, setWithUntracked] = useState(false)
  const [saving, setSaving] = useState(false)

  const onPush = async () => {
    setSaving(true)
    try {
      await stashPush(msg || undefined, withUntracked)
      setMsg('')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <div
        className="flex flex-col gap-1 px-2 py-1.5 border-b"
        style={{ borderColor: 'var(--border-1)' }}
      >
        <div className="flex items-center gap-1">
          <Archive size={12} style={{ color: 'var(--text-3)' }} />
          <input
            value={msg}
            onChange={(e) => setMsg(e.target.value)}
            placeholder="stash message (optional)"
            className="flex-1 text-[11px] font-mono bg-transparent outline-none"
            style={{ color: 'var(--text-1)' }}
          />
          <button
            onClick={() => void onPush()}
            disabled={saving}
            className="text-[10px] font-mono px-1.5 py-0.5 rounded border"
            style={{ borderColor: 'var(--border-1)', color: 'var(--text-2)' }}
          >
            Stash
          </button>
        </div>
        <label className="flex items-center gap-1 text-[10px] font-mono" style={{ color: 'var(--text-3)' }}>
          <input
            type="checkbox"
            checked={withUntracked}
            onChange={(e) => setWithUntracked(e.target.checked)}
          />
          include untracked
        </label>
      </div>

      <div className="flex-1 overflow-y-auto">
        {stashes.length === 0 && (
          <div
            className="px-3 py-4 text-[11px] font-mono text-center"
            style={{ color: 'var(--text-3)' }}
          >
            no stashes
          </div>
        )}
        {stashes.map((s) => (
          <div
            key={s.oid}
            className="flex items-center gap-1.5 px-2 py-1 text-[11px] font-mono hover:bg-[var(--bg-hover)] group"
            style={{ color: 'var(--text-2)' }}
          >
            <span
              className="text-[10px] px-1 rounded"
              style={{ background: 'var(--bg-2)', color: 'var(--text-3)' }}
            >
              #{s.index}
            </span>
            <span className="flex-1 truncate" title={s.message}>{s.message}</span>
            <button
              onClick={() => void stashApply(s.index, false)}
              className="opacity-0 group-hover:opacity-100"
              title="Apply stash"
              aria-label={`Apply stash ${s.index}`}
            >
              <Play size={10} style={{ color: 'var(--success)' }} />
            </button>
            <button
              onClick={() => void stashApply(s.index, true)}
              className="opacity-0 group-hover:opacity-100 text-[9px]"
              title="Pop (apply + drop)"
              style={{ color: 'var(--accent)' }}
            >
              pop
            </button>
            <button
              onClick={() => void stashDrop(s.index)}
              className="opacity-0 group-hover:opacity-100"
              title="Drop stash"
              aria-label={`Drop stash ${s.index}`}
            >
              <Trash2 size={10} style={{ color: 'var(--danger)' }} />
            </button>
          </div>
        ))}
      </div>
    </div>
  )
}
