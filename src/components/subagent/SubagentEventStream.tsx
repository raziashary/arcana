// SubagentEventStream — Phase 4.9 frontend.
//
// Live feed of SubagentEvent frames for a single subagent. Mirrors the
// chat message bubble style so users see the subagent thinking/streaming
// in real time. Includes per-event type rendering for tokens, thinking,
// tool calls, status transitions, errors, and final messages.

import React, { useEffect, useRef } from 'react';
import { SubagentEvent, useSubagentStore } from '../../stores/subagentStore';
import './SubagentEventStream.css';

interface SubagentEventStreamProps {
  subagentId: string;
  onClose?: () => void;
}

export const SubagentEventStream: React.FC<SubagentEventStreamProps> = ({
  subagentId,
  onClose,
}) => {
  const events = useSubagentStore((s) => s.events[subagentId] ?? []);
  const handle = useSubagentStore((s) => s.active[subagentId]);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Auto-scroll to the bottom on each new event.
    if (containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [events.length]);

  return (
    <div className="subagent-event-stream">
      <div className="subagent-event-stream-header">
        <span className="subagent-event-stream-title">
          {subagentId}
        </span>
        {handle && (
          <span className={`subagent-event-stream-status status-${handle.status}`}>
            {handle.status}
          </span>
        )}
        {onClose && (
          <button className="subagent-event-stream-close" onClick={onClose} title="Close">
            ✕
          </button>
        )}
      </div>

      <div className="subagent-event-stream-body" ref={containerRef}>
        {events.length === 0 && (
          <div className="subagent-event-stream-empty">
            Waiting for events from {subagentId}…
          </div>
        )}
        {events.map((evt, i) => (
          <EventLine key={i} event={evt} />
        ))}
      </div>
    </div>
  );
};

const EventLine: React.FC<{ event: SubagentEvent }> = ({ event }) => {
  switch (event.kind) {
    case 'token':
      return (
        <span className="subagent-event token">{event.text}</span>
      );
    case 'thinking':
      return (
        <div className="subagent-event thinking">
          <span className="subagent-event-icon">💭</span>
          <span className="subagent-event-text">{event.text}</span>
        </div>
      );
    case 'tool_start':
      return (
        <div className="subagent-event tool-start">
          <span className="subagent-event-icon">🔧</span>
          <span className="subagent-event-text">
            <strong>{event.tool}</strong> started
          </span>
        </div>
      );
    case 'tool_end':
      return (
        <div className={`subagent-event tool-end ${event.is_error ? 'error' : ''}`}>
          <span className="subagent-event-icon">{event.is_error ? '⚠️' : '✓'}</span>
          <span className="subagent-event-text">
            <strong>{event.tool}</strong> {event.is_error ? 'failed' : 'completed'}
            {event.output && (
              <pre className="subagent-event-output">{truncate(event.output, 400)}</pre>
            )}
          </span>
        </div>
      );
    case 'final':
      return (
        <div className="subagent-event final">
          <span className="subagent-event-icon">🏁</span>
          <span className="subagent-event-text">{event.text}</span>
        </div>
      );
    case 'cost':
      return (
        <div className="subagent-event cost">
          <span className="subagent-event-icon">💰</span>
          <span className="subagent-event-text">
            ${event.cost_usd.toFixed(4)} ·{' '}
            {event.tokens_in.toLocaleString()} in /{' '}
            {event.tokens_out.toLocaleString()} out
          </span>
        </div>
      );
    case 'status':
      return (
        <div className={`subagent-event status status-${event.status}`}>
          <span className="subagent-event-icon">●</span>
          <span className="subagent-event-text">
            {event.status}
            {event.message && <em> — {event.message}</em>}
          </span>
        </div>
      );
    case 'error':
      return (
        <div className="subagent-event error">
          <span className="subagent-event-icon">❌</span>
          <span className="subagent-event-text">{event.message}</span>
        </div>
      );
  }
};

function truncate(s: string, max: number): string {
  return s.length <= max ? s : `${s.slice(0, max)}…`;
}
