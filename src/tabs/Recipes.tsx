// Recipes tab — stub for Phase 4 (shareable workflow definitions).
// Phase 1: placeholder panel.

export default function RecipesTab() {
  return (
    <div
      className="flex-1 flex flex-col items-center justify-center p-8"
      style={{ color: 'var(--text-3)' }}
    >
      <div className="max-w-md space-y-4 text-center">
        <span className="font-mono text-xs" style={{ color: 'var(--text-1)' }}>
          Recipes
        </span>
        <p className="text-sm leading-relaxed" style={{ color: 'var(--text-2)' }}>
          Shareable YAML workflow definitions. Each recipe specifies agent roles, tools, context, and execution mode. Implemented in Phase 4.
        </p>
        <div className="space-y-2 text-left">
          {[
            'YAML recipe definitions with execution_mode field',
            'Recipe library browser with search and tags',
            'Run recipe → spawns Worker agent via core-orchestrator',
            'Progress view with live agent status + FSM state',
            'Recipe sharing and import from AIRS Foundry registry',
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
