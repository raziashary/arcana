// SkillParameterModal — dynamic parameter form generated from a Skill's schema.
// Rendered inside the SkillsPalette when a skill is selected for execution.
// Phase 3.1

import { useState } from 'react'
import { X, Play, AlertCircle } from 'lucide-react'
import type { Skill, SkillParameter } from '../../stores/skillStore'

interface Props {
  skill:    Skill
  onExecute: (params: Record<string, unknown>) => Promise<void>
  onClose:  () => void
}

export default function SkillParameterModal({ skill, onExecute, onClose }: Props) {
  const [values,    setValues]    = useState<Record<string, unknown>>(() => buildDefaults(skill.parameters))
  const [errors,    setErrors]    = useState<Record<string, string>>({})
  const [executing, setExecuting] = useState(false)
  const [execError, setExecError] = useState<string | null>(null)

  function buildDefaults(params: SkillParameter[]): Record<string, unknown> {
    const defaults: Record<string, unknown> = {}
    for (const p of params) {
      if (p.default !== undefined) defaults[p.name] = p.default
    }
    return defaults
  }

  function setValue(name: string, value: unknown) {
    setValues(v => ({ ...v, [name]: value }))
    setErrors(e => { const next = { ...e }; delete next[name]; return next })
  }

  function validate(): boolean {
    const errs: Record<string, string> = {}
    for (const p of skill.parameters) {
      if (p.required && values[p.name] === undefined) {
        errs[p.name] = `${p.name} is required`
      }
      if (p.param_type === 'enum' && values[p.name] !== undefined) {
        if (!p.values?.includes(String(values[p.name]))) {
          errs[p.name] = `Must be one of: ${p.values?.join(', ')}`
        }
      }
    }
    setErrors(errs)
    return Object.keys(errs).length === 0
  }

  async function handleExecute() {
    if (!validate()) return
    setExecuting(true)
    setExecError(null)
    try {
      await onExecute(values)
      onClose()
    } catch (e) {
      setExecError(String(e))
    } finally {
      setExecuting(false)
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: 'rgba(0,0,0,0.6)' }}
      onClick={e => { if (e.target === e.currentTarget) onClose() }}
    >
      <div
        className="w-full max-w-lg rounded-lg shadow-xl flex flex-col"
        style={{ background: 'var(--bg-panel)', border: '1px solid var(--border)', maxHeight: '90vh' }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4" style={{ borderBottom: '1px solid var(--border)' }}>
          <div>
            <div className="font-mono text-sm font-semibold" style={{ color: 'var(--text-1)' }}>
              /{skill.name}
            </div>
            <div className="text-xs mt-0.5" style={{ color: 'var(--text-3)' }}>
              {skill.description}
            </div>
          </div>
          <button title="Close skill parameter modal" onClick={onClose} className="p-1 rounded hover:opacity-70">
            <X size={16} style={{ color: 'var(--text-3)' }} />
          </button>
        </div>

        {/* Parameter form */}
        <div className="overflow-y-auto px-5 py-4 space-y-4 flex-1">
          {skill.parameters.length === 0 ? (
            <p className="text-xs" style={{ color: 'var(--text-3)' }}>
              This skill has no parameters.
            </p>
          ) : (
            skill.parameters.map(param => (
              <ParamField
                key={param.name}
                param={param}
                value={values[param.name]}
                error={errors[param.name]}
                onChange={v => setValue(param.name, v)}
              />
            ))
          )}

          {execError && (
            <div className="flex items-start gap-2 text-xs p-3 rounded" style={{ background: 'rgba(255,99,99,0.1)', color: 'var(--danger)' }}>
              <AlertCircle size={14} className="mt-0.5 shrink-0" />
              <span>{execError}</span>
            </div>
          )}
        </div>

        {/* Footer */}
        <div
          className="flex items-center justify-end gap-3 px-5 py-4"
          style={{ borderTop: '1px solid var(--border)' }}
        >
          <button
            onClick={onClose}
            className="px-4 py-2 rounded text-xs font-mono"
            style={{ background: 'var(--bg-2)', color: 'var(--text-2)' }}
          >
            Cancel
          </button>
          <button
            onClick={handleExecute}
            disabled={executing}
            className="flex items-center gap-2 px-4 py-2 rounded text-xs font-mono font-semibold"
            style={{ background: 'var(--accent)', color: '#011627', opacity: executing ? 0.6 : 1 }}
          >
            <Play size={13} />
            {executing ? 'Executing…' : 'Execute'}
          </button>
        </div>
      </div>
    </div>
  )
}

// ── Single parameter field ─────────────────────────────────────────────────────

function ParamField({
  param, value, error, onChange,
}: {
  param:    SkillParameter
  value:    unknown
  error?:   string
  onChange: (v: unknown) => void
}) {
  const labelStyle = { color: 'var(--text-2)', fontSize: '11px', fontFamily: 'monospace' }
  const inputStyle = {
    background:  'var(--bg-input)',
    border:      `1px solid ${error ? 'var(--danger)' : 'var(--border)'}`,
    color:       'var(--text-1)',
    borderRadius: '4px',
    padding:     '6px 10px',
    fontSize:    '12px',
    width:       '100%',
    outline:     'none',
    fontFamily:  'monospace',
  }

  return (
    <div className="space-y-1">
      <div className="flex items-center gap-2">
        <label style={labelStyle}>{param.name}</label>
        {param.required && (
          <span className="text-xs" style={{ color: 'var(--danger)' }}>*</span>
        )}
        {!param.required && (
          <span className="text-xs" style={{ color: 'var(--text-3)' }}>optional</span>
        )}
      </div>

      {param.description && (
        <div className="text-xs" style={{ color: 'var(--text-3)' }}>{param.description}</div>
      )}

      {param.param_type === 'boolean' ? (
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={!!value}
            onChange={e => onChange(e.target.checked)}
            className="w-4 h-4"
            style={{ accentColor: 'var(--accent)' }}
          />
          <span className="text-xs" style={{ color: 'var(--text-2)' }}>
            {value ? 'Yes' : 'No'}
          </span>
        </label>
      ) : param.param_type === 'enum' && param.values ? (
        <select
          title={`Select ${param.name}`}
          value={String(value ?? '')}
          onChange={e => onChange(e.target.value)}
          style={{ ...inputStyle, paddingRight: '28px' }}
        >
          {!param.required && <option value="">— select —</option>}
          {param.values.map(v => (
            <option key={v} value={v}>{v}</option>
          ))}
        </select>
      ) : param.param_type === 'integer' ? (
        <input
          type="number"
          value={value !== undefined ? String(value) : ''}
          onChange={e => onChange(e.target.value === '' ? undefined : parseInt(e.target.value, 10))}
          style={inputStyle}
          placeholder={String(param.default ?? '')}
        />
      ) : (
        <textarea
          value={value !== undefined ? String(value) : ''}
          onChange={e => onChange(e.target.value)}
          rows={2}
          style={{ ...inputStyle, resize: 'vertical' }}
          placeholder={String(param.default ?? '')}
        />
      )}

      {error && (
        <div className="text-xs" style={{ color: 'var(--danger)' }}>{error}</div>
      )}
    </div>
  )
}
