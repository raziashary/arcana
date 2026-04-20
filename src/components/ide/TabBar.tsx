// Tab bar — horizontal list of open files with close buttons + middle-click close.

import { useRef } from 'react'
import { X } from 'lucide-react'
import { useIdeStore } from '../../stores/ideStore'

export default function TabBar() {
  const openTabs  = useIdeStore((s) => s.openTabs)
  const activeTab = useIdeStore((s) => s.activeTab)
  const switchTab = useIdeStore((s) => s.switchTab)
  const closeTab  = useIdeStore((s) => s.closeTab)

  const scroller = useRef<HTMLDivElement>(null)

  if (openTabs.length === 0) return null

  return (
    <div
      className="flex overflow-x-auto border-b"
      style={{ borderColor: 'var(--border-1)', background: 'var(--bg-surface)' }}
    >
      <div
        ref={scroller}
        role="tablist"
        className="flex flex-1"
      >
        {openTabs.map((tab) => {
          const isActive = tab.relativePath === activeTab
          const fileName = tab.relativePath.split('/').pop() ?? tab.relativePath
          
          return (
            <div
              key={tab.relativePath}
              role="tab"
              {...(isActive && { 'aria-selected': 'true' })}
              {...(!isActive && { 'aria-selected': 'false' })}
              onClick={() => switchTab(tab.relativePath)}
              onMouseDown={(e: React.MouseEvent) => {
                if (e.button === 1) {
                  e.preventDefault()
                  closeTab(tab.relativePath)
                }
              }}
              className="flex items-center gap-1.5 pl-2.5 pr-1 py-1.5 cursor-pointer select-none border-r group"
              style={{
                borderColor: 'var(--border-1)',
                background: isActive ? 'var(--bg-card)' : undefined,
                borderBottom: isActive ? '1px solid var(--accent)' : '1px solid transparent',
              }}
              title={tab.relativePath}
            >
              <span
                className="text-[11px] font-mono whitespace-nowrap"
                style={{ color: isActive ? 'var(--text-1)' : 'var(--text-2)' }}
              >
                {fileName}
              </span>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation()
                  closeTab(tab.relativePath)
                }}
                className="p-0.5 rounded hover:bg-[var(--bg-hover)] inline-flex items-center focus:outline-none"
                style={{
                  opacity: isActive ? 0.7 : 0,
                }}
                onMouseEnter={(e) => {
                  (e.currentTarget as HTMLButtonElement).style.opacity = '1'
                }}
                onMouseLeave={(e) => {
                  (e.currentTarget as HTMLButtonElement).style.opacity = isActive ? '0.7' : '0'
                }}
                title="Close tab"
                aria-label={`Close ${fileName}`}
              >
                <X size={10} style={{ color: 'var(--text-3)' }} />
              </button>
            </div>
          )
        })}
      </div>
    </div>
  )
}
