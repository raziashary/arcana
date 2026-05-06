// SubagentMonitor — Phase 4.9 frontend.
//
// Tree view of currently-active subagents. Groups by parent_slug, shows
// status, depth, runtime, and the last received event for each. Lets the
// user open the EventStream / ResultCard for any single subagent and abort
// runaway ones.

import React, { useEffect, useMemo, useState } from 'react';
import {
  SubagentHandle,
  SubagentStatus,
  useSubagentStore,
} from '../../stores/subagentStore';
import { SubagentEventStream } from './SubagentEventStream';
import { SubagentResultCard } from './SubagentResultCard';
import './SubagentMonitor.css';

const STATUS_ICON: Record<SubagentStatus, string> = {
  spawning:   '⏳',
  idle:       '⏸',
  running:    '🔄',
  completed:  '✅',
  failed:     '❌',
  terminated: '🛑',
};

interface SubagentMonitorProps {
  /** Filter to a single parent session (e.g. the current chat session). */
  parentSessionId?: string;
}

export const SubagentMonitor: React.FC<SubagentMonitorProps> = ({ parentSessionId }) => {
  const { active, results, startListening, refreshActive, abortSubagent, forget } =
    useSubagentStore();

  const [selectedId, setSelectedId] = useState<string | null>(null);

  useEffect(() => {
    void startListening();
    void refreshActive(parentSessionId);

    const poll = setInterval(() => {
      void refreshActive(parentSessionId);
    }, 5_000);

    return () => clearInterval(poll);
  }, [parentSessionId, startListening, refreshActive]);

  // Group active subagents by parent_slug.
  const grouped = useMemo(() => {
    const filtered = Object.values(active).filter(
      (h) =>
        !parentSessionId ||
        h.config.parent_session_id === parentSessionId,
    );
    const map = new Map<string, SubagentHandle[]>();
    for (const h of filtered) {
      const list = map.get(h.config.parent_slug) ?? [];
      list.push(h);
      map.set(h.config.parent_slug, list);
    }
    return Array.from(map.entries()).sort(([a], [b]) => a.localeCompare(b));
  }, [active, parentSessionId]);

  const totalActive = useMemo(
    () => Object.values(active).filter((h) => !['completed', 'failed', 'terminated'].includes(h.status)).length,
    [active],
  );

  return (
    <div className="subagent-monitor">
      <div className="subagent-monitor-header">
        <div className="subagent-monitor-title">
          <span className="subagent-monitor-title-icon">🤖</span>
          Subagent Monitor
        </div>
        <div className="subagent-monitor-stats">
          <span>{totalActive} active</span>
          <button
            className="subagent-monitor-refresh"
            onClick={() => void refreshActive(parentSessionId)}
            title="Refresh from backend"
          >
            ⟳
          </button>
        </div>
      </div>

      <div className="subagent-monitor-body">
        {grouped.length === 0 && (
          <div className="subagent-monitor-empty">
            No active subagents. Spawned subagents will appear here.
          </div>
        )}

        {grouped.map(([parent, list]) => (
          <div key={parent} className="subagent-group">
            <div className="subagent-group-header">
              <span className="subagent-group-parent">{parent}</span>
              <span className="subagent-group-count">{list.length}</span>
            </div>
            <ul className="subagent-list">
              {list.map((h) => (
                <li
                  key={h.id}
                  className={`subagent-row ${selectedId === h.id ? 'selected' : ''}`}
                  onClick={() => setSelectedId(h.id)}
                >
                  <span className={`subagent-row-status status-${h.status}`}>
                    {STATUS_ICON[h.status]}
                  </span>
                  <span className="subagent-row-id" title={h.id}>
                    {shortId(h.id)}
                  </span>
                  <span className="subagent-row-depth">d{h.config.depth}</span>
                  <span className="subagent-row-model">{h.config.model || 'default'}</span>
                  <span className="subagent-row-spacer" />
                  {!['completed', 'failed', 'terminated'].includes(h.status) && (
                    <button
                      className="subagent-row-abort"
                      onClick={(e) => {
                        e.stopPropagation();
                        void abortSubagent(h.id);
                      }}
                      title="Abort this subagent"
                    >
                      ✕
                    </button>
                  )}
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>

      {selectedId && (
        <div className="subagent-monitor-detail">
          {results[selectedId] ? (
            <SubagentResultCard
              result={results[selectedId]}
              onDismiss={() => {
                forget(selectedId);
                setSelectedId(null);
              }}
            />
          ) : (
            <SubagentEventStream subagentId={selectedId} onClose={() => setSelectedId(null)} />
          )}
        </div>
      )}
    </div>
  );
};

function shortId(id: string): string {
  // subagent-worker-backend-1745740800123-1 → worker-backend-…123-1
  const m = id.match(/^subagent-(.+?)-(\d{10,})-(\d+)$/);
  if (!m) return id;
  const [, parent, millis, seq] = m;
  return `${parent}-…${millis.slice(-3)}-${seq}`;
}
