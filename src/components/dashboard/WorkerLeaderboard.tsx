// WorkerLeaderboard widget — displays top performing workers with trending indicators
import React from 'react';
import { TrendingUp, TrendingDown } from 'lucide-react';
import { useDashboardStore } from '../../stores/dashboardStore';
import './widgets.css';

interface Worker {
  rank: number;
  name: string;
  successRate: number;
  avgDuration: number;
  costPerTask: number;
  successTrend?: number; // % change from last period
  efficiencyRank?: number; // cost per successful task
}

const WorkerLeaderboard: React.FC = () => {
  const tokenUsage = useDashboardStore(s => s.tokenUsage);

  // Group top_sessions by agent_type to produce per-agent cost and token totals.
  // Fall back to by_agent_type token-only data when session detail is not yet loaded.
  const workers: Worker[] = (() => {
    const sessions = tokenUsage?.top_sessions ?? [];
    const byType: Record<string, { tokens: number; cost: number; sessions: number }> = {};

    sessions.forEach(s => {
      const key = s.agent_type || 'unknown';
      if (!byType[key]) byType[key] = { tokens: 0, cost: 0, sessions: 0 };
      byType[key].tokens += s.tokens;
      byType[key].cost += s.cost_usd;
      byType[key].sessions += 1;
    });

    // Supplement with by_agent_type if sessions didn't cover a type
    Object.entries(tokenUsage?.by_agent_type ?? {}).forEach(([type, tokens]) => {
      if (!byType[type]) byType[type] = { tokens, cost: 0, sessions: 0 };
    });

    const entries = Object.entries(byType).sort(([, a], [, b]) => b.tokens - a.tokens).slice(0, 3);
    if (entries.length === 0) return [];

    return entries.map(([name, data], i) => ({
      rank: i + 1,
      name,
      successRate: 100,
      avgDuration: 0,
      costPerTask: data.sessions > 0 ? data.cost / data.sessions : 0,
      successTrend: undefined,
      efficiencyRank: i === 0 ? 1 : undefined,
    }));
  })();

  const medals = ['🥇', '🥈', '🥉'];

  const getTrendIcon = (trend?: number) => {
    if (!trend) return null;
    if (trend > 0) return <TrendingUp size={12} style={{ color: '#7FDBCA' }} />;
    if (trend < 0) return <TrendingDown size={12} style={{ color: '#FF6363' }} />;
    return null;
  };

  const getTrendLabel = (trend?: number) => {
    if (!trend) return '';
    return trend > 0 ? `+${trend}%` : `${trend}%`;
  };

  return (
    <div className="widget worker-leaderboard">
      <div className="widget-header">
        <h3>🏆 Worker Efficiency</h3>
      </div>
      <div className="widget-content">
        {workers.length === 0 ? (
          <div className="no-data-placeholder">No session data yet</div>
        ) : null}
        <div className="leaderboard-list">
          {workers.map((worker) => (
            <div key={worker.rank} className="leaderboard-item">
              <div className="rank-badge">{medals[worker.rank - 1]}</div>
              <div className="worker-info">
                <div className="worker-name">{worker.name}</div>
                <div className="worker-stats">
                  <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                    <span className="stat">{worker.successRate}% success</span>
                    {getTrendIcon(worker.successTrend)}
                    {worker.successTrend && (
                      <span style={{ fontSize: '10px', color: worker.successTrend > 0 ? '#7FDBCA' : '#FF6363' }}>
                        {getTrendLabel(worker.successTrend)}
                      </span>
                    )}
                  </div>
                  <span className="stat">{worker.avgDuration}m avg</span>
                  <span className="stat" style={{ color: '#ECC48D' }}>
                    ${worker.costPerTask.toFixed(2)}/task
                  </span>
                </div>
              </div>
              {worker.efficiencyRank === 1 && (
                <div style={{
                  padding: '2px 6px',
                  background: '#7FDBCA20',
                  border: '1px solid #7FDBCA',
                  borderRadius: '2px',
                  fontSize: '9px',
                  color: '#7FDBCA',
                  whiteSpace: 'nowrap',
                }}>
                  Most Efficient
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};

export default WorkerLeaderboard;
