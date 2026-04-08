// TabBar — horizontal tab navigation matching autodevmockup.html .tabbar

import type { TabId } from '../../App'
import { useSettingsStore } from '../../stores/settingsStore'

interface Tab { id: TabId; label: string }

interface Props {
  tabs:        Tab[]
  activeTab:   TabId
  onTabChange: (id: TabId) => void
}

export default function TabBar({ tabs, activeTab, onTabChange }: Props) {
  return (
    <div
      className="flex items-center justify-between px-3.5 shrink-0"
      style={{
        background:   'var(--bg-surface)',
        borderBottom: '1px solid var(--border-1)',
      }}
    >
      <div className="flex">
        {tabs.map(tab => (
          <button
            key={tab.id}
            onClick={() => onTabChange(tab.id)}
            className="font-mono text-xs px-4 py-2.5 border-none cursor-pointer transition-colors duration-150"
            style={{
              background:   'transparent',
              color:        activeTab === tab.id ? 'var(--accent)' : 'var(--text-2)',
              borderBottom: activeTab === tab.id
                ? '2px solid var(--accent)'
                : '2px solid transparent',
              letterSpacing: '0.03em',
            }}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Right-side status chip */}
      <div
        className="font-mono flex items-center gap-2"
        style={{ fontSize: '11px', color: 'var(--text-3)' }}
      >
        <ActiveModelBadge />
      </div>
    </div>
  )
}

function ActiveModelBadge() {
  const activeModel = useSettingsStore(s => s.activeModel)
  if (!activeModel) return null
  return (
    <span
      className="font-mono px-2 py-0.5 rounded-sm"
      style={{
        background: 'var(--bg-card)',
        border:     '1px solid var(--border-2)',
        color:      'var(--text-2)',
        fontSize:   '10px',
      }}
    >
      {activeModel}
    </span>
  )
}
