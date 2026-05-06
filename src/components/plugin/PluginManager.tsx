// PluginManager — Marketplace browser for the AIRS Plugin System.
// Phase 5.2: Plugin System & Worker Setup.
// Lists all available plugins, shows install / verify / uninstall actions.

import React, { useEffect, useState } from 'react'
import {
  usePluginStore,
  selectFilteredPlugins,
  PluginInfo,
  VerifyResult,
  InstallResult,
} from '../../stores/pluginStore'

// ─── Styles ──────────────────────────────────────────────────────────────────

const s: Record<string, React.CSSProperties> = {
  root: {
    display:       'flex',
    flexDirection: 'column',
    height:        '100%',
    background:    'var(--bg-1)',
    color:         'var(--text-1)',
    fontFamily:    'var(--font-sans)',
  },
  header: {
    display:        'flex',
    alignItems:     'center',
    gap:            12,
    padding:        '14px 20px',
    borderBottom:   '1px solid var(--border)',
    background:     'var(--bg-2)',
    flexShrink:     0,
  },
  title: {
    fontSize:   16,
    fontWeight: 600,
    color:      'var(--text-1)',
    margin:     0,
  },
  filterBar: {
    display:     'flex',
    gap:         8,
    padding:     '10px 20px',
    borderBottom:'1px solid var(--border)',
    flexShrink:  0,
    flexWrap:    'wrap',
    background:  'var(--bg-1)',
  },
  searchInput: {
    flex:         1,
    padding:      '4px 12px',
    borderRadius: 20,
    border:       '1px solid var(--border)',
    background:   'var(--bg-2)',
    color:        'var(--text-1)',
    fontSize:     13,
    minWidth:     0,
    outline:      'none',
  },
  grid: {
    flex:           1,
    overflowY:      'auto',
    padding:        16,
    display:        'grid',
    gridTemplateColumns: 'repeat(auto-fill, minmax(300px, 1fr))',
    gap:            14,
    alignContent:   'start',
  },
  cardHeader: {
    display:     'flex',
    alignItems:  'center',
    gap:         10,
  },
  icon: {
    fontSize:   26,
    flexShrink: 0,
  },
  cardName: {
    fontSize:   15,
    fontWeight: 600,
    color:      'var(--text-1)',
  },
  cardVersion: {
    fontSize: 12,
    color:    'var(--text-3)',
  },
  installedBadge: {
    marginLeft:   'auto',
    padding:      '2px 8px',
    borderRadius: 10,
    background:   'var(--accent)',
    color:        '#fff',
    fontSize:     11,
    fontWeight:   600,
    flexShrink:   0,
  },
  cardDesc: {
    fontSize: 13,
    color:    'var(--text-2)',
    lineHeight: 1.5,
  },
  tags: {
    display:  'flex',
    gap:      6,
    flexWrap: 'wrap',
  },
  tag: {
    padding:      '2px 8px',
    borderRadius: 10,
    background:   'var(--bg-3, #2a2a2a)',
    color:        'var(--text-3)',
    fontSize:     11,
  },
  cardActions: {
    display:        'flex',
    gap:            8,
    marginTop:      'auto',
    paddingTop:     8,
    borderTop:      '1px solid var(--border)',
  },
  emptyState: {
    textAlign:  'center',
    padding:    40,
    color:      'var(--text-3)',
    gridColumn: '1 / -1',
  },
  // Detail panel
  detail: {
    position:   'fixed',
    inset:      0,
    background: 'rgba(0,0,0,0.5)',
    display:    'flex',
    alignItems: 'center',
    justifyContent: 'center',
    zIndex:     100,
  },
  detailBox: {
    background:   'var(--bg-2)',
    border:       '1px solid var(--border)',
    borderRadius: 12,
    padding:      28,
    width:        520,
    maxHeight:    '85vh',
    overflowY:    'auto',
    display:      'flex',
    flexDirection:'column',
    gap:          16,
    boxShadow:    '0 8px 32px rgba(0,0,0,0.4)',
  },
  detailTitle: {
    fontSize:   20,
    fontWeight: 700,
    margin:     0,
    color:      'var(--text-1)',
  },
}

const filterBtn = (active: boolean): React.CSSProperties => ({
  padding:      '4px 14px',
  borderRadius: 20,
  border:       `1px solid ${active ? 'var(--accent)' : 'var(--border)'}`,
  background:   active ? 'var(--accent)' : 'transparent',
  color:        active ? '#fff' : 'var(--text-2)',
  fontSize:     13,
  cursor:       'pointer',
  transition:   'all 0.15s',
})

const card = (installed: boolean): React.CSSProperties => ({
  background:   'var(--bg-2)',
  border:       `1px solid ${installed ? 'var(--accent)' : 'var(--border)'}`,
  borderRadius: 10,
  padding:      16,
  display:      'flex',
  flexDirection:'column',
  gap:          10,
  cursor:       'pointer',
  transition:   'border-color 0.15s, box-shadow 0.15s',
})

const btn = (variant: 'primary' | 'ghost' | 'danger'): React.CSSProperties => ({
  padding:      '5px 14px',
  borderRadius: 6,
  border:       variant === 'ghost' ? '1px solid var(--border)' : 'none',
  background:   variant === 'primary' ? 'var(--accent)' :
                variant === 'danger'  ? 'var(--error, #c0392b)' :
                                        'transparent',
  color:        variant === 'ghost' ? 'var(--text-2)' : '#fff',
  fontSize:     13,
  cursor:       'pointer',
  transition:   'opacity 0.15s',
})

const checkRow = (present: boolean, required: boolean): React.CSSProperties => ({
  display:     'flex',
  alignItems:  'center',
  gap:         10,
  padding:     '6px 0',
  borderBottom:'1px solid var(--border)',
  color:       !present && required ? 'var(--error, #c0392b)' : 'var(--text-2)',
})

const resultBox = (success: boolean): React.CSSProperties => ({
  padding:      12,
  borderRadius: 8,
  background:   success ? 'rgba(39,174,96,0.1)' : 'rgba(192,57,43,0.1)',
  border:       `1px solid ${success ? 'var(--success, #27ae60)' : 'var(--error, #c0392b)'}`,
  color:        'var(--text-1)',
  fontSize:     13,
})

// ─── Main component ───────────────────────────────────────────────────────────

const PluginManager: React.FC = () => {
  const {
    loadingPlugins,
    pluginError,
    activePlugin,
    operationLoading,
    lastInstallResult,
    lastVerifyResult,
    filter,
    tagFilter,
    loadPlugins,
    setActivePlugin,
    installPlugin,
    uninstallPlugin,
    verifyPlugin,
    setFilter,
    setTagFilter,
    clearError,
  } = usePluginStore()

  const filteredPlugins = usePluginStore(selectFilteredPlugins)
  const [showDetail, setShowDetail] = useState(false)

  useEffect(() => {
    loadPlugins()
  }, [loadPlugins])

  const handleCardClick = (plugin: PluginInfo) => {
    setActivePlugin(plugin)
    setShowDetail(true)
  }

  const handleInstall = async (e: React.MouseEvent, plugin: PluginInfo) => {
    e.stopPropagation()
    await installPlugin(plugin.name)
    await loadPlugins()
  }

  const handleUninstall = async (e: React.MouseEvent, plugin: PluginInfo) => {
    e.stopPropagation()
    if (!confirm(`Uninstall plugin "${plugin.name}"?`)) return
    await uninstallPlugin(plugin.name)
    await loadPlugins()
  }

  const handleVerify = async (e: React.MouseEvent, plugin: PluginInfo) => {
    e.stopPropagation()
    setActivePlugin(plugin)
    await verifyPlugin(plugin.name)
    setShowDetail(true)
  }

  return (
    <div style={s.root}>
      {/* Header */}
      <div style={s.header}>
        <h2 style={s.title}>🔌 Plugin Marketplace</h2>
        <span style={{ marginLeft: 'auto', fontSize: 13, color: 'var(--text-3)' }}>
          {filteredPlugins.length} plugin{filteredPlugins.length !== 1 ? 's' : ''}
        </span>
      </div>

      {/* Filters */}
      <div style={s.filterBar}>
        {(['all', 'installed', 'builtin'] as const).map(f => (
          <button
            key={f}
            style={filterBtn(filter === f)}
            onClick={() => setFilter(f)}
          >
            {f.charAt(0).toUpperCase() + f.slice(1)}
          </button>
        ))}
        <input
          style={s.searchInput}
          placeholder="Filter by tag..."
          value={tagFilter}
          onChange={e => setTagFilter(e.target.value)}
        />
      </div>

      {/* Error banner */}
      {pluginError && (
        <div style={{ padding: '10px 20px', background: 'rgba(192,57,43,0.12)', color: 'var(--error, #e74c3c)', fontSize: 13 }}>
          {pluginError}
          <button
            onClick={clearError}
            style={{ marginLeft: 12, cursor: 'pointer', background: 'none', border: 'none', color: 'inherit' }}
          >✕</button>
        </div>
      )}

      {/* Grid */}
      <div style={s.grid}>
        {loadingPlugins && (
          <div style={s.emptyState}>Loading plugins...</div>
        )}
        {!loadingPlugins && filteredPlugins.length === 0 && (
          <div style={s.emptyState}>No plugins found.</div>
        )}
        {filteredPlugins.map(plugin => {
          const isLoading = operationLoading[plugin.name]
          return (
            <div
              key={plugin.name}
              style={card(plugin.installed)}
              onClick={() => handleCardClick(plugin)}
            >
              <div style={s.cardHeader}>
                <span style={s.icon}>{plugin.icon}</span>
                <div>
                  <div style={s.cardName}>{plugin.name}</div>
                  <div style={s.cardVersion}>v{plugin.version} · {plugin.author}</div>
                </div>
                {plugin.installed && <span style={s.installedBadge}>Installed</span>}
              </div>

              <div style={s.cardDesc}>{plugin.description}</div>

              <div style={s.tags}>
                {plugin.tags.map(t => <span key={t} style={s.tag}>{t}</span>)}
              </div>

              <div style={s.cardActions}>
                {!plugin.installed ? (
                  <button
                    style={btn('primary')}
                    disabled={isLoading}
                    onClick={e => handleInstall(e, plugin)}
                  >
                    {isLoading ? 'Installing…' : 'Install'}
                  </button>
                ) : (
                  <>
                    <button
                      style={btn('ghost')}
                      disabled={operationLoading[`verify_${plugin.name}`]}
                      onClick={e => handleVerify(e, plugin)}
                    >
                      Verify
                    </button>
                    <button
                      style={btn('danger')}
                      disabled={isLoading}
                      onClick={e => handleUninstall(e, plugin)}
                    >
                      Uninstall
                    </button>
                  </>
                )}
                <button
                  style={{ ...btn('ghost'), marginLeft: 'auto' }}
                  onClick={e => { e.stopPropagation(); handleCardClick(plugin) }}
                >
                  Details
                </button>
              </div>
            </div>
          )
        })}
      </div>

      {/* Detail overlay */}
      {showDetail && activePlugin && (
        <PluginDetailPanel
          plugin={activePlugin}
          verifyResult={lastVerifyResult?.plugin === activePlugin.name ? lastVerifyResult : null}
          installResult={lastInstallResult?.plugin === activePlugin.name ? lastInstallResult : null}
          onClose={() => setShowDetail(false)}
          onInstall={() => installPlugin(activePlugin.name).then(() => loadPlugins())}
          onUninstall={() => uninstallPlugin(activePlugin.name).then(() => loadPlugins())}
          onVerify={() => verifyPlugin(activePlugin.name)}
          loading={operationLoading[activePlugin.name] ?? false}
        />
      )}
    </div>
  )
}

// ─── Detail Panel ─────────────────────────────────────────────────────────────

interface DetailPanelProps {
  plugin:        PluginInfo
  verifyResult:  VerifyResult | null
  installResult: InstallResult | null
  onClose:       () => void
  onInstall:     () => void
  onUninstall:   () => void
  onVerify:      () => void
  loading:       boolean
}

const PluginDetailPanel: React.FC<DetailPanelProps> = ({
  plugin, verifyResult, installResult, onClose, onInstall, onUninstall, onVerify, loading,
}) => {
  return (
    <div style={s.detail} onClick={onClose}>
      <div style={s.detailBox} onClick={e => e.stopPropagation()}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <span style={{ fontSize: 32 }}>{plugin.icon}</span>
          <div>
            <h3 style={s.detailTitle}>{plugin.name}</h3>
            <div style={{ color: 'var(--text-3)', fontSize: 13 }}>
              v{plugin.version} · {plugin.author} · {plugin.plugin_type}
            </div>
          </div>
          <button
            onClick={onClose}
            style={{ marginLeft: 'auto', background: 'none', border: 'none', color: 'var(--text-3)', fontSize: 18, cursor: 'pointer' }}
          >✕</button>
        </div>

        <p style={{ margin: 0, color: 'var(--text-2)', fontSize: 14, lineHeight: 1.6 }}>
          {plugin.description}
        </p>

        <div style={s.tags}>
          {plugin.tags.map(t => <span key={t} style={s.tag}>{t}</span>)}
          {plugin.builtin && <span style={{ ...s.tag, background: 'var(--accent)', color: '#fff' }}>built-in</span>}
        </div>

        <div style={{ fontSize: 13, color: 'var(--text-3)' }}>
          Supports: {plugin.os_support.join(', ')}
        </div>

        {/* Install result */}
        {installResult && (
          <div style={resultBox(installResult.success)}>
            {installResult.success ? '✓ ' : '✗ '}{installResult.message}
            {installResult.warnings.map((w, i) => (
              <div key={i} style={{ color: 'var(--warning, #f39c12)', marginTop: 4 }}>⚠ {w}</div>
            ))}
          </div>
        )}

        {/* Verify results */}
        {verifyResult && (
          <div>
            <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 6, color: 'var(--text-1)' }}>
              Dependency Checks
            </div>
            {verifyResult.checks.map(check => (
              <div key={check.name} style={checkRow(check.present, check.required)}>
                <span style={{ fontSize: 16 }}>{check.present ? '✓' : check.required ? '✗' : '~'}</span>
                <span>{check.name}</span>
                {check.version && <span style={{ color: 'var(--text-3)', fontSize: 12 }}>{check.version}</span>}
                <span style={{ marginLeft: 'auto', fontSize: 12 }}>{check.message}</span>
              </div>
            ))}
            <div style={resultBox(verifyResult.success)}>
              {verifyResult.message}
            </div>
          </div>
        )}

        {/* Actions */}
        <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
          {!plugin.installed ? (
            <button style={btn('primary')} disabled={loading} onClick={onInstall}>
              {loading ? 'Installing…' : 'Install'}
            </button>
          ) : (
            <>
              <button style={btn('ghost')} onClick={onVerify}>Verify</button>
              <button style={btn('danger')} onClick={onUninstall}>Uninstall</button>
            </>
          )}
        </div>
      </div>
    </div>
  )
}

export default PluginManager
