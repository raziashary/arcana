// IDE tab — stub for Phase 3 (IDE integration, file tree, git sub-panel).
// Phase 1: placeholder panel.

export default function IDETab() {
  return (
    <div
      className="flex-1 flex flex-col items-center justify-center p-8"
      style={{ color: 'var(--text-3)' }}
    >
      <div className="max-w-md space-y-4 text-center">
        <span className="font-mono text-xs" style={{ color: 'var(--text-1)' }}>
          IDE
        </span>
        <p className="text-sm leading-relaxed" style={{ color: 'var(--text-2)' }}>
          File tree, editor preview, and git sub-panel. Implemented in Phase 3.
        </p>
        <div className="space-y-2 text-left">
          {[
            'File tree with expand/collapse and edit actions',
            'Agent-driven file viewer (read_file output rendering)',
            'Git status sub-panel (staged, unstaged, untracked)',
            'Clone repos from AIRS Foundry within this tab',
            'git2-rs worktree isolation per task (.autodev/worktrees/)',
          ].map((item, i) => (
            <div key={i} className="flex gap-2 text-xs font-mono" style={{ color: 'var(--text-4)' }}>
              <span style={{ color: 'var(--text-3)' }}>—</span>
              <span>{item}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
