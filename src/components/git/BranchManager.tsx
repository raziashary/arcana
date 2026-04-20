// Branch manager — list, switch, create, delete branches.

import { useState } from 'react'
import { Check, GitBranch, Plus, Trash2 } from 'lucide-react'
import { useGitStore } from '../../stores/gitStore'

export default function BranchManager() {
  const branches        = useGitStore((s) => s.branches)
  const checkoutBranch  = useGitStore((s) => s.checkoutBranch)
  const createBranch    = useGitStore((s) => s.createBranch)
  const deleteBranch    = useGitStore((s) => s.deleteBranch)

  const [newName, setNewName] = useState('')
  const [creating, setCreating] = useState(false)

  const local  = branches.filter((b) => !b.is_remote)
  const remote = branches.filter((b) =>  b.is_remote)

  const onCreate = async () => {
    const name = newName.trim()
    if (!name) return
    setCreating(true)
    try {
      await createBranch(name, true)
      setNewName('')
    } finally {
      setCreating(false)
    }
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Create new branch */}
      <div
        className="flex items-center gap-1 px-2 py-1.5 border-b"
        style={{ borderColor: 'var(--border-1)' }}
      >
        <Plus size={12} style={{ color: 'var(--text-3)' }} />
        <input
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Enter') void onCreate() }}
          placeholder="new-branch-name"
          className="flex-1 text-[11px] font-mono bg-transparent outline-none"
          style={{ color: 'var(--text-1)' }}
        />
        <button
          onClick={() => void onCreate()}
          disabled={creating || !newName.trim()}
          className="text-[10px] font-mono px-1.5 py-0.5 rounded border"
          style={{ borderColor: 'var(--border-1)', color: 'var(--text-2)' }}
        >
          Create + Checkout
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        <Section label="Local">
          {local.map((b) => (
            <BranchRow
              key={b.name}
              name={b.name}
              isHead={b.is_head}
              upstream={b.upstream}
              ahead={b.ahead}
              behind={b.behind}
              onCheckout={() => void checkoutBranch(b.name)}
              onDelete={() => void deleteBranch(b.name, false)}
            />
          ))}
        </Section>
        <Section label="Remote">
          {remote.map((b) => (
            <BranchRow
              key={b.name}
              name={b.name}
              isHead={false}
              upstream={null}
              ahead={0}
              behind={0}
              onCheckout={() => void checkoutBranch(b.name.replace(/^[^/]+\//, ''))}
              onDelete={null}
            />
          ))}
        </Section>
      </div>
    </div>
  )
}

function Section({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <div
        className="px-2 py-1 text-[10px] font-mono uppercase tracking-wider"
        style={{ color: 'var(--text-3)', background: 'var(--bg-2)' }}
      >
        {label}
      </div>
      {children}
    </div>
  )
}

interface BranchRowProps {
  name:       string
  isHead:     boolean
  upstream:   string | null
  ahead:      number
  behind:     number
  onCheckout: () => void
  onDelete:   (() => void) | null
}

function BranchRow({ name, isHead, upstream, ahead, behind, onCheckout, onDelete }: BranchRowProps) {
  return (
    <div
      className="flex items-center gap-1.5 px-2 py-1 text-[11px] font-mono hover:bg-[var(--bg-hover)] group"
      style={{ color: isHead ? 'var(--accent)' : 'var(--text-2)' }}
    >
      {isHead
        ? <Check size={11} />
        : <GitBranch size={11} style={{ color: 'var(--text-3)' }} />}
      <button className="flex-1 text-left truncate" onClick={onCheckout} title={name}>
        {name}
      </button>
      {(ahead > 0 || behind > 0) && (
        <span className="text-[10px]" style={{ color: 'var(--text-3)' }}>
          {ahead > 0 && `↑${ahead}`}{behind > 0 && `↓${behind}`}
        </span>
      )}
      {upstream && (
        <span className="text-[10px]" style={{ color: 'var(--text-3)' }} title={`upstream: ${upstream}`}>
          ⇡
        </span>
      )}
      {onDelete && !isHead && (
        <button
          onClick={onDelete}
          className="opacity-0 group-hover:opacity-100 transition-opacity"
          title="Delete branch"
          aria-label={`Delete ${name}`}
        >
          <Trash2 size={10} style={{ color: 'var(--danger)' }} />
        </button>
      )}
    </div>
  )
}
