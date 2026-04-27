// SkillsPalette — Command palette for browsing and executing Skills.
// Opens via Cmd/Ctrl+Shift+S or the / button in chat input.
// Phase 3.1

import { useEffect, useRef, useState } from 'react'
import { Search, X, Clock, Zap, Bug, Server, Users, Plus, ChevronRight } from 'lucide-react'
import { useSkillStore, type Skill, type SkillCategory, type SkillSummary } from '../../stores/skillStore'
import SkillParameterModal from './SkillParameterModal'
import SkillCreatorModal from './SkillCreatorModal'

const CATEGORY_META: Record<SkillCategory, { label: string; icon: React.ReactNode }> = {
  coding:    { label: 'Coding',    icon: <Zap size={13} /> },
  debugging: { label: 'Debugging', icon: <Bug size={13} /> },
  devops:    { label: 'DevOps',    icon: <Server size={13} /> },
  fleet:     { label: 'Fleet',     icon: <Users size={13} /> },
  custom:    { label: 'Custom',    icon: <Plus size={13} /> },
}

interface Props {
  onInsert?: (rendered: string) => void
}

export default function SkillsPalette({ onInsert }: Props) {
  const store    = useSkillStore()
  const inputRef = useRef<HTMLInputElement>(null)

  const [query,        setQuery]        = useState('')
  const [activeCategory, setCategory]  = useState<SkillCategory | 'recent' | null>(null)
  const [searchResults,  setResults]   = useState<SkillSummary[] | null>(null)
  const [selectedSkill,  setSelected]  = useState<Skill | null>(null)
  const [showCreator,    setShowCreator] = useState(false)

  // Load skills on open
  useEffect(() => {
    if (store.paletteOpen) {
      store.loadSkills()
      inputRef.current?.focus()
      setQuery('')
      setCategory(null)
      setResults(null)
      setSelected(null)
    }
  }, [store.paletteOpen])

  // Keyboard shortcut: Cmd/Ctrl+Shift+S
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'S') {
        e.preventDefault()
        store.paletteOpen ? store.closePalette() : store.openPalette()
      }
      if (e.key === 'Escape' && store.paletteOpen) {
        store.closePalette()
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [store.paletteOpen])

  if (!store.paletteOpen) return null

  async function handleSearch(q: string) {
    setQuery(q)
    if (q.trim().length < 1) { setResults(null); return }
    const res = await store.searchSkills(q)
    setResults(res)
  }

  async function handleSelectSummary(summary: SkillSummary) {
    const skill = await store.getSkill(summary.name)
    setSelected(skill)
  }

  async function handleExecute(params: Record<string, unknown>) {
    if (!selectedSkill) return
    const result = await store.executeSkill(selectedSkill.name, params)
    onInsert?.(result.rendered)
    store.closePalette()
  }

  // Determine displayed list
  const displayList: SkillSummary[] = (() => {
    if (searchResults) return searchResults
    if (activeCategory === 'recent') {
      return store.recentSkills
        .map(name => {
          for (const skills of Object.values(store.byCategory)) {
            const found = skills.find(s => s.name === name)
            if (found) return found
          }
          return null
        })
        .filter(Boolean) as SkillSummary[]
    }
    if (activeCategory) return store.byCategory[activeCategory] ?? []
    // Default: flatten all
    return Object.values(store.byCategory).flat()
  })()

  return (
    <>
      <div
        className="fixed inset-0 z-40"
        style={{ background: 'rgba(0,0,0,0.5)' }}
        onClick={store.closePalette}
      />

      <div
        className="fixed z-50 top-16 left-1/2 -translate-x-1/2 w-full max-w-2xl rounded-xl shadow-2xl flex flex-col overflow-hidden"
        style={{ background: 'var(--bg-panel)', border: '1px solid var(--border)', maxHeight: '75vh' }}
      >
        {/* Search bar */}
        <div
          className="flex items-center gap-3 px-4 py-3"
          style={{ borderBottom: '1px solid var(--border)' }}
        >
          <Search size={15} style={{ color: 'var(--text-3)', flexShrink: 0 }} />
          <input
            ref={inputRef}
            value={query}
            onChange={e => handleSearch(e.target.value)}
            placeholder="Search skills… (e.g. feature, debug, fleet) — Cmd/Ctrl+Shift+S"
            className="flex-1 bg-transparent outline-none font-mono text-sm"
            style={{ color: 'var(--text-1)' }}
          />
          <button title="Create new skill" onClick={() => setShowCreator(true)} className="p-1 hover:opacity-70">
            <Plus size={14} style={{ color: 'var(--accent)' }} />
          </button>
          <kbd
            className="text-xs px-1.5 py-0.5 rounded font-mono"
            style={{ background: 'var(--bg-2)', color: 'var(--text-3)', border: '1px solid var(--border)' }}
          >
            ESC
          </kbd>
          <button title="Close skills palette" onClick={store.closePalette} className="p-1 hover:opacity-70">
            <X size={14} style={{ color: 'var(--text-3)' }} />
          </button>
        </div>

        <div className="flex flex-1 overflow-hidden">
          {/* Category sidebar */}
          <div
            className="w-36 py-3 shrink-0 overflow-y-auto"
            style={{ borderRight: '1px solid var(--border)' }}
          >
            {store.recentSkills.length > 0 && (
              <CategoryBtn
                label="Recent"
                icon={<Clock size={13} />}
                active={activeCategory === 'recent'}
                onClick={() => { setCategory('recent'); setResults(null); setQuery('') }}
              />
            )}
            {(Object.keys(CATEGORY_META) as SkillCategory[]).map(cat => (
              <CategoryBtn
                key={cat}
                label={CATEGORY_META[cat].label}
                icon={CATEGORY_META[cat].icon}
                active={activeCategory === cat}
                onClick={() => { setCategory(cat); setResults(null); setQuery('') }}
                count={(store.byCategory[cat] ?? []).length}
              />
            ))}
          </div>

          {/* Skill list */}
          <div className="flex-1 overflow-y-auto py-2">
            {store.isLoading ? (
              <div className="px-4 py-6 text-xs text-center" style={{ color: 'var(--text-3)' }}>
                Loading skills…
              </div>
            ) : displayList.length === 0 ? (
              <div className="px-4 py-6 text-xs text-center" style={{ color: 'var(--text-3)' }}>
                {query ? `No skills match "${query}"` : 'No skills in this category'}
              </div>
            ) : (
              displayList.map(skill => (
                <SkillRow
                  key={skill.name}
                  skill={skill}
                  onClick={() => handleSelectSummary(skill)}
                />
              ))
            )}
          </div>
        </div>

        {/* Footer hint */}
        <div
          className="px-4 py-2 text-xs flex items-center gap-4"
          style={{ borderTop: '1px solid var(--border)', color: 'var(--text-3)' }}
        >
          <span><kbd className="font-mono">↑↓</kbd> Navigate</span>
          <span><kbd className="font-mono">↵</kbd> Select</span>
          <span><kbd className="font-mono">⌘⇧S</kbd> Toggle</span>
          <span className="ml-auto">{Object.values(store.byCategory).flat().length} skills loaded</span>
        </div>
      </div>

      {/* Parameter modal */}
      {selectedSkill && (
        <SkillParameterModal
          skill={selectedSkill}
          onExecute={handleExecute}
          onClose={() => setSelected(null)}
        />
      )}

      {/* Skill creator modal */}
      {showCreator && <SkillCreatorModal onClose={() => setShowCreator(false)} />}
    </>
  )
}

// ── Sub-components ─────────────────────────────────────────────────────────────

function CategoryBtn({
  label, icon, active, onClick, count,
}: {
  label:   string
  icon:    React.ReactNode
  active:  boolean
  onClick: () => void
  count?:  number
}) {
  return (
    <button
      onClick={onClick}
      className="w-full flex items-center gap-2 px-3 py-2 text-xs font-mono transition-colors"
      style={{
        background: active ? 'var(--bg-selected)' : 'transparent',
        color:      active ? 'var(--accent)' : 'var(--text-3)',
        textAlign:  'left',
      }}
    >
      {icon}
      <span className="flex-1">{label}</span>
      {count !== undefined && count > 0 && (
        <span style={{ color: 'var(--text-3)', fontSize: '10px' }}>{count}</span>
      )}
    </button>
  )
}

function SkillRow({ skill, onClick }: { skill: SkillSummary; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className="w-full flex items-start gap-3 px-4 py-3 text-left transition-colors hover:opacity-80"
      style={{ borderBottom: '1px solid var(--border)' }}
    >
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="font-mono text-xs font-semibold" style={{ color: 'var(--accent)' }}>
            /{skill.name}
          </span>
          {skill.is_builtin && (
            <span className="text-xs px-1.5 rounded" style={{ background: 'var(--bg-2)', color: 'var(--text-3)' }}>
              builtin
            </span>
          )}
        </div>
        <div className="text-xs mt-0.5 truncate" style={{ color: 'var(--text-2)' }}>
          {skill.description}
        </div>
        {skill.tags.length > 0 && (
          <div className="flex gap-1 mt-1 flex-wrap">
            {skill.tags.slice(0, 4).map(tag => (
              <span key={tag} className="text-xs px-1.5 rounded" style={{ background: 'var(--bg-2)', color: 'var(--text-3)' }}>
                {tag}
              </span>
            ))}
          </div>
        )}
      </div>
      <div className="flex items-center gap-1 shrink-0 mt-1">
        {skill.param_count > 0 && (
          <span className="text-xs" style={{ color: 'var(--text-3)' }}>
            {skill.param_count}p
          </span>
        )}
        <ChevronRight size={12} style={{ color: 'var(--text-3)' }} />
      </div>
    </button>
  )
}
