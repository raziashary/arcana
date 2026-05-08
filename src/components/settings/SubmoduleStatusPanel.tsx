// SubmoduleStatusPanel.tsx — Display status of core-orchestrator and memory-mcp-server submodules.
// Phase 5.3 — Installer bundling: checks if submodules are found at installed location,
// environment variable paths, or relative dev paths.

import { useEffect, useState } from 'react'
import { CheckCircle, XCircle, Folder, RefreshCw, AlertCircle } from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'

interface SubmoduleStatus {
  id: string
  name: string
  path: string
  exists: boolean
  source: string  // "installed" | "env_var" | "relative" | "not_found"
  required: boolean
}

export default function SubmoduleStatusPanel() {
  const [submodules, setSubmodules] = useState<SubmoduleStatus[]>([])
  const [loading, setLoading] = useState(true)

  const loadStatus = async () => {
    setLoading(true)
    try {
      const result = await invoke<SubmoduleStatus[]>('get_submodule_paths')
      setSubmodules(result)
    } catch (e) {
      console.error('Failed to load submodule status:', e)
    }
    setLoading(false)
  }

  useEffect(() => {
    loadStatus()
  }, [])

  const getSourceBadge = (source: string) => {
    switch (source) {
      case 'installed':
        return <span className="text-xs px-2 py-0.5 rounded" style={{ background: 'var(--accent-1)', color: 'var(--bg-0)' }}>Installed</span>
      case 'env_var':
        return <span className="text-xs px-2 py-0.5 rounded" style={{ background: 'var(--accent-2)', color: 'var(--bg-0)' }}>Env Var</span>
      case 'relative':
        return <span className="text-xs px-2 py-0.5 rounded" style={{ background: 'var(--accent-3)', color: 'var(--bg-0)' }}>Dev</span>
      case 'not_found':
        return <span className="text-xs px-2 py-0.5 rounded" style={{ background: 'var(--error-1)', color: 'var(--bg-0)' }}>Not Found</span>
      default:
        return null
    }
  }

  return (
    <div
      className="rounded-sm p-4 space-y-3"
      style={{ background: 'var(--bg-card)', border: '1px solid var(--border-1)' }}
    >
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium" style={{ color: 'var(--text-1)' }}>
          Submodule Paths
        </h3>
        <button
          onClick={loadStatus}
          className="p-1 rounded hover:bg-gray-700 transition-colors"
          title="Refresh"
        >
          <RefreshCw size={14} style={{ color: 'var(--text-2)' }} />
        </button>
      </div>

      {loading ? (
        <div className="text-xs font-mono" style={{ color: 'var(--text-3)' }}>
          Checking submodule paths...
        </div>
      ) : (
        <div className="space-y-2">
          {submodules.map((sub) => (
            <div
              key={sub.id}
              className="flex items-start gap-3 p-2 rounded"
              style={{ background: 'var(--bg-1)' }}
            >
              <div className="mt-0.5">
                {sub.exists ? (
                  <CheckCircle size={16} className="text-green-500" />
                ) : (
                  <XCircle size={16} className="text-red-500" />
                )}
              </div>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-sm font-medium" style={{ color: 'var(--text-1)' }}>
                    {sub.name}
                  </span>
                  {getSourceBadge(sub.source)}
                  {sub.required && (
                    <span className="text-xs px-2 py-0.5 rounded" style={{ background: 'var(--border-2)', color: 'var(--text-2)' }}>
                      Required
                    </span>
                  )}
                </div>
                <div className="flex items-center gap-2 text-xs font-mono" style={{ color: 'var(--text-3)' }}>
                  <Folder size={12} />
                  <span className="truncate">{sub.path}</span>
                </div>
              </div>
            </div>
          ))}

          {submodules.some(s => !s.exists) && (
            <div className="flex items-start gap-2 p-3 rounded" style={{ background: 'var(--error-1-alpha)', border: '1px solid var(--error-1)' }}>
              <AlertCircle size={16} className="text-red-500 mt-0.5" />
              <div className="flex-1">
                <p className="text-sm font-medium" style={{ color: 'var(--text-1)' }}>
                  Missing submodules
                </p>
                <p className="text-xs mt-1" style={{ color: 'var(--text-2)' }}>
                  One or more submodules are not found. Fleet Mode 2 (LivingAgent) requires both submodules to be present.
                  Set <code className="font-mono">AUTODEV_SIDECAR_PATH</code> or <code className="font-mono">AUTODEV_MEMORY_PATH</code> environment variables,
                  or ensure the submodules are bundled in the installer.
                </p>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
