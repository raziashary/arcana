// UpdateSettings — settings panel for the auto-updater.
// Phase 5.3 — Auto-updater UI.
// Shows current version, last checked, update channel selector,
// manual check trigger, update history, and data integrity report.

import { useEffect, useState } from 'react'
import { useUpdateStore } from '../../stores/updateStore'

const VERSION = '__APP_VERSION__'   // replaced by Vite at build time via define

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`
}

export default function UpdateSettings() {
  const store = useUpdateStore()
  const [channel, setChannel] = useState<string>('stable')
  const [showIntegrity, setShowIntegrity] = useState(false)
  const [showHistory, setShowHistory]     = useState(false)

  useEffect(() => {
    store.loadHistory()
  }, [])

  const handleCheck = async () => {
    await store.checkForUpdate(channel)
  }

  const handleVerify = async () => {
    await store.verifyIntegrity()
    setShowIntegrity(true)
  }

  return (
    <div className="space-y-5">
      {/* Section header */}
      <div>
        <div className="font-mono text-sm font-semibold" style={{ color: 'var(--text-1)' }}>
          Updates
        </div>
        <div className="text-xs mt-0.5" style={{ color: 'var(--text-3)' }}>
          Manage AutoDev application updates
        </div>
      </div>

      {/* Current version card */}
      <div className="rounded-lg border p-4 space-y-1"
        style={{ background: 'var(--bg-2)', borderColor: 'var(--border-1)' }}>
        <div className="flex items-center justify-between">
          <span className="text-xs font-medium" style={{ color: 'var(--text-2)' }}>
            Current version
          </span>
          <span className="font-mono text-sm font-bold" style={{ color: 'var(--text-1)' }}>
            {VERSION}
          </span>
        </div>
        {store.lastChecked && (
          <div className="flex items-center justify-between">
            <span className="text-xs" style={{ color: 'var(--text-3)' }}>
              Last checked
            </span>
            <span className="text-xs" style={{ color: 'var(--text-3)' }}>
              {store.lastChecked.toLocaleString()}
            </span>
          </div>
        )}
      </div>

      {/* Available update notice */}
      {store.availableUpdate && (
        <div className="rounded-lg border p-4"
          style={{
            background: store.isCritical ? 'var(--orange-950, #1c0a00)' : 'var(--blue-950, #00091c)',
            borderColor: store.isCritical ? 'var(--orange-700, #c2550a)' : 'var(--blue-700, #0a6cc2)',
          }}>
          <div className="flex items-start justify-between gap-3">
            <div>
              <p className="text-sm font-semibold" style={{ color: 'var(--text-1)' }}>
                {store.isCritical ? '⚠ Critical update: ' : '🔵 '}
                Version {store.availableUpdate.version} available
              </p>
              {store.availableUpdate.release_notes && (
                <p className="text-xs mt-1 line-clamp-2" style={{ color: 'var(--text-3)' }}>
                  {store.availableUpdate.release_notes}
                </p>
              )}
            </div>
            <button
              onClick={store.openDialog}
              className="shrink-0 px-3 py-1.5 rounded-md text-xs font-medium text-white transition-colors"
              style={{ background: store.isCritical ? '#c2550a' : '#1d6fbf' }}
            >
              View
            </button>
          </div>
        </div>
      )}

      {/* Check for updates */}
      <div className="space-y-3">
        <div className="text-xs font-semibold uppercase tracking-wider" style={{ color: 'var(--text-3)' }}>
          Update channel
        </div>
        <div className="flex gap-2">
          {['stable', 'beta', 'nightly'].map((ch) => (
            <button
              key={ch}
              onClick={() => setChannel(ch)}
              className={`px-3 py-1.5 rounded-md text-xs font-medium capitalize transition-colors border
                ${channel === ch
                  ? 'text-white'
                  : 'opacity-60 hover:opacity-90'
                }`}
              style={{
                background: channel === ch ? 'var(--accent-1)' : 'var(--bg-2)',
                borderColor: channel === ch ? 'var(--accent-2)' : 'var(--border-1)',
                color: channel === ch ? '#fff' : 'var(--text-2)',
              }}
            >
              {ch}
            </button>
          ))}
        </div>

        <button
          onClick={handleCheck}
          disabled={store.isChecking}
          className="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          style={{ background: 'var(--bg-3)', color: 'var(--text-1)', border: '1px solid var(--border-1)' }}
        >
          {store.isChecking
            ? <><span className="animate-spin">⟳</span> Checking…</>
            : '⟳ Check for updates'}
        </button>

        {store.checkError && (
          <p className="text-xs text-red-400">{store.checkError}</p>
        )}
        {store.lastChecked && !store.isChecking && !store.availableUpdate && (
          <p className="text-xs" style={{ color: 'var(--text-3)' }}>
            ✓ AutoDev is up to date
          </p>
        )}
      </div>

      {/* Data integrity */}
      <div className="space-y-2 border-t pt-4" style={{ borderColor: 'var(--border-1)' }}>
        <div className="flex items-center justify-between">
          <div>
            <div className="text-xs font-semibold uppercase tracking-wider" style={{ color: 'var(--text-3)' }}>
              Data integrity
            </div>
            <div className="text-xs mt-0.5" style={{ color: 'var(--text-3)' }}>
              Verify that all user data is intact
            </div>
          </div>
          <button
            onClick={handleVerify}
            className="px-3 py-1.5 rounded-md text-xs font-medium transition-colors"
            style={{ background: 'var(--bg-3)', color: 'var(--text-2)', border: '1px solid var(--border-1)' }}
          >
            Verify
          </button>
        </div>

        {store.integrityReport && showIntegrity && (
          <div className="mt-2 rounded-lg border p-3 space-y-2"
            style={{ background: 'var(--bg-2)', borderColor: 'var(--border-1)' }}>
            <div className="flex items-center gap-2">
              <span className={store.integrityReport.all_ok ? 'text-green-400' : 'text-red-400'}>
                {store.integrityReport.all_ok ? '✓' : '✗'}
              </span>
              <span className="text-xs font-medium" style={{ color: 'var(--text-1)' }}>
                {store.integrityReport.message}
              </span>
            </div>
            <div className="space-y-1">
              {store.integrityReport.checks.map((check) => (
                <div key={check.name} className="flex items-center justify-between text-xs">
                  <span className="flex items-center gap-1.5" style={{ color: 'var(--text-2)' }}>
                    <span className={check.present ? 'text-green-400' : check.message.includes('critical') ? 'text-red-400' : 'text-yellow-500'}>
                      {check.present ? '✓' : '○'}
                    </span>
                    {check.name}
                  </span>
                  <span style={{ color: 'var(--text-3)' }}>
                    {check.present ? formatBytes(check.size) : 'not found'}
                  </span>
                </div>
              ))}
            </div>
            <button
              onClick={() => setShowIntegrity(false)}
              className="text-xs mt-1" style={{ color: 'var(--text-3)' }}
            >
              Close
            </button>
          </div>
        )}
      </div>

      {/* Update history */}
      <div className="space-y-2 border-t pt-4" style={{ borderColor: 'var(--border-1)' }}>
        <button
          onClick={() => setShowHistory(!showHistory)}
          className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider w-full"
          style={{ color: 'var(--text-3)' }}
        >
          <span>{showHistory ? '▾' : '▸'}</span>
          Update history
          {store.updateHistory.length > 0 && (
            <span className="ml-1 px-1.5 py-0.5 rounded text-xs"
              style={{ background: 'var(--bg-3)', color: 'var(--text-2)' }}>
              {store.updateHistory.length}
            </span>
          )}
        </button>

        {showHistory && (
          store.updateHistory.length === 0
            ? (
              <p className="text-xs py-2" style={{ color: 'var(--text-3)' }}>
                No updates installed yet.
              </p>
            )
            : (
              <div className="space-y-1.5">
                {[...store.updateHistory].reverse().map((record, i) => (
                  <div key={i} className="rounded-lg border p-3 text-xs"
                    style={{ background: 'var(--bg-2)', borderColor: 'var(--border-1)' }}>
                    <div className="flex items-center justify-between">
                      <span className="font-mono font-semibold" style={{ color: 'var(--text-1)' }}>
                        v{record.version}
                      </span>
                      <span style={{ color: 'var(--text-3)' }}>
                        {record.installed_at.slice(0, 10)}
                      </span>
                    </div>
                    {record.previous_version && (
                      <p style={{ color: 'var(--text-3)' }} className="mt-0.5">
                        from v{record.previous_version} · {record.channel}
                      </p>
                    )}
                  </div>
                ))}
              </div>
            )
        )}
      </div>
    </div>
  )
}
