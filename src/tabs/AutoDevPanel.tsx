// AutoDev tab — stub for Phase 2 (Worker Agent mode + multi-agent coordination).
// Phase 1: placeholder panel with Phase 2 roadmap items.

export default function AutoDevTab() {
  return (
    <StubPanel
      title="AutoDev"
      badge="Phase 2"
      description="Worker agent mode with core-orchestrator delegation, A2A protocol, and semantic memory via mem0."
      items={[
        'Worker agent sessions via HTTP POST → core-orchestrator :4000',
        'A2A protocol for multi-agent coordination (Supervisor, QC, Worker)',
        'Semantic memory integration with mem0 vector store',
        '21 FSMs for task lifecycle management',
        'Context scoring and feedback synopsis',
        'AutoDev tool routing via __autodev suffix',
      ]}
    />
  )
}

function StubPanel({ title, badge, description, items }: {
  title: string
  badge: string
  description: string
  items: string[]
}) {
  return (
    <div
      className="flex-1 flex flex-col items-center justify-center p-8"
      style={{ color: 'var(--text-3)' }}
    >
      <div className="max-w-md space-y-4 text-center">
        <div className="flex items-center justify-center gap-2">
          <span className="font-mono text-xs" style={{ color: 'var(--text-1)' }}>
            {title}
          </span>
          <span
            className="font-mono px-1.5 py-0.5"
            style={{
              fontSize:   '10px',
              background: 'var(--bg-card)',
              border:     '1px solid var(--border-2)',
              color:      'var(--warning)',
              borderRadius: '2px',
            }}
          >
            {badge}
          </span>
        </div>

        <p className="text-sm leading-relaxed" style={{ color: 'var(--text-2)' }}>
          {description}
        </p>

        <div className="text-left space-y-2 mt-4">
          {items.map((item, i) => (
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
