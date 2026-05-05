// ITDashboard — Bulk deployment view for IT admins.
// Shows plugin status across the fleet, allows bulk install / verify,
// and provides a quick path to generate setup tokens for any worker type.
// Phase 5.2: Plugin System & Worker Setup.

import React, { useEffect, useState } from 'react'
import {
  usePluginStore,
  VerifyResult,
} from '../../stores/pluginStore'
import WorkerSetupWizard from './WorkerSetupWizard'

// ─── Styles ──────────────────────────────────────────────────────────────────

// Dynamic style helpers — typed functions kept outside `s` so the static
// `Record<string, React.CSSProperties>` annotation can be applied to `s`.
const trStyle = (even: boolean): React.CSSProperties => ({
  background: even ? 'var(--bg-2)' : 'transparent',
})

type BadgeVariant = 'success' | 'error' | 'warning' | 'info'
const badgeStyle = (variant: BadgeVariant): React.CSSProperties => ({
  display:      'inline-block',
  padding:      '2px 8px',
  borderRadius: 10,
  fontSize:     11,
  fontWeight:   600,
  background: ({
    success: 'rgba(39,174,96,0.15)',
    error:   'rgba(192,57,43,0.15)',
    warning: 'rgba(243,156,18,0.15)',
    info:    'rgba(52,152,219,0.15)',
  } as Record<BadgeVariant, string>)[variant],
  color: ({
    success: 'var(--success, #27ae60)',
    error:   'var(--error, #e74c3c)',
    warning: 'var(--warning, #f39c12)',
    info:    'var(--info, #3498db)',
  } as Record<BadgeVariant, string>)[variant],
})

const s: Record<string, React.CSSProperties> = {
  root: {
    display:        'flex',
    flexDirection:  'column',
    height:         '100%',
    background:     'var(--bg-1)',
    color:          'var(--text-1)',
  },
  header: {
    display:      'flex',
    alignItems:   'center',
    gap:          12,
    padding:      '14px 20px',
    borderBottom: '1px solid var(--border)',
    background:   'var(--bg-2)',
    flexShrink:   0,
  },
  title: {
    fontSize:   16,
    fontWeight: 600,
    margin:     0,
  },
  toolbar: {
    display:      'flex',
    gap:          10,
    padding:      '10px 20px',
    borderBottom: '1px solid var(--border)',
    flexShrink:   0,
    flexWrap:     'wrap',
    alignItems:   'center',
  },
  body: {
    flex:           1,
    overflowY:      'auto',
    padding:        16,
    display:        'flex',
    flexDirection:  'column',
    gap:            12,
  },
  table: {
    width:           '100%',
    borderCollapse:  'collapse',
    fontSize:        13,
  },
  th: {
    textAlign:    'left',
    padding:      '8px 12px',
    borderBottom: '2px solid var(--border)',
    color:        'var(--text-3)',
    fontWeight:   500,
    whiteSpace:   'nowrap',
  },
  td: {
    padding:       '10px 12px',
    borderBottom:  '1px solid var(--border)',
    verticalAlign: 'middle',
  },
  iconBtn: {
    padding:      '5px 12px',
    borderRadius: 6,
    border:       '1px solid var(--border)',
    background:   'transparent',
    color:        'var(--text-2)',
    fontSize:     12,
    cursor:       'pointer',
    transition:   'background 0.1s',
  },
  primaryBtn: {
    padding:      '7px 16px',
    borderRadius: 8,
    border:       'none',
    background:   'var(--accent)',
    color:        '#fff',
    fontSize:     13,
    cursor:       'pointer',
    fontWeight:   500,
  },
  sectionTitle: {
    fontSize:   14,
    fontWeight: 600,
    color:      'var(--text-1)',
    margin:     '0 0 10px 0',
  },
  statCard: {
    background:   'var(--bg-2)',
    border:       '1px solid var(--border)',
    borderRadius: 10,
    padding:      '16px 20px',
    minWidth:     120,
  },
  statValue: {
    fontSize:   28,
    fontWeight: 700,
    color:      'var(--text-1)',
  },
  statLabel: {
    fontSize:  12,
    color:     'var(--text-3)',
    marginTop: 2,
  },
  verifyPanel: {
    background:   'var(--bg-2)',
    border:       '1px solid var(--border)',
    borderRadius: 10,
    padding:      14,
    fontSize:     13,
  },
}

// ─── Component ────────────────────────────────────────────────────────────────

const ITDashboard: React.FC = () => {
  const {
    plugins,
    loadingPlugins,
    operationLoading,
    loadPlugins,
    installPlugin,
    uninstallPlugin,
    verifyPlugin,
  } = usePluginStore()

  const [showWizard, setShowWizard] = useState(false)
  const [verifyResults, setVerifyResults] = useState<Record<string, VerifyResult>>({})
  const [bulkLoading, setBulkLoading] = useState(false)
  const [selectedPlugins, setSelectedPlugins] = useState<Set<string>>(new Set())

  useEffect(() => {
    loadPlugins()
  }, [loadPlugins])

  const installedPlugins = plugins.filter(p => p.installed)
  const notInstalled     = plugins.filter(p => !p.installed)

  // ── Selection ────────────────────────────────────────────────────────────

  const toggleSelect = (name: string) => {
    setSelectedPlugins(prev => {
      const next = new Set(prev)
      if (next.has(name)) next.delete(name); else next.add(name)
      return next
    })
  }

  const toggleSelectAll = () => {
    if (selectedPlugins.size === installedPlugins.length) {
      setSelectedPlugins(new Set())
    } else {
      setSelectedPlugins(new Set(installedPlugins.map(p => p.name)))
    }
  }

  // ── Bulk actions ─────────────────────────────────────────────────────────

  const handleBulkVerify = async () => {
    const targets = selectedPlugins.size > 0
      ? installedPlugins.filter(p => selectedPlugins.has(p.name))
      : installedPlugins

    setBulkLoading(true)
    const results: Record<string, VerifyResult> = {}
    for (const p of targets) {
      results[p.name] = await verifyPlugin(p.name)
    }
    setVerifyResults(prev => ({ ...prev, ...results }))
    setBulkLoading(false)
  }

  const handleBulkInstall = async () => {
    const targets = notInstalled.filter(p =>
      selectedPlugins.size === 0 || selectedPlugins.has(p.name)
    )
    setBulkLoading(true)
    for (const p of targets) {
      await installPlugin(p.name)
    }
    await loadPlugins()
    setBulkLoading(false)
  }

  return (
    <div style={s.root}>
      {/* Header */}
      <div style={s.header}>
        <h2 style={s.title}>🖥️ IT Admin Dashboard</h2>
        <span style={{ color: 'var(--text-3)', fontSize: 13 }}>
          Bulk plugin deployment and worker enrollment
        </span>
        <button
          style={{ ...s.primaryBtn, marginLeft: 'auto' }}
          onClick={() => setShowWizard(true)}
        >
          + Enroll Worker
        </button>
      </div>

      {/* Toolbar */}
      <div style={s.toolbar}>
        <button style={s.iconBtn} onClick={handleBulkVerify} disabled={bulkLoading || loadingPlugins}>
          {bulkLoading ? 'Running…' : '✓ Verify All'}
        </button>
        <button style={s.iconBtn} onClick={handleBulkInstall} disabled={bulkLoading || loadingPlugins}>
          ↓ Install All
        </button>
        <button style={s.iconBtn} onClick={loadPlugins} disabled={loadingPlugins}>
          ↺ Refresh
        </button>
        <span style={{ marginLeft: 'auto', fontSize: 12, color: 'var(--text-3)' }}>
          {selectedPlugins.size > 0 ? `${selectedPlugins.size} selected` : 'All plugins'}
        </span>
      </div>

      {/* Body */}
      <div style={s.body}>

        {/* Stat cards */}
        <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
          {[
            { value: plugins.length,       label: 'Available',  variant: 'info'    as const },
            { value: installedPlugins.length, label: 'Installed', variant: 'success' as const },
            { value: notInstalled.length,  label: 'Not installed', variant: 'warning' as const },
            { value: Object.values(verifyResults).filter(r => !r.success).length, label: 'Failing', variant: 'error' as const },
          ].map(stat => (
            <div key={stat.label} style={s.statCard}>
              <div style={{ ...s.statValue, color: badgeStyle(stat.variant).color }}>{stat.value}</div>
              <div style={s.statLabel}>{stat.label}</div>
            </div>
          ))}
        </div>

        {/* Installed plugins table */}
        <div>
          <h4 style={s.sectionTitle}>Installed Plugins</h4>
          {installedPlugins.length === 0 ? (
            <div style={{ color: 'var(--text-3)', fontSize: 13 }}>
              No plugins installed. Use the Plugin Marketplace to install.
            </div>
          ) : (
            <table style={s.table}>
              <thead>
                <tr>
                  <th style={s.th}>
                    <input
                      type="checkbox"
                      checked={selectedPlugins.size === installedPlugins.length && installedPlugins.length > 0}
                      onChange={toggleSelectAll}
                    />
                  </th>
                  <th style={s.th}>Plugin</th>
                  <th style={s.th}>Version</th>
                  <th style={s.th}>Type</th>
                  <th style={s.th}>Status</th>
                  <th style={s.th}>Actions</th>
                </tr>
              </thead>
              <tbody>
                {installedPlugins.map((p, i) => {
                  const vr = verifyResults[p.name]
                  const isVerifying = operationLoading[`verify_${p.name}`]
                  return (
                    <tr key={p.name} style={trStyle(i % 2 === 1)}>
                      <td style={s.td}>
                        <input
                          type="checkbox"
                          checked={selectedPlugins.has(p.name)}
                          onChange={() => toggleSelect(p.name)}
                        />
                      </td>
                      <td style={s.td}>
                        <span style={{ marginRight: 8 }}>{p.icon}</span>
                        <strong>{p.name}</strong>
                      </td>
                      <td style={s.td}>{p.version}</td>
                      <td style={s.td}>{p.plugin_type}</td>
                      <td style={s.td}>
                        {vr ? (
                          <span style={badgeStyle(vr.success ? 'success' : 'error')}>
                            {vr.success ? '✓ OK' : '✗ Failed'}
                          </span>
                        ) : (
                          <span style={badgeStyle('info')}>Not verified</span>
                        )}
                      </td>
                      <td style={s.td}>
                        <div style={{ display: 'flex', gap: 6 }}>
                          <button
                            style={s.iconBtn}
                            disabled={isVerifying}
                            onClick={async () => {
                              const r = await verifyPlugin(p.name)
                              setVerifyResults(prev => ({ ...prev, [p.name]: r }))
                            }}
                          >
                            {isVerifying ? '…' : 'Verify'}
                          </button>
                          <button
                            style={{ ...s.iconBtn, color: 'var(--error, #e74c3c)' }}
                            onClick={async () => {
                              if (!confirm(`Remove ${p.name}?`)) return
                              await uninstallPlugin(p.name)
                              await loadPlugins()
                            }}
                          >
                            Remove
                          </button>
                        </div>
                      </td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          )}
        </div>

        {/* Verify summary */}
        {Object.keys(verifyResults).length > 0 && (
          <div style={s.verifyPanel}>
            <strong style={{ display: 'block', marginBottom: 8 }}>Verify Summary</strong>
            {Object.entries(verifyResults).map(([name, r]) => (
              <div key={name} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '4px 0' }}>
                <span style={{ fontSize: 16 }}>{r.success ? '✓' : '✗'}</span>
                <span style={{ fontWeight: 500 }}>{name}</span>
                <span style={{ color: 'var(--text-3)', fontSize: 12 }}>{r.message}</span>
                {r.checks.filter(c => !c.present && c.required).map(c => (
                  <span key={c.name} style={{ ...badgeStyle('error'), marginLeft: 4 }}>{c.name} missing</span>
                ))}
              </div>
            ))}
          </div>
        )}

        {/* Not installed */}
        {notInstalled.length > 0 && (
          <div>
            <h4 style={s.sectionTitle}>Available (not installed)</h4>
            <table style={s.table}>
              <thead>
                <tr>
                  <th style={s.th}>Plugin</th>
                  <th style={s.th}>Description</th>
                  <th style={s.th}>Tags</th>
                  <th style={s.th}>Actions</th>
                </tr>
              </thead>
              <tbody>
                {notInstalled.map((p, i) => (
                  <tr key={p.name} style={trStyle(i % 2 === 1)}>
                    <td style={s.td}>
                      <span style={{ marginRight: 8 }}>{p.icon}</span>
                      <strong>{p.name}</strong>
                    </td>
                    <td style={{ ...(s.td as React.CSSProperties), maxWidth: 280, color: 'var(--text-2)' }}>
                      {p.description}
                    </td>
                    <td style={s.td}>
                      {p.tags.slice(0, 3).map(t => (
                        <span
                          key={t}
                          style={{
                            marginRight: 4,
                            padding: '2px 6px',
                            borderRadius: 8,
                            background: 'var(--bg-3, #2a2a2a)',
                            fontSize: 11,
                            color: 'var(--text-3)',
                          }}
                        >{t}</span>
                      ))}
                    </td>
                    <td style={s.td}>
                      <button
                        style={s.iconBtn}
                        disabled={operationLoading[p.name]}
                        onClick={async () => {
                          await installPlugin(p.name)
                          await loadPlugins()
                        }}
                      >
                        {operationLoading[p.name] ? 'Installing…' : 'Install'}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

      </div>

      {/* Worker setup wizard */}
      {showWizard && <WorkerSetupWizard onClose={() => setShowWizard(false)} />}
    </div>
  )
}

export default ITDashboard
