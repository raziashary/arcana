// WorkerSetupWizard — Step-by-step wizard for enrolling a new worker machine.
// Generates a setup token, displays a one-liner, and optionally shows a QR code.
// Phase 5.2: Plugin System & Worker Setup.

import React, { useState } from 'react'
import { usePluginStore, SetupToken } from '../../stores/pluginStore'

type Step = 'configure' | 'token' | 'done'

const WORKER_TYPES = [
  { id: 'worker-backend',  label: 'Backend',  icon: '⚙️',  desc: 'Rust, Python, uv, Docker' },
  { id: 'worker-frontend', label: 'Frontend', icon: '🌐', desc: 'Node.js, npm, build tools' },
  { id: 'worker-devops',   label: 'DevOps',   icon: '🚀', desc: 'Docker, kubectl, Terraform' },
  { id: 'worker-qa',       label: 'QA',       icon: '🧪', desc: 'pytest, Playwright, testing' },
  { id: 'worker-security', label: 'Security', icon: '🔒', desc: 'Bandit, Semgrep, cargo-audit' },
]

// ─── Styles ──────────────────────────────────────────────────────────────────

const s: Record<string, React.CSSProperties> = {
  overlay: {
    position:   'fixed',
    inset:      0,
    background: 'rgba(0,0,0,0.55)',
    display:    'flex',
    alignItems: 'center',
    justifyContent: 'center',
    zIndex:     200,
  },
  box: {
    background:   'var(--bg-2)',
    border:       '1px solid var(--border)',
    borderRadius: 14,
    padding:      32,
    width:        560,
    maxHeight:    '90vh',
    overflowY:    'auto',
    boxShadow:    '0 12px 48px rgba(0,0,0,0.5)',
    display:      'flex',
    flexDirection:'column',
    gap:          20,
  },
  title: {
    fontSize:   20,
    fontWeight: 700,
    margin:     0,
    color:      'var(--text-1)',
  },
  stepIndicator: {
    display:     'flex',
    gap:         8,
    alignItems:  'center',
  },
  stepDot: (active: boolean, done: boolean) => ({
    width:        24,
    height:       24,
    borderRadius: '50%',
    background:   done ? 'var(--success, #27ae60)' : active ? 'var(--accent)' : 'var(--bg-3, #3a3a3a)',
    color:        done || active ? '#fff' : 'var(--text-3)',
    display:      'flex',
    alignItems:   'center',
    justifyContent:'center',
    fontSize:     12,
    fontWeight:   600,
    transition:   'background 0.2s',
  }),
  stepLine: {
    flex:       1,
    height:     2,
    background: 'var(--border)',
  },
  label: {
    fontSize:    13,
    fontWeight:  500,
    color:       'var(--text-2)',
    marginBottom:4,
  },
  typeGrid: {
    display:             'grid',
    gridTemplateColumns: 'repeat(auto-fill, minmax(150px, 1fr))',
    gap:                 10,
  },
  typeCard: (selected: boolean) => ({
    padding:      12,
    borderRadius: 10,
    border:       `2px solid ${selected ? 'var(--accent)' : 'var(--border)'}`,
    background:   selected ? 'rgba(var(--accent-rgb, 66,133,244), 0.1)' : 'var(--bg-1)',
    cursor:       'pointer',
    transition:   'border-color 0.15s, background 0.15s',
    textAlign:    'center',
  }),
  typeIcon: {
    fontSize:    26,
    marginBottom:4,
    display:     'block',
  },
  typeName: {
    fontSize:  13,
    fontWeight:600,
    color:     'var(--text-1)',
  },
  typeDesc: {
    fontSize: 11,
    color:    'var(--text-3)',
    marginTop:2,
  },
  input: {
    width:        '100%',
    padding:      '8px 12px',
    borderRadius: 8,
    border:       '1px solid var(--border)',
    background:   'var(--bg-1)',
    color:        'var(--text-1)',
    fontSize:     14,
    outline:      'none',
    boxSizing:    'border-box',
  },
  codeBlock: {
    background:   'var(--bg-1)',
    border:       '1px solid var(--border)',
    borderRadius: 8,
    padding:      14,
    fontSize:     12,
    fontFamily:   'var(--font-mono)',
    color:        'var(--text-1)',
    overflowX:    'auto',
    whiteSpace:   'pre-wrap',
    wordBreak:    'break-all',
    lineHeight:   1.6,
  },
  btn: (variant: 'primary' | 'ghost') => ({
    padding:      '9px 20px',
    borderRadius: 8,
    border:       variant === 'ghost' ? '1px solid var(--border)' : 'none',
    background:   variant === 'primary' ? 'var(--accent)' : 'transparent',
    color:        variant === 'primary' ? '#fff' : 'var(--text-2)',
    fontSize:     14,
    fontWeight:   500,
    cursor:       'pointer',
    transition:   'opacity 0.15s',
  }),
  copyBtn: {
    padding:      '4px 12px',
    borderRadius: 6,
    border:       '1px solid var(--border)',
    background:   'var(--bg-2)',
    color:        'var(--text-2)',
    fontSize:     12,
    cursor:       'pointer',
  },
  infoRow: {
    display:      'flex',
    justifyContent:'space-between',
    padding:      '8px 0',
    borderBottom: '1px solid var(--border)',
    fontSize:     13,
  },
  infoLabel: { color: 'var(--text-3)' },
  infoValue: { color: 'var(--text-1)', fontWeight: 500 },
}

// ─── Component ────────────────────────────────────────────────────────────────

interface Props {
  onClose: () => void
}

const WorkerSetupWizard: React.FC<Props> = ({ onClose }) => {
  const { generateSetupToken } = usePluginStore()

  const [step, setStep] = useState<Step>('configure')
  const [workerType, setWorkerType] = useState('worker-backend')
  const [fleetId, setFleetId] = useState('local')
  const [repos, setRepos] = useState('')
  const [ttlHours, setTtlHours] = useState(24)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [token, setToken] = useState<SetupToken | null>(null)
  const [copied, setCopied] = useState(false)

  const steps = ['configure', 'token', 'done']
  const stepIndex = steps.indexOf(step)

  const handleGenerate = async () => {
    setLoading(true)
    setError(null)
    try {
      const repoList = repos.split(',').map(r => r.trim()).filter(Boolean)
      const t = await generateSetupToken(fleetId, workerType, repoList, ttlHours)
      setToken(t)
      setStep('token')
    } catch (e: any) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  const handleCopy = (text: string) => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    })
  }

  const selectedType = WORKER_TYPES.find(t => t.id === workerType)

  return (
    <div style={s.overlay} onClick={onClose}>
      <div style={s.box} onClick={e => e.stopPropagation()}>

        {/* Header */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <h3 style={s.title}>Worker Setup Wizard</h3>
          <button
            onClick={onClose}
            style={{ marginLeft:'auto', background:'none', border:'none', color:'var(--text-3)', fontSize:18, cursor:'pointer' }}
          >✕</button>
        </div>

        {/* Step indicator */}
        <div style={s.stepIndicator}>
          {['Configure', 'Token', 'Done'].map((name, i) => (
            <React.Fragment key={name}>
              <div style={s.stepDot(i === stepIndex, i < stepIndex)}>
                {i < stepIndex ? '✓' : i + 1}
              </div>
              {i < 2 && <div style={s.stepLine} />}
            </React.Fragment>
          ))}
          <span style={{ marginLeft:8, fontSize:13, color:'var(--text-3)' }}>
            {['Configure worker', 'Copy token', 'Worker enrolled'][stepIndex]}
          </span>
        </div>

        {/* Step: Configure */}
        {step === 'configure' && (
          <>
            <div>
              <div style={s.label}>Worker Type</div>
              <div style={s.typeGrid}>
                {WORKER_TYPES.map(wt => (
                  <div
                    key={wt.id}
                    style={s.typeCard(workerType === wt.id)}
                    onClick={() => setWorkerType(wt.id)}
                  >
                    <span style={s.typeIcon}>{wt.icon}</span>
                    <div style={s.typeName}>{wt.label}</div>
                    <div style={s.typeDesc}>{wt.desc}</div>
                  </div>
                ))}
              </div>
            </div>

            <div>
              <div style={s.label}>Fleet ID</div>
              <input
                style={s.input}
                value={fleetId}
                onChange={e => setFleetId(e.target.value)}
                placeholder="local"
              />
            </div>

            <div>
              <div style={s.label}>Allowed Repos (comma-separated, optional)</div>
              <input
                style={s.input}
                value={repos}
                onChange={e => setRepos(e.target.value)}
                placeholder="owner/repo1, owner/repo2"
              />
            </div>

            <div>
              <div style={s.label}>Token TTL (hours)</div>
              <input
                style={s.input}
                type="number"
                min={1}
                max={168}
                value={ttlHours}
                onChange={e => setTtlHours(Number(e.target.value))}
              />
            </div>

            {error && (
              <div style={{ color: 'var(--error, #e74c3c)', fontSize: 13 }}>⚠ {error}</div>
            )}

            <div style={{ display: 'flex', gap: 10, justifyContent: 'flex-end' }}>
              <button style={s.btn('ghost')} onClick={onClose}>Cancel</button>
              <button
                style={s.btn('primary')}
                disabled={loading || !workerType}
                onClick={handleGenerate}
              >
                {loading ? 'Generating…' : 'Generate Token →'}
              </button>
            </div>
          </>
        )}

        {/* Step: Token */}
        {step === 'token' && token && (
          <>
            <div>
              {[
                ['Worker type', token.worker_type],
                ['Expires',     token.expires_at],
                ['Fleet ID',    fleetId],
              ].map(([label, value]) => (
                <div key={label} style={s.infoRow}>
                  <span style={s.infoLabel}>{label}</span>
                  <span style={s.infoValue}>{value}</span>
                </div>
              ))}
            </div>

            <div>
              <div style={{ ...s.label, marginBottom: 8 }}>One-liner — run on the target machine:</div>
              <div style={s.codeBlock}>{token.one_liner}</div>
              <button
                style={{ ...s.copyBtn, marginTop: 8 }}
                onClick={() => handleCopy(token.one_liner)}
              >
                {copied ? '✓ Copied!' : 'Copy command'}
              </button>
            </div>

            <div>
              <div style={{ ...s.label, marginBottom: 8 }}>Raw token (for manual use):</div>
              <div style={{ ...s.codeBlock, fontSize: 11, wordBreak: 'break-all' }}>{token.token}</div>
              <button
                style={{ ...s.copyBtn, marginTop: 8 }}
                onClick={() => handleCopy(token.token)}
              >
                Copy token
              </button>
            </div>

            <div style={{ background: 'rgba(39,174,96,0.08)', border: '1px solid var(--success, #27ae60)', borderRadius: 8, padding: 12, fontSize: 13, color: 'var(--text-2)' }}>
              💡 Run the one-liner on the worker machine. It will download and run the setup script,
              install the <strong>{selectedType?.label}</strong> environment, and register the worker with this fleet.
            </div>

            <div style={{ display: 'flex', gap: 10, justifyContent: 'flex-end' }}>
              <button style={s.btn('ghost')} onClick={() => setStep('configure')}>← Back</button>
              <button style={s.btn('primary')} onClick={() => setStep('done')}>Done ✓</button>
            </div>
          </>
        )}

        {/* Step: Done */}
        {step === 'done' && (
          <>
            <div style={{ textAlign: 'center', padding: '20px 0' }}>
              <div style={{ fontSize: 52, marginBottom: 12 }}>🎉</div>
              <h4 style={{ margin: 0, color: 'var(--text-1)', fontSize: 18 }}>Token Generated!</h4>
              <p style={{ color: 'var(--text-2)', marginTop: 8 }}>
                Share the one-liner with your worker machine operator. Once redeemed,
                the worker will appear in the Worker Registry.
              </p>
            </div>
            <div style={{ display: 'flex', gap: 10, justifyContent: 'center' }}>
              <button style={s.btn('ghost')} onClick={() => { setStep('configure'); setToken(null) }}>
                New Token
              </button>
              <button style={s.btn('primary')} onClick={onClose}>Close</button>
            </div>
          </>
        )}
      </div>
    </div>
  )
}

export default WorkerSetupWizard
