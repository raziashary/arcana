// Tab bar — horizontal list of open files with close + middle-click close.
// Phase 3.2 — now supports drag-drop tab reordering.

import { useRef } from 'react'
import { X } from 'lucide-react'
import { useIdeStore } from '../../stores/ideStore'

export default function TabBar() {
  const openTabs   = useIdeStore((s) => s.openTabs)
  const activeTab  = useIdeStore((s) => s.activeTab)
  const switchTab  = useIdeStore((s) => s.switchTab)
  const closeTab   = useIdeStore((s) => s.closeTab)
  const reorderTabs = useIdeStore((s) => s.reorderTabs)

  const scroller  = useRef<HTMLDivElement>(null)
  const dragIdx   = useRef<number | null>(null)

  if (openTabs.length === 0) return null

  function handleDragStart(idx: number) {
    dragIdx.current = idx
  }

  function handleDragOver(e: React.DragEvent, idx: number) {
    e.preventDefault()
    e.dataTransfer.dropEffect = 'move'
  }

  function handleDrop(e: React.DragEvent, toIdx: number) {
    e.preventDefault()
    const from = dragIdx.current
    dragIdx.current = null
    if (from === null || from === toIdx) return
    reorderTabs(from, toIdx)
  }

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
        {openTabs.map((tab, idx) => {
          const isActive = tab.relativePath === activeTab
          const fileName = tab.relativePath.split('/').pop() ?? tab.relativePath

          return (
            <div
              key={tab.relativePath}
              role="tab"
              draggable
              aria-selected={isActive}
              onClick={() => switchTab(tab.relativePath)}
              onMouseDown={(e: React.MouseEvent) => {
                if (e.button === 1) {
                  e.preventDefault()
                  closeTab(tab.relativePath)
                }
              }}
              onDragStart={() => handleDragStart(idx)}
              onDragOver={e  => handleDragOver(e, idx)}
              onDrop={e      => handleDrop(e, idx)}
              className="flex items-center gap-1.5 pl-2.5 pr-1 py-1.5 cursor-pointer select-none border-r group"
              style={{
                borderColor:  'var(--border-1)',
                background:   isActive ? 'var(--bg-card)' : undefined,
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
                style={{ opacity: isActive ? 0.7 : 0 }}
                onMouseEnter={(e) => { (e.currentTarget as HTMLButtonElement).style.opacity = '1' }}
                onMouseLeave={(e) => { (e.currentTarget as HTMLButtonElement).style.opacity = isActive ? '0.7' : '0' }}
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
