// BreadcrumbNav — shows the active file's path as clickable path segments.
// Phase 3.2
//
// src / components / ide / BreadcrumbNav.tsx
// └─ each segment is clickable; folder segments expand that dir in the tree,
//    file segment does nothing (it's already open).

import { ChevronRight } from 'lucide-react'
import { useIdeStore } from '../../stores/ideStore'

interface Props {
  /** Relative path of the active tab, e.g. "src/components/ide/BreadcrumbNav.tsx" */
  path: string
}

export default function BreadcrumbNav({ path }: Props) {
  const toggleDir = useIdeStore((s) => s.toggleDir)

  if (!path) return null

  const parts = path.replace(/\\/g, '/').split('/')
  // Build cumulative paths: ["src", "src/components", "src/components/ide", ...]
  const crumbs = parts.map((part, i) => ({
    label:    part,
    path:     parts.slice(0, i + 1).join('/'),
    isFile:   i === parts.length - 1,
  }))

  async function handleClick(crumb: { path: string; isFile: boolean }) {
    if (crumb.isFile) {
      // Already active — no-op; could jump to top if desired
    } else {
      await toggleDir(crumb.path)
    }
  }

  return (
    <div
      className="flex items-center px-3 py-0.5 overflow-x-auto"
      style={{
        borderBottom: '1px solid var(--border-1)',
        background:   'var(--bg-surface)',
        flexShrink:   0,
        gap:          0,
        minHeight:    '22px',
      }}
    >
      {crumbs.map((crumb, i) => (
        <span key={crumb.path} className="flex items-center">
          {i > 0 && (
            <ChevronRight
              size={10}
              style={{ color: 'var(--text-3)', flexShrink: 0, margin: '0 1px' }}
            />
          )}
          <button
            onClick={() => void handleClick(crumb)}
            className="text-[11px] font-mono whitespace-nowrap hover:underline"
            style={{
              color:  crumb.isFile ? 'var(--text-1)' : 'var(--text-3)',
              cursor: crumb.isFile ? 'default'       : 'pointer',
            }}
          >
            {crumb.label}
          </button>
        </span>
      ))}
    </div>
  )
}
