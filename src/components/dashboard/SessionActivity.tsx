// SessionActivity widget — displays session metrics
import React from 'react';
import { DashboardSummary } from '../../stores/dashboardStore';
import './widgets.css';

interface SessionActivityProps {
  summary: DashboardSummary;
}

const SessionActivity: React.FC<SessionActivityProps> = ({ summary }) => {
  return (
    <div className="widget session-activity">
      <div className="widget-header">
        <h3>📊 Sessions</h3>
        <span className="widget-label">Today</span>
      </div>
      <div className="widget-content">
        <div className="metric-value">{summary.sessions_today}</div>
        <div className="session-stats">
          <div className="stat-item">
            <span className="label">Active now:</span>
            <span className="value">{summary.active_sessions}</span>
          </div>
          <div className="stat-item">
            <span className="label">Messages:</span>
            <span className="value">{summary.messages_today}</span>
          </div>
          <div className="stat-item">
            <span className="label">Tool calls:</span>
            <span className="value">{summary.tool_calls_today}</span>
          </div>
        </div>
      </div>
    </div>
  );
};

export default SessionActivity;
