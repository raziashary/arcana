// SubagentResultCard — Phase 4.9 frontend.
//
// Compact summary card shown after a subagent completes. Displays the
// final result, files modified, cost / token usage, runtime, and (on
// failure) the error message. Used inline in chat as well as in the
// SubagentMonitor's detail pane.

import React, { useState } from 'react';
import { SubagentResult } from '../../stores/subagentStore';
import './SubagentResultCard.css';

interface SubagentResultCardProps {
  result: SubagentResult;
  onDismiss?: () => void;
  onViewLog?: () => void;
}

export const SubagentResultCard: React.FC<SubagentResultCardProps> = ({
  result,
  onDismiss,
  onViewLog,
}) => {
  const [expanded, setExpanded] = useState(false);
  const isError = result.status === 'failed' || result.status === 'terminated';

  return (
    <div className={`subagent-result-card status-${result.status}`}>
      <div className="subagent-result-card-header">
        <span className="subagent-result-card-icon">
          {isError ? '❌' : '✅'}
        </span>
        <span className="subagent-result-card-title">
          Subagent {isError ? 'failed' : 'completed'}
        </span>
        <span className="subagent-result-card-id" title={result.subagent_id}>
          {shortId(result.subagent_id)}
        </span>
        <span className="subagent-result-card-spacer" />
        {onDismiss && (
          <button
            className="subagent-result-card-dismiss"
            onClick={onDismiss}
            title="Dismiss"
          >
            ✕
          </button>
        )}
      </div>

      {isError && result.error && (
        <div className="subagent-result-card-error">
          <strong>Error:</strong> {result.error}
        </div>
      )}

      {!isError && (
        <div className="subagent-result-card-summary">
          {expanded
            ? result.result_summary
            : truncate(result.result_summary, 300)}
          {result.result_summary.length > 300 && (
            <button
              className="subagent-result-card-expand"
              onClick={() => setExpanded((e) => !e)}
            >
              {expanded ? 'Show less' : 'Show more'}
            </button>
          )}
        </div>
      )}

      {result.files_modified.length > 0 && (
        <div className="subagent-result-card-files">
          <div className="subagent-result-card-files-title">
            Files modified ({result.files_modified.length})
          </div>
          <ul className="subagent-result-card-files-list">
            {result.files_modified.map((f) => (
              <li key={f}>
                <code>{f}</code>
              </li>
            ))}
          </ul>
        </div>
      )}

      <div className="subagent-result-card-stats">
        <span className="subagent-result-card-stat">
          ⏱ {result.duration_secs.toFixed(1)}s
        </span>
        <span className="subagent-result-card-stat">
          💰 ${result.cost_usd.toFixed(4)}
        </span>
        <span className="subagent-result-card-stat">
          📥 {result.tokens_in.toLocaleString()} in
        </span>
        <span className="subagent-result-card-stat">
          📤 {result.tokens_out.toLocaleString()} out
        </span>
        {onViewLog && (
          <>
            <span className="subagent-result-card-stat-spacer" />
            <button className="subagent-result-card-log-btn" onClick={onViewLog}>
              View Full Log
            </button>
          </>
        )}
      </div>
    </div>
  );
};

function truncate(s: string, max: number): string {
  return s.length <= max ? s : `${s.slice(0, max)}…`;
}

function shortId(id: string): string {
  const m = id.match(/^subagent-(.+?)-(\d{10,})-(\d+)$/);
  if (!m) return id;
  const [, parent, , seq] = m;
  return `${parent}-${seq}`;
}
