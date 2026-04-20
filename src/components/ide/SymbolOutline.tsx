// Symbol outline — shows functions/classes/etc. for the active tab.
// Clicking a symbol jumps Monaco to its declaration line.

import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import {
  Code2,
  Hash,
  Layers,
  Package,
  Type as TypeIcon,
} from 'lucide-react'
import { useIdeStore, type SymbolTree, type SymbolKind } from '../../stores/ideStore'

export default function SymbolOutline() {
  const activeTab = useIdeStore((s) => s.activeTab)
  const openFile  = useIdeStore((s) => s.openFile)

  const [tree,    setTree]    = useState<SymbolTree | null>(null)
  const [loading, setLoading] = useState(false)
  const [filter,  setFilter]  = useState('')

  useEffect(() => {
    if (!activeTab) {
      setTree(null)
      return
    }
    let cancelled = false
    setLoading(true)
    invoke<SymbolTree>('get_symbol_tree', { path: activeTab })
      .then((r) => { if (!cancelled) setTree(r) })
      .catch((e) => { if (!cancelled) console.error('get_symbol_tree', e) })
      .finally(() => { if (!cancelled) setLoading(false) })
    return () => { cancelled = true }
  }, [activeTab])

  if (!activeTab) {
    return (
      <EmptyState>No file selected</EmptyState>
    )
  }

  const filtered = tree?.symbols.filter((s) =>
    filter.trim() === '' ? true : s.name.toLowerCase().includes(filter.toLowerCase())
  ) ?? []

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <div
        className="flex items-center justify-between px-2 py-1.5 border-b"
        style={{ borderColor: 'var(--border-1)' }}
      >
        <span
          className="text-[10px] font-mono uppercase tracking-wider"
          style={{ color: 'var(--text-2)' }}
        >
          Outline
        </span>
        <span className="text-[10px] font-mono" style={{ color: 'var(--text-3)' }}>
          {tree?.language ?? ''}
        </span>
      </div>

      <input
        placeholder="Filter symbols…"
        value={filter}
        onChange={(e) => setFilter(e.target.value)}
        className="px-2 py-1 text-[11px] font-mono outline-none border-b"
        style={{
          background:   'var(--bg-input)',
          color:        'var(--text-1)',
          borderColor:  'var(--border-1)',
        }}
      />

      <div className="flex-1 overflow-y-auto py-1">
        {loading && <EmptyState>Extracting symbols…</EmptyState>}
        {!loading && filtered.length === 0 && <EmptyState>No symbols</EmptyState>}
        {filtered.map((sym, idx) => (
          <button
            key={`${sym.name}-${sym.line}-${idx}`}
            onClick={() => openFile(activeTab, sym.line)}
            className="w-full flex items-center gap-1.5 px-2 py-0.5 text-left hover:bg-[var(--bg-hover)]"
            style={{ color: 'var(--text-1)' }}
          >
            <SymbolGlyph kind={sym.kind} />
            <span className="text-[12px] font-mono truncate">{sym.name}</span>
            <span
              className="text-[10px] font-mono ml-auto"
              style={{ color: 'var(--text-3)' }}
            >
              L{sym.line}
            </span>
          </button>
        ))}
      </div>
    </div>
  )
}

function EmptyState({ children }: { children: React.ReactNode }) {
  return (
    <div
      className="px-3 py-2 text-[11px] font-mono"
      style={{ color: 'var(--text-3)' }}
    >
      {children}
    </div>
  )
}

function SymbolGlyph({ kind }: { kind: SymbolKind }) {
  const color = symbolColor(kind)
  const Icon  = symbolIcon(kind)
  return <Icon size={11} style={{ color }} />
}

function symbolIcon(kind: SymbolKind) {
  switch (kind) {
    case 'function':
    case 'method':    return Code2
    case 'class':
    case 'struct':    return Layers
    case 'enum':
    case 'interface':
    case 'trait':     return TypeIcon
    case 'module':    return Package
    case 'constant':
    case 'variable':
    case 'type':
    default:          return Hash
  }
}

function symbolColor(kind: SymbolKind): string {
  switch (kind) {
    case 'function':
    case 'method':    return 'var(--accent)'
    case 'class':
    case 'struct':    return 'var(--aqua)'
    case 'enum':
    case 'interface':
    case 'trait':     return 'var(--info)'
    case 'module':    return 'var(--success)'
    case 'constant':  return 'var(--gold)'
    default:          return 'var(--text-2)'
  }
}
