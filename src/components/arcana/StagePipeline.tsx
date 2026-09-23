// StagePipeline — horizontal strip of task-lifecycle stage pills
// Stateless: receives stages[] as props, renders current state.

import type { Stage } from '../../types/live'

interface Props {
  stages: Stage[]
}

function formatMs(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`
  return `${Math.floor(ms / 60_000)}m ${String(Math.floor((ms % 60_000) / 1000)).padStart(2, '0')}s`
}

export function StagePipeline({ stages }: Props) {
  return (
    <div className="stage-pipeline">
      {stages.map((stage, i) => (
        <div key={stage.id} className="stage-pill-wrapper">
          <div
            className={`stage-pill stage-pill--${stage.status}`}
            title={
              stage.elapsed_ms != null
                ? `${stage.label} — ${formatMs(stage.elapsed_ms)}`
                : stage.label
            }
          >
            {stage.status === 'done'    && <span className="stage-icon">✓ </span>}
            {stage.status === 'error'   && <span className="stage-icon">✗ </span>}
            {stage.status === 'blocked' && <span className="stage-icon">⊘ </span>}
            {stage.label}
          </div>
          {i < stages.length - 1 && (
            <span className="stage-sep" aria-hidden>›</span>
          )}
        </div>
      ))}
    </div>
  )
}
