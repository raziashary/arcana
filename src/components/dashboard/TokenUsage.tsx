// TokenUsage widget — displays today's token consumption with 7-day trend and model breakdown
import React, { useState } from 'react';
import { LineChart, Line, PieChart, Pie, Cell, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts';
import { ChevronDown } from 'lucide-react';
import { DashboardSummary, TokenUsageAnalytics } from '../../stores/dashboardStore';
import './widgets.css';

interface TokenUsageProps {
  summary: DashboardSummary;
  analytics: TokenUsageAnalytics | null;
}

interface DailyTokenData {
  date: string;
  tokens: number;
}

interface ModelUsageData {
  name: string;
  value: number;
  color: string;
}

const TokenUsage: React.FC<TokenUsageProps> = ({ summary, analytics }) => {
  const [showCharts, setShowCharts] = useState(false);

  const formatTokens = (tokens: number) => {
    if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(1)}M`;
    if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(1)}K`;
    return tokens.toString();
  };

  const getTrendIcon = (percent: number) => {
    if (percent > 0) return '↑';
    if (percent < 0) return '↓';
    return '→';
  };

  const getTrendColor = (percent: number) => {
    if (percent > 10) return 'trend-up';
    if (percent < -10) return 'trend-down';
    return 'trend-neutral';
  };

  // Mock 7-day token data
  const sevenDayData: DailyTokenData[] = [
    { date: 'Mon', tokens: 85000 },
    { date: 'Tue', tokens: 92000 },
    { date: 'Wed', tokens: 78000 },
    { date: 'Thu', tokens: 105000 },
    { date: 'Fri', tokens: 98000 },
    { date: 'Sat', tokens: 65000 },
    { date: 'Sun', tokens: summary.today_tokens },
  ];

  // Mock model usage breakdown
  const modelUsage: ModelUsageData[] = [
    { name: 'Claude 3.5 Sonnet', value: summary.today_tokens * 0.55, color: '#82AAFF' },
    { name: 'GPT-4o', value: summary.today_tokens * 0.30, color: '#7FDBCA' },
    { name: 'Gemini 2.0', value: summary.today_tokens * 0.15, color: '#FFCB8B' },
  ];

  // Mock top sessions
  const topSessions = [
    { id: 'session-001', tokens: summary.today_tokens * 0.35, name: 'Code Review Task' },
    { id: 'session-002', tokens: summary.today_tokens * 0.28, name: 'Documentation' },
    { id: 'session-003', tokens: summary.today_tokens * 0.20, name: 'Bug Analysis' },
    { id: 'session-004', tokens: summary.today_tokens * 0.17, name: 'Feature Design' },
  ];

  return (
    <>
      <div className="widget token-usage">
        <div className="widget-header">
          <h3>🪙 Tokens</h3>
          <span className="widget-label">Today</span>
        </div>
        <div className="widget-content">
          <div className="metric-value">{formatTokens(summary.today_tokens)}</div>
          <div className={`metric-change ${getTrendColor(summary.token_change_percent)}`}>
            <span className="trend-icon">{getTrendIcon(summary.token_change_percent)}</span>
            <span className="trend-percent">{Math.abs(summary.token_change_percent).toFixed(1)}%</span>
            <span className="trend-label">vs yesterday</span>
          </div>
          {analytics && (
            <div className="token-breakdown">
              <div className="breakdown-item">
                <span className="label">Prompt:</span>
                <span className="value">{formatTokens(analytics.prompt_tokens)}</span>
              </div>
              <div className="breakdown-item">
                <span className="label">Completion:</span>
                <span className="value">{formatTokens(analytics.completion_tokens)}</span>
              </div>
            </div>
          )}

          {/* View Charts Button */}
          <button
            onClick={() => setShowCharts(true)}
            style={{
              marginTop: '8px',
              width: '100%',
              padding: '6px 8px',
              background: '#0B3A5C',
              border: '1px solid #1A527A',
              borderRadius: '2px',
              color: '#637777',
              fontSize: '11px',
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              gap: '4px',
            }}
          >
            <span>7-Day Trend & Models</span>
            <ChevronDown size={12} />
          </button>
        </div>
      </div>

      {/* Charts Modal */}
      {showCharts && (
        <div style={{
          position: 'fixed',
          top: 0,
          left: 0,
          right: 0,
          bottom: 0,
          background: 'rgba(0, 0, 0, 0.7)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          zIndex: 1000,
          overflowY: 'auto',
        }}>
          <div style={{
            background: '#00233B',
            border: '1px solid #0D3652',
            borderRadius: '4px',
            padding: '20px',
            maxWidth: '900px',
            width: '95%',
            maxHeight: '90vh',
            overflowY: 'auto',
          }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
              <h3 style={{ margin: 0, color: '#D6DEEB' }}>Token Analytics</h3>
              <button
                onClick={() => setShowCharts(false)}
                style={{
                  background: 'none',
                  border: 'none',
                  color: '#637777',
                  cursor: 'pointer',
                  fontSize: '20px',
                }}
              >
                ✕
              </button>
            </div>

            {/* 7-Day Trend Chart */}
            <div style={{ marginBottom: '24px' }}>
              <h4 style={{ color: '#D6DEEB', marginBottom: '12px', fontSize: '12px' }}>7-Day Token Trend</h4>
              <ResponsiveContainer width="100%" height={250}>
                <LineChart data={sevenDayData}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#0D3652" />
                  <XAxis dataKey="date" stroke="#637777" style={{ fontSize: '12px' }} />
                  <YAxis stroke="#637777" style={{ fontSize: '12px' }} />
                  <Tooltip
                    contentStyle={{ background: '#011627', border: '1px solid #0D3652', borderRadius: '2px' }}
                    labelStyle={{ color: '#D6DEEB' }}
                    formatter={(value: any) => formatTokens(value as number)}
                  />
                  <Line
                    type="monotone"
                    dataKey="tokens"
                    stroke="#82AAFF"
                    strokeWidth={2}
                    dot={{ fill: '#82AAFF', r: 4 }}
                    activeDot={{ r: 6 }}
                  />
                </LineChart>
              </ResponsiveContainer>
            </div>

            {/* Model Breakdown Pie Chart */}
            <div style={{ marginBottom: '24px' }}>
              <h4 style={{ color: '#D6DEEB', marginBottom: '12px', fontSize: '12px' }}>Model Distribution</h4>
              <ResponsiveContainer width="100%" height={250}>
                <PieChart>
                  <Pie
                    data={modelUsage}
                    cx="50%"
                    cy="50%"
                    labelLine={false}
                    label={({ name, percent }: any) => `${name}: ${(percent * 100).toFixed(0)}%`}
                    outerRadius={80}
                    fill="#8884d8"
                    dataKey="value"
                  >
                    {modelUsage.map((entry, index) => (
                      <Cell key={`cell-${index}`} fill={entry.color} />
                    ))}
                  </Pie>
                  <Tooltip
                    contentStyle={{ background: '#011627', border: '1px solid #0D3652', borderRadius: '2px' }}
                    labelStyle={{ color: '#D6DEEB' }}
                    formatter={(value: any) => formatTokens(value as number)}
                  />
                </PieChart>
              </ResponsiveContainer>
            </div>

            {/* Top Sessions */}
            <div style={{ marginBottom: '16px' }}>
              <h4 style={{ color: '#D6DEEB', marginBottom: '12px', fontSize: '12px' }}>Top Sessions by Token Usage</h4>
              {topSessions.map((session, idx) => (
                <div key={session.id} style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  padding: '8px 0',
                  borderBottom: '1px solid #0D3652',
                  fontSize: '12px',
                }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <span style={{ color: '#637777' }}>#{idx + 1}</span>
                    <span style={{ color: '#D6DEEB' }}>{session.name}</span>
                  </div>
                  <span style={{ color: '#82AAFF' }}>{formatTokens(session.tokens)}</span>
                </div>
              ))}
            </div>

            <button
              onClick={() => setShowCharts(false)}
              style={{
                marginTop: '16px',
                width: '100%',
                padding: '8px',
                background: '#0B3A5C',
                border: '1px solid #1A527A',
                borderRadius: '2px',
                color: '#637777',
                cursor: 'pointer',
                fontSize: '12px',
              }}
            >
              Close
            </button>
          </div>
        </div>
      )}
    </>
  );
};

export default TokenUsage;
