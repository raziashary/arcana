// Quick-open overlay — Ctrl+P / Cmd+P style fuzzy file search.
//
// Keyboard:
//   ↑/↓        navigate
//   Enter      open file
//   Escape     close overlay
//
// Debounced at 120ms to avoid hammering the backend while typing.

import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { File as FileIcon, Folder } from 'lucide-react'
import { useIdeStore, type FileSearchResult } from '../../stores/ideStore'

export default function FileSearch() {
  const open         = useIdeStore((s) => s.searchOpen)
  const setOpen      = useIdeStore((s) => s.setSearchOpen)
  const openFile     = useIdeStore((s) => s.openFile)
  const recentFiles  = useIdeStore((s) => s.recentFiles)

  const [query,     setQuery]     = useState('')
  const [results,   setResults]   = useState<FileSearchResult[]>([])
  const [selected,  setSelected]  = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)

  // Focus on open, reset on close.
  useEffect(() => {
    if (open) {
      setQuery('')
      setResults([])
      setSelected(0)
      setTimeout(() => inputRef.current?.focus(), 0)
    }
  }, [open])

  // Debounced search.
  useEffect(() => {
    if (!open) return
    const handle = setTimeout(async () => {
      try {
        const r = await invoke<FileSearchResult[]>('search_files', {
          query,
          path:       null,
          glob:       null,
          maxResults: 40,
        })
        setResults(r)
        setSelected(0)
      } catch (e) {
        console.error('search_files', e)
      }
    }, 120)
    return () => clearTimeout(handle)
  }, [query, open])

  if (!open) return null

  // Empty query → show recent files as a useful default.
  const list: Array<Pick<FileSearchResult, 'name' | 'relative_path' | 'is_directory'>> =
    query.trim() === '' && results.length === 0
      ? recentFiles.map((p) => ({
          name:          p.split('/').pop() ?? p,
          relative_path: p,
          is_directory:  false,
        }))
      : results

  const onKey = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Escape') {
      setOpen(false)
    } else if (e.key === 'ArrowDown') {
      e.preventDefault()
      setSelected((s) => Math.min(s + 1, list.length - 1))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setSelected((s) => Math.max(s - 1, 0))
    } else if (e.key === 'Enter') {
      e.preventDefault()
      const target = list[selected]
      if (target && !target.is_directory) {
        void openFile(target.relative_path)
        setOpen(false)
      }
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center pt-24"
      style={{ background: 'rgba(1, 22, 39, 0.5)' }}
      onClick={() => setOpen(false)}
    >
      <div
        className="w-[520px] max-w-[90vw] rounded overflow-hidden shadow-2xl"
        style={{ background: 'var(--bg-card)', border: '1px solid var(--border-1)' }}
        onClick={(e) => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKey}
          placeholder="Go to file — type to search"
          className="w-full px-3 py-2 text-xs font-mono outline-none"
          style={{
            background: 'var(--bg-input)',
            color:      'var(--text-1)',
            borderBottom: '1px solid var(--border-1)',
          }}
        />

        <div className="max-h-[360px] overflow-y-auto">
          {list.length === 0 && (
            <div className="px-3 py-4 text-[11px] font-mono text-center" style={{ color: 'var(--text-3)' }}>
              {query.trim() === '' ? 'No recent files' : 'No matches'}
            </div>
          )}
          {list.map((item, i) => (
            <button
              key={item.relative_path}
              onClick={() => {
                if (!item.is_directory) {
                  void openFile(item.relative_path)
                  setOpen(false)
                }
              }}
              onMouseEnter={() => setSelected(i)}
              className="w-full flex items-center gap-2 px-3 py-1.5 text-left"
              style={{
                background: selected === i ? 'var(--bg-selected)' : undefined,
                color:      'var(--text-1)',
              }}
            >
              {item.is_directory
                ? <Folder size={12} style={{ color: 'var(--accent)' }} />
                : <FileIcon size={12} style={{ color: 'var(--text-3)' }} />}
              <span className="text-[12px] font-mono truncate">{item.name}</span>
              <span
                className="text-[10px] font-mono ml-auto truncate"
                style={{ color: 'var(--text-3)' }}
              >
                {item.relative_path}
              </span>
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}
