// PA Mode Toggle — compact 3-option segmented control in the chat header.
// Controls which tools the PA agent has access to and which system prompt is used.
// See: DEVELOPMENT_PLAN.md Phase 2.1 — PA Mode Toggle

import { useSettingsStore, PA_MODES, type PAMode } from '../../stores/settingsStore'

export default function PaModeToggle() {
  const paMode   = useSettingsStore(s => s.paMode)
  const setPAMode = useSettingsStore(s => s.setPAMode)

  return (
    <div
      className="flex items-center gap-0.5 rounded shrink-0"
      style={{
        background:   'var(--bg-card)',
        border:       '1px solid var(--border-2)',
        padding:      '2px',
      }}
      title="PA Mode — controls tool availability"
    >
      {PA_MODES.map(m => (
        <button
          key={m.value}
          onClick={() => setPAMode(m.value as PAMode)}
          className="font-mono flex items-center gap-1 transition-colors rounded"
          style={{
            fontSize:    '10px',
            padding:     '2px 7px',
            background:  paMode === m.value ? 'var(--bg-surface)' : 'transparent',
            color:       paMode === m.value ? 'var(--text-1)' : 'var(--text-4)',
            border:      paMode === m.value ? '1px solid var(--border-2)' : '1px solid transparent',
            cursor:      'pointer',
            whiteSpace:  'nowrap',
          }}
          title={m.description}
        >
          <span>{m.icon}</span>
          <span>{m.label}</span>
        </button>
      ))}
    </div>
  )
}
