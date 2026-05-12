// Settings tab — provider key management, model selection, GitHub PAT, worker management, model config.
// Phase 2.1: Providers + System Prompt.
// Phase 2.2: Workers section + GitHub PAT vault storage.
// Phase 2.3: Model Config section (temperature, top_p, reasoning effort, thinking).
// Phase 2.3+: Workspace folder selection — root dir for all AutoDev agents.
// References: DEVELOPMENT_PLAN.md Phase 2.3 — Provider Support & Model Enhancements

import { useEffect, useState } from 'react'
import { CheckCircle, XCircle, RefreshCw, Eye, EyeOff, RotateCcw, Save, Brain, FolderOpen, X, AlertTriangle, Globe } from 'lucide-react'
import { useSettingsStore } from '../stores/settingsStore'
import { useWorkerStore } from '../stores/workerStore'
import { useModelConfigStore, THINKING_MODELS, REASONING_EFFORT_MODELS } from '../stores/modelConfigStore'
import WorkerTable from '../components/workers/WorkerTable'
import WorkerCreateModal from '../components/workers/WorkerCreateModal'
import { invoke } from '@tauri-apps/api/core'
import { useWorkspaceStore } from '../stores/workspaceStore'
import { useRuntimeConfigStore } from '../stores/runtimeConfigStore'
import HooksSettings from '../components/settings/HooksSettings'
import UpdateSettings from '../components/settings/UpdateSettings'
import SidecarPanel from '../components/sidecar/SidecarPanel'
import McpToolBrowser from '../components/sidecar/McpToolBrowser'
import SubmoduleStatusPanel from '../components/settings/SubmoduleStatusPanel'

export default function SettingsTab() {
  const store          = useSettingsStore()
  const workerStore    = useWorkerStore()
  const workspaceStore = useWorkspaceStore()
  const [showCreateModal, setShowCreateModal] = useState(false)

  useEffect(() => {
    workerStore.loadWorkers()
    workspaceStore.loadWorkspaceDir()
  }, [])

  return (
    <div className="flex-1 overflow-y-auto p-5">
      <div className="max-w-2xl mx-auto space-y-5">

        <SectionHeader title="Workspace" />
        <WorkspaceCard />

        <SectionHeader title="Demo Mode" />
        <DemoModeCard />

        <SectionHeader title="Default Worker Budget" />
        <DefaultWorkerBudgetCard />

        <SectionHeader title="Services" />
        <SidecarPanel />

        <SectionHeader title="Submodules" />
        <SubmoduleStatusPanel />

        <SectionHeader title="MCP Tools" />
        <McpToolBrowser />

        <SectionHeader title="Browser Testing" />
        <BrowserTestingCard />

        <SectionHeader title="Providers" />

        {store.isLoadingProviders ? (
          <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
            Loading…
          </div>
        ) : (
          store.providers.map(provider => (
            <ProviderCard key={provider.id} providerId={provider.id} />
          ))
        )}

        {store.activeModel && (
          <div className="pt-2">
            <SectionHeader title="Model Parameters" />
            <ModelConfigSection modelId={store.activeModel} />
          </div>
        )}

        <div className="pt-4">
          <SectionHeader title="GitHub" />
          <div className="space-y-3">
            <GitHubPATCard />
            <GitHubMCPCard />
          </div>
        </div>

        <div className="pt-4">
          <SectionHeader title="Workers" />
          <div
            className="rounded-sm p-4"
            style={{ background: 'var(--bg-card)', border: '1px solid var(--border-1)' }}
          >
            <WorkerTable onAddWorker={() => setShowCreateModal(true)} />
          </div>
        </div>

        <div className="pt-4">
          <SectionHeader title="System Prompt" />
          <SystemPromptEditor />
        </div>

        <div className="pt-4">
          <SectionHeader title="Hooks" />
          <HooksSettings />
        </div>

        <div className="pt-4">
          <SectionHeader title="Updates" />
          <UpdateSettings />
        </div>

      </div>

      {showCreateModal && (
        <WorkerCreateModal
          onClose={() => setShowCreateModal(false)}
          onCreated={(_slug) => setShowCreateModal(false)}
        />
      )}
    </div>
  )
}

// ─── Workspace Folder ─────────────────────────────────────────────────────────

function WorkspaceCard() {
  const ws = useWorkspaceStore()

  const displayPath = ws.workspaceDir
    ? (ws.workspaceDir.length > 60
        ? '…' + ws.workspaceDir.slice(-57)
        : ws.workspaceDir)
    : null

  return (
    <div
      className="rounded-sm p-4 space-y-3"
      style={{ background: 'var(--bg-card)', border: '1px solid var(--border-1)' }}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span
            className="w-1.5 h-1.5 rounded-full shrink-0"
            style={{ background: ws.workspaceDir ? 'var(--aqua)' : '#0D3652' }}
          />
          <span className="font-mono text-xs" style={{ color: 'var(--text-1)' }}>
            Workspace Folder
          </span>
          {ws.workspaceDir && (
            <span
              className="font-mono px-1.5 py-0.5"
              style={{
                fontSize: '10px', background: 'var(--accent-muted)',
                border: '1px solid var(--accent-border)', color: 'var(--accent)', borderRadius: '2px',
              }}
            >
              ACTIVE
            </span>
          )}
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => ws.pickWorkspaceDir()}
            disabled={ws.isSaving}
            className="font-mono text-xs px-2 py-1 rounded-sm cursor-pointer disabled:opacity-40 flex items-center gap-1"
            style={{ background: 'var(--bg-2)', border: '1px solid var(--border-1)', color: 'var(--text-1)' }}
          >
            <FolderOpen size={11} />
            {ws.workspaceDir ? 'Change' : 'Select Folder'}
          </button>
          {ws.workspaceDir && (
            <button
              onClick={() => ws.clearWorkspaceDir()}
              className="font-mono text-xs px-2 py-1 rounded-sm cursor-pointer"
              style={{ background: 'transparent', border: '1px solid #FF636330', color: 'var(--danger)' }}
              aria-label="Clear workspace"
            >
              <X size={11} />
            </button>
          )}
        </div>
      </div>

      {displayPath ? (
        <div
          className="font-mono text-xs px-2 py-1.5 rounded-sm truncate"
          style={{ background: 'var(--bg-2)', color: 'var(--text-2)', border: '1px solid var(--border-1)' }}
          title={ws.workspaceDir}
        >
          {displayPath}
        </div>
      ) : (
        <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
          No workspace selected. All agents will use their default directory.
        </div>
      )}

      <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
        All agents (PA, Supervisor, Workers) will operate inside this folder.
        Use <span style={{ color: 'var(--accent)' }}>@path/to/file</span> in chat
        to reference files relative to this workspace.
      </div>

      {ws.error && (
        <div className="font-mono text-xs" style={{ color: 'var(--danger)' }}>
          {ws.error}
        </div>
      )}
    </div>
  )
}

// ─── Demo Mode (skip Python sidecars) ────────────────────────────────────────
//
// Toggles `runtime_config.json::skip_sidecars` on disk. When ON:
//   - memory-mcp-server and core-orchestrator are NOT spawned at next launch
//   - the StartupProgress FSM fast-forwards through every phase to "ready"
//   - Chat + Worker recipes still work via the native Rust AgentEngine
//   - LivingAgent / mem0 features are unavailable
// The change requires an app restart to take effect (sidecar spawn happens
// once, in the Tauri setup hook).

function DemoModeCard() {
  const cfg = useRuntimeConfigStore()

  useEffect(() => {
    cfg.load()
  }, [])

  const skip   = cfg.config?.skip_sidecars ?? false
  const dirty  = cfg.dirty
  const dotBg  = skip ? 'var(--warning)' : 'var(--aqua)'
  const badgeBg     = skip ? '#FFCB8B15' : 'var(--accent-muted)'
  const badgeBorder = skip ? '#FFCB8B40' : 'var(--accent-border)'
  const badgeColor  = skip ? 'var(--warning)' : 'var(--accent)'

  const onToggle = () => {
    cfg.setSkipSidecars(!skip)
  }

  return (
    <div
      className="rounded-sm p-4 space-y-3"
      style={{ background: 'var(--bg-card)', border: '1px solid var(--border-1)' }}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span
            className="w-1.5 h-1.5 rounded-full shrink-0"
            style={{ background: dotBg }}
          />
          <span className="font-mono text-xs" style={{ color: 'var(--text-1)' }}>
            Skip Python Sidecars
          </span>
          <span
            className="font-mono px-1.5 py-0.5"
            style={{
              fontSize: '10px', background: badgeBg,
              border: `1px solid ${badgeBorder}`, color: badgeColor, borderRadius: '2px',
            }}
          >
            {skip ? 'ON' : 'OFF'}
          </span>
        </div>
        <button
          onClick={onToggle}
          disabled={cfg.isLoading}
          className="font-mono text-xs px-2 py-1 rounded-sm cursor-pointer disabled:opacity-40"
          style={{
            background: skip ? 'var(--bg-2)' : 'var(--accent-muted)',
            border: `1px solid ${skip ? 'var(--border-1)' : 'var(--accent-border)'}`,
            color: skip ? 'var(--text-1)' : 'var(--accent)',
          }}
        >
          {skip ? 'Disable Demo Mode' : 'Enable Demo Mode'}
        </button>
      </div>

      <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
        When <span style={{ color: 'var(--accent)' }}>ON</span>, AutoDev launches without spawning
        memory-mcp-server or core-orchestrator. Chat and Worker recipes still run via the
        native Rust <span style={{ color: 'var(--accent)' }}>AgentEngine</span> (calls the LLM
        provider directly). Use this when the sidecars are under maintenance or for fast demos.
      </div>

      <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
        <span style={{ color: 'var(--text-2)' }}>Disabled features in Demo Mode:</span>{' '}
        LivingAgent memory persistence, mem0 vector search, A2A protocol endpoints,
        agent CRUD via core-orchestrator API.
      </div>

      {dirty && (
        <div
          className="font-mono text-xs px-2 py-1.5 rounded-sm flex items-center gap-2"
          style={{
            background: '#FFCB8B15', border: '1px solid #FFCB8B40', color: 'var(--warning)',
          }}
        >
          <RotateCcw size={11} />
          Restart AutoDev for this change to take effect.
        </div>
      )}

      {cfg.error && (
        <div className="font-mono text-xs" style={{ color: 'var(--danger)' }}>
          {cfg.error}
        </div>
      )}
    </div>
  )
}

// ─── Default Worker Budget (Ruflo Feature 4) ────────────────────────────────
//
// These values are persisted to ~/.autodev/runtime_config.json and read by
// the Rust recipe runner when assigning tasks without an explicit budget.

function DefaultWorkerBudgetCard() {
  const cfg = useRuntimeConfigStore()

  const [tokens,    setTokens]    = useState(100_000)
  const [usd,       setUsd]       = useState(0.50)
  const [turns,     setTurns]     = useState(20)
  const [warnPct,   setWarnPct]   = useState(80)
  const [saving,    setSaving]    = useState(false)
  const [saved,     setSaved]     = useState(false)
  const [error,     setError]     = useState<string | null>(null)

  useEffect(() => {
    cfg.load()
  }, [])

  useEffect(() => {
    if (cfg.config) {
      setTokens(cfg.config.default_worker_budget_tokens)
      setUsd(cfg.config.default_worker_budget_usd)
      setTurns(cfg.config.default_worker_budget_turns)
      setWarnPct(cfg.config.budget_warn_pct)
    }
  }, [cfg.config])

  const handleSave = async () => {
    setSaving(true)
    setError(null)
    try {
      await cfg.setDefaultWorkerBudget(tokens, usd, turns, warnPct)
      setSaved(true)
      setTimeout(() => setSaved(false), 2000)
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }

  const labelStyle = { color: 'var(--text-3)', fontSize: '10px' } as const
  const inputStyle = {
    background: 'var(--bg-2)', border: '1px solid var(--border-2)',
    color: 'var(--text-1)', borderRadius: '2px', padding: '3px 6px',
    fontFamily: 'monospace', fontSize: '11px', width: 90,
  } as const

  return (
    <div
      className="rounded-sm p-4 space-y-4"
      style={{ background: 'var(--bg-card)', border: '1px solid var(--border-1)' }}
    >
      <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
        Applied when Supervisor assigns a task without an explicit budget.
        Stored in <span style={{ color: 'var(--accent)' }}>runtime_config.json</span> — available to the Rust engine.
      </div>

      <div className="grid grid-cols-2 gap-4">
        {/* Max Tokens */}
        <div>
          <div className="font-mono mb-1" style={labelStyle}>MAX TOKENS PER TASK</div>
          <input
            type="number" min={1000} max={1_000_000} step={1000}
            value={tokens}
            onChange={e => setTokens(Math.max(1000, parseInt(e.target.value) || 1000))}
            style={inputStyle}
            title="Max tokens per task"
          />
          <div className="font-mono mt-0.5" style={{ ...labelStyle, fontSize: '9px' }}>
            ceiling: 1,000,000
          </div>
        </div>

        {/* Max USD */}
        <div>
          <div className="font-mono mb-1" style={labelStyle}>MAX USD PER TASK</div>
          <input
            type="number" min={0.01} max={100} step={0.01}
            value={usd}
            onChange={e => setUsd(Math.max(0.01, parseFloat(e.target.value) || 0.01))}
            style={inputStyle}
            title="Max USD per task"
          />
          <div className="font-mono mt-0.5" style={{ ...labelStyle, fontSize: '9px' }}>
            ceiling: $100.00
          </div>
        </div>

        {/* Max Turns */}
        <div>
          <div className="font-mono mb-1" style={labelStyle}>MAX TURNS</div>
          <input
            type="number" min={1} max={100} step={1}
            value={turns}
            onChange={e => setTurns(Math.max(1, parseInt(e.target.value) || 1))}
            style={inputStyle}
            title="Max agent turns per task"
          />
          <div className="font-mono mt-0.5" style={{ ...labelStyle, fontSize: '9px' }}>
            ceiling: 100
          </div>
        </div>

        {/* Warn % */}
        <div>
          <div className="font-mono mb-1" style={labelStyle}>
            WARN AT &nbsp;<span style={{ color: 'var(--warning)' }}>{warnPct}%</span>
          </div>
          <input
            type="range" min={10} max={100} step={5}
            value={warnPct}
            onChange={e => setWarnPct(parseInt(e.target.value))}
            className="w-full"
            title={`Warn when ${warnPct}% of budget used`}
          />
        </div>
      </div>

      {error && (
        <div className="font-mono text-xs" style={{ color: 'var(--danger)' }}>{error}</div>
      )}

      <div className="flex justify-end">
        <button
          onClick={handleSave}
          disabled={saving}
          className="font-mono text-xs px-3 py-1 rounded-sm cursor-pointer disabled:opacity-40 flex items-center gap-1.5"
          style={{ background: 'var(--accent-muted)', border: '1px solid var(--accent-border)', color: 'var(--accent)' }}
        >
          {saving ? 'Saving…' : saved ? 'Saved!' : <><Save size={11} /> Save</>}
        </button>
      </div>
    </div>
  )
}

// ─── GitHub PAT ───────────────────────────────────────────────────────────────

function GitHubPATCard() {
  const [pat,       setPat]       = useState('')
  const [hasPat,    setHasPat]    = useState<boolean | null>(null)
  const [showPat,   setShowPat]   = useState(false)
  const [saving,    setSaving]    = useState(false)
  const [status,    setStatus]    = useState<string | null>(null)

  useEffect(() => {
    invoke<string | null>('get_github_pat')
      .then(v   => setHasPat(v !== null && v !== ''))
      .catch(()  => setHasPat(false))
  }, [])

  const handleSave = async () => {
    if (!pat.trim()) return
    setSaving(true)
    setStatus(null)
    try {
      await invoke('store_github_pat', { pat: pat.trim() })
      setHasPat(true)
      setPat('')
      setStatus('PAT saved to OS keychain')
    } catch (e) {
      setStatus(`Error: ${e instanceof Error ? e.message : String(e)}`)
    } finally {
      setSaving(false)
    }
  }

  const handleRemove = async () => {
    if (!confirm('Remove GitHub PAT from keychain?')) return
    try {
      await invoke('delete_github_pat')
      setHasPat(false)
      setStatus('PAT removed')
    } catch (e) {
      setStatus(`Error: ${e instanceof Error ? e.message : String(e)}`)
    }
  }

  return (
    <div
      className="rounded-sm p-4 space-y-3"
      style={{ background: 'var(--bg-card)', border: '1px solid var(--border-1)' }}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span
            className="w-1.5 h-1.5 rounded-full shrink-0"
            style={{ background: hasPat ? 'var(--aqua)' : '#0D3652' }}
          />
          <span className="font-mono text-xs" style={{ color: 'var(--text-1)' }}>
            GitHub Personal Access Token
          </span>
          {hasPat && (
            <span
              className="font-mono px-1.5 py-0.5"
              style={{
                fontSize: '10px', background: 'var(--accent-muted)',
                border: '1px solid var(--accent-border)', color: 'var(--accent)', borderRadius: '2px',
              }}
            >
              STORED
            </span>
          )}
        </div>
        {hasPat && (
          <button
            onClick={handleRemove}
            className="font-mono text-xs px-2 py-1 rounded-sm cursor-pointer"
            style={{ background: 'transparent', border: '1px solid #FF636330', color: 'var(--danger)' }}
          >
            Remove
          </button>
        )}
      </div>

      <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
        Required for workers to create pull requests. Stored securely in your OS keychain.
      </div>

      {status && (
        <div className="font-mono text-xs" style={{ color: status.startsWith('Error') ? 'var(--danger)' : 'var(--accent)' }}>
          {status}
        </div>
      )}

      {!hasPat && (
        <div className="flex gap-2 items-center">
          <div className="flex-1 flex items-center relative">
            <input
              type={showPat ? 'text' : 'password'}
              placeholder="ghp_..."
              value={pat}
              onChange={e => setPat(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && handleSave()}
              className="flex-1 font-mono text-xs px-3 py-1.5 rounded-sm"
              style={{
                background: 'var(--bg-surface)', border: '1px solid var(--border-2)',
                color: 'var(--text-1)', paddingRight: '32px',
              }}
            />
            <button
              onClick={() => setShowPat(!showPat)}
              className="absolute right-2 border-none cursor-pointer"
              style={{ background: 'transparent', color: 'var(--text-3)' }}
            >
              {showPat ? <EyeOff size={12} /> : <Eye size={12} />}
            </button>
          </div>
          <button
            onClick={handleSave}
            disabled={saving || !pat.trim()}
            className="font-mono text-xs px-3 py-1.5 rounded-sm cursor-pointer disabled:opacity-40"
            style={{ background: 'var(--accent-muted)', border: '1px solid var(--accent-border)', color: 'var(--accent)' }}
          >
            {saving ? 'Saving…' : 'Save'}
          </button>
        </div>
      )}
    </div>
  )
}

// ─── GitHub MCP Card —————————————————————————————————————————————————————————————————

interface GhMcpConfig {
  enabled:       boolean
  org:           string | null
  repos:         string[]
  server_url:    string | null
}

function GitHubMCPCard() {
  const [cfg,     setCfg]     = useState<GhMcpConfig>({ enabled: false, org: null, repos: [], server_url: null })
  const [orgInput, setOrgInput] = useState('')
  const [repoInput, setRepoInput] = useState('')
  const [saving,  setSaving]  = useState(false)
  const [saved,   setSaved]   = useState(false)

  useEffect(() => {
    invoke<GhMcpConfig>('get_github_mcp_config')
      .then(c => { setCfg(c); setOrgInput(c.org ?? ''); setRepoInput(c.repos.join('\n')) })
      .catch(() => {})
  }, [])

  const handleSave = async () => {
    setSaving(true)
    try {
      await invoke('set_github_mcp_config', {
        enabled:   cfg.enabled,
        org:       orgInput.trim() || null,
        repos:     repoInput.split('\n').map(r => r.trim()).filter(Boolean),
        serverUrl: cfg.server_url || null,
      })
      setSaved(true)
      setTimeout(() => setSaved(false), 2000)
    } catch (e) {
      console.error('GitHub MCP save failed:', e)
    } finally {
      setSaving(false)
    }
  }

  return (
    <div
      className="rounded-sm p-4 space-y-3"
      style={{ background: 'var(--bg-card)', border: `1px solid ${cfg.enabled ? 'var(--accent-border)' : 'var(--border-1)'}` }}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span
            className="w-1.5 h-1.5 rounded-full shrink-0"
            style={{ background: cfg.enabled ? 'var(--aqua)' : '#0D3652' }}
          />
          <span className="font-mono text-xs" style={{ color: 'var(--text-1)' }}>GitHub MCP Server</span>
          {cfg.enabled && (
            <span
              className="font-mono px-1.5 py-0.5"
              style={{ fontSize: '10px', background: 'var(--accent-muted)', border: '1px solid var(--accent-border)', color: 'var(--accent)', borderRadius: '2px' }}
            >
              ENABLED
            </span>
          )}
        </div>
        <button
          onClick={() => setCfg(c => ({ ...c, enabled: !c.enabled }))}
          className="font-mono text-xs px-3 py-1 rounded-sm cursor-pointer"
          style={{
            background: cfg.enabled ? 'var(--accent-muted)' : 'var(--bg-surface)',
            border: `1px solid ${cfg.enabled ? 'var(--accent-border)' : 'var(--border-2)'}`,
            color: cfg.enabled ? 'var(--accent)' : 'var(--text-2)',
          }}
        >
          {cfg.enabled ? 'Disable' : 'Enable'}
        </button>
      </div>

      <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
        Connects to the GitHub MCP server (<code>@modelcontextprotocol/server-github</code>) using your stored PAT.
        Enables workers to list repos, create PRs, and review code.
      </div>

      <div className="space-y-2">
        <div className="font-mono text-xs" style={{ color: 'var(--text-3)', fontSize: '10px', textTransform: 'uppercase', letterSpacing: '0.1em' }}>GitHub Org / Username</div>
        <input
          type="text"
          placeholder="AIRS-Foundry (leave empty for all accessible repos)"
          value={orgInput}
          onChange={e => setOrgInput(e.target.value)}
          className="w-full font-mono text-xs px-3 py-1.5 rounded-sm"
          style={{ background: 'var(--bg-surface)', border: '1px solid var(--border-2)', color: 'var(--text-1)' }}
        />
      </div>

      <div className="space-y-2">
        <div className="font-mono text-xs" style={{ color: 'var(--text-3)', fontSize: '10px', textTransform: 'uppercase', letterSpacing: '0.1em' }}>Allowed Repos (one per line, empty = all)</div>
        <textarea
          placeholder="autodev&#10;core-orchestrator"
          value={repoInput}
          onChange={e => setRepoInput(e.target.value)}
          rows={3}
          className="w-full font-mono text-xs px-3 py-1.5 rounded-sm resize-none"
          style={{ background: 'var(--bg-surface)', border: '1px solid var(--border-2)', color: 'var(--text-1)' }}
        />
      </div>

      <button
        onClick={handleSave}
        disabled={saving}
        className="font-mono text-xs px-3 py-1 rounded-sm cursor-pointer disabled:opacity-40 flex items-center gap-1.5"
        style={{ background: 'var(--accent-muted)', border: '1px solid var(--accent-border)', color: 'var(--accent)' }}
      >
        {saving ? 'Saving…' : saved ? 'Saved!' : <><Save size={11} /> Save Config</>}
      </button>
    </div>
  )
}

// ─── Browser Testing (Ruflo Feature 7) ───────────────────────────────────────

interface BrowserInfo      { name: string; installed: boolean }
interface BrowserAvailability {
  playwright_installed: boolean
  playwright_version:   string | null
  browsers:             BrowserInfo[]
  npx_available:        boolean
}

function BrowserTestingCard() {
  const [avail,       setAvail]       = useState<BrowserAvailability | null>(null)
  const [loading,     setLoading]     = useState(false)
  const [installing,  setInstalling]  = useState(false)
  const [installErr,  setInstallErr]  = useState<string | null>(null)
  const [stagingUrl,  setStagingUrl]  = useState('http://localhost:3000')
  const [browserEnabled, setBrowserEnabled] = useState(false)

  const checkAvailability = async () => {
    setLoading(true)
    try {
      const result = await invoke<BrowserAvailability>('check_playwright_available')
      setAvail(result)
      if (result.playwright_installed) setBrowserEnabled(true)
    } catch { /* ignore */ } finally { setLoading(false) }
  }

  useEffect(() => { void checkAvailability() }, [])

  const handleInstall = async () => {
    setInstalling(true)
    setInstallErr(null)
    try {
      await invoke('install_playwright_browsers', { browser: 'chromium' })
      await checkAvailability()
    } catch (e) {
      setInstallErr(String(e))
    } finally {
      setInstalling(false)
    }
  }

  const notInstalled = avail && !avail.playwright_installed

  return (
    <div
      className="rounded-sm p-4 space-y-3"
      style={{ background: 'var(--bg-card)', border: '1px solid var(--border-1)' }}
    >
      {/* Playwright not installed banner */}
      {notInstalled && (
        <div
          className="flex items-center gap-2 p-2 rounded-sm"
          style={{ background: 'rgba(255,203,139,0.08)', border: '1px solid rgba(255,203,139,0.2)' }}
        >
          <AlertTriangle size={13} style={{ color: 'var(--warning, #FFCB8B)', flexShrink: 0 }} />
          <span className="font-mono text-xs" style={{ color: 'var(--warning, #FFCB8B)' }}>
            Browser testing requires Playwright. Click Install to set it up.
          </span>
        </div>
      )}

      {/* Status row */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Globe size={13} style={{ color: 'var(--text-3)' }} />
          <span className="font-mono text-xs" style={{ color: 'var(--text-2)' }}>
            Playwright
          </span>
          {avail ? (
            avail.playwright_installed ? (
              <span className="font-mono text-xs" style={{ color: 'var(--aqua, #7fdbff)' }}>
                ✓ {avail.playwright_version ?? 'installed'}
              </span>
            ) : (
              <span className="font-mono text-xs" style={{ color: 'var(--danger, #ff6b6b)' }}>
                not installed
              </span>
            )
          ) : (
            <span className="font-mono text-xs" style={{ color: 'var(--text-4)' }}>
              {loading ? 'checking…' : '—'}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => void checkAvailability()}
            disabled={loading}
            title="Re-check"
            style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-4)', padding: 2 }}
          >
            <RefreshCw size={10} className={loading ? 'anim-spin' : ''} />
          </button>
          {!avail?.playwright_installed && (
            <button
              onClick={() => void handleInstall()}
              disabled={installing || !avail?.npx_available}
              className="font-mono px-2 py-1 rounded-sm"
              style={{
                fontSize: '10px',
                background: 'rgba(127,219,255,0.08)',
                border: '1px solid rgba(127,219,255,0.3)',
                color: 'var(--aqua, #7fdbff)',
                cursor: installing || !avail?.npx_available ? 'default' : 'pointer',
                opacity: installing || !avail?.npx_available ? 0.6 : 1,
              }}
            >
              {installing ? 'Installing…' : 'Install Playwright'}
            </button>
          )}
        </div>
      </div>

      {/* Install error */}
      {installErr && (
        <div className="font-mono text-xs" style={{ color: 'var(--danger, #ff6b6b)' }}>
          {installErr}
        </div>
      )}

      {/* Browser list */}
      {avail?.playwright_installed && avail.browsers.length > 0 && (
        <div className="space-y-1">
          {avail.browsers.map(b => (
            <div key={b.name} className="flex items-center gap-2">
              <span
                style={{
                  width: 6, height: 6, borderRadius: '50%', flexShrink: 0,
                  background: b.installed ? 'var(--aqua, #7fdbff)' : 'var(--text-4)',
                  display: 'inline-block',
                }}
              />
              <span className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
                {b.name}
              </span>
              {!b.installed && (
                <button
                  onClick={() => invoke('install_playwright_browsers', { browser: b.name })}
                  className="font-mono"
                  style={{
                    fontSize: '9px', padding: '1px 5px', borderRadius: 2,
                    background: 'var(--accent-muted)', border: '1px solid var(--border-1)',
                    color: 'var(--text-3)', cursor: 'pointer',
                  }}
                >
                  install
                </button>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Staging URL */}
      <div className="space-y-1">
        <label className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
          Staging URL
        </label>
        <input
          type="url"
          value={stagingUrl}
          onChange={e => setStagingUrl(e.target.value)}
          placeholder="http://localhost:3000"
          className="w-full font-mono rounded-sm px-2 py-1"
          style={{
            fontSize: '11px',
            background: 'var(--bg-elevated, #1a1a1a)',
            border: '1px solid var(--border-1)',
            color: 'var(--text-1)',
            outline: 'none',
          }}
        />
        <div className="font-mono text-xs" style={{ color: 'var(--text-4)' }}>
          QC Agent navigates here for visual regression screenshots on high-risk frontend PRs.
        </div>
      </div>

      {/* Enable toggle */}
      <div className="flex items-center justify-between">
        <span className="font-mono text-xs" style={{ color: 'var(--text-2)' }}>
          Enable browser testing for frontend PRs
        </span>
        <button
          onClick={() => setBrowserEnabled(v => !v)}
          disabled={!avail?.playwright_installed}
          style={{
            width: 32, height: 18, borderRadius: 9, border: 'none',
            background: browserEnabled ? 'var(--aqua, #7fdbff)' : 'var(--bg-elevated, #1a1a1a)',
            cursor: avail?.playwright_installed ? 'pointer' : 'default',
            position: 'relative', transition: 'background 0.2s',
            outline: '1px solid var(--border-1)',
            opacity: avail?.playwright_installed ? 1 : 0.4,
          }}
          title={avail?.playwright_installed ? undefined : 'Install Playwright first'}
        >
          <span
            style={{
              position: 'absolute', top: 3, borderRadius: '50%',
              width: 12, height: 12,
              background: browserEnabled ? '#0d1117' : 'var(--text-4)',
              left: browserEnabled ? 17 : 3,
              transition: 'left 0.2s',
            }}
          />
        </button>
      </div>
    </div>
  )
}

// ─────────────────────────────────────────────────────────────────────────────

function SectionHeader({ title }: { title: string }) {
  return (
    <div
      className="font-mono text-xs mb-2"
      style={{
        color:         'var(--text-3)',
        letterSpacing: '0.1em',
        textTransform: 'uppercase',
        borderBottom:  '1px solid var(--border-1)',
        paddingBottom: '8px',
      }}
    >
      {title}
    </div>
  )
}

function SystemPromptEditor() {
  const store = useSettingsStore()
  const [draft, setDraft] = useState(store.systemPrompt ?? store.defaultSystemPrompt)
  const [saving, setSaving] = useState(false)
  const hasChanges = draft !== (store.systemPrompt ?? store.defaultSystemPrompt)
  const isUsingDefault = store.systemPrompt === null

  useEffect(() => {
    store.loadSystemPrompt()
  }, [])

  useEffect(() => {
    setDraft(store.systemPrompt ?? store.defaultSystemPrompt)
  }, [store.systemPrompt])

  const handleSave = async () => {
    setSaving(true)
    try {
      const trimmed = draft.trim()
      if (trimmed === store.defaultSystemPrompt.trim() || trimmed === '') {
        await store.resetSystemPrompt()
      } else {
        await store.setSystemPrompt(trimmed)
      }
    } finally {
      setSaving(false)
    }
  }

  const handleReset = () => {
    setDraft(store.defaultSystemPrompt)
  }

  const handleClear = async () => {
    await store.resetSystemPrompt()
    setDraft(store.defaultSystemPrompt)
  }

  return (
    <div
      className="rounded-sm p-4 space-y-3"
      style={{
        background: 'var(--bg-card)',
        border:     '1px solid var(--border-1)',
      }}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="font-mono text-xs" style={{ color: 'var(--text-2)' }}>
            {isUsingDefault ? 'Using default prompt' : 'Custom prompt active'}
          </span>
          {!isUsingDefault && (
            <span
              className="font-mono px-1.5 py-0.5"
              style={{
                fontSize:   '10px',
                background: 'var(--accent-muted)',
                border:     '1px solid var(--accent-border)',
                color:      'var(--accent)',
                borderRadius: '2px',
              }}
            >
              MODIFIED
            </span>
          )}
        </div>
        <div className="flex gap-2">
          {!isUsingDefault && (
            <button
              onClick={handleClear}
              className="font-mono text-xs px-2 py-1 rounded-sm cursor-pointer"
              style={{
                background: 'transparent',
                border:     '1px solid var(--border-2)',
                color:      'var(--text-3)',
              }}
            >
              Reset to Default
            </button>
          )}
          <button
            onClick={handleReset}
            disabled={!hasChanges}
            title="Revert changes"
            className="font-mono text-xs px-2 py-1 rounded-sm cursor-pointer disabled:opacity-40"
            style={{
              background: 'var(--bg-surface)',
              border:     '1px solid var(--border-2)',
              color:      'var(--text-2)',
            }}
          >
            <RotateCcw size={11} />
          </button>
          <button
            onClick={handleSave}
            disabled={!hasChanges || saving}
            className="font-mono text-xs px-3 py-1 rounded-sm cursor-pointer disabled:opacity-40 flex items-center gap-1.5"
            style={{
              background: 'var(--accent-muted)',
              border:     '1px solid var(--accent-border)',
              color:      'var(--accent)',
            }}
          >
            {saving ? 'Saving…' : <><Save size={11} /> Save</>}
          </button>
        </div>
      </div>

      <textarea
        value={draft}
        onChange={e => setDraft(e.target.value)}
        rows={12}
        className="w-full font-mono text-xs p-3 rounded-sm resize-y"
        style={{
          background: 'var(--bg-surface)',
          border:     '1px solid var(--border-2)',
          color:      'var(--text-1)',
          lineHeight: '1.6',
        }}
        placeholder="Enter custom system prompt..."
      />

      <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
        The system prompt defines how the AI behaves. Changes take effect on the next message.
        CLAUDE.md files in your home or project directory will still be appended.
      </div>
    </div>
  )
}

function ProviderCard({ providerId }: { providerId: string }) {
  const store        = useSettingsStore()
  const provider     = store.providers.find(p => p.id === providerId)
  const [key,        setKey]        = useState('')
  const [showKey,    setShowKey]    = useState(false)
  const [testing,    setTesting]    = useState(false)
  const [testResult, setTestResult] = useState<string | null>(null)
  const [saving,     setSaving]     = useState(false)

  if (!provider) return null

  const isActive      = store.activeProvider === providerId
  const displayName   = provider.name
  const hasKey        = provider.has_key
  const defaultModel  = provider.models[1]?.id ?? provider.models[0]?.id ?? ''

  const handleSaveKey = async () => {
    if (!key.trim()) return
    setSaving(true)
    try {
      await store.storeProviderKey(providerId, key.trim())
      if (!isActive && defaultModel) {
        await store.setActiveProvider(providerId, defaultModel)
      }
      setKey('')
      setTestResult(null)
    } catch (e) {
      console.error('Failed to save key:', e)
    } finally {
      setSaving(false)
    }
  }

  const handleDeleteKey = async () => {
    if (!confirm(`Remove ${displayName} API key?`)) return
    await store.deleteProviderKey(providerId)
    setTestResult(null)
  }

  const handleTest = async () => {
    setTesting(true)
    setTestResult(null)
    try {
      const result = await store.testProvider(providerId)
      setTestResult(result)
    } catch (e: unknown) {
      setTestResult(`Error: ${e instanceof Error ? e.message : String(e)}`)
    } finally {
      setTesting(false)
    }
  }

  const handleSetActive = async (model: string) => {
    await store.setActiveProvider(providerId, model)
  }

  return (
    <div
      className="rounded-sm p-4 space-y-3"
      style={{
        background: 'var(--bg-card)',
        border:     `1px solid ${isActive ? 'var(--accent-border)' : 'var(--border-1)'}`,
      }}
    >
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span
            className="w-1.5 h-1.5 rounded-full shrink-0"
            style={{ background: hasKey ? 'var(--aqua)' : '#0D3652' }}
          />
          <span className="font-mono text-xs" style={{ color: 'var(--text-1)' }}>
            {displayName}
          </span>
          {isActive && (
            <span
              className="font-mono px-1.5 py-0.5"
              style={{
                fontSize:   '10px',
                background: 'var(--accent-muted)',
                border:     '1px solid var(--accent-border)',
                color:      'var(--accent)',
                borderRadius: '2px',
              }}
            >
              ACTIVE
            </span>
          )}
        </div>

        {hasKey && (
          <div className="flex gap-2">
            <button
              onClick={handleTest}
              disabled={testing}
              className="font-mono text-xs px-2 py-1 rounded-sm cursor-pointer disabled:opacity-40"
              style={{
                background: 'var(--bg-surface)',
                border:     '1px solid var(--border-2)',
                color:      'var(--text-2)',
              }}
            >
              {testing ? <RefreshCw size={11} className="animate-spin" /> : 'Test'}
            </button>
            <button
              onClick={handleDeleteKey}
              className="font-mono text-xs px-2 py-1 rounded-sm cursor-pointer"
              style={{
                background: 'transparent',
                border:     '1px solid #FF636330',
                color:      'var(--danger)',
              }}
            >
              Remove
            </button>
          </div>
        )}
      </div>

      {/* Test result */}
      {testResult && (
        <div
          className="flex items-center gap-1.5 font-mono text-xs"
          style={{ color: testResult.toLowerCase().includes('connected') ? 'var(--accent)' : 'var(--danger)' }}
        >
          {testResult.toLowerCase().includes('connected')
            ? <CheckCircle size={12} />
            : <XCircle    size={12} />
          }
          {testResult}
        </div>
      )}

      {/* API Key input */}
      {!hasKey && (
        <div className="flex gap-2 items-center">
          <div className="flex-1 flex items-center relative">
            <input
              type={showKey ? 'text' : 'password'}
              placeholder={`${displayName} API Key`}
              value={key}
              onChange={e => setKey(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && handleSaveKey()}
              className="flex-1 font-mono text-xs px-3 py-1.5 rounded-sm"
              style={{
                background: 'var(--bg-surface)',
                border:     '1px solid var(--border-2)',
                color:      'var(--text-1)',
                paddingRight: '32px',
              }}
            />
            <button
              onClick={() => setShowKey(!showKey)}
              className="absolute right-2 border-none cursor-pointer"
              style={{ background: 'transparent', color: 'var(--text-3)' }}
            >
              {showKey ? <EyeOff size={12} /> : <Eye size={12} />}
            </button>
          </div>
          <button
            onClick={handleSaveKey}
            disabled={saving || !key.trim()}
            className="font-mono text-xs px-3 py-1.5 rounded-sm cursor-pointer disabled:opacity-40"
            style={{
              background: 'var(--accent-muted)',
              border:     '1px solid var(--accent-border)',
              color:      'var(--accent)',
            }}
          >
            {saving ? 'Saving…' : 'Save'}
          </button>
        </div>
      )}

      {/* Model selector */}
      {hasKey && provider.models.length > 0 && (
        <div>
          <div
            className="font-mono text-xs mb-1.5"
            style={{ color: 'var(--text-3)', fontSize: '10px' }}
          >
            MODEL
          </div>
          <div className="flex flex-wrap gap-1.5">
            {provider.models.map(model => {
              const isSelectedModel = isActive && store.activeModel === model.id
              return (
                <button
                  key={model.id}
                  onClick={() => handleSetActive(model.id)}
                  className="font-mono text-xs px-2 py-1 rounded-sm cursor-pointer"
                  style={{
                    background: isSelectedModel ? 'var(--accent-muted)' : 'var(--bg-surface)',
                    border:     `1px solid ${isSelectedModel ? 'var(--accent-border)' : 'var(--border-2)'}`,
                    color:      isSelectedModel ? 'var(--accent)' : 'var(--text-2)',
                  }}
                >
                  {model.name}
                </button>
              )
            })}
          </div>
        </div>
      )}
    </div>
  )
}

// ─── Model Config Section ─────────────────────────────────────────────────────

function ModelConfigSection({ modelId }: { modelId: string }) {
  const cfgStore  = useModelConfigStore()
  const [draft,   setDraft]   = useState(cfgStore.getConfig(modelId))
  const [saving,  setSaving]  = useState(false)
  const [saved,   setSaved]   = useState(false)

  useEffect(() => {
    cfgStore.loadConfig(modelId).then(cfg => setDraft(cfg))
  }, [modelId])

  const supportsThinking = THINKING_MODELS.has(modelId)
  const supportsEffort   = REASONING_EFFORT_MODELS.has(modelId)

  const handleSave = async () => {
    setSaving(true)
    try {
      await cfgStore.saveConfig(draft)
      setSaved(true)
      setTimeout(() => setSaved(false), 2000)
    } catch (e) {
      console.error('Model config save failed:', e)
    } finally {
      setSaving(false)
    }
  }

  const handleReset = async () => {
    await cfgStore.resetConfig(modelId)
    const fresh = cfgStore.getConfig(modelId)
    setDraft(fresh)
  }

  const label = (txt: string) => (
    <div className="font-mono text-xs mb-1" style={{ color: 'var(--text-3)', fontSize: '10px' }}>
      {txt}
    </div>
  )

  return (
    <div
      className="rounded-sm p-4 space-y-4"
      style={{ background: 'var(--bg-card)', border: '1px solid var(--border-1)' }}
    >
      <div className="flex items-center gap-2" style={{ color: 'var(--text-2)' }}>
        <Brain size={12} />
        <span className="font-mono text-xs">{modelId}</span>
      </div>

      {/* Temperature */}
      <div>
        {label(`TEMPERATURE  ${draft.temperature !== null ? draft.temperature.toFixed(2) : 'default'}`)}
        <div className="flex items-center gap-2">
          <input
            type="range" min={0} max={2} step={0.05}
            value={draft.temperature ?? 1.0}
            onChange={e => setDraft(d => ({ ...d, temperature: parseFloat(e.target.value) }))}
            className="flex-1"
            title="Temperature"
          />
          <button
            onClick={() => setDraft(d => ({ ...d, temperature: null }))}
            className="font-mono text-xs px-1.5 py-0.5 rounded-sm cursor-pointer"
            style={{ background: 'var(--bg-surface)', border: '1px solid var(--border-2)', color: 'var(--text-3)' }}
          >
            reset
          </button>
        </div>
      </div>

      {/* Top-P */}
      <div>
        {label(`TOP-P  ${draft.top_p !== null ? draft.top_p.toFixed(2) : 'default'}`)}
        <div className="flex items-center gap-2">
          <input
            type="range" min={0} max={1} step={0.01}
            value={draft.top_p ?? 1.0}
            onChange={e => setDraft(d => ({ ...d, top_p: parseFloat(e.target.value) }))}
            className="flex-1"
            title="Top-P"
          />
          <button
            onClick={() => setDraft(d => ({ ...d, top_p: null }))}
            className="font-mono text-xs px-1.5 py-0.5 rounded-sm cursor-pointer"
            style={{ background: 'var(--bg-surface)', border: '1px solid var(--border-2)', color: 'var(--text-3)' }}
          >
            reset
          </button>
        </div>
      </div>

      {/* Reasoning effort (o-series / Gemini thinking models) */}
      {supportsEffort && (
        <div>
          {label('REASONING EFFORT')}
          <div className="flex gap-1.5">
            {(['low', 'medium', 'high'] as const).map(level => (
              <button
                key={level}
                onClick={() => setDraft(d => ({ ...d, reasoning_effort: d.reasoning_effort === level ? null : level }))}
                className="font-mono text-xs px-3 py-1 rounded-sm cursor-pointer"
                style={{
                  background: draft.reasoning_effort === level ? 'var(--accent-muted)' : 'var(--bg-surface)',
                  border: `1px solid ${draft.reasoning_effort === level ? 'var(--accent-border)' : 'var(--border-2)'}`,
                  color: draft.reasoning_effort === level ? 'var(--accent)' : 'var(--text-2)',
                }}
              >
                {level}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Extended thinking toggle */}
      {supportsThinking && (
        <div>
          {label('EXTENDED THINKING')}
          <div className="flex items-center gap-3">
            <button
              onClick={() => setDraft(d => ({ ...d, thinking_enabled: !d.thinking_enabled }))}
              className="font-mono text-xs px-3 py-1 rounded-sm cursor-pointer"
              style={{
                background: draft.thinking_enabled ? 'var(--accent-muted)' : 'var(--bg-surface)',
                border: `1px solid ${draft.thinking_enabled ? 'var(--accent-border)' : 'var(--border-2)'}`,
                color: draft.thinking_enabled ? 'var(--accent)' : 'var(--text-2)',
              }}
            >
              {draft.thinking_enabled ? 'Enabled' : 'Disabled'}
            </button>
            {draft.thinking_enabled && (
              <div className="flex items-center gap-2 flex-1">
                <span className="font-mono text-xs" style={{ color: 'var(--text-3)', whiteSpace: 'nowrap' }}>
                  Budget: {(draft.thinking_budget ?? 5000).toLocaleString()} tokens
                </span>
                <input
                  type="range" min={1024} max={32000} step={256}
                  value={draft.thinking_budget ?? 5000}
                  onChange={e => setDraft(d => ({ ...d, thinking_budget: parseInt(e.target.value) }))}
                  className="flex-1"
                  title="Thinking token budget"
                />
              </div>
            )}
          </div>
        </div>
      )}

      {/* Actions */}
      <div className="flex gap-2 pt-1">
        <button
          onClick={handleReset}
          className="font-mono text-xs px-2 py-1 rounded-sm cursor-pointer"
          style={{ background: 'transparent', border: '1px solid var(--border-2)', color: 'var(--text-3)' }}
        >
          Reset to defaults
        </button>
        <button
          onClick={handleSave}
          disabled={saving}
          className="font-mono text-xs px-3 py-1 rounded-sm cursor-pointer disabled:opacity-40 flex items-center gap-1.5"
          style={{ background: 'var(--accent-muted)', border: '1px solid var(--accent-border)', color: 'var(--accent)' }}
        >
          {saving ? 'Saving…' : saved ? 'Saved!' : <><Save size={11} /> Save</>}
        </button>
      </div>
    </div>
  )
}
