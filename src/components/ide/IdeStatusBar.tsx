// IDE status bar — small contextual line at the bottom of the IDE tab.
// Shows: active tab path, cursor position placeholder (reserved for future
// cursor-tracking hook), language, and file size.

import { useIdeStore } from '../../stores/ideStore'

export default function IdeStatusBar() {
  const tab = useIdeStore((s) =>
    s.openTabs.find((t) => t.relativePath === s.activeTab),
  )

  if (!tab) return null

  return (
    <div
      className="flex items-center justify-between px-3 py-1 text-[10px] font-mono border-t"
      style={{
        borderColor: 'var(--border-1)',
        background:  'var(--bg-surface)',
        color:       'var(--text-2)',
      }}
    >
      <div className="flex items-center gap-3 truncate">
        <span>{tab.relativePath}</span>
        {tab.content?.encoding && (
          <span style={{ color: 'var(--text-3)' }}>{tab.content.encoding}</span>
        )}
      </div>
      <div className="flex items-center gap-3">
        {tab.content && (
          <>
            <span>{tab.content.line_count} lines</span>
            <span>{formatBytes(tab.content.size_bytes)}</span>
            <span style={{ color: 'var(--accent)' }}>
              {tab.content.language}
            </span>
          </>
        )}
      </div>
    </div>
  )
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / (1024 * 1024)).toFixed(1)} MB`
}
