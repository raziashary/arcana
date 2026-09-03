// McpToolBrowser.tsx — Phase 3.4b
//
// Lists every builtin tool AutoDev exposes to its agents (the canonical Rust
// ToolRegistry). Data comes from the `list_mcp_tools` Tauri command, which
// iterates the in-process ToolRegistry — NOT a JSON-RPC round-trip through
// the MCP server (which would require Streamable HTTP session management).
//
// Each tool row is collapsible; expanding reveals the JSON input schema
// so developers can see the exact shape the fleet agents receive.

import { useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { ChevronDown, ChevronRight, RefreshCw, Wrench, Search } from 'lucide-react'

interface McpToolInfo {
  name:         string
  description:  string
  input_schema: unknown
}

export default function McpToolBrowser() {
  const [tools,     setTools]     = useState<McpToolInfo[]>([])
  const [loading,   setLoading]   = useState(false)
  const [error,     setError]     = useState<string | null>(null)
  const [expanded,  setExpanded]  = useState<Set<string>>(new Set())
  const [query,     setQuery]     = useState('')

  async function load() {
    setLoading(true)
    try {
      const result = await invoke<McpToolInfo[]>('list_mcp_tools')
      setTools(result)
      setError(null)
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => { void load() }, [])

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return tools
    return tools.filter((t) =>
      t.name.toLowerCase().includes(q) ||
      t.description.toLowerCase().includes(q)
    )
  }, [tools, query])

  function toggle(name: string) {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(name)) next.delete(name)
      else next.add(name)
      return next
    })
  }

  return (
    <div
      className="rounded-sm p-4 space-y-3"
      style={{ background: 'var(--bg-card)', border: '1px solid var(--border-1)' }}
    >
      <div className="flex items-center justify-between gap-2">
        <div className="font-mono text-xs" style={{ color: 'var(--text-2)' }}>
          Built-in tools available to AutoDev agents{' '}
          <span style={{ color: 'var(--text-3)' }}>
            ({tools.length} total, called in-process)
          </span>
        </div>
        <button
          type="button"
          onClick={() => { void load() }}
          disabled={loading}
          className="flex items-center gap-1.5 font-mono px-2 py-1 rounded-sm"
          style={{
            fontSize:   '11px',
            background: 'var(--bg-elevated, #1a1a1a)',
            border:     '1px solid var(--border-1)',
            color:      'var(--text-2)',
            opacity:    loading ? 0.6 : 1,
            cursor:     loading ? 'wait' : 'pointer',
          }}
        >
          <RefreshCw size={12} className={loading ? 'spin' : undefined} />
          Reload
        </button>
      </div>

      {/* Search */}
      <div
        className="flex items-center gap-2 px-2 py-1.5 rounded-sm"
        style={{
          background: 'var(--bg-elevated, #1a1a1a)',
          border:     '1px solid var(--border-1)',
        }}
      >
        <Search size={12} style={{ color: 'var(--text-3)' }} />
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Filter by tool name or description…"
          className="flex-1 bg-transparent outline-none font-mono"
          style={{ fontSize: '12px', color: 'var(--text-1)' }}
        />
      </div>

      {error && (
        <div className="font-mono text-xs" style={{ color: 'var(--danger, #ff6b6b)' }}>
          {error}
        </div>
      )}

      {filtered.length === 0 && !loading && !error && (
        <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
          {query ? 'No tools match the filter.' : 'No tools registered.'}
        </div>
      )}

      <ul className="space-y-1" style={{ listStyle: 'none', padding: 0, margin: 0 }}>
        {filtered.map((tool) => {
          const isOpen = expanded.has(tool.name)
          return (
            <li
              key={tool.name}
              className="rounded-sm"
              style={{
                background: 'var(--bg-elevated, #1a1a1a)',
                border:     '1px solid var(--border-1)',
              }}
            >
              <button
                type="button"
                onClick={() => toggle(tool.name)}
                className="w-full flex items-start gap-2 text-left p-2"
                style={{ background: 'transparent', border: 'none', cursor: 'pointer' }}
              >
                <div style={{ paddingTop: 2 }}>
                  {isOpen
                    ? <ChevronDown  size={12} style={{ color: 'var(--text-2)' }} />
                    : <ChevronRight size={12} style={{ color: 'var(--text-2)' }} />}
                </div>
                <Wrench size={12} style={{ color: 'var(--accent)', marginTop: 2 }} />
                <div className="flex-1 min-w-0">
                  <div className="font-mono text-xs" style={{ color: 'var(--text-1)' }}>
                    {tool.name}
                  </div>
                  <div
                    className="font-mono"
                    style={{ fontSize: '11px', color: 'var(--text-3)', marginTop: 2 }}
                  >
                    {tool.description}
                  </div>
                </div>
              </button>

              {isOpen && (
                <pre
                  className="font-mono"
                  style={{
                    fontSize:   '10.5px',
                    color:      'var(--text-2)',
                    background: 'var(--bg-deep, #0f0f0f)',
                    margin:     0,
                    padding:    '8px 10px',
                    borderTop:  '1px solid var(--border-1)',
                    overflow:   'auto',
                    maxHeight:  '280px',
                  }}
                >
                  {JSON.stringify(tool.input_schema, null, 2)}
                </pre>
              )}
            </li>
          )
        })}
      </ul>
    </div>
  )
}
