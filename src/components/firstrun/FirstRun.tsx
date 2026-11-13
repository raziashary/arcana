// First run (P7-01..04, D-018): add a provider key, pick a workspace folder, choose a
// spend limit, start a session. Shown in the Chat tab while no session exists and the
// setup is incomplete.
//
// The key is typed into a password field, sent once to the backend (which checks it
// with the provider before storing it), and cleared. It is never rendered back, never
// put in a message, and no "show" toggle exists. Every failure says what to do next.

import { useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useSettingsStore, type ProviderInfo } from '../../stores/settingsStore'
import { useWorkspaceStore } from '../../stores/workspaceStore'
import { useSessionStore } from '../../stores/sessionStore'

export const DEFAULT_CAP_USD = 50

/** True when a key is stored for some provider and a workspace folder is chosen. */
export function isSetupComplete(providers: ProviderInfo[], workspaceDir: string): boolean {
  return providers.some(p => p.has_key) && workspaceDir.trim().length > 0
}

/** What the user can do about a failed key save, in one sentence. The message never contains the key. */
export function keyFailureMessage(providerName: string, raw: string, key: string): string {
  const scrubbed = key ? raw.split(key).join('••••') : raw
  const why = scrubbed.trim() || 'The key could not be saved.'
  const rejected = /not accepted|NOT been saved|did not confirm/i.test(why)
  const next = rejected
    ? `Copy the whole key again from ${providerName}'s console and check that it was created for ${providerName}.`
    : 'Try again, or add the key later under Settings, Providers.'
  return `${why} ${next}`
}

type Tone = 'ok' | 'warn' | 'error'

const toneColour: Record<Tone, string> = {
  ok: 'var(--success)',
  warn: 'var(--warning)',
  error: 'var(--danger)',
}

function StepHeader({ n, title, done }: { n: number; title: string; done: boolean }) {
  return (
    <div className="flex items-center gap-2" style={{ marginBottom: 8 }}>
      <span
        aria-hidden
        style={{
          width: 20, height: 20, borderRadius: 999, display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
          fontSize: 11, fontWeight: 500,
          background: done ? 'var(--success)' : 'var(--bg-2)',
          color: done ? 'var(--text-on-accent)' : 'var(--text-secondary)',
        }}
      >
        {done ? '✓' : n}
      </span>
      <span style={{ fontSize: 13, fontWeight: 500, color: 'var(--text-primary)' }}>{title}</span>
    </div>
  )
}

export default function FirstRun() {
  const providers = useSettingsStore(s => s.providers)
  const storeProviderKey = useSettingsStore(s => s.storeProviderKey)
  const setActiveProvider = useSettingsStore(s => s.setActiveProvider)
  const workspaceDir = useWorkspaceStore(s => s.workspaceDir)
  const pickWorkspaceDir = useWorkspaceStore(s => s.pickWorkspaceDir)
  const loadWorkspaceDir = useWorkspaceStore(s => s.loadWorkspaceDir)
  const workspaceError = useWorkspaceStore(s => s.error)
  const createSession = useSessionStore(s => s.createSession)
  const switchSession = useSessionStore(s => s.switchSession)

  const [providerId, setProviderId] = useState<string>('')
  const [key, setKey] = useState('')
  const [checking, setChecking] = useState(false)
  const [keyNote, setKeyNote] = useState<{ tone: Tone; text: string } | null>(null)
  const [capOn, setCapOn] = useState(true)
  const [capUsd, setCapUsd] = useState(DEFAULT_CAP_USD)
  const [starting, setStarting] = useState(false)
  const [startError, setStartError] = useState<string | null>(null)

  useEffect(() => { void loadWorkspaceDir() }, [loadWorkspaceDir])

  const selected = useMemo(
    () => providers.find(p => p.id === providerId) ?? providers.find(p => p.has_key) ?? providers[0],
    [providers, providerId],
  )
  if (!selected) return null

  const hasKey = providers.some(p => p.has_key)
  const keyedProvider = providers.find(p => p.has_key) ?? selected
  const defaultModel = (p: ProviderInfo) => p.models[1]?.id ?? p.models[0]?.id ?? ''

  const saveKey = async () => {
    const entered = key.trim()
    if (!entered || checking) return
    setChecking(true)
    setKeyNote(null)
    try {
      const { validated } = await storeProviderKey(selected.id, entered)
      const model = defaultModel(selected)
      if (model) await setActiveProvider(selected.id, model)
      setKey('')
      setKeyNote(
        validated
          ? { tone: 'ok', text: `${selected.name} accepted the key.` }
          : {
              tone: 'warn',
              text: `Saved, but ${selected.name} could not be reached to check it. Your first message will show whether it works.`,
            },
      )
    } catch (e) {
      const raw = e instanceof Error ? e.message : String(e)
      setKeyNote({ tone: 'error', text: keyFailureMessage(selected.name, raw, entered) })
    } finally {
      setChecking(false)
    }
  }

  const start = async () => {
    setStarting(true)
    setStartError(null)
    try {
      if (capOn) {
        try {
          await invoke('set_daily_cap', { enabled: true, limitUsd: capUsd })
        } catch (e) {
          const raw = e instanceof Error ? e.message : String(e)
          setStartError(`The spend limit was not saved: ${raw} Change the limit above, or untick it, then press Start again.`)
          return
        }
      }
      try {
        const id = await createSession(workspaceDir, keyedProvider.id, defaultModel(keyedProvider))
        switchSession(id)
      } catch {
        // `createSession` already raised a toast with the reason; this says what to do about it.
        setStartError('The session could not be created. Pick a different workspace folder above, then press Start again.')
      }
    } finally {
      setStarting(false)
    }
  }

  const ready = hasKey && workspaceDir.trim().length > 0 && (!capOn || (Number.isFinite(capUsd) && capUsd > 0))

  const field: React.CSSProperties = {
    background: 'var(--bg-input)', border: '1px solid var(--border-secondary)', borderRadius: 'var(--radius)',
    color: 'var(--text-primary)', fontSize: 13, padding: '7px 10px', width: '100%',
  }
  const primary: React.CSSProperties = {
    background: 'var(--accent)', color: 'var(--text-on-accent)', border: 'none', borderRadius: 'var(--radius)',
    fontSize: 13, padding: '7px 14px', cursor: 'pointer',
  }

  return (
    <div className="flex flex-1 items-center justify-center overflow-y-auto" style={{ padding: 24 }}>
      <div style={{ width: '100%', maxWidth: 520 }}>
        <h1 style={{ fontSize: 28, fontWeight: 500, letterSpacing: '-0.02em', color: 'var(--text-primary)', lineHeight: 1.15 }}>
          Welcome to Arcana
        </h1>
        <p style={{ marginTop: 6, marginBottom: 22, fontSize: 14, lineHeight: 1.5, color: 'var(--text-muted)' }}>
          Arcana runs on your own provider key. It goes into the Windows credential vault and is never shown again.
        </p>

        {/* 1 — key */}
        <section style={{ marginBottom: 20 }}>
          <StepHeader n={1} title="Add a provider key" done={hasKey} />
          {hasKey ? (
            <p style={{ fontSize: 13, color: 'var(--text-muted)' }}>
              A {keyedProvider.name} key is stored.{' '}
              <span style={{ color: 'var(--text-dim)' }}>Add or change keys later under Settings, Providers.</span>
            </p>
          ) : (
            <div className="flex flex-col" style={{ gap: 8 }}>
              <select
                aria-label="Provider"
                value={selected.id}
                onChange={e => { setProviderId(e.target.value); setKeyNote(null) }}
                style={field}
              >
                {providers.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
              </select>
              <div className="flex" style={{ gap: 8 }}>
                <input
                  type="password"
                  aria-label={`${selected.name} API key`}
                  placeholder={`${selected.name} API key`}
                  autoComplete="off"
                  spellCheck={false}
                  value={key}
                  onChange={e => setKey(e.target.value)}
                  onKeyDown={e => e.key === 'Enter' && void saveKey()}
                  style={field}
                />
                <button onClick={() => void saveKey()} disabled={!key.trim() || checking} style={{ ...primary, opacity: !key.trim() || checking ? 0.5 : 1, whiteSpace: 'nowrap' }}>
                  {checking ? 'Checking…' : 'Verify and save'}
                </button>
              </div>
            </div>
          )}
          {keyNote && (
            <p role={keyNote.tone === 'error' ? 'alert' : 'status'} style={{ marginTop: 8, fontSize: 12, lineHeight: 1.5, color: toneColour[keyNote.tone] }}>
              {keyNote.text}
            </p>
          )}
        </section>

        {/* 2 — folder */}
        <section style={{ marginBottom: 20 }}>
          <StepHeader n={2} title="Choose a workspace folder" done={workspaceDir.trim().length > 0} />
          <div className="flex items-center" style={{ gap: 8 }}>
            <div style={{ ...field, color: workspaceDir ? 'var(--text-primary)' : 'var(--text-dim)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={workspaceDir || undefined}>
              {workspaceDir || 'No folder selected'}
            </div>
            <button onClick={() => void pickWorkspaceDir()} style={{ ...primary, background: 'var(--bg-2)', color: 'var(--text-primary)', whiteSpace: 'nowrap' }}>
              {workspaceDir ? 'Change' : 'Select folder'}
            </button>
          </div>
          <p style={{ marginTop: 6, fontSize: 12, color: 'var(--text-dim)' }}>Arcana reads and edits files only inside this folder.</p>
          {workspaceError && <p role="alert" style={{ marginTop: 6, fontSize: 12, color: 'var(--danger)' }}>{workspaceError} Pick a different folder.</p>}
        </section>

        {/* 3 — spend limit */}
        <section style={{ marginBottom: 22 }}>
          <StepHeader n={3} title="Set a daily spend limit" done={false} />
          <label className="flex items-center" style={{ gap: 8, fontSize: 13, fontWeight: 400, color: 'var(--text-primary)' }}>
            <input type="checkbox" checked={capOn} onChange={e => setCapOn(e.target.checked)} />
            Stop all spending once a day reaches
            <span style={{ color: 'var(--text-muted)' }}>$</span>
            <input
              type="number" min={1} step={1} aria-label="Daily spend limit in US dollars"
              value={capUsd} disabled={!capOn}
              onChange={e => setCapUsd(Number(e.target.value))}
              style={{ ...field, width: 72, padding: '3px 6px' }}
            />
          </label>
          <p style={{ marginTop: 6, fontSize: 12, color: 'var(--text-dim)' }}>
            {capOn
              ? 'Arcana refuses further model calls at the limit and pauses workers. Change or turn it off under Settings, Daily Spend Ceiling.'
              : 'No limit: a long run can spend without stopping. You can turn the limit on later under Settings, Daily Spend Ceiling.'}
          </p>
        </section>

        <button onClick={() => void start()} disabled={!ready || starting} style={{ ...primary, opacity: !ready || starting ? 0.5 : 1 }}>
          {starting ? 'Starting…' : 'Start'}
        </button>
        {!ready && !starting && (
          <span style={{ marginLeft: 10, fontSize: 12, color: 'var(--text-dim)' }}>
            {!hasKey ? 'Add a key first.' : !workspaceDir.trim() ? 'Choose a folder first.' : 'Enter a limit above zero.'}
          </span>
        )}
        {startError && <p role="alert" style={{ marginTop: 10, fontSize: 12, lineHeight: 1.5, color: 'var(--danger)' }}>{startError}</p>}
      </div>
    </div>
  )
}
