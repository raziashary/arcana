// Docs tab — Arcana documentation panel.
// Three-column layout: sidebar nav | content | on-page anchors.
// Data: docsAllA (01-04), docsAllB (05-07), docsAllC (08-11), docsAllD (12-14).

import { useState, useRef, useEffect, useCallback } from 'react'
import { Search, BookOpen, ChevronRight, Copy, Check, ArrowLeft, ArrowRight } from 'lucide-react'
import { NAV_CATEGORIES, type DocSection, type Block } from '../data/docs'
import { A_SECTIONS } from '../data/docsAllA'
import { B_SECTIONS } from '../data/docsAllB'
import { C_SECTIONS } from '../data/docsAllC'
import { D_SECTIONS } from '../data/docsAllD'

const ALL_SECTIONS: DocSection[] = [...A_SECTIONS, ...B_SECTIONS, ...C_SECTIONS, ...D_SECTIONS]

// ── Icon map (lucide icon names → emoji/text fallback since we use string ids) ─
const ICON_MAP: Record<string, string> = {
  'info-circle': '○', 'topology-star': '⬡', 'robot': '◈', 'users': '◉',
  'clock-play': '◷', 'wand': '✦', 'webhook': '⇄', 'settings': '⚙',
  'dashboard': '▤', 'code': '⌨', 'chart-dots-3': '⬡', 'cpu': '◻',
  'git-branch': '⑂', 'book-open': '◎', 'alphabet-latin': 'α',
  'list-details': '≡', 'terminal': '>_', 'database': '▬', 'server': '▫',
}

// ── Block renderer ────────────────────────────────────────────────────────────
function CodeBlock({ lang, code }: { lang: string; code: string }) {
  const [copied, setCopied] = useState(false)
  const copy = () => {
    void navigator.clipboard.writeText(code)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }
  return (
    <div style={{ background: 'var(--bg-surface)', border: '1px solid var(--border-1)', borderRadius: 6, margin: '12px 0' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '6px 12px', borderBottom: '1px solid var(--border-1)' }}>
        <span style={{ fontSize: 9, color: 'var(--text-4)', letterSpacing: '0.12em', textTransform: 'uppercase', fontFamily: 'var(--font-mono)' }}>{lang}</span>
        <button onClick={copy} style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 10, color: copied ? 'var(--accent)' : 'var(--text-4)', background: 'none', border: 'none', cursor: 'pointer', fontFamily: 'var(--font-mono)', padding: '2px 4px' }}>
          {copied ? <Check size={11} /> : <Copy size={11} />}{copied ? 'Copied' : 'Copy'}
        </button>
      </div>
      <pre style={{ padding: '12px 14px', fontSize: 11, color: '#7EC8A4', lineHeight: 1.7, overflowX: 'auto', margin: 0, fontFamily: 'var(--font-mono)', whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>{code}</pre>
    </div>
  )
}

function BlockRenderer({ block }: { block: Block }) {
  switch (block.t) {
    case 'h2':
      return <div style={{ fontSize: 14, fontWeight: 500, color: 'var(--text-secondary)', margin: '22px 0 8px', display: 'flex', alignItems: 'center', gap: 8 }}><span style={{ display: 'block', width: 3, height: 14, background: 'var(--accent)', borderRadius: 2, flexShrink: 0 }} />{block.text}</div>
    case 'h3':
      return <div style={{ fontSize: 11, fontWeight: 500, color: 'var(--accent)', textTransform: 'uppercase', letterSpacing: '0.1em', margin: '14px 0 6px', fontFamily: 'var(--font-mono)' }}>{block.text}</div>
    case 'prose':
      return <p style={{ fontSize: 13, color: 'var(--text-muted)', lineHeight: 1.75, marginBottom: 12, fontFamily: 'var(--font-sans)' }}>{block.text}</p>
    case 'callout':
      return (
        <div style={{ background: block.variant === 'warn' ? 'rgba(255,170,0,0.06)' : 'rgba(0,255,136,0.05)', borderLeft: `3px solid ${block.variant === 'warn' ? 'var(--warning)' : 'var(--accent)'}`, borderRadius: '0 6px 6px 0', padding: '8px 12px', margin: '12px 0', fontSize: 12, color: block.variant === 'warn' ? 'var(--warning)' : 'var(--accent)', fontFamily: 'var(--font-sans)', lineHeight: 1.6 }}>
          {block.text}
        </div>
      )
    case 'list':
      return (
        <ul style={{ margin: '8px 0 12px 0', paddingLeft: 20 }}>
          {block.items.map((item, i) => (
            <li key={i} style={{ fontSize: 13, color: 'var(--text-muted)', lineHeight: 1.7, marginBottom: 4, fontFamily: 'var(--font-sans)' }}>{item}</li>
          ))}
        </ul>
      )
    case 'table':
      return (
        <div style={{ overflowX: 'auto', margin: '12px 0' }}>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12, fontFamily: 'var(--font-sans)' }}>
            <thead>
              <tr>{block.headers.map((h, i) => <th key={i} style={{ textAlign: 'left', padding: '6px 10px', fontSize: 10, letterSpacing: '0.1em', textTransform: 'uppercase', color: 'var(--text-4)', borderBottom: '1px solid var(--border-1)', fontWeight: 500 }}>{h}</th>)}</tr>
            </thead>
            <tbody>
              {block.rows.map((row, ri) => (
                <tr key={ri} style={{ borderBottom: '1px solid var(--border-1)' }}>
                  {row.map((cell, ci) => <td key={ci} style={{ padding: '6px 10px', color: 'var(--text-muted)' }}><code style={{ background: ci === 0 ? 'var(--bg-card)' : 'transparent', padding: ci === 0 ? '1px 5px' : 0, borderRadius: 3, fontFamily: 'var(--font-mono)', fontSize: 11, color: ci === 0 ? '#7EC8A4' : 'var(--text-muted)' }}>{cell}</code></td>)}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )
    case 'code':
      return <CodeBlock lang={block.lang} code={block.code} />
    default:
      return null
  }
}

// ── Main component ────────────────────────────────────────────────────────────
export default function Docs() {
  const [activeId,  setActiveId]  = useState('what')
  const [query,     setQuery]     = useState('')
  const [showResults, setShowResults] = useState(false)
  const searchRef = useRef<HTMLInputElement>(null)

  const section = ALL_SECTIONS.find(s => s.id === activeId) ?? ALL_SECTIONS[0]
  const idx     = ALL_SECTIONS.findIndex(s => s.id === activeId)
  const prev    = idx > 0                        ? ALL_SECTIONS[idx - 1] : null
  const next    = idx < ALL_SECTIONS.length - 1  ? ALL_SECTIONS[idx + 1] : null

  const searchResults = query.length > 1
    ? ALL_SECTIONS.filter(s =>
        s.title.toLowerCase().includes(query.toLowerCase()) ||
        s.lead.toLowerCase().includes(query.toLowerCase()) ||
        s.anchors.some(a => a.toLowerCase().includes(query.toLowerCase()))
      ).slice(0, 6)
    : []

  const navTo = useCallback((id: string) => {
    setActiveId(id)
    setQuery('')
    setShowResults(false)
  }, [])

  // Ctrl/Cmd+K to focus search
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault()
        searchRef.current?.focus()
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [])

  return (
    <div className="flex-1 flex flex-col overflow-hidden" style={{ background: 'var(--bg-app)', fontFamily: 'var(--font-mono)' }}>

      {/* ── Top bar ──────────────────────────────────────────────────────── */}
      <div style={{ background: 'var(--bg-surface)', borderBottom: '1px solid var(--border-1)', padding: '0 14px', height: 42, display: 'flex', alignItems: 'center', gap: 10, flexShrink: 0 }}>
        <BookOpen size={14} style={{ color: 'var(--accent)', flexShrink: 0 }} />
        <span style={{ fontSize: 11, letterSpacing: '0.12em', color: 'var(--accent)', textTransform: 'uppercase', fontWeight: 500, flexShrink: 0 }}>Docs</span>

        {/* Search */}
        <div style={{ flex: 1, position: 'relative', maxWidth: 360, marginLeft: 'auto' }}>
          <Search size={12} style={{ position: 'absolute', left: 9, top: '50%', transform: 'translateY(-50%)', color: 'var(--text-4)' }} />
          <input
            ref={searchRef}
            value={query}
            onChange={e => { setQuery(e.target.value); setShowResults(true) }}
            onFocus={() => setShowResults(true)}
            onBlur={() => setTimeout(() => setShowResults(false), 150)}
            placeholder="Search documentation…  ⌘K"
            style={{ width: '100%', background: 'var(--bg-card)', border: '1px solid var(--border-1)', borderRadius: 6, color: 'var(--text-primary)', fontSize: 12, fontFamily: 'var(--font-mono)', padding: '5px 10px 5px 28px', outline: 'none', boxSizing: 'border-box' }}
          />
          {showResults && searchResults.length > 0 && (
            <div style={{ position: 'absolute', top: 34, left: 0, right: 0, background: 'var(--bg-card)', border: '1px solid var(--accent)', borderRadius: 6, zIndex: 99, padding: '4px 0', boxShadow: '0 8px 24px rgba(0,0,0,0.5)' }}>
              {searchResults.map(s => (
                <div key={s.id} onMouseDown={() => navTo(s.id)} style={{ padding: '7px 12px', fontSize: 12, color: 'var(--text-muted)', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 8, fontFamily: 'var(--font-sans)' }}
                  onMouseEnter={e => (e.currentTarget.style.background = 'rgba(0,255,136,0.06)')}
                  onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}>
                  <span style={{ color: 'var(--accent)', fontFamily: 'var(--font-mono)' }}>{ICON_MAP[s.icon] ?? '○'}</span>
                  <span>{s.num} — {s.title}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* ── Body ─────────────────────────────────────────────────────────── */}
      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>

        {/* Sidebar */}
        <div style={{ width: 212, flexShrink: 0, background: 'var(--bg-surface)', borderRight: '1px solid var(--border-1)', overflowY: 'auto', padding: '10px 0' }}>
          {NAV_CATEGORIES.map(cat => (
            <div key={cat.label}>
              <div style={{ fontSize: 9, letterSpacing: '0.14em', color: 'var(--text-4)', textTransform: 'uppercase', padding: '8px 14px 4px', fontWeight: 500 }}>{cat.label}</div>
              {cat.ids.map(id => {
                const s = ALL_SECTIONS.find(x => x.id === id)
                if (!s) return null
                const active = id === activeId
                return (
                  <div key={id} onClick={() => navTo(id)}
                    style={{ display: 'flex', alignItems: 'center', gap: 7, padding: '5px 14px', fontSize: 12, color: active ? 'var(--accent)' : 'var(--text-muted)', cursor: 'pointer', borderLeft: `2px solid ${active ? 'var(--accent)' : 'transparent'}`, background: active ? 'rgba(0,255,136,0.04)' : 'transparent', transition: 'all 0.1s' }}
                    onMouseEnter={e => { if (!active) { e.currentTarget.style.color = 'var(--text-secondary)'; e.currentTarget.style.background = 'var(--bg-card)' }}}
                    onMouseLeave={e => { if (!active) { e.currentTarget.style.color = 'var(--text-muted)'; e.currentTarget.style.background = 'transparent' }}}>
                    <span style={{ fontSize: 9, color: active ? 'var(--accent)' : 'var(--text-4)', minWidth: 18, fontFamily: 'var(--font-mono)' }}>{s.num}</span>
                    <span style={{ fontFamily: 'var(--font-mono)' }}>{ICON_MAP[s.icon] ?? '○'}</span>
                    <span>{s.title}</span>
                  </div>
                )
              })}
            </div>
          ))}
        </div>

        {/* Main content */}
        <div style={{ flex: 1, overflowY: 'auto', padding: '28px 36px 40px', background: 'var(--bg-app)' }}>
          {/* Breadcrumb */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, color: 'var(--text-4)', marginBottom: 20, fontFamily: 'var(--font-sans)' }}>
            <span>Docs</span>
            <ChevronRight size={10} />
            <span>{section.category}</span>
            <ChevronRight size={10} />
            <span style={{ color: 'var(--text-muted)' }}>{section.title}</span>
          </div>

          {/* Badge */}
          <div style={{ display: 'inline-flex', alignItems: 'center', gap: 5, background: 'rgba(0,255,136,0.06)', border: '1px solid rgba(0,255,136,0.12)', borderRadius: 4, padding: '3px 9px', fontSize: 10, color: 'var(--accent)', marginBottom: 12, letterSpacing: '0.08em', fontFamily: 'var(--font-mono)' }}>
            {ICON_MAP[section.icon] ?? '○'} Section {section.num}
          </div>

          {/* Title */}
          <h1 style={{ fontSize: 22, fontWeight: 500, color: 'var(--text-bright)', margin: '0 0 6px', letterSpacing: '-0.02em', fontFamily: 'var(--font-sans)' }}>{section.title}</h1>

          {/* Lead */}
          <p style={{ fontSize: 13, color: 'var(--text-muted)', marginBottom: 24, lineHeight: 1.7, fontFamily: 'var(--font-sans)', borderBottom: '1px solid var(--border-1)', paddingBottom: 20 }}>{section.lead}</p>

          {/* Blocks */}
          {section.blocks.map((block, i) => <BlockRenderer key={i} block={block} />)}
        </div>

        {/* On-page anchors */}
        <div style={{ width: 160, flexShrink: 0, borderLeft: '1px solid var(--border-1)', padding: '16px 12px', background: 'var(--bg-surface)', overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: 2 }}>
          <div style={{ fontSize: 9, letterSpacing: '0.14em', color: 'var(--text-4)', textTransform: 'uppercase', marginBottom: 8, fontWeight: 500, fontFamily: 'var(--font-mono)' }}>On this page</div>
          {section.anchors.map((a, i) => (
            <div key={i} style={{ fontSize: 11, color: i === 0 ? 'var(--accent)' : 'var(--text-4)', cursor: 'pointer', padding: '3px 0 3px 10px', borderLeft: `1px solid ${i === 0 ? 'var(--accent)' : 'transparent'}`, transition: 'all 0.1s', fontFamily: 'var(--font-sans)' }}
              onMouseEnter={e => { e.currentTarget.style.color = 'var(--text-secondary)'; e.currentTarget.style.borderLeftColor = 'var(--border-2)' }}
              onMouseLeave={e => { e.currentTarget.style.color = i === 0 ? 'var(--accent)' : 'var(--text-4)'; e.currentTarget.style.borderLeftColor = i === 0 ? 'var(--accent)' : 'transparent' }}>
              {a}
            </div>
          ))}
        </div>
      </div>

      {/* ── Footer ───────────────────────────────────────────────────────── */}
      <div style={{ background: 'var(--bg-surface)', borderTop: '1px solid var(--border-1)', padding: '0 16px', height: 36, display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexShrink: 0 }}>
        <span style={{ fontSize: 10, color: 'var(--text-4)', letterSpacing: '0.1em', fontFamily: 'var(--font-mono)' }}>ARCANA DOCS</span>
        <div style={{ display: 'flex', gap: 4 }}>
          {prev && (
            <button onClick={() => navTo(prev.id)} style={{ display: 'flex', alignItems: 'center', gap: 5, fontSize: 11, color: 'var(--text-4)', cursor: 'pointer', padding: '4px 10px', borderRadius: 4, background: 'none', border: 'none', fontFamily: 'var(--font-mono)', transition: 'all 0.1s' }}
              onMouseEnter={e => { e.currentTarget.style.color = 'var(--accent)'; e.currentTarget.style.background = 'rgba(0,255,136,0.06)' }}
              onMouseLeave={e => { e.currentTarget.style.color = 'var(--text-4)'; e.currentTarget.style.background = 'none' }}>
              <ArrowLeft size={12} /> {prev.title}
            </button>
          )}
          {next && (
            <button onClick={() => navTo(next.id)} style={{ display: 'flex', alignItems: 'center', gap: 5, fontSize: 11, color: 'var(--text-4)', cursor: 'pointer', padding: '4px 10px', borderRadius: 4, background: 'none', border: 'none', fontFamily: 'var(--font-mono)', transition: 'all 0.1s' }}
              onMouseEnter={e => { e.currentTarget.style.color = 'var(--accent)'; e.currentTarget.style.background = 'rgba(0,255,136,0.06)' }}
              onMouseLeave={e => { e.currentTarget.style.color = 'var(--text-4)'; e.currentTarget.style.background = 'none' }}>
              {next.title} <ArrowRight size={12} />
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
