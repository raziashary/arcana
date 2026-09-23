// TabBar — spec Section 2.2.
// Height 36px, active tab accent bottom border, Arcana task count badge, model + cost pills.

import type { TabId } from '../../App'
import { useSettingsStore } from '../../stores/settingsStore'
import { useCostStore }     from '../../stores/costStore'
import { useTaskStore }     from '../../stores/taskStore'

interface Tab { id: TabId; label: string }

interface Props {
  tabs:        Tab[]
  activeTab:   TabId
  onTabChange: (id: TabId) => void
}

export default function TabBar({ tabs, activeTab, onTabChange }: Props) {
  const activeTasks = useTaskStore(s =>
    s.tasks.filter(t => t.status === 'in_progress' || t.status === 'assigned').length
  )

  return (
    <div
      className="flex items-center justify-between shrink-0"
      style={{
        height:       '36px',
        background:   'var(--bg-surface)',
        borderBottom: '1px solid var(--border-1)',
        paddingLeft:  '4px',
      }}
    >
      <div className="flex h-full">
        {tabs.map(tab => {
          const badge = tab.id === 'arcana' && activeTasks > 0 ? activeTasks : null
          const isActive = activeTab === tab.id
          return (
            <button
              key={tab.id}
              onClick={() => onTabChange(tab.id)}
              className="relative font-mono flex items-center gap-1.5 h-full px-3.5 border-none cursor-pointer transition-colors duration-150"
              style={{
                background:    'transparent',
                color:         isActive ? 'var(--accent)' : 'var(--text-3)',
                borderBottom:  isActive ? '2px solid var(--accent)' : '2px solid transparent',
                letterSpacing: '0.04em',
                fontSize:      '11px',
              }}
            >
              {tab.label}
              {badge !== null && (
                <span
                  className="font-mono rounded-full flex items-center justify-center"
                  style={{
                    fontSize:   '9px',
                    background: 'var(--warning)',
                    color:      '#000',
                    minWidth:   '14px',
                    height:     '14px',
                    padding:    '0 3px',
                    lineHeight: 1,
                  }}
                >
                  {badge}
                </span>
              )}
            </button>
          )
        })}
      </div>

      {/* Right pills: model + cost */}
      <div className="flex items-center gap-1.5 pr-3">
        <ActiveModelBadge />
        <CostPill />
      </div>
    </div>
  )
}

function ActiveModelBadge() {
  const activeModel = useSettingsStore(s => s.activeModel)
  if (!activeModel) return null
  return (
    <span
      className="font-mono px-2 py-0.5"
      style={{
        background:   'var(--bg-card)',
        border:       '1px solid var(--border-2)',
        borderRadius: '2px',
        color:        'var(--text-3)',
        fontSize:     '10px',
      }}
    >
      {activeModel}
    </span>
  )
}

function CostPill() {
  const cost = useCostStore(s => s.costTodayUsd)
  if (cost === 0) return null
  const fmt = cost < 0.001
    ? `$${cost.toFixed(6)}`
    : cost < 1
      ? `$${cost.toFixed(4)}`
      : `$${cost.toFixed(2)}`
  return (
    <span
      className="font-mono px-2 py-0.5"
      style={{
        background:   'var(--bg-card)',
        border:       '1px solid var(--border-2)',
        borderRadius: '2px',
        color:        'var(--accent)',
        fontSize:     '10px',
      }}
    >
      {fmt}
    </span>
  )
}
