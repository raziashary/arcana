// WorkerLeaderboard widget — displays top performing workers with trending indicators
import React from 'react';
import { TrendingUp, TrendingDown } from 'lucide-react';
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
  // TODO: Connect to worker analytics data
  const workers: Worker[] = [
    { rank: 1, name: 'worker-backend-01', successRate: 95, avgDuration: 45, costPerTask: 2.30, successTrend: 5, efficiencyRank: 1 },
    { rank: 2, name: 'worker-frontend-03', successRate: 92, avgDuration: 38, costPerTask: 1.85, successTrend: -2, efficiencyRank: 2 },
    { rank: 3, name: 'worker-mcp-01', successRate: 88, avgDuration: 52, costPerTask: 3.10, successTrend: 8, efficiencyRank: 3 },
  ];

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
